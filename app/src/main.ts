import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ask, open } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type { Analysis, BpmRange, FileResult, PackSummary, Plan, SaveMode, Settings, Summary, Warning } from "./api";
import { applyI18n, t } from "./i18n";

const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const dropZone = byId<HTMLElement>("drop-zone");
const packList = byId<HTMLUListElement>("pack-list");
const output = byId<HTMLElement>("output");
const progressSection = byId<HTMLElement>("progress");
const progressBar = byId<HTMLProgressElement>("progress-bar");
const progressLabel = byId<HTMLElement>("progress-label");
const convertBtn = byId<HTMLButtonElement>("convert-btn");
const dryRunBtn = byId<HTMLButtonElement>("dry-run-btn");
const undoBtn = byId<HTMLButtonElement>("undo-btn");
const clearBtn = byId<HTMLButtonElement>("clear-btn");
const addFoldersBtn = byId<HTMLButtonElement>("add-folders-btn");
const addFilesBtn = byId<HTMLButtonElement>("add-files-btn");
const cancelBtn = byId<HTMLButtonElement>("cancel-btn");
const settingsBtn = byId<HTMLButtonElement>("settings-btn");
const settingsDialog = byId<HTMLDialogElement>("settings-dialog");
const settingsForm = byId<HTMLFormElement>("settings-form");
const chooseFixedBtn = byId<HTMLButtonElement>("choose-fixed-btn");
const fixedDirLabel = byId<HTMLElement>("fixed-dir-label");

// Keep in sync with CONVERTIBLE_EXTENSIONS and UNSUPPORTED_AUDIO_EXTENSIONS in core/src/scan.rs.
const AUDIO_EXTENSIONS = [
  "wav", "wave", "aif", "aiff", "aifc", "flac", "mp3", "m4a", "aac", "ogg", "oga", "caf",
  "opus", "wma", "ape", "wv", "mpc", "ra", "amr", "dsf", "dff",
];

let packs: PackSummary[] = [];
let looseFiles: string[] = [];
let settings: Settings;
let busy = false;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;
const relative = (path: string, root: string) =>
  path.startsWith(root) ? path.slice(root.length).replace(/^[\\/]/, "") : path;

function refreshButtons() {
  const empty = packs.length === 0 && looseFiles.length === 0;
  convertBtn.disabled = busy || empty;
  dryRunBtn.disabled = busy || empty;
  clearBtn.disabled = busy || empty;
  addFoldersBtn.disabled = busy;
  addFilesBtn.disabled = busy;
  settingsBtn.disabled = busy;
}

async function refreshUndo() {
  undoBtn.disabled = busy || !(await api.hasUndo());
}

function setBusy(value: boolean) {
  busy = value;
  progressSection.hidden = !value;
  undoBtn.disabled = value;
  refreshButtons();
}

function showMessage(text: string, kind: "info" | "error" = "info") {
  output.replaceChildren(el("p", text, kind));
}

function renderPacks() {
  packList.replaceChildren(
    ...packs.map((pack) => {
      const item = el("li");
      const remove = el("button", "×", "icon");
      remove.title = t("remove");
      remove.addEventListener("click", () => {
        packs = packs.filter((p) => p.path !== pack.path);
        renderPacks();
      });
      item.append(
        el("strong", pack.name),
        el("span", t("packFiles", { audio: pack.audioFiles, other: pack.otherFiles, unsupported: pack.unsupportedFiles }), "muted"),
        remove,
      );
      return item;
    }),
  );
  if (looseFiles.length > 0) {
    const item = el("li");
    const remove = el("button", "×", "icon");
    remove.title = t("remove");
    remove.addEventListener("click", () => {
      looseFiles = [];
      renderPacks();
    });
    item.append(el("strong", t("looseEntry", { count: looseFiles.length })), remove);
    packList.append(item);
  }
  dropZone.classList.toggle("compact", packs.length > 0 || looseFiles.length > 0);
  refreshButtons();
}

