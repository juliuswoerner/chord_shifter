# Features

What Chord Shifter (branded **SheetWave** in the UI) does today, based on the
code in `src/` as of commit `ebdd33e`, plus ideas for what is missing.

> **Note:** parts of the README are out of date. There is no scale-degree /
> roman-numeral model or Chord/Degree toggle in the code any more, the README's
> `Chord` struct is outdated, and the licence is proprietary (APSOS), not MIT.
> See ROADMAP.md, Phase 0.

## Current features

### Song structure
- Songs have a name, artist and key (major/minor).
- Parts are named freely (Verse, Chorus, anything). Each part has one of four kinds:
  **Chords**, **Riff** (guitar tab), **Bass riff** (4-string tab) or **Drum beat** (8-track grid).
- Chord parts are an ordered list of items: chords, line breaks, repeat bars
  (`‖: … :‖`, optionally with a count), plain repeat-start double bars, volta
  brackets ("1.", "2."), and coloured free-text annotations.
- Each part can have lyrics/vocals text with colour and PDF font size, optionally
  shown next to the part's chords in a two-column layout.
- A song-level "Vocals / Notes" field.

### Chords and harmony
- Nine qualities: major, minor, 7, maj7, m7, dim, aug, sus2, sus4.
- Slash chords (optional bass note, e.g. `G/B`).
- Transpose the whole song to a new key. Sharp/flat spelling follows the target
  key, and instrument-specific parts are transposed too (unit-tested).
- Note-name notation: English (B / B♭), German (H / B) or Custom (H / B♭).

### Instrument sheets
- Six instruments: Electric, Acoustic, Bass, Piano, Drums, Vocals.
- Each instrument can have its own copy of the parts, so the sheets can diverge
  from the base sheet.
- Per-instrument capo (guitars and bass): chord shapes are shown shifted down.
  Piano gets a transpose of −12 to +12 semitones instead (capo range is 0–12). Drums have no capo.
- Each instrument has its own page (`/song/:id/instrument/:instrument`) with its
  own accent colour and icon.

### Tab editors
- Click-to-edit grid for guitar (6 strings), bass (4) and drums (8 tracks).
- Cell types: fret numbers (0–24), muted, ghost notes, hammer-on, pull-off,
  bend, release, slide up/down, plus free-form text such as `5h7`.
- Barlines and line breaks inside the grid; ASCII tab export helper in `song.rs`.

### Views and export
- Editing view and a "Rendered" view laid out like the PDF.
- Live PDF preview in the app.
- PDF export (client-side, `printpdf`): multi-page, superscript chord quality,
  tab and drum rendering, repeats/voltas, lyrics, notation and capo applied.
- Configurable chord and part-label font sizes, stored per sheet.
- "Export as PDF" downloads a ZIP with one PDF per sheet (base plus each instrument).

### Library, accounts and storage
- Landing page, register/login, "My Songs" list with instrument badges, create
  and delete. A "Let It Be" example song is seeded on first run.
- **The web app stores everything in browser `localStorage`**, including users
  (Argon2 password hashes) and songs. Nothing is synced between devices.
- Separate Axum + SQLite server (`src/bin/server.rs`, deployed on Fly.io):
  email registration with verification link (token expiry), login, JWT (30 days),
  songs CRUD under `/api`, AES-256-GCM email encryption at rest, Argon2id,
  per-IP rate limiting on auth routes, CORS locked to `APP_URL`. The server also
  serves the built frontend. **The frontend does not call these endpoints yet**
  (no HTTP client code in `main.rs`).

### Quality
- About 70 unit tests: `song.rs` (transposition, capo, notation, tabs, JSON
  round trips), `pdf.rs` (valid output for many layouts), `auth.rs`, and email
  validation in the server.
- No tests for the UI (`main.rs`, ~4,500 lines). No CI workflow in the repo.
  Pre-commit hook adds licence headers.

## Gaps and ideas

Grouped roughly by how much they would matter to users.

**Data and accounts**
- One-command local demo environment with a demo user and sample songs (ROADMAP Phase 1).
- Bands with members and a default instrument, setlists with per-setlist key overrides,
  setlist import, per-member instrument choice per song, and one combined setlist PDF
  (planned in ROADMAP Phase 3). Setlist import/export as JSON and CSV; all band members
  can edit shared songs, with a version check to avoid silent overwrites.
- Connect the frontend to the server API (cloud sync, multi-device).
- Import/export a song as JSON (backup, sharing); import from ChordPro or plain text.
- Share a read-only link; setlists; folders or tags; search and sort in the library.
- Password reset and resend-verification flow (the server only has verify).

**Editing**
- Undo/redo; duplicate/reorder/move parts and chords (drag and drop).
- More chord qualities: 6, 9, add9, m(maj7), 7sus4, m7b5, extensions, alterations.
- Free-text chord entry (type `F#m7/C#` instead of picking from dropdowns).
- Chords placed over lyric syllables (the classic chord-over-lyrics layout);
  today lyrics and chords are separate blocks.
- Time signature, tempo, section bar counts, rhythm/strumming notation.
- Tab: note lengths/rhythm, more than one bar-length template, copy bars.
- Auto-detect key; suggest capo positions with the easiest shapes.

**Output**
- Written song notes stored on the base song and printed in every PDF (ROADMAP Phase 5).
- Print-friendly layout settings: page size (A4 only now), margins, fonts, columns.
- Export formats beyond PDF: ChordPro, MusicXML/MIDI, plain text, image.
- Stage mode: large font, dark theme, auto-scroll, page-turn.
- Chord diagrams (guitar, piano) printed with the sheet.

**Platform**
- Installable app for iPad/iOS and Android (PWA first, then store packaging) with a live
  performance mode: rendered sheet with on-the-spot instrument switch and transpose,
  offline setlists (ROADMAP Phase 6).
- Desktop build: the README says `cargo run` runs a desktop app, but the manifest
  only defines the web and server binaries (see ROADMAP Phase 0).
- Accessibility pass (keyboard navigation in the grids, contrast, labels).
- i18n (the notation setting hints at a German-speaking audience; UI is English).
