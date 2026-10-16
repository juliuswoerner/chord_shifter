// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.

//! HTTP client for the Axum server (`src/bin/server.rs`, routes under `/api`).
//!
//! The browser calls are thin (`fetch`); the request bodies and response parsing
//! are plain functions so they can be unit-tested natively.

use chord_shifter::song::{Instrument, Song};
use serde::Deserialize;

/// Result of `POST /api/auth/login`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LoginInfo {
    pub id: i64,
    pub email: String,
    pub token: String,
}

/// One entry of `GET /api/songs`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SongListItem {
    pub id: i64,
    pub name: String,
    pub artist: String,
    #[serde(default)]
    pub instruments: Vec<Instrument>,
}

/// Outcome of a registration request.
#[derive(Debug, Clone, PartialEq)]
pub struct RegisterInfo {
    /// Server message, e.g. "Registration successful. Please check your email…".
    pub message: String,
}

// ── Pure helpers (tested natively) ───────────────────────────────────────────

/// Turn a non-2xx response into a message for the user.
pub fn error_message(status: u16, body: &str) -> String {
    let body = body.trim();
    if !body.is_empty() && body.len() < 300 && !body.starts_with('<') {
        return body.to_string();
    }
    match status {
        401 => "Invalid email or password.".to_string(),
        403 => "Please verify your email address before logging in.".to_string(),
        429 => "Too many attempts. Please wait a moment and try again.".to_string(),
        s => format!("Request failed (HTTP {s})."),
    }
}

fn ok_or_error(status: u16, body: String) -> Result<String, String> {
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(error_message(status, &body))
    }
}

pub fn parse_login(status: u16, body: String) -> Result<LoginInfo, String> {
    let body = ok_or_error(status, body)?;
    serde_json::from_str(&body).map_err(|e| format!("Unexpected login response: {e}"))
}

pub fn parse_register(status: u16, body: String) -> Result<RegisterInfo, String> {
    let body = ok_or_error(status, body)?;
    let message = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| "Registration successful.".to_string());
    Ok(RegisterInfo { message })
}

pub fn parse_song_list(status: u16, body: String) -> Result<Vec<SongListItem>, String> {
    let body = ok_or_error(status, body)?;
    serde_json::from_str(&body).map_err(|e| format!("Unexpected song list: {e}"))
}

pub fn parse_song(status: u16, body: String) -> Result<Song, String> {
    let body = ok_or_error(status, body)?;
    serde_json::from_str(&body).map_err(|e| format!("Unexpected song: {e}"))
}

pub fn parse_saved_id(status: u16, body: String) -> Result<i64, String> {
    let body = ok_or_error(status, body)?;
    serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["id"].as_i64())
        .ok_or_else(|| "Unexpected save response.".to_string())
}

pub fn credentials_body(email: &str, password: &str) -> String {
    serde_json::json!({ "email": email, "password": password }).to_string()
}

pub fn save_body(song: &Song) -> String {
    serde_json::json!({ "song": song }).to_string()
}

// ── Browser transport ────────────────────────────────────────────────────────

/// Send a request to the same origin and return `(status, body text)`.
async fn fetch(
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<String>,
) -> Result<(u16, String), String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let js_err = |e: wasm_bindgen::JsValue| format!("Network error: {e:?}");
    let window = web_sys::window().ok_or("No browser window")?;

    let headers = web_sys::Headers::new().map_err(js_err)?;
    if let Some(t) = token {
        headers
            .set("Authorization", &format!("Bearer {t}"))
            .map_err(js_err)?;
    }
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    if let Some(b) = &body {
        headers
            .set("Content-Type", "application/json")
            .map_err(js_err)?;
        init.set_body(&wasm_bindgen::JsValue::from_str(b));
    }
    init.set_headers(&headers);

    let request = web_sys::Request::new_with_str_and_init(path, &init).map_err(js_err)?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(js_err)?
        .dyn_into()
        .map_err(|_| "Unexpected response type".to_string())?;
    let text = JsFuture::from(response.text().map_err(js_err)?)
        .await
        .map_err(js_err)?
        .as_string()
        .unwrap_or_default();
    Ok((response.status(), text))
}

