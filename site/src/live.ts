// The site opens in ilo. It comes as a page; on a computer that can draw it, ilo's own interface,
// built for the web from the browser repo's preview (GPL), then appears around the page, and the
// page is the tab it has open. It's the app's UI code with made-up tabs and no engine: the other
// pages it shows are iframes this page keeps where the demo says the page goes, as the app's host
// keeps Chromium's view there. Without the demo, on a phone or on a connection saving data, the
// page stays a page.

export type Tab = { title: string; url: string; icon?: string };
// `selected` is the tab the Space shows first.
export type DemoSpace = {
  name: string;
  color: number;
  pinned: Tab[];
  today: Tab[];
  selected?: string;
};
export type DemoSpaces = { favorites: Tab[]; spaces: DemoSpace[] };

// The demo's interface (the brief in the browser orchestrator thread, 2026-10-28). Without
// `scheme` the demo takes the visitor's light or dark.
type Host = {
  scheme?: "dark" | "light";
  layOut(x: number, y: number, width: number, height: number, radius: number): void;
  show(url: string | null): void;
  cover(covered: boolean): void;
  space(index: number): void;
  ready(): void;
};

const DEMO = "/demo/ilo_demo.js";

/**
 * Decides, before the first paint, whether ilo's interface will run around the page, and says so
 * with `will-live` on the document, which lays the page out for it (src/styles.css): on a wide
 * screen with a pointer, where the browser can draw it and isn't saving data. A copy of the page
 * inside the demo, in one of its tabs, stays a page. It's the home page's one script in the head,
 * as its own source.
 */
export const WILL_LIVE = `if(top===self&&matchMedia("(min-width: 1024px) and (pointer: fine)").matches&&!navigator.connection?.saveData&&("gpu"in navigator||!!document.createElement("canvas").getContext("webgl2")))document.documentElement.classList.add("will-live")`;

/**
 * What scrolls the page: the page's own box where ilo will run around it, as it's then a tab in
 * ilo's window, and otherwise the window, as null. Whatever watches the page's scroll, or what's
 * on screen in it, takes this as its root.
 */
export const scroller = () =>
  document.documentElement.classList.contains("will-live")
    ? document.querySelector<HTMLElement>(".pitch")
    : null;

// Sites whose headers let another site frame them, checked 2026-10-28. Any other page gets a note
// and a link instead of the browser's refusal.
const FRAMABLE = [
  /(^|\.)wikipedia\.org$/,
  /(^|\.)wikivoyage\.org$/,
  /(^|\.)wikisource\.org$/,
  /^commons\.wikimedia\.org$/,
  /^doc\.rust-lang\.org$/,
  /^www\.rust-lang\.org$/,
  /^www\.gutenberg\.org$/,
  /^lobste\.rs$/,
  /^archive\.org$/,
];

/** Whether a page at `url` can be shown in a frame on a page from `origin`. */
export function framable(url: string, origin: string) {
  let page: URL;
  try {
    page = new URL(url);
  } catch {
    return false;
  }
  if (page.protocol !== "https:" && page.protocol !== "http:") return false;
  if (page.origin === origin) return true;
  // OpenStreetMap lets only its embed be framed.
  if (page.hostname === "www.openstreetmap.org") return page.pathname === "/export/embed.html";
  return FRAMABLE.some((host) => host.test(page.hostname));
}

// Pages kept loaded, so going back to a tab is as quick as in ilo; the oldest hidden one goes.
const KEPT = 10;

/**
 * Starts the demo over `stage` once the page has loaded and is idle, where `will-live` says it
 * runs. `stage` holds `.pages`, whose first child is the page itself: the tab at `home`. The demo's
 * canvas goes under them, and the other tabs' pages beside the page. `on.shown` hears whether the
 * tab shown is the page, and `on.ready` gets what opens the command bar, once ilo has drawn.
 * Returns a stop.
 */
