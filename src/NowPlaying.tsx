import { useEffect, useRef, useState } from "react";
import type { Music } from "./useMusic";

/** A small "Now playing" note that appears for a few seconds when a new track starts. */
export default function NowPlaying({ music }: { music: Music }) {
  const [shown, setShown] = useState<string | null>(null);
  const last = useRef<string | null>(null);

  useEffect(() => {
    if (!music.playing || !music.title || music.title === last.current) return;
    last.current = music.title;
    setShown(music.title);
    const t = window.setTimeout(() => setShown(null), 4500);
    return () => window.clearTimeout(t);
  }, [music.playing, music.title]);

  if (!shown) return null;
  return (
    <div className="now-playing" role="status" aria-live="polite">
      <span className="muted small">Now playing</span>
      <strong>{shown}</strong>
    </div>
  );
}
