export type Source =
  | { type: "github"; repo: string; assetPattern: string }
  | { type: "page"; url: string; assetPattern?: string | null };

export interface System {
  id: string;
  name: string;
  emulator: string;
  source: Source;
  exts: string[];
}

export interface Registry {
  systems: System[];
}
