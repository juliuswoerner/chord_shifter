/// SQLite persistence layer for Chord Shifter.
///
/// Schema
/// ──────
/// users (id INTEGER PK, username TEXT UNIQUE, password_hash TEXT)
/// songs (id INTEGER PK, name TEXT, artist TEXT, key TEXT, parts_json TEXT,
///        instruments_json TEXT, vocals_notes TEXT, user_id INTEGER FK→users)
///        UNIQUE(name, artist, user_id)
/// pdfs  (id INTEGER PK, song_id INTEGER FK → songs.id, created_at TEXT, data BLOB)
use rusqlite::{params, Connection, Result};

use crate::auth;
use crate::song::{Instrument, Song};

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
        // Base schema for fresh databases.
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS users (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                username      TEXT    NOT NULL UNIQUE,
                password_hash TEXT    NOT NULL
            );

            CREATE TABLE IF NOT EXISTS songs (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                name             TEXT    NOT NULL,
                artist           TEXT    NOT NULL,
                key              TEXT    NOT NULL,
                parts_json       TEXT    NOT NULL,
                instruments_json TEXT    NOT NULL DEFAULT '[]',
                vocals_notes     TEXT    NOT NULL DEFAULT '',
                user_id          INTEGER REFERENCES users(id) ON DELETE SET NULL,
                UNIQUE(name, artist, user_id)
            );

            CREATE TABLE IF NOT EXISTS pdfs (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                song_id    INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
                created_at TEXT    NOT NULL DEFAULT (datetime('now')),
                data       BLOB    NOT NULL
            );
            ",
        )?;

        // If the existing on-disk songs table lacks user_id, rebuild it to add
        // the column and update the UNIQUE constraint.
        let has_user_id: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('songs') WHERE name='user_id'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_user_id {
            let has_instruments: bool = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('songs') WHERE name='instruments_json'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|c| c > 0)
                .unwrap_or(false);
            let has_vocals: bool = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('songs') WHERE name='vocals_notes'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|c| c > 0)
                .unwrap_or(false);

            let sel_instruments = if has_instruments {
                "instruments_json"
            } else {
                "'[]'"
            };
            let sel_vocals = if has_vocals { "vocals_notes" } else { "''" };

            self.conn.execute_batch(&format!(
                "
                CREATE TABLE songs_new (
                    id               INTEGER PRIMARY KEY AUTOINCREMENT,
                    name             TEXT    NOT NULL,
                    artist           TEXT    NOT NULL,
                    key              TEXT    NOT NULL,
                    parts_json       TEXT    NOT NULL,
                    instruments_json TEXT    NOT NULL DEFAULT '[]',
                    vocals_notes     TEXT    NOT NULL DEFAULT '',
                    user_id          INTEGER REFERENCES users(id) ON DELETE SET NULL,
                    UNIQUE(name, artist, user_id)
                );
                INSERT INTO songs_new
                    (id, name, artist, key, parts_json, instruments_json, vocals_notes, user_id)
                    SELECT id, name, artist, key, parts_json,
                        {sel_instruments}, {sel_vocals}, NULL
                    FROM songs;
                DROP TABLE songs;
                ALTER TABLE songs_new RENAME TO songs;
                ",
            ))?;
        }

        Ok(())
    }
}

// ── User ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct User {
    #[allow(dead_code)]
    pub id: i64,
    pub username: String,
}

