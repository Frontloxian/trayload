//! USB backup: copies saves, save states and emulator settings (and optionally save files that
//! sit next to your games, plus your library data) into a "Trayload Backup" folder, and back.
//! Copies only; nothing is ever deleted from the backup or from your PC except overwritten files on restore.

use crate::{installer, library::Db, registry};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

const FOLDER: &str = "Trayload Backup";
const GAME_SAVE_EXTS: &[&str] = &["sav", "srm", "dsv", "eep", "fla", "sra", "mpk", "mcr"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub key: String,
    pub name: String,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub created_at: u64,
    pub app_version: String,
    pub sources: Vec<SourceInfo>,
    /// ROM folders whose save files were copied: (index, original path).
    pub game_folders: Vec<(usize, String)>,
    pub game_files: u64,
    pub library: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    phase: String,
    done: u64,
    current: String,
}

/// One emulator install whose data we know how to find.
struct Source {
    key: String,
    name: String,
    base: PathBuf,
    rels: Vec<String>,
}

fn safe_rel(p: &str) -> bool {
    !p.is_empty() && Path::new(p).components().all(|c| matches!(c, Component::Normal(_)))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Installed emulators that declare backup paths, merged by the folder they live in
/// (all RetroArch cores share one folder, for example).
fn sources(app: &AppHandle) -> Result<Vec<Source>, String> {
    let reg = registry::load(app)?;
    let root = installer::emulators_dir(app)?;
    let canon_root = fs::canonicalize(&root).unwrap_or(root.clone());
    let mut map: BTreeMap<String, Source> = BTreeMap::new();
    for s in &reg.systems {
        if s.backup.is_empty() {
            continue;
        }
        let id = installer::slug(&s.emulator);
        let Ok(dir) = installer::emu_dir(app, &id) else { continue };
        let Some(info) = installer::read_installed(&dir) else { continue };
        let Ok(exe) = fs::canonicalize(dir.join(&info.exe_rel_path)) else { continue };
        let Some(base) = exe.parent().map(Path::to_path_buf) else { continue };
        let key = base
            .strip_prefix(&canon_root)
            .map(|p| p.to_string_lossy().replace(['\\', '/'], "~"))
            .unwrap_or(id.clone());
        let name = if s.tier.as_deref() == Some("core") { "RetroArch".to_string() } else { s.emulator.clone() };
        let e = map.entry(key.clone()).or_insert(Source { key, name, base, rels: vec![] });
        for r in &s.backup {
            if safe_rel(r) && !e.rels.contains(r) {
                e.rels.push(r.clone());
            }
        }
    }
    Ok(map.into_values().collect())
}

fn walk_files(p: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    let Ok(meta) = fs::symlink_metadata(p) else { return };
    if meta.file_type().is_symlink() || depth > 14 {
        return;
    }
    if meta.is_dir() {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                walk_files(&e.path(), out, depth + 1);
            }
        }
    } else if meta.is_file() {
        out.push(p.to_path_buf());
    }
}

fn needs_copy(src: &Path, dst: &Path) -> bool {
    match (fs::metadata(src), fs::metadata(dst)) {
        (Ok(a), Ok(b)) => a.len() != b.len() || a.modified().ok() > b.modified().ok(),
        _ => true,
    }
}

fn copy_file(src: &Path, dst: &Path) -> Result<bool, String> {
    if !needs_copy(src, dst) {
        return Ok(false);
    }
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::copy(src, dst).map_err(|e| format!("could not copy {}: {e}", src.display()))?;
    Ok(true)
}

/// All files of one source that exist right now, as (absolute path, path relative to base).
fn source_files(s: &Source) -> Vec<(PathBuf, PathBuf)> {
    let mut out = vec![];
    for r in &s.rels {
        let mut found = vec![];
        walk_files(&s.base.join(r), &mut found, 0);
        for f in found {
            if let Ok(rel) = f.strip_prefix(&s.base) {
                out.push((f.clone(), rel.to_path_buf()));
            }
        }
    }
    out
}

