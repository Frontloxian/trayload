import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { NewsItem, NewsPayload, UpdateInfo } from "./types";

type Filter = "all" | "release" | "official" | "community";

const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "release", label: "Releases" },
  { id: "official", label: "Official blogs" },
  { id: "community", label: "Community" },
];

const TIER_LABEL: Record<string, string> = {
  release: "Release",
  official: "Official",
  community: "Community, unverified",
};

function when(date: string): string {
  if (!date) return "";
  const t = Date.parse(date.endsWith("Z") || /[+-]\d\d:?\d\d$/.test(date) ? date : date + "Z");
  if (Number.isNaN(t)) return date.slice(0, 10);
  const days = Math.floor((Date.now() - t) / 86400000);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 30) return `${days} days ago`;
  return new Date(t).toLocaleDateString();
}

export default function News({ updates, onEmulators }: { updates: UpdateInfo[]; onEmulators: () => void }) {
  const [data, setData] = useState<NewsPayload | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");

  const load = useCallback((force: boolean) => {
    setLoading(true);
    setError(null);
    invoke<NewsPayload>("get_news", { force })
      .then(setData)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => load(false), [load]);

  const open = (item: NewsItem) => {
    invoke("open_news_link", { url: item.link }).catch((e) => setError(String(e)));
  };

  const items = (data?.items ?? []).filter((i) => filter === "all" || i.tier === filter);
  const failed = (data?.sources ?? []).filter((s) => !s.ok);

  return (
    <>
      <p className="muted small">
        News is for reading only. Trayload never downloads or installs anything from a news item. Emulators come only
        from their official sources on the Emulators tab.
      </p>

      {updates.length > 0 && (
        <div className="card update-card">
          <strong>
            {updates.length} emulator update{updates.length > 1 ? "s" : ""} ready
          </strong>
          <div className="muted small">{updates.map((u) => `${u.id} ${u.latest}`).join(", ")}</div>
          <button className="btn" onClick={onEmulators}>
            Open Emulators
          </button>
        </div>
      )}

      {data?.watch.map((w) => {
        const hits = data.items.filter(
          (i) => i.tier !== "release" && w.keywords.some((k) => i.title.toLowerCase().includes(k)),
        );
        return (
          <div key={w.id} className="card watch-card">
            <div className="row">
              <strong>{w.title}</strong> <span className="pill amber">Watching</span>
            </div>
            <p className="muted small">{w.note}</p>
            {hits.length > 0 ? (
              <ul className="news-list compact">
                {hits.slice(0, 5).map((i) => (
                  <li key={i.link}>
                    <button className="link" onClick={() => open(i)}>
                      {i.title}
                    </button>
                    <span className="muted small"> {i.source}, {when(i.date)}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="muted small">Nothing in the latest posts.</p>
            )}
          </div>
        );
      })}

      <div className="toolbar">
        <div className="chips" role="group" aria-label="Filter news">
          {FILTERS.map((f) => (
            <button key={f.id} className="chip" aria-pressed={filter === f.id} onClick={() => setFilter(f.id)}>
              {f.label}
            </button>
          ))}
        </div>
        <button className="btn" disabled={loading} onClick={() => load(true)}>
          {loading ? "Refreshing…" : "Refresh"}
        </button>
      </div>

      {error && <p className="error">{error}</p>}
      {data && data.fetchedAt > 0 && (
        <p className="muted small">Updated {new Date(data.fetchedAt * 1000).toLocaleTimeString()}</p>
      )}
      {failed.length > 0 && (
        <details className="muted small">
          <summary>{failed.length} source{failed.length > 1 ? "s" : ""} could not refresh</summary>
          <ul>
            {failed.map((s) => (
              <li key={s.id}>
                {s.name}: {s.error}
              </li>
            ))}
          </ul>
        </details>
      )}

      {!data && loading && <p className="muted">Loading news…</p>}
      {data && items.length === 0 && !loading && <p className="muted">Nothing to show here yet.</p>}

      <ul className="news-list">
        {items.map((i) => (
          <li key={i.sourceId + i.link} className={`news-item ${i.tier}`}>
            <button className="link news-title" onClick={() => open(i)}>
              {i.title}
            </button>
            <div className="muted small">
              <span className={`pill ${i.tier === "community" ? "amber" : i.tier === "release" ? "green" : "blue"}`}>
                {TIER_LABEL[i.tier]}
              </span>{" "}
              {i.source}, {when(i.date)}
            </div>
          </li>
        ))}
      </ul>
    </>
  );
}
