#!/usr/bin/env node
// Ports the upstream editor's code-drawn icon set to SVG markup.
//
// The upstream icons (`crates/ui-egui/src/icons.rs`, drawing in `crates/ui-egui/src/icons/draw.rs`)
// are original artwork painted with egui primitives on a 16x16 grid. Their drawing code uses loops,
// closures and trigonometry, so it is not parsed: `DRAW` below is a line-by-line transliteration
// of the upstream `draw()` onto a painter that emits SVG instead of egui shapes, keeping what egui
// does at a given number of device pixels per grid unit:
//
// - `line` hints axis-aligned strokes (crossways onto the pixel grid, lengthways onto pixel edges),
//   `poly` and `rect` snap their vertices like stroke centres (pixel centres for odd widths, edges
//   for even ones), `fill_rect` snaps onto pixel edges; paths with curves are drawn as given.
//   Coordinates follow f32 arithmetic (`Math.fround`), so the snapping lands where upstream lands.
// - egui's tessellator then rounds exactly vertical or horizontal segments onto the pixel grid,
//   fades each open stroke's end out over one pixel past its end point (a butt end half a pixel
//   further out here), miters every join of a closed outline but cuts open joins sharper than a
//   right angle (a bevel here, through a miter limit of sqrt(2)), and strokes circles outside
//   their radius.
// - Convex fills are anti-aliased polygons; concave fills are an aliased mesh plus a one-pixel
//   hairline, as upstream; a gradient mesh (triangles with per-vertex alpha) becomes a luminance
//   mask of linear gradients, each exact for its triangle.
//
// The set is drawn at the scales the upstream draws its glyph sizes at (10, 12, 14 and 16 px, at 1x
// and 2x: 0.625 to 2 device pixels per unit); other sizes scale the nearest drawing.
//
// Rerun after an upstream change, from the repository root, giving the path of an upstream checkout:
//
//   node web/scripts/port-icons.mjs <upstream-checkout>
//
// The script reads the upstream icon list, fails if a variant has no drawing here (or the reverse),
// warns when `draw.rs` differs from the revision this port was made from, and writes
// `web/src/icons/icons.ts` and `web/src/icons/gallery.html`.

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** Upstream commit and `draw.rs` digest this port transliterates. */
const UPSTREAM_COMMIT = '43af145a';
const DRAW_RS_SHA256 = '42f5955bacd221d401baba82091440ea471942081124be4525b1db614282bf10';

const here = dirname(fileURLToPath(import.meta.url));
const webDir = resolve(here, '..');
if (!process.argv[2] || process.argv[2].startsWith('--')) {
  console.error('usage: node web/scripts/port-icons.mjs <upstream-checkout> [--out <dir>]');
  process.exit(2);
}
const upstream = resolve(process.argv[2]);
const flag = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined);
/** `--out <dir>` writes elsewhere (for a comparison build). */
const outDir = flag('--out') ? resolve(flag('--out')) : join(webDir, 'src', 'icons');

// ---------------------------------------------------------------------------------------------
// f32 arithmetic, as the upstream computes.

const F = Math.fround;
const PI = F(Math.PI);
const TAU = F(2 * Math.PI);
const FRAC_PI_2 = F(Math.PI / 2);
const FRAC_PI_3 = F(Math.PI / 3);
const FRAC_PI_4 = F(Math.PI / 4);
const FRAC_PI_6 = F(Math.PI / 6);
const sin = (a) => F(Math.sin(a));
const cos = (a) => F(Math.cos(a));
const add = (a, b) => F(a + b);
const sub = (a, b) => F(a - b);
const mul = (a, b) => F(a * b);
const div = (a, b) => F(a / b);
/** Rust's `f32::round`: halves away from zero. */
const round = (v) => (v < 0 ? -Math.round(-v) : Math.round(v));

/** Points along a circular arc from `a0` to `a1` (radians, y down: increasing is clockwise). */
function arcPts(cx, cy, r, a0, a1) {
  const n = Math.min(48, Math.max(6, Math.ceil(mul(mul(Math.abs(sub(a1, a0)), r), 1.5))));
  const out = [];
  for (let i = 0; i <= n; i++) {
    const t = add(a0, div(mul(sub(a1, a0), i), n));
    out.push([add(cx, mul(r, cos(t))), add(cy, mul(r, sin(t)))]);
  }
  return out;
}

/** A capsule (stadium) outline from `a` to `b` with radius `r`. */
function capsule(a, b, r) {
  const th = F(Math.atan2(sub(b[1], a[1]), sub(b[0], a[0])));
  return [
    ...arcPts(b[0], b[1], r, sub(th, FRAC_PI_2), add(th, FRAC_PI_2)),
    ...arcPts(a[0], a[1], r, add(th, FRAC_PI_2), add(th, mul(3, FRAC_PI_2))),
  ];
}

// ---------------------------------------------------------------------------------------------
// The painter: egui's primitives at `k` device pixels per grid unit (k = 1 is 16 px at 1x, k = 2
// is 16 px on a 2x display or 32 px at 1x), emitting coordinates in grid units.

const num = (v) => {
  const s = (Math.round(v * 1000) / 1000).toFixed(3).replace(/\.?0+$/, '');
  return s === '-0' ? '0' : s;
};
const pt = ([x, y]) => `${num(x)} ${num(y)}`;
const d = (pts, closed) => `M${pts.map(pt).join('L')}${closed ? 'Z' : ''}`;

/**
 * egui joins a closed outline with full miters at every angle (an open one cuts joins sharper than
 * a right angle, which the root's miter limit approximates with a bevel).
 */
const MITER_LOOP = ' stroke-miterlimit="64"';

/**
 * A device-pixel coordinate as it lands on screen: the upstream snaps after adding the box's
 * origin, where f32 drops the last few ulps of a grid value (7.9999995 is 8 at any real position).
 */
const onScreen = (v) => Math.round(v * 1e4) / 1e4;

/** The vertices egui gives a circle of radius `r` points (its `CIRCLE_8` ... `CIRCLE_128`). */
function circleVertices(r) {
  return r <= 2 ? 8 : r <= 5 ? 16 : r < 18 ? 32 : r < 50 ? 64 : 128;
}

