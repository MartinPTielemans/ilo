import { expect, test } from "vite-plus/test";
import { framable } from "./live";

const ORIGIN = "https://ilo.example";

test("the site's own pages and the sites that allow it are framed", () => {
  expect(framable("https://ilo.example/ilo", ORIGIN)).toBe(true);
  expect(framable("https://en.wikipedia.org/wiki/Alfama", ORIGIN)).toBe(true);
  expect(framable("https://de.wikipedia.org/wiki/Bauhaus", ORIGIN)).toBe(true);
  expect(framable("https://doc.rust-lang.org/book/", ORIGIN)).toBe(true);
});

test("a page that refuses frames gets the note instead", () => {
  expect(framable("https://github.com/MartinPTielemans/ilo", ORIGIN)).toBe(false);
  expect(framable("https://duckduckgo.com/?q=rust", ORIGIN)).toBe(false);
  // A name that only ends like an allowed one isn't it.
  expect(framable("https://notwikipedia.org/", ORIGIN)).toBe(false);
});

test("OpenStreetMap is framed only as its embed", () => {
  expect(framable("https://www.openstreetmap.org/export/embed.html?bbox=1,2,3,4", ORIGIN)).toBe(
    true,
  );
  expect(framable("https://www.openstreetmap.org/#map=15/38.71/-9.13", ORIGIN)).toBe(false);
});

test("only web addresses are framed", () => {
  expect(framable("javascript:alert(1)", ORIGIN)).toBe(false);
  expect(framable("file:///etc/hosts", ORIGIN)).toBe(false);
  expect(framable("not a url", ORIGIN)).toBe(false);
});
