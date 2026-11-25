# Privacy

Trayload has **no telemetry, no analytics, no accounts and no crash reporting**. Nothing about you, your games or your PC is ever sent anywhere.

Everything Trayload stores stays on your PC, in `%LOCALAPPDATA%\com.loxianthepeak.trayload\` (library, covers, emulators, logs) and in the Windows credential store (your optional SteamGridDB key).

## Every network call Trayload makes

Trayload only talks to these hosts, and only over HTTPS. Redirects to any other host are refused.

| Why | Host |
| --- | --- |
| Downloading emulators and checking for updates | `github.com`, `api.github.com`, `objects.githubusercontent.com`, `release-assets.githubusercontent.com`, `github-releases.githubusercontent.com`, `dolphin-emu.org`, `dl.dolphin-emu.org`, `git.eden-emu.dev`, `stable.eden-emu.dev`, `ppsspp.org`, `www.ppsspp.org` |
| News tab | `github.com`, `git.eden-emu.dev`, `dolphin-emu.org`, `www.reddit.com`, `old.reddit.com` |
| Game covers | `raw.githubusercontent.com` (libretro-thumbnails), and `www.steamgriddb.com` only if you add your own key |
| Updates to Trayload itself | `github.com` (this project's releases) |
| Newer emulator list and compatibility notes (optional, on by default, off switch in Settings) | `raw.githubusercontent.com` (two small files from this project's repository) |
| Desktop notifications (optional, off by default) | none: they are made on your PC from the update check above |

Each request is a plain download or feed read. A downloaded emulator list is checked before use: it can fix file names and launch options, but it can never point an emulator at a different project or a host that is not on the list above. Like any web request, the host can see your IP address.

## What Trayload never does

- It never downloads, hosts or links to game ROMs, BIOS files, firmware or keys.
- It never uploads your library, play time or files.
- The USB backup only copies files to a folder or drive you choose.
