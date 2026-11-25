/**
 * Up and down the line: one month's outlook for a synthetic gold mine as a tree of claims, from the board's margin to
 * the loads at one face. Picking a desk lights the number it lives by and everything that number rests on. The
 * surprise sends a harder ore test at face N-07 up the tree, node by node, to the board's number the same morning.
 * Every value is computed here from the leaves; the readers and questions are plan §3's.
 */
import { chip, claimId, esc as escape, inline, json, shortId, tidy, weakest, type Ev } from "./claim-ev";
import { $, $$, reduced, seg } from "./lib";

const esc = (s: string) => escape(tidy(s));

/* ---------- The model: leaves in, every other number computed ---------- */
interface State {
  bwiN07: number;
}
const BOND = 10 / Math.sqrt(106) - 10 / Math.sqrt(2500); // Bond's law at P80 106 µm, F80 2,500 µm
const POWER = 8200; // kW drawn by the grinding circuit
const HOURS = 22 * 31;
const PRICE = 2800;
const COSTS = { Mining: 9.6, Processing: 8.9, "G&A": 3.1, "Sustaining capital": 4.2, "Royalties, refining": 1.71 };
const FACES = { s03: [31, 219.0, 13.2], n07: [26, 221.0, 14.0], e12: [19, 222.0, 15.0] } as const;

function compute(s: State) {
  const t = { s03: FACES.s03[0] * FACES.s03[1], n07: FACES.n07[0] * FACES.n07[1], e12: FACES.e12[0] * FACES.e12[1] };
  const bwi = { s03: FACES.s03[2], n07: s.bwiN07, e12: FACES.e12[2] };
  const hauled = t.s03 + t.n07 + t.e12;
  const blend = (t.s03 * bwi.s03 + t.n07 * bwi.n07 + t.e12 * bwi.e12) / hauled;
  const energy = blend * BOND;
  const tph = POWER / energy;
  const tonnes = tph * HOURS;
  const grade = 1.48 * 0.96;
  const rec = 0.893 * 0.96;
  const oz = (tonnes * grade * rec) / 31.1035;
  const costs = Object.values(COSTS).reduce((a, b) => a + b, 0);
  const aisc = (costs * 1e6) / oz;
  const margin = (oz * PRICE) / 1e6 - costs;
  return { t, bwi, hauled, blend, energy, tph, tonnes, grade, rec, oz, costs, aisc, margin };
}
type M = ReturnType<typeof compute>;

