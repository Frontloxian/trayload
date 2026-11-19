import type { Game } from "./types";
import { useCover } from "./useCover";
import { hue, initials, playtime } from "./util";

interface Props {
  game: Game;
  systemName: string;
  running: boolean;
  onPlay: () => void;
  onOpen: () => void;
  onFavorite: () => void;
  onRemove?: () => void;
}

export default function GameTile({ game: g, systemName, running, onPlay, onOpen, onFavorite, onRemove }: Props) {
  const cover = useCover(g.id, g.artPath);
  const h = hue(g.title);
  const time = playtime(g.totalPlaytimeSec);
  return (
    <li className="tile">
      <button className="cover-btn" onClick={onPlay} disabled={running} aria-label={`Play ${g.title}`}>
        <div
          className="cover"
          style={{ background: `linear-gradient(135deg, hsl(${h} 55% 34%), hsl(${(h + 40) % 360} 55% 16%))` }}
        >
          {cover ? (
            <img className="cover-img" src={cover} alt="" />
          ) : (
            <span>{initials(g.title)}</span>
          )}
          {g.missing && <em className="badge">Missing</em>}
          {running ? <em className="badge live">Playing</em> : <i className="play">▶ Play</i>}
        </div>
      </button>
      <button
        className={"star" + (g.favorite ? " on" : "")}
        onClick={onFavorite}
        aria-pressed={g.favorite}
        aria-label={g.favorite ? `Remove ${g.title} from favorites` : `Add ${g.title} to favorites`}
      >
        {g.favorite ? "★" : "☆"}
      </button>
      {onRemove && (
        <button className="tile-remove" onClick={onRemove} aria-label={`Remove ${g.title} from the library`} title="Remove from library (the file is not deleted)">
          ✕
        </button>
      )}
      <button className="tile-title-btn" onClick={onOpen} title={`${g.title}\n${g.path}`}>
        {g.title}
      </button>
      <div className="muted small">
        {systemName}
        {time && ` · ${time}`}
      </div>
    </li>
  );
}
