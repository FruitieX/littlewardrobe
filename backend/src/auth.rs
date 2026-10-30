use crate::{
    Server,
    error::{ApiError, Result},
    model::{Member, SessionInfo},
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
};
use chrono::Utc;
use rand::Rng;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn digest(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn email(value: &str) -> Result<String> {
    let value = value.trim().to_lowercase();
    if value.len() > 254
        || value.len() < 3
        || !value.contains('@')
        || value.chars().any(char::is_whitespace)
    {
        return Err(ApiError::bad("Enter a valid email address."));
    }
    Ok(value)
}
pub fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 {
        return Err(ApiError::bad(
            "Names must contain between 1 and 80 characters.",
        ));
    }
    Ok(value.into())
}
fn password(value: &str) -> Result<()> {
    if value.chars().count() < 12 || value.len() > 256 {
        Err(ApiError::bad(
            "Use a password with at least 12 characters (up to 256 bytes).",
        ))
    } else {
        Ok(())
    }
}
static AUTH_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
async fn hash_password(value: String) -> Result<String> {
    password(&value)?;
    let _permit = AUTH_SLOTS.acquire().await.map_err(ApiError::internal)?;
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(value.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(ApiError::internal)
    })
    .await
    .map_err(ApiError::internal)?
}
async fn verify_password(value: String, hash: String) -> Result<bool> {
    if value.len() > 256 {
        return Ok(false);
    }
    let _permit = AUTH_SLOTS.acquire().await.map_err(ApiError::internal)?;
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(value.as_bytes(), &hash)
                .is_ok()
        })
    })
    .await
    .map_err(ApiError::internal)
}

