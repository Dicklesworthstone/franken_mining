/**
 * The Claim, exploded. Six numbers from a synthetic mine sit in a row like spreadsheet cells; opening one lays out
 * what the universal Claim of plan §8 carries: value and interval, support, evidence class, the calibration record of
 * the method, the unknown-unknown term, provenance, guards, and what `fmine claim why` prints for it.
 */
import { ANCHORS, CLAIMS, SPACE, TIME, UU_FORMS, type ClaimData, type Dist } from "./claim-data";
import { EV, FLAGS, MORE, claimId, esc as escape, group, inline, json, shortId, tidy, type Ev } from "./claim-ev";

/** Prose: escaped, with ids kept whole across a line break. */
const esc = (s: string) => escape(tidy(s));
import { $, $$, reduced, seen } from "./lib";

export const EVC: Record<Ev, string> = {
  measured: "#5fd38d",
  derived: "#2dd4bf",
  calibrated: "#6cc8f7",
  uncalibrated: "#b7a2fb",
  transferred: "#f2a33c",
  elicited: "#f2a33c",
  assumed: "#fbbf24",
  assertion: "#fb7185",
};
const RUNGS: Ev[][] = [["measured"], ["derived"], ["calibrated"], ["uncalibrated", "transferred", "elicited"], ["assumed"], ["assertion"]];
const EPI: Record<string, string> = {
  head: "KnownUnknown(Aleatory)",
  payload: "KnownUnknown(Aleatory)",
  recovery: "KnownUnknown(Parametric)",
  aisc: "KnownUnknown(Parametric)",
  fos: "KnownUnknown(Parametric)",
  freeboard: "KnownUnknown(Aleatory)",
};
const COLS = "ABCDEF";
const Z80 = 1.2816;

const ln = (s: string) => esc(s).replace(/\n/g, " ");
const head = (n: number, title: string, extra = "") =>
  `<header class="cl-h"><i class="cl-n">${n}</i><span>${title}</span>${extra}</header>`;

function band(d: Dist): [lo: number, mid: number, hi: number] {
  return d.kind === "normal" ? [d.mean - Z80 * d.sd, d.mean, d.mean + Z80 * d.sd] : [d.p10, d.p50, d.p90];
}
function density(d: Dist, x: number) {
  if (d.kind === "normal") return Math.exp(-0.5 * ((x - d.mean) / d.sd) ** 2) / d.sd;
  const sl = (d.p50 - d.p10) / Z80;
  const sr = (d.p90 - d.p50) / Z80;
  return (Math.exp(-0.5 * ((x - d.p50) / (x < d.p50 ? sl : sr)) ** 2) * 2) / (sl + sr);
}

