// Opens the site with the live demo in headless Chrome and runs steps against it. It's for a check
// VM or a Linux runner, never the owner's Mac: Chrome for Testing puts a status icon in the menu bar
// even when headless, so on a Mac it refuses to run outside a virtual machine unless told to.
//
//   node tools/live-check.mjs <chrome> <url> [width] [height] <step>...
//
// Steps: state | wait:ms | size:w,h | eval:js | click:x,y | click:<selector> | clickin:dx,dy |
// move:dx,dy | swipe:dx,dy,deltaX,n | wheels:dx,dy,d1;d2;... | key:Meta+k | key:Space | type:text |
// shot:out.jpg[@x,y,w,h,scale | @selector]. Coordinates for clickin, move, swipe and wheels are offsets from
// the demo canvas's top left.
import { execFileSync, spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [chrome, page, W = "1440", H = "900", ...steps] = process.argv.slice(2);
if (!chrome || !page) {
  console.error("usage: node tools/live-check.mjs <chrome> <url> [width] [height] <step>...");
  process.exit(2);
}
if (process.platform === "darwin" && !process.env.LIVE_CHECK_ON_HOST) {
  let vm = "0";
  try {
    vm = execFileSync("sysctl", ["-n", "kern.hv_vmm_present"], { encoding: "utf8" }).trim();
  } catch {}
  if (vm !== "1") {
    console.error("live-check: this Mac isn't a VM; run it on a check VM or the Linux runner.");
    process.exit(2);
  }
}

const profile = mkdtempSync(join(tmpdir(), "ilo-site-real-"));
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
process.on("uncaughtException", (error) => {
  console.error("live-check:", error);
  process.exit(1);
});
process.on("unhandledRejection", (error) => {
  console.error("live-check:", error);
  process.exit(1);
});
const limit = Number(process.env.LIVE_CHECK_TIMEOUT ?? 180) * 1000;
setTimeout(() => {
  console.error(`live-check: gave up after ${limit / 1000} s`);
  process.exit(1);
}, limit).unref();

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
let id = 0;
const logs = [];
const send = (method, params = {}, sessionId) =>
  new Promise((done) => {
    const call = ++id;
    waiting.set(call, done);
    toChrome.write(JSON.stringify({ id: call, method, params, sessionId }) + "\0");
  });

try {
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
      if (m.method === "Runtime.exceptionThrown")
        logs.push("exception: " + m.params.exceptionDetails.exception?.description?.slice(0, 300));
      if (m.method === "Runtime.consoleAPICalled" && m.params.type !== "debug")
        logs.push(
          m.params.type +
            ": " +
            m.params.args
              .map((a) => a.value ?? a.description)
              .join(" ")
              .slice(0, 200),
        );
    }
  });
  await run();
} finally {
  // Ask Chrome to close, as browser.close() does, then make sure it did.
  if (!exited) await Promise.race([send("Browser.close"), sleep(2000)]);
  toChrome.end();
  for (let i = 0; i < 30 && !exited; i++) await sleep(100);
  reap();
}

