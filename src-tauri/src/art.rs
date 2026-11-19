//! Artwork: a throttled background queue that finds cover art and caches it on disk.
//! Sources, in order: SteamGridDB (needs your own free API key), then libretro-thumbnails (no key).
//! Only allowlisted hosts are contacted, only real PNG/JPEG/WebP files are kept.

use crate::{
    library::Db,
    registry::{self, System},
};
use regex::Regex;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::{
    fs,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

const ART_HOSTS: &[&str] = &[
    "raw.githubusercontent.com",
    "www.steamgriddb.com",
    "steamgriddb.com",
    "cdn2.steamgriddb.com",
    "cdn.steamgriddb.com",
];
const MAX_BYTES: usize = 15 * 1024 * 1024;
const RETRY_NONE_SECS: i64 = 600; // network trouble: try again after 10 minutes
const RETRY_NOTFOUND_SECS: i64 = 7 * 24 * 3600; // nothing found: try again after a week

type Img = (Vec<u8>, &'static str);

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn e<E: ToString>(x: E) -> String {
    x.to_string()
}

fn host_ok(h: &str) -> bool {
    ART_HOSTS.contains(&h)
}

fn client() -> Result<reqwest::Client, String> {
    let policy = reqwest::redirect::Policy::custom(|a| {
        if a.previous().len() >= 5 {
            return a.error("too many redirects");
        }
        let ok = a.url().scheme() == "https" && a.url().host_str().map(host_ok).unwrap_or(false);
        if ok {
            a.follow()
        } else {
            a.error("redirect to a host that is not allowed")
        }
    });
    reqwest::Client::builder()
        .user_agent("Trayload/0.1")
        .redirect(policy)
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(e)
}

// ---------- name handling ----------

/// "Super Mario Advance 4 (USA) (Rev 1)" -> "Super Mario Advance 4"
pub fn clean_title(t: &str) -> String {
    let re = Regex::new(r"\s*[\(\[][^\)\]]*[\)\]]").unwrap();
    let s = re.replace_all(t, "").to_string();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// Equal after normalising, or one contains the other and they are close in length.
fn similar(a: &str, b: &str) -> bool {
    let (na, nb) = (norm(a), norm(b));
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    if na == nb {
        return true;
    }
    let (short, long) = if na.len() <= nb.len() { (&na, &nb) } else { (&nb, &na) };
    short.len() >= 4 && long.contains(short.as_str()) && short.len() * 10 >= long.len() * 6
}

fn libretro_name(s: &str) -> String {
    s.chars().map(|c| if "&*/:`<>?\\|\"".contains(c) { '_' } else { c }).collect()
}

fn image_ext(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

// ---------- fetching ----------

async fn fetch_image(client: &reqwest::Client, url: &str) -> Result<Option<Img>, String> {
    let u = reqwest::Url::parse(url).map_err(e)?;
    if u.scheme() != "https" || !u.host_str().map(host_ok).unwrap_or(false) {
        return Err(format!("art host not on the allowlist: {url}"));
    }
    let resp = client.get(u).send().await.map_err(|x| format!("request failed: {x}"))?;
    let st = resp.status();
    if st == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !st.is_success() {
        return Err(format!("{url} answered {st}"));
    }
    if resp.content_length().map(|n| n as usize > MAX_BYTES).unwrap_or(false) {
        return Err("image is too large".into());
    }
    let bytes = resp.bytes().await.map_err(e)?;
    if bytes.len() > MAX_BYTES {
        return Err("image is too large".into());
    }
    match image_ext(&bytes) {
        Some(ext) => Ok(Some((bytes.to_vec(), ext))),
        None => Err(format!("{url} is not a PNG, JPEG or WebP image")),
    }
}

async fn from_libretro(client: &reqwest::Client, sys: &System, title: &str) -> Result<Option<Img>, String> {
    let Some(repos) = &sys.art_repos else { return Ok(None) };
    let clean = clean_title(title);
    let mut names = vec![title.to_string()];
    for tag in ["(USA)", "(Europe)", "(World)", "(Japan)"] {
        let n = format!("{clean} {tag}");
        if !names.contains(&n) {
            names.push(n);
        }
    }
    for repo in repos {
        for name in &names {
            let file = format!("{}.png", libretro_name(name));
            let mut u = reqwest::Url::parse("https://raw.githubusercontent.com/libretro-thumbnails/").map_err(e)?;
            u.path_segments_mut()
                .map_err(|_| "bad URL".to_string())?
                .pop_if_empty()
                .extend([repo.as_str(), "master", "Named_Boxarts", file.as_str()]);
            if let Some(img) = fetch_image(client, u.as_str()).await? {
                return Ok(Some(img));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
    Ok(None)
}

async fn sgdb_json(client: &reqwest::Client, key: &str, url: &str) -> Result<Option<serde_json::Value>, String> {
    let resp = client
        .get(url)
        .bearer_auth(key)
        .send()
        .await
        .map_err(|x| format!("SteamGridDB request failed: {x}"))?;
    match resp.status().as_u16() {
        401 | 403 => Err("SteamGridDB rejected the API key".into()),
        404 => Ok(None),
        s if (200..300).contains(&s) => Ok(Some(resp.json().await.map_err(e)?)),
        s => Err(format!("SteamGridDB answered {s}")),
    }
}

async fn from_sgdb(client: &reqwest::Client, key: &str, title: &str) -> Result<Option<Img>, String> {
    let clean = clean_title(title);
    if clean.is_empty() {
        return Ok(None);
    }
    let mut u = reqwest::Url::parse("https://www.steamgriddb.com/api/v2/search/autocomplete/").map_err(e)?;
    u.path_segments_mut().map_err(|_| "bad URL".to_string())?.pop_if_empty().push(&clean);
    let Some(search) = sgdb_json(client, key, u.as_str()).await? else { return Ok(None) };
    let items = search["data"].as_array().cloned().unwrap_or_default();
    for item in items.iter().take(3) {
        if !similar(item["name"].as_str().unwrap_or(""), &clean) {
            continue;
        }
        let Some(id) = item["id"].as_i64() else { continue };
        let url = format!(
            "https://www.steamgriddb.com/api/v2/grids/game/{id}?dimensions=600x900,342x482,660x930&types=static&nsfw=false&humor=false"
        );
        let Some(grids) = sgdb_json(client, key, &url).await? else { continue };
        let list = grids["data"].as_array().cloned().unwrap_or_default();
        for g in list.iter().take(3) {
            if let Some(img_url) = g["url"].as_str() {
                if let Some(img) = fetch_image(client, img_url).await? {
                    return Ok(Some(img));
                }
            }
        }
    }
    Ok(None)
}

async fn find_art(
    client: &reqwest::Client,
    sys: &System,
    key: Option<&str>,
    title: &str,
) -> Result<Option<(Img, &'static str)>, String> {
    let mut first_err: Option<String> = None;
    if let Some(k) = key {
        match from_sgdb(client, k, title).await {
            Ok(Some(i)) => return Ok(Some((i, "SteamGridDB"))),
            Ok(None) => {}
            Err(er) => first_err = Some(er),
        }
    }
    match from_libretro(client, sys, title).await {
        Ok(Some(i)) => Ok(Some((i, "libretro-thumbnails"))),
        Ok(None) => match first_err {
            Some(er) => Err(er),
            None => Ok(None),
        },
        Err(er) => Err(er),
    }
}

// ---------- storage ----------

fn get_key(db: &Db) -> Option<String> {
    if let Some(k) = crate::secrets::get("steamgriddb") {
        return Some(k);
    }
    let conn = db.0.lock().ok()?;
    conn.query_row("SELECT value FROM settings WHERE key = 'steamgriddb_key'", [], |r| r.get::<_, String>(0))
        .optional()
        .ok()
        .flatten()
        .filter(|k| !k.trim().is_empty())
}

fn save_art(app: &AppHandle, db: &Db, id: i64, img: &Img, source: &str, manual: bool) -> Result<(), String> {
    let dir = app.path().app_local_data_dir().map_err(e)?.join("art").join(id.to_string());
    fs::create_dir_all(&dir).map_err(e)?;
    if let Ok(rd) = fs::read_dir(&dir) {
        for f in rd.flatten() {
            let _ = fs::remove_file(f.path());
        }
    }
    let file = dir.join(format!("cover-{}.{}", now(), img.1));
    fs::write(&file, &img.0).map_err(e)?;
    let status = if manual { "manual" } else { "found" };
    let conn = db.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute(
        "UPDATE games SET art_status = ?2, art_source = ?3, art_file = ?4, art_tried_at = ?5
         WHERE id = ?1 AND (?6 = 1 OR art_status IN ('none', 'notfound'))",
        params![id, status, source, file.to_string_lossy().to_string(), now(), manual as i64],
    )
    .map_err(e)?;
    Ok(())
}

fn mark(db: &Db, id: i64, status: &str) {
    if let Ok(conn) = db.0.lock() {
        let _ = conn.execute(
            "UPDATE games SET art_status = ?2, art_tried_at = ?3 WHERE id = ?1 AND art_status IN ('none', 'notfound')",
            params![id, status, now()],
        );
    }
}

struct Job {
    id: i64,
    system_id: String,
    title: String,
}

fn next_job(db: &Db) -> Result<Option<Job>, String> {
    let conn = db.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    let t = now();
    conn.query_row(
        "SELECT id, system_id, title FROM games
         WHERE removed = 0 AND (
           (art_status = 'none' AND COALESCE(art_tried_at, 0) < ?1) OR
           (art_status = 'notfound' AND COALESCE(art_tried_at, 0) < ?2))
         ORDER BY id LIMIT 1",
        params![t - RETRY_NONE_SECS, t - RETRY_NOTFOUND_SECS],
        |r| Ok(Job { id: r.get(0)?, system_id: r.get(1)?, title: r.get(2)? }),
    )
    .optional()
    .map_err(e)
}

async fn process(app: &AppHandle, db: &Db, job: Job) {
    let reg = match registry::load(app) {
        Ok(r) => r,
        Err(er) => {
            crate::applog!("[art] {er}");
            mark(db, job.id, "none");
            return;
        }
    };
    let Some(sys) = reg.systems.iter().find(|s| s.id == job.system_id) else {
        mark(db, job.id, "notfound");
        return;
    };
    let key = get_key(db);
    let client = match client() {
        Ok(c) => c,
        Err(er) => {
            crate::applog!("[art] {er}");
            return;
        }
    };
    match find_art(&client, sys, key.as_deref(), &job.title).await {
        Ok(Some((img, source))) => match save_art(app, db, job.id, &img, source, false) {
            Ok(()) => {
                let _ = app.emit("art-updated", job.id);
            }
            Err(er) => {
                crate::applog!("[art] could not save art for {}: {er}", job.title);
                mark(db, job.id, "none");
            }
        },
        Ok(None) => mark(db, job.id, "notfound"),
        Err(er) => {
            crate::applog!("[art] {}: {er}", job.title);
            mark(db, job.id, "none");
        }
    }
}

/// Runs for the life of the app: one game at a time, with a pause between requests.
pub async fn worker(app: AppHandle, db: Db) {
    tokio::time::sleep(Duration::from_secs(3)).await;
    loop {
        match next_job(&db) {
            Ok(Some(job)) => {
                process(&app, &db, job).await;
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
            Ok(None) => tokio::time::sleep(Duration::from_secs(5)).await,
            Err(er) => {
                crate::applog!("[art] {er}");
                tokio::time::sleep(Duration::from_secs(15)).await;
            }
        }
    }
}

// ---------- commands ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtSettings {
    has_steamgriddb_key: bool,
}

#[tauri::command]
pub fn get_art_settings(db: tauri::State<'_, Db>) -> ArtSettings {
    ArtSettings { has_steamgriddb_key: get_key(&db).is_some() }
}

fn reset_retries(db: &Db) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "database lock poisoned".to_string())?;
    conn.execute_batch(
        "UPDATE games SET art_status = 'none', art_tried_at = NULL WHERE art_status = 'notfound';
         UPDATE games SET art_tried_at = NULL WHERE art_status = 'none';",
    )
    .map_err(e)
}

/// Saves (or clears, if empty) the SteamGridDB key, then lets missing covers try the new source.
#[tauri::command]
pub fn set_steamgriddb_key(db: tauri::State<'_, Db>, key: String) -> Result<(), String> {
    {
        let key = key.trim();
        let in_store = if key.is_empty() {
            crate::secrets::delete("steamgriddb");
            false
        } else {
            crate::secrets::set("steamgriddb", key).is_ok()
        };
        let conn = db.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        if in_store || key.is_empty() {
            // Stored safely (or cleared): make sure no copy is left in the database.
            conn.execute("DELETE FROM settings WHERE key = 'steamgriddb_key'", []).map_err(e)?;
        } else {
            crate::applog!("[secrets] credential store unavailable; keeping the key in the library database");
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('steamgriddb_key', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key],
            )
            .map_err(e)?;
        }
    }
    reset_retries(&db)
}

#[tauri::command]
pub fn retry_art(db: tauri::State<'_, Db>) -> Result<(), String> {
    reset_retries(&db)
}

/// Use your own image as a game's cover. Manual covers are never replaced automatically.
#[tauri::command]
pub fn set_art_from_file(app: AppHandle, db: tauri::State<'_, Db>, id: i64, path: String) -> Result<(), String> {
    let meta = fs::metadata(&path).map_err(|_| "Could not read that file.".to_string())?;
    if meta.len() as usize > MAX_BYTES {
        return Err("That image is larger than 15 MB.".into());
    }
    let bytes = fs::read(&path).map_err(e)?;
    let ext = image_ext(&bytes).ok_or("Please pick a PNG, JPG or WebP image.")?;
    save_art(&app, &db, id, &(bytes, ext), "manual", true)?;
    let _ = app.emit("art-updated", id);
    Ok(())
}

/// Returns a game's cover as a data URL, read straight from the app's own art folder.
/// (Only the file recorded for that game in the library is ever read.)
#[tauri::command]
pub fn get_cover(db: tauri::State<'_, Db>, id: i64) -> Result<String, String> {
    let path: Option<String> = {
        let conn = db.0.lock().map_err(|_| "database lock poisoned".to_string())?;
        conn.query_row("SELECT art_file FROM games WHERE id = ?1", [id], |r| r.get::<_, Option<String>>(0))
            .optional()
            .map_err(e)?
            .flatten()
    };
    let path = path.ok_or("this game has no cover")?;
    let bytes = fs::read(&path).map_err(|x| format!("cannot read the cover file: {x}"))?;
    let ext = image_ext(&bytes).ok_or("the cover file is not a PNG, JPEG or WebP image")?;
    let mime = match ext {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        _ => "image/webp",
    };
    use base64::{engine::general_purpose::STANDARD, Engine};
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_tags() {
        assert_eq!(clean_title("Super Mario Advance 4 (USA) (Rev 1)"), "Super Mario Advance 4");
        assert_eq!(clean_title("Final Fantasy VII [Disc 1] (Europe)"), "Final Fantasy VII");
        assert_eq!(clean_title("Plain Title"), "Plain Title");
    }

    #[test]
    fn name_similarity() {
        assert!(similar("Super Mario Advance 4", "super mario advance 4"));
        assert!(similar("Pokemon Emerald Version", "Pokemon Emerald"));
        assert!(!similar("Mario", "Super Mario Advance 4"));
        assert!(!similar("", "anything"));
    }

    #[test]
    fn libretro_names_replace_forbidden_characters() {
        assert_eq!(libretro_name("Zelda: A Link & More"), "Zelda_ A Link _ More");
    }

    #[test]
    fn only_real_images_pass() {
        assert_eq!(image_ext(&[0x89, b'P', b'N', b'G', 0, 0]), Some("png"));
        assert_eq!(image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(image_ext(b"<html>nope</html>"), None);
    }

    #[test]
    fn host_allowlist() {
        assert!(host_ok("raw.githubusercontent.com"));
        assert!(!host_ok("evil.example.com"));
        assert!(!host_ok("raw.githubusercontent.com.evil.example.com"));
    }
}