async function addPaths(paths: string[]) {
  const fresh = paths.filter((p) => !packs.some((pack) => pack.path === p) && !looseFiles.includes(p));
  if (fresh.length === 0 || busy) return;
  try {
    const summary = await api.inspectInputs(fresh);
    const folderPaths = new Set(summary.packs.map((pack) => pack.path));
    const rejected = new Set(summary.rejected);
    packs = packs.concat(summary.packs);
    for (const path of fresh) {
      if (!folderPaths.has(path) && !rejected.has(path) && !looseFiles.includes(path)) looseFiles.push(path);
    }
    renderPacks();
    if (summary.rejected.length > 0) {
      showMessage(summary.rejected.map((path) => t("notAudio", { name: fileName(path) })).join("\n"), "error");
    } else {
      output.replaceChildren();
    }
  } catch (error) {
    showMessage(String(error), "error");
  }
}

async function addFolders() {
  if (busy) return;
  const picked = await open({ directory: true, multiple: true, title: t("addFolders") });
  if (picked) await addPaths(Array.isArray(picked) ? picked : [picked]);
}

async function addFiles() {
  if (busy) return;
  const picked = await open({
    directory: false,
    multiple: true,
    title: t("addFiles"),
    filters: [{ name: t("audioFilesFilter"), extensions: AUDIO_EXTENSIONS }],
  });
  if (picked) await addPaths(Array.isArray(picked) ? picked : [picked]);
}

const CANCELLED = "cancelled";

// Events from parallel workers can arrive out of order: show the highest count seen, never less.
function analysisProgress(): (event: api.PlanEvent) => void {
  let best = 0;
  return (event) => {
    best = Math.max(best, event.done);
    progressBar.max = Math.max(event.total, 1);
    progressBar.value = best;
    progressLabel.textContent = t("analyzing", { done: best, total: event.total });
  };
}

/** Builds the plan while showing the analysis progress; `undefined` when the user cancelled anywhere. */
async function buildPlan(): Promise<Plan | undefined> {
  let outputDir: string | null = null;
  if (settings.saveMode === "ask" || looseFiles.length > 0) {
    const title = packs.length === 0 ? t("chooseOutputLoose") : t("chooseOutput");
    const picked = await open({ directory: true, multiple: false, title });
    if (typeof picked !== "string") return undefined;
    outputDir = picked;
  }
  setBusy(true);
  progressBar.max = 1;
  progressBar.value = 0;
  progressLabel.textContent = "";
  output.replaceChildren();
  try {
    return await api.planConversion([...packs.map((p) => p.path), ...looseFiles], outputDir, analysisProgress());
  } catch (error) {
    if (String(error) === CANCELLED) {
      showMessage(t("analysisCancelled"));
      return undefined;
    }
    throw error;
  }
}

const FAILURE_REASONS = { noAudio: "errNoAudio", crashed: "errCrashed", unreadable: "errUnreadable" } as const;

// A plan from an older core has no errorKind: treat the failure as an unreadable file.
function failureReason(analysis: Analysis): string {
  return t(FAILURE_REASONS[analysis.errorKind ?? "unreadable"]);
}

// Files listed (and counted) as "to check by ear": any uncertain value, or not analysed.
// The CLI counts the same thing.
function uncertaintyNotes(analysis: Analysis | undefined): string[] {
  if (!analysis) return [];
  if (analysis.error) return [t("notAnalyzed", { reason: failureReason(analysis) })];
  const notes: string[] = [];
  if (analysis.labelUncertain) notes.push(t("keyUncertain"));
  if (analysis.bpmUncertain) notes.push(t("bpmUncertain"));
  if (analysis.kindUncertain) notes.push(t("typeUncertain"));
  return notes;
}

function renderPlan(plan: Plan) {
  const nodes: HTMLElement[] = [el("h2", t("dryRunTitle"))];
  for (const pack of plan.packs) {
    nodes.push(el("h3", `${pack.name} → ${pack.destRoot}`));
    const list = el("ul", undefined, "file-list");
    for (const job of pack.jobs) {
      const uncertain = uncertaintyNotes(job.analysis);
      let note = job.kind === "unsupported" ? ` — ${t("unsupported")}` : job.alreadyExists ? ` — ${t("willSkip")}` : "";
      if (uncertain.length > 0) note += ` — ${uncertain.join("; ")}`;
      let cls = job.kind === "unsupported" ? "error" : job.alreadyExists ? "muted" : undefined;
      if (!cls && uncertain.length > 0) cls = "warning";
      list.append(el("li", `${pack.loose ? job.source : relative(job.source, pack.sourceRoot)} → ${relative(job.dest, pack.destRoot)}${note}`, cls));
    }
    nodes.push(list);
  }
  output.replaceChildren(...nodes);
}

