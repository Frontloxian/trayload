//! Trayload's games folder: dragged-in games are copied here, one subfolder per console.
//! Also permanent delete. Files in this folder go to the Recycle Bin; files anywhere else are never touched.

use crate::library::{
    self, collect_files, disc_tag, e, get_folders, key, lock, referenced_tracks, set_folders, sheet_tracks, unix_now,
    Db, Detected, Detector, Summary,
};
use crate::registry;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

const CHUNK: usize = 4 * 1024 * 1024;
const SPACE_MARGIN: u64 = 64 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct CopyState {
    busy: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

struct BusyGuard(Arc<AtomicBool>);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl CopyState {
    fn start(&self) -> Result<BusyGuard, String> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err("Games are already being copied. Wait for that to finish first.".into());
        }
        self.cancel.store(false, Ordering::SeqCst);
        Ok(BusyGuard(self.busy.clone()))
    }
}

// ---------- settings and folders ----------

fn setting(db: &Db, name: &str) -> Option<String> {
    let conn = db.0.lock().ok()?;
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [name], |r| r.get::<_, String>(0))
        .optional()
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
}

fn default_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().document_dir().map_err(e)?.join("Trayload").join("Games"))
}

pub fn games_root(app: &AppHandle, db: &Db) -> Result<PathBuf, String> {
    match setting(db, "games_root") {
        Some(p) => Ok(PathBuf::from(p)),
        None => default_root(app),
    }
}

/// Every folder that is scanned: the ones the person added, plus Trayload's own games folder.
pub fn scan_folders(app: &AppHandle, db: &Db) -> Result<Vec<String>, String> {
    let mut v = get_folders(db)?;
    let root = games_root(app, db)?;
    let _ = fs::create_dir_all(&root);
    if !v.iter().any(|f| key(Path::new(f)) == key(&root)) {
        v.push(root.to_string_lossy().to_string());
    }
    Ok(v)
}

fn inside(path: &Path, folder: &Path) -> bool {
    let k = key(path);
    let f = key(folder);
    k.starts_with(&format!("{}\\", f.trim_end_matches('\\')))
}

/// Games from outside Trayload's folders are copied in. Games already inside a scanned folder stay put.
fn should_copy(path: &Path, folders: &[String]) -> bool {
    !folders.iter().any(|f| inside(path, Path::new(f)))
}

/// A folder name for a console: forbidden characters become dashes ("Sega Genesis / Mega Drive").
pub fn system_folder_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = s.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    if s.is_empty() {
        "Other".into()
    } else {
        s
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamesFolder {
    path: String,
    is_default: bool,
    exists: bool,
}

#[tauri::command]
pub fn get_games_folder(app: AppHandle, db: tauri::State<'_, Db>) -> Result<GamesFolder, String> {
    let root = games_root(&app, &db)?;
    Ok(GamesFolder {
        path: root.to_string_lossy().to_string(),
        is_default: setting(&db, "games_root").is_none(),
        exists: root.is_dir(),
    })
}

/// Changes where new games are copied to. Games already in the old folder stay in the library:
/// the old folder is kept as a scanned folder.
#[tauri::command]
pub fn set_games_folder(app: AppHandle, db: tauri::State<'_, Db>, path: String) -> Result<(), String> {
    let new_root = PathBuf::from(path.trim());
    if new_root.as_os_str().is_empty() {
        return Err("Pick a folder.".into());
    }
    fs::create_dir_all(&new_root).map_err(|x| format!("could not use that folder: {x}"))?;
    let old = games_root(&app, &db)?;
    if key(&old) != key(&new_root) && old.is_dir() {
        let mut folders = get_folders(&db)?;
        if !folders.iter().any(|f| key(Path::new(f)) == key(&old)) {
            folders.push(old.to_string_lossy().to_string());
            set_folders(&db, &folders)?;
        }
    }
    let conn = lock(&db)?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('games_root', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [new_root.to_string_lossy().to_string()],
    )
    .map_err(e)?;
    Ok(())
}

#[tauri::command]
pub fn open_games_folder(app: AppHandle, db: tauri::State<'_, Db>) -> Result<(), String> {
    let root = games_root(&app, &db)?;
    fs::create_dir_all(&root).map_err(e)?;
    std::process::Command::new("explorer")
        .arg(&root)
        .spawn()
        .map_err(|x| format!("could not open Explorer: {x}"))?;
    Ok(())
}

#[tauri::command]
pub fn cancel_copy(state: tauri::State<'_, CopyState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

// ---------- sets of files that belong to one game ----------

/// The files that belong with a game file: a cue sheet's tracks, a .ccd's .img and .sub, and so on.
fn companions(head: &Path) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    seen.insert(key(head));
    sheet_tracks(head).into_iter().filter(|p| p.is_file() && seen.insert(key(p))).collect()
}

struct Group {
    heads: Vec<PathBuf>,
    system: String,
    /// Name for the game's own folder when it has several files.
    name: String,
}

struct Job {
    src: PathBuf,
    rel: PathBuf,
    size: u64,
}

fn jobs_of(g: &Group) -> Vec<Job> {
    let base = g.heads[0].parent().map(Path::to_path_buf).unwrap_or_default();
    let mut seen = HashSet::new();
    let mut out = vec![];
    for h in &g.heads {
        let mut all = vec![h.clone()];
        all.extend(companions(h));
        for f in all {
            if !seen.insert(key(&f)) {
                continue;
            }
            let rel = match f.strip_prefix(&base) {
                Ok(r) if r.components().all(|c| matches!(c, Component::Normal(_))) => r.to_path_buf(),
                _ => PathBuf::from(f.file_name().unwrap_or_default()),
            };
            let size = fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
            out.push(Job { src: f, rel, size });
        }
    }
    out
}

fn same_content(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (fs::metadata(a), fs::metadata(b)) else { return false };
    if ma.len() != mb.len() {
        return false;
    }
    let (Ok(mut fa), Ok(mut fb)) = (fs::File::open(a), fs::File::open(b)) else { return false };
    let (mut x, mut y) = (vec![0u8; 1 << 20], vec![0u8; 1 << 20]);
    loop {
        let Ok(n) = fa.read(&mut x) else { return false };
        if n == 0 {
            return true;
        }
        if fb.read_exact(&mut y[..n]).is_err() || x[..n] != y[..n] {
            return false;
        }
    }
}

fn numbered(stem: &str, ext: &str, n: u32) -> String {
    match (n, ext.is_empty()) {
        (1, true) => stem.to_string(),
        (1, false) => format!("{stem}.{ext}"),
        (_, true) => format!("{stem} ({n})"),
        (_, false) => format!("{stem} ({n}).{ext}"),
    }
}

/// Where each file of a game goes. Never overwrites: a different file with the same name gets
/// a number. Returns the destination of every job, and whether an identical copy is already there.
fn choose_dest(sysdir: &Path, g: &Group, jobs: &[Job]) -> (Vec<PathBuf>, bool) {
    if jobs.len() == 1 {
        let name = jobs[0].rel.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, x)) => (s.to_string(), x.to_string()),
            None => (name.clone(), String::new()),
        };
        for n in 1.. {
            let cand = sysdir.join(numbered(&stem, &ext, n));
            if !cand.exists() {
                return (vec![cand], false);
            }
            if same_content(&jobs[0].src, &cand) {
                return (vec![cand], true);
            }
        }
    }
    let name = system_folder_name(&g.name);
    for n in 1.. {
        let dir = sysdir.join(numbered(&name, "", n));
        let dests: Vec<PathBuf> = jobs.iter().map(|j| dir.join(&j.rel)).collect();
        if !dir.exists() {
            return (dests, false);
        }
        if jobs.iter().zip(&dests).all(|(j, d)| same_content(&j.src, d)) {
            return (dests, true);
        }
    }
    unreachable!()
}

