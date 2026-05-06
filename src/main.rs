#![allow(non_snake_case)]

use dioxus::prelude::*;

mod auth;
#[cfg(not(target_arch = "wasm32"))]
mod db;
mod pdf;
mod song;

// ── Web-only: trigger a browser PDF download ──────────────────────────────────
#[cfg(target_arch = "wasm32")]
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

#[cfg(not(target_arch = "wasm32"))]
use db::{Db, SongRow, User};

// ── Web storage backend (localStorage, wasm32 only) ──────────────────────────
// Desktop uses SQLite via db.rs; the browser uses localStorage so the Library
// panel works the same way on both platforms.

#[cfg(target_arch = "wasm32")]
const LS_SONGS_KEY: &str = "chord_shifter_songs";
#[cfg(target_arch = "wasm32")]
const LS_USERS_KEY: &str = "chord_shifter_users";

#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredUser {
    id: i64,
    username: String,
    password_hash: String,
}

#[cfg(target_arch = "wasm32")]
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
}

#[cfg(target_arch = "wasm32")]
fn ls_read_users() -> Vec<StoredUser> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(LS_USERS_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn ls_write_users(users: &[StoredUser]) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        if let Ok(json) = serde_json::to_string(users) {
            let _ = storage.set_item(LS_USERS_KEY, &json);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn ls_read() -> Vec<StoredSong> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(LS_SONGS_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn ls_write(songs: &[StoredSong]) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        if let Ok(json) = serde_json::to_string(songs) {
            let _ = storage.set_item(LS_SONGS_KEY, &json);
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
struct Db;

#[cfg(target_arch = "wasm32")]
impl Db {
    fn open(_: &str) -> Result<Self, String> {
        // Seed the example song into localStorage on first run.
        if ls_read().is_empty() {
            use song::{Chord, ChordQuality};
            let s = song::Song::new("Let It Be", "C Major", "The Beatles")
                .with_part(
                    "Verse",
                    vec![
                        Chord::new("C", ChordQuality::Major).with_degree(1),
                        Chord::new("G", ChordQuality::Major).with_degree(5),
                        Chord::new("A", ChordQuality::Minor).with_degree(6),
                        Chord::new("F", ChordQuality::Major).with_degree(4),
                    ],
                )
                .with_part(
                    "Chorus",
                    vec![
                        Chord::new("F", ChordQuality::Major).with_degree(4),
                        Chord::new("C", ChordQuality::Major).with_degree(1),
                        Chord::new("G", ChordQuality::Major).with_degree(5),
                        Chord::new("F", ChordQuality::Major).with_degree(4),
                    ],
                )
                .with_part(
                    "Bridge",
                    vec![
                        Chord::new("G", ChordQuality::Major).with_degree(5),
                        Chord::new("F", ChordQuality::Major).with_degree(4),
                        Chord::new("C", ChordQuality::Major).with_degree(1),
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
            }]);
        }
        Ok(Self)
    }

    fn save_song(&self, song: &song::Song, user_id: i64) -> Result<i64, String> {
        let parts_json = serde_json::to_string(&song.parts).map_err(|e| e.to_string())?;
        let instruments_json =
            serde_json::to_string(&song.instruments).map_err(|e| e.to_string())?;
        let mut songs = ls_read();
        if let Some(row) = songs
            .iter_mut()
            .find(|s| s.name == song.name && s.artist == song.artist && s.user_id == user_id)
        {
            row.key = song.key.clone();
            row.parts_json = parts_json;
            row.instruments_json = instruments_json;
            row.vocals_notes = song.vocals_notes.clone();
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
                Ok(song::Song {
                    name: s.name,
                    artist: s.artist,
                    key: s.key,
                    parts,
                    instruments,
                    vocals_notes: s.vocals_notes,
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

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
struct User {
    id: i64,
    username: String,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
struct SongRow {
    id: i64,
    name: String,
    artist: String,
    instruments: Vec<song::Instrument>,
    username: String,
}

use song::{Chord, ChordQuality, Instrument, ScaleDegree, Song};

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
                Chord::new("C", ChordQuality::Major).with_degree(1),
                Chord::new("G", ChordQuality::Major).with_degree(5),
                Chord::new("A", ChordQuality::Minor).with_degree(6),
                Chord::new("F", ChordQuality::Major).with_degree(4),
            ],
        )
        .with_part(
            "Chorus",
            vec![
                Chord::new("F", ChordQuality::Major).with_degree(4),
                Chord::new("C", ChordQuality::Major).with_degree(1),
                Chord::new("G", ChordQuality::Major).with_degree(5),
                Chord::new("F", ChordQuality::Major).with_degree(4),
            ],
        )
        .with_part(
            "Bridge",
            vec![
                Chord::new("G", ChordQuality::Major).with_degree(5),
                Chord::new("F", ChordQuality::Major).with_degree(4),
                Chord::new("C", ChordQuality::Major).with_degree(1),
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
    let mut show_degrees = use_signal(|| false);

    let chords_btn_style = if !show_degrees() {
        "padding: 5px 16px; border-radius: 16px; border: none; font-size: 12px; font-weight: 700; font-family: inherit; cursor: pointer; background: #1a1a2e; color: #f0ece2;"
    } else {
        "padding: 5px 16px; border-radius: 16px; border: none; font-size: 12px; font-weight: 700; font-family: inherit; cursor: pointer; background: transparent; color: #aaa;"
    };
    let degrees_btn_style = if show_degrees() {
        "padding: 5px 16px; border-radius: 16px; border: none; font-size: 12px; font-weight: 700; font-family: inherit; cursor: pointer; background: #1a1a2e; color: #f0ece2;"
    } else {
        "padding: 5px 16px; border-radius: 16px; border: none; font-size: 12px; font-weight: 700; font-family: inherit; cursor: pointer; background: transparent; color: #aaa;"
    };

    let mut transpose_root = use_signal(|| "C".to_string());
    let mut capo = use_signal(|| 0_u8);
    let mut part_name_size = use_signal(|| 9_u32);
    let mut chord_size = use_signal(|| 18_u32);

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

                // Key pill with inline input
                div {
                    style: "display: inline-flex; align-items: center; background: #1a1a2e; border-radius: 20px; overflow: hidden;",
                    span {
                        style: "color: #f0ece2; padding: 5px 6px 5px 16px; font-size: 12px; font-weight: 700; letter-spacing: 1.2px; text-transform: uppercase; white-space: nowrap;",
                        "Key:"
                    }
                    input {
                        style: "
                            color: #f0ece2;
                            background: transparent;
                            border: none;
                            outline: none;
                            padding: 5px 16px 5px 4px;
                            font-size: 12px;
                            font-weight: 700;
                            letter-spacing: 1.2px;
                            text-transform: uppercase;
                            font-family: inherit;
                            width: 90px;
                        ",
                        value: "{song.read().key}",
                        placeholder: "C Major",
                        oninput: move |e| song.write().key = e.value(),
                    }
                }

                // Display mode toggle
                div {
                    style: "margin-top: 16px; display: flex; align-items: center; gap: 10px;",
                    span {
                        style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;",
                        "Show:"
                    }
                    div {
                        style: "display: flex; background: #f0ece2; border-radius: 20px; padding: 3px; border: 1px solid #e0dbd0;",
                        button {
                            style: "{chords_btn_style}",
                            onclick: move |_| *show_degrees.write() = false,
                            "Chords"
                        }
                        button {
                            style: "{degrees_btn_style}",
                            onclick: move |_| *show_degrees.write() = true,
                            "Degrees"
                        }
                    }
                }

                // Transpose row
                div {
                    style: "margin-top: 14px; display: flex; align-items: center; gap: 10px;",
                    span {
                        style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;",
                        "Transpose to:"
                    }
                    select {
                        style: "
                            font-size: 13px;
                            font-weight: 700;
                            color: #1a1a2e;
                            background: #f0ece2;
                            border: 1px solid #d9d4c5;
                            border-radius: 8px;
                            padding: 5px 10px;
                            outline: none;
                            cursor: pointer;
                            font-family: inherit;
                        ",
                        onchange: move |e| *transpose_root.write() = e.value(),
                        for root in ["C","C#","D","Eb","E","F","F#","G","Ab","A","Bb","B"] {
                            option { value: "{root}", "{root}" }
                        }
                    }
                    button {
                        style: "
                            padding: 5px 18px;
                            background: #1a1a2e;
                            color: #f0ece2;
                            border: none;
                            border-radius: 8px;
                            font-size: 12px;
                            font-weight: 700;
                            letter-spacing: 0.5px;
                            cursor: pointer;
                            font-family: inherit;
                        ",
                        onclick: move |_| {
                            let root = transpose_root.read().clone();
                            song.write().transpose_to(&root);
                        },
                        "Transpose"
                    }
                }

                // Capo row
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

                // ── Instrument picker ─────────────────────────────────────────
                div {
                    style: "margin-top: 18px; display: flex; align-items: center; gap: 12px; flex-wrap: wrap;",
                    span {
                        style: "font-size: 11px; font-weight: 700; color: #aaa; text-transform: uppercase; letter-spacing: 1.2px;",
                        "Instruments:"
                    }
                    div {
                        style: "display: flex; gap: 8px; flex-wrap: wrap;",
                        for inst in Instrument::all() {
                            {
                                let is_active = song.read().instruments.contains(&inst);
                                let btn_style = if is_active {
                                    "display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#1a1a2e;border:none;border-radius:10px;cursor:pointer;font-family:inherit;"
                                } else {
                                    "display:flex;flex-direction:column;align-items:center;gap:3px;padding:8px 14px;background:#f0ece2;border:1.5px solid #d8d4ca;border-radius:10px;cursor:pointer;font-family:inherit;"
                                };
                                let icon_style = if is_active {
                                    "font-size:22px;"
                                } else {
                                    "font-size:22px;opacity:0.35;"
                                };
                                let label_style = if is_active {
                                    "font-size:9px;font-weight:700;letter-spacing:0.8px;text-transform:uppercase;color:#f0ece2;"
                                } else {
                                    "font-size:9px;font-weight:700;letter-spacing:0.8px;text-transform:uppercase;color:#888;"
                                };
                                rsx! {
                                    button {
                                        key: "{inst.label()}",
                                        style: "{btn_style}",
                                        title: "{inst.label()}",
                                        onclick: move |_| {
                                            if let Some(id) = song_id {
                                                nav.push(Route::InstrumentSheetPage {
                                                    id,
                                                    instrument: inst.label().to_string(),
                                                });
                                            } else {
                                                let mut s = song.write();
                                                if s.instruments.contains(&inst) {
                                                    s.instruments.retain(|i| i != &inst);
                                                } else {
                                                    s.instruments.push(inst);
                                                }
                                            }
                                        },
                                        span { style: "{icon_style}", "{inst.icon()}" }
                                        span { style: "{label_style}", "{inst.label()}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Parts ─────────────────────────────────────────────────────────
            for part_index in 0..song.read().parts.len() {
                PartView { key: "{part_index}", song, part_index, show_degrees, capo }
            }

            // ── Add part button ───────────────────────────────────────────────
            button {
                style: "
                    margin-top: 8px;
                    margin-bottom: 24px;
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
                onclick: move |_| {
                    song.write().parts.push(
                        crate::song::SongPart::new("New Part")
                    );
                },
                "+ Add Part"
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
                    let deg = show_degrees();
                    let pns = part_name_size() as f32;
                    let cs  = chord_size() as f32;
                    let cap = capo();

                    // Collect: base sheet + one entry per instrument
                    let mut exports: Vec<(Song, String)> = Vec::new();
                    exports.push((s.clone(), s.name.clone()));
                    for inst in Instrument::all() {
                        let filename = format!("{}_{}", s.name, inst.label());
                        exports.push((s.clone(), filename));
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    for (sheet, filename) in &exports {
                        let path = format!("{filename}.pdf");
                        match pdf::save_pdf(sheet, &path, deg, pns, cs, cap) {
                            Ok(_)  => println!("✅  PDF saved to {path}"),
                            Err(e) => eprintln!("❌  PDF export failed for {path}: {e}"),
                        }
                    }

                    #[cfg(target_arch = "wasm32")]
                    for (sheet, filename) in exports {
                        match pdf::generate_pdf_bytes(&sheet, deg, pns, cs, cap) {
                            Ok(bytes) => trigger_download(bytes, &filename),
                            Err(e)    => web_sys::console::error_1(
                                &format!("PDF export failed for {filename}: {e}").into(),
                            ),
                        }
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
                    let deg     = show_degrees();
                    let pns     = part_name_size() as f32;
                    let cs      = chord_size() as f32;
                    let cap     = capo();
                    let user_id = current_user.read().as_ref().map(|u| u.id).unwrap_or(0);
                    if let Some(db_ref) = db.read().as_ref() {
                        match db_ref.save_song(&s, user_id) {
                            Ok(song_id) => {
                                println!("✅  Song saved (id={song_id})");
                                // Also generate and store the current PDF
                                match pdf::generate_pdf_bytes(&s, deg, pns, cs, cap) {
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

// ── Part block ────────────────────────────────────────────────────────────────

#[component]
fn PartView(
    song: Signal<Song>,
    part_index: usize,
    show_degrees: Signal<bool>,
    capo: Signal<u8>,
) -> Element {
    let chord_count = song
        .read()
        .parts
        .get(part_index)
        .map(|p| p.chords.len())
        .unwrap_or(0);
    let part_name = song
        .read()
        .parts
        .get(part_index)
        .map(|p| p.name.clone())
        .unwrap_or_default();

    rsx! {
        div {
            style: "
                margin-bottom: 32px;
                border: 1px solid #ece8df;
                border-radius: 12px;
                padding: 18px 20px 16px;
                position: relative;
            ",

            // Remove part button
            button {
                style: "
                    position: absolute;
                    top: 14px;
                    right: 14px;
                    background: none;
                    border: none;
                    font-size: 13px;
                    color: #ccc;
                    cursor: pointer;
                    padding: 0;
                    font-family: inherit;
                    line-height: 1;
                ",
                onclick: move |_| {
                    let mut s = song.write();
                    if part_index < s.parts.len() {
                        s.parts.remove(part_index);
                    }
                },
                "\u{2715}"
            }

            // Editable part name
            input {
                style: "
                    display: block;
                    margin: 0 0 14px 0;
                    font-size: 10px;
                    font-weight: 700;
                    text-transform: uppercase;
                    letter-spacing: 3px;
                    color: #aaa;
                    border: none;
                    border-bottom: 1px dashed #ddd;
                    background: transparent;
                    outline: none;
                    font-family: inherit;
                    padding: 0 0 3px;
                    width: calc(100% - 24px);
                ",
                value: "{part_name}",
                placeholder: "Part name…",
                oninput: move |e| {
                    if let Some(part) = song.write().parts.get_mut(part_index) {
                        part.name = e.value();
                    }
                },
            }

            // Chord row
            div {
                style: "display: flex; flex-wrap: wrap; gap: 10px; align-items: flex-start;",

                for chord_index in 0..chord_count {
                    ChordEditor { key: "{chord_index}", song, part_index, chord_index, show_degrees, capo }
                }

                // Add chord button
                button {
                    style: "
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        width: 44px;
                        height: 44px;
                        background: #f5f2ea;
                        border: 2px dashed #c8c3b3;
                        border-radius: 10px;
                        font-size: 22px;
                        color: #bbb;
                        cursor: pointer;
                        padding: 0;
                        flex-shrink: 0;
                        align-self: center;
                        font-family: inherit;
                    ",
                    onclick: move |_| {
                        if let Some(part) = song.write().parts.get_mut(part_index) {
                            part.chords.push(Chord::new("C", ChordQuality::Major));
                        }
                    },
                    "+"
                }
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
                                                span { style: "font-size: 13px;", "{inst.icon()}" }
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
    let inst_icon = inst.map(|i| i.icon()).unwrap_or("🎵");
    let inst_label = inst
        .map(|i| i.label())
        .unwrap_or_else(|| instrument.as_str());

    let mut capo = use_signal(|| 0_u8);

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
                        span { style: "font-size: 28px; line-height: 1;", "{inst_icon}" }
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

                // ── Chord parts (read-only) ──────────────────────────────────────────────────
                for part_index in 0..song.read().parts.len() {{
                    let part_name = song
                        .read()
                        .parts
                        .get(part_index)
                        .map(|p| p.name.clone())
                        .unwrap_or_default();
                    let chord_count = song
                        .read()
                        .parts
                        .get(part_index)
                        .map(|p| p.chords.len())
                        .unwrap_or(0);
                    rsx! {
                        div {
                            key: "{part_index}",
                            style: "margin-bottom: 32px; border: 1px solid #ece8df; border-radius: 12px; padding: 18px 20px 16px;",
                            p {
                                style: "margin: 0 0 14px; font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 3px; color: #aaa;",
                                "{part_name}"
                            }
                            div {
                                style: "display: flex; flex-wrap: wrap; gap: 10px;",
                                for chord_index in 0..chord_count {{
                                    let chord = song
                                        .read()
                                        .parts
                                        .get(part_index)
                                        .and_then(|p| p.chords.get(chord_index))
                                        .cloned()
                                        .unwrap_or_else(|| Chord::new("C", ChordQuality::Major));
                                    let capo_label = if capo() > 0 {
                                        format!("{}{}", song::shift_note(&chord.root, capo()), chord.quality.symbol())
                                    } else {
                                        chord.display()
                                    };
                                    rsx! {
                                        div {
                                            key: "{chord_index}",
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
                                }}
                            }
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
    chord_index: usize,
    show_degrees: Signal<bool>,
    capo: Signal<u8>,
) -> Element {
    let chord = song
        .read()
        .parts
        .get(part_index)
        .and_then(|p| p.chords.get(chord_index))
        .cloned()
        .unwrap_or_else(|| Chord::new("C", ChordQuality::Major));

    let display_label = if show_degrees() {
        chord.degree_display()
    } else if capo() > 0 {
        // Show the chord shape the player needs to play with the capo.
        format!(
            "{}{}",
            song::shift_note(&chord.root, capo()),
            chord.quality.symbol()
        )
    } else {
        chord.display()
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
                        if chord_index < part.chords.len() {
                            part.chords.remove(chord_index);
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
                            if let Some(c) = part.chords.get_mut(chord_index) {
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
                            if let Some(c) = part.chords.get_mut(chord_index) {
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

                // Degree input
                input {
                    r#type: "number",
                    min: "1",
                    max: "7",
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
                    value: chord.degree.map(|d| d.get().to_string()).unwrap_or_default(),
                    placeholder: "Degree",
                    oninput: move |e: Event<FormData>| {
                        if let Some(part) = song.write().parts.get_mut(part_index) {
                            if let Some(c) = part.chords.get_mut(chord_index) {
                                c.degree = e.value().parse::<u8>().ok()
                                    .and_then(ScaleDegree::new);
                            }
                        }
                    }
                }
            }
        }
    }
}
