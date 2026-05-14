use std::collections::HashMap;

// ── Chord quality ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
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
    #[allow(dead_code)]
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

// ── Part kind ─────────────────────────────────────────────────────────────────

/// Whether a song part holds chords or a guitar-tab riff.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum PartKind {
    #[default]
    Chords,
    /// Free-form guitar tab (stored as a plain string).
    Riff,
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
    /// Start of a volta bracket, e.g. "1." or "2.".
    VoltaBracketStart { label: String },
    /// End of a volta bracket.
    VoltaBracketEnd,
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
    /// All items in this part in order — only used when `kind == PartKind::Chords`.
    #[serde(alias = "chords")]
    pub items: Vec<PartItem>,
}

impl SongPart {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: PartKind::Chords,
            tab: String::new(),
            items: Vec::new(),
        }
    }

    pub fn new_riff(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: PartKind::Riff,
            tab: String::new(),
            items: Vec::new(),
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

    /// Iterate over only the `Chord` items in this part.
    #[allow(dead_code)]
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
}

impl Instrument {
    pub fn label(self) -> &'static str {
        match self {
            Instrument::Guitar => "Electric",
            Instrument::AcousticGuitar => "Acoustic",
            Instrument::Bass => "Bass",
            Instrument::Piano => "Piano",
            Instrument::Drums => "Drums",
        }
    }

    /// Accent colour used on the instrument sheet page.
    #[allow(dead_code)]
    pub fn accent_color(self) -> &'static str {
        match self {
            Instrument::Guitar => "#1a5c38",
            Instrument::AcousticGuitar => "#7c4a00",
            Instrument::Bass => "#1a2e5c",
            Instrument::Piano => "#4a1a6e",
            Instrument::Drums => "#7c1a1a",
        }
    }

    /// Parse from the label string (used for URL routing).
    #[allow(dead_code)]
    pub fn from_label(s: &str) -> Option<Instrument> {
        match s {
            "Electric" => Some(Instrument::Guitar),
            "Acoustic" => Some(Instrument::AcousticGuitar),
            "Bass" => Some(Instrument::Bass),
            "Piano" => Some(Instrument::Piano),
            "Drums" => Some(Instrument::Drums),
            _ => None,
        }
    }

    pub fn all() -> [Instrument; 5] {
        [
            Instrument::Guitar,
            Instrument::AcousticGuitar,
            Instrument::Bass,
            Instrument::Piano,
            Instrument::Drums,
        ]
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
    /// Per-instrument capo settings. Key = Instrument::label().
    #[serde(default)]
    pub instrument_capos: HashMap<String, u8>,
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
        }
    }

    /// Builder-style helper – appends a new named part with the given chords.
    pub fn with_part(mut self, name: impl Into<String>, chords: Vec<Chord>) -> Self {
        self.parts.push(SongPart {
            name: name.into(),
            kind: PartKind::Chords,
            tab: String::new(),
            items: chords.into_iter().map(PartItem::Chord).collect(),
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

    /// Return a copy of this song with every chord root shifted **down** by
    /// `capo` semitones — the shapes you need to play when you place a capo
    /// on fret `capo` to sound in the original key.
    ///
    /// When `capo == 0` returns an unchanged clone.
    pub fn apply_capo(&self, capo: u8) -> Song {
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
                chord.root = shift_note(&chord.root, capo, is_minor);
                if let Some(bass) = &chord.bass_note {
                    chord.bass_note = Some(shift_note(bass, capo, is_minor));
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
}
