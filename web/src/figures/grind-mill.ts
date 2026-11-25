/**
 * A stylised ball mill, seen end on: a lifter-lined shell turning at a fixed speed, a steel charge that rides up and
 * cascades back to the toe, and ore lumps fed through the trunnion. Each lump shrinks at a rate set by its hardness
 * (soft ore breaks fast, hard ore lingers) and leaves as fines, so the feed rate and the stream out both follow the
 * throughput the engine computed for the state on show. Canvas 2D; no DOM access at module load.
 */
import { clamp, mix, rng } from "./lib";

export interface MillState {
  /** Work index of the ore on show, kWh/t, and its throughput, t/h. */
  wi: number;
  tph: number;
}

interface Ball {
  rho: number;
  u: number;
  /** Fraction of the cycle spent riding up with the shell. */
  rise: number;
  period: number;
  toe: number;
  shoulder: number;
  r: number;
}
interface Lump {
  stage: 0 | 1 | 2 | 3; // 0 chute, 1 falling, 2 in the charge, 3 fines
  x: number;
  y: number;
  vx: number;
  vy: number;
  t: number;
  size: number;
  wi: number;
  ball: Ball;
  shape: number[];
  spin: number;
  turn: number;
}

const TAU = Math.PI * 2;
/** The charge's free surface: the line at distance SURFACE·R from the centre whose normal points at angle NORMAL
 * (measured from straight down, towards the rising side). Each layer rides the shell below it and cascades along it. */
const SURFACE = 0.3;
const NORMAL = 0.42;
const OUTER = 0.955;
const SPIN = 0.95;
/** Lump size, as a fraction of the shell radius: feed size and the size at which it leaves. */
const FEED = 0.052;
const DONE = 0.022;

/** Soft ore is warm sand, hard ore cold slate. */
export function oreColour(wi: number, alpha = 1) {
  const t = clamp((wi - 4) / 26, 0, 1);
  const r = Math.round(mix(232, 92, t));
  const g = Math.round(mix(184, 118, t));
  const b = Math.round(mix(112, 150, t));
  return `rgb(${r} ${g} ${b} / ${alpha})`;
}

