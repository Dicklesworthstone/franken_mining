// Adapted from FrankenSim's kitchen lab (apps/kitchen-web/src/three/models/people.ts): the body, skeleton and motion core.
/**
 * Procedural, animated people built to the anthropometric cohorts p05 / p50 / p95 (ISO 7250-1, NASA-STD-3000), and a
 * motion layer that walks them along floor paths on planted feet and works them with two-bone arm and leg IK inside
 * the table's ranges of motion. What they wear is up to the caller (`StaffOptions.dress`): this module has the
 * body-fitted garment builders (a shirt or coverall top, trousers, boots, gloves) and a plain set of workwear.
 *
 * Every person is synthetic. Heads are sculpted like an artist's mannequin (skull, jaw, ears, neck, hair) with no
 * eyes, nose or mouth, so nothing biometric is drawn.
 *
 * Body frame (metres): the root sits on the floor between the feet, y up, the person facing +z, their left at +x.
 * The table's heights are barefoot; footwear lifts the whole body by its sole (`StaffOptions.sole`, by default the
 * row's `shoe_sole_m`).
 *
 * Meshes: one SkinnedMesh per material, all bound to the same skeleton. Cloth is built as lofts shaped round the
 * body; vertices are rigid (weight 1 to a bone) except where cloth or skin crosses a joint, where two bones blend so
 * that joints stay closed. Geometry is in body space at the bind pose, so a dresser may share it between people of
 * the same cohort.
 *
 * Skeleton: 29 base bones (root, pelvis, spine_01..03, neck, head, gaze_target, carry_chest, clavicles, arms, hands,
 * grip sockets, legs, feet, toes) plus 15 finger bones a hand: thumb_01..03, index_01..03, middle_01..03,
 * ring_01..03, pinky_01..03, suffixed _L and _R (59 bones in all). Every deforming bone's bind rotation is the
 * identity, so a bone's local rotation is a rotation in the body frame.
 *
 * Grip frame (`hand(side)`, the grip_L/grip_R bones): the handle of a held tool runs along +z through the fist, from
 * the heel of the palm out past the thumb and forefinger towards the tool's working end; +y is the thumb side. Model
 * a tool with its handle's middle at the origin, the handle along +z and its top along +y, and add it as a child of
 * `hand("R")`.
 *
 * Nothing here touches the DOM at import time; textures are drawn on first use, and not at all without a document.
 */
import * as THREE from "three";

/** Mesh detail: 0 far, 1 mid (default), 2 close-up. */
export type Detail = 0 | 1 | 2;
export type Cohort = "p05" | "p50" | "p95";
export type Side = "L" | "R";
/** The work the core knows; dressers add their own by name (`StaffOptions.works`). */
export type CoreWork = "stand" | "carry" | "pick" | "place";
export type WorkKind = CoreWork | (string & {});
export type RomKey =
  | "neck_pitch"
  | "neck_yaw"
  | "shoulder_flex"
  | "shoulder_abd"
  | "elbow_flex"
  | "wrist_flex"
  | "torso_flex"
  | "torso_yaw"
  | "hip_flex"
  | "knee_flex"
  | "ankle_pitch";

/** One cohort's row of anthropometrics.tsv (metres, degrees), with a body mass. */
export interface Anthro {
  readonly percentile_id: string;
  readonly cohort_label: string;
  readonly standing_height_m: number;
  readonly eye_height_m: number;
  readonly shoulder_height_m: number;
  readonly elbow_height_m: number;
  readonly knuckle_height_m: number;
  readonly hip_height_m: number;
  readonly knee_height_m: number;
  readonly ankle_height_m: number;
  readonly shoulder_breadth_m: number;
  readonly chest_depth_m: number;
  readonly upper_arm_m: number;
  readonly forearm_m: number;
  readonly hand_length_m: number;
  readonly forward_reach_m: number;
  readonly overhead_reach_m: number;
  readonly shoe_length_m: number;
  readonly shoe_width_m: number;
  readonly shoe_sole_m: number;
  readonly torso_capsule_radius_m: number;
  readonly joint_rom_deg: Readonly<Record<RomKey, readonly [number, number]>>;
  readonly source_citation: string;
  /** Not in the table: body masses for the L5/S1 estimate (52, 74, 98 kg). */
  readonly body_mass_kg: number;
}

const ROM: Record<RomKey, readonly [number, number]> = {
  neck_pitch: [-45, 45],
  neck_yaw: [-65, 65],
  shoulder_flex: [-45, 165],
  shoulder_abd: [0, 150],
  elbow_flex: [0, 142],
  wrist_flex: [-65, 70],
  torso_flex: [-20, 55],
  torso_yaw: [-40, 40],
  hip_flex: [-15, 115],
  knee_flex: [0, 135],
  ankle_pitch: [-35, 20],
};

function row(id: string, label: string, n: readonly number[], mass: number, cite: string): Anthro {
  const [h, eye, sh, el, kn, hip, knee, ank, sb, cd, ua, fa, hl, fr, or, sl, sw, ss, tr] = n as [
    number, number, number, number, number, number, number, number, number, number,
    number, number, number, number, number, number, number, number, number,
  ];
  return {
    percentile_id: id,
    cohort_label: label,
    standing_height_m: h,
    eye_height_m: eye,
    shoulder_height_m: sh,
    elbow_height_m: el,
    knuckle_height_m: kn,
    hip_height_m: hip,
    knee_height_m: knee,
    ankle_height_m: ank,
    shoulder_breadth_m: sb,
    chest_depth_m: cd,
    upper_arm_m: ua,
    forearm_m: fa,
    hand_length_m: hl,
    forward_reach_m: fr,
    overhead_reach_m: or,
    shoe_length_m: sl,
    shoe_width_m: sw,
    shoe_sole_m: ss,
    torso_capsule_radius_m: tr,
    joint_rom_deg: ROM,
    source_citation: cite,
    body_mass_kg: mass,
  };
}

/** The three cohorts, transcribed from FrankenSim's anthropometrics.tsv (barefoot heights). */
export const ANTHROPOMETRICS: Readonly<Record<Cohort, Anthro>> = {
  p05: row("P05_FEMALE", "5th Percentile Adult Female",
    [1.511, 1.402, 1.234, 0.942, 0.678, 0.785, 0.422, 0.068, 0.382, 0.208, 0.305, 0.232, 0.165, 0.676, 1.854, 0.242, 0.092, 0.026, 0.175], 52,
    "ISO 7250-1:2017; NASA-STD-3000 Vol. I sec. 3.3.1 (5th percentile female anthropometry & joint ROM)"),
  p50: row("P50_ADULT", "50th Percentile Combined Adult",
    [1.715, 1.6, 1.406, 1.074, 0.758, 0.892, 0.49, 0.076, 0.448, 0.238, 0.345, 0.264, 0.186, 0.772, 2.115, 0.276, 0.104, 0.028, 0.205], 74,
    "ISO 7250-1:2017; NASA-STD-3000 Vol. I sec. 3.3.1 (50th percentile adult midpoint anthropometry & joint ROM)"),
  p95: row("P95_MALE", "95th Percentile Adult Male",
    [1.878, 1.76, 1.552, 1.186, 0.832, 0.985, 0.548, 0.084, 0.506, 0.272, 0.38, 0.292, 0.206, 0.862, 2.34, 0.308, 0.116, 0.03, 0.235], 98,
    "ISO 7250-1:2017; NASA-STD-3000 Vol. I sec. 3.3.1 (95th percentile male anthropometry & joint ROM)"),
};

/* ------------------------------------------------------------------------------------------------------------ */
/* Small maths                                                                                                    */
/* ------------------------------------------------------------------------------------------------------------ */

export const DEG = Math.PI / 180;
export const clamp = (x: number, a: number, b: number) => (x < a ? a : x > b ? b : x);
export const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
/** Hermite smoothstep of x between e0 and e1 (either order). */
export function smooth(e0: number, e1: number, x: number): number {
  const t = clamp((x - e0) / (e1 - e0), 0, 1);
  return t * t * (3 - 2 * t);
}
/** Minimum-jerk position profile on [0, 1]. */
export function mj(u: number): number {
  const t = clamp(u, 0, 1);
  return t * t * t * (10 + t * (-15 + 6 * t));
}
/** Ease in and out on [0, 1] (cosine). */
export function ease(u: number): number {
  return 0.5 - 0.5 * Math.cos(Math.PI * clamp(u, 0, 1));
}
export function wrapPi(a: number): number {
  let x = (a + Math.PI) % (Math.PI * 2);
  if (x < 0) x += Math.PI * 2;
  return x - Math.PI;
}
/** Sign-preserving power, for superellipses. */
export function spow(x: number, p: number): number {
  return Math.sign(x) * Math.pow(Math.abs(x), p);
}
/** Soft minimum of a and b with blending width k. */
export function smin(a: number, b: number, k: number): number {
  const h = clamp(0.5 + (0.5 * (b - a)) / k, 0, 1);
  return lerp(b, a, h) - k * h * (1 - h);
}

/** xorshift32 from a seed: repeatable draws in [0, 1). */
export function rng(seed: number): () => number {
  let s = (seed * 2654435761) >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s >>>= 0;
    s ^= s >>> 17;
    s ^= s << 5;
    s >>>= 0;
    return s / 4294967296;
  };
}

export const V = (x = 0, y = 0, z = 0) => new THREE.Vector3(x, y, z);
export const X = V(1, 0, 0);
export const Y = V(0, 1, 0);
export const Z = V(0, 0, 1);
const ZERO = V(0, 0, 0);

/** Monotone cubic interpolation (Fritsch-Carlson) through (x_i, y_i), x increasing; clamps outside. */
export class Mono {
  private m: number[];
  constructor(readonly xs: readonly number[], readonly ys: readonly number[]) {
    const n = xs.length;
    const d: number[] = [];
    for (let i = 0; i < n - 1; i++) d.push((ys[i + 1]! - ys[i]!) / Math.max(1e-9, xs[i + 1]! - xs[i]!));
    const m: number[] = new Array<number>(n).fill(0);
    if (n > 1) {
      m[0] = d[0]!;
      m[n - 1] = d[n - 2]!;
      for (let i = 1; i < n - 1; i++) m[i] = d[i - 1]! * d[i]! <= 0 ? 0 : (d[i - 1]! + d[i]!) / 2;
      for (let i = 0; i < n - 1; i++) {
        if (d[i] === 0) {
          m[i] = 0;
          m[i + 1] = 0;
          continue;
        }
        const a = m[i]! / d[i]!;
        const b = m[i + 1]! / d[i]!;
        const h = a * a + b * b;
        if (h > 9) {
          const t = 3 / Math.sqrt(h);
          m[i] = t * a * d[i]!;
          m[i + 1] = t * b * d[i]!;
        }
      }
    }
    this.m = m;
  }
  /** Force the end slopes (for starting and stopping at rest). */
  ends(m0: number, m1: number): this {
    this.m[0] = m0;
    this.m[this.m.length - 1] = m1;
    return this;
  }
  at(x: number): number {
    const xs = this.xs;
    const n = xs.length;
    if (n === 1 || x <= xs[0]!) return this.ys[0]!;
    if (x >= xs[n - 1]!) return this.ys[n - 1]!;
    let lo = 0;
    let hi = n - 1;
    while (hi - lo > 1) {
      const mid = (lo + hi) >> 1;
      if (xs[mid]! <= x) lo = mid;
      else hi = mid;
    }
    const h = xs[hi]! - xs[lo]!;
    const t = (x - xs[lo]!) / h;
    const t2 = t * t;
    const t3 = t2 * t;
    return (2 * t3 - 3 * t2 + 1) * this.ys[lo]! + (t3 - 2 * t2 + t) * h * this.m[lo]! + (-2 * t3 + 3 * t2) * this.ys[hi]! + (t3 - t2) * h * this.m[hi]!;
  }
}

const _fm0 = new THREE.Matrix4();
const _fm1 = new THREE.Matrix4();
const _fc0 = V();
const _fc1 = V();
/** The rotation taking the orthonormal pair (a0, b0) onto (a, b). */
export function frameQuat(a0: THREE.Vector3, b0: THREE.Vector3, a: THREE.Vector3, b: THREE.Vector3, out: THREE.Quaternion): THREE.Quaternion {
  _fm0.makeBasis(a0, b0, _fc0.crossVectors(a0, b0));
  _fm1.makeBasis(a, b, _fc1.crossVectors(a, b));
  _fm0.transpose();
  _fm1.multiply(_fm0);
  return out.setFromRotationMatrix(_fm1);
}
/** Make b perpendicular to unit a and unit length (falling back to `alt`). */
export function orthoTo(b: THREE.Vector3, a: THREE.Vector3, alt: THREE.Vector3 = X): THREE.Vector3 {
  b.addScaledVector(a, -b.dot(a));
  if (b.lengthSq() < 1e-10) b.copy(alt).addScaledVector(a, -alt.dot(a));
  return b.normalize();
}
/** Swing-twist: the twist angle of q about unit axis a. */
export function twistAngle(q: THREE.Quaternion, a: THREE.Vector3): number {
  const p = q.x * a.x + q.y * a.y + q.z * a.z;
  return 2 * Math.atan2(p, q.w);
}
export const qAxis = (axis: THREE.Vector3, ang: number, out = new THREE.Quaternion()) => out.setFromAxisAngle(axis, ang);


/* ------------------------------------------------------------------------------------------------------------ */
/* Cloth, skin and hair. Drawn once on canvases from fixed seeds and shared by every person. Garment colours ride  */
/* on vertex colours (one cloth material serves every colour); skin and hair tones ride on the material, one per  */
/* tone, so that the geometry itself can be shared by everyone of a cohort.                                       */
/* ------------------------------------------------------------------------------------------------------------ */

/** Whether canvases can be drawn (a browser). Without one (a Bun script) the materials go without textures. */
export function canDraw(): boolean {
  return typeof document !== "undefined" && typeof document.createElement === "function";
}
export function canvas2d(w: number, h: number): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  return [c, c.getContext("2d")!];
}
/** A texture whose UVs are metres, repeating every `tile` metres. */
export function canvasTex(c: HTMLCanvasElement, srgb: boolean, tile: number): THREE.CanvasTexture {
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = srgb ? THREE.SRGBColorSpace : THREE.NoColorSpace;
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.repeat.set(1 / tile, 1 / tile);
  t.anisotropy = 8;
  t.needsUpdate = true;
  return t;
}
/** A tangent-space normal map from a height function on the unit square (periodic), `k` its strength. */
export function normalMap(S: number, h: (u: number, v: number) => number, k: number): HTMLCanvasElement {
  const [c, x] = canvas2d(S, S);
  const hs = new Float32Array(S * S);
  for (let j = 0; j < S; j++) for (let i = 0; i < S; i++) hs[j * S + i] = h(i / S, j / S);
  const img = x.createImageData(S, S);
  for (let j = 0; j < S; j++)
    for (let i = 0; i < S; i++) {
      const dx = (hs[j * S + ((i + 1) % S)]! - hs[j * S + ((i + S - 1) % S)]!) * k;
      const dy = (hs[((j + 1) % S) * S + i]! - hs[((j + S - 1) % S) * S + i]!) * k;
      const l = Math.hypot(dx, dy, 1);
      const o = (j * S + i) * 4;
      img.data[o] = Math.round((0.5 - (0.5 * dx) / l) * 255);
      img.data[o + 1] = Math.round((0.5 + (0.5 * dy) / l) * 255);
      img.data[o + 2] = Math.round((0.5 + 0.5 / l) * 255);
      img.data[o + 3] = 255;
    }
  x.putImageData(img, 0, 0);
  return c;
}

/** 2/1 twill: diagonal wales with a little yarn noise. One tile is `n` wales. */
export function twill(seed: number, n: number): HTMLCanvasElement {
  const r = rng(seed);
  const noise = new Float32Array(64 * 64).map(() => r());
  const nz = (u: number, v: number) => noise[(Math.floor(v * 64) % 64) * 64 + (Math.floor(u * 64) % 64)]!;
  return normalMap(256, (u, v) => {
    const w = (u + v) * n;
    const f = w - Math.floor(w);
    const ridge = Math.sin(Math.PI * f) ** 0.6;
    const yarn = 0.25 * Math.sin(Math.PI * 2 * (u - v) * n * 3);
    return ridge + yarn * ridge + 0.35 * nz(u, v);
  }, 1.4);
}

/** Fine skin: pores and a soft mottling, as a bump map. */
export function skinBump(): HTMLCanvasElement {
  const S = 256;
  const [c, x] = canvas2d(S, S);
  const r = rng(4242);
  x.fillStyle = "#808080";
  x.fillRect(0, 0, S, S);
  for (let i = 0; i < 900; i++) {
    const v = 110 + r() * 40;
    x.fillStyle = `rgba(${v | 0},${v | 0},${v | 0},0.25)`;
    x.beginPath();
    x.arc(r() * S, r() * S, 4 + r() * 14, 0, Math.PI * 2);
    x.fill();
  }
  for (let i = 0; i < 5200; i++) {
    x.fillStyle = `rgba(40,40,40,${(0.25 + r() * 0.35).toFixed(2)})`;
    x.fillRect(r() * S, r() * S, 1, 1);
  }
  return c;
}

/** Hair: fine strands running down the texture's v, light and dark. */
export function hairStrands(): HTMLCanvasElement {
  const S = 256;
  const [c, x] = canvas2d(S, S);
  const r = rng(9091);
  x.fillStyle = "#9a9a9a";
  x.fillRect(0, 0, S, S);
  for (let i = 0; i < 2600; i++) {
    const v = r() < 0.5 ? 40 + r() * 60 : 170 + r() * 85;
    x.strokeStyle = `rgba(${v | 0},${v | 0},${v | 0},${(0.25 + r() * 0.4).toFixed(2)})`;
    x.lineWidth = 0.6 + r() * 0.9;
    const x0 = r() * S;
    const y0 = r() * S;
    const len = 20 + r() * 70;
    const bend = (r() - 0.5) * 8;
    x.beginPath();
    for (const dx of [-S, 0]) {
      x.moveTo(x0 + dx, y0);
      x.quadraticCurveTo(x0 + dx + bend, y0 + len / 2, x0 + dx + bend * 0.3, y0 + len);
    }
    x.stroke();
  }
  return c;
}

/**
 * Per-vertex material flags, the `aux` attribute of a soup's geometry, read by materials patched with `auxPatch`:
 *   [0] retro: retroreflective tape (glass beads): silver, a little metallic, and blazing back towards a light
 *       that shines from near the viewer's direction (a headlight, a cap lamp, the sun behind the camera);
 *   [1] fluoro: fluorescent dye (hi-vis orange or lime): more light back in its colour than falls on it;
 *   [2] glow: self-lit (a lamp's lens, a screen), its vertex colour as radiance;
 *   [3] base: 1 keeps the material's roughness, lower values make the surface glossier (a rubber boot, a lens).
 * Geometry without the attribute reads (0, 0, 0, 1): plain.
 */
export type Aux = readonly [number, number, number, number];
export const AUX_PLAIN: Aux = [0, 0, 0, 1];

const AUX_LIGHTS = THREE.ShaderChunk.lights_physical_pars_fragment.replace(
  "reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );",
  `reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
	{
		// Fluorescent dye sends back more light in its band than falls on it.
		reflectedLight.directDiffuse += vAux.y * 0.5 * irradiance * BRDF_Lambert( material.diffuseColor );
		// Retroreflective tape: its glass beads return light towards its source, a narrow bright lobe round the
		// light's direction (seen when the light is near the eye) on a broader glow.
		float back = saturate( dot( geometryViewDir, directLight.direction ) );
		float lobe = 7.0 * pow( back, 40.0 ) + 0.9 * pow( back, 6.0 );
		float facing = saturate( dot( geometryNormal, directLight.direction ) * 0.8 + 0.35 );
		reflectedLight.directSpecular += vAux.x * lobe * facing * directLight.color;
	}`,
).replace(
  "reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );",
  `reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor ) * ( 1.0 + 0.4 * vAux.y + 0.25 * vAux.x );`,
);

/**
 * Teach a physical material the `aux` attribute (see `Aux`): x retroreflective, y fluorescent, z glow (emissive by
 * the vertex colour), w the roughness (0 glossy, 1 the material's; above 1 rougher still, without its clearcoat).
 */
export function auxPatch<T extends THREE.MeshPhysicalMaterial>(m: T): T {
  m.onBeforeCompile = (sh) => {
    sh.vertexShader = sh.vertexShader
      .replace("#include <common>", "#include <common>\nattribute vec4 aux;\nvarying vec4 vAux;")
      .replace("#include <begin_vertex>", "#include <begin_vertex>\n\tvAux = aux;");
    sh.fragmentShader = sh.fragmentShader
      .replace("#include <common>", "#include <common>\nvarying vec4 vAux;")
      .replace("#include <lights_physical_pars_fragment>", AUX_LIGHTS)
      .replace(
        "#include <roughnessmap_fragment>",
        "#include <roughnessmap_fragment>\n\troughnessFactor = mix( 0.2, roughnessFactor, vAux.w );\n\troughnessFactor = mix( roughnessFactor, 0.38, vAux.x );",
      )
      .replace("#include <metalnessmap_fragment>", "#include <metalnessmap_fragment>\n\tmetalnessFactor = mix( metalnessFactor, 0.55, vAux.x );")
      .replace(
        "#include <lights_physical_fragment>",
        "#include <lights_physical_fragment>\n\t#ifdef USE_CLEARCOAT\n\t\tmaterial.clearcoat *= clamp( 2.0 - vAux.w, 0.0, 1.0 );\n\t#endif",
      )
      .replace(
        "#include <emissivemap_fragment>",
        "#include <emissivemap_fragment>\n\t#ifdef USE_COLOR\n\t\ttotalEmissiveRadiance += vColor.rgb * vAux.z * 4.0;\n\t#endif",
      );
  };
  m.customProgramCacheKey = () => "people-aux-3";
  return m;
}

/** Skin: light wrapping past the terminator, reddened, for the scatter under the surface. */
function skinScatter(m: THREE.MeshPhysicalMaterial): THREE.MeshPhysicalMaterial {
  m.onBeforeCompile = (sh) => {
    sh.fragmentShader = sh.fragmentShader.replace(
      "#include <lights_physical_pars_fragment>",
      THREE.ShaderChunk.lights_physical_pars_fragment.replace(
        "reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );",
        `reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
	{
		float nl = dot( geometryNormal, directLight.direction );
		float wrapped = saturate( ( nl + 0.5 ) / 1.5 ) - saturate( nl );
		reflectedLight.directDiffuse += wrapped * directLight.color * vec3( 0.55, 0.2, 0.12 ) * BRDF_Lambert( material.diffuseColor );
	}`,
      ),
    );
  };
  m.customProgramCacheKey = () => "people-skin-scatter-1";
  return m;
}

interface SharedTex {
  fine: THREE.Texture | null;
  heavy: THREE.Texture | null;
  skin: THREE.Texture | null;
  strands: THREE.Texture | null;
}
let texCache: SharedTex | null = null;
/** The shared textures (none without a document). */
export function peopleTextures(): SharedTex {
  if (texCache) return texCache;
  const draw = canDraw();
  texCache = {
    fine: draw ? canvasTex(twill(31, 24), false, 0.03) : null,
    heavy: draw ? canvasTex(twill(57, 14), false, 0.03) : null,
    skin: draw ? canvasTex(skinBump(), false, 0.05) : null,
    strands: draw ? canvasTex(hairStrands(), true, 0.06) : null,
  };
  return texCache;
}

export interface PeopleMaterials {
  /** Woven workwear (shirts, coveralls, trousers, hi-vis), coloured by vertex colour, `aux`-patched. */
  cloth: THREE.MeshPhysicalMaterial;
  /** Boots, gloves, belts and straps: leather, rubber and webbing by vertex colour, `aux`-patched. */
  gear: THREE.MeshPhysicalMaterial;
  /** Skin of one tone (its vertex colours only shade it: the inside of an ear). */
  skin(tone: THREE.ColorRepresentation): THREE.MeshPhysicalMaterial;
  /** Hair of one colour, with an anisotropic sheen along the strands. */
  hair(color: THREE.ColorRepresentation): THREE.MeshPhysicalMaterial;
}

let matCache: PeopleMaterials | null = null;

/** The shared materials, made the first time they are asked for. */
export function peopleMaterials(): PeopleMaterials {
  if (matCache) return matCache;
  const T = peopleTextures();
  const skins = new Map<number, THREE.MeshPhysicalMaterial>();
  const hairs = new Map<number, THREE.MeshPhysicalMaterial>();
  matCache = {
    cloth: auxPatch(
      new THREE.MeshPhysicalMaterial({
        vertexColors: true,
        side: THREE.DoubleSide,
        color: 0xffffff,
        roughness: 0.84,
        metalness: 0,
        normalMap: T.fine,
        normalScale: new THREE.Vector2(0.5, 0.5),
        sheen: 0.45,
        sheenRoughness: 0.75,
        sheenColor: new THREE.Color(0xffffff),
      }),
    ),
    gear: auxPatch(
      new THREE.MeshPhysicalMaterial({
        vertexColors: true,
        side: THREE.DoubleSide,
        color: 0xffffff,
        roughness: 0.66,
        metalness: 0,
        normalMap: T.heavy,
        normalScale: new THREE.Vector2(0.25, 0.25),
        clearcoat: 0.18,
        clearcoatRoughness: 0.55,
      }),
    ),
    skin(tone) {
      const c = new THREE.Color(tone);
      const k = c.getHex();
      let m = skins.get(k);
      if (!m) {
        m = skinScatter(
          new THREE.MeshPhysicalMaterial({
            vertexColors: true,
            color: c,
            roughness: 0.56,
            metalness: 0,
            bumpMap: T.skin,
            bumpScale: 0.12,
            sheen: 0.35,
            sheenRoughness: 0.55,
            sheenColor: new THREE.Color(0x9c4a3a),
            clearcoat: 0.06,
            clearcoatRoughness: 0.55,
          }),
        );
        skins.set(k, m);
      }
      return m;
    },
    hair(color) {
      const c = new THREE.Color(color);
      const k = c.getHex();
      let m = hairs.get(k);
      if (!m) {
        m = new THREE.MeshPhysicalMaterial({ vertexColors: true, color: c, roughness: 0.5, metalness: 0, map: T.strands, anisotropy: 0.7, anisotropyRotation: Math.PI / 2, sheen: 0.2, sheenRoughness: 0.4 });
        hairs.set(k, m);
      }
      return m;
    },
  };
  return matCache;
}


/* ------------------------------------------------------------------------------------------------------------ */
/* Skinned geometry: a vertex soup per material, filled by lofts (rings swept along a body part) and primitives.  */
/* ------------------------------------------------------------------------------------------------------------ */

/** Bone influences: [bone index, weight] pairs (normalised on the way in, four at most kept). */
export type Wts = readonly (readonly [number, number])[];
export type WFn = (p: THREE.Vector3) => Wts;

/** A vertex's colour and, for `aux`-patched materials, its flags (see `Aux`). */
export interface Paint {
  c: THREE.Color;
  a?: Aux;
}
export const paint = (c: THREE.ColorRepresentation, a?: Aux): Paint => ({ c: new THREE.Color(c), a });
export type Tint = THREE.Color | Paint;
const isPaint = (x: Tint): x is Paint => !(x as THREE.Color).isColor;

/** A vertex soup: one SkinnedMesh's worth of positions, normals, UVs (metres), colours, flags and bone weights. */
export class Soup {
  pos: number[] = [];
  nrm: number[] = [];
  uv: number[] = [];
  col: number[] = [];
  aux: number[] = [];
  si: number[] = [];
  sw: number[] = [];
  idx: number[] = [];
  private flagged = false;
  get verts(): number {
    return this.pos.length / 3;
  }
  get tris(): number {
    return this.idx.length / 3;
  }
  vert(p: THREE.Vector3, n: THREE.Vector3, u: number, v: number, c: Tint, w: Wts): number {
    const i = this.verts;
    this.pos.push(p.x, p.y, p.z);
    this.nrm.push(n.x, n.y, n.z);
    this.uv.push(u, v);
    const col = isPaint(c) ? c.c : c;
    const a = (isPaint(c) ? c.a : undefined) ?? AUX_PLAIN;
    this.col.push(col.r, col.g, col.b);
    this.aux.push(a[0], a[1], a[2], a[3]);
    if (a !== AUX_PLAIN && (a[0] !== 0 || a[1] !== 0 || a[2] !== 0 || a[3] !== 1)) this.flagged = true;
    const acc: [number, number][] = [];
    for (const [b, x] of w) {
      if (x <= 1e-4) continue;
      const e = acc.find((a) => a[0] === b);
      if (e) e[1] += x;
      else acc.push([b, x]);
    }
    acc.sort((a, b) => b[1] - a[1]);
    acc.length = Math.min(acc.length, 4);
    let sum = 0;
    for (const a of acc) sum += a[1];
    for (let k = 0; k < 4; k++) {
      const a = acc[k];
      this.si.push(a ? a[0] : 0);
      this.sw.push(a && sum > 0 ? a[1] / sum : k === 0 && !a ? 1 : 0);
    }
    return i;
  }
  tri(a: number, b: number, c: number): void {
    this.idx.push(a, b, c);
  }
  /** The soup as geometry; `skinned` false leaves out the bone attributes (a rigid part). */
  geometry(skinned = true): THREE.BufferGeometry {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(this.pos, 3));
    g.setAttribute("normal", new THREE.Float32BufferAttribute(this.nrm, 3));
    g.setAttribute("uv", new THREE.Float32BufferAttribute(this.uv, 2));
    g.setAttribute("color", new THREE.Float32BufferAttribute(this.col, 3));
    if (this.flagged) g.setAttribute("aux", new THREE.Float32BufferAttribute(this.aux, 4));
    if (skinned) {
      g.setAttribute("skinIndex", new THREE.Uint16BufferAttribute(this.si, 4));
      g.setAttribute("skinWeight", new THREE.Float32BufferAttribute(this.sw, 4));
    }
    g.setIndex(this.verts > 65535 ? new THREE.Uint32BufferAttribute(this.idx, 1) : new THREE.Uint16BufferAttribute(this.idx, 1));
    g.computeBoundingSphere();
    return g;
  }
}

export type ColorFn = Tint | ((i: number, j: number, p: THREE.Vector3) => Tint);
export interface LoftOpts {
  /** Rings are closed loops (default) or open strips. */
  closed?: boolean;
  /** Influences of the vertex at ring i, point j. */
  w: (i: number, j: number, p: THREE.Vector3) => Wts;
  color: ColorFn;
  /** A direction the surface should face at a point (default: away from its ring's centre). */
  out?: (i: number, j: number, p: THREE.Vector3) => THREE.Vector3;
  /** Override UVs (default: metres round the ring and along the loft). */
  uv?: (i: number, j: number, p: THREE.Vector3, u: number, v: number) => readonly [number, number];
}

const _la = V();
const _lb = V();
const _lc = V();
/**
 * Sweep rings into a skinned surface. Every ring has the same number of points. Normals come from the grid's own
 * derivatives (so seams of closed rings stay smooth), and the winding follows whichever way `out` says is outside.
 */
