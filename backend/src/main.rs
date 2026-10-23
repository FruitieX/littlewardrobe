mod ai;
mod api;
mod auth;
mod clothing;
#[cfg(test)]
mod clothing_checks;
mod companion;
mod db;
mod domain;
mod error;
mod icon_jobs;
mod insights;
mod model;
mod oidc;
mod photo_jobs;
#[cfg(test)]
mod tests;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post, put},
};
use error::{ApiError, Result};
use serde_json::json;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

pub struct Config {
    pub origin: String,
    pub secure_cookie: bool,
    pub setup_token: String,
    pub data_dir: PathBuf,
    pub encryption_key: [u8; 32],
    pub oidc: Option<oidc::Oidc>,
}
#[derive(Clone)]
pub struct Server {
    pub db: db::Database,
    pub config: Arc<Config>,
    pub notices: broadcast::Sender<String>,
    pub dummy_hash: Arc<str>,
    pub client: reqwest::Client,
    pub limits: Arc<Mutex<HashMap<String, (u32, i64)>>>,
}
impl Server {
    pub fn notify(&self, household: &str) {
        let _ = self.notices.send(household.into());
    }
    pub fn rate_limit(&self, key: &str, max: u32, window: i64) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let mut limits = self.limits.lock().unwrap_or_else(|poisoned| {
            self.limits.clear_poison();
            poisoned.into_inner()
        });
        if limits.len() > 10000 {
            limits.retain(|_, (_, until)| *until > now);
        }
        let entry = limits.entry(key.into()).or_insert((0, now + window));
        if entry.1 <= now {
            *entry = (0, now + window);
        }
        entry.0 += 1;
        if entry.0 > max {
            Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many attempts. Please try again in a few minutes.".into(),
            ))
        } else {
            Ok(())
        }
    }
}

pub fn app(server: Server) -> Router {
    let router = Router::new()
        .route(
            "/api/health",
            get(|| async { Json(json!({"status":"ok","service":"littlewardrobe-api"})) }),
        )
        .route("/api/ready", get(ready))
        .route("/api/auth/status", get(auth::status))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/setup", post(auth::setup))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/oidc/login", get(oidc::login))
        .route("/api/auth/oidc/callback", get(oidc::callback))
        .route("/api/auth/recover", post(auth::recover))
        .route("/api/auth/join", post(auth::join))
        .route("/api/auth/profile", put(auth::profile))
        .route("/api/household", put(auth::household_profile))
        .route("/api/auth/recovery-code", post(auth::new_recovery_code))
        .route("/api/invitations", post(auth::invite))
        .route("/api/invitations/{token}", get(auth::invitation))
        .route("/api/members/{id}", delete(auth::remove_member))
        .route("/api/state", get(api::state))
        .route("/api/operations", post(api::mutate))
        .route("/api/events", get(api::stream))
        .route("/api/history", get(api::history))
        .route("/api/insights", get(insights::insights))
        .route("/api/images", post(api::upload))
        .route("/api/images/cleanup", post(api::cleanup_images))
        .route(
            "/api/images/{id}",
            get(api::image).delete(api::delete_image),
        )
        .route("/api/backup", get(api::backup))
        .route(
            "/api/restore",
            post(api::restore).layer(DefaultBodyLimit::max(96 * 1024 * 1024)),
        )
        .route(
            "/api/ai/config",
            get(ai::config)
                .put(ai::save_config)
                .delete(ai::delete_config),
        )
        .route("/api/ai/models", get(ai::models))
        .route("/api/ai/test", post(ai::test))
        .route("/api/ai/assistant", post(ai::assistant))
        .route("/api/sales/{id}/export", get(clothing::export_listing))
        .route("/api/ai/analyze", post(clothing::analyze))
        .route(
            "/api/ai/photo-jobs",
            get(photo_jobs::status).post(photo_jobs::create),
        )
        .route("/api/ai/photo-jobs/{id}", post(photo_jobs::manage))
        .route("/api/ai/listing", post(clothing::draft_listing))
        .route("/api/ai/enhance", post(clothing::enhance))
        .route("/api/ai/image", post(ai::generate_image))
        .route(
            "/api/ai/item-icons",
            get(icon_jobs::status).post(icon_jobs::manage),
        )
        .route(
            "/api/ai/companion",
            post(companion::pair).delete(companion::disconnect),
        )
        .route("/api/companion/claim", post(companion::claim))
        .route("/api/companion/heartbeat", post(companion::heartbeat))
        .route(
            "/api/companion/complete",
            post(companion::complete).layer(DefaultBodyLimit::max(28 * 1024 * 1024)),
        )
        .layer(DefaultBodyLimit::max(9 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(server.clone(), protect))
        .layer(tower_http::catch_panic::CatchPanicLayer::custom(
            panic_response,
        ))
        .with_state(server);
    if let Ok(directory) = std::env::var("LITTLEWARDROBE_STATIC_DIR") {
        router
            .route(
                "/api/{*path}",
                axum::routing::any(|| async { StatusCode::NOT_FOUND }),
            )
            .fallback_service(tower_http::services::ServeDir::new(&directory).fallback(
                tower_http::services::ServeFile::new(
                    std::path::Path::new(&directory).join("_shell.html"),
                ),
            ))
    } else {
        router
    }
}
fn panic_response(_: Box<dyn std::any::Any + Send + 'static>) -> Response {
    use axum::response::IntoResponse;
    ApiError::internal("Request handler panicked").into_response()
}

