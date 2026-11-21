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
      { version: "9.9.9", summary: "Soon.", notes: [] },
      {
        version: "0.0.2",
        date: "2 November 2026",
        summary: "Tabs & <Spaces>.",
        notes: ["A note.", { text: "A clip's note.", clip: "x" }],
      },
    ]);
    expect(feed).not.toContain("9.9.9");
    expect(feed).toContain("<id>https://ilobrowser.com/changelog#0.0.2</id>");
    expect(feed).toContain("<updated>2026-11-02T12:00:00Z</updated>");
    expect(feed).toContain("<summary>Tabs &amp; &lt;Spaces&gt;.</summary>");
    // The notes are HTML inside the XML, so they're escaped twice.
    expect(feed).toContain("&lt;li&gt;A clip's note.&lt;/li&gt;");
    expect(feed).toContain("Tabs &amp;amp; &amp;lt;Spaces&amp;gt;.");
  });

  it("says no release's summary twice", () => {
    for (const release of RELEASES) {
      const notes = release.notes.map((note) => (typeof note === "string" ? note : note.text));
      expect(notes).not.toContain(release.summary);
    }
  });
});
