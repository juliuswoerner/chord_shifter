// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.

//! Sample data for the local demo environment: a demo user and a few songs that
//! exercise the main features (repeats and voltas, slash chords, tabs, drums,
//! per-instrument sheets, capo, lyrics, flat keys).
//!
//! Everything here is invented for demo purposes. The module has no web or
//! server dependencies so it can be used by both binaries and tested natively.

use crate::song::{
    Chord, ChordQuality as Q, Instrument, PartItem, PartKind, PartTextSettings, PdfSettings, Song,
    SongPart, TabCell, TabCol,
};

/// Login of the demo user. Shown on the login screen in demo mode.
pub const DEMO_EMAIL: &str = "demo@example.com";
/// Password of the demo user. Public by design: only use it in a local demo.
pub const DEMO_PASSWORD: &str = "Demo-Sheetwave-2026!";

fn c(root: &str, q: Q) -> PartItem {
    PartItem::Chord(Chord::new(root, q))
}

fn slash(root: &str, q: Q, bass: &str) -> PartItem {
    PartItem::Chord(Chord {
        root: root.to_string(),
        quality: q,
        bass_note: Some(bass.to_string()),
    })
}

fn chords_part(name: &str, items: Vec<PartItem>) -> SongPart {
    SongPart {
        items,
        ..SongPart::new(name)
    }
}

fn with_lyrics(mut part: SongPart, text: &str, show_chords: bool) -> SongPart {
    part.part_text = Some(PartTextSettings {
        content: text.to_string(),
        show_chords,
        ..PartTextSettings::new()
    });
    part
}

/// One tab column: `(string_index, cell)` pairs, every other string empty.
/// Index 0 is the high e string (guitar) — bass uses indices 2–5, drums all 8.
fn col(cells: &[(usize, TabCell)]) -> TabCol {
    let mut notes: [TabCell; 8] = Default::default();
    for (i, cell) in cells {
        notes[*i] = cell.clone();
    }
    TabCol::Notes(notes)
}

/// One column of a piano sheet: right-hand notes, left-hand notes and the note length
/// ("1" whole, "2" half, "4" quarter, "8" eighth). Notes are comma-separated.
fn piano_col(right: &str, left: &str, length: &str) -> TabCol {
    let mut notes: [TabCell; 8] = Default::default();
    if !right.is_empty() {
        notes[0] = TabCell::Custom(right.to_string());
    }
    if !left.is_empty() {
        notes[1] = TabCell::Custom(left.to_string());
    }
    notes[2] = TabCell::Custom(length.to_string());
    TabCol::Notes(notes)
}

fn riff(name: &str, kind: PartKind, grid: Vec<TabCol>) -> SongPart {
    SongPart {
        kind,
        tab_grid: grid,
        ..SongPart::new(name)
    }
}

fn sunrise_ballad() -> Song {
    use Q::*;
    let verse = with_lyrics(
        chords_part(
            "Verse",
            vec![
                c("C", Major),
                slash("G", Major, "B"),
                c("A", Minor),
                c("F", Major),
                PartItem::LineBreak,
                c("C", Major),
                c("G", Major),
                c("F", Major),
                c("C", Major),
            ],
        ),
        "First light on the window,\nthe kettle starts to sing.",
        true,
    );
    let chorus = chords_part(
        "Chorus",
        vec![
            PartItem::RepeatStart,
            c("F", Major7),
            c("C", Major),
            c("G", Sus4),
            c("G", Major),
            PartItem::Repeat { times: 2 },
            PartItem::LineBreak,
            PartItem::Text {
                content: "Last time: hold the final chord".to_string(),
                color: "#888888".to_string(),
            },
        ],
    );
    let outro = chords_part(
        "Outro",
        vec![
            PartItem::VoltaBracketStart {
                label: "1.".to_string(),
            },
            c("C", Major),
            c("G", Major),
            PartItem::VoltaBracketEnd,
            PartItem::VoltaBracketStart {
                label: "2.".to_string(),
            },
            c("F", Major),
            c("C", Major),
            PartItem::VoltaBracketEnd,
        ],
    );

    // Acoustic sheet: simpler progressions, stored at concert pitch like the base
    // sheet. The capo of 3 below is applied when the sheet is rendered, so the
    // player sees (and the PDF prints) the open shapes A E F#m D.
    let acoustic = vec![
        chords_part(
            "Verse",
            vec![c("C", Major), c("G", Major), c("A", Minor), c("F", Major)],
        ),
        chords_part(
            "Chorus",
            vec![c("F", Major7), c("C", Major), c("G", Sus4), c("G", Major)],
        ),
    ];

    let mut song = Song::new("Sunrise Ballad", "C Major", "Demo Band");
    song.parts = vec![verse, chorus, outro];
    song.instruments = vec![
        Instrument::AcousticGuitar,
        Instrument::Guitar,
        Instrument::Vocals,
    ];
    song.vocals_notes = "Verse softly, open up in the chorus.".to_string();
    song.instrument_parts
        .insert("Acoustic".to_string(), acoustic);
    song.instrument_capos.insert("Acoustic".to_string(), 3);
    song
}

