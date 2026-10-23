// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.

//! Chord Shifter — Axum + SQLite backend server.
//!
//! # Build
//! ```bash
//! cargo build --bin server --no-default-features --features server --release
//! ```
//!
//! # Run (required + optional env vars)
//! ```bash
//! DATABASE_URL=sqlite:///data/chord_shifter.db \
//! JWT_SECRET=<32-byte-hex> \
//! ENCRYPTION_KEY=<64-char-hex-32-bytes> \
//! APP_URL=https://chord-shifter.fly.dev \
//! ./target/release/server
//! ```
//!
//! Optional SMTP env vars (enable email verification emails):
//! `SMTP_HOST`, `SMTP_PORT` (default `587`), `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`
//!
//! # API
//! POST   /api/auth/register         — register with email + password (sends verification email when SMTP is configured)
//! POST   /api/auth/login            — login (email must be verified)
//! GET    /api/auth/verify/:token    — verify email address
//! GET    /api/songs                 — list songs (requires Bearer token)
//! POST   /api/songs                 — save song (requires Bearer token)
//! GET    /api/songs/:id             — load song (requires Bearer token)
//! DELETE /api/songs/:id             — delete song (requires Bearer token)
//!
//! # Security
//! - Passwords hashed with Argon2id
//! - Email addresses encrypted at rest with AES-256-GCM
//! - JWT HS256 tokens, 30-day expiry
//! - Login blocked until email is verified

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use axum::{
    extract::{FromRequestParts, Path, State},
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, services::ServeDir};

use chord_shifter::{auth, song};

#[path = "server/bands.rs"]
mod bands;

// ── App state ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    jwt_secret: String,
    /// AES-256-GCM cipher used to encrypt/decrypt email addresses at rest.
    cipher: Aes256Gcm,
    /// SMTP config — all `None` if SMTP env vars are not set; email verification
    /// is skipped (users are auto-verified) when SMTP is unconfigured.
    smtp_host: Option<String>,
    smtp_port: u16,
    smtp_username: Option<String>,
    smtp_password: Option<String>,
    smtp_from: Option<String>,
    app_url: String,
}

// ── JWT types ─────────────────────────────────────────────────────────────────

/// Payload embedded inside every JWT token.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    /// User id, as a string (JWT `sub` must be a string; jsonwebtoken 9.3 rejects numbers).
    sub: String,
    email: String,
    /// Expiry as a Unix timestamp (seconds).
    exp: usize,
}

/// Axum extractor: reads `Authorization: Bearer <token>`, validates the
/// signature and expiry, and injects the caller's identity into handlers.
struct AuthUser {
    id: i64,
    #[allow(dead_code)]
    email: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Missing Bearer token".into()))?;

        let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_required_spec_claims(&["exp", "sub"]);
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("Invalid token: {e}")))?;

        let id = data.claims.sub.parse::<i64>().map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "Invalid token: bad subject".to_string(),
            )
        })?;
        Ok(AuthUser {
            id,
            email: data.claims.email,
        })
    }
}

/// Mint a JWT token for `user_id` / `email` that expires in 30 days.
fn make_token(user_id: i64, email: &str, secret: &str) -> Result<String, String> {
    let exp = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 30 * 24 * 3600) as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

// ── Encryption helpers ────────────────────────────────────────────────────────

/// Encrypt a plaintext string with AES-256-GCM.
/// Returns `"<nonce_hex>:<ciphertext_hex>"`.
fn encrypt(cipher: &Aes256Gcm, plaintext: &str) -> Result<String, String> {
    use getrandom::getrandom;
    let mut nonce_bytes = [0u8; 12];
    getrandom(&mut nonce_bytes).map_err(|e| format!("RNG error: {e}"))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encrypt error: {e}"))?;
    Ok(format!(
        "{}:{}",
        hex::encode(nonce_bytes),
        hex::encode(ciphertext)
    ))
}

/// Decrypt a `"<nonce_hex>:<ciphertext_hex>"` string produced by `encrypt`.
fn decrypt(cipher: &Aes256Gcm, stored: &str) -> Result<String, String> {
    let (nonce_hex, ct_hex) = stored.split_once(':').ok_or("Invalid encrypted format")?;
    let nonce_bytes = hex::decode(nonce_hex).map_err(|e| e.to_string())?;
    let ciphertext = hex::decode(ct_hex).map_err(|e| e.to_string())?;
    if nonce_bytes.len() != 12 {
        return Err(format!(
            "Invalid nonce length: expected 12 bytes, got {}",
            nonce_bytes.len()
        ));
    }
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| format!("Decrypt error: {e}"))?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if s.len() % 2 != 0 {
            return Err("Hex string must have an even number of characters".into());
        }
        if !s.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
            return Err("Hex string contains non-hex characters".into());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
            .collect()
    }
}

// ── Password validation ───────────────────────────────────────────────────────

