/// <reference types="node" />
// Stages ilo with a few Spaces of public pages and captures its window for the site.
//
// It runs a dev build of ilo (app/build/Browser.app in the browser repo) with a throwaway profile,
// on the Mac's built-in display, and drives it from inside with BROWSER_DRIVE=1, so no click or key
// reaches another app. The stills come from screencapture, of ilo's window alone. ilo has to be the
// app in front each of the two times it opens: only then does its command bar take an address, and
// its window look like the one in use. So it runs where nobody is working, a Mac VM or a Mac left
// alone, never under someone at theirs. With another app in front it waits two minutes for ilo to
// come forward, which macOS lets no script do, and stops. The screen has to be retina: a 1x
// screen gives half-size stills. The Work Space shows this site's /ilo page, served from dist/client
// at a made-up host, so build the site first. Retake them whenever the app's look changes, and
// tools/stills.sh makes the hero's pictures of what this leaves in the out folder:
//
//     vp build && node tools/capture.ts path/to/Browser.app path/to/out [--again]
import { type ChildProcess, execFileSync, spawn } from "node:child_process";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { homedir } from "node:os";
import { extname, join, normalize } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const WORK = join(homedir(), ".cache/ilo-site-capture");
const PROFILE = join(WORK, "profile");
const LOG = join(WORK, "app.log");
const SITE = new URL("../dist/client/", import.meta.url).pathname;

// The window, in points: 2560 × 1600 pixels at 2×.
const WIDTH = 1280;
const HEIGHT = 800;

// How long the app may take to start, in seconds: a dev build's first launch after it's built loads
// its engine's libraries cold, which alone can pass a minute.
const LAUNCH = 180;

// The site's own pages, at a host reserved for examples, so the shot names no one's real site.
const HOST = "ilo.example";

type Space = { name: string; color: number; pinned: string[]; today: string[] };

// Each Space: its name, its colour, its pinned pages and its Today pages, the shown one first. They
// are the Spaces the live interface opens over the stills (src/components/spaces.tsx), tab for
// tab, so nothing in the sidebar changes when it takes their place.
const SPACES: Space[] = [
  {
    name: "Work",
    color: 0x0e9f55,
    pinned: ["https://github.com/MartinPTielemans/ilo"],
    today: [`http://${HOST}/ilo`, "https://doc.rust-lang.org/book/", "https://lobste.rs/"],
  },
  {
    name: "Lisbon",
    color: 0xbf8355,
    pinned: ["https://www.openstreetmap.org/#map=15/38.7115/-9.1340"],
    today: [
      "https://en.wikivoyage.org/wiki/Lisbon",
      "https://en.wikipedia.org/wiki/Alfama",
      "https://en.wikipedia.org/wiki/Pastel_de_nata",
    ],
  },
  {
    name: "Reading",
    color: 0x5583bf,
    pinned: [],
    today: [
      "https://www.gutenberg.org/",
      "https://en.wikipedia.org/wiki/Bauhaus",
      "https://en.wikisource.org/wiki/Main_Page",
    ],
  },
];
const FAVORITES = [
  "https://en.wikipedia.org/wiki/Main_Page",
  "https://archive.org/",
  "https://commons.wikimedia.org/wiki/Main_Page",
];
// A click, in points from the window's corner, once the page has loaded: a cookie dialog's Decline,
// so the dialog stays away when the Space is shot. None of today's pages opens one.
const CLICKS: Record<string, [number, number]> = {};

const PAGES = [...SPACES.flatMap((space) => [...space.pinned, ...space.today]), ...FAVORITES];

const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css",
  ".js": "text/javascript",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".webp": "image/webp",
  ".woff2": "font/woff2",
};

