import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ask, open } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type { FileResult, PackSummary, Plan, SaveMode, Settings, Summary, Warning } from "./api";
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
const browseBtn = byId<HTMLButtonElement>("browse-btn");
const cancelBtn = byId<HTMLButtonElement>("cancel-btn");
const settingsBtn = byId<HTMLButtonElement>("settings-btn");
const settingsDialog = byId<HTMLDialogElement>("settings-dialog");
const settingsForm = byId<HTMLFormElement>("settings-form");
const chooseFixedBtn = byId<HTMLButtonElement>("choose-fixed-btn");
const fixedDirLabel = byId<HTMLElement>("fixed-dir-label");

let packs: PackSummary[] = [];
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
  const empty = packs.length === 0;
  convertBtn.disabled = busy || empty;
  dryRunBtn.disabled = busy || empty;
  clearBtn.disabled = busy || empty;
  browseBtn.disabled = busy;
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
  dropZone.classList.toggle("compact", packs.length > 0);
  refreshButtons();
}

async function addPaths(paths: string[]) {
  const fresh = paths.filter((p) => !packs.some((pack) => pack.path === p));
  if (fresh.length === 0 || busy) return;
  try {
    packs = packs.concat(await api.inspectPacks(fresh));
    renderPacks();
    output.replaceChildren();
  } catch (error) {
    showMessage(String(error), "error");
  }
}

async function browse() {
  if (busy) return;
  const picked = await open({ directory: true, multiple: true, title: t("browse") });
  if (picked) await addPaths(Array.isArray(picked) ? picked : [picked]);
}

async function buildPlan(): Promise<Plan | undefined> {
  let outputDir: string | null = null;
  if (settings.saveMode === "ask") {
    const picked = await open({ directory: true, multiple: false, title: t("chooseOutput") });
    if (typeof picked !== "string") return undefined;
    outputDir = picked;
  }
  return api.planConversion(packs.map((p) => p.path), outputDir);
}

function renderPlan(plan: Plan) {
  const nodes: HTMLElement[] = [el("h2", t("dryRunTitle"))];
  for (const pack of plan.packs) {
    nodes.push(el("h3", `${pack.name} → ${pack.destRoot}`));
    const list = el("ul", undefined, "file-list");
    for (const job of pack.jobs) {
      const note = job.kind === "unsupported" ? ` — ${t("unsupported")}` : job.alreadyExists ? ` — ${t("willSkip")}` : "";
      const cls = job.kind === "unsupported" ? "error" : job.alreadyExists ? "muted" : undefined;
      list.append(el("li", `${relative(job.source, pack.sourceRoot)} → ${relative(job.dest, pack.destRoot)}${note}`, cls));
    }
    nodes.push(list);
  }
  output.replaceChildren(...nodes);
}

async function dryRun() {
  if (busy || packs.length === 0) return;
  try {
    const plan = await buildPlan();
    if (plan) renderPlan(plan);
  } catch (error) {
    showMessage(String(error), "error");
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
  const problems = summary.results.map(describeResult).filter((item): item is HTMLElement => item !== undefined);
  if (problems.length > 0) {
    const list = el("ul", undefined, "file-list");
    list.append(...problems);
    nodes.push(list);
  }
  output.replaceChildren(...nodes);
}

async function convertAll() {
  if (busy || packs.length === 0) return;
  let plan: Plan | undefined;
  try {
    plan = await buildPlan();
  } catch (error) {
    showMessage(String(error), "error");
    return;
  }
  if (!plan) return;
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

const BOOLEAN_SETTINGS = ["sanitizeFilenames", "sanitizeFolderNames", "flattenFolders", "downmixMultichannel", "writeJsonLog"] as const;

function field<T extends HTMLInputElement | HTMLSelectElement>(name: string): T {
  return settingsForm.elements.namedItem(name) as T;
}

function fillSettingsForm() {
  for (const key of BOOLEAN_SETTINGS) field<HTMLInputElement>(key).checked = settings[key];
  field<HTMLInputElement>("maxFilenameLength").value = settings.maxFilenameLength?.toString() ?? "";
  field<HTMLSelectElement>("saveMode").value = settings.saveMode;
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

  browseBtn.addEventListener("click", () => void browse());
  dryRunBtn.addEventListener("click", () => void dryRun());
  convertBtn.addEventListener("click", () => void convertAll());
  undoBtn.addEventListener("click", () => void undoLast());
  cancelBtn.addEventListener("click", () => void api.cancelConversion());
  clearBtn.addEventListener("click", () => {
    packs = [];
    renderPacks();
    output.replaceChildren();
  });
  settingsBtn.addEventListener("click", openSettings);
  settingsForm.addEventListener("change", () => {
    settings = readSettingsForm();
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
      void browse();
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
