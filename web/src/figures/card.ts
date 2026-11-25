/**
 * Is the test worth buying? The README's invented GB-29 DecisionCard: build an expansion (+$300M if the ore recovers
 * well, −$200M if it does not, 60/40), hold ($0), or buy metallurgical testwork first ($6M, plus $3.8M for the 180-day
 * delay). EVPI, EVSI and the net value of the test come from FrankenMining's decision engine (fmine-numerics voi) in
 * WebAssembly, recomputed as the reader changes how good the test is and how sure the team already is.
 */
import { voi, type VoiResult } from "./engine";
import { $, fmt, onWidth, range, seen } from "./lib";
import { range as rangeHtml } from "../parts";

/** payoffs[state][action]: states high and low recovery; actions build and hold. $M. */
const PAYOFFS = [
  [300, 0],
  [-200, 0],
];
const TEST = 6;
const DELAY = 3.8;
const COST = TEST + DELAY;
const PINK = "#f78fb3";
const CORAL = "#fb7185";
const GREEN = "#5fd38d";
const ACTIONS = ["Build", "Hold"];

/** Money in $M the way the README writes it: $80M, $31.2M, −$9.8M. */
export const money = (v: number, sign = false) => {
  const r = Math.round(v * 10) / 10;
  const abs = Math.abs(r);
  if (abs === 0) return "$0";
  const s = `$${fmt(abs, Number.isInteger(abs) ? 0 : 1)}M`;
  return r < 0 ? `−${s}` : sign && r > 0 ? `+${s}` : s;
};
const pct = (v: number) => `${fmt(v * 100, 0)}%`;

interface Branch {
  prob: number;
  posterior: number;
  best: number;
  value: number;
  other: number;
}
interface State {
  p: number;
  a: number;
  v: VoiResult;
  build: number;
  branches: Branch[];
}

/** The engine's answer for one prior and one accuracy, with each test result's posterior and best action. */
async function solve(p: number, a: number): Promise<State> {
  const prior = [p, 1 - p];
  const likelihood = [
    [a, 1 - a],
    [1 - a, a],
  ];
  const v = await voi({ prior, payoffs: PAYOFFS, likelihood, cost: COST });
  const branches: Branch[] = [];
  for (const s of [0, 1]) {
    // Bayes on the two states; then the engine picks the best action under that posterior.
    const joint = [p * likelihood[0]![s]!, (1 - p) * likelihood[1]![s]!];
    const prob = joint[0]! + joint[1]!;
    const post = prob > 0 ? joint[0]! / prob : p;
    const r = await voi({ prior: [post, 1 - post], payoffs: PAYOFFS });
    const ev = (act: number) => post * PAYOFFS[0]![act]! + (1 - post) * PAYOFFS[1]![act]!;
    branches.push({ prob, posterior: post, best: r.bestAction, value: r.priorValue, other: ev(1 - r.bestAction) });
  }
  const build = p * PAYOFFS[0]![0]! + (1 - p) * PAYOFFS[1]![0]!;
  return { p, a, v, build, branches };
}

