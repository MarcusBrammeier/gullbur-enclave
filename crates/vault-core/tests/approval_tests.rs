//! Extra tests for vault-core approval paths.
//! Coverage for the least-tested critical path — approval queue gating.

use std::collections::HashMap;
use vault_core::approval::{ApprovalQueue, ApprovalResponse};

fn make_summary() -> HashMap<String, String> {
    let mut h = HashMap::new();
    h.insert("network".into(), "ethereum".into());
    h
}

#[test]
fn test_approval_multiple_queued() {
    let mut queue = ApprovalQueue::new();
    let (id1, _) = queue.submit("dapp1", "sign", "Sign tx", make_summary());
    let (id2, _) = queue.submit("dapp2", "sign", "Another tx", make_summary());
    assert_eq!(queue.list_pending().len(), 2);
    assert_ne!(id1, id2);
    assert!(queue.respond(&id1, true, None));
    assert_eq!(queue.list_pending().len(), 1);
}

#[test]
fn test_approval_deny_reason() {
    let mut queue = ApprovalQueue::new();
    let (id, rx) = queue.submit("dapp", "sign", "Dangerous tx", make_summary());
    assert!(queue.respond(&id, false, Some("User cancelled".into())));
    match rx.blocking_recv() {
        Ok(ApprovalResponse::Denied(msg)) => assert_eq!(msg, "User cancelled"),
        other => panic!("expected Denied, got {other:?}"),
    }
}

#[test]
fn test_approval_respond_unknown_id() {
    let mut queue = ApprovalQueue::new();
    queue.submit("dapp", "sign", "Sign tx", make_summary());
    assert!(!queue.respond("approval-999", true, None));
}

#[test]
fn test_clean_stale_removes_dropped() {
    let mut queue = ApprovalQueue::new();
    let (id, rx) = queue.submit("dapp", "sign", "Drop test", make_summary());
    drop(rx);
    assert_eq!(queue.clean_stale(), 1);
    assert!(!queue.respond(&id, true, None));
    assert!(queue.list_pending().is_empty());
}
