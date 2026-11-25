/**
 * Section A–A: an x-ray slice through the embankment at its tallest, drawn as an engineer draws a section and stood
 * in the scene where it cuts the dam. It shows the foundation, the starter dam and the three raises with their core,
 * filter and drains, the tailings behind them, the phreatic surface, the boreholes with each piezometer's tip, the
 * head of water it reads and its trigger levels, the inclinometers, and the freeboard as a dimension: the crest's low
 * point, the wave and residual allowance, the design storm's storage, and the pond's level projected onto the section.
 *
 * The pond line, the freeboard dimension and the phreatic surface a storm would raise are live: `update` moves them.
 */
import * as THREE from "three";
import { clamp } from "./lib";
import { CRITERIA, phreaticRise } from "./tailings-balance";
import {
  CREST_LOW,
  crestZ,
  DAM,
  damTop,
  HOLES,
  INCLINOMETERS,
  natural,
  PHREATIC,
  PIEZOMETERS,
  POND,
  SECTION_X,
  settle,
  tailings,
} from "./tailings-model";

export const S0 = -128;
export const S1 = 128;
export const Y0 = -18;
export const Y1 = 40;

type Zone = 0 | 1 | 2 | 3 | 4 | 5 | 6;
const FOUND: Zone = 0;
const TAIL: Zone = 1;
const CORE: Zone = 2;
const FILTER: Zone = 3;
const DRAIN: Zone = 4;
const SHELL_UP: Zone = 5;
const SHELL: Zone = 6;

const RAISES = DAM.raises;
const coreHalf = (y: number) => 4 + 3 * (1 - clamp(y / 36, 0, 1));
const dsFace = (k: number, y: number) => DAM.crestHalf + DAM.down * (RAISES[k]!.top - y);
const usFace = (k: number, y: number) => -DAM.crestHalf - DAM.up * (RAISES[k]!.top - y);

/** Which body (starter 0, raises 1–3) a point inside the embankment belongs to, or −1. */
function body(s: number, y: number, top: number) {
  for (let k = 0; k < RAISES.length; k++) {
    const r = RAISES[k]!;
    if (y > r.top || s < usFace(k, y)) continue;
    const last = k === RAISES.length - 1;
    if (last ? s > DAM.crestHalf && y > top : s > dsFace(k, y)) continue;
    const beach = k === 0 ? -Infinity : RAISES[k - 1]!.top - 0.5;
    if (y < beach && s < dsFace(k - 1, y)) continue;
    return k;
  }
  return -1;
}

export const phreaticAt = (s: number) => {
  for (let i = 0; i < PHREATIC.length - 1; i++) {
    const [a, ya] = PHREATIC[i]!;
    const [b, yb] = PHREATIC[i + 1]!;
    if (s >= a && s <= b) return ya + ((s - a) / (b - a)) * (yb - ya);
  }
  return s < PHREATIC[0]![0] ? PHREATIC[0]![1] : PHREATIC[PHREATIC.length - 1]![1];
};

const FILL: Record<Zone, [number, number, number, number]> = {
  0: [78, 62, 50, 236],
  1: [122, 126, 128, 226],
  2: [170, 110, 62, 238],
  3: [222, 194, 112, 238],
  4: [186, 172, 136, 238],
  5: [146, 142, 132, 232],
  6: [124, 117, 107, 232],
};

export interface Section {
  /** The slice, and the live overlay drawn over it. */
  group: THREE.Group;
  /** World position of each piezometer's tip and each borehole's top. */
  tips: Record<string, THREE.Vector3>;
  /** Where the freeboard dimension's label sits, for a given pond level. */
  dimAt(level: number): THREE.Vector3;
  /** Moves the pond line and the dimension to a level, and draws the phreatic surface a storm would raise. */
  update(level: number, rain: number, scale: number): void;
}

const MONO = "ui-monospace, Menlo, monospace";

