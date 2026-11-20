import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { EmulatorEntry, InstallProgress, UpdateInfo } from "./types";

const mb = (n: number) => (n / 1048576).toFixed(1);

export default function Emulators({ updates, onChanged }: { updates: UpdateInfo[]; onChanged: () => void }) {
  const [items, setItems] = useState<EmulatorEntry[]>([]);
  const [progress, setProgress] = useState<Record<string, InstallProgress>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [showExp, setShowExp] = useState(() => {
    try {
      return localStorage.getItem("trayload.showExperimental") === "1";
    } catch {
      return false;
    }
  });

  const toggleExp = (on: boolean) => {
    setShowExp(on);
    try {
      localStorage.setItem("trayload.showExperimental", on ? "1" : "0");
    } catch {
      /* remembering the choice is optional */
    }
  };

  const refresh = useCallback(() => {
    invoke<EmulatorEntry[]>("list_emulators")
      .then(setItems)
      .catch((e) => setErrors((p) => ({ ...p, _list: String(e) })));
  }, []);

  useEffect(() => {
    refresh();
    const un = listen<InstallProgress>("install-progress", (e) =>
      setProgress((p) => ({ ...p, [e.payload.id]: e.payload })),
    );
    return () => {
      un.then((f) => f());
    };
  }, [refresh]);

  const setErr = (id: string, msg: string | null) =>
    setErrors((p) => {
      const n = { ...p };
      if (msg) n[id] = msg;
      else delete n[id];
      return n;
    });

  async function install(entry: EmulatorEntry) {
    const id = entry.id;
    if (entry.warning && !window.confirm(`${entry.name}\n\n${entry.warning}\n\nInstall it anyway?`)) return;
    setErr(id, null);
    setProgress((p) => ({ ...p, [id]: { id, stage: "resolving", downloaded: 0, total: 0 } }));
    try {
      await invoke("install_emulator", { id });
    } catch (e) {
      setErr(id, String(e));
    } finally {
      setProgress((p) => {
        const n = { ...p };
        delete n[id];
        return n;
      });
      refresh();
      onChanged();
    }
  }

  async function launch(id: string) {
    setErr(id, null);
    try {
      await invoke("launch_emulator", { id });
    } catch (e) {
      setErr(id, String(e));
    }
  }

  async function uninstall(e: EmulatorEntry) {
    const ok = window.confirm(
      `Remove ${e.name}?\n\nThis deletes its whole folder, including any saves and settings stored there.`,
    );
    if (!ok) return;
    setErr(e.id, null);
    try {
      await invoke("uninstall_emulator", { id: e.id });
    } catch (err) {
      setErr(e.id, String(err));
    }
    refresh();
    onChanged();
  }

  return (
    <>
      {errors._list && <p className="error">{errors._list}</p>}
      <label className="toggle">
        <input type="checkbox" checked={showExp} onChange={(e) => toggleExp(e.target.checked)} /> Show experimental
        emulators
      </label>
      <ul className="cards">
        {items
          .filter((e) => e.tier !== "experimental" || showExp || e.installed)
          .map((e) => {
          const p = progress[e.id];
          const busy = !!p;
          const upd = updates.find((u) => u.id === e.id);
          const pct = p && p.total > 0 ? Math.min(100, (p.downloaded / p.total) * 100) : 0;
          return (
            <li key={e.id} className="card">
              <div className="card-main">
                <div>
                  <strong>{e.name}</strong>
                  <div className="muted small">
                    {e.systems.join(", ")}{" "}
                    {e.tier === "experimental" && <span className="pill purple">Experimental</span>}
                    {e.tier === "core" && <span className="pill muted">RetroArch core</span>}
                  </div>
                </div>
                <div className="status">
                  {busy ? (
                    <span className="pill blue">
                      {p.stage === "downloading"
                        ? `Downloading ${mb(p.downloaded)}${p.total ? " / " + mb(p.total) : ""} MB`
                        : p.stage === "extracting"
                          ? "Extracting…"
                          : "Looking up latest version…"}
                    </span>
                  ) : e.installed ? (
                    <>
                      <span className="pill green">Installed {e.installed.version}</span>
                      {upd && <span className="pill amber update-pill"> Update ready: {upd.latest}</span>}
                    </>
                  ) : e.installable ? (
                    <span className="pill muted">Not installed</span>
                  ) : (
                    <span className="pill amber">Not available yet</span>
                  )}
                </div>
                <div className="actions">
                  {e.installed && (
                    <button className="btn primary" disabled={busy} onClick={() => launch(e.id)}>
                      Launch
                    </button>
                  )}
                  {e.installable && (
                    <button className="btn" disabled={busy} onClick={() => install(e)}>
                      {upd ? "Update" : e.installed ? "Reinstall" : "Install"}
                    </button>
                  )}
                  {e.installed && (
                    <button className="btn danger" disabled={busy} onClick={() => uninstall(e)}>
                      Uninstall
                    </button>
                  )}
                </div>
              </div>
              {busy && p.stage === "downloading" && (
                <div className="bar" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
                  <div style={{ width: `${pct}%` }} />
                </div>
              )}
              {e.warning && <p className="muted small warn">{e.warning}</p>}
              {e.tier === "core" && !e.installed && (
                <p className="muted small warn">
                  Runs inside RetroArch, which is installed automatically the first time (about 240 MB). Later cores are
                  small.
                </p>
              )}
              {errors[e.id] && <p className="error small">{errors[e.id]}</p>}
            </li>
          );
        })}
      </ul>
    </>
  );
}
