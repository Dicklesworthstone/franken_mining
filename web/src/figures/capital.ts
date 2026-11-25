/**
 * Choose the portfolio, bound the downside: the README's invented GB-30 capital program. Three $400M packages share
 * five equally likely scenarios under an $800M budget. Each affordable programme's tail loss (the average loss in the
 * worst 1 in 5 scenarios, CVaR at 80%) and mean come from FrankenMining's risk kernel (fmine-numerics compute_cvar) in
 * WebAssembly, on the programme's payoffs summed within each shared scenario. The board's limit picks the best mean
 * whose tail stays inside it: A+E unconstrained, A+D at $150M, at a cost of $130.82M of expected value.
 */
import { cvar } from "./engine";
import { $, fmt, onWidth, range } from "./lib";
import { range as rangeHtml } from "../parts";

const SCENARIOS = ["Low price and overrun", "Low price", "Base", "High price", "Country shock"];
const SHORT = ["low, overrun", "low", "base", "high", "shock"];
const BUDGET = 800;
interface Project {
  id: string;
  name: string;
  cost: number;
  colour: string;
  payoff: number[];
}
/** tests/capital/gb30.toml, in $M: net delta-NAV with capital already included. */
const PROJECTS: Project[] = [
  { id: "A", name: "Brownfield expansion", cost: 400, colour: "#6cc8f7", payoff: [-60, 40, 220, 480, 220] },
  { id: "E", name: "Greenfield mine", cost: 400, colour: "#b7a2fb", payoff: [-150, 60, 420, 960, -330] },
  { id: "D", name: "Share buyback at 0.85 × NAV", cost: 400, colour: "#ffd166", payoff: [-70.6, 0, 70.6, 258.8, 47.1] },
];
const PINK = "#f78fb3";
const CORAL = "#fb7185";
const GREEN = "#5fd38d";

interface Program {
  ids: string[];
  label: string;
  cost: number;
  payoff: number[];
  mean: number;
  tail: number;
  affordable: boolean;
}

/** $M with up to two decimals, as the README prints them: $241.18M, $130.6M, $372M. */
const money = (v: number, sign = false) => {
  const r = Math.round(v * 100) / 100;
  const abs = Math.abs(r);
  const s = abs === 0 ? "$0" : `$${fmt(abs, Number.isInteger(abs) ? 0 : Number.isInteger(abs * 10) ? 1 : 2)}M`;
  return r < 0 ? `−${s}` : sign && r > 0 ? `+${s}` : s;
};

