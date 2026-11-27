//! The articles' fixed lists: their kinds and their statuses, as stored.

/// What happens now: a disease, a price, a policy of the province.
pub const KIND_NEWS: &str = "news";
/// What stays true: how to treat a disease, the season's calendar.
pub const KIND_GUIDE: &str = "guide";
/// Both, in the tabs' order; also each one's locale key (`article.kind.<code>`).
pub const POST_KINDS: [&str; 2] = [KIND_NEWS, KIND_GUIDE];

/// Written, not shown yet.
pub const STATUS_DRAFT: &str = "draft";
/// Everyone reads it, while the feature is open.
pub const STATUS_PUBLISHED: &str = "published";
/// Taken down; it keeps the day it was published.
pub const STATUS_HIDDEN: &str = "hidden";
/// Every status, in the admin's chips' order.
pub const POST_STATUSES: [&str; 3] = [STATUS_DRAFT, STATUS_PUBLISHED, STATUS_HIDDEN];
