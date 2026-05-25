// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.

use std::collections::HashMap;

// ── Chord quality ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChordQuality {
    Major,
    Minor,
    Dominant7,
    Major7,
    Minor7,
    Diminished,
    Augmented,
    Sus2,
    Sus4,
}

impl ChordQuality {
    pub fn symbol(&self) -> &str {
        match self {
            ChordQuality::Major => "",
            ChordQuality::Minor => "m",
            ChordQuality::Dominant7 => "7",
            ChordQuality::Major7 => "maj7",
            ChordQuality::Minor7 => "m7",
            ChordQuality::Diminished => "dim",
            ChordQuality::Augmented => "aug",
            ChordQuality::Sus2 => "sus2",
            ChordQuality::Sus4 => "sus4",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            ChordQuality::Major => "Major",
            ChordQuality::Minor => "Minor",
            ChordQuality::Dominant7 => "Dom 7",
            ChordQuality::Major7 => "Maj 7",
            ChordQuality::Minor7 => "Min 7",
            ChordQuality::Diminished => "Dim",
            ChordQuality::Augmented => "Aug",
            ChordQuality::Sus2 => "Sus 2",
            ChordQuality::Sus4 => "Sus 4",
        }
    }

    pub fn all() -> Vec<ChordQuality> {
        vec![
            ChordQuality::Major,
            ChordQuality::Minor,
            ChordQuality::Dominant7,
            ChordQuality::Major7,
            ChordQuality::Minor7,
            ChordQuality::Diminished,
            ChordQuality::Augmented,
            ChordQuality::Sus2,
            ChordQuality::Sus4,
        ]
    }

    pub fn from_symbol(s: &str) -> ChordQuality {
        match s {
            "m" => ChordQuality::Minor,
            "7" => ChordQuality::Dominant7,
            "maj7" => ChordQuality::Major7,
            "m7" => ChordQuality::Minor7,
            "dim" => ChordQuality::Diminished,
            "aug" => ChordQuality::Augmented,
            "sus2" => ChordQuality::Sus2,
            "sus4" => ChordQuality::Sus4,
            _ => ChordQuality::Major,
        }
    }
}

// ── Notation ─────────────────────────────────────────────────────────────────

/// Which note-naming convention to use when displaying chords.
///
/// | Name    | B natural | B-flat |
/// |---------|-----------|--------|
/// | English | B         | B♭     |
/// | German  | H         | B      |
/// | Custom  | H         | B♭     |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Notation {
    #[default]
    English,
    German,
    Custom,
}

/// Apply a notation convention to a single note name.
/// A# is always left as A# regardless of convention — only Bb is renamed.
pub fn apply_notation(note: &str, notation: Notation) -> String {
    match notation {
        Notation::English => note.to_string(),
        Notation::German => match note {
            "B" => "H".to_string(),
            "Bb" => "B".to_string(),
            _ => note.to_string(),
        },
        Notation::Custom => match note {
            "B" => "H".to_string(),
            _ => note.to_string(), // Bb stays Bb, A# stays A#
        },
    }
}

// ── Chord ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Chord {
    pub root: String,
    pub quality: ChordQuality,
    /// Optional bass note for slash chord notation, e.g. `G/B`.
    #[serde(default)]
    pub bass_note: Option<String>,
}

impl Chord {
    pub fn new(root: impl Into<String>, quality: ChordQuality) -> Self {
        Self {
            root: root.into(),
            quality,
            bass_note: None,
        }
    }

    /// Human-readable chord name, e.g. `"Am"`, `"G7"`, `"Fmaj7"`, `"G/B"`.
    pub fn display(&self) -> String {
        match &self.bass_note {
            Some(b) => format!("{}{}/{}", self.root, self.quality.symbol(), b),
            None => format!("{}{}", self.root, self.quality.symbol()),
        }
    }

    /// Chord name with the given notation convention applied to root and bass.
    pub fn display_with_notation(&self, notation: Notation) -> String {
        let root = apply_notation(&self.root, notation);
        match &self.bass_note {
            Some(b) => format!(
                "{}{}/{}",
                root,
                self.quality.symbol(),
                apply_notation(b, notation)
            ),
            None => format!("{}{}", root, self.quality.symbol()),
        }
    }
}

// ── Tab grid types ───────────────────────────────────────────────────────────

/// A single note cell in a tab grid column.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum TabCell {
    /// No note on this string at this beat.
    #[default]
    Empty,
    /// A fretted note (0 = open, 1–24).
    Fret(u8),
    /// Muted / dead string (written as "x").
    Muted,
    /// Ghost note — written as "(n)" where n is the fret.
    Ghost(u8),
    /// Hammer-on ("h").
    HammerOn,
    /// Pull-off ("p").
    PullOff,
    /// Release bend ("r").
    Release,
    /// Bend ("b").
    Bend,
    /// Slide up ("/").
    SlideUp,
    /// Slide down ("\\").
    SlideDown,
    /// Free-form combination (e.g. "5h", "7b", "5h7", "(5)b").
    Custom(String),
}

/// A single column in a tab grid — either a beat with 6 note cells, a barline, or a row break.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TabCol {
    /// Six note cells (one per string), index 0 = high e.
    Notes([TabCell; 8]),
    /// A vertical barline (end-of-bar marker).
    Barline,
    /// Ends the current row and starts a new one with fresh string labels.
    LineBreak,
}

// ── Part kind ─────────────────────────────────────────────────────────────────

/// Whether a song part holds chords or a guitar-tab riff.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum PartKind {
    #[default]
    Chords,
    /// Free-form guitar tab (stored as a plain string).
    Riff,
    /// 4-string bass tab (G, D, A, E — indices 2-5 of the shared TabCol array).
    BassRiff,
    /// 8-track drum beat grid (K, S, Hi, R, C, T1, T2, T3 — all 8 cells).
    DrumBeat,
}

