import type { Music } from "./useMusic";

export default function MiniPlayer({ music }: { music: Music }) {
  if (!music.prefs.enabled && !music.hasTracks) return null;
  const status = !music.hasTracks
    ? "No music yet. Add some in Settings."
    : music.pausedByGame
      ? "Paused while a game runs"
      : music.title ?? "Music";
  return (
    <div className="mini" role="group" aria-label="Music">
      <div className="mini-title" title={status}>
        {status}
      </div>
      <div className="mini-controls">
        <button className="icon-btn" aria-label="Previous track" disabled={!music.hasTracks} onClick={music.prev}>
          ⏮
        </button>
        <button
          className="icon-btn"
          aria-label={music.playing ? "Pause music" : "Play music"}
          disabled={!music.hasTracks}
          onClick={music.toggle}
        >
          {music.playing ? "⏸" : "▶"}
        </button>
        <button className="icon-btn" aria-label="Next track" disabled={!music.hasTracks} onClick={music.next}>
          ⏭
        </button>
      </div>
      <input
        type="range"
        min={0}
        max={100}
        value={Math.round(music.prefs.volume * 100)}
        aria-label="Music volume"
        onChange={(e) => music.setPrefs({ volume: Number(e.target.value) / 100 })}
      />
    </div>
  );
}
