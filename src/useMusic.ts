import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { MusicPack } from "./types";

const KEY = "trayload.music";

export interface MusicPrefs {
  enabled: boolean;
  volume: number;
  pack: string;
  shuffle: boolean;
}

const DEFAULTS: MusicPrefs = { enabled: true, volume: 0.5, pack: "trayload", shuffle: false };

function loadPrefs(): MusicPrefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return { ...DEFAULTS, ...JSON.parse(raw) };
  } catch {
    /* use the defaults */
  }
  return DEFAULTS;
}

export interface Music {
  packs: MusicPack[];
  prefs: MusicPrefs;
  setPrefs: (p: Partial<MusicPrefs>) => void;
  playing: boolean;
  pausedByGame: boolean;
  title: string | null;
  hasTracks: boolean;
  error: string | null;
  toggle: () => void;
  next: () => void;
  prev: () => void;
  reload: () => void;
}

/** Owns the audio element. Music pauses while a game runs and resumes afterwards. */
export function useMusic(): Music {
  const [packs, setPacks] = useState<MusicPack[]>([]);
  const [prefs, setPrefsState] = useState<MusicPrefs>(loadPrefs);
  const [want, setWant] = useState(true); // the listener's play/pause choice
  const [playing, setPlaying] = useState(false);
  const [index, setIndex] = useState(0);
  const [order, setOrder] = useState<number[]>([]);
  const [loadedId, setLoadedId] = useState("");
  const [gamesRunning, setGamesRunning] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  if (!audioRef.current) audioRef.current = new Audio();
  const a = audioRef.current;

  const pack = packs.find((p) => p.id === prefs.pack) ?? packs.find((p) => p.builtin) ?? null;
  const track = pack && order.length ? pack.tracks[order[index % order.length]] : null;
  const trackId = pack && track ? `${pack.id}/${track.file}` : "";
  const pausedByGame = gamesRunning > 0;

  const setPrefs = useCallback((p: Partial<MusicPrefs>) => {
    setPrefsState((old) => {
      const n = { ...old, ...p };
      try {
        localStorage.setItem(KEY, JSON.stringify(n));
      } catch {
        /* remembering is optional */
      }
      return n;
    });
  }, []);

  const reload = useCallback(() => {
    invoke<MusicPack[]>("list_music_packs")
      .then(setPacks)
      .catch((e) => setError(String(e)));
  }, []);
  useEffect(reload, [reload]);

  // Track order: in sequence, or shuffled. Rebuilt when the pack or the shuffle choice changes.
  useEffect(() => {
    const n = pack?.tracks.length ?? 0;
    const o = Array.from({ length: n }, (_, i) => i);
    if (prefs.shuffle) {
      for (let i = n - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [o[i], o[j]] = [o[j], o[i]];
      }
    }
    setOrder(o);
    setIndex(0);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pack?.id, pack?.tracks.length, prefs.shuffle]);

  // Pause while any game runs.
  useEffect(() => {
    invoke<number[]>("running_games").then((r) => setGamesRunning(r.length)).catch(() => {});
    const s = listen("game-started", () => setGamesRunning((n) => n + 1));
    const t = listen("game-stopped", () => setGamesRunning((n) => Math.max(0, n - 1)));
    return () => {
      s.then((f) => f());
      t.then((f) => f());
    };
  }, []);

  useEffect(() => {
    a.volume = Math.min(1, Math.max(0, prefs.volume));
  }, [a, prefs.volume]);

  useEffect(() => {
    a.loop = order.length === 1;
  }, [a, order.length]);

  // Load the current track.
  useEffect(() => {
    if (!pack || !track) {
      a.pause();
      setLoadedId("");
      return;
    }
    let cancelled = false;
    invoke<string>("get_track", { pack: pack.id, file: track.file })
      .then((url) => {
        if (cancelled) return;
        a.src = url;
        setLoadedId(trackId);
        setError(null);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [trackId]);

  // Play or pause: only when music is on, wanted, no game is running, and the track is loaded.
  useEffect(() => {
    const go = prefs.enabled && want && !pausedByGame && trackId !== "" && loadedId === trackId;
    if (go) {
      a.play().then(() => setPlaying(true)).catch(() => setPlaying(false));
    } else {
      a.pause();
      setPlaying(false);
    }
  }, [a, prefs.enabled, want, pausedByGame, trackId, loadedId]);

  useEffect(() => {
    const onEnd = () => setIndex((i) => i + 1);
    const onErr = () => {
      if (a.getAttribute("src")) setError("A track could not be played.");
    };
    a.addEventListener("ended", onEnd);
    a.addEventListener("error", onErr);
    return () => {
      a.removeEventListener("ended", onEnd);
      a.removeEventListener("error", onErr);
    };
  }, [a]);

  const toggle = useCallback(() => {
    if (!prefs.enabled) {
      setPrefs({ enabled: true });
      setWant(true);
    } else {
      setWant((w) => !w);
    }
  }, [prefs.enabled, setPrefs]);

  const next = useCallback(() => setIndex((i) => i + 1), []);
  const prev = useCallback(() => setIndex((i) => Math.max(0, i - 1)), []);

  return {
    packs,
    prefs,
    setPrefs,
    playing,
    pausedByGame,
    title: track?.title ?? null,
    hasTracks: order.length > 0,
    error,
    toggle,
    next,
    prev,
    reload,
  };
}
