import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import BackupPanel from "./BackupPanel";
import LegacyCleanup from "./LegacyCleanup";
import { Dialog } from "./Overlays";
import type {
  AppInfo,
  ArtSettings,
  DeleteSummary,
  GamesFolder,
  LegacyStatus,
  LeftoverOriginal,
  OrganizeCandidate,
  OrganizeSummary,
  RemoteStatus,
  RemovedGame,
} from "./types";
import type { VerifyProgress, VerifySummary } from "./types";
import { ensurePermission, notifyEnabled, setNotifyEnabled } from "./notify";
import type { Music } from "./useMusic";
import type { AppUpdate } from "./useAppUpdate";

export default function Settings({ music, appUpdate, onWhatsNew }: { music: Music; appUpdate: AppUpdate; onWhatsNew: () => void }) {
  const [tab, setTab] = useState<string>(() => {
    try {
      return localStorage.getItem("trayload.settingsTab") || "games";
    } catch {
      return "games";
    }
  });
  const [query, setQuery] = useState("");
  const [grouped, setGrouped] = useState<boolean>(() => {
    try {
      return localStorage.getItem("trayload.groupVersions") !== "0";
    } catch {
      return true;
    }
  });
  function toggleGrouped(on: boolean) {
    setGrouped(on);
    try {
      localStorage.setItem("trayload.groupVersions", on ? "1" : "0");
    } catch {
      /* fine without it */
    }
    window.dispatchEvent(new Event("trayload-grouping"));
  }
  const [verifying, setVerifying] = useState(false);
  const [vprog, setVprog] = useState<VerifyProgress | null>(null);
  const [vsum, setVsum] = useState<VerifySummary | null>(null);
  const [vErr, setVErr] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [key, setKey] = useState("");
  const [folders, setFolders] = useState<string[]>([]);
  const [removed, setRemoved] = useState<RemovedGame[]>([]);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [gamesFolder, setGamesFolder] = useState<GamesFolder | null>(null);
  const [ignored, setIgnored] = useState<string[]>([]);
  const [legacy, setLegacy] = useState<LegacyStatus | null>(null);
  const [online, setOnline] = useState(() => {
    try {
      return localStorage.getItem("trayload.checkRegistry") !== "0";
    } catch {
      return true;
    }
  });
  const [notify, setNotify] = useState(notifyEnabled);
  const [remote, setRemote] = useState<RemoteStatus | null>(null);
  const [remoteNote, setRemoteNote] = useState("");
  const [mode, setMode] = useState<"copy" | "move">("copy");
  // Feedback for the "bring existing games in" button, shown right under it (the top of the page may be out of view).
  const [orgNote, setOrgNote] = useState<{ text: string; bad: boolean } | null>(null);
  const [leftovers, setLeftovers] = useState<{ items: LeftoverOriginal[]; picked: Set<string> } | null>(null);
  const [plan, setPlan] = useState<{ items: OrganizeCandidate[]; ids: Set<number> } | null>(null);
  // Permanent delete asks first: one game (its id), everything removed ("all"), or nothing (null).
  const [confirmDelete, setConfirmDelete] = useState<RemovedGame | "all" | null>(null);
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
    invoke<GamesFolder>("get_games_folder").then(setGamesFolder).catch(() => {});
    invoke<string[]>("list_ignored_files").then(setIgnored).catch(() => {});
    invoke<LegacyStatus>("legacy_status").then(setLegacy).catch(() => {});
    invoke<RemoteStatus>("remote_status").then(setRemote).catch(() => {});
    invoke<string>("get_organize_mode").then((m) => setMode(m === "move" ? "move" : "copy")).catch(() => {});
  }, []);

  async function checkNow() {
    setRemoteNote("Checking…");
    try {
      const s = await invoke<RemoteStatus>("refresh_remote");
      setRemote(s);
      setRemoteNote(
        !s.online
          ? "Could not reach GitHub. Trayload keeps using the list it has."
          : s.registryUpdated || s.compatUpdated
            ? "Updated. Restart Trayload to be sure everything uses the new list."
            : "You already have the newest list.",
      );
    } catch (e) {
      setRemoteNote(String(e));
    }
  }

  async function toggleNotify(on: boolean) {
    if (on && !(await ensurePermission())) {
      setRemoteNote("Windows did not allow notifications for Trayload. Check Settings > System > Notifications.");
      return;
    }
    setNotify(on);
    setNotifyEnabled(on);
  }

  async function chooseMode(m: "copy" | "move") {
    setMode(m);
    await invoke("set_organize_mode", { mode: m }).catch((e) => setError(String(e)));
  }

  async function openOrganize() {
    setOrgNote({ text: "Looking for games to bring in…", bad: false });
    try {
      const items = await invoke<OrganizeCandidate[]>("plan_organize");
      if (items.length === 0) {
        setOrgNote({
          text: "Nothing to do: every game in your library is already inside the games folder, or its file can't be found.",
          bad: false,
        });
        return;
      }
      setOrgNote(null);
      setPlan({ items, ids: new Set(items.map((i) => i.id)) });
    } catch (e) {
      setOrgNote({ text: String(e), bad: true });
    }
  }

  async function openLeftovers() {
    setOrgNote({ text: "Looking for originals…", bad: false });
    try {
      const items = await invoke<LeftoverOriginal[]>("plan_originals");
      if (items.length === 0) {
        setOrgNote({
          text: "No copied originals found. Trayload only knows about games it copied since this version. To include older copies, drag the original files onto the Trayload window once: identical files are skipped, and Trayload remembers where they came from.",
          bad: false,
        });
        return;
      }
      setOrgNote(null);
      setLeftovers({ items, picked: new Set(items.map((i) => i.original)) });
    } catch (e) {
      setOrgNote({ text: String(e), bad: true });
    }
  }

  async function runRemoveOriginals() {
    if (!leftovers) return;
    const originals = [...leftovers.picked];
    setLeftovers(null);
    setOrgNote({ text: "Removing originals…", bad: false });
    try {
      const r = await invoke<{ removed: number; failed: string[] }>("remove_originals", { originals });
      const text = `Moved ${r.removed} original file${r.removed === 1 ? "" : "s"} to the Recycle Bin.`;
      setOrgNote({ text: r.failed.length ? `${text} ${r.failed.join(" ")}` : text, bad: r.failed.length > 0 });
    } catch (e) {
      setOrgNote({ text: String(e), bad: true });
    }
  }

  async function runOrganize() {
    if (!plan) return;
    const ids = plan.items.filter((i) => plan.ids.has(i.id)).map((i) => i.id);
    setPlan(null);
    setOrgNote({ text: "Working… the progress bar shows at the bottom of the window.", bad: false });
    try {
      const r = await invoke<OrganizeSummary>("organize_games", { ids });
      const verb = r.mode === "move" ? "Moved" : "Copied";
      const text = `${verb} ${r.done} game${r.done === 1 ? "" : "s"} into the games folder.${r.cancelled ? " Cancelled before the rest." : ""}`;
      setOrgNote({ text: r.failed.length ? `${text} ${r.failed.join(" ")}` : text, bad: r.failed.length > 0 });
    } catch (e) {
      setOrgNote({ text: String(e), bad: true });
    }
    load();
  }

  async function pickGamesFolder() {
    const dir = await open({ directory: true, title: "Choose the games folder" });
    if (typeof dir === "string") {
      await act(async () => {
        await invoke("set_games_folder", { path: dir });
        await invoke("rescan");
      }, "Games folder changed. Games in the old folder stay in your library.");
    }
  }

  async function deletePermanently(target: RemovedGame | "all") {
    setConfirmDelete(null);
    setError(null);
    setMsg("");
    try {
      const r =
        target === "all"
          ? await invoke<DeleteSummary>("delete_all_removed")
          : await invoke<DeleteSummary>("delete_removed_game", { id: target.id });
      const bits = [`Deleted ${r.deleted} game${r.deleted === 1 ? "" : "s"} from Trayload.`];
      if (r.recycled) bits.push(`${r.recycled} file${r.recycled === 1 ? "" : "s"} moved to the Recycle Bin.`);
      if (r.returned) bits.push(`${r.returned} file${r.returned === 1 ? "" : "s"} put back where ${r.returned === 1 ? "it" : "they"} came from.`);
      if (r.leftAlone) bits.push(`${r.leftAlone} file${r.leftAlone === 1 ? " was" : "s were"} outside Trayload's folder and left alone.`);
      setMsg(bits.join(" "));
      if (r.failed.length) setError(r.failed.join(" "));
    } catch (e) {
      setError(String(e));
    }
    load();
  }

  useEffect(load, [load]);

  // Show one section at a time; a search shows every matching section instead.
  const paneRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const q = query.trim().toLowerCase();
    let hits = 0;
    paneRef.current?.querySelectorAll<HTMLElement>("[data-group]").forEach((el) => {
      const show = q ? (el.textContent ?? "").toLowerCase().includes(q) : el.dataset.group === tab;
      el.hidden = !show;
      if (show) hits++;
    });
    if (paneRef.current) paneRef.current.dataset.hits = String(hits);
  });
  useEffect(() => {
    const un = listen<VerifyProgress>("verify-progress", (e) => setVprog(e.payload));
    return () => {
      void un.then((f) => f());
    };
  }, []);

  async function useOfficialNames() {
    setVErr("");
    try {
      const n = await invoke<number>("use_official_names", { ids: null, dryRun: true });
      if (n === 0) {
        setMsg("Every verified game already has its official name.");
        return;
      }
      if (!window.confirm(`Rename ${n} verified game${n === 1 ? "" : "s"} to their official names?\n\nOnly the name shown in Trayload changes. Your files are not renamed.`)) return;
      await invoke("use_official_names", { ids: null, dryRun: false });
      setMsg(`Renamed ${n} game${n === 1 ? "" : "s"}.`);
    } catch (err) {
      setVErr(String(err));
    }
  }

  async function runVerify() {
    setVerifying(true);
    setVsum(null);
    setVErr("");
    setVprog(null);
    try {
      setVsum(await invoke<VerifySummary>("verify_games", { ids: null }));
    } catch (err) {
      setVErr(String(err));
    }
    setVerifying(false);
  }

  function pickTab(id: string) {
    setTab(id);
    setQuery("");
    try {
      localStorage.setItem("trayload.settingsTab", id);
    } catch {
      /* fine without it */
    }
  }

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

  const TABS: [string, string][] = [
    ["games", "Games"],
    ["look", "Artwork and music"],
    ["updates", "Updates"],
    ["backup", "Backup"],
    ["advanced", "Advanced"],
  ];

  return (
    <div className="settings-layout">
      <nav className="settings-nav" aria-label="Settings sections">
        <input
          type="search"
          className="settings-search"
          placeholder="Search settings"
          aria-label="Search settings"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {TABS.map(([id, label]) => (
          <button
            key={id}
            className={"settings-tab" + (!query && tab === id ? " active" : "")}
            aria-current={!query && tab === id ? "page" : undefined}
            onClick={() => pickTab(id)}
          >
            {label}
          </button>
        ))}
      </nav>
      <div className="settings-pane" ref={paneRef}>
      {(error || msg) && (
        <div className="settings-flash" aria-live="polite">
          {error && <p className="error">{error}</p>}
          {msg && <p className="muted">{msg}</p>}
        </div>
      )}
      {query && <p className="muted small">Showing sections that mention "{query}". Clear the search to go back to the sections.</p>}

      <section className="section" data-group="look">
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

      <section className="section" data-group="look">
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

      <section className="section" data-group="games">
        <h2>Games folder</h2>
        <p className="muted">
          Games you drag onto Trayload are copied into this folder, in one folder per console (for example Nintendo
          64). Your originals are never changed. The folder is scanned automatically.
        </p>
        {gamesFolder && (
          <div className="folder">
            <code>{gamesFolder.path}</code>
            {gamesFolder.isDefault && <span className="muted small">Default</span>}
          </div>
        )}
        <div className="row wrap">
          <button className="btn" onClick={() => void pickGamesFolder()}>
            Change folder…
          </button>
          <button className="btn" onClick={() => act(() => invoke("open_games_folder"), "Opened the games folder.")}>
            Open games folder
          </button>
        </div>
        <h3>Games you added earlier</h3>
        <p className="muted">
          Bring games that are already in your library into the games folder. You see the list first and pick what
          to include.
        </p>
        <div className="row wrap">
          <label>
            <input type="radio" name="organize" checked={mode === "copy"} onChange={() => void chooseMode("copy")} /> Copy
            (originals stay where they are)
          </label>
          <label>
            <input type="radio" name="organize" checked={mode === "move"} onChange={() => void chooseMode("move")} /> Move
            (originals are removed after the copy is checked)
          </label>
        </div>
        <div className="row">
          <button className="btn" onClick={() => void openOrganize()}>
            Bring existing games in…
          </button>
          {mode === "move" && (
            <button className="btn" onClick={() => void openLeftovers()}>
              Remove originals of copied games…
            </button>
          )}
        </div>
        {orgNote && <p className={orgNote.bad ? "error small" : "muted small"}>{orgNote.text}</p>}
      </section>

      <section className="section" data-group="games">
        <h2>Check games against official lists</h2>
        <p className="muted">
          Compares your games with the community lists: No-Intro for cartridges (NES, SNES, Game Boy, Game Boy Advance,
          Nintendo 64, DS, Sega 8 and 16-bit) and Redump for discs (PlayStation, PS2, PSP, GameCube, Wii, Dreamcast,
          Saturn, Sega CD, Xbox), so you can tell a clean dump from a hack or a bad file. Disc images in .chd, .rvz and
          similar compressed formats can't be checked. Trayload downloads the list for each console from GitHub
          (libretro-database) the first time, keeps it for 30 days, and reads your files on this PC. Nothing about your files is sent anywhere. Results show on each game's page.
        </p>
        <div className="row wrap">
          <button className="btn" disabled={verifying} onClick={() => void runVerify()}>
            {verifying ? "Checking…" : "Check my games"}
          </button>
          <button className="btn" disabled={verifying} onClick={() => void useOfficialNames()}>
            Use official names…
          </button>
          {verifying && (
            <button className="btn" onClick={() => void invoke("cancel_verify")}>
              Cancel
            </button>
          )}
        </div>
        {verifying && vprog && vprog.total > 0 && (
          <>
            <div className="verify-bar" aria-hidden="true">
              <span style={{ width: `${Math.round((vprog.done / vprog.total) * 100)}%` }} />
            </div>
            <p className="muted small" aria-live="polite">
              {vprog.done} of {vprog.total}
              {vprog.title ? ` · ${vprog.title}` : ""}
            </p>
          </>
        )}
        {vErr && <p className="error small">{vErr}</p>}
        {vsum && (
          <p className="muted" aria-live="polite">
            {vsum.cancelled ? "Stopped. " : "Done. "}
            {vsum.verified} verified, {vsum.unknown} not in the list
            {vsum.unsupported ? `, ${vsum.unsupported} could not be checked` : ""}
            {vsum.skipped ? `, ${vsum.skipped} already verified` : ""}.
            {vsum.noList > 0 &&
              ` ${vsum.noList} game${vsum.noList === 1 ? "" : "s"} skipped because the list could not be downloaded. Check your internet connection and try again.`}
          </p>
        )}
      </section>

      <section className="section" data-group="games">
        <h2>Regions and versions</h2>
        <label>
          <input type="checkbox" checked={grouped} onChange={(e) => toggleGrouped(e.target.checked)} /> Show the regions
          and versions of a game as one tile in the Library
        </label>
        <p className="muted small">
          For example "Tetris (USA)" and "Tetris (Japan)" become one tile, with the others listed on the game's page.
          Your favorite or most played version is shown, otherwise USA, then World, then Europe. Games are matched by
          name, so it works best after "Use official names". Lists that follow your play history always show every game.
        </p>
      </section>

      <section className="section" data-group="games">
        <h2>Scanned folders</h2>
        <p className="muted">
          These folders are scanned every time Trayload starts, and their games stay where they are (nothing is copied).
          Add folders from the Library tab. Removing one here doesn't delete anything and keeps the games already in your
          library.
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

      <section className="section" data-group="updates">
        <h2>Trayload updates</h2>
        <p className="muted">
          Trayload {info?.version ?? ""}. Checking for an update contacts github.com (this project's releases) and
          nothing else. Updates are signed and checked before they are installed.
        </p>
        <label>
          <input
            type="checkbox"
            checked={appUpdate.startupCheck}
            onChange={(e) => appUpdate.setStartupCheck(e.target.checked)}
          />{" "}
          Check for Trayload updates when it starts
        </label>
        <div className="row wrap">
          <button
            className="btn"
            disabled={appUpdate.state === "checking" || appUpdate.state === "installing"}
            onClick={() => void appUpdate.check()}
          >
            {appUpdate.state === "checking" ? "Checking…" : "Check for updates"}
          </button>
          {appUpdate.state === "found" && (
            <button className="btn primary" onClick={() => void appUpdate.install()}>
              Install {appUpdate.version} and restart
            </button>
          )}
          <button className="btn" onClick={onWhatsNew}>
            What's new
          </button>
        </div>
        {appUpdate.state === "none" && <p className="muted">You are up to date.</p>}
        {appUpdate.state === "found" && (
          <p className="muted" aria-live="polite">
            Version {appUpdate.version} is available.
          </p>
        )}
        {appUpdate.state === "installing" && <p className="muted">Downloading and installing…</p>}
        {appUpdate.state === "error" && <p className="error">Could not check for updates: {appUpdate.error}</p>}
      </section>

      <section className="section" data-group="updates">
        <h2>Online updates</h2>
        <p className="muted">
          Trayload can fetch a newer emulator list (download names and launch options) and compatibility notes from its
          own GitHub page, so a broken download can be fixed without a new app version. It is a plain download of a
          public file: nothing about you is sent. A downloaded list can never point an emulator at a different project.
        </p>
        <label>
          <input
            type="checkbox"
            checked={online}
            onChange={(e) => {
              setOnline(e.target.checked);
              try {
                localStorage.setItem("trayload.checkRegistry", e.target.checked ? "1" : "0");
              } catch {
                /* remembering is optional */
              }
            }}
          />{" "}
          Check for emulator list updates when Trayload starts
        </label>
        <div className="row wrap">
          <button className="btn" onClick={() => void checkNow()}>
            Check now
          </button>
          {remote && (
            <span className="muted small">
              Emulator list version {remote.registryRevision}, compatibility notes version {remote.compatRevision}
            </span>
          )}
        </div>
        {remoteNote && <p className="muted small">{remoteNote}</p>}
        <label>
          <input type="checkbox" checked={notify} onChange={(e) => void toggleNotify(e.target.checked)} />{" "}
          Show a desktop notification when an emulator update is ready (only while Trayload is open)
        </label>
      </section>

      <section className="section" data-group="updates">
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

      <section className="section" data-group="games">
        <h2>Removed games</h2>
        <p className="muted">
          Games you removed from the library stay out of it, even when a folder is scanned again. Restore one to bring
          it back. Delete permanently removes every trace of a game from Trayload (its cover, play time, favorite and
          collections too). Files in Trayload's games folder go to the Recycle Bin (games that were moved in go back where they came from). Files anywhere else are never touched.
        </p>
        {removed.length > 1 && (
          <div className="row">
            <button className="btn danger" onClick={() => setConfirmDelete("all")}>
              Delete all permanently
            </button>
          </div>
        )}
        {removed.length === 0 ? (
          <p className="muted">Nothing removed.</p>
        ) : (
          removed.map((g) => (
            <div key={g.id} className="folder">
              <span>
                {g.title} <span className="muted small">{g.path}</span>
              </span>
              <span className="row">
                <button className="btn" onClick={() => act(() => invoke("restore_game", { id: g.id }), "Game restored.")}>
                  Restore
                </button>
                <button className="btn danger" onClick={() => setConfirmDelete(g)}>
                  Delete permanently
                </button>
              </span>
            </div>
          ))
        )}
      </section>

      {ignored.length > 0 && (
        <section className="section" data-group="games">
          <h2>Ignored files</h2>
          <p className="muted">
            These files were deleted from Trayload but still sit in a folder it scans, so Trayload remembers only their
            path to keep them from coming back. Clearing the list makes them show up again on the next scan.
          </p>
          {ignored.map((p) => (
            <div key={p} className="folder">
              <code>{p}</code>
              <button className="btn" onClick={() => act(() => invoke("clear_ignored_files", { path: p }), "No longer ignored.")}>
                Stop ignoring
              </button>
            </div>
          ))}
          <div className="row">
            <button className="btn" onClick={() => act(() => invoke("clear_ignored_files", { path: null }), "Ignored list cleared.")}>
              Clear the list
            </button>
          </div>
        </section>
      )}

      {legacy?.present && (
        <section className="section" data-group="advanced">
          <h2>Old RetroArch folder</h2>
          <LegacyCleanup status={legacy} onChanged={load} />
        </section>
      )}

      <div data-group="backup">
        <BackupPanel />
      </div>

      <section className="section" data-group="advanced">
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
      {leftovers && (
        <Dialog title="Remove the originals of copied games" onClose={() => setLeftovers(null)} wide>
          <p className="muted">
            These files were copied into the games folder and the originals are still where they were. Each original is
            compared with its copy first, then moved to the Recycle Bin. If you delete the game permanently later, the
            files are put back.
          </p>
          <ul className="plain pick-list">
            {leftovers.items.map((i) => (
              <li key={i.original}>
                <label>
                  <input
                    type="checkbox"
                    checked={leftovers.picked.has(i.original)}
                    onChange={(ev) =>
                      setLeftovers((p) => {
                        if (!p) return p;
                        const picked = new Set(p.picked);
                        if (ev.target.checked) picked.add(i.original);
                        else picked.delete(i.original);
                        return { ...p, picked };
                      })
                    }
                  />{" "}
                  <span className="small">{i.original}</span>{" "}
                  <span className="muted small">{Math.max(1, Math.round(i.bytes / 1048576))} MB</span>
                </label>
              </li>
            ))}
          </ul>
          <div className="row">
            <button className="btn danger" disabled={leftovers.picked.size === 0} onClick={() => void runRemoveOriginals()}>
              Remove {leftovers.picked.size} original{leftovers.picked.size === 1 ? "" : "s"}
            </button>
            <button className="btn" onClick={() => setLeftovers(null)}>
              Cancel
            </button>
          </div>
        </Dialog>
      )}
      {plan && (
        <Dialog title="Bring existing games into the games folder" onClose={() => setPlan(null)} wide>
          <p className="muted">
            {mode === "move"
              ? "Mode: move. Each game's files are copied, checked, then removed from where they were. If you delete a moved game permanently later, its files go back to their original place."
              : "Mode: copy. Your originals stay where they are, so this uses extra disk space. (Change the mode in Settings.)"}
          </p>
          <ul className="plain pick-list">
            {plan.items.map((i) => (
              <li key={i.id}>
                <label>
                  <input
                    type="checkbox"
                    checked={plan.ids.has(i.id)}
                    onChange={(ev) =>
                      setPlan((p) => {
                        if (!p) return p;
                        const ids = new Set(p.ids);
                        if (ev.target.checked) ids.add(i.id);
                        else ids.delete(i.id);
                        return { ...p, ids };
                      })
                    }
                  />{" "}
                  <strong>{i.title}</strong>{" "}
                  <span className="muted small">
                    {i.systemName} · {Math.max(1, Math.round(i.bytes / 1048576))} MB
                  </span>
                </label>
                <div className="muted small">{i.path}</div>
              </li>
            ))}
          </ul>
          <p className="muted small">
            {plan.ids.size} selected ·{" "}
            {(plan.items.filter((i) => plan.ids.has(i.id)).reduce((n, i) => n + i.bytes, 0) / 1073741824).toFixed(1)} GB
          </p>
          <div className="row">
            <button className="btn primary" disabled={plan.ids.size === 0} onClick={() => void runOrganize()}>
              {mode === "move" ? "Move" : "Copy"} {plan.ids.size} game{plan.ids.size === 1 ? "" : "s"}
            </button>
            <button className="btn" onClick={() => setPlan(null)}>
              Cancel
            </button>
          </div>
        </Dialog>
      )}
      {confirmDelete && (
        <Dialog title="Delete permanently?" onClose={() => setConfirmDelete(null)}>
          <p className="muted">
            {confirmDelete === "all"
              ? `This removes all ${removed.length} removed games from Trayload for good.`
              : `This removes ${confirmDelete.title} from Trayload for good.`}{" "}
            Their covers, play time, favorites and collection entries go too. Files in Trayload's games folder are moved
            to the Recycle Bin (games you moved in go back where they came from). Files anywhere else are not touched.
          </p>
          <div className="row">
            <button className="btn danger" onClick={() => void deletePermanently(confirmDelete)}>
              Delete permanently
            </button>
            <button className="btn" onClick={() => setConfirmDelete(null)}>
              Cancel
            </button>
          </div>
        </Dialog>
      )}
      </div>
    </div>
  );
}
