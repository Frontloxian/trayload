# Roadmap: content and features to add

Name: **Trayload** (decided). Windows, Tauri + React.
Phases are in priority order. Items marked **(decide)** need a call from you before we plan them in detail.

## Release plan
Small releases, roughly one every few weeks. Each release gets a `## x.y.z` section in `CHANGELOG.md`, which feeds the in-app release notes and the GitHub release page.

| Release | Theme | Contents |
| --- | --- | --- |
| **1.1.0** | Polish | Logo and icons, five-track built-in soundtrack, update notice, in-app release notes, installer artwork, UI polish |
| **1.1.x** | Fixes and housekeeping | Bug fixes from first-week feedback, README screenshots, GitHub issue templates, a short contributing note (contributions are under GPL-3.0) |
| **1.1.5** | Games folder and cleanup (first half) | Shipped early because it does so much: default games folder with a folder per console and copy on drag, RetroArch removed (Sega systems move to ares and Ymir), permanent delete in Removed games, multi-disc handling |
| **1.2.0** | Library tools | Move existing games into the games folder (see below), Install all, smart collections, music skip and shuffle with a "Now playing" toast, an About and Credits page |
| **1.3.0** | Keeps itself working | Hosted registry (fix a broken emulator download without a new app release), the docs site (getting started, BIOS checklist per system, FAQ, troubleshooting), optional desktop notifications for major emulator releases, per-game compatibility notes |
| **1.4.0** | Know your games | Settings page reorganised into sections with search, ROM verification against No-Intro and Redump lists, release year, developer and genre info, manual cover picker, region and version grouping |
| **1.5.0** | Addons and play features | Data-only addons (themes, emulator profiles, extra news feeds, music packs), saves and save-state manager, controller layouts per system, screenshot gallery, CRT and shader presets |
| **1.6.0** | Community | Homebrew and freeware shelf, IPS/BPS patching on a copy, RetroAchievements |
| **1.7.0** | Code addons | Addons that run code (launch hooks, Discord presence, overlays), with permissions and a clear warning before enabling |
| **2.0.0** | New systems | Computers (Amiga, C64, DOS, MSX, ZX Spectrum, Apple II), Lichii play support once it works properly, PS5 from watch-only to experimental only when the criteria in PLAN.md are met |
| **Later** | Far back | Arcade (MAME, FinalBurn Neo, Neo Geo) and Neo Geo CD |

Checklist for every release:
1. `cargo test` passes
2. A fresh install works, and so does an update from the previous version
3. `CHANGELOG.md` has a section for the version
4. Any new network host is listed in `PRIVACY.md`
5. Tag it (`git tag vX.Y.Z`, `git push origin vX.Y.Z`) and check the Actions run turns green

Not tied to a version: code signing (removes the SmartScreen warning; look into free signing programs for open-source projects if Trayload gains users), and the Steam and itch.io pages if wanted.

## 1.2.0 in detail
**Decisions**
- RetroArch is dropped completely (buggy, BIOS setup is hard, individual emulators are easier to handle). The RetroArch cores, the RetroArch install and update code, and the nightly-build handling are removed.
- Sega systems move to standalone emulators: ares for Master System, Game Gear, SG-1000, Genesis, Sega CD and 32X, and Ymir for Saturn. Both come from their official GitHub releases. Exact download names, launch options and BIOS folders are verified before they are added.
- Every other former RetroArch system (PC Engine, Atari 2600/5200/7800/Lynx/Jaguar, Neo Geo Pocket, WonderSwan, Virtual Boy, Pokemon Mini, ColecoVision, Intellivision, Vectrex, MSX, 3DO, PC-FX) is left to community addons (1.5.0). Games already in the library for these systems stay in the library and show "no emulator installed". Nothing is deleted.
- Before RetroArch is removed from an existing install, Trayload offers a backup of its saves and states and asks before deleting its folder.

**Default games folder**
- Default location `Documents\Trayload\Games`, changeable in Settings, with an "Open games folder" button. It is added to scanning automatically.
- One subfolder per console (for example `Nintendo 64`, `PlayStation 2`).
- Dragging games onto the window detects the system and copies the game into that console's folder. Unknown systems ask which console first.
- Multi-file games are copied as one set (`.cue` with its `.bin` tracks, `.m3u` playlists, folder-based games).
- Never overwrites: a same-name file gets a number, an identical copy is skipped. Large copies show progress with a cancel button, and free disk space is checked first.
- In 1.1.5, games already in the library stay where they are. Moving them into the games folder comes in 1.2.0 (next section).

