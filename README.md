# Trayload

A Windows launcher for emulators. Install standalone emulators from their official sources in one click, drag in your own games, and play from one library with covers, play time, news and optional built-in music.

> Trayload is a launcher. It is not an emulator, and it **does not provide, host or link to game ROMs, BIOS files, firmware or encryption keys**. You need to supply those yourself, from games and consoles you own.

## Features

- One-click installs and updates for standalone emulators, downloaded only from each project's official pinned source
- Drag-and-drop game import with automatic system detection and cover art
- Setup checklist per system (which BIOS or firmware files you need to supply)
- Long-tail consoles through RetroArch cores
- News tab following emulator releases and blogs
- USB backup of saves, settings and your library
- Built-in music with support for your own music packs
- Controller navigation and keyboard shortcuts
- No telemetry. See [PRIVACY.md](PRIVACY.md)

## Install

1. Go to **Releases** and download `Trayload_x.y.z_x64-setup.exe`.
2. Run it. Windows may show a "Windows protected your PC" (SmartScreen) warning because Trayload is not code-signed. Choose **More info**, then **Run anyway**. You can read the source and build it yourself if you prefer.
3. Trayload checks for its own updates in **Settings > Trayload updates**.

## Build from source

You need Node.js, Rust and the Tauri 2 prerequisites for Windows.

```powershell
npm install
npm run tauri dev
```

## Legal

- Trayload is released under the [MIT License](LICENSE). Any original music shipped with it belongs to its author and is not covered by that license unless stated.
- Emulators that Trayload downloads are separate programs with their own licenses and are not part of Trayload.
- Nintendo, PlayStation, Xbox, Sega and all other console and game names are trademarks of their owners. Trayload is not affiliated with or endorsed by any of them.
- Use emulation only with games and files you legally own.
