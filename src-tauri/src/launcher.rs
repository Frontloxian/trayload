//! Launcher: start a game in its emulator and record play sessions.
//! Playtime is the sum of play_sessions. A heartbeat keeps each open session's end time fresh,
//! so a crash or power loss costs at most ~30 seconds, and open sessions are closed on next start.

use crate::{installer, library::Db, registry};
use rusqlite::params;
use serde::Serialize;
use std::{
    collections::HashSet,
    path::Path,
    process::Command,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct Running(pub Mutex<HashSet<i64>>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameEvent {
    game_id: i64,
    seconds: i64,
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn e<E: ToString>(x: E) -> String {
    x.to_string()
}

fn build_args(template: &[String], rom: &str) -> Vec<String> {
    template.iter().map(|a| if a == "{rom}" { rom.to_string() } else { a.clone() }).collect()
}

/// Close sessions left open by a crash: count the time up to the last heartbeat.
pub fn recover_sessions(db: &Db) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute_batch(
        "UPDATE games SET
           total_playtime_sec = total_playtime_sec + COALESCE((SELECT SUM(ended_at - started_at) FROM play_sessions WHERE game_id = games.id AND completed = 0), 0),
           last_played = COALESCE((SELECT MAX(ended_at) FROM play_sessions WHERE game_id = games.id AND completed = 0), last_played)
         WHERE id IN (SELECT game_id FROM play_sessions WHERE completed = 0);
         UPDATE play_sessions SET completed = 1 WHERE completed = 0;",
    )
    .map_err(e)
}

#[tauri::command]
pub fn running_games(running: tauri::State<'_, Running>) -> Vec<i64> {
    running.0.lock().map(|r| r.iter().copied().collect()).unwrap_or_default()
}

#[tauri::command]
pub fn play_game(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    running: tauri::State<'_, Running>,
    id: i64,
) -> Result<(), String> {
    let (system_id, path): (String, String) = {
        let conn = db.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        conn.query_row("SELECT system_id, path FROM games WHERE id = ?1 AND removed = 0", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|_| "That game is no longer in the library.".to_string())?
    };
    if !Path::new(&path).exists() {
        return Err("The game file is missing. Check that its drive or folder is connected, then Rescan.".into());
    }

    let reg = registry::load(&app)?;
    let sys = reg.systems.iter().find(|s| s.id == system_id).ok_or("Unknown system.")?;
    let template = sys
        .launch_args
        .clone()
        .ok_or_else(|| format!("{} can't be started from Trayload yet.", sys.emulator))?;
    let dir = installer::emu_dir(&app, &installer::slug(&sys.emulator))?;
    let info = installer::read_installed(&dir)
        .ok_or_else(|| format!("{} isn't installed yet. Install it in the Emulators tab first.", sys.emulator))?;
    let exe = dir.join(&info.exe_rel_path);
    if !exe.exists() {
        return Err(format!("{}'s files are missing. Reinstall it in the Emulators tab.", sys.emulator));
    }

    if !running.0.lock().map_err(|_| "lock poisoned".to_string())?.insert(id) {
        return Err("This game is already running.".into());
    }
    let cwd = exe.parent().unwrap_or(&dir).to_path_buf();
    let mut child = match Command::new(&exe).args(build_args(&template, &path)).current_dir(cwd).spawn() {
        Ok(c) => c,
        Err(err) => {
            if let Ok(mut r) = running.0.lock() {
                r.remove(&id);
            }
            return Err(format!("Could not start {}: {err}", exe.display()));
        }
    };

    let started = unix_now();
    let session_id: i64 = {
        let conn = db.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        conn.execute(
            "INSERT INTO play_sessions (game_id, started_at, ended_at, completed) VALUES (?1, ?2, ?2, 0)",
            params![id, started],
        )
        .map_err(e)?;
        conn.last_insert_rowid()
    };
    let _ = app.emit("game-started", GameEvent { game_id: id, seconds: 0 });

    let db2: Db = db.inner().clone();
    std::thread::spawn(move || {
        let mut last_beat = Instant::now();
        loop {
            match child.try_wait() {
                Ok(None) => {}
                _ => break, // exited (or we can no longer tell)
            }
            std::thread::sleep(Duration::from_secs(1));
            if last_beat.elapsed() >= Duration::from_secs(30) {
                if let Ok(conn) = db2.0.lock() {
                    let _ = conn.execute("UPDATE play_sessions SET ended_at = ?2 WHERE id = ?1", params![session_id, unix_now()]);
                }
                last_beat = Instant::now();
            }
        }
        let now = unix_now();
        let secs = (now - started).max(0);
        if let Ok(conn) = db2.0.lock() {
            let _ = conn.execute(
                "UPDATE play_sessions SET ended_at = ?2, completed = 1 WHERE id = ?1",
                params![session_id, now],
            );
            let _ = conn.execute(
                "UPDATE games SET last_played = ?2, total_playtime_sec = total_playtime_sec + ?3 WHERE id = ?1",
                params![id, now, secs],
            );
        }
        if let Ok(mut r) = app.state::<Running>().0.lock() {
            r.remove(&id);
        }
        let _ = app.emit("game-stopped", GameEvent { game_id: id, seconds: secs });
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_placeholder_is_replaced_as_one_argument() {
        let t = vec!["-batch".to_string(), "--".to_string(), "{rom}".to_string()];
        let a = build_args(&t, "C:\\roms\\My Game (USA).cue");
        assert_eq!(a, vec!["-batch", "--", "C:\\roms\\My Game (USA).cue"]);
    }
}