/** Serves the built site on a free local port, which the app reaches as HOST. */
async function serveSite(): Promise<{ port: number; close: () => void }> {
  if (!existsSync(join(SITE, "index.html"))) throw new Error(`no build in ${SITE}; run vp build`);
  const server = createServer((request, response) => {
    const path = normalize(decodeURIComponent(new URL(request.url ?? "/", "http://x").pathname));
    const file = [path, `${path}.html`, join(path, "index.html")]
      .map((candidate) => join(SITE, candidate))
      .find(
        (candidate) =>
          candidate.startsWith(SITE) && existsSync(candidate) && statSync(candidate).isFile(),
      );
    if (!file) {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
    response.end(readFileSync(file));
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  return { port: (server.address() as AddressInfo).port, close: () => server.close() };
}

/** The app, its log, and its driver (platform/mac/drive.mm in the browser repo). */
class App {
  proc: ChildProcess;
  log = "";
  step = 0;

  constructor(binary: string, switches: string[], url?: string) {
    const env: NodeJS.ProcessEnv = {
      ...process.env,
      BROWSER_PROFILE_DIR: PROFILE,
      BROWSER_DISPLAY: "builtin",
      BROWSER_DRIVE: "1",
      // A new profile would play the onboarding first.
      BROWSER_ONBOARDING: "skip",
    };
    delete env.BROWSER_URL;
    if (url) env.BROWSER_URL = url;
    writeFileSync(LOG, "");
    this.proc = spawn(binary, switches, { env, stdio: ["pipe", "ignore", "pipe"] });
    this.proc.stderr?.setEncoding("utf8");
    this.proc.stderr?.on("data", (chunk: string) => {
      this.log += chunk;
      appendFileSync(LOG, chunk);
    });
  }

  exited() {
    return this.proc.exitCode !== null || this.proc.signalCode !== null;
  }

  /** The first match of `pattern` logged after offset `since`, waiting up to `timeout` s. */
  async wait(pattern: string, since = 0, timeout = 30): Promise<RegExpExecArray> {
    const regex = new RegExp(pattern, "m");
    const end = Date.now() + timeout * 1000;
    while (Date.now() < end) {
      const match = regex.exec(this.log.slice(since));
      if (match) return match;
      if (this.exited()) throw new Error(`the app exited, waiting for ${pattern}; see ${LOG}`);
      await sleep(100);
    }
    throw new Error(`timed out waiting for ${pattern}; see ${LOG}`);
  }

  post(command: string) {
    this.step += 1;
    const mark = this.log.length;
    this.proc.stdin?.write(`${command}\n`);
    return mark;
  }

  async send(command: string) {
    const mark = this.post(command);
    await this.wait(`^browser: drive ${this.step} done`, mark);
    if (new RegExp(`^browser: drive ${this.step}: can't do that`, "m").test(this.log)) {
      throw new Error(`the driver can't do ${command}`);
    }
    return mark;
  }

  /** Waits until the app is the one in front, which takes a click on its window. */
  async front(timeout = 120) {
    const end = Date.now() + timeout * 1000;
    for (let asked = false; ; asked = true) {
      const mark = await this.send("state");
      if (/^browser: drive \d+ done.* active 1/m.test(this.log.slice(mark))) return;
      if (Date.now() > end) throw new Error("ilo wasn't brought in front, so nothing was captured");
      if (!asked)
        console.error("Click ilo's window, on the Mac's own screen, and leave it in front.");
      await sleep(500);
    }
  }

  async openTab(url: string) {
    let mark = await this.send("key t cmd");
    await this.wait("^browser: command bar shown$", mark);
    await this.send(`insert ${url}`);
    mark = await this.send("key return");
    await this.wait("^browser: loaded \\d+ ", mark, 60);
  }

  async quit() {
    const mark = this.post("key q cmd");
    await this.wait("^browser: dialog shown Quit", mark);
    await this.wait("^browser: dialog settled$", mark);
    this.post("key return");
    this.proc.stdin?.end();
    const end = Date.now() + 30_000;
    while (!this.exited() && Date.now() < end) await sleep(100);
    stopHelpers();
  }

  /** Ends the app and its helpers after a failure. */
  abort() {
    this.proc.kill("SIGKILL");
    stopHelpers();
  }
}

/** Ends Chromium helpers left with this profile, and only those. */
function stopHelpers() {
  let found: string[] = [];
  try {
    // The pattern starts with dashes, so "--" keeps pgrep from reading it as options.
    found = execFileSync("pgrep", ["-f", "--", `--user-data-dir=${PROFILE}`], { encoding: "utf8" })
      .split("\n")
      .filter(Boolean);
  } catch {
    // pgrep exits 1 when nothing matches.
  }
  for (const pid of found) process.kill(Number(pid), "SIGTERM");
}

/** Opens every page once, so the session keeps a real history for each tab. */
async function populate(binary: string, switches: string[]) {
  rmSync(PROFILE, { recursive: true, force: true });
  mkdirSync(PROFILE, { recursive: true });
  const app = new App(binary, switches, PAGES[0]);
  try {
    await app.wait("^browser: loaded \\d+ ", 0, LAUNCH);
    await app.front();
    await app.send(`resize ${WIDTH} ${HEIGHT}`);
    for (const url of PAGES.slice(1)) {
      await app.openTab(url);
      const click = CLICKS[url];
      if (click) await app.send(`click ${click[0]} ${click[1]}`);
    }
    await app.quit();
  } catch (error) {
    app.abort();
    throw error;
  }
}

/** Python's colorsys.rgb_to_hls, which the theme below was first written with. */
function hls(r: number, g: number, b: number): [number, number, number] {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const light = (max + min) / 2;
  if (max === min) return [0, light, 0];
  const range = max - min;
  const saturation = light <= 0.5 ? range / (max + min) : range / (2 - max - min);
  const [rc, gc, bc] = [r, g, b].map((c) => (max - c) / range);
  const hue = r === max ? bc - gc : g === max ? 2 + rc - bc : 4 + gc - rc;
  return [(((hue / 6) % 1) + 1) % 1, light, saturation];
}

/** The theme text the app keeps for a Space of `color` alone, as Theme::of_color makes it. */
function theme(color: number) {
  const [r, g, b] = [16, 8, 0].map((shift) => ((color >> shift) & 0xff) / 255);
  const [hue, light, saturation] = hls(r, g, b);
  const angle = hue * 2 * Math.PI;
  const text =
    `x=${(light * Math.cos(angle)).toFixed(4)} y=${(light * Math.sin(angle)).toFixed(4)} ` +
    "colors=1 intensity=0.810 grain=0.000 appearance=light";
  return saturation < 1 ? `${text} saturation=${saturation.toFixed(4)}` : text;
}

type Tab = { pinned_url?: string; [key: string]: unknown };
type Saved = {
  spaces: { today: Tab[] }[];
  favorites: Tab[];
  archive: Tab[];
  windows: { space: number }[];
  [key: string]: unknown;
};

/** Sorts the tabs the populate run opened into the Spaces above, as a user would have. */
function arrange() {
  const path = join(PROFILE, "Spaces.json");
  const saved = JSON.parse(readFileSync(path, "utf8")) as Saved;
  const tabs = [...saved.spaces[0].today].reverse();
  if (tabs.length !== PAGES.length) {
    throw new Error(`${tabs.length} tabs were saved for ${PAGES.length} pages`);
  }
  const byUrl = new Map(PAGES.map((url, index) => [url, tabs[index]]));
  const tab = (url: string) => byUrl.get(url) as Tab;
  for (const url of [...FAVORITES, ...SPACES.flatMap((space) => space.pinned)]) {
    tab(url).pinned_url = url;
  }
  saved.favorites = FAVORITES.map(tab);
  saved.spaces = SPACES.map((space) => ({
    name: space.name,
    color: space.color,
    theme: theme(space.color),
    profile: "Default",
    pinned: space.pinned.map(tab),
    today: space.today.map(tab),
    shown: space.pinned.length,
  }));
  saved.archive = [];
  for (const window of saved.windows) window.space = 0;
  writeFileSync(path, JSON.stringify(saved));
}

// The app's windows at the normal level: their window numbers and areas in points.
const WINDOWS_JS = `
ObjC.import('CoreGraphics');
function run(argv) {
  const pid = parseInt(argv[0]);
  const all = ObjC.deepUnwrap(ObjC.castRefToObject(
    $.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, 0)));
  return all.filter(w => w.kCGWindowOwnerPID === pid && w.kCGWindowLayer === 0)
    .map(w => [w.kCGWindowNumber, w.kCGWindowBounds.Width * w.kCGWindowBounds.Height].join(' '))
    .join('\\n');
}
`;

/** The browser window as on screen, and nothing of other apps. */
function captureOnce(app: App, out: string) {
  const script = join(WORK, "windows.js");
  writeFileSync(script, WINDOWS_JS);
  const rows = execFileSync("osascript", ["-l", "JavaScript", script, String(app.proc.pid)], {
    encoding: "utf8",
  })
    .split("\n")
    .filter((row) => row.trim())
    .map((row) => row.split(" "));
  if (!rows.length) throw new Error("the app has no window on screen");
  // The browser window is the largest. Its capture already holds the child windows over it, such
  // as the command bar and menus, so they need no shots of their own.
  const [number] = rows.reduce((a, b) => (Number(b[1]) > Number(a[1]) ? b : a));
  execFileSync("screencapture", ["-x", "-o", "-l", number, out], { timeout: 30_000 });
}

/** The window once two captures in a row match, so pictures and fonts have arrived. */
function screenshot(app: App, out: string, tries = 8) {
  let last: Buffer | undefined;
  for (let i = 0; i < tries; i++) {
    captureOnce(app, out);
    const data = readFileSync(out);
    if (last?.equals(data)) return;
    last = data;
  }
}

async function shoot(binary: string, switches: string[], out: string) {
  mkdirSync(out, { recursive: true });
  const app = new App(binary, switches);
  try {
    let mark = 0;
    for (const [index, space] of SPACES.entries()) {
      if (index) mark = await app.send(`key ${index + 1} ctrl`);
      // A Space's tabs come back unloaded; the one it shows loads as it's shown.
      const [, tab] = await app.wait("^browser: selected (\\d+)$", mark, index ? 30 : LAUNCH);
      await app.wait(`^browser: loaded ${tab} `, mark, 60);
      if (!index) await app.front();
      await app.send("move 1270 790");
      screenshot(app, join(out, `space-${space.name.toLowerCase()}.png`));
    }
    // The first Space again, at its next tab, for the section about the window itself.
    await app.send("key 1 ctrl");
    mark = await app.send("key down alt cmd");
    const [, tab] = await app.wait("^browser: selected (\\d+)( in [\\d.]+ ms)?$", mark);
    await app.wait(`^browser: loaded ${tab} `, mark, 60);
    await app.send("move 1270 790");
    screenshot(app, join(out, `${SPACES[0].name.toLowerCase()}-next.png`));
    await app.quit();
  } catch (error) {
    app.abort();
    throw error;
  }
}

const args = process.argv.slice(2).filter((arg) => arg !== "--again");
if (args.length !== 2) {
  console.error("usage: node tools/capture.ts path/to/Browser.app path/to/out [--again]");
  process.exit(2);
}
const [bundle, out] = args;
mkdirSync(WORK, { recursive: true });
const binary = join(bundle, "Contents/MacOS/Browser");
const site = await serveSite();
// Switches given to the app go on to Chromium: the made-up host leads to the server above.
const switches = [`--host-resolver-rules=MAP ${HOST} 127.0.0.1:${site.port}`];
try {
  // --again shoots the Spaces the last run arranged, without opening every page anew.
  if (!process.argv.includes("--again")) {
    await populate(binary, switches);
    arrange();
  }
  await shoot(binary, switches, out);
} finally {
  site.close();
}
