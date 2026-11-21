import { describe, expect, it } from "vite-plus/test";
import { PLATFORM_LIST, platformFor, runsIlo } from "./downloads";

const offered = (agent: string) => platformFor(agent).id;

describe("platformFor", () => {
  it("offers each computer its own download", () => {
    expect(
      offered(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36",
      ),
    ).toBe("mac");
    expect(
      offered(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36",
      ),
    ).toBe("windows");
    expect(offered("Mozilla/5.0 (X11; Linux x86_64; rv:144.0) Gecko/20100101 Firefox/144.0")).toBe(
      "linux",
    );
  });

  it("offers phones and Chromebooks the first download, not Linux's", () => {
    expect(
      offered(
        "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36",
      ),
    ).toBe("mac");
    expect(
      offered(
        "Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36",
      ),
    ).toBe("mac");
    expect(
      offered(
        "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1",
      ),
    ).toBe("mac");
  });
});

const MAC =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Safari/605.1.15";

describe("runsIlo", () => {
  it("is true on a computer", () => {
    expect(runsIlo(MAC, 0)).toBe(true);
    expect(runsIlo("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/141.0.0.0", 0)).toBe(true);
    // A laptop with a touch screen is still a computer.
    expect(runsIlo("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/141.0.0.0", 10)).toBe(true);
    expect(runsIlo("Mozilla/5.0 (X11; Linux x86_64; rv:144.0) Gecko/20100101 Firefox/144.0")).toBe(
      true,
    );
  });

  it("is false on a phone, a tablet and a Chromebook", () => {
    expect(runsIlo("Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) Mobile/15E148", 5)).toBe(
      false,
    );
    expect(
      runsIlo("Mozilla/5.0 (Linux; Android 10; K) Chrome/141.0.0.0 Mobile Safari/537.36"),
    ).toBe(false);
    expect(runsIlo("Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) Chrome/141.0.0.0")).toBe(false);
  });

  it("tells an iPad, which says it's a Mac, by its touch screen", () => {
    expect(runsIlo(MAC, 5)).toBe(false);
  });
});

describe("PLATFORM_LIST", () => {
  it("names the platforms as a sentence does", () => {
    expect(PLATFORM_LIST).toBe("Mac, Windows and Linux");
  });
});
