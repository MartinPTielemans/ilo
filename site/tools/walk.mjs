// Walks every page of the built site in headless Chrome, at a desk's width and a phone's, and fails
// on what a reader would trip over: a script error, a file that didn't load, a page wider than its
// screen, a link to nowhere, a download for the wrong computer. It leaves a picture of each page's
// top, middle and foot to look at, which is the other half of the check. It's for a check VM or a
// Linux runner, never the owner's Mac: Chrome for Testing puts a status icon in the menu bar even
// when headless, so on a Mac it refuses to run outside a virtual machine unless told to.
//
//   vp build && vp preview            (or any server of dist/client)
//   node tools/walk.mjs <chrome> <origin, as http://127.0.0.1:4173> [out dir]
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [chrome, origin, out = "walk"] = process.argv.slice(2);
if (!chrome || !origin) {
  console.error("usage: node tools/walk.mjs <chrome> <origin> [out dir]");
  process.exit(2);
}
if (process.platform === "darwin" && !process.env.WALK_ON_HOST) {
  let vm = "0";
  try {
    vm = execFileSync("sysctl", ["-n", "kern.hv_vmm_present"], { encoding: "utf8" }).trim();
  } catch {}
  if (vm !== "1") {
    console.error("walk: this Mac isn't a VM; run it on a check VM or the Linux runner.");
    process.exit(2);
  }
}
mkdirSync(out, { recursive: true });

