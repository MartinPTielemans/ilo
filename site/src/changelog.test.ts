import { existsSync } from "node:fs";
import { describe, expect, it } from "vite-plus/test";
import { CLIPS, RELEASES } from "./changelog";

describe("the changelog", () => {
  it("has every file each clip it names needs", () => {
    const missing = RELEASES.flatMap((release) => release.notes)
      .flatMap((note) => (typeof note === "string" ? [] : [note.clip]))
      .flatMap((name) => [`${name}.mp4`, `${name}.webp`, `${name}-still.webp`])
      .filter((file) => !existsSync(new URL(`../public${CLIPS}/${file}`, import.meta.url)));
    expect(missing).toEqual([]);
  });

  it("gives each release one anchor, its version, which the app's update notes link to", () => {
    const versions = RELEASES.map((release) => release.version);
    expect(new Set(versions).size).toBe(versions.length);
    for (const version of versions) expect(version).toMatch(/^\d+\.\d+\.\d+$/);
  });
});
