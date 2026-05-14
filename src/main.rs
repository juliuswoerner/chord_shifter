#![allow(non_snake_case)]

use dioxus::prelude::*;
use manganis::Asset;

use song::Instrument;

const ICON_BASE: Asset = manganis::asset!("/assets/icons/base.png");
const ICON_ELECTRIC: Asset = manganis::asset!("/assets/icons/electric.png");
const ICON_ACOUSTIC: Asset = manganis::asset!("/assets/icons/acoustic.png");
const ICON_BASS: Asset = manganis::asset!("/assets/icons/bass.png");
const ICON_PIANO: Asset = manganis::asset!("/assets/icons/piano.png");
const ICON_DRUMS: Asset = manganis::asset!("/assets/icons/drums.png");

fn inst_icon(inst: Instrument) -> Asset {
    match inst {
        Instrument::Guitar => ICON_ELECTRIC,
        Instrument::AcousticGuitar => ICON_ACOUSTIC,
        Instrument::Bass => ICON_BASS,
        Instrument::Piano => ICON_PIANO,
        Instrument::Drums => ICON_DRUMS,
    }
}

mod auth;
mod pdf;
mod song;

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
                Ok(song::Song {
                    name: s.name,
                    artist: s.artist,
                    key: s.key,
                    parts,
                    instruments,
                    vocals_notes: s.vocals_notes,
                    instrument_parts,
                    instrument_capos,
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
            return Err(format!("Username '{username}' is already taken"));
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

    fn has_users(&self) -> Result<bool, String> {
        Ok(!ls_read_users().is_empty())
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
    apply_notation, Chord, ChordQuality, Notation, PartItem, PartKind, Song, TabCell, TabCol,
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

// ── Root component ────────────────────────────────────────────────────────────

#[component]
fn App() -> Element {
    let db: Signal<Option<Db>> = use_signal(|| {
        Db::open("chord_shifter.db")
            .map_err(|e| eprintln!("DB open failed: {e}"))
            .ok()
    });
    let current_user: Signal<Option<User>> = use_signal(|| None);

    use_context_provider(|| db);
    use_context_provider(|| current_user);

    rsx! {
        div {
            style: "
                font-family: 'Helvetica Neue', Arial, sans-serif;
                min-height: 100vh;
                background: #f0ece2;
            ",

            if current_user.read().is_none() {
                div {
                    style: "display: flex; align-items: flex-start; justify-content: center; padding: 48px 20px;",
                    LoginScreen { db, current_user }
                }
            } else {
                Router::<Route> {}
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
    let mut part_name_size = use_signal(|| 9_u32);
    let mut chord_size = use_signal(|| 18_u32);
    let mut preview_open = use_signal(|| false);
    let mut preview_url: Signal<String> = use_signal(String::new);
    let mut notation: Signal<Notation> = use_signal(|| Notation::English);
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
    let guitar_capo = {
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
    let acoustic_capo = {
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
    let piano_capo = {
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
    let drums_capo = {
        let cap = *song.read().instrument_capos.get("Drums").unwrap_or(&0);
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
    });

    // Keep instrument_capos in song in sync with the capo signals so the
    // values are persisted even without an explicit "Save instrument" click.
    use_effect(move || {
        let caps = [
            ("Electric", guitar_capo()),
            ("Acoustic", acoustic_capo()),
            ("Bass", bass_capo()),
            ("Piano", piano_capo()),
            ("Drums", drums_capo()),
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
                    border-bottom: 2px solid #e8e4da;
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
                        color: #1a1a2e;
                        letter-spacing: -0.5px;
                        border: none;
                        border-bottom: 2px dashed #e0dbd0;
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
                        border-bottom: 1px dashed #e0dbd0;
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
                        style: "display: inline-flex; align-items: center; background: #1a1a2e; border-radius: 20px; overflow: hidden;",
                        span {
                            style: "color: #f0ece2; padding: 5px 6px 5px 14px; font-size: 12px; font-weight: 700; letter-spacing: 1.2px; text-transform: uppercase; white-space: nowrap;",
                            "Key:"
                        }
                        // − button
                        button {
                            style: "background: rgba(255,255,255,0.10); border: none; color: #f0ece2; font-size: 16px; font-weight: 700; padding: 0 8px; cursor: pointer; font-family: inherit; line-height: 1; height: 30px;",
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
                            style: "color: #f0ece2; padding: 5px 6px; font-size: 14px; font-weight: 800; letter-spacing: 0.5px; min-width: 28px; text-align: center;",
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
                                        style: "background: rgba(255,255,255,0.18); border: none; color: #f0ece2; font-size: 11px; font-weight: 700; padding: 0 7px; cursor: pointer; font-family: inherit; line-height: 1; height: 30px; white-space: nowrap;",
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
                            style: "background: rgba(255,255,255,0.10); border: none; color: #f0ece2; font-size: 16px; font-weight: 700; padding: 0 8px; cursor: pointer; font-family: inherit; line-height: 1; height: 30px;",
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
                            "padding: 5px 12px; border-radius: 16px 0 0 16px; border: none; font-size: 11px; font-weight: 700; font-family: inherit; cursor: pointer; background: #1a1a2e; color: #f0ece2;"
                        } else {
                            "padding: 5px 12px; border-radius: 16px 0 0 16px; border: 1.5px solid #d9d4c5; border-right: none; font-size: 11px; font-weight: 700; font-family: inherit; cursor: pointer; background: #f0ece2; color: #888;"
                        };
                        let min_style = if is_minor {
                            "padding: 5px 12px; border-radius: 0 16px 16px 0; border: none; font-size: 11px; font-weight: 700; font-family: inherit; cursor: pointer; background: #1a1a2e; color: #f0ece2;"
                        } else {
                            "padding: 5px 12px; border-radius: 0 16px 16px 0; border: 1.5px solid #d9d4c5; border-left: none; font-size: 11px; font-weight: 700; font-family: inherit; cursor: pointer; background: #f0ece2; color: #888;"
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
                                style: "font-size: 13px; font-weight: 700; color: #1a1a2e; background: #f0ece2; border: 1.5px solid #d9d4c5; border-radius: 10px; padding: 4px 10px; outline: none; cursor: pointer; font-family: inherit;",
                                title: "Change key without transposing chords",
                                onchange: move |e| {
                                    song.write().transpose_to(&e.value());
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
                        style: "font-size: 13px; font-weight: 700; color: #1a1a2e; background: #f0ece2; border: 1.5px solid #d9d4c5; border-radius: 10px; padding: 4px 10px; outline: none; cursor: pointer; font-family: inherit;",
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
                                "display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#1a1a2e;border:none;border-radius:10px;cursor:pointer;font-family:inherit;"
                            } else {
                                "display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#f0ece2;border:1.5px solid #d8d4ca;border-radius:10px;cursor:pointer;font-family:inherit;"
                            };
                            let lbl_s = if is_base {
                                "font-size:9px;font-weight:700;letter-spacing:0.8px;text-transform:uppercase;color:#f0ece2;"
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
                                    img { src: ICON_BASE.to_string(), style: "width: 28px; height: 28px; object-fit: contain;", alt: "Base" }
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
                                    format!("display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#f0ece2;border:2px solid {accent};border-radius:10px;cursor:pointer;font-family:inherit;")
                                } else {
                                    "display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#f0ece2;border:1.5px solid #d8d4ca;border-radius:10px;cursor:pointer;font-family:inherit;".to_string()
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
                                        img { src: inst_icon(inst).to_string(), style: "width: 28px; height: 28px; object-fit: contain;", alt: "{inst.label()}" }
                                        span { style: "{lbl_s}", "{inst.label()}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Parts editor (base or instrument) ─────────────────────────────────
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
                                    style: "padding: 2px 10px; background: transparent; color: #999; border: 1.5px dashed #c8c3b3; border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; font-family: inherit; white-space: nowrap;",
                                    title: "Insert part here",
                                    onclick: move |_| {
                                        let mut s = song.write();
                                        s.parts.insert(insert_index, crate::song::SongPart::new("New Part"));
                                    },
                                    "+ Part"
                                }
                                button {
                                    style: "padding: 2px 10px; background: transparent; color: #5c7a5c; border: 1.5px dashed #8fba8f; border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; font-family: inherit; white-space: nowrap;",
                                    title: "Insert tab here",
                                    onclick: move |_| {
                                        let mut s = song.write();
                                        s.parts.insert(insert_index, crate::song::SongPart::new_riff("Riff"));
                                    },
                                    "~ Tab"
                                }
                                div { style: "flex: 1; height: 1px; background: #ddd;" }
                            }
                        }
                    }
                    PartView { key: "{part_index}", song, part_index, notation, capo: use_signal(|| 0_u8) }
                }
                button {
                    style: "
                        margin-top: 8px;
                        margin-bottom: 12px;
                        padding: 10px 20px;
                        background: transparent;
                        color: #999;
                        border: 2px dashed #c8c3b3;
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
                    onclick: move |_| { song.write().parts.push(crate::song::SongPart::new_riff("Riff")); },
                    "+ Add Tab"
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
                        // Also overwrite saved overrides so the changes persist
                        // when the user saves an instrument sheet.
                        let mut s = song.write();
                        for label in ["Electric", "Acoustic", "Bass", "Piano", "Drums"] {
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
                        Instrument::Drums => (drums_song, drums_capo),
                    };
                    let accent = inst.accent_color();
                    let inst_label = inst.label();
                    rsx! {
                        // Info banner
                        div {
                            style: "margin-bottom: 18px; background: #fff8e1; border: 1px solid #ffe082; border-radius: 8px; padding: 10px 16px; font-size: 12px; color: #795548; display: flex; align-items: center; gap: 8px;",
                            img { src: inst_icon(inst).to_string(), style: "width: 22px; height: 22px; object-fit: contain;", alt: "{inst_label}" }
                            span { "✏️  " strong { "{inst_label}" } " sheet — edits apply to this instrument only" }
                        }
                        // Instrument capo control
                        div {
                            style: "margin-bottom: 20px; display: flex; align-items: center; gap: 10px;",
                            span {
                                style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;",
                                "Capo:"
                            }
                            button {
                                style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #d9d4c5; background: #f0ece2; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #1a1a2e;",
                                onclick: move |_| { if act_capo() > 0 { *act_capo.write() -= 1; } },
                                "−"
                            }
                            span {
                                style: "min-width: 52px; text-align: center; font-size: 13px; font-weight: 800; color: #1a1a2e;",
                                if act_capo() == 0 { "Off" } else { "{act_capo()}" }
                            }
                            button {
                                style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #d9d4c5; background: #f0ece2; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #1a1a2e;",
                                onclick: move |_| { if act_capo() < 12 { *act_capo.write() += 1; } },
                                "+"
                            }
                            if act_capo() > 0 {
                                span {
                                    style: "font-size: 11px; color: #888; font-style: italic;",
                                    "→ play in {act_song.read().apply_capo(act_capo()).key}"
                                }
                            }
                        }
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
                                            style: "padding: 2px 10px; background: transparent; color: #999; border: 1.5px dashed #c8c3b3; border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; font-family: inherit; white-space: nowrap;",
                                            title: "Insert part here",
                                            onclick: move |_| {
                                                let mut s = act_song.write();
                                                s.parts.insert(insert_index, crate::song::SongPart::new("New Part"));
                                            },
                                            "+ Part"
                                        }
                                        button {
                                            style: "padding: 2px 10px; background: transparent; color: #5c7a5c; border: 1.5px dashed #8fba8f; border-radius: 6px; font-size: 11px; font-weight: 600; cursor: pointer; font-family: inherit; white-space: nowrap;",
                                            title: "Insert tab here",
                                            onclick: move |_| {
                                                let mut s = act_song.write();
                                                s.parts.insert(insert_index, crate::song::SongPart::new_riff("Riff"));
                                            },
                                            "~ Tab"
                                        }
                                        div { style: "flex: 1; height: 1px; background: #ddd;" }
                                    }
                                }
                            }
                            PartView { key: "{part_index}", song: act_song, part_index, notation, capo: act_capo }
                        }
                        button {
                            style: "
                                margin-top: 8px;
                                margin-bottom: 16px;
                                padding: 10px 20px;
                                background: transparent;
                                color: #999;
                                border: 2px dashed #c8c3b3;
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

            // ── Vocals / notes ────────────────────────────────────────────────
            div {
                style: "
                    margin-bottom: 24px;
                    border: 1.5px solid #e8e4da;
                    border-radius: 12px;
                    overflow: hidden;
                ",
                div {
                    style: "
                        display: flex;
                        align-items: center;
                        gap: 8px;
                        padding: 10px 16px;
                        background: #f7f5f0;
                        border-bottom: 1.5px solid #e8e4da;
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
                            color: #1a1a2e;
                            text-align: center;
                            border: 1px solid #d0cbc0;
                            border-radius: 6px;
                            background: #fff;
                            outline: none;
                            padding: 4px 6px;
                            font-family: inherit;
                        ",
                        value: "{part_name_size}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                *part_name_size.write() = v.clamp(6, 24);
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
                            color: #1a1a2e;
                            text-align: center;
                            border: 1px solid #d0cbc0;
                            border-radius: 6px;
                            background: #fff;
                            outline: none;
                            padding: 4px 6px;
                            font-family: inherit;
                        ",
                        value: "{chord_size}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                *chord_size.write() = v.clamp(10, 36);
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
                    background: #f5f2ea;
                    color: #1a1a2e;
                    border: 2px solid #d9d4c5;
                    border-radius: 10px;
                    font-size: 15px;
                    font-weight: 700;
                    letter-spacing: 0.6px;
                    cursor: pointer;
                    font-family: inherit;
                ",
                onclick: move |_| {
                    let s    = song.read().clone();
                    let pns  = part_name_size() as f32;
                    let cs   = chord_size() as f32;
                    let note = notation();
                    use js_sys::Uint8Array;
                    use web_sys::{Blob, BlobPropertyBag, Url};
                    match pdf::generate_pdf_bytes(&s, note, pns, cs, 0) {
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
                    background: #1a1a2e;
                    color: #f0ece2;
                    border: none;
                    border-radius: 10px;
                    font-size: 15px;
                    font-weight: 700;
                    letter-spacing: 0.6px;
                    cursor: pointer;
                    font-family: inherit;
                ",
                onclick: move |_| {
                    let s   = song.read().clone();
                    let pns = part_name_size() as f32;
                    let cs  = chord_size() as f32;
                    let cap = 0_u8;
                    let note = notation();

                    // Collect: base sheet + one entry per instrument that has saved overrides.
                    // Each entry is (song_with_correct_parts, filename, capo_for_that_sheet).
                    let mut exports: Vec<(Song, String, u8)> = Vec::new();
                    exports.push((s.clone(), s.name.clone(), cap));
                    for inst in Instrument::all() {
                        if let Some(parts) = s.instrument_parts.get(inst.label()).cloned() {
                            let inst_cap = *s.instrument_capos.get(inst.label()).unwrap_or(&0);
                            let mut inst_sheet = s.clone();
                            inst_sheet.parts = parts;
                            let filename = format!("{}_{}", s.name, inst.label());
                            exports.push((inst_sheet, filename, inst_cap));
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

                            for (sheet, filename, sheet_cap) in &exports {
                                match pdf::generate_pdf_bytes(sheet, note, pns, cs, *sheet_cap) {
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
                    color: #f0ece2;
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
                    let pns     = part_name_size() as f32;
                    let cs      = chord_size() as f32;
                    let cap     = 0_u8;
                    let note    = notation();
                    let user_id = current_user.read().as_ref().map(|u| u.id).unwrap_or(0);
                    if let Some(db_ref) = db.read().as_ref() {
                        match db_ref.save_song(&s, user_id) {
                            Ok(song_id) => {
                                println!("✅  Song saved (id={song_id})");
                                // Also generate and store the current PDF
                                match pdf::generate_pdf_bytes(&s, note, pns, cs, cap) {
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
                                border-bottom: 1px solid #ece8df;
                                flex-shrink: 0;
                            ",
                            span {
                                style: "font-size: 15px; font-weight: 700; color: #1a1a2e; font-family: inherit;",
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
fn LoginScreen(db: Signal<Option<Db>>, mut current_user: Signal<Option<User>>) -> Element {
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error_msg: Signal<String> = use_signal(String::new);

    // true = show Register form, false = show Login form
    // Start on Register if there are no users yet, Login otherwise.
    let no_users = db
        .read()
        .as_ref()
        .and_then(|d| d.has_users().ok())
        .unwrap_or(false);
    let mut is_register = use_signal(move || !no_users);

    let form_title = if is_register() {
        "Create account"
    } else {
        "Sign in"
    };
    let submit_label = if is_register() { "Register" } else { "Log in" };
    let switch_label = if is_register() {
        "Already have an account? Log in"
    } else {
        "No account yet? Register"
    };

    rsx! {
        div {
            style: "
                background: #ffffff;
                border-radius: 16px;
                padding: 48px 52px;
                box-shadow: 0 4px 32px rgba(0,0,0,0.12);
                width: 380px;
                display: flex;
                flex-direction: column;
                gap: 16px;
            ",

            h2 {
                style: "margin: 0 0 8px; font-size: 26px; font-weight: 800; color: #1a1a2e;",
                "🎵  Chord Shifter"
            }
            p {
                style: "margin: 0 0 16px; font-size: 14px; color: #666;",
                "{form_title}"
            }

            // Username
            input {
                style: "
                    width: 100%; padding: 12px 14px; font-size: 14px;
                    border: 1.5px solid #d0cbc0; border-radius: 8px;
                    outline: none; font-family: inherit; box-sizing: border-box;
                ",
                r#type: "text",
                placeholder: "Username",
                value: "{username}",
                oninput: move |e| {
                    *username.write() = e.value();
                    *error_msg.write() = String::new();
                },
            }

            // Password
            input {
                style: "
                    width: 100%; padding: 12px 14px; font-size: 14px;
                    border: 1.5px solid #d0cbc0; border-radius: 8px;
                    outline: none; font-family: inherit; box-sizing: border-box;
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
                    padding: 14px; background: #1a1a2e; color: #f0ece2;
                    border: none; border-radius: 10px; font-size: 15px;
                    font-weight: 700; cursor: pointer; font-family: inherit;
                    letter-spacing: 0.5px;
                ",
                onclick: move |_| {
                    let u = username.read().trim().to_string();
                    let p = password.read().clone();
                    if u.is_empty() || p.is_empty() {
                        *error_msg.write() = "Please fill in all fields.".into();
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
                                        "Invalid username or password.".into();
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

// ── Tab grid editor ───────────────────────────────────────────────────────────

#[component]
fn TabEditor(song: Signal<Song>, part_index: usize) -> Element {
    let mut editing: Signal<Option<(usize, usize)>> = use_signal(|| None);
    let mut edit_buf: Signal<String> = use_signal(String::new);

    const STRING_NAMES: [&str; 6] = ["e", "B", "G", "D", "A", "E"];

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
                                div { style: "height: 1px; flex: 1; background: #b5d6b5;" }
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
                                div { style: "height: 1px; flex: 1; background: #b5d6b5;" }
                            }
                        }

                        // ── Segment block ─────────────────────────────────────────
                        div {
                            key: "seg-{seg_idx}",
                            style: "display: inline-block; background: #fff; border: 1.5px solid #b5d6b5; border-radius: 8px; padding: 10px 14px 12px;",

                            // Header row: delete buttons + (on last segment) add buttons
                            div {
                                style: "display: flex; align-items: center; margin-bottom: 2px; padding-left: 6px;",

                                for local_i in 0..seg_len {
                                    {
                                        let col = seg_cols[local_i];
                                        let col_w_kind = song
                                            .read()
                                            .parts
                                            .get(part_index)
                                            .and_then(|p| p.tab_grid.get(col))
                                            .map(|c| match c {
                                                TabCol::Barline => 1u8,
                                                TabCol::RepeatStart | TabCol::RepeatEnd => 2u8,
                                                _ => 0u8,
                                            })
                                            .unwrap_or(0);
                                        let w = match col_w_kind { 1 => "18px", 2 => "22px", _ => "36px" };
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
                                            style: "background: none; border: 1px dashed #8fba8f; border-radius: 4px; font-size: 12px; color: #5c7a5c; cursor: pointer; padding: 1px 7px; font-family: inherit;",
                                            title: "Add 4 beats",
                                            onclick: move |_| {
                                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                                    for _ in 0..4 {
                                                        part.tab_grid.push(TabCol::Notes([TabCell::Empty; 6]));
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
                                            style: "background: none; border: 1px dashed #3a5a8a; border-radius: 4px; font-size: 11px; font-weight: 700; color: #3a5a8a; cursor: pointer; padding: 1px 6px; font-family: Courier, monospace;",
                                            title: "Add repeat start (||:)",
                                            onclick: move |_| {
                                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                                    part.tab_grid.push(TabCol::RepeatStart);
                                                }
                                            },
                                            "||:"
                                        }
                                        button {
                                            style: "background: none; border: 1px dashed #3a5a8a; border-radius: 4px; font-size: 11px; font-weight: 700; color: #3a5a8a; cursor: pointer; padding: 1px 6px; font-family: Courier, monospace;",
                                            title: "Add repeat end (:||)",
                                            onclick: move |_| {
                                                if let Some(part) = song.write().parts.get_mut(part_index) {
                                                    part.tab_grid.push(TabCol::RepeatEnd);
                                                }
                                            },
                                            ":||"
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
                                    for si in 0usize..6 {
                                        div {
                                            key: "lbl-{seg_idx}-{si}",
                                            style: "height: 32px; width: 20px; display: flex; align-items: center; justify-content: flex-end;",
                                            span {
                                                style: "font-family: Courier, monospace; font-size: 12px; font-weight: 700; color: #5c7a5c;",
                                                "{STRING_NAMES[si]}"
                                            }
                                        }
                                    }
                                }
                                // the actual grid columns
                                div {
                                    style: "display: flex; flex-direction: column;",

                                    // ── Six string rows ───────────────────────────────────
                                    for str_idx in 0usize..6 {
                                        div {
                                            key: "row-{seg_idx}-{str_idx}",
                                            style: "display: flex; align-items: center; height: 32px;",

                                            div { style: "width: 6px; height: 2px; background: #aac8aa; flex-shrink: 0;" }

                                    for local_i in 0..seg_len {
                                        {
                                            let col = seg_cols[local_i];
                                            let col_kind = song
                                                .read()
                                                .parts
                                                .get(part_index)
                                                .and_then(|p| p.tab_grid.get(col))
                                                .map(|c| match c {
                                                    TabCol::Barline => 1u8,
                                                    TabCol::RepeatStart => 2u8,
                                                    TabCol::RepeatEnd => 3u8,
                                                    _ => 0u8,
                                                })
                                                .unwrap_or(0);
                                            let is_barline = col_kind == 1;
                                            let is_repeat_start = col_kind == 2;
                                            let is_repeat_end = col_kind == 3;

                                            if is_barline {
                                                rsx! {
                                                    div {
                                                        key: "{col}",
                                                        style: "position:relative;width:18px;height:32px;display:flex;align-items:center;justify-content:center;flex-shrink:0;",
                                                        div { style: "position:absolute;top:50%;left:0;right:0;height:2px;background:#aac8aa;transform:translateY(-50%);z-index:0;" }
                                                        div { style: "position:absolute;top:2px;bottom:2px;left:50%;width:2px;background:#6a7fa6;border-radius:1px;transform:translateX(-50%);z-index:1;" }
                                                    }
                                                }
                                            } else if is_repeat_start {
                                                // ||:  — thick line + thin line + dot on right
                                                rsx! {
                                                    div {
                                                        key: "{col}",
                                                        style: "position:relative;width:22px;height:32px;display:flex;align-items:center;justify-content:center;flex-shrink:0;",
                                                        div { style: "position:absolute;top:50%;left:0;right:0;height:2px;background:#aac8aa;transform:translateY(-50%);z-index:0;" }
                                                        div { style: "position:absolute;top:2px;bottom:2px;left:2px;width:4px;background:#3a5a8a;border-radius:1px;z-index:1;" }
                                                        div { style: "position:absolute;top:2px;bottom:2px;left:9px;width:2px;background:#3a5a8a;border-radius:1px;z-index:1;" }
                                                        div { style: "position:absolute;top:50%;left:15px;width:5px;height:5px;background:#3a5a8a;border-radius:50%;transform:translateY(-50%);z-index:1;" }
                                                    }
                                                }
                                            } else if is_repeat_end {
                                                // :||  — dot on left + thin line + thick line
                                                rsx! {
                                                    div {
                                                        key: "{col}",
                                                        style: "position:relative;width:22px;height:32px;display:flex;align-items:center;justify-content:center;flex-shrink:0;",
                                                        div { style: "position:absolute;top:50%;left:0;right:0;height:2px;background:#aac8aa;transform:translateY(-50%);z-index:0;" }
                                                        div { style: "position:absolute;top:50%;left:2px;width:5px;height:5px;background:#3a5a8a;border-radius:50%;transform:translateY(-50%);z-index:1;" }
                                                        div { style: "position:absolute;top:2px;bottom:2px;left:11px;width:2px;background:#3a5a8a;border-radius:1px;z-index:1;" }
                                                        div { style: "position:absolute;top:2px;bottom:2px;left:16px;width:4px;background:#3a5a8a;border-radius:1px;z-index:1;" }
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
                                                            Some(arr[str_idx])
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                    .unwrap_or(TabCell::Empty);
                                                let (cell_label, label_color, cell_bg) = match cell {
                                                    TabCell::Fret(n) => (n.to_string(), "#1a1a2e", "background:#d8edd8;"),
                                                    TabCell::Muted => ("x".to_string(), "#c0392b", "background:#fde8e8;"),
                                                    TabCell::Empty => ("\u{2013}".to_string(), "#c8dcc8", "background:transparent;"),
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
                                                            div { style: "position:absolute;top:50%;left:0;right:0;height:2px;background:#aac8aa;transform:translateY(-50%);z-index:0;" }
                                                            input {
                                                                style: "position:relative;z-index:1;width:30px;height:26px;font-family:Courier,monospace;font-size:13px;font-weight:700;text-align:center;border:2px solid #5c7a5c;border-radius:4px;background:#f6fbf6;outline:none;padding:0;box-sizing:border-box;",
                                                                r#type: "text",
                                                                maxlength: "2",
                                                                autofocus: true,
                                                                value: "{edit_buf}",
                                                                oninput: move |e| { edit_buf.set(e.value()); },
                                                                onblur: move |_| {
                                                                    let val = edit_buf.read().trim().to_lowercase();
                                                                    let new_cell = if val == "x" {
                                                                        TabCell::Muted
                                                                    } else if let Some(n) =
                                                                        val.parse::<u8>().ok().filter(|&n| n <= 24)
                                                                    {
                                                                        TabCell::Fret(n)
                                                                    } else {
                                                                        TabCell::Empty
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
                                                            div { style: "position:absolute;top:50%;left:0;right:0;height:2px;background:#aac8aa;transform:translateY(-50%);z-index:0;" }
                                                            div {
                                                                style: "{cell_style}",
                                                                onclick: move |_| {
                                                                    let init = match cell {
                                                                        TabCell::Fret(n) => n.to_string(),
                                                                        TabCell::Muted => "x".to_string(),
                                                                        TabCell::Empty => String::new(),
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
    capo: Signal<u8>,
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
        .map(|p| p.kind == PartKind::Riff)
        .unwrap_or(false);

    let part_border = if is_riff { "#b5d6b5" } else { "#ece8df" };
    let part_bg = if is_riff { "#f6fbf6" } else { "#fff" };
    let part_name_color = if is_riff { "#5c7a5c" } else { "#aaa" };
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

            if is_riff {
                TabEditor { song, part_index }
            }
            if !is_riff {
                // ── Chord items + add buttons ──────────────────────────────
                div {
                    style: "display: flex; flex-wrap: wrap; gap: 10px; align-items: flex-start;",

                    for item_index in 0..item_count {
                        {
                            let item = song
                                .read()
                                .parts
                                .get(part_index)
                                .and_then(|p| p.items.get(item_index))
                                .cloned();
                            match item {
                                Some(PartItem::Chord(_)) => rsx! {
                                    ChordEditor {
                                        key: "{item_index}",
                                        song,
                                        part_index,
                                        item_index,
                                        notation,
                                        capo,
                                    }
                                },
                                Some(PartItem::LineBreak) => rsx! {
                                    div {
                                        key: "{item_index}",
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
                                        key: "{item_index}",
                                        song,
                                        part_index,
                                        item_index,
                                        times,
                                    }
                                },
                                Some(PartItem::VoltaBracketStart { label }) => rsx! {
                                    VoltaEditor {
                                        key: "{item_index}",
                                        song,
                                        part_index,
                                        item_index,
                                        label,
                                        is_start: true,
                                    }
                                },
                                Some(PartItem::VoltaBracketEnd) => rsx! {
                                    VoltaEditor {
                                        key: "{item_index}",
                                        song,
                                        part_index,
                                        item_index,
                                        label: String::new(),
                                        is_start: false,
                                    }
                                },
                                Some(PartItem::RepeatStart) => rsx! {
                                    div {
                                        key: "{item_index}",
                                        style: "
                                            display: flex; flex-direction: column; align-items: center; gap: 4px;
                                            padding: 10px 12px 8px;
                                            background: #eef2fa; border: 2px solid #3a5a8a;
                                            border-radius: 12px; min-width: 52px; position: relative;
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
                                            style: "font-size: 22px; font-weight: 800; color: #3a5a8a; line-height: 1;",
                                            "||:"
                                        }
                                    }
                                },
                                Some(PartItem::RepeatEnd) => rsx! {
                                    div {
                                        key: "{item_index}",
                                        style: "
                                            display: flex; flex-direction: column; align-items: center; gap: 4px;
                                            padding: 10px 12px 8px;
                                            background: #eef2fa; border: 2px solid #3a5a8a;
                                            border-radius: 12px; min-width: 52px; position: relative;
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
                                            style: "font-size: 22px; font-weight: 800; color: #3a5a8a; line-height: 1;",
                                            ":||"
                                        }
                                    }
                                },
                                None => rsx! { span {} },
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
                                background: #f5f2ea; border: 2px dashed #c8c3b3;
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
                                    background: #f5f2ea; border: 1.5px solid #d0cbc0;
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
                                    background: #f5f2ea; border: 1.5px solid #d0cbc0;
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
                                    background: #f5f2ea; border: 1.5px solid #d0cbc0;
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
                                    background: #f5f2ea; border: 1.5px solid #d0cbc0;
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
                background: #f5f2ea; border: 2px solid #d9d4c5; border-radius: 12px;
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
                style: "font-size: 24px; font-weight: 800; color: #1a1a2e; line-height: 1;",
                "‖:"
            }
            div {
                style: "display: flex; align-items: center; gap: 4px;",
                span { style: "font-size: 11px; color: #888; font-weight: 700;", "×" }
                input {
                    style: "width: 36px; text-align: center; font-size: 12px; font-weight: 700;
                        border: 1px solid #d0cbc0; border-radius: 5px; background: #fff;
                        outline: none; padding: 3px; font-family: inherit; color: #1a1a2e;",
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
                        outline: none; padding: 3px; font-family: inherit; color: #1a1a2e;",
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
                        style: "margin: 0; font-size: 22px; font-weight: 800; color: #1a1a2e; letter-spacing: -0.3px;",
                        "🎵  Chord Shifter"
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
                        style: "margin: 0; font-size: 16px; font-weight: 800; color: #1a1a2e; letter-spacing: 0.3px;",
                        "📚  My Songs"
                    }
                    button {
                        style: "
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            width: 36px;
                            height: 36px;
                            background: #1a1a2e;
                            color: #f0ece2;
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
                                    background: #f7f5f0;
                                    border-radius: 10px;
                                    padding: 12px 14px;
                                    margin-bottom: 8px;
                                    cursor: pointer;
                                ",
                                onclick: move |_| { nav.push(Route::SongPage { id: row_id }); },

                                div {
                                    style: "flex: 1; overflow: hidden;",
                                    div {
                                        style: "font-size: 14px; font-weight: 700; color: #1a1a2e; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
                                        "{row.name}"
                                    }
                                    div {
                                        style: "font-size: 12px; color: #777; margin-top: 2px;",
                                        "{row.artist}"
                                    }
                                    // Instruments + username chips
                                    div {
                                        style: "display: flex; flex-wrap: wrap; align-items: center; gap: 5px; margin-top: 7px;",
                                        for inst in row.instruments.iter().cloned() {
                                            span {
                                                key: "{inst.label()}",
                                                style: "display: inline-flex; align-items: center; gap: 3px; font-size: 11px; font-weight: 600; background: #f0ece2; border: 1px solid #d8d4ca; border-radius: 6px; padding: 2px 7px; color: #555;",
                                                img { src: inst_icon(inst).to_string(), style: "width: 16px; height: 16px; object-fit: contain;", alt: "{inst.label()}" }
                                                "{inst.label()}"
                                            }
                                        }
                                        if !row.username.is_empty() {
                                            span {
                                                style: "display: inline-flex; align-items: center; gap: 3px; font-size: 11px; font-weight: 600; background: #e8f0e8; border: 1px solid #c8d8c8; border-radius: 6px; padding: 2px 7px; color: #2d6a4f;",
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
    let accent = inst.map(|i| i.accent_color()).unwrap_or("#1a1a2e");
    let inst_icon_asset = inst.map(inst_icon).unwrap_or(ICON_BASE);
    let inst_label = inst
        .map(|i| i.label())
        .unwrap_or_else(|| instrument.as_str());

    let mut capo = use_signal(|| 0_u8);
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
                    style: "border-bottom: 2px solid #e8e4da; padding-bottom: 28px; margin-bottom: 36px;",

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
                        img { src: inst_icon_asset.to_string(), style: "width: 36px; height: 36px; object-fit: contain;", alt: "{inst_label}" }
                        span { style: "font-size: 16px; font-weight: 800; letter-spacing: 0.5px;", "{inst_label}" }
                    }

                    // Song title + artist
                    h1 {
                        style: "margin: 0 0 6px; font-size: 38px; font-weight: 800; color: #1a1a2e; letter-spacing: -0.5px;",
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

                    // Capo control
                    div {
                        style: "margin-top: 14px; display: flex; align-items: center; gap: 10px;",
                        span {
                            style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;",
                            "Capo:"
                        }
                        button {
                            style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #d9d4c5; background: #f0ece2; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #1a1a2e;",
                            onclick: move |_| { if capo() > 0 { *capo.write() -= 1; } },
                            "−"
                        }
                        span {
                            style: "min-width: 52px; text-align: center; font-size: 13px; font-weight: 800; color: #1a1a2e;",
                            if capo() == 0 { "Off" } else { "{capo()}" }
                        }
                        button {
                            style: "width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid #d9d4c5; background: #f0ece2; font-size: 16px; font-weight: 700; cursor: pointer; font-family: inherit; display: flex; align-items: center; justify-content: center; color: #1a1a2e;",
                            onclick: move |_| { if capo() < 12 { *capo.write() += 1; } },
                            "+"
                        }
                        if capo() > 0 {
                            span {
                                style: "font-size: 11px; color: #888; font-style: italic;",
                                "→ play in {song.read().apply_capo(capo()).key}"
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
                        .map(|p| p.kind == PartKind::Riff)
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
                    let part_border = if is_riff { "#b5d6b5" } else { "#ece8df" };
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
                                        color: #1a1a2e;
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
                                            let capo_label = if capo() > 0 {
                                                let is_minor = song.read().key.to_lowercase().contains("minor");
                                                let shifted = song::shift_note(&chord.root, capo(), is_minor);
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
                                                        background: #f5f2ea;
                                                        border: 2px solid {accent};
                                                        border-radius: 12px;
                                                        padding: 14px 18px;
                                                        min-width: 72px;
                                                        text-align: center;
                                                    ",
                                                    span {
                                                        style: "font-size: 36px; font-weight: 800; color: #1a1a2e; letter-spacing: -1px; line-height: 1; display: block;",
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
                                                    background: #f0ece0;
                                                    border: 2px solid {accent};
                                                    border-radius: 12px;
                                                    padding: 14px 18px;
                                                    min-width: 72px;
                                                    text-align: center;
                                                ",
                                                span {
                                                    style: "font-size: 28px; font-weight: 800; color: #1a1a2e; line-height: 1; display: block;",
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
                                        Some(PartItem::RepeatStart) => rsx! {
                                            div {
                                                key: "{item_index}",
                                                style: "
                                                    background: #eef2fa;
                                                    border: 2px solid #3a5a8a;
                                                    border-radius: 12px;
                                                    padding: 14px 18px;
                                                    min-width: 64px;
                                                    text-align: center;
                                                ",
                                                span {
                                                    style: "font-size: 24px; font-weight: 800; color: #3a5a8a; line-height: 1; display: block;",
                                                    "||:"
                                                }
                                            }
                                        },
                                        Some(PartItem::RepeatEnd) => rsx! {
                                            div {
                                                key: "{item_index}",
                                                style: "
                                                    background: #eef2fa;
                                                    border: 2px solid #3a5a8a;
                                                    border-radius: 12px;
                                                    padding: 14px 18px;
                                                    min-width: 64px;
                                                    text-align: center;
                                                ",
                                                span {
                                                    style: "font-size: 24px; font-weight: 800; color: #3a5a8a; line-height: 1; display: block;",
                                                    ":||"
                                                }
                                            }
                                        },
                                        None => rsx! { div { key: "{item_index}" } },
                                    }
                                }}
                            }  // end else div
                            }  // end else branch
                        }
                    }
                }}

                // ── Vocals / notes (read-only) ─────────────────────────────────────────────
                if !song.read().vocals_notes.is_empty() {
                    div {
                        style: "border: 1.5px solid #e8e4da; border-radius: 12px; overflow: hidden;",
                        div {
                            style: "display: flex; align-items: center; gap: 8px; padding: 10px 16px; background: #f7f5f0; border-bottom: 1.5px solid #e8e4da;",
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
    capo: Signal<u8>,
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

    let display_label = if capo() > 0 {
        // Show the chord shape the player needs to play with the capo.
        let is_minor = song.read().key.to_lowercase().contains("minor");
        let shifted_root =
            apply_notation(&song::shift_note(&chord.root, capo(), is_minor), notation());
        let shifted_bass = chord.bass_note.as_deref().map(|b| {
            format!(
                "/{}",
                apply_notation(&song::shift_note(b, capo(), is_minor), notation())
            )
        });
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
                background: #f5f2ea;
                border: 2px solid #d9d4c5;
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
                    color: #1a1a2e;
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
                    border-top: 1px solid #d9d4c5;
                    padding-top: 8px;
                    width: 100%;
                ",

                // Root note input
                input {
                    style: "
                        width: 70px;
                        font-size: 13px;
                        font-weight: 600;
                        color: #1a1a2e;
                        text-align: center;
                        border: 1px solid #d0cbc0;
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
                        border: 1px solid #d0cbc0;
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
                        color: #1a1a2e;
                        text-align: center;
                        border: 1px solid #d0cbc0;
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
