pub use crate::model::Room;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;
use uuid::Uuid;

/// Sanitize an input string into a URL-friendly slug.
pub fn sanitize_slug(input: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for c in input.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

/// Create a public room ensuring unique slug.
pub fn create_public_room(conn: &Connection, name: &str, slug_input: Option<&str>) -> Result<Room> {
    let slug_src = slug_input.unwrap_or(name);
    let slug = sanitize_slug(slug_src);
    if slug.is_empty() {
        return Err(anyhow!("invalid_slug"));
    }
    let id = Uuid::new_v4();
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let res = conn.execute(
        "INSERT INTO rooms (id, slug, name, is_dm, created_at) VALUES (?1, ?2, ?3, 0, ?4)",
        params![id.to_string(), slug, name, now],
    );
    match res {
        Ok(_) => Ok(Room {
            id,
            slug,
            name: name.into(),
            is_dm: false,
            created_at: now,
        }),
        Err(e) => {
            if matches!(
                e.sqlite_error_code(),
                Some(rusqlite::ErrorCode::ConstraintViolation)
            ) {
                Err(anyhow!("duplicate_slug"))
            } else {
                Err(e.into())
            }
        }
    }
}

/// Deterministic UUID for a DM between two users.
pub fn dm_room_id(a: u32, b: u32) -> Uuid {
    let (min, max) = if a < b { (a, b) } else { (b, a) };
    let ns = Uuid::NAMESPACE_OID;
    let name = format!("dm:{}:{}", min, max);
    Uuid::new_v5(&ns, name.as_bytes())
}

/// Create or fetch a DM room for two users.
pub fn get_or_create_dm_room(conn: &Connection, a: u32, b: u32) -> Result<Room> {
    let id = dm_room_id(a, b);
    if let Some(room) = get_room_by_id(conn, &id)? {
        return Ok(room);
    }
    let slug = format!("dm-{}-{}", a.min(b), a.max(b));
    let now = OffsetDateTime::now_utc().unix_timestamp();
    conn.execute(
        "INSERT INTO rooms (id, slug, name, is_dm, created_at) VALUES (?1, ?2, '', 1, ?3)",
        params![id.to_string(), slug, now],
    )?;
    conn.execute(
        "INSERT INTO room_members (room_id, user_id) VALUES (?1, ?2)",
        params![id.to_string(), a],
    )?;
    conn.execute(
        "INSERT INTO room_members (room_id, user_id) VALUES (?1, ?2)",
        params![id.to_string(), b],
    )?;
    Ok(Room {
        id,
        slug,
        name: String::new(),
        is_dm: true,
        created_at: now,
    })
}

fn get_room_by_id(conn: &Connection, id: &Uuid) -> Result<Option<Room>> {
    let mut stmt =
        conn.prepare("SELECT id, slug, name, is_dm, created_at FROM rooms WHERE id = ?1")?;
    let room = stmt
        .query_row([id.to_string()], |row| {
            Ok(Room {
                id: Uuid::parse_str(row.get::<_, String>(0)?.as_str()).unwrap(),
                slug: row.get(1)?,
                name: row.get(2)?,
                is_dm: row.get::<_, i64>(3)? != 0,
                created_at: row.get(4)?,
            })
        })
        .optional()?;
    Ok(room)
}

/// List rooms visible to a user.
pub fn list_rooms_for_user(conn: &Connection, user_id: u32) -> Result<Vec<Room>> {
    let mut stmt = conn.prepare(
        "SELECT id, slug, name, is_dm, created_at FROM rooms WHERE is_dm = 0 OR id IN (SELECT room_id FROM room_members WHERE user_id = ?1) ORDER BY created_at",
    )?;
    let rooms = stmt
        .query_map([user_id], |row| {
            Ok(Room {
                id: Uuid::parse_str(row.get::<_, String>(0)?.as_str()).unwrap(),
                slug: row.get(1)?,
                name: row.get(2)?,
                is_dm: row.get::<_, i64>(3)? != 0,
                created_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rooms)
}

/// Check if a user can access a room.
pub fn user_can_access_room(conn: &Connection, room_id: &Uuid, user_id: u32) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT is_dm FROM rooms WHERE id = ?1")?;
    let is_dm: Option<i64> = stmt
        .query_row([room_id.to_string()], |row| row.get(0))
        .optional()?;
    let Some(is_dm) = is_dm else { return Ok(false) };
    if is_dm == 0 {
        return Ok(true);
    }
    let mut stmt =
        conn.prepare("SELECT 1 FROM room_members WHERE room_id = ?1 AND user_id = ?2")?;
    let exists: Option<i64> = stmt
        .query_row(params![room_id.to_string(), user_id], |row| row.get(0))
        .optional()?;
    Ok(exists.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn slug_unique_and_list() {
        let conn = db::init_db(":memory:").unwrap();
        create_public_room(&conn, "General", Some("general")).unwrap();
        assert!(create_public_room(&conn, "Other", Some("general")).is_err());
        get_or_create_dm_room(&conn, 1, 2).unwrap();
        let rooms = list_rooms_for_user(&conn, 1).unwrap();
        assert_eq!(rooms.len(), 2);
        let rooms = list_rooms_for_user(&conn, 3).unwrap();
        assert_eq!(rooms.len(), 1);
    }

    #[test]
    fn dm_id_is_deterministic() {
        let id1 = dm_room_id(1, 2);
        let id2 = dm_room_id(2, 1);
        let id3 = dm_room_id(1, 3);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }
}
