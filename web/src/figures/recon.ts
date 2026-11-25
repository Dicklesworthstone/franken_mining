/** Reconciliation across the material-flow boundaries (plan §20): a steppable Sankey of tonnes and of copper for one
 * synthetic dig polygon, with the ratio at every boundary. The centrepiece is the README's matched-cohort example: polygon A
 * delivered the planned metal on 20% more tonnes at 83.33% of the planned grade, and `fmine recon run` puts the two
 * offsetting misses in kilograms. Everything here is synthetic. */
import { keep } from "../html";
import { icon, seg as segMarkup } from "../parts";
import { $, $$, clamp, fmt, onWidth, q, reduced, seen, seg, smooth, tallest, type Q } from "./lib";

/* ---------- The synthetic month, exact ---------- */

/** [dry tonnes, kg Cu] */
type Pair = readonly [Q, Q];
const P = (t: Q | number, kg: Q | number): Pair => [typeof t === "number" ? q(t) : t, typeof kg === "number" ? q(kg) : kg];

interface Node {
  name: string;
  short: string;
  how: string;
  v: Pair;
}
/** Polygon A, North pit, bench 240, 2026-10: the worksheet's call and actual, carried on through an illustrative plant. */
const NODES: Node[] = [
  { name: "Resource model", short: "Model", how: "the block model over the surveyed mined solid", v: P(1000, 10000) },
  { name: "Grade control", short: "GC", how: "blasthole assays and the grade-control call", v: P(1000, 10000) },
  { name: "Mined", short: "Mined", how: "truck counts × weighed payload, then the survey", v: P(1200, 10000) },
  { name: "ROM pad", short: "Pad", how: "dump records and the pad survey", v: P(1200, 10000) },
  { name: "Mill feed", short: "Mill", how: "the belt weightometer and sampler", v: P(1000, q(25000, 3)) },
  { name: "Concentrate", short: "Conc", how: "assays and the concentrate stocktake", v: P(25, q(20000, 3)) },
  { name: "Sales", short: "Sales", how: "the smelter settlement", v: P(25, q(19300, 3)) },
];

type Tone = "gain" | "loss" | "stock" | "tails" | "ded";
interface Branch {
  b: number;
  dir: "in" | "out";
  tone: Tone;
  v: Pair;
  label: string;
  sub: string;
  /** Where the side node sits across the gap, 0 to 1, wide and narrow. */
  at: [number, number];
}
const BRANCHES: Branch[] = [
  { b: 2, dir: "out", tone: "loss", v: P(0, q(5500, 3)), label: "Lower grade", sub: "than called", at: [0.3, 0.17] },
  { b: 2, dir: "in", tone: "gain", v: P(200, q(5500, 3)), label: "Extra tonnes", sub: "not in the call", at: [0.7, 0.62] },
  { b: 4, dir: "out", tone: "stock", v: P(200, q(5000, 3)), label: "Left on the pad", sub: "kept as inventory", at: [0.5, 0.32] },
  { b: 5, dir: "out", tone: "tails", v: P(975, q(5000, 3)), label: "Tailings", sub: "", at: [0.5, 0.32] },
  { b: 6, dir: "out", tone: "ded", v: P(0, q(700, 3)), label: "Smelter deductions", sub: "", at: [0.5, 0.32] },
];

const ZERO = q(0);
const sum = (xs: Q[]) => xs.reduce((a, b) => a.add(b), ZERO);
const lossesAt = (b: number, i: 0 | 1) => sum(BRANCHES.filter((x) => x.b === b && x.dir === "out").map((x) => x.v[i]));
const pass = (b: number, i: 0 | 1) => NODES[b - 1]!.v[i].sub(lossesAt(b, i));

/* ---------- Formatting ---------- */
const num = (x: Q) => fmt(x.value, x.d === 1n ? 0 : 1);
const pct = (x: Q, d = 2) => `${fmt(x.value * 100, d)}%`;
const gradePct = (v: Pair) => `${fmt(v[1].value / v[0].value / 10, 2)}% Cu`;
/** [tonnes, grade, metal]: downstream over upstream. */
const ratios = (a: Pair, b: Pair): [Q, Q, Q] => {
  const t = a[0].div(b[0]);
  const m = a[1].div(b[1]);
  return [t, m.div(t), m];
};