async function dryRun() {
  if (busy || (packs.length === 0 && looseFiles.length === 0)) return;
  // Claim before any await (the folder dialog) so a double click cannot start a second run.
  busy = true;
  refreshButtons();
  try {
    const plan = await buildPlan();
    if (plan) renderPlan(plan);
  } catch (error) {
    showMessage(String(error), "error");
  } finally {
    setBusy(false);
  }
}

function setProgress(done: number, total: number) {
  progressBar.max = Math.max(total, 1);
  progressBar.value = done;
  progressLabel.textContent = t("progress", { done, total });
}

function describeWarning(warning: Warning): string {
  switch (warning.kind) {
    case "clipped":
      return t("warnClipped", { samples: warning.samples });
    case "tooLong":
      return t("warnTooLong", { minutes: (warning.seconds / 60).toFixed(1) });
    case "downmixed":
      return t("warnDownmixed", { channels: warning.channels });
  }
}

function describeResult(result: FileResult): HTMLElement | undefined {
  const parts: string[] = [];
  if (result.status.kind === "failed") parts.push(result.status.reason);
  parts.push(...result.warnings.map(describeWarning));
  if (parts.length === 0) return undefined;
  return el("li", `${fileName(result.source)} — ${parts.join("; ")}`, result.status.kind === "failed" ? "error" : "warning");
}

function renderSummary(summary: Summary) {
  const nodes: HTMLElement[] = [
    el("h2", t("summaryTitle")),
    el(
      "p",
      t("summaryCounts", {
        converted: summary.converted,
        copied: summary.copied,
        skipped: summary.skipped,
        failed: summary.failed,
        cancelled: summary.cancelled,
        warnings: summary.withWarnings,
      }),
    ),
  ];
  const uncertain = summary.results.filter((result) => uncertaintyNotes(result.analysis).length > 0);
  if (uncertain.length > 0) {
    nodes.push(el("p", t("uncertainCount", { count: uncertain.length }), "warning"));
    const list = el("ul", undefined, "file-list");
    list.append(...uncertain.map((result) => el("li", `${fileName(result.dest)} — ${uncertaintyNotes(result.analysis).join("; ")}`, "warning")));
    nodes.push(list);
  }
  const problems = summary.results.map(describeResult).filter((item): item is HTMLElement => item !== undefined);
  if (problems.length > 0) {
    const list = el("ul", undefined, "file-list");
    list.append(...problems);
    nodes.push(list);
  }
  output.replaceChildren(...nodes);
}

async function convertAll() {
  if (busy || (packs.length === 0 && looseFiles.length === 0)) return;
  // Claim before any await so a double click or a repeated shortcut cannot start a second run.
  busy = true;
  refreshButtons();
  let plan: Plan | undefined;
  try {
    plan = await buildPlan();
  } catch (error) {
    showMessage(String(error), "error");
  }
  if (!plan) {
    setBusy(false);
    return;
  }
  setBusy(true);
  setProgress(0, 0);
  output.replaceChildren();
  try {
    const summary = await api.convert(plan, (event) => {
      if (event.event === "started") setProgress(0, event.total);
      else if (event.event === "fileDone") setProgress(event.done, event.total);
    });
    renderSummary(summary);
  } catch (error) {
    showMessage(String(error), "error");
  } finally {
    setBusy(false);
    await refreshUndo();
  }
}

async function undoLast() {
  if (busy) return;
  if (!(await ask(t("undoConfirm"), { title: t("undo"), kind: "warning" }))) return;
  const removeEmptyDirs = await ask(t("undoRemoveDirs"), { title: t("undo") });
  try {
    const report = await api.undoLast(removeEmptyDirs);
    showMessage(
      report
        ? t("undoDone", { removed: report.removed, dirs: report.removedDirs, kept: report.keptModified.length + report.failed.length })
        : t("nothingToUndo"),
    );
  } catch (error) {
    showMessage(String(error), "error");
  }
  await refreshUndo();
}

