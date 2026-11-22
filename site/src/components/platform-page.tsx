import type { ReactNode } from "react";
import { RELEASES, isOut } from "../changelog";
import { PLATFORMS, type Platform } from "../downloads";
import { DownloadLink } from "./download";
import { TextPage } from "./text-page";

type Id = Platform["id"];

/** A real capture of ilo on a platform: its picture at 1× and 2×, what it shows, and whose pages. */
type Capture = { name: string; alt: string; credit: string };

/**
 * What each platform's page says beyond what every one does: how to install, what differs, and
 * `capture`, ilo on that platform. A platform with no capture of its own shows none: another
 * platform's window isn't what a visitor there would get.
 */
const COPY: Record<Id, { install: ReactNode; differs: ReactNode; capture?: Capture }> = {
  mac: {
    capture: {
      name: "space-lisbon",
      alt: "ilo on a Mac, in a Space called Lisbon: a warm brown sidebar with a map pinned, three tabs for the trip, and Wikivoyage's guide to Lisbon open.",
      credit: "The capture shows Wikivoyage (CC BY-SA).",
    },
    install: <p>Open the download and drag ilo into Applications.</p>,
    differs: (
      <p>
        Netflix and other protected video play in macOS's own web engine, so ilo downloads nothing
        for them.
      </p>
    ),
  },
  windows: {
    install: (
      <p>
        Run the installer, which installs ilo for you alone and asks for no administrator. The
        installer isn't signed yet, so Windows may say it protected your PC: choose More info, then
        Run anyway.
      </p>
    ),
    differs: (
      <p>
        Netflix and other protected video play with Windows' own PlayReady, so ilo downloads nothing
        for them.
      </p>
    ),
  },
  linux: {
    install: (
      <>
        <p>
          ilo is a Flatpak. Open the download with your software centre, or run{" "}
          <code>flatpak install --user ilo-linux-x86_64.flatpak</code> where you saved it. Flatpak
          fetches the runtime ilo runs on from Flathub if you don't have it.
        </p>
        <p>ilo runs natively on Wayland.</p>
      </>
    ),
    differs: (
      <p>
        Netflix and other protected video need Google's Widevine module, which ilo doesn't ship.
        Once you allow protected video, ilo downloads it for you, and Settings removes it again.
      </p>
    ),
  },
};

const platform = (id: Id) => PLATFORMS.find((one) => one.id === id)!;

/** A platform's page: what ilo needs there, its download, and how to install and update it. */
export function PlatformPage({ id }: { id: Id }) {
  const one = platform(id);
  const copy = COPY[id];
  // The newest version the changelog has, with the day it came out.
  const newest = RELEASES.find((release) => isOut(release, import.meta.env.ILO_PUBLISHED));
  const updates =
    id === "linux"
      ? "Flatpak updates ilo with your other Flatpak apps."
      : "ilo updates itself. To look now, choose Check for Updates… in the ilo menu, or type “update” in the command bar.";
  return (
    <TextPage
      title={`ilo for ${one.name}`}
      lead={
        <>
          A free, open-source browser, {one.stage} for {one.name}: every part of your life in its
          own Space, a sidebar that tidies itself and one bar for every command, on Chromium, so
          your sites and Chrome's extensions work.
        </>
      }
      fine={copy.capture?.credit}
    >
      <p className="fetch">
        <DownloadLink className="button" platform={one} />
        {newest && (
          <a href="/changelog">
            ilo {newest.version}, {newest.date}
          </a>
        )}
      </p>

      {copy.capture && (
        <figure className="capture">
          <img
            src={`/media/${copy.capture.name}-1x.webp`}
            srcSet={`/media/${copy.capture.name}-1x.webp 1x, /media/${copy.capture.name}.webp 2x`}
            width={1280}
            height={800}
            alt={copy.capture.alt}
          />
        </figure>
      )}

      <section>
        <h2>What it needs</h2>
        <p>{one.needs}.</p>
      </section>

      <section>
        <h2>Installing</h2>
        {copy.install}
        <p>
          To bring your tabs, bookmarks and history from another browser, choose Import from Other
          Browsers… in the ilo menu, or type “import” in the command bar.
        </p>
      </section>

      <section>
        <h2>Updates</h2>
        <p>
          {updates} What changed is in the <a href="/changelog">changelog</a>.
        </p>
      </section>

      <section>
        <h2>Privacy</h2>
        <p>
          ilo blocks ads and trackers with uBlock Origin, built in, and needs no account. It sends
          one anonymous count a day only if you turn that on, and crash reports only if you say so.{" "}
          <a href="/privacy">Privacy</a> lists every service it talks to.
        </p>
        {copy.differs}
      </section>

      <section>
        <h2>Learning it</h2>
        <p>
          <a href="/help">Help</a> is the short version of how ilo works: Spaces, tabs, the command
          bar, split view and extensions.
        </p>
      </section>
    </TextPage>
  );
}
