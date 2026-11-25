/**
 * What sits around FrankenMining (plan §1.5, §1.6, §27, §31.3, §32.2): the systems it reads, the subsystems that
 * take each feed (§0.5), the readers it serves and the sibling engines beneath it. Generic system names only.
 */

export type Posture = "ot" | "recon" | "read" | "public";
export interface System {
  id: string;
  name: string;
  kind: string;
  posture: Posture;
  badge: string;
  takes: string[];
  gives: string[];
  feeds: string[];
}
export const SYSTEMS: System[] = [
  {
    id: "erp",
    name: "ERP and accounting",
    kind: "Transactions and the statutory ledger",
    posture: "recon",
    badge: "Reconciled against, never posted to",
    takes: ["Flat-file, IDoc or OData extracts: trial balance, cost centres, stores and payroll totals."],
    gives: [
      "A reconciling book that rebuilds its trial balance and must agree with it. Every difference is a cited register item.",
      "Estimates that move money, such as depletion, royalties and survey true-ups, reach the controllers as workpapers. The controllers post them in their own ERP.",
    ],
    feeds: ["bullion", "helm", "muster"],
  },
  {
    id: "fleet",
    name: "Fleet management",
    kind: "Dispatch, loads, payloads, cycle times",
    posture: "ot",
    badge: "Read-only, one way",
    takes: ["Content-hashed snapshots of loads, payloads, sources and destinations, into quarantine first."],
    gives: ["Loads reconciled with survey and plant, so it is known which ore went where.", "Nothing back to dispatch or to autonomous haulage."],
    feeds: ["headframe", "touchstone"],
  },
  {
    id: "historian",
    name: "Plant historian and SCADA",
    kind: "Tags from the control system",
    posture: "ot",
    badge: "Read-only, one way",
    takes: ["Snapshots, or a read-only client on a relay outside the control network. Outbound only."],
    gives: ["Throughput, recovery and tailings levels as Claims, each with its uncertainty and freshness.", "No setpoint and no write path to a control system."],
    feeds: ["crucible", "freeboard", "headframe"],
  },
  {
    id: "lims",
    name: "Laboratory (LIMS)",
    kind: "Assays, standards, duplicates",
    posture: "read",
    badge: "Read-only",
    takes: ["Assay batches with their reference materials, blanks and duplicates."],
    gives: ["Each assay with its quality-control state. A failed batch stays visible; it is not averaged in."],
    feeds: ["corebox", "crucible", "touchstone"],
  },
  {
    id: "geology",
    name: "Geology and mine planning",
    kind: "Models, designs, schedules",
    posture: "read",
    badge: "Read-only exports",
    takes: ["CSV, OMF, DXF and string exports, read through a registry of file dialects."],
    gives: ["Plans and models tracked against what was mined, with their forecasts frozen and scored."],
    feeds: ["lodestar", "strata", "highwall", "pushback"],
  },
  {
    id: "filings",
    name: "Public filings and documents",
    kind: "Reports, releases, technical reports",
    posture: "public",
    badge: "Public basis",
    takes: ["Published figures, each cited to its page and table."],
    gives: ["Public-basis Claims, kept apart from internal ones.", "Draft text checked against the ledger before it goes out."],
    feeds: ["hallmark", "bullion"],
  },
];

export interface Sub {
  id: string;
  name: string;
  what: string;
  to?: string[];
}
/** Ordered so feeds cross as little as they can. */
export const SUBS: Sub[] = [
  { id: "bullion", name: "Bullion", what: "Metal accounting, AISC and costs", to: ["fa", "board"] },
  { id: "helm", name: "Helm", what: "Decisions, value of information, capital", to: ["board"] },
  { id: "muster", name: "Muster", what: "Rosters, competencies, fatigue" },
  { id: "headframe", name: "Headframe", what: "Drill and blast, load and haul" },
  { id: "touchstone", name: "Touchstone", what: "Reconciliation and F‑factors" },
  { id: "crucible", name: "Crucible", what: "Comminution, flotation, leaching" },
  { id: "freeboard", name: "Freeboard", what: "Tailings, water and closure" },
  { id: "corebox", name: "Corebox", what: "Drilling, sampling, assays, QA/QC" },
  { id: "lodestar", name: "Lodestar", what: "Exploration targets" },
  { id: "strata", name: "Strata", what: "Geological interpretation, domains" },
  { id: "highwall", name: "Highwall", what: "Slopes, ground control, TARPs" },
  { id: "pushback", name: "Pushback", what: "Plans, schedules, stockpiles, blends" },
  { id: "hallmark", name: "Hallmark", what: "Resources, reserves, disclosure", to: ["board"] },
  { id: "adit", name: "Adit", what: "CLI, robot JSON, reports", to: ["people", "agents"] },
];

export interface Reader {
  id: string;
  name: string;
  sub: string;
  gets: string[];
  limit: string;
}
export const READERS: Reader[] = [
  {
    id: "people",
    name: "People at every desk",
    sub: "Board to shift supervisor",
    gets: ["The same number at every desk, with its evidence, its interval and where it came from.", "Reports, a terminal view and the CLI."],
    limit: "Authority stays with people: they approve, sign and post.",
  },
  {
    id: "agents",
    name: "AI agents",
    sub: "CLI and robot JSON today · MCP server in design",
    gets: ["Cited numbers and typed refusals in one JSON envelope, with schemas a fresh agent can read.", "The same verbs as MCP tools once the server (fmine‑mcp), now in design, is built."],
    limit: "Agents read and prepare. Their output enters quarantine as evidence, and they cannot approve or execute.",
  },
  {
    id: "board",
    name: "The board pack",
    sub: "Every figure cited to its Claim",
    gets: ["Each figure replayable from source, with its evidence class.", "Decisions as DecisionCards, with the value of finding out first."],
    limit: "Nothing is published as a reserve statement or a sign-off: those stay with the competent people.",
  },
  {
    id: "fa",
    name: "FrankenAccounting reconciling book",
    sub: "Must agree with the ERP trial balance",
    gets: ["A book rebuilt from ERP extracts that must agree with its trial balance. Every difference is a cited register item."],
    limit: "The ERP stays the ledger of record, and the controllers post entries there.",
  },
];

export interface Engine {
  id: string;
  name: string;
  role: string;
  how: string;
}
export const ENGINES: Engine[] = [
  { id: "sim", name: "FrankenSim", role: "Numerics and geometry", how: "Routines are copied in with provenance headers." },
  { id: "acct", name: "FrankenAccounting", role: "The books", how: "Runs as a separate process. It has no mining logic; Bullion supplies that." },
  { id: "surv", name: "FrankenSurveillance", role: "Evidence doctrine", how: "Doctrine only, no code: planes of evidence, a nine-state lattice, blanks never read as absence." },
];

export const LEDGER = ["Claims, each with its evidence", "Forecasts, frozen and scored", "Decisions, as DecisionCards"];
