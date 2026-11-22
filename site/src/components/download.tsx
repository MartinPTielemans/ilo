import { PLATFORMS, type Platform, downloadUrl } from "../downloads";

/**
 * The download: the first platform's as the page is built, which the page's script (src/page.ts)
 * makes the visitor's own, or `platform`'s alone on that platform's page. On a phone, which can't
 * run ilo, the script has the same link send ilo's address to the visitor's computer.
 */
export function DownloadLink({ className, platform }: { className: string; platform?: Platform }) {
  const one = platform ?? PLATFORMS[0];
  return (
    <a className={className} href={downloadUrl(one)} data-get={platform?.id ?? ""}>
      {`Download for ${one.name}`}
    </a>
  );
}
