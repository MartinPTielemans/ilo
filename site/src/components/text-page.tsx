import type { ReactNode } from "react";
import { READING } from "../reading";
import { DownloadLink } from "./download";
import { Foot } from "./foot";
import { Wordmark } from "./wordmark";

/**
 * A page of reading, as the app's Help menu and Settings open them: the wordmark home, the other
 * reading pages and the download, then `title`, `lead` under it, and the page's sections, with
 * `fine` as the foot's fine print. No React runs on it in the browser, so nothing here or in its
 * sections can keep state or run an effect: what such a page does is in its script (src/reading.ts).
 */
export function TextPage({
  title,
  lead,
  fine,
  children,
}: {
  title: string;
  lead: ReactNode;
  fine?: ReactNode;
  children: ReactNode;
}) {
  return (
    <>
      <header className="top">
        <a className="brand" href="/" aria-label="ilo, home">
          <Wordmark />
        </a>
        <nav className="links" aria-label="Pages">
          <a href="/help">Help</a>
          <a href="/changelog">Changelog</a>
          <a href="/privacy">Privacy</a>
          <DownloadLink className="button" />
        </nav>
      </header>
      <main className="text">
        <h1>{title}</h1>
        <p className="lead">{lead}</p>
        {children}
      </main>
      <Foot>{fine}</Foot>
      <script dangerouslySetInnerHTML={{ __html: READING }} />
    </>
  );
}
