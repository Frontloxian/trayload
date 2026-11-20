import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import BackupPanel from "./BackupPanel";
import type { AppInfo, ArtSettings, RemovedGame } from "./types";
import type { Music } from "./useMusic";

export default function Settings({ music }: { music: Music }) {
  const [hasKey, setHasKey] = useState(false);
  const [key, setKey] = useState("");
  const [folders, setFolders] = useState<string[]>([]);
  const [removed, setRemoved] = useState<RemovedGame[]>([]);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [auto, setAuto] = useState(() => {
    try {
      return localStorage.getItem("trayload.autoUpdate") === "1";
    } catch {
      return false;
    }
  });
  const [msg, setMsg] = useState("");
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    invoke<ArtSettings>("get_art_settings").then((s) => setHasKey(s.hasSteamgriddbKey));
    invoke<string[]>("get_rom_folders").then(setFolders);
    invoke<RemovedGame[]>("list_removed_games").then(setRemoved).catch(() => {});
    invoke<AppInfo>("get_app_info").then(setInfo).catch(() => {});
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
          steamgriddb.com under Preferences, then API. The key is kept in the Windows credential store on this PC.
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
        <h2>Music</h2>
        <p className="muted">
          Trayload plays the soundtrack in the background and pauses it while a game runs. A music pack is a folder of
          audio files (mp3, ogg, opus, wav, flac, m4a). An optional pack.json in the folder can set a name and author,
          like {"{"}"name": "My pack", "author": "Me"{"}"}. Only use music you have the right to use.
        </p>
        <div className="row wrap">
          <label>
            <input
              type="checkbox"
              checked={music.prefs.enabled}
              onChange={(e) => music.setPrefs({ enabled: e.target.checked })}
            />{" "}
            Play music
          </label>
          <label>
            <input
              type="checkbox"
              checked={music.prefs.shuffle}
              onChange={(e) => music.setPrefs({ shuffle: e.target.checked })}
            />{" "}
            Shuffle
          </label>
        </div>
        <div className="row">
          <input
            className="vol"
            type="range"
            min={0}
            max={100}
            value={Math.round(music.prefs.volume * 100)}
            aria-label="Music volume"
            onChange={(e) => music.setPrefs({ volume: Number(e.target.value) / 100 })}
          />
        </div>
        {music.packs.map((p) => (
          <div key={p.id} className="pack">
            <label>
              <input
                type="radio"
                name="music-pack"
                checked={(music.packs.find((x) => x.id === music.prefs.pack) ?? music.packs[0])?.id === p.id}
                onChange={() => music.setPrefs({ pack: p.id })}
              />
              <span>
                <strong>{p.name}</strong>
                <span className="muted small">
                  {" "}
                  {p.tracks.length} track{p.tracks.length === 1 ? "" : "s"}
                  {p.author ? `, by ${p.author}` : ""}
                </span>
              </span>
            </label>
            {!p.builtin && (
              <button
                className="btn danger"
                onClick={() => {
                  if (window.confirm(`Remove the pack "${p.name}"? This deletes its copy of the music files.`))
                    act(async () => {
                      await invoke("remove_music_pack", { id: p.id });
                      music.reload();
                    }, "Pack removed.");
                }}
              >
                Remove
              </button>
            )}
          </div>
        ))}
        <div className="row wrap">
          <button
            className="btn"
            onClick={() =>
              act(async () => {
                const dir = await open({ directory: true });
                if (typeof dir === "string") {
                  await invoke("add_music_pack", { path: dir });
                  music.reload();
                }
              }, "Pack added.")
            }
          >
            Add pack from folder
          </button>
          <button className="btn" onClick={() => act(() => invoke("open_music_folder"), "Opened the music folder.")}>
            Open music folder
          </button>
          <button className="btn" onClick={() => { music.reload(); setMsg("Packs reloaded."); }}>
            Reload
          </button>
        </div>
        {music.error && <p className="error small">{music.error}</p>}
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

      <section className="section">
        <h2>Emulator updates</h2>
        <p className="muted">
          Trayload always shows "Update ready" when a newer emulator version exists. You can also let it install
          updates by itself when it starts. The old version is kept as a backup until the next update.
        </p>
        <label>
          <input
            type="checkbox"
            checked={auto}
            onChange={(e) => {
              setAuto(e.target.checked);
              try {
                localStorage.setItem("trayload.autoUpdate", e.target.checked ? "1" : "0");
              } catch {
                /* remembering is optional */
              }
            }}
          />{" "}
          Update emulators automatically
        </label>
      </section>

      <section className="section">
        <h2>Removed games</h2>
        <p className="muted">
          Games you removed from the library stay out of it, even when a folder is scanned again. Their files are never
          deleted. Restore one to bring it back.
        </p>
        {removed.length === 0 ? (
          <p className="muted">Nothing removed.</p>
        ) : (
          removed.map((g) => (
            <div key={g.id} className="folder">
              <span>
                {g.title} <span className="muted small">{g.path}</span>
              </span>
              <button className="btn" onClick={() => act(() => invoke("restore_game", { id: g.id }), "Game restored.")}>
                Restore
              </button>
            </div>
          ))
        )}
      </section>

      <BackupPanel />

      <section className="section">
        <h2>About and troubleshooting</h2>
        <p className="muted">
          Trayload {info?.version ?? ""} keeps a small log file on this PC to help find problems. Nothing is ever sent
          anywhere.
        </p>
        <div className="row wrap">
          <button className="btn" onClick={() => act(() => invoke("open_logs_folder"), "Opened the log folder.")}>
            Open log folder
          </button>
        </div>
        {info && <div className="muted small folder-path">{info.dataFolder}</div>}
      </section>
    </>
  );
}