/** The value's distribution, its 80% band, and the unknown-unknown allowance drawn as a floor under it. */
function chart(c: ClaimData, w: number) {
  const h = 136;
  const top = 26;
  const base = 100;
  const [a, b] = c.axis;
  const X = (v: number) => 6 + ((v - a) / (b - a)) * (w - 12);
  const peak = Math.max(...Array.from({ length: 201 }, (_, i) => density(c.dist, a + ((b - a) * i) / 200)));
  const Y = (v: number) => base - (density(c.dist, v) / peak) * (base - top);
  const curve = (lo: number, hi: number) => {
    const n = 90;
    let s = `M${X(lo).toFixed(1)} ${base}`;
    for (let i = 0; i <= n; i++) {
      const v = lo + ((hi - lo) * i) / n;
      s += `L${X(v).toFixed(1)} ${Y(v).toFixed(1)}`;
    }
    return `${s}L${X(hi).toFixed(1)} ${base}Z`;
  };
  const [lo, mid, hi] = band(c.dist);
  const ticks = w < 380 ? c.ticks.filter((_, i) => i % 2 === 0) : c.ticks;
  const fmt = (v: number) => v.toLocaleString("en-US", { minimumFractionDigits: c.digits === 0 ? 0 : c.digits, maximumFractionDigits: c.digits });
  let out = `<svg class="cl-dist" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" role="img" aria-label="Distribution of ${esc(c.tile)}: 80% interval ${esc(c.interval)} ${esc(c.unit)}">
<defs><linearGradient id="cl-g-${c.key}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="var(--chapter)" stop-opacity=".5"/><stop offset="1" stop-color="var(--chapter)" stop-opacity=".04"/></linearGradient>
<pattern id="cl-hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><path d="M0 0v6" stroke="#fbbf24" stroke-opacity=".55" stroke-width="2"/></pattern></defs>
<path class="cl-area" d="${curve(a, b)}"/>
<path class="cl-band" d="${curve(lo, hi)}" fill="url(#cl-g-${c.key})"/>
<line class="cl-axis" x1="4" x2="${w - 4}" y1="${base}" y2="${base}"/>`;
  for (const t of ticks)
    out += `<line class="cl-tick" x1="${X(t)}" x2="${X(t)}" y1="${base}" y2="${base + 4}"/><text x="${X(t)}" y="${base + 17}" text-anchor="middle">${fmt(t)}</text>`;
  // The 80% interval, bracketed under the axis.
  out += `<path class="cl-brk" d="M${X(lo)} ${base + 26}v-4H${X(hi)}v4"/><text class="cl-brk-t" x="${(X(lo) + X(hi)) / 2}" y="${base + 34}" text-anchor="middle">80%</text>`;
  if (c.uu.eps) {
    const fh = 7;
    out += `<rect class="cl-floor" x="6" y="${base - fh}" width="${w - 12}" height="${fh}" fill="url(#cl-hatch)"/><rect x="7" y="${top - 15}" width="16" height="8" rx="1.5" fill="url(#cl-hatch)" stroke="#fbbf24" stroke-opacity=".5"/><text class="cl-floor-t" x="29" y="${top - 8}">${c.uu.kind === "catch" ? "catch-all floor" : "ε"} ${c.uu.eps.toFixed(2)}: outside the model</text>`;
  }
  if (c.mark) {
    const x = X(c.mark.at);
    const left = x > w * 0.6;
    out += `<line class="cl-mark" x1="${x}" x2="${x}" y1="${top - 14}" y2="${base}"/><text class="cl-mark-t" x="${x + (left ? -5 : 5)}" y="${top - 6}" text-anchor="${left ? "end" : "start"}">${esc(c.mark.label)}</text>`;
  }
  if (c.key === "freeboard")
    out += `<text class="cl-mark-t" x="8" y="${top - 6}">← 1.0 m minimum, 1.38 m below the scale</text>`;
  const ym = Y(mid);
  out += `<line class="cl-mid" x1="${X(mid)}" x2="${X(mid)}" y1="${ym}" y2="${base}"/><circle class="cl-pt" cx="${X(mid)}" cy="${ym}" r="4"/></svg>`;
  return out;
}

