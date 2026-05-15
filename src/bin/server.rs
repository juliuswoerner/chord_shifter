//! Chord Shifter — Axum + SQLite backend server.
//!
//! # Build
//! ```bash
//! cargo build --bin server --features server --release
//! ```
//!
//! # Run
//! ```bash
//! DATABASE_URL=sqlite:chord_shifter.db PORT=8080 ./target/release/server
//! ```
//!
//! # API
//! POST   /api/auth/register          — create account
//! POST   /api/auth/login             — verify credentials, returns user id
//! GET    /api/songs?user_id=<id>     — list songs for a user (+ shared songs)
//! POST   /api/songs                  — create / update a song
//! GET    /api/songs/:id              — load full song
//! DELETE /api/songs/:id              — delete a song
//!
//! Static files are served from ./dist/ (the output of `dx build --release`).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, services::ServeDir};

use chord_shifter::{auth, song};

// ── App state ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    id: i64,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    id: i64,
    username: String,
}

/// Body for both create and update operations.
#[derive(Deserialize)]
struct SaveSongRequest {
    song: song::Song,
    user_id: i64,
}

#[derive(Serialize)]
struct SaveSongResponse {
    id: i64,
}

#[derive(Deserialize)]
struct ListSongsQuery {
    /// Omit or pass 0 to list only the shared demo songs.
    user_id: Option<i64>,
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
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to SQLite database");

    run_migrations(&pool).await;

    let state = AppState { db: pool };

    let api = Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
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
    // Users table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT    NOT NULL UNIQUE,
            password_hash TEXT    NOT NULL
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
) -> Result<Json<RegisterResponse>, (StatusCode, String)> {
    let hash =
        auth::hash_password(&req.password).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (username, password_hash) VALUES (?, ?) RETURNING id",
    )
    .bind(&req.username)
    .bind(&hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;

    Ok(Json(RegisterResponse { id }))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, username, password_hash FROM users WHERE username = ?",
    )
    .bind(&req.username)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Invalid credentials".into()))?;

    if !auth::verify_password(&req.password, &row.2) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".into()));
    }

    Ok(Json(LoginResponse {
        id: row.0,
        username: row.1,
    }))
}

// ── Song handlers ─────────────────────────────────────────────────────────────

async fn list_songs(
    State(state): State<AppState>,
    Query(q): Query<ListSongsQuery>,
) -> Result<Json<Vec<SongListItem>>, (StatusCode, String)> {
    let user_id = q.user_id.unwrap_or(0);

    let rows = sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT id, name, artist, instruments_json
         FROM   songs
         WHERE  user_id = ? OR user_id = 0
         ORDER  BY id",
    )
    .bind(user_id)
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

async fn save_song(
    State(state): State<AppState>,
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

    // Upsert: update existing row if name + artist + user_id already exist.
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM songs WHERE name = ? AND artist = ? AND user_id = ?",
    )
    .bind(&s.name)
    .bind(&s.artist)
    .bind(req.user_id)
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
        .bind(req.user_id)
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

async fn load_song(
    State(state): State<AppState>,
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
         FROM   songs WHERE id = ?",
    )
    .bind(id)
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

async fn delete_song(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    sqlx::query("DELETE FROM songs WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
