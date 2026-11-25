/**
 * Section A–A′ as a drawing, at true scale (one SVG unit per metre, no vertical exaggeration): the row of blocks
 * behind the section coloured by grade or value, the block set's stepped outline, the natural surface, each nested
 * shell's benched profile, the shown pit's benched profile, and the dimensions an engineer reads off a section:
 * crest to crest, depth to the floor, and the overall slope each wall stands at on this section. A bench detail sits
 * beside it at four times the scale.
 */
import * as THREE from "three";
import { fmt } from "./lib";
import { oreColour, SHELL_COLOURS, valueColour } from "./pit-blocks";
import { B, CUT_X0, CUT_X1, CUT_Y, H } from "./pit-model";

export interface SheetBlock {
  /** Block index across the section (0 at A) and down (0 at the surface). */
  i: number;
  k: number;
  grade: number;
  ore: boolean;
  inSet: boolean;
  value: number;
}

export interface SheetData {
  xs: Float32Array;
  ground: Float32Array;
  /** The benched profile of the set drawn as ground, then of each shell, inner first. */
  design: Float32Array;
  shells: Float32Array[];
  shown: number;
  blocks: SheetBlock[];
  by: "grade" | "value";
  slope: number;
  berm: number;
  face: number;
}

const W = 760;
const L = 26;
const T = 50;
const DEPTH = -CUT_Y;
const HH = T + DEPTH + 40;
const X = (x: number) => L + (x - CUT_X0);
const Y = (y: number) => T - y;
const f1 = (v: number) => v.toFixed(1);

const arrow = (x: number, y: number, dx: number, dy: number) => {
  const l = Math.hypot(dx, dy) || 1;
  const ux = dx / l;
  const uy = dy / l;
  const s = 6;
  const w = 2.6;
  return `M${f1(x)} ${f1(y)}L${f1(x - ux * s - uy * w)} ${f1(y - uy * s + ux * w)}L${f1(x - ux * s + uy * w)} ${f1(y - uy * s - ux * w)}Z`;
};

