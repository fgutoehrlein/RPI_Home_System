#![allow(dead_code)]

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

/// Initialize the SQLite database and run migrations.
pub fn init_db<P: AsRef<Path>>(path: P) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY,
  username TEXT UNIQUE NOT NULL,
  display_name TEXT NOT NULL,
  avatar_url TEXT
);

CREATE TABLE IF NOT EXISTS config (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  passphrase_hash TEXT NOT NULL,
  jwt_secret BLOB NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS rooms (
  id TEXT PRIMARY KEY,
  slug TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  is_dm INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS room_members (
  room_id TEXT NOT NULL REFERENCES rooms(id),
  user_id INTEGER NOT NULL,
  PRIMARY KEY (room_id, user_id)
);

CREATE TABLE IF NOT EXISTS messages (
  id TEXT PRIMARY KEY,
  room_id TEXT NOT NULL REFERENCES rooms(id),
  author_id TEXT NOT NULL,
  text_md TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  edited_at INTEGER,
  idempotency_key TEXT,
  UNIQUE(author_id, idempotency_key)
);

CREATE TABLE IF NOT EXISTS attachments (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  file_id TEXT NOT NULL,
  file_name TEXT NOT NULL,
  mime TEXT,
  size_bytes INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS reads (
  user_id TEXT NOT NULL REFERENCES users(id),
  message_id TEXT NOT NULL REFERENCES messages(id),
  read_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, message_id)
);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(text_md, content='messages', content_rowid='rowid');
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_fts(rowid, text_md) VALUES (new.rowid, new.text_md);
END;
CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, text_md) VALUES ('delete', old.rowid, old.text_md);
END;
CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, text_md) VALUES ('delete', old.rowid, old.text_md);
  INSERT INTO messages_fts(rowid, text_md) VALUES (new.rowid, new.text_md);
END;
"#;
