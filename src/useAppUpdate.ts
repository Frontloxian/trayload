import { useCallback, useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type AppUpdateState = "idle" | "checking" | "none" | "found" | "installing" | "error";

const SKIP_KEY = "trayload.skipVersion";
const AUTO_KEY = "trayload.checkUpdates";

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* remembering is optional */
  }
}

export interface AppUpdate {
  state: AppUpdateState;
  version?: string;
  notes?: string;
  error?: string;
  /** The update banner is shown when a new version exists and the person hasn't dismissed it. */
  showBanner: boolean;
  startupCheck: boolean;
  setStartupCheck: (on: boolean) => void;
  check: () => Promise<void>;
  install: () => Promise<void>;
  /** Hide the banner until the next start. */
  later: () => void;
  /** Hide the banner for this version. */
  skip: () => void;
}

/** Checks github.com for a newer Trayload. Only runs at startup if the person hasn't turned that off. */
export function useAppUpdate(): AppUpdate {
  const [state, setState] = useState<AppUpdateState>("idle");
  const [version, setVersion] = useState<string>();
  const [notes, setNotes] = useState<string>();
  const [error, setError] = useState<string>();
  const [hidden, setHidden] = useState(false);
  const [skipped, setSkipped] = useState<string | null>(() => read(SKIP_KEY));
  const [startupCheck, setStartup] = useState(() => read(AUTO_KEY) !== "0");
  const found = useRef<Update | null>(null);

  const run = useCallback(async (manual: boolean) => {
    setState("checking");
    setError(undefined);
    if (manual) {
      setHidden(false);
      setSkipped(null);
      write(SKIP_KEY, "");
    }
    try {
      const u = await check();
      if (!u) {
        setState("none");
        return;
      }
      found.current = u;
      setVersion(u.version);
      setNotes(u.body ?? "");
      setState("found");
    } catch (e) {
      // A failed check at startup stays quiet. Only a manual check shows the reason.
      setState(manual ? "error" : "idle");
      if (manual) setError(String(e));
    }
  }, []);

  useEffect(() => {
    if (!startupCheck) return;
    const t = window.setTimeout(() => void run(false), 5000);
    return () => window.clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const install = useCallback(async () => {
    const u = found.current;
    if (!u) return;
    setState("installing");
    try {
      await u.downloadAndInstall();
      await relaunch();
    } catch (e) {
      setState("error");
      setError(String(e));
    }
  }, []);

  return {
    state,
    version,
    notes,
    error,
    showBanner: (state === "found" && !hidden && skipped !== version) || state === "installing",
    startupCheck,
    setStartupCheck: (on) => {
      setStartup(on);
      write(AUTO_KEY, on ? "1" : "0");
    },
    check: () => run(true),
    install,
    later: () => setHidden(true),
    skip: () => {
      if (version) {
        setSkipped(version);
        write(SKIP_KEY, version);
      }
    },
  };
}
