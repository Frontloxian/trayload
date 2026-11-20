import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

interface SourceInfo { key: string; name: string; files: number; bytes: number }
interface Preview { sources: SourceInfo[]; gameFiles: number }
interface Manifest { createdAt: number; sources: SourceInfo[]; gameFolders: [number, string][]; gameFiles: number; library: boolean }
interface Restore { restored: string[]; skipped: string[]; files: number; libraryPending: boolean }

const mb = (b: number) => (b < 1048576 ? `${Math.max(1, Math.round(b / 1024))} KB` : `${(b / 1048576).toFixed(1)} MB`);

export default function BackupPanel() {
  const [dest, setDest] = useState<string | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [info, setInfo] = useState<Manifest | null>(null);
  const [gameSaves, setGameSaves] = useState(true);
  const [restoreLib, setRestoreLib] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");
  const [count, setCount] = useState(0);

  useEffect(() => {
    invoke<Preview>("backup_preview").then(setPreview).catch(() => {});
    const p = listen<{ done: number }>("backup-progress", (e) => setCount(e.payload.done));
    return () => void p.then((f) => f());
  }, []);

  const pick = async () => {
    const dir = await open({ directory: true });
    if (typeof dir !== "string") return;
    setDest(dir);
    setMsg("");
    setErr("");
    setInfo(await invoke<Manifest | null>("backup_info", { dest: dir }).catch(() => null));
  };

  const run = async (label: string, fn: () => Promise<void>) => {
    setBusy(label);
    setErr("");
    setMsg("");
    setCount(0);
    try {
      await fn();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const backup = () =>
    run("backup", async () => {
      const m = await invoke<Manifest>("backup_run", { dest, gameSaves });
      setInfo(m);
      setMsg(`Backed up ${m.sources.length} emulator${m.sources.length === 1 ? "" : "s"}${m.gameFiles ? `, ${m.gameFiles} game save files` : ""}${m.library ? " and your library data" : ""}. You can unplug the drive now.`);
    });

  const restore = () => {
    if (!window.confirm("Restore from this backup?\n\nSave files and settings in the backup will overwrite the ones on this PC.")) return;
    run("restore", async () => {
      const r = await invoke<Restore>("restore_run", { dest, restoreLibrary: restoreLib });
      setMsg(`Restored ${r.files} files${r.restored.length ? ` for ${r.restored.join(", ")}` : ""}.${r.libraryPending ? " Restart Trayload to bring back your library data." : ""}${r.skipped.length ? ` Skipped: ${r.skipped.join("; ")}.` : ""}`);
    });
  };

  return (
    <section className="section">
      <h2>USB backup</h2>
      <p className="muted">
        Copies your save files, save states and emulator settings to a USB drive or any folder, and puts them back on
        this or another PC. Only copies are made; nothing on the drive is deleted. Pick the drive, then back up. A
        second backup only copies what changed.
      </p>
      {preview && (
        <p className="muted small">
          Found now: {preview.sources.length === 0 ? "no emulator saves yet" : preview.sources.map((s) => `${s.name} (${mb(s.bytes)})`).join(", ")}
          {preview.gameFiles > 0 ? `, ${preview.gameFiles} save files next to your games` : ""}.
        </p>
      )}
      <div className="row wrap">
        <button className="btn" disabled={busy !== null} onClick={pick}>{dest ? "Change drive or folder" : "Choose drive or folder"}</button>
        {dest && <code className="small">{dest}</code>}
      </div>
      {dest && (
        <>
          <div className="row wrap">
            <label><input type="checkbox" checked={gameSaves} onChange={(e) => setGameSaves(e.target.checked)} /> Include save files that sit next to my games</label>
          </div>
          <div className="row wrap">
            <button className="btn primary" disabled={busy !== null} onClick={backup}>{busy === "backup" ? `Backing up… ${count} files` : "Back up now"}</button>
          </div>
          {info ? (
            <>
              <p className="muted small">Backup found here from {new Date(info.createdAt * 1000).toLocaleString()}: {info.sources.map((s) => s.name).join(", ") || "no emulator data"}{info.gameFiles ? `, ${info.gameFiles} game save files` : ""}.</p>
              <div className="row wrap">
                <label><input type="checkbox" checked={restoreLib} onChange={(e) => setRestoreLib(e.target.checked)} /> Also restore library data (favorites, playtime, collections)</label>
              </div>
              <div className="row wrap">
                <button className="btn" disabled={busy !== null} onClick={restore}>{busy === "restore" ? "Restoring…" : "Restore from this backup"}</button>
              </div>
            </>
          ) : (
            <p className="muted small">No Trayload backup on this drive yet.</p>
          )}
        </>
      )}
      {msg && <p className="small">{msg}</p>}
      {err && <p className="error small">{err}</p>}
      <p className="muted small">Not covered: Vita3K, xemu and experimental emulators. Install a game's save from its emulator if it isn't listed above.</p>
    </section>
  );
}
