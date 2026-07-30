//! Origin-based permission manager.
//!
//! Tracks which dApp origins are authorized to access which accounts.
//! Supports grant, revoke, and pending-approval workflows.

use std::collections::{HashMap, HashSet};

/// Tracks which origins have permission to access which accounts.
#[derive(Default)]
pub struct PermissionManager {
    /// origin → set of permitted account IDs
    grants: HashMap<String, HashSet<String>>,
    /// origins pending user approval: origin → accounts
    pending: HashMap<String, Vec<String>>,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Grant an origin access to an account.
    pub fn grant(&mut self, origin: &str, account_id: &str) {
        self.grants
            .entry(origin.to_string())
            .or_default()
            .insert(account_id.to_string());
        self.pending.remove(origin);
    }

    /// Check if an origin has access to an account.
    pub fn is_permitted(&self, origin: &str, account_id: &str) -> bool {
        self.grants
            .get(origin)
            .map(|accts| accts.contains(account_id))
            .unwrap_or(false)
    }

    /// Queue a permission request for user approval.
    /// Returns true if this is a new request (not already pending/approved).
    pub fn request_permission(&mut self, origin: &str, accounts: Vec<String>) -> bool {
        if accounts.iter().all(|a| self.is_permitted(origin, a)) {
            return false;
        }
        let pending = self.pending.entry(origin.to_string()).or_default();
        let new_count = accounts.iter().filter(|a| !pending.contains(a)).count();
        pending.extend(accounts);
        new_count > 0
    }

    /// List pending permission requests for a given origin.
    pub fn pending_for(&self, origin: &str) -> Vec<String> {
        self.pending.get(origin).cloned().unwrap_or_default()
    }

    /// Revoke all permissions for an origin.
    pub fn revoke_all(&mut self, origin: &str) {
        self.grants.remove(origin);
        self.pending.remove(origin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grant_and_check() {
        let mut pm = PermissionManager::new();
        assert!(!pm.is_permitted("https://app.uniswap.org", "evm-0"));
        pm.grant("https://app.uniswap.org", "evm-0");
        assert!(pm.is_permitted("https://app.uniswap.org", "evm-0"));
    }

    #[test]
    fn test_revoke() {
        let mut pm = PermissionManager::new();
        pm.grant("https://dapp.example", "evm-1");
        pm.revoke_all("https://dapp.example");
        assert!(!pm.is_permitted("https://dapp.example", "evm-1"));
    }

    #[test]
    fn test_pending() {
        let mut pm = PermissionManager::new();
        assert!(pm.request_permission(
            "https://dapp.example",
            vec!["evm-0".into()]
        ));
        assert_eq!(pm.pending_for("https://dapp.example"), vec!["evm-0"]);
        // Second request for same origin+account should not signal new
        assert!(!pm.request_permission(
            "https://dapp.example",
            vec!["evm-0".into()]
        ));
    }

    #[test]
    fn test_multiple_origins() {
        let mut pm = PermissionManager::new();
        pm.grant("https://a.com", "acct-1");
        pm.grant("https://b.com", "acct-2");
        assert!(pm.is_permitted("https://a.com", "acct-1"));
        assert!(pm.is_permitted("https://b.com", "acct-2"));
        assert!(!pm.is_permitted("https://a.com", "acct-2"));
    }

    #[test]
    fn test_grant_clears_pending() {
        let mut pm = PermissionManager::new();
        pm.request_permission("https://dapp.example", vec!["evm-0".into()]);
        assert!(!pm.pending_for("https://dapp.example").is_empty());
        pm.grant("https://dapp.example", "evm-0");
        assert!(pm.pending_for("https://dapp.example").is_empty());
    }
}
