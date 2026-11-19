//! News tab: release feeds, official blogs and community posts, plus "update ready" checks.
//! Reading only. Nothing from a news item is ever downloaded or installed by Trayload.

use crate::registry::{self, Source};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::Duration};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

const EMBEDDED: &str = include_str!("../resources/news.json");

/// Hosts news is read from, and the only hosts "Open" will send you to. Fixed in code on purpose:
/// a downloaded feed list can add feeds on these hosts but can never widen the list.
const NEWS_HOSTS: &[&str] = &[
    "github.com",
    "git.eden-emu.dev",
    "dolphin-emu.org",
    "www.reddit.com",
    "old.reddit.com",
];

const PER_FEED: usize = 12;
const MAX_BYTES: usize = 2_000_000;

#[derive(Debug, Clone, Deserialize)]
struct FeedDef {
    id: String,
    name: String,
    /// "official" or "community"
    tier: String,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchDef {
    pub id: String,
    pub title: String,
    pub note: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct NewsConfig {
    feeds: Vec<FeedDef>,
    #[serde(default)]
    watch: Vec<WatchDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub source_id: String,
    pub source: String,
    /// "release", "official" or "community"
    pub tier: String,
    pub title: String,
    pub link: String,
    /// ISO 8601 UTC-ish text, empty if the feed gave none.
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NewsPayload {
    pub fetched_at: u64,
    pub items: Vec<NewsItem>,
    pub sources: Vec<SourceStatus>,
    pub watch: Vec<WatchDef>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

fn load_config(app: &AppHandle) -> Result<NewsConfig, String> {
    if let Ok(d) = data_dir(app) {
        if let Ok(text) = fs::read_to_string(d.join("news.json")) {
            match serde_json::from_str::<NewsConfig>(&text) {
                Ok(c) => return Ok(c),
                Err(e) => crate::applog!("[news] ignoring bad news.json: {e}"),
            }
        }
    }
    serde_json::from_str(EMBEDDED).map_err(|e| format!("built-in news list is broken: {e}"))
}

fn host_ok(url: &str) -> bool {
    match reqwest::Url::parse(url) {
        Ok(u) => u.scheme() == "https" && u.host_str().map(|h| NEWS_HOSTS.contains(&h)).unwrap_or(false),
        Err(_) => false,
    }
}

fn client() -> Result<reqwest::Client, String> {
    let policy = reqwest::redirect::Policy::custom(|a| {
        if a.previous().len() >= 5 {
            return a.error("too many redirects");
        }
        let ok = a.url().scheme() == "https"
            && a.url().host_str().map(|h| NEWS_HOSTS.contains(&h)).unwrap_or(false);
        if ok {
            a.follow()
        } else {
            let h = a.url().host_str().unwrap_or("?").to_string();
            a.error(format!("redirected to a host that is not allowed: {h}"))
        }
    });
    reqwest::Client::builder()
        .user_agent("Trayload/0.1 (news reader)")
        .redirect(policy)
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

// ---------- feed parsing (RSS and Atom, no extra crates) ----------

fn unescape(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix("<![CDATA[")
        .and_then(|x| x.strip_suffix("]]>"))
        .unwrap_or(s);
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tag(block: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r"(?s)<{n}(?:\s[^>]*)?>(.*?)</{n}>", n = regex::escape(name))).ok()?;
    re.captures(block).map(|c| unescape(&c[1])).filter(|s| !s.is_empty())
}

/// "Tue, 10 Jun 2025 12:00:00 +0000" -> "2025-06-10T12:00:00". Time zone is ignored (fine for sorting).
fn rfc2822_to_iso(s: &str) -> Option<String> {
    let re = Regex::new(r"(\d{1,2})\s+([A-Za-z]{3})[a-z]*\s+(\d{4})(?:\s+(\d{2}):(\d{2})(?::(\d{2}))?)?").ok()?;
    let c = re.captures(s)?;
    let months = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let m = months.iter().position(|x| x.eq_ignore_ascii_case(&c[2]))? + 1;
    let g = |i: usize| c.get(i).map(|x| x.as_str().to_string()).unwrap_or_else(|| "00".into());
    Some(format!("{}-{:02}-{:02}T{}:{}:{}", &c[3], m, c[1].parse::<u32>().ok()?, g(4), g(5), g(6)))
}

pub fn parse_feed(xml: &str) -> Vec<(String, String, String)> {
    let mut out = vec![];
    let atom = xml.contains("<entry");
    let block_re = if atom {
        Regex::new(r"(?s)<entry[\s>].*?</entry>").unwrap()
    } else {
        Regex::new(r"(?s)<item[\s>].*?</item>").unwrap()
    };
    let href_re = Regex::new(r#"<link\b[^>]*?\bhref\s*=\s*"([^"]+)"[^>]*>"#).unwrap();
    for m in block_re.find_iter(xml) {
        let b = m.as_str();
        let Some(title) = tag(b, "title") else { continue };
        let link = if atom {
            // Prefer rel="alternate" (or no rel at all).
            href_re
                .captures_iter(b)
                .map(|c| (c[0].to_string(), c[1].to_string()))
                .find(|(whole, _)| !whole.contains("rel=\"") || whole.contains("rel=\"alternate\""))
                .map(|(_, h)| h)
        } else {
            tag(b, "link")
        };
        let Some(link) = link.map(|l| unescape(&l)) else { continue };
        let date = if atom {
            tag(b, "updated").or_else(|| tag(b, "published")).unwrap_or_default()
        } else {
            tag(b, "pubDate")
                .and_then(|d| rfc2822_to_iso(&d))
                .or_else(|| tag(b, "dc:date"))
                .unwrap_or_default()
        };
        out.push((title, link, date));
    }
    out
}

// ---------- fetching ----------

struct Source1 {
    id: String,
    name: String,
    tier: String,
    url: String,
}

/// Release feeds come from the emulator registry itself: every emulator downloaded from GitHub
/// or a Forgejo server also gets its release feed in the News tab.
fn release_sources(app: &AppHandle) -> Vec<Source1> {
    let mut v: Vec<Source1> = vec![];
    let Ok(reg) = registry::load(app) else { return v };
    for s in &reg.systems {
        if s.tier.as_deref() == Some("experimental") {
            continue;
        }
        let url = match &s.source {
            Source::Github { repo, .. } => format!("https://github.com/{repo}/releases.atom"),
            Source::Forgejo { host, repo, .. } => format!("https://{host}/{repo}/releases.atom"),
            _ => continue,
        };
        if v.iter().any(|x| x.url == url) {
            continue;
        }
        v.push(Source1 {
            id: format!("rel-{}", crate::installer::slug(&s.emulator)),
            name: format!("{} releases", s.emulator),
            tier: "release".into(),
            url,
        });
    }
    v
}

async fn fetch_one(c: &reqwest::Client, s: &Source1) -> Result<Vec<NewsItem>, String> {
    if !host_ok(&s.url) {
        return Err("host is not on the news allowlist".into());
    }
    let resp = c
        .get(&s.url)
        .header("Accept", "application/atom+xml, application/rss+xml, application/xml, text/xml")
        .send()
        .await
        .map_err(|e| format!("could not connect: {e}"))?
        .error_for_status()
        .map_err(|e| format!("{e}"))?;
    let bytes = resp.bytes().await.map_err(|e| format!("download failed: {e}"))?;
    if bytes.len() > MAX_BYTES {
        return Err("feed is unexpectedly large".into());
    }
    let xml = String::from_utf8_lossy(&bytes).to_string();
    let entries = parse_feed(&xml);
    if entries.is_empty() {
        return Err("no posts found in this feed (it may have moved)".into());
    }
    Ok(entries
        .into_iter()
        .filter(|(_, link, _)| host_ok(link))
        .take(PER_FEED)
        .map(|(title, link, date)| NewsItem {
            source_id: s.id.clone(),
            source: s.name.clone(),
            tier: s.tier.clone(),
            title,
            link,
            date,
        })
        .collect())
}

fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("news-cache.json"))
}

fn read_cache(app: &AppHandle) -> Option<NewsPayload> {
    let text = fs::read_to_string(cache_path(app).ok()?).ok()?;
    serde_json::from_str(&text).ok()
}

/// Cached news (instant, works offline) unless `force` or the cache is older than 30 minutes.
#[tauri::command]
pub async fn get_news(app: AppHandle, force: bool) -> Result<NewsPayload, String> {
    let cfg = load_config(&app)?;
    if let Some(c) = read_cache(&app) {
        if !force && now().saturating_sub(c.fetched_at) < 1800 {
            return Ok(c);
        }
    }
    let mut sources = release_sources(&app);
    for f in &cfg.feeds {
        sources.push(Source1 {
            id: f.id.clone(),
            name: f.name.clone(),
            tier: if f.tier == "community" { "community".into() } else { "official".into() },
            url: f.url.clone(),
        });
    }
    let c = client()?;
    let results = futures_util::future::join_all(sources.iter().map(|s| fetch_one(&c, s))).await;
    let old = read_cache(&app);
    let mut items: Vec<NewsItem> = vec![];
    let mut statuses = vec![];
    for (s, r) in sources.iter().zip(results) {
        match r {
            Ok(list) => {
                items.extend(list);
                statuses.push(SourceStatus { id: s.id.clone(), name: s.name.clone(), tier: s.tier.clone(), ok: true, error: None });
            }
            Err(e) => {
                // Keep what we showed last time for this source, and say it couldn't refresh.
                if let Some(o) = &old {
                    items.extend(o.items.iter().filter(|i| i.source_id == s.id).cloned());
                }
                statuses.push(SourceStatus { id: s.id.clone(), name: s.name.clone(), tier: s.tier.clone(), ok: false, error: Some(e) });
            }
        }
    }
    items.sort_by(|a, b| b.date.cmp(&a.date));
    let payload = NewsPayload { fetched_at: now(), items, sources: statuses, watch: cfg.watch };
    if let Ok(p) = cache_path(&app) {
        if let Ok(text) = serde_json::to_string(&payload) {
            let _ = fs::write(p, text);
        }
    }
    Ok(payload)
}

/// Opens a news link in the normal browser, only for the hosts news is read from.
#[tauri::command]
pub fn open_news_link(app: AppHandle, url: String) -> Result<(), String> {
    if !host_ok(&url) {
        return Err("that link is not on a site Trayload opens from the News tab".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_config_parses() {
        let c: NewsConfig = serde_json::from_str(EMBEDDED).unwrap();
        assert!(!c.feeds.is_empty());
        for f in &c.feeds {
            assert!(host_ok(&f.url), "feed host not allowed: {}", f.url);
            assert!(f.tier == "official" || f.tier == "community");
        }
    }

    #[test]
    fn atom_github() {
        let xml = r#"<feed><entry><id>x</id><updated>2026-01-02T03:04:05Z</updated>
          <link rel="alternate" type="text/html" href="https://github.com/a/b/releases/tag/v1"/>
          <title>v1 &amp; more</title></entry></feed>"#;
        let e = parse_feed(xml);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].0, "v1 & more");
        assert_eq!(e[0].1, "https://github.com/a/b/releases/tag/v1");
        assert_eq!(e[0].2, "2026-01-02T03:04:05Z");
    }

    #[test]
    fn rss_item() {
        let xml = "<rss><channel><item><title><![CDATA[Hello]]></title><link>https://dolphin-emu.org/blog/x/</link><pubDate>Tue, 10 Jun 2025 12:30:00 +0000</pubDate></item></channel></rss>";
        let e = parse_feed(xml);
        assert_eq!(e[0].0, "Hello");
        assert_eq!(e[0].2, "2025-06-10T12:30:00");
    }

    #[test]
    fn only_allowed_links() {
        assert!(host_ok("https://github.com/x"));
        assert!(!host_ok("http://github.com/x"));
        assert!(!host_ok("https://evil.example/x"));
    }
}