fn game_save_files(folder: &str) -> Vec<(PathBuf, PathBuf)> {
    let base = PathBuf::from(folder);
    let mut all = vec![];
    walk_files(&base, &mut all, 0);
    all.into_iter()
        .filter(|f| {
            f.extension()
                .and_then(|e| e.to_str())
                .map(|e| GAME_SAVE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
                .unwrap_or(false)
        })
        .filter_map(|f| f.strip_prefix(&base).ok().map(|r| (f.clone(), r.to_path_buf())))
        .collect()
}

fn rom_folders(db: &Db) -> Vec<String> {
    let Ok(conn) = db.0.lock() else { return vec![] };
    conn.query_row("SELECT value FROM settings WHERE key = 'rom_folders'", [], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    sources: Vec<SourceInfo>,
    game_files: u64,
}

#[tauri::command]
pub async fn backup_preview(app: AppHandle, db: tauri::State<'_, Db>) -> Result<Preview, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let mut out = vec![];
        for s in sources(&app)? {
            let files = source_files(&s);
            if files.is_empty() {
                continue;
            }
            let bytes = files.iter().filter_map(|(a, _)| fs::metadata(a).ok()).map(|m| m.len()).sum();
            out.push(SourceInfo { key: s.key.clone(), name: s.name.clone(), files: files.len() as u64, bytes });
        }
        let game_files = rom_folders(&db).iter().map(|f| game_save_files(f).len() as u64).sum();
        Ok(Preview { sources: out, game_files })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn backup_root(dest: &str) -> Result<PathBuf, String> {
    let d = PathBuf::from(dest);
    if !d.is_dir() {
        return Err("pick a folder or drive that exists".into());
    }
    if d.join("manifest.json").is_file() {
        return Ok(d);
    }
    Ok(d.join(FOLDER))
}

#[tauri::command]
pub fn backup_info(dest: String) -> Option<Manifest> {
    let root = backup_root(&dest).ok()?;
    serde_json::from_str(&fs::read_to_string(root.join("manifest.json")).ok()?).ok()
}

fn emit(app: &AppHandle, phase: &str, done: u64, current: &str) {
    let _ = app.emit("backup-progress", Progress { phase: phase.into(), done, current: current.into() });
}

#[tauri::command]
pub async fn backup_run(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    dest: String,
    game_saves: bool,
) -> Result<Manifest, String> {
    let db = db.inner().clone();
    let res = tokio::task::spawn_blocking(move || -> Result<Manifest, String> {
        let root = backup_root(&dest)?;
        fs::create_dir_all(&root).map_err(|e| format!("cannot write to that location: {e}"))?;
        let mut done = 0u64;
        let mut infos = vec![];
        for s in sources(&app)? {
            let files = source_files(&s);
            if files.is_empty() {
                continue;
            }
            let (mut n, mut bytes) = (0u64, 0u64);
            for (abs, rel) in &files {
                copy_file(abs, &root.join("emulators").join(&s.key).join(rel))?;
                n += 1;
                bytes += fs::metadata(abs).map(|m| m.len()).unwrap_or(0);
                done += 1;
                if done % 20 == 0 {
                    emit(&app, "copying", done, &s.name);
                }
            }
            infos.push(SourceInfo { key: s.key.clone(), name: s.name.clone(), files: n, bytes });
        }
        let mut game_folders = vec![];
        let mut game_files = 0u64;
        if game_saves {
            for (i, folder) in rom_folders(&db).iter().enumerate() {
                let files = game_save_files(folder);
                if files.is_empty() {
                    continue;
                }
                for (abs, rel) in &files {
                    copy_file(abs, &root.join("games").join(i.to_string()).join(rel))?;
                    game_files += 1;
                    done += 1;
                }
                game_folders.push((i, folder.clone()));
            }
        }
        // Library data (favorites, playtime, collections) as a consistent copy.
        emit(&app, "library", done, "library");
        let lib = root.join("library.db");
        let _ = fs::remove_file(&lib);
        let library = {
            let conn = db.0.lock().map_err(|_| "database lock poisoned".to_string())?;
            conn.execute("VACUUM INTO ?1", [lib.to_string_lossy().to_string()]).is_ok()
        };
        let m = Manifest {
            created_at: now(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            sources: infos,
            game_folders,
            game_files,
            library,
        };
        fs::write(root.join("manifest.json"), serde_json::to_vec_pretty(&m).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        crate::applog!("[backup] wrote {} emulator sources, {} game save files to {}", m.sources.len(), m.game_files, root.display());
        Ok(m)
    })
    .await
    .map_err(|e| e.to_string())?;
    res
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSummary {
    restored: Vec<String>,
    skipped: Vec<String>,
    files: u64,
    library_pending: bool,
}

#[tauri::command]
pub async fn restore_run(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    dest: String,
    restore_library: bool,
) -> Result<RestoreSummary, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<RestoreSummary, String> {
        let root = backup_root(&dest)?;
        let m: Manifest = serde_json::from_str(
            &fs::read_to_string(root.join("manifest.json")).map_err(|_| "no Trayload backup found there".to_string())?,
        )
        .map_err(|_| "that backup's manifest can't be read".to_string())?;
        let installed = sources(&app)?;
        let (mut restored, mut skipped, mut files) = (vec![], vec![], 0u64);
        for info in &m.sources {
            let Some(s) = installed.iter().find(|s| s.key == info.key) else {
                skipped.push(format!("{} (not installed)", info.name));
                continue;
            };
            let src_root = root.join("emulators").join(&info.key);
            let mut all = vec![];
            walk_files(&src_root, &mut all, 0);
            for f in all {
                let Ok(rel) = f.strip_prefix(&src_root) else { continue };
                if !safe_rel(&rel.to_string_lossy().replace('\\', "/")) {
                    continue;
                }
                copy_overwrite(&f, &s.base.join(rel))?;
                files += 1;
            }
            restored.push(s.name.clone());
        }
        let current = rom_folders(&db);
        for (i, original) in &m.game_folders {
            if !current.contains(original) {
                skipped.push(format!("game saves for {original} (folder is not in your library)"));
                continue;
            }
            let src_root = root.join("games").join(i.to_string());
            let mut all = vec![];
            walk_files(&src_root, &mut all, 0);
            for f in all {
                let Ok(rel) = f.strip_prefix(&src_root) else { continue };
                if !safe_rel(&rel.to_string_lossy().replace('\\', "/")) {
                    continue;
                }
                copy_overwrite(&f, &Path::new(original).join(rel))?;
                files += 1;
            }
            restored.push(format!("game saves in {original}"));
        }
        let mut library_pending = false;
        if restore_library && root.join("library.db").is_file() {
            let target = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("library.restore.db");
            fs::copy(root.join("library.db"), target).map_err(|e| e.to_string())?;
            library_pending = true;
        }
        crate::applog!("[backup] restored {} files from {}", files, root.display());
        Ok(RestoreSummary { restored, skipped, files, library_pending })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn copy_overwrite(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::copy(src, dst).map_err(|e| format!("could not restore {}: {e}", dst.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_paths() {
        assert!(safe_rel("User/GC"));
        assert!(!safe_rel("../x"));
        assert!(!safe_rel(""));
        assert!(!safe_rel("/abs"));
    }

    #[test]
    fn copy_only_when_changed() {
        let d = std::env::temp_dir().join(format!("trayload-bk-{}", now()));
        fs::create_dir_all(&d).unwrap();
        let (a, b) = (d.join("a.sav"), d.join("sub").join("a.sav"));
        fs::write(&a, b"one").unwrap();
        assert!(copy_file(&a, &b).unwrap());
        assert!(!copy_file(&a, &b).unwrap());
        fs::write(&a, b"three").unwrap();
        assert!(copy_file(&a, &b).unwrap());
        let _ = fs::remove_dir_all(&d);
    }
}
