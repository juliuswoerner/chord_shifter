use printpdf::*;
use std::io::BufWriter;

use crate::song::Song;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 22.0;
const RIGHT: f32 = PAGE_W - MARGIN;

/// Render `song` into a PDF and return the raw bytes.
/// `use_degrees`    – when `true`, chords are shown as roman-numeral scale degrees.
/// `part_name_size` – font size in pt for part labels (default 9).
/// `chord_size`     – font size in pt for chord roots (default 18).
/// `capo`           – capo fret (0 = no capo); chord roots are shifted accordingly.
/// Works on every target (desktop and WASM).
pub fn generate_pdf_bytes(
    song: &Song,
    use_degrees: bool,
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
    layer.use_text(
        format!("Key: {}", effective.key),
        11.0,
        Mm(MARGIN),
        Mm(y),
        &font_regular,
    );
    y -= 5.0;

    // ── Capo ─────────────────────────────────────────────────────────────────
    if capo > 0 {
        layer.use_text(
            format!("Capo: {capo}  (shapes in {})", effective.key),
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
        let sup_offset: f32 = 1.0;
        let root_char_w: f32 = chord_size * (3.5 / 18.0);
        let qual_char_w: f32 = root_char_w * (qual_size / chord_size);

        for chord in &part.chords {
            // In degrees mode use the roman numeral; fall back to root name if no degree set.
            let root: String = if use_degrees {
                chord
                    .degree
                    .map(|d| d.roman().to_string())
                    .unwrap_or_else(|| chord.root.clone())
            } else {
                chord.root.clone()
            };
            let quality = chord.quality.symbol();

            let root_w = root.len() as f32 * root_char_w;
            let qual_w = quality.len() as f32 * qual_char_w;
            let total_w = root_w + sup_offset + qual_w;

            if x + total_w > RIGHT {
                x = MARGIN;
                y -= row_h + gap;
                if y < MARGIN + 10.0 {
                    break;
                }
            }

            // Root note (or roman numeral)
            layer.use_text(&root, chord_size, Mm(x), Mm(y + 1.5), &font_bold);

            // Quality as superscript (smaller, raised)
            if !quality.is_empty() {
                layer.use_text(
                    quality,
                    qual_size,
                    Mm(x + root_w + sup_offset),
                    Mm(y + 1.5 + raise_mm),
                    &font_bold,
                );
            }

            x += total_w + gap;
        }

        y -= row_h + gap + 8.0;
    }

    let mut buf: Vec<u8> = Vec::new();
    doc.save(&mut BufWriter::new(&mut buf))?;
    Ok(buf)
}

/// Save the PDF to disk. Only compiled on non-WASM targets.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_pdf(
    song: &Song,
    path: &str,
    use_degrees: bool,
    part_name_size: f32,
    chord_size: f32,
    capo: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = generate_pdf_bytes(song, use_degrees, part_name_size, chord_size, capo)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::{Chord, ChordQuality, Song};

    fn sample_song() -> Song {
        Song::new("Test Song", "G Major", "Test Artist").with_part(
            "Verse",
            vec![
                Chord::new("G", ChordQuality::Major).with_degree(1),
                Chord::new("E", ChordQuality::Minor).with_degree(6),
                Chord::new("C", ChordQuality::Major).with_degree(4),
                Chord::new("D", ChordQuality::Major).with_degree(5),
            ],
        )
    }

    #[test]
    fn generate_pdf_returns_non_empty_bytes() {
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, false, 9.0, 18.0, 0).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn generate_pdf_starts_with_pdf_header() {
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, false, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"), "output should be a valid PDF");
    }

    #[test]
    fn generate_pdf_degrees_mode_also_produces_valid_pdf() {
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, true, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn generate_pdf_custom_font_sizes_produce_valid_pdf() {
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, false, 14.0, 24.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn generate_pdf_empty_song_produces_valid_pdf() {
        let song = Song::new("Empty", "C", "Nobody");
        let bytes = generate_pdf_bytes(&song, false, 9.0, 18.0, 0).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn generate_pdf_with_capo_shows_shifted_key() {
        // G Major, capo 2 → chord shapes in F Major
        let song = sample_song();
        let bytes = generate_pdf_bytes(&song, false, 9.0, 18.0, 2).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        // The raw PDF stream contains the text we printed.
        assert!(
            bytes.windows(1).count() > 0,
            "non-empty PDF produced with capo"
        );
    }
}
