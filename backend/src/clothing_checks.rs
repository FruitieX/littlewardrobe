use super::*;
use crate::tests::{operation, owner, request, server};
use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

fn garment() -> Value {
    json!({"id":"garment-a","name":"Cotton shirt","category":"tops","brand":"Example","size":"98","sizeSystem":"cm","color":"green","material":"cotton","season":"all","condition":"good","defects":"Small mark on sleeve","status":"sell","childId":"","nextChildId":"","locationId":"","purchasePrice":12.5,"purchaseDate":"2025-06-12","purchaseSource":"Second hand","notes":"private family note","photos":[],"createdAt":"2026-10-01T12:00:00Z"})
}
fn patch(collection: &str, key: &str, before: Value, after: Value) -> Value {
    json!({"type":"patch","label":"Clothing test","changes":[{"collection":collection,"key":key,"field":null,"before":before,"after":after}]})
}
fn photo(server: &Server, owner_cookie: &str, source: &str) -> String {
    let headers = axum::http::HeaderMap::from_iter([(
        axum::http::header::COOKIE,
        owner_cookie.parse().unwrap(),
    )]);
    let actor = auth::authenticate(server, &headers).unwrap();
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(32, 32)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    server.db.lock().unwrap().execute("INSERT INTO uploads(id,household_id,mime,bytes,created_at,source_kind) VALUES(?1,?2,'image/png',?3,datetime('now'),?4)",rusqlite::params![id,actor.household_id,png.into_inner(),source]).unwrap();
    format!("/api/images/{id}")
}

#[test]
fn unreviewed_background_details_cannot_approve_a_sale() {
    let mut state = model::fresh_state(false, "UTC".into());
    let mut item: clothing::Garment = serde_json::from_value(garment()).unwrap();
    item.ai_review = true;
    let url = "/api/images/00000000-0000-4000-8000-000000000001".to_string();
    item.photos.push(clothing::GarmentPhoto {
        url: url.clone(),
        kind: "front".into(),
    });
    state.garments.push(item);
    state.sale_listings.push(clothing::SaleListing {
        id: "listing-a".into(),
        garment_ids: vec!["garment-a".into()],
        platform: "vinted".into(),
        title: "Paita".into(),
        description: "Pieni jälki hihassa".into(),
        photos: vec![url],
        status: "approved".into(),
        ..Default::default()
    });
    assert!(
        clothing::validate(&state)
            .unwrap_err()
            .1
            .contains("before approval")
    );
    state.garments[0].ai_review = false;
    clothing::validate(&state).unwrap();
}

