import "@fontsource/atkinson-hyperlegible-next/400.css";
import "@fontsource/atkinson-hyperlegible-next/700.css";
import "./styles.css";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ask, open } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type { Analysis, BpmRange, FileResult, Job, PackPlan, PackSummary, Plan, SampleKind, SaveMode, Settings, Summary, Warning } from "./api";
import { applyI18n, t, type I18nKey } from "./i18n";
import { hydrateIcons, icon, type IconName } from "./icons";

const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const packList = byId<HTMLUListElement>("pack-list");
const railEmpty = byId<HTMLElement>("rail-empty");
const message = byId<HTMLElement>("message");
const arrival = byId<HTMLElement>("arrival");
const route = byId<HTMLElement>("route");
const routeDest = byId<HTMLElement>("route-dest");
const routeKeyBpm = byId<HTMLElement>("route-keybpm");
const busySign = byId<HTMLElement>("busy-sign");
const busyLabel = byId<HTMLElement>("busy-label");
const busySub = byId<HTMLElement>("busy-sub");
const results = byId<HTMLElement>("results");
const resultsTitle = byId<HTMLElement>("results-title");
const resultsCounts = byId<HTMLElement>("results-counts");
const tabs = byId<HTMLElement>("tabs");
const tableBody = byId<HTMLTableElement>("result-table").tBodies[0];
const tableEmpty = byId<HTMLElement>("table-empty");
const gateNumber = byId<HTMLElement>("gate-number");
const gateLabel = byId<HTMLElement>("gate-label");
const progress = byId<HTMLElement>("progress");
const progressFill = byId<HTMLElement>("progress-fill");
const footerHint = byId<HTMLElement>("footer-hint");
const convertBtn = byId<HTMLButtonElement>("convert-btn");
const dryRunBtn = byId<HTMLButtonElement>("dry-run-btn");
const undoBtn = byId<HTMLButtonElement>("undo-btn");
const clearBtn = byId<HTMLButtonElement>("clear-btn");
const addFoldersBtn = byId<HTMLButtonElement>("add-folders-btn");
const addFilesBtn = byId<HTMLButtonElement>("add-files-btn");
const cancelBtn = byId<HTMLButtonElement>("cancel-btn");
const settingsBtn = byId<HTMLButtonElement>("settings-btn");
const routeSettingsBtn = byId<HTMLButtonElement>("route-settings-btn");
const settingsDialog = byId<HTMLDialogElement>("settings-dialog");
const settingsForm = byId<HTMLFormElement>("settings-form");
const chooseFixedBtn = byId<HTMLButtonElement>("choose-fixed-btn");
const fixedDirLabel = byId<HTMLElement>("fixed-dir-label");

// Keep in sync with CONVERTIBLE_EXTENSIONS and UNSUPPORTED_AUDIO_EXTENSIONS in core/src/scan.rs.
const AUDIO_EXTENSIONS = [
  "wav", "wave", "aif", "aiff", "aifc", "flac", "mp3", "m4a", "aac", "ogg", "oga", "caf",
  "opus", "wma", "ape", "wv", "mpc", "ra", "amr", "dsf", "dff",
];

/** Where the user is on the way from a pack to the SD card; drives the sign band. */
type Stage = "empty" | "ready" | "analyzing" | "reviewed" | "converting" | "done";
type RowState = "ok" | "copy" | "check" | "skip" | "warn" | "error";
type Tab = "all" | "check" | "skipped" | "problems";
type View = { kind: "plan"; plan: Plan } | { kind: "summary"; plan: Plan; summary: Summary };

/** What the user says an uncertain file is: two choices that map to one SampleKind. */
type Shape = "oneShot" | "loop" | "full";
type Sound = "drum" | "melodic" | "note" | "chord";
interface KindChoice {
  shape: Shape;
  sound: Sound;
}

