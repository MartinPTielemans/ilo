import { createFileRoute } from "@tanstack/react-router";
import { RELEASES, published } from "../changelog";
import { DownloadLink } from "../components/download";
import { Shell } from "../components/shell";
import { Wordmark } from "../components/wordmark";
import { PLATFORMS, PLATFORM_LIST, SOURCE, STAGE } from "../downloads";
import { CONTRIBUTORS, FACES, FACES_SHOWN, STARS } from "../github";
import { pageHead, structuredData } from "../seo";

const description = `ilo is a free, open-source browser on Chromium: every part of your life in its own Space, a sidebar that tidies itself and one bar for every command. ${STAGE.charAt(0).toUpperCase()}${STAGE.slice(1)} for ${PLATFORM_LIST}.`;

export const Route = createFileRoute("/")({
  head: () =>
    pageHead({
      path: "/",
      title: "ilo: a free, open-source browser you get to keep",
      description,
      data: structuredData(description),
    }),
  component: Home,
});

/**
 * One of ilo's clips: its window, whole, standing on the field. The page's script plays it while
 * it's on screen; a visitor who asked for no motion sees `still`, the moment its caption is about,
 * and gets its controls.
 */
function Clip({ name, wide, still = true }: { name: string; wide?: boolean; still?: boolean }) {
  const size = wide ? { width: 2400, height: 1350 } : { width: 1600, height: 900 };
  return (
    <video
      muted
      loop
      playsInline
      preload="none"
      {...size}
      poster={`/media/${name}.jpg`}
      data-still={still ? `/media/${name}-still.jpg` : undefined}
    >
      <source src={`/media/${name}.mp4`} type="video/mp4" />
    </video>
  );
}

/** The four small things: each with its name, how it's reached, and what it does. */
const PARTS = [
  {
    clip: "zap",
    name: "Zap",
    how: "in Boosts",
    what: "Click what you don’t want on a site and it stays gone next time.",
  },
  {
    clip: "peek",
    name: "Peek",
    how: "⇧ click a link",
    what: "Open any link over the page you’re on, then press Escape to go back.",
  },
  {
    clip: "split",
    name: "Split view",
    how: "drag a tab onto the page",
    what: "Two pages sit side by side in one window.",
  },
  {
    clip: "little",
    name: "Little ilo",
    how: "⌥⌘N",
    what: "A small window for a single page, moved into the main window with one key.",
  },
];

/** The Spaces a visitor can turn the page to, each named and with its field's lit tone (src/land.js). */
const SPACES = [
  ["Green", "#12A853"],
  ["Violet", "#7C60FF"],
  ["Red", "#F04A2C"],
  ["Blue", "#3F7DF5"],
  ["Pink", "#E8408F"],
] as const;

