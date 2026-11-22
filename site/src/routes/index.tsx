import { createFileRoute } from "@tanstack/react-router";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { Download, useVisitor } from "../components/download";
import { Reel } from "../components/reel";
import { Wordmark } from "../components/wordmark";
import { PLATFORM_LIST, SOURCE as GITHUB } from "../downloads";
import { useEntrances } from "../entrances";
import { type DemoSpaces, type Tab, WILL_LIVE, scroller, startLive } from "../live";
import { pageHead, structuredData } from "../seo";
import { grain, startSky } from "../sky";

const description = `ilo is a free, open-source browser: every part of your life in its own Space, a sidebar that tidies itself and one bar for every command, on Chromium. In beta for Mac, Windows and Linux.`;

export const Route = createFileRoute("/")({
  head: () => ({
    ...pageHead({
      path: "/",
      title: "ilo: the free, open-source browser that keeps going",
      description,
      data: structuredData(description),
    }),
    // Known before the first paint, so the first screen is already the one that fits (src/live.ts).
    scripts: [{ children: WILL_LIVE }],
  }),
  // The one page whose interactions need React in the browser (src/routes/__root.tsx).
  staticData: { hydrates: true },
  component: Home,
});

const icon = (site: string) => `/media/fav-${site}.png`;
const WIKIPEDIA = icon("wikipedia");
// The page itself, as a tab in ilo.
const HOME = "/";

// What ilo has open around the page: three Spaces of pages that let another site frame them, and
// this site's own. GitHub won't be framed, so its tab shows a note and a link.
const FAVORITES: Tab[] = [
  { title: "Wikipedia", url: "https://en.wikipedia.org/wiki/Main_Page", icon: WIKIPEDIA },
  { title: "Internet Archive", url: "https://archive.org/", icon: icon("archive") },
  {
    title: "Wikimedia Commons",
    url: "https://commons.wikimedia.org/wiki/Main_Page",
    icon: icon("commons"),
  },
];
const SPACES: { name: string; color: string; pinned: Tab[]; today: Tab[]; selected?: string }[] = [
  {
    name: "ilo",
    color: "#0E9F55",
    pinned: [
      {
        title: "MartinPTielemans/ilo",
        url: "https://github.com/MartinPTielemans/ilo",
        icon: icon("github"),
      },
    ],
    today: [
      { title: "ilo, a good tool", url: HOME, icon: icon("ilo") },
      { title: "What's new in ilo", url: "/changelog", icon: icon("ilo") },
      { title: "Help", url: "/help", icon: icon("ilo") },
      {
        title: "The Rust Programming Language",
        url: "https://doc.rust-lang.org/book/",
        icon: icon("rust"),
      },
      { title: "Lobsters", url: "https://lobste.rs/", icon: icon("lobsters") },
      { title: "Tool – Wikipedia", url: "https://en.wikipedia.org/wiki/Tool", icon: WIKIPEDIA },
    ],
    selected: HOME,
  },
  {
    name: "Lisbon",
    color: "#BF8355",
    pinned: [
      {
        title: "OpenStreetMap",
        url: "https://www.openstreetmap.org/export/embed.html?bbox=-9.144%2C38.7065%2C-9.124%2C38.7165&layer=mapnik",
        icon: icon("osm"),
      },
    ],
    today: [
      {
        title: "Lisbon – Wikivoyage",
        url: "https://en.wikivoyage.org/wiki/Lisbon",
        icon: icon("wikivoyage"),
      },
      { title: "Alfama – Wikipedia", url: "https://en.wikipedia.org/wiki/Alfama", icon: WIKIPEDIA },
    ],
  },
  {
    name: "Reading",
    color: "#5583BF",
    pinned: [
      { title: "Project Gutenberg", url: "https://www.gutenberg.org/", icon: icon("gutenberg") },
    ],
    today: [
      {
        title: "Bauhaus – Wikipedia",
        url: "https://en.wikipedia.org/wiki/Bauhaus",
        icon: WIKIPEDIA,
      },
      {
        title: "Wikisource",
        url: "https://en.wikisource.org/wiki/Main_Page",
        icon: icon("wikisource"),
      },
    ],
  },
];