interface Row {
  /** Shown with the format/sound fields: the type was uncertain, or the user already chose one. */
  kindChoice: KindChoice | null;
  source: string;
  /** Full source path, shown as a tooltip (loose files list only their name). */
  sourcePath: string;
  target: string;
  prefix: string | null;
  state: RowState;
  /** Uncertain key/BPM/type, or not analysed: listed under "check by ear". */
  uncertain: boolean;
  notes: string[];
}

interface Group {
  pack: PackPlan;
  rows: Row[];
}

let packs: PackSummary[] = [];
let looseFiles: string[] = [];
let settings: Settings;
let busy = false;
let stage: Stage = "empty";
let pendingAction: "dryRun" | "convert" = "dryRun";
let view: View | null = null;
let activeTab: Tab = "all";
/** The user's type choices, by source path; sent with every plan until the entries are cleared. */
const kindChoices = new Map<string, KindChoice>();
/** The output folder picked for the last Dry Run, reused when a type choice re-plans. */
let lastOutputDir: string | null = null;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string, className?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
}

function pict(name: IconName, className = "pict"): HTMLElement {
  const box = el("span", undefined, className);
  box.append(icon(name));
  return box;
}

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;
const relative = (path: string, root: string) =>
  path.startsWith(root) ? path.slice(root.length).replace(/^[\\/]/, "") : path;
const hasEntries = () => packs.length > 0 || looseFiles.length > 0;

// ---------- the sign band ----------

const STEPS = ["add", "review", "convert", "card"] as const;

function currentStep(): (typeof STEPS)[number] {
  switch (stage) {
    case "empty":
      return "add";
    case "ready":
      return "review";
    case "analyzing":
      return pendingAction === "dryRun" ? "review" : "convert";
    case "reviewed":
    case "converting":
      return "convert";
    case "done":
      return "card";
  }
}

function setGate(number: string, label: string) {
  const resigned = gateLabel.textContent !== label;
  gateNumber.textContent = number;
  gateLabel.textContent = label;
  // The band re-signs itself when the step changes: a split-flap turn, not on every progress tick.
  if (resigned) {
    gateNumber.classList.remove("flap");
    void gateNumber.offsetWidth;
    gateNumber.classList.add("flap");
  }
}

function audioFileCount(): number {
  return packs.reduce((sum, pack) => sum + pack.audioFiles, 0) + looseFiles.length;
}

function plannedWrites(plan: Plan): number {
  return plan.packs.reduce(
    (sum, pack) => sum + pack.jobs.filter((job) => job.kind !== "unsupported" && !job.alreadyExists).length,
    0,
  );
}

function renderBand() {
  const now = STEPS.indexOf(currentStep());
  document.querySelectorAll<HTMLLIElement>("#steps li").forEach((item, index) => {
    item.classList.toggle("done", index < now);
    item.classList.toggle("current", index === now);
    if (index === now) item.setAttribute("aria-current", "step");
    else item.removeAttribute("aria-current");
    const box = item.querySelector(".pict")!;
    box.replaceChildren(icon(index < now ? "check" : STEPS[index]));
  });
  if (stage === "empty") setGate("0", t("gateEmpty"));
  else if (stage === "ready") setGate(String(audioFileCount()), t("gateAudio"));
  else if (stage === "reviewed" && view) setGate(String(plannedWrites(view.plan)), t("gatePlanned"));
  else if (stage === "done" && view?.kind === "summary") setGate(String(view.summary.converted + view.summary.copied), t("gateDone"));
  document.body.dataset.stage = stage;
}

function setProgress(done: number, total: number, label: string) {
  progress.hidden = false;
  progressFill.style.width = `${total > 0 ? Math.min(100, (done / total) * 100) : 0}%`;
  setGate(`${done}/${total}`, label);
}

// ---------- state and layout ----------

function setStage(next: Stage) {
  stage = next;
  render();
}

