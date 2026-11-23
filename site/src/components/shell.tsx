import type { ReactNode } from "react";
import { SOURCE } from "../downloads";
import { STARS } from "../github";
import LAND from "../land.js?raw";
import { PAGE } from "../page";
import { DownloadLink } from "./download";
import { Wordmark } from "./wordmark";

/** The pages the bar names, which mark the one a visitor is on. */
export type Here = "/changelog" | "/help";

/**
 * What every page is made of: the land it stands on, the bar, the page's own `children`, and the
 * foot, with `fine` as its fine print and `after` for what a page puts over itself. No React runs in
 * the browser, so nothing here or in a page can keep state or run an effect: what a page does is in
 * its two scripts, src/page.ts and then src/land.js, which measures the page the first has finished.
 */
export function Shell({
  here,
  name,
  fine,
  after,
  children,
}: {
  here?: Here;
  /** The page's class on `main`, for what only it lays out. */
  name?: string;
  fine?: ReactNode;
  after?: ReactNode;
  children: ReactNode;
}) {
  const current = (path: Here) => (here === path ? "page" : undefined);
  return (
    <>
      <canvas id="land" aria-hidden />
      {/* Open on the land at the top of a page, and gathered into one dark pill once it moves. */}
      <header className="nav" id="nav">
        <div className="nav-in">
          <a className="nav-brand" href="/" aria-label="ilo, home" data-clear="0" data-pin="">
            <Wordmark />
          </a>
          <nav className="nav-links" aria-label="Site" data-clear="0" data-pin="">
            <a href="/changelog" aria-current={current("/changelog")}>
              Changelog
            </a>
            <a href="/help" aria-current={current("/help")}>
              Help
            </a>
            <a href={SOURCE} aria-label={`GitHub, ${STARS} stars`}>
              GitHub
              <span className="stars">
                <svg viewBox="0 0 16 16" aria-hidden>
                  <path
                    fill="currentColor"
                    d="M8 .9l2.1 4.5 4.9.6-3.6 3.4.9 4.9L8 11.9l-4.3 2.4.9-4.9L1 6l4.9-.6z"
                  />
                </svg>
                {STARS}
              </span>
            </a>
            <DownloadLink className="nav-cta" />
          </nav>
        </div>
      </header>
      <main className={name}>{children}</main>
      <footer className="foot">
        <nav aria-label="More">
          <a href="/mac">Mac</a>
          <a href="/windows">Windows</a>
          <a href="/linux">Linux</a>
          <a href="/help">Help</a>
          <a href="/changelog">Changelog</a>
          <a href="/privacy">Privacy</a>
          <a href={SOURCE}>GitHub</a>
          <a href={`${SOURCE}/blob/main/LICENSE`}>GPL-3.0</a>
          <a href={`${SOURCE}/tree/main/site`}>This site's code</a>
          <a href="/ilo">What ilo means</a>
        </nav>
        <p>ilo is in beta. This site uses no cookies and no tracking.</p>
        {fine && <p>{fine}</p>}
      </footer>
      {after}
      <script dangerouslySetInnerHTML={{ __html: PAGE }} />
      <script dangerouslySetInnerHTML={{ __html: LAND }} />
    </>
  );
}
