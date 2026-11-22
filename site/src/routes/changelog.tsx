import { createFileRoute } from "@tanstack/react-router";
import { CLIPS, type Change, FOLD, KINDS, RELEASES, type Release, published } from "../changelog";
import { Shell } from "../components/shell";
import { Side } from "../components/side";
import { PLATFORMS, SOURCE } from "../downloads";
import { FEED, pageHead } from "../seo";

export const Route = createFileRoute("/changelog")({
  head: () =>
    pageHead({
      path: "/changelog",
      title: "Changelog · ilo",
      description:
        "What changed in each version of ilo, a free, open-source browser, with a short clip of what's new.",
    }),
  component: Changelog,
});

const name = (id: string) => PLATFORMS.find((platform) => platform.id === id)?.name ?? id;

/** A release's day as the index has room for it: "21 Nov". */
const short = (date: string) => {
  const [day, month] = date.split(" ");
  return `${day} ${month?.slice(0, 3)}`;
};

/** A change in a list, marked with the platforms it's for when it isn't for all, which the filter reads. */
function Listed({ change }: { change: Change }) {
  return (
    <li data-os={change.only?.join(" ")}>
      {change.text}
      {change.only && (
        <>
          {" "}
          <span className="os">{change.only.map(name).join(", ")}</span>
        </>
      )}
    </li>
  );
}

/**
 * A release: its summary, the changes that have a clip, then the rest by kind and, in each kind, by
 * the part of ilo they're in. A long one that isn't the newest folds its Changed and Fixed behind a
 * button (src/page.ts), so the page reads as what's new, with everything else a press away.
 */
function ReleasePanel({ release, newest }: { release: Release; newest: boolean }) {
  const shown = release.changes.filter((change) => change.clip);
  const listed = release.changes.filter((change) => !change.clip);
  const lead = (change: Change) => change.kind === "security" || change.kind === "new";
  const folds = !newest && listed.length > FOLD && listed.some(lead) && !listed.every(lead);
  return (
    <article className={`panel release${folds ? " folded" : ""}`} id={release.version}>
      <header>
        <h2>
          <a href={`#${release.version}`}>{release.version}</a>
        </h2>
        <time>{release.date}</time>
        {release.urgent && <span className="tag">Security update</span>}
      </header>
      <p className="sum">{release.summary}</p>
      {shown.length > 0 && (
        <div className={shown.length > 1 ? "clips pair" : "clips"}>
          {shown.map((change) => (
            <figure key={change.clip}>
              <div className="clip">
                <video
                  muted
                  loop
                  playsInline
                  preload="none"
                  width={1280}
                  height={720}
                  poster={`${CLIPS}/${change.clip}.webp`}
                  data-still={`${CLIPS}/${change.clip}-still.webp`}
                  aria-label={change.text}
                >
                  <source src={`${CLIPS}/${change.clip}.mp4`} type="video/mp4" />
                </video>
                <button
                  type="button"
                  className="zoom"
                  aria-label={`Enlarge the clip: ${change.area}`}
                >
                  <svg viewBox="0 0 16 16" aria-hidden>
                    <path
                      d="M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.8"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                </button>
              </div>
              <figcaption>
                <b>{change.area}</b>
                {change.text}
              </figcaption>
            </figure>
          ))}
        </div>
      )}
      {KINDS.map(({ kind, label }) => {
        const mine = listed.filter((change) => change.kind === kind);
        if (!mine.length) return null;
        // By area, in the order each first comes up.
        const areas = [...new Set(mine.map((change) => change.area))];
        return (
          <div
            key={kind}
            className={`kind ${kind}`}
            data-more={folds && !lead(mine[0]!) ? "" : undefined}
          >
            <h3>
              {label} <span>{mine.length}</span>
            </h3>
            <dl>
              {areas.map((area) => (
                <div key={area}>
                  <dt>{area}</dt>
                  <dd>
                    <ul>
                      {mine
                        .filter((change) => change.area === area)
                        .map((change) => (
                          <Listed key={change.text} change={change} />
                        ))}
                    </ul>
                  </dd>
                </div>
              ))}
            </dl>
          </div>
        );
      })}
      {folds && (
        <button type="button" className="more" aria-expanded="false" data-n={listed.length}>
          Show all {listed.length} changes
        </button>
      )}
    </article>
  );
}

function Changelog() {
  const releases = published(RELEASES, import.meta.env.ILO_PUBLISHED);
  return (
    <Shell
      here="/changelog"
      after={
        // Where a clip stands up at a size to read (src/page.ts).
        <dialog className="view" aria-label="Clip">
          <button type="button" className="shut" aria-label="Close">
            <svg viewBox="0 0 14 14" aria-hidden>
              <path
                d="M2 2l10 10M12 2L2 12"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
          </button>
          <div className="stage" />
          <p />
        </dialog>
      }
    >
      <div className="top">
        <div data-clear="0">
          <h1>Changelog</h1>
          <p className="lede">
            What changed in each version of ilo. Also in{" "}
            <a href={`${SOURCE}/releases`}>GitHub releases</a> and as <a href={FEED}>a feed</a>.
          </p>
        </div>
      </div>
      <div className="cols">
        <Side label="Versions">
          {releases.map((release) => (
            <a
              key={release.version}
              href={`#${release.version}`}
              className={release.urgent ? "sec" : undefined}
            >
              <b>{release.version}</b>
              <span>{short(release.date ?? "")}</span>
            </a>
          ))}
        </Side>
        <div>
          <div className="filter" role="group" aria-label="Platform">
            <span>Changes for</span>
            <button type="button" aria-pressed="true" data-os="">
              All
            </button>
            {PLATFORMS.map((platform) => (
              <button key={platform.id} type="button" aria-pressed="false" data-os={platform.id}>
                {platform.name}
              </button>
            ))}
          </div>
          {releases.map((release, index) => (
            <ReleasePanel key={release.version} release={release} newest={index === 0} />
          ))}
        </div>
      </div>
    </Shell>
  );
}
