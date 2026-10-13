//! Durable original-photo intake. Provider calls run outside the database lock;
//! completion merges only unchanged descriptive fields and never approves facts.
use crate::{
    Server, ai,
    auth::authenticate,
    clothing::{self, AnalyzeInput, Garment, GarmentPhoto},
    db::Database,
    domain,
    error::{ApiError, Result},
    model::Snapshot,
};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct PhotoJobView {
    pub id: String,
    pub garment_id: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone, Default, Deserialize, Serialize, TS)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct PhotoDefaults {
    pub child_id: Option<String>,
    pub location_id: Option<String>,
    pub status: Option<String>,
    pub category: Option<String>,
    pub size: Option<String>,
    pub size_system: Option<String>,
    pub season: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Intake {
    pub id: String,
    pub photos: Vec<GarmentPhoto>,
    #[serde(default)]
    pub defaults: PhotoDefaults,
}

fn history(
    conn: &Connection,
    household: &str,
    actor: &str,
    before: &crate::model::HouseholdState,
    snapshot: &mut Snapshot,
    label: &str,
) -> Result<()> {
    clothing::invalidate_approvals(before, &mut snapshot.state);
    domain::validate(&snapshot.state)?;
    clothing::validate(&snapshot.state)?;
    clothing::validate_images(conn, household, &snapshot.state)?;
    let changes = domain::changes(before, &snapshot.state)?;
    snapshot.revision += 1;
    Database::save(conn, household, snapshot)?;
    conn.execute("INSERT INTO events(id,household_id,actor,kind,at,detail,changes) VALUES(?1,?2,?3,'photoIntake',?4,?5,?6)", params![Uuid::new_v4().to_string(), household, actor, chrono::Utc::now().to_rfc3339(), json!({"label":label}).to_string(), serde_json::to_string(&changes)?])?;
    Ok(())
}

pub async fn create(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<Intake>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    if Uuid::parse_str(&input.id).is_err()
        || input.photos.is_empty()
        || input.photos.len() > 4
        || input
            .photos
            .iter()
            .any(|p| !["front", "back", "tag", "detail"].contains(&p.kind.as_str()))
    {
        return Err(ApiError::bad(
            "Choose one to four original clothing photos.",
        ));
    }
    let (config, _) = ai::provider(&server, &actor.household_id)?;
    if !config.enabled {
        return Err(ApiError::bad("Enable the AI connection in Settings first."));
    }
    server.rate_limit(&format!("photo-intake:{}", actor.household_id), 60, 3600)?;
    let result = {
        let mut conn = server.db.lock()?;
        let tx = conn.transaction()?;
        let accepted = json!({"photos":input.photos,"defaults":input.defaults});
        let previous: Option<(String, String, String)> = tx
            .query_row(
                "SELECT household_id,baseline,request FROM clothing_photo_jobs WHERE id=?1",
                [&input.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((household, baseline, request)) = previous {
            let baseline: Garment = serde_json::from_str(&baseline)?;
            let request: Value = serde_json::from_str(&request)?;
            let expected = if request == json!({}) {
                json!({"photos":baseline.photos,"defaults":PhotoDefaults::default()})
            } else {
                request
            };
            if household != actor.household_id || expected != accepted {
                return Err(ApiError::conflict());
            }
            return Ok(Json(json!({"id":input.id,"garmentId":baseline.id})));
        }
        let outstanding: i64 = tx.query_row("SELECT count(*) FROM clothing_photo_jobs WHERE household_id=?1 AND status IN ('queued','running')", [&actor.household_id], |r| r.get(0))?;
        if outstanding >= 30 {
            return Err(ApiError::bad(
                "Let the queued photos finish before adding more.",
            ));
        }
        let garment = Garment {
            id: Uuid::new_v4().to_string(),
            name: "Photo awaiting details".into(),
            category: input.defaults.category.unwrap_or_else(|| "other".into()),
            size: input.defaults.size.unwrap_or_default(),
            size_system: input.defaults.size_system.unwrap_or_else(|| "other".into()),
            season: input.defaults.season.unwrap_or_else(|| "all".into()),
            condition: "unknown".into(),
            status: input.defaults.status.unwrap_or_else(|| "stored".into()),
            child_id: input.defaults.child_id.unwrap_or_default(),
            location_id: input.defaults.location_id.unwrap_or_default(),
            photos: input.photos,
            created_at: chrono::Utc::now().to_rfc3339(),
            ai_review: true,
            ..Default::default()
        };
        let mut snapshot = Database::snapshot(&tx, &actor.household_id)?;
        let before = snapshot.state.clone();
        snapshot.state.garments.push(garment.clone());
        history(
            &tx,
            &actor.household_id,
            &actor.user.name,
            &before,
            &mut snapshot,
            "Added photo for AI review",
        )?;
        tx.execute("INSERT INTO clothing_photo_jobs(id,household_id,garment_id,actor,baseline,request) VALUES(?1,?2,?3,?4,?5,?6)",params![input.id,actor.household_id,garment.id,actor.user.name,serde_json::to_string(&garment)?,accepted.to_string()])?;
        tx.commit()?;
        json!({"id":input.id,"garmentId":garment.id})
    };
    server.notify(&actor.household_id);
    Ok(Json(result))
}

pub async fn status(
    State(server): State<Server>,
    headers: HeaderMap,
) -> Result<Json<Vec<PhotoJobView>>> {
    let actor = authenticate(&server, &headers)?;
    let conn = server.db.lock()?;
    let mut statement = conn.prepare("SELECT id,garment_id,status,error FROM clothing_photo_jobs WHERE household_id=?1 AND status IN ('queued','running','failed') ORDER BY created_at DESC LIMIT 60")?;
    let jobs = statement
        .query_map([&actor.household_id], |r| {
            Ok(PhotoJobView {
                id: r.get(0)?,
                garment_id: r.get(1)?,
                status: r.get(2)?,
                error: r.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(Json(jobs))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobAction {
    pub action: String,
}
pub async fn manage(
    State(server): State<Server>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<JobAction>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    if !["retry", "cancel"].contains(&input.action.as_str()) {
        return Err(ApiError::bad("Choose retry or cancel."));
    }
    let mut conn = server.db.lock()?;
    let tx = conn.transaction()?;
    let (garment_id, status): (String, String) = tx
        .query_row(
            "SELECT garment_id,status FROM clothing_photo_jobs WHERE id=?1 AND household_id=?2",
            params![id, actor.household_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(ApiError::unauthorized)?;
    if input.action == "retry" {
        if status != "failed" {
            return Err(ApiError::conflict());
        }
        let (config, _) = {
            drop(tx);
            drop(conn);
            ai::provider(&server, &actor.household_id)?
        };
        if !config.enabled {
            return Err(ApiError::bad("Enable the AI connection first."));
        }
        server.rate_limit(&format!("photo-retry:{}", actor.household_id), 60, 3600)?;
        let mut conn = server.db.lock()?;
        let tx = conn.transaction()?;
        let state = Database::snapshot(&tx, &actor.household_id)?.state;
        let g = state
            .garments
            .iter()
            .find(|g| g.id == garment_id && g.ai_review)
            .ok_or_else(|| ApiError::bad("This item was removed or already reviewed."))?;
        let count = g.photos.iter().filter(|p| p.kind != "enhanced").count();
        if count == 0 || count > 4 {
            return Err(ApiError::bad(
                "Use the editor to suggest details from these photos.",
            ));
        }
        let changed = tx.execute("UPDATE clothing_photo_jobs SET status='queued',run_id=NULL,error=NULL,baseline=?3 WHERE id=?1 AND household_id=?2 AND status='failed'",params![id,actor.household_id,serde_json::to_string(g)?])?;
        if changed == 0 {
            return Err(ApiError::conflict());
        }
        tx.commit()?;
    } else {
        tx.execute("UPDATE clothing_photo_jobs SET status='canceled',run_id=NULL,error=NULL WHERE id=?1 AND household_id=?2 AND status IN ('queued','running','failed')",params![id,actor.household_id])?;
        tx.commit()?;
    }
    server.notify(&actor.household_id);
    Ok(Json(json!({"ok":true})))
}

pub struct Job {
    id: String,
    household: String,
    actor: String,
    baseline: Garment,
    run_id: String,
    defaults: PhotoDefaults,
}

/// Manual review, deleted entries and replaced photos invalidate queued work
/// in the same transaction as the household change.
pub fn guard_changes(
    conn: &Connection,
    household: &str,
    state: &crate::model::HouseholdState,
) -> Result<()> {
    let mut statement = conn.prepare("SELECT id,baseline FROM clothing_photo_jobs WHERE household_id=?1 AND status IN ('queued','running','failed')")?;
    let jobs = statement
        .query_map([household], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, baseline) in jobs {
        let baseline: Garment = serde_json::from_str(&baseline)?;
        if !state.garments.iter().any(|g| {
            g.id == baseline.id
                && g.ai_review
                && g.photos == baseline.photos
                && g.notes == baseline.notes
        }) {
            conn.execute("UPDATE clothing_photo_jobs SET status='canceled',run_id=NULL,error=NULL WHERE id=?1",[id])?;
        }
    }
    Ok(())
}
fn claim(server: &Server) -> Result<Option<Job>> {
    let mut conn = server.db.lock()?;
    let tx = conn.transaction()?;
    let next: Option<(String,String,String,String,String)> = tx.query_row("SELECT id,household_id,actor,baseline,request FROM clothing_photo_jobs WHERE status='queued' ORDER BY created_at,id LIMIT 1", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
    let Some((id, household, actor, baseline, request)) = next else {
        return Ok(None);
    };
    let run_id = Uuid::new_v4().to_string();
    tx.execute(
        "UPDATE clothing_photo_jobs SET status='running',run_id=?2 WHERE id=?1",
        params![id, run_id],
    )?;
    tx.commit()?;
    Ok(Some(Job {
        id,
        household,
        actor,
        baseline: serde_json::from_str(&baseline)?,
        defaults: serde_json::from_value(
            serde_json::from_str::<Value>(&request)?["defaults"]
                .as_object()
                .map(|v| json!(v))
                .unwrap_or_else(|| json!({})),
        )?,
        run_id,
    }))
}

fn finish(server: &Server, job: &Job, result: Result<Value>) -> Result<()> {
    let mut conn = server.db.lock()?;
    let tx = conn.transaction()?;
    let current: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM clothing_photo_jobs WHERE id=?1 AND run_id=?2 AND status='running')",params![job.id,job.run_id],|r|r.get(0))?;
    if !current {
        return Ok(());
    }
    let mut snapshot = Database::snapshot(&tx, &job.household)?;
    let before = snapshot.state.clone();
    let g = snapshot
        .state
        .garments
        .iter_mut()
        .find(|g| g.id == job.baseline.id);
    if g.as_ref().is_none_or(|g| {
        !g.ai_review || g.photos != job.baseline.photos || g.notes != job.baseline.notes
    }) {
        tx.execute(
            "UPDATE clothing_photo_jobs SET status='canceled',run_id=NULL WHERE id=?1",
            [&job.id],
        )?;
    } else if let Ok(result) = result {
        let g = g.unwrap();
        let mut current = serde_json::to_value(&*g)?;
        let baseline = serde_json::to_value(&job.baseline)?;
        let mut applied = std::collections::BTreeSet::new();
        for field in [
            "name",
            "category",
            "brand",
            "size",
            "sizeSystem",
            "color",
            "material",
            "season",
            "condition",
            "defects",
            "notes",
        ] {
            let selected = match field {
                "category" => job.defaults.category.is_some(),
                "size" => job.defaults.size.is_some(),
                "sizeSystem" => job.defaults.size_system.is_some(),
                "season" => job.defaults.season.is_some(),
                _ => false,
            };
            if !selected && current[field] == baseline[field] {
                current[field] = result["garment"][field].clone();
                applied.insert(field);
            }
        }
        *g = serde_json::from_value(current)?;
        g.ai_evidence = serde_json::from_value(result["evidence"].clone())?;
        g.ai_evidence
            .retain(|field, _| applied.contains(field.as_str()));
        g.ai_uncertain_fields = serde_json::from_value(result["uncertainFields"].clone())?;
        g.ai_uncertain_fields
            .retain(|field| applied.contains(field.as_str()));
        g.ai_needs_tag_photo = g.size.is_empty()
            || (job.defaults.size.is_none() && result["needsTagPhoto"].as_bool().unwrap_or(false));
        history(
            &tx,
            &job.household,
            &job.actor,
            &before,
            &mut snapshot,
            "AI filled clothing details; review required",
        )?;
        tx.execute(
            "UPDATE clothing_photo_jobs SET status='done',run_id=NULL,error=NULL WHERE id=?1",
            [&job.id],
        )?;
    } else {
        tx.execute(
            "UPDATE clothing_photo_jobs SET status='failed',run_id=NULL,error=?2 WHERE id=?1",
            params![job.id, result.unwrap_err().1],
        )?;
    }
    tx.commit()?;
    server.notify(&job.household);
    Ok(())
}

pub async fn run_one(server: &Server) -> Result<bool> {
    let Some(job) = claim(server)? else {
        return Ok(false);
    };
    server.notify(&job.household);
    let input = AnalyzeInput {
        photos: job
            .baseline
            .photos
            .iter()
            .filter(|p| p.kind != "enhanced")
            .cloned()
            .collect(),
        note: job.baseline.notes.clone(),
    };
    let app = server.clone();
    let household = job.household.clone();
    // A provider or conversion panic fails just this job, leaving the queue alive.
    let result =
        tokio::spawn(async move { clothing::analyze_photos(&app, &household, input).await })
            .await
            .unwrap_or_else(|_| {
                Err(ApiError::bad(
                    "The photo task stopped unexpectedly. Retry or enter details manually.",
                ))
            });
    if let Err(error) = finish(server, &job, result) {
        let conn = server.db.lock()?;
        conn.execute("UPDATE clothing_photo_jobs SET status='failed',run_id=NULL,error='The clothing details could not be saved. Retry or enter them manually.' WHERE id=?1 AND run_id=?2",params![job.id,job.run_id])?;
        server.notify(&job.household);
        return Err(error);
    }
    Ok(true)
}

pub fn resume_interrupted(server: &Server) -> Result<()> {
    server.db.lock()?.execute(
        "UPDATE clothing_photo_jobs SET status='queued',run_id=NULL WHERE status='running'",
        [],
    )?;
    Ok(())
}

pub fn start(server: Server) -> Result<()> {
    resume_interrupted(&server)?;
    tokio::spawn(async move {
        loop {
            let app = server.clone();
            match tokio::spawn(async move { run_one(&app).await }).await {
                Ok(Err(error)) => tracing::error!("Clothing photo queue: {}", error.1),
                Err(error) => {
                    tracing::error!("Clothing photo queue panicked: {error}");
                    if let Ok(conn) = server.db.lock() {
                        let _ = conn.execute("UPDATE clothing_photo_jobs SET status='failed',run_id=NULL,error='The photo task stopped unexpectedly. Try again.' WHERE status='running'",[]);
                    }
                }
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
    Ok(())
}
