//! ID generation utilities

use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique ID for RPC requests
pub fn generate_id() -> String {
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("req_{}", count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_ids() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }
}
