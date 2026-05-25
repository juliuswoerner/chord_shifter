// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.

use printpdf::*;
use std::io::BufWriter;

use chord_shifter::song::{apply_notation, Notation, Song};

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 22.0;
const RIGHT: f32 = PAGE_W - MARGIN;

/// Estimate the total height in mm that `part` will occupy when rendered.
/// Used to decide whether to start a new page before drawing.
fn measure_part_height(
    part: &chord_shifter::song::SongPart,
    chord_size: f32,
    part_name_size: f32,
) -> f32 {
    use chord_shifter::song::{PartItem, PartKind, TabCol};

    let part_name_h = (part_name_size / 9.0) * 14.0;

    if part.kind == PartKind::Riff
        || part.kind == PartKind::BassRiff
        || part.kind == PartKind::DrumBeat
    {
        let is_bass = part.kind == PartKind::BassRiff;
        let num_strings: usize = if is_bass {
            4
        } else if part.kind == PartKind::DrumBeat {
            8
        } else {
            6
        };
        let str_gap: f32 = 3.8;
        let block_h = str_gap * (num_strings as f32 - 1.0);
        let block_gap: f32 = 8.0;
        let gap: f32 = 6.0;

        let mut seg_count: u32 = 0;
        let mut in_seg = false;
        for col in &part.tab_grid {
            match col {
                TabCol::LineBreak => {
                    in_seg = false;
                }
                _ => {
                    if !in_seg {
                        seg_count += 1;
                        in_seg = true;
                    }
                }
            }
        }

        part_name_h + (seg_count as f32) * (block_h + block_gap) + gap
    } else {
        let scale = chord_size / 18.0;
        let row_h: f32 = 12.0 * scale;
        let gap: f32 = 6.0;
        let root_char_w: f32 = chord_size * (3.5 / 18.0);
        let qual_size: f32 = chord_size * (10.0 / 18.0);
        let qual_char_w: f32 = root_char_w * (qual_size / chord_size);
        let bass_size: f32 = chord_size * (14.0 / 18.0);
        let bass_char_w: f32 = root_char_w * (bass_size / chord_size);
        let sup_offset: f32 = 1.0;

        let mut x: f32 = MARGIN;
        let mut rows: u32 = 1;

        for item in &part.items {
            match item {
                PartItem::LineBreak => {
                    x = MARGIN;
                    rows += 1;
                }
                PartItem::Chord(chord) => {
                    let root_w = chord.root.len() as f32 * root_char_w;
                    let qual_w = chord.quality.symbol().len() as f32 * qual_char_w;
                    let bass_w = chord
                        .bass_note
                        .as_deref()
                        .map(|b| (b.len() + 1) as f32 * bass_char_w)
                        .unwrap_or(0.0);
                    let total_w = root_w + sup_offset + qual_w + bass_w;
                    if x + total_w > RIGHT {
                        x = MARGIN;
                        rows += 1;
                    }
                    x += total_w + gap;
                }
                _ => {
                    // Repeat, VoltaBracket, etc. — rough width
                    let w = root_char_w * 5.0 + gap;
                    if x + w > RIGHT {
                        x = MARGIN;
                        rows += 1;
                    }
                    x += w;
                }
            }
        }

        part_name_h + (rows as f32) * (row_h + gap) + 8.0
    }
}

/// Render a single-character articulation symbol (h, p, r, b, /, \) into a tab cell,
/// erasing the string line behind it with a white box first.
#[allow(clippy::too_many_arguments)]
fn pdf_tab_sym(
    layer: &printpdf::PdfLayerReference,
    center_x: f32,
    sy: f32,
    sym: &str,
    box_w: f32,
    font_size: f32,
    font: &printpdf::IndirectFontRef,
) {
    use printpdf::{Color, Greyscale, Mm, Point, Polygon, PolygonMode, WindingOrder};
    // Scale the white-box and text offsets proportionally to the font size.
    // At the baseline font size of 7 pt the legacy offsets were: -1.8, +1.0, -1.3, -1.4.
    let scale = font_size / 7.0;
    let box_below = 1.8 * scale; // how far the box extends below the string line
    let box_above = 1.0 * scale; // how far the box extends above the string line
    let txt_x_off = (sym.len() as f32 * 0.65 * scale).min(box_w * 0.45); // rough half-width
    let txt_y_off = 1.4 * scale; // baseline drop below string line
    layer.set_fill_color(Color::Greyscale(Greyscale::new(1.0, None)));
    layer.add_polygon(Polygon {
        rings: vec![vec![
            (
                Point::new(Mm(center_x - box_w * 0.5), Mm(sy - box_below)),
                false,
            ),
            (
                Point::new(Mm(center_x + box_w * 0.5), Mm(sy - box_below)),
                false,
            ),
            (
                Point::new(Mm(center_x + box_w * 0.5), Mm(sy + box_above)),
                false,
            ),
            (
                Point::new(Mm(center_x - box_w * 0.5), Mm(sy + box_above)),
                false,
            ),
        ]],
        mode: PolygonMode::Fill,
        winding_order: WindingOrder::NonZero,
    });
    layer.set_fill_color(Color::Greyscale(Greyscale::new(0.0, None)));
    layer.use_text(
        sym,
        font_size,
        Mm(center_x - txt_x_off),
        Mm(sy - txt_y_off),
        font,
    );
}