const profile = mkdtempSync(join(tmpdir(), "ilo-site-walk-"));
const proc = spawn(
  chrome,
  [
    "--headless=new",
    // DevTools over fds 3 and 4: when this script dies, even by SIGKILL, the pipe closes and Chrome
    // quits by itself, which no exit handler here could make it do.
    "--remote-debugging-pipe",
    `--user-data-dir=${profile}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--hide-scrollbars",
    // The land is WebGL, which a machine with no GPU draws in software, and the clips play unasked.
    "--enable-unsafe-swiftshader",
    "--autoplay-policy=no-user-gesture-required",
    // Chrome for Testing turns on the "Open Search with Chromium" menu bar icon, headless or not.
    "--disable-features=OmniboxEverywhere",
    "about:blank",
  ],
  { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] },
);
const [, , , toChrome, fromChrome] = proc.stdio;

// Whatever ends this script, Chrome goes with it: a normal end, a thrown error, a rejected promise,
// a signal or the watchdog. The exit handler runs synchronously, so it can still kill.
let exited = false;
const waiting = new Map();
proc.on("exit", () => {
  exited = true;
  // A call Chrome will never answer fails now instead of waiting for the watchdog.
  for (const done of waiting.values()) done({ error: { message: "Chrome exited" } });
  waiting.clear();
});
// Writing to a Chrome that has just gone fails here, not as an uncaught error.
toChrome.on("error", () => {});
function reap() {
  if (!exited) {
    try {
      proc.kill("SIGKILL");
    } catch {}
  }
  try {
    rmSync(profile, { recursive: true, force: true });
  } catch {}
}
process.on("exit", reap);
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) process.on(signal, () => process.exit(130));
for (const kind of ["uncaughtException", "unhandledRejection"]) {
  process.on(kind, (error) => {
    console.error("walk:", error);
    process.exit(1);
  });
}
const limit = Number(process.env.WALK_TIMEOUT ?? 600) * 1000;
setTimeout(() => {
  console.error(`walk: gave up after ${limit / 1000} s`);
  process.exit(1);
}, limit).unref();

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
let id = 0;
let session;
// What the page being walked said and failed to load, and whether it has finished loading.
let errors = [];
let failed = [];
let loaded = false;
const asked = new Map();
const call = (method, params = {}) =>
  new Promise((done) => {
    waiting.set(++id, done);
    toChrome.write(JSON.stringify({ id, method, params, sessionId: session }) + "\0");
  });
const send = async (method, params) => {
  const answer = await call(method, params);
  if (answer.error) throw new Error(`${method}: ${answer.error.message}`);
  return answer.result;
};

let buffer = "";
fromChrome.setEncoding("utf8");
fromChrome.on("data", (chunk) => {
  buffer += chunk;
  for (let end = buffer.indexOf("\0"); end >= 0; end = buffer.indexOf("\0")) {
    const m = JSON.parse(buffer.slice(0, end));
    buffer = buffer.slice(end + 1);
    if (m.id && waiting.has(m.id)) {
      waiting.get(m.id)(m);
      waiting.delete(m.id);
    }
    const p = m.params;
    if (m.method === "Page.loadEventFired") loaded = true;
    if (m.method === "Runtime.exceptionThrown")
      errors.push(p.exceptionDetails.exception?.description ?? p.exceptionDetails.text);
    if (m.method === "Runtime.consoleAPICalled" && p.type === "error")
      errors.push(p.args.map((a) => a.value ?? a.description).join(" "));
    if (m.method === "Network.requestWillBeSent") asked.set(p.requestId, p.request.url);
    if (m.method === "Network.responseReceived" && p.response.status >= 400)
      failed.push(`${p.response.status} ${p.response.url.replace(origin, "")}`);
    if (m.method === "Network.loadingFailed" && !p.canceled)
      failed.push(`${p.errorText} ${(asked.get(p.requestId) ?? "").replace(origin, "")}`);
  }
});

const problems = [];
const report = { pages: {} };
try {
  await walk();
} finally {
  // Ask Chrome to close, as browser.close() does, then make sure it did.
  session = undefined;
  if (!exited) await Promise.race([call("Browser.close"), sleep(2000)]);
  toChrome.end();
  for (let i = 0; i < 30 && !exited; i++) await sleep(100);
  reap();
}
writeFileSync(join(out, "report.json"), JSON.stringify(report, null, 1));
console.log(JSON.stringify(report));
if (problems.length) {
  console.error(`walk: ${problems.length} problems\n  ${problems.join("\n  ")}`);
  process.exit(1);
}
console.log(`walk: every page is in order; its pictures are in ${out}`);

async function walk() {
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  session = (await send("Target.attachToTarget", { targetId, flatten: true })).sessionId;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Network.enable");

  /** Runs `expression` in the page; `pressed` as a visitor's press would, which fullscreen asks for. */
  const js = async (expression, pressed = false) => {
    const r = await send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
      userGesture: pressed,
    });
    if (r.exceptionDetails)
      throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  const shot = async (name) => {
    const r = await send("Page.captureScreenshot", { format: "jpeg", quality: 70 });
    writeFileSync(join(out, `${name}.jpg`), Buffer.from(r.data, "base64"));
  };
  const AGENTS = {
    mac: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36",
    windows:
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36",
    linux:
      "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36",
    phone:
      "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1",
  };
  const PLATFORM = { mac: "MacIntel", windows: "Win32", linux: "Linux x86_64", phone: "iPhone" };
  /** The visitor: a desk's screen on `who`'s computer, or a phone, held under a finger, of the size given. */
  const visitor = async (who, width, height) => {
    const phone = who === "phone";
    await send("Emulation.setDeviceMetricsOverride", {
      width: width ?? (phone ? 390 : 1440),
      height: height ?? (phone ? 844 : 900),
      deviceScaleFactor: 1,
      mobile: phone,
    });
    await send("Emulation.setTouchEmulationEnabled", { enabled: phone });
    await send("Emulation.setUserAgentOverride", {
      userAgent: AGENTS[who],
      platform: PLATFORM[who],
    });
  };
  /** Opens `path` and waits until it has loaded and its scripts have settled. */
  const visit = async (path) => {
    errors = [];
    failed = [];
    loaded = false;
    await send("Page.navigate", { url: origin + path });
    for (let i = 0; i < 150 && !loaded; i++) await sleep(100);
    if (!loaded) problems.push(`${path}: never finished loading`);
    await js(`document.documentElement.style.scrollBehavior = "auto"`);
    await js(`document.fonts.ready.then(() => new Promise((done) => setTimeout(done, 500)))`);
  };
  const box = `(el) => { const r = el.getBoundingClientRect(); return [r.left, r.top, r.width, r.height].map(Math.round); }`;
  // What every page shares, as this page has it.
  const SHARED = `(() => {
    const box = ${box};
    const land = document.getElementById("land");
    return {
      title: document.title,
      wide: [innerWidth, document.documentElement.scrollWidth],
      tall: document.documentElement.scrollHeight,
      land: land && land.width + "x" + land.height,
      live: document.documentElement.classList.contains("live"),
      bar: box(document.querySelector(".nav-in")),
      links: [...document.querySelectorAll(".nav-links a")].filter((a) => a.offsetParent).map((a) => a.textContent.trim() + (a.getAttribute("aria-current") ? "*" : "")),
      get: [...document.querySelectorAll("a[data-get]")].map((a) => a.textContent + " -> " + a.getAttribute("href").split("/").pop().slice(0, 40)),
      foot: document.querySelectorAll(".foot nav a").length,
      nowhere: [...document.querySelectorAll('a[href^="#"]')].map((a) => a.getAttribute("href")).filter((h) => h.length > 1 && !document.getElementById(decodeURIComponent(h.slice(1)))),
      inner: [...new Set([...document.querySelectorAll('a[href^="/"]')].map((a) => a.getAttribute("href")))],
      small: [...document.querySelectorAll("main p, main li, main dd, main td, main figcaption, .foot a, .foot p")].filter((el) => el.offsetParent && parseFloat(getComputedStyle(el).fontSize) < 15).length,
      first: (() => {
        // The page's first words as they're set, line by line: how near they come to the bar and to the window's sides.
        const lines = [...document.querySelectorAll("main h1, main h1 ~ p, .hero .note")].map((el) => { const all = document.createRange(); all.selectNodeContents(el); return all.getBoundingClientRect(); });
        return [Math.min(...lines.map((b) => b.top)) - document.querySelector(".nav-in").getBoundingClientRect().bottom, Math.min(...lines.map((b) => Math.min(b.left, innerWidth - b.right)))].map(Math.round);
      })(),
    };
  })()`;
  // Where the bar's ink and the hero's mark are, and whether the mark shows.
  const MARK = `(() => {
    const at = (el) => el.getBoundingClientRect();
    const nav = document.getElementById("nav");
    const ink = nav.classList.contains("compact") ? [at(nav.querySelector(".nav-in"))] : [at(nav.querySelector(".nav-brand")), at(nav.querySelector(".nav-cta"))];
    const mark = document.querySelector(".hero .wordmark");
    const m = at(mark);
    return { shows: parseFloat(getComputedStyle(mark).opacity), under: ink.some((b) => m.left < b.right && m.right > b.left && m.top < b.bottom && m.bottom > b.top) };
  })()`;
  // What of the gathered bar lies outside it by the insets it's drawn with, 22 px before the mark and 8 after the
  // button: each thing's name and how many px it's out.
  const INSIDE = `(() => {
    const bar = document.querySelector(".nav-in").getBoundingClientRect();
    const things = [document.querySelector(".nav-brand svg"), ...[...document.querySelectorAll(".nav-links a")].filter((a) => a.offsetParent)];
    return things.map((el) => {
      const b = el.getBoundingClientRect();
      const out = Math.max(bar.left + 22 - b.left, b.right - (bar.right - 8), bar.top - b.top, b.bottom - bar.bottom);
      return [el.localName === "a" ? el.textContent.trim() : "the mark", Math.round(out * 10) / 10];
    }).filter(([, out]) => out > 0.5);
  })()`;
  // Which of the hero's words the Space switcher sits on, if it shows.
  const SWITCHER = `(() => {
    const switcher = document.querySelector(".spaces");
    const style = getComputedStyle(switcher);
    if (style.visibility === "hidden" || parseFloat(style.opacity) === 0) return [];
    const s = switcher.getBoundingClientRect();
    return [...document.querySelectorAll(".hero h1, .hero .lede, .hero .btn, .hero .note")].filter((el) => {
      const words = document.createRange();
      words.selectNodeContents(el);
      const b = el.matches(".btn") ? el.getBoundingClientRect() : words.getBoundingClientRect();
      return b.left < s.right && b.right > s.left && b.top < s.bottom && b.bottom > s.top;
    }).map((el) => el.localName + (el.className ? "." + el.className : ""));
  })()`;
  const PAGES = [
    ["home", "/"],
    ["changelog", "/changelog"],
    ["help", "/help"],
    ["privacy", "/privacy"],
    ["mac", "/mac"],
    ["windows", "/windows"],
    ["linux", "/linux"],
    ["ilo", "/ilo"],
    // An address with no page, which a host answers with 404.html.
    ["404", "/404.html"],
  ];
  const inner = new Set();

  for (const who of ["mac", "phone"]) {
    await visitor(who);
    const width = who === "phone" ? 390 : 1440;
    for (const [name, path] of PAGES) {
      const at = `${name} at ${width}`;
      await visit(path);
      const page = await js(SHARED);
      for (const link of page.inner) inner.add(link);
      delete page.inner;
      await shot(`${name}-${width}-top`);
      // The bar gathers into its pill once the page moves.
      await js(
        `scrollTo(0, Math.min(innerHeight * 0.9, document.documentElement.scrollHeight - innerHeight))`,
      );
      await sleep(1300);
      page.moved = await js(`scrollY > 40`);
      page.pill = await js(`(${box})(document.querySelector(".nav-in"))`);
      if (who === "phone" && page.moved) page.outside = await js(INSIDE);
      await shot(`${name}-${width}-mid`);
      await js(`scrollTo(0, document.documentElement.scrollHeight)`);
      await sleep(900);
      await shot(`${name}-${width}-foot`);
      page.errors = errors;
      page.failed = failed.filter((one) => name !== "404" || !one.startsWith("404 /404"));
      report.pages[at] = page;

      for (const error of page.errors) problems.push(`${at}: ${error}`);
      for (const one of page.failed) problems.push(`${at}: didn't load: ${one}`);
      if (page.wide[0] !== width || page.wide[1] > width)
        problems.push(`${at}: the page is ${Math.max(...page.wide)} wide`);
      if (!page.live || !page.land) problems.push(`${at}: the land isn't drawn`);
      if (page.foot !== 10) problems.push(`${at}: the foot has ${page.foot} links`);
      if (page.nowhere.length)
        problems.push(`${at}: links to no part of the page: ${page.nowhere}`);
      if (page.small) problems.push(`${at}: ${page.small} lines of text under 15 px`);
      if (
        who === "mac" &&
        page.links.map((one) => one.replace(/\d+$/, "")).join() !== expectLinks(path)
      )
        problems.push(`${at}: the bar reads ${page.links.join(" · ")}`);
      if (page.moved && page.pill[2] >= page.bar[2]) problems.push(`${at}: the bar didn't gather`);
      for (const [thing, out] of page.outside ?? [])
        problems.push(`${at}: gathered, ${thing} lies ${out} px past the bar's inset`);
      if (page.first[0] < 0) problems.push(`${at}: the bar lies over the page's first words`);
      if (page.first[1] < 16)
        problems.push(
          `${at}: its first words come within ${page.first[1]} px of the screen's side`,
        );
    }
  }

  // Gathered on a phone of any width from 320 px up, the bar holds its mark and its button whatever the button says:
  // the phone's label (src/send.ts's brief one: the bar's button is the visitor's own download), and each computer's
  // in a narrow window (src/downloads.ts), in its face, a step larger, and in the faces it falls back to before
  // Figtree arrives. Larger and in a fallback face at once is only reported: at 320 the longest labels can't fit both.
  const LABELS = [
    "Send to my computer",
    "Download for Mac",
    "Download for Windows",
    "Download for Linux",
  ];
  const FALLBACK = `font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", "Helvetica Neue", Arial, sans-serif`;
  const FACES = [
    ["in its face", "", true],
    ["a step larger", "font-size: 17.5px", true],
    ["in its fallback face", FALLBACK, true],
    ["a step larger in its fallback face", `font-size: 17.5px; ${FALLBACK}`, false],
  ];
  report.gathered = {};
  for (const width of [320, 360, 390, 440]) {
    await visitor("phone", width, 800);
    await visit("/");
    await js(`scrollTo(0, 600)`);
    await sleep(1300);
    await shot(`home-${width}-gathered`);
    await js(
      `document.head.appendChild(Object.assign(document.createElement("style"), { textContent: "* { transition: none !important; }" }))`,
    );
    for (const label of LABELS) {
      for (const [face, style, held] of FACES) {
        const outside = await js(
          `(() => { const a = document.querySelector(".nav-cta"); a.textContent = ${JSON.stringify(label)}; a.style.cssText = ${JSON.stringify(style)}; return new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))); })().then(() => ${INSIDE})`,
        );
        const at = `home gathered at ${width}, "${label}" ${face}`;
        report.gathered[at] = outside;
        if (held)
          for (const [thing, out] of outside)
            problems.push(`${at}: ${thing} lies ${out} px past the bar's inset`);
      }
    }
  }

  // Home in a phone's own shapes: a wide phone with its browser's bars showing, and lying down. The first words
  // keep off the screen's sides, the switcher stays off them at rest, and nowhere down the first screen does the
  // bar lie over the hero's mark, which is back at the top of the page.
  for (const [width, height] of [
    [440, 760],
    [852, 340],
    [844, 390],
    [667, 375],
  ]) {
    const at = `home at ${width} by ${height}`;
    await visitor("phone", width, height);
    await visit("/");
    const page = await js(SHARED);
    const covered = await js(SWITCHER);
    await shot(`home-${width}-rest`);
    // What eases is looked at where it comes to rest.
    await js(
      `document.head.appendChild(Object.assign(document.createElement("style"), { textContent: "* { transition: none !important; }" }))`,
    );
    const tall = await js(`document.querySelector(".hero").offsetHeight`);
    const under = [];
    let comes = false;
    for (let y = 0; y <= tall; y += 10) {
      await js(
        `scrollTo(0, ${y}); new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)))`,
      );
      const mark = await js(MARK);
      if (mark.under && mark.shows > 0) under.push(y);
      if (y === 150) {
        await shot(`home-${width}-moved`);
        comes = await js(
          `(() => { const s = getComputedStyle(document.querySelector(".spaces")); return s.visibility === "visible" && s.opacity === "1"; })()`,
        );
      }
    }
    await js(`scrollTo(0, 0); new Promise((done) => requestAnimationFrame(done))`);
    const back = (await js(MARK)).shows;
    report.pages[at] = { wide: page.wide, first: page.first, covered, comes, under, back };
    if (page.wide[1] > width) problems.push(`${at}: the page is ${page.wide[1]} wide`);
    if (page.first[0] < 0) problems.push(`${at}: the bar lies over the page's first words`);
    if (page.first[1] < 16)
      problems.push(`${at}: its first words come within ${page.first[1]} px of the screen's side`);
    if (covered.length)
      problems.push(`${at}: the switcher lies over ${covered.join(", ")} at rest`);
    if (!comes) problems.push(`${at}: the switcher isn't there once the page has moved`);
    if (under.length)
      problems.push(
        `${at}: the bar lies over the hero's mark from ${under[0]} to ${under.at(-1)} px down the page`,
      );
    if (back !== 1) problems.push(`${at}: the hero's mark isn't back at the top of the page`);
  }

  // Every page a link on the site names is there.
  report.inner = {};
  for (const link of inner) {
    // Asked from the page, as a visitor's browser would.
    const status = await js(
      `fetch(${JSON.stringify(link.split("#")[0])}).then((answer) => answer.status)`,
    );
    report.inner[link] = status;
    if (status !== 200) problems.push(`a link to ${link} gets ${status}`);
  }

  // The download is for the visitor's computer on every page; a platform's page keeps its own too.
  report.downloads = {};
  const files = { windows: "ilo-windows-x64-setup.exe", linux: "ilo-linux-x86_64.flatpak" };
  for (const who of ["windows", "linux"]) {
    await visitor(who);
    for (const path of ["/", "/mac", "/help"]) {
      await visit(path);
      const gets = await js(
        `[...document.querySelectorAll("a[data-get]")].map((a) => [a.dataset.get, a.getAttribute("href").split("/").pop()])`,
      );
      report.downloads[`${who} ${path}`] = gets;
      for (const [own, file] of gets) {
        const want = own === "mac" ? "ilo-mac-arm64.dmg" : files[who];
        if (file !== want) problems.push(`${path} on ${who}: a download is ${file}, not ${want}`);
      }
    }
  }

  // Home: its type, the Space switcher, and a clip playing once it's on the screen.
  await visitor("mac");
  await visit("/");
  report.fonts = await js(
    `[...document.fonts].filter((f) => f.status === "loaded").map((f) => f.family + " " + f.weight)`,
  );
  if (report.fonts.length < 2) problems.push(`home: its fonts are ${report.fonts}`);
  const colour = `document.querySelector('.spaces button[aria-pressed="true"]').getAttribute("aria-label") + " " + document.querySelector('meta[name="theme-color"]').content`;
  const before = await js(colour);
  await js(`document.querySelectorAll(".spaces button")[3].click()`);
  await sleep(1800);
  report.switcher = [before, await js(colour)];
  await shot("home-1440-turned");
  if (report.switcher[0] === report.switcher[1])
    problems.push("home: the switcher changed nothing");
  await js(`document.querySelectorAll(".spaces button")[0].click()`);
  await sleep(1200);
  for (const [name, to] of [
    ["bar", `document.querySelectorAll("video")[1].scrollIntoView({ block: "center" })`],
    [
      "parts",
      `document.querySelector(".parts").scrollIntoView({ block: "start" }); scrollBy(0, -90)`,
    ],
    ["open", `document.querySelector(".rows").scrollIntoView({ block: "center" })`],
    ["end", `document.querySelector(".end").scrollIntoView({ block: "start" })`],
  ]) {
    await js(to);
    await sleep(name === "bar" ? 2800 : 1200);
    if (name === "bar") {
      report.clip = await js(
        `(() => { const v = document.querySelectorAll("video")[1]; return { src: v.currentSrc.split("/").pop(), at: +v.currentTime.toFixed(2), paused: v.paused, width: v.videoWidth }; })()`,
      );
      if (report.clip.paused || !report.clip.at)
        problems.push("home: the clip on screen isn't playing");
    }
    await shot(`home-1440-${name}`);
  }
  for (const error of errors) problems.push(`home: ${error}`);

  // The changelog: its index, a long release unfolded, one platform's changes, and a clip stood up.
  await visit("/changelog");
  const log = (report.changelog = {});
  log.releases = await js(`document.querySelectorAll(".release").length`);
  log.folded = await js(`document.querySelectorAll(".release.folded").length`);
  const third = `document.querySelectorAll(".release")[2]`;
  await js(`${third}.scrollIntoView({ block: "start" })`);
  await sleep(600);
  log.read = await js(`[document.querySelector(".side a.here")?.hash, "#" + ${third}.id]`);
  if (log.read[0] !== log.read[1])
    problems.push(`changelog: the index marks ${log.read[0]}, reading ${log.read[1]}`);
  if (await js(`!!document.querySelector(".clips .zoom")`)) {
    await js(`document.querySelector(".clips .zoom").scrollIntoView({ block: "center" })`);
    await js(`document.querySelector(".clips .zoom").click()`);
    // The view is read once it has stood up and its clip has started, as the clip says by its own time moving, not
    // after a fixed wait: a clip decoded in software can take longer than one to start (#4029). It gives up after 5 s.
    log.waited = await js(
      `(() => { const start = performance.now(); return new Promise((done) => { const look = () => { const d = document.querySelector(".view"), v = d && d.querySelector("video"); if (d && d.open && v && !v.paused && v.currentTime > 0 && !d.getAnimations({ subtree: true }).some((a) => a.playState === "running")) done(Math.round(performance.now() - start)); else if (performance.now() - start > 5000) done(null); else requestAnimationFrame(look); }; look(); }); })()`,
    );
    log.view = await js(
      `(() => { const d = document.querySelector(".view"), v = d.querySelector("video"); return { open: d.open, size: v && (${box})(v).slice(2), playing: v && !v.paused, screen: [innerWidth, innerHeight] }; })()`,
    );
    await shot("changelog-1440-clip");
    if (!log.view.open || !log.view.playing)
      problems.push("changelog: a clip didn't stand up and play");
    await js(`document.querySelector(".view .shut").click()`);
    await sleep(500);
    if (await js(`document.querySelector(".view").open || !!document.querySelector(".view video")`))
      problems.push("changelog: the clip's view didn't close and clear");
  }
  if (log.folded) {
    await js(`document.querySelector(".release.folded .more").scrollIntoView({ block: "center" })`);
    await shot("changelog-1440-folded");
    await js(`document.querySelector(".release.folded .more").click()`);
    await sleep(300);
    if ((await js(`document.querySelectorAll(".release.folded").length`)) !== log.folded - 1)
      problems.push("changelog: a long release didn't unfold");
  }
  await js(`document.querySelector('.filter button[data-os="linux"]').click(); scrollTo(0, 560)`);
  await sleep(400);
  log.linux = await js(
    `[document.querySelectorAll(".kind li:not([hidden])").length, document.querySelectorAll(".kind li").length]`,
  );
  await shot("changelog-1440-linux");
  if (!log.linux[0] || log.linux[0] >= log.linux[1])
    problems.push(`changelog: Linux's changes are ${log.linux[0]} of ${log.linux[1]}`);
  for (const error of errors) problems.push(`changelog: ${error}`);

  // On a phone a clip goes to the phone's own player, with the controls that leave it.
  await visitor("phone");
  await visit("/changelog");
  if (await js(`!!document.querySelector(".clips .zoom")`)) {
    await js(`document.querySelector(".clips .zoom").scrollIntoView({ block: "center" })`);
    await js(`document.querySelector(".clips .zoom").click()`, true);
    await sleep(900);
    log.phone = await js(
      `({ whole: document.fullscreenElement?.tagName, controls: document.fullscreenElement?.controls })`,
    );
    if (log.phone.whole !== "VIDEO" || !log.phone.controls)
      problems.push("changelog on a phone: a clip didn't open in the player with its controls");
    await js(`document.fullscreenElement && document.exitFullscreen()`);
    await sleep(500);
  }

  // A page's index follows the reading.
  await visitor("mac");
  for (const path of ["/help", "/privacy"]) {
    await visit(path);
    const part = `document.querySelectorAll(".text > section")[2]`;
    await js(`${part}.scrollIntoView({ block: "start" })`);
    await sleep(500);
    const read = await js(`[document.querySelector(".side a.here")?.hash, "#" + ${part}.id]`);
    await shot(`${path.slice(1)}-1440-read`);
    if (read[0] !== read[1])
      problems.push(`${path}: the index marks ${read[0]}, reading ${read[1]}`);
  }

  // A visitor who asked for no motion: nothing plays, and each clip has its controls and its still.
  await send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-reduced-motion", value: "reduce" }],
  });
  for (const path of ["/", "/changelog"]) {
    await visit(path);
    await js(`document.querySelectorAll("video")[1]?.scrollIntoView({ block: "center" })`);
    await sleep(900);
    const clips = await js(
      `[...document.querySelectorAll("video")].slice(0, 4).map((v) => [v.controls, v.paused, v.poster.split("/").pop()])`,
    );
    report[`still ${path}`] = clips;
    if (clips.some(([controls, paused]) => !controls || !paused))
      problems.push(
        `${path}: a clip moves, or has no controls, for a visitor who asked for no motion`,
      );
  }
}

/** What the bar reads on a desk at `path`, with a star on the page the visitor is on. */
function expectLinks(path) {
  return ["Changelog", "Help"]
    .map((name) => name + (path === "/" + name.toLowerCase() ? "*" : ""))
    .concat("GitHub", "Download for Mac")
    .join();
}
