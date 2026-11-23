//! Roles and permissions are named by their code from the locale file
//! (`role.<code>`, `role_description.<code>`, `permission.<key>`), not by the
//! wording stored with them: the database keeps codes, the words follow the
//! locale. Pages and the JSON API both word their results here before
//! answering. A row the catalogue no longer has keeps its stored name.

use application::outputs::{
    AdminPagedResponse, AdminUserDetailDto, AdminUserDto, AlertDto, CodedDto, GrantDto,
    PermissionDto, PermissionMatrixDto, RoleDto, RoleGrantsDto, UserPermissionsDto,
};
use serde::Serialize;

use crate::locale::Locale;

/// A role's name by its code.
pub fn role_name(locale: &Locale, code: &str, stored: &str) -> String {
    locale.label(&format!("role.{code}"), stored)
}

/// A choice of a select or a switch: its code, its word, its icon.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CodeOption {
    pub value: String,
    pub label: String,
    pub icon: String,
}

/// The icon of a sector or an alert kind with none of its own.
pub const SECTOR_ICON_DEFAULT: &str = "fa-layer-group";

/// For whom and where an alert is, in words: "Chăn nuôi · Đồng Tháp",
/// "Mọi ngành · Cả vùng".
pub fn alert_reach(locale: &Locale, sectors: &[CodedDto], a: &AlertDto) -> String {
    let sector = match a.sector.as_deref() {
        Some(s) => sector_name(sectors, s),
        None => locale.text("sector.all"),
    };
    let place = a
        .province_name
        .clone()
        .unwrap_or_else(|| locale.text("alert.whole_region"));
    format!("{sector} · {place}")
}

/// A sector's name, from `sectors` (the stored ones); its code for one
/// no longer there.
pub fn sector_name(sectors: &[CodedDto], code: &str) -> String {
    sectors
        .iter()
        .find(|s| s.code == code)
        .map_or_else(|| code.to_owned(), |s| s.name.clone())
}

/// The sector of a row in words: its name, or "every sector" for none.
pub fn sector_label(
    state: &crate::app_state::AppState,
    sectors: &[CodedDto],
    code: Option<&str>,
) -> String {
    code.map_or_else(
        || state.locale().text("sector.all"),
        |c| sector_name(sectors, c),
    )
}

/// `sectors`, in their order, for a select or a switch.
pub fn sector_options(sectors: &[CodedDto]) -> Vec<CodeOption> {
    sectors
        .iter()
        .map(|s| CodeOption {
            value: s.code.clone(),
            label: s.name.clone(),
            icon: s
                .icon
                .clone()
                .unwrap_or_else(|| SECTOR_ICON_DEFAULT.to_owned()),
        })
        .collect()
}

fn permission_name(locale: &Locale, code: &str, stored: &str) -> String {
    locale.label(&format!("permission.{code}"), stored)
}

/// Replaces stored role and permission wording with the locale's.
pub trait Worded: Sized {
    fn worded(self, locale: &Locale) -> Self;
}

impl Worded for RoleDto {
    fn worded(mut self, locale: &Locale) -> Self {
        self.name = role_name(locale, &self.code, &self.name);
        self.description = locale
            .lookup(&format!("role_description.{}", self.code))
            .map(str::to_owned)
            .or(self.description);
        self
    }
}

impl Worded for GrantDto {
    fn worded(mut self, locale: &Locale) -> Self {
        self.name = permission_name(locale, &self.code, &self.name);
        self
    }
}

impl Worded for PermissionDto {
    fn worded(mut self, locale: &Locale) -> Self {
        self.name = permission_name(locale, &self.code, &self.name);
        self
    }
}

impl Worded for RoleGrantsDto {
    fn worded(self, locale: &Locale) -> Self {
        Self {
            role: self.role.worded(locale),
            permissions: self.permissions.worded(locale),
        }
    }
}

impl Worded for PermissionMatrixDto {
    fn worded(mut self, locale: &Locale) -> Self {
        self.roles = self.roles.worded(locale);
        for row in &mut self.rows {
            row.name = permission_name(locale, &row.code, &row.name);
        }
        self
    }
}

impl Worded for UserPermissionsDto {
    fn worded(mut self, locale: &Locale) -> Self {
        self.permissions = self.permissions.worded(locale);
        self
    }
}

impl Worded for AdminUserDto {
    fn worded(mut self, locale: &Locale) -> Self {
        self.role_name = role_name(locale, &self.role_code, &self.role_name);
        self
    }
}

impl Worded for AdminUserDetailDto {
    fn worded(mut self, locale: &Locale) -> Self {
        self.role_name = role_name(locale, &self.role_code, &self.role_name);
        self
    }
}

impl<T: Worded> Worded for Vec<T> {
    fn worded(self, locale: &Locale) -> Self {
        self.into_iter().map(|x| x.worded(locale)).collect()
    }
}

impl<T: Worded + Serialize> Worded for AdminPagedResponse<T> {
    fn worded(mut self, locale: &Locale) -> Self {
        self.items = self.items.worded(locale);
        self
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use application::catalogue::{PERMISSIONS, ROLES};

    use super::*;

    #[test]
    fn every_catalogue_role_and_permission_has_words() {
        let vi = Locale::vi().expect("vi.toml parses");
        let mut missing: Vec<String> = ROLES
            .iter()
            .flat_map(|r| {
                [
                    format!("role.{}", r.code),
                    format!("role_description.{}", r.code),
                ]
            })
            .chain(PERMISSIONS.iter().map(|p| format!("permission.{}", p.key)))
            .filter(|k| vi.lookup(k).is_none())
            .collect();
        missing.sort();
        assert!(missing.is_empty(), "vi.toml has no text for {missing:?}");
    }

    #[test]
    fn a_code_the_locale_does_not_know_keeps_its_stored_name() {
        let vi = Locale::vi().expect("vi.toml parses");
        let role = RoleDto {
            id: 1,
            code: "auditor".into(),
            name: "Auditor".into(),
            description: Some("Reads".into()),
            is_active: true,
        }
        .worded(&vi);
        assert_eq!(
            (role.name.as_str(), role.description.as_deref()),
            ("Auditor", Some("Reads"))
        );

        let admin = RoleDto {
            id: 2,
            code: "admin".into(),
            name: "admin".into(),
            description: None,
            is_active: true,
        }
        .worded(&vi);
        assert_eq!(admin.name, vi.text("role.admin"));
        assert!(admin.description.is_some());
    }
}
