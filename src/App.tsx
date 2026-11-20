import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { Registry, ScanSummary, UpdateInfo } from "./types";
import Emulators from "./Emulators";
import ErrorBoundary from "./ErrorBoundary";
import Home from "./Home";
import GameDetail from "./GameDetail";
import Library from "./Library";
import MiniPlayer from "./MiniPlayer";
import News from "./News";
import Settings from "./Settings";
import Setup from "./Setup";
import { QuickSettings, ShortcutHelp, Welcome } from "./Overlays";
import { useGamepad, useShortcuts } from "./useInput";
import { useMusic } from "./useMusic";
import { listen } from "@tauri-apps/api/event";
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
  const [welcome, setWelcome] = useState(() => {
    try {
      return localStorage.getItem("trayload.welcomed") !== "1";
    } catch {
      return false;
    }
  });
  const [help, setHelp] = useState(false);
  const [quick, setQuick] = useState(false);
  const [scan, setScan] = useState<ScanSummary | null>(null);

  const closeWelcome = () => {
    setWelcome(false);
    try {
      localStorage.setItem("trayload.welcomed", "1");
    } catch {
      /* shown again next time */
    }
  };

  useEffect(() => {
    const load = () => invoke<ScanSummary | null>("last_scan").then(setScan).catch(() => {});
    load();
    const p = listen("library-changed", load);
    return () => void p.then((f) => f());
  }, []);

  const goPage = (p: Page) => {
    setPage(p);
    if (detailId !== null) setLibKey((k) => k + 1);
    setDetailId(null);
  };
  const rescan = () => {
    invoke("rescan").catch((e) => setError(String(e)));
    setLibKey((k) => k + 1);
  };
  const back = () => {
    if (detailId !== null) {
      setDetailId(null);
      setLibKey((k) => k + 1);
    } else if (page !== "home") setPage("home");
  };
  const ids: Page[] = ["home", "library", "emulators", "setup", "news"];

  useShortcuts({ go: goPage, rescan, back, help: () => setHelp(true), toggleMusic: music.toggle });
  // Controller navigation is off while a game is running, so the launcher never reacts to game input.
  useGamepad(!music.pausedByGame, {
    back,
    tab: (d) => goPage(ids[(Math.max(0, ids.indexOf(page)) + d + ids.length) % ids.length]),
  });

  const checkUpdates = () => {
    invoke<UpdateInfo[]>("check_updates").then(setUpdates).catch(() => setUpdates([]));
  };

  // Look for newer emulator versions once at launch, quietly in the background.
  useEffect(checkUpdates, []);

  // Optional: install updates on their own (Settings, "Update emulators automatically").
  const autoTried = useRef<Set<string>>(new Set());
  useEffect(() => {
    let on = false;
    try {
      on = localStorage.getItem("trayload.autoUpdate") === "1";
    } catch {
      /* off */
    }
    if (!on || updates.length === 0) return;
    const todo = updates.filter((u) => !autoTried.current.has(u.id + u.latest));
    if (todo.length === 0) return;
    todo.forEach((u) => autoTried.current.add(u.id + u.latest));
    (async () => {
      for (const u of todo) {
        try {
          await invoke("install_emulator", { id: u.id });
        } catch (e) {
          invoke("log_frontend_error", { message: `auto-update of ${u.id} failed: ${e}` }).catch(() => {});
        }
      }
      checkUpdates();
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [updates]);

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
        {scan && (
          <div className="muted small scan-note">
            Library scanned at launch. {scan.added} new, {scan.missing} missing.
          </div>
        )}
        <button className="nav" onClick={() => setQuick(true)}>
          Quick settings
        </button>
        <button className="nav" aria-current={page === "settings" ? "page" : undefined} onClick={() => setPage("settings")}>
          Settings
        </button>
      </nav>

      <main className="content">
        <header>
          <h1>{title}</h1>
        </header>

        {error && <p className="error">{error}</p>}

        <ErrorBoundary resetKey={page}>
        {page === "home" ? (
          <Home
            systems={registry?.systems ?? []}
            updates={updates}
            onOpenGame={(id) => {
              setPage("library");
              setDetailId(id);
            }}
            onGo={setPage}
            onUpdated={checkUpdates}
          />
        ) : page === "emulators" ? (
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
        ) : null}
        </ErrorBoundary>
      </main>

      {welcome && (
        <Welcome
          onClose={closeWelcome}
          onGo={(p) => {
            setPage(p);
          }}
        />
      )}
      {help && <ShortcutHelp onClose={() => setHelp(false)} />}
      {quick && (
        <QuickSettings music={music} onClose={() => setQuick(false)} onGo={setPage} onRescan={rescan} onHelp={() => setHelp(true)} />
      )}

      {dragging && (
        <div className="dropzone" aria-hidden="true">
          <div>Drop games or folders to add them to your library</div>
        </div>
      )}
    </div>
  );
}
