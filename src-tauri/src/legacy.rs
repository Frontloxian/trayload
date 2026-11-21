//! Clean-up of the RetroArch install that older Trayload versions made.
//! Trayload 1.2.0 no longer uses RetroArch. Nothing here runs by itself: the person is asked first,
//! can back up their saves, save states and BIOS folders, and decides whether to remove the folder.

use crate::installer;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;

const RETROARCH_DIR: &str = "retroarch";
/// Folders worth keeping: saves, save states and BIOS / system files.
const KEEP: &[&str] = &["saves", "states", "system"];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyFolder {
    name: String,
    bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyStatus {
    present: bool,
    path: String,
    total_bytes: u64,
    /// The saves / states / system folders that exist and have something in them.
    folders: Vec<LegacyFolder>,
}

fn dir_size(p: &Path, depth: usize) -> u64 {
    let Ok(meta) = fs::symlink_metadata(p) else { return 0 };
    if meta.file_type().is_symlink() || depth > 14 {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    fs::read_dir(p)
        .map(|rd| rd.flatten().map(|e| dir_size(&e.path(), depth + 1)).sum())
        .unwrap_or(0)
}

fn copy_tree(src: &Path, dst: &Path, depth: usize) -> Result<(), String> {
    let meta = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() || depth > 14 {
        return Ok(());
    }
    if meta.is_dir() {
        fs::create_dir_all(dst).map_err(|e| format!("could not create {}: {e}", dst.display()))?;
        for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
            copy_tree(&entry.path(), &dst.join(entry.file_name()), depth + 1)?;
        }
    } else if meta.is_file() {
        fs::copy(src, dst).map_err(|e| format!("could not copy {}: {e}", src.display()))?;
    }
    Ok(())
}

/// Small folders that older versions made for each core. They only point at RetroArch.
fn stub_dirs(root: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    if let Ok(rd) = fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            if e.file_name() == RETROARCH_DIR || !p.is_dir() {
                continue;
            }
            let is_stub = installer::read_installed(&p)
                .map(|i| i.exe_rel_path.starts_with(&format!("../{RETROARCH_DIR}/")))
                .unwrap_or(false);
            if is_stub {
                v.push(p);
            }
        }
    }
    v
}

#[tauri::command]
pub fn legacy_status(app: AppHandle) -> Result<LegacyStatus, String> {
    let root = installer::emulators_dir(&app)?;
    let ra = root.join(RETROARCH_DIR);
    let present = ra.is_dir() || !stub_dirs(&root).is_empty();
    let mut folders = vec![];
    if ra.is_dir() {
        for name in KEEP {
            let bytes = dir_size(&ra.join(name), 0);
            if bytes > 0 {
                folders.push(LegacyFolder { name: name.to_string(), bytes });
            }
        }
    }
    Ok(LegacyStatus {
        present,
        path: ra.to_string_lossy().to_string(),
        total_bytes: if ra.is_dir() { dir_size(&ra, 0) } else { 0 },
        folders,
    })
}

/// Copies RetroArch's saves, states and system folders into a new folder inside `dest`.
/// Returns the folder that was created.
#[tauri::command]
pub async fn legacy_backup(app: AppHandle, dest: String) -> Result<String, String> {
    let root = installer::emulators_dir(&app)?;
    tokio::task::spawn_blocking(move || {
        let ra = root.join(RETROARCH_DIR);
        if !ra.is_dir() {
            return Err("There is no RetroArch folder to back up.".to_string());
        }
        let dest = PathBuf::from(dest);
        if !dest.is_dir() {
            return Err("Pick an existing folder for the backup.".to_string());
        }
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let mut out = dest.join(format!("RetroArch backup {}", secs / 86400));
        let mut n = 2;
        while out.exists() {
            out = dest.join(format!("RetroArch backup {} ({n})", secs / 86400));
            n += 1;
        }
        let mut copied = 0;
        for name in KEEP {
            let src = ra.join(name);
            if src.is_dir() && dir_size(&src, 0) > 0 {
                copy_tree(&src, &out.join(name), 0)?;
                copied += 1;
            }
        }
        if copied == 0 {
            return Err("RetroArch has no saves, states or BIOS files to back up.".to_string());
        }
        Ok(out.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Removes the RetroArch folder and the small per-core folders. Only called after the person agreed.
#[tauri::command]
pub fn legacy_remove(app: AppHandle) -> Result<(), String> {
    let root = installer::emulators_dir(&app)?;
    for d in stub_dirs(&root) {
        installer::remove_dir_robust(&d)?;
    }
    for name in [RETROARCH_DIR.to_string(), format!("{RETROARCH_DIR}.staging"), format!("{RETROARCH_DIR}.prev")] {
        let d = root.join(name);
        if d.exists() {
            installer::remove_dir_robust(&d)?;
        }
    }
    crate::applog!("[legacy] removed the old RetroArch folder");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_a_tree_and_measures_it() {
        let d = std::env::temp_dir().join(format!("trayload-legacy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("a/b")).unwrap();
        fs::write(d.join("a/b/x.sav"), b"12345").unwrap();
        assert_eq!(dir_size(&d.join("a"), 0), 5);
        copy_tree(&d.join("a"), &d.join("out"), 0).unwrap();
        assert_eq!(fs::read(d.join("out/b/x.sav")).unwrap(), b"12345");
        let _ = fs::remove_dir_all(&d);
    }
}
