/** Small shared pieces for the figures: a frame loop that sleeps off screen, canvases that fit, and controls. */

// The figures are also imported by tests and by the stills script, where there is no window.
export const reduced = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

export const $ = <T extends Element = HTMLElement>(root: ParentNode, selector: string): T => {
  const el = root.querySelector<T>(selector);
  if (!el) throw Error("Missing " + selector);
  return el;
};
export const $$ = <T extends Element = HTMLElement>(root: ParentNode, selector: string): T[] => [
  ...root.querySelectorAll<T>(selector),
];
export const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
export const mix = (a: number, b: number, t: number) => a + (b - a) * t;
export const smooth = (t: number) => {
  const c = clamp(t, 0, 1);
  return c * c * (3 - 2 * c);
};
/** Minus signs and grouping the way the text around the figures sets them. */
export const fmt = (v: number, digits = 0) =>
  v
    .toLocaleString("en-US", { minimumFractionDigits: digits, maximumFractionDigits: digits })
    .replace("-", "−");

/** Letters with a hat or a dot, which the web font has no glyph for, as markup that draws the mark. */
export const marks = (s: string) =>
  s.replace(
    /([A-Za-z])([\u0302\u0307])/g,
    (_, letter: string, mark: string) => `<span class="over" data-o="${mark === "\u0302" ? "ˆ" : "˙"}">${letter}</span>`,
  );

/** mulberry32: the figures that draw lots are repeatable from their seed. */
/** Runs a builder written as steps (a generator) to the end, all at once. */
export function finish<T>(steps: Generator<unknown, T>): T {
  for (;;) {
    const r = steps.next();
    if (r.done) return r.value;
  }
}

/** Gives the page a moment: input and painting get a turn, then the work goes on at the same priority. */
export const breathe = (): Promise<void> => {
  const s = (globalThis as { scheduler?: { yield?: () => Promise<void> } }).scheduler;
  return s?.yield ? s.yield() : new Promise((res) => setTimeout(res, 0));
};

/** Runs a builder written as steps, pausing for the page whenever a stretch of steps has taken more than `budget` ms,
 *  so a big scene never holds the page in one long task. The steps and their results are the same as `finish`. */
export async function pace<T>(steps: Generator<unknown, T>, budget = 16): Promise<T> {
  let start = performance.now();
  for (;;) {
    const r = steps.next();
    if (r.done) return r.value;
    if (performance.now() - start > budget) {
      await breathe();
      start = performance.now();
    }
  }
}

export function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Runs `fn` the first time half of `el`, or half a screen of it, is in view. */
export function seen(el: Element, fn: () => void) {
  const watch = new IntersectionObserver(
    (list) => {
      const e = list.at(-1)!;
      if (!e.isIntersecting || (e.intersectionRatio < 0.5 && e.intersectionRect.height < innerHeight * 0.5)) return;
      watch.disconnect();
      fn();
    },
    // Fine steps at the low end, so an element several screens tall still reports as it scrolls in.
    { threshold: [0, 0.02, 0.05, 0.1, 0.15, 0.2, 0.25, 0.5, 0.75, 1] },
  );
  watch.observe(el);
}

export interface Loop {
  running: boolean;
  /** Draws one frame soon, whether or not the loop is running. */
  poke(): void;
}

/** Calls draw on every frame while `root` is on screen and the page is visible. */
export function loop(root: Element, draw: (dt: number, now: number) => void, running = !reduced): Loop {
  let raf = 0;
  let last = 0;
  let shown = false;
  let once = false;
  const tick = (now: number) => {
    raf = 0;
    once = false;
    const dt = Math.min(0.05, Math.max(0, (now - last) / 1000));
    last = now;
    draw(dt, now);
    if (running) wake();
  };
  const wake = () => {
    if (raf || document.hidden || !shown || !(running || once)) return;
    if (!last || performance.now() - last > 100) last = performance.now() - 16;
    raf = requestAnimationFrame(tick);
  };
  new IntersectionObserver(
    ([entry]) => {
      shown = entry!.isIntersecting;
      if (!shown && raf) {
        cancelAnimationFrame(raf);
        raf = 0;
      }
      wake();
    },
    { rootMargin: "60px" },
  ).observe(root);
  document.addEventListener("visibilitychange", wake);
  return {
    get running() {
      return running;
    },
    set running(v) {
      running = v;
      wake();
    },
    poke() {
      once = true;
      wake();
    },
  };
}

/** Keeps a canvas at its displayed size. Calls back with CSS pixels after every change. */
export function fit(
  canvas: HTMLCanvasElement,
  onSize: (w: number, h: number, ratio: number) => void,
  maxRatio = 2,
) {
  const apply = () => {
    const { clientWidth: w, clientHeight: h } = canvas;
    if (!w || !h) return;
    const ratio = Math.min(maxRatio, devicePixelRatio || 1);
    const pw = Math.round(w * ratio);
    const ph = Math.round(h * ratio);
    if (canvas.width !== pw || canvas.height !== ph) {
      canvas.width = pw;
      canvas.height = ph;
    }
    onSize(w, h, ratio);
  };
  new ResizeObserver(apply).observe(canvas);
  apply();
}