impl Db {
    /// Create a new user. The password is hashed with Argon2id before storage.
    /// Returns the new user's id, or an error if the username is already taken.
    pub fn create_user(&self, username: &str, password: &str) -> Result<i64> {
        let hash = auth::hash_password(password).map_err(rusqlite::Error::InvalidParameterName)?;
        self.conn.execute(
            "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
            params![username, hash],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Verify credentials. Returns `Some(User)` on success, `None` on bad
    /// username or wrong password.
    pub fn verify_user(&self, username: &str, password: &str) -> Result<Option<User>> {
        let result = self.conn.query_row(
            "SELECT id, password_hash FROM users WHERE username = ?1",
            params![username],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        );
        match result {
            Ok((id, hash)) => {
                if auth::verify_password(password, &hash) {
                    Ok(Some(User {
                        id,
                        username: username.to_string(),
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Check if any users exist (used to show Register vs Login on first run).
    pub fn has_users(&self) -> Result<bool> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        Ok(count > 0)
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
    /// Active instruments for this song.
    pub instruments: Vec<Instrument>,
    /// Username of the owner.
    pub username: String,
}

impl Db {
    /// Persist a song for `user_id`; returns the new row id.
    /// If the same user already has a song with the same name + artist it is **updated**.
    pub fn save_song(&self, song: &Song, user_id: i64) -> Result<i64> {
        let parts_json = serde_json::to_string(&song.parts).expect("Song is always serialisable");
        let instruments_json =
            serde_json::to_string(&song.instruments).expect("Instruments are always serialisable");

        // Upsert by (name, artist, user_id)
        self.conn.execute(
            "INSERT INTO songs (name, artist, key, parts_json, instruments_json, vocals_notes, user_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(name, artist, user_id) DO UPDATE SET
                 key              = excluded.key,
                 parts_json       = excluded.parts_json,
                 instruments_json = excluded.instruments_json,
                 vocals_notes     = excluded.vocals_notes",
            params![
                song.name,
                song.artist,
                song.key,
                parts_json,
                instruments_json,
                song.vocals_notes,
                user_id
            ],
        )?;

        let id: i64 = self.conn.query_row(
            "SELECT id FROM songs WHERE name = ?1 AND artist = ?2 AND user_id = ?3",
            params![song.name, song.artist, user_id],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    /// List all songs for a given user (id, name, artist, key, instruments, username).
    pub fn list_songs(&self, user_id: i64) -> Result<Vec<SongRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, s.artist, s.key, s.instruments_json, COALESCE(u.username, '')
             FROM songs s
             LEFT JOIN users u ON s.user_id = u.id
             WHERE s.user_id = ?1
             ORDER BY s.name",
        )?;

        let rows = stmt
            .query_map(params![user_id], |row| {
                let instruments_json: String = row.get(4)?;
                let username: String = row.get(5)?;
                let instruments =
                    serde_json::from_str::<Vec<Instrument>>(&instruments_json).unwrap_or_default();
                Ok(SongRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    artist: row.get(2)?,
                    key: row.get(3)?,
                    instruments,
                    username,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(rows)
    }

    /// Load the full `Song` for a given id.
    pub fn load_song(&self, id: i64) -> Result<Song> {
        self.conn.query_row(
            "SELECT name, artist, key, parts_json, instruments_json, vocals_notes
             FROM songs WHERE id = ?1",
            params![id],
            |row| {
                let name: String = row.get(0)?;
                let artist: String = row.get(1)?;
                let key: String = row.get(2)?;
                let parts_json: String = row.get(3)?;
                let instruments_json: String = row.get(4)?;
                let vocals_notes: String = row.get(5)?;

                let parts = serde_json::from_str(&parts_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let instruments = serde_json::from_str(&instruments_json).unwrap_or_default();

                Ok(Song {
                    name,
                    artist,
                    key,
                    parts,
                    instruments,
                    vocals_notes,
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

    fn setup_user(db: &Db) -> i64 {
        db.create_user("testuser", "testpass").unwrap()
    }

    #[test]
    fn create_and_verify_user() {
        let db = Db::open_in_memory().unwrap();
        db.create_user("alice", "s3cr3t").unwrap();
        let user = db.verify_user("alice", "s3cr3t").unwrap();
        assert!(user.is_some());
        assert_eq!(user.unwrap().username, "alice");
    }

    #[test]
    fn wrong_password_returns_none() {
        let db = Db::open_in_memory().unwrap();
        db.create_user("bob", "correct").unwrap();
        let result = db.verify_user("bob", "wrong").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn duplicate_username_is_error() {
        let db = Db::open_in_memory().unwrap();
        db.create_user("carol", "pw1").unwrap();
        assert!(db.create_user("carol", "pw2").is_err());
    }

    #[test]
    fn save_and_load_song_round_trips() {
        let db = Db::open_in_memory().unwrap();
        let uid = setup_user(&db);
        let song = sample_song();
        let id = db.save_song(&song, uid).unwrap();
        let loaded = db.load_song(id).unwrap();
        assert_eq!(loaded, song);
    }

    #[test]
    fn list_songs_returns_saved_row() {
        let db = Db::open_in_memory().unwrap();
        let uid = setup_user(&db);
        db.save_song(&sample_song(), uid).unwrap();
        let rows = db.list_songs(uid).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Hey Jude");
        assert_eq!(rows[0].username, "testuser");
    }

    #[test]
    fn save_song_twice_upserts() {
        let db = Db::open_in_memory().unwrap();
        let uid = setup_user(&db);
        let mut song = sample_song();
        db.save_song(&song, uid).unwrap();
        song.key = "G Major".to_string();
        db.save_song(&song, uid).unwrap();
        let rows = db.list_songs(uid).unwrap();
        assert_eq!(rows.len(), 1); // still one row
        let loaded = db.load_song(rows[0].id).unwrap();
        assert_eq!(loaded.key, "G Major");
    }

    #[test]
    fn users_only_see_their_own_songs() {
        let db = Db::open_in_memory().unwrap();
        let uid_a = db.create_user("alice", "pw").unwrap();
        let uid_b = db.create_user("bob", "pw").unwrap();
        db.save_song(&sample_song(), uid_a).unwrap();
        assert_eq!(db.list_songs(uid_a).unwrap().len(), 1);
        assert_eq!(db.list_songs(uid_b).unwrap().len(), 0);
    }

    #[test]
    fn delete_song_removes_it() {
        let db = Db::open_in_memory().unwrap();
        let uid = setup_user(&db);
        let id = db.save_song(&sample_song(), uid).unwrap();
        db.delete_song(id).unwrap();
        assert!(db.list_songs(uid).unwrap().is_empty());
    }

    #[test]
    fn save_and_load_pdf_round_trips() {
        let db = Db::open_in_memory().unwrap();
        let uid = setup_user(&db);
        let song_id = db.save_song(&sample_song(), uid).unwrap();
        let data = b"fake-pdf-bytes";
        let pdf_id = db.save_pdf(song_id, data).unwrap();
        let loaded = db.load_pdf(pdf_id).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn list_pdfs_returns_metadata() {
        let db = Db::open_in_memory().unwrap();
        let uid = setup_user(&db);
        let song_id = db.save_song(&sample_song(), uid).unwrap();
        db.save_pdf(song_id, b"pdf1").unwrap();
        db.save_pdf(song_id, b"pdf2").unwrap();
        let rows = db.list_pdfs(song_id).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn delete_song_cascades_to_pdfs() {
        let db = Db::open_in_memory().unwrap();
        let uid = setup_user(&db);
        let song_id = db.save_song(&sample_song(), uid).unwrap();
        db.save_pdf(song_id, b"pdf").unwrap();
        db.delete_song(song_id).unwrap();
        // pdf rows should be gone too
        assert!(db.list_pdfs(song_id).unwrap().is_empty());
    }
}