async fn ready(
    axum::extract::State(server): axum::extract::State<Server>,
) -> Result<Json<serde_json::Value>> {
    server
        .db
        .lock()?
        .query_row("SELECT 1", [], |row| row.get::<_, i32>(0))?;
    Ok(Json(json!({"status":"ready"})))
}
async fn protect(
    axum::extract::State(server): axum::extract::State<Server>,
    request: Request,
    next: Next,
) -> Result<Response> {
    if !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) {
        if request
            .headers()
            .get("x-littlewardrobe-request")
            .and_then(|value| value.to_str().ok())
            != Some("1")
        {
            return Err(ApiError(
                StatusCode::FORBIDDEN,
                "The request is missing its same-origin protection header.".into(),
            ));
        }
        if request
            .headers()
            .get("origin")
            .is_some_and(|value| value.to_str().ok() != Some(&server.config.origin))
        {
            return Err(ApiError(
                StatusCode::FORBIDDEN,
                "Requests must come from the configured application origin.".into(),
            ));
        }
        server.rate_limit("all-writes", 2000, 60)?;
        if matches!(
            request.uri().path(),
            "/api/auth/login" | "/api/auth/recover" | "/api/auth/join"
        ) {
            server.rate_limit("authentication", 60, 60)?;
        }
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("same-origin"));
    Ok(response)
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "littlewardrobe_api=info,tower_http=info".into()),
        )
        .init();
    let data_dir =
        PathBuf::from(std::env::var("LITTLEWARDROBE_DATA_DIR").unwrap_or_else(|_| "./data".into()));
    if let Some(destination) = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .as_slice()
        .strip_prefix(&["--backup".to_string()])
        .and_then(|args| args.first())
    {
        let destination = PathBuf::from(destination);
        if destination.exists() {
            return Err("Backup destination already exists; choose a new path".into());
        }
        let conn = rusqlite::Connection::open(data_dir.join("littlewardrobe.sqlite3"))?;
        conn.backup("main", &destination, None)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o600))?;
        }
        tracing::info!("Consistent database backup completed");
        return Ok(());
    }
    std::fs::create_dir_all(&data_dir)?;
    let origin = std::env::var("LITTLEWARDROBE_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:3101".into())
        .trim_end_matches('/')
        .to_string();
    let setup_token = if let Ok(token) = std::env::var("LITTLEWARDROBE_SETUP_TOKEN") {
        if token.len() < 16 {
            return Err("LITTLEWARDROBE_SETUP_TOKEN needs at least 16 characters".into());
        }
        token
    } else {
        let path = data_dir.join("setup-token");
        if path.exists() {
            std::fs::read_to_string(path)?
        } else {
            let token = auth::token();
            std::fs::write(&path, &token)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            }
            tracing::info!(
                "First-use setup token created in the data directory's setup-token file"
            );
            token
        }
    };
    let secure_cookie = origin.starts_with("https://");
    let secret = if let Ok(value) = std::env::var("LITTLEWARDROBE_SECRET_KEY") {
        value
    } else {
        let path = data_dir.join("secret-key");
        if path.exists() {
            std::fs::read_to_string(path)?
        } else {
            let value = auth::token();
            std::fs::write(&path, &value)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            }
            value
        }
    };
    let mut encryption_key = [0u8; 32];
    let secret = secret.trim();
    if secret.len() != 64 || !secret.is_ascii() {
        return Err("LITTLEWARDROBE_SECRET_KEY must be 64 hexadecimal characters".into());
    }
    for (index, byte) in encryption_key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&secret[index * 2..index * 2 + 2], 16)
            .map_err(|_| "LITTLEWARDROBE_SECRET_KEY must be hexadecimal")?;
    }
    let config = Arc::new(Config {
        oidc: oidc::Oidc::from_env(&origin),
        origin,
        secure_cookie,
        setup_token,
        data_dir: data_dir.clone(),
        encryption_key,
    });
    let journal = db::JournalMode::parse(
        &std::env::var("LITTLEWARDROBE_SQLITE_JOURNAL_MODE").unwrap_or_else(|_| "WAL".into()),
    )?;
    let db = db::Database::open_with_journal(&data_dir.join("littlewardrobe.sqlite3"), journal)
        .map_err(|error| error.1)?;
    let (notices, _) = broadcast::channel(128);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let server = Server {
        db,
        config,
        notices,
        dummy_hash: auth::dummy_hash().into(),
        client,
        limits: Arc::new(Mutex::new(HashMap::new())),
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--remove-member") {
        // Maintenance: removes a non-owner account, e.g. one a passkey
        // sign-in created, together with its sessions and provider link.
        let Some(email) = args.get(1) else {
            return Err("Usage: --remove-member EMAIL".into());
        };
        let removed = server.db.lock().map_err(|e| e.1)?.execute(
            "DELETE FROM users WHERE email=?1 AND role='member'",
            [email.trim().to_lowercase()],
        )?;
        println!("Removed {removed} member account(s).");
        return Ok(());
    }
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "--pair-companion" | "--check-companion"))
    {
        let checking = args[0] == "--check-companion";
        let households: Vec<String> = server
            .db
            .lock()
            .map_err(|e| e.1)?
            .prepare("SELECT id FROM households")?
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        let household_arg = if checking {
            args.get(1).filter(|id| *id != "--image")
        } else {
            args.get(3)
        };
        let household = if let Some(id) = household_arg {
            households
                .iter()
                .find(|h| *h == id)
                .ok_or("Unknown household")?
        } else {
            if households.len() != 1 {
                return Err(
                    "Specify a household ID when there is not exactly one household".into(),
                );
            }
            &households[0]
        };
        if checking {
            let result =
                ai::check_companion(&server, household, args.iter().any(|a| a == "--image"))
                    .await
                    .map_err(|e| e.1)?;
            println!("{result}");
            return Ok(());
        }
        let model = args
            .get(1)
            .ok_or("Usage: --pair-companion MODEL REASONING [HOUSEHOLD_ID]")?;
        let reasoning = args.get(2).ok_or("Specify a reasoning effort")?;
        let result =
            ai::provision_companion(&server, household, model, reasoning).map_err(|e| e.1)?;
        println!("{result}");
        return Ok(());
    }
    let bind = std::env::var("LITTLEWARDROBE_BIND").unwrap_or_else(|_| "127.0.0.1:3002".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("Little Wardrobe API listening on {bind}");
    icon_jobs::start(server.clone()).map_err(|error| error.1)?;
    photo_jobs::start(server.clone()).map_err(|error| error.1)?;
    axum::serve(listener, app(server))
        .with_graceful_shutdown(async {
            #[cfg(unix)]
            {
                let mut terminate =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("termination signal handler");
                tokio::select! {_ = tokio::signal::ctrl_c()=>{},_ = terminate.recv()=>{}}
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
        })
        .await?;
    Ok(())
}
