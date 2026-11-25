/**
 * Six numbers from one synthetic gold mine, each written out as the universal Claim of plan §8: value and
 * representation, support, evidence class (§7.1), calibration flag, unknown-unknown term (§8.1 UuTerm), provenance and
 * the guards of §8.3. Refusal codes and their fixes are the registered ones in crates/fmine-evidence (CATALOG).
 * Every value is synthetic.
 */
import type { Ev } from "./claim-ev";

export type Dist = { kind: "normal"; mean: number; sd: number } | { kind: "split"; p10: number; p50: number; p90: number };
export type GuardStatus = "pass" | "fail" | "warn" | "na";
export interface Guard {
  id: string;
  status: GuardStatus;
  note: string;
}
export interface Refusal {
  code: string;
  exit_code: number;
  what: string;
  fix: string;
}
export interface ClaimData {
  key: string;
  tile: string;
  when: string;
  value: string;
  unit: string;
  delta?: string;
  subject: string;
  repr: string;
  dist: Dist;
  axis: [number, number];
  ticks: number[];
  digits: number;
  interval: string;
  mark?: { at: number; label: string };
  state: "Known" | "Estimated";
  space: string;
  time: string;
  where: string;
  span: string;
  validity: string;
  additivity: "ExtensiveAdditive" | "IntensiveWeighted" | "NonAdditive";
  addNote: string;
  ev: Ev;
  evWhy: string;
  authority: string;
  cal: {
    kind: "Calibrated" | "Miscalibrated" | "NotCalibratable" | "Uncalibrated";
    record?: string;
    n?: number;
    /** Outcomes shown in the strip, and how many fell inside the 80% interval. */
    shown?: number;
    hits?: number;
    cov80?: number;
    bias?: string;
    inflation?: number;
    note: string;
  };
  uu: { kind: "ref" | "catch" | "flip" | "ndr"; head: string; text: string; eps?: number; scenario: string; trigger: string; exposure: number };
  prov: { sources: [label: string, ev: Ev][]; steps: string[] };
  guards: Guard[];
  color: Record<string, unknown>;
  admission?: Refusal;
  warnings?: string[];
}

export const SPACE = ["Point", "Composite", "Block", "Bench", "Wall", "Lot", "Stream", "Site"];
export const TIME = ["Instant", "Shift", "Day", "Week", "Month", "Quarter", "Year", "LoM"];

export const UU_FORMS = [
  ["ref", "Reference class ε"],
  ["catch", "Catch-all floor"],
  ["flip", "Flip margin"],
  ["ndr", "Not decision-relevant"],
] as const;

const REF_205: Refusal = {
  code: "FMINE-REF-205",
  exit_code: 3,
  what: "east-wall factor of safety is ModelledUncalibrated; decision-grade use needs calibrated evidence",
  fix: "Calibrate the inputs, or accept the lower colour the evidence supports.",
};

