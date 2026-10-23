//! Optional sign-in through an OpenID Connect provider (Pocket ID): the
//! authorization code flow with PKCE, ending in Little Wardrobe's own sliding
//! session. Password accounts keep working alongside it.

use crate::{
    Server,
    auth::{create_session, digest, token},
    error::{ApiError, Result},
};
use axum::{
    extract::{Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{LOCATION, SET_COOKIE},
    },
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use uuid::Uuid;

pub const STATE_COOKIE: &str = "littlewardrobe_oidc";
const STATE_SECONDS: i64 = 600;
const CALLBACK_PATH: &str = "/api/auth/oidc/callback";

pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    /// Sign-in needs one of these groups; empty allows anyone the provider admits.
    pub allowed_groups: Vec<String>,
    /// Provider usernames (lowercase) mapped to local account emails (lowercase),
    /// used once to link existing password accounts.
    pub user_map: Vec<(String, String)>,
}

#[derive(Clone, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
}

pub struct Oidc {
    pub config: OidcConfig,
    discovery: Mutex<Option<Discovery>>,
}

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub iss: String,
    pub sub: String,
    aud: Audience,
    exp: i64,
    nonce: Option<String>,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    #[serde(default)]
    pub groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}
impl Audience {
    fn contains(&self, id: &str) -> bool {
        match self {
            Audience::One(aud) => aud == id,
            Audience::Many(auds) => auds.iter().any(|aud| aud == id),
        }
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
fn list(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
}

impl Oidc {
    pub fn new(config: OidcConfig) -> Self {
        Self {
            config,
            discovery: Mutex::new(None),
        }
    }
    /// Reads `LITTLEWARDROBE_OIDC_*`. Sign-in stays password-only unless the
    /// issuer, client id and client secret are all set.
    pub fn from_env(origin: &str) -> Option<Self> {
        let (issuer, client_id, client_secret) = (
            env("LITTLEWARDROBE_OIDC_ISSUER"),
            env("LITTLEWARDROBE_OIDC_CLIENT_ID"),
            env("LITTLEWARDROBE_OIDC_CLIENT_SECRET"),
        );
        let (Some(issuer), Some(client_id), Some(client_secret)) =
            (issuer, client_id, client_secret)
        else {
            if env("LITTLEWARDROBE_OIDC_ISSUER").is_some()
                || env("LITTLEWARDROBE_OIDC_CLIENT_ID").is_some()
            {
                tracing::warn!(
                    "OIDC sign-in is disabled: set LITTLEWARDROBE_OIDC_ISSUER, _CLIENT_ID and _CLIENT_SECRET"
                );
            }
            return None;
        };
        let user_map = env("LITTLEWARDROBE_OIDC_USER_MAP")
            .map(|value| {
                list(&value)
                    .filter_map(|pair| pair.split_once(':'))
                    .map(|(user, email)| (user.trim().to_lowercase(), email.trim().to_lowercase()))
                    .collect()
            })
            .unwrap_or_default();
        Some(Self::new(OidcConfig {
            issuer: issuer.trim_end_matches('/').to_string(),
            client_id,
            client_secret,
            redirect_url: env("LITTLEWARDROBE_OIDC_REDIRECT_URL")
                .unwrap_or_else(|| format!("{origin}{CALLBACK_PATH}")),
            allowed_groups: env("LITTLEWARDROBE_OIDC_ALLOWED_GROUPS")
                .map(|value| list(&value).map(String::from).collect())
                .unwrap_or_default(),
            user_map,
        }))
    }

    async fn discovery(&self, client: &reqwest::Client) -> Result<Discovery> {
        if let Some(discovery) = self
            .discovery
            .lock()
            .map_err(|_| ApiError::internal("lock"))?
            .clone()
        {
            return Ok(discovery);
        }
        let url = format!("{}/.well-known/openid-configuration", self.config.issuer);
        let discovery: Discovery = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(ApiError::internal)?
            .json()
            .await
            .map_err(ApiError::internal)?;
        *self
            .discovery
            .lock()
            .map_err(|_| ApiError::internal("lock"))? = Some(discovery.clone());
        Ok(discovery)
    }

    fn authorization_url(
        &self,
        discovery: &Discovery,
        state: &str,
        nonce: &str,
        verifier: &str,
    ) -> String {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let query = form_encode(&[
            ("response_type", "code"),
            ("client_id", &self.config.client_id),
            ("redirect_uri", &self.config.redirect_url),
            ("scope", "openid profile email groups"),
            ("state", state),
            ("nonce", nonce),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
        ]);
        let separator = if discovery.authorization_endpoint.contains('?') {
            '&'
        } else {
            '?'
        };
        format!("{}{separator}{query}", discovery.authorization_endpoint)
    }

    /// The ID token comes straight from the token endpoint over TLS, so its
    /// signature need not be checked (OpenID Connect Core 3.1.3.7); the claims
    /// still are.
    pub fn check_id_token(
        &self,
        issuer: &str,
        nonce: &str,
        id_token: &str,
    ) -> std::result::Result<Claims, String> {
        let payload = id_token.split('.').nth(1).ok_or("malformed ID token")?;
        let bytes = URL_SAFE_NO_PAD
            .decode(payload.trim_end_matches('='))
            .map_err(|_| "malformed ID token")?;
        let claims: Claims =
            serde_json::from_slice(&bytes).map_err(|_| "unreadable ID token claims")?;
        if claims.iss != issuer {
            return Err("ID token from an unexpected issuer".into());
        }
        if !claims.aud.contains(&self.config.client_id) {
            return Err("ID token is for another client".into());
        }
        if claims.exp <= Utc::now().timestamp() {
            return Err("ID token has expired".into());
        }
        if claims.nonce.as_deref() != Some(nonce) {
            return Err("ID token nonce does not match".into());
        }
        let allowed = &self.config.allowed_groups;
        if !allowed.is_empty() && !claims.groups.iter().any(|group| allowed.contains(group)) {
            return Err("not_allowed".into());
        }
        Ok(claims)
    }
}

fn form_encode(pairs: &[(&str, &str)]) -> String {
    fn encode(s: &str) -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect()
    }
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn redirect(location: &str) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(LOCATION, value);
    }
    response
}
/// Returns to the app with a message the sign-in screen shows.
fn failed(server: &Server, code: &str) -> Response {
    let mut response = redirect(&format!("/?oidcError={code}"));
    if let Ok(value) = HeaderValue::from_str(&state_cookie(server, "", 0)) {
        response.headers_mut().append(SET_COOKIE, value);
    }
    response
}
fn state_cookie(server: &Server, value: &str, max_age: i64) -> String {
    format!(
        "{STATE_COOKIE}={value}; Path=/api/auth/oidc; HttpOnly; SameSite=Lax; Max-Age={max_age}{}",
        if server.config.secure_cookie {
            "; Secure"
        } else {
            ""
        }
    )
}
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .map(str::trim)
        .find_map(|cookie| cookie.strip_prefix(name)?.strip_prefix('='))
}

