//! Setlists: an ordered list of a band's songs, with an optional key per entry (roadmap phase 3).
//!
//! A setlist only *references* songs. The key an entry carries is applied when the setlist is
//! rendered; it never edits the song, which is what keeps a setlist's key changes private to
//! that setlist. Every band member can read and edit the band's setlists; deleting one needs its
//! creator or the band owner. Non-members get 404 for everything.

use super::{bands, AppState, AuthUser};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chord_shifter::song::is_note_name;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const MAX_NAME_CHARS: usize = 100;
const MAX_SETLISTS_PER_BAND: i64 = 200;
const MAX_ENTRIES: usize = 200;

type ApiError = (StatusCode, String);

fn internal(e: impl ToString) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn not_found() -> ApiError {
    (StatusCode::NOT_FOUND, "Setlist not found.".into())
}

pub async fn migrate(pool: &SqlitePool) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS setlists (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            band_id    INTEGER NOT NULL,
            name       TEXT    NOT NULL,
            created_by INTEGER NOT NULL,
            version    INTEGER NOT NULL DEFAULT 1
        )",
    )
    .execute(pool)
    .await
    .expect("Failed to create setlists table");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS setlist_entries (
            setlist_id   INTEGER NOT NULL,
            position     INTEGER NOT NULL,
            song_id      INTEGER NOT NULL,
            key_override TEXT,
            PRIMARY KEY (setlist_id, position),
            UNIQUE (setlist_id, song_id)
        )",
    )
    .execute(pool)
    .await
    .expect("Failed to create setlist_entries table");

    sqlx::query("CREATE INDEX IF NOT EXISTS setlists_by_band ON setlists (band_id)")
        .execute(pool)
        .await
        .expect("Failed to index setlists");
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/bands/{id}/setlists",
            get(list_setlists).post(create_setlist),
        )
        .route(
            "/setlists/{id}",
            get(get_setlist).put(save_setlist).delete(delete_setlist),
        )
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct NewSetlist {
    name: String,
}

#[derive(Deserialize)]
struct EntryInput {
    song_id: i64,
    /// Root note the song is played in for this setlist (`"Bb"`), or none for the song's own key.
    #[serde(default)]
    key_override: Option<String>,
}

#[derive(Deserialize)]
struct SaveSetlist {
    name: String,
    entries: Vec<EntryInput>,
    /// The version the client last loaded; a different stored version is rejected with 409.
    #[serde(default)]
    expected_version: Option<i64>,
}

#[derive(Serialize)]
struct Created {
    id: i64,
}

#[derive(Serialize)]
struct Saved {
    version: i64,
}

#[derive(Serialize)]
struct SetlistSummary {
    id: i64,
    name: String,
    songs: i64,
}

#[derive(Serialize)]
struct EntryView {
    song_id: i64,
    name: String,
    artist: String,
    /// The song's own key.
    song_key: String,
    key_override: Option<String>,
}

#[derive(Serialize)]
struct SetlistDetail {
    id: i64,
    band_id: i64,
    name: String,
    version: i64,
    entries: Vec<EntryView>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn clean_name(raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("The setlist name must be 1–{MAX_NAME_CHARS} characters."),
        ));
    }
    Ok(name.to_string())
}

/// The band a setlist belongs to, if the caller is a member of that band. Everything else is 404.
async fn band_of(pool: &SqlitePool, setlist_id: i64, user_id: i64) -> Result<i64, ApiError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT s.band_id FROM setlists s
         JOIN band_members m ON m.band_id = s.band_id AND m.user_id = ?
         WHERE s.id = ?",
    )
    .bind(user_id)
    .bind(setlist_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(not_found)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn list_setlists(
    State(state): State<AppState>,
    user: AuthUser,
    Path(band_id): Path<i64>,
) -> Result<Json<Vec<SetlistSummary>>, ApiError> {
    bands::role_of(&state.db, band_id, user.id).await?;
    let rows = sqlx::query_as::<_, (i64, String, i64)>(
        "SELECT s.id, s.name,
                (SELECT COUNT(*) FROM setlist_entries e
                 JOIN songs g ON g.id = e.song_id WHERE e.setlist_id = s.id)
         FROM   setlists s
         WHERE  s.band_id = ?
         ORDER  BY s.name COLLATE NOCASE, s.id",
    )
    .bind(band_id)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, name, songs)| SetlistSummary { id, name, songs })
            .collect(),
    ))
}

