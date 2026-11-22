// The land every page stands on, and what follows it: the bar that gathers into its pill, the page's accent, the
// Space switcher and the clips. It is the browser's own script, put in each page as it is written here
// (src/components/shell.tsx), so it is plain JavaScript that uses nothing but the page it is in.
(() => {
  "use strict";

  /* The land: plain data and one pure function, shade(), that colours a pixel. The fragment shader further down is a
     line-by-line port of it, and the 2D fallback calls it directly, so both draw the same place.
     Seen from straight above, the land is fields, one under each part of the page. A field takes a Space's colours and
     is planted in rows of one of the wordmark's three marks (a dot, a stem or a ring), a few rows of one bloom and then
     a few of the next, with a furrow between. Wind crosses it in wave trains, and where a wave is the marks stand tall.
     Sunlight and cloud shade drift over it. Where words stand, the crop is cleared and no light lifts the ground, so
     white type always sits on the Space's own ground tone or a deeper one. */
  const K = 0.8; // the land pans at 0.8 of the page, so panels float over it
  const WIND = 0.52; // wind heading, radians
  const MAX_CLEAR = 8;
  const HOLD = 1.6; // how strongly a block of words keeps its own field under it
  /* one tone ladder per Space: shade, ground, lit, three blooms, high; ink is for type on white.
     White on every ground is at least 5:1; lit is lighter than that and never comes near words. */
  const SPACES = [
    {
      name: "meadow",
      shade: "#05602F",
      ground: "#07803F",
      lit: "#12A853",
      marks: ["#CFFA7E", "#FFE98A", "#9FF0C0"],
      high: "#FBFFE6",
      ink: "#04170E",
    },
    {
      name: "lavender",
      shade: "#3F27B8",
      ground: "#5B3DEB",
      lit: "#7C60FF",
      marks: ["#D9CBFF", "#FFC2EC", "#B7D4FF"],
      high: "#FBF7FF",
      ink: "#0F0A2C",
    },
    {
      name: "poppy",
      shade: "#A32415",
      ground: "#CE3320",
      lit: "#F04A2C",
      marks: ["#FFD093", "#FFE77A", "#FFC0C8"],
      high: "#FFF8E6",
      ink: "#1F0807",
    },
    {
      name: "flax",
      shade: "#1A45B0",
      ground: "#2560DC",
      lit: "#3F7DF5",
      marks: ["#BFE0FF", "#C9F5E6", "#DCD2FF"],
      high: "#F5FBFF",
      ink: "#061230",
    },
    {
      name: "clover",
      shade: "#A3175F",
      ground: "#CB2678",
      lit: "#E8408F",
      marks: ["#FFC6E0", "#FFD9A8", "#E9C8FF"],
      high: "#FFF5FA",
      ink: "#22061A",
    },
  ];
  const NS = SPACES.length;
  /* Three fields for each of the page's seven parts: the one its words stand in, and one to each side.
     Per field: where it sits across the land (viewport heights from the centre), how far up or down from its part's
     middle, its crop (0 dot, 1 stem, 2 ring), the angle of its rows, and its tone (0, or -1 deeper). */
  const PARTS = 7;
  const MAIN_X = [0.05, 0.22, -0.12, 0.1, -0.18, 0.15, 0.0];
  const MAIN_A = [-0.24, -0.42, 0.2, -0.12, 0.34, -0.3, 0.16];
  const LAY = [];
  for (let i = 0; i < PARTS; i++) {
    LAY.push([MAIN_X[i], 0, i % 3, MAIN_A[i], 0]);
    LAY.push([-1.15 - 0.3 * (i % 2), -0.28, (i + 1) % 3, MAIN_A[i] + 0.7, i % 2 ? 0 : -1]);
    LAY.push([1.05 + 0.3 * ((i + 1) % 2), 0.3, (i + 2) % 3, MAIN_A[i] - 0.5, i % 2 ? -1 : 0]);
  }
  const NF = LAY.length;
  const SLOT = LAY.map((f, i) => Math.floor(i / 3) % NS);
  const CROP = LAY.map((f) => f[2]),
    ANG = LAY.map((f) => f[3]),
    TONE = LAY.map((f) => f[4]);
  const hex = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
  const PAL = SPACES.map((s) => ({
    shade: hex(s.shade),
    ground: hex(s.ground),
    lit: hex(s.lit),
    marks: s.marks.map(hex),
    high: hex(s.high),
  }));
  const PATH = hex("#FFF8E4");

  const fract = (x) => x - Math.floor(x);
  const clamp = (x, a, b) => Math.min(Math.max(x, a), b);
  const mix = (a, b, t) => a + (b - a) * t;
  const mix3 = (a, b, t) => [mix(a[0], b[0], t), mix(a[1], b[1], t), mix(a[2], b[2], t)];
  const smooth = (a, b, x) => {
    const t = clamp((x - a) / (b - a), 0, 1);
    return t * t * (3 - 2 * t);
  };
  const rnd = (x, y) => {
    let a = Math.imul(x | 0, 0x9e3779b1) ^ Math.imul(y | 0, 0x85ebca77);
    a ^= a >>> 15;
    a = Math.imul(a, 0x2c1b3c6d);
    a ^= a >>> 12;
    a = Math.imul(a, 0x297a2d39);
    a ^= a >>> 15;
    return (a >>> 0) / 4294967296;
  };
  const vn = (x, y) => {
    const ix = Math.floor(x),
      iy = Math.floor(y),
      fx = x - ix,
      fy = y - iy;
    const ux = fx * fx * (3 - 2 * fx),
      uy = fy * fy * (3 - 2 * fy);
    return mix(
      mix(rnd(ix, iy), rnd(ix + 1, iy), ux),
      mix(rnd(ix, iy + 1), rnd(ix + 1, iy + 1), ux),
      uy,
    );
  };
  /* which field a point (in viewport heights) is in, its nearest neighbour, and how far the point is from the edge
     between them. S holds x, y and a weight per field; the weights put each edge where one part of the page ends.
     Words keep their part's own field under them: where the ground is cleared (by g), the field `home` reaches further,
     so an edge that would cross a block of words goes round it instead. The bar's words keep no field. */
  const fieldAt = (qx, qy, S, home, g, out) => {
    const wx = qx + 0.045 * (vn(qx * 1.9 + 3.1, qy * 1.9 + 1.7) - 0.5),
      wy = qy + 0.045 * (vn(qx * 1.9 + 17.4, qy * 1.9 + 9.2) - 0.5);
    let d1 = 1e9,
      d2 = 1e9,
      i1 = 0,
      i2 = 0;
    for (let i = 0; i < NF; i++) {
      const dx = wx - S[i * 3],
        dy = wy - S[i * 3 + 1],
        dd = dx * dx + dy * dy - S[i * 3 + 2] - (i === home ? HOLD * g : 0);
      if (dd < d1) {
        d2 = d1;
        i2 = i1;
        d1 = dd;
        i1 = i;
      } else if (dd < d2) {
        d2 = dd;
        i2 = i;
      }
    }
    out.i = i1;
    out.j = i2;
    out.edge = (d2 - d1) / (2 * Math.hypot(S[i2 * 3] - S[i1 * 3], S[i2 * 3 + 1] - S[i1 * 3 + 1]));
  };
  /* the wind: bending wave trains that travel along WIND, with lulls between them; 0 calm .. 1 a gust */
  const wind = (qx, qy, t) => {
    const c = Math.cos(WIND),
      s = Math.sin(WIND),
      al = qx * c + qy * s,
      ac = -qx * s + qy * c;
    const bend =
      vn(al * 1.1 + 2.0, ac * 1.4 - 4.0) * 6.0 + vn(al * 3.3 - 7.0, ac * 3.0 + 1.0) * 1.6;
    const wave = 0.5 + 0.5 * Math.sin(al * 9.5 + bend - t * 0.5);
    const lull = smooth(0.22, 0.72, vn(al * 0.9 - t * 0.05 + 11.0, ac * 0.8 + 23.0));
    return smooth(0.18, 0.92, wave) * mix(0.22, 1.0, lull);
  };
  /* the light: broad pools of sun and of cloud shade that drift across the land; 0 shade .. 1 sun */
  const light = (qx, qy, t) =>
    0.62 * vn(qx * 0.85 + t * 0.016 + 5.0, qy * 0.85 - t * 0.007 + 2.0) +
    0.38 * vn(qx * 2.1 - t * 0.022 + 31.0, qy * 2.1 + t * 0.011 + 17.0);
  /* the three marks, as distances in CSS px: the dot, the stem and the ring (its stroke is 24 to a radius of 50, as the wordmark's) */
  const markDist = (crop, x, y, t) => {
    if (crop === 0) return Math.hypot(x, y) - mix(1.1, 5.3, t);
    if (crop === 1)
      return Math.hypot(x, Math.max(Math.abs(y) - mix(1.0, 7.6, t), 0)) - mix(1.5, 2.9, t);
    const R = mix(2.6, 8.4, t);
    return Math.abs(Math.hypot(x, y) - R * 0.76) - R * 0.24;
  };
  /* clearings: how tall the crop may stand at a point on the screen. Inside a block of words it is the block's own
     cap (0 for type), and it comes back over about 190 px along an uneven edge, so no box shows. */
  let capHome = -1; // the field of the block of words that capAt() last found nearest
  const capAt = (sx, sy, clear) => {
    let cap = 1;
    capHome = -1;
    for (let i = 0; i < clear.length; i++) {
      const r = clear[i],
        cx = 0.5 * (r[0] + r[2]),
        cy = 0.5 * (r[1] + r[3]);
      const qx = Math.abs(sx - cx) - 0.5 * (r[2] - r[0]),
        qy = Math.abs(sy - cy) - 0.5 * (r[3] - r[1]);
      const sd = Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0);
      if (sd > 230) continue;
      const e = sd + 80 * (vn((sx - cx) / 110 + 3.3 + 9.1 * i, (sy - cy) / 110 + 1.7) - 0.5);
      const here = mix(r[4], 1, smooth(40, 190, e));
      if (here < cap) {
        cap = here;
        capHome = r[5];
      }
    }
    return cap;
  };
  const F1 = { i: 0, j: 0, edge: 0 },
    F2 = { i: 0, j: 0, edge: 0 };
  /* One pixel. x, y in CSS px from the viewport's top left.
     u = { w, h, dpr, time, scroll, sun: [x, y], ptr: [headX, headY, tailX, tailY, strength], clear: [[l, t, r, b, cap, home], ...],
           sites: Float32Array, shift: [from, to], wave: [x, y, radius, on] } */
  const shade = (x, y, u) => {
    const wx = x - 0.5 * u.w,
      wy = y + u.scroll * K,
      qx = wx / u.h,
      qy = wy / u.h;
    const capG = capAt(x, y, u.clear);
    fieldAt(qx, qy, u.sites, capHome, smooth(0.35, 1, 1 - capG), F1);
    const fi = F1.i,
      crop = CROP[fi],
      ang = ANG[fi];
    const edgePx = F1.edge * u.h;
    /* the Space: every field moves on by the same number of colours; the switcher's wave carries a new number out from its dot */
    const wv = u.wave,
      on = wv[3] > 0;
    const inG =
      on &&
      Math.hypot(x - wv[0], y - wv[1]) + 44 * (vn(qx * 3.1 + 7.7, qy * 3.1 + 2.3) - 0.5) < wv[2];
    const slot = (SLOT[fi] + (inG ? u.shift[1] : u.shift[0])) % NS,
      pal = PAL[slot];
    /* ground: the field's own tone, lit where the sun falls (never near words), deeper under cloud */
    const lt = light(qx, qy, u.time),
      sunA = smooth(0.52, 0.72, lt),
      shA = 1 - smooth(0.28, 0.48, lt);
    let col = TONE[fi] < 0 ? mix3(pal.ground, pal.shade, 0.5) : pal.ground;
    col = mix3(col, pal.lit, 0.78 * sunA * smooth(0.5, 1, capG));
    col = mix3(col, pal.shade, 0.5 * shA);
    /* this field's rows: a lattice turned to the field's angle, every other row set half a step along */
    const ca = Math.cos(ang),
      sa = Math.sin(ang);
    let lx = ca * wx + sa * wy;
    const ly = -sa * wx + ca * wy;
    const cw = crop === 0 ? 12 : crop === 1 ? 9 : 19,
      ch = crop === 0 ? 12 : crop === 1 ? 21 : 19;
    const row = Math.floor(ly / ch),
      shift = (crop === 2 ? 0 : 0.5 * cw) * (row - 2 * Math.floor(row / 2));
    /* rows are planted in bands of one bloom, with a bare furrow after each band */
    const bn = crop === 0 ? 6 : crop === 1 ? 3 : 4,
      band = Math.floor(row / bn),
      furrow = row - band * bn > bn - 1.5;
    const hue = band - 3 * Math.floor(band / 3);
    if (furrow) col = mix3(col, pal.shade, 0.3 * capG);
    lx += shift;
    const cl = Math.floor(lx / cw);
    const px = (fract(lx / cw) - 0.5) * cw,
      py = (fract(ly / ch) - 0.5) * ch;
    /* the mark's own place, back on the land (c) and on the screen (s) */
    const mx = (cl + 0.5) * cw - shift,
      my = (row + 0.5) * ch;
    const cx = ca * mx - sa * my,
      cy = sa * mx + ca * my;
    const sx = cx + 0.5 * u.w,
      sy = cy - u.scroll * K;
    const capS = capAt(sx, sy, u.clear);
    fieldAt(cx / u.h, cy / u.h, u.sites, capHome, smooth(0.35, 1, 1 - capS), F2);
    let t =
      0.2 +
      0.78 * wind(cx / u.h, cy / u.h, u.time) +
      0.16 * (rnd(cl + 71 * crop, row) - 0.5) +
      0.1 * sunA;
    /* the wave from the switcher: a mark takes its new colour whole, and stands tall as the front passes */
    let palM = pal;
    if (on) {
      const dm =
        Math.hypot(sx - wv[0], sy - wv[1]) +
        44 * (vn((cx / u.h) * 3.1 + 7.7, (cy / u.h) * 3.1 + 2.3) - 0.5);
      palM = PAL[(SLOT[fi] + (dm < wv[2] ? u.shift[1] : u.shift[0])) % NS];
      const z = (dm - wv[2]) / 70;
      t += 0.7 * Math.exp(-z * z);
    }
    /* the pointer: a wake between where it is and where it was */
    if (u.ptr[4] > 0) {
      const ax = u.ptr[0],
        ay = u.ptr[1],
        bx = u.ptr[2] - ax,
        by = u.ptr[3] - ay;
      const h = clamp(((sx - ax) * bx + (sy - ay) * by) / Math.max(bx * bx + by * by, 1e-4), 0, 1);
      const dp = Math.hypot(sx - ax - bx * h, sy - ay - by * h);
      t += 0.75 * u.ptr[4] * (1 - smooth(18, mix(150, 70, h), dp));
    }
    t = Math.min(clamp(t, 0, 1), capS);
    /* no marks in a furrow, on a field's edge, or over it */
    t *= !furrow && F2.i === fi ? smooth(5, 13, F2.edge * u.h) : 0;
    if (t > 0.02) {
      /* each mark throws a short shadow away from the sun, then the mark itself, paler on its sunward side */
      const ox = ca * u.sun[0] + sa * u.sun[1],
        oy = -sa * u.sun[0] + ca * u.sun[1];
      const sh = clamp(0.5 - markDist(crop, px - ox, py - oy, t) * u.dpr, 0, 1);
      col = mix3(col, pal.shade, 0.5 * sh * smooth(0.05, 0.3, t) * (1 - smooth(0.55, 0.85, t)));
      const a = clamp(0.5 - markDist(crop, px, py, t) * u.dpr, 0, 1);
      let mc = mix3(palM.marks[hue], palM.high, clamp(0.6 * smooth(0.62, 1, t) + 0.3 * sunA, 0, 1));
      mc = mix3(mc, palM.ground, 0.22 * shA);
      col = mix3(col, mc, a);
      const hl =
        clamp(0.5 - (markDist(crop, px + 0.4 * ox, py + 0.4 * oy, t) + 1.2) * u.dpr, 0, 1) *
        smooth(0.3, 0.6, t);
      col = mix3(col, palM.high, 0.7 * hl * a);
    }
    /* paths: a pale one between two Spaces, a thin dark one between two fields of the same Space; both stop short of words */
    const other = SLOT[F1.j] !== SLOT[fi];
    const line =
      (other ? clamp((2.6 - edgePx) * u.dpr, 0, 1) : 0.7 * clamp((1.0 - edgePx) * u.dpr, 0, 1)) *
      smooth(0, 0.6, capG);
    return mix3(col, other ? PATH : pal.shade, line);
  };
  /* WORLD-END */

  /* The same function for the GPU. Constants and the field table are written in from the data above. */
  const fl = (n) => (Number.isInteger(n) ? n.toFixed(1) : String(n));
  const VERT = `#version 300 es
in vec2 aPos;
void main() {
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;
  const FRAG = `#version 300 es
precision highp float;
precision highp int;

uniform vec2 uRes;       // canvas, device px
uniform vec2 uView;      // viewport, CSS px
uniform float uDpr;
uniform float uTime;     // wind clock, s
uniform float uScroll;   // page scroll, CSS px
uniform vec2 uSun;       // shadow offset, CSS px
uniform vec4 uPtr;       // pointer wake: head x, y and tail x, y, CSS px
uniform float uPtrS;     // its strength, 0..1
uniform vec4 uClear[${MAX_CLEAR}];   // clearings: l, t, r, b, CSS px
uniform float uCap[${MAX_CLEAR}];    // how tall the crop may stand in each
uniform int uHome[${MAX_CLEAR}];     // the field each keeps under it
uniform int uClearN;
uniform vec3 uSite[${NF}];           // fields: x, y in viewport heights, and a weight
uniform ivec2 uShift;    // how many colours on every field is: before the wave, and behind it
uniform vec4 uWave;      // the switcher's wave: x, y, radius in CSS px, and whether one is out
uniform vec3 uShade[${NS}];
uniform vec3 uGround[${NS}];
uniform vec3 uLit[${NS}];
uniform vec3 uMark[${NS * 3}];
uniform vec3 uHigh[${NS}];

out vec4 oc;

const float K = ${fl(K)};
const float WIND = ${fl(WIND)};
const float HOLD = ${fl(HOLD)};
const vec3 PATH = vec3(${PATH.map((v) => v.toFixed(4)).join(", ")});
const int SLOT[${NF}] = int[${NF}](${SLOT.join(", ")});
const int CROP[${NF}] = int[${NF}](${CROP.join(", ")});
const float ANG[${NF}] = float[${NF}](${ANG.map((a) => a.toFixed(4)).join(", ")});
const float TONE[${NF}] = float[${NF}](${TONE.map(fl).join(", ")});

float rnd(ivec2 p) {
  uvec2 v = uvec2(p);
  uint a = (v.x * 0x9E3779B1u) ^ (v.y * 0x85EBCA77u);
  a ^= a >> 15u; a *= 0x2C1B3C6Du; a ^= a >> 12u; a *= 0x297A2D39u; a ^= a >> 15u;
  return float(a) * (1.0 / 4294967296.0);
}

float vn(vec2 p) {
  vec2 i = floor(p);
  vec2 f = p - i;
  vec2 u = f * f * (3.0 - 2.0 * f);
  ivec2 c = ivec2(i);
  return mix(mix(rnd(c), rnd(c + ivec2(1, 0)), u.x), mix(rnd(c + ivec2(0, 1)), rnd(c + ivec2(1, 1)), u.x), u.y);
}

void fieldAt(vec2 q, int home, float g, out int i1, out int i2, out float edge) {
  vec2 w = q + 0.045 * (vec2(vn(q * 1.9 + vec2(3.1, 1.7)), vn(q * 1.9 + vec2(17.4, 9.2))) - 0.5);
  float d1 = 1e9;
  float d2 = 1e9;
  int n1 = 0;
  int n2 = 0;
  for (int i = 0; i < ${NF}; i++) {
    vec2 d = w - uSite[i].xy;
    float dd = dot(d, d) - uSite[i].z - (i == home ? HOLD * g : 0.0);
    if (dd < d1) { d2 = d1; n2 = n1; d1 = dd; n1 = i; }
    else if (dd < d2) { d2 = dd; n2 = i; }
  }
  i1 = n1;
  i2 = n2;
  edge = (d2 - d1) / (2.0 * length(uSite[n2].xy - uSite[n1].xy));
}

float wind(vec2 q, float t) {
  float c = cos(WIND);
  float s = sin(WIND);
  float al = q.x * c + q.y * s;
  float ac = -q.x * s + q.y * c;
  float bend = vn(vec2(al * 1.1 + 2.0, ac * 1.4 - 4.0)) * 6.0 + vn(vec2(al * 3.3 - 7.0, ac * 3.0 + 1.0)) * 1.6;
  float wave = 0.5 + 0.5 * sin(al * 9.5 + bend - t * 0.5);
  float lull = smoothstep(0.22, 0.72, vn(vec2(al * 0.9 - t * 0.05 + 11.0, ac * 0.8 + 23.0)));
  return smoothstep(0.18, 0.92, wave) * mix(0.22, 1.0, lull);
}

float light(vec2 q, float t) {
  return 0.62 * vn(vec2(q.x * 0.85 + t * 0.016 + 5.0, q.y * 0.85 - t * 0.007 + 2.0))
       + 0.38 * vn(vec2(q.x * 2.1 - t * 0.022 + 31.0, q.y * 2.1 + t * 0.011 + 17.0));
}

float markDist(int crop, vec2 p, float t) {
  if (crop == 0) return length(p) - mix(1.1, 5.3, t);
  if (crop == 1) return length(vec2(p.x, max(abs(p.y) - mix(1.0, 7.6, t), 0.0))) - mix(1.5, 2.9, t);
  float R = mix(2.6, 8.4, t);
  return abs(length(p) - R * 0.76) - R * 0.24;
}

float capAt(vec2 s, out int home) {
  float cap = 1.0;
  home = -1;
  for (int i = 0; i < ${MAX_CLEAR}; i++) {
    if (i >= uClearN) break;
    vec4 r = uClear[i];
    vec2 c = 0.5 * (r.xy + r.zw);
    vec2 d = abs(s - c) - 0.5 * (r.zw - r.xy);
    float sd = length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
    if (sd > 230.0) continue;
    float e = sd + 80.0 * (vn((s - c) / 110.0 + vec2(3.3 + 9.1 * float(i), 1.7)) - 0.5);
    float here = mix(uCap[i], 1.0, smoothstep(40.0, 190.0, e));
    if (here < cap) { cap = here; home = uHome[i]; }
  }
  return cap;
}

void main() {
  vec2 fc = vec2(gl_FragCoord.x, uRes.y - gl_FragCoord.y) / uDpr;
  vec2 w = vec2(fc.x - 0.5 * uView.x, fc.y + uScroll * K);
  vec2 q = w / uView.y;
  int i1;
  int i2;
  float edge;
  int homeG;
  float capG = capAt(fc, homeG);
  fieldAt(q, homeG, smoothstep(0.35, 1.0, 1.0 - capG), i1, i2, edge);
  int crop = CROP[i1];
  float ang = ANG[i1];
  float edgePx = edge * uView.y;

  bool on = uWave.w > 0.0;
  bool inG = on && length(fc - uWave.xy) + 44.0 * (vn(q * 3.1 + vec2(7.7, 2.3)) - 0.5) < uWave.z;
  int slot = (SLOT[i1] + (inG ? uShift.y : uShift.x)) % ${NS};

  float lt = light(q, uTime);
  float sunA = smoothstep(0.52, 0.72, lt);
  float shA = 1.0 - smoothstep(0.28, 0.48, lt);
  vec3 col = TONE[i1] < 0.0 ? mix(uGround[slot], uShade[slot], 0.5) : uGround[slot];
  col = mix(col, uLit[slot], 0.78 * sunA * smoothstep(0.5, 1.0, capG));
  col = mix(col, uShade[slot], 0.5 * shA);

  float ca = cos(ang);
  float sa = sin(ang);
  vec2 l = vec2(ca * w.x + sa * w.y, -sa * w.x + ca * w.y);
  vec2 cell = crop == 0 ? vec2(12.0, 12.0) : (crop == 1 ? vec2(9.0, 21.0) : vec2(19.0, 19.0));
  float row = floor(l.y / cell.y);
  float shift = (crop == 2 ? 0.0 : 0.5 * cell.x) * (row - 2.0 * floor(row / 2.0));
  float bn = crop == 0 ? 6.0 : (crop == 1 ? 3.0 : 4.0);
  float band = floor(row / bn);
  bool furrow = row - band * bn > bn - 1.5;
  int hue = int(band - 3.0 * floor(band / 3.0));
  if (furrow) col = mix(col, uShade[slot], 0.3 * capG);
  l.x += shift;
  float cl = floor(l.x / cell.x);
  vec2 p = (fract(l / cell) - 0.5) * cell;

  vec2 m = vec2((cl + 0.5) * cell.x - shift, (row + 0.5) * cell.y);
  vec2 c = vec2(ca * m.x - sa * m.y, sa * m.x + ca * m.y);
  vec2 s = vec2(c.x + 0.5 * uView.x, c.y - uScroll * K);
  int j1;
  int j2;
  float edge2;
  int homeS;
  float capS = capAt(s, homeS);
  fieldAt(c / uView.y, homeS, smoothstep(0.35, 1.0, 1.0 - capS), j1, j2, edge2);
  float t = 0.2 + 0.78 * wind(c / uView.y, uTime) + 0.16 * (rnd(ivec2(int(cl) + 71 * crop, int(row))) - 0.5) + 0.1 * sunA;

  int slotM = slot;
  if (on) {
    float dm = length(s - uWave.xy) + 44.0 * (vn(c / uView.y * 3.1 + vec2(7.7, 2.3)) - 0.5);
    slotM = (SLOT[i1] + (dm < uWave.z ? uShift.y : uShift.x)) % ${NS};
    float z = (dm - uWave.z) / 70.0;
    t += 0.7 * exp(-z * z);
  }

  if (uPtrS > 0.0) {
    vec2 pa = uPtr.xy;
    vec2 pb = uPtr.zw - pa;
    float h = clamp(dot(s - pa, pb) / max(dot(pb, pb), 1e-4), 0.0, 1.0);
    float dp = length(s - pa - pb * h);
    t += 0.75 * uPtrS * (1.0 - smoothstep(18.0, mix(150.0, 70.0, h), dp));
  }
  t = min(clamp(t, 0.0, 1.0), capS);
  t *= (!furrow && j1 == i1) ? smoothstep(5.0, 13.0, edge2 * uView.y) : 0.0;

  if (t > 0.02) {
    vec2 o = vec2(ca * uSun.x + sa * uSun.y, -sa * uSun.x + ca * uSun.y);
    float sh = clamp(0.5 - markDist(crop, p - o, t) * uDpr, 0.0, 1.0);
    col = mix(col, uShade[slot], 0.5 * sh * smoothstep(0.05, 0.3, t) * (1.0 - smoothstep(0.55, 0.85, t)));
    float ma = clamp(0.5 - markDist(crop, p, t) * uDpr, 0.0, 1.0);
    vec3 mc = mix(uMark[slotM * 3 + hue], uHigh[slotM], clamp(0.6 * smoothstep(0.62, 1.0, t) + 0.3 * sunA, 0.0, 1.0));
    mc = mix(mc, uGround[slotM], 0.22 * shA);
    col = mix(col, mc, ma);
    float hl = clamp(0.5 - (markDist(crop, p + 0.4 * o, t) + 1.2) * uDpr, 0.0, 1.0) * smoothstep(0.3, 0.6, t);
    col = mix(col, uHigh[slotM], 0.7 * hl * ma);
  }

  bool other = SLOT[i2] != SLOT[i1];
  float line = (other ? clamp((2.6 - edgePx) * uDpr, 0.0, 1.0) : 0.7 * clamp((1.0 - edgePx) * uDpr, 0.0, 1.0)) * smoothstep(0.0, 0.6, capG);
  col = mix(col, other ? PATH : uShade[slot], line);
  oc = vec4(col, 1.0);
}`;

  /* ---- the page ---- */
  const T0 = 9; // the wind's clock on the first frame
  const IDLE_MS = 20000; // with no input for this long the wind dies down and nothing is drawn
  const WAVE_MS = 1150; // how long a new colour takes to cross the screen
  const root = document.documentElement;
  const reduce = matchMedia("(prefers-reduced-motion: reduce)");
  /* The page's parts, each with three fields under it: what the page marks with data-part, or, where it marks none,
     slices of it a little over a screen tall. The land holds seven parts' fields, so on a longer page it holds the
     seven around the screen: part p stands on set p % PARTS, and a set changes parts only while it is screens away. */
  const marked = [...document.querySelectorAll("main [data-part]")];
  const main = document.querySelector("main");
  let parts = []; // [top, height] of each part, down the page
  let first = 0; // the first of the seven parts the land holds now
  const partTop = (p) =>
    p < parts.length ? parts[p][0] : main.offsetHeight + (p - parts.length) * vh;
  const partTall = (p) => (p < parts.length ? parts[p][1] : vh);
  const partAt = (y) => {
    let now = 0;
    parts.forEach((s, i) => {
      if (s[0] <= y) now = i;
    });
    return now;
  };
  const slotOf = (p) => (p % PARTS) % NS;
  const nav = document.getElementById("nav");
  const theme = document.querySelector('meta[name="theme-color"]');
  const picks = [...document.querySelectorAll("#switcher button")];
  const clearEls = [...document.querySelectorAll("[data-clear]")];
  let canvas = document.getElementById("land");

  let vw = 1,
    vh = 1,
    dpr = 1;
  let clock = T0,
    gust = reduce.matches ? 0 : 1,
    lastInput = performance.now();
  let scrollY = window.scrollY,
    drawnScroll = -1;
  let clears = []; // [l, top, r, bottom, cap, pinned, home]: on the page, or on the screen when pinned
  let bar = 0,
    barWant = 0; // how far the bar is gathered into its pill; at 1 the crop has grown back where its words stood
  const sites = new Float32Array(NF * 3);
  const ptr = { x: 0, y: 0, hx: 0, hy: 0, tx: 0, ty: 0, s: 0, seen: 0, has: false };
  let part = -1; // the part of the page under the middle of the screen
  let shift = 0,
    shiftFrom = 0; // how many colours on every field is, and was before the wave now out
  const wave = { on: false, x: 0, y: 0, t0: 0, r: 0 };

  /* shadows fall away from the sun: short at noon, longer and leaning in the morning and the evening, by the visitor's clock */
  const sun = () => {
    const d = new Date(),
      a = ((clamp(d.getHours() + d.getMinutes() / 60, 7, 19) - 12) / 6) * 1.1,
      len = 1.5 + (2.0 * Math.abs(a)) / 1.1;
    return [Math.sin(a) * len, 0.75 * len];
  };
  let sunNow = sun();

  const measure = () => {
    const box = canvas.getBoundingClientRect();
    vw = box.width || window.innerWidth;
    vh = box.height || window.innerHeight;
    scrollY = window.scrollY;
    /* words keep 14 px of bare ground to each side and 8 above and below before the crop starts to come back */
    /* the bar's words clear the ground where they stand with the bar open, which its measurements give whatever it is doing now */
    const ns = getComputedStyle(nav),
      px = (name) => parseFloat(ns.getPropertyValue(name));
    const barW = Math.min(nav.clientWidth, px("--bar-w")),
      barL = (nav.clientWidth - barW) / 2,
      barH = px("--bar-h"),
      barPad = px("--bar-pad");
    clears = clearEls.map((el) => {
      const r = el.getBoundingClientRect(),
        cap = parseFloat(el.dataset.clear);
      if ("pin" in el.dataset) {
        const left = el.classList.contains("nav-brand")
            ? barL + barPad
            : barL + barW - barPad - r.width,
          top = (barH - r.height) / 2;
        return [left - 14, top - 8, left + r.width + 14, top + r.height + 8, cap, true, -1];
      }
      return [
        r.left - 14,
        r.top + scrollY - 8,
        r.right + 14,
        r.bottom + scrollY + 8,
        cap,
        false,
        3 * (partAt(r.top + scrollY + r.height / 2) % PARTS),
      ];
    });
    if (marked.length) parts = marked.map((s) => [s.offsetTop, s.offsetHeight]);
    else {
      const n = Math.max(1, Math.round(main.offsetHeight / (vh * 1.25))),
        tall = main.offsetHeight / n;
      parts = Array.from({ length: n }, (_, i) => [i * tall, tall]);
    }
    place();
  };
  /* a field under each part the land holds, with its edges where the part begins and ends */
  const place = () => {
    const land = (y) => (vh / 2 + (y - vh / 2) * K) / vh;
    let weight = 0;
    for (let p = first; p < first + PARTS; p++) {
      const top = partTop(p),
        mid = land(top + partTall(p) / 2),
        set = p % PARTS;
      if (p > first) {
        const edge = land(top),
          was = land(partTop(p - 1) + partTall(p - 1) / 2);
        weight += (edge - mid) * (edge - mid) - (edge - was) * (edge - was);
      }
      for (let k = 0; k < 3; k++) {
        const f = LAY[set * 3 + k];
        sites.set([f[0], mid + f[1], weight], (set * 3 + k) * 3);
      }
    }
    if (gl && loc) gl.uniform3fv(loc.uSite, sites);
  };
  const uniforms = (ratio) => {
    const open = smooth(0, 1, bar),
      clear = []; // the bar's clearing closes as the bar gathers into its own dark pill
    for (const r of clears) {
      const top = r[5] ? r[1] : r[1] - scrollY,
        bottom = r[5] ? r[3] : r[3] - scrollY;
      if (bottom < -180 || top > vh + 180 || clear.length === MAX_CLEAR) continue;
      clear.push([r[0], top, r[2], bottom, r[5] ? mix(r[4], 1, open) : r[4], r[6]]);
    }
    return {
      w: vw,
      h: vh,
      dpr: ratio,
      time: clock,
      scroll: scrollY,
      sun: sunNow,
      sites,
      clear,
      ptr: [ptr.hx, ptr.hy, ptr.tx, ptr.ty, ptr.s],
      shift: [shiftFrom, shift],
      wave: [wave.x, wave.y, wave.r, wave.on ? 1 : 0],
    };
  };
  /* the Space under the middle of the screen gives the page its accent: the wordmark's dot, the buttons' ink, the switcher */
  const accent = (force) => {
    const now = partAt(scrollY + vh / 2);
    if (now === part && !force) return;
    part = now;
    /* the land moves on with the page: once the screen is two parts from an end of the seven it holds, it holds the
       seven around the screen, and the sets that change parts are the ones farthest from it */
    const want = clamp(now - 3, 0, Math.max(0, parts.length - PARTS));
    if (want !== first && (force || now - first < 2 || now - first > 4)) {
      first = want;
      place();
      dirty = true;
    }
    const k = (slotOf(now) + shift) % NS,
      s = SPACES[k];
    root.style.setProperty("--ground", s.ground);
    root.style.setProperty("--shade", s.shade);
    root.style.setProperty("--mark", s.marks[0]);
    root.style.setProperty("--ink", s.ink);
    picks.forEach((b, i) => b.setAttribute("aria-pressed", i === k ? "true" : "false"));
    theme.content = s.ground; // a phone's status bar takes the field's colour
  };

  /* ---- WebGL2 ---- */
  let gl = null,
    prog = null,
    loc = null;
  const palette = (key) =>
    new Float32Array(PAL.flatMap((p) => (key === "marks" ? p.marks.flat() : p[key])));
  const setupGL = () => {
    gl = canvas.getContext("webgl2", {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      powerPreference: "low-power",
    });
    if (!gl) return false;
    const compile = (type, src, name) => {
      const sh = gl.createShader(type);
      gl.shaderSource(sh, src);
      gl.compileShader(sh);
      if (gl.getShaderParameter(sh, gl.COMPILE_STATUS)) return sh;
      console.error(
        "ilo land: the " + name + " shader did not compile\n" + gl.getShaderInfoLog(sh),
      );
      return null;
    };
    const vs = compile(gl.VERTEX_SHADER, VERT, "vertex"),
      fs = compile(gl.FRAGMENT_SHADER, FRAG, "fragment");
    if (!vs || !fs) return false;
    prog = gl.createProgram();
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.bindAttribLocation(prog, 0, "aPos");
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      console.error("ilo land: the program did not link\n" + gl.getProgramInfoLog(prog));
      return false;
    }
    gl.useProgram(prog);
    gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    loc = {};
    for (const n of [
      "uRes",
      "uView",
      "uDpr",
      "uTime",
      "uScroll",
      "uSun",
      "uPtr",
      "uPtrS",
      "uClear",
      "uCap",
      "uHome",
      "uClearN",
      "uSite",
      "uShift",
      "uWave",
      "uShade",
      "uGround",
      "uLit",
      "uMark",
      "uHigh",
    ])
      loc[n] = gl.getUniformLocation(prog, n);
    gl.uniform3fv(loc.uShade, palette("shade"));
    gl.uniform3fv(loc.uGround, palette("ground"));
    gl.uniform3fv(loc.uLit, palette("lit"));
    gl.uniform3fv(loc.uMark, palette("marks"));
    gl.uniform3fv(loc.uHigh, palette("high"));
    gl.uniform3fv(loc.uSite, sites);
    return true;
  };
  const sizeGL = () => {
    /* at most two device px per CSS px, and no more than about 5.5 million of them */
    dpr = Math.min(window.devicePixelRatio || 1, 2, Math.sqrt(5.5e6 / (vw * vh)));
    const w = Math.max(1, Math.round(vw * dpr)),
      h = Math.max(1, Math.round(vh * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    gl.viewport(0, 0, w, h);
  };
  const clearBuf = new Float32Array(MAX_CLEAR * 4),
    capBuf = new Float32Array(MAX_CLEAR),
    homeBuf = new Int32Array(MAX_CLEAR);
  const drawGL = () => {
    const u = uniforms(dpr);
    gl.uniform2f(loc.uRes, canvas.width, canvas.height);
    gl.uniform2f(loc.uView, u.w, u.h);
    gl.uniform1f(loc.uDpr, dpr);
    gl.uniform1f(loc.uTime, u.time);
    gl.uniform1f(loc.uScroll, u.scroll);
    gl.uniform2f(loc.uSun, u.sun[0], u.sun[1]);
    gl.uniform4f(loc.uPtr, u.ptr[0], u.ptr[1], u.ptr[2], u.ptr[3]);
    gl.uniform1f(loc.uPtrS, u.ptr[4]);
    clearBuf.fill(0);
    capBuf.fill(1);
    u.clear.forEach((r, i) => {
      clearBuf.set(r.slice(0, 4), i * 4);
      capBuf[i] = r[4];
      homeBuf[i] = r[5];
    });
    gl.uniform4fv(loc.uClear, clearBuf);
    gl.uniform1fv(loc.uCap, capBuf);
    gl.uniform1iv(loc.uHome, homeBuf);
    gl.uniform1i(loc.uClearN, u.clear.length);
    gl.uniform2i(loc.uShift, u.shift[0], u.shift[1]);
    gl.uniform4f(loc.uWave, u.wave[0], u.wave[1], u.wave[2], u.wave[3]);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  };

  /* ---- the loop: runs only while something is changing ---- */
  let raf = 0,
    last = 0,
    lastDraw = 0,
    onScreen = true,
    dirty = true;
  const frame = (now) => {
    raf = 0;
    const dt = last ? Math.min((now - last) / 1000, 0.1) : 0;
    last = now;
    const still = reduce.matches;
    const want = still || now - lastInput > IDLE_MS ? 0 : 1;
    gust += (want - gust) * (1 - Math.exp(-dt * 0.9));
    if (still || (want === 0 && gust < 0.004)) gust = 0;
    clock += dt * gust;

    /* the switcher's wave: out from its dot to the far corner, quick at first */
    if (wave.on) {
      const p = (now - wave.t0) / WAVE_MS;
      if (p >= 1 || still) {
        wave.on = false;
        shiftFrom = shift;
      } else
        wave.r =
          (1 - Math.pow(1 - p, 3)) *
          (Math.hypot(Math.max(wave.x, vw - wave.x), Math.max(wave.y, vh - wave.y)) + 190);
      dirty = true;
    }

    /* the bar: the crop grows back where its words stood as they gather into the pill, and lies down a little quicker
       when they come back out, so the ground is bare by the time they arrive */
    if (bar !== barWant) {
      bar = still
        ? barWant
        : barWant > bar
          ? Math.min(barWant, bar + dt / 0.7)
          : Math.max(barWant, bar - dt / 0.42);
      dirty = true;
    }

    /* the wake's head follows the pointer closely and its tail slowly; it fades a moment after the pointer rests */
    const active = !still && ptr.has && now - ptr.seen < 1400;
    ptr.s += ((active ? 1 : 0) - ptr.s) * (1 - Math.exp(-dt * (active ? 6 : 2.2)));
    if (!active && ptr.s < 0.004) ptr.s = 0;
    ptr.hx += (ptr.x - ptr.hx) * (1 - Math.exp(-dt * 16));
    ptr.hy += (ptr.y - ptr.hy) * (1 - Math.exp(-dt * 16));
    ptr.tx += (ptr.x - ptr.tx) * (1 - Math.exp(-dt * 3.2));
    ptr.ty += (ptr.y - ptr.ty) * (1 - Math.exp(-dt * 3.2));

    const moved = scrollY !== drawnScroll || ptr.s > 0 || dirty;
    /* scroll, the pointer and the wave draw every frame; the wind alone draws at half that */
    if (moved || now - lastDraw > 30) {
      drawGL();
      lastDraw = now;
      drawnScroll = scrollY;
      dirty = false;
    }
    if ((gust > 0 || ptr.s > 0 || wave.on || bar !== barWant) && onScreen && !document.hidden)
      raf = requestAnimationFrame(frame);
    else last = 0;
  };
  const wake = (input) => {
    if (input) lastInput = performance.now();
    if (gl && !raf && onScreen && !document.hidden) raf = requestAnimationFrame(frame);
  };

  /* ---- 2D fallback: the same shade(), one still picture, redrawn when the page comes to rest ---- */
  let job = 0,
    shownScroll = 0,
    settle = 0;
  const draw2d = () => {
    const id = ++job,
      w = Math.max(1, Math.round(vw)),
      h = Math.max(1, Math.round(vh)),
      u = uniforms(1);
    u.ptr[4] = 0;
    const img = new ImageData(w, h),
      px = img.data;
    let y = 0;
    const step = () => {
      if (id !== job) return;
      const until = performance.now() + 10;
      while (y < h && performance.now() < until) {
        for (let x = 0, o = y * w * 4; x < w; x++, o += 4) {
          const c = shade(x + 0.5, y + 0.5, u);
          px[o] = c[0] * 255 + 0.5;
          px[o + 1] = c[1] * 255 + 0.5;
          px[o + 2] = c[2] * 255 + 0.5;
          px[o + 3] = 255;
        }
        y++;
      }
      if (y < h) {
        requestAnimationFrame(step);
        return;
      }
      /* the canvas stays clear, with the page's own ground showing through, until the picture is whole */
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
      }
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.putImageData(img, 0, 0);
      shownScroll = u.scroll;
      canvas.style.transform = `translateY(${(shownScroll - scrollY) * K}px)`;
    };
    step();
  };
  const start2d = () => {
    /* a canvas that has given out a WebGL context can't give a 2D one, so take a fresh canvas */
    const fresh = canvas.cloneNode(false);
    canvas.replaceWith(fresh);
    canvas = fresh;
    gl = null;
    redraw();
  };

  /* ---- wiring ---- */
  let useGL = false;
  const redraw = () => {
    if (useGL) {
      dirty = true;
      wake(false);
    } else {
      clearTimeout(settle);
      settle = setTimeout(draw2d, 140);
    }
  };
  /* the bar gathers once the page has moved; the land follows it in time, or at once where nothing eases
     (the still picture, reduced motion, a page still loading) */
  const gather = (compact) => {
    nav.classList.toggle("compact", compact);
    barWant = compact ? 1 : 0;
    if (!useGL || reduce.matches || !root.classList.contains("live")) bar = barWant;
  };
  gather(scrollY > 40);
  measure();
  accent(false);
  try {
    useGL = setupGL();
  } catch {
    console.error("ilo land: WebGL2 failed to start", err);
    useGL = false;
  }
  if (useGL) {
    sizeGL();
    drawGL();
    drawnScroll = scrollY;
    dirty = false; // the first frame, before anything moves
    wake(false);
    canvas.addEventListener("webglcontextlost", (e) => {
      e.preventDefault();
      cancelAnimationFrame(raf);
      raf = 0;
    });
    canvas.addEventListener("webglcontextrestored", () => {
      if (setupGL()) {
        sizeGL();
        redraw();
      }
    });
  } else {
    start2d();
  }

  /* pick a Space: the colour under the screen becomes the one picked, and every other field moves on with it */
  const turn = (to, x, y) => {
    to = ((to % NS) + NS) % NS;
    if (to === shift) return;
    shiftFrom = wave.on ? shiftFrom : shift;
    shift = to;
    if (useGL && !reduce.matches) {
      wave.on = true;
      wave.x = x;
      wave.y = y;
      wave.r = 0;
      wave.t0 = performance.now();
    } else {
      wave.on = false;
      shiftFrom = shift;
    }
    accent(true);
    if (useGL) {
      dirty = true;
      wake(true);
    } else redraw();
  };
  picks.forEach((b, k) =>
    b.addEventListener("click", () => {
      const r = b.getBoundingClientRect();
      turn(k - slotOf(part), r.left + r.width / 2, r.top + r.height / 2);
    }),
  );

  window.addEventListener(
    "scroll",
    () => {
      scrollY = window.scrollY;
      const compact = scrollY > 40;
      if (compact !== nav.classList.contains("compact")) {
        gather(compact);
        dirty = true;
      }
      accent(false);
      if (useGL) wake(true);
      else {
        canvas.style.transform = `translateY(${(shownScroll - scrollY) * K}px)`;
        redraw();
      }
    },
    { passive: true },
  );
  window.addEventListener(
    "pointermove",
    (e) => {
      if (!useGL || reduce.matches) return;
      ptr.x = e.clientX;
      ptr.y = e.clientY;
      ptr.seen = performance.now();
      if (!ptr.has || ptr.s === 0) {
        ptr.hx = ptr.tx = ptr.x;
        ptr.hy = ptr.ty = ptr.y;
      }
      ptr.has = true;
      wake(true);
    },
    { passive: true },
  );
  window.addEventListener("keydown", () => wake(true));
  let resizing = 0;
  const onResize = () => {
    measure();
    accent(true);
    sunNow = sun();
    if (useGL) sizeGL();
    redraw();
  };
  window.addEventListener("resize", () => {
    clearTimeout(resizing);
    resizing = setTimeout(onResize, 60);
  });
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) {
      sunNow = sun();
      redraw();
    }
  });
  if ("IntersectionObserver" in window) {
    new IntersectionObserver((es) => {
      onScreen = es[es.length - 1].isIntersecting;
      if (onScreen) redraw();
    }).observe(canvas);
  }
  if (document.fonts && document.fonts.ready)
    void document.fonts.ready.then(() => {
      measure();
      accent(true);
      redraw();
    });
  window.addEventListener("load", () => {
    measure();
    accent(true);
    redraw();
  });
  /* stills and clips change the page's height as they arrive, and the fields follow the page */
  if ("ResizeObserver" in window)
    new ResizeObserver(() => {
      clearTimeout(resizing);
      resizing = setTimeout(onResize, 60);
    }).observe(main);

  /* ---- clips: each plays only while it is on the screen, and never when motion is reduced. Then it gets controls,
     and shows the moment its caption is about instead of the frame it starts on ---- */
  const clips = [...document.querySelectorAll("video")];
  const stillClips = () =>
    clips.forEach((v) => {
      v.pause();
      v.controls = reduce.matches;
      if (reduce.matches && v.dataset.still) v.poster = v.dataset.still;
    });
  if ("IntersectionObserver" in window) {
    const seen = new IntersectionObserver(
      (es) =>
        es.forEach((e) => {
          if (e.isIntersecting && !reduce.matches) {
            const p = e.target.play();
            if (p) p.catch(() => {});
          } else e.target.pause();
        }),
      { threshold: 0.35 },
    );
    clips.forEach((v) => seen.observe(v));
  }
  if (reduce.matches) stillClips();

  /* ---- the Spaces clip leads the page: at each swipe in it, every field moves on (or back) a colour as the window
     does, in a wave out from where the fingers are. The clip ends on the Space it began on, so a loop leaves the page
     as it was; a clip that stops part way is wound back, and the page with it. ---- */
  const lead = document.getElementById("swipes");
  if (lead) {
    const swipes = lead.dataset.swipes.split(",").map((w) => w.split(":").map(Number));
    const from = lead.dataset.from.split(" ").map(Number);
    let next = 0,
      moved = 0,
      last = 0,
      watching = 0;
    const send = (by) => {
      const r = lead.getBoundingClientRect();
      moved += by;
      turn(shift + by, r.left + r.width * from[0], r.top + r.height * from[1]);
    };
    const watch = () => {
      const t = lead.currentTime;
      if (t < last) next = 0; // it looped
      last = t;
      while (next < swipes.length && t >= swipes[next][0]) send(swipes[next++][1]);
      watching = lead.paused ? 0 : requestAnimationFrame(watch);
    };
    lead.addEventListener("play", () => {
      if (!watching) watching = requestAnimationFrame(watch);
    });
    lead.addEventListener("pause", () => {
      cancelAnimationFrame(watching);
      watching = 0;
      if (moved) {
        const by = -moved;
        moved = 0;
        send(by);
        moved = 0;
      }
      next = 0;
      last = 0;
      try {
        lead.currentTime = 0;
      } catch {
        /* not loaded yet */
      }
    });
  }
  reduce.addEventListener("change", () => {
    lastInput = performance.now();
    stillClips();
    redraw();
  });

  /* From here the bar and the colours ease. Not before: a browser puts a reloaded page's scroll back after the first
     frame, and the bar would be seen gathering itself from the top of the page. */
  const live = () =>
    requestAnimationFrame(() => requestAnimationFrame(() => root.classList.add("live")));
  if (document.readyState === "complete") live();
  else window.addEventListener("load", live);
})();
