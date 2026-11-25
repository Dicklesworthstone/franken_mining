import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import MarkdownIt from "markdown-it";
import { escape } from "./layout";

type Token = ReturnType<typeof md.parse>[number];
interface Env {
  /** Inside a link or a heading, where no further links may be added. */
  links?: number;
  heading?: number;
  entry?: Entry;
  used?: Set<string>;
}

export interface Entry {
  id: string;
  title: string;
  /** "§25" or "Appendix M". */
  label: string;
  group: string;
  start: number;
  url: string;
  summary: string;
  words: number;
  tables: number;
  figures: number;
  sub: { title: string; id: string; level: number }[];
  body: Token[];
}
export interface Definition {
  id: string;
  entry: Entry;
  text: string;
  href: string;
}

export const sourcePath = join(
  resolve(import.meta.dir, ".."),
  "../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md",
);
export const bytes = readFileSync(sourcePath);
const source = bytes.toString("utf8");
const lines = source.split("\n");
export const version = source.match(/^\| Version \| (\d+\.\d+)/m)?.[1] ?? "";
if (!version) throw Error("The plan's version row has moved.");

const md = new MarkdownIt({ html: false, linkify: true, typographer: false });
md.linkify.set({ fuzzyLink: false, fuzzyEmail: false });
// In C-32 the interval extension [c](B) is mathematical notation.
md.inline.ruler.before("link", "interval_enclosure", (state, silent) => {
  if (!state.src.startsWith("[c](B)", state.pos)) return false;
  if (!silent) state.pending += "[c](B)";
  state.pos += "[c](B)".length;
  return true;
});

const plain = (t: Token) =>
  (t.children ?? [])
    .map((c) => (c.type === "softbreak" || c.type === "hardbreak" ? " " : c.type === "text" || c.type === "code_inline" ? c.content : ""))
    .join("");
const untiered = (s: string) => s.replace(/(?:\s*\[[SFM]\](?:\s*\/)?)+\s*$/, "");
const clip = (s: string, n: number) => {
  if (s.length <= n) return s;
  const cut = s.slice(0, n);
  const stop = Math.max(cut.lastIndexOf(". "), cut.lastIndexOf("; "));
  return stop > n * 0.55 ? cut.slice(0, stop + 1).replace(/;$/, ".") : `${cut.slice(0, cut.lastIndexOf(" "))}…`;
};

/* ---------- Sections ---------- */
export const entries: Entry[] = [];
let group = "Overview";
const add = (id: string, title: string, label: string, start: number) =>
  entries.push({
    id,
    title,
    label,
    start,
    group,
    url: `/plan/${id}/`,
    summary: "",
    words: 0,
    tables: 0,
    figures: 0,
    sub: [],
    body: [],
  });
