import heights from "./heights.json";
import { escape } from "./html";

/** Where a section, appendix or construction of the plan is published. Filled in by the build. */
export type Refs = (id: string) => string;

/** The small tag on a figure. A picture needs none. */
export type Kind = "live" | "example" | "engine" | "scene";
const KIND: Record<Kind, string> = { live: "Interactive", example: "Worked example", engine: "Runs FrankenMining’s Rust in your browser", scene: "Live 3D" };

export const icon = (name: keyof typeof ICONS) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name]}</svg>`;
const ICONS = {
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.6-3.6"/>',
  down: '<path d="M12 5v14M6 13l6 6 6-6"/>',
  right: '<path d="M5 12h14M13 6l6 6-6 6"/>',
  out: '<path d="M8 6h10v10M18 6 6 18"/>',
  pause: '<path d="M9 5v14M15 5v14"/>',
  play: '<path d="M8 5.5v13l10.5-6.5z"/>',
  redo: '<path d="M4 12a8 8 0 1 0 2.6-5.9M4 4v5h5"/>',
  question: '<path d="M12 3v3M12 18v3M3 12h3M18 12h3"/><circle cx="12" cy="12" r="5"/>',
  body: '<path d="M5 9h14v4H5zM7 13l-2 4 1.5 3.5M17 13l2 4-1.5 3.5M11 13l-1 7M14 13l1 7"/>',
  world: '<path d="M4 20V9l8-5 8 5v11M4 20h16M10 20v-6h4v6"/>',
  answer: '<path d="M4 4h16v16H4zM4 12h16M12 4v16"/><path d="m14.6 16.2 1.4 1.4 2.4-3"/>',
  evidence: '<path d="M6 3h9l4 4v14H6zM15 3v4h4M9 12h7M9 16h7"/>',
  test: '<path d="M12 21a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM12 9v4l3 2M9 2h6"/>',
  book: '<path d="M4 5a2 2 0 0 1 2-2h13v15H6a2 2 0 0 0-2 2zM4 19a2 2 0 0 0 2 2h13v-3"/>',
  list: '<path d="M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01"/>',
  menu: '<path d="M4 8h16M4 16h16"/>',
  left: '<path d="M19 12H5M11 6l-6 6 6 6"/>',
  home: '<path d="m4 11 8-7 8 7v9H4zM10 20v-6h4v6"/>',
  check: '<path d="m5 12.5 4.5 4.5L19 7.5"/>',
  claim: '<path d="M4 6h16v12H4z"/><path d="M8 10h5M8 14h8"/><circle cx="17" cy="10" r="1.2"/>',
  drill: '<path d="M12 3v18M8 3h8M9 21l3-3 3 3M10 8h4M10 13h4"/>',
  pit: '<path d="M2 7h3l2 3h2l2 3h2l2-3h2l2-3h3"/><path d="M5 17h14"/>',
  truck: '<path d="M3 15V8h11l3 4h4v3"/><circle cx="7" cy="17" r="2"/><circle cx="17" cy="17" r="2"/><path d="M9 17h6"/>',
  plant: '<path d="M3 20V10l5 3V10l5 3V6h3l1 6v8z"/><path d="M3 20h18"/>',
  scale: '<path d="M12 4v16M7 20h10M5 8h14"/><path d="m5 8-3 6h6zM19 8l-3 6h6z"/>',
  decide: '<path d="M12 3v6M12 9l-6 6M12 9l6 6"/><circle cx="6" cy="18" r="2.4"/><circle cx="18" cy="18" r="2.4"/>',
  agent: '<rect x="4" y="7" width="16" height="12" rx="3"/><path d="M12 3v4M9 13h.01M15 13h.01M9.5 16h5"/>',
  layers: '<path d="m12 3 9 5-9 5-9-5z"/><path d="m3 13 9 5 9-5"/>',
  star: '<path d="m12 3.6 2.6 5.3 5.8.8-4.2 4.1 1 5.8-5.200-2.700-5.200 2.700 1-5.800-4.200-4.100 5.800-.8z"/>',
  engine: '<circle cx="12" cy="12" r="3.200"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.600 5.600l2.100 2.100M16.300 16.300l2.100 2.100M18.400 5.600l-2.100 2.100M7.700 16.300l-2.100 2.100"/>',
  crash: '<path d="m13 2-7 11h5l-1 9 8-12h-5.500z"/>',
  cross: '<path d="M6 6l12 12M18 6 6 18"/>',
} as const;

/** A heading with one word picked out: `A body is *data*.` */
export const title = (s: string) => escape(s).replace(/\*(.+?)\*/g, '<em class="g">$1</em>');

/** Inline copy: `code`, **bold** and [links](url). The text is ours, and is escaped all the same. */
export const copy = (s: string, ref?: Refs) =>
  escape(s)
    .replace(/`(.+?)`/g, "<code>$1</code>")
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/\[(.+?)\]\((.+?)\)/g, (_, text, to) => {
      const href = to.startsWith("§") ? ref!(to.slice(1)) : to;
      return `<a class="txt" href="${href}">${text}</a>`;
    });

export interface Chapter {
  id: string;
  number: string;
  label: string;
  short: string;
  icon: keyof typeof ICONS;
  colour: string;
  title: string;
  thesis: string;
  sections: [id: string, label: string][];
}

export const chapterOpen = (c: Chapter) => `
<div class="chapter" data-chapter="${c.id}" style="--chapter:${c.colour}">
<section class="opener" aria-labelledby="${c.id}-title"><div class="wrap" id="${c.id}"><div class="reveal">
<p class="kick"><span class="joint on" aria-hidden="true"></span>${escape(c.label)}</p>
<h2 class="d2" id="${c.id}-title">${title(c.title)}</h2><p class="lede">${escape(c.thesis)}</p>
<ul class="opener-index">${c.sections
  .map(([id, label]) => `<li><a href="#${id}">${escape(label)}</a></li>`)
  .join("")}</ul></div></div></section>`;
export const chapterClose = "</div>";

export function section(o: {
  id: string;
  eyebrow: string;
  title: string;
  kicker?: string;
  /** Where the plan goes into it: what the link says, and the section it opens. */
  more?: [text: string, id: string];
  body: string;
  layout?: "wide" | "split";
  ref: Refs;
}) {
  const head = `<p class="kick small"><span class="joint on" aria-hidden="true"></span>${escape(o.eyebrow)}</p><h3 class="d3" id="${o.id}-title">${title(o.title)}</h3>`;
  const more = o.more ? `<a class="more" href="${o.ref(o.more[1])}">${escape(o.more[0])}${icon("right")}</a>` : "";
  const kicker = o.kicker ? `<p class="kicker">${copy(o.kicker, o.ref)}</p>` : "";
  return o.layout === "split"
    ? `<section class="sec split" id="${o.id}" aria-labelledby="${o.id}-title"><div class="wrap split-grid"><div class="sec-head reveal">${head}${kicker}${more}</div><div class="sec-body">${o.body}</div></div></section>`
    : `<section class="sec" id="${o.id}" aria-labelledby="${o.id}-title"><div class="wrap"><div class="sec-head wide reveal"><div>${head}${kicker}</div>${more}</div><div class="sec-body">${o.body}</div></div></section>`;
}

/** The height a figure settles at on a phone, a tablet and a desktop, kept free until its script has drawn it. */
const room = (name: string) => {
  const h = (heights as Record<string, number[]>)[name];
  return h ? ` style="--hp:${h[0]}px;--ht:${h[1]}px;--hd:${h[2]}px"` : "";
};

/** A figure: one line that says what to do with it, the thing itself, and a footnote where one helps. */
export function figure(o: {
  name: string;
  todo: string;
  kind?: Kind;
  note?: string;
  body: string;
  className?: string;
  ref?: Refs;
}) {
  return `<figure class="fig reveal ${o.className ?? ""}" id="fig-${o.name}" data-fig="${o.name}">
