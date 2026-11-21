import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CopyFinished, CopyProgress } from "./types";

function size(b: number): string {
  return b >= 1073741824 ? `${(b / 1073741824).toFixed(1)} GB` : `${Math.round(b / 1048576)} MB`;
}

/** Shows the progress of copying dropped games into the games folder, with a Cancel button, then a short result. */
export default function CopyStatus() {
  const [prog, setProg] = useState<CopyProgress | null>(null);
  const [done, setDone] = useState<CopyFinished | null>(null);

  useEffect(() => {
    const subs = [
      listen<CopyProgress>("copy-progress", (e) => {
        setDone(null);
        setProg(e.payload);
      }),
      listen<CopyFinished>("copy-finished", (e) => {
        setProg(null);
        setDone(e.payload);
      }),
    ];
    return () => {
      subs.forEach((p) => void p.then((f) => f()));
    };
  }, []);

  if (prog) {
    const pct = prog.bytesTotal > 0 ? Math.min(100, (prog.bytesDone / prog.bytesTotal) * 100) : 0;
    return (
      <div className="copy-bar" role="status">
        <div className="copy-text">
          <strong>Copying {prog.name || "games"}</strong>
          <span className="muted small">
            {" "}
            {prog.filesDone} of {prog.filesTotal} files · {size(prog.bytesDone)} of {size(prog.bytesTotal)}
          </span>
        </div>
        <div className="bar" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
          <div style={{ width: `${pct}%` }} />
        </div>
        <button className="btn" onClick={() => void invoke("cancel_copy")}>
          Cancel
        </button>
      </div>
    );
  }
  if (done) {
    const parts: string[] = [];
    if (done.copied) parts.push(`Copied ${done.copied} game${done.copied === 1 ? "" : "s"} into your games folder.`);
    if (done.skipped) parts.push(`${done.skipped} already there.`);
    if (done.cancelled) parts.push("Copying was cancelled. Games already copied were kept.");
    return (
      <div className="copy-bar" role="status">
        <div className="copy-text">
          <span>{parts.join(" ") || "Nothing was copied."}</span>
          {done.failed.map((f) => (
            <div key={f} className="error small">
              {f}
            </div>
          ))}
        </div>
        <button className="btn" onClick={() => setDone(null)}>
          OK
        </button>
      </div>
    );
  }
  return null;
}