/** The section drawing's SVG markup (for a `viewBox="0 0 760 390"` element). */
export function sheet(d: SheetData): string {
  const c = new THREE.Color();
  const hex = () => `#${c.getHexString()}`;
  const parts: string[] = [];
  parts.push(`<rect class="bg" x="${L}" y="${T}" width="${CUT_X1 - CUT_X0}" height="${DEPTH}"/>`);

  // Blocks: ore filled by grade or value (faint where the set leaves it), waste in the set barely tinted.
  const inSet = new Set<number>();
  const key = (i: number, k: number) => i * 1000 + k;
  for (const b of d.blocks) {
    if (b.inSet) inSet.add(key(b.i, b.k));
    const x = L + b.i * B;
    const y = T + b.k * H;
    if (b.ore) {
      if (d.by === "value") valueColour(b.value, c);
      else oreColour(Math.max(b.grade, 0.15), c);
      c.convertLinearToSRGB();
      parts.push(`<rect x="${x}" y="${y}" width="${B}" height="${H}" fill="${hex()}" opacity="${b.inSet ? 0.95 : 0.38}"/>`);
    } else if (b.inSet) parts.push(`<rect class="wi" x="${x}" y="${y}" width="${B}" height="${H}"/>`);
  }
  // The block grid.
  let grid = "";
  for (let x = 0; x <= CUT_X1 - CUT_X0; x += B) grid += `M${L + x} ${T}V${T + DEPTH}`;
  for (let y = 0; y <= DEPTH; y += H) grid += `M${L} ${T + y}H${L + CUT_X1 - CUT_X0}`;
  parts.push(`<path class="grid" d="${grid}"/>`);

  // The block set's stepped outline along this row.
  let edge = "";
  const cols = (CUT_X1 - CUT_X0) / B;
  for (const b of d.blocks) {
    if (!b.inSet) continue;
    const x = L + b.i * B;
    const y = T + b.k * H;
    const has = (i: number, k: number) => i >= 0 && i < cols && k >= 0 && inSet.has(key(i, k));
    if (!has(b.i - 1, b.k)) edge += `M${x} ${y}V${y + H}`;
    if (!has(b.i + 1, b.k)) edge += `M${x + B} ${y}V${y + H}`;
    if (!has(b.i, b.k + 1)) edge += `M${x} ${y + H}H${x + B}`;
  }
  parts.push(`<path class="blk" d="${edge}"/>`);

  // Profiles: the natural surface, each shell, then the shown pit's benches.
  const poly = (ys: Float32Array, cls: string, style = "") => {
    let p = "";
    for (let s = 0; s < d.xs.length; s++) p += `${s ? "L" : "M"}${f1(X(d.xs[s]!))} ${f1(Y(ys[s]!))}`;
    return `<path class="${cls}" d="${p}"${style ? ` style="${style}"` : ""}/>`;
  };
  d.shells.forEach((sh, s) => {
    if (s === d.shown) return;
    parts.push(poly(sh, "shell", `stroke:${SHELL_COLOURS[s]}`));
  });
  parts.push(poly(d.ground, "nat"));
  parts.push(poly(d.design, "design"));

  // Dimensions, read off the shown profile.
  const n = d.xs.length;
  let a = -1;
  let z = -1;
  let deep = 0;
  let deepAt = 0;
  for (let s = 0; s < n; s++) {
    const cut = d.ground[s]! - d.design[s]!;
    if (cut > 0.5) {
      if (a < 0) a = s;
      z = s;
    }
    if (cut > deep) {
      deep = cut;
      deepAt = s;
    }
  }
  const dims: string[] = [];
  const text: string[] = [];
  if (a >= 0 && z > a) {
    const xa = d.xs[a]!;
    const xz = d.xs[z]!;
    const floorY = d.design[deepAt]!;
    // Crest to crest, above the surface.
    const yd = T - 26;
    dims.push(`M${f1(X(xa))} ${f1(Y(d.ground[a]!) - 4)}V${yd - 5}M${f1(X(xz))} ${f1(Y(d.ground[z]!) - 4)}V${yd - 5}`);
    dims.push(`M${f1(X(xa))} ${yd}H${f1(X(xz))}`);
    const heads = [arrow(X(xa), yd, -1, 0), arrow(X(xz), yd, 1, 0)];
    text.push(`<text class="dim" x="${f1((X(xa) + X(xz)) / 2)}" y="${yd - 6}">Crest to crest ${fmt(xz - xa)} m</text>`);
    // Depth: beside the deeper wall's far side, from the surface to the floor.
    const right = X(xz) + 70 < L + (CUT_X1 - CUT_X0);
    const xd = right ? X(xz) + 22 : X(xa) - 22;
    const yTop = Y(Math.max(d.ground[a]!, d.ground[z]!));
    const yBot = Y(floorY);
    dims.push(`M${f1(X(d.xs[deepAt]!))} ${f1(yBot)}H${f1(xd + (right ? 5 : -5))}`);
    dims.push(`M${f1(xd)} ${f1(yTop)}V${f1(yBot)}`);
    heads.push(arrow(xd, yTop, 0, -1), arrow(xd, yBot, 0, 1));
    text.push(
      `<text class="dim ${right ? "l" : "r"}" x="${f1(xd + (right ? 7 : -7))}" y="${f1((yTop + yBot) / 2 + 4)}">${fmt(deep)} m</text>`,
      `<text class="dim sub ${right ? "l" : "r"}" x="${f1(xd + (right ? 7 : -7))}" y="${f1((yTop + yBot) / 2 + 17)}">${fmt(Math.round(deep / H))} benches</text>`,
    );
    // Overall slope of each wall on this section: crest to the floor's toe.
    let ta = deepAt;
    while (ta > a && d.design[ta - 1]! <= floorY + 0.5) ta--;
    let tz = deepAt;
    while (tz < z && d.design[tz + 1]! <= floorY + 0.5) tz++;
    const wall = (xc: number, yc: number, xt: number, left: boolean) => {
      const run = Math.abs(xt - xc);
      if (run < 5) return;
      const ang = (Math.atan2(yc - floorY, run) * 180) / Math.PI;
      dims.push(`M${f1(X(xc))} ${f1(Y(yc))}L${f1(X(xt))} ${f1(Y(floorY))}`);
      // The angle's arc at the crest, from the surface down to the slope line, and its value beside it.
      const r = 30;
      const th = (ang * Math.PI) / 180;
      const sx = left ? 1 : -1;
      const x0 = X(xc) + sx * r;
      const y0 = Y(yc);
      const x1 = X(xc) + sx * r * Math.cos(th);
      const y1 = Y(yc) + r * Math.sin(th);
      dims.push(`M${f1(x0)} ${f1(y0)}A${r} ${r} 0 0 ${left ? 1 : 0} ${f1(x1)} ${f1(y1)}`);
      const lx = X(xc) + sx * (r + 6) * Math.cos(th / 2);
      const ly = Y(yc) + (r + 6) * Math.sin(th / 2) + 4;
      text.push(`<text class="dim halo ${left ? "l" : "r"}" x="${f1(lx)}" y="${f1(ly)}">${fmt(ang, 0)}°</text>`);
    };
    wall(xa, d.ground[a]!, d.xs[ta]!, true);
    wall(xz, d.ground[z]!, d.xs[tz]!, false);
    parts.push(`<path class="dimln" d="${dims.join("")}"/>`, `<path class="head" d="${heads.join("")}"/>`);
  }
  parts.push(...text);

  // Ends of the section and a scale bar.
  parts.push(`<text class="end" x="${L - 4}" y="${T - 34}">A</text><text class="end" x="${L + (CUT_X1 - CUT_X0) + 4}" y="${T - 34}" text-anchor="end">A′</text>`);
  const sb = T + DEPTH + 18;
  parts.push(
    `<path class="scale" d="M${L} ${sb}h100M${L} ${sb - 4}v8M${L + 50} ${sb - 3}v6M${L + 100} ${sb - 4}v8"/>`,
    `<text class="tick" x="${L}" y="${sb + 16}">0</text><text class="tick" x="${L + 50}" y="${sb + 16}">50</text><text class="tick" x="${L + 100}" y="${sb + 16}">100 m</text>`,
    `<text class="tick r" x="${L + (CUT_X1 - CUT_X0)}" y="${sb + 4}">${B} × ${H} m blocks · 1:1, no vertical exaggeration</text>`,
  );
  return parts.join("");
}