/// Returns `Err` with a human-readable message if the password does not meet
/// the minimum requirements: 8+ chars, ≥1 uppercase, ≥1 digit, ≥1 symbol.
fn validate_password_strength(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters long.".into());
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        return Err("Password must contain at least one uppercase letter.".into());
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("Password must contain at least one number.".into());
    }
    if !password
        .chars()
        .any(|c| !c.is_alphanumeric() && c.is_ascii())
    {
        return Err("Password must contain at least one symbol (e.g. !@#$%).".into());
    }
    Ok(())
}

/// Email format check: non-empty local part, exactly one '@', domain with at least one
/// dot that is neither leading nor trailing, alphabetic TLD with 2+ chars, max 254
/// chars (RFC 5321).
fn validate_email(email: &str) -> Result<(), String> {
    if email.len() > 254 {
        return Err("Email address is too long.".into());
    }
    let (local, domain) = email
        .split_once('@')
        .ok_or_else(|| "Invalid email address.".to_string())?;
    if local.is_empty() {
        return Err("Invalid email address.".into());
    }
    if domain.contains('@') {
        return Err("Invalid email address.".into());
    }
    let dot_pos = domain
        .rfind('.')
        .ok_or_else(|| "Invalid email address.".to_string())?;
    let tld = &domain[dot_pos + 1..];
    if dot_pos == 0 || tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("Invalid email address.".into());
    }
    Ok(())
}

// ── Email sending ─────────────────────────────────────────────────────────────

