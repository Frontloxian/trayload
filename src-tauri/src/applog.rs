//! A small log file (<app data>/logs/trayload.log) so problems can be looked at after the fact.
//! No telemetry: the file stays on this PC and nothing is ever sent anywhere.

use serde::Serialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
use tauri::{AppHandle, Manager};

static LOG: OnceLock<Mutex<PathBuf>> = OnceLock::new();
const MAX_BYTES: u64 = 1_000_000;

/// Writes one line to the log (and to the console in dev). Use through `applog!`.
pub fn write(msg: &str) {
    eprintln!("{msg}");
    let Some(m) = LOG.get() else { return };
    let Ok(path) = m.lock() else { return };
    if fs::metadata(&*path).map(|x| x.len() > MAX_BYTES).unwrap_or(false) {
        let _ = fs::rename(&*path, path.with_extension("log.1"));
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&*path) {
        let _ = writeln!(f, "{} {msg}", stamp());
    }
}

#[macro_export]
macro_rules! applog {
    ($($arg:tt)*) => { $crate::applog::write(&format!($($arg)*)) };
}

/// "2026-11-20 01:30:00" in UTC, without extra crates.
fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn logs_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("logs");
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

/// Starts the log file and records crashes (panics) in it.
pub fn init(app: &AppHandle) {
    if let Ok(dir) = logs_dir(app) {
        let _ = LOG.set(Mutex::new(dir.join("trayload.log")));
    }
    std::panic::set_hook(Box::new(|info| {
        write(&format!("[panic] {info}"));
    }));
    write(&format!("[app] Trayload {} started", env!("CARGO_PKG_VERSION")));
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    data_folder: String,
    log_folder: String,
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> Result<AppInfo, String> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        data_folder: app.path().app_local_data_dir().map_err(|e| e.to_string())?.to_string_lossy().to_string(),
        log_folder: logs_dir(&app)?.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn open_logs_folder(app: AppHandle) -> Result<(), String> {
    let d = logs_dir(&app)?;
    std::process::Command::new("explorer")
        .arg(&d)
        .spawn()
        .map_err(|e| format!("could not open Explorer: {e}"))?;
    Ok(())
}

/// Lets the interface record a crash it caught.
#[tauri::command]
pub fn log_frontend_error(message: String) {
    let m: String = message.chars().take(2000).collect();
    write(&format!("[ui] {m}"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn stamp_shape() {
        let s = super::stamp();
        assert_eq!(s.len(), 19);
        assert!(s.starts_with("20"));
    }
}