export const SHEET_VIEWBOX = `0 0 ${W} ${HH}`;

/** Two benches at four times the scale, with their dimensions: bench height, berm width, face angle, overall slope. */
export function benchDetail(slope: number, berm: number, face: number): string {
  const k = 4;
  const run = H / Math.tan((face * Math.PI) / 180);
  const m = 14;
  const pts: [number, number][] = [
    [0, 0],
    [berm, 0],
    [berm + run, H],
    [2 * berm + run, H],
    [2 * berm + 2 * run, 2 * H],
    [3 * berm + 2 * run, 2 * H],
  ];
  const px = (x: number) => m + x * k;
  const py = (y: number) => m + 8 + y * k;
  const P = (p: [number, number]) => `${f1(px(p[0]))} ${f1(py(p[1]))}`;
  const end = pts[5]![0];
  const w = Math.max(px(end) + 52, px(pts[3]![0]) + 76);
  const h = py(2 * H) + 30;
  const rock = `M${P(pts[0]!)}${pts.slice(1).map((p) => `L${P(p)}`).join("")}L${f1(px(end))} ${f1(h - 4)}L${m} ${f1(h - 4)}Z`;
  // Bench height beside the lower bench; berm width on the middle berm; the face angle at the lower toe.
  const xh = px(end) + 10;
  const yb = py(H) - 6;
  const toe = pts[4]!;
  const r = 18;
  const th = (face * Math.PI) / 180;
  const arc = `M${f1(px(toe[0]) + r)} ${f1(py(toe[1]))}A${r} ${r} 0 0 0 ${f1(px(toe[0]) - r * Math.cos(th))} ${f1(py(toe[1]) - r * Math.sin(th))}`;
  return `<svg class="pit-bench" viewBox="0 0 ${f1(w)} ${f1(h)}" role="img" aria-label="Bench detail: ${H} m benches, ${face}° faces, ${fmt(berm, 1)} m berms, ${slope}° overall">
<path class="rk" d="${rock}"/>
<path class="ov" d="M${P(pts[1]!)}L${P(pts[4]!)}"/>
<path class="dimln" d="M${f1(px(end) + 3)} ${f1(py(H))}H${f1(xh + 4)}M${f1(px(end) + 3)} ${f1(py(2 * H))}H${f1(xh + 4)}M${f1(xh)} ${f1(py(H))}V${f1(py(2 * H))}M${f1(px(pts[2]![0]))} ${f1(yb)}H${f1(px(pts[3]![0]))}M${f1(px(pts[2]![0]))} ${f1(yb - 3)}v6M${f1(px(pts[3]![0]))} ${f1(yb - 3)}v6${arc}"/>
<text class="dim l" x="${f1(xh + 4)}" y="${f1(py(1.5 * H) + 4)}">${H} m</text>
<text class="dim l" x="${f1(px(pts[3]![0]) + 5)}" y="${f1(yb + 2)}">berm ${fmt(berm, 1)} m</text>
<text class="dim l" x="${f1(px(toe[0]) + r + 3)}" y="${f1(py(toe[1]) - 5)}">${face}°</text>
<text class="dim sub l" x="${f1(px(pts[3]![0]) + 5)}" y="${f1(py(0) + 12)}">${slope}° overall</text>
</svg>`;
}
