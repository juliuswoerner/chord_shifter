//! Bands: groups of users who share songs and setlists (roadmap phase 3).
//!
//! A band has one owner and any number of members. Only the owner can rename or delete
//! the band and add or remove members; a member can leave and can set their own default
//! instrument. Non-members get 404 for everything, so band ids do not leak.

use super::{decrypt, email_sha256, AppState, AuthUser};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use chord_shifter::song::Instrument;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const MAX_NAME_CHARS: usize = 100;
const MAX_OWNED_BANDS: i64 = 50;
const MAX_MEMBERS: i64 = 100;

type ApiError = (StatusCode, String);

fn internal(e: impl ToString) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

fn not_found() -> ApiError {
    (StatusCode::NOT_FOUND, "Band not found.".into())
}

pub async fn migrate(pool: &SqlitePool) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bands (
            id       INTEGER PRIMARY KEY AUTOINCREMENT,
            name     TEXT    NOT NULL,
            owner_id INTEGER NOT NULL
        )",
    )
    .execute(pool)
    .await
    .expect("Failed to create bands table");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS band_members (
            band_id            INTEGER NOT NULL,
            user_id            INTEGER NOT NULL,
            role               TEXT    NOT NULL,
            default_instrument TEXT,
            PRIMARY KEY (band_id, user_id)
        )",
    )
    .execute(pool)
    .await
    .expect("Failed to create band_members table");

    sqlx::query("CREATE INDEX IF NOT EXISTS band_members_by_user ON band_members (user_id)")
        .execute(pool)
        .await
        .expect("Failed to index band_members");
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/bands", get(list_bands).post(create_band))
        .route(
            "/bands/{id}",
            get(get_band).put(rename_band).delete(delete_band),
        )
        .route("/bands/{id}/members", post(add_member))
        .route("/bands/{id}/members/{user_id}", delete(remove_member))
        .route("/bands/{id}/me", put(set_default_instrument))
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct BandName {
    name: String,
}

#[derive(Serialize)]
struct BandSummary {
    id: i64,
    name: String,
    /// The caller's role in this band: "owner" or "member".
    role: String,
    members: i64,
}

#[derive(Serialize)]
struct Member {
    user_id: i64,
    email: String,
    role: String,
    default_instrument: Option<Instrument>,
}

#[derive(Serialize)]
struct BandDetail {
    id: i64,
    name: String,
    role: String,
    members: Vec<Member>,
}

#[derive(Deserialize)]
struct AddMember {
    email: String,
    #[serde(default)]
    default_instrument: Option<Instrument>,
}

#[derive(Deserialize)]
struct DefaultInstrument {
    default_instrument: Option<Instrument>,
}

#[derive(Serialize)]
struct Created {
    id: i64,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn clean_name(raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("The band name must be 1–{MAX_NAME_CHARS} characters."),
        ));
    }
    Ok(name.to_string())
}

fn instrument_to_db(i: Option<Instrument>) -> Option<String> {
    i.and_then(|i| serde_json::to_value(i).ok())
        .and_then(|v| v.as_str().map(str::to_string))
}

fn instrument_from_db(s: Option<String>) -> Option<Instrument> {
    s.and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok())
}

/// The caller's role in the band, or 404 if they are not a member.
async fn role_of(pool: &SqlitePool, band_id: i64, user_id: i64) -> Result<String, ApiError> {
    sqlx::query_scalar::<_, String>(
        "SELECT role FROM band_members WHERE band_id = ? AND user_id = ?",
    )
    .bind(band_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(internal)?
    .ok_or_else(not_found)
}

async fn owner_only(pool: &SqlitePool, band_id: i64, user_id: i64) -> Result<(), ApiError> {
    if role_of(pool, band_id, user_id).await? == "owner" {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            "Only the band owner can do that.".into(),
        ))
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn create_band(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<BandName>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    let name = clean_name(&req.name)?;

    let owned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bands WHERE owner_id = ?")
        .bind(user.id)
        .fetch_one(&state.db)
        .await
        .map_err(internal)?;
    if owned >= MAX_OWNED_BANDS {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("You can own at most {MAX_OWNED_BANDS} bands."),
        ));
    }

    let mut tx = state.db.begin().await.map_err(internal)?;
    let id: i64 =
        sqlx::query_scalar("INSERT INTO bands (name, owner_id) VALUES (?, ?) RETURNING id")
            .bind(&name)
            .bind(user.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal)?;
    sqlx::query("INSERT INTO band_members (band_id, user_id, role) VALUES (?, ?, 'owner')")
        .bind(id)
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;

    Ok((StatusCode::CREATED, Json(Created { id })))
}

