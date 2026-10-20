import {
  ArrowDown,
  ArrowRight,
  Ban,
  CardSim,
  Check,
  Ear,
  Eraser,
  Eye,
  FilePlus,
  FolderPlus,
  Minus,
  RefreshCw,
  Settings,
  TriangleAlert,
  Undo2,
  X,
  createElement,
  type IconNode,
} from "lucide";

// One icon family (Lucide, ISC licence), one stroke weight, drawn as signage pictograms.
const ICONS = {
  add: FolderPlus,
  addFile: FilePlus,
  arrowDown: ArrowDown,
  arrowRight: ArrowRight,
  card: CardSim,
  check: Check,
  clear: Eraser,
  convert: RefreshCw,
  ear: Ear,
  error: Ban,
  review: Eye,
  settings: Settings,
  skip: Minus,
  undo: Undo2,
  warning: TriangleAlert,
  x: X,
} satisfies Record<string, IconNode>;

export type IconName = keyof typeof ICONS;

export function icon(name: IconName): SVGElement {
  const svg = createElement(ICONS[name], { "stroke-width": 2.25, "aria-hidden": "true", focusable: "false" });
  svg.classList.add("icon");
  return svg;
}

/** Replaces every `<span data-icon="name">` placeholder in the static markup with its SVG. */
export function hydrateIcons(root: ParentNode = document): void {
  root.querySelectorAll<HTMLElement>("[data-icon]").forEach((slot) => {
    slot.replaceWith(icon(slot.dataset.icon as IconName));
  });
}
