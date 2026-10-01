use crate::{
    Server,
    auth::authenticate,
    db::Database,
    domain,
    error::{ApiError, Result},
    model::*,
};
use axum::{
    Json,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
};
use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{convert::Infallible, time::Duration};
use uuid::Uuid;

pub async fn state(State(server): State<Server>, headers: HeaderMap) -> Result<Json<Snapshot>> {
    let actor = authenticate(&server, &headers)?;
    Ok(Json(Database::snapshot(
        &*server.db.lock()?,
        &actor.household_id,
    )?))
}

pub async fn mutate(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<Mutation>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    if Uuid::parse_str(&input.id).is_err() {
        return Err(ApiError::bad("Operations need a unique UUID."));
    }
    let fingerprint = crate::auth::digest(&serde_json::to_string(&input.operation)?);
    let result = {
        let mut conn = server.db.lock()?;
        let tx = conn.transaction()?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT fingerprint FROM operations WHERE household_id=?1 AND id=?2",
                params![actor.household_id, input.id],
                |row| row.get(0),
            )
            .optional()?;
        let mut snapshot = Database::snapshot(&tx, &actor.household_id)?;
        if let Some(previous) = previous {
            if previous != fingerprint {
                return Err(ApiError::bad(
                    "This operation ID was already used for a different change.",
                ));
            }
            return Ok(Json(
                json!({"revision":snapshot.revision,"state":snapshot.state,"eventId":input.id}),
            ));
        }
        let before = snapshot.state.clone();
        let detail = match &input.operation {
            Operation::ApplyProposal {
                proposal_id,
                shopping_indices,
                recipe_ids,
                days,
            } => {
                let stored:Option<(String,bool,i64)>=tx.query_row("SELECT proposal,applied,created_at FROM ai_proposals WHERE id=?1 AND household_id=?2",params![proposal_id,actor.household_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
                let (stored, applied, created) =
                    stored.ok_or_else(|| ApiError::bad("This proposal no longer exists."))?;
                if applied || created < Utc::now().timestamp() - 86400 {
                    return Err(ApiError::bad(
                        "This proposal has expired or was already applied. Ask for a fresh suggestion.",
                    ));
                }
                let proposal: crate::ai::AssistantProposal = serde_json::from_str(&stored)?;
                if snapshot.revision != proposal.base_revision {
                    return Err(ApiError::conflict());
                }
                let selected_shopping = shopping_indices
                    .iter()
                    .map(|index| {
                        proposal
                            .shopping
                            .get(*index)
                            .cloned()
                            .ok_or_else(|| ApiError::bad("Invalid shopping selection."))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let selected_meals = days
                    .iter()
                    .map(|day| {
                        proposal
                            .plan
                            .iter()
                            .find(|meal| meal.day == *day)
                            .ok_or_else(|| ApiError::bad("Invalid meal selection."))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let mut selected_recipe_ids = recipe_ids
                    .iter()
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>();
                for meal in &selected_meals {
                    if proposal
                        .recipes
                        .iter()
                        .any(|recipe| recipe.id == meal.recipe_id)
                    {
                        selected_recipe_ids.insert(meal.recipe_id.clone());
                    }
                }
                let selected_recipes = selected_recipe_ids
                    .iter()
                    .map(|id| {
                        proposal
                            .recipes
                            .iter()
                            .find(|recipe| recipe.id == *id)
                            .cloned()
                            .ok_or_else(|| ApiError::bad("Invalid recipe selection."))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let needed = selected_shopping
                    .iter()
                    .chain(
                        selected_recipes
                            .iter()
                            .flat_map(|recipe| recipe.ingredients.iter()),
                    )
                    .map(|ingredient| ingredient.product_id.clone())
                    .collect::<std::collections::BTreeSet<_>>();
                snapshot.state.custom_products.extend(
                    proposal
                        .products
                        .into_iter()
                        .filter(|product| needed.contains(&product.id)),
                );
                snapshot.state.recipe_book.extend(selected_recipes);
                for meal in selected_meals {
                    snapshot
                        .state
                        .plan
                        .insert(meal.day.clone(), meal.recipe_id.clone());
                    snapshot
                        .state
                        .servings
                        .insert(meal.day.clone(), meal.servings);
                    snapshot.state.meal_feedback.remove(&meal.day);
                }
                let today = domain::today(&snapshot.state);
                domain::apply(
                    &mut snapshot.state,
                    &Operation::AddIngredients {
                        ingredients: selected_shopping,
                        multiplier: 1.0,
                    },
                    &today,
                )?;
                tx.execute(
                    "UPDATE ai_proposals SET applied=1 WHERE id=?1 AND household_id=?2",
                    params![proposal_id, actor.household_id],
                )?;
                json!({"proposalId":proposal_id,"shopping":shopping_indices.len(),"recipes":selected_recipe_ids.len(),"days":days})
            }
            Operation::Undo { event_id } => {
                let found: Option<(String, bool)> = tx
                    .query_row(
                        "SELECT changes,undone FROM events WHERE id=?1 AND household_id=?2",
                        params![event_id, actor.household_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let (changes, undone) =
                    found.ok_or_else(|| ApiError::bad("This event no longer exists."))?;
                if undone {
                    return Err(ApiError::bad("This event has already been undone."));
                }
                let changes: Vec<Change> = serde_json::from_str(&changes)?;
                let reversed = changes
                    .into_iter()
                    .rev()
                    .map(|change| Change {
                        before: change.after,
                        after: change.before,
                        ..change
                    })
                    .collect::<Vec<_>>();
                domain::apply_history_changes(&mut snapshot.state, &reversed)?;
                tx.execute(
                    "UPDATE events SET undone=1 WHERE id=?1 AND household_id=?2",
                    params![event_id, actor.household_id],
                )?;
                json!({"eventId":event_id})
            }
            Operation::ResetHistory => {
                actor.owner()?;
                tx.execute(
                    "DELETE FROM events WHERE household_id=?1",
                    [&actor.household_id],
                )?;
                snapshot.state.purchases.clear();
                snapshot.state.suggestion_feedback.clear();
                snapshot.state.meal_feedback.clear();
                json!({"cleared":true})
            }
            operation => {
                let today = domain::today(&snapshot.state);
                domain::apply(&mut snapshot.state, operation, &today)?
            }
        };
        if !snapshot
            .state
            .stores
            .iter()
            .any(|store| store.id == snapshot.state.store_id)
        {
            snapshot.state.store_id = snapshot
                .state
                .stores
                .first()
                .ok_or_else(|| ApiError::bad("Keep at least one shop."))?
                .id
                .clone();
        }
        domain::validate(&snapshot.state)?;
        crate::icon_jobs::enqueue_new(&tx, &actor.household_id, &before, &snapshot.state)?;
        crate::clothing::invalidate_approvals(&before, &mut snapshot.state);
        crate::clothing::validate(&snapshot.state)?;
        crate::clothing::validate_images(&tx, &actor.household_id, &snapshot.state)?;
        let changes = domain::changes(&before, &snapshot.state)?;
        let kind = serde_json::to_value(&input.operation)?["type"]
            .as_str()
            .unwrap_or("change")
            .to_string();
        let mut detail = if let Operation::Patch { changes, .. } = &input.operation {
            json!({"changes":changes,"detail":detail})
        } else {
            detail
        };
        if let Some(store) = &input.store_id {
            if !before.stores.iter().any(|candidate| candidate.id == *store) {
                return Err(ApiError::bad("This shop no longer exists."));
            }
            detail["storeId"] = json!(store);
        }
        snapshot.revision += 1;
        Database::save(&tx, &actor.household_id, &snapshot)?;
        tx.execute("INSERT INTO events(id,household_id,actor,kind,at,detail,changes) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![input.id,actor.household_id,actor.user.name,kind,Utc::now().to_rfc3339(),serde_json::to_string(&detail)?,serde_json::to_string(&changes)?])?;
        tx.execute(
            "INSERT INTO operations(household_id,id,fingerprint,revision) VALUES(?1,?2,?3,?4)",
            params![actor.household_id, input.id, fingerprint, snapshot.revision],
        )?;
        tx.commit()?;
        json!({"revision":snapshot.revision,"state":snapshot.state,"eventId":input.id})
    };
    server.notify(&actor.household_id);
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct Page {
    pub before: Option<String>,
    pub limit: Option<u32>,
}
pub async fn history(
    State(server): State<Server>,
    headers: HeaderMap,
    Query(query): Query<Page>,
) -> Result<Json<Vec<HistoryEvent>>> {
    let actor = authenticate(&server, &headers)?;
    let conn = server.db.lock()?;
    let events=conn.prepare("SELECT id,actor,kind,at,detail,undone FROM events WHERE household_id=?1 AND at<?2 ORDER BY at DESC LIMIT ?3")?.query_map(params![actor.household_id,query.before.unwrap_or_else(||"9999".into()),query.limit.unwrap_or(100).min(500)],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,bool>(5)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
    Ok(Json(
        events
            .into_iter()
            .map(|(id, actor, kind, at, detail, undone)| HistoryEvent {
                id,
                actor,
                kind,
                at,
                detail: serde_json::from_str(&detail).unwrap_or(Value::Null),
                undone,
            })
            .collect(),
    ))
}

pub async fn stream(State(server): State<Server>, headers: HeaderMap) -> Result<Response> {
    let actor = authenticate(&server, &headers)?;
    let mut rx = server.notices.subscribe();
    let revision = Database::snapshot(&*server.db.lock()?, &actor.household_id)?.revision;
    let stream = async_stream::stream! {
        yield Ok::<Event,Infallible>(Event::default().event("sync").id(revision.to_string()).data(revision.to_string()));
        let mut heartbeat=tokio::time::interval(Duration::from_secs(20));
        loop {
            tokio::select! {
                notice=rx.recv()=>match notice {
                    Ok(household) if household==actor.household_id=>{
                        if authenticate(&server,&headers).is_err(){yield Ok(Event::default().event("signedOut").data(""));break;}
                        let revision=server.db.lock().ok().and_then(|conn|Database::snapshot(&conn,&actor.household_id).ok()).map(|snapshot|snapshot.revision);
                        if let Some(revision)=revision{yield Ok(Event::default().event("sync").id(revision.to_string()).data(revision.to_string()));}
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_))=>yield Ok(Event::default().event("sync").data("refresh")),
                    Err(tokio::sync::broadcast::error::RecvError::Closed)=>break,
                    _=>{},
                },
                _=heartbeat.tick()=>{if authenticate(&server,&headers).is_err(){yield Ok(Event::default().event("signedOut").data(""));break;}}
            }
        }
    };
    let mut response = Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    response
        .headers_mut()
        .insert("x-accel-buffering", "no".parse().expect("static header"));
    Ok(response)
}

pub async fn upload(
    State(server): State<Server>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    server.rate_limit(&format!("upload:{}", actor.household_id), 60, 3600)?;
    let field = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::bad("Choose an image file."))?
        .ok_or_else(|| ApiError::bad("Choose an image file."))?;
    let bytes = field
        .bytes()
        .await
        .map_err(|_| ApiError::bad("The image could not be read."))?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(ApiError::bad("Choose an image under 8 MB."));
    }
    let image = tokio::task::spawn_blocking(move || encode_image(&bytes))
        .await
        .map_err(ApiError::internal)??;
    let result = save_image(&server, &actor.household_id, image)?;
    let id = result.0["url"]
        .as_str()
        .unwrap()
        .trim_start_matches("/api/images/");
    server.db.lock()?.execute(
        "UPDATE uploads SET source_kind='original' WHERE id=?1 AND household_id=?2",
        params![id, actor.household_id],
    )?;
    Ok(result)
}
pub struct StoredImage {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}
pub fn encode_image(bytes: &[u8]) -> Result<StoredImage> {
    encode_image_at_size(bytes, 1600)
}
pub fn encode_image_at_size(bytes: &[u8], edge: u32) -> Result<StoredImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ApiError::bad("Use a JPG, PNG, or WebP image."))?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::WebP)
    ) {
        return Err(ApiError::bad("Use a JPG, PNG, or WebP image."));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    use image::ImageDecoder;
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| ApiError::bad("The image is damaged or too large."))?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut decoded = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| ApiError::bad("The image is damaged or too large."))?;
    decoded.apply_orientation(orientation);
    let image = decoded.thumbnail(edge, edge);
    let mut encoded = Vec::new();
    let transparent =
        image.color().has_alpha() && image.to_rgba8().pixels().any(|pixel| pixel[3] < 255);
    let mime = if transparent || edge == 256 {
        image::codecs::webp::WebPEncoder::new_lossless(&mut encoded)
            .encode(
                image.to_rgba8().as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(ApiError::internal)?;
        "image/webp"
    } else {
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 85)
            .encode_image(&image.to_rgb8())
            .map_err(ApiError::internal)?;
        "image/jpeg"
    };
    Ok(StoredImage {
        bytes: encoded,
        mime,
    })
}
pub fn save_image(server: &Server, household: &str, image: StoredImage) -> Result<Json<Value>> {
    let id = Uuid::new_v4().to_string();
    let conn = server.db.lock()?;
    let size: i64 = conn.query_row(
        "SELECT COALESCE(SUM(length(bytes)),0) FROM uploads WHERE household_id=?1",
        [household],
        |row| row.get(0),
    )?;
    if size + image.bytes.len() as i64 > 64 * 1024 * 1024 {
        return Err(ApiError::bad(
            "The household image limit is 64 MB. Remove unused images first.",
        ));
    }
    conn.execute(
        "INSERT INTO uploads(id,household_id,mime,bytes,created_at,source_kind) VALUES(?1,?2,?3,?4,?5,'generated')",
        params![
            id,
            household,
            image.mime,
            image.bytes,
            Utc::now().to_rfc3339()
        ],
    )?;
    Ok(Json(json!({"url":format!("/api/images/{id}")})))
}
pub async fn image(
    State(server): State<Server>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response> {
    let actor = authenticate(&server, &headers)?;
    let stored: Option<(String, Vec<u8>)> = server
        .db
        .lock()?
        .query_row(
            "SELECT mime,bytes FROM uploads WHERE id=?1 AND household_id=?2",
            params![id, actor.household_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (mime, bytes) =
        stored.ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Image not found.".into()))?;
    Ok((
        [
            (header::CONTENT_TYPE, mime.as_str()),
            (header::CACHE_CONTROL, "private, max-age=86400"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
}
pub async fn delete_image(
    State(server): State<Server>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    let conn = server.db.lock()?;
    let state = Database::snapshot(&conn, &actor.household_id)?.state;
    let url = format!("/api/images/{id}");
    if state
        .garments
        .iter()
        .any(|item| item.photos.iter().any(|p| p.url == url))
        || state
            .sale_listings
            .iter()
            .any(|item| item.photos.contains(&url))
        || state
            .items
            .iter()
            .any(|item| item.image.as_deref() == Some(&url))
        || state
            .stores
            .iter()
            .any(|store| store.image.as_deref() == Some(&url))
        || state
            .custom_products
            .iter()
            .any(|product| product.image.as_deref() == Some(&url))
        || state
            .recipe_book
            .iter()
            .chain(&state.custom_recipes)
            .any(|recipe| recipe.image == url)
        || state.product_images.values().any(|image| image == &url)
    {
        return Err(ApiError::bad(
            "Remove this image from its recipe or product first.",
        ));
    }
    conn.execute(
        "DELETE FROM uploads WHERE id=?1 AND household_id=?2",
        params![id, actor.household_id],
    )?;
    Ok(Json(json!({"ok":true})))
}

pub async fn backup(State(server): State<Server>, headers: HeaderMap) -> Result<Response> {
    let actor = authenticate(&server, &headers)?;
    actor.owner()?;
    let bytes = {
        let conn = server.db.lock()?;
        let snapshot = Database::snapshot(&conn, &actor.household_id)?;
        let uploads = conn
            .prepare(
                "SELECT id,mime,bytes,created_at,source_kind FROM uploads WHERE household_id=?1",
            )?
            .query_map([&actor.household_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        use base64::Engine;
        let events=conn.prepare("SELECT id,actor,kind,at,detail,changes,undone FROM events WHERE household_id=?1")?.query_map([&actor.household_id],|row|Ok(json!({"id":row.get::<_,String>(0)?,"actor":row.get::<_,String>(1)?,"kind":row.get::<_,String>(2)?,"at":row.get::<_,String>(3)?,"detail":row.get::<_,String>(4)?,"changes":row.get::<_,String>(5)?,"undone":row.get::<_,bool>(6)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
        serde_json::to_vec(
            &json!({"format":"littlewardrobe-household-v1","state":snapshot.state,"events":events,"images":uploads.into_iter().map(|(id,mime,bytes,at,source)|json!({"id":id,"mime":mime,"sourceKind":source,"data":base64::engine::general_purpose::STANDARD.encode(bytes),"at":at})).collect::<Vec<_>>()}),
        )?
    };
    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"littlewardrobe-backup.json\"",
            ),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreRequest {
    pub backup: Value,
    pub expected_revision: u32,
}
fn remap_images(value: &mut Value, images: &std::collections::BTreeMap<String, String>) {
    match value {
        Value::String(text) => {
            if let Some(id) = text.strip_prefix("/api/images/")
                && let Some(replacement) = images.get(id)
            {
                *text = format!("/api/images/{replacement}")
            }
        }
        Value::Array(values) => {
            for value in values {
                remap_images(value, images)
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                remap_images(value, images)
            }
        }
        _ => {}
    }
}
pub async fn restore(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(mut input): Json<RestoreRequest>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    actor.owner()?;
    server.rate_limit(&format!("restore:{}", actor.household_id), 4, 3600)?;
    if input.backup["format"] != "littlewardrobe-household-v1" {
        return Err(ApiError::bad("Choose a Little Wardrobe household backup."));
    }
    let images = input.backup["images"]
        .as_array()
        .ok_or_else(|| ApiError::bad("The backup has no image collection."))?;
    if images.len() > 2000 {
        return Err(ApiError::bad("Too many images in this backup."));
    }
    use base64::Engine;
    let mut remap = std::collections::BTreeMap::new();
    let mut uploads = Vec::new();
    let mut total = 0;
    for image in images {
        let id = image["id"]
            .as_str()
            .filter(|id| domain::valid_id(id))
            .ok_or_else(|| ApiError::bad("Invalid backup image ID."))?;
        if remap.contains_key(id) {
            return Err(ApiError::bad("Duplicate backup image ID."));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(
                image["data"]
                    .as_str()
                    .ok_or_else(|| ApiError::bad("Invalid backup image."))?,
            )
            .map_err(|_| ApiError::bad("Invalid backup image encoding."))?;
        total += bytes.len();
        if total > 64 * 1024 * 1024 {
            return Err(ApiError::bad(
                "Backup images exceed 64 MB. Restore a full database backup instead.",
            ));
        }
        let id_new = Uuid::new_v4().to_string();
        remap.insert(id.to_string(), id_new.clone());
        let source = image["sourceKind"]
            .as_str()
            .unwrap_or("original")
            .to_string();
        if !["original", "enhanced", "generated"].contains(&source.as_str()) {
            return Err(ApiError::bad("Invalid image provenance."));
        }
        uploads.push((id_new, bytes, source));
    }
    let uploads = tokio::task::spawn_blocking(move || {
        uploads
            .into_iter()
            .map(|(id, bytes, source)| encode_image(&bytes).map(|bytes| (id, bytes, source)))
            .collect::<Result<Vec<_>>>()
    })
    .await
    .map_err(ApiError::internal)??;
    remap_images(&mut input.backup["state"], &remap);
    let state: HouseholdState = serde_json::from_value(input.backup["state"].clone())?;
    domain::validate(&state)?;
    let events = input.backup["events"]
        .as_array()
        .ok_or_else(|| ApiError::bad("Invalid backup history."))?;
    if events.len() > 100000 {
        return Err(ApiError::bad(
            "Backup history is too large. Restore a full database backup instead.",
        ));
    }
    let mut imported = vec![];
    for event in events {
        let at = event["at"]
            .as_str()
            .filter(|at| chrono::DateTime::parse_from_rfc3339(at).is_ok())
            .ok_or_else(|| ApiError::bad("Invalid history date."))?;
        let actor = event["actor"]
            .as_str()
            .filter(|s| s.len() <= 320)
            .ok_or_else(|| ApiError::bad("Invalid history member."))?;
        let kind = event["kind"]
            .as_str()
            .filter(|s| s.len() <= 80)
            .ok_or_else(|| ApiError::bad("Invalid history event."))?;
        let mut detail: Value = serde_json::from_str(
            event["detail"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Invalid history details."))?,
        )?;
        let mut changes: Value = serde_json::from_str(
            event["changes"]
                .as_str()
                .ok_or_else(|| ApiError::bad("Invalid history changes."))?,
        )?;
        remap_images(&mut detail, &remap);
        remap_images(&mut changes, &remap);
        let _: Vec<Change> = serde_json::from_value(changes.clone())?;
        imported.push((
            Uuid::new_v4().to_string(),
            actor.to_string(),
            kind.to_string(),
            at.to_string(),
            detail,
            changes,
            event["undone"].as_bool().unwrap_or(false),
        ));
    }
    {
        let mut conn = server.db.lock()?;
        let tx = conn.transaction()?;
        let previous = Database::snapshot(&tx, &actor.household_id)?;
        if previous.revision != input.expected_revision {
            return Err(ApiError::conflict());
        }
        for table in ["events", "uploads", "ai_proposals", "companion_jobs"] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE household_id=?1"),
                [&actor.household_id],
            )?;
        }
        for (id, image, source) in uploads {
            tx.execute(
                "INSERT INTO uploads(id,household_id,mime,bytes,created_at,source_kind) VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    id,
                    actor.household_id,
                    image.mime,
                    image.bytes,
                    Utc::now().to_rfc3339(), source
                ],
            )?;
        }
        for (id, member, kind, at, detail, changes, undone) in imported {
            tx.execute("INSERT INTO events(id,household_id,actor,kind,at,detail,changes,undone) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,actor.household_id,member,kind,at,detail.to_string(),changes.to_string(),undone])?;
        }
        crate::icon_jobs::cancel_all(&tx, &actor.household_id)?;
        // Reject references to images outside the restored household.
        let text = serde_json::to_string(&state)?;
        for part in text.split("/api/images/").skip(1) {
            let id = part.split('"').next().unwrap_or("");
            if !remap.values().any(|value| value == id) {
                return Err(ApiError::bad(
                    "The backup references a missing private image.",
                ));
            }
        }
        crate::clothing::validate_images(&tx, &actor.household_id, &state)?;
        Database::save(
            &tx,
            &actor.household_id,
            &Snapshot {
                revision: previous.revision + 1,
                state,
            },
        )?;
        tx.commit()?;
    }
    server.notify(&actor.household_id);
    Ok(Json(json!({"ok":true})))
}

pub async fn cleanup_images(
    State(server): State<Server>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    actor.owner()?;
    let conn = server.db.lock()?;
    let state = serde_json::to_string(&Database::snapshot(&conn, &actor.household_id)?.state)?;
    let images = conn
        .prepare("SELECT id FROM uploads WHERE household_id=?1 AND created_at<?2")?
        .query_map(
            params![
                actor.household_id,
                (Utc::now() - chrono::Duration::days(1)).to_rfc3339()
            ],
            |row| row.get::<_, String>(0),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut removed = 0;
    for id in images {
        let url = format!("/api/images/{id}");
        let used:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE household_id=?1 AND undone=0 AND changes LIKE ?2)",params![actor.household_id,format!("%{url}%")],|row|row.get(0))?;
        if !state.contains(&url) && !used {
            removed += conn.execute(
                "DELETE FROM uploads WHERE id=?1 AND household_id=?2",
                params![id, actor.household_id],
            )?;
        }
    }
    Ok(Json(json!({"removed":removed})))
}
