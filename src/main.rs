#![allow(non_snake_case)]

use dioxus::prelude::*;

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
use db::{Db, SongRow};

// ── Web storage backend (localStorage, wasm32 only) ──────────────────────────
// Desktop uses SQLite via db.rs; the browser uses localStorage so the Library
// panel works the same way on both platforms.

#[cfg(target_arch = "wasm32")]
const LS_SONGS_KEY: &str = "chord_shifter_songs";

#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize, serde::Deserialize)]
struct StoredSong {
    id: i64,
    name: String,
    artist: String,
    key: String,
    parts_json: String,
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
        Ok(Self)
    }

    fn save_song(&self, song: &song::Song) -> Result<i64, String> {
        let parts_json = serde_json::to_string(&song.parts).map_err(|e| e.to_string())?;
        let mut songs = ls_read();
        if let Some(row) = songs
            .iter_mut()
            .find(|s| s.name == song.name && s.artist == song.artist)
        {
            row.key = song.key.clone();
            row.parts_json = parts_json;
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
            });
            ls_write(&songs);
            Ok(id)
        }
    }

    fn list_songs(&self) -> Result<Vec<SongRow>, String> {
        Ok(ls_read()
            .into_iter()
            .map(|s| SongRow {
                id: s.id,
                name: s.name,
                artist: s.artist,
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
                Ok(song::Song {
                    name: s.name,
                    artist: s.artist,
                    key: s.key,
                    parts,
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
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
struct SongRow {
    id: i64,
    name: String,
    artist: String,
}

use song::{Chord, ChordQuality, ScaleDegree, Song};

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

// ── Root component ────────────────────────────────────────────────────────────

#[component]
fn App() -> Element {
    let song = use_signal(example_song);

    let db: Signal<Option<Db>> = use_signal(|| {
        Db::open("chord_shifter.db")
            .map_err(|e| eprintln!("DB open failed: {e}"))
            .ok()
    });
    // Incremented after every successful save; SongLibrary watches it to re-fetch.
    let library_rev: Signal<u32> = use_signal(|| 0);

    rsx! {
        div {
            style: "
                font-family: 'Helvetica Neue', Arial, sans-serif;
                min-height: 100vh;
                background: #f0ece2;
                display: flex;
                align-items: flex-start;
                justify-content: center;
                padding: 48px 20px;
                gap: 32px;
            ",

            // ── Song library sidebar ───────────────────────────────────────
            SongLibrary { song, db, library_rev }

            SongView { song, db, library_rev }
        }
    }
}

// ── Song sheet ────────────────────────────────────────────────────────────────

#[component]
fn SongView(song: Signal<Song>, db: Signal<Option<Db>>, mut library_rev: Signal<u32>) -> Element {
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
            }

            // ── Parts ─────────────────────────────────────────────────────────
            for part_index in 0..song.read().parts.len() {
                PartView { key: "{part_index}", song, part_index, show_degrees }
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

                    #[cfg(not(target_arch = "wasm32"))]
                    match pdf::save_pdf(&s, "chord_sheet.pdf", deg, pns, cs) {
                        Ok(_)  => println!("✅  PDF saved to chord_sheet.pdf"),
                        Err(e) => eprintln!("❌  PDF export failed: {e}"),
                    }

                    #[cfg(target_arch = "wasm32")]
                    match pdf::generate_pdf_bytes(&s, deg, pns, cs) {
                        Ok(bytes) => trigger_download(bytes, &s.name),
                        Err(e)    => web_sys::console::error_1(
                            &format!("PDF export failed: {e}").into(),
                        ),
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
                    let s   = song.read().clone();
                    let deg = show_degrees();
                    let pns = part_name_size() as f32;
                    let cs  = chord_size() as f32;
                    if let Some(db_ref) = db.read().as_ref() {
                        match db_ref.save_song(&s) {
                            Ok(song_id) => {
                                println!("✅  Song saved (id={song_id})");
                                *library_rev.write() += 1;
                                // Also generate and store the current PDF
                                match pdf::generate_pdf_bytes(&s, deg, pns, cs) {
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

// ── Part block ────────────────────────────────────────────────────────────────

#[component]
fn PartView(song: Signal<Song>, part_index: usize, show_degrees: Signal<bool>) -> Element {
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
                    ChordEditor { key: "{chord_index}", song, part_index, chord_index, show_degrees }
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

// ── Song library sidebar ──────────────────────────────────────────────────────

#[component]
fn SongLibrary(song: Signal<Song>, db: Signal<Option<Db>>, library_rev: Signal<u32>) -> Element {
    // Local list of song rows, refreshed on demand
    let mut rows: Signal<Vec<SongRow>> = use_signal(Vec::new);
    let mut status: Signal<String> = use_signal(String::new);

    // Re-fetch whenever library_rev changes (incremented by Save button)
    use_effect(move || {
        let _rev = library_rev();
        if let Some(db_ref) = db.read().as_ref() {
            match db_ref.list_songs() {
                Ok(list) => *rows.write() = list,
                Err(e) => *status.write() = format!("Load error: {e}"),
            }
        }
    });

    rsx! {
        div {
            style: "
                background: #ffffff;
                border-radius: 14px;
                padding: 24px 20px;
                box-shadow: 0 4px 32px rgba(0,0,0,0.10);
                width: 260px;
                min-width: 220px;
                align-self: flex-start;
                display: flex;
                flex-direction: column;
                gap: 8px;
            ",

            // Header + Refresh
            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 4px;",
                h3 {
                    style: "margin: 0; font-size: 15px; font-weight: 800; color: #1a1a2e; letter-spacing: 0.5px;",
                    "📚  Library"
                }
                button {
                    style: "
                        padding: 4px 10px;
                        background: #f0ece2;
                        border: 1px solid #d8d4ca;
                        border-radius: 8px;
                        font-size: 12px;
                        font-weight: 700;
                        cursor: pointer;
                        font-family: inherit;
                        color: #1a1a2e;
                    ",
                    onclick: move |_| {
                        if let Some(db_ref) = db.read().as_ref() {
                            match db_ref.list_songs() {
                                Ok(list) => *rows.write() = list,
                                Err(e) => *status.write() = format!("Refresh error: {e}"),
                            }
                        }
                    },
                    "↻  Refresh"
                }
            }

            // Status message
            if !status.read().is_empty() {
                p { style: "color: #c0392b; font-size: 12px; margin: 0;", "{status}" }
            }

            // Song list
            if rows.read().is_empty() {
                p {
                    style: "color: #aaa; font-size: 13px; margin: 0; text-align: center; padding: 16px 0;",
                    "No songs saved yet."
                }
            }

            for row in rows.read().iter().cloned() {
                {
                    let row_id = row.id;
                    rsx! {
                        div {
                            key: "{row_id}",
                            style: "
                                display: flex;
                                align-items: center;
                                gap: 6px;
                                background: #f7f5f0;
                                border-radius: 8px;
                                padding: 8px 10px;
                            ",

                            // Load button (the song title)
                            button {
                                style: "
                                    flex: 1;
                                    text-align: left;
                                    background: none;
                                    border: none;
                                    cursor: pointer;
                                    font-family: inherit;
                                    font-size: 13px;
                                    font-weight: 700;
                                    color: #1a1a2e;
                                    padding: 0;
                                    overflow: hidden;
                                    text-overflow: ellipsis;
                                    white-space: nowrap;
                                ",
                                title: "{row.name} – {row.artist}",
                                onclick: move |_| {
                                    if let Some(db_ref) = db.read().as_ref() {
                                        match db_ref.load_song(row_id) {
                                            Ok(loaded) => *song.write() = loaded,
                                            Err(e) => eprintln!("Load error: {e}"),
                                        }
                                    }
                                },
                                "{row.name}"
                                span {
                                    style: "font-weight: 400; font-size: 11px; color: #777; display: block;",
                                    "{row.artist}"
                                }
                            }

                            // Delete button
                            button {
                                style: "
                                    background: none;
                                    border: none;
                                    cursor: pointer;
                                    font-size: 14px;
                                    padding: 2px 4px;
                                    color: #c0392b;
                                    border-radius: 4px;
                                ",
                                title: "Delete",
                                onclick: move |_| {
                                    if let Some(db_ref) = db.read().as_ref() {
                                        let _ = db_ref.delete_song(row_id);
                                        match db_ref.list_songs() {
                                            Ok(list) => *rows.write() = list,
                                            Err(e) => *status.write() = format!("Refresh error: {e}"),
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
// ── Chord editor ─────────────────────────────────────────────────────────────

#[component]
fn ChordEditor(
    song: Signal<Song>,
    part_index: usize,
    chord_index: usize,
    show_degrees: Signal<bool>,
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