class P {
  /** `k` device pixels and `s` points per grid unit. */
  constructor(k, s) {
    this.k = k;
    this.s = s;
    /** The stroke width in device pixels, as `paint()` picks it. */
    this.wPx = Math.max(1, round(k * 0.9));
    this.out = [];
    this.grads = 0;
  }
  /** egui's `round_to_pixel_center`, in grid units. */
  centre(v) {
    return (round(onScreen(v * this.k) - 0.5) + 0.5) / this.k;
  }
  /** egui's `round_to_pixels`, in grid units. */
  edge(v) {
    return round(onScreen(v * this.k)) / this.k;
  }
  /** A stroke coordinate: on a pixel centre for odd widths, on an edge for even ones. */
  snapC(v) {
    return this.wPx % 2 === 1 ? this.centre(v) : this.edge(v);
  }
  /** A fill edge on a pixel edge. */
  snapE(v) {
    return this.edge(v);
  }
  sp(x, y) {
    return [this.snapC(x), this.snapC(y)];
  }
  /**
   * egui's tessellator rounds every straight segment that is exactly vertical or horizontal on
   * screen (`round_line_segments_to_pixels`): its centre like a stroke coordinate and each end,
   * pulled in a quarter pixel, onto a pixel centre. Grid coordinates a few f32 ulps apart collapse
   * at any screen position.
   */
  roundSegment(a, b) {
    a = [...a];
    b = [...b];
    const same = (u, v) => Math.abs(u - v) < 1e-5;
    const q = 0.25 / this.k;
    if (same(a[0], b[0])) {
      a[0] = b[0] = this.snapC(a[0]);
      const s = a[1] < b[1] ? q : -q;
      a[1] = this.centre(a[1] + s);
      b[1] = this.centre(b[1] - s);
    }
    if (same(a[1], b[1])) {
      a[1] = b[1] = this.snapC(a[1]);
      const s = a[0] < b[0] ? q : -q;
      a[0] = this.centre(a[0] + s);
      b[0] = this.centre(b[0] - s);
    }
    return [a, b];
  }
  /**
   * An open stroke: egui fades each end out over one pixel past its end point, which is a butt
   * end half a pixel further out.
   */
  open(pts) {
    const v = pts.map((p) => [...p]);
    const ext = 0.5 / this.k;
    const push = (end, from) => {
      const [dx, dy] = [end[0] - from[0], end[1] - from[1]];
      const len = Math.hypot(dx, dy);
      if (len > 0) {
        end[0] += (dx / len) * ext;
        end[1] += (dy / len) * ext;
      }
    };
    push(v[0], v[1]);
    push(v[v.length - 1], v[v.length - 2]);
    return `<path d="${d(v, false)}"/>`;
  }
  stroke(pts, closed) {
    this.out.push(closed ? `<path d="${d(pts, true)}"${MITER_LOOP}/>` : this.open(pts));
  }
  line(x0, y0, x1, y1) {
    [x0, y0, x1, y1] = [x0, y0, x1, y1].map(F);
    let a, b;
    if (y0 === y1) {
      const y = this.snapC(y0);
      a = [this.snapE(x0), y];
      b = [this.snapE(x1), y];
    } else if (x0 === x1) {
      const x = this.snapC(x0);
      a = [x, this.snapE(y0)];
      b = [x, this.snapE(y1)];
    } else {
      a = [x0, y0];
      b = [x1, y1];
    }
    this.stroke(this.roundSegment(a, b), false);
  }
  poly(pts, closed) {
    this.stroke(pts.map(([x, y]) => this.sp(F(x), F(y))), closed);
  }
  path(pts, closed) {
    this.stroke(pts, closed);
  }
  fill(pts) {
    const v = pts.map(([x, y]) => [F(x), F(y)]);
    const n = v.length;
    // Tested on screen positions, where points a few f32 ulps apart coincide.
    const q = v.map(([x, y]) => [onScreen(x * this.k), onScreen(y * this.k)]);
    const turn = (i) => {
      const [a, b, c] = [q[i], q[(i + 1) % n], q[(i + 2) % n]];
      return (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
    };
    const turns = v.map((_, i) => turn(i));
    const convex = n >= 3 && (turns.every((t) => t >= 0) || turns.every((t) => t <= 0));
    if (convex) {
      this.out.push(`<path d="${d(v, true)}" fill="currentColor" stroke="none"/>`);
      return;
    }
    // A mesh (aliased) and a hairline around it for the soft edge.
    this.out.push(`<path d="${d(v, true)}" fill="currentColor" stroke="none" shape-rendering="crispEdges"/>`);
    this.out.push(`<path d="${d(v, true)}"${MITER_LOOP} style="stroke-width:var(--design-icon-px,1.01)"/>`);
  }
  /** Triangles over (x, y, alpha) vertices, the alpha interpolated across each (an egui mesh). */
  /**
   * Triangles over (x, y, alpha) vertices, the alpha interpolated across each (an egui mesh). The
   * alpha field is drawn as a luminance mask over a fill of the icon's colour: each triangle is an
   * opaque grey linear gradient (exact for a linear interpolation), its outline stroked a little
   * wider with the same gradient so neighbours overlap; opaque overlaps leave no seams.
   */
  shade(verts, tris) {
    const defs = [];
    const shapes = [];
    const seam = num(0.4 / this.k);
    const outline = [];
    for (let t = 0; t < tris.length; t += 3) {
      const [p0, p1, p2] = [verts[tris[t]], verts[tris[t + 1]], verts[tris[t + 2]]];
      const [ux, uy, ua] = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
      const [vx, vy, va] = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
      const det = ux * vy - uy * vx;
      if (Math.abs(det) < 1e-9) continue;
      // Wound one way, so the outline's triangles union under the nonzero rule.
      const tri = det > 0 ? `M${pt(p0)}L${pt(p1)}L${pt(p2)}Z` : `M${pt(p0)}L${pt(p2)}L${pt(p1)}Z`;
      outline.push(tri);
      // alpha(p) = a0 + g . (p - p0)
      const gx = (ua * vy - va * uy) / det;
      const gy = (va * ux - ua * vx) / det;
      const g2 = gx * gx + gy * gy;
      let paint;
      if (g2 < 1e-12) {
        const v = Math.round(p0[2] * 255);
        paint = `rgb(${v},${v},${v})`;
      } else {
        const ax = p0[0] - (gx * p0[2]) / g2;
        const ay = p0[1] - (gy * p0[2]) / g2;
        const id = `{id}-${this.grads++}`;
        defs.push(
          `<linearGradient id="${id}" gradientUnits="userSpaceOnUse" x1="${num(ax)}" y1="${num(ay)}" x2="${num(ax + gx / g2)}" y2="${num(ay + gy / g2)}">` +
            `<stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient>`,
        );
        paint = `url(#${id})`;
      }
      shapes.push(`<path d="${tri}" fill="${paint}" stroke="${paint}" stroke-width="${seam}" stroke-linejoin="round"/>`);
    }
    const xs = verts.map((v) => v[0]);
    const ys = verts.map((v) => v[1]);
    const [x0, y0] = [Math.min(...xs) - 1, Math.min(...ys) - 1];
    const [w, h] = [Math.max(...xs) + 1 - x0, Math.max(...ys) + 1 - y0];
    const box = `x="${num(x0)}" y="${num(y0)}" width="${num(w)}" height="${num(h)}"`;
    const mask = `{id}-${this.grads++}`;
    // The mesh's own outline bounds it (the widened triangles stay inside).
    const clip = `{id}-${this.grads++}`;
    this.out.push(
      `<defs>${defs.join('')}<mask id="${mask}" maskUnits="userSpaceOnUse" ${box} style="mask-type:luminance;color-interpolation:sRGB">${shapes.join('')}</mask>` +
        `<clipPath id="${clip}"><path d="${outline.join('')}"/></clipPath></defs>`,
      `<rect ${box} fill="currentColor" stroke="none" mask="url(#${mask})" clip-path="url(#${clip})"/>`,
    );
  }
  shadeFan(r, centre, rim) {
    const SLICES = 32;
    const at = (t) => {
      const a = add(-FRAC_PI_2, mul(t, TAU));
      return [add(8, mul(r, cos(a))), add(8, mul(r, sin(a)))];
    };
    const verts = [];
    for (let i = 0; i < SLICES; i++) {
      const [t0, t1] = [F(i / SLICES), F((i + 1) / SLICES)];
      const [a, b] = [at(t0), at(t1)];
      verts.push([8, 8, centre((t0 + t1) / 2)], [a[0], a[1], rim(t0)], [b[0], b[1], rim(t1)]);
    }
    this.shade(verts, [...Array(SLICES * 3).keys()]);
  }
  /** egui's corner radius: whole points. */
  radius(r) {
    return round(r * this.s) / this.s;
  }
  rect(x0, y0, x1, y1, r) {
    const [a, b] = [this.sp(F(x0), F(y0)), this.sp(F(x1), F(y1))];
    this.out.push(this.rectEl(a, b, this.radius(r), MITER_LOOP));
  }
  fillRect(x0, y0, x1, y1, r) {
    const a = [this.snapE(F(x0)), this.snapE(F(y0))];
    const b = [this.snapE(F(x1)), this.snapE(F(y1))];
    this.out.push(this.rectEl(a, b, this.radius(r), ' fill="currentColor" stroke="none"'));
  }
  /** A rectangle, its rounded corners built from egui's precomputed circle vertices. */
  rectEl(a, b, r, extra) {
    const [w, h] = [b[0] - a[0], b[1] - a[1]];
    r = Math.min(r, w / 2, h / 2);
    if (r <= 0) return `<rect x="${num(a[0])}" y="${num(a[1])}" width="${num(w)}" height="${num(h)}"${extra}/>`;
    const quadrant = (cx, cy, q) => {
      const n = circleVertices(r * this.s) / 4;
      return range(n + 1).map((i) => {
        const t = ((q * n + i) / (4 * n)) * 2 * Math.PI;
        return [cx + r * Math.cos(t), cy + r * Math.sin(t)];
      });
    };
    const eps = 1e-6;
    const v = [...quadrant(b[0] - r, b[1] - r, 0)];
    if (w <= 2 * r + eps) v.pop();
    v.push(...quadrant(a[0] + r, b[1] - r, 1));
    if (h <= 2 * r + eps) v.pop();
    v.push(...quadrant(a[0] + r, a[1] + r, 2));
    if (w <= 2 * r + eps) v.pop();
    v.push(...quadrant(b[0] - r, a[1] + r, 3));
    if (h <= 2 * r + eps) v.pop();
    return `<path d="${d(v, true)}"${extra}/>`;
  }
  /**
   * A circle outline: egui's polygon for the radius, stroked outside it (the stroke's centre line
   * is half a stroke out).
   */
  circle(x, y, r) {
    const n = circleVertices(r * this.s);
    const ring = r + this.wPx / this.k / 2;
    const v = range(n).map((i) => [x + ring * Math.cos((i / n) * 2 * Math.PI), y + ring * Math.sin((i / n) * 2 * Math.PI)]);
    this.out.push(`<path d="${d(v, true)}"${MITER_LOOP}/>`);
  }
  dot(x, y, r) {
    this.out.push(`<circle cx="${num(x)}" cy="${num(y)}" r="${num(r)}" fill="currentColor" stroke="none"/>`);
  }
  arc(cx, cy, r, a0, a1) {
    this.path(arcPts(cx, cy, r, a0, a1), false);
  }
  head(tip, dir, len) {
    const arm = (a) => [add(tip[0], mul(len, cos(a))), add(tip[1], mul(len, sin(a)))];
    this.path([arm(sub(add(dir, PI), FRAC_PI_4)), tip, arm(add(add(dir, PI), FRAC_PI_4))], false);
  }
  arcArrow(cx, cy, r, a0, a1) {
    this.arc(cx, cy, r, a0, a1);
    const tip = [add(cx, mul(r, cos(a1))), add(cy, mul(r, sin(a1)))];
    const dir = add(a1, a1 > a0 ? FRAC_PI_2 : -FRAC_PI_2);
    this.head(tip, dir, 2.6);
  }
  corner(x, y, dx, dy, len, r) {
    const a0 = F(Math.atan2(0, -dx));
    let a1 = F(Math.atan2(-dy, 0));
    if (a1 - a0 > PI) a1 = sub(a1, TAU);
    else if (a0 - a1 > PI) a1 = add(a1, TAU);
    const c = [[x, y + len * dy], ...arcPts(x + r * dx, y + r * dy, r, a0, a1), [x + len * dx, y]];
    this.path(c, false);
  }
  dashedBox(x0, y0, x1, y1, n) {
    const seg = (a, b, i) => {
      const step = div(sub(b, a), 2 * n - 1);
      return [add(a, mul(2 * i, step)), add(a, mul(2 * i + 1, step))];
    };
    for (let i = 0; i < n; i++) {
      let [a, b] = seg(x0 - 0.5, x1 + 0.5, i);
      this.line(a, y0, b, y0);
      this.line(a, y1, b, y1);
      [a, b] = seg(y0 - 0.5, y1 + 0.5, i);
      this.line(x0, a, x0, b);
      this.line(x1, a, x1, b);
    }
  }
}

// ---------------------------------------------------------------------------------------------
// The drawing, transliterated from the upstream `draw()` arm by arm.

const range = (n) => [...Array(n).keys()];

/** @type {Record<string, (p: P) => void>} */
const DRAW = {
  Move: (p) => p.fill([[4.0, 2.0], [4.0, 13.6], [6.9, 10.9], [8.8, 14.8], [10.6, 14.0], [8.7, 10.2], [12.6, 10.2]]),
  Scale: (p) => {
    p.rect(2.5, 7.5, 8.5, 13.5, 0);
    p.line(6.0, 10.0, 13.0, 3.0);
    p.poly([[9.5, 2.5], [13.5, 2.5], [13.5, 6.5]], false);
  },
  Hand: (p) => {
    for (const [x, top] of [[4.5, 5.0], [6.5, 3.5], [8.5, 4.0], [10.5, 5.5]]) {
      p.path([[x, 8.0], ...arcPts(x + 1, top, 1, PI, TAU), [x + 2, 8.0]], false);
    }
    const h = [[12.5, 8.0], [12.5, 10.0], ...arcPts(9.5, 10.0, 3.0, 0, FRAC_PI_2), [8.0, 13.0], [3.0, 9.4]];
    h.push(...arcPts(3.6, 8.6, 1.0, 2.2, 5.3), [4.5, 8.5]);
    p.path(h, false);
  },
  Frame: (p) => {
    p.line(5.5, 3.0, 5.5, 13.0);
    p.line(10.5, 3.0, 10.5, 13.0);
    p.line(3.0, 5.5, 13.0, 5.5);
    p.line(3.0, 10.5, 13.0, 10.5);
  },
  Section: (p) => {
    p.rect(2.5, 5.5, 13.5, 13.5, 1);
    p.fillRect(2.0, 2.0, 9.0, 6.0, 1);
  },
  Slice: (p) => {
    p.poly([[13.5, 2.5], [5.0, 8.0], [8.0, 11.0]], true);
    p.fill(capsule([2.8, 13.2], [5.9, 10.1], 1.3));
  },
  Rectangle: (p) => p.rect(2.5, 2.5, 13.5, 13.5, 1),
  Line: (p) => p.line(2.5, 13.5, 13.5, 2.5),
  Arrow: (p) => {
    p.line(2.5, 13.5, 13.0, 3.0);
    p.poly([[7.5, 2.5], [13.5, 2.5], [13.5, 8.5]], false);
  },
  Ellipse: (p) => p.circle(8.0, 8.0, 5.5),
  Polygon: (p) => p.poly([[8.0, 2.5], [13.5, 12.5], [2.5, 12.5]], true),
  Star: (p) => {
    const pts = range(10).map((i) => {
      const a = add(-FRAC_PI_2, div(mul(PI, i), 5));
      const r = i % 2 === 0 ? 6.0 : 2.7;
      return [add(8, mul(r, cos(a))), add(8.6, mul(r, sin(a)))];
    });
    p.path(pts, true);
  },
  Image: (p) => {
    p.rect(2.5, 3.5, 13.5, 12.5, 1);
    p.poly([[2.5, 11.5], [6.0, 8.0], [9.0, 11.0], [10.5, 9.5], [13.5, 12.5]], false);
    p.dot(10.5, 6.5, 1.1);
  },
  Pen: (p) => {
    p.poly([[8.0, 2.0], [12.5, 8.5], [10.5, 13.5], [5.5, 13.5], [3.5, 8.5]], true);
    p.line(8.0, 2.0, 8.0, 7.0);
    p.dot(8.0, 8.8, 1.2);
  },
  Pencil: (p) => {
    p.poly([[2.5, 13.5], [3.2, 10.3], [10.5, 3.0], [13.0, 5.5], [5.7, 12.8]], true);
    p.line(9.0, 4.5, 11.5, 7.0);
  },
  Text: (p) => {
    p.line(3.0, 3.5, 12.0, 3.5);
    p.line(7.5, 3.0, 7.5, 12.5);
    p.line(3.5, 3.0, 3.5, 5.5);
    p.line(11.5, 3.0, 11.5, 5.5);
    p.line(5.5, 12.5, 9.5, 12.5);
  },
  TextOnPath: (p) => {
    p.line(4.5, 3.0, 11.5, 3.0);
    p.line(8.0, 3.0, 8.0, 8.5);
    p.poly([[2.0, 13.5], [5.0, 11.5], [8.0, 10.8], [11.0, 11.5], [14.0, 13.5]], false);
  },
  Comment: (p) => {
    const b = arcPts(8.5, 7.5, 5.0, 2.75, add(TAU, 1.95));
    b.push([2.5, 13.5]);
    p.path(b, true);
  },
  Actions: (p) => p.poly([[9.0, 2.0], [3.5, 9.0], [7.5, 9.0], [6.5, 14.0], [12.5, 7.0], [8.5, 7.0]], true),
  Chevron: (p) => p.poly([[4.5, 6.0], [8.0, 9.5], [11.5, 6.0]], false),
  ChevronRight: (p) => p.poly([[6.0, 4.5], [9.5, 8.0], [6.0, 11.5]], false),
  ChevronLeft: (p) => p.poly([[10.0, 4.5], [6.5, 8.0], [10.0, 11.5]], false),
  ChevronUp: (p) => p.poly([[4.5, 10.0], [8.0, 6.5], [11.5, 10.0]], false),
  Plus: (p) => {
    p.line(7.5, 2.0, 7.5, 13.0);
    p.line(2.0, 7.5, 13.0, 7.5);
  },
  Minus: (p) => p.line(2.0, 7.5, 13.0, 7.5),
  Eye: (p) => eye(p, true),
  EyeOff: (p) => eye(p, false),
  Lock: (p) => lock(p, true),
  Unlock: (p) => lock(p, false),
  Component: (p) => {
    for (const [cx, cy] of [[8.0, 4.2], [11.8, 8.0], [8.0, 11.8], [4.2, 8.0]]) {
      p.fill([[cx, cy - 2.5], [cx + 2.5, cy], [cx, cy + 2.5], [cx - 2.5, cy]]);
    }
  },
  Instance: (p) => p.path([[8.0, 2.0], [14.0, 8.0], [8.0, 14.0], [2.0, 8.0]], true),
  ComponentSet: (p) => {
    for (const [cx, cy] of [[8.0, 4.3], [11.7, 8.0], [8.0, 11.7], [4.3, 8.0]]) {
      p.path([[cx, F(cy - 2.2)], [F(cx + 2.2), cy], [cx, F(cy + 2.2)], [F(cx - 2.2), cy]], true);
    }
  },
  Group: (p) => {
    for (const [x, y, dx, dy] of [[2.5, 2.5, 1, 1], [13.5, 2.5, -1, 1], [2.5, 13.5, 1, -1], [13.5, 13.5, -1, -1]]) {
      p.poly([[x + 3.5 * dx, y], [x, y], [x, y + 3.5 * dy]], false);
    }
  },
  Vector: (p) => {
    p.path([[3.0, 12.5], [6.0, 4.0], [10.0, 12.0], [13.0, 3.5]], false);
    p.fillRect(1.5, 11.0, 4.5, 14.0, 0);
    p.fillRect(11.5, 2.0, 14.5, 5.0, 0);
  },
  Boolean: (p) => booleanUnion(p),
  Page: (p) => {
    p.poly([[3.5, 2.5], [9.5, 2.5], [12.5, 5.5], [12.5, 13.5], [3.5, 13.5]], true);
    p.poly([[9.5, 2.5], [9.5, 5.5], [12.5, 5.5]], false);
  },
  Check: (p) => p.poly([[3.0, 8.5], [6.5, 12.0], [13.0, 4.5]], false),
  Close: (p) => {
    p.line(4.0, 4.0, 12.0, 12.0);
    p.line(12.0, 4.0, 4.0, 12.0);
  },
  WinMinimize: (p) => p.line(3.0, 8.5, 13.0, 8.5),
  WinMaximize: (p) => p.rect(3.5, 3.5, 12.5, 12.5, 0),
  WinRestore: (p) => {
    p.rect(3.5, 5.5, 10.5, 12.5, 0);
    p.poly([[5.5, 5.5], [5.5, 3.5], [12.5, 3.5], [12.5, 10.5], [10.5, 10.5]], false);
  },
  Search: (p) => {
    p.circle(7.0, 7.0, 4.5);
    p.line(10.5, 10.5, 14.0, 14.0);
  },
  Grid: (p) => grid(p),
  AlignLeft: (p) => {
    p.line(2.5, 2.0, 2.5, 14.0);
    p.fillRect(4.0, 4.0, 13.0, 7.0, 0);
    p.fillRect(4.0, 9.0, 9.0, 12.0, 0);
  },
  AlignHCenter: (p) => {
    p.line(7.5, 2.0, 7.5, 14.0);
    p.fillRect(2.0, 4.0, 13.0, 7.0, 0);
    p.fillRect(4.0, 9.0, 11.0, 12.0, 0);
  },
  AlignRight: (p) => {
    p.line(13.5, 2.0, 13.5, 14.0);
    p.fillRect(3.0, 4.0, 12.0, 7.0, 0);
    p.fillRect(7.0, 9.0, 12.0, 12.0, 0);
  },
  AlignTop: (p) => {
    p.line(2.0, 2.5, 14.0, 2.5);
    p.fillRect(4.0, 4.0, 7.0, 13.0, 0);
    p.fillRect(9.0, 4.0, 12.0, 9.0, 0);
  },
  AlignVCenter: (p) => {
    p.line(2.0, 7.5, 14.0, 7.5);
    p.fillRect(4.0, 2.0, 7.0, 13.0, 0);
    p.fillRect(9.0, 4.0, 12.0, 11.0, 0);
  },
  AlignBottom: (p) => {
    p.line(2.0, 13.5, 14.0, 13.5);
    p.fillRect(4.0, 3.0, 7.0, 12.0, 0);
    p.fillRect(9.0, 7.0, 12.0, 12.0, 0);
  },
  DistributeH: (p) => distributeH(p),
  DistributeV: (p) => {
    p.line(2.0, 2.5, 14.0, 2.5);
    p.line(2.0, 13.5, 14.0, 13.5);
    p.fillRect(4.0, 6.0, 12.0, 10.0, 0);
  },
  Rotate: (p) => p.arcArrow(8.0, 8.5, 5.0, mul(-PI, 0.55), mul(PI, 1.25)),
  FlipH: (p) => {
    for (const y of [1.0, 5.0, 9.0, 13.0]) p.line(7.5, y, 7.5, y + 2);
    p.poly([[5.5, 3.5], [5.5, 12.5], [1.5, 12.5]], true);
    p.fill([[9.0, 3.0], [14.0, 13.0], [9.0, 13.0]]);
  },
  FlipV: (p) => {
    for (const x of [1.0, 5.0, 9.0, 13.0]) p.line(x, 7.5, x + 2, 7.5);
    p.poly([[3.5, 5.5], [12.5, 5.5], [12.5, 1.5]], true);
    p.fill([[3.0, 9.0], [13.0, 9.0], [13.0, 14.0]]);
  },
  CornerRadius: (p) => p.corner(2.5, 2.5, 1, 1, 11.5, 6.0),
  CornerTopLeft: (p) => p.corner(2.5, 2.5, 1, 1, 11.5, 6.0),
  CornerTopRight: (p) => p.corner(13.5, 2.5, -1, 1, 11.5, 6.0),
  CornerBottomRight: (p) => p.corner(13.5, 13.5, -1, -1, 11.5, 6.0),
  CornerBottomLeft: (p) => p.corner(2.5, 13.5, 1, -1, 11.5, 6.0),
  Angle: (p) => {
    p.path([[13.5, 13.0], [2.5, 13.0], [10.5, 3.0]], false);
    p.path(arcPts(2.5, 13.0, 6.0, F(Math.atan2(-10, 8)), 0), false);
  },
  IndependentCorners: (p) => {
    for (const [x, y, dx, dy] of [[2.5, 2.5, 1, 1], [13.5, 2.5, -1, 1], [2.5, 13.5, 1, -1], [13.5, 13.5, -1, -1]]) {
      p.corner(x, y, dx, dy, 4.0, 2.0);
    }
  },
  IndependentPadding: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 2);
    p.line(8.0, 2.5, 8.0, 5.0);
    p.line(13.5, 8.0, 11.0, 8.0);
    p.line(8.0, 13.5, 8.0, 11.0);
    p.line(2.5, 8.0, 5.0, 8.0);
  },
  LinkOn: (p) => link(p),
  LinkOff: (p) => {
    p.path(capsule([3.5, 12.5], [5.5, 10.5], 2.2), true);
    p.path(capsule([10.5, 5.5], [12.5, 3.5], 2.2), true);
    p.line(9.5, 12.0, 9.5, 14.0);
    p.line(12.0, 9.5, 14.0, 9.5);
    p.line(2.0, 6.5, 4.0, 6.5);
    p.line(6.5, 2.0, 6.5, 4.0);
  },
  LayoutH: (p) => {
    p.fillRect(2.0, 3.0, 7.0, 13.0, 1);
    p.fillRect(9.0, 3.0, 14.0, 13.0, 1);
  },
  LayoutV: (p) => {
    p.fillRect(3.0, 2.0, 13.0, 7.0, 1);
    p.fillRect(3.0, 9.0, 13.0, 14.0, 1);
  },
  LayoutWrap: (p) => {
    p.path([[2.0, 4.5], [10.5, 4.5], ...arcPts(10.5, 7.5, 3.0, -FRAC_PI_2, FRAC_PI_2), [4.0, 10.5]], false);
    p.head([3.5, 10.5], PI, 2.8);
  },
  LayoutGrid: (p) => grid(p),
  AutoLayout: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 2);
    p.fillRect(5.0, 5.0, 7.0, 11.0, 0);
    p.fillRect(9.0, 5.0, 11.0, 11.0, 0);
  },
  ClipContent: (p) => {
    p.rect(4.5, 4.5, 13.5, 13.5, 0);
    p.line(2.5, 1.0, 2.5, 11.0);
    p.line(1.0, 2.5, 11.0, 2.5);
  },
  Effects: (p) => rays(p, 3.0),
  Styles: (p) => {
    for (const [x, y] of [[5.0, 5.0], [11.0, 5.0], [5.0, 11.0], [11.0, 11.0]]) p.dot(x, y, 2.2);
  },
  Menu: (p) => {
    for (const y of [3.5, 7.5, 11.5]) p.line(2.0, y, 14.0, y);
  },
  Play: (p) => p.fill([[4.5, 2.5], [13.5, 8.0], [4.5, 13.5]]),
  Home: (p) => {
    p.path([[6.5, 13.5], [3.5, 13.5], [3.5, 7.0], [8.0, 2.8], [12.5, 7.0], [12.5, 13.5], [9.5, 13.5]].map(([x, y]) => [x, F(y)]), false);
    p.poly([[6.5, 13.5], [6.5, 9.5], [9.5, 9.5], [9.5, 13.5]], false);
  },
  Bell: (p) => {
    p.path([[2.5, 11.5], [4.0, 10.0], [4.0, 7.0], ...arcPts(8.0, 7.0, 4.0, PI, TAU), [12.0, 10.0], [13.5, 11.5]], true);
    p.arc(8.0, 12.0, 1.8, 0, PI);
  },
  Panels: (p) => {
    p.rect(2.5, 3.5, 13.5, 12.5, 1);
    p.line(5.5, 4.0, 5.5, 12.0);
    p.line(10.5, 4.0, 10.5, 12.0);
  },
  Help: (p) => {
    p.circle(8.0, 8.0, 6.0);
    p.arc(8.0, 6.4, 2.0, mul(PI, 1.05), add(TAU, 0.7));
    p.path([[9.5, 7.7], [8.0, 8.8], [8.0, 9.8]], false);
    p.dot(8.0, 11.6, 0.9);
  },
  Dots: (p) => {
    for (const x of [3.5, 8.0, 12.5]) p.dot(x, 8.0, 1.25);
  },
  Mask: (p) => halfDisc(p, -FRAC_PI_2),
  TextAlignLeft: (p) => {
    for (const [y, x1] of [[3.5, 14.0], [6.5, 10.0], [9.5, 14.0], [12.5, 9.0]]) p.line(2.0, y, x1, y);
  },
  TextAlignCenter: (p) => {
    for (const [y, h] of [[3.5, 6.0], [6.5, 4.0], [9.5, 6.0], [12.5, 3.0]]) p.line(8 - h, y, 8 + h, y);
  },
  TextAlignRight: (p) => {
    for (const [y, x0] of [[3.5, 2.0], [6.5, 6.0], [9.5, 2.0], [12.5, 7.0]]) p.line(x0, y, 14.0, y);
  },
  TextAlignJustify: (p) => {
    for (const y of [3.5, 6.5, 9.5, 12.5]) p.line(2.0, y, 14.0, y);
  },
  TextTop: (p) => {
    p.line(2.0, 2.5, 14.0, 2.5);
    p.line(7.5, 5.0, 7.5, 14.0);
    p.head([7.5, 5.0], -FRAC_PI_2, 3.2);
  },
  TextMiddle: (p) => {
    p.line(2.0, 7.5, 14.0, 7.5);
    p.line(7.5, 1.0, 7.5, 5.0);
    p.head([7.5, 5.0], FRAC_PI_2, 2.6);
    p.line(7.5, 10.0, 7.5, 14.0);
    p.head([7.5, 10.0], -FRAC_PI_2, 2.6);
  },
  TextBottom: (p) => {
    p.line(2.0, 13.5, 14.0, 13.5);
    p.line(7.5, 2.0, 7.5, 11.0);
    p.head([7.5, 11.0], FRAC_PI_2, 3.2);
  },
  Eyedropper: (p) => {
    p.poly([[2.5, 13.5], [3.0, 11.0], [8.5, 5.5], [10.5, 7.5], [5.0, 13.0]], true);
    p.path(capsule([10.5, 5.5], [12.0, 4.0], 1.6), true);
    p.line(7.5, 4.5, 11.5, 8.5);
  },
  Code: (p) => {
    p.poly([[5.5, 4.0], [2.0, 8.0], [5.5, 12.0]], false);
    p.poly([[10.5, 4.0], [14.0, 8.0], [10.5, 12.0]], false);
    p.line(9.0, 3.0, 7.0, 13.0);
  },
  Assets: (p) => {
    p.path([[5.0, 2.0], [8.0, 5.0], [5.0, 8.0], [2.0, 5.0]], true);
    p.circle(11.0, 5.0, 2.5);
    p.rect(2.5, 9.5, 7.5, 13.5, 0);
    p.poly([[11.0, 9.0], [13.5, 13.5], [8.5, 13.5]], true);
  },
  Library: (p) => {
    p.poly([[7.5, 4.5], [5.5, 3.5], [1.5, 3.5], [1.5, 12.5], [5.5, 12.5], [7.5, 13.5]], false);
    p.poly([[8.5, 4.5], [10.5, 3.5], [14.5, 3.5], [14.5, 12.5], [10.5, 12.5], [8.5, 13.5]], false);
    p.line(7.5, 4.5, 7.5, 13.5);
    p.line(8.5, 4.5, 8.5, 13.5);
  },
  Layers: (p) => {
    p.poly([[8.0, 2.5], [14.0, 5.5], [8.0, 8.5], [2.0, 5.5]], true);
    p.poly([[2.0, 8.0], [8.0, 11.0], [14.0, 8.0]], false);
    p.poly([[2.0, 10.5], [8.0, 13.5], [14.0, 10.5]], false);
  },
  Opacity: (p) => halfDisc(p, FRAC_PI_2),
  Blend: (p) => {
    p.circle(6.0, 8.0, 4.0);
    p.circle(10.0, 8.0, 4.0);
  },
  Export: (p) => {
    p.line(7.5, 2.0, 7.5, 10.0);
    p.head([7.5, 2.0], -FRAC_PI_2, 3.5);
    p.poly([[2.5, 8.5], [2.5, 13.5], [12.5, 13.5], [12.5, 8.5]], false);
  },
  StrokeIcon: (p) => p.rect(2.5, 2.5, 13.5, 13.5, 2),
  Fill: (p) => p.fillRect(2.0, 2.0, 14.0, 14.0, 2),
  GradientLinear: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 2);
    p.shade([[4, 4, 1], [12, 4, 0], [12, 12, 0], [4, 12, 1]], [0, 1, 2, 0, 2, 3]);
  },
  GradientRadial: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 2);
    p.shadeFan(5.5, () => 1, () => 0);
  },
  GradientAngular: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 2);
    p.shadeFan(4.0, (t) => 1 - t, (t) => 1 - t);
  },
  GradientDiamond: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 2);
    const v = [[8, 8, 1], [8, 2.5, 0], [13.5, 8, 0], [8, 13.5, 0], [2.5, 8, 0]];
    p.shade(v, [0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 1]);
  },
  Sun: (p) => rays(p, 2.5),
  Moon: (p) => {
    const m = arcPts(8.0, 8.0, 5.5, add(0.166, 0.12), sub(sub(TAU, 1.737), 0.12));
    m.push(...arcPts(11.0, 5.0, 4.6, sub(-2.586, 0.18), add(sub(1.016, TAU), 0.18)));
    p.path(m, true);
  },
  Undo: (p) => undo(p, true),
  Redo: (p) => undo(p, false),
  Union: (p) => booleanUnion(p),
  Subtract: (p) => {
    p.fillRect(2.0, 2.0, 10.0, 10.0, 0);
    p.rect(6.5, 6.5, 13.5, 13.5, 0);
  },
  Intersect: (p) => {
    p.rect(2.5, 2.5, 9.5, 9.5, 0);
    p.rect(6.5, 6.5, 13.5, 13.5, 0);
    p.fillRect(6.0, 6.0, 10.0, 10.0, 0);
  },
  Exclude: (p) => {
    p.fillRect(2.0, 2.0, 10.0, 6.0, 0);
    p.fillRect(2.0, 6.0, 6.0, 10.0, 0);
    p.fillRect(10.0, 6.0, 14.0, 14.0, 0);
    p.fillRect(6.0, 10.0, 10.0, 14.0, 0);
    p.rect(6.5, 6.5, 9.5, 9.5, 0);
  },
  Hug: (p) => {
    p.poly([[2.5, 4.5], [5.5, 7.5], [2.5, 10.5]], false);
    p.poly([[13.5, 4.5], [10.5, 7.5], [13.5, 10.5]], false);
    p.line(6.0, 7.5, 10.0, 7.5);
  },
  FillContainer: (p) => {
    p.poly([[5.0, 4.5], [2.0, 7.5], [5.0, 10.5]], false);
    p.poly([[11.0, 4.5], [14.0, 7.5], [11.0, 10.5]], false);
    p.line(2.0, 7.5, 14.0, 7.5);
  },
  Fixed: (p) => {
    p.line(2.5, 4.0, 2.5, 11.0);
    p.line(13.5, 4.0, 13.5, 11.0);
    p.line(3.0, 7.5, 13.0, 7.5);
  },
  PaddingH: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 1);
    p.fillRect(2.5, 2.5, 5.5, 13.5, 1);
    p.fillRect(10.5, 2.5, 13.5, 13.5, 1);
  },
  PaddingV: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 1);
    p.fillRect(2.5, 2.5, 13.5, 5.5, 1);
    p.fillRect(2.5, 10.5, 13.5, 13.5, 1);
  },
  PaddingTop: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 1);
    p.fillRect(2.5, 2.5, 13.5, 5.5, 1);
  },
  PaddingRight: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 1);
    p.fillRect(10.5, 2.5, 13.5, 13.5, 1);
  },
  PaddingBottom: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 1);
    p.fillRect(2.5, 10.5, 13.5, 13.5, 1);
  },
  PaddingLeft: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 1);
    p.fillRect(2.5, 2.5, 5.5, 13.5, 1);
  },
  Gap: (p) => distributeH(p),
  Reset: (p) => p.arcArrow(8.0, 8.5, 5.0, mul(-PI, 0.45), mul(-PI, 2.25)),
  Detach: (p) => {
    p.path([[8.0, 2.0], [14.0, 8.0], [8.0, 14.0], [2.0, 8.0]], true);
    p.line(3.0, 13.0, 13.0, 3.0);
  },
  Variable: (p) => {
    const pts = range(6).map((i) => {
      const a = add(mul(FRAC_PI_3, i), FRAC_PI_6);
      return [add(8, mul(6, cos(a))), add(8, mul(6, sin(a)))];
    });
    p.path(pts, true);
    p.dot(8.0, 8.0, 1.7);
  },
  Clock: (p) => {
    p.circle(7.5, 7.5, 5.5);
    p.line(7.5, 4.0, 7.5, 8.0);
    p.line(7.5, 7.5, 10.0, 10.0);
  },
  Eraser: (p) => {
    p.poly([[2.5, 10.0], [9.5, 3.0], [13.5, 7.0], [7.0, 13.5], [6.0, 13.5]], true);
    p.line(5.5, 7.0, 9.5, 11.0);
    p.line(7.0, 13.5, 14.0, 13.5);
  },
  VariableWidth: (p) => {
    const edge = (i, side) => {
      const f = div(i, 12);
      return [add(2, mul(11, f)), add(7.5, mul(mul(side, 3), sin(mul(PI, f))))];
    };
    const lens = [...range(13).map((i) => edge(i, -1)), ...range(13).reverse().map((i) => edge(i, 1))];
    p.fill(lens);
    p.line(7.5, 2.0, 7.5, 13.0);
    p.dot(7.5, 2.0, 1.5);
    p.dot(7.5, 13.0, 1.5);
  },
  ShapeBuilder: (p) => shapeBuilder(p, true),
  ShapeBuilderRemove: (p) => shapeBuilder(p, false),
  Swap: (p) => {
    p.line(2.0, 4.5, 13.0, 4.5);
    p.head([13.0, 4.5], 0, 2.8);
    p.line(3.0, 11.5, 14.0, 11.5);
    p.head([3.0, 11.5], PI, 2.8);
  },
  Link: (p) => link(p),
  Prototype: (p) => {
    p.circle(4.5, 11.5, 2.0);
    p.line(6.0, 10.0, 13.0, 3.0);
    p.poly([[8.5, 2.5], [13.5, 2.5], [13.5, 7.5]], false);
  },
  Preview: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 1);
    p.line(3.0, 5.5, 13.0, 5.5);
    p.fill([[6.5, 7.5], [10.5, 10.0], [6.5, 12.5]]);
  },
  Section2: (p) => {
    p.rect(2.5, 5.5, 13.5, 13.5, 1);
    p.fillRect(2.0, 2.0, 9.0, 6.0, 1);
  },
  Logo: (p) => {
    const tail = [[13.75, 13.0]];
    tail.push(...arcPts(7.25, 8.0, 5.0, FRAC_PI_2, mul(3, FRAC_PI_2)));
    tail.push(...arcPts(7.25, 6.5, 3.5, mul(3, FRAC_PI_2), mul(5, FRAC_PI_2)));
    tail.push(...arcPts(7.25, 8.25, 1.75, FRAC_PI_2, mul(3, FRAC_PI_2)));
    // One bold unhinted stroke, 1.7 units at every size.
    p.out.push(p.open(tail).replace('/>', ' stroke-width="1.7"/>'));
    p.dot(13.75, 13.0, 0.85);
    p.dot(7.25, 6.5, 0.85);
  },
  AbsolutePosition: (p) => {
    p.rect(2.5, 2.5, 13.5, 13.5, 1);
    p.fillRect(6.0, 6.0, 10.0, 10.0, 0);
    p.line(10.0, 7.5, 13.0, 7.5);
    p.line(7.5, 3.0, 7.5, 6.0);
  },
  Slot: (p) => {
    p.dashedBox(2.5, 2.5, 12.5, 12.5, 3);
    p.line(7.5, 5.0, 7.5, 10.0);
    p.line(5.0, 7.5, 10.0, 7.5);
  },
  Adjust: (p) => {
    p.line(2.0, 4.5, 14.0, 4.5);
    p.line(2.0, 11.5, 14.0, 11.5);
    p.dot(10.5, 4.5, 2.2);
    p.dot(5.5, 11.5, 2.2);
  },
  ViewList: (p) => {
    for (const y of [2.0, 6.0, 10.0]) {
      p.fillRect(2.0, y, 5.0, y + 3, 0);
      p.line(7.0, y + 1.5, 14.0, y + 1.5);
    }
  },
  ViewGrid: (p) => {
    for (const [x, y] of [[2.5, 2.5], [9.5, 2.5], [2.5, 9.5], [9.5, 9.5]]) p.rect(x, y, x + 4, y + 4, 1);
  },
  Reaction: (p) => {
    p.circle(8.0, 8.0, 5.5);
    p.dot(6.0, 6.5, 0.9);
    p.dot(10.0, 6.5, 0.9);
    p.arc(8.0, 8.5, 3.0, 0.6, sub(PI, 0.6));
  },
  DragHandle: (p) => {
    for (const y of [4.0, 8.0, 12.0]) {
      p.dot(6.0, y, 1.1);
      p.dot(10.0, y, 1.1);
    }
  },
  ArcStart: (p) => {
    p.circle(8.0, 8.0, 5.5);
    p.line(8.0, 8.0, 13.5, 8.0);
    p.dot(13.5, 8.0, 1.6);
  },
  ArcSweep: (p) => {
    p.path([[8.0, 8.0], ...arcPts(8.0, 8.0, 5.5, 0, mul(1.5, PI))], true);
  },
  ArcRatio: (p) => {
    p.circle(8.0, 8.0, 5.5);
    p.circle(8.0, 8.0, 2.5);
  },
  GamutWarning: (p) => {
    p.poly([[8.0, 2.0], [14.5, 13.5], [1.5, 13.5]], true);
    p.line(8.0, 6.0, 8.0, 9.5);
    p.dot(8.0, 11.5, 0.9);
  },
  Online: (p) => {
    p.circle(8.0, 8.0, 5.5);
    p.line(2.5, 8.0, 13.5, 8.0);
    p.path(arcPts(0, 0, 1, 0, TAU).map(([x, y]) => [add(8, mul(2.4, x)), add(8, mul(5.5, y))]), true);
  },
  LiveShared: (p) => {
    p.dot(8.0, 8.0, 1.6);
    for (const r of [3.6, 6.0]) {
      p.arc(8.0, 8.0, r, -FRAC_PI_4, FRAC_PI_4);
      p.arc(8.0, 8.0, r, sub(PI, FRAC_PI_4), add(PI, FRAC_PI_4));
    }
  },
  LivePrivate: (p) => {
    p.dot(8.0, 8.0, 1.6);
    p.circle(8.0, 8.0, 5.0);
  },
  PenNib: (p) => {
    const [s, c] = [0.61, 0.79];
    const at = (x, y) => [add(8, sub(mul(x, c), mul(y, s))), add(8, add(mul(x, s), mul(y, c)))];
    p.path([[0.0, 6.5], [3.4, 1.0], [2.4, -2.5], [-2.4, -2.5], [-3.4, 1.0]].map(([x, y]) => at(x, y)), true);
    p.path([at(0.0, 6.5), at(0.0, 2.0)], false);
    const [dx, dy] = at(0.0, 0.6);
    p.dot(dx, dy, 1.0);
    p.fill([[-2.4, -3.4], [2.4, -3.4], [2.4, -5.6], [-2.4, -5.6]].map(([x, y]) => at(x, y)));
  },
  Board: (p) => {
    p.poly([[2.5, 2.5], [13.5, 2.5], [13.5, 9.5], [9.5, 13.5], [2.5, 13.5]], true);
    p.fill([[9.5, 13.5], [9.5, 9.5], [13.5, 9.5]]);
    p.line(5.0, 6.5, 11.0, 6.5);
    p.line(5.0, 9.5, 8.0, 9.5);
  },
  Templates: (p) => {
    p.rect(1.5, 2.5, 14.5, 13.5, 1.5);
    p.fillRect(3.5, 4.5, 7.5, 8.5, 0.5);
    p.line(9.5, 5.5, 12.5, 5.5);
    p.line(9.5, 7.5, 12.5, 7.5);
    p.line(3.5, 11.0, 12.5, 11.0);
  },
  Table: (p) => {
    p.rect(1.5, 2.5, 14.5, 13.5, 1);
    p.line(1.5, 6.5, 14.5, 6.5);
    p.line(1.5, 10.0, 14.5, 10.0);
    p.line(6.0, 2.5, 6.0, 13.5);
    p.line(10.0, 2.5, 10.0, 13.5);
  },
  AddRow: (p) => row(p, true),
  RemoveRow: (p) => row(p, false),
  AddColumn: (p) => column(p, true),
  RemoveColumn: (p) => column(p, false),
  Marker: (p) => {
    p.poly([[3.0, 13.0], [3.0, 10.5], [10.0, 3.5], [12.5, 6.0], [5.5, 13.0]], true);
    p.line(8.5, 5.0, 11.0, 7.5);
    p.fill([[2.0, 14.0], [2.0, 11.5], [4.5, 14.0]]);
  },
  Highlighter: (p) => {
    p.poly([[4.0, 10.0], [10.5, 3.5], [13.5, 6.5], [7.0, 13.0]], true);
    p.poly([[4.0, 10.0], [2.5, 12.5], [4.5, 12.5], [7.0, 13.0]], false);
    p.fillRect(1.0, 14.0, 15.0, 15.5, 0);
  },
  Connector: (p) => {
    p.rect(1.5, 1.5, 6.5, 6.5, 1);
    p.rect(9.5, 9.5, 14.5, 14.5, 1);
    p.poly([[6.5, 4.0], [12.0, 4.0], [12.0, 9.0]], false);
    p.head([12.0, 9.0], FRAC_PI_2, 2.4);
  },
  CurvedConnector: (p) => {
    p.rect(1.5, 1.5, 6.5, 6.5, 1);
    p.rect(9.5, 9.5, 14.5, 14.5, 1);
    p.arc(6.5, 9.0, 5.5, -FRAC_PI_2, 0);
    p.head([12.0, 9.0], FRAC_PI_2, 2.4);
  },
  Shapes: (p) => {
    p.rect(1.5, 5.5, 9.5, 13.5, 1);
    p.circle(10.5, 5.5, 4.0);
  },
  Stamp: (p) => {
    p.circle(8.0, 8.0, 6.0);
    p.poly([[5.0, 8.3], [7.2, 10.5], [11.0, 6.0]], false);
  },
  Timer: (p) => {
    p.circle(8.0, 9.0, 5.5);
    p.line(8.0, 9.0, 8.0, 6.0);
    p.line(6.0, 1.5, 10.0, 1.5);
    p.line(8.0, 1.5, 8.0, 3.5);
    p.line(12.0, 4.0, 13.5, 2.5);
  },
  Pause: (p) => {
    p.fillRect(4.0, 3.0, 6.5, 13.0, 0.5);
    p.fillRect(9.5, 3.0, 12.0, 13.0, 0.5);
  },
  Vote: (p) => {
    p.rect(2.5, 8.5, 13.5, 13.5, 1);
    p.poly([[5.5, 8.5], [5.5, 2.5], [10.5, 2.5], [10.5, 8.5]], false);
    p.poly([[6.6, 5.6], [7.7, 6.7], [9.4, 4.6]], false);
  },
  Measure: (p) => {
    p.line(2.5, 4.0, 2.5, 12.0);
    p.line(13.5, 4.0, 13.5, 12.0);
    p.line(3.0, 8.0, 13.0, 8.0);
    p.poly([[5.5, 5.5], [3.0, 8.0], [5.5, 10.5]], false);
    p.poly([[10.5, 5.5], [13.0, 8.0], [10.5, 10.5]], false);
  },
};

