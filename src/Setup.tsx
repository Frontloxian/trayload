import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SystemSetup } from "./types";

function Card({ r, onEmulators, onOpen, onRecheck }: {
  r: SystemSetup;
  onEmulators: () => void;
  onOpen: (id: string) => void;
  onRecheck: () => void;
}) {
  const pill =
    r.state === "ready" ? (
      <span className="pill green">Ready</span>
    ) : r.state === "needs" ? (
      <span className="pill amber">Needs files</span>
    ) : (
      <span className="pill muted">Not installed</span>
    );
  return (
    <li className="card">
      <div className="card-main setup-head">
        <div>
          <strong>{r.systemName}</strong>
          <div className="muted small">{r.emulator}</div>
        </div>
        <div>{pill}</div>
      </div>
      {r.intro && <p className="muted small setup-intro">{r.intro}</p>}
      <ul className="setup-items">
        {r.items.map((it) => (
          <li key={it.label}>
            <span
              className={"mark " + (it.found ? "ok" : it.required ? "bad" : "opt")}
              role="img"
              aria-label={it.found ? "Found" : it.required ? "Missing" : "Optional, not found"}
            >
              {it.found ? "✓" : it.required ? "✗" : "○"}
            </span>
            <div>
              <div>
                {it.label}
                {!it.required && <span className="muted small"> · optional</span>}
              </div>
              {it.found && it.matched && <div className="muted small">Found: {it.matched}</div>}
              {it.note && <div className="muted small">{it.note}</div>}
            </div>
          </li>
        ))}
      </ul>
      {r.folder && (
        <div className="muted small folder-path" title={r.folder}>
          {r.folder}
        </div>
      )}
      <div className="row">
        {r.installed ? (
          <>
            <button className="btn" onClick={() => onOpen(r.systemId)}>
              Open folder
            </button>
            <button className="btn" onClick={onRecheck}>
              Re-check
            </button>
          </>
        ) : r.installable ? (
          <button className="btn primary" onClick={onEmulators}>
            Install {r.emulator}
          </button>
        ) : (
          <span className="muted small">{r.emulator} can't be installed from Trayload yet.</span>
        )}
      </div>
    </li>
  );
}

export default function Setup({ onEmulators }: { onEmulators: () => void }) {
  const [rows, setRows] = useState<SystemSetup[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    invoke<SystemSetup[]>("check_setup")
      .then(setRows)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    load();
    // Re-check automatically when you come back from dropping files into a folder.
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, [load]);

  const open = (systemId: string) => {
    setError(null);
    invoke("open_setup_folder", { systemId }).catch((e) => setError(String(e)));
  };

  if (!rows) return <p className="muted">{error ?? "Checking…"}</p>;

  const needs = rows.filter((r) => r.state === "needs");
  const ready = rows.filter((r) => r.state === "ready");
  const notInstalled = rows.filter((r) => r.state === "notinstalled");
  const nothing = rows.filter((r) => r.state === "none");
  const installed = rows.filter((r) => r.installed);
  const okCount = installed.filter((r) => r.state !== "needs").length;

  const card = (r: SystemSetup) => (
    <Card key={r.systemId} r={r} onEmulators={onEmulators} onOpen={open} onRecheck={load} />
  );

  return (
    <>
      <div className="toolbar">
        <p className="setup-progress" aria-live="polite">
          {installed.length === 0
            ? "No emulators installed yet"
            : `${okCount} of ${installed.length} installed system${installed.length === 1 ? "" : "s"} ready`}
        </p>
        <span className="spacer" />
        <button className="btn" onClick={load}>
          Re-check all
        </button>
      </div>
      <p className="muted small">
        Trayload checks for the files each emulator needs, but it never provides, downloads or links BIOS files,
        firmware or keys. Use files dumped from your own console.
      </p>
      {error && <p className="error">{error}</p>}

      {needs.length > 0 && (
        <>
          <h2 className="group">Needs attention</h2>
          <ul className="cards setup-grid">{needs.map(card)}</ul>
        </>
      )}
      {ready.length > 0 && (
        <>
          <h2 className="group">Ready</h2>
          <ul className="cards setup-grid">{ready.map(card)}</ul>
        </>
      )}
      {notInstalled.length > 0 && (
        <>
          <h2 className="group">Not installed yet</h2>
          <ul className="cards setup-grid">{notInstalled.map(card)}</ul>
        </>
      )}
      {nothing.length > 0 && (
        <p className="muted small">
          <strong>No setup checks yet for:</strong> {nothing.map((r) => r.systemName).join(", ")}. These either need
          nothing or haven't been added to the checklist.
        </p>
      )}
    </>
  );
}
