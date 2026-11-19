//! Emulator installer: resolve latest release -> download -> extract safely -> launch.
//! Rules: official sources only (host allowlist, also enforced on redirects),
//! never write outside the app data folder, never follow symlinks from archives.

use crate::registry::{self, Source};
use futures_util::StreamExt;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

const ALLOWED_HOSTS: &[&str] = &[
    "api.github.com",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "github-releases.githubusercontent.com",
    "dolphin-emu.org",
    "dl.dolphin-emu.org",
];

#[derive(Default)]
pub struct Installing(pub Mutex<HashSet<String>>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledInfo {
    pub version: String,
    pub asset_name: String,
    pub installed_at: u64,
    pub exe_rel_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorEntry {
    pub id: String,
    pub name: String,
    pub systems: Vec<String>,
    pub installable: bool,
    pub installed: Option<InstalledInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    id: &'a str,
    stage: &'a str,
    downloaded: u64,
    total: u64,
}

fn progress(app: &AppHandle, id: &str, stage: &str, downloaded: u64, total: u64) {
    let _ = app.emit("install-progress", Progress { id, stage, downloaded, total });
}

// ---------- paths ----------

pub fn slug(name: &str) -> String {
    let mapped: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    mapped.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-")
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_local_data_dir().map_err(|e| e.to_string())
}

fn emulators_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("emulators"))
}

pub fn emu_dir(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err("invalid emulator id".into());
    }
    Ok(emulators_dir(app)?.join(id))
}

pub fn read_installed(dir: &Path) -> Option<InstalledInfo> {
    let text = fs::read_to_string(dir.join(".trayload.json")).ok()?;
    serde_json::from_str(&text).ok()
}

// ---------- network ----------

fn host_ok(host: &str) -> bool {
    ALLOWED_HOSTS.contains(&host)
}

fn check_host(url: &str) -> Result<(), String> {
    let u = reqwest::Url::parse(url).map_err(|e| format!("bad URL {url}: {e}"))?;
    if u.scheme() != "https" {
        return Err(format!("refusing non-HTTPS URL: {url}"));
    }
    match u.host_str() {
        Some(h) if host_ok(h) => Ok(()),
        other => Err(format!("host not on the allowlist: {}", other.unwrap_or("?"))),
    }
}

fn http_client() -> Result<reqwest::Client, String> {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 5 {
            return attempt.error("too many redirects");
        }
        let ok = attempt.url().scheme() == "https"
            && attempt.url().host_str().map(host_ok).unwrap_or(false);
        if ok {
            attempt.follow()
        } else {
            attempt.error("redirect to a host that is not allowed")
        }
    });
    reqwest::Client::builder()
        .user_agent("Trayload/0.1")
        .redirect(policy)
        .build()
        .map_err(|e| e.to_string())
}

struct Resolved {
    url: String,
    file_name: String,
    version: String,
}

fn from_link(link: &str) -> Resolved {
    let file_name = link.rsplit('/').next().unwrap_or("download").to_string();
    let version = file_name.trim_end_matches(".7z").trim_end_matches(".zip").to_string();
    Resolved { url: link.to_string(), file_name, version }
}