export function loft(soup: Soup, rings: readonly THREE.Vector3[][], o: LoftOpts): void {
  const R = rings.length;
  const n = rings[0]!.length;
  const closed = o.closed !== false;
  const cen = rings.map((r) => r.reduce((a, p) => a.add(p), V()).multiplyScalar(1 / r.length));
  const P = (i: number, j: number) => rings[clamp(i, 0, R - 1)]![closed ? (j + n) % n : clamp(j, 0, n - 1)]!;
  const degenerate = rings.map((r) => r.every((p) => p.distanceToSquared(r[0]!) < 1e-12));
  const N: THREE.Vector3[][] = [];
  for (let i = 0; i < R; i++) {
    const row: THREE.Vector3[] = [];
    for (let j = 0; j < n; j++) {
      if (degenerate[i]) {
        const nb = i === 0 ? cen[1]! : cen[R - 2]!;
        row.push(V().subVectors(rings[i]![0]!, nb).normalize());
        continue;
      }
      _la.subVectors(P(i, j + 1), P(i, j - 1));
      let i0 = i - 1;
      let i1 = i + 1;
      i0 = Math.max(0, i0);
      i1 = Math.min(R - 1, i1);
      _lb.subVectors(P(i1, j), P(i0, j));
      row.push(V().crossVectors(_la, _lb).normalize());
    }
    N.push(row);
  }
  // Orientation, decided by a majority vote over a middle ring.
  const mid = Math.floor(R / 2);
  let vote = 0;
  for (let j = 0; j < n; j++) {
    const p = rings[mid]![j]!;
    const ref = o.out ? o.out(mid, j, p) : _lc.subVectors(p, cen[mid]!);
    vote += Math.sign(N[mid]![j]!.dot(ref));
  }
  const flip = vote < 0;
  if (flip) for (const row of N) for (const nn of row) if (nn.lengthSq() > 0) nn.negate();
  for (let i = 0; i < R; i++)
    if (degenerate[i]) {
      const nb = i === 0 ? cen[1]! : cen[R - 2]!;
      for (const nn of N[i]!) nn.subVectors(rings[i]![0]!, nb).normalize();
    }
  // UVs in metres.
  const vs: number[] = [0];
  for (let i = 1; i < R; i++) {
    let d = 0;
    for (let j = 0; j < n; j++) d += rings[i]![j]!.distanceTo(rings[i - 1]![j]!);
    vs.push(vs[i - 1]! + d / n);
  }
  const cols = n + (closed ? 1 : 0);
  const base = soup.verts;
  for (let i = 0; i < R; i++) {
    let u = 0;
    for (let j = 0; j < cols; j++) {
      const jj = j % n;
      if (j > 0) u += rings[i]![jj]!.distanceTo(rings[i]![(j - 1) % n]!);
      const p = rings[i]![jj]!;
      const c = typeof o.color === "function" ? o.color(i, jj, p) : o.color;
      const [uu, vv] = o.uv ? o.uv(i, jj, p, u, vs[i]!) : [u, vs[i]!];
      soup.vert(p, N[i]![jj]!, uu, vv, c, o.w(i, jj, p));
    }
  }
  for (let i = 0; i < R - 1; i++)
    for (let j = 0; j < cols - 1; j++) {
      const a = base + i * cols + j;
      const b = a + 1;
      const c = a + cols + 1;
      const d = a + cols;
      if (flip) {
        soup.tri(a, c, b);
        soup.tri(a, d, c);
      } else {
        soup.tri(a, b, c);
        soup.tri(a, c, d);
      }
    }
}

const _gm = new THREE.Matrix3();
const _gp = V();
const _gn = V();
/** Add a three.js primitive, transformed by `m`, rigid or with per-point influences. */
export function addGeo(soup: Soup, g: THREE.BufferGeometry, m: THREE.Matrix4, w: Wts | WFn, color: Tint | ((p: THREE.Vector3) => Tint), uvScale = 1): void {
  const pos = g.attributes.position!;
  const nrm = g.attributes.normal;
  const uv = g.attributes.uv;
  _gm.getNormalMatrix(m);
  const base = soup.verts;
  for (let i = 0; i < pos.count; i++) {
    _gp.fromBufferAttribute(pos, i).applyMatrix4(m);
    if (nrm) _gn.fromBufferAttribute(nrm, i).applyMatrix3(_gm).normalize();
    else _gn.set(0, 1, 0);
    const c = typeof color === "function" ? color(_gp) : color;
    soup.vert(_gp, _gn, uv ? uv.getX(i) * uvScale : 0, uv ? uv.getY(i) * uvScale : 0, c, typeof w === "function" ? w(_gp) : w);
  }
  const mirrored = m.determinant() < 0;
  const ix = g.index;
  const cnt = ix ? ix.count : pos.count;
  for (let k = 0; k < cnt; k += 3) {
    const a = base + (ix ? ix.getX(k) : k);
    const b = base + (ix ? ix.getX(k + 1) : k + 1);
    const c = base + (ix ? ix.getX(k + 2) : k + 2);
    if (mirrored) soup.tri(a, c, b);
    else soup.tri(a, b, c);
  }
  g.dispose();
}
export const mat4 = (p: THREE.Vector3, q?: THREE.Quaternion, s?: THREE.Vector3) => new THREE.Matrix4().compose(p, q ?? new THREE.Quaternion(), s ?? V(1, 1, 1));

/* ------------------------------------------------------------------------------------------------------------ */
/* Skeleton                                                                                                      */
/* ------------------------------------------------------------------------------------------------------------ */

export const FINGERS = ["thumb", "index", "middle", "ring", "pinky"] as const;
export type Finger = (typeof FINGERS)[number];
export const SIDES: readonly Side[] = ["L", "R"];
/** +1 for the left side (at +x), -1 for the right. */
export const sgn = (s: Side) => (s === "L" ? 1 : -1);

/** The 29 base bone names (FrankenSim's staff rig manifest). */
export const REQUIRED_BONES: readonly string[] = [
  "root", "pelvis", "spine_01", "spine_02", "spine_03", "neck", "head", "gaze_target", "carry_chest",
  "clavicle_L", "clavicle_R", "upper_arm_L", "upper_arm_R", "forearm_L", "forearm_R", "hand_L", "hand_R",
  "grip_L", "grip_R", "claw_grip_L", "pinch_grip_R", "thigh_L", "thigh_R", "shin_L", "shin_R",
  "foot_L", "foot_R", "toe_L", "toe_R",
];
/** The finger bones added to them: three a digit, proximal to distal. */
export const FINGER_BONES: readonly string[] = SIDES.flatMap((s) => FINGERS.flatMap((f) => [1, 2, 3].map((k) => `${f}_0${k}_${s}`)));
export const ALL_BONES: readonly string[] = [...REQUIRED_BONES, ...FINGER_BONES];

function parentOf(b: string): string | null {
  if (b === "root") return null;
  const m = /^(.*)_([LR])$/.exec(b);
  const s = m ? m[2]! : "";
  const base = m ? m[1]! : b;
  const fm = /^(thumb|index|middle|ring|pinky)_0(\d)$/.exec(base);
  if (fm) return fm[2] === "1" ? `hand_${s}` : `${fm[1]}_0${Number(fm[2]) - 1}_${s}`;
  const table: Record<string, string> = {
    pelvis: "root",
    spine_01: "pelvis",
    spine_02: "spine_01",
    spine_03: "spine_02",
    neck: "spine_03",
    head: "neck",
    gaze_target: "head",
    carry_chest: "spine_02",
    clavicle: "spine_03",
    upper_arm: "clavicle",
    forearm: "upper_arm",
    hand: "forearm",
    grip: "hand",
    claw_grip: "hand",
    pinch_grip: "hand",
    thigh: "pelvis",
    shin: "thigh",
    foot: "shin",
    toe: "foot",
  };
  const p = table[base]!;
  return ["pelvis", "spine_01", "spine_02", "spine_03", "neck", "head", "root"].includes(p) ? p : `${p}_${s}`;
}

export interface DigitDef {
  /** Base joint (MCP, or CMC for the thumb), bind body space. */
  base: THREE.Vector3;
  /** Straight direction of the digit at bind. */
  dir: THREE.Vector3;
  /** Flexion axis: a positive turn about it curls the digit towards the palm. */
  k: THREE.Vector3;
  /** Back of the digit (where the nail is). */
  back: THREE.Vector3;
  len: readonly [number, number, number];
  rad: number;
}
export interface HandDef {
  side: Side;
  sd: number;
  wrist: THREE.Vector3;
  /** Distal along the hand, palm normal (facing in, towards the body at bind), thumb side. */
  da: THREE.Vector3;
  dn: THREE.Vector3;
  dt: THREE.Vector3;
  HL: number;
  breadth: number;
  sh: number;
  digits: Record<Finger, DigitDef>;
  /** Grip frames relative to the hand bone (bind orientation is the identity): tool (oblique), bar (power), palm. */
  toolPos: THREE.Vector3;
  toolQ: THREE.Quaternion;
  barPos: THREE.Vector3;
  barAxis: THREE.Vector3;
  hookPos: THREE.Vector3;
  palmPos: THREE.Vector3;
}

/** Everything a body's build and motion needs, derived from its row. */
export interface Body {
  d: Anthro;
  cohort: Cohort;
  s: number;
  lift: number;
  fem: number;
  J: Record<string, THREE.Vector3>;
  thigh: number;
  shin: number;
  upper: number;
  fore: number;
  footLen: number;
  heelBack: number;
  mtpFwd: number;
  toeFwd: number;
  shoeW: number;
  hipY: number;
  kneeY: number;
  ankY: number;
  shY: number;
  eyeY: number;
  topY: number;
  waistY: number;
  chestY: number;
  armpitY: number;
  neckBaseY: number;
  hemY: number;
  hands: Record<Side, HandDef>;
  /** Stance half-width (ankle centres) standing at work, and walking. */
  standHalf: number;
  walkHalf: number;
}

function digit(h: { wrist: THREE.Vector3; da: THREE.Vector3; dn: THREE.Vector3; dt: THREE.Vector3; sd: number }, a: number, n: number, t: number, fan: number, len: [number, number, number], rad: number): DigitDef {
  const base = h.wrist.clone().addScaledVector(h.da, a).addScaledVector(h.dn, n).addScaledVector(h.dt, t);
  const dir = h.da.clone().multiplyScalar(Math.cos(fan)).addScaledVector(h.dt, Math.sin(fan)).normalize();
  const k = V().crossVectors(dir, h.dn).normalize();
  return { base, dir, k, back: h.dn.clone().negate(), len, rad };
}

/** The body's joints and proportions for a cohort, standing on soles `sole` thick (default: the row's shoe sole). */
export function layout(cohort: Cohort, sole?: number): Body {
  const d = ANTHROPOMETRICS[cohort];
  const H = d.standing_height_m;
  const s = H / 1.715;
  const lift = sole ?? d.shoe_sole_m;
  const fem = cohort === "p05" ? 1 : cohort === "p50" ? 0.4 : 0;
  const hipY = d.hip_height_m + lift;
  const kneeY = d.knee_height_m + lift;
  const ankY = d.ankle_height_m + lift;
  const shY = d.shoulder_height_m + lift;
  const eyeY = d.eye_height_m + lift;
  const topY = H + lift;
  const SB = d.shoulder_breadth_m;
  const hx = 0.215 * SB * (1 + 0.07 * fem);
  const c7y = shY + 0.035 * s;
  const l5y = hipY + 0.085 * s;
  const span = c7y - l5y;
  const J: Record<string, THREE.Vector3> = {};
  J.root = V();
  J.pelvis = V(0, hipY, 0);
  J.spine_01 = V(0, l5y, -0.035 * s);
  J.spine_02 = V(0, l5y + 0.37 * span, -0.045 * s);
  J.spine_03 = V(0, l5y + 0.66 * span, -0.058 * s);
  J.neck = V(0, c7y, -0.066 * s);
  J.head = V(0, eyeY - 0.068 * s, -0.026 * s);
  J.gaze_target = V(0, eyeY, 0.25);
  J.carry_chest = V(0, J.spine_02.y + 0.06 * s, 0.16 * s);
  const footLen = d.shoe_length_m - 0.022;
  const mtpFwd = 0.5 * footLen;
  const hands = {} as Record<Side, HandDef>;
  for (const side of SIDES) {
    const sd = sgn(side);
    J[`clavicle_${side}`] = V(sd * 0.02 * s, shY - 0.03 * s, 0.05 * s);
    const gh = V(sd * (SB / 2 - 0.046 * s), shY - 0.035 * s, -0.012 * s);
    J[`upper_arm_${side}`] = gh;
    const el = gh.clone().add(V(sd * Math.sin(6 * DEG), -Math.cos(6 * DEG), 0).multiplyScalar(d.upper_arm_m));
    J[`forearm_${side}`] = el;
    const da = V(sd * Math.sin(10 * DEG), -Math.cos(10 * DEG), 0);
    const wr = el.clone().addScaledVector(da, d.forearm_m);
    J[`hand_${side}`] = wr;
    J[`thigh_${side}`] = V(sd * hx, hipY, 0);
    J[`shin_${side}`] = V(sd * hx * 0.965, kneeY, 0);
    J[`foot_${side}`] = V(sd * hx * 0.93, ankY, 0);
    J[`toe_${side}`] = V(sd * hx * 0.93, lift + 0.022 * s, mtpFwd);
    // The hand, in its own terms: distal, palm normal (medial), thumb side (forward at bind).
    const HL = d.hand_length_m;
    const sh = HL / 0.186;
    const B = 0.45 * HL;
    const dn = V(-sd * Math.cos(10 * DEG), -Math.sin(10 * DEG), 0);
    const dt = V(0, 0, 1);
    const h = { wrist: wr, da, dn, dt, sd };
    const digits: Record<Finger, DigitDef> = {
      index: digit(h, 0.505 * HL, -0.002 * sh, 0.37 * B, 4 * DEG, [0.225 * HL, 0.135 * HL, 0.112 * HL], 0.0093 * sh),
      middle: digit(h, 0.52 * HL, -0.002 * sh, 0.12 * B, 0, [0.245 * HL, 0.155 * HL, 0.116 * HL], 0.0096 * sh),
      ring: digit(h, 0.505 * HL, -0.002 * sh, -0.12 * B, -4 * DEG, [0.23 * HL, 0.148 * HL, 0.114 * HL], 0.009 * sh),
      pinky: digit(h, 0.462 * HL, 0, -0.35 * B, -9 * DEG, [0.182 * HL, 0.108 * HL, 0.104 * HL], 0.0079 * sh),
      thumb: digit(h, 0.13 * HL, 0.006 * sh, 0.36 * B, 0, [0.235 * HL, 0.168 * HL, 0.148 * HL], 0.0112 * sh),
    };
    // The thumb points distally, palmwards and out; it curls across the palm towards the little finger.
    const th = digits.thumb;
    th.dir.copy(da).multiplyScalar(0.8).addScaledVector(dn, 0.38).addScaledVector(dt, 0.46).normalize();
    const curl = orthoTo(dn.clone().multiplyScalar(0.55).addScaledVector(dt, -0.83), th.dir);
    th.k.crossVectors(th.dir, curl).normalize();
    th.back.copy(curl).negate();
    // Oblique grip: the handle crosses the palm from the heel of the hand on the little-finger side to the web
    // of the thumb, 55 degrees off the hand's long axis, so the fingers wrap round it and the thumb lies along it.
    const OBL = 55 * DEG;
    const zg = da.clone().multiplyScalar(Math.cos(OBL)).addScaledVector(dt, Math.sin(OBL)).normalize();
    const yg = orthoTo(da.clone().multiplyScalar(-Math.sin(OBL)).addScaledVector(dt, Math.cos(OBL)), zg);
    const xg = V().crossVectors(yg, zg);
    const toolQ = new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(xg, yg, zg));
    hands[side] = {
      side,
      sd,
      wrist: wr,
      da,
      dn,
      dt,
      HL,
      breadth: B,
      sh,
      digits,
      toolPos: da.clone().multiplyScalar(0.36 * HL).addScaledVector(dn, 0.026 * sh).addScaledVector(dt, 0.004 * sh),
      toolQ,
      barPos: da.clone().multiplyScalar(0.47 * HL).addScaledVector(dn, 0.024 * sh),
      barAxis: da.clone().multiplyScalar(0.22).addScaledVector(dt, 0.975).normalize(),
      hookPos: da.clone().multiplyScalar(0.6 * HL).addScaledVector(dn, 0.018 * sh),
      palmPos: da.clone().multiplyScalar(0.36 * HL).addScaledVector(dn, 0.016 * sh),
    };
    for (const f of FINGERS) {
      const g = digits[f];
      J[`${f}_01_${side}`] = g.base.clone();
      J[`${f}_02_${side}`] = g.base.clone().addScaledVector(g.dir, g.len[0]);
      J[`${f}_03_${side}`] = g.base.clone().addScaledVector(g.dir, g.len[0] + g.len[1]);
    }
    J[`grip_${side}`] = wr.clone().add(hands[side].toolPos);
  }
  J.claw_grip_L = J.hand_L!.clone().addScaledVector(hands.L.da, 0.62 * hands.L.HL).addScaledVector(hands.L.dn, 0.012);
  J.pinch_grip_R = J.hand_R!.clone().addScaledVector(hands.R.da, 0.78 * hands.R.HL).addScaledVector(hands.R.dn, 0.02).addScaledVector(hands.R.dt, 0.03);
  return {
    d,
    cohort,
    s,
    lift,
    fem,
    J,
    thigh: hipY - kneeY,
    shin: kneeY - ankY,
    upper: d.upper_arm_m,
    fore: d.forearm_m,
    footLen,
    heelBack: 0.21 * footLen,
    mtpFwd,
    toeFwd: 0.79 * footLen,
    shoeW: d.shoe_width_m,
    hipY,
    kneeY,
    ankY,
    shY,
    eyeY,
    topY,
    waistY: d.elbow_height_m + lift - 0.01 * s,
    chestY: shY - 0.135 * s,
    armpitY: shY - 0.09 * s,
    neckBaseY: c7y + 0.012 * s,
    hemY: hipY - 0.085 * s,
    hands,
    standHalf: hx * 1.02,
    walkHalf: Math.max(0.5 * d.shoe_width_m + 0.016, hx * 0.62),
  };
}


/** Bone index by name, in the skeleton's order. */
export const BI: Record<string, number> = Object.fromEntries(ALL_BONES.map((b, i) => [b, i]));
export const rigid = (bone: string): Wts => [[BI[bone]!, 1]];

/* ------------------------------------------------------------------------------------------------------------ */
/* The torso: the jacket's outer surface as horizontal superellipse sections, and how it follows the spine.       */
/* ------------------------------------------------------------------------------------------------------------ */

export interface Sect {
  a: number;
  f: number;
  b: number;
  c: number;
  n: number;
}
export interface Torso {
  sect(y: number): Sect;
  /** Point on the jacket surface at height y, angle th (0 front, +pi/2 the person's left). */
  at(y: number, th: number, out?: THREE.Vector3): THREE.Vector3;
  /** Outward horizontal normal at that point (approximate). */
  normal(y: number, th: number, out?: THREE.Vector3): THREE.Vector3;
  /** Angle at which the surface reaches x on the front, at height y. */
  angleAtX(y: number, x: number): number;
  hipHalf: number;
}

export function torsoModel(B: Body): Torso {
  const s = B.s;
  const sb = B.d.shoulder_breadth_m / 0.448;
  const cd = B.d.chest_depth_m / 0.238;
  const fem = B.fem;
  const hipHalf = 0.188 * s * (1 + 0.06 * fem);
  const waistHalf = 0.158 * s * (1 - 0.04 * fem);
  const keys: [number, number, number, number, number, number][] = [
    [B.hemY - 0.01, hipHalf * 1.04, 0.124 * cd, (0.138 + 0.006 * fem) * cd, 0, 2.2],
    [B.hipY, hipHalf, 0.116 * cd, (0.134 + 0.008 * fem) * cd, 0, 2.3],
    [(B.hipY + B.waistY) / 2, lerp(hipHalf, waistHalf, 0.55), 0.112 * cd, 0.118 * cd, 0.002, 2.35],
    [B.waistY, waistHalf, 0.111 * cd, 0.103 * cd, 0.004, 2.4],
    [B.chestY - 0.075 * s, 0.158 * sb, 0.12 * cd, 0.104 * cd, 0.002, 2.5],
    [B.chestY, 0.163 * sb, (0.132 + 0.01 * fem) * cd, 0.109 * cd, 0, 2.55],
    [B.armpitY, 0.16 * sb, 0.127 * cd, 0.111 * cd, -0.004, 2.6],
    [B.shY - 0.035 * s, 0.166 * sb, 0.106 * cd, 0.098 * cd, -0.012, 2.7],
    [B.shY + 0.004 * s, 0.158 * sb, 0.084 * cd, 0.086 * cd, -0.02, 2.6],
    [B.shY + 0.026 * s, 0.112 * sb, 0.07 * s, 0.074 * s, -0.03 * s, 2.3],
    [B.neckBaseY, 0.066 * s, 0.056 * s, 0.062 * s, -0.035 * s, 2.1],
  ];
  const ys = keys.map((k) => k[0]);
  const sp = [1, 2, 3, 4, 5].map((c) => new Mono(ys, keys.map((k) => k[c]!)));
  const sect = (y: number): Sect => ({ a: sp[0]!.at(y), f: sp[1]!.at(y), b: sp[2]!.at(y), c: sp[3]!.at(y), n: sp[4]!.at(y) });
  const at = (y: number, th: number, out = V()) => {
    const S = sect(y);
    const e = 2 / S.n;
    const cs = Math.cos(th);
    return out.set(S.a * spow(Math.sin(th), e), y, S.c + (cs >= 0 ? S.f : S.b) * spow(cs, e));
  };
  const _p = V();
  const _q = V();
  const normal = (y: number, th: number, out = V()) => {
    at(y, th - 0.01, _p);
    at(y, th + 0.01, _q);
    _q.sub(_p);
    return out.set(-_q.z, 0, _q.x).normalize();
  };
  const angleAtX = (y: number, x: number) => {
    const S = sect(y);
    const r = clamp(Math.abs(x) / S.a, 0, 0.999);
    return Math.sign(x) * Math.asin(Math.pow(r, S.n / 2));
  };
  return { sect, at, normal, angleAtX, hipHalf };
}

/** How the trunk's skin and cloth follow the spine, the clavicles and (below the hips) the thighs. */
/**
 * How much of a garment over the hips the thighs carry at height y: the same rule for the trousers, the jacket's
 * hem and a vest over it, so the layers move together and the inner never comes through the outer.
 */
export function legShare(B: Body, y: number): number {
  return smooth(-0.05 * B.s, 0.12 * B.s, B.hipY - y);
}

export function trunkW(B: Body, p: THREE.Vector3, thighShare = 0.5): Wts {
  const J = B.J;
  const s = B.s;
  const y = p.y;
  const band = 0.045 * s;
  const w = new Map<number, number>();
  const add = (b: string, x: number) => w.set(BI[b]!, (w.get(BI[b]!) ?? 0) + x);
  const cuts = [J.spine_01!.y - 0.015 * s, J.spine_02!.y, J.spine_03!.y];
  const chain = ["pelvis", "spine_01", "spine_02", "spine_03"];
  // Piecewise blend along the chain.
  let k = 0;
  while (k < 3 && y > cuts[k]! + band) k++;
  let spine: [string, number][];
  if (k < 3 && y > cuts[k]! - band) {
    const t = smooth(cuts[k]! - band, cuts[k]! + band, y);
    spine = [[chain[k]!, 1 - t], [chain[k + 1]!, t]];
  } else spine = [[chain[k]!, 1]];
  // Shoulders ride on the clavicles.
  const ax = Math.abs(p.x);
  const cw = 0.85 * smooth(0.06 * s, 0.15 * s, ax) * smooth(B.armpitY - 0.03 * s, B.shY - 0.015 * s, y);
  // Below the hip joints the hem is carried partly by the thighs.
  const tw = Math.min(1, 2 * thighShare) * legShare(B, y);
  const rest = (1 - cw) * (1 - tw);
  for (const [b, x] of spine) add(b, x * rest);
  if (cw > 0) add(p.x > 0 ? "clavicle_L" : "clavicle_R", cw * (1 - tw));
  if (tw > 0) {
    const wl = smooth(-0.05 * s, 0.05 * s, p.x);
    add("thigh_L", tw * wl);
    add("thigh_R", tw * (1 - wl));
  }
  return [...w.entries()];
}
/* ------------------------------------------------------------------------------------------------------------ */
/* Garments. Each builder takes paint by the part's own coordinate (a top's body by height, a sleeve or a trouser */
/* leg by distance from its joint), and the edges where the paint changes: there it puts a pair of rings a        */
/* millimetre apart, so that a colour block or a band of reflective tape has a crisp edge.                        */
/* ------------------------------------------------------------------------------------------------------------ */

/** Paint by a part's own coordinate u (a height, metres) at a bind-pose point p. */
export type PartPaint = (u: number, p: THREE.Vector3) => Paint;
/** Paint along a limb: t metres from its joint (shoulder or hip), at bind-pose point p. */
export type LimbPaint = (t: number, p: THREE.Vector3, side: Side) => Paint;

/** Merge `edges` (each as a ring pair `gap` apart) into the sorted stations `ts`, strictly inside (lo, hi). */
export function withEdges(ts: readonly number[], edges: readonly number[] | undefined, lo: number, hi: number, gap = 0.0012): number[] {
  const out = [...ts];
  for (const e of edges ?? []) {
    if (e - gap <= lo || e + gap >= hi) continue;
    // A station right at the edge would make a sliver: let the pair stand in for it.
    for (let i = out.length - 1; i >= 0; i--) if (Math.abs(out[i]! - e) < 0.008 && out[i]! > lo && out[i]! < hi) out.splice(i, 1);
    out.push(e - gap / 2, e + gap / 2);
  }
  return out.sort((a, b) => a - b);
}

export interface TopSpec {
  /** The body's paint by height. */
  body: PartPaint;
  /** A sleeve's paint by distance from the shoulder joint (negative over the cap). */
  sleeve: LimbPaint;
  /** Heights (below the armpit) and sleeve distances where the paint changes. */
  bodyEdges?: readonly number[];
  sleeveEdges?: readonly number[];
  /** Sleeves to the wrist (default) or rolled above the elbow. */
  sleeves?: "wrist" | "rolled";
  /** Hem height (default `B.hemY`: hip length, or a coverall's top half over its trousers). */
  hem?: number;
  /** The shirt collar's paint (default the body's at the neck). */
  collar?: Paint;
  /** A centre-front zip or button placket from the collar down to `placketTo` (height; default the hem). */
  placket?: Paint;
  placketTo?: number;
  /** Two flapped chest pockets in this paint. */
  pockets?: Paint;
  /** Room through the body over the torso model, metres (workwear is cut fuller than a fitted jacket). */
  ease?: number;
}

/** A point on a top's outer surface at height y, angle th (0 the front, +pi/2 the person's left), `lift` off it. */
export function topAt(B: Body, T: Torso, y: number, th: number, ease: number, lift = 0, out = V()): THREE.Vector3 {
  const k = ease * (1 - smooth(B.shY - 0.03 * B.s, B.neckBaseY, y)) + lift;
  return T.at(y, th, out).addScaledVector(T.normal(y, th, _tn), k);
}
const _tn = V();

/** A shirt, jacket or coverall top: the body over the torso model, a shirt collar, sleeves, placket and pockets. */
export function buildTop(soup: Soup, B: Body, T: Torso, spec: TopSpec, detail: Detail): void {
  const s = B.s;
  const ease = spec.ease ?? 0.004 * s;
  const nAround = detail === 2 ? 40 : detail === 1 ? 26 : 14;
  const y0 = spec.hem ?? B.hemY;
  const y1 = B.neckBaseY;
  const ySh = B.armpitY;
  const nBody = detail === 2 ? 18 : detail === 1 ? 11 : 6;
  const nTop = detail === 2 ? 8 : detail === 1 ? 6 : 3;
  let ys: number[] = [];
  for (let i = 0; i <= nBody; i++) ys.push(lerp(y0, ySh, i / nBody));
  for (let i = 1; i <= nTop; i++) ys.push(lerp(ySh, y1, Math.sin((i / nTop) * Math.PI * 0.5)));
  ys = withEdges(ys, spec.bodyEdges, y0, ySh);
  const rings: THREE.Vector3[][] = ys.map((y) => {
    const ring: THREE.Vector3[] = [];
    // The seam at the back.
    for (let j = 0; j < nAround; j++) ring.push(topAt(B, T, y, (j / nAround) * Math.PI * 2 + Math.PI, ease));
    return ring;
  });
  loft(soup, rings, { w: (_i, _j, p) => trunkW(B, p), color: (i, _j, p) => spec.body(ys[i]!, p) });

  // A shirt collar: the band up round the neck, the fall turned down over it, its points longer at the front.
  const cH = 0.036 * s;
  const cR0 = 0.071 * s;
  const cz = -0.03 * s;
  const nC = detail === 2 ? 24 : detail === 1 ? 16 : 8;
  const open = 0.3;
  const prof: [number, number, number][] =
    detail > 0
      ? [[-0.001, 0, 0], [-0.0015, 0.85, 0], [0.002, 1.04, 0], [0.008, 0.98, 1], [0.014, 0.6, 1], [0.019, 0.12, 1]]
      : [[0, 0, 0], [0.002, 1, 0], [0.017, 0.2, 1]];
  const cRings: THREE.Vector3[][] = [];
  for (let k = 0; k <= nC; k++) {
    const th = open + ((Math.PI * 2 - 2 * open) * k) / nC;
    const front = Math.max(0, Math.cos(th));
    const ring: THREE.Vector3[] = [];
    for (const [dr, h, fall] of prof) {
      const y = B.neckBaseY - 0.006 * s + h * cH - front * 0.012 * s - fall * front * front * 0.03 * s;
      const r = cR0 * (1 - 0.08 * h) + dr * s + fall * front * 0.014 * s;
      ring.push(V(Math.sin(th) * r * 1.02, y, cz + Math.cos(th) * r * (1 + 0.12 * front)));
    }
    cRings.push(ring);
  }
  const neckW = (p: THREE.Vector3): Wts => {
    const t = smooth(B.neckBaseY, B.neckBaseY + cH, p.y) * 0.45;
    return [[BI.spine_03!, 1 - t], [BI.neck!, t]];
  };
  const collar = spec.collar ?? spec.body(y1, V(0, y1, 0));
  loft(soup, cRings, { closed: false, w: (_i, _j, p) => neckW(p), color: collar, out: (_i, _j, p) => V(p.x, 0, p.z - cz) });

  // The placket: a strip down the front centre.
  if (spec.placket && detail > 0) {
    const yA = B.neckBaseY - 0.03 * s;
    const yB = Math.max(spec.placketTo ?? y0 + 0.01, y0 + 0.01);
    const n = detail === 2 ? 16 : 8;
    const pts: THREE.Vector3[] = [];
    const ns: THREE.Vector3[] = [];
    for (let k = 0; k <= n; k++) {
      const y = lerp(yA, yB, k / n);
      pts.push(topAt(B, T, y, 0, ease, 0.0012));
      ns.push(T.normal(y, 0));
    }
    tape(soup, pts, ns, 0.026 * s, 0.0024, (p) => trunkW(B, p), spec.placket, detail);
  }

  // Two flapped chest pockets.
  if (spec.pockets && detail > 0) {
    for (const sd of [1, -1]) {
      const xc = sd * 0.075 * s;
      const w = 0.054 * s;
      const yTop = B.chestY + 0.03 * s;
      const yBot = yTop - 0.13 * s;
      const patch = (ya: number, yb: number, lift: number, nu: number, nv: number) => {
        const rows: THREE.Vector3[][] = [];
        for (let i = 0; i <= nv; i++) {
          const y = lerp(ya, yb, i / nv);
          const row: THREE.Vector3[] = [];
          for (let j = 0; j <= nu; j++) {
            const th = T.angleAtX(y, xc + lerp(-w, w, j / nu));
            // The patch stands off the cloth most in its middle, as a filled pocket does.
            const bulge = Math.sin((Math.PI * j) / nu) * Math.sin((Math.PI * i) / nv);
            row.push(topAt(B, T, y, th, ease, lift + 0.0015 * bulge));
          }
          rows.push(row);
        }
        loft(soup, rows, { closed: false, w: (_i, _j, p) => trunkW(B, p), color: spec.pockets!, out: (_i, _j, p) => V(p.x, 0, p.z) });
      };
      patch(yBot, yTop, 0.0018, detail === 2 ? 5 : 3, detail === 2 ? 4 : 2);
      patch(yTop - 0.04 * s, yTop + 0.006 * s, 0.0042, detail === 2 ? 5 : 3, 1);
    }
  }

  for (const side of SIDES) buildSleeve(soup, B, side, spec, detail);
}

