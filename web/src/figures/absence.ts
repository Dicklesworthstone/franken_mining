/** Typed negative evidence (plan §11.3–§11.4, `fmine-explore/src/absence.rs`): a cross-section through transported
 * cover onto a buried deposit, with four surveys that each came back blank. Cover thins each survey's detection,
 * q = q̄·(1 − cover ÷ DOI), exactly as `effective_detection_q` does, and never the deposit's existence. The class each
 * blank earns follows `assess_site` in its own order. Methods, depths and the deposit are illustrative. */
import { keep } from "../html";
import { range as rangeMarkup, toggle as toggleMarkup } from "../parts";
import { $, $$, onWidth, q, range, rng, toggle, type Q } from "./lib";

interface Method {
  id: string;
  name: string;
  short: string;
  doi: number;
  /** Baseline detection q̄ with no cover. */
  base: Q;
  why: string;
}
// Soil: the q̄ and DOI of absence.rs's own test survey. IP: GB-04's survey sensitivity. Magnetics: GB-01's q.
// Drillhole: GB-02's d_h per hole. Every survey here counts as one look (e_j = 1) and was recorded in full.
const METHODS: Method[] = [
  { id: "soil", name: "Soil geochemistry", short: "Soil", doi: 5, base: q(80, 100), why: "samples the top few metres" },
  { id: "ip", name: "Induced polarisation", short: "IP", doi: 250, base: q(85, 100), why: "sees chargeable sulphides at depth" },
  { id: "mag", name: "Magnetics", short: "Mag", doi: 400, base: q(10, 100), why: "sees deep, but rarely tells ore from rock" },
  { id: "hole", name: "One drillhole", short: "Hole", doi: 300, base: q(35, 100), why: "sees only what it passes through" },
];
const Q_MIN = q(5, 100); // absence.rs test: q_min separating A1 from A2
const MANSKI = q(60, 100); // absence.rs test: π_era_max for unrecorded effort (A4)
const MAX_DEPTH = 400;

const CLASSES = [
  { code: "A0", name: "Not looked", type: "A0NotLooked", state: "NotObservable" },
  { code: "A1", name: "Looked, insensitive", type: "A1LookedInsensitive", state: "NotObservable" },
  { code: "A2", name: "Looked, sensitive, nothing", type: "A2LookedSensitiveNothing", state: "Estimated" },
  { code: "A3", name: "Found sub-economic", type: "A3FoundSubEconomic", state: "Known" },
  { code: "A4", name: "Effort suspected, unrecorded", type: "A4EffortSuspectedUnrecorded", state: "Unknown" },
] as const;
type Code = (typeof CLASSES)[number]["code"];

const ZERO = q(0);
const ONE = q(1);
/** `effective_detection_q`: 0 when cover reaches the depth of investigation, else q̄·(1 − cover/DOI). */
export function effectiveQ(base: Q, cover: Q, doi: Q): Q {
  if (cover.cmp(doi) >= 0) return ZERO;
  return base.mul(ONE.sub(cover.div(doi)));
}
/** One blank, single-target mode (plan GB-02): p′ = p(1 − D) / (p(1 − D) + 1 − p). */
export const afterBlank = (p: Q, d: Q) => p.mul(ONE.sub(d)).div(p.mul(ONE.sub(d)).add(ONE.sub(p)));

