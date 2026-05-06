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

// ── Scale degree ──────────────────────────────────────────────────────────────

/// Scale degree (1–7) of a chord relative to the song's key.
///
/// Examples in C Major:
///   C → 1 (I),  D → 2 (II),  E → 3 (III),  F → 4 (IV),
///   G → 5 (V),  A → 6 (VI),  B → 7 (VII)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct ScaleDegree(pub u8);

#[allow(dead_code)]
impl ScaleDegree {
    /// Returns `Some(ScaleDegree)` for values 1–7, `None` otherwise.
    pub fn new(degree: u8) -> Option<Self> {
        if (1..=7).contains(&degree) {
            Some(Self(degree))
        } else {
            None
        }
    }

    /// The raw degree number (1–7).
    pub fn get(self) -> u8 {
        self.0
    }

    /// Upper-case Roman numeral, e.g. `"IV"`.
    pub fn roman(self) -> &'static str {
        match self.0 {
            1 => "I",
            2 => "II",
            3 => "III",
            4 => "IV",
            5 => "V",
            6 => "VI",
            7 => "VII",
            _ => "?",
        }
    }
}

impl std::fmt::Display for ScaleDegree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.roman())
    }
}

// ── Chord ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Chord {
    pub root: String,
    pub quality: ChordQuality,
    /// Scale degree of this chord relative to the song's key (1–7).
    /// `None` means the degree hasn't been set yet.
    pub degree: Option<ScaleDegree>,
}

impl Chord {
    /// Create a chord without a scale degree assigned.
    pub fn new(root: impl Into<String>, quality: ChordQuality) -> Self {
        Self {
            root: root.into(),
            quality,
            degree: None,
        }
    }

    /// Builder helper – assign a scale degree (1–7) to the chord.
    #[allow(dead_code)]
    pub fn with_degree(mut self, degree: u8) -> Self {
        self.degree = ScaleDegree::new(degree);
        self
    }

    /// Human-readable chord name, e.g. `"Am"`, `"G7"`, `"Fmaj7"`.
    pub fn display(&self) -> String {
        format!("{}{}", self.root, self.quality.symbol())
    }

    /// Scale-degree display: roman numeral + quality symbol (e.g. `"IVm"`, `"Imaj7"`).
    /// Falls back to `display()` if no degree has been assigned yet.
    pub fn degree_display(&self) -> String {
        match self.degree {
            Some(d) => format!("{}{}", d.roman(), self.quality.symbol()),
            None => self.display(),
        }
    }
}

// ── Song part ─────────────────────────────────────────────────────────────────

/// A named section of a song (e.g. "Verse", "Chorus", or anything the user chooses).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SongPart {
    pub name: String,
    pub chords: Vec<Chord>,
}

impl SongPart {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            chords: Vec::new(),
        }
    }
}

// ── Music-theory helpers ────────────────────────────────────────────────────

/// Semitone offsets of scale degrees 1–7 in a major scale.
const MAJOR_INTERVALS: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

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