export function startLive(
  stage: HTMLElement,
  spaces: DemoSpaces,
  home: string,
  on: { shown(home: boolean): void; ready(openBar: () => void): void },
): () => void {
  const root = document.documentElement;
  const pages = stage.querySelector<HTMLElement>(".pages");
  const page = pages?.firstElementChild as HTMLElement | null;
  if (!root.classList.contains("will-live") || !pages || !page) return () => {};

  const canvas = document.createElement("canvas");
  canvas.className = "ui";
  canvas.setAttribute("aria-label", "ilo's interface, running in this page");

  const frames = new Map<string, HTMLElement>();
  const tabs = new Map(
    [
      ...spaces.favorites,
      ...spaces.spaces.flatMap((space) => [...space.pinned, ...space.today]),
    ].map((tab) => [tab.url, tab]),
  );

  function frameFor(url: string) {
    const tab = tabs.get(url);
    if (framable(url, location.origin)) {
      const frame = document.createElement("iframe");
      frame.src = url;
      frame.title = tab?.title ?? new URL(url).hostname;
      // No top navigation: a page in the demo can't take this one away.
      frame.setAttribute(
        "sandbox",
        "allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox",
      );
      frame.referrerPolicy = "no-referrer";
      // This site's own pages hand ⌘K to the demo too; another site's keep their keys.
      if (new URL(url).origin === location.origin) {
        frame.addEventListener("load", () => frame.contentWindow?.addEventListener("keydown", key));
      }
      return frame;
    }
    // The page as a card a visitor can act on, so a site that won't be framed doesn't read as the
    // demo failing.
    let host = url;
    try {
      host = new URL(url).hostname.replace(/^www\./, "");
    } catch {}
    const note = document.createElement("div");
    note.className = "refused";
    if (tab?.icon) {
      const icon = document.createElement("img");
      icon.src = tab.icon;
      icon.width = icon.height = 32;
      icon.alt = "";
      note.append(icon);
    }
    const title = document.createElement("p");
    title.className = "title";
    title.textContent = tab?.title ?? host;
    const text = document.createElement("p");
    text.textContent = `${host} doesn't let other sites show its pages, so this one can't open here. In ilo it opens like any other.`;
    note.append(title, text);
    if (/^https?:/.test(url)) {
      const link = document.createElement("a");
      link.className = "button";
      link.textContent = `Open ${host}`;
      link.href = url;
      link.target = "_blank";
      link.rel = "noopener noreferrer";
      note.append(link);
    }
    return note;
  }

  let stopped = false;
  let live = false;
  let gaveUp = 0;
  let arrived = 0;
  let openBar: (() => void) | undefined;
  // Where the demo says the page goes: x, y, width, height and the corner's radius.
  let place: number[] | undefined;

  // The line on the first screen says to press ⌘K, so it works wherever the visitor's keyboard is;
  // in a text field the key stays the field's.
  function key(event: KeyboardEvent) {
    if (!openBar || !live) return;
    if (event.key.toLowerCase() !== "k" || !(event.metaKey || event.ctrlKey)) return;
    if (event.altKey || event.shiftKey) return;
    const target = event.target as HTMLElement;
    if (target.isContentEditable || target.closest?.("input, textarea, select")) return;
    event.preventDefault();
    openBar();
  }

  function lay() {
    if (!live || !place || !pages) return;
    const [x, y, width, height, radius] = place;
    Object.assign(pages.style, {
      inset: "auto",
      left: `${x}px`,
      top: `${y}px`,
      width: `${width}px`,
      height: `${height}px`,
      borderRadius: `${radius}px`,
    });
    // The page's first screen is as tall as ilo's page, and the button that follows the visitor
    // keeps to its middle.
    stage.style.setProperty("--px", `${x}px`);
    stage.style.setProperty("--pw", `${width}px`);
    stage.style.setProperty("--ph", `${height}px`);
  }

  // The page stays a page: the first screen closes with the clip, as where the demo never runs.
  function giveUp() {
    stopped = true;
    canvas.remove();
    root.classList.add("gave-up");
  }

  const host: Host = {
    // The page around the demo is dark for every visitor, so ilo is too.
    scheme: "dark",
    layOut(...rect) {
      place = rect;
      lay();
    },
    show(url) {
      if (stopped || !page || !pages) return;
      page.hidden = url !== home;
      for (const [shown, frame] of frames) frame.hidden = shown !== url;
      on.shown(url === home);
      if (!url || url === home || frames.has(url)) return;
      if (frames.size >= KEPT) {
        const [oldest, frame] = [...frames].find(([, kept]) => kept.hidden) ?? [];
        if (oldest && frame) {
          frame.remove();
          frames.delete(oldest);
        }
      }
      const shown = frameFor(url);
      frames.set(url, shown);
      pages.append(shown);
    },
    // While the bar, a menu or a dialog is drawn over the page, the page goes under the canvas and
    // shows through it, so a click outside them reaches the demo, as in ilo.
    cover(covered) {
      stage.toggleAttribute("data-covered", covered);
    },
    space() {},
    // ilo has drawn: the page, which filled the window, moves to where ilo's page goes.
    ready() {
      if (stopped || !pages) return;
      clearTimeout(gaveUp);
      const box = pages.getBoundingClientRect();
      Object.assign(pages.style, {
        inset: "auto",
        left: "0px",
        top: "0px",
        width: `${box.width}px`,
        height: `${box.height}px`,
      });
      requestAnimationFrame(() => {
        if (stopped) return;
        pages.classList.add("arriving");
        live = true;
        stage.toggleAttribute("data-live", true);
        lay();
        arrived = window.setTimeout(() => pages.classList.remove("arriving"), 900);
        if (openBar) on.ready(openBar);
      });
    },
  };

  async function begin() {
    if (stopped) return;
    try {
      // A whole URL, so the dev server fetches the file as it is instead of transforming it.
      const demo = await import(/* @vite-ignore */ new URL(DEMO, location.origin).href);
      await demo.default();
      if (stopped) return;
      stage.prepend(canvas);
      // A demo that hasn't drawn in ten seconds won't.
      gaveUp = window.setTimeout(giveUp, 10_000);
      openBar = demo.openBar;
      demo.start(canvas, spaces, host);
      addEventListener("keydown", key);
    } catch (error) {
      giveUp();
      console.warn("ilo's live interface didn't start; the page stays a page.", error);
    }
  }

  // After the page's own work, so the first screen is never held up by the demo.
  const idle = () => {
    if ("requestIdleCallback" in window) requestIdleCallback(begin, { timeout: 1500 });
    else setTimeout(begin, 200);
  };
  if (document.readyState === "complete") idle();
  else addEventListener("load", idle, { once: true });

  return () => {
    stopped = true;
    live = false;
    removeEventListener("load", idle);
    removeEventListener("keydown", key);
    clearTimeout(gaveUp);
    clearTimeout(arrived);
    canvas.remove();
    for (const frame of frames.values()) frame.remove();
    page.hidden = false;
    pages.classList.remove("arriving");
    pages.removeAttribute("style");
    stage.removeAttribute("style");
    stage.removeAttribute("data-live");
    stage.removeAttribute("data-covered");
  };
}