interface Assessment {
  code: Code;
  qs: Map<string, Q>;
  h: Q;
  recorded: Q;
  lb: Q;
  ub: Q;
  maxQ: Q;
}
/** `assess_site` for one location with no catalogued occurrence: A3 cannot arise here. */
export function assess(on: Set<string>, cover: Q, unrecorded: boolean): Assessment {
  const qs = new Map<string, Q>();
  let h = ZERO;
  let recorded = ZERO;
  let maxQ = ZERO;
  for (const m of METHODS) {
    const qj = effectiveQ(m.base, cover, q(m.doi));
    qs.set(m.id, qj);
    if (!on.has(m.id)) continue;
    if (qj.cmp(maxQ) > 0) maxQ = qj;
    recorded = recorded.add(qj);
    if (qj.cmp(Q_MIN) > 0) h = h.add(qj);
  }
  const intersecting = on.size + (unrecorded ? 1 : 0);
  const u = unrecorded ? MANSKI : ZERO;
  const lb = h.div(ONE.add(h));
  const ub = recorded.cmp(ONE.sub(u)) < 0 ? recorded.add(u) : ONE;
  const code: Code = intersecting === 0 ? "A0" : unrecorded ? "A4" : h.n === 0n ? "A1" : "A2";
  return { code, qs, h, recorded, lb, ub, maxQ };
}

/** Up to three decimals, at least two: 0.68, 0.646, 0.085. */
const dec = (x: Q) => {
  const s = x.value.toFixed(3).replace(/0$/, "");
  return s.replace("-", "−");
};
const pct = (x: Q | number) => `${((typeof x === "number" ? x : x.value) * 100).toFixed(1)}%`;