// ---------- copying with progress ----------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyProgress {
    name: String,
    files_done: usize,
    files_total: usize,
    bytes_done: u64,
    bytes_total: u64,
}

struct Prog {
    total: u64,
    done: u64,
    files_total: usize,
    files_done: usize,
    name: String,
    last: Instant,
    emit: Box<dyn Fn(&CopyProgress) + Send>,
}

impl Prog {
    fn send(&mut self, force: bool) {
        if !force && self.last.elapsed() < Duration::from_millis(200) {
            return;
        }
        self.last = Instant::now();
        (self.emit)(&CopyProgress {
            name: self.name.clone(),
            files_done: self.files_done,
            files_total: self.files_total,
            bytes_done: self.done,
            bytes_total: self.total,
        });
    }
}

enum CopyErr {
    Cancelled,
    Other(String),
}

fn copy_one(src: &Path, dst: &Path, prog: &mut Prog, cancel: &AtomicBool) -> Result<(), CopyErr> {
    let other = |m: String| CopyErr::Other(m);
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p).map_err(|x| other(format!("could not create {}: {x}", p.display())))?;
    }
    let mut tmp_name = dst.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".trayload-part");
    let tmp = dst.with_file_name(tmp_name);
    let result = (|| -> Result<(), CopyErr> {
        let mut r = fs::File::open(src).map_err(|x| other(format!("could not read {}: {x}", src.display())))?;
        let mut w = fs::File::create(&tmp).map_err(|x| other(format!("could not write {}: {x}", tmp.display())))?;
        let mut buf = vec![0u8; CHUNK];
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(CopyErr::Cancelled);
            }
            let n = r.read(&mut buf).map_err(|x| other(format!("could not read {}: {x}", src.display())))?;
            if n == 0 {
                break;
            }
            w.write_all(&buf[..n]).map_err(|x| other(format!("could not write {}: {x}", dst.display())))?;
            prog.done += n as u64;
            prog.send(false);
        }
        w.flush().map_err(|x| other(x.to_string()))?;
        drop(w);
        fs::rename(&tmp, dst).map_err(|x| other(format!("could not finish {}: {x}", dst.display())))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[derive(Default)]
struct Outcome {
    /// Destination of each group's first file (None if that group was not copied).
    dest_heads: Vec<Option<PathBuf>>,
    dest_files: Vec<PathBuf>,
    /// For each group, which source file went to which destination (empty if the group was not copied).
    mapping: Vec<Vec<(PathBuf, PathBuf)>>,
    handled: HashSet<String>,
    copied: usize,
    skipped: usize,
    failed: Vec<String>,
    cancelled: bool,
}

fn format_bytes(b: u64) -> String {
    let gb = b as f64 / 1_073_741_824.0;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", b as f64 / 1_048_576.0)
    }
}