// More arms (the output follows the upstream enum's order, not this table's).
Object.assign(DRAW, {
  Bend: (p) => {
    const pts = range(17).map((i) => {
      const t = F(i / 16);
      const u = sub(1, t);
      return [
        add(add(mul(mul(u, u), 2.5), mul(mul(mul(2, u), t), 8)), mul(mul(t, t), 13.5)),
        add(add(mul(mul(u, u), 13.5), mul(mul(mul(2, u), t), 1.5)), mul(mul(t, t), 13.5)),
      ];
    });
    p.path(pts, false);
    p.line(3.0, 7.5, 13.0, 7.5);
    p.dot(3.0, 7.5, 1.3);
    p.dot(13.0, 7.5, 1.3);
    p.fillRect(7.0, 6.0, 9.0, 9.0, 0);
  },
  PaintBucket: (p) => {
    p.poly([[2.5, 8.0], [7.5, 3.0], [12.5, 8.0], [7.5, 13.0]], true);
    p.fill([[2.5, 8.0], [12.5, 8.0], [7.5, 13.0]]);
    p.line(7.5, 3.0, 5.0, 0.5);
    p.dot(13.5, 11.5, 1.5);
  },
  Lasso: (p) => {
    p.path(arcPts(8.5, 6.5, 5.0, 0, TAU).map(([x, y]) => [x, add(6.5, mul(sub(y, 6.5), 0.7))]), true);
    p.path([[5.0, 9.3], [4.5, 11.5], [6.0, 12.5], [4.5, 14.5]], false);
  },
  Cut: (p) => {
    p.circle(4.5, 11.5, 2.0);
    p.circle(11.5, 11.5, 2.0);
    p.line(6.0, 10.0, 11.5, 2.5);
    p.line(10.0, 10.0, 4.5, 2.5);
  },
});