/** One boundary: what is measured there, the ratio a site would report, and what FrankenMining adds. */
interface Step {
  title: string;
  key: string;
  chip: [tonnes: string, copper: string];
  state: "ok" | "masked" | "refused" | "note";
  finding: string;
  body: string;
  side: () => string;
}
const gauge = (label: string, r: Q) => {
  const v = r.value;
  const x = clamp((v - 0.75) / 0.5, 0, 1) * 100;
  const out = Math.abs(v - 1) > 0.05 + 1e-12;
  return `<div class="rc-g ${out ? "out" : "in"}"><small>${label}</small><b>${pct(r)}</b><span class="rc-track" aria-hidden="true"><i class="band"></i><i class="one"></i><i class="dot" style="left:${x.toFixed(2)}%"></i></span></div>`;
};
const gauges = (a: number) => () => {
  const [t, g, m] = ratios(NODES[a + 1]!.v, NODES[a]!.v);
  return `<div class="rc-gauges">${gauge("Dry tonnes", t)}${gauge("Grade", g)}${gauge("Contained Cu", m)}</div><p class="rc-tol">Shaded: the declared ±5% review tolerance.</p>`;
};
const lines = (rows: [string, string][]) => `<dl class="rc-lines">${rows.map(([k, v]) => `<div><dt>${k}</dt><dd>${v}</dd></div>`).join("")}</dl>`;
const STEPS: Step[] = [
  {
    title: "Resource model to grade control",
    key: "F1_tonnes@v1 and F1 metal@v1 · grade control ÷ resource model, over the mined polygon",
    chip: ["100.0%", "100.0%"],
    state: "ok",
    finding: "Within tolerance",
    body: "F1 compares two models over the same mined volume. Here they agree on tonnes, grade and metal, so whatever happens next did not start in the resource model.",
    side: gauges(0),
  },
  {
    title: "Grade control to mined",
    key: "matched_cohort.actual_over_call@v1 · North pit / bench 240 / polygon A · tolerance 500 bp",
    chip: ["120.0%", "100.0%"],
    state: "masked",
    finding: "Masked within metal tolerance: review",
    body: "The trucks carried 1,200 dry tonnes where the call said 1,000, at 83.33% of the called grade. Metal landed exactly on plan, so a check on metal alone passes. FrankenMining splits the change in metal into a tonnes part and a grade part, averaging the two orders in which they can be applied: +5,500/3 kg from the extra tonnes and −5,500/3 kg from the lower grade. It does not guess why: dilution at the dig line and an over-called grade fit the same numbers.",
    side: gauges(1),
  },
  {
    title: "Mined to the ROM pad",
    key: "Truck payload totals against the pad survey",
    chip: ["100.0%", "100.0%"],
    state: "ok",
    finding: "Closes",
    body: "Every load is dumped on the run-of-mine pad, and this month truck counts and the survey agree. When they do not, the least precise measurement, usually the truck factor, takes most of the correction. Over months that correction is estimated as a payload bias with its own interval.",
    side: gauges(2),
  },
  {
    title: "ROM pad to mill feed",
    key: "F2_tonnes@v1 · requires_stockpile_adjustment = true",
    chip: ["refused", "refused"],
    state: "refused",
    finding: "Milled ÷ mined refused across the pad",
    body: "The mill took 1,000 t of the 1,200 t this month. Dividing milled by mined across a stockpile would report a 17% loss that is really 200 t of inventory, so FrankenMining refuses that ratio as a support mismatch (FMINE-REF-102, guard G-TOUCH-3). Counted with the pad’s change, the boundary closes. Pad grade follows a declared mixing model, here well mixed.",
    side: () =>
      lines([
        ["Milled ÷ mined, as a spreadsheet would", `<s>83.33%</s> <span class="bad">refused</span>`],
        ["Tonnes, with the pad’s change", "(1,000 + 200) ÷ 1,200 = 100.00%"],
        ["Copper, with the pad’s change", "(8,333.3 + 1,666.7) ÷ 10,000 = 100.00%"],
      ]),
  },
  {
    title: "Mill feed to concentrate",
    key: "Metallurgical balance · feed = concentrate + tailings",
    chip: ["2.5%", "80.0%"],
    state: "note",
    finding: "Recovery 80.00%",
    body: "2.5% of the feed’s mass carries 80% of its copper into a 26.67% Cu concentrate, and the rest leaves in the tailings. Feed, concentrate and tailings balance in tonnes and in kilograms. Grades are derived from tonnes and metal after the balance; a direct grade reconciliation is refused (G-TOUCH-1).",
    side: () =>
      lines([
        ["Dry tonnes", "1,000 = 25 + 975"],
        ["Copper, kg", "8,333.3 = 6,666.7 + 1,666.7"],
        ["Recovery to concentrate", "80.00%"],
        ["Concentrate grade", "26.67% Cu"],
      ]),
  },
  {
    title: "Concentrate to sales",
    key: "Smelter settlement · assay exchange and outturn",
    chip: ["100.0%", "96.5%"],
    state: "note",
    finding: "Payable 96.50%",
    body: "The smelter pays for 96.5% of the copper in the concentrate. The settlement statement and the assay exchange are the last measurements in the chain, so every payable kilogram traces back, boundary by boundary, to the polygon it came from.",
    side: () =>
      lines([
        ["Contained copper shipped", "6,666.7 kg"],
        ["Payable at 96.5%", "6,433.3 kg"],
        ["Smelter deductions", "233.3 kg"],
      ]),
  },
];