// ── Part items ────────────────────────────────────────────────────────────────

/// A single item inside a song part — either a chord or a structural marker.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum PartItem {
    /// A regular chord.
    Chord(Chord),
    /// Forces a new row in the chord grid.
    LineBreak,
    /// A repeat barline (e.g. ‖: … :‖). `times` = 0 means plain repeat with no number.
    Repeat { times: u8 },
    /// A plain double barline (||) marking the start of a repeated section.
    RepeatStart,
    /// Start of a volta bracket, e.g. "1." or "2.".
    VoltaBracketStart { label: String },
    /// End of a volta bracket.
    VoltaBracketEnd,
    /// A free-text comment / annotation for this part. Always rendered on its own line.
    /// `color` is a CSS colour string (e.g. `"#888888"`).
    Text { content: String, color: String },
}

// ── Part text ─────────────────────────────────────────────────────────────────

/// Per-part vocal / lyrics annotation shown below the chords or tab.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PartTextSettings {
    /// The lyrics / comment text. Use `\n` for line breaks.
    pub content: String,
    /// CSS colour string for the text (e.g. `"#555555"`).
    #[serde(default = "PartTextSettings::default_color")]
    pub color: String,
    /// Font size in pt used in the PDF (default 10).
    #[serde(default = "PartTextSettings::default_size")]
    pub size: u32,
    /// Whether to show the part's chords alongside the lyrics (two-column layout).
    #[serde(default)]
    pub show_chords: bool,
}

impl PartTextSettings {
    fn default_color() -> String {
        "#555555".to_string()
    }
    fn default_size() -> u32 {
        10
    }
    pub fn new() -> Self {
        Self {
            content: String::new(),
            color: Self::default_color(),
            size: Self::default_size(),
            show_chords: false,
        }
    }
}

impl Default for PartTextSettings {
    fn default() -> Self {
        Self::new()
    }
}

// ── Song part ─────────────────────────────────────────────────────────────────

/// A named section of a song (e.g. "Verse", "Chorus", or anything the user chooses).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SongPart {
    pub name: String,
    /// Whether this part holds chords or a tab riff.
    #[serde(default)]
    pub kind: PartKind,
    /// Tab content — only used when `kind == PartKind::Riff`.
    #[serde(default)]
    pub tab: String,
    /// Structured tab grid — 6 strings × N beats. Index 0 = high e.
    /// Each column is either `Notes([TabCell; 8])` or a `Barline`.
    #[serde(default)]
    pub tab_grid: Vec<TabCol>,
    /// All items in this part in order — only used when `kind == PartKind::Chords`.
    #[serde(alias = "chords")]
    pub items: Vec<PartItem>,
    /// Optional vocals / lyrics text shown below the chords or tab.
    #[serde(default)]
    pub part_text: Option<PartTextSettings>,
}

impl SongPart {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: PartKind::Chords,
            tab: String::new(),
            tab_grid: Vec::new(),
            items: Vec::new(),
            part_text: None,
        }
    }

    pub fn new_riff(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: PartKind::Riff,
            tab: String::new(),
            tab_grid: Vec::new(),
            items: Vec::new(),
            part_text: None,
        }
    }

    pub fn new_bass_riff(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: PartKind::BassRiff,
            tab: String::new(),
            tab_grid: Vec::new(),
            items: Vec::new(),
            part_text: None,
        }
    }

    pub fn new_drum_beat(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: PartKind::DrumBeat,
            tab: String::new(),
            tab_grid: Vec::new(),
            items: Vec::new(),
            part_text: None,
        }
    }

    /// Iterate mutably over only the `Chord` items in this part.
    pub fn chords_mut(&mut self) -> impl Iterator<Item = &mut Chord> {
        self.items.iter_mut().filter_map(|item| {
            if let PartItem::Chord(c) = item {
                Some(c)
            } else {
                None
            }
        })
    }

    /// Render this riff part as an ASCII-tab string, e.g.
    /// ```text
    /// e|--0--2--|
    /// B|--------|
    /// ```
    /// Falls back to `self.tab` when `tab_grid` is empty (legacy free-text).
    pub fn tab_as_ascii(&self) -> String {
        if self.tab_grid.is_empty() {
            return self.tab.clone();
        }
        let (labels, string_range): (&[&str], std::ops::Range<usize>) =
            if self.kind == PartKind::BassRiff {
                (&["G", "D", "A", "E"], 2..6)
            } else if self.kind == PartKind::DrumBeat {
                (&["K", "S", "Hi", "R", "C", "T1", "T2", "T3"], 0..8)
            } else {
                (&["e", "B", "G", "D", "A", "E"], 0..6)
            };
        let mut rows: Vec<String> = labels.iter().map(|l| format!("{l}|")).collect();
        let mut output = String::new();
        let mut block_has_content = false;
        for col in &self.tab_grid {
            match col {
                TabCol::LineBreak => {
                    // Only flush if the current block has actual content.
                    if block_has_content {
                        for row in &mut rows {
                            row.push('|');
                        }
                        for row in &rows {
                            output.push_str(row);
                            output.push('\n');
                        }
                        output.push('\n'); // blank separator line
                    }
                    // start fresh block
                    rows = labels.iter().map(|l| format!("{l}|")).collect();
                    block_has_content = false;
                }
                TabCol::Barline => {
                    for row in &mut rows {
                        row.push('|');
                    }
                    block_has_content = true;
                }
                TabCol::Notes(arr) => {
                    for (row_i, str_idx) in string_range.clone().enumerate() {
                        let cell = &arr[str_idx];
                        match cell {
                            TabCell::Fret(f) if *f >= 10 => rows[row_i].push_str(&format!("{f}-")),
                            TabCell::Fret(f) => rows[row_i].push_str(&format!("-{f}-")),
                            TabCell::Muted => rows[row_i].push_str("-x-"),
                            TabCell::Empty => rows[row_i].push_str("---"),
                            TabCell::Ghost(f) => rows[row_i].push_str(&format!("({f})")),
                            TabCell::HammerOn => rows[row_i].push_str("-h-"),
                            TabCell::PullOff => rows[row_i].push_str("-p-"),
                            TabCell::Release => rows[row_i].push_str("-r-"),
                            TabCell::Bend => rows[row_i].push_str("-b-"),
                            TabCell::SlideUp => rows[row_i].push_str("-/-"),
                            TabCell::SlideDown => rows[row_i].push_str("-\\-"),
                            TabCell::Custom(s) => rows[row_i].push_str(s),
                        }
                    }
                    block_has_content = true;
                }
            }
        }
        // close and flush last block (only if it has content)
        if block_has_content {
            for row in &mut rows {
                row.push('|');
            }
            for row in &rows {
                output.push_str(row);
                output.push('\n');
            }
        }
        if output.ends_with('\n') {
            output.pop();
        }
        output
    }

    /// Iterate over only the `Chord` items in this part.
    pub fn chords(&self) -> impl Iterator<Item = &Chord> {
        self.items.iter().filter_map(|item| {
            if let PartItem::Chord(c) = item {
                Some(c)
            } else {
                None
            }
        })
    }
}