/** A sleeve from its cap in the shoulder to a turned-back cuff at the wrist, or a roll above the elbow. */
function buildSleeve(soup: Soup, B: Body, side: Side, spec: TopSpec, detail: Detail): void {
  const s = B.s;
  const sd = sgn(side);
  const gh = B.J[`upper_arm_${side}`]!;
  const el = B.J[`forearm_${side}`]!;
  const wr = B.J[`hand_${side}`]!;
  const up = V().subVectors(el, gh).normalize();
  const fo = V().subVectors(wr, el).normalize();
  const lu = B.upper;
  const lf = B.fore;
  const rolled = spec.sleeves === "rolled";
  const tEnd = rolled ? lu + 0.07 * s : lu + lf - 0.004 * s;
  const full = 1.04;
  // The cloth over the deltoid is fullest just below the shoulder point, easing to the elbow.
  const rAt = (t: number) => {
    if (t < lu) return lerp(0.0565, 0.049, smooth(0.03 * s, lu, t)) * s * full;
    return lerp(0.05, rolled ? 0.046 : 0.042, clamp((t - lu) / lf, 0, 1)) * s * full;
  };
  // The cap's crown stands only a little over the shoulder line (a workwear sleeve is set in low, not puffed).
  const capTop = -0.034 * s;
  const capN = detail === 2 ? 7 : detail === 1 ? 4 : 2;
  const cap: number[] = [];
  for (let i = 0; i < capN; i++) cap.push(capTop * Math.cos(((i / capN) * Math.PI) / 2));
  const nU = detail === 2 ? 9 : detail === 1 ? 6 : 3;
  const tRollStart = tEnd - (rolled ? 0.05 : 0.07) * s;
  let mid: number[] = [];
  for (let i = 0; i <= nU; i++) mid.push(lerp(0, tRollStart, i / nU));
  // Rings round the elbow so the bend stays round.
  if (detail > 0 && !rolled) for (const dt of [-0.04, -0.015, 0.015, 0.04]) mid.push(lu + dt * s);
  mid = withEdges(mid.sort((a, b) => a - b), spec.sleeveEdges, 0, tRollStart);
  // The cuff (or the roll): a fat fold that tucks in round the forearm.
  const tail: [number, number][] = rolled
    ? [[0.008, 0.004], [0.016, 0.011], [0.026, 0.015], [0.036, 0.014], [0.045, 0.009], [0.05, 0.001], [0.051, -0.006]]
    : [[0.0, 0.004], [0.006, 0.0055], [0.064, 0.0055], [0.069, 0.003], [0.071, -0.004]];
  const ts = [...cap, ...mid.slice(1)];
  const extra: number[] = [];
  for (const [dt, dr] of tail) {
    ts.push(tRollStart + dt * s);
    extra.push(dr * s);
  }
  const nA = detail === 2 ? 14 : detail === 1 ? 10 : 6;
  const rings: THREE.Vector3[][] = [];
  const nTail = tail.length;
  const centre = (t: number) => (t <= lu ? gh.clone().addScaledVector(up, t) : el.clone().addScaledVector(fo, t - lu));
  for (let i = 0; i < ts.length; i++) {
    const t = ts[i]!;
    const iTail = i - (ts.length - nTail);
    const c = centre(t);
    const dir = t < lu - 0.04 * s ? up : t > lu + 0.04 * s ? fo : up.clone().lerp(fo, smooth(lu - 0.04 * s, lu + 0.04 * s, t)).normalize();
    const u = orthoTo(V(sd, 0, 0), dir);
    const v = V().crossVectors(dir, u).multiplyScalar(sd);
    let r = rAt(Math.max(0, t));
    let rz = 1.06;
    if (t < 0) {
      // The cap: the deltoid's dome, its crown drawn in towards the neck so that the top's shoulder line runs on
      // over it into the sleeve as one curve (its inner half lies inside the shoulder, unseen).
      const k = Math.sqrt(Math.max(0, 1 - (t / capTop) ** 2));
      r = rAt(0) * (0.25 + 0.75 * k);
      rz = 1.1;
      c.add(V(-sd * 0.022 * s * (1 - k), 0, 0));
    }
    if (iTail >= 0) r += extra[iTail]!;
    const ring: THREE.Vector3[] = [];
    for (let j = 0; j < nA; j++) {
      const a = (j / nA) * Math.PI * 2;
      ring.push(c.clone().addScaledVector(u, Math.cos(a) * r).addScaledVector(v, Math.sin(a) * r * rz));
    }
    rings.push(ring);
  }
  const iClav = BI[`clavicle_${side}`]!;
  const iUp = BI[`upper_arm_${side}`]!;
  const iFo = BI[`forearm_${side}`]!;
  loft(soup, rings, {
    w: (i, _j, p) => {
      const t = ts[i]!;
      if (t < 0.03 * s) {
        // The cap's inner half (towards the neck) rides more with the shoulder girdle than with the arm.
        const k = smooth(capTop, 0.03 * s, t);
        const inner = smooth(0, -0.03 * s, sd * (p.x - gh.x));
        const wc = (0.3 + 0.35 * inner) * (1 - k);
        return [[iClav, wc], [iUp, 1 - wc]];
      }
      const e = smooth(lu - 0.035 * s, lu + 0.035 * s, t);
      return [[iUp, 1 - e], [iFo, e]];
    },
    color: (i, _j, p) => spec.sleeve(ts[i]!, p, side),
    out: (i, _j, p) => p.clone().sub(centre(ts[i]!)),
  });
}

export interface TrouserSpec {
  /** The seat's paint by height. */
  seat: PartPaint;
  /** A leg's paint by distance from the hip joint. */
  leg: LimbPaint;
  /** Distances along the leg where the paint changes. */
  legEdges?: readonly number[];
  /** Hem height above the floor at the back (default: 3.4 cm over the ankle, a shoe's trousers). */
  hem?: number;
  /** Radius scale (workwear is cut fuller; over boots fuller still). Default 1. */
  ease?: number;
  /** Cargo pockets on the outside of the thighs. */
  cargo?: Paint;
}

/** Trousers (or a coverall's legs): the seat closing between the legs, and two legs breaking over the footwear. */
export function buildTrousers(soup: Soup, B: Body, T: Torso, spec: TrouserSpec, detail: Detail): void {
  const s = B.s;
  const k = spec.ease ?? 1;
  const nA = detail === 2 ? 18 : detail === 1 ? 14 : 8;
  // The seat: from the waist (under the top) down to the crotch, closing between the legs.
  const yTop = B.waistY - 0.03 * s;
  const yC = B.hipY - 0.078 * s;
  const seatYs: number[] = [];
  const nS = detail === 2 ? 8 : 4;
  for (let i = 0; i <= nS; i++) seatYs.push(lerp(yTop, yC + 0.02 * s, i / nS));
  const close: [number, number, number][] = [[yC + 0.008 * s, 0.86, 0.8], [yC, 0.62, 0.55], [yC - 0.004 * s, 0.3, 0.25]];
  const seat: THREE.Vector3[][] = [];
  const seatY: number[] = [];
  const ring = (y: number, kx: number, kz: number) => {
    const r: THREE.Vector3[] = [];
    for (let j = 0; j < nA; j++) {
      const p = T.at(Math.max(y, B.hemY), (j / nA) * Math.PI * 2 + Math.PI);
      r.push(V(p.x * 0.955 * kx * (0.5 + 0.5 * k), y, p.z * 0.95 * kz * (0.5 + 0.5 * k)));
    }
    return r;
  };
  for (const y of seatYs) {
    seat.push(ring(y, 1, 1));
    seatY.push(y);
  }
  for (const [y, kx, kz] of close) {
    seat.push(ring(y, kx, kz));
    seatY.push(y);
  }
  const seatW = (p: THREE.Vector3): Wts => {
    const tw = legShare(B, p.y);
    const wl = smooth(-0.05 * s, 0.05 * s, p.x);
    return [[BI.pelvis!, 1 - tw], [BI.thigh_L!, tw * wl], [BI.thigh_R!, tw * (1 - wl)]];
  };
  loft(soup, seat, { w: (_i, _j, p) => seatW(p), color: (i, _j, p) => spec.seat(seatY[i]!, p) });

  for (const side of SIDES) {
    const sd = sgn(side);
    const hip = B.J[`thigh_${side}`]!;
    const knee = B.J[`shin_${side}`]!;
    const ank = B.J[`foot_${side}`]!;
    const th = V().subVectors(knee, hip).normalize();
    const sh = V().subVectors(ank, knee).normalize();
    const lt = B.thigh;
    const ls = B.shin;
    const hem = spec.hem ?? 2 * B.ankY - B.lift - 0.034 * s;
    const tHem = lt + ls - (hem - B.ankY);
    const rAt = (t: number) => {
      const key: [number, number][] = [[-0.08, 0.07], [0.0, 0.085], [0.06, 0.097], [0.16, 0.09], [lt - 0.06, 0.071], [lt, 0.066], [lt + 0.12, 0.066], [lt + ls * 0.75, 0.064], [tHem, 0.069]];
      for (let i = 1; i < key.length; i++) if (t <= key[i]![0]) return lerp(key[i - 1]![1], key[i]![1], smooth(key[i - 1]![0], key[i]![0], t)) * s;
      return key[key.length - 1]![1] * s;
    };
    // Fuller below the knee than above it, where the cloth hangs free.
    const fullAt = (t: number) => lerp(1 + 0.6 * (k - 1), k, smooth(lt - 0.1, lt + 0.1, t));
    let ts: number[] = [];
    const nT = detail === 2 ? 16 : detail === 1 ? 11 : 6;
    for (let i = 0; i <= nT; i++) ts.push(lerp(-0.08 * s, tHem, i / nT));
    // Extra rings round the knee so the bend stays round.
    if (detail > 0) for (const dt of [-0.03, -0.012, 0.012, 0.03]) ts.push(lt + dt * s);
    ts.sort((a, b) => a - b);
    ts = withEdges(ts, spec.legEdges, -0.08 * s, tHem);
    const rings: THREE.Vector3[][] = [];
    const lip = detail > 0 ? 1 : 0;
    const all = [...ts];
    if (lip) all.push(tHem + 0.0005);
    for (let i = 0; i < all.length; i++) {
      const t = all[i]!;
      const isLip = lip && i === all.length - 1;
      const c = t <= lt ? hip.clone().addScaledVector(th, t) : knee.clone().addScaledVector(sh, t - lt);
      const dir = t < lt - 0.05 * s ? th : t > lt + 0.05 * s ? sh : th.clone().lerp(sh, smooth(lt - 0.05 * s, lt + 0.05 * s, t)).normalize();
      const u = orthoTo(V(sd, 0, 0), dir);
      const v = V().crossVectors(u, dir).multiplyScalar(-sd);
      const fwd = orthoTo(Z.clone(), dir);
      const r = rAt(Math.min(t, tHem)) * fullAt(t) - (isLip ? 0.004 * s : 0);
      const near = smooth(tHem - 0.08 * s, tHem, t);
      const rr: THREE.Vector3[] = [];
      for (let j = 0; j < nA; j++) {
        // Start on the inside seam, so the pattern's join is hidden between the legs.
        const a = (j / nA) * Math.PI * 2 + Math.PI;
        const p = c.clone().addScaledVector(u, Math.cos(a) * r * 0.97).addScaledVector(v, Math.sin(a) * r * 1.04);
        // The hem breaks over the instep: higher at the front.
        const front = Math.max(0, V().subVectors(p, c).normalize().dot(fwd));
        p.addScaledVector(dir, -near * front * front * 0.03 * s);
        rr.push(p);
      }
      rings.push(rr);
    }
    const iTh = BI[`thigh_${side}`]!;
    const iSh = BI[`shin_${side}`]!;
    const iFt = BI[`foot_${side}`]!;
    const legW = (t: number, p: THREE.Vector3): Wts => {
      if (t < 0.12 * s) {
        // legShare's rule, measured along the thigh.
        const q = smooth(-0.05 * s, 0.12 * s, t);
        return [[BI.pelvis!, 1 - q], [iTh, q]];
      }
      const e = smooth(lt - 0.055 * s, lt + 0.055 * s, t);
      // The hem's front rides on the boot.
      const front = clamp((p.z - ank.z) / (0.06 * s), 0, 1) * smooth(tHem - 0.05 * s, tHem, t) * 0.45;
      return [[iTh, 1 - e], [iSh, e * (1 - front)], [iFt, e * front]];
    };
    const centre = (t: number) => (t <= lt ? hip.clone().addScaledVector(th, t) : knee.clone().addScaledVector(sh, t - lt));
    loft(soup, rings, {
      w: (i, _j, p) => legW(all[i]!, p),
      color: (i, _j, p) => spec.leg(all[i]!, p, side),
      out: (i, _j, p) => p.clone().sub(centre(all[i]!)),
    });
    // A cargo pocket on the outside of the thigh, with its flap.
    if (spec.cargo && detail > 0) {
      const u = orthoTo(V(sd, 0, 0), th);
      const v = V().crossVectors(u, th).multiplyScalar(-sd);
      const pocket = (t0: number, t1: number, lift: number) => {
        const rows: THREE.Vector3[][] = [];
        const nv = detail === 2 ? 4 : 2;
        const nu = detail === 2 ? 4 : 2;
        for (let i = 0; i <= nv; i++) {
          const t = lerp(t0, t1, i / nv);
          const c = centre(t);
          const r = rAt(t) * fullAt(t);
          const row: THREE.Vector3[] = [];
          for (let j = 0; j <= nu; j++) {
            const a = lerp(-0.62, 0.38, j / nu);
            const bulge = Math.sin((Math.PI * j) / nu) * Math.sin((Math.PI * i) / nv);
            const rr = r + lift + 0.004 * s * bulge;
            row.push(c.clone().addScaledVector(u, Math.cos(a) * rr * 0.97).addScaledVector(v, Math.sin(a) * rr * 1.04));
          }
          rows.push(row);
        }
        loft(soup, rows, { closed: false, w: (i) => legW(lerp(t0, t1, i / nv), V()), color: spec.cargo!, out: (_i, _j, p) => p.clone().sub(hip).projectOnPlane(th) });
      };
      pocket(0.36 * lt, 0.78 * lt, 0.002);
      pocket(0.33 * lt, 0.42 * lt, 0.0055);
    }
  }
}

