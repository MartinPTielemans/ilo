import { createFileRoute } from "@tanstack/react-router";
import { pageHead } from "../seo";
import { TextPage } from "../components/text-page";
import { SOURCE } from "../downloads";

export const Route = createFileRoute("/privacy")({
  head: () =>
    pageHead({
      path: "/privacy",
      title: "Privacy · ilo",
      description:
        "What leaves your computer when you use ilo, and every service and address it talks to.",
    }),
  component: Privacy,
});

// The services, each with the hosts it reaches, as the browser's code on main names them: the
// branding files (app/branding/update-feed, app/branding/crash-reports, app/branding/daily-count),
// core/crash_reports.cc's and core/daily_count.cc's traffic annotations, core/search.cc and
// core/suggest.h, core/link_card.cc, engine/README.md › What reaches Google, core/widevine.cc,
// tools/flatpak-repo.sh's runtime repository, and uBlock Origin's own assets.json for the lists,
// and for sync core/sync.cc, core/sync_guard.h, app/src/account.rs and ilo-server's README › What
// the server can see. It says no more than the code does. Every host it names is in the browser's
// app/check/hosts.txt, whose hosts run fails on any other the browser asks (#684).

// The services below in a line each, as the page tells them: what, when and to whom. A change to a
// service changes its row too.
const GLANCE = [
  {
    id: "pages",
    what: "The pages you open",
    when: "As you browse and search, and with ⇧ on a link",
    to: "Each site, and your search engine",
  },
  {
    id: "chrome-web-store",
    what: "Extensions",
    when: "When you use the store, and about every five hours with one installed",
    to: "Google",
  },
  {
    id: "security-updates",
    what: "Security updates",
    when: "A minute after launch, then every five hours",
    to: "Google",
  },
  {
    id: "widevine",
    what: "Protected video",
    when: "On Linux, once you allow it",
    to: "Google",
  },
  {
    id: "ad-block-lists",
    what: "Ad block lists",
    when: "About once a week",
    to: "The lists' hosts",
  },
  {
    id: "updates",
    what: "ilo's updates",
    when: "Every 8 hours on a Mac and Windows; with Flatpak on Linux",
    to: "GitHub, or your Flatpak remote",
  },
  {
    id: "daily-count",
    what: "A daily count",
    when: "Once a day, only if you turn it on",
    to: "ilo's server",
  },
  {
    id: "crash-reports",
    what: "Crash reports",
    when: "Only those you send",
    to: "Sentry, in the EU",
  },
  {
    id: "sync",
    what: "Sync",
    when: "Only while you're signed in, end-to-end encrypted",
    to: "ilo's sync server, or your own",
  },
];
/** The page's sections, by id, as the index beside them names each. */
const INDEX: [string, string][] = [
  ["at-a-glance", "At a glance"],
  ["pages", "The pages you open"],
  ["services", "Every service"],
  ["network", "Names and proxies"],
  ["never", "Never sent"],
  ["builds", "Other builds"],
  ["site", "This site"],
  ["checked", "How it's checked"],
  ["questions", "Questions and changes"],
];