function eye(p, open) {
  const [r, cy, half] = [7.0, 7.5, 5.5];
  const a = F(Math.asin(div(half, r)));
  const dd = mul(r, cos(a));
  const e = arcPts(8.0, add(cy, dd), r, sub(-FRAC_PI_2, a), add(-FRAC_PI_2, a));
  e.push(...arcPts(8.0, sub(cy, dd), r, sub(FRAC_PI_2, a), add(FRAC_PI_2, a)));
  p.path(e, true);
  if (open) p.circle(8.0, cy, 2.0);
  else p.line(3.0, 13.0, 13.0, 2.0);
}

function lock(p, locked) {
  p.rect(3.5, 7.5, 12.5, 13.5, 1);
  p.dot(8.0, 10.5, 1.0);
  if (locked) {
    p.line(5.5, 7.5, 5.5, 5.5);
    p.line(10.5, 7.5, 10.5, 5.5);
    p.arc(8.0, 5.5, 2.5, PI, TAU);
  } else {
    p.line(10.5, 7.5, 10.5, 4.5);
    p.arc(13.0, 4.5, 2.5, PI, TAU);
  }
}

function booleanUnion(p) {
  p.rect(2.5, 2.5, 9.5, 9.5, 0);
  p.rect(6.5, 6.5, 13.5, 13.5, 0);
}

