//! Optional online updates for the emulator list and the compatibility notes.
//!
//! Trayload reads two small public files from its own GitHub repository (the same files that are
//! built into the app) and keeps a copy if it is newer and passes every check in registry.rs and
//! compat.rs. Nothing is sent about you: it is a plain download of a public file. The Settings page
//! has an off switch, and the app works fully without it.

use crate::{compat, registry};
use serde::Serialize;
use std::{fs, path::Path, time::Duration};
use tauri::{AppHandle, Manager};

const BASE: &str = "https://raw.githubusercontent.com/Frontloxian/trayload/main/src-tauri/resources/";
const MAX_BYTES: usize = 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    registry_revision: u32,
    compat_revision: u32,
    registry_updated: bool,
    compat_updated: bool,
    /// False when the files could not be reached (offline, or GitHub is down).
    online: bool,
}

async fn fetch(name: &str) -> Result<String, String> {
    // This client only talks to raw.githubusercontent.com and follows no redirects. It is separate
    // from the emulator download client, whose allowlist is deliberately narrower.
    let client = reqwest::Client::builder()
        .user_agent("Trayload")
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(format!("{BASE}{name}")).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("file is too large".into());
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| "file is not text".to_string())
}

/// Writes to a temporary file first, so a half-written file is never read.
fn save(dir: &Path, name: &str, text: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{name}.tmp"));
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join(name)).map_err(|e| e.to_string())
}

fn status(app: &AppHandle, registry_updated: bool, compat_updated: bool, online: bool) -> RemoteStatus {
    RemoteStatus {
        registry_revision: registry::revision_in_use(app),
        compat_revision: compat::load(app).revision,
        registry_updated,
        compat_updated,
        online,
    }
}

#[tauri::command]
pub fn remote_status(app: AppHandle) -> RemoteStatus {
    status(&app, false, false, true)
}

#[tauri::command]
pub async fn refresh_remote(app: AppHandle) -> Result<RemoteStatus, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let (mut reg_new, mut compat_new, mut online) = (false, false, true);

    match fetch("emulators.json").await {
        Ok(text) => match registry::accept_remote(&text, registry::revision_in_use(&app)) {
            Ok(_) => {
                save(&dir, "registry.remote.json", &text)?;
                applog!("[remote] emulator list updated");
                reg_new = true;
            }
            Err(m) if m == "not newer" => {}
            Err(m) => applog!("[remote] ignored the downloaded emulator list: {m}"),
        },
        Err(m) => {
            applog!("[remote] could not reach the emulator list: {m}");
            online = false;
        }
    }
    match fetch("compat.json").await {
        Ok(text) => match compat::accept_remote(&text, compat::load(&app).revision) {
            Ok(_) => {
                save(&dir, "compat.remote.json", &text)?;
                applog!("[remote] compatibility notes updated");
                compat_new = true;
            }
            Err(m) if m == "not newer" => {}
            Err(m) => applog!("[remote] ignored the downloaded compatibility notes: {m}"),
        },
        Err(m) => {
            applog!("[remote] could not reach the compatibility notes: {m}");
            online = false;
        }
    }
    Ok(status(&app, reg_new, compat_new, online))
}