fn run_groups(
    app: &AppHandle,
    state: &CopyState,
    root: &Path,
    reg: &registry::Registry,
    groups: Vec<Group>,
) -> Result<Outcome, String> {
    fs::create_dir_all(root).map_err(|x| format!("could not create the games folder: {x}"))?;
    let all_jobs: Vec<Vec<Job>> = groups.iter().map(jobs_of).collect();
    let total: u64 = all_jobs.iter().flatten().map(|j| j.size).sum();
    let mut out = Outcome::default();
    for j in all_jobs.iter().flatten() {
        out.handled.insert(key(&j.src));
    }
    if let Ok(free) = fs2::available_space(root) {
        if free < total + SPACE_MARGIN {
            return Err(format!(
                "Not enough free space for the games folder: they need {} and only {} is free.",
                format_bytes(total),
                format_bytes(free)
            ));
        }
    }
    let app2 = app.clone();
    let mut prog = Prog {
        total,
        done: 0,
        files_total: all_jobs.iter().map(|j| j.len()).sum(),
        files_done: 0,
        name: String::new(),
        last: Instant::now() - Duration::from_secs(1),
        emit: Box::new(move |p| {
            let _ = app2.emit("copy-progress", p);
        }),
    };
    for (g, jobs) in groups.iter().zip(&all_jobs) {
        if state.cancel.load(Ordering::SeqCst) {
            out.cancelled = true;
            out.dest_heads.push(None);
            out.mapping.push(vec![]);
            continue;
        }
        let sys_name = reg.systems.iter().find(|s| s.id == g.system).map(|s| s.name.clone()).unwrap_or_else(|| g.system.clone());
        let sysdir = root.join(system_folder_name(&sys_name));
        let (dests, already) = choose_dest(&sysdir, g, jobs);
        prog.name = g.name.clone();
        if already {
            prog.done += jobs.iter().map(|j| j.size).sum::<u64>();
            prog.files_done += jobs.len();
            prog.send(true);
            out.skipped += 1;
            out.dest_heads.push(Some(dests[0].clone()));
            out.mapping.push(jobs.iter().zip(&dests).map(|(j, d)| (j.src.clone(), d.clone())).collect());
            out.dest_files.extend(dests);
            continue;
        }
        let mut created: Vec<PathBuf> = vec![];
        let mut failure: Option<CopyErr> = None;
        for (j, d) in jobs.iter().zip(&dests) {
            prog.send(true);
            match copy_one(&j.src, d, &mut prog, &state.cancel) {
                Ok(()) => {
                    created.push(d.clone());
                    prog.files_done += 1;
                }
                Err(x) => {
                    failure = Some(x);
                    break;
                }
            }
        }
        match failure {
            None => {
                out.copied += 1;
                out.dest_heads.push(Some(dests[0].clone()));
                out.mapping.push(jobs.iter().zip(&dests).map(|(j, d)| (j.src.clone(), d.clone())).collect());
                out.dest_files.extend(dests);
            }
            Some(x) => {
                for f in &created {
                    let _ = fs::remove_file(f);
                }
                if jobs.len() > 1 {
                    if let Some(dir) = dests[0].parent() {
                        let _ = fs::remove_dir(dir);
                    }
                }
                out.dest_heads.push(None);
                out.mapping.push(vec![]);
                match x {
                    CopyErr::Cancelled => out.cancelled = true,
                    CopyErr::Other(m) => out.failed.push(format!("{}: {m}", g.name)),
                }
            }
        }
    }
    prog.send(true);
    Ok(out)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CopyFinished {
    copied: usize,
    skipped: usize,
    failed: Vec<String>,
    cancelled: bool,
}

fn import_dropped(app: &AppHandle, db: &Db, state: &CopyState, paths: Vec<PathBuf>) -> Result<Summary, String> {
    let reg = registry::load(app)?;
    let det = Detector::new(&reg.systems);
    let root = games_root(app, db)?;
    let folders = scan_folders(app, db)?;
    let files = collect_files(&paths);
    let skip = referenced_tracks(&files);

    let mut groups: Vec<Group> = vec![];
    let mut by_disc: HashMap<(String, String, String), usize> = HashMap::new();
    for f in &files {
        if skip.contains(&key(f)) || !should_copy(f, &folders) {
            continue;
        }
        let Some(Detected::System(sys)) = det.detect(f) else { continue };
        let stem = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        match disc_tag(&stem) {
            Some((base, _)) => {
                let k = (f.parent().map(key).unwrap_or_default(), base.to_lowercase(), sys.clone());
                if let Some(&i) = by_disc.get(&k) {
                    groups[i].heads.push(f.clone());
                } else {
                    by_disc.insert(k, groups.len());
                    groups.push(Group { heads: vec![f.clone()], system: sys, name: base });
                }
            }
            None => groups.push(Group { heads: vec![f.clone()], system: sys, name: stem }),
        }
    }

    let guard = if groups.is_empty() { None } else { Some(state.start()?) };
    let out = if groups.is_empty() {
        Outcome::default()
    } else {
        run_groups(app, state, &root, &reg, groups)?
    };
    drop(guard);
    for pairs in &out.mapping {
        if let Ok(conn) = lock(db) {
            let _ = record_origins(&conn, pairs, false);
        }
    }

    let mut to_import: Vec<PathBuf> = files.into_iter().filter(|f| !out.handled.contains(&key(f))).collect();
    to_import.extend(out.dest_files.iter().cloned());
    let summary = library::import_files(db, &det, to_import)?;
    if out.copied + out.skipped + out.failed.len() > 0 || out.cancelled {
        let _ = app.emit(
            "copy-finished",
            &CopyFinished { copied: out.copied, skipped: out.skipped, failed: out.failed, cancelled: out.cancelled },
        );
    }
    Ok(summary)
}

#[tauri::command]
pub async fn import_paths(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    state: tauri::State<'_, CopyState>,
    paths: Vec<String>,
) -> Result<Summary, String> {
    let (db, state) = (db.inner().clone(), state.inner().clone());
    let app2 = app.clone();
    let s = tokio::task::spawn_blocking(move || {
        import_dropped(&app2, &db, &state, paths.iter().map(PathBuf::from).collect())
    })
    .await
    .map_err(e)??;
    let _ = app.emit("library-changed", &s);
    Ok(s)
}

/// Gives a waiting file its console. If the file is outside Trayload's folders it is copied into
/// that console's folder first.
#[tauri::command]
pub async fn assign_system(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    state: tauri::State<'_, CopyState>,
    path: String,
    system_id: String,
) -> Result<(), String> {
    let (db, state) = (db.inner().clone(), state.inner().clone());
    let app2 = app.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let reg = registry::load(&app2)?;
        if !reg.systems.iter().any(|s| s.id == system_id) {
            return Err("unknown system".into());
        }
        {
            let conn = lock(&db)?;
            conn.query_row("SELECT 1 FROM unassigned WHERE path = ?1", [&path], |r| r.get::<_, i64>(0))
                .map_err(|_| "that file is no longer waiting for a system".to_string())?;
        }
        let root = games_root(&app2, &db)?;
        let folders = scan_folders(&app2, &db)?;
        let mut final_path = PathBuf::from(&path);
        if should_copy(&final_path, &folders) {
            let _guard = state.start()?;
            let stem = final_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let g = Group { heads: vec![final_path.clone()], system: system_id.clone(), name: stem };
            let out = run_groups(&app2, &state, &root, &reg, vec![g])?;
            if let (Some(pairs), Ok(conn)) = (out.mapping.first(), lock(&db)) {
                let _ = record_origins(&conn, pairs, false);
            }
            let _ = app2.emit(
                "copy-finished",
                &CopyFinished { copied: out.copied, skipped: out.skipped, failed: out.failed.clone(), cancelled: out.cancelled },
            );
            match out.dest_heads.into_iter().next().flatten() {
                Some(p) => final_path = p,
                None if out.cancelled => return Err("Copying was cancelled.".into()),
                None => return Err(out.failed.first().cloned().unwrap_or_else(|| "The game could not be copied.".into())),
            }
        }
        let meta = fs::metadata(&final_path).map_err(|x| format!("cannot read the game file: {x}"))?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let mut conn = lock(&db)?;
        let tx = conn.transaction().map_err(e)?;
        tx.execute(
            "INSERT OR IGNORE INTO games (system_id, title, path, size, mtime, added_at) VALUES (?1,?2,?3,?4,?5,?6)",
            params![system_id, library::title_of(&final_path), final_path.to_string_lossy().to_string(), meta.len() as i64, mtime, unix_now()],
        )
        .map_err(e)?;
        tx.execute("DELETE FROM unassigned WHERE path = ?1", [&path]).map_err(e)?;
        tx.commit().map_err(e)
    })
    .await
    .map_err(e)??;
    let _ = app.emit("library-changed", ());
    Ok(())
}

