/**
 * Labels for the drill scene that keep out of each other's way and off the HUD cards. Each tag is a stage label
 * pinned exactly to its point; every frame the most important tags are placed first, each lifted on a stem above
 * its point to the right, or flipped to the left, or lifted higher, whichever first lands clear of the cards, the
 * frame's edges and the tags already placed. A tag with no clear place fades out until it has one, as does a tag
 * far beyond what the camera is looking at.
 */
import * as THREE from "three";
import type { Label, Stage } from "./stage";

export interface Tag {
  html(s: string): void;
  anchor(to: THREE.Vector3 | THREE.Object3D): void;
  remove(): void;
}

interface Item {
  label: Label;
  at: THREE.Vector3 | THREE.Object3D;
  prio: number;
  /** Pixels above the point; 0 puts the tag beside the point, with no stem. */
  lift: number;
  w: number;
  h: number;
  on: boolean;
}

type Rect = [x0: number, y0: number, x1: number, y1: number];
const hit = (a: Rect, b: Rect) => a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1];

export function tags(s: Stage, avoid: () => Iterable<Element>) {
  const items = new Set<Item>();
  const world = new THREE.Vector3();
  const rem = 0.68 * 16;
  const place = () => {
    const vr = s.view.getBoundingClientRect();
    const blocked: Rect[] = [];
    for (const el of avoid()) {
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) continue;
      blocked.push([r.left - vr.left - 6, r.top - vr.top - 6, r.right - vr.left + 6, r.bottom - vr.top + 6]);
    }
    // Sizes are read every frame (fonts arrive late, and a tag's text changes), before anything is written.
    for (const it of items) {
      it.w = it.label.el.offsetWidth;
      it.h = it.label.el.offsetHeight;
    }
    const order = [...items].sort((a, b) => b.prio - a.prio);
    const W = vr.width;
    const H = vr.height;
    for (const it of order) {
      const p = it.at instanceof THREE.Vector3 ? world.copy(it.at) : it.at.getWorldPosition(world);
      // Up close, tags for things far beyond what the camera is looking at would float over the near scene.
      const far = s.camera.position.distanceTo(p) > 3 * s.orbit.distance + 60;
      const sp = far ? null : s.project(p);
      let chosen: { r: Rect; dx: number; dy: number; left: boolean; lift: number } | null = null;
      if (sp && sp.x > 0 && sp.x < W && sp.y > 0 && sp.y < H) {
        // Where the stage itself puts the tag: hanging right of the point, or left of it near the right edge.
        const stageLeft = sp.x > W * 0.62;
        const baseX = stageLeft ? sp.x + rem - it.w : sp.x - rem;
        const lifts = it.lift ? [it.lift, it.lift + it.h + 6, it.lift + 2 * (it.h + 6)] : [0];
        search: for (const lift of lifts)
          for (const left of [stageLeft, !stageLeft]) {
            let r: Rect;
            if (it.lift) {
              const x0 = left ? sp.x + rem - it.w : sp.x - rem;
              r = [x0, sp.y - lift - it.h, x0 + it.w, sp.y - lift];
            } else {
              const x0 = left ? sp.x - 8 - it.w : sp.x + 8;
              r = [x0, sp.y - it.h / 2, x0 + it.w, sp.y + it.h / 2];
            }
            if (r[0] < 4 || r[1] < 4 || r[2] > W - 4 || r[3] > H - 4) continue;
            if (blocked.some((bl) => hit(bl, r))) continue;
            chosen = { r, dx: r[0] - baseX, dy: it.lift ? -lift : it.h / 2, left, lift };
            break search;
          }
      }
      if (!chosen) {
        if (it.on) {
          it.label.show(false);
          it.on = false;
        }
        continue;
      }
      blocked.push([chosen.r[0] - 4, chosen.r[1] - 3, chosen.r[2] + 4, chosen.r[3] + 3]);
      const el = it.label.el;
      el.style.translate = `${chosen.dx.toFixed(1)}px ${chosen.dy.toFixed(1)}px`;
      el.style.setProperty("--stem", `${chosen.lift}px`);
      el.classList.toggle("dl-left", chosen.left);
      el.classList.toggle("dl-right", !chosen.left);
      if (!it.on) {
        it.label.show(true);
        it.on = true;
      }
    }
  };
  s.frame(place);
  return {
    add(html: string, at: THREE.Vector3 | THREE.Object3D, o: { prio: number; lift?: number; className?: string }): Tag {
      // Free: this module places the tag; the stage only stands it at its point.
      const label = s.label(`<span>${html}</span>`, at, { className: o.className, free: true });
      // Hidden until first placed, so a new tag never shows for a frame where the stage has yet to put it.
      label.show(false);
      const it: Item = { label, at, prio: o.prio, lift: o.lift ?? 0, w: 0, h: 0, on: false };
      items.add(it);
      s.loop.poke();
      return {
        html(h) {
          label.html(`<span>${h}</span>`);
        },
        anchor(to) {
          it.at = to;
          label.anchor(to);
        },
        remove() {
          items.delete(it);
          label.remove();
        },
      };
    },
  };
}