pub async fn login(State(server): State<Server>, headers: HeaderMap) -> Response {
    let Some(oidc) = server.config.oidc.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // The state cookie must live on the host the provider returns to, so
    // legacy hostnames first hop over to the canonical origin.
    let origin_host = server.config.origin.split("://").nth(1).unwrap_or_default();
    if headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| !host.eq_ignore_ascii_case(origin_host))
    {
        return redirect(&format!("{}/api/auth/oidc/login", server.config.origin));
    }
    let discovery = match oidc.discovery(&server.client).await {
        Ok(discovery) => discovery,
        Err(error) => {
            tracing::warn!("OIDC discovery failed: {}", error.1);
            return failed(&server, "unavailable");
        }
    };
    let (state, nonce, verifier) = (token(), token(), token());
    let stored = server.db.lock().and_then(|conn| {
        let now = Utc::now().timestamp();
        conn.execute("DELETE FROM oidc_logins WHERE expires_at<=?1", [now])?;
        conn.execute(
            "INSERT INTO oidc_logins(state_hash,nonce,verifier,expires_at) VALUES(?1,?2,?3,?4)",
            params![digest(&state), nonce, verifier, now + STATE_SECONDS],
        )?;
        Ok(())
    });
    if stored.is_err() {
        return failed(&server, "unavailable");
    }
    let mut response = redirect(&oidc.authorization_url(&discovery, &state, &nonce, &verifier));
    if let Ok(value) = HeaderValue::from_str(&state_cookie(&server, &state, STATE_SECONDS)) {
        response.headers_mut().append(SET_COOKIE, value);
    }
    response
}

