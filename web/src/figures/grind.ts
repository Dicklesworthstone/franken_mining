/**
 * Average ore is not average throughput. FrankenMining's Bond grinding model (fmine-plant grinding::evaluate, in
 * WebAssembly) runs on every hardness state and averages after; the same model run once at the average hardness
 * gives the number a plan built on averages would use. The gap between them is Jensen's inequality, drawn here on
 * the convex curve of throughput against work index. Every number shown comes back from the engine.
 */
import { EngineError, grind, type GrindResult } from "./engine";
import { makeMill, oreColour } from "./grind-mill";
import { $, fit, fmt, loop, onWidth, range, seg } from "./lib";
import { range as rangeHtml, read as readHtml, seg as segHtml } from "../parts";

/** The README's illustrative circuit: 1,000 kW at 80% utilisation, F80 400 µm to P80 100 µm. */
const CIRCUIT = { f80: 400, p80: 100, powerKw: 1000, utilization: 0.8 };
const MANY = 41;
const X_MAX = 40;
const Y_MAX = 520;
const TEAL = "#34d3b5";
const AMBER = "#fbbf24";

/** Acklam's inverse normal CDF, for equally likely states at the quantiles of a lognormal hardness. */
function qnorm(p: number): number {
  const a = [-39.69683028665376, 220.9460984245205, -275.9285104469687, 138.357751867269, -30.66479806614716, 2.506628277459239];
  const b = [-54.47609879822406, 161.5858368580409, -155.6989798598866, 66.80131188771972, -13.28068155288572];
  const c = [-0.007784894002430293, -0.3223964580411365, -2.400758277161838, -2.549732539343734, 4.374664141464968, 2.938163982698783];
  const d = [0.007784695709041462, 0.3224671290700398, 2.445134137142996, 3.754408661907416];
  const poly = (k: number[], x: number) => k.reduce((s, v) => s * x + v, 0);
  if (p < 0.02425) {
    const q = Math.sqrt(-2 * Math.log(p));
    return poly(c, q) / (poly(d, q) * q + 1);
  }
  if (p > 1 - 0.02425) return -qnorm(1 - p);
  const q = p - 0.5;
  const r = q * q;
  return (poly(a, r) * q) / (poly(b, r) * r + 1);
}

/** The hardness states: two equally likely ores at mean ± sd, or a lognormal spread as equally likely quantiles. */
export function states(mode: string, mean: number, sd: number): number[] {
  if (mode === "two") return [mean - sd, mean + sd];
  if (sd <= 0) return Array.from({ length: MANY }, () => mean);
  const s = Math.sqrt(Math.log(1 + (sd / mean) ** 2));
  const mu = Math.log(mean) - (s * s) / 2;
  return Array.from({ length: MANY }, (_, i) => Math.exp(mu + s * qnorm((i + 0.5) / MANY)));
}

