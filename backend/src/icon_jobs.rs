use crate::{Server, ai, api, auth::authenticate, db::Database, domain, error::Result, model::*};
use axum::{Json, extract::State, http::HeaderMap};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;
use uuid::Uuid;

pub fn missing_icon(product: &Product) -> bool {
    product
        .image
        .as_ref()
        .is_none_or(|image| image.trim().is_empty())
        && matches!(product.icon.trim(), "" | "🛒")
}

fn config_for(conn: &Connection, household: &str) -> Result<Option<ai::ProviderConfig>> {
    let config: Option<String> = conn
        .query_row(
            "SELECT config FROM providers WHERE household_id=?1",
            [household],
            |row| row.get(0),
        )
        .optional()?;
    config
        .map(|config| serde_json::from_str(&config).map_err(Into::into))
        .transpose()
}

fn enqueue(conn: &Connection, household: &str, id: &str, source: &str) -> Result<()> {
    // An explicit bulk request adopts already queued automatic work without
    // duplicating it. Turning automatic generation off must keep that batch.
    if source == "manual" {
        conn.execute("UPDATE item_icon_jobs SET source='manual' WHERE household_id=?1 AND product_id=?2 AND status IN ('queued','running')", params![household,id])?;
    }
    conn.execute("INSERT INTO item_icon_jobs(household_id,product_id,source) VALUES(?1,?2,?3) ON CONFLICT(household_id,product_id) DO UPDATE SET source=excluded.source,status='queued',run_id=NULL,error=NULL,created_at=unixepoch() WHERE item_icon_jobs.status NOT IN ('queued','running')", params![household,id,source])?;
    Ok(())
}

pub fn enqueue_new(
    conn: &Connection,
    household: &str,
    before: &HouseholdState,
    after: &HouseholdState,
) -> Result<()> {
    if !config_for(conn, household)?.is_some_and(|config| {
        config.enabled && config.auto_item_icons && !config.image_model.is_empty()
    }) {
        return Ok(());
    }
    let previous_ids: std::collections::BTreeSet<_> = domain::products(before, true)
        .into_iter()
        .map(|product| product.id)
        .collect();
    for product in domain::products(after, false) {
        if missing_icon(&product) && !previous_ids.contains(&product.id) {
            enqueue(conn, household, &product.id, "automatic")?;
        }
    }
    Ok(())
}

pub fn cancel_all(conn: &Connection, household: &str) -> Result<()> {
    conn.execute("UPDATE item_icon_jobs SET status='canceled',run_id=NULL,error=NULL WHERE household_id=?1 AND status IN ('queued','running')", [household])?;
    Ok(())
}

