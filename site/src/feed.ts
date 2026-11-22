import { KINDS, type Release, published } from "./changelog";
import { PLATFORMS } from "./downloads";
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

/** A release's notes as the page lists them: the summary, then its changes by kind, each naming the platforms it's for when it isn't for all. */
function notes(release: Release) {
  const lists = KINDS.flatMap(({ kind, label }) => {
    const items = release.changes
      .filter((change) => change.kind === kind)
      .map((change) => {
        const only = change.only?.map((id) => PLATFORMS.find((one) => one.id === id)?.name ?? id);
        return `<li>${escape(change.text)}${only ? ` (${only.join(", ")})` : ""}</li>`;
      });
    return items.length ? [`<h3>${label}</h3><ul>${items.join("")}</ul>`] : [];
  });
  return `<p>${escape(release.summary)}</p>${lists.join("")}`;
}

/** The feed: every release that's out, newest first, each linking to its place in the changelog. */
export function changelogFeed(releases?: Release[], listed?: string) {
  const out = published(releases, listed);
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
