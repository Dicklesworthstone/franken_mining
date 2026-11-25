/**
 * The shared stage for FrankenMining's 3D scenes: a renderer on the figure's own canvas, a sky and sun that follow a
 * clock, soft shadows, haze, reflections, an orbit camera that never steals the page's scroll, and HTML labels pinned
 * to points in the world. Every scene builds on this, so they share one look.
 *
 *   const s = stage(host, { clock: 17.2, fog: 0.0011 });
 *   s.orbit.set({ target: [0, -60, 0], distance: 900, azimuth: 35, elevation: 32 });
 *   const tag = s.label("Payload <b>348 t</b>", truck.group, { y: 9 });
 *   s.frame((dt, t) => { ...move things... });
 *   s.start();
 */
import * as THREE from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { clamp, loop, mix, reduced, type Loop } from "./lib";

export type Sky = "open" | "studio" | "underground";

export interface StageOptions {
  /** Hours on a 24-hour clock; sets the sun, the sky and the light. Ignored by the studio and underground skies. */
  clock?: number;
  sky?: Sky;
  /** Exponential haze density per metre. */
  fog?: number;
  /** Azimuth of the sun's path in degrees (where it sets), measured from +z towards +x. */
  sunBearing?: number;
  /** Half-width of the sun's shadow box, in metres, around `shadowAt`. */
  shadowRadius?: number;
  shadowAt?: [number, number, number];
  shadowMap?: number;
  fov?: number;
  near?: number;
  far?: number;
  maxPixelRatio?: number;
  exposure?: number;
  /** Let the camera turn by itself when nobody is touching it (degrees per second). */
  drift?: number;
  /** Whether a drag turns the camera. */
  interactive?: boolean;
  /** An aria-label for the canvas. */
  label?: string;
  /** Moves the picture across the frame, as fractions of its width and height (a lens shift, not a camera move). */
  shift?: () => [number, number];
  /** Where labels may stand, as fractions of the view [left, top, right, bottom]; a label whose anchor falls outside hides. */
  labelArea?: () => [number, number, number, number];
  /** A line (a fraction of the view's height) no label may rise above, such as the bottom of text laid over the scene. */
  labelCeiling?: () => number;
  /** What is laid over the view (its cards, a hint line) that labels keep out from under. */
  avoid?: string;
}

export interface Orbit {
  target: THREE.Vector3;
  distance: number;
  /** Degrees around the target, from +z towards +x. */
  azimuth: number;
  /** Degrees above the horizon. */
  elevation: number;
  minElevation: number;
  maxElevation: number;
  minDistance: number;
  maxDistance: number;
  /** Eases towards a new view over `seconds`. */
  set(v: Partial<{ target: [number, number, number] | THREE.Vector3; distance: number; azimuth: number; elevation: number; fov: number }>, seconds?: number): void;
  /** True while a person is dragging or the camera is still easing. */
  readonly busy: boolean;
}

export interface Label {
  el: HTMLElement;
  /** Replaces the label's contents (HTML). */
  html(s: string): void;
  show(on: boolean): void;
  /** Moves the anchor: a fixed point, or an object followed every frame (plus an offset in metres). */
  anchor(to: THREE.Vector3 | THREE.Object3D, offset?: { x?: number; y?: number; z?: number }): void;
  remove(): void;
}

export interface Stage {
  view: HTMLElement;
  canvas: HTMLCanvasElement;
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  sun: THREE.DirectionalLight;
  hemi: THREE.HemisphereLight;
  orbit: Orbit;
  loop: Loop;
  /** Current clock in hours, and how dark it is (0 day, 1 night). */
  readonly clock: number;
  readonly night: number;
  setClock(hours: number): void;
  /** Re-centres the shadow box (call when the camera's region of interest moves). */
  shadowBox(at: [number, number, number] | THREE.Vector3, radius?: number): void;
  /** A label pinned to a point. One that is `free` is placed by its figure, which keeps it clear of the rest: the stage
   *  only stands it at its point, hanging left of it in the right part of the view. */
  label(html: string, at: THREE.Vector3 | THREE.Object3D, o?: { x?: number; y?: number; z?: number; className?: string; free?: boolean }): Label;
  /** Called every frame with the step and the time in seconds. */
  frame(fn: (dt: number, t: number) => void): void;
  /** Called when the night fraction changes, e.g. to switch lamps on. */
  onNight(fn: (night: number) => void): void;
  start(): void;
  render(): void;
  /** Called once, just after the first frame is drawn (for swapping a still picture for the live scene). */
  firstFrame(fn: () => void): void;
  /** Screen position of a world point in CSS pixels, or null when behind the camera. */
  project(p: THREE.Vector3): { x: number; y: number } | null;
  readonly width: number;
  readonly height: number;
}

