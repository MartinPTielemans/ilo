import { createFileRoute } from "@tanstack/react-router";
import { FEED, pageHead } from "../seo";
import { TextPage } from "../components/text-page";
import { Clip } from "../components/clip";
import { SOURCE } from "../downloads";
import { RELEASES } from "../changelog";

export const Route = createFileRoute("/changelog")({
  head: () =>
    pageHead({
      path: "/changelog",
      title: "Changelog · ilo",
      description:
        "What changed in each version of ilo, the free, open-source browser, with a short clip of what's new.",
    }),
  component: Changelog,
});

function Changelog() {
  return (
    <TextPage
      title="Changelog"
      lead={
        <>
          What changed in each version of ilo. Every change is in{" "}
          <a href={`${SOURCE}/releases`}>the releases on GitHub</a>, and a feed reader can follow
          this page by <a href={FEED}>its feed</a>.
        </>
      }
    >
      {/* A release without its date isn't out, so the page doesn't name it yet. */}
      {RELEASES.filter((release) => release.date).map((release) => {
        const shown = release.notes.filter((note) => typeof note !== "string");
        const listed = release.notes.filter((note) => typeof note === "string");
        return (
          <section key={release.version} className="release" id={release.version}>
            <h2>
              <a href={`#${release.version}`}>{release.version}</a> <span>{release.date}</span>
            </h2>
            <p>{release.summary}</p>
            {shown.map((note) => (
              <figure key={note.clip}>
                <Clip name={note.clip} label={note.text} />
                <figcaption>{note.text}</figcaption>
              </figure>
            ))}
            {listed.length > 0 && (
              <ul>
                {listed.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
            )}
          </section>
        );
      })}
    </TextPage>
  );
}
