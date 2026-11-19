import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import GameTile from "./GameTile";
import type { Collection, Game, Membership, ScanSummary, System, Unassigned } from "./types";

type SortKey = "title" | "added" | "played" | "time";

const SORTS: { id: SortKey; label: string }[] = [
  { id: "title", label: "Title A to Z" },
  { id: "added", label: "Recently added" },
  { id: "played", label: "Last played" },
  { id: "time", label: "Most played" },
];

const COMPARE: Record<SortKey, (a: Game, b: Game) => number> = {
  title: (a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: "base" }),
  added: (a, b) => b.addedAt - a.addedAt,
  played: (a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0),
  time: (a, b) => b.totalPlaytimeSec - a.totalPlaytimeSec,
};

function describe(s: ScanSummary): string {
  const parts: string[] = [];
  if (s.added) parts.push(`${s.added} new`);
  if (s.updated) parts.push(`${s.updated} changed`);
  if (s.restored) parts.push(`${s.restored} back`);
  if (s.missing) parts.push(`${s.missing} missing`);
  if (s.unassigned) parts.push(`${s.unassigned} need a system`);
  return parts.length ? parts.join(", ") : "Library up to date";
}

interface Props {
  systems: System[];
  refreshKey: number;
  onOpen: (id: number) => void;
}

