//! In-process `RateLimiter` (governor, GCRA). Correct for the single VM we
//! run; with more than one instance each would count separately, and a shared
//! adapter (e.g. Redis) would replace this behind the same port.

use std::{net::IpAddr, num::NonZeroU32};

use application::ports::RateLimiter;
use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota};

pub struct InProcessRateLimiter {
    inner: governor::RateLimiter<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock>,
}

impl InProcessRateLimiter {
    pub fn per_minute(budget: NonZeroU32) -> Self {
        Self {
            inner: governor::RateLimiter::keyed(Quota::per_minute(budget)),
        }
    }
}

impl RateLimiter for InProcessRateLimiter {
    fn allow(&self, key: IpAddr) -> bool {
        self.inner.check_key(&key).is_ok()
    }

    fn forget_idle(&self) {
        self.inner.retain_recent();
        self.inner.shrink_to_fit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_is_per_client() {
        let limiter = InProcessRateLimiter::per_minute(NonZeroU32::MIN.saturating_add(1));
        let (a, b) = (IpAddr::from([1, 1, 1, 1]), IpAddr::from([2, 2, 2, 2]));
        assert!(limiter.allow(a) && limiter.allow(a));
        assert!(
            !limiter.allow(a),
            "third request in the minute must be refused"
        );
        assert!(limiter.allow(b), "another client keeps its own budget");
    }
}
