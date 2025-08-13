use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

/// Update read pointer for a user and room.
pub fn set_read_pointer(conn: &Connection, user_id: u32, room_id: &Uuid, ts: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO read_pointers (room_id, user_id, last_read_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT(room_id, user_id) DO UPDATE SET last_read_at = excluded.last_read_at",
        params![room_id.to_string(), user_id, ts],
    )?;
    Ok(())
}

/// Get last read timestamp for a user and room.
pub fn get_last_read_at(conn: &Connection, user_id: u32, room_id: &Uuid) -> Result<i64> {
    let mut stmt = conn.prepare(
        "SELECT last_read_at FROM read_pointers WHERE room_id = ?1 AND user_id = ?2",
    )?;
    let ts: Option<i64> = stmt
        .query_row(params![room_id.to_string(), user_id], |row| row.get(0))
        .optional()?;
    Ok(ts.unwrap_or(0))
}

/// Calculate unread count for a user in a room.
pub fn unread_count(conn: &Connection, user_id: u32, room_id: &Uuid) -> Result<u32> {
    let last = get_last_read_at(conn, user_id, room_id)?;
    let mut stmt = conn.prepare(
        "SELECT COUNT(*) FROM messages WHERE room_id = ?1 AND created_at > ?2 AND author_id <> ?3",
    )?;
    let count: u32 = stmt
        .query_row(
            params![room_id.to_string(), last, user_id.to_string()],
            |row| row.get::<_, u32>(0),
        )?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, messages};
    use time::OffsetDateTime;

    #[test]
    fn last_read_math() {
        let conn = db::init_db(":memory:").unwrap();
        let room_id = Uuid::new_v4();
        conn.execute(
            "INSERT INTO rooms (id, slug, name, is_dm, created_at) VALUES (?1, 'r', 'R', 0, 0)",
            params![room_id.to_string()],
        )
        .unwrap();
        let _m1 = messages::create_message(&conn, &room_id, 1, "m1", None).unwrap();
        let m2 = messages::create_message(&conn, &room_id, 2, "m2", None).unwrap();
        assert_eq!(unread_count(&conn, 1, &room_id).unwrap(), 1);
        set_read_pointer(&conn, 1, &room_id, m2.created_at).unwrap();
        assert_eq!(unread_count(&conn, 1, &room_id).unwrap(), 0);
        let now = OffsetDateTime::now_utc().unix_timestamp();
        set_read_pointer(&conn, 1, &room_id, now).unwrap();
        assert_eq!(get_last_read_at(&conn, 1, &room_id).unwrap(), now);
    }
}
