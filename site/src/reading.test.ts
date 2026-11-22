import { runInNewContext } from "node:vm";
import { describe, expect, it } from "vite-plus/test";
import { downloadUrl, PLATFORMS } from "./downloads";
import { READING } from "./reading";
import { SEND, SEND_LABEL } from "./send";

const WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/141.0.0.0 Safari/537.36";
const IPHONE = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) Mobile/15E148 Safari/604.1";

/** A download link as the page gives it: the first platform's, or `get`'s alone. */
function link(get: string) {
  let pressed: ((event: { preventDefault(): void }) => void) | undefined;
  return {
    dataset: { get },
    href: "first",
    textContent: "Download for Mac",
    addEventListener: (_: string, press: typeof pressed) => (pressed = press),
    press: (event: { preventDefault(): void }) => pressed?.(event),
  };
}

/**
 * Runs the script as a page does, as source with nothing of this module in reach, on a page with
 * the top's download and the Linux page's own. A name the script took from outside itself would
 * throw here.
 */
function run(agent: string, navigator: object = {}) {
  const links = [link(""), link("linux")];
  runInNewContext(READING, {
    navigator: { userAgent: agent, maxTouchPoints: 0, ...navigator },
    document: {
      querySelector: () => ({ href: "https://ilobrowser.com/linux" }),
      querySelectorAll: (selector: string) => (selector.startsWith("a") ? links : []),
    },
    matchMedia: () => ({ matches: false, addEventListener() {} }),
  });
  return links;
}

describe("the reading pages' script", () => {
  it("names the top's download for the visitor's computer, and leaves a platform's own alone", () => {
    const [top, own] = run(WINDOWS);
    const windows = PLATFORMS.find((platform) => platform.id === "windows")!;
    expect(top.textContent).toBe("Download for Windows");
    expect(top.href).toBe(downloadUrl(windows));
    expect(own.textContent).toBe("Download for Mac");
    expect(own.href).toBe("first");
  });

  it("sends ilo to the visitor's computer from a phone, a platform's page by its own address", () => {
    const [top, own] = run(IPHONE);
    expect(top.textContent).toBe(SEND_LABEL.brief);
    expect(top.href).toContain(encodeURIComponent(SEND.url));
    expect(own.textContent).toBe(SEND_LABEL.full);
    expect(own.href).toMatch(/^mailto:\?subject=ilo&body=/);
    expect(own.href).toContain(encodeURIComponent("https://ilobrowser.com/linux"));
  });

  it("opens the phone's share sheet where there is one, and the mail where there isn't", () => {
    const shared: unknown[] = [];
    let prevented = 0;
    const event = { preventDefault: () => prevented++ };
    const share = (sent: unknown) => (shared.push(sent), Promise.resolve());

    run(IPHONE, { share })[0].press(event);
    expect(shared).toEqual([SEND]);
    expect(prevented).toBe(1);

    run(IPHONE)[0].press(event);
    expect(prevented).toBe(1);
  });
});