#[derive(Deserialize)]
pub struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn callback(
    State(server): State<Server>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Response {
    let Some(oidc) = server.config.oidc.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Some(error) = query.error {
        tracing::info!("OIDC provider returned {error}");
        return failed(
            &server,
            if error == "access_denied" {
                "not_allowed"
            } else {
                "failed"
            },
        );
    }
    let (Some(code), Some(state)) = (query.code, query.state) else {
        return failed(&server, "failed");
    };
    if cookie(&headers, STATE_COOKIE) != Some(state.as_str()) {
        return failed(&server, "expired");
    }
    // Each sign-in attempt can be redeemed once.
    let login: Option<(String, String)> = server.db.lock().ok().and_then(|conn| {
        let found = conn
            .query_row(
                "SELECT nonce,verifier FROM oidc_logins WHERE state_hash=?1 AND expires_at>?2",
                params![digest(&state), Utc::now().timestamp()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .ok()
            .flatten();
        let _ = conn.execute(
            "DELETE FROM oidc_logins WHERE state_hash=?1",
            [digest(&state)],
        );
        found
    });
    let Some((nonce, verifier)) = login else {
        return failed(&server, "expired");
    };
    let claims = match redeem(&server, oidc, &code, &nonce, &verifier).await {
        Ok(claims) => claims,
        Err(reason) => {
            tracing::warn!("OIDC sign-in refused: {reason}");
            return failed(
                &server,
                if reason == "not_allowed" {
                    "not_allowed"
                } else {
                    "failed"
                },
            );
        }
    };
    let user = match server.db.lock().map_err(|e| e.1).and_then(|mut conn| {
        let user = link(&mut conn, &oidc.config, &claims).map_err(|e| e.1)?;
        Ok(user)
    }) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::warn!(
                "OIDC user {:?} has no Little Wardrobe account to link",
                claims.preferred_username
            );
            return failed(&server, "no_account");
        }
        Err(error) => {
            tracing::error!("OIDC account linking failed: {error}");
            return failed(&server, "failed");
        }
    };
    let Ok((_, session)) = create_session(&server, &user.0, true) else {
        return failed(&server, "failed");
    };
    if let Some(household) = user.1 {
        server.notify(&household);
    }
    let mut response = redirect("/");
    for value in [session, state_cookie(&server, "", 0)] {
        if let Ok(value) = HeaderValue::from_str(&value) {
            response.headers_mut().append(SET_COOKIE, value);
        }
    }
    response
}

async fn redeem(
    server: &Server,
    oidc: &Oidc,
    code: &str,
    nonce: &str,
    verifier: &str,
) -> std::result::Result<Claims, String> {
    let discovery = oidc.discovery(&server.client).await.map_err(|e| e.1)?;
    #[derive(Deserialize)]
    struct Tokens {
        id_token: String,
    }
    let response = server
        .client
        .post(&discovery.token_endpoint)
        .timeout(std::time::Duration::from_secs(15))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form_encode(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &oidc.config.redirect_url),
            ("client_id", &oidc.config.client_id),
            ("client_secret", &oidc.config.client_secret),
            ("code_verifier", verifier),
        ]))
        .send()
        .await
        .map_err(|error| format!("token request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("token endpoint answered {}", response.status()));
    }
    let tokens: Tokens = response
        .json()
        .await
        .map_err(|error| format!("unreadable token response: {error}"))?;
    oidc.check_id_token(&discovery.issuer, nonce, &tokens.id_token)
}

