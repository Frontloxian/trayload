//! Favorites, collections, per-game launch options and play history.

use crate::library::Db;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

fn e<E: ToString>(x: E) -> String {
    x.to_string()
}

fn lock(db: &Db) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, String> {
    db.0.lock().map_err(|_| "database lock poisoned".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    id: i64,
    name: String,
    count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Membership {
    collection_id: i64,
    game_id: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    started_at: i64,
    seconds: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetails {
    extra_args: String,
    notes: String,
    sessions: Vec<SessionInfo>,
    session_count: i64,
}

#[tauri::command]
pub fn set_favorite(db: tauri::State<'_, Db>, id: i64, favorite: bool) -> Result<(), String> {
    let conn = lock(&db)?;
    conn.execute("UPDATE games SET favorite = ?2 WHERE id = ?1", params![id, favorite as i64]).map_err(e)?;
    Ok(())
}

#[tauri::command]
pub fn list_collections(db: tauri::State<'_, Db>) -> Result<Vec<Collection>, String> {
    let conn = lock(&db)?;
    let mut st = conn
        .prepare(
            "SELECT c.id, c.name,
               (SELECT COUNT(*) FROM collection_games cg JOIN games g ON g.id = cg.game_id
                WHERE cg.collection_id = c.id AND g.removed = 0)
             FROM collections c ORDER BY c.name COLLATE NOCASE",
        )
        .map_err(e)?;
    let rows = st
        .query_map([], |r| Ok(Collection { id: r.get(0)?, name: r.get(1)?, count: r.get(2)? }))
        .map_err(e)?;
    let out: Vec<Collection> = rows.collect::<Result<_, _>>().map_err(e)?;
    Ok(out)
}

fn clean_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Please type a name.".into());
    }
    if n.chars().count() > 60 {
        return Err("That name is too long (60 characters at most).".into());
    }
    Ok(n.to_string())
}

#[tauri::command]
pub fn create_collection(db: tauri::State<'_, Db>, name: String) -> Result<i64, String> {
    let name = clean_name(&name)?;
    let conn = lock(&db)?;
    let exists: Option<i64> = conn
        .query_row("SELECT id FROM collections WHERE name = ?1 COLLATE NOCASE", [&name], |r| r.get(0))
        .optional()
        .map_err(e)?;
    if exists.is_some() {
        return Err("A collection with that name already exists.".into());
    }
    conn.execute("INSERT INTO collections (name) VALUES (?1)", [&name]).map_err(e)?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn rename_collection(db: tauri::State<'_, Db>, id: i64, name: String) -> Result<(), String> {
    let name = clean_name(&name)?;
    let conn = lock(&db)?;
    let clash: Option<i64> = conn
        .query_row("SELECT id FROM collections WHERE name = ?1 COLLATE NOCASE AND id <> ?2", params![name, id], |r| r.get(0))
        .optional()
        .map_err(e)?;
    if clash.is_some() {
        return Err("A collection with that name already exists.".into());
    }
    conn.execute("UPDATE collections SET name = ?2 WHERE id = ?1", params![id, name]).map_err(e)?;
    Ok(())
}

/// Deletes the collection only. The games stay in the library.
#[tauri::command]
pub fn delete_collection(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = lock(&db)?;
    conn.execute("DELETE FROM collection_games WHERE collection_id = ?1", [id]).map_err(e)?;
    conn.execute("DELETE FROM collections WHERE id = ?1", [id]).map_err(e)?;
    Ok(())
}

#[tauri::command]
pub fn list_collection_members(db: tauri::State<'_, Db>) -> Result<Vec<Membership>, String> {
    let conn = lock(&db)?;
    let mut st = conn.prepare("SELECT collection_id, game_id FROM collection_games").map_err(e)?;
    let rows = st
        .query_map([], |r| Ok(Membership { collection_id: r.get(0)?, game_id: r.get(1)? }))
        .map_err(e)?;
    let out: Vec<Membership> = rows.collect::<Result<_, _>>().map_err(e)?;
    Ok(out)
}

#[tauri::command]
pub fn set_game_in_collection(
    db: tauri::State<'_, Db>,
    collection_id: i64,
    game_id: i64,
    member: bool,
) -> Result<(), String> {
    let conn = lock(&db)?;
    if member {
        conn.execute(
            "INSERT OR IGNORE INTO collection_games (collection_id, game_id) VALUES (?1, ?2)",
            params![collection_id, game_id],
        )
        .map_err(e)?;
    } else {
        conn.execute(
            "DELETE FROM collection_games WHERE collection_id = ?1 AND game_id = ?2",
            params![collection_id, game_id],
        )
        .map_err(e)?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_game_details(db: tauri::State<'_, Db>, id: i64) -> Result<GameDetails, String> {
    let conn = lock(&db)?;
    let overrides: Option<(String, String)> = conn
        .query_row("SELECT extra_args, notes FROM game_overrides WHERE game_id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
        .map_err(e)?;
    let (extra_args, notes) = overrides.unwrap_or_default();
    let mut st = conn
        .prepare(
            "SELECT started_at, ended_at - started_at FROM play_sessions
             WHERE game_id = ?1 AND completed = 1 ORDER BY started_at DESC LIMIT 15",
        )
        .map_err(e)?;
    let rows = st
        .query_map([id], |r| Ok(SessionInfo { started_at: r.get(0)?, seconds: r.get(1)? }))
        .map_err(e)?;
    let sessions: Vec<SessionInfo> = rows.collect::<Result<_, _>>().map_err(e)?;
    let session_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM play_sessions WHERE game_id = ?1 AND completed = 1", [id], |r| r.get(0))
        .map_err(e)?;
    Ok(GameDetails { extra_args, notes, sessions, session_count })
}

#[tauri::command]
pub fn save_overrides(db: tauri::State<'_, Db>, id: i64, extra_args: String, notes: String) -> Result<(), String> {
    if extra_args.len() > 2000 || notes.len() > 10000 {
        return Err("That text is too long.".into());
    }
    let conn = lock(&db)?;
    conn.execute(
        "INSERT INTO game_overrides (game_id, extra_args, notes) VALUES (?1, ?2, ?3)
         ON CONFLICT(game_id) DO UPDATE SET extra_args = excluded.extra_args, notes = excluded.notes",
        params![id, extra_args.trim(), notes],
    )
    .map_err(e)?;
    Ok(())
}