fn midnight_groove() -> Song {
    use TabCell::{Custom, Fret, HammerOn, Muted};
    use Q::*;
    let chords = chords_part(
        "Groove",
        vec![
            c("A", Minor7),
            c("D", Minor7),
            c("G", Dominant7),
            c("C", Major7),
        ],
    );
    // Guitar: index 0 = high e … 5 = low E.
    let guitar = riff(
        "Main riff",
        PartKind::Riff,
        vec![
            col(&[(4, Fret(0))]),
            col(&[(4, Fret(0))]),
            col(&[(4, Fret(2)), (3, Fret(2))]),
            col(&[(3, Fret(2)), (2, Fret(2))]),
            TabCol::Barline,
            col(&[(4, Fret(0))]),
            col(&[(3, Fret(0)), (4, HammerOn)]),
            col(&[(3, Fret(2))]),
            col(&[(5, Muted)]),
            TabCol::LineBreak,
            col(&[(4, Fret(3))]),
            col(&[(3, Fret(5))]),
            TabCol::Barline,
        ],
    );
    // Bass: uses indices 2–5 (G, D, A, E).
    let bass = riff(
        "Bass line",
        PartKind::BassRiff,
        vec![
            col(&[(4, Fret(0))]),
            col(&[(4, Fret(0))]),
            col(&[(4, Fret(3))]),
            col(&[(5, Fret(3))]),
            TabCol::Barline,
            col(&[(5, Fret(3))]),
            col(&[(4, Fret(2))]),
            col(&[(4, Fret(0))]),
            col(&[(5, Fret(0))]),
            TabCol::Barline,
        ],
    );
    // Drums: K, S, Hi, R, C, T1, T2, T3 (all 8 cells).
    let hit = || Custom("X".to_string());
    let drums = riff(
        "Beat",
        PartKind::DrumBeat,
        vec![
            col(&[(0, hit()), (2, hit())]),
            col(&[(2, hit())]),
            col(&[(1, hit()), (2, hit())]),
            col(&[(2, hit())]),
            col(&[(0, hit()), (2, hit())]),
            col(&[(0, hit()), (2, hit())]),
            col(&[(1, hit()), (2, hit())]),
            col(&[(2, hit()), (4, hit())]),
            TabCol::Barline,
        ],
    );

    let mut song = Song::new("Midnight Groove", "A Minor", "Demo Band");
    song.parts = vec![chords, guitar];
    song.instruments = vec![Instrument::Guitar, Instrument::Bass, Instrument::Drums];
    song.instrument_parts.insert("Bass".to_string(), vec![bass]);
    song.instrument_parts
        .insert("Drums".to_string(), vec![drums]);
    song
}

fn piano_waltz() -> Song {
    use Q::*;
    let a = chords_part(
        "Theme",
        vec![
            c("Bb", Major),
            c("G", Minor7),
            c("Eb", Major),
            c("F", Dominant7),
            PartItem::LineBreak,
            c("Bb", Major),
            slash("D", Minor, "F"),
            c("C", Minor7),
            c("F", Dominant7),
        ],
    );
    let b = chords_part(
        "Bridge",
        vec![
            c("G", Minor),
            c("D", Diminished),
            c("Eb", Major),
            c("Bb", Major),
        ],
    );
    // The piano sheet: a melody over an oom-pah-pah left hand, written in B♭ major, 3/4.
    let mut melody = SongPart::new_piano_riff("Melody");
    melody.time_sig = "3/4".to_string();
    melody.tab_grid = vec![
        piano_col("D5", "Bb2", "4"),
        piano_col("F5", "D3, F3", "4"),
        piano_col("F5", "D3, F3", "4"),
        TabCol::Barline,
        piano_col("Eb5", "Eb3", "2"),
        piano_col("D5", "G3, Bb3", "4"),
        TabCol::Barline,
        piano_col("C5, F5", "F2", "4"),
        piano_col("A4", "A3, C4", "4"),
        piano_col("C5", "A3, C4", "4"),
        TabCol::Barline,
        piano_col("Bb4", "Bb2, F3", "2"),
        piano_col("D5", "D3", "8"),
        piano_col("C5", "D3", "8"),
        TabCol::Barline,
    ];
    let mut piano_sheet = vec![melody];
    piano_sheet.extend([a.clone(), b.clone()]);

    let mut song = Song::new("Piano Waltz", "Bb Major", "Demo Band");
    song.parts = vec![a, b];
    song.instrument_parts
        .insert("Piano".to_string(), piano_sheet);
    song.instruments = vec![Instrument::Piano];
    // The piano sheet is transposed two semitones up (+ is higher): B♭ major becomes C major.
    song.instrument_capos.insert("Piano".to_string(), 2);
    song
}

