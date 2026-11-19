//! Setup checklist: looks for the files each emulator needs (BIOS and similar) in its own folder.
//! Presence only (name and size). Trayload never supplies, downloads or links these files.

use crate::{installer, registry::{self, SetupItem, System}};
use regex::RegexBuilder;
use serde::Serialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use tauri::AppHandle;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStatus {
    label: String,
    required: bool,
    found: bool,
    matched: Option<String>,
    note: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSetup {
    system_id: String,
    system_name: String,
    emulator: String,
    installed: bool,
    installable: bool,
    /// "none" (nothing to set up), "notinstalled", "needs" or "ready"
    state: String,
    intro: Option<String>,
    folder: Option<String>,
    items: Vec<ItemStatus>,
}

fn safe_rel(p: &str) -> bool {
    let path = Path::new(p);
    path.components().next().is_some() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

fn find_match(folder: &Path, item: &SetupItem) -> Option<String> {
    let re = RegexBuilder::new(item.name_pattern.as_deref().unwrap_or("."))
        .case_insensitive(true)
        .build()
        .ok()?;
    let mut stack = vec![(folder.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            let Ok(meta) = fs::symlink_metadata(&p) else { continue };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                if item.deep && depth < 3 {
                    stack.push((p, depth + 1));
                }
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if meta.len() == 0 || !re.is_match(&name) {
                continue;
            }
            if let Some(sizes) = &item.sizes {
                if !sizes.contains(&meta.len()) {
                    continue;
                }
            }
            return Some(name);
        }
    }
    None
}

/// The folder a system's files belong in, or None if its emulator isn't installed.
fn resolve_folder(app: &AppHandle, sys: &System) -> Result<Option<PathBuf>, String> {
    let Some(def) = &sys.setup else { return Ok(None) };
    if !safe_rel(&def.folder) {
        return Err(format!("the registry has an unsafe setup folder for {}", sys.name));
    }
    let dir = installer::emu_dir(app, &installer::slug(&sys.emulator))?;
    let Some(info) = installer::read_installed(&dir) else { return Ok(None) };
    let exe = dir.join(&info.exe_rel_path);
    let base = exe.parent().unwrap_or(&dir);
    Ok(Some(base.join(&def.folder)))
}

#[tauri::command]
pub fn check_setup(app: AppHandle) -> Result<Vec<SystemSetup>, String> {
    let reg = registry::load(&app)?;
    let mut out = vec![];
    for s in &reg.systems {
        let id = installer::slug(&s.emulator);
        let installed = installer::emu_dir(&app, &id)
            .ok()
            .and_then(|d| installer::read_installed(&d))
            .is_some();
        let installable = s.exe_pattern.is_some();
        let folder = resolve_folder(&app, s)?;
        let mut items = vec![];
        let mut state = "none".to_string();
        if let Some(def) = &s.setup {
            let mut all_required_found = true;
            for it in &def.items {
                let matched = folder.as_ref().and_then(|f| find_match(f, it));
                if it.required && matched.is_none() {
                    all_required_found = false;
                }
                items.push(ItemStatus {
                    label: it.label.clone(),
                    required: it.required,
                    found: matched.is_some(),
                    matched,
                    note: it.note.clone(),
                });
            }
            state = if !installed {
                "notinstalled"
            } else if all_required_found {
                "ready"
            } else {
                "needs"
            }
            .to_string();
        }
        out.push(SystemSetup {
            system_id: s.id.clone(),
            system_name: s.name.clone(),
            emulator: s.emulator.clone(),
            installed,
            installable,
            state,
            intro: s.setup.as_ref().and_then(|d| d.intro.clone()),
            folder: folder.map(|f| f.to_string_lossy().to_string()),
            items,
        });
    }
    Ok(out)
}

/// Opens the system's folder in Explorer (creating it if needed) so you can drop files in.
#[tauri::command]
pub fn open_setup_folder(app: AppHandle, system_id: String) -> Result<(), String> {
    let reg = registry::load(&app)?;
    let sys = reg.systems.iter().find(|s| s.id == system_id).ok_or("unknown system")?;
    let folder = resolve_folder(&app, sys)?.ok_or("Install the emulator first.")?;
    fs::create_dir_all(&folder).map_err(|e| format!("could not create {}: {e}", folder.display()))?;
    std::process::Command::new("explorer")
        .arg(&folder)
        .spawn()
        .map_err(|e| format!("could not open Explorer: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("trayload-setup-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn item(pattern: Option<&str>, sizes: Option<Vec<u64>>, deep: bool) -> SetupItem {
        SetupItem {
            label: "x".into(),
            required: true,
            name_pattern: pattern.map(|s| s.to_string()),
            sizes,
            note: None,
            deep,
        }
    }

    #[test]
    fn relative_folders_only() {
        assert!(safe_rel("bios"));
        assert!(safe_rel("User/GC"));
        assert!(!safe_rel("../outside"));
        assert!(!safe_rel("C:\\Windows"));
        assert!(!safe_rel(""));
    }

    #[test]
    fn matches_by_name_and_size() {
        let d = tmp("size");
        fs::write(d.join("a.bin"), vec![1u8; 10]).unwrap();
        fs::write(d.join("b.bin"), vec![1u8; 20]).unwrap();
        assert_eq!(find_match(&d, &item(None, Some(vec![20]), false)), Some("b.bin".into()));
        assert_eq!(find_match(&d, &item(None, Some(vec![99]), false)), None);
        assert_eq!(find_match(&d, &item(Some("^A\\.BIN$"), None, false)), Some("a.bin".into()));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn deep_search_finds_nested_files_only_when_asked() {
        let d = tmp("deep");
        fs::create_dir_all(d.join("USA")).unwrap();
        fs::write(d.join("USA").join("IPL.bin"), vec![1u8; 5]).unwrap();
        assert!(find_match(&d, &item(Some("^IPL\\.bin$"), None, false)).is_none());
        assert!(find_match(&d, &item(Some("^IPL\\.bin$"), None, true)).is_some());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn registry_setup_folders_are_safe() {
        let reg: registry::Registry = serde_json::from_str(include_str!("../resources/emulators.json")).unwrap();
        for s in &reg.systems {
            if let Some(def) = &s.setup {
                assert!(safe_rel(&def.folder), "unsafe folder for {}", s.id);
                assert!(!def.items.is_empty());
            }
        }
    }
}