async function run() {
  const {
    result: { targetId },
  } = await send("Target.createTarget", { url: "about:blank" });
  const {
    result: { sessionId },
  } = await send("Target.attachToTarget", { targetId, flatten: true });
  await send("Runtime.enable", {}, sessionId);
  await send(
    "Emulation.setDeviceMetricsOverride",
    { width: +W, height: +H, deviceScaleFactor: 1, mobile: false },
    sessionId,
  );
  const t0 = Date.now();
  await send("Page.navigate", { url: page }, sessionId);
  const evaluate = async (expression) => {
    const r = (
      await send(
        "Runtime.evaluate",
        { expression, awaitPromise: true, returnByValue: true },
        sessionId,
      )
    ).result;
    if (r.exceptionDetails)
      return "threw: " + (r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result?.value;
  };
  const canvasAt = async () =>
    evaluate(
      `(() => { const b = document.querySelector(".live canvas").getBoundingClientRect(); return [b.left, b.top]; })()`,
    );
  const state = `(() => { const s = document.querySelector(".stage"), l = document.querySelector(".live"), p = l?.querySelector(".pages"), c = l?.querySelector("canvas");
    const r = (e) => { if (!e) return null; const b = e.getBoundingClientRect(); return [b.left, b.top, b.width, b.height].map(Math.round).join(" "); };
    const frames = p ? [...p.children].map((f) => (f.hidden ? "-" : "+") + (f.tagName === "IFRAME" ? new URL(f.src).host + new URL(f.src).pathname.slice(0, 24) : "note:" + f.textContent.slice(0, 30))) : [];
    return { live: s?.hasAttribute("data-live"), dot: getComputedStyle(document.documentElement).getPropertyValue("--dot").trim(),
      covered: l?.hasAttribute("data-covered"), y: Math.round(scrollY), canvas: r(c), pages: r(p), radius: p?.style.borderRadius, frames, focus: document.activeElement?.tagName }; })()`;

  const live = await evaluate(
    `new Promise((r) => { const c = () => document.querySelector(".stage[data-live]") ? r(true) : setTimeout(c, 25); c(); setTimeout(() => r(false), 20000); })`,
  );
  console.log("live after", Date.now() - t0, "ms:", live);
  const mods = { Alt: 1, Control: 2, Meta: 4, Shift: 8 };
  const codes = {
    Escape: ["Escape", 27],
    Enter: ["Enter", 13],
    ArrowDown: ["ArrowDown", 40],
    ArrowUp: ["ArrowUp", 38],
    Tab: ["Tab", 9],
    Backspace: ["Backspace", 8],
    Space: ["Space", 32],
  };
  const wheel = (x, y, deltaX) =>
    send("Input.dispatchMouseEvent", { type: "mouseWheel", x, y, deltaX, deltaY: 0 }, sessionId);

  for (const step of steps) {
    const [kind, arg = ""] = step.split(/:(.*)/s);
    if (kind === "state") console.log("state", JSON.stringify(await evaluate(state)));
    else if (kind === "wait") await sleep(+arg);
    else if (kind === "size") {
      const [w, h] = arg.split(",").map(Number);
      await send(
        "Emulation.setDeviceMetricsOverride",
        { width: w, height: h, deviceScaleFactor: 1, mobile: false },
        sessionId,
      );
      await sleep(800);
    } else if (kind === "eval") console.log("eval", JSON.stringify(await evaluate(arg)));
    else if (kind === "click" || kind === "clickin") {
      let [x, y] = arg.split(",").map(Number);
      if (kind === "click" && !/^\d/.test(arg)) {
        [x, y] = await evaluate(
          `(() => { const e = document.querySelector(${JSON.stringify(arg)}); e.scrollIntoView({ block: "center", behavior: "instant" }); const b = e.getBoundingClientRect(); return [b.x + b.width / 2, b.y + b.height / 2]; })()`,
        );
      }
      if (kind === "clickin") {
        const [l, t] = await canvasAt();
        x += l;
        y += t;
      }
      for (const type of ["mouseMoved", "mousePressed", "mouseReleased"])
        await send(
          "Input.dispatchMouseEvent",
          { type, x, y, button: "left", clickCount: 1 },
          sessionId,
        );
      await sleep(400);
    } else if (kind === "move") {
      const [dx, dy] = arg.split(",").map(Number);
      const [l, t] = await canvasAt();
      await send(
        "Input.dispatchMouseEvent",
        { type: "mouseMoved", x: l + dx, y: t + dy },
        sessionId,
      );
      await sleep(300);
    } else if (kind === "swipe") {
      // A swipe's fingers, then its momentum dying away, without the pointer moving.
      const [dx, dy, deltaX, n] = arg.split(",").map(Number);
      const [l, t] = await canvasAt();
      for (let i = 0; i < n; i++) {
        const d = i < n / 2 ? deltaX : Math.round(deltaX * (1 - (i - n / 2) / (n / 2)));
        await wheel(l + dx, t + dy, d);
        await sleep(16);
      }
      await sleep(300);
    } else if (kind === "wheels") {
      const [dx, dy, list] = arg.split(",");
      const [l, t] = await canvasAt();
      for (const d of list.split(";").map(Number)) {
        await wheel(l + +dx, t + +dy, d);
        await sleep(16);
      }
    } else if (kind === "key") {
      const parts = arg.split("+");
      const name = parts.pop();
      const modifiers = parts.reduce((m, p) => m | mods[p], 0);
      const [code, keyCode] = codes[name] ?? [
        "Key" + name.toUpperCase(),
        name.toUpperCase().charCodeAt(0),
      ];
      // Space is named in a step but types a character, and a page scrolls on that character.
      const key = name === "Space" ? " " : name;
      const text = name === "Space" ? " " : codes[name] || modifiers & 6 ? undefined : key;
      await send(
        "Input.dispatchKeyEvent",
        {
          type: text ? "keyDown" : "rawKeyDown",
          key,
          code,
          windowsVirtualKeyCode: keyCode,
          modifiers,
          text,
          commands: modifiers & 4 && key === "k" ? [] : undefined,
        },
        sessionId,
      );
      await send(
        "Input.dispatchKeyEvent",
        { type: "keyUp", key, code, windowsVirtualKeyCode: keyCode, modifiers },
        sessionId,
      );
      await sleep(400);
    } else if (kind === "type") {
      await send("Input.insertText", { text: arg }, sessionId);
      await sleep(400);
    } else if (kind === "shot") {
      // The clip is in page coordinates, so a shot after scrolling names the scrolled-to y. A
      // selector instead scrolls to that element and takes all of it, at half size.
      const [file, box] = arg.split("@");
      const [x, y, w, h, scale] =
        box && !/^\d/.test(box)
          ? await evaluate(
              `new Promise((done) => { const e = document.querySelector(${JSON.stringify(box)}); scrollTo(0, e.getBoundingClientRect().top + scrollY); setTimeout(() => { const b = e.getBoundingClientRect(); done([b.left + scrollX, b.top + scrollY, b.width, b.height, 0.5]); }, 1200); })`,
            )
          : box
            ? box.split(",").map(Number)
            : [0, 0, +W, +H, 0.5];
      const shot = await send(
        "Page.captureScreenshot",
        {
          format: "jpeg",
          quality: 70,
          clip: { x, y, width: w, height: h, scale },
          captureBeyondViewport: true,
        },
        sessionId,
      );
      writeFileSync(file, Buffer.from(shot.result.data, "base64"));
      console.log("shot", file);
    } else throw new Error(`unknown step: ${step}`);
  }
  console.log("logs:", logs.length ? "\n  " + logs.slice(0, 20).join("\n  ") : "none");
}