fn twelve_bar_blues() -> Song {
    use Q::*;
    let blues = chords_part(
        "12-bar",
        vec![
            c("E", Dominant7),
            c("E", Dominant7),
            c("E", Dominant7),
            c("E", Dominant7),
            PartItem::LineBreak,
            c("A", Dominant7),
            c("A", Dominant7),
            c("E", Dominant7),
            c("E", Dominant7),
            PartItem::LineBreak,
            c("B", Dominant7),
            c("A", Dominant7),
            c("E", Dominant7),
            c("B", Dominant7),
        ],
    );
    let mut song = Song::new("Twelve Bar Blues", "E Major", "Demo Band");
    song.parts = vec![blues];
    song.instruments = vec![Instrument::Guitar, Instrument::Bass];
    // Larger chords for reading from a distance.
    song.pdf_settings.insert(
        "Base".to_string(),
        PdfSettings {
            part_name_size: 10,
            chord_size: 22,
            part_gap: 0,
        },
    );
    song
}

/// The demo songs, in library order.
pub fn demo_songs() -> Vec<Song> {
    vec![
        sunrise_ballad(),
        midnight_groove(),
        piano_waltz(),
        twelve_bar_blues(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_several_distinct_songs() {
        let songs = demo_songs();
        assert!(songs.len() >= 4);
        let mut names: Vec<_> = songs.iter().map(|s| s.name.clone()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), songs.len());
    }

    #[test]
    fn songs_survive_a_json_round_trip() {
        for song in demo_songs() {
            let json = serde_json::to_string(&song).unwrap();
            let back: Song = serde_json::from_str(&json).unwrap();
            assert_eq!(song, back, "{}", song.name);
        }
    }

    #[test]
    fn demo_covers_the_main_features() {
        let songs = demo_songs();
        let parts = || songs.iter().flat_map(|s| s.parts.iter());
        let kinds = |k: PartKind| {
            songs
                .iter()
                .flat_map(|s| s.instrument_parts.values().flatten().chain(s.parts.iter()))
                .any(|p| p.kind == k)
        };
        assert!(kinds(PartKind::Riff));
        assert!(kinds(PartKind::BassRiff));
        assert!(kinds(PartKind::DrumBeat));
        assert!(kinds(PartKind::PianoRiff));
        assert!(parts().any(|p| p
            .items
            .iter()
            .any(|i| matches!(i, PartItem::VoltaBracketStart { .. }))));
        assert!(parts().any(|p| p.items.iter().any(|i| matches!(i, PartItem::Repeat { .. }))));
        assert!(parts().any(|p| p.part_text.is_some()));
        assert!(songs.iter().any(|s| !s.instrument_capos.is_empty()));
        assert!(songs.iter().any(|s| s
            .parts
            .iter()
            .flat_map(|p| p.chords())
            .any(|c| c.bass_note.is_some())));
    }

    #[test]
    fn transposing_a_demo_song_keeps_its_structure() {
        let mut song = sunrise_ballad();
        let before = song.parts.len();
        song.transpose_to("G");
        assert_eq!(song.key.split_whitespace().next(), Some("G"));
        assert_eq!(song.parts.len(), before);
    }

    #[test]
    fn acoustic_sheet_is_at_concert_pitch_and_gets_open_shapes_from_the_capo() {
        let song = sunrise_ballad();
        let acoustic = &song.instrument_parts["Acoustic"];
        let first = |s: &SongPart| s.chords().next().unwrap().root.clone();
        // Same chords as the base sheet; the capo (applied at render time) does the shifting.
        assert_eq!(first(&acoustic[0]), first(&song.parts[0]));
        let capo = song.instrument_capos["Acoustic"];
        let mut shaped = song.clone();
        shaped.parts = acoustic.clone();
        let shaped = shaped.apply_capo(capo);
        assert_eq!(first(&shaped.parts[0]), "A");
    }

    #[test]
    fn the_demo_piano_sheet_is_transposed_up_by_its_stored_shift() {
        use crate::song::TabCell;
        let song = piano_waltz();
        let shift = Instrument::Piano.shift(song.instrument_capos["Piano"]);
        assert_eq!(shift, crate::song::Shift::Transpose(2)); // + is higher
        let sheet = Song {
            parts: song.instrument_parts["Piano"].clone(),
            ..song.clone()
        };
        let shifted = sheet.apply_capo(shift.as_capo());
        assert_eq!(shifted.key, "C Major"); // B-flat major, two semitones up
        let first_right_hand = |s: &Song| match &s.parts[0].tab_grid[0] {
            TabCol::Notes(cells) => match &cells[0] {
                TabCell::Custom(text) => text.clone(),
                other => panic!("unexpected cell {other:?}"),
            },
            other => panic!("unexpected column {other:?}"),
        };
        assert_eq!(first_right_hand(&sheet), "D5");
        assert_eq!(first_right_hand(&shifted), "E5"); // higher, not lower
    }
}