/// Finds the local account for a provider identity: an existing link, then
/// the configured username map, and otherwise a new member of the only
/// household. Returns the user id and, for a new member, its household.
pub fn link(
    conn: &mut Connection,
    config: &OidcConfig,
    claims: &Claims,
) -> Result<Option<(String, Option<String>)>> {
    let tx = conn.transaction()?;
    let linked: Option<String> = tx
        .query_row(
            "SELECT user_id FROM oidc_identities WHERE issuer=?1 AND subject=?2",
            params![claims.iss, claims.sub],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(user) = linked {
        return Ok(Some((user, None)));
    }
    let username = claims
        .preferred_username
        .clone()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let mapped = config
        .user_map
        .iter()
        .find(|(user, _)| !username.is_empty() && *user == username)
        .map(|(_, email)| email.clone());
    let now = Utc::now().timestamp();
    let (user, created) = if let Some(email) = mapped {
        let Some(user) = tx
            .query_row("SELECT id FROM users WHERE email=?1", [&email], |row| {
                row.get::<_, String>(0)
            })
            .optional()?
        else {
            return Ok(None);
        };
        // One provider identity per account, so a recreated provider user
        // with the same username cannot take over a linked account.
        let taken: i64 = tx.query_row(
            "SELECT COUNT(*) FROM oidc_identities WHERE issuer=?1 AND user_id=?2",
            params![claims.iss, user],
            |row| row.get(0),
        )?;
        if taken > 0 {
            return Ok(None);
        }
        (user, None)
    } else {
        let households: Vec<String> = tx
            .prepare("SELECT id FROM households LIMIT 2")?
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        let [household] = households.as_slice() else {
            return Ok(None);
        };
        let members: i64 = tx.query_row(
            "SELECT COUNT(*) FROM users WHERE household_id=?1",
            [household],
            |row| row.get(0),
        )?;
        if members >= 20 {
            return Ok(None);
        }
        let verified = claims
            .email
            .as_deref()
            .filter(|_| claims.email_verified)
            .and_then(|email| crate::auth::email(email).ok())
            .filter(|email| {
                tx.query_row(
                    "SELECT COUNT(*) FROM users WHERE email=?1",
                    [email],
                    |row| row.get::<_, i64>(0),
                )
                .is_ok_and(|count| count == 0)
            });
        let host = claims
            .iss
            .split("://")
            .nth(1)
            .unwrap_or("oidc")
            .split('/')
            .next()
            .unwrap_or("oidc");
        let local = if username.is_empty() {
            claims.sub.to_lowercase()
        } else {
            username.clone()
        };
        let email = verified.unwrap_or_else(|| format!("{local}@{host}"));
        if tx.query_row(
            "SELECT COUNT(*) FROM users WHERE email=?1",
            [&email],
            |row| row.get::<_, i64>(0),
        )? > 0
        {
            return Ok(None);
        }
        let name = claims
            .name
            .as_deref()
            .or(claims.preferred_username.as_deref())
            .and_then(|name| crate::auth::name(name).ok())
            .unwrap_or_else(|| "Family member".into());
        let id = Uuid::new_v4().to_string();
        // Provider-only accounts have no usable password; "!" never verifies.
        tx.execute(
            "INSERT INTO users(id,household_id,email,name,password_hash,role,recovery_hash) VALUES(?1,?2,?3,?4,'!','member',?5)",
            params![id, household, email, name, digest(&token())],
        )?;
        (id, Some(household.clone()))
    };
    tx.execute(
        "INSERT INTO oidc_identities(issuer,subject,user_id,username,linked_at) VALUES(?1,?2,?3,?4,?5)",
        params![claims.iss, claims.sub, user, username, now],
    )?;
    tx.commit()?;
    Ok(Some((user, created)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oidc(groups: &[&str]) -> Oidc {
        Oidc::new(OidcConfig {
            issuer: "https://id.example.org".into(),
            client_id: "littlewardrobe".into(),
            client_secret: "s".into(),
            redirect_url: "https://app.example.org/api/auth/oidc/callback".into(),
            allowed_groups: groups.iter().map(|g| g.to_string()).collect(),
            user_map: vec![("kim".into(), "kim@example.org".into())],
        })
    }
    fn id_token(claims: &serde_json::Value) -> String {
        format!(
            "e30.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).unwrap())
        )
    }
    fn claims() -> serde_json::Value {
        serde_json::json!({
            "iss": "https://id.example.org", "sub": "u1", "aud": "littlewardrobe",
            "exp": Utc::now().timestamp() + 60, "nonce": "n1", "name": "Kim",
            "preferred_username": "kim", "groups": ["family"],
        })
    }

    #[test]
    fn accepts_valid_claims_and_rejects_bad_ones() {
        let checked = oidc(&["family"])
            .check_id_token("https://id.example.org", "n1", &id_token(&claims()))
            .unwrap();
        assert_eq!(checked.sub, "u1");
        for (key, value) in [
            ("iss", serde_json::json!("https://evil.example.org")),
            ("aud", serde_json::json!(["other"])),
            ("exp", serde_json::json!(1)),
            ("nonce", serde_json::json!("n2")),
            ("groups", serde_json::json!(["kids"])),
        ] {
            let mut claims = claims();
            claims[key] = value;
            assert!(
                oidc(&["family"])
                    .check_id_token("https://id.example.org", "n1", &id_token(&claims))
                    .is_err(),
                "{key} should be rejected"
            );
        }
        let mut no_groups = claims();
        no_groups["groups"] = serde_json::json!([]);
        assert_eq!(
            oidc(&["family"])
                .check_id_token("https://id.example.org", "n1", &id_token(&no_groups))
                .unwrap_err(),
            "not_allowed"
        );
        assert!(
            oidc(&[])
                .check_id_token("https://id.example.org", "n1", &id_token(&no_groups))
                .is_ok()
        );
        assert!(
            oidc(&[])
                .check_id_token("https://id.example.org", "n1", "garbage")
                .is_err()
        );
    }

    #[test]
    fn authorization_url_uses_pkce_and_state() {
        let discovery = Discovery {
            issuer: "https://id.example.org".into(),
            authorization_endpoint: "https://id.example.org/authorize".into(),
            token_endpoint: "https://id.example.org/api/oidc/token".into(),
        };
        let url = oidc(&[]).authorization_url(&discovery, "st", "n1", "verifier");
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(b"verifier"));
        assert!(url.starts_with(
            "https://id.example.org/authorize?response_type=code&client_id=littlewardrobe&"
        ));
        assert!(url.contains(&format!(
            "code_challenge={challenge}&code_challenge_method=S256"
        )));
        assert!(
            url.contains(
                "redirect_uri=https%3A%2F%2Fapp.example.org%2Fapi%2Fauth%2Foidc%2Fcallback"
            )
        );
        assert!(url.contains("state=st&nonce=n1"));
        assert!(url.contains("scope=openid%20profile%20email%20groups"));
    }

    #[test]
    fn cookie_lookup_matches_whole_names() {
        let headers = HeaderMap::from_iter([(
            axum::http::header::COOKIE,
            HeaderValue::from_static("littlewardrobe_oidc_x=1; littlewardrobe_oidc=abc"),
        )]);
        assert_eq!(cookie(&headers, STATE_COOKIE), Some("abc"));
    }
}
