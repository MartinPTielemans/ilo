// What ilo is offered for, in the order the site names them. Each platform's file has one name in
// every release (the browser repo's release.yml names them), so GitHub always serves the newest
// release's at the same address. Taking a platform out of this list takes it out of the buttons and
// the copy. `note` is the short word the list of computers gives it, and `needs` what the line
// under the download button says it takes.
export const PLATFORMS = [
  {
    id: "mac",
    name: "Mac",
    file: "ilo-mac-arm64.dmg",
    note: "Apple silicon",
    needs: "macOS 14 Sonoma or later, on Apple silicon",
  },
  {
    id: "windows",
    name: "Windows",
    file: "ilo-windows-x64-setup.exe",
    note: "64-bit",
    needs: "64-bit Windows 10 or later",
  },
  {
    id: "linux",
    name: "Linux",
    file: "ilo-linux-x86_64.flatpak",
    note: "x64",
    needs: "A Flatpak, for x64",
  },
] as const;

export type Platform = (typeof PLATFORMS)[number];

/** How far along ilo is, on every platform alike, as a sentence says it. */
export const STAGE = "in beta";

// The browser's source, where its releases are published.
export const SOURCE = "https://github.com/MartinPTielemans/ilo";
const RELEASES = `${SOURCE}/releases`;

export const downloadUrl = (platform: Platform) => `${RELEASES}/latest/download/${platform.file}`;

/** The platforms as a sentence names them: "Mac, Windows and Linux". */
export const PLATFORM_LIST = PLATFORMS.map((platform) => platform.name)
  .join(", ")
  .replace(/, ([^,]*)$/, " and $1");

/**
 * Whether ilo runs on a visitor's device, from its user agent: not on a phone, a tablet or a
 * Chromebook. An iPad says it's a Mac, so its touch screen (`touches`, navigator.maxTouchPoints)
 * tells it from one. Every page's script carries this function's source (src/page.ts), so
 * it uses nothing outside itself.
 */
export function runsIlo(agent: string, touches = 0) {
  return !/Android|iPhone|iPad|CrOS/.test(agent) && !(/Macintosh/.test(agent) && touches > 1);
}

/**
 * The platform a visitor's computer runs, from its user agent, or none for a phone or anything else
 * ilo isn't made for. It uses nothing outside itself either, for the same reason.
 */
export function platformIdFor(agent: string): Platform["id"] | undefined {
  return /Android|iPhone|iPad|CrOS/.test(agent)
    ? undefined
    : /Windows/.test(agent)
      ? "windows"
      : /Linux|X11/.test(agent)
        ? "linux"
        : /Mac/.test(agent)
          ? "mac"
          : undefined;
}

/** The platform to offer a visitor: their computer's, or the first, as the prerendered page does. */
export function platformFor(agent: string): Platform {
  const id = platformIdFor(agent);
  return PLATFORMS.find((platform) => platform.id === id) ?? PLATFORMS[0];
}
