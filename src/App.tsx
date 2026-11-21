import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { AppInfo, LegacyStatus, Registry, ScanSummary, System, UpdateInfo } from "./types";
import CopyStatus from "./CopyStatus";
import LegacyCleanup from "./LegacyCleanup";
import Emulators from "./Emulators";
import ErrorBoundary from "./ErrorBoundary";
import Home from "./Home";
import GameDetail from "./GameDetail";
import Library from "./Library";
import MiniPlayer from "./MiniPlayer";
import News from "./News";
import Settings from "./Settings";
import Setup from "./Setup";
import { Dialog, QuickSettings, ShortcutHelp, Welcome } from "./Overlays";
import { useGamepad, useShortcuts } from "./useInput";
import { useMusic } from "./useMusic";
import { useAppUpdate } from "./useAppUpdate";
import { Notes, WhatsNew, notesFor } from "./Notes";
import { Icon, type IconName } from "./icons";
import { listen } from "@tauri-apps/api/event";
import logo from "./assets/trayload-logo-small.svg";
import "./App.css";

type Page = "home" | "library" | "emulators" | "setup" | "news" | "settings";

const NAV: { id: Page; label: string; icon: IconName; sub: string }[] = [
  { id: "home", label: "Home", icon: "home", sub: "Pick up where you left off" },
  { id: "library", label: "Library", icon: "library", sub: "Your games, by system" },
  { id: "emulators", label: "Emulators", icon: "emulators", sub: "Install and update emulators from their official sources" },
  { id: "setup", label: "Setup checklist", icon: "setup", sub: "The files each system needs, checked for you" },
  { id: "news", label: "News", icon: "news", sub: "Releases and updates from the emulation world" },
];
const SETTINGS_SUB = "Artwork, music, folders, backups and updates";

