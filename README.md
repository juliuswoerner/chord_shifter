# Chord Shifter (SheetWave)

A chord sheet editor that runs in the browser (Rust + [Dioxus](https://dioxuslabs.com/), compiled to WebAssembly). Build songs from named parts, add guitar/bass tabs and drum beats, transpose, and export print-ready PDFs.

## Features

- **Free-form song structure** — any number of named parts (Verse, Chorus, Bridge, …). A part holds chords, a guitar riff, a bass riff or a drum beat.
- **Chord editor** — root note, nine qualities (major, minor, 7, maj7, m7, dim, aug, sus2, sus4) and optional slash-chord bass notes.
- **Part items** — line breaks, repeat bars (with optional count), volta brackets (1., 2., …) and coloured text annotations. Each part can also carry lyrics text, optionally shown next to its chords.
- **Transpose** — move the whole song to a new key; sharp/flat spelling follows the target key.
- **Note naming** — English (`B`, `B♭`), German (`H`, `B`) or a custom mix (`H`, `B♭`).
- **Instrument sheets** — Electric, Acoustic, Bass, Piano, Drums and Vocals. Each instrument can have its own copy of the parts and its own capo (piano uses a transpose of −12…+12 instead).
- **Tab editor** — click-to-edit grid for guitar (6 strings), bass (4) and drums (8 tracks) with frets, mutes, ghost notes, hammer-ons, pull-offs, bends, slides and free-form entries.
- **Rendered view and PDF preview** — a layout that matches the PDF.
- **PDF export** — multi-page, superscript chord quality, configurable chord and part-label font sizes per sheet. Export downloads a ZIP with one PDF per sheet.
- **Library and accounts** — register/log in and keep a list of songs. In the web app, **accounts and songs are stored in the browser's `localStorage`** (nothing is synced between devices).
- **Server (optional)** — an Axum + SQLite backend with email registration and verification, JWT login and a songs API. The web app does not call it yet.

## Getting started

### Prerequisites

- [Rust](https://rustup.rs/) (stable) with the WASM target: `rustup target add wasm32-unknown-unknown`
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/getting_started) 0.6: `cargo install dioxus-cli --version 0.6.3 --locked`

### Run in the browser

```bash
dx serve --platform web
```

### Run the unit tests

```bash
cargo test --lib --bin chord_shifter                                  # song, auth, pdf
cargo test --bin server --no-default-features --features server       # server
```

### Run the server

The server serves the built frontend from a `dist/` folder in its working directory, and the API under `/api`.

```bash
dx build --platform web --release
cargo build --bin server --no-default-features --features server --release
cp -r target/dx/chord_shifter/release/web/public dist     # same layout as the Dockerfile

DATABASE_URL=sqlite:chord_shifter.db?mode=rwc \
JWT_SECRET=$(openssl rand -hex 32) \
ENCRYPTION_KEY=$(openssl rand -hex 32) \
APP_URL=http://localhost:8080 \
./target/release/server
```

`JWT_SECRET` and `ENCRYPTION_KEY` must each be 64 hex characters (32 bytes); `PORT` defaults to 8080. Keep the `ENCRYPTION_KEY`: user emails are encrypted with it. When `APP_URL` is `localhost`/`127.0.0.1`, new users are verified automatically; for any other `APP_URL` the optional SMTP settings (`SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`) are required to send verification emails. The `Dockerfile` and `fly.toml` deploy the server and frontend to Fly.io. See the header of `src/bin/server.rs` for the API and security notes.

There is currently no native desktop build; the app targets the browser.

## Project structure

```
src/
  main.rs         # Dioxus UI (library, song editor, tab editor, instrument sheets) and localStorage backend
  song.rs         # Data model: Song, SongPart, PartItem, Chord, tabs, instruments, transpose and capo
  pdf.rs          # PDF generation (printpdf)
  auth.rs         # Argon2id password hashing (shared by frontend and server)
  lib.rs          # Exposes song and auth to both binaries
  bin/server.rs   # Axum + SQLite backend
assets/           # Logo and instrument icons
scripts/          # Licence-header helper and pre-commit hook
```

## Data model (simplified)

```rust
Song {
    name, key, artist,
    parts: Vec<SongPart>,
    instruments: Vec<Instrument>,
    instrument_parts: HashMap<String, Vec<SongPart>>,  // per-instrument overrides
    instrument_capos: HashMap<String, i8>,
    pdf_settings:     HashMap<String, PdfSettings>,    // "Base" or an instrument
    vocals_notes: String,
}

SongPart { name, kind: Chords | Riff | BassRiff | DrumBeat, items, tab_grid, part_text }
PartItem::{ Chord, LineBreak, Repeat, RepeatStart, VoltaBracketStart, VoltaBracketEnd, Text }
Chord    { root, quality: ChordQuality, bass_note: Option<String> }
```

## Contributing

`scripts/pre-commit.sh` adds the licence header to staged source files; install it with `cp scripts/pre-commit.sh .git/hooks/pre-commit`.

## License

Proprietary — Copyright © 2026 APSOS — App and Software Solutions Wörner. All rights reserved. See `LICENSE`.
