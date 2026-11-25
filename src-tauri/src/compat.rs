//! Per-game compatibility notes ("needs the Vulkan renderer" and similar).
//! A built-in list ships with the app; a newer one can be downloaded by remote.rs.
//! Notes are plain text and are never run as anything.

use serde::{Deserialize, Serialize};
use std::fs;
use tauri::{AppHandle, Manager};

const EMBEDDED: &str = include_str!("../resources/compat.json");
const MAX_NOTE: usize = 400;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub system: String,
    /// Optional: applies to games whose title contains this text (not case-sensitive).
    #[serde(default)]
    pub title: Option<String>,
    pub note: String,
    /// Where the tip comes from. Only https://github.com/ links are opened.
    #[serde(default)]
    pub link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatFile {
    #[serde(default)]
    pub revision: u32,
    pub notes: Vec<Note>,
}

fn parse(text: &str) -> Result<CompatFile, String> {
    serde_json::from_str(text).map_err(|e| format!("compat parse error: {e}"))
}

pub fn check(f: &CompatFile) -> Result<(), String> {
    for n in &f.notes {
        if n.system.trim().is_empty() || n.note.trim().is_empty() || n.note.chars().count() > MAX_NOTE {
            return Err("a note is empty or too long".into());
        }
        if let Some(l) = &n.link {
            if !l.starts_with("https://github.com/") {
                return Err(format!("link is not a GitHub link: {l}"));
            }
        }
    }
    Ok(())
}

pub fn accept_remote(text: &str, current_revision: u32) -> Result<CompatFile, String> {
    let f = parse(text)?;
    let built_in = parse(EMBEDDED)?;
    if f.revision <= current_revision.max(built_in.revision) {
        return Err("not newer".into());
    }
    check(&f)?;
    Ok(f)
}

pub fn load(app: &AppHandle) -> CompatFile {
    let built_in = parse(EMBEDDED).unwrap_or(CompatFile { revision: 0, notes: vec![] });
    if let Ok(dir) = app.path().app_local_data_dir() {
        if let Ok(text) = fs::read_to_string(dir.join("compat.remote.json")) {
            if let Ok(f) = parse(&text) {
                if f.revision > built_in.revision && check(&f).is_ok() {
                    return f;
                }
            }
        }
    }
    built_in
}

fn matching(f: &CompatFile, system_id: &str, title: &str) -> Vec<Note> {
    let t = title.to_lowercase();
    f.notes
        .iter()
        .filter(|n| n.system == system_id)
        .filter(|n| n.title.as_ref().map(|x| t.contains(&x.to_lowercase())).unwrap_or(true))
        .cloned()
        .collect()
}

#[tauri::command]
pub fn get_compat_notes(app: AppHandle, system_id: String, title: String) -> Vec<Note> {
    matching(&load(&app), &system_id, &title)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> CompatFile {
        parse(
            r#"{"revision":2,"notes":[
              {"system":"n64","note":"System-wide tip"},
              {"system":"n64","title":"mario","note":"Only Mario","link":"https://github.com/x/y/issues/1"},
              {"system":"gba","note":"Other system"}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn embedded_file_is_valid() {
        let f = parse(EMBEDDED).unwrap();
        check(&f).unwrap();
    }

    #[test]
    fn notes_match_by_system_and_title() {
        let f = file();
        assert_eq!(matching(&f, "n64", "Super Mario 64").len(), 2);
        assert_eq!(matching(&f, "n64", "Zelda").len(), 1);
        assert_eq!(matching(&f, "gba", "Anything").len(), 1);
        assert_eq!(matching(&f, "ps1", "Anything").len(), 0);
    }

    #[test]
    fn bad_notes_are_rejected() {
        let mut f = file();
        f.notes[1].link = Some("https://evil.example/x".into());
        assert!(check(&f).is_err());
        let mut g = file();
        g.notes[0].note = "x".repeat(401);
        assert!(check(&g).is_err());
    }
}
