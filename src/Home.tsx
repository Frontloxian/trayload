import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import GameTile from "./GameTile";
import { useCover } from "./useCover";
import { hue, initials } from "./util";
import type { EmulatorEntry, Game, NewsPayload, System, SystemSetup, UpdateInfo } from "./types";

interface Props {
  systems: System[];
  updates: UpdateInfo[];
  onOpenGame: (id: number) => void;
  onGo: (page: "library" | "emulators" | "setup" | "news" | "settings") => void;
  onUpdated: () => void;
}

const ROW = 6;
const DISMISS_KEY = "trayload.gettingStartedDismissed";

function ago(sec: number | null): string {
  if (!sec) return "Not played yet";
  const days = Math.floor((Date.now() / 1000 - sec) / 86400);
  if (days <= 0) return "Played today";
  if (days === 1) return "Played yesterday";
  return `Played ${days} days ago`;
}

function PlayCard({ g, system, running, onPlay, onOpen }: { g: Game; system: string; running: boolean; onPlay: () => void; onOpen: () => void }) {
  const cover = useCover(g.id, g.artPath);
  const h = hue(g.title);
  return (
    <li className="play-card">
      <button className="play-thumb" onClick={onPlay} disabled={running} aria-label={`Play ${g.title}`}
        style={{ background: `linear-gradient(135deg, hsl(${h} 55% 34%), hsl(${(h + 40) % 360} 55% 16%))` }}>
        {cover ? <img src={cover} alt="" /> : <span>{initials(g.title)}</span>}
      </button>
      <div className="play-info">
        <button className="link news-title" onClick={onOpen}>{g.title}</button>
        <div className="muted small">{system}, {ago(g.lastPlayed)}</div>
        <button className="btn primary" onClick={onPlay} disabled={running}>{running ? "Playing" : "Play"}</button>
      </div>
    </li>
  );
}

