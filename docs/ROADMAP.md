# Roadmap

A realistic plan for a small project. Phases are ordered by value over effort;
within a phase, items are ordered by priority. Effort: S ≈ a day, M ≈ a few days, L ≈ 1–2 weeks.
Current state is described in FEATURES.md.

## Phase 0 — Housekeeping (1–2 days)

Fix things that mislead contributors and users before adding features.

| Item | Why | Effort |
|---|---|---|
| Update README: remove scale degrees / Chord-Degree toggle, document tabs, instruments, capo, notation, localStorage vs server, real run/build commands, licence | README describes an older app | S |
| Decide on the desktop story: either add a desktop binary/feature or drop the claim | `cargo run` as documented does not match `Cargo.toml` (only `web` and `server` bins) | S |
| Add CI (GitHub Actions): `cargo fmt --check`, `cargo clippy`, `cargo test` (native, for `song`/`pdf`/`auth`), and a server build | The Dockerfile deploys straight from main and there is no CI at all; commit #39 was a deploy-compat fix | S |
| Remove or document the leftover `Db::save_pdf` stub and the `#[allow(dead_code)]` bits in `TabEditor` | Dead code in the storage layer will confuse the API migration | S |

## Phase 1 — Make the data durable (2–3 weeks)

The biggest gap: the server exists, but the app stores songs and password hashes
in `localStorage`, so clearing the browser loses everything and accounts are not real accounts.

1. **Wire the frontend to `/api`** (login, register, list/load/save/delete). Keep a
   local-only "guest" mode. L
2. **One-time import of existing localStorage songs** into the account on first
   login, so current users lose nothing. M
3. **JSON import/export** of a song. Cheap, works without a server, and is the safety
   net while migrating. S
4. **Auth flows**: password reset, resend verification, logout everywhere/token handling in the client. M
5. **Server tests** (register → verify → login → CRUD, ownership checks) and a
   documented backup for the SQLite volume on Fly. M

## Phase 2 — Bands and setlists (3–5 weeks)

Requested features: multiple bands, importable setlists, one combined PDF per
setlist, per-setlist key changes, and a per-member instrument choice per song
with a default instrument. This needs accounts on the server, so it depends on Phase 1.

### Data model (sketch)

```
Band          { id, name, owner_id }
BandMember    { band_id, user_id, role, default_instrument }
Setlist       { id, band_id, name, date?, entries: Vec<SetlistEntry> }
SetlistEntry  { song_id, position, key_override: Option<String> }
MemberChoice  { setlist_id, entry position/song_id, user_id, instrument: Option<Instrument> }
```

- **Songs stay the single source of truth.** A setlist entry only *references* a song and
  stores a `key_override`; it never edits the song. This is what makes key changes
  "remain only for that setlist". The song is cloned and `Song::transpose_to` is applied
  at render time (that function already shifts instrument parts too).
- **Instrument choice is per user, not per setlist.** `MemberChoice` is keyed by user, so
  each member picks their own instrument per song. If none is chosen, fall back to the
  member's `default_instrument` (the band-level default), then to the base sheet.
  `Instrument` already exists and sheets already exist per instrument.
- Songs belong to a user today; decide whether band songs are shared within the band
  (recommended: a song has an optional `band_id`, visible to all members) or copied per member.
- New server tables and routes: `/bands`, `/bands/{id}/members`, `/bands/{id}/setlists`,
  `/setlists/{id}`. Membership checks on every route. Invites by email reuse the existing
  email/verification plumbing. M–L

### Steps

1. **Bands**: create/rename/delete, invite and remove members, roles (owner/member),
   default instrument per member. M
2. **Setlists**: create, add/remove/reorder songs from the band's songs. S–M
3. **Setlist import**: JSON first, CSV second. JSON is the full-fidelity format (a versioned
   export of a setlist: name, entries with song title/artist or id, key, notes), and the same
   format is used for export so setlists can be backed up and moved between bands. CSV is a
   simple table (`title,artist,key`, header row required) for lists made in a spreadsheet.
   Both go through the same matching step: entries are matched to the band's songs by id,
   then by title+artist; unmatched rows show on a review screen (pick a song, create a
   placeholder, or skip). Imported keys become `key_override`s. M
4. **Key overview**: one table for the setlist: song, original key, setlist key (editable
   dropdown or ±semitone buttons), reset button. Saves only the override. S–M
5. **Instrument selection**: per-song instrument dropdown for the logged-in member in the
   setlist view, prefilled from the default. S
6. **Combined PDF (per member)**: every member exports their own PDF, built from their own
   instrument selections for that setlist (see the decisions below). `pdf::generate_pdf_bytes` takes a single song and builds one document.
   Refactor into a function that appends a song to an existing document (page numbering,
   title page/table of contents optional), then loop over entries: clone the song, apply
   the key override, pick the member's instrument sheet and capo/PDF font settings, render.
   The exporting member's choices are used, so two members get different PDFs from the same setlist. Page breaks between songs; stay within `printpdf` 0.6. Add tests for page count and
   for the overrides. M–L
7. **Tests** for override isolation (editing a setlist key leaves the song unchanged),
   membership permissions and the import parser. M

