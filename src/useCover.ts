import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Covers are loaded once per cover file and kept in memory for the rest of the session.
const cache = new Map<string, string>();
const pending = new Map<string, Promise<string>>();

/** The cover image of a game as a data URL, or null while loading or when there is none. */
export function useCover(id: number, artPath: string | null): string | null {
  const [src, setSrc] = useState<string | null>(artPath ? (cache.get(artPath) ?? null) : null);

  useEffect(() => {
    if (!artPath) {
      setSrc(null);
      return;
    }
    const hit = cache.get(artPath);
    if (hit) {
      setSrc(hit);
      return;
    }
    let live = true;
    let p = pending.get(artPath);
    if (!p) {
      p = invoke<string>("get_cover", { id })
        .then((url) => {
          cache.set(artPath, url);
          pending.delete(artPath);
          return url;
        })
        .catch((e) => {
          pending.delete(artPath);
          throw e;
        });
      pending.set(artPath, p);
    }
    p.then((url) => live && setSrc(url)).catch(() => live && setSrc(null));
    return () => {
      live = false;
    };
  }, [id, artPath]);

  return src;
}
