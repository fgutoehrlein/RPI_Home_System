use crate::model::{Message, SearchResult};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;
use uuid::Uuid;

static MENTION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"@([A-Za-z0-9_]+)").unwrap());

/// Cursor for pagination.
#[derive(Clone, Copy)]
pub enum Cursor {
    Timestamp(i64),
    Id(Uuid),
}

/// Create a new text message.
pub fn create_message(
    conn: &Connection,
    room_id: &Uuid,
    author_id: u32,
    text_md: &str,
    reply_to: Option<&Uuid>,
    idem_key: Option<&str>,
) -> Result<Message> {
    if text_md.trim().is_empty() {
        return Err(anyhow!("empty_message"));
    }
    if let Some(key) = idem_key {
        let mut stmt = conn.prepare(
            "SELECT id, room_id, author_id, text_md, created_at, edited_at FROM messages WHERE author_id = ?1 AND idempotency_key = ?2",
        )?;
        if let Some(existing) = stmt
            .query_row(params![author_id.to_string(), key], row_to_msg)
            .optional()?
        {
            return Ok(existing);
        }
    }
    let id = Uuid::new_v4();
    let now = OffsetDateTime::now_utc().unix_timestamp();
    conn.execute(
        "INSERT INTO messages (id, room_id, author_id, text_md, created_at, reply_to, idempotency_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id.to_string(),
            room_id.to_string(),
            author_id.to_string(),
            text_md,
            now,
            reply_to.map(|r| r.to_string()),
            idem_key
        ],
    )?;
    sync_mentions(conn, &id, text_md)?;
    Ok(Message {
        id,
        room_id: *room_id,
        author_id,
        text_md: text_md.into(),
        created_at: now,
        edited_at: None,
        reply_to: reply_to.copied(),
    })
}

fn row_to_msg(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: Uuid::parse_str(row.get::<_, String>(0)?.as_str()).unwrap(),
        room_id: Uuid::parse_str(row.get::<_, String>(1)?.as_str()).unwrap(),
        author_id: row.get::<_, String>(2)?.parse::<u32>().unwrap_or_default(),
        text_md: row.get(3)?,
        created_at: row.get(4)?,
        edited_at: row.get(5).ok(),
        reply_to: row
            .get::<_, Option<String>>(6)?
            .and_then(|s| Uuid::parse_str(&s).ok()),
    })
}

/// List messages for a room with optional before cursor.
pub fn list_messages(
    conn: &Connection,
    room_id: &Uuid,
    before: Option<Cursor>,
    limit: usize,
) -> Result<Vec<Message>> {
    let limit = limit.min(200);
    let (ts, id) = match before {
        Some(Cursor::Timestamp(ts)) => (ts, Uuid::nil()),
        Some(Cursor::Id(id)) => {
            let mut stmt = conn.prepare("SELECT created_at FROM messages WHERE id = ?1")?;
            let ts: Option<i64> = stmt
                .query_row([id.to_string()], |row| row.get(0))
                .optional()?;
            (ts.unwrap_or(i64::MAX), id)
        }
        None => (i64::MAX, Uuid::nil()),
    };
    let mut stmt = conn.prepare(
        "SELECT id, room_id, author_id, text_md, created_at, edited_at, reply_to FROM messages WHERE room_id = ?1 AND (created_at < ?2 OR (created_at = ?2 AND id < ?3)) ORDER BY created_at DESC, id DESC LIMIT ?4",
    )?;
    let iter = stmt.query_map(
        params![room_id.to_string(), ts, id.to_string(), limit as i64],
        row_to_msg,
    )?;
    let mut msgs = Vec::new();
    for m in iter {
        msgs.push(m?);
    }
    Ok(msgs)
}

fn sync_mentions(conn: &Connection, message_id: &Uuid, text: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM message_mentions WHERE message_id = ?1",
        [message_id.to_string()],
    )?;
    for cap in MENTION_RE.captures_iter(text) {
        let uname = cap.get(1).unwrap().as_str();
        if let Ok(uid) = conn.query_row(
            "SELECT id FROM users WHERE lower(username) = lower(?1)",
            [uname],
            |row| row.get::<_, String>(0),
        ) {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO message_mentions (message_id, user_id) VALUES (?1, ?2)",
                params![message_id.to_string(), uid],
            );
        }
    }
    Ok(())
}

pub fn edit_message(
    conn: &Connection,
    message_id: &Uuid,
    author_id: u32,
    text_md: &str,
) -> Result<Message> {
    if text_md.trim().is_empty() {
        return Err(anyhow!("empty_message"));
    }
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let changed = conn.execute(
        "UPDATE messages SET text_md = ?2, edited_at = ?3 WHERE id = ?1 AND author_id = ?4",
        params![message_id.to_string(), text_md, now, author_id.to_string()],
    )?;
    if changed == 0 {
        anyhow::bail!("not_found");
    }
    sync_mentions(conn, message_id, text_md)?;
    let mut stmt = conn.prepare(
        "SELECT id, room_id, author_id, text_md, created_at, edited_at, reply_to FROM messages WHERE id = ?1",
    )?;
    let msg = stmt.query_row([message_id.to_string()], row_to_msg)?;
    Ok(msg)
}