export const CLAIMS: ClaimData[] = [
  {
    key: "head",
    tile: "Head grade",
    when: "October",
    value: "1.42",
    unit: "g/t Au",
    subject: "Mill feed through sampler CV-3, 1–31 October",
    repr: "Gaussian { mean: 1.42, std_dev: 0.039 }",
    dist: { kind: "normal", mean: 1.42, sd: 0.039 },
    axis: [1.27, 1.57],
    ticks: [1.3, 1.35, 1.4, 1.45, 1.5, 1.55],
    digits: 2,
    interval: "1.37 – 1.47",
    state: "Known",
    space: "Stream",
    time: "Month",
    where: "Mill feed stream, cross-stream sampler CV-3",
    span: "1–31 October, 93 shifts",
    validity: "Current flowsheet and sampler; closed at month end",
    additivity: "IntensiveWeighted",
    addNote: "Grades combine by dry mass. A plain average of grades is refused (FMINE-REF-103).",
    ev: "derived",
    evWhy: "A mass-weighted mean of 93 shift composites, each fire-assayed in a batch whose reference materials passed.",
    authority: "Computed by fmine-plant",
    cal: {
      kind: "Calibrated",
      record: "cal_head-vs-backcalc",
      n: 24,
      shown: 24,
      hits: 19,
      cov80: 0.79,
      note: "Each month is checked against the head grade back-calculated from gold poured plus tails. 19 of the last 24 fell inside the 80% interval.",
    },
    uu: {
      kind: "ref",
      eps: 0.02,
      head: "Reference class, ε = 0.02",
      text: "A 2% allowance that the sampling itself is biased in a way no QA/QC check would see, taken from the record of cross-stream samplers at gold plants.",
      scenario: "uu-sampler-bias",
      trigger: "check-sampler duplicates outside ±5%",
      exposure: 0.4,
    },
    prov: {
      sources: [
        ["Shift composites ×93\nfire assay", "measured"],
        ["Weightometer WT-2\nwet tonnes", "measured"],
        ["Moisture assays ×31", "measured"],
        ["Reference-material\nbatches, passed", "measured"],
      ],
      steps: ["Wet to dry\nby moisture", "Mass-weighted\nmean"],
    },
    guards: [
      { id: "G-UU", status: "pass", note: "One unknown-unknown term, citing its reference class" },
      { id: "G-EXEC", status: "pass", note: "Not refused, not stale; calibration shown as recorded" },
      { id: "G-SUPPORT", status: "pass", note: "Shifts combined by dry mass" },
      { id: "G-CONS", status: "pass", note: "Feed gold = concentrate + tails, within tolerance" },
      { id: "G-DISC", status: "na", note: "Internal figure, not disclosure-facing" },
    ],
    color: { status: "available", value: { kind: "validated", dataset: "qaqc:lims-batches-oct" }, refusal: null },
  },
  {
    key: "payload",
    tile: "Truck payload",
    when: "load 4471",
    value: "221.6",
    unit: "t wet",
    subject: "Haul truck HT-14 from face N-07, shift B, 23 October 09:41",
    repr: "Gaussian { mean: 221.6, std_dev: 1.7 }",
    dist: { kind: "normal", mean: 221.6, sd: 1.7 },
    axis: [215.5, 227.7],
    ticks: [217, 219, 221, 223, 225, 227],
    digits: 1,
    interval: "219.4 – 223.8",
    state: "Known",
    space: "Lot",
    time: "Instant",
    where: "One haul-truck load: a custody lot",
    span: "Instant: tipped at the crusher, 09:58",
    validity: "Payload calibration card v3, valid to 30 November",
    additivity: "ExtensiveAdditive",
    addNote: "Loads add up to shift tonnes. These are wet tonnes: dry needs a moisture claim, or it is refused (FMINE-REF-105).",
    ev: "measured",
    evWhy: "Read by the truck’s on-board payload system from strut pressures, inside its calibration envelope.",
    authority: "Computed by fmine-ops",
    cal: {
      kind: "Calibrated",
      record: "cal_payload-vs-weighbridge",
      n: 412,
      shown: 30,
      hits: 25,
      cov80: 0.81,
      bias: "−0.4 t",
      note: "Checked against weighbridge tickets on 412 paired loads, bias −0.4 t. The strip shows the last 30.",
    },
    uu: {
      kind: "ndr",
      head: "Not decision-relevant",
      text: "One load drives no decision by itself. It reaches decisions through shift totals, which carry their own term.",
      scenario: "uu-not-decision-relevant",
      trigger: "none: single custody lot",
      exposure: 0,
    },
    prov: {
      sources: [
        ["Strut-pressure log\nfleet snapshot, hashed", "measured"],
        ["Weighbridge pairs\nlast 30 days", "measured"],
        ["Calibration card v3", "derived"],
      ],
      steps: ["Payload model\npressure → tonnes"],
    },
    guards: [
      { id: "G-UU", status: "pass", note: "Term present: not decision-relevant, with its reason" },
      { id: "G-EXEC", status: "pass", note: "Inside the calibration card’s validity window" },
      { id: "G-SUPPORT", status: "pass", note: "Adds by wet mass within the shift" },
      { id: "G-CONS", status: "pass", note: "Shift loads reconcile with the crusher weightometer" },
    ],
    color: { status: "available", value: { kind: "validated", dataset: "cal:payload-card-v3" }, refusal: null },
  },
  {
    key: "recovery",
    tile: "Flotation recovery",
    when: "week 43",
    value: "86.2",
    unit: "% Au",
    delta: "−3.1 pts",
    subject: "Rougher and cleaner flotation, gold to concentrate, 19–25 October",
    repr: "Gaussian { mean: 86.2, std_dev: 0.85 }",
    dist: { kind: "normal", mean: 86.2, sd: 0.85 },
    axis: [83.2, 90.6],
    ticks: [84, 85, 86, 87, 88, 89, 90],
    digits: 1,
    interval: "85.1 – 87.3",
    mark: { at: 89.3, label: "week 42: 89.3" },
    state: "Estimated",
    space: "Stream",
    time: "Week",
    where: "Flotation circuit: feed, concentrate and tail streams",
    span: "Week 43, 19–25 October",
    validity: "Current reagent suite and grind; balance closes within tolerance",
    additivity: "NonAdditive",
    addNote: "A ratio. Weeks combine through metal and mass; an average of percentages is refused (FMINE-REF-103).",
    ev: "derived",
    evWhy: "The two-product formula on feed, concentrate and tail assays, after a balance that conserves mass and metal.",
    authority: "Computed by fmine-plant",
    cal: {
      kind: "Calibrated",
      record: "cal_flot-recovery-vs-poured",
      n: 12,
      shown: 12,
      hits: 10,
      cov80: 0.83,
      note: "Checked each month against gold poured: 10 of 12 inside. This week also fell below the geometallurgy forecast’s 80% interval, and a surprise monitor raised a flag.",
    },
    uu: {
      kind: "catch",
      eps: 0.05,
      head: "Catch-all, floor 0.05",
      text: "At least 5% weight stays on causes outside the modelled set: coarse free gold, carbon in the feed, a reagent fault.",
      scenario: "uu-recovery-mechanisms",
      trigger: "tail grade above 0.25 g/t for two shifts",
      exposure: 1.1,
    },
    prov: {
      sources: [
        ["Feed assays\nshift composites", "measured"],
        ["Concentrate assays", "measured"],
        ["Tail assays", "measured"],
        ["Flow and density\nmeters", "measured"],
      ],
      steps: ["Mass balance\nfull covariance", "Two-product\nrecovery"],
    },
    guards: [
      { id: "G-UU", status: "pass", note: "Catch-all term with its floor" },
      { id: "G-EXEC", status: "pass", note: "Not refused, not stale" },
      { id: "G-CONS", status: "pass", note: "Balance conserves mass and gold within tolerance" },
      { id: "G-SUPPORT", status: "pass", note: "One circuit, one week, one basis" },
      { id: "Monitor", status: "warn", note: "Surprise flag: below the frozen forecast’s 80% interval" },
    ],
    color: { status: "available", value: { kind: "validated", dataset: "qaqc:plant-balance-wk43" }, refusal: null },
  },
  {
    key: "aisc",
    tile: "AISC outlook",
    when: "Q4",
    value: "1,318",
    unit: "$/oz",
    subject: "Whole site, 100% basis, 1 October – 31 December, price deck $2,800/oz",
    repr: "Quantiles { p10: 1262.0, p50: 1318.0, p90: 1391.0 }",
    dist: { kind: "split", p10: 1262, p50: 1318, p90: 1391 },
    axis: [1170, 1490],
    ticks: [1200, 1250, 1300, 1350, 1400, 1450],
    digits: 0,
    interval: "1,262 – 1,391",
    state: "Estimated",
    space: "Site",
    time: "Quarter",
    where: "Whole site, 100% project basis",
    span: "Quarter: 1 October – 31 December",
    validity: "Price deck $2,800/oz, current mine plan, AISC definition v2",
    additivity: "NonAdditive",
    addNote: "A unit cost. Costs and ounces are summed separately, then divided.",
    ev: "calibrated",
    evWhy: "A driver-based cost model, run inside the domain where its quarterly outlooks have a scored record.",
    authority: "Computed by fmine-metrics",
    cal: {
      kind: "Miscalibrated",
      record: "cal_aisc-outlook-quarterly",
      n: 12,
      shown: 12,
      hits: 7,
      cov80: 0.58,
      bias: "+2.9%",
      inflation: 1.4,
      note: "Past outlooks ran 2.9% low and covered 7 of 12 outcomes. The interval above is shifted and widened 1.4× to match that record.",
    },
    uu: {
      kind: "ref",
      eps: 0.05,
      head: "Reference class, ε = 0.05",
      text: "A 5% allowance for cost shocks outside the model, from the record of quarterly cost outlooks at gold producers.",
      scenario: "uu-cost-shock",
      trigger: "diesel or reagent prices 20% above the deck",
      exposure: 2.6,
    },
    prov: {
      sources: [
        ["ERP cost extract\nOctober, read-only", "measured"],
        ["Contracts and rates", "measured"],
        ["Mine plan, Q4\ntonnes and grade", "calibrated"],
        ["Ounces outlook, Q4", "calibrated"],
      ],
      steps: ["Cost model v7\nby driver", "÷ ounces"],
    },
    guards: [
      { id: "G-UU", status: "pass", note: "Reference-class term present" },
      { id: "G-EXEC", status: "pass", note: "Miscalibration shown as recorded, with its correction" },
      { id: "G-SUPPORT", status: "pass", note: "Costs and ounces summed separately, then divided" },
      { id: "G-DISC", status: "fail", note: "Reference class not yet verified, no attested record: blocked from disclosure" },
    ],
    color: { status: "available", value: { kind: "validated", dataset: "cal_aisc-outlook-quarterly" }, refusal: null },
  },
  {
    key: "fos",
    tile: "Factor of safety",
    when: "east wall E3",
    value: "1.34",
    unit: "FoS",
    subject: "East wall sector E3, benches 1,140–1,260, current pushback",
    repr: "EnsembleSummary { n: 10000, mean: 1.34, p10: 1.24, p50: 1.34, p90: 1.45 }",
    dist: { kind: "split", p10: 1.24, p50: 1.34, p90: 1.45 },
    axis: [1.08, 1.62],
    ticks: [1.1, 1.2, 1.3, 1.4, 1.5, 1.6],
    digits: 2,
    interval: "1.24 – 1.45",
    mark: { at: 1.2, label: "design acceptance 1.2" },
    state: "Estimated",
    space: "Wall",
    time: "Instant",
    where: "East wall, sector E3, benches 1,140–1,260",
    span: "The wall as surveyed on 20 October",
    validity: "Until the next survey, or a 2 m rise at piezometer VWP-E3-2",
    additivity: "NonAdditive",
    addNote: "Never averaged across sectors or walls (FMINE-REF-103).",
    ev: "uncalibrated",
    evWhy: "Limit equilibrium over 10,000 draws of strength, joint persistence and pore pressure. One input is an engineer’s elicited prior.",
    authority: "Computed by fmine-geotech. Sign-off stays with the engineer of record.",
    cal: {
      kind: "NotCalibratable",
      note: "Wall failures here are too rare to score the model, so it has no calibration record. It is checked against prism movement instead.",
    },
    uu: {
      kind: "flip",
      head: "Flip margin",
      text: "Design acceptance (FoS ≥ 1.2) holds unless pore pressure at VWP-E3-2 rises 6 m above its last reading.",
      scenario: "uu-pore-pressure",
      trigger: "VWP-E3-2 up 3 m in 7 days",
      exposure: 14,
    },
    prov: {
      sources: [
        ["Lab strength tests ×38", "measured"],
        ["Joint mapping and\nwall survey", "measured"],
        ["Piezometer VWP-E3-2", "measured"],
        ["Joint persistence\nprior, engineer", "elicited"],
      ],
      steps: ["Limit equilibrium\n10,000 draws"],
    },
    guards: [
      { id: "G-UU", status: "pass", note: "Flip margin stated" },
      { id: "G-EXEC", status: "pass", note: "Piezometer read 5 days ago, inside its 7-day window" },
      { id: "G-AOA", status: "pass", note: "Strength cards from this pit’s own tests" },
      { id: "Decision", status: "fail", note: "FMINE-REF-205: uncalibrated, so refused as a decision input" },
      { id: "Sign-off", status: "na", note: "Never: geotechnical_signoff is false in every output" },
    ],
    color: {
      status: "available",
      value: { kind: "estimated", estimator: "limit-equilibrium-mc", dispersion: 0.081, dispersion_status: "finite" },
      refusal: null,
    },
    admission: REF_205,
  },
  {
    key: "freeboard",
    tile: "Tailings freeboard",
    when: "TSF-1",
    value: "2.38",
    unit: "m",
    subject: "Tailings storage facility 1, north embankment, 24 October 06:00",
    repr: "Gaussian { mean: 2.38, std_dev: 0.05 }",
    dist: { kind: "normal", mean: 2.38, sd: 0.05 },
    axis: [2.19, 2.57],
    ticks: [2.2, 2.25, 2.3, 2.35, 2.4, 2.45, 2.5, 2.55],
    digits: 2,
    interval: "2.32 – 2.44",
    state: "Known",
    space: "Point",
    time: "Instant",
    where: "Lowest point of the north embankment crest",
    span: "Instant: 24 October 06:00",
    validity: "Crest survey of 2 October; radar gauge inside its check window",
    additivity: "NonAdditive",
    addNote: "A level difference at one point. Never averaged along the crest.",
    ev: "derived",
    evWhy: "Surveyed crest elevation minus the pond level read by the radar gauge, both measured.",
    authority: "Computed by fmine-tailings. The engineer of record decides.",
    cal: {
      kind: "Calibrated",
      record: "cal_radar-vs-staff-gauge",
      n: 58,
      shown: 30,
      hits: 25,
      cov80: 0.81,
      note: "The radar level is checked against the staff gauge at each inspection: 58 checks. The strip shows the last 30.",
    },
    uu: {
      kind: "flip",
      head: "Flip margin",
      text: "The 1.0 m minimum holds unless the pond rises 1.38 m beyond forecast. The design storm adds 0.9 m.",
      scenario: "uu-pond-rise",
      trigger: "pond up 0.4 m in 24 h",
      exposure: 9.5,
    },
    prov: {
      sources: [
        ["Crest survey\n2 October", "measured"],
        ["Radar pond gauge\n06:00 reading", "measured"],
        ["Staff-gauge checks ×58", "measured"],
      ],
      steps: ["Crest elevation\n− pond level"],
    },
    guards: [
      { id: "G-UU", status: "pass", note: "Flip margin stated" },
      { id: "G-EXEC", status: "pass", note: "Gauge read at 06:00, fresh" },
      { id: "G-CONS", status: "pass", note: "The week’s water balance closes" },
      { id: "TARP", status: "fail", note: "FMINE-REF-404: piezometer P-7 last read 9 days ago, so no ‘within TARP’ statement" },
    ],
    color: { status: "available", value: { kind: "validated", dataset: "cal:radar-vs-staff-gauge" }, refusal: null },
    warnings: ["FMINE-REF-404: no 'within TARP' statement while piezometer P-7 is stale (last read 9 days ago, window 72 h)"],
  },
];