export function makeMill(canvas: HTMLCanvasElement) {
  const ctx = canvas.getContext("2d")!;
  const random = rng(17);
  let w = 0;
  let h = 0;
  let ratio = 1;
  let cx = 0;
  let cy = 0;
  let R = 0;
  let angle = 0;
  let feedClock = 0;
  let current: MillState = { wi: 12.5, tph: 128 };
  let shown = 128;
  const balls: Ball[] = [];
  const lumps: Lump[] = [];
  let sprite: HTMLCanvasElement | undefined;

  const makeBall = (): Ball => {
    // Uniform over the area of the charge: rho² uniform between the surface and the shell.
    const rho = Math.sqrt(SURFACE ** 2 + random() * (OUTER ** 2 - SURFACE ** 2));
    const half = Math.acos(Math.min(1, SURFACE / rho));
    const toe = NORMAL - half;
    const shoulder = NORMAL + half;
    const ride = (shoulder - toe) / SPIN;
    const fall = 0.32 + 0.5 * rho * Math.sin(half);
    return { rho, u: random(), rise: ride / (ride + fall), period: ride + fall, toe, shoulder, r: 0.04 + random() * 0.014 };
  };
  for (let i = 0; i < 230; i++) balls.push(makeBall());

  /** Where a body on cycle `b` is at phase u: riding the shell up from the toe, then cascading back down. */
  const place = (b: Ball, u: number): [number, number] => {
    if (u < b.rise) {
      const th = mix(b.toe, b.shoulder, u / b.rise);
      return [cx + b.rho * R * Math.sin(th), cy + b.rho * R * Math.cos(th)];
    }
    const s = (u - b.rise) / (1 - b.rise);
    const x0 = cx + b.rho * R * Math.sin(b.shoulder);
    const y0 = cy + b.rho * R * Math.cos(b.shoulder);
    const x1 = cx + b.rho * R * Math.sin(b.toe);
    const y1 = cy + b.rho * R * Math.cos(b.toe);
    // Outer layers are thrown a little clear of the surface (cataracting); inner ones roll down it (cascading).
    const lift = R * 0.16 * Math.max(0, b.rho - 0.6);
    const qx = mix(x0, x1, 0.5) - Math.sin(NORMAL) * lift;
    const qy = mix(y0, y1, 0.5) - Math.cos(NORMAL) * lift;
    const a = (1 - s) * (1 - s);
    const m = 2 * (1 - s) * s;
    const c = s * s;
    return [a * x0 + m * qx + c * x1, a * y0 + m * qy + c * y1];
  };

  const chute = () => {
    const x0 = cx - R * 1.45;
    const y0 = cy - R * 1.2;
    return { x0, y0, x1: cx - R * 0.08, y1: cy - R * 0.05 };
  };

  const resize = (cw: number, ch: number, r: number) => {
    w = cw;
    h = ch;
    ratio = r;
    R = Math.min(w * 0.35, h * 0.37);
    cx = w * 0.55;
    cy = h * 0.52;
    sprite = document.createElement("canvas");
    const s = Math.ceil(R * 0.13 * ratio) * 2;
    sprite.width = sprite.height = s;
    const g = sprite.getContext("2d")!;
    const grad = g.createRadialGradient(s * 0.36, s * 0.32, s * 0.04, s * 0.5, s * 0.5, s * 0.5);
    grad.addColorStop(0, "#f4f1ec");
    grad.addColorStop(0.35, "#a8a39c");
    grad.addColorStop(0.8, "#4a4541");
    grad.addColorStop(1, "#24201e");
    g.fillStyle = grad;
    g.beginPath();
    g.arc(s / 2, s / 2, s / 2, 0, TAU);
    g.fill();
  };

  const spawn = () => {
    const c = chute();
    const shape: number[] = [];
    const n = 6 + Math.floor(random() * 3);
    for (let i = 0; i < n; i++) shape.push(0.72 + random() * 0.45);
    lumps.push({
      stage: 0,
      x: c.x0,
      y: c.y0,
      vx: 0,
      vy: 0,
      t: 0,
      size: FEED * (0.85 + random() * 0.3),
      wi: current.wi,
      ball: makeBall(),
      shape,
      spin: (random() - 0.5) * 3,
      turn: random() * TAU,
    });
  };

  const rock = (l: Lump, x: number, y: number, size: number, alpha: number) => {
    const n = l.shape.length;
    ctx.beginPath();
    for (let i = 0; i <= n; i++) {
      const a = l.turn + (i / n) * TAU;
      const k = l.shape[i % n]!;
      const px = x + Math.cos(a) * size * R * k;
      const py = y + Math.sin(a) * size * R * k;
      if (i) ctx.lineTo(px, py);
      else ctx.moveTo(px, py);
    }
    ctx.closePath();
    ctx.fillStyle = oreColour(l.wi, alpha);
    ctx.fill();
    ctx.strokeStyle = `rgb(20 14 10 / ${0.55 * alpha})`;
    ctx.lineWidth = 1;
    ctx.stroke();
  };

  const step = (dt: number) => {
    angle += dt * SPIN;
    shown += (current.tph - shown) * Math.min(1, dt * 3);
    // Feed: lumps per second in proportion to throughput.
    feedClock += dt * (shown / 55);
    while (feedClock > 1) {
      feedClock -= 1;
      spawn();
    }
    for (const b of balls) b.u = (b.u + dt / b.period) % 1;
    const c = chute();
    const speed = 0.25 + shown / 420;
    for (let i = lumps.length - 1; i >= 0; i--) {
      const l = lumps[i]!;
      l.turn += l.spin * dt;
      if (l.stage === 0) {
        l.t += dt * speed;
        l.x = mix(c.x0, c.x1, l.t);
        l.y = mix(c.y0, c.y1, l.t);
        if (l.t >= 1) {
          l.stage = 1;
          l.vx = R * 0.25 * speed;
          l.vy = R * 0.2;
        }
      } else if (l.stage === 1) {
        l.vy += R * 3.2 * dt;
        l.x += l.vx * dt;
        l.y += l.vy * dt;
        // Lands on the charge's free surface, then joins the cascade at the nearest point of its own layer.
        const depth = (l.x - cx) * Math.sin(NORMAL) + (l.y - cy) * Math.cos(NORMAL);
        if (depth >= SURFACE * R) {
          l.stage = 2;
          let best = l.ball.rise;
          let near = Infinity;
          for (let k = 0; k <= 16; k++) {
            const u = l.ball.rise + (1 - l.ball.rise) * (k / 16) * 0.999;
            const [px, py] = place(l.ball, u);
            const d = (px - l.x) ** 2 + (py - l.y) ** 2;
            if (d < near) {
              near = d;
              best = u;
            }
          }
          l.ball.u = best;
        }
      } else if (l.stage === 2) {
        l.ball.u = (l.ball.u + dt / l.ball.period) % 1;
        const [x, y] = place(l.ball, l.ball.u);
        l.x = x;
        l.y = y;
        // Bond: the energy to grind a tonne is proportional to Wi, so at fixed power a lump's size falls at 1/Wi.
        l.size -= (dt * 0.13) / l.wi;
        if (l.size <= DONE) {
          l.stage = 3;
          l.t = 0;
        }
      } else {
        l.t += dt * 0.9;
        l.x = mix(l.x, cx, dt * 1.6);
        l.y = mix(l.y, cy, dt * 1.6);
        if (l.t >= 1) lumps.splice(i, 1);
      }
    }
  };

  const draw = () => {
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, w, h);
    // Shell.
    const shell = ctx.createLinearGradient(cx - R, cy - R, cx + R, cy + R);
    shell.addColorStop(0, "#5a524c");
    shell.addColorStop(0.5, "#2c2724");
    shell.addColorStop(1, "#45403b");
    ctx.beginPath();
    ctx.arc(cx, cy, R * 1.1, 0, TAU);
    ctx.fillStyle = shell;
    ctx.fill();
    const inside = ctx.createRadialGradient(cx, cy - R * 0.3, R * 0.1, cx, cy, R);
    inside.addColorStop(0, "#1d1917");
    inside.addColorStop(1, "#0b0908");
    ctx.beginPath();
    ctx.arc(cx, cy, R, 0, TAU);
    ctx.fillStyle = inside;
    ctx.fill();
    // Bolts on the shell, turning with it.
    ctx.fillStyle = "#7a716a";
    for (let k = 0; k < 24; k++) {
      const a = angle + (k / 24) * TAU;
      ctx.beginPath();
      ctx.arc(cx + Math.sin(a) * R * 1.055, cy + Math.cos(a) * R * 1.055, Math.max(1, R * 0.012), 0, TAU);
      ctx.fill();
    }
    // Lifters.
    ctx.fillStyle = "#3a3431";
    for (let k = 0; k < 18; k++) {
      const a = angle + (k / 18) * TAU;
      const half = 0.05;
      ctx.beginPath();
      ctx.moveTo(cx + Math.sin(a - half) * R, cy + Math.cos(a - half) * R);
      ctx.lineTo(cx + Math.sin(a - half * 0.55) * R * 0.9, cy + Math.cos(a - half * 0.55) * R * 0.9);
      ctx.lineTo(cx + Math.sin(a + half * 0.55) * R * 0.9, cy + Math.cos(a + half * 0.55) * R * 0.9);
      ctx.lineTo(cx + Math.sin(a + half) * R, cy + Math.cos(a + half) * R);
      ctx.closePath();
      ctx.fill();
    }
    // Charge, back to front by depth so the surface layer sits on top.
    ctx.save();
    ctx.beginPath();
    ctx.arc(cx, cy, R * 0.985, 0, TAU);
    ctx.clip();
    // The slurry pool in the charge.
    const pool = ctx.createLinearGradient(0, cy + R * 0.1, 0, cy + R);
    pool.addColorStop(0, "rgb(52 211 181 / 0)");
    pool.addColorStop(1, "rgb(52 211 181 / 0.13)");
    ctx.fillStyle = pool;
    ctx.fillRect(cx - R, cy, R * 2, R);
    const sp = sprite;
    for (const b of balls) {
      const [x, y] = place(b, b.u);
      const r = b.r * R;
      if (sp) ctx.drawImage(sp, x - r, y - r, r * 2, r * 2);
    }
    for (const l of lumps) {
      if (l.stage === 2) rock(l, l.x, l.y, l.size, 1);
      else if (l.stage === 1) rock(l, l.x, l.y, l.size, 1);
      else if (l.stage === 3) {
        const a = 1 - l.t;
        ctx.fillStyle = oreColour(l.wi, 0.8 * a);
        for (let k = 0; k < 3; k++) ctx.fillRect(l.x + (k - 1) * 3, l.y + ((k * 7) % 5) - 2, 1.8, 1.8);
      }
    }
    ctx.restore();
    // Rim highlight.
    ctx.beginPath();
    ctx.arc(cx, cy, R * 1.1, 0, TAU);
    ctx.strokeStyle = "rgb(255 224 196 / 0.18)";
    ctx.lineWidth = 1;
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(cx, cy, R, 0, TAU);
    ctx.strokeStyle = "rgb(0 0 0 / 0.6)";
    ctx.lineWidth = 2;
    ctx.stroke();
    // Feed chute, into the trunnion.
    const c = chute();
    ctx.lineCap = "round";
    ctx.strokeStyle = "#2b2623";
    ctx.lineWidth = R * 0.2;
    ctx.beginPath();
    ctx.moveTo(c.x0 - R * 0.2, c.y0 - R * 0.16);
    ctx.lineTo(c.x1, c.y1);
    ctx.stroke();
    ctx.strokeStyle = "#4b433e";
    ctx.lineWidth = R * 0.14;
    ctx.stroke();
    ctx.lineCap = "butt";
    // Trunnion ring.
    ctx.beginPath();
    ctx.arc(cx, cy, R * 0.2, 0, TAU);
    ctx.strokeStyle = "rgb(255 224 196 / 0.14)";
    ctx.lineWidth = R * 0.035;
    ctx.stroke();
    for (const l of lumps) if (l.stage === 0) rock(l, l.x, l.y, l.size, 1);
    // A throughput arc around the shell: full circle at 400 t/h.
    const frac = clamp(shown / 400, 0, 1);
    ctx.beginPath();
    ctx.arc(cx, cy, R * 1.19, -Math.PI / 2, -Math.PI / 2 + frac * TAU);
    ctx.strokeStyle = "#34d3b5";
    ctx.lineWidth = 3;
    ctx.lineCap = "round";
    ctx.stroke();
    ctx.lineCap = "butt";
    ctx.beginPath();
    ctx.arc(cx, cy, R * 1.19, 0, TAU);
    ctx.strokeStyle = "rgb(255 224 196 / 0.08)";
    ctx.lineWidth = 3;
    ctx.stroke();
  };

  // Settle the charge before the first frame so a still picture already looks worked.
  const warm = () => {
    for (let i = 0; i < 240; i++) step(1 / 30);
  };

  return {
    resize,
    set(s: MillState) {
      current = s;
    },
    frame(dt: number) {
      if (!w) return;
      step(dt);
      draw();
    },
    warm() {
      if (w) warm();
    },
    get tph() {
      return shown;
    },
  };
}