/// Is the Axum API served from this origin? True only for a 401 on a protected
/// route: a static dev server answers 404 (or the app shell with 200).
pub async fn server_available() -> bool {
    matches!(fetch("GET", "/api/songs", None, None).await, Ok((401, _)))
}

pub async fn register(email: &str, password: &str) -> Result<RegisterInfo, String> {
    let (s, b) = fetch(
        "POST",
        "/api/auth/register",
        None,
        Some(credentials_body(email, password)),
    )
    .await?;
    parse_register(s, b)
}

pub async fn login(email: &str, password: &str) -> Result<LoginInfo, String> {
    let (s, b) = fetch(
        "POST",
        "/api/auth/login",
        None,
        Some(credentials_body(email, password)),
    )
    .await?;
    parse_login(s, b)
}

pub async fn list_songs(token: &str) -> Result<Vec<SongListItem>, String> {
    let (s, b) = fetch("GET", "/api/songs", Some(token), None).await?;
    parse_song_list(s, b)
}

pub async fn load_song(token: &str, id: i64) -> Result<Song, String> {
    let (s, b) = fetch("GET", &format!("/api/songs/{id}"), Some(token), None).await?;
    parse_song(s, b)
}

pub async fn save_song(token: &str, song: &Song) -> Result<i64, String> {
    let (s, b) = fetch("POST", "/api/songs", Some(token), Some(save_body(song))).await?;
    parse_saved_id(s, b)
}

pub async fn delete_song(token: &str, id: i64) -> Result<(), String> {
    let (s, b) = fetch("DELETE", &format!("/api/songs/{id}"), Some(token), None).await?;
    ok_or_error(s, b).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_response_is_parsed() {
        let body = r#"{"id":7,"email":"a@example.com","token":"abc.def.ghi"}"#.to_string();
        let info = parse_login(200, body).unwrap();
        assert_eq!(info.id, 7);
        assert_eq!(info.token, "abc.def.ghi");
    }

    #[test]
    fn server_error_text_is_shown_to_the_user() {
        let e = parse_login(
            403,
            "Please verify your email address before logging in.".into(),
        )
        .unwrap_err();
        assert_eq!(e, "Please verify your email address before logging in.");
        let e = parse_register(409, "Email already registered.".into()).unwrap_err();
        assert_eq!(e, "Email already registered.");
    }

    #[test]
    fn html_or_empty_error_bodies_fall_back_to_a_status_message() {
        assert_eq!(error_message(401, ""), "Invalid email or password.");
        assert_eq!(
            error_message(429, "<html>nope</html>"),
            "Too many attempts. Please wait a moment and try again."
        );
        assert_eq!(error_message(502, ""), "Request failed (HTTP 502).");
    }

    #[test]
    fn song_list_tolerates_missing_instruments() {
        let body = r#"[{"id":1,"name":"A","artist":"B"},{"id":2,"name":"C","artist":"D","instruments":["Bass"]}]"#;
        let list = parse_song_list(200, body.to_string()).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].instruments.is_empty());
        assert_eq!(list[1].instruments, vec![Instrument::Bass]);
    }

    #[test]
    fn a_saved_song_round_trips_through_the_request_and_response_shapes() {
        let song = chord_shifter::demo::demo_songs().remove(0);
        let sent: serde_json::Value = serde_json::from_str(&save_body(&song)).unwrap();
        let back: Song = serde_json::from_value(sent["song"].clone()).unwrap();
        assert_eq!(back, song);
        assert_eq!(parse_saved_id(200, r#"{"id":42}"#.into()).unwrap(), 42);
        assert!(parse_saved_id(200, "{}".into()).is_err());
        assert_eq!(
            parse_song(200, serde_json::to_string(&song).unwrap()).unwrap(),
            song
        );
    }

    #[test]
    fn credentials_are_json_encoded_not_interpolated() {
        let body = credentials_body("a@example.com", r#"pa"ss\word"#);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["password"], r#"pa"ss\word"#);
    }

    #[test]
    fn register_message_is_used_when_present() {
        let ok = parse_register(201, r#"{"message":"Check your email."}"#.into()).unwrap();
        assert_eq!(ok.message, "Check your email.");
    }
}
