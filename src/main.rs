// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.

#![allow(non_snake_case)]

use dioxus::prelude::*;
use manganis::Asset;

use chord_shifter::song::Instrument;

const ICON_BASE: Asset = manganis::asset!("/assets/icons/base_icon.png");
const ICON_ELECTRIC: Asset = manganis::asset!("/assets/icons/electric_icon.png");
const ICON_ACOUSTIC: Asset = manganis::asset!("/assets/icons/acoustic_icon.png");
const ICON_BASS: Asset = manganis::asset!("/assets/icons/bass_icon.png");
const ICON_PIANO: Asset = manganis::asset!("/assets/icons/piano_icon.png");
const ICON_DRUMS: Asset = manganis::asset!("/assets/icons/drums_icon.png");
const ICON_VOCALS: Asset = manganis::asset!("/assets/icons/vocals_icon.png");
const LOGO: Asset = manganis::asset!("/assets/icons/sheetwave_logo.png");

fn inst_icon(inst: Instrument) -> Asset {
    match inst {
        Instrument::Guitar => ICON_ELECTRIC,
        Instrument::AcousticGuitar => ICON_ACOUSTIC,
        Instrument::Bass => ICON_BASS,
        Instrument::Piano => ICON_PIANO,
        Instrument::Drums => ICON_DRUMS,
        Instrument::Vocals => ICON_VOCALS,
    }
}

// song and auth live in the shared library crate (src/lib.rs)
use chord_shifter::auth;
use chord_shifter::song;
mod pdf;

// ── Trigger a browser PDF download ───────────────────────────────────────────
#[allow(dead_code)]
fn trigger_download(bytes: Vec<u8>, filename: &str) {
    use js_sys::Uint8Array;
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

    let array = Uint8Array::from(bytes.as_slice());
    let parts = js_sys::Array::new();
    parts.push(&array);

    let opts = BlobPropertyBag::new();
    opts.set_type("application/pdf");

    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &opts).expect("blob");
    let url = Url::create_object_url_with_blob(&blob).expect("object url");

    let window = web_sys::window().expect("window");
    let document = window.document().expect("document");
    let a: HtmlAnchorElement = document
        .create_element("a")
        .expect("a")
        .dyn_into()
        .expect("cast");

    a.set_href(&url);
    a.set_download(&format!("{filename}.pdf"));
    a.click();
    let _ = Url::revoke_object_url(&url);
}

// ── Web storage backend (localStorage) ───────────────────────────────────────

const LS_SONGS_KEY: &str = "chord_shifter_songs";
const LS_USERS_KEY: &str = "chord_shifter_users";

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredUser {
    id: i64,
    username: String,
    password_hash: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredSong {
    id: i64,
    name: String,
    artist: String,
    key: String,
    parts_json: String,
    #[serde(default)]
    instruments_json: String,
    #[serde(default)]
    vocals_notes: String,
    #[serde(default)]
    user_id: i64,
    #[serde(default)]
    instrument_parts_json: String,
    #[serde(default)]
    instrument_capos_json: String,
    #[serde(default)]
    pdf_settings_json: String,
}

fn ls_read_users() -> Vec<StoredUser> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(LS_USERS_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn ls_write_users(users: &[StoredUser]) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        if let Ok(json) = serde_json::to_string(users) {
            let _ = storage.set_item(LS_USERS_KEY, &json);
        }
    }
}

fn ls_read() -> Vec<StoredSong> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(LS_SONGS_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn ls_write(songs: &[StoredSong]) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        if let Ok(json) = serde_json::to_string(songs) {
            let _ = storage.set_item(LS_SONGS_KEY, &json);
        }
    }
}

#[derive(Clone)]
struct Db;

impl Db {
    fn open(_: &str) -> Result<Self, String> {
        // Seed the example song into localStorage on first run.
        if ls_read().is_empty() {
            use song::{Chord, ChordQuality};
            let s = song::Song::new("Let It Be", "C Major", "The Beatles")
                .with_part(
                    "Verse",
                    vec![
                        Chord::new("C", ChordQuality::Major),
                        Chord::new("G", ChordQuality::Major),
                        Chord::new("A", ChordQuality::Minor),
                        Chord::new("F", ChordQuality::Major),
                    ],
                )
                .with_part(
                    "Chorus",
                    vec![
                        Chord::new("F", ChordQuality::Major),
                        Chord::new("C", ChordQuality::Major),
                        Chord::new("G", ChordQuality::Major),
                        Chord::new("F", ChordQuality::Major),
                    ],
                )
                .with_part(
                    "Bridge",
                    vec![
                        Chord::new("G", ChordQuality::Major),
                        Chord::new("F", ChordQuality::Major),
                        Chord::new("C", ChordQuality::Major),
                    ],
                );
            let parts_json = serde_json::to_string(&s.parts).unwrap_or_default();
            let instruments_json = serde_json::to_string(&s.instruments).unwrap_or_default();
            ls_write(&[StoredSong {
                id: 1,
                name: s.name,
                artist: s.artist,
                key: s.key,
                parts_json,
                instruments_json,
                vocals_notes: String::new(),
                user_id: 0, // sentinel: visible to all users
                instrument_parts_json: "{}".to_string(),
                instrument_capos_json: "{}".to_string(),
                pdf_settings_json: "{}".to_string(),
            }]);
        }
        Ok(Self)
    }

    fn save_song(&self, song: &song::Song, user_id: i64) -> Result<i64, String> {
        let parts_json = serde_json::to_string(&song.parts).map_err(|e| e.to_string())?;
        let instruments_json =
            serde_json::to_string(&song.instruments).map_err(|e| e.to_string())?;
        let instrument_parts_json =
            serde_json::to_string(&song.instrument_parts).map_err(|e| e.to_string())?;
        let instrument_capos_json =
            serde_json::to_string(&song.instrument_capos).map_err(|e| e.to_string())?;
        let pdf_settings_json =
            serde_json::to_string(&song.pdf_settings).map_err(|e| e.to_string())?;
        let mut songs = ls_read();
        if let Some(row) = songs
            .iter_mut()
            .find(|s| s.name == song.name && s.artist == song.artist && s.user_id == user_id)
        {
            row.key = song.key.clone();
            row.parts_json = parts_json;
            row.instruments_json = instruments_json;
            row.vocals_notes = song.vocals_notes.clone();
            row.instrument_parts_json = instrument_parts_json;
            row.instrument_capos_json = instrument_capos_json;
            row.pdf_settings_json = pdf_settings_json;
            let id = row.id;
            ls_write(&songs);
            Ok(id)
        } else {
            let id = songs.iter().map(|s| s.id).max().unwrap_or(0) + 1;
            songs.push(StoredSong {
                id,
                name: song.name.clone(),
                artist: song.artist.clone(),
                key: song.key.clone(),
                parts_json,
                instruments_json,
                vocals_notes: song.vocals_notes.clone(),
                user_id,
                instrument_parts_json,
                instrument_capos_json,
                pdf_settings_json,
            });
            ls_write(&songs);
            Ok(id)
        }
    }

    fn list_songs(&self, user_id: i64) -> Result<Vec<SongRow>, String> {
        let users = ls_read_users();
        let username = users
            .iter()
            .find(|u| u.id == user_id)
            .map(|u| u.username.clone())
            .unwrap_or_default();
        Ok(ls_read()
            .into_iter()
            .filter(|s| s.user_id == user_id || s.user_id == 0)
            .map(|s| {
                let instruments = if s.instruments_json.is_empty() {
                    Vec::new()
                } else {
                    serde_json::from_str(&s.instruments_json).unwrap_or_default()
                };
                SongRow {
                    id: s.id,
                    name: s.name,
                    artist: s.artist,
                    instruments,
                    username: username.clone(),
                }
            })
            .collect())
    }

    fn load_song(&self, id: i64) -> Result<song::Song, String> {
        ls_read()
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("Song {id} not found"))
            .and_then(|s| {
                let parts = serde_json::from_str(&s.parts_json).map_err(|e| e.to_string())?;
                let instruments = if s.instruments_json.is_empty() {
                    Vec::new()
                } else {
                    serde_json::from_str(&s.instruments_json).unwrap_or_default()
                };
                let instrument_parts = if s.instrument_parts_json.is_empty() {
                    Default::default()
                } else {
                    serde_json::from_str(&s.instrument_parts_json).unwrap_or_default()
                };
                let instrument_capos = if s.instrument_capos_json.is_empty() {
                    Default::default()
                } else {
                    serde_json::from_str(&s.instrument_capos_json).unwrap_or_default()
                };
                let pdf_settings = if s.pdf_settings_json.is_empty() {
                    Default::default()
                } else {
                    serde_json::from_str(&s.pdf_settings_json).unwrap_or_default()
                };
                Ok(song::Song {
                    name: s.name,
                    artist: s.artist,
                    key: s.key,
                    parts,
                    instruments,
                    vocals_notes: s.vocals_notes,
                    instrument_parts,
                    instrument_capos,
                    pdf_settings,
                })
            })
    }

    fn delete_song(&self, id: i64) -> Result<(), String> {
        let mut songs = ls_read();
        songs.retain(|s| s.id != id);
        ls_write(&songs);
        Ok(())
    }

    fn save_pdf(&self, _: i64, _: &[u8]) -> Result<i64, String> {
        Ok(0) // PDFs not persisted in the browser (localStorage size limits)
    }

    fn create_user(&self, username: &str, password: &str) -> Result<i64, String> {
        let mut users = ls_read_users();
        if users.iter().any(|u| u.username == username) {
            return Err(format!("Email '{username}' is already registered"));
        }
        let hash = auth::hash_password(password)?;
        let id = users.iter().map(|u| u.id).max().unwrap_or(0) + 1;
        users.push(StoredUser {
            id,
            username: username.to_string(),
            password_hash: hash,
        });
        ls_write_users(&users);
        Ok(id)
    }

    fn verify_user(&self, username: &str, password: &str) -> Result<Option<User>, String> {
        Ok(ls_read_users()
            .into_iter()
            .find(|u| u.username == username)
            .and_then(|u| {
                if auth::verify_password(password, &u.password_hash) {
                    Some(User {
                        id: u.id,
                        username: u.username,
                    })
                } else {
                    None
                }
            }))
    }
}

#[derive(Clone, Debug)]
struct User {
    id: i64,
    username: String,
}

#[derive(Clone, Debug)]
struct SongRow {
    id: i64,
    name: String,
    artist: String,
    instruments: Vec<song::Instrument>,
    username: String,
}

use song::{
    apply_notation, Chord, ChordQuality, Notation, PartItem, PartKind, PartTextSettings, Song,
    SongPart, TabCell, TabCol,
};

// ── Routes ────────────────────────────────────────────────────────────────────
#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
#[allow(clippy::enum_variant_names)]
enum Route {
    #[route("/")]
    LibraryPage {},
    #[route("/song/new")]
    NewSongPage {},
    #[route("/song/:id")]
    SongPage { id: i64 },
    #[route("/song/:id/instrument/:instrument")]
    InstrumentSheetPage { id: i64, instrument: String },
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    dioxus::launch(App);
}

// ── Example data ──────────────────────────────────────────────────────────────

fn example_song() -> Song {
    Song::new("Let It Be", "C Major", "The Beatles")
        .with_part(
            "Verse",
            vec![
                Chord::new("C", ChordQuality::Major),
                Chord::new("G", ChordQuality::Major),
                Chord::new("A", ChordQuality::Minor),
                Chord::new("F", ChordQuality::Major),
            ],
        )
        .with_part(
            "Chorus",
            vec![
                Chord::new("F", ChordQuality::Major),
                Chord::new("C", ChordQuality::Major),
                Chord::new("G", ChordQuality::Major),
                Chord::new("F", ChordQuality::Major),
            ],
        )
        .with_part(
            "Bridge",
            vec![
                Chord::new("G", ChordQuality::Major),
                Chord::new("F", ChordQuality::Major),
                Chord::new("C", ChordQuality::Major),
            ],
        )
}

fn blank_song() -> Song {
    Song::new("", "", "")
}

// ── App-level screen state ───────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum AppScreen {
    Landing,
    Login,
    Register,
}

// ── Root component ────────────────────────────────────────────────────────────

#[component]
fn App() -> Element {
    let db: Signal<Option<Db>> = use_signal(|| {
        Db::open("chord_shifter.db")
            .map_err(|e| eprintln!("DB open failed: {e}"))
            .ok()
    });
    let current_user: Signal<Option<User>> = use_signal(|| None);
    let screen: Signal<AppScreen> = use_signal(|| AppScreen::Landing);

    use_context_provider(|| db);
    use_context_provider(|| current_user);

    rsx! {
        div {
            style: "
                font-family: 'Helvetica Neue', Arial, sans-serif;
                min-height: 100vh;
                background: #f0f4ff;
            ",

            if current_user.read().is_some() {
                Router::<Route> {}
            } else if *screen.read() == AppScreen::Landing {
                LandingPage { screen }
            } else {
                div {
                    style: "min-height: 100vh; background: linear-gradient(180deg, #ffffff 0%, #eef3fc 100%); display: flex; align-items: flex-start; justify-content: center; padding: 60px 20px;",
                    LoginScreen { db, current_user, screen }
                }
            }
        }
    }
}

// ── Landing page ─────────────────────────────────────────────────────────────