/** Calls back with the width of an element, so that drawn text keeps its size on a phone. */
export function onWidth(el: Element, draw: (w: number) => void) {
  let last = 0;
  const apply = () => {
    const w = Math.max(280, Math.round(el.clientWidth));
    if (w === last) return;
    last = w;
    draw(w);
  };
  new ResizeObserver(apply).observe(el);
  apply();
}

export interface Slider {
  value: number;
  set(v: number): void;
}
/** Binds a range control: keeps its fill and its readout in step with its value. */
export function range(
  root: ParentNode,
  name: string,
  shown: (v: number) => string,
  onInput: (v: number) => void,
): Slider {
  const label = $(root, `[data-range="${name}"]`);
  const input = $<HTMLInputElement>(label, "input");
  const output = $<HTMLOutputElement>(label, "output");
  const paint = () => {
    const lo = Number(input.min);
    const hi = Number(input.max);
    const v = Number(input.value);
    input.style.setProperty("--p", `${((v - lo) / (hi - lo)) * 100}%`);
    const text = shown(slider.value);
    output.textContent = text;
    input.setAttribute("aria-valuetext", text);
  };
  const slider: Slider = {
    value: Number(input.value),
    /** The readout keeps exact values that the slider's own steps cannot hold. */
    set(v) {
      slider.value = v;
      input.value = String(v);
      paint();
    },
  };
  input.addEventListener("input", () => {
    slider.value = Number(input.value);
    paint();
    onInput(slider.value);
  });
  paint();
  return slider;
}

export function seg(root: ParentNode, name: string, onPick: (v: string) => void) {
  const group = $(root, `[data-seg="${name}"]`);
  const buttons = $$<HTMLButtonElement>(group, "button");
  const state = {
    value: buttons.find((b) => b.getAttribute("aria-pressed") === "true")?.dataset.v ?? "",
    set(v: string) {
      state.value = v;
      for (const b of buttons) b.setAttribute("aria-pressed", String(b.dataset.v === v));
    },
  };
  group.addEventListener("click", (event) => {
    const button = (event.target as Element).closest<HTMLButtonElement>("button");
    if (!button) return;
    state.set(button.dataset.v!);
    onPick(state.value);
  });
  return state;
}

export function toggle(root: ParentNode, name: string, onChange: (on: boolean) => void) {
  const button = $(root, `[data-toggle="${name}"]`);
  const state = { on: button.getAttribute("aria-pressed") === "true" };
  button.addEventListener("click", () => {
    state.on = !state.on;
    button.setAttribute("aria-pressed", String(state.on));
    onChange(state.on);
  });
  return state;
}

export const line = (pts: [number, number][]) =>
  pts.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(2)} ${y.toFixed(2)}`).join("");

/** Exact fractions, for the figures whose answers are exact. */
export class Q {
  readonly n: bigint;
  readonly d: bigint;
  constructor(n: bigint | number, d: bigint | number = 1n) {
    let a = BigInt(n);
    let b = BigInt(d);
    if (b === 0n) throw Error("Division by zero");
    if (b < 0n) [a, b] = [-a, -b];
    let x = a < 0n ? -a : a;
    let y = b;
    while (y) [x, y] = [y, x % y];
    const g = x || 1n;
    this.n = a / g;
    this.d = b / g;
  }
  add = (o: Q) => new Q(this.n * o.d + o.n * this.d, this.d * o.d);
  sub = (o: Q) => new Q(this.n * o.d - o.n * this.d, this.d * o.d);
  mul = (o: Q) => new Q(this.n * o.n, this.d * o.d);
  div = (o: Q) => new Q(this.n * o.d, this.d * o.n);
  cmp = (o: Q) => {
    const v = this.n * o.d - o.n * this.d;
    return v < 0n ? -1 : v > 0n ? 1 : 0;
  };
  get value() {
    return Number(this.n) / Number(this.d);
  }
  toString() {
    const s = this.d === 1n ? `${this.n}` : `${this.n}/${this.d}`;
    return s.replace("-", "−");
  }
}
export const q = (n: number | bigint, d: number | bigint = 1) => new Q(n, d);

/**
 * Holds each element at the tallest height the figure's states give it, so stepping through them never moves what
 * lies below. Safari keeps no scroll anchor: anything above the reader that changes height moves what they are
 * reading. `keep` notes the current state and returns a function that draws it again; `draw(i)` draws state i of `n`.
 * All of it runs in one task, so no state but the current one is ever painted. Measured again when the width changes;
 * while `when` does not match, the elements size to their content.
 */
export function tallest(els: HTMLElement[], n: number, keep: () => () => void, draw: (i: number) => void, when?: MediaQueryList) {
  let width = -1;
  const fit = () => {
    width = els[0]!.clientWidth;
    for (const el of els) el.style.minHeight = "";
    if (when && !when.matches) return;
    const back = keep();
    const max = els.map(() => 0);
    for (let i = 0; i < n; i++) {
      draw(i);
      els.forEach((el, k) => (max[k] = Math.max(max[k]!, el.offsetHeight)));
    }
    back();
    els.forEach((el, k) => (el.style.minHeight = `${max[k]}px`));
  };
  fit();
  new ResizeObserver(() => {
    if (els[0]!.clientWidth !== width) fit();
  }).observe(els[0]!);
  when?.addEventListener("change", fit);
}
