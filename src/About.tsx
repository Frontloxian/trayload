import { useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import logo from "./assets/trayload-logo.svg";
import type { Registry } from "./types";

const REPO = "https://github.com/Frontloxian/trayload";

interface Credit {
  name: string;
  systems: string[];
  link: string | null;
}

function link(url: string) {
  invoke("open_about_link", { url }).catch(() => {});
}

export default function About({ version, registry }: { version: string; registry: Registry | null }) {
  const credits = useMemo<Credit[]>(() => {
    const map = new Map<string, Credit>();
    for (const s of registry?.systems ?? []) {
      let url: string | null = null;
      if (s.source.type === "github") url = `https://github.com/${s.source.repo}`;
      else if (s.source.type === "forgejo") url = `https://${s.source.host}/${s.source.repo}`;
      const c = map.get(s.emulator) ?? { name: s.emulator, systems: [], link: url };
      c.systems.push(s.name);
      map.set(s.emulator, c);
    }
    return [...map.values()].sort((a, b) => a.name.localeCompare(b.name));
  }, [registry]);

  return (
    <>
      <section className="section about-head">
        <img src={logo} width={72} height={72} alt="" />
        <div>
          <h2>Trayload {version && `v${version}`}</h2>
          <p className="muted">A launcher for emulators. Free and open source under the GPL-3.0 license.</p>
          <button className="link" onClick={() => link(REPO)}>
            Source code and releases on GitHub
          </button>
        </div>
      </section>

      <section className="section">
        <h2>What Trayload does not do</h2>
        <p className="muted">
          Trayload is not an emulator. It does not provide, host or link to game ROMs, BIOS files, firmware or
          encryption keys. You supply those yourself, from games and consoles you own. Trayload sends no telemetry.
        </p>
      </section>

      <section className="section">
        <h2>Emulators</h2>
        <p className="muted">
          Trayload downloads these from their official release pages. All of the work in them belongs to their authors.
          Please support them.
        </p>
        <ul className="plain credit-list">
          {credits.map((c) => (
            <li key={c.name}>
              {c.link ? (
                <button className="link" onClick={() => link(c.link!)}>
                  {c.name}
                </button>
              ) : (
                <strong>{c.name}</strong>
              )}{" "}
              <span className="muted small">{c.systems.join(", ")}</span>
            </li>
          ))}
        </ul>
      </section>

      <section className="section">
        <h2>Game lists</h2>
        <p className="muted">
          The "Check games" feature compares files with No-Intro (cartridges) and Redump (discs) lists and adds year, developer and genre, which Trayload gets from the libretro-database
          project (licensed CC-BY-SA-4.0). Thank you to No-Intro, Redump and the libretro team for keeping them up.
        </p>
      </section>

      <section className="section">
        <h2>Music and logo</h2>
        <p className="muted">
          The built-in soundtrack and the Trayload logo are original work, licensed under CC BY-NC 4.0 (see
          LICENSE-MUSIC.md in the source code). Cover art comes from the sources you turn on in Settings.
        </p>
      </section>
    </>
  );
}
