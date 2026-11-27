//! Articles: made drafts by an admin, read by farmers once published and the
//! articles feature is open, each kind a list of its own address, one
//! article at its own slug, its text never carrying typed HTML, and the
//! admin's pages closed to everyone else.
//! Database tests need TEST_DATABASE_URL and `--include-ignored`.

use application::features::Feature;
use axum::http::{header, StatusCode};
use integration_tests::{
    body_text, env_with_origin, features_form, get_page, install_admin, post_form,
    post_multipart_form, register_provider, send, sign_in, Env,
};

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

async fn open_articles(e: &Env, admin: &str, stage: &str) {
    let res = send(
        &e.router,
        post_form(
            "/admin/settings",
            &features_form(&[
                ("access.registration_open", "true"),
                (Feature::Articles.setting_key(), stage),
            ]),
            Some(admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "articles {stage}");
}

/// A draft; its admin page's address.
async fn draft(e: &Env, admin: &str, kind: &str, title: &str, body: &str) -> String {
    let res = send(
        &e.router,
        post_multipart_form(
            "/admin/articles",
            &[("kind", kind), ("title", title), ("body", body)],
            None,
            admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "create {title}");
    res.headers()[header::LOCATION]
        .to_str()
        .unwrap_or_default()
        .to_owned()
}

async fn set_status(e: &Env, admin: &str, record: &str, status: &str) {
    let res = send(
        &e.router,
        post_form(
            &format!("{record}/status"),
            &[("status", status), ("reason", "kiểm tra")],
            Some(admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER, "status {status}");
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn an_article_reaches_farmers_once_published_and_the_part_is_open() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;

    // Every rule at once, the sheet back open.
    let res = send(
        &e.router,
        post_multipart_form(
            "/admin/articles",
            &[("kind", "story"), ("title", " "), ("body", "")],
            None,
            &admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("Vui lòng chọn loại bài: tin tức hoặc kiến thức."));
    assert!(html.contains("Vui lòng nhập tiêu đề bài viết."));
    assert!(html.contains("Vui lòng nhập nội dung bài viết."));

    let record = draft(
        &e,
        &admin,
        "guide",
        "Phòng rầy nâu cho lúa",
        "## Dấu hiệu\n\nLá vàng.\n\n<script>alert(1)</script>",
    )
    .await;
    let id = record.rsplit('/').next().unwrap_or_default().to_owned();

    // The part is off until the admin opens it.
    assert_eq!(
        status_of(&e, "/articles", None).await,
        StatusCode::NOT_FOUND
    );
    open_articles(&e, &admin, "on").await;
    let hub = html_of(&e, "/articles", None).await;
    assert!(
        hub.contains("Chưa có bài viết nào."),
        "a draft is not public"
    );
    assert_eq!(
        status_of(&e, &format!("/articles/{id}"), None).await,
        StatusCode::NOT_FOUND
    );

    // Published: on the hub, its kind's list, its own slug; never the other
    // kind's list.
    set_status(&e, &admin, &record, "published").await;
    let res = send(&e.router, get_page(&format!("/articles/{id}"), None)).await;
    assert_eq!(res.status(), StatusCode::MOVED_PERMANENTLY);
    let canonical = res.headers()[header::LOCATION]
        .to_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(canonical, format!("/articles/{id}/phong-ray-nau-cho-lua"));
    let page = html_of(&e, &canonical, None).await;
    assert!(page.contains(r#"<h2 id="dau-hieu">"#));
    assert!(page.contains("&lt;script&gt;") && !page.contains("<script>alert(1)"));
    assert!(html_of(&e, "/articles", None).await.contains(&canonical));
    assert!(html_of(&e, "/articles/guides", None)
        .await
        .contains(&canonical));
    assert!(!html_of(&e, "/articles/news", None)
        .await
        .contains(&canonical));
    assert_eq!(
        status_of(&e, &format!("/articles/{id}/sai-duong-dan"), None).await,
        StatusCode::MOVED_PERMANENTLY
    );
    // The JSON API reads the same.
    let json = body_text(
        send(
            &e.router,
            get_page("/api/explore/articles?kind=guide", None),
        )
        .await,
    )
    .await;
    assert!(json.contains("Phòng rầy nâu cho lúa"), "{json}");

    // The home page's strip while the part is shown.
    assert!(html_of(&e, "/", None).await.contains(&canonical));

    // An edit (multipart, no photo) is saved; its preview saves nothing.
    let res = send(
        &e.router,
        post_multipart_form(
            &record,
            &[
                ("kind", "guide"),
                ("title", "Phòng rầy nâu cho lúa"),
                ("summary", "Ba bước giữ ruộng."),
                ("body", "## Bản xem trước"),
                ("intent", "preview"),
            ],
            None,
            &admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_text(res).await.contains(r#"<h2 id="ban-xem-truoc">"#));
    assert!(!html_of(&e, &canonical, None)
        .await
        .contains("Bản xem trước"));
    let res = send(
        &e.router,
        post_multipart_form(
            &record,
            &[
                ("kind", "guide"),
                ("title", "Phòng rầy nâu cho lúa"),
                ("summary", "Ba bước giữ ruộng."),
                ("body", "## Dấu hiệu\n\nLá vàng úa."),
            ],
            None,
            &admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(html_of(&e, &canonical, None)
        .await
        .contains("Ba bước giữ ruộng."));

    // Taken down: gone from farmers; its history says why.
    set_status(&e, &admin, &record, "hidden").await;
    assert_eq!(status_of(&e, &canonical, None).await, StatusCode::NOT_FOUND);
    let admin_page = html_of(&e, &record, Some(&admin)).await;
    assert!(admin_page.contains("Đổi trạng thái bài viết"));

    // The part off again: nothing of it shows, not even to the admin.
    set_status(&e, &admin, &record, "published").await;
    open_articles(&e, &admin, "off").await;
    assert_eq!(
        status_of(&e, &canonical, Some(&admin)).await,
        StatusCode::NOT_FOUND
    );
    assert!(!html_of(&e, "/", None).await.contains(&canonical));
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn only_an_admin_keeps_the_articles() {
    let e = env_with_origin().await;
    install_admin(&e).await;
    register_provider(&e.state, "nam", "0901234567").await;
    let provider = sign_in(&e, "nam", "matkhau123").await;
    assert_eq!(
        status_of(&e, "/admin/articles", None).await,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        status_of(&e, "/admin/articles", Some(&provider)).await,
        StatusCode::FORBIDDEN
    );
    let res = send(
        &e.router,
        post_multipart_form(
            "/admin/articles",
            &[("kind", "news"), ("title", "Giả"), ("body", "x")],
            None,
            &provider,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_demo_data_brings_articles_and_pages_to_start_from_and_takes_them_away() {
    let e = env_with_origin().await;
    let actor = integration_tests::first_admin(&e.state).await;
    let admin = sign_in(&e, "quantri", "matkhau123").await;
    e.state
        .usecases
        .setup
        .load_demo(&actor)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    open_articles(&e, &admin, "on").await;

    // The demo articles are published; the pages wait in preview, so a
    // sign-up is not yet asked to accept them.
    assert!(html_of(&e, "/articles/guides", None)
        .await
        .contains("Phòng rầy nâu cho lúa Đông Xuân"));
    assert_eq!(
        status_of(&e, "/info/lien-he", None).await,
        StatusCode::NOT_FOUND
    );
    assert!(html_of(&e, "/info/lien-he", Some(&admin))
        .await
        .contains("preview-strip"));
    assert!(!html_of(&e, "/register", None)
        .await
        .contains(r#"name="accepted_terms""#));

    e.state
        .usecases
        .setup
        .remove_demo(&actor)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert!(html_of(&e, "/articles", None)
        .await
        .contains("Chưa có bài viết nào."));
    assert_eq!(
        status_of(&e, "/info/lien-he", Some(&admin)).await,
        StatusCode::NOT_FOUND
    );
    e.cleanup().await;
}

/// A JSON request as the editors' scripts send it, from our own origin.
fn json_post(path: &str, body: &str, cookie: &str) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json")
        .header(header::ORIGIN, integration_tests::TEST_ORIGIN)
        .header(header::COOKIE, cookie)
        .body(axum::body::Body::from(body.to_owned()))
        .unwrap_or_default()
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn the_editor_previews_names_and_publishes_in_one_go() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    open_articles(&e, &admin, "on").await;

    // The new article's page: an editor of its own, not a sheet.
    let editor = html_of(&e, "/admin/articles/new", Some(&admin)).await;
    assert!(editor.contains(r#"x-data="markdownEditor""#));
    assert!(editor.contains(r#"x-data="addressPreview""#));

    // The preview the editor asks renders as the page will, typed HTML as text.
    let res = send(
        &e.router,
        json_post(
            "/api/admin/markdown/preview",
            r###"{"text":"## Mục một\n\n<script>x</script>"}"###,
            &admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_text(res).await;
    assert!(json.contains(r#"id=\"muc-mot\""#), "{json}");
    assert!(json.contains("&lt;script&gt;"), "{json}");

    // The address follows the title; an information page's is numbered
    // while another page has it.
    let slug = body_text(
        send(
            &e.router,
            get_page(
                "/api/admin/articles/slug?text=Gi%C3%A1%20l%C3%BAa",
                Some(&admin),
            ),
        )
        .await,
    )
    .await;
    assert!(slug.contains(r#""slug":"gia-lua""#), "{slug}");
    let res = send(
        &e.router,
        post_form(
            "/admin/info-pages",
            &[("title", "Liên hệ"), ("footer_group", "about")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let code = body_text(
        send(
            &e.router,
            get_page(
                "/api/admin/info-pages/slug?text=Li%C3%AAn%20h%E1%BB%87",
                Some(&admin),
            ),
        )
        .await,
    )
    .await;
    assert!(
        code.contains(r#""slug":"lien-he-2""#) && code.contains(r#""taken":true"#),
        "{code}"
    );
    // A second page of the same title takes the next free address itself.
    let res = send(
        &e.router,
        post_form(
            "/admin/info-pages",
            &[("title", "Liên hệ"), ("footer_group", "about")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(html_of(&e, "/admin/info-pages", Some(&admin))
        .await
        .contains("/info/lien-he-2"));

    // "Save and publish": made and published at once.
    let res = send(
        &e.router,
        post_multipart_form(
            "/admin/articles",
            &[
                ("kind", "news"),
                ("title", "Giá lúa tăng"),
                ("body", "## Tin\n\nGiá lúa tăng."),
                ("intent", "publish"),
            ],
            None,
            &admin,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let record = res.headers()[header::LOCATION]
        .to_str()
        .unwrap_or_default()
        .to_owned();
    let id = record.rsplit('/').next().unwrap_or_default().to_owned();
    let page = html_of(&e, &format!("/articles/{id}/gia-lua-tang"), None).await;
    assert!(page.contains("Giá lúa tăng."));
    // The home page leads with it.
    assert!(html_of(&e, "/", None).await.contains("article-feature"));

    // Neither helper is open to a provider.
    register_provider(&e.state, "nam", "0901234567").await;
    let provider = sign_in(&e, "nam", "matkhau123").await;
    let res = send(
        &e.router,
        json_post("/api/admin/markdown/preview", r#"{"text":"x"}"#, &provider),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    e.cleanup().await;
}
