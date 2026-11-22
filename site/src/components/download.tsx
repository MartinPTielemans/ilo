import { type MouseEvent, type ReactNode, useEffect, useState } from "react";
import { PLATFORMS, type Platform, downloadUrl, platformFor, runsIlo } from "../downloads";
import { SEND, SEND_LABEL, mailFor } from "../send";

/**
 * What the visitor is on: the platform to offer them, and whether ilo runs on their device at all.
 * The prerendered page offers the first platform, as if it did.
 */
export function useVisitor() {
  const [platform, setPlatform] = useState<Platform>(PLATFORMS[0]);
  const [runs, setRuns] = useState(true);
  useEffect(() => {
    setPlatform(platformFor(navigator.userAgent));
    setRuns(runsIlo(navigator.userAgent, navigator.maxTouchPoints));
  }, []);
  return { platform, runs };
}

/** Opens the phone's share sheet where there is one; without it the link's mail opens instead. */
function share(event: MouseEvent<HTMLAnchorElement>) {
  if (!("share" in navigator)) return;
  event.preventDefault();
  // A visitor who closes the sheet has chosen, so its refusal is no error.
  navigator.share(SEND).catch(() => {});
}

/**
 * The download for the visitor's computer, named for it. On a phone, which can't run ilo, the same
 * link sends ilo's address to their computer instead, in fewer words where `brief` says there's
 * little room. `children` draws the link its own way from the platform, where the words alone
 * won't do; on a phone such a link is named by its label alone.
 */
export function Download({
  className,
  brief,
  children,
}: {
  className: string;
  brief?: boolean;
  children?: (platform: Platform) => ReactNode;
}) {
  const { platform, runs } = useVisitor();
  const send = brief ? SEND_LABEL.brief : SEND_LABEL.full;
  return runs ? (
    <a className={className} href={downloadUrl(platform)}>
      {children ? children(platform) : `Download for ${platform.name}`}
    </a>
  ) : (
    <a className={className} href={mailFor(SEND)} onClick={share} aria-label={send}>
      {children ? null : send}
    </a>
  );
}

/**
 * The download on a page with no React in the browser: the first platform's, which the page's
 * script (src/reading.ts) makes the visitor's, or `platform`'s alone on that platform's page.
 */
export function DownloadLink({ className, platform }: { className: string; platform?: Platform }) {
  const one = platform ?? PLATFORMS[0];
  return (
    <a className={className} href={downloadUrl(one)} data-get={platform?.id ?? ""}>
      {`Download for ${one.name}`}
    </a>
  );
}
