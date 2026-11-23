import { PLATFORMS, downloadUrl, platformIdFor, runsIlo } from "./downloads";
import { SEND, SEND_LABEL, type Sent, mailFor } from "./send";

// No page runs React in the browser. Each is its prerendered HTML and two scripts at its end: this
// one, which does the little a page needs (the download named for the visitor's computer, or sent
// there from a phone, the index that marks where the reader is, and the changelog's filter, folds and
// clips), and then src/land.js, which draws the land under the page this one has finished.

type Downloads = Record<string, { name: string; url: string }>;

/**
 * The script itself. It's put in the page as its own source, so it can use nothing but what it's
 * given: `runs`, `idFor` and `mail` are runsIlo, platformIdFor and mailFor.
 */
export function page(
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

  // The index beside a page marks the part being read: a release in the changelog, a section in Help.
  const marks = new Map<Element, HTMLAnchorElement>();
  for (const link of document.querySelectorAll<HTMLAnchorElement>(".side a")) {
    const part = document.getElementById(decodeURIComponent(link.hash.slice(1)));
    if (part) marks.set(part, link);
  }
  if (marks.size && "IntersectionObserver" in window) {
    const seen = new Set<Element>();
    const watch = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) seen.add(entry.target);
          else seen.delete(entry.target);
        }
        // The first of those in the band under the bar, in the page's own order.
        let read: HTMLAnchorElement | undefined;
        for (const [part, link] of marks) read ??= seen.has(part) ? link : undefined;
        for (const link of marks.values()) link.classList.toggle("here", link === read);
        // A long index scrolls by itself to keep that one in sight.
        const list = read?.parentElement;
        if (read && list) {
          const under = read.offsetTop + read.offsetHeight - list.clientHeight;
          if (read.offsetTop < list.scrollTop) list.scrollTop = read.offsetTop - 12;
          else if (under > list.scrollTop) list.scrollTop = under + 12;
        }
      },
      { rootMargin: "-96px 0px -55% 0px" },
    );
    for (const part of marks.keys()) watch.observe(part);
  }

  // The changelog. A folded release shows its Security and New changes, and its button the rest.
  const releases = [...document.querySelectorAll<HTMLElement>(".release")];
  const label = (release: HTMLElement) => {
    const more = release.querySelector<HTMLButtonElement>(".more");
    if (!more) return;
    const folded = release.classList.contains("folded");
    more.textContent = folded ? `Show all ${more.dataset.n} changes` : "Show highlights";
    more.setAttribute("aria-expanded", String(!folded));
  };
  for (const more of document.querySelectorAll<HTMLButtonElement>(".more")) {
    more.addEventListener("click", () => {
      const release = more.closest<HTMLElement>(".release");
      if (!release) return;
      const top = more.getBoundingClientRect().top;
      release.classList.toggle("folded");
      label(release);
      // Folding takes the lists above the button away, so the page moves to keep it under the pointer.
      if (release.classList.contains("folded")) {
        scrollBy({ top: more.getBoundingClientRect().top - top, behavior: "instant" });
      }
    });
  }
  // A link to a version, as the app's What's New gives, opens that release whole.
  const unfold = () => {
    const release = document.getElementById(decodeURIComponent(location.hash.slice(1)));
    if (release?.classList.contains("folded")) {
      release.classList.remove("folded");
      label(release);
    }
  };
  if (releases.length) {
    addEventListener("hashchange", unfold);
    unfold();
  }

  // A platform picked: changes for another platform alone step aside, an area or a kind left with
  // none goes with them, and each count says what's left.
  const picks = [...document.querySelectorAll<HTMLButtonElement>(".filter button")];
  for (const pick of picks) {
    pick.addEventListener("click", () => {
      const os = pick.dataset.os ?? "";
      for (const each of picks) each.setAttribute("aria-pressed", String(each === pick));
      for (const release of releases) {
        let left = 0;
        for (const kind of release.querySelectorAll<HTMLElement>(".kind")) {
          let inKind = 0;
          for (const area of kind.querySelectorAll<HTMLElement>("dl > div")) {
            let inArea = 0;
            for (const change of area.querySelectorAll<HTMLElement>("li")) {
              const only = change.dataset.os;
              change.hidden = !!os && !!only && !only.split(" ").includes(os);
              if (!change.hidden) inArea++;
            }
            area.hidden = !inArea;
            inKind += inArea;
          }
          kind.hidden = !inKind;
          const count = kind.querySelector("h3 span");
          if (count) count.textContent = String(inKind);
          left += inKind;
        }
        const more = release.querySelector<HTMLButtonElement>(".more");
        if (more) more.dataset.n = String(left);
        label(release);
      }
    });
  }

  // A changelog clip stands up at a size to read, from a press on it or on its corner button: over
  // the page on a computer, and in the phone's own player on a phone, where the page is no wider than
  // the clip already is. Escape, a press beside it, on it or on its close button puts it back.
  const view = document.querySelector<HTMLDialogElement>("dialog.view");
  const stage = view?.querySelector<HTMLElement>(".stage");
  const words = view?.querySelector<HTMLElement>("p");
  if (view && stage && words) {
    const reduce = matchMedia("(prefers-reduced-motion: reduce)");
    const narrow = matchMedia("(max-width: 900px)");
    const stand = (figure: HTMLElement) => {
      const from = figure.querySelector("video");
      if (!from) return;
      if (narrow.matches) {
        // iPhone's Safari has its own name for it, and no other, and brings its own controls. Elsewhere
        // the clip has its controls for as long as it's whole, which is the way back out of it.
        const phone = from as HTMLVideoElement & { webkitEnterFullscreen?: () => void };
        if (phone.requestFullscreen) {
          const had = from.controls;
          from.controls = true;
          const back = () => {
            if (document.fullscreenElement === from) return;
            from.controls = had;
            from.removeEventListener("fullscreenchange", back);
          };
          from.addEventListener("fullscreenchange", back);
          phone.requestFullscreen().catch(back);
        } else phone.webkitEnterFullscreen?.();
        return;
      }
      const video = document.createElement("video");
      video.muted = true;
      video.loop = true;
      video.playsInline = true;
      video.poster = from.poster;
      video.src = from.currentSrc || from.querySelector("source")?.src || "";
      video.setAttribute("aria-label", from.getAttribute("aria-label") ?? "");
      // It goes on from where the small one is, unless the visitor asked for no motion.
      if (reduce.matches) video.controls = true;
      else {
        video.autoplay = true;
        video.currentTime = from.currentTime;
      }
      stage.replaceChildren(video);
      words.innerHTML = figure.querySelector("figcaption")?.innerHTML ?? "";
      view.showModal();
    };
    for (const figure of document.querySelectorAll<HTMLElement>(".clips figure")) {
      figure.querySelector(".zoom")?.addEventListener("click", () => stand(figure));
      figure.querySelector("video")?.addEventListener("click", (event) => {
        // A clip with its own controls keeps its presses for them.
        if (!(event.currentTarget as HTMLVideoElement).controls) stand(figure);
      });
    }
    view.addEventListener("click", (event) => {
      const on = event.target as HTMLElement;
      const clip = on instanceof HTMLVideoElement && !on.controls;
      if (on === view || on.closest(".shut") || clip) view.close();
    });
    view.addEventListener("close", () => {
      const video = stage.querySelector("video");
      if (video) {
        video.pause();
        video.removeAttribute("src");
        video.load();
      }
      stage.replaceChildren();
    });
  }
}

const DOWNLOADS: Downloads = Object.fromEntries(
  PLATFORMS.map((platform) => [platform.id, { name: platform.name, url: downloadUrl(platform) }]),
);

// Data goes in as JSON, with no "<" in it, so nothing in it can end the script's tag.
const data = (value: unknown) => JSON.stringify(value).replace(/</g, "\\u003c");

/** The script as the page carries it: the function and what it's given, as source. */
export const PAGE = `(${String(page)})(${[runsIlo, platformIdFor, mailFor].map(String).join(",")},${data(DOWNLOADS)},${data({ label: SEND_LABEL, sent: SEND })})`;
