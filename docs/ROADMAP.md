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

## Phase 2 — Faster, safer editing (2–3 weeks)

1. Undo/redo for the song signal (snapshot stack). M
2. Reorder and duplicate parts and chords. M
3. Free-text chord input that parses to `Chord` (root, quality, slash bass), with
   more qualities (6, 9, add9, m7b5, sus4/7 …). `ChordQuality` is a closed enum used in
   serialized data, so add variants with serde defaults and update PDF rendering. M
4. Autosave with a visible "saved" indicator. S
5. Library search/sort. S

## Phase 3 — Better sheets (3–4 weeks)

1. Chords over lyrics (aligned to syllables), the format most players expect. L
2. Chord diagrams for guitar and piano in the PDF. M
3. Page options: size (A4/Letter), margins, columns, font choice. M
4. Tempo, time signature, bar counts in the header. S
5. Stage/performance view (large text, dark mode, auto-scroll). M

## Phase 4 — Reach and sharing (open-ended)

- Read-only share links and setlists (needs Phase 1).
- ChordPro import/export. M
- Mobile layout and PWA/offline. M–L
- Accessibility and i18n (German UI first). M
- Real-time collaboration, MusicXML/MIDI, audio playback: only if there is demand.

## Risks and notes

- `main.rs` is ~4,500 lines of inline-styled components with no tests. Before the
  Phase 2–3 UI work, split it into modules (`library`, `song_view`, `tab_editor`,
  `pdf_preview`) and move shared styles out; do it incrementally alongside Phase 1.
- Song JSON is the contract between localStorage, SQLite and PDF export. New fields
  must keep `#[serde(default)]` (as `pdf_settings` did) so old songs still load.
- The Dockerfile and `fly.toml` run a single 256 MB machine with a SQLite volume;
  that is fine for now, but don't add multi-machine scaling without moving off SQLite.
