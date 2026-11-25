# Contributing to Trayload

Thanks for wanting to help. Bug reports, ideas, documentation and code are all welcome.

By contributing you agree that your work is released under the project's license, **GPL-3.0** (see `LICENSE`). The music and logo are under CC BY-NC 4.0 (see `LICENSE-MUSIC.md`).

## Ground rules

- **No ROMs, BIOS files, firmware or keys.** Do not attach, paste or link them in issues, pull requests or code. Trayload only launches and sets up emulators. Setup text should say that Trayload does not provide or link these files.
- **Official sources only.** Emulator downloads must come from the project's own release page (GitHub, Forgejo or its official site), pinned in `src-tauri/resources/emulators.json`.
- **No telemetry.** Trayload sends nothing about its users. See `PRIVACY.md`.
- Be kind. See `CODE_OF_CONDUCT.md`.

## Reporting a bug

Open an issue with the **Bug report** form. The most useful reports include your Trayload version, what you clicked, what you expected and what happened. The log folder is under **Settings** if you need to attach a log (check it for private paths first).

## Suggesting something

Use the **Feature request** form. Check `ROADMAP.md` first, because your idea may already be planned.

## Building from source

You need Node.js, Rust and the [Tauri 2 prerequisites for Windows](https://tauri.app/start/prerequisites/).

```powershell
npm install
cd src-tauri
cargo test
cd ..
npm run tauri dev
```

## Sending a pull request

1. Fork the repository and make a branch.
2. Keep the change focused. One fix or feature per pull request.
3. Run `cargo test` and `npm run build` before you push.
4. Add a line to the top section of `CHANGELOG.md` if people using Trayload would notice the change.
5. Describe what you changed and how you tried it.

Changes to the emulator list (`emulators.json`) should say where the download comes from and how you checked the file names and launch options.

## Fixing a broken emulator download

Emulator downloads are described in `src-tauri/resources/emulators.json`. If an emulator renames its download file, edit that entry's `assetPattern` (and `exePattern` if the program name changed), then **raise `revision` at the top of the file by one**. Once the change is on the `main` branch, Trayloads that have online updates turned on pick it up without a new release.

An online update can fix patterns and launch options only. It can not point an emulator at a different project, so adding a new emulator, or changing where one comes from, always needs a normal app release.

## Compatibility notes

Per-game tips live in `src-tauri/resources/compat.json`. Every note needs a system id, one or two sentences, and a GitHub link showing where the tip comes from. Raise `revision` when you add one.