async fn send_verification_email(
    state: &AppState,
    to_email: &str,
    token: &str,
) -> Result<(), String> {
    let (smtp_host, smtp_username, smtp_password, smtp_from) = match (
        state.smtp_host.as_deref(),
        state.smtp_username.as_deref(),
        state.smtp_password.as_deref(),
        state.smtp_from.as_deref(),
    ) {
        (Some(h), Some(u), Some(p), Some(f)) => (h, u, p, f),
        _ => {
            eprintln!("SMTP not configured — skipping verification email to {to_email}");
            return Ok(());
        }
    };
    let verify_url = format!("{}/api/auth/verify/{}", state.app_url, token);

    let body = format!(
        "Welcome to Chord Shifter!\n\nPlease verify your email by clicking the link below:\n\n{verify_url}\n\nIf you did not register, please ignore this email."
    );

    let email = Message::builder()
        .from(
            smtp_from
                .parse()
                .map_err(|e| format!("Invalid from: {e}"))?,
        )
        .to(to_email.parse().map_err(|e| format!("Invalid to: {e}"))?)
        .subject("Verify your Chord Shifter account")
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .map_err(|e| format!("Build email error: {e}"))?;

    let creds = Credentials::new(smtp_username.to_string(), smtp_password.to_string());

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
        .map_err(|e| format!("SMTP relay error: {e}"))?
        .credentials(creds)
        .port(state.smtp_port)
        .build();

    mailer
        .send(email)
        .await
        .map_err(|e| format!("Send error: {e}"))?;

    Ok(())
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterRequest {
    email: String,
    password: String,
}

/// Returned only after successful login.
/// Registration returns a plain message — user must verify their email first.
#[derive(Serialize)]
struct AuthResponse {
    id: i64,
    email: String,
    /// JWT — store this in the frontend and send as `Authorization: Bearer <token>`.
    token: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

/// Song body — `user_id` is intentionally absent; it is derived from the token.
#[derive(Deserialize)]
struct SaveSongRequest {
    song: song::Song,
}

#[derive(Serialize)]
struct SaveSongResponse {
    id: i64,
}

#[derive(Serialize)]
struct SongListItem {
    id: i64,
    name: String,
    artist: String,
    instruments: Vec<song::Instrument>,
}

// ── Router ────────────────────────────────────────────────────────────────────

/// Rate-limit key: the client IP. On Fly.io the proxy sets `Fly-Client-IP` (clients
/// cannot override it there); otherwise the TCP peer address is used. Without the
/// header every user behind the proxy would share one limit.
#[derive(Clone)]
struct ClientIpKeyExtractor;

impl tower_governor::key_extractor::KeyExtractor for ClientIpKeyExtractor {
    type Key = std::net::IpAddr;

    fn extract<T>(
        &self,
        req: &axum::http::Request<T>,
    ) -> Result<Self::Key, tower_governor::GovernorError> {
        req.headers()
            .get("fly-client-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok())
            .or_else(|| {
                req.extensions()
                    .get::<axum::extract::ConnectInfo<SocketAddr>>()
                    .map(|c| c.0.ip())
            })
            .ok_or(tower_governor::GovernorError::UnableToExtractKey)
    }
}

/// Build the full application router. `rate_limit` is switched off in tests, which
/// call the router directly without a peer address.
fn build_app(state: AppState, rate_limit: bool) -> Router {
    let mut auth_routes = Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/verify/{token}", get(verify_email));
    if rate_limit {
        // Burst of 5, then max 1 request every 2 s per client IP.
        let conf = std::sync::Arc::new(
            tower_governor::governor::GovernorConfigBuilder::default()
                .period(std::time::Duration::from_secs(2))
                .burst_size(5)
                .key_extractor(ClientIpKeyExtractor)
                .finish()
                .unwrap(),
        );
        auth_routes = auth_routes.layer(tower_governor::GovernorLayer { config: conf });
    }

    let song_routes = Router::new()
        .route("/songs", get(list_songs).post(save_song))
        .route("/songs/{id}", get(load_song).delete(delete_song));

    let api = auth_routes.merge(song_routes).merge(bands::routes());

    // Lock CORS to the configured APP_URL origin only.
    let allowed_origin = state
        .app_url
        .parse::<axum::http::HeaderValue>()
        .unwrap_or_else(|err| {
            panic!(
                "Invalid APP_URL for CORS allow_origin: {:?} ({err})",
                state.app_url
            )
        });
    let cors = CorsLayer::new()
        .allow_origin(allowed_origin)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ]);

    Router::new()
        .nest("/api", api)
        .fallback_service(ServeDir::new("dist"))
        .layer(cors)
        .with_state(state)
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:chord_shifter.db".to_string());
    let jwt_secret =
        std::env::var("JWT_SECRET").expect("JWT_SECRET environment variable must be set");
    let jwt_secret_bytes = hex::decode(&jwt_secret).expect("JWT_SECRET must be valid hex");
    assert!(
        jwt_secret_bytes.len() == 32,
        "JWT_SECRET must be 32 bytes (64 hex chars)"
    );
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    // AES-256-GCM key: 64 hex chars = 32 bytes.
    let encryption_key_hex = std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set");
    let key_bytes = hex::decode(&encryption_key_hex).expect("ENCRYPTION_KEY must be valid hex");
    assert!(
        key_bytes.len() == 32,
        "ENCRYPTION_KEY must be 32 bytes (64 hex chars)"
    );
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));

    let smtp_host = std::env::var("SMTP_HOST").ok();
    let smtp_port: u16 = std::env::var("SMTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(587);
    let smtp_username = std::env::var("SMTP_USERNAME").ok();
    let smtp_password = std::env::var("SMTP_PASSWORD").ok();
    let smtp_from = std::env::var("SMTP_FROM").ok();
    // All four fields must be present for SMTP to be considered configured.
    // A partial configuration would leave users stuck as unverified with no way to
    // receive a verification email.
    let smtp_configured = smtp_host.is_some()
        && smtp_username.is_some()
        && smtp_password.is_some()
        && smtp_from.is_some();
    let app_url = std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let is_local = app_url.contains("localhost") || app_url.contains("127.0.0.1");
    if is_local {
        println!("Running in local mode — email verification skipped.");
    } else if smtp_configured {
        println!("SMTP configured — email verification enabled.");
    } else {
        // Warn if partially configured so operators catch misconfiguration early.
        let any_smtp = smtp_host.is_some()
            || smtp_username.is_some()
            || smtp_password.is_some()
            || smtp_from.is_some();
        if any_smtp {
            eprintln!("Warning: SMTP partially configured (need SMTP_HOST, SMTP_USERNAME, SMTP_PASSWORD, SMTP_FROM). Registration will be rejected until fixed.");
        } else {
            eprintln!("Warning: SMTP not configured. Registration will be rejected on non-local deployments.");
        }
    }

    let connect_opts = database_url
        .parse::<SqliteConnectOptions>()
        .expect("Invalid DATABASE_URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_opts)
        .await
        .expect("Failed to connect to SQLite database");

    // Enable WAL mode for better concurrent read performance.
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await
        .expect("Failed to enable WAL mode");

    run_migrations(&pool).await;

    let state = AppState {
        db: pool,
        jwt_secret,
        cipher,
        smtp_host,
        smtp_port,
        smtp_username,
        smtp_password,
        smtp_from,
        app_url,
    };

    let app = build_app(state, true);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Chord Shifter server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    // `ConnectInfo` gives the rate limiter the peer address when no proxy header is present.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

// ── Migrations ────────────────────────────────────────────────────────────────

async fn run_migrations(pool: &SqlitePool) {
    // Detect the old schema (has `username` column, lacks `email_hash`) and
    // drop it so we start clean. Existing users will need to re-register.
    // This only applies to the first deploy after the auth rewrite.
    let has_old_schema = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'username'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0)
        > 0;

    if has_old_schema {
        eprintln!("Detected old username-based users table — dropping and recreating with new email schema.");
        sqlx::query("DROP TABLE IF EXISTS users")
            .execute(pool)
            .await
            .expect("Failed to drop old users table");
    }

    // Users table: email is stored AES-256-GCM encrypted; email_hash (SHA-256)
    // is stored in plain for fast O(1) lookups without decrypting every row.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            email_encrypted    TEXT    NOT NULL,
            email_hash         TEXT    NOT NULL UNIQUE,
            email_verified     INTEGER NOT NULL DEFAULT 0,
            verification_token TEXT,
            password_hash      TEXT    NOT NULL
        )",
    )
    .execute(pool)
    .await
    .expect("Failed to create users table");

    // Idempotent: add verification_token_expires_at to existing deployments.
    match sqlx::query("ALTER TABLE users ADD COLUMN verification_token_expires_at INTEGER")
        .execute(pool)
        .await
    {
        Ok(_) => {}
        Err(sqlx::Error::Database(db_err))
            if db_err
                .message()
                .contains("duplicate column name: verification_token_expires_at") => {}
        Err(err) => {
            panic!("Failed to add users.verification_token_expires_at column: {err}");
        }
    }

    // Songs table — JSON blobs mirror the localStorage schema so the frontend
    // can be switched between backends without data-model changes.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS songs (
            id                    INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id               INTEGER NOT NULL DEFAULT 0,
            name                  TEXT    NOT NULL,
            artist                TEXT    NOT NULL,
            key_name              TEXT    NOT NULL,
            parts_json            TEXT    NOT NULL DEFAULT '[]',
            instruments_json      TEXT    NOT NULL DEFAULT '[]',
            vocals_notes          TEXT    NOT NULL DEFAULT '',
            instrument_parts_json TEXT    NOT NULL DEFAULT '{}',
            instrument_capos_json TEXT    NOT NULL DEFAULT '{}'
        )",
    )
    .execute(pool)
    .await
    .expect("Failed to create songs table");

    // Idempotent: per-sheet PDF font sizes were not persisted by earlier versions.
    match sqlx::query("ALTER TABLE songs ADD COLUMN pdf_settings_json TEXT NOT NULL DEFAULT '{}'")
        .execute(pool)
        .await
    {
        Ok(_) => {}
        Err(sqlx::Error::Database(db_err))
            if db_err
                .message()
                .contains("duplicate column name: pdf_settings_json") => {}
        Err(err) => {
            panic!("Failed to add songs.pdf_settings_json column: {err}");
        }
    }

    bands::migrate(pool).await;
}

