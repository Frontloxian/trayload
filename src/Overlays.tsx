import { useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import type { Music } from "./useMusic";

function Dialog({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>("button")?.focus();
  }, []);
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-label={title} ref={ref}
        onKeyDown={(e) => e.key === "Escape" && onClose()}>
        <h2>{title}</h2>
        {children}
      </div>
    </div>
  );
}

export function Welcome({ onClose, onGo }: { onClose: () => void; onGo: (p: "emulators" | "setup" | "library") => void }) {
  const folder = async () => {
    const dir = await open({ directory: true });
    if (typeof dir === "string") await invoke("add_rom_folder", { path: dir });
    onClose();
  };
  return (
    <Dialog title="Welcome to Trayload" onClose={onClose}>
      <p className="muted">Three steps and you're playing. You can do them in any order, and the Home screen keeps track.</p>
      <ol className="steps">
        <li><strong>Add your games.</strong> Drop files anywhere in this window, or pick a folder to scan.</li>
        <li><strong>Install an emulator.</strong> One click each, always from the official source.</li>
        <li><strong>Check setup files.</strong> A few consoles need files from your own console. Trayload never provides or links them.</li>
      </ol>
      <div className="row wrap">
        <button className="btn primary" onClick={folder}>Pick a games folder</button>
        <button className="btn" onClick={() => { onGo("emulators"); onClose(); }}>Install emulators</button>
        <button className="btn" onClick={onClose}>Later</button>
      </div>
    </Dialog>
  );
}

const KEYS: [string, string][] = [
  ["Ctrl + 1 to 5", "Home, Library, Emulators, Setup, News"],
  ["Ctrl + ,", "Settings"],
  ["/", "Search the library"],
  ["Alt + R", "Rescan the library"],
  ["Ctrl + M", "Play or pause music"],
  ["Esc", "Go back"],
  ["?", "Show this list"],
  ["Controller", "D-pad or left stick moves, A selects, B goes back, LB and RB switch tabs"],
];

export function ShortcutHelp({ onClose }: { onClose: () => void }) {
  return (
    <Dialog title="Keyboard and controller" onClose={onClose}>
      <dl className="keys">
        {KEYS.map(([k, v]) => (
          <div key={k}><dt>{k}</dt><dd className="muted">{v}</dd></div>
        ))}
      </dl>
      <button className="btn" onClick={onClose}>Close</button>
    </Dialog>
  );
}

export function QuickSettings({ music, onClose, onGo, onRescan, onHelp }: {
  music: Music; onClose: () => void; onGo: (p: "settings") => void; onRescan: () => void; onHelp: () => void;
}) {
  return (
    <Dialog title="Quick settings" onClose={onClose}>
      <div className="row wrap">
        <label><input type="checkbox" checked={music.prefs.enabled} onChange={(e) => music.setPrefs({ enabled: e.target.checked })} /> Play music</label>
        <label><input type="checkbox" checked={music.prefs.shuffle} onChange={(e) => music.setPrefs({ shuffle: e.target.checked })} /> Shuffle</label>
      </div>
      <div className="row">
        <input className="vol" type="range" min={0} max={100} aria-label="Music volume" value={Math.round(music.prefs.volume * 100)}
          onChange={(e) => music.setPrefs({ volume: Number(e.target.value) / 100 })} />
      </div>
      <div className="row wrap">
        <button className="btn" onClick={() => { onRescan(); onClose(); }}>Rescan library</button>
        <button className="btn" onClick={() => { onClose(); onHelp(); }}>Keyboard and controller</button>
        <button className="btn" onClick={() => { onGo("settings"); onClose(); }}>All settings</button>
        <button className="btn" onClick={onClose}>Close</button>
      </div>
    </Dialog>
  );
}
