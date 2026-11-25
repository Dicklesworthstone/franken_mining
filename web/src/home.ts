import { chapters, renderChapters } from "./chapters";
import { repo } from "./layout";
import { icon, range, seg, title, type Refs } from "./parts";

export { chapters };

export const homeNav = chapters
  .map(
    (c) =>
      `<a href="#${c.id}" data-spy="${c.id}" style="--chapter:${c.colour}"><span class="joint" aria-hidden="true"></span>${c.short}</a>`,
  )
  .join("");

/** Facts the README records, each opening the part of the page that shows it. */
const stats: [n: string, label: string, note: string, to: string][] = [
  ["11 of 11", "benchmark pits exact", "The certified pit equals MineLib’s published optimum on every instance.", "#pit"],
  ["1.2 s", "for 2.1 million blocks", "MineLib’s largest open pit, solved and certified on one machine.", "#pit"],
  ["22,732", "published figures as claims", "A producer’s quarterly operating statistics, each with its source cell.", "#public"],
  ["2,915", "AISC identities checked", "Recomputed and compared across 20 registered quarterly workbooks.", "#public"],
  ["0", "third-party crates", "Every line of Rust is in the repository, so all of it can be audited.", "#stack"],
];

const hero = () => `
<section class="hero mine-hero" aria-labelledby="hero-title" id="fig-hero" data-fig="hero">
<div class="lineup"><picture><source media="(max-width: 999px)" srcset="/media/lineup-tall.webp" width="1080" height="1180"><img src="/media/lineup.webp" width="1920" height="1056" alt="" fetchpriority="high"></picture><canvas aria-label="A live 3D open-pit mine at golden hour: haul trucks climb the ramp from a shovel at the pit floor to the crusher, a drill works a bench, and labels over each machine show the claims FrankenMining keeps."></canvas><div class="pins" data-pins aria-hidden="true"></div></div>
<div class="wrap hero-copy"><p class="pill"><span class="joint on" aria-hidden="true"></span>System of record for mining<i></i>Designed in the open</p>
<h1 class="d1" id="hero-title"><span>Every number</span> <span>in the mine,</span> <span><em class="g">with its evidence</em>.</span></h1>
<p class="lede">FrankenMining is a system of record for a mining company’s numbers: drillhole assays, truckloads, plant recoveries, cost per ounce. Each one carries its evidence, its uncertainty and its track record, so the people and AI agents who act on it can see what it rests on.</p>
<div class="cta"><a class="btn go" href="#idea">See how it works${icon("down")}</a><a class="btn" href="${repo}">${icon("star")}Star on GitHub</a></div></div>
<div class="wrap console-wrap"><div class="console"><div class="console-place"><p class="micro">Read the mine as</p>${seg(
  "lens",
  "Whose view",
  [
    ["operator", "Supervisor"],
    ["geology", "Geologist"],
    ["plant", "Metallurgist"],
    ["cfo", "CFO"],
    ["agent", "AI agent"],
  ],
  "operator",
)}</div><div class="console-dials">${range("clock", "Shift clock", 5, 21, 0.25, 17.25, "17:15")}</div><div class="console-out"><p class="tally"><b data-n="loads">112</b> loads in custody</p><div class="tallybar" aria-hidden="true"><i data-bar="ore" style="flex-grow:0.5869809203142536"></i><i data-bar="waste" style="flex-grow:0.41301907968574636"></i><i data-bar="open" style="flex-grow:0.06"></i></div><p class="tallykeys"><span class="ore"><b data-n="ore">26,150</b> t ore</span><span class="waste"><b data-n="waste">18,400</b> t waste</span><span class="open"><b data-n="open">3</b> open questions</span></p></div><button type="button" class="console-pause" data-pause aria-label="Pause" aria-pressed="false"><span class="on">${icon("pause")}</span><span class="off">${icon("play")}</span></button></div>
<p class="console-note">Pick a desk and the labels change: the same mine, read as a supervisor, a geologist, a metallurgist, a CFO or an agent. The drillholes, the block model and the pit limit under the floor are worked out in your browser by FrankenMining’s own engine. The mine is synthetic.</p></div></section>
<div class="wrap"><ul class="stats">${stats
  .map(
    ([n, label, note, to]) =>
      `<li><a href="${to}"><b>${n}</b><strong>${label}</strong><span>${note}</span></a></li>`,
  )
  .join("")}</ul></div>`;