// ── Music-theory helpers ────────────────────────────────────────────────────

/// Returns the chromatic index (0 = C … 11 = B) for a note name, or `None`.
fn note_to_index(note: &str) -> Option<u8> {
    match note.trim() {
        "C" | "B#" => Some(0),
        "C#" | "Db" => Some(1),
        "D" => Some(2),
        "D#" | "Eb" => Some(3),
        "E" | "Fb" => Some(4),
        "F" | "E#" => Some(5),
        "F#" | "Gb" => Some(6),
        "G" => Some(7),
        "G#" | "Ab" => Some(8),
        "A" => Some(9),
        "A#" | "Bb" => Some(10),
        "B" | "Cb" => Some(11),
        _ => None,
    }
}

/// Converts a chromatic index back to a note name.
/// Uses sharps for C, G, D, A, E, B, F#, C# and flats for the rest.
fn index_to_note(index: u8, prefer_sharps: bool) -> &'static str {
    match (index % 12, prefer_sharps) {
        (0, _) => "C",
        (1, true) => "C#",
        (1, false) => "Db",
        (2, _) => "D",
        (3, true) => "D#",
        (3, false) => "Eb",
        (4, _) => "E",
        (5, _) => "F",
        (6, true) => "F#",
        (6, false) => "Gb",
        (7, _) => "G",
        (8, true) => "G#",
        (8, false) => "Ab",
        (9, _) => "A",
        (10, true) => "A#",
        (10, false) => "Bb",
        (11, _) => "B",
        _ => "?",
    }
}

/// Returns whether `root` is conventionally written with sharps in a major or minor key.
/// Major sharp keys:  C  G  D  A  E  B  F#  C#
/// Minor sharp keys:  A  E  B  F#  C#  G#  D#  (relative minors of the major sharp keys)
pub fn prefer_sharps_for_key(root: &str, is_minor: bool) -> bool {
    if is_minor {
        matches!(root, "A" | "E" | "B" | "F#" | "C#" | "G#" | "D#")
    } else {
        matches!(root, "C" | "G" | "D" | "A" | "E" | "B" | "F#" | "C#")
    }
}

/// When the user *explicitly* picks a note name (e.g. from a dropdown), honour
/// the spelling they chose: a `#` suffix → sharps, a `b` suffix → flats,
/// natural notes → defer to the conventional key-signature logic.
fn explicit_prefer_sharps(root: &str, is_minor: bool) -> bool {
    if root.ends_with('#') {
        true
    } else if root.ends_with('b') && root.len() > 1 {
        false
    } else {
        prefer_sharps_for_key(root, is_minor)
    }
}

/// Shift a note root up by `semitones` chromatically.
/// `is_minor` selects minor-key vs major-key sharp/flat conventions.
/// Returns the original string unchanged if it cannot be parsed.
pub fn shift_note_up(root: &str, semitones_up: u8, is_minor: bool) -> String {
    if semitones_up == 0 {
        return root.to_string();
    }
    note_to_index(root)
        .map(|idx| {
            let new_idx = ((idx as u16) + (semitones_up as u16)) % 12;
            // Determine the canonical note name in sharps first, then check preference.
            let sharp_name = index_to_note(new_idx as u8, true);
            let prefer_sharps = prefer_sharps_for_key(sharp_name, is_minor);
            index_to_note(new_idx as u8, prefer_sharps).to_string()
        })
        .unwrap_or_else(|| root.to_string())
}

/// Shift a note root down by `semitones` chromatically.
/// `is_minor` selects minor-key vs major-key sharp/flat conventions.
/// Returns the original string unchanged if it cannot be parsed.
pub fn shift_note(root: &str, semitones_down: u8, is_minor: bool) -> String {
    if semitones_down == 0 {
        return root.to_string();
    }
    note_to_index(root)
        .map(|idx| {
            let new_idx = ((idx as i16) - (semitones_down as i16)).rem_euclid(12) as u8;
            let sharp_name = index_to_note(new_idx, true);
            let prefer_sharps = prefer_sharps_for_key(sharp_name, is_minor);
            index_to_note(new_idx, prefer_sharps).to_string()
        })
        .unwrap_or_else(|| root.to_string())
}