/** Where each claim's sources sit, as `fmine claim trace` lists anchors: file, locator, digest. Synthetic paths. */
export const ANCHORS: Record<string, [path: string, locator: string][]> = {
  head: [
    ["lims/oct/shift-composites.csv", "rows 2–94"],
    ["historian/wt-2/oct.snapshot", "tag WT2.TPH, 1–31 Oct"],
    ["lims/oct/reference-batches.csv", "batches B-0412 to B-0446"],
  ],
  payload: [
    ["fleet/snapshots/shift-b-23oct.jsonl", "load 4471"],
    ["weighbridge/tickets-oct.csv", "pairs 381–412"],
    ["cards/payload-v3.toml", "[calibration]"],
  ],
  recovery: [
    ["lims/wk43/flotation-survey.csv", "feed, concentrate, tail rows"],
    ["historian/flotation/wk43.snapshot", "tags FIT-301, DIT-302"],
    ["balance/wk43.toml", "covariance block"],
  ],
  aisc: [
    ["erp/extracts/gl-oct.csv", "cost centres 4100–4900"],
    ["plans/q4-schedule.omf", "periods 10–12"],
    ["models/cost-v7.toml", "driver table"],
  ],
  fos: [
    ["geotech/lab/strength-tests.csv", "38 tests, east domain"],
    ["geotech/vwp/VWP-E3-2.csv", "last reading, 19 Oct"],
    ["survey/east-wall-20oct.dxf", "sector E3"],
  ],
  freeboard: [
    ["survey/tsf1-crest-02oct.csv", "lowest point, N-14"],
    ["historian/tsf1/radar-level.snapshot", "06:00 reading"],
    ["inspections/staff-gauge.csv", "58 checks"],
  ],
};
