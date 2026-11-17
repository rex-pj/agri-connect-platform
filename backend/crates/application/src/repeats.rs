//! Telling a visitor's repeat from a first time, for the demand figures: the
//! same search again, the same call button tapped again. Fingerprints are
//! hashes kept in memory only, never stored, and forgotten once their window
//! has passed (`forget_older_than`, called by the server every few minutes).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// What was seen lately. Cloning shares the memory.
#[derive(Clone, Default)]
pub struct RepeatFilter {
    seen: Arc<Mutex<HashMap<String, Instant>>>,
}

impl RepeatFilter {
    /// True the first time `fingerprint` is seen within `window`: one lookup,
    /// however many are remembered. What is older stays until
    /// `forget_older_than` drops it; it is not a repeat meanwhile.
    pub fn first_in_window(&self, fingerprint: String, window: Duration) -> bool {
        let now = Instant::now();
        let Ok(mut seen) = self.seen.lock() else {
            return true;
        };
        match seen.get(&fingerprint) {
            Some(at) if now.duration_since(*at) < window => false,
            _ => {
                seen.insert(fingerprint, now);
                true
            }
        }
    }

    /// Forgets what was seen longer ago than `window`, so the memory holds
    /// one window of visitors and no more. The server calls it now and then:
    /// doing it on every tap would walk the whole map under the lock.
    pub fn forget_older_than(&self, window: Duration) {
        let now = Instant::now();
        if let Ok(mut seen) = self.seen.lock() {
            seen.retain(|_, at| now.duration_since(*at) < window);
            seen.shrink_to_fit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeat_within_the_window_is_not_a_first_time() {
        let filter = RepeatFilter::default();
        let hour = Duration::from_secs(3600);
        assert!(filter.first_in_window("a".into(), hour));
        assert!(!filter.first_in_window("a".into(), hour));
        assert!(filter.first_in_window("b".into(), hour));
        // A window that has passed is a first time again.
        assert!(filter.first_in_window("a".into(), Duration::ZERO));
    }

    #[test]
    fn what_is_past_its_window_is_forgotten() {
        let filter = RepeatFilter::default();
        let remembered = || filter.seen.lock().map(|s| s.len()).ok();
        assert!(filter.first_in_window("a".into(), Duration::from_secs(3600)));
        filter.forget_older_than(Duration::from_secs(3600));
        assert_eq!(remembered(), Some(1), "still in its window");
        filter.forget_older_than(Duration::ZERO);
        assert_eq!(remembered(), Some(0));
    }
}