export default function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("ab");
  const on = new Set<string>(["soil"]);
  let cover = 60;
  let prior = 10;
  let unrecorded = false;

  host.innerHTML = keep(`
<div class="ab-top">
  <p class="ab-tag"><span class="tag">Synthetic</span><span>Illustrative section · surveys over one buried deposit · every survey came back blank</span></p>
</div>
<div class="ab-ctl">
  ${rangeMarkup("cover", "Transported cover", 0, 250, 5, cover, `${cover} m`)}
  ${rangeMarkup("prior", "Chance a deposit is here, before looking", 1, 50, 1, prior, `${prior}%`)}
</div>
<div class="ab-view"><svg class="ab-svg" role="img" aria-label="Cross-section: surface, transported cover, bedrock and a buried deposit, with each survey’s depth of investigation"></svg></div>
<div class="ab-main">
  <div class="ab-methods">
    <p class="micro">Surveys, each blank · press to add or remove</p>
    <ul class="ab-list">${METHODS.map(
      (m) =>
        `<li><button type="button" class="ab-m" data-m="${m.id}" aria-pressed="${on.has(m.id)}"><span class="ab-mk" aria-hidden="true"></span><span class="ab-mname"><b>${m.name}</b><small>DOI ${m.doi} m · <span class="ab-qbar">q</span> ${dec(m.base)}<span class="ab-why"> · ${m.why}</span></small></span><span class="ab-mq"><small>q now</small><b data-q></b></span><span class="ab-mp"><small>after its blank</small><b data-p></b></span><span class="ab-mf" data-f></span></button></li>`,
    ).join("")}
    </ul>
    ${toggleMarkup("rab", "A 1987 report mentions RAB drilling here, with no collars recorded")}
  </div>
  <div class="ab-out" aria-live="polite">
    <ol class="ab-classes">${CLASSES.map(
      (c) => `<li data-c="${c.code}"><b>${c.code}</b><span>${c.name}</span></li>`,
    ).join("")}</ol>
    <div class="ab-earned" data-earned></div>
    <div class="ab-meter">
      <div class="ab-mrow"><span>Before</span><span class="ab-track"><i data-prior></i></span><b data-prior-v></b></div>
      <div class="ab-mrow after"><span>After all blanks</span><span class="ab-track"><em data-band></em><i data-post></i></span><b data-post-v></b></div>
    </div>
    <dl class="ab-sums" data-sums></dl>
  </div>
</div>
<p class="micro asis ab-note">The section, the deposit and the survey depths are illustrative. The baseline <span class="ab-qbar">q</span> for soil is absence.rs’s own test value; IP, magnetics and the drillhole take the plan’s worked examples (GB-\u206004, GB-\u206001, GB-\u206002). Each survey counts as one look. A look with q at or below q_min = 0.05 adds nothing to H and only widens the upper bound. The point estimate uses π = 1 − e<sup>−H</sup>; the bracket is the code’s own: H ÷ (1 + H) to the recorded hazard plus any unrecorded era bound, capped at 1. A3 needs a catalogued sub-economic intercept, which this site does not have.</p>`);

  const svg = $<SVGSVGElement>(host, ".ab-svg");
  const view = $(host, ".ab-view");
  const NS = "http://www.w3.org/2000/svg";
  const el = <K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>, parent: Element) => {
    const e = document.createElementNS(NS, tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
    parent.append(e);
    return e;
  };

  let W = 800;
  let narrow = false;
  const geo = () => {
    const H = narrow ? 380 : 440;
    const top = narrow ? 64 : 70;
    const left = narrow ? 30 : 46;
    const bottom = 8;
    const y = (d: number) => top + (d / MAX_DEPTH) * (H - top - bottom);
    // Each survey's column, over the deposit.
    const xs = narrow ? [0.33, 0.52, 0.71, 0.89] : [0.3, 0.46, 0.62, 0.78];
    const cx = (i: number) => left + (W - left) * xs[i]!;
    const colW = Math.min(narrow ? 40 : 74, (W - left) * 0.11);
    return { H, top, left, y, cx, colW };
  };

  /** The static ground: drawn once per width. */
  const ground = (g: ReturnType<typeof geo>) => {
    svg.replaceChildren();
    svg.setAttribute("viewBox", `0 0 ${W} ${g.H}`);
    svg.classList.toggle("narrow", narrow);
    const defs = el("defs", {}, svg);
    const grad = (id: string, stops: [number, string, number?][], v = true) => {
      const lg = el("linearGradient", { id, x1: 0, y1: 0, x2: v ? 0 : 1, y2: v ? 1 : 0 }, defs);
      for (const [o, c, a] of stops) el("stop", { offset: o, "stop-color": c, "stop-opacity": a ?? 1 }, lg);
    };
    grad("ab-sky", [
      [0, "#1b1530", 0],
      [1, "#2a2140", 0.55],
    ]);
    grad("ab-cover", [
      [0, "#8a6a43"],
      [1, "#6b5134"],
    ]);
    grad("ab-rock", [
      [0, "#2b2236"],
      [1, "#17121f"],
    ]);
    const rg = el("radialGradient", { id: "ab-ore", cx: "45%", cy: "40%", r: "65%" }, defs);
    el("stop", { offset: 0, "stop-color": "#ffd58a" }, rg);
    el("stop", { offset: 0.45, "stop-color": "#e08a2e" }, rg);
    el("stop", { offset: 1, "stop-color": "#7a3b12" }, rg);
    const pat = el("pattern", { id: "ab-peb", width: 14, height: 11, patternUnits: "userSpaceOnUse" }, defs);
    el("circle", { cx: 3, cy: 3, r: 1.3, fill: "#b8956a", opacity: 0.55 }, pat);
    el("circle", { cx: 10, cy: 7, r: 0.9, fill: "#d8b88c", opacity: 0.45 }, pat);
    el("ellipse", { cx: 7, cy: 1.5, rx: 1.8, ry: 0.8, fill: "#4e3a24", opacity: 0.5 }, pat);
    const hatch = el("pattern", { id: "ab-hatch", width: 6, height: 6, patternUnits: "userSpaceOnUse", patternTransform: "rotate(45)" }, defs);
    el("rect", { width: 2, height: 6, fill: "#fff", opacity: 0.22 }, hatch);
    const clip = el("clipPath", { id: "ab-below" }, defs);
    el("rect", { x: g.left, y: g.top, width: W - g.left, height: g.H - g.top }, clip);

    el("rect", { x: 0, y: 0, width: W, height: g.top, fill: "url(#ab-sky)" }, svg);
    const rock = el("g", { "clip-path": "url(#ab-below)" }, svg);
    el("rect", { x: g.left, y: g.top, width: W - g.left, height: g.H - g.top, fill: "url(#ab-rock)" }, rock);
    // Dipping strata and one fault.
    const tones = ["#3a2d4a", "#2e2540", "#463656", "#33283f", "#3f3150", "#2a2138"];
    const span = g.H - g.top;
    for (let i = -4; i < 14; i++) {
      const y0 = g.top + i * 42;
      const d = `M${g.left} ${y0} C ${W * 0.35} ${y0 + 30}, ${W * 0.6} ${y0 - 10}, ${W} ${y0 + span * 0.5} L ${W} ${y0 + span * 0.5 + 24} C ${W * 0.6} ${y0 + 14}, ${W * 0.35} ${y0 + 54}, ${g.left} ${y0 + 24} Z`;
      el("path", { d, fill: tones[(i + 8) % tones.length]!, opacity: 0.85 }, rock);
    }
    el("path", { d: `M${g.left + (W - g.left) * 0.9} ${g.top} L ${g.left + (W - g.left) * 0.8} ${g.H}`, class: "ab-fault" }, rock);
    return rock;
  };

  let parts: {
    g: ReturnType<typeof geo>;
    cover: SVGPathElement;
    coverEdge: SVGPathElement;
    peb: SVGPathElement;
    ore: SVGPathElement;
    oreLab: SVGTextElement;
    oreSub: SVGTextElement;
    coverLab: SVGTextElement;
    cols: { band: SVGRectElement; spent: SVGRectElement; seen: SVGRectElement; cap: SVGTextElement; qt: SVGTextElement; hit: SVGRectElement; line?: SVGLineElement }[];
    rab: SVGGElement;
  } | null = null;

  const build = () => {
    const g = geo();
    const rock = ground(g);
    const r = rng(7);
    // The deposit: a lens under the cover, in bedrock.
    const ore = el("path", { class: "ab-ore", fill: "url(#ab-ore)" }, rock);
    const oreLab = el("text", { class: "ab-orelab", "text-anchor": "end" }, svg);
    const oreSub = el("text", { class: "ab-oresub", "text-anchor": "end" }, svg);
    // Cover above it, then the soil line and the surface.
    const cover = el("path", { class: "ab-cover", fill: "url(#ab-cover)" }, svg);
    const pebbles = el("path", { class: "ab-peb", fill: "url(#ab-peb)" }, svg);
    const coverEdge = el("path", { class: "ab-unconf" }, svg);
    el("rect", { x: g.left, y: g.top - 1, width: W - g.left, height: 5, class: "ab-soil" }, svg);
    el("line", { x1: g.left, y1: g.top - 1, x2: W, y2: g.top - 1, class: "ab-surface" }, svg);
    // Vegetation: grass and a few trees, repeatable.
    const veg = el("g", { class: "ab-veg" }, svg);
    for (let x = g.left + 4; x < W; x += narrow ? 7 : 6) {
      const h = 2 + r() * 4;
      el("path", { d: `M${x} ${g.top - 1} l ${r() * 2 - 1} ${-h} M${x + 2} ${g.top - 1} l ${r() * 2} ${-h * 0.7}` }, veg);
    }
    for (const fx of [0.06, 0.17, 0.4, 0.55, 0.93, 0.99]) {
      const x = g.left + (W - g.left) * fx;
      const s = (narrow ? 0.7 : 1) * (0.8 + r() * 0.5);
      el("line", { x1: x, y1: g.top - 1, x2: x, y2: g.top - 1 - 12 * s, class: "ab-trunk" }, veg);
      el("circle", { cx: x, cy: g.top - 1 - 16 * s, r: 7 * s, class: "ab-crown" }, veg);
    }
    // Depth axis.
    const axis = el("g", { class: "ab-axis" }, svg);
    for (let d = 0; d <= MAX_DEPTH; d += 100) {
      el("line", { x1: g.left - 4, x2: g.left, y1: g.y(d), y2: g.y(d) }, axis);
      el("text", { x: g.left - 7, y: g.y(d) + 3.5, "text-anchor": "end" }, axis).textContent = d === 0 ? "0 m" : String(d);
    }
    const coverLab = el("text", { class: "ab-coverlab", "text-anchor": "start" }, svg);
    el("text", { class: "ab-rocklab", x: W - 8, y: g.H - 12, "text-anchor": "end" }, svg).textContent = "Bedrock";
    // Survey columns.
    const cols = METHODS.map((m, i) => {
      const cg = el("g", { class: `ab-col ${m.id}`, "data-m": m.id }, svg);
      const x = g.cx(i);
      const w = m.id === "hole" ? Math.max(8, g.colW * 0.28) : g.colW;
      const band = el("rect", { x: x - w / 2, y: g.top, width: w, height: g.y(m.doi) - g.top, class: "ab-band" }, cg);
      const spent = el("rect", { x: x - w / 2, y: g.top, width: w, height: 0, class: "ab-spent", fill: "url(#ab-hatch)" }, cg);
      const seen = el("rect", { x: x - w / 2, width: w, class: "ab-seen" }, cg);
      let line: SVGLineElement | undefined;
      if (m.id === "soil") {
        for (let k = 0; k < 5; k++) el("rect", { x: x - w / 2 + (w * (k + 0.5)) / 5 - 2, y: g.top - 5, width: 4, height: 4, class: "ab-sample" }, cg);
      }
      if (m.id === "hole") {
        line = el("line", { x1: x, x2: x, y1: g.top - 14, y2: g.y(m.doi), class: "ab-hole" }, cg);
        el("path", { d: `M${x - 8} ${g.top - 1} L ${x} ${g.top - 22} L ${x + 8} ${g.top - 1}`, class: "ab-rig" }, cg);
      }
      el("line", { x1: x - w / 2, x2: x + w / 2, y1: g.y(m.doi), y2: g.y(m.doi), class: "ab-doi" }, cg);
      const cap = el("text", { x, y: narrow ? 16 : 18, "text-anchor": "middle", class: "ab-cap" }, cg);
      cap.textContent = narrow ? m.short : m.name.replace("One drillhole", "Drillhole").replace("Induced polarisation", "IP survey");
      const qt = el("text", { x, y: narrow ? 30 : 34, "text-anchor": "middle", class: "ab-qt" }, cg);
      const hit = el("rect", { x: x - Math.max(w, 30) / 2, y: 0, width: Math.max(w, 30), height: g.y(m.doi), class: "ab-hit" }, cg);
      hit.addEventListener("click", () => flip(m.id));
      return { band, spent, seen, cap, qt, hit, line };
    });
    // Unrecorded 1987 RAB holes: dashed, short, to the left of the surveys.
    const rab = el("g", { class: "ab-rab" }, svg);
    const rx = g.left + (W - g.left) * (narrow ? 0.12 : 0.13);
    for (const dx of [-14, 0, 14]) el("line", { x1: rx + dx, x2: rx + dx + 2, y1: g.top, y2: g.y(50), class: "ab-rabh" }, rab);
    el("text", { x: rx, y: narrow ? 16 : 18, "text-anchor": "middle", class: "ab-cap" }, rab).textContent = narrow ? "1987" : "1987 RAB";
    el("text", { x: rx, y: narrow ? 30 : 34, "text-anchor": "middle", class: "ab-qt" }, rab).textContent = "no collars";
    parts = { g, cover, coverEdge, peb: pebbles, ore, oreLab, oreSub, coverLab, cols, rab };
    draw();
  };

  const wavy = (g: ReturnType<typeof geo>, y0: number, amp: number) => {
    const pts: string[] = [];
    const n = 24;
    for (let i = 0; i <= n; i++) {
      const x = g.left + ((W - g.left) * i) / n;
      pts.push(`${i ? "L" : "M"}${x.toFixed(1)} ${(y0 + Math.sin(i * 1.3) * amp + Math.sin(i * 0.47) * amp * 0.8).toFixed(1)}`);
    }
    return pts.join("");
  };

  const out = $(host, ".ab-out");
  const buttons = $$<HTMLButtonElement>(host, ".ab-m");
  const draw = () => {
    if (!parts) return;
    const { g } = parts;
    const c = q(cover);
    const p = q(prior, 100);
    const a = assess(on, c, unrecorded);
    // Ground.
    const yb = g.y(cover);
    const amp = cover > 4 ? 2.2 : 0;
    const edge = wavy(g, yb, amp);
    const area = `M${g.left} ${g.top}${edge.replace(/^M/, " L")} L ${W} ${g.top} Z`;
    parts.cover.setAttribute("d", area);
    parts.peb.setAttribute("d", area);
    parts.coverEdge.setAttribute("d", edge);
    parts.cover.style.display = cover ? "" : "none";
    parts.coverEdge.style.display = cover ? "" : "none";
    const ox = g.left + (W - g.left) * (narrow ? 0.6 : 0.56);
    const ow = (W - g.left) * (narrow ? 0.56 : 0.4);
    const ot = g.y(cover + 15);
    const oh = g.y(cover + 15 + 120) - ot;
    parts.ore.setAttribute(
      "d",
      `M${ox - ow / 2} ${ot + oh * 0.2} C ${ox - ow * 0.3} ${ot - oh * 0.06}, ${ox + ow * 0.2} ${ot - oh * 0.04}, ${ox + ow / 2} ${ot + oh * 0.12} C ${ox + ow * 0.36} ${ot + oh * 0.5}, ${ox + ow * 0.12} ${ot + oh * 0.95}, ${ox - ow * 0.04} ${ot + oh} C ${ox - ow * 0.2} ${ot + oh * 0.8}, ${ox - ow * 0.46} ${ot + oh * 0.5}, ${ox - ow / 2} ${ot + oh * 0.2} Z`,
    );
    parts.oreLab.setAttribute("x", String(ox - ow / 2 - 8));
    parts.oreLab.setAttribute("y", String(ot + oh * 0.34));
    parts.oreLab.textContent = "Buried deposit";
    parts.oreSub.setAttribute("x", String(ox - ow / 2 - 8));
    parts.oreSub.setAttribute("y", String(ot + oh * 0.34 + 14));
    parts.oreSub.textContent = narrow ? "always there" : "there in every case";
    parts.coverLab.setAttribute("x", String(g.left + 8));
    parts.coverLab.setAttribute("y", String(cover >= 22 ? g.top + Math.min(yb - g.top, 60) / 2 + 8 : yb + 14));
    parts.coverLab.textContent = cover ? `Transported cover · ${cover} m` : "No cover";
    // Columns.
    for (const [i, m] of METHODS.entries()) {
      const col = parts.cols[i]!;
      const qj = a.qs.get(m.id)!;
      const doiY = g.y(m.doi);
      const spentY = Math.min(yb, doiY);
      col.spent.setAttribute("height", String(Math.max(0, spentY - g.top)));
      col.seen.setAttribute("y", String(spentY));
      col.seen.setAttribute("height", String(Math.max(0, doiY - spentY)));
      col.seen.style.setProperty("--o", String(0.25 + 0.75 * (qj.value / m.base.value || 0)));
      col.qt.textContent = `q ${dec(qj)}`;
      const g2 = col.band.parentElement!;
      g2.classList.toggle("off", !on.has(m.id));
      g2.classList.toggle("blind", qj.cmp(Q_MIN) <= 0);
    }
    parts.rab.classList.toggle("off", !unrecorded);

    // Methods list.
    for (const b of buttons) {
      const m = METHODS.find((x) => x.id === b.dataset.m)!;
      const qj = a.qs.get(m.id)!;
      b.setAttribute("aria-pressed", String(on.has(m.id)));
      $(b, "[data-q]").textContent = dec(qj);
      $(b, "[data-p]").textContent = pct(afterBlank(p, qj));
      const blind = qj.cmp(Q_MIN) <= 0;
      b.classList.toggle("blind", blind);
      $(b, "[data-f]").innerHTML = keep(
        blind
          ? cover >= m.doi
            ? `${m.short === "Hole" ? "The hole" : "It"} cannot see through ${cover} m of cover: ${cover} ≥ DOI ${m.doi} m, so q = 0.`
            : `q ≤ q_min: a blank here is logged, and adds nothing.`
          : `q = ${dec(m.base)} × (1 − ${cover} ÷ ${m.doi}) = ${dec(qj)}`,
      );
    }

    // Class and numbers.
    const cls = CLASSES.find((x) => x.code === a.code)!;
    for (const li of $$(host, ".ab-classes li")) li.classList.toggle("on", li.dataset.c === a.code);
    out.dataset.c = a.code;
    const piPoint = 1 - Math.exp(-a.h.value);
    const post = (prior / 100) * (1 - piPoint) / ((prior / 100) * (1 - piPoint) + 1 - prior / 100);
    const postLo = afterBlank(p, a.ub);
    const postHi = afterBlank(p, a.lb);
    const say: Record<Code, string> = {
      A0: `No survey looked here, so the chance stays at ${prior}%.`,
      A1: `Every survey here was blind at ${cover} m of cover, so the chance stays near ${prior}%.`,
      A2: `Surveys that could see came back blank, so the chance falls from ${prior}% to about ${pct(post)}.`,
      A3: "",
      A4: `The 1987 RAB holes may have tested this ground. Their result is unknown, so the chance is carried as a range.`,
    };
    $(host, "[data-earned]").innerHTML = keep(
      `<p class="ab-cls"><b>${cls.code}</b><span>${cls.name}</span></p><p class="ab-type"><code>${cls.type}</code> · knowledge state <code>${cls.state}</code></p><p class="ab-say">${say[a.code]}</p>`,
    );
    const pos = (x: number) => `${(x * 100).toFixed(2)}%`;
    const scale = 0.5; // the meter spans 0–50%
    $(host, "[data-prior]").style.width = pos(prior / 100 / scale);
    $(host, "[data-prior-v]").textContent = `${prior}%`;
    const point = a.code === "A0" ? prior / 100 : post;
    $(host, "[data-post]").style.left = pos(point / scale);
    const band = $(host, "[data-band]");
    band.style.left = pos(postLo.value / scale);
    band.style.width = pos((postHi.value - postLo.value) / scale);
    const same = pct(postLo) === pct(postHi);
    $(host, "[data-post-v]").textContent = a.code === "A0" || same ? pct(a.code === "A0" ? prior / 100 : postHi) : `${pct(postLo)} to ${pct(postHi)}`;
    const sums = $(host, "[data-sums]");
    sums.innerHTML = keep(
      [
        ["Hazard H = Σ e·q, over looks with q > q_min", dec(a.h)],
        ["Detection π = 1 − e<sup>−H</sup>", a.h.n === 0n ? "0" : piPoint.toFixed(3)],
        ["π bounds, as assess_site reports them", `${dec(a.lb)} to ${dec(a.ub)}`],
        ["Chance after all blanks", a.code === "A0" || same ? pct(point) : `${pct(point)} (${pct(postLo)} to ${pct(postHi)})`],
      ]
        .map(([k, v]) => `<div><dt>${k}</dt><dd>${v}</dd></div>`)
        .join(""),
    );
    host.dataset.c = a.code;
  };

  const flip = (id: string) => {
    if (on.has(id)) on.delete(id);
    else on.add(id);
    draw();
  };
  for (const b of buttons) b.addEventListener("click", () => flip(b.dataset.m!));
  range(host, "cover", (v) => `${v} m`, (v) => {
    cover = v;
    draw();
  });
  range(host, "prior", (v) => `${v}%`, (v) => {
    prior = v;
    draw();
  });
  toggle(host, "rab", (v) => {
    unrecorded = v;
    draw();
  });
  onWidth(view, (w) => {
    W = w;
    narrow = w < 600;
    build();
  });
}
