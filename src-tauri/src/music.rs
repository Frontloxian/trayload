//! Music: the Trayload soundtrack and custom music packs.
//! A pack is a folder inside <app data>/music/ holding audio files and an optional pack.json
//! ({"name": "...", "author": "..."}). The player lives in the frontend; this file lists packs,
//! hands over one track at a time, and manages the folders.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};
use tauri::{AppHandle, Manager};

const EXTS: &[&str] = &["mp3", "ogg", "oga", "opus", "wav", "flac", "m4a", "aac"];
const MAX_TRACK_BYTES: u64 = 80 * 1024 * 1024;
const BUILTIN: &str = "trayload";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub file: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub builtin: bool,
    pub tracks: Vec<Track>,
}

#[derive(Deserialize)]
struct PackMeta {
    name: Option<String>,
    author: Option<String>,
}

fn music_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = app.path().app_data_dir().map_err(|e| e.to_string())?.join("music");
    fs::create_dir_all(d.join(BUILTIN)).map_err(|e| format!("cannot create the music folder: {e}"))?;
    Ok(d)
}

/// Copies every plain-named file from `src` into `dest`, but only when the file is missing or its size differs.
/// Anything else already in `dest` (a person's own files) is left alone. Returns how many files were copied.
fn copy_missing(src: &Path, dest: &Path) -> std::io::Result<usize> {
    fs::create_dir_all(dest)?;
    let mut copied = 0;
    for e in fs::read_dir(src)?.flatten() {
        let from = e.path();
        let Some(name) = e.file_name().to_str().map(String::from) else { continue };
        if !from.is_file() || !plain_name(&name) {
            continue;
        }
        let to = dest.join(&name);
        let same = match (fs::metadata(&from), fs::metadata(&to)) {
            (Ok(a), Ok(b)) => a.len() == b.len(),
            _ => false,
        };
        if !same {
            fs::copy(&from, &to)?;
            copied += 1;
        }
    }
    Ok(copied)
}

/// Puts the soundtrack that ships with the installer into the music folder, so it shows up as the built-in pack.
/// Runs at every start. It only copies what is missing, so it costs nothing once the files are there.
pub fn seed_builtin(app: &AppHandle) {
    let Ok(root) = music_dir(app) else { return };
    let mut candidates: Vec<PathBuf> = vec![];
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("resources").join("music").join(BUILTIN));
        candidates.push(res.join("music").join(BUILTIN));
    }
    // While developing, the files are still in the source folder.
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources").join("music").join(BUILTIN));
    let Some(src) = candidates.into_iter().find(|p| p.is_dir()) else {
        crate::applog!("[music] the bundled soundtrack was not found");
        return;
    };
    match copy_missing(&src, &root.join(BUILTIN)) {
        Ok(n) if n > 0 => crate::applog!("[music] installed {n} soundtrack file(s)"),
        Ok(_) => {}
        Err(e) => crate::applog!("[music] could not install the soundtrack: {e}"),
    }
}

/// Folder and file names must be plain names: no separators, no "..", nothing hidden.
fn plain_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 120
        && !n.starts_with('.')
        && !n.contains(['/', '\\', ':', '\0'])
        && n != ".."
}

