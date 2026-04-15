/// SQLite persistence layer for Chord Shifter.
///
/// Schema
/// ──────
/// songs (id INTEGER PK, name TEXT, artist TEXT, key TEXT, parts_json TEXT)
/// pdfs  (id INTEGER PK, song_id INTEGER FK → songs.id, created_at TEXT, data BLOB)
use rusqlite::{params, Connection, Result};

use crate::song::Song;

// ── Database handle ───────────────────────────────────────────────────────────

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open (or create) the database at `path` and run migrations.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// In-memory database – useful for tests.
    #[allow(dead_code)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS songs (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                name       TEXT    NOT NULL,
                artist     TEXT    NOT NULL,
                key        TEXT    NOT NULL,
                parts_json TEXT    NOT NULL,
                UNIQUE(name, artist)
            );

            CREATE TABLE IF NOT EXISTS pdfs (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                song_id    INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
                created_at TEXT    NOT NULL DEFAULT (datetime('now')),
                data       BLOB    NOT NULL
            );
            ",
        )
    }
}

// ── Song row ──────────────────────────────────────────────────────────────────

/// Lightweight summary row returned by `list_songs`.
#[derive(Debug, Clone)]
pub struct SongRow {
    pub id: i64,
    pub name: String,
    pub artist: String,
    #[allow(dead_code)]
    pub key: String,
}

impl Db {
    /// Persist a song; returns the new row id.
    /// If a song with the same name + artist already exists it is **updated**.
    pub fn save_song(&self, song: &Song) -> Result<i64> {
        let parts_json = serde_json::to_string(&song.parts).expect("Song is always serialisable");

        // Upsert by (name, artist)
        self.conn.execute(
            "INSERT INTO songs (name, artist, key, parts_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name, artist) DO UPDATE SET
                 key        = excluded.key,
                 parts_json = excluded.parts_json",
            params![song.name, song.artist, song.key, parts_json],
        )?;

        let id: i64 = self.conn.query_row(
            "SELECT id FROM songs WHERE name = ?1 AND artist = ?2",
            params![song.name, song.artist],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    /// List all songs (id, name, artist, key) without loading parts.
    pub fn list_songs(&self) -> Result<Vec<SongRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, artist, key FROM songs ORDER BY name")?;

        let rows = stmt
            .query_map([], |row| {
                Ok(SongRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    artist: row.get(2)?,
                    key: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(rows)
    }

    /// Load the full `Song` for a given id.
    pub fn load_song(&self, id: i64) -> Result<Song> {
        self.conn.query_row(
            "SELECT name, artist, key, parts_json FROM songs WHERE id = ?1",
            params![id],
            |row| {
                let name: String = row.get(0)?;
                let artist: String = row.get(1)?;
                let key: String = row.get(2)?;
                let parts_json: String = row.get(3)?;

                let parts = serde_json::from_str(&parts_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok(Song {
                    name,
                    artist,
                    key,
                    parts,
                })
            },
        )
    }

    /// Delete a song (and its PDFs via CASCADE).
    pub fn delete_song(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM songs WHERE id = ?1", params![id])?;
        Ok(())
    }
}

// ── PDF blobs ─────────────────────────────────────────────────────────────────

/// Metadata row for a stored PDF.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PdfRow {
    pub id: i64,
    pub song_id: i64,
    pub created_at: String,
}

impl Db {
    /// Store a PDF blob for a given song; returns the new PDF row id.
    pub fn save_pdf(&self, song_id: i64, data: &[u8]) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO pdfs (song_id, data) VALUES (?1, ?2)",
            params![song_id, data],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// List all PDF metadata rows for a song (most recent first).
    #[allow(dead_code)]
    pub fn list_pdfs(&self, song_id: i64) -> Result<Vec<PdfRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, song_id, created_at FROM pdfs
             WHERE song_id = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt
            .query_map(params![song_id], |row| {
                Ok(PdfRow {
                    id: row.get(0)?,
                    song_id: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(rows)
    }

    /// Load the raw bytes of a stored PDF by its row id.
    #[allow(dead_code)]
    pub fn load_pdf(&self, id: i64) -> Result<Vec<u8>> {
        self.conn
            .query_row("SELECT data FROM pdfs WHERE id = ?1", params![id], |row| {
                row.get(0)
            })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::{Chord, ChordQuality};

    fn sample_song() -> Song {
        Song::new("Hey Jude", "F Major", "The Beatles").with_part(
            "Verse",
            vec![
                Chord::new("F", ChordQuality::Major).with_degree(1),
                Chord::new("C", ChordQuality::Major).with_degree(5),
            ],
        )
    }

    #[test]
    fn save_and_load_song_round_trips() {
        let db = Db::open_in_memory().unwrap();
        let song = sample_song();
        let id = db.save_song(&song).unwrap();
        let loaded = db.load_song(id).unwrap();
        assert_eq!(loaded, song);
    }

    #[test]
    fn list_songs_returns_saved_row() {
        let db = Db::open_in_memory().unwrap();
        db.save_song(&sample_song()).unwrap();
        let rows = db.list_songs().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Hey Jude");
    }

    #[test]
    fn save_song_twice_upserts() {
        let db = Db::open_in_memory().unwrap();
        let mut song = sample_song();
        db.save_song(&song).unwrap();
        song.key = "G Major".to_string();
        db.save_song(&song).unwrap();
        let rows = db.list_songs().unwrap();
        assert_eq!(rows.len(), 1); // still one row
        let loaded = db.load_song(rows[0].id).unwrap();
        assert_eq!(loaded.key, "G Major");
    }

    #[test]
    fn delete_song_removes_it() {
        let db = Db::open_in_memory().unwrap();
        let id = db.save_song(&sample_song()).unwrap();
        db.delete_song(id).unwrap();
        assert!(db.list_songs().unwrap().is_empty());
    }

    #[test]
    fn save_and_load_pdf_round_trips() {
        let db = Db::open_in_memory().unwrap();
        let song_id = db.save_song(&sample_song()).unwrap();
        let data = b"fake-pdf-bytes";
        let pdf_id = db.save_pdf(song_id, data).unwrap();
        let loaded = db.load_pdf(pdf_id).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn list_pdfs_returns_metadata() {
        let db = Db::open_in_memory().unwrap();
        let song_id = db.save_song(&sample_song()).unwrap();
        db.save_pdf(song_id, b"pdf1").unwrap();
        db.save_pdf(song_id, b"pdf2").unwrap();
        let rows = db.list_pdfs(song_id).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn delete_song_cascades_to_pdfs() {
        let db = Db::open_in_memory().unwrap();
        let song_id = db.save_song(&sample_song()).unwrap();
        db.save_pdf(song_id, b"pdf").unwrap();
        db.delete_song(song_id).unwrap();
        // pdf rows should be gone too
        assert!(db.list_pdfs(song_id).unwrap().is_empty());
    }
}
