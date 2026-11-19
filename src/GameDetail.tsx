import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { Collection, Game, GameDetails, Membership, System } from "./types";
import { useCover } from "./useCover";
import { dateTime, duration, hue, initials } from "./util";

interface Props {
  id: number;
  systems: System[];
  onBack: () => void;
}

export default function GameDetail({ id, systems, onBack }: Props) {
  const [game, setGame] = useState<Game | null>(null);
  const [details, setDetails] = useState<GameDetails | null>(null);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [members, setMembers] = useState<Membership[]>([]);
  const [extra, setExtra] = useState("");
  const [notes, setNotes] = useState("");
  const [running, setRunning] = useState(false);
  const [msg, setMsg] = useState("");
  const [error, setError] = useState<string | null>(null);
  const cover = useCover(id, game ? game.artPath : null);

  const load = useCallback(() => {
    const fail = (e: unknown) => setError(String(e));
    invoke<Game[]>("list_games")
      .then((all) => setGame(all.find((g) => g.id === id) ?? null))
      .catch(fail);
    invoke<GameDetails>("get_game_details", { id }).then(setDetails).catch(fail);
    invoke<Collection[]>("list_collections").then(setCollections).catch(fail);
    invoke<Membership[]>("list_collection_members").then(setMembers).catch(fail);
  }, [id]);

  // The two text boxes are filled once, so refreshing never overwrites what you are typing.
  useEffect(() => {
    invoke<GameDetails>("get_game_details", { id }).then((d) => {
      setExtra(d.extraArgs);
      setNotes(d.notes);
    });
    invoke<number[]>("running_games").then((ids) => setRunning(ids.includes(id)));
  }, [id]);

  useEffect(() => {
    load();
    const u1 = listen<{ gameId: number }>("game-started", (e) => e.payload.gameId === id && setRunning(true));
    const u2 = listen<{ gameId: number }>("game-stopped", (e) => {
      if (e.payload.gameId === id) {
        setRunning(false);
        load();
      }
    });
    const u3 = listen("art-updated", load);
    return () => {
      u1.then((f) => f());
      u2.then((f) => f());
      u3.then((f) => f());
    };
  }, [id, load]);

  async function act(fn: () => Promise<unknown>, done = "") {
    setError(null);
    setMsg("");
    try {
      await fn();
      setMsg(done);
    } catch (e) {
      setError(String(e));
    }
    load();
  }

  if (!game) {
    return (
      <>
        <button className="btn" onClick={onBack}>
          ← Library
        </button>
        <p className="muted">This game is no longer in the library.</p>
      </>
    );
  }

  const system = systems.find((s) => s.id === game.systemId);
  const h = hue(game.title);
  const art = cover;

  return (
    <>
      <button className="btn" onClick={onBack}>
        ← Library
      </button>

      <section className="hero">
        <div
          className="hero-bg"
          style={art ? { backgroundImage: `url("${art}")` } : { background: `linear-gradient(135deg, hsl(${h} 55% 30%), hsl(${(h + 40) % 360} 55% 14%))` }}
        />
        <div className="hero-body">
          <div
            className="hero-cover"
            style={{ background: `linear-gradient(135deg, hsl(${h} 55% 34%), hsl(${(h + 40) % 360} 55% 16%))` }}
          >
            {art ? <img src={art} alt="" /> : <span>{initials(game.title)}</span>}
          </div>
          <div className="hero-info">
            <h2>{game.title}</h2>
            <div className="muted">
              {system?.name ?? game.systemId}
              {system && ` · plays in ${system.emulator}`}
            </div>
            <div className="row">
              <button className="btn primary big" disabled={running || game.missing} onClick={() => act(() => invoke("play_game", { id }))}>
                {running ? "Playing…" : "▶ Play"}
              </button>
              <button
                className="btn"
                aria-pressed={game.favorite}
                onClick={() => act(() => invoke("set_favorite", { id, favorite: !game.favorite }))}
              >
                {game.favorite ? "★ Favorite" : "☆ Add to favorites"}
              </button>
            </div>
            {game.missing && <p className="error small">The game file is missing. Check its drive or folder, then Rescan.</p>}
          </div>
        </div>
      </section>

      {error && <p className="error">{error}</p>}
      {msg && (
        <p className="muted" aria-live="polite">
          {msg}
        </p>
      )}

      <div className="detail-grid">
        <section className="section">
          <h2>Collections</h2>
          {collections.length === 0 ? (
            <p className="muted">No collections yet. Create one in the Library tab, under Collections.</p>
          ) : (
            <ul className="checks">
              {collections.map((c) => {
                const on = members.some((m) => m.collectionId === c.id && m.gameId === id);
                return (
                  <li key={c.id}>
                    <label>
                      <input
                        type="checkbox"
                        checked={on}
                        onChange={() =>
                          act(() => invoke("set_game_in_collection", { collectionId: c.id, gameId: id, member: !on }))
                        }
                      />{" "}
                      {c.name}
                    </label>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <section className="section">
          <h2>Launch options</h2>
          <p className="muted small">
            Extra command-line options for this game only, added before the usual ones. Leave empty if you're not sure.
            Wrap values with spaces in "double quotes".
          </p>
          <input
            className="search full"
            aria-label="Extra launch arguments"
            placeholder="Extra arguments"
            value={extra}
            onChange={(e) => setExtra(e.target.value)}
          />
          <label className="field">
            Notes
            <textarea
              className="search full"
              rows={4}
              aria-label="Notes"
              placeholder="Anything you want to remember about this game"
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
            />
          </label>
          <div className="row">
            <button
              className="btn primary"
              onClick={() => act(() => invoke("save_overrides", { id, extraArgs: extra, notes }), "Saved.")}
            >
              Save
            </button>
          </div>
        </section>

        <section className="section">
          <h2>Play history</h2>
          <p>
            <strong>{duration(game.totalPlaytimeSec)}</strong> total
            {details && ` · ${details.sessionCount} session${details.sessionCount === 1 ? "" : "s"}`}
            {game.lastPlayed && ` · last played ${dateTime(game.lastPlayed)}`}
          </p>
          {details && details.sessions.length > 0 ? (
            <ul className="history">
              {details.sessions.map((s) => (
                <li key={s.startedAt}>
                  <span>{dateTime(s.startedAt)}</span>
                  <span className="muted">{duration(s.seconds)}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">Not played yet.</p>
          )}
        </section>

        <section className="section">
          <h2>This game</h2>
          <p className="muted small" title={game.path}>
            {game.path}
          </p>
          <div className="row">
            <button
              className="btn"
              onClick={() =>
                act(async () => {
                  const f = await open({
                    multiple: false,
                    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }],
                  });
                  if (typeof f === "string") await invoke("set_art_from_file", { id, path: f });
                }, "Cover updated.")
              }
            >
              Change cover…
            </button>
            <button
              className="btn danger"
              onClick={() => {
                if (!window.confirm(`Remove "${game.title}" from the library?\n\nThe game file itself is not deleted.`)) return;
                invoke("remove_game", { id }).then(onBack).catch((e) => setError(String(e)));
              }}
            >
              Remove from library
            </button>
          </div>
        </section>
      </div>
    </>
  );
}