function render() {
  const working = stage === "analyzing" || stage === "converting";
  arrival.hidden = stage !== "empty";
  route.hidden = stage !== "ready";
  busySign.hidden = !working;
  results.hidden = !(stage === "reviewed" || stage === "done") || !view;
  progress.hidden = !working;
  footerHint.textContent = stage === "reviewed" ? t("nothingWritten") : "";
  if (stage === "ready") renderRoute();
  if (!results.hidden) renderResults();
  renderBand();
  refreshButtons();
}

function refreshButtons() {
  const working = stage === "analyzing" || stage === "converting";
  convertBtn.hidden = working;
  dryRunBtn.hidden = working;
  cancelBtn.hidden = !working;
  convertBtn.disabled = busy || !hasEntries();
  dryRunBtn.disabled = busy || !hasEntries();
  clearBtn.disabled = busy || (!hasEntries() && !view);
  addFoldersBtn.disabled = busy;
  addFilesBtn.disabled = busy;
  settingsBtn.disabled = busy;
  packList.querySelectorAll("button").forEach((button) => (button.disabled = busy));
}

async function refreshUndo() {
  undoBtn.disabled = busy || !(await api.hasUndo());
}

function setBusy(value: boolean) {
  busy = value;
  undoBtn.disabled = value;
  refreshButtons();
}

function showMessage(text: string, kind: "info" | "error" = "info") {
  message.className = kind;
  message.replaceChildren(pict(kind === "error" ? "error" : "warning"), el("p", text));
  message.hidden = false;
}

function clearMessage() {
  message.hidden = true;
  message.replaceChildren();
}

/** New or removed entries invalidate any plan or summary on screen. */
function entriesChanged() {
  view = null;
  renderPacks();
  setStage(hasEntries() ? "ready" : "empty");
}

// ---------- entries rail ----------

function entryItem(name: string, detail: string | null, onRemove: () => void): HTMLLIElement {
  const item = el("li");
  const text = el("div");
  text.append(el("strong", name));
  if (detail) text.append(el("span", detail));
  const remove = el("button", undefined, "pict-btn small");
  remove.append(icon("x"));
  remove.title = t("remove");
  remove.setAttribute("aria-label", `${t("remove")} ${name}`);
  remove.addEventListener("click", onRemove);
  item.append(text, remove);
  return item;
}

