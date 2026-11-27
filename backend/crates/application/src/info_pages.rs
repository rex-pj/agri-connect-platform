//! The information pages' fixed lists: the footer's two groups. Who sees a
//! page is a `features::Stage`, stored by its code.

/// Under "about us" in the footer: who runs the platform, how to reach it.
pub const FOOTER_GROUP_ABOUT: &str = "about";
/// Under "policies": the terms, the policies, the rules and procedures.
pub const FOOTER_GROUP_POLICY: &str = "policy";
/// Both, in the footer's order; also each one's locale key
/// (`info.group.<code>`).
pub const FOOTER_GROUPS: [&str; 2] = [FOOTER_GROUP_ABOUT, FOOTER_GROUP_POLICY];

/// The addresses of the pages a provider accepts when signing up, once the
/// admin made them and turned them on: the terms of use and the personal
/// data policy. Also the pages the farmers' forms point to.
pub const CODE_TERMS: &str = "dieu-khoan-su-dung";
pub const CODE_PERSONAL_DATA: &str = "chinh-sach-du-lieu-ca-nhan";
pub const SIGNUP_CONSENT: [&str; 2] = [CODE_TERMS, CODE_PERSONAL_DATA];
