# Changelog

Each release's notes are shown inside Trayload (after updating, and under Settings > Trayload updates > What's new) and on the GitHub release page. The newest version goes at the top, under its own `## x.y.z` heading.

## 1.3.0

### New
- **Online updates for the emulator list:** if an emulator's download name changes, Trayload can pick up the fix from its GitHub page without a new app release. It is a plain download of a public file, there is an off switch in Settings > Online updates, and a downloaded list can never point an emulator at a different project or an unlisted host.
- **Compatibility notes:** short per-game tips (for example "needs the Vulkan renderer") show on the game page, from a small community-editable list with a source link for every note.
- **Desktop notifications** (off by default): a Windows notification when an emulator update is ready. Only while Trayload is open. Trayload also re-checks for updates every few hours while it stays open.

## 1.2.0

### New
- **Install all:** one button on the Emulators tab installs everything that is not installed yet. A list shows each emulator with its download size, and you can untick any. Experimental emulators start unticked. Installs run one after another ("3 of 20"), keep going if one fails, can be cancelled, and end with a summary.
- **Bring existing games into the games folder:** Settings > Games folder lists the games you added before the games folder existed, and you pick which to include. The default is copy (your originals stay). Switch to move if you want the originals removed after the copy is checked. If you permanently delete a moved game later, its files go back to where they came from, and the folders are recreated if they are gone. With Move selected, "Remove originals of copied games…" sends leftover originals of earlier copies to the Recycle Bin once their copies check out.
- **Smart collections:** Never played and Recently added join Recently played and Most played, under their own heading in the Library.
- **Music:** a shuffle button in the mini player, and a short "Now playing" note when a new track starts.
- **About and Credits** page: version, licenses, and links to every emulator project Trayload downloads from.

## 1.1.5

### New
- **Games folder:** games you drag onto Trayload are now copied into `Documents\Trayload\Games`, in one folder per console (for example `Nintendo 64`). Your originals are never changed. Change the folder, or open it, from Settings. Games with several files (.cue and .bin, and so on) and several discs are kept together. Nothing is overwritten, identical files are skipped, a progress bar shows the copy, and you can cancel it.
- **Delete permanently:** in Settings > Removed games, remove a game from Trayload for good: its cover, play time, favorite and collections go too. Files in Trayload's games folder move to the Recycle Bin. Files anywhere else are never touched.
- **Multi-disc games** show up as one entry.
- **Sega on its own emulators:** Master System, Game Gear, SG-1000, Genesis, Sega CD and 32X now use ares. Saturn uses Ymir.

### Changed
- **RetroArch is gone.** Trayload no longer installs or uses it. If an older version left a RetroArch folder behind, Trayload offers to back up its saves and BIOS files and asks before deleting it.
- PC Engine, Atari, Neo Geo Pocket, WonderSwan, Virtual Boy, Pokemon Mini, ColecoVision, Intellivision, Vectrex, MSX, 3DO and PC-FX have no emulator in Trayload for now (community addons will cover them later). Games you already added stay in your library and are marked "no emulator installed". Nothing is deleted.
- Folders you add with "Add folder" are still scanned in place, without copying.

## 1.1.0

### New
- **Built-in soundtrack:** five original tracks. Mute, pause and volume stay in the mini player.
- **Update notice:** Trayload now tells you when a new version is out, with the release notes and one-click install. You can turn the startup check off in Settings.
- **What's new:** release notes appear once after an update, and any time from Settings.
- **New logo and app icon** in the taskbar, window, installer and sidebar.
- **Installer artwork:** the Windows installer now has Trayload branding.

### Improved
- **A more detailed look:** sidebar icons, clearer page headers, softer panels and glow, smoother hover effects, better progress bars and dialogs.
- Page titles now explain what each screen is for.

### Licensing
- Trayload's code is now released under the GNU General Public License v3.0. The music and logo are under CC BY-NC 4.0.

## 1.0.0

First public release.

- One-click installs and updates for standalone emulators, only from each project's official source
- Drag-and-drop game import with automatic system detection and cover art
- Setup checklist for BIOS and firmware files you provide yourself
- Long-tail consoles through RetroArch cores
- News tab, USB backup, quick settings, shortcuts and controller navigation
- No telemetry
