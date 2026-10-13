use super::*;
use axum::{
    body::Body,
    http::{Method, Request, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn item_icons_are_opt_in_durable_deduplicated_private_and_retryable() {
    use axum::{Json, routing::post};
    use base64::Engine;
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(512, 512, |x, _| {
        if x < 50 {
            image::Rgba([0, 0, 0, 0])
        } else {
            image::Rgba([255, 100, 0, 255])
        }
    }))
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(png.get_ref());
    let failing = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let fail = failing.clone();
    let received = calls.clone();
    let fixture = Router::new().route(
        "/v1/images/generations",
        post(move |Json(body): Json<Value>| {
            received.lock().unwrap().push(body);
            let fail = fail.swap(false, Ordering::SeqCst);
            let encoded = encoded.clone();
            async move {
                if fail {
                    (StatusCode::TOO_MANY_REQUESTS, Json(json!({})))
                } else {
                    (StatusCode::OK, Json(json!({"data":[{"b64_json":encoded}]})))
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let (server, dir) = server();
    let owner = owner(&server).await;
    let mut config = json!({"enabled":true,"baseUrl":format!("http://{address}/v1"),"model":"fixture-chat","imageModel":"gpt-image-2","imageSize":"1024x1024","responseFormat":"json","tokenParameter":"omit"});
    let (_, saved, _) = request(
        &server,
        Method::PUT,
        "/api/ai/config",
        json!({"config":config,"apiKey":"fixture"}),
        Some(&owner),
    )
    .await;
    assert_eq!(saved["config"]["autoItemIcons"], false);
    // Give pre-existing catalog placeholders artwork so this fixture's bulk
    // batch contains just the newly created missing item.
    let existing: Vec<_> = domain::products(&model::fresh_state(true,"UTC".into()),false).into_iter().filter(crate::icon_jobs::missing_icon).map(|product|json!({"collection":"productImages","key":product.id,"field":null,"before":null,"after":"/images/pasta.jpg"})).collect();
    assert_eq!(
        operation(
            &server,
            &owner,
            json!({"type":"patch","label":"Existing catalog artwork","changes":existing})
        )
        .await
        .0,
        StatusCode::OK
    );
    for (id, icon) in [("custom-missing", "🛒"), ("custom-emoji", "🍎")] {
        assert_eq!(operation(&server,&owner,json!({"type":"patch","label":"Create item","changes":[{"collection":"customProducts","key":id,"field":null,"before":null,"after":{"id":id,"name":id,"category":"Fruits","icon":icon}}]})).await.0,StatusCode::OK);
    }
    assert!(
        !crate::icon_jobs::run_one(&server).await.unwrap(),
        "Generation is off by default"
    );
    let (_, jobs, _) = request(
        &server,
        Method::GET,
        "/api/ai/item-icons",
        Value::Null,
        Some(&owner),
    )
    .await;
    assert_eq!(jobs["missing"], 1);
    assert_eq!(jobs["queued"], 0);
    for _ in 0..2 {
        assert_eq!(
            request(
                &server,
                Method::POST,
                "/api/ai/item-icons",
                json!({"action":"generate"}),
                Some(&owner)
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let reopened = db::Database::open(&dir.path().join("test.sqlite3")).unwrap();
    assert_eq!(
        reopened
            .lock()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM item_icon_jobs WHERE status='queued'",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
    drop(reopened);
    failing.store(true, Ordering::SeqCst);
    assert!(crate::icon_jobs::run_one(&server).await.unwrap());
    assert!(
        !crate::icon_jobs::run_one(&server).await.unwrap(),
        "Provider failures must not retry automatically"
    );
    let (_, jobs, _) = request(
        &server,
        Method::GET,
        "/api/ai/item-icons",
        Value::Null,
        Some(&owner),
    )
    .await;
    assert_eq!(jobs["failed"], 1);
    request(
        &server,
        Method::POST,
        "/api/ai/item-icons",
        json!({"action":"retry"}),
        Some(&owner),
    )
    .await;
    assert!(crate::icon_jobs::run_one(&server).await.unwrap());
    let (_, snapshot, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&owner),
    )
    .await;
    let url = snapshot["state"]["productImages"]["custom-missing"]
        .as_str()
        .unwrap();
    assert!(snapshot["state"]["productImages"]["custom-emoji"].is_null());
    assert!(snapshot["state"]["productImages"]["apples"].is_null());
    let household = server
        .db
        .lock()
        .unwrap()
        .query_row("SELECT id FROM households", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap();
    let bytes: Vec<u8> = server
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT bytes FROM uploads WHERE household_id=?1 AND id=?2",
            rusqlite::params![household, url.trim_start_matches("/api/images/")],
            |row| row.get(0),
        )
        .unwrap();
    let decoded = image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_eq!(decoded.dimensions(), (256, 256));
    assert_eq!(decoded.get_pixel(0, 0)[3], 0);
    assert_eq!(
        request(&server, Method::GET, url, Value::Null, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert!(
        calls
            .lock()
            .unwrap()
            .iter()
            .all(|body| body["background"] == "transparent"
                && body["size"] == "816x816"
                && body["prompt"].as_str().unwrap().contains("Twemoji"))
    );
    config["autoItemIcons"] = json!(true);
    request(
        &server,
        Method::PUT,
        "/api/ai/config",
        json!({"config":config}),
        Some(&owner),
    )
    .await;
    let mutation = json!({"id":uuid::Uuid::new_v4().to_string(),"operation":{"type":"patch","label":"Create automatic item","changes":[{"collection":"customProducts","key":"custom-auto","field":null,"before":null,"after":{"id":"custom-auto","name":"Automatic item","category":"Fruits","icon":"🛒"}}]}});
    for _ in 0..2 {
        assert_eq!(
            request(
                &server,
                Method::POST,
                "/api/operations",
                mutation.clone(),
                Some(&owner)
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let (_, jobs, _) = request(
        &server,
        Method::GET,
        "/api/ai/item-icons",
        Value::Null,
        Some(&owner),
    )
    .await;
    assert_eq!(jobs["queued"], 1);
    {
        let conn = server.db.lock().unwrap();
        for number in 0..20 {
            conn.execute(
                "INSERT INTO item_icon_attempts(id,household_id) VALUES(?1,?2)",
                rusqlite::params![format!("pace-{number}"), household],
            )
            .unwrap();
        }
    }
    assert!(
        !crate::icon_jobs::run_one(&server).await.unwrap(),
        "A full hourly budget keeps the remaining work queued"
    );
    {
        let conn = server.db.lock().unwrap();
        conn.execute(
            "UPDATE item_icon_attempts SET started_at=unixepoch()-3601 WHERE id LIKE 'pace-%'",
            [],
        )
        .unwrap();
        conn.execute("UPDATE item_icon_jobs SET status='running',run_id='interrupted' WHERE product_id='custom-auto'",[]).unwrap();
    }
    crate::icon_jobs::resume_interrupted(&server).unwrap();
    request(
        &server,
        Method::POST,
        "/api/ai/item-icons",
        json!({"action":"generate"}),
        Some(&owner),
    )
    .await;
    config["autoItemIcons"] = json!(false);
    request(
        &server,
        Method::PUT,
        "/api/ai/config",
        json!({"config":config}),
        Some(&owner),
    )
    .await;
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/ai/item-icons",
            Value::Null,
            Some(&owner)
        )
        .await
        .1["queued"],
        1,
        "A manual batch adopts queued automatic work and survives switching auto off"
    );
    // A partner can create missing items but cannot spend quota on bulk jobs.
    let (_, invite, _) = request(
        &server,
        Method::POST,
        "/api/invitations",
        json!({"email":"wife@example.test"}),
        Some(&owner),
    )
    .await;
    let token = invite["url"]
        .as_str()
        .unwrap()
        .split("invite=")
        .nth(1)
        .unwrap();
    let (_,_,wife)=request(&server,Method::POST,"/api/auth/join",json!({"token":token,"email":"wife@example.test","name":"Wife","password":"another good long password"}),None).await;
    for method in [Method::GET, Method::POST] {
        assert_eq!(
            request(
                &server,
                method,
                "/api/ai/item-icons",
                json!({"action":"generate"}),
                wife.as_deref()
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert!(crate::icon_jobs::run_one(&server).await.unwrap());
    let (_, shared, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        wife.as_deref(),
    )
    .await;
    assert!(
        shared["state"]["productImages"]["custom-auto"]
            .as_str()
            .is_some()
    );
    config["autoItemIcons"] = json!(false);
    request(
        &server,
        Method::PUT,
        "/api/ai/config",
        json!({"config":config}),
        Some(&owner),
    )
    .await;
    task.abort();
}

#[tokio::test]
async fn background_icons_do_not_overwrite_edits_or_apply_after_canceling() {
    use axum::{Json, routing::post};
    use base64::Engine;
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(32, 32)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(png.get_ref());
    let begin = started.clone();
    let gate = release.clone();
    let fixture = Router::new().route(
        "/v1/images/generations",
        post(move || {
            let begin = begin.clone();
            let gate = gate.clone();
            let encoded = encoded.clone();
            async move {
                begin.notify_one();
                gate.notified().await;
                Json(json!({"data":[{"b64_json":encoded}]}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let (server, _dir) = server();
    let owner = owner(&server).await;
    let mut config = json!({"enabled":true,"autoItemIcons":true,"baseUrl":format!("http://{address}/v1"),"model":"fixture-chat","imageModel":"gpt-image-2","imageSize":"1024x1024","responseFormat":"json","tokenParameter":"omit"});
    for action in ["upload", "delete", "archive", "cancel", "disable", "rename"] {
        config["autoItemIcons"] = json!(true);
        request(
            &server,
            Method::PUT,
            "/api/ai/config",
            json!({"config":config,"apiKey":"fixture"}),
            Some(&owner),
        )
        .await;
        let product = json!({"id":format!("custom-{action}"),"name":format!("Item {action}"),"category":"Fruits","icon":"🛒"});
        let id = product["id"].as_str().unwrap();
        assert_eq!(operation(&server,&owner,json!({"type":"patch","label":"Create item","changes":[{"collection":"customProducts","key":id,"field":null,"before":null,"after":product}]})).await.0,StatusCode::OK);
        let runner = server.clone();
        let generation =
            tokio::spawn(async move { crate::icon_jobs::run_one(&runner).await.unwrap() });
        tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
            .await
            .unwrap();
        let change = match action {
            "upload" => Some(
                json!({"collection":"productImages","key":id,"field":null,"before":null,"after":"/images/pasta.jpg"}),
            ),
            "delete" => {
                let (_, snapshot, _) = request(
                    &server,
                    Method::GET,
                    "/api/state",
                    Value::Null,
                    Some(&owner),
                )
                .await;
                let stored = snapshot["state"]["customProducts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|entry| entry["id"] == id)
                    .unwrap();
                Some(
                    json!({"collection":"customProducts","key":id,"field":null,"before":stored,"after":null}),
                )
            }
            "archive" => Some(
                json!({"collection":"archivedProductIds","key":"","field":null,"before":[],"after":[id]}),
            ),
            "rename" => Some(
                json!({"collection":"customProducts","key":id,"field":"name","before":product["name"],"after":"Changed item"}),
            ),
            _ => None,
        };
        if let Some(change) = change {
            assert_eq!(
                operation(
                    &server,
                    &owner,
                    json!({"type":"patch","label":"Edit while generating","changes":[change]})
                )
                .await
                .0,
                StatusCode::OK
            );
        }
        if action == "cancel" {
            request(
                &server,
                Method::POST,
                "/api/ai/item-icons",
                json!({"action":"cancel"}),
                Some(&owner),
            )
            .await;
        }
        if action == "disable" {
            config["autoItemIcons"] = json!(false);
            request(
                &server,
                Method::PUT,
                "/api/ai/config",
                json!({"config":config}),
                Some(&owner),
            )
            .await;
        }
        release.notify_one();
        assert!(generation.await.unwrap());
        let (_, snapshot, _) = request(
            &server,
            Method::GET,
            "/api/state",
            Value::Null,
            Some(&owner),
        )
        .await;
        assert_eq!(
            snapshot["state"]["productImages"][id],
            if action == "upload" {
                json!("/images/pasta.jpg")
            } else {
                Value::Null
            },
            "{action}"
        );
    }
    assert_eq!(
        server
            .db
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM uploads", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        0,
        "Discarded generation must not leak unused uploads"
    );
    task.abort();
}

#[test]
fn restoring_a_purchase_removes_pantry_stock_including_released_meal_surplus() {
    let today = "2026-10-01";
    for scenario in ["manual", "planned", "released"] {
        let mut state = model::fresh_state(true, "UTC".into());
        let id = if scenario == "manual" {
            "cherry-tomatoes"
        } else {
            "pasta"
        };
        let quantity = if scenario == "manual" { 2.0 } else { 800.0 };
        let unit = if scenario == "manual" { "pc" } else { "g" };
        if scenario != "manual" {
            state.plan.insert(today.into(), "green-pasta".into());
        }
        domain::apply(
            &mut state,
            &model::Operation::AddItem {
                product_id: id.into(),
                quantity,
                unit: unit.into(),
                note: "Keep this note".into(),
                accumulate: false,
            },
            today,
        )
        .unwrap();
        domain::apply(
            &mut state,
            &model::Operation::CheckItem {
                id: id.into(),
                checked: true,
                expected: None,
            },
            today,
        )
        .unwrap();
        assert!(state.pantry.contains_key(id));
        if scenario == "released" {
            domain::apply(
                &mut state,
                &model::Operation::Patch {
                    label: "Remove lunch".into(),
                    changes: vec![model::Change {
                        collection: "plan".into(),
                        key: today.into(),
                        field: None,
                        before: json!("green-pasta"),
                        after: Value::Null,
                    }],
                },
                today,
            )
            .unwrap();
            assert_eq!(state.pantry[id].amounts[0].quantity, 800.0);
        }
        let expected = domain::displayed(&state, id, today).unwrap();
        assert!(expected.checked);
        domain::apply(
            &mut state,
            &model::Operation::CheckItem {
                id: id.into(),
                checked: false,
                expected: Some(Box::new(expected)),
            },
            today,
        )
        .unwrap();
        assert!(!state.pantry.contains_key(id), "{scenario}");
        assert!(
            state
                .purchased_coverage
                .values()
                .all(|products| products.get(id).is_none_or(|amounts| amounts.is_empty()))
        );
        let restored = domain::displayed(&state, id, today).unwrap();
        assert!(!restored.checked);
        assert_eq!(restored.note, "Keep this note");
        let mut reloaded: model::HouseholdState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        domain::apply(
            &mut reloaded,
            &model::Operation::Patch {
                label: "Change a setting".into(),
                changes: vec![],
            },
            today,
        )
        .unwrap();
        assert!(!reloaded.pantry.contains_key(id));
        if scenario == "released" {
            reloaded.plan.insert(today.into(), "green-pasta".into());
            assert!(
                domain::outstanding(&reloaded, today)
                    .iter()
                    .any(|credit| credit.product_id == id)
            );
        }
    }
}

#[test]
fn pantry_removal_accepts_the_same_whole_number_serialized_by_a_browser() {
    let mut state = model::fresh_state(false, "UTC".into());
    state.pantry.insert(
        "cherry-tomatoes".into(),
        model::PantryItem {
            amounts: vec![model::Amount {
                quantity: 2.0,
                unit: "pc".into(),
            }],
            confirmed: false,
            observed_at: "2026-10-01T12:00:00+00:00".into(),
            expires_at: None,
            note: "".into(),
        },
    );
    let before = json!({"amounts":[{"quantity":2,"unit":"pc"}],"confirmed":false,"observedAt":"2026-10-01T12:00:00+00:00","expiresAt":null,"note":""});
    let change = model::Change {
        collection: "pantry".into(),
        key: "cherry-tomatoes".into(),
        field: None,
        before,
        after: Value::Null,
    };
    domain::apply_changes(&mut state, &[change]).unwrap();
    assert!(!state.pantry.contains_key("cherry-tomatoes"));
}

#[test]
fn browser_number_comparison_preserves_actual_conflicts_and_large_integer_precision() {
    assert!(domain::json_equal(
        &json!({"quantity": 2.0, "nested":[1.0,0.5]}),
        &json!({"quantity":2,"nested":[1,0.5]})
    ));
    assert!(!domain::json_equal(
        &json!({"quantity":2.0}),
        &json!({"quantity":3})
    ));
    assert!(!domain::json_equal(
        &json!(9007199254740993u64),
        &json!(9007199254740992.0)
    ));
    assert!(!domain::json_equal(
        &json!({"quantity":2}),
        &json!({"quantity":2,"note":null})
    ));
    let mut state = model::fresh_state(false, "UTC".into());
    state.pantry.insert(
        "cherry-tomatoes".into(),
        model::PantryItem {
            amounts: vec![model::Amount {
                quantity: 3.0,
                unit: "pc".into(),
            }],
            confirmed: true,
            observed_at: "2026-10-01T12:00:00+00:00".into(),
            expires_at: None,
            note: "".into(),
        },
    );
    let mut before = serde_json::to_value(&state.pantry["cherry-tomatoes"]).unwrap();
    before["amounts"][0]["quantity"] = json!(2);
    let changes = [model::Change {
        collection: "pantry".into(),
        key: "cherry-tomatoes".into(),
        field: None,
        before,
        after: Value::Null,
    }];
    assert!(domain::apply_changes(&mut state, &changes).is_err());
    assert_eq!(state.pantry["cherry-tomatoes"].amounts[0].quantity, 3.0);
}

#[tokio::test]
async fn household_catalog_overrides_are_validated_and_archives_keep_recipe_references() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let mut pasta = model::seed()
        .catalog
        .into_iter()
        .find(|product| product.id == "pasta")
        .unwrap();
    pasta.name = "Family pasta".into();
    pasta.aliases = vec!["family noodles".into()];
    let details = serde_json::to_value(&pasta).unwrap();
    let (status, state) = operation(&server, &cookie, json!({"type":"patch","label":"Edit catalog","changes":[{"collection":"productDetails","key":"pasta","field":null,"before":null,"after":details},{"collection":"archivedProductIds","key":"","field":null,"before":[],"after":["pasta"]}]})).await;
    assert_eq!(status, StatusCode::OK, "{state}");
    let household: model::HouseholdState = serde_json::from_value(state["state"].clone()).unwrap();
    assert_eq!(
        domain::product(&household, "pasta").unwrap().name,
        "Family pasta"
    );
    assert!(
        !domain::products(&household, false)
            .iter()
            .any(|product| product.id == "pasta")
    );
    assert!(
        domain::products(&household, true)
            .iter()
            .any(|product| product.id == "pasta")
    );
    let invalid = operation(&server,&cookie,json!({"type":"patch","label":"Invalid edit","changes":[{"collection":"productDetails","key":"missing-product","field":null,"before":null,"after":details}]})).await;
    assert_eq!(invalid.0, StatusCode::BAD_REQUEST);
    let stale = operation(&server,&cookie,json!({"type":"patch","label":"Stale edit","changes":[{"collection":"productDetails","key":"pasta","field":null,"before":null,"after":null}]})).await;
    assert_eq!(stale.0, StatusCode::CONFLICT);
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/state",
            Value::Null,
            Some(&cookie)
        )
        .await
        .1["state"]["productDetails"]["pasta"]["name"],
        "Family pasta"
    );
}

fn worker_headers(token: &str) -> axum::http::HeaderMap {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
    headers
}

#[tokio::test]
async fn companion_jobs_are_scoped_claimed_once_revocable_and_forward_requested_reasoning() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let household: String = server
        .db
        .lock()
        .unwrap()
        .query_row("SELECT id FROM households", [], |r| r.get(0))
        .unwrap();
    let pairing = ai::provision_companion(&server, &household, "gpt-6-luna", "xhigh").unwrap();
    let token = pairing["token"].as_str().unwrap();
    let headers = worker_headers(token);
    let (_, config, _) = request(
        &server,
        Method::GET,
        "/api/ai/config",
        Value::Null,
        Some(&cookie),
    )
    .await;
    assert_eq!(config["config"]["transport"], "companion");
    assert_eq!(config["config"]["reasoningEffort"], "xhigh");
    assert!(!config.to_string().contains(token));
    assert_eq!(config["companion"]["online"], false);
    assert!(
        companion::send(&server, &household, "models", None)
            .await
            .is_err()
    );
    let _ = companion::heartbeat(axum::extract::State(server.clone()), headers.clone())
        .await
        .unwrap();
    let second_state: String = server
        .db
        .lock()
        .unwrap()
        .query_row("SELECT state FROM households", [], |r| r.get(0))
        .unwrap();
    server
        .db
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO households(id,name,state) VALUES('other','Other',?1)",
            [second_state],
        )
        .unwrap();
    let other = worker_headers(&companion::rotate(&server, "other").unwrap());
    let test_server = server.clone();
    let test_cookie = cookie.clone();
    let waiting = tokio::spawn(async move {
        request(&test_server,Method::POST,"/api/ai/test",json!({"config":{"enabled":true,"transport":"companion","baseUrl":"https://api.openai.com/v1","model":"gpt-6-luna","reasoningEffort":"xhigh","imageModel":"gpt-image-2-medium","imageSize":"1024x1024","responseFormat":"schema","tokenParameter":"omit"}}),Some(&test_cookie)).await
    });
    let mut job = Value::Null;
    for _ in 0..100 {
        job = companion::claim(axum::extract::State(server.clone()), headers.clone())
            .await
            .unwrap()
            .0["job"]
            .clone();
        if !job.is_null() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(job["body"]["model"], "gpt-6-luna");
    assert_eq!(job["body"]["reasoning_effort"], "xhigh");
    assert!(
        companion::claim(axum::extract::State(server.clone()), headers.clone())
            .await
            .unwrap()
            .0["job"]
            .is_null()
    );
    assert!(
        companion::claim(axum::extract::State(server.clone()), other.clone())
            .await
            .unwrap()
            .0["job"]
            .is_null()
    );
    let payload = || companion::Completion {
        id: job["id"].as_str().unwrap().into(),
        lease: job["lease"].as_str().unwrap().into(),
        failed: false,
        result: Some(json!({"choices":[{"message":{"content":"Connected"}}]})),
    };
    assert!(
        companion::complete(axum::extract::State(server.clone()), other, Json(payload()))
            .await
            .is_err()
    );
    let mut invalid = payload();
    invalid.lease = "a".repeat(64);
    assert!(
        companion::complete(
            axum::extract::State(server.clone()),
            headers.clone(),
            Json(invalid)
        )
        .await
        .is_err()
    );
    let _ = companion::complete(
        axum::extract::State(server.clone()),
        headers.clone(),
        Json(payload()),
    )
    .await
    .unwrap();
    assert!(
        companion::complete(
            axum::extract::State(server.clone()),
            headers.clone(),
            Json(payload())
        )
        .await
        .is_err()
    );
    assert_eq!(waiting.await.unwrap().0, StatusCode::OK);
    assert_eq!(
        server
            .db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM companion_jobs", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    companion::rotate(&server, &household).unwrap();
    assert!(
        companion::claim(axum::extract::State(server.clone()), headers)
            .await
            .is_err()
    );
    let request = Request::builder()
        .uri("/api/state")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app(server).oneshot(request).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

pub(super) fn server() -> (Server, tempfile::TempDir) {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let config = Arc::new(Config {
        origin: "http://localhost:3101".into(),
        secure_cookie: false,
        setup_token: "test-bootstrap-secret".into(),
        data_dir: directory.path().into(),
        encryption_key: [19u8; 32],
    });
    let (notices, _) = broadcast::channel(64);
    (
        Server {
            db: db::Database::open(&directory.path().join("test.sqlite3"))
                .map_err(|error| error.1)
                .unwrap(),
            config,
            notices,
            dummy_hash: auth::dummy_hash().into(),
            client: reqwest::Client::new(),
            limits: Arc::new(Mutex::new(HashMap::new())),
        },
        directory,
    )
}
pub(super) async fn request(
    server: &Server,
    method: Method,
    path: &str,
    body: Value,
    cookie: Option<&str>,
) -> (StatusCode, Value, Option<String>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-littlewardrobe-request", "1")
        .header(header::ORIGIN, &server.config.origin);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = app(server.clone())
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .map(|cookie| cookie.to_str().unwrap().split(';').next().unwrap().into());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
    )
}
pub(super) async fn owner(server: &Server) -> String {
    let (status,body,cookie)=request(server,Method::POST,"/api/auth/setup",json!({"setupToken":"test-bootstrap-secret","email":"owner@example.test","name":"Owner","password":"a sufficiently long password","householdName":"Our family","timeZone":"Europe/Helsinki","samples":true}),None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["recoveryCode"].as_str().unwrap().len() >= 32);
    cookie.unwrap()
}
pub(super) async fn operation(
    server: &Server,
    cookie: &str,
    operation: Value,
) -> (StatusCode, Value) {
    let (status, body, _) = request(
        server,
        Method::POST,
        "/api/operations",
        json!({"id":uuid::Uuid::new_v4().to_string(),"operation":operation}),
        Some(cookie),
    )
    .await;
    (status, body)
}

#[tokio::test]
async fn accounts_require_setup_token_then_share_invited_household_and_reject_unauthenticated_access()
 {
    let (server, _dir) = server();
    assert_eq!(
        request(&server, Method::GET, "/api/state", Value::Null, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let cookie = owner(&server).await;
    let (_, owner_info, _) = request(
        &server,
        Method::GET,
        "/api/auth/me",
        Value::Null,
        Some(&cookie),
    )
    .await;
    assert_eq!(owner_info["user"]["role"], "owner");
    let (_, invite, _) = request(
        &server,
        Method::POST,
        "/api/invitations",
        json!({"email":"wife@example.test"}),
        Some(&cookie),
    )
    .await;
    let token = invite["url"]
        .as_str()
        .unwrap()
        .split("invite=")
        .nth(1)
        .unwrap();
    let (status,wife,wife_cookie)=request(&server,Method::POST,"/api/auth/join",json!({"token":token,"email":"wife@example.test","name":"Wife","password":"another good long password"}),None).await;
    assert_eq!(status, StatusCode::OK, "{wife}");
    assert_eq!(wife["session"]["householdId"], owner_info["householdId"]);
    let (status,_)=operation(&server,&cookie,json!({"type":"addItem","productId":"pasta","quantity":400,"unit":"g","note":"Whole wheat","accumulate":true})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, shared, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        wife_cookie.as_deref(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(shared["state"]["items"][0]["note"], "Whole wheat");
    assert_eq!(
        request(
            &server,
            Method::POST,
            "/api/invitations",
            json!({"email":"child@example.test"}),
            wife_cookie.as_deref()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(request(&server,Method::POST,"/api/auth/join",json!({"token":token,"email":"wife@example.test","name":"Repeat","password":"another good long password"}),None).await.0,StatusCode::BAD_REQUEST);
    request(
        &server,
        Method::POST,
        "/api/auth/logout",
        json!({}),
        wife_cookie.as_deref(),
    )
    .await;
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/state",
            Value::Null,
            wife_cookie.as_deref()
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn mutation_retries_are_idempotent_and_stale_edits_fail_without_losing_other_changes() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let mutation = json!({"id":uuid::Uuid::new_v4().to_string(),"operation":{"type":"addItem","productId":"pasta","quantity":200,"unit":"g","note":"","accumulate":true}});
    let (status, first, _) = request(
        &server,
        Method::POST,
        "/api/operations",
        mutation.clone(),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (_, again, _) = request(
        &server,
        Method::POST,
        "/api/operations",
        mutation,
        Some(&cookie),
    )
    .await;
    assert_eq!(again["revision"], first["revision"]);
    assert_eq!(again["state"]["items"][0]["quantity"].as_f64(), Some(200.0));
    let patch = |before: &str, after: &str| json!({"type":"patch","label":"Description","changes":[{"collection":"items","key":"pasta","field":"note","before":before,"after":after}]});
    assert_eq!(
        operation(&server, &cookie, patch("", "Penne")).await.0,
        StatusCode::OK
    );
    assert_eq!(
        operation(&server, &cookie, patch("", "Rigatoni")).await.0,
        StatusCode::CONFLICT
    );
    let (_, state, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&cookie),
    )
    .await;
    assert_eq!(state["state"]["items"][0]["note"], "Penne");
}

#[tokio::test]
async fn independent_concurrent_additions_accumulate_atomically_and_undo_is_guarded() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let add = json!({"type":"addItem","productId":"pasta","quantity":200,"unit":"g","note":"","accumulate":true});
    let (a, b) = tokio::join!(
        operation(&server, &cookie, add.clone()),
        operation(&server, &cookie, add)
    );
    assert_eq!(a.0, StatusCode::OK);
    assert_eq!(b.0, StatusCode::OK);
    let (_, state, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&cookie),
    )
    .await;
    assert_eq!(state["state"]["items"][0]["quantity"].as_f64(), Some(400.0));
    let first = a.1["eventId"].as_str().unwrap();
    assert_eq!(
        operation(&server, &cookie, json!({"type":"undo","eventId":first}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let last = b.1["eventId"].as_str().unwrap();
    assert_eq!(
        operation(&server, &cookie, json!({"type":"undo","eventId":last}))
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn cross_origin_writes_and_requests_without_csrf_header_are_rejected() {
    let (server, _dir) = server();
    for (origin, protection) in [
        ("https://other.example", true),
        ("http://localhost:3101", false),
    ] {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, origin);
        if protection {
            request = request.header("x-littlewardrobe-request", "1");
        }
        let response=app(server.clone()).oneshot(request.body(Body::from(json!({"email":"owner@example.test","password":"some long enough password"}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

#[test]
fn automatic_requirements_merge_convert_and_credit_checks_without_reappearing() {
    let mut state = model::fresh_state(true, "Europe/Helsinki".into());
    state.plan.insert("2026-09-30".into(), "green-pasta".into());
    state.plan.insert("2026-10-01".into(), "green-pasta".into());
    let pasta = domain::displayed(&state, "pasta", "2026-09-30").unwrap();
    assert_eq!(pasta.quantity, 800.0);
    domain::apply(
        &mut state,
        &model::Operation::CheckItem {
            id: "pasta".into(),
            checked: true,
            expected: None,
        },
        "2026-09-30",
    )
    .map_err(|error| error.1)
    .unwrap();
    assert!(
        domain::displayed(&state, "pasta", "2026-09-30")
            .unwrap()
            .checked
    );
    domain::apply(
        &mut state,
        &model::Operation::CheckItem {
            id: "pasta".into(),
            checked: false,
            expected: None,
        },
        "2026-09-30",
    )
    .map_err(|error| error.1)
    .unwrap();
    assert_eq!(
        domain::displayed(&state, "pasta", "2026-09-30")
            .unwrap()
            .quantity,
        800.0
    );
}

#[test]
fn pantry_covers_plans_and_invalid_recipe_products_roll_back() {
    let mut state = model::fresh_state(true, "Europe/Helsinki".into());
    state.plan.insert("2026-09-30".into(), "green-pasta".into());
    state.pantry.insert(
        "pasta".into(),
        model::PantryItem {
            amounts: vec![model::Amount {
                quantity: 0.25,
                unit: "kg".into(),
            }],
            confirmed: true,
            observed_at: chrono::Utc::now().to_rfc3339(),
            expires_at: None,
            note: "Whole wheat".into(),
        },
    );
    assert_eq!(
        domain::displayed(&state, "pasta", "2026-09-30")
            .unwrap()
            .quantity,
        150.0
    );
    let before = state.clone();
    let changes = vec![model::Change {
        collection: "plan".into(),
        key: "2026-09-30".into(),
        field: None,
        before: json!("green-pasta"),
        after: json!("nonexistent-recipe"),
    }];
    assert!(domain::apply_changes(&mut state, &changes).is_err());
    // The enclosing SQLite transaction restores persistence on validation errors.
    assert_eq!(before.plan["2026-09-30"], "green-pasta");
}

#[tokio::test]
async fn ai_provider_keys_stay_encrypted_and_validated_proposals_apply_only_after_review() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let fixture=Router::new().route("/v1/chat/completions",post(|headers:axum::http::HeaderMap,Json(body):Json<Value>|async move {
        assert_eq!(headers.get("authorization").unwrap(),"Bearer fixture-key-never-expose");
        let content=if body["messages"][0]["role"]=="user"{"Connected".to_string()}else{json!({"message":"Try a bright rice lunch and pick up oat milk.","shopping":[{"name":"Oat milk","productId":"oat-milk","category":null,"quantity":2,"unit":"L","note":"Barista"}],"recipes":[{"name":"Spinach lemon rice","subtitle":"A fresh variation","minutes":20,"servings":4,"tags":["Vegetarian"],"ingredients":[{"name":"Rice","productId":"rice","category":null,"quantity":300,"unit":"g","note":""},{"name":"Spinach","productId":"spinach","category":null,"quantity":150,"unit":"g","note":""}],"steps":["Cook the rice.","Fold in spinach and serve."]}],"plan":[]}).to_string()};
        Json(json!({"choices":[{"message":{"content":content}}]}))
    })).route("/v1/models",get(||async{Json(json!({"data":[{"id":"fixture-chat"}]}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let provider = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let config = json!({"enabled":true,"baseUrl":format!("http://{address}/v1"),"model":"fixture-chat","imageModel":"","imageSize":"1024x1024","responseFormat":"schema","tokenParameter":"max_completion_tokens"});
    let (status, saved, _) = request(
        &server,
        Method::PUT,
        "/api/ai/config",
        json!({"config":config,"apiKey":"fixture-key-never-expose"}),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert!(saved.to_string().find("fixture-key-never-expose").is_none());
    let stored: String = server
        .db
        .lock()
        .unwrap_or_else(|error| panic!("{}", error.1))
        .query_row("SELECT secret FROM providers", [], |row| row.get(0))
        .unwrap();
    assert!(!stored.contains("fixture-key"));
    let (status, connection, _) = request(
        &server,
        Method::POST,
        "/api/ai/test",
        json!({"config":config,"apiKey":null}),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{connection}");
    let (status, proposal, _) = request(
        &server,
        Method::POST,
        "/api/ai/assistant",
        json!({"prompt":"A new vegetarian lunch and oat milk"}),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{proposal}");
    let (_, before, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&cookie),
    )
    .await;
    assert!(before["state"]["items"].as_array().unwrap().is_empty());
    assert_eq!(before["state"]["recipeBook"].as_array().unwrap().len(), 5);
    let (status,applied)=operation(&server,&cookie,json!({"type":"applyProposal","proposalId":proposal["id"],"shoppingIndices":[0],"recipeIds":[proposal["recipes"][0]["id"]],"days":[]})).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(applied["state"]["items"][0]["name"], "Oat milk");
    assert_eq!(applied["state"]["recipeBook"].as_array().unwrap().len(), 6);
    assert_eq!(operation(&server,&cookie,json!({"type":"applyProposal","proposalId":proposal["id"],"shoppingIndices":[0],"recipeIds":[],"days":[]})).await.0,StatusCode::BAD_REQUEST);
    provider.abort();
}

#[tokio::test]
async fn malformed_ai_output_and_secret_reading_by_nonowners_are_rejected() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let fixture=Router::new().route("/v1/chat/completions",post(||async{Json(json!({"choices":[{"message":{"content":"{\"message\":\"try this\",\"shopping\":[{\"name\":\"Pasta\",\"productId\":\"pasta\",\"category\":null,\"quantity\":-20,\"unit\":\"g\",\"note\":\"\"}],\"recipes\":[],\"plan\":[]}"}}]}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let provider = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let config = json!({"enabled":true,"baseUrl":format!("http://{address}/v1"),"model":"fixture-chat","imageModel":"","imageSize":"1024x1024","responseFormat":"json","tokenParameter":"omit"});
    request(
        &server,
        Method::PUT,
        "/api/ai/config",
        json!({"config":config,"apiKey":null}),
        Some(&cookie),
    )
    .await;
    assert_eq!(
        request(
            &server,
            Method::POST,
            "/api/ai/assistant",
            json!({"prompt":"Add pasta"}),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, invitation, _) = request(
        &server,
        Method::POST,
        "/api/invitations",
        json!({"email":"member@example.test"}),
        Some(&cookie),
    )
    .await;
    let token = invitation["url"]
        .as_str()
        .unwrap()
        .split("invite=")
        .nth(1)
        .unwrap();
    let (_,_,member)=request(&server,Method::POST,"/api/auth/join",json!({"token":token,"email":"member@example.test","name":"Member","password":"another long strong password"}),None).await;
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/ai/config",
            Value::Null,
            member.as_deref()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    provider.abort();
}

#[test]
fn stale_offline_checks_do_not_credit_unseen_demand_and_changed_meals_release_stock() {
    let mut state = model::fresh_state(true, "Europe/Helsinki".into());
    let today = "2026-09-30";
    state.plan.insert(today.into(), "green-pasta".into());
    let expected = domain::displayed(&state, "pasta", today).unwrap();
    state.servings.insert(today.into(), 8.0);
    assert!(
        domain::apply(
            &mut state,
            &model::Operation::CheckItem {
                id: "pasta".into(),
                checked: true,
                expected: Some(Box::new(expected))
            },
            today
        )
        .is_err()
    );
    assert!(state.plan_coverage.is_empty());
    domain::apply(
        &mut state,
        &model::Operation::CheckItem {
            id: "pasta".into(),
            checked: true,
            expected: None,
        },
        today,
    )
    .unwrap();
    assert_eq!(state.purchased_coverage[today]["pasta"][0].quantity, 800.0);
    domain::apply(
        &mut state,
        &model::Operation::Patch {
            label: "Remove lunch".into(),
            changes: vec![model::Change {
                collection: "plan".into(),
                key: today.into(),
                field: None,
                before: json!("green-pasta"),
                after: Value::Null,
            }],
        },
        today,
    )
    .unwrap();
    assert_eq!(state.pantry["pasta"].amounts[0].quantity, 800.0);
    assert!(!state.pantry["pasta"].confirmed);
    state.plan.insert(today.into(), "green-pasta".into());
    assert!(
        !domain::outstanding(&state, today)
            .iter()
            .any(|credit| credit.product_id == "pasta")
    );
}

#[tokio::test]
async fn backup_restore_is_guarded_atomic_and_owner_only() {
    let (server, _dir) = server();
    let owner = owner(&server).await;
    operation(&server,&owner,json!({"type":"addItem","productId":"coffee","quantity":2,"unit":"pack","note":"Dark roast","accumulate":true})).await;
    let (status, backup, _) = request(
        &server,
        Method::GET,
        "/api/backup",
        Value::Null,
        Some(&owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    operation(&server,&owner,json!({"type":"addItem","productId":"bananas","quantity":3,"unit":"pc","note":"","accumulate":true})).await;
    assert_eq!(
        request(
            &server,
            Method::POST,
            "/api/restore",
            json!({"backup":backup,"expectedRevision":1}),
            Some(&owner)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let snapshot = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    let (status, result, _) = request(
        &server,
        Method::POST,
        "/api/restore",
        json!({"backup":backup,"expectedRevision":snapshot["revision"]}),
        Some(&owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let restored = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    assert_eq!(restored["state"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(restored["state"]["items"][0]["note"], "Dark roast");
    assert!(restored["revision"].as_u64() > snapshot["revision"].as_u64());
    let mut invalid = backup;
    invalid["state"]["plan"]["2026-09-30"] = json!("missing-recipe");
    assert_eq!(
        request(
            &server,
            Method::POST,
            "/api/restore",
            json!({"backup":invalid,"expectedRevision":restored["revision"]}),
            Some(&owner)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/state",
            Value::Null,
            Some(&owner)
        )
        .await
        .1,
        restored
    );
}

#[tokio::test]
async fn learned_intervals_ignore_restores_and_undone_purchases() {
    let (server, _dir) = server();
    let owner = owner(&server).await;
    for days in [24, 16, 8] {
        operation(&server,&owner,json!({"type":"addItem","productId":"oat-milk","quantity":if days == 8 {4} else {2},"unit":"L","note":"Barista","accumulate":false})).await;
        operation(
            &server,
            &owner,
            json!({"type":"checkItem","id":"oat-milk","checked":true}),
        )
        .await;
        let conn = server.db.lock().unwrap();
        let id: String = conn
            .query_row(
                "SELECT id FROM events WHERE kind='checkItem' ORDER BY at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "UPDATE events SET at=?1 WHERE id=?2",
            rusqlite::params![
                (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339(),
                id
            ],
        )
        .unwrap();
    }
    let (status, insights, _) = request(
        &server,
        Method::GET,
        "/api/insights",
        Value::Null,
        Some(&owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(insights["restock"][0]["intervalDays"], 8);
    assert_eq!(insights["restock"][0]["note"], "Barista");
    assert_eq!(insights["restock"][0]["quantity"].as_f64(), Some(2.0));
    assert_eq!(insights["restock"][0]["unit"], "L");
    assert_eq!(insights["purchaseCount"], 3);
    operation(
        &server,
        &owner,
        json!({"type":"checkItem","id":"oat-milk","checked":false}),
    )
    .await;
    let insights = request(
        &server,
        Method::GET,
        "/api/insights",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    assert_eq!(insights["purchaseCount"], 2);
    assert!(insights["restock"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn compatible_image_generation_stores_a_private_resized_attachment() {
    use axum::routing::post;
    use base64::Engine;
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(24, 24)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
    let fixture = Router::new().route(
        "/v1/images/generations",
        post(move || {
            let encoded = encoded.clone();
            async move { Json(json!({"data":[{"b64_json":encoded}]})) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let (server, _dir) = server();
    let owner = owner(&server).await;
    let(status,_,_)=request(&server,Method::PUT,"/api/ai/config",json!({"config":{"enabled":true,"baseUrl":format!("http://{address}/v1"),"model":"fixture-chat","imageModel":"fixture-image","imageSize":"1024x1024","responseFormat":"json","tokenParameter":"omit"},"apiKey":"fixture-key"}),Some(&owner)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, result, _) = request(
        &server,
        Method::POST,
        "/api/ai/image",
        json!({"prompt":"A bowl of lunch"}),
        Some(&owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let url = result["url"].as_str().unwrap();
    let response = app(server.clone())
        .oneshot(
            Request::builder()
                .uri(url)
                .header(header::COOKIE, &owner)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(
        request(&server, Method::GET, url, Value::Null, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    operation(&server,&owner,json!({"type":"patch","label":"Attach photo","changes":[{"collection":"productImages","key":"coffee","field":null,"before":null,"after":url}]})).await;
    let backup = request(
        &server,
        Method::GET,
        "/api/backup",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    let snapshot = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    let (status, body, _) = request(
        &server,
        Method::POST,
        "/api/restore",
        json!({"backup":backup,"expectedRevision":snapshot["revision"]}),
        Some(&owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let restored = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    let new_url = restored["state"]["productImages"]["coffee"]
        .as_str()
        .unwrap();
    assert_ne!(new_url, url);
    let response = app(server.clone())
        .oneshot(
            Request::builder()
                .uri(new_url)
                .header(header::COOKIE, &owner)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    task.abort();
}

#[tokio::test]
async fn recipe_vision_icon_alpha_and_generated_draft_photos_are_private_and_reviewed() {
    use axum::routing::post;
    use base64::Engine;
    let rgba = image::RgbaImage::from_fn(1024, 1024, |x, y| {
        if x < 200 || y < 200 {
            image::Rgba([0, 0, 0, 0])
        } else {
            image::Rgba([255, 80, 40, 255])
        }
    });
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(png.get_ref());
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let chat_calls = calls.clone();
    let image_calls = calls.clone();
    let fixture = Router::new().route("/v1/chat/completions", post(move |Json(body):Json<Value>| {
        chat_calls.lock().unwrap().push(body);
        async { Json(json!({"choices":[{"message":{"content":json!({"message":"Transcribed. Review the notes.","shopping":[],"recipes":[{"name":"Screenshot recipe","subtitle":"## Notes\n\n**Keep chilled**.\n\n- Serve fresh","minutes":10,"servings":4,"tags":["Imported"],"ingredients":[{"name":"Synthetic truffle flakes","productId":null,"category":null,"quantity":80,"unit":"g","note":"As written"}],"steps":["Mix the ingredients.","Serve **fresh**."]}],"plan":[]}).to_string()}}]})) }
    })).route("/v1/images/generations",post(move |Json(body):Json<Value>| {
        image_calls.lock().unwrap().push(body);
        let encoded=encoded.clone();
        async move { Json(json!({"data":[{"b64_json":encoded}]})) }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let (server, _dir) = server();
    let owner = owner(&server).await;
    let household: String = server
        .db
        .lock()
        .unwrap()
        .query_row("SELECT id FROM households", [], |row| row.get(0))
        .unwrap();
    let upload = api::save_image(
        &server,
        &household,
        api::encode_image(png.get_ref()).unwrap(),
    )
    .unwrap()
    .0;
    let config = json!({"enabled":true,"baseUrl":format!("http://{address}/v1"),"model":"fixture-chat","imageModel":"gpt-image-2","imageSize":"1024x1024","responseFormat":"schema","tokenParameter":"omit","prompts":{"assistant":"Use metric quantities and concise Markdown notes.","itemImage":"Draw a flat Twemoji icon with transparent background.","recipeImage":"Photograph the finished dish in natural light."}});
    assert_eq!(
        request(
            &server,
            Method::PUT,
            "/api/ai/config",
            json!({"config":config,"apiKey":"fixture"}),
            Some(&owner)
        )
        .await
        .0,
        StatusCode::OK
    );
    for images in [
        json!(["https://example.test/private.png"]),
        json!(["/api/images/00000000-0000-0000-0000-000000000000"]),
        json!([
            upload["url"],
            upload["url"],
            upload["url"],
            upload["url"],
            upload["url"]
        ]),
    ] {
        assert_eq!(
            request(
                &server,
                Method::POST,
                "/api/ai/assistant",
                json!({"prompt":"Read this recipe","images":images}),
                Some(&owner)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, proposal, _) = request(
        &server,
        Method::POST,
        "/api/ai/assistant",
        json!({"prompt":"","images":[upload["url"]]}),
        Some(&owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{proposal}");
    {
        let seen = calls.lock().unwrap();
        assert!(
            seen[0]["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("Use metric quantities")
        );
        assert!(
            seen[0]["messages"][1]["content"][1]["image_url"]["url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/webp;base64,")
        );
    }
    let before = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    assert!(
        !before["state"]["recipeBook"]
            .as_array()
            .unwrap()
            .iter()
            .any(|recipe| recipe["name"] == "Screenshot recipe")
    );
    let (status,photo,_)=request(&server,Method::POST,"/api/ai/image",json!({"kind":"recipe","proposalId":proposal["id"],"recipeId":proposal["recipes"][0]["id"]}),Some(&owner)).await;
    assert_eq!(status, StatusCode::OK, "{photo}");
    assert!(
        calls.lock().unwrap()[1]["prompt"]
            .as_str()
            .unwrap()
            .contains("80 g Synthetic truffle flakes")
    );
    let (status,icon,_)=request(&server,Method::POST,"/api/ai/image",json!({"kind":"item","proposalId":proposal["id"],"productId":proposal["products"][0]["id"]}),Some(&owner)).await;
    assert_eq!(status, StatusCode::OK, "{icon}");
    {
        let seen = calls.lock().unwrap();
        assert_eq!(seen[2]["background"], "transparent");
        assert_eq!(seen[2]["size"], "816x816");
        assert_eq!(seen[2]["output_format"], "png");
        assert!(
            seen[2]["prompt"]
                .as_str()
                .unwrap()
                .contains("Draw a flat Twemoji icon")
        );
    }
    let (status,applied)=operation(&server,&owner,json!({"type":"applyProposal","proposalId":proposal["id"],"recipeIds":[proposal["recipes"][0]["id"]],"shoppingIndices":[],"days":[]})).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let snapshot = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    assert!(
        snapshot["state"]["recipeBook"]
            .as_array()
            .unwrap()
            .iter()
            .any(|recipe| recipe["image"] == photo["url"]
                && recipe["subtitle"]
                    .as_str()
                    .unwrap()
                    .contains("**Keep chilled**"))
    );
    assert!(
        snapshot["state"]["customProducts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| product["image"] == icon["url"])
    );
    let url = icon["url"].as_str().unwrap();
    let response = app(server.clone())
        .oneshot(
            Request::builder()
                .uri(url)
                .header(header::COOKIE, &owner)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/webp");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(decoded.dimensions(), (256, 256));
    assert_eq!(decoded.get_pixel(0, 0)[3], 0);
    assert_eq!(
        request(&server, Method::GET, url, Value::Null, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let backup = request(
        &server,
        Method::GET,
        "/api/backup",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    let (status, restored, _) = request(
        &server,
        Method::POST,
        "/api/restore",
        json!({"backup":backup,"expectedRevision":snapshot["revision"]}),
        Some(&owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    let bytes:Vec<u8>=server.db.lock().unwrap().query_row("SELECT bytes FROM uploads WHERE household_id=?1 AND mime='image/webp' ORDER BY length(bytes) LIMIT 1",[&household],|row|row.get(0)).unwrap();
    assert_eq!(
        image::load_from_memory(&bytes)
            .unwrap()
            .to_rgba8()
            .get_pixel(0, 0)[3],
        0
    );
    task.abort();
}

#[tokio::test]
async fn family_rename_is_owner_only_scoped_and_rejects_stale_names() {
    let (server, _dir) = server();
    let owner = owner(&server).await;
    let current = request(
        &server,
        Method::GET,
        "/api/auth/me",
        Value::Null,
        Some(&owner),
    )
    .await
    .1;
    let mut notices = server.notices.subscribe();
    let body = json!({"name":"New family","expectedName":current["householdName"]});
    assert_eq!(
        request(&server, Method::PUT, "/api/household", body.clone(), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    server
        .db
        .lock()
        .unwrap()
        .execute("UPDATE users SET role='member'", [])
        .unwrap();
    assert_eq!(
        request(
            &server,
            Method::PUT,
            "/api/household",
            body.clone(),
            Some(&owner)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    server
        .db
        .lock()
        .unwrap()
        .execute("UPDATE users SET role='owner'", [])
        .unwrap();
    assert_eq!(
        request(
            &server,
            Method::PUT,
            "/api/household",
            body.clone(),
            Some(&owner)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        notices.try_recv().unwrap(),
        current["householdId"].as_str().unwrap()
    );
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/auth/me",
            Value::Null,
            Some(&owner)
        )
        .await
        .1["householdName"],
        "New family"
    );
    assert_eq!(
        request(&server, Method::PUT, "/api/household", body, Some(&owner))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &server,
            Method::PUT,
            "/api/household",
            json!({"name":" ","expectedName":"New family"}),
            Some(&owner)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn pantry_confirmation_includes_reserved_purchases_and_reveals_shortages() {
    let mut state = model::fresh_state(true, "UTC".into());
    let today = "2026-09-30";
    state.plan.insert(today.into(), "green-pasta".into());
    domain::apply(
        &mut state,
        &model::Operation::CheckItem {
            id: "pasta".into(),
            checked: true,
            expected: None,
        },
        today,
    )
    .unwrap();
    domain::apply(
        &mut state,
        &model::Operation::ObservePantry {
            product_id: "pasta".into(),
            quantity: 500.0,
            unit: "g".into(),
            expires_at: None,
        },
        today,
    )
    .unwrap();
    assert_eq!(state.pantry["pasta"].amounts[0].quantity, 100.0);
    assert_eq!(state.purchased_coverage[today]["pasta"][0].quantity, 400.0);
    domain::apply(
        &mut state,
        &model::Operation::ObservePantry {
            product_id: "pasta".into(),
            quantity: 200.0,
            unit: "g".into(),
            expires_at: None,
        },
        today,
    )
    .unwrap();
    assert!(state.pantry["pasta"].amounts.is_empty());
    assert_eq!(state.purchased_coverage[today]["pasta"][0].quantity, 200.0);
    assert_eq!(
        domain::displayed(&state, "pasta", today).unwrap().quantity,
        200.0
    );
}

#[tokio::test]
async fn recovery_rotates_codes_and_invalidates_existing_sessions() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let (status, result, _) = request(
        &server,
        Method::POST,
        "/api/auth/recovery-code",
        json!({"currentPassword":"a sufficiently long password"}),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let(status,recovered,new_cookie)=request(&server,Method::POST,"/api/auth/recover",json!({"email":"owner@example.test","recoveryCode":result["recoveryCode"],"newPassword":"a replacement family passphrase"}),None).await;
    assert_eq!(status, StatusCode::OK, "{recovered}");
    assert_ne!(result["recoveryCode"], recovered["recoveryCode"]);
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/state",
            Value::Null,
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/state",
            Value::Null,
            new_cookie.as_deref()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(request(&server,Method::POST,"/api/auth/recover",json!({"email":"owner@example.test","recoveryCode":result["recoveryCode"],"newPassword":"another family passphrase"}),None).await.0,StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn households_cannot_read_each_others_state_history_or_private_images() {
    let (server, _dir) = server();
    let owner = owner(&server).await;
    operation(&server,&owner,json!({"type":"addItem","productId":"coffee","quantity":1,"unit":"pc","note":"Private household note","accumulate":false})).await;
    let other_household = uuid::Uuid::new_v4().to_string();
    let other_user = uuid::Uuid::new_v4().to_string();
    let raw = auth::token();
    {
        let conn = server.db.lock().unwrap();
        conn.execute(
            "INSERT INTO households(id,name,state) VALUES(?1,'Other household',?2)",
            rusqlite::params![
                other_household,
                serde_json::to_string(&model::fresh_state(false, "UTC".into())).unwrap()
            ],
        )
        .unwrap();
        conn.execute("INSERT INTO users(id,household_id,email,name,password_hash,role,recovery_hash) VALUES(?1,?2,'other@example.test','Other',?3,'owner','unused')",rusqlite::params![other_user,other_household,server.dummy_hash.as_ref()]).unwrap();
        conn.execute(
            "INSERT INTO sessions(token_hash,user_id,expires_at,created_at) VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                auth::digest(&raw),
                other_user,
                chrono::Utc::now().timestamp() + 3600,
                chrono::Utc::now().timestamp()
            ],
        )
        .unwrap();
        let owner_household: String = conn
            .query_row(
                "SELECT household_id FROM users WHERE email='owner@example.test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute("INSERT INTO uploads(id,household_id,mime,bytes,created_at) VALUES('private-picture',?1,'image/jpeg',?2,?3)",rusqlite::params![owner_household,vec![0u8;10],chrono::Utc::now().to_rfc3339()]).unwrap();
    }
    let other = format!("littlewardrobe_session={raw}");
    let (status, state, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&other),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(state["state"]["items"].as_array().unwrap().is_empty());
    assert!(
        request(
            &server,
            Method::GET,
            "/api/history",
            Value::Null,
            Some(&other)
        )
        .await
        .1
        .as_array()
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/images/private-picture",
            Value::Null,
            Some(&other)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_panic_holding_the_database_lock_does_not_break_later_requests() {
    let (server, _dir) = server();
    let owner = owner(&server).await;
    let db = server.db.clone();
    let panicked = std::thread::spawn(move || {
        let conn = db.lock().unwrap();
        conn.execute_batch("BEGIN IMMEDIATE; UPDATE households SET revision=revision+1000;")
            .unwrap();
        panic!("simulated handler bug");
    })
    .join();
    assert!(panicked.is_err());
    assert!(server.db.0.is_poisoned());
    {
        let conn = server
            .db
            .lock()
            .expect("a poisoned connection is recovered");
        assert!(
            conn.is_autocommit(),
            "the unfinished transaction was rolled back"
        );
        let revision: i64 = conn
            .query_row("SELECT MAX(revision) FROM households", [], |row| row.get(0))
            .unwrap();
        assert!(revision < 1000);
    }
    assert!(!server.db.0.is_poisoned());
    let (status, _, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&owner),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    operation(
        &server,
        &owner,
        json!({"type":"addItem","productId":"milk","quantity":1.0,"unit":"L","note":"","accumulate":false}),
    )
    .await;
}

#[tokio::test]
async fn handler_panics_return_json_and_rate_limits_recover() {
    let router = Router::new()
        .route(
            "/panic",
            get(|| async {
                panic!("simulated handler bug");
                #[allow(unreachable_code)]
                ""
            }),
        )
        .layer(tower_http::catch_panic::CatchPanicLayer::custom(
            super::panic_response,
        ));
    let response = router
        .oneshot(
            Request::builder()
                .uri("/panic")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        body["message"],
        "The operation could not be completed. Please try again."
    );
    let (server, _dir) = server();
    let limits = server.limits.clone();
    assert!(
        std::thread::spawn(move || {
            let _guard = limits.lock().unwrap();
            panic!("simulated limiter bug");
        })
        .join()
        .is_err()
    );
    server.rate_limit("after-panic", 2, 60).unwrap();
    assert!(!server.limits.is_poisoned());
}