// ---------- permanent delete ----------

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSummary {
    deleted: usize,
    /// Files moved to the Recycle Bin from Trayload's games folder.
    recycled: usize,
    /// Files left alone because they are outside Trayload's games folder.
    left_alone: usize,
    /// Files of games that had been moved in: put back where they came from.
    returned: usize,
    failed: Vec<String>,
}

/// Removes a game's own folder once it is empty. Never the console folder or the games folder.
fn tidy_parent(f: &Path, root: &Path) {
    if let Some(parent) = f.parent() {
        let rel = key(parent).strip_prefix(&format!("{}\\", key(root))).map(|r| r.to_string());
        if rel.map(|r| r.contains('\\')).unwrap_or(false) {
            let _ = fs::remove_dir(parent);
        }
    }
}

/// Puts a file back at `original`, creating its folders. Never overwrites: a taken name gets a number.
fn return_file(src: &Path, original: &Path) -> Result<PathBuf, String> {
    let dir = original.parent().ok_or("the original location has no folder")?;
    fs::create_dir_all(dir).map_err(|x| format!("could not create {}: {x}", dir.display()))?;
    let name = original.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, x)) => (s.to_string(), x.to_string()),
        None => (name.clone(), String::new()),
    };
    let mut target = original.to_path_buf();
    let mut n = 2;
    while target.exists() {
        target = dir.join(numbered(&stem, &ext, n));
        n += 1;
    }
    if fs::rename(src, &target).is_err() {
        // Different drive: copy, then remove the source.
        fs::copy(src, &target).map_err(|x| x.to_string())?;
        fs::remove_file(src).map_err(|x| x.to_string())?;
    }
    Ok(target)
}

