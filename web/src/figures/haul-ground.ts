/**
 * Close-range ground for the haul scene: the floor roads the trucks have packed down between the ramp and the
 * loaders, with the darker bands their tyres run in, and spilled rock round each loading spot. Pure geometry and
 * data textures; nothing touches the DOM.
 */
import * as THREE from "three";
import { makeRocks } from "./fleet";

/** A road surface: packed fines, tyre bands, ruts and grain; alpha fades at the edges into the floor. */
function roadTexture() {
  const W = 128;
  const Hh = 256;
  const data = new Uint8Array(W * Hh * 4);
  let seed = 99;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const streak = Array.from({ length: W }, () => rnd());
  for (let y = 0; y < Hh; y++) {
    for (let x = 0; x < W; x++) {
      const u = x / (W - 1);
      // Two wheel paths each side of the middle, worn darker and smoother.
      const band = (c: number, w: number) => Math.max(0, 1 - ((u - c) / w) ** 2);
      const tyres = Math.max(band(0.3, 0.07), band(0.7, 0.07), band(0.36, 0.035) * 0.8, band(0.64, 0.035) * 0.8);
      const n = rnd();
      const s = streak[x]! * 0.5 + 0.5 * streak[(x + Math.floor(y / 32)) % W]!;
      let v = 0.94 - tyres * 0.2 + (n - 0.5) * 0.08 + (s - 0.5) * 0.06;
      v *= 1 - 0.05 * Math.sin((y / Hh) * Math.PI * 14 + u * 3);
      const edge = Math.min(1, Math.min(u, 1 - u) / 0.16);
      const i = (y * W + x) * 4;
      data[i] = Math.round(176 * v);
      data[i + 1] = Math.round(160 * v);
      data[i + 2] = Math.round(133 * v);
      data[i + 3] = Math.round(255 * edge ** 1.4 * (0.82 + 0.18 * n));
    }
  }
  const tex = new THREE.DataTexture(data, W, Hh, THREE.RGBAFormat);
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.RepeatWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.anisotropy = 8;
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.needsUpdate = true;
  return tex;
}

/** A strip of road along a path, draped on the ground. */
function ribbon(pts: THREE.Vector3[], width: number, H: (x: number, z: number) => number, out: { pos: number[]; uv: number[]; idx: number[] }) {
  const base = out.pos.length / 3;
  let run = 0;
  const n = pts.length;
  for (let i = 0; i < n; i++) {
    const a = pts[Math.max(0, i - 1)]!;
    const b = pts[Math.min(n - 1, i + 1)]!;
    const tx = b.x - a.x;
    const tz = b.z - a.z;
    const l = Math.hypot(tx, tz) || 1;
    const nx = -tz / l;
    const nz = tx / l;
    if (i) run += pts[i]!.distanceTo(pts[i - 1]!);
    for (const k of [-1, 1]) {
      const x = pts[i]!.x + nx * k * width * 0.5;
      const z = pts[i]!.z + nz * k * width * 0.5;
      out.pos.push(x, H(x, z) + 0.16, z);
      out.uv.push(k < 0 ? 0 : 1, run / 18);
    }
    if (i) {
      const q = base + (i - 1) * 2;
      out.idx.push(q, q + 2, q + 1, q + 1, q + 2, q + 3);
    }
  }
}