### Decisions

- **Import formats:** JSON (primary) and CSV. Export uses the same JSON.
- **Combined PDF:** each member exports a PDF with *their own* instrument selections; no
  per-instrument bundle is needed. A band-wide "export for everyone" can come later.
- **Editing:** all band members can edit band songs. Consequences to plan for: add
  `updated_at`/`version` to songs and reject a save made against a stale version (show a
  "someone else changed this, reload or merge" message) so two members don't silently
  overwrite each other; optionally keep a short revision history so a bad edit can be
  undone. Real-time co-editing stays out of scope. M
- **Key overrides apply to every instrument sheet**; piano's own transpose and guitar capo are
  applied on top.

## Phase 3 — Faster, safer editing (2–3 weeks)

1. Undo/redo for the song signal (snapshot stack). M
2. Reorder and duplicate parts and chords. M
3. Free-text chord input that parses to `Chord` (root, quality, slash bass), with
   more qualities (6, 9, add9, m7b5, sus4/7 …). `ChordQuality` is a closed enum used in
   serialized data, so add variants with serde defaults and update PDF rendering. M
4. Autosave with a visible "saved" indicator. S
5. Library search/sort. S

## Phase 4 — Better sheets (3–4 weeks)

1. Chords over lyrics (aligned to syllables), the format most players expect. L
2. Chord diagrams for guitar and piano in the PDF. M
3. Page options: size (A4/Letter), margins, columns, font choice. M
4. Tempo, time signature, bar counts in the header. S
5. Stage/performance view (large text, dark mode, auto-scroll). M

## Phase 5 — iPad/iOS and Android app, with a live performance mode (4–8 weeks)

Goal: an installable app for iPad, iPhone and Android, including a mode that shows the
rendered sheet while still letting the player change instrument or transpose on the spot.
Depends on Phase 1 (accounts/sync) and Phase 2 (setlists); the live mode itself can be
built earlier on the web app.

### Live performance mode (build this first, on the web)

The app already has a rendered view that matches the PDF layout, a per-instrument sheet
with its own capo/transpose, and `Song::transpose_to`/`apply_capo`. Live mode combines them:

- Full-screen rendered sheet, large text, optional dark theme, screen kept awake.
- A compact toolbar: **instrument switcher** (the six instruments), **transpose ±1 semitone** with
  the resulting key shown, and capo (piano shows transpose instead, as today).
- Changes are **temporary by default** (live only, reset when leaving the song). Optional
  buttons: "keep for this setlist" (writes the setlist's `key_override`) and "keep as my
  instrument for this song" (writes the member's choice).
- Setlist navigation: next/previous song, jump list, swipe or large tap targets.
- Touch-friendly tab/drum rendering that fits the screen width, or horizontal scroll.
- Must work offline: the setlist and its songs are cached on the device. M–L

### Packaging as an app

The current frontend is a Dioxus WASM web app that relies on browser APIs (`localStorage`,
Blob downloads via `web-sys`). Options, cheapest first:

1. **Installable PWA** (manifest, service worker, offline cache). Works on iPad, iPhone and
   Android from the browser right away, no store needed. Limits on iOS: storage can be
   evicted and some APIs are restricted. S–M
2. **Native shell around the web build** (for example Capacitor, or Tauri 2 mobile) published to
   the App Store and Google Play. Reuses the existing WASM frontend and `web-sys` code, and
   adds file sharing for PDFs, background storage, and screen-wake. M–L
3. **Dioxus native mobile** (`dx` targets for iOS/Android). Single Rust codebase, but the
   `web-sys` code (storage, downloads, PDF blob) would have to be ported to native APIs and
   the target is less mature. Not recommended unless there is a reason to leave the web build.

Recommendation: ship the PWA first, then wrap the same build with option 2 if store presence
or reliable offline storage is needed. I have not built or tested any of these here; check
the current state of each toolchain before committing.

Also needed: Apple Developer (annual fee) and Google Play (one-time fee) accounts, app icons and
splash screens (SheetWave logo exists in `assets/`), a privacy policy, and a tested PDF
share/save flow on iOS (downloads work differently from desktop browsers).

## Phase 6 — Reach and sharing (open-ended)

- Read-only share links and setlists (needs Phase 1; setlist sharing is covered in Phase 2).
- ChordPro import/export. M
- Mobile layout polish for phones (the iPad/Android app is Phase 5). M
- Accessibility and i18n (German UI first). M
- Real-time collaboration, MusicXML/MIDI, audio playback: only if there is demand.

## Risks and notes

- `main.rs` is ~4,500 lines of inline-styled components with no tests. Before the
  Phase 2–4 UI work, split it into modules (`library`, `song_view`, `tab_editor`,
  `pdf_preview`) and move shared styles out; do it incrementally alongside Phase 1.
- Song JSON is the contract between localStorage, SQLite and PDF export. New fields
  must keep `#[serde(default)]` (as `pdf_settings` did) so old songs still load.
- The Dockerfile and `fly.toml` run a single 256 MB machine with a SQLite volume;
  that is fine for now, but don't add multi-machine scaling without moving off SQLite.
