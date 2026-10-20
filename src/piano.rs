// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.

//! Piano sheet music: notes, key signatures, note lengths, transposition and the
//! layout of a grand staff as drawing primitives.
//!
//! A piano part stores its notes in the shared tab grid: cell 0 is the right hand,
//! cell 1 the left hand (comma-separated note names such as `"C4, E4, G4"`), cell 2
//! the note length (`"1"`, `"2"`, `"4"` or `"8"`). Everything here is pure, so it is
//! unit-tested natively; the PDF renderer draws the [`Prim`] list produced by
//! [`layout_systems`].

use crate::song::{PartKind, SongPart, TabCell, TabCol};

// ── Notes ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accidental {
    Natural,
    Sharp,
    Flat,
}

/// A written note: letter (0 = C … 6 = B), accidental and octave (middle C = C4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note {
    pub letter: u8,
    pub accidental: Accidental,
    pub octave: i32,
}

const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
const LETTER_SEMITONES: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const SHARP_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const FLAT_NAMES: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

impl Note {
    /// Parse `"C4"`, `"F#5"`, `"Bb3"` (letter case ignored, octave -1…9).
    pub fn parse(text: &str) -> Option<Note> {
        let t = text.trim();
        let mut chars = t.chars();
        let letter_char = chars.next()?.to_ascii_uppercase();
        let letter = LETTERS.iter().position(|&l| l == letter_char)? as u8;
        let rest = chars.as_str();
        let (accidental, octave_text) = match rest.chars().next() {
            Some('#') => (Accidental::Sharp, &rest[1..]),
            Some('b') => (Accidental::Flat, &rest[1..]),
            _ => (Accidental::Natural, rest),
        };
        let octave: i32 = octave_text.trim().parse().ok()?;
        if !(-1..=9).contains(&octave) {
            return None;
        }
        Some(Note {
            letter,
            accidental,
            octave,
        })
    }

    /// Absolute diatonic position (C4 = 28).
    pub fn diatonic(&self) -> i32 {
        self.octave * 7 + self.letter as i32
    }

    /// MIDI pitch (middle C = 60).
    pub fn midi(&self) -> i32 {
        let acc = match self.accidental {
            Accidental::Natural => 0,
            Accidental::Sharp => 1,
            Accidental::Flat => -1,
        };
        (self.octave + 1) * 12 + LETTER_SEMITONES[self.letter as usize] + acc
    }

    /// The name as typed in the editor, e.g. `"F#5"`.
    pub fn name(&self) -> String {
        let acc = match self.accidental {
            Accidental::Natural => "",
            Accidental::Sharp => "#",
            Accidental::Flat => "b",
        };
        format!("{}{}{}", LETTERS[self.letter as usize], acc, self.octave)
    }

    /// The note `semitones` away, spelled with sharps or flats.
    pub fn transposed(&self, semitones: i32, prefer_sharps: bool) -> Note {
        let midi = self.midi() + semitones;
        let pc = midi.rem_euclid(12) as usize;
        let name = if prefer_sharps {
            SHARP_NAMES[pc]
        } else {
            FLAT_NAMES[pc]
        };
        let octave = midi.div_euclid(12) - 1;
        Note::parse(&format!("{name}{octave}")).unwrap_or(*self)
    }
}

/// Parse a comma-separated chord (`"C4, E4, G4"`). Entries that are not notes are skipped.
pub fn parse_chord(text: &str) -> Vec<Note> {
    text.split(',').filter_map(Note::parse).collect()
}

// ── Staff positions ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clef {
    Treble,
    Bass,
}

impl Clef {
    /// Diatonic position of the bottom staff line: E4 (treble) or G2 (bass).
    fn bottom_line(self) -> i32 {
        match self {
            Clef::Treble => 30,
            Clef::Bass => 18,
        }
    }
}

/// Steps above the bottom staff line (0 = bottom line, 1 = first space, 2 = second line …).
pub fn staff_step(note: &Note, clef: Clef) -> i32 {
    note.diatonic() - clef.bottom_line()
}

/// Steps (even numbers) at which ledger lines are needed for notes at these steps.
/// Below the staff: -2, -4 …; above it: 10, 12 …
pub fn ledger_steps(steps: &[i32]) -> Vec<i32> {
    let mut out: Vec<i32> = Vec::new();
    for &s in steps {
        let mut e = -2;
        while e >= s {
            if !out.contains(&e) {
                out.push(e);
            }
            e -= 2;
        }
        let mut e = 10;
        while e <= s {
            if !out.contains(&e) {
                out.push(e);
            }
            e += 2;
        }
    }
    out.sort_unstable();
    out
}

// ── Key signatures ────────────────────────────────────────────────────────────

/// One accidental of a key signature, with its staff step on each clef.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyAccidental {
    pub sharp: bool,
    pub treble_step: i32,
    pub bass_step: i32,
}

const SHARP_POSITIONS: [(i32, i32); 7] = [(8, 6), (5, 3), (9, 7), (6, 4), (10, 8), (7, 5), (4, 2)];
const FLAT_POSITIONS: [(i32, i32); 7] = [(4, 2), (7, 5), (3, 1), (6, 4), (2, 0), (5, 3), (1, -1)];

