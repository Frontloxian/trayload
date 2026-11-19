import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import GameTile from "./GameTile";
import type { Game, NewsPayload, System, SystemSetup, UpdateInfo } from "./types";

interface Props {
  systems: System[];
  updates: UpdateInfo[];
  onOpenGame: (id: number) => void;
  onGo: (page: "library" | "emulators" | "setup" | "news") => void;
}

const ROW = 6;

export default function Home({ systems, updates, onOpenGame, onGo }: Props) {
  const [games, setGames] = useState<Game[] | null>(null);
  const [running, setRunning] = useState<Set<number>>(new Set());
  const [needs, setNeeds] = useState(0);
  const [news, setNews] = useState<NewsPayload | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    invoke<Game[]>("list_games").then(setGames).catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    load();
    invoke<number[]>("running_games").then((ids) => setRunning(new Set(ids))).catch(() => {});
    invoke<SystemSetup[]>("check_setup")
      .then((rows) => setNeeds(rows.filter((r) => r.state === "needs").length))
      .catch(() => {});
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

  const play = (g: Game) => {
    setError(null);
    invoke("play_game", { id: g.id }).catch((e) => setError(String(e)));
  };
  const favorite = (g: Game) =>
    invoke("set_favorite", { id: g.id, favorite: !g.favorite }).then(load).catch((e) => setError(String(e)));
  const remove = (g: Game) => {
    if (!window.confirm(`Remove "${g.title}" from the library?\n\nThe game file itself is not deleted.`)) return;
    invoke("remove_game", { id: g.id }).then(load).catch((e) => setError(String(e)));
  };

  const row = (title: string, list: Game[], more?: string) =>
    list.length > 0 && (
      <section className="home-row" aria-label={title}>
        <div className="toolbar">
          <h2 className="home-h">{title}</h2>
          {more && (
            <button className="btn" onClick={() => onGo("library")}>
              {more}
            </button>
          )}
        </div>
        <ul className="grid">
          {list.map((g) => (
            <GameTile
              key={g.id}
              game={g}
              systemName={names[g.systemId] ?? g.systemId}
              running={running.has(g.id)}
              onPlay={() => play(g)}
              onOpen={() => onOpenGame(g.id)}
              onFavorite={() => favorite(g)}
              onRemove={() => remove(g)}
            />
          ))}
        </ul>
      </section>
    );

  const all = games ?? [];
  const played = all
    .filter((g) => g.lastPlayed && !g.missing)
    .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))
    .slice(0, ROW);
  const favs = all.filter((g) => g.favorite && !g.missing).slice(0, ROW);
  const added = [...all].sort((a, b) => b.addedAt - a.addedAt).slice(0, ROW);
  const headlines = (news?.items ?? []).filter((i) => i.tier !== "community").slice(0, 5);

  return (
    <>
      {error && <p className="error">{error}</p>}

      {(updates.length > 0 || needs > 0) && (
        <div className="attention">
          {updates.length > 0 && (
            <button className="card attention-card" onClick={() => onGo("emulators")}>
              <strong>
                {updates.length} emulator update{updates.length > 1 ? "s" : ""} ready
              </strong>
              <span className="muted small">{updates.map((u) => `${u.id} ${u.latest}`).join(", ")}</span>
            </button>
          )}
          {needs > 0 && (
            <button className="card attention-card" onClick={() => onGo("setup")}>
              <strong>
                {needs} system{needs > 1 ? "s" : ""} need setup files
              </strong>
              <span className="muted small">Open the Setup checklist to see what's missing.</span>
            </button>
          )}
        </div>
      )}

      {games !== null && all.length === 0 && (
        <div className="section">
          <h2>Welcome to Trayload</h2>
          <p className="muted">
            Drop game files or folders anywhere in this window to add them. To play them you need an emulator for the
            console, which you can install in one click.
          </p>
          <div className="row">
            <button className="btn primary" onClick={() => onGo("emulators")}>
              Install emulators
            </button>
            <button className="btn" onClick={() => onGo("library")}>
              Add games
            </button>
          </div>
        </div>
      )}

      {row("Continue playing", played)}
      {row("Favorites", favs)}
      {row("Recently added", added, "Open library")}

      {headlines.length > 0 && (
        <section className="home-row" aria-label="Latest news">
          <div className="toolbar">
            <h2 className="home-h">Latest news</h2>
            <button className="btn" onClick={() => onGo("news")}>
              All news
            </button>
          </div>
          <ul className="news-list">
            {headlines.map((i) => (
              <li key={i.sourceId + i.link} className="news-item">
                <button
                  className="link news-title"
                  onClick={() => invoke("open_news_link", { url: i.link }).catch((e) => setError(String(e)))}
                >
                  {i.title}
                </button>
                <div className="muted small">{i.source}</div>
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
}
