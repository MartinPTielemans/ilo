import { RELEASES, type Release } from "./changelog";
import { FEED, SITE } from "./seo";

// The changelog as an Atom feed, for anyone who'd rather be told of a release than come and look.
// The build writes it beside the pages (vite.config.ts), from the same notes the page shows.

const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/** A release's day, "29 October 2026", as a feed gives it: noon UTC, the same day in any zone. */
export function dayOf(date: string) {
  const [day, month, year] = date.split(" ");
  const number = MONTHS.indexOf(month ?? "") + 1;
  if (!number || !Number(day) || !Number(year)) throw new Error(`"${date}" isn't a release's day`);
  return `${year}-${String(number).padStart(2, "0")}-${day?.padStart(2, "0")}T12:00:00Z`;
}

const escape = (text: string) =>
  text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** A release's notes as the page lists them: the summary, then each change. */
function notes(release: Release) {
  const items = release.notes.map((note) => (typeof note === "string" ? note : note.text));
  return `<p>${escape(release.summary)}</p><ul>${items.map((item) => `<li>${escape(item)}</li>`).join("")}</ul>`;
}

/** The feed: every release that's out, newest first, each linking to its place in the changelog. */
export function changelogFeed(releases: Release[] = RELEASES) {
  const out = releases.filter((release) => release.date);
  const page = `${SITE}/changelog`;
  const entries = out.map((release) => {
    const url = `${page}#${release.version}`;
    return `  <entry>
    <id>${url}</id>
    <title>ilo ${release.version}</title>
    <link href="${url}"/>
    <updated>${dayOf(release.date ?? "")}</updated>
    <summary>${escape(release.summary)}</summary>
    <content type="html">${escape(notes(release))}</content>
  </entry>`;
  });
  return `<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>${page}</id>
  <title>ilo's changelog</title>
  <subtitle>What changed in each version of ilo, the free, open-source browser.</subtitle>
  <link href="${page}"/>
  <link rel="self" href="${SITE}${FEED}"/>
  <updated>${out[0]?.date ? dayOf(out[0].date) : "1970-01-01T00:00:00Z"}</updated>
  <author><name>ilo</name></author>
${entries.join("\n")}
</feed>
`;
}
