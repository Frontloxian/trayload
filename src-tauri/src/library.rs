//! Library: SQLite storage, game detection, importing and scanning.
//! ROMs are never copied or modified. Only their paths are stored.

use crate::registry::{self, System};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

const SCHEMA: &str = "
PRAGMA journal_mode=WAL;
CREATE TABLE IF NOT EXISTS games (
  id INTEGER PRIMARY KEY,
  system_id TEXT NOT NULL,
  title TEXT NOT NULL,
  path TEXT NOT NULL UNIQUE,
  size INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  added_at INTEGER NOT NULL,
  last_played INTEGER,
  total_playtime_sec INTEGER NOT NULL DEFAULT 0,
  favorite INTEGER NOT NULL DEFAULT 0,
  missing INTEGER NOT NULL DEFAULT 0,
  removed INTEGER NOT NULL DEFAULT 0,
  art_status TEXT NOT NULL DEFAULT 'none'
);
CREATE TABLE IF NOT EXISTS unassigned (
  path TEXT PRIMARY KEY,
  size INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  candidates TEXT NOT NULL,
  dismissed INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS play_sessions (
  id INTEGER PRIMARY KEY,
  game_id INTEGER NOT NULL,
  started_at INTEGER NOT NULL,
  ended_at INTEGER NOT NULL,
  completed INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS collections (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE
);
CREATE TABLE IF NOT EXISTS collection_games (
  collection_id INTEGER NOT NULL,
  game_id INTEGER NOT NULL,
  PRIMARY KEY (collection_id, game_id)
);
CREATE TABLE IF NOT EXISTS game_overrides (
  game_id INTEGER PRIMARY KEY,
  extra_args TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
";

#[derive(Clone)]
pub struct Db(pub Arc<Mutex<Connection>>);

#[derive(Default)]
pub struct LastScan(pub Mutex<Option<Summary>>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    id: i64,
    system_id: String,
    title: String,
    path: String,
    size: i64,
    added_at: i64,
    last_played: Option<i64>,
    total_playtime_sec: i64,
    favorite: bool,
    missing: bool,
    art_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unassigned {
    path: String,
    file_name: String,
    candidates: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub unassigned: usize,
    pub restored: usize,
    pub missing: usize,
}

fn e<E: ToString>(x: E) -> String {
    x.to_string()
}

fn lock(db: &Db) -> Result<MutexGuard<'_, Connection>, String> {
    db.0.lock().map_err(|_| "database lock poisoned".to_string())
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Schema upgrades after the first release of the tables, tracked by PRAGMA user_version.
fn migrate(conn: &Connection) -> Result<(), String> {
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(e)?;
    if v < 1 {
        conn.execute_batch(
            "ALTER TABLE games ADD COLUMN art_source TEXT;
             ALTER TABLE games ADD COLUMN art_tried_at INTEGER;
             ALTER TABLE games ADD COLUMN art_file TEXT;
             PRAGMA user_version = 1;",
        )
        .map_err(e)?;
    }
    Ok(())
}

pub fn open(app: &AppHandle) -> Result<Db, String> {
    let dir = app.path().app_local_data_dir().map_err(e)?;
    fs::create_dir_all(&dir).map_err(e)?;
    // A restore from a USB backup is applied here, before the database is opened.
    let (db_path, restore) = (dir.join("library.db"), dir.join("library.restore.db"));
    if restore.is_file() {
        if db_path.exists() {
            let _ = fs::rename(&db_path, dir.join("library.before-restore.db"));
        }
        let _ = fs::rename(&restore, &db_path);
    }
    let conn = Connection::open(db_path).map_err(e)?;
    conn.execute_batch(SCHEMA).map_err(e)?;
    migrate(&conn)?;
    Ok(Db(Arc::new(Mutex::new(conn))))
}

// ---------- detection ----------

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

enum Detected {
    System(String),
    Ambiguous(Vec<String>),
}

pub struct Detector {
    by_ext: HashMap<String, Vec<String>>,
    keys: HashMap<String, Vec<String>>,
}

impl Detector {
    pub fn new(systems: &[System]) -> Self {
        let mut by_ext: HashMap<String, Vec<String>> = HashMap::new();
        let mut keys = HashMap::new();
        for s in systems {
            for x in &s.exts {
                let list = by_ext.entry(x.to_lowercase()).or_default();
                if !list.contains(&s.id) {
                    list.push(s.id.clone());
                }
            }
            let mut k = vec![norm(&s.id), norm(&s.name)];
            k.extend(s.aliases.iter().map(|a| norm(a)));
            keys.insert(s.id.clone(), k);
        }
        Detector { by_ext, keys }
    }

    /// None = not a game file we know. Layers: unique extension, header sniff, folder name, ask.
    fn detect(&self, path: &Path) -> Option<Detected> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        let cands = self.by_ext.get(&ext)?;
        if cands.len() == 1 {
            return Some(Detected::System(cands[0].clone()));
        }
        if let Some(s) = sniff(path, &ext, cands) {
            return Some(Detected::System(s));
        }
        if let Some(s) = self.folder_hint(path, cands) {
            return Some(Detected::System(s));
        }
        Some(Detected::Ambiguous(cands.clone()))
    }

    fn folder_hint(&self, path: &Path, cands: &[String]) -> Option<String> {
        let names: Vec<String> = path
            .ancestors()
            .skip(1)
            .take(3)
            .filter_map(|a| a.file_name())
            .map(|n| norm(&n.to_string_lossy()))
            .collect();
        let mut scored: Vec<(usize, &String)> = vec![];
        for c in cands {
            let Some(keys) = self.keys.get(c) else { continue };
            let mut best = 0usize;
            for n in &names {
                for k in keys {
                    if k.len() >= 3 && n.contains(k.as_str()) {
                        best = best.max(k.len());
                    }
                }
            }
            if best > 0 {
                scored.push((best, c));
            }
        }
        let max = scored.iter().map(|x| x.0).max()?;
        let top: Vec<_> = scored.iter().filter(|x| x.0 == max).collect();
        if top.len() == 1 {
            Some(top[0].1.clone())
        } else {
            None
        }
    }
}

/// The first 40 KB of a disc's first data track (for a .cue, the first FILE it names).
fn disc_head(path: &Path, ext: &str) -> Option<Vec<u8>> {
    let target = if ext == "cue" {
        let text = String::from_utf8_lossy(&fs::read(path).ok()?).to_string();
        let re = Regex::new(r#"(?i)^\s*FILE\s+"([^"]+)""#).ok()?;
        let name = text.lines().find_map(|l| re.captures(l).map(|c| c[1].to_string()))?;
        path.parent()?.join(name)
    } else {
        path.to_path_buf()
    };
    let mut buf = vec![0u8; 0xA000];
    let n = fs::File::open(target).ok()?.read(&mut buf).ok()?;
    buf.truncate(n);
    Some(buf)
}

/// Which console a disc is for, from text written in its first sectors.
/// (Markers are from the consoles' disc formats; if a console is mis-detected you can still pick it by hand.)
fn disc_system(head: &[u8]) -> Option<&'static str> {
    let has = |needle: &[u8]| head.windows(needle.len()).any(|w| w == needle);
    if has(b"SEGA SEGASATURN") {
        return Some("saturn");
    }
    if has(b"SEGADISCSYSTEM") {
        return Some("segacd");
    }
    if has(b"PC Engine CD-ROM SYSTEM") {
        return Some("pce");
    }
    let zz = [0x01, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x01];
    if head.len() > 23 && (head[..7] == zz || head[16..23] == zz) {
        return Some("3do");
    }
    // Mega Drive / Genesis cartridge dumps say "SEGA" at 0x100.
    if head.len() > 0x104 && &head[0x100..0x104] == b"SEGA" {
        return Some("genesis");
    }
    if has(b"PC-FX:Hu_CD-ROM") {
        return Some("pcfx");
    }
    if has(b"PLAYSTATION") {
        return Some("ps1");
    }
    None
}

/// Header check for discs. GameCube magic 0xC2339F3D at 0x1C, Wii magic 0x5D1C9EA3 at 0x18.
/// (Verify against Dolphin's docs if detection ever misbehaves.)
fn sniff(path: &Path, ext: &str, cands: &[String]) -> Option<String> {
    if matches!(ext, "cue" | "iso" | "bin") {
        if let Some(head) = disc_head(path, ext) {
            if let Some(id) = disc_system(&head) {
                if cands.iter().any(|c| c == id) {
                    return Some(id.to_string());
                }
            }
        }
    }
    if ext != "iso" && ext != "gcm" {
        return None;
    }
    let mut buf = [0u8; 0x20];
    fs::File::open(path).ok()?.read_exact(&mut buf).ok()?;
    if buf[0x1C..0x20] == [0xC2, 0x33, 0x9F, 0x3D] && cands.iter().any(|c| c == "gc") {
        return Some("gc".into());
    }
    if buf[0x18..0x1C] == [0x5D, 0x1C, 0x9E, 0xA3] && cands.iter().any(|c| c == "wii") {
        return Some("wii".into());
    }
    None
}

// ---------- file collection ----------

fn key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase().replace('/', "\\")
}

fn walk(p: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    let Ok(meta) = fs::symlink_metadata(p) else { return };
    if meta.file_type().is_symlink() {
        return;
    }
    if meta.is_dir() {
        if depth > 12 {
            return;
        }
        if let Ok(rd) = fs::read_dir(p) {
            for entry in rd.flatten() {
                walk(&entry.path(), out, depth + 1);
            }
        }
    } else if meta.is_file() {
        out.push(p.to_path_buf());
    }
}

pub fn collect_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = vec![];
    for p in paths {
        walk(p, &mut out, 0);
    }
    out
}

/// Track files named by .cue / .gdi sheets belong to that game and are not games themselves.
fn referenced_tracks(files: &[PathBuf]) -> HashSet<String> {
    let cue_re = Regex::new(r#"(?i)^\s*FILE\s+"([^"]+)""#).unwrap();
    let quoted = Regex::new(r#""([^"]+)""#).unwrap();
    let mut set = HashSet::new();
    for f in files {
        let ext = f.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
        if ext != "cue" && ext != "gdi" {
            continue;
        }
        let Ok(bytes) = fs::read(f) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let Some(dir) = f.parent() else { continue };
        for (i, line) in text.lines().enumerate() {
            let name: Option<String> = if ext == "cue" {
                cue_re.captures(line).map(|c| c[1].to_string())
            } else if i == 0 {
                None
            } else if let Some(c) = quoted.captures(line) {
                Some(c[1].to_string())
            } else {
                line.split_whitespace().nth(4).map(|s| s.to_string())
            };
            if let Some(n) = name {
                set.insert(key(&dir.join(n)));
            }
        }
    }
    set
}

fn title_of(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().replace('_', " ")).unwrap_or_else(|| "Untitled".into())
}

// ---------- importing ----------

struct Row {
    path: String,
    size: i64,
    mtime: i64,
    title: String,
    detected: Detected,
}

pub fn import_files(db: &Db, det: &Detector, files: Vec<PathBuf>) -> Result<Summary, String> {
    let skip = referenced_tracks(&files);
    let mut rows = vec![];
    for f in &files {
        if skip.contains(&key(f)) {
            continue;
        }
        let Some(detected) = det.detect(f) else { continue };
        let Ok(meta) = fs::metadata(f) else { continue };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        rows.push(Row {
            path: f.to_string_lossy().to_string(),
            size: meta.len() as i64,
            mtime,
            title: title_of(f),
            detected,
        });
    }

    let mut conn = lock(db)?;
    let tx = conn.transaction().map_err(e)?;
    let mut s = Summary::default();
    let now = unix_now();
    for r in rows {
        let existing: Option<(i64, i64, i64, i64)> = tx
            .query_row(
                "SELECT size, mtime, missing, removed FROM games WHERE path = ?1",
                [&r.path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(e)?;
        if let Some((size, mtime, missing, removed)) = existing {
            if removed != 0 {
                continue; // the user removed it from the library; keep it gone
            }
            if size == r.size && mtime == r.mtime {
                if missing != 0 {
                    tx.execute("UPDATE games SET missing = 0 WHERE path = ?1", [&r.path]).map_err(e)?;
                    s.restored += 1;
                } else {
                    s.unchanged += 1;
                }
            } else {
                tx.execute(
                    "UPDATE games SET size = ?2, mtime = ?3, missing = 0 WHERE path = ?1",
                    params![r.path, r.size, r.mtime],
                )
                .map_err(e)?;
                s.updated += 1;
            }
            continue;
        }
        match r.detected {
            Detected::System(sys) => {
                tx.execute(
                    "INSERT INTO games (system_id, title, path, size, mtime, added_at) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![sys, r.title, r.path, r.size, r.mtime, now],
                )
                .map_err(e)?;
                s.added += 1;
            }
            Detected::Ambiguous(c) => {
                let json = serde_json::to_string(&c).map_err(e)?;
                s.unassigned += tx
                    .execute(
                        "INSERT OR IGNORE INTO unassigned (path, size, mtime, candidates) VALUES (?1,?2,?3,?4)",
                        params![r.path, r.size, r.mtime, json],
                    )
                    .map_err(e)?;
            }
        }
    }
    tx.commit().map_err(e)?;
    Ok(s)
}

fn refresh_missing(db: &Db, s: &mut Summary) -> Result<(), String> {
    let list: Vec<(i64, String, i64)> = {
        let conn = lock(db)?;
        let mut st = conn.prepare("SELECT id, path, missing FROM games WHERE removed = 0").map_err(e)?;
        let rows = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(e)?;
        let v: Vec<(i64, String, i64)> = rows.collect::<Result<_, _>>().map_err(e)?;
        v
    };
    let checked: Vec<(i64, bool, bool)> =
        list.iter().map(|(id, p, m)| (*id, Path::new(p).exists(), *m != 0)).collect();
    let mut conn = lock(db)?;
    let tx = conn.transaction().map_err(e)?;
    let mut missing = 0;
    for (id, exists, was_missing) in checked {
        if exists && was_missing {
            tx.execute("UPDATE games SET missing = 0 WHERE id = ?1", [id]).map_err(e)?;
            s.restored += 1;
        } else if !exists {
            if !was_missing {
                tx.execute("UPDATE games SET missing = 1 WHERE id = ?1", [id]).map_err(e)?;
            }
            missing += 1;
        }
    }
    tx.commit().map_err(e)?;
    s.missing = missing;
    Ok(())
}

fn get_folders(db: &Db) -> Result<Vec<String>, String> {
    let conn = lock(db)?;
    let v: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = 'rom_folders'", [], |r| r.get(0))
        .optional()
        .map_err(e)?;
    Ok(v.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default())
}

fn set_folders(db: &Db, folders: &[String]) -> Result<(), String> {
    let conn = lock(db)?;
    let json = serde_json::to_string(folders).map_err(e)?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('rom_folders', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [json],
    )
    .map_err(e)?;
    Ok(())
}

/// Scan every configured ROM folder (incremental), then check for missing files.
pub fn rescan_blocking(app: &AppHandle, db: &Db) -> Result<Summary, String> {
    let folders = get_folders(db)?;
    let reg = registry::load(app)?;
    let det = Detector::new(&reg.systems);
    let paths: Vec<PathBuf> = folders.iter().map(PathBuf::from).collect();
    let files = collect_files(&paths);
    let mut s = import_files(db, &det, files)?;
    refresh_missing(db, &mut s)?;
    if let Ok(mut last) = app.state::<LastScan>().0.lock() {
        *last = Some(s.clone());
    }
    let _ = app.emit("library-changed", &s);
    Ok(s)
}

// ---------- commands ----------

#[tauri::command]
pub fn list_games(db: tauri::State<'_, Db>) -> Result<Vec<Game>, String> {
    let conn = lock(&db)?;
    let mut st = conn
        .prepare(
            "SELECT id, system_id, title, path, size, added_at, last_played, total_playtime_sec, favorite, missing, art_file
             FROM games WHERE removed = 0 ORDER BY title COLLATE NOCASE",
        )
        .map_err(e)?;
    let rows = st
        .query_map([], |r| {
            Ok(Game {
                id: r.get(0)?,
                system_id: r.get(1)?,
                title: r.get(2)?,
                path: r.get(3)?,
                size: r.get(4)?,
                added_at: r.get(5)?,
                last_played: r.get(6)?,
                total_playtime_sec: r.get(7)?,
                favorite: r.get::<_, i64>(8)? != 0,
                missing: r.get::<_, i64>(9)? != 0,
                art_path: r.get(10)?,
            })
        })
        .map_err(e)?;
    let out: Vec<Game> = rows.collect::<Result<_, _>>().map_err(e)?;
    Ok(out)
}

#[tauri::command]
pub fn list_unassigned(db: tauri::State<'_, Db>) -> Result<Vec<Unassigned>, String> {
    let conn = lock(&db)?;
    let mut st = conn
        .prepare("SELECT path, candidates FROM unassigned WHERE dismissed = 0 ORDER BY path")
        .map_err(e)?;
    let rows = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(e)?;
    let mut out = vec![];
    for row in rows {
        let (path, c) = row.map_err(e)?;
        let file_name = Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        out.push(Unassigned { path, file_name, candidates: serde_json::from_str(&c).unwrap_or_default() });
    }
    Ok(out)
}

#[tauri::command]
pub fn last_scan(last: tauri::State<'_, LastScan>) -> Option<Summary> {
    last.0.lock().ok().and_then(|l| l.clone())
}

#[tauri::command]
pub fn get_rom_folders(db: tauri::State<'_, Db>) -> Result<Vec<String>, String> {
    get_folders(&db)
}

#[tauri::command]
pub async fn import_paths(app: AppHandle, db: tauri::State<'_, Db>, paths: Vec<String>) -> Result<Summary, String> {
    let db = db.inner().clone();
    let reg = registry::load(&app)?;
    let s = tokio::task::spawn_blocking(move || {
        let det = Detector::new(&reg.systems);
        let files = collect_files(&paths.iter().map(PathBuf::from).collect::<Vec<_>>());
        import_files(&db, &det, files)
    })
    .await
    .map_err(e)??;
    let _ = app.emit("library-changed", &s);
    Ok(s)
}

#[tauri::command]
pub async fn add_rom_folder(app: AppHandle, db: tauri::State<'_, Db>, path: String) -> Result<Summary, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let mut folders = get_folders(&db)?;
        if !folders.iter().any(|f| key(Path::new(f)) == key(Path::new(&path))) {
            folders.push(path);
            set_folders(&db, &folders)?;
        }
        rescan_blocking(&app, &db)
    })
    .await
    .map_err(e)?
}

#[tauri::command]
pub async fn remove_rom_folder(db: tauri::State<'_, Db>, path: String) -> Result<(), String> {
    let folders: Vec<String> = get_folders(&db)?.into_iter().filter(|f| key(Path::new(f)) != key(Path::new(&path))).collect();
    set_folders(&db, &folders)
}

#[tauri::command]
pub async fn rescan(app: AppHandle, db: tauri::State<'_, Db>) -> Result<Summary, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || rescan_blocking(&app, &db)).await.map_err(e)?
}

#[tauri::command]
pub fn assign_system(app: AppHandle, db: tauri::State<'_, Db>, path: String, system_id: String) -> Result<(), String> {
    let reg = registry::load(&app)?;
    if !reg.systems.iter().any(|s| s.id == system_id) {
        return Err("unknown system".into());
    }
    let mut conn = lock(&db)?;
    let tx = conn.transaction().map_err(e)?;
    let (size, mtime): (i64, i64) = tx
        .query_row("SELECT size, mtime FROM unassigned WHERE path = ?1", [&path], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|_| "that file is no longer waiting for a system".to_string())?;
    tx.execute(
        "INSERT OR IGNORE INTO games (system_id, title, path, size, mtime, added_at) VALUES (?1,?2,?3,?4,?5,?6)",
        params![system_id, title_of(Path::new(&path)), path, size, mtime, unix_now()],
    )
    .map_err(e)?;
    tx.execute("DELETE FROM unassigned WHERE path = ?1", [&path]).map_err(e)?;
    tx.commit().map_err(e)
}

#[tauri::command]
pub fn dismiss_unassigned(db: tauri::State<'_, Db>, path: String) -> Result<(), String> {
    let conn = lock(&db)?;
    conn.execute("UPDATE unassigned SET dismissed = 1 WHERE path = ?1", [path]).map_err(e)?;
    Ok(())
}

/// Removes the game from the library only. The file on disk is never touched.
#[tauri::command]
pub fn remove_game(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = lock(&db)?;
    conn.execute("UPDATE games SET removed = 1 WHERE id = ?1", [id]).map_err(e)?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedGame {
    id: i64,
    title: String,
    system_id: String,
    path: String,
}

/// Games you removed from the library earlier, so they can be brought back.
#[tauri::command]
pub fn list_removed_games(db: tauri::State<'_, Db>) -> Result<Vec<RemovedGame>, String> {
    let conn = lock(&db)?;
    let mut st = conn
        .prepare("SELECT id, title, system_id, path FROM games WHERE removed = 1 ORDER BY title COLLATE NOCASE")
        .map_err(e)?;
    let rows = st
        .query_map([], |r| Ok(RemovedGame { id: r.get(0)?, title: r.get(1)?, system_id: r.get(2)?, path: r.get(3)? }))
        .map_err(e)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(e)
}

#[tauri::command]
pub fn restore_game(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = lock(&db)?;
    conn.execute("UPDATE games SET removed = 0 WHERE id = ?1", [id]).map_err(e)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use registry::Registry;

    fn det() -> Detector {
        let reg: Registry = serde_json::from_str(include_str!("../resources/emulators.json")).unwrap();
        Detector::new(&reg.systems)
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("trayload-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn disc_markers() {
        let mut v = vec![0u8; 64];
        v[16..31].copy_from_slice(b"SEGA SEGASATURN");
        assert_eq!(disc_system(&v), Some("saturn"));
        let mut s = vec![0u8; 64];
        s[..14].copy_from_slice(b"SEGADISCSYSTEM");
        assert_eq!(disc_system(&s), Some("segacd"));
        let mut t = vec![0u8; 64];
        t[16..23].copy_from_slice(&[1, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 1]);
        assert_eq!(disc_system(&t), Some("3do"));
        let mut g = vec![0u8; 0x200];
        g[0x100..0x110].copy_from_slice(b"SEGA MEGA DRIVE ");
        assert_eq!(disc_system(&g), Some("genesis"));
        assert_eq!(disc_system(&[0u8; 64]), None);
    }

    #[test]
    fn unique_extension_is_decided() {
        match det().detect(Path::new("C:/roms/Game.gba")) {
            Some(Detected::System(s)) => assert_eq!(s, "gba"),
            _ => panic!("gba should map to one system"),
        }
        assert!(det().detect(Path::new("C:/roms/readme.txt")).is_none());
    }

    #[test]
    fn iso_uses_folder_hint_or_asks() {
        let d = det();
        match d.detect(Path::new("C:/roms/PS2/Some Game.iso")) {
            Some(Detected::System(s)) => assert_eq!(s, "ps2"),
            _ => panic!("PS2 folder should decide"),
        }
        assert!(matches!(d.detect(Path::new("C:/stuff/Some Game.iso")), Some(Detected::Ambiguous(_))));
    }

    #[test]
    fn gamecube_header_is_sniffed() {
        let dir = tmp("gc");
        let f = dir.join("disc.iso");
        let mut data = vec![0u8; 0x40];
        data[0x1C..0x20].copy_from_slice(&[0xC2, 0x33, 0x9F, 0x3D]);
        fs::write(&f, data).unwrap();
        match det().detect(&f) {
            Some(Detected::System(s)) => assert_eq!(s, "gc"),
            _ => panic!("GC magic should decide"),
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cue_tracks_are_not_separate_games() {
        let dir = tmp("cue");
        fs::write(dir.join("Game.cue"), "FILE \"Game (Track 1).bin\" BINARY\n").unwrap();
        fs::write(dir.join("Game (Track 1).bin"), b"x").unwrap();
        let files = collect_files(&[dir.clone()]);
        let skip = referenced_tracks(&files);
        assert!(skip.contains(&key(&dir.join("Game (Track 1).bin"))));
        let _ = fs::remove_dir_all(dir);
    }
}
