/**
 * Forecasts, frozen then scored. Twelve quarters of a synthetic site: each quarter's gold guidance is registered and
 * frozen (with the sha256 of its text) before the quarter opens, and scored when the outcome arrives; the grade
 * model's forecast for every blast polygon resolves as the polygon is mined. A running calibration view and track
 * record update as outcomes land, and FrankenMining's betting e-process (engine.monitor) watches the polygon stream.
 * Switch on the drift and it raises its alarm inside Q7, weeks before that quarter's guidance misses.
 */
import { DRIFT_AT, PER, QUARTERS, calibrationScore, plan, world, type Guide, type Plan, type World } from "./journal-data";
import { $, clamp, fmt, loop, onWidth, range, reduced, seen, tallest, toggle } from "./lib";
import { icon, range as rangeHtml, toggle as toggleHtml } from "../parts";

const END = QUARTERS + 0.15;
const SECONDS_PER_QUARTER = 1.6;
/** A quarter's guidance is registered just before the quarter opens and resolved just after it closes. */
const REGISTER = -0.15;
const RESOLVE = 1.12;
const PINK = "#f78fb3";
const CORAL = "#fb7185";
const GREEN = "#5fd38d";
const VIOLET = "#b7a2fb";

const LOCK = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/></svg>`;

type Phase = "none" | "frozen" | "scored";
const phase = (q: number, tau: number): Phase => (tau < q + REGISTER ? "none" : tau < q + RESOLVE ? "frozen" : "scored");
const short = (h: string) => `${h.slice(0, 8)}…${h.slice(-4)}`;
const koz = (v: number) => fmt(v, 1);

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.innerHTML = `<div class="jr">
<div class="jr-main"><svg class="chart jr-time" role="img" aria-label="Forecasts and outcomes over twelve quarters"></svg>
<div class="jr-bar">
<div class="jr-buttons"><button type="button" class="btn sm jr-play" data-j="play" aria-label="Play">${icon("play")}<span>Play</span></button><button type="button" class="btn sm" data-j="step">${icon("right")}<span>Next quarter</span></button><button type="button" class="btn sm" data-j="restart" aria-label="Start again">${icon("redo")}</button></div>
${toggleHtml("drift", "The grade model drifts after Q6")}
${rangeHtml("tau", "Time", 0, END, 0.01, 0, "before Q1")}
</div></div>
<aside class="jr-side">
<div class="jr-card frozen" data-j="frozen"></div>
<div class="jr-card scored" data-j="scored"></div>
<div class="jr-cal" data-j="cal"></div>
</aside>
</div>
<p class="jr-foot">Synthetic site and forecasts, generated in your browser. Each polygon’s outcome is drawn from its own frozen forecast, so the record is calibrated until the drift: from Q7 each polygon comes in one forecast standard deviation low. CRPS, the 80% interval score and the PIT are the standard formulas, computed in the page. <span data-j="engine">The drift monitor is FrankenMining’s betting e-process from fmine-calib, compiled to WebAssembly and fed each polygon’s standardised residual; it alarms when either bet’s wealth reaches 2/α = 40, which under a calibrated model happens with probability at most 5%, however long it runs.</span></p>`;

  const svg = $<SVGSVGElement>(host, ".jr-time");
  const el = (k: string) => $(host, `[data-j="${k}"]`);
  const playBtn = $<HTMLButtonElement>(host, '[data-j="play"]');

  const p: Plan = plan();
  const worlds = { calm: await world(p, false), drift: await world(p, true) };
  el("engine").textContent += ` Both histories, ${2 * p.polys.length} observations, took ${fmt(worlds.calm.ms + worlds.drift.ms, 1)} ms.`;
  let drift = false;
  let tau = 0;
  let playing = false;
  let width = 760;
  let lastKey = "";
  let refusal = "";
  let refusalAt = 0;
  const W = () => (drift ? worlds.drift : worlds.calm);

  const lo = Math.min(...p.guides.map((g) => g.p10), ...worlds.drift.outcomes.map((o) => o.value), ...worlds.calm.outcomes.map((o) => o.value)) - 2;
  const hi = Math.max(...p.guides.map((g) => g.p90), ...worlds.drift.outcomes.map((o) => o.value), ...worlds.calm.outcomes.map((o) => o.value)) + 2;

  /** Polygons mined by time tau: they resolve through their quarter. */
  const mined = (t: number) => clamp(Math.floor(t * PER), 0, p.polys.length);

  const geometry = () => {
    const w = width;
    const narrow = w < 560;
    const L = narrow ? 34 : 44;
    const R = 8;
    const X = (t: number) => L + (t / QUARTERS) * (w - L - R);
    const A = { top: 22, h: narrow ? 120 : 150 };
    const B = { top: A.top + A.h + 34, h: narrow ? 96 : 112 };
    const C = { top: B.top + B.h + 34, h: narrow ? 96 : 112 };
    const H = C.top + C.h + 48;
    return { w, narrow, L, R, X, A, B, C, H };
  };

  const render = () => {
    const g = geometry();
    const { X, A, B, C, L, H, narrow } = g;
    const w = W();
    const n = mined(tau);
    const qw = X(1) - X(0);
    let s = `<defs><pattern id="jr-hatch" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><path d="M0 0V5" stroke="${PINK}" stroke-opacity=".55" stroke-width="1.3"/></pattern></defs>`;
    // Drift region.
    if (drift) {
      const by = C.top + C.h + 27;
      s += `<rect x="${X(6)}" y="${A.top - 6}" width="${X(QUARTERS) - X(6)}" height="${C.top + C.h - A.top + 6}" fill="${CORAL}" fill-opacity=".045"/>`;
      s += `<path d="M${X(6) + 3} ${by - 4}V${by}H${X(QUARTERS) - 3}V${by - 4}" fill="none" stroke="${CORAL}" stroke-opacity=".7"/><text x="${(X(6) + X(QUARTERS)) / 2}" y="${by + 13}" text-anchor="middle" class="jr-drift">${narrow ? "model drifts, 1 sd low" : "the grade model drifts: each polygon 1 sd low"}</text>`;
    }
    // Quarter grid and labels.
    for (let q = 0; q <= QUARTERS; q++) s += `<path class="grid" d="M${X(q).toFixed(1)} ${A.top}V${C.top + C.h}"/>`;
    for (let q = 0; q < QUARTERS; q++)
      if (!narrow || q % 2 === 0) s += `<text x="${(X(q) + qw / 2).toFixed(1)}" y="${C.top + C.h + 17}" text-anchor="middle" class="jr-q">Q${q + 1}</text>`;
    // Panel A: guidance.
    const YA = (v: number) => A.top + (1 - (v - lo) / (hi - lo)) * A.h;
    s += `<text x="${L}" y="${A.top - 9}" class="jr-pt">${narrow ? "Gold guidance, koz · frozen P10–P90" : "Quarterly gold guidance, koz · frozen P10–P90"}</text>`;
    for (let v = Math.ceil(lo / 5) * 5; v <= hi; v += 5) s += `<text x="${L - 6}" y="${YA(v) + 4}" text-anchor="end">${v}</text>`;
    s += `<path class="axis" d="M${L} ${A.top}V${A.top + A.h}"/>`;
    p.guides.forEach((gd, q) => {
      const ph = phase(q, tau);
      const x0 = X(q) + qw * 0.2;
      const bw = qw * 0.6;
      const cxq = X(q) + qw / 2;
      if (ph === "none") {
        s += `<rect x="${x0.toFixed(1)}" y="${A.top + 4}" width="${bw.toFixed(1)}" height="${A.h - 8}" rx="3" class="jr-slot"/>`;
        return;
      }
      const y0 = YA(gd.p90);
      const y1 = YA(gd.p10);
      const o = w.outcomes[q]!;
      if (ph === "frozen") {
        s += `<rect x="${x0.toFixed(1)}" y="${y0.toFixed(1)}" width="${bw.toFixed(1)}" height="${(y1 - y0).toFixed(1)}" rx="3" fill="url(#jr-hatch)" stroke="${PINK}" stroke-opacity=".7"/>`;
        s += `<path d="M${x0.toFixed(1)} ${YA(gd.mean).toFixed(1)}h${bw.toFixed(1)}" stroke="${PINK}" stroke-width="1.6"/>`;
        s += `<g class="jr-lock" transform="translate(${(cxq - 6).toFixed(1)} ${(y0 - 17).toFixed(1)}) scale(.5)">${LOCK.replace(/<\/?svg[^>]*>/g, "")}</g>`;
      } else {
        const tone = o.side === 0 ? GREEN : CORAL;
        s += `<rect x="${x0.toFixed(1)}" y="${y0.toFixed(1)}" width="${bw.toFixed(1)}" height="${(y1 - y0).toFixed(1)}" rx="3" fill="${tone}" fill-opacity=".16" stroke="${tone}" stroke-opacity=".55"/>`;
        s += `<path d="M${x0.toFixed(1)} ${YA(gd.mean).toFixed(1)}h${bw.toFixed(1)}" stroke="#fff" stroke-opacity=".4"/>`;
        const fresh = tau - (q + RESOLVE) < 0.35;
        if (fresh) s += `<circle cx="${cxq.toFixed(1)}" cy="${YA(o.value).toFixed(1)}" r="5" class="jr-ping" stroke="${tone}"/>`;
        s += `<circle cx="${cxq.toFixed(1)}" cy="${YA(o.value).toFixed(1)}" r="${narrow ? 3.6 : 4.6}" fill="${tone}" stroke="#0d0b0a" stroke-width="1.5"/>`;
        if (!narrow) s += `<text x="${cxq.toFixed(1)}" y="${(A.top + A.h + 13).toFixed(1)}" text-anchor="middle" class="jr-crps">${fmt(o.crps, 1)}</text>`;
      }
    });
    if (!narrow) s += `<text x="${L - 6}" y="${A.top + A.h + 13}" text-anchor="end" class="jr-crps">CRPS</text>`;
    // Panel B: polygons.
    const ZM = 3.4;
    const YB = (z: number) => B.top + (1 - (clamp(z, -ZM, ZM) + ZM) / (2 * ZM)) * B.h;
    s += `<text x="${L}" y="${B.top - 9}" class="jr-pt">${narrow ? "Each polygon’s grade: outcome − forecast, sd" : "Grade model, every blast polygon: outcome against forecast, in sd"}</text>`;
    s += `<rect x="${L}" y="${YB(1.2816).toFixed(1)}" width="${X(QUARTERS) - L}" height="${(YB(-1.2816) - YB(1.2816)).toFixed(1)}" fill="#fff" fill-opacity=".04"/>`;
    s += `<path d="M${L} ${YB(0)}H${X(QUARTERS)}" stroke="#fff" stroke-opacity=".18"/>`;
    for (const z of [-2, 0, 2]) s += `<text x="${L - 6}" y="${YB(z) + 4}" text-anchor="end">${z > 0 ? "+" : z < 0 ? "−" : ""}${Math.abs(z)}</text>`;
    s += `<path class="axis" d="M${L} ${B.top}V${B.top + B.h}"/>`;
    let dots = "";
    for (let k = 0; k < n; k++) {
      const z = w.z[k]!;
      const x = X((k + 0.5) / PER);
      dots += `<circle cx="${x.toFixed(1)}" cy="${YB(z).toFixed(1)}" r="${narrow ? 1.5 : 1.9}" fill="${w.hit[k] ? "#e9dfd6" : CORAL}" fill-opacity="${w.hit[k] ? 0.5 : 0.9}"/>`;
    }
    s += dots;
    // A running mean of the last 24 residuals: the bias, if any.
    if (n > 4) {
      let d = "";
      for (let k = 3; k < n; k++) {
        const from = Math.max(0, k - 23);
        let sum = 0;
        for (let j = from; j <= k; j++) sum += w.z[j]!;
        d += `${k === 3 ? "M" : "L"}${X((k + 0.5) / PER).toFixed(1)} ${YB(sum / (k - from + 1)).toFixed(1)}`;
      }
      s += `<path d="${d}" fill="none" stroke="${PINK}" stroke-width="2"/>`;
    }
    if (!narrow) s += `<text x="${X(QUARTERS) - 4}" y="${YB(1.2816) - 4}" text-anchor="end" class="jr-band">80% interval</text>`;
    // Panel C: the e-process.
    const LMIN = -1.2;
    const LMAX = 2.7;
    const YC = (l10: number) => C.top + (1 - (clamp(l10, LMIN, LMAX) - LMIN) / (LMAX - LMIN)) * C.h;
    const thr = w.logThreshold / Math.LN10;
    s += `<text x="${L}" y="${C.top - 9}" class="jr-pt">${narrow ? "Drift monitor: two bets’ wealth, log scale" : "Drift monitor: wealth of two bets against the model (log scale)"}</text>`;
    for (const [v, t] of [[-1, "0.1"], [0, "1"], [1, "10"], [2, "100"]] as const) s += `<text x="${L - 6}" y="${YC(v) + 4}" text-anchor="end">${t}</text>`;
    s += `<path class="axis" d="M${L} ${C.top}V${C.top + C.h}"/>`;
    // Words inside the panels go on top, after the time cursor, so that it passes behind them.
    let over = `<text x="${L + 6}" y="${YC(thr) - 5}" class="jr-thr">alarm at 40 = 2/α</text>`;
    s += `<path d="M${L} ${YC(thr).toFixed(1)}H${X(QUARTERS)}" stroke="${CORAL}" stroke-dasharray="5 4"/>`;
    const line = (arr: Float64Array, colour: string, width2: number) => {
      if (!n) return "";
      let d = `M${L} ${YC(0).toFixed(1)}`;
      for (let k = 0; k < n; k++) d += `L${X((k + 1) / PER).toFixed(1)} ${YC(arr[k]! / Math.LN10).toFixed(1)}`;
      return `<path d="${d}" fill="none" stroke="${colour}" stroke-width="${width2}" stroke-linejoin="round"/>`;
    };
    s += line(w.logD, VIOLET, 1.4) + line(w.logL, PINK, 2.2);
    for (let k = 0; k < n; k++)
      if (Math.max(w.logL[k]!, w.logD[k]!) / Math.LN10 > LMAX) {
        over += `<text x="${X(QUARTERS) - 4}" y="${C.top + 11}" text-anchor="end" class="jr-thr">off the scale ↑</text>`;
        break;
      }
    if (n) {
      const lx = X(n / PER);
      s += `<circle cx="${lx.toFixed(1)}" cy="${YC(w.logL[n - 1]! / Math.LN10).toFixed(1)}" r="3.5" fill="${PINK}"/>`;
    }
    if (!narrow) over += `<text x="${L + 6}" y="${C.top + C.h - 6}" class="jr-key"><tspan fill="${PINK}">━ bias bet</tspan><tspan dx="12" fill="${VIOLET}">━ spread bet</tspan></text>`;
    // The alarm, once raised, across every panel.
    if (w.alarmAt >= 0 && n > w.alarmAt) {
      const ax = X((w.alarmAt + 1) / PER);
      s += `<path d="M${ax.toFixed(1)} ${A.top}V${A.top + A.h}M${ax.toFixed(1)} ${B.top}V${B.top + B.h}M${ax.toFixed(1)} ${C.top}V${C.top + C.h}" stroke="${CORAL}" stroke-width="1.6"/>`;
      const q = Math.floor(w.alarmAt / PER) + 1;
      const k = (w.alarmAt % PER) + 1;
      const text = narrow ? `ALARM · Q${q}, polygon ${k}` : `ALARM · Q${q}, polygon ${k} · the bias bet reached 40×`;
      const bw = text.length * 6.9 + 14;
      const bx = Math.min(ax + 5, X(QUARTERS) - bw - 2);
      over += `<g class="jr-alarm"><rect x="${bx.toFixed(1)}" y="${C.top + C.h - 21}" width="${bw.toFixed(1)}" height="18" rx="5"/><text x="${(bx + 7).toFixed(1)}" y="${C.top + C.h - 8.5}" class="a1">${text}</text></g>`;
    }
    // Not yet happened: dimmed.
    const px = X(clamp(tau, 0, QUARTERS));
    s += `<rect x="${px.toFixed(1)}" y="${A.top}" width="${Math.max(0, X(QUARTERS) - px).toFixed(1)}" height="${C.top + C.h - A.top}" fill="#0d0b0a" fill-opacity=".35" pointer-events="none"/>`;
    // Through each panel only: the gaps between them hold the panels' titles, which a line would cross out.
    const through = (x: string, a: number, c: number) => `M${x} ${A.top - a}V${A.top + A.h}M${x} ${B.top}V${B.top + B.h}M${x} ${C.top}V${C.top + C.h + c}`;
    s += `<path d="${through(px.toFixed(1), 2, 2)}" stroke="#fff" stroke-opacity=".8" stroke-width="1.2"/><path d="M${(px - 5).toFixed(1)} ${C.top + C.h + 7}l5 -6l5 6z" fill="#fff"/>` + over;
    svg.setAttribute("viewBox", `0 0 ${g.w} ${H}`);
    svg.innerHTML = s;
  };

  /** The forecast in play: the earliest one frozen and still waiting for its outcome. */
  const pending = (): Guide | undefined => p.guides.find((g) => phase(g.q, tau) === "frozen");
  const lastScored = () => [...p.guides].reverse().find((g) => phase(g.q, tau) === "scored");

  const side = () => {
    const w = W();
    const g = pending();
    const frozen = el("frozen");
    if (g) {
      frozen.innerHTML = `<p class="jr-tag">${LOCK}<span>Frozen</span><em>Q${g.q + 1} gold guidance</em></p>
<p class="jr-range"><span>P10 <b>${koz(g.p10)}</b></span><span>P50 <b>${koz(g.mean)}</b></span><span>P90 <b>${koz(g.p90)}</b></span><small>koz</small></p>
<p class="jr-meta">Registered before Q${g.q + 1} opened. sha256 <code title="${g.hash}">${short(g.hash)}</code></p>
<div class="jr-acts"><button type="button" class="btn sm" data-act="edit">Edit this forecast</button>${g.q > 0 ? `<button type="button" class="btn sm" data-act="late">Register a forecast for Q${g.q}</button>` : ""}</div>
${refusal && performance.now() - refusalAt < 6000 ? `<p class="jr-refusal"><span class="ev refused">refused</span> ${refusal}</p>` : ""}`;
    } else frozen.innerHTML = `<p class="jr-tag done"><span>Record complete</span><em>every forecast resolved</em></p><p class="jr-meta">Nothing is left to freeze or to edit. Play it again with the drift switched ${drift ? "off" : "on"}.</p>`;
    const sc = lastScored();
    const scored = el("scored");
    if (sc) {
      const o = w.outcomes[sc.q]!;
      const verdict = o.side === 0 ? "inside P10–P90" : o.side < 0 ? "below P10" : "above P90";
      scored.className = `jr-card scored ${o.side === 0 ? "jr-hit" : "jr-miss"}`;
      scored.innerHTML = `<p class="jr-tag">${icon(o.side === 0 ? "check" : "cross")}<span>Scored</span><em>Q${sc.q + 1} gold guidance</em></p>
<p class="jr-range"><span>outcome <b>${koz(o.value)}</b> koz</span><span class="v">${verdict}</span></p>
<p class="jr-meta">CRPS <b>${fmt(o.crps, 2)}</b> koz · 80% interval score <b>${fmt(o.score, 1)}</b> · PIT <b>${fmt(o.pit, 2)}</b></p>`;
    } else {
      scored.className = "jr-card scored";
      scored.innerHTML = `<p class="jr-tag"><span>Scored</span><em>nothing yet</em></p><p class="jr-meta">Q1’s outcome arrives after Q1 closes. Until then its forecast stays frozen.</p>`;
    }
    // Calibration so far.
    const n = mined(tau);
    const pits: number[] = [];
    let hits = 0;
    for (let k = 0; k < n; k++) {
      pits.push(w.pit[k]!);
      hits += w.hit[k]!;
    }
    const bins = new Array<number>(10).fill(0);
    for (const u of pits) bins[Math.min(9, Math.floor(u * 10))]!++;
    const peak = Math.max(4, ...bins, (n / 10) * 1.8);
    const hw = 200;
    const hh = 62;
    let hist = "";
    bins.forEach((c, i) => {
      const h = (c / peak) * hh;
      const tail = i === 0 || i === 9;
      hist += `<rect x="${i * 20 + 1}" y="${(hh - h).toFixed(1)}" width="18" height="${h.toFixed(1)}" rx="2" fill="${tail ? CORAL : PINK}" fill-opacity="${tail ? 0.75 : 0.55}"/>`;
    });
    if (n) hist += `<path d="M0 ${(hh - (n / 10 / peak) * hh).toFixed(1)}H${hw}" stroke="#fff" stroke-opacity=".55" stroke-dasharray="3 3"/>`;
    const scoredQ = p.guides.filter((gd) => phase(gd.q, tau) === "scored");
    const inside = scoredQ.filter((gd) => w.outcomes[gd.q]!.side === 0).length;
    const meanCrps = scoredQ.length ? scoredQ.reduce((t, gd) => t + w.outcomes[gd.q]!.crps, 0) / scoredQ.length : NaN;
    const cal = calibrationScore(pits, hits);
    const alarmed = w.alarmAt >= 0 && n > w.alarmAt;
    const k = Math.max(0, n - 1);
    el("cal").innerHTML = `<p class="micro">Calibration, as outcomes land</p>
<div class="jr-hist"><svg viewBox="0 0 ${hw} ${hh + 14}" class="chart" aria-label="PIT histogram">${hist}<text x="0" y="${hh + 12}">0</text><text x="${hw}" y="${hh + 12}" text-anchor="end">1</text><text x="${hw / 2}" y="${hh + 12}" text-anchor="middle">PIT of each polygon</text></svg>
<p>Flat means calibrated. Outcomes piling into the low bins mean the model forecasts too high.</p></div>
<dl class="jr-dl">
<dt>Polygons inside their 80% interval</dt><dd>${n ? `${hits} of ${n} · ${fmt((hits / n) * 100, 0)}%` : "–"}</dd>
<dt>Grade model calibration score</dt><dd>${n >= 10 ? fmt(cal, 2) : "–"}</dd>
<dt>Guidance inside P10–P90</dt><dd>${scoredQ.length ? `${inside} of ${scoredQ.length}` : "–"}</dd>
<dt>Guidance mean CRPS</dt><dd>${scoredQ.length ? `${fmt(meanCrps, 2)} koz` : "–"}</dd>
<dt>Monitor</dt><dd class="${alarmed ? "alarm" : ""}">${alarmed ? `alarm at Q${Math.floor(w.alarmAt / PER) + 1}` : n ? `quiet · bias bet ${((e) => fmt(e, e < 10 ? 2 : 1))(Math.exp(w.logL[k]!))}×` : "armed"}</dd>
</dl>`;
  };

  const refresh = (force = false) => {
    const n = mined(tau);
    const phases = p.guides.map((g) => phase(g.q, tau)[0]).join("");
    const fresh = p.guides.some((g) => {
      const d = tau - (g.q + RESOLVE);
      return d >= 0 && d < 0.4;
    });
    const key = `${n}|${phases}|${drift}|${width}|${fresh ? Math.floor(tau * 20) : ""}|${refusal}`;
    if (!force && key === lastKey) return;
    lastKey = key;
    render();
    side();
    slider.set(tau);
  };

  const label = (t: number) => {
    if (t >= QUARTERS) return "all twelve quarters";
    const q = Math.floor(t) + 1;
    const k = Math.floor((t % 1) * PER);
    return t <= 0 ? "before Q1" : `Q${q}, polygon ${k + 1} of ${PER}`;
  };
  const slider = range(host, "tau", label, (v) => {
    tau = v;
    setPlaying(false);
    refresh();
  });

  const setPlaying = (on: boolean) => {
    playing = on;
    playBtn.innerHTML = on ? `${icon("pause")}<span>Pause</span>` : `${icon("play")}<span>${tau >= END ? "Replay" : "Play"}</span>`;
    playBtn.setAttribute("aria-label", on ? "Pause" : "Play");
    frames.running = on;
  };

  const frames = loop(
    fig,
    (dt) => {
      if (!playing) return;
      tau = Math.min(END, tau + dt / SECONDS_PER_QUARTER);
      if (tau >= END) setPlaying(false);
      refresh();
    },
    false,
  );

  playBtn.addEventListener("click", () => {
    if (!playing && tau >= END) tau = 0;
    setPlaying(!playing);
    refresh(true);
  });
  el("step").addEventListener("click", () => {
    setPlaying(false);
    // To just after the next outcome lands.
    const next = p.guides.find((g) => g.q + RESOLVE > tau + 1e-6);
    tau = next ? Math.min(END, next.q + RESOLVE + 0.02) : END;
    refresh(true);
  });
  el("restart").addEventListener("click", () => {
    tau = 0;
    setPlaying(!reduced);
    refresh(true);
  });
  toggle(host, "drift", (on) => {
    drift = on;
    // Q1–Q6 are the same in both histories; replay from where they part.
    if (tau > DRIFT_AT / PER) {
      tau = DRIFT_AT / PER - 0.05;
      setPlaying(!reduced);
    }
    refresh(true);
  });
  host.addEventListener("click", (e) => {
    const b = (e.target as Element).closest<HTMLButtonElement>("[data-act]");
    if (!b) return;
    const g = pending();
    if (!g) return;
    refusal =
      b.dataset.act === "edit"
        ? `<b>FMINE-REF-507</b> frozen record mutation. Q${g.q + 1} guidance is frozen as <code>${short(g.hash)}</code>; a changed forecast is a new forecast under a new ID, and this one is still scored.`
        : `<b>FMINE-REF-206</b> late registration. Q${g.q}’s outcome is already recorded, so a forecast for it can no longer be registered.`;
    refusalAt = performance.now();
    refresh(true);
    const chip = host.querySelector(".jr-refusal");
    chip?.classList.add("shake");
    setTimeout(() => {
      if (performance.now() - refusalAt >= 5900) {
        refusal = "";
        refresh(true);
      }
    }, 6000);
  });

  onWidth($(host, ".jr-main"), (w) => {
    width = w;
    refresh(true);
  });

  // Where the panel sits under the chart (below 1,000 px), it is held at its tallest: the record playing out then never
  // moves the page below it (Safari keeps no scroll anchor). Its height changes only between a few states (before Q1,
  // mid-record with a quarter scored, the record complete, the monitor's alarm), so a few points in time, with and
  // without the drift, find it; each one costs a layout.
  const AT = [0, 0.5, 1.5, 2.5, 5.5, 8.5, QUARTERS - 0.5, END];
  tallest(
    [$(host, ".jr-side")],
    2 * AT.length,
    () => {
      const [t, d] = [tau, drift];
      return () => {
        [tau, drift] = [t, d];
        side();
      };
    },
    (i) => {
      drift = i >= AT.length;
      tau = AT[i % AT.length]!;
      side();
    },
    matchMedia("(max-width: 999px)"),
  );
  if (reduced) {
    tau = END;
    setPlaying(false);
    refresh(true);
  } else {
    refresh(true);
    seen(host, () => {
      if (tau === 0) setPlaying(true);
    });
  }
}
