# Chord Shifter

A cross-platform chord sheet editor built with [Dioxus](https://dioxuslabs.com/). Create, edit, and export chord progressions as PDF — runs as a native desktop app or in the browser.

## Features

- **Free-form song structure** — add any number of named parts (Verse, Chorus, Bridge, or anything you like)
- **Interactive chord editor** — edit root note, quality, and scale degree per chord
- **Chord / Degree toggle** — switch the view between written chord names (`Am`, `Fmaj7`) and roman-numeral scale degrees (`VIm`, `IVmaj7`)
- **Transpose** — shift all chords to a new key in one click; degrees are preserved
- **PDF export** — exports the chord sheet with superscript quality notation; respects the current Chords / Degrees view and configurable font sizes
- **Desktop + Web** — same codebase runs natively via Dioxus desktop or in the browser via WASM

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started) (`cargo install dioxus-cli`)

### Run as desktop app

```bash
cargo run
```

### Run in the browser

```bash
dx serve --platform web
```

## Project Structure

```
src/
  main.rs   # Dioxus UI components and app entry point
  song.rs   # Data model: Song, SongPart, Chord, ChordQuality, ScaleDegree
  pdf.rs    # PDF generation (printpdf)
```

## Data Model

```rust
Song {
    name, key, artist,
    parts: Vec<SongPart>,
}

SongPart {
    name: String,
    chords: Vec<Chord>,
}

Chord {
    root:    String,          // e.g. "A", "F#"
    quality: ChordQuality,    // Major, Minor, Maj7, …
    degree:  Option<ScaleDegree>, // 1–7 relative to the song key
}
```

`ScaleDegree` stores a value 1–7 and renders as a Roman numeral (`I`–`VII`).

## Chord Qualities

| Symbol | Name |
|--------|------|
| _(none)_ | Major |
| `m` | Minor |
| `7` | Dominant 7 |
| `maj7` | Major 7 |
| `m7` | Minor 7 |
| `dim` | Diminished |
| `aug` | Augmented |
| `sus2` | Suspended 2 |
| `sus4` | Suspended 4 |

## License

MIT
