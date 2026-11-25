//! /admin/categories and /admin/locations: create, edit, hide and delete
//! through page forms, with every broken rule reported on its field.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, json_body, post_form, register_provider,
    send, sign_in, Env,
};
use sea_orm::{ConnectionTrait, Statement};

fn location_of(res: &axum::response::Response<Body>) -> String {
    res.headers()[header::LOCATION].to_str().unwrap().to_owned()
}

async fn page(e: &Env, path: &str, cookie: &str) -> String {
    body_text(send(&e.router, get_page(path, Some(cookie))).await).await
}

async fn last_audit(e: &Env, entity_type: &str) -> String {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            format!(
                "SELECT action || ': ' || coalesce(reason, '') AS line FROM audit_logs \
                 WHERE entity_type = '{entity_type}' ORDER BY id DESC LIMIT 1"
            ),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "line")
        .unwrap()
}

const NEW_CATEGORY: &[(&str, &str)] = &[
    ("category_type", "service"),
    ("name", "Bơm nước"),
    ("slug", "bom-nuoc"),
    ("icon", "fa-solid fa-faucet"),
    ("color", ""),
    ("description", "Bơm tưới, bơm tiêu"),
    ("display_order", "12"),
];

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_category_form_reports_every_broken_rule_and_keeps_what_was_typed() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    let res = send(
        &e.router,
        post_form(
            "/admin/categories",
            &[
                ("category_type", "weather"),
                ("name", "  "),
                ("slug", "Bơm Nước"),
                ("description", "Tưới tiêu"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Vui lòng nhập tên."));
    assert!(html.contains("Đường dẫn chỉ gồm chữ thường không dấu"));
    assert!(html.contains("Vui lòng chọn dịch vụ hay vật tư."));
    assert!(html.contains(r#"value="Bơm Nước""#), "typed slug kept");
    assert!(
        html.contains(">Tưới tiêu</textarea>"),
        "typed description kept"
    );
    assert!(
        html.contains(r#"data-open="true""#),
        "the create sheet comes back open"
    );
    assert!(
        page(&e, "/admin/categories", &admin)
            .await
            .contains(r#"data-open="false""#),
        "and starts closed"
    );

    let res = send(
        &e.router,
        post_form(
            "/admin/categories",
            &[
                ("category_type", "service"),
                ("name", "Bơm nước"),
                ("slug", "bom-nuoc"),
                ("display_order", "mười"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Nhập một số nguyên hợp lệ."));

    // A slug already in use is refused before the insert, on its field.
    let res = send(
        &e.router,
        post_form(
            "/admin/categories",
            &[
                ("category_type", "service"),
                ("name", "Múc bùn 2"),
                ("slug", "muc-bun"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Đường dẫn này đã được dùng cho loại khác."));
    assert!(
        html.contains(r#"id="new-slug-error""#),
        "shown under the slug"
    );
    e.cleanup().await;
}

async fn slug_of(e: &Env, detail: &str) -> String {
    let id = detail.rsplit('/').next().unwrap();
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            format!(
                "SELECT slug FROM service_types WHERE id = {id}
                 UNION ALL SELECT slug FROM supply_types WHERE id = {id}"
            ),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "slug")
        .unwrap()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_icon_box_finds_font_awesome_names_for_the_catalogue_keepers_only() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let search = |query: &str, who: Option<String>| {
        send(
            &e.router,
            get_page(&format!("/api/admin/icons?q={query}"), who.as_deref()),
        )
    };

    let res = search("sun", Some(admin.clone())).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    let icons = body["icons"].as_array().unwrap();
    assert_eq!(icons.first().unwrap(), "fa-sun");
    assert!(icons.iter().all(|i| i.as_str().unwrap().contains("sun")));
    let body = json_body(search("cloud%20rain", Some(admin.clone())).await).await;
    assert_eq!(body["icons"][0], "fa-cloud-rain");

    // Saved typed without its fa-, an icon is stored as Font Awesome names it.
    let res = send(
        &e.router,
        post_form(
            "/admin/categories",
            &[
                ("category_type", "service"),
                ("name", "Bơm nước"),
                ("icon", "faucet"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let id = location_of(&res).rsplit('/').next().unwrap().to_owned();
    let icon: Option<String> =
        e.t.db
            .query_one(Statement::from_string(
                e.t.db.get_database_backend(),
                format!("SELECT icon FROM service_types WHERE id = {id}"),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "icon")
            .unwrap();
    assert_eq!(icon.as_deref(), Some("fa-faucet"));

    // A provider keeps no list with icons; nobody signed in, nobody at all.
    register_provider(&e.state, "nongdan", "0901234567").await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;
    assert_eq!(
        search("sun", Some(provider)).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(search("sun", None).await.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_blank_slug_is_made_from_the_name_and_never_repeats_one() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let suggest = |query: &str| {
        send(
            &e.router,
            get_page(&format!("/api/admin/categories/slug?{query}"), Some(&admin)),
        )
    };

    // The catalogue has "muc-bun": the name's slug is numbered.
    let res = suggest("text=M%C3%BAc%20b%C3%B9n").await;
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;
    assert_eq!(body["slug"], "muc-bun-2");
    assert_eq!(body["taken"], true);
    let body = json_body(suggest("text=bom-nuoc").await).await;
    assert_eq!(body["slug"], "bom-nuoc");
    assert_eq!(body["taken"], false);

    // Saved blank, the slug is made the same way, one number up each time.
    let blank = [
        ("category_type", "service"),
        ("name", "Múc bùn"),
        ("slug", ""),
    ];
    let res = send(
        &e.router,
        post_form("/admin/categories", &blank, Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let second = location_of(&res);
    assert_eq!(slug_of(&e, &second).await, "muc-bun-2");
    let res = send(
        &e.router,
        post_form("/admin/categories", &blank, Some(&admin)),
    )
    .await;
    let third = location_of(&res);
    assert_eq!(slug_of(&e, &third).await, "muc-bun-3");

    // A category's own slug is free to it while it is edited.
    let id = second.rsplit('/').next().unwrap();
    let body = json_body(suggest(&format!("text=muc-bun-2&id={id}")).await).await;
    assert_eq!(body["slug"], "muc-bun-2");
    assert_eq!(body["taken"], false);

    // Typed onto another category's slug, an edit is refused on the field.
    let res = send(
        &e.router,
        post_form(
            &third,
            &[("name", "Múc bùn"), ("slug", "muc-bun-2")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Đường dẫn này đã được dùng cho loại khác."));
    assert!(html.contains(r#"id="edit-slug-error""#));
    assert_eq!(slug_of(&e, &third).await, "muc-bun-3", "nothing saved");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_category_is_created_edited_and_hidden_from_the_public_pages() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    let res = send(
        &e.router,
        post_form("/admin/categories", NEW_CATEGORY, Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let detail = location_of(&res);
    assert!(detail.starts_with("/admin/categories/"));
    let mut cookie = admin.clone();
    for c in res.headers().get_all(header::SET_COOKIE) {
        if let Some(pair) = c.to_str().unwrap().split(';').next() {
            cookie.push_str("; ");
            cookie.push_str(pair);
        }
    }
    let html = page(&e, &detail, &cookie).await;
    assert!(html.contains("Đã thêm loại."), "flash shown once");
    assert!(html.contains(r#"value="bom-nuoc""#));
    assert!(page(&e, "/admin/categories", &admin)
        .await
        .contains(&format!(r#"href="{detail}""#)));

    let mut edited = NEW_CATEGORY.to_vec();
    edited[1] = ("name", "Bơm nước tưới");
    let res = send(&e.router, post_form(&detail, &edited, Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);

    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/status"),
            &[("is_active", "false"), ("reason", "trùng danh mục")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        last_audit(&e, "category").await,
        "hide_category: trùng danh mục"
    );
    let public = body_text(
        send(
            &e.router,
            Request::get("/api/explore/categories?type=service")
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    )
    .await;
    assert!(!public.contains("bom-nuoc"), "hidden from the public");
    let html = page(&e, &detail, &admin).await;
    assert!(html.contains("Bơm nước tưới"));
    assert!(html.contains("Hiển thị lại"));

    // The same switch from the list's row lands back on the list.
    let id = detail.rsplit('/').next().unwrap();
    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/status"),
            &[("is_active", "true"), ("back", "list")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(
        location_of(&res),
        "/admin/categories?type=service",
        "on the category's own tab"
    );
    let html = page(&e, &format!("/admin/categories?hide={id}"), &admin).await;
    assert!(html.contains(r#"data-open="true""#));
    assert!(html.contains("Ẩn Bơm nước tưới?"));
    assert!(
        html.contains(r#"name="back" value="list""#),
        "the confirmation lands back on the list"
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_catalogue_pages_need_their_permissions() {
    let e = env_with_origin().await;
    install_admin(&e).await;
    register_provider(&e.state, "nongdan", "0901234567").await;
    let provider = sign_in(&e, "nongdan", "matkhau123").await;

    for path in ["/admin/categories", "/admin/locations"] {
        let res = send(&e.router, get_page(path, Some(&provider))).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "{path}");
    }
    let res = send(
        &e.router,
        post_form("/admin/categories", NEW_CATEGORY, Some(&provider)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = send(&e.router, get_page("/admin/locations", None)).await;
    assert_eq!(res.headers()[header::LOCATION], "/login");
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_location_is_validated_created_hidden_and_deleted() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    let res = send(
        &e.router,
        post_form(
            "/admin/locations",
            &[("label", ""), ("lat", "mười"), ("lng", "106,1")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Vui lòng nhập tên khu vực."));
    assert!(html.contains("Toạ độ không hợp lệ."));
    assert!(html.contains(r#"value="mười""#), "typed value kept");
    assert!(
        html.contains(r#"data-open="true""#),
        "the create sheet comes back open"
    );

    let fields = [
        ("label", "Mỹ Phong"),
        ("lat", "10,4048"),
        ("lng", "106.118"),
        ("display_order", "3"),
    ];
    let res = send(
        &e.router,
        post_form("/admin/locations", &fields, Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let detail = location_of(&res);
    assert!(page(&e, &detail, &admin)
        .await
        .contains(r#"value="10.4048""#));

    let res = send(
        &e.router,
        post_form("/admin/locations", &fields, Some(&admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res).await.contains("Đã có khu vực với tên này."));

    // Hiding asks first, on the page and from the list's row, like every
    // other hide; the reason is audited.
    assert!(page(&e, &detail, &admin)
        .await
        .contains(r##"href="#an-khu-vuc""##));
    let id = detail.rsplit('/').next().unwrap();
    let html = page(&e, &format!("/admin/locations?hide={id}"), &admin).await;
    assert!(html.contains(r#"data-open="true""#));
    assert!(html.contains("Ẩn khu vực Mỹ Phong?"));
    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/status"),
            &[
                ("is_active", "false"),
                ("reason", "trùng"),
                ("back", "list"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(location_of(&res), "/admin/locations");
    assert_eq!(last_audit(&e, "location").await, "hide_location: trùng");
    let public = body_text(
        send(
            &e.router,
            Request::get("/api/explore/locations")
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    )
    .await;
    assert!(!public.contains("Mỹ Phong"), "hidden from the explore page");

    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/delete"),
            &[("reason", "nhập nhầm")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&res), "/admin/locations");
    assert_eq!(
        last_audit(&e, "location").await,
        "delete_location: nhập nhầm"
    );
    let res = send(&e.router, get_page(&detail, Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_location_is_filled_from_the_ward_chosen() {
    let e = env_with_origin().await;
    integration_tests::load_admin_units(&e).await;
    let admin = install_admin(&e).await;

    // The create sheet offers the wards, each with its preset name and point.
    let list = page(&e, "/admin/locations", &admin).await;
    assert!(list.contains(r#"<option value="w28471" data-preset="Bình Phú, Đồng Tháp""#));
    assert!(list.contains(r#"x-data="wardFill""#));

    // Without JS: the blank boxes take the ward's name and point.
    let res = send(
        &e.router,
        post_form(
            "/admin/locations",
            &[("ward", "w28471"), ("label", ""), ("lat", ""), ("lng", "")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let preset = location_of(&res);
    let detail = page(&e, &preset, &admin).await;
    assert!(detail.contains("Bình Phú, Đồng Tháp"));

    // The edit form has the same picker, and the same rule.
    assert!(detail.contains(r#"<option value="w28378" data-preset="Mỹ Thiện, Đồng Tháp""#));
    assert!(detail.contains(r#"x-data="wardFill""#));
    let res = send(
        &e.router,
        post_form(
            &preset,
            &[("ward", "w28378"), ("label", ""), ("lat", ""), ("lng", "")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let detail = page(&e, &preset, &admin).await;
    assert!(
        detail.contains("<title>Mỹ Thiện, Đồng Tháp"),
        "renamed and moved to Mỹ Thiện"
    );

    // What was typed stays.
    let res = send(
        &e.router,
        post_form(
            "/admin/locations",
            &[
                ("ward", "w28471"),
                ("label", "Cai Lậy cũ"),
                ("lat", ""),
                ("lng", ""),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(page(&e, &location_of(&res), &admin)
        .await
        .contains("Cai Lậy cũ"));

    // An unknown ward is reported on its box, with what was typed kept.
    let res = send(
        &e.router,
        post_form(
            "/admin/locations",
            &[("ward", "w99999"), ("label", "Thử")],
            Some(&admin),
        ),
    )
    .await;
    assert!(res.status().is_client_error());
    let html = body_text(res).await;
    assert!(html.contains("Không tìm thấy khu vực."));
    assert!(html.contains(r#"value="Thử""#));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_location_page_acts_from_its_header_and_says_who_hid_it() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let fields = [("label", "Tân Phú"), ("lat", "10.40"), ("lng", "106.10")];
    let res = send(
        &e.router,
        post_form("/admin/locations", &fields, Some(&admin)),
    )
    .await;
    let detail = location_of(&res);

    // One header: hide as its button, the map and the deletion in its menu,
    // each confirmation opened by a link to its sheet.
    let html = page(&e, &detail, &admin).await;
    assert!(html.contains(r##"href="#an-khu-vuc""##));
    assert!(html.contains(r##"href="#xoa-khu-vuc""##));
    assert!(html.contains("https://www.google.com/maps?q=10.4,106.1"));
    assert!(html.contains("Thêm khu vực"), "its creation in its history");

    let res = send(
        &e.router,
        post_form(
            &format!("{detail}/status"),
            &[("is_active", "false"), ("reason", "Trùng với Tân Phú Đông")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let html = page(&e, &detail, &admin).await;
    assert!(html.contains("Ẩn khu vực"), "the strip says who hid it");
    assert!(html.contains("Trùng với Tân Phú Đông"));
    assert!(
        !html.contains(r##"href="#an-khu-vuc""##),
        "shown again with one button"
    );
    e.cleanup().await;
}

/// A whole number the query answers as `n`.
async fn count_of(e: &Env, sql: &str) -> i64 {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap()
}

/// One admin catalogue list grown past a page: every row on exactly one of
/// its pages, the first page linking the second, past the end the last page.
struct PagedList {
    /// The list's address, `?` and its other parameters included when any.
    list: &'static str,
    table: &'static str,
    /// Adds `ADMIN_PAGE_SIZE + 2` rows named `Zz {n}`.
    insert: String,
}

async fn assert_paged(e: &Env, admin: &str, l: &PagedList) {
    use application::constants::ADMIN_PAGE_SIZE;

    e.t.db.execute_unprepared(&l.insert).await.unwrap();
    let total = count_of(e, &format!("SELECT count(*) AS n FROM {}", l.table)).await;
    let last = (total + ADMIN_PAGE_SIZE - 1) / ADMIN_PAGE_SIZE;
    assert!(last >= 2, "{}: more than one page", l.list);
    let join = if l.list.contains('?') { "&" } else { "?" };

    let first = page(e, l.list, admin).await;
    let next = format!(r#"href="{}{}page=2""#, l.list, join).replace('&', "&amp;");
    assert!(first.contains(&next), "{}: no link to page 2", l.list);

    let mut seen = vec![0; (ADMIN_PAGE_SIZE + 2) as usize];
    for p in 1..=last {
        let body = page(e, &format!("{}{}page={p}", l.list, join), admin).await;
        for (n, hits) in seen.iter_mut().enumerate() {
            if body.contains(&format!(">Zz {}<", n + 1)) {
                *hits += 1;
            }
        }
    }
    assert!(seen.iter().all(|h| *h == 1), "{}: {seen:?}", l.list);

    let res = send(
        &e.router,
        get_page(&format!("{}{}page=99", l.list, join), Some(admin)),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&res), format!("{}{}page={last}", l.list, join));
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_catalogue_lists_are_paged_and_a_row_lands_back_on_its_page() {
    use application::constants::ADMIN_PAGE_SIZE;

    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let rows = ADMIN_PAGE_SIZE + 2;
    let types = |table: &str, slug: &str| {
        format!(
            "INSERT INTO {table} (name, slug, display_order)
             SELECT 'Zz ' || n, '{slug}-' || n, 30000 + n FROM generate_series(1, {rows}) AS n"
        )
    };
    let lists = [
        PagedList {
            list: "/admin/commodities",
            table: "commodities",
            insert: format!(
                "INSERT INTO commodities (name, sector, display_order)
                 SELECT 'Zz ' || n, (SELECT code FROM sectors ORDER BY id LIMIT 1), 30000 + n
                 FROM generate_series(1, {rows}) AS n"
            ),
        },
        PagedList {
            list: "/admin/categories?type=service",
            table: "service_types",
            insert: types("service_types", "zz-dich-vu"),
        },
        PagedList {
            list: "/admin/categories?type=product",
            table: "supply_types",
            insert: types("supply_types", "zz-vat-tu"),
        },
        PagedList {
            list: "/admin/locations",
            table: "locations",
            insert: format!(
                "INSERT INTO locations (label, lat, lng, display_order)
                 SELECT 'Zz ' || n, 10.3, 106.1, 30000 + n FROM generate_series(1, {rows}) AS n"
            ),
        },
    ];
    for list in &lists {
        assert_paged(&e, &admin, list).await;
    }

    // A row's "show" sent from page 2 lands back on page 2, on its tab.
    for (route, table, label, back) in [
        (
            "commodities",
            "commodities",
            "name",
            "/admin/commodities?page=2",
        ),
        (
            "categories",
            "supply_types",
            "name",
            "/admin/categories?type=product&page=2",
        ),
        ("locations", "locations", "label", "/admin/locations?page=2"),
    ] {
        let id = count_of(
            &e,
            &format!("SELECT id::bigint AS n FROM {table} WHERE {label} = 'Zz 1'"),
        )
        .await;
        let res = send(
            &e.router,
            post_form(
                &format!("/admin/{route}/{id}/status"),
                &[("is_active", "true"), ("back", "list"), ("page", "2")],
                Some(&admin),
            ),
        )
        .await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER, "{route}");
        assert_eq!(location_of(&res), back);
    }

    // A preset deleted from page 2 lands back there.
    let id = count_of(
        &e,
        "SELECT id::bigint AS n FROM locations WHERE label = 'Zz 2'",
    )
    .await;
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/locations/{id}/delete"),
            &[("page", "2")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(location_of(&res), "/admin/locations?page=2");
    e.cleanup().await;
}