function renderPacks() {
  const items = packs.map((pack) =>
    entryItem(pack.name, t("packFiles", { audio: pack.audioFiles, other: pack.otherFiles, unsupported: pack.unsupportedFiles }), () => {
      packs = packs.filter((p) => p.path !== pack.path);
      entriesChanged();
    }),
  );
  if (looseFiles.length > 0) {
    items.push(
      entryItem(t("looseEntry", { count: looseFiles.length }), null, () => {
        looseFiles = [];
        entriesChanged();
      }),
    );
  }
  packList.replaceChildren(...items);
  railEmpty.hidden = items.length > 0;
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
    entriesChanged();
    if (summary.rejected.length > 0) {
      showMessage(summary.rejected.map((path) => t("notAudio", { name: fileName(path) })).join("\n"), "error");
    } else {
      clearMessage();
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

// ---------- route sign (entries added, nothing planned yet) ----------

function renderRoute() {
  const destinations: string[] = [];
  if (packs.length > 0) {
    if (settings.saveMode === "fixed" && settings.fixedDirectory) destinations.push(t("routeDestFixed", { path: settings.fixedDirectory }));
    else if (settings.saveMode === "nextToSource") destinations.push(t("routeDestNext"));
    else destinations.push(t("routeDestAsk"));
  }
  if (looseFiles.length > 0) destinations.push(t("routeDestLoose"));
  routeDest.textContent = destinations.join(" ");
  routeKeyBpm.textContent = settings.detectKeyBpm ? t("keyBpmOn", { range: settings.bpmRange.replace("-", "–") }) : t("keyBpmOff");
}

// ---------- planning and conversion ----------

const CANCELLED = "cancelled";

// Events from parallel workers can arrive out of order: show the highest count seen, never less.
function analysisProgress(): (event: api.PlanEvent) => void {
  let best = 0;
  return (event) => {
    best = Math.max(best, event.done);
    setProgress(best, event.total, t("gateAnalyzing"));
    busyLabel.textContent = t("analyzing", { done: best, total: event.total });
  };
}

/** Builds the plan while showing the analysis progress; `undefined` when the user cancelled anywhere. */
// Full sample/song has no kind of its own in the core: it is named like a loop.
function kindOf(choice: KindChoice): SampleKind {
  if (choice.shape === "oneShot") {
    return { drum: "unpitched", melodic: "phrase", note: "note", chord: "chord" }[choice.sound] as SampleKind;
  }
  return choice.sound === "drum" ? "drumLoop" : "melodicLoop";
}

function choiceOf(kind: SampleKind): KindChoice {
  switch (kind) {
    case "unpitched":
      return { shape: "oneShot", sound: "drum" };
    case "note":
      return { shape: "oneShot", sound: "note" };
    case "chord":
      return { shape: "oneShot", sound: "chord" };
    case "phrase":
      return { shape: "oneShot", sound: "melodic" };
    case "melodicLoop":
      return { shape: "loop", sound: "melodic" };
    case "drumLoop":
      return { shape: "loop", sound: "drum" };
  }
}

function chosenKinds(): Record<string, SampleKind> {
  return Object.fromEntries([...kindChoices].map(([path, choice]) => [path, kindOf(choice)]));
}

function inputPaths(): string[] {
  return [...packs.map((p) => p.path), ...looseFiles];
}

async function buildPlan(): Promise<Plan | undefined> {
  let outputDir: string | null = null;
  if (settings.saveMode === "ask" || looseFiles.length > 0) {
    const title = packs.length === 0 ? t("chooseOutputLoose") : t("chooseOutput");
    const picked = await open({ directory: true, multiple: false, title });
    if (typeof picked !== "string") return undefined;
    outputDir = picked;
  }
  lastOutputDir = outputDir;
  setBusy(true);
  clearMessage();
  busyLabel.textContent = t("analyzing", { done: 0, total: audioFileCount() });
  busySub.textContent = t("busyAnalyzeSub");
  setStage("analyzing");
  setProgress(0, audioFileCount(), t("gateAnalyzing"));
  try {
    return await api.planConversion(inputPaths(), outputDir, chosenKinds(), analysisProgress());
  } catch (error) {
    if (String(error) === CANCELLED) {
      showMessage(t("analysisCancelled"));
      return undefined;
    }
    throw error;
  }
}

/** A type choice changed: re-plan in place (only that file is analysed again), keeping the
 *  output folder, the tab and the scroll position. */
async function replanForChoice() {
  if (busy || view?.kind !== "plan") return;
  busy = true;
  refreshButtons();
  tableBody.querySelectorAll("select").forEach((select) => (select.disabled = true));
  const wrap = byId<HTMLElement>("table-wrap");
  const scroll = wrap.scrollTop;
  try {
    const plan = await api.planConversion(inputPaths(), lastOutputDir, chosenKinds(), () => {});
    view = { kind: "plan", plan };
  } catch (error) {
    showMessage(String(error), "error");
  } finally {
    busy = false;
    render();
    wrap.scrollTop = scroll;
  }
}

function backToEntries() {
  setStage(view ? (view.kind === "summary" ? "done" : "reviewed") : hasEntries() ? "ready" : "empty");
}

async function dryRun() {
  if (busy || !hasEntries()) return;
  // Claim before any await (the folder dialog) so a double click cannot start a second run.
  busy = true;
  pendingAction = "dryRun";
  refreshButtons();
  try {
    const plan = await buildPlan();
    if (plan) {
      view = { kind: "plan", plan };
      activeTab = "all";
    }
  } catch (error) {
    showMessage(String(error), "error");
  } finally {
    setBusy(false);
    backToEntries();
  }
}

async function convertAll() {
  if (busy || !hasEntries()) return;
  // Claim before any await so a double click or a repeated shortcut cannot start a second run.
  busy = true;
  pendingAction = "convert";
  refreshButtons();
  let plan: Plan | undefined;
  try {
    plan = await buildPlan();
  } catch (error) {
    showMessage(String(error), "error");
  }
  if (!plan) {
    setBusy(false);
    backToEntries();
    return;
  }
  const converting = plan;
  setBusy(true);
  busyLabel.textContent = t("progress", { done: 0, total: 0 });
  busySub.textContent = t("busyConvertSub");
  setStage("converting");
  setProgress(0, 0, t("gateConverting"));
  try {
    const summary = await api.convert(converting, (event) => {
      if (event.event === "started" || event.event === "fileDone") {
        const done = event.event === "started" ? 0 : event.done;
        setProgress(done, event.total, t("gateConverting"));
        busyLabel.textContent = t("progress", { done, total: event.total });
      }
    });
    view = { kind: "summary", plan: converting, summary };
    activeTab = "all";
  } catch (error) {
    showMessage(String(error), "error");
  } finally {
    setBusy(false);
    backToEntries();
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

// ---------- results: the plan (Dry Run) or the summary ----------

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

function sourceLabel(pack: PackPlan, path: string): string {
  return pack.loose ? fileName(path) : relative(path, pack.sourceRoot);
}

function planRow(pack: PackPlan, job: Job): Row {
  const uncertain = uncertaintyNotes(job.analysis);
  const row: Row = {
    source: sourceLabel(pack, job.source),
    sourcePath: job.source,
    target: relative(job.dest, pack.destRoot),
    prefix: job.analysis?.prefix ?? null,
    state: job.kind === "copy" ? "copy" : "ok",
    uncertain: uncertain.length > 0,
    notes: [],
    kindChoice: null,
  };
  const analysis = job.analysis;
  if (analysis && !analysis.error && (analysis.kindUncertain || kindChoices.has(job.source))) {
    row.kindChoice = kindChoices.get(job.source) ?? choiceOf(analysis.kind);
  }
  if (job.kind === "unsupported") {
    row.state = "error";
    row.notes.push(t("unsupported"));
  } else if (job.alreadyExists) {
    // Already converted: listed under "Skipped", nothing to report on the row itself.
    row.state = "skip";
  }
  if (uncertain.length > 0) {
    row.notes.push(...uncertain);
    if (row.state === "ok" || row.state === "copy") row.state = "check";
  }
  return row;
}

function summaryRow(pack: PackPlan, result: FileResult): Row {
  const uncertain = uncertaintyNotes(result.analysis);
  const row: Row = {
    source: sourceLabel(pack, result.source),
    sourcePath: result.source,
    target: relative(result.dest, pack.destRoot),
    prefix: result.analysis?.prefix ?? null,
    state: result.status.kind === "copied" ? "copy" : "ok",
    uncertain: uncertain.length > 0,
    notes: [],
    kindChoice: null,
  };
  if (result.status.kind === "failed") {
    row.state = "error";
    row.notes.push(result.status.reason);
  } else if (result.status.kind === "skipped") {
    row.state = "skip";
  } else if (result.status.kind === "cancelled") {
    row.state = "skip";
    row.notes.push(t("cancelledNote"));
  }
  if (result.warnings.length > 0) {
    row.notes.push(...result.warnings.map(describeWarning));
    if (row.state !== "error") row.state = "warn";
  }
  if (uncertain.length > 0) {
    row.notes.push(...uncertain);
    if (row.state === "ok" || row.state === "copy") row.state = "check";
  }
  return row;
}

function groups(current: View): Group[] {
  if (current.kind === "plan") {
    return current.plan.packs.map((pack) => ({ pack, rows: pack.jobs.map((job) => planRow(pack, job)) }));
  }
  const byPack = new Map<PackPlan, Row[]>(current.plan.packs.map((pack) => [pack, []]));
  for (const result of current.summary.results) {
    const pack = current.plan.packs.find((p) => result.dest.startsWith(p.destRoot)) ?? current.plan.packs[0];
    byPack.get(pack)?.push(summaryRow(pack, result));
  }
  return [...byPack].map(([pack, rows]) => ({ pack, rows }));
}

const TAB_FILTERS: Record<Tab, (row: Row) => boolean> = {
  all: () => true,
  check: (row) => row.uncertain,
  skipped: (row) => row.state === "skip",
  problems: (row) => row.state === "error" || row.state === "warn",
};

const TAB_LABELS: Record<Tab, I18nKey> = { all: "tabAll", check: "tabCheck", skipped: "tabSkipped", problems: "tabProblems" };
const STATE_ICONS: Record<RowState, IconName> = {
  ok: "check",
  copy: "check",
  check: "ear",
  skip: "skip",
  warn: "warning",
  error: "error",
};

/** Long paths break only after a "/" (a <wbr>), never in the middle of a word. */
function sourceCell(path: string, fullPath: string): HTMLTableCellElement {
  const cell = el("td", undefined, "source");
  cell.title = fullPath;
  path.split("/").forEach((part, index, parts) => {
    cell.append(document.createTextNode(index < parts.length - 1 ? `${part}/` : part));
    if (index < parts.length - 1) cell.append(document.createElement("wbr"));
  });
  return cell;
}

function folderOf(path: string): string {
  return path.slice(0, path.length - fileName(path).length);
}

function targetCell(row: Row): HTMLTableCellElement {
  const cell = el("td", undefined, "target");
  const name = fileName(row.target);
  // The folder only matters when it differs from the source's (cleaned or flattened folders).
  const folder = folderOf(row.target);
  if (folder && folder !== folderOf(row.source)) cell.append(el("span", folder, "folder"));
  const label = el("span", undefined, "name");
  if (row.prefix && name.startsWith(`${row.prefix}_`)) {
    label.append(el("span", row.prefix, "tag"), document.createTextNode(name.slice(row.prefix.length)));
  } else {
    label.append(document.createTextNode(name));
  }
  cell.append(label);
  // Narrow windows fold the note column into a second line under the name.
  if (row.notes.length > 0) cell.append(el("span", row.notes.join(" · "), "note-inline"));
  // The type fields sit right under the name they change.
  if (row.kindChoice) cell.append(kindFields(row.sourcePath, row.kindChoice));
  return cell;
}

const SHAPES: [Shape, I18nKey][] = [["oneShot", "shapeOneShot"], ["loop", "shapeLoop"], ["full", "shapeFull"]];
const SOUNDS: [Sound, I18nKey][] = [["drum", "soundDrum"], ["melodic", "soundMelodic"], ["note", "soundNote"], ["chord", "soundChord"]];

function kindSelect<T extends string>(label: I18nKey, options: [T, I18nKey][], value: T, onChange: (value: T) => void): HTMLLabelElement {
  const field = el("label", undefined, "kind-field");
  const select = el("select");
  for (const [optionValue, optionLabel] of options) {
    const option = el("option", t(optionLabel));
    option.value = optionValue;
    select.append(option);
  }
  select.value = value;
  select.addEventListener("change", () => onChange(select.value as T));
  field.append(el("span", t(label)), select);
  return field;
}

function kindFields(path: string, choice: KindChoice): HTMLElement {
  const box = el("div", undefined, "kind-fields");
  const choose = (next: KindChoice) => {
    kindChoices.set(path, next);
    void replanForChoice();
  };
  box.append(
    kindSelect("kShape", SHAPES, choice.shape, (shape) => choose({ ...choice, shape })),
    kindSelect("kSound", SOUNDS, choice.sound, (sound) => choose({ ...choice, sound })),
  );
  return box;
}

function noteCell(row: Row): HTMLTableCellElement {
  if (row.notes.length > 0) return el("td", row.notes.join(" · "), "note");
  return el("td", t("okNote"), "note ok");
}

function renderResults() {
  if (!view) return;
  const all = groups(view);
  const flat = all.flatMap((group) => group.rows);
  const isPlan = view.kind === "plan";
  resultsTitle.textContent = isPlan ? t("dryRunTitle") : t("summaryTitle");
  const counters: [number, I18nKey, string?][] =
    view.kind === "summary"
      ? [
          [view.summary.converted, "cntConverted"],
          [view.summary.copied, "cntCopied"],
          [view.summary.skipped, "cntSkipped"],
          [flat.filter(TAB_FILTERS.check).length, "cntCheck", "check"],
          [view.summary.withWarnings, "cntWarnings", "warn"],
          [view.summary.failed, "cntFailed", "error"],
          ...(view.summary.cancelled > 0 ? [[view.summary.cancelled, "cntCancelled"] as [number, I18nKey]] : []),
        ]
      : [
          [plannedWrites(view.plan), "cntWrite"],
          [flat.filter((row) => row.state === "skip").length, "cntExists"],
          [flat.filter(TAB_FILTERS.check).length, "cntCheck", "check"],
          [flat.filter((row) => row.state === "error").length, "cntUnsupported", "error"],
        ];
  resultsCounts.replaceChildren(
    ...counters.map(([count, label, tone]) => {
      const item = el("div", undefined, `counter${tone && count > 0 ? ` tone-${tone}` : ""}`);
      item.append(el("b", String(count)), el("span", t(label)));
      return item;
    }),
  );

  tabs.replaceChildren(
    ...(Object.keys(TAB_FILTERS) as Tab[]).map((tab) => {
      const count = flat.filter(TAB_FILTERS[tab]).length;
      const button = el("button", undefined, "tab");
      button.setAttribute("role", "tab");
      button.setAttribute("aria-selected", String(tab === activeTab));
      button.append(el("span", t(TAB_LABELS[tab])), el("span", String(count), "count"));
      button.disabled = tab !== "all" && count === 0;
      button.addEventListener("click", () => {
        activeTab = tab;
        renderResults();
      });
      return button;
    }),
  );

  const filter = TAB_FILTERS[activeTab];
  const rows: HTMLTableRowElement[] = [];
  for (const group of all) {
    const visible = group.rows.filter(filter);
    if (visible.length === 0) continue;
    const head = el("tr", undefined, "group");
    const cell = el("th");
    cell.colSpan = 4;
    cell.scope = "colgroup";
    const lockup = el("div", undefined, "group-lockup");
    const text = el("div");
    text.append(el("strong", group.pack.name), el("span", group.pack.destRoot, "path"));
    lockup.append(pict("arrowRight", "pict pict-sm"), text);
    cell.append(lockup);
    head.append(cell);
    rows.push(head);
    for (const row of visible) {
      const tr = el("tr", undefined, `state-${row.state}`);
      const stateCell = el("td", undefined, "col-state");
      stateCell.append(icon(STATE_ICONS[row.state]));
      tr.append(stateCell, sourceCell(row.source, row.sourcePath), targetCell(row), noteCell(row));
      rows.push(tr);
    }
  }
  tableBody.replaceChildren(...rows);
  tableEmpty.hidden = rows.length > 0;
}

// ---------- settings ----------

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

// ---------- start ----------

async function init() {
  applyI18n();
  hydrateIcons();
  settings = await api.getSettings();
  renderPacks();
  render();
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
    kindChoices.clear();
    clearMessage();
    entriesChanged();
  });
  settingsBtn.addEventListener("click", openSettings);
  routeSettingsBtn.addEventListener("click", openSettings);
  settingsDialog.addEventListener("close", () => {
    if (stage === "ready") renderRoute();
  });
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
      document.body.classList.toggle("dragging", !busy);
    } else if (payload.type === "leave") {
      document.body.classList.remove("dragging");
    } else if (payload.type === "drop") {
      document.body.classList.remove("dragging");
      void addPaths(payload.paths);
    }
  });
}

void init();
