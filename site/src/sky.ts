// Northern lights in the Signal green, behind the first screen. They're drawn smaller than the
// screen and stretched, since the light is soft, and only while the canvas is on screen. `foot`
// says where the light stands, in pixels from the canvas's top.

const SCALE = 0.7;
const QUAD = `attribute vec2 a; void main(){ gl_Position = vec4(a, 0., 1.); }`;
const SKY = `precision highp float;
uniform vec2 uSize;
uniform float uScale, uTime, uFoot, uPointer;
uniform vec3 uTint;
float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p){
  vec2 i = floor(p), f = fract(p), u = f * f * (3. - 2. * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}
float fbm(vec2 p){
  float a = .5, s = 0.;
  for (int i = 0; i < 4; i++) { s += a * noise(p); p = mat2(1.6, 1.2, -1.2, 1.6) * p; a *= .5; }
  return s;
}
void main(){
  vec2 px = gl_FragCoord.xy / uScale;
  vec2 q = vec2(px.x - uSize.x * .5, px.y - uFoot) / 1000.;
  float t = uTime;
  vec3 tint = uTint;
  vec3 deep = uTint * vec3(0.32, 0.5, 0.62);
  vec3 pale = mix(uTint, vec3(1.), 0.74);
  vec3 acc = vec3(0.);
  for (int i = 0; i < 4; i++) {
    float fi = float(i);
    float x = q.x * (1. + fi * 0.13) + fi * 3.1 + uPointer * 0.03 * (fi + 1.);
    float fold = fbm(vec2(x * 1.1 + fi * 5.2, t * 0.04 + fi * 1.7)) - .5;
    float hem = fi * 0.07 + fold * 0.3 + 0.05 * sin(x * 1.9 + t * 0.1 + fi * 2.1);
    float v = q.y - hem;
    float rays = noise(vec2(x * 22. + fold * 14., t * 0.16 + fi * 3.));
    rays = 0.1 + 0.9 * pow(rays, 2.3) * 1.7;
    rays *= 0.72 + 0.28 * noise(vec2(x * 64. + fi * 13., t * 0.3));
    float up = exp(-max(v, 0.) * (2.7 + fi * 0.7));
    float foot = smoothstep(-0.045, 0.01, v);
    float along = smoothstep(0.3, 0.72, fbm(vec2(x * 0.6 - t * 0.022 + fi * 9., fi * 4.)));
    float I = up * foot * rays * (0.06 + 0.94 * along);
    vec3 c = mix(deep, tint, smoothstep(0., 0.7, up)) + pale * pow(up, 5.) * 0.9;
    acc += c * I * (1. - fi * 0.17);
  }
  acc += tint * 0.32 * exp(-abs(q.y) * 9.) * (0.4 + 0.6 * fbm(vec2(q.x * 1.3, t * 0.05)));
  acc *= 1. - smoothstep(uSize.y - uFoot - 260., uSize.y - uFoot, px.y - uFoot) * 0.7;
  vec3 ink = vec3(0.086, 0.078, 0.059);
  vec3 col = ink + (1. - exp(-acc * 2.3));
  vec2 cell = floor(px / 3.);
  float seed = hash(cell);
  float star = step(0.9982, seed) * (0.5 + 0.5 * sin(t * 1.1 + seed * 300.)) * smoothstep(0.05, 0.4, q.y);
  col += star * 0.55 * (1. - smoothstep(0., 0.25, max(acc.r, max(acc.g, acc.b))));
  gl_FragColor = vec4(col, 1.);
}`;

// Signal green, #0E9F55.
const TINT = [0x0e / 255, 0x9f / 255, 0x55 / 255] as const;

/**
 * Starts the lights on `canvas`. Returns `size`, to call when what `foot` measures has moved, and
 * `stop`. Where there's no WebGL the canvas stays clear and the screen stays dark.
 */
export function startSky(
  canvas: HTMLCanvasElement,
  foot: (width: number, height: number) => number,
): { size(): void; stop(): void } {
  const gl = canvas.getContext("webgl", { antialias: false, alpha: false });
  if (!gl) return { size() {}, stop() {} };

  const sky = gl.createProgram();
  for (const [type, source] of [
    [gl.VERTEX_SHADER, QUAD],
    [gl.FRAGMENT_SHADER, SKY],
  ] as const) {
    const shader = gl.createShader(type);
    if (!shader) throw new Error("No shader.");
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      throw new Error(gl.getShaderInfoLog(shader) ?? "The sky's shader didn't compile.");
    }
    gl.attachShader(sky, shader);
  }
  gl.bindAttribLocation(sky, 0, "a");
  gl.linkProgram(sky);
  gl.useProgram(sky);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  const at = (name: string) => gl.getUniformLocation(sky, name);
  const [uSize, uTime, uFoot, uPointer] = ["uSize", "uTime", "uFoot", "uPointer"].map(at);
  gl.uniform1f(at("uScale"), SCALE);
  gl.uniform3f(at("uTint"), ...TINT);

  let width = 0;
  let height = 0;
  let stands = 0;
  // The curtains lean a little toward the pointer.
  let pointer = 0;
  let aim = 0;
  function size() {
    width = canvas.clientWidth;
    height = canvas.clientHeight;
    canvas.width = Math.round(width * SCALE);
    canvas.height = Math.round(height * SCALE);
    gl!.viewport(0, 0, canvas.width, canvas.height);
    stands = height - foot(width, height);
  }
  function render(now: number) {
    pointer += (aim - pointer) * 0.04;
    gl!.uniform2f(uSize!, width, height);
    gl!.uniform1f(uTime!, now / 1000 + 20);
    gl!.uniform1f(uFoot!, stands);
    gl!.uniform1f(uPointer!, pointer);
    gl!.drawArrays(gl!.TRIANGLES, 0, 3);
  }

  // It moves only while it's on screen, and for a visitor who asked for less motion it's one frame.
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  let seen = false;
  let frame = 0;
  function loop(now: number) {
    render(now);
    frame = seen && !still ? requestAnimationFrame(loop) : 0;
  }
  const watch = new IntersectionObserver(([entry]) => {
    seen = !!entry?.isIntersecting;
    if (seen && !frame) frame = requestAnimationFrame(loop);
  });
  const resize = () => {
    size();
    if (still) render(performance.now());
  };
  const move = (event: PointerEvent) => (aim = (event.clientX / innerWidth - 0.5) * 2);

  size();
  // The words above set where the light stands, so it's measured again once their font is in.
  void document.fonts.ready.then(resize);
  watch.observe(canvas);
  addEventListener("resize", resize);
  addEventListener("pointermove", move);

  return {
    size() {
      size();
      render(performance.now());
    },
    stop() {
      cancelAnimationFrame(frame);
      watch.disconnect();
      removeEventListener("resize", resize);
      removeEventListener("pointermove", move);
    },
  };
}

/** Film grain over the soft light, so it never bands: a tile of noise as `element`'s background. */
export function grain(element: HTMLElement) {
  const size = 220;
  const tile = document.createElement("canvas");
  tile.width = tile.height = size;
  const context = tile.getContext("2d");
  if (!context) return;
  const data = context.createImageData(size, size);
  for (let i = 0; i < data.data.length; i += 4) {
    data.data[i] = data.data[i + 1] = data.data[i + 2] = Math.random() * 255;
    data.data[i + 3] = 255;
  }
  context.putImageData(data, 0, 0);
  element.style.background = `url(${tile.toDataURL()}) 0 0 / ${size / 2}px`;
}
