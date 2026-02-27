use std::sync::atomic::{AtomicBool, Ordering};

/// Emergency stop flag. Thread-safe, shareable via Arc<EStop>.
pub struct EStop {
    stopped: AtomicBool,
}

impl EStop {
    pub fn new() -> Self {
        Self {
            stopped: AtomicBool::new(false),
        }
    }

    /// Activate the emergency stop. Idempotent.
    /// Public API: intended for external callers (e.g., signal handlers, admin commands).
    pub fn halt(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    /// Returns true if emergency stop has been activated.
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
}

impl Default for EStop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn initially_not_stopped() {
        assert!(!EStop::new().is_stopped());
    }

    #[test]
    fn halt_sets_stopped() {
        let e = EStop::new();
        e.halt();
        assert!(e.is_stopped());
    }

    #[test]
    fn halt_idempotent() {
        let e = EStop::new();
        e.halt();
        e.halt();
        assert!(e.is_stopped());
    }

    #[test]
    fn arc_shared_halt() {
        let e = Arc::new(EStop::new());
        let e2 = Arc::clone(&e);
        e2.halt();
        assert!(e.is_stopped());
    }
}
