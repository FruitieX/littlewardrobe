//! Outbound-only personal companion. Its token grants jobs, never household APIs.
use crate::{
    Server, auth,
    error::{ApiError, Result},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub fn rotate(server: &Server, household: &str) -> Result<String> {
    let token = auth::token();
    let mut conn = server.db.lock()?;
    let tx = conn.transaction()?;
    tx.execute("INSERT INTO companion_workers(household_id,token_hash) VALUES(?1,?2) ON CONFLICT(household_id) DO UPDATE SET token_hash=excluded.token_hash,last_seen=0",params![household,auth::digest(&token)])?;
    tx.execute(
        "DELETE FROM companion_jobs WHERE household_id=?1",
        [household],
    )?;
    tx.commit()?;
    Ok(token)
}
pub fn status(server: &Server, household: &str) -> Result<Value> {
    let seen: Option<i64> = server
        .db
        .lock()?
        .query_row(
            "SELECT last_seen FROM companion_workers WHERE household_id=?1",
            [household],
            |row| row.get(0),
        )
        .optional()?;
    Ok(
        json!({"paired":seen.is_some(),"online":seen.is_some_and(|seen|seen>chrono::Utc::now().timestamp()-45)}),
    )
}
pub async fn pair(State(server): State<Server>, headers: HeaderMap) -> Result<Json<Value>> {
    let actor = auth::authenticate(&server, &headers)?;
    actor.owner()?;
    Ok(Json(json!({"token":rotate(&server,&actor.household_id)?})))
}
pub async fn disconnect(State(server): State<Server>, headers: HeaderMap) -> Result<Json<Value>> {
    let actor = auth::authenticate(&server, &headers)?;
    actor.owner()?;
    let mut conn = server.db.lock()?;
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM companion_workers WHERE household_id=?1",
        [&actor.household_id],
    )?;
    tx.execute(
        "DELETE FROM companion_jobs WHERE household_id=?1",
        [&actor.household_id],
    )?;
    tx.commit()?;
    Ok(Json(json!({"ok":true})))
}
fn worker(conn: &rusqlite::Connection, headers: &HeaderMap) -> Result<String> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| v.len() == 64)
        .ok_or_else(ApiError::unauthorized)?;
    conn.query_row(
        "SELECT household_id FROM companion_workers WHERE token_hash=?1",
        [auth::digest(token)],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(ApiError::unauthorized)
}
pub async fn claim(State(server): State<Server>, headers: HeaderMap) -> Result<Json<Value>> {
    let mut conn = server.db.lock()?;
    let tx = conn.transaction()?;
    let household = worker(&tx, &headers)?;
    server.rate_limit(&format!("companion:{household}"), 240, 60)?;
    tx.execute(
        "UPDATE companion_workers SET last_seen=unixepoch() WHERE household_id=?1",
        [&household],
    )?;
    tx.execute(
        "DELETE FROM companion_jobs WHERE deadline < unixepoch()",
        [],
    )?;
    let job: Option<(String,String,String)> = tx.query_row("SELECT id,path,body FROM companion_jobs WHERE household_id=?1 AND status='pending' AND deadline>unixepoch() ORDER BY rowid LIMIT 1",[&household],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    let value = if let Some((id, path, body)) = job {
        let lease = auth::token();
        tx.execute(
            "UPDATE companion_jobs SET status='running',lease=?2,body='' WHERE id=?1",
            params![id, lease],
        )?;
        json!({"job":{"id":id,"lease":lease,"path":path,"body":serde_json::from_str::<Value>(&body)?}})
    } else {
        json!({"job":null})
    };
    tx.commit()?;
    Ok(Json(value))
}
pub async fn heartbeat(State(server): State<Server>, headers: HeaderMap) -> Result<Json<Value>> {
    let conn = server.db.lock()?;
    let household = worker(&conn, &headers)?;
    conn.execute(
        "UPDATE companion_workers SET last_seen=unixepoch() WHERE household_id=?1",
        [household],
    )?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completion {
    pub id: String,
    pub lease: String,
    pub result: Option<Value>,
    pub failed: bool,
}
pub async fn complete(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<Completion>,
) -> Result<Json<Value>> {
    let conn = server.db.lock()?;
    let household = worker(&conn, &headers)?;
    if input.id.len() > 64 || input.lease.len() != 64 || input.failed == input.result.is_some() {
        return Err(ApiError::bad("Invalid companion completion."));
    }
    let result = serde_json::to_string(&input.result)?;
    if result.len() > 28 * 1024 * 1024 {
        return Err(ApiError::bad("Companion result too large."));
    }
    let count=conn.execute("UPDATE companion_jobs SET status=?4,result=?5,lease=NULL WHERE id=?1 AND household_id=?2 AND lease=?3 AND status='running' AND deadline>unixepoch()",params![input.id,household,input.lease,if input.failed {"failed"} else {"done"},result])?;
    if count != 1 {
        return Err(ApiError::conflict());
    }
    Ok(Json(json!({"ok":true})))
}
pub async fn send(
    server: &Server,
    household: &str,
    path: &str,
    body: Option<&Value>,
) -> Result<Value> {
    if status(server, household)?["online"] != true {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "Your personal companion is offline. Start it on your agent machine.".into(),
        ));
    }
    let id = Uuid::new_v4().to_string();
    {
        let mut conn = server.db.lock()?;
        let tx = conn.transaction()?;
        let active: u32 = tx.query_row(
            "SELECT COUNT(*) FROM companion_jobs WHERE household_id=?1 AND deadline>unixepoch()",
            [household],
            |r| r.get(0),
        )?;
        if active >= 4 {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "The companion has several requests already. Try again shortly.".into(),
            ));
        }
        tx.execute("INSERT INTO companion_jobs(id,household_id,path,body,deadline) VALUES(?1,?2,?3,?4,unixepoch()+360)",params![id,household,path,serde_json::to_string(&body)?])?;
        tx.commit()?;
    }
    // Never reassign a claimed job: a crashed image worker must not cause duplicate usage.
    for _ in 0..1200 {
        let result: Option<(String, Option<String>)> = server
            .db
            .lock()?
            .query_row(
                "SELECT status,result FROM companion_jobs WHERE id=?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match result {
            Some((status, result)) if status == "done" || status == "failed" => {
                server
                    .db
                    .lock()?
                    .execute("DELETE FROM companion_jobs WHERE id=?1", [&id])?;
                if status == "failed" {
                    return Err(ApiError(StatusCode::BAD_GATEWAY,"The personal companion could not complete this request. Check its login, model access or usage limit.".into()));
                }
                return Ok(serde_json::from_str(&result.unwrap_or_default())?);
            }
            None => {
                return Err(ApiError(
                    StatusCode::BAD_GATEWAY,
                    "The companion connection changed or the request expired. Please try again."
                        .into(),
                ));
            }
            _ => {}
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    server
        .db
        .lock()?
        .execute("DELETE FROM companion_jobs WHERE id=?1", [&id])?;
    Err(ApiError(
        StatusCode::GATEWAY_TIMEOUT,
        "The companion took too long. Please try again.".into(),
    ))
}