/// The accidentals of a key such as `"G Major"`, `"Bb Minor"` or `"F#m"` (empty for C / A minor).
pub fn key_signature(key: &str) -> Vec<KeyAccidental> {
    let key = key.trim();
    let mut parts = key.splitn(2, ' ');
    let root = parts.next().unwrap_or("").trim_end_matches('m');
    let mode_word = parts.next().unwrap_or("").to_ascii_lowercase();
    let is_minor = mode_word.starts_with("min") || key.ends_with('m');

    let (count, sharps) = if is_minor {
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
    let table = if sharps {
        &SHARP_POSITIONS
    } else {
        &FLAT_POSITIONS
    };
    table[..count]
        .iter()
        .map(|&(t, b)| KeyAccidental {
            sharp: sharps,
            treble_step: t,
            bass_step: b,
        })
        .collect()
}

// ── Time signature and note lengths ───────────────────────────────────────────

/// Split `"3/4"` into `("3", "4")`; anything unusable becomes `("4", "4")`.
pub fn split_time_sig(text: &str) -> (String, String) {
    let mut it = text.trim().splitn(2, '/');
    let top = it.next().unwrap_or("").trim();
    let bottom = it.next().unwrap_or("").trim();
    let ok = |s: &str| !s.is_empty() && s.len() <= 2 && s.chars().all(|c| c.is_ascii_digit());
    if ok(top) && ok(bottom) {
        (top.to_string(), bottom.to_string())
    } else {
        ("4".to_string(), "4".to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duration {
    Whole,
    Half,
    Quarter,
    Eighth,
}

impl Duration {
    /// Parse the stored value (`"1"`, `"2"`, `"4"`, `"8"`); anything else is a quarter note.
    pub fn parse(text: &str) -> Duration {
        match text.trim() {
            "1" => Duration::Whole,
            "2" => Duration::Half,
            "8" => Duration::Eighth,
            _ => Duration::Quarter,
        }
    }
    /// Hollow note heads: whole and half notes.
    pub fn hollow(self) -> bool {
        matches!(self, Duration::Whole | Duration::Half)
    }
    pub fn has_stem(self) -> bool {
        self != Duration::Whole
    }
    pub fn has_flag(self) -> bool {
        self == Duration::Eighth
    }
}

// ── Columns of a piano part ───────────────────────────────────────────────────

/// One column of a piano part's grid.
#[derive(Debug, Clone, PartialEq)]
pub enum Column {
    Barline,
    Notes {
        right: Vec<Note>,
        left: Vec<Note>,
        duration: Duration,
    },
}

/// Read the grid of a piano part into columns, splitting into lines at `LineBreak`s.
pub fn columns_by_line(part: &SongPart) -> Vec<Vec<Column>> {
    let mut lines: Vec<Vec<Column>> = vec![Vec::new()];
    let cell_text = |cells: &[TabCell; 8], i: usize| match &cells[i] {
        TabCell::Custom(s) => s.clone(),
        _ => String::new(),
    };
    for col in &part.tab_grid {
        match col {
            TabCol::LineBreak => lines.push(Vec::new()),
            TabCol::Barline => lines.last_mut().unwrap().push(Column::Barline),
            TabCol::Notes(cells) => lines.last_mut().unwrap().push(Column::Notes {
                right: parse_chord(&cell_text(cells, 0)),
                left: parse_chord(&cell_text(cells, 1)),
                duration: Duration::parse(&cell_text(cells, 2)),
            }),
        }
    }
    lines
}

/// Shift every note of a piano part by `semitones` (spelled with sharps or flats).
/// Other parts are left alone. Entries that are not valid notes are kept unchanged.
pub fn transpose_part(part: &mut SongPart, semitones: i32, prefer_sharps: bool) {
    if part.kind != PartKind::PianoRiff || semitones == 0 {
        return;
    }
    for col in &mut part.tab_grid {
        if let TabCol::Notes(cells) = col {
            for cell in cells.iter_mut().take(2) {
                if let TabCell::Custom(text) = cell {
                    let moved: Vec<String> = text
                        .split(',')
                        .map(|entry| match Note::parse(entry) {
                            Some(n) => n.transposed(semitones, prefer_sharps).name(),
                            None => entry.trim().to_string(),
                        })
                        .collect();
                    *text = moved.join(", ");
                }
            }
        }
    }
}

// ── Layout of the grand staff ─────────────────────────────────────────────────
//
// Coordinates are in "staff spaces" (the distance between two staff lines = 1.0),
// x to the right and y downwards from the top of the system. The PDF renderer scales
// them to millimetres.

/// A drawing primitive.
#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
    },
    /// An open polyline.
    Path { points: Vec<(f32, f32)>, width: f32 },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        tilt_deg: f32,
        filled: bool,
        width: f32,
    },
    /// Text with its baseline starting at (x, y); `size` is the font size in staff spaces.
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        bold: bool,
    },
}

/// One line of music: a treble and a bass staff joined by a system line.
#[derive(Debug, Clone, PartialEq)]
pub struct System {
    pub width: f32,
    pub height: f32,
    pub prims: Vec<Prim>,
}