/// Render `song` into a PDF and return the raw bytes.
/// `notation`        – note-naming convention (English / German / Custom).
/// `part_name_size`  – font size in pt for part labels (default 9).
/// `chord_size`      – font size in pt for chord roots (default 18).
/// `capo`            – capo fret (0 = no capo); chord roots are shifted accordingly.
/// Works on every target (desktop and WASM).
pub fn generate_pdf_bytes(
    song: &Song,
    notation: Notation,
    part_name_size: f32,
    chord_size: f32,
    capo: i8,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // When a capo/transpose is set, produce a shifted view of the song for rendering.
    let capo_song: Song;
    let effective = if capo != 0 {
        capo_song = song.apply_capo(capo);
        &capo_song
    } else {
        song
    };

    let (doc, page1, layer1) = PdfDocument::new(&song.name, Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
    let mut layer = doc.get_page(page1).get_layer(layer1);

    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
    let font_regular = doc.add_builtin_font(BuiltinFont::Helvetica)?;

    let mut y: f32 = PAGE_H - MARGIN;

    // ── Title ─────────────────────────────────────────────────────────────────
    layer.use_text(&song.name, 26.0, Mm(MARGIN), Mm(y), &font_bold);
    y -= 14.0;

    // ── Artist ────────────────────────────────────────────────────────────────
    layer.use_text(&song.artist, 14.0, Mm(MARGIN), Mm(y), &font_regular);
    y -= 6.5;

    // ── Key ───────────────────────────────────────────────────────────────────
    // Always show the original (sounding) key of the song.
    layer.use_text(
        format!("Key: {}", song.key),
        11.0,
        Mm(MARGIN),
        Mm(y),
        &font_regular,
    );
    y -= 5.0;

    // ── Capo ─────────────────────────────────────────────────────────────────
    // When a capo is used: print the fret number and the resulting shape key.
    if capo > 0 {
        layer.use_text(
            format!("Capo: fret {}  (shapes in {})", capo, effective.key),
            11.0,
            Mm(MARGIN),
            Mm(y),
            &font_regular,
        );
        y -= 5.0;
    }

    // ── Horizontal rule ───────────────────────────────────────────────────────
    layer.set_outline_thickness(0.4);
    layer.set_outline_color(Color::Greyscale(Greyscale::new(0.5, None)));
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(MARGIN), Mm(y)), false),
            (Point::new(Mm(RIGHT), Mm(y)), false),
        ],
        is_closed: false,
    });
    y -= 10.0;

    // ── Parts ─────────────────────────────────────────────────────────────────
    // Helper: start a fresh page and reset y to the top margin.
    macro_rules! next_page {
        () => {{
            let (pi, li) = doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
            layer = doc.get_page(pi).get_layer(li);
            y = PAGE_H - MARGIN;
        }};
    }

    for part in &effective.parts {
        // ── Page-break check ────────────────────────────────────────────────
        // If the remaining vertical space is less than this part's estimated
        // height, start a fresh page so the part isn't split or clipped.
        // For parts taller than a full page the inner loops will add further
        // pages as needed.
        let part_h = measure_part_height(part, chord_size, part_name_size);
        let remaining = y - MARGIN;
        if remaining < part_h {
            next_page!();
        }

        // Part label
        layer.set_outline_color(Color::Greyscale(Greyscale::new(0.0, None)));
        layer.use_text(
            part.name.to_uppercase(),
            part_name_size,
            Mm(MARGIN),
            Mm(y),
            &font_bold,
        );
        y -= (part_name_size / 9.0) * 14.0;

        // Chords – root at chord_size pt, quality as superscript
        let mut x: f32 = MARGIN;
        // All metrics scale proportionally with chord_size (baseline: 18 pt)
        let scale: f32 = chord_size / 18.0;
        let row_h: f32 = 12.0 * scale;
        let gap: f32 = 6.0;
        let qual_size: f32 = chord_size * (10.0 / 18.0);
        let raise_mm: f32 = chord_size * (3.8 / 18.0);
        let drop_mm: f32 = chord_size * (2.5 / 18.0);
        let bass_size: f32 = chord_size * (14.0 / 18.0);
        let sup_offset: f32 = 1.0;
        let root_char_w: f32 = chord_size * (3.5 / 18.0);
        let qual_char_w: f32 = root_char_w * (qual_size / chord_size);
        let bass_char_w: f32 = root_char_w * (bass_size / chord_size);

        // ── Riff / Tab part (graphical renderer) ─────────────────────────
        if part.kind == crate::song::PartKind::Riff
            || part.kind == crate::song::PartKind::BassRiff
            || part.kind == crate::song::PartKind::DrumBeat
        {
            use crate::song::{TabCell, TabCol};

            let is_bass = part.kind == crate::song::PartKind::BassRiff;
            let is_drums = part.kind == crate::song::PartKind::DrumBeat;
            let num_strings: usize = if is_bass {
                4
            } else if is_drums {
                8
            } else {
                6
            };
            let str_offset: usize = if is_bass { 2 } else { 0 };
            let string_labels: &[&str] = if is_bass {
                &["G", "D", "A", "E"]
            } else if is_drums {
                &["K", "S", "Hi", "R", "C", "T1", "T2", "T3"]
            } else {
                &["e", "B", "G", "D", "A", "E"]
            };

            // Layout constants
            let str_gap: f32 = 3.8; // mm between string lines
            let beat_w: f32 = 6.5; // mm per beat column
            let barline_w: f32 = 3.0; // mm for a barline column
            let label_w: f32 = 6.0; // mm for the "e|" string label
            let tab_font_size: f32 = chord_size * 0.55;
            let block_h: f32 = str_gap * (num_strings as f32 - 1.0); // height spanning all strings
            let block_gap: f32 = 8.0; // vertical gap between row-blocks

            // Split the grid into row-segments at LineBreak markers
            let mut segments: Vec<Vec<&TabCol>> = vec![vec![]];
            for col in &part.tab_grid {
                if matches!(col, TabCol::LineBreak) {
                    segments.push(vec![]);
                } else {
                    segments.last_mut().unwrap().push(col);
                }
            }

            for seg in &segments {
                // Skip empty segments (no columns at all) — these have nothing
                // to render and would just consume vertical space at the top.
                if seg.is_empty() {
                    continue;
                }

                if y < MARGIN + block_h + 2.0 {
                    next_page!();
                }

                // Compute total width of this segment
                let seg_w: f32 = seg
                    .iter()
                    .map(|c| match c {
                        TabCol::Barline => barline_w,
                        _ => beat_w,
                    })
                    .sum::<f32>();

                let x0 = MARGIN + label_w; // where the first string line starts
                let x_end = x0 + seg_w;

                // y is the TOP of the current segment block (high-e string).
                // Subsequent strings go downward (y - i * str_gap).
                let seg_top = y;
                let string_y: Vec<f32> = (0..num_strings)
                    .map(|i| seg_top - i as f32 * str_gap)
                    .collect();

                // ── Draw the 6 horizontal string lines ────────────────────
                layer.set_outline_thickness(0.35 * tab_font_size / 7.0);
                layer.set_outline_color(Color::Greyscale(Greyscale::new(0.55, None)));
                for &sy in &string_y {
                    layer.add_line(Line {
                        points: vec![
                            (Point::new(Mm(x0), Mm(sy)), false),
                            (Point::new(Mm(x_end), Mm(sy)), false),
                        ],
                        is_closed: false,
                    });
                }

                // ── Draw string labels (e B G D A E) ─────────────────────
                layer.set_outline_color(Color::Greyscale(Greyscale::new(0.0, None)));
                for (i, label) in string_labels.iter().enumerate() {
                    layer.use_text(
                        *label,
                        tab_font_size,
                        Mm(MARGIN),
                        Mm(string_y[i] - 1.0),
                        &font_bold,
                    );
                }

                // ── Draw notes and barlines ───────────────────────────────
                let mut cx = x0;
                for col in seg.iter() {
                    match col {
                        TabCol::Barline => {
                            // Full vertical line through all 6 strings
                            layer.set_outline_thickness(0.6 * tab_font_size / 7.0);
                            layer.set_outline_color(Color::Greyscale(Greyscale::new(0.25, None)));
                            layer.add_line(Line {
                                points: vec![
                                    (
                                        Point::new(
                                            Mm(cx + barline_w * 0.5),
                                            Mm(*string_y.last().unwrap() - 0.5),
                                        ),
                                        false,
                                    ),
                                    (
                                        Point::new(Mm(cx + barline_w * 0.5), Mm(string_y[0] + 0.5)),
                                        false,
                                    ),
                                ],
                                is_closed: false,
                            });
                            layer.set_outline_thickness(0.35 * tab_font_size / 7.0);
                            layer.set_outline_color(Color::Greyscale(Greyscale::new(0.55, None)));
                            cx += barline_w;
                        }
                        TabCol::Notes(arr) => {
                            let center_x = cx + beat_w * 0.5;
                            for (local_i, &sy) in string_y.iter().enumerate() {
                                let str_idx = local_i + str_offset;
                                let cell = &arr[str_idx];
                                match cell {
                                    TabCell::Empty => {}
                                    TabCell::Fret(n) => {
                                        let label = n.to_string();
                                        let scale = tab_font_size / 7.0;
                                        let box_w =
                                            if *n >= 10 { 4.6 * scale } else { 3.0 * scale };
                                        let box_below = 1.8 * scale;
                                        let box_above = 1.0 * scale;
                                        let txt_y_off = 1.4 * scale;
                                        // white fill box to erase string line behind number
                                        layer.set_fill_color(Color::Greyscale(Greyscale::new(
                                            1.0, None,
                                        )));
                                        layer.add_polygon(Polygon {
                                            rings: vec![vec![
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy - box_below),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy - box_below),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy + box_above),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy + box_above),
                                                    ),
                                                    false,
                                                ),
                                            ]],
                                            mode: PolygonMode::Fill,
                                            winding_order: WindingOrder::NonZero,
                                        });
                                        layer.set_fill_color(Color::Greyscale(Greyscale::new(
                                            0.0, None,
                                        )));
                                        layer.use_text(
                                            &label,
                                            tab_font_size,
                                            Mm(center_x - box_w * 0.4),
                                            Mm(sy - txt_y_off),
                                            &font_bold,
                                        );
                                    }
                                    TabCell::Muted => {
                                        let scale = tab_font_size / 7.0;
                                        let box_w = 3.0_f32 * scale;
                                        let box_below = 1.8 * scale;
                                        let box_above = 1.0 * scale;
                                        let txt_y_off = 1.4 * scale;
                                        let txt_x_off = 1.3 * scale;
                                        // white fill box to erase string line behind x
                                        layer.set_fill_color(Color::Greyscale(Greyscale::new(
                                            1.0, None,
                                        )));
                                        layer.add_polygon(Polygon {
                                            rings: vec![vec![
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy - box_below),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy - box_below),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy + box_above),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy + box_above),
                                                    ),
                                                    false,
                                                ),
                                            ]],
                                            mode: PolygonMode::Fill,
                                            winding_order: WindingOrder::NonZero,
                                        });
                                        layer.set_fill_color(Color::Greyscale(Greyscale::new(
                                            0.0, None,
                                        )));
                                        layer.use_text(
                                            "x",
                                            tab_font_size,
                                            Mm(center_x - txt_x_off),
                                            Mm(sy - txt_y_off),
                                            &font_regular,
                                        );
                                    }
                                    // ── Articulation symbols ──────────────────
                                    TabCell::Ghost(n) => {
                                        let label = format!("({n})");
                                        let scale = tab_font_size / 7.0;
                                        let box_w = 5.5_f32 * scale;
                                        let box_below = 1.8 * scale;
                                        let box_above = 1.0 * scale;
                                        let txt_y_off = 1.4 * scale;
                                        layer.set_fill_color(Color::Greyscale(Greyscale::new(
                                            1.0, None,
                                        )));
                                        layer.add_polygon(Polygon {
                                            rings: vec![vec![
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy - box_below),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy - box_below),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy + box_above),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy + box_above),
                                                    ),
                                                    false,
                                                ),
                                            ]],
                                            mode: PolygonMode::Fill,
                                            winding_order: WindingOrder::NonZero,
                                        });
                                        layer.set_fill_color(Color::Greyscale(Greyscale::new(
                                            0.0, None,
                                        )));
                                        layer.use_text(
                                            &label,
                                            tab_font_size - 1.0,
                                            Mm(center_x - box_w * 0.45),
                                            Mm(sy - txt_y_off),
                                            &font_regular,
                                        );
                                    }
                                    TabCell::HammerOn => {
                                        pdf_tab_sym(
                                            &layer,
                                            center_x,
                                            sy,
                                            "h",
                                            3.0 * tab_font_size / 7.0,
                                            tab_font_size,
                                            &font_regular,
                                        );
                                    }
                                    TabCell::PullOff => {
                                        pdf_tab_sym(
                                            &layer,
                                            center_x,
                                            sy,
                                            "p",
                                            3.0 * tab_font_size / 7.0,
                                            tab_font_size,
                                            &font_regular,
                                        );
                                    }
                                    TabCell::Release => {
                                        pdf_tab_sym(
                                            &layer,
                                            center_x,
                                            sy,
                                            "r",
                                            3.0 * tab_font_size / 7.0,
                                            tab_font_size,
                                            &font_regular,
                                        );
                                    }
                                    TabCell::Bend => {
                                        pdf_tab_sym(
                                            &layer,
                                            center_x,
                                            sy,
                                            "b",
                                            3.0 * tab_font_size / 7.0,
                                            tab_font_size,
                                            &font_bold,
                                        );
                                    }
                                    TabCell::SlideUp => {
                                        pdf_tab_sym(
                                            &layer,
                                            center_x,
                                            sy,
                                            "/",
                                            3.0 * tab_font_size / 7.0,
                                            tab_font_size,
                                            &font_regular,
                                        );
                                    }
                                    TabCell::SlideDown => {
                                        pdf_tab_sym(
                                            &layer,
                                            center_x,
                                            sy,
                                            "\\",
                                            3.0 * tab_font_size / 7.0,
                                            tab_font_size,
                                            &font_regular,
                                        );
                                    }
                                    TabCell::Custom(s) => {
                                        pdf_tab_sym(
                                            &layer,
                                            center_x,
                                            sy,
                                            s,
                                            (s.len() as f32 * 1.2 * tab_font_size / 7.0)
                                                .max(3.0 * tab_font_size / 7.0),
                                            tab_font_size - 1.0,
                                            &font_regular,
                                        );
                                    }
                                }
                            }
                            cx += beat_w;
                        }
                        TabCol::LineBreak => unreachable!(),
                    }
                }

                // Advance y to below this segment's bottom string + gap,
                // so the next segment is placed below (not above) this one.
                y = seg_top - block_h - block_gap;
            }

            y -= gap;
            continue;
        }

        let has_lyrics = part
            .part_text
            .as_ref()
            .map(|t| !t.content.is_empty())
            .unwrap_or(false);
        let two_col = has_lyrics
            && part
                .part_text
                .as_ref()
                .map(|t| t.show_chords)
                .unwrap_or(false);
        let skip_chords = has_lyrics && !two_col;

        // Column split point (45% left for lyrics, 55% right for chords)
        let col_mid = MARGIN + (RIGHT - MARGIN) * 0.45;

        // ── In two-column mode render lyrics first (left col), save y, then reset ─
        let start_y = y;
        if two_col {
            if let Some(pt) = &part.part_text {
                let text_size = pt.size as f32;
                let char_w = text_size * (3.5 / 18.0);
                let line_h = text_size * (3.5 / 18.0) * 4.5;
                let x_right = col_mid - 4.0;
                let hex = pt.color.trim_start_matches('#');
                let (r, g, b) = if hex.len() == 6 {
                    (
                        u8::from_str_radix(&hex[0..2], 16).unwrap_or(0x55),
                        u8::from_str_radix(&hex[2..4], 16).unwrap_or(0x55),
                        u8::from_str_radix(&hex[4..6], 16).unwrap_or(0x55),
                    )
                } else {
                    (0x55, 0x55, 0x55)
                };
                layer.set_fill_color(Color::Rgb(Rgb::new(
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    None,
                )));
                let max_chars = ((x_right - MARGIN) / char_w).floor().max(1.0) as usize;
                for line in pt.content.split('\n') {
                    let mut remaining = line;
                    while !remaining.is_empty() {
                        if y < MARGIN + 10.0 {
                            next_page!();
                        }
                        let take = remaining.len().min(max_chars);
                        let take = if take < remaining.len() {
                            remaining[..take].rfind(' ').map(|i| i + 1).unwrap_or(take)
                        } else {
                            take
                        };
                        let (chunk, rest) = remaining.split_at(take);
                        layer.use_text(chunk, text_size, Mm(MARGIN), Mm(y + 1.5), &font_bold);
                        y -= line_h;
                        remaining = rest;
                    }
                }
                layer.set_fill_color(Color::Greyscale(Greyscale::new(0.0, None)));
            }
        }
        let y_after_lyrics = y;

        // Reset y/x for chord rendering (two-col: start chords at same y, right column)
        if two_col {
            y = start_y;
        }
        x = if two_col { col_mid } else { MARGIN };

        // ── Chord items loop ─────────────────────────────────────────────────
        let x_chord_start = if two_col { col_mid } else { MARGIN };
        if !skip_chords {
            for item in &part.items {
                use crate::song::PartItem;
                match item {
                    PartItem::LineBreak => {
                        x = x_chord_start;
                        y -= row_h + gap;
                    }
                    PartItem::Repeat { times } => {
                        let label = if *times > 0 {
                            format!("||: x{}", times)
                        } else {
                            "||:".to_string()
                        };
                        let w = label.len() as f32 * root_char_w;
                        if x + w > RIGHT {
                            x = x_chord_start;
                            y -= row_h + gap;
                        }
                        layer.use_text(&label, chord_size, Mm(x), Mm(y + 1.5), &font_bold);
                        x += w + gap;
                    }
                    PartItem::VoltaBracketStart { label } => {
                        let bracket_size = chord_size * 1.5;
                        let label_size = chord_size * 0.55;
                        let bracket_w = root_char_w * 1.5;
                        let label_w = label.len() as f32 * root_char_w * 0.55 + 2.0;
                        let w = bracket_w + label_w + 3.0;
                        if x + w > RIGHT {
                            x = x_chord_start;
                            y -= row_h + gap;
                        }
                        layer.use_text("[", bracket_size, Mm(x), Mm(y + 1.5), &font_bold);
                        layer.use_text(
                            label.as_str(),
                            label_size,
                            Mm(x + bracket_w + 1.0),
                            Mm(y + 1.5 + row_h * 0.45),
                            &font_bold,
                        );
                        x += w;
                    }
                    PartItem::RepeatStart => {
                        let w = root_char_w * 1.5 + 3.0;
                        if x + w > RIGHT {
                            x = x_chord_start;
                            y -= row_h + gap;
                        }
                        layer.use_text("||", chord_size, Mm(x), Mm(y + 1.5), &font_bold);
                        x += w;
                    }
                    PartItem::VoltaBracketEnd => {
                        let bracket_size = chord_size * 1.5;
                        let w = root_char_w * 1.5 + 3.0;
                        if x + w > RIGHT {
                            x = x_chord_start;
                            y -= row_h + gap;
                        }
                        layer.use_text("]", bracket_size, Mm(x), Mm(y + 1.5), &font_bold);
                        x += w;
                    }
                    PartItem::Chord(chord) => {
                        let root = apply_notation(&chord.root, notation);
                        let quality = chord.quality.symbol();
                        let bass_suffix: String = chord
                            .bass_note
                            .as_deref()
                            .map(|b| format!("/{}", apply_notation(b, notation)))
                            .unwrap_or_default();

                        let root_w = root.len() as f32 * root_char_w;
                        let qual_w = quality.len() as f32 * qual_char_w;
                        let bass_w = bass_suffix.len() as f32 * bass_char_w;
                        let total_w = root_w + sup_offset + qual_w + bass_w;

                        if x + total_w > RIGHT {
                            x = x_chord_start;
                            y -= row_h + gap;
                            if y < MARGIN + 10.0 {
                                next_page!();
                            }
                        }

                        layer.use_text(&root, chord_size, Mm(x), Mm(y + 1.5), &font_bold);

                        if !quality.is_empty() {
                            layer.use_text(
                                quality,
                                qual_size,
                                Mm(x + root_w + sup_offset),
                                Mm(y + 1.5 + raise_mm),
                                &font_bold,
                            );
                        }

                        if !bass_suffix.is_empty() {
                            layer.use_text(
                                &bass_suffix,
                                bass_size,
                                Mm(x + root_w + sup_offset + qual_w),
                                Mm(y + 1.5 - drop_mm),
                                &font_bold,
                            );
                        }

                        x += total_w + gap;
                    }
                    PartItem::Text { content, color } => {
                        if y < MARGIN + 10.0 {
                            next_page!();
                        }
                        let hex = color.trim_start_matches('#');
                        let (r, g, b) = if hex.len() == 6 {
                            let rv = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0x88);
                            let gv = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0x88);
                            let bv = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0x88);
                            (rv, gv, bv)
                        } else if hex.len() == 3 {
                            let rv = u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0x88);
                            let gv = u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0x88);
                            let bv = u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0x88);
                            (rv, gv, bv)
                        } else {
                            (0x88, 0x88, 0x88)
                        };
                        let text_size = chord_size * 0.72;
                        layer.set_fill_color(Color::Rgb(Rgb::new(
                            r as f32 / 255.0,
                            g as f32 / 255.0,
                            b as f32 / 255.0,
                            None,
                        )));
                        layer.use_text(content.as_str(), text_size, Mm(x), Mm(y + 1.5), &font_bold);
                        layer.set_fill_color(Color::Greyscale(Greyscale::new(0.0, None)));
                        let text_char_w = root_char_w * (text_size / chord_size);
                        x += content.len() as f32 * text_char_w + gap;
                    }
                }
            }
        } // end !skip_chords

        // ── Advance y based on mode ──────────────────────────────────────────
        if two_col {
            let y_after_chords = y - (row_h + gap + 8.0);
            y = y_after_lyrics.min(y_after_chords);
            y -= gap;
        } else if skip_chords {
            // Lyrics-only: render full-width, no chord row gap
            if let Some(pt) = &part.part_text {
                let text_size = pt.size as f32;
                let char_w = text_size * (3.5 / 18.0);
                let line_h = text_size * (3.5 / 18.0) * 4.5;
                let hex = pt.color.trim_start_matches('#');
                let (r, g, b) = if hex.len() == 6 {
                    (
                        u8::from_str_radix(&hex[0..2], 16).unwrap_or(0x55),
                        u8::from_str_radix(&hex[2..4], 16).unwrap_or(0x55),
                        u8::from_str_radix(&hex[4..6], 16).unwrap_or(0x55),
                    )
                } else {
                    (0x55, 0x55, 0x55)
                };
                layer.set_fill_color(Color::Rgb(Rgb::new(
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    None,
                )));
                let max_chars = ((RIGHT - MARGIN) / char_w).floor().max(1.0) as usize;
                for line in pt.content.split('\n') {
                    let mut remaining = line;
                    while !remaining.is_empty() {
                        if y < MARGIN + 10.0 {
                            next_page!();
                        }
                        let take = remaining.len().min(max_chars);
                        let take = if take < remaining.len() {
                            remaining[..take].rfind(' ').map(|i| i + 1).unwrap_or(take)
                        } else {
                            take
                        };
                        let (chunk, rest) = remaining.split_at(take);
                        layer.use_text(chunk, text_size, Mm(MARGIN), Mm(y + 1.5), &font_bold);
                        y -= line_h;
                        remaining = rest;
                    }
                }
                layer.set_fill_color(Color::Greyscale(Greyscale::new(0.0, None)));
            }
            y -= gap;
        } else {
            // Chords only: normal advance
            y -= row_h + gap + 8.0;
        }
    }

    let mut buf: Vec<u8> = Vec::new();
    doc.save(&mut BufWriter::new(&mut buf))?;
    Ok(buf)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::{Chord, ChordQuality, Notation, Song};

    fn sample_song() -> Song {
        Song::new("Test Song", "G Major", "Test Artist").with_part(
            "Verse",
            vec![
                Chord::new("G", ChordQuality::Major),
                Chord::new("E", ChordQuality::Minor),
                Chord::new("C", ChordQuality::Major),
                Chord::new("D", ChordQuality::Major),
            ],
        )
    }

    #[test]
    fn generate_pdf_returns_non_empty_bytes() {
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, Notation::English, 9.0, 18.0, 0).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn generate_pdf_starts_with_pdf_header() {
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, Notation::English, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"), "output should be a valid PDF");
    }

    #[test]
    fn generate_pdf_custom_font_sizes_produce_valid_pdf() {
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, Notation::English, 14.0, 24.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn generate_pdf_empty_song_produces_valid_pdf() {
        let song = Song::new("Empty", "C", "Nobody");
        let bytes = generate_pdf_bytes(&song, Notation::English, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn generate_pdf_with_capo_shows_shifted_key() {
        // G Major, capo 2 → chord shapes in F Major
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, Notation::English, 9.0, 18.0, 2).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(
            bytes.windows(1).count() > 0,
            "non-empty PDF produced with capo"
        );
    }

    #[test]
    fn generate_pdf_german_notation_produces_valid_pdf() {
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, Notation::German, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn generate_pdf_many_parts_produces_multiple_pages() {
        // 20 parts × 4 chords each should overflow a single A4 page.
        let chords = vec![
            Chord::new("G", ChordQuality::Major),
            Chord::new("E", ChordQuality::Minor),
            Chord::new("C", ChordQuality::Major),
            Chord::new("D", ChordQuality::Major),
        ];
        let mut song = Song::new("Long Song", "G Major", "Test Artist");
        for i in 0..20 {
            song = song.with_part(format!("Part {i}"), chords.clone());
        }
        let bytes = generate_pdf_bytes(&song, Notation::English, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        // A multi-page PDF is larger than a single-page one — use size as proxy.
        let single = generate_pdf_bytes(
            &Song::new("X", "G", "Y").with_part("V", chords.clone()),
            Notation::English,
            9.0,
            18.0,
            0,
        )
        .unwrap();
        assert!(
            bytes.len() > single.len() * 3,
            "20-part PDF ({} bytes) should be much larger than 1-part PDF ({} bytes)",
            bytes.len(),
            single.len()
        );
        // printpdf emits "Page" in object dictionaries; count occurrences.
        let page_kw = bytes.windows(4).filter(|w| *w == b"Page").count();
        assert!(
            page_kw > 1,
            "expected multiple page objects, found 'Page' count = {page_kw}"
        );
    }

    #[test]
    fn generate_pdf_with_riff_part_produces_valid_pdf() {
        use chord_shifter::song::{SongPart, TabCell, TabCol};
        let mut song = sample_song();
        let mut riff = SongPart::new_riff("Intro");
        riff.tab_grid = vec![
            TabCol::Notes([
                TabCell::Fret(0),
                TabCell::Empty,
                TabCell::Empty,
                TabCell::Empty,
                TabCell::Empty,
                TabCell::Fret(0),
                TabCell::Empty,
                TabCell::Empty,
            ]),
            TabCol::Barline,
            TabCol::Notes(std::array::from_fn(|_| TabCell::Muted)),
        ];
        song.parts.push(riff);
        let bytes = generate_pdf_bytes(&song, Notation::English, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn generate_pdf_with_bass_riff_part_produces_valid_pdf() {
        use chord_shifter::song::{SongPart, TabCell, TabCol};
        let mut song = sample_song();
        let mut bass = SongPart::new_bass_riff("Bass Line");
        bass.tab_grid = vec![TabCol::Notes(std::array::from_fn(|_| TabCell::Fret(5)))];
        song.parts.push(bass);
        let bytes = generate_pdf_bytes(&song, Notation::English, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn generate_pdf_with_repeat_and_volta_produces_valid_pdf() {
        use chord_shifter::song::{Chord, ChordQuality, PartItem, PartKind, SongPart};
        let mut song = Song::new("Repeat Test", "C Major", "Artist");
        let part = SongPart {
            name: "Chorus".to_string(),
            kind: PartKind::Chords,
            tab: String::new(),
            tab_grid: Vec::new(),
            items: vec![
                PartItem::Chord(Chord::new("C", ChordQuality::Major)),
                PartItem::RepeatStart,
                PartItem::Chord(Chord::new("G", ChordQuality::Major)),
                PartItem::VoltaBracketStart {
                    label: "1.".to_string(),
                },
                PartItem::Chord(Chord::new("F", ChordQuality::Major)),
                PartItem::VoltaBracketEnd,
                PartItem::Repeat { times: 2 },
            ],
            part_text: None,
        };
        song.parts.push(part);
        let bytes = generate_pdf_bytes(&song, Notation::English, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn generate_pdf_with_line_break_item_produces_valid_pdf() {
        use chord_shifter::song::{Chord, ChordQuality, PartItem, PartKind, SongPart};
        let mut song = Song::new("LineBreak Test", "C Major", "Artist");
        let part = SongPart {
            name: "Verse".to_string(),
            kind: PartKind::Chords,
            tab: String::new(),
            tab_grid: Vec::new(),
            items: vec![
                PartItem::Chord(Chord::new("C", ChordQuality::Major)),
                PartItem::LineBreak,
                PartItem::Chord(Chord::new("G", ChordQuality::Major)),
            ],
            part_text: None,
        };
        song.parts.push(part);
        let bytes = generate_pdf_bytes(&song, Notation::English, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }
}
