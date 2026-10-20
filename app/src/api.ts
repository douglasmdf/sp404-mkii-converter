import { Channel, invoke } from "@tauri-apps/api/core";

export type BpmRange = "58-115" | "68-135" | "78-166" | "88-175" | "98-195";

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
  detectKeyBpm: boolean;
  bpmRange: BpmRange;
}

export interface PackSummary {
  name: string;
  path: string;
  audioFiles: number;
  otherFiles: number;
  unsupportedFiles: number;
}

export interface InputsSummary {
  packs: PackSummary[];
  loose: PackSummary | null;
  rejected: string[];
}

export type SampleKind = "unpitched" | "note" | "chord" | "phrase" | "melodicLoop" | "drumLoop";

export interface Analysis {
  kind: SampleKind;
  label: string | null;
  bpm: number | null;
  kindUncertain: boolean;
  labelUncertain: boolean;
  bpmUncertain: boolean;
  labelFromName: boolean;
  bpmFromName: boolean;
  prefix: string | null;
  /** Why the file could not be analysed, in English (CLI, JSON log). */
  error: string | null;
  /** The same reason as a code the app translates. */
  errorKind?: AnalysisErrorKind;
  analyzerVersion: number;
}

export type AnalysisErrorKind = "noAudio" | "crashed" | "unreadable";

export type JobKind = "convert" | "copy" | "unsupported";

export interface Job {
  source: string;
  dest: string;
  kind: JobKind;
  alreadyExists: boolean;
  analysis?: Analysis;
}

export interface PackPlan {
  name: string;
  sourceRoot: string;
  destRoot: string;
  jobs: Job[];
  loose: boolean;
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
  analysis?: Analysis;
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

export type PlanEvent = { event: "analyzing"; done: number; total: number };

export interface UndoReport {
  removed: number;
  keptModified: string[];
  failed: string[];
  missing: number;
  removedDirs: number;
}

export const getSettings = () => invoke<Settings>("get_settings");
export const saveSettings = (settings: Settings) => invoke<void>("save_settings", { settings });
export const inspectInputs = (paths: string[]) => invoke<InputsSummary>("inspect_inputs", { paths });
export function planConversion(
  paths: string[],
  outputDir: string | null,
  onEvent: (event: PlanEvent) => void,
): Promise<Plan> {
  const channel = new Channel<PlanEvent>();
  channel.onmessage = onEvent;
  return invoke<Plan>("plan_conversion", { paths, outputDir, onEvent: channel });
}
export const cancelConversion = () => invoke<void>("cancel_conversion");
export const undoLast = (removeEmptyDirs: boolean) => invoke<UndoReport | null>("undo_last", { removeEmptyDirs });
export const hasUndo = () => invoke<boolean>("has_undo");

export function convert(plan: Plan, onEvent: (event: ConvertEvent) => void): Promise<Summary> {
  const channel = new Channel<ConvertEvent>();
  channel.onmessage = onEvent;
  return invoke<Summary>("convert", { plan, onEvent: channel });
}
