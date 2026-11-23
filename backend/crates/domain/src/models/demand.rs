/// How a farmer reached out: what the demand report counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeadChannel {
    Call,
    Zalo,
    Inquiry,
}

/// Which rows a report counts. Demo data never reaches a real partner's
/// report; a demo partner's counts only demo data; the admin's counts
/// everything there is, demo data while it is loaded (the page says so).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DemoScope {
    #[default]
    Real,
    Demo,
    All,
}

impl DemoScope {
    /// The one `is_demo` value counted, or `None` for both.
    pub fn only(self) -> Option<bool> {
        match self {
            Self::Real => Some(false),
            Self::Demo => Some(true),
            Self::All => None,
        }
    }
}

/// Leads in the report's `DemoScope`, counted by the database: the call or
/// Zalo taps of one report week in one ward about one category, or the
/// contact requests (not spam) one phone sent there. A report then reads
/// rows by ward and category, however many businesses and taps there are.
/// Never who, except a request's phone, which only tells two requests from
/// the same person apart.
#[derive(Debug, Clone)]
pub struct LeadCount {
    /// The report week it falls in, 0 for the oldest.
    pub week: usize,
    /// The ward its business lies in; `None` for a business with no position
    /// in reach of a ward, or deleted since.
    pub ward_code: Option<String>,
    /// The category of the service or product it was about; `None` for a tap
    /// on a shop's own page or a request with no service chosen.
    pub category_id: Option<i32>,
    /// The business, only for leads with no category: such a lead takes the
    /// category of its business's one live service, when it has just one.
    pub business_id: Option<i32>,
    pub channel: LeadChannel,
    pub count: i64,
    /// The requester's phone, for inquiries only.
    pub requester_phone: Option<String>,
}

/// A business with exactly one live service, and that service's category:
/// a tap on its shop page is about that service.
pub struct SoleCategory {
    pub business_id: i32,
    pub category_id: i32,
}

/// A search to store: the words (normalised), the category filter, the ward
/// (of the preset chosen, or of the searcher's position) and how many
/// results came back. Never who searched.
pub struct NewSearch {
    pub query: Option<String>,
    pub category_id: Option<i32>,
    pub ward_code: Option<String>,
    pub results: i64,
}

/// Stored searches as the demand report reads them, counted by the
/// database: those of one report week, category filter and ward that found
/// something (`query` is then `None`: only what found nothing is listed by
/// its words), or those for the same words that found nothing.
pub struct SearchCount {
    /// The report week, 0 for the oldest.
    pub week: usize,
    pub query: Option<String>,
    pub category_id: Option<i32>,
    pub ward_code: Option<String>,
    /// They found nothing.
    pub unmet: bool,
    pub count: i64,
}