const here = (path: string) => new URL(path, location.origin).href;

/** The Spaces as the live interface takes them, with this site's own pages at its own address. */
function demoSpaces(): DemoSpaces {
  const tab = (one: Tab) => ({
    ...one,
    url: here(one.url),
    ...(one.icon && { icon: here(one.icon) }),
  });
  return {
    favorites: FAVORITES.map(tab),
    spaces: SPACES.map((space) => ({
      name: space.name,
      color: Number.parseInt(space.color.slice(1), 16),
      pinned: space.pinned.map(tab),
      today: space.today.map(tab),
      ...(space.selected && { selected: here(space.selected) }),
    })),
  };
}

/** Whether the visitor's keyboard is Apple's, with ⌘ and ⇧ on its keys, as the prerendered page takes it to be. */
function useApple() {
  const [apple, setApple] = useState(true);
  useEffect(() => setApple(/Mac|iPhone|iPad/.test(navigator.userAgent)), []);
  return apple;
}

// The bar on top: it stays as the page scrolls, and takes the page's ground once it has left the
// first screen's sky.
function Head() {
  const [stuck, setStuck] = useState(false);
  useEffect(() => {
    const within = scroller();
    const scrolled = within ?? window;
    const update = () => setStuck((within ? within.scrollTop : scrollY) > 24);
    update();
    scrolled.addEventListener("scroll", update, { passive: true });
    return () => scrolled.removeEventListener("scroll", update);
  }, []);
  return (
    <header className={stuck ? "head stuck" : "head"}>
      <a className="brand" href="#top" aria-label="ilo, back to the top">
        <Wordmark />
      </a>
      <nav className="links" aria-label="Sections">
        <a href="#look">What it does</a>
        <a href="#why">Why trust it</a>
        <a href={GITHUB}>GitHub</a>
        <Download className="button" brief />
      </nav>
    </header>
  );
}

/** One of the parts to see: its clip, its name, how it's reached and what it does. */
function Part({
  clip,
  shows,
  name,
  keys,
  children,
}: {
  clip: string;
  shows: string;
  name: string;
  keys: string;
  children: ReactNode;
}) {
  return (
    <figure>
      <Reel name={clip} width={1440} height={900} label={shows} />
      <figcaption>
        <h3>{name}</h3>
        <kbd>{keys}</kbd>
        <p>{children}</p>
      </figcaption>
    </figure>
  );
}