const HAZE_DAY = new THREE.Color("#cdbfa9");
const HAZE_GOLD = new THREE.Color("#c99a6b");
const HAZE_NIGHT = new THREE.Color("#0a0c12");

/** A canvas inside `host`, or the one already there, and the overlay for labels. */
function surface(host: HTMLElement, label?: string) {
  let view = host.classList.contains("scene-view") || host.querySelector(":scope > canvas") ? host : host.querySelector<HTMLElement>(".scene-view");
  if (!view) {
    view = document.createElement("div");
    view.className = "scene-view";
    host.append(view);
  }
  let canvas = view.querySelector("canvas");
  if (!canvas) {
    canvas = document.createElement("canvas");
    view.prepend(canvas);
  }
  if (label) canvas.setAttribute("aria-label", label);
  let labels = view.querySelector<HTMLElement>(".scene-labels");
  if (!labels) {
    labels = document.createElement("div");
    labels.className = "scene-labels";
    labels.setAttribute("aria-hidden", "true");
    view.append(labels);
  }
  return { view, canvas, labels };
}

/** Sun direction and colours for a clock time. Sunrise at 6, sunset at 18. */
function solar(hours: number, bearing: number) {
  const day = (hours - 6) / 12; // 0 at sunrise, 1 at sunset
  const elevation = Math.sin(clamp(day, -0.15, 1.15) * Math.PI) * 58; // degrees
  const az = THREE.MathUtils.degToRad(bearing + mix(-75, 75, clamp(day, 0, 1)) + 180);
  const el = THREE.MathUtils.degToRad(Math.max(elevation, -8));
  const dir = new THREE.Vector3(Math.sin(az) * Math.cos(el), Math.sin(el), Math.cos(az) * Math.cos(el)).normalize();
  // 0 at full day, 1 at full night; the golden hour sits between.
  const night = clamp(1 - (elevation + 6) / 10, 0, 1);
  const gold = clamp(1 - (elevation - 2) / 22, 0, 1) * (1 - night);
  return { dir, elevation, night, gold };
}