add("introduction", "Introduction and contents", `Version ${version}`, 0);
const all = md.parse(source, {});
for (const [i, t] of all.entries()) {
  if (t.type !== "heading_open") continue;
  const heading = all[i + 1]!.content;
  if (t.tag === "h1" && (heading.startsWith("PART ") || heading === "APPENDICES"))
    group =
      heading === "APPENDICES"
        ? "Appendices"
        : heading.replace(
            /^PART (\S+) — (.+)$/,
            (_, n, name) => `Part ${n} · ${name[0]}${name.slice(1).toLowerCase()}`,
          );
  if (t.tag !== "h2") continue;
  const m = heading.match(/^(\d+(?:\.\d+)?)\s*[.]? (.+)$/);
  const a = heading.match(/^Appendix ([A-O]) · (.+)$/);
  if (m) add(m[1]!, m[2]!, `§${m[1]}`, t.map![0]);
  else if (a) add(`appendix-${a[1]!.toLowerCase()}`, a[2]!, `Appendix ${a[1]}`, t.map![0]);
}
// Move each part divider to its opening chapter, preserving every intervening line.
for (const p of entries.slice(1)) {
  let i = p.start - 1;
  while (i > 0 && !lines[i]!.trim()) i--;
  if (/^# (PART |APPENDICES$)/.test(lines[i]!)) p.start = i;
}
if (entries.length < 50 || new Set(entries.map((p) => p.id)).size !== entries.length)
  throw Error(`The plan's structure changed: ${entries.length} entries`);
const chunks = entries.map((p, i) => lines.slice(p.start, entries[i + 1]?.start ?? lines.length).join("\n"));
if (chunks.join("\n") !== source) throw Error("Incomplete plan segmentation");
export const groups = [...new Set(entries.map((e) => e.group))];
export const groupSlug = (g: string) => `part-${groups.indexOf(g)}`;
export const words = entries.reduce((n, _, i) => n + chunks[i]!.split(/\s+/).filter(Boolean).length, 0);

/* ---------- Headings, references and definitions ---------- */
const refs = new Map<string, string>();
const candidates = new Map<string, { entry: Entry; token: Token; text: string; score: number }[]>();
const ID = /^[A-Z]{1,8}(?:-[A-Z]+)*-\d+[a-z]?$/;
for (const [i, p] of entries.entries()) {
  refs.set(p.id, p.url);
  const block = md.parse(chunks[i]!, {});
  const ids = new Set<string>();
  p.words = chunks[i]!.split(/\s+/).filter(Boolean).length;
  for (let j = 0; j < block.length; j++) {
    const t = block[j]!;
    if (t.type === "table_open") p.tables++;
    if (t.type === "fence" || t.type === "code_block") p.figures++;
    if (t.type !== "heading_open") continue;
    const title = block[j + 1]!.content;
    if (p.id !== "introduction" && (t.tag === "h1" || (t.tag === "h2" && title !== "Contents"))) {
      block.splice(j, 3);
      j--;
      continue;
    }
    if (t.tag === "h1") {
      t.tag = block[j + 2]!.tag = "p";
      t.attrSet("class", "source-heading");
      continue;
    }
    const match = title.match(/^(\d+(?:\.\d+)+|[A-O]\.\d+(?:\.\d+)*|C-\d+)\b/);
    let id = match ? `s-${match[1]!.toLowerCase()}` : `heading-${p.sub.length + 1}`;
    if (ids.has(id)) id += `-${j}`;
    ids.add(id);
    t.attrSet("id", id);
    const text = untiered(plain(block[j + 1]!));
    p.sub.push({ title: text, id, level: Number(t.tag[1]) });
    if (match) {
      refs.set(match[1]!, `${p.url}#${id}`);
      if (match[1]!.startsWith("C-"))
        (candidates.get(match[1]!) ?? candidates.set(match[1]!, []).get(match[1]!)!).push({
          entry: p,
          token: t,
          text: text.replace(/^C-\d+\.?\s*/, ""),
          score: 1e6,
        });
    }
  }
  // The document outline starts one level under the page title and never skips a level.
  const least = Math.min(...p.sub.map((s) => s.level));
  let before = 1;
  for (const [j, t] of block.entries()) {
    if (t.type !== "heading_open") continue;
    const sub = p.sub.find((s) => s.id === t.attrGet("id"));
    if (!sub) continue;
    const level = Math.min(before + 1, sub.level - least + 2, 6);
    before = sub.level = level;
    t.tag = `h${level}`;
    const close = block.slice(j).find((x) => x.type === "heading_close");
    close!.tag = t.tag;
  }
  // Where an identifier is defined: a table row or a list item that opens with it.
  for (const [j, t] of block.entries()) {
    const note = (id: string, token: Token, text: string, score: number) =>
      (candidates.get(id) ?? candidates.set(id, []).get(id)!).push({ entry: p, token, text, score });
    if (t.type === "tr_open" && block[j + 1]?.type === "td_open") {
      const id = block[j + 2]!.content.replace(/[`*]/g, "").trim();
      if (!ID.test(id)) continue;
      const cells: string[] = [];
      for (let k = j + 4; block[k]!.type !== "tr_close"; k++)
        if (block[k]!.type === "inline") cells.push(plain(block[k]!).trim());
      const text = cells.filter(Boolean).join(" · ");
      note(id, t, text, text.length + (p.id.startsWith("appendix") ? 0 : 400));
    } else if (t.type === "list_item_open" && block[j + 2]?.type === "inline") {
      const id = block[j + 2]!.content.match(/^\*\*([A-Z]{1,8}(?:-[A-Z]+)*-\d+)\b/)?.[1];
      if (id) note(id, t, plain(block[j + 2]!).replace(id, "").replace(/^[\s.:·]+/, ""), 2000);
    }
  }
  p.body = block;
  const first = block.findIndex((t, j) => t.type === "paragraph_open" && t.level === 0 && plain(block[j + 1]!).length > 60);
  if (first >= 0) p.summary = clip(plain(block[first + 1]!).replace(/\s+/g, " "), 190);
}
export const definitions = new Map<string, Definition>();
for (const [id, list] of candidates) {
  const best = list.reduce((a, b) => (b.score > a.score ? b : a));
  let anchor = best.token.attrGet("id");
  if (!anchor) best.token.attrSet("id", (anchor = id.toLowerCase()));
  const href = `${best.entry.url}#${anchor}`;
  definitions.set(id, { id, entry: best.entry, text: clip(best.text.replace(/\s+/g, " "), 340), href });
  if (!refs.has(id)) refs.set(id, href);
}

/** The address of a section, appendix, construction or identifier. An unknown one stops the build. */
export function ref(id: string) {
  const href = refs.get(id);
  if (!href) throw Error(`The plan has no ${id}`);
  return href;
}
/** The nearest published heading: §25.13.2 falls back to §25.13. */
function near(n: string) {
  for (let s = n; s; s = s.slice(0, Math.max(0, s.lastIndexOf(".")))) {
    const href = refs.get(s);
    if (href) return href;
  }
}

/* ---------- Rendering ---------- */
const LABELS: Record<string, string> = {
  FACT: "Observed in a sibling repository, on this machine, or in a cited public source",
  DESIGN: "A decision of this plan, with its reason",
  HYPOTHESIS: "Believed and not established; it names the test that would settle it",
  TARGET: "A number the plan intends to hit; not a measurement",
  OPEN: "Undecided, with an owner, a deadline and a fail-closed default",
  S: "Solid: established mathematics and engineering",
  F: "Frontier: published research, with a pilot and a kill gate",
  M: "Moonshot: proposed here, behind a feature flag",
};
const chip = (k: string, text = k) => `<span class="ev ev-${k.toLowerCase()}" title="${LABELS[k]}">${text}</span>`;
const PATTERN =
  /§(\d+(?:\.\d+)*)|\bAppendix ([A-O])\b|\b([A-Z]{1,8}(?:-[A-Z]+)*-\d+)\b|\b(FACT|DESIGN|HYPOTHESIS|TARGET|OPEN)\b(?![-_.]\w)|\[([SFM])\]/g;
type Rule = (ts: Token[], i: number, options: typeof md.options, env: Env, self: typeof md.renderer) => string;
const rules = md.renderer.rules as Record<string, Rule>;
const idLink = (id: string, inner: string, env: Env) => {
  const d = definitions.get(id)!;
  env.used?.add(id);
  return `<a class="id" href="${d.href}" data-id="${id}">${inner}</a>`;
};
rules.text = (ts, i, _, env) => {
  const s = escape(ts[i]!.content);
  return s.replace(PATTERN, (text, n, letter, id, label, tier) => {
    if (label) return chip(label);
    if (tier) return chip(tier);
    if (env.links || env.heading) return text;
    if (id) return definitions.has(id) ? idLink(id, text, env) : text;
    const href = n ? near(n) : refs.get(`appendix-${letter.toLowerCase()}`);
    return href ? `<a class="xref" href="${href}">${text}</a>` : text;
  });
};
rules.code_inline = (ts, i, _, env) => {
  const s = ts[i]!.content;
  const html = `<code>${escape(s)}</code>`;
  return !env.links && !env.heading && definitions.has(s) ? idLink(s, html, env) : html;
};
rules.link_open = (ts, i, options, env, self) => {
  env.links = (env.links ?? 0) + 1;
  return self.renderToken(ts, i, options);
};
rules.link_close = (ts, i, options, env, self) => {
  env.links = (env.links ?? 1) - 1;
  return self.renderToken(ts, i, options);
};
rules.heading_open = (ts, i, options, env, self) => {
  env.heading = 1;
  return self.renderToken(ts, i, options);
};
rules.heading_close = (ts, i, _, env) => {
  env.heading = 0;
  const id = ts[i - 2]!.attrGet("id");
  return `${id ? `<a class="anchor" href="#${id}" aria-hidden="true" tabindex="-1"></a>` : ""}</${ts[i]!.tag}>\n`;
};
rules.table_open = (ts, i) => {
  const heads: string[] = [];
  for (let k = i; ts[k]!.type !== "thead_close"; k++) if (ts[k]!.type === "inline") heads.push(plain(ts[k]!).trim());
  // Wide enough that a long cell is not squeezed into a column of single words.
  const width = [0, 20, 32, 46, 58, 70, 82][heads.length] ?? heads.length * 12.5;
  const name = heads.filter(Boolean).slice(0, 3).join(", ") || "Details";
  return `<div class="tablewrap wide"><div class="tablescroll" tabindex="0" role="region" aria-label="${escape(`Table: ${name}`)}"><table style="--tw:${width}rem"${heads.some(Boolean) ? "" : ' class="headless"'}>`;
};
rules.table_close = () => "</table></div></div>\n";

const KEYWORDS: Record<string, string> = {
  rust: "pub|fn|struct|enum|impl|trait|for|let|mut|const|type|use|where|match|if|else|return|self|Self|mod|crate|as|in|ref|move|dyn|static|loop|while|unsafe|async|await",
  c: "void|int|uint|float|double|struct|const|static|inline|if|else|for|while|return|kernel|device|constant|threadgroup",
};
/** Comments, strings, numbers and keywords. Enough to read by, and it never changes the text. */
function paint(code: string, lang: string) {
  const kind = /^(rust|rs)$/.test(lang) ? "rust" : /^(c|cpp|metal|cuda|wgsl|glsl)$/.test(lang) ? "c" : lang;
  const comment = kind === "rust" || kind === "c" || kind === "jsonc" ? "//[^\\n]*" : /^(sh|bash|toml|yaml|python|py)$/.test(kind) ? "#[^\\n]*" : "(?!)";
  if (!kind || /^(text|txt|ebnf)$/.test(kind)) return escape(code);
  const words = KEYWORDS[kind];
  const re = new RegExp(`(${comment})|("(?:\\\\.|[^"\\\\\\n])*")|(\\b\\d[\\d_.]*(?:e-?\\d+)?\\b)${words ? `|\\b(${words})\\b` : ""}`, "g");
  let out = "";
  let last = 0;
  for (const m of code.matchAll(re)) {
    out += escape(code.slice(last, m.index));
    out += `<span class="tk-${m[1] ? "c" : m[2] ? "s" : m[3] ? "n" : "k"}">${escape(m[0])}</span>`;
    last = m.index + m[0].length;
  }
  return out + escape(code.slice(last));
}
rules.fence = rules.code_block = (ts, i) => {
  const t = ts[i]!;
  const lang = (t.info ?? "").trim().split(/\s+/)[0] ?? "";
  return `<div class="code wide"><button type="button" class="copy" data-pagefind-ignore hidden>Copy</button><pre tabindex="0"><code>${paint(t.content, lang)}</code></pre></div>\n`;
};

/** One paragraph of the plan's own Markdown, with its identifiers linked. */
export const inline = (text: string) => md.renderInline(text, {} satisfies Env);

export function render(entry: Entry) {
  const used = new Set<string>();
  const html = md.renderer.render(entry.body, md.options, { entry, used } satisfies Env);
  return { html, used };
}

/** The n-th code block under a numbered heading. */
export function codeUnder(id: string, n: number) {
  const href = ref(id);
  const entry = entries.find((e) => href.startsWith(`${e.url}#`))!;
  const anchor = href.split("#")[1];
  const from = entry.body.findIndex((t) => t.type === "heading_open" && t.attrGet("id") === anchor);
  const level = entry.body[from]!.tag;
  let seen = 0;
  for (const t of entry.body.slice(from + 1)) {
    if (t.type === "heading_open" && t.tag <= level) break;
    if (t.type === "fence" && seen++ === n) return t.content;
  }
  throw Error(`${id} has no code block ${n}`);
}
