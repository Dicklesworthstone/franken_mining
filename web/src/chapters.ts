import { chapterClose, chapterOpen, figure, icon, section, type Chapter, type Kind, type Refs } from "./parts";
import { escape } from "./html";

/* The home page's chapters, in the order a mine's life runs: the idea, the ground, the plan, the operation, the plant,
 * the reconciliation, the decision, and the agents that work across all of them. Every example is synthetic or
 * illustrative, and every capability is described as the plan designs it unless the README records it as running. */

export const idea: Chapter = {
  id: "idea",
  number: "01",
  label: "The idea",
  short: "Idea",
  icon: "claim",
  colour: "#f2a33c",
  title: "Every number *is a Claim*.",
  thesis:
    "A mine runs on numbers that pass through many hands: a grade at the face, tonnes on a truck, metal in the concentrate, a cost per ounce for the board. FrankenMining keeps each one as a Claim with its evidence, its uncertainty and its track record attached, so the operator, the CFO and an AI agent all read the same thing.",
  sections: [
    ["claim", "Each number carries its sources"],
    ["ladder", "One ledger, read at every desk"],
  ],
};

export const explore: Chapter = {
  id: "explore",
  number: "02",
  label: "Exploration and the ground",
  short: "Explore",
  icon: "drill",
  colour: "#b794f6",
  title: "The ground *between the holes*.",
  thesis:
    "A drillhole samples a core a few centimetres across; everything between the holes is an estimate. FrankenMining keeps what each hole found, what it ruled out and how uncertain the model is between holes, so the next metre of drilling goes where it is worth the most.",
  sections: [
    ["drill", "Drill a few holes, estimate the rest"],
    ["absence", "A dry hole is evidence too"],
  ],
};

export const plan: Chapter = {
  id: "plan",
  number: "03",
  label: "Mine planning",
  short: "Plan",
  icon: "pit",
  colour: "#60c3f0",
  title: "The pit, *proven optimal*.",
  thesis:
    "An ultimate pit is the most valuable shape you can dig without breaking the slope rules. FrankenMining solves it exactly and hands back a certificate anyone can check, then shows how the shape moves with price, slope and the blast that breaks the rock.",
  sections: [
    ["pit", "Solved exactly, checked independently"],
    ["blast", "Blast with the ore boundary in view"],
  ],
};

export const operate: Chapter = {
  id: "operate",
  number: "04",
  label: "Operations",
  short: "Operate",
  icon: "truck",
  colour: "#f5c13d",
  title: "The mine, *as it is right now*.",
  thesis:
    "Trucks, shovels and loaders report position, payload and status every few seconds. FrankenMining turns those reports into claims with custody: which tonnes came from which face, at what grade, through which crusher, and how sure anyone can be.",
  sections: [
    ["haul", "Every truckload in custody"],
    ["underground", "Underground, in the same ledger"],
  ],
};

export const processing: Chapter = {
  id: "process",
  number: "05",
  label: "Processing and tailings",
  short: "Process",
  icon: "plant",
  colour: "#34d3b5",
  title: "The plant, *stream by stream*.",
  thesis:
    "A plant blends ore from many faces and returns metal, concentrate and tailings. FrankenMining balances every stream, carries hardness and recovery uncertainty through the circuit, and keeps the tailings dam’s evidence where the engineer can see it.",
  sections: [
    ["plant", "Every stream balanced"],
    ["grind", "Average hardness understates throughput"],
    ["tailings", "The dam, with the age of every reading"],
  ],
};

export const reconcile: Chapter = {
  id: "reconcile",
  number: "06",
  label: "Reconciliation and accounting",
  short: "Reconcile",
  icon: "scale",
  colour: "#ef7f4c",
  title: "Where *the metal went*.",
  thesis:
    "The model, the mine, the mill and the refinery each count metal their own way. FrankenMining reconciles them at every boundary, in tonnes, grade and metal, and carries the result into a reconciling book that sits beside your ERP.",
  sections: [
    ["recon", "On-plan metal can hide a grade miss"],
    ["ledger", "Books beside the ERP"],
    ["public", "Checked against what was published"],
  ],
};

export const decide: Chapter = {
  id: "decide",
  number: "07",
  label: "Decisions and capital",
  short: "Decide",
  icon: "decide",
  colour: "#f78fb3",
  title: "Decisions, *with the working shown*.",
  thesis:
    "Capital decisions in mining are bets on uncertain ground, prices and plants. FrankenMining frames each one as a DecisionCard: the options, the evidence, what more information is worth, and, later, how the call scored.",
  sections: [
    ["card", "Is the testwork worth it?"],
    ["capital", "Choose the portfolio, bound the downside"],
    ["journal", "Forecasts, frozen then scored"],
  ],
};

