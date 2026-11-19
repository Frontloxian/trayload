export type Source =
  | { type: "github"; repo: string; assetPattern: string }
  | { type: "page"; url: string; assetPattern?: string | null };

export interface System {
  id: string;
  name: string;
  emulator: string;
  source: Source;
  exts: string[];
  exePattern?: string | null;
  portableMarker?: string | null;
}

export interface Registry {
  systems: System[];
}

export interface InstalledInfo {
  version: string;
  assetName: string;
  installedAt: number;
  exeRelPath: string;
}

export interface EmulatorEntry {
  id: string;
  name: string;
  systems: string[];
  installable: boolean;
  installed: InstalledInfo | null;
}

export interface InstallProgress {
  id: string;
  stage: "resolving" | "downloading" | "extracting" | "done";
  downloaded: number;
  total: number;
}

export interface Game {
  id: number;
  systemId: string;
  title: string;
  path: string;
  size: number;
  addedAt: number;
  lastPlayed: number | null;
  totalPlaytimeSec: number;
  favorite: boolean;
  missing: boolean;
  artPath: string | null;
}

export interface ArtSettings {
  hasSteamgriddbKey: boolean;
}

export interface Unassigned {
  path: string;
  fileName: string;
  candidates: string[];
}

export interface ScanSummary {
  added: number;
  updated: number;
  unchanged: number;
  unassigned: number;
  restored: number;
  missing: number;
}
