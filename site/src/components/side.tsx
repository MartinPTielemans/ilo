import type { ReactNode } from "react";

/**
 * The index beside a page of reading: its links stay in reach as the page scrolls, and the page's
 * script marks the one being read (src/page.ts). The links scroll in a list inside the panel, which
 * has no ground of its own: a scroller with a solid ground blacked out the land above it.
 */
export function Side({ label, children }: { label: string; children: ReactNode }) {
  return (
    <nav className="side" aria-label={label}>
      <div>{children}</div>
    </nav>
  );
}