export function stage(host: HTMLElement, o: StageOptions = {}): Stage {
  const { view, canvas, labels } = surface(host, o.label);
  const sky: Sky = o.sky ?? "open";
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, powerPreference: "high-performance", alpha: false });
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = o.exposure ?? 1;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  // Reading a shader's compile log makes the page wait for the driver to finish compiling it, a stall of tens to
  // hundreds of milliseconds per shader. The checks run only when asked for (?shaders).
  renderer.debug.checkShaderErrors = typeof location !== "undefined" && location.search.includes("shaders");
  const maxRatio = o.maxPixelRatio ?? 1.75;

  const scene = new THREE.Scene();
  const environment = () => {
    const pmrem = new THREE.PMREMGenerator(renderer);
    scene.environment?.dispose();
    scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
    pmrem.dispose();
  };
  environment();
  scene.environmentIntensity = 0.22;
  const fogDensity = o.fog ?? 0.0009;
  scene.fog = new THREE.FogExp2(HAZE_DAY.clone(), fogDensity);

  const camera = new THREE.PerspectiveCamera(o.fov ?? 32, 1, o.near ?? 0.5, o.far ?? 12000);

  /* ---------- Sky ---------- */
  const skyUniforms = {
    sun: { value: new THREE.Vector3(0, 1, 0) },
    night: { value: 0 },
    gold: { value: 0 },
    mode: { value: sky === "open" ? 0 : sky === "studio" ? 1 : 2 },
  };
  const dome = new THREE.Mesh(
    new THREE.SphereGeometry(1, 48, 24),
    new THREE.ShaderMaterial({
      side: THREE.BackSide,
      depthWrite: false,
      depthTest: false,
      fog: false,
      uniforms: skyUniforms,
      vertexShader: `varying vec3 vD;
void main(){ vD = normalize(position); vec4 p = projectionMatrix * mat4(mat3(modelViewMatrix)) * vec4(position, 1.0); gl_Position = p.xyww; }`,
      fragmentShader: `uniform vec3 sun; uniform float night; uniform float gold; uniform float mode; varying vec3 vD;
void main(){
  vec3 d = normalize(vD);
  float h = clamp(d.y, -0.25, 1.0);
  vec3 c;
  if (mode > 0.5) {
    // A dark studio: a soft warm glow behind the subject, near-black elsewhere.
    float g = pow(max(0.0, 1.0 - abs(h - 0.12) * 1.7), 2.0);
    c = mode > 1.5 ? vec3(0.006, 0.005, 0.005) : mix(vec3(0.016, 0.012, 0.010), vec3(0.075, 0.055, 0.042), g);
  } else {
    vec3 zen = mix(vec3(0.20, 0.38, 0.68), vec3(0.20, 0.25, 0.42), gold);
    vec3 hor = mix(vec3(0.80, 0.77, 0.72), vec3(0.98, 0.66, 0.42), gold);
    c = mix(hor, zen, pow(max(h, 0.0), 0.5));
    float s = max(dot(d, sun), 0.0);
    c += mix(vec3(1.0, 0.85, 0.62), vec3(1.0, 0.55, 0.25), gold) * (pow(s, 6.0) * (0.25 + 0.55 * gold) + pow(s, 600.0) * 3.0);
    c = mix(c, mix(vec3(0.72, 0.64, 0.55), vec3(0.55, 0.38, 0.28), gold), smoothstep(0.0, -0.18, d.y));
    vec3 nightC = mix(vec3(0.010, 0.014, 0.028), vec3(0.045, 0.05, 0.075), pow(1.0 - max(h, 0.0), 3.0));
    // A few stars.
    vec3 q = floor(d * 380.0);
    float n = fract(sin(dot(q, vec3(12.9898, 78.233, 37.719))) * 43758.5453);
    nightC += step(0.9975, n) * smoothstep(0.05, 0.4, h) * vec3(0.7, 0.75, 0.85);
    c = mix(c, nightC, night);
  }
  gl_FragColor = vec4(c, 1.0);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}`,
    }),
  );
  dome.frustumCulled = false;
  dome.renderOrder = -1000;
  scene.add(dome);

  /* ---------- Light ---------- */
  const hemi = new THREE.HemisphereLight("#b9cde6", "#6a5640", 0.55);
  scene.add(hemi);
  const sun = new THREE.DirectionalLight("#ffe7c7", 5);
  sun.castShadow = true;
  const sm = o.shadowMap ?? 4096;
  sun.shadow.mapSize.set(sm, sm);
  sun.shadow.bias = -0.00025;
  sun.shadow.normalBias = 0.04;
  sun.shadow.radius = 3;
  scene.add(sun, sun.target);

  let shadowCentre = new THREE.Vector3(...(o.shadowAt ?? [0, 0, 0]));
  let shadowRadius = o.shadowRadius ?? 120;
  let sunDir = new THREE.Vector3(0, 1, 0);
  const placeSun = () => {
    const c = sun.shadow.camera;
    c.left = c.bottom = -shadowRadius;
    c.right = c.top = shadowRadius;
    c.near = 1;
    c.far = shadowRadius * 6 + 600;
    c.updateProjectionMatrix();
    sun.target.position.copy(shadowCentre);
    sun.position.copy(shadowCentre).addScaledVector(sunDir, shadowRadius * 3 + 300);
    sun.target.updateMatrixWorld();
  };

  let clock = o.clock ?? 16.5;
  let night = 0;
  const nightFns: ((n: number) => void)[] = [];
  const setClock = (hours: number) => {
    clock = hours;
    if (sky !== "open") {
      sunDir = new THREE.Vector3(-0.5, 0.75, 0.45).normalize();
      night = 0;
      sun.color.set(sky === "studio" ? "#fff1e0" : "#ffe9cf");
      sun.intensity = sky === "studio" ? 3.2 : 0;
      hemi.intensity = sky === "studio" ? 0.35 : 0.06;
      hemi.color.set("#c9d6e8");
      hemi.groundColor.set("#3a2c22");
      scene.environmentIntensity = sky === "studio" ? 0.3 : 0.05;
      (scene.fog as THREE.FogExp2).color.set(sky === "studio" ? "#120e0b" : "#050404");
      placeSun();
      return;
    }
    const s = solar(hours, o.sunBearing ?? 0);
    sunDir = s.dir;
    night = s.night;
    skyUniforms.sun.value.copy(s.dir);
    skyUniforms.night.value = s.night;
    skyUniforms.gold.value = s.gold;
    // Warm and low at the golden hour, white at noon, a dim blue moonlight at night.
    const day = 1 - s.night;
    sun.color.set("#fff3e2").lerp(new THREE.Color("#ff9a52"), s.gold * 0.85);
    sun.intensity = mix(0.12, mix(5.4, 3.6, s.gold), day);
    if (s.night > 0.6) {
      // Moonlight from high in the opposite quarter.
      sunDir = new THREE.Vector3(-s.dir.x, 0.8, -s.dir.z).normalize();
      sun.color.set("#9fb4e0");
    }
    hemi.intensity = mix(0.07, mix(0.6, 0.45, s.gold), day);
    hemi.color.set("#b9cde6").lerp(new THREE.Color("#e8b48a"), s.gold * 0.6);
    scene.environmentIntensity = mix(0.03, 0.22, day);
    const haze = (scene.fog as THREE.FogExp2).color;
    haze.copy(HAZE_DAY).lerp(HAZE_GOLD, s.gold).lerp(HAZE_NIGHT, s.night);
    renderer.toneMappingExposure = (o.exposure ?? 1) * mix(1.35, 1, day);
    placeSun();
    for (const fn of nightFns) fn(night);
  };

  /* ---------- Camera ---------- */
  const want = { target: new THREE.Vector3(), distance: 300, azimuth: 30, elevation: 30, fov: camera.fov };
  const now = { target: new THREE.Vector3(), distance: 300, azimuth: 30, elevation: 30, fov: camera.fov };
  let ease = 0;
  let dragging = false;
  let idle = 0;
  const orbit: Orbit = {
    get target() {
      return want.target;
    },
    get distance() {
      return want.distance;
    },
    set distance(v) {
      want.distance = clamp(v, orbit.minDistance, orbit.maxDistance);
    },
    get azimuth() {
      return want.azimuth;
    },
    set azimuth(v) {
      want.azimuth = v;
    },
    get elevation() {
      return want.elevation;
    },
    set elevation(v) {
      want.elevation = clamp(v, orbit.minElevation, orbit.maxElevation);
    },
    minElevation: 4,
    maxElevation: 85,
    minDistance: 5,
    maxDistance: 5000,
    set(v, seconds = 0) {
      if (v.target) want.target.copy(v.target instanceof THREE.Vector3 ? v.target : new THREE.Vector3(...v.target));
      if (v.distance !== undefined) want.distance = v.distance;
      if (v.azimuth !== undefined) want.azimuth = v.azimuth;
      if (v.elevation !== undefined) want.elevation = v.elevation;
      if (v.fov !== undefined) want.fov = v.fov;
      if (seconds <= 0 || reduced) {
        now.target.copy(want.target);
        now.distance = want.distance;
        now.azimuth = want.azimuth;
        now.elevation = want.elevation;
        now.fov = want.fov;
        ease = 0;
      } else ease = 1 / seconds;
      st.loop.poke();
    },
    get busy() {
      return dragging || ease > 0;
    },
  } as Orbit;

  const placeCamera = (dt: number) => {
    const k = ease > 0 ? 1 - Math.exp(-dt * 4 * ease * 1.6) : 1;
    now.target.lerp(want.target, k);
    now.distance = mix(now.distance, want.distance, k);
    // Turn the short way round.
    let da = want.azimuth - now.azimuth;
    da = ((((da + 180) % 360) + 360) % 360) - 180;
    now.azimuth += da * k;
    now.elevation = mix(now.elevation, want.elevation, k);
    now.fov = mix(now.fov, want.fov, k);
    if (ease > 0 && Math.abs(now.distance - want.distance) < 0.05 && Math.abs(da) < 0.02 && now.target.distanceTo(want.target) < 0.02)
      ease = 0;
    const az = THREE.MathUtils.degToRad(now.azimuth);
    const el = THREE.MathUtils.degToRad(now.elevation);
    camera.position.set(
      now.target.x + now.distance * Math.cos(el) * Math.sin(az),
      now.target.y + now.distance * Math.sin(el),
      now.target.z + now.distance * Math.cos(el) * Math.cos(az),
    );
    // Depth precision: the near plane moves out with the orbit, so a wide view of a pit keeps centimetres of depth
    // resolution at a kilometre and coplanar-looking surfaces never fight. A scene may pin it with `near`.
    if (o.near === undefined) {
      const near = clamp(now.distance * 0.035, 0.25, 40);
      if (Math.abs(near - camera.near) > camera.near * 0.08) {
        camera.near = near;
        camera.updateProjectionMatrix();
      }
    }
    if (camera.fov !== now.fov) {
      camera.fov = now.fov;
      camera.updateProjectionMatrix();
    }
    camera.lookAt(now.target);
  };

  // A drag turns the view; a vertical swipe on a phone still scrolls the page. Two fingers zoom.
  if (o.interactive !== false) {
    let px = 0;
    let py = 0;
    const touches = new Map<number, { x: number; y: number }>();
    let pinch = 0;
    canvas.addEventListener("pointerdown", (e) => {
      touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (e.pointerType === "mouse" && e.button !== 0) return;
      dragging = true;
      px = e.clientX;
      py = e.clientY;
      canvas.setPointerCapture(e.pointerId);
      st.loop.poke();
    });
    canvas.addEventListener("pointermove", (e) => {
      if (touches.has(e.pointerId)) touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
      if (touches.size === 2) {
        const [a, b] = [...touches.values()];
        const d = Math.hypot(a!.x - b!.x, a!.y - b!.y);
        if (pinch) orbit.distance = want.distance * (pinch / d);
        pinch = d;
        ease = Math.max(ease, 4);
        st.loop.poke();
        return;
      }
      if (!dragging) return;
      const dx = e.clientX - px;
      const dy = e.clientY - py;
      px = e.clientX;
      py = e.clientY;
      want.azimuth -= dx * 0.28;
      want.elevation = clamp(want.elevation + dy * 0.2, orbit.minElevation, orbit.maxElevation);
      ease = Math.max(ease, 6);
      idle = 0;
      st.loop.poke();
    });
    const end = (e: PointerEvent) => {
      touches.delete(e.pointerId);
      if (touches.size < 2) pinch = 0;
      dragging = false;
    };
    canvas.addEventListener("pointerup", end);
    canvas.addEventListener("pointercancel", end);
    // The wheel zooms only with a modifier, so it never traps the page's scroll.
    canvas.addEventListener(
      "wheel",
      (e) => {
        if (!e.ctrlKey && !e.metaKey) return;
        e.preventDefault();
        orbit.distance = want.distance * Math.exp(e.deltaY * 0.0022);
        ease = Math.max(ease, 6);
        st.loop.poke();
      },
      { passive: false },
    );
  }

  /* ---------- Labels ---------- */
  type Pin = { el: HTMLElement; at: THREE.Vector3 | THREE.Object3D; off: THREE.Vector3; on: boolean; free?: boolean; nat?: number; h?: number; cap?: string };
  const pins = new Set<Pin>();
  // A label measured before the web fonts arrived has the fallback font's size: measure every label again once they do.
  document.fonts?.addEventListener?.("loadingdone", () => {
    for (const pin of pins) pin.nat = undefined;
  });
  const tmp = new THREE.Vector3();
  let width = 1;
  let height = 1;
  const project = (p: THREE.Vector3) => {
    tmp.copy(p).project(camera);
    if (tmp.z > 1 || tmp.z < -1) return null;
    return { x: (tmp.x * 0.5 + 0.5) * width, y: (-tmp.y * 0.5 + 0.5) * height };
  };
  let area: [number, number, number, number] = [-0.15, -0.1, 1.15, 1.1];
  let ceilingAt = -Infinity;
  // The cards and notes laid over the view, as boxes in its pixels [left, top, right, bottom]. A label whose point is
  // under one hides (what it names cannot be seen); the rest keep clear of them as they keep clear of each other.
  // Measured when one of them or the view changes size, just after layout, so a frame never asks for it.
  let cover: [number, number, number, number][] = [];
  // Some figures lay their cards inside the view, others beside it in a wrapper; only the ones over the view count.
  const overlays = [...(view.closest("figure, section") ?? view.parentElement ?? view).querySelectorAll<HTMLElement>(o.avoid ?? ".hud-card, [data-avoid]")];
  const measureCover = () => {
    const v = view.getBoundingClientRect();
    cover = [];
    for (const el of overlays) {
      const r = el.getBoundingClientRect();
      if (r.width < 2 || r.height < 2 || r.right <= v.left || r.left >= v.right || r.bottom <= v.top || r.top >= v.bottom) continue;
      cover.push([r.left - v.left, r.top - v.top, r.right - v.left, r.bottom - v.top]);
    }
  };
  if (overlays.length) {
    const ro = new ResizeObserver(measureCover);
    ro.observe(view);
    for (const el of overlays) ro.observe(el);
  }
  const placed: { pin: Pin; x: number; y: number; w: number; h: number; flip: boolean; lift: number }[] = [];
  const placeLabels = () => {
    placed.length = 0;
    for (const pin of pins) {
      if (!pin.on) continue;
      const base = pin.at instanceof THREE.Vector3 ? pin.at : pin.at.getWorldPosition(tmp);
      const world = (pin.at instanceof THREE.Vector3 ? tmp.copy(base) : base).add(pin.off);
      const s = project(world);
      const [l, tp, r, b] = area;
      const under = !pin.free && !!s && cover.some(([cl, ct, cr, cb]) => s.x > cl && s.x < cr && s.y > ct && s.y < cb);
      if (!s || s.x < l * width || s.x > r * width || s.y < tp * height || s.y > b * height || under) {
        pin.el.style.visibility = "hidden";
        continue;
      }
      pin.el.style.visibility = "";
      if (pin.free) {
        const flip = s.x > width * 0.62;
        if (pin.el.classList.contains("flip") !== flip) pin.el.classList.toggle("flip", flip);
        pin.el.style.transform = flip
          ? `translate3d(${s.x.toFixed(1)}px, ${s.y.toFixed(1)}px, 0) translate(calc(-100% + 0.68rem), -100%)`
          : `translate3d(${s.x.toFixed(1)}px, ${s.y.toFixed(1)}px, 0) translate(-0.68rem, -100%)`;
        continue;
      }
      // A label's natural width, measured once per text (a capped label cannot say how wide it wants to be). Sizes
      // are read only when the text or its cap changes, so a steady frame never asks the browser for layout.
      if (pin.nat === undefined) {
        pin.el.style.maxWidth = "";
        pin.el.classList.remove("capped");
        pin.cap = "";
        pin.nat = pin.el.offsetWidth;
        pin.h = pin.el.offsetHeight;
      }
      // Near the right edge a label hangs to the left of its point; one too long for its side takes the roomier
      // side, and one too long for either is capped (its words take a line of their own above its number).
      // Labels keep to the label area (a little wider), so none runs over the page's text beside the scene.
      const leftB = clamp((area[0] - 0.03) * width, 6, width);
      const rightB = clamp((area[2] + 0.1) * width, 0, width - 6);
      const roomR = rightB - s.x + 11;
      const roomL = s.x + 11 - leftB;
      let flip = s.x > width * 0.62;
      if (!flip && pin.nat > roomR && roomL > roomR) flip = true;
      else if (flip && pin.nat > roomL && roomR > roomL) flip = false;
      const room = flip ? roomL : roomR;
      // Caps move in 8 px steps, so a label drifting across the view does not re-wrap (and re-measure) every frame.
      const cap = pin.nat > room ? `${Math.max(80, Math.floor(room / 8) * 8)}px` : "";
      if (pin.cap !== cap) {
        pin.el.style.maxWidth = cap;
        // A capped label puts its words on a line of their own (in the CSS) instead of cutting them short.
        pin.el.classList.toggle("capped", !!cap);
        pin.cap = cap;
        pin.h = pin.el.offsetHeight;
      }
      placed.push({ pin, x: s.x, y: s.y, w: Math.min(pin.nat, room), h: pin.h ?? 0, flip, lift: 0 });
    }
    // Labels never sit on one another: from the lowest on screen up, a label that would overlap one already placed
    // lifts clear of it, and a leader line runs back down to its point.
    placed.sort((p, q) => q.y - p.y);
    const boxes: [number, number, number, number][] = cover.slice();
    const nudge = 11;
    const ceiling = Math.max(area[1] * height - 2, ceilingAt * height);
    for (const p of placed) {
      const left = () => (p.flip ? p.x - p.w + nudge : p.x - nudge);
      let bottom = p.y;
      for (let guard = 0; guard < 8; guard++) {
        const l0 = left();
        const hit = boxes.find(([bl, bt, br, bb]) => l0 < br + 6 && l0 + p.w > bl - 6 && bottom - p.h < bb + 4 && bottom > bt - 4);
        if (!hit) break;
        bottom = hit[1] - 6;
      }
      p.lift = p.y - bottom;
      const el = p.pin.el;
      // A label lifted out of the label area, or one that would reach over the page's text or buttons, gives way to
      // the ones below it.
      if ((p.lift > 1 || ceilingAt > -Infinity) && bottom - p.h < ceiling) {
        el.style.visibility = "hidden";
        continue;
      }
      boxes.push([left(), bottom - p.h, left() + p.w, bottom]);
      if (el.classList.contains("flip") !== p.flip) el.classList.toggle("flip", p.flip);
      el.classList.toggle("lifted", p.lift > 1);
      if (p.lift > 1) el.style.setProperty("--lift", `${p.lift.toFixed(0)}px`);
      el.style.transform = p.flip
        ? `translate3d(${p.x.toFixed(1)}px, ${bottom.toFixed(1)}px, 0) translate(calc(-100% + 0.68rem), -100%)`
        : `translate3d(${p.x.toFixed(1)}px, ${bottom.toFixed(1)}px, 0) translate(-0.68rem, -100%)`;
    }
  };

  /* ---------- Size ---------- */
  const resize = () => {
    const w = view.clientWidth;
    const h = view.clientHeight;
    if (!w || !h) return;
    width = w;
    height = h;
    area = o.labelArea?.() ?? [-0.15, -0.1, 1.15, 1.1];
    ceilingAt = o.labelCeiling?.() ?? -Infinity;
    for (const pin of pins) pin.nat = undefined;
    renderer.setPixelRatio(Math.min(maxRatio, devicePixelRatio || 1));
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    const [sx, sy] = o.shift?.() ?? [0, 0];
    if (sx || sy) camera.setViewOffset(w, h, -sx * w, -sy * h, w, h);
    else camera.clearViewOffset();
    camera.updateProjectionMatrix();
    st.loop.poke();
  };

  /* ---------- Frame ---------- */
  const frames: ((dt: number, t: number) => void)[] = [];
  let t = 0;
  const draw = (dt: number) => {
    t += dt;
    if (!dragging && o.drift && !reduced) {
      idle += dt;
      if (idle > 2.5) want.azimuth += o.drift * dt * Math.min(1, (idle - 2.5) / 2);
      ease = Math.max(ease, 3);
    }
    for (const fn of frames) fn(dt, t);
    placeCamera(Math.max(dt, 1 / 60));
    dome.position.copy(camera.position);
    dome.scale.setScalar(camera.far * 0.9);
    renderer.render(scene, camera);
    placeLabels();
    if (firsts.length) for (const fn of firsts.splice(0)) fn();
  };
  const firsts: (() => void)[] = [];

  const st: Stage = {
    view,
    canvas,
    renderer,
    scene,
    camera,
    sun,
    hemi,
    orbit,
    loop: undefined as unknown as Loop,
    get clock() {
      return clock;
    },
    get night() {
      return night;
    },
    get width() {
      return width;
    },
    get height() {
      return height;
    },
    setClock,
    shadowBox(at, radius) {
      shadowCentre = at instanceof THREE.Vector3 ? at.clone() : new THREE.Vector3(...at);
      if (radius) shadowRadius = radius;
      placeSun();
    },
    label(html, at, off = {}) {
      const el = document.createElement("div");
      el.className = `label3d ${off.className ?? ""}`.trim();
      el.innerHTML = html;
      el.style.visibility = "hidden";
      labels.append(el);
      const pin: Pin = { el, at, off: new THREE.Vector3(off.x ?? 0, off.y ?? 0, off.z ?? 0), on: true, free: off.free };
      pins.add(pin);
      return {
        el,
        html(s) {
          if (el.innerHTML === s) return;
          el.innerHTML = s;
          pin.nat = undefined;
        },
        show(on) {
          pin.on = on;
          el.classList.toggle("off", !on);
        },
        anchor(to, offset) {
          pin.at = to;
          if (offset) pin.off.set(offset.x ?? 0, offset.y ?? 0, offset.z ?? 0);
        },
        remove() {
          pins.delete(pin);
          el.remove();
        },
      };
    },
    frame(fn) {
      frames.push(fn);
    },
    onNight(fn) {
      nightFns.push(fn);
      fn(night);
    },
    start() {
      resize();
      let done = false;
      const go = () => {
        if (done) return;
        done = true;
        st.loop.running = !reduced;
        st.loop.poke();
      };
      // Every shader the scene needs compiles in parallel (KHR_parallel_shader_compile) before the first frame,
      // so the first frame does not stop the page while the driver compiles them one by one. If the context is lost
      // meanwhile (a scene scrolled far away hands it back), three's wait never ends: the scene starts anyway.
      if (reduced) go();
      else {
        renderer.compileAsync(scene, camera).then(go, go);
        setTimeout(go, 4000);
      }
    },
    firstFrame(fn) {
      firsts.push(fn);
    },
    render() {
      draw(0);
    },
    project,
  };
  st.loop = loop(view, (dt) => draw(dt), false);
  new ResizeObserver(resize).observe(view);
  // A page of scenes can hold more WebGL contexts than a phone allows. A scene far off screen hands its context back
  // and takes it again when it comes near; three.js uploads the geometry, textures and shaders again by itself, and
  // the reflections are baked again here.
  let parked = false;
  new IntersectionObserver(
    ([entry]) => {
      if (!entry) return;
      if (!entry.isIntersecting && !parked) {
        parked = true;
        renderer.forceContextLoss();
      } else if (entry.isIntersecting && parked) {
        parked = false;
        renderer.forceContextRestore();
      }
    },
    { rootMargin: "300% 0px" },
  ).observe(view);
  canvas.addEventListener("webglcontextrestored", () => {
    environment();
    resize();
    st.loop.poke();
  });
  setClock(clock);
  // With ?stage in the address, scenes are reachable from the console for tuning: __stages[i].orbit.set({...}, 0).
  if (typeof location !== "undefined" && location.search.includes("stage")) {
    const w = window as unknown as { __stages?: Stage[] };
    (w.__stages ??= []).push(st);
    st.view.dataset.stage = String(w.__stages.length - 1);
  }
  return st;
}