async fn list_bands(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<BandSummary>>, ApiError> {
    let rows = sqlx::query_as::<_, (i64, String, String, i64)>(
        "SELECT b.id, b.name, m.role,
                (SELECT COUNT(*) FROM band_members c WHERE c.band_id = b.id)
         FROM   bands b JOIN band_members m ON m.band_id = b.id
         WHERE  m.user_id = ?
         ORDER  BY b.name COLLATE NOCASE, b.id",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, role, members)| BandSummary {
                id,
                name,
                role,
                members,
            })
            .collect(),
    ))
}

async fn get_band(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<BandDetail>, ApiError> {
    let role = role_of(&state.db, id, user.id).await?;
    let name: String = sqlx::query_scalar("SELECT name FROM bands WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(internal)?;

    let rows = sqlx::query_as::<_, (i64, String, String, Option<String>)>(
        "SELECT m.user_id, u.email_encrypted, m.role, m.default_instrument
         FROM   band_members m JOIN users u ON u.id = m.user_id
         WHERE  m.band_id = ?
         ORDER  BY (m.role = 'owner') DESC, m.user_id",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(internal)?;

    let mut members = Vec::with_capacity(rows.len());
    for (user_id, email_encrypted, role, instrument) in rows {
        members.push(Member {
            user_id,
            email: decrypt(&state.cipher, &email_encrypted).map_err(internal)?,
            role,
            default_instrument: instrument_from_db(instrument),
        });
    }

    Ok(Json(BandDetail {
        id,
        name,
        role,
        members,
    }))
}

async fn rename_band(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<BandName>,
) -> Result<StatusCode, ApiError> {
    owner_only(&state.db, id, user.id).await?;
    let name = clean_name(&req.name)?;
    sqlx::query("UPDATE bands SET name = ? WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_band(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    owner_only(&state.db, id, user.id).await?;
    let mut tx = state.db.begin().await.map_err(internal)?;
    sqlx::query("DELETE FROM band_members WHERE band_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    sqlx::query("DELETE FROM bands WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Add an existing, verified user to the band by email address.
async fn add_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<AddMember>,
) -> Result<StatusCode, ApiError> {
    owner_only(&state.db, id, user.id).await?;

    let email = req.email.trim().to_lowercase();
    let new_user: i64 =
        sqlx::query_scalar("SELECT id FROM users WHERE email_hash = ? AND email_verified = 1")
            .bind(email_sha256(&email))
            .fetch_optional(&state.db)
            .await
            .map_err(internal)?
            .ok_or((
                StatusCode::NOT_FOUND,
                "No verified account with that email address.".into(),
            ))?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM band_members WHERE band_id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(internal)?;
    if count >= MAX_MEMBERS {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("A band can have at most {MAX_MEMBERS} members."),
        ));
    }

    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO band_members (band_id, user_id, role, default_instrument)
         VALUES (?, ?, 'member', ?)",
    )
    .bind(id)
    .bind(new_user)
    .bind(instrument_to_db(req.default_instrument))
    .execute(&state.db)
    .await
    .map_err(internal)?
    .rows_affected();
    if inserted == 0 {
        return Err((
            StatusCode::CONFLICT,
            "Already a member of this band.".into(),
        ));
    }
    Ok(StatusCode::CREATED)
}

/// Remove a member. The owner can remove anyone but themselves; members can remove themselves.
async fn remove_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, member_id)): Path<(i64, i64)>,
) -> Result<StatusCode, ApiError> {
    let role = role_of(&state.db, id, user.id).await?;
    if role != "owner" && member_id != user.id {
        return Err((
            StatusCode::FORBIDDEN,
            "Only the band owner can remove other members.".into(),
        ));
    }
    let target = role_of(&state.db, id, member_id).await;
    match target.as_deref() {
        Ok("owner") => {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "The owner cannot leave; delete the band instead.".into(),
            ))
        }
        Ok(_) => {}
        Err(_) => return Err((StatusCode::NOT_FOUND, "Member not found.".into())),
    }
    sqlx::query("DELETE FROM band_members WHERE band_id = ? AND user_id = ?")
        .bind(id)
        .bind(member_id)
        .execute(&state.db)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Set (or clear) the caller's own default instrument in this band.
