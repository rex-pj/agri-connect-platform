//! The top of a service's or a supply's admin page: one header (its state,
//! hide / show, the public page; the photo and deletion in its "more" menu)
//! and the confirmations it opens. The two pages differ only in words and
//! sheet ids (`ListingWords`).

use domain::Error;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    pages::admin::{url, Badge, Confirm, RecordAction, RecordHeader, Says, Toggle},
};

/// What a kind of listing is called on its page: locale keys and the ids of
/// its confirmation sheets.
pub struct ListingWords {
    pub icon: &'static str,
    pub hide: &'static str,
    pub hide_title: &'static str,
    pub what: &'static str,
    pub delete: &'static str,
    pub delete_title: &'static str,
    pub delete_note: &'static str,
    pub hide_sheet: &'static str,
    pub delete_sheet: &'static str,
}

pub const SERVICE_WORDS: ListingWords = ListingWords {
    icon: "fa-screwdriver-wrench",
    hide: "admin.hide_service",
    hide_title: "admin.hide_service_title",
    what: "admin.service_what",
    delete: "dashboard.delete_service",
    delete_title: "admin.delete_service_title",
    delete_note: "admin.delete_service_note",
    hide_sheet: "an-dich-vu",
    delete_sheet: "xoa-dich-vu",
};

pub const PRODUCT_WORDS: ListingWords = ListingWords {
    icon: "fa-box-open",
    hide: "admin.hide_product",
    hide_title: "admin.hide_product_title",
    what: "admin.product_what",
    delete: "dashboard.delete_product",
    delete_title: "admin.delete_product_title",
    delete_note: "admin.delete_product_note",
    hide_sheet: "an-san-pham",
    delete_sheet: "xoa-san-pham",
};

/// The photo's removal sheet, the same on both pages.
const PHOTO_SHEET: &str = "go-anh";

/// One listing as its header needs it.
pub struct ListingState<'a> {
    pub name: &'a str,
    pub is_active: bool,
    pub image_file_id: Option<Uuid>,
    /// Its public page, while farmers can open it.
    pub public_href: Option<String>,
    pub back_href: String,
    pub back_label: String,
    pub status_action: String,
    pub delete_action: String,
    pub image_delete_action: String,
}

/// The header and the confirmations of one listing's page. Hiding is the
/// one button for a shown listing (the public page beside it); showing it
/// again is a one-button form. The menu holds the photo (view, remove) and
/// the deletion, last.
pub fn listing_controls(
    state: &AppState,
    words: &ListingWords,
    l: ListingState<'_>,
) -> Result<(RecordHeader, Vec<Confirm>), Error> {
    let locale = state.locale();
    let text = |key: &str| locale.text(key);
    let named = |key: &str| locale.text_with(key, &[("name", &l.name)]);

    let mut actions = Vec::new();
    let mut confirms = Vec::new();
    let (toggle, hide) = Toggle::visibility(
        state,
        l.is_active,
        l.status_action,
        words.hide_sheet,
        text(words.hide),
        named(words.hide_title),
        Says::Hide(text(words.what)),
    )
    .split();
    actions.push(toggle);
    confirms.extend(hide);
    if let Some(href) = l.public_href {
        actions.push(RecordAction::link(
            text("admin.public_page"),
            "fa-arrow-up-right-from-square",
            href,
        ));
    }

    let mut menu = Vec::new();
    if let Some(file) = l.image_file_id {
        menu.push(RecordAction::link(
            text("admin.view_photo_short"),
            "fa-image",
            url("file", &[("id", &file.to_string())])?,
        ));
        menu.push(RecordAction::sheet(
            text("admin.remove_photo"),
            "fa-image",
            PHOTO_SHEET,
            "danger",
        ));
        confirms.push(
            Confirm::delete(
                PHOTO_SHEET,
                named("admin.remove_photo_title"),
                "fa-image",
                l.image_delete_action,
                text("admin.remove_photo"),
            )
            .noting(text("admin.remove_photo_note")),
        );
    }
    menu.push(RecordAction::sheet(
        text(words.delete),
        "fa-trash",
        words.delete_sheet,
        "danger",
    ));
    confirms.push(
        Confirm::delete(
            words.delete_sheet,
            named(words.delete_title),
            "fa-trash",
            l.delete_action,
            text("common.confirm_delete"),
        )
        .noting(text(words.delete_note)),
    );

    let header = RecordHeader {
        title: l.name.to_owned(),
        icon: words.icon.to_owned(),
        sub: None,
        back_href: l.back_href,
        back_label: l.back_label,
        badges: vec![Badge::new("visibility", l.is_active)],
        actions,
        menu,
    };
    Ok((header, confirms))
}