/** Sources, through transforms, to the claim; dots run along the edges. */
function provenance(c: ClaimData, w: number) {
  const S = c.prov.sources;
  const T = c.prov.steps;
  const bh = w > 340 ? 40 : 36;
  const gap = w > 340 ? 16 : 10;
  const H = Math.max(S.length * bh + (S.length - 1) * gap, T.length * 50);
  const sw = Math.round(w * 0.45);
  const tx = Math.round(w * 0.515);
  const tw = Math.round(w * 0.28);
  const cx = Math.round(w * 0.83);
  const cw = w - cx - 1;
  const th = 36;
  const ty = T.length === 1 ? [H / 2 - th / 2] : [H * 0.27 - th / 2, H * 0.73 - th / 2];
  const ch = 52;
  const cy = H / 2 - ch / 2;
  const id = (i: number) => `clp-${c.key}-${i}`;
  const paths: string[] = [];
  const t0 = ty[0]!;
  S.forEach((_, i) => {
    const y = i * (bh + gap) + bh / 2;
    const m = (sw + tx) / 2;
    paths.push(`M${sw} ${y}C${m} ${y} ${m} ${t0 + th / 2} ${tx} ${t0 + th / 2}`);
  });
  if (T.length > 1) paths.push(`M${tx + tw / 2} ${t0 + th}L${tx + tw / 2} ${ty[1]!}`);
  const tl = ty.at(-1)!;
  const m2 = (tx + tw + cx) / 2;
  paths.push(`M${tx + tw} ${tl + th / 2}C${m2} ${tl + th / 2} ${m2} ${cy + ch / 2} ${cx} ${cy + ch / 2}`);
  const text = (label: string, x: number, y: number, h: number, anchor = "start") => {
    const lines = label.split("\n");
    const y0 = y + h / 2 - (lines.length - 1) * 6.5 + 4;
    return lines
      .map((l, i) => `<text x="${x}" y="${y0 + i * 13}" text-anchor="${anchor}" class="${i ? "cl-pv-sub" : ""}">${esc(l)}</text>`)
      .join("");
  };
  let out = `<svg class="cl-prov" viewBox="0 0 ${w} ${H}" width="${w}" height="${H}" role="img" aria-label="Provenance: ${esc(S.map(([s]) => ln(s)).join(", "))}, through ${esc(T.map(ln).join(" then "))}, to the claim">`;
  paths.forEach((d, i) => (out += `<path id="${id(i)}" class="cl-edge" d="${d}"/>`));
  S.forEach(([label, ev], i) => {
    const y = i * (bh + gap);
    out += `<g class="cl-pn"><rect x="0.5" y="${y + 0.5}" width="${sw - 1}" height="${bh - 1}" rx="8"/><circle cx="11" cy="${y + bh / 2}" r="3.6" fill="${EVC[ev]}"/>${text(label, 21, y, bh)}</g>`;
  });
  T.forEach((label, i) => {
    out += `<g class="cl-pn step"><rect x="${tx + 0.5}" y="${ty[i]! + 0.5}" width="${tw - 1}" height="${th - 1}" rx="8"/>${text(label, tx + tw / 2, ty[i]!, th, "middle")}</g>`;
  });
  out += `<g class="cl-pn end"><rect x="${cx + 0.5}" y="${cy + 0.5}" width="${cw - 1}" height="${ch - 1}" rx="9"/><text x="${cx + cw / 2}" y="${cy + 19}" text-anchor="middle" class="cl-pv-sub">Claim</text><text x="${cx + cw / 2}" y="${cy + 37}" text-anchor="middle" class="cl-pv-v">${esc(c.value)}</text></g>`;
  if (!reduced)
    paths.forEach((_, i) => {
      const stage = i < S.length ? 0 : i - S.length + 1;
      for (let k = 0; k < 2; k++)
        out += `<circle r="2.3" class="cl-dot"><animateMotion dur="2.4s" begin="${(-(i * 0.37 + k * 1.2 + stage * 0.6)).toFixed(2)}s" repeatCount="indefinite"><mpath href="#${id(i)}"/></animateMotion></circle>`;
    });
  return `${out}</svg>`;
}

/** Outcomes of the method's past intervals: inside or outside, spread the way a record would show them. */
function strip(shown: number, hits: number, seed: number) {
  const out = Array.from({ length: shown }, () => true);
  let s = seed;
  for (let miss = shown - hits; miss > 0; ) {
    s = (s * 9301 + 49297) % 233280;
    const i = Math.floor((s / 233280) * shown);
    if (out[i]) {
      out[i] = false;
      miss--;
    }
  }
  return out;
}