/* ---------- Geometry ---------- */
const NS = "http://www.w3.org/2000/svg";
interface Geo {
  wide: boolean;
  W: number;
  H: number;
  /** Along-the-flow position of each main node's leading edge. */
  u: number[];
  nw: number;
  gap: number;
  /** Across-the-flow pixels per tonne and per kilogram. */
  k: [number, number];
  /** Across offset of the main band's edge, and of the side row. */
  a0: number;
  side: number;
}
function layout(width: number): Geo {
  if (width >= 760) {
    const nw = 10;
    const left = 14;
    const right = 56;
    const gap = (width - left - right - nw * 7) / 6;
    const main = 128;
    return { wide: true, W: width, H: 0, u: NODES.map((_, i) => left + i * (gap + nw)), nw, gap, k: [main / 1200, main / 10000], a0: 62, side: 62 + main + 42 };
  }
  const nw = 8;
  const gap = 118;
  const a0 = Math.round(width * 0.34);
  const main = Math.round(width * 0.25);
  return { wide: false, W: width, H: 0, u: NODES.map((_, i) => 14 + i * (gap + nw)), nw, gap, k: [main / 1200, main / 10000], a0, side: a0 + main + 16 };
}

export default function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("rc");
  host.innerHTML = keep(`
<div class="rc-top">
  <p class="rc-tag"><span class="tag">Synthetic</span><span>Example copper mine · North pit, bench 240, polygon A · 2026-10 · 100% project</span></p>
  <div class="rc-ctl">${segMarkup("q", "Quantity shown", [["t", "Dry tonnes"], ["kg", "Copper"]], "t")}
    <div class="rc-nav"><button type="button" class="btn sm" data-prev aria-label="Previous boundary">${icon("left")}</button><button type="button" class="btn sm" data-play>${icon("play")}<span>Play</span></button><button type="button" class="btn sm" data-next aria-label="Next boundary">${icon("right")}</button></div></div>
</div>
<ol class="rc-steps" aria-label="Boundaries">${STEPS.map(
    (s, i) =>
      `<li><button type="button" data-step="${i + 1}" aria-pressed="false" class="${s.state}" aria-label="Boundary ${i + 1}: ${s.title}"><b>${i + 1}</b><span>${NODES[i]!.short}</span><i aria-hidden="true">${icon("right")}</i><span>${NODES[i + 1]!.short}</span></button></li>`,
  ).join("")}</ol>
<div class="rc-view"><svg class="rc-svg" role="img" aria-label="Sankey diagram of polygon A’s material from the resource model to sales, in dry tonnes or copper"></svg></div>
<div class="rc-panel" aria-live="polite"></div>
<div class="rc-verdict">
  <section class="rc-sheet" aria-label="What a monthly spreadsheet reports">
    <p class="micro">A monthly spreadsheet</p>
    <div class="rc-grid" role="table" aria-label="Metal factors">
      <div role="row" class="hd"><span role="columnheader"></span><span role="columnheader">Factor</span><span role="columnheader">Metal</span><span role="columnheader">Check</span></div>
      <div role="row"><span role="cell">F1</span><span role="cell">GC ÷ model</span><span role="cell">100.0%</span><span role="cell" class="pass">pass</span></div>
      <div role="row"><span role="cell">F2</span><span role="cell">mill ÷ GC, pad adjusted</span><span role="cell">100.0%</span><span role="cell" class="pass">pass</span></div>
      <div role="row"><span role="cell">F3</span><span role="cell">F1 × F2</span><span role="cell">100.0%</span><span role="cell" class="pass">pass</span></div>
      <div role="row"><span role="cell">A</span><span role="cell">polygon A, mined ÷ call</span><span role="cell">100.0%</span><span role="cell" class="pass">pass</span></div>
    </div>
    <p class="rc-say"><b>Metal on plan.</b> All four checks pass.</p>
  </section>
  <section class="rc-fm" aria-label="What FrankenMining reports">
    <p class="micro">FrankenMining</p>
    <p class="rc-where"><b>Two offsetting misses</b>, both at <button type="button" class="rc-link" data-step="2">grade control to mined</button> on polygon A.</p>
    <div class="rc-split" aria-hidden="true"><span class="lab l">lower grade</span><div class="rc-axis"><i class="loss"></i><i class="gain"></i><b></b></div><span class="lab r">extra tonnes</span></div>
    <dl class="rc-kg">
      <div><dt>From 200 extra dry tonnes</dt><dd class="gain">+5,500/3 kg Cu <small>+1,833.3</small></dd></div>
      <div><dt>From grade at 83.33% of the call</dt><dd class="loss">−5,500/3 kg Cu <small>−1,833.3</small></dd></div>
      <div class="net"><dt>Net change in metal</dt><dd>0 kg</dd></div>
    </dl>
    <p class="rc-orders">Applied tonnes first, the split is +2,000 and −2,000 kg; grade first, +1,666.7 and −1,666.7 kg. The report averages the two orders, so neither choice decides the answer.</p>
  </section>
</div>
<div class="term rc-term">
  <div class="term-bar"><i></i><i></i><i></i><span>fmine recon run --input tests/recon/matched_cohorts.toml --format markdown</span></div>
  <table class="data rc-table">
    <thead><tr><th>Material support</th><th>Source line</th><th>Dry tonnes</th><th>Grade</th><th>Contained metal</th><th>Finding</th><th>Difference (dry t)</th><th>Difference (kg metal)</th><th>From tonnes (kg)</th><th>From grade (kg)</th></tr></thead>
    <tbody>${[
      ["North pit / bench 240 / polygon A", "9", "120.00%", "83.33%", "100.00%", "masked within metal tolerance", "200/1", "0/1", "5500/3", "-5500/3"],
      ["North pit / bench 240 / polygon B", "17", "95.00%", "100.00%", "95.00%", "within tolerance", "-50/1", "-500/1", "-500/1", "0/1"],
      ["East pit / unplanned cut", "25", "undefined", "undefined", "undefined", "unplanned movement", "200/1", "2500/1", "undefined grade", "undefined grade"],
    ]
      .map(
        (r, i) =>
          `<tr${i ? "" : ' class="rc-hit"'}>${r
            .map((c, j) => `<td data-h="${["", "Source line", "Dry tonnes", "Grade", "Contained metal", "Finding", "Difference (dry t)", "Difference (kg metal)", "From tonnes (kg)", "From grade (kg)"][j]}">${c}</td>`)
            .join("")}</tr>`,
      )
      .join("")}</tbody></table>
  <p class="rc-out">2 of 3 cohorts need review.</p>
</div>
<p class="micro asis rc-note">The worksheet <code>tests/recon/matched_cohorts.toml</code> is synthetic, and the table follows the command’s markdown report for it. Polygon B sits exactly on the 5% boundary and passes; the unplanned cut has no plan to divide by, so both its differences are shown in full. The plant and sale figures after the pad are illustrative. Touchstone reports every factor as a posterior with an interval; this screen works on declared totals and does not name the cause of a variance.</p>`);

  const svg = $<SVGSVGElement>(host, ".rc-svg");
  const panel = $(host, ".rc-panel");
  const stepButtons = $$<HTMLButtonElement>(host, ".rc-steps [data-step]");
  const playButton = $<HTMLButtonElement>(host, "[data-play]");

  let geo = layout(600);
  let mix = 0; // 0 dry tonnes, 1 copper
  let target = 0;
  let step = 0;
  let front = 0; // boundaries revealed along the flow, animated
  let frontTarget = 0;
  let raf = 0;
  let playing = 0;

  const el = <K extends keyof SVGElementTagNameMap>(name: K, attrs: Record<string, string | number>, parent: Element) => {
    const e = document.createElementNS(NS, name);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
    parent.append(e);
    return e;
  };
  interface Parts {
    clip: SVGRectElement;
    mains: { band: SVGPathElement; flow: SVGPathElement; ghost: SVGPathElement }[];
    branches: { band: SVGPathElement; flow: SVGPathElement; bar: SVGRectElement; ghost: SVGPathElement; text: SVGTextElement; spans: SVGTSpanElement[] }[];
    nodes: { bar: SVGRectElement; name: SVGTextElement; val: SVGTextElement; sub: SVGTextElement }[];
    chips: { g: SVGGElement; rect: SVGRectElement; text: SVGTextElement }[];
    gates: SVGLineElement[];
  }
  let parts: Parts | undefined;

  const map = (u: number, a: number): [number, number] => (geo.wide ? [u, a] : [a, u]);
  const pt = (u: number, a: number) =>
    map(u, a)
      .map((n) => n.toFixed(1))
      .join(" ");
  /** A band from (u0, a0..a0+w) to (u1, a1..a1+w), with tangents along the flow at both ends. */
  const band = (u0: number, a0: number, u1: number, a1: number, w: number) => {
    const um = (u0 + u1) / 2;
    return `M${pt(u0, a0)}C${pt(um, a0)} ${pt(um, a1)} ${pt(u1, a1)}L${pt(u1, a1 + w)}C${pt(um, a1 + w)} ${pt(um, a0 + w)} ${pt(u0, a0 + w)}Z`;
  };
  const centre = (u0: number, a0: number, u1: number, a1: number) => {
    const um = (u0 + u1) / 2;
    return `M${pt(u0, a0)}C${pt(um, a0)} ${pt(um, a1)} ${pt(u1, a1)}`;
  };

  function build() {
    svg.replaceChildren();
    geo.H = geo.wide ? geo.side + 148 : geo.u[6]! + geo.nw + 30;
    svg.setAttribute("viewBox", `0 0 ${geo.W} ${geo.H}`);
    svg.setAttribute("width", String(geo.W));
    svg.setAttribute("height", String(geo.H));
    svg.classList.toggle("narrow", !geo.wide);
    const defs = el("defs", {}, svg);
    const clip = el("rect", { x: 0, y: 0, width: 0, height: 0 }, el("clipPath", { id: "rc-clip" }, defs));
    for (const [id, a, b] of [
      ["rc-t", "#b48a63", "#dcb68a"],
      ["rc-k", "#e5703f", "#f2a33c"],
    ] as const) {
      const g = el("linearGradient", { id, x1: 0, y1: 0, x2: geo.wide ? 1 : 0, y2: geo.wide ? 0 : 1 }, defs);
      el("stop", { offset: 0, "stop-color": a }, g);
      el("stop", { offset: 1, "stop-color": b }, g);
    }
    const ghost = el("g", { class: "rc-ghost" }, svg);
    const gates = el("g", { class: "rc-gates" }, svg);
    const live = el("g", { class: "rc-live", "clip-path": "url(#rc-clip)" }, svg);
    const nodeLayer = el("g", { class: "rc-nodes" }, svg);
    const labels = el("g", { class: "rc-labels" }, svg);
    const chipLayer = el("g", { class: "rc-chips" }, svg);

    const mains = STEPS.map((_, i) => {
      const g = el("g", { class: "rc-b", "data-b": i + 1 }, live);
      return {
        ghost: el("path", { class: "rc-gband" }, ghost),
        band: el("path", { class: "rc-band main" }, g),
        flow: el("path", { class: "rc-flow" }, g),
      };
    });
    const branches = BRANCHES.map((br) => {
      const gg = el("path", { class: "rc-gband" }, ghost);
      const g = el("g", { class: `rc-b ${br.tone}`, "data-b": br.b }, live);
      const bandEl = el("path", { class: `rc-band ${br.tone}` }, g);
      const flow = el("path", { class: "rc-flow" }, g);
      const bar = el("rect", { class: `rc-side ${br.tone}`, rx: 1.5 }, g);
      const text = el("text", { class: `rc-slabel ${br.tone}`, "data-b": br.b }, labels);
      const spans = [el("tspan", { class: "n" }, text), el("tspan", { class: "l" }, text)];
      spans[1]!.textContent = br.label;
      if (br.sub && geo.wide) {
        const s = el("tspan", { class: "s" }, text);
        s.textContent = br.sub;
        spans.push(s);
      }
      return { band: bandEl, flow, bar, ghost: gg, text, spans };
    });
    const nodes = NODES.map((n, i) => {
      const bar = el("rect", { class: "rc-node", rx: 2, "data-n": i }, nodeLayer);
      const name = el("text", { class: "rc-name", "data-n": i }, labels);
      name.textContent = n.name;
      return { bar, name, val: el("text", { class: "rc-val", "data-n": i }, labels), sub: el("text", { class: "rc-sub", "data-n": i }, labels) };
    });
    const gateEls = STEPS.map((_, i) => el("line", { class: "rc-gate", "data-b": i + 1 }, gates));
    const chips = STEPS.map((s, i) => {
      const g = el("g", { class: `rc-chip ${s.state}`, "data-b": i + 1 }, chipLayer);
      const rect = el("rect", { rx: 9, height: 18 }, g);
      const text = el("text", { "text-anchor": "middle" }, g);
      g.addEventListener("click", () => go(i + 1, true));
      return { g, rect, text };
    });
    parts = { clip, mains, branches, nodes, chips, gates: gateEls };
    apply();
  }

  /** Puts every path, bar and label where the current mix of tonnes and copper puts it. */
  function apply() {
    if (!parts) return;
    const m = smooth(mix);
    const k = (v: Pair) => v[0].value * geo.k[0] * (1 - m) + v[1].value * geo.k[1] * m;
    const { u, nw, gap, a0, side, wide } = geo;
    const kgShown = mix >= 0.5;
    const shownTo = Math.ceil(front - 1e-6);
    for (const [i, n] of NODES.entries()) {
      const w = Math.max(1.5, k(n.v));
      const p = parts.nodes[i]!;
      const [x, y] = map(u[i]!, a0);
      p.bar.setAttribute("x", x.toFixed(1));
      p.bar.setAttribute("y", y.toFixed(1));
      p.bar.setAttribute("width", (wide ? nw : w).toFixed(1));
      p.bar.setAttribute("height", (wide ? w : nw).toFixed(1));
      p.val.textContent = kgShown ? `${num(n.v[1])} kg Cu` : `${num(n.v[0])} t`;
      p.sub.textContent = i === 6 ? (kgShown ? "payable" : "shipped") : kgShown ? `${num(n.v[0])} t` : gradePct(n.v);
      const later = i > shownTo && step > 0;
      for (const t of [p.bar, p.name, p.val, p.sub]) t.classList.toggle("later", later);
      const rows: [SVGTextElement, number][] = wide
        ? [
            [p.name, a0 - 40],
            [p.val, a0 - 24],
            [p.sub, a0 - 10],
          ]
        : [
            [p.name, u[i]! + nw / 2 - 9],
            [p.val, u[i]! + nw / 2 + 6],
            [p.sub, u[i]! + nw / 2 + 19],
          ];
      const anchor = wide ? (i === 0 ? "start" : i === 6 ? "end" : "middle") : "end";
      const lx = wide ? (i === 0 ? u[i]! : i === 6 ? u[i]! + nw : u[i]! + nw / 2) : a0 - 9;
      for (const [t, y2] of rows) {
        t.setAttribute("x", lx.toFixed(1));
        t.setAttribute("y", y2.toFixed(1));
        t.setAttribute("text-anchor", anchor);
      }
    }
    for (let b = 1; b <= 6; b++) {
      const from = u[b - 1]! + nw;
      const to = u[b]!;
      const through = k([pass(b, 0), pass(b, 1)]);
      const d = band(from, a0, to, a0, through);
      const main = parts.mains[b - 1]!;
      main.band.setAttribute("d", d);
      main.ghost.setAttribute("d", d);
      main.band.setAttribute("fill", `url(#rc-${kgShown ? "k" : "t"})`);
      main.flow.setAttribute("d", centre(from, a0 + through / 2, to, a0 + through / 2));
      main.flow.style.strokeWidth = String(clamp(through * 0.1, 0.5, 2));
      const um = (from + to) / 2;
      const [gx1, gy1] = map(um, a0 - (wide ? 2 : 4));
      const [gx2, gy2] = map(um, wide ? geo.H - 8 : geo.W - 4);
      const gate = parts.gates[b - 1]!;
      gate.setAttribute("x1", gx1.toFixed(1));
      gate.setAttribute("y1", gy1.toFixed(1));
      gate.setAttribute("x2", gx2.toFixed(1));
      gate.setAttribute("y2", gy2.toFixed(1));
      const chip = parts.chips[b - 1]!;
      chip.text.textContent = STEPS[b - 1]!.chip[kgShown ? 1 : 0];
      const cw = chip.text.textContent.length * 6.7 + 16;
      const [cx, cy] = map(um, a0 + Math.max(through, 2) / 2);
      chip.rect.setAttribute("x", (cx - cw / 2).toFixed(1));
      chip.rect.setAttribute("y", (cy - 9).toFixed(1));
      chip.rect.setAttribute("width", cw.toFixed(1));
      chip.text.setAttribute("x", cx.toFixed(1));
      chip.text.setAttribute("y", (cy + 3.8).toFixed(1));
      chip.g.classList.toggle("later", b > shownTo && step > 0);
    }
    // Branches: a loss leaves from under the band that carries on; a gain joins under it.
    const stacked = new Map<string, number>();
    for (const [j, br] of BRANCHES.entries()) {
      const p = parts.branches[j]!;
      const w = k(br.v);
      const from = u[br.b - 1]! + nw;
      const to = u[br.b]!;
      const through = k([pass(br.b, 0), pass(br.b, 1)]);
      const key = `${br.b}${br.dir}`;
      const off = stacked.get(key) ?? 0;
      stacked.set(key, off + w);
      const us = from + gap * br.at[wide ? 0 : 1];
      const sw = 5;
      if (br.dir === "out") {
        p.band.setAttribute("d", band(from, a0 + through + off, us, side, w));
        p.flow.setAttribute("d", centre(from, a0 + through + off + w / 2, us, side + w / 2));
      } else {
        p.band.setAttribute("d", band(us + sw, side, to, a0 + through + off, w));
        p.flow.setAttribute("d", centre(us + sw, side + w / 2, to, a0 + through + off + w / 2));
      }
      p.ghost.setAttribute("d", p.band.getAttribute("d")!);
      p.ghost.style.opacity = w < 0.3 ? "0" : "";
      p.flow.style.opacity = w < 0.3 ? "0" : "";
      p.flow.style.strokeWidth = String(clamp(w * 0.1, 0.5, 1.8));
      const [bx, by] = map(us, side);
      p.bar.setAttribute("x", bx.toFixed(1));
      p.bar.setAttribute("y", by.toFixed(1));
      p.bar.setAttribute("width", (wide ? sw : Math.max(w, 0.01)).toFixed(1));
      p.bar.setAttribute("height", (wide ? Math.max(w, 0.01) : sw).toFixed(1));
      const amount = br.v[kgShown ? 1 : 0];
      const sign = br.dir === "in" ? "+" : br.tone === "loss" ? "−" : "";
      p.spans[0]!.textContent = amount.n === 0n ? "" : `${sign}${kgShown ? `${num(amount)} kg` : `${num(amount)} t`}`;
      p.text.classList.toggle("none", amount.n === 0n || w < 0.3);
      p.text.classList.toggle("later", br.b > shownTo && step > 0);
      p.bar.style.opacity = w < 0.3 ? "0" : "";
      if (wide) {
        const lx = us + sw / 2;
        const ly = side + Math.max(w, 1) + 15;
        p.text.setAttribute("text-anchor", "middle");
        for (const [i, s] of p.spans.entries()) {
          s.setAttribute("x", lx.toFixed(1));
          s.setAttribute("y", (ly + i * 13).toFixed(1));
        }
      } else {
        // A gain's band leaves downward from its bar, so its words sit beside the bar; a loss's sit below it.
        const beside = br.dir === "in";
        p.text.setAttribute("text-anchor", "start");
        for (const [i, s] of p.spans.entries()) {
          s.setAttribute("x", (beside ? side + w + 7 : side).toFixed(1));
          s.setAttribute("y", (beside ? us - 3 + i * 13 : us + sw + 13 + i * 13).toFixed(1));
        }
      }
    }
    const along = front >= 6 ? 1e5 : u[0]! + nw + front * (gap + nw);
    parts.clip.setAttribute("width", String(wide ? along : geo.W));
    parts.clip.setAttribute("height", String(wide ? geo.H : along));
  }

  /* ---------- The panel under the diagram ---------- */
  function showPanel() {
    if (!step) return;
    const s = STEPS[step - 1]!;
    const tone = { ok: "in", masked: "warn", refused: "bad", note: "note" }[s.state];
    panel.innerHTML = keep(`<div class="rc-p-main"><p class="micro">Boundary ${step} of 6</p><h4>${s.title}</h4><p class="rc-key"><code>${s.key}</code></p><p>${s.body}</p><p class="rc-meas">Counted from ${NODES[step - 1]!.how}, then from ${NODES[step]!.how}.</p></div>
<div class="rc-p-side"><p class="rc-find ${tone}"><i aria-hidden="true"></i>${s.finding}</p>${s.side()}</div>`);
  }

  /* ---------- Stepping ---------- */
  function go(n: number, user = false) {
    if (user) stop();
    step = clamp(n, 1, 6);
    frontTarget = Math.max(frontTarget, step);
    for (const b of stepButtons) b.setAttribute("aria-pressed", String(Number(b.dataset.step) === step));
    host.dataset.step = String(step);
    svg.dataset.step = String(step);
    showPanel();
    kick();
  }
  function stop() {
    if (playing) clearInterval(playing);
    playing = 0;
    playButton.classList.remove("on");
    playButton.querySelector("span")!.textContent = "Play";
  }
  function play() {
    if (playing) return stop();
    playButton.classList.add("on");
    playButton.querySelector("span")!.textContent = "Pause";
    if (step >= 6) {
      front = frontTarget = 0;
      step = 0;
    }
    go(step + 1);
    playing = window.setInterval(() => {
      if (step >= 6) {
        stop();
        go(2);
        return;
      }
      go(step + 1);
    }, 3400);
  }

  /* ---------- The tween between tonnes and copper, and the reveal ---------- */
  function kick() {
    if (raf) return;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      raf = 0;
      let moving = false;
      if (mix !== target) {
        mix = reduced ? target : target > mix ? Math.min(target, mix + dt / 0.7) : Math.max(target, mix - dt / 0.7);
        moving = true;
      }
      if (front !== frontTarget) {
        front = reduced || frontTarget < front ? frontTarget : Math.min(frontTarget, front + dt / 0.8);
        moving = true;
      }
      apply();
      if (moving) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
  }

  host.dataset.q = "t";
  seg(host, "q", (v) => {
    target = v === "kg" ? 1 : 0;
    host.dataset.q = v;
    kick();
  });
  for (const b of $$<HTMLButtonElement>(host, "[data-step]")) b.addEventListener("click", () => go(Number(b.dataset.step), true));
  $(host, "[data-prev]").addEventListener("click", () => go(Math.max(1, step - 1), true));
  $(host, "[data-next]").addEventListener("click", () => go(Math.min(6, step + 1), true));
  playButton.addEventListener("click", play);

  onWidth($(host, ".rc-view"), (w) => {
    geo = layout(w);
    build();
  });

  // The panel is held at its tallest boundary, so it neither grows when it first fills in nor moves the page below it
  // as the boundaries change (Safari keeps no scroll anchor).
  tallest(
    [panel],
    6,
    () => {
      const at = step;
      return () => {
        step = at;
        if (at) showPanel();
        else panel.replaceChildren();
      };
    },
    (i) => {
      step = i + 1;
      showPanel();
    },
  );
  // On first sight the first boundary fills in, then the figure stops at the one that hides a miss.
  if (reduced) {
    front = frontTarget = 6;
    go(2);
    apply();
  } else {
    seen(host, () => {
      go(1);
      window.setTimeout(() => {
        if (!playing && step === 1) go(2);
      }, 2400);
    });
  }
}