async fn resolve_page(client: &reqwest::Client, url: &str, pat: &str) -> Result<Resolved, String> {
    check_host(url)?;
    let re = RegexBuilder::new(pat)
        .case_insensitive(true)
        .build()
        .map_err(|e| format!("bad assetPattern: {e}"))?;
    let body = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("page request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("page said: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let m = re.find(&body).ok_or("no link on the download page matched the pattern")?;
    let link = m.as_str();
    check_host(link)?;
    Ok(from_link(link))
}

async fn resolve(client: &reqwest::Client, src: &Source) -> Result<Resolved, String> {
    match src {
        Source::Github { repo, asset_pattern } => {
            let re = RegexBuilder::new(asset_pattern)
                .case_insensitive(true)
                .build()
                .map_err(|e| format!("bad assetPattern: {e}"))?;
            let url = format!("https://api.github.com/repos/{repo}/releases/latest");
            let rel: serde_json::Value = client
                .get(&url)
                .header("Accept", "application/vnd.github+json")
                .send()
                .await
                .map_err(|e| format!("GitHub request failed: {e}"))?
                .error_for_status()
                .map_err(|e| format!("GitHub said: {e}"))?
                .json()
                .await
                .map_err(|e| format!("could not read GitHub's answer: {e}"))?;
            let tag = rel["tag_name"].as_str().unwrap_or("unknown").to_string();
            let assets = rel["assets"].as_array().ok_or("release has no assets")?;
            let mut names = vec![];
            for a in assets {
                let name = a["name"].as_str().unwrap_or("");
                names.push(name.to_string());
                if re.is_match(name) {
                    let dl = a["browser_download_url"].as_str().ok_or("asset has no download URL")?;
                    return Ok(Resolved { url: dl.into(), file_name: name.into(), version: tag });
                }
            }
            Err(format!(
                "No asset in release {tag} matches /{asset_pattern}/. Assets: {}",
                names.join(", ")
            ))
        }
        Source::Page { url, asset_pattern, fallback_url } => {
            let scraped = match asset_pattern.as_deref() {
                Some(pat) => resolve_page(client, url, pat).await,
                None => Err("this page source has no assetPattern".to_string()),
            };
            match (scraped, fallback_url) {
                (Ok(r), _) => Ok(r),
                (Err(err), Some(fb)) => {
                    eprintln!("[installer] page lookup failed ({err}); using the pinned link from the registry");
                    check_host(fb)?;
                    Ok(from_link(fb))
                }
                (Err(err), None) => Err(err),
            }
        }
    }
}

async fn download(
    app: &AppHandle,
    client: &reqwest::Client,
    id: &str,
    r: &Resolved,
    dest: &Path,
) -> Result<(), String> {
    check_host(&r.url)?;
    let resp = client
        .get(&r.url)
        .send()
        .await
        .map_err(|e| format!("download failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download failed: {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let part = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&part).await.map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    let mut done: u64 = 0;
    let mut last = Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download interrupted: {e}"))?;
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        if last.elapsed() > Duration::from_millis(120) {
            progress(app, id, "downloading", done, total);
            last = Instant::now();
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);
    progress(app, id, "downloading", done, total.max(done));
    tokio::fs::rename(&part, dest).await.map_err(|e| e.to_string())
}

// ---------- extraction ----------

fn extract(archive: &Path, staging: &Path) -> Result<(), String> {
    fs::create_dir_all(staging).map_err(|e| e.to_string())?;
    let ext = archive.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "zip" => {
            let f = fs::File::open(archive).map_err(|e| e.to_string())?;
            let mut z = zip::ZipArchive::new(f).map_err(|e| format!("bad zip: {e}"))?;
            // zip 2.x rejects entries that would escape the target folder.
            z.extract(staging).map_err(|e| format!("zip extract failed: {e}"))
        }
        "7z" => {
            // Windows ships bsdtar (libarchive), which reads .7z and refuses ".." paths by default.
            let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
            let tar = PathBuf::from(root).join("System32").join("tar.exe");
            let mut cmd = std::process::Command::new(tar);
            cmd.arg("-xf").arg(archive).arg("-C").arg(staging);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
            }
            let out = cmd.output().map_err(|e| format!("could not run tar.exe: {e}"))?;
            if out.status.success() {
                Ok(())
            } else {
                Err(format!("tar.exe could not extract the archive: {}", String::from_utf8_lossy(&out.stderr)))
            }
        }
        other => Err(format!("unsupported archive type .{other}")),
    }
}

/// Reject symlinks and anything whose real path is outside `root`.
fn verify_tree(root: &Path) -> Result<(), String> {
    let canon_root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            let meta = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            if meta.file_type().is_symlink() {
                return Err(format!("archive contains a link, refusing: {}", p.display()));
            }
            let canon = fs::canonicalize(&p).map_err(|e| e.to_string())?;
            if !canon.starts_with(&canon_root) {
                return Err(format!("archive entry escapes its folder: {}", p.display()));
            }
            if meta.is_dir() {
                stack.push(p);
            }
        }
    }
    Ok(())
}

