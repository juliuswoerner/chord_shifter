#![allow(non_snake_case)]

use dioxus::prelude::*;

mod pdf;
mod song;

// ── Web-only: trigger a browser PDF download ──────────────────────────────────
#[cfg(target_arch = "wasm32")]
fn trigger_download(bytes: Vec<u8>, filename: &str) {
    use js_sys::Uint8Array;
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

    let array = Uint8Array::from(bytes.as_slice());
    let parts  = js_sys::Array::new();
    parts.push(&array);

    let opts = BlobPropertyBag::new();
    opts.set_type("application/pdf");

    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
        .expect("blob");
    let url  = Url::create_object_url_with_blob(&blob).expect("object url");

    let window   = web_sys::window().expect("window");
    let document = window.document().expect("document");
    let a: HtmlAnchorElement = document
        .create_element("a").expect("a")
        .dyn_into().expect("cast");

    a.set_href(&url);
    a.set_download(&format!("{filename}.pdf"));
    a.click();
    let _ = Url::revoke_object_url(&url);
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
            ",
            SongView { song }
        }
    }
}

// ── Song sheet ────────────────────────────────────────────────────────────────

#[component]
fn SongView(song: Signal<Song>) -> Element {
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
    let mut part_name_size  = use_signal(|| 9_u32);
    let mut chord_size      = use_signal(|| 18_u32);

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
        }
    }
}

// ── Part block ───────────────────────────────────────────────────────────────

#[component]
fn PartView(song: Signal<Song>, part_index: usize, show_degrees: Signal<bool>) -> Element {
    let chord_count = song.read().parts.get(part_index).map(|p| p.chords.len()).unwrap_or(0);
    let part_name   = song.read().parts.get(part_index).map(|p| p.name.clone()).unwrap_or_default();

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

// ── Chord editor ─────────────────────────────────────────────────────────────

#[component]
fn ChordEditor(song: Signal<Song>, part_index: usize, chord_index: usize, show_degrees: Signal<bool>) -> Element {
    let chord = song
        .read()
        .parts
        .get(part_index)
        .and_then(|p| p.chords.get(chord_index))
        .cloned()
        .unwrap_or_else(|| Chord::new("C", ChordQuality::Major));

    let display_label = if show_degrees() { chord.degree_display() } else { chord.display() };

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
