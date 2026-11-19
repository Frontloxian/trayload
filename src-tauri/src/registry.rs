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
        /// Optional regex run on the asset name. Its first capture group becomes the version label.
        #[serde(rename = "versionRegex", default)]
        version_regex: Option<String>,
    },
    /// Latest stable release from a Gitea or Forgejo server (same JSON shape as GitHub's).
    Forgejo {
        host: String,
        repo: String,
        #[serde(rename = "assetPattern")]
        asset_pattern: String,
        #[serde(rename = "versionRegex", default)]
        version_regex: Option<String>,
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
    /// "experimental" entries are hidden behind an opt-in and need a confirmation to install.
    #[serde(default)]
    pub tier: Option<String>,
    /// Shown on the emulator's card and in the install confirmation.
    #[serde(default)]
    pub warning: Option<String>,
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
        assert!(r
            .systems
            .iter()
            .all(|s| !s.id.is_empty() && (!s.exts.is_empty() || s.tier.as_deref() == Some("experimental"))));
    }

    #[test]
    fn registry_entries_are_well_formed() {
        use regex::Regex;
        use std::path::{Component, Path};
        let r = parse(EMBEDDED).unwrap();
        for s in &r.systems {
            if let Some(p) = &s.exe_pattern {
                assert!(Regex::new(p).is_ok(), "bad exePattern for {}", s.id);
            }
            match &s.source {
                Source::Github { asset_pattern, version_regex, .. } => {
                    assert!(Regex::new(asset_pattern).is_ok(), "bad assetPattern for {}", s.id);
                    if let Some(v) = version_regex {
                        assert!(Regex::new(v).is_ok(), "bad versionRegex for {}", s.id);
                    }
                }
                Source::Page { asset_pattern: Some(p), .. } => {
                    assert!(Regex::new(p).is_ok(), "bad assetPattern for {}", s.id);
                }
                Source::Forgejo { asset_pattern, .. } => {
                    assert!(Regex::new(asset_pattern).is_ok(), "bad assetPattern for {}", s.id);
                }
                _ => {}
            }
            if let Some(a) = &s.launch_args {
                assert_eq!(a.iter().filter(|x| *x == "{rom}").count(), 1, "launchArgs of {} needs one {{rom}}", s.id);
            }
            if let Some(m) = &s.portable_marker {
                let t = m.trim_end_matches('/');
                assert!(!t.is_empty() && Path::new(t).components().all(|c| matches!(c, Component::Normal(_))));
            }
        }
    }

    #[test]
    fn systems_sharing_an_emulator_agree() {
        let r = parse(EMBEDDED).unwrap();
        let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for s in &r.systems {
            let key = crate::installer::slug(&s.emulator);
            let sig = format!(
                "{}|{:?}|{:?}",
                serde_json::to_string(&s.source).unwrap(),
                s.exe_pattern,
                s.portable_marker
            );
            if let Some(prev) = seen.insert(key.clone(), sig.clone()) {
                assert_eq!(prev, sig, "systems sharing {key} must have the same source, exePattern and marker");
            }
        }
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