export default function App() {
  const [page, setPage] = useState<Page>("home");
  const [registry, setRegistry] = useState<Registry | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const [detailId, setDetailId] = useState<number | null>(null);
  const [libKey, setLibKey] = useState(0);
  const music = useMusic();
  const appUpdate = useAppUpdate();
  const [version, setVersion] = useState("");
  const [showNotes, setShowNotes] = useState(false);
  const [showUpdateNotes, setShowUpdateNotes] = useState(false);
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
  const [legacy, setLegacy] = useState<LegacyStatus | null>(null);

  // Retired systems keep their names so games already in the library still read properly.
  const allSystems = useMemo<System[]>(() => {
    const active = registry?.systems ?? [];
    const retired = (registry?.retired ?? []).map(
      (r): System => ({ id: r.id, name: r.name, emulator: "", source: { type: "direct", url: "" }, exts: [] }),
    );
    return [...active, ...retired];
  }, [registry]);

  // Offer once (per launch until dismissed) to clean up the RetroArch folder older versions made.
  useEffect(() => {
    let dismissed = false;
    try {
      dismissed = localStorage.getItem("trayload.legacyDismissed") === "1";
    } catch {
      /* ignore */
    }
    if (dismissed) return;
    invoke<LegacyStatus>("legacy_status")
      .then((s) => s.present && setLegacy(s))
      .catch(() => {});
  }, []);

  // After an update, show what's new once. A fresh install sees the welcome dialog instead.
  useEffect(() => {
    invoke<AppInfo>("get_app_info")
      .then((i) => {
        setVersion(i.version);
        let last: string | null = null;
        let welcomed = false;
        try {
          last = localStorage.getItem("trayload.lastSeenVersion");
          welcomed = localStorage.getItem("trayload.welcomed") === "1";
        } catch {
          /* nothing remembered */
        }
        if (last !== i.version && (last !== null || welcomed) && notesFor(i.version)) setShowNotes(true);
        try {
          localStorage.setItem("trayload.lastSeenVersion", i.version);
        } catch {
          /* optional */
        }
      })
      .catch(() => {});
  }, []);

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

  const current = NAV.find((n) => n.id === page);
  const title = current?.label ?? "Settings";
  const subtitle = current?.sub ?? SETTINGS_SUB;

  return (
    <div className="shell">
      <nav className="sidebar" aria-label="Main">
        <div className="brand">
          <img src={logo} alt="" width={30} height={30} />
          <span className="brand-name">Trayload</span>
          {version && <span className="brand-version">v{version}</span>}
        </div>
        {NAV.map((n) => (
          <button key={n.id} className="nav" aria-current={page === n.id ? "page" : undefined} onClick={() => {
              setPage(n.id);
              if (detailId !== null) setLibKey((k) => k + 1);
              setDetailId(null);
            }}>
            <Icon name={n.icon} />
            <span className="nav-label">{n.label}</span>
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
          <Icon name="quick" />
          <span className="nav-label">Quick settings</span>
        </button>
        <button className="nav" aria-current={page === "settings" ? "page" : undefined} onClick={() => setPage("settings")}>
          <Icon name="settings" />
          <span className="nav-label">Settings</span>
        </button>
      </nav>

      <main className="content">
        <header className="page-head">
          <h1>{title}</h1>
          <p className="page-sub">{subtitle}</p>
        </header>

        {appUpdate.showBanner && (
          <div className="update-banner" role="status">
            <div className="update-text">
              <strong>
                {appUpdate.state === "installing"
                  ? "Installing the update…"
                  : `Trayload ${appUpdate.version} is available`}
              </strong>
              <span className="muted small">
                {appUpdate.state === "installing"
                  ? " Trayload will restart when it is done."
                  : " You are on " + (version || "an older version") + "."}
              </span>
            </div>
            {appUpdate.state !== "installing" && (
              <div className="row">
                {appUpdate.notes && (
                  <button className="btn" onClick={() => setShowUpdateNotes(true)}>
                    What's new
                  </button>
                )}
                <button className="btn primary" onClick={() => void appUpdate.install()}>
                  Install and restart
                </button>
                <button className="btn" onClick={appUpdate.later}>
                  Later
                </button>
                <button className="link small" onClick={appUpdate.skip}>
                  Skip this version
                </button>
              </div>
            )}
          </div>
        )}

        {error && <p className="error">{error}</p>}

        <ErrorBoundary resetKey={page}>
        {page === "home" ? (
          <Home
            systems={allSystems}
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
                systems={allSystems}
                onBack={() => {
                  setDetailId(null);
                  setLibKey((k) => k + 1);
                }}
              />
            )}
            <div hidden={detailId !== null}>
              <Library systems={allSystems} refreshKey={libKey} onOpen={setDetailId} />
            </div>
          </>
        ) : page === "setup" ? (
          <Setup onEmulators={() => setPage("emulators")} />
        ) : page === "news" ? (
          <News updates={updates} onEmulators={() => setPage("emulators")} />
        ) : page === "settings" ? (
          <Settings music={music} appUpdate={appUpdate} onWhatsNew={() => setShowNotes(true)} />
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
      {showNotes && version && <WhatsNew version={version} onClose={() => setShowNotes(false)} />}
      {showUpdateNotes && (
        <Dialog title={`Trayload ${appUpdate.version}`} onClose={() => setShowUpdateNotes(false)} wide>
          <Notes markdown={appUpdate.notes ?? ""} />
          <div className="row">
            <button className="btn primary" onClick={() => setShowUpdateNotes(false)}>
              Close
            </button>
          </div>
        </Dialog>
      )}
      {legacy && (
        <Dialog
          title="RetroArch is no longer used"
          onClose={() => {
            try {
              localStorage.setItem("trayload.legacyDismissed", "1");
            } catch {
              /* ignore */
            }
            setLegacy(null);
          }}
          wide
        >
          <LegacyCleanup status={legacy} onChanged={() => setLegacy(null)} />
          <div className="row">
            <button
              className="btn"
              onClick={() => {
                try {
                  localStorage.setItem("trayload.legacyDismissed", "1");
                } catch {
                  /* ignore */
                }
                setLegacy(null);
              }}
            >
              Decide later (also in Settings)
            </button>
          </div>
        </Dialog>
      )}
      <CopyStatus />
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
