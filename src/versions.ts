import type { Game } from "./types";

// Different regions and revisions of one game ("Tetris (USA)", "Tetris (Japan) (Rev 1)") share a name
// once the bracketed tags are removed. The Library can show them as one tile.

const DISC = /[(\[]\s*(disc|disk|cd)\s*\d/i;

export function baseTitle(title: string): string {
  return title
    .replace(/\s*[([][^)\]]*[)\]]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .toLowerCase();
}

/** Lower is the better default version to show: USA first, then World, Europe, others, Japan. */
export function versionScore(title: string): number {
  const t = title.toLowerCase();
  let score = 4;
  if (t.includes("(usa") || /\(.*\busa\b.*\)/.test(t)) score = 0;
  else if (t.includes("(world")) score = 1;
  else if (t.includes("(europe")) score = 2;
  else if (t.includes("(australia")) score = 3;
  else if (t.includes("(japan")) score = 5;
  if (/\((beta|proto|demo|sample|unl|pirate|hack)/.test(t) || /\[(b|h|p|t)\d*\]/.test(t)) score += 20;
  return score;
}

function key(g: Game): string {
  return `${g.systemId}|${baseTitle(g.title)}`;
}

/** The other versions of a game, not including the game itself. */
export function siblingsOf(game: Game, all: Game[]): Game[] {
  const base = baseTitle(game.title);
  if (!base || DISC.test(game.title)) return [];
  return all.filter((g) => g.id !== game.id && g.systemId === game.systemId && baseTitle(g.title) === base && !DISC.test(g.title));
}

function better(a: Game, b: Game): number {
  // A favorite wins, then the one played most, then the preferred region.
  if (a.favorite !== b.favorite) return a.favorite ? -1 : 1;
  if (a.totalPlaytimeSec !== b.totalPlaytimeSec) return b.totalPlaytimeSec - a.totalPlaytimeSec;
  const s = versionScore(a.title) - versionScore(b.title);
  return s !== 0 ? s : a.title.localeCompare(b.title);
}

/** One entry per game, with its other versions. The order of the list is kept. */
export function groupVersions(list: Game[]): { game: Game; versions: number }[] {
  const groups = new Map<string, Game[]>();
  for (const g of list) {
    const base = baseTitle(g.title);
    if (!base || DISC.test(g.title)) continue;
    const k = key(g);
    groups.set(k, [...(groups.get(k) ?? []), g]);
  }
  const out: { game: Game; versions: number }[] = [];
  const done = new Set<string>();
  for (const g of list) {
    const base = baseTitle(g.title);
    if (!base || DISC.test(g.title)) {
      out.push({ game: g, versions: 1 });
      continue;
    }
    const k = key(g);
    if (done.has(k)) continue;
    done.add(k);
    const members = groups.get(k) ?? [g];
    const primary = [...members].sort(better)[0];
    out.push({ game: primary, versions: members.length });
  }
  return out;
}
