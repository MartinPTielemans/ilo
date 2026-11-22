import { describe, expect, it } from "vite-plus/test";
import { RELEASES } from "./changelog";
import { changelogFeed, dayOf } from "./feed";

describe("the changelog's feed", () => {
  it("reads every release's day", () => {
    expect(dayOf("1 November 2026")).toBe("2026-11-01T12:00:00Z");
    expect(dayOf("29 October 2026")).toBe("2026-10-29T12:00:00Z");
    expect(() => dayOf("November 2026")).toThrow();
    for (const release of RELEASES) if (release.date) dayOf(release.date);
  });

  it("has an entry for each release that's out, and none for one still to come", () => {
    const feed = changelogFeed([
      { version: "9.9.9", summary: "Soon.", changes: [] },
      {
        version: "0.0.2",
        date: "2 November 2026",
        summary: "Tabs & <Spaces>.",
        changes: [
          { kind: "fixed", area: "Tabs", text: "A fix.", only: ["linux"] },
          { kind: "new", area: "Spaces", text: "A clip's change.", clip: "x" },
        ],
      },
    ]);
    expect(feed).not.toContain("9.9.9");
    expect(feed).toContain("<id>https://ilobrowser.com/changelog#0.0.2</id>");
    expect(feed).toContain("<updated>2026-11-02T12:00:00Z</updated>");
    expect(feed).toContain("<summary>Tabs &amp; &lt;Spaces&gt;.</summary>");
    // The notes are HTML inside the XML, so they're escaped twice.
    expect(feed).toContain("Tabs &amp;amp; &amp;lt;Spaces&amp;gt;.");
    // What's new comes before what's fixed, whatever order the release lists them in, and a change
    // for one platform says which.
    expect(feed).toContain(
      "&lt;h3&gt;New&lt;/h3&gt;&lt;ul&gt;&lt;li&gt;A clip's change.&lt;/li&gt;&lt;/ul&gt;&lt;h3&gt;Fixed&lt;/h3&gt;&lt;ul&gt;&lt;li&gt;A fix. (Linux)&lt;/li&gt;",
    );
  });

  it("leaves out a dated release the deploy doesn't list as published", () => {
    const releases = [
      { version: "0.0.3", date: "3 November 2026", summary: "Tagged.", changes: [] },
      { version: "0.0.2", date: "2 November 2026", summary: "Out.", changes: [] },
    ];
    const feed = changelogFeed(releases, "0.0.1 0.0.2");
    expect(feed).not.toContain("0.0.3");
    expect(feed).toContain("changelog#0.0.2");
    expect(changelogFeed(releases)).toContain("changelog#0.0.3");
  });

  it("says no release's summary twice", () => {
    for (const release of RELEASES) {
      expect(release.changes.map((change) => change.text)).not.toContain(release.summary);
    }
  });
});