const LEDGER_PAD: f32 = 2.25; // room for ledger lines above and below a staff
const STAFF_GAP: f32 = 3.0; // between the two staves
const TREBLE_BOTTOM_Y: f32 = LEDGER_PAD + 4.0;
const BASS_BOTTOM_Y: f32 = 2.0 * LEDGER_PAD + 4.0 + STAFF_GAP + LEDGER_PAD + 4.0;
/// Height of a system in staff spaces.
pub const SYSTEM_HEIGHT: f32 = 2.0 * (2.0 * LEDGER_PAD + 4.0) + STAFF_GAP;
const BEAT_W: f32 = 3.8;
const BARLINE_W: f32 = 1.8;
const HEAD_RX: f32 = 0.72;
const HEAD_RY: f32 = 0.5;
const STEM_LEN: f32 = 3.5;
const THIN: f32 = 0.08;
const MEDIUM: f32 = 0.12;
const STEM_W: f32 = 0.1;
const CLEF_W: f32 = 4.2;
const KEY_SIG_STEP: f32 = 1.1;
const TIME_SIG_W: f32 = 2.4;

fn step_y(step: i32, clef: Clef) -> f32 {
    let bottom = match clef {
        Clef::Treble => TREBLE_BOTTOM_Y,
        Clef::Bass => BASS_BOTTOM_Y,
    };
    bottom - step as f32 * 0.5
}

fn treble_top_y() -> f32 {
    LEDGER_PAD
}

fn bass_top_y() -> f32 {
    BASS_BOTTOM_Y - 4.0
}

fn sharp_prims(x: f32, y: f32, out: &mut Vec<Prim>) {
    for dx in [-0.22, 0.22] {
        out.push(Prim::Line {
            x1: x + dx,
            y1: y - 1.0,
            x2: x + dx,
            y2: y + 1.0,
            width: THIN,
        });
    }
    for dy in [-0.28, 0.28] {
        out.push(Prim::Line {
            x1: x - 0.5,
            y1: y + dy + 0.12,
            x2: x + 0.5,
            y2: y + dy - 0.12,
            width: 0.2,
        });
    }
}

fn flat_prims(x: f32, y: f32, out: &mut Vec<Prim>) {
    out.push(Prim::Line {
        x1: x - 0.2,
        y1: y - 1.6,
        x2: x - 0.2,
        y2: y + 0.45,
        width: MEDIUM,
    });
    out.push(Prim::Path {
        points: vec![
            (x - 0.2, y + 0.45),
            (x + 0.25, y + 0.28),
            (x + 0.5, y - 0.1),
            (x + 0.3, y - 0.5),
            (x - 0.2, y - 0.25),
        ],
        width: MEDIUM,
    });
}

fn accidental_prims(accidental: Accidental, x: f32, y: f32, out: &mut Vec<Prim>) {
    match accidental {
        Accidental::Sharp => sharp_prims(x, y, out),
        Accidental::Flat => flat_prims(x, y, out),
        Accidental::Natural => {}
    }
}

