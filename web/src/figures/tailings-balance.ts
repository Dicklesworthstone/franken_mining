/**
 * The pond's water balance through a storm, hour by hour: rain on the pond, runoff from the beach and the hills
 * (each with its initial loss, runoff coefficient and lag), slurry water from the spigots, the decant return pumps,
 * evaporation and seepage. The level follows the stage–storage curve of the bed itself. An ensemble of runs with
 * the uncertain inputs drawn at random gives the tail that FrankenMining's CVaR kernel reads.
 *
 * Every rate and criterion here is illustrative, for a synthetic facility. Nothing touches the DOM.
 */
import { rng } from "./lib";
import { type Balance, CREST_LOW, edgeAt, HILLS_M2, PMP, POND, rainFor } from "./tailings-model";

/** Hours simulated: the 72-hour storm and a day after it. */
export const HOURS = 96;
export const STORM_HOURS = 72;
const DT = 0.25;
const STEPS = Math.round(HOURS / DT);

/** The rates the balance runs on (illustrative). */
export const RATES = {
  /** Slurry water that reaches the pond, m³/s: deposition carries on through the storm. */
  slurry: 0.155,
  /** The decant return pumps, m³/s, when they run. */
  pumps: 0.2,
  /** Open-water evaporation, mm a day, in dry hours and under rain. */
  evapDry: 6,
  evapWet: 1,
  /** Seepage, m³/s: through the drains (measured at the weir) and into the foundation (estimated). */
  seepDrains: 0.0064,
  seepFoundation: 0.002,
  /** Initial loss (mm), runoff coefficient and lag (h, a linear reservoir) for the hills and the beach. */
  hills: { loss: 20, c: 0.42, lag: 4 },
  beach: { loss: 4, c: 0.75, lag: 1.2 },
};

/**
 * The engineer of record's freeboard criteria (illustrative): after the design storm the pond must still sit a
 * wave allowance and a residual below the crest, and its edge must stay a beach length from the crest. The least
 * operating freeboard adds the design storm's own rise to the first two.
 */
export const CRITERIA = { wave: 0.5, residual: 0.5, beach: 150, designYears: 100 };

export interface Terms {
  rainPond: number;
  beach: number;
  hills: number;
  slurry: number;
  pumped: number;
  evaporation: number;
  seepage: number;
}

export interface Run {
  mm: number;
  pumps: boolean;
  /** Per step: hour, rain intensity (mm/h) and rain so far (mm), pond level, freeboard to the crest's low point,
   * beach length. */
  hour: Float64Array;
  rain: Float64Array;
  rainSoFar: Float64Array;
  level: Float64Array;
  freeboard: Float64Array;
  beachLength: Float64Array;
  /** Volumes over the whole run, m³ (outflows positive), and their running totals at each step. */
  terms: Terms;
  cum: Record<keyof Terms, Float64Array>;
  /** The highest the pond gets: its rise, the hour, and the freeboard and beach then. */
  peak: { rise: number; hour: number; freeboard: number; beach: number };
  /** Peak inflow to the pond, m³/s. */
  peakInflow: number;
}

/** The storm's hyetograph: share of the 72-hour depth in each step, a long rain with a burst near hour 30. */
const PATTERN = (() => {
  const w = new Float64Array(STEPS);
  let sum = 0;
  for (let i = 0; i < STEPS; i++) {
    const t = (i + 0.5) * DT;
    if (t >= STORM_HOURS) continue;
    const u = t / STORM_HOURS;
    const base = u ** 1.2 * (1 - u) ** 2;
    const burst = Math.exp(-0.5 * ((t - 30) / 2.6) ** 2);
    const v = 0.6 * (base / 0.0405) / STORM_HOURS + (0.4 * burst) / (2.6 * Math.sqrt(2 * Math.PI));
    w[i] = v;
    sum += v;
  }
  for (let i = 0; i < STEPS; i++) w[i]! /= sum;
  return w;
})();

interface Knobs {
  rainScale: number;
  hillsC: number;
  beachC: number;
  areaScale: number;
  /** Hour the pumps stop, if they were running (Infinity: never). */
  pumpStop: number;
}
const NOMINAL: Knobs = { rainScale: 1, hillsC: RATES.hills.c, beachC: RATES.beach.c, areaScale: 1, pumpStop: Infinity };

