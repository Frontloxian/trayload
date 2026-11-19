import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { Game, ScanSummary, System, Unassigned } from "./types";

function describe(s: ScanSummary): string {
  const parts: string[] = [];
  if (s.added) parts.push(`${s.added} new`);
  if (s.updated) parts.push(`${s.updated} changed`);
  if (s.restored) parts.push(`${s.restored} back`);
  if (s.missing) parts.push(`${s.missing} missing`);
  if (s.unassigned) parts.push(`${s.unassigned} need a system`);
  return parts.length ? parts.join(", ") : "Library up to date";
}

function playtime(sec: number): string {
  if (sec < 60) return "";
  const m = Math.floor(sec / 60);
  return m < 60 ? `${m} min played` : `${Math.floor(m / 60)}h ${m % 60}m played`;
}

function hue(text: string): number {
  let h = 0;
  for (const c of text) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

function initials(title: string): string {
  const words = title.replace(/\(.*?\)|\[.*?\]/g, "").split(/\s+/).filter(Boolean);
  return (words[0]?.[0] ?? "?").toUpperCase() + (words[1]?.[0] ?? "").toUpperCase();
}

export default function Library({ systems }: { systems: System[] }) {
  const [games, setGames] = useState<Game[]>([]);
  const [unassigned, setUnassigned] = useState<Unassigned[]>([]);
  const [summary, setSummary] = useState<string>("");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<string>("all");
  const [busy, setBusy] = useState(false);
  const [review, setReview] = useState(false);
  const [choice, setChoice] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState<Set<number>>(new Set());

  const names = useMemo(() => Object.fromEntries(systems.map((s) => [s.id, s.name])), [systems]);
  const exts = useMemo(() => Array.from(new Set(systems.flatMap((s) => s.exts))).sort(), [systems]);

  const refresh = useCallback(() => {
    invoke<Game[]>("list_games").then(setGames).catch((e) => setError(String(e)));
    invoke<Unassigned[]>("list_unassigned").then(setUnassigned).catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    refresh();
    invoke<ScanSummary | null>("last_scan").then((s) => s && setSummary(describe(s)));
    invoke<number[]>("running_games").then((ids) => setRunning(new Set(ids)));
    const un = listen<ScanSummary>("library-changed", (e) => {
      setSummary(describe(e.payload));
      refresh();
    });
    const u1 = listen<{ gameId: number }>("game-started", (e) =>
      setRunning((r) => new Set(r).add(e.payload.gameId)),
    );
    const u2 = listen<{ gameId: number }>("game-stopped", (e) => {
      setRunning((r) => {
        const n = new Set(r);
        n.delete(e.payload.gameId);
        return n;
      });
      refresh();
    });
    return () => {
      un.then((f) => f());
      u1.then((f) => f());
      u2.then((f) => f());
    };
  }, [refresh]);

  async function run(fn: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      refresh();
    }
  }

  const addFiles = () =>
    run(async () => {
      const picked = await open({ multiple: true, filters: [{ name: "Game files", extensions: exts }] });
      if (picked && picked.length) await invoke("import_paths", { paths: picked });
    });

  const addFolder = () =>
    run(async () => {
      const dir = await open({ directory: true });
      if (typeof dir === "string") await invoke("add_rom_folder", { path: dir });
    });

  const rescan = () => run(() => invoke("rescan"));

  const play = (g: Game) => {
    setError(null);
    invoke("play_game", { id: g.id }).catch((e) => setError(String(e)));
  };

  const removeGame = (g: Game) => {
    if (!window.confirm(`Remove "${g.title}" from the library?\n\nThe game file itself is not deleted.`)) return;
    run(() => invoke("remove_game", { id: g.id }));
  };

  const counts = useMemo(() => {
    const m: Record<string, number> = {};
    for (const g of games) m[g.systemId] = (m[g.systemId] ?? 0) + 1;
    return m;
  }, [games]);

  const shown = games.filter(
    (g) =>
      (filter === "all" || g.systemId === filter) && g.title.toLowerCase().includes(query.trim().toLowerCase()),
  );

  return (
    <>
      <div className="toolbar">
        <input
          className="search"
          type="search"
          placeholder="Search your library"
          aria-label="Search your library"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button className="btn" disabled={busy} onClick={rescan}>
          Rescan
        </button>
        <button className="btn" disabled={busy} onClick={addFolder}>
          Add folder…
        </button>
        <button className="btn primary" disabled={busy} onClick={addFiles}>
          Add games…
        </button>
      </div>
      <p className="muted small" aria-live="polite">
        {busy ? "Working…" : summary}
      </p>
      {error && <p className="error">{error}</p>}

      {unassigned.length > 0 && (
        <div className="banner">
          <span>
            {unassigned.length} file{unassigned.length === 1 ? "" : "s"} need a system.
          </span>
          <button className="btn" onClick={() => setReview(!review)}>
            {review ? "Hide" : "Review"}
          </button>
        </div>
      )}
      {review && (
        <ul className="cards">
          {unassigned.map((u) => {
            const options = u.candidates.length ? u.candidates : systems.map((s) => s.id);
            const value = choice[u.path] ?? options[0];
            return (
              <li key={u.path} className="card">
                <div className="card-main">
                  <div>
                    <strong>{u.fileName}</strong>
                    <div className="muted small">{u.path}</div>
                  </div>
                  <select
                    className="select"
                    value={value}
                    aria-label={`System for ${u.fileName}`}
                    onChange={(e) => setChoice({ ...choice, [u.path]: e.target.value })}
                  >
                    {options.map((id) => (
                      <option key={id} value={id}>
                        {names[id] ?? id}
                      </option>
                    ))}
                  </select>
                  <div className="actions">
                    <button
                      className="btn primary"
                      onClick={() => run(() => invoke("assign_system", { path: u.path, systemId: value }))}
                    >
                      Add
                    </button>
                    <button className="btn" onClick={() => run(() => invoke("dismiss_unassigned", { path: u.path }))}>
                      Ignore
                    </button>
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      )}

      {games.length === 0 ? (
        <div className="empty">
          <h2>Your library is empty</h2>
          <p className="muted">
            Drag game files or folders anywhere onto this window, or use Add games / Add folder above. Trayload only
            remembers where your files are. It never copies or changes them.
          </p>
        </div>
      ) : (
        <div className="lib">
          <nav className="systems" aria-label="Systems">
            <button className="chip" aria-pressed={filter === "all"} onClick={() => setFilter("all")}>
              All games <span>{games.length}</span>
            </button>
            {Object.keys(counts)
              .sort((a, b) => (names[a] ?? a).localeCompare(names[b] ?? b))
              .map((id) => (
                <button key={id} className="chip" aria-pressed={filter === id} onClick={() => setFilter(id)}>
                  {names[id] ?? id} <span>{counts[id]}</span>
                </button>
              ))}
          </nav>
          <ul className="grid">
            {shown.map((g) => {
              const h = hue(g.title);
              return (
                <li key={g.id} className="tile">
                  <button
                    className="cover-btn"
                    onClick={() => play(g)}
                    disabled={running.has(g.id)}
                    aria-label={`Play ${g.title}`}
                  >
                    <div
                      className="cover"
                      style={{ background: `linear-gradient(135deg, hsl(${h} 55% 34%), hsl(${(h + 40) % 360} 55% 16%))` }}
                    >
                      <span>{initials(g.title)}</span>
                      {g.missing && <em className="badge">Missing</em>}
                      {running.has(g.id) ? <em className="badge live">Playing</em> : <i className="play">▶ Play</i>}
                    </div>
                  </button>
                  <div className="tile-title" title={g.path}>
                    {g.title}
                  </div>
                  <div className="muted small">
                    {names[g.systemId] ?? g.systemId}
                    {playtime(g.totalPlaytimeSec) && ` · ${playtime(g.totalPlaytimeSec)}`}
                  </div>
                  <button className="tile-remove" onClick={() => removeGame(g)} aria-label={`Remove ${g.title} from library`}>
                    Remove
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </>
  );
}