/// If the archive had one top-level folder, use that as the emulator root.
fn content_root(staging: &Path) -> Result<PathBuf, String> {
    let entries: Vec<_> = fs::read_dir(staging)
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e: std::io::Error| e.to_string())?;
    if entries.len() == 1 && entries[0].path().is_dir() {
        Ok(entries[0].path())
    } else {
        Ok(staging.to_path_buf())
    }
}

fn find_exe(root: &Path, re: &regex::Regex) -> Option<PathBuf> {
    let mut level = vec![root.to_path_buf()];
    while !level.is_empty() {
        let mut next = vec![];
        for dir in &level {
            let Ok(rd) = fs::read_dir(dir) else { continue };
            let mut entries: Vec<_> = rd.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let p = e.path();
                if p.is_dir() {
                    next.push(p);
                } else if p.file_name().and_then(|n| n.to_str()).map(|n| re.is_match(n)).unwrap_or(false) {
                    return Some(p);
                }
            }
        }
        level = next;
    }
    None
}

// ---------- commands ----------

#[tauri::command]
pub fn list_emulators(app: AppHandle) -> Result<Vec<EmulatorEntry>, String> {
    let reg = registry::load(&app)?;
    let mut out: Vec<EmulatorEntry> = vec![];
    for s in &reg.systems {
        let id = slug(&s.emulator);
        if let Some(e) = out.iter_mut().find(|e| e.id == id) {
            e.systems.push(s.name.clone());
            continue;
        }
        let source_ready = match &s.source {
            Source::Github { .. } => true,
            Source::Page { asset_pattern, .. } => asset_pattern.is_some(),
        };
        let installed = emu_dir(&app, &id).ok().and_then(|d| read_installed(&d));
        out.push(EmulatorEntry {
            id,
            name: s.emulator.clone(),
            systems: vec![s.name.clone()],
            installable: source_ready && s.exe_pattern.is_some(),
            installed,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn install_emulator(
    app: AppHandle,
    state: tauri::State<'_, Installing>,
    id: String,
) -> Result<InstalledInfo, String> {
    {
        let mut set = state.0.lock().unwrap();
        if !set.insert(id.clone()) {
            return Err("this emulator is already being installed".into());
        }
    }
    let res = do_install(&app, &id).await;
    state.0.lock().unwrap().remove(&id);
    if let Err(e) = &res {
        eprintln!("[installer] {id}: {e}");
    }
    res
}

async fn do_install(app: &AppHandle, id: &str) -> Result<InstalledInfo, String> {
    if !valid_id(id) {
        return Err("invalid emulator id".into());
    }
    let reg = registry::load(app)?;
    let sys = reg
        .systems
        .iter()
        .find(|s| slug(&s.emulator) == id)
        .ok_or("unknown emulator")?;
    let exe_pat = sys.exe_pattern.clone().ok_or("the registry has no exePattern for this emulator yet")?;
    let exe_re = RegexBuilder::new(&exe_pat)
        .case_insensitive(true)
        .build()
        .map_err(|e| format!("bad exePattern: {e}"))?;
    let portable = sys.portable_marker.clone();

    let client = http_client()?;
    progress(app, id, "resolving", 0, 0);
    let r = resolve(&client, &sys.source).await?;

    let base = data_dir(app)?;
    let dl_dir = base.join("downloads");
    let emu_root = emulators_dir(app)?;
    tokio::fs::create_dir_all(&dl_dir).await.map_err(|e| e.to_string())?;
    tokio::fs::create_dir_all(&emu_root).await.map_err(|e| e.to_string())?;

    let archive = dl_dir.join(&r.file_name);
    download(app, &client, id, &r, &archive).await?;

    progress(app, id, "extracting", 0, 0);
    let staging = emu_root.join(format!("{id}.staging"));
    let final_dir = emu_root.join(id);
    let prev_dir = emu_root.join(format!("{id}.prev"));
    let version = r.version.clone();
    let asset_name = r.file_name.clone();
    let archive2 = archive.clone();

    let info = tokio::task::spawn_blocking(move || -> Result<InstalledInfo, String> {
        if staging.exists() {
            fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
        }
        let result = (|| {
            extract(&archive2, &staging)?;
            verify_tree(&staging)?;
            let root = content_root(&staging)?;
            let exe = find_exe(&root, &exe_re)
                .ok_or("extracted fine, but the emulator's .exe was not found (check exePattern)")?;
            if let Some(marker) = &portable {
                if let Some(parent) = exe.parent() {
                    fs::write(parent.join(marker), b"").map_err(|e| e.to_string())?;
                }
            }
            let rel = exe.strip_prefix(&root).map_err(|e| e.to_string())?.to_string_lossy().to_string();
            let info = InstalledInfo {
                version,
                asset_name,
                installed_at: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                exe_rel_path: rel,
            };
            fs::write(root.join(".trayload.json"), serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;

            // swap in, keeping the previous version for rollback
            if prev_dir.exists() {
                fs::remove_dir_all(&prev_dir).map_err(|e| e.to_string())?;
            }
            let had_old = final_dir.exists();
            if had_old {
                fs::rename(&final_dir, &prev_dir).map_err(|e| e.to_string())?;
            }
            if let Err(e) = fs::rename(&root, &final_dir) {
                if had_old {
                    let _ = fs::rename(&prev_dir, &final_dir);
                }
                return Err(format!("could not move the new version into place: {e}"));
            }
            Ok(info)
        })();
        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_file(&archive2);
        result
    })
    .await
    .map_err(|e| e.to_string())??;

    progress(app, id, "done", 0, 0);
    Ok(info)
}

#[tauri::command]
pub fn launch_emulator(app: AppHandle, id: String) -> Result<(), String> {
    let dir = emu_dir(&app, &id)?;
    let info = read_installed(&dir).ok_or("not installed")?;
    let exe = dir.join(&info.exe_rel_path);
    if !exe.exists() {
        return Err("the emulator's files are missing, please reinstall it".into());
    }
    let cwd = exe.parent().unwrap_or(&dir);
    std::process::Command::new(&exe)
        .current_dir(cwd)
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", exe.display()))?;
    Ok(())
}

#[tauri::command]
pub fn uninstall_emulator(app: AppHandle, id: String) -> Result<(), String> {
    let dir = emu_dir(&app, &id)?;
    let prev = emulators_dir(&app)?.join(format!("{id}.prev"));
    for d in [dir, prev] {
        if d.exists() {
            fs::remove_dir_all(&d).map_err(|e| format!("could not remove {}: {e}", d.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("Mesen 2"), "mesen-2");
        assert_eq!(slug("DuckStation"), "duckstation");
        assert_eq!(slug("Xenia Canary"), "xenia-canary");
    }

    #[test]
    fn host_allowlist() {
        assert!(check_host("https://github.com/x/y").is_ok());
        assert!(check_host("http://github.com/x/y").is_err());
        assert!(check_host("https://evil.example.com/a.zip").is_err());
        assert!(check_host("https://github.com.evil.example.com/a.zip").is_err());
    }

    #[test]
    fn ids() {
        assert!(valid_id("duckstation"));
        assert!(!valid_id("../x"));
        assert!(!valid_id(""));
    }
}
