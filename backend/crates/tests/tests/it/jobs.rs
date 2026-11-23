//! Job posts: a farmer posts a job, the providers nearby that offer its
//! category are emailed and see it (with the phone) on their jobs page, one
//! far away does not; a phone has only so many open posts; the admin marks
//! one spam and it leaves the providers' pages. Database tests need
//! TEST_DATABASE_URL and `--include-ignored`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use integration_tests::{
    body_text, categories_of, env_with_origin, get_page, install_admin, json_body, post_form,
    register_provider, send, Env, TEST_ORIGIN,
};
use sea_orm::{ConnectionTrait, Statement};

async fn scalar(e: &Env, sql: &str) -> String {
    e.t.db
        .query_one(Statement::from_string(
            e.t.db.get_database_backend(),
            sql.to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "v")
        .unwrap()
}

/// A provider with one service in `category`, placed at (lng, lat); its
/// session token.
async fn provider_at(
    e: &Env,
    username: &str,
    phone: &str,
    category: i32,
    lng: f64,
    lat: f64,
) -> String {
    let token = register_provider(&e.state, username, phone).await;
    let req = Request::post("/api/provider/services")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, TEST_ORIGIN)
        .body(Body::from(format!(r#"{{"category_id":{category}}}"#)))
        .unwrap();
    assert_eq!(send(&e.router, req).await.status(), StatusCode::CREATED);
    e.t.db
        .execute_unprepared(&format!(
            "UPDATE businesses SET location = \
             ST_GeographyFromText('SRID=4326;POINT({lng} {lat})') WHERE name = 'Vườn {username}'"
        ))
        .await
        .unwrap();
    token
}

async fn own_jobs(e: &Env, token: &str) -> serde_json::Value {
    json_body(
        send(
            &e.router,
            Request::get("/api/provider/jobs")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    )
    .await
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn a_job_reaches_the_providers_nearby_that_offer_it() {
    let e = env_with_origin().await;
    let admin = install_admin(&e).await;
    let category = categories_of(&e, "service").await[0].0.to_string();
    let near = provider_at(
        &e,
        "phundrone",
        "0901111111",
        category.parse().unwrap(),
        106.090,
        10.420,
    )
    .await;
    let far = provider_at(
        &e,
        "longxuyen",
        "0903333333",
        category.parse().unwrap(),
        105.446,
        10.374,
    )
    .await;
    let cai_lay = scalar(
        &e,
        "SELECT id::text AS v FROM locations WHERE label = 'Đồng Tháp — Cai Lậy'",
    )
    .await;

    // No place, no description: refused, both boxes at once.
    let res = send(
        &e.router,
        post_form(
            "/jobs",
            &[
                ("category_id", &category),
                ("requester_name", "Chu Ba"),
                ("requester_phone", "0912345678"),
            ],
            None,
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let html = body_text(res).await;
    assert!(html.contains("chọn khu vực") && html.contains("ghi việc cần mướn"));

    // A job by Cai Lậy: back to the form with the flash.
    let job = [
        ("category_id", category.as_str()),
        ("location_id", cai_lay.as_str()),
        ("description", "Phun thuoc 3 cong lua"),
        ("requester_name", "Chu Ba"),
        ("requester_phone", "0912345678"),
    ];
    // Every provider here confirmed its address: who is told is a matter of
    // distance and of what they offer.
    e.t.db
        .execute_unprepared("UPDATE users SET email_verified_at = now()")
        .await
        .unwrap();
    let res = send(&e.router, post_form("/jobs", &job, None)).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/jobs/new");

    // Only the provider nearby is emailed, with the farmer's phone.
    let sent = e.mail.jobs(1).await;
    assert_eq!(sent.len(), 1, "one provider within the radius");
    assert_eq!(sent[0].0, "phundrone@example.vn");
    assert_eq!(sent[0].1.requester_phone, "0912345678");
    assert_eq!(sent[0].1.jobs_path, "/provider/jobs");

    // The jobs pages: the near one has it, the far one does not.
    let mine = own_jobs(&e, &near).await;
    assert_eq!(mine["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(mine["total"], 1);
    assert_eq!(mine["items"][0]["requester_phone"], "0912345678");
    assert!(mine["items"][0]["distance_m"].as_f64().is_some());
    assert_eq!(
        own_jobs(&e, &far).await["items"].as_array().map(Vec::len),
        Some(0)
    );
    for page in ["/provider/jobs", "/provider/dashboard"] {
        let res = send(
            &e.router,
            Request::get(page)
                .header(header::ACCEPT, "text/html")
                .header(header::AUTHORIZATION, format!("Bearer {near}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK, "{page}");
        let html = body_text(res).await;
        assert!(
            html.contains("Phun thuoc 3 cong lua") || html.contains("1 tin đang tìm người"),
            "{page}"
        );
    }

    // A phone has at most JOB_POSTS_OPEN_PER_PHONE open posts.
    for _ in 0..2 {
        let res = send(&e.router, post_form("/jobs", &job, None)).await;
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
    }
    let res = send(&e.router, post_form("/jobs", &job, None)).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(res)
        .await
        .contains("nhiều tin cần mướn chưa xong"));

    // The admin marks the first spam: it leaves the provider's page.
    let first = scalar(&e, "SELECT MIN(id)::text AS v FROM job_posts").await;
    let res = send(
        &e.router,
        post_form(
            &format!("/admin/jobs/{first}/status"),
            &[("status", "spam"), ("reason", "tin ảo")],
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        own_jobs(&e, &near).await["items"].as_array().map(Vec::len),
        Some(2)
    );
    let res = send(&e.router, get_page("/admin/jobs", Some(&admin))).await;
    assert_eq!(res.status(), StatusCode::OK);
    let html = body_text(res).await;
    assert!(html.contains(r#"<span class="badge text-bg-secondary">Rác</span>"#));
    assert!(
        html.contains(r#"name="status" value="open""#),
        "a spam post reopens"
    );
    assert!(!html.contains(&format!("/admin/jobs?spam={first}#xac-nhan")));
    let html =
        body_text(send(&e.router, get_page("/admin/jobs?state=spam", Some(&admin))).await).await;
    assert!(!html.contains("Đang tìm người</span>"), "only the spam one");
    assert_eq!(
        scalar(
            &e,
            &format!(
                "SELECT reason AS v FROM audit_logs WHERE entity_type = 'job_post'                  AND entity_id = {first}"
            )
        )
        .await,
        "spam: tin ảo"
    );
    // A provider cannot moderate.
    let res = send(
        &e.router,
        Request::patch(format!("/api/admin/jobs/{first}/status"))
            .header(header::AUTHORIZATION, format!("Bearer {near}"))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(r#"{"status":"open"}"#))
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // The way in: the home page's card, the form itself.
    let html = body_text(send(&e.router, get_page("/", None)).await).await;
    assert!(html.contains("Chưa tìm được nhà cung cấp?"));
    let res = send(&e.router, get_page("/jobs/new", None)).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn job_posts_past_keeping_are_deleted_whatever_their_status() {
    let e = Env::new().await;
    let category = categories_of(&e, "service").await[0].0;
    let area = scalar(&e, "SELECT min(id)::text AS v FROM locations").await;
    let point = "ST_GeographyFromText('SRID=4326;POINT(106.09 10.42)')";
    e.t.db
        .execute_unprepared(&format!(
            "INSERT INTO job_posts
               (service_type_id, location_id, location, description, requester_name,
                requester_phone, status, created_on)
             VALUES
               ({category}, {area}, {point}, 'Phun thuoc', 'Chu Ba', '0912345678', 'open',
                now() - interval '13 months'),
               ({category}, {area}, {point}, 'Muc bun', 'Chu Ba', '0912345678', 'closed',
                now() - interval '13 months'),
               ({category}, {area}, {point}, 'Sua may', 'Chu Ba', '0912345678', 'closed',
                now() - interval '11 months')"
        ))
        .await
        .unwrap();

    let jobs = &e.state.usecases.job;
    assert_eq!(jobs.purge_old_posts(chrono::Utc::now()).await.unwrap(), 2);
    assert_eq!(
        scalar(
            &e,
            "SELECT string_agg(description, ',') AS v FROM job_posts"
        )
        .await,
        "Sua may",
        "a post under a year old stays"
    );
    assert_eq!(jobs.purge_old_posts(chrono::Utc::now()).await.unwrap(), 0);
    e.cleanup().await;
}

#[tokio::test]
#[ignore = "needs TEST_DATABASE_URL"]
async fn one_client_posts_a_few_jobs_a_day_whatever_phones_it_types() {
    let e = env_with_origin().await;
    let category = categories_of(&e, "service").await[0].0;
    let area = scalar(&e, "SELECT min(id)::text AS v FROM locations").await;
    let post_from = |phone: String| {
        Request::post("/api/explore/jobs")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(format!(
                r#"{{"category_id":{category},"location_id":{area},"description":"Phun thuoc",
                    "requester_name":"Chu Ba","requester_phone":"{phone}"}}"#
            )))
            .unwrap()
    };
    let limit = application::constants::JOB_POSTS_PER_CLIENT_PER_DAY_DEFAULT;
    for n in 0..limit {
        let res = send(&e.router, post_from(format!("09123450{n:02}"))).await;
        assert_eq!(res.status(), StatusCode::CREATED, "post {n}");
    }
    // A new phone does not help: the client is the same.
    let res = send(&e.router, post_from("0987654321".into())).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json_body(res).await["code"], "job_post_daily_limit");
    // Another client is not held back by it.
    let res =
        integration_tests::send_from(&e.router, post_from("0987654321".into()), [203, 0, 113, 9])
            .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    e.cleanup().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs TEST_DATABASE_URL"]
async fn posts_sent_at_the_same_moment_never_pass_a_cap() {
    let e = env_with_origin().await;
    let category = categories_of(&e, "service").await[0].0;
    let area = scalar(&e, "SELECT min(id)::text AS v FROM locations").await;
    let post = move |phone: String| {
        Request::post("/api/explore/jobs")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, TEST_ORIGIN)
            .body(Body::from(format!(
                r#"{{"category_id":{category},"location_id":{area},"description":"Phun thuoc",
                    "requester_name":"Chu Ba","requester_phone":"{phone}"}}"#
            )))
            .unwrap()
    };
    // Sends every request at once; how many were taken.
    let at_once = |requests: Vec<(Request<Body>, [u8; 4])>| {
        let router = e.router.clone();
        async move {
            let mut runs = tokio::task::JoinSet::new();
            for (req, peer) in requests {
                let router = router.clone();
                runs.spawn(async move { integration_tests::send_from(&router, req, peer).await });
            }
            let mut created = 0;
            while let Some(res) = runs.join_next().await {
                if res.unwrap().status() == StatusCode::CREATED {
                    created += 1;
                }
            }
            created
        }
    };

    // One phone from many addresses: only its open-post cap.
    let one_phone = (0..10_u8)
        .map(|n| (post("0912345678".into()), [10, 1, 0, n]))
        .collect();
    assert_eq!(
        at_once(one_phone).await,
        application::constants::JOB_POSTS_OPEN_PER_PHONE_DEFAULT,
        "one phone"
    );

    // One address typing many phones: only its daily cap.
    let one_client = (0..10_u8)
        .map(|n| (post(format!("09876543{n:02}")), [10, 2, 0, 1]))
        .collect();
    assert_eq!(
        at_once(one_client).await,
        application::constants::JOB_POSTS_PER_CLIENT_PER_DAY_DEFAULT,
        "one address"
    );
    e.cleanup().await;
}
