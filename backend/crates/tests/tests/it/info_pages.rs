//! Information pages: made off and without text by an admin, shown once on
//! and written, in preview only to who may preview (never indexed), each
//! version in force from its day, the typed HTML of a text never reaching a
//! page, and the admin's pages closed to everyone else.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use axum::http::{header, StatusCode};
use chrono::{Duration, Utc};
use integration_tests::{
    body_text, env_with_origin, get_page, install_admin, post_form, register_provider, send,
    sign_in, Env,
};

const PAGE: &str = "/info/dieu-khoan-su-dung";

async fn status_of(e: &Env, path: &str, cookie: Option<&str>) -> StatusCode {
    send(&e.router, get_page(path, cookie)).await.status()
}

async fn html_of(e: &Env, path: &str, cookie: Option<&str>) -> String {
    let res = send(&e.router, get_page(path, cookie)).await;
    let status = res.status();
    let html = body_text(res).await;
    assert_eq!(status, StatusCode::OK, "{path}: {html}");
    html
}

/// A day counted from today in Vietnam, as the date box sends it.
fn day(offset: i64) -> String {
    ((Utc::now() + Duration::hours(7)).date_naive() + Duration::days(offset))
        .format("%Y-%m-%d")
        .to_string()
}

/// Creates the terms page; its admin page's address.
async fn create_terms(e: &Env, admin: &str) -> String {
    let res = send(
        &e.router,
        post_form(
            "/admin/info-pages",
            &[
                ("title", "Điều khoản sử dụng"),
                ("footer_group", "policy"),
                ("in_footer", "on"),
            ],
            Some(admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "create");
    res.headers()[header::LOCATION]
        .to_str()
        .unwrap_or_default()
        .to_owned()
}

async fn write(e: &Env, admin: &str, record: &str, pairs: &[(&str, &str)]) -> (StatusCode, String) {
    let res = send(
        &e.router,
        post_form(&format!("{record}/text"), pairs, Some(admin)),
    )
    .await;
    let status = res.status();
    (status, body_text(res).await)
}

async fn set_stage(e: &Env, admin: &str, record: &str, stage: &str) {
    let res = send(
        &e.router,
        post_form(
            &format!("{record}/stage"),
            &[("stage", stage), ("reason", "kiểm tra")],
            Some(admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "stage {stage}");
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_page_shows_once_it_is_on_and_written() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let record = create_terms(&e, &admin).await;
    assert!(record.starts_with("/admin/info-pages/"), "{record}");

    // Made off: nobody sees it; on but without text: still nowhere.
    assert_eq!(status_of(&e, PAGE, None).await, StatusCode::NOT_FOUND);
    set_stage(&e, &admin, &record, "on").await;
    assert_eq!(status_of(&e, PAGE, None).await, StatusCode::NOT_FOUND);
    assert!(!html_of(&e, "/info", None).await.contains(PAGE));

    // A preview shows the text as it would be and saves nothing.
    let text = "# Phạm vi\n\nNội dung **quan trọng**.\n\n## Khiếu nại\n\n<script>alert(1)</script>\n\n[xấu](javascript:alert(1))";
    let (status, html) = write(
        &e,
        &admin,
        &record,
        &[
            ("body", text),
            ("effective_on", &day(0)),
            ("intent", "preview"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains(r#"<h2 id="pham-vi">"#), "{html}");
    assert!(html.contains("<strong>quan trọng</strong>"));
    assert_eq!(status_of(&e, PAGE, None).await, StatusCode::NOT_FOUND);

    // A day before today is refused, the text kept as typed.
    let (status, html) = write(
        &e,
        &admin,
        &record,
        &[("body", text), ("effective_on", &day(-1))],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(html.contains("Ngày hiệu lực không được trước hôm nay."));
    assert!(html.contains("Nội dung **quan trọng**."));

    let (status, _) = write(
        &e,
        &admin,
        &record,
        &[
            ("body", text),
            ("effective_on", &day(0)),
            ("change_note", "Bản đầu"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    // Written and on: everyone reads it, its outline above, and no typed
    // HTML reaches the page.
    let public = html_of(&e, PAGE, None).await;
    assert!(
        public.contains(r##"<a href="#khieu-nai">Khiếu nại</a>"##),
        "{public}"
    );
    assert!(public.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!public.contains("<script>alert(1)"));
    assert!(!public.contains("javascript:"));
    assert!(public.contains("xấu"));
    assert!(html_of(&e, "/info", None).await.contains(PAGE));

    // A version written ahead waits for its day.
    let (status, _) = write(
        &e,
        &admin,
        &record,
        &[
            ("body", "## Bản mới\n\nSắp áp dụng."),
            ("effective_on", &day(3)),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let public = html_of(&e, PAGE, None).await;
    assert!(public.contains("Khiếu nại") && !public.contains("Sắp áp dụng."));
    let admin_page = html_of(&e, &record, Some(&admin)).await;
    assert!(admin_page.contains("Sắp áp dụng") && admin_page.contains("Đang áp dụng"));

    // In preview: only who may preview sees it, with the strip, never indexed.
    set_stage(&e, &admin, &record, "preview").await;
    assert_eq!(status_of(&e, PAGE, None).await, StatusCode::NOT_FOUND);
    assert!(!html_of(&e, "/info", None).await.contains(PAGE));
    let res = send(&e.router, get_page(PAGE, Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()
            .get("x-robots-tag")
            .and_then(|v| v.to_str().ok()),
        Some("noindex")
    );
    let html = body_text(res).await;
    assert!(html.contains("preview-strip"), "{html}");
    assert!(
        html.contains(&record),
        "the strip leads to the page's admin page"
    );

    // Off: nobody, admins neither.
    set_stage(&e, &admin, &record, "off").await;
    assert_eq!(
        status_of(&e, PAGE, Some(&admin)).await,
        StatusCode::NOT_FOUND
    );

    // Its history says what was done.
    let admin_page = html_of(&e, &record, Some(&admin)).await;
    assert!(admin_page.contains("Lưu phiên bản nội dung mới"));
    assert!(admin_page.contains("Đổi người xem trang"));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_page_is_checked_and_its_address_is_its_own() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    create_terms(&e, &admin).await;

    // Every rule at once, the sheet back open with its messages.
    let res = send(
        &e.router,
        post_form(
            "/admin/info-pages",
            &[
                ("title", " "),
                ("footer_group", "side"),
                ("icon", "no-such-icon-x"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Vui lòng nhập tiêu đề trang."));
    assert!(html.contains("Vui lòng chọn nhóm ở chân trang."));

    // An address another page has is refused.
    let res = send(
        &e.router,
        post_form(
            "/admin/info-pages",
            &[
                ("title", "Điều khoản sử dụng"),
                ("code", "dieu-khoan-su-dung"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Đường dẫn này đã có trang khác dùng."));

    // An unknown page or version is a missing page.
    assert_eq!(
        status_of(&e, "/info/khong-co", None).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status_of(&e, "/info/dieu-khoan-su-dung?version=abc", None).await,
        StatusCode::NOT_FOUND
    );
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn only_an_admin_keeps_the_pages() {
    let e = env_with_origin().await;
    install_admin(&e).await;
    register_provider(&e.state, "nam", "0901234567").await;
    let provider = sign_in(&e, "nam", "matkhau123").await;

    let res = send(&e.router, get_page("/admin/info-pages", None)).await;
    assert_eq!(
        res.status(),
        StatusCode::SEE_OTHER,
        "anonymous: to the login"
    );
    let res = send(&e.router, get_page("/admin/info-pages", Some(&provider))).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = send(
        &e.router,
        post_form(
            "/admin/info-pages",
            &[("title", "Giả mạo")],
            Some(&provider),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_footer_lists_the_pages_put_there_and_who_runs_the_platform() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let record = create_terms(&e, &admin).await;
    let (status, _) = write(
        &e,
        &admin,
        &record,
        &[
            ("body", "## Phạm vi\n\nNội dung."),
            ("effective_on", &day(0)),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    // Off: not in the footer; on: in its group, with the way to all of them.
    assert!(!html_of(&e, "/", None).await.contains(PAGE));
    set_stage(&e, &admin, &record, "on").await;
    let home = html_of(&e, "/", None).await;
    assert!(home.contains(&format!(r#"href="{PAGE}""#)), "in the footer");
    assert!(home.contains("Chính sách &amp; quy định"));
    assert!(home.contains(r#"href="/info""#));

    // Taken out of the footer, it is still on the hub.
    let res = send(
        &e.router,
        post_form(
            &record,
            &[("title", "Điều khoản sử dụng"), ("footer_group", "policy")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(!html_of(&e, "/", None)
        .await
        .contains(&format!(r#"href="{PAGE}""#)));
    assert!(html_of(&e, "/info", None).await.contains(PAGE));

    // The legal entity's lines, each only once set.
    assert!(!html_of(&e, "/", None).await.contains("app-footer__entity"));
    let res = send(
        &e.router,
        post_form(
            "/admin/settings",
            &[
                ("group", "legal"),
                ("legal.company_name", "Công ty TNHH Qhortus"),
                ("legal.tax_code", "0312345678"),
                ("legal.address", "Phường 1, Đồng Tháp"),
                ("legal.representative", ""),
                ("legal.license", ""),
                ("legal.moit_url", "https://online.gov.vn/Home/WebDetails/1"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let home = html_of(&e, "/", None).await;
    assert!(home.contains("Công ty TNHH Qhortus"));
    assert!(home.contains("Mã số thuế: 0312345678"));
    assert!(home.contains(r#"href="https://online.gov.vn/Home/WebDetails/1""#));
    assert!(
        !home.contains("Người đại diện:"),
        "an empty line is not shown"
    );

    // A link that is not https is refused.
    let res = send(
        &e.router,
        post_form(
            "/admin/settings",
            &[
                ("group", "legal"),
                ("legal.moit_url", "javascript:alert(1)"),
            ],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    e.cleanup().await;
}

/// A page at `code`, on and written today.
async fn publish(e: &Env, admin: &str, title: &str, code: &str) {
    let res = send(
        &e.router,
        post_form(
            "/admin/info-pages",
            &[("title", title), ("code", code), ("footer_group", "policy")],
            Some(admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "create {code}");
    let record = res.headers()[header::LOCATION]
        .to_str()
        .unwrap_or_default()
        .to_owned();
    let (status, _) = write(
        e,
        admin,
        &record,
        &[
            ("body", "## Nội dung\n\nĐiều khoản."),
            ("effective_on", &day(0)),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    set_stage(e, admin, &record, "on").await;
}

fn sign_up(email: &str, ticked: bool) -> Vec<(&str, &str)> {
    let mut form = vec![
        ("business_name", "Vườn Sáu"),
        ("business_phone", "0907777777"),
        ("email", email),
        ("password", "matkhau123"),
        ("password2", "matkhau123"),
    ];
    if ticked {
        form.push(("accepted_terms", "on"));
    }
    form
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_sign_up_accepts_the_terms_once_they_are_on_and_it_is_kept() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    // Without the pages there is nothing to accept.
    let register = html_of(&e, "/register", None).await;
    assert!(!register.contains(r#"name="accepted_terms""#));
    assert!(!html_of(&e, "/jobs/new", None)
        .await
        .contains("consent-note"));

    publish(&e, &admin, "Điều khoản sử dụng", "dieu-khoan-su-dung").await;
    publish(
        &e,
        &admin,
        "Chính sách dữ liệu cá nhân",
        "chinh-sach-du-lieu-ca-nhan",
    )
    .await;
    let register = html_of(&e, "/register", None).await;
    assert!(register.contains(r#"name="accepted_terms""#));
    assert!(register.contains(r#"href="/info/dieu-khoan-su-dung""#));
    assert!(register.contains(r#"href="/info/chinh-sach-du-lieu-ca-nhan""#));
    // The farmers' forms name the policy without a box to tick.
    let job = html_of(&e, "/jobs/new", None).await;
    assert!(job.contains("consent-note"));
    assert!(job.contains(r#"href="/info/chinh-sach-du-lieu-ca-nhan""#));

    // Not ticked: refused, nothing made.
    let res = send(
        &e.router,
        post_form("/register", &sign_up("sau@example.vn", false), None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("Vui lòng đồng ý với điều khoản và chính sách để tạo tài khoản."));
    assert_eq!(
        scalar(
            &e,
            "SELECT count(*)::text AS v FROM users WHERE email = 'sau@example.vn'"
        )
        .await,
        "0"
    );

    // Ticked: the account keeps which versions it accepted.
    let res = send(
        &e.router,
        post_form("/register", &sign_up("sau@example.vn", true), None),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        scalar(
            &e,
            "SELECT count(*)::text AS v FROM consents c JOIN users u ON u.id = c.user_id
              WHERE u.email = 'sau@example.vn'"
        )
        .await,
        "2"
    );
    e.cleanup().await;
}

async fn scalar(e: &Env, sql: &str) -> String {
    use sea_orm::{ConnectionTrait, Statement};
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .ok()
        .flatten()
        .and_then(|row| row.try_get::<String>("", "v").ok())
        .unwrap_or_default()
}