**Move existing games into the games folder (1.2.0)**
- Games added before 1.1.5 (and games from scanned folders) are found by a scan and offered for moving into the games folder, each into its console's folder.
- A review list shows what will be moved, with sizes and a free-space check, and items can be unticked. Nothing happens until the person confirms.
- Same rules as dragging games in: sets are kept together, nothing is overwritten, a same-name file gets a number, an identical copy is skipped, with progress and cancel.
- Default is **copy**: originals stay where they are (this uses extra disk space). A setting in Settings switches to **move**, where each original is removed only after its copy is verified.
- Trayload remembers each moved game's original path. If a moved game is later deleted permanently, its files go **back to the original path** instead of the Recycle Bin. If that folder no longer exists, Trayload creates it. If a file with the same name is already there, nothing is overwritten (the returned file gets a number).
- Copied games (originals still in place) and games dragged in work as before: deleting them permanently sends the games-folder copy to the Recycle Bin.
- Library entries are updated to the new location, so play time, favorites and collections are kept.

**Install all**
- Button on the Emulators tab. A confirmation list shows what will be installed and the approximate download size, with items that can be unticked. Experimental emulators are left out unless ticked.
- Installs run one at a time with progress ("3 of 20"), skip anything already installed, continue past failures, can be cancelled, and end with a summary.

**Permanent delete (Removed games)**
- "Delete permanently" per game and "Delete all". It removes every trace of the game from Trayload: the library entry, cover, play time and history, favorites and collection membership.
- Files in Trayload's own games folder are moved to the Recycle Bin, except games that were moved there from another place (1.2.0), which are returned to their original path, creating it if needed. Files outside Trayload's folders are never touched.
- A game whose file still sits in a scanned folder outside Trayload's folders would come back on the next scan, so only its path is kept in an "Ignored files" list (Settings), which can be cleared.

**Also in 1.2.0**
- Smart collections (Never played, Recently added, Recently played)
- Multi-disc handling (first version in 1.1.5): one library entry per game
- Music skip and shuffle, and a "Now playing" toast
- About and Credits page

**Build order:** M15 (games folder, RetroArch removal, Sega standalone emulators, permanent delete, multi-disc) is released as 1.1.5. M16 (move existing games, Install all, smart collections, music controls, About page) is released as 1.2.0.

## Phase 0: v1 (already planned in PLAN.md)
Installer, drag-and-drop import, library with art, launcher with playtime, setup checklist, News tab, update badges, and the built-in soundtrack (your own music, with volume, mute and pause-while-playing).

## Phase 1: Make v1 feel finished
- **Custom music packs:** add a folder or `.zip` of audio files (with an optional `pack.json`) in Settings > Music, preview and switch packs, remove them with a confirmation. User-supplied files only, nothing downloaded. Details in PLAN.md 5.10
- **Logo and icon set:** export the Trayload logo as a multi-size `.ico`, a tray icon and installer art, using the simplified version for the small sizes
- **First-run onboarding:** pick ROM folders, install your first emulator, see the setup checklist, all in one short flow
- **Empty states and copy:** friendly text for an empty library, a missing BIOS, a failed download
- **Error and log viewer:** a Settings page that shows what went wrong and offers "copy log"
- **Per-emulator quick settings:** fullscreen, resolution and the few options people change most, written into each emulator's own config
- **Backup and export:** library database plus the saves folders, one click

## Phase 2: Better library content
- **ROM verification:** match files against community DAT lists (No-Intro for cartridges, Redump for discs) to flag bad dumps and name games correctly
- **Metadata:** release year, developer, genre, description, from IGDB and ScreenScraper, shown on the game page
- **Artwork variety:** covers, heroes, logos, screenshots, and a manual art picker
- **Duplicate and version handling:** group regions and revisions under one game, choose a preferred region
- **Multi-disc and playlists:** disc switching and `.m3u` handling, shown as one entry
- **Smart collections:** auto lists such as "Never played", "Recently added", "By decade", "By genre"
- **Patches:** apply IPS/BPS/UPS patches (translations, fixes) to a copy, never the original file

## Phase 3: More systems
- **Homebrew and freeware shelf (first item in this phase, not in v1):** a dedicated section of the library for homebrew, freeware and fan-made games. Users find and download these themselves from the authors' own pages and add them like any other game, so Trayload keeps no curated list and offers no one-click downloads. What we build is the shelf itself: a Homebrew tag or category on import, a filter and its own row in the library, and a short "how to add homebrew" tip. Nothing in the app hosts or links to commercial games or unauthorized ROM hacks of them
- **Computers:** Amiga, C64, DOS, MSX, ZX Spectrum, Apple II
- **Remaining long-tail consoles:** RetroArch is dropped (see 1.1.5). Sega systems use standalone emulators; the rest are left to community addons (1.5.0)
- **Newer experimental systems:** PS5 moving from watch-only to 🧪 once the criteria in PLAN.md are met
- **Lichii play-from-Trayload (Switch 2, opt-in, 🧪):** deferred until after M13 (release), but ahead of arcade. It can already be installed and launched from the Emulators tab; it loads an extracted ExeFS `main`, not a single game file, so Play needs its own design
- **Arcade (MAME, FinalBurn Neo, Neo Geo, Neo Geo CD):** pushed far back, last in this phase