#[tokio::test]
async fn background_photo_jobs_survive_requests_and_guard_edits_cancellation_review_and_retry() {
    let arrived = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let answer = Arc::new(Mutex::new(
        json!({"name":"AI cotton shirt","category":"tops","brand":"Example","size":"98","sizeSystem":"cm","color":"green","material":"cotton","season":"all","condition":"good","defects":"","notes":"","evidence":{"size":"Printed tag"},"uncertainFields":["condition"],"needsTagPhoto":false}),
    ));
    let (incoming, gate, reply) = (arrived.clone(), release.clone(), answer.clone());
    let fixture = Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(move || {
            let (incoming, gate, reply) = (incoming.clone(), gate.clone(), reply.clone());
            async move {
                incoming.notify_one();
                gate.notified().await;
                Json(json!({"choices":[{"message":{"content":reply.lock().unwrap().to_string()}}]}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let (server, dir) = server();
    let cookie = owner(&server).await;
    let original = photo(&server, &cookie, "original");
    let enhanced = photo(&server, &cookie, "enhanced");
    let config = json!({"enabled":true,"baseUrl":format!("http://{address}/v1"),"model":"fixture","imageModel":"","imageSize":"1024x1024","responseFormat":"schema","tokenParameter":"omit"});
    assert_eq!(
        request(
            &server,
            Method::PUT,
            "/api/ai/config",
            json!({"config":config,"apiKey":"fixture-secret"}),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(request(&server,Method::POST,"/api/ai/photo-jobs",json!({"id":uuid::Uuid::new_v4().to_string(),"photos":[{"url":enhanced,"kind":"front"}]}),Some(&cookie)).await.0,StatusCode::BAD_REQUEST);
    for mode in ["success", "edit", "cancel", "delete", "review", "fail"] {
        let id = uuid::Uuid::new_v4().to_string();
        let input = json!({"id":id,"photos":[{"url":original,"kind":"front"}]});
        let (code, created, _) = request(
            &server,
            Method::POST,
            "/api/ai/photo-jobs",
            input.clone(),
            Some(&cookie),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{created}");
        let garment_id = created["garmentId"].as_str().unwrap();
        let (_, retry, _) = request(
            &server,
            Method::POST,
            "/api/ai/photo-jobs",
            input,
            Some(&cookie),
        )
        .await;
        assert_eq!(retry, created);
        // Reopening the persisted database sees the queued work and saved photo.
        let reopened = db::Database::open(&dir.path().join("test.sqlite3")).unwrap();
        assert_eq!(
            reopened
                .lock()
                .unwrap()
                .query_row(
                    "SELECT status FROM clothing_photo_jobs WHERE id=?1",
                    [&id],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "queued"
        );
        server
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE clothing_photo_jobs SET status='running',run_id='interrupted' WHERE id=?1",
                [&id],
            )
            .unwrap();
        crate::photo_jobs::resume_interrupted(&server).unwrap();
        let app = server.clone();
        let pending = tokio::spawn(async move { crate::photo_jobs::run_one(&app).await });
        tokio::time::timeout(std::time::Duration::from_secs(5), arrived.notified())
            .await
            .unwrap();
        let (_, snapshot, _) = request(
            &server,
            Method::GET,
            "/api/state",
            Value::Null,
            Some(&cookie),
        )
        .await;
        let item = snapshot["state"]["garments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"] == garment_id)
            .unwrap()
            .clone();
        assert_eq!(item["aiReview"], true);
        match mode {
            "edit" | "review" => {
                let mut updated = item.clone();
                updated["name"] = json!("My manual name");
                if mode == "review" {
                    updated["aiReview"] = json!(false);
                }
                assert_eq!(
                    operation(
                        &server,
                        &cookie,
                        patch("garments", garment_id, item.clone(), updated)
                    )
                    .await
                    .0,
                    StatusCode::OK
                );
            }
            "delete" => {
                assert_eq!(
                    operation(
                        &server,
                        &cookie,
                        patch("garments", garment_id, item.clone(), Value::Null)
                    )
                    .await
                    .0,
                    StatusCode::OK
                );
            }
            "cancel" => {
                assert_eq!(
                    request(
                        &server,
                        Method::POST,
                        &format!("/api/ai/photo-jobs/{id}"),
                        json!({"action":"cancel"}),
                        Some(&cookie)
                    )
                    .await
                    .0,
                    StatusCode::OK
                );
            }
            "fail" => {
                answer.lock().unwrap()["season"] = json!("");
            }
            _ => {}
        }
        release.notify_one();
        assert!(pending.await.unwrap().unwrap());
        let (_, snapshot, _) = request(
            &server,
            Method::GET,
            "/api/state",
            Value::Null,
            Some(&cookie),
        )
        .await;
        let item = snapshot["state"]["garments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"] == garment_id);
        if mode == "delete" {
            assert!(item.is_none());
            continue;
        }
        let item = item.unwrap();
        assert_eq!(item["photos"][0]["url"], original);
        match mode {
            "success" => {
                assert_eq!(item["name"], "AI cotton shirt");
                assert_eq!(item["aiReview"], true);
                assert_eq!(item["aiEvidence"]["size"], "Printed tag");
            }
            "edit" => {
                assert_eq!(item["name"], "My manual name");
                assert_eq!(item["size"], "98");
            }
            "review" => {
                assert_eq!(item["name"], "My manual name");
                assert_eq!(item["size"], "");
                assert_eq!(item["aiReview"], false);
            }
            "cancel" | "fail" => {
                assert_eq!(item["name"], "Photo awaiting details");
            }
            _ => {}
        }
        if mode == "fail" {
            let (_, jobs, _) = request(
                &server,
                Method::GET,
                "/api/ai/photo-jobs",
                Value::Null,
                Some(&cookie),
            )
            .await;
            assert!(jobs.as_array().unwrap().iter().any(|j| j["id"] == id
                && j["status"] == "failed"
                && j["error"].as_str().unwrap().contains("season")));
            answer.lock().unwrap()["season"] = json!("all");
            assert_eq!(
                request(
                    &server,
                    Method::POST,
                    &format!("/api/ai/photo-jobs/{id}"),
                    json!({"action":"retry"}),
                    Some(&cookie)
                )
                .await
                .0,
                StatusCode::OK
            );
            release.notify_one();
            assert!(crate::photo_jobs::run_one(&server).await.unwrap());
            let (_, snapshot, _) = request(
                &server,
                Method::GET,
                "/api/state",
                Value::Null,
                Some(&cookie),
            )
            .await;
            assert_eq!(
                snapshot["state"]["garments"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|g| g["id"] == garment_id)
                    .unwrap()["name"],
                "AI cotton shirt"
            );
        }
    }
    task.abort();
}

#[tokio::test]
async fn clothing_fields_persist_and_stale_edits_do_not_overwrite_inventory() {
    let (server, dir) = server();
    let cookie = owner(&server).await;
    let child = json!({"id":"child-a","name":"A","birthDate":"2022-05-01","clothingSize":"104","shoeSize":"27","color":"sage"});
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("children", "child-a", Value::Null, child.clone())
        )
        .await
        .0,
        StatusCode::OK
    );
    let location = json!({"id":"box-a","name":"Winter box","note":"Blue lid"});
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("locations", "box-a", Value::Null, location)
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut item = garment();
    item["childId"] = json!("child-a");
    item["locationId"] = json!("box-a");
    let op = patch("garments", "garment-a", Value::Null, item.clone());
    let id = uuid::Uuid::new_v4().to_string();
    let (_, first, _) = request(
        &server,
        Method::POST,
        "/api/operations",
        json!({"id":id,"operation":op}),
        Some(&cookie),
    )
    .await;
    let (_, retry, _) = request(
        &server,
        Method::POST,
        "/api/operations",
        json!({"id":id,"operation":op}),
        Some(&cookie),
    )
    .await;
    assert_eq!(first["revision"], retry["revision"]);
    let mut updated = item.clone();
    updated["name"] = json!("Green cotton shirt");
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("garments", "garment-a", item.clone(), updated.clone())
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut stale = item.clone();
    stale["name"] = json!("Overwriting name");
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("garments", "garment-a", item, stale)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("children", "child-a", child, Value::Null)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let reopened = db::Database::open(&dir.path().join("test.sqlite3")).unwrap();
    let id: String = reopened
        .lock()
        .unwrap()
        .query_row("SELECT id FROM households", [], |r| r.get(0))
        .unwrap();
    let state = db::Database::snapshot(&reopened.lock().unwrap(), &id)
        .unwrap()
        .state;
    assert_eq!(state.garments[0].name, "Green cotton shirt");
    assert_eq!(state.garments[0].purchase_price, Some(12.5));
    assert_eq!(state.children[0].clothing_size, "104");
}

#[tokio::test]
async fn sale_approval_is_invalidated_and_edited_photos_cannot_be_disguised_as_originals() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let original = photo(&server, &cookie, "original");
    let enhanced = photo(&server, &cookie, "enhanced");
    assert_eq!(
        request(
            &server,
            Method::POST,
            "/api/ai/analyze",
            json!({"photos":[{"url":enhanced,"kind":"front"}],"note":""}),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut item = garment();
    item["photos"] = json!([{"url":original,"kind":"front"},{"url":enhanced,"kind":"enhanced"}]);
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("garments", "garment-a", Value::Null, item.clone())
        )
        .await
        .0,
        StatusCode::OK
    );
    let listing = json!({"id":"listing-a","garmentIds":["garment-a"],"platform":"vinted","title":"Puuvillapaita 98","description":"Pieni jälki hihassa.","price":5.0,"priceReason":"Unverified estimate","photos":[original],"status":"approved","externalUrl":"","soldPrice":null,"soldAt":"","createdAt":"2026-10-01T12:00:00Z"});
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("saleListings", "listing-a", Value::Null, listing.clone())
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/sales/listing-a/export",
            Value::Null,
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut bad = listing.clone();
    bad["photos"] = json!([enhanced]);
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("saleListings", "listing-a", listing.clone(), bad)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut relabeled = item.clone();
    relabeled["photos"][1]["kind"] = json!("front");
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("garments", "garment-a", item.clone(), relabeled)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut changed = item.clone();
    changed["size"] = json!("104");
    let (code, state) = operation(
        &server,
        &cookie,
        patch("garments", "garment-a", item, changed),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(state["state"]["saleListings"][0]["status"], "draft");
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/sales/listing-a/export",
            Value::Null,
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn clothing_references_cannot_cross_households_and_invalid_data_rolls_back() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let id = uuid::Uuid::new_v4().to_string();
    let household = uuid::Uuid::new_v4().to_string();
    server
        .db
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO households(id,name,state,revision) VALUES(?1,'Other',?2,0)",
            rusqlite::params![
                household,
                serde_json::to_string(&model::fresh_state(false, "UTC".into())).unwrap()
            ],
        )
        .unwrap();
    server.db.lock().unwrap().execute("INSERT INTO uploads(id,household_id,mime,bytes,created_at) VALUES(?1,?2,'image/png',X'00',datetime('now'))",rusqlite::params![id,household]).unwrap();
    let mut item = garment();
    item["photos"] = json!([{"url":format!("/api/images/{id}"),"kind":"front"}]);
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("garments", "garment-a", Value::Null, item)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut item = garment();
    item["purchaseDate"] = json!("2026-02-31");
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("garments", "garment-a", Value::Null, item)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, state, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&cookie),
    )
    .await;
    assert_eq!(state["state"]["garments"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn clothing_ai_reads_private_photos_returns_only_drafts_and_excludes_private_notes_from_sales()
 {
    let received = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen = received.clone();
    let fixture=Router::new().route("/v1/chat/completions",axum::routing::post(move |Json(body):Json<Value>| {
        let extraction=body["messages"][0]["content"].as_str().unwrap().contains("cataloguing ONE");
        seen.lock().unwrap().push(body);
        async move { let content=if extraction { json!({"name":"Green cotton shirt","category":"tops","brand":"Example","size":"","sizeSystem":"cm","color":"green","material":"cotton","season":"all","condition":"good","defects":"Small mark","notes":"Size unreadable","evidence":{"condition":"Front photo shows a mark."},"uncertainFields":["condition","size"],"needsTagPhoto":true}) } else { json!({"title":"Puuvillapaita","description":"Pieni jälki hihassa.","price":5,"priceReason":"Unverified estimate, not researched."}) }; Json(json!({"choices":[{"message":{"content":content.to_string()}}]})) }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let original = photo(&server, &cookie, "original");
    let config = json!({"enabled":true,"baseUrl":format!("http://{address}/v1"),"model":"fixture","imageModel":"","imageSize":"1024x1024","responseFormat":"schema","tokenParameter":"omit"});
    assert_eq!(
        request(
            &server,
            Method::PUT,
            "/api/ai/config",
            json!({"config":config,"apiKey":"fixture-secret"}),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::OK
    );
    let (code, result, _) = request(
        &server,
        Method::POST,
        "/api/ai/analyze",
        json!({"photos":[{"url":original,"kind":"front"}],"note":""}),
        Some(&cookie),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["needsTagPhoto"], true);
    assert_eq!(result["garment"]["size"], "");
    let (_, state, _) = request(
        &server,
        Method::GET,
        "/api/state",
        Value::Null,
        Some(&cookie),
    )
    .await;
    assert_eq!(state["state"]["garments"], json!([]));
    let mut item = garment();
    item["photos"] = json!([{"url":original,"kind":"front"}]);
    assert_eq!(
        operation(
            &server,
            &cookie,
            patch("garments", "garment-a", Value::Null, item)
        )
        .await
        .0,
        StatusCode::OK
    );
    let (code, result, _) = request(
        &server,
        Method::POST,
        "/api/ai/listing",
        json!({"garmentIds":["garment-a"],"platform":"tori"}),
        Some(&cookie),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{result}");
    assert_eq!(result["listing"]["status"], "draft");
    let calls = received.lock().unwrap();
    assert!(
        calls[0]["messages"][1]["content"][1]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert!(calls[0].get("omit").is_none());
    for field in ["category", "sizeSystem", "season", "condition"] {
        assert_eq!(
            calls[0]["response_format"]["json_schema"]["schema"]["properties"][field]["enum"],
            json!(match field {
                "category" => vec![
                    "tops",
                    "bottoms",
                    "one-pieces",
                    "knitwear",
                    "outerwear",
                    "sleepwear",
                    "shoes",
                    "accessories",
                    "other"
                ],
                "sizeSystem" => vec!["cm", "age", "eu-shoe", "alpha", "other"],
                "season" => vec!["all", "spring", "summer", "autumn", "winter"],
                _ => vec!["unknown", "new", "excellent", "good", "fair", "repair"],
            })
        );
    }
    assert!(!calls[1].to_string().contains("private family note"));
    task.abort();
}

#[tokio::test]
async fn clothing_companion_routes_photos_and_rejects_malformed_results_without_saving() {
    let (server, _dir) = server();
    let cookie = owner(&server).await;
    let original = photo(&server, &cookie, "original");
    let household: String = server
        .db
        .lock()
        .unwrap()
        .query_row("SELECT id FROM households", [], |r| r.get(0))
        .unwrap();
    let pairing = ai::provision_companion(&server, &household, "fixture-vision", "").unwrap();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "authorization",
        format!("Bearer {}", pairing["token"].as_str().unwrap())
            .parse()
            .unwrap(),
    );
    let input = json!({"photos":[{"url": original,"kind":"tag"}],"note":""});
    // An absent companion reports an error rather than applying a partial garment.
    assert_ne!(
        request(
            &server,
            Method::POST,
            "/api/ai/analyze",
            input.clone(),
            Some(&cookie)
        )
        .await
        .0,
        StatusCode::OK
    );
    let _ = companion::heartbeat(axum::extract::State(server.clone()), headers.clone())
        .await
        .unwrap();
    for mode in ["valid", "not-json", "empty-season"] {
        let app = server.clone();
        let auth = cookie.clone();
        let body = input.clone();
        let pending = tokio::spawn(async move {
            request(&app, Method::POST, "/api/ai/analyze", body, Some(&auth)).await
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
        assert_eq!(job["path"], "chat/completions");
        assert!(
            job["body"]["messages"][1]["content"][1]["image_url"]["url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        let text = if mode == "not-json" {
            "not JSON".to_owned()
        } else {
            json!({"name":"Cotton shirt","size":"98","category":"tops","sizeSystem":"cm","condition":"good","season":if mode == "empty-season" { "" } else { "all" },"evidence":{"size":"Printed tag"},"uncertainFields":["condition"],"needsTagPhoto":false}).to_string()
        };
        let _ = companion::complete(
            axum::extract::State(server.clone()),
            headers.clone(),
            Json(companion::Completion {
                id: job["id"].as_str().unwrap().into(),
                lease: job["lease"].as_str().unwrap().into(),
                failed: false,
                result: Some(json!({"choices":[{"message":{"content":text}}]})),
            }),
        )
        .await
        .unwrap();
        let (code, result, _) = pending.await.unwrap();
        assert_eq!(
            code,
            if mode != "valid" {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::OK
            },
            "{result}"
        );
        if mode == "empty-season" {
            assert!(
                result["message"]
                    .as_str()
                    .unwrap()
                    .contains("unsupported season"),
                "{result}"
            );
        }
        if mode == "valid" {
            assert_eq!(result["garment"]["size"], "98");
        }
    }
    assert_eq!(
        request(
            &server,
            Method::GET,
            "/api/state",
            Value::Null,
            Some(&cookie)
        )
        .await
        .1["state"]["garments"],
        json!([])
    );
}
