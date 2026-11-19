//! API keys live in the Windows Credential Manager, not in the library database.
//! If the credential store can't be used, the key falls back to the database so nothing breaks.

use crate::library::Db;
use rusqlite::OptionalExtension;

const SERVICE: &str = "Trayload";

fn entry(name: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())
}

pub fn get(name: &str) -> Option<String> {
    entry(name).ok()?.get_password().ok().filter(|k| !k.trim().is_empty())
}

pub fn set(name: &str, value: &str) -> Result<(), String> {
    entry(name)?.set_password(value).map_err(|e| e.to_string())
}

pub fn delete(name: &str) {
    if let Ok(e) = entry(name) {
        let _ = e.delete_credential();
    }
}

/// Moves a key saved by an older version out of the database into the credential store.
pub fn migrate(db: &Db, name: &str, db_key: &str) {
    let old: Option<String> = {
        let Ok(conn) = db.0.lock() else { return };
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [db_key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    };
    let Some(old) = old.filter(|k| !k.trim().is_empty()) else { return };
    if set(name, &old).is_ok() && get(name).as_deref() == Some(old.as_str()) {
        if let Ok(conn) = db.0.lock() {
            let _ = conn.execute("DELETE FROM settings WHERE key = ?1", [db_key]);
        }
        crate::applog!("[secrets] moved the {name} key to the Windows credential store");
    }
}