function why(c: ClaimData, id: string) {
  const payload: Record<string, unknown> = {
    a: group({ schema: "fmine.claim.why.v1", claim: inline({ claim_id: shortId(id), more: MORE }) }),
    b: group({ evidence_class: EV[c.ev].code, knowledge_state: c.state, additivity: c.additivity }),
    representation: c.repr,
    color: { a: group({ status: "available", refusal: null }), value: inline(c.color.value) },
    uu_terms: [
      {
        a: group({ scenario_id: c.uu.scenario, description: c.uu.head }),
        b: group({ downside_exposure_usd_m: c.uu.exposure, exposure_is_finite: true, trigger_indicator: c.uu.trigger }),
      },
    ],
    uu_guard: inline({ status: "passed", refusal: null }),
    decision_admission: c.admission
      ? { a: group({ scope: "Claim color and UU guards only", claim_guards_pass: false }), first_refusal: { a: group({ code: c.admission.code, exit_code: c.admission.exit_code }), what: c.admission.what, fix: c.admission.fix }, b: group({ decision_context: null, flip_margin: null }) }
      : inline({ scope: "Claim color and UU guards only", claim_guards_pass: true, first_refusal: null, flip_margin: null }),
    c: group({ calibration: inline({ failed: false, assessment: null }), freshness: inline({ recorded_stale: false, assessment: "not_assessed" }) }),
  };
  const env = {
    a: group({ schema_version: "fmine.env.v1", verb: "claim why", status: "ok", exit_code: 0, anchor_id: null }),
    flags: FLAGS,
    b: group({ certificate: null, warnings: c.warnings ?? [] }),
    next_actions: [
      inline({ command: `fmine claim trace ${id}`, why: "the source anchors and available provenance" }),
      inline({ command: `fmine claim show ${id}`, why: "the canonical value, semantic key and source citation" }),
    ],
    payload,
    refusal: null,
  };
  // Each line is a block with its indent as a hanging indent, so a wrapped line stays under its key.
  const lines = json(env)
    .split("\n")
    .map((l) => {
      const body = l.replace(/^ +/, "");
      return `<span class="ln" style="--ind:${l.length - body.length}">${body}</span>`;
    })
    .join("");
  return `<span class="ln cmd"><span class="t-p">$</span> <span class="t-c">fmine claim why ${id}</span></span>${lines}`;
}

