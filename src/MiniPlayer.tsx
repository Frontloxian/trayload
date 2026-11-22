import type { Music } from "./useMusic";

export default function MiniPlayer({ music }: { music: Music }) {
  if (!music.prefs.enabled && !music.hasTracks) return null;
  // The track name appears in the "Now playing" note, so only problems and pauses are written here.
  const status = !music.hasTracks
    ? "No music yet. Add some in Settings."
    : music.pausedByGame
      ? "Paused while a game runs"
      : null;
  return (
    <div className="mini" role="group" aria-label="Music">
      {status && (
        <div className="mini-title" title={status}>
          {status}
        </div>
      )}
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
        <button
          className="icon-btn"
          aria-label="Shuffle"
          aria-pressed={music.prefs.shuffle}
          title={music.prefs.shuffle ? "Shuffle is on" : "Shuffle is off"}
          disabled={!music.hasTracks}
          style={{ opacity: music.prefs.shuffle ? 1 : 0.5 }}
          onClick={() => music.setPrefs({ shuffle: !music.prefs.shuffle })}
        >
          🔀
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
