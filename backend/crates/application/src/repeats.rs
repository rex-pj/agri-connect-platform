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

/// How many times each key happened within a window that starts at its first
/// time: wrong passwords from one client for one account, job posts from one
/// client in a day. Keys are hashes, kept in memory only, never stored; a
/// restart forgets them (one server, like the rate limiters). Cloning shares
/// the memory.
#[derive(Clone, Default)]
pub struct WindowCounter {
    counts: Arc<Mutex<HashMap<String, (Instant, u32)>>>,
}

impl WindowCounter {
    /// How many times `key` happened in its current window (none once the
    /// window has passed).
    pub fn count(&self, key: &str, window: Duration) -> u32 {
        let now = Instant::now();
        let Ok(counts) = self.counts.lock() else {
            return 0;
        };
        match counts.get(key) {
            Some((since, n)) if now.duration_since(*since) < window => *n,
            _ => 0,
        }
    }

    /// Counts one more time for `key`; the count, this one included. A key
    /// whose window has passed starts a new one.
    pub fn hit(&self, key: String, window: Duration) -> u32 {
        let now = Instant::now();
        let Ok(mut counts) = self.counts.lock() else {
            return 1;
        };
        let entry = counts.entry(key).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
            *entry = (now, 0);
        }
        entry.1 = entry.1.saturating_add(1);
        entry.1
    }

    /// Takes one of `max` slots for `key` in its window: `true`, counted,
    /// when one was free; `false`, nothing counted, when all are taken.
    /// Checked and counted in one step, so callers at the same moment never
    /// all see the same free slot (`count` then `hit` would let them).
    pub fn take(&self, key: &str, window: Duration, max: u32) -> bool {
        let now = Instant::now();
        let Ok(mut counts) = self.counts.lock() else {
            return true;
        };
        let entry = counts.entry(key.to_owned()).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
            *entry = (now, 0);
        }
        if entry.1 >= max {
            return false;
        }
        entry.1 += 1;
        true
    }

    /// Gives back a slot `take` took, for an attempt that came to nothing
    /// (refused, or failed to store).
    pub fn give_back(&self, key: &str) {
        if let Ok(mut counts) = self.counts.lock() {
            if let Some(entry) = counts.get_mut(key) {
                entry.1 = entry.1.saturating_sub(1);
            }
        }
    }

    /// Forgets `key` (the right password clears the wrong ones before it).
    pub fn clear(&self, key: &str) {
        if let Ok(mut counts) = self.counts.lock() {
            counts.remove(key);
        }
    }

    /// Forgets the keys whose window has passed; the server calls it now and
    /// then, as for `RepeatFilter`.
    pub fn forget_older_than(&self, window: Duration) {
        let now = Instant::now();
        if let Ok(mut counts) = self.counts.lock() {
            counts.retain(|_, (since, _)| now.duration_since(*since) < window);
            counts.shrink_to_fit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_counter_counts_within_its_window_and_starts_again_after() {
        let counter = WindowCounter::default();
        let hour = Duration::from_secs(3600);
        assert_eq!(counter.count("a", hour), 0);
        assert_eq!(counter.hit("a".into(), hour), 1);
        assert_eq!(counter.hit("a".into(), hour), 2);
        assert_eq!(counter.count("a", hour), 2);
        assert_eq!(counter.count("b", hour), 0, "each key on its own");
        // A window that has passed starts a new count.
        assert_eq!(counter.count("a", Duration::ZERO), 0);
        assert_eq!(counter.hit("a".into(), Duration::ZERO), 1);
        counter.clear("a");
        assert_eq!(counter.count("a", hour), 0);
        counter.hit("c".into(), hour);
        counter.forget_older_than(Duration::ZERO);
        assert_eq!(counter.count("c", hour), 0);
    }

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

    #[test]
    fn a_slot_is_taken_only_while_one_is_free_and_can_be_given_back() {
        let counter = WindowCounter::default();
        let day = Duration::from_secs(86_400);
        assert!(counter.take("k", day, 2));
        assert!(counter.take("k", day, 2));
        assert!(!counter.take("k", day, 2), "both taken");
        assert_eq!(counter.count("k", day), 2, "a refusal is not counted");
        counter.give_back("k");
        assert!(counter.take("k", day, 2), "a slot given back is free again");
        assert!(!counter.take("k", day, 0), "no slot at all");
    }

    #[test]
    fn many_at_once_never_take_more_slots_than_there_are() {
        let counter = WindowCounter::default();
        let day = Duration::from_secs(86_400);
        let taken: usize = std::thread::scope(|s| {
            let runs: Vec<_> = (0..32)
                .map(|_| s.spawn(|| counter.take("k", day, 5)))
                .collect();
            runs.into_iter()
                .map(|r| usize::from(r.join().unwrap_or(false)))
                .sum()
        });
        assert_eq!(taken, 5);
    }
}
