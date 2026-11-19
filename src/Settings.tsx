import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ArtSettings } from "./types";

export default function Settings() {
  const [hasKey, setHasKey] = useState(false);
  const [key, setKey] = useState("");
  const [folders, setFolders] = useState<string[]>([]);
  const [msg, setMsg] = useState("");
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    invoke<ArtSettings>("get_art_settings").then((s) => setHasKey(s.hasSteamgriddbKey));
    invoke<string[]>("get_rom_folders").then(setFolders);
  }, []);

  useEffect(load, [load]);

  async function act(fn: () => Promise<unknown>, done: string) {
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

  return (
    <>
      {error && <p className="error">{error}</p>}
      {msg && (
        <p className="muted" aria-live="polite">
          {msg}
        </p>
      )}

      <section className="section">
        <h2>Artwork</h2>
        <p className="muted">
          Covers come from libretro-thumbnails (no key needed) and, if you add a key, SteamGridDB. Get a free key on
          steamgriddb.com under Preferences, then API. The key stays on this PC.
        </p>
        <div className="row">
          <input
            className="search"
            type="password"
            autoComplete="off"
            placeholder={hasKey ? "A key is saved. Paste a new one to replace it" : "SteamGridDB API key"}
            aria-label="SteamGridDB API key"
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
          <button
            className="btn primary"
            disabled={!key.trim()}
            onClick={() =>
              act(async () => {
                await invoke("set_steamgriddb_key", { key });
                setKey("");
              }, "Key saved. Games without a cover will be looked up again.")
            }
          >
            Save key
          </button>
          {hasKey && (
            <button className="btn danger" onClick={() => act(() => invoke("set_steamgriddb_key", { key: "" }), "Key removed.")}>
              Remove key
            </button>
          )}
        </div>
        <div className="row">
          <button className="btn" onClick={() => act(() => invoke("retry_art"), "Looking for missing covers again.")}>
            Retry missing covers
          </button>
        </div>
      </section>

      <section className="section">
        <h2>ROM folders</h2>
        <p className="muted">
          These folders are scanned every time Trayload starts. Add folders from the Library tab. Removing one here
          doesn't delete anything and keeps the games already in your library.
        </p>
        {folders.length === 0 ? (
          <p className="muted">No folders yet.</p>
        ) : (
          folders.map((f) => (
            <div key={f} className="folder">
              <code>{f}</code>
              <button className="btn" onClick={() => act(() => invoke("remove_rom_folder", { path: f }), "Folder removed from scanning.")}>
                Remove
              </button>
            </div>
          ))
        )}
      </section>
    </>
  );
}