const running: [string, string][] = [
  ["Certified ultimate pit", "Exact on all eleven MineLib benchmark pits; 2.1 million blocks in about 1.2 s on one machine."],
  ["Public assurance", "20 registered quarterly workbooks, 2,915 AISC identities checked, every unexplained restatement listed."],
  ["Reconciliation", "Dry tonnes and contained metal reconciled across boundaries on supplied worksheets."],
  ["Forecast journal", "Register, freeze, resolve and score, with track records and calibration monitors."],
  ["Decision engine", "Exact value of information, policy risk and downside for a DecisionCard."],
  ["Plant tools", "Mass balance, raw-measurement reconciliation, flowsheets, grinding and residence time."],
];
const designed: [string, string][] = [
  ["Strata", "Block models as claims, with a kriging engine held to a reference."],
  ["Headframe", "The live operations twin, read from fleet and historian systems."],
  ["Freeboard", "Tailings and water evidence for the engineer of record."],
  ["Bullion", "The reconciling book in FrankenAccounting, beside the ERP."],
  ["Adit", "The agent surface, including an MCP server, fmine-mcp."],
];

const roles: [name: string, pages: [string, string][], text: string][] = [
  ["You run a mine", [["Headframe", "16"], ["Crucible", "17"], ["Touchstone", "20"], ["Muster", "19"]], "Operations, the plant, reconciliation and crews."],
  ["You plan and model", [["Lodestar", "11"], ["Strata", "13"], ["Highwall", "14"], ["Pushback", "15"]], "Exploration, the ground, slopes and the pit."],
  ["You own the numbers", [["Bullion", "21"], ["Helm I", "22"], ["Helm II", "23"], ["Hallmark", "24"]], "Accounting, decisions, capital and disclosure."],
  ["You build agents", [["Adit", "30"], ["The Claim", "8"], ["Forecasts", "10"], ["Security", "31"]], "Commands, schemas, refusals and the lines agents cannot cross."],
];

const close = (ref: Refs, sections: number, words: string) => `
<section class="libcta" aria-labelledby="lib-title"><div class="lattice" aria-hidden="true"></div><div class="wrap"><div class="reveal"><p class="kick"><span class="joint on" aria-hidden="true"></span>Designed in the open</p><h2 class="d2" id="lib-title">${title("Follow *the build*.")}</h2><p class="lede">FrankenMining is designed and built in public. The whole plan is here to read and search: ${sections} sections and appendices, about ${words} words, with every equation and every gate.</p><div class="cta"><a class="btn go" href="${repo}">${icon("star")}Star on GitHub</a><a class="btn" href="/plan/">Read the plan${icon("right")}</a><a class="btn" href="/search/" data-palette>${icon("search")}Search it</a></div></div>
<div class="status reveal"><div><p class="micro"><span class="dot run" aria-hidden="true"></span>Running today</p><ul>${running.map(([h, p]) => `<li><b>${h}</b><span>${p}</span></li>`).join("")}</ul></div><div><p class="micro"><span class="dot design" aria-hidden="true"></span>In design</p><ul>${designed.map(([h, p]) => `<li><b>${h}</b><span>${p}</span></li>`).join("")}</ul></div></div>
<div class="paths">${roles
  .map(
    ([name, pages, text]) =>
      `<div class="path reveal"><h3>${name}</h3><p>${text}</p><p class="steps">${pages.map(([l, s]) => `<a class="tag" href="${ref(s)}">${l}</a>`).join("")}</p></div>`,
  )
  .join("")}</div></div></section>`;

export function home(o: { ref: Refs; sections: number; words: string }) {
  return hero() + renderChapters(o.ref) + close(o.ref, o.sections, o.words);
}
