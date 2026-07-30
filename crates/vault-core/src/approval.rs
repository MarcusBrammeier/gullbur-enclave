//! Approval queue — user-consent gating for state-changing operations.
//!
//! When a dApp or extension requests a state-changing operation
//! (eth_sendTransaction, personal_sign, vault_executeBatch, etc.),
//! the operation is queued here. The Tauri/Svelte UI polls for pending
//! approvals and presents a non-bypassable modal. The operation is only
//! executed after the user explicitly approves.
//!
//! # Architecture
//!
//! ```text
//! dApp → Extension → Relay → vault-core IPC     Tauri App (Svelte)
//!                              │                      │
//!                              ▼                      │
//!                         ApprovalQueue ─── poll ──────┘
//!                              │             respond (approve/deny)
//!                              ▼
//!                         Execute operation
//! ```
//!
//! # Thread Safety
//!
//! The queue is designed to be shared via `Arc<RwLock<ApprovalQueue>>`.
//! Submitters get a `tokio::sync::oneshot::Receiver` that resolves
//! when the UI responds. The IPC handler blocks on the receiver,
//! effectively pausing the WebSocket request until the user acts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::oneshot;

/// A single pending approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique request ID
    pub id: String,
    /// dApp origin URL (e.g., "https://app.uniswap.org")
    pub origin: String,
    /// The vault method being requested (e.g., "vault.sign_transaction")
    pub method: String,
    /// Human-readable description for the UI
    pub description: String,
    /// Key metadata fields from the params (truncated for UI)
    pub summary: HashMap<String, String>,
    /// Unix timestamp when the request was created
    pub created_at: u64,
}

/// Response from the UI to an approval request.
#[derive(Debug, Clone)]
pub enum ApprovalResponse {
    /// User approved — proceed with the operation
    Approved,
    /// User denied — return an error to the caller
    Denied(String),
}

/// The shared approval queue.
///
/// IPC handlers submit requests and block until the UI responds.
/// The Tauri commands list pending requests and respond to them.
pub struct ApprovalQueue {
    /// Pending requests: id → (request, response channel)
    pending: HashMap<String, (ApprovalRequest, oneshot::Sender<ApprovalResponse>)>,
    /// Monotonic ID counter
    next_id: u64,
}

impl ApprovalQueue {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            next_id: 1,
        }
    }

    /// Submit an approval request and get a receiver that resolves
    /// when the UI responds. The caller should `await` the receiver
    /// to block until the user acts.
    ///
    /// Returns (request_id, receiver) so the caller can log the id
    /// while waiting.
    pub fn submit(
        &mut self,
        origin: &str,
        method: &str,
        description: &str,
        summary: HashMap<String, String>,
    ) -> (String, oneshot::Receiver<ApprovalResponse>) {
        let id = format!("approval-{}", self.next_id);
        self.next_id += 1;

        let request = ApprovalRequest {
            id: id.clone(),
            origin: origin.to_string(),
            method: method.to_string(),
            description: description.to_string(),
            summary,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id.clone(), (request, tx));
        (id, rx)
    }

    /// List all pending approval requests (for the UI to poll).
    pub fn list_pending(&self) -> Vec<ApprovalRequest> {
        self.pending
            .values()
            .map(|(req, _)| req.clone())
            .collect()
    }

    /// Respond to a pending approval request.
    ///
    /// Returns `true` if the request was found and responded to,
    /// `false` if it was already handled or doesn't exist.
    pub fn respond(&mut self, id: &str, approved: bool, reason: Option<String>) -> bool {
        if let Some((_, tx)) = self.pending.remove(id) {
            let response = if approved {
                ApprovalResponse::Approved
            } else {
                ApprovalResponse::Denied(
                    reason.unwrap_or_else(|| "User denied the request".into()),
                )
            };
            // oneshot send can fail if the receiver was dropped (timeout).
            // That's fine — it means the caller gave up waiting.
            let _ = tx.send(response);
            true
        } else {
            false
        }
    }

    /// Clean up stale requests (caller side dropped).
    /// Returns the number of cleaned requests.
    pub fn clean_stale(&mut self) -> usize {
        let before = self.pending.len();
        self.pending.retain(|_, (_, tx)| !tx.is_closed());
        before - self.pending.len()
    }
}

impl Default for ApprovalQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submit_and_list() {
        let mut queue = ApprovalQueue::new();
        let mut summary = HashMap::new();
        summary.insert("network".into(), "ethereum".into());

        let (id, _rx) = queue.submit(
            "https://app.uniswap.org",
            "vault.sign_transaction",
            "Confirm transaction",
            summary,
        );

        let pending = queue.list_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, id);
        assert_eq!(pending[0].origin, "https://app.uniswap.org");
        assert_eq!(pending[0].method, "vault.sign_transaction");
        assert_eq!(pending[0].description, "Confirm transaction");
    }

    #[test]
    fn test_approve() {
        let mut queue = ApprovalQueue::new();
        let (id, rx) = queue.submit(
            "https://dapp.example",
            "vault_executeBatch",
            "Execute batch",
            HashMap::new(),
        );

        // Approve
        assert!(queue.respond(&id, true, None));
        // Should resolve immediately
        match rx.blocking_recv() {
            Ok(ApprovalResponse::Approved) => {}
            other => panic!("Expected Approved, got {:?}", other),
        }

        // List should be empty now
        assert!(queue.list_pending().is_empty());
    }

    #[test]
    fn test_deny() {
        let mut queue = ApprovalQueue::new();
        let (id, rx) = queue.submit(
            "https://evil.com",
            "personal_sign",
            "Sign message",
            HashMap::new(),
        );

        // Deny with reason
        assert!(queue.respond(&id, false, Some("Untrusted origin".into())));
        match rx.blocking_recv() {
            Ok(ApprovalResponse::Denied(msg)) => assert_eq!(msg, "Untrusted origin"),
            other => panic!("Expected Denied, got {:?}", other),
        }
    }

    #[test]
    fn test_respond_unknown_id() {
        let mut queue = ApprovalQueue::new();
        assert!(!queue.respond("approval-999", true, None));
    }

    #[test]
    fn test_dropped_receiver() {
        let mut queue = ApprovalQueue::new();
        let (id, rx) = queue.submit("https://dapp.com", "eth_sendTransaction", "Send", HashMap::new());
        drop(rx); // caller gives up

        // Clean should remove it
        assert_eq!(queue.clean_stale(), 1);
        assert!(queue.list_pending().is_empty());
        // Responding after clean is a no-op
        assert!(!queue.respond(&id, true, None));
    }

    #[test]
    fn test_multiple_pending() {
        let mut queue = ApprovalQueue::new();
        let (id1, _) = queue.submit("a", "m1", "d1", HashMap::new());
        let (id2, _) = queue.submit("b", "m2", "d2", HashMap::new());
        let (id3, _) = queue.submit("c", "m3", "d3", HashMap::new());

        assert_eq!(queue.list_pending().len(), 3);

        queue.respond(&id1, true, None);
        assert_eq!(queue.list_pending().len(), 2);

        queue.respond(&id3, false, None);
        assert_eq!(queue.list_pending().len(), 1);

        let pending = queue.list_pending();
        assert_eq!(pending[0].id, id2);
    }
}