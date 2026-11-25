//! Checks game files against the community No-Intro lists.
//!
//! A list is a plain text file of official names with each file's size and CRC32. Trayload gets the
//! list for a console from the libretro-database project on GitHub (a bulk copy of the No-Intro
//! lists, CC-BY-SA-4.0), only when you ask for a check, and keeps it on this PC for 30 days. A list
//! holds names and checksums only, never games. Nothing is sent about you or your files: your
//! files are read and hashed here, and only the result is kept in the library database.
//!
//! Cartridge consoles only for now. Discs (Redump) come later.

use crate::library::Db;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};

const BASE: &str = "https://raw.githubusercontent.com/libretro/libretro-database/master/metadat/no-intro/";
const MAX_DAT_BYTES: usize = 40 * 1024 * 1024;
const MAX_AGE_SECS: u64 = 30 * 24 * 3600;

type Lookup = HashMap<(u32, u64), String>;

fn e<T: ToString>(x: T) -> String {
    x.to_string()
}

/// The No-Intro lists that cover each console (file names in libretro-database, without ".dat").
fn lists_for(system: &str) -> &'static [&'static str] {
    match system {
        "nes" => &[
            "Nintendo - Nintendo Entertainment System",
            "Nintendo - Family Computer Disk System",
        ],
        "snes" => &["Nintendo - Super Nintendo Entertainment System"],
        "gb" => &["Nintendo - Game Boy", "Nintendo - Game Boy Color"],
        "gba" => &["Nintendo - Game Boy Advance"],
        "n64" => &["Nintendo - Nintendo 64"],
        "nds" => &["Nintendo - Nintendo DS", "Nintendo - Nintendo DSi"],
        "sms" => &["Sega - Master System - Mark III"],
        "gg" => &["Sega - Game Gear"],
        "sg1000" => &["Sega - SG-1000"],
        "genesis" => &["Sega - Mega Drive - Genesis"],
        "s32x" => &["Sega - 32X"],
        _ => &[],
    }
}

// ---------- checksums ----------

fn table() -> &'static [u32; 256] {
    static T: OnceLock<[u32; 256]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    })
}

struct Crc(u32);
impl Crc {
    fn new() -> Crc {
        Crc(0xFFFF_FFFF)
    }
    fn update(&mut self, data: &[u8]) {
        let t = table();
        for &b in data {
            self.0 = t[((self.0 ^ b as u32) & 0xFF) as usize] ^ (self.0 >> 8);
        }
    }
    fn finish(&self) -> u32 {
        !self.0
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Order {
    Native,
    /// .v64: every pair of bytes swapped
    Pairs,
    /// .n64: every four bytes reversed
    Words,
}

/// The lists use the big-endian layout (.z64). Other Nintendo 64 dumps are converted for the check.
fn n64_order(first: &[u8]) -> Order {
    match first.get(..4) {
        Some([0x37, 0x80, 0x40, 0x12]) => Order::Pairs,
        Some([0x40, 0x12, 0x37, 0x80]) => Order::Words,
        _ => Order::Native,
    }
}

fn reorder(chunk: &mut [u8], order: Order) {
    match order {
        Order::Native => {}
        Order::Pairs => chunk.chunks_exact_mut(2).for_each(|c| c.swap(0, 1)),
        Order::Words => chunk.chunks_exact_mut(4).for_each(|c| c.reverse()),
    }
}

/// The lists hash a file without its copier or emulator header, so the header size is found first.
fn header_skip(system: &str, first: &[u8], len: u64) -> u64 {
    match system {
        "nes" if first.starts_with(b"NES\x1A") || first.starts_with(b"FDS\x1A") => 16,
        "snes" if len % 1024 == 512 => 512,
        _ => 0,
    }
}

fn fill<R: Read>(r: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        let got = r.read(&mut buf[n..])?;
        if got == 0 {
            break;
        }
        n += got;
    }
    Ok(n)
}

#[derive(Debug, PartialEq)]
enum Hashed {
    /// (crc, size) as the file is, and again without its header if it has one.
    Done(Vec<(u32, u64)>),
    Cancelled,
}

fn hash_reader<R: Read>(mut r: R, len: u64, system: &str, cancel: &AtomicBool) -> Result<Hashed, String> {
    let mut buf = vec![0u8; 1 << 20];
    let mut full = Crc::new();
    let mut headless: Option<Crc> = None;
    let (mut skip, mut order, mut pos) = (0u64, Order::Native, 0u64);
    let mut first = true;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Hashed::Cancelled);
        }
        let n = fill(&mut r, &mut buf).map_err(e)?;
        if n == 0 {
            break;
        }
        let chunk = &mut buf[..n];
        if first {
            first = false;
            if system == "n64" {
                order = n64_order(chunk);
            }
            skip = header_skip(system, chunk, len);
            if skip > 0 {
                headless = Some(Crc::new());
            }
        }
        reorder(chunk, order);
        full.update(chunk);
        if let Some(h) = headless.as_mut() {
            let from = skip.saturating_sub(pos) as usize;
            if from < n {
                h.update(&chunk[from..]);
            }
        }
        pos += n as u64;
    }
    let mut out = vec![(full.finish(), pos)];
    if let Some(h) = headless {
        out.push((h.finish(), pos.saturating_sub(skip)));
    }
    Ok(Hashed::Done(out))
}

fn hash_game(path: &Path, system: &str, cancel: &AtomicBool) -> Result<Hashed, String> {
    let ext = path
        .extension()
        .map(|x| x.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let file = File::open(path).map_err(e)?;
    if ext == "zip" {
        let mut z = zip::ZipArchive::new(file).map_err(e)?;
        let mut only = None;
        for i in 0..z.len() {
            let f = z.by_index(i).map_err(e)?;
            if f.is_dir() {
                continue;
            }
            if only.is_some() {
                return Err("The zip holds more than one file.".into());
            }
            only = Some(i);
        }
        let idx = only.ok_or_else(|| "The zip is empty.".to_string())?;
        let f = z.by_index(idx).map_err(e)?;
        let len = f.size();
        hash_reader(f, len, system, cancel)
    } else {
        let len = file.metadata().map_err(e)?.len();
        hash_reader(file, len, system, cancel)
    }
}

// ---------- reading a list ----------

/// Splits a line into words, keeping "quoted text" together.
fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut quoted, mut any) = (false, false);
    for ch in line.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Reads the clrmamepro list format: `game ( name "..." rom ( name "..." size N crc HEX ... ) )`.
fn parse_list(text: &str, into: &mut Lookup) {
    let mut game: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("game (") || line.starts_with("machine (") {
            game = Some(String::new());
        } else if line.starts_with("rom (") {
            let Some(name) = game.as_ref().filter(|n| !n.is_empty()) else {
                continue;
            };
            let t = tokens(line);
            let find = |key: &str| t.iter().position(|w| w == key).and_then(|i| t.get(i + 1));
            let (Some(size), Some(crc)) = (find("size"), find("crc")) else {
                continue;
            };
            if let (Ok(size), Ok(crc)) = (size.parse::<u64>(), u32::from_str_radix(crc, 16)) {
                into.entry((crc, size)).or_insert_with(|| name.clone());
            }
        } else if line.starts_with("name ") && game.as_ref().is_some_and(|n| n.is_empty()) {
            if let Some(n) = tokens(line).get(1) {
                game = Some(n.clone());
            }
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn download(name: &str) -> Result<String, String> {
    // Only raw.githubusercontent.com, and no redirects.
    let client = reqwest::Client::builder()
        .user_agent("Trayload")
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(e)?;
    let url = format!("{BASE}{}.dat", name.replace(' ', "%20"));
    let resp = client.get(url).send().await.map_err(e)?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(e)?;
    if bytes.len() > MAX_DAT_BYTES {
        return Err("list is too large".into());
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| "list is not text".to_string())
}

/// A list from the cache if it is under 30 days old, otherwise downloaded again. An old copy is
/// still used if the download fails.
async fn get_list(dir: &Path, name: &str) -> Result<String, String> {
    let file = dir.join(format!("{name}.dat"));
    let age = fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|d| d.as_secs());
    if let (Some(a), Ok(text)) = (age, fs::read_to_string(&file)) {
        if a < MAX_AGE_SECS {
            return Ok(text);
        }
    }
    match download(name).await {
        Ok(text) => {
            let _ = fs::create_dir_all(dir);
            let tmp = dir.join(format!("{name}.dat.tmp"));
            if fs::write(&tmp, &text).is_ok() {
                let _ = fs::rename(&tmp, &file);
            }
            Ok(text)
        }
        Err(err) => fs::read_to_string(&file).map_err(|_| err),
    }
}

// ---------- state, commands ----------

#[derive(Clone, Default)]
pub struct VerifyState {
    busy: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

struct BusyGuard(Arc<AtomicBool>);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VerifyProgress {
    done: usize,
    total: usize,
    title: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerifySummary {
    verified: usize,
    unknown: usize,
    unsupported: usize,
    /// Already checked earlier and unchanged.
    skipped: usize,
    /// Games whose console's list could not be downloaded (offline, or GitHub is down).
    no_list: usize,
    cancelled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    /// "verified", "unknown" or "unsupported"
    status: String,
    /// The official name for "verified", or the reason for "unsupported".
    name: String,
}

struct Job {
    id: i64,
    system: String,
    title: String,
    path: PathBuf,
    size: i64,
    mtime: i64,
}

#[tauri::command]
pub fn cancel_verify(state: tauri::State<'_, VerifyState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn get_verification(db: tauri::State<'_, Db>, game_id: i64) -> Option<Verification> {
    let conn = db.0.lock().ok()?;
    conn.query_row(
        "SELECT status, name FROM game_verify WHERE game_id = ?1",
        [game_id],
        |r| {
            Ok(Verification {
                status: r.get(0)?,
                name: r.get(1)?,
            })
        },
    )
    .optional()
    .ok()
    .flatten()
}

/// Checks the given games, or every game of a cartridge console when no ids are given. A game that
/// was already verified and has not changed is skipped, unless it is named in `ids`.
#[tauri::command]
pub async fn verify_games(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    state: tauri::State<'_, VerifyState>,
    ids: Option<Vec<i64>>,
) -> Result<VerifySummary, String> {
    let (db, state) = (db.inner().clone(), state.inner().clone());
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("A check is already running.".into());
    }
    let _guard = BusyGuard(state.busy.clone());
    state.cancel.store(false, Ordering::SeqCst);
    let force = ids.is_some();

    // Which games.
    let jobs: Vec<Job> = {
        let conn = db.0.lock().map_err(e)?;
        let mut stmt = conn
            .prepare("SELECT id, system_id, title, path, size, mtime FROM games WHERE removed = 0 AND missing = 0")
            .map_err(e)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Job {
                    id: r.get(0)?,
                    system: r.get(1)?,
                    title: r.get(2)?,
                    path: PathBuf::from(r.get::<_, String>(3)?),
                    size: r.get(4)?,
                    mtime: r.get(5)?,
                })
            })
            .map_err(e)?;
        let list: Vec<Job> = rows
            .filter_map(Result::ok)
            .filter(|j| !lists_for(&j.system).is_empty())
            .filter(|j| ids.as_ref().map_or(true, |v| v.contains(&j.id)))
            .collect();
        list
    };

    // The lists for those consoles.
    let dir = app.path().app_local_data_dir().map_err(e)?.join("dats");
    let mut lookups: HashMap<String, Arc<Lookup>> = HashMap::new();
    let mut summary = VerifySummary::default();
    let mut systems: Vec<String> = jobs.iter().map(|j| j.system.clone()).collect();
    systems.sort();
    systems.dedup();
    for sys in &systems {
        let mut map = Lookup::new();
        let mut ok = true;
        for name in lists_for(sys) {
            match get_list(&dir, name).await {
                Ok(text) => parse_list(&text, &mut map),
                Err(err) => {
                    applog!("[verify] could not get the list \"{name}\": {err}");
                    ok = false;
                }
            }
        }
        if ok && !map.is_empty() {
            lookups.insert(sys.clone(), Arc::new(map));
        }
    }

    let app2 = app.clone();
    let cancel = state.cancel.clone();
    let result = tokio::task::spawn_blocking(move || run(&app2, &db, &cancel, jobs, &lookups, force, summary))
        .await
        .map_err(e)?;
    summary = result;
    Ok(summary)
}

fn run(
    app: &AppHandle,
    db: &Db,
    cancel: &AtomicBool,
    jobs: Vec<Job>,
    lookups: &HashMap<String, Arc<Lookup>>,
    force: bool,
    mut sum: VerifySummary,
) -> VerifySummary {
    let total = jobs.len();
    for (i, job) in jobs.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            sum.cancelled = true;
            break;
        }
        let _ = app.emit(
            "verify-progress",
            VerifyProgress {
                done: i,
                total,
                title: job.title.clone(),
            },
        );
        let Some(lookup) = lookups.get(&job.system) else {
            sum.no_list += 1;
            continue;
        };
        if !force && already_done(db, job) {
            sum.skipped += 1;
            continue;
        }
        let ext = job
            .path
            .extension()
            .map(|x| x.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let (status, name) = if job.system == "genesis" && ext == "smd" {
            ("unsupported", "Interleaved .smd files can't be checked.".to_string())
        } else {
            match hash_game(&job.path, &job.system, cancel) {
                Ok(Hashed::Cancelled) => {
                    sum.cancelled = true;
                    break;
                }
                Ok(Hashed::Done(found)) => match found.iter().find_map(|k| lookup.get(k)) {
                    Some(n) => ("verified", n.clone()),
                    None => ("unknown", String::new()),
                },
                Err(err) => ("unsupported", err),
            }
        };
        match status {
            "verified" => sum.verified += 1,
            "unknown" => sum.unknown += 1,
            _ => sum.unsupported += 1,
        }
        if let Ok(conn) = db.0.lock() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO game_verify (game_id, status, name, size, mtime, checked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![job.id, status, name, job.size, job.mtime, now() as i64],
            );
        }
    }
    let _ = app.emit(
        "verify-progress",
        VerifyProgress {
            done: total,
            total,
            title: String::new(),
        },
    );
    applog!(
        "[verify] {} verified, {} not in the list, {} unsupported, {} skipped, {} without a list{}",
        sum.verified,
        sum.unknown,
        sum.unsupported,
        sum.skipped,
        sum.no_list,
        if sum.cancelled { " (cancelled)" } else { "" }
    );
    sum
}

fn already_done(db: &Db, job: &Job) -> bool {
    let Ok(conn) = db.0.lock() else { return false };
    conn.query_row(
        "SELECT status, size, mtime FROM game_verify WHERE game_id = ?1",
        [job.id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
    )
    .optional()
    .ok()
    .flatten()
    .is_some_and(|(s, size, mtime)| s == "verified" && size == job.size && mtime == job.mtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crc_of(data: &[u8]) -> u32 {
        let mut c = Crc::new();
        c.update(data);
        c.finish()
    }

    #[test]
    fn crc32_matches_the_standard_test_value() {
        assert_eq!(crc_of(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn reads_a_list() {
        let text = "clrmamepro (\n\tname \"Nintendo - Game Boy\"\n)\n\ngame (\n\tname \"Tetris (World) (Rev 1)\"\n\tdescription \"Tetris (World) (Rev 1)\"\n\trom ( name \"Tetris (World) (Rev 1).gb\" size 32768 crc 46DF91AD md5 abc sha1 def )\n)\n";
        let mut m = Lookup::new();
        parse_list(text, &mut m);
        assert_eq!(
            m.get(&(0x46DF91AD, 32768)).map(String::as_str),
            Some("Tetris (World) (Rev 1)")
        );
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn quoted_words_stay_together() {
        assert_eq!(
            tokens("rom ( name \"A B.gb\" size 5 )"),
            vec!["rom", "(", "name", "A B.gb", "size", "5", ")"]
        );
    }

    #[test]
    fn nes_header_is_skipped_for_the_second_checksum() {
        let mut file = b"NES\x1A".to_vec();
        file.extend_from_slice(&[0u8; 12]);
        let body = b"123456789".to_vec();
        file.extend_from_slice(&body);
        let cancel = AtomicBool::new(false);
        let Hashed::Done(h) = hash_reader(&file[..], file.len() as u64, "nes", &cancel).unwrap() else {
            panic!()
        };
        assert_eq!(h.len(), 2);
        assert_eq!(h[1], (0xCBF4_3926, 9));
    }

    #[test]
    fn snes_copier_header_is_skipped() {
        let mut file = vec![0u8; 512];
        file.extend_from_slice(&vec![7u8; 1024]);
        let cancel = AtomicBool::new(false);
        let Hashed::Done(h) = hash_reader(&file[..], file.len() as u64, "snes", &cancel).unwrap() else {
            panic!()
        };
        assert_eq!(h[1], (crc_of(&vec![7u8; 1024]), 1024));
    }

    #[test]
    fn n64_byte_orders_all_give_the_big_endian_checksum() {
        let z64: Vec<u8> = vec![0x80, 0x37, 0x12, 0x40, 1, 2, 3, 4];
        let v64: Vec<u8> = vec![0x37, 0x80, 0x40, 0x12, 2, 1, 4, 3];
        let n64: Vec<u8> = vec![0x40, 0x12, 0x37, 0x80, 4, 3, 2, 1];
        let cancel = AtomicBool::new(false);
        let h = |d: &Vec<u8>| hash_reader(&d[..], d.len() as u64, "n64", &cancel).unwrap();
        assert_eq!(h(&z64), h(&v64));
        assert_eq!(h(&z64), h(&n64));
    }

    #[test]
    fn every_listed_console_has_lists_and_others_have_none() {
        assert!(!lists_for("nes").is_empty());
        assert!(lists_for("ps1").is_empty());
        assert!(lists_for("arcade").is_empty());
    }
}
