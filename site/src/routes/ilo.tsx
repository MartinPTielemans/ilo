import { createFileRoute } from "@tanstack/react-router";
import { pageHead } from "../seo";
import { Wordmark } from "../components/wordmark";

export const Route = createFileRoute("/ilo")({
  head: () =>
    pageHead({
      path: "/ilo",
      title: "What ilo means",
      description: "ilo is Esperanto for a tool, and the name of a free, open-source web browser.",
    }),
  component: Meaning,
});

// The word's entry, as a dictionary gives it.
function Meaning() {
  return (
    <main className="meaning">
      <article className="word">
        <h1>
          <Wordmark label="ilo" />
        </h1>
        <p className="pron">
          /ˈi.lo/ <span>noun</span>
        </p>
        <ol className="senses">
          <li>
            <p>
              Esperanto for a tool, from the ending that makes a tool of a verb: <i>retumi</i>, to
              surf the web, gives <i>retumilo</i>, a web browser.
            </p>
            <p className="eg">Lingvo estas ilo. A language is a tool.</p>
          </li>
          <li>
            <p>
              <b>A web browser</b> (2026–), free and open source: tabs kept in Spaces down a
              sidebar, one bar for every command, Chromium underneath.
            </p>
            <p className="eg">I moved my Spaces to ilo.</p>
          </li>
        </ol>
        <p className="see">
          Compare Finnish <i>ilo</i>, joy. · <a href="/">ilo, the browser</a>
        </p>
      </article>
    </main>
  );
}
