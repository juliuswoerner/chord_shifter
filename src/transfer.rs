// Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved.

//! Moving songs in and out of the app as JSON files (backup, sharing between accounts).
//!
//! A file is an envelope `{"format": "chord-shifter", "version": 1, "song": {...}}` for one
//! song or `{"format": "chord-shifter", "version": 1, "songs": [...]}` for several. A bare
//! song object (the shape the server API returns) is accepted on import too.
//!
//! Pure functions with no web or server dependencies, so they are tested natively.

use crate::song::Song;

/// Value of the `format` field.
pub const FORMAT: &str = "chord-shifter";
/// Current file version. Files with a higher version are rejected rather than half-read.
pub const VERSION: u32 = 1;
/// Largest file accepted on import.
pub const MAX_BYTES: usize = 5 * 1024 * 1024;
/// Most songs accepted from one file.
pub const MAX_SONGS: usize = 500;
/// Most parts accepted in one song (base sheet or any instrument sheet).
pub const MAX_PARTS: usize = 500;

/// Export one song as a pretty-printed JSON file body.
pub fn export_song(song: &Song) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "format": FORMAT,
        "version": VERSION,
        "song": song,
    }))
    .unwrap_or_default()
}

/// Export several songs (a backup of the library) as one JSON file body.
pub fn export_songs(songs: &[Song]) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "format": FORMAT,
        "version": VERSION,
        "songs": songs,
    }))
    .unwrap_or_default()
}

/// Like [`export_songs`], but refuses a backup that [`import_songs`] would reject
/// (too many songs or too large), so a "successful" export is always restorable.
pub fn export_songs_checked(songs: &[Song]) -> Result<String, String> {
    if songs.len() > MAX_SONGS {
        return Err(format!(
            "Your library has {} songs; a backup file can hold at most {MAX_SONGS}. Export the songs individually instead.",
            songs.len()
        ));
    }
    let text = export_songs(songs);
    if text.len() > MAX_BYTES {
        return Err(format!(
            "Your library is too large for one backup file ({} MB, the limit is {} MB). Export the songs individually instead.",
            text.len() / (1024 * 1024) + 1,
            MAX_BYTES / (1024 * 1024)
        ));
    }
    Ok(text)
}

/// Parse and validate the contents of an import file.
pub fn import_songs(text: &str) -> Result<Vec<Song>, String> {
    if text.len() > MAX_BYTES {
        return Err(format!(
            "The file is too large ({} MB, the limit is {} MB).",
            text.len() / (1024 * 1024),
            MAX_BYTES / (1024 * 1024)
        ));
    }
    let value: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|_| "This is not a valid JSON file.".to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "Expected a song file, but found a different JSON value.".to_string())?;

    let raw: Vec<serde_json::Value> = if object.contains_key("format") {
        if object["format"] != FORMAT {
            return Err("This file was not exported by Chord Shifter.".to_string());
        }
        match object.get("version").and_then(|v| v.as_u64()) {
            Some(v) if v <= VERSION as u64 => {}
            Some(v) => {
                return Err(format!(
                    "This file uses a newer format (version {v}); please update the app."
                ))
            }
            None => return Err("The file has no valid version number.".to_string()),
        }
        if let Some(song) = object.get("song") {
            vec![song.clone()]
        } else if let Some(songs) = object.get("songs").and_then(|s| s.as_array()) {
            songs.clone()
        } else {
            return Err("The file contains no songs.".to_string());
        }
    } else if object.contains_key("parts") && object.contains_key("name") {
        vec![value.clone()] // a bare song, as returned by the server API
    } else {
        return Err("This does not look like a Chord Shifter song file.".to_string());
    };

    if raw.is_empty() {
        return Err("The file contains no songs.".to_string());
    }
    if raw.len() > MAX_SONGS {
        return Err(format!("The file has too many songs (limit {MAX_SONGS})."));
    }

    let mut songs = Vec::with_capacity(raw.len());
    for (i, item) in raw.into_iter().enumerate() {
        let mut song: Song = serde_json::from_value(item)
            .map_err(|e| format!("Song {} could not be read: {e}", i + 1))?;
        song.name = song.name.trim().to_string();
        if song.name.is_empty() {
            song.name = "Untitled song".to_string();
        }
        song.artist = song.artist.trim().to_string();
        let too_many = song.parts.len() > MAX_PARTS
            || song.instrument_parts.values().any(|p| p.len() > MAX_PARTS);
        if too_many {
            return Err(format!(
                "\"{}\" has too many parts (limit {MAX_PARTS}).",
                song.name
            ));
        }
        songs.push(song);
    }
    Ok(songs)
}

/// A file name for a download: letters, digits, spaces, dashes and underscores only.
pub fn safe_filename(name: &str, fallback: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('_').to_string();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned.chars().take(80).collect()
    }
}