const BOOLEAN_SETTINGS = ["sanitizeFilenames", "sanitizeFolderNames", "flattenFolders", "downmixMultichannel", "writeJsonLog", "detectKeyBpm"] as const;

function field<T extends HTMLInputElement | HTMLSelectElement>(name: string): T {
  return settingsForm.elements.namedItem(name) as T;
}

function fillSettingsForm() {
  for (const key of BOOLEAN_SETTINGS) field<HTMLInputElement>(key).checked = settings[key];
  field<HTMLInputElement>("maxFilenameLength").value = settings.maxFilenameLength?.toString() ?? "";
  field<HTMLSelectElement>("saveMode").value = settings.saveMode;
  field<HTMLSelectElement>("bpmRange").value = settings.bpmRange;
  field<HTMLSelectElement>("bpmRange").disabled = !settings.detectKeyBpm;
  field<HTMLInputElement>("workers").value = String(settings.workers);
  fixedDirLabel.textContent = settings.fixedDirectory ?? t("notSet");
}

function readSettingsForm(): Settings {
  const maxLength = parseInt(field<HTMLInputElement>("maxFilenameLength").value, 10);
  const workers = parseInt(field<HTMLInputElement>("workers").value, 10);
  const next: Settings = {
    ...settings,
    maxFilenameLength: Number.isFinite(maxLength) && maxLength > 0 ? maxLength : null,
    saveMode: field<HTMLSelectElement>("saveMode").value as SaveMode,
    bpmRange: field<HTMLSelectElement>("bpmRange").value as BpmRange,
    workers: Number.isFinite(workers) && workers > 0 ? Math.min(workers, 32) : 1,
  };
  for (const key of BOOLEAN_SETTINGS) next[key] = field<HTMLInputElement>(key).checked;
  return next;
}

async function persistSettings() {
  try {
    await api.saveSettings(settings);
  } catch (error) {
    showMessage(String(error), "error");
  }
}

function openSettings() {
  if (busy) return;
  fillSettingsForm();
  settingsDialog.showModal();
}

async function init() {
  applyI18n();
  settings = await api.getSettings();
  renderPacks();
  await refreshUndo();

  addFoldersBtn.addEventListener("click", () => void addFolders());
  addFilesBtn.addEventListener("click", () => void addFiles());
  dryRunBtn.addEventListener("click", () => void dryRun());
  convertBtn.addEventListener("click", () => void convertAll());
  undoBtn.addEventListener("click", () => void undoLast());
  cancelBtn.addEventListener("click", () => void api.cancelConversion());
  clearBtn.addEventListener("click", () => {
    packs = [];
    looseFiles = [];
    renderPacks();
    output.replaceChildren();
  });
  settingsBtn.addEventListener("click", openSettings);
  settingsForm.addEventListener("change", () => {
    settings = readSettingsForm();
    field<HTMLSelectElement>("bpmRange").disabled = !settings.detectKeyBpm;
    void persistSettings();
  });
  chooseFixedBtn.addEventListener("click", async () => {
    const picked = await open({ directory: true, multiple: false, title: t("sFixed") });
    if (typeof picked !== "string") return;
    settings = { ...readSettingsForm(), fixedDirectory: picked, saveMode: "fixed" };
    fillSettingsForm();
    await persistSettings();
  });

  window.addEventListener("keydown", (event) => {
    if (!(event.metaKey || event.ctrlKey)) return;
    if (event.key.toLowerCase() === "o") {
      event.preventDefault();
      void (event.shiftKey ? addFiles() : addFolders());
    } else if (event.key === "Enter") {
      event.preventDefault();
      void convertAll();
    } else if (event.key === ",") {
      event.preventDefault();
      openSettings();
    }
  });

  await getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "enter" || payload.type === "over") {
      dropZone.classList.add("hover");
    } else if (payload.type === "leave") {
      dropZone.classList.remove("hover");
    } else if (payload.type === "drop") {
      dropZone.classList.remove("hover");
      void addPaths(payload.paths);
    }
  });
}

void init();
