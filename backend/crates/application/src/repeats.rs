//! Telling a visitor's repeat from a first time, for the demand figures: the
//! same search again, the same call button tapped again. Fingerprints are
//! hashes kept in memory only, never stored, and forgotten once their window
//! has passed.

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
    /// True the first time `fingerprint` is seen within `window`; older ones
    /// are forgotten as it goes, so the memory stays small. One filter is
    /// asked with one window.
    pub fn first_in_window(&self, fingerprint: String, window: Duration) -> bool {
        let now = Instant::now();
        let Ok(mut seen) = self.seen.lock() else {
            return true;
        };
        seen.retain(|_, at| now.duration_since(*at) < window);
        if seen.contains_key(&fingerprint) {
            return false;
        }
        seen.insert(fingerprint, now);
        true
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
        // A window that has passed forgets it.
        assert!(filter.first_in_window("a".into(), Duration::ZERO));
    }
}