// ── Auth handlers ─────────────────────────────────────────────────────────────

async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let email = req.email.trim().to_lowercase();
    validate_email(&email).map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
    validate_password_strength(&req.password).map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;

    let email_hash = email_sha256(&email);

    // Reject duplicate registrations.
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE email_hash = ?")
        .bind(&email_hash)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if exists.is_some() {
        return Err((StatusCode::CONFLICT, "Email already registered.".into()));
    }

    let email_encrypted =
        encrypt(&state.cipher, &email).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let password_hash =
        auth::hash_password(&req.password).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Random 32-byte hex verification token.
    let mut token_bytes = [0u8; 32];
    getrandom::getrandom(&mut token_bytes)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("RNG error: {e}")))?;
    let verification_token = hex::encode(token_bytes);

    // Auto-verify only for local development (APP_URL points to localhost).
    // In all other environments, email verification is always required — even if
    // SMTP is misconfigured, we'd rather surface a clear error than silently skip verification.
    let is_local = state.app_url.contains("localhost") || state.app_url.contains("127.0.0.1");
    let smtp_available = state.smtp_host.is_some()
        && state.smtp_username.is_some()
        && state.smtp_password.is_some()
        && state.smtp_from.is_some();

    if !is_local && !smtp_available {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Registration is temporarily unavailable: email verification cannot be sent. Please try again later or contact support.".into(),
        ));
    }

    let verified_flag = if is_local { 1 } else { 0 };
    let token_to_store = if is_local {
        None
    } else {
        Some(verification_token.as_str())
    };
    // Verification tokens expire after 24 hours.
    let token_expiry: Option<i64> = if is_local {
        None
    } else {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        Some(secs + 24 * 3600)
    };

    sqlx::query(
        "INSERT INTO users
             (email_encrypted, email_hash, email_verified, verification_token,
              verification_token_expires_at, password_hash)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&email_encrypted)
    .bind(&email_hash)
    .bind(verified_flag)
    .bind(token_to_store)
    .bind(token_expiry)
    .bind(&password_hash)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;

    // Best-effort: log but don't fail if the email cannot be sent.
    if !is_local {
        if let Err(e) = send_verification_email(&state, &email, &verification_token).await {
            eprintln!("Warning: failed to send verification email to {email}: {e}");
        }
    }

    let message = if is_local {
        "Registration successful. You can now log in."
    } else {
        "Registration successful. Please check your email to verify your account."
    };
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "message": message })),
    ))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let email = req.email.trim().to_lowercase();
    let email_hash = email_sha256(&email);

    let row = sqlx::query_as::<_, (i64, String, i64, String)>(
        "SELECT id, email_encrypted, email_verified, password_hash
         FROM   users WHERE email_hash = ?",
    )
    .bind(&email_hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Invalid credentials.".into()))?;

    let (user_id, email_encrypted, email_verified, password_hash) = row;

    if email_verified == 0 {
        return Err((
            StatusCode::FORBIDDEN,
            "Please verify your email address before logging in.".into(),
        ));
    }

    if !auth::verify_password(&req.password, &password_hash) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials.".into()));
    }

    let stored_email = decrypt(&state.cipher, &email_encrypted)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let token = make_token(user_id, &stored_email, &state.jwt_secret)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(AuthResponse {
        id: user_id,
        email: stored_email,
        token,
    }))
}