/** A seeded value-noise field and fractal sum, for terrain and textures. */
const GX = new Float64Array([1, -1, 1, -1, 2, 2, -2, -2]);
const GY = new Float64Array([2, 2, -2, -2, 1, -1, 1, -1]);
export function noise2(seed = 1) {
  const perm = new Uint16Array(512);
  let a = seed >>> 0 || 1;
  const rand = () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const p = Array.from({ length: 256 }, (_, i) => i);
  for (let i = 255; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1));
    [p[i], p[j]] = [p[j]!, p[i]!];
  }
  for (let i = 0; i < 512; i++) perm[i] = p[i & 255]!;
  // Perlin noise with its eight gradients as coefficients on (x, y): h & 7 below 4 gives ±x ± 2y, above it ±y ± 2x.
  // The same arithmetic as the branching form (IEEE addition commutes), so the same numbers, bit for bit, about four
  // times as fast; every terrain on the page is built from it.
  const n = (x: number, y: number) => {
    const fx = Math.floor(x);
    const fy = Math.floor(y);
    const X = fx & 255;
    const Y = fy & 255;
    const xf = x - fx;
    const yf = y - fy;
    const u = xf * xf * xf * (xf * (xf * 6 - 15) + 10);
    const v = yf * yf * yf * (yf * (yf * 6 - 15) + 10);
    const pX = perm[X]!;
    const pX1 = perm[X + 1]!;
    const aa = perm[pX + Y]! & 7;
    const ab = perm[pX + Y + 1]! & 7;
    const ba = perm[pX1 + Y]! & 7;
    const bb = perm[pX1 + Y + 1]! & 7;
    const xm = xf - 1;
    const ym = yf - 1;
    const gaa = GX[aa]! * xf + GY[aa]! * yf;
    const gba = GX[ba]! * xm + GY[ba]! * yf;
    const gab = GX[ab]! * xf + GY[ab]! * ym;
    const gbb = GX[bb]! * xm + GY[bb]! * ym;
    const x1 = gaa + (gba - gaa) * u;
    const x2 = gab + (gbb - gab) * u;
    return (x1 + (x2 - x1) * v) * 0.36;
  };
  const fbm = (x: number, y: number, octaves = 5, lacunarity = 2.03, gain = 0.5) => {
    let s = 0;
    let amp = 1;
    let f = 1;
    let norm = 0;
    for (let i = 0; i < octaves; i++) {
      s += amp * n(x * f + i * 17.3, y * f - i * 9.1);
      norm += amp;
      amp *= gain;
      f *= lacunarity;
    }
    return s / norm;
  };
  return { n, fbm };
}