/// Shift a note root down by `semitones` chromatically.
/// Chooses sharps or flats based on which side of the circle the result falls on.
/// Returns the original string unchanged if it cannot be parsed.
pub fn shift_note(root: &str, semitones_down: u8) -> String {
    if semitones_down == 0 {
        return root.to_string();
    }
    note_to_index(root)
        .map(|idx| {
            let new_idx = ((idx as i16) - (semitones_down as i16)).rem_euclid(12) as u8;
            let prefer_sharps = matches!(
                index_to_note(new_idx, true),
                "C" | "G" | "D" | "A" | "E" | "B" | "F#" | "C#"
            );
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
    pub fn icon(self) -> &'static str {
        match self {
            Instrument::Guitar => "\u{1F3B8}",
            Instrument::AcousticGuitar => "\u{1FA95}",
            Instrument::Bass => "\u{1F3B8}",
            Instrument::Piano => "\u{1F3B9}",
            Instrument::Drums => "\u{1F941}",
        }
    }

    pub fn icon_path(self) -> &'static str {
        match self {
            Instrument::Guitar => "/assets/icons/electric.png",
            Instrument::AcousticGuitar => "/assets/icons/acoustic.png",
            Instrument::Bass => "/assets/icons/bass.png",
            Instrument::Piano => "/assets/icons/piano.png",
            Instrument::Drums => "/assets/icons/drums.png",
        }
    }

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
            chords,
        });
        self
    }

    /// Transpose all chords to a new key root (e.g. `"G"`, `"Bb"`, `"F#"`).
    ///
    /// Only chords that have a `degree` assigned are moved; chords without a
    /// degree are left unchanged.  The song's `key` field is updated to
    /// `"<new_root> <mode>"`, preserving whatever mode suffix was there before.
    pub fn transpose_to(&mut self, new_root: &str) {
        let root_idx = match note_to_index(new_root) {
            Some(i) => i,
            None => return,
        };
        let prefer_sharps = matches!(new_root, "C" | "G" | "D" | "A" | "E" | "B" | "F#" | "C#");
        for part in &mut self.parts {
            for chord in &mut part.chords {
                if let Some(degree) = chord.degree {
                    let d = degree.get() as usize;
                    if (1..=7).contains(&d) {
                        let semitones = MAJOR_INTERVALS[d - 1];
                        let note_idx = (root_idx + semitones) % 12;
                        chord.root = index_to_note(note_idx, prefer_sharps).to_string();
                    }
                }
            }
        }
        // Preserve mode suffix ("Major", "Minor", …) from the old key string.
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
        // Also transpose instrument-specific overrides.
        for parts in self.instrument_parts.values_mut() {
            for part in parts.iter_mut() {
                for chord in &mut part.chords {
                    if let Some(degree) = chord.degree {
                        let d = degree.get() as usize;
                        if (1..=7).contains(&d) {
                            let semitones = MAJOR_INTERVALS[d - 1];
                            let note_idx = (root_idx + semitones) % 12;
                            chord.root = index_to_note(note_idx, prefer_sharps).to_string();
                        }
                    }
                }
            }
        }
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
        // Shift every chord root.
        for part in &mut result.parts {
            for chord in &mut part.chords {
                chord.root = shift_note(&chord.root, capo);
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

    // ── ScaleDegree ──────────────────────────────────────────────────────────

    #[test]
    fn scale_degree_valid_range() {
        for d in 1u8..=7 {
            assert!(ScaleDegree::new(d).is_some());
        }
    }

    #[test]
    fn scale_degree_out_of_range() {
        assert!(ScaleDegree::new(0).is_none());
        assert!(ScaleDegree::new(8).is_none());
    }

    #[test]
    fn scale_degree_roman_numerals() {
        let expected = ["I", "II", "III", "IV", "V", "VI", "VII"];
        for (i, &roman) in expected.iter().enumerate() {
            assert_eq!(ScaleDegree(i as u8 + 1).roman(), roman);
        }
    }

    #[test]
    fn scale_degree_display_uses_roman() {
        assert_eq!(format!("{}", ScaleDegree(4)), "IV");
    }

    #[test]
    fn scale_degree_get_returns_raw_value() {
        assert_eq!(ScaleDegree(5).get(), 5);
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

    #[test]
    fn chord_new_has_no_degree() {
        let c = Chord::new("C", ChordQuality::Major);
        assert!(c.degree.is_none());
    }

    #[test]
    fn chord_with_degree_sets_degree() {
        let c = Chord::new("C", ChordQuality::Major).with_degree(1);
        assert_eq!(c.degree, Some(ScaleDegree(1)));
    }

    #[test]
    fn chord_with_degree_out_of_range_leaves_none() {
        let c = Chord::new("C", ChordQuality::Major).with_degree(0);
        assert!(c.degree.is_none());
    }

    #[test]
    fn chord_degree_display_with_degree() {
        let c = Chord::new("A", ChordQuality::Minor).with_degree(6);
        assert_eq!(c.degree_display(), "VIm");
    }

    #[test]
    fn chord_degree_display_falls_back_without_degree() {
        let c = Chord::new("A", ChordQuality::Minor);
        assert_eq!(c.degree_display(), "Am");
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
        assert!(p.chords.is_empty());
    }

    // ── Transpose ────────────────────────────────────────────────────────────

    fn c_major_song() -> Song {
        Song::new("Test", "C Major", "Artist").with_part(
            "Verse",
            vec![
                Chord::new("C", ChordQuality::Major).with_degree(1),
                Chord::new("G", ChordQuality::Major).with_degree(5),
                Chord::new("A", ChordQuality::Minor).with_degree(6),
                Chord::new("F", ChordQuality::Major).with_degree(4),
            ],
        )
    }

    #[test]
    fn transpose_c_to_g_major() {
        let mut song = c_major_song();
        song.transpose_to("G");
        let roots: Vec<&str> = song.parts[0]
            .chords
            .iter()
            .map(|c| c.root.as_str())
            .collect();
        assert_eq!(roots, ["G", "D", "E", "C"]);
    }

    #[test]
    fn transpose_c_to_f_uses_flats() {
        let mut song = c_major_song();
        song.transpose_to("F");
        let roots: Vec<&str> = song.parts[0]
            .chords
            .iter()
            .map(|c| c.root.as_str())
            .collect();
        // F major: I=F, V=C, VI=D, IV=Bb
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
        let qualities: Vec<&ChordQuality> =
            song.parts[0].chords.iter().map(|c| &c.quality).collect();
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
    fn transpose_skips_chords_without_degree() {
        let mut song = Song::new("S", "C Major", "A").with_part(
            "Verse",
            vec![Chord::new("C", ChordQuality::Major)], // no degree
        );
        song.transpose_to("G");
        // root should be unchanged because no degree was set
        assert_eq!(song.parts[0].chords[0].root, "C");
    }

    #[test]
    fn transpose_invalid_root_is_noop() {
        let mut song = c_major_song();
        song.transpose_to("Z"); // not a valid note
        assert_eq!(song.parts[0].chords[0].root, "C"); // unchanged
        assert_eq!(song.key, "C Major"); // unchanged
    }

    #[test]
    fn transpose_c_to_bb_uses_flats() {
        let mut song = c_major_song();
        song.transpose_to("Bb");
        // Bb major: I=Bb, V=F, VI=G, IV=Eb
        let roots: Vec<&str> = song.parts[0]
            .chords
            .iter()
            .map(|c| c.root.as_str())
            .collect();
        assert_eq!(roots, ["Bb", "F", "G", "Eb"]);
    }

    #[test]
    fn transpose_c_to_fsharp_uses_sharps() {
        let mut song = c_major_song();
        song.transpose_to("F#");
        // F# major: I=F#, V=C#, VI=D#, IV=B
        let roots: Vec<&str> = song.parts[0]
            .chords
            .iter()
            .map(|c| c.root.as_str())
            .collect();
        assert_eq!(roots, ["F#", "C#", "D#", "B"]);
    }

    #[test]
    fn transpose_same_key_is_noop() {
        // Transposing to the same root should leave all chord roots unchanged.
        // This verifies that the interval calculation wraps correctly at 0 semitones.
        let mut song = c_major_song();
        song.transpose_to("C");
        let roots: Vec<&str> = song.parts[0]
            .chords
            .iter()
            .map(|c| c.root.as_str())
            .collect();
        assert_eq!(roots, ["C", "G", "A", "F"]);
        assert_eq!(song.key, "C Major");
    }

    #[test]
    fn capo_zero_is_noop() {
        let song = c_major_song();
        let result = song.apply_capo(0);
        assert_eq!(result.key, "C Major");
        let roots: Vec<&str> = result.parts[0]
            .chords
            .iter()
            .map(|c| c.root.as_str())
            .collect();
        assert_eq!(roots, ["C", "G", "A", "F"]);
    }

    #[test]
    fn capo_2_shifts_roots_down_two_semitones() {
        // Original key C, capo 2 → play shapes in Bb Major
        let song = c_major_song();
        let result = song.apply_capo(2);
        assert_eq!(result.key, "Bb Major");
        let roots: Vec<&str> = result.parts[0]
            .chords
            .iter()
            .map(|c| c.root.as_str())
            .collect();
        // C→Bb, G→F, A→G, F→Eb
        assert_eq!(roots, ["Bb", "F", "G", "Eb"]);
    }

    #[test]
    fn capo_does_not_mutate_original() {
        let song = c_major_song();
        let _shifted = song.apply_capo(5);
        // Original unchanged
        assert_eq!(song.key, "C Major");
        assert_eq!(song.parts[0].chords[0].root, "C");
    }
}