#[component]
fn LandingPage(mut screen: Signal<AppScreen>) -> Element {
    rsx! {
        div {
            style: "
                min-height: 100vh;
                background: #ffffff;
                display: flex;
                flex-direction: column;
                font-family: 'Helvetica Neue', Arial, sans-serif;
            ",

            // ── Nav bar ───────────────────────────────────────────────────────
            div {
                style: "
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    padding: 18px 52px;
                    border-bottom: 1.5px solid #e8edf5;
                    background: #ffffff;
                ",
                // Logo + wordmark
                div {
                    style: "display: flex; align-items: center; gap: 10px;",
                    img {
                        src: LOGO.to_string(),
                        style: "height: 38px; width: auto; object-fit: contain;",
                        alt: "SheetWave"
                    }
                    span {
                        style: "font-size: 19px; font-weight: 900; color: #0a0f1e; letter-spacing: -0.4px;",
                        "SheetWave"
                    }
                }
                // Nav actions
                div {
                    style: "display: flex; align-items: center; gap: 10px;",
                    button {
                        style: "
                            padding: 9px 22px;
                            background: transparent;
                            border: 1.5px solid #2563eb;
                            border-radius: 8px;
                            font-size: 13px;
                            font-weight: 700;
                            color: #2563eb;
                            cursor: pointer;
                            font-family: inherit;
                            letter-spacing: 0.2px;
                        ",
                        onclick: move |_| *screen.write() = AppScreen::Login,
                        "Log in"
                    }
                    button {
                        style: "
                            padding: 9px 22px;
                            background: #2563eb;
                            border: none;
                            border-radius: 8px;
                            font-size: 13px;
                            font-weight: 700;
                            color: #ffffff;
                            cursor: pointer;
                            font-family: inherit;
                            letter-spacing: 0.2px;
                        ",
                        onclick: move |_| *screen.write() = AppScreen::Register,
                        "Get started"
                    }
                }
            }

            // ── Hero ──────────────────────────────────────────────────────────
            div {
                style: "
                    flex: 1;
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                    padding: 64px 24px 40px;
                    text-align: center;
                    background: linear-gradient(180deg, #ffffff 0%, #eef3fc 100%);
                ",

                // Large logo
                img {
                    src: LOGO.to_string(),
                    style: "width: 180px; height: 180px; object-fit: contain; margin-bottom: 32px; filter: drop-shadow(0 6px 24px rgba(37,99,235,0.18));",
                    alt: "SheetWave"
                }

                // Headline
                h1 {
                    style: "
                        margin: 0 0 16px;
                        font-size: clamp(34px, 5.5vw, 60px);
                        font-weight: 900;
                        color: #0a0f1e;
                        letter-spacing: -1.5px;
                        line-height: 1.1;
                    ",
                    "Your songs."
                    br {}
                    span {
                        style: "color: #2563eb;",
                        "Every instrument."
                    }
                }

                // Sub-headline
                p {
                    style: "
                        margin: 0 auto 48px;
                        max-width: 500px;
                        font-size: 18px;
                        line-height: 1.65;
                        color: #4b5563;
                        font-weight: 400;
                    ",
                    "Write chord sheets, tabs, and lyrics for every instrument in one place — "
                    "then export to PDF in seconds."
                }

                // CTA buttons
                div {
                    style: "display: flex; align-items: center; gap: 14px; flex-wrap: wrap; justify-content: center; margin-bottom: 72px;",

                    button {
                        style: "
                            padding: 16px 44px;
                            background: #2563eb;
                            border: none;
                            border-radius: 12px;
                            font-size: 16px;
                            font-weight: 800;
                            color: #ffffff;
                            cursor: pointer;
                            font-family: inherit;
                            letter-spacing: 0.3px;
                            box-shadow: 0 4px 20px rgba(37,99,235,0.35);
                        ",
                        onclick: move |_| *screen.write() = AppScreen::Register,
                        "✦  Let's get started"
                    }

                    button {
                        style: "
                            padding: 16px 44px;
                            background: #ffffff;
                            border: 2px solid #2563eb;
                            border-radius: 12px;
                            font-size: 16px;
                            font-weight: 700;
                            color: #2563eb;
                            cursor: pointer;
                            font-family: inherit;
                            letter-spacing: 0.3px;
                        ",
                        onclick: move |_| *screen.write() = AppScreen::Login,
                        "Log in to my account"
                    }
                }

                // ── Feature cards ─────────────────────────────────────────────
                div {
                    style: "
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
                        gap: 16px;
                        max-width: 880px;
                        width: 100%;
                    ",

                    // Card 1
                    div {
                        style: "
                            background: #ffffff;
                            border: 1.5px solid #dbeafe;
                            border-radius: 14px;
                            padding: 24px 22px;
                            text-align: left;
                            box-shadow: 0 2px 12px rgba(37,99,235,0.07);
                        ",
                        div { style: "font-size: 28px; margin-bottom: 12px;", "🎸" }
                        div { style: "font-size: 14px; font-weight: 800; color: #0a0f1e; margin-bottom: 6px;", "Multi-instrument sheets" }
                        div { style: "font-size: 13px; color: #6b7280; line-height: 1.55;", "Guitar, bass, piano, drums, and vocals — all in one song." }
                    }

                    // Card 2
                    div {
                        style: "
                            background: #ffffff;
                            border: 1.5px solid #dbeafe;
                            border-radius: 14px;
                            padding: 24px 22px;
                            text-align: left;
                            box-shadow: 0 2px 12px rgba(37,99,235,0.07);
                        ",
                        div { style: "font-size: 28px; margin-bottom: 12px;", "🎼" }
                        div { style: "font-size: 14px; font-weight: 800; color: #0a0f1e; margin-bottom: 6px;", "Chord transposition & capo" }
                        div { style: "font-size: 13px; color: #6b7280; line-height: 1.55;", "Shift any chord sheet to a new key or capo position instantly." }
                    }

                    // Card 3
                    div {
                        style: "
                            background: #ffffff;
                            border: 1.5px solid #dbeafe;
                            border-radius: 14px;
                            padding: 24px 22px;
                            text-align: left;
                            box-shadow: 0 2px 12px rgba(37,99,235,0.07);
                        ",
                        div { style: "font-size: 28px; margin-bottom: 12px;", "📄" }
                        div { style: "font-size: 14px; font-weight: 800; color: #0a0f1e; margin-bottom: 6px;", "One-click PDF export" }
                        div { style: "font-size: 13px; color: #6b7280; line-height: 1.55;", "Export print-ready PDF sheets for rehearsals and gigs." }
                    }

                    // Card 4
                    div {
                        style: "
                            background: #ffffff;
                            border: 1.5px solid #dbeafe;
                            border-radius: 14px;
                            padding: 24px 22px;
                            text-align: left;
                            box-shadow: 0 2px 12px rgba(37,99,235,0.07);
                        ",
                        div { style: "font-size: 28px; margin-bottom: 12px;", "☁️" }
                        div { style: "font-size: 14px; font-weight: 800; color: #0a0f1e; margin-bottom: 6px;", "Cloud library" }
                        div { style: "font-size: 13px; color: #6b7280; line-height: 1.55;", "Your song library syncs automatically and is always with you." }
                    }
                }
            }

            // ── Footer ──────────────────────────────────────────────────────
            div {
                style: "
                    text-align: center;
                    padding: 24px;
                    font-size: 12px;
                    color: #9ca3af;
                    border-top: 1.5px solid #e8edf5;
                    background: #ffffff;
                ",
                "© 2026 APSOS — App and Software Solutions Wörner"
            }
        }
    }
}

// ── Song sheet ────────────────────────────────────────────────────────────────

#[component]
fn SongView(
    song: Signal<Song>,
    db: Signal<Option<Db>>,
    mut current_user: Signal<Option<User>>,
    song_id: Option<i64>,
) -> Element {
    let nav = use_navigator();
    let mut preview_open = use_signal(|| false);
    let mut preview_url: Signal<String> = use_signal(String::new);
    let mut notation: Signal<Notation> = use_signal(|| Notation::English);
    let mut render_mode: Signal<bool> = use_signal(|| false);
    let base_capo: Signal<i8> = use_signal(|| 0_i8);
    // None = base sheet; Some(inst) = that instrument's sheet
    let mut active_instrument: Signal<Option<Instrument>> = use_signal(|| None);
    // Per-instrument working copies — each instrument has its own isolated signal
    let mut guitar_song = {
        let s = song.read();
        let parts = s
            .instrument_parts
            .get("Electric")
            .cloned()
            .unwrap_or_else(|| s.parts.clone());
        let sc = s.clone();
        drop(s);
        use_signal(move || Song { parts, ..sc })
    };
    let mut guitar_capo = {
        let cap = *song.read().instrument_capos.get("Electric").unwrap_or(&0);
        use_signal(move || cap)
    };
    let mut acoustic_song = {
        let s = song.read();
        let parts = s
            .instrument_parts
            .get("Acoustic")
            .cloned()
            .unwrap_or_else(|| s.parts.clone());
        let sc = s.clone();
        drop(s);
        use_signal(move || Song { parts, ..sc })
    };
    let mut acoustic_capo = {
        let cap = *song.read().instrument_capos.get("Acoustic").unwrap_or(&0);
        use_signal(move || cap)
    };
    let mut bass_song = {
        let s = song.read();
        let parts = s
            .instrument_parts
            .get("Bass")
            .cloned()
            .unwrap_or_else(|| s.parts.clone());
        let sc = s.clone();
        drop(s);
        use_signal(move || Song { parts, ..sc })
    };
    let bass_capo = {
        let cap = *song.read().instrument_capos.get("Bass").unwrap_or(&0);
        use_signal(move || cap)
    };
    let mut piano_song = {
        let s = song.read();
        let parts = s
            .instrument_parts
            .get("Piano")
            .cloned()
            .unwrap_or_else(|| s.parts.clone());
        let sc = s.clone();
        drop(s);
        use_signal(move || Song { parts, ..sc })
    };
    let mut piano_capo = {
        let cap = *song.read().instrument_capos.get("Piano").unwrap_or(&0);
        use_signal(move || cap)
    };
    let mut drums_song = {
        let s = song.read();
        let parts = s
            .instrument_parts
            .get("Drums")
            .cloned()
            .unwrap_or_else(|| s.parts.clone());
        let sc = s.clone();
        drop(s);
        use_signal(move || Song { parts, ..sc })
    };
    let mut vocals_song = {
        let s = song.read();
        let parts = s
            .instrument_parts
            .get("Vocals")
            .cloned()
            .unwrap_or_else(|| s.parts.clone());
        let sc = s.clone();
        drop(s);
        use_signal(move || Song { parts, ..sc })
    };
    let vocals_capo = {
        let cap = *song.read().instrument_capos.get("Vocals").unwrap_or(&0);
        use_signal(move || cap)
    };
    let mut inst_save_msg: Signal<Option<&'static str>> = use_signal(|| None);

    // Propagate base-sheet edits to any instrument that has no saved override.
    // Runs automatically whenever `song` changes (e.g. chord edits, transpose).
    use_effect(move || {
        let s = song.read();
        let base = s.parts.clone();
        let overrides = &s.instrument_parts;
        if !overrides.contains_key("Electric") {
            guitar_song.write().parts = base.clone();
        }
        if !overrides.contains_key("Acoustic") {
            acoustic_song.write().parts = base.clone();
        }
        if !overrides.contains_key("Bass") {
            bass_song.write().parts = base.clone();
        }
        if !overrides.contains_key("Piano") {
            piano_song.write().parts = base.clone();
        }
        if !overrides.contains_key("Drums") {
            drums_song.write().parts = base.clone();
        }
        if !overrides.contains_key("Vocals") {
            vocals_song.write().parts = base.clone();
        }
    });

    // Keep instrument_capos in song in sync with the capo signals so the
    // values are persisted even without an explicit "Save instrument" click.
    use_effect(move || {
        let caps = [
            ("Electric", guitar_capo()),
            ("Acoustic", acoustic_capo()),
            ("Bass", bass_capo()),
            ("Piano", piano_capo()),
        ];
        let mut s = song.write();
        for (label, cap) in caps {
            s.instrument_capos.insert(label.to_string(), cap);
        }
    });

    rsx! {
        div {
            style: "
                background: #ffffff;
                border-radius: 14px;
                padding: 48px 52px;
                box-shadow: 0 4px 32px rgba(0,0,0,0.10);
                max-width: 720px;
                width: 100%;
            ",

            // ── Editable header ───────────────────────────────────────────────
            div {
                style: "
                    border-bottom: 2px solid #dbeafe;
                    padding-bottom: 24px;
                    margin-bottom: 36px;
                ",

                // Top row: back button + user greeting + logout
                div {
                    style: "display: flex; justify-content: space-between; align-items: center; gap: 10px; margin-bottom: 16px;",
                    button {
                        style: "
                            padding: 4px 12px;
                            background: transparent;
                            border: 1px solid #ccc;
                            border-radius: 8px;
                            font-size: 11px;
                            font-weight: 700;
                            cursor: pointer;
                            font-family: inherit;
                            color: #888;
                        ",
                        onclick: move |_| { nav.push(Route::LibraryPage {}); },
                        "← Library"
                    }
                    div {
                        style: "display: flex; align-items: center; gap: 10px;",
                        if let Some(user) = current_user.read().as_ref() {
                            span {
                                style: "font-size: 12px; color: #888; font-weight: 600;",
                                "👤  {user.username}"
                            }
                        }
                        button {
                            style: "
                                padding: 4px 12px;
                                background: transparent;
                                border: 1px solid #ccc;
                                border-radius: 8px;
                                font-size: 11px;
                                font-weight: 700;
                                cursor: pointer;
                                font-family: inherit;
                                color: #888;
                            ",
                            onclick: move |_| *current_user.write() = None,
                            "Log out"
                        }
                    }
                }

                // Song name
                input {
                    style: "
                        display: block;
                        width: 100%;
                        margin: 0 0 6px 0;
                        padding: 2px 0;
                        font-size: 38px;
                        font-weight: 800;
                        color: #0a0f1e;
                        letter-spacing: -0.5px;
                        border: none;
                        border-bottom: 2px dashed #bfdbfe;
                        background: transparent;
                        outline: none;
                        font-family: inherit;
                        box-sizing: border-box;
                    ",
                    value: "{song.read().name}",
                    placeholder: "Song name",
                    oninput: move |e| song.write().name = e.value(),
                }

                // Artist
                input {
                    style: "
                        display: block;
                        width: 100%;
                        margin: 0 0 14px 0;
                        padding: 2px 0;
                        font-size: 17px;
                        color: #666;
                        font-style: italic;
                        border: none;
                        border-bottom: 1px dashed #bfdbfe;
                        background: transparent;
                        outline: none;
                        font-family: inherit;
                        box-sizing: border-box;
                    ",
                    value: "{song.read().artist}",
                    placeholder: "Artist",
                    oninput: move |e| song.write().artist = e.value(),
                }

                // Key pill with − / + semitone buttons and Major/Minor toggle
                div {
                    style: "display: inline-flex; align-items: center; gap: 6px; flex-wrap: wrap; margin-top: 6px;",

                    // ── Key root pill (− root +) ───────────────────────────
                    div {
                        style: "display: inline-flex; align-items: center; background: #2563eb; border-radius: 20px; overflow: hidden;",
                        span {
                            style: "color: #ffffff; padding: 5px 6px 5px 14px; font-size: 12px; font-weight: 700; letter-spacing: 1.2px; text-transform: uppercase; white-space: nowrap;",
                            "Key:"
                        }
                        // − button
                        button {
                            style: "background: rgba(255,255,255,0.10); border: none; color: #ffffff; font-size: 16px; font-weight: 700; padding: 0 8px; cursor: pointer; font-family: inherit; line-height: 1; height: 30px;",
                            onclick: move |_| {
                                let key = song.read().key.clone();
                                let is_minor = key.to_lowercase().contains("minor");
                                let root = key.split_whitespace().next().unwrap_or("C").to_string();
                                let new_root = song::shift_note(&root, 1, is_minor);
                                song.write().transpose_to(&new_root);
                            },
                            "−"
                        }
                        // Key root display (read-only)
                        span {
                            style: "color: #ffffff; padding: 5px 6px; font-size: 14px; font-weight: 800; letter-spacing: 0.5px; min-width: 28px; text-align: center;",
                            {
                                let k = song.read().key.clone();
                                k.split_whitespace().next().unwrap_or("C").to_string()
                            }
                        }
                        // ♭/♯ enharmonic flip button — only active when root has an equivalent
                        {
                            let key = song.read().key.clone();
                            let is_minor = key.to_lowercase().contains("minor");
                            let root = key.split_whitespace().next().unwrap_or("C").to_string();
                            let enharmonic: Option<&str> = match root.as_str() {
                                "C#" => Some("Db"),
                                "Db" => Some("C#"),
                                "D#" => Some("Eb"),
                                "Eb" => Some("D#"),
                                "F#" => Some("Gb"),
                                "Gb" => Some("F#"),
                                "G#" => Some("Ab"),
                                "Ab" => Some("G#"),
                                "A#" => Some("Bb"),
                                "Bb" => Some("A#"),
                                _    => None,
                            };
                            rsx! {
                                if let Some(enh) = enharmonic {
                                    button {
                                        style: "background: rgba(255,255,255,0.18); border: none; color: #ffffff; font-size: 11px; font-weight: 700; padding: 0 7px; cursor: pointer; font-family: inherit; line-height: 1; height: 30px; white-space: nowrap;",
                                        title: "Switch to {enh}",
                                        onclick: move |_| {
                                            let mode = if is_minor { "Minor" } else { "Major" };
                                            song.write().transpose_to(enh);
                                            song.write().key = format!("{} {}", enh, mode);
                                        },
                                        if enh.contains('b') { "→♭" } else { "→♯" }
                                    }
                                }
                            }
                        }
                        // + button
                        button {
                            style: "background: rgba(255,255,255,0.10); border: none; color: #ffffff; font-size: 16px; font-weight: 700; padding: 0 8px; cursor: pointer; font-family: inherit; line-height: 1; height: 30px;",
                            onclick: move |_| {
                                let key = song.read().key.clone();
                                let is_minor = key.to_lowercase().contains("minor");
                                let root = key.split_whitespace().next().unwrap_or("C").to_string();
                                let new_root = song::shift_note_up(&root, 1, is_minor);
                                song.write().transpose_to(&new_root);
                            },
                            "+"
                        }
                    }

                    // ── Major / Minor toggle ───────────────────────────────
                    {
                        let key = song.read().key.clone();
                        let is_minor = key.to_lowercase().contains("minor");
                        let maj_style = if !is_minor {
                            "padding: 5px 12px; border-radius: 16px 0 0 16px; border: none; font-size: 11px; font-weight: 700; font-family: inherit; cursor: pointer; background: #2563eb; color: #ffffff;"
                        } else {
                            "padding: 5px 12px; border-radius: 16px 0 0 16px; border: 1.5px solid #bfdbfe; border-right: none; font-size: 11px; font-weight: 700; font-family: inherit; cursor: pointer; background: #eff6ff; color: #888;"
                        };
                        let min_style = if is_minor {
                            "padding: 5px 12px; border-radius: 0 16px 16px 0; border: none; font-size: 11px; font-weight: 700; font-family: inherit; cursor: pointer; background: #2563eb; color: #ffffff;"
                        } else {
                            "padding: 5px 12px; border-radius: 0 16px 16px 0; border: 1.5px solid #bfdbfe; border-left: none; font-size: 11px; font-weight: 700; font-family: inherit; cursor: pointer; background: #eff6ff; color: #888;"
                        };
                        rsx! {
                            div {
                                style: "display: inline-flex; align-items: center;",
                                button {
                                    style: "{maj_style}",
                                    onclick: move |_| {
                                        let key = song.read().key.clone();
                                        let root = key.split_whitespace().next().unwrap_or("C").to_string();
                                        song.write().key = format!("{} Major", root);
                                    },
                                    "Major"
                                }
                                button {
                                    style: "{min_style}",
                                    onclick: move |_| {
                                        let key = song.read().key.clone();
                                        let root = key.split_whitespace().next().unwrap_or("C").to_string();
                                        song.write().key = format!("{} Minor", root);
                                    },
                                    "Minor"
                                }
                            }
                        }
                    }
                }

                // ── Set key without transposing ────────────────────────────
                {
                    let key = song.read().key.clone();
                    let _is_minor = key.to_lowercase().contains("minor");
                    let current_root = key.split_whitespace().next().unwrap_or("C").to_string();
                    let notes = [
                        "C", "C#", "Db", "D", "D#", "Eb", "E", "F", "F#", "Gb",
                        "G", "G#", "Ab", "A", "A#", "Bb", "B",
                    ];
                    rsx! {
                        div {
                            style: "display: inline-flex; align-items: center; gap: 8px; margin-top: 28px;",
                            span {
                                style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px; white-space: nowrap;",
                                "Set key:"
                            }
                            select {
                                style: "font-size: 13px; font-weight: 700; color: #0a0f1e; background: #eff6ff; border: 1.5px solid #bfdbfe; border-radius: 10px; padding: 4px 10px; outline: none; cursor: pointer; font-family: inherit;",
                                title: "Change key without transposing chords",
                                onchange: move |e| {
                                    let new_root = e.value();
                                    let mut s = song.write();
                                    // Only update the key label — do NOT transpose chords.
                                    let mode = s.key
                                        .split_whitespace()
                                        .skip(1)
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    s.key = if mode.is_empty() {
                                        new_root.clone()
                                    } else {
                                        format!("{} {}", new_root, mode)
                                    };
                                },
                                for note in notes {
                                    option {
                                        value: "{note}",
                                        selected: current_root == note,
                                        "{note}"
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Notation ────────────────────────────────────────────────
                div {
                    style: "display: inline-flex; align-items: center; gap: 8px; margin-top: 14px;",
                    span {
                        style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px; white-space: nowrap;",
                        "Notation:"
                    }
                    select {
                        style: "font-size: 13px; font-weight: 700; color: #0a0f1e; background: #eff6ff; border: 1.5px solid #bfdbfe; border-radius: 10px; padding: 4px 10px; outline: none; cursor: pointer; font-family: inherit;",
                        onchange: move |e| {
                            *notation.write() = match e.value().as_str() {
                                "german" => Notation::German,
                                "custom" => Notation::Custom,
                                _        => Notation::English,
                            };
                        },
                        option { value: "english", selected: notation() == Notation::English, "English  (B / B♭)" }
                        option { value: "german",  selected: notation() == Notation::German,  "German  (H / B)" }
                        option { value: "custom",  selected: notation() == Notation::Custom,  "Custom  (H / B♭)" }
                    }
                }

                // ── Instrument tabs ─────────────────────────────────────────
                div {
                    style: "margin-top: 28px; display: flex; align-items: center; gap: 12px; flex-wrap: wrap;",
                    span {
                        style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;",
                        "Sheet:"
                    }
                    div {
                        style: "display: flex; gap: 8px; flex-wrap: wrap;",

                        // Base tab
                        {
                            let is_base = active_instrument.read().is_none();
                            let s = if is_base {
                                "display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#2563eb;border:none;border-radius:10px;cursor:pointer;font-family:inherit;"
                            } else {
                                "display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#eff6ff;border:1.5px solid #dbeafe;border-radius:10px;cursor:pointer;font-family:inherit;"
                            };
                            let lbl_s = if is_base {
                                "font-size:9px;font-weight:700;letter-spacing:0.8px;text-transform:uppercase;color:#ffffff;"
                            } else {
                                "font-size:9px;font-weight:700;letter-spacing:0.8px;text-transform:uppercase;color:#888;"
                            };
                            rsx! {
                                button {
                                    style: "{s}",
                                    onclick: move |_| {
                                        *active_instrument.write() = None;
                                        *inst_save_msg.write() = None;
                                    },
                                    img { src: ICON_BASE.to_string(), style: "width: 44px; height: 44px; object-fit: contain;", alt: "Base" }
                                    span { style: "{lbl_s}", "Base" }
                                }
                            }
                        }

                        // Per-instrument tabs
                        for inst in Instrument::all() {
                            {
                                let is_active = active_instrument.read().as_ref() == Some(&inst);
                                let has_override = song.read().instrument_parts.contains_key(inst.label());
                                let accent = inst.accent_color();
                                let btn_s = if is_active {
                                    format!("display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:{accent};border:none;border-radius:10px;cursor:pointer;font-family:inherit;")
                                } else if has_override {
                                    format!("display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#eff6ff;border:2px solid {accent};border-radius:10px;cursor:pointer;font-family:inherit;")
                                } else {
                                    "display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#eff6ff;border:1.5px solid #dbeafe;border-radius:10px;cursor:pointer;font-family:inherit;".to_string()
                                };
                                let lbl_s = if is_active {
                                    "font-size:9px;font-weight:700;letter-spacing:0.8px;text-transform:uppercase;color:#fff;".to_string()
                                } else {
                                    "font-size:9px;font-weight:700;letter-spacing:0.8px;text-transform:uppercase;color:#888;".to_string()
                                };
                                rsx! {
                                    button {
                                        key: "{inst.label()}",
                                        style: "{btn_s}",
                                        title: "{inst.label()} sheet",
                                        onclick: move |_| {
                                            if active_instrument.read().as_ref() == Some(&inst) {
                                                *active_instrument.write() = None;
                                                *inst_save_msg.write() = None;
                                            } else {
                                                *active_instrument.write() = Some(inst);
                                                *inst_save_msg.write() = None;
                                            }
                                        },
                                        img { src: inst_icon(inst).to_string(), style: "width: 44px; height: 44px; object-fit: contain;", alt: "{inst.label()}" }
                                        span { style: "{lbl_s}", "{inst.label()}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Rendered View / Edit toggle ───────────────────────────────────────
            div {
                style: "margin-top: 20px; display: flex; justify-content: flex-end;",
                button {
                    style: {
                        if render_mode() {
                            "display: inline-flex; align-items: center; gap: 6px; padding: 7px 18px; background: #2563eb; color: #ffffff; border: 2px solid #2563eb; border-radius: 20px; font-size: 12px; font-weight: 700; cursor: pointer; font-family: inherit; letter-spacing: 0.4px;"
                        } else {
                            "display: inline-flex; align-items: center; gap: 6px; padding: 7px 18px; background: #eff6ff; color: #0a0f1e; border: 2px solid #bfdbfe; border-radius: 20px; font-size: 12px; font-weight: 700; cursor: pointer; font-family: inherit; letter-spacing: 0.4px;"
                        }
                    },
                    onclick: move |_| { *render_mode.write() = !render_mode(); },
                    if render_mode() { "✏️  Edit" } else { "🖨  Rendered View" }
                }
            }

            // ── Parts editor (base or instrument) ─────────────────────────────────
            if render_mode() && active_instrument.read().is_none() {
                RenderedSheet { song, notation, capo: base_capo }
            }
            if render_mode() && *active_instrument.read() == Some(Instrument::Guitar) {
                div {
                    style: "display: flex; align-items: center; gap: 10px; margin-top: 16px;",
                    span { style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;", "Capo:" }
                    button {
                        style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                        onclick: move |_| { if guitar_capo() > 0 { *guitar_capo.write() -= 1; } },
                        "−"
                    }
                    span { style: "min-width: 52px; text-align: center; font-size: 13px; font-weight: 800; color: #0a0f1e;",
                        if guitar_capo() == 0 { "Off" } else { "{guitar_capo()}" }
                    }
                    button {
                        style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                        onclick: move |_| { if guitar_capo() < 12 { *guitar_capo.write() += 1; } },
                        "+"
                    }
                    if guitar_capo() > 0 {
                        span { style: "font-size: 11px; color: #888; font-style: italic;",
                            "→ play in {guitar_song.read().apply_capo(guitar_capo()).key}"
                        }
                    }
                }
                RenderedSheet { song: guitar_song, notation, capo: guitar_capo }
            }
            if render_mode() && *active_instrument.read() == Some(Instrument::AcousticGuitar) {
                div {
                    style: "display: flex; align-items: center; gap: 10px; margin-top: 16px;",
                    span { style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;", "Capo:" }
                    button {
                        style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                        onclick: move |_| { if acoustic_capo() > 0 { *acoustic_capo.write() -= 1; } },
                        "−"
                    }
                    span { style: "min-width: 52px; text-align: center; font-size: 13px; font-weight: 800; color: #0a0f1e;",
                        if acoustic_capo() == 0 { "Off" } else { "{acoustic_capo()}" }
                    }
                    button {
                        style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                        onclick: move |_| { if acoustic_capo() < 12 { *acoustic_capo.write() += 1; } },
                        "+"
                    }
                    if acoustic_capo() > 0 {
                        span { style: "font-size: 11px; color: #888; font-style: italic;",
                            "→ play in {acoustic_song.read().apply_capo(acoustic_capo()).key}"
                        }
                    }
                }
                RenderedSheet { song: acoustic_song, notation, capo: acoustic_capo }
            }
            if render_mode() && *active_instrument.read() == Some(Instrument::Bass) {
                RenderedSheet { song: bass_song, notation, capo: bass_capo }
            }
            if render_mode() && *active_instrument.read() == Some(Instrument::Piano) {
                div {
                    style: "display: flex; align-items: center; gap: 10px; margin-top: 16px;",
                    span { style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;", "Transpose:" }
                    button {
                        style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                        onclick: move |_| { if piano_capo() > -12 { *piano_capo.write() -= 1; } },
                        "−"
                    }
                    span { style: "min-width: 52px; text-align: center; font-size: 13px; font-weight: 800; color: #0a0f1e;",
                        if piano_capo() == 0 { "Off" } else if piano_capo() > 0 { "+{piano_capo()}" } else { "{piano_capo()}" }
                    }
                    button {
                        style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                        onclick: move |_| { if piano_capo() < 12 { *piano_capo.write() += 1; } },
                        "+"
                    }
                    if piano_capo() != 0 {
                        span { style: "font-size: 11px; color: #888; font-style: italic;",
                            "→ sounds in {piano_song.read().apply_capo(-piano_capo()).key}"
                        }
                    }
                }
                RenderedSheet { song: piano_song, notation, capo: piano_capo }
            }
            if render_mode() && *active_instrument.read() == Some(Instrument::Drums) {
                RenderedSheet { song: drums_song, notation, capo: use_signal(|| 0_i8) }
            }
            if render_mode() && *active_instrument.read() == Some(Instrument::Vocals) {
                RenderedSheet { song: vocals_song, notation, capo: vocals_capo }
            }
            if !render_mode() {
            if active_instrument.read().is_none() {
                // Base sheet
                for part_index in 0..song.read().parts.len() {
                    // Insert-between divider
                    {
                        let insert_index = part_index;
                        rsx! {
                            div {
                                key: "ins-{part_index}",
                                style: "display: flex; align-items: center; gap: 6px; margin-bottom: 8px; opacity: 0.45;",
                                class: "insert-divider",
                                div { style: "flex: 1; height: 1px; background: #ddd;" }
                                button {
                                    style: "padding: 2px 10px; background: transparent; color: #999; border: 1.5px dashed #93c5fd; border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; font-family: inherit; white-space: nowrap;",
                                    title: "Insert part here",
                                    onclick: move |_| {
                                        let mut s = song.write();
                                        s.parts.insert(insert_index, crate::song::SongPart::new("New Part"));
                                    },
                                    "+ Part"
                                }
                                div { style: "flex: 1; height: 1px; background: #ddd;" }
                            }
                        }
                    }
                    PartView { key: "{part_index}", song, part_index, notation, capo: use_signal(|| 0_i8) }
                }
                button {
                    style: "
                        margin-top: 8px;
                        margin-bottom: 12px;
                        padding: 10px 20px;
                        background: transparent;
                        color: #999;
                        border: 2px dashed #93c5fd;
                        border-radius: 10px;
                        font-size: 13px;
                        font-weight: 600;
                        cursor: pointer;
                        font-family: inherit;
                        width: 100%;
                    ",
                    onclick: move |_| { song.write().parts.push(crate::song::SongPart::new("New Part")); },
                    "+ Add Part"
                }
                button {
                    style: "
                        margin-bottom: 24px;
                        padding: 10px 20px;
                        background: transparent;
                        color: #6a7fa6;
                        border: 1.5px solid #a0b4cc;
                        border-radius: 10px;
                        font-size: 13px;
                        font-weight: 600;
                        cursor: pointer;
                        font-family: inherit;
                        width: 100%;
                    ",
                    onclick: move |_| {
                        let base_parts = song.read().parts.clone();
                        // Push base parts to every instrument signal.
                        guitar_song.write().parts = base_parts.clone();
                        acoustic_song.write().parts = base_parts.clone();
                        bass_song.write().parts = base_parts.clone();
                        piano_song.write().parts = base_parts.clone();
                        drums_song.write().parts = base_parts.clone();
                        vocals_song.write().parts = base_parts.clone();
                        // Also overwrite saved overrides so the changes persist
                        // when the user saves an instrument sheet.
                        let mut s = song.write();
                        for label in ["Electric", "Acoustic", "Bass", "Piano", "Drums", "Vocals"] {
                            if s.instrument_parts.contains_key(label) {
                                s.instrument_parts.insert(label.to_string(), base_parts.clone());
                            }
                        }
                    },
                    "⬇ Copy base to all instruments"
                }
            } else {
                // Instrument-specific sheet
                {
                    let inst = (*active_instrument.read()).unwrap();
                    let (mut act_song, mut act_capo) = match inst {
                        Instrument::Guitar => (guitar_song, guitar_capo),
                        Instrument::AcousticGuitar => (acoustic_song, acoustic_capo),
                        Instrument::Bass => (bass_song, bass_capo),
                        Instrument::Piano => (piano_song, piano_capo),
                        Instrument::Drums => (drums_song, use_signal(|| 0_i8)),
                        Instrument::Vocals => (vocals_song, vocals_capo),
                    };
                    let accent = inst.accent_color();
                    let inst_label = inst.label();
                    rsx! {
                        // Info banner
                        div {
                            style: "margin-bottom: 18px; background: #eff6ff; border: 1px solid #bfdbfe; border-radius: 8px; padding: 10px 16px; font-size: 12px; color: #1d4ed8; display: flex; align-items: center; gap: 8px;",
                            img { src: inst_icon(inst).to_string(), style: "width: 22px; height: 22px; object-fit: contain;", alt: "{inst_label}" }
                            span { "✏️  " strong { "{inst_label}" } " sheet — edits apply to this instrument only" }
                        }
                        // Instrument capo/transpose control (hidden for drums and vocals)
                        if inst == Instrument::Vocals {
                            // ── Global chords on/off for vocals ─────────────────
                            {
                                let all_on = act_song.read().parts.iter()
                                    .all(|p| p.part_text.as_ref().map(|t| t.show_chords).unwrap_or(false));
                                let btn_border = if all_on { "#7a9060" } else { "#bfdbfe" };
                                let btn_bg     = if all_on { "#e8f0e0" } else { "#eff6ff" };
                                let btn_fg     = if all_on { "#4a6040" } else { "#aaa" };
                                let btn_label  = if all_on { "\u{1F3B8} Chords: on (all parts)" } else { "\u{1F3B8} Chords: off (all parts)" };
                                rsx! {
                                    div {
                                        style: "margin-bottom: 16px; display: flex; align-items: center; gap: 10px;",
                                        button {
                                            style: "padding: 5px 14px; font-size: 11px; font-weight: 700; border-radius: 8px; cursor: pointer; font-family: inherit; border: 1.5px solid {btn_border}; background: {btn_bg}; color: {btn_fg};",
                                            title: "Toggle chord display for all parts",
                                            onclick: move |_| {
                                                let new_val = !all_on;
                                                let mut s = act_song.write();
                                                for part in s.parts.iter_mut() {
                                                    let t = part.part_text.get_or_insert_with(Default::default);
                                                    t.show_chords = new_val;
                                                }
                                            },
                                            "{btn_label}"
                                        }
                                    }
                                }
                            }
                        }
                        if inst != Instrument::Drums {
                        div {
                            style: "margin-bottom: 20px; display: flex; align-items: center; gap: 10px;",
                            span {
                                style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;",
                                if inst == Instrument::Piano { "Transpose:" } else { "Capo:" }
                            }
                            button {
                                style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                                onclick: move |_| { if act_capo() > if inst == Instrument::Piano { -12 } else { 0 } { *act_capo.write() -= 1; } },
                                "−"
                            }
                            span {
                                style: "min-width: 52px; text-align: center; font-size: 13px; font-weight: 800; color: #0a0f1e;",
                                if act_capo() == 0 { "Off" } else if act_capo() > 0 { "+{act_capo()}" } else { "{act_capo()}" }
                            }
                            button {
                                style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                                onclick: move |_| { if act_capo() < 12 { *act_capo.write() += 1; } },
                                "+"
                            }
                            if act_capo() != 0 {
                                span {
                                    style: "font-size: 11px; color: #888; font-style: italic;",
                                    if inst == Instrument::Piano {
                                        "→ sounds in {act_song.read().apply_capo(-act_capo()).key}"
                                    } else {
                                        "→ play in {act_song.read().apply_capo(act_capo()).key}"
                                    }
                                }
                            }
                        }
                        } // end if inst != Drums
                        // Editable chord parts
                        for part_index in 0..act_song.read().parts.len() {
                            // Insert-between divider
                            {
                                let insert_index = part_index;
                                rsx! {
                                    div {
                                        key: "ins-{part_index}",
                                        style: "display: flex; align-items: center; gap: 6px; margin-bottom: 8px; opacity: 0.35;",
                                        class: "insert-divider",
                                        div { style: "flex: 1; height: 1px; background: #ddd;" }
                                        button {
                                            style: "padding: 2px 10px; background: transparent; color: #999; border: 1.5px dashed #93c5fd; border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; font-family: inherit; white-space: nowrap;",
                                            title: "Insert part here",
                                            onclick: move |_| {
                                                let mut s = act_song.write();
                                                s.parts.insert(insert_index, crate::song::SongPart::new("New Part"));
                                            },
                                            "+ Part"
                                        }
                                        if inst == Instrument::Guitar || inst == Instrument::AcousticGuitar {
                                            button {
                                                style: "padding: 2px 10px; background: transparent; color: #5c7a5c; border: 1.5px dashed #8fba8f; border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; font-family: inherit; white-space: nowrap;",
                                                title: "Insert tab here",
                                                onclick: move |_| {
                                                    let mut s = act_song.write();
                                                    s.parts.insert(insert_index, crate::song::SongPart::new_riff("Riff"));
                                                },
                                                "~ Tab"
                                            }
                                        }
                                        if inst == Instrument::Bass {
                                            button {
                                                style: "padding: 2px 10px; background: transparent; color: #7a5c5c; border: 1.5px dashed #ba8f8f; border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; font-family: inherit; white-space: nowrap;",
                                                title: "Insert bass tab here",
                                                onclick: move |_| {
                                                    let mut s = act_song.write();
                                                    s.parts.insert(insert_index, SongPart::new_bass_riff("Bass"));
                                                },
                                                "~ Bass Tab"
                                            }
                                        }
                                        if inst == Instrument::Piano {
                                            button {
                                                style: "padding: 2px 10px; background: transparent; color: #6b1a8a; border: 1.5px dashed #c4a8e8; border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; font-family: inherit; white-space: nowrap;",
                                                title: "Insert piano sheet here",
                                                onclick: move |_| {
                                                    let mut s = act_song.write();
                                                    s.parts.insert(insert_index, SongPart::new_piano_riff("Piano"));
                                                },
                                                "~ Piano Sheet"
                                            }
                                        }
                                        div { style: "flex: 1; height: 1px; background: #ddd;" }
                                    }
                                }
                            }
                            PartView { key: "{part_index}", song: act_song, part_index, notation, capo: act_capo, vocals_only: inst == Instrument::Vocals }
                        }
                        button {
                            style: "
                                margin-top: 8px;
                                margin-bottom: 16px;
                                padding: 10px 20px;
                                background: transparent;
                                color: #999;
                                border: 2px dashed #93c5fd;
                                border-radius: 10px;
                                font-size: 13px;
                                font-weight: 600;
                                cursor: pointer;
                                font-family: inherit;
                                width: 100%;
                            ",
                            onclick: move |_| { act_song.write().parts.push(crate::song::SongPart::new("New Part")); },
                            "+ Add Part"
                        }
                        if inst == Instrument::Guitar || inst == Instrument::AcousticGuitar {
                            button {
                                style: "
                                    margin-bottom: 12px;
                                    padding: 10px 20px;
                                    background: transparent;
                                    color: #5c7a5c;
                                    border: 2px dashed #8fba8f;
                                    border-radius: 10px;
                                    font-size: 13px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    font-family: inherit;
                                    width: 100%;
                                ",
                                onclick: move |_| { act_song.write().parts.push(crate::song::SongPart::new_riff("Riff")); },
                                "+ Add Tab"
                            }
                        }
                        if inst == Instrument::Bass {
                            button {
                                style: "
                                    margin-bottom: 12px;
                                    padding: 10px 20px;
                                    background: transparent;
                                    color: #7a5c5c;
                                    border: 2px dashed #ba8f8f;
                                    border-radius: 10px;
                                    font-size: 13px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    font-family: inherit;
                                    width: 100%;
                                ",
                                onclick: move |_| { act_song.write().parts.push(SongPart::new_bass_riff("Bass")); },
                                "+ Add Bass Tab"
                            }
                        }
                        if inst == Instrument::Piano {
                            button {
                                style: "
                                    margin-bottom: 12px;
                                    padding: 10px 20px;
                                    background: transparent;
                                    color: #6b1a8a;
                                    border: 2px dashed #c4a8e8;
                                    border-radius: 10px;
                                    font-size: 13px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    font-family: inherit;
                                    width: 100%;
                                ",
                                onclick: move |_| { act_song.write().parts.push(SongPart::new_piano_riff("Piano")); },
                                "+ Add Piano Sheet"
                            }
                        }
                        if inst == Instrument::Drums {
                            button {
                                style: "
                                    margin-bottom: 12px;
                                    padding: 10px 20px;
                                    background: transparent;
                                    color: #7a5a10;
                                    border: 2px dashed #d4b040;
                                    border-radius: 10px;
                                    font-size: 13px;
                                    font-weight: 600;
                                    cursor: pointer;
                                    font-family: inherit;
                                    width: 100%;
                                ",
                                onclick: move |_| { act_song.write().parts.push(SongPart::new_drum_beat("Beat")); },
                                "+ Add Beat"
                            }
                        }
                        div {
                            style: "margin-bottom: 24px; display: flex; align-items: center; gap: 16px;",
                            button {
                                style: "
                                    padding: 12px 28px;
                                    background: {accent};
                                    color: #fff;
                                    border: none;
                                    border-radius: 10px;
                                    font-size: 14px;
                                    font-weight: 800;
                                    cursor: pointer;
                                    font-family: inherit;
                                ",
                                onclick: move |_| {
                                    let label = inst.label().to_string();
                                    let inst_parts = act_song.read().parts.clone();
                                    let cap = act_capo();
                                    let mut updated = song.read().clone();
                                    updated.instrument_parts.insert(label.clone(), inst_parts);
                                    updated.instrument_capos.insert(label, cap);
                                    let user_id = current_user.read().as_ref().map(|u| u.id).unwrap_or(0);
                                    if let Some(db_ref) = db.read().as_ref() {
                                        match db_ref.save_song(&updated, user_id) {
                                            Ok(_) => {
                                                *song.write() = updated;
                                                *inst_save_msg.write() = Some("✅ Saved!");
                                            }
                                            Err(_) => *inst_save_msg.write() = Some("❌ Save failed"),
                                        }
                                    } else {
                                        *song.write() = updated;
                                        *inst_save_msg.write() = Some("✅ Saved!");
                                    }
                                },
                                "💾  Save {inst_label} Sheet"
                            }
                            if let Some(msg) = inst_save_msg() {
                                span { style: "font-size: 13px; font-weight: 600; color: #555;", "{msg}" }
                            }
                        }
                    }
                }
            }
            } // end if !render_mode() parts editor

            // ── Vocals / notes ────────────────────────────────────────────────
            div {
                style: "
                    margin-bottom: 24px;
                    border: 1.5px solid #dbeafe;
                    border-radius: 12px;
                    overflow: hidden;
                ",
                div {
                    style: "
                        display: flex;
                        align-items: center;
                        gap: 8px;
                        padding: 10px 16px;
                        background: #eff6ff;
                        border-bottom: 1.5px solid #dbeafe;
                    ",
                    span { style: "font-size: 18px; line-height: 1;", "\u{1F3A4}" }
                    span {
                        style: "font-size: 11px; font-weight: 700; color: #888; text-transform: uppercase; letter-spacing: 1.2px;",
                        "Vocals / Notes"
                    }
                }
                textarea {
                    style: "
                        display: block;
                        width: 100%;
                        min-height: 90px;
                        padding: 12px 16px;
                        font-size: 14px;
                        color: #444;
                        background: #fff;
                        border: none;
                        outline: none;
                        font-family: inherit;
                        resize: vertical;
                        box-sizing: border-box;
                        line-height: 1.6;
                    ",
                    placeholder: "Add lyrics, vocal notes, cues\u{2026}",
                    value: "{song.read().vocals_notes}",
                    oninput: move |e| song.write().vocals_notes = e.value(),
                }
            }

            // ── PDF font sizes ────────────────────────────────────────────────
            div {
                style: "margin-top: 24px; display: flex; align-items: center; gap: 16px; flex-wrap: wrap;",

                span {
                    style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;",
                    "PDF font sizes:"
                }

                // Part label size
                div {
                    style: "display: flex; align-items: center; gap: 6px;",
                    span { style: "font-size: 12px; color: #666; font-weight: 600;", "Part labels" }
                    input {
                        r#type: "number",
                        min: "6", max: "24",
                        style: "
                            width: 52px;
                            font-size: 13px;
                            font-weight: 600;
                            color: #0a0f1e;
                            text-align: center;
                            border: 1px solid #bfdbfe;
                            border-radius: 6px;
                            background: #fff;
                            outline: none;
                            padding: 4px 6px;
                            font-family: inherit;
                        ",
                        value: {
                            let key = (*active_instrument.read()).map(|i| i.label().to_string()).unwrap_or_else(|| "Base".to_string());
                            song.read().pdf_settings.get(&key).map(|p| p.part_name_size).unwrap_or(9).to_string()
                        },
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                let key = (*active_instrument.read()).map(|i| i.label().to_string()).unwrap_or_else(|| "Base".to_string());
                                let cs = song.read().pdf_settings.get(&key).map(|p| p.chord_size).unwrap_or(18);
                                song.write().pdf_settings.insert(key, song::PdfSettings { part_name_size: v.clamp(6, 24), chord_size: cs });
                            }
                        }
                    }
                    span { style: "font-size: 11px; color: #aaa;", "pt" }
                }

                // Chord size
                div {
                    style: "display: flex; align-items: center; gap: 6px;",
                    span { style: "font-size: 12px; color: #666; font-weight: 600;", "Chords" }
                    input {
                        r#type: "number",
                        min: "10", max: "36",
                        style: "
                            width: 52px;
                            font-size: 13px;
                            font-weight: 600;
                            color: #0a0f1e;
                            text-align: center;
                            border: 1px solid #bfdbfe;
                            border-radius: 6px;
                            background: #fff;
                            outline: none;
                            padding: 4px 6px;
                            font-family: inherit;
                        ",
                        value: {
                            let key = (*active_instrument.read()).map(|i| i.label().to_string()).unwrap_or_else(|| "Base".to_string());
                            song.read().pdf_settings.get(&key).map(|p| p.chord_size).unwrap_or(18).to_string()
                        },
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                let key = (*active_instrument.read()).map(|i| i.label().to_string()).unwrap_or_else(|| "Base".to_string());
                                let pns = song.read().pdf_settings.get(&key).map(|p| p.part_name_size).unwrap_or(9);
                                song.write().pdf_settings.insert(key, song::PdfSettings { part_name_size: pns, chord_size: v.clamp(10, 36) });
                            }
                        }
                    }
                    span { style: "font-size: 11px; color: #aaa;", "pt" }
                }
            }

            // ── Preview button ────────────────────────────────────────────────
            button {
                style: "
                    margin-top: 24px;
                    width: 100%;
                    padding: 14px;
                    background: #eff6ff;
                    color: #0a0f1e;
                    border: 2px solid #bfdbfe;
                    border-radius: 10px;
                    font-size: 15px;
                    font-weight: 700;
                    letter-spacing: 0.6px;
                    cursor: pointer;
                    font-family: inherit;
                ",
                onclick: move |_| {
                    let base = song.read().clone();
                    let (preview_song, capo_val) = match *active_instrument.read() {
                        None => (base.clone(), 0_i8),
                        Some(Instrument::Guitar) => {
                            let parts = guitar_song.read().parts.clone();
                            (Song { parts, ..base.clone() }, guitar_capo())
                        }
                        Some(Instrument::AcousticGuitar) => {
                            let parts = acoustic_song.read().parts.clone();
                            (Song { parts, ..base.clone() }, acoustic_capo())
                        }
                        Some(Instrument::Bass) => {
                            let parts = bass_song.read().parts.clone();
                            (Song { parts, ..base.clone() }, bass_capo())
                        }
                        Some(Instrument::Piano) => {
                            let parts = piano_song.read().parts.clone();
                            (Song { parts, ..base.clone() }, piano_capo())
                        }
                        Some(Instrument::Drums) => {
                            let parts = drums_song.read().parts.clone();
                            (Song { parts, ..base.clone() }, 0_i8)
                        }
                        Some(Instrument::Vocals) => {
                            let parts = vocals_song.read().parts.clone();
                            (Song { parts, ..base.clone() }, vocals_capo())
                        }
                    };
                    let note = notation();
                    let pdf_key = (*active_instrument.read()).map(|i| i.label().to_string()).unwrap_or_else(|| "Base".to_string());
                    let pdf_s = song.read().pdf_settings.get(&pdf_key).cloned().unwrap_or_default();
                    let pns = pdf_s.part_name_size as f32;
                    let cs  = pdf_s.chord_size as f32;
                    use js_sys::Uint8Array;
                    use web_sys::{Blob, BlobPropertyBag, Url};
                    match pdf::generate_pdf_bytes(&preview_song, note, pns, cs, capo_val) {
                        Ok(bytes) => {
                            let array = Uint8Array::from(bytes.as_slice());
                            let parts = js_sys::Array::new();
                            parts.push(&array);
                            let opts = BlobPropertyBag::new();
                            opts.set_type("application/pdf");
                            if let Ok(blob) = Blob::new_with_u8_array_sequence_and_options(&parts, &opts) {
                                if let Ok(url) = Url::create_object_url_with_blob(&blob) {
                                    *preview_url.write() = url;
                                    *preview_open.write() = true;
                                }
                            }
                        }
                        Err(e) => web_sys::console::error_1(&format!("Preview failed: {e}").into()),
                    }
                },
                "🔍  Preview PDF"
            }

            // ── Export button ─────────────────────────────────────────────────
            button {
                style: "
                    margin-top: 32px;
                    width: 100%;
                    padding: 15px;
                    background: #2563eb;
                    color: #ffffff;
                    border: none;
                    border-radius: 10px;
                    font-size: 15px;
                    font-weight: 700;
                    letter-spacing: 0.6px;
                    cursor: pointer;
                    font-family: inherit;
                ",
                onclick: move |_| {
                    let s    = song.read().clone();
                    let note = notation();

                    // Collect: base sheet + one entry per instrument that has saved overrides.
                    // Each entry is (song_with_correct_parts, filename, capo_for_that_sheet, pns, cs).
                    let mut exports: Vec<(Song, String, i8, f32, f32)> = Vec::new();
                    let base_pdf = s.pdf_settings.get("Base").cloned().unwrap_or_default();
                    exports.push((s.clone(), s.name.clone(), 0_i8, base_pdf.part_name_size as f32, base_pdf.chord_size as f32));
                    for inst in Instrument::all() {
                        if let Some(parts) = s.instrument_parts.get(inst.label()).cloned() {
                            let inst_cap = *s.instrument_capos.get(inst.label()).unwrap_or(&0);
                            let inst_pdf = s.pdf_settings.get(inst.label()).cloned().unwrap_or_default();
                            let mut inst_sheet = s.clone();
                            inst_sheet.parts = parts;
                            let filename = format!("{}_{}", s.name, inst.label());
                            exports.push((inst_sheet, filename, inst_cap, inst_pdf.part_name_size as f32, inst_pdf.chord_size as f32));
                        }
                    }

                    {
                        use js_sys::Uint8Array;
                        use wasm_bindgen::JsCast;
                        use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};
                        use std::io::Write;

                        // Build a ZIP containing base.pdf + one pdf per instrument override.
                        // A single download is the only approach that works in Safari
                        // (which blocks downloads not triggered directly by a user gesture).
                        let mut zip_buf: Vec<u8> = Vec::new();
                        {
                            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
                            let opts = zip::write::SimpleFileOptions::default()
                                .compression_method(zip::CompressionMethod::Deflated);

                            for (sheet, filename, sheet_cap, pns, cs) in &exports {
                                match pdf::generate_pdf_bytes(sheet, note, *pns, *cs, *sheet_cap) {
                                    Ok(bytes) => {
                                        let _ = zip.start_file(format!("{filename}.pdf"), opts);
                                        let _ = zip.write_all(&bytes);
                                    }
                                    Err(e) => web_sys::console::error_1(
                                        &format!("PDF generation failed for {filename}: {e}").into(),
                                    ),
                                }
                            }
                            let _ = zip.finish();
                        }

                        let array = Uint8Array::from(zip_buf.as_slice());
                        let parts = js_sys::Array::new();
                        parts.push(&array);
                        let opts = BlobPropertyBag::new();
                        opts.set_type("application/zip");
                        let blob =
                            Blob::new_with_u8_array_sequence_and_options(&parts, &opts).expect("blob");
                        let url = Url::create_object_url_with_blob(&blob).expect("url");
                        let window = web_sys::window().expect("window");
                        let document = window.document().expect("document");
                        let a: HtmlAnchorElement = document
                            .create_element("a").expect("a")
                            .dyn_into().expect("cast");
                        a.set_href(&url);
                        a.set_download(&format!("{}.zip", s.name));
                        a.click();
                        let _ = Url::revoke_object_url(&url);
                    }
                },
                "📄  Export as PDF"
            }

            // ── Save to DB button ─────────────────────────────────────────
            button {
                style: "
                    margin-top: 12px;
                    width: 100%;
                    padding: 15px;
                    background: #2d6a4f;
                    color: #ffffff;
                    border: none;
                    border-radius: 10px;
                    font-size: 15px;
                    font-weight: 700;
                    letter-spacing: 0.6px;
                    cursor: pointer;
                    font-family: inherit;
                ",
                onclick: move |_| {
                    let s       = song.read().clone();
                    let note    = notation();
                    let user_id = current_user.read().as_ref().map(|u| u.id).unwrap_or(0);
                    let base_pdf = s.pdf_settings.get("Base").cloned().unwrap_or_default();
                    let pns = base_pdf.part_name_size as f32;
                    let cs  = base_pdf.chord_size as f32;
                    if let Some(db_ref) = db.read().as_ref() {
                        match db_ref.save_song(&s, user_id) {
                            Ok(song_id) => {
                                println!("✅  Song saved (id={song_id})");
                                // Also generate and store the current PDF
                                match pdf::generate_pdf_bytes(&s, note, pns, cs, 0_i8) {
                                    Ok(bytes) => match db_ref.save_pdf(song_id, &bytes) {
                                        Ok(pdf_id) => println!("✅  PDF stored (id={pdf_id})"),
                                        Err(e) => eprintln!("❌  PDF store failed: {e}"),
                                    },
                                    Err(e) => eprintln!("❌  PDF generation failed: {e}"),
                                }
                            }
                            Err(e) => eprintln!("❌  Save failed: {e}"),
                        }
                    }
                },
                "💾  Save to Library"
            }

            // ── PDF Preview modal ─────────────────────────────────────────
            if preview_open() {
                div {
                    style: "
                        position: fixed;
                        inset: 0;
                        background: rgba(0,0,0,0.65);
                        z-index: 1000;
                        display: flex;
                        flex-direction: column;
                        align-items: center;
                        justify-content: center;
                        padding: 24px;
                    ",
                    onclick: move |_| *preview_open.write() = false,

                    div {
                        style: "
                            background: #fff;
                            border-radius: 14px;
                            width: min(92vw, 860px);
                            height: min(90vh, 1100px);
                            display: flex;
                            flex-direction: column;
                            overflow: hidden;
                            box-shadow: 0 24px 64px rgba(0,0,0,0.45);
                        ",
                        onclick: move |e| e.stop_propagation(),

                        // Header bar
                        div {
                            style: "
                                display: flex;
                                align-items: center;
                                justify-content: space-between;
                                padding: 14px 20px;
                                border-bottom: 1px solid #dbeafe;
                                flex-shrink: 0;
                            ",
                            span {
                                style: "font-size: 15px; font-weight: 700; color: #0a0f1e; font-family: inherit;",
                                "PDF Preview"
                            }
                            button {
                                style: "
                                    background: none;
                                    border: none;
                                    font-size: 22px;
                                    color: #888;
                                    cursor: pointer;
                                    line-height: 1;
                                    padding: 0 4px;
                                    font-family: inherit;
                                ",
                                onclick: move |_| {
                                    use web_sys::Url;
                                    let url = preview_url.read().clone();
                                    if !url.is_empty() {
                                        let _ = Url::revoke_object_url(&url);
                                    }
                                    *preview_url.write() = String::new();
                                    *preview_open.write() = false;
                                },
                                "\u{2715}"
                            }
                        }

                        // PDF iframe
                        iframe {
                            style: "flex: 1; border: none; width: 100%;",
                            src: "{preview_url}",
                        }
                    }
                }
            }
        }
    }
}

// ── Login / Register screen ───────────────────────────────────────────────────

#[component]
fn LoginScreen(
    db: Signal<Option<Db>>,
    mut current_user: Signal<Option<User>>,
    mut screen: Signal<AppScreen>,
) -> Element {
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error_msg: Signal<String> = use_signal(String::new);

    // Initialise form mode from the screen that opened us.
    let start_register = *screen.read() == AppScreen::Register;
    let mut is_register = use_signal(move || start_register);

    let form_title = if is_register() {
        "Create your account"
    } else {
        "Welcome back"
    };
    let submit_label = if is_register() {
        "Create account"
    } else {
        "Log in"
    };
    let switch_label = if is_register() {
        "Already have an account? Log in"
    } else {
        "No account yet? Register"
    };

    rsx! {
        div {
            style: "
                background: #ffffff;
                border-radius: 20px;
                padding: 44px 48px;
                box-shadow: 0 4px 32px rgba(37,99,235,0.12), 0 1px 4px rgba(0,0,0,0.06);
                border: 1.5px solid #dbeafe;
                width: 400px;
                display: flex;
                flex-direction: column;
                gap: 16px;
            ",

            // Back to landing
            button {
                style: "
                    align-self: flex-start;
                    background: none;
                    border: none;
                    cursor: pointer;
                    font-family: inherit;
                    font-size: 13px;
                    color: #6b7280;
                    padding: 0 0 4px 0;
                    display: flex;
                    align-items: center;
                    gap: 4px;
                ",
                onclick: move |_| *screen.write() = AppScreen::Landing,
                "← Back"
            }

            // Logo + name
            div {
                style: "display: flex; align-items: center; gap: 10px; margin-bottom: 4px;",
                img {
                    src: LOGO.to_string(),
                    style: "width: 44px; height: 44px; object-fit: contain;",
                    alt: "SheetWave"
                }
                span {
                    style: "font-size: 20px; font-weight: 900; color: #0a0f1e; letter-spacing: -0.3px;",
                    "SheetWave"
                }
            }

            h2 {
                style: "margin: 0 0 2px; font-size: 22px; font-weight: 800; color: #0a0f1e;",
                "{form_title}"
            }
            p {
                style: "margin: 0 0 8px; font-size: 14px; color: #6b7280;",
                if is_register() {
                    "Join SheetWave and start building your song library."
                } else {
                    "Sign in to access your song library."
                }
            }

            // Email
            input {
                style: "
                    width: 100%; padding: 12px 14px; font-size: 14px;
                    border: 1.5px solid #dbeafe; border-radius: 8px;
                    outline: none; font-family: inherit; box-sizing: border-box;
                    color: #0a0f1e;
                ",
                r#type: "email",
                placeholder: "Email address",
                value: "{email}",
                oninput: move |e| {
                    *email.write() = e.value();
                    *error_msg.write() = String::new();
                },
            }

            // Password
            input {
                style: "
                    width: 100%; padding: 12px 14px; font-size: 14px;
                    border: 1.5px solid #dbeafe; border-radius: 8px;
                    outline: none; font-family: inherit; box-sizing: border-box;
                    color: #0a0f1e;
                ",
                r#type: "password",
                placeholder: "Password",
                value: "{password}",
                oninput: move |e| {
                    *password.write() = e.value();
                    *error_msg.write() = String::new();
                },
            }

            // Error message
            if !error_msg.read().is_empty() {
                p {
                    style: "margin: 0; color: #c0392b; font-size: 13px; font-weight: 600;",
                    "{error_msg}"
                }
            }

            // Submit
            button {
                style: "
                    padding: 14px; background: #2563eb; color: #ffffff;
                    border: none; border-radius: 10px; font-size: 15px;
                    font-weight: 700; cursor: pointer; font-family: inherit;
                    letter-spacing: 0.4px;
                ",
                onclick: move |_| {
                    let u = email.read().trim().to_lowercase();
                    let p = password.read().clone();
                    if u.is_empty() || p.is_empty() {
                        *error_msg.write() = "Please fill in all fields.".into();
                        return;
                    }
                    // Validate email format: must contain '@' and a '.' after it.
                    let at = u.find('@');
                    let valid_email = at
                        .map(|i| u[i + 1..].contains('.'))
                        .unwrap_or(false);
                    if !valid_email {
                        *error_msg.write() = "Please enter a valid email address.".into();
                        return;
                    }
                    if let Some(db_ref) = db.read().as_ref() {
                        if is_register() {
                            match db_ref.create_user(&u, &p) {
                                Ok(id) => {
                                    *current_user.write() =
                                        Some(User { id, username: u });
                                }
                                Err(e) => *error_msg.write() = e.to_string(),
                            }
                        } else {
                            match db_ref.verify_user(&u, &p) {
                                Ok(Some(user)) => *current_user.write() = Some(user),
                                Ok(None) => {
                                    *error_msg.write() =
                                        "Invalid email or password.".into();
                                }
                                Err(e) => *error_msg.write() = e.to_string(),
                            }
                        }
                    }
                },
                "{submit_label}"
            }

            // Switch between login / register
            button {
                style: "
                    background: none; border: none; cursor: pointer;
                    font-family: inherit; font-size: 13px; color: #888;
                    text-decoration: underline; padding: 0;
                ",
                onclick: move |_| {
                    *is_register.write() = !is_register();
                    *error_msg.write() = String::new();
                },
                "{switch_label}"
            }
        }
    }
}

// ── Piano sheet-music editor ──────────────────────────────────────────────────

#[component]
fn PianoSheetEditor(song: Signal<Song>, part_index: usize) -> Element {
    let mut editing_col: Signal<Option<usize>> = use_signal(|| None);
    let mut edit_rh: Signal<String> = use_signal(String::new);
    let mut edit_lh: Signal<String> = use_signal(String::new);
    let mut edit_dur: Signal<u8> = use_signal(|| 4u8);
    let mut editing_time_sig: Signal<bool> = use_signal(|| false);
    let mut edit_time_sig_buf: Signal<String> = use_signal(String::new);

    // ── Layout constants (px) ────────────────────────────────────────────────
    const LG: f32 = 7.0; // gap between adjacent staff lines
    const LP: f32 = 11.0; // ledger-line padding above/below the 5 lines
    const SH: f32 = 28.0; // staff height (LG × 4)
    const SSH: f32 = 50.0; // per-staff section height  (LP + SH + LP)
    const SG: f32 = 16.0; // gap between treble and bass staves
    const GH: f32 = 116.0 + 14.0; // SVG height (SSH×2 + SG + label row)
    const CW: f32 = 60.0; // clef area width
    const BW: f32 = 34.0; // note-column width
    const BLW: f32 = 14.0; // barline-column width
    const NRX: f32 = 5.5; // note-head x radius
    const NRY: f32 = 3.5; // note-head y radius
    const TREBLE_TOP: f32 = 0.0;
    const BASS_TOP: f32 = SSH + SG; // = 66.0
    const STEM_LEN: f32 = 26.0; // stem length in px

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Parse a single note name like "C4", "F#5", "Bb3" → SVG y-coordinate.
    fn note_svg_y(name: &str, is_treble: bool) -> Option<f32> {
        let b = name.trim().as_bytes();
        if b.is_empty() {
            return None;
        }
        let letter = (b[0] as char).to_ascii_uppercase();
        let diatonic: i32 = match letter {
            'C' => 0,
            'D' => 1,
            'E' => 2,
            'F' => 3,
            'G' => 4,
            'A' => 5,
            'B' => 6,
            _ => return None,
        };
        let acc_skip: usize = if b.len() > 1 && (b[1] == b'#' || b[1] == b'b') {
            1
        } else {
            0
        };
        let oct: i32 = std::str::from_utf8(&b[1 + acc_skip..])
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let abs_pos = oct * 7 + diatonic;
        let bottom_abs: i32 = if is_treble { 30 } else { 18 };
        let steps = abs_pos - bottom_abs;
        let section_top = if is_treble { TREBLE_TOP } else { BASS_TOP };
        let bottom_y = section_top + LP + SH;
        Some(bottom_y - steps as f32 * (LG / 2.0))
    }

    /// Parse a comma-separated chord string → Vec of (y, accidental_str).
    fn chord_notes(chord_str: &str, is_treble: bool) -> Vec<(f32, &'static str)> {
        chord_str
            .split(',')
            .filter_map(|n| {
                let n = n.trim();
                let y = note_svg_y(n, is_treble)?;
                let b = n.as_bytes();
                let acc: &'static str = if b.len() >= 2 {
                    match b[1] {
                        b'#' => "♯",
                        b'b' => "♭",
                        _ => "",
                    }
                } else {
                    ""
                };
                Some((y, acc))
            })
            .collect()
    }

    /// Collect all unique ledger-line y positions needed for a set of notes.
    fn chord_ledger_ys(notes: &[(f32, &str)], is_treble: bool) -> Vec<f32> {
        let section_top = if is_treble { TREBLE_TOP } else { BASS_TOP };
        let top_line_y = section_top + LP;
        let bot_line_y = section_top + LP + SH;
        let mut set: Vec<f32> = Vec::new();
        for &(ny, _) in notes {
            let mut ly = bot_line_y + LG;
            while ly <= ny + 0.5 {
                if !set.iter().any(|&v: &f32| (v - ly).abs() < 0.1) {
                    set.push(ly);
                }
                ly += LG;
            }
            let mut ly = top_line_y - LG;
            while ly >= ny - 0.5 {
                if !set.iter().any(|&v: &f32| (v - ly).abs() < 0.1) {
                    set.push(ly);
                }
                ly -= LG;
            }
        }
        set
    }

    // ── Build segments (split at LineBreak columns) ──────────────────────────
    let mut segments: Vec<Vec<usize>> = vec![vec![]];
    let mut lb_indices: Vec<usize> = vec![];
    {
        let p = song.read();
        if let Some(part) = p.parts.get(part_index) {
            for (i, col) in part.tab_grid.iter().enumerate() {
                match col {
                    TabCol::LineBreak => {
                        lb_indices.push(i);
                        segments.push(vec![]);
                    }
                    _ => segments.last_mut().unwrap().push(i),
                }
            }
        }
    }
    let seg_count = segments.len();
    let song_key: String = song.read().key.clone();
    let time_sig: String = song
        .read()
        .parts
        .get(part_index)
        .map(|p| p.time_sig.clone())
        .unwrap_or_else(|| "4/4".to_string());
    let ts_parts: Vec<String> = time_sig.splitn(2, '/').map(|s| s.to_string()).collect();
    let ts_top: String = ts_parts.first().cloned().unwrap_or_else(|| "4".to_string());
    let ts_bot: String = ts_parts.get(1).cloned().unwrap_or_else(|| "4".to_string());

    // ── Key-signature accidentals ─────────────────────────────────────────────
    // Returns (symbol, treble_steps, bass_steps) per accidental in standard order.
    // Steps are diatonic positions from the BOTTOM staff line upward
    //   (0 = bottom line, 1 = 1st space, 2 = 2nd line, …)
    // Each step = LG/2 in SVG pixels.  Negative = below bottom line.
    //
    // Treble bottom = E4.   Notes: E=0 F=1 G=2 A=3 B=4 C=5 D=6 E=7 F=8 G=9 A=10 …
    // Bass   bottom = G2.   Notes: G=0 A=1 B=2 C=3 D=4 E=5 F=6 G=7 A=8 …
    //
    // Sharp order (F C G D A E B) — traditional positions:
    //   treble: F5=8  C5=5  G5=9  D5=6  A5=10  E5=7  B4=4
    //   bass  : F3=6  C3=3  G3=7  D3=4  A3=8   E3=5  B2=2
    // Flat order  (B E A D G C F):
    //   treble: B4=4  E5=7  A4=3  D5=6  G4=2  C5=5  F4=1
    //   bass  : B2=2  E3=5  A2=1  D3=4  G2=0  C3=3  F2=-1
    fn key_sig_accidentals(key: &str) -> Vec<(&'static str, i32, i32)> {
        // Key is stored as "G Major", "Bb Minor", "F#m", "C", etc.
        let key = key.trim();
        let mut parts = key.splitn(2, ' ');
        let root = parts.next().unwrap_or("").trim_end_matches('m');
        let mode_word = parts.next().unwrap_or("").to_ascii_lowercase();
        let is_minor = mode_word.starts_with("min") || key.ends_with('m');

        const SHARPS: [(&str, i32, i32); 7] = [
            ("♯", 8, 6),  // F#
            ("♯", 5, 3),  // C#
            ("♯", 9, 7),  // G#
            ("♯", 6, 4),  // D#
            ("♯", 10, 8), // A#
            ("♯", 7, 5),  // E#
            ("♯", 4, 2),  // B#
        ];
        const FLATS: [(&str, i32, i32); 7] = [
            ("♭", 4, 2),  // Bb
            ("♭", 7, 5),  // Eb
            ("♭", 3, 1),  // Ab
            ("♭", 6, 4),  // Db
            ("♭", 2, 0),  // Gb
            ("♭", 5, 3),  // Cb
            ("♭", 1, -1), // Fb
        ];

        let (count, use_sharps) = if is_minor {
            match root {
                "A" => (0, true),
                "E" => (1, true),
                "B" => (2, true),
                "F#" => (3, true),
                "C#" => (4, true),
                "G#" => (5, true),
                "D#" => (6, true),
                "A#" => (7, true),
                "D" => (1, false),
                "G" => (2, false),
                "C" => (3, false),
                "F" => (4, false),
                "Bb" => (5, false),
                "Eb" => (6, false),
                "Ab" => (7, false),
                _ => (0, true),
            }
        } else {
            match root {
                "C" => (0, true),
                "G" => (1, true),
                "D" => (2, true),
                "A" => (3, true),
                "E" => (4, true),
                "B" => (5, true),
                "F#" => (6, true),
                "C#" => (7, true),
                "F" => (1, false),
                "Bb" => (2, false),
                "Eb" => (3, false),
                "Ab" => (4, false),
                "Db" => (5, false),
                "Gb" => (6, false),
                "Cb" => (7, false),
                _ => (0, true),
            }
        };

        if count == 0 {
            vec![]
        } else if use_sharps {
            SHARPS[..count.min(7)].to_vec()
        } else {
            FLATS[..count.min(7)].to_vec()
        }
    }
    let key_acc = key_sig_accidentals(&song_key);
    let key_acc_count = key_acc.len();
    const ACC_START_X: f32 = 32.0; // x of first accidental (right after clef)
    const ACC_STEP_X: f32 = 10.0; // horizontal spacing
    let ks_width = key_acc_count as f32 * ACC_STEP_X;
    let ts_x_actual = ACC_START_X + ks_width + 12.0; // time-sig shifts right
    let extra_cw = ks_width + if key_acc_count > 0 { 14.0 } else { 0.0 };
    // Build raw SVG markup for key-sig accidentals (injected via dangerous_inner_html)
    let ks_svg_html: String = key_acc.iter().enumerate().map(|(ai, &(sym, t_steps, b_steps))| {
        let ax = ACC_START_X + ai as f32 * ACC_STEP_X;
        let ty = TREBLE_TOP + LP + SH - t_steps as f32 * (LG / 2.0) + 4.0;
        let by = BASS_TOP   + LP + SH - b_steps as f32 * (LG / 2.0) + 4.0;
        format!(
            r##"<text x="{ax:.1}" y="{ty:.1}" font-size="13" font-family="serif" font-weight="bold" fill="#111" text-anchor="middle">{sym}</text><text x="{ax:.1}" y="{by:.1}" font-size="13" font-family="serif" font-weight="bold" fill="#111" text-anchor="middle">{sym}</text>"##
        )
    }).collect();

    rsx! {
        div {
            style: "display: flex; flex-direction: column; gap: 28px; padding: 4px 0;",

            for seg_idx in 0..seg_count {
                {
                    let seg_cols = segments[seg_idx].clone();
                    let has_lb_before = seg_idx > 0;
                    let lb_col = if has_lb_before { lb_indices[seg_idx - 1] } else { 0 };
                    let is_last_seg = seg_idx + 1 == seg_count;

                    #[derive(Clone)]
                    struct ColLayout { col_idx: usize, x_center: f32, width: f32, is_barline: bool, rh: String, lh: String, dur: u8 }

                    let eff_cw = CW + extra_cw;
                    let col_layouts: Vec<ColLayout> = {
                        let p = song.read();
                        let mut x = eff_cw;
                        seg_cols.iter().map(|&ci| {
                            let is_bl = matches!(p.parts.get(part_index).and_then(|p| p.tab_grid.get(ci)), Some(TabCol::Barline));
                            let w = if is_bl { BLW } else { BW };
                            let xc = x + w / 2.0;
                            let (rh, lh, dur) = if !is_bl {
                                if let Some(TabCol::Notes(arr)) = p.parts.get(part_index).and_then(|p| p.tab_grid.get(ci)) {
                                    let r = match &arr[0] { TabCell::Custom(s) => s.clone(), _ => String::new() };
                                    let l = match &arr[1] { TabCell::Custom(s) => s.clone(), _ => String::new() };
                                    let d: u8 = match &arr[2] { TabCell::Custom(s) => s.parse().unwrap_or(4), _ => 4 };
                                    (r, l, d)
                                } else { (String::new(), String::new(), 4u8) }
                            } else { (String::new(), String::new(), 4u8) };
                            let cl = ColLayout { col_idx: ci, x_center: xc, width: w, is_barline: is_bl, rh, lh, dur };
                            x += w;
                            cl
                        }).collect()
                    };
                    let content_w: f32 = col_layouts.iter().map(|cl| cl.width).sum::<f32>();
                    let svg_w = eff_cw + content_w + 10.0;

                    rsx! {
                        // ── Linebreak separator ──────────────────────────────────
                        if has_lb_before {
                            div {
                                key: "lb-{seg_idx}",
                                style: "display: flex; align-items: center; gap: 8px;",
                                div { style: "height: 1px; flex: 1; background: #c4a8e8;" }
                                button {
                                    style: "font-size: 11px; color: #9b6fc4; background: none; border: 1px dashed #c8a8e8; border-radius: 4px; padding: 1px 8px; cursor: pointer; font-family: inherit;",
                                    title: "Remove line break",
                                    onclick: move |_| {
                                        if let Some(part) = song.write().parts.get_mut(part_index) {
                                            if lb_col < part.tab_grid.len() { part.tab_grid.remove(lb_col); }
                                        }
                                    },
                                    "↵ ×"
                                }
                                div { style: "height: 1px; flex: 1; background: #c4a8e8;" }
                            }
                        }

                        // ── Grand staff block ────────────────────────────────────
                        div {
                            key: "seg-{seg_idx}",
                            style: "display: inline-block; background: #f8f3fd; border: 1.5px solid #c4a8e8; border-radius: 8px; padding: 12px 16px 10px; position: relative;",

                            // Delete-column buttons
                            div {
                                style: "display: flex; align-items: center; padding-left: {eff_cw}px; margin-bottom: 2px; min-height: 14px;",
                                for col_i in 0..col_layouts.len() {
                                    {
                                        let cl = col_layouts[col_i].clone();
                                        let ci = cl.col_idx;
                                        rsx! {
                                            div {
                                                key: "del-{ci}",
                                                style: "width: {cl.width}px; display: flex; justify-content: center;",
                                                button {
                                                    style: "background: none; border: none; font-size: 10px; color: #ccc; cursor: pointer; padding: 0; line-height: 1; font-family: inherit;",
                                                    title: "Remove",
                                                    onclick: move |_| {
                                                        if let Some(part) = song.write().parts.get_mut(part_index) {
                                                            if ci < part.tab_grid.len() { part.tab_grid.remove(ci); }
                                                        }
                                                        editing_col.set(None);
                                                    },
                                                    "×"
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // ── SVG grand staff ──────────────────────────────────
                            svg {
                                width: "{svg_w}",
                                height: "{GH}",
                                style: "display: block; overflow: visible;",

                                // Grand-staff brace
                                line {
                                    x1: "3", y1: "{TREBLE_TOP + LP}",
                                    x2: "3", y2: "{BASS_TOP + LP + SH}",
                                    stroke: "#333", stroke_width: "3"
                                }
                                line {
                                    x1: "{eff_cw - 5.0}", y1: "{TREBLE_TOP + LP}",
                                    x2: "{eff_cw - 5.0}", y2: "{BASS_TOP + LP + SH}",
                                    stroke: "#444", stroke_width: "1.5"
                                }

                                // Treble staff lines
                                for li in 0usize..5 {
                                    line {
                                        key: "tsl-{li}",
                                        x1: "4", y1: "{TREBLE_TOP + LP + li as f32 * LG}",
                                        x2: "{svg_w - 4.0}", y2: "{TREBLE_TOP + LP + li as f32 * LG}",
                                        stroke: "#333", stroke_width: "1.0"
                                    }
                                }
                                // Bass staff lines
                                for li in 0usize..5 {
                                    line {
                                        key: "bsl-{li}",
                                        x1: "4", y1: "{BASS_TOP + LP + li as f32 * LG}",
                                        x2: "{svg_w - 4.0}", y2: "{BASS_TOP + LP + li as f32 * LG}",
                                        stroke: "#333", stroke_width: "1.0"
                                    }
                                }

                                // Treble clef 𝄞
                                text {
                                    x: "5", y: "{TREBLE_TOP + LP + SH + LG * 0.5}",
                                    font_size: "46", font_family: "serif", fill: "#222",
                                    "𝄞"
                                }
                                // Bass clef 𝄢
                                text {
                                    x: "6", y: "{BASS_TOP + LP + LG * 2.2}",
                                    font_size: "26", font_family: "serif", fill: "#222",
                                    "𝄢"
                                }

                                // Key-signature accidentals (first segment only)
                                if seg_idx == 0 && !ks_svg_html.is_empty() {
                                    g {
                                        key: "ks",
                                        dangerous_inner_html: "{ks_svg_html}"
                                    }
                                }

                                // Time signature (first segment only)
                                if seg_idx == 0 {
                                    {
                                        let ts_click = time_sig.clone();
                                        rsx! {
                                            g {
                                                style: "cursor: pointer;",
                                                onclick: move |_| {
                                                    edit_time_sig_buf.set(ts_click.clone());
                                                    editing_time_sig.set(true);
                                                },
                                                // numerator (treble staff centre)
                                                text {
                                                    x: "{ts_x_actual}", y: "{TREBLE_TOP + LP + LG}",
                                                    font_size: "15", font_family: "sans-serif",
                                                    font_weight: "700", fill: "#333",
                                                    text_anchor: "middle", "{ts_top}"
                                                }
                                                text {
                                                    x: "{ts_x_actual}", y: "{TREBLE_TOP + LP + SH}",
                                                    font_size: "15", font_family: "sans-serif",
                                                    font_weight: "700", fill: "#333",
                                                    text_anchor: "middle", "{ts_bot}"
                                                }
                                                // same for bass staff
                                                text {
                                                    x: "{ts_x_actual}", y: "{BASS_TOP + LP + LG}",
                                                    font_size: "15", font_family: "sans-serif",
                                                    font_weight: "700", fill: "#333",
                                                    text_anchor: "middle", "{ts_top}"
                                                }
                                                text {
                                                    x: "{ts_x_actual}", y: "{BASS_TOP + LP + SH}",
                                                    font_size: "15", font_family: "sans-serif",
                                                    font_weight: "700", fill: "#333",
                                                    text_anchor: "middle", "{ts_bot}"
                                                }
                                            }
                                        }
                                    }
                                }

                                // Closing barline
                                line {
                                    x1: "{svg_w - 5.0}", y1: "{TREBLE_TOP + LP}",
                                    x2: "{svg_w - 5.0}", y2: "{BASS_TOP + LP + SH}",
                                    stroke: "#444", stroke_width: "1.5"
                                }

                                // ── Per-column rendering ─────────────────────────
                                for col_i in 0..col_layouts.len() {
                                    {
                                        let cl = col_layouts[col_i].clone();
                                        let ci = cl.col_idx;
                                        let x = cl.x_center;

                                        if cl.is_barline {
                                            rsx! {
                                                line {
                                                    key: "bline-{ci}",
                                                    x1: "{x}", y1: "{TREBLE_TOP + LP}",
                                                    x2: "{x}", y2: "{BASS_TOP + LP + SH}",
                                                    stroke: "#555", stroke_width: "1.5"
                                                }
                                            }
                                        } else {
                                            let is_editing = *editing_col.read() == Some(ci);
                                            // Parse comma-separated chords
                                            let rh_notes: Vec<(f32, &'static str)> = chord_notes(&cl.rh, true);
                                            let lh_notes: Vec<(f32, &'static str)> = chord_notes(&cl.lh, false);
                                            let rh_ledgers = chord_ledger_ys(&rh_notes, true);
                                            let lh_ledgers = chord_ledger_ys(&lh_notes, false);
                                            let rect_fill = if is_editing { "#e8d0f8" } else { "transparent" };
                                            let cur_rh = cl.rh.clone();
                                            let cur_lh = cl.lh.clone();
                                            let cur_dur = cl.dur;
                                            let note_fill = if cl.dur == 1 || cl.dur == 2 { "white" } else { "#1a0a3a" };
                                            let dur_sym = match cl.dur { 1 => "𝅝", 2 => "𝅗𝅥", 8 => "♪", _ => "♩" };
                                            // For staggering overlapping note heads
                                            let rh_ys: Vec<f32> = rh_notes.iter().map(|&(y,_)| y).collect();
                                            let lh_ys: Vec<f32> = lh_notes.iter().map(|&(y,_)| y).collect();
                                            let rh_top_y = rh_notes.iter().map(|&(y,_)| y).fold(f32::INFINITY, f32::min);
                                            let lh_bot_y = lh_notes.iter().map(|&(y,_)| y).fold(f32::NEG_INFINITY, f32::max);

                                            rsx! {
                                                g {
                                                    key: "col-{ci}",
                                                    style: "cursor: pointer;",
                                                    onclick: move |_| {
                                                        edit_rh.set(cur_rh.clone());
                                                        edit_lh.set(cur_lh.clone());
                                                        edit_dur.set(cur_dur);
                                                        editing_col.set(Some(ci));
                                                    },

                                                    // Highlight rect
                                                    rect {
                                                        x: "{x - BW / 2.0}", y: "{TREBLE_TOP}",
                                                        width: "{BW}", height: "{GH}",
                                                        fill: "{rect_fill}", fill_opacity: "0.45", rx: "3"
                                                    }

                                                    // Beat guide lines (dashed)
                                                    line {
                                                        x1: "{x}", y1: "{TREBLE_TOP + LP - 1.0}",
                                                        x2: "{x}", y2: "{TREBLE_TOP + LP + SH + 1.0}",
                                                        stroke: "#d0b8f0", stroke_width: "0.7",
                                                        stroke_dasharray: "2,3"
                                                    }
                                                    line {
                                                        x1: "{x}", y1: "{BASS_TOP + LP - 1.0}",
                                                        x2: "{x}", y2: "{BASS_TOP + LP + SH + 1.0}",
                                                        stroke: "#d0b8f0", stroke_width: "0.7",
                                                        stroke_dasharray: "2,3"
                                                    }

                                                    // ── Treble (RH) chord ────────────────────────
                                                    // Ledger lines first
                                                    for &ly in rh_ledgers.iter() {
                                                        line {
                                                            key: "rhl-{ly as i32}",
                                                            x1: "{x - NRX - 4.0}", y1: "{ly}",
                                                            x2: "{x + NRX + 4.0}", y2: "{ly}",
                                                            stroke: "#333", stroke_width: "1.0"
                                                        }
                                                    }
                                                    // Note heads
                                                    for (ni, &(ty, acc)) in rh_notes.iter().enumerate() {
                                                        {
                                                            // Stagger adjacent note heads (interval of a 2nd = LG/2 apart)
                                                            let stagger = rh_ys.iter().enumerate()
                                                                .any(|(j, &oy)| j < ni && (ty - oy).abs() < LG / 2.0 + 1.0);
                                                            let nx = if stagger { x + NRX + 1.0 } else { x };
                                                            rsx! {
                                                                g { key: "rhn-{ni}",
                                                                    if !acc.is_empty() {
                                                                        text {
                                                                            x: "{nx - NRX - 6.0}", y: "{ty + 4.0}",
                                                                            font_size: "11", font_family: "serif",
                                                                            fill: "#222", text_anchor: "middle",
                                                                            "{acc}"
                                                                        }
                                                                    }
                                                                    ellipse {
                                                                        cx: "{nx}", cy: "{ty}",
                                                                        rx: "{NRX}", ry: "{NRY}",
                                                                        fill: "{note_fill}", stroke: "#1a0a3a", stroke_width: "1.5",
                                                                        transform: "rotate(-15 {nx} {ty})"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    // RH stem (up) + eighth flag
                                                    if !rh_notes.is_empty() && cur_dur != 1 {
                                                        {
                                                            let sx = x + NRX;
                                                            let sy_top = rh_top_y - STEM_LEN;
                                                            rsx! {
                                                                line {
                                                                    key: "rh-stem",
                                                                    x1: "{sx}", y1: "{rh_top_y}",
                                                                    x2: "{sx}", y2: "{sy_top}",
                                                                    stroke: "#1a0a3a", stroke_width: "1.5"
                                                                }
                                                                if cur_dur == 8 {
                                                                    path {
                                                                        key: "rh-flag",
                                                                        d: "M {sx} {sy_top} C {sx+12.0} {sy_top+6.0}, {sx+10.0} {sy_top+18.0}, {sx+2.0} {sy_top+22.0}",
                                                                        stroke: "#1a0a3a", stroke_width: "1.5", fill: "none"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }

                                                    // ── Bass (LH) chord ──────────────────────────
                                                    // Ledger lines
                                                    for &ly in lh_ledgers.iter() {
                                                        line {
                                                            key: "lhl-{ly as i32}",
                                                            x1: "{x - NRX - 4.0}", y1: "{ly}",
                                                            x2: "{x + NRX + 4.0}", y2: "{ly}",
                                                            stroke: "#333", stroke_width: "1.0"
                                                        }
                                                    }
                                                    // Note heads
                                                    for (ni, &(by, acc)) in lh_notes.iter().enumerate() {
                                                        {
                                                            let stagger = lh_ys.iter().enumerate()
                                                                .any(|(j, &oy)| j < ni && (by - oy).abs() < LG / 2.0 + 1.0);
                                                            let nx = if stagger { x + NRX + 1.0 } else { x };
                                                            rsx! {
                                                                g { key: "lhn-{ni}",
                                                                    if !acc.is_empty() {
                                                                        text {
                                                                            x: "{nx - NRX - 6.0}", y: "{by + 4.0}",
                                                                            font_size: "11", font_family: "serif",
                                                                            fill: "#222", text_anchor: "middle",
                                                                            "{acc}"
                                                                        }
                                                                    }
                                                                    ellipse {
                                                                        cx: "{nx}", cy: "{by}",
                                                                        rx: "{NRX}", ry: "{NRY}",
                                                                        fill: "{note_fill}", stroke: "#1a0a3a", stroke_width: "1.5",
                                                                        transform: "rotate(-15 {nx} {by})"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    // LH stem (down) + eighth flag
                                                    if !lh_notes.is_empty() && cur_dur != 1 {
                                                        {
                                                            let sx = x - NRX;
                                                            let sy_bot = lh_bot_y + STEM_LEN;
                                                            rsx! {
                                                                line {
                                                                    key: "lh-stem",
                                                                    x1: "{sx}", y1: "{lh_bot_y}",
                                                                    x2: "{sx}", y2: "{sy_bot}",
                                                                    stroke: "#1a0a3a", stroke_width: "1.5"
                                                                }
                                                                if cur_dur == 8 {
                                                                    path {
                                                                        key: "lh-flag",
                                                                        d: "M {sx} {sy_bot} C {sx+12.0} {sy_bot-6.0}, {sx+10.0} {sy_bot-18.0}, {sx+2.0} {sy_bot-22.0}",
                                                                        stroke: "#1a0a3a", stroke_width: "1.5", fill: "none"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    // Duration label below staff
                                                    text {
                                                        key: "dur-lbl",
                                                        x: "{x}", y: "{BASS_TOP + LP + SH + 14.0}",
                                                        font_size: "12", font_family: "serif",
                                                        fill: "#9060b8", text_anchor: "middle",
                                                        "{dur_sym}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // ── Edit popup ───────────────────────────────────────
                            {
                                let ec = *editing_col.read();
                                if ec.is_some() && ec.is_some_and(|ci| seg_cols.contains(&ci)) {
                                    let ci = ec.unwrap();
                                    rsx! {
                                        div {
                                            style: "margin-top: 10px; padding: 8px 14px; background: #ede0f8; border: 1.5px solid #b890d8; border-radius: 8px; display: flex; align-items: center; gap: 12px; flex-wrap: wrap;",
                                            div {
                                                style: "display: flex; flex-direction: column; gap: 5px;",
                                                // RH input
                                                div {
                                                    style: "display: flex; align-items: center; gap: 8px;",
                                                    span { style: "font-size: 18px; font-family: serif; color: #6b1a8a; width: 22px;", "𝄞" }
                                                    span { style: "font-size: 10px; font-weight: 700; color: #888; width: 24px;", "RH" }
                                                    input {
                                                        style: "width: 160px; font-family: 'Courier New', monospace; font-size: 13px; border: 1.5px solid #b890d8; border-radius: 5px; padding: 3px 7px; background: white; outline: none;",
                                                        r#type: "text",
                                                        placeholder: "e.g. C5,E5,G5",
                                                        value: "{edit_rh}",
                                                        oninput: move |e| edit_rh.set(e.value()),
                                                        onkeydown: move |e: Event<KeyboardData>| {
                                                            if e.key() == Key::Enter { editing_col.set(None); }
                                                        },
                                                    }
                                                }
                                                // LH input
                                                div {
                                                    style: "display: flex; align-items: center; gap: 8px;",
                                                    span { style: "font-size: 18px; font-family: serif; color: #6b1a8a; width: 22px;", "𝄢" }
                                                    span { style: "font-size: 10px; font-weight: 700; color: #888; width: 24px;", "LH" }
                                                    input {
                                                        style: "width: 160px; font-family: 'Courier New', monospace; font-size: 13px; border: 1.5px solid #b890d8; border-radius: 5px; padding: 3px 7px; background: white; outline: none;",
                                                        r#type: "text",
                                                        placeholder: "e.g. C3,G3",
                                                        value: "{edit_lh}",
                                                        oninput: move |e| edit_lh.set(e.value()),
                                                        onkeydown: move |e: Event<KeyboardData>| {
                                                            if e.key() == Key::Enter { editing_col.set(None); }
                                                        },
                                                    }
                                                }
                                                // Duration selector
                                                div {
                                                    style: "display: flex; align-items: center; gap: 6px; margin-top: 4px;",
                                                    span { style: "font-size: 10px; font-weight: 700; color: #888; width: 50px;", "Duration" }
                                                    for (label, val) in [("1", 1u8), ("1/2", 2u8), ("1/4", 4u8), ("1/8", 8u8)] {
                                                        {
                                                            let is_sel = *edit_dur.read() == val;
                                                            rsx! {
                                                                button {
                                                                    key: "dur-{val}",
                                                                    style: if is_sel {
                                                                        "padding: 2px 9px; background: #6b1a8a; color: white; border: 1.5px solid #6b1a8a; border-radius: 4px; font-size: 12px; cursor: pointer; font-family: inherit; font-weight: 700;"
                                                                    } else {
                                                                        "padding: 2px 9px; background: white; color: #6b1a8a; border: 1.5px solid #b890d8; border-radius: 4px; font-size: 12px; cursor: pointer; font-family: inherit;"
                                                                    },
                                                                    onclick: move |_| edit_dur.set(val),
                                                                    "{label}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                span {
                                                    style: "font-size: 10px; color: #a078c0; margin-top: 2px;",
                                                    "Separate notes with commas for chords"
                                                }
                                            }
                                            button {
                                                style: "padding: 6px 14px; background: #6b1a8a; color: white; border: none; border-radius: 6px; font-size: 13px; font-weight: 700; cursor: pointer; font-family: inherit;",
                                                onclick: move |_| {
                                                    let rh_val = edit_rh.read().trim().to_string();
                                                    let lh_val = edit_lh.read().trim().to_string();
                                                    if let Some(part) = song.write().parts.get_mut(part_index) {
                                                        if let Some(TabCol::Notes(arr)) = part.tab_grid.get_mut(ci) {
                                                            arr[0] = if rh_val.is_empty() { TabCell::Empty } else { TabCell::Custom(rh_val) };
                                                            arr[1] = if lh_val.is_empty() { TabCell::Empty } else { TabCell::Custom(lh_val) };
                                                            arr[2] = TabCell::Custom(edit_dur.read().to_string());
                                                        }
                                                    }
                                                    editing_col.set(None);
                                                },
                                                "✓ Apply"
                                            }
                                            button {
                                                style: "padding: 6px 10px; background: transparent; color: #888; border: 1px solid #ccc; border-radius: 6px; font-size: 12px; cursor: pointer; font-family: inherit;",
                                                onclick: move |_| { editing_col.set(None); },
                                                "✕"
                                            }
                                        }
                                    }
                                } else { rsx! {} }
                            }

                            // ── Add-beat / barline / linebreak buttons ────────────
                            if is_last_seg {
                                // Time-sig editor (opens when time-sig is clicked)
                                if *editing_time_sig.read() {
                                    div {
                                        style: "display: flex; align-items: center; gap: 8px; margin-top: 8px;",
                                        span { style: "font-size: 11px; font-weight: 700; color: #888;", "Time sig:" }
                                        input {
                                            style: "width: 60px; font-family: 'Courier New', monospace; font-size: 13px; border: 1.5px solid #b890d8; border-radius: 5px; padding: 2px 6px; background: white; outline: none; text-align: center;",
                                            r#type: "text",
                                            placeholder: "4/4",
                                            value: "{edit_time_sig_buf}",
                                            oninput: move |e| edit_time_sig_buf.set(e.value()),
                                            onkeydown: move |e: Event<KeyboardData>| {
                                                if e.key() == Key::Enter {
                                                    let v = edit_time_sig_buf.read().trim().to_string();
                                                    if !v.is_empty() {
                                                        if let Some(part) = song.write().parts.get_mut(part_index) {
                                                            part.time_sig = v;
                                                        }
                                                    }
                                                    editing_time_sig.set(false);
                                                }
                                            },
                                        }
                                        button {
                                            style: "padding: 2px 10px; background: #6b1a8a; color: white; border: none; border-radius: 5px; font-size: 12px; cursor: pointer; font-family: inherit;",
                                            onclick: move |_| {
                                                let v = edit_time_sig_buf.read().trim().to_string();
                                                if !v.is_empty() {
                                                    if let Some(part) = song.write().parts.get_mut(part_index) {
                                                        part.time_sig = v;
                                                    }
                                                }
                                                editing_time_sig.set(false);
                                            },
                                            "✓"
                                        }
                                        button {
                                            style: "padding: 2px 8px; background: transparent; color: #888; border: 1px solid #ccc; border-radius: 5px; font-size: 12px; cursor: pointer; font-family: inherit;",
                                            onclick: move |_| editing_time_sig.set(false),
                                            "✕"
                                        }
                                    }
                                }
                                div {
                                    style: "display: flex; gap: 4px; margin-top: 8px;",
                                    button {
                                        style: "background: none; border: 1px dashed #b890d8; border-radius: 4px; font-size: 12px; color: #6b1a8a; cursor: pointer; padding: 2px 10px; font-family: inherit;",
                                        title: "Add 8 beats",
                                        onclick: move |_| {
                                            if let Some(part) = song.write().parts.get_mut(part_index) {
                                                for _ in 0..8 {
                                                    part.tab_grid.push(TabCol::Notes(std::array::from_fn(|_| TabCell::Empty)));
                                                }
                                            }
                                        },
                                        "+ 8 beats"
                                    }
                                    button {
                                        style: "background: none; border: 1px dashed #a0b4cc; border-radius: 4px; font-size: 13px; font-weight: 700; color: #6a7fa6; cursor: pointer; padding: 1px 9px; font-family: Courier, monospace;",
                                        title: "Add barline",
                                        onclick: move |_| {
                                            if let Some(part) = song.write().parts.get_mut(part_index) {
                                                part.tab_grid.push(TabCol::Barline);
                                            }
                                        },
                                        "|"
                                    }
                                    button {
                                        style: "background: none; border: 1px dashed #c8a8e8; border-radius: 4px; font-size: 12px; color: #9b6fc4; cursor: pointer; padding: 1px 7px; font-family: inherit;",
                                        title: "Add line break (new row)",
                                        onclick: move |_| {
                                            if let Some(part) = song.write().parts.get_mut(part_index) {
                                                part.tab_grid.push(TabCol::LineBreak);
                                            }
                                        },
                                        "↵"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Tab grid editor ───────────────────────────────────────────────────────────

#[component]
fn TabEditor(
    song: Signal<Song>,
    part_index: usize,
    bass: bool,
    drums: bool,
    #[props(default = false)] piano: bool,
) -> Element {
    let mut editing: Signal<Option<(usize, usize)>> = use_signal(|| None);
    let mut edit_buf: Signal<String> = use_signal(String::new);

    const STRING_NAMES_GUITAR: [&str; 6] = ["e", "B", "G", "D", "A", "E"];
    const STRING_NAMES_BASS: [&str; 4] = ["G", "D", "A", "E"];
    #[allow(dead_code)]
    const STRING_NAMES_DRUMS: [&str; 8] = ["K", "S", "Hi", "R", "C", "T1", "T2", "T3"];
    const STRING_NAMES_PIANO: [&str; 2] = ["RH", "LH"];
    let num_strings: usize = if bass {
        4
    } else if drums {
        8
    } else if piano {
        2
    } else {
        6
    };
    // For bass: indices 2-5 of the 6-cell array map to G,D,A,E
    let str_offset: usize = if bass { 2 } else { 0 };
    // Colour theme: green for guitar/bass, amber for drums, purple for piano.
    let (grid_line_color, grid_border_color, grid_bg_color, lbl_color) = if drums {
        ("#c8a840", "#d4b040", "#fffbf0", "#7a5a10")
    } else if piano {
        ("#c4a8e8", "#b890d8", "#f8f3fd", "#6b1a8a")
    } else {
        ("#aac8aa", "#b5d6b5", "#f6fbf6", "#5c7a5c")
    };

    // Pre-compute segments: each is a Vec of global column indices (skipping LineBreaks).
    // lb_indices[i] = global index of the LineBreak that follows segment i.
    let mut segments: Vec<Vec<usize>> = vec![vec![]];
    let mut lb_indices: Vec<usize> = vec![];
    {
        let p = song.read();
        if let Some(part) = p.parts.get(part_index) {
            for (i, col) in part.tab_grid.iter().enumerate() {
                if matches!(col, TabCol::LineBreak) {
                    lb_indices.push(i);
                    segments.push(vec![]);
                } else {
                    segments.last_mut().unwrap().push(i);
                }
            }
        }
    }
    let seg_count = segments.len();

    rsx! {
        div {
            style: "overflow-x: auto; padding: 4px 0; display: flex; flex-direction: column; gap: 6px;",

            for seg_idx in 0..seg_count {
                {
                    let seg_cols: Vec<usize> = segments[seg_idx].clone();
                    let seg_len = seg_cols.len();
                    let has_lb_before = seg_idx > 0;
                    let lb_col = if has_lb_before { lb_indices[seg_idx - 1] } else { 0 };
                    let is_last_seg = seg_idx + 1 == seg_count;

                    rsx! {
                        // ── Line-break separator ──────────────────────────────────
                        if has_lb_before {
                            div {
                                key: "lb-{seg_idx}",
                                style: "display: flex; align-items: center; gap: 8px;",
                                div { style: "height: 1px; flex: 1; background: {grid_border_color};" }
                                button {
                                    style: "font-size: 11px; color: #9b6fc4; background: none; border: 1px dashed #c8a8e8; border-radius: 4px; padding: 1px 8px; cursor: pointer; font-family: inherit;",
                                    title: "Remove line break",
                                    onclick: move |_| {
                                        if let Some(part) = song.write().parts.get_mut(part_index) {
                                            if lb_col < part.tab_grid.len() {
                                                part.tab_grid.remove(lb_col);
                                            }
                                        }
                                    },
                                    "\u{21b5} \u{00d7}"
                                }
                                div { style: "height: 1px; flex: 1; background: {grid_border_color};" }
                            }
                        }

                        // ── Segment block ─────────────────────────────────────────
                        div {
                            key: "seg-{seg_idx}",
                            style: "display: inline-block; background: {grid_bg_color}; border: 1.5px solid {grid_border_color}; border-radius: 8px; padding: 10px 14px 12px;",

                            // Header row: delete buttons + (on last segment) add buttons
                            div {
                                style: "display: flex; align-items: center; margin-bottom: 2px; padding-left: 6px;",

                                for local_i in 0..seg_len {
                                    {
                                        let col = seg_cols[local_i];
                                        let is_barline = song
                                            .read()
                                            .parts
                                            .get(part_index)
                                            .and_then(|p| p.tab_grid.get(col))
                                            .map(|c| matches!(c, TabCol::Barline))
                                            .unwrap_or(false);
                                        let w = if is_barline { "18px" } else { "36px" };
                                        rsx! {
                                            div {
                                                key: "hd-{col}",
                                                style: "width: {w}; display: flex; justify-content: center;",
                                                button {
                                                    style: "background: none; border: none; font-size: 10px; color: #ccc; cursor: pointer; padding: 0; line-height: 1; font-family: inherit;",
                                                    title: "Remove",
                                                    onclick: move |_| {
                                                        if let Some(part) = song.write().parts.get_mut(part_index) {
                                                            if col < part.tab_grid.len() {
                                                                part.tab_grid.remove(col);
                                                            }
                                                        }
                                                        editing.set(None);
                                                    },
                                                    "\u{2715}"
                                                }
                                            }
                                        }
                                    }
                                }

                                if is_last_seg {
                                    div {
                                        style: "display: flex; gap: 4px; margin-left: 6px;",
                                        button {
                                            style: "background: none; border: 1px dashed {grid_border_color}; border-radius: 4px; font-size: 12px; color: {lbl_color}; cursor: pointer; padding: 1px 7px; font-family: inherit;",
                                            title: "Add 4 beats",
                                            onclick: move |_| {
                                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                                    for _ in 0..4 {
                                                        part.tab_grid.push(TabCol::Notes(std::array::from_fn(|_| TabCell::Empty)));
                                                    }
                                                }
                                            },
                                            "+"
                                        }
                                        button {
                                            style: "background: none; border: 1px dashed #a0b4cc; border-radius: 4px; font-size: 13px; font-weight: 700; color: #6a7fa6; cursor: pointer; padding: 1px 8px; font-family: Courier, monospace;",
                                            title: "Add barline",
                                            onclick: move |_| {
                                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                                    part.tab_grid.push(TabCol::Barline);
                                                }
                                            },
                                            "|"
                                        }
                                        button {
                                            style: "background: none; border: 1px dashed #c8a8e8; border-radius: 4px; font-size: 12px; color: #9b6fc4; cursor: pointer; padding: 1px 7px; font-family: inherit;",
                                            title: "Add line break (new row)",
                                            onclick: move |_| {
                                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                                    part.tab_grid.push(TabCol::LineBreak);
                                                }
                                            },
                                            "\u{21b5}"
                                        }
                                    }
                                }
                            }

                            // ── String name column header ─────────────────────────
                            div {
                                style: "display: flex; padding-left: 0px; margin-bottom: 1px;",
                                // spacer matching the label column width
                                div { style: "display: flex; flex-direction: column; margin-right: 8px;",
                                    for si in 0usize..num_strings {
                                        div {
                                            key: "lbl-{seg_idx}-{si}",
                                            style: "height: 32px; width: 20px; display: flex; align-items: center; justify-content: flex-end;",
                                            span {
                                                style: "font-family: Courier, monospace; font-size: 12px; font-weight: 700; color: {lbl_color};",
                                                if bass {
                                                    "{STRING_NAMES_BASS[si]}"
                                                } else if drums {
                                                    "{STRING_NAMES_DRUMS[si]}"
                                                } else if piano {
                                                    "{STRING_NAMES_PIANO[si]}"
                                                } else {
                                                    "{STRING_NAMES_GUITAR[si]}"
                                                }
                                            }
                                        }
                                    }
                                }
                                // the actual grid columns
                                div {
                                    style: "display: flex; flex-direction: column;",

                                    // ── String rows ───────────────────────────────────────
                                    for str_local in 0usize..num_strings {
                                        {
                                        let str_idx = str_local + str_offset;
                                        rsx! {
                                        div {
                                            key: "row-{seg_idx}-{str_local}",
                                            style: "display: flex; align-items: center; height: 32px;",

                                            div { style: "width: 6px; height: 2px; background: #aac8aa; flex-shrink: 0;" }

                                    for local_i in 0..seg_len {
                                        {
                                            let col = seg_cols[local_i];
                                            let is_barline = song
                                                .read()
                                                .parts
                                                .get(part_index)
                                                .and_then(|p| p.tab_grid.get(col))
                                                .map(|c| matches!(c, TabCol::Barline))
                                                .unwrap_or(false);

                                            if is_barline {
                                                rsx! {
                                                    div {
                                                        key: "{col}",
                                                        style: "position:relative;width:18px;height:32px;display:flex;align-items:center;justify-content:center;flex-shrink:0;",
                                                        div { style: "position:absolute;top:50%;left:0;right:0;height:2px;background:#aac8aa;transform:translateY(-50%);z-index:0;" }
                                                        div { style: "position:absolute;top:2px;bottom:2px;left:50%;width:2px;background:#6a7fa6;border-radius:1px;transform:translateX(-50%);z-index:1;" }
                                                    }
                                                }
                                            } else {
                                                let is_editing_cell = matches!(
                                                    *editing.read(),
                                                    Some((c, s)) if c == col && s == str_idx
                                                );
                                                let cell: TabCell = song
                                                    .read()
                                                    .parts
                                                    .get(part_index)
                                                    .and_then(|p| p.tab_grid.get(col))
                                                    .and_then(|cd| {
                                                        if let TabCol::Notes(arr) = cd {
                                                            Some(arr[str_idx].clone())
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                    .unwrap_or(TabCell::Empty);
                                                let (cell_label, label_color, cell_bg) = if drums {
                                                    match &cell {
                                                        TabCell::Empty     => ("\u{00b7}".to_string(), "#c8a840", "background:transparent;"),
                                                        TabCell::Muted     => ("x".to_string(),  "#2a2a2a", "background:#e8d8a0;"),
                                                        TabCell::Custom(s) => {
                                                            let (c, bg) = match s.as_str() {
                                                                "X"       => ("#8a1a1a", "background:#fde8e8;"),
                                                                "o" | "O" => ("#1a5c5c", "background:#d0f0ec;"),
                                                                "f"       => ("#6b1a8a", "background:#f0e8f8;"),
                                                                _         => ("#2a2a2a", "background:#e8d8a0;"),
                                                            };
                                                            (s.clone(), c, bg)
                                                        }
                                                        TabCell::Fret(n) => (n.to_string(), "#2a2a2a", "background:#e8d8a0;"),
                                                        _                => ("?".to_string(), "#2a2a2a", "background:#e8d8a0;"),
                                                    }
                                                } else if piano {
                                                    match cell {
                                                    TabCell::Empty => ("\u{2013}".to_string(), "#c4a8e8", "background:transparent;"),
                                                    TabCell::Muted => ("x".to_string(), "#c0392b", "background:#fde8e8;"),
                                                    TabCell::Fret(n) => (n.to_string(), "#3a0a5a", "background:#e8d8f8;"),
                                                    TabCell::Ghost(n) => (format!("({n})"), "#7b5ea7", "background:#f0e8f8;"),
                                                    TabCell::Custom(ref s) => (s.clone(), "#3a0a5a", "background:#ede0f8;"),
                                                    _ => ("?".to_string(), "#3a0a5a", "background:#ede0f8;"),
                                                    }
                                                } else {
                                                    match cell {
                                                    TabCell::Fret(n) => (n.to_string(), "#0a0f1e", "background:#d8edd8;"),
                                                    TabCell::Muted => ("x".to_string(), "#c0392b", "background:#fde8e8;"),
                                                    TabCell::Empty => ("\u{2013}".to_string(), "#c8dcc8", "background:transparent;"),
                                                    TabCell::Ghost(n) => (format!("({n})"), "#7b5ea7", "background:#f0e8f8;"),
                                                    TabCell::HammerOn  => ("h".to_string(),  "#1a6b3c", "background:#d4f0e0;"),
                                                    TabCell::PullOff   => ("p".to_string(),  "#1a6b3c", "background:#d4f0e0;"),
                                                    TabCell::Release   => ("r".to_string(),  "#7a5c1e", "background:#fef3d0;"),
                                                    TabCell::Bend      => ("b".to_string(),  "#7a5c1e", "background:#fef3d0;"),
                                                    TabCell::SlideUp   => ("/".to_string(),  "#1a4a8a", "background:#d8e8f8;"),
                                                    TabCell::SlideDown => ("\\".to_string(), "#1a4a8a", "background:#d8e8f8;"),
                                                    TabCell::Custom(ref s) => (s.clone(), "#0a0f1e", "background:#eff6ff;"),
                                                    }
                                                };
                                                let has_value = !matches!(cell, TabCell::Empty);
                                                let fw = if has_value { "700" } else { "400" };
                                                let cell_style = format!("position:relative;z-index:1;width:30px;height:26px;display:flex;align-items:center;justify-content:center;cursor:pointer;user-select:none;border-radius:4px;{cell_bg}");
                                                let label_style = format!("font-family:Courier,monospace;font-size:13px;font-weight:{fw};color:{label_color};");

                                                if is_editing_cell {
                                                    rsx! {
                                                        div {
                                                            key: "{col}",
                                                            style: "position:relative;width:36px;height:32px;display:flex;align-items:center;justify-content:center;flex-shrink:0;",
                                                            div { style: "position:absolute;top:50%;left:0;right:0;height:2px;background:{grid_line_color};transform:translateY(-50%);z-index:0;" }
                                                            input {
                                                                style: "position:relative;z-index:1;width:30px;height:26px;font-family:Courier,monospace;font-size:13px;font-weight:700;text-align:center;border:2px solid {lbl_color};border-radius:4px;background:{grid_bg_color};outline:none;padding:0;box-sizing:border-box;",
                                                                r#type: "text",
                                                                maxlength: "8",
                                                                autofocus: true,
                                                                value: "{edit_buf}",
                                                                oninput: move |e| { edit_buf.set(e.value()); },
                                                                onblur: move |_| {
                                                                    let raw = edit_buf.read();
                                                                    let val = raw.trim();
                                                                    let new_cell = if val.eq_ignore_ascii_case("x") {
                                                                        TabCell::Muted
                                                                    } else if val.eq_ignore_ascii_case("h") {
                                                                        TabCell::HammerOn
                                                                    } else if val.eq_ignore_ascii_case("p") {
                                                                        TabCell::PullOff
                                                                    } else if val.eq_ignore_ascii_case("r") {
                                                                        TabCell::Release
                                                                    } else if val.eq_ignore_ascii_case("b") {
                                                                        TabCell::Bend
                                                                    } else if val == "/" {
                                                                        TabCell::SlideUp
                                                                    } else if val == "\\" {
                                                                        TabCell::SlideDown
                                                                    } else if val.starts_with('(') && val.ends_with(')') {
                                                                        let inner = &val[1..val.len()-1];
                                                                        if let Ok(n) = inner.parse::<u8>().map(|n| n.min(24)) {
                                                                            TabCell::Ghost(n)
                                                                        } else {
                                                                            TabCell::Empty
                                                                        }
                                                                    } else if let Some(n) =
                                                                        val.parse::<u8>().ok().filter(|&n| n <= 24)
                                                                    {
                                                                        TabCell::Fret(n)
                                                                    } else if val.is_empty() {
                                                                        TabCell::Empty
                                                                    } else {
                                                                        TabCell::Custom(val.to_string())
                                                                    };
                                                                    if let Some(part) = song.write().parts.get_mut(part_index) {
                                                                        if let Some(TabCol::Notes(arr)) = part.tab_grid.get_mut(col) {
                                                                            arr[str_idx] = new_cell;
                                                                        }
                                                                    }
                                                                    editing.set(None);
                                                                },
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    rsx! {
                                                        div {
                                                            key: "{col}",
                                                            style: "position:relative;width:36px;height:32px;display:flex;align-items:center;justify-content:center;flex-shrink:0;",
                                                            div { style: "position:absolute;top:50%;left:0;right:0;height:2px;background:{grid_line_color};transform:translateY(-50%);z-index:0;" }
                                                            div {
                                                                style: "{cell_style}",
                                                                onclick: move |_| {
                                                                    let init = match cell {
                                                                        TabCell::Fret(n) => n.to_string(),
                                                                        TabCell::Muted => "x".to_string(),
                                                                        TabCell::Empty => String::new(),
                                                                        TabCell::Ghost(n) => format!("({n})"),
                                                                        TabCell::HammerOn  => "h".to_string(),
                                                                        TabCell::PullOff   => "p".to_string(),
                                                                        TabCell::Release   => "r".to_string(),
                                                                        TabCell::Bend      => "b".to_string(),
                                                                        TabCell::SlideUp   => "/".to_string(),
                                                                        TabCell::SlideDown => "\\".to_string(),
                                                                        TabCell::Custom(ref s) => s.clone(),
                                                                    };
                                                                    edit_buf.set(init);
                                                                    editing.set(Some((col, str_idx)));
                                                                },
                                                                span {
                                                                    style: "{label_style}",
                                                                    "{cell_label}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                            div { style: "width: 6px; height: 2px; background: #aac8aa; flex-shrink: 0;" }
                                        }
                                        } // end rsx!
                                        } // end let str_idx block
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Tab notation legend ───────────────────────────────────────────
            div {
                style: "margin-top: 10px; display: flex; flex-wrap: wrap; gap: 6px 14px; padding: 8px 12px; background: {grid_bg_color}; border: 1px solid {grid_border_color}; border-radius: 8px;",
                span {
                    style: "font-size: 10px; font-weight: 700; color: {lbl_color}; text-transform: uppercase; letter-spacing: 1px; width: 100%; margin-bottom: 2px;",
                    "Legend"
                }
                if drums {
                    for (sym, label) in [
                        ("x",  "Closed hit"),
                        ("X",  "Accent"),
                        ("o",  "Open / soft"),
                        ("O",  "Loud open"),
                        ("f",  "Flam"),
                    ] {
                        span {
                            style: "display: inline-flex; align-items: baseline; gap: 4px; font-size: 11px; color: #555;",
                            span {
                                style: "font-family: Courier, monospace; font-size: 12px; font-weight: 700; color: #0a0f1e; min-width: 20px;",
                                "{sym}"
                            }
                            span { "{label}" }
                        }
                    }
                } else {
                    for (sym, label) in [
                        ("(n)", "Ghost note"),
                        ("h",   "Hammer-on"),
                        ("p",   "Pull-off"),
                        ("r",   "Release"),
                        ("b",   "Bend"),
                        ("/",   "Slide up"),
                        ("\\",  "Slide down"),
                    ] {
                        span {
                            style: "display: inline-flex; align-items: baseline; gap: 4px; font-size: 11px; color: #555;",
                            span {
                                style: "font-family: Courier, monospace; font-size: 12px; font-weight: 700; color: #0a0f1e; min-width: 20px;",
                                "{sym}"
                            }
                            span { "{label}" }
                        }
                    }
                }
            }
        }
    }
}

// ── Rendered Sheet ────────────────────────────────────────────────────────────

#[component]
fn RenderedSheet(song: Signal<Song>, notation: Signal<Notation>, capo: Signal<i8>) -> Element {
    let s = song.read();
    let n = notation();
    let cap = capo();
    let title = s.name.clone();
    let artist = s.artist.clone();
    let key = s.key.clone();
    // Apply capo/transpose shift (positive = capo down, negative = transpose up)
    let effective = if cap != 0 {
        s.apply_capo(cap)
    } else {
        s.clone()
    };
    let shape_key = effective.key.clone();
    let parts = effective.parts.clone();
    drop(s);
    rsx! {
        div {
            style: "
                background: #ffffff;
                border-radius: 4px;
                box-shadow: 0 4px 32px rgba(0,0,0,0.13);
                padding: 64px 72px;
                margin-top: 24px;
                margin-bottom: 24px;
                font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
                max-width: 960px;
            ",
            // Title
            div {
                style: "font-size: 34px; font-weight: 700; color: #111; line-height: 1.2; margin-bottom: 8px;",
                "{title}"
            }
            // Artist
            if !artist.is_empty() {
                div {
                    style: "font-size: 18px; color: #333; font-style: italic; margin-bottom: 6px;",
                    "{artist}"
                }
            }
            // Key
            if !key.is_empty() {
                div {
                    style: "font-size: 14px; color: #333; margin-bottom: 4px;",
                    "Key: {key}"
                }
            }
            // Capo / Transpose annotation
            if cap > 0 {
                div {
                    style: "font-size: 14px; color: #333; margin-bottom: 4px;",
                    "Capo: fret {cap}  (shapes in {shape_key})"
                }
            }
            if cap < 0 {
                div {
                    style: "font-size: 14px; color: #333; margin-bottom: 4px;",
                    "Transpose: {cap} semitones  (sounds in {shape_key})"
                }
            }
            // Horizontal rule
            div { style: "border-bottom: 1.5px solid #bbb; margin: 20px 0 32px 0;" }
            // Parts
            for part in parts.iter() {
                div {
                    style: "margin-bottom: 44px;",
                    // Part label
                    div {
                        style: "font-size: 11px; font-weight: 700; color: #555; letter-spacing: 3px; text-transform: uppercase; margin-bottom: 14px;",
                        "{part.name}"
                    }
                    if part.part_text.as_ref().map(|t| t.show_chords && !t.content.is_empty()).unwrap_or(false) {
                        // ── Two-column: lyrics left, chords right ──────────────
                        div {
                            style: "display: flex; gap: 48px; align-items: flex-start;",
                            // Lyrics column (left)
                            if let Some(pt) = &part.part_text {
                                div {
                                    style: "flex: 1; min-width: 0; white-space: pre-wrap; font-size: {pt.size}px; color: {pt.color}; line-height: 1.8;",
                                    "{pt.content}"
                                }
                            }
                            // Chords / tab column (right)
                            div {
                                style: "flex: 1; min-width: 0;",
                                if part.kind == PartKind::Riff || part.kind == PartKind::BassRiff || part.kind == PartKind::DrumBeat || part.kind == PartKind::PianoRiff {
                                    pre {
                                        style: "font-family: 'Courier New', monospace; font-size: 15px; color: #333; white-space: pre-wrap; margin: 0;",
                                        "{part.tab_as_ascii()}"
                                    }
                                } else {
                                    {
                                        enum Row2<'a> {
                                            Chords(Vec<&'a PartItem>),
                                            Text { content: &'a str, color: &'a str },
                                        }
                                        let mut rows: Vec<Row2> = vec![Row2::Chords(vec![])];
                                        for item in &part.items {
                                            match item {
                                                PartItem::LineBreak => rows.push(Row2::Chords(vec![])),
                                                PartItem::Text { content, color } => {
                                                    if let Some(Row2::Chords(v)) = rows.last() {
                                                        if !v.is_empty() { rows.push(Row2::Chords(vec![])); }
                                                    }
                                                    rows.push(Row2::Text { content, color });
                                                    rows.push(Row2::Chords(vec![]));
                                                }
                                                _ => {
                                                    if let Some(Row2::Chords(v)) = rows.last_mut() {
                                                        v.push(item);
                                                    }
                                                }
                                            }
                                        }
                                        rsx! {
                                            div {
                                                style: "display: flex; flex-direction: column; gap: 24px;",
                                                for (ri, row) in rows.iter().enumerate() {
                                                    match row {
                                                        Row2::Text { content, color } => rsx! {
                                                            div {
                                                                key: "text-{ri}",
                                                                style: "width: 100%; font-size: 13px; font-style: italic; color: {color}; line-height: 1.5; padding: 2px 0;",
                                                                "{content}"
                                                            }
                                                        },
                                                        Row2::Chords(items) => rsx! {
                                                            div {
                                                                key: "row-{ri}",
                                                                style: "display: flex; flex-wrap: wrap; gap: 32px; align-items: baseline;",
                                                                for item in items.iter() {
                                                                    match item {
                                                                        PartItem::Chord(chord) => {
                                                                            let root = apply_notation(&chord.root, n);
                                                                            let qual = chord.quality.symbol().to_string();
                                                                            let bass = chord.bass_note.as_ref().map(|b| apply_notation(b, n));
                                                                            rsx! {
                                                                                span {
                                                                                    style: "display: inline-flex; align-items: baseline; white-space: nowrap;",
                                                                                    span { style: "font-size: 28px; font-weight: 700; color: #111; line-height: 1;", "{root}" }
                                                                                    if !qual.is_empty() {
                                                                                        sup { style: "font-size: 15px; font-weight: 600; color: #111; vertical-align: super; margin-left: 1px;", "{qual}" }
                                                                                    }
                                                                                    if let Some(b) = bass {
                                                                                        span { style: "font-size: 19px; font-weight: 500; color: #222; vertical-align: sub; margin-left: 1px;", "/{b}" }
                                                                                    }
                                                                                }
                                                                            }
                                                                        },
                                                                        PartItem::Repeat { times } => {
                                                                            let label = if *times == 0 { "||:".to_string() } else { format!("||: x{times}") };
                                                                            rsx! { span { style: "font-size: 22px; color: #888; font-weight: 700; letter-spacing: -1px; font-family: 'Courier New', monospace;", "{label}" } }
                                                                        },
                                                                        PartItem::RepeatStart => rsx! { span { style: "font-size: 22px; color: #888; font-weight: 700; letter-spacing: -1px; font-family: 'Courier New', monospace;", "||" } },
                                                                        PartItem::VoltaBracketStart { label } => rsx! {
                                                                            span {
                                                                                style: "display: inline-flex; align-items: flex-start; white-space: nowrap;",
                                                                                span { style: "font-size: 28px; font-weight: 300; color: #888; line-height: 1;", "[" }
                                                                                sup { style: "font-size: 13px; font-weight: 700; color: #555; margin-left: 1px;", "{label}" }
                                                                            }
                                                                        },
                                                                        PartItem::VoltaBracketEnd => rsx! { span { style: "font-size: 24px; font-weight: 300; color: #888;", "]" } },
                                                                        _ => rsx! { span {} },
                                                                    }
                                                                }
                                                            }
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if part.part_text.as_ref().map(|t| !t.content.is_empty()).unwrap_or(false) {
                        // ── Vocals only: lyrics replace the chords area ─────────
                        if let Some(pt) = &part.part_text {
                            div {
                                style: "white-space: pre-wrap; font-size: {pt.size}px; color: {pt.color}; line-height: 1.8;",
                                "{pt.content}"
                            }
                        }
                    } else {
                        // ── No vocals text: normal chords / tab ─────────────────
                        if part.kind == PartKind::Riff || part.kind == PartKind::BassRiff || part.kind == PartKind::DrumBeat || part.kind == PartKind::PianoRiff {
                            pre {
                                style: "font-family: 'Courier New', monospace; font-size: 15px; color: #333; white-space: pre-wrap; margin: 0;",
                                "{part.tab_as_ascii()}"
                            }
                        } else {
                            {
                                enum Row<'a> {
                                    Chords(Vec<&'a PartItem>),
                                    Text { content: &'a str, color: &'a str },
                                }
                                let mut rows: Vec<Row> = vec![Row::Chords(vec![])];
                                for item in &part.items {
                                    match item {
                                        PartItem::LineBreak => rows.push(Row::Chords(vec![])),
                                        PartItem::Text { content, color } => {
                                            if let Some(Row::Chords(v)) = rows.last() {
                                                if !v.is_empty() { rows.push(Row::Chords(vec![])); }
                                            }
                                            rows.push(Row::Text { content, color });
                                            rows.push(Row::Chords(vec![]));
                                        }
                                        _ => {
                                            if let Some(Row::Chords(v)) = rows.last_mut() {
                                                v.push(item);
                                            }
                                        }
                                    }
                                }
                                rsx! {
                                    div {
                                        style: "display: flex; flex-direction: column; gap: 24px;",
                                        for (ri, row) in rows.iter().enumerate() {
                                            match row {
                                                Row::Text { content, color } => rsx! {
                                                    div {
                                                        key: "text-{ri}",
                                                        style: "width: 100%; font-size: 13px; font-style: italic; color: {color}; line-height: 1.5; padding: 2px 0;",
                                                        "{content}"
                                                    }
                                                },
                                                Row::Chords(items) => rsx! {
                                                    div {
                                                        key: "row-{ri}",
                                                        style: "display: flex; flex-wrap: wrap; gap: 32px; align-items: baseline;",
                                                        for item in items.iter() {
                                                            match item {
                                                                PartItem::Chord(chord) => {
                                                                    let root = apply_notation(&chord.root, n);
                                                                    let qual = chord.quality.symbol().to_string();
                                                                    let bass = chord.bass_note.as_ref().map(|b| apply_notation(b, n));
                                                                    rsx! {
                                                                        span {
                                                                            style: "display: inline-flex; align-items: baseline; white-space: nowrap;",
                                                                            span { style: "font-size: 28px; font-weight: 700; color: #111; line-height: 1;", "{root}" }
                                                                            if !qual.is_empty() {
                                                                                sup { style: "font-size: 15px; font-weight: 600; color: #111; vertical-align: super; margin-left: 1px;", "{qual}" }
                                                                            }
                                                                            if let Some(b) = bass {
                                                                                span { style: "font-size: 19px; font-weight: 500; color: #222; vertical-align: sub; margin-left: 1px;", "/{b}" }
                                                                            }
                                                                        }
                                                                    }
                                                                },
                                                                PartItem::Repeat { times } => {
                                                                    let label = if *times == 0 { "||:".to_string() } else { format!("||: x{times}") };
                                                                    rsx! { span { style: "font-size: 22px; color: #888; font-weight: 700; letter-spacing: -1px; font-family: 'Courier New', monospace;", "{label}" } }
                                                                },
                                                                PartItem::RepeatStart => rsx! {
                                                                    span { style: "font-size: 22px; color: #888; font-weight: 700; letter-spacing: -1px; font-family: 'Courier New', monospace;", "||" }
                                                                },
                                                                PartItem::VoltaBracketStart { label } => rsx! {
                                                                    span {
                                                                        style: "display: inline-flex; align-items: flex-start; white-space: nowrap;",
                                                                        span { style: "font-size: 28px; font-weight: 300; color: #888; line-height: 1;", "[" }
                                                                        sup { style: "font-size: 13px; font-weight: 700; color: #555; margin-left: 1px;", "{label}" }
                                                                    }
                                                                },
                                                                PartItem::VoltaBracketEnd => rsx! {
                                                                    span { style: "font-size: 24px; font-weight: 300; color: #888;", "]" }
                                                                },
                                                                _ => rsx! { span {} },
                                                            }
                                                        }
                                                    }
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

        }
    }
}

// ── Part block ────────────────────────────────────────────────────────────────

#[component]
fn PartView(
    song: Signal<Song>,
    part_index: usize,
    notation: Signal<Notation>,
    capo: Signal<i8>,
    #[props(default = false)] vocals_only: bool,
) -> Element {
    let item_count = song
        .read()
        .parts
        .get(part_index)
        .map(|p| p.items.len())
        .unwrap_or(0);
    let part_name = song
        .read()
        .parts
        .get(part_index)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let is_riff = song
        .read()
        .parts
        .get(part_index)
        .map(|p| {
            p.kind == PartKind::Riff
                || p.kind == PartKind::BassRiff
                || p.kind == PartKind::DrumBeat
                || p.kind == PartKind::PianoRiff
        })
        .unwrap_or(false);
    let is_bass_riff = song
        .read()
        .parts
        .get(part_index)
        .map(|p| p.kind == PartKind::BassRiff)
        .unwrap_or(false);
    let is_drum_beat = song
        .read()
        .parts
        .get(part_index)
        .map(|p| p.kind == PartKind::DrumBeat)
        .unwrap_or(false);
    let is_piano_riff = song
        .read()
        .parts
        .get(part_index)
        .map(|p| p.kind == PartKind::PianoRiff)
        .unwrap_or(false);
    let show_chords_with_vocals = song
        .read()
        .parts
        .get(part_index)
        .and_then(|p| p.part_text.as_ref().map(|t| t.show_chords))
        .unwrap_or(false);

    let part_border = if is_drum_beat {
        "#d4b040"
    } else if is_piano_riff {
        "#c4a8e8"
    } else if is_riff {
        "#b5d6b5"
    } else {
        "#dbeafe"
    };
    let part_bg = if is_drum_beat {
        "#fffbf0"
    } else if is_piano_riff {
        "#f8f3fd"
    } else if is_riff {
        "#f6fbf6"
    } else {
        "#fff"
    };
    let part_name_color = if is_drum_beat {
        "#7a5a10"
    } else if is_piano_riff {
        "#6b1a8a"
    } else if is_riff {
        "#5c7a5c"
    } else {
        "#aaa"
    };
    let mut drag_source: Signal<Option<usize>> = use_signal(|| None);
    let card_style = format!(
        "margin-bottom: 32px; border: 1px solid {part_border}; border-radius: 12px; padding: 18px 20px 16px; position: relative; background: {part_bg};"
    );
    let name_input_style = format!(
        "display: block; margin: 0 0 14px 0; font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 3px; color: {part_name_color}; border: none; border-bottom: 1px dashed #ddd; background: transparent; outline: none; font-family: inherit; padding: 0 0 3px; width: calc(100% - 24px);"
    );

    rsx! {
        div {
            style: "{card_style}",

            // Part controls (move up / move down / remove)
            div {
                style: "position: absolute; top: 12px; right: 12px; display: flex; align-items: center; gap: 4px;",

                // Move up
                button {
                    style: "background: none; border: none; font-size: 13px; color: #bbb; cursor: pointer; padding: 0 2px; font-family: inherit; line-height: 1;",
                    title: "Move up",
                    disabled: part_index == 0,
                    onclick: move |_| {
                        let mut s = song.write();
                        if part_index > 0 && part_index < s.parts.len() {
                            s.parts.swap(part_index - 1, part_index);
                        }
                    },
                    "\u{2191}"
                }

                // Move down
                button {
                    style: "background: none; border: none; font-size: 13px; color: #bbb; cursor: pointer; padding: 0 2px; font-family: inherit; line-height: 1;",
                    title: "Move down",
                    disabled: {
                        let len = song.read().parts.len();
                        part_index + 1 >= len
                    },
                    onclick: move |_| {
                        let mut s = song.write();
                        if part_index + 1 < s.parts.len() {
                            s.parts.swap(part_index, part_index + 1);
                        }
                    },
                    "\u{2193}"
                }

                // Remove
                button {
                    style: "background: none; border: none; font-size: 13px; color: #ccc; cursor: pointer; padding: 0 2px; font-family: inherit; line-height: 1;",
                    title: "Remove part",
                    onclick: move |_| {
                        let mut s = song.write();
                        if part_index < s.parts.len() {
                            s.parts.remove(part_index);
                        }
                    },
                    "\u{2715}"
                }
            }

            // Editable part name
            input {
                style: "{name_input_style}",
                value: "{part_name}",
                placeholder: "Part name…",
                oninput: move |e| {
                    if let Some(part) = song.write().parts.get_mut(part_index) {
                        part.name = e.value();
                    }
                },
            }

            if is_piano_riff && (!vocals_only || show_chords_with_vocals) {
                PianoSheetEditor { song, part_index }
            }
            if is_riff && !is_piano_riff && (!vocals_only || show_chords_with_vocals) {
                TabEditor { song, part_index, bass: is_bass_riff, drums: is_drum_beat, piano: false }
            }
            if !is_riff && (!vocals_only || show_chords_with_vocals) {
                // ── Chord items + add buttons ──────────────────────────────
                div {
                    style: "display: flex; flex-wrap: wrap; gap: 10px; align-items: flex-start;",
                    ondragover: move |e: Event<DragData>| e.prevent_default(),

                    for item_index in 0..item_count {
                        {
                            let item = song
                                .read()
                                .parts
                                .get(part_index)
                                .and_then(|p| p.items.get(item_index))
                                .cloned();
                            let is_line_break = matches!(item, Some(PartItem::LineBreak));
                            let is_dragging_over = drag_source.read().is_some_and(|src| src != item_index);
                            let wrapper_style = if is_line_break {
                                "width: 100%; flex-basis: 100%; cursor: grab;".to_string()
                            } else if is_dragging_over {
                                "cursor: grab; border-radius: 10px; outline: 2px dashed #aaa; outline-offset: 2px;".to_string()
                            } else {
                                "cursor: grab;".to_string()
                            };
                            rsx! {
                                div {
                                    key: "{item_index}",
                                    style: "{wrapper_style}",
                                    draggable: "true",
                                    ondragstart: move |_| {
                                        *drag_source.write() = Some(item_index);
                                    },
                                    ondragover: move |e: Event<DragData>| {
                                        e.prevent_default();
                                    },
                                    ondrop: move |e: Event<DragData>| {
                                        e.prevent_default();
                                        let src_opt = *drag_source.read();
                                        if let Some(src) = src_opt {
                                            if src != item_index {
                                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                                    let moved = part.items.remove(src);
                                                    let dst = if src < item_index { item_index - 1 } else { item_index };
                                                    part.items.insert(dst, moved);
                                                }
                                            }
                                        }
                                        *drag_source.write() = None;
                                    },
                                    ondragend: move |_| {
                                        *drag_source.write() = None;
                                    },
                                    match item {
                                        Some(PartItem::Chord(_)) => rsx! {
                                            ChordEditor {
                                                song,
                                                part_index,
                                                item_index,
                                                notation,
                                                capo,
                                            }
                                        },
                                        Some(PartItem::LineBreak) => rsx! {
                                            div {
                                                style: "width: 100%; display: flex; align-items: center; gap: 6px; flex-basis: 100%;",
                                                div { style: "flex: 1; height: 1px; background: #e0dbd0;" }
                                                span {
                                                    style: "font-size: 10px; color: #bbb; white-space: nowrap;",
                                                    "↵"
                                                }
                                                div { style: "flex: 1; height: 1px; background: #e0dbd0;" }
                                                button {
                                                    style: "background: none; border: none; font-size: 11px; color: #ccc; cursor: pointer; padding: 0 2px; font-family: inherit;",
                                                    onclick: move |e: Event<MouseData>| {
                                                        e.stop_propagation();
                                                        if let Some(part) = song.write().parts.get_mut(part_index) {
                                                            if item_index < part.items.len() {
                                                                part.items.remove(item_index);
                                                            }
                                                        }
                                                    },
                                                    "\u{2715}"
                                                }
                                            }
                                        },
                                        Some(PartItem::Repeat { times }) => rsx! {
                                            RepeatEditor {
                                                song,
                                                part_index,
                                                item_index,
                                                times,
                                            }
                                        },
                                        Some(PartItem::VoltaBracketStart { label }) => rsx! {
                                            VoltaEditor {
                                                song,
                                                part_index,
                                                item_index,
                                                label,
                                                is_start: true,
                                            }
                                        },
                                        Some(PartItem::RepeatStart) => rsx! {
                                            div {
                                                style: "
                                                    background: #eef2fa;
                                                    border: 2px solid #3a5a8a;
                                                    border-radius: 10px;
                                                    padding: 10px 14px;
                                                    min-width: 54px;
                                                    text-align: center;
                                                    position: relative;
                                                ",
                                                span {
                                                    style: "font-size: 20px; font-weight: 700; color: #3a5a8a;",
                                                    "||"
                                                }
                                                button {
                                                    style: "position: absolute; top: 2px; right: 4px; background: none; border: none; cursor: pointer; font-size: 10px; color: #888;",
                                                    onclick: move |_| {
                                                        if let Some(part) = song.write().parts.get_mut(part_index) {
                                                            if item_index < part.items.len() {
                                                                part.items.remove(item_index);
                                                            }
                                                        }
                                                    },
                                                    "\u{2715}"
                                                }
                                            }
                                        },
                                        Some(PartItem::VoltaBracketEnd) => rsx! {
                                            VoltaEditor {
                                                song,
                                                part_index,
                                                item_index,
                                                label: String::new(),
                                                is_start: false,
                                            }
                                        },
                                        Some(PartItem::Text { content, color }) => rsx! {
                                            TextAnnotationEditor {
                                                song,
                                                part_index,
                                                item_index,
                                                content,
                                                color,
                                            }
                                        },
                                        None => rsx! { span {} },
                                    }
                                }
                            }
                        }
                    }

                    // ── Add item buttons ──────────────────────────────────
                    div {
                        style: "display: flex; flex-direction: column; gap: 5px; align-self: center;",

                        button {
                            style: "
                                display: flex; align-items: center; justify-content: center;
                                width: 44px; height: 44px;
                                background: #eff6ff; border: 2px dashed #93c5fd;
                                border-radius: 10px; font-size: 22px; color: #bbb;
                                cursor: pointer; padding: 0; font-family: inherit;
                            ",
                            title: "Add chord",
                            onclick: move |_| {
                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                    part.items.push(PartItem::Chord(Chord::new("C", ChordQuality::Major)));
                                }
                            },
                            "+"
                        }

                        div {
                            style: "display: flex; gap: 4px;",

                            button {
                                style: "
                                    padding: 3px 7px; font-size: 11px; font-weight: 700;
                                    background: #eff6ff; border: 1.5px solid #bfdbfe;
                                    border-radius: 7px; color: #888; cursor: pointer; font-family: inherit;
                                ",
                                title: "Insert line break",
                                onclick: move |_| {
                                    if let Some(part) = song.write().parts.get_mut(part_index) {
                                        part.items.push(PartItem::LineBreak);
                                    }
                                },
                                "↵"
                            }

                            button {
                                style: "
                                    padding: 3px 7px; font-size: 11px; font-weight: 700;
                                    background: #eff6ff; border: 1.5px solid #bfdbfe;
                                    border-radius: 7px; color: #888; cursor: pointer; font-family: inherit;
                                ",
                                title: "Insert repeat sign",
                                onclick: move |_| {
                                    if let Some(part) = song.write().parts.get_mut(part_index) {
                                        part.items.push(PartItem::Repeat { times: 2 });
                                    }
                                },
                                "‖:"
                            }

                            button {
                                style: "
                                    padding: 3px 7px; font-size: 11px; font-weight: 700;
                                    background: #eef2fa; border: 1.5px solid #3a5a8a;
                                    border-radius: 7px; color: #3a5a8a; cursor: pointer; font-family: inherit;
                                ",
                                title: "Insert repeat start (||)",
                                onclick: move |_| {
                                    if let Some(part) = song.write().parts.get_mut(part_index) {
                                        part.items.push(PartItem::RepeatStart);
                                    }
                                },
                                "||"
                            }

                            button {
                                style: "
                                    padding: 3px 7px; font-size: 11px; font-weight: 700;
                                    background: #eff6ff; border: 1.5px solid #bfdbfe;
                                    border-radius: 7px; color: #888; cursor: pointer; font-family: inherit;
                                ",
                                title: "Insert volta bracket (e.g. 1st ending)",
                                onclick: move |_| {
                                    if let Some(part) = song.write().parts.get_mut(part_index) {
                                        part.items.push(PartItem::VoltaBracketStart { label: "1.".to_string() });
                                    }
                                },
                                "[1."
                            }

                            button {
                                style: "
                                    padding: 3px 7px; font-size: 11px; font-weight: 700;
                                    background: #eff6ff; border: 1.5px solid #bfdbfe;
                                    border-radius: 7px; color: #888; cursor: pointer; font-family: inherit;
                                ",
                                title: "Close volta bracket",
                                onclick: move |_| {
                                    if let Some(part) = song.write().parts.get_mut(part_index) {
                                        part.items.push(PartItem::VoltaBracketEnd);
                                    }
                                },
                                "]"
                            }

                            button {
                                style: "
                                    padding: 3px 7px; font-size: 11px; font-weight: 700;
                                    background: #fffde7; border: 1.5px solid #c8a800;
                                    border-radius: 7px; color: #7a6000; cursor: pointer; font-family: inherit;
                                ",
                                title: "Insert text comment",
                                onclick: move |_| {
                                    if let Some(part) = song.write().parts.get_mut(part_index) {
                                        part.items.push(PartItem::Text {
                                            content: String::new(),
                                            color: "#888888".to_string(),
                                        });
                                    }
                                },
                                "✎"
                            }
                        }
                    }
                }
            }

            // ── Per-part text / lyrics editor ─────────────────────────────
            PartTextEditor { song, part_index }
        }
    }
}

// ── Per-part text / lyrics editor ─────────────────────────────────────────────

#[component]
fn PartTextEditor(song: Signal<Song>, part_index: usize) -> Element {
    let part_text = song
        .read()
        .parts
        .get(part_index)
        .and_then(|p| p.part_text.clone());
    let has_text = part_text.is_some();
    let content = part_text
        .as_ref()
        .map(|t| t.content.clone())
        .unwrap_or_default();
    let color = part_text
        .as_ref()
        .map(|t| t.color.clone())
        .unwrap_or_else(|| "#555555".to_string());
    let size = part_text.as_ref().map(|t| t.size).unwrap_or(10);
    let show_chords = part_text.as_ref().map(|t| t.show_chords).unwrap_or(false);
    let chords_btn_border = if show_chords { "#7a9060" } else { "#bfdbfe" };
    let chords_btn_bg = if show_chords { "#e8f0e0" } else { "#eff6ff" };
    let chords_btn_fg = if show_chords { "#4a6040" } else { "#aaa" };
    let chords_btn_label = if show_chords {
        "\u{1F3B8} Chords: on"
    } else {
        "\u{1F3B8} Chords: off"
    };

    rsx! {
        div {
            style: "margin-top: 10px; border-top: 1px dashed #e8e4d8; padding-top: 10px;",

            if !has_text {
                // Show a small "Add lyrics/text" button
                button {
                    style: "
                        padding: 4px 12px; font-size: 11px; font-weight: 700;
                        background: transparent; border: 1.5px dashed #b0c0a0;
                        border-radius: 7px; color: #7a9060; cursor: pointer; font-family: inherit;
                    ",
                    title: "Add vocals / lyrics text to this part",
                    onclick: move |_| {
                        if let Some(part) = song.write().parts.get_mut(part_index) {
                            part.part_text = Some(PartTextSettings::new());
                        }
                    },
                    "🎤  Add vocals / text"
                }
            } else {
                div {
                    style: "display: flex; flex-direction: column; gap: 6px;",

                    // Header row: label + colour + size + remove button
                    div {
                        style: "display: flex; align-items: center; gap: 8px; flex-wrap: wrap;",
                        span {
                            style: "font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 1.5px; color: #7a9060;",
                            "🎤 Vocals / text"
                        }
                        // Colour picker
                        input {
                            r#type: "color",
                            value: "{color}",
                            style: "width: 24px; height: 24px; border: none; background: none; cursor: pointer; padding: 0;",
                            title: "Text colour",
                            oninput: move |e: Event<FormData>| {
                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                    if let Some(t) = part.part_text.as_mut() {
                                        t.color = e.value();
                                    }
                                }
                            }
                        }
                        // Size stepper
                        span { style: "font-size: 10px; color: #aaa;", "Size:" }
                        button {
                            style: "width: 20px; height: 20px; border-radius: 4px; border: 1px solid #bfdbfe; background: #eff6ff; font-size: 12px; font-weight: 700; cursor: pointer; font-family: inherit; display:flex; align-items:center; justify-content:center; color:#0a0f1e;",
                            onclick: move |_| {
                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                    if let Some(t) = part.part_text.as_mut() {
                                        if t.size > 6 { t.size -= 1; }
                                    }
                                }
                            },
                            "−"
                        }
                        span { style: "font-size: 11px; font-weight: 700; color: #0a0f1e; min-width: 20px; text-align: center;", "{size}" }
                        button {
                            style: "width: 20px; height: 20px; border-radius: 4px; border: 1px solid #bfdbfe; background: #eff6ff; font-size: 12px; font-weight: 700; cursor: pointer; font-family: inherit; display:flex; align-items:center; justify-content:center; color:#0a0f1e;",
                            onclick: move |_| {
                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                    if let Some(t) = part.part_text.as_mut() {
                                        if t.size < 36 { t.size += 1; }
                                    }
                                }
                            },
                            "+"
                        }
                        // Show-chords toggle
                        button {
                            style: "padding: 2px 8px; font-size: 10px; font-weight: 700; border-radius: 6px; cursor: pointer; font-family: inherit; border: 1.5px solid {chords_btn_border}; background: {chords_btn_bg}; color: {chords_btn_fg};",
                            title: "Show chords alongside lyrics",
                            onclick: move |_| {
                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                    if let Some(t) = part.part_text.as_mut() {
                                        t.show_chords = !t.show_chords;
                                    }
                                }
                            },
                            "{chords_btn_label}"
                        }
                        // Remove button
                        button {
                            style: "background: none; border: none; font-size: 11px; color: #ccc; cursor: pointer; padding: 0 2px; font-family: inherit; margin-left: auto;",
                            title: "Remove vocals text",
                            onclick: move |_| {
                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                    part.part_text = None;
                                }
                            },
                            "\u{2715}"
                        }
                    }

                    // Textarea
                    textarea {
                        style: "
                            width: 100%; box-sizing: border-box;
                            min-height: 64px; padding: 8px 10px;
                            border: 1px solid #d8e8c8; border-radius: 8px;
                            background: #f8fbf5; outline: none;
                            font-size: {size}px; color: {color};
                            font-family: inherit; line-height: 1.55; resize: vertical;
                        ",
                        value: "{content}",
                        placeholder: "Enter lyrics or notes…",
                        oninput: move |e: Event<FormData>| {
                            if let Some(part) = song.write().parts.get_mut(part_index) {
                                if let Some(t) = part.part_text.as_mut() {
                                    t.content = e.value();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Repeat editor ─────────────────────────────────────────────────────────────

#[component]
fn RepeatEditor(song: Signal<Song>, part_index: usize, item_index: usize, times: u8) -> Element {
    rsx! {
        div {
            style: "
                display: flex; flex-direction: column; align-items: center; gap: 6px;
                padding: 10px 14px 8px;
                background: #eff6ff; border: 2px solid #bfdbfe; border-radius: 12px;
                min-width: 68px; position: relative;
            ",
            button {
                style: "position: absolute; top: 6px; right: 8px; background: none; border: none;
                    font-size: 12px; color: #c0bab0; cursor: pointer; padding: 0; font-family: inherit;",
                onclick: move |e: Event<MouseData>| {
                    e.stop_propagation();
                    if let Some(part) = song.write().parts.get_mut(part_index) {
                        if item_index < part.items.len() { part.items.remove(item_index); }
                    }
                },
                "\u{2715}"
            }
            span {
                style: "font-size: 24px; font-weight: 800; color: #0a0f1e; line-height: 1;",
                "‖:"
            }
            div {
                style: "display: flex; align-items: center; gap: 4px;",
                span { style: "font-size: 11px; color: #888; font-weight: 700;", "×" }
                input {
                    style: "width: 36px; text-align: center; font-size: 12px; font-weight: 700;
                        border: 1px solid #bfdbfe; border-radius: 5px; background: #fff;
                        outline: none; padding: 3px; font-family: inherit; color: #0a0f1e;",
                    r#type: "number",
                    min: "0",
                    max: "99",
                    value: "{times}",
                    oninput: move |e: Event<FormData>| {
                        let t = e.value().parse::<u8>().unwrap_or(0);
                        if let Some(part) = song.write().parts.get_mut(part_index) {
                            if let Some(PartItem::Repeat { times }) = part.items.get_mut(item_index) {
                                *times = t;
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Text annotation editor ─────────────────────────────────────────────────────

#[component]
fn TextAnnotationEditor(
    song: Signal<Song>,
    part_index: usize,
    item_index: usize,
    content: String,
    color: String,
) -> Element {
    rsx! {
        div {
            style: "
                width: 100%; flex-basis: 100%;
                display: flex; align-items: center; gap: 8px;
                padding: 8px 10px;
                background: #fffde7; border: 1.5px solid #c8a800;
                border-radius: 10px; position: relative;
            ",
            // Colour picker
            input {
                r#type: "color",
                value: "{color}",
                style: "width: 26px; height: 26px; border: none; background: none; cursor: pointer; padding: 0; flex-shrink: 0;",
                title: "Choose text colour",
                oninput: move |e: Event<FormData>| {
                    if let Some(part) = song.write().parts.get_mut(part_index) {
                        if let Some(PartItem::Text { color, .. }) = part.items.get_mut(item_index) {
                            *color = e.value();
                        }
                    }
                }
            }
            // Pencil icon
            span { style: "font-size: 13px; color: #c8a800; flex-shrink: 0;", "✎" }
            // Text input
            input {
                style: "
                    flex: 1; border: none; border-bottom: 1px dashed #c8a800;
                    background: transparent; outline: none; font-size: 13px;
                    font-style: italic; color: {color}; font-family: inherit; padding: 2px 0;
                ",
                value: "{content}",
                placeholder: "Add a comment…",
                oninput: move |e: Event<FormData>| {
                    if let Some(part) = song.write().parts.get_mut(part_index) {
                        if let Some(PartItem::Text { content, .. }) = part.items.get_mut(item_index) {
                            *content = e.value();
                        }
                    }
                }
            }
            // Delete button
            button {
                style: "background: none; border: none; font-size: 12px; color: #c0bab0; cursor: pointer; padding: 0 2px; font-family: inherit; flex-shrink: 0;",
                onclick: move |e: Event<MouseData>| {
                    e.stop_propagation();
                    if let Some(part) = song.write().parts.get_mut(part_index) {
                        if item_index < part.items.len() {
                            part.items.remove(item_index);
                        }
                    }
                },
                "\u{2715}"
            }
        }
    }
}

// ── Volta bracket editor ───────────────────────────────────────────────────────

#[component]
fn VoltaEditor(
    song: Signal<Song>,
    part_index: usize,
    item_index: usize,
    label: String,
    is_start: bool,
) -> Element {
    rsx! {
        div {
            style: "
                display: flex; flex-direction: column; align-items: center; gap: 6px;
                padding: 10px 14px 8px;
                background: #f0edf5; border: 2px solid #c5bad9; border-radius: 12px;
                min-width: 60px; position: relative;
            ",
            button {
                style: "position: absolute; top: 6px; right: 8px; background: none; border: none;
                    font-size: 12px; color: #c0bab0; cursor: pointer; padding: 0; font-family: inherit;",
                onclick: move |e: Event<MouseData>| {
                    e.stop_propagation();
                    if let Some(part) = song.write().parts.get_mut(part_index) {
                        if item_index < part.items.len() { part.items.remove(item_index); }
                    }
                },
                "\u{2715}"
            }
            if is_start {
                span { style: "font-size: 13px; color: #6a4a9a; font-weight: 800; line-height: 1;", "[" }
                input {
                    style: "width: 38px; text-align: center; font-size: 12px; font-weight: 700;
                        border: 1px solid #c5bad9; border-radius: 5px; background: #fff;
                        outline: none; padding: 3px; font-family: inherit; color: #0a0f1e;",
                    value: "{label}",
                    placeholder: "1.",
                    oninput: move |e: Event<FormData>| {
                        if let Some(part) = song.write().parts.get_mut(part_index) {
                            if let Some(PartItem::VoltaBracketStart { label }) = part.items.get_mut(item_index) {
                                *label = e.value();
                            }
                        }
                    }
                }
            } else {
                span { style: "font-size: 20px; color: #6a4a9a; font-weight: 800; line-height: 1;", "]" }
                span { style: "font-size: 10px; color: #aaa;", "end" }
            }
        }
    }
}

// ── Library page ─────────────────────────────────────────────────────────────

#[component]
fn LibraryPage() -> Element {
    let db: Signal<Option<Db>> = use_context();
    let mut current_user: Signal<Option<User>> = use_context();
    let nav = use_navigator();

    let mut rows: Signal<Vec<SongRow>> = use_signal(Vec::new);
    let mut status: Signal<String> = use_signal(String::new);

    use_effect(move || {
        let user_id = current_user.read().as_ref().map(|u| u.id).unwrap_or(0);
        if let Some(db_ref) = db.read().as_ref() {
            match db_ref.list_songs(user_id) {
                Ok(list) => *rows.write() = list,
                Err(e) => *status.write() = format!("Load error: {e}"),
            }
        }
    });

    rsx! {
        div {
            style: "display: flex; align-items: flex-start; justify-content: center; padding: 48px 20px;",

            div {
                style: "
                    background: #ffffff;
                    border-radius: 14px;
                    padding: 28px 28px 32px;
                    box-shadow: 0 4px 32px rgba(0,0,0,0.10);
                    width: 440px;
                    min-width: 320px;
                ",

                // ── App header ───────────────────────────────────────────
                div {
                    style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 28px;",
                    h2 {
                        style: "margin: 0; font-size: 22px; font-weight: 800; color: #0a0f1e; letter-spacing: -0.3px;",
                        "🎵  SheetWave"
                    }
                    div {
                        style: "display: flex; align-items: center; gap: 10px;",
                        if let Some(user) = current_user.read().as_ref() {
                            span {
                                style: "font-size: 12px; color: #888; font-weight: 600;",
                                "👤  {user.username}"
                            }
                        }
                        button {
                            style: "
                                padding: 4px 12px;
                                background: transparent;
                                border: 1px solid #ccc;
                                border-radius: 8px;
                                font-size: 11px;
                                font-weight: 700;
                                cursor: pointer;
                                font-family: inherit;
                                color: #888;
                            ",
                            onclick: move |_| *current_user.write() = None,
                            "Log out"
                        }
                    }
                }

                // ── "My Songs" title + New Song button ───────────────────
                div {
                    style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px;",
                    h3 {
                        style: "margin: 0; font-size: 16px; font-weight: 800; color: #0a0f1e; letter-spacing: 0.3px;",
                        "📚  My Songs"
                    }
                    button {
                        style: "
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            width: 36px;
                            height: 36px;
                            background: #2563eb;
                            color: #ffffff;
                            border: none;
                            border-radius: 50%;
                            font-size: 24px;
                            font-weight: 300;
                            cursor: pointer;
                            font-family: inherit;
                            line-height: 1;
                            flex-shrink: 0;
                        ",
                        title: "New Song",
                        onclick: move |_| { nav.push(Route::NewSongPage {}); },
                        "+"
                    }
                }

                // ── Status message ────────────────────────────────────────
                if !status.read().is_empty() {
                    p { style: "color: #c0392b; font-size: 12px; margin: 0 0 8px;", "{status}" }
                }

                // ── Empty state ───────────────────────────────────────────
                if rows.read().is_empty() {
                    div {
                        style: "text-align: center; padding: 40px 0; color: #aaa;",
                        div { style: "font-size: 48px; margin-bottom: 12px;", "🎶" }
                        p { style: "font-size: 14px; margin: 0; font-weight: 600;", "No songs yet." }
                        p {
                            style: "font-size: 13px; margin: 6px 0 0; color: #bbb;",
                            "Tap + to create your first song."
                        }
                    }
                }

                // ── Song rows ─────────────────────────────────────────────
                for row in rows.read().iter().cloned() {
                    {
                        let row_id = row.id;
                        rsx! {
                            div {
                                key: "{row_id}",
                                style: "
                                    display: flex;
                                    align-items: center;
                                    gap: 8px;
                                    background: #ffffff;
                                    border: 1.5px solid #dbeafe;
                                    border-radius: 12px;
                                    padding: 12px 14px;
                                    margin-bottom: 8px;
                                    cursor: pointer;
                                    box-shadow: 0 1px 6px rgba(37,99,235,0.06);
                                ",
                                onclick: move |_| { nav.push(Route::SongPage { id: row_id }); },

                                div {
                                    style: "flex: 1; overflow: hidden;",
                                    div {
                                        style: "font-size: 14px; font-weight: 700; color: #0a0f1e; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
                                        "{row.name}"
                                    }
                                    div {
                                        style: "font-size: 12px; color: #6b7280; margin-top: 2px;",
                                        "{row.artist}"
                                    }
                                    // Instruments + username chips
                                    div {
                                        style: "display: flex; flex-wrap: wrap; align-items: center; gap: 5px; margin-top: 7px;",
                                        for inst in row.instruments.iter().cloned() {
                                            span {
                                                key: "{inst.label()}",
                                                style: "display: inline-flex; align-items: center; gap: 3px; font-size: 11px; font-weight: 600; background: #eff6ff; border: 1px solid #bfdbfe; border-radius: 6px; padding: 2px 7px; color: #1d4ed8;",
                                                img { src: inst_icon(inst).to_string(), style: "width: 16px; height: 16px; object-fit: contain;", alt: "{inst.label()}" }
                                                "{inst.label()}"
                                            }
                                        }
                                        if !row.username.is_empty() {
                                            span {
                                                style: "display: inline-flex; align-items: center; gap: 3px; font-size: 11px; font-weight: 600; background: #eff6ff; border: 1px solid #bfdbfe; border-radius: 6px; padding: 2px 7px; color: #1d4ed8;",
                                                "👤  {row.username}"
                                            }
                                        }
                                    }
                                }

                                button {
                                    style: "
                                        background: none;
                                        border: none;
                                        cursor: pointer;
                                        font-size: 14px;
                                        padding: 4px 6px;
                                        color: #c0392b;
                                        border-radius: 4px;
                                        flex-shrink: 0;
                                    ",
                                    title: "Delete",
                                    onclick: move |e| {
                                        e.stop_propagation();
                                        let uid = current_user.read().as_ref().map(|u| u.id).unwrap_or(0);
                                        if let Some(db_ref) = db.read().as_ref() {
                                            let _ = db_ref.delete_song(row_id);
                                            match db_ref.list_songs(uid) {
                                                Ok(list) => *rows.write() = list,
                                                Err(err) => *status.write() = format!("Error: {err}"),
                                            }
                                        }
                                    },
                                    "✕"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Song detail page ──────────────────────────────────────────────────────────

#[component]
fn SongPage(id: i64) -> Element {
    let db: Signal<Option<Db>> = use_context();
    let current_user: Signal<Option<User>> = use_context();

    let song: Signal<Song> = use_signal(move || {
        db.read()
            .as_ref()
            .and_then(|d| d.load_song(id).ok())
            .unwrap_or_else(example_song)
    });

    rsx! {
        div {
            style: "display: flex; align-items: flex-start; justify-content: center; padding: 48px 20px;",
            SongView { song, db, current_user, song_id: Some(id) }
        }
    }
}

// ── New song page ─────────────────────────────────────────────────────────────

#[component]
fn NewSongPage() -> Element {
    let db: Signal<Option<Db>> = use_context();
    let current_user: Signal<Option<User>> = use_context();

    let song: Signal<Song> = use_signal(blank_song);

    rsx! {
        div {
            style: "display: flex; align-items: flex-start; justify-content: center; padding: 48px 20px;",
            SongView { song, db, current_user, song_id: None }
        }
    }
}

// ── Instrument sheet page ───────────────────────────────────────────────────────────────

#[component]
fn InstrumentSheetPage(id: i64, instrument: String) -> Element {
    let db: Signal<Option<Db>> = use_context();
    let mut current_user: Signal<Option<User>> = use_context();
    let nav = use_navigator();

    let song: Signal<Song> = use_signal(move || {
        db.read()
            .as_ref()
            .and_then(|d| d.load_song(id).ok())
            .unwrap_or_else(example_song)
    });

    let inst = Instrument::from_label(&instrument);
    let accent = inst.map(|i| i.accent_color()).unwrap_or("#2563eb");
    let inst_icon_asset = inst.map(inst_icon).unwrap_or(ICON_BASE);
    let inst_label = inst
        .map(|i| i.label())
        .unwrap_or_else(|| instrument.as_str());

    let mut capo = use_signal(|| 0_i8);
    let notation: Signal<Notation> = use_signal(|| Notation::English);

    rsx! {
        div {
            style: "display: flex; align-items: flex-start; justify-content: center; padding: 48px 20px;",

            div {
                style: "
                    background: #ffffff;
                    border-radius: 14px;
                    padding: 48px 52px;
                    box-shadow: 0 4px 32px rgba(0,0,0,0.10);
                    max-width: 720px;
                    width: 100%;
                ",

                // ── Page header ───────────────────────────────────────────────────────────────
                div {
                    style: "border-bottom: 2px solid #dbeafe; padding-bottom: 28px; margin-bottom: 36px;",

                    // Top nav: back + user + logout
                    div {
                        style: "display: flex; justify-content: space-between; align-items: center; gap: 10px; margin-bottom: 20px;",
                        button {
                            style: "padding: 4px 12px; background: transparent; border: 1px solid #ccc; border-radius: 8px; font-size: 11px; font-weight: 700; cursor: pointer; font-family: inherit; color: #888;",
                            onclick: move |_| { nav.push(Route::SongPage { id }); },
                            "←  Full Sheet"
                        }
                        div {
                            style: "display: flex; align-items: center; gap: 10px;",
                            if let Some(user) = current_user.read().as_ref() {
                                span {
                                    style: "font-size: 12px; color: #888; font-weight: 600;",
                                    "👤  {user.username}"
                                }
                            }
                            button {
                                style: "padding: 4px 12px; background: transparent; border: 1px solid #ccc; border-radius: 8px; font-size: 11px; font-weight: 700; cursor: pointer; font-family: inherit; color: #888;",
                                onclick: move |_| *current_user.write() = None,
                                "Log out"
                            }
                        }
                    }

                    // Instrument badge
                    div {
                        style: "display: inline-flex; align-items: center; gap: 10px; background: {accent}; color: #fff; border-radius: 12px; padding: 10px 20px; margin-bottom: 22px;",
                        img { src: inst_icon_asset.to_string(), style: "width: 44px; height: 44px; object-fit: contain;", alt: "{inst_label}" }
                        span { style: "font-size: 16px; font-weight: 800; letter-spacing: 0.5px;", "{inst_label}" }
                    }

                    // Song title + artist
                    h1 {
                        style: "margin: 0 0 6px; font-size: 38px; font-weight: 800; color: #0a0f1e; letter-spacing: -0.5px;",
                        "{song.read().name}"
                    }
                    p {
                        style: "margin: 0 0 14px; font-size: 17px; color: #666; font-style: italic;",
                        "{song.read().artist}"
                    }

                    // Key pill
                    span {
                        style: "display: inline-block; background: {accent}; color: #fff; border-radius: 20px; padding: 5px 16px; font-size: 12px; font-weight: 700; letter-spacing: 1px; text-transform: uppercase;",
                        "Key: {song.read().key}"
                    }

                    // Capo / Transpose control
                    div {
                        style: "margin-top: 14px; display: flex; align-items: center; gap: 10px;",
                        span {
                            style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;",
                            if inst == Some(Instrument::Piano) { "Transpose:" } else { "Capo:" }
                        }
                        button {
                            style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                            onclick: move |_| { if capo() > if inst == Some(Instrument::Piano) { -12 } else { 0 } { *capo.write() -= 1; } },
                            "−"
                        }
                        span {
                            style: "min-width: 52px; text-align: center; font-size: 13px; font-weight: 800; color: #0a0f1e;",
                            if capo() == 0 { "Off" } else if capo() > 0 { "+{capo()}" } else { "{capo()}" }
                        }
                        button {
                            style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #bfdbfe; background: #eff6ff; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #0a0f1e;",
                            onclick: move |_| { if capo() < 12 { *capo.write() += 1; } },
                            "+"
                        }
                        if capo() != 0 {
                            span {
                                style: "font-size: 11px; color: #888; font-style: italic;",
                                if inst == Some(Instrument::Piano) {
                                    "→ sounds in {song.read().apply_capo(-capo()).key}"
                                } else {
                                    "→ play in {song.read().apply_capo(capo()).key}"
                                }
                            }
                        }
                    }
                }

                // ── Chord / Riff parts (read-only) ──────────────────────────────────────────
                for part_index in 0..song.read().parts.len() {{
                    let part_name = song
                        .read()
                        .parts
                        .get(part_index)
                        .map(|p| p.name.clone())
                        .unwrap_or_default();
                    let is_riff = song
                        .read()
                        .parts
                        .get(part_index)
                        .map(|p| p.kind == PartKind::Riff || p.kind == PartKind::BassRiff || p.kind == PartKind::DrumBeat || p.kind == PartKind::PianoRiff)
                        .unwrap_or(false);
                    let item_count = song
                        .read()
                        .parts
                        .get(part_index)
                        .map(|p| p.items.len())
                        .unwrap_or(0);
                    let tab_text = song
                        .read()
                        .parts
                        .get(part_index)
                        .map(|p| p.tab_as_ascii())
                        .unwrap_or_default();
                    let part_border = if is_riff { "#b5d6b5" } else { "#dbeafe" };
                    let part_bg = if is_riff { "#f6fbf6" } else { "transparent" };
                    let label_color = if is_riff { "#5c7a5c" } else { "#aaa" };
                    rsx! {
                        div {
                            key: "{part_index}",
                            style: "margin-bottom: 32px; border: 1px solid {part_border}; border-radius: 12px; padding: 18px 20px 16px; background: {part_bg};",
                            p {
                                style: "margin: 0 0 14px; font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 3px; color: {label_color};",
                                "{part_name}"
                            }
                            if is_riff {
                                pre {
                                    style: "
                                        font-family: Courier, monospace;
                                        font-size: 13px;
                                        line-height: 1.7;
                                        color: #0a0f1e;
                                        background: #fff;
                                        border: 1.5px solid #b5d6b5;
                                        border-radius: 8px;
                                        padding: 12px 14px;
                                        margin: 0;
                                        overflow-x: auto;
                                        white-space: pre;
                                    ",
                                    "{tab_text}"
                                }
                            } else {
                                div {
                                    style: "display: flex; flex-wrap: wrap; gap: 10px;",
                                    for item_index in 0..item_count {{
                                    let item = song
                                        .read()
                                        .parts
                                        .get(part_index)
                                        .and_then(|p| p.items.get(item_index))
                                        .cloned();
                                    match item {
                                        Some(PartItem::Chord(chord)) => {
                                            let capo_label = if capo() != 0 {
                                                let is_minor = song.read().key.to_lowercase().contains("minor");
                                                let shifted = if capo() > 0 {
                                                    song::shift_note(&chord.root, capo() as u8, is_minor)
                                                } else {
                                                    song::shift_note_up(&chord.root, (-capo()) as u8, is_minor)
                                                };
                                                let note = notation();
                                                format!("{}{}", apply_notation(&shifted, note), chord.quality.symbol())
                                            } else {
                                                let note = notation();
                                                chord.display_with_notation(note)
                                            };
                                            rsx! {
                                                div {
                                                    key: "{item_index}",
                                                    style: "
                                                        background: #eff6ff;
                                                        border: 2px solid {accent};
                                                        border-radius: 12px;
                                                        padding: 14px 18px;
                                                        min-width: 72px;
                                                        text-align: center;
                                                    ",
                                                    span {
                                                        style: "font-size: 36px; font-weight: 800; color: #0a0f1e; letter-spacing: -1px; line-height: 1; display: block;",
                                                        "{capo_label}"
                                                    }
                                                }
                                            }
                                        }
                                        Some(PartItem::LineBreak) => rsx! {
                                            div {
                                                key: "{item_index}",
                                                style: "flex-basis: 100%; height: 0;",
                                            }
                                        },
                                        Some(PartItem::Repeat { times }) => rsx! {
                                            div {
                                                key: "{item_index}",
                                                style: "
                                                    background: #eff6ff;
                                                    border: 2px solid {accent};
                                                    border-radius: 12px;
                                                    padding: 14px 18px;
                                                    min-width: 72px;
                                                    text-align: center;
                                                ",
                                                span {
                                                    style: "font-size: 28px; font-weight: 800; color: #0a0f1e; line-height: 1; display: block;",
                                                    "‖: ×{times}"
                                                }
                                            }
                                        },
                                        Some(PartItem::VoltaBracketStart { label }) => rsx! {
                                            div {
                                                key: "{item_index}",
                                                style: "
                                                    background: #f0ecf8;
                                                    border: 2px solid #9b8fc0;
                                                    border-radius: 12px;
                                                    padding: 14px 18px;
                                                    min-width: 72px;
                                                    text-align: center;
                                                ",
                                                span {
                                                    style: "font-size: 22px; font-weight: 700; color: #5c3d99; line-height: 1; display: block;",
                                                    "[{label}"
                                                }
                                            }
                                        },
                                        Some(PartItem::RepeatStart) => rsx! {
                                            div {
                                                key: "{item_index}",
                                                style: "
                                                    background: #eef2fa;
                                                    border: 2px solid #3a5a8a;
                                                    border-radius: 12px;
                                                    padding: 14px 18px;
                                                    min-width: 72px;
                                                    text-align: center;
                                                ",
                                                span {
                                                    style: "font-size: 22px; font-weight: 700; color: #3a5a8a; line-height: 1; display: block;",
                                                    "||"
                                                }
                                            }
                                        },
                                        Some(PartItem::VoltaBracketEnd) => rsx! {
                                            div {
                                                key: "{item_index}",
                                                style: "
                                                    background: #f0ecf8;
                                                    border: 2px solid #9b8fc0;
                                                    border-radius: 12px;
                                                    padding: 14px 18px;
                                                    min-width: 72px;
                                                    text-align: center;
                                                ",
                                                span {
                                                    style: "font-size: 22px; font-weight: 700; color: #5c3d99; line-height: 1; display: block;",
                                                    "]"
                                                }
                                            }
                                        },
                                        Some(PartItem::Text { content, color }) => rsx! {
                                            div {
                                                key: "{item_index}",
                                                style: "flex-basis: 100%; font-size: 13px; font-style: italic; color: {color}; line-height: 1.5; padding: 2px 0;",
                                                "{content}"
                                            }
                                        },
                                        None => rsx! { div { key: "{item_index}" } },
                                    }
                                }}
                            }  // end else div
                            }  // end else branch

                        // ── Part text / lyrics (read-only) ────────────────
                        {
                            let pt = song.read().parts.get(part_index).and_then(|p| p.part_text.clone());
                            if let Some(t) = pt {
                                rsx! {
                                    div {
                                        style: "margin-top: 14px; padding-top: 10px; border-top: 1px solid #dbeafe; white-space: pre-wrap; font-size: {t.size}px; color: {t.color}; line-height: 1.6; font-family: inherit;",
                                        "{t.content}"
                                    }
                                }
                            } else {
                                rsx! { span {} }
                            }
                        }
                        }
                    }
                }}

                // ── Vocals / notes (read-only) ─────────────────────────────────────────────
                if !song.read().vocals_notes.is_empty() {
                    div {
                        style: "border: 1.5px solid #dbeafe; border-radius: 12px; overflow: hidden;",
                        div {
                            style: "display: flex; align-items: center; gap: 8px; padding: 10px 16px; background: #eff6ff; border-bottom: 1.5px solid #dbeafe;",
                            span { style: "font-size: 18px; line-height: 1;", "🎤" }
                            span {
                                style: "font-size: 11px; font-weight: 700; color: #888; text-transform: uppercase; letter-spacing: 1.2px;",
                                "Vocals / Notes"
                            }
                        }
                        div {
                            style: "padding: 14px 16px; font-size: 14px; color: #444; line-height: 1.6; white-space: pre-wrap;",
                            "{song.read().vocals_notes}"
                        }
                    }
                }
            }
        }
    }
}

// ── Chord editor ─────────────────────────────────────────────────────────────

#[component]
fn ChordEditor(
    song: Signal<Song>,
    part_index: usize,
    item_index: usize,
    notation: Signal<Notation>,
    capo: Signal<i8>,
) -> Element {
    let chord = song
        .read()
        .parts
        .get(part_index)
        .and_then(|p| p.items.get(item_index))
        .and_then(|item| {
            if let PartItem::Chord(c) = item {
                Some(c.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| Chord::new("C", ChordQuality::Major));

    let display_label = if capo() != 0 {
        // Show the chord shape the player needs to play with the capo/transpose.
        let is_minor = song.read().key.to_lowercase().contains("minor");
        let shift_fn = |root: &str| -> String {
            if capo() > 0 {
                apply_notation(&song::shift_note(root, capo() as u8, is_minor), notation())
            } else {
                apply_notation(
                    &song::shift_note_up(root, (-capo()) as u8, is_minor),
                    notation(),
                )
            }
        };
        let shifted_root = shift_fn(&chord.root);
        let shifted_bass = chord
            .bass_note
            .as_deref()
            .map(|b| format!("/{}", shift_fn(b)));
        format!(
            "{}{}{}",
            shifted_root,
            chord.quality.symbol(),
            shifted_bass.unwrap_or_default()
        )
    } else {
        chord.display_with_notation(notation())
    };

    rsx! {
        div {
            style: "
                background: #eff6ff;
                border: 2px solid #bfdbfe;
                border-radius: 12px;
                padding: 18px 18px 12px;
                display: flex;
                flex-direction: column;
                align-items: center;
                gap: 10px;
                min-width: 90px;
                position: relative;
            ",

            // Remove button
            button {
                style: "
                    position: absolute;
                    top: 6px;
                    right: 8px;
                    background: none;
                    border: none;
                    font-size: 12px;
                    color: #c0bab0;
                    cursor: pointer;
                    padding: 0;
                    line-height: 1;
                    font-family: inherit;
                ",
                onclick: move |e: Event<MouseData>| {
                    e.stop_propagation();
                    if let Some(part) = song.write().parts.get_mut(part_index) {
                        if item_index < part.items.len() {
                            part.items.remove(item_index);
                        }
                    }
                },
                "\u{2715}"
            }

            // Large chord name
            span {
                style: "
                    font-size: 42px;
                    font-weight: 800;
                    color: #0a0f1e;
                    letter-spacing: -1px;
                    line-height: 1;
                ",
                "{display_label}"
            }

            // Edit controls
            div {
                style: "
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    gap: 5px;
                    border-top: 1px solid #bfdbfe;
                    padding-top: 8px;
                    width: 100%;
                ",

                // Root note input
                input {
                    style: "
                        width: 70px;
                        font-size: 13px;
                        font-weight: 600;
                        color: #0a0f1e;
                        text-align: center;
                        border: 1px solid #bfdbfe;
                        border-radius: 6px;
                        background: #fff;
                        outline: none;
                        padding: 4px 6px;
                        font-family: inherit;
                    ",
                    value: "{chord.root}",
                    placeholder: "Root (C, F#…)",
                    oninput: move |e: Event<FormData>| {
                        if let Some(part) = song.write().parts.get_mut(part_index) {
                            if let Some(PartItem::Chord(c)) = part.items.get_mut(item_index) {
                                c.root = e.value();
                            }
                        }
                    }
                }

                // Quality dropdown
                select {
                    style: "
                        font-size: 12px;
                        color: #555;
                        background: #fff;
                        border: 1px solid #bfdbfe;
                        border-radius: 6px;
                        outline: none;
                        cursor: pointer;
                        padding: 4px 6px;
                        width: 70px;
                        font-family: inherit;
                        text-align: center;
                    ",
                    onchange: move |e: Event<FormData>| {
                        if let Some(part) = song.write().parts.get_mut(part_index) {
                            if let Some(PartItem::Chord(c)) = part.items.get_mut(item_index) {
                                c.quality = ChordQuality::from_symbol(&e.value());
                            }
                        }
                    },
                    for q in ChordQuality::all() {
                        option {
                            key: "{q.label()}",
                            value: "{q.symbol()}",
                            selected: q == chord.quality,
                            "{q.label()}"
                        }
                    }
                }

                // Bass note input (slash chord, e.g. G/B)
                input {
                    style: "
                        width: 70px;
                        font-size: 13px;
                        font-weight: 600;
                        color: #0a0f1e;
                        text-align: center;
                        border: 1px solid #bfdbfe;
                        border-radius: 6px;
                        background: #fff;
                        outline: none;
                        padding: 4px 6px;
                        font-family: inherit;
                    ",
                    value: chord.bass_note.clone().unwrap_or_default(),
                    placeholder: "/ Bass",
                    oninput: move |e: Event<FormData>| {
                        if let Some(part) = song.write().parts.get_mut(part_index) {
                            if let Some(PartItem::Chord(c)) = part.items.get_mut(item_index) {
                                let v = e.value();
                                c.bass_note = if v.is_empty() { None } else { Some(v) };
                            }
                        }
                    }
                }
            }
        }
    }
}
