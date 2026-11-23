import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vite-plus/test";
import type { Release } from "../changelog";

vi.mock("../changelog", async (actual) => ({
  ...(await actual<typeof import("../changelog")>()),
  RELEASES: [
    {
      version: "9.9.9",
      summary: "Not out yet.",
      changes: [{ kind: "new", area: "Tabs", text: "Something still to come." }],
    },
    {
      version: "9.9.8",
      date: "2 November 2026",
      summary: "Out.",
      changes: [
        { kind: "fixed", area: "Tabs", text: "Something fixed.", only: ["linux"] },
        { kind: "new", area: "Tabs", text: "Something new." },
      ],
    },
  ] satisfies Release[],
}));

describe("the changelog page", () => {
  it("leaves out a release with no date, and lists one that's out by kind", async () => {
    const { Route } = await import("./changelog");
    // The route's component loads with its page, so it's loaded before it renders.
    const component = Route.options.component as NonNullable<typeof Route.options.component> & {
      preload?: () => Promise<void>;
    };
    await component.preload?.();
    const page = renderToStaticMarkup(createElement(component));
    expect(page).toContain('id="9.9.8"');
    expect(page).not.toContain("9.9.9");
    expect(page).not.toContain("Something still to come.");
    expect(page.indexOf("Something new.")).toBeLessThan(page.indexOf("Something fixed."));
    // What the platform filter reads (src/page.ts).
    expect(page).toContain('<li data-os="linux">');
  });
});
