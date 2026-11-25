import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import type { Registry, UpdateInfo } from "./types";

const KEY = "trayload.notify";
const SEEN = "trayload.notified";

export function notifyEnabled(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

export function setNotifyEnabled(on: boolean) {
  try {
    localStorage.setItem(KEY, on ? "1" : "0");
  } catch {
    /* remembering is optional */
  }
}

/** Asks Windows for permission when needed. Returns whether notifications can be shown. */
export async function ensurePermission(): Promise<boolean> {
  try {
    if (await isPermissionGranted()) return true;
    return (await requestPermission()) === "granted";
  } catch {
    return false;
  }
}

const norm = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");

/** One desktop notification for emulator updates that were not announced before. Only while Trayload is open. */
export async function notifyUpdates(updates: UpdateInfo[], registry: Registry | null) {
  if (!notifyEnabled() || updates.length === 0) return;
  let seen: string[] = [];
  try {
    seen = JSON.parse(localStorage.getItem(SEEN) ?? "[]");
  } catch {
    seen = [];
  }
  const fresh = updates.filter((u) => !seen.includes(`${u.id}@${u.latest}`));
  if (fresh.length === 0) return;
  if (!(await ensurePermission())) return;
  const names = fresh.map(
    (u) => registry?.systems.find((s) => norm(s.emulator) === norm(u.id))?.emulator ?? u.id,
  );
  const unique = [...new Set(names)];
  try {
    sendNotification({
      title: unique.length === 1 ? `${unique[0]} has an update` : `${unique.length} emulator updates are ready`,
      body: unique.length === 1 ? "Open Trayload and go to Emulators to install it." : unique.join(", "),
    });
    localStorage.setItem(SEEN, JSON.stringify([...seen, ...fresh.map((u) => `${u.id}@${u.latest}`)].slice(-200)));
  } catch {
    /* a missed notification is not worth an error */
  }
}
