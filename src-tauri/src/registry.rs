//! Emulator registry: data, not code.
//! Loads `emulators.json`. A copy is embedded in the binary. Newer copies win, in this order:
//! 1. `registry.json` in the app's local data folder (for people who edit it by hand),
//! 2. `registry.remote.json`, saved by remote.rs from the project's GitHub page, but only if its
//!    `revision` is higher than the embedded one and it passes every check below.
//! A downloaded list can fix patterns and launch options. It can never point an emulator at a
//! different project: sources must match the ones built into this version of Trayload.

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
    /// A fixed official download address. The version is read from the server's Last-Modified date.
    Direct { url: String },
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
    /// Other names for the system, used to recognise folder names like "Mega Drive".
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Folders and files (relative to the emulator's folder) holding saves, states and settings.
    #[serde(default)]
    pub backup: Vec<String>,
    /// Shown on the emulator's card and in the install confirmation.
    #[serde(default)]
    pub warning: Option<String>,
}

/// A system Trayload no longer has an emulator for. Games already in the library keep their name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Retired {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    /// Goes up with every edit, so the newest copy can be told apart.
    #[serde(default)]
    pub revision: u32,
    pub systems: Vec<System>,
    #[serde(default)]
    pub retired: Vec<Retired>,
}

fn parse(text: &str) -> Result<Registry, String> {
    serde_json::from_str(text).map_err(|e| format!("registry parse error: {e}"))
}

/// Where a system's emulator comes from, without the parts that may change (asset patterns, file names).
fn origin(s: &Source) -> String {
    match s {
        Source::Github { repo, .. } => format!("github:{}", repo.to_lowercase()),
        Source::Forgejo { host, repo, .. } => format!("forgejo:{}/{}", host.to_lowercase(), repo.to_lowercase()),
        Source::Direct { url } => format!("direct:{}", host_of(url)),
        Source::Page { url, .. } => format!("page:{}", host_of(url)),
    }
}

fn host_of(url: &str) -> String {
    reqwest::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_lowercase())).unwrap_or_default()
}

/// Checks every entry: patterns compile, launch options have one game slot, paths stay inside the
/// emulator's folder, and every address is on the download allowlist.
pub fn check_entries(r: &Registry) -> Result<(), String> {
    use regex::Regex;
    use std::path::{Component, Path};
    let re_ok = |p: &str, what: &str, id: &str| -> Result<(), String> {
        Regex::new(p).map(|_| ()).map_err(|_| format!("bad {what} for {id}"))
    };
    let safe = |p: &str| {
        let t = p.trim_end_matches('/');
        !t.is_empty() && Path::new(t).components().all(|c| matches!(c, Component::Normal(_)))
    };
    let mut ids = std::collections::HashSet::new();
    for s in &r.systems {
        if s.id.is_empty() || !ids.insert(s.id.clone()) {
            return Err(format!("empty or duplicate system id: {}", s.id));
        }
        if let Some(p) = &s.exe_pattern {
            re_ok(p, "exePattern", &s.id)?;
        }
        match &s.source {
            Source::Github { repo, asset_pattern, version_regex } => {
                if repo.matches('/').count() != 1 || repo.contains("..") {
                    return Err(format!("bad repo for {}", s.id));
                }
                re_ok(asset_pattern, "assetPattern", &s.id)?;
                if let Some(v) = version_regex {
                    re_ok(v, "versionRegex", &s.id)?;
                }
            }
            Source::Forgejo { host, asset_pattern, version_regex, .. } => {
                crate::installer::check_host(&format!("https://{host}/"))?;
                re_ok(asset_pattern, "assetPattern", &s.id)?;
                if let Some(v) = version_regex {
                    re_ok(v, "versionRegex", &s.id)?;
                }
            }
            Source::Direct { url } => crate::installer::check_host(url)?,
            Source::Page { url, asset_pattern, fallback_url } => {
                crate::installer::check_host(url)?;
                if let Some(f) = fallback_url {
                    crate::installer::check_host(f)?;
                }
                if let Some(p) = asset_pattern {
                    re_ok(p, "assetPattern", &s.id)?;
                }
            }
        }
        if let Some(a) = &s.launch_args {
            let n = a.iter().filter(|x| *x == "{rom}" || x.contains("{titleid}")).count();
            if n != 1 {
                return Err(format!("launchArgs of {} needs exactly one {{rom}} or {{titleid}}", s.id));
            }
        }
        if s.backup.iter().any(|b| !safe(b)) {
            return Err(format!("bad backup path for {}", s.id));
        }
        if let Some(m) = &s.portable_marker {
            if !safe(m) {
                return Err(format!("bad portableMarker for {}", s.id));
            }
        }
        if let Some(setup) = &s.setup {
            if setup.folder != "." && !safe(&setup.folder) {
                return Err(format!("bad setup folder for {}", s.id));
            }
            for it in &setup.items {
                if let Some(p) = &it.name_pattern {
                    re_ok(p, "setup namePattern", &s.id)?;
                }
            }
        }
    }
    Ok(())
}