async fn create_setlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(band_id): Path<i64>,
    Json(req): Json<NewSetlist>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    bands::role_of(&state.db, band_id, user.id).await?;
    let name = clean_name(&req.name)?;
    // The limit is part of the insert, so concurrent requests cannot both slip under it.
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO setlists (band_id, name, created_by)
         SELECT ?1, ?2, ?3 WHERE (SELECT COUNT(*) FROM setlists WHERE band_id = ?1) < ?4
         RETURNING id",
    )
    .bind(band_id)
    .bind(&name)
    .bind(user.id)
    .bind(MAX_SETLISTS_PER_BAND)
    .fetch_optional(&state.db)
    .await
    .map_err(internal)?
    .ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        format!("A band can have at most {MAX_SETLISTS_PER_BAND} setlists."),
    ))?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

async fn get_setlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<SetlistDetail>, ApiError> {
    let band_id = band_of(&state.db, id, user.id).await?;
    let (name, version): (String, i64) =
        sqlx::query_as("SELECT name, version FROM setlists WHERE id = ?")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .map_err(internal)?;
    // Entries whose song has been deleted meanwhile are simply not listed.
    let rows = sqlx::query_as::<_, (i64, String, String, String, Option<String>)>(
        "SELECT e.song_id, g.name, g.artist, g.key_name, e.key_override
         FROM   setlist_entries e JOIN songs g ON g.id = e.song_id
         WHERE  e.setlist_id = ?
         ORDER  BY e.position",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;
    Ok(Json(SetlistDetail {
        id,
        band_id,
        name,
        version,
        entries: rows
            .into_iter()
            .map(
                |(song_id, name, artist, song_key, key_override)| EntryView {
                    song_id,
                    name,
                    artist,
                    song_key,
                    key_override,
                },
            )
            .collect(),
    }))
}

/// Replace a setlist's name and its whole ordered entry list (add, remove, reorder and key
/// changes are all one save, so they are atomic and share the version check).
async fn save_setlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<SaveSetlist>,
) -> Result<Json<Saved>, ApiError> {
    let band_id = band_of(&state.db, id, user.id).await?;
    let name = clean_name(&req.name)?;

    if req.entries.len() > MAX_ENTRIES {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("A setlist can have at most {MAX_ENTRIES} songs."),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for entry in &req.entries {
        if !seen.insert(entry.song_id) {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "A song can appear in a setlist only once.".into(),
            ));
        }
        if let Some(key) = &entry.key_override {
            if !is_note_name(key) {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("\"{key}\" is not a note name such as C, F# or Bb."),
                ));
            }
        }
    }

    let mut tx = state.db.begin().await.map_err(internal)?;
    let version = sqlx::query_scalar::<_, i64>(
        "UPDATE setlists SET name = ?, version = version + 1
         WHERE id = ? AND (? IS NULL OR version = ?)
         RETURNING version",
    )
    .bind(&name)
    .bind(id)
    .bind(req.expected_version)
    .bind(req.expected_version)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?
    .ok_or((
        StatusCode::CONFLICT,
        "Someone else changed this setlist since you opened it. Reload it and try again.".into(),
    ))?;

    sqlx::query("DELETE FROM setlist_entries WHERE setlist_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    for (position, entry) in req.entries.iter().enumerate() {
        // Only songs of this band can be listed; the check is part of the insert.
        let inserted = sqlx::query(
            "INSERT INTO setlist_entries (setlist_id, position, song_id, key_override)
             SELECT ?1, ?2, id, ?4 FROM songs WHERE id = ?3 AND band_id = ?5",
        )
        .bind(id)
        .bind(position as i64)
        .bind(entry.song_id)
        .bind(&entry.key_override)
        .bind(band_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?
        .rows_affected();
        if inserted == 0 {
            // Dropping the transaction rolls the whole save back.
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Song {} is not one of this band's songs.", entry.song_id),
            ));
        }
    }
    tx.commit().await.map_err(internal)?;
    Ok(Json(Saved { version }))
}