async fn verify_email(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Look up the token first so we can check its expiry.
    let row = sqlx::query_as::<_, (i64, Option<i64>)>(
        "SELECT id, verification_token_expires_at FROM users WHERE verification_token = ?",
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            "Invalid verification token. It may have already been used.".to_string(),
        )
    })?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    if row.1.is_none_or(|expires_at| now > expires_at) {
        // Remove the expired unverified account so the address can be re-registered.
        let _ = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(row.0)
            .execute(&state.db)
            .await;
        return Err((
            StatusCode::GONE,
            "Verification token has expired. Please register again.".into(),
        ));
    }

    let result = sqlx::query(
        "UPDATE users
         SET    email_verified = 1,
                verification_token = NULL,
                verification_token_expires_at = NULL
         WHERE  verification_token = ?",
    )
    .bind(&token)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            "Invalid verification token. It may have already been used.".into(),
        ));
    }

    Ok(Json(serde_json::json!({
        "message": "Email verified successfully. You can now log in."
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verify_email_treats_null_expiry_as_expired() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");
        run_migrations(&pool).await;

        let key_bytes = [0u8; 32];
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
        let email = "legacy@example.com";
        let encrypted_email = encrypt(&cipher, email).expect("Failed to encrypt email");

        sqlx::query(
            "INSERT INTO users
             (email_encrypted, email_hash, email_verified, verification_token,
              verification_token_expires_at, password_hash)
             VALUES (?, ?, 0, ?, NULL, ?)",
        )
        .bind(encrypted_email)
        .bind(email_sha256(email))
        .bind("legacy-token")
        .bind("password-hash")
        .execute(&pool)
        .await
        .expect("Failed to insert legacy user");

        let state = AppState {
            db: pool.clone(),
            jwt_secret: "0".repeat(64),
            cipher,
            smtp_host: None,
            smtp_port: 587,
            smtp_username: None,
            smtp_password: None,
            smtp_from: None,
            app_url: "http://localhost:8080".into(),
        };

        let result = verify_email(State(state), Path("legacy-token".to_string())).await;

        match result {
            Err((status, message)) => {
                assert_eq!(status, StatusCode::GONE);
                assert_eq!(
                    message,
                    "Verification token has expired. Please register again."
                );
            }
            Ok(_) => panic!("Expected legacy token with NULL expiry to be rejected"),
        }
        let remaining_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await
            .expect("Failed to count users");
        assert_eq!(remaining_users, 0);
    }
}

/// SHA-256 of the lowercased email address, hex-encoded, used as a lookup key.
fn email_sha256(email: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(email.as_bytes()))
}

// ── Song handlers ─────────────────────────────────────────────────────────────

/// List the authenticated user's songs, plus shared demo songs (user_id = 0).
async fn list_songs(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<SongListItem>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT id, name, artist, instruments_json
         FROM   songs
         WHERE  user_id = ? OR user_id = 0
         ORDER  BY id",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items = rows
        .into_iter()
        .map(|(id, name, artist, instruments_json)| {
            let instruments = serde_json::from_str(&instruments_json).unwrap_or_default();
            SongListItem {
                id,
                name,
                artist,
                instruments,
            }
        })
        .collect();

    Ok(Json(items))
}

/// Create or update a song. The owner is always the authenticated user.
async fn save_song(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<SaveSongRequest>,
) -> Result<Json<SaveSongResponse>, (StatusCode, String)> {
    let s = &req.song;

    let parts_json = serde_json::to_string(&s.parts)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let instruments_json = serde_json::to_string(&s.instruments)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let ip_json = serde_json::to_string(&s.instrument_parts)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let ic_json = serde_json::to_string(&s.instrument_capos)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let pdf_json = serde_json::to_string(&s.pdf_settings)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Upsert: update if name + artist already exist for this user.
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM songs WHERE name = ? AND artist = ? AND user_id = ?",
    )
    .bind(&s.name)
    .bind(&s.artist)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let id = if let Some(id) = existing {
        sqlx::query(
            "UPDATE songs
             SET    key_name              = ?,
                    parts_json            = ?,
                    instruments_json      = ?,
                    vocals_notes          = ?,
                    instrument_parts_json = ?,
                    instrument_capos_json = ?,
                    pdf_settings_json     = ?
             WHERE  id = ?",
        )
        .bind(&s.key)
        .bind(&parts_json)
        .bind(&instruments_json)
        .bind(&s.vocals_notes)
        .bind(&ip_json)
        .bind(&ic_json)
        .bind(&pdf_json)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        id
    } else {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO songs
                (user_id, name, artist, key_name, parts_json, instruments_json,
                 vocals_notes, instrument_parts_json, instrument_capos_json,
                 pdf_settings_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id",
        )
        .bind(user.id)
        .bind(&s.name)
        .bind(&s.artist)
        .bind(&s.key)
        .bind(&parts_json)
        .bind(&instruments_json)
        .bind(&s.vocals_notes)
        .bind(&ip_json)
        .bind(&ic_json)
        .bind(&pdf_json)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(SaveSongResponse { id }))
}