function Home() {
  const newest = published(RELEASES, import.meta.env.ILO_PUBLISHED)[0];
  const faces = CONTRIBUTORS.filter((one) => one.face).slice(0, FACES_SHOWN);
  return (
    <Shell
      name="home"
      after={
        <>
          {/* The page signs off with the mark, under the foot, and the switcher under that. */}
          <Wordmark className="wordmark giant" label="ilo" />
          {/* As at the foot of ilo's sidebar: pick a colour and it crosses the page from the dot pressed. */}
          <div className="spaces" id="switcher" role="group" aria-label="Space colour">
            {SPACES.map(([name, colour], index) => (
              <button key={name} type="button" aria-label={name} aria-pressed={index === 0}>
                <i style={{ background: colour }} />
              </button>
            ))}
          </div>
        </>
      }
    >
      <section className="hero" data-part="">
        <div className="hero-in">
          <Wordmark className="wordmark" label="ilo" />
          <div data-clear="0">
            <h1>a browser you get to keep.</h1>
            <p className="lede">
              Free and open source, with Spaces for your tabs, one bar for every command, no ads and
              no account.
            </p>
          </div>
          <DownloadLink className="btn" />
          <p className="note" data-clear="0">
            In beta for {PLATFORM_LIST}. iPhone on the way.
          </p>
        </div>
      </section>

      <section data-part="">
        <div className="wrap broad stack">
          <div className="head" data-clear="0">
            <h2>Switch Space and the window follows.</h2>
            <p className="lede">
              Put every part of your life in its own Space, with pinned tabs, today’s tabs and a
              colour you pick.
            </p>
          </div>
          <figure className="win wide">
            {/* This clip leads the page: at each swipe in it, every field moves on a colour as the
                window does. data-swipes is when, and which way; data-from where the fingers are. */}
            <video
              id="swipes"
              muted
              loop
              playsInline
              preload="none"
              width={2400}
              height={1350}
              poster="/media/spaces.jpg"
              data-from="0.069 0.578"
              data-swipes="1.50:1,3.90:1,6.30:1,8.70:-1,10.30:-1,11.90:-1"
            >
              <source src="/media/spaces.mp4" type="video/mp4" />
            </video>
            <figcaption data-clear="0">Switching Spaces swaps your tabs and colour.</figcaption>
          </figure>
        </div>
      </section>

      <section data-part="">
        <div className="wrap broad stack">
          <div className="mid" data-clear="0">
            <h2>One bar for every command</h2>
            <p className="lede">
              Type a few letters to find an open tab, open a page or run a command. ⌘T on Mac, Ctrl
              T elsewhere.
            </p>
          </div>
          <figure className="win wide">
            <Clip name="bar" wide />
            <figcaption data-clear="0">Tabs, pages and commands in one bar.</figcaption>
          </figure>
        </div>
      </section>

      <section data-part="">
        <div className="wrap broad stack">
          <div className="mid" data-clear="0">
            <h2>Four small things</h2>
            <p className="lede">Built right in, ready when you need them.</p>
          </div>
          <div className="parts">
            {PARTS.map((part) => (
              <figure key={part.clip} className="win part">
                <Clip name={part.clip} />
                <figcaption data-clear="0">
                  <b>{part.name}</b>
                  <kbd>{part.how}</kbd>
                  <span>{part.what}</span>
                </figcaption>
              </figure>
            ))}
          </div>
        </div>
      </section>

      <section data-part="">
        <div className="wrap stack">
          <div className="mid" data-clear="0">
            <h2>It keeps itself in order.</h2>
          </div>
          <dl className="list three">
            <div>
              <dt>Archive</dt>
              <dd>A tab you haven’t looked at in 12 hours moves to the archive.</dd>
            </div>
            <div>
              <dt>Pinned tabs</dt>
              <dd>Pin what you want to keep, and it stays right where you put it.</dd>
            </div>
            <div>
              <dt>Backups</dt>
              <dd>
                ilo keeps sidebar backups, and{" "}
                <span className="menu">Help › Restore Earlier Sidebar</span> brings one back.
              </dd>
            </div>
          </dl>
        </div>
      </section>

      <section data-part="">
        <div className="wrap two">
          <div data-clear="0">
            <h2>No one can fence it off.</h2>
          </div>
          <dl className="list rows">
            <div>
              <dt>Free</dt>
              <dd>
                The whole browser, on every computer it runs on. No ads, no licence checks and no
                paid features.
              </dd>
            </div>
            <div>
              <dt>Open</dt>
              <dd>
                All of ilo that runs on your computer is under the GPL-3.0. The source is on GitHub.
                <span className="counts">
                  <a href={`${SOURCE}/stargazers`}>
                    <span>
                      <b>{STARS}</b> stars
                    </span>
                  </a>
                  <a href={`${SOURCE}/graphs/contributors`}>
                    <span className="faces">
                      {faces.map(({ login, face }) => (
                        <img
                          key={login}
                          src={`${FACES}/${face}`}
                          alt=""
                          width={32}
                          height={32}
                          loading="lazy"
                        />
                      ))}
                      {CONTRIBUTORS.length > faces.length && (
                        <i>+{CONTRIBUTORS.length - faces.length}</i>
                      )}
                    </span>
                    <span>
                      <b>{CONTRIBUTORS.length}</b> contributors
                    </span>
                  </a>
                </span>
              </dd>
            </div>
            <div>
              <dt>Private</dt>
              <dd>
                No account needed. It sends one anonymous count a day, with no ID, only if you turn
                that on.
              </dd>
            </div>
            <div>
              <dt>Compatible</dt>
              <dd>Install extensions from the Chrome Web Store. uBlock Origin is built in.</dd>
            </div>
            <div>
              <dt>Switching</dt>
              <dd>
                Bring your tabs, bookmarks and history in from the other browsers on your computer.
              </dd>
            </div>
          </dl>
        </div>
      </section>

      <section className="end" data-part="">
        <div className="mid" data-clear="0">
          <h2>Pick a colour and move in.</h2>
          <p className="lede">
            Free for{" "}
            {PLATFORMS.map((platform, index) => (
              <span key={platform.id}>
                {index > 0 && (index === PLATFORMS.length - 1 ? " and " : ", ")}
                <a href={`/${platform.id}`}>{platform.name}</a>
              </span>
            ))}
            . No account needed.
          </p>
        </div>
        <DownloadLink className="btn" />
        {newest && (
          <p className="note new" data-clear="0">
            <a href="/changelog">
              ilo {newest.version}, {newest.date}
            </a>
          </p>
        )}
      </section>
    </Shell>
  );
}