const n0 = (v: number) => Math.round(v).toLocaleString("en-US");
const n1 = (v: number) => v.toLocaleString("en-US", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const n2 = (v: number) => v.toFixed(2);

interface Node {
  id: string;
  label: string;
  /** Level (row) and half-column on a wide screen; depth in the outline on a phone. */
  l: number;
  h: number;
  d: number;
  parent?: string;
  desk: string;
  value: (m: M) => number;
  show: (v: number) => string;
  unit: string;
  sub?: (m: M) => string;
  /** Leaves carry their class; the rest take their weakest input's (NL-2). */
  ev?: (s: State) => Ev;
  /** How it is made, with the numbers in. */
  how: (m: M) => string;
}

const NODES: Node[] = [
  { id: "margin", label: "AISC margin, October", l: 0, h: 7, d: 0, desk: "board", value: (m) => m.margin, show: (v) => `$${n1(v)}M`, unit: "", how: (m) => `${n0(m.oz)} oz × ($${n0(PRICE)} − $${n0(m.aisc)}) per oz` },
  { id: "price", label: "Price deck", l: 1, h: 0, d: 1, parent: "margin", desk: "board", value: () => PRICE, show: (v) => `$${n0(v)}`, unit: "/oz", ev: () => "assumed", how: () => "The board’s planning price, an assumption" },
  { id: "ounces", label: "Gold poured", l: 1, h: 7, d: 1, parent: "margin", desk: "gm", value: (m) => m.oz, show: n0, unit: "oz", how: (m) => `${n1(m.tonnes / 1000)} kt × ${m.grade.toFixed(3)} g/t × ${(m.rec * 100).toFixed(2)}% ÷ 31.1035 g/oz` },
  { id: "tonnes", label: "Tonnes milled", l: 2, h: 3, d: 2, parent: "ounces", desk: "met", value: (m) => m.tonnes / 1000, show: n1, unit: "kt", how: (m) => `${n1(m.tph)} t/h × ${HOURS} h` },
  { id: "tph", label: "Mill throughput", l: 3, h: 2, d: 3, parent: "tonnes", desk: "met", value: (m) => m.tph, show: n1, unit: "t/h", sub: (m) => `${n2(m.energy)} kWh/t`, how: (m) => `${(POWER / 1000).toFixed(1)} MW ÷ ${n2(m.energy)} kWh/t (Bond, at the blend’s hardness)` },
  { id: "blend", label: "Blend hardness", l: 4, h: 2, d: 4, parent: "tph", desk: "met", value: (m) => m.blend, show: n2, unit: "kWh/t", how: () => "Bond work index of the faces, weighted by their tonnes" },
  { id: "s03", label: "Face S-03", l: 5, h: 0, d: 5, parent: "blend", desk: "sup", value: (m) => m.t.s03, show: n0, unit: "t/d", sub: (m) => `BWi ${n1(m.bwi.s03)}`, ev: () => "calibrated", how: () => "31 loads × 219 t a day; hardness from the geometallurgy model" },
  { id: "n07", label: "Face N-07", l: 5, h: 2, d: 5, parent: "blend", desk: "sup", value: (m) => m.t.n07, show: n0, unit: "t/d", sub: (m) => `BWi ${n1(m.bwi.n07)}`, how: () => "26 loads × 221 t a day, at its tested hardness" },
  { id: "hard", label: "N-07 hardness", l: 6, h: 0, d: 6, parent: "n07", desk: "op", value: (m) => m.bwi.n07, show: n1, unit: "kWh/t", ev: (s) => (s.bwiN07 > 14 ? "measured" : "calibrated"), how: (m) => (m.bwi.n07 > 14 ? "Lab comminution test on this morning’s face sample" : "Geometallurgy model for the face’s ore type") },
  { id: "loads", label: "N-07 loads", l: 6, h: 2, d: 6, parent: "n07", desk: "op", value: () => 26, show: n0, unit: "/day", ev: () => "measured", how: () => "Counted by the fleet system, shovel SH-02" },
  { id: "payload", label: "Mean payload", l: 6, h: 4, d: 6, parent: "n07", desk: "op", value: () => 221.0, show: n1, unit: "t", ev: () => "measured", how: () => "On-board payload, checked against the weighbridge" },
  { id: "e12", label: "Face E-12", l: 5, h: 4, d: 5, parent: "blend", desk: "sup", value: (m) => m.t.e12, show: n0, unit: "t/d", sub: (m) => `BWi ${n1(m.bwi.e12)}`, ev: () => "calibrated", how: () => "19 loads × 222 t a day; hardness from the geometallurgy model" },
  { id: "hours", label: "Run hours", l: 3, h: 4, d: 3, parent: "tonnes", desk: "met", value: () => HOURS, show: n0, unit: "h", ev: () => "calibrated", how: () => "22.0 h a day × 31 days, from the availability forecast" },
  { id: "grade", label: "Head grade", l: 2, h: 7, d: 2, parent: "ounces", desk: "geo", value: (m) => m.grade, show: (v) => v.toFixed(3), unit: "g/t", how: () => "1.48 g/t × F2 0.96" },
  { id: "gc", label: "Grade control", l: 3, h: 6, d: 3, parent: "grade", desk: "geo", value: () => 1.48, show: n2, unit: "g/t", ev: () => "calibrated", how: () => "Estimate from blast-hole assays, inside its scored domain" },
  { id: "bh", label: "Blast-hole assays", l: 4, h: 6, d: 4, parent: "gc", desk: "geo", value: () => 1240, show: n0, unit: "", ev: () => "measured", how: () => "Fire assays, reference batches passed" },
  { id: "f2", label: "F2 factor", l: 3, h: 8, d: 3, parent: "grade", desk: "geo", value: () => 0.96, show: n2, unit: "", ev: () => "derived", how: () => "Mill-reconciled grade ÷ grade control, last six months" },
  { id: "recovery", label: "Recovery", l: 2, h: 11, d: 2, parent: "ounces", desk: "met", value: (m) => m.rec * 100, show: (v) => v.toFixed(2), unit: "%", how: () => "Flotation 89.3% × leach 96.0%" },
  { id: "flot", label: "Flotation", l: 3, h: 10, d: 3, parent: "recovery", desk: "met", value: () => 89.3, show: n1, unit: "%", ev: () => "derived", how: () => "Two-product formula on a balanced survey" },
  { id: "leach", label: "Leach", l: 3, h: 12, d: 3, parent: "recovery", desk: "met", value: () => 96.0, show: n1, unit: "%", ev: () => "derived", how: () => "Concentrate leach balance" },
  { id: "aisc", label: "AISC", l: 1, h: 12, d: 1, parent: "margin", desk: "cfo", value: (m) => m.aisc, show: (v) => `$${n0(v)}`, unit: "/oz", how: (m) => `$${n2(m.costs)}M ÷ ${n0(m.oz)} oz` },
  { id: "costs", label: "AISC costs", l: 2, h: 14, d: 2, parent: "aisc", desk: "cfo", value: (m) => m.costs, show: (v) => `$${n2(v)}`, unit: "M", ev: () => "calibrated", how: () => Object.entries(COSTS).map(([k, v]) => `${k} ${v.toFixed(2)}`).join(" + ") },
];
const BY = new Map(NODES.map((n) => [n.id, n]));
const kids = (id: string) => NODES.filter((n) => n.parent === id).map((n) => n.id);
/** AISC divides by the ounces: an edge across the tree. */
const CROSS: [from: string, to: string] = ["aisc", "ounces"];
const inputs = (id: string) => [...kids(id), ...(id === CROSS[0] ? [CROSS[1]] : [])];
function evOf(id: string, s: State): Ev {
  const n = BY.get(id)!;
  if (n.ev) return n.ev(s);
  const own: Ev[] = id === "tph" ? ["calibrated"] : id === "gc" ? ["calibrated"] : [];
  return weakest([...own, ...inputs(id).map((k) => evOf(k, s))]);
}
const below = (roots: string[]) => {
  const out = new Set<string>();
  const walk = (id: string) => {
    if (out.has(id)) return;
    out.add(id);
    inputs(id).forEach(walk);
  };
  roots.forEach(walk);
  return out;
};

/* ---------- Desks: plan §3's readers where the plan names them ---------- */
interface Desk {
  id: string;
  name: string;
  roots: string[];
  reader?: string;
  q: string;
  note: (m: M) => string;
}
const DESKS: Desk[] = [
  { id: "board", name: "Board", roots: ["margin"], reader: "CEO and board", q: "Which of my numbers can I trust, and which decision this quarter would change if I knew more?", note: () => "Its evidence class is Assumed: a computed number ranks no higher than its weakest input, and the price deck is an assumption." },
  { id: "cfo", name: "CFO", roots: ["aisc"], reader: "CFO and controller", q: "Why did AISC move against guidance, which parts are controllable, and can I replay every published non-GAAP figure from source?", note: () => "The cost lines hardly move with the ore; the ounces do. AISC divides costs by ounces, so a harder face raises it." },
  { id: "gm", name: "Site GM", roots: ["ounces"], reader: "COO, technical leadership and site GMs", q: "Where is metal lost between the block model and the bullion: geology or execution?", note: () => "Three branches, each with its owner: tonnes from the mill, grade from geology, recovery from the plant." },
  { id: "geo", name: "Geologist", roots: ["grade"], reader: "Head of Mineral Resource Management and QPs/CPs", q: "Does reconciliation confirm my model, does the evidence support my classification, and will this system ever speak for me?", note: () => "F2 0.96: over six months the mill found 4% less gold than grade control predicted. Classification stays with the competent person; the system never classifies." },
  { id: "met", name: "Metallurgist", roots: ["tph", "recovery"], reader: "Plant metallurgist", q: "What feed is coming, how will it behave, and what is my real recovery with its uncertainty?", note: (m) => `Hardness sets throughput: ${(POWER / 1000).toFixed(1)} MW buys ${n1(m.tph)} t/h at ${n2(m.energy)} kWh/t. Recovery ${(m.rec * 100).toFixed(2)}% is a separate branch.` },
  { id: "sup", name: "Supervisor", roots: ["s03", "n07", "e12"], q: "Which faces are feeding the crusher today, and is the ore on the trucks the ore the plan expects?", note: (m) => `Ore hauled: 76 loads, ${n0(m.hauled)} t a day from three faces. Each face’s share sets the blend the mill sees.` },
  { id: "op", name: "Operator", roots: ["loads", "payload"], q: "Where is this load going, and is it ore?", note: () => "Every load at N-07 is counted and weighed. These leaves feed the blend, throughput, ounces and margin." },
  { id: "agent", name: "AI agent", roots: ["margin"], reader: "AI agents (internal or third-party)", q: "Give me certified, cited numbers, or a typed refusal I can act on.", note: () => "An agent reads the same tree as typed Claims, each with an id, and walks it with fmine claim trace. It drafts; people approve." },
];

/* ---------- The surprise, as it travels ---------- */
const PATH = ["hard", "n07", "blend", "tph", "tonnes", "ounces", "aisc", "margin"];
const LOG: [time: string, text: (a: M, b: M) => string][] = [
  ["06:40", (a, b) => `Lab: the N-07 face sample tests BWi ${n1(b.bwi.n07)} kWh/t. The model had ${n1(a.bwi.n07)}. The face’s claim is superseded.`],
  ["06:40", () => "Face N-07 now carries a measured hardness; its tonnes are unchanged."],
  ["06:40", (a, b) => `Blend hardness ${n2(a.blend)} → ${n2(b.blend)} kWh/t.`],
  ["06:41", (a, b) => `Mill throughput outlook ${n1(a.tph)} → ${n1(b.tph)} t/h.`],
  ["06:41", (a, b) => `Tonnes milled this month ${n1(a.tonnes / 1000)} → ${n1(b.tonnes / 1000)} kt.`],
  ["06:41", (a, b) => `Gold poured ${n0(a.oz)} → ${n0(b.oz)} oz (−${n0(a.oz - b.oz)}).`],
  ["06:41", (a, b) => `AISC $${n0(a.aisc)} → $${n0(b.aisc)}/oz: the same costs over fewer ounces.`],
  ["06:42", (a, b) => `Board pack: October margin outlook $${n1(a.margin)}M → $${n1(b.margin)}M, two minutes after the lab result.`],
];

function nodeHtml(n: Node, m: M, s: State) {
  const costs =
    n.id === "costs"
      ? `<span class="ld-bars" aria-hidden="true">${Object.values(COSTS)
          .map((v, i) => `<i style="flex:${v};--i:${i}"></i>`)
          .join("")}</span>`
      : "";
  return `<div class="ld-node" data-id="${n.id}" style="--gc:${n.h + 1} / span 2;--gr:${n.l + 1};--d:${n.d}" tabindex="-1">
  <span class="ld-l">${esc(n.label)}</span>
  <span class="ld-vv"><b class="ld-v num">${esc(n.show(n.value(m)))}</b><span class="ld-u">${esc(n.unit)}</span></span>
  ${n.sub ? `<span class="ld-sub num">${esc(n.sub(m))}</span>` : ""}${costs}
  <span class="ld-ev">${chip(evOf(n.id, s), true)}</span>
  <code class="ld-id">${shortId(claimId(`ladder:${n.id}`))}</code>
  <i class="ld-delta num" aria-hidden="true"></i>
</div>`;
}

export default function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("ld");
  const before: State = { bwiN07: 14.0 };
  const after: State = { bwiN07: 16.4 };
  const A = compute(before);
  const B = compute(after);
  let state = before;
  let m = A;
  let desk = DESKS[0]!;
  let running = false;

  host.innerHTML = `<div class="ld-bar">
  <div class="seg ld-desks" role="group" aria-label="Pick a desk" data-seg="desk">${DESKS.map(
    (d, i) => `<button type="button" data-v="${d.id}" aria-pressed="${i === 0}">${esc(d.name)}</button>`,
  ).join("")}</div>
  <p class="micro">Synthetic mine · October outlook · one ledger</p>
</div>
<div class="ld-tree" data-tree>
  <svg class="ld-lines" aria-hidden="true"></svg>
  ${NODES.map((n) => nodeHtml(n, m, state)).join("")}
  <aside class="ld-card" data-card aria-live="polite"></aside>
</div>
<ol class="ld-log" data-log aria-live="polite"></ol>`;

  const tree = $(host, "[data-tree]");
  const svg = $<SVGSVGElement>(tree, ".ld-lines");
  const card = $(host, "[data-card]");
  const log = $(host, "[data-log]");
  const el = (id: string) => $(tree, `.ld-node[data-id="${id}"]`);

  /* ---------- Lines ---------- */
  const paths = new Map<string, SVGPathElement>();
  const draw = () => {
    const grid = getComputedStyle(tree).display === "grid";
    const T = { w: tree.clientWidth, h: tree.clientHeight };
    svg.setAttribute("viewBox", `0 0 ${T.w} ${T.h}`);
    const box = (id: string) => {
      const e = el(id);
      return { x: e.offsetLeft, y: e.offsetTop, w: e.offsetWidth, h: e.offsetHeight };
    };
    let out = "";
    for (const n of NODES) {
      if (!n.parent) continue;
      const p = box(n.parent);
      const c = box(n.id);
      let d: string;
      if (grid) {
        const x1 = p.x + p.w / 2;
        const y1 = p.y + p.h;
        const x2 = c.x + c.w / 2;
        const y2 = c.y;
        const my = (y1 + y2) / 2;
        d = `M${x1} ${y1}C${x1} ${my} ${x2} ${my} ${x2} ${y2}`;
      } else {
        const x = p.x + 9;
        const y2 = c.y + Math.min(22, c.h / 2);
        d = `M${x} ${p.y + p.h}L${x} ${y2 - 6}Q${x} ${y2} ${x + 6} ${y2}L${c.x} ${y2}`;
      }
      out += `<path data-e="${n.parent}>${n.id}" d="${d}"/>`;
    }
    if (grid) {
      const a = box(CROSS[0]);
      const o = box(CROSS[1]);
      const y = o.y + o.h / 2;
      out += `<path class="cross" data-e="${CROSS[0]}>${CROSS[1]}" d="M${a.x} ${y}L${o.x + o.w} ${y}"/><text class="ld-div" x="${(a.x + o.x + o.w) / 2}" y="${y - 6}" text-anchor="middle">÷ ounces</text>`;
    }
    svg.innerHTML = out;
    paths.clear();
    for (const p of $$<SVGPathElement>(svg, "path")) paths.set(p.dataset.e!, p);
    light();
  };

  /* ---------- Desks ---------- */
  const light = () => {
    const lit = below(desk.roots);
    const owned = new Set(NODES.filter((n) => n.desk === desk.id).map((n) => n.id));
    for (const n of NODES) {
      const e = el(n.id);
      e.classList.toggle("dim", !lit.has(n.id));
      e.classList.toggle("root", desk.roots.includes(n.id));
      e.classList.toggle("own", owned.has(n.id) && !desk.roots.includes(n.id));
    }
    for (const [k, p] of paths) {
      const [a, b] = k.split(">") as [string, string];
      p.classList.toggle("lit", lit.has(a) && lit.has(b));
    }
    tree.classList.toggle("ids", desk.id === "agent");
  };
  const deskCard = () => {
    const root = BY.get(desk.roots[0]!)!;
    const second = desk.id === "met" ? BY.get("recovery")! : undefined;
    const changed = state !== before && PATH.includes(root.id);
    const old = root.show(root.value(A));
    const now = root.show(root.value(m));
    const hauled = desk.id === "sup";
    const agent = desk.id === "agent";
    const id = (k: string) => claimId(`ladder:${k}`);
    const trace = agent
      ? `<div class="term ld-term"><div class="term-bar"><i></i><i></i><i></i><span>fmine claim trace · illustrative, abridged</span></div><pre class="ld-pre">$ fmine claim trace ${shortId(id("margin"))}
${json({
  schema: "fmine.claim.trace.v1",
  root: shortId(id("margin")),
  complete_for_claim: true,
  nodes: [
    inline({ claim_id: shortId(id("margin")), parents: [shortId(id("price")), shortId(id("ounces")), shortId(id("aisc"))] }),
    inline({ claim_id: shortId(id("ounces")), parents: [shortId(id("tonnes")), shortId(id("grade")), shortId(id("recovery"))] }),
    inline(`… ${NODES.length - 2} more`),
  ],
  unresolved_parents: [],
  unanchored_leaves: [],
})}</pre></div>`
      : "";
    card.innerHTML = `<p class="micro">${esc(desk.reader ?? desk.name)} asks</p>
<h4>${esc(desk.name)}</h4>
<blockquote>“${esc(desk.q)}”</blockquote>
<div class="ld-lives"><span class="micro">Lives by</span>
  <p><span>${esc(hauled ? "Ore hauled" : root.label)}</span><b class="num">${hauled ? n0(m.hauled) : esc(now)}</b><small>${hauled ? "t/day" : esc(root.unit)}</small>${chip(evOf(hauled ? "n07" : root.id, state))}</p>
  ${changed && old !== now ? `<p class="ld-was num">was ${esc(old)} ${esc(root.unit)} before 06:40</p>` : ""}
  ${second ? `<p class="ld-also"><span>${esc(second.label)}</span><b class="num">${esc(second.show(second.value(m)))}${esc(second.unit)}</b></p>` : ""}
  <p class="ld-how">= ${esc(hauled ? "76 loads × 219–222 t, from faces S-03, N-07 and E-12" : root.how(m))}</p>
</div>
<p class="ld-note">${esc(desk.note(m))}</p>${trace}
<div class="ld-go">
  <button type="button" class="btn sm go" data-go>${state === before ? "Harder ore at face N-07" : "Replay the surprise"}</button>
  ${state === before ? "" : `<button type="button" class="btn sm" data-reset>Reset</button>`}
</div>`;
    card.querySelector("[data-go]")!.addEventListener("click", () => void surprise());
    card.querySelector("[data-reset]")?.addEventListener("click", () => reset());
  };
  seg(host, "desk", (v) => {
    desk = DESKS.find((d) => d.id === v) ?? DESKS[0]!;
    light();
    deskCard();
  });

  /* ---------- Values ---------- */
  const paint = (id: string, from: M, to: M, s: State) => {
    const n = BY.get(id)!;
    const e = el(id);
    const v = $(e, ".ld-v");
    const a = n.value(from);
    const b = n.value(to);
    $(e, ".ld-ev").innerHTML = chip(evOf(id, s), true);
    const sub = e.querySelector(".ld-sub");
    if (sub && n.sub) sub.textContent = tidy(n.sub(to));
    const delta = $(e, ".ld-delta");
    if (Math.abs(b - a) > 1e-9) {
      const dv = b - a;
      const bad = ["hard", "n07", "blend", "aisc"].includes(id) ? dv > 0 : dv < 0;
      const mag = Math.abs(dv);
      const t = n.show(mag).replace("$", "");
      delta.textContent = `${dv > 0 ? "+" : "−"}${id === "aisc" || id === "margin" ? "$" : ""}${t}`;
      delta.classList.toggle("bad", bad);
      e.classList.add("hit");
    }
    if (reduced) {
      v.textContent = tidy(n.show(b));
      return;
    }
    const t0 = performance.now();
    const step = (now: number) => {
      const k = Math.min(1, (now - t0) / 650);
      const e2 = 1 - (1 - k) ** 3;
      v.textContent = tidy(n.show(a + (b - a) * e2));
      if (k < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
    e.animate([{ boxShadow: "0 0 0 0 rgb(251 113 133 / .7)" }, { boxShadow: "0 0 0 14px rgb(251 113 133 / 0)" }], { duration: 900, easing: "ease-out" });
  };
  const pulse = (from: string, to: string) =>
    new Promise<void>((done) => {
      const p = paths.get(`${to}>${from}`) ?? paths.get(`${from}>${to}`);
      if (!p || reduced) return done();
      const reverse = paths.has(`${to}>${from}`);
      const len = p.getTotalLength();
      const dot = document.createElementNS("http://www.w3.org/2000/svg", "circle");
      dot.setAttribute("r", "5");
      dot.setAttribute("class", "ld-pulse");
      svg.append(dot);
      p.classList.add("hot");
      const t0 = performance.now();
      const step = (now: number) => {
        const k = Math.min(1, (now - t0) / 520);
        const at = p.getPointAtLength(len * (reverse ? 1 - k : k));
        dot.setAttribute("cx", String(at.x));
        dot.setAttribute("cy", String(at.y));
        if (k < 1) requestAnimationFrame(step);
        else {
          dot.remove();
          done();
        }
      };
      requestAnimationFrame(step);
    });
  const wait = (ms: number) => new Promise((r) => setTimeout(r, reduced ? 0 : ms));

  const addLog = (i: number) => {
    const [time, text] = LOG[i]!;
    const li = document.createElement("li");
    li.innerHTML = `<time class="num">${time}</time><span>${esc(text(A, B))}</span>`;
    log.append(li);
  };

  const reset = () => {
    running = false;
    state = before;
    m = A;
    for (const n of NODES) {
      const e = el(n.id);
      e.classList.remove("hit");
      $(e, ".ld-delta").textContent = "";
      $(e, ".ld-v").textContent = tidy(n.show(n.value(A)));
      $(e, ".ld-ev").innerHTML = chip(evOf(n.id, before), true);
      const sub = e.querySelector(".ld-sub");
      if (sub && n.sub) sub.textContent = tidy(n.sub(A));
    }
    for (const p of paths.values()) p.classList.remove("hot");
    log.innerHTML = "";
    host.classList.remove("after");
    deskCard();
  };

  async function surprise() {
    if (running) return;
    if (state !== before) reset();
    running = true;
    host.classList.add("after");
    state = after;
    m = B;
    deskCard();
    const go = card.querySelector<HTMLButtonElement>("[data-go]");
    if (go) go.disabled = true;
    for (let i = 0; i < PATH.length; i++) {
      if (!running) return;
      const id = PATH[i]!;
      if (i > 0) await pulse(PATH[i - 1]!, id);
      if (!running) return;
      paint(id, A, B, after);
      addLog(i);
      // The margin also divides by the ounces directly.
      if (id === "ounces") void pulse("ounces", "margin");
      await wait(380);
    }
    running = false;
    deskCard();
  }

  draw();
  deskCard();
  new ResizeObserver(() => draw()).observe(tree);
}

