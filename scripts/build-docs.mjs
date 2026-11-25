// Builds the Trayload docs site into ./docs (plain HTML, no dependencies).
// Run from the repository root:  node scripts/build-docs.mjs
// The "Systems and BIOS" page is generated from src-tauri/resources/emulators.json,
// so it always matches what the app checks. Edit the other pages in this file.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";

const REPO = "https://github.com/Frontloxian/trayload";
const registry = JSON.parse(readFileSync("src-tauri/resources/emulators.json", "utf8"));
let version = "";
try {
  version = (readFileSync("src-tauri/Cargo.toml", "utf8").match(/^version\s*=\s*"([^"]+)"/m) || [])[1] || "";
} catch {}
mkdirSync("docs", { recursive: true });

const esc = (s) =>
  String(s ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const PAGES = [
  ["index.html", "Home"],
  ["getting-started.html", "Getting started"],
  ["systems.html", "Systems and BIOS"],
  ["faq.html", "FAQ"],
  ["troubleshooting.html", "Troubleshooting"],
];

function layout(file, title, body) {
  const nav = PAGES.map(
    ([f, t]) => `<a href="${f}"${f === file ? ' aria-current="page"' : ""}>${esc(t)}</a>`,
  ).join("");
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)} · Trayload</title>
<meta name="description" content="Trayload is a free, open-source Windows launcher that installs and runs emulators from their official sources.">
<link rel="stylesheet" href="style.css">
</head>
<body>
<header>
  <a class="brand" href="index.html">Trayload</a>
  <nav aria-label="Main">${nav}</nav>
</header>
<main>
${body}
</main>
<footer>
  <p>Trayload${version ? " " + esc(version) : ""} · GPL-3.0 · <a href="${REPO}">Source on GitHub</a> · <a href="${REPO}/blob/main/PRIVACY.md">Privacy</a> · <a href="${REPO}/issues/new/choose">Report a problem</a></p>
  <p>Trayload does not provide or link game ROMs, BIOS files, firmware or keys. Use files from games and consoles you own.</p>
