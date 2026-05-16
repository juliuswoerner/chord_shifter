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
    /// User id.
    sub: i64,
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

        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("Invalid token: {e}")))?;

        Ok(AuthUser {
            id: data.claims.sub,
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
        sub: user_id,
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

/// Basic email format check.
fn validate_email(email: &str) -> Result<(), String> {
    if email.contains('@') && email.contains('.') {
        Ok(())
    } else {
        Err("Invalid email address.".into())
    }
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

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:chord_shifter.db".to_string());
    let jwt_secret =
        std::env::var("JWT_SECRET").expect("JWT_SECRET environment variable must be set");
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
    if smtp_configured {
        println!("SMTP configured — email verification enabled.");
    } else {
        // Warn if partially configured so operators catch misconfiguration early.
        let any_smtp = smtp_host.is_some()
            || smtp_username.is_some()
            || smtp_password.is_some()
            || smtp_from.is_some();
        if any_smtp {
            eprintln!("Warning: SMTP partially configured (need SMTP_HOST, SMTP_USERNAME, SMTP_PASSWORD, SMTP_FROM). Users will be auto-verified.");
        } else {
            println!("Warning: SMTP not configured. Users will be auto-verified on registration.");
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

    let api = Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/verify/{token}", get(verify_email))
        .route("/songs", get(list_songs).post(save_song))
        .route("/songs/{id}", get(load_song).delete(delete_song));

    let app = Router::new()
        .nest("/api", api)
        // Serve the compiled WASM frontend from ./dist/
        .fallback_service(ServeDir::new("dist"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Chord Shifter server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
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

    // If SMTP is not fully configured, auto-verify so the user can log in immediately.
    // Requires all four fields — a partial config would leave users permanently unverified.
    let smtp_available = state.smtp_host.is_some()
        && state.smtp_username.is_some()
        && state.smtp_password.is_some()
        && state.smtp_from.is_some();
    let verified_flag = if smtp_available { 0 } else { 1 };
    let token_to_store = if smtp_available {
        Some(verification_token.as_str())
    } else {
        None
    };

    sqlx::query(
        "INSERT INTO users
             (email_encrypted, email_hash, email_verified, verification_token, password_hash)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&email_encrypted)
    .bind(&email_hash)
    .bind(verified_flag)
    .bind(token_to_store)
    .bind(&password_hash)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;

    // Best-effort: log but don't fail if the email cannot be sent.
    if smtp_available {
        if let Err(e) = send_verification_email(&state, &email, &verification_token).await {
            eprintln!("Warning: failed to send verification email to {email}: {e}");
        }
    }

    let message = if smtp_available {
        "Registration successful. Please check your email to verify your account."
    } else {
        "Registration successful. You can now log in."
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
    let result = sqlx::query(
        "UPDATE users SET email_verified = 1, verification_token = NULL
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
                    instrument_capos_json = ?
             WHERE  id = ?",
        )
        .bind(&s.key)
        .bind(&parts_json)
        .bind(&instruments_json)
        .bind(&s.vocals_notes)
        .bind(&ip_json)
        .bind(&ic_json)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        id
    } else {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO songs
                (user_id, name, artist, key_name, parts_json, instruments_json,
                 vocals_notes, instrument_parts_json, instrument_capos_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
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
    );

    let row = sqlx::query_as::<_, Row>(
        "SELECT name, artist, key_name, parts_json, instruments_json,
                vocals_notes, instrument_parts_json, instrument_capos_json
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

    Ok(Json(song::Song {
        name: row.0,
        artist: row.1,
        key: row.2,
        parts,
        instruments,
        vocals_notes: row.5,
        instrument_parts,
        instrument_capos,
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