interface Row {
  depth: number;
  kind: "decide" | "chance" | "end";
  label: string;
  sub?: string;
  value?: string;
  tone?: "chosen" | "rejected" | "plain";
  parent: number;
  /** The edge from the parent carries this much probability (stroke width). */
  weight?: number;
  hue?: string;
}

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.innerHTML = `<div class="card-grid">
<div class="dcard">
<header class="dcard-head"><div><p class="micro">DecisionCard · invented GB-29 exercise</p><h4>Build the expansion now, test first, or hold?</h4></div><span class="ev assumed">assumed inputs</span></header>
<div class="dcard-facts">
<table class="data dcard-pay"><thead><tr><th>Payoff, $M</th><th data-c="hi">Ore recovers well</th><th data-c="lo">Recovers poorly</th></tr></thead>
<tbody><tr><td>Build</td><td class="pos">+300</td><td class="neg">−200</td></tr><tr><td>Hold</td><td>0</td><td>0</td></tr></tbody></table>
<p class="dcard-test"><b>The test:</b> metallurgical drilling and testwork. <span data-c="acc"></span> $6M, and 180 days of delay priced at $3.8M.</p>
</div>
<div class="dtree"><svg class="chart" role="img" aria-label="Decision tree"></svg></div>
</div>
<div class="card-side">
<div class="card-verdict"><p class="micro">Best choice</p><p class="card-best" data-c="best">–</p><p class="card-why" data-c="why"></p></div>
<div class="card-bars"><svg class="chart" role="img" aria-label="Value of information"></svg></div>
<div class="card-curve"><p class="micro asis">How the test’s value grows with its accuracy</p><svg class="chart" role="img" aria-label="EVSI against test accuracy"></svg></div>
</div>
</div>
<div class="controls card-controls">
${rangeHtml("acc", "How often the test is right", 50, 100, 1, 85, "85%")}
${rangeHtml("prior", "Chance the ore recovers well, before testing", 5, 95, 1, 60, "60%")}
</div>
<p class="card-foot">Illustrative inputs from FrankenMining’s invented GB-29 exercise. <span data-c="engine">EVPI, EVSI and each branch’s best action are computed by FrankenMining’s decision engine, fmine-numerics <code>voi</code>, running in this page. It checks 0 ≤ EVSI ≤ EVPI on every call and clamps nothing.</span></p>`;

  const treeSvg = $<SVGSVGElement>(host, ".dtree svg");
  const barSvg = $<SVGSVGElement>(host, ".card-bars svg");
  const curveSvg = $<SVGSVGElement>(host, ".card-curve svg");
  const el = (k: string) => $(host, `[data-c="${k}"]`);

  let p = 0.6;
  let a = 0.85;
  let state = await solve(p, a);
  let shown = state;
  let sweep: { a: number; evsi: number }[] = [];
  let treeW = 600;
  let sideW = 420;
  let calls = 0;
  let started = false;

  const doSweep = async () => {
    const out: { a: number; evsi: number }[] = [];
    for (let i = 0; i <= 50; i++) {
      const acc = 0.5 + i / 100;
      const v = await voi({
        prior: [p, 1 - p],
        payoffs: PAYOFFS,
        likelihood: [
          [acc, 1 - acc],
          [1 - acc, acc],
        ],
        cost: COST,
      });
      out.push({ a: acc, evsi: v.evsi ?? 0 });
    }
    return out;
  };
  sweep = await doSweep();

  const choice = (s: State) => {
    const net = s.v.netVoi ?? -COST;
    if (net > 1e-9) return "test";
    return s.v.bestAction === 0 ? "build" : "hold";
  };

  const rows = (s: State): Row[] => {
    const pick = choice(s);
    const tone = (k: string): Row["tone"] => (k === pick ? "chosen" : "rejected");
    const withTest = s.v.priorValue + (s.v.evsi ?? 0) - COST;
    const best = pick === "test" ? withTest : Math.max(s.build, 0);
    const out: Row[] = [
      { depth: 0, kind: "decide", label: "Decide", sub: "best expected value", value: money(best), parent: -1, tone: "chosen" },
      { depth: 1, kind: "chance", label: "Build now", value: money(s.build), parent: 0, tone: tone("build"), weight: 1 },
      { depth: 2, kind: "end", label: "recovers well", sub: pct(s.p), value: money(300, true), parent: 1, tone: pick === "build" ? "chosen" : "plain", weight: s.p, hue: GREEN },
      { depth: 2, kind: "end", label: "recovers poorly", sub: pct(1 - s.p), value: money(-200), parent: 1, tone: pick === "build" ? "chosen" : "plain", weight: 1 - s.p, hue: CORAL },
      { depth: 1, kind: "end", label: "Hold", value: money(0), parent: 0, tone: tone("hold"), weight: 1 },
      { depth: 1, kind: "chance", label: "Test first", sub: `${money(withTest + COST)} less ${money(COST)} test and delay`, value: money(withTest), parent: 0, tone: tone("test"), weight: 1 },
    ];
    const testOn = pick === "test";
    for (const [i, b] of s.branches.entries()) {
      const at = out.length;
      out.push({
        depth: 2,
        kind: "decide",
        label: i === 0 ? "good result" : "poor result",
        sub: `${pct(b.prob)} · then ${pct(b.posterior)} chance it recovers well`,
        value: money(b.value),
        parent: 5,
        tone: testOn ? "chosen" : "plain",
        weight: b.prob,
        hue: i === 0 ? GREEN : CORAL,
      });
      for (const act of [0, 1]) {
        const chosen = act === b.best;
        out.push({
          depth: 3,
          kind: "end",
          label: ACTIONS[act]!,
          value: money(chosen ? b.value : b.other),
          parent: at,
          tone: chosen ? (testOn ? "chosen" : "plain") : "rejected",
          weight: chosen ? 1 : 0.35,
        });
      }
    }
    return out;
  };

  const drawTree = () => {
    const list = rows(shown);
    const W = treeW;
    const narrow = W < 460;
    const RH = narrow ? 30 : 29;
    const IND = narrow ? 18 : 30;
    const X0 = 12;
    // A sub-line goes under its label where the row has no room for both beside its value (estimated from the type:
    // 12.5px Inter is about 7.4px a character, the 11px mono sub-line 6.6px, the 12px mono value 7.2px).
    const two = (r: Row) => {
      if (!r.sub) return false;
      if (narrow) return r.sub.length > 18;
      const need = X0 + r.depth * IND + 13 + r.label.length * 7.4 + 8 + r.sub.length * 6.6;
      return need > W - 18 - (r.value?.length ?? 0) * 7.2;
    };
    const ys: number[] = [];
    let acc = 4;
    for (const r of list) {
      ys.push(acc + RH / 2);
      acc += RH + (two(r) ? 13 : 0);
    }
    const H = acc + 4;
    const gx = (r: Row) => X0 + r.depth * IND;
    const gy = (i: number) => ys[i]!;
    let s = "";
    // Edges first.
    for (const [i, r] of list.entries()) {
      if (r.parent < 0) continue;
      const pr = list[r.parent]!;
      const x0 = gx(pr);
      const y0 = gy(r.parent) + 7;
      const x1 = gx(r) - 8;
      const y1 = gy(i);
      const d = `M${x0} ${y0}V${y1 - 6}Q${x0} ${y1} ${x0 + 6} ${y1}H${x1}`;
      const w = 1 + 3.5 * (r.weight ?? 1);
      const cls = r.tone === "chosen" ? "e on" : r.tone === "rejected" ? "e off" : "e";
      s += `<path class="${cls}" d="${d}" stroke-width="${w.toFixed(2)}"${r.hue && r.tone !== "rejected" ? ` style="--hue:${r.hue}"` : ""}/>`;
    }
    for (const [i, r] of list.entries()) {
      const x = gx(r);
      const y = gy(i);
      const cls = `n ${r.tone ?? "plain"}`;
      let g = "";
      if (r.kind === "decide") g = `<rect x="${x - 6}" y="${y - 6}" width="12" height="12" rx="2"/>`;
      else if (r.kind === "chance") g = `<circle cx="${x}" cy="${y}" r="6.5"/>`;
      else g = `<path d="M${x - 5} ${y - 6}L${x + 6} ${y}L${x - 5} ${y + 6}Z"/>`;
      const tx = x + 13;
      // On two lines, a sub-line that leads with a short figure ("57% · then 89% chance it recovers well") keeps the
      // figure beside its label and puts the rest underneath.
      const cut = two(r) ? r.sub!.indexOf(" · ") : -1;
      const [lead, rest] = !two(r) ? [r.sub ?? "", ""] : cut > 0 && cut <= 6 ? [r.sub!.slice(0, cut), r.sub!.slice(cut + 3)] : ["", r.sub!];
      const label = `<tspan>${r.label}</tspan>${lead ? `<tspan class="sub" dx="8">${lead}</tspan>` : ""}`;
      s += `<g class="${cls}">${g}<text x="${tx}" y="${y + 4}" class="lab">${label}</text>${r.value ? `<text x="${W - 6}" y="${y + 4}" text-anchor="end" class="val">${r.value}</text>` : ""}</g>`;
      if (rest) s += `<text x="${tx}" y="${y + 17}" class="sub2">${rest}</text>`;
    }
    treeSvg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    treeSvg.innerHTML = s;
  };

  const drawBars = () => {
    const v = shown.v;
    const W = sideW;
    const evsi = v.evsi ?? 0;
    const net = v.netVoi ?? -COST;
    const items: [string, number, string, string][] = [
      ["Perfect information, EVPI", v.evpi, "ceiling", "rgb(255 255 255 / .22)"],
      ["This test, gross EVSI", evsi, "evsi", PINK],
      ["Test and delay cost", -COST, "cost", CORAL],
      ["Net value of the test", net, "net", net >= 0 ? GREEN : CORAL],
    ];
    const lo = -20;
    const hi = Math.max(100, v.evpi * 1.05);
    const LW = W < 420 ? 0 : 170;
    const X = (m: number) => LW + 6 + ((m - lo) / (hi - lo)) * (W - LW - 70);
    const RH = W < 420 ? 44 : 34;
    let s = `<path class="axis" d="M${X(0)} 2V${items.length * RH + 2}"/>`;
    for (const [i, [name, val, key, colour]] of items.entries()) {
      const y = 4 + i * RH + (W < 420 ? 16 : 0);
      if (W < 420) s += `<text x="0" y="${y - 4}" class="bl">${name}</text>`;
      else s += `<text x="0" y="${y + 15}" class="bl">${name}</text>`;
      const x0 = X(Math.min(0, val));
      const x1 = X(Math.max(0, val));
      const fill = key === "ceiling" ? `fill="none" stroke="rgb(255 255 255 / .5)" stroke-dasharray="4 3"` : `fill="${colour}"`;
      s += `<rect class="cb" x="${x0.toFixed(1)}" y="${y + 3}" width="${Math.max(1, x1 - x0).toFixed(1)}" height="16" rx="3" ${fill}/>`;
      s += `<text x="${(x1 + 7).toFixed(1)}" y="${y + 16}" class="bv">${money(val)}</text>`;
    }
    // The EVPI ceiling carried down over the EVSI bar.
    s += `<path d="M${X(v.evpi).toFixed(1)} ${RH * 0.6}V${RH * 2 + 2}" stroke="#fff" stroke-opacity=".5" stroke-dasharray="2 3"/>`;
    barSvg.setAttribute("viewBox", `0 0 ${W} ${items.length * RH + 6}`);
    barSvg.innerHTML = s;
  };

  const drawCurve = () => {
    const W = sideW;
    const H = W < 420 ? 190 : 200;
    const L = 40;
    const R = 10;
    const T = 22;
    const B = 34;
    const top = Math.max(100, shown.v.evpi * 1.15);
    const X = (acc: number) => L + ((acc - 0.5) / 0.5) * (W - L - R);
    const Y = (m: number) => T + (1 - m / top) * (H - T - B);
    let s = "";
    for (let m = 0; m <= top; m += top > 150 ? 50 : 20) s += `<path class="grid" d="M${L} ${Y(m)}H${W - R}"/><text x="${L - 6}" y="${Y(m) + 4}" text-anchor="end">${m}</text>`;
    for (let acc = 0.5; acc <= 1.0001; acc += 0.1) s += `<text x="${X(acc)}" y="${H - B + 16}" text-anchor="middle">${fmt(acc * 100, 0)}%</text>`;
    s += `<path class="axis" d="M${L} ${T}V${H - B}H${W - R}"/>`;
    // Where no test result would change the decision.
    const kink = sweep.reduce((k, pt) => (pt.evsi <= 1e-9 ? pt.a : k), 0.5);
    if (kink > 0.5) {
      s += `<rect x="${X(0.5)}" y="${T}" width="${X(kink) - X(0.5)}" height="${H - B - T}" fill="rgb(255 255 255 / .035)"/>`;
      if (X(kink) - X(0.5) > 70) s += `<text x="${(X(0.5) + X(kink)) / 2}" y="${(Y(COST) + Y(shown.v.evpi)) / 2 - 3}" text-anchor="middle" class="cv-note">no result</text><text x="${(X(0.5) + X(kink)) / 2}" y="${(Y(COST) + Y(shown.v.evpi)) / 2 + 10}" text-anchor="middle" class="cv-note">changes the call</text>`;
    }
    s += `<path d="M${L} ${Y(shown.v.evpi)}H${W - R}" stroke="#fff" stroke-opacity=".55" stroke-dasharray="5 4"/><text x="${W - R - 4}" y="${Y(shown.v.evpi) - 6}" text-anchor="end" class="cv-l">EVPI ${money(shown.v.evpi)}: the ceiling</text>`;
    s += `<path d="M${L} ${Y(COST)}H${W - R}" stroke="${CORAL}" stroke-opacity=".8" stroke-dasharray="2 3"/><text x="${W - R - 4}" y="${Y(COST) - 5}" text-anchor="end" class="cv-c">cost ${money(COST)}</text>`;
    const path = sweep.map((pt, i) => `${i ? "L" : "M"}${X(pt.a).toFixed(1)} ${Y(pt.evsi).toFixed(1)}`).join("");
    s += `<path d="${path}L${X(1)} ${Y(0)}L${X(0.5)} ${Y(0)}Z" fill="${PINK}" fill-opacity=".12"/><path d="${path}" fill="none" stroke="${PINK}" stroke-width="2.4"/>`;
    // Break-even: the first accuracy at which the test pays for itself.
    const even = sweep.find((pt) => pt.evsi > COST);
    if (even) s += `<path d="M${X(even.a)} ${Y(COST)}V${H - B}" stroke="${CORAL}" stroke-opacity=".5"/><text x="${X(even.a) - 5}" y="${Y(COST) - 5}" text-anchor="end" class="cv-c">pays from ${fmt(even.a * 100, 0)}%</text>`;
    const evsi = shown.v.evsi ?? 0;
    s += `<path d="M${X(shown.a)} ${Y(evsi)}V${H - B}" stroke="${PINK}" stroke-opacity=".6"/><circle cx="${X(shown.a)}" cy="${Y(evsi)}" r="6" fill="${PINK}" stroke="#0d0b0a" stroke-width="2"/>`;
    s += `<text x="${X(shown.a) + (shown.a > 0.85 ? -10 : 10)}" y="${Y(evsi) + (shown.a > 0.85 ? -10 : 18)}" text-anchor="${shown.a > 0.85 ? "end" : "start"}" class="cv-v">EVSI ${money(evsi)}</text>`;
    s += `<text x="${W - R}" y="${H - 4}" text-anchor="end">test accuracy →</text><text x="${L - 6}" y="${T - 12}" text-anchor="end">$M</text>`;
    curveSvg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    curveSvg.innerHTML = s;
  };

  const words = () => {
    const s = shown;
    const pick = choice(s);
    const evsi = s.v.evsi ?? 0;
    const net = s.v.netVoi ?? -COST;
    el("best").textContent = pick === "test" ? "Test first" : pick === "build" ? "Build now" : "Hold";
    el("best").dataset.pick = pick;
    el("why").textContent =
      pick === "test"
        ? `The test is worth ${money(evsi)} before its cost and ${money(net)} after it. Perfect information would be worth ${money(s.v.evpi)}, the ceiling for any test.`
        : evsi <= 1e-9
          ? `No test result would change the decision, so the test is worth nothing and costs ${money(COST)}.`
          : `The test could change the decision, but it is worth ${money(evsi)} and costs ${money(COST)}.`;
    el("acc").textContent = `It reads the ore right ${pct(s.a)} of the time, in either state.`;
    el("hi").textContent = `Ore recovers well, ${pct(s.p)}`;
    el("lo").textContent = `Recovers poorly, ${pct(1 - s.p)}`;
  };

  const draw = () => {
    words();
    drawTree();
    drawBars();
    drawCurve();
  };

  const update = async (resweep: boolean) => {
    const id = ++calls;
    const next = await solve(p, a);
    const nextSweep = resweep ? await doSweep() : sweep;
    if (id !== calls) return;
    state = next;
    sweep = nextSweep;
    shown = state;
    draw();
    host.classList.remove("flash");
    void host.offsetWidth;
    host.classList.add("flash");
  };

  range(host, "acc", (v) => `${fmt(v, 0)}%`, (v) => {
    a = v / 100;
    void update(false);
  });
  range(host, "prior", (v) => `${fmt(v, 0)}%`, (v) => {
    p = v / 100;
    void update(true);
  });
  onWidth($(host, ".dtree"), (w) => {
    treeW = w;
    if (started) drawTree();
  });
  onWidth($(host, ".card-side"), (w) => {
    sideW = w;
    if (started) {
      drawBars();
      drawCurve();
    }
  });
  started = true;
  draw();
  seen(host, () => host.classList.add("grow"));
}
