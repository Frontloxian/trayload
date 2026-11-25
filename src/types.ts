export type Source =
  | { type: "github"; repo: string; assetPattern: string }
  | { type: "page"; url: string; assetPattern?: string | null }
  | { type: "forgejo"; host: string; repo: string; assetPattern: string }
  | { type: "direct"; url: string };

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
  /** Systems Trayload no longer has an emulator for. Their games stay in the library. */
  retired?: { id: string; name: string }[];
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
  leftover?: boolean;
  tier: string | null;
  warning: string | null;
}

export interface EmulatorSize {
  id: string;
  bytes: number | null;
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

export interface Collection {
  id: number;
  name: string;
  count: number;
}

export interface Membership {
  collectionId: number;
  gameId: number;
}

export interface SessionInfo {
  startedAt: number;
  seconds: number;
}

export interface GameDetails {
  extraArgs: string;
  notes: string;
  sessions: SessionInfo[];
  sessionCount: number;
}

export interface SetupItemStatus {
  label: string;
  required: boolean;
  found: boolean;
  matched: string | null;
  note: string | null;
}

export interface SystemSetup {
  systemId: string;
  systemName: string;
  emulator: string;
  installed: boolean;
  installable: boolean;
  state: "none" | "notinstalled" | "needs" | "ready";
  intro: string | null;
  folder: string | null;
  items: SetupItemStatus[];
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

export interface UpdateInfo {
  id: string;
  installed: string;
  latest: string;
}

export interface NewsItem {
  sourceId: string;
  source: string;
  tier: "release" | "official" | "community";
  title: string;
  link: string;
  date: string;
}

export interface NewsSource {
  id: string;
  name: string;
  tier: string;
  ok: boolean;
  error: string | null;
}

export interface WatchDef {
  id: string;
  title: string;
  note: string;
  keywords: string[];
}

export interface NewsPayload {
  fetchedAt: number;
  items: NewsItem[];
  sources: NewsSource[];
  watch: WatchDef[];
}

export interface MusicTrack {
  file: string;
  title: string;
}

export interface MusicPack {
  id: string;
  name: string;
  author: string | null;
  builtin: boolean;
  tracks: MusicTrack[];
}

export interface RemovedGame {
  id: number;
  title: string;
  systemId: string;
  path: string;
}

export interface RemoteStatus {
  registryRevision: number;
  compatRevision: number;
  registryUpdated: boolean;
  compatUpdated: boolean;
  online: boolean;
}

export interface CompatNote {
  system: string;
  title?: string | null;
  note: string;
  link?: string | null;
}

export interface GamesFolder {
  path: string;
  isDefault: boolean;
  exists: boolean;
}

export interface DeleteSummary {
  deleted: number;
  recycled: number;
  leftAlone: number;
  returned: number;
  failed: string[];
}

export interface OrganizeCandidate {
  id: number;
  title: string;
  systemName: string;
  path: string;
  bytes: number;
}

export interface LeftoverOriginal {
  original: string;
  dest: string;
  bytes: number;
}

export interface OrganizeSummary {
  done: number;
  failed: string[];
  cancelled: boolean;
  mode: "copy" | "move";
}

export interface LegacyStatus {
  present: boolean;
  path: string;
  totalBytes: number;
  folders: { name: string; bytes: number }[];
}

export interface CopyProgress {
  name: string;
  filesDone: number;
  filesTotal: number;
  bytesDone: number;
  bytesTotal: number;
}

export interface CopyFinished {
  copied: number;
  skipped: number;
  failed: string[];
  cancelled: boolean;
}

export interface AppInfo {
  version: string;
  dataFolder: string;
  logFolder: string;
}

/** Consoles whose games can be checked against the No-Intro lists (see verify.rs). */
export const VERIFY_SYSTEMS = ["nes", "snes", "gb", "gba", "n64", "nds", "sms", "gg", "sg1000", "genesis", "s32x"];

export interface Verification {
  status: "verified" | "unknown" | "unsupported";
  name: string;
}

export interface VerifySummary {
  verified: number;
  unknown: number;
  unsupported: number;
  skipped: number;
  noList: number;
  cancelled: boolean;
}

export interface VerifyProgress {
  done: number;
  total: number;
  title: string;
}

export interface GameInfo {
  year: string;
  developer: string;
  publisher: string;
  genre: string;
}