/** Draws the slice into a canvas, in chunks of columns so no frame stalls. */
export async function makeSection(o: { small: boolean; storage: number; pause: () => Promise<void> }): Promise<Section> {
  const W = o.small ? 1400 : 2600;
  const H = Math.round((W * (Y1 - Y0)) / (S1 - S0));
  const cv = document.createElement("canvas");
  cv.width = W;
  cv.height = H;
  const g = cv.getContext("2d")!;
  // The canvas runs downstream to upstream, so that seen from the camera's side it reads left to right.
  const px = (s: number) => ((S1 - s) / (S1 - S0)) * W;
  const py = (y: number) => ((Y1 - y) / (Y1 - Y0)) * H;
  const sAt = (i: number) => S1 - ((i + 0.5) / W) * (S1 - S0);
  const yAt = (j: number) => Y1 - ((j + 0.5) / H) * (Y1 - Y0);
  const zc = crestZ(SECTION_X);
  const sett = settle(SECTION_X);

  const gs = new Float32Array(W);
  const ds = new Float32Array(W);
  const top = new Float32Array(W);
  for (let i = 0; i < W; i++) {
    const s = sAt(i);
    gs[i] = natural(SECTION_X, zc + s);
    ds[i] = damTop(s) - sett;
    top[i] = Math.max(gs[i]!, ds[i]!, tailings(SECTION_X, zc + s));
  }
  const img = g.createImageData(W, H);
  const d = img.data;
  for (let i = 0; i < W; i++) {
    const s = sAt(i);
    const phr = phreaticAt(s);
    for (let j = 0; j < H; j++) {
      const y = yAt(j);
      if (y > top[i]!) continue;
      let z: Zone;
      let k = -1;
      if (y < gs[i]!) z = Math.abs(s) < coreHalf(0) - 1 && y > gs[i]! - 4 ? CORE : FOUND;
      else {
        k = body(s, y, ds[i]!);
        if (k < 0) z = TAIL;
        else {
          const ch = coreHalf(y);
          if (Math.abs(s) <= ch) z = CORE;
          else if (s > ch && y < 2) z = DRAIN;
          else if (s > ch && s <= ch + 3) z = FILTER;
          else z = s < 0 ? SHELL_UP : SHELL;
        }
      }
      let [r, gg, b, a] = FILL[z];
      if (z === SHELL) {
        const lift = k * 9;
        r += lift;
        gg += lift;
        b += lift;
      }
      const hash = Math.sin(i * 12.9898 + j * 78.233) * 43758.5453;
      const n = hash - Math.floor(hash);
      if (z === TAIL) {
        const lam = Math.sin(y * 7.5 + Math.sin(s * 0.05) * 1.2);
        r += lam * 6;
        gg += lam * 6;
        b += lam * 6;
      } else if (z === FOUND) {
        if ((i + j) % 22 < 1.6) {
          r -= 22;
          gg -= 20;
          b -= 18;
        }
      } else if (z === SHELL || z === SHELL_UP || z === DRAIN) {
        const v = (n - 0.5) * 26;
        r += v;
        gg += v;
        b += v;
      }
      if (y < phr && z !== FOUND) {
        r = r * 0.8 + 18;
        gg = gg * 0.84 + 40;
        b = b * 0.84 + 66;
      }
      const o4 = (j * W + i) * 4;
      d[o4] = r;
      d[o4 + 1] = gg;
      d[o4 + 2] = b;
      d[o4 + 3] = a;
    }
    if (i % 400 === 399) await o.pause();
  }
  g.putImageData(img, 0, 0);
  await o.pause();

  const S = W / (S1 - S0);
  const lw = (m: number) => Math.max(1, m * S);
  const line = (pts: [number, number][], width: number, style: string, dash: number[] = []) => {
    g.beginPath();
    pts.forEach(([s, y], i) => (i ? g.lineTo(px(s), py(y)) : g.moveTo(px(s), py(y))));
    g.setLineDash(dash);
    g.lineWidth = width;
    g.strokeStyle = style;
    g.stroke();
    g.setLineDash([]);
  };
  const text = (t: string, s: number, y: number, size: number, colour = "rgba(255,255,255,0.82)", align: CanvasTextAlign = "center", weight = 600) => {
    g.font = `${weight} ${Math.round(lw(size))}px ${MONO}`;
    g.textAlign = align;
    g.fillStyle = colour;
    g.fillText(t, px(s), py(y));
  };

  // The outlines of the starter dam and the raises inside the embankment, and the beaches they were built on.
  for (let k = 0; k < RAISES.length - 1; k++) {
    const r = RAISES[k]!;
    const beach = k === 0 ? 0 : RAISES[k - 1]!.top - 0.5;
    line(
      [
        [usFace(k, beach), beach],
        [-DAM.crestHalf, r.top],
        [DAM.crestHalf, r.top],
        [dsFace(k, 0), 0],
      ],
      lw(0.3),
      "rgba(255,255,255,0.5)",
      [lw(1.2), lw(0.8)],
    );
    line(
      [
        [S0, r.top - 0.5],
        [usFace(k, r.top - 0.5) - 0.01, r.top - 0.5],
      ],
      lw(0.16),
      "rgba(255,255,255,0.26)",
      [lw(2), lw(1.4)],
    );
  }
  const ground: [number, number][] = [];
  for (let i = 0; i < W; i += 4) ground.push([sAt(i), gs[i]!]);
  line(ground, lw(0.24), "rgba(255,236,210,0.55)", [lw(1.5), lw(1)]);

  // The freeboard criteria, hung from the crest's low point: wave and residual, then the design storm's storage.
  const crest = CREST_LOW;
  const fbS0 = S0 + 2;
  const fbS1 = -DAM.crestHalf;
  const band = (y0: number, y1: number, fill: string) => {
    g.fillStyle = fill;
    g.fillRect(px(fbS1), py(y0), px(fbS0) - px(fbS1), py(y1) - py(y0));
  };
  const allow = CRITERIA.wave + CRITERIA.residual;
  band(crest, crest - allow, "rgba(251,113,133,0.2)");
  band(crest - allow, crest - allow - o.storage, "rgba(250,204,21,0.14)");
  const level = (y: number, style: string, dash: number[] = []) =>
    line(
      [
        [fbS0, y],
        [fbS1, y],
      ],
      lw(0.18),
      style,
      dash,
    );
  level(crest, "rgba(255,255,255,0.95)");
  level(crest - allow, "rgba(251,113,133,0.9)", [lw(0.9), lw(0.6)]);
  level(crest - allow - o.storage, "rgba(250,204,21,0.9)", [lw(0.9), lw(0.6)]);
  text(`CREST LOW POINT RL ${(1000 + crest).toFixed(2)} · P-07`, -16, crest + 0.7, 1.2, "rgba(255,255,255,0.95)", "right");
  text(`WAVE + RESIDUAL ${allow.toFixed(2)} m`, -16, crest - allow * 0.5 - 0.3, 1.0, "rgba(255,190,200,0.95)", "right");
  text(`DESIGN STORM STORAGE ${o.storage.toFixed(2)} m · 1:${CRITERIA.designYears} AEP 72 h`, -16, crest - allow - o.storage * 0.5 - 0.3, 1.0, "rgba(253,230,138,0.95)", "right");

  // The phreatic surface today, glowing.
  g.shadowColor = "rgba(97,201,255,0.9)";
  g.shadowBlur = lw(1.2);
  line(
    PHREATIC.map(([s, y]) => [s, y]),
    lw(0.5),
    "#7fd4ff",
  );
  g.shadowBlur = 0;
  const surf: [number, number][] = [];
  for (let i = 0; i < W; i += 2) surf.push([sAt(i), top[i]!]);
  line(surf, lw(0.42), "rgba(255,255,255,0.95)");

  // Boreholes and the piezometers in them, with each tip's head and its trigger levels.
  const tips: Record<string, THREE.Vector3> = {};
  for (const [hole, s] of Object.entries(HOLES)) {
    const deepest = Math.min(...PIEZOMETERS.filter((p) => p.hole === hole).map((p) => p.y));
    const surfY = top[clamp(Math.round(((S1 - s) / (S1 - S0)) * W), 0, W - 1)]!;
    line(
      [
        [s, surfY],
        [s, deepest - 0.6],
      ],
      lw(0.2),
      "rgba(255,255,255,0.85)",
      [lw(0.9), lw(0.5)],
    );
    text(hole, s, surfY + 1.5, 1.0, "rgba(255,255,255,0.9)");
    tips[hole] = new THREE.Vector3(SECTION_X, surfY, zc + s);
  }
  for (const p of PIEZOMETERS) {
    const x = px(p.s);
    const y = py(p.y);
    const head = py(p.head);
    const col = p.stale ? "#f2913a" : "#7fd4ff";
    const cx = x + lw(0.9);
    const cw = lw(0.7);
    g.fillStyle = p.stale ? "rgba(242,145,58,0.22)" : "rgba(127,212,255,0.55)";
    g.fillRect(cx, head, cw, y - head);
    g.strokeStyle = col;
    g.lineWidth = lw(0.14);
    g.setLineDash(p.stale ? [lw(0.5), lw(0.4)] : []);
    g.strokeRect(cx, head, cw, y - head);
    g.setLineDash([]);
    // Trigger levels: L1 amber, L2 red, as ticks across the column.
    for (const [lvl, c] of [
      [p.l1, "#facc15"],
      [p.l2, "#fb7185"],
    ] as const) {
      g.fillStyle = c;
      g.fillRect(cx - lw(0.35), py(lvl) - lw(0.09), cw + lw(0.7), lw(0.18));
    }
    g.beginPath();
    g.moveTo(cx - lw(0.3), head - lw(0.85));
    g.lineTo(cx + cw + lw(0.3), head - lw(0.85));
    g.lineTo(cx + cw / 2, head);
    g.closePath();
    g.fillStyle = col;
    g.fill();
    g.beginPath();
    g.arc(x, y, lw(0.8), 0, Math.PI * 2);
    g.fillStyle = p.stale ? "rgba(20,16,12,0.9)" : "#5fd38d";
    g.fill();
    g.lineWidth = lw(0.26);
    g.strokeStyle = p.stale ? "#f2913a" : "#ffffff";
    g.stroke();
    if (p.stale) {
      g.font = `700 ${Math.round(lw(1.5))}px ${MONO}`;
      g.fillStyle = "#f2913a";
      g.textAlign = "center";
      g.fillText("?", cx + cw / 2, head - lw(1.5));
    }
    tips[p.id] = new THREE.Vector3(SECTION_X, p.y, zc + p.s);
  }
  // Inclinometers: casings through the berms into the foundation.
  for (const inc of INCLINOMETERS) {
    const surfY = damTop(inc.s) - sett;
    const s = inc.s + 1.6;
    line(
      [
        [s, surfY],
        [s, inc.bottom],
      ],
      lw(0.24),
      "rgba(196,181,253,0.95)",
    );
    for (let y = surfY - 2; y > inc.bottom; y -= 3)
      line(
        [
          [s - 0.5, y],
          [s + 0.5, y],
        ],
        lw(0.12),
        "rgba(196,181,253,0.8)",
      );
    text(inc.id, s, inc.bottom - 1.4, 0.95, "rgba(221,214,254,0.95)");
  }

  text("TAILINGS", -95, 24, 1.9);
  text("FOUNDATION", -80, -11, 1.9, "rgba(255,236,210,0.7)");
  text("CORE", 0, 27, 1.5);
  text("FILTER", 9.2, 33.2, 1.1, "rgba(60,44,10,0.9)", "left");
  text("DRAIN", 92, 0.35, 1.2, "rgba(50,40,20,0.85)");
  RAISES.forEach((r, k) => {
    const s = k === 0 ? 20 : k === 1 ? 37 : k === 2 ? 57 : 82;
    text(k === 0 ? "STARTER" : `RAISE ${k}`, s, 7.2, 1.25);
    text(String(r.year), s, 4.6, 1.1, "rgba(255,255,255,0.6)");
  });
  text("PHREATIC SURFACE TODAY", -70, 31.0, 1.15, "#9adfff");
  text("TARP L1", S1 - 3, -14.2, 1.0, "#facc15", "left");
  text("L2", S1 - 14.5, -14.2, 1.0, "#fb7185", "left");
  text(`SECTION A–A · CH 0+${String(Math.round(SECTION_X + 400)).padStart(3, "0")}`, S1 - 2, 38.4, 1.55, "rgba(255,255,255,0.95)", "left");

  const tex = new THREE.CanvasTexture(cv);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.anisotropy = 8;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  const mat = new THREE.MeshBasicMaterial({
    map: tex,
    transparent: true,
    depthTest: false,
    depthWrite: false,
    side: THREE.DoubleSide,
    toneMapped: false,
    fog: false,
    opacity: 0.95,
  });
  const group = new THREE.Group();
  group.name = "section";
  const slice = new THREE.Mesh(new THREE.PlaneGeometry(S1 - S0, Y1 - Y0), mat);
  slice.rotation.y = Math.PI / 2;
  slice.position.set(SECTION_X, (Y0 + Y1) / 2, zc + (S0 + S1) / 2);
  slice.renderOrder = 14;
  slice.frustumCulled = false;
  group.add(slice);

  /* ---------- The live overlay: pond line, freeboard dimension, the phreatic surface under the storm ---------- */
  const flat = (colour: string, opacity = 1) =>
    new THREE.MeshBasicMaterial({ color: colour, transparent: true, opacity, depthTest: false, depthWrite: false, toneMapped: false, fog: false, side: THREE.DoubleSide });
  const ox = SECTION_X - 0.05;
  // A strip in the section's plane between two (s, y) points, `w` metres wide.
  const strip = (m: THREE.Material) => {
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(new Float32Array(12), 3));
    geo.setIndex([0, 1, 2, 2, 1, 3]);
    const mesh = new THREE.Mesh(geo, m);
    mesh.renderOrder = 16;
    mesh.frustumCulled = false;
    group.add(mesh);
    return (s0: number, y0: number, s1: number, y1: number, w: number) => {
      const a = geo.getAttribute("position") as THREE.BufferAttribute;
      const dx = s1 - s0;
      const dy = y1 - y0;
      const l = Math.hypot(dx, dy) || 1;
      const nx = (-dy / l) * w * 0.5;
      const ny = (dx / l) * w * 0.5;
      a.setXYZ(0, ox, y0 + ny, zc + s0 + nx);
      a.setXYZ(1, ox, y0 - ny, zc + s0 - nx);
      a.setXYZ(2, ox, y1 + ny, zc + s1 + nx);
      a.setXYZ(3, ox, y1 - ny, zc + s1 - nx);
      a.needsUpdate = true;
    };
  };
  const pondMat = flat("#6cc8f7");
  const pondLine = strip(pondMat);
  const dimMat = flat("#ffffff");
  const dimLine = strip(dimMat);
  const tickA = strip(dimMat);
  const tickB = strip(dimMat);
  // The phreatic surface under the storm: a ribbon that rises over the tailings.
  const N = 60;
  const ribGeo = new THREE.BufferGeometry();
  ribGeo.setAttribute("position", new THREE.BufferAttribute(new Float32Array(N * 2 * 3), 3));
  const idx: number[] = [];
  for (let i = 0; i < N - 1; i++) idx.push(i * 2, i * 2 + 1, i * 2 + 2, i * 2 + 2, i * 2 + 1, i * 2 + 3);
  ribGeo.setIndex(idx);
  const rib = new THREE.Mesh(ribGeo, flat("#e0f6ff", 0.95));
  rib.renderOrder = 16;
  rib.frustumCulled = false;
  group.add(rib);
  const sPts = Array.from({ length: N }, (_, i) => S0 + 2 + ((-4 - (S0 + 2)) * i) / (N - 1));

  const dimS = S0 + 9;
  return {
    group,
    tips,
    dimAt: (lv) => new THREE.Vector3(ox, (lv + crest) / 2, zc + dimS),
    update(lv, rain, scale) {
      const w = Math.max(0.12, scale * 0.0028);
      const below = CREST_LOW - lv < allow;
      pondMat.color.set(below ? "#fb7185" : lv - POND > 0.005 ? "#9fe0ff" : "#6cc8f7");
      pondLine(S0 + 1, lv, -DAM.crestHalf - DAM.up * (CREST_LOW - lv), lv, w * 1.2);
      dimLine(dimS, lv + w, dimS, crest - w, w * 0.8);
      tickA(dimS - 2.2, crest, dimS + 2.2, crest, w * 0.8);
      tickB(dimS - 2.2, lv, dimS + 2.2, lv, w * 0.8);
      const rise = lv - POND;
      const a = ribGeo.getAttribute("position") as THREE.BufferAttribute;
      const show = rise > 0.01 || rain > 1;
      rib.visible = show;
      if (show) {
        for (let i = 0; i < N; i++) {
          const s = sPts[i]!;
          const y = phreaticAt(s) + phreaticRise(s, rise, rain);
          a.setXYZ(i * 2, ox, y + w * 0.45, zc + s);
          a.setXYZ(i * 2 + 1, ox, y - w * 0.45, zc + s);
        }
        a.needsUpdate = true;
      }
    },
  };
}
