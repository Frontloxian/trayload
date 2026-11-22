import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Dialog } from "./Overlays";
import type { EmulatorEntry, EmulatorSize, InstallProgress, UpdateInfo } from "./types";

const mb = (n: number) => (n / 1048576).toFixed(1);
const nice = (n: number) => (n >= 1073741824 ? `${(n / 1073741824).toFixed(1)} GB` : `${Math.max(1, Math.round(n / 1048576))} MB`);

interface BulkRun {
  names: string[];
  index: number;
  current: string;
}
interface BulkResult {
  installed: string[];
  failed: { name: string; error: string }[];
  cancelled: boolean;
  notStarted: number;
}

export default function Emulators({ updates, onChanged }: { updates: UpdateInfo[]; onChanged: () => void }) {
  const [items, setItems] = useState<EmulatorEntry[]>([]);
  const [progress, setProgress] = useState<Record<string, InstallProgress>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  // Install all: the list to confirm, the run in progress, and the summary.
  const [pick, setPick] = useState<{ ids: Set<string>; sizes: Record<string, number | null> | null } | null>(null);
  const [bulk, setBulk] = useState<BulkRun | null>(null);
  const [result, setResult] = useState<BulkResult | null>(null);
  const cancelBulk = useRef(false);
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
    if (entry.warning && !window.confirm(`${entry.name}\n\n${entry.warning}\n\nInstall it anyway?`)) return;
    await installOne(entry);
  }

  /** Installs one emulator and returns an error message, or null when it worked. */
  async function installOne(entry: EmulatorEntry): Promise<string | null> {
    const id = entry.id;
    let failure: string | null = null;
    setErr(id, null);
    setProgress((p) => ({ ...p, [id]: { id, stage: "resolving", downloaded: 0, total: 0 } }));
    try {
      await invoke("install_emulator", { id });
    } catch (e) {
      failure = String(e);
      setErr(id, failure);
    } finally {
      setProgress((p) => {
        const n = { ...p };
        delete n[id];
        return n;
      });
      refresh();
      onChanged();
    }
    return failure;
  }

  const candidates = items.filter((e) => e.installable && !e.installed);

  function openInstallAll() {
    const ids = new Set(candidates.filter((e) => e.tier !== "experimental").map((e) => e.id));
    setPick({ ids, sizes: null });
    invoke<EmulatorSize[]>("emulator_sizes")
      .then((list) =>
        setPick((p) => (p ? { ...p, sizes: Object.fromEntries(list.map((s) => [s.id, s.bytes])) } : p)),
      )
      .catch(() => setPick((p) => (p ? { ...p, sizes: {} } : p)));
  }

  async function runInstallAll() {
    if (!pick) return;
    const chosen = candidates.filter((e) => pick.ids.has(e.id));
    setPick(null);
    if (chosen.length === 0) return;
    cancelBulk.current = false;
    const res: BulkResult = { installed: [], failed: [], cancelled: false, notStarted: 0 };
    for (let i = 0; i < chosen.length; i++) {
      if (cancelBulk.current) {
        res.cancelled = true;
        res.notStarted = chosen.length - i;
        break;
      }
      setBulk({ names: chosen.map((c) => c.name), index: i, current: chosen[i].name });
      const err = await installOne(chosen[i]);
      if (err) res.failed.push({ name: chosen[i].name, error: err });
      else res.installed.push(chosen[i].name);
    }
    setBulk(null);
    setResult(res);
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
      {bulk && (
        <div className="card" role="status">
          <strong>
            Installing {bulk.index + 1} of {bulk.names.length}: {bulk.current}
          </strong>
          <div className="bar" role="progressbar" aria-valuenow={bulk.index} aria-valuemin={0} aria-valuemax={bulk.names.length}>
            <div style={{ width: `${(bulk.index / bulk.names.length) * 100}%` }} />
          </div>
          <div className="row">
            <button className="btn" onClick={() => (cancelBulk.current = true)}>
              Cancel
            </button>
            <span className="muted small">Cancel stops after the download that is running now.</span>
          </div>
        </div>
      )}
      <div className="row">
        <button className="btn primary" disabled={!!bulk || candidates.length === 0} onClick={openInstallAll}>
          Install all…
        </button>
        {candidates.length === 0 && items.length > 0 && <span className="muted small">Everything is installed.</span>}
      </div>
      <label className="toggle">
        <input type="checkbox" checked={showExp} onChange={(e) => toggleExp(e.target.checked)} /> Show experimental
        emulators
      </label>
      <ul className="cards">
        {items
          .filter((e) => e.tier !== "experimental" || showExp || e.installed || e.leftover)
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
                  ) : e.leftover ? (
                    <span className="pill amber">Incomplete install</span>
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
                  {(e.installed || e.leftover) && (
                    <button className="btn danger" disabled={busy} onClick={() => uninstall(e)}>
                      {e.installed ? "Uninstall" : "Remove files"}
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
              {errors[e.id] && <p className="error small">{errors[e.id]}</p>}
            </li>
          );
        })}
      </ul>
      {pick && (
        <Dialog title="Install all emulators" onClose={() => setPick(null)} wide>
          <p className="muted">
            These come from each emulator's official release page, one after another. Tick the ones you want.
            Experimental emulators are left unticked.
          </p>
          <ul className="plain pick-list">
            {candidates.map((e) => {
              const size = pick.sizes?.[e.id];
              return (
                <li key={e.id}>
                  <label>
                    <input
                      type="checkbox"
                      checked={pick.ids.has(e.id)}
                      onChange={(ev) =>
                        setPick((p) => {
                          if (!p) return p;
                          const ids = new Set(p.ids);
                          if (ev.target.checked) ids.add(e.id);
                          else ids.delete(e.id);
                          return { ...p, ids };
                        })
                      }
                    />{" "}
                    <strong>{e.name}</strong>{" "}
                    {e.tier === "experimental" && <span className="pill purple">Experimental</span>}{" "}
                    <span className="muted small">
                      {pick.sizes === null ? "checking size…" : size ? nice(size) : "size unknown"}
                    </span>
                  </label>
                  <div className="muted small">{e.systems.join(", ")}</div>
                  {e.warning && <div className="muted small warn">{e.warning}</div>}
                </li>
              );
            })}
          </ul>
          <p className="muted small">
            {(() => {
              const chosen = candidates.filter((e) => pick.ids.has(e.id));
              const known = chosen.reduce((n, e) => n + (pick.sizes?.[e.id] ?? 0), 0);
              const unknown = chosen.filter((e) => pick.sizes && !pick.sizes[e.id]).length;
              return `${chosen.length} selected${known ? ` · about ${nice(known)}` : ""}${unknown ? ` plus ${unknown} of unknown size` : ""}`;
            })()}
          </p>
          <div className="row">
            <button className="btn primary" disabled={pick.ids.size === 0} onClick={() => void runInstallAll()}>
              Install {pick.ids.size}
            </button>
            <button className="btn" onClick={() => setPick(null)}>
              Cancel
            </button>
          </div>
        </Dialog>
      )}
      {result && (
        <Dialog title="Install all: summary" onClose={() => setResult(null)}>
          <p>
            Installed {result.installed.length}
            {result.failed.length > 0 && `, ${result.failed.length} failed`}
            {result.cancelled && `, cancelled with ${result.notStarted} not started`}.
          </p>
          {result.failed.map((f) => (
            <p key={f.name} className="error small">
              <strong>{f.name}:</strong> {f.error}
            </p>
          ))}
          <p className="muted small">Emulators that failed can be installed again from their own card.</p>
          <div className="row">
            <button className="btn primary" onClick={() => setResult(null)}>
              Done
            </button>
          </div>
        </Dialog>
      )}
    </>
  );
}
