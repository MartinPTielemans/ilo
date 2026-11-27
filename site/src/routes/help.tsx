import { createFileRoute } from "@tanstack/react-router";
import { Fragment } from "react";
import { pageHead } from "../seo";
import { TextPage } from "../components/text-page";
import { SOURCE } from "../downloads";
import { FAQ, faqData } from "../faq";

export const Route = createFileRoute("/help")({
  head: () =>
    pageHead({
      path: "/help",
      title: "Help · ilo",
      description:
        "How to use ilo: installing it on Mac, Windows and Linux, Spaces and profiles, tabs, the command bar, split view, extensions, ad blocking and updates, and answers to common questions.",
      data: faqData(),
    }),
  component: Help,
});

/** The page's sections, by id, as the index beside them names each. */
const INDEX: [string, string][] = [
  ["install", "Installing"],
  ["spaces", "Spaces and profiles"],
  ["tabs", "Tabs"],
  ["command-bar", "The command bar"],
  ["split", "Split view"],
  ["extensions", "Extensions and ad blocking"],
  ["updates", "Updates"],
  ["problem", "Reporting a problem"],
  ["questions", "Questions"],
];

// The keys are the app's (the browser's docs/ux-spec.md › Keyboard). Off the Mac each is pressed
// with Ctrl for ⌘ and Alt for ⌃, which the page says once rather than at every key.
function Help() {
  return (
    <TextPage
      here="/help"
      title="Help"
      index={INDEX}
      lead={
        <>
          The short version of how ilo works. The keys are the Mac's: on Windows and Linux, press
          Ctrl for <kbd>⌘</kbd> and Alt for <kbd>⌃</kbd>. Settings › Shortcuts lists them all and
          lets you change them.
        </>
      }
    >
      <section id="install">
        <h2>Installing</h2>
        <p>
          On a Mac, ilo needs macOS 14 Sonoma or later on Apple silicon. Open the download and drag
          ilo into Applications.
        </p>
        <p>
          On Windows, ilo needs 64-bit Windows 10 or later. Run the installer, which installs ilo
          for you alone and asks for no administrator. The installer isn't signed yet, so Windows
          may say it protected your PC: choose More info, then Run anyway.
        </p>
        <p>
          On Linux, ilo is a Flatpak for x64. Open the download with your software centre, or run{" "}
          <code>flatpak install --user ilo-linux-x86_64.flatpak</code> where you saved it. Flatpak
          fetches the runtime ilo runs on from Flathub if you don't have it.
        </p>
        <p>
          To bring your tabs, bookmarks and history from another browser, choose Import from Other
          Browsers… in the ilo menu, or type "import" in the command bar.
        </p>
      </section>

      <section id="spaces">
        <h2>Spaces and profiles</h2>
        <p>
          A Space holds the tabs for one part of your life, such as work or a trip, each with its
          own colour. Switch Spaces with <kbd>⌃1</kbd> to <kbd>⌃9</kbd>, <kbd>⌥⌘←</kbd> and{" "}
          <kbd>⌥⌘→</kbd>, a click on a dot at the foot of the sidebar, or a two-finger swipe over
          the sidebar.
        </p>
        <p>
          A profile holds your logins, cookies, history and extensions. Several Spaces can share a
          profile, so you stay signed in across them, or a Space can have a profile of its own to
          keep a work account apart.
        </p>
      </section>

      <section id="tabs">
        <h2>Tabs</h2>
        <p>
          The sidebar keeps a Space's tabs. New tabs go under Today. Pin a tab with <kbd>⌘D</kbd> to
          keep it, or drag it into the favorites at the top, which the Spaces of a profile share
          unless you choose otherwise in Settings › General. <kbd>⌘S</kbd> hides the sidebar and
          shows it again.
        </p>
        <p>
          Today tabs tidy themselves: one you haven't looked at for 12 hours goes to the archive, as
          one you close with <kbd>⌘W</kbd> does. Clear, or <kbd>⇧⌘K</kbd>, closes Today's tabs but
          the one you're on, and <kbd>⌘Z</kbd> brings them back. <kbd>⇧⌘T</kbd> reopens the tab you
          closed last. The archive is in the Library, <kbd>⇧⌘L</kbd>, for 30 days.
        </p>
      </section>

      <section id="command-bar">
        <h2>The command bar</h2>
        <p>
          <kbd>⌘T</kbd> opens the command bar. Type an address, a search, an open tab's name or a
          command, such as "clear cookies", and press Return. <kbd>⌘L</kbd> opens it on the page's
          address to change it. <kbd>⇧⌘C</kbd> copies the page's address.
        </p>
      </section>

      <section id="split">
        <h2>Split view</h2>
        <p>
          <kbd>⌃⇧=</kbd> adds a page beside the one you're on, and you can drag a tab from the
          sidebar onto the page to split it there. A split stays together in the sidebar as one row.
        </p>
      </section>

      <section id="extensions">
        <h2>Extensions and ad blocking</h2>
        <p>
          Chrome's extensions work in ilo: install them from the Chrome Web Store, and find them in
          the Extensions menu. ilo blocks ads and trackers from the start. If a site breaks, turn
          blocking off for that site in Site Controls, the button in the address field, or
          everywhere in Settings › Ad Blocking.
        </p>
      </section>

      <section id="updates">
        <h2>Updates</h2>
        <p>
          ilo updates itself. To look now, choose Check for Updates… in the ilo menu, or type
          "update" in the command bar. On Linux, Flatpak updates it with your other Flatpak apps.
          What changed is in the <a href="/changelog">changelog</a>.
        </p>
      </section>

      <section id="problem">
        <h2>Reporting a problem</h2>
        <p>
          Help › Report a Problem… opens a new issue on GitHub with ilo's version and your system
          filled in. Nothing is sent until you submit it. You can also{" "}
          <a href={`${SOURCE}/issues`}>look through the issues</a> others have reported. What ilo
          sends and where is on <a href="/privacy">Privacy</a>.
        </p>
      </section>

      <section id="questions">
        <h2>Questions</h2>
        <dl className="services">
          {FAQ.map(({ question, answer }) => (
            <Fragment key={question}>
              <dt>{question}</dt>
              <dd>
                <p>{answer}</p>
              </dd>
            </Fragment>
          ))}
        </dl>
      </section>
    </TextPage>
  );
}