pub fn delete_message(conn: &Connection, message_id: &Uuid, author_id: u32) -> Result<Uuid> {
    let mut stmt = conn.prepare("SELECT room_id, author_id FROM messages WHERE id = ?1")?;
    let (room_id, author): (String, String) = stmt.query_row([message_id.to_string()], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    if author.parse::<u32>().unwrap_or_default() != author_id {
        anyhow::bail!("forbidden");
    }
    conn.execute(
        "DELETE FROM messages WHERE id = ?1",
        [message_id.to_string()],
    )?;
    conn.execute(
        "DELETE FROM message_mentions WHERE message_id = ?1",
        [message_id.to_string()],
    )?;
    Ok(Uuid::parse_str(&room_id).unwrap())
}

pub fn search_messages(
    conn: &Connection,
    q: &str,
    room_id: Option<&Uuid>,
) -> Result<Vec<SearchResult>> {
    let mut sql = String::from("SELECT m.id, m.room_id, m.author_id, m.text_md, m.created_at, m.edited_at, m.reply_to, highlight(messages_fts, 0, '<b>', '</b>') FROM messages_fts JOIN messages m ON m.rowid = messages_fts.rowid WHERE messages_fts MATCH ?1");
    let mut params: Vec<String> = vec![q.to_string()];
    if let Some(r) = room_id {
        sql.push_str(" AND m.room_id = ?2");
        params.push(r.to_string());
    }
    sql.push_str(" ORDER BY m.created_at DESC LIMIT 50");
    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|s| s as _).collect();
    let mut rows = stmt.query(rusqlite::params_from_iter(param_refs))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let msg = Message {
            id: Uuid::parse_str(row.get::<_, String>(0)?.as_str()).unwrap(),
            room_id: Uuid::parse_str(row.get::<_, String>(1)?.as_str()).unwrap(),
            author_id: row.get::<_, String>(2)?.parse::<u32>().unwrap_or_default(),
            text_md: row.get(3)?,
            created_at: row.get(4)?,
            edited_at: row.get(5).ok(),
            reply_to: row
                .get::<_, Option<String>>(6)?
                .and_then(|s| Uuid::parse_str(&s).ok()),
        };
        let snippet: String = row.get(7)?;
        out.push(SearchResult {
            message: msg,
            highlights: if snippet.is_empty() {
                vec![]
            } else {
                vec![snippet]
            },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn create_and_validate() {
        let conn = db::init_db(":memory:").unwrap();
        let room_id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO rooms (id, slug, name, is_dm, created_at) VALUES (?1, 'r', 'R', 0, 0)",
            params![room_id.to_string()],
        )
        .unwrap();
        assert!(create_message(&conn, &room_id, 1, "", None, None).is_err());
        let m = create_message(&conn, &room_id, 1, "hi", None, None).unwrap();
        assert_eq!(m.text_md, "hi");
    }

    #[test]
    fn pagination_order() {
        let conn = db::init_db(":memory:").unwrap();
        let room_id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO rooms (id, slug, name, is_dm, created_at) VALUES (?1, 'r', 'R', 0, 0)",
            params![room_id.to_string()],
        )
        .unwrap();
        create_message(&conn, &room_id, 1, "m1", None, None).unwrap();
        create_message(&conn, &room_id, 1, "m2", None, None).unwrap();
        create_message(&conn, &room_id, 1, "m3", None, None).unwrap();
        let all = list_messages(&conn, &room_id, None, 10).unwrap();
        let first = list_messages(&conn, &room_id, None, 2).unwrap();
        assert_eq!(first.len(), 2);
        let second = list_messages(
            &conn,
            &room_id,
            Some(Cursor::Id(first.last().unwrap().id)),
            2,
        )
        .unwrap();
        assert_eq!(second.len(), 1);
        let mut combined = first.clone();
        combined.extend(second.clone());
        assert_eq!(combined, all);
    }

    #[test]
    fn edit_delete_and_search() {
        let conn = db::init_db(":memory:").unwrap();
        let room_id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO rooms (id, slug, name, is_dm, created_at) VALUES (?1, 'r', 'R', 0, 0)",
            params![room_id.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users (id, username, display_name) VALUES ('1','bob','Bob')",
            [],
        )
        .unwrap();
        let m = create_message(&conn, &room_id, 1, "hi @bob", None, None).unwrap();
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message_mentions WHERE user_id='1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cnt, 1);
        let res = search_messages(&conn, "hi", None).unwrap();
        assert_eq!(res.len(), 1);
        let edited = edit_message(&conn, &m.id, 1, "bye").unwrap();
        assert!(edited.edited_at.is_some());
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message_mentions WHERE user_id='1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cnt, 0);
        assert_eq!(search_messages(&conn, "hi", None).unwrap().len(), 0);
        assert_eq!(search_messages(&conn, "bye", None).unwrap().len(), 1);
        delete_message(&conn, &m.id, 1).unwrap();
        assert_eq!(search_messages(&conn, "bye", None).unwrap().len(), 0);
    }
}
