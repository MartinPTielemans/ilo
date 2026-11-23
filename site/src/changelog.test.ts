import { existsSync } from "node:fs";
import { describe, expect, it } from "vite-plus/test";
import { CLIPS, RELEASES, isOut, published } from "./changelog";

describe("the changelog", () => {
  it("has every file each clip it names needs", () => {
    const missing = RELEASES.flatMap((release) => release.changes)
      .flatMap((change) => (change.clip ? [change.clip] : []))
      .flatMap((name) => [`${name}.mp4`, `${name}.webp`, `${name}-still.webp`])
      .filter((file) => !existsSync(new URL(`../public${CLIPS}/${file}`, import.meta.url)));
    expect(missing).toEqual([]);
  });

  it("gives each release one anchor, its version, which the app's update notes link to", () => {
    const versions = RELEASES.map((release) => release.version);
    expect(new Set(versions).size).toBe(versions.length);
    for (const version of versions) expect(version).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("shows a release once it has its day and, where the deploy lists them, is published", () => {
    const tagged = { version: "0.2.5", date: "22 November 2026", summary: "", changes: [] };
    expect(isOut(tagged)).toBe(true);
    expect(isOut(tagged, "0.2.3 0.2.4")).toBe(false);
    expect(isOut(tagged, "0.2.4 0.2.5")).toBe(true);
    expect(isOut({ ...tagged, version: "0.2.50" }, "0.2.5")).toBe(false);
    expect(isOut({ version: "0.2.5", summary: "", changes: [] }, "0.2.5")).toBe(false);
  });

  it("marks a release a security update only when it has a security change to show for it", () => {
    for (const release of published()) {
      if (release.urgent) expect(release.changes.some((one) => one.kind === "security")).toBe(true);
    }
  });
});
