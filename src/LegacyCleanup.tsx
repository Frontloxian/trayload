import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { LegacyStatus } from "./types";

function size(b: number): string {
  return b >= 1073741824 ? `${(b / 1073741824).toFixed(1)} GB` : `${Math.max(1, Math.round(b / 1048576))} MB`;
}

const LABELS: Record<string, string> = { saves: "Game saves", states: "Save states", system: "BIOS and system files" };

/** Backs up and removes the RetroArch folder that older Trayload versions installed. Never acts without being asked. */
export default function LegacyCleanup({ status, onChanged }: { status: LegacyStatus; onChanged: () => void }) {
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [backedUp, setBackedUp] = useState(false);
  const [confirm, setConfirm] = useState(false);

  useEffect(() => setConfirm(false), [status.path]);

  async function backup() {
    setError(null);
    const dir = await open({ directory: true, title: "Where should the backup go?" });
    if (typeof dir !== "string") return;
    setBusy(true);
    try {
      const out = await invoke<string>("legacy_backup", { dest: dir });
      setBackedUp(true);
      setNote(`Backed up to ${out}`);
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }

  async function remove() {
    setBusy(true);
    setError(null);
    try {
      await invoke("legacy_remove");
      setNote("The old RetroArch folder was removed.");
      onChanged();
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }

  return (
    <div>
      <p className="muted">
        Trayload no longer uses RetroArch. Your games stay in the library. Sega consoles now use their own emulators
        (ares and Ymir), and other consoles RetroArch used to run will come back through community addons.
      </p>
      <p className="muted">
        The old RetroArch folder still takes up {size(status.totalBytes)}.
        {status.folders.length > 0 && " It holds files that are yours:"}
      </p>
      {status.folders.length > 0 && (
        <ul className="plain">
          {status.folders.map((f) => (
            <li key={f.name}>
              {LABELS[f.name] ?? f.name} <span className="muted small">({size(f.bytes)})</span>
            </li>
          ))}
        </ul>
      )}
      <div className="row wrap">
        {status.folders.length > 0 && (
          <button className="btn" disabled={busy} onClick={() => void backup()}>
            Back up saves and BIOS files…
          </button>
        )}
        {!confirm ? (
          <button className="btn danger" disabled={busy} onClick={() => setConfirm(true)}>
            Remove RetroArch…
          </button>
        ) : (
          <button className="btn danger" disabled={busy} onClick={() => void remove()}>
            {backedUp || status.folders.length === 0
              ? "Yes, delete the RetroArch folder"
              : "Delete it without a backup"}
          </button>
        )}
      </div>
      {confirm && (
        <p className="muted small">
          This permanently deletes {status.path}, including its saves and BIOS files. Click the button again to
          confirm.
        </p>
      )}
      {note && <p className="muted small">{note}</p>}
      {error && <p className="error small">{error}</p>}
    </div>
  );
}