fn is_audio(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn title_of(file: &str) -> String {
    let stem = Path::new(file).file_stem().and_then(|s| s.to_str()).unwrap_or(file);
    // "01 - Title" or "01_Title" -> "Title"
    let t = stem.trim_start_matches(|c: char| c.is_ascii_digit() || " -_.".contains(c));
    let t = if t.is_empty() { stem } else { t };
    t.replace('_', " ")
}

fn read_pack(dir: &Path, id: &str) -> Pack {
    let meta: Option<PackMeta> = fs::read_to_string(dir.join("pack.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let mut files: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_file() && is_audio(&e.path()))
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|f| f.to_lowercase());
    let builtin = id == BUILTIN;
    let default_name = if builtin { "Trayload soundtrack".to_string() } else { id.replace(['_', '-'], " ") };
    Pack {
        id: id.to_string(),
        name: meta.as_ref().and_then(|m| m.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or(default_name),
        author: meta.and_then(|m| m.author).filter(|a| !a.trim().is_empty()),
        builtin,
        tracks: files.into_iter().map(|f| Track { title: title_of(&f), file: f }).collect(),
    }
}

#[tauri::command]
pub fn list_music_packs(app: AppHandle) -> Result<Vec<Pack>, String> {
    let root = music_dir(&app)?;
    let mut packs: Vec<Pack> = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .filter(|n| plain_name(n))
        .map(|n| read_pack(&root.join(&n), &n))
        .collect();
    packs.sort_by_key(|p| (!p.builtin, p.name.to_lowercase()));
    Ok(packs)
}

/// One track as a data URL, so the webview can play it without file-access permissions.
#[tauri::command]
pub fn get_track(app: AppHandle, pack: String, file: String) -> Result<String, String> {
    if !plain_name(&pack) || !plain_name(&file) {
        return Err("invalid track".into());
    }
    let path = music_dir(&app)?.join(&pack).join(&file);
    if !is_audio(&path) {
        return Err("that is not an audio file Trayload plays".into());
    }
    let len = fs::metadata(&path).map_err(|e| format!("cannot find the track: {e}"))?.len();
    if len > MAX_TRACK_BYTES {
        return Err("this track is larger than 80 MB".into());
    }
    let bytes = fs::read(&path).map_err(|e| format!("cannot read the track: {e}"))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mime = match ext.as_str() {
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/ogg; codecs=opus",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" | "aac" => "audio/mp4",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(&bytes)))
}

#[tauri::command]
pub fn open_music_folder(app: AppHandle) -> Result<(), String> {
    let root = music_dir(&app)?;
    std::process::Command::new("explorer")
        .arg(&root)
        .spawn()
        .map_err(|e| format!("could not open Explorer: {e}"))?;
    Ok(())
}

fn unique_dir(root: &Path, base: &str) -> PathBuf {
    let mut d = root.join(base);
    let mut n = 2;
    while d.exists() {
        d = root.join(format!("{base} {n}"));
        n += 1;
    }
    d
}

/// Copies the audio files (and pack.json) of a folder you pick into a new pack.
#[tauri::command]
pub fn add_music_pack(app: AppHandle, path: String) -> Result<String, String> {
    let src = PathBuf::from(&path);
    if !src.is_dir() {
        return Err("pick a folder that contains your music files".into());
    }
    let files: Vec<PathBuf> = fs::read_dir(&src)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_audio(p))
        .collect();
    if files.is_empty() {
        return Err("no music files found in that folder (mp3, ogg, opus, wav, flac, m4a)".into());
    }
    let raw = src.file_name().and_then(|n| n.to_str()).unwrap_or("Music pack");
    let base: String = raw.chars().filter(|c| !"/\\:*?\"<>|".contains(*c)).collect();
    let base = if plain_name(base.trim()) { base.trim().to_string() } else { "Music pack".to_string() };
    let dest = unique_dir(&music_dir(&app)?, &base);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    for f in &files {
        let name = f.file_name().ok_or("bad file name")?;
        fs::copy(f, dest.join(name)).map_err(|e| format!("could not copy {}: {e}", f.display()))?;
    }
    let meta = src.join("pack.json");
    if meta.is_file() {
        let _ = fs::copy(&meta, dest.join("pack.json"));
    }
    Ok(dest.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string())
}

#[tauri::command]
pub fn remove_music_pack(app: AppHandle, id: String) -> Result<(), String> {
    if !plain_name(&id) || id == BUILTIN {
        return Err("this pack can't be removed".into());
    }
    let dir = music_dir(&app)?.join(&id);
    if !dir.is_dir() {
        return Ok(());
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("could not remove the pack: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(plain_name("My Pack"));
        assert!(!plain_name("../x"));
        assert!(!plain_name("a\\b"));
        assert!(!plain_name(".hidden"));
        assert!(!plain_name(""));
    }

    #[test]
    fn audio_only() {
        assert!(is_audio(Path::new("a.MP3")));
        assert!(is_audio(Path::new("a.flac")));
        assert!(!is_audio(Path::new("a.exe")));
        assert!(!is_audio(Path::new("a")));
    }

    #[test]
    fn copies_only_what_is_missing() {
        let base = std::env::temp_dir().join(format!("trayload-music-test-{}", std::process::id()));
        let (src, dest) = (base.join("src"), base.join("dest"));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("01 - A.mp3"), b"aaaa").unwrap();
        fs::write(src.join("pack.json"), b"{}").unwrap();
        assert_eq!(copy_missing(&src, &dest).unwrap(), 2);
        // A second run copies nothing, and a person's own file is kept.
        fs::write(dest.join("mine.mp3"), b"keep").unwrap();
        assert_eq!(copy_missing(&src, &dest).unwrap(), 0);
        assert!(dest.join("mine.mp3").exists());
        // A changed file (different size) is replaced.
        fs::write(src.join("01 - A.mp3"), b"aaaaaa").unwrap();
        assert_eq!(copy_missing(&src, &dest).unwrap(), 1);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn titles() {
        assert_eq!(title_of("01 - Boot Chime.mp3"), "Boot Chime");
        assert_eq!(title_of("02_Tray_Open.ogg"), "Tray Open");
        assert_eq!(title_of("2049.mp3"), "2049");
    }
}