export function makeFloorRoads(paths: THREE.Vector3[][], H: (x: number, z: number) => number, width = 17) {
  const out = { pos: [] as number[], uv: [] as number[], idx: [] as number[] };
  for (const p of paths) {
    // Resample to about 2 m so the strip follows the floor's relief.
    const curve = new THREE.CatmullRomCurve3(p, false, "centripetal");
    const len = curve.getLength();
    ribbon(curve.getSpacedPoints(Math.max(4, Math.ceil(len / 2))), width, H, out);
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(out.pos, 3));
  geo.setAttribute("uv", new THREE.Float32BufferAttribute(out.uv, 2));
  geo.setIndex(out.idx);
  geo.computeVertexNormals();
  const mat = new THREE.MeshStandardMaterial({
    map: roadTexture(),
    transparent: true,
    depthWrite: false,
    roughness: 0.95,
    metalness: 0,
    polygonOffset: true,
    polygonOffsetFactor: -4,
    polygonOffsetUnits: -4,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.receiveShadow = true;
  mesh.renderOrder = 1;
  return mesh;
}

/** Spilled rock round a loading spot: a few boulders and a scatter of fines, set on the ground. */
export function makeSpill(at: THREE.Vector3, H: (x: number, z: number) => number, o: { count: number; radius: number; palette: "ore" | "waste"; seed: number }) {
  return makeRocks({
    count: o.count,
    seed: o.seed,
    shapes: 4,
    detail: 1,
    palette: o.palette,
    place: (_i, r) => {
      const a = r() * Math.PI * 2;
      const d = Math.sqrt(r()) * o.radius;
      const s = 0.25 + 0.9 * r() ** 3;
      const x = at.x + Math.cos(a) * d;
      const z = at.z + Math.sin(a) * d;
      return { x, y: H(x, z) + s * 0.18, z, size: s, flat: 0.6 };
    },
  }).group;
}

/** Gravel for the working floor: packed crushed rock with stones of every size, as an albedo and a normal map. */
function gravelTextures(N = 256) {
  let seed = 4711;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const h = new Float32Array(N * N);
  // Value noise in a few octaves, tiling.
  const lattice = (f: number) => {
    const g = Array.from({ length: f * f }, () => rnd());
    return (x: number, y: number) => {
      const fx = (x / N) * f;
      const fy = (y / N) * f;
      const x0 = Math.floor(fx);
      const y0 = Math.floor(fy);
      const tx = fx - x0;
      const ty = fy - y0;
      const at = (i: number, j: number) => g[((j % f) + f) % f * f + (((i % f) + f) % f)]!;
      const sx = tx * tx * (3 - 2 * tx);
      const sy = ty * ty * (3 - 2 * ty);
      return (at(x0, y0) * (1 - sx) + at(x0 + 1, y0) * sx) * (1 - sy) + (at(x0, y0 + 1) * (1 - sx) + at(x0 + 1, y0 + 1) * sx) * sy;
    };
  };
  const oct = [lattice(4), lattice(16), lattice(48), lattice(128)];
  const tint = new Float32Array(N * N);
  for (let y = 0; y < N; y++) {
    for (let x = 0; x < N; x++) {
      const i = y * N + x;
      h[i] = oct[0]!(x, y) * 0.25 + oct[1]!(x, y) * 0.3 + oct[2]!(x, y) * 0.25 + oct[3]!(x, y) * 0.2;
      tint[i] = oct[1]!(x + 97, y + 31);
    }
  }
  // Stones: rounded bumps, a few big and many small, each with its own shade.
  const shadeOf = new Float32Array(N * N).fill(0);
  for (let k = 0; k < 900; k++) {
    const r = 1.2 + rnd() ** 3 * 6;
    const cx = rnd() * N;
    const cy = rnd() * N;
    const sh = (rnd() - 0.5) * 0.5;
    for (let dy = -Math.ceil(r); dy <= Math.ceil(r); dy++) {
      for (let dx = -Math.ceil(r); dx <= Math.ceil(r); dx++) {
        const q = (dx * dx + dy * dy) / (r * r);
        if (q > 1) continue;
        const x = (((Math.floor(cx) + dx) % N) + N) % N;
        const y = (((Math.floor(cy) + dy) % N) + N) % N;
        const i = y * N + x;
        const b = Math.sqrt(1 - q) * r * 0.07;
        if (b > h[i]! - 0.5) {
          h[i] = Math.max(h[i]!, 0.5 + b);
          shadeOf[i] = sh;
        }
      }
    }
  }
  const albedo = new Uint8Array(N * N * 4);
  const normal = new Uint8Array(N * N * 4);
  for (let y = 0; y < N; y++) {
    for (let x = 0; x < N; x++) {
      const i = y * N + x;
      const hx = h[y * N + ((x + 1) % N)]! - h[y * N + ((x + N - 1) % N)]!;
      const hy = h[((y + 1) % N) * N + x]! - h[((y + N - 1) % N) * N + x]!;
      const nx = -hx * 3.2;
      const ny = -hy * 3.2;
      const l = Math.hypot(nx, ny, 1);
      normal[i * 4] = Math.round((nx / l * 0.5 + 0.5) * 255);
      normal[i * 4 + 1] = Math.round((ny / l * 0.5 + 0.5) * 255);
      normal[i * 4 + 2] = Math.round((1 / l * 0.5 + 0.5) * 255);
      normal[i * 4 + 3] = 255;
      // Fines between the stones are paler; stones vary from grey to rust; hollows hold a little shade.
      const v = 0.86 + (h[i]! - 0.5) * 0.25 + shadeOf[i]! * 0.5 + (rnd() - 0.5) * 0.06;
      const warm = (tint[i]! - 0.5) * 0.12;
      albedo[i * 4] = Math.max(0, Math.min(255, Math.round(178 * v * (1 + warm))));
      albedo[i * 4 + 1] = Math.max(0, Math.min(255, Math.round(162 * v)));
      albedo[i * 4 + 2] = Math.max(0, Math.min(255, Math.round(136 * v * (1 - warm))));
      albedo[i * 4 + 3] = 255;
    }
  }
  const tex = (data: Uint8Array) => {
    const t = new THREE.DataTexture(data, N, N, THREE.RGBAFormat);
    t.wrapS = t.wrapT = THREE.RepeatWrapping;
    t.magFilter = THREE.LinearFilter;
    t.minFilter = THREE.LinearMipmapLinearFilter;
    t.generateMipmaps = true;
    t.anisotropy = 4;
    t.needsUpdate = true;
    return t;
  };
  const map = tex(albedo);
  map.colorSpace = THREE.SRGBColorSpace;
  return { map, normalMap: tex(normal) };
}

/**
 * The working floor round the loaders: packed crushed rock draped over the ground, patchy at large scale, fading out
 * at its edge into the pit floor. Lifted a little and drawn with a polygon offset so it never fights the terrain.
 */
export function makeApron(centre: THREE.Vector3, radius: number, H: (x: number, z: number) => number, cell = 2.5) {
  const n = Math.ceil((radius * 2) / cell);
  const pos: number[] = [];
  const uv: number[] = [];
  const col: number[] = [];
  const idx: number[] = [];
  const big = (x: number, z: number) => 0.5 + 0.25 * Math.sin(x * 0.031 + z * 0.012) + 0.25 * Math.sin(z * 0.043 - x * 0.017 + 1.7);
  for (let j = 0; j <= n; j++) {
    for (let i = 0; i <= n; i++) {
      const x = centre.x - radius + i * cell;
      const z = centre.z - radius + j * cell;
      const d = Math.hypot(x - centre.x, z - centre.z) / radius;
      const edge = 1 - Math.min(1, Math.max(0, (d - 0.62 + (big(x * 3, z * 3) - 0.5) * 0.12) / 0.34));
      pos.push(x, H(x, z) + 0.08, z);
      uv.push(x / 7, z / 7);
      const b = big(x, z);
      col.push(0.9 + b * 0.16, 0.9 + b * 0.14, 0.88 + b * 0.12, edge * edge * (3 - 2 * edge));
    }
  }
  for (let j = 0; j < n; j++) {
    for (let i = 0; i < n; i++) {
      const a = j * (n + 1) + i;
      idx.push(a, a + n + 1, a + 1, a + 1, a + n + 1, a + n + 2);
    }
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
  geo.setAttribute("uv", new THREE.Float32BufferAttribute(uv, 2));
  geo.setAttribute("color", new THREE.Float32BufferAttribute(col, 4));
  geo.setIndex(idx);
  geo.computeVertexNormals();
  const { map, normalMap } = gravelTextures();
  const mat = new THREE.MeshStandardMaterial({
    map,
    normalMap,
    normalScale: new THREE.Vector2(0.9, 0.9),
    vertexColors: true,
    transparent: true,
    depthWrite: false,
    roughness: 0.97,
    metalness: 0,
    polygonOffset: true,
    polygonOffsetFactor: -2,
    polygonOffsetUnits: -2,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.receiveShadow = true;
  mesh.renderOrder = 0;
  mesh.name = "haul-apron";
  return mesh;
}
