import { useEffect, useRef } from "react";

export interface Shortcuts {
  go: (page: "home" | "library" | "emulators" | "setup" | "news" | "settings") => void;
  rescan: () => void;
  back: () => void;
  help: () => void;
  toggleMusic: () => void;
}

const PAGES = ["home", "library", "emulators", "setup", "news"] as const;

function typing(el: EventTarget | null): boolean {
  const t = el as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable) &&
    !(t instanceof HTMLInputElement && ["checkbox", "radio", "range", "button"].includes(t.type));
}

/** Keyboard shortcuts for the whole app. */
export function useShortcuts(s: Shortcuts) {
  const ref = useRef(s);
  ref.current = s;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const c = ref.current;
      if (e.key === "Escape") {
        if (!typing(e.target)) c.back();
        return;
      }
      if (typing(e.target)) return;
      if ((e.ctrlKey || e.metaKey) && /^[1-5]$/.test(e.key)) {
        e.preventDefault();
        c.go(PAGES[Number(e.key) - 1]);
      } else if ((e.ctrlKey || e.metaKey) && e.key === ",") {
        e.preventDefault();
        c.go("settings");
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "m") {
        e.preventDefault();
        c.toggleMusic();
      } else if (e.altKey && e.key.toLowerCase() === "r") {
        e.preventDefault();
        c.rescan();
      } else if (e.key === "/" && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        c.go("library");
        setTimeout(() => document.querySelector<HTMLInputElement>("input.search")?.focus(), 60);
      } else if (e.key === "?") {
        c.help();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

const FOCUSABLE = 'button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

function visible(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && !el.closest("[hidden]") && getComputedStyle(el).visibility !== "hidden";
}

function move(dir: "left" | "right" | "up" | "down") {
  const items = Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(visible);
  if (items.length === 0) return;
  const cur = document.activeElement as HTMLElement | null;
  if (!cur || cur === document.body || !items.includes(cur)) {
    items[0].focus();
    return;
  }
  const a = cur.getBoundingClientRect();
  const [ax, ay] = [a.left + a.width / 2, a.top + a.height / 2];
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of items) {
    if (el === cur) continue;
    const r = el.getBoundingClientRect();
    const [dx, dy] = [r.left + r.width / 2 - ax, r.top + r.height / 2 - ay];
    const [along, across] = dir === "left" ? [-dx, Math.abs(dy)] : dir === "right" ? [dx, Math.abs(dy)] : dir === "up" ? [-dy, Math.abs(dx)] : [dy, Math.abs(dx)];
    if (along <= 4) continue;
    const score = along + across * 2.2;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  if (best) {
    best.focus();
    best.scrollIntoView({ block: "nearest", inline: "nearest" });
  }
}

/** Basic controller navigation: D-pad or left stick moves, A selects, B goes back, shoulders switch tabs. */
export function useGamepad(enabled: boolean, actions: { back: () => void; tab: (delta: number) => void }) {
  const act = useRef(actions);
  act.current = actions;
  useEffect(() => {
    if (!enabled) return;
    let raf = 0;
    const down = new Set<string>();
    let repeatAt = 0;
    const loop = () => {
      raf = requestAnimationFrame(loop);
      if (!document.hasFocus()) return;
      const pad = Array.from(navigator.getGamepads?.() ?? []).find((p) => p && p.connected);
      if (!pad) return;
      const now = performance.now();
      const b = (i: number) => !!pad.buttons[i]?.pressed;
      const [x, y] = [pad.axes[0] ?? 0, pad.axes[1] ?? 0];
      const state: Record<string, boolean> = {
        up: b(12) || y < -0.6, down: b(13) || y > 0.6, left: b(14) || x < -0.6, right: b(15) || x > 0.6,
        a: b(0), back: b(1), lb: b(4), rb: b(5),
      };
      for (const k of Object.keys(state)) {
        const isDir = k === "up" || k === "down" || k === "left" || k === "right";
        if (!state[k]) {
          down.delete(k);
          continue;
        }
        const fresh = !down.has(k);
        if (!fresh && !(isDir && now > repeatAt)) continue;
        down.add(k);
        if (isDir) repeatAt = now + (fresh ? 380 : 120);
        document.body.classList.add("pad");
        if (isDir) move(k as "up");
        else if (k === "a") (document.activeElement as HTMLElement | null)?.click();
        else if (k === "back") act.current.back();
        else if (k === "lb") act.current.tab(-1);
        else if (k === "rb") act.current.tab(1);
      }
    };
    raf = requestAnimationFrame(loop);
    const off = () => document.body.classList.remove("pad");
    window.addEventListener("mousemove", off);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("mousemove", off);
    };
  }, [enabled]);
}
