import type { ReactNode } from "react";
import { SOURCE as GITHUB } from "../downloads";
import { Wordmark } from "./wordmark";

/** Every page's foot: the wordmark and the tagline, where else to go, and `children` as its fine print. */
export function Foot({ children }: { children?: ReactNode }) {
  return (
    <footer className="foot">
      <a className="mark" href="/ilo" aria-label="What ilo means">
        <Wordmark />
        <span>a good tool.</span>
      </a>
      <nav className="where" aria-label="Elsewhere">
        <a href="/mac">Mac</a>
        <a href="/windows">Windows</a>
        <a href="/linux">Linux</a>
        <a href="/help">Help</a>
        <a href="/changelog">Changelog</a>
        <a href="/privacy">Privacy</a>
        <a href={GITHUB}>GitHub</a>
        <a href={`${GITHUB}/blob/main/LICENSE`}>GPL-3.0</a>
        <a href="https://github.com/MartinPTielemans/ilo-site">This site's code</a>
        <a href="/ilo">What ilo means</a>
      </nav>
      <p className="when">2026 · No cookies, no tracking</p>
      {children && <p className="fine">{children}</p>}
    </footer>
  );
}