## Phase 4: Play-time features
- **Achievements:** RetroAchievements login and per-game progress
- **Save states and saves manager:** browse, back up and restore saves and states per game
- **Save to USB (confirmed, replaces cloud sync):** pick a USB drive or any external folder as a backup target so your data survives a PC failure. Covers all three: save files, save states and emulator settings. Options: back up on a button press, automatically after each play session, and when the drive is plugged in. Keep timestamped copies, restore from the drive on a new PC, and handle the drive being unplugged without errors. Cloud sync stays out of scope
- **Controller profiles:** one controller layout per system, applied to every emulator
- **Screenshot and clip gallery:** collect captures per game
- **Cheats (not in v1):** per-emulator cheat file support, revisit after v1 ships
- **Shader and filter presets:** CRT looks for retro systems, chosen from the game page
- **Handheld and TV modes (not in v1):** a bigger "couch" layout with on-screen keyboard, revisit after v1 ships

## Phase 5: Community and content feeds
- **News upgrades:** filters per emulator, "what's new since you last played", optional desktop notifications for major releases
- **Release notes in the app:** the changelog shown at the Update button
- **Hosted registry:** emulator list and asset patterns served from a file we update without shipping a new app version
- **Compatibility notes:** per-game tips such as "needs the Vulkan renderer", from a small community-editable list
- **Themes:** a few dark palettes, and later a theme file format
- **Music extras:** per-screen track lists, a pack format that can also carry menu sound effects (click, launch, install-complete sounds), and a "now playing" toast **(decide)**

## Phase 6: Release readiness
- **Naming checks:** done, all clean
- **Legal copy:** disclaimer that the app ships no games, BIOS files or keys, plus a clear stance on Switch and Switch 2 handling
- **Licensing:** pick an app license and check that bundled pieces (a 7z library, icons, fonts) allow redistribution
- **Code signing and installer:** signing certificate, MSI or NSIS installer, SmartScreen reputation
- **Auto-update for the app itself**
- **Docs site:** getting started, BIOS checklist per system, FAQ, troubleshooting (built in 1.3.0; regenerate with `node scripts/build-docs.mjs` whenever the emulator list changes)
- **Privacy:** state clearly that there is no telemetry, and list every network call
- **Beta test:** a small group, a feedback channel, a crash report option that is opt-in

## Phase 7: Ideas parked for later
- Linux and Steam Deck build
- Plugins or scripting: data-only addons are planned for 1.5.0 and code-running addons for 1.7.0
- Multi-user profiles
- Netplay setup helpers
- Mobile companion for browsing the library
- Localization

## Rules that apply to every phase
- Never download, host or link to ROMs, BIOS files, firmware or keys
- Never fetch an emulator from anywhere except its official, pinned source
- Experimental and watch-only systems stay behind the 🧪 and 👀 rules
- No telemetry
- Registry data is data, not code, so it can change without an app release
- Music never plays over a running game, and Trayload never downloads or hosts music

## Decided
- Homebrew and freeware shelf: yes, but after v1, as the first item of Phase 3
- Cloud save sync: dropped. Save to USB instead (Phase 4), covering saves, save states and emulator settings
- Import from other launchers: not needed, removed
- Name: Trayload (name, domain, GitHub and store-page checks came back clean)
- Licenses: GPL-3.0 for the code, CC BY-NC 4.0 for the music and logo
- Release plan: see the table at the top. The docs site ships with 1.3.0
- Built so far: M0-M16 (M15 shipped as 1.1.5, M16 as 1.2.0) (Switch via Eden, News, music, polish, USB backup, release setup, update notice, soundtrack). Long-tail consoles via RetroArch cores were built in M11 and are removed in 1.2.0. v1.0.0 and v1.1.0 are the release milestones
- RetroArch was dropped in 1.1.5
- Order of deferred items: Lichii play-from-Trayload first, arcade far behind it
- Homebrew shelf: users do the finding and downloading themselves, no curated list or one-click downloads
- Music: yes. Your own original soundtrack is built in (v1), and custom music packs are supported (Phase 1)
- Cheats: not in v1
- Couch mode: not in v1

## Still open
Nothing blocks the release plan. Code signing and store pages are optional and not tied to a version.