/// A downloaded list may only describe emulators this version of Trayload already knows,
/// and each must still come from the same project.
pub fn check_against(remote: &Registry, built_in: &Registry) -> Result<(), String> {
    for s in &remote.systems {
        let slug = crate::installer::slug(&s.emulator);
        let known = built_in.systems.iter().find(|b| crate::installer::slug(&b.emulator) == slug);
        match known {
            None => return Err(format!("{} is not an emulator this version knows", s.emulator)),
            Some(b) if origin(&b.source) != origin(&s.source) => {
                return Err(format!("{} would come from a different place ({})", s.emulator, origin(&s.source)))
            }
            _ => {}
        }
    }
    Ok(())
}

/// Accepts a downloaded list only if it is newer than what is built in and passes every check.
pub fn accept_remote(text: &str, current_revision: u32) -> Result<Registry, String> {
    let remote = parse(text)?;
    let built_in = parse(EMBEDDED)?;
    if remote.revision <= current_revision.max(built_in.revision) {
        return Err("not newer".into());
    }
    check_entries(&remote)?;
    check_against(&remote, &built_in)?;
    Ok(remote)
}

pub fn revision_in_use(app: &AppHandle) -> u32 {
    load(app).map(|r| r.revision).unwrap_or(0)
}

pub fn load(app: &AppHandle) -> Result<Registry, String> {
    if let Ok(dir) = app.path().app_local_data_dir() {
        let path = dir.join("registry.json");
        if path.exists() {
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            match parse(&text) {
                Ok(r) => return Ok(r),
                // An old registry.json (from before a Trayload update) must not break the app.
                Err(e) => crate::applog!("[registry] ignoring {}: {e}", path.display()),
            }
        }
    }
    let built_in = parse(EMBEDDED)?;
    if let Ok(dir) = app.path().app_local_data_dir() {
        if let Ok(text) = fs::read_to_string(dir.join("registry.remote.json")) {
            // Re-checked on every load, so a damaged or tampered file is simply ignored.
            if let Ok(r) = parse(&text) {
                if r.revision > built_in.revision && check_entries(&r).is_ok() && check_against(&r, &built_in).is_ok() {
                    return Ok(r);
                }
            }
        }
    }
    Ok(built_in)
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
        let r = parse(EMBEDDED).unwrap();
        check_entries(&r).expect("embedded registry must pass every check");
    }

    #[test]
    fn downloaded_list_cannot_change_the_source() {
        let built_in = parse(EMBEDDED).unwrap();
        // Same list, newer revision: fine.
        let mut ok = built_in.clone();
        ok.revision += 1;
        assert!(accept_remote(&serde_json::to_string(&ok).unwrap(), 0).is_ok());
        // Not newer: ignored.
        assert!(accept_remote(&serde_json::to_string(&built_in).unwrap(), 0).is_err());
        // A different project for an existing emulator: rejected.
        let mut bad = ok.clone();
        for s in bad.systems.iter_mut() {
            if let Source::Github { repo, .. } = &mut s.source {
                *repo = "someone-else/evil".into();
                break;
            }
        }
        assert!(accept_remote(&serde_json::to_string(&bad).unwrap(), 0).is_err());
        // An emulator this version does not know: rejected.
        let mut extra = ok.clone();
        let mut s = extra.systems[0].clone();
        s.id = "newsys".into();
        s.emulator = "Brand New Emulator".into();
        extra.systems.push(s);
        assert!(accept_remote(&serde_json::to_string(&extra).unwrap(), 0).is_err());
        // A changed asset pattern is allowed.
        let mut tweak = ok.clone();
        if let Source::Github { asset_pattern, .. } = &mut tweak.systems[0].source {
            *asset_pattern = "NewName\\.zip$".into();
        }
        assert!(accept_remote(&serde_json::to_string(&tweak).unwrap(), 0).is_ok());
    }

    #[test]
    fn downloaded_list_cannot_use_other_hosts_or_bad_patterns() {
        let mut r = parse(EMBEDDED).unwrap();
        r.revision += 1;
        r.systems[0].source = Source::Direct { url: "https://evil.example/x.zip".into() };
        assert!(check_entries(&r).is_err());
        let mut r2 = parse(EMBEDDED).unwrap();
        r2.systems[0].exe_pattern = Some("(".into());
        assert!(check_entries(&r2).is_err());
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
    fn retired_systems_are_not_active() {
        let r = parse(EMBEDDED).unwrap();
        assert!(!r.retired.is_empty());
        for t in &r.retired {
            assert!(!r.systems.iter().any(|s| s.id == t.id), "{} is both active and retired", t.id);
        }
    }

    #[test]
    fn no_system_uses_retroarch() {
        let r = parse(EMBEDDED).unwrap();
        for s in &r.systems {
            assert_ne!(s.tier.as_deref(), Some("core"), "{} still uses a core", s.id);
            assert!(!s.exe_pattern.clone().unwrap_or_default().to_lowercase().contains("retroarch"));
            assert!(!s.launch_args.clone().unwrap_or_default().iter().any(|a| a == "-L"));
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