fn purge(app: &AppHandle, db: &Db, ids: Vec<i64>) -> Result<DeleteSummary, String> {
    let root = games_root(app, db)?;
    let folders = scan_folders(app, db)?;
    let art_root = app.path().app_local_data_dir().map_err(e)?.join("art");
    let mut sum = DeleteSummary::default();
    for id in ids {
        let (path, title, discs): (String, String, Vec<String>) = {
            let conn = lock(db)?;
            let Some((p, t)) = conn
                .query_row("SELECT path, title FROM games WHERE id = ?1 AND removed = 1", [id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .optional()
                .map_err(e)?
            else {
                continue;
            };
            let mut st = conn.prepare("SELECT path FROM game_discs WHERE game_id = ?1").map_err(e)?;
            let rows = st.query_map([id], |r| r.get::<_, String>(0)).map_err(e)?;
            let d: Vec<String> = rows.collect::<Result<_, _>>().map_err(e)?;
            (p, t, d)
        };
        let heads: Vec<PathBuf> = std::iter::once(path).chain(discs).map(PathBuf::from).collect();
        let mut files: Vec<PathBuf> = vec![];
        let mut seen = HashSet::new();
        for h in &heads {
            for f in std::iter::once(h.clone()).chain(companions(h)) {
                if f.is_file() && seen.insert(key(&f)) {
                    files.push(f);
                }
            }
        }
        let files_all = files.clone();
        let (own, other): (Vec<PathBuf>, Vec<PathBuf>) = files.into_iter().partition(|f| inside(f, &root));
        sum.left_alone += other.len();
        let mut ignore: Vec<PathBuf> =
            heads.iter().filter(|h| h.is_file() && !inside(h, &root) && folders.iter().any(|f| inside(h, Path::new(f)))).cloned().collect();
        // Games that were moved into the games folder go back to where they came from.
        let moved: HashMap<String, PathBuf> = {
            let conn = lock(db)?;
            let mut m = HashMap::new();
            for f in &own {
                let o: Option<String> = conn
                    .query_row(
                        "SELECT original FROM file_origins WHERE dest = ?1 AND moved = 1",
                        [f.to_string_lossy().to_string()],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(e)?;
                if let Some(o) = o {
                    m.insert(key(f), PathBuf::from(o));
                }
            }
            m
        };
        let (give_back, own): (Vec<PathBuf>, Vec<PathBuf>) = own.into_iter().partition(|f| moved.contains_key(&key(f)));
        for f in &give_back {
            match return_file(f, &moved[&key(f)]) {
                Ok(back) => {
                    sum.returned += 1;
                    if folders.iter().any(|d| inside(&back, Path::new(d))) {
                        ignore.push(back);
                    }
                    tidy_parent(f, &root);
                }
                Err(x) => {
                    sum.failed.push(format!("{title}: could not put {} back ({x})", f.display()));
                    ignore.push(f.clone());
                }
            }
        }
        if !own.is_empty() {
            match trash::delete_all(&own) {
                Ok(()) => {
                    sum.recycled += own.len();
                    for f in &own {
                        tidy_parent(f, &root);
                    }
                }
                Err(x) => {
                    sum.failed.push(format!("{title}: could not move to the Recycle Bin ({x})"));
                    ignore.extend(heads.iter().filter(|h| h.is_file() && inside(h, &root)).cloned());
                }
            }
        }
        {
            let mut conn = lock(db)?;
            let tx = conn.transaction().map_err(e)?;
            for p in &ignore {
                tx.execute("INSERT OR IGNORE INTO ignored_files (path) VALUES (?1)", [p.to_string_lossy().to_string()]).map_err(e)?;
            }
            for h in &heads {
                tx.execute("DELETE FROM unassigned WHERE path = ?1", [h.to_string_lossy().to_string()]).map_err(e)?;
            }
            for f in files_all.iter() {
                tx.execute("DELETE FROM file_origins WHERE dest = ?1", [f.to_string_lossy().to_string()]).map_err(e)?;
            }
            for sql in [
                "DELETE FROM collection_games WHERE game_id = ?1",
                "DELETE FROM game_overrides WHERE game_id = ?1",
                "DELETE FROM play_sessions WHERE game_id = ?1",
                "DELETE FROM game_discs WHERE game_id = ?1",
                "DELETE FROM game_verify WHERE game_id = ?1",
                "DELETE FROM games WHERE id = ?1",
            ] {
                tx.execute(sql, [id]).map_err(e)?;
            }
            tx.commit().map_err(e)?;
        }
        let art = art_root.join(id.to_string());
        if art.is_dir() {
            let _ = fs::remove_dir_all(&art);
        }
        sum.deleted += 1;
    }
    Ok(sum)
}

#[tauri::command]
pub async fn delete_removed_game(app: AppHandle, db: tauri::State<'_, Db>, id: i64) -> Result<DeleteSummary, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || purge(&app, &db, vec![id])).await.map_err(e)?
}

#[tauri::command]
pub async fn delete_all_removed(app: AppHandle, db: tauri::State<'_, Db>) -> Result<DeleteSummary, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let ids: Vec<i64> = {
            let conn = lock(&db)?;
            let mut st = conn.prepare("SELECT id FROM games WHERE removed = 1").map_err(e)?;
            let rows = st.query_map([], |r| r.get::<_, i64>(0)).map_err(e)?;
            rows.collect::<Result<_, _>>().map_err(e)?
        };
        purge(&app, &db, ids)
    })
    .await
    .map_err(e)?
}

#[tauri::command]
pub fn list_ignored_files(db: tauri::State<'_, Db>) -> Result<Vec<String>, String> {
    let conn = lock(&db)?;
    let mut st = conn.prepare("SELECT path FROM ignored_files ORDER BY path COLLATE NOCASE").map_err(e)?;
    let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(e)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(e)
}

/// Forget ignored files (all of them, or one). They come back on the next scan if they are still in a scanned folder.
#[tauri::command]
pub fn clear_ignored_files(db: tauri::State<'_, Db>, path: Option<String>) -> Result<(), String> {
    let conn = lock(&db)?;
    match path {
        Some(p) => conn.execute("DELETE FROM ignored_files WHERE path = ?1", [p]),
        None => conn.execute("DELETE FROM ignored_files", []),
    }
    .map_err(e)?;
    Ok(())
}

/// Remembers where a copied game's files came from, so the originals can be cleaned up later
/// and a moved game's files can be put back.
fn record_origins(conn: &rusqlite::Connection, pairs: &[(PathBuf, PathBuf)], moved: bool) -> Result<(), String> {
    for (src, dst) in pairs {
        conn.execute(
            "INSERT INTO file_origins (dest, original, moved) VALUES (?1, ?2, ?3)
             ON CONFLICT(dest) DO UPDATE SET original = excluded.original, moved = MAX(moved, excluded.moved)",
            params![dst.to_string_lossy().to_string(), src.to_string_lossy().to_string(), moved as i64],
        )
        .map_err(e)?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverOriginal {
    original: String,
    dest: String,
    bytes: u64,
}

/// Original files that were copied into the games folder and are still where they were.
#[tauri::command]
pub async fn plan_originals(db: tauri::State<'_, Db>) -> Result<Vec<LeftoverOriginal>, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let rows: Vec<(String, String)> = {
            let conn = lock(&db)?;
            let mut st = conn
                .prepare("SELECT original, dest FROM file_origins WHERE moved = 0 ORDER BY original COLLATE NOCASE")
                .map_err(e)?;
            let r = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(e)?;
            r.collect::<Result<_, _>>().map_err(e)?
        };
        let mut out = vec![];
        for (original, dest) in rows {
            let (o, d) = (Path::new(&original), Path::new(&dest));
            if let (Ok(mo), Ok(md)) = (fs::metadata(o), fs::metadata(d)) {
                if mo.is_file() && md.is_file() && mo.len() == md.len() {
                    out.push(LeftoverOriginal { original, dest, bytes: mo.len() });
                }
            }
        }
        Ok(out)
    })
    .await
    .map_err(e)?
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OriginalsResult {
    removed: usize,
    failed: Vec<String>,
}

/// Sends original files to the Recycle Bin, but only after checking that the copy in the games folder is identical.
/// Deleting that game permanently later puts the files back.
#[tauri::command]
pub async fn remove_originals(db: tauri::State<'_, Db>, originals: Vec<String>) -> Result<OriginalsResult, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let mut res = OriginalsResult::default();
        for original in originals {
            let dest: Option<String> = {
                let conn = lock(&db)?;
                conn.query_row("SELECT dest FROM file_origins WHERE original = ?1 AND moved = 0", [&original], |r| r.get(0))
                    .optional()
                    .map_err(e)?
            };
            let Some(dest) = dest else { continue };
            if !same_content(Path::new(&original), Path::new(&dest)) {
                res.failed.push(format!("{original}: the copy in the games folder is not identical, so it was kept"));
                continue;
            }
            match trash::delete(&original) {
                Ok(()) => {
                    lock(&db)?.execute("UPDATE file_origins SET moved = 1 WHERE dest = ?1", [&dest]).map_err(e)?;
                    res.removed += 1;
                }
                Err(x) => res.failed.push(format!("{original}: {x}")),
            }
        }
        Ok(res)
    })
    .await
    .map_err(e)?
}

// ---------- bring existing games into the games folder ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeCandidate {
    id: i64,
    title: String,
    system_name: String,
    path: String,
    bytes: u64,
}

fn organize_mode(db: &Db) -> String {
    match setting(db, "organize_mode").as_deref() {
        Some("move") => "move".into(),
        _ => "copy".into(),
    }
}

#[tauri::command]
pub fn get_organize_mode(db: tauri::State<'_, Db>) -> String {
    organize_mode(&db)
}

#[tauri::command]
pub fn set_organize_mode(db: tauri::State<'_, Db>, mode: String) -> Result<(), String> {
    if mode != "copy" && mode != "move" {
        return Err("mode must be copy or move".into());
    }
    let conn = lock(&db)?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('organize_mode', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [mode],
    )
    .map_err(e)?;
    Ok(())
}

fn game_group(db: &Db, id: i64) -> Result<Option<(Group, String)>, String> {
    let conn = lock(db)?;
    let Some((path, system, title)) = conn
        .query_row("SELECT path, system_id, title FROM games WHERE id = ?1 AND removed = 0", [id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })
        .optional()
        .map_err(e)?
    else {
        return Ok(None);
    };
    let mut st = conn.prepare("SELECT path FROM game_discs WHERE game_id = ?1 ORDER BY path").map_err(e)?;
    let rows = st.query_map([id], |r| r.get::<_, String>(0)).map_err(e)?;
    let discs: Vec<String> = rows.collect::<Result<_, _>>().map_err(e)?;
    let heads: Vec<PathBuf> = std::iter::once(path.clone()).chain(discs).map(PathBuf::from).collect();
    Ok(Some((Group { heads, system, name: title.clone() }, title)))
}

/// Library games whose files are not in Trayload's games folder yet.
#[tauri::command]
pub async fn plan_organize(app: AppHandle, db: tauri::State<'_, Db>) -> Result<Vec<OrganizeCandidate>, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let reg = registry::load(&app)?;
        let root = games_root(&app, &db)?;
        let rows: Vec<(i64, String, String)> = {
            let conn = lock(&db)?;
            let mut st = conn
                .prepare("SELECT id, path, system_id FROM games WHERE removed = 0 AND missing = 0 ORDER BY title COLLATE NOCASE")
                .map_err(e)?;
            let r = st
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
                .map_err(e)?;
            r.collect::<Result<_, _>>().map_err(e)?
        };
        let mut out = vec![];
        for (id, path, system) in rows {
            if inside(Path::new(&path), &root) || !Path::new(&path).is_file() {
                continue;
            }
            let Some((g, title)) = game_group(&db, id)? else { continue };
            let bytes: u64 = jobs_of(&g).iter().map(|j| j.size).sum();
            let system_name = reg.systems.iter().find(|s| s.id == system).map(|s| s.name.clone()).unwrap_or(system);
            out.push(OrganizeCandidate { id, title, system_name, path, bytes });
        }
        Ok(out)
    })
    .await
    .map_err(e)?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeSummary {
    done: usize,
    failed: Vec<String>,
    cancelled: bool,
    mode: String,
}

