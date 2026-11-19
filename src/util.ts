export function hue(text: string): number {
  let h = 0;
  for (const c of text) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

export function initials(title: string): string {
  const words = title.replace(/\(.*?\)|\[.*?\]/g, "").split(/\s+/).filter(Boolean);
  return (words[0]?.[0] ?? "?").toUpperCase() + (words[1]?.[0] ?? "").toUpperCase();
}

/** "12 min played", "2h 5m played", or "" when there is nothing worth showing. */
export function playtime(sec: number): string {
  if (sec < 60) return "";
  const m = Math.floor(sec / 60);
  return m < 60 ? `${m} min played` : `${Math.floor(m / 60)}h ${m % 60}m played`;
}

export function duration(sec: number): string {
  if (sec < 60) return `${Math.max(0, Math.round(sec))} sec`;
  const m = Math.round(sec / 60);
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)}h ${m % 60}m`;
}

export function dateTime(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString();
}