function grid(p) {
  for (const i of [2.5, 6.5, 10.5]) for (const j of [2.5, 6.5, 10.5]) p.rect(i, j, i + 3, j + 3, 0);
}

function distributeH(p) {
  p.line(2.5, 2.0, 2.5, 14.0);
  p.line(13.5, 2.0, 13.5, 14.0);
  p.fillRect(6.0, 4.0, 10.0, 12.0, 0);
}

function link(p) {
  p.path(capsule([4.0, 12.0], [6.5, 9.5], 2.4), true);
  p.path(capsule([9.5, 6.5], [12.0, 4.0], 2.4), true);
  p.line(6.2, 9.8, 9.8, 6.2);
}

function rays(p, r) {
  p.circle(8.0, 8.0, r);
  for (let i = 0; i < 8; i++) {
    const a = mul(i, FRAC_PI_4);
    const [r0, r1] = i % 2 === 0 ? [4.8, 6.6] : [4.6, 5.8];
    const [c, s] = [cos(a), sin(a)];
    p.line(add(8, mul(r0, c)), add(8, mul(r0, s)), add(8, mul(r1, c)), add(8, mul(r1, s)));
  }
}

function halfDisc(p, side) {
  p.circle(8.0, 8.0, 5.5);
  p.fill(arcPts(8.0, 8.0, 5.5, side, add(side, PI)));
}

