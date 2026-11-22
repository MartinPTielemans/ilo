import { PLATFORMS, downloadUrl, platformIdFor, runsIlo } from "./downloads";
import { SEND, SEND_LABEL, type Sent, mailFor } from "./send";

// Only the home page runs React in the browser. Every other page is its prerendered HTML and this
// script, a few lines at its end, which do the little those pages need: the download named for the
// visitor's computer, or sent there from a phone, and the changelog's clips.

type Downloads = Record<string, { name: string; url: string }>;

/**
 * The script itself. It's put in the page as its own source, so it can use nothing but what it's
 * given: `runs`, `idFor` and `mail` are runsIlo, platformIdFor and mailFor.
 */
export function reading(
  runs: (agent: string, touches: number) => boolean,
  idFor: (agent: string) => string | undefined,
  mail: (sent: Sent) => string,
  downloads: Downloads,
  send: { label: typeof SEND_LABEL; sent: Sent },
) {
  // A link marked data-get is the download: with no value the visitor's own, which the page gave
  // as the first platform's, and with a platform's that one's alone.
  const here = runs(navigator.userAgent, navigator.maxTouchPoints);
  const mine = downloads[idFor(navigator.userAgent) ?? ""];
  const address = document.querySelector<HTMLLinkElement>('link[rel="canonical"]')?.href;
  for (const link of document.querySelectorAll<HTMLAnchorElement>("a[data-get]")) {
    const own = !link.dataset.get;
    if (here) {
      if (own && mine) {
        link.href = mine.url;
        link.textContent = `Download for ${mine.name}`;
      }
      continue;
    }
    // On a phone it sends ilo to the visitor's computer: a platform's page sends its own address.
    const sent = { ...send.sent, url: (!own && address) || send.sent.url };
    link.href = mail(sent);
    link.textContent = own ? send.label.brief : send.label.full;
    link.addEventListener("click", (event) => {
      if (!("share" in navigator)) return;
      event.preventDefault();
      navigator.share(sent).catch(() => {});
    });
  }

  // A changelog clip: silent and looping while it's on screen, and fetched only then. A visitor who
  // prefers reduced motion sees its still until they press it; a press pauses or plays it for
  // anyone.
  const reduce = matchMedia("(prefers-reduced-motion: reduce)");
  for (const clip of document.querySelectorAll<HTMLButtonElement>("button[data-clip]")) {
    const video = clip.querySelector("video");
    if (!video) continue;
    // What the visitor chose by pressing it; until then, motion follows their preference.
    let chosen: boolean | undefined;
    let seen = false;
    const moving = () => chosen ?? !reduce.matches;
    const update = () => {
      if (moving() && seen) {
        if (!video.src) video.src = clip.dataset.clip ?? "";
        video.play().catch(() => {});
      } else {
        video.pause();
      }
      // The still shows instead of the video while it has never played.
      clip.dataset.still = String(!video.src);
      clip.setAttribute("aria-label", `${moving() ? "Pause" : "Play"}: ${clip.dataset.label}`);
    };
    // It starts loading a screen early, so it's playing by the time it's in view.
    new IntersectionObserver(
      ([entry]) => {
        seen = !!entry?.isIntersecting;
        update();
      },
      { rootMargin: "100% 0px" },
    ).observe(clip);
    reduce.addEventListener("change", update);
    clip.addEventListener("click", () => {
      chosen = !moving();
      update();
    });
  }
}

const DOWNLOADS: Downloads = Object.fromEntries(
  PLATFORMS.map((platform) => [platform.id, { name: platform.name, url: downloadUrl(platform) }]),
);

// Data goes in as JSON, with no "<" in it, so nothing in it can end the script's tag.
const data = (value: unknown) => JSON.stringify(value).replace(/</g, "\\u003c");

/** The script as the page carries it: the function and what it's given, as source. */
export const READING = `(${String(reading)})(${[runsIlo, platformIdFor, mailFor].map(String).join(",")},${data(DOWNLOADS)},${data({ label: SEND_LABEL, sent: SEND })})`;