// The site opens in ilo. It comes as a page; on a computer that can draw it, ilo's real interface
// then appears around the page, and the page is a tab in it, so a visitor has used ilo before they
// are asked for anything. Everywhere else the page is a page, with a clip where the live interface
// would be.
function Home() {
  const stage = useRef<HTMLDivElement>(null);
  const hero = useRef<HTMLElement>(null);
  const sky = useRef<HTMLCanvasElement>(null);
  const dust = useRef<HTMLDivElement>(null);
  // What opens ilo's command bar, once ilo has drawn.
  const openBar = useRef<() => void>(undefined);
  // Whether ilo shows another tab than the page, and whether one of the page's own download
  // buttons is on screen: the button that follows is up when the visitor has neither before them.
  const [away, setAway] = useState(false);
  const [offered, setOffered] = useState(true);
  const { runs } = useVisitor();
  const apple = useApple();
  useEntrances();

  // The lights stand on the foot of the first screen, or on the clip where ilo's interface can't
  // run.
  useEffect(() => {
    const screen = hero.current;
    if (!screen || !sky.current || !dust.current) return;
    grain(dust.current);
    try {
      const lights = startSky(sky.current, (_, height) => {
        const clip = screen.querySelector(".film")?.getBoundingClientRect();
        return clip?.height
          ? clip.top + clip.height * 0.5 - screen.getBoundingClientRect().top
          : height;
      });
      const sized = new ResizeObserver(() => lights.size());
      sized.observe(screen);
      return () => {
        sized.disconnect();
        lights.stop();
      };
    } catch (error) {
      console.warn("The sky didn't start; the first screen stays dark.", error);
    }
  }, []);

  useEffect(() => {
    if (!stage.current) return;
    // The page scrolls in its own box where ilo will run around it, so the keys that scroll a
    // page reach it only once it has the focus.
    const page = scroller();
    if (page && document.activeElement === document.body) {
      page.tabIndex = -1;
      page.focus({ preventScroll: true });
    }
    return startLive(stage.current, demoSpaces(), here(HOME), {
      shown: (home) => setAway(!home),
      ready: (open) => (openBar.current = open),
    });
  }, []);

  useEffect(() => {
    const seen = new Set<Element>();
    const watch = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) seen.add(entry.target);
          else seen.delete(entry.target);
        }
        setOffered(seen.size > 0);
      },
      { root: scroller() },
    );
    for (const button of document.querySelectorAll(".cta")) watch.observe(button);
    return () => watch.disconnect();
  }, []);

  const follows = away || !offered;
  return (
    <div className="stage" ref={stage}>
      <div className="pages">
        <div className="pitch">
          <Head />

          <main>
            <section className="hero" id="top" ref={hero}>
              <canvas className="sky" ref={sky} aria-hidden="true" />
              <div className="grain" ref={dust} aria-hidden="true" />
              <h1>
                The browser <br />
                that keeps going.
              </h1>
              <div className="say">
                <p className="lede">
                  Every part of your life in its own Space, a sidebar that tidies itself, and one
                  bar for every command. Free and open source, on Chromium.
                </p>
                <div className="act">
                  <Download className="button big cta" />
                  <p className="note">
                    {runs ? "In beta for Mac, Windows and Linux." : `ilo is for ${PLATFORM_LIST}.`}
                  </p>
                </div>
              </div>
              {/* One line says the ilo around the page is real, and what to press. A window that
                  looks like a picture has to say it isn't one, or nobody clicks it. */}
              <p className="hint">
                <b>That is ilo around this page, running for real.</b> Press{" "}
                <button
                  type="button"
                  className="key"
                  aria-label={`Open the command bar (${apple ? "⌘K" : "Ctrl K"})`}
                  onClick={() => openBar.current?.()}
                >
                  {apple ? "⌘K" : "Ctrl K"}
                </button>{" "}
                or open a tab in its sidebar.
              </p>
              <div className="film">
                <Reel
                  name="bar"
                  width={1280}
                  height={648}
                  label="ilo's command bar finding an open tab from a few typed letters"
                />
              </div>
            </section>

            {/* What the page can't run, in real footage cut down to the action. */}
            <section className="look" id="look" aria-labelledby="look-title">
              <div className="lead">
                <h2 id="look-title">
                  The parts you <br />
                  have to see.
                </h2>
                <p>Filmed in ilo, and played a little faster than life.</p>
              </div>
              <div className="clips">
                <div className="col">
                  <Part
                    clip="zap"
                    shows="Zap hiding the notices at the top of a travel guide, one click each"
                    name="Zap"
                    keys="Boosts"
                  >
                    Click what you don't want on a site. It's gone, and it stays gone when you come
                    back.
                  </Part>
                  <Part
                    clip="peek"
                    shows="A link opened in Peek, over the page it was on, then closed"
                    name="Peek"
                    keys={apple ? "⇧ click" : "Shift click"}
                  >
                    Open any link over the page you're on. Escape, and you're back.
                  </Part>
                  <p className="get">
                    <Download className="button big cta" />
                    Free, with no account.
                  </p>
                </div>
                <div className="col">
                  <Part
                    clip="split"
                    shows="A tab dragged from the sidebar onto the page, where the two sit side by side"
                    name="Split view"
                    keys="Drag a tab"
                  >
                    Drag a tab onto the page and the two sit side by side.
                  </Part>
                  <Part
                    clip="little"
                    shows="Little ilo opening as a small window, loading a page and moving it into the main window"
                    name="Little ilo"
                    keys={apple ? "⌥⌘N" : "Ctrl Alt N"}
                  >
                    A small window for a quick page. One key moves it into the main window.
                  </Part>
                </div>
              </div>
            </section>

            {/* The one thing no other browser does for you, on the brand's own green. */}
            <section className="order" aria-labelledby="order-title">
              <h2 id="order-title">
                It keeps itself <br />
                in order.
              </h2>
              <ul>
                <li>
                  <b>12 hours</b>
                  <span>A tab you haven't looked at in 12 hours moves to the archive.</span>
                </li>
                <li>
                  <b>Pinned</b>
                  <span>Pin what you want to keep, and it stays.</span>
                </li>
                <li>
                  <b>Backed up</b>
                  <span>
                    ilo keeps backups of your sidebar, and Help › Restore Earlier Sidebar brings one
                    back.
                  </span>
                </li>
              </ul>
            </section>

            {/* Every reason not to try it, answered. Each answer says no more than Help and Privacy
                do. */}
            <section className="why" id="why" aria-labelledby="why-title">
              <h2 id="why-title">No catch.</h2>
              <dl>
                <dt>Free</dt>
                <dd>
                  The whole browser, on every computer it runs on. No ads, no licence checks and no
                  paid features.
                </dd>
                <dt>Open</dt>
                <dd>
                  All of ilo that runs on your computer is under the{" "}
                  <a href={`${GITHUB}/blob/main/LICENSE`}>GPL-3.0</a>, and its source is{" "}
                  <a href={GITHUB}>on GitHub</a>.
                </dd>
                <dt>Private</dt>
                <dd>
                  It needs no account. It sends one anonymous count a day, with no ID, only if you
                  turn that on.
                </dd>
                <dt>Compatible</dt>
                <dd>
                  Chrome's extensions work, from the Chrome Web Store. uBlock Origin is built in.
                </dd>
                <dt>Moved in</dt>
                <dd>
                  Bring your tabs, bookmarks and history in from the other browsers on your
                  computer.
                </dd>
              </dl>
            </section>

            {/* The name, as tall as the room, with the download where its dot is. */}
            <section className="end" id="get" aria-labelledby="get-title">
              <div className="name">
                <div className="letters">
                  <svg viewBox="0 12 226 162" aria-hidden="true">
                    <rect x="7" y="70" width="24" height="100" rx="12" />
                    <rect x="67" y="20" width="24" height="150" rx="12" />
                    <path
                      fillRule="evenodd"
                      d="M171 70a50 50 0 1 0 0 100a50 50 0 1 0 0-100zm0 24a26 26 0 1 1 0 52a26 26 0 1 1 0-52z"
                    />
                  </svg>
                  <Download className="dot cta">
                    {(platform) => (
                      <span>
                        Download <small>for {platform.name}</small>
                      </span>
                    )}
                  </Download>
                </div>
                <div className="words">
                  <h2 id="get-title">a good tool.</h2>
                  <p>
                    ilo is <a href="/ilo">Esperanto for a tool</a>. This one is free, and yours to
                    keep.
                  </p>
                  <nav className="more" aria-label="Elsewhere">
                    <a href="/mac">Mac</a>
                    <a href="/windows">Windows</a>
                    <a href="/linux">Linux</a>
                    <a href="/changelog">Changelog</a>
                    <a href="/help">Help</a>
                    <a href="/privacy">Privacy</a>
                    <a href={GITHUB}>GitHub</a>
                  </nav>
                  <Download className="button big cta" />
                </div>
              </div>
              {/* The GPL's offer of source for the interface this page runs, and whose pages the
                  clips show. */}
              <p className="fine">
                This site runs ilo's own interface, built from{" "}
                <a href={import.meta.env.ILO_DEMO_SOURCE}>its source</a> under the GPL-3.0. The
                clips show Wikivoyage and Wikipedia (CC BY-SA) and OpenStreetMap (ODbL). No cookies,
                no tracking.
              </p>
            </section>
          </main>
        </div>
      </div>

      {/* The button follows a visitor who has left the page's own buttons, or the page's tab. It's
          the site's, laid over ilo's window, not part of ilo. */}
      <div className={follows ? "dock up" : "dock"} inert={!follows}>
        <Download className="button" brief />
      </div>
    </div>
  );
}