function run(bal: Balance, mm: number, pumps: boolean, k: Knobs, record: boolean): Run | { peakRise: number; minBeach: number } {
  const hour = record ? new Float64Array(STEPS + 1) : null;
  const rain = record ? new Float64Array(STEPS + 1) : null;
  const rainSoFar = record ? new Float64Array(STEPS + 1) : null;
  const level = record ? new Float64Array(STEPS + 1) : null;
  const terms: Terms = { rainPond: 0, beach: 0, hills: 0, slurry: 0, pumped: 0, evaporation: 0, seepage: 0 };
  const keys = Object.keys(terms) as (keyof Terms)[];
  const running = record ? (Object.fromEntries(keys.map((k) => [k, new Float64Array(STEPS + 1)])) as Record<keyof Terms, Float64Array>) : null;
  let h = POND;
  let cum = 0;
  let hillStore = 0;
  let beachStore = 0;
  let peak = POND;
  let peakHour = 0;
  let peakIn = 0;
  const dtS = DT * 3600;
  const depth = mm * k.rainScale;
  for (let i = 0; i < STEPS; i++) {
    const t = i * DT;
    const p = depth * PATTERN[i]!; // mm this step
    const pond = bal.areaAt(h - POND) * k.areaScale;
    const beach = Math.max(0, bal.impoundment - pond);
    // Rain excess after the initial loss, onto each surface, then routed through its lag.
    const before = cum;
    cum += p;
    const ex = (loss: number) => Math.max(0, cum - loss) - Math.max(0, before - loss);
    hillStore += (ex(RATES.hills.loss) / 1000) * HILLS_M2 * k.hillsC;
    beachStore += (ex(RATES.beach.loss) / 1000) * beach * k.beachC;
    const hillOut = hillStore * (1 - Math.exp(-DT / RATES.hills.lag));
    const beachOut = beachStore * (1 - Math.exp(-DT / RATES.beach.lag));
    hillStore -= hillOut;
    beachStore -= beachOut;
    const onPond = (p / 1000) * pond;
    const slurry = RATES.slurry * dtS;
    const pumped = pumps && t < k.pumpStop ? RATES.pumps * dtS : 0;
    const evap = ((p > 0.05 ? RATES.evapWet : RATES.evapDry) / 1000 / 24) * DT * pond;
    const seep = (RATES.seepDrains + RATES.seepFoundation) * dtS;
    const dv = onPond + beachOut + hillOut + slurry - pumped - evap - seep;
    h += dv / Math.max(1, pond);
    terms.rainPond += onPond;
    terms.beach += beachOut;
    terms.hills += hillOut;
    terms.slurry += slurry;
    terms.pumped += pumped;
    terms.evaporation += evap;
    terms.seepage += seep;
    peakIn = Math.max(peakIn, (onPond + beachOut + hillOut + slurry) / dtS);
    if (h > peak) {
      peak = h;
      peakHour = t + DT;
    }
    if (record) {
      hour![i] = t;
      rain![i] = p / DT;
      rainSoFar![i + 1] = cum;
      level![i + 1] = h;
      for (const key of keys) running![key][i + 1] = terms[key];
    }
  }
  if (!record) return { peakRise: peak - POND, minBeach: edgeAt(peak) };
  hour![STEPS] = HOURS;
  level![0] = POND;
  const freeboard = level!.map((l) => CREST_LOW - l);
  const beachLength = level!.map((l) => edgeAt(l));
  return {
    mm,
    pumps,
    hour: hour!,
    rain: rain!,
    rainSoFar: rainSoFar!,
    level: level!,
    freeboard,
    beachLength,
    terms,
    cum: running!,
    peak: { rise: peak - POND, hour: peakHour, freeboard: CREST_LOW - peak, beach: edgeAt(peak) },
    peakInflow: peakIn,
  };
}

/** One run with the nominal inputs. */
export const simulate = (bal: Balance, mm: number, pumps: boolean) => run(bal, mm, pumps, NOMINAL, true) as Run;

/** Interpolates a run's series at an hour. */
export function at(r: Run, series: Float64Array, hour: number) {
  const f = Math.min(Math.max(hour, 0), HOURS) / DT;
  const i = Math.min(Math.floor(f), STEPS - 1);
  const u = f - i;
  return series[i]! * (1 - u) + series[i + 1]! * u;
}

/**
 * The same storm many times over, with what is uncertain drawn at random: the forecast depth (lognormal, 22%), the
 * hills' and the beach's runoff coefficients, the stage–storage curve (±4%) and, where the pumps run, a one-in-six
 * chance they stop partway through. Returns each run's peak rise and least beach length.
 */
export function ensemble(bal: Balance, mm: number, pumps: boolean, n: number, seed = 11) {
  const r = rng(seed);
  const gauss = () => Math.sqrt(-2 * Math.log(Math.max(1e-12, r()))) * Math.cos(2 * Math.PI * r());
  const rise = new Float64Array(n);
  const beach = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const k: Knobs = {
      rainScale: Math.exp(0.22 * gauss() - 0.5 * 0.22 * 0.22),
      hillsC: 0.3 + 0.25 * r(),
      beachC: 0.6 + 0.25 * r(),
      areaScale: 1 + 0.04 * gauss(),
      pumpStop: pumps && r() < 1 / 6 ? r() * HOURS : Infinity,
    };
    const o = run(bal, mm, pumps, k, false) as { peakRise: number; minBeach: number };
    rise[i] = o.peakRise;
    beach[i] = o.minBeach;
  }
  return { rise, beach };
}

/**
 * The design storm's rise (pumps stopped), and the least operating freeboard it sets. And the check that stands in
 * for a spillway while the facility operates: the probable maximum precipitation, pumps stopped, from today's level,
 * must still leave the wave allowance and the residual below the crest. If it did not, the facility would need a
 * spillway (or a lower pond) now rather than at closure.
 */
export function designBasis(bal: Balance) {
  const r = simulate(bal, rainFor(CRITERIA.designYears), false);
  const storage = r.peak.rise;
  const pmp = simulate(bal, PMP, false);
  return {
    storage,
    operating: CRITERIA.wave + CRITERIA.residual + storage,
    afterStorm: CRITERIA.wave + CRITERIA.residual,
    pmp: { mm: PMP, freeboard: pmp.peak.freeboard, hour: pmp.peak.hour, stored: pmp.peak.freeboard >= CRITERIA.wave + CRITERIA.residual },
  };
}

/**
 * How far the phreatic surface in section A–A would rise with the pond, an estimate: fed from the pond and the
 * rain soaking the beach, damped towards the core and nothing downstream of the filter.
 */
export function phreaticRise(s: number, rise: number, mm: number) {
  if (s > -4) return 0;
  const reach = Math.min(1, (-4 - s) / 120);
  const taper = reach * reach * (3 - 2 * reach);
  return taper * (0.55 * rise + 0.0011 * mm) + (1 - taper) * 0.12 * rise * Math.min(1, (-4 - s) / 20);
}