/// A title that does not collide with an existing (title, artist) pair: the title itself,
/// else "Title (2)", "Title (3)", … Imports never overwrite existing songs.
pub fn unique_name(name: &str, artist: &str, existing: &[(String, String)]) -> String {
    let taken = |n: &str| existing.iter().any(|(en, ea)| en == n && ea == artist);
    if !taken(name) {
        return name.to_string();
    }
    (2..)
        .map(|n| format!("{name} ({n})"))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::demo_songs;

    #[test]
    fn a_single_song_round_trips_with_every_field() {
        for song in demo_songs() {
            let text = export_song(&song);
            let back = import_songs(&text).unwrap();
            assert_eq!(back, vec![song]);
        }
    }

    #[test]
    fn a_bundle_round_trips_in_order() {
        let songs = demo_songs();
        let back = import_songs(&export_songs(&songs)).unwrap();
        assert_eq!(back, songs);
    }

    #[test]
    fn a_backup_that_import_would_refuse_is_not_exported() {
        let song = demo_songs().remove(0);
        let too_many = vec![song.clone(); MAX_SONGS + 1];
        assert!(export_songs_checked(&too_many)
            .unwrap_err()
            .contains("at most"));
        // Under the limits the checked export equals the plain one and imports back.
        let few = demo_songs();
        let text = export_songs_checked(&few).unwrap();
        assert_eq!(text, export_songs(&few));
        assert_eq!(import_songs(&text).unwrap(), few);
        // Too large by size.
        let mut fat = song;
        fat.vocals_notes = "x".repeat(MAX_BYTES / 4);
        let fat_library = vec![fat; 5];
        assert!(export_songs_checked(&fat_library)
            .unwrap_err()
            .contains("too large"));
    }

    #[test]
    fn the_envelope_declares_format_and_version() {
        let v: serde_json::Value = serde_json::from_str(&export_song(&demo_songs()[0])).unwrap();
        assert_eq!(v["format"], FORMAT);
        assert_eq!(v["version"], VERSION);
    }

    #[test]
    fn a_bare_song_as_returned_by_the_api_is_accepted() {
        let song = demo_songs().remove(1);
        let text = serde_json::to_string(&song).unwrap();
        assert_eq!(import_songs(&text).unwrap(), vec![song]);
    }

    #[test]
    fn a_byte_order_mark_is_ignored() {
        let text = format!("\u{feff}{}", export_song(&demo_songs()[0]));
        assert!(import_songs(&text).is_ok());
    }

    #[test]
    fn garbage_and_foreign_files_are_rejected_with_a_clear_message() {
        assert_eq!(
            import_songs("not json").unwrap_err(),
            "This is not a valid JSON file."
        );
        assert!(import_songs("[1,2,3]")
            .unwrap_err()
            .contains("different JSON value"));
        assert!(import_songs(r#"{"hello":"world"}"#)
            .unwrap_err()
            .contains("does not look like"));
        assert!(import_songs(r#"{"format":"other","version":1,"song":{}}"#)
            .unwrap_err()
            .contains("not exported by Chord Shifter"));
        assert!(import_songs(r#"{"format":"chord-shifter","version":1}"#)
            .unwrap_err()
            .contains("no songs"));
        assert!(
            import_songs(r#"{"format":"chord-shifter","version":1,"songs":[]}"#)
                .unwrap_err()
                .contains("no songs")
        );
    }

    #[test]
    fn newer_or_missing_versions_are_rejected() {
        let newer = format!(
            r#"{{"format":"chord-shifter","version":{},"songs":[]}}"#,
            VERSION + 1
        );
        assert!(import_songs(&newer).unwrap_err().contains("newer format"));
        assert!(import_songs(r#"{"format":"chord-shifter","songs":[]}"#)
            .unwrap_err()
            .contains("version"));
    }

    #[test]
    fn a_broken_song_names_its_position() {
        let text = format!(
            r#"{{"format":"chord-shifter","version":1,"songs":[{},{{"name":"x"}}]}}"#,
            serde_json::to_string(&demo_songs()[0]).unwrap()
        );
        assert!(import_songs(&text)
            .unwrap_err()
            .starts_with("Song 2 could not be read"));
    }

    #[test]
    fn oversized_files_and_songs_are_rejected() {
        let big = " ".repeat(MAX_BYTES + 1);
        assert!(import_songs(&big).unwrap_err().contains("too large"));
        let mut song = demo_songs().remove(0);
        let part = song.parts[0].clone();
        song.parts = vec![part; MAX_PARTS + 1];
        assert!(import_songs(&export_song(&song))
            .unwrap_err()
            .contains("too many parts"));
    }

    #[test]
    fn names_are_trimmed_and_blank_names_get_a_placeholder() {
        let mut song = demo_songs().remove(0);
        song.name = "   ".to_string();
        song.artist = "  Someone ".to_string();
        let back = import_songs(&export_song(&song)).unwrap();
        assert_eq!(back[0].name, "Untitled song");
        assert_eq!(back[0].artist, "Someone");
    }

    #[test]
    fn file_names_are_made_safe() {
        assert_eq!(safe_filename("Let It Be", "song"), "Let It Be");
        assert_eq!(safe_filename("a/b\\c:d*?", "song"), "a_b_c_d");
        assert_eq!(safe_filename("../../etc/passwd", "song"), "etc_passwd");
        assert_eq!(safe_filename("???", "song"), "song");
        assert_eq!(safe_filename(&"x".repeat(200), "song").len(), 80);
    }

    #[test]
    fn duplicates_get_a_numbered_title_and_never_overwrite() {
        let existing = vec![
            ("Tune".to_string(), "Me".to_string()),
            ("Tune (2)".to_string(), "Me".to_string()),
        ];
        assert_eq!(unique_name("Tune", "Me", &existing), "Tune (3)");
        assert_eq!(unique_name("Tune", "Someone else", &existing), "Tune");
        assert_eq!(unique_name("Fresh", "Me", &existing), "Fresh");
    }
}