// ── Instrument ────────────────────────────────────────────────────────────────

/// Which instrument(s) this chord sheet is arranged for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Instrument {
    Guitar,
    AcousticGuitar,
    Bass,
    Piano,
    Drums,
    Vocals,
}

impl Instrument {
    pub fn label(self) -> &'static str {
        match self {
            Instrument::Guitar => "Electric",
            Instrument::AcousticGuitar => "Acoustic",
            Instrument::Bass => "Bass",
            Instrument::Piano => "Piano",
            Instrument::Drums => "Drums",
            Instrument::Vocals => "Vocals",
        }
    }

    /// Accent colour used on the instrument sheet page.
    pub fn accent_color(self) -> &'static str {
        match self {
            Instrument::Guitar => "#1a5c38",
            Instrument::AcousticGuitar => "#7c4a00",
            Instrument::Bass => "#1a2e5c",
            Instrument::Piano => "#4a1a6e",
            Instrument::Drums => "#7c1a1a",
            Instrument::Vocals => "#3a3a4e",
        }
    }

    /// Parse from the label string (used for URL routing).
    pub fn from_label(s: &str) -> Option<Instrument> {
        match s {
            "Electric" => Some(Instrument::Guitar),
            "Acoustic" => Some(Instrument::AcousticGuitar),
            "Bass" => Some(Instrument::Bass),
            "Piano" => Some(Instrument::Piano),
            "Drums" => Some(Instrument::Drums),
            "Vocals" => Some(Instrument::Vocals),
            _ => None,
        }
    }

    pub fn all() -> [Instrument; 6] {
        [
            Instrument::Guitar,
            Instrument::AcousticGuitar,
            Instrument::Bass,
            Instrument::Piano,
            Instrument::Drums,
            Instrument::Vocals,
        ]
    }
}

// ── PDF settings ─────────────────────────────────────────────────────────────

/// Per-sheet PDF font-size settings.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PdfSettings {
    /// Font size for part-label text (default 9 pt).
    pub part_name_size: u32,
    /// Font size for chord roots (default 18 pt).
    pub chord_size: u32,
}

impl Default for PdfSettings {
    fn default() -> Self {
        Self {
            part_name_size: 9,
            chord_size: 18,
        }
    }
}

// ── Song ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Song {
    pub name: String,
    pub key: String,
    pub artist: String,
    /// Ordered list of named parts, each with its own chord progression.
    pub parts: Vec<SongPart>,
    /// Which instruments this arrangement is for.
    #[serde(default)]
    pub instruments: Vec<Instrument>,
    /// Free-form vocals / lyrics notes.
    #[serde(default)]
    pub vocals_notes: String,
    /// Per-instrument chord overrides. Key = Instrument::label().
    #[serde(default)]
    pub instrument_parts: HashMap<String, Vec<SongPart>>,
    /// Per-instrument capo/transpose settings. Key = Instrument::label(). Positive = capo (shift down), negative = transpose up.
    #[serde(default)]
    pub instrument_capos: HashMap<String, i8>,
    /// Per-sheet PDF font-size settings. Key = "Base" or Instrument::label().
    #[serde(default)]
    pub pdf_settings: HashMap<String, PdfSettings>,
}

