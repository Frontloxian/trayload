import type { ReactNode } from "react";
import changelog from "../CHANGELOG.md?raw";
import { Dialog } from "./Overlays";

/** The part of CHANGELOG.md under "## <version>", without the heading. Empty if that version isn't listed. */
export function notesFor(version: string): string {
  const lines = changelog.split(/\r?\n/);
  const start = lines.findIndex((l) => new RegExp(`^##\\s+v?${version.replace(/\./g, "\\.")}(\\s|$)`).test(l));
  if (start < 0) return "";
  const out: string[] = [];
  for (const l of lines.slice(start + 1)) {
    if (/^##\s/.test(l)) break;
    out.push(l);
  }
  return out.join("\n").trim();
}

function inline(text: string): ReactNode[] {
  return text.split(/(\*\*[^*]+\*\*|`[^`]+`)/g).map((part, i) => {
    if (part.startsWith("**") && part.endsWith("**")) return <strong key={i}>{part.slice(2, -2)}</strong>;
    if (part.startsWith("`") && part.endsWith("`")) return <code key={i}>{part.slice(1, -1)}</code>;
    return part;
  });
}

/** A tiny renderer for the release notes: ### headings, "- " bullets and plain paragraphs. */
export function Notes({ markdown }: { markdown: string }) {
  const blocks: ReactNode[] = [];
  let bullets: string[] = [];
  const flush = () => {
    if (bullets.length) {
      blocks.push(
        <ul key={`u${blocks.length}`}>
          {bullets.map((b, i) => (
            <li key={i}>{inline(b)}</li>
          ))}
        </ul>,
      );
      bullets = [];
    }
  };
  for (const line of markdown.split(/\r?\n/)) {
    const t = line.trim();
    if (!t) {
      flush();
    } else if (/^#{1,6}\s/.test(t)) {
      flush();
      blocks.push(<h3 key={`h${blocks.length}`}>{t.replace(/^#{1,6}\s+/, "")}</h3>);
    } else if (/^[-*]\s/.test(t)) {
      bullets.push(t.replace(/^[-*]\s+/, ""));
    } else {
      flush();
      blocks.push(<p key={`p${blocks.length}`}>{inline(t)}</p>);
    }
  }
  flush();
  return <div className="notes">{blocks}</div>;
}

export function WhatsNew({ version, onClose }: { version: string; onClose: () => void }) {
  const text = notesFor(version);
  return (
    <Dialog title={`What's new in Trayload ${version}`} onClose={onClose} wide>
      {text ? <Notes markdown={text} /> : <p className="muted">No release notes for this version.</p>}
      <div className="row">
        <button className="btn primary" onClick={onClose}>
          Got it
        </button>
      </div>
    </Dialog>
  );
}