export default function Home({ systems, updates, onOpenGame, onGo, onUpdated }: Props) {
  const [games, setGames] = useState<Game[] | null>(null);
  const [running, setRunning] = useState<Set<number>>(new Set());
  const [needs, setNeeds] = useState(0);
  const [installed, setInstalled] = useState(0);
  const [news, setNews] = useState<NewsPayload | null>(null);
  const [folders, setFolders] = useState(0);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(() => {
    try {
      return localStorage.getItem(DISMISS_KEY) === "1";
    } catch {
      return false;
    }
  });

  const load = useCallback(() => {
    invoke<Game[]>("list_games").then(setGames).catch((e) => setError(String(e)));
    invoke<string[]>("get_rom_folders").then((f) => setFolders(f.length)).catch(() => {});
  }, []);

  useEffect(() => {
    load();
    invoke<number[]>("running_games").then((ids) => setRunning(new Set(ids))).catch(() => {});
    invoke<SystemSetup[]>("check_setup").then((rows) => setNeeds(rows.filter((r) => r.state === "needs").length)).catch(() => {});
    invoke<EmulatorEntry[]>("list_emulators").then((l) => setInstalled(l.filter((e) => e.installed).length)).catch(() => {});
    invoke<NewsPayload>("get_news", { force: false }).then(setNews).catch(() => {});
    const u = [
      listen<{ gameId: number }>("game-started", (e) => setRunning((s) => new Set(s).add(e.payload.gameId))),
      listen<{ gameId: number }>("game-stopped", (e) => {
        setRunning((s) => {
          const n = new Set(s);
          n.delete(e.payload.gameId);
          return n;
        });
        load();
      }),
      listen("library-changed", load),
    ];
    return () => u.forEach((p) => p.then((f) => f()));
  }, [load]);

  const names: Record<string, string> = {};
  systems.forEach((s) => (names[s.id] = s.name));

  const run = async (label: string, fn: () => Promise<unknown>) => {
    setBusy(label);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  const play = (g: Game) => {
    setError(null);
    invoke("play_game", { id: g.id }).catch((e) => setError(String(e)));
  };
  const favorite = (g: Game) => invoke("set_favorite", { id: g.id, favorite: !g.favorite }).then(load).catch((e) => setError(String(e)));
  const remove = (g: Game) => {
    if (!window.confirm(`Remove "${g.title}" from the library?\n\nThe game file itself is not deleted.`)) return;
    invoke("remove_game", { id: g.id }).then(load).catch((e) => setError(String(e)));
  };
  const addGames = () =>
    run("add", async () => {
      const picked = await open({ multiple: true });
      if (picked && picked.length) await invoke("import_paths", { paths: picked });
    });
  const addFolder = () =>
    run("folder", async () => {
      const dir = await open({ directory: true });
      if (typeof dir === "string") await invoke("add_rom_folder", { path: dir });
    });
  const updateOne = (u: UpdateInfo) =>
    run(u.id, async () => {
      await invoke("install_emulator", { id: u.id });
      onUpdated();
    });

  const tile = (g: Game) => (
    <GameTile key={g.id} game={g} systemName={names[g.systemId] ?? g.systemId} running={running.has(g.id)}
      onPlay={() => play(g)} onOpen={() => onOpenGame(g.id)} onFavorite={() => favorite(g)} onRemove={() => remove(g)} />
  );

  const all = games ?? [];
  const played = all.filter((g) => g.lastPlayed && !g.missing).sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)).slice(0, 3);
  const favs = all.filter((g) => g.favorite && !g.missing).slice(0, ROW);
  const added = [...all].sort((a, b) => b.addedAt - a.addedAt).slice(0, ROW);
  const headlines = (news?.items ?? []).filter((i) => i.tier !== "community").slice(0, 4);

  const steps = [
    { done: all.length > 0 || folders > 0, label: "Add your games", hint: "Drop files anywhere, or pick a folder to scan.", action: "Add a folder", run: addFolder },
    { done: installed > 0, label: "Install an emulator", hint: "One click each, from official sources.", action: "Open Emulators", run: () => onGo("emulators") },
    { done: installed > 0 && needs === 0, label: "Check setup files", hint: "Some consoles need files from your own console.", action: "Open checklist", run: () => onGo("setup") },
  ];
  const showStart = !dismissed && steps.some((s) => !s.done);

  return (
    <>
      <div className="toolbar home-actions">
        <span />
        <div className="row">
          <button className="btn" disabled={busy !== null} onClick={() => run("rescan", () => invoke("rescan"))}>
            {busy === "rescan" ? "Scanning…" : "Rescan library"}
          </button>
          <button className="btn primary" disabled={busy !== null} onClick={addGames}>Add games</button>
        </div>
      </div>
      {error && <p className="error">{error}</p>}

      <div className="home-grid">
        <div className="home-main">
          {games !== null && all.length === 0 && (
            <div className="section">
              <h2>Welcome to Trayload</h2>
              <p className="muted">Drop game files or folders anywhere in this window to add them. To play them you need an emulator for the console.</p>
            </div>
          )}

          {played.length > 0 && (
            <section className="home-row" aria-label="Continue playing">
              <h2 className="home-h">Continue playing</h2>
              <ul className="play-cards">
                {played.map((g) => (
                  <PlayCard key={g.id} g={g} system={names[g.systemId] ?? g.systemId} running={running.has(g.id)} onPlay={() => play(g)} onOpen={() => onOpenGame(g.id)} />
                ))}
              </ul>
            </section>
          )}

          {added.length > 0 && (
            <section className="home-row" aria-label="Recently added">
              <div className="toolbar"><h2 className="home-h">Recently added</h2><button className="btn" onClick={() => onGo("library")}>See all</button></div>
              <ul className="grid">{added.map(tile)}</ul>
            </section>
          )}

          {favs.length > 0 && (
            <section className="home-row" aria-label="Favorites">
              <div className="toolbar"><h2 className="home-h">Favorites</h2><button className="btn" onClick={() => onGo("library")}>See all</button></div>
              <ul className="grid">{favs.map(tile)}</ul>
            </section>
          )}
        </div>

        <aside className="home-side" aria-label="Status">
          {showStart && (
            <section className="card side-card">
              <div className="toolbar"><h2 className="home-h">Getting started</h2>
                <button className="link small" onClick={() => { setDismissed(true); try { localStorage.setItem(DISMISS_KEY, "1"); } catch { /* optional */ } }}>Hide</button>
              </div>
              <ol className="steps">
                {steps.map((s) => (
                  <li key={s.label} className={s.done ? "done" : ""}>
                    <strong>{s.done ? "✓ " : ""}{s.label}</strong>
                    <div className="muted small">{s.hint}</div>
                    {!s.done && <button className="btn" disabled={busy !== null} onClick={s.run}>{s.action}</button>}
                  </li>
                ))}
              </ol>
            </section>
          )}

          {updates.length > 0 && (
            <section className="card side-card" aria-label="Updates ready">
              <h2 className="home-h">Updates ready <span className="badge">{updates.length}</span></h2>
              <ul className="side-list">
                {updates.map((u) => (
                  <li key={u.id}>
                    <div><strong>{u.id}</strong><div className="muted small">{u.installed} → {u.latest}</div></div>
                    <button className="btn" disabled={busy !== null} onClick={() => updateOne(u)}>{busy === u.id ? "Updating…" : "Update"}</button>
                  </li>
                ))}
              </ul>
              <button className="link small" onClick={() => onGo("emulators")}>Manage emulators</button>
            </section>
          )}

          {needs > 0 && (
            <button className="card side-card attention-card" onClick={() => onGo("setup")}>
              <strong>{needs} system{needs > 1 ? "s" : ""} need setup files</strong>
              <span className="muted small">Open the Setup checklist to see what's missing.</span>
            </button>
          )}

          {headlines.length > 0 && (
            <section className="card side-card" aria-label="Latest news">
              <div className="toolbar"><h2 className="home-h">Latest news</h2><button className="link small" onClick={() => onGo("news")}>All news</button></div>
              <ul className="side-list col">
                {headlines.map((i) => (
                  <li key={i.sourceId + i.link}>
                    <button className="link" onClick={() => invoke("open_news_link", { url: i.link }).catch((e) => setError(String(e)))}>{i.title}</button>
                    <div className="muted small">{i.source}</div>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </aside>
      </div>
    </>
  );
}