pub fn cancel_automatic_if_disabled(
    server: &Server,
    household: &str,
    config: &ai::ProviderConfig,
) -> Result<()> {
    if !config.auto_item_icons || !config.enabled || config.image_model.is_empty() {
        server.db.lock()?.execute("UPDATE item_icon_jobs SET status='canceled',run_id=NULL,error=NULL WHERE household_id=?1 AND source='automatic' AND status IN ('queued','running')", [household])?;
    }
    Ok(())
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct IconJobFailure {
    pub product_id: String,
    pub name: String,
    pub error: String,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/api/")]
pub struct IconJobStatus {
    pub missing: usize,
    pub queued: u32,
    pub running: u32,
    pub completed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub ready: bool,
    pub failures: Vec<IconJobFailure>,
}

pub async fn status(
    State(server): State<Server>,
    headers: HeaderMap,
) -> Result<Json<IconJobStatus>> {
    let actor = authenticate(&server, &headers)?;
    actor.owner()?;
    let conn = server.db.lock()?;
    let products = domain::products(
        &Database::snapshot(&conn, &actor.household_id)?.state,
        false,
    );
    let mut result = IconJobStatus {
        missing: products
            .iter()
            .filter(|product| missing_icon(product))
            .count(),
        queued: 0,
        running: 0,
        completed: 0,
        failed: 0,
        skipped: 0,
        ready: config_for(&conn, &actor.household_id)?
            .is_some_and(|config| config.enabled && !config.image_model.is_empty()),
        failures: vec![],
    };
    let mut statement = conn.prepare(
        "SELECT status,count(*) FROM item_icon_jobs WHERE household_id=?1 GROUP BY status",
    )?;
    for row in statement.query_map([&actor.household_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
    })? {
        let (status, count) = row?;
        match status.as_str() {
            "queued" => result.queued = count,
            "running" => result.running = count,
            "completed" => result.completed = count,
            "failed" => result.failed = count,
            "skipped" => result.skipped = count,
            _ => {}
        }
    }
    let mut statement=conn.prepare("SELECT product_id,error FROM item_icon_jobs WHERE household_id=?1 AND status='failed' ORDER BY created_at DESC LIMIT 10")?;
    for row in statement.query_map([&actor.household_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (id, error) = row?;
        result.failures.push(IconJobFailure {
            name: products
                .iter()
                .find(|product| product.id == id)
                .map(|product| product.name.clone())
                .unwrap_or_else(|| "Removed item".into()),
            product_id: id,
            error,
        });
    }
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobAction {
    pub action: String,
}

pub async fn manage(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<JobAction>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    actor.owner()?;
    let mut conn = server.db.lock()?;
    let tx = conn.transaction()?;
    match input.action.as_str() {
        "generate" | "retry" => {
            if !config_for(&tx, &actor.household_id)?
                .is_some_and(|config| config.enabled && !config.image_model.is_empty())
            {
                return Err(crate::error::ApiError::bad(
                    "Enable the assistant and configure an image model first.",
                ));
            }
            let products =
                domain::products(&Database::snapshot(&tx, &actor.household_id)?.state, false);
            let missing: std::collections::BTreeSet<_> = products
                .iter()
                .filter(|product| missing_icon(product))
                .map(|product| product.id.as_str())
                .collect();
            let failed: Vec<String> = tx.prepare("SELECT product_id FROM item_icon_jobs WHERE household_id=?1 AND status='failed'")?.query_map([&actor.household_id],|row|row.get(0))?.collect::<std::result::Result<_,_>>()?;
            for id in failed {
                if !missing.contains(id.as_str()) {
                    tx.execute("UPDATE item_icon_jobs SET status='skipped',error=NULL WHERE household_id=?1 AND product_id=?2 AND status='failed'",params![actor.household_id,id])?;
                }
            }
            for product in products.iter().filter(|product| missing_icon(product)) {
                if input.action=="generate" || tx.query_row("SELECT EXISTS(SELECT 1 FROM item_icon_jobs WHERE household_id=?1 AND product_id=?2 AND status='failed')",params![actor.household_id,product.id],|row|row.get::<_,bool>(0))? {
                    enqueue(&tx,&actor.household_id,&product.id,"manual")?;
                }
            }
        }
        "cancel" => cancel_all(&tx, &actor.household_id)?,
        _ => {
            return Err(crate::error::ApiError::bad(
                "Choose generate, retry or cancel.",
            ));
        }
    }
    tx.commit()?;
    server.notify(&actor.household_id);
    Ok(Json(json!({"ok":true})))
}

struct Job {
    household: String,
    product: Product,
    run_id: String,
}

fn claim(server: &Server) -> Result<Option<Job>> {
    let mut conn = server.db.lock()?;
    let tx = conn.transaction()?;
    let candidates: Vec<(String,String,String)> = tx.prepare("SELECT household_id,product_id,source FROM item_icon_jobs WHERE status='queued' ORDER BY created_at,product_id")?.query_map([],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?.collect::<std::result::Result<_,_>>()?;
    for (household, id, source) in candidates {
        let Some(config) = config_for(&tx, &household)? else {
            continue;
        };
        if !config.enabled || config.image_model.is_empty() {
            continue;
        }
        if source == "automatic" && !config.auto_item_icons {
            tx.execute("UPDATE item_icon_jobs SET status='canceled' WHERE household_id=?1 AND product_id=?2",params![household,id])?;
            continue;
        }
        let state = Database::snapshot(&tx, &household)?.state;
        let product = domain::products(&state, false)
            .into_iter()
            .find(|product| product.id == id && missing_icon(product));
        let Some(product) = product else {
            tx.execute("UPDATE item_icon_jobs SET status='skipped' WHERE household_id=?1 AND product_id=?2",params![household,id])?;
            continue;
        };
        // Persisted pacing prevents a bulk click from flooding the provider.
        let recent:u32=tx.query_row("SELECT count(*) FROM item_icon_attempts WHERE household_id=?1 AND started_at>unixepoch()-3600",[&household],|row|row.get(0))?;
        if recent >= 20 {
            continue;
        }
        let run_id = Uuid::new_v4().to_string();
        tx.execute(
            "DELETE FROM item_icon_attempts WHERE started_at<unixepoch()-86400",
            [],
        )?;
        tx.execute(
            "INSERT INTO item_icon_attempts(id,household_id) VALUES(?1,?2)",
            params![run_id, household],
        )?;
        tx.execute("UPDATE item_icon_jobs SET status='running',run_id=?3,started_at=unixepoch(),error=NULL WHERE household_id=?1 AND product_id=?2",params![household,id,run_id])?;
        tx.commit()?;
        server.notify(&household);
        return Ok(Some(Job {
            household,
            product,
            run_id,
        }));
    }
    tx.commit()?;
    Ok(None)
}

fn finish(server: &Server, job: &Job, image: Result<api::StoredImage>) -> Result<()> {
    let mut conn = server.db.lock()?;
    let tx = conn.transaction()?;
    let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM item_icon_jobs WHERE household_id=?1 AND product_id=?2 AND run_id=?3 AND status='running')",params![job.household,job.product.id,job.run_id],|row|row.get(0))?;
    if !active {
        return Ok(());
    }
    let mut snapshot = Database::snapshot(&tx, &job.household)?;
    let current = domain::products(&snapshot.state, false)
        .into_iter()
        .find(|product| product.id == job.product.id && missing_icon(product));
    let (status, error) = match current {
        None => ("skipped", None),
        Some(product)
            if product.name != job.product.name || product.category != job.product.category =>
        {
            (
                "failed",
                Some(
                    "The item changed during generation. Retry to use its current name and group."
                        .into(),
                ),
            )
        }
        Some(_) => match image {
            Err(error) => ("failed", Some(error.1)),
            Ok(image) => {
                let size: i64 = tx.query_row(
                    "SELECT COALESCE(SUM(length(bytes)),0) FROM uploads WHERE household_id=?1",
                    [&job.household],
                    |row| row.get(0),
                )?;
                if size + image.bytes.len() as i64 > 64 * 1024 * 1024 {
                    (
                        "failed",
                        Some(
                            "The household image limit is 64 MB. Remove unused images first."
                                .into(),
                        ),
                    )
                } else {
                    let before = snapshot.state.clone();
                    let id = Uuid::new_v4().to_string();
                    tx.execute("INSERT INTO uploads(id,household_id,mime,bytes,created_at) VALUES(?1,?2,?3,?4,?5)",params![id,job.household,image.mime,image.bytes,chrono::Utc::now().to_rfc3339()])?;
                    snapshot
                        .state
                        .product_images
                        .insert(job.product.id.clone(), format!("/api/images/{id}"));
                    domain::validate(&snapshot.state)?;
                    let changes = domain::changes(&before, &snapshot.state)?;
                    snapshot.revision += 1;
                    Database::save(&tx, &job.household, &snapshot)?;
                    tx.execute("INSERT INTO events(id,household_id,actor,kind,at,detail,changes) VALUES(?1,?2,'AI assistant','generateItemIcon',?3,?4,?5)",params![Uuid::new_v4().to_string(),job.household,chrono::Utc::now().to_rfc3339(),json!({"productId":job.product.id,"name":job.product.name}).to_string(),serde_json::to_string(&changes)?])?;
                    ("completed", None)
                }
            }
        },
    };
    tx.execute("UPDATE item_icon_jobs SET status=?4,error=?5,run_id=NULL WHERE household_id=?1 AND product_id=?2 AND run_id=?3",params![job.household,job.product.id,job.run_id,status,error])?;
    tx.commit()?;
    server.notify(&job.household);
    Ok(())
}

pub async fn run_one(server: &Server) -> Result<bool> {
    let Some(job) = claim(server)? else {
        return Ok(false);
    };
    let image = ai::render_image(
        server,
        &job.household,
        ai::ImageKind::Item,
        &format!("{}; item group: {}", job.product.name, job.product.category),
    )
    .await;
    if let Err(error) = finish(server, &job, image) {
        server.db.lock()?.execute("UPDATE item_icon_jobs SET status='failed',error='The icon could not be saved. Try again.',run_id=NULL WHERE household_id=?1 AND product_id=?2 AND run_id=?3",params![job.household,job.product.id,job.run_id])?;
        server.notify(&job.household);
        return Err(error);
    }
    Ok(true)
}

pub fn resume_interrupted(server: &Server) -> Result<()> {
    // Only the serving process calls this; administrative diagnostics don't
    // disturb active jobs. A crash resumes the queue after restart.
    server.db.lock()?.execute(
        "UPDATE item_icon_jobs SET status='queued',run_id=NULL WHERE status='running'",
        [],
    )?;
    Ok(())
}

pub fn start(server: Server) -> Result<()> {
    resume_interrupted(&server)?;
    tokio::spawn(async move {
        loop {
            if let Err(error) = run_one(&server).await {
                tracing::error!("Item icon queue: {}", error.1);
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
    Ok(())
}
