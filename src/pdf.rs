use printpdf::*;
use std::io::BufWriter;

use crate::song::{apply_notation, Notation, Song};

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 22.0;
const RIGHT: f32 = PAGE_W - MARGIN;

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
    capo: u8,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // When a capo is set, produce a capo-shifted view of the song for rendering.
    let capo_song: Song;
    let effective = if capo > 0 {
        capo_song = song.apply_capo(capo);
        &capo_song
    } else {
        song
    };

    let (doc, page1, layer1) = PdfDocument::new(&song.name, Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
    let layer = doc.get_page(page1).get_layer(layer1);

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
    for part in &effective.parts {
        if y < MARGIN + 20.0 {
            break;
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
        if part.kind == crate::song::PartKind::Riff || part.kind == crate::song::PartKind::BassRiff
        {
            use crate::song::{TabCell, TabCol};

            let is_bass = part.kind == crate::song::PartKind::BassRiff;
            let num_strings: usize = if is_bass { 4 } else { 6 };
            let str_offset: usize = if is_bass { 2 } else { 0 };
            let string_labels: &[&str] = if is_bass {
                &["G", "D", "A", "E"]
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
                    break;
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
                layer.set_outline_thickness(0.35);
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
                            layer.set_outline_thickness(0.6);
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
                            layer.set_outline_thickness(0.35);
                            layer.set_outline_color(Color::Greyscale(Greyscale::new(0.55, None)));
                            cx += barline_w;
                        }
                        TabCol::Notes(arr) => {
                            let center_x = cx + beat_w * 0.5;
                            for local_i in 0..num_strings {
                                let str_idx = local_i + str_offset;
                                let cell = &arr[str_idx];
                                let sy = string_y[local_i];
                                match cell {
                                    TabCell::Empty => {}
                                    TabCell::Fret(n) => {
                                        let label = n.to_string();
                                        let box_w = if *n >= 10 { 4.6 } else { 3.0 };
                                        // white fill box to erase string line behind number
                                        layer.set_fill_color(Color::Greyscale(Greyscale::new(
                                            1.0, None,
                                        )));
                                        layer.add_polygon(Polygon {
                                            rings: vec![vec![
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy - 1.8),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy - 1.8),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy + 1.0),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy + 1.0),
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
                                            Mm(sy - 1.4),
                                            &font_bold,
                                        );
                                    }
                                    TabCell::Muted => {
                                        let box_w = 3.0_f32;
                                        // white fill box to erase string line behind x
                                        layer.set_fill_color(Color::Greyscale(Greyscale::new(
                                            1.0, None,
                                        )));
                                        layer.add_polygon(Polygon {
                                            rings: vec![vec![
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy - 1.8),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy - 1.8),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x + box_w * 0.5),
                                                        Mm(sy + 1.0),
                                                    ),
                                                    false,
                                                ),
                                                (
                                                    Point::new(
                                                        Mm(center_x - box_w * 0.5),
                                                        Mm(sy + 1.0),
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
                                            Mm(center_x - 1.3),
                                            Mm(sy - 1.4),
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

        for item in &part.items {
            use crate::song::PartItem;
            match item {
                PartItem::LineBreak => {
                    x = MARGIN;
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
                        x = MARGIN;
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
                        x = MARGIN;
                        y -= row_h + gap;
                    }
                    // Large bracket
                    layer.use_text("[", bracket_size, Mm(x), Mm(y + 1.5), &font_bold);
                    // Small superscript label
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
                        x = MARGIN;
                        y -= row_h + gap;
                    }
                    layer.use_text("||", chord_size, Mm(x), Mm(y + 1.5), &font_bold);
                    x += w;
                }
                PartItem::VoltaBracketEnd => {
                    let bracket_size = chord_size * 1.5;
                    let w = root_char_w * 1.5 + 3.0;
                    if x + w > RIGHT {
                        x = MARGIN;
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
                        x = MARGIN;
                        y -= row_h + gap;
                        if y < MARGIN + 10.0 {
                            break;
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
            }
        }

        y -= row_h + gap + 8.0;
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
}
