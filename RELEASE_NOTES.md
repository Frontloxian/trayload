# Release notes for GitHub

This file feeds the text on each GitHub release page. The notes inside the app come from CHANGELOG.md instead, which can be longer.
Keep these short: one line per feature, plain words. Each version goes under its own `## x.y.z` heading, newest first.
The install steps and the "no ROMs or BIOS" notice are added automatically below each section, so don't repeat them here.

## 1.4.0

### New
- **Easier Settings.** Sections in a list on the left, plus a search box.
- **Check your games.** Compare cartridge games (NES, SNES, Game Boy, GBA, N64, DS, Sega) with the No-Intro lists. Each game shows "Verified" or "Not in the list". Nothing about your files is sent anywhere.
- **Game info.** Verified games show release year, developer, publisher and genre.
- **Pick your cover.** "Find covers online" shows box art, title screens and in-game pictures, including other regions.
- **Official names.** Verified games can use their official name. Files are never renamed.
- **Disc checks.** PlayStation, PS2, PSP, GameCube, Wii, Dreamcast, Saturn, Sega CD and Xbox discs can be checked against the Redump lists. Compressed formats (.chd, .rvz) can't.
- **Versions in one tile.** Regions and revisions of a game share a tile in the Library. Can be turned off.

## 1.3.0

### New
- **Docs site.** Getting started, a BIOS checklist for every console, FAQ and troubleshooting: https://frontloxian.github.io/trayload/
- **Emulator list updates.** If an emulator renames its download, Trayload can fix it without a new app release. Can be turned off.
- **Compatibility notes.** Short per-game tips on the game page, each with a source link.
- **Optional desktop notifications** when an emulator update is ready (off by default).

## 1.2.0

### New
- **Install all.** One button installs every emulator, with sizes shown first and a cancel option.
- **Bring existing games in.** Copies (or moves) older games into the games folder. Delete one permanently and a moved game goes back where it came from.
- **Smart collections.** "Never played" and "Recently added".
- **Music.** Shuffle button and a "Now playing" note.
- **About and Credits** page.

## 1.1.5

### New
- **Games folder.** Dropped games are copied into one folder per console. Your originals stay untouched.
- **Delete permanently** for removed games, including cover and play time.
- **Multi-disc games** show as one entry.
- **Sega consoles** now run on ares (Saturn on Ymir).

### Changed
- **RetroArch is gone.** Trayload offers to back up and clean up any old RetroArch folder.
- A few older consoles have no emulator for now. Your games stay in the library.

## 1.1.0

### New
- Built-in soundtrack, update notices and a "What's new" page.
- New logo and a more polished look.
- Code released under GPL-3.0.

## 1.0.0

First public release: one-click emulator installs and updates from official sources, drag-and-drop library with covers, a setup checklist for BIOS files you provide, News, USB backup and no telemetry.
