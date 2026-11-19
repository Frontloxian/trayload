//! Emulator registry: data, not code.
//! Loads `emulators.json`. A copy is embedded in the binary; if a file named
//! `registry.json` exists in the app's local data folder it wins, so the list
//! can be updated later without shipping a new app version.

use serde::{Deserialize, Serialize};
use std::fs;
use tauri::{AppHandle, Manager};

const EMBEDDED: &str = include_str!("../resources/emulators.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Source {
    /// Latest GitHub release; pick the asset whose name matches `assetPattern`.
    Github {
        repo: String,
        #[serde(rename = "assetPattern")]
        asset_pattern: String,
    },
    /// Vendor download page (resolved later by the installer).
    Page {
        url: String,
        #[serde(rename = "assetPattern", default)]
        asset_pattern: Option<String>,
        #[serde(rename = "fallbackUrl", default)]
        fallback_url: Option<String>,
    },
}

fn yes() -> bool {
    true
}

/// One thing a system needs (or can optionally use), such as a BIOS file.
/// Matched by file name (regex) and, where known, exact file size. Never by downloading anything.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupItem {
    pub label: String,
    #[serde(default = "yes")]
    pub required: bool,
    #[serde(default)]
    pub name_pattern: Option<String>,
    #[serde(default)]
    pub sizes: Option<Vec<u64>>,
    #[serde(default)]
    pub note: Option<String>,
    /// Also look inside sub-folders (up to 3 levels deep).
    #[serde(default)]
    pub deep: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupDef {
    /// Folder to check, relative to the emulator's own folder.
    pub folder: String,
    #[serde(default)]
    pub intro: Option<String>,
    pub items: Vec<SetupItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct System {
    pub id: String,
    pub name: String,
    pub emulator: String,
    pub source: Source,
    pub exts: Vec<String>,
    /// Case-insensitive regex for the emulator's main .exe file name.
    #[serde(rename = "exePattern", default)]
    pub exe_pattern: Option<String>,
    /// Empty file created next to the .exe so the emulator keeps its data in its own folder.
    #[serde(rename = "portableMarker", default)]
    pub portable_marker: Option<String>,
    /// Command-line arguments to start a game. The element "{rom}" is replaced by the game path.
    #[serde(rename = "launchArgs", default)]
    pub launch_args: Option<Vec<String>>,
    /// libretro-thumbnails repository names for this system's box art.
    #[serde(rename = "artRepos", default)]
    pub art_repos: Option<Vec<String>>,
    /// What the setup checklist looks for (BIOS files and similar).
    #[serde(default)]
    pub setup: Option<SetupDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub systems: Vec<System>,
}

fn parse(text: &str) -> Result<Registry, String> {
    serde_json::from_str(text).map_err(|e| format!("registry parse error: {e}"))
}

pub fn load(app: &AppHandle) -> Result<Registry, String> {
    if let Ok(dir) = app.path().app_local_data_dir() {
        let path = dir.join("registry.json");
        if path.exists() {
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            return parse(&text);
        }
    }
    parse(EMBEDDED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_parses() {
        let r = parse(EMBEDDED).expect("embedded registry must parse");
        assert!(!r.systems.is_empty());
        assert!(r.systems.iter().all(|s| !s.id.is_empty() && !s.exts.is_empty()));
    }

    #[test]
    fn ids_are_unique() {
        let r = parse(EMBEDDED).unwrap();
        let mut ids: Vec<_> = r.systems.iter().map(|s| s.id.as_str()).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(n, ids.len(), "duplicate system id in registry");
    }
}
