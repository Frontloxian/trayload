import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { Registry, UpdateInfo } from "./types";
import Emulators from "./Emulators";
import GameDetail from "./GameDetail";
import Library from "./Library";
import MiniPlayer from "./MiniPlayer";
import News from "./News";
import Settings from "./Settings";
import Setup from "./Setup";
import { useMusic } from "./useMusic";
import "./App.css";

type Page = "home" | "library" | "emulators" | "setup" | "news" | "settings";

const NAV: { id: Page; label: string }[] = [
  { id: "home", label: "Home" },
  { id: "library", label: "Library" },
  { id: "emulators", label: "Emulators" },
  { id: "setup", label: "Setup checklist" },
  { id: "news", label: "News" },
];

export default function App() {
  const [page, setPage] = useState<Page>("home");
  const [registry, setRegistry] = useState<Registry | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const [detailId, setDetailId] = useState<number | null>(null);
  const [libKey, setLibKey] = useState(0);
  const music = useMusic();
  const [updates, setUpdates] = useState<UpdateInfo[]>([]);

  const checkUpdates = () => {
    invoke<UpdateInfo[]>("check_updates").then(setUpdates).catch(() => setUpdates([]));
  };

  // Look for newer emulator versions once at launch, quietly in the background.
  useEffect(checkUpdates, []);

  useEffect(() => {
    invoke<Registry>("get_registry").then(setRegistry).catch((e) => setError(String(e)));
  }, []);

  // Drop game files or folders anywhere in the window to import them.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter" || p.type === "over") setDragging(true);
        else if (p.type === "leave") setDragging(false);
        else if (p.type === "drop") {
          setDragging(false);
          setPage("library");
          setDetailId(null);
          invoke("import_paths", { paths: p.paths }).catch((e) => setError(String(e)));
        }
      })
      .then((f) => {
        if (cancelled) f();
        else unlisten = f;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const title = [...NAV, { id: "settings" as Page, label: "Settings" }].find((n) => n.id === page)!.label;

  return (
    <div className="shell">
      <nav className="sidebar" aria-label="Main">
        <div className="brand">Trayload</div>
        {NAV.map((n) => (
          <button key={n.id} className="nav" aria-current={page === n.id ? "page" : undefined} onClick={() => {
              setPage(n.id);
              if (detailId !== null) setLibKey((k) => k + 1);
              setDetailId(null);
            }}>
            {n.label}
            {n.id === "emulators" && updates.length > 0 && (
              <span className="badge" title="Updates ready">
                {updates.length}
              </span>
            )}
          </button>
        ))}
        <div className="spacer" />
        <MiniPlayer music={music} />
        <button className="nav" aria-current={page === "settings" ? "page" : undefined} onClick={() => setPage("settings")}>
          Settings
        </button>
      </nav>

      <main className="content">
        <header>
          <h1>{title}</h1>
        </header>

        {error && <p className="error">{error}</p>}

        {page === "emulators" ? (
          <Emulators updates={updates} onChanged={checkUpdates} />
        ) : page === "library" ? (
          <>
            {detailId !== null && (
              <GameDetail
                id={detailId}
                systems={registry?.systems ?? []}
                onBack={() => {
                  setDetailId(null);
                  setLibKey((k) => k + 1);
                }}
              />
            )}
            <div hidden={detailId !== null}>
              <Library systems={registry?.systems ?? []} refreshKey={libKey} onOpen={setDetailId} />
            </div>
          </>
        ) : page === "setup" ? (
          <Setup onEmulators={() => setPage("emulators")} />
        ) : page === "news" ? (
          <News updates={updates} onEmulators={() => setPage("emulators")} />
        ) : page === "settings" ? (
          <Settings music={music} />
        ) : (
          <p className="muted">Coming in a later milestone.</p>
        )}
      </main>

      {dragging && (
        <div className="dropzone" aria-hidden="true">
          <div>Drop games or folders to add them to your library</div>
        </div>
      )}
    </div>
  );
}