/// Smooth curve through the control points (Catmull-Rom spline, `per_segment` samples each).
fn smooth(points: &[(f32, f32)], per_segment: usize) -> Vec<(f32, f32)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let at = |i: isize| points[i.clamp(0, points.len() as isize - 1) as usize];
    let mut out = Vec::new();
    for i in 0..points.len() as isize - 1 {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        for k in 0..per_segment {
            let t = k as f32 / per_segment as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * ((2.0 * b)
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out.push(*points.last().unwrap());
    out
}

/// Treble (G) clef: an upper curl running into a spiral around the G line, plus a long
/// stem that ends in a small tail below the staff.
fn treble_clef(x: f32, out: &mut Vec<Prim>) {
    let g_y = TREBLE_BOTTOM_Y - 1.0; // the G line
    let at = |dx: f32, dy: f32| (x + dx, g_y + dy);

    // Top curl, down across the staff and round the G line.
    let curl = [
        at(1.2, -3.95),
        at(1.75, -3.3),
        at(1.65, -2.3),
        at(1.05, -1.4),
        at(0.15, -0.5),
        at(-0.15, 0.55),
        at(0.35, 1.35),
        at(1.25, 1.5),
        at(1.95, 0.9),
        at(1.8, -0.05),
        at(1.1, -0.5),
        at(0.6, 0.1),
        at(0.75, 0.75),
    ];
    out.push(Prim::Path {
        points: smooth(&curl, 8),
        width: 0.17,
    });

    // Stem with the tail.
    let stem = [
        at(1.2, -3.95),
        at(1.28, -1.5),
        at(1.3, 1.0),
        at(1.25, 2.0),
        at(0.95, 2.75),
        at(0.45, 2.7),
        at(0.3, 2.2),
    ];
    out.push(Prim::Path {
        points: smooth(&stem, 8),
        width: 0.17,
    });
}

/// Bass (F) clef: a curl starting at the F line and two dots either side of it.
fn bass_clef(x: f32, out: &mut Vec<Prim>) {
    let f_y = bass_top_y() + 1.0; // the F line (second from the top)
    out.push(Prim::Ellipse {
        cx: x + 0.55,
        cy: f_y,
        rx: 0.42,
        ry: 0.42,
        tilt_deg: 0.0,
        filled: true,
        width: 0.0,
    });
    out.push(Prim::Path {
        points: vec![
            (x + 0.55, f_y),
            (x + 1.1, f_y - 0.55),
            (x + 1.9, f_y - 0.45),
            (x + 2.4, f_y + 0.35),
            (x + 2.2, f_y + 1.3),
            (x + 1.6, f_y + 2.2),
            (x + 0.8, f_y + 2.9),
            (x + 0.1, f_y + 3.1),
        ],
        width: 0.2,
    });
    for dy in [-0.5, 0.5] {
        out.push(Prim::Ellipse {
            cx: x + 3.05,
            cy: f_y + dy,
            rx: 0.26,
            ry: 0.26,
            tilt_deg: 0.0,
            filled: true,
            width: 0.0,
        });
    }
}

fn text_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * 0.56 * size
}

fn time_sig_prims(x: f32, top: &str, bottom: &str, out: &mut Vec<Prim>) {
    let size = 2.9;
    for (clef_top, _) in [(treble_top_y(), Clef::Treble), (bass_top_y(), Clef::Bass)] {
        for (text, baseline) in [(top, clef_top + 2.0 - 0.1), (bottom, clef_top + 4.0 - 0.1)] {
            out.push(Prim::Text {
                x: x + (TIME_SIG_W - text_width(text, size)) / 2.0,
                y: baseline,
                text: text.to_string(),
                size,
                bold: true,
            });
        }
    }
}

fn head_prims(cx: f32, y: f32, filled: bool, out: &mut Vec<Prim>) {
    out.push(Prim::Ellipse {
        cx,
        cy: y,
        rx: HEAD_RX,
        ry: HEAD_RY,
        tilt_deg: -18.0,
        filled,
        width: 0.12,
    });
}

/// Draw one hand of one column: ledger lines, accidentals, heads, stem and flag.
fn hand_prims(
    notes: &[Note],
    clef: Clef,
    duration: Duration,
    stem_up: bool,
    cx: f32,
    out: &mut Vec<Prim>,
) {
    if notes.is_empty() {
        return;
    }
    let mut sorted: Vec<Note> = notes.to_vec();
    sorted.sort_by_key(|n| n.diatonic());
    let steps: Vec<i32> = sorted.iter().map(|n| staff_step(n, clef)).collect();

    for e in ledger_steps(&steps) {
        let y = step_y(e, clef);
        out.push(Prim::Line {
            x1: cx - HEAD_RX - 0.55,
            y1: y,
            x2: cx + HEAD_RX + 0.55 + if sorted.len() > 1 { HEAD_RX } else { 0.0 },
            y2: y,
            width: THIN,
        });
    }

    // Heads of a chord a second apart sit on opposite sides of the stem.
    let mut displaced = false;
    let mut prev_step: Option<i32> = None;
    let mut ys: Vec<f32> = Vec::new();
    for (note, &step) in sorted.iter().zip(&steps) {
        let y = step_y(step, clef);
        ys.push(y);
        let shift = matches!(prev_step, Some(p) if step - p == 1) && !displaced;
        displaced = shift;
        let x = if shift { cx + 2.0 * HEAD_RX - 0.1 } else { cx };
        accidental_prims(note.accidental, cx - HEAD_RX - 1.1, y, out);
        head_prims(x, y, !duration.hollow(), out);
        prev_step = Some(step);
    }

    if duration.has_stem() {
        let top = ys.iter().cloned().fold(f32::MAX, f32::min);
        let bottom = ys.iter().cloned().fold(f32::MIN, f32::max);
        let (sx, y_from, y_to) = if stem_up {
            (cx + HEAD_RX - 0.05, bottom, top - STEM_LEN)
        } else {
            (cx - HEAD_RX + 0.05, top, bottom + STEM_LEN)
        };
        out.push(Prim::Line {
            x1: sx,
            y1: y_from,
            x2: sx,
            y2: y_to,
            width: STEM_W,
        });
        if duration.has_flag() {
            let d = if stem_up { 1.0 } else { -1.0 };
            out.push(Prim::Path {
                points: vec![
                    (sx, y_to),
                    (sx + 0.9, y_to + 0.9 * d),
                    (sx + 1.25, y_to + 2.0 * d),
                    (sx + 0.7, y_to + 3.0 * d),
                ],
                width: 0.16,
            });
        }
    }
}

fn column_width(col: &Column) -> f32 {
    match col {
        Column::Barline => BARLINE_W,
        Column::Notes { .. } => BEAT_W,
    }
}

fn header_width(key_count: usize, with_time_sig: bool) -> f32 {
    let key = if key_count > 0 {
        key_count as f32 * KEY_SIG_STEP + 0.4
    } else {
        0.0
    };
    CLEF_W + key + if with_time_sig { TIME_SIG_W + 0.4 } else { 0.0 } + 1.0
}

fn build_system(cols: &[Column], key: &[KeyAccidental], time_sig: Option<(&str, &str)>) -> System {
    let header = header_width(key.len(), time_sig.is_some());
    let cols_w: f32 = cols.iter().map(column_width).sum();
    let width = header + cols_w + 0.6;
    let mut prims: Vec<Prim> = Vec::new();

    // Staff lines.
    for i in 0..5 {
        for top in [treble_top_y(), bass_top_y()] {
            let y = top + i as f32;
            prims.push(Prim::Line {
                x1: 0.0,
                y1: y,
                x2: width,
                y2: y,
                width: THIN,
            });
        }
    }
    // System line joining the staves.
    prims.push(Prim::Line {
        x1: 0.0,
        y1: treble_top_y(),
        x2: 0.0,
        y2: BASS_BOTTOM_Y,
        width: 0.18,
    });

    treble_clef(0.3, &mut prims);
    bass_clef(0.3, &mut prims);

    // Key signature on both staves.
    for (i, acc) in key.iter().enumerate() {
        let x = CLEF_W + 0.3 + i as f32 * KEY_SIG_STEP;
        let kind = if acc.sharp {
            Accidental::Sharp
        } else {
            Accidental::Flat
        };
        accidental_prims(kind, x, step_y(acc.treble_step, Clef::Treble), &mut prims);
        accidental_prims(kind, x, step_y(acc.bass_step, Clef::Bass), &mut prims);
    }
    if let Some((top, bottom)) = time_sig {
        let x = CLEF_W
            + if key.is_empty() {
                0.0
            } else {
                key.len() as f32 * KEY_SIG_STEP + 0.4
            };
        time_sig_prims(x + 0.2, top, bottom, &mut prims);
    }

    // Columns.
    let mut x = header;
    for col in cols {
        match col {
            Column::Barline => {
                prims.push(Prim::Line {
                    x1: x + BARLINE_W / 2.0,
                    y1: treble_top_y(),
                    x2: x + BARLINE_W / 2.0,
                    y2: BASS_BOTTOM_Y,
                    width: 0.12,
                });
            }
            Column::Notes {
                right,
                left,
                duration,
            } => {
                let cx = x + BEAT_W / 2.0;
                hand_prims(right, Clef::Treble, *duration, true, cx, &mut prims);
                hand_prims(left, Clef::Bass, *duration, false, cx, &mut prims);
            }
        }
        x += column_width(col);
    }

    System {
        width,
        height: SYSTEM_HEIGHT,
        prims,
    }
}

/// Lay out the lines of a piano part as systems no wider than `max_width` staff spaces.
/// The time signature is shown on the first system only; clefs and key signature on every
/// system. Lines are wrapped at a barline where possible. Empty lines produce nothing.
pub fn layout_systems(
    lines: &[Vec<Column>],
    key: &str,
    time_sig: &str,
    max_width: f32,
) -> Vec<System> {
    let key_sig = key_signature(key);
    let (ts_top, ts_bottom) = split_time_sig(time_sig);
    let mut systems: Vec<System> = Vec::new();

    for line in lines {
        let mut rest: &[Column] = line;
        while !rest.is_empty() {
            let first = systems.is_empty();
            let header = header_width(key_sig.len(), first);
            // Greedily take columns until the next one would not fit.
            let mut used = header + 0.6;
            let mut take = 0;
            let mut last_barline: Option<usize> = None;
            for (i, col) in rest.iter().enumerate() {
                let w = column_width(col);
                if used + w > max_width && take > 0 {
                    break;
                }
                used += w;
                take = i + 1;
                if matches!(col, Column::Barline) {
                    last_barline = Some(i + 1);
                }
            }
            // Prefer to break after a barline, unless the whole rest fits.
            if take < rest.len() {
                if let Some(b) = last_barline {
                    take = b;
                }
            }
            let chunk = &rest[..take];
            let has_notes = chunk.iter().any(|c| matches!(c, Column::Notes { .. }));
            if has_notes || chunk.len() > 1 {
                let ts = first.then_some((ts_top.as_str(), ts_bottom.as_str()));
                systems.push(build_system(chunk, &key_sig, ts));
            }
            rest = &rest[take..];
        }
    }
    systems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(text: &str) -> Note {
        Note::parse(text).unwrap()
    }

    fn cell(text: &str) -> TabCell {
        TabCell::Custom(text.to_string())
    }

    fn notes_col(right: &str, left: &str, dur: &str) -> TabCol {
        let mut cells: [TabCell; 8] = Default::default();
        cells[0] = cell(right);
        cells[1] = cell(left);
        cells[2] = cell(dur);
        TabCol::Notes(cells)
    }

    // ── notes ───────────────────────────────────────────────────────────────

    #[test]
    fn notes_parse_with_accidentals_and_octaves() {
        assert_eq!(n("C4").midi(), 60);
        assert_eq!(n("A4").midi(), 69);
        assert_eq!(n("F#5").midi(), 78);
        assert_eq!(n("Bb3").midi(), 58);
        assert_eq!(n("c4").name(), "C4"); // letter case is ignored
        assert_eq!(n(" E2 ").name(), "E2");
        assert_eq!(n("C-1").midi(), 0);
        assert_eq!(n("G9").midi(), 127);
    }

    #[test]
    fn invalid_notes_are_rejected() {
        for bad in ["", "H4", "C", "C#", "4", "C10", "C-2", "Cx4", "C 4 4", "##"] {
            assert!(Note::parse(bad).is_none(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn names_round_trip() {
        for text in ["C4", "F#5", "Bb3", "E2", "A0"] {
            assert_eq!(n(text).name(), text);
        }
    }

    #[test]
    fn chords_skip_invalid_entries() {
        let chord = parse_chord("C4, E4 ,, x, G4");
        assert_eq!(
            chord.iter().map(|x| x.name()).collect::<Vec<_>>(),
            ["C4", "E4", "G4"]
        );
        assert!(parse_chord("").is_empty());
    }

    #[test]
    fn transposing_keeps_pitch_arithmetic_and_spelling() {
        assert_eq!(n("C4").transposed(7, true).name(), "G4");
        assert_eq!(n("C4").transposed(-5, true).name(), "G3");
        assert_eq!(n("B4").transposed(1, true).name(), "C5"); // octave rolls over
        assert_eq!(n("C4").transposed(-1, true).name(), "B3");
        assert_eq!(n("C4").transposed(1, true).name(), "C#4");
        assert_eq!(n("C4").transposed(1, false).name(), "Db4");
        assert_eq!(n("F#4").transposed(0, false).name(), "Gb4"); // re-spelling only
        assert_eq!(n("A4").transposed(12, true).midi(), n("A4").midi() + 12);
    }

    // ── staff positions ─────────────────────────────────────────────────────

    #[test]
    fn staff_steps_match_the_clefs() {
        // Treble: E4 is the bottom line, F5 the top line, middle C one ledger line below.
        assert_eq!(staff_step(&n("E4"), Clef::Treble), 0);
        assert_eq!(staff_step(&n("G4"), Clef::Treble), 2);
        assert_eq!(staff_step(&n("F5"), Clef::Treble), 8);
        assert_eq!(staff_step(&n("C4"), Clef::Treble), -2);
        // Bass: G2 is the bottom line, A3 the top line, middle C one ledger line above.
        assert_eq!(staff_step(&n("G2"), Clef::Bass), 0);
        assert_eq!(staff_step(&n("A3"), Clef::Bass), 8);
        assert_eq!(staff_step(&n("C4"), Clef::Bass), 10);
        // Accidentals do not move a note on the staff.
        assert_eq!(
            staff_step(&n("F#5"), Clef::Treble),
            staff_step(&n("F5"), Clef::Treble)
        );
        assert_eq!(
            staff_step(&n("Bb4"), Clef::Treble),
            staff_step(&n("B4"), Clef::Treble)
        );
    }

    #[test]
    fn ledger_lines_start_one_step_outside_the_staff() {
        assert!(ledger_steps(&[0, 2, 4, 6, 8]).is_empty());
        assert!(ledger_steps(&[-1, 9]).is_empty()); // the spaces just outside need none
        assert_eq!(ledger_steps(&[-2]), vec![-2]);
        assert_eq!(ledger_steps(&[-3]), vec![-2]);
        assert_eq!(ledger_steps(&[-4]), vec![-4, -2]);
        assert_eq!(ledger_steps(&[10]), vec![10]);
        assert_eq!(ledger_steps(&[12, -2]), vec![-2, 10, 12]);
    }

    // ── key and time signatures ─────────────────────────────────────────────

    #[test]
    fn key_signatures_have_the_right_accidentals() {
        assert!(key_signature("C Major").is_empty());
        assert!(key_signature("A Minor").is_empty());
        let g = key_signature("G Major");
        assert_eq!(g.len(), 1);
        assert!(g[0].sharp);
        assert_eq!(g[0].treble_step, 8); // F# on the top line
        assert_eq!(key_signature("D Major").len(), 2);
        assert_eq!(key_signature("F#m").len(), 3);
        let f = key_signature("F Major");
        assert_eq!(f.len(), 1);
        assert!(!f[0].sharp);
        assert_eq!(f[0].treble_step, 4); // Bb on the middle line
        assert_eq!(key_signature("Bb Major").len(), 2);
        assert_eq!(key_signature("G Minor").len(), 2);
        assert_eq!(key_signature("C# Major").len(), 7);
        assert_eq!(key_signature("Cb Major").len(), 7);
        assert!(key_signature("").is_empty());
        assert!(key_signature("nonsense").is_empty());
    }

    #[test]
    fn time_signatures_are_split_and_sanitised() {
        assert_eq!(split_time_sig("3/4"), ("3".into(), "4".into()));
        assert_eq!(split_time_sig(" 12 / 8 "), ("12".into(), "8".into()));
        for bad in ["", "4", "x/y", "4/", "/4", "123/4", "4/4/4"] {
            assert_eq!(split_time_sig(bad), ("4".into(), "4".into()), "{bad:?}");
        }
    }

    #[test]
    fn durations_decide_head_and_stem() {
        assert_eq!(Duration::parse("1"), Duration::Whole);
        assert_eq!(Duration::parse("2"), Duration::Half);
        assert_eq!(Duration::parse("8"), Duration::Eighth);
        assert_eq!(Duration::parse("4"), Duration::Quarter);
        assert_eq!(Duration::parse("junk"), Duration::Quarter);
        assert!(Duration::Whole.hollow() && !Duration::Whole.has_stem());
        assert!(Duration::Half.hollow() && Duration::Half.has_stem());
        assert!(!Duration::Quarter.hollow() && !Duration::Quarter.has_flag());
        assert!(Duration::Eighth.has_flag());
    }

    // ── reading and transposing a part ──────────────────────────────────────

    fn sample_part() -> SongPart {
        let mut part = SongPart::new_piano_riff("Piano");
        part.tab_grid = vec![
            notes_col("C4, E4, G4", "C3", "4"),
            TabCol::Barline,
            notes_col("", "G2", "2"),
            TabCol::LineBreak,
            notes_col("D5", "", "8"),
        ];
        part
    }

    #[test]
    fn a_part_is_read_into_lines_of_columns() {
        let lines = columns_by_line(&sample_part());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].len(), 3);
        assert_eq!(lines[1].len(), 1);
        match &lines[0][0] {
            Column::Notes {
                right,
                left,
                duration,
            } => {
                assert_eq!(right.len(), 3);
                assert_eq!(left.len(), 1);
                assert_eq!(*duration, Duration::Quarter);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(lines[0][1], Column::Barline);
    }

    #[test]
    fn transposing_a_piano_part_moves_every_note_and_nothing_else() {
        let mut part = sample_part();
        transpose_part(&mut part, 2, true);
        let lines = columns_by_line(&part);
        match &lines[0][0] {
            Column::Notes {
                right,
                left,
                duration,
            } => {
                assert_eq!(
                    right.iter().map(|x| x.name()).collect::<Vec<_>>(),
                    ["D4", "F#4", "A4"]
                );
                assert_eq!(left[0].name(), "D3");
                assert_eq!(*duration, Duration::Quarter); // the length cell is untouched
            }
            other => panic!("unexpected {other:?}"),
        }
        // Other part kinds are not touched.
        let mut chords = SongPart::new("Verse");
        chords.tab_grid = sample_part().tab_grid;
        let before = chords.clone();
        transpose_part(&mut chords, 5, true);
        assert_eq!(chords, before);
        // Zero is a no-op, and unparseable entries survive.
        let mut odd = SongPart::new_piano_riff("Piano");
        odd.tab_grid = vec![notes_col("C4, ???", "", "4")];
        transpose_part(&mut odd, 0, true);
        assert_eq!(odd.tab_grid, vec![notes_col("C4, ???", "", "4")]);
        transpose_part(&mut odd, 1, true);
        assert_eq!(odd.tab_grid, vec![notes_col("C#4, ???", "", "4")]);
    }

    // ── layout ──────────────────────────────────────────────────────────────

    fn count(prims: &[Prim], f: impl Fn(&Prim) -> bool) -> usize {
        prims.iter().filter(|p| f(p)).count()
    }

    fn heads(prims: &[Prim]) -> Vec<(f32, f32, bool)> {
        prims
            .iter()
            .filter_map(|p| match p {
                Prim::Ellipse {
                    cx, cy, rx, filled, ..
                } if *rx > 0.5 => Some((*cx, *cy, *filled)),
                _ => None,
            })
            .collect()
    }

    fn layout(cols: Vec<Column>, key: &str) -> Vec<System> {
        layout_systems(&[cols], key, "4/4", 100.0)
    }

    fn note_col(right: &str, left: &str, duration: Duration) -> Column {
        Column::Notes {
            right: parse_chord(right),
            left: parse_chord(left),
            duration,
        }
    }

    #[test]
    fn every_note_of_a_chord_gets_a_head() {
        let sys = layout(
            vec![note_col("C4, E4, G4", "C2, G2", Duration::Quarter)],
            "C Major",
        );
        assert_eq!(sys.len(), 1);
        assert_eq!(heads(&sys[0].prims).len(), 5);
    }

    #[test]
    fn whole_and_half_notes_are_hollow_quarters_and_eighths_are_filled() {
        let filled_of = |d| heads(&layout(vec![note_col("C5", "", d)], "C Major")[0].prims)[0].2;
        assert!(!filled_of(Duration::Whole));
        assert!(!filled_of(Duration::Half));
        assert!(filled_of(Duration::Quarter));
        assert!(filled_of(Duration::Eighth));
    }

    #[test]
    fn stems_and_flags_follow_the_duration() {
        let stems = |d| {
            let sys = layout(vec![note_col("C5", "", d)], "C Major");
            let vertical = count(
                &sys[0].prims,
                |p| matches!(p, Prim::Line { x1, x2, y1, y2, width } if (x1 - x2).abs() < 1e-4 && (y1 - y2).abs() > 3.0 && *width <= 0.11 && *width >= 0.09),
            );
            let flags = count(
                &sys[0].prims,
                |p| matches!(p, Prim::Path { points, width } if (*width - 0.16).abs() < 1e-4 && points.len() == 4),
            );
            (vertical, flags)
        };
        assert_eq!(stems(Duration::Whole), (0, 0));
        assert_eq!(stems(Duration::Half), (1, 0));
        assert_eq!(stems(Duration::Quarter), (1, 0));
        assert_eq!(stems(Duration::Eighth), (1, 1));
    }

    #[test]
    fn higher_notes_are_drawn_higher_and_the_bass_staff_is_below() {
        let sys = layout(vec![note_col("C5, G5", "C3", Duration::Quarter)], "C Major");
        let mut h = heads(&sys[0].prims);
        h.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        // G5 (top), C5, then C3 in the bass staff: y grows downwards.
        assert!(h[0].1 < h[1].1 && h[1].1 < h[2].1);
        assert!(h[2].1 > sys[0].height / 2.0);
        assert!(h[0].1 >= -1.0 && h[2].1 <= sys[0].height);
    }

    #[test]
    fn middle_c_gets_a_ledger_line_in_each_clef() {
        let ledgers = |right: &str, left: &str| {
            let sys = layout(vec![note_col(right, left, Duration::Quarter)], "C Major");
            count(
                &sys[0].prims,
                |p| matches!(p, Prim::Line { y1, y2, x1, x2, width } if (y1 - y2).abs() < 1e-4 && (x2 - x1) < 3.0 && (x2 - x1) > 1.0 && *width < 0.09),
            )
        };
        assert_eq!(ledgers("C4", ""), 1);
        assert_eq!(ledgers("", "C4"), 1);
        assert_eq!(ledgers("E4, G4", "G2, A3"), 0);
        assert_eq!(ledgers("C6", ""), 2); // steps 10 and 12
        assert_eq!(ledgers("E6", ""), 3); // steps 10, 12 and 14
    }

    #[test]
    fn neighbouring_notes_of_a_chord_do_not_overlap() {
        let sys = layout(vec![note_col("B4, C5", "", Duration::Quarter)], "C Major");
        let h = heads(&sys[0].prims);
        assert_eq!(h.len(), 2);
        assert!(
            (h[0].0 - h[1].0).abs() > 1.0,
            "second-interval heads must be side by side"
        );
    }

    #[test]
    fn the_key_signature_is_drawn_on_both_staves_and_changes_the_width() {
        let none = layout(vec![note_col("C5", "", Duration::Quarter)], "C Major");
        let two = layout(vec![note_col("C5", "", Duration::Quarter)], "D Major");
        assert!(two[0].width > none[0].width);
        let sharps = |sys: &System| {
            count(
                &sys.prims,
                |p| matches!(p, Prim::Line { x1, x2, y1, y2, width } if (x1 - x2).abs() < 1e-4 && (y2 - y1 - 2.0).abs() < 1e-4 && (*width - 0.08).abs() < 1e-4),
            )
        };
        // Each sharp has two vertical bars; D major has 2 sharps on 2 staves.
        assert_eq!(sharps(&two[0]) - sharps(&none[0]), 2 * 2 * 2);
    }

    #[test]
    fn only_the_first_system_shows_the_time_signature() {
        let lines = vec![
            vec![note_col("C5", "", Duration::Quarter)],
            vec![note_col("D5", "", Duration::Quarter)],
        ];
        let sys = layout_systems(&lines, "C Major", "3/4", 100.0);
        assert_eq!(sys.len(), 2);
        let digits = |s: &System| count(&s.prims, |p| matches!(p, Prim::Text { .. }));
        assert_eq!(digits(&sys[0]), 4); // 3 over 4, on both staves
        assert_eq!(digits(&sys[1]), 0);
        assert!(sys[1].width < sys[0].width);
    }

    #[test]
    fn long_lines_wrap_at_barlines_within_the_page_width() {
        let mut cols = Vec::new();
        for _ in 0..6 {
            for _ in 0..4 {
                cols.push(note_col("C5", "C3", Duration::Quarter));
            }
            cols.push(Column::Barline);
        }
        let sys = layout_systems(&[cols], "C Major", "4/4", 45.0);
        assert!(
            sys.len() >= 3,
            "expected wrapping, got {} system(s)",
            sys.len()
        );
        for s in &sys {
            assert!(
                s.width <= 45.0 + 0.001,
                "system of width {} exceeds the page",
                s.width
            );
            // Every system starts with clefs and ends at a barline or the last column.
            assert!(!s.prims.is_empty());
        }
        let total_heads: usize = sys.iter().map(|s| heads(&s.prims).len()).sum();
        assert_eq!(total_heads, 6 * 4 * 2); // nothing lost when wrapping
    }

    #[test]
    fn a_single_column_wider_than_the_page_still_produces_output() {
        let sys = layout_systems(
            &[vec![note_col("C5", "", Duration::Quarter)]],
            "C# Major",
            "4/4",
            1.0,
        );
        assert_eq!(sys.len(), 1);
        assert_eq!(heads(&sys[0].prims).len(), 1);
    }

    #[test]
    fn empty_parts_and_lines_produce_no_systems() {
        assert!(layout_systems(&[vec![]], "C Major", "4/4", 100.0).is_empty());
        assert!(layout_systems(&[], "C Major", "4/4", 100.0).is_empty());
        assert!(layout_systems(&[vec![Column::Barline]], "C Major", "4/4", 100.0).is_empty());
    }

    #[test]
    fn invalid_notes_are_ignored_by_the_layout_but_do_not_break_it() {
        let sys = layout(
            vec![note_col("nope, C5", "???", Duration::Quarter)],
            "C Major",
        );
        assert_eq!(heads(&sys[0].prims).len(), 1);
    }
}
