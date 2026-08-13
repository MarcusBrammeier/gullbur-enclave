//! Extension-relay rate limiter — sliding-window per-origin.
use std::collections::HashMap;
use std::time::Instant;

const WINDOW: std::time::Duration = std::time::Duration::from_secs(60);
const MAX_REQUESTS: usize = 30;
const MAX_PENDING_APPROVALS: usize = 3;

pub struct RateLimiter {
    windows: HashMap<String, Vec<Instant>>,
    pending_approvals: HashMap<String, usize>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            pending_approvals: HashMap::new(),
        }
    }

    /// Returns Ok(()) if the origin can proceed, Err(msg) if rate-limited.
    pub fn check(&mut self, origin: &str) -> Result<(), String> {
        let now = Instant::now();
        let entries = self.windows.entry(origin.to_string()).or_default();
        entries.retain(|t| now.duration_since(*t) < WINDOW);
        if entries.len() >= MAX_REQUESTS {
            return Err(format!("Rate limited: {} req/min max", MAX_REQUESTS));
        }
        entries.push(now);
        Ok(())
    }

    pub fn reserve_approval(&mut self, origin: &str) -> Result<(), String> {
        let count = self
            .pending_approvals
            .entry(origin.to_string())
            .or_insert(0);
        if *count >= MAX_PENDING_APPROVALS {
            return Err(format!("Max {} pending approvals", MAX_PENDING_APPROVALS));
        }
        *count += 1;
        Ok(())
    }

    pub fn release_approval(&mut self, origin: &str) {
        if let Some(c) = self.pending_approvals.get_mut(origin) {
            *c = c.saturating_sub(1);
        }
    }
}