export default function Library({ systems, refreshKey, onOpen }: Props) {
  const [games, setGames] = useState<Game[]>([]);
  const [unassigned, setUnassigned] = useState<Unassigned[]>([]);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [members, setMembers] = useState<Membership[]>([]);
  const [summary, setSummary] = useState("");
  const [query, setQuery] = useState("");
  const [view, setView] = useState("all");
  const [sort, setSort] = useState<SortKey>("title");
  const [busy, setBusy] = useState(false);
  const [review, setReview] = useState(false);
  const [choice, setChoice] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState<Set<number>>(new Set());
  const [newName, setNewName] = useState("");
  const [renaming, setRenaming] = useState<string | null>(null);

  const names = useMemo(() => Object.fromEntries(systems.map((s) => [s.id, s.name])), [systems]);
  const exts = useMemo(() => Array.from(new Set(systems.flatMap((s) => s.exts))).sort(), [systems]);

  const refresh = useCallback(() => {
    const fail = (e: unknown) => setError(String(e));
    invoke<Game[]>("list_games").then(setGames).catch(fail);
    invoke<Unassigned[]>("list_unassigned").then(setUnassigned).catch(fail);
    invoke<Collection[]>("list_collections").then(setCollections).catch(fail);
    invoke<Membership[]>("list_collection_members").then(setMembers).catch(fail);
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
    let t: number | undefined;
    const u3 = listen("art-updated", () => {
      window.clearTimeout(t);
      t = window.setTimeout(refresh, 800);
    });
    return () => {
      window.clearTimeout(t);
      un.then((f) => f());
      u1.then((f) => f());
      u2.then((f) => f());
      u3.then((f) => f());
    };
  }, [refresh]);

  // Coming back from a game's page: pick up favorites, collections and playtime it may have changed.
  useEffect(() => {
    if (refreshKey > 0) refresh();
  }, [refreshKey, refresh]);

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
    if (!window.confirm(`Remove "${g.title}" from the library?\n\nThe game file itself is not deleted. You can bring it back in Settings.`)) return;
    run(() => invoke("remove_game", { id: g.id }));
  };

  const toggleFavorite = (g: Game) =>
    run(() => invoke("set_favorite", { id: g.id, favorite: !g.favorite }));

  // ----- collections -----
  const selectedCollection = collections.find((c) => view === `c:${c.id}`);

  const createCollection = () =>
    run(async () => {
      const id = await invoke<number>("create_collection", { name: newName });
      setNewName("");
      setView(`c:${id}`);
    });

  const renameCollection = () =>
    run(async () => {
      if (!selectedCollection || renaming === null) return;
      await invoke("rename_collection", { id: selectedCollection.id, name: renaming });
      setRenaming(null);
    });

  const deleteCollection = () => {
    if (!selectedCollection) return;
    if (!window.confirm(`Delete the collection "${selectedCollection.name}"?\n\nThe games in it stay in your library.`)) return;
    run(async () => {
      await invoke("delete_collection", { id: selectedCollection.id });
      setView("all");
    });
  };

  // ----- views -----
  const memberSets = useMemo(() => {
    const m = new Map<number, Set<number>>();
    for (const x of members) {
      if (!m.has(x.collectionId)) m.set(x.collectionId, new Set());
      m.get(x.collectionId)!.add(x.gameId);
    }
    return m;
  }, [members]);

  const sysCounts = useMemo(() => {
    const m: Record<string, number> = {};
    for (const g of games) m[g.systemId] = (m[g.systemId] ?? 0) + 1;
    return m;
  }, [games]);

  const favCount = games.filter((g) => g.favorite).length;
  const fixedOrder = view === "recent" || view === "most";

  const shown = useMemo(() => {
    let list = games;
    if (view === "fav") list = list.filter((g) => g.favorite);
    else if (view === "recent") list = list.filter((g) => g.lastPlayed !== null);
    else if (view === "most") list = list.filter((g) => g.totalPlaytimeSec > 0);
    else if (view.startsWith("c:")) {
      const set = memberSets.get(Number(view.slice(2)));
      list = list.filter((g) => set?.has(g.id));
    } else if (view.startsWith("s:")) list = list.filter((g) => g.systemId === view.slice(2));
    const q = query.trim().toLowerCase();
    if (q) list = list.filter((g) => g.title.toLowerCase().includes(q));
    const key: SortKey = view === "recent" ? "played" : view === "most" ? "time" : sort;
    const sorted = [...list].sort(COMPARE[key]);
    return view === "recent" ? sorted.slice(0, 40) : sorted;
  }, [games, view, query, sort, memberSets]);

  const emptyText =
    view === "fav"
      ? "No favorites yet. Click the star on a game to add it here."
      : view === "recent"
        ? "Nothing played yet."
        : view === "most"
          ? "Play something and it will show up here."
          : selectedCollection
            ? "This collection is empty. Open a game and tick the collection on its page."
            : query
              ? "No games match your search."
              : "Nothing here.";

  const navButton = (id: string, label: string, count?: number) => (
    <button key={id} className="chip" aria-pressed={view === id} onClick={() => setView(id)}>
      {label} {count !== undefined && <span>{count}</span>}
    </button>
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
          <nav className="systems" aria-label="Library views">
            {navButton("all", "All games", games.length)}
            {navButton("fav", "Favorites", favCount)}
            {navButton("recent", "Recently played")}
            {navButton("most", "Most played")}

            <div className="nav-heading">Collections</div>
            {collections.map((c) => navButton(`c:${c.id}`, c.name, c.count))}
            <form
              className="new-collection"
              onSubmit={(e) => {
                e.preventDefault();
                if (newName.trim()) createCollection();
              }}
            >
              <input
                className="search"
                placeholder="New collection"
                aria-label="New collection name"
                maxLength={60}
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
              />
              <button className="btn" type="submit" disabled={!newName.trim() || busy}>
                Add
              </button>
            </form>

            <div className="nav-heading">Systems</div>
            {Object.keys(sysCounts)
              .sort((a, b) => (names[a] ?? a).localeCompare(names[b] ?? b))
              .map((id) => navButton(`s:${id}`, names[id] ?? id, sysCounts[id]))}
          </nav>

          <div>
            <div className="viewbar">
              {selectedCollection ? (
                renaming === null ? (
                  <>
                    <h2>{selectedCollection.name}</h2>
                    <button className="btn" onClick={() => setRenaming(selectedCollection.name)}>
                      Rename
                    </button>
                    <button className="btn danger" onClick={deleteCollection}>
                      Delete collection
                    </button>
                  </>
                ) : (
                  <form
                    className="row"
                    onSubmit={(e) => {
                      e.preventDefault();
                      renameCollection();
                    }}
                  >
                    <input
                      className="search"
                      aria-label="Collection name"
                      maxLength={60}
                      autoFocus
                      value={renaming}
                      onChange={(e) => setRenaming(e.target.value)}
                    />
                    <button className="btn primary" type="submit" disabled={!renaming.trim()}>
                      Save
                    </button>
                    <button className="btn" type="button" onClick={() => setRenaming(null)}>
                      Cancel
                    </button>
                  </form>
                )
              ) : (
                <span className="muted small">
                  {shown.length} game{shown.length === 1 ? "" : "s"}
                </span>
              )}
              <span className="spacer" />
              {!fixedOrder && (
                <label className="sortlabel">
                  Sort{" "}
                  <select className="select" value={sort} onChange={(e) => setSort(e.target.value as SortKey)}>
                    {SORTS.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.label}
                      </option>
                    ))}
                  </select>
                </label>
              )}
            </div>

            {shown.length === 0 ? (
              <p className="muted">{emptyText}</p>
            ) : (
              <ul className="grid">
                {shown.map((g) => (
                  <GameTile
                    key={g.id}
                    game={g}
                    systemName={names[g.systemId] ?? g.systemId}
                    running={running.has(g.id)}
                    onPlay={() => play(g)}
                    onOpen={() => onOpen(g.id)}
                    onFavorite={() => toggleFavorite(g)}
                    onRemove={() => removeGame(g)}
                  />
                ))}
              </ul>
            )}
          </div>
        </div>
      )}
    </>
  );
}