function undo(p, isUndo) {
  const f = (x) => (isUndo ? x : 16 - x);
  const a1 = isUndo ? FRAC_PI_2 : mul(-3, FRAC_PI_2);
  p.path([[f(3.0), 5.5], [f(9.5), 5.5], ...arcPts(f(9.5), 9.0, 3.5, -FRAC_PI_2, a1), [f(6.0), 12.5]], false);
  p.head([f(3.0), 5.5], isUndo ? PI : 0, 3.0);
}

function shapeBuilder(p, merge) {
  const [r, dd] = [4.5, 2.0];
  const a = F(Math.acos(div(dd, r)));
  p.circle(8 - dd, 8.0, r);
  p.circle(8 + dd, 8.0, r);
  if (merge) p.fill([...arcPts(8 - dd, 8, r, -a, a), ...arcPts(8 + dd, 8, r, sub(PI, a), add(PI, a))]);
  else p.line(6.2, 9.8, 9.8, 6.2);
}

function row(p, add_) {
  p.rect(1.5, 2.5, 14.5, 8.5, 1);
  p.line(8.0, 2.5, 8.0, 8.5);
  p.line(5.5, 12.5, 10.5, 12.5);
  if (add_) p.line(8.0, 10.0, 8.0, 15.0);
}

function column(p, add_) {
  p.rect(2.5, 1.5, 8.5, 14.5, 1);
  p.line(2.5, 8.0, 8.5, 8.0);
  p.line(10.0, 8.0, 15.0, 8.0);
  if (add_) p.line(12.5, 5.5, 12.5, 10.5);
}