impl Song {
    pub fn new(name: impl Into<String>, key: impl Into<String>, artist: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            key: key.into(),
            artist: artist.into(),
            parts: Vec::new(),
            instruments: Vec::new(),
            vocals_notes: String::new(),
            instrument_parts: HashMap::new(),
            instrument_capos: HashMap::new(),
            pdf_settings: HashMap::new(),
        }
    }

    /// Builder-style helper – appends a new named part with the given chords.
    pub fn with_part(mut self, name: impl Into<String>, chords: Vec<Chord>) -> Self {
        self.parts.push(SongPart {
            name: name.into(),
            kind: PartKind::Chords,
            tab: String::new(),
            tab_grid: Vec::new(),
            items: chords.into_iter().map(PartItem::Chord).collect(),
            part_text: None,
        });
        self
    }

    /// Transpose all chords to a new key root (e.g. `"G"`, `"Bb"`, `"F#"`).
    ///
    /// All chord roots are shifted by the same semitone interval between the
    /// current key root and `new_root`. The song's `key` field is updated.
    pub fn transpose_to(&mut self, new_root: &str) {
        let new_idx = match note_to_index(new_root) {
            Some(i) => i,
            None => return,
        };
        let old_root = self.key.split_whitespace().next().unwrap_or("C");
        let old_idx = match note_to_index(old_root) {
            Some(i) => i,
            None => return,
        };
        let interval = (new_idx + 12 - old_idx) % 12;
        if interval == 0 {
            // Same pitch, but possibly a different spelling (e.g. A# → Bb).
            // Re-spell all chord roots to match the new key's accidental preference.
            let is_minor = self.key.to_lowercase().contains("minor");
            let prefer_sharps = explicit_prefer_sharps(new_root, is_minor);
            let respell = |chord: &mut Chord| {
                if let Some(idx) = note_to_index(&chord.root) {
                    chord.root = index_to_note(idx, prefer_sharps).to_string();
                }
                if let Some(bass) = &chord.bass_note {
                    if let Some(idx) = note_to_index(bass) {
                        chord.bass_note = Some(index_to_note(idx, prefer_sharps).to_string());
                    }
                }
            };
            for part in &mut self.parts {
                for chord in part.chords_mut() {
                    respell(chord);
                }
            }
            for parts in self.instrument_parts.values_mut() {
                for part in parts.iter_mut() {
                    for chord in part.chords_mut() {
                        respell(chord);
                    }
                }
            }
            let mode = self
                .key
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>()
                .join(" ");
            self.key = if mode.is_empty() {
                new_root.to_string()
            } else {
                format!("{} {}", new_root, mode)
            };
            return;
        }
        let is_minor = self.key.to_lowercase().contains("minor");
        let prefer_sharps = explicit_prefer_sharps(new_root, is_minor);
        let shift_chord = |chord: &mut Chord| {
            if let Some(root_idx) = note_to_index(&chord.root) {
                chord.root = index_to_note((root_idx + interval) % 12, prefer_sharps).to_string();
            }
            if let Some(bass) = &chord.bass_note {
                if let Some(bass_idx) = note_to_index(bass) {
                    chord.bass_note =
                        Some(index_to_note((bass_idx + interval) % 12, prefer_sharps).to_string());
                }
            }
        };
        for part in &mut self.parts {
            for chord in part.chords_mut() {
                shift_chord(chord);
            }
        }
        for parts in self.instrument_parts.values_mut() {
            for part in parts.iter_mut() {
                for chord in part.chords_mut() {
                    shift_chord(chord);
                }
            }
        }
        let mode = self
            .key
            .split_whitespace()
            .skip(1)
            .collect::<Vec<_>>()
            .join(" ");
        self.key = if mode.is_empty() {
            new_root.to_string()
        } else {
            format!("{} {}", new_root, mode)
        };
    }

    /// Return a copy of this song with every chord root shifted by
    /// `capo` semitones.  Positive = shift **down** (capo on a string instrument);
    /// negative = shift **up** (transpose up for piano etc.).
    ///
    /// When `capo == 0` returns an unchanged clone.
    pub fn apply_capo(&self, capo: i8) -> Song {
        if capo == 0 {
            return self.clone();
        }
        let mut result = self.clone();
        let key_root = self.key.split_whitespace().next().unwrap_or("C");
        let prefer_sharps = note_to_index(key_root)
            .map(|orig_idx| {
                let shifted = ((orig_idx as i16) - (capo as i16)).rem_euclid(12) as u8;
                matches!(
                    index_to_note(shifted, true),
                    "C" | "G" | "D" | "A" | "E" | "B" | "F#" | "C#"
                )
            })
            .unwrap_or(true);
        // Update the key's root note.
        if let Some(orig_idx) = note_to_index(key_root) {
            let new_idx = ((orig_idx as i16) - (capo as i16)).rem_euclid(12) as u8;
            let new_root = index_to_note(new_idx, prefer_sharps);
            let mode = self
                .key
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>()
                .join(" ");
            result.key = if mode.is_empty() {
                new_root.to_string()
            } else {
                format!("{} {}", new_root, mode)
            };
        }
        // Shift every chord root and bass note.
        // apply_capo shifts down (for display), preserve the current key's sharp/flat preference.
        let is_minor = result.key.to_lowercase().contains("minor");
        let key_root = result.key.split_whitespace().next().unwrap_or("C");
        let _prefer = prefer_sharps_for_key(key_root, is_minor);
        for part in &mut result.parts {
            for chord in part.chords_mut() {
                if capo > 0 {
                    chord.root = shift_note(&chord.root, capo as u8, is_minor);
                    if let Some(bass) = &chord.bass_note {
                        chord.bass_note = Some(shift_note(bass, capo as u8, is_minor));
                    }
                } else {
                    chord.root = shift_note_up(&chord.root, (-capo) as u8, is_minor);
                    if let Some(bass) = &chord.bass_note {
                        chord.bass_note = Some(shift_note_up(bass, (-capo) as u8, is_minor));
                    }
                }
            }
        }
        result
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChordQuality ─────────────────────────────────────────────────────────

    #[test]
    fn chord_quality_symbols_round_trip() {
        for q in ChordQuality::all() {
            assert_eq!(ChordQuality::from_symbol(q.symbol()), q);
        }
    }

    #[test]
    fn chord_quality_major_symbol_is_empty() {
        assert_eq!(ChordQuality::Major.symbol(), "");
    }

    #[test]
    fn chord_quality_unknown_symbol_falls_back_to_major() {
        assert_eq!(ChordQuality::from_symbol("xyz"), ChordQuality::Major);
    }

    #[test]
    fn chord_quality_all_has_nine_variants() {
        assert_eq!(ChordQuality::all().len(), 9);
    }

    // ── Chord ────────────────────────────────────────────────────────────────

    #[test]
    fn chord_display_major_has_no_suffix() {
        let c = Chord::new("G", ChordQuality::Major);
        assert_eq!(c.display(), "G");
    }

    #[test]
    fn chord_display_minor() {
        let c = Chord::new("A", ChordQuality::Minor);
        assert_eq!(c.display(), "Am");
    }

    #[test]
    fn chord_display_complex_quality() {
        let c = Chord::new("F", ChordQuality::Major7);
        assert_eq!(c.display(), "Fmaj7");
    }

    // ── Song / SongPart ──────────────────────────────────────────────────────

    #[test]
    fn song_new_starts_with_no_parts() {
        let s = Song::new("Test", "C Major", "Artist");
        assert!(s.parts.is_empty());
    }

    #[test]
    fn song_with_part_appends_in_order() {
        let s = Song::new("S", "C", "A")
            .with_part("Verse", vec![Chord::new("C", ChordQuality::Major)])
            .with_part("Chorus", vec![Chord::new("G", ChordQuality::Major)]);
        assert_eq!(s.parts.len(), 2);
        assert_eq!(s.parts[0].name, "Verse");
        assert_eq!(s.parts[1].name, "Chorus");
    }

    #[test]
    fn song_part_new_has_no_chords() {
        let p = SongPart::new("Bridge");
        assert!(p.items.is_empty());
    }

    // ── Transpose ────────────────────────────────────────────────────────────

    fn c_major_song() -> Song {
        Song::new("Test", "C Major", "Artist").with_part(
            "Verse",
            vec![
                Chord::new("C", ChordQuality::Major),
                Chord::new("G", ChordQuality::Major),
                Chord::new("A", ChordQuality::Minor),
                Chord::new("F", ChordQuality::Major),
            ],
        )
    }

    #[test]
    fn transpose_c_to_g_major() {
        let mut song = c_major_song();
        song.transpose_to("G");
        let roots: Vec<&str> = song.parts[0].chords().map(|c| c.root.as_str()).collect();
        assert_eq!(roots, ["G", "D", "E", "C"]);
    }

    #[test]
    fn transpose_c_to_f_uses_flats() {
        let mut song = c_major_song();
        song.transpose_to("F");
        let roots: Vec<&str> = song.parts[0].chords().map(|c| c.root.as_str()).collect();
        assert_eq!(roots, ["F", "C", "D", "Bb"]);
    }

    #[test]
    fn transpose_updates_key_field() {
        let mut song = c_major_song();
        song.transpose_to("G");
        assert_eq!(song.key, "G Major");
    }

    #[test]
    fn transpose_preserves_qualities() {
        let mut song = c_major_song();
        song.transpose_to("G");
        let qualities: Vec<&ChordQuality> = song.parts[0].chords().map(|c| &c.quality).collect();
        assert_eq!(
            qualities,
            [
                &ChordQuality::Major,
                &ChordQuality::Major,
                &ChordQuality::Minor,
                &ChordQuality::Major
            ]
        );
    }

    #[test]
    fn transpose_invalid_root_is_noop() {
        let mut song = c_major_song();
        song.transpose_to("Z"); // not a valid note
        assert_eq!(
            song.parts[0].chords().next().map(|c| c.root.as_str()),
            Some("C")
        ); // unchanged
        assert_eq!(song.key, "C Major"); // unchanged
    }

    #[test]
    fn transpose_c_to_bb_uses_flats() {
        let mut song = c_major_song();
        song.transpose_to("Bb");
        let roots: Vec<&str> = song.parts[0].chords().map(|c| c.root.as_str()).collect();
        assert_eq!(roots, ["Bb", "F", "G", "Eb"]);
    }

    #[test]
    fn transpose_c_to_fsharp_uses_sharps() {
        let mut song = c_major_song();
        song.transpose_to("F#");
        let roots: Vec<&str> = song.parts[0].chords().map(|c| c.root.as_str()).collect();
        assert_eq!(roots, ["F#", "C#", "D#", "B"]);
    }

    #[test]
    fn transpose_same_key_is_noop() {
        let mut song = c_major_song();
        song.transpose_to("C");
        let roots: Vec<&str> = song.parts[0].chords().map(|c| c.root.as_str()).collect();
        assert_eq!(roots, ["C", "G", "A", "F"]);
        assert_eq!(song.key, "C Major");
    }

    #[test]
    fn capo_zero_is_noop() {
        let song = c_major_song();
        let result = song.apply_capo(0);
        assert_eq!(result.key, "C Major");
        let roots: Vec<&str> = result.parts[0].chords().map(|c| c.root.as_str()).collect();
        assert_eq!(roots, ["C", "G", "A", "F"]);
    }

    #[test]
    fn capo_2_shifts_roots_down_two_semitones() {
        // Original key C, capo 2 → play shapes in Bb Major
        let song = c_major_song();
        let result = song.apply_capo(2);
        assert_eq!(result.key, "Bb Major");
        let roots: Vec<&str> = result.parts[0].chords().map(|c| c.root.as_str()).collect();
        // C→Bb, G→F, A→G, F→Eb
        assert_eq!(roots, ["Bb", "F", "G", "Eb"]);
    }

    #[test]
    fn capo_does_not_mutate_original() {
        let song = c_major_song();
        let _shifted = song.apply_capo(5);
        // Original unchanged
        assert_eq!(song.key, "C Major");
        assert_eq!(
            song.parts[0].chords().next().map(|c| c.root.as_str()),
            Some("C")
        );
    }

    // ── ChordQuality::label ──────────────────────────────────────────────────

    #[test]
    fn chord_quality_labels_are_human_readable() {
        assert_eq!(ChordQuality::Major.label(), "Major");
        assert_eq!(ChordQuality::Minor.label(), "Minor");
        assert_eq!(ChordQuality::Dominant7.label(), "Dom 7");
        assert_eq!(ChordQuality::Major7.label(), "Maj 7");
        assert_eq!(ChordQuality::Minor7.label(), "Min 7");
        assert_eq!(ChordQuality::Diminished.label(), "Dim");
        assert_eq!(ChordQuality::Augmented.label(), "Aug");
        assert_eq!(ChordQuality::Sus2.label(), "Sus 2");
        assert_eq!(ChordQuality::Sus4.label(), "Sus 4");
    }

    // ── Notation / apply_notation ────────────────────────────────────────────

    #[test]
    fn apply_notation_english_is_identity() {
        for note in &["C", "D", "E", "F", "G", "A", "B", "Bb", "F#"] {
            assert_eq!(apply_notation(note, Notation::English), *note);
        }
    }

    #[test]
    fn apply_notation_german_renames_b_to_h() {
        assert_eq!(apply_notation("B", Notation::German), "H");
    }

    #[test]
    fn apply_notation_german_renames_bb_to_b() {
        assert_eq!(apply_notation("Bb", Notation::German), "B");
    }

    #[test]
    fn apply_notation_german_leaves_other_notes_unchanged() {
        assert_eq!(apply_notation("C", Notation::German), "C");
        assert_eq!(apply_notation("F#", Notation::German), "F#");
        assert_eq!(apply_notation("Eb", Notation::German), "Eb");
    }

    #[test]
    fn apply_notation_custom_renames_b_to_h_but_keeps_bb() {
        assert_eq!(apply_notation("B", Notation::Custom), "H");
        assert_eq!(apply_notation("Bb", Notation::Custom), "Bb");
    }

    // ── Chord::display_with_notation ─────────────────────────────────────────

    #[test]
    fn chord_display_with_notation_english() {
        let c = Chord::new("B", ChordQuality::Minor);
        assert_eq!(c.display_with_notation(Notation::English), "Bm");
    }

    #[test]
    fn chord_display_with_notation_german_b_becomes_h() {
        let c = Chord::new("B", ChordQuality::Minor);
        assert_eq!(c.display_with_notation(Notation::German), "Hm");
    }

    #[test]
    fn chord_display_with_notation_bass_note_also_converted() {
        let mut c = Chord::new("B", ChordQuality::Major);
        c.bass_note = Some("Bb".to_string());
        // German: B → H, Bb → B
        assert_eq!(c.display_with_notation(Notation::German), "H/B");
    }

    // ── Chord with bass note ──────────────────────────────────────────────────

    #[test]
    fn chord_display_with_bass_note() {
        let mut c = Chord::new("G", ChordQuality::Major);
        c.bass_note = Some("B".to_string());
        assert_eq!(c.display(), "G/B");
    }

    #[test]
    fn chord_display_minor_with_bass_note() {
        let mut c = Chord::new("D", ChordQuality::Minor);
        c.bass_note = Some("F".to_string());
        assert_eq!(c.display(), "Dm/F");
    }

    // ── SongPart::new_riff / new_bass_riff ────────────────────────────────────

    #[test]
    fn song_part_new_riff_has_riff_kind() {
        let p = SongPart::new_riff("Intro Riff");
        assert_eq!(p.kind, PartKind::Riff);
        assert_eq!(p.name, "Intro Riff");
        assert!(p.tab_grid.is_empty());
    }

    #[test]
    fn song_part_new_bass_riff_has_bass_riff_kind() {
        let p = SongPart::new_bass_riff("Bass Line");
        assert_eq!(p.kind, PartKind::BassRiff);
        assert_eq!(p.name, "Bass Line");
    }

    // ── SongPart::tab_as_ascii ────────────────────────────────────────────────

    #[test]
    fn tab_as_ascii_falls_back_to_tab_string_when_grid_empty() {
        let mut p = SongPart::new_riff("Riff");
        p.tab = "e|--0--|".to_string();
        assert_eq!(p.tab_as_ascii(), "e|--0--|");
    }

    #[test]
    fn tab_as_ascii_single_open_string_col() {
        let mut p = SongPart::new_riff("Riff");
        // One column: open on every string (TabCell::Fret(0))
        p.tab_grid = vec![TabCol::Notes([
            TabCell::Fret(0),
            TabCell::Fret(0),
            TabCell::Fret(0),
            TabCell::Fret(0),
            TabCell::Fret(0),
            TabCell::Fret(0),
            TabCell::Empty,
            TabCell::Empty,
        ])];
        let ascii = p.tab_as_ascii();
        assert!(
            ascii.contains("e|-0-|"),
            "expected e string row, got: {ascii}"
        );
        assert!(
            ascii.contains("E|-0-|"),
            "expected E string row, got: {ascii}"
        );
    }

    #[test]
    fn tab_as_ascii_muted_string_shows_x() {
        let mut p = SongPart::new_riff("Riff");
        let mut cells: [TabCell; 8] = std::array::from_fn(|_| TabCell::Empty);
        cells[0] = TabCell::Muted; // high e muted
        p.tab_grid = vec![TabCol::Notes(cells)];
        let ascii = p.tab_as_ascii();
        assert!(ascii.contains("e|-x-|"), "expected muted e, got: {ascii}");
    }

    #[test]
    fn tab_as_ascii_barline_inserts_bar_marker() {
        let mut p = SongPart::new_riff("Riff");
        let col = TabCol::Notes(std::array::from_fn(|_| TabCell::Empty));
        p.tab_grid = vec![col.clone(), TabCol::Barline, col];
        let ascii = p.tab_as_ascii();
        // Each row should contain a mid-bar "|"
        let e_row = ascii.lines().next().unwrap_or("");
        assert!(
            e_row.matches('|').count() >= 3,
            "expected barline in row: {e_row}"
        );
    }

    #[test]
    fn tab_as_ascii_empty_grid_returns_empty_string() {
        let p = SongPart::new_riff("Riff");
        assert_eq!(p.tab_as_ascii(), "");
    }

    #[test]
    fn tab_as_ascii_bass_riff_uses_four_strings() {
        let mut p = SongPart::new_bass_riff("Bass");
        p.tab_grid = vec![TabCol::Notes(std::array::from_fn(|_| TabCell::Empty))];
        let ascii = p.tab_as_ascii();
        let rows: Vec<&str> = ascii.lines().collect();
        assert_eq!(rows.len(), 4, "bass riff should have 4 string rows");
        assert!(rows[0].starts_with("G|"), "first row should be G string");
        assert!(rows[3].starts_with("E|"), "last row should be E string");
    }

    // ── prefer_sharps_for_key ─────────────────────────────────────────────────

    #[test]
    fn prefer_sharps_for_major_sharp_keys() {
        for key in &["C", "G", "D", "A", "E", "B", "F#", "C#"] {
            assert!(
                prefer_sharps_for_key(key, false),
                "{key} major should prefer sharps"
            );
        }
    }

    #[test]
    fn prefer_flats_for_major_flat_keys() {
        for key in &["F", "Bb", "Eb", "Ab", "Db", "Gb"] {
            assert!(
                !prefer_sharps_for_key(key, false),
                "{key} major should prefer flats"
            );
        }
    }

    #[test]
    fn prefer_sharps_for_minor_sharp_keys() {
        for key in &["A", "E", "B", "F#", "C#", "G#", "D#"] {
            assert!(
                prefer_sharps_for_key(key, true),
                "{key} minor should prefer sharps"
            );
        }
    }

    // ── shift_note_up / shift_note ────────────────────────────────────────────

    #[test]
    fn shift_note_up_c_by_7_is_g() {
        assert_eq!(shift_note_up("C", 7, false), "G");
    }

    #[test]
    fn shift_note_up_wraps_around_octave() {
        assert_eq!(shift_note_up("B", 1, false), "C");
    }

    #[test]
    fn shift_note_up_zero_is_identity() {
        assert_eq!(shift_note_up("F#", 0, false), "F#");
    }

    #[test]
    fn shift_note_up_unknown_root_returns_unchanged() {
        assert_eq!(shift_note_up("Z", 3, false), "Z");
    }

    #[test]
    fn shift_note_down_c_by_2_is_bb() {
        assert_eq!(shift_note("C", 2, false), "Bb");
    }

    #[test]
    fn shift_note_down_zero_is_identity() {
        assert_eq!(shift_note("G#", 0, false), "G#");
    }

    #[test]
    fn shift_note_down_unknown_root_returns_unchanged() {
        assert_eq!(shift_note("Z", 5, false), "Z");
    }

    // ── Instrument ────────────────────────────────────────────────────────────

    #[test]
    fn instrument_labels_are_distinct_and_nonempty() {
        let labels: Vec<&str> = Instrument::all().iter().map(|i| i.label()).collect();
        for label in &labels {
            assert!(!label.is_empty());
        }
        // All labels should be unique
        let mut unique = labels.clone();
        unique.dedup();
        // dedup only removes consecutive, so sort first
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            labels.len(),
            "instrument labels must be unique"
        );
    }

    #[test]
    fn instrument_from_label_round_trips() {
        for inst in Instrument::all() {
            let label = inst.label();
            assert_eq!(
                Instrument::from_label(label),
                Some(inst),
                "from_label({label:?}) should return {inst:?}"
            );
        }
    }

    #[test]
    fn instrument_from_label_unknown_returns_none() {
        assert_eq!(Instrument::from_label("Banjo"), None);
        assert_eq!(Instrument::from_label(""), None);
    }

    #[test]
    fn instrument_accent_colors_are_nonempty_hex() {
        for inst in Instrument::all() {
            let color = inst.accent_color();
            assert!(
                color.starts_with('#'),
                "{inst:?} accent color should start with #, got {color}"
            );
            assert_eq!(
                color.len(),
                7,
                "{inst:?} color should be 7 chars (#{:06x}), got {color}",
                0
            );
        }
    }

    #[test]
    fn instrument_all_returns_five_instruments() {
        assert_eq!(Instrument::all().len(), 5);
    }

    // ── transpose_to with instrument_parts overrides ──────────────────────────

    #[test]
    fn transpose_also_shifts_instrument_parts_overrides() {
        let mut song = c_major_song();
        // Add a guitar override part
        song.instrument_parts.insert(
            "Electric".to_string(),
            vec![SongPart {
                name: "Verse".to_string(),
                kind: PartKind::Chords,
                tab: String::new(),
                tab_grid: Vec::new(),
                items: vec![
                    PartItem::Chord(Chord::new("C", ChordQuality::Major)),
                    PartItem::Chord(Chord::new("F", ChordQuality::Major)),
                ],
                part_text: None,
            }],
        );
        song.transpose_to("G");
        let overrides = &song.instrument_parts["Electric"];
        let roots: Vec<&str> = overrides[0].chords().map(|c| c.root.as_str()).collect();
        assert_eq!(roots, ["G", "C"]);
    }

    // ── Song serde round-trip ─────────────────────────────────────────────────

    #[test]
    fn song_json_round_trip() {
        let song = c_major_song();
        let json = serde_json::to_string(&song).expect("serialize");
        let back: Song = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(song, back);
    }

    #[test]
    fn song_with_instruments_and_vocals_round_trip() {
        let mut song = c_major_song();
        song.instruments = vec![Instrument::Guitar, Instrument::Drums];
        song.vocals_notes = "La la la".to_string();
        let json = serde_json::to_string(&song).expect("serialize");
        let back: Song = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.instruments,
            vec![Instrument::Guitar, Instrument::Drums]
        );
        assert_eq!(back.vocals_notes, "La la la");
    }

    #[test]
    fn part_item_repeat_and_volta_serialize() {
        let part = SongPart {
            name: "Chorus".to_string(),
            kind: PartKind::Chords,
            tab: String::new(),
            tab_grid: Vec::new(),
            items: vec![
                PartItem::Chord(Chord::new("G", ChordQuality::Major)),
                PartItem::Repeat { times: 2 },
                PartItem::VoltaBracketStart {
                    label: "1.".to_string(),
                },
                PartItem::Chord(Chord::new("C", ChordQuality::Major)),
                PartItem::VoltaBracketEnd,
            ],
            part_text: None,
        };
        let json = serde_json::to_string(&part).expect("serialize");
        let back: SongPart = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(part, back);
    }
}
