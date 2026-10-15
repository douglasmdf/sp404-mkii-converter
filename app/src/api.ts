import { Channel, invoke } from "@tauri-apps/api/core";

export type SaveMode = "ask" | "fixed" | "nextToSource";

export interface Settings {
  sanitizeFilenames: boolean;
  sanitizeFolderNames: boolean;
  maxFilenameLength: number | null;
  flattenFolders: boolean;
  downmixMultichannel: boolean;
  saveMode: SaveMode;
  fixedDirectory: string | null;
  workers: number;
  writeJsonLog: boolean;
}

export interface PackSummary {
  name: string;
  path: string;
  audioFiles: number;
  otherFiles: number;
  unsupportedFiles: number;
}

export type JobKind = "convert" | "copy" | "unsupported";

export interface Job {
  source: string;
  dest: string;
  kind: JobKind;
  alreadyExists: boolean;
}

export interface PackPlan {
  name: string;
  sourceRoot: string;
  destRoot: string;
  jobs: Job[];
}

export interface Plan {
  packs: PackPlan[];
}

export type Warning =
  | { kind: "clipped"; samples: number }
  | { kind: "tooLong"; seconds: number }
  | { kind: "downmixed"; channels: number };

export type FileStatus =
  | { kind: "converted" }
  | { kind: "copied" }
  | { kind: "skipped" }
  | { kind: "failed"; reason: string }
  | { kind: "cancelled" };

export interface FileResult {
  source: string;
  dest: string;
  status: FileStatus;
  warnings: Warning[];
}

export interface Summary {
  converted: number;
  copied: number;
  skipped: number;
  failed: number;
  cancelled: number;
  withWarnings: number;
  results: FileResult[];
}

export type ConvertEvent =
  | { event: "started"; total: number }
  | { event: "fileDone"; done: number; total: number; result: FileResult }
  | { event: "finished"; summary: Summary };

export interface UndoReport {
  removed: number;
  keptModified: string[];
  failed: string[];
  missing: number;
  removedDirs: number;
}

export const getSettings = () => invoke<Settings>("get_settings");
export const saveSettings = (settings: Settings) => invoke<void>("save_settings", { settings });
export const inspectPacks = (paths: string[]) => invoke<PackSummary[]>("inspect_packs", { paths });
export const planConversion = (paths: string[], outputDir: string | null) =>
  invoke<Plan>("plan_conversion", { paths, outputDir });
export const cancelConversion = () => invoke<void>("cancel_conversion");
export const undoLast = (removeEmptyDirs: boolean) => invoke<UndoReport | null>("undo_last", { removeEmptyDirs });
export const hasUndo = () => invoke<boolean>("has_undo");

export function convert(plan: Plan, onEvent: (event: ConvertEvent) => void): Promise<Summary> {
  const channel = new Channel<ConvertEvent>();
  channel.onmessage = onEvent;
  return invoke<Summary>("convert", { plan, onEvent: channel });
}