/// Delete a setlist: its creator or the band owner.
async fn delete_setlist(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let band_id = band_of(&state.db, id, user.id).await?;
    let mut tx = state.db.begin().await.map_err(internal)?;
    let deleted = sqlx::query(
        "DELETE FROM setlists
         WHERE id = ?1 AND (created_by = ?2 OR EXISTS
               (SELECT 1 FROM band_members WHERE band_id = ?3 AND user_id = ?2 AND role = 'owner'))",
    )
    .bind(id)
    .bind(user.id)
    .bind(band_id)
    .execute(&mut *tx)
    .await
    .map_err(internal)?
    .rows_affected();
    if deleted == 0 {
        return Err((
            StatusCode::FORBIDDEN,
            "Only the setlist's creator or the band owner can delete it.".into(),
        ));
    }
    sqlx::query("DELETE FROM setlist_entries WHERE setlist_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::super::api_tests::{register_and_login, send, test_state};
    use super::super::build_app;
    use axum::http::{Method, StatusCode};
    use serde_json::{json, Value};

    struct Fixture {
        app: axum::Router,
        pool: sqlx::SqlitePool,
        owner: String,
        member: String,
        outsider: String,
        band: i64,
    }

    async fn call(
        app: &axum::Router,
        method: Method,
        uri: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let (s, text) = send(app, method, uri, Some(token), body, None).await;
        (
            s,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }

    /// A band (owner + member) and an outsider.
    async fn fixture() -> Fixture {
        let state = test_state().await;
        let pool = state.db.clone();
        let app = build_app(state, false);
        let owner = register_and_login(&app, "owner@example.com").await;
        let member = register_and_login(&app, "member@example.com").await;
        let outsider = register_and_login(&app, "outsider@example.com").await;
        let (_, b) = call(
            &app,
            Method::POST,
            "/api/bands",
            &owner,
            Some(json!({ "name": "Duo" })),
        )
        .await;
        let band = b["id"].as_i64().unwrap();
        call(
            &app,
            Method::POST,
            &format!("/api/bands/{band}/members"),
            &owner,
            Some(json!({ "email": "member@example.com" })),
        )
        .await;
        Fixture {
            app,
            pool,
            owner,
            member,
            outsider,
            band,
        }
    }

    async fn song(f: &Fixture, token: &str, name: &str, band: Option<i64>) -> i64 {
        let mut song = chord_shifter::demo::demo_songs().remove(0);
        song.name = name.to_string();
        song.key = "G Major".to_string();
        let (s, b) = call(
            &f.app,
            Method::POST,
            "/api/songs",
            token,
            Some(json!({ "song": song, "band_id": band })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        b["id"].as_i64().unwrap()
    }

    async fn new_setlist(f: &Fixture, token: &str, name: &str) -> i64 {
        let (s, b) = call(
            &f.app,
            Method::POST,
            &format!("/api/bands/{}/setlists", f.band),
            token,
            Some(json!({ "name": name })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
        b["id"].as_i64().unwrap()
    }

    async fn save(f: &Fixture, token: &str, id: i64, body: Value) -> (StatusCode, Value) {
        call(
            &f.app,
            Method::PUT,
            &format!("/api/setlists/{id}"),
            token,
            Some(body),
        )
        .await
    }

    async fn load(f: &Fixture, token: &str, id: i64) -> (StatusCode, Value) {
        call(
            &f.app,
            Method::GET,
            &format!("/api/setlists/{id}"),
            token,
            None,
        )
        .await
    }

    fn entries(v: &Value) -> Vec<(i64, Option<String>)> {
        v["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e["song_id"].as_i64().unwrap(),
                    e["key_override"].as_str().map(str::to_string),
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn setlist_routes_require_a_token() {
        let f = fixture().await;
        let (s, _) = send(&f.app, Method::GET, "/api/setlists/1", None, None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_new_setlist_is_empty_and_visible_to_every_member() {
        let f = fixture().await;
        let id = new_setlist(&f, &f.member, "  Friday gig ").await;
        for token in [&f.owner, &f.member] {
            let (s, list) = call(
                &f.app,
                Method::GET,
                &format!("/api/bands/{}/setlists", f.band),
                token,
                None,
            )
            .await;
            assert_eq!(s, StatusCode::OK);
            assert_eq!(
                list,
                json!([{ "id": id, "name": "Friday gig", "songs": 0 }])
            );
            let (s, sl) = load(&f, token, id).await;
            assert_eq!(s, StatusCode::OK);
            assert_eq!(sl["band_id"], f.band);
            assert_eq!(sl["version"], 1);
            assert_eq!(sl["entries"], json!([]));
        }
    }

    #[tokio::test]
    async fn outsiders_cannot_see_or_touch_setlists() {
        let f = fixture().await;
        let id = new_setlist(&f, &f.owner, "Private").await;
        let o = &f.outsider;
        let band_uri = format!("/api/bands/{}/setlists", f.band);
        assert_eq!(
            call(&f.app, Method::GET, &band_uri, o, None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &f.app,
                Method::POST,
                &band_uri,
                o,
                Some(json!({ "name": "x" }))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(load(&f, o, id).await.0, StatusCode::NOT_FOUND);
        assert_eq!(
            save(&f, o, id, json!({ "name": "x", "entries": [] }))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &f.app,
                Method::DELETE,
                &format!("/api/setlists/{id}"),
                o,
                None
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn saving_stores_the_order_and_keys_and_bumps_the_version() {
        let f = fixture().await;
        let a = song(&f, &f.owner, "Alpha", Some(f.band)).await;
        let b = song(&f, &f.member, "Beta", Some(f.band)).await;
        let id = new_setlist(&f, &f.owner, "Gig").await;

        let (s, saved) = save(
            &f,
            &f.member,
            id,
            json!({ "name": "Gig 2", "entries": [
                { "song_id": b },
                { "song_id": a, "key_override": "Bb" },
            ], "expected_version": 1 }),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{saved}");
        assert_eq!(saved["version"], 2);

        let (_, sl) = load(&f, &f.owner, id).await;
        assert_eq!(sl["name"], "Gig 2");
        assert_eq!(sl["version"], 2);
        assert_eq!(entries(&sl), [(b, None), (a, Some("Bb".to_string()))]);
        assert_eq!(sl["entries"][1]["name"], "Alpha");
        assert_eq!(sl["entries"][1]["song_key"], "G Major");

        // Reordering is just another save.
        save(
            &f,
            &f.owner,
            id,
            json!({ "name": "Gig 2", "entries": [{ "song_id": a }, { "song_id": b }] }),
        )
        .await;
        let (_, sl) = load(&f, &f.owner, id).await;
        assert_eq!(entries(&sl), [(a, None), (b, None)]);
    }

    #[tokio::test]
    async fn a_key_override_never_changes_the_song() {
        let f = fixture().await;
        let a = song(&f, &f.owner, "Alpha", Some(f.band)).await;
        let id = new_setlist(&f, &f.owner, "Gig").await;
        save(
            &f,
            &f.owner,
            id,
            json!({ "name": "Gig", "entries": [{ "song_id": a, "key_override": "D" }] }),
        )
        .await;
        let (_, stored) = call(
            &f.app,
            Method::GET,
            &format!("/api/songs/{a}"),
            &f.owner,
            None,
        )
        .await;
        assert_eq!(stored["key"], "G Major");
        assert_eq!(stored["version"], 1, "the song was not even saved");
    }

    #[tokio::test]
    async fn a_stale_save_is_rejected_and_changes_nothing() {
        let f = fixture().await;
        let a = song(&f, &f.owner, "Alpha", Some(f.band)).await;
        let id = new_setlist(&f, &f.owner, "Gig").await;
        save(
            &f,
            &f.owner,
            id,
            json!({ "name": "Gig", "entries": [{ "song_id": a }], "expected_version": 1 }),
        )
        .await;

        let (s, _) = save(
            &f,
            &f.member,
            id,
            json!({ "name": "Mine", "entries": [], "expected_version": 1 }),
        )
        .await;
        assert_eq!(s, StatusCode::CONFLICT);
        let (_, sl) = load(&f, &f.owner, id).await;
        assert_eq!(sl["name"], "Gig");
        assert_eq!(entries(&sl), [(a, None)]);
    }

    #[tokio::test]
    async fn invalid_saves_are_rejected_and_leave_the_setlist_untouched() {
        let f = fixture().await;
        let a = song(&f, &f.owner, "Alpha", Some(f.band)).await;
        let personal = song(&f, &f.owner, "Solo", None).await;
        let id = new_setlist(&f, &f.owner, "Gig").await;
        save(
            &f,
            &f.owner,
            id,
            json!({ "name": "Gig", "entries": [{ "song_id": a }] }),
        )
        .await;

        let bad = [
            json!({ "name": "Gig", "entries": [{ "song_id": a }, { "song_id": personal }] }),
            json!({ "name": "Gig", "entries": [{ "song_id": 9999 }] }),
            json!({ "name": "Gig", "entries": [{ "song_id": a }, { "song_id": a }] }),
            json!({ "name": "Gig", "entries": [{ "song_id": a, "key_override": "H" }] }),
            json!({ "name": "", "entries": [] }),
            json!({ "name": "x".repeat(101), "entries": [] }),
        ];
        for body in bad {
            let (s, _) = save(&f, &f.owner, id, body.clone()).await;
            assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        }
        let (_, sl) = load(&f, &f.owner, id).await;
        assert_eq!(
            entries(&sl),
            [(a, None)],
            "a rejected save rolls back completely"
        );
        assert_eq!(sl["version"], 2, "only the one valid save counted");
    }

    #[tokio::test]
    async fn songs_of_another_band_cannot_be_listed() {
        let f = fixture().await;
        let (_, other) = call(
            &f.app,
            Method::POST,
            "/api/bands",
            &f.owner,
            Some(json!({ "name": "Other" })),
        )
        .await;
        let foreign = song(&f, &f.owner, "Foreign", Some(other["id"].as_i64().unwrap())).await;
        let id = new_setlist(&f, &f.owner, "Gig").await;
        let (s, _) = save(
            &f,
            &f.owner,
            id,
            json!({ "name": "Gig", "entries": [{ "song_id": foreign }] }),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn a_deleted_song_drops_out_of_its_setlists() {
        let f = fixture().await;
        let a = song(&f, &f.owner, "Alpha", Some(f.band)).await;
        let b = song(&f, &f.owner, "Beta", Some(f.band)).await;
        let id = new_setlist(&f, &f.owner, "Gig").await;
        save(
            &f,
            &f.owner,
            id,
            json!({ "name": "Gig", "entries": [{ "song_id": a }, { "song_id": b }] }),
        )
        .await;
        let (s, _) = call(
            &f.app,
            Method::DELETE,
            &format!("/api/songs/{a}"),
            &f.owner,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NO_CONTENT);
        let (_, sl) = load(&f, &f.owner, id).await;
        assert_eq!(entries(&sl), [(b, None)]);
        let left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM setlist_entries WHERE song_id = ?")
                .bind(a)
                .fetch_one(&f.pool)
                .await
                .unwrap();
        assert_eq!(left, 0);
    }

    #[tokio::test]
    async fn only_the_creator_or_the_band_owner_can_delete_a_setlist() {
        let f = fixture().await;
        let by_owner = new_setlist(&f, &f.owner, "By owner").await;
        let by_member = new_setlist(&f, &f.member, "By member").await;
        let del = |id: i64| format!("/api/setlists/{id}");
        assert_eq!(
            call(&f.app, Method::DELETE, &del(by_owner), &f.member, None)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(&f.app, Method::DELETE, &del(by_member), &f.member, None)
                .await
                .0,
            StatusCode::NO_CONTENT
        );
        // The band owner can remove a member's setlist.
        let another = new_setlist(&f, &f.member, "Another").await;
        assert_eq!(
            call(&f.app, Method::DELETE, &del(another), &f.owner, None)
                .await
                .0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(load(&f, &f.owner, by_owner).await.0, StatusCode::OK);
    }

    #[tokio::test]
    async fn deleting_a_band_removes_its_setlists_and_entries() {
        let f = fixture().await;
        let a = song(&f, &f.owner, "Alpha", Some(f.band)).await;
        let id = new_setlist(&f, &f.owner, "Gig").await;
        save(
            &f,
            &f.owner,
            id,
            json!({ "name": "Gig", "entries": [{ "song_id": a }] }),
        )
        .await;
        call(
            &f.app,
            Method::DELETE,
            &format!("/api/bands/{}", f.band),
            &f.owner,
            None,
        )
        .await;
        assert_eq!(load(&f, &f.owner, id).await.0, StatusCode::NOT_FOUND);
        for table in ["setlists", "setlist_entries"] {
            let rows: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&f.pool)
                .await
                .unwrap();
            assert_eq!(rows, 0, "{table}");
        }
    }

    #[tokio::test]
    async fn setlists_are_per_band() {
        let f = fixture().await;
        let (_, other) = call(
            &f.app,
            Method::POST,
            "/api/bands",
            &f.member,
            Some(json!({ "name": "Other" })),
        )
        .await;
        let other = other["id"].as_i64().unwrap();
        new_setlist(&f, &f.owner, "Duo gig").await;
        let (_, list) = call(
            &f.app,
            Method::GET,
            &format!("/api/bands/{other}/setlists"),
            &f.member,
            None,
        )
        .await;
        assert_eq!(list, json!([]));
    }

    #[tokio::test]
    async fn concurrent_creation_cannot_exceed_the_band_limit() {
        let f = fixture().await;
        for i in 0..(super::MAX_SETLISTS_PER_BAND - 1) {
            sqlx::query("INSERT INTO setlists (band_id, name, created_by) VALUES (?, ?, ?)")
                .bind(f.band)
                .bind(format!("List {i}"))
                .bind(1_i64) // creator id: only the count matters here
                .execute(&f.pool)
                .await
                .unwrap();
        }
        let uri = format!("/api/bands/{}/setlists", f.band);
        let body = || Some(json!({ "name": "Racer" }));
        let (x, y) = tokio::join!(
            call(&f.app, Method::POST, &uri, &f.owner, body()),
            call(&f.app, Method::POST, &uri, &f.member, body())
        );
        let mut results = [x.0, y.0];
        results.sort();
        assert_eq!(
            results,
            [StatusCode::CREATED, StatusCode::UNPROCESSABLE_ENTITY]
        );
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM setlists WHERE band_id = ?")
            .bind(f.band)
            .fetch_one(&f.pool)
            .await
            .unwrap();
        assert_eq!(count, super::MAX_SETLISTS_PER_BAND);
    }
}