// ---------------------------------------------------------------------------------------------
// The upstream icon list, and the checks against it.

const iconsRs = readFileSync(join(upstream, 'crates/ui-egui/src/icons.rs'), 'utf8');
const drawRs = readFileSync(join(upstream, 'crates/ui-egui/src/icons/draw.rs'), 'utf8');
const block = iconsRs.match(/icons!\s*\{([\s\S]*?)\n\}/);
if (!block) throw new Error('icons.rs: the `icons! { ... }` list was not found');
const names = block[1]
  .split('\n')
  .map((l) => l.trim())
  .filter((l) => l && !l.startsWith('//'))
  .map((l) => l.replace(/,$/, ''));

const missing = names.filter((n) => typeof DRAW[n] !== 'function');
const extra = Object.keys(DRAW).filter((n) => !names.includes(n));
if (missing.length || extra.length) {
  throw new Error(`icon list mismatch; not ported: [${missing}], not upstream: [${extra}]`);
}
const digest = createHash('sha256').update(drawRs).digest('hex');
if (DRAW_RS_SHA256 !== digest) {
  console.warn(
    `warning: draw.rs differs from the ported revision (${UPSTREAM_COMMIT}); review its diff and port the changes.\n  sha256 now: ${digest}`,
  );
}

const portAt = (k, s) => Object.fromEntries(
  names.map((n) => {
    const p = new P(k, s);
    DRAW[n](p);
    const markup = p.out.join('');
    if (/['\\`]/.test(markup)) throw new Error(`${n}: markup needs escaping`);
    return [n, markup];
  }),
);
/**
 * The scales drawn (device pixels per grid unit): 10, 12, 14 and 16 px, the glyph sizes the editor
 * draws (below 16 px the upstream hints each size's strokes onto the pixel grid), at 1x and 2x. Points per unit follow from the display: 2x above one device pixel per unit, unless
 * `--ppp 1` asks for a 1x display throughout (24 and 32 px at 1x; for comparing with a 1x capture).
 */
const SCALES = [0.625, 0.75, 0.875, 1, 1.25, 1.5, 1.75, 2];
const ppp1 = flag('--ppp') === '1';
const AT = Object.fromEntries(SCALES.map((k) => [k, portAt(k, ppp1 || k <= 1 ? k : k / 2)]));
const ICONS = AT[1];

// ---------------------------------------------------------------------------------------------
// Output.

mkdirSync(outDir, { recursive: true });

// Sizing shared by icons.ts and the gallery (the gallery's copy has its types stripped).
const SIZING_TS = `/** The root attributes every icon is drawn with. */
const HAIRLINE_PAD = 0.01;

export const ICON_SVG_ATTRS = {
  fill: 'none',
  stroke: 'currentColor',
  strokeLinecap: 'butt',
  strokeLinejoin: 'miter',
  strokeMiterlimit: 1.42,
} as const;

export interface IconMetrics {
  /** Device pixels per grid unit. */
  k: number;
  /** The stroke width, in grid units. */
  stroke: number;
  /** One device pixel, in grid units. */
  px: number;
  viewBox: string;
}

/**
 * How the upstream sizes a glyph in a \`size\`-point box at \`pixelRatio\` device pixels per point:
 * whole quarter steps of device pixels per grid unit from 16 px up (the exact fit below), a stroke
 * a whole number of device pixels wide, and the viewBox that centres the 16-unit glyph in the box.
 */
export function iconMetrics(size: number, pixelRatio: number = 1): IconMetrics {
  const fit = (size * pixelRatio) / 16;
  const k = fit >= 1 ? Math.floor(fit * 4) / 4 : fit;
  const span = (size * pixelRatio) / k;
  const origin = 8 - span / 2;
  // A hundredth of a pixel over: Chrome draws strokes up to one device pixel wide as hairlines,
  // which drop their joins.
  const stroke = (Math.max(1, Math.round(k * 0.9)) + HAIRLINE_PAD) / k;
  return { k, stroke, px: (1 + HAIRLINE_PAD) / k, viewBox: [origin, origin, span, span].join(' ') };
}

/**
 * An icon's markup for \`metrics\`, its gradient ids prefixed with \`id\`: the drawing at the
 * nearest scale in \`ICONS_AT\` (exact at those, scaled between them).
 */
export function iconMarkup(name: IconName, metrics: IconMetrics, id: string): string {
  const scales = Object.keys(ICONS_AT);
  const near = scales.reduce((a, b) => (Math.abs(Math.log(Number(b) / metrics.k)) < Math.abs(Math.log(Number(a) / metrics.k)) ? b : a));
  return ICONS_AT[near][name].split('{id}').join(id);
}
`;
const SIZING_JS = SIZING_TS.replace(/^export interface[\s\S]*?\n}\n\n/m, '')
  .replace(/export /g, '')
  .replace(/ as const/g, '')
  .replace(/\): IconMetrics/g, ')')
  .replace(/\): string/g, ')')
  .replace(/: (number|string|IconMetrics|IconName)/g, '');