/** A flat tape (a strap, a band, a lace) along a path, lying on surfaces whose normals are `nrm`. */
export function tape(soup: Soup, pts: THREE.Vector3[], nrm: THREE.Vector3[], width: number, thick: number, w: WFn, c: Tint, detail: Detail): void {
  const rings: THREE.Vector3[][] = [];
  const n = detail === 2 ? 6 : 4;
  for (let i = 0; i < pts.length; i++) {
    const p = pts[i]!;
    const tan = V().subVectors(pts[Math.min(i + 1, pts.length - 1)]!, pts[Math.max(i - 1, 0)]!).normalize();
    const nn = orthoTo(nrm[i]!.clone(), tan);
    const side = V().crossVectors(tan, nn);
    const r: THREE.Vector3[] = [];
    for (let j = 0; j < n; j++) {
      const a = (j / n) * Math.PI * 2;
      r.push(p.clone().addScaledVector(side, (Math.cos(a) * width) / 2).addScaledVector(nn, (spow(Math.sin(a), 0.5) * thick) / 2));
    }
    rings.push(r);
  }
  // Close the ends.
  rings.unshift(rings[0]!.map(() => pts[0]!.clone()));
  rings.push(rings[rings.length - 1]!.map(() => pts[pts.length - 1]!.clone()));
  loft(soup, rings, { w: (_i, _j, p) => w(p), color: c });
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Footwear: lace-up safety boots or rubber gumboots on thick lugged soles                                        */
/* ------------------------------------------------------------------------------------------------------------ */

export interface BootSpec {
  /** Lace-up leather safety boots, or rubber gumboots. */
  kind: "lace" | "gum";
  upper: Paint;
  /** The outsole (the tread). */
  sole: Paint;
  /** The midsole above it (default the outsole's). */
  midsole?: Paint;
  laces?: Paint;
  /** A toe cap's scuff guard (default the upper's). */
  toe?: Paint;
  /** The shaft's top above the floor (default lace 0.16 m, gum 0.34 m, scaled to the body). */
  shaft?: number;
}

/** Boots on soles `B.lift` thick: a steel toe box, a padded collar, laces up the front or a rubber shaft. */
export function buildBoots(soup: Soup, B: Body, spec: BootSpec, detail: Detail): void {
  const s = B.s;
  const W = B.shoeW * 1.08;
  const gum = spec.kind === "gum";
  const z0 = -B.heelBack - 0.016 * s;
  const z1 = B.toeFwd + 0.024 * s;
  const sole = B.lift;
  const nZ = detail === 2 ? 16 : detail === 1 ? 9 : 4;
  const nA = detail === 2 ? 16 : detail === 1 ? 10 : 6;
  const width = (z: number) => {
    const u = (z - z0) / (z1 - z0);
    const k: [number, number][] = [[0, 0.66], [0.12, 0.76], [0.4, 0.86], [0.66, 1.02], [0.86, 0.98], [1, 0.66]];
    for (let i = 1; i < k.length; i++) if (u <= k[i]![0]) return lerp(k[i - 1]![1], k[i]![1], smooth(k[i - 1]![0], k[i]![0], u)) * W;
    return k[k.length - 1]![1] * W;
  };
  // The upper's height over the sole: up round the ankle, the steel toe box standing tall to the front.
  const height = (z: number) => {
    const u = (z - z0) / (z1 - z0);
    const k: [number, number][] = [[0, 0.08], [0.18, 0.088], [0.38, 0.092], [0.55, 0.07], [0.68, 0.058], [0.82, 0.054], [0.93, 0.046], [1, 0.024]];
    for (let i = 1; i < k.length; i++) if (u <= k[i]![0]) return lerp(k[i - 1]![1], k[i]![1], smooth(k[i - 1]![0], k[i]![0], u)) * s;
    return k[k.length - 1]![1] * s;
  };
  // The toe spring: the sole's front curves up off the floor.
  const spring = (z: number) => 0.011 * s * smooth(B.mtpFwd + 0.01 * s, z1, z) ** 1.6;
  const zs: number[] = [];
  for (let i = 0; i <= nZ; i++) zs.push(lerp(z0, z1, 0.5 - 0.5 * Math.cos((i / nZ) * Math.PI)));
  const midsole = spec.midsole ?? spec.sole;
  const toe = spec.toe ?? spec.upper;
  const shaftTop = (spec.shaft ?? (gum ? 0.34 : 0.16)) * s;
  for (const side of SIDES) {
    const ank = B.J[`foot_${side}`]!;
    const iF = BI[`foot_${side}`]!;
    const iT = BI[`toe_${side}`]!;
    const iS = BI[`shin_${side}`]!;
    const wt = (p: THREE.Vector3): Wts => {
      const e = smooth(B.mtpFwd - 0.02 * s, B.mtpFwd + 0.02 * s, p.z);
      // The upper's back and top ride with the shin a little above the ankle.
      const up = smooth(B.ankY - 0.01 * s, B.ankY + 0.04 * s, p.y) * smooth(B.mtpFwd * 0.4, -0.02, p.z);
      return [[iF, (1 - e) * (1 - up)], [iT, e * (1 - up)], [iS, up]];
    };
    const at = (x: number, y: number, z: number) => V(ank.x + x, y, z);
    // Sole: a rounded slab, heel and toe domed; the tread's band darker than the midsole above it.
    const soleRings: THREE.Vector3[][] = [];
    const domes = detail > 0 ? [0.0, 0.35, 0.7] : [0];
    const ns = detail === 2 ? 12 : detail === 1 ? 8 : 6;
    const soleRing = (z: number, k: number) => {
      const hw = width(z) / 2 + 0.005 * s;
      const y0 = spring(z);
      const r: THREE.Vector3[] = [];
      for (let j = 0; j < ns; j++) {
        const a = (j / ns) * Math.PI * 2;
        const cx = Math.cos(a);
        const sy = Math.sin(a);
        const yy = y0 + (sole * (1 + spow(sy, 0.25))) / 2;
        r.push(at(hw * spow(cx, 0.3) * k, lerp(y0 + sole / 2, yy, k), z));
      }
      return r;
    };
    for (const k of domes) soleRings.push(soleRing(z0 - 0.004 * s * (1 - k), k * 0.9 + 0.05));
    for (const z of zs) soleRings.push(soleRing(z, 1));
    for (const k of [...domes].reverse()) soleRings.push(soleRing(z1 + 0.004 * s * (1 - k), k * 0.9 + 0.05));
    loft(soup, soleRings, { w: (_i, _j, p) => wt(p), color: (_i, _j, p) => (p.y - spring(p.z) < sole * 0.5 ? spec.sole : midsole) });
    // Lugs round the tread's edge (close up only).
    if (detail === 2) {
      const nL = 13;
      for (let k = 0; k < nL; k++) {
        const z = lerp(z0 + 0.02 * s, z1 - 0.03 * s, (k + 0.5) / nL);
        for (const sx of [1, -1]) {
          const x = sx * (width(z) / 2 + 0.006 * s);
          const p = at(x, spring(z) + 0.007 * s, z);
          addGeo(soup, new THREE.BoxGeometry(0.008 * s, 0.012 * s, 0.013 * s), mat4(p), wt(p), spec.sole);
        }
      }
    }
    // Upper: a dome over the foot standing on the sole, the toe box rounder and higher than a shoe's.
    const upRings: THREE.Vector3[][] = [];
    const upZ: number[] = [];
    const upRing = (z: number, k: number) => {
      const hw = (width(z) / 2) * k;
      const h = height(z) * (0.25 + 0.75 * k);
      const y0 = spring(z) + sole - 0.002 * s;
      const r: THREE.Vector3[] = [];
      for (let j = 0; j < nA; j++) {
        const a = (j / nA) * Math.PI * 2;
        const sy = Math.sin(a);
        r.push(at(hw * spow(Math.cos(a), 0.5), y0 + (sy > 0 ? h * spow(sy, 0.7) : 0.002 * s * sy), z));
      }
      upZ.push(z);
      return r;
    };
    upRings.push(upRing(z0 + 0.004 * s, 0.35));
    upRings.push(upRing(z0 + 0.01 * s, 0.8));
    for (const z of zs) if (z > z0 + 0.014 * s && z < z1 - 0.012 * s) upRings.push(upRing(z, 1));
    upRings.push(upRing(z1 - 0.007 * s, 0.8));
    upRings.push(upRing(z1 - 0.002 * s, 0.35));
    const toeFrom = B.toeFwd - 0.02 * s;
    loft(soup, upRings, { w: (_i, _j, p) => wt(p), color: (i, _j, p) => (upZ[i]! > toeFrom && p.y < spring(p.z) + sole + 0.035 * s ? toe : spec.upper) });
    // The shaft: up round the ankle from inside the upper to the collar (gumboots on up the calf).
    const shaftRings: THREE.Vector3[][] = [];
    const shaftYs: number[] = [];
    const nSh = detail === 2 ? 16 : detail === 1 ? 10 : 6;
    const y0 = sole + 0.03 * s;
    const levels = gum ? [0, 0.2, 0.45, 0.7, 0.9, 1] : [0, 0.35, 0.65, 0.88, 1];
    const rad = (u: number) => (gum ? lerp(0.05, 0.062, smooth(0.2, 1, u)) : lerp(0.047, 0.05, u)) * s;
    for (const u of levels) {
      const y = lerp(y0, shaftTop, u);
      const r: THREE.Vector3[] = [];
      for (let j = 0; j < nSh; j++) {
        const a = (j / nSh) * Math.PI * 2;
        // Fuller at the back (the heel counter and the calf), flatter at the front over the tongue.
        const back = Math.cos(a) < 0 ? 1.08 : 0.96;
        r.push(V(ank.x + Math.sin(a) * rad(u) * 0.95, y, ank.z - 0.012 * s + Math.cos(a) * rad(u) * back + (gum ? 0.004 * s * u : 0)));
      }
      shaftRings.push(r);
      shaftYs.push(y);
    }
    // The padded collar (lace boots) or the rolled rim (gumboots): a fat lip turned in at the top.
    const top = shaftRings[shaftRings.length - 1]!;
    const lipR = gum ? 0.004 * s : 0.008 * s;
    const lipRing = (dy: number, dr: number) =>
      top.map((p) => {
        const d = V(p.x - ank.x, 0, p.z - (ank.z - 0.012 * s)).normalize();
        return p.clone().addScaledVector(d, dr).add(V(0, dy, 0));
      });
    shaftRings.push(lipRing(0.004 * s, lipR), lipRing(0.009 * s, lipR * 0.4), lipRing(0.006 * s, -0.006 * s));
    shaftYs.push(shaftTop, shaftTop, shaftTop);
    const shaftW = (p: THREE.Vector3): Wts => {
      const e = smooth(B.ankY - 0.015 * s, B.ankY + 0.03 * s, p.y);
      return [[iF, 1 - e], [iS, e]];
    };
    loft(soup, shaftRings, { w: (_i, _j, p) => shaftW(p), color: spec.upper });
    if (!gum && detail > 0 && spec.laces) {
      // Laces across the tongue up the front of the shaft and over the instep, criss-crossed.
      const n = detail === 2 ? 6 : 4;
      for (let k = 0; k < n; k++) {
        const u = k / (n - 1);
        const z = lerp(0.045, -0.005, u) * s;
        const y = lerp(sole + height(0.045 * s) + 0.004 * s, shaftTop - 0.012 * s, u);
        const hw = lerp(0.022, 0.03, u) * s;
        const p = at(0, y, ank.z - 0.012 * s + lerp(0.055, 0.047, u) * s + 0.003 * s + (z - 0.02 * s) * 0.1);
        for (const tilt of detail === 2 ? [0.32, -0.32] : [0]) {
          const q = new THREE.Quaternion().setFromEuler(new THREE.Euler(0.15, 0, Math.PI / 2 + tilt));
          addGeo(soup, new THREE.CylinderGeometry(0.0022 * s, 0.0022 * s, 2 * hw, 5), mat4(p, q), shaftW(p), spec.laces);
        }
      }
    }
    if (!gum && detail === 2) {
      // A pull loop at the back of the collar.
      const p = at(0, shaftTop + 0.012 * s, ank.z - 0.012 * s - rad(1) * 1.08 - 0.004 * s);
      addGeo(soup, new THREE.TorusGeometry(0.011 * s, 0.0026 * s, 5, 10, Math.PI), mat4(p, new THREE.Quaternion().setFromEuler(new THREE.Euler(0, Math.PI / 2, 0))), shaftW(p), spec.upper);
    }
  }
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Forearms and hands, bare or gloved                                                                             */
/* ------------------------------------------------------------------------------------------------------------ */

export interface HandSpec {
  /** Skin (with `material` the skin one) or the glove's back. */
  paint: Paint;
  /** A glove: its thickness over the skin, its palm (a dipped grip coating), and its gauntlet over the cuff. */
  glove?: { thick: number; palm?: Paint; cuff?: Paint; cuffLen?: number };
  /** Where the arm starts, metres from the elbow along the forearm (default: just inside a wrist-length cuff). */
  from?: number;
  /** Nails, for bare hands (close up). */
  nails?: Paint;
}

/** One forearm and hand, its fingers on their bones: skin, or a work glove with a gauntlet cuff. */
export function buildHand(soup: Soup, B: Body, side: Side, spec: HandSpec, detail: Detail): void {
  const s = B.s;
  const H = B.hands[side];
  const sh = H.sh;
  const HL = H.HL;
  const el = B.J[`forearm_${side}`]!;
  const fo = B.fore;
  const g = spec.glove;
  const thick = g?.thick ?? 0;
  const palmP = g?.palm ?? spec.paint;
  const t0 = clamp(spec.from ?? (g ? fo - 0.03 * s : fo - 0.055 * s), 0.03 * s, fo - 0.01 * s);
  // Stations from the elbow along the forearm and on through the palm to the knuckles: [t, r_thumbside, r_palm, n].
  const prof: [number, number, number, number][] = [
    [0.1 * fo, 0.04 * s, 0.037 * s, 2.0],
    [0.5 * fo, 0.034 * s, 0.029 * s, 2.0],
    [fo - 0.035 * s, 0.028 * s, 0.02 * s, 2.1],
    [fo, 0.0272 * sh, 0.0182 * sh, 2.3],
    [fo + 0.14 * HL, 0.0335 * sh, 0.0168 * sh, 2.8],
    [fo + 0.34 * HL, 0.0385 * sh, 0.0152 * sh, 3.0],
    [fo + 0.49 * HL, 0.0405 * sh, 0.0135 * sh, 3.0],
    [fo + 0.535 * HL, 0.036 * sh, 0.0105 * sh, 2.6],
    [fo + 0.56 * HL, 0.022 * sh, 0.0065 * sh, 2.2],
    [fo + 0.568 * HL, 0.006 * sh, 0.002 * sh, 2.0],
  ];
  const pts = prof.filter((p) => p[0] >= t0 - 1e-6);
  pts.unshift([t0, ...(() => {
    const i = prof.findIndex((p) => p[0] > t0);
    const a = prof[Math.max(0, i - 1)]!;
    const b = prof[i]!;
    const u = clamp((t0 - a[0]) / Math.max(1e-6, b[0] - a[0]), 0, 1);
    return [lerp(a[1], b[1], u), lerp(a[2], b[2], u), lerp(a[3], b[3], u)] as [number, number, number];
  })()]);
  const ts: number[] = [];
  const steps = detail === 2 ? 2 : 1;
  for (let i = 0; i < pts.length - 1; i++) for (let k = 0; k < steps; k++) ts.push(lerp(pts[i]![0], pts[i + 1]![0], k / steps));
  ts.push(pts[pts.length - 1]![0]);
  const tt = pts.map((p) => p[0]);
  const fR = new Mono(tt, pts.map((p) => p[1] + thick));
  const fN = new Mono(tt, pts.map((p) => p[2] + thick));
  const fE = new Mono(tt, pts.map((p) => p[3]));
  const nA = detail === 2 ? 14 : detail === 1 ? 10 : 5;
  const rings: THREE.Vector3[][] = [];
  const palmSide: boolean[] = [];
  for (const t of ts) {
    const c = el.clone().addScaledVector(H.da, t);
    const rt = fR.at(t);
    const rn = fN.at(t);
    const e = 2 / fE.at(t);
    const palm = smooth(fo, fo + 0.1 * HL, t);
    c.addScaledVector(H.dn, 0.0025 * sh * palm);
    const ring: THREE.Vector3[] = [];
    for (let j = 0; j < nA; j++) {
      const a = (j / nA) * Math.PI * 2;
      const sn = Math.sin(a);
      // The back of the hand is flatter than the palm.
      const kn = sn < 0 ? 1 - 0.18 * palm : 1;
      ring.push(c.clone().addScaledVector(H.dt, spow(Math.cos(a), e) * rt).addScaledVector(H.dn, spow(sn, e) * rn * kn));
      if (rings.length === 0) palmSide.push(sn > 0.2);
    }
    rings.push(ring);
  }
  const iFo = BI[`forearm_${side}`]!;
  const iHa = BI[`hand_${side}`]!;
  const fingerAt = (p: THREE.Vector3) => {
    const t = V().subVectors(p, H.wrist).dot(H.dt);
    let best: Finger = "middle";
    let bd = 1e9;
    for (const f of ["index", "middle", "ring", "pinky"] as const) {
      const d = Math.abs(V().subVectors(H.digits[f].base, H.wrist).dot(H.dt) - t);
      if (d < bd) {
        bd = d;
        best = f;
      }
    }
    return BI[`${best}_01_${side}`]!;
  };
  loft(soup, rings, {
    w: (i, _j, p) => {
      const t = ts[i]!;
      const e = smooth(fo - 0.018 * s, fo + 0.012 * s, t);
      const k = 0.5 * smooth(fo + 0.42 * HL, fo + 0.545 * HL, t);
      return [[iFo, 1 - e], [iHa, e * (1 - k)], [fingerAt(p), e * k]];
    },
    color: (i, j) => (g && ts[i]! > fo + 0.04 * HL && palmSide[j] ? palmP : spec.paint),
    out: (i, _j, p) => p.clone().sub(el.clone().addScaledVector(H.da, ts[i]!)),
  });
  // A glove's gauntlet: a short flared tube over the sleeve's cuff.
  if (g && g.cuff !== undefined) {
    const len = g.cuffLen ?? 0.075 * s;
    const ca = fo - len;
    const cb = fo + 0.02 * HL;
    const n = detail === 2 ? 5 : 3;
    const cr: THREE.Vector3[][] = [];
    const cts: number[] = [];
    for (let i = 0; i <= n; i++) {
      const u = i / n;
      const t = lerp(ca, cb, u);
      const r = lerp(0.05, 0.036, smooth(0, 1, u)) * s + thick;
      const c = el.clone().addScaledVector(H.da, t);
      const ring: THREE.Vector3[] = [];
      for (let j = 0; j < nA; j++) {
        const a = (j / nA) * Math.PI * 2;
        ring.push(c.clone().addScaledVector(H.dt, Math.cos(a) * r).addScaledVector(H.dn, Math.sin(a) * r * 0.86));
      }
      cr.push(ring);
      cts.push(t);
    }
    // The turned edge at the open end.
    cr.unshift(cr[0]!.map((p) => p.clone().addScaledVector(H.da, 0.004 * s).lerp(el.clone().addScaledVector(H.da, ca + 0.004 * s), 0.08)));
    cts.unshift(ca + 0.004 * s);
    loft(soup, cr, {
      w: (i) => {
        const e = smooth(fo - 0.02 * s, fo + 0.01 * s, cts[i]!);
        return [[iFo, 1 - e], [iHa, e]];
      },
      color: g.cuff,
      out: (i, _j, p) => p.clone().sub(el.clone().addScaledVector(H.da, cts[i]!)),
    });
  }
  for (const f of FINGERS) buildDigit(soup, H, f, spec.paint, palmP, thick, g ? undefined : spec.nails, detail);
}

function buildDigit(soup: Soup, H: HandDef, f: Finger, back: Paint, palmP: Paint, thick: number, nails: Paint | undefined, detail: Detail): void {
  const g = H.digits[f];
  const side = H.side;
  const [l1, l2, l3] = g.len;
  const r0 = g.rad + thick;
  const thumb = f === "thumb";
  const total = l1 + l2 + l3 + thick;
  const start = thumb ? -0.028 * H.sh : -0.013 * H.sh;
  // Radius along the digit: knuckles a little proud, the pulp of the tip full, the tip domed.
  const prof: [number, number][] = thumb
    ? [[start, 0.6], [0, 1.45], [0.4 * l1, 1.32], [l1, 1.0], [l1 + 0.5 * l2, 0.95], [l1 + l2, 0.92], [l1 + l2 + 0.5 * l3, 0.92], [total - 0.45 * r0, 0.72], [total - 0.12 * r0, 0.35], [total, 0.02]]
    : [[start, 1.02], [0.12 * l1, 1.02], [0.6 * l1, 0.93], [l1, 0.97], [l1 + 0.5 * l2, 0.86], [l1 + l2, 0.86], [l1 + l2 + 0.55 * l3, 0.84], [total - 0.5 * r0, 0.7], [total - 0.15 * r0, 0.36], [total, 0.02]];
  const fr = new Mono(prof.map((p) => p[0]), prof.map((p) => p[1]));
  const n = detail === 2 ? 13 : detail === 1 ? 6 : 3;
  const ss: number[] = [];
  for (let i = 0; i <= n; i++) ss.push(lerp(start, total, 1 - (1 - i / n) ** 1.25));
  const nA = detail === 2 ? 8 : detail === 1 ? 6 : 3;
  const across = g.k;
  const palmward = V().crossVectors(g.k, g.dir).normalize();
  if (palmward.dot(g.back) > 0) palmward.negate();
  const rings: THREE.Vector3[][] = [];
  for (const sv of ss) {
    const c = g.base.clone().addScaledVector(g.dir, sv);
    const r = fr.at(sv) * r0;
    const pulp = smooth(l1 + l2, l1 + l2 + 0.4 * l3, sv) * (1 - smooth(total - 0.6 * r0, total, sv));
    c.addScaledVector(palmward, 0.12 * r0 * pulp);
    const ring: THREE.Vector3[] = [];
    for (let j = 0; j < nA; j++) {
      const a = (j / nA) * Math.PI * 2;
      ring.push(c.clone().addScaledVector(across, Math.cos(a) * r * 1.07).addScaledVector(palmward, Math.sin(a) * r * 0.92));
    }
    rings.push(ring);
  }
  const b1 = BI[`${f}_01_${side}`]!;
  const b2 = BI[`${f}_02_${side}`]!;
  const b3 = BI[`${f}_03_${side}`]!;
  const iHa = BI[`hand_${side}`]!;
  const wAt = (sv: number): Wts => {
    const h = thumb ? smooth(-0.012 * H.sh, 0.012 * H.sh, sv) : smooth(-0.004 * H.sh, 0.006 * H.sh, sv);
    const e2 = smooth(l1 - 0.0038, l1 + 0.0038, sv);
    const e3 = smooth(l1 + l2 - 0.003, l1 + l2 + 0.003, sv);
    return [[iHa, 1 - h], [b1, h * (1 - e2)], [b2, h * e2 * (1 - e3)], [b3, h * e3]];
  };
  const pal = (j: number) => Math.sin((j / nA) * Math.PI * 2) > 0.2;
  loft(soup, rings, {
    w: (i) => wAt(ss[i]!),
    color: (_i, j) => (pal(j) ? palmP : back),
    out: (i, _j, p) => p.clone().sub(g.base.clone().addScaledVector(g.dir, ss[i]!)),
  });
  if (nails && detail === 2) {
    // The nail: a thin curved plate on the back of the distal phalanx.
    const s0 = l1 + l2 + 0.38 * l3;
    const s1 = total - 0.2 * r0;
    const rows: THREE.Vector3[][] = [];
    for (let k = 0; k <= 4; k++) {
      const sv = lerp(s0, s1, k / 4);
      const c = g.base.clone().addScaledVector(g.dir, sv);
      const pulp = smooth(l1 + l2, l1 + l2 + 0.4 * l3, sv) * (1 - smooth(total - 0.6 * r0, total, sv));
      c.addScaledVector(palmward, 0.12 * r0 * pulp);
      const r = fr.at(sv) * r0 + 0.0004;
      const row: THREE.Vector3[] = [];
      const half = 0.78 * (k === 0 ? 0.85 : 1);
      for (let j = 0; j <= 4; j++) {
        const a = -Math.PI / 2 + lerp(-half, half, j / 4);
        row.push(c.clone().addScaledVector(across, Math.cos(a) * r * 1.07).addScaledVector(palmward, Math.sin(a) * r * 0.92));
      }
      rows.push(row);
    }
    loft(soup, rows, { closed: false, w: () => [[b3, 1]], color: nails, out: () => palmward.clone().negate() });
  }
}


/* ------------------------------------------------------------------------------------------------------------ */
/* Neck, head (sculpted, faceless), ears and hair                                                                 */
/* ------------------------------------------------------------------------------------------------------------ */

export interface HeadShape {
  c: THREE.Vector3;
  /** Distance from the centre to the surface along unit direction d. */
  r(d: THREE.Vector3): number;
  /** A cap's (or a hard hat's band's) line round the head: the plane d . n = k (d unit), higher at the brow. */
  capN: THREE.Vector3;
  capK: number;
}

/** The skull as a radius function round its centre (faceless: no eyes, nose or mouth). */
export function headShape(B: Body): HeadShape {
  const s = B.s;
  const fem = B.fem;
  const c = V(0, B.topY - 0.098 * s, 0.008 * s);
  const ax = (0.077 - 0.004 * fem) * s;
  const ayU = 0.098 * s;
  const ayD = (0.134 - 0.007 * fem) * s;
  const azF = 0.096 * s;
  const azB = (0.103 - 0.003 * fem) * s;
  const bump = (d: THREE.Vector3, x: number, y: number, z: number, wx: number, wy: number, a: number) => {
    if (d.z * z < -0.2 && z !== 0) return 0;
    return a * Math.exp(-(((d.x - x) / wx) ** 2) - ((d.y - y) / wy) ** 2);
  };
  const r = (d: THREE.Vector3) => {
    const low = smooth(0.05, -0.85, d.y);
    const fwd = smooth(-0.2, 0.6, d.z);
    const axE = ax * (1 - 0.15 * low * (0.5 + 0.5 * fwd));
    const azE = d.z >= 0 ? azF * (1 - 0.05 * low) : azB * (1 - 0.42 * smooth(-0.2, -0.85, d.y) * smooth(0.05, -0.55, d.z));
    const ay = d.y >= 0 ? ayU : ayD * (1 - 0.2 * smooth(0.1, -0.7, d.z));
    // Below the cheekbones the front view squares off (jaw), then rounds at the chin.
    const pe = 2 + 0.95 * low * (0.4 + 0.6 * fwd);
    const xy = Math.pow(Math.pow(Math.abs(d.x / axE), pe) + Math.pow(Math.abs(d.y / ay), pe), 2 / pe);
    let rr = 1 / Math.sqrt(xy + (d.z / azE) ** 2);
    const sc = s;
    rr += bump(d, 0, 0.24, 1, 0.55, 0.11, 0.0028 * sc);
    rr -= bump(d, 0.33, 0.07, 1, 0.17, 0.11, 0.0024 * sc) + bump(d, -0.33, 0.07, 1, 0.17, 0.11, 0.0024 * sc);
    rr += bump(d, 0.6, -0.14, 1, 0.2, 0.15, 0.0042 * sc) + bump(d, -0.6, -0.14, 1, 0.2, 0.15, 0.0042 * sc);
    rr += bump(d, 0, -0.7, 1, 0.28, 0.16, 0.0055 * sc * (1 - 0.4 * fem));
    rr += bump(d, 0.62, -0.6, 0, 0.18, 0.18, 0.0036 * sc * (1 - 0.5 * fem)) + bump(d, -0.62, -0.6, 0, 0.18, 0.18, 0.0036 * sc * (1 - 0.5 * fem));
    rr += d.z < -0.5 ? 0.0045 * sc * Math.exp(-(d.x ** 2) / 0.2 - ((d.y - 0.05) ** 2) / 0.12) : 0;
    rr -= bump(d, 0.95, 0.25, 0, 0.2, 0.2, 0.0028 * sc) + bump(d, -0.95, 0.25, 0, 0.2, 0.2, 0.0028 * sc);
    return rr;
  };
  // The cap sits higher at the forehead than at the nape: a plane through mid-forehead and the occiput.
  const capN = V(0, 1, -0.24).normalize();
  const capK = 0.3 * capN.y + 0.954 * capN.z;
  return { c, r, capN, capK };
}

export function dirOf(lat: number, lon: number, out = V()): THREE.Vector3 {
  // lon 0 faces +z (front), increasing towards +x (the left); lat from -pi/2 (down) to pi/2 (up).
  return out.set(Math.cos(lat) * Math.sin(lon), Math.sin(lat), Math.cos(lat) * Math.cos(lon));
}

/** The neck, from inside the collar up into the head. */
export function buildNeck(soup: Soup, B: Body, c: Tint, detail: Detail): void {
  const s = B.s;
  const J = B.J;
  const r = (0.052 + 0.007 * (1 - B.fem)) * s;
  const bot = V(0, B.neckBaseY - 0.035 * s, -0.026 * s);
  const top = V(0, J.head!.y + 0.04 * s, J.head!.z + 0.022 * s);
  const n = detail === 2 ? 6 : 5;
  const nA = detail === 2 ? 14 : detail === 1 ? 12 : 7;
  const rings: THREE.Vector3[][] = [];
  for (let i = 0; i <= n; i++) {
    const u = i / n;
    const cc = bot.clone().lerp(top, u);
    const k = 1 + 0.12 * smooth(0.6, 1, u) + 0.06 * smooth(0.25, 0, u);
    const ring: THREE.Vector3[] = [];
    for (let j = 0; j < nA; j++) {
      const a = (j / nA) * Math.PI * 2;
      const cz = Math.cos(a);
      ring.push(cc.clone().add(V(Math.sin(a) * r * 1.06 * k, 0, cz * r * (cz > 0 ? 0.9 : 1.02) * k)));
    }
    rings.push(ring);
  }
  loft(soup, rings, {
    w: (_i, _j, p) => {
      const a = smooth(B.neckBaseY - 0.01 * s, B.neckBaseY + 0.02 * s, p.y);
      const b = smooth(J.head!.y - 0.015 * s, J.head!.y + 0.03 * s, p.y);
      return [[BI.spine_03!, 1 - a], [BI.neck!, a * (1 - b)], [BI.head!, a * b]];
    },
    color: c,
  });
}

export interface HeadSpec {
  /** Skin shade (1 with a per-tone skin material) and the shade inside the ears. */
  skin: Tint;
  skinDark: Tint;
  hair: Tint;
  /** Short hair, or longer hair gathered in a bun at the nape. */
  bun?: boolean;
  /** Hair over the crown too (false under a hat: only the hair below its band is drawn). */
  crown?: boolean;
}

/** The sculpted, faceless head with its ears, and hair from the nape and temples up. */
export function buildHead(skin: Soup, hair: Soup, B: Body, spec: HeadSpec, detail: Detail): void {
  const s = B.s;
  const Hs = headShape(B);
  const head = rigid("head");
  const nLat = detail === 2 ? 22 : detail === 1 ? 14 : 8;
  const nLon = detail === 2 ? 30 : detail === 1 ? 18 : 8;
  const d = V();
  const surf = (lat: number, lon: number, off = 0) => {
    dirOf(lat, lon, d);
    return Hs.c.clone().addScaledVector(d, Hs.r(d) + off);
  };
  const rings: THREE.Vector3[][] = [];
  for (let i = 0; i <= nLat; i++) {
    const lat = -Math.PI / 2 + (Math.PI * i) / nLat;
    const ring: THREE.Vector3[] = [];
    for (let j = 0; j < nLon; j++) ring.push(surf(lat, (j / nLon) * Math.PI * 2));
    rings.push(ring);
  }
  loft(skin, rings, { w: () => head, color: spec.skin, out: (_i, _j, p) => p.clone().sub(Hs.c) });

  // Ears: a rolled helix round a hollowed shell, set out from the side of the head and tilted back.
  for (const sd of [1, -1]) {
    const ec = Hs.c.clone().add(V(sd * (0.074 - 0.004 * B.fem) * s, -0.03 * s, -0.018 * s));
    const out = V(sd, 0, -0.32).normalize();
    const upA = V(0, 1, 0.22).normalize();
    const back = V().crossVectors(upA, out).multiplyScalar(sd);
    const eh = 0.031 * s;
    const ew = 0.018 * s;
    const nK = detail === 2 ? 5 : detail === 1 ? 3 : 1;
    const nE = detail === 2 ? 14 : detail === 1 ? 10 : 6;
    const rimH = [0.0016, 0.0058, 0.0044, 0.0034, 0.0052, 0.0024, 0.0006, 0.0];
    const front: THREE.Vector3[][] = [];
    const backS: THREE.Vector3[][] = [];
    for (let k = 0; k <= nK; k++) {
      const u = k / nK;
      const scale = 1 - u;
      const hh = rimH[Math.round(u * (rimH.length - 1))]! * s;
      const fr: THREE.Vector3[] = [];
      const br: THREE.Vector3[] = [];
      for (let j = 0; j < nE; j++) {
        const a = (j / nE) * Math.PI * 2;
        // Egg outline: broad at the top, a soft lobe below; the front edge straighter where it meets the head.
        const yy = Math.sin(a) * eh * (Math.sin(a) > 0 ? 1 : 0.92);
        const xx = Math.cos(a) * ew * (Math.sin(a) > 0 ? 1.08 : 0.8) * (Math.cos(a) > 0 ? 1 : 0.72);
        const base = ec.clone().addScaledVector(upA, yy * scale).addScaledVector(back, xx * scale);
        const lobe = Math.sin(a) < -0.6 ? 0.0012 * s : 0;
        fr.push(base.clone().addScaledVector(out, hh + 0.002 * s + lobe));
        br.push(base.clone().addScaledVector(out, -0.004 * s * u + 0.0005 * s));
      }
      front.push(fr);
      backS.push(br);
    }
    loft(skin, front, { w: () => head, color: spec.skin, out: () => out });
    loft(skin, [front[0]!, ...backS], { w: () => head, color: spec.skinDark, out: () => out.clone().negate() });
  }

  // The band line round the head (a cap's, a hard hat's), as a latitude for each longitude.
  const capLat = (lon: number) => {
    let lo = -Math.PI / 2;
    let hi = Math.PI / 2;
    for (let k = 0; k < 24; k++) {
      const m = (lo + hi) / 2;
      dirOf(m, lon, d);
      if (d.dot(Hs.capN) > Hs.capK) hi = m;
      else lo = m;
    }
    return (lo + hi) / 2;
  };
  // Hair: from the hairline (at the temples, round above the ears, down to the nape) up past the band line.
  const hairLow = (lon: number) => {
    const psi = Math.abs(wrapPi(lon));
    const k: [number, number][] = [[0.85, 0.02], [1.12, -0.36], [1.32, -0.3], [1.48, 0.06], [1.86, 0.06], [2.1, -0.44], [2.6, -0.6], [Math.PI, -0.64]];
    if (psi <= k[0]![0]) return Math.asin(k[0]![1]);
    for (let i = 1; i < k.length; i++) if (psi <= k[i]![0]) return Math.asin(lerp(k[i - 1]![1], k[i]![1], smooth(k[i - 1]![0], k[i]![0], psi)));
    return Math.asin(-0.64);
  };
  const nHu = detail === 2 ? 36 : detail === 1 ? 24 : 12;
  const nHv = detail === 2 ? 6 : detail === 1 ? 4 : 2;
  const lon0 = 0.8;
  const hRows: THREE.Vector3[][] = [];
  for (let i = 0; i <= nHv; i++) {
    const row: THREE.Vector3[] = [];
    for (let j = 0; j <= nHu; j++) {
      const lon = lon0 + ((Math.PI * 2 - 2 * lon0) * j) / nHu;
      const a = hairLow(lon);
      const b = capLat(lon) + 0.12;
      const lat = lerp(a, Math.max(a, b), i / nHv);
      const th = (0.0016 + 0.0042 * smooth(0, 0.35, i / nHv)) * s * (spec.bun ? 0.8 : 1);
      row.push(surf(lat, lon, th));
    }
    hRows.push(row);
  }
  loft(hair, hRows, { closed: false, w: () => head, color: spec.hair, out: (_i, _j, p) => p.clone().sub(Hs.c), uv: (_i, _j, _p, u, v) => [u, -v] });
  // The crown: hair from the band line over the top.
  if (spec.crown !== false) {
    const nV = detail === 2 ? 8 : detail === 1 ? 5 : 3;
    const nU = detail === 2 ? 32 : detail === 1 ? 20 : 10;
    const rows: THREE.Vector3[][] = [];
    for (let i = 0; i <= nV; i++) {
      const row: THREE.Vector3[] = [];
      for (let j = 0; j < nU; j++) {
        const lon = (j / nU) * Math.PI * 2;
        const lat = lerp(capLat(lon) - 0.03, Math.PI / 2, (i / nV) ** 1.1);
        row.push(surf(lat, lon, (0.0045 + 0.0025 * Math.min(1, (3 * i) / nV)) * s));
      }
      rows.push(row);
    }
    loft(hair, rows, { w: () => head, color: spec.hair, out: (_i, _j, p) => p.clone().sub(Hs.c), uv: (_i, _j, _p, u, v) => [u, -v] });
  }
  // A bun at the nape for longer hair.
  if (spec.bun && detail > 0) {
    const bc = Hs.c.clone().add(V(0, -0.045 * s, -0.1 * s));
    addGeo(hair, new THREE.SphereGeometry(0.036 * s, detail === 2 ? 18 : 9, detail === 2 ? 12 : 6), mat4(bc, new THREE.Quaternion().setFromEuler(new THREE.Euler(0.5, 0, 0)), V(1.15, 0.85, 0.9)), head, spec.hair, 6);
  }
}


/* ------------------------------------------------------------------------------------------------------------ */
/* Poses                                                                                                         */
/* ------------------------------------------------------------------------------------------------------------ */

/**
 * How a hand holds: a tool's handle in the oblique grip with the working end past the thumb (a hammer, a radio, a
 * pen), the same grip with the working end below the little finger (`dagger`), a bar or a rim (power grip, hook),
 * or flat on a surface.
 */
export type GripKind = "tool" | "dagger" | "bar" | "hook" | "palm";
/**
 * Finger shapes. `close` blends from relaxed to the shape. `grasp` closes every digit, joint by joint, until it
 * touches the held item's surface (`Staff.itemSdf`, in the item socket's frame): fingers flat on a wall, a thumb
 * over a lip. `point` straightens the forefinger over a fist; `fist` closes them all.
 */
export type HandShape = "relax" | "tool" | "bar" | "hook" | "claw" | "flat" | "grasp" | "point" | "fist";

export interface FootPose {
  /** Ankle joint, body space. */
  pos: THREE.Vector3;
  /** Foot orientation, body space (identity: flat, pointing +z). */
  quat: THREE.Quaternion;
  /** Toe (MTP) extension relative to the foot, radians. */
  toe: number;
}
export interface ArmPose {
  /** Free arm (used where `ik` < 1): shoulder flexion, abduction, elbow flexion, humeral twist, radians. */
  flex: number;
  abd: number;
  elbow: number;
  twist: number;
  /** Weight of the IK target, 0..1. */
  ik: number;
  grip: GripKind;
  /** The grip point to reach, body space. */
  pos: THREE.Vector3;
  /** Target frame: tool: handle axis (+z of the grip frame) and its up (+y); bar/hook: the bar's axis towards the
   *  thumb side and the palm's facing; palm: the fingers' direction and the palm's facing. */
  a: THREE.Vector3;
  b: THREE.Vector3;
  /** Where the elbow should point, body space. */
  pole: THREE.Vector3;
  shape: HandShape;
  close: number;
  /** Handle radius for the tool and bar shapes, metres. */
  radius: number;
  /** How far the hand may roll round the handle axis to stay comfortable (round handles), radians. */
  roll: number;
  /**
   * A hook on a lip may sit anywhere from fingers-forward under it to fingers-down over its edge: the target
   * (frame and point) may turn by 0..`spinMax` about `spinAxis` through `spinPivot`, shifting by `spinShift` times
   * the sine of the turn, whichever suits the arm. `spinMax` 0 for none.
   */
  spinMax: number;
  spinAxis: THREE.Vector3;
  spinPivot: THREE.Vector3;
  spinShift: THREE.Vector3;
}
export interface BodyPose {
  /** Pelvis (between the hip joints), body space. */
  pelvis: THREE.Vector3;
  /** Pelvis forward tilt, roll (left hip up positive), yaw, radians. */
  tilt: number;
  roll: number;
  yaw: number;
  /** Trunk on the pelvis: flexion (forward positive), side bend (to the right positive), twist (to the left positive). */
  flex: number;
  side: number;
  twist: number;
  /** Clavicles: elevation and protraction, left then right. */
  clav: [number, number, number, number];
  feet: [FootPose, FootPose];
  arms: [ArmPose, ArmPose];
  /** Where the eyes would look, body space. */
  gaze: THREE.Vector3;
  /** The two-handed item's frame, body space (when `itemOn`). */
  itemOn: boolean;
  itemPos: THREE.Vector3;
  itemQuat: THREE.Quaternion;
}

function armPose(): ArmPose {
  return { flex: 0, abd: 0, elbow: 0.2, twist: 0, ik: 0, grip: "tool", pos: V(), a: Z.clone(), b: Y.clone(), pole: V(0, -1, -0.5).normalize(), shape: "relax", close: 0, radius: 0.013, roll: 0, spinMax: 0, spinAxis: X.clone(), spinPivot: V(), spinShift: V() };
}
function footPose(): FootPose {
  return { pos: V(), quat: new THREE.Quaternion(), toe: 0 };
}
export function newPose(): BodyPose {
  return {
    pelvis: V(),
    tilt: 0,
    roll: 0,
    yaw: 0,
    flex: 0,
    side: 0,
    twist: 0,
    clav: [0, 0, 0, 0],
    feet: [footPose(), footPose()],
    arms: [armPose(), armPose()],
    gaze: V(0, 1.6, 3),
    itemOn: false,
    itemPos: V(),
    itemQuat: new THREE.Quaternion(),
  };
}
function copyArm(o: ArmPose, a: ArmPose): void {
  o.flex = a.flex;
  o.abd = a.abd;
  o.elbow = a.elbow;
  o.twist = a.twist;
  o.ik = a.ik;
  o.grip = a.grip;
  o.pos.copy(a.pos);
  o.a.copy(a.a);
  o.b.copy(a.b);
  o.pole.copy(a.pole);
  o.shape = a.shape;
  o.close = a.close;
  o.radius = a.radius;
  o.roll = a.roll;
  o.spinMax = a.spinMax;
  o.spinAxis.copy(a.spinAxis);
  o.spinPivot.copy(a.spinPivot);
  o.spinShift.copy(a.spinShift);
}
export function copyPose(o: BodyPose, p: BodyPose): BodyPose {
  o.pelvis.copy(p.pelvis);
  o.tilt = p.tilt;
  o.roll = p.roll;
  o.yaw = p.yaw;
  o.flex = p.flex;
  o.side = p.side;
  o.twist = p.twist;
  o.clav = [...p.clav];
  for (let i = 0; i < 2; i++) {
    o.feet[i]!.pos.copy(p.feet[i]!.pos);
    o.feet[i]!.quat.copy(p.feet[i]!.quat);
    o.feet[i]!.toe = p.feet[i]!.toe;
    copyArm(o.arms[i]!, p.arms[i]!);
  }
  o.gaze.copy(p.gaze);
  o.itemOn = p.itemOn;
  o.itemPos.copy(p.itemPos);
  o.itemQuat.copy(p.itemQuat);
  return o;
}
/**
 * Blend two poses into `o` (which may be `a`). Feet are expected to agree (planted); arms blend their free swing
 * and their targets, and where only one side reaches, the reach weight ramps instead.
 */
export function blendPose(o: BodyPose, a: BodyPose, b: BodyPose, w: number): BodyPose {
  const k = clamp(w, 0, 1);
  o.pelvis.lerpVectors(a.pelvis, b.pelvis, k);
  o.tilt = lerp(a.tilt, b.tilt, k);
  o.roll = lerp(a.roll, b.roll, k);
  o.yaw = lerp(a.yaw, b.yaw, k);
  o.flex = lerp(a.flex, b.flex, k);
  o.side = lerp(a.side, b.side, k);
  o.twist = lerp(a.twist, b.twist, k);
  o.clav = [0, 1, 2, 3].map((i) => lerp(a.clav[i]!, b.clav[i]!, k)) as [number, number, number, number];
  for (let i = 0; i < 2; i++) {
    const fa = a.feet[i]!;
    const fb = b.feet[i]!;
    const fo = o.feet[i]!;
    fo.pos.lerpVectors(fa.pos, fb.pos, k);
    fo.quat.slerpQuaternions(fa.quat, fb.quat, k);
    fo.toe = lerp(fa.toe, fb.toe, k);
    const aa = a.arms[i]!;
    const ab = b.arms[i]!;
    const ao = o.arms[i]!;
    const ikA = aa.ik;
    const ikB = ab.ik;
    ao.flex = lerp(aa.flex, ab.flex, k);
    ao.abd = lerp(aa.abd, ab.abd, k);
    ao.elbow = lerp(aa.elbow, ab.elbow, k);
    ao.twist = lerp(aa.twist, ab.twist, k);
    if (ikA > 0 && ikB > 0) {
      ao.pos.lerpVectors(aa.pos, ab.pos, k);
      ao.a.lerpVectors(aa.a, ab.a, k).normalize();
      ao.b.lerpVectors(aa.b, ab.b, k).normalize();
      ao.pole.lerpVectors(aa.pole, ab.pole, k).normalize();
      ao.grip = k < 0.5 ? aa.grip : ab.grip;
    } else {
      const src = ikB > 0 ? ab : aa;
      ao.pos.copy(src.pos);
      ao.a.copy(src.a);
      ao.b.copy(src.b);
      ao.pole.copy(src.pole);
      ao.grip = src.grip;
    }
    ao.ik = lerp(ikA, ikB, k);
    const sameShape = aa.shape === ab.shape;
    if (sameShape) {
      ao.shape = aa.shape;
      ao.close = lerp(aa.close, ab.close, k);
    } else {
      // Open the first shape, then close the second.
      ao.shape = k < 0.5 ? aa.shape : ab.shape;
      ao.close = k < 0.5 ? aa.close * (1 - 2 * k) : ab.close * (2 * k - 1);
    }
    ao.radius = lerp(aa.radius, ab.radius, k);
    ao.roll = Math.max(aa.roll, ab.roll);
    const sa = aa.spinMax >= ab.spinMax ? aa : ab;
    const both = aa.spinMax > 0 && ab.spinMax > 0;
    const piv = both ? V().lerpVectors(aa.spinPivot, ab.spinPivot, k) : sa.spinPivot.clone();
    const ax = sa.spinAxis.clone();
    const sh = both ? V().lerpVectors(aa.spinShift, ab.spinShift, k) : sa.spinShift.clone();
    ao.spinMax = sa.spinMax;
    ao.spinAxis.copy(ax);
    ao.spinPivot.copy(piv);
    ao.spinShift.copy(sh);
  }
  o.gaze.lerpVectors(a.gaze, b.gaze, k);
  const aOn = a.itemOn;
  const bOn = b.itemOn;
  if (aOn && bOn) {
    o.itemPos.lerpVectors(a.itemPos, b.itemPos, k);
    o.itemQuat.slerpQuaternions(a.itemQuat, b.itemQuat, k);
  } else if (bOn || aOn) {
    o.itemPos.copy(bOn ? b.itemPos : a.itemPos);
    o.itemQuat.copy(bOn ? b.itemQuat : a.itemQuat);
  }
  o.itemOn = k < 0.5 ? aOn : bOn;
  return o;
}

/* ------------------------------------------------------------------------------------------------------------ */
/* A person                                                                                                       */
/* ------------------------------------------------------------------------------------------------------------ */

/** A capsule: the segment a-b swept by radius r (world space when a Staff hands it out). */
export interface Capsule {
  name: string;
  a: THREE.Vector3;
  b: THREE.Vector3;
  r: number;
}
/**
 * Asks the scene whether a capsule of the body is inside something: return the horizontal push (world, metres) that
 * would take it out, or null when it is clear. Set on a Staff as `obstacles`.
 */
export type ObstacleQuery = (c: Readonly<Capsule>) => THREE.Vector3 | null;

/** One skinned part of a person: geometry in body space at the bind pose (bones as `ALL_BONES`), and its material. */
export interface StaffPart {
  /** The mesh's name suffix ("cloth", "skin"). */
  key: string;
  geometry: THREE.BufferGeometry;
  material: THREE.Material;
  /** The geometry is shared with other people (a cache's): disposing this person leaves it be. */
  shared?: boolean;
  /** Cast shadows (default: the person's `shadows`). */
  castShadow?: boolean;
}
/** Dresses a body: returns its skinned parts, built here or taken from a cache (the body depends on cohort and sole). */
export type Dresser = (B: Body, T: Torso, detail: Detail) => StaffPart[];

/** A work pose: change `p`, which comes in standing at ease at time t (seconds into the work), at the placement. */
export type WorkFn = (st: Staff, t: number, o: WorkOpts, p: BodyPose) => void;
export interface WorkDef {
  pose: WorkFn;
  /** The upper body can keep this up while walking (the hands hold something): a walk may carry it on. */
  walk?: boolean;
  /** The pose places the feet itself (a foot up on a step), instead of standing them where the routine planted
   *  them. Only while the person stands still at the work. */
  feet?: boolean;
}

/**
 * What the item socket holds for carry, pick and place: a box-like item with its frame at the centre of its top,
 * long side across the body (x). The hands lie flat on the end walls just under the rim, thumbs over it.
 */
export interface CarrySpec {
  /** Outer length (across the body), width (front to back) and depth, metres. */
  length: number;
  width: number;
  depth: number;
  /** Inset of the walls from the rim's outer edge at the top, and their lean inward going down (radians). */
  flange: number;
  taper: number;
  /** Thickness of the rim's lip. */
  lip: number;
  /** Half the distance between the grips (where the palms lie on the end walls). */
  gripHalf: number;
}

export interface StaffOptions {
  seed?: number;
  /** Cast shadows (default true). */
  shadows?: boolean;
  /** The footwear's sole, metres: the body stands this much above the floor (default: the row's shoe sole). */
  sole?: number;
  /** What the person wears (default `plainDress`: a work shirt, trousers, boots, bare hands). */
  dress?: Dresser;
  /** Work poses by name, besides the core's ("stand", "carry", "pick", "place"). */
  works?: Readonly<Record<string, WorkDef>>;
  /** The item the socket holds for carry, pick and place (default a full-size hotel pan, GN 1/1). */
  carry?: CarrySpec;
}

export const SKIN_TONES = ["#f2cdb0", "#e8b896", "#d29a72", "#b97d56", "#9a6141", "#7a4a30", "#5c3622", "#f0c6a0"] as const;
export const HAIR_COLOURS = ["#1a1411", "#2b1c13", "#3f2a1c", "#5d4029", "#8a6440", "#b89766", "#7b7874", "#2e2520"] as const;

/** A plain set of workwear from a seed: a work shirt, trousers, lace-up boots, bare hands, short hair. */
export function plainDress(seed = 1): Dresser {
  return (B, T, detail) => {
    const r = rng(seed * 7919 + Math.round(B.s * 1000));
    const pick = <X>(a: readonly X[]) => a[Math.floor(r() * a.length) % a.length]!;
    const M = peopleMaterials();
    const shirt = paint(pick(["#3b4a5e", "#4f5b47", "#6c7a89", "#2c3440"]));
    const trousers = paint(pick(["#23262c", "#2f3a4d", "#4a4134"]));
    const cloth = new Soup();
    buildTop(cloth, B, T, { body: () => shirt, sleeve: () => shirt, placket: paint(shirt.c.clone().multiplyScalar(0.85)), pockets: shirt }, detail);
    buildTrousers(cloth, B, T, { seat: () => trousers, leg: () => trousers, hem: B.lift + 0.06 * B.s, ease: 1.04 }, detail);
    const gear = new Soup();
    buildBoots(gear, B, { kind: "lace", upper: paint("#3a2a1e"), sole: paint("#141414"), midsole: paint("#2a2420"), laces: paint("#1c1712") }, detail);
    const skin = new Soup();
    const tone = pick(SKIN_TONES);
    for (const side of SIDES) buildHand(skin, B, side, { paint: paint(0xffffff), nails: paint("#f4dcd4") }, detail);
    buildNeck(skin, B, new THREE.Color(1, 1, 1), detail);
    const hair = new Soup();
    buildHead(skin, hair, B, { skin: new THREE.Color(1, 1, 1), skinDark: new THREE.Color(0.8, 0.8, 0.8), hair: new THREE.Color(1, 1, 1), bun: r() < B.fem * 0.6 }, detail);
    return [
      { key: "cloth", geometry: cloth.geometry(), material: M.cloth },
      { key: "gear", geometry: gear.geometry(), material: M.gear },
      { key: "skin", geometry: skin.geometry(), material: M.skin(tone) },
      { key: "hair", geometry: hair.geometry(), material: M.hair(pick(HAIR_COLOURS)) },
    ];
  };
}

const RELAX: Record<Finger, [number, number, number]> = { thumb: [0.12, 0.14, 0.12], index: [0.22, 0.32, 0.16], middle: [0.28, 0.4, 0.2], ring: [0.33, 0.45, 0.22], pinky: [0.4, 0.5, 0.25] };
const CLAW: Record<Finger, [number, number, number]> = { thumb: [0.55, 0.5, 0.4], index: [0.42, 1.5, 0.8], middle: [0.36, 1.55, 0.85], ring: [0.4, 1.5, 0.8], pinky: [0.5, 1.45, 0.75] };
const FLAT: Record<Finger, [number, number, number]> = { thumb: [-0.05, 0.05, 0.05], index: [0.04, 0.08, 0.04], middle: [0.05, 0.1, 0.05], ring: [0.06, 0.1, 0.05], pinky: [0.08, 0.12, 0.06] };
const FIST: Record<Finger, [number, number, number]> = { thumb: [0.6, 0.55, 0.45], index: [1.35, 1.55, 0.9], middle: [1.4, 1.6, 0.95], ring: [1.42, 1.6, 0.95], pinky: [1.45, 1.55, 0.9] };
const POINT: Record<Finger, [number, number, number]> = { ...FIST, thumb: [0.55, 0.45, 0.35], index: [0.05, 0.06, 0.04] };

export interface WorkOpts {
  /** World point the work centres on (a target to point at, a core tray to inspect, a rail to hold, an item's rim
   *  centre to pick from or place on). Defaults are set from the body and `surfaceY`. */
  at?: THREE.Vector3;
  /** World height of the work surface (a table, a step, a shelf). */
  surfaceY?: number;
  /** Half the distance between a carried item's grip points along its x axis (default the carry spec's). */
  halfWidth?: number;
  /** Tempo multiplier. */
  tempo?: number;
}

const _v1 = V();
const _v2 = V();
const _v3 = V();
const _v4 = V();
const _tv1 = V();
const _tv2 = V();
const _tv3 = V();
const _tq = new THREE.Quaternion();
const _sw1 = V();
const _sw2 = V();
const _sw3 = V();
const _q1 = new THREE.Quaternion();
const _q2 = new THREE.Quaternion();
const _q3 = new THREE.Quaternion();

export class Staff {
  readonly group = new THREE.Group();
  readonly bones = new Map<string, THREE.Bone>();
  readonly dims: Anthro;
  readonly cohort: Cohort;
  readonly detail: Detail;
  readonly skeleton: THREE.Skeleton;
  readonly meshes: THREE.SkinnedMesh[] = [];
  /** Socket for a two-handed item (a box, a tray, a tablet): parent the item to it; the work poses move it. */
  readonly item = new THREE.Object3D();
  /** The item carry, pick and place handle (`StaffOptions.carry`). */
  carrySpec: CarrySpec;
  /**
   * The held item's solid as a signed distance in the socket's frame (negative inside), which `grasp` hands close
   * on. The carry spec's box by default; set it when the socket holds something else, or null for none.
   */
  itemSdf: ((p: THREE.Vector3) => number) | null;
  tris: number;
  /** Mass of what the item socket holds (kg). While it is carried the hips move back and the trunk leans back so
   *  that the body and the load together stay balanced over the feet. */
  itemKg = 4;
  /** The scene's obstacles, if it has any to report (see `capsules` and `collide`). */
  obstacles: ObstacleQuery | null = null;
  /** Work poses by name besides the core's. */
  readonly works: Readonly<Record<string, WorkDef>>;
  /** Triangles by part. */
  readonly trisBy: Record<string, number> = {};
  /** The last pose applied. */
  readonly current = newPose();
  /** Largest range-of-motion excursions in the last pose, degrees beyond the table's limits (0 when inside). */
  readonly rom: Record<string, number> = {};
  /** The measured angle behind each `rom` entry (the side nearer its limit), degrees. */
  readonly romAngle: Record<string, number> = {};
  /** The body's layout (joints, proportions). */
  readonly B: Body;
  /** The torso's outer surface model. */
  readonly torso: Torso;
  /** A per-person number in [0, 1) from the seed: phases idle motion so a crew never moves in step. */
  readonly phase: number;
  private bonesArr: THREE.Bone[] = [];
  private Pw: THREE.Vector3[] = [];
  private Qw: THREE.Quaternion[] = [];
  private wrapCache = new Map<string, Record<Finger, [number, number, number]>>();
  private placed = { x: 0, z: 0, yaw: 0 };
  private scratch = newPose();
  /** @internal Last arm solve: iteration, forearm twist, wrist flexion, deviation, residual twist (degrees). */
  armDebug: Record<string, number[]> = {};
  /** The grip point each arm's last solve aimed at (its target, turned by any spin), body space. */
  private reachTarget: Record<Side, THREE.Vector3> = { L: V(), R: V() };
  private own: THREE.BufferGeometry[] = [];

  constructor(cohort: Cohort, detail: Detail, opts: StaffOptions = {}) {
    this.cohort = cohort;
    this.detail = detail;
    const B = (this.B = layout(cohort, opts.sole));
    this.dims = B.d;
    const T = (this.torso = torsoModel(B));
    this.works = opts.works ?? {};
    this.carrySpec = opts.carry ?? HOTEL_PAN;
    this.itemSdf = boxSdf(this.carrySpec);
    this.phase = rng((opts.seed ?? 1) * 2654435 + 17)();
    // Bones, all at identity bind rotations.
    for (const name of ALL_BONES) {
      const b = new THREE.Bone();
      b.name = name;
      this.bones.set(name, b);
      this.bonesArr.push(b);
      this.Pw.push(V());
      this.Qw.push(new THREE.Quaternion());
    }
    for (const name of ALL_BONES) {
      const b = this.bones.get(name)!;
      const p = parentOf(name);
      const at = B.J[name]!;
      if (p) {
        this.bones.get(p)!.add(b);
        b.position.copy(at).sub(B.J[p]!);
      } else b.position.copy(at);
    }
    const root = this.bones.get("root")!;
    this.group.add(root);
    this.group.add(this.item);
    this.group.name = `person-${cohort}`;
    this.group.updateMatrixWorld(true);
    this.skeleton = new THREE.Skeleton(this.bonesArr);
    // Sockets: the grip frames carry the grip's orientation.
    for (const side of SIDES) this.bones.get(`grip_${side}`)!.quaternion.copy(B.hands[side].toolQ);
    this.bones.get("claw_grip_L")!.quaternion.copy(B.hands.L.toolQ);
    this.bones.get("pinch_grip_R")!.quaternion.copy(B.hands.R.toolQ);

    const parts = (opts.dress ?? plainDress(opts.seed ?? 1))(B, T, detail);
    let tris = 0;
    const shadows = opts.shadows !== false;
    const H = B.topY;
    for (const part of parts) {
      const g = part.geometry;
      const n = (g.index ? g.index.count : g.getAttribute("position").count) / 3;
      if (!n) continue;
      const mesh = new THREE.SkinnedMesh(g, part.material);
      mesh.name = `${this.group.name}-${part.key}`;
      mesh.castShadow = shadows && part.castShadow !== false;
      mesh.receiveShadow = shadows;
      mesh.bind(this.skeleton, new THREE.Matrix4());
      // A generous fixed bound, so culling never trims a limb mid-stride.
      mesh.boundingSphere = new THREE.Sphere(V(0, H * 0.5, 0), H * 0.95);
      mesh.frustumCulled = true;
      this.group.add(mesh);
      this.meshes.push(mesh);
      if (!part.shared) this.own.push(g);
      this.trisBy[part.key] = (this.trisBy[part.key] ?? 0) + n;
      tris += n;
    }
    this.tris = tris;
    this.place(0, 0, 0);
    this.stand(0);
  }

  /** The grip frame where a held tool's handle goes (the grip_L / grip_R bone). */
  hand(side: Side): THREE.Object3D {
    return this.bones.get(`grip_${side}`)!;
  }
  bone(name: string): THREE.Bone {
    return this.bones.get(name)!;
  }
  get massKg(): number {
    return this.dims.body_mass_kg;
  }
  /** Stand at (x, z) on the floor facing yaw (0 faces +z; positive turns to the person's left). */
  place(x: number, z: number, yaw: number): void {
    this.placed = { x, z, yaw };
    // The height stays the caller's: a person on a bench or a platform stands at the group's y.
    this.group.position.set(x, this.group.position.y, z);
    this.group.rotation.set(0, yaw, 0);
    this.group.updateMatrix();
  }
  get placement(): { x: number; z: number; yaw: number } {
    return { ...this.placed };
  }
  /** World point to body space (relative to the current placement). */
  toBody(p: THREE.Vector3, out = V()): THREE.Vector3 {
    const c = Math.cos(this.placed.yaw);
    const s = Math.sin(this.placed.yaw);
    const dx = p.x - this.placed.x;
    const dz = p.z - this.placed.z;
    return out.set(c * dx - s * dz, p.y, s * dx + c * dz);
  }
  /** Body-space point to world. */
  toWorld(p: THREE.Vector3, out = V()): THREE.Vector3 {
    const c = Math.cos(this.placed.yaw);
    const s = Math.sin(this.placed.yaw);
    return out.set(this.placed.x + c * p.x + s * p.z, p.y, this.placed.z - s * p.x + c * p.z);
  }

  /* -------------------------------------------------------------------------------------------------------- */
  /* Forward kinematics bookkeeping in body space                                                              */

  private idx(name: string): number {
    return BI[name]!;
  }
  /** Set a bone's local rotation and update its body-space frame from its parent's. */
  private setLocal(name: string, q: THREE.Quaternion): void {
    const i = this.idx(name);
    const b = this.bonesArr[i]!;
    b.quaternion.copy(q);
    const p = parentOf(name);
    if (!p) {
      this.Pw[i]!.copy(b.position);
      this.Qw[i]!.copy(q);
      return;
    }
    const pi = this.idx(p);
    this.Pw[i]!.copy(b.position).applyQuaternion(this.Qw[pi]!).add(this.Pw[pi]!);
    this.Qw[i]!.copy(this.Qw[pi]!).multiply(q);
  }
  /** Set a bone's body-space rotation (its parent's frame must be current). */
  private setWorld(name: string, q: THREE.Quaternion): void {
    const p = parentOf(name)!;
    _q3.copy(this.Qw[this.idx(p)]!).invert().multiply(q);
    this.setLocal(name, _q3);
  }
  /** Body-space position of a bone's joint. */
  jointPos(name: string, out = V()): THREE.Vector3 {
    return out.copy(this.Pw[this.idx(name)]!);
  }
  jointQuat(name: string, out = new THREE.Quaternion()): THREE.Quaternion {
    return out.copy(this.Qw[this.idx(name)]!);
  }

  /* -------------------------------------------------------------------------------------------------------- */
  /* Applying a pose                                                                                           */

  pose(p: BodyPose): void {
    if (p !== this.current) copyPose(this.current, p);
    for (const k of Object.keys(this.rom)) this.rom[k] = 0;
    // Root and pelvis.
    this.setLocal("root", _q1.identity());
    const pel = this.bones.get("pelvis")!;
    pel.position.copy(p.pelvis);
    _q1.setFromAxisAngle(Y, p.yaw).multiply(qAxis(X, p.tilt, _q2)).multiply(qAxis(Z, p.roll, _q3));
    this.setLocal("pelvis", _q1);
    // Spine: flexion mostly lumbar, twist mostly thoracic.
    const fl = [0.48, 0.32, 0.2];
    const tw = [0.2, 0.35, 0.45];
    const names = ["spine_01", "spine_02", "spine_03"];
    for (let k = 0; k < 3; k++) {
      _q1.setFromAxisAngle(Y, p.twist * tw[k]!).multiply(qAxis(X, p.flex * fl[k]!, _q2)).multiply(qAxis(Z, p.side / 3, _q3));
      this.setLocal(names[k]!, _q1);
    }
    this.romNote("torso_flex", (p.flex + 0) / DEG);
    this.romNote("torso_yaw", p.twist / DEG);
    this.setLocal("carry_chest", _q1.identity());
    // Neck and head turn towards the gaze point.
    this.look(p.gaze);
    // Arms.
    for (let s = 0; s < 2; s++) this.arm(SIDES[s]!, p.arms[s]!, p.clav[s * 2]!, p.clav[s * 2 + 1]!);
    // Legs.
    for (let s = 0; s < 2; s++) this.leg(SIDES[s]!, p.feet[s]!);
    // Item socket.
    this.item.visible = true;
    if (p.itemOn) {
      this.item.position.copy(p.itemPos);
      this.item.quaternion.copy(p.itemQuat);
    }
  }

  /**
   * The body as capsules in world space for the current pose: trunk, upper arms, forearms, thighs and shins (and
   * the head), for a scene's navigation or fixtures to keep bodies out of counters and each other.
   */
  capsules(out: Capsule[] = []): Capsule[] {
    const s = this.B.s;
    const segs: [string, string, string, number][] = [
      ["trunk", "spine_01", "neck", this.dims.torso_capsule_radius_m],
      ["head", "head", "gaze_target", 0.1 * s],
      ["upper_arm_L", "upper_arm_L", "forearm_L", 0.048 * s],
      ["upper_arm_R", "upper_arm_R", "forearm_R", 0.048 * s],
      ["forearm_L", "forearm_L", "hand_L", 0.04 * s],
      ["forearm_R", "forearm_R", "hand_R", 0.04 * s],
      ["thigh_L", "thigh_L", "shin_L", 0.075 * s],
      ["thigh_R", "thigh_R", "shin_R", 0.075 * s],
      ["shin_L", "shin_L", "foot_L", 0.055 * s],
      ["shin_R", "shin_R", "foot_R", 0.055 * s],
    ];
    out.length = segs.length;
    segs.forEach(([name, a, b, r], i) => {
      const c = out[i] ?? { name, a: V(), b: V(), r };
      c.name = name;
      c.r = r;
      this.toWorld(this.Pw[this.idx(a)]!, c.a);
      if (b === "gaze_target") c.b.copy(this.Pw[this.idx("head")]!).add(_sw1.set(0, 0.16 * s, 0.02 * s));
      else c.b.copy(this.Pw[this.idx(b)]!);
      this.toWorld(c.b, c.b);
      out[i] = c;
    });
    return out;
  }
  private capsCache: Capsule[] = [];
  /**
   * The deepest horizontal push (world) that `obstacles` asks for over the body's capsules this pose, or null when
   * the body is clear (or there is no query).
   */
  collide(): THREE.Vector3 | null {
    if (!this.obstacles) return null;
    let best: THREE.Vector3 | null = null;
    for (const c of this.capsules(this.capsCache)) {
      const push = this.obstacles(c);
      if (push && (!best || push.lengthSq() > best.lengthSq())) best = push.clone();
    }
    return best;
  }

  private romNote(key: RomKey, deg: number): void {
    const [lo, hi] = this.dims.joint_rom_deg[key];
    const over = deg < lo ? lo - deg : deg > hi ? deg - hi : 0;
    if (over >= (this.rom[key] ?? 0)) this.romAngle[key] = deg;
    this.rom[key] = Math.max(this.rom[key] ?? 0, over);
  }

  private look(g: THREE.Vector3): void {
    // Gaze direction in the upper thorax's frame.
    const i3 = this.idx("spine_03");
    const eye = _v1.set(0, this.B.eyeY, 0.08).sub(this.B.J.spine_03!).applyQuaternion(this.Qw[i3]!).add(this.Pw[i3]!);
    const dir = _v2.subVectors(g, eye).applyQuaternion(_q1.copy(this.Qw[i3]!).invert());
    const yaw = Math.atan2(dir.x, dir.z);
    const pitch = Math.atan2(-dir.y, Math.hypot(dir.x, dir.z));
    // The eyes take a share of the turn; the neck and head do the rest.
    const y = clamp(yaw * 0.85, -60 * DEG, 60 * DEG);
    const pt = clamp(pitch * 0.72, -35 * DEG, 50 * DEG);
    this.setLocal("neck", _q1.setFromAxisAngle(Y, y * 0.45).multiply(qAxis(X, pt * 0.55, _q2)));
    this.setLocal("head", _q1.setFromAxisAngle(Y, y * 0.55).multiply(qAxis(X, pt * 0.45, _q2)));
    this.setLocal("gaze_target", _q1.identity());
    this.romNote("neck_yaw", (y * 0.45) / DEG);
    this.romNote("neck_pitch", (pt * 0.55) / DEG);
  }

  /** Grip offset and canonical frame for a grip kind. */
  private gripFrame(side: Side, kind: GripKind): { off: THREE.Vector3; a0: THREE.Vector3; b0: THREE.Vector3 } {
    const H = this.B.hands[side];
    if (kind === "tool") return { off: H.toolPos, a0: V(0, 0, 1).applyQuaternion(H.toolQ), b0: V(0, 1, 0).applyQuaternion(H.toolQ) };
    if (kind === "dagger") return { off: H.toolPos, a0: V(0, 0, -1).applyQuaternion(H.toolQ), b0: V(0, 1, 0).applyQuaternion(H.toolQ) };
    if (kind === "bar") return { off: H.barPos, a0: H.barAxis, b0: H.dn };
    if (kind === "hook") return { off: H.hookPos, a0: H.barAxis, b0: H.dn };
    return { off: H.palmPos, a0: H.da, b0: H.dn };
  }

  private arm(side: Side, a: ArmPose, elev: number, prot: number): void {
    const B = this.B;
    const H = B.hands[side];
    const sd = H.sd;
    // Clavicle.
    _q1.setFromAxisAngle(Z, sd * elev).multiply(qAxis(Y, -sd * prot, _q2));
    this.setLocal(`clavicle_${side}`, _q1);
    const J = B.J;
    const u0 = _v1.subVectors(J[`forearm_${side}`]!, J[`upper_arm_${side}`]!).normalize().clone();
    const f0 = _v1.subVectors(J[`hand_${side}`]!, J[`forearm_${side}`]!).normalize().clone();
    const n0u = V().crossVectors(u0, Z).normalize();
    const n0f = V().crossVectors(f0, Z).normalize();
    const kw = V().crossVectors(H.da, H.dn).normalize();
    // Free swing (FK), as local rotations.
    const fkUp = new THREE.Quaternion().setFromAxisAngle(X, -a.flex).multiply(qAxis(Z, sd * a.abd, _q2)).multiply(qAxis(u0, a.twist, _q3));
    const fkFo = new THREE.Quaternion().setFromAxisAngle(n0f, a.elbow);
    const fkHa = new THREE.Quaternion().setFromAxisAngle(kw, 0.12);
    let upL = fkUp;
    let foL = fkFo;
    let haL = fkHa;
    if (a.ik > 0) {
      const ik = this.solveArm(side, a, u0, f0, n0u, n0f, kw);
      if (a.ik >= 1) {
        upL = ik[0];
        foL = ik[1];
        haL = ik[2];
      } else {
        upL = fkUp.clone().slerp(ik[0], a.ik);
        foL = fkFo.clone().slerp(ik[1], a.ik);
        haL = fkHa.clone().slerp(ik[2], a.ik);
      }
    }
    this.setLocal(`upper_arm_${side}`, upL);
    this.setLocal(`forearm_${side}`, foL);
    this.setLocal(`hand_${side}`, haL);
    // ROM bookkeeping: shoulder flexion/abduction of the upper arm in the thorax, elbow flexion.
    const i3 = this.idx("spine_03");
    const ua = _v3.subVectors(this.Pw[this.idx(`forearm_${side}`)]!, this.Pw[this.idx(`upper_arm_${side}`)]!).normalize().applyQuaternion(_q1.copy(this.Qw[i3]!).invert());
    this.romNote("shoulder_flex", Math.atan2(ua.z, -ua.y) / DEG);
    this.romNote("shoulder_abd", Math.max(0, Math.atan2(sd * ua.x, -ua.y) / DEG));
    const fa = _v4.subVectors(this.Pw[this.idx(`hand_${side}`)]!, this.Pw[this.idx(`forearm_${side}`)]!).normalize();
    const ub = _v3.subVectors(this.Pw[this.idx(`forearm_${side}`)]!, this.Pw[this.idx(`upper_arm_${side}`)]!).normalize();
    this.romNote("elbow_flex", Math.acos(clamp(ub.dot(fa), -1, 1)) / DEG);
    // Fingers.
    this.fingers(side, a.shape, a.close, a.radius);
    // The grip socket carries the tool: its +z is the handle's axis as the fist holds it (towards the working end),
    // its +y the tool's up as asked; a hand that rolled round a round handle does not roll the tool with it.
    if (a.ik > 0 && (a.grip === "tool" || a.grip === "dagger")) {
      const Qhand = this.Qw[this.idx(`hand_${side}`)]!;
      const zh = V(0, 0, a.grip === "dagger" ? -1 : 1).applyQuaternion(H.toolQ).applyQuaternion(Qhand);
      const yh = orthoTo(a.b.clone(), zh);
      const Qt = frameQuat(Z, Y, zh, yh, new THREE.Quaternion());
      const local = Qhand.clone().invert().multiply(Qt);
      this.setLocal(`grip_${side}`, a.ik >= 1 ? local : H.toolQ.clone().slerp(local, a.ik));
    } else this.setLocal(`grip_${side}`, H.toolQ);
    if (side === "L") this.setLocal("claw_grip_L", H.toolQ);
    else this.setLocal("pinch_grip_R", H.toolQ);
  }

  /**
   * Two-bone arm IK to the grip target, inside the joint limits; returns local rotations (upper, fore, hand).
   * The elbow's swivel about the shoulder-wrist line is chosen, near the pose's pole, to keep the forearm's twist
   * and the wrist's flexion and deviation inside their ranges and the elbow out of the trunk, as people do. What
   * still cannot be met is given up in the hand's orientation, never in the joints.
   */
  private solveArm(side: Side, a: ArmPose, u0: THREE.Vector3, f0: THREE.Vector3, n0u: THREE.Vector3, n0f: THREE.Vector3, kw: THREE.Vector3): [THREE.Quaternion, THREE.Quaternion, THREE.Quaternion] {
    const B = this.B;
    const H = B.hands[side];
    const sd = H.sd;
    const gf = this.gripFrame(side, a.grip);
    // The shoulder joint as the clavicle now carries it (its own stored frame is from the last pose).
    const ic = this.idx(`clavicle_${side}`);
    const Qcl = this.Qw[ic]!.clone();
    const S = this.bonesArr[this.idx(`upper_arm_${side}`)]!.position.clone().applyQuaternion(Qcl).add(this.Pw[ic]!);
    const lu = B.upper;
    const lf = B.fore;
    const aT = a.a.clone().normalize();
    const bT = orthoTo(a.b.clone(), aT);
    const a0 = gf.a0.clone().normalize();
    const b0 = orthoTo(gf.b0.clone(), a0);
    const dMin = Math.sqrt(lu * lu + lf * lf - 2 * lu * lf * Math.cos(Math.PI - 140 * DEG));
    const FL: [number, number] = [-62 * DEG, 68 * DEG];
    const DV: [number, number] = [-32 * DEG, 18 * DEG];
    const TW = 85 * DEG;
    const over = (x: number, [lo, hi]: [number, number]) => (x < lo ? lo - x : x > hi ? x - hi : 0);
    // One solve for a hand orientation and an elbow pole.
    let P = a.pos.clone();
    const solve = (Qh: THREE.Quaternion, poleIn: THREE.Vector3) => {
      const W = P.clone().sub(gf.off.clone().applyQuaternion(Qh));
      const dirV = V().subVectors(W, S);
      const short = Math.max(0, dirV.length() - (lu + lf) * 0.9995);
      const d = clamp(dirV.length(), dMin, (lu + lf) * 0.9995);
      const dir = dirV.normalize();
      const alpha = Math.acos(clamp((lu * lu + d * d - lf * lf) / (2 * lu * d), -1, 1));
      let pole = orthoTo(poleIn.clone(), dir, V(sd, 0, 0));
      let E = S.clone().addScaledVector(dir, lu * Math.cos(alpha)).addScaledVector(pole, lu * Math.sin(alpha));
      let inside = 0;
      for (let k = 0; k < 8 && this.insideTrunk(E, 0.052 * B.s); k++) {
        inside++;
        pole = orthoTo(pole.lerp(orthoTo(V(sd, 0, 0), dir), 0.3), dir);
        E = S.clone().addScaledVector(dir, lu * Math.cos(alpha)).addScaledVector(pole, lu * Math.sin(alpha));
      }
      const Wc = S.clone().addScaledVector(dir, d);
      const u = V().subVectors(E, S).normalize();
      const f = V().subVectors(Wc, E).normalize();
      let n = V().crossVectors(u, f);
      if (n.lengthSq() < 1e-6) n = V().crossVectors(u, pole.clone().negate());
      n.normalize();
      const Qup = frameQuat(u0, orthoTo(n0u.clone(), u0), u, n, new THREE.Quaternion());
      const Qfo0 = frameQuat(f0, orthoTo(n0f.clone(), f0), f, n, new THREE.Quaternion());
      // The forearm takes the twist (pronation, supination) the hand needs; the wrist takes the rest.
      const want = wrapPi(twistAngle(Qfo0.clone().invert().multiply(Qh), f0));
      const tau = clamp(want, -TW, TW);
      const Qfo = Qfo0.clone().multiply(qAxis(f0, tau));
      const hl = Qfo.clone().invert().multiply(Qh);
      const v = H.da.clone().applyQuaternion(hl);
      const flex = Math.atan2(v.dot(H.dn), v.dot(H.da));
      // Deviation towards the thumb (radial) positive.
      const dev = Math.atan2(v.dot(H.dt), v.dot(H.da));
      const rest = wrapPi(twistAngle(hl, H.da));
      const cost = over(flex, FL) + over(dev, DV) + Math.abs(want - tau) + Math.max(0, Math.abs(rest) - 10 * DEG) + inside * 0.3 + short * 12;
      return { Qup, Qfo, hl, flex, dev, rest, cost, pole, tau, want, short };
    };
    // Swivel the elbow round the shoulder-wrist line, preferring the pose's own pole; on a round handle the hand
    // may also roll round it.
    let best: (ReturnType<typeof solve> & { Qh: THREE.Quaternion; P: THREE.Vector3 }) | null = null;
    const rolls = a.roll > 0 ? [0, 30, -30, 60, -60, 90, -90, 120, -120, 150, -150, 180].filter((r) => Math.abs(r) * DEG <= a.roll + 1e-6) : [0];
    const swivels = [0, 12, -12, 25, -25, 40, -40, 55, -55, 70, -70, 90, -90, 110];
    const trial = (rd: number, sp: number, deg: number) => {
      const qs = qAxis(a.spinAxis, sp * DEG);
      const aS = aT.clone().applyQuaternion(qs);
      const bS = bT.clone().applyQuaternion(qs).applyAxisAngle(aS, rd * DEG);
      P = a.pos.clone().sub(a.spinPivot).applyQuaternion(qs).add(a.spinPivot).addScaledVector(a.spinShift, Math.sin(sp * DEG));
      const Qh0 = frameQuat(a0, b0, aS, bS, new THREE.Quaternion());
      const W0 = P.clone().sub(gf.off.clone().applyQuaternion(Qh0));
      const axis = V().subVectors(W0, S).normalize();
      const r = solve(Qh0, a.pole.clone().applyAxisAngle(axis, sd * deg * DEG));
      r.cost += Math.abs(deg) * DEG * 0.08 + Math.abs(rd) * DEG * 0.03 + sp * DEG * 0.04;
      return { ...r, Qh: Qh0, P: P.clone() };
    };
    // The spin, where allowed, lays the fingers along the forearm (a straight wrist): a smooth function of the
    // target, found by a few fixed-point passes, so the hand never flicks between samples.
    let spin = 0;
    if (a.spinMax > 0) {
      const ax = a.spinAxis.clone().normalize();
      const e1 = orthoTo(aT.clone(), ax);
      const e2 = V().crossVectors(ax, e1);
      for (let it = 0; it < 4; it++) {
        const r = trial(0, spin, 0);
        const f = f0.clone().applyQuaternion(r.Qfo);
        const fp = f.addScaledVector(ax, -f.dot(ax));
        spin = clamp(Math.atan2(fp.dot(e2), fp.dot(e1)) / DEG, 0, a.spinMax / DEG);
      }
    }
    const spins = [spin];
    // Every roll and swivel sample is scored; the choice is their soft minimum (cost-weighted mean), so the elbow
    // and the hand move continuously as the target moves, never jumping between samples.
    const samples: [number, number, number][] = [];
    let cMin = Infinity;
    for (const rd of rolls)
      for (const sp of spins)
        for (const deg of swivels) {
          const c = trial(rd, sp, deg).cost;
          samples.push([rd, deg, c]);
          cMin = Math.min(cMin, c);
        }
    const TEMP = 0.04;
    let wS = 0;
    let rS = 0;
    let dS = 0;
    for (const [rd, deg, c] of samples) {
      const w = Math.exp(-(c - cMin) / TEMP);
      wS += w;
      rS += w * rd;
      dS += w * deg;
    }
    const bestAt: [number, number, number] = [rS / wS, spin, dS / wS];
    best = trial(bestAt[0], spin, bestAt[2]);
    P = best!.P;
    this.reachTarget[side].copy(P);
    const Qh0 = best!.Qh;
    let { Qup, Qfo, hl } = best!;
    const pole = best!.pole;
    // If the wrist is still out of range, bring it inside and let the hand follow; re-solve the reach.
    let Qh = Qh0;
    for (let iter = 0; iter < 6; iter++) {
      const r = iter === 0 ? best! : solve(Qh, pole);
      Qup = r.Qup;
      Qfo = r.Qfo;
      hl = r.hl;
      const flexC = clamp(r.flex, FL[0], FL[1]);
      const devC = clamp(r.dev, DV[0], DV[1]);
      const restC = clamp(r.rest, -10 * DEG, 10 * DEG);
      if (flexC === r.flex && devC === r.dev && restC === r.rest) break;
      hl = qAxis(kw, flexC).multiply(qAxis(H.dn, sd * devC)).multiply(qAxis(H.da, restC));
      Qh = Qfo.clone().multiply(hl);
    }
    this.armDebug[side] = [best!.cost / DEG, best!.flex / DEG, best!.dev / DEG, best!.rest / DEG, best!.tau / DEG, best!.want / DEG, best!.short * 1000, bestAt[1], bestAt[2]];
    const vf = H.da.clone().applyQuaternion(hl);
    this.romNote("wrist_flex", Math.atan2(vf.dot(H.dn), vf.dot(H.da)) / DEG);
    const upL = Qcl.clone().invert().multiply(Qup);
    const foL = Qup.clone().invert().multiply(Qfo);
    return [upL, foL, hl];
  }

  /** Is a body-space point within `r` of the trunk's surface or inside it? */
  insideTrunk(p: THREE.Vector3, r: number): boolean {
    // Bring the point into the bind frame of the nearest spine segment.
    const seg = p.y > this.Pw[this.idx("spine_03")]!.y - 0.02 ? "spine_03" : p.y > this.Pw[this.idx("spine_02")]!.y ? "spine_02" : "spine_01";
    const i = this.idx(seg);
    const q = _q2.copy(this.Qw[i]!).invert();
    const b = _v4.subVectors(p, this.Pw[i]!).applyQuaternion(q).add(this.B.J[seg]!);
    if (b.y < this.B.hemY || b.y > this.B.neckBaseY) return false;
    const S = this.torso.sect(b.y);
    const zz = b.z - S.c;
    const dz = zz >= 0 ? S.f + r : S.b + r;
    return Math.abs(b.x) ** S.n / (S.a + r) ** S.n + Math.abs(zz) ** S.n / dz ** S.n < 1;
  }

  private leg(side: Side, f0: FootPose): void {
    // The ankle's dorsiflexion (the shin's lean over the foot) has a limit (20 degrees in the table): where a pose
    // would pass it, the heel comes up and the foot turns on its ball joint, the toes staying down, as people do in
    // a deep crouch or a pivot.
    const lim = this.dims.joint_rom_deg.ankle_pitch[1] - 2;
    const df = this.legSolve(side, f0, false);
    if (df <= lim) {
      this.legSolve(side, f0, true);
      return;
    }
    const B = this.B;
    const f = this.liftHeel;
    // Raising the heel by an angle takes back about 0.6 of it at the ankle (the knee straightens a little too).
    const raise = ((df - lim) / 0.6) * DEG;
    // Ankle about the ball's contact on the floor (not the joint above it, which would drag the sole): in the foot's
    // frame the ankle sits mtpFwd behind it and ankY above. The toes stay put (see legSolve).
    const ball = V(0, -B.ankY, B.mtpFwd).applyQuaternion(f0.quat).add(f0.pos);
    f.quat.copy(f0.quat).multiply(qAxis(X, raise, _q1));
    f.pos.copy(V(0, B.ankY, -B.mtpFwd).applyQuaternion(f.quat)).add(ball);
    f.toe = f0.toe + raise;
    this.legSolve(side, f, true);
  }
  private liftHeel: FootPose = { pos: V(), quat: new THREE.Quaternion(), toe: 0 };

  /** Two-bone leg to the foot; with `apply` false only measures the ankle's dorsiflexion it would need (degrees). */
  private legSolve(side: Side, f: FootPose, apply: boolean): number {
    const B = this.B;
    const J = B.J;
    const lt = B.thigh;
    const ls = B.shin;
    const ip = this.idx("pelvis");
    const Hp = _v1.subVectors(J[`thigh_${side}`]!, J.pelvis!).applyQuaternion(this.Qw[ip]!).add(this.Pw[ip]!).clone();
    const A = f.pos;
    const dirV = V().subVectors(A, Hp);
    let d = dirV.length();
    const dir = dirV.normalize();
    d = clamp(d, Math.abs(lt - ls) + 0.05, (lt + ls) * 0.99995);
    const cosA = clamp((lt * lt + d * d - ls * ls) / (2 * lt * d), -1, 1);
    const alpha = Math.acos(cosA);
    // The knee tracks over the foot (a touch outward).
    const fwd = Z.clone().applyQuaternion(f.quat);
    const pole = orthoTo(fwd.addScaledVector(X, sgn(side) * 0.12), dir);
    const K = Hp.clone().addScaledVector(dir, lt * Math.cos(alpha)).addScaledVector(pole, lt * Math.sin(alpha));
    const Ac = Hp.clone().addScaledVector(dir, d);
    const t = V().subVectors(K, Hp).normalize();
    const sv = V().subVectors(Ac, K).normalize();
    let n = V().crossVectors(t, sv);
    if (n.lengthSq() < 1e-6) n = V().crossVectors(t, pole.clone().negate());
    n.normalize();
    const t0 = V().subVectors(J[`shin_${side}`]!, J[`thigh_${side}`]!).normalize();
    const s0 = V().subVectors(J[`foot_${side}`]!, J[`shin_${side}`]!).normalize();
    const nt0 = orthoTo(V().crossVectors(t0, Z.clone().negate()), t0);
    const ns0 = orthoTo(V().crossVectors(s0, Z.clone().negate()), s0);
    if (!apply) {
      const qs = frameQuat(s0, ns0, sv, n, new THREE.Quaternion()).invert();
      const fl = Z.clone().applyQuaternion(f.quat).applyQuaternion(qs);
      return Math.atan2(fl.y, fl.z) / DEG;
    }
    this.setWorld(`thigh_${side}`, frameQuat(t0, nt0, t, n, _q1));
    this.setWorld(`shin_${side}`, frameQuat(s0, ns0, sv, n, _q1));
    this.setWorld(`foot_${side}`, f.quat);
    // A planted toe stays where it is: with the heel up the foot turns about the floor under the ball, and the toe
    // joint is held straight above that point (the sole bends there) instead of swinging forward with the foot.
    {
      const J = this.B.J;
      const tb = this.bonesArr[this.idx(`toe_${side}`)]!;
      const rest = _tv1.copy(J[`toe_${side}`]!).sub(J[`foot_${side}`]!);
      const fw = _tv2.set(0, 0, 1).applyQuaternion(f.quat);
      const heelUp = Math.max(0, -Math.atan2(fw.y, Math.hypot(fw.x, fw.z)));
      const plant = heelUp > 1e-4 ? clamp(f.toe / heelUp, 0, 1) : 0;
      if (plant > 0) {
        const h = J[`toe_${side}`]!.y;
        const up = _tv2.set(0, h, 0).applyQuaternion(_tq.copy(f.quat).invert());
        rest.addScaledVector(up.sub(_tv3.set(0, h, 0)), plant);
      }
      tb.position.copy(rest);
    }
    this.setLocal(`toe_${side}`, qAxis(X, -f.toe, _q1));
    // ROM: hip flexion against the pelvis, knee flexion, ankle pitch against the shin.
    const tp = t.clone().applyQuaternion(_q2.copy(this.Qw[ip]!).invert());
    this.romNote("hip_flex", Math.atan2(tp.z, -tp.y) / DEG);
    this.romNote("knee_flex", Math.acos(clamp(t.dot(sv), -1, 1)) / DEG);
    const fl = Z.clone().applyQuaternion(f.quat).applyQuaternion(_q2.copy(this.Qw[this.idx(`shin_${side}`)]!).invert());
    const df = Math.atan2(fl.y, fl.z) / DEG;
    this.romNote("ankle_pitch", df);
    return df;
  }

  /** Finger angles for a shape, solved against the handle for tool, bar and hook grips. */
  private fingers(side: Side, shape: HandShape, close: number, radius: number): void {
    const H = this.B.hands[side];
    let target: Record<Finger, [number, number, number]> = RELAX;
    if (shape === "claw") target = CLAW;
    else if (shape === "flat") target = FLAT;
    else if (shape === "fist") target = FIST;
    else if (shape === "point") target = POINT;
    else if (shape === "tool" || shape === "bar" || shape === "hook") target = this.wrap(side, shape, radius);
    else if (shape === "grasp") target = this.grasp(side);
    const c = clamp(close, 0, 1);
    for (const f of FINGERS) {
      const g = H.digits[f];
      const t = target[f];
      // A grasp opens from its contact towards a flat hand, never bending past the contact (into the item).
      const r = shape === "grasp" ? (FLAT[f].map((x, k) => Math.min(x, t[k]!)) as [number, number, number]) : RELAX[f];
      for (let k = 0; k < 3; k++) this.setLocal(`${f}_0${k + 1}_${side}`, qAxis(g.k, lerp(r[k]!, t[k]!, c), _q1));
    }
  }

  /** Close each digit round a cylinder until it touches, joint by joint (computed once per handle size). */
  private wrap(side: Side, kind: "tool" | "bar" | "hook", radius: number): Record<Finger, [number, number, number]> {
    const key = `${side}${kind}${radius.toFixed(4)}`;
    const hit = this.wrapCache.get(key);
    if (hit) return hit;
    const H = this.B.hands[side];
    const gf = this.gripFrame(side, kind);
    const O = H.wrist.clone().add(gf.off);
    const D = kind === "tool" ? V(0, 0, 1).applyQuaternion(H.toolQ) : H.barAxis.clone();
    const r = kind === "hook" ? Math.min(radius, 0.006) : radius;
    const dist = (p: THREE.Vector3) => {
      const w = _v4.subVectors(p, O);
      return w.addScaledVector(D, -w.dot(D)).length();
    };
    const out = {} as Record<Finger, [number, number, number]>;
    for (const f of FINGERS) {
      const g = H.digits[f];
      const thumb = f === "thumb";
      const pts = (a: [number, number, number]) => {
        const P: THREE.Vector3[] = [g.base.clone()];
        let ang = 0;
        for (let k = 0; k < 3; k++) {
          ang += a[k]!;
          P.push(P[k]!.clone().addScaledVector(g.dir.clone().applyAxisAngle(g.k, ang), g.len[k]!));
        }
        return P;
      };
      const touches = (a: [number, number, number], from: number) => {
        const P = pts(a);
        for (let k = from; k < 3; k++)
          for (let m = 1; m <= 4; m++) {
            const q = P[k]!.clone().lerp(P[k + 1]!, m / 4);
            if (dist(q) < r + g.rad * (m === 4 && k === 2 ? 0.75 : 0.92)) return true;
          }
        return false;
      };
      const max: [number, number, number] = thumb ? [0.75, 0.8, 0.95] : kind === "hook" ? [0.95, 1.5, 0.9] : [1.45, 1.75, 1.25];
      const a: [number, number, number] = [0, 0, 0];
      {
        // Stage 1: all joints together until something touches.
        let u = 0;
        if (!touches(a, 0)) {
          for (; u <= 1; u += 0.02) if (touches([max[0] * u, max[1] * u, max[2] * u], 0)) break;
          u = Math.max(0, u - 0.02);
        }
        a[0] = max[0] * u;
        a[1] = max[1] * u;
        a[2] = max[2] * u;
        // Stage 2: the middle and end joints wrap on.
        for (let v = 0; v < 60 && a[1] < max[1] * 1.08 && !touches([a[0], a[1] + 0.03, a[2] + 0.02], 1); v++) {
          a[1] += 0.03;
          a[2] += 0.02;
        }
        // Stage 3: the tip curls in.
        for (let v = 0; v < 40 && a[2] < max[2] * 1.1 && !touches([a[0], a[1], a[2] + 0.03], 2); v++) a[2] += 0.03;
      }
      out[f] = a;
    }
    this.wrapCache.set(key, out);
    return out;
  }

  /**
   * Close each digit on the held item, as the hand now lies against it: a finger bends at its knuckle until its pad
   * meets the surface, then its middle and end joints until they do (curling round an edge where there is one); a
   * thumb first opens until it clears the item (over a lip, round an edge), then curls down onto it. Computed from
   * the item's signed distance every pose, so the fingers follow the hand's real contact, never a stored shape.
   */
  private grasp(side: Side): Record<Finger, [number, number, number]> {
    const sdf = this.itemSdf;
    const c = this.current;
    if (!sdf || !c.itemOn) return RELAX;
    const H = this.B.hands[side];
    const ih = this.idx(`hand_${side}`);
    const qi = new THREE.Quaternion().copy(c.itemQuat).invert();
    const R = qi.clone().multiply(this.Qw[ih]!);
    const T = this.Pw[ih]!.clone().sub(c.itemPos).applyQuaternion(qi);
    const Jh = this.B.J[`hand_${side}`]!;
    const tmp = V();
    const dist = (p: THREE.Vector3) => sdf(tmp.copy(p).sub(Jh).applyQuaternion(R).add(T));
    const out = {} as Record<Finger, [number, number, number]>;
    const seg = [V(), V(), V(), V()];
    const dir = V();
    const q = V();
    for (const f of FINGERS) {
      const g = H.digits[f];
      const thumb = f === "thumb";
      const place = (a: readonly number[]) => {
        seg[0]!.copy(g.base);
        let ang = 0;
        for (let k = 0; k < 3; k++) {
          ang += a[k]!;
          dir.copy(g.dir).applyAxisAngle(g.k, ang);
          seg[k + 1]!.copy(seg[k]!).addScaledVector(dir, g.len[k]!);
        }
      };
      // Does any part of the digit from segment `from` on come within its own radius of the surface?
      const touches = (a: readonly number[], from: number) => {
        place(a);
        for (let k = from; k < 3; k++)
          for (let m = 1; m <= 6; m++) {
            q.copy(seg[k]!).lerp(seg[k + 1]!, m / 6);
            // The skin stands about 1.05 radii off the bone line on the pad side (less at the domed tip).
            if (dist(q) < g.rad * (k === 2 && m >= 5 ? (m === 6 ? 0.55 : 1.0) : 1.15)) return true;
          }
        return false;
      };
      const max: [number, number, number] = thumb ? [0.75, 0.85, 0.95] : [1.45, 1.75, 1.25];
      const a: [number, number, number] = thumb ? [0.3, 0.12, 0.1] : [0, 0.12, 0.08];
      if (thumb) {
        // Open the thumb (towards the back of the hand) until it is clear of the item.
        while (a[0] > -0.95 && touches(a, 0)) a[0] -= 0.05;
      } else {
        // A finger pressed into the surface at rest straightens back out of it.
        while (a[0] > -0.35 && touches(a, 0)) a[0] -= 0.04;
        // Knuckle first (the middle and end joints follow a little) until the finger meets the surface.
        for (let v = 0; v < 60 && a[0] < max[0] && !touches([a[0] + 0.025, a[1] + 0.008, a[2] + 0.005], 0); v++) {
          a[0] += 0.025;
          a[1] += 0.008;
          a[2] += 0.005;
        }
      }
      // The middle and end joints wrap on, then the tip curls in, each until it touches.
      for (let v = 0; v < 60 && a[1] < max[1] && !touches([a[0], a[1] + 0.03, a[2] + 0.02], 1); v++) {
        a[1] += 0.03;
        a[2] += 0.02;
      }
      for (let v = 0; v < 40 && a[2] < max[2] && !touches([a[0], a[1], a[2] + 0.03], 2); v++) a[2] += 0.03;
      out[f] = a;
    }
    return out;
  }

  /* -------------------------------------------------------------------------------------------------------- */
  /* Readouts                                                                                                  */

  /** Trunk flexion the body shows: the L5/S1-to-C7 line's lean from vertical, forward positive, degrees. */
  lumbarFlexDeg(): number {
    const l5 = this.Pw[this.idx("spine_01")]!;
    const c7 = this.Pw[this.idx("neck")]!;
    const v = _v1.subVectors(c7, l5);
    // Forward is the pelvis's facing.
    const fwd = _v2.set(0, 0, 1).applyQuaternion(this.Qw[this.idx("pelvis")]!);
    fwd.y = 0;
    fwd.normalize();
    return Math.atan2(v.dot(fwd), v.y) / DEG;
  }
  /**
   * What the L5/S1 estimate needs from the pose: trunk flexion, the hands' horizontal reach from L5/S1, and the held
   * item's: the horizontal distance from L5/S1 to `itemCom` (a point in the item socket's frame, its centre of mass;
   * default the socket's origin), or the hands' reach when nothing is held.
   */
  liftMetrics(itemCom?: THREE.Vector3): { torsoFlexDeg: number; reachM: number; handsY: number; loadReachM: number } {
    const l5 = this.Pw[this.idx("spine_01")]!;
    const gl = this.gripPoint("L", _v3);
    const gr = this.gripPoint("R", _v4);
    const mid = gl.add(gr).multiplyScalar(0.5);
    const reachM = Math.hypot(mid.x - l5.x, mid.z - l5.z);
    let loadReachM = reachM;
    const c = this.current;
    if (c.itemOn) {
      const com = _v1.copy(itemCom ?? ZERO).applyQuaternion(c.itemQuat).add(c.itemPos);
      loadReachM = Math.hypot(com.x - l5.x, com.z - l5.z);
    }
    return { torsoFlexDeg: this.lumbarFlexDeg(), reachM, handsY: mid.y, loadReachM };
  }
  /** How far (metres) a reaching hand's grip point is from where its last solve aimed; 0 for a free arm. */
  gripError(side: Side): number {
    const a = this.current.arms[side === "L" ? 0 : 1]!;
    if (a.ik < 1) return 0;
    const i = this.idx(`hand_${side}`);
    const off = this.gripFrame(side, a.grip).off;
    return off.clone().applyQuaternion(this.Qw[i]!).add(this.Pw[i]!).distanceTo(this.reachTarget[side]);
  }
  /** Body-space grip point of a hand (tool grip). */
  gripPoint(side: Side, out = V()): THREE.Vector3 {
    const i = this.idx(`hand_${side}`);
    return out.copy(this.B.hands[side].toolPos).applyQuaternion(this.Qw[i]!).add(this.Pw[i]!);
  }

  /* -------------------------------------------------------------------------------------------------------- */
  /* Motions at the current placement                                                                          */

  /** The time the next pose belongs to (seconds, any origin). Nothing in the core has inertia now; kept for callers. */
  clock(_t: number): void {}
  /** Stand at ease at the current placement: slow weight shifts and breathing (t in seconds). */
  stand(t = 0): void {
    const p = standPose(this, t, this.scratch);
    fitHips(this, p);
    this.pose(p);
  }
  /**
   * Work at the current placement, t seconds into the work. Targets in `opts.at` are world points (the parent's
   * frame): what to point at, the tray to inspect, the rail to hold, the item's rim centre to pick from or place on.
   */
  work(kind: WorkKind, t: number, opts: WorkOpts = {}): void {
    const p = workPose(this, kind, t, opts, this.scratch);
    fitHips(this, p);
    this.pose(p);
  }

  /** Free what this person owns (geometry a dresser marked shared stays, as do the shared materials). */
  dispose(): void {
    for (const g of this.own) g.dispose();
    this.own.length = 0;
    this.skeleton.dispose();
    this.group.removeFromParent();
  }
}


/* ------------------------------------------------------------------------------------------------------------ */
/* Standing and working at a station. Poses are in body space: the root between the feet, facing +z.             */
/* ------------------------------------------------------------------------------------------------------------ */

export const TOE_OUT = 7 * DEG;

/** Both feet flat at the standing stance. */
export function standFeet(st: Staff, p: BodyPose): void {
  const B = st.B;
  for (let i = 0; i < 2; i++) {
    const sd = i === 0 ? 1 : -1;
    const f = p.feet[i]!;
    f.pos.set(sd * B.standHalf, B.ankY, 0);
    f.quat.setFromAxisAngle(Y, sd * TOE_OUT);
    f.toe = 0;
  }
}

const _fq = new THREE.Quaternion();
const _fv = V();
/**
 * Keep the hips low enough for both legs to reach their feet with the knees just short of locked: the pelvis
 * height becomes a soft minimum of the wanted height and each leg's reach. Returns how far it moved (<= 0).
 */
export function fitHips(st: Staff, p: BodyPose, k = 0.012, weight: readonly [number, number] = [1, 1]): number {
  const B = st.B;
  const J = B.J;
  const Lmax = (B.thigh + B.shin) * 0.9985;
  _fq.setFromAxisAngle(Y, p.yaw).multiply(qAxis(X, p.tilt, _q1)).multiply(qAxis(Z, p.roll, _q2));
  let py = p.pelvis.y;
  for (let i = 0; i < 2; i++) {
    const side = SIDES[i]!;
    const off = _fv.subVectors(J[`thigh_${side}`]!, J.pelvis!).applyQuaternion(_fq);
    const f = p.feet[i]!.pos;
    const d2 = (p.pelvis.x + off.x - f.x) ** 2 + (p.pelvis.z + off.z - f.z) ** 2;
    const reach = f.y + Math.sqrt(Math.max(0, Lmax * Lmax - d2)) - off.y;
    py = smin(py, reach + (1 - clamp(weight[i]!, 0, 1)) * 0.3, k * B.s);
  }
  const dy = py - p.pelvis.y;
  p.pelvis.y = py;
  return dy;
}

/**
 * A carried load's balance: the body's own centre of mass moves the other way by the load's moment over its mass,
 * so that body and load together stay over the feet. Most of it is the hips going back, the rest the trunk leaning
 * back from them; the load and the hands holding it move with the hips. Returns the hips' shift (body z, metres).
 */
export function balanceLoad(st: Staff, p: BodyPose): number {
  if (!p.itemOn || st.itemKg <= 0) return 0;
  const B = st.B;
  const f0 = p.feet[0]!.pos;
  const f1 = p.feet[1]!.pos;
  // The middle of the support: between the feet, a little ahead of the ankles (mid-foot).
  const sx = 0.5 * (f0.x + f1.x);
  const sz = 0.5 * (f0.z + f1.z) + 0.35 * B.toeFwd;
  const k = st.itemKg / st.massKg;
  const dx = -k * (p.itemPos.x - sx);
  const dz = -k * (p.itemPos.z - sz);
  const hip = 0.6;
  p.pelvis.x += hip * dx;
  p.pelvis.z += hip * dz;
  p.itemPos.x += hip * dx;
  p.itemPos.z += hip * dz;
  for (const a of p.arms)
    if (a.ik > 0) {
      a.pos.x += hip * dx;
      a.pos.z += hip * dz;
    }
  // The rest from the trunk: its mass centre sits about 0.28 m above the hips.
  p.flex += Math.atan2((1 - hip) * dz, 0.28 * B.s);
  return hip * dz;
}

export function relaxArms(p: BodyPose): void {
  for (const a of p.arms) {
    a.flex = 0.06;
    a.abd = 0.05;
    a.elbow = 0.24;
    a.twist = 0;
    a.ik = 0;
    a.roll = 0;
    a.spinMax = 0;
    a.shape = "relax";
    a.close = 1;
  }
}

/** Standing at ease: weight drifting slowly from foot to foot, breathing. */
export function standPose(st: Staff, t: number, p: BodyPose = newPose()): BodyPose {
  const B = st.B;
  standFeet(st, p);
  const ph = st.phase * Math.PI * 2;
  const w = Math.sin((t * 2 * Math.PI) / 7.3 + 0.7 * (st.dims.standing_height_m * 10) + ph);
  p.pelvis.set(0.012 * w * B.s, B.hipY - (0.0015 + 0.0015 * Math.abs(w)) * B.s, 0.006 * B.s);
  p.tilt = 0.04;
  // Contrapposto: the loaded side's hip rides higher, the shoulders tilt the other way.
  p.roll = 0.012 * w;
  p.yaw = 0.01 * Math.sin(t * 0.5 + ph);
  p.flex = 0.02 + 0.006 * Math.sin((t * 2 * Math.PI) / 4.2 + ph);
  p.side = -0.02 * w;
  p.twist = 0;
  p.clav = [0.01, 0, 0.01, 0];
  relaxArms(p);
  p.gaze.set(0.3 * Math.sin(t * 0.23 + ph), B.eyeY - 0.15, 3);
  p.itemOn = false;
  return p;
}

/** Reach a hand to hold a tool's handle at `pos`, along `axis` with its top towards `up`, the elbow towards `pole`. */
export function setTool(a: ArmPose, pos: THREE.Vector3, axis: THREE.Vector3, up: THREE.Vector3, pole: THREE.Vector3, radius = 0.012, grip: GripKind = "tool", roll = 0): void {
  a.ik = 1;
  a.grip = grip;
  a.roll = roll;
  a.spinMax = 0;
  a.pos.copy(pos);
  a.a.copy(axis).normalize();
  a.b.copy(up).normalize();
  a.pole.copy(pole).normalize();
  a.shape = "tool";
  a.close = 1;
  a.radius = radius;
}
/** Reach a hand's palm to `pos`, its fingers along `fingers`, the palm facing `facing`, the elbow towards `pole`. */
export function setPalm(a: ArmPose, pos: THREE.Vector3, fingers: THREE.Vector3, facing: THREE.Vector3, pole: THREE.Vector3, shape: HandShape = "flat", close = 1): void {
  a.ik = 1;
  a.roll = 0;
  a.spinMax = 0;
  a.grip = "palm";
  a.pos.copy(pos);
  a.a.copy(fingers).normalize();
  a.b.copy(facing).normalize();
  a.pole.copy(pole).normalize();
  a.shape = shape;
  a.close = close;
}

/** World `at` (if given) to body space, else the default. */
export function target(st: Staff, o: WorkOpts, def: THREE.Vector3): THREE.Vector3 {
  return o.at ? st.toBody(o.at) : def;
}

/**
 * A full-size hotel pan (GN 1/1: 530 x 325 mm over the flange, 100 mm deep), its frame at the rim's centre, long side
 * across the body (x). The walls stand `flange` inside the flange's edge at the top and lean in by `taper` (radians)
 * going down; the rolled flange is `lip` thick. The kitchen's item; the default carry.
 */
export const HOTEL_PAN: CarrySpec = { length: 0.53, width: 0.325, depth: 0.1, flange: 0.0125, gripHalf: 0.27, taper: 0.155, lip: 0.0045 };

/**
 * Signed distance (metres) from a point in the item frame to a carried item's solid: its tapered body (the inside
 * counted as solid, so no finger reaches into it) and its rim. Negative inside.
 */
export function boxSdf(K: CarrySpec): (p: THREE.Vector3) => number {
  const ct = Math.cos(K.taper);
  const tt = Math.tan(K.taper);
  return (p) => {
    const inset = Math.max(0, -p.y) * tt;
    const dx = Math.abs(p.x) - (K.length / 2 - K.flange - inset);
    const dz = Math.abs(p.z) - (K.width / 2 - K.flange - inset);
    const dy = Math.max(-K.depth - p.y, p.y);
    const ex = Math.max(dx * ct, 0);
    const ez = Math.max(dz * ct, 0);
    const ey = Math.max(dy, 0);
    const body = Math.hypot(ex, ez, ey) + Math.min(Math.max(dx * ct, dz * ct, dy), 0);
    const fx = Math.abs(p.x) - K.length / 2;
    const fz = Math.abs(p.z) - K.width / 2;
    const fy = Math.max(-K.lip - p.y, p.y);
    const rim = Math.hypot(Math.max(fx, 0), Math.max(fz, 0), Math.max(fy, 0)) + Math.min(Math.max(fx, fz, fy), 0);
    return Math.min(body, rim);
  };
}

/** Where a carried item rides: at the waist, its near wall a couple of centimetres clear of the belly. */
export function carryFrame(st: Staff, bob: number, pos: THREE.Vector3, q: THREE.Quaternion): void {
  const B = st.B;
  const K = st.carrySpec;
  // A deep item rides higher, so that its bottom stays clear of the thighs as they swing.
  const y = Math.max(B.waistY - 0.03 * B.s, B.hipY - 0.06 * B.s + K.depth * 0.7);
  const belly = Math.max(st.torso.at(y - 0.05, 0).z, st.torso.at(y - 0.1, 0).z, st.torso.at(Math.max(B.hemY, y - K.depth), 0).z);
  pos.set(0, y + bob, belly + 0.022 + K.width / 2 + 0.025);
  q.setFromAxisAngle(X, -0.03);
}

/**
 * Hands at an item's ends as people hold a full one. Carried at the waist (forearms level), each palm lies flat on
 * the outside of an end wall just under the rim, fingers along the wall towards the far side, thumb over the rim
 * and down on it. Reaching down to it (forearms steep), the fingers tip down the wall and the palm rises and stands
 * out onto the rim's edge, the fingers curling in under the rim to the wall. Between the two the arm chooses (a turn
 * about the wall's normal that keeps the wrist straight). `half` moves the walls (default the spec's `gripHalf`).
 * The fingers close on the item by contact (`grasp`).
 */
export function panGrips(st: Staff, p: BodyPose, itemPos: THREE.Vector3, itemQ: THREE.Quaternion, half: number, close: number, ik: number): void {
  const K = st.carrySpec;
  const H = st.B.hands.L;
  const sh = H.sh;
  const tilt0 = 12 * DEG;
  // How far the hand stands above its palm point at a tilt: the thumb side of the palm and wrist (the thumb itself
  // goes over the rim).
  const top = (phi: number) => Math.max(0.0335 * sh * Math.cos(phi) + 0.22 * H.HL * Math.sin(phi), 0.0272 * sh * Math.cos(phi) + 0.36 * H.HL * Math.sin(phi));
  const y = -(K.lip + 0.0035) - top(tilt0);
  const tanT = Math.tan(K.taper);
  const wallX = K.length / 2 - K.flange - -y * tanT + (half - K.gripHalf);
  const ct = Math.cos(K.taper);
  const stT = Math.sin(K.taper);
  for (let i = 0; i < 2; i++) {
    const sd = i === 0 ? 1 : -1;
    const a = p.arms[i]!;
    const n = V(sd * ct, -stT, 0);
    const down = V(-sd * stT, -ct, 0);
    const fingers = V(0, 0, Math.cos(tilt0)).addScaledVector(down, Math.sin(tilt0));
    // The palm point a few millimetres off the wall: the palm's skin (or glove) and the thenar's swell meet it.
    const g = V(sd * wallX, y, -Math.min(0.055, K.width * 0.17)).addScaledVector(n, 0.0055);
    setPalm(
      a,
      g.clone().applyQuaternion(itemQ).add(itemPos),
      fingers.applyQuaternion(itemQ),
      n.clone().negate().applyQuaternion(itemQ),
      V(sd * 0.55, -0.8, -0.25),
      "grasp",
      close,
    );
    a.ik = ik;
    // Fingers from level along the wall to straight down it; the palm rises to the rim and stands out past its
    // edge (by the rim's overhang and a little), so that it never rides into the rim on the way.
    const tip = 78 * DEG;
    a.spinMax = tip;
    // A positive turn about this axis tips the fingers down the wall, on either hand.
    a.spinAxis.copy(n).multiplyScalar(sd).applyQuaternion(itemQ);
    a.spinPivot.copy(g).applyQuaternion(itemQ).add(itemPos);
    const rise = -y - 0.004;
    const out = K.flange + -y * tanT + 0.006;
    a.spinShift.set(sd * out, rise, 0).multiplyScalar(1 / Math.sin(tip)).applyQuaternion(itemQ);
  }
}

/**
 * Both hands on a bar (a handrail, a push bar) in the power grip, `half` either side of `barPos` along the bar's x
 * axis, palms towards the bar's +z and down; a round bar lets the fist roll round it to keep the wrist straight.
 */
export function barGrips(st: Staff, p: BodyPose, barPos: THREE.Vector3, barQ: THREE.Quaternion, half = 0.2 * st.B.s, radius = 0.0125): void {
  for (let i = 0; i < 2; i++) {
    const sd = i === 0 ? 1 : -1;
    const a = p.arms[i]!;
    a.ik = 1;
    a.roll = (75 * Math.PI) / 180;
    a.spinMax = 0;
    a.grip = "bar";
    a.pos.copy(V(sd * half, 0, 0).applyQuaternion(barQ).add(barPos));
    a.a.copy(V(-sd, 0, 0).applyQuaternion(barQ));
    a.b.copy(V(0, -0.45, 0.89).applyQuaternion(barQ));
    a.pole.set(sd * 0.55, -0.8, -0.2).normalize();
    a.shape = "bar";
    a.close = 1;
    a.radius = radius;
  }
}

/**
 * Lower the body for a reach down: c = 0 stands, c = 1 is a half squat with the knees at 85 degrees, the ankles at
 * 18 degrees of dorsiflexion (near their limit) and the trunk leaning 50 degrees; beyond 1 (to 1.3) the legs stay
 * and the back stoops further, as tall people do over a low shelf.
 */
export function crouch(st: Staff, p: BodyPose, c: number): void {
  const B = st.B;
  const cl = Math.min(c, 1);
  const cx = Math.max(0, c - 1);
  const phi = 18 * DEG * cl;
  const kappa = 85 * DEG * cl;
  const ank = V(0, B.ankY, 0);
  const knee = ank.clone().add(V(0, Math.cos(phi), Math.sin(phi)).multiplyScalar(B.shin));
  const th = kappa - phi;
  const hip = knee.clone().add(V(0, Math.cos(th), -Math.sin(th)).multiplyScalar(B.thigh));
  // At c = 0 this is the straight standing leg; keep the knees just unlocked.
  p.pelvis.set(p.pelvis.x, Math.min(hip.y, B.hipY - 0.0015 * B.s), hip.z + 0.006 * B.s);
  const lean = 50 * DEG * cl;
  p.tilt = 0.04 + 0.55 * lean + 0.3 * 50 * DEG * cx;
  p.flex = 0.02 + 0.45 * lean + 0.7 * 50 * DEG * cx;
}

/**
 * Down to a full squat, for the lowest shelves: up to c = 1 the crouch above; on to c = 2 the knees bend on to 125
 * degrees with the shins leaning further over the feet (the heels lift where the ankles run out of range) and the
 * trunk leans on a little, the hips sitting back over the heels.
 */
export function crouchDeep(st: Staff, p: BodyPose, c: number): void {
  if (c <= 1) return crouch(st, p, c);
  const B = st.B;
  const e = Math.min(c - 1, 1);
  const phi = (18 + 17 * e) * DEG;
  const kappa = (85 + 40 * e) * DEG;
  const ank = V(0, B.ankY, 0);
  const knee = ank.clone().add(V(0, Math.cos(phi), Math.sin(phi)).multiplyScalar(B.shin));
  const th = kappa - phi;
  const hip = knee.clone().add(V(0, Math.cos(th), -Math.sin(th)).multiplyScalar(B.thigh));
  p.pelvis.set(p.pelvis.x, hip.y, hip.z + 0.006 * B.s);
  // The pelvis tucks under as the hips go down (keeping the hips inside their range), the spine bending on instead.
  const lean = 50 * DEG;
  p.tilt = 0.04 + 0.55 * lean - 0.48 * e;
  p.flex = 0.02 + 0.45 * lean + 0.42 * e;
  // Past the full squat (c up to 2.6) the back rounds a little further and the shoulders reach forward and down
  // (the shoulder blades round out) to reach the floor.
  const x = clamp(c - 2, 0, 0.6) / 0.6;
  if (x > 0) {
    p.flex += 0.12 * x;
    p.clav[0] = lerp(p.clav[0]!, -0.08, x);
    p.clav[1] = lerp(p.clav[1]!, 0.26, x);
    p.clav[2] = lerp(p.clav[2]!, -0.08, x);
    p.clav[3] = lerp(p.clav[3]!, 0.26, x);
  }
}

/**
 * Up on the balls of the feet by `a` (radians of heel rise), the toes flat: each foot turns about the floor under
 * its ball; with `pelvis` the hips go up by as much as the ankles do.
 */
export function heelRaise(st: Staff, p: BodyPose, a: number, pelvis = true): void {
  if (a <= 1e-4) return;
  const B = st.B;
  let rise = 0;
  for (const f of p.feet) {
    const ballL = V(0, -B.ankY, B.mtpFwd);
    const ball = ballL.clone().applyQuaternion(f.quat).add(f.pos);
    const y0 = f.pos.y;
    f.quat.multiply(qAxis(X, a, _q1));
    f.pos.copy(ballL.applyQuaternion(f.quat)).negate().add(ball);
    f.toe = Math.max(f.toe, a);
    rise += (f.pos.y - y0) / 2;
  }
  if (pelvis) p.pelvis.y += rise;
}

/**
 * A pick's (or place's) phases t seconds in: the crouch as a fraction of the lowest it goes, the hands' reach and
 * grip, and the item's way between the shelf and the hold: `out` slides it out of (or into) the shelf, `go` carries it
 * between there and the hold (0 at the shelf, 1 in the carry). The two overlap a little, so the item comes out and up
 * in one movement, at no more than about a metre a second.
 */
function pickPhase(kind: "pick" | "place", tt: number, high = 0): { c: number; reachW: number; close: number; out: number; go: number } {
  if (kind === "pick") {
    if (tt < 0.2) return { c: 0, reachW: 0, close: 0, out: 0, go: 0 };
    if (tt < 1.6) {
      const u = ease((tt - 0.2) / 1.4);
      // The hands go out once the body is most of the way down; up to a high pan (no crouch) from the start.
      return { c: u, reachW: smooth(lerp(0.5, 0, high), 1, u), close: 0, out: 0, go: 0 };
    }
    if (tt < 1.95) return { c: 1, reachW: 1, close: smooth(1.6, 1.9, tt), out: 0, go: 0 };
    const out = ease(clamp((tt - 1.95) / 0.45, 0, 1));
    const go = mj(clamp((tt - 2.25) / (PICK_S - 2.25), 0, 1));
    return { c: 1 - go, reachW: 1, close: 1, out, go };
  }
  if (tt < 1.85) {
    const go = 1 - mj(clamp(tt / 1.55, 0, 1));
    const out = 1 - ease(clamp((tt - 1.4) / 0.45, 0, 1));
    return { c: 1 - go, reachW: 1, close: 1, out, go };
  }
  if (tt < 2.2) return { c: 1, reachW: 1, close: 1 - smooth(1.85, 2.15, tt), out: 0, go: 0 };
  // Hands off the item first, then up.
  const u = clamp((tt - 2.2) / (PLACE_S - 2.2), 0, 1);
  return { c: 1 - ease(clamp((u - 0.2) / 0.8, 0, 1)), reachW: 1 - smooth(0, 0.3, u), close: 0, out: 0, go: 0 };
}

/** The crouch a pick needs to bring the shoulders within reach of the grips: none for an item at or above the
 *  shoulders (going down would take them further away). */
function pickCrouch(st: Staff, goals: readonly [THREE.Vector3, THREE.Vector3]): number {
  if (Math.min(goals[0].y, goals[1].y) > st.B.J.upper_arm_L!.y - 0.12 * st.B.s) return 0;
  return crouchFor(st, goals, crouchDeep, 2.6);
}

/** How high an item is for a pick or place: 0 a hand's span or more below the shoulders, 1 at or above them. */
function pickHigh(st: Staff, o: WorkOpts): number {
  const shY = st.B.J.upper_arm_L!.y;
  const y = o.at ? o.at.y : (o.surfaceY ?? 0.32) + 0.1;
  return smooth(shY - 0.25 * st.B.s, shY, y);
}

/** How much of a rise onto the toes an item at this height asks for (0 at or below the shoulders, 1 well above). */
function pickToes(st: Staff, o: WorkOpts): number {
  const B = st.B;
  const shelfY = o.at ? o.at.y : (o.surfaceY ?? 0.32) + 0.1;
  const sh = B.J.upper_arm_L!.y;
  return smooth(sh + 0.05 * B.s, sh + 0.3 * B.s, shelfY - 0.05);
}

const isPickPlace = (k: WorkKind): k is "pick" | "place" => k === "pick" || k === "place";

/** How far up on the toes a pick or place goes, t seconds in: for an item above the shoulders, while the hands are
 *  at the shelf (until the item is well on its way down). */
function pickHeelRaise(st: Staff, o: WorkOpts, kind: WorkKind, tt: number): number {
  const need = pickToes(st, o);
  if (need <= 0 || !isPickPlace(kind)) return 0;
  const ph = pickPhase(kind, tt, pickHigh(st, o));
  const atShelf = ph.reachW * (1 - smooth(0, 0.4, ph.go));
  return 22 * DEG * need * atShelf;
}

/** A carried item's grip point on one end wall (sd +1 the left), body space, for an item with its rim centre at `at`. */
function gripGoal(st: Staff, at: THREE.Vector3, sd: number): THREE.Vector3 {
  const K = st.carrySpec;
  return at.clone().add(V(sd * (K.length / 2 - K.flange - 0.003), -Math.min(0.05, K.depth * 0.5), -Math.min(0.055, K.width * 0.17)));
}

/**
 * How far (metres) a person's hands would fall short of an item's grip points for a "pick" or "place" with these
 * options, at the person's current placement: at the lowest crouch, or up on the toes for a high shelf, arms at
 * full reach. 0 when the item is in reach; otherwise stand that much closer (or choose another shelf). Place the
 * person (st.place) where the pick will start first. It poses the body to measure, then puts the pose back, so call
 * it while planning rather than every frame.
 */
export function pickShortfall(st: Staff, o: WorkOpts): number {
  const B = st.B;
  const s = B.s;
  const shelf = target(st, o, V(0, (o.surfaceY ?? 0.32) + 0.1, 0.43 * s));
  const goals = [gripGoal(st, shelf, 1), gripGoal(st, shelf, -1)] as const;
  const c = pickCrouch(st, goals);
  const reach = (B.upper + B.fore) * 0.93 + 0.36 * B.d.hand_length_m;
  const saved = copyPose(newPose(), st.current);
  const tmp = newPose();
  standPose(st, 0, tmp);
  tmp.pelvis.x *= 0.4;
  crouchDeep(st, tmp, c);
  heelRaise(st, tmp, 22 * DEG * pickToes(st, o));
  fitHips(st, tmp);
  st.pose(tmp);
  const short = Math.max(0, st.jointPos("upper_arm_L").distanceTo(goals[0]) - reach, st.jointPos("upper_arm_R").distanceTo(goals[1]) - reach);
  st.pose(saved);
  return short;
}

const crouchCache = new WeakMap<Staff, Map<string, number>>();

/**
 * The least crouch that brings both grip points (body space, left then right) within 95% of the arms' reach,
 * found by posing the body itself (so the spine's bend and the hips' fit are as they will be), then restoring it.
 * Cached per person and goal.
 */
export function crouchFor(st: Staff, goals: readonly [THREE.Vector3, THREE.Vector3], how: (st: Staff, p: BodyPose, c: number) => void = crouch, cMax = 1.3): number {
  const key = (how === crouch ? "" : "deep:") + goals.map((g) => g.toArray().map((x) => x.toFixed(3)).join(",")).join(";");
  let m = crouchCache.get(st);
  if (!m) crouchCache.set(st, (m = new Map()));
  const hit = m.get(key);
  if (hit !== undefined) return hit;
  const B = st.B;
  // The wrist within 93% of the arm's length, the palm point (0.36 of the hand's length on) beyond it.
  const reach = (B.upper + B.fore) * 0.93 + 0.36 * B.d.hand_length_m;
  const saved = copyPose(newPose(), st.current);
  const tmp = newPose();
  const ok = (c: number) => {
    standPose(st, 0, tmp);
    tmp.pelvis.x *= 0.4;
    how(st, tmp, c);
    fitHips(st, tmp);
    st.pose(tmp);
    return st.jointPos("upper_arm_L").distanceTo(goals[0]) <= reach && st.jointPos("upper_arm_R").distanceTo(goals[1]) <= reach;
  };
  let hi = 0;
  if (!ok(0)) {
    let lo = 0;
    hi = cMax;
    for (let k = 0; k < 16; k++) {
      const mid = (lo + hi) / 2;
      if (ok(mid)) hi = mid;
      else lo = mid;
    }
  }
  st.armDebug.crouch = [hi, st.jointPos("upper_arm_L").distanceTo(goals[0]), reach];
  st.pose(saved);
  m.set(key, hi);
  return hi;
}

/** Seconds a "pick" takes: from standing, down or out to the item, hands closed on it, and up into the carry. */
export const PICK_S = 3.85;
/** Seconds a "place" takes: from the carry down or out onto the spot, hands opened and off, and standing again. */
export const PLACE_S = 3.55;
let pickScratch: [BodyPose, BodyPose] | null = null;

/**
 * The body's pose at work, t seconds into the work, at the current placement: the core's kinds ("stand", "carry",
 * "pick", "place") or one of the person's own (`StaffOptions.works`). `clock` is the time on the clock the
 * neighbouring poses use (a Routine's time): "pick" starts from standing and ends in the carry at that time, and
 * "place" the reverse, so they meet a walk with the item, or standing, exactly. It defaults to t.
 */
export function workPose(st: Staff, kind: WorkKind, t: number, o: WorkOpts = {}, p: BodyPose = newPose(), clock?: number): BodyPose {
  if (isPickPlace(kind)) return pickPlacePose(st, kind, t, o, p, clock ?? t);
  standPose(st, t, p);
  if (kind === "carry") {
    const B = st.B;
    const tt = t * (o.tempo ?? 1);
    carryFrame(st, 0.004 * Math.sin(tt * 2.1), p.itemPos, p.itemQuat);
    p.itemOn = true;
    p.pelvis.x *= 0.4;
    panGrips(st, p, p.itemPos, p.itemQuat, o.halfWidth ?? st.carrySpec.gripHalf, 1, 1);
    p.flex -= 2 * DEG;
    p.gaze.set(0, B.eyeY - 0.3, 2.5);
    balanceLoad(st, p);
    return p;
  }
  st.works[kind]?.pose(st, t, o, p);
  return p;
}


/**
 * Take an item from a surface into the carry ("pick"), or put it from the carry onto a surface ("place"). The
 * item's rim centre is `o.at` (world) on `o.surfaceY`; any height from the ground to about 1.6 m. The body goes as low as the hands need (`crouchFor`), no lower; above the waist it stays
 * up and the arms reach. The item comes off the shelf the way it went on: lifted clear of the rails, slid out
 * towards the body (a short way at knee height, where the shins are), then carried up or down into the hold.
 */
function pickPlacePose(st: Staff, kind: "pick" | "place", tt: number, o: WorkOpts, p: BodyPose, clock: number): BodyPose {
  const B = st.B;
  const s = B.s;
  standPose(st, tt, p);
  p.pelvis.x *= 0.4;
  const shelf = target(st, o, V(0, (o.surfaceY ?? 0.32) + 0.1, 0.43 * s));
  const half = o.halfWidth ?? st.carrySpec.gripHalf;
  const carryP = V();
  const carryQ = new THREE.Quaternion();
  carryFrame(st, 0, carryP, carryQ);
  const cLow = pickCrouch(st, [gripGoal(st, shelf, 1), gripGoal(st, shelf, -1)]);
  const high = pickHigh(st, o);
  const { c, reachW, close, out, go } = pickPhase(kind, tt, high);
  // The item's way between the shelf and the hold: lifted clear of the rails and slid straight out (16 cm, clear of
  // a cabinet's shelf front; less for an item above the shoulders, which would come at the face), and only then up
  // or down with the body as it rises or sinks. Low down it comes out in front of the shins, under the knees.
  const slide =
    Math.min(Math.max(0, shelf.z - carryP.z), 0.16 * s) * (1 - 0.6 * smooth(B.J.upper_arm_L!.y - 0.1 * s, B.J.upper_arm_L!.y + 0.1 * s, shelf.y));
  const itemP = V(
    lerp(shelf.x, carryP.x, go),
    shelf.y + 0.012 * smooth(0, 0.25, out) + (carryP.y - shelf.y - 0.012) * go,
    lerp(lerp(shelf.z, shelf.z - slide, out), carryP.z, go),
  );
  const itemQ = new THREE.Quaternion().slerp(carryQ, go);
  p.itemOn = true;
  p.itemPos.copy(itemP);
  p.itemQuat.copy(itemQ);
  crouchDeep(st, p, c * cLow);
  heelRaise(st, p, pickHeelRaise(st, o, kind, tt));
  panGrips(st, p, itemP, itemQ, half, close, reachW);
  // Up to an item at or above the shoulders the hands travel up in front of the body to it (the arm's IK on from early
  // in the reach), rather than the arm swinging out sideways from its hang to the overhead solve.
  const shY = B.J.upper_arm_L!.y;
  if (high > 0 && reachW < 1) {
    for (let i = 0; i < 2; i++) {
      const a = p.arms[i]!;
      const sd = i === 0 ? 1 : -1;
      const from = V(sd * (Math.abs(B.J.upper_arm_L!.x) + 0.02 * s), shY - 0.8 * (B.upper + B.fore), 0.15 * s);
      const path = V().lerpVectors(from, a.pos, ease(reachW));
      a.pos.lerp(path, high);
    }
  }
  for (const a of p.arms) {
    a.flex = 0.06 + 0.9 * c * cLow;
    a.elbow = 0.25;
  }
  const tall = 1 - c * Math.min(1, cLow / 0.4);
  p.gaze.copy(itemP).lerp(V(0, B.eyeY - 0.45 * s, 1.8 * s), 0.5 * tall * (1 - go) * (itemP.y < B.eyeY - 0.3 ? 1 : 0)).lerp(V(0, B.eyeY, 3), go * 0.6);
  // Exactly standing at the open end, exactly the carry at the other (on the shared clock).
  pickScratch ??= [newPose(), newPose()];
  const [carry, stand] = pickScratch;
  const onShelf = out < 1e-6 && go < 1e-6;
  if (kind === "pick") {
    if (tt < 0.35) blendPose(p, standPose(st, clock, stand), p, smooth(0, 0.35, tt));
    const w = smooth(PICK_S - 0.5, PICK_S, tt);
    if (w > 0) blendPose(p, p, workPose(st, "carry", clock, o, carry), w);
  } else {
    const w = 1 - smooth(0, 0.5, tt);
    if (w > 0) blendPose(p, p, workPose(st, "carry", clock, o, carry), w);
    const ws = smooth(PLACE_S - 0.45, PLACE_S, tt);
    if (ws > 0) blendPose(p, p, standPose(st, clock, stand), ws);
  }
  // The item stays in the socket throughout: in the hands, or where it sits on the shelf.
  p.itemOn = true;
  if (onShelf) {
    p.itemPos.copy(itemP);
    p.itemQuat.copy(itemQ);
  }
  return p;
}

/* ------------------------------------------------------------------------------------------------------------ */
/* Walking: a footstep plan along a smoothed path, feet planted where they land, the body carried over them.     */
/* ------------------------------------------------------------------------------------------------------------ */

/** A floor path in the scene's xz plane, its corners rounded, measured by arc length. Yaw 0 faces +z. */
export class FloorPath {
  readonly xs: number[] = [];
  readonly zs: number[] = [];
  readonly ss: number[] = [];
  readonly length: number;
  constructor(points: readonly (readonly [number, number])[], fillet = 0.5) {
    const P = points.filter((p, i) => i === 0 || Math.hypot(p[0] - points[i - 1]![0], p[1] - points[i - 1]![1]) > 1e-4);
    const push = (x: number, z: number) => {
      const n = this.xs.length;
      if (n && Math.hypot(x - this.xs[n - 1]!, z - this.zs[n - 1]!) < 1e-5) return;
      this.xs.push(x);
      this.zs.push(z);
    };
    push(P[0]![0], P[0]![1]);
    for (let i = 1; i < P.length - 1; i++) {
      const [ax, az] = P[i - 1]!;
      const [bx, bz] = P[i]!;
      const [cx, cz] = P[i + 1]!;
      const l0 = Math.hypot(bx - ax, bz - az);
      const l1 = Math.hypot(cx - bx, cz - bz);
      const d0x = (bx - ax) / l0;
      const d0z = (bz - az) / l0;
      const d1x = (cx - bx) / l1;
      const d1z = (cz - bz) / l1;
      const turn = Math.acos(clamp(d0x * d1x + d0z * d1z, -1, 1));
      if (turn < 0.02) {
        push(bx, bz);
        continue;
      }
      const tl = Math.min(fillet * Math.tan(turn / 2), 0.45 * Math.min(l0, l1));
      const r = tl / Math.tan(turn / 2);
      const sx = bx - d0x * tl;
      const sz = bz - d0z * tl;
      // Centre of the arc, on the inside of the turn.
      const cross = d0x * d1z - d0z * d1x;
      const nx = cross > 0 ? -d0z : d0z;
      const nz = cross > 0 ? d0x : -d0x;
      const ox = sx + nx * r;
      const oz = sz + nz * r;
      const a0 = Math.atan2(sx - ox, sz - oz);
      const sgnT = cross > 0 ? -1 : 1;
      const n = Math.max(2, Math.ceil(turn / (5 * DEG)));
      for (let k = 0; k <= n; k++) {
        const a = a0 + (sgnT * turn * k) / n;
        push(ox + Math.sin(a) * r, oz + Math.cos(a) * r);
      }
    }
    push(P[P.length - 1]![0], P[P.length - 1]![1]);
    let s = 0;
    this.ss.push(0);
    for (let i = 1; i < this.xs.length; i++) {
      s += Math.hypot(this.xs[i]! - this.xs[i - 1]!, this.zs[i]! - this.zs[i - 1]!);
      this.ss.push(s);
    }
    this.length = s;
  }
  private seg(s: number): number {
    const ss = this.ss;
    let lo = 0;
    let hi = ss.length - 1;
    while (hi - lo > 1) {
      const m = (lo + hi) >> 1;
      if (ss[m]! <= s) lo = m;
      else hi = m;
    }
    return lo;
  }
  private dirYaw(i: number): number {
    const j = clamp(i, 0, this.xs.length - 2);
    return Math.atan2(this.xs[j + 1]! - this.xs[j]!, this.zs[j + 1]! - this.zs[j]!);
  }
  /** Position and heading at arc length s. */
  at(s: number): { x: number; z: number; yaw: number } {
    if (this.xs.length < 2) return { x: this.xs[0] ?? 0, z: this.zs[0] ?? 0, yaw: 0 };
    const c = clamp(s, 0, this.length);
    const i = this.seg(c);
    const u = (c - this.ss[i]!) / Math.max(1e-9, this.ss[i + 1]! - this.ss[i]!);
    // Heading blends between neighbouring segments' directions so it turns smoothly along arcs.
    const y0 = this.dirYaw(i);
    const yPrev = this.dirYaw(i - 1);
    const yNext = this.dirYaw(i + 1);
    const ya = y0 + wrapPi(yPrev - y0) * 0.5 * (1 - u) * (i > 0 ? 1 : 0);
    const yb = y0 + wrapPi(yNext - y0) * 0.5 * u * (i < this.xs.length - 2 ? 1 : 0);
    return { x: lerp(this.xs[i]!, this.xs[i + 1]!, u), z: lerp(this.zs[i]!, this.zs[i + 1]!, u), yaw: ya + (yb - y0) };
  }
}

interface Plant {
  x: number;
  z: number;
  yaw: number;
  tOn: number;
  tOff: number;
  arrive: "walk" | "step" | "init";
  leave: "walk" | "step" | "end";
  hsAng: number;
  hsDur: number;
  toAng: number;
  toDur: number;
  /** Seconds over which the foot takes up the body's weight after landing, and gives it up before lifting. */
  rUp: number;
  rDn: number;
}

/**
 * What the upper body does while walking or turning: swinging free, or keeping up a work that allows it (carrying
 * an item, or one of the person's own works marked `walk`: a tablet held up, a radio at the ear).
 */
export type Upper = "free" | WorkKind;

export type RoutineStep =
  | { kind: "walk"; path: readonly (readonly [number, number])[]; speed?: number; upper?: Upper; opts?: WorkOpts }
  | { kind: "turn"; yaw: number; upper?: Upper; opts?: WorkOpts }
  /** `phase`: seconds already into the work as the step starts; `held`: the body is already in the work's pose
   *  then (no easing in from standing), as when a routine is re-planned in the middle of a work. */
  | { kind: "work"; work: WorkKind; duration: number; opts?: WorkOpts; phase?: number; held?: boolean }
  | { kind: "stand"; duration: number }
  /** A few centimetres in any direction (back from a counter, sideways along it) without turning: short steps,
   *  two for up to about a third of a stride, more for further, the body carried with the feet. */
  | { kind: "shuffle"; to: readonly [number, number]; upper?: Upper; opts?: WorkOpts };

interface Seg {
  t0: number;
  t1: number;
  type: "walk" | "turn" | "work" | "stand";
  x: number;
  z: number;
  yaw0: number;
  yaw1: number;
  path?: FloorPath;
  prog?: Mono;
  speed?: number;
  stepLen?: number;
  upper: Upper;
  work?: WorkKind;
  opts?: WorkOpts;
  phase?: number;
  held?: boolean;
}

export interface RoutineState {
  activity: "walk" | "turn" | "work" | "stand";
  work?: WorkKind;
  /** Seconds into the current activity. */
  local: number;
  /** Whether the item socket is in use (an item carried, picked up or put down, a tablet held). */
  holding: boolean;
  /** When the staff's `obstacles` say the body is inside something this pose: the push (world) that clears it. */
  blocked?: THREE.Vector3 | null;
}

const BIG = 1e9;

/** Whether the upper body can keep a work up while walking. */
export function walkable(st: Staff, k: WorkKind): boolean {
  return k === "carry" || st.works[k]?.walk === true;
}

/** What a segment's hands hold as it starts, and as it ends (a pick ends in the carry; a place starts in it). */
function upAtStart(g: Seg): Upper {
  return g.type === "work" && g.work === "place" ? "carry" : g.upper;
}
function upAtEnd(g: Seg): Upper {
  return g.type === "work" && g.work === "pick" ? "carry" : g.upper;
}
/** Whether a segment ends (starts) in plain standing: its upper body is the standing pose there. */
function standsAtEnd(g: Seg): boolean {
  return upAtEnd(g) === "free" && (g.type !== "work" || g.work === "place");
}
function standsAtStart(g: Seg): boolean {
  return upAtStart(g) === "free" && (g.type !== "work" || g.work === "pick");
}

/**
 * A person's timeline: walks, turns in place and work at stations, planned once into footsteps so that every pose
 * is a pure function of time. `apply(t)` places and poses the person; with `loop` the timeline repeats seamlessly
 * (the routine must end where it started, as `routeRoutine` arranges).
 */
export class Routine {
  readonly duration: number;
  private plants: Record<Side, Plant[]> = { L: [], R: [] };
  private segs: Seg[] = [];
  private cx: number;
  private cz: number;
  private cyaw: number;
  private t = 0;
  private lastStep: Side = "L";
  private pose = newPose();
  private aux = newPose();

  /**
   * `clock0` is the time on the person's own clock at the routine's start: idle motion and works kept up across
   * walks run on it, so a re-planned routine carries on from the last without a jump.
   */
  constructor(readonly staff: Staff, start: { x: number; z: number; yaw: number }, steps: readonly RoutineStep[], readonly loop = true, readonly clock0 = 0) {
    this.cx = start.x;
    this.cz = start.z;
    this.cyaw = start.yaw;
    for (const side of SIDES) {
      const [x, z] = this.stanceAt(this.cx, this.cz, this.cyaw, side);
      this.plants[side].push(this.plant(x, z, this.cyaw + sgn(side) * TOE_OUT, -BIG, "init"));
    }
    for (const st of steps) {
      if (st.kind === "turn") this.planTurn(st.yaw, st.upper ?? "free", st.opts);
      else if (st.kind === "walk") this.planWalk(st.path, st.speed ?? 1.0, st.upper ?? "free", st.opts);
      else if (st.kind === "shuffle") this.planShuffle(st.to, st.upper ?? "free", st.opts);
      else {
        const dur = st.duration;
        if (dur <= 0) continue;
        const work = st.kind === "work" ? st.work : undefined;
        const upper: Upper = work !== undefined && walkable(staff, work) ? work : "free";
        const w = st.kind === "work" ? st : undefined;
        this.segs.push({ t0: this.t, t1: this.t + dur, type: w ? "work" : "stand", x: this.cx, z: this.cz, yaw0: this.cyaw, yaw1: this.cyaw, upper, work, opts: w?.opts, phase: w?.phase, held: w?.held });
        this.t += dur;
      }
    }
    if (!this.segs.length) this.segs.push({ t0: 0, t1: 1, type: "stand", x: this.cx, z: this.cz, yaw0: this.cyaw, yaw1: this.cyaw, upper: "free" });
    this.duration = Math.max(this.t, 1e-3);
    // Weight transfer: a foot takes the load while the other is still down (double support) and hands it over
    // before it lifts, so the ramps are the overlaps of the two feet's plants.
    for (const side of SIDES) {
      const other = this.plants[side === "L" ? "R" : "L"];
      for (const p of this.plants[side]) {
        if (p.tOn > -BIG / 2) {
          const q = other.find((o) => o.tOn <= p.tOn && p.tOn <= o.tOff);
          p.rUp = q && q.tOff < BIG / 2 ? clamp(q.tOff - p.tOn, 0.06, 0.3) : 0.3;
        }
        if (p.tOff < BIG / 2) {
          let q: Plant | undefined;
          for (const o of other) if (o.tOn <= p.tOff) q = o;
          p.rDn = q && q.tOn > -BIG / 2 ? clamp(p.tOff - q.tOn, 0.06, 0.3) : 0.24;
        }
      }
    }
  }

  private stanceAt(x: number, z: number, yaw: number, side: Side, half = this.staff.B.standHalf): [number, number] {
    const sd = sgn(side);
    return [x + Math.cos(yaw) * sd * half, z - Math.sin(yaw) * sd * half];
  }
  private plant(x: number, z: number, yaw: number, tOn: number, arrive: Plant["arrive"]): Plant {
    return { x, z, yaw, tOn, tOff: BIG, arrive, leave: "end", hsAng: 0, hsDur: 0, toAng: 0, toDur: 0, rUp: 0.3, rDn: 0.3 };
  }
  private last(side: Side): Plant {
    const a = this.plants[side];
    return a[a.length - 1]!;
  }
  /** Lift a foot at tLift and set it down at tLand on a new plant. */
  private step(side: Side, tLift: number, leave: Plant["leave"], np: Plant): void {
    const lp = this.last(side);
    lp.tOff = tLift;
    lp.leave = leave;
    this.plants[side].push(np);
    this.lastStep = side;
  }

  private planTurn(yaw: number, upper: Upper = "free", opts?: WorkOpts): void {
    const d = wrapPi(yaw - this.cyaw);
    if (Math.abs(d) < 0.01) {
      this.cyaw = yaw;
      return;
    }
    const m = Math.max(1, Math.ceil(Math.abs(d) / (62 * DEG)));
    const lead: Side = d > 0 ? "L" : "R";
    const trail: Side = lead === "L" ? "R" : "L";
    const t0 = this.t;
    const sw = 0.34;
    const dsT = 0.14;
    const pre = 0.2;
    for (let j = 1; j <= m; j++) {
      const yj = this.cyaw + (d * j) / m;
      for (const [k, side] of [[0, lead], [1, trail]] as [number, Side][]) {
        const lift = t0 + pre + (j - 1) * 2 * (sw + dsT) + k * (sw + dsT);
        const [x, z] = this.stanceAt(this.cx, this.cz, yj, side);
        this.step(side, lift, "step", this.plant(x, z, yj + sgn(side) * TOE_OUT, lift + sw, "step"));
      }
    }
    const t1 = t0 + pre + m * 2 * (sw + dsT) + 0.2;
    this.segs.push({ t0, t1, type: "turn", x: this.cx, z: this.cz, yaw0: this.cyaw, yaw1: this.cyaw + d, upper, opts });
    this.t = t1;
    this.cyaw = this.cyaw + d;
  }

  /** Step to (x, z) keeping the heading: pairs of short steps, the foot on the side moved towards leading. */
  private planShuffle(to: readonly [number, number], upper: Upper, opts?: WorkOpts): void {
    const s = this.staff.B.s;
    const dx = to[0] - this.cx;
    const dz = to[1] - this.cz;
    const D = Math.hypot(dx, dz);
    if (D < 0.004) return;
    // A third of a stride (two steps at 1 m/s) a pair.
    const pairs = Math.max(1, Math.ceil(D / (0.36 * s)));
    const lat = dx * Math.cos(this.cyaw) - dz * Math.sin(this.cyaw);
    const lead: Side = Math.abs(lat) > 0.35 * D ? (lat > 0 ? "L" : "R") : this.lastStep === "L" ? "R" : "L";
    const other: Side = lead === "L" ? "R" : "L";
    const t0 = this.t;
    let k = 0;
    for (let j = 1; j <= pairs; j++) {
      const cx = this.cx + (dx * j) / pairs;
      const cz = this.cz + (dz * j) / pairs;
      for (const side of [lead, other]) {
        const [x, z] = this.stanceAt(cx, cz, this.cyaw, side);
        const lift = t0 + 0.2 + k * 0.5;
        this.step(side, lift, "step", this.plant(x, z, this.cyaw + sgn(side) * TOE_OUT, lift + 0.36, "step"));
        k++;
      }
    }
    const t1 = t0 + 0.3 + k * 0.5;
    // The root moves between the stances over the shuffle, carried by the feet; the heading stays.
    const path = new FloorPath([[this.cx, this.cz], [to[0], to[1]]], 0.55 * s);
    this.segs.push({ t0, t1, type: "turn", x: this.cx, z: this.cz, yaw0: this.cyaw, yaw1: this.cyaw, path, upper, opts });
    this.t = t1;
    this.cx = to[0];
    this.cz = to[1];
  }

  private planWalk(pts: readonly (readonly [number, number])[], v: number, upper: Upper, opts?: WorkOpts): void {
    const B = this.staff.B;
    const s = B.s;
    const path = new FloorPath([[this.cx, this.cz], ...pts], 0.55 * s);
    const D = path.length;
    if (D < 0.04) return;
    const h0 = path.at(0).yaw;
    if (Math.abs(wrapPi(h0 - this.cyaw)) > 0.03) this.planTurn(h0, upper, opts);
    const end = path.at(D);
    if (D < 0.32 * s) {
      // A shuffle: two short steps to the new spot.
      const t0 = this.t;
      const lead: Side = this.lastStep === "L" ? "R" : "L";
      for (const [k, side] of [[0, lead], [1, lead === "L" ? "R" : "L"]] as [number, Side][]) {
        const [x, z] = this.stanceAt(end.x, end.z, this.cyaw, side);
        const lift = t0 + 0.2 + k * 0.5;
        this.step(side, lift, "step", this.plant(x, z, this.cyaw + sgn(side) * TOE_OUT, lift + 0.36, "step"));
      }
      // The root moves between the stances over the shuffle, carried by the feet.
      this.segs.push({ t0, t1: t0 + 1.3, type: "turn", x: this.cx, z: this.cz, yaw0: this.cyaw, yaw1: this.cyaw, path, upper, opts });
      this.t = t0 + 1.3;
      this.cx = end.x;
      this.cz = end.z;
      return;
    }
    // Step length from the walk ratio (step length over cadence, about 0.0066 m per step/min for adults).
    // Carrying a load people take shorter, flatter steps with longer double support.
    const loaded = upper === "carry";
    const L0 = clamp(Math.sqrt(v * 60 * 0.006 * s), 0.3 * s, 0.78 * s) * (loaded ? 0.84 : 1);
    const n = Math.max(2, Math.round(D / L0 + 0.5));
    const L = D / (n - 0.5);
    const T = L / v;
    const ds = (loaded ? 0.27 : 0.2) * T;
    const sw = T - ds;
    const vk = clamp(v / 1.3, 0.2, 1.2);
    const hsAng = (7 + 7 * vk) * DEG * (loaded ? 0.75 : 1);
    // The foot rolls over the ball until it stands steep at toe-off (about 50 degrees at a brisk walk).
    const toAng = (32 + 20 * vk) * DEG * (loaded ? 0.8 : 1);
    const lead: Side = this.lastStep === "L" ? "R" : "L";
    const other: Side = lead === "L" ? "R" : "L";
    const t0 = this.t;
    const ws = 0.24;
    const side = (k: number): Side => (k % 2 === 1 ? lead : other);
    const hs: number[] = [0];
    const rock = (p: Plant, leaving: boolean, amp = 1) => {
      if (leaving) {
        p.toAng = toAng * amp;
        p.toDur = 0.55 * T;
      }
    };
    // The lead foot leaves its standing plant after the weight shifts onto the other.
    const first = this.last(lead);
    first.leave = "walk";
    first.tOff = t0 + ws;
    rock(first, true, 0.55);
    for (let k = 1; k <= n; k++) {
      const hk = k === 1 ? t0 + ws + sw * 0.9 : hs[k - 1]! + T;
      hs.push(hk);
      const a = (k - 0.5) * L;
      const at = path.at(a);
      const sd = sgn(side(k));
      const half = k === n ? B.standHalf : B.walkHalf;
      const x = at.x + Math.cos(at.yaw) * sd * half;
      const z = at.z - Math.sin(at.yaw) * sd * half;
      const np = this.plant(x, z, at.yaw + sd * TOE_OUT * (k === n ? 1 : 0.5), hk, "walk");
      np.hsAng = hsAng * (k === 1 ? 0.7 : 1);
      np.hsDur = 0.2 * T;
      // The foot this one replaces lifted ds after the previous heel strike.
      const prev = this.last(side(k));
      if (k >= 2) {
        prev.tOff = hs[k - 1]! + ds;
        prev.leave = "walk";
        rock(prev, true, k === 2 ? 0.8 : 1);
      }
      this.plants[side(k)].push(np);
    }
    // The closing step brings the trailing foot alongside.
    const closer = side(n - 1);
    const hn = hs[n]!;
    const tl = hn + ds;
    const hc = tl + sw * 0.85;
    const [cx2, cz2] = this.stanceAt(end.x, end.z, end.yaw, closer);
    const cp = this.last(closer);
    cp.tOff = tl;
    cp.leave = "walk";
    rock(cp, true, 0.7);
    const np = this.plant(cx2, cz2, end.yaw + sgn(closer) * TOE_OUT, hc, "walk");
    np.hsAng = hsAng * 0.4;
    np.hsDur = 0.16 * T;
    this.plants[closer].push(np);
    this.lastStep = closer;
    // Body progress along the path: midway between the feet at each heel strike, at rest at both ends.
    const tk = [t0, t0 + ws * 0.8];
    const sk = [0, 0.004 * s];
    for (let k = 1; k <= n; k++) {
      tk.push(hs[k]!);
      // At each heel strike the body is a little nearer the trailing foot than the leading one.
      sk.push(k === 1 ? 0.22 * L : (k - 1.05) * L);
    }
    tk.push(hc, hc + 0.32);
    sk.push(D - 0.05 * L, D);
    const prog = new Mono(tk, sk).ends(0, 0);
    const t1 = hc + 0.32;
    this.segs.push({ t0, t1, type: "walk", x: this.cx, z: this.cz, yaw0: this.cyaw, yaw1: end.yaw, path, prog, speed: v, stepLen: L, upper, opts });
    this.t = t1;
    this.cx = end.x;
    this.cz = end.z;
    this.cyaw = end.yaw;
  }

  /* -------------------------------------------------------------------------------------------------------- */

  private segIndex(t: number): number {
    for (let i = 0; i < this.segs.length; i++) if (t < this.segs[i]!.t1) return i;
    return this.segs.length - 1;
  }

  private foot(side: Side, t: number): { x: number; y: number; z: number; yaw: number; pitch: number; toe: number; swing: number; base: number; clear: number } {
    const arr = this.plants[side];
    let lo = 0;
    let hi = arr.length - 1;
    while (lo < hi) {
      const m = (lo + hi + 1) >> 1;
      if (arr[m]!.tOn <= t) lo = m;
      else hi = m - 1;
    }
    const pl = arr[lo]!;
    if (t <= pl.tOff || lo === arr.length - 1) {
      const f = this.stance(pl, Math.min(t, pl.tOff));
      return { ...f, swing: -1, base: f.y, clear: 0 };
    }
    const nx = arr[lo + 1]!;
    const a = this.stance(pl, pl.tOff);
    const b = this.stance(nx, nx.tOn);
    const u = clamp((t - pl.tOff) / Math.max(1e-6, nx.tOn - pl.tOff), 0, 1);
    const B = this.staff.B;
    const s = B.s;
    const walking = nx.arrive === "walk";
    // The swing foot leaves with some speed (the toe slides off the floor) and settles to land: a Hermite ramp
    // with a starting slope, rather than minimum jerk, which would leave it trailing behind the hip.
    const v0 = walking ? 0.45 : 0;
    const m = walking ? v0 * u + (3 - 2 * v0) * u * u + (v0 - 2) * u * u * u : mj(u);
    const yaw = a.yaw + wrapPi(b.yaw - a.yaw) * mj(clamp(u * 1.15, 0, 1));
    const pitch = walking ? a.pitch * (1 - smooth(0, 0.45, u)) + b.pitch * smooth(0.45, 1, u) : 5 * DEG * Math.sin(Math.PI * u);
    const toe = a.toe * (1 - smooth(0, 0.35, u));
    // The knee folds early in swing and lifts the ankle; it comes down to the floor as the shank swings through.
    const fold = walking ? (0.055 * s * Math.sin(Math.PI * u) * Math.exp(-3 * u)) / 0.334 : 0;
    const base = lerp(a.y, b.y, m) + (walking ? 0.012 : 0.032) * s * Math.pow(Math.sin(Math.PI * u), 0.8) + fold;
    // Keep heel, ball and toe tip clear of the floor in mid swing.
    const clear = 0.012 * s * Math.sin(Math.PI * u);
    const y = this.clearY(base, pitch, toe, clear);
    return { x: lerp(a.x, b.x, m), y, z: lerp(a.z, b.z, m), yaw, pitch, toe, swing: u, base, clear };
  }

  /** The lowest the ankle may be for the heel, ball and toe tip all to clear the floor by `clear`. */
  private clearY(y: number, pitch: number, toe: number, clear: number): number {
    const B = this.staff.B;
    const s = B.s;
    const heel = -B.ankY * Math.cos(pitch) - (B.heelBack + 0.013 * s) * Math.sin(pitch);
    const mtpY = B.J.toe_R!.y - B.ankY;
    const ball = mtpY * Math.cos(pitch) + B.mtpFwd * Math.sin(pitch);
    const sole = -B.ankY * Math.cos(pitch) + B.mtpFwd * Math.sin(pitch);
    const tip = ball + (B.toeFwd + 0.015 * s - B.mtpFwd) * Math.sin(pitch + toe) - (B.J.toe_R!.y - 0.009 * s) * Math.cos(pitch + toe);
    return Math.max(y, clear - Math.min(heel, sole, tip));
  }

  /** The shin's forward lean for a leg reaching from its hip to a body-space ankle (radians, forward positive). */
  private shinLean(p: BodyPose, i: number, fwd: THREE.Vector3): number {
    const B = this.staff.B;
    const side = SIDES[i]!;
    _q2.setFromAxisAngle(Y, p.yaw).multiply(qAxis(X, p.tilt, _q3)).multiply(qAxis(Z, p.roll, _q1));
    const H = _v2.subVectors(B.J[`thigh_${side}`]!, B.J.pelvis!).applyQuaternion(_q2).add(p.pelvis);
    const A = p.feet[i]!.pos;
    const dirV = _v3.subVectors(A, H);
    const d = clamp(dirV.length(), Math.abs(B.thigh - B.shin) + 0.05, (B.thigh + B.shin) * 0.99995);
    const dir = dirV.normalize();
    const ca = clamp((B.thigh * B.thigh + d * d - B.shin * B.shin) / (2 * B.thigh * d), -1, 1);
    const pole = orthoTo(fwd.clone().addScaledVector(X, sgn(side) * 0.12), dir);
    const K = H.clone().addScaledVector(dir, B.thigh * ca).addScaledVector(pole, B.thigh * Math.sqrt(1 - ca * ca));
    const sv = K.sub(H.clone().addScaledVector(dir, d));
    return Math.atan2(sv.dot(fwd), sv.y);
  }

  /** The foot on a plant at time t: heel rocker after a heel strike, rising on the ball before toe-off. */
  private stance(pl: Plant, t: number): { x: number; y: number; z: number; yaw: number; pitch: number; toe: number } {
    const B = this.staff.B;
    const s = B.s;
    let pitch = 0;
    let toe = 0;
    let pv: [number, number] | null = null;
    if (pl.arrive === "walk" && pl.hsDur > 0 && t < pl.tOn + pl.hsDur) {
      const u = clamp((t - pl.tOn) / pl.hsDur, 0, 1);
      pitch = pl.hsAng * (1 - u) * (1 - u * 0.4);
      pv = [-(B.heelBack + 0.013 * s), -B.ankY];
    }
    if (pl.leave === "walk" && pl.toDur > 0 && t > pl.tOff - pl.toDur) {
      const u = clamp((t - (pl.tOff - pl.toDur)) / pl.toDur, 0, 1);
      pitch = -pl.toAng * Math.pow(u, 1.25);
      toe = -pitch;
      // About the floor under the ball, where the sole stays in contact (the toes are held planted by the leg).
      pv = [B.mtpFwd, -B.ankY];
    }
    let x = pl.x;
    let y = B.ankY;
    let z = pl.z;
    if (pv) {
      // Rotate the ankle about the pivot (heel edge or ball joint) in the foot's own vertical plane.
      const [pz, py] = pv;
      const c = Math.cos(-pitch);
      const sn = Math.sin(-pitch);
      // Foot frame: rotation about x by -pitch takes (y, z) to (y c - z s, y s + z c).
      const ry = py * c - pz * sn;
      const rz = py * sn + pz * c;
      const dzf = pz - rz;
      y = B.ankY + (py - ry);
      x += Math.sin(pl.yaw) * dzf;
      z += Math.cos(pl.yaw) * dzf;
    }
    return { x, y, z, yaw: pl.yaw, pitch, toe };
  }

  /** The share of the body's weight on the left foot at t (unsmoothed). */
  private loadL(t: number): number {
    const load = (side: Side) => {
      const arr = this.plants[side];
      let lo = 0;
      let hi = arr.length - 1;
      while (lo < hi) {
        const m = (lo + hi + 1) >> 1;
        if (arr[m]!.tOn <= t) lo = m;
        else hi = m - 1;
      }
      const p = arr[lo]!;
      if (t > p.tOff || t < p.tOn) return 0;
      return smooth(p.tOn, p.tOn + p.rUp, t) * (1 - smooth(p.tOff - p.rDn, p.tOff, t));
    };
    const l = load("L");
    const r = load("R");
    return l + r > 1e-6 ? l / (l + r) : 0.5;
  }
  /** The left foot's share, smoothed over a short window (the body's mass cannot jump sideways). */
  private support(t: number): number {
    let w = 0;
    for (let k = -3; k <= 3; k++) w += ((4 - Math.abs(k)) / 16) * this.loadL(t + k * 0.045);
    return w;
  }

  /**
   * The upper-body pose a segment holds at time t (standing, working, or keeping a work up while walking). Idle
   * motion and works that can walk run on the person's clock; other works on their own, from their start.
   */
  private upperPose(g: Seg, tm: number, p: BodyPose): BodyPose {
    const st = this.staff;
    const clock = this.clock0 + tm;
    if (g.type === "work") return workPose(st, g.work!, g.upper !== "free" ? clock : tm - g.t0 + (g.phase ?? 0), g.opts ?? {}, p, clock);
    if (g.upper !== "free") return workPose(st, g.upper, clock, g.opts ?? {}, p);
    return standPose(st, clock, p);
  }

  /** Place and pose the person at time t; returns what they are doing. */
  apply(t: number): RoutineState {
    const r = this.poseAt(t);
    this.staff.pose(r.pose);
    return { ...r.state, blocked: this.staff.obstacles ? this.staff.collide() : undefined };
  }

  /**
   * Place the person at time t and work out their pose without applying it (`apply` applies it), so that a caller
   * may blend it with another first. The pose is this routine's scratch: copy it to keep it.
   */
  poseAt(t: number): { state: RoutineState; pose: BodyPose } {
    const st = this.staff;
    const B = st.B;
    const s = B.s;
    const D = this.duration;
    const tm = this.loop ? ((t % D) + D) % D : clamp(t, 0, D);
    const clock = this.clock0 + tm;
    const gi = this.segIndex(tm);
    const g = this.segs[gi]!;
    const n = this.segs.length;
    const prev = this.segs[gi > 0 ? gi - 1 : this.loop ? n - 1 : 0]!;
    const next = this.segs[gi < n - 1 ? gi + 1 : this.loop ? 0 : n - 1]!;
    const local = tm - g.t0;
    // Root placement.
    let rx = g.x;
    let rz = g.z;
    let ryaw = g.yaw0;
    let along = 0;
    if (g.type === "walk") {
      along = g.prog!.at(tm);
      const a = g.path!.at(along);
      rx = a.x;
      rz = a.z;
      ryaw = a.yaw;
    } else if (g.type === "turn") {
      const u = ease(clamp((tm - g.t0 - 0.1) / Math.max(0.1, g.t1 - g.t0 - 0.3), 0, 1));
      ryaw = g.yaw0 + wrapPi(g.yaw1 - g.yaw0) * u;
      if (g.path) {
        const a = g.path.at(g.path.length * u);
        rx = a.x;
        rz = a.z;
      }
    }
    st.place(rx, rz, ryaw);
    // Upper body: the segment's own, eased from and to standing where the neighbours hold something else.
    const p = this.upperPose(g, tm, this.pose);
    const ramp = g.type === "work" ? 0.9 : 0.6;
    const span = g.t1 - g.t0;
    // What the hands hold as a segment starts and ends: a pick ends in the carry, a place starts in it. Where
    // neighbours meet holding the same thing there is no ramp; nor where a pick starts, or a place ends, standing
    // (they do that themselves, on the routine's clock).
    const uS = upAtStart(g);
    const uE = upAtEnd(g);
    const rIn = g.held || (uS !== "free" && upAtEnd(prev) === uS) || (g.work === "pick" && g.type === "work" && standsAtEnd(prev)) ? 1 : smooth(g.t0, g.t0 + Math.min(ramp, span / 2), tm);
    const rOut = (uE !== "free" && upAtStart(next) === uE) || (g.work === "place" && g.type === "work" && standsAtStart(next)) ? 1 : 1 - smooth(g.t1 - Math.min(ramp, span / 2), g.t1, tm);
    const wU = g.type === "work" || g.upper !== "free" ? rIn * rOut : 1;
    if (wU < 1) blendPose(p, standPose(st, clock, this.aux), p, wU);
    // Feet from the plan (unless the work places them itself).
    const ownFeet = g.type === "work" && g.work !== undefined && st.works[g.work]?.feet === true;
    const reachW: [number, number] = [1, 1];
    const fs: ReturnType<Routine["foot"]>[] = [];
    for (let i = 0; i < 2; i++) {
      const f = this.foot(SIDES[i]!, tm);
      fs.push(f);
      // A foot in mid swing does not hold the hips down; one about to land or just lifting does.
      reachW[i] = f.swing < 0 ? 1 : (1 - smooth(0, 0.3, f.swing)) + smooth(0.65, 1, f.swing);
      if (ownFeet) continue;
      const fo = p.feet[i]!;
      st.toBody(_v1.set(f.x, f.y, f.z), fo.pos);
      fo.quat.setFromAxisAngle(Y, f.yaw - ryaw).multiply(qAxis(X, -f.pitch, _q1));
      fo.toe = f.toe;
    }
    // A pick or place up at a high shelf goes up on the toes (the hips are already up in the upper pose).
    if (g.type === "work" && (g.work === "pick" || g.work === "place")) heelRaise(st, p, pickHeelRaise(st, g.opts ?? {}, g.work, local), false);
    // The gait on top: eased in and out at the segment's ends so it meets standing without a jump.
    const moving = g.type === "walk" || g.type === "turn";
    const restY = p.pelvis.y;
    const restZ = B.J.pelvis!.z;
    if (moving) {
      const m = smooth(g.t0, g.t0 + 0.45, tm) * (1 - smooth(g.t1 - 0.45, g.t1, tm));
      const mx = smooth(g.t0, g.t0 + 0.15, tm) * (1 - smooth(g.t1 - 0.15, g.t1, tm));
      const wl = this.support(tm);
      const fl = p.feet[0]!.pos;
      const fr = p.feet[1]!.pos;
      const L = g.stepLen ?? 0.5 * s;
      const fd = clamp((fl.z - fr.z) / L, -1.3, 1.3);
      const vk = clamp((g.speed ?? 0.6) / 1.3, 0.2, 1.2);
      const walk = g.type === "walk" ? 1 : 0.5;
      // Over the loaded foot, a third of the way out.
      p.pelvis.x = lerp(p.pelvis.x, 0.32 * (wl * fl.x + (1 - wl) * fr.x), mx);
      // With a load the hips stay back where its balance put them (see balanceLoad).
      const loadBack = g.upper === "carry" ? p.pelvis.z - restZ : 0;
      p.pelvis.z = lerp(p.pelvis.z, 0.012 * s + loadBack, m);
      p.pelvis.y = lerp(p.pelvis.y, B.hipY - 0.004 * s, m);
      // The pelvis turns ahead of the feet into a turn or a bend in the path (the head leads it further, by gaze).
      let lead = 0;
      if (g.type === "turn") lead = wrapPi(g.yaw1 - ryaw);
      else if (g.path) {
        const a0 = g.path.at(along);
        const a1 = g.path.at(along + 0.7 * s);
        if (Math.hypot(a1.x - a0.x, a1.z - a0.z) > 1e-3) lead = wrapPi(Math.atan2(a1.x - a0.x, a1.z - a0.z) - ryaw);
      }
      p.yaw = lerp(p.yaw, -0.07 * fd + clamp(0.32 * lead, -0.24, 0.24), m);
      p.roll = lerp(p.roll, 0.06 * (2 * wl - 1) * walk, mx);
      p.tilt = lerp(p.tilt, 0.03 + 0.01 * Math.abs(fd), m);
      if (g.upper === "free") {
        p.flex = lerp(p.flex, 0.035 + 0.035 * vk * walk, m);
        p.twist = lerp(p.twist, 0.11 * fd, m);
        p.side = lerp(p.side, -0.35 * p.roll, m);
        // Arms swing against the legs, more the faster the walk; the forward arm's elbow bends.
        const A = (0.16 + 0.22 * vk) * walk;
        for (let i = 0; i < 2; i++) {
          const a = p.arms[i]!;
          const sw = (i === 0 ? -1 : 1) * A * fd + 0.04;
          a.flex = lerp(a.flex, sw, m);
          a.elbow = lerp(a.elbow, 0.24 + (0.32 * Math.max(0, sw)) / (A + 0.04), m);
          a.abd = lerp(a.abd, 0.07, m);
        }
        // Eyes on the way ahead, or round the turn.
        if (g.type === "walk") {
          const ahead = g.path!.at(along + 2.6);
          st.toBody(_v2.set(ahead.x, B.eyeY - 0.55, ahead.z), _v3);
        } else {
          // The turn still to go (unwrapped: a half turn must not flip the eyes to the other side), led a little.
          const dir = clamp((g.yaw1 - ryaw) * 1.4, -1.4, 1.4);
          _v3.set(Math.sin(dir) * 3, B.eyeY - 0.25, Math.cos(dir) * 3);
        }
        p.gaze.lerp(_v3, m);
      } else {
        p.twist = lerp(p.twist, p.twist + 0.04 * fd, m);
        // A hand the work leaves free (one holding a radio up, say) still swings.
        const A = (0.12 + 0.16 * vk) * walk;
        for (let i = 0; i < 2; i++) {
          const a = p.arms[i]!;
          if (a.ik > 0) continue;
          const sw = (i === 0 ? -1 : 1) * A * fd + 0.04;
          a.flex = lerp(a.flex, sw, m);
          a.elbow = lerp(a.elbow, 0.24 + (0.32 * Math.max(0, sw)) / (A + 0.04), m);
        }
      }
    }
    // Hips as high as wanted and no higher than both legs reach; a carried pan rides down with the shoulders.
    fitHips(st, p, 0.012, reachW);
    // In swing the foot hangs from the shin: pointed as it leaves, level by mid swing, toes up to land.
    if (g.type === "walk")
      for (let i = 0; i < 2; i++) {
        const f = fs[i]!;
        if (f.swing < 0) continue;
        const fwd = V(Math.sin(f.yaw - ryaw), 0, Math.cos(f.yaw - ryaw));
        const lean = this.shinLean(p, i, fwd);
        const rel = lerp(-16, 2, smooth(0.1, 0.65, f.swing)) * DEG;
        const w = smooth(0, 0.22, f.swing) * (1 - smooth(0.72, 1, f.swing));
        const pitch = lerp(f.pitch, rel - lean, w);
        st.toBody(_v1.set(f.x, this.clearY(f.base, pitch, f.toe, f.clear), f.z), p.feet[i]!.pos);
        p.feet[i]!.quat.setFromAxisAngle(Y, f.yaw - ryaw).multiply(qAxis(X, -pitch, _q1));
      }
    const dy = p.pelvis.y - restY;
    const held =
      g.type === "work" && g.work === "pick"
        ? smooth(g.t1 - 0.5, g.t1, tm)
        : g.type === "work" && g.work === "place"
          ? 1 - smooth(g.t0, g.t0 + 0.5, tm)
          : g.upper !== "free"
            ? 1
            : 0;
    if (held > 0) {
      p.itemPos.y += dy * held;
      for (const a of p.arms) if (a.ik > 0) a.pos.y += dy * held;
    }
    return { state: { activity: g.type, work: g.work, local, holding: p.itemOn }, pose: p };
  }
}

export interface RouteOptions {
  /** Walking speed, m/s (default 0.8). */
  speed?: number;
  /** Seconds spent at each waypoint (one value, or one per waypoint). Default 4. */
  dwell?: number | readonly number[];
  /** What is done at each waypoint (one kind, or one per waypoint). Default "stand". */
  work?: WorkKind | readonly WorkKind[];
  /** Which way to face at each waypoint (yaw), or null to face the way they arrived. */
  facing?: readonly (number | null | undefined)[];
  /** Work options per waypoint (world targets). */
  workOpts?: readonly (WorkOpts | undefined)[];
  /** Upper body while walking each leg (leg i runs from waypoint i to i + 1). */
  upper?: readonly (Upper | undefined)[];
}

/**
 * A looping route through floor waypoints (the scene's x, z): at each waypoint the person turns to face the
 * station and works there, then turns and walks to the next, round and back to the first.
 */
export function routeRoutine(staff: Staff, waypoints: readonly (readonly [number, number])[], o: RouteOptions = {}): Routine {
  const n = waypoints.length;
  const at = <T>(v: T | readonly T[] | undefined, i: number, d: T): T => (Array.isArray(v) ? ((v as readonly T[])[i] ?? d) : ((v as T | undefined) ?? d));
  const heading = (a: readonly [number, number], b: readonly [number, number]) => Math.atan2(b[0] - a[0], b[1] - a[1]);
  if (n < 2) {
    const w = waypoints[0] ?? [0, 0];
    return new Routine(staff, { x: w[0], z: w[1], yaw: o.facing?.[0] ?? 0 }, [{ kind: "work", work: at(o.work, 0, "stand"), duration: at(o.dwell, 0, 4), opts: o.workOpts?.[0] }]);
  }
  const face = (i: number) => o.facing?.[i] ?? heading(waypoints[(i - 1 + n) % n]!, waypoints[i]!);
  const steps: RoutineStep[] = [];
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    steps.push({ kind: "work", work: at(o.work, i, "stand"), duration: at(o.dwell, i, 4), opts: o.workOpts?.[i] });
    const upper = o.upper?.[i] ?? "free";
    steps.push({ kind: "walk", path: [waypoints[j]!], speed: o.speed ?? 0.8, upper });
    steps.push({ kind: "turn", yaw: face(j), upper });
  }
  return new Routine(staff, { x: waypoints[0]![0], z: waypoints[0]![1], yaw: face(0) }, steps, true);
}

/** Build a person on a body built to the cohort's row, dressed by `opts.dress` (default: plain workwear). */
export function makeStaff(cohort: Cohort, detail: Detail = 1, opts: StaffOptions = {}): Staff {
  return new Staff(cohort, detail, opts);
}