<header class="fig-head"><p class="todo">${escape(o.todo)}</p>${o.kind ? `<span class="tagged ${o.kind}">${KIND[o.kind]}</span>` : ""}</header>
<div class="fig-body"${room(o.name)}>${o.body}</div>${o.note ? `
<figcaption class="fig-foot">${copy(o.note, o.ref)}</figcaption>` : ""}</figure>`;
}

export const seg = (name: string, label: string, options: [value: string, text: string][], on: string) =>
  `<div class="seg" role="group" aria-label="${escape(label)}" data-seg="${name}">${options
    .map(
      ([v, text]) =>
        `<button type="button" data-v="${v}" aria-pressed="${v === on}">${escape(text)}</button>`,
    )
    .join("")}</div>`;

export const range = (
  name: string,
  label: string,
  min: number,
  max: number,
  step: number,
  value: number,
  shown: string,
) =>
  `<label class="range" data-range="${name}"><span><b>${escape(label)}</b><output aria-live="off">${escape(shown)}</output></span><input type="range" aria-label="${escape(label)}" min="${min}" max="${max}" step="${step}" value="${value}" style="--p:${((value - min) / (max - min)) * 100}%"></label>`;

export const toggle = (name: string, label: string, on = false) =>
  `<button type="button" class="toggle" data-toggle="${name}" aria-pressed="${on}"><i></i>${escape(label)}</button>`;

export const read = (name: string, label: string, value: string, note = "") =>
  `<div class="read" data-read="${name}"><small>${escape(label)}</small><b>${escape(value)}</b>${note || note === "" ? `<span>${escape(note)}</span>` : ""}</div>`;

const ONES = "zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen".split(" ");
const TENS = "twenty thirty forty fifty sixty seventy eighty ninety".split(" ");
/** 24 as "twenty-four", so counts taken from the plan can open a heading. */
export function spell(n: number, capital = false) {
  const s = n < 20 ? ONES[n]! : `${TENS[Math.floor(n / 10) - 2]}${n % 10 ? `-${ONES[n % 10]}` : ""}`;
  return capital ? s[0]!.toUpperCase() + s.slice(1) : s;
}