async fn set_default_instrument(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<DefaultInstrument>,
) -> Result<StatusCode, ApiError> {
    role_of(&state.db, id, user.id).await?;
    sqlx::query("UPDATE band_members SET default_instrument = ? WHERE band_id = ? AND user_id = ?")
        .bind(instrument_to_db(req.default_instrument))
        .bind(id)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::super::api_tests::{register_and_login, send, test_state};
    use super::super::build_app;
    use axum::http::{Method, StatusCode};
    use serde_json::{json, Value};

    async fn app_with_users() -> (axum::Router, String, String, String) {
        let app = build_app(test_state().await, false);
        let owner = register_and_login(&app, "owner@example.com").await;
        let member = register_and_login(&app, "member@example.com").await;
        let outsider = register_and_login(&app, "outsider@example.com").await;
        (app, owner, member, outsider)
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

    async fn create(app: &axum::Router, token: &str, name: &str) -> i64 {
        let (s, b) = call(
            app,
            Method::POST,
            "/api/bands",
            token,
            Some(json!({ "name": name })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
        b["id"].as_i64().unwrap()
    }

    async fn add(app: &axum::Router, token: &str, band: i64, email: &str) -> StatusCode {
        call(
            app,
            Method::POST,
            &format!("/api/bands/{band}/members"),
            token,
            Some(json!({ "email": email })),
        )
        .await
        .0
    }

    #[tokio::test]
    async fn band_routes_require_a_token() {
        let app = build_app(test_state().await, false);
        let (s, _) = send(&app, Method::GET, "/api/bands", None, None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn creating_a_band_makes_the_creator_its_owner() {
        let (app, owner, _, _) = app_with_users().await;
        let id = create(&app, &owner, "  The Waltzers ").await;
        let (s, list) = call(&app, Method::GET, "/api/bands", &owner, None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(list[0]["id"], id);
        assert_eq!(list[0]["name"], "The Waltzers");
        assert_eq!(list[0]["role"], "owner");
        assert_eq!(list[0]["members"], 1);
    }

    #[tokio::test]
    async fn band_names_are_validated() {
        let (app, owner, _, _) = app_with_users().await;
        for name in ["", "   ", &"x".repeat(101)] {
            let (s, _) = call(
                &app,
                Method::POST,
                "/api/bands",
                &owner,
                Some(json!({ "name": name })),
            )
            .await;
            assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{name:?}");
        }
    }

    #[tokio::test]
    async fn non_members_cannot_see_or_touch_a_band() {
        let (app, owner, _, outsider) = app_with_users().await;
        let id = create(&app, &owner, "Private").await;
        let uri = format!("/api/bands/{id}");
        assert_eq!(
            call(&app, Method::GET, &uri, &outsider, None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &app,
                Method::PUT,
                &uri,
                &outsider,
                Some(json!({ "name": "Mine" }))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(&app, Method::DELETE, &uri, &outsider, None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            add(&app, &outsider, id, "outsider@example.com").await,
            StatusCode::NOT_FOUND
        );
        let (_, list) = call(&app, Method::GET, "/api/bands", &outsider, None).await;
        assert_eq!(list, json!([]));
    }

    #[tokio::test]
    async fn the_owner_adds_members_who_then_see_the_band() {
        let (app, owner, member, _) = app_with_users().await;
        let id = create(&app, &owner, "Duo").await;
        assert_eq!(
            add(&app, &owner, id, " Member@Example.com ").await,
            StatusCode::CREATED
        );
        assert_eq!(
            add(&app, &owner, id, "member@example.com").await,
            StatusCode::CONFLICT
        );

        let (s, band) = call(
            &app,
            Method::GET,
            &format!("/api/bands/{id}"),
            &member,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(band["role"], "member");
        let emails: Vec<_> = band["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["email"].as_str().unwrap())
            .collect();
        assert_eq!(emails, ["owner@example.com", "member@example.com"]);
    }

    #[tokio::test]
    async fn adding_an_unknown_email_is_a_404() {
        let (app, owner, _, _) = app_with_users().await;
        let id = create(&app, &owner, "Duo").await;
        assert_eq!(
            add(&app, &owner, id, "nobody@example.com").await,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn members_cannot_manage_the_band() {
        let (app, owner, member, _) = app_with_users().await;
        let id = create(&app, &owner, "Duo").await;
        add(&app, &owner, id, "member@example.com").await;
        let uri = format!("/api/bands/{id}");
        assert_eq!(
            call(
                &app,
                Method::PUT,
                &uri,
                &member,
                Some(json!({ "name": "Mine" }))
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(&app, Method::DELETE, &uri, &member, None).await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            add(&app, &member, id, "outsider@example.com").await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(
                &app,
                Method::DELETE,
                &format!("{uri}/members/1"),
                &member,
                None
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn the_owner_renames_and_deletes_the_band() {
        let (app, owner, member, _) = app_with_users().await;
        let id = create(&app, &owner, "Old").await;
        add(&app, &owner, id, "member@example.com").await;
        let uri = format!("/api/bands/{id}");
        assert_eq!(
            call(
                &app,
                Method::PUT,
                &uri,
                &owner,
                Some(json!({ "name": "New" }))
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            call(&app, Method::GET, &uri, &owner, None).await.1["name"],
            "New"
        );
        assert_eq!(
            call(&app, Method::DELETE, &uri, &owner, None).await.0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            call(&app, Method::GET, &uri, &owner, None).await.0,
            StatusCode::NOT_FOUND
        );
        // The membership rows are gone too.
        let (_, list) = call(&app, Method::GET, "/api/bands", &member, None).await;
        assert_eq!(list, json!([]));
    }

    #[tokio::test]
    async fn members_can_leave_but_the_owner_cannot() {
        let (app, owner, member, _) = app_with_users().await;
        let id = create(&app, &owner, "Duo").await;
        add(&app, &owner, id, "member@example.com").await;
        let (_, band) = call(&app, Method::GET, &format!("/api/bands/{id}"), &owner, None).await;
        let owner_id = band["members"][0]["user_id"].as_i64().unwrap();
        let member_id = band["members"][1]["user_id"].as_i64().unwrap();

        let uri = |u: i64| format!("/api/bands/{id}/members/{u}");
        assert_eq!(
            call(&app, Method::DELETE, &uri(owner_id), &owner, None)
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            call(&app, Method::DELETE, &uri(member_id), &member, None)
                .await
                .0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            call(
                &app,
                Method::GET,
                &format!("/api/bands/{id}"),
                &member,
                None
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(&app, Method::DELETE, &uri(member_id), &owner, None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn the_owner_can_remove_a_member() {
        let (app, owner, member, _) = app_with_users().await;
        let id = create(&app, &owner, "Duo").await;
        add(&app, &owner, id, "member@example.com").await;
        let (_, band) = call(&app, Method::GET, &format!("/api/bands/{id}"), &owner, None).await;
        let member_id = band["members"][1]["user_id"].as_i64().unwrap();
        let (s, _) = call(
            &app,
            Method::DELETE,
            &format!("/api/bands/{id}/members/{member_id}"),
            &owner,
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NO_CONTENT);
        let (_, list) = call(&app, Method::GET, "/api/bands", &member, None).await;
        assert_eq!(list, json!([]));
    }

    #[tokio::test]
    async fn each_member_sets_their_own_default_instrument() {
        let (app, owner, member, _) = app_with_users().await;
        let id = create(&app, &owner, "Duo").await;
        add(&app, &owner, id, "member@example.com").await;
        let me = format!("/api/bands/{id}/me");
        assert_eq!(
            call(
                &app,
                Method::PUT,
                &me,
                &member,
                Some(json!({ "default_instrument": "Piano" }))
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        let (_, band) = call(&app, Method::GET, &format!("/api/bands/{id}"), &owner, None).await;
        assert_eq!(band["members"][0]["default_instrument"], Value::Null);
        assert_eq!(band["members"][1]["default_instrument"], "Piano");

        // Clearing works, and an unknown instrument is rejected.
        call(
            &app,
            Method::PUT,
            &me,
            &member,
            Some(json!({ "default_instrument": null })),
        )
        .await;
        let (_, band) = call(&app, Method::GET, &format!("/api/bands/{id}"), &owner, None).await;
        assert_eq!(band["members"][1]["default_instrument"], Value::Null);
        let (s, _) = call(
            &app,
            Method::PUT,
            &me,
            &member,
            Some(json!({ "default_instrument": "Kazoo" })),
        )
        .await;
        assert!(s.is_client_error());
    }

    #[tokio::test]
    async fn the_owner_can_set_a_default_instrument_when_adding() {
        let (app, owner, member, _) = app_with_users().await;
        let id = create(&app, &owner, "Duo").await;
        let (s, _) = call(
            &app,
            Method::POST,
            &format!("/api/bands/{id}/members"),
            &owner,
            Some(json!({ "email": "member@example.com", "default_instrument": "Bass" })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);
        let (_, band) = call(
            &app,
            Method::GET,
            &format!("/api/bands/{id}"),
            &member,
            None,
        )
        .await;
        assert_eq!(band["members"][1]["default_instrument"], "Bass");
    }

    #[tokio::test]
    async fn a_user_can_own_a_limited_number_of_bands() {
        let (app, owner, _, _) = app_with_users().await;
        for i in 0..super::MAX_OWNED_BANDS {
            create(&app, &owner, &format!("Band {i}")).await;
        }
        let (s, _) = call(
            &app,
            Method::POST,
            "/api/bands",
            &owner,
            Some(json!({ "name": "One too many" })),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    }
}
