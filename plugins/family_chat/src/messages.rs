use crate::model::Message;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;
use uuid::Uuid;

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
            .query_row(params![author_id.to_string(), key], |row| row_to_msg(row))
            .optional()?
        {
            return Ok(existing);
        }
    }
    let id = Uuid::new_v4();
    let now = OffsetDateTime::now_utc().unix_timestamp();
    conn.execute(
        "INSERT INTO messages (id, room_id, author_id, text_md, created_at, idempotency_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id.to_string(),
            room_id.to_string(),
            author_id.to_string(),
            text_md,
            now,
            idem_key
        ],
    )?;
    Ok(Message {
        id,
        room_id: *room_id,
        author_id,
        text_md: text_md.into(),
        created_at: now,
        edited_at: None,
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
        "SELECT id, room_id, author_id, text_md, created_at, edited_at FROM messages WHERE room_id = ?1 AND (created_at < ?2 OR (created_at = ?2 AND id < ?3)) ORDER BY created_at DESC, id DESC LIMIT ?4",
    )?;
    let iter = stmt.query_map(
        params![room_id.to_string(), ts, id.to_string(), limit as i64],
        |row| row_to_msg(row),
    )?;
    let mut msgs = Vec::new();
    for m in iter {
        msgs.push(m?);
    }
    Ok(msgs)
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
        assert!(create_message(&conn, &room_id, 1, "", None).is_err());
        let m = create_message(&conn, &room_id, 1, "hi", None).unwrap();
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
        create_message(&conn, &room_id, 1, "m1", None).unwrap();
        create_message(&conn, &room_id, 1, "m2", None).unwrap();
        create_message(&conn, &room_id, 1, "m3", None).unwrap();
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
}