const t = (v: number) => `${fmt(v, Math.abs(v) < 10 ? 1 : 0)} t/h`;
const kwh = (v: number) => `${fmt(v, 2)} kWh/t`;

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.innerHTML = `<div class="grind">
<div class="grind-mill"><div class="grind-view"><canvas aria-hidden="true"></canvas></div>
<div class="grind-hud"><div><p class="micro">One possible state</p><p class="grind-state"><b data-g="wi">–</b> <span data-g="chance"></span></p></div><p class="grind-tph"><b data-g="tph">–</b><small>t/h</small></p></div>
<p class="grind-cap">Ore enters through the trunnion and leaves as fines. Soft ore breaks fast and hard ore lingers, so at fixed power the feed rate follows hardness.</p></div>
<div class="grind-chart"><svg class="chart" role="img" aria-label="Throughput against hardness"></svg></div>
</div>
<div class="grind-bar controls">
${segHtml("mix", "Hardness states", [["two", "Two states"], ["many", "A spread of states"]], "two")}
${rangeHtml("mean", "Average hardness, Wi", 8, 24, 0.5, 12.5, "12.5 kWh/t")}
${rangeHtml("sd", "Spread, standard deviation", 0, 10, 0.5, 7.5, "7.5 kWh/t")}
</div>
<div class="reads grind-reads">
${readHtml("mean", "Expected throughput", "–", "each state run through the model, then averaged")}
${readHtml("at", "At the average hardness", "–", "the model run once on the average Wi")}
${readHtml("gap", "The gap", "–", "")}
${readHtml("energy", "Specific energy", "–", "")}
</div>
<p class="grind-who">A plant budget built on the average hardness understates throughput by this gap, so every cost per tonne in it comes out too high. FrankenMining runs the model on every hardness state and averages after.</p>
<p class="grind-foot">Illustrative circuit: 1,000 kW ball mill at 80% utilisation, F80 400 µm to P80 100 µm. Each state is an equally likely alternative for the whole feed. <span data-g="engine">Computed by FrankenMining’s Bond model, fmine-plant <code>grinding::evaluate</code>, compiled to WebAssembly.</span></p>`;

  const svg = $<SVGSVGElement>(host, ".grind-chart svg");
  const canvas = $<HTMLCanvasElement>(host, ".grind-mill canvas");
  const mill = makeMill(canvas);
  const hud = {
    wi: $(host, '[data-g="wi"]'),
    chance: $(host, '[data-g="chance"]'),
    tph: $(host, '[data-g="tph"]'),
    engine: $(host, '[data-g="engine"]'),
  };
  const reads = (name: string) => ({
    b: $(host, `[data-read="${name}"] b`),
    span: $(host, `[data-read="${name}"] span`),
  });
  const rMean = reads("mean");
  const rAt = reads("at");
  const rGap = reads("gap");
  const rEnergy = reads("energy");

  // The curve itself, from the engine: one call, one scenario per point.
  const curveWi = Array.from({ length: 150 }, (_, i) => 1.2 + (i / 149) * (X_MAX - 1.2));
  const curve = await grind({ wi: curveWi, ...CIRCUIT });

  let mode = "two";
  let mean = 12.5;
  let sd = 7.5;
  let result: GrindResult | undefined;
  let wis: number[] = [];
  let refusal = "";
  let width = 600;
  let showIndex = 0;
  let showClock = 0;
  let calls = 0;

  /** The states the mill takes turns showing: both, or five spread across the distribution. */
  const showList = () =>
    wis.length === 2 ? [0, 1] : [0.1, 0.3, 0.5, 0.7, 0.9].map((p) => Math.min(wis.length - 1, Math.floor(p * wis.length)));

  const setMill = () => {
    if (!result) return;
    const list = showList();
    const i = list[showIndex % list.length]!;
    const wi = wis[i]!;
    const tph = result.points.throughput[i]!;
    mill.set({ wi, tph });
    hud.wi.textContent = `Wi ${fmt(wi, 1)} kWh/t`;
    hud.wi.style.color = oreColour(wi);
    hud.chance.textContent = wis.length === 2 ? "chance 1 in 2" : `chance 1 in ${wis.length}`;
    hud.tph.textContent = fmt(tph, 0);
    draw();
  };

  const run = async () => {
    const id = ++calls;
    const list = states(mode, mean, sd);
    try {
      const r = await grind({ wi: list, ...CIRCUIT });
      if (id !== calls) return;
      result = r;
      wis = list;
      refusal = "";
      // A time of zero means under one tick of the clock, a whole millisecond in Safari.
      hud.engine.textContent = `Computed by FrankenMining’s Bond model, fmine-plant grinding::evaluate, compiled to WebAssembly: ${list.length} states in ${r.ms < 0.005 ? "under 1" : r.ms < 1 ? r.ms.toFixed(2) : r.ms.toFixed(1)} ms.`;
    } catch (e) {
      if (id !== calls) return;
      if (!(e instanceof EngineError)) throw e;
      const m = /work_index_kwh_per_dry_t=([\d.]+) lies outside declared \[([\d.]+), ([\d.]+)\]/.exec(e.message);
      refusal = m
        ? `${e.code ?? "Refused"} · Wi ${fmt(Number(m[1]), 1)} kWh/t lies outside the model’s declared validity box, ${m[2]}–${m[3]}. No extrapolation.`
        : e.message;
    }
    fillReads();
    setMill();
    draw();
  };

  const fillReads = () => {
    host.classList.toggle("refused", !!refusal);
    if (!result) return;
    const r = result;
    const gap = r.throughput.mean - r.atMeanWi.throughput;
    rMean.b.textContent = t(r.throughput.mean);
    rMean.span.textContent = `P10 ${fmt(r.throughput.p10, 0)}, P90 ${fmt(r.throughput.p90, 0)} t/h across ${wis.length} states`;
    rAt.b.textContent = t(r.atMeanWi.throughput);
    rAt.span.textContent = `the model run once at the average, Wi ${fmt(r.atMeanWi.wi, 2)} kWh/t`;
    rGap.b.textContent = `${gap >= 0 ? "+" : ""}${t(gap)}`;
    rGap.span.textContent =
      Math.abs(gap) < 0.05
        ? "no spread, so no gap"
        : `${fmt((gap * 8760) / 1e6, 2)} Mt a year; the plan sees ${fmt((Math.abs(gap) / r.throughput.mean) * 100, 0)}% fewer tonnes than it should expect`;
    rEnergy.b.textContent = kwh(r.energy.mean);
    rEnergy.span.textContent = `${kwh(r.atMeanWi.energy)} at the average Wi: energy is linear in hardness, so its average has no gap`;
  };

  const draw = () => {
    const W = width;
    const narrow = W < 520;
    const H = narrow ? 330 : 410;
    const L = narrow ? 40 : 52;
    const Rm = narrow ? 10 : 18;
    const T = 16;
    const B = narrow ? 64 : 58;
    const X = (wi: number) => L + (wi / X_MAX) * (W - L - Rm);
    const Y = (q: number) => T + (1 - q / Y_MAX) * (H - T - B);
    const base = Y(0);
    let s = `<defs><linearGradient id="gr-fill" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${TEAL}" stop-opacity=".13"/><stop offset="1" stop-color="${TEAL}" stop-opacity="0"/></linearGradient><pattern id="gr-hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><path d="M0 0V6" stroke="${AMBER}" stroke-opacity=".5" stroke-width="1.4"/></pattern><clipPath id="gr-clip"><rect x="${L}" y="${T}" width="${W - L - Rm}" height="${base - T}"/></clipPath></defs>`;
    for (let q = 0; q <= 500; q += 100)
      s += `<path class="grid" d="M${L} ${Y(q).toFixed(1)}H${W - Rm}"/><text x="${L - 8}" y="${Y(q) + 4}" text-anchor="end">${q}</text>`;
    for (let wi = 0; wi <= X_MAX; wi += narrow ? 10 : 5)
      s += `<path class="grid" d="M${X(wi).toFixed(1)} ${T}V${base}"/><text x="${X(wi)}" y="${base + 16}" text-anchor="middle">${wi}</text>`;
    s += `<path class="axis" d="M${L} ${T}V${base}H${W - Rm}"/>`;
    s += `<text x="${W - Rm}" y="${base + 34}" text-anchor="end">Bond work index Wi, kWh/t · harder →</text>`;
    s += `<text x="${L}" y="${T - 4}" text-anchor="start" class="gr-unit">t/h</text>`;
    // The curve, from the engine.
    const pts: [number, number][] = curveWi.map((wi, i) => [X(wi), Y(curve.points.throughput[i]!)]);
    const path = pts.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`).join("");
    const under = pts.filter((_, i) => curve.points.throughput[i]! <= Y_MAX);
    const first = under[0];
    const area = first ? `M${first[0].toFixed(1)} ${base}${under.map(([x, y]) => `L${x.toFixed(1)} ${y.toFixed(1)}`).join("")}L${X(X_MAX)} ${base}Z` : "";
    s += `<g clip-path="url(#gr-clip)"><path d="${area}" fill="url(#gr-fill)"/><path d="${path}" fill="none" stroke="${TEAL}" stroke-width="2.4"/></g>`;
    if (!narrow) {
      s += `<text x="${W - Rm - 8}" y="${T + 18}" text-anchor="end" class="gr-curve">throughput = power × utilisation ÷ E</text>`;
      s += `<text x="${W - Rm - 8}" y="${T + 36}" text-anchor="end" class="gr-curve dim">E = 10 Wi (1/√P80 − 1/√F80)</text>`;
    }
    const r = result;
    if (r) {
      const mWi = r.atMeanWi.wi;
      const atQ = r.atMeanWi.throughput;
      const eQ = r.throughput.mean;
      // The hardness distribution, along the bottom.
      if (wis.length === 2) {
        for (const [i, wi] of wis.entries()) {
          const q = r.points.throughput[i]!;
          const x = X(wi);
          s += `<path d="M${x.toFixed(1)} ${base}V${Math.max(T, Y(q)).toFixed(1)}" stroke="${oreColour(wi)}" stroke-opacity=".45" stroke-dasharray="2 4"/>`;
          s += `<rect x="${(x - 9).toFixed(1)}" y="${base - 26}" width="18" height="26" rx="3" fill="${oreColour(wi, 0.85)}"/>`;
          s += `<text x="${(x - 13).toFixed(1)}" y="${base - 6}" text-anchor="end" class="gr-w">50%</text>`;
        }
        const [a, b] = [0, 1].map((i) => [X(wis[i]!), Y(r.points.throughput[i]!)] as const);
        if (a && b && Math.abs(a[0] - b[0]) > 1)
          s += `<path d="M${a[0].toFixed(1)} ${a[1].toFixed(1)}L${b[0].toFixed(1)} ${b[1].toFixed(1)}" stroke="#fff" stroke-opacity=".35" stroke-dasharray="5 5" clip-path="url(#gr-clip)"/>`;
      } else {
        // A density silhouette from the equally likely states.
        const bins = 60;
        const dens = new Array<number>(bins).fill(0);
        const bw = Math.max(0.6, (sd || 1) * 0.35);
        for (let k = 0; k < bins; k++) {
          const x = ((k + 0.5) / bins) * X_MAX;
          for (const wi of wis) dens[k]! += Math.exp(-0.5 * ((x - wi) / bw) ** 2);
        }
        const peak = Math.max(...dens, 1e-9);
        let d = `M${X(0)} ${base}`;
        for (let k = 0; k < bins; k++) d += `L${X(((k + 0.5) / bins) * X_MAX).toFixed(1)} ${(base - (dens[k]! / peak) * 34).toFixed(1)}`;
        d += `L${X(X_MAX)} ${base}Z`;
        s += `<path d="${d}" fill="${oreColour(mean, 0.32)}" stroke="${oreColour(mean, 0.8)}"/>`;
        for (const wi of wis) s += `<path d="M${X(wi).toFixed(1)} ${base}v-5" stroke="${oreColour(wi)}"/>`;
      }
      // The gap, shaded from the axis to the average hardness, between the two answers.
      const gy0 = Math.max(T, Y(Math.max(atQ, eQ)));
      const gy1 = Y(Math.min(atQ, eQ));
      if (gy1 - gy0 > 0.5) {
        s += `<rect x="${L}" y="${gy0.toFixed(1)}" width="${(X(mWi) - L).toFixed(1)}" height="${(gy1 - gy0).toFixed(1)}" fill="url(#gr-hatch)"/>`;
        s += `<rect x="${L}" y="${gy0.toFixed(1)}" width="${(X(mWi) - L).toFixed(1)}" height="${(gy1 - gy0).toFixed(1)}" fill="${AMBER}" fill-opacity=".07"/>`;
      }
      s += `<path d="M${L} ${Y(eQ).toFixed(1)}H${X(mWi).toFixed(1)}" stroke="#fff" stroke-width="1.4"/>`;
      s += `<path d="M${L} ${Y(atQ).toFixed(1)}H${X(mWi).toFixed(1)}" stroke="${AMBER}" stroke-width="1.4" stroke-dasharray="4 4"/>`;
      s += `<path d="M${X(mWi).toFixed(1)} ${base}V${Math.max(T, Y(Math.max(eQ, atQ))).toFixed(1)}" stroke="#fff" stroke-opacity=".3" stroke-dasharray="2 3"/>`;
      // The states themselves on the curve.
      const list = showList();
      const on = list[showIndex % list.length];
      for (const [i, wi] of wis.entries()) {
        const q = r.points.throughput[i]!;
        const big = wis.length === 2;
        const y = Y(q);
        if (y < T) {
          s += `<path d="M${X(wi) - 6} ${T + 9}L${X(wi)} ${T}L${X(wi) + 6} ${T + 9}Z" fill="${oreColour(wi)}"/><text x="${X(wi) + 9}" y="${T + 9}" class="gr-w">${fmt(q, 0)}</text>`;
          continue;
        }
        s += `<circle cx="${X(wi).toFixed(1)}" cy="${y.toFixed(1)}" r="${big ? 7 : 3.2}" fill="${oreColour(wi)}" stroke="#0d0b0a" stroke-width="${big ? 2 : 1}"/>`;
        if (i === on) s += `<circle cx="${X(wi).toFixed(1)}" cy="${y.toFixed(1)}" r="${big ? 13 : 8}" fill="none" stroke="#fff" stroke-opacity=".7" class="gr-on"/>`;
        if (big) s += `<text x="${(X(wi) + 12).toFixed(1)}" y="${(y < (T + base) / 2 ? y - 8 : y + 18).toFixed(1)}" class="gr-pt gr-halo">${fmt(q, 0)} t/h</text>`;
      }
      // The two answers.
      s += `<circle cx="${X(mWi).toFixed(1)}" cy="${Y(atQ).toFixed(1)}" r="6.5" fill="#0d0b0a" stroke="${AMBER}" stroke-width="2.4"/>`;
      s += `<circle cx="${X(mWi).toFixed(1)}" cy="${Y(eQ).toFixed(1)}" r="6.5" fill="#fff"/>`;
      const gap = eQ - atQ;
      const lx = X(mWi) + 12;
      const eText = narrow ? `${fmt(eQ, 0)} expected` : `${fmt(eQ, 0)} t/h expected`;
      const aText = narrow ? `${fmt(atQ, 0)} at average Wi` : `${fmt(atQ, 0)} t/h at the average Wi`;
      const room = W - Rm - lx > (narrow ? 112 : 190);
      const labelX = room ? lx : X(mWi) - 12;
      const anchor = room ? "start" : "end";
      const sep = Math.abs(Y(eQ) - Y(atQ)) < 30;
      s += `<text x="${labelX.toFixed(1)}" y="${(Y(eQ) - (sep ? 8 : -4)).toFixed(1)}" text-anchor="${anchor}" class="label gr-halo">${eText}</text>`;
      s += `<text x="${labelX.toFixed(1)}" y="${(Y(atQ) + (sep ? 18 : 4)).toFixed(1)}" text-anchor="${anchor}" class="gr-at gr-halo">${aText}</text>`;
      if (Math.abs(gap) >= 0.5) {
        const mid = (Y(eQ) + Y(atQ)) / 2;
        s += `<text x="${(L + 8).toFixed(1)}" y="${(mid + 4).toFixed(1)}" class="gr-gap">gap ${fmt(gap, 0)} t/h</text>`;
      }
      s += `<text x="${X(mWi).toFixed(1)}" y="${base + (narrow ? 50 : 50)}" text-anchor="middle" class="gr-mean">average Wi ${fmt(mWi, 1)}</text>`;
    }
    if (refusal) {
      s += `<foreignObject x="${L + 10}" y="${T + 6}" width="${Math.max(200, W - L - Rm - 20)}" height="80"><div xmlns="http://www.w3.org/1999/xhtml" class="gr-refusal"><span class="ev refused">refused</span> ${refusal}</div></foreignObject>`;
    }
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    svg.innerHTML = s;
  };

  seg(host, "mix", (v) => {
    mode = v;
    showIndex = 0;
    void run();
  });
  range(host, "mean", (v) => `${fmt(v, 1)} kWh/t`, (v) => {
    mean = v;
    void run();
  });
  range(host, "sd", (v) => `${fmt(v, 1)} kWh/t`, (v) => {
    sd = v;
    void run();
  });

  onWidth($(host, ".grind-chart"), (w) => {
    width = w;
    draw();
  });

  const frames = loop(fig, (dt) => {
    showClock += dt;
    if (showClock > 3.6 && result) {
      showClock = 0;
      showIndex++;
      setMill();
    }
    mill.frame(dt);
  });
  let sized = false;
  fit(canvas, (w, h, ratio) => {
    mill.resize(w, h, ratio);
    if (!sized) {
      sized = true;
      mill.warm();
    }
    frames.poke();
  });

  await run();
  mill.warm();
  frames.poke();
}
