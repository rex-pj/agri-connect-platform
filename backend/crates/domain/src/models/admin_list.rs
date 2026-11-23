//! What an admin list is narrowed to and how it is ordered: the one set of
//! criteria every admin list's repository takes, so the search box, the
//! state chips, the sort and the page links mean the same on every list.

/// An order an admin list offers besides its own default (each list's
/// default stays what it was: the newest first, a queue's newest first, the
/// accounts in the order they were made, the providers verified first). Every
/// order ends with the id, so a page never repeats or skips a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminSort {
    Newest,
    Oldest,
    /// By name, A to Z (a queue: by the sender's name).
    Name,
}

impl AdminSort {
    pub const ALL: [Self; 3] = [Self::Newest, Self::Oldest, Self::Name];

    /// The code in a URL (`?sort=`).
    pub const fn code(self) -> &'static str {
        match self {
            Self::Newest => "newest",
            Self::Oldest => "oldest",
            Self::Name => "name",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.code() == code)
    }
}

/// A list's own state codes (`?state=`), each list naming the ones it has:
/// a listing shown or hidden by the admin.
pub const LIST_STATE_SHOWN: &str = "shown";
pub const LIST_STATE_HIDDEN: &str = "hidden";
/// A review not yet shown (waiting for the admin's approval).
pub const LIST_STATE_WAITING: &str = "waiting";
/// A provider on and verified, on and not yet verified (the queue to vet).
pub const LIST_STATE_VERIFIED: &str = "verified";
pub const LIST_STATE_UNVERIFIED: &str = "unverified";
/// An account, a provider or a partner turned on / off.
pub const LIST_STATE_ON: &str = "on";
pub const LIST_STATE_OFF: &str = "off";

/// The states of the services' and supplies' lists.
pub const LISTING_LIST_STATES: &[&str] = &[LIST_STATE_SHOWN, LIST_STATE_HIDDEN];
/// The states of the reviews' list: the waiting ones first, the queue.
pub const REVIEW_LIST_STATES: &[&str] = &[LIST_STATE_WAITING, LIST_STATE_SHOWN];
/// The states of the providers' list: the ones to vet first.
pub const PROVIDER_LIST_STATES: &[&str] =
    &[LIST_STATE_UNVERIFIED, LIST_STATE_VERIFIED, LIST_STATE_OFF];
/// The states of the accounts' and partners' lists.
pub const SWITCH_LIST_STATES: &[&str] = &[LIST_STATE_ON, LIST_STATE_OFF];
/// The states of the alerts' list, by their dates: in force today, not yet
/// begun, over, or switched off.
pub const ALERT_STATE_LIVE: &str = "live";
pub const ALERT_STATE_UPCOMING: &str = "upcoming";
pub const ALERT_STATE_ENDED: &str = "ended";
/// The states of a partner's contracts: in force, not yet begun, over, or
/// stopped with its partner switched off.
pub const CONTRACT_STATE_IN_FORCE: &str = "in_force";
pub const CONTRACT_STATE_UPCOMING: &str = "upcoming";
pub const CONTRACT_STATE_ENDED: &str = "ended";
pub const CONTRACT_STATE_PAUSED: &str = "paused";
pub const CONTRACT_LIST_STATES: &[&str] = &[
    CONTRACT_STATE_IN_FORCE,
    CONTRACT_STATE_UPCOMING,
    CONTRACT_STATE_ENDED,
    CONTRACT_STATE_PAUSED,
];
pub const ALERT_LIST_STATES: &[&str] = &[
    ALERT_STATE_LIVE,
    ALERT_STATE_UPCOMING,
    ALERT_STATE_ENDED,
    LIST_STATE_OFF,
];

/// The state among `states` that was typed (`?state=`), if any: anything
/// else is the whole list.
pub fn state_in(states: &[&'static str], typed: &str) -> Option<&'static str> {
    let typed = typed.trim();
    states.iter().copied().find(|s| *s == typed)
}

/// The criteria of an admin list; every one set narrows it. `state` is one
/// of that list's own codes (the page checks it against its list), `sort`
/// `None` the list's default order.
#[derive(Debug, Default, Clone)]
pub struct AdminFilter {
    pub provider_id: Option<i32>,
    /// One service type (a job post list): the type it asks for.
    pub category_id: Option<i32>,
    /// One area (a job post list).
    pub location_id: Option<i32>,
    /// One star rating (the reviews' list).
    pub rating: Option<i16>,
    /// Only what has no reply yet (the reviews' list).
    pub unanswered: bool,
    /// What the record is about, by code (the contact requests' list:
    /// `service` or `product`); the repository ignores a code it lacks.
    pub kind: Option<String>,
    /// One role (the accounts' list).
    pub role_id: Option<i32>,
    /// What a provider lacks: `no_services`, `no_products` or `no_account`.
    pub gap: Option<String>,
    /// Words of the address.
    pub area: Option<String>,
    /// Partners by their contracts: `in_force`, `expiring`, `ended` or `none`.
    pub contract: Option<String>,
    /// The day a contract filter is read against (set by the use case).
    pub today: Option<chrono::NaiveDate>,
    /// One sector, by code (the catalogue lists).
    pub sector: Option<String>,
    pub search: Option<String>,
    pub state: Option<&'static str>,
    pub sort: Option<AdminSort>,
    /// Made on or after / on or before this day (Vietnam time).
    pub created_from: Option<chrono::NaiveDate>,
    pub created_to: Option<chrono::NaiveDate>,
}

impl AdminFilter {
    /// The search, trimmed; `None` when blank.
    pub fn search(&self) -> Option<&str> {
        self.search
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
    }

    /// Only the provider: what a count by status of one provider's rows
    /// takes.
    pub fn of_provider(provider_id: Option<i32>) -> Self {
        Self {
            provider_id,
            ..Self::default()
        }
    }

    /// This filter narrowed to `state` too.
    pub fn in_state(&self, state: Option<&'static str>) -> Self {
        Self {
            state,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sort_reads_back_its_own_code() {
        for sort in AdminSort::ALL {
            assert_eq!(AdminSort::parse(sort.code()), Some(sort));
        }
        assert_eq!(AdminSort::parse("price"), None);
    }

    #[test]
    fn a_blank_search_is_no_search() {
        let filter = AdminFilter {
            search: Some("  ".into()),
            ..AdminFilter::default()
        };
        assert_eq!(filter.search(), None);
    }
}