/// Copies (default) or moves the chosen games into the games folder and points the library at the new files.
/// A moved game remembers where it came from, so deleting it permanently puts its files back.
#[tauri::command]
pub async fn organize_games(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    state: tauri::State<'_, CopyState>,
    ids: Vec<i64>,
) -> Result<OrganizeSummary, String> {
    let (db, state) = (db.inner().clone(), state.inner().clone());
    let app2 = app.clone();
    let summary = tokio::task::spawn_blocking(move || -> Result<OrganizeSummary, String> {
        let reg = registry::load(&app2)?;
        let root = games_root(&app2, &db)?;
        let folders = scan_folders(&app2, &db)?;
        let mode = organize_mode(&db);
        let moving = mode == "move";
        let mut groups = vec![];
        let mut order = vec![];
        for id in ids {
            if let Some((g, _)) = game_group(&db, id)? {
                if !inside(&g.heads[0], &root) {
                    groups.push(g);
                    order.push(id);
                }
            }
        }
        let guard = state.start()?;
        let heads: Vec<Vec<PathBuf>> = groups.iter().map(|g| g.heads.clone()).collect();
        let out = run_groups(&app2, &state, &root, &reg, groups)?;
        drop(guard);
        let mut done = 0;
        let mut failed = out.failed.clone();
        for (i, id) in order.iter().enumerate() {
            let map = &out.mapping[i];
            if map.is_empty() {
                continue;
            }
            let lookup = |p: &Path| map.iter().find(|(s, _)| key(s) == key(p)).map(|(_, d)| d.clone());
            let Some(new_head) = lookup(&heads[i][0]) else { continue };
            let mut conn = lock(&db)?;
            let tx = conn.transaction().map_err(e)?;
            let clash: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM games WHERE path = ?1 AND id != ?2",
                    params![new_head.to_string_lossy().to_string(), id],
                    |r| r.get(0),
                )
                .map_err(e)?;
            if clash > 0 {
                failed.push(format!("{}: an identical file is already in your library", heads[i][0].display()));
                continue;
            }
            tx.execute(
                "UPDATE games SET path = ?2, missing = 0 WHERE id = ?1",
                params![id, new_head.to_string_lossy().to_string()],
            )
            .map_err(e)?;
            for d in heads[i].iter().skip(1) {
                if let Some(nd) = lookup(d) {
                    tx.execute(
                        "UPDATE game_discs SET path = ?3 WHERE game_id = ?1 AND path = ?2",
                        params![id, d.to_string_lossy().to_string(), nd.to_string_lossy().to_string()],
                    )
                    .map_err(e)?;
                }
            }
            record_origins(&tx, map, false)?;
            if !moving {
                // The originals stay on disk. Keep a scanned folder from adding them again.
                for h in &heads[i] {
                    if folders.iter().any(|f| inside(h, Path::new(f))) {
                        tx.execute("INSERT OR IGNORE INTO ignored_files (path) VALUES (?1)", [h.to_string_lossy().to_string()])
                            .map_err(e)?;
                    }
                }
            }
            tx.commit().map_err(e)?;
            drop(conn);
            if moving {
                for (src, dst) in map {
                    let same = fs::metadata(src).map(|m| m.len()).ok() == fs::metadata(dst).map(|m| m.len()).ok();
                    if same && fs::remove_file(src).is_ok() {
                        if let Ok(conn) = lock(&db) {
                            let _ = conn.execute(
                                "UPDATE file_origins SET moved = 1 WHERE dest = ?1",
                                [dst.to_string_lossy().to_string()],
                            );
                        }
                    }
                }
            }
            done += 1;
        }
        let _ = app2.emit(
            "copy-finished",
            &CopyFinished { copied: done, skipped: 0, failed: vec![], cancelled: out.cancelled },
        );
        Ok(OrganizeSummary { done, failed, cancelled: out.cancelled, mode })
    })
    .await
    .map_err(e)??;
    let _ = app.emit("library-changed", ());
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("trayload-games-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn quiet() -> Prog {
        Prog {
            total: 0,
            done: 0,
            files_total: 0,
            files_done: 0,
            name: String::new(),
            last: Instant::now(),
            emit: Box::new(|_| {}),
        }
    }

    #[test]
    fn console_folder_names() {
        assert_eq!(system_folder_name("Nintendo 64"), "Nintendo 64");
        assert_eq!(system_folder_name("Sega Genesis / Mega Drive"), "Sega Genesis - Mega Drive");
        assert_eq!(system_folder_name("  ..  "), "Other");
    }

    #[test]
    fn numbering() {
        assert_eq!(numbered("Game", "gba", 1), "Game.gba");
        assert_eq!(numbered("Game", "gba", 2), "Game (2).gba");
        assert_eq!(numbered("Game", "", 3), "Game (3)");
    }

    #[test]
    fn copy_never_overwrites_and_skips_identical() {
        let d = tmp("copy");
        let src = d.join("in");
        let sysdir = d.join("games").join("Game Boy Advance");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&sysdir).unwrap();
        fs::write(src.join("Game.gba"), b"hello").unwrap();
        fs::write(sysdir.join("Game.gba"), b"something else").unwrap();
        let g = Group { heads: vec![src.join("Game.gba")], system: "gba".into(), name: "Game".into() };
        let jobs = jobs_of(&g);
        let (dests, already) = choose_dest(&sysdir, &g, &jobs);
        assert!(!already);
        assert_eq!(dests[0], sysdir.join("Game (2).gba"));
        let cancel = AtomicBool::new(false);
        assert!(copy_one(&jobs[0].src, &dests[0], &mut quiet(), &cancel).is_ok());
        assert_eq!(fs::read(sysdir.join("Game.gba")).unwrap(), b"something else");
        assert_eq!(fs::read(sysdir.join("Game (2).gba")).unwrap(), b"hello");
        // Now an identical copy exists, so nothing new is made.
        let (dests2, already2) = choose_dest(&sysdir, &g, &jobs);
        assert!(already2);
        assert_eq!(dests2[0], sysdir.join("Game (2).gba"));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn cue_sets_go_in_their_own_folder() {
        let d = tmp("set");
        let src = d.join("in");
        let sysdir = d.join("games").join("Sony PlayStation");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("Game.cue"), "FILE \"Game.bin\" BINARY\n  TRACK 01 MODE2/2352\n").unwrap();
        fs::write(src.join("Game.bin"), b"data").unwrap();
        let g = Group { heads: vec![src.join("Game.cue")], system: "ps1".into(), name: "Game".into() };
        let jobs = jobs_of(&g);
        assert_eq!(jobs.len(), 2);
        let (dests, _) = choose_dest(&sysdir, &g, &jobs);
        assert_eq!(dests[0], sysdir.join("Game").join("Game.cue"));
        assert_eq!(dests[1], sysdir.join("Game").join("Game.bin"));
        let cancel = AtomicBool::new(false);
        for (j, dst) in jobs.iter().zip(&dests) {
            copy_one(&j.src, dst, &mut quiet(), &cancel).ok().unwrap();
        }
        assert!(sysdir.join("Game").join("Game.bin").is_file());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn cancel_leaves_no_partial_file() {
        let d = tmp("cancel");
        fs::write(d.join("a.bin"), vec![1u8; 100]).unwrap();
        let cancel = AtomicBool::new(true);
        let r = copy_one(&d.join("a.bin"), &d.join("out").join("a.bin"), &mut quiet(), &cancel);
        assert!(matches!(r, Err(CopyErr::Cancelled)));
        assert!(!d.join("out").join("a.bin").exists());
        assert!(!d.join("out").join("a.bin.trayload-part").exists());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn returns_a_file_without_overwriting() {
        let d = tmp("return");
        fs::write(d.join("held.gba"), b"mine").unwrap();
        let original = d.join("gone").join("deep").join("Game.gba");
        // The folder does not exist yet: it is created.
        let back = return_file(&d.join("held.gba"), &original).unwrap();
        assert_eq!(back, original);
        assert_eq!(fs::read(&original).unwrap(), b"mine");
        // Something is already at the original path: the file gets a number.
        fs::write(d.join("held2.gba"), b"other").unwrap();
        let back2 = return_file(&d.join("held2.gba"), &original).unwrap();
        assert_eq!(back2, d.join("gone").join("deep").join("Game (2).gba"));
        assert_eq!(fs::read(&original).unwrap(), b"mine");
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn only_outside_files_are_copied() {
        let folders = vec!["C:\\Users\\me\\Documents\\Trayload\\Games".to_string()];
        assert!(!should_copy(Path::new("C:\\Users\\me\\Documents\\Trayload\\Games\\GBA\\x.gba"), &folders));
        assert!(should_copy(Path::new("C:\\Users\\me\\Downloads\\x.gba"), &folders));
        assert!(should_copy(Path::new("C:\\Users\\me\\Documents\\Trayload\\Games2\\x.gba"), &folders));
    }
}