/// Load a song. Allowed if the song belongs to the caller or is a shared demo (user_id = 0).
async fn load_song(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<song::Song>, (StatusCode, String)> {
    type Row = (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    );

    let row = sqlx::query_as::<_, Row>(
        "SELECT name, artist, key_name, parts_json, instruments_json,
                vocals_notes, instrument_parts_json, instrument_capos_json,
                pdf_settings_json
         FROM   songs
         WHERE  id = ? AND (user_id = ? OR user_id = 0)",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Song {id} not found")))?;

    let parts = serde_json::from_str(&row.3)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let instruments = serde_json::from_str(&row.4).unwrap_or_default();
    let instrument_parts = serde_json::from_str(&row.6).unwrap_or_default();
    let instrument_capos = serde_json::from_str(&row.7).unwrap_or_default();
    let pdf_settings = serde_json::from_str(&row.8).unwrap_or_default();

    Ok(Json(song::Song {
        name: row.0,
        artist: row.1,
        key: row.2,
        parts,
        instruments,
        vocals_notes: row.5,
        instrument_parts,
        instrument_capos,
        pdf_settings,
    }))
}

/// Delete a song. Only the owner can delete; shared demo songs (user_id = 0) are protected.
async fn delete_song(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let affected = sqlx::query("DELETE FROM songs WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .rows_affected();

    if affected == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Song {id} not found or not owned by you"),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod validation_tests {
    use super::validate_email;

    #[test]
    fn validate_email_accepts_basic_address() {
        assert!(validate_email("user@example.com").is_ok());
    }

    #[test]
    fn validate_email_rejects_multiple_at_signs() {
        assert!(validate_email("a@b@c.com").is_err());
    }

    #[test]
    fn validate_email_rejects_short_or_non_alphabetic_tld() {
        assert!(validate_email("a@b.c").is_err());
        assert!(validate_email("a@b.c0m").is_err());
    }
}

#[cfg(test)]
pub(crate) mod api_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Method, Request, StatusCode};
    use tower::ServiceExt;

    const PASSWORD: &str = "Str0ng-Passw0rd!";

    pub(super) async fn test_state() -> AppState {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");
        run_migrations(&pool).await;
        AppState {
            db: pool,
            jwt_secret: hex::encode([7u8; 32]),
            cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&[0u8; 32])),
            smtp_host: None,
            smtp_port: 587,
            smtp_username: None,
            smtp_password: None,
            smtp_from: None,
            // localhost = local mode: users are verified automatically.
            app_url: "http://localhost:8080".to_string(),
        }
    }

    pub(super) async fn send(
        app: &Router,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
        extra_header: Option<(&str, &str)>,
    ) -> (StatusCode, String) {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        if let Some((k, v)) = extra_header {
            req = req.header(k, v);
        }
        let req = match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    fn creds(email: &str) -> serde_json::Value {
        serde_json::json!({ "email": email, "password": PASSWORD })
    }

    #[tokio::test]
    async fn register_then_login_returns_a_token() {
        let app = build_app(test_state().await, false);
        let (s, _) = send(
            &app,
            Method::POST,
            "/api/auth/register",
            None,
            Some(creds("a@example.com")),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);
        let (s, body) = send(
            &app,
            Method::POST,
            "/api/auth/login",
            None,
            Some(creds("a@example.com")),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(v["token"].as_str().is_some_and(|t| !t.is_empty()));
        assert_eq!(v["email"], "a@example.com");
    }

    #[tokio::test]
    async fn duplicate_registration_is_rejected() {
        let app = build_app(test_state().await, false);
        send(
            &app,
            Method::POST,
            "/api/auth/register",
            None,
            Some(creds("a@example.com")),
            None,
        )
        .await;
        let (s, _) = send(
            &app,
            Method::POST,
            "/api/auth/register",
            None,
            Some(creds("A@Example.com")),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn wrong_password_is_unauthorized() {
        let app = build_app(test_state().await, false);
        send(
            &app,
            Method::POST,
            "/api/auth/register",
            None,
            Some(creds("a@example.com")),
            None,
        )
        .await;
        let bad = serde_json::json!({ "email": "a@example.com", "password": "nope" });
        let (s, _) = send(&app, Method::POST, "/api/auth/login", None, Some(bad), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn weak_password_is_rejected() {
        let app = build_app(test_state().await, false);
        let weak = serde_json::json!({ "email": "a@example.com", "password": "short" });
        let (s, _) = send(
            &app,
            Method::POST,
            "/api/auth/register",
            None,
            Some(weak),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn song_routes_require_a_token() {
        let app = build_app(test_state().await, false);
        let (s, _) = send(&app, Method::GET, "/api/songs", None, None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        let (s, _) = send(
            &app,
            Method::GET,
            "/api/songs",
            Some("not-a-token"),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }

    // Regression: with the rate limiter on, auth routes answered 500 "Unable To Extract Key!"
    // because no client address was available.
    #[tokio::test]
    async fn rate_limited_auth_routes_work_with_a_proxy_header() {
        let app = build_app(test_state().await, true);
        let hdr = Some(("fly-client-ip", "203.0.113.9"));
        let (s, body) = send(
            &app,
            Method::POST,
            "/api/auth/register",
            None,
            Some(creds("a@example.com")),
            hdr,
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{body}");
    }

    #[tokio::test]
    async fn rate_limiter_works_with_the_peer_address() {
        let app = build_app(test_state().await, true);
        let mut req = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(creds("nobody@example.com").to_string()))
            .unwrap();
        req.extensions_mut()
            .insert(axum::extract::ConnectInfo(SocketAddr::from((
                [127, 0, 0, 1],
                5555,
            ))));
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED); // reached the handler
    }

    #[tokio::test]
    async fn rate_limit_is_per_client_ip() {
        let app = build_app(test_state().await, true);
        let login = |ip: &'static str| {
            let app = app.clone();
            async move {
                send(
                    &app,
                    Method::POST,
                    "/api/auth/login",
                    None,
                    Some(creds("nobody@example.com")),
                    Some(("fly-client-ip", ip)),
                )
                .await
                .0
            }
        };
        // Burst of 5 passes (401 = reached the handler), the 6th from the same IP is limited.
        for _ in 0..5 {
            assert_eq!(login("198.51.100.1").await, StatusCode::UNAUTHORIZED);
        }
        assert_eq!(login("198.51.100.1").await, StatusCode::TOO_MANY_REQUESTS);
        // Another client is unaffected.
        assert_eq!(login("198.51.100.2").await, StatusCode::UNAUTHORIZED);
    }

    // ── Songs ────────────────────────────────────────────────────────────────

    pub(super) async fn register_and_login(app: &Router, email: &str) -> String {
        send(
            app,
            Method::POST,
            "/api/auth/register",
            None,
            Some(creds(email)),
            None,
        )
        .await;
        let (s, body) = send(
            app,
            Method::POST,
            "/api/auth/login",
            None,
            Some(creds(email)),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{body}");
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn song_body(name: &str) -> serde_json::Value {
        let mut song = chord_shifter::demo::demo_songs().remove(0);
        song.name = name.to_string();
        serde_json::json!({ "song": song })
    }

    async fn save(app: &Router, token: &str, body: serde_json::Value) -> i64 {
        let (s, resp) = send(
            app,
            Method::POST,
            "/api/songs",
            Some(token),
            Some(body),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{resp}");
        serde_json::from_str::<serde_json::Value>(&resp).unwrap()["id"]
            .as_i64()
            .unwrap()
    }

    #[tokio::test]
    async fn every_demo_song_round_trips_with_all_its_fields() {
        // The demo songs cover chords, tabs, drums, per-instrument sheets, capos, PDF font
        // sizes and a piano sheet (notes, note lengths and time signature).
        let app = build_app(test_state().await, false);
        let token = register_and_login(&app, "a@example.com").await;
        let demo = chord_shifter::demo::demo_songs();
        assert!(demo.iter().any(|s| !s.pdf_settings.is_empty()));
        assert!(demo.iter().any(|s| s
            .instrument_parts
            .values()
            .flatten()
            .any(|p| p.kind == song::PartKind::PianoRiff && p.time_sig == "3/4")));
        for original in demo {
            let id = save(&app, &token, serde_json::json!({ "song": original })).await;
            let (s, body) = send(
                &app,
                Method::GET,
                &format!("/api/songs/{id}"),
                Some(&token),
                None,
                None,
            )
            .await;
            assert_eq!(s, StatusCode::OK, "{body}");
            let loaded: song::Song = serde_json::from_str(&body).unwrap();
            assert_eq!(loaded, original, "{}", original.name);
        }
    }

    #[tokio::test]
    async fn saving_the_same_name_and_artist_updates_instead_of_duplicating() {
        let app = build_app(test_state().await, false);
        let token = register_and_login(&app, "a@example.com").await;
        let first = save(&app, &token, song_body("Tune")).await;
        let mut changed = song_body("Tune");
        changed["song"]["key"] = serde_json::json!("G Major");
        let second = save(&app, &token, changed).await;
        assert_eq!(first, second);
        let (_, list) = send(&app, Method::GET, "/api/songs", Some(&token), None, None).await;
        let items: Vec<serde_json::Value> = serde_json::from_str(&list).unwrap();
        assert_eq!(items.len(), 1);
        let (_, body) = send(
            &app,
            Method::GET,
            &format!("/api/songs/{first}"),
            Some(&token),
            None,
            None,
        )
        .await;
        assert_eq!(
            serde_json::from_str::<song::Song>(&body).unwrap().key,
            "G Major"
        );
    }

    #[tokio::test]
    async fn users_cannot_see_or_delete_each_others_songs() {
        let app = build_app(test_state().await, false);
        let alice = register_and_login(&app, "alice@example.com").await;
        let bob = register_and_login(&app, "bob@example.com").await;
        let id = save(&app, &alice, song_body("Private")).await;

        let (_, list) = send(&app, Method::GET, "/api/songs", Some(&bob), None, None).await;
        assert_eq!(
            serde_json::from_str::<Vec<serde_json::Value>>(&list)
                .unwrap()
                .len(),
            0
        );
        let (s, _) = send(
            &app,
            Method::GET,
            &format!("/api/songs/{id}"),
            Some(&bob),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        let (s, _) = send(
            &app,
            Method::DELETE,
            &format!("/api/songs/{id}"),
            Some(&bob),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        // Still there for the owner.
        let (s, _) = send(
            &app,
            Method::GET,
            &format!("/api/songs/{id}"),
            Some(&alice),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
    }

    #[tokio::test]
    async fn deleting_a_song_removes_it() {
        let app = build_app(test_state().await, false);
        let token = register_and_login(&app, "a@example.com").await;
        let id = save(&app, &token, song_body("Gone")).await;
        let (s, _) = send(
            &app,
            Method::DELETE,
            &format!("/api/songs/{id}"),
            Some(&token),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NO_CONTENT);
        let (s, _) = send(
            &app,
            Method::GET,
            &format!("/api/songs/{id}"),
            Some(&token),
            None,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }

    // A database created before pdf_settings was stored must be upgraded in place.
    #[tokio::test]
    async fn migration_adds_the_pdf_settings_column_to_an_old_songs_table() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE songs (
                id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER NOT NULL DEFAULT 0,
                name TEXT NOT NULL, artist TEXT NOT NULL, key_name TEXT NOT NULL,
                parts_json TEXT NOT NULL DEFAULT '[]', instruments_json TEXT NOT NULL DEFAULT '[]',
                vocals_notes TEXT NOT NULL DEFAULT '', instrument_parts_json TEXT NOT NULL DEFAULT '{}',
                instrument_capos_json TEXT NOT NULL DEFAULT '{}')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO songs (user_id, name, artist, key_name) VALUES (1, 'Old', 'Me', 'C Major')")
            .execute(&pool)
            .await
            .unwrap();
        run_migrations(&pool).await;
        run_migrations(&pool).await; // idempotent
        let json: String =
            sqlx::query_scalar("SELECT pdf_settings_json FROM songs WHERE name = 'Old'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(json, "{}");
    }

    fn token_with(sub: &str, exp: usize, secret: &str) -> String {
        encode(
            &Header::default(),
            &Claims {
                sub: sub.to_string(),
                email: "a@example.com".to_string(),
                exp,
            },
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn expired_malformed_and_foreign_tokens_are_rejected() {
        let state = test_state().await;
        let secret = state.jwt_secret.clone();
        let app = build_app(state, false);
        let far_future = 4_000_000_000usize;
        // Sanity: a well-formed token for this secret is accepted.
        let ok = token_with("1", far_future, &secret);
        let (s, _) = send(&app, Method::GET, "/api/songs", Some(&ok), None, None).await;
        assert_eq!(s, StatusCode::OK);
        for (label, token) in [
            ("expired", token_with("1", 1_000_000_000, &secret)),
            (
                "non-numeric subject",
                token_with("abc", far_future, &secret),
            ),
            (
                "wrong secret",
                token_with("1", far_future, "another-secret"),
            ),
        ] {
            let (s, _) = send(&app, Method::GET, "/api/songs", Some(&token), None, None).await;
            assert_eq!(s, StatusCode::UNAUTHORIZED, "{label}");
        }
    }
}