function Privacy() {
  return (
    <TextPage
      title="Privacy"
      index={INDEX}
      lead={
        <>
          ilo needs no account, has no ads, and sells nothing about you. If you turn it on, it sends
          one anonymous count a day. What leaves your computer is the pages you open, that count and
          the few checks below, each with where it goes.
        </>
      }
    >
      <section id="at-a-glance">
        <h2>At a glance</h2>
        <table className="glance">
          <thead>
            <tr>
              <th scope="col">What</th>
              <th scope="col">When</th>
              <th scope="col">To</th>
            </tr>
          </thead>
          <tbody>
            {GLANCE.map((row) => (
              <tr key={row.id}>
                <th scope="row">
                  <a href={`#${row.id}`}>{row.what}</a>
                </th>
                <td>{row.when}</td>
                <td>{row.to}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section id="pages">
        <h2>The pages you open</h2>
        <p>
          A page you open, or preview with Peek, gets what any browser sends it: your IP address,
          the page you asked for and the cookies it set before. ilo blocks ads and trackers on every
          site unless you turn that off.
        </p>
        <p>
          When you hold Shift (⇧) on a link, ilo asks that link's page for its title, description
          and picture and shows them in a card. It sends no cookies with the request. The picture
          can be on another site. Both sites see your IP address and the address they're asked for,
          before you click. Settings › Links › Link cards turns this off.
        </p>
        <p>
          A search goes to your search engine when you press return: DuckDuckGo (
          <code>duckduckgo.com</code>) unless you choose another in Settings › General. Nothing you
          type in the command bar leaves your computer before you search: ilo asks your search
          engine for no suggestions as you type.
        </p>
      </section>

      <section id="services">
        <h2>Every service ilo talks to</h2>
        <dl className="services">
          <dt id="chrome-web-store">The Chrome Web Store, from Google</dt>
          <dd>
            <p>
              Extensions come from the Chrome Web Store. When you visit the store, it sees which
              extensions you have. Installing one you've confirmed fetches its icon from Google's
              image host, and the extension from Google.
            </p>
            <p>
              With a store extension installed, ilo checks for its updates soon after launch, then
              about every five hours. The check sends each extension's id and version, and ilo's
              version, system and language, without cookies.
            </p>
            <p className="hosts">
              <code>chromewebstore.google.com</code> <code>clients2.google.com</code>{" "}
              <code>clients2.googleusercontent.com</code>
            </p>
          </dd>

          <dt id="security-updates">Chromium's security updates, from Google</dt>
          <dd>
            <p>
              ilo is built on ungoogled-chromium, which cuts Chromium's calls to Google. ilo brings
              one back, for safety: a check for three components that keep the browser from trusting
              what's known to be bad. They're the list of revoked certificates (CRLSet), the
              certificate authorities and logs Chromium trusts (PKI Metadata), and the certificate
              error helper. It runs a minute after launch, then every five hours.
            </p>
            <p>
              The check names ilo's engine version, your operating system and its version, and the
              components it has. It carries no cookies and nothing that ties one check to the next,
              but Google sees your IP address. There's no switch for it, as without it a revoked
              certificate would stay trusted.
            </p>
            <p className="hosts">
              <code className="path">update.googleapis.com/service/update2/json</code>, with
              downloads only from <code>dl.google.com</code>, <code>*.gvt1.com</code> and{" "}
              <code>www.google.com/dl/</code>
            </p>
          </dd>

          <dt id="widevine">Widevine, for protected video on Linux</dt>
          <dd>
            <p>
              On Linux, Netflix and other protected video need Google's Widevine module, which ilo
              doesn't ship. Once you allow protected video, ilo downloads it, and keeps it up to
              date, from the same Google hosts as the security updates. Removing it in Settings
              stops that. On a Mac, protected video plays in macOS's own web engine, and on Windows
              with Windows' own PlayReady, so ilo fetches nothing for it there.
            </p>
          </dd>

          <dt id="ad-block-lists">Ad block lists</dt>
          <dd>
            <p>
              ilo blocks ads with uBlock Origin, built in. It ships with its lists, so it blocks
              from the first page, and fetches new ones about once a week from the hosts the lists
              come from. uBlock Origin may also add a list for your language. Those hosts see your
              IP address and that the lists were fetched; the blocking itself happens on your
              computer, so nothing about the pages you visit goes with it. Settings › Ad Blocking ›
              Block ads and trackers turns it off, and a site's own controls turn it off there.
            </p>
            <p className="hosts">
              <code>ublockorigin.github.io</code> <code>ublockorigin.pages.dev</code>{" "}
              <code>cdn.jsdelivr.net</code> <code>raw.githubusercontent.com</code>{" "}
              <code>publicsuffix.org</code> <code>pgl.yoyo.org</code>{" "}
              <code>malware-filter.gitlab.io</code> <code>malware-filter.pages.dev</code>{" "}
              <code>curbengh.github.io</code>
            </p>
          </dd>

          <dt id="updates">ilo's updates</dt>
          <dd>
            <p>
              Only a copy that updates itself checks. On a Mac and on Windows, ilo reads its update
              feed every 8 hours, sending its version and no profile of your system, and downloads
              an update from ilo's releases on GitHub, which hands the file over from its own
              download host. Each update is signed, and ilo checks the signature before it installs.
              A copy from the Microsoft Store leaves its updates to the Store. There's no switch to
              stop the checks yet (
              <a href="https://github.com/MartinPTielemans/ilo/issues/1793">#1793</a>).
            </p>
            <p className="hosts">
              <code className="path">
                github.com/MartinPTielemans/ilo/releases/download/updates/appcast.xml
              </code>{" "}
              <code>release-assets.githubusercontent.com</code>
            </p>
            <p>
              On Linux, only the Flatpak updates, through Flatpak's own remote, as your other
              Flatpak apps do. Installing it fetches the runtime it runs on from Flathub, if you
              don't have it yet.
            </p>
            <p className="hosts">
              <code>dl.flathub.org</code>
            </p>
          </dd>

          <dt id="daily-count">A daily count, only if you turn it on</dt>
          <dd>
            <p>
              Once a day, about 30 seconds after ilo opens, or when you come back to it on a new
              day, ilo sends one count to ilo's server, so its makers know how many copies are in
              use. It says which version of ilo you have and whether it's a release or a nightly
              build, your operating system and its major version, and your chip. With it go four
              yes-or-no answers: whether it's the copy's first count since you turned it on, and
              whether it's its first today, this week and this month. ilo works those out from the
              date of its last count, which it keeps with the fact that you turned it on.
            </p>
            <p>
              The count has no ID, so no two counts can be tied together or to you. It carries no
              cookies and nothing about your pages, tabs, Spaces, agents, accounts or settings. A
              count that fails isn't sent again until ilo next opens that day, and a day ilo isn't
              used sends nothing. The server adds it to that day's totals and keeps only the
              numbers, not your IP address or the time. Like any server, it sees the address the
              count comes from; ilo-server doesn't keep or log it, but a proxy in front of it could.
            </p>
            <p>
              It's off until you turn it on: the first switch of the last step when ilo first opens,
              or Settings › Privacy › Send a daily usage count, which also turns it off again at
              once. A copy that sent counts before it was off by default stops with the update that
              made it so, and isn't asked again. The count goes to ilo's server whichever sync
              server you use, and a build of ilo that names no server sends none.
            </p>
            <p className="hosts">
              <code className="path">sync.ilobrowser.com/v1/daily</code>
            </p>
          </dd>

          <dt id="crash-reports">Crash reports, only the ones you send</dt>
          <dd>
            <p>
              When ilo crashes it writes a report on your computer, and the report leaves it only if
              you say so. The next time ilo opens, it asks whether to send that one. Settings ›
              Privacy › Send crash reports, off unless you turn it on, sends each one without
              asking, and turning it off deletes those not yet sent.
            </p>
            <p>
              A report is what the crashed part of ilo was doing: its threads and their stacks,
              which can hold bits of what it was working on, the code it had loaded, and ilo's
              version, its switches, your graphics chip and your extensions' ids. Switches naming
              your folders are left out. It goes to Sentry, in the EU, told to keep no IP address
              with it, and carries no cookies.
            </p>
            <p className="hosts">
              <code>o4512341293334528.ingest.de.sentry.io</code>
            </p>
          </dd>

          <dt id="sync">Sync, only if you sign in</dt>
          <dd>
            <p>
              Sync is off until you sign in to an ilo account, in Settings › Sync or when ilo first
              opens. Signing in sends your email address, then a code mailed to you or a passkey, to
              the account's server: ilo's, unless you give ilo the address of one you run yourself
              from <a href="https://github.com/MartinPTielemans/ilo-server">its source</a>. It needs
              no Google account.
            </p>
            <p>
              Signed in, ilo syncs your Spaces and your first profile's bookmarks, passwords and
              open tabs. They're encrypted on your computer with your 24 recovery words, which only
              your devices have, and the server keeps them encrypted and turns away anything that
              arrives unencrypted. Even which site and username a password is for stays hidden.
            </p>
            <p>
              What the server can see is your email address and when your account was made. It knows
              each device by a label, such as "ilo on Mac", which can include its hardware model, as
              in "ilo on Mac (MacBookPro18,3)", with its form factor, its operating system and when
              it was last active. A device being added also gives its label and operating system.
              The server also sees a few of Chromium's settings it keeps unencrypted; and, for the
              rest, which kinds of data you sync, how many items, how large, when they change and
              how bookmarks nest, but not their names, addresses or contents. When your devices
              message each other through it, it sees when and how much, not what they say.
            </p>
            <p>
              Like any server, it sees the IP address each request comes from. ilo-server, the
              software it runs, doesn't log addresses, but a proxy in front of it could. Signing out
              stops it all.
            </p>
            <p className="hosts">
              <code>sync.ilobrowser.com</code>
            </p>
          </dd>
        </dl>
      </section>

      <section id="network">
        <h2>Names, proxies and the rest</h2>
        <p>
          ilo finds the sites you visit, and the services above, through your system's DNS, as your
          other apps do. It sends those lookups to no DNS service of its own.
        </p>
        <p>
          Where your system's proxy settings say to find a proxy automatically, as Windows' do by
          default, ilo asks your local network for one (<code>wpad</code>), as your system does.
        </p>
        <p>
          When you ask an agent from ilo, ilo runs the <code>claude</code> or <code>codex</code>{" "}
          program you installed, which talks to its own company under your account. ilo has no AI
          service of its own and sends it nothing itself.
        </p>
        <p>
          On a Windows computer joined to a work or school directory, ilo can sign you in to your
          organisation's sites with Windows' own Microsoft sign-in, as your organisation set it up.
        </p>
      </section>

      <section id="never">
        <h2>What ilo never sends</h2>
        <p>
          Nothing about what you do in ilo, and nothing for advertisers: the daily count above says
          only that a copy is in use. Of Google's services, ilo brings back only those above: no
          Safe Browsing, no Google sync and no Google metrics.
        </p>
      </section>

      <section id="builds">
        <h2>Someone else's build of ilo</h2>
        <p>
          ilo is open source, so anyone can build their own. Official builds differ from such a fork
          only in their name and logo, their signing keys and a few addresses: the update feed,
          where crash reports and the daily count go, where ad block lists come from and this site.
          A fork chooses its own. What a fork sends is up to whoever makes it, so read theirs.
        </p>
      </section>

      <section id="site">
        <h2>This site</h2>
        <p>
          ilobrowser.com sets no cookies and runs no analytics or third-party scripts. Cloudflare
          serves it, and sees each request as any host does.
        </p>
      </section>

      <section id="checked">
        <h2>How this list is checked</h2>
        <p>
          A check in ilo's code starts ilo on a new profile, waits through its first security check,
          opens a page and types a search, then reads every address ilo reached. It fails if ilo
          asked any host this page doesn't name, or one of those it names only for what you turn on.
          It runs on a Mac, Windows and Linux.
        </p>
      </section>

      <section id="questions">
        <h2>Questions and changes</h2>
        <p>
          Ask on <a href={`${SOURCE}/issues`}>GitHub</a>. Every change to this page is in{" "}
          <a href={`${SOURCE}/commits/main/site/src/routes/privacy.tsx`}>the site's history</a>.
          Last changed 18 November 2026.
        </p>
      </section>
    </TextPage>
  );
}