/** The Claim's own fields, each pointing at the part that opens it. */
function struct(c: ClaimData, flag: string): [field: string, value: string, part: string, n: number][] {
  const S = c.prov.sources.length;
  const T = c.prov.steps.length;
  const pass = c.guards.filter((g) => g.status === "pass").length;
  const fail = c.guards.filter((g) => g.status === "fail").length;
  return [
    ["repr", c.repr, "hero", 1],
    ["support", `${c.space} × ${c.time}, ${c.additivity}`, "sup", 2],
    ["evidence", EV[c.ev].code, "ev", 3],
    ["calibration", flag.replace(/\{.*/, "").trim() + (c.cal.cov80 !== undefined ? ` (cov80 ${c.cal.cov80})` : ""), "cal", 4],
    ["uu", { ref: `ReferenceClass { ε: ${c.uu.eps} }`, catch: `CatchAll { floor: ${c.uu.eps} }`, flip: "FlipMargin", ndr: "NotDecisionRelevant" }[c.uu.kind], "uu", 5],
    ["provenance", `${S} source${S > 1 ? "s" : ""} → ${T} transform${T > 1 ? "s" : ""}`, "prov", 6],
    ["state", `${c.state.toLowerCase()}; ${pass} guard${pass === 1 ? "" : "s"} passed${fail ? `, ${fail} refused` : ""}`, "guard", 7],
    ["confidentiality", "Internal", "hero", 1],
  ];
}

function anatomy(c: ClaimData, i: number) {
  const id = claimId(`synthetic:${c.key}`);
  const cal = c.cal;
  const flag =
    cal.kind === "Calibrated"
      ? `Calibrated { record: ${cal.record}, n: ${cal.n}, cov80: ${cal.cov80} }`
      : cal.kind === "Miscalibrated"
        ? `Miscalibrated { record: ${cal.record}, bias: ${cal.bias}, inflation: ${cal.inflation} }`
        : cal.kind === "NotCalibratable"
          ? "NotCalibratable { reason: failures too rare to score }"
          : "Uncalibrated";
  const outcomes = cal.shown && cal.hits !== undefined ? strip(cal.shown, cal.hits, 17 + i * 31) : [];
  const meter =
    cal.cov80 !== undefined
      ? `<div class="cl-cov" style="--v:${cal.cov80}"><span class="cl-cov-bar"><i></i><b title="target 0.80"></b></span><span class="num">cov80 ${cal.cov80.toFixed(2)} <small>target 0.80</small></span></div>`
      : "";
  return `<svg class="cl-lead" aria-hidden="true"></svg>
<section class="cl-part cl-hero" data-p="hero" aria-label="Value and interval">
  ${head(1, "Value and interval", `<code class="cl-id" title="${id}">${shortId(id)}</code>`)}
  <p class="cl-subj">${esc(c.subject)}</p>
  <div class="cl-val"><b class="num">${esc(c.value)}</b><span>${esc(c.unit)}</span>${c.delta ? `<em class="num">${esc(c.delta)} on week 42</em>` : ""}</div>
  <p class="cl-int">80% interval <b class="num">${esc(c.interval)}</b> ${esc(c.unit)}</p>
  <div class="cl-chart" data-chart></div>
  <div class="cl-struct" role="list" aria-label="The Claim’s fields, each opened in a numbered part">
    <p class="cl-sk">Claim {</p>
    ${struct(c, flag)
      .map(
        ([f, v, to, n]) =>
          `<p class="cl-row" role="listitem" data-to="${to}"><span class="f">${f}</span><span class="v">${esc(v)}</span><i class="cl-n">${n}</i></p>`,
      )
      .join("")}
    <p class="cl-sk">}</p>
  </div>
</section>
<section class="cl-part" data-p="sup" aria-label="Support">
  ${head(2, "Support: where and when it applies")}
  <div class="cl-sup">
    <p class="micro">Space</p><div class="cl-chips">${SPACE.map((s) => `<span${s === c.space ? ' class="on"' : ""}>${s}</span>`).join("")}</div>
    <p class="micro">Time</p><div class="cl-chips">${TIME.map((s) => `<span${s === c.time ? ' class="on"' : ""}>${s}</span>`).join("")}</div>
  </div>
  <dl class="cl-dl"><dt>Where</dt><dd>${esc(c.where)}</dd><dt>When</dt><dd>${esc(c.span)}</dd><dt>Valid for</dt><dd>${esc(c.validity)}</dd></dl>
  <p class="cl-add"><code>${c.additivity}</code> ${esc(c.addNote)}</p>
</section>
<section class="cl-part" data-p="ev" aria-label="Evidence class">
  ${head(3, "Evidence class: how it is known")}
  <ol class="cl-rungs" aria-label="Evidence classes, strongest first">${RUNGS.map(
    (r) =>
      `<li${r.includes(c.ev) ? ' class="on"' : ""}>${r
        .map((k) => `<span class="${k === c.ev ? "on" : ""}" style="--k:${EVC[k]}">${esc(EV[k].name)}</span>`)
        .join("")}</li>`,
  ).join("")}</ol>
  <p class="cl-txt">${esc(c.evWhy)} A computed number ranks no higher than its weakest input.</p>
  <p class="cl-auth"><span class="micro">Authority</span> ${esc(c.authority.replace(/\.$/, ""))}. Signing a number does not change its evidence class.</p>
</section>
<section class="cl-part" data-p="cal" aria-label="Calibration">
  ${head(4, "Calibration of the method")}
  <p class="cl-flag"><code>${esc(flag)}</code></p>
  ${outcomes.length ? `<div class="cl-strip" role="img" aria-label="${cal.hits} of ${cal.shown} past outcomes inside the 80% interval">${outcomes.map((x) => `<i class="${x ? "in" : "out"}"></i>`).join("")}</div><p class="cl-key"><i class="in"></i>inside its 80% interval <i class="out"></i>outside</p>` : `<div class="cl-strip none"><span>No scored outcomes</span></div>`}
  ${meter}
  <p class="cl-txt">${esc(cal.note)}</p>
</section>
<section class="cl-part" data-p="uu" aria-label="Unknown-unknown allowance">
  ${head(5, "Allowance for causes outside the model")}
  <div class="cl-forms">${UU_FORMS.map(([k, label]) => `<span class="${k === c.uu.kind ? "on" : ""}">${label}</span>`).join("")}</div>
  <p class="cl-uu"><b>${esc(c.uu.head)}</b> ${esc(c.uu.text)}</p>
  <p class="cl-small">A decision-facing number carries exactly one of these four, or it is not rendered (guard G-UU, FMINE-REF-204).</p>
</section>
<section class="cl-part" data-p="prov" aria-label="Provenance">
  ${head(6, "Provenance: sources, transforms, claim")}
  <div class="cl-provbox" data-prov></div>
  <p class="cl-key">${[...new Set(c.prov.sources.map(([, k]) => k))].map((k) => `<i style="background:${EVC[k]}"></i>${esc(EV[k].name)}`).join(" ")}</p>
  <p class="micro cl-anch-h">Source anchors</p>
  <ul class="cl-anchors">${(ANCHORS[c.key] ?? [])
    .map(([path, at]) => `<li><code>${escape(path)}</code><span>${esc(at)} · sha256 ${claimId(path).slice(4, 12)}…</span></li>`)
    .join("")}</ul>
  <p class="cl-small">Each edge keeps the code revision, parameters and seed that made it, so the number can be rebuilt. If a source is missing or altered, the claim is refused; no cached copy is used.</p>
</section>
<section class="cl-part" data-p="guard" aria-label="Guards">
  ${head(7, "Guards: checked before use")}
  <ul class="cl-guards">${c.guards
    .map(
      (g) =>
        `<li class="${g.status}"><b>${esc(g.id)}</b><span>${esc(g.note)}</span><em>${{ pass: "passed", fail: "refused", warn: "flag", na: "n/a" }[g.status]}</em></li>`,
    )
    .join("")}</ul>
</section>
<div class="cl-part term cl-term" data-p="term">
  <div class="term-bar"><i></i><i></i><i></i><span>fmine claim why · illustrative, indented and abridged</span></div>
  <pre class="cl-pre">${why(c, id)}</pre>
</div>`;
}

export default function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");
  host.classList.add("cl");
  host.innerHTML = `<div class="cl-top"><p class="micro">Synthetic mine · six numbers, as a spreadsheet would hold them</p><p class="cl-hint">Open one</p></div>
<div class="cl-tiles" role="group" aria-label="Numbers to open">${CLAIMS.map(
    (c, i) =>
      `<button type="button" class="cl-tile" data-k="${c.key}" aria-pressed="false"><span class="cl-ref num">${COLS[i]}7</span><span class="cl-tl">${esc(c.tile)} <small>${esc(c.when)}</small></span><span class="cl-tv"><b class="num">${esc(c.value)}</b> <span>${esc(c.unit)}</span></span></button>`,
  ).join("")}</div>
<div class="cl-anat${reduced ? "" : " pre"}" data-anat></div>`;
  const anat = $(host, "[data-anat]");
  const tiles = $$<HTMLButtonElement>(host, ".cl-tile");
  let current = CLAIMS[0]!;
  let shown = reduced;

  const charts = () => {
    const box = anat.querySelector<HTMLElement>("[data-chart]");
    if (box) box.innerHTML = chart(current, Math.max(240, Math.round(box.clientWidth)));
    const prov = anat.querySelector<HTMLElement>("[data-prov]");
    if (prov) prov.innerHTML = provenance(current, Math.max(250, Math.round(prov.clientWidth)));
  };

  /** Callout lines from each part to the value, where a part sits beside it. */
  const leaders = (animate: boolean) => {
    const svg = anat.querySelector<SVGSVGElement>(".cl-lead");
    const hero = anat.querySelector<HTMLElement>(".cl-hero");
    if (!svg || !hero) return;
    const A = { width: anat.clientWidth, height: anat.clientHeight };
    // Layout boxes, not screen boxes: the parts may be mid-flight in their entrance.
    const r = (el: HTMLElement) => ({ x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight });
    const H = r(hero);
    svg.setAttribute("viewBox", `0 0 ${A.width} ${A.height}`);
    let out = "";
    const clampY = (y: number) => Math.min(H.y + H.h - 18, Math.max(H.y + 18, y));
    for (const el of $$(anat, ".cl-part:not(.cl-hero):not(.cl-term)")) {
      const P = r(el);
      let seg: [number, number, number, number] | undefined;
      if (P.x + P.w < H.x + 2) seg = [P.x + P.w, P.y + 19, H.x, clampY(P.y + 19)];
      else if (P.x > H.x + H.w - 2) seg = [P.x, P.y + 19, H.x + H.w, clampY(P.y + 19)];
      else if (P.y > H.y + H.h - 2 && P.y - (H.y + H.h) < 40) seg = [P.x + 40, P.y, P.x + 40, H.y + H.h];
      if (!seg) continue;
      const [x1, y1, x2, y2] = seg;
      const len = Math.hypot(x2 - x1, y2 - y1);
      out += `<g data-p="${el.dataset.p}"><line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" style="--len:${len.toFixed(1)}"/><circle cx="${x1}" cy="${y1}" r="3"/><circle cx="${x2}" cy="${y2}" r="3"/></g>`;
    }
    svg.innerHTML = out;
    svg.classList.toggle("draw", animate);
  };

  const explode = () => {
    const hero = anat.querySelector<HTMLElement>(".cl-hero");
    if (!hero) return;
    const hb = hero.getBoundingClientRect();
    const hx = hb.left + hb.width / 2;
    const hy = hb.top + hb.height / 2;
    hero.animate([{ transform: "scale(0.965)", opacity: 0 }, { transform: "none", opacity: 1 }], {
      duration: 600,
      easing: "cubic-bezier(.19,1,.22,1)",
      fill: "backwards",
    });
    $$(anat, ".cl-part:not(.cl-hero)").forEach((el, k) => {
      const b = el.getBoundingClientRect();
      const dx = (hx - (b.left + b.width / 2)) * 0.32;
      const dy = (hy - (b.top + b.height / 2)) * 0.32;
      el.animate([{ transform: `translate(${dx}px, ${dy}px) scale(0.93)`, opacity: 0 }, { transform: "none", opacity: 1 }], {
        duration: 850,
        delay: 90 + k * 60,
        easing: "cubic-bezier(.19,1,.22,1)",
        fill: "backwards",
      });
    });
  };

  const open = (key: string, animate: boolean) => {
    const i = CLAIMS.findIndex((c) => c.key === key);
    current = CLAIMS[i] ?? CLAIMS[0]!;
    for (const t of tiles) t.setAttribute("aria-pressed", String(t.dataset.k === current.key));
    anat.innerHTML = anatomy(current, Math.max(0, i));
    for (const row of $$(anat, ".cl-row")) {
      const to = row.dataset.to!;
      const mark = (on: boolean) => {
        for (const el of $$(anat, `[data-p="${to}"]`)) el.classList.toggle("hl", on);
        row.classList.toggle("hl", on);
      };
      row.addEventListener("pointerenter", () => mark(true));
      row.addEventListener("pointerleave", () => mark(false));
      row.addEventListener("click", () => {
        const part = anat.querySelector<HTMLElement>(`.cl-part[data-p="${to}"]`);
        if (part && to !== "hero") part.scrollIntoView({ block: "nearest", behavior: reduced ? "auto" : "smooth" });
        mark(true);
        setTimeout(() => mark(false), 1400);
      });
    }
    charts();
    leaders(animate && !reduced);
    if (animate && !reduced) explode();
  };

  for (const t of tiles) t.addEventListener("click", () => open(t.dataset.k!, shown));
  open(current.key, false);

  let width = 0;
  new ResizeObserver(() => {
    const w = Math.round(anat.clientWidth);
    if (w !== width) {
      width = w;
      charts();
    }
    leaders(false);
  }).observe(anat);

  // The hero is short enough to be seen whole; the figure, on a phone, is not.
  if (!reduced)
    seen($(anat, ".cl-hero"), () => {
      shown = true;
      anat.classList.remove("pre");
      leaders(true);
      explode();
    });
}