export const agents: Chapter = {
  id: "agents",
  number: "08",
  label: "AI agents",
  short: "Agents",
  icon: "agent",
  colour: "#8be27a",
  title: "The *agentic nerve centre*.",
  thesis:
    "FrankenMining is built to be read and operated by AI agents as well as people. Every claim, refusal and decision has a typed, documented form, so an agent can analyse the operation, propose an intervention and show its evidence, while people keep the authority to act.",
  sections: [
    ["agent", "From a question to a costed proposal"],
    ["stack", "Alongside your systems"],
    ["never", "What it will never do"],
  ],
};

export const chapters = [idea, explore, plan, operate, processing, reconcile, decide, agents];

/** A figure that a client module draws into. The host keeps its room until the module has drawn. */
const fig = (name: string, todo: string, kind: Kind, note?: string, className?: string, ref?: Refs) =>
  figure({
    name,
    todo,
    kind,
    note,
    className,
    ref,
    body: `<div class="host" data-host></div><noscript><p class="noscript">This figure is drawn by a script.</p></noscript>`,
  });
/** A full-width live 3D scene. */
const scene = (name: string, todo: string, note?: string, kind: Kind = "scene", ref?: Refs) =>
  fig(name, todo, kind, note, "bleed scene", ref);

export function renderChapters(ref: Refs) {
  const out: string[] = [];

  /* ---------- 01 The idea ---------- */
  out.push(
    chapterOpen(idea) +
      section({
        ref,
        id: "claim",
        eyebrow: "The Claim",
        title: "Each number *carries its sources*.",
        kicker:
          "Open a number. Each one carries its value and interval, the samples and instruments behind it, its evidence class, how well its past forecasts scored, and an explicit allowance for causes outside the model. Ask `fmine claim why` and it answers.",
        more: ["The universal Claim", "8"],
        body: fig("claim", "Open a number to see what holds it up", "live", undefined, undefined, ref),
      }) +
      section({
        ref,
        id: "ladder",
        eyebrow: "Up and down the line",
        title: "One ledger, *read at every desk*.",
        kicker:
          "The shift supervisor sees trucks and faces. The CFO sees ounces and cost per ounce. Both read the same ledger, so the CFO’s cost per ounce traces down to the loads behind it, and a grade surprise at the shovel shows up in the CFO’s numbers the same day.",
        more: ["Readers and their questions", "3"],
        body: fig("ladder", "Pick a desk and follow one number down", "live"),
      }) +
      chapterClose,
  );

  /* ---------- 02 Explore ---------- */
  out.push(
    chapterOpen(explore) +
      section({
        ref,
        id: "drill",
        eyebrow: "From drillholes to a block model",
        title: "Drill a few holes. *Estimate the rest*, with error bars.",
        kicker:
          "The grade of every block between the holes is estimated here by FrankenMining’s own conditional Gaussian predictor, compiled from its Rust into WebAssembly. Add holes and the uncertainty narrows where they land; it stays wide where nobody has drilled.",
        more: ["Strata: the subsurface belief ledger", "13"],
        body: scene(
          "drill",
          "Drill more holes and watch the model sharpen",
          "The orebody is synthetic, generated in your browser. The estimate runs the exponential-covariance predictor inside FrankenMining’s spatial calibration monitor. A production kriging engine, held to a reference within 1e-6 relative, is a design target ([§13.11](§13.11)).",
          "engine",
          ref,
        ),
      }) +
      section({
        ref,
        id: "absence",
        eyebrow: "Negative evidence",
        title: "A dry hole is *evidence too*.",
        kicker:
          "FrankenMining records every survey that found nothing, with how deep it could see. A miss under thick cover counts for less than a miss at surface, and both stay on the map as evidence.",
        more: ["Lodestar: exploration", "11"],
        body: fig("absence", "Thicken the cover and see what a miss is worth", "live"),
      }) +
      chapterClose,
  );

  /* ---------- 03 Plan ---------- */
  out.push(
    chapterOpen(plan) +
      section({
        ref,
        id: "pit",
        eyebrow: "Certified ultimate pit",
        title: "Solved exactly, *checked independently*.",
        kicker:
          "Move the gold price, the mining cost or the slope angle and FrankenMining’s pseudoflow solver, running here as WebAssembly, finds the optimal pit again, then an independent checker verifies it. On all eleven MineLib benchmark pits its values match the published optima.",
        more: ["Pushback: planning under uncertainty", "15"],
        body: scene("pit", "Change the price and the slope; the pit re-solves", undefined, "engine"),
      }) +
      section({
        ref,
        id: "blast",
        eyebrow: "Drill and blast",
        title: "Blast with *the ore boundary in view*.",
        kicker:
          "A blast throws ore and waste metres from where the model put them. FrankenMining keeps the pattern, the firing sequence and the movement the blast monitors recorded, then redraws the dig lines where the ore landed and counts the dilution.",
        more: ["Drill and blast", "16.3"],
        body: scene("blast", "Fire the pattern"),
      }) +
      chapterClose,
  );

  /* ---------- 04 Operate ---------- */
  out.push(
    chapterOpen(operate) +
      section({
        ref,
        id: "haul",
        eyebrow: "Load and haul",
        title: "Every truckload, *in custody*.",
        kicker:
          "Each load is tied to its face, its weighed payload and its destination. When a queue builds at a shovel, an agent sees it in the ledger, sizes the lost tonnes and drafts the dispatch change for the shift supervisor to approve.",
        more: ["Headframe: the operations twin", "16"],
        body: scene("haul", "Watch the shift; pick a truck to see its load"),
      }) +
      section({
        ref,
        id: "underground",
        eyebrow: "Underground",
        title: "Underground, *in the same ledger*.",
        kicker:
          "Stopes, headings and loaders live in the same model as the pit. Each stope’s surveyed shape is compared with its design after firing, and ground support is tracked as a verified critical control.",
        more: ["Muster: workforce and critical controls", "19"],
        body: scene("underground", "Drag to look around the mine"),
      }) +
      chapterClose,
  );

  /* ---------- 05 Process ---------- */
  out.push(
    chapterOpen(processing) +
      section({
        ref,
        id: "plant",
        eyebrow: "Concentrator",
        title: "Every stream *balanced*.",
        kicker:
          "Crushing, grinding, flotation and refining, each with measured and inferred streams. FrankenMining reconciles the measurements into one balance that conserves mass and metal, and says which instrument it trusts least and why.",
        more: ["Crucible: processing and refining", "17"],
        body: scene("plant", "Follow the ore through the plant"),
      }) +
      section({
        ref,
        id: "grind",
        eyebrow: "Grinding",
        title: "Average hardness *understates throughput*.",
        kicker:
          "A mill’s throughput is inversely proportional to the ore’s Bond work index. Fed soft and hard ore in turn, it averages more tonnes an hour than it would on ore of the average hardness. FrankenMining’s grinding model, running here, shows how big the gap is.",
        more: ["Crucible: processing and refining", "17"],
        body: fig("grind", "Spread the hardness and watch the throughput", "engine"),
      }) +
      section({
        ref,
        id: "tailings",
        eyebrow: "Tailings and water",
        title: "The dam, *with the age of every reading*.",
        kicker:
          "Freeboard shows each piezometer’s last reading and its age, the survey that set the crest, and what a storm would do to the pond. Whether the dam is safe is the engineer of record’s call; Freeboard puts the readings, their ages and the storm forecast in front of them.",
        more: ["Freeboard: tailings, water and closure", "18"],
        body: scene("tailings", "Bring in the storm"),
      }) +
      chapterClose,
  );

  /* ---------- 06 Reconcile ---------- */
  out.push(
    chapterOpen(reconcile) +
      section({
        ref,
        id: "recon",
        eyebrow: "Reconciliation",
        title: "On-plan metal can *hide a grade miss*.",
        kicker:
          "In this synthetic example, polygon A delivered 1,200 dry tonnes carrying the 10,000 kg of copper expected from 1,000 tonnes. Metal reached 100% of plan; grade reached 83.33%. A monthly spreadsheet calls that a pass. FrankenMining puts the offset in kilograms and names the boundary where it happened.",
        more: ["Touchstone: reconciliation", "20"],
        body: fig("recon", "Step through the boundaries", "example"),
      }) +
      section({
        ref,
        id: "ledger",
        eyebrow: "Accounting",
        title: "Books *beside the ERP*.",
        kicker:
          "Physical claims become entries in a reconciling book kept with FrankenAccounting, which must agree with your ERP’s trial balance. Cost per ounce is built line by line, and every line can say what it rests on.",
        more: ["Bullion: accounting", "21"],
        body: fig("ledger", "Build cost per ounce line by line", "live"),
      }) +
      section({
        ref,
        id: "public",
        eyebrow: "Public assurance",
        title: "Checked against *what was published*.",
        kicker:
          "FrankenMining reads a producer’s public quarterly workbooks and checks every identity in them. It holds 22,732 published operating figures as claims, each with its source cell, recomputes 2,915 AISC identities across 20 registered workbooks, and lists every restatement that came without a note.",
        more: ["Hallmark: disclosure", "24"],
        body: fig("public", "Run the check", "live"),
      }) +
      chapterClose,
  );

  /* ---------- 07 Decide ---------- */
  out.push(
    chapterOpen(decide) +
      section({
        ref,
        id: "card",
        eyebrow: "The value of information",
        title: "Is the testwork *worth it*?",
        kicker:
          "Before paying for a test, FrankenMining asks what its answer could change. In this example, perfect information would be worth $80M; the metallurgical testwork actually on offer is worth $41M, or $31.2M after its cost and delay. The decision engine that computes it runs in this page.",
        more: ["Helm I: DecisionCards", "22"],
        body: fig("card", "Change how good the test is", "engine"),
      }) +
      section({
        ref,
        id: "capital",
        eyebrow: "Capital programme",
        title: "Choose the portfolio, *bound the downside*.",
        kicker:
          "Projects compete for the same capital and share the same risks. FrankenMining picks the portfolio with the best expected value whose bad tail stays inside the board’s limit, measured as conditional value at risk, and shows what the limit costs.",
        more: ["Helm II: the capital programme", "23"],
        body: fig("capital", "Tighten the board’s tail limit", "engine"),
      }) +
      section({
        ref,
        id: "journal",
        eyebrow: "Forecast journal",
        title: "Forecasts, *frozen then scored*.",
        kicker:
          "Every forecast is registered before its outcome and scored after it, so site guidance, model grades and an agent’s own predictions each build a track record in an append-only journal. Monitors raise an alarm when a forecast stream drifts out of calibration: switch on the drift below and the alarm sounds partway through the quarter, before that quarter’s guidance misses.",
        more: ["The forecast and calibration ledger", "10"],
        body: fig("journal", "Let the quarters resolve", "live"),
      }) +
      chapterClose,
  );

  /* ---------- 08 Agents ---------- */
  out.push(
    chapterOpen(agents) +
      section({
        ref,
        id: "agent",
        eyebrow: "An agent at work",
        title: "From a question *to a costed proposal*.",
        kicker:
          "An agent asks why recovery fell. It traces the claim to a hardness shift, checks the forecast record, meets a typed refusal where evidence is missing, and drafts a DecisionCard for the metallurgist. Every step is a documented command with a JSON schema.",
        more: ["Adit: surfaces and the agent model", "30"],
        body: fig("agent", "Play the session", "example"),
      }) +
      section({
        ref,
        id: "stack",
        eyebrow: "Alongside your systems",
        title: "It reads *the systems you already run*.",
        kicker:
          "ERP, fleet management, historians, laboratory systems and geology tools stay where they are. FrankenMining reads from them, keeps the evidence, and gives people and agents one place to ask.",
        more: ["Alongside the systems of record", "1.5"],
        body: fig("stack", "Pick a system to see what flows", "live"),
      }) +
      section({
        ref,
        id: "never",
        eyebrow: "Boundaries",
        title: "What it *will never do*.",
        kicker: "These limits are in the design, and they bind agents as well as people.",
        more: ["Security, confidentiality and OT safety", "31"],
        body: never(),
      }) +
      chapterClose,
  );
  return out.join("");
}

const NEVER: [string, string][] = [
  ["Write to a control system", "It reads plant and fleet data. It never sends a setpoint, a dispatch or a trip to operational technology."],
  ["Post to your ERP", "The ERP stays the statutory ledger. Agents can draft entries; they cannot post them anywhere."],
  ["Classify a reserve", "Resources and reserves are declared by a competent person. FrankenMining assembles the evidence they sign."],
  ["Sign off a dam", "The engineer of record owns that call. Freeboard shows every reading with its age, and refuses a “within TARP” statement while any is stale."],
  ["Hide an unknown", "Every claim carries an explicit allowance for causes outside the model; a decision-facing number without one is not shown."],
  ["Rescore a forecast", "Forecasts are frozen before the outcome. An edit is refused, from a person or an agent; a changed forecast is a new one, and the old one is still scored."],
];
const never = () =>
  `<ul class="never reveal">${NEVER.map(
    ([h, p]) => `<li><span class="never-x" aria-hidden="true">${icon("cross")}</span><h4>${escape(h)}</h4><p>${escape(p)}</p></li>`,
  ).join("")}</ul>`;