</footer>
</body>
</html>
`;
}

const write = (file, title, body) => writeFileSync(`docs/${file}`, layout(file, title, body));

// ---------- style ----------
writeFileSync(
  "docs/style.css",
  `:root{--bg:#0b0e16;--panel:#131826;--line:#252c3f;--text:#e6e9f2;--muted:#9aa3b8;--accent:#6aa6ff;--warn:#f0b35a}
*{box-sizing:border-box}
html{scroll-behavior:smooth}
body{margin:0;background:var(--bg);color:var(--text);font:16px/1.6 system-ui,Segoe UI,Roboto,sans-serif}
a{color:var(--accent)}
header{display:flex;flex-wrap:wrap;gap:12px 28px;align-items:center;padding:14px 24px;border-bottom:1px solid var(--line);background:var(--panel);position:sticky;top:0}
.brand{font-weight:700;font-size:1.15rem;color:var(--text);text-decoration:none}
nav{display:flex;flex-wrap:wrap;gap:6px 18px}
nav a{color:var(--muted);text-decoration:none;padding:2px 0}
nav a[aria-current=page],nav a:hover{color:var(--text);border-bottom:2px solid var(--accent)}
main{max-width:860px;margin:0 auto;padding:28px 24px 60px}
h1{font-size:2rem;margin:.2em 0 .4em}
h2{margin:1.8em 0 .4em;font-size:1.4rem}
h3{margin:1.4em 0 .3em;font-size:1.1rem}
.lead{font-size:1.15rem;color:var(--muted)}
.btn{display:inline-block;background:var(--accent);color:#08101f;padding:10px 18px;border-radius:8px;font-weight:600;text-decoration:none}
.card,.system{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:14px 18px;margin:14px 0}
.note{border-left:3px solid var(--warn);padding:6px 14px;background:#1a1810;border-radius:4px;color:#e8d9b8}
.muted{color:var(--muted)}
.pill{display:inline-block;font-size:.8rem;border:1px solid var(--line);border-radius:99px;padding:0 9px;color:var(--muted);margin-left:6px}
table{width:100%;border-collapse:collapse;margin:14px 0;font-size:.95rem}
th,td{text-align:left;padding:7px 10px;border-bottom:1px solid var(--line);vertical-align:top}
th{color:var(--muted);font-weight:600}
code{background:#1b2236;padding:1px 6px;border-radius:4px;font-size:.92em}
details{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:8px 16px;margin:10px 0}
summary{cursor:pointer;font-weight:600;padding:4px 0}
ol li,ul li{margin:.35em 0}
footer{border-top:1px solid var(--line);color:var(--muted);padding:18px 24px;text-align:center;font-size:.9rem}
@media (prefers-reduced-motion:reduce){html{scroll-behavior:auto}}
`,
);
writeFileSync("docs/.nojekyll", "");

// ---------- home ----------
write(
  "index.html",
  "Trayload",
  `<h1>Trayload</h1>
<p class="lead">One place to install, update and launch your emulators on Windows. Free, open source, no telemetry.</p>
<p><a class="btn" href="${REPO}/releases/latest">Download the latest version</a></p>
<div class="card">
<ul>
<li><strong>Official sources only.</strong> Emulators are downloaded from each project's own release page, never from a mirror.</li>
<li><strong>A clean library.</strong> Drop games onto the window. Trayload recognises the console, copies them into a folder per console, and fetches covers.</li>
<li><strong>A checklist for the awkward bits.</strong> The Setup tab shows which BIOS or firmware files each console needs and checks whether you have them.</li>
<li><strong>Stays up to date.</strong> "Update ready" badges, optional automatic updates, and a News tab for emulator releases.</li>
</ul>
</div>
<p class="note">Trayload does not provide, host or link game ROMs, BIOS files, firmware or encryption keys. You supply those yourself from games and consoles you own.</p>
<h2>Where to go next</h2>
<ul>
<li><a href="getting-started.html">Getting started</a>: your first game in five minutes</li>
<li><a href="systems.html">Systems and BIOS</a>: what every console needs, and which emulator Trayload uses</li>
<li><a href="faq.html">FAQ</a></li>
<li><a href="troubleshooting.html">Troubleshooting</a></li>
</ul>`,
);

// ---------- getting started ----------
write(
  "getting-started.html",
  "Getting started",
  `<h1>Getting started</h1>
<p class="lead">From install to your first game.</p>
<h2>1. Install Trayload</h2>
<p>Download the installer from the <a href="${REPO}/releases/latest">latest release</a> and run it. It installs for your user account only, so it does not ask for administrator rights.</p>
<p class="note">Windows may show a "Windows protected your PC" (SmartScreen) message, because the installer is not code-signed yet. Choose <strong>More info</strong>, then <strong>Run anyway</strong>. The code is open source, and every release is built on GitHub from the public source.</p>
<h2>2. Install an emulator</h2>
<ol>
<li>Open the <strong>Emulators</strong> tab.</li>
<li>Click <strong>Install</strong> next to the emulator for your console, or <strong>Install all…</strong> to pick several at once. You see the download sizes first.</li>
<li>Trayload downloads it from the emulator's official page and sets it up in its own folder. Nothing else on your PC is changed.</li>
</ol>
<h2>3. Check what the console needs</h2>
<p>Open the <strong>Setup checklist</strong> tab. Some consoles need files from your own console, such as a BIOS or firmware. The checklist shows what is missing, and <strong>Open folder</strong> takes you to the exact place to put each file. See <a href="systems.html">Systems and BIOS</a> for the full list.</p>
<h2>4. Add your games</h2>
<ul>
<li><strong>Drag game files or folders onto the Trayload window.</strong> Trayload recognises the console and copies each game into <code>Documents\\Trayload\\Games\\&lt;console&gt;</code>. Your originals are not changed. Games with several files (such as a <code>.cue</code> with <code>.bin</code> files) are kept together, and several discs of one game become one entry.</li>
<li>If Trayload cannot tell which console a file is for, it asks you.</li>
<li><strong>Add folder</strong> in the Library scans a folder where it is, without copying. Good for a large collection you already keep somewhere.</li>
<li>You can change the games folder, and bring games you added earlier into it, under <strong>Settings</strong>.</li>
</ul>
<h2>5. Play</h2>
<p>Click a game, then <strong>Play</strong>. Trayload starts the right emulator, tracks play time, and lists the game under Recently played. Use <strong>Favorites</strong> and <strong>Collections</strong> to organise, and the game's page for per-game launch options and notes.</p>
<h2>Handy things</h2>
<ul>
<li>Press <code>?</code> for the keyboard shortcuts. A controller can move around the app too.</li>
<li><strong>Settings</strong> has covers (optional SteamGridDB key), music, backups to a USB drive or any folder, online updates and the games folder.</li>
<li>Trayload updates itself: an update banner appears when a new version is out, with release notes.</li>
</ul>`,
);

// ---------- systems and BIOS (generated) ----------
const fmtSize = (b) => (b >= 1048576 ? `${b / 1048576} MB` : b >= 1024 ? `${b / 1024} KB` : `${b} bytes`);
const needs = (s) => {
  const items = s.setup?.items ?? [];
  if (items.some((i) => i.required !== false)) return "Required files";
  if (items.length) return "Optional files";
  return "Nothing extra";
};
const byEmulator = new Map();
for (const s of registry.systems) {
  if (!byEmulator.has(s.emulator)) byEmulator.set(s.emulator, []);
  byEmulator.get(s.emulator).push(s);
}

const rows = registry.systems
  .map((s) => `<tr><td><a href="#${esc(s.id)}">${esc(s.name)}</a></td><td>${esc(s.emulator)}</td><td>${needs(s)}</td></tr>`)
  .join("\n");

const sections = registry.systems
  .map((s) => {
    const items = s.setup?.items ?? [];
    const list = items.length
      ? `<table><thead><tr><th>File</th><th></th><th>Notes</th></tr></thead><tbody>${items
          .map((i) => {
            const bits = [];
            if (i.sizes?.length) bits.push(`Size: ${i.sizes.map(fmtSize).join(" or ")}`);
            if (i.note) bits.push(i.note);
            return `<tr><td>${esc(i.label)}</td><td>${i.required === false ? "Optional" : "<strong>Required</strong>"}</td><td>${esc(bits.join(" "))}</td></tr>`;
          })
          .join("")}</tbody></table>`
      : "";
    return `<section class="system" id="${esc(s.id)}">
<h3>${esc(s.name)}<span class="pill">${esc(s.emulator)}</span>${s.tier === "experimental" ? '<span class="pill">Experimental</span>' : ""}</h3>
${s.setup?.intro ? `<p>${esc(s.setup.intro)}</p>` : ""}
${list}
${s.warning ? `<p class="note">${esc(s.warning)}</p>` : ""}
<p class="muted">File types: ${s.exts.map((e) => `<code>.${esc(e)}</code>`).join(" ")}</p>
</section>`;
  })
  .join("\n");

const retired = (registry.retired ?? []).map((r) => esc(r.name)).join(", ");

write(
  "systems.html",
  "Systems and BIOS",
  `<h1>Systems and BIOS</h1>
<p class="lead">Which emulator Trayload uses for each console, and the files each one needs.</p>
<p class="note">Trayload does not provide or link BIOS files, firmware or keys. Where one is needed, dump it from your own console. The Setup checklist in the app checks for these files and shows exactly where to put them.</p>
<table><thead><tr><th>Console</th><th>Emulator</th><th>Needs</th></tr></thead><tbody>
${rows}
</tbody></table>
<h2>Details</h2>
${sections}
<h2>Not supported right now</h2>
<p>Trayload no longer ships an emulator for: ${retired}. Games you already added for these stay in your library, marked "no emulator installed". Community addons are planned to cover them later.</p>
<p class="muted">This page is generated from the emulator list built into Trayload (list version ${esc(registry.revision)}), so it matches what the app checks.</p>`,
);

// ---------- FAQ ----------
const faq = [
  ["Does Trayload include games, BIOS files or keys?", "No. Trayload does not provide, host or link ROMs, BIOS files, firmware or keys, and it will never ask you to share them. Use files dumped from games and consoles you own."],
  ["Is it free? What is the license?", "Yes. Trayload is free and open source under the GPL-3.0 license. The built-in music and the logo are under CC BY-NC 4.0."],
  ["Does it collect any data?", `No telemetry, no analytics, no accounts and no crash reporting. Every network request Trayload makes is listed in the <a href="${REPO}/blob/main/PRIVACY.md">privacy page</a>.`],
  ["Why does Windows warn me when I install it?", "The installer is not code-signed yet (a signing certificate costs money), so SmartScreen shows a warning for unknown publishers. Choose More info, then Run anyway. Releases are built by GitHub from the public source code."],
  ["Where does Trayload get emulators?", "Only from each emulator's official release page (GitHub, or the project's own site). Downloads from any other site are refused."],
  ["Which consoles are supported?", `See <a href="systems.html">Systems and BIOS</a>. It lists every console, its emulator and what it needs.`],
  ["What happened to RetroArch?", "Trayload used to run some older consoles through RetroArch. It was dropped because BIOS setup was awkward and individual emulators are easier to manage. Sega consoles moved to their own emulators (ares and Ymir). The other consoles are planned to come back through community addons. Games you already added stay in your library."],
  ["Does Trayload copy my games?", "Games you drag onto the window are copied into the games folder, one folder per console, and your originals are left alone. Folders you add with Add folder are scanned in place. Existing games can be brought into the games folder from Settings, as a copy or a move."],
  ["What does 'Delete permanently' do?", "It removes every trace of a game from Trayload: the entry, cover, play time, favorite and collections. Files in Trayload's games folder go to the Recycle Bin (games that were moved in go back to where they came from). Files anywhere else are never touched."],
  ["Can I use an emulator that is not on the list?", "Not yet. Addons that add emulators and themes are planned for a later version."],
  ["Does it work on Mac or Linux?", "Not at the moment. Trayload is Windows-only."],
  ["How do I back up my saves?", "In Settings, use the backup section to copy saves, save states and emulator settings to a USB drive or any folder you pick."],
  ["How do I uninstall it?", `Use Windows "Installed apps". Trayload's data (library, covers, emulators, logs) is in <code>%LOCALAPPDATA%\\com.loxianthepeak.trayload</code> and your games folder is under <code>Documents\\Trayload</code>. Neither is deleted automatically, so you can remove them by hand if you want everything gone.`],
];
write(
  "faq.html",
  "FAQ",
  `<h1>FAQ</h1>
${faq.map(([q, a]) => `<details><summary>${esc(q)}</summary><p>${a}</p></details>`).join("\n")}`,
);

// ---------- troubleshooting ----------
const trouble = [
  ["Windows blocks or warns about the installer", "That is SmartScreen, because the installer is not code-signed. Choose <strong>More info</strong>, then <strong>Run anyway</strong>. If your antivirus quarantines an emulator after it is installed, add Trayload's emulator folder (<code>%LOCALAPPDATA%\\com.loxianthepeak.trayload\\emulators</code>) to its exclusions. Emulators are sometimes flagged wrongly."],
  ["An emulator will not install, or says no matching download", "Emulators sometimes rename their download files. Open <strong>Settings</strong>, then <strong>Online updates</strong> and click <strong>Check now</strong>, because a fixed list may already be available. If it still fails, open an issue with the name of the emulator and the message you see."],
  ["A game does not start", "1) Check the <strong>Setup checklist</strong> for missing BIOS or firmware. 2) Make sure the emulator is installed (Emulators tab). 3) Open <strong>Settings</strong>, then <strong>Open log folder</strong> to see what happened. 4) Try the game directly in the emulator to tell whether the problem is the game file or Trayload."],
  ["The game page says \"no emulator installed\"", "Trayload no longer has an emulator for that console (for example PC Engine or Atari, which RetroArch used to cover). The game stays in your library. Community addons are planned to cover these consoles."],
  ["Trayload asks which console a file is for", "Some file types (like <code>.iso</code>, <code>.bin</code> or <code>.chd</code>) are used by several consoles and cannot always be recognised. Pick the console once and Trayload remembers. A folder named after the console (for example <code>PlayStation</code>) also helps it decide."],
  ["A game shows as missing", "Trayload cannot find the file where it was last seen, for example because a drive is disconnected or the file moved. Reconnect the drive or put the file back, then press <strong>Rescan</strong>."],
  ["Copying games stops with a space error", "Trayload checks free disk space before it copies. Free some space, or change the games folder to a drive with more room in <strong>Settings</strong>, then drop the games again. Anything already copied is kept, and identical files are skipped."],
  ["A cover is missing or wrong", "Covers come from libretro-thumbnails, and from SteamGridDB if you add a free key in Settings. Games with unusual names may not match. You can pick a different cover on the game's page."],
  ["There is no music", "Check that <strong>Play music</strong> is ticked in Settings and the volume is up. Music pauses automatically while a game is running."],
  ["I still have an old RetroArch folder", "Older versions installed RetroArch. Trayload offers to back up its saves and BIOS files and asks before deleting it. You can do it any time under <strong>Settings</strong>, then <strong>Old RetroArch folder</strong>."],
  ["Something else is wrong", `Open <strong>Settings</strong>, then <strong>Open log folder</strong>, and look at the newest log. Then <a href="${REPO}/issues/new/choose">open an issue</a> with your Trayload version and what happened. Please do not attach ROMs, BIOS files or keys. Check the log for private file paths before pasting it.`],
];
write(
  "troubleshooting.html",
  "Troubleshooting",
  `<h1>Troubleshooting</h1>
${trouble.map(([q, a]) => `<details><summary>${esc(q)}</summary><p>${a}</p></details>`).join("\n")}`,
);

console.log("Docs written to ./docs (" + (PAGES.length + 1) + " files plus style.css)");
