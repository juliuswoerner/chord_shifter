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
3. **Setlist import**: this needs a decision on the source (see open questions). Start with
   a plain-text/CSV list of titles (optionally `title;key`) matched against the library,
   with a review screen for unmatched lines, and JSON import of a whole setlist.
   Imported keys become `key_override`s. M
4. **Key overview**: one table for the setlist: song, original key, setlist key (editable
   dropdown or ±semitone buttons), reset button. Saves only the override. S–M
5. **Instrument selection**: per-song instrument dropdown for the logged-in member in the
   setlist view, prefilled from the default. S
6. **Combined PDF**: `pdf::generate_pdf_bytes` takes a single song and builds one document.
   Refactor into a function that appends a song to an existing document (page numbering,
   title page/table of contents optional), then loop over entries: clone the song, apply
   the key override, pick the member's instrument sheet and capo/PDF font settings, render.
   Page breaks between songs; stay within `printpdf` 0.6. Add tests for page count and
   for the overrides. M–L
7. **Tests** for override isolation (editing a setlist key leaves the song unchanged),
   membership permissions and the import parser. M

### Open questions

- What format should setlist import accept? (plain text, CSV, JSON, another app's export?)
- Should a setlist be one PDF per instrument/member, or only the PDF for the person
  exporting? (Plan above: the exporting member's instrument choices.)
- Can band members edit shared songs, or only the owner?
- Do key overrides apply to every instrument sheet? (Plan above: yes; piano's own
  transpose and guitar capo are applied on top.)

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

## Phase 5 — Reach and sharing (open-ended)

- Read-only share links and setlists (needs Phase 1; setlist sharing is covered in Phase 2).
- ChordPro import/export. M
- Mobile layout and PWA/offline. M–L
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
