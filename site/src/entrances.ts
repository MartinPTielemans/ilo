import { useEffect } from "react";
import { scroller } from "./live";

// Each part rises once as it comes into view, the parts arriving together one after another. The
// first screen is left alone, so it's there at once, and the classes are only added here, so
// without script nothing is hidden. Only parts whose class React never changes are listed.
const PARTS = [
  ".look .lead",
  ".clips figure",
  ".look .get",
  ".order h2",
  ".order li",
  ".why h2",
  ".why dt",
  ".why dd",
];

export function useEntrances() {
  useEffect(() => {
    const enter = new IntersectionObserver(
      (entries) => {
        let order = 0;
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          const part = entry.target as HTMLElement;
          // A long arrival reads as a page still loading, so the fourth part on comes with the third.
          part.style.setProperty("--i", String(Math.min(order++, 3)));
          part.classList.add("in");
          enter.unobserve(part);
        }
      },
      { root: scroller(), rootMargin: "0px 0px -8% 0px" },
    );
    for (const part of document.querySelectorAll(PARTS.join(","))) {
      part.classList.add("rise");
      enter.observe(part);
    }
    return () => enter.disconnect();
  }, []);
}
