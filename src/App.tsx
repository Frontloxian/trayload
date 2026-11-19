import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Registry } from "./types";
import Emulators from "./Emulators";
import "./App.css";

type Page = "home" | "library" | "emulators" | "setup" | "news" | "settings";

const NAV: { id: Page; label: string }[] = [
  { id: "home", label: "Home" },
  { id: "library", label: "Library" },
  { id: "emulators", label: "Emulators" },
  { id: "setup", label: "Setup checklist" },
  { id: "news", label: "News" },
];

export default function App() {
  const [page, setPage] = useState<Page>("home");
  const [registry, setRegistry] = useState<Registry | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<Registry>("get_registry").then(setRegistry).catch((e) => setError(String(e)));
  }, []);

  const title = [...NAV, { id: "settings" as Page, label: "Settings" }].find((n) => n.id === page)!.label;

  return (
    <div className="shell">
      <nav className="sidebar" aria-label="Main">
        <div className="brand">Trayload</div>
        {NAV.map((n) => (
          <button key={n.id} className="nav" aria-current={page === n.id ? "page" : undefined} onClick={() => setPage(n.id)}>
            {n.label}
          </button>
        ))}
        <div className="spacer" />
        <button className="nav" aria-current={page === "settings" ? "page" : undefined} onClick={() => setPage("settings")}>
          Settings
        </button>
      </nav>

      <main className="content">
        <header>
          <h1>{title}</h1>
        </header>

        {error && <p className="error">Registry failed to load: {error}</p>}

        {page === "emulators" ? (
          <Emulators />
        ) : (
          <p className="muted">
            Milestone 0 placeholder. Registry loaded: {registry ? `${registry.systems.length} systems` : "…"}.
          </p>
        )}
      </main>
    </div>
  );
}