#[derive(Clone)]
pub struct Actor {
    pub user: Member,
    pub household_id: String,
    pub session_hash: String,
    pub sliding: bool,
}
/// Password sign-ins last 30 days; passkey (OIDC) sign-ins slide for a year so
/// a device in regular use never has to sign in again.
pub const SESSION_SECONDS: i64 = 30 * 86400;
pub const SLIDING_SESSION_SECONDS: i64 = 365 * 86400;
pub const SESSION_COOKIE: &str = "littlewardrobe_session";
fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .map(str::trim)
        .find_map(|cookie| cookie.strip_prefix("littlewardrobe_session="))
}
pub fn session_cookie(server: &Server, raw: &str, max_age: i64) -> String {
    format!(
        "{SESSION_COOKIE}={raw}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{}",
        if server.config.secure_cookie {
            "; Secure"
        } else {
            ""
        }
    )
}
impl Actor {
    pub fn owner(&self) -> Result<()> {
        if self.user.role == "owner" {
            Ok(())
        } else {
            Err(ApiError::forbidden())
        }
    }
}
pub fn authenticate(server: &Server, headers: &HeaderMap) -> Result<Actor> {
    let session = session_token(headers).ok_or_else(ApiError::unauthorized)?;
    if session.len() != 64 {
        return Err(ApiError::unauthorized());
    }
    let session_hash = digest(session);
    let now = Utc::now().timestamp();
    let conn = server.db.lock()?;
    let (actor, expires_at) = conn.query_row("SELECT u.id,u.name,u.email,u.role,u.household_id,s.sliding,s.expires_at FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=?1 AND s.expires_at>?2", params![session_hash, now], |row| Ok((Actor { user: Member { id: row.get(0)?, name: row.get(1)?, email: row.get(2)?, role: row.get(3)? }, household_id: row.get(4)?, session_hash: session_hash.clone(), sliding: row.get(5)? }, row.get::<_, i64>(6)?))).optional()?.ok_or_else(ApiError::unauthorized)?;
    // Extend sliding sessions at most once a day to keep reads cheap.
    if actor.sliding && expires_at < now + SLIDING_SESSION_SECONDS - 86400 {
        conn.execute(
            "UPDATE sessions SET expires_at=?1 WHERE token_hash=?2",
            params![now + SLIDING_SESSION_SECONDS, session_hash],
        )?;
    }
    Ok(actor)
}
pub fn session_info(server: &Server, actor: &Actor) -> Result<SessionInfo> {
    let conn = server.db.lock()?;
    let household_name = conn.query_row(
        "SELECT name FROM households WHERE id=?1",
        [&actor.household_id],
        |row| row.get(0),
    )?;
    let members = conn
        .prepare("SELECT id,name,email,role FROM users WHERE household_id=?1 ORDER BY role,name")?
        .query_map([&actor.household_id], |row| {
            Ok(Member {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                role: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let config: Option<String> = conn
        .query_row(
            "SELECT config FROM providers WHERE household_id=?1",
            [&actor.household_id],
            |row| row.get(0),
        )
        .optional()?;
    let config: Value = config
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or(Value::Null);
    Ok(SessionInfo {
        user: actor.user.clone(),
        household_id: actor.household_id.clone(),
        household_name,
        members,
        ai_enabled: config["enabled"] == true,
        image_enabled: config["enabled"] == true
            && config["imageModel"]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub session: SessionInfo,
    pub recovery_code: Option<String>,
}
/// Stores a new session and returns its Set-Cookie value.
pub fn create_session(server: &Server, user_id: &str, sliding: bool) -> Result<(String, String)> {
    let raw = token();
    let hash = digest(&raw);
    let now = Utc::now().timestamp();
    let lifetime = if sliding {
        SLIDING_SESSION_SECONDS
    } else {
        SESSION_SECONDS
    };
    {
        let conn = server.db.lock()?;
        conn.execute("DELETE FROM sessions WHERE expires_at<=?1", [now])?;
        conn.execute(
            "INSERT INTO sessions(token_hash,user_id,expires_at,created_at,sliding) VALUES(?1,?2,?3,?4,?5)",
            params![hash, user_id, now + lifetime, now, sliding],
        )?;
        conn.execute("DELETE FROM sessions WHERE user_id=?1 AND token_hash NOT IN (SELECT token_hash FROM sessions WHERE user_id=?1 ORDER BY created_at DESC LIMIT 10)", [user_id])?;
    }
    let cookie = session_cookie(server, &raw, lifetime);
    Ok((raw, cookie))
}
fn signed_in(
    server: &Server,
    user_id: &str,
    recovery_code: Option<String>,
) -> Result<(HeaderMap, Json<AuthResponse>)> {
    let (raw, cookie) = create_session(server, user_id, false)?;
    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.parse().map_err(ApiError::internal)?);
    let actor = authenticate(
        server,
        &HeaderMap::from_iter([(
            axum::http::header::COOKIE,
            format!("littlewardrobe_session={raw}")
                .parse()
                .map_err(ApiError::internal)?,
        )]),
    )?;
    Ok((
        headers,
        Json(AuthResponse {
            session: session_info(server, &actor)?,
            recovery_code,
        }),
    ))
}

pub async fn status(State(server): State<Server>) -> Result<Json<Value>> {
    let count: i64 = server
        .db
        .lock()?
        .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
    Ok(Json(
        json!({"setupRequired": count == 0, "oidc": server.config.oidc.is_some()}),
    ))
}
pub async fn me(
    State(server): State<Server>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<SessionInfo>)> {
    let actor = authenticate(&server, &headers)?;
    let mut response = HeaderMap::new();
    // The app asks for /me whenever it opens, so this keeps the browser's copy
    // of a sliding session cookie as fresh as the server's.
    if actor.sliding
        && let Some(raw) = session_token(&headers)
    {
        response.insert(
            SET_COOKIE,
            session_cookie(&server, raw, SLIDING_SESSION_SECONDS)
                .parse()
                .map_err(ApiError::internal)?,
        );
    }
    Ok((response, Json(session_info(&server, &actor)?)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Setup {
    pub setup_token: String,
    pub email: String,
    pub name: String,
    pub password: String,
    pub household_name: String,
    pub time_zone: String,
    #[serde(default)]
    pub samples: bool,
}
pub async fn setup(
    State(server): State<Server>,
    Json(input): Json<Setup>,
) -> Result<(HeaderMap, Json<AuthResponse>)> {
    server.rate_limit("setup", 10, 900)?;
    if digest(&input.setup_token) != digest(&server.config.setup_token) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "The setup token is incorrect. Find it in the server configuration.".into(),
        ));
    }
    let email = email(&input.email)?;
    let name = name(&input.name)?;
    let household_name = name.trim().to_string();
    let household_name = if input.household_name.trim().is_empty() {
        format!("{household_name}’s household")
    } else {
        self::name(&input.household_name)?
    };
    input
        .time_zone
        .parse::<chrono_tz::Tz>()
        .map_err(|_| ApiError::bad("Choose a valid time zone."))?;
    let hash = hash_password(input.password).await?;
    let recovery = token();
    let user_id = Uuid::new_v4().to_string();
    let household_id = Uuid::new_v4().to_string();
    {
        let mut conn = server.db.lock()?;
        let tx = conn.transaction()?;
        if tx.query_row("SELECT COUNT(*) FROM users", [], |row| row.get::<_, i64>(0))? != 0 {
            return Err(ApiError::bad(
                "Setup is already complete. Sign in or use an invitation.",
            ));
        }
        tx.execute(
            "INSERT INTO households(id,name,state) VALUES(?1,?2,?3)",
            params![
                household_id,
                household_name,
                serde_json::to_string(&crate::model::fresh_state(input.samples, input.time_zone))?
            ],
        )?;
        tx.execute("INSERT INTO users(id,household_id,email,name,password_hash,role,recovery_hash) VALUES(?1,?2,?3,?4,?5,'owner',?6)", params![user_id, household_id, email, name, hash, digest(&recovery)])?;
        tx.commit()?;
    }
    signed_in(&server, &user_id, Some(recovery))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    pub email: String,
    pub password: String,
}
pub async fn login(
    State(server): State<Server>,
    Json(input): Json<Login>,
) -> Result<(HeaderMap, Json<AuthResponse>)> {
    let email = email(&input.email)?;
    server.rate_limit(&format!("login:{}", digest(&email)), 10, 900)?;
    let found = server
        .db
        .lock()?
        .query_row(
            "SELECT id,password_hash FROM users WHERE email=?1",
            [&email],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let (id, hash) = found.unwrap_or_else(|| (String::new(), server.dummy_hash.to_string()));
    if !verify_password(input.password, hash).await? || id.is_empty() {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "The email or password is incorrect.".into(),
        ));
    }
    signed_in(&server, &id, None)
}
pub async fn logout(State(server): State<Server>, headers: HeaderMap) -> Result<HeaderMap> {
    if let Ok(actor) = authenticate(&server, &headers) {
        server.db.lock()?.execute(
            "DELETE FROM sessions WHERE token_hash=?1",
            [actor.session_hash],
        )?;
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        format!(
            "littlewardrobe_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
            if server.config.secure_cookie {
                "; Secure"
            } else {
                ""
            }
        )
        .parse()
        .map_err(ApiError::internal)?,
    );
    Ok(headers)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    pub email: String,
}
pub async fn invite(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<Invite>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    actor.owner()?;
    let email = email(&input.email)?;
    let raw = token();
    let conn = server.db.lock()?;
    if conn.query_row(
        "SELECT COUNT(*) FROM users WHERE email=?1",
        [&email],
        |row| row.get::<_, i64>(0),
    )? > 0
    {
        return Err(ApiError::bad("That email already has an account."));
    }
    if conn.query_row(
        "SELECT COUNT(*) FROM users WHERE household_id=?1",
        [&actor.household_id],
        |row| row.get::<_, i64>(0),
    )? >= 20
    {
        return Err(ApiError::bad("This household already has 20 members."));
    }
    conn.execute(
        "DELETE FROM invitations WHERE household_id=?1 AND (email=?2 OR expires_at<=?3)",
        params![actor.household_id, email, Utc::now().timestamp()],
    )?;
    conn.execute("INSERT INTO invitations(token_hash,household_id,email,expires_at,created_by) VALUES(?1,?2,?3,?4,?5)", params![digest(&raw),actor.household_id,email,Utc::now().timestamp()+7*86400,actor.user.id])?;
    Ok(Json(
        json!({ "url": format!("{}/?invite={raw}", server.config.origin), "expiresInDays": 7 }),
    ))
}
pub async fn invitation(
    State(server): State<Server>,
    Path(raw): Path<String>,
) -> Result<Json<Value>> {
    let conn = server.db.lock()?;
    let found: Option<(String,String)> = conn.query_row("SELECT i.email,h.name FROM invitations i JOIN households h ON h.id=i.household_id WHERE i.token_hash=?1 AND i.expires_at>?2", params![digest(&raw),Utc::now().timestamp()], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
    found
        .map(|(email, household)| Json(json!({"email":email,"householdName":household})))
        .ok_or_else(|| ApiError::bad("This invitation has expired or has already been used."))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Join {
    pub token: String,
    pub email: String,
    pub name: String,
    pub password: String,
}
pub async fn join(
    State(server): State<Server>,
    Json(input): Json<Join>,
) -> Result<(HeaderMap, Json<AuthResponse>)> {
    server.rate_limit("join", 20, 900)?;
    let email = email(&input.email)?;
    let name = name(&input.name)?;
    let hash = hash_password(input.password).await?;
    let recovery = token();
    let user_id = Uuid::new_v4().to_string();
    {
        let mut conn = server.db.lock()?;
        let tx = conn.transaction()?;
        let household: Option<String> = tx.query_row("SELECT household_id FROM invitations WHERE token_hash=?1 AND email=?2 AND expires_at>?3", params![digest(&input.token),email,Utc::now().timestamp()], |row| row.get(0)).optional()?;
        let household = household.ok_or_else(|| {
            ApiError::bad("This invitation is invalid, expired, or for a different email.")
        })?;
        if tx.query_row(
            "SELECT COUNT(*) FROM users WHERE email=?1",
            [&email],
            |row| row.get::<_, i64>(0),
        )? > 0
        {
            return Err(ApiError::bad("That email already has an account."));
        }
        tx.execute("INSERT INTO users(id,household_id,email,name,password_hash,role,recovery_hash) VALUES(?1,?2,?3,?4,?5,'member',?6)", params![user_id,household,email,name,hash,digest(&recovery)])?;
        tx.execute(
            "DELETE FROM invitations WHERE token_hash=?1",
            [digest(&input.token)],
        )?;
        tx.commit()?;
        server.notify(&household);
    }
    signed_in(&server, &user_id, Some(recovery))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub current_password: Option<String>,
    pub new_password: Option<String>,
}
pub async fn profile(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<Profile>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    let name = name(&input.name)?;
    if let Some(new_password) = input.new_password {
        server.rate_limit(&format!("profile:{}", actor.user.id), 10, 900)?;
        let current = input
            .current_password
            .ok_or_else(|| ApiError::bad("Enter your current password."))?;
        let hash = server.db.lock()?.query_row(
            "SELECT password_hash FROM users WHERE id=?1",
            [&actor.user.id],
            |row| row.get(0),
        )?;
        if !verify_password(current, hash).await? {
            return Err(ApiError::bad("Your current password is incorrect."));
        }
        let hash = hash_password(new_password).await?;
        let conn = server.db.lock()?;
        conn.execute(
            "UPDATE users SET password_hash=?1,name=?2 WHERE id=?3",
            params![hash, name, actor.user.id],
        )?;
        conn.execute(
            "DELETE FROM sessions WHERE user_id=?1 AND token_hash<>?2",
            params![actor.user.id, actor.session_hash],
        )?;
    } else {
        server.db.lock()?.execute(
            "UPDATE users SET name=?1 WHERE id=?2",
            params![name, actor.user.id],
        )?;
    }
    server.notify(&actor.household_id);
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HouseholdProfile {
    pub name: String,
    pub expected_name: String,
}
pub async fn household_profile(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<HouseholdProfile>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    actor.owner()?;
    let name = name(&input.name)?;
    let changed = server.db.lock()?.execute(
        "UPDATE households SET name=?1 WHERE id=?2 AND name=?3",
        params![name, actor.household_id, input.expected_name],
    )?;
    if changed == 0 {
        return Err(ApiError::conflict());
    }
    server.notify(&actor.household_id);
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recover {
    pub email: String,
    pub recovery_code: String,
    pub new_password: String,
}
pub async fn recover(
    State(server): State<Server>,
    Json(input): Json<Recover>,
) -> Result<(HeaderMap, Json<AuthResponse>)> {
    let email = email(&input.email)?;
    server.rate_limit(&format!("recovery:{}", digest(&email)), 5, 900)?;
    let id: Option<String> = server
        .db
        .lock()?
        .query_row(
            "SELECT id FROM users WHERE email=?1 AND recovery_hash=?2",
            params![email, digest(&input.recovery_code)],
            |row| row.get(0),
        )
        .optional()?;
    let id = id.ok_or_else(|| {
        ApiError(
            StatusCode::UNAUTHORIZED,
            "The email or recovery code is incorrect.".into(),
        )
    })?;
    let hash = hash_password(input.new_password).await?;
    let recovery = token();
    {
        let mut conn = server.db.lock()?;
        let tx = conn.transaction()?;
        if tx.execute(
            "UPDATE users SET password_hash=?1,recovery_hash=?2 WHERE id=?3 AND recovery_hash=?4",
            params![hash, digest(&recovery), id, digest(&input.recovery_code)],
        )? != 1
        {
            return Err(ApiError::unauthorized());
        }
        tx.execute("DELETE FROM sessions WHERE user_id=?1", [&id])?;
        tx.commit()?;
    }
    signed_in(&server, &id, Some(recovery))
}
pub async fn remove_member(
    State(server): State<Server>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    actor.owner()?;
    if id == actor.user.id {
        return Err(ApiError::bad("The household owner cannot be removed."));
    }
    server.db.lock()?.execute(
        "DELETE FROM users WHERE id=?1 AND household_id=?2 AND role='member'",
        params![id, actor.household_id],
    )?;
    server.notify(&actor.household_id);
    Ok(Json(json!({"ok":true})))
}

pub fn dummy_hash() -> String {
    Argon2::default()
        .hash_password(b"dummy-account-password-only")
        .expect("Argon2 dummy hash")
        .to_string()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryKeyRequest {
    pub current_password: String,
}
pub async fn new_recovery_code(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(input): Json<RecoveryKeyRequest>,
) -> Result<Json<Value>> {
    let actor = authenticate(&server, &headers)?;
    server.rate_limit(&format!("recovery-key:{}", actor.user.id), 10, 900)?;
    let hash = server.db.lock()?.query_row(
        "SELECT password_hash FROM users WHERE id=?1",
        [&actor.user.id],
        |row| row.get(0),
    )?;
    if !verify_password(input.current_password, hash).await? {
        return Err(ApiError::bad("Your current password is incorrect."));
    }
    let code = token();
    server.db.lock()?.execute(
        "UPDATE users SET recovery_hash=?1 WHERE id=?2",
        params![digest(&code), actor.user.id],
    )?;
    Ok(Json(json!({"recoveryCode":code})))
}