const header = `// Generated by web/scripts/port-icons.mjs from the upstream editor's code-drawn icons
// (crates/ui-egui/src/icons/draw.rs at ${UPSTREAM_COMMIT}). Do not edit by hand; rerun the script.
`;

const ts = `${header}
/** The icon names, as the upstream \`Icon\` enum declares them. */
export const ICON_NAMES = ${JSON.stringify(names, null, 2).replace(/"/g, "'")} as const;

export type IconName = (typeof ICON_NAMES)[number];

/**
 * Each icon's SVG inner markup on a \`0 0 16 16\` viewBox, exact at 16 px. Strokes and fills use
 * \`currentColor\`; the root \`<svg>\` supplies \`ICON_SVG_ATTRS\`, the stroke width and
 * \`--design-icon-px\` (one device pixel in grid units). Gradient ids contain \`{id}\`, to be replaced
 * with an id unique in the page. \`iconMarkup\` picks the drawing for a size.
 */
export const ICONS: Record<IconName, string> = {
${names.map((n) => `  ${n}: '${ICONS[n]}',`).join('\n')}
};

/**
 * The set as the upstream draws it at each scale it is drawn at here (device pixels per grid
 * unit: 12 and 16 px at 1x and 2x); \`ICONS\` is the one at 1. Strokes snap differently at each
 * (two-pixel strokes onto pixel edges, for one).
 */
export const ICONS_AT: Record<string, Record<IconName, string>> = {
${SCALES.filter((k) => k !== 1).map((k) => `  '${k}': {\n${names.map((n) => `    ${n}: '${AT[k][n]}',`).join('\n')}\n  },`).join('\n')}
  '1': ICONS,
};

${SIZING_TS}`;
writeFileSync(join(outDir, 'icons.ts'), ts);

// The gallery: every icon at 16, 24 and 32 px on the upstream's light and dark panels.
const gallery = `<!doctype html>
<!-- Generated by web/scripts/port-icons.mjs (dev only): every icon at 16, 24 and 32 px, light and dark. -->
<html lang="en">
<head>
<meta charset="utf-8">
<title>Kition Design icons</title>
<style>
  body { margin: 0; font: 11px/1.3 system-ui, sans-serif; }
  .theme { padding: 12px; display: grid; grid-template-columns: repeat(auto-fill, 128px); gap: 8px 6px; }
  .light { background: #ffffff; color: #1e1e1e; }
  .dark { background: #2c2c2c; color: #f5f5f5; }
  .cell { display: flex; flex-direction: column; gap: 4px; }
  .row { display: flex; align-items: flex-start; gap: 8px; height: 32px; }
  .name { opacity: 0.8; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  svg { display: block; flex: none; }
</style>
</head>
<body>
<div class="theme light" id="light"></div>
<div class="theme dark" id="dark"></div>
<script>
const NAMES = ${JSON.stringify(names)};
const ICONS = ${JSON.stringify(ICONS)};
const ICONS_AT = ${JSON.stringify(AT)};
const SIZES = (new URLSearchParams(location.search).get('sizes') || '16,24,32').split(',').map(Number);
${SIZING_JS}
let uid = 0;
function svg(name, size) {
  const m = iconMetrics(size, window.devicePixelRatio || 1);
  const a = ICON_SVG_ATTRS;
  return '<svg xmlns="http://www.w3.org/2000/svg" width="' + size + '" height="' + size + '" viewBox="' + m.viewBox +
    '" fill="' + a.fill + '" stroke="' + a.stroke + '" stroke-width="' + m.stroke + '" stroke-linecap="' + a.strokeLinecap +
    '" stroke-linejoin="' + a.strokeLinejoin + '" stroke-miterlimit="' + a.strokeMiterlimit + '" style="--design-icon-px:' + m.px +
    '" aria-hidden="true">' + iconMarkup(name, m, 'i' + uid++) + '</svg>';
}
for (const theme of ['light', 'dark']) {
  document.getElementById(theme).innerHTML = NAMES.map((n) =>
    '<div class="cell" data-icon="' + n + '"><div class="row">' + SIZES.map((s) => svg(n, s)).join('') +
    '</div><div class="name">' + n + '</div></div>').join('');
}
</script>
</body>
</html>
`;
writeFileSync(join(outDir, 'gallery.html'), gallery);

console.log(`ported ${names.length} icons to ${outDir}`);