export default async function mount(fig: HTMLElement) {
  const host = $(fig, "[data-host]");

  // Every subset of the three packages, the empty one (hold cash) included.
  const programs: Program[] = [];
  for (let mask = 0; mask < 8; mask++) {
    const parts = PROJECTS.filter((_, i) => mask & (1 << i));
    const payoff = SCENARIOS.map((_, s) => parts.reduce((t, p) => t + p.payoff[s]!, 0));
    const cost = parts.reduce((t, p) => t + p.cost, 0);
    // Losses are negated payoffs; the tail is the worst 1 in 5, so alpha = 4/5.
    const r = await cvar(
      payoff.map((v) => -v),
      0.8,
    );
    programs.push({
      ids: parts.map((p) => p.id),
      label: parts.length ? parts.map((p) => p.id).join("+") : "Hold cash",
      cost,
      payoff,
      mean: -r.meanLoss,
      tail: r.cvar,
      affordable: cost <= BUDGET,
    });
  }
  const affordable = programs.filter((p) => p.affordable);
  const best = (limit: number) =>
    affordable.filter((p) => p.tail <= limit + 1e-9).reduce((a, b) => (b.mean > a.mean ? b : a));
  const free = affordable.reduce((a, b) => (b.mean > a.mean ? b : a));
  const lo = Math.min(...PROJECTS.flatMap((p) => p.payoff));
  const hi = Math.max(...PROJECTS.flatMap((p) => p.payoff));

  host.innerHTML = `<div class="cap-projects">${PROJECTS.map(
    (p) => `<article class="cap-proj" data-p="${p.id}" style="--pc:${p.colour}">
<header><span class="cap-id">${p.id}</span><div><h4>${p.name}</h4><p class="micro asis">${money(p.cost)} · invented</p></div><span class="cap-in">chosen</span></header>
<svg class="chart cap-mini" role="img" aria-label="${p.name}: net value in each scenario"></svg>
<p class="cap-stat"><span>mean <b>${money(programs[1 << PROJECTS.indexOf(p)]!.mean)}</b></span><span>tail loss <b>${money(programs[1 << PROJECTS.indexOf(p)]!.tail)}</b></span></p>
</article>`,
  ).join("")}</div>
<div class="cap-main">
<div class="cap-chart"><p class="micro asis">Every programme the $800M budget allows, by expected value and tail loss</p><svg class="chart" role="img" aria-label="Expected value against tail loss for each programme"></svg>
<div class="cap-controls">${rangeHtml("limit", "The board’s tail-loss limit (worst 1 in 5)", 0, 350, 5, 150, "$150M")}</div></div>
<div class="cap-side">
<div class="cap-verdict"><p class="micro">The board’s programme</p><p class="cap-pick" data-k="pick">–</p><p class="cap-why" data-k="why"></p>
<div class="cap-budget" data-k="budget"></div></div>
<div class="cap-tailchart"><p class="micro asis">The chosen programme in each scenario, beside the best without a limit</p><svg class="chart" role="img" aria-label="The chosen programme’s value in each scenario"></svg></div>
</div>
</div>
<p class="cap-foot">Invented GB-30 exercise: three $400M packages, five shared and equally likely scenarios, an $800M budget. Payoffs are net change in NAV with capital already included, so spending limits the budget and is not deducted again. <span data-k="engine">Each programme’s mean and tail loss (CVaR at 80%: the average loss in the worst 1 in 5 scenarios) are computed by FrankenMining’s risk kernel, fmine-numerics <code>compute_cvar</code>, running in this page on payoffs summed within each scenario, so losses in the same scenario add up.</span></p>`;

  const k = (name: string) => $(host, `[data-k="${name}"]`);
  const chart = $<SVGSVGElement>(host, ".cap-chart svg");
  const tailSvg = $<SVGSVGElement>(host, ".cap-tailchart svg");
  let limit = 150;
  let chartW = 700;
  let sideW = 420;
  let started = false;

  const mini = (p: Project, w: number) => {
    const H = w < 330 ? 96 : 104;
    const T = 14;
    const B = 34;
    const Y = (v: number) => T + ((hi - v) / (hi - lo)) * (H - T - B);
    const cw = w / 5;
    let s = `<path class="axis" d="M0 ${Y(0)}H${w}"/>`;
    for (const [i, v] of p.payoff.entries()) {
      const x = i * cw + cw * 0.2;
      const y0 = Y(Math.max(0, v));
      const y1 = Y(Math.min(0, v));
      s += `<rect x="${x.toFixed(1)}" y="${y0.toFixed(1)}" width="${(cw * 0.6).toFixed(1)}" height="${Math.max(1, y1 - y0).toFixed(1)}" rx="2" fill="${v < 0 ? CORAL : p.colour}" fill-opacity="${v < 0 ? 0.85 : 0.75}"><title>${SCENARIOS[i]}: ${money(v)}</title></rect>`;
      s += `<text x="${(i * cw + cw / 2).toFixed(1)}" y="${v < 0 ? y1 + 11 : y0 - 3}" text-anchor="middle" class="cm-v">${fmt(v, Number.isInteger(v) ? 0 : 1)}</text>`;
      s += `<text x="${(i * cw + cw / 2).toFixed(1)}" y="${H - 4}" text-anchor="middle" class="cm-s">${SHORT[i]}</text>`;
    }
    return { s, H };
  };

  const drawMinis = () => {
    for (const p of PROJECTS) {
      const svg = $<SVGSVGElement>(host, `.cap-proj[data-p="${p.id}"] svg`);
      const w = Math.max(200, svg.clientWidth || 300);
      const { s, H } = mini(p, w);
      svg.setAttribute("viewBox", `0 0 ${w} ${H}`);
      svg.innerHTML = s;
    }
  };

  const drawChart = () => {
    const W = chartW;
    const narrow = W < 520;
    const H = narrow ? 330 : 380;
    const L = narrow ? 44 : 54;
    const R = 14;
    const T = 16;
    const B = 44;
    const XM = 350;
    const YM = 460;
    const X = (v: number) => L + (v / XM) * (W - L - R);
    const Y = (v: number) => T + (1 - v / YM) * (H - T - B);
    const pick = best(limit);
    let s = "";
    for (let v = 0; v <= 400; v += 100) s += `<path class="grid" d="M${L} ${Y(v)}H${W - R}"/><text x="${L - 7}" y="${Y(v) + 4}" text-anchor="end">${v}</text>`;
    for (let v = 0; v <= XM; v += narrow ? 100 : 50) s += `<path class="grid" d="M${X(v)} ${T}V${Y(0)}"/><text x="${X(v)}" y="${Y(0) + 16}" text-anchor="middle">${v}</text>`;
    // Over the limit: shaded.
    s += `<rect x="${X(limit)}" y="${T}" width="${Math.max(0, W - R - X(limit))}" height="${Y(0) - T}" fill="${CORAL}" fill-opacity=".07"/>`;
    s += `<path class="axis" d="M${L} ${T}V${Y(0)}H${W - R}"/>`;
    s += `<text x="${W - R}" y="${H - 6}" text-anchor="end">tail loss, $M · worse →</text><text x="${L - 7}" y="${T - 4}" text-anchor="end">$M</text>`;
    // The frontier: the best mean available at each limit, a staircase through the programs that win somewhere.
    const steps: [number, number][] = [];
    for (let v = 0; v <= XM; v += 0.5) steps.push([v, best(v).mean]);
    let d = "";
    for (const [i, [v, m]] of steps.entries()) {
      const prev = steps[i - 1];
      if (!prev) d += `M${X(v).toFixed(1)} ${Y(m).toFixed(1)}`;
      else if (prev[1] !== m) d += `H${X(v).toFixed(1)}V${Y(m).toFixed(1)}`;
    }
    d += `H${X(XM)}`;
    s += `<path d="${d}" fill="none" stroke="${PINK}" stroke-opacity=".5" stroke-width="1.6" stroke-dasharray="5 4"/>`;
    // The limit.
    s += `<path d="M${X(limit)} ${T}V${Y(0)}" stroke="${CORAL}" stroke-width="2"/><text x="${X(limit) + (limit > 250 ? -6 : 6)}" y="${Y(0) - 8}" text-anchor="${limit > 250 ? "end" : "start"}" class="cc-lim">limit ${money(limit)}</text>`;
    // The expected value given up.
    if (pick !== free) {
      s += `<path d="M${X(pick.tail)} ${Y(free.mean)}H${X(free.tail)}" stroke="#fff" stroke-opacity=".2" stroke-dasharray="2 3"/>`;
      s += `<path d="M${X(pick.tail) - 10} ${Y(free.mean)}V${Y(pick.mean)}" stroke="${PINK}" stroke-width="1.6" marker-end="url(#cc-arrow)"/>`;
      s += `<text x="${X(pick.tail) - 16}" y="${(Y(free.mean) + Y(pick.mean)) / 2 + 4}" text-anchor="end" class="cc-gap">−${money(free.mean - pick.mean)}</text>`;
    }
    s = `<defs><marker id="cc-arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0 10 5 0 10Z" fill="${PINK}"/></marker></defs>` + s;
    // Each point's words go on the side with room, else the other side, else the nearest height clear of the words
    // already placed: two programmes close in mean would otherwise print over each other. The chosen one goes first.
    const words = (p: (typeof programs)[number]) => ({
      lab: p.affordable ? p.label : narrow ? `${p.label}: over budget` : `${p.label}, over the $800M budget`,
      val: narrow ? "" : `${money(p.mean)} · ${money(p.tail)}`,
    });
    const taken: number[][] = [];
    const place = new Map<(typeof programs)[number], { right: boolean; dy: number }>();
    for (const p of [pick, ...programs.filter((q) => q !== pick)]) {
      const x = X(p.tail);
      const y = Y(p.mean);
      const { lab, val } = words(p);
      const off = p === pick ? 18 : 12;
      // 12.5px Inter is about 6.6px a character, 10.5px mono 6.3px.
      const wide = Math.max(lab.length * 6.6, val.length * 6.3);
      const box = (right: boolean, dy: number) => {
        const l = right ? x + off : x - off - wide;
        return [l, y - 18 + dy, l + wide, y + (val ? 11 : -3) + dy];
      };
      const clear = (b: number[]) => !taken.some((o) => b[0]! < o[2]! && b[2]! > o[0]! && b[1]! < o[3]! && b[3]! > o[1]!);
      const prefer = x + 14 + Math.max(lab.length * 6.6, 104) < W;
      const room = (right: boolean) => (right ? x + off + wide < W : x - off - wide > L);
      let at = { right: prefer, dy: 0 };
      if (!clear(box(prefer, 0))) {
        if (room(!prefer) && clear(box(!prefer, 0))) at = { right: !prefer, dy: 0 };
        else
          for (let k = 2; k <= 60; k += 2) {
            const dy = [k, -k].find((d) => clear(box(prefer, d)));
            if (dy !== undefined) {
              at = { right: prefer, dy };
              break;
            }
          }
      }
      place.set(p, at);
      taken.push(box(at.right, at.dy));
    }
    for (const p of programs) {
      const x = X(p.tail);
      const y = Y(p.mean);
      const over = p.tail > limit + 1e-9;
      const chosen = p === pick;
      let cls = "pt";
      if (!p.affordable) cls += " pt-broke";
      else if (over) cls += " pt-over";
      if (chosen) cls += " on";
      if (chosen) s += `<circle cx="${x}" cy="${y}" r="15" class="halo"/>`;
      s += `<g class="${cls}"><circle cx="${x}" cy="${y}" r="${chosen ? 8 : 6}"/>`;
      // Small pie-like stripes: one dot per package in the programme.
      const dots = p.ids.map((id, i) => {
        const pr = PROJECTS.find((q) => q.id === id)!;
        return `<circle cx="${x - (p.ids.length - 1) * 4 + i * 8}" cy="${y + 15}" r="2.6" fill="${pr.colour}"/>`;
      });
      s += dots.join("");
      const { lab, val } = words(p);
      const { right, dy } = place.get(p)!;
      const off = chosen ? 18 : 12;
      s += `<text x="${right ? x + off : x - off}" y="${y - 7 + dy}" text-anchor="${right ? "start" : "end"}" class="pl">${lab}</text>`;
      if (val) s += `<text x="${right ? x + off : x - off}" y="${y + 7 + dy}" text-anchor="${right ? "start" : "end"}" class="pv">${val}</text>`;
      s += `</g>`;
    }
    chart.setAttribute("viewBox", `0 0 ${W} ${H}`);
    chart.innerHTML = s;
  };

  const drawTail = () => {
    const pick = best(limit);
    const W = sideW;
    const H = 168;
    const T = 16;
    const B = 34;
    const vals = [...pick.payoff, ...free.payoff];
    const top = Math.max(400, ...vals);
    const bot = Math.min(-250, ...vals);
    const Y = (v: number) => T + ((top - v) / (top - bot)) * (H - T - B);
    const cw = W / 5;
    const worst = pick.payoff.indexOf(Math.min(...pick.payoff));
    let s = `<path class="axis" d="M0 ${Y(0)}H${W}"/>`;
    for (let i = 0; i < 5; i++) {
      const v = pick.payoff[i]!;
      const f = free.payoff[i]!;
      const x = i * cw + cw * 0.18;
      const bw = cw * 0.64;
      if (pick !== free) {
        const y0 = Y(Math.max(0, f));
        const y1 = Y(Math.min(0, f));
        s += `<rect x="${x.toFixed(1)}" y="${y0.toFixed(1)}" width="${bw.toFixed(1)}" height="${Math.max(1, y1 - y0).toFixed(1)}" rx="2" fill="none" stroke="#fff" stroke-opacity=".3" stroke-dasharray="3 3"/>`;
      }
      const y0 = Y(Math.max(0, v));
      const y1 = Y(Math.min(0, v));
      const tail = i === worst && v < 0;
      s += `<rect x="${(x + bw * 0.14).toFixed(1)}" y="${y0.toFixed(1)}" width="${(bw * 0.72).toFixed(1)}" height="${Math.max(1, y1 - y0).toFixed(1)}" rx="2" fill="${v < 0 ? CORAL : PINK}" fill-opacity="${tail ? 1 : 0.7}"/>`;
      s += `<text x="${(i * cw + cw / 2).toFixed(1)}" y="${v < 0 ? y1 + 12 : y0 - 4}" text-anchor="middle" class="ct-v">${fmt(v, Number.isInteger(v) ? 0 : 1)}</text>`;
      s += `<text x="${(i * cw + cw / 2).toFixed(1)}" y="${H - 16}" text-anchor="middle" class="ct-s">${SHORT[i]}</text>`;
      if (tail) s += `<text x="${(i * cw + cw / 2).toFixed(1)}" y="${H - 3}" text-anchor="middle" class="ct-t">the tail</text>`;
    }
    tailSvg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    tailSvg.innerHTML = s;
  };

  const words = () => {
    const pick = best(limit);
    k("pick").textContent = pick.ids.length ? pick.ids.join(" + ") : "Hold cash";
    const names = pick.ids.map((id, i) => {
      const n = PROJECTS.find((p) => p.id === id)!.name;
      return i ? n[0]!.toLowerCase() + n.slice(1) : n;
    });
    const gave = free.mean - pick.mean;
    // The next limit at which a better programme comes in.
    const next = affordable.filter((p) => p.mean > pick.mean).sort((a, b) => a.tail - b.tail)[0];
    k("why").innerHTML =
      (pick.ids.length ? `${names.join(" and ")}. ` : "Nothing fits under this limit except cash. ") +
      `Expected <b>${money(pick.mean)}</b>, tail loss <b>${money(pick.tail)}</b> against a limit of ${money(limit)}.` +
      (gave > 0.005
        ? ` The limit gives up <b class="cost">${money(gave)}</b> of expected value against ${free.label} (${money(free.mean)}, tail loss ${money(free.tail)}).`
        : " The limit costs nothing here: the best programme already fits.") +
      (next ? ` Raising the limit to ${money(next.tail)} admits ${next.label}, ${money(next.mean - pick.mean, true)}.` : "");
    k("budget").innerHTML =
      `<span class="micro asis">$800M budget</span><div class="cap-slots">${[0, 1]
        .map((i) => {
          const id = pick.ids[i];
          const p = id ? PROJECTS.find((q) => q.id === id) : undefined;
          return p ? `<i style="--pc:${p.colour}"><b>${p.id}</b> ${money(p.cost)}</i>` : `<i class="cash">cash ${money(400)}</i>`;
        })
        .join("")}</div>`;
    for (const p of PROJECTS) $(host, `.cap-proj[data-p="${p.id}"]`).classList.toggle("sel", pick.ids.includes(p.id));
  };

  const draw = () => {
    words();
    drawChart();
    drawTail();
  };

  range(host, "limit", (v) => money(v), (v) => {
    limit = v;
    draw();
  });
  onWidth($(host, ".cap-chart"), (w) => {
    chartW = w;
    if (started) drawChart();
  });
  onWidth($(host, ".cap-tailchart"), (w) => {
    sideW = w;
    if (started) drawTail();
  });
  onWidth($(host, ".cap-projects"), () => drawMinis());
  started = true;
  draw();
}
