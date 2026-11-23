import type { Platform } from "./downloads";

/** What a change is to whoever reads it: a security fix, something new, something that now works another way, or a fix. */
export type Kind = "security" | "new" | "changed" | "fixed";

/** The kinds in the order a release lists them, each with its heading. */
export const KINDS: { kind: Kind; label: string }[] = [
  { kind: "security", label: "Security" },
  { kind: "new", label: "New" },
  { kind: "changed", label: "Changed" },
  { kind: "fixed", label: "Fixed" },
];

/** One change, in a sentence or a few. */
export type Change = {
  kind: Kind;
  /** The part of ilo it's in, which a release groups its changes by: "Tabs", "Command bar". Reuse one a release
   * before it has, so the same part keeps one name. */
  area: string;
  text: string;
  /** The platforms it's for, when it isn't for every one. The text doesn't say so again. */
  only?: Platform["id"][];
  /** The clip that shows it, which puts it at the release's top instead of in its lists. */
  clip?: string;
};

export type Release = {
  version: string;
  /** The day it's tagged, as "29 October 2026"; none while it's still to come, which leaves it off
   * the page and the feed. */
  date?: string;
  summary: string;
  /** A fix worth updating for now. The page marks the release "Security update". */
  urgent?: true;
  changes: Change[];
};

/** Where a clip's files are: NAME.mp4, NAME.webp (its first frame) and NAME-still.webp. */
export const CLIPS = "/media/changelog";

/** A release with more listed changes than this, unless it's the newest, shows its Security and New ones and folds
 * the rest behind a button. */
export const FOLD = 12;

/**
 * Whether a release is out: it has its day and, where the build was given the versions published
 * on GitHub (`listed`, ILO_PUBLISHED, which the deploy sets from the releases), it's one of them.
 * An entry merged before its release is published so stays off the live site until the release's
 * own deploy (#3798). The checks and `vp dev` have no list, and show every dated release.
 */
export function isOut(release: Release, listed?: string) {
  return Boolean(release.date) && (!listed || listed.split(/\s+/).includes(release.version));
}

/** The releases that are out, newest first, which is what the site shows. A page passes
 * `import.meta.env.ILO_PUBLISHED` and the build's feed `process.env`'s (vite.config.ts). */
export const published = (releases: Release[] = RELEASES, listed?: string) =>
  releases.filter((release) => isOut(release, listed));

// Newest first. A release's changes go here before it's tagged, with the day it's tagged, and show
// once it's published (isOut): the app's What's New and the notes each update shows link to
// /changelog#<version>. A clip is the files tools/clip.sh makes from a recording and its name
// here, on the change it shows.
export const RELEASES: Release[] = [
  {
    version: "0.3.0",
    date: "22 November 2026",
    summary:
      "Agents always ask you first on the sites you list, their request cards lead with what they want, and an agent in a terminal can use ilo through the ilo command. On the Mac, a page that needs protected video moves to Safari's engine.",
    changes: [
      {
        kind: "new",
        area: "Agents",
        text: "Settings › Agents › Always ask first holds sites (and their subdomains), such as your bank, where an agent asks you before opening a page, using one it reached, or sending a click, key or form, whatever you answered before. Agents can't see or change the list.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "Settings › Agents › Command line adds the ilo command, so an agent in a terminal can use ilo. On Windows it's added to your PATH, and in the Flatpak, Add gives you a line to run once.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "Request cards lead with what the agent says it wants under ilo's own line, with See the whole request for details, and agent cards and Library › Agents entries take the task's name.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "Sign-in pages and file upload fields hand the tab to you, and later downloads ask you first. When you take a tab, the agent sees only its number.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "Agents can run several steps in one go, take a screenshot of their tab even when your window isn't showing a page, and get the content of an app inside a frame.",
      },
      {
        kind: "security",
        area: "Agents",
        text: "Security fixes in what agents are shown and in when they must ask you first. Details will follow once most copies of ilo have them.",
      },
      {
        kind: "new",
        area: "Pages",
        only: ["mac"],
        clip: "protected-video",
        text: "When a page needs protected video as you click or press a key in it, its tab moves to Safari's engine, with a note and Move Back, and ilo asks you first while you're typing or using the camera, microphone or screen.",
      },
      {
        kind: "new",
        area: "Pages",
        only: ["mac"],
        clip: "apple-music",
        text: "Apple Music's web player opens in Safari's engine from its first page, and Clear Browsing Data… also clears what Safari's engine keeps.",
      },
      {
        kind: "new",
        area: "Library",
        clip: "new-board",
        text: "An empty Boards list offers New Board with its shortcut, and an empty Boosts list says where to make a Boost.",
      },
      {
        kind: "new",
        area: "Library",
        clip: "board-pictures",
        text: "A Board's picture in Library › Boards shows its cards.",
      },
      {
        kind: "new",
        area: "Previews",
        text: "Clicking a row in a pinned tab's or Favorite's hover card opens it in that tab.",
      },
      {
        kind: "new",
        area: "Keyboard",
        clip: "shortcuts",
        text: "Settings › Shortcuts lists every shortcut a command has, and you can replace one at a time.",
      },
      {
        kind: "new",
        area: "Spaces",
        clip: "mouse-spaces",
        text: "With the sidebar revealed, a mouse's back and forward buttons go to the previous and next Space.",
      },
      {
        kind: "fixed",
        area: "Split view",
        text: "A page's dialog stays over its own pane, ilo's shortcuts work next to an alert from the same site, and Previous Pane, Next Pane and Close this Split Pane work on Danish, German, French and Japanese layouts, from a page too.",
      },
      {
        kind: "fixed",
        area: "Tabs",
        text: "When you open a tab from the command bar while on a Favorite, pinned tab or Today tab, closing it returns you to that tab.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "Each session of an agent, such as one in each of two terminals, gets its own card, and a card stops saying Working once the agent is done. Open on an earlier card no longer moves the tabs a newer task is working in.",
      },
      {
        kind: "fixed",
        area: "Spaces",
        text: "Clicks and keys right after Create on a new Space land on the new Space.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        text: "Pressing ⌘L twice no longer sends a key to the page, and when you go back to the previous window before a new one from ⌘N appears, the command bar waits for you to switch to it.",
      },
      {
        kind: "fixed",
        area: "Downloads",
        text: "Dragging a download's row quickly still drags its file.",
      },
      {
        kind: "fixed",
        area: "Pages",
        only: ["mac"],
        text: "When a dialog closes, the page gets clicks and the keyboard back at once, a key the page didn't use as an alert closes is no longer typed into the next field, and what a page in Safari's engine saves right before you quit is kept.",
      },
      {
        kind: "fixed",
        area: "Pages",
        only: ["windows", "linux"],
        text: "Page alerts and the shortcuts around them are also fixed on Windows and Linux.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "A page stays inside the window with a larger Text size.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows", "linux"],
        text: "The sidebar and the top of the window are solid when background blur isn't available, so other windows don't show through under Wayland.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "A double-click on a busy machine counts as one.",
      },
    ],
  },
  {
    version: "0.2.5",
    date: "21 November 2026",
    summary: "A privacy fix on Linux, and a newer ad blocker.",
    changes: [
      {
        kind: "security",
        area: "Privacy",
        only: ["linux"],
        text: "ilo no longer writes URLs you open to standard error, which GNOME keeps in the system journal. Earlier versions did, in Incognito too; nothing left your computer. Existing entries stay until the journal drops them or you clear it, and ilo's Clear Browsing Data doesn't remove them.",
      },
      {
        kind: "security",
        area: "Ad blocking",
        text: "uBlock Origin is now 1.76.6, which has its latest security fixes.",
      },
    ],
  },
  {
    version: "0.2.4",
    date: "20 November 2026",
    summary: "A security fix for agent permissions. Update now.",
    urgent: true,
    changes: [
      {
        kind: "security",
        area: "Agents",
        text: "A security fix in the rules for when an agent has to ask you before acting. It's worth updating now; details will follow once most copies of ilo have this fix.",
      },
    ],
  },
  {
    version: "0.2.3",
    date: "20 November 2026",
    summary:
      "The keyboard and a screen reader now reach a Board's cards, and a Library row's menu opens by key. Agents are told at once when a page's own dialog stops them, and can close its tab.",
    changes: [
      {
        kind: "new",
        area: "Boards",
        text: "Tab reaches a Board's cards, in reading order from top to bottom, and goes from card to card, panning to one out of view.",
      },
      {
        kind: "new",
        area: "Boards",
        text: "A screen reader names each card: a note by its first line, a picture, a drawing, a link by its text.",
      },
      {
        kind: "new",
        area: "Boards",
        text: "The arrows move what's selected, with ⇧ for bigger steps, and each move is undone with ⌘Z. Delete puts the keyboard on the next card, Esc from a note's text puts it on the note, and ↩ on a link card opens it. A screen reader hears when a card is moved or deleted.",
      },
      {
        kind: "new",
        area: "Library",
        text: "In the Library, with a row picked by ↓ and ↑, ⇧F10 or the menu key opens that row's menu under it, with the keyboard on its first item, so a Board's Duplicate and Delete are in reach of the keyboard. Esc closes it and leaves the row picked.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "When a page in an agent's tab shows its own alert or other dialog, the agent is told at once, rather than left waiting, and can close the tab and the dialog with it. A tab in its Space it didn't open closes only once you say yes.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "An action whose page stops answering ends and says so.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "The activity log says when an agent's page paused at a dialog.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "A press in any of a watched agent's tabs takes that tab over, not only in the one it acted in last.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "A watched agent page that crashed shows the crash card in its place, and a press on it takes the tab over.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "A click on another picture in Pages while the watched page shrinks back watches that page.",
      },
      {
        kind: "fixed",
        area: "Tabs",
        text: "⌘W pressed just as the tab you're on closes now closes the tab shown next, rather than nothing, so closing tabs fast never skips a press.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        text: "While a page's dialog asks, ilo's shortcuts work from another page of the same site, such as one it opened in another window.",
      },
      {
        kind: "fixed",
        area: "Window",
        text: "A window showing no page, such as one on an empty Space, comes back where it was after a quit.",
      },
      {
        kind: "fixed",
        area: "First launch",
        text: "Each first-launch step puts the keyboard on its main button or first control, so a screen reader says each step as you go on to it, and a stray Space on the opt-ins can't turn a switch on.",
      },
      {
        kind: "fixed",
        area: "First launch",
        text: "The search engine list shows the keyboard's ring.",
      },
      {
        kind: "fixed",
        area: "Ad blocking",
        text: "Ad blocking's malware and phishing lists no longer update from a copy that stopped being updated, and a list older than the one ilo holds is passed over for the next.",
      },
      {
        kind: "fixed",
        area: "Boards",
        text: "A Board's picture in Library › Boards takes the window's light or dark look, not the one it was saved in.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["mac"],
        text: "Go to Pane 1 to 4 (⌃⇧1 to ⌃⇧4) works on keyboard layouts where ⇧ and a digit type another character, such as Danish and German, from a page and from ilo's own controls.",
      },
      {
        kind: "fixed",
        area: "Screen readers",
        only: ["windows", "linux"],
        text: "A screen reader names the command bar as it opens.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["linux"],
        text: "Go to Pane 1 to 4 (Alt+Shift+1 to 4) works from ilo's own controls, such as the sidebar after F6, on keyboard layouts where Shift and a digit type another character.",
      },
      {
        kind: "fixed",
        area: "Screen readers",
        only: ["linux"],
        text: 'Orca names the Space, rather than an unnamed frame, as the command bar closes, and no longer says "Matches" as ilo launches.',
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["linux"],
        text: "Under Wayland, after a shortcut shows another tab while Ctrl+L's command bar is open, the bar keeps your input method, so Japanese and other composed text can be typed into it, and a Board made from the sidebar's + keeps the keyboard on its name, so what you type names it.",
      },
    ],
  },
  {
    version: "0.2.2",
    date: "19 November 2026",
    summary: "A fix to when agents ask you first.",
    changes: [
      {
        kind: "security",
        area: "Agents",
        text: "A request that may change something always asks first.",
      },
    ],
  },
  {
    version: "0.2.1",
    date: "19 November 2026",
    summary:
      "Live Folders bring the GitHub pull requests waiting on you into your sidebar, a pinned GitHub tab previews them, Boards give you a canvas to collect things on, and ⇧ on a link shows what's behind it. The keyboard lands where you expect around dialogs, agents and screen readers on every platform.",
    changes: [
      {
        kind: "new",
        area: "Live Folders",
        text: "Live Folders › GitHub Pull Requests, in a Space's menu, adds a folder at the end of its pinned tabs that lists the pull requests waiting for your review, then your own, from the GitHub account signed in on the Space's profile.",
      },
      {
        kind: "new",
        area: "Live Folders",
        text: "A new one is bold until you open it and a draft is faded. Pressing a row opens it as a tab in its place, and once the pull request is merged or closed its tab moves above the lists as a plain tab. Closed, the folder shows how many reviews wait.",
      },
      {
        kind: "new",
        area: "Live Folders",
        text: "It refreshes only when you do something that may have changed it: coming back to the window, showing its Space, opening it, visiting GitHub in its Space, or Refresh in its menu. Nothing checks in the background. Signed out, it offers Sign in to GitHub. A refresh that fails keeps the rows and marks the folder.",
      },
      {
        kind: "new",
        area: "Live Folders",
        text: "⌘Z brings back a Live Folder you deleted or converted.",
      },
      {
        kind: "new",
        area: "Previews",
        text: "Resting on a pinned tab or favorite on GitHub shows your open pull requests in its hover card, the reviews asked of you first.",
      },
      {
        kind: "new",
        area: "Previews",
        text: "A click opens one in that tab, ⌘-click in a new tab behind it and ⌥-click beside the page. Settings › General › What's recent in hover cards turns it off, and so does the site's switch in Site Controls. Nothing of it is kept once ilo quits.",
      },
      {
        kind: "new",
        area: "Boards",
        clip: "boards",
        text: "New Board, from ⌃⇧E, the command bar, the + menu and File, opens a canvas as a pinned tab, where you pan and zoom through notes, link cards, pictures and ink, with undo, copy, paste and drop.",
      },
      {
        kind: "new",
        area: "Boards",
        text: "Library › Boards lists your Boards with their pictures, to open, rename, Duplicate or Delete, and ⌘Z brings a deleted one back.",
      },
      {
        kind: "new",
        area: "Boards",
        text: "A capture's menu has Add to Board › and New Board, and the capture lands in the middle of what's in view when its Board opens.",
      },
      {
        kind: "new",
        area: "Boards",
        text: "A Board whose file can't be read is never written over, and offers Show in Finder (Show in Folder off the Mac). Private windows don't offer Boards, and agents can't open one.",
      },
      {
        kind: "new",
        area: "Link card",
        clip: "link-card",
        text: "With a link's address showing in the window's corner, press ⇧ and it grows into a card of what the page says about itself, with its picture, title, description, and the site it ends up on when that's another one.",
      },
      {
        kind: "new",
        area: "Link card",
        text: "The page is asked with none of your cookies or sign-ins, and never at an address on your own network. Esc closes it, and Settings › Links › Link cards turns it off.",
      },
      {
        kind: "new",
        area: "Top bar",
        clip: "top-bar",
        text: "A top bar, off unless you turn it on, puts the URL field across the window's top. Settings › Tabs & Sidebar › Top bar, ⇧⌘D, the command bar, the sidebar's right-click menu and the Mac's View menu show and hide it.",
      },
      {
        kind: "new",
        area: "First launch",
        text: "The first launch asks which search engine to use, with DuckDuckGo picked, and Settings marks it as the default. Google can now be picked in both, but never comes picked.",
      },
      {
        kind: "new",
        area: "Keyboard",
        text: "Tab past a page's last control, or ⇧Tab before its first, moves to its tab's row in the sidebar, and Tab from there goes back into the page. Tab and ⇧Tab in the window walk the sidebar's rows, and an empty Space's New Tab row.",
      },
      {
        kind: "new",
        area: "Library",
        text: "Opening the Library by key gives its search the keyboard, ↓ and ↑ move through its rows, ↩ opens one, Tab goes over its controls, and closing it gives the keyboard back to what had it.",
      },
      {
        kind: "new",
        area: "Screen readers",
        text: "A screen reader reads each first-launch step by its title, names its controls, and follows the keyboard through the sidebar and the window.",
      },
      {
        kind: "new",
        area: "Boosts",
        text: "A Boost's CSS can use the colours of its tab's Space, as --ilo-space-background, --ilo-space-text, --ilo-space-accent and --ilo-space-accent-text. A Boost that doesn't name them tells its site nothing.",
      },
      {
        kind: "changed",
        area: "Sync",
        text: "Settings › Sync turns red only once a sync trouble lasts, says which trouble and since when, and opens sync's details.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "Agents see what a field holds when it came from their own typing or the page itself, and pick a select's option by the name they're shown.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "An agent's close asks the page as yours does, so a page with unsaved work keeps its tab, and the agent's card says the page kept it rather than that the close failed.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "A press on an agent's page or one of its dialogs takes that tab over, in a watched split view too.",
      },
      {
        kind: "new",
        area: "Engine",
        only: ["mac"],
        text: "Music in a tab shown in Safari's engine gets the sidebar's player, which plays, pauses and skips, and the tab shows its page's own icon.",
      },
      {
        kind: "new",
        area: "Top bar",
        only: ["windows"],
        text: "With the top bar on, the window's buttons sit at its right end, where Windows has them, and Snap layouts still show over maximize.",
      },
      {
        kind: "new",
        area: "Sidebar",
        only: ["windows", "linux"],
        text: "A menu control in the sidebar's footer opens ilo's menu: Settings…, Import from other browsers…, Privacy policy, About ilo and Quit ilo.",
      },
      {
        kind: "new",
        area: "Screen readers",
        only: ["windows", "linux"],
        text: "NVDA and Orca hear the command bar's selected suggestion, the toast and the find bar's count.",
      },
      {
        kind: "new",
        area: "Screen readers",
        only: ["windows"],
        text: "On Windows, a screen reader hears the control that has the keyboard, and the page you go back to says the control you land on.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "An agent's tab never takes your keyboard: a watched agent tab showing a warning, a tab an agent's page opens, an agent's download, and an agent's tab in a Space you aren't looking at all leave the keyboard where it was.",
      },
      {
        kind: "security",
        area: "Agents",
        text: "Agents can't read pages that aren't http or https, or a Board, and each refusal is in the activity log.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "An option with a line break reads as one line in the question you're asked and the task's steps.",
      },
      {
        kind: "fixed",
        area: "Split view",
        text: "A click on a split pane whose tab has the warning before an http page up focuses that pane and shows the warning.",
      },
      {
        kind: "fixed",
        area: "Split view",
        text: "A press on a page under its own hidden dialog focuses its pane.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        text: "⌘L just as the tab you're on closes edits the tab shown next.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        text: "A shortcut that shows another tab while ⌘L's bar is open makes the bar's pick open in a new tab, rather than in the tab you left.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        text: "The page the command bar's ↩ goes to takes the keyboard.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        text: "Shortcuts with ⇧ and a symbol, such as ⌃⇧2 and ⌘⇧], work while the sidebar has the keyboard.",
      },
      {
        kind: "fixed",
        area: "Tabs",
        text: "A click on a row of the ⌃Tab switcher picks that tab, and a click on a sidebar row while it shows keeps that row's tab.",
      },
      {
        kind: "fixed",
        area: "Library",
        text: "⌘Z in the Library's search field undoes Clear Archive and what the toast offers again.",
      },
      {
        kind: "fixed",
        area: "Ad blocking",
        text: "A pop-up the ad block closes no longer shows in Today for a moment, and doesn't take the keyboard while it's held. One a page opens blank and then sends to a blocked page closes too.",
      },
      {
        kind: "fixed",
        area: "Ad blocking",
        text: "Ad blocking turned on in Settings blocks at every launch, even after uBlock Origin was turned off on the extensions page or cut short by a quit.",
      },
      {
        kind: "fixed",
        area: "Ad blocking",
        text: "After a filter list updates, a launch blocks with the last copy rather than waiting for the new one.",
      },
      {
        kind: "fixed",
        area: "Boosts",
        text: "Arrange's drops land as shown: letting go where a drop doesn't fit lands nothing, a release while the page moves lands under the pointer, lifting a thing from a spaced row moves nothing after it, a drop on the page's bare background with no key held changes nothing, a trade between two nested things puts each where the other was, and later changes move what earlier ones placed, keeping the gaps.",
      },
      {
        kind: "fixed",
        area: "Tabs",
        text: "When memory runs short, ilo lets go of the page of the tab you haven't seen longest, one at a time, rather than one it had already let go of.",
      },
      {
        kind: "fixed",
        area: "Passwords",
        text: "A sign-in page opened in a new tab shows the saved login's key from its first page.",
      },
      {
        kind: "fixed",
        area: "Window",
        text: "A window that comes to the front takes back a page's question that a closed window left, so ilo's shortcuts work again.",
      },
      {
        kind: "fixed",
        area: "Stability",
        text: "A browser message with nowhere to show no longer closes ilo.",
      },
      {
        kind: "fixed",
        area: "Offline",
        text: "The game on the offline page is no longer cut short when the connection comes back mid-run.",
      },
      {
        kind: "fixed",
        area: "Peek",
        text: "Peek opens on the page's own colour rather than white, and Esc as its card lands sends its page away first.",
      },
      {
        kind: "fixed",
        area: "Mini Player",
        text: "The Mini Player's controls show on one flat black under the pointer.",
      },
      {
        kind: "fixed",
        area: "Capture",
        text: "Capture mode's menu lights the title of the row under the pointer, and on a dark window the link's address shows its path dimmer than its site.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["mac"],
        text: "A tab's dialog is part of its window: the command bar stays open as one shows under it, and a click in another pane of a split lands while a dialog has the keyboard.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["mac"],
        text: "A page no longer stops taking keys after a way into full screen is cut short.",
      },
      {
        kind: "fixed",
        area: "Mini Player",
        only: ["mac"],
        text: "A Mini Player thrown while ilo is busy is thrown.",
      },
      {
        kind: "fixed",
        area: "Engine",
        only: ["mac"],
        text: "A tab in Safari's engine comes back after a quit at the page it was on, shows its own page's icon rather than the page before's, takes Esc twice to leave a page's full screen, and lets Tab and ⇧Tab out of the page at its ends.",
      },
      {
        kind: "fixed",
        area: "Engine",
        only: ["mac"],
        text: "A page that comes in after one in Safari's engine draws.",
      },
      {
        kind: "fixed",
        area: "Boosts",
        only: ["mac"],
        text: "Boosts aren't offered over a page in Safari's engine, which they can't change.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["windows"],
        text: "The keyboard stays where you put it around a tab's dialogs and pop-ups: a click on the page beside a dialog or on ilo's UI, a press that brings the window back from another app, F6 over a dialog, Go to Pane 2 after Go to Pane 1 from the warning before an http page, a tab's dialog while its tab is behind, and the page that opened a pop-up all land where they should.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        only: ["windows"],
        text: "The command bar stays open when the warning before an http page first shows under it.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["windows"],
        text: "Japanese and Korean typed right after Ctrl+L in a page that keeps the shortcut compose in the page, and nothing typed for the command bar composes in the page.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["windows"],
        text: "A click or key that comes while ilo is busy waits its turn.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "The command bar, tab switcher, find bar and offer draw their own corner and shadow, with no sliver round them.",
      },
      {
        kind: "fixed",
        area: "Peek",
        only: ["windows"],
        text: "Peek holds its card until its page has left.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "The sidebar draws again after the display adapter restarts with no overlay shown, and Settings' taskbar button carries the Space's dot.",
      },
      {
        kind: "fixed",
        area: "Mini Player",
        only: ["windows"],
        text: "A Mini Player dragged as an outline, or with window contents off, goes where you throw or leave it.",
      },
      {
        kind: "fixed",
        area: "Agents",
        only: ["windows"],
        text: "An agent typing in a tab you aren't watching no longer slows down.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["linux"],
        text: "Under Wayland, Settings and ilo's other windows compose through your input method, and Japanese typed right after Ctrl+L composes in the command bar.",
      },
      {
        kind: "fixed",
        area: "Extensions",
        only: ["linux"],
        text: "An extension's popup opened from the sidebar takes the keyboard and closes with Esc, and the keyboard comes back after.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["linux"],
        text: "One Ctrl+N after a Ctrl+F the page kept opens one window.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "An overlay closes as you go to another of ilo's windows, Peek's page leaves only once the frame closing it is on screen, and a window maximized as it opens no longer crashes ilo.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["linux"],
        text: "Under X11, a tab's dialog takes the keyboard back after a click on the page beside it, so Ctrl+T works there.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "An overlay takes the keyboard and its place on top back from a window manager that focuses the window under it, closes as you go to another of ilo's windows, and shows whole in the tile a tiling manager gives it.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "Under KWin a prompt draws as soon as it has its shape, and under sway's Xwayland a prompt takes the keys back once its window returns from another workspace.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "Under X11 and Wayland alike, every window draws again once the graphics card recovers, a tab's dialog shown again as the tab in front closes takes the keyboard, Peek's page keeps the keys when you come back from another app, a folder's double-click opens its name, the window resizes from its left and right edges, and the pointer at the very edge brings out the hidden sidebar.",
      },
      {
        kind: "fixed",
        area: "Flatpak",
        only: ["linux"],
        text: "In the Flatpak, media keys and the desktop's media controls reach ilo, a page can use the camera through the desktop's camera portal, Copy Link Address no longer closes ilo, and a crash of the browser itself leaves a crash report, sent only with Send Crash Reports on.",
      },
    ],
  },
  {
    version: "0.2.0",
    date: "16 November 2026",
    summary:
      "Your browsing history now lives in the Library beside your closed tabs, a Boost can rearrange the pieces of a site's pages, and the sidebar can move to the right. Ad blocking gets its own Settings section, and agents read pages more fully while keeping your typing out of their screenshots.",
    changes: [
      {
        kind: "security",
        area: "Engine",
        text: "The engine is on Chromium 157.0.8103.78, with its security fixes.",
      },
      {
        kind: "changed",
        area: "Privacy",
        text: "The daily usage count is now off until you turn it on, in the first launch's Up to you step or Settings › Privacy › Send a daily usage count. A copy that was sending it stops at this update, without asking.",
      },
      {
        kind: "new",
        area: "Library",
        text: "Library › History lists the pages you've visited beside your closed tabs, with a search and a Filter menu, and loads more as you scroll. Removing a row deletes that day's visits to the page, and ⌘Z brings them back.",
      },
      {
        kind: "new",
        area: "Command bar",
        text: "The command bar says whether each result is an open tab, a closed tab or a page you've visited.",
      },
      {
        kind: "new",
        area: "Boosts",
        text: "Arrange, on a Boost's card beside Customize…, lets you carry a page's own elements where you want them, and puts them there each time the site's pages load.",
      },
      {
        kind: "new",
        area: "Boosts",
        text: "A small bar over the thing selected hides it, puts it back, or sets whether it stays on screen as you scroll, ↑ and ↓ pick the box around it or the one inside, ⌘Z takes back the last step, and Esc is Done. The card counts what moved and lists each change, with ↺ to put it back.",
      },
      {
        kind: "new",
        area: "Boosts",
        text: "A Boost can run your own script: Edit Script…, in the footer of the editor Customize… opens and in Library › Boosts, opens the site's script in your editor, and it runs only once you turn it on.",
      },
      {
        kind: "new",
        area: "Boosts",
        text: "The editor's Advanced opens sliders for the page's contrast, brightness and saturation, and its header shows the site's icon.",
      },
      {
        kind: "new",
        area: "Sidebar",
        text: "The sidebar can sit on the window's right, from Settings › Tabs & Sidebar › Sidebar position. The Library opens on its side, and the launch shows the sidebar there.",
      },
      {
        kind: "new",
        area: "Ad blocking",
        text: "Settings › Ad Blocking is a section of its own: the filter lists, each with a switch, My filters, and the sites ads are shown on.",
      },
      {
        kind: "fixed",
        area: "Ad blocking",
        text: "uBlock Origin no longer shows in the extension lists, and the malware and phishing lists no longer update from mirrors that went stale.",
      },
      {
        kind: "changed",
        area: "Site Controls",
        text: "Site Controls list only what differs for the site: its own switches, then the permissions that aren't the profile's default or that the page asked for, its logins and its devices.",
      },
      {
        kind: "new",
        area: "Ad blocking",
        text: "A shield in the action row says whether ads and trackers are blocked, and how many were on the page.",
      },
      {
        kind: "new",
        area: "Sidebar",
        text: "The sidebar's media card plays its tab's video live while the stack is open.",
      },
      {
        kind: "changed",
        area: "Mini Player",
        only: ["mac", "windows"],
        text: "On the Mac and Windows, a Mini Player thrown toward a corner settles there, and one let go near an edge settles along it.",
      },
      {
        kind: "changed",
        area: "Split view",
        text: "A split view's row gives most of its width to the pane you're in, with its whole title, and folds the others to their favicons.",
      },
      { kind: "new", area: "Split view", text: "⌃⇧1 to ⌃⇧4 go to the split's pane at that place." },
      {
        kind: "new",
        area: "Tabs",
        text: "A held ⌃Tab shows the Space's tabs, most recently shown first, and letting go of ⌃ switches to the one selected; Esc goes back to where you started. A quick ⌃Tab still switches once.",
      },
      {
        kind: "new",
        area: "Tabs",
        text: "Sort Today Tabs by Site, in the Today divider's right-click menu, the Tabs menu and the command bar, puts each site's Today tabs together. ⌘Z undoes it.",
      },
      {
        kind: "new",
        area: "Tabs",
        text: "A pinned tab or favorite that's away from its pinned URL shows a small return mark on its icon. One whose pinned URL redirects counts as home once it's back there.",
      },
      {
        kind: "new",
        area: "Spaces",
        text: "With more than one profile, the Space's name row and Site Controls name the profile a page is in.",
      },
      {
        kind: "changed",
        area: "Settings",
        text: "Settings chooses Sidebar position, Appearance and Share favorites from small drawings, picked with a click or the arrow keys. Appearance follows View › Appearance.",
      },
      {
        kind: "new",
        area: "Settings",
        text: "A control you Tab to scrolls into sight, and Page Up, Page Down, Space, Home and End scroll the page.",
      },
      {
        kind: "new",
        area: "First launch",
        text: "The first launch's Up to you switches take ⇥, ⇧⇥ and Space, with ↩ still Continue.",
      },
      {
        kind: "new",
        area: "Stability",
        text: "If ilo crashes twice in a row just after it opens, the next launch starts with extensions and Boosts held, and a card at the sidebar's foot says so, with Turn Back On and Manage Extensions….",
      },
      {
        kind: "changed",
        area: "Little ilo",
        text: "A sign-in that another app opens in Little ilo closes its window once it's done, and leaves no visit or closed tab behind. Settings › Features › Little ilo turns this off.",
      },
      {
        kind: "changed",
        area: "Privacy",
        text: "Copied links now also leave out a site's own share IDs, such as YouTube's and Instagram's, and keep a video's time. Settings › Privacy's Copy links without tracking turns the cleaning off.",
      },
      {
        kind: "changed",
        area: "Command bar",
        text: "An address typed with http:// loads over http, as you wrote it; a bare address still tries https first.",
      },
      {
        kind: "security",
        area: "Privacy",
        text: "A page can ask to share your screen only right after you click or press a key.",
      },
      {
        kind: "new",
        area: "Updates",
        text: "With automatic update checks off, the update banner offers Update Automatically, and Settings' switch follows it.",
      },
      {
        kind: "new",
        area: "Settings",
        text: "New Note has a switch in Settings › Features: off, its commands and keys go, and the notes you have stay.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "Each request in Settings › Agents' activity opens to show where it went and what it sent, with secrets hidden.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "Settings › Agents lists Claude's models by the names Claude gives them, and says which one Default is.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "Agents see more of a page: a table's rows, a list's items, the options of a select or a picker, choice tiles whose buttons the page hid, and read-only fields, and they're told what changed in an element.",
      },
      {
        kind: "security",
        area: "Agents",
        text: "An agent's screenshot covers whatever you or a password manager put in a field.",
      },
      {
        kind: "security",
        area: "Agents",
        text: "Agents can't act on an extension's controls in a page, and an extension doesn't hear their input.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "An agent types as keys a page reads like yours, and a line break never sends a message. Its scroll to an element leaves room around it, so a sticky header doesn't hide it.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "Handing a tab to you, or moving it into one of your Spaces, stops the agent's action at once.",
      },
      {
        kind: "fixed",
        area: "Stability",
        text: "A click, key or typed text that comes while ilo is busy lands, in order, instead of being lost or closing ilo.",
      },
      {
        kind: "fixed",
        area: "Stability",
        text: "Turning agents off, sharing a Today tab, ⌘T after Set Up and Copy Link Address in a page's right-click menu no longer close ilo.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        text: "On the Mac, Windows and Linux under Wayland, ilo's shortcuts, such as ⌘T and ⌘W, keep working while a tab shows one of the browser's dialogs.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        text: "⇧⌘W right after ⌘N closes the new window, not the one behind it, and an ⌥-click on a link still clicks it when your hand moves a little.",
      },
      {
        kind: "fixed",
        area: "Engine",
        only: ["mac"],
        text: "Pages in Safari's engine no longer look up each address with Apple's site-safety service, a press on the Mini Player leaves the keyboard with the window that had it, the window you go to keeps the keyboard as another window's command bar closes, and the sync recovery words you copy leave the clipboard on time.",
      },
      {
        kind: "new",
        area: "Window",
        only: ["windows"],
        text: "Portrait Mode offers your desktop's picture as a background, Settings has a switch for the taskbar button's Space-coloured dot, the download menu says Move to Recycle Bin, and the card for a video ilo can't play offers to open it in your default browser.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "A tab dragged out opens in front of the window it left, and signing out or shutting down with ilo behind another app no longer crashes it.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "A menu opened from the sidebar while it's slid out shows in front of it, the page and ilo's overlays get the keyboard back after Win+D and after Site Controls close, ilo's windows draw again after the display adapter restarts, and holding Esc in a page's full screen counts as one held key, as on the Mac.",
      },
      {
        kind: "changed",
        area: "Peek",
        only: ["windows", "linux"],
        text: "Peek grows from the link and shrinks back into it, in the page's own colour, with no white flash.",
      },
      {
        kind: "changed",
        area: "Stability",
        only: ["windows", "linux"],
        text: "Report a Problem and Copy Build Info say how ilo is installed, with Windows' edition and processor, or Linux's session and desktop.",
      },
      {
        kind: "new",
        area: "Keyboard",
        only: ["linux"],
        text: "Shift+Alt-click does what ⌥-click does, for window managers that keep Alt-click for themselves.",
      },
      {
        kind: "new",
        area: "Flatpak",
        only: ["linux"],
        text: "In the Flatpak, a crash now leaves a crash report, sent only with Send Crash Reports on.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["linux"],
        text: "Under Wayland, ilo's fields paste what another app or a page copied, Korean keeps composing in the command bar, a shortcut that closes a tab or window from the page runs once, and the recovery words leave the clipboard on time even with ilo in the background.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "Under X11, the command bar and other overlays hide with their window and come back with it, a click in the sidebar or the find bar keeps the keyboard there, a drag onto the page widens the sidebar, and under KWin, Peek, a tab's dialog and the toast no longer draw over another app's window.",
      },
    ],
  },
  {
    version: "0.1.16",
    date: "12 November 2026",
    summary: "A fix for importing from another browser at the first launch.",
    changes: [
      {
        kind: "fixed",
        area: "Import",
        text: "Importing your open tabs from another browser at the first launch no longer closes ilo.",
      },
    ],
  },
  {
    version: "0.1.15",
    date: "11 November 2026",
    summary:
      "Sync takes only what arrives encrypted with your key, and the Mac and Windows can get nightly builds.",
    changes: [
      {
        kind: "new",
        area: "Updates",
        only: ["mac", "windows"],
        text: "Settings › About has Get nightly builds, for a copy that updates itself to each night's build from main.",
      },
      {
        kind: "security",
        area: "Sync",
        text: "Sync takes your bookmarks, passwords and other encrypted data only when they arrive encrypted with your key, and drops anything else.",
      },
      {
        kind: "security",
        area: "Sync",
        text: "Sync no longer takes two kinds of data that Chromium never encrypts: another person's invitations to share a password, and Chromium's own priority preferences.",
      },
      {
        kind: "fixed",
        area: "Agents",
        only: ["windows"],
        text: "A tab that an agent's page opens no longer takes your keyboard from where you were typing.",
      },
    ],
  },
  {
    version: "0.1.14",
    date: "10 November 2026",
    summary:
      "A Space's Today tabs follow your account to your other computers. Boosts can be made and edited without code, from a Boost's card, the sidebar's + menu and a tab's menu. The Mini Player has a new look, and a tab kept from an archived tab's preview goes back to its own Space.",
    changes: [
      {
        kind: "new",
        area: "Sync",
        text: "A Space's Today tabs follow your account to your other computers, almost as you open and close them.",
      },
      {
        kind: "changed",
        area: "Sync",
        text: "A computer whose ilo is older than the one that last changed your Spaces stops syncing them, rather than undo what it doesn't know, and Settings says to update it.",
      },
      {
        kind: "new",
        area: "Boosts",
        text: "Customize… on a Boost's card opens an editor beside the sidebar, for a Boost made without code.",
      },
      {
        kind: "new",
        area: "Boosts",
        text: "The sidebar's + menu and a Today tab's menu start the page's Boost, or edit it if it has one.",
      },
      { kind: "new", area: "Boosts", text: "Window › View Boosts… opens Library › Boosts." },
      {
        kind: "changed",
        area: "Mini Player",
        text: "The Mini Player has rounded corners and a rim. Its controls fade in under one even shade, and its buttons sit further in.",
      },
      {
        kind: "fixed",
        area: "Tabs",
        text: "A tab kept from an archived tab's preview, by ⌘↩ or Open as Tab, goes back to the Space it closed in, as one restored from the Library does.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "An agent's click on a link that shows only once it's focused, such as a page's skip link, focuses it and presses it, instead of pressing above the page.",
      },
      {
        kind: "fixed",
        area: "Window",
        text: "A page's picture in a window it left no longer comes up empty right after it loads.",
      },
      {
        kind: "fixed",
        area: "Previews",
        text: "A hidden tab's hover card picture is taken at the screen's own scale.",
      },
      {
        kind: "fixed",
        area: "Little ilo",
        only: ["mac"],
        text: "Show All brings a Little ilo window left on another desktop over the main window when it's full screen.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "Under Wayland, quitting with a toast or another overlay on screen no longer crashes.",
      },
      {
        kind: "fixed",
        area: "Little ilo",
        only: ["linux"],
        text: "Under Wayland, a link from another app for a Little ilo window that's open already brings that window forward.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "A picker shown again while it fades out after Esc stays open.",
      },
    ],
  },
  {
    version: "0.1.13",
    date: "10 November 2026",
    summary:
      "Boosts give a site your own CSS, or zap away what you click on its pages. Capture can put a page on a background in Portrait Mode and draw on what's chosen before it's shared, the media player skips back and sets a tab's volume, and the import brings Today tabs and link rules from Arc. On the Mac a language can be translated always.",
    changes: [
      {
        kind: "new",
        area: "Boosts",
        text: "Give a site your own CSS, or Zap what you click on its pages to hide it.",
      },
      {
        kind: "new",
        area: "Boosts",
        text: "A Boost shows as a page loads and at once while it's open. Each is a plain file in your profile, made from Site Controls, the command bar or the Library.",
      },
      {
        kind: "new",
        area: "Capture",
        text: "Capture in Portrait Mode puts the page on a background, ready to share.",
      },
      {
        kind: "new",
        area: "Capture",
        text: "Capture's Edit draws a pen, an arrow or a box on what's chosen before it's copied, saved or sent.",
      },
      {
        kind: "new",
        area: "Media player",
        text: "The sidebar's media player skips back and sets its tab's volume.",
      },
      { kind: "new", area: "Tabs", text: "⌘} and ⌘{ show the next and previous tab." },
      {
        kind: "new",
        area: "Menus",
        text: "File › New Profile and Spaces › Edit Theme… are in the menus.",
      },
      {
        kind: "new",
        area: "Tabs",
        text: "A tab let go over the page, away from the split view targets, opens in a window of its own.",
      },
      {
        kind: "changed",
        area: "Window",
        text: "A page you left in another window: its picture there follows the page as it loads and navigates.",
      },
      {
        kind: "new",
        area: "Import",
        text: "The import brings Arc's Today tabs to their Spaces, under a switch of their own, and Arc's link rules to Little ilo, the Space in front and Everything else.",
      },
      {
        kind: "new",
        area: "Sync",
        text: "View › Appearance and Show Space Header follow your account to your other computers, as ilo's other settings do.",
      },
      {
        kind: "new",
        area: "Privacy",
        text: "Settings › Privacy says what ilo never sees, beside the policy that names every service it talks to.",
      },
      {
        kind: "new",
        area: "Settings",
        text: "Settings › Tabs & Sidebar has Battery Saver's switch.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "Library › Agents opens a past agent session's page: its steps and the sites it used, each with Revoke.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "An agent's question left unanswered for 5 minutes is refused, and a handoff not handed back in 15 minutes answers the agent, so an agent never waits forever.",
      },
      {
        kind: "new",
        area: "Translate",
        only: ["mac"],
        text: "Always Translate a language from the translate card: every page in it translates as it opens, in every window, until Stop Always Translating. Settings › Features lists the languages, each with Remove.",
      },
      {
        kind: "new",
        area: "Translate",
        only: ["mac"],
        text: "The page menu has Translate and Use Original too.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "⌘↩ takes over or hands back an agent's page even when the page has the keyboard.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "A task card that turns to wait for you brings a hidden sidebar out, as an agent's question does.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "A tab a page opens in an agent's Space starts muted, as the agent's own tabs do.",
      },
      {
        kind: "fixed",
        area: "Sidebar",
        text: "The sidebar shows the hover of the row under the pointer as it comes out.",
      },
      {
        kind: "fixed",
        area: "Menus",
        text: "A menu's submenu closes once the pointer rests outside it.",
      },
      {
        kind: "fixed",
        area: "Settings",
        text: "A Settings page that fits its card doesn't scroll.",
      },
      {
        kind: "fixed",
        area: "Pages",
        text: "The browser's Customize page, typed in a private window, is refused instead of crashing.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["mac"],
        text: "⌘N's window comes on screen drawn, instead of blank for a moment.",
      },
      {
        kind: "changed",
        area: "Performance",
        only: ["mac"],
        text: "ilo's helper processes share the framework's constant data, 11.7 MiB less memory in each.",
      },
      {
        kind: "fixed",
        area: "Notifications",
        only: ["mac"],
        text: "macOS asks whether ilo may send notifications, under ilo's name.",
      },
      {
        kind: "changed",
        area: "Window",
        only: ["mac"],
        text: "Pages and split views are rounded on the window's continuous curve, so the card's corners sit concentric with it.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["mac"],
        text: "An overlay's shadow grows and shrinks with it, and the theme picker's shadow no longer leaves a white band at the window's top.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["mac"],
        text: "A toast that appears under the pointer leaves on time.",
      },
      {
        kind: "fixed",
        area: "Performance",
        only: ["windows"],
        text: "An idle window no longer repaints at the display's rate.",
      },
      {
        kind: "changed",
        area: "Performance",
        only: ["windows"],
        text: "The browser process uses 42 MiB less memory per window.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "Closing a window whose page is full screen no longer crashes.",
      },
      {
        kind: "fixed",
        area: "Ad blocking",
        only: ["windows"],
        text: "uBlock Origin no longer runs out of memory.",
      },
      {
        kind: "fixed",
        area: "Little ilo",
        only: ["windows"],
        text: "Little ilo windows hand the foreground back to the other app, and let ilo activate again afterwards.",
      },
      {
        kind: "fixed",
        area: "Split view",
        only: ["windows"],
        text: "A split view's handles take the pointer, instead of the window's top edge resizing over them.",
      },
      {
        kind: "fixed",
        area: "Spaces",
        only: ["linux"],
        text: "Switching Spaces under X11 no longer flashes the window's card over the page.",
      },
      {
        kind: "fixed",
        area: "Mini Player",
        only: ["linux"],
        text: "Under Wayland, the browser window takes the keyboard back from a video's Mini Player (#2060).",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "The icon and theme pickers blur what's behind them under KWin.",
      },
      {
        kind: "fixed",
        area: "Capture",
        only: ["linux"],
        text: "Capture's pointer is the camera, and a popover's arrow casts its shadow.",
      },
    ],
  },
  {
    version: "0.1.12",
    date: "8 November 2026",
    summary:
      "On the Mac, a page in another language can be translated on the device from the URL field. The command bar finds the tabs open on your other computers, an Arc split view comes in from the import as a split view, and on Linux and Windows a Bluetooth device's PIN is asked in the page's card. Battery Saver turns on by itself on Linux now.",
    changes: [
      {
        kind: "new",
        area: "Command bar",
        text: "Type in the command bar to find the tabs open on your other computers that sync. Each row names its computer.",
      },
      {
        kind: "new",
        area: "Import",
        text: "An Arc split view comes in from the import as a split view, with its name, icon and orientation.",
      },
      {
        kind: "new",
        area: "Translate",
        only: ["mac"],
        text: "A page in a language you don't read shows a mark in the URL field. Click it to translate the page on your Mac, with Use Original to go back.",
      },
      {
        kind: "new",
        area: "Translate",
        only: ["mac"],
        text: "Translate Page and Use Original are in the command bar too. It needs macOS 26, and macOS's model for the language, which Settings… opens System Settings to add.",
      },
      {
        kind: "new",
        area: "Devices",
        only: ["windows", "linux"],
        text: "When a Bluetooth device asks for a PIN, or to confirm a passkey, the page's card asks you. An agent's tab is never asked: its pairing is cancelled.",
      },
      {
        kind: "changed",
        area: "Battery Saver",
        only: ["windows"],
        text: "Battery Saver reads the battery when Windows says it changed, not once a minute, so an idle ilo stays asleep.",
      },
      {
        kind: "new",
        area: "Battery Saver",
        only: ["linux"],
        text: "Battery Saver turns on by itself: ilo reads the battery from UPower.",
      },
      {
        kind: "changed",
        area: "Site search",
        text: "A site search lists only the search for what you typed, under Search….",
      },
      {
        kind: "changed",
        area: "Sidebar",
        text: "The tab you're on slides in under its closed folders, rather than appearing there at once.",
      },
      {
        kind: "fixed",
        area: "Pages",
        only: ["mac"],
        text: "Share on the Copied Current URL toast, from ⇧⌘C or the URL field's copy button, opens the share sheet again.",
      },
    ],
  },
  {
    version: "0.1.11",
    date: "8 November 2026",
    summary:
      "Sync can now carry your ilo settings and your extensions, and a Space can be exported into a folder in another Space. Tabs restored from the archive go back to their Space, and old downloads leave the Library's list on their own. On Windows, sign-ins outlive a quit, and ilo follows a display's new scale.",
    changes: [
      {
        kind: "new",
        area: "Sync",
        text: "Sync can carry your ilo settings and the extensions you added from the Chrome Web Store. Each device chooses with the switches in Settings › Sync › Also sync.",
      },
      {
        kind: "new",
        area: "Spaces",
        text: "A Space's menu has Export into Folder. It moves the Space's pinned tabs into a new folder in the Space you pick, and its Today tabs into that Space's Today. ⌘Z brings the Space back.",
      },
      {
        kind: "changed",
        area: "Library",
        text: "A tab restored from the Library's Archived Tabs opens in the Space it closed in.",
      },
      {
        kind: "changed",
        area: "Downloads",
        text: "Downloads leave the Library's list after the profile's archive time, as tabs do. Their files stay where they are.",
      },
      {
        kind: "new",
        area: "Command bar",
        text: "Type an extension's name in the command bar to run it on the tab you're on.",
      },
      {
        kind: "changed",
        area: "Split view",
        text: "While you drag a tab or a link over the page, the page draws a little smaller, and one split target slides in at the edge nearest the pointer.",
      },
      {
        kind: "changed",
        area: "Sidebar",
        text: "The tab you're on stays listed when its folder is closed, under the outermost closed folder, whose icon shows three dots. It goes when you close the folder over it.",
      },
      {
        kind: "fixed",
        area: "Site search",
        text: "Leaving a site search in the command bar puts back what was there before.",
      },
      {
        kind: "fixed",
        area: "Split view",
        text: "⌘Z after Separate All Tabs on a favorite split makes it the favorite split again, and keeps the tab you were on shown.",
      },
      {
        kind: "fixed",
        area: "Little ilo",
        text: "A Little ilo window's page turns Developer Mode on and off.",
      },
      {
        kind: "fixed",
        area: "Sidebar",
        only: ["mac"],
        text: "A name field that a menu choice opens stays open as the menu closes.",
      },
      {
        kind: "fixed",
        area: "Profiles",
        only: ["windows"],
        text: "Sites that keep a sign-in in a session cookie keep you signed in after you quit ilo.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "ilo and its pages follow a display's new scale, and a move to a display with another scale.",
      },
      {
        kind: "fixed",
        area: "Little ilo",
        only: ["windows"],
        text: "Closing a Little ilo window over another app leaves that app's window in front.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows", "linux"],
        text: "ilo's windows draw at the full rate while ilo is the active app, so closing Peek is smooth.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        only: ["linux"],
        text: "On Wayland, a page loses its focus while the command bar is open and gets it back when the bar closes, so a page that pauses when it loses focus does.",
      },
    ],
  },
  {
    version: "0.1.10",
    date: "7 November 2026",
    summary:
      "ilo now sends one anonymous usage count a day, which Settings › Privacy turns off. On the Mac, an update downloads only what changed. The Mini Player keeps its place across launches, and this release fixes crashes and lost keys on Windows and Linux.",
    changes: [
      {
        kind: "changed",
        area: "Privacy",
        text: "Once a day ilo sends one anonymous count: its version, your operating system and chip, and whether it's this copy's first run, or its first count this week or month. It has no ID and nothing about the pages you visit or what you do in ilo. Turn it off in Settings › Privacy › Send a Daily Usage Count.",
      },
      {
        kind: "changed",
        area: "Updates",
        only: ["mac"],
        text: "An update from any of the three releases before it downloads only what changed, not the whole app.",
      },
      {
        kind: "fixed",
        area: "Mini Player",
        text: "The Mini Player comes back where you left it after a restart, and a Mini Player you leave with a menu open still goes back to its tab.",
      },
      {
        kind: "fixed",
        area: "Sidebar",
        text: "A favicon that's black reads on a dark sidebar, and one that's white on a light sidebar.",
      },
      {
        kind: "fixed",
        area: "Sync",
        text: "A new device's first sign-in to sync waits for the connection instead of failing.",
      },
      {
        kind: "fixed",
        area: "Peek",
        only: ["mac"],
        text: "Closing Peek is smooth again while ilo is the active app.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "Quitting while a page is full screen no longer crashes.",
      },
      {
        kind: "fixed",
        area: "Tabs",
        only: ["windows"],
        text: "Signing out of Windows keeps the tab you opened just before.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["linux"],
        text: "A shortcut pressed before a new tab's page is ready, such as a quick second ⇧⌘T, is no longer lost.",
      },
      {
        kind: "fixed",
        area: "Mini Player",
        only: ["linux"],
        text: "On Wayland, the window keeps the keyboard when a tab's Mini Player opens as you switch away.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "On Wayland, ilo no longer crashes when a pop-up closes under its dialog.",
      },
    ],
  },
  {
    version: "0.1.9",
    date: "6 November 2026",
    summary:
      "Agents can do much more in your browser, and every step that reaches outside the page asks you first: an agent's download waits for your OK, and so does any change it sends to its tab's site. Site search, a Blank Window, Battery Saver, a Library for private windows, and Little ilo from Windows' taskbar and Linux's launcher are new.",
    changes: [
      {
        kind: "new",
        area: "Site search",
        text: "Type a site's name in the command bar and press Tab to search it there. 19 sites are built in, each in its own colour.",
      },
      {
        kind: "new",
        area: "Window",
        text: "A Blank Window (⌃⌘N on the Mac) holds tabs that aren't kept or shown anywhere else.",
      },
      {
        kind: "new",
        area: "Library",
        text: "A private window has a Library of its own, with its own downloads.",
      },
      {
        kind: "changed",
        area: "Previews",
        text: "An archived tab opens as a preview first: ⌘W closes it and leaves it archived, ⌘↩ brings it back. ⌘-click still restores it at once.",
      },
      {
        kind: "new",
        area: "Previews",
        text: "Resting on a closed folder previews its tabs, newest first, with their ages.",
      },
      {
        kind: "new",
        area: "Spaces",
        text: "Reorder Spaces by dragging a dot in the Space switcher, or a Library card by its handle.",
      },
      {
        kind: "new",
        area: "Split view",
        text: "A split view can have its own name and icon. Drag one onto the favorites to keep it as a single favorite.",
      },
      {
        kind: "new",
        area: "Capture",
        text: "⇧⌘-drag on a page captures the rectangle you drag, at once.",
      },
      {
        kind: "new",
        area: "Library",
        text: "⌘Z puts back a page or search you removed from history.",
      },
      {
        kind: "new",
        area: "Pages",
        text: "Right-click the reload button to reload, hard reload, or clear the site's data and reload.",
      },
      {
        kind: "new",
        area: "Passwords",
        text: "Edit › AutoFill opens the autofill dropdown at the focused field.",
      },
      {
        kind: "changed",
        area: "Mini Player",
        text: "The Mini Player opens at the top right, stays where you leave it, and takes you back to its tab.",
      },
      {
        kind: "new",
        area: "Battery Saver",
        text: "Battery Saver turns on below 20 %, with Turn Off. A leaf shows while it's on, and a page it froze says so.",
      },
      {
        kind: "security",
        area: "Privacy",
        text: "Calls connect directly without telling a page your local address.",
      },
      {
        kind: "new",
        area: "Privacy",
        text: "Settings › Privacy › Relay calls sends every call through a relay.",
      },
      {
        kind: "new",
        area: "Settings",
        text: "Settings has page zoom, Ask where to save, notification prompts, and Archive after: Never.",
      },
      {
        kind: "new",
        area: "Little ilo",
        text: "Little ilo can be turned off in Settings › Features.",
      },
      {
        kind: "new",
        area: "Sync",
        text: "Settings › Sync says when it last synced and what stops it, and can delete the account.",
      },
      {
        kind: "new",
        area: "Little ilo",
        only: ["windows", "linux"],
        text: "Little ilo opens from Windows' taskbar (a jump-list task and a tray icon) and from Linux's launcher (New Little ilo Window).",
      },
      {
        kind: "new",
        area: "Window",
        only: ["windows", "linux"],
        text: "Linux's launcher and Windows' jump list also open a new window or a private one.",
      },
      {
        kind: "new",
        area: "Sidebar",
        only: ["windows"],
        text: "Alt pressed alone shows the hidden sidebar.",
      },
      {
        kind: "new",
        area: "Policies",
        only: ["windows", "linux"],
        text: "Administrators can set policies: on Linux in /etc/ilo/policies (the Flatpak reads its extension point), on Windows in SOFTWARE\\Policies\\ilo.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "An agent's download asks you first, and nothing reaches your disk until you allow it.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "Agents can send requests to their tab's own site, and every change asks you first.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "Agents choose options in selects and dropdowns by their text, scroll inside panels and frames, and hear when nothing moved.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "Agents wait for the element and for the page to settle before they act, and an agent acting on a page that's still loading is told so.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "Agents make and manage split views in their own Space, and pin, unpin and remove their own tabs. Your pins stay yours.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "Tabs an agent's page opens stay in the agent's Space, and the agent hears of them.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "Library › Agents keeps your agents' finished tasks for 30 days, and deletes them with the history of their time.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "An agent's tabs are no longer synced to your other devices.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "An agent's close never shows a dialog, and tab ids never repeat across launches.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "An agent's screenshot of a tab out of sight answers instead of hanging.",
      },
      {
        kind: "fixed",
        area: "Profiles",
        only: ["mac", "linux"],
        text: "Sites keep you signed in after a restart, an update or an unloaded tab.",
      },
      {
        kind: "fixed",
        area: "Ad blocking",
        text: "A launch's first page waits for the ad blocker, so it never shows ads.",
      },
      {
        kind: "fixed",
        area: "Notifications",
        text: "Clicking a site's notification brings up the tab that sent it.",
      },
      {
        kind: "new",
        area: "Devices",
        text: "A page asking for a device shows ilo's chooser in its question card.",
      },
      { kind: "fixed", area: "Tabs", text: "A tab in a call is never unloaded." },
      { kind: "fixed", area: "Tabs", text: "A tab a DevTools client opens lands in your window." },
      {
        kind: "fixed",
        area: "Little ilo",
        text: "In Little ilo, a page's sign-in pop-up opens as its own window over it, and its size no longer becomes the size of the next Little ilo window.",
      },
      {
        kind: "fixed",
        area: "Profiles",
        text: "Deleting a profile with no Space removes its favorites too, and says how many first.",
      },
      {
        kind: "fixed",
        area: "Spaces",
        text: "Deleting a Space after the last window has closed no longer crashes.",
      },
      {
        kind: "fixed",
        area: "Privacy",
        text: "Delete in Clear browsing data, with history ticked, no longer crashes.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        text: "The command bar's picks forget deleted history, and stay in each profile's own folder.",
      },
      {
        kind: "fixed",
        area: "Tabs",
        text: "A new tab's address no longer lingers after Chromium stops showing it.",
      },
      { kind: "fixed", area: "Sync", text: "A Space's change reaches a device added soon after." },
      {
        kind: "fixed",
        area: "Pages",
        text: "A page that fails to load reads in Light appearance.",
      },
      {
        kind: "changed",
        area: "Settings",
        text: "Settings' grey text is easier to read, and its rows have more room.",
      },
      {
        kind: "changed",
        area: "Previews",
        text: "A tab's hover-card picture is taken after a tab switch instead of during it.",
      },
      {
        kind: "fixed",
        area: "Performance",
        text: "An idle browser no longer wakes every minute or two for measurements nothing uses.",
      },
      {
        kind: "fixed",
        area: "Sidebar",
        only: ["mac"],
        text: "The window's sidebar shows at once at launch, even when the Dock or menu bar changed size since you quit.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "Windows maximized when you quit come back maximized.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "A page fills its window again after moving to a display of another scale.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["linux"],
        text: "A shortcut pressed right after answering one of ilo's dialogs is no longer lost, and a key typed as a dialog closes reaches the page.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "On Wayland, a link from another app brings ilo's window forward.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "On Wayland, leaving full screen lays the page out once, at the right size, and a window gets its own size back after a page's full screen.",
      },
    ],
  },
  {
    version: "0.1.8",
    date: "5 November 2026",
    summary:
      "A click in an agent's page takes it over, and an agent's tabs open muted. Keys typed fast after a dialog closes, or after Ctrl+F on Windows, land where you meant them.",
    changes: [
      {
        kind: "new",
        area: "Agents",
        text: 'A click in the page an agent works in takes it over, and the click goes on to the page. The strip under it then reads "You\'re driving", with Hand Back. ⌘↩ still takes over and hands back.',
      },
      {
        kind: "changed",
        area: "Agents",
        text: "An agent's tabs open muted. A crossed speaker shows on its tile and its strip while it plays, and a click there unmutes it. Open, which moves a finished task's page to your Space, unmutes it.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["mac", "windows"],
        text: "A key typed while a dialog is closing reaches the browser window instead of being lost with the dialog.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["windows"],
        text: "The keys typed right after Ctrl+F reach the find bar, whether the page or ilo's own controls had the keyboard.",
      },
    ],
  },
  {
    version: "0.1.7",
    date: "5 November 2026",
    summary:
      "Little ilo grows into a full window, with its own command bar, its pinned extensions and full screen, and sends its page to any Space. The agent surfaces say what needs you and how far the agent has got.",
    changes: [
      {
        kind: "new",
        area: "Little ilo",
        text: "Little ilo opens its command bar inside its window, from ⌘L and ⌘T.",
      },
      {
        kind: "new",
        area: "Little ilo",
        text: "Open in <Space> and its Spaces menu send its page to any Space.",
      },
      {
        kind: "changed",
        area: "Little ilo",
        only: ["mac"],
        text: "On the Mac it grows in from under the pointer.",
      },
      {
        kind: "new",
        area: "Little ilo",
        text: "Little ilo's bar has your pinned extensions at its title's end, with their popups and menus, and a page's video goes full screen in the Little ilo window. Esc leaves full screen and keeps the window.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: 'The agent surfaces say what needs you. An agent\'s card has one primary button, Allow Once, Show Tab or Hand Back, and says how far the agent has got: "Step 5 · Clicking …", "Stopped after 6 steps", or "Needs you" and why. The conversation\'s header counts steps and pages.',
      },
      {
        kind: "changed",
        area: "Agents",
        text: 'A ⌘T task\'s Always Allow and Never answers belong to its CLI, Claude or Codex. Settings › Agents lists them under "What Claude may do from ⌘T", each with Revoke. Answers an earlier version kept for a single task are cleared at launch.',
      },
      {
        kind: "changed",
        area: "Import",
        text: "The import reads at a glance, in Settings and on the first launch: one line for each kind of data with its count, fewer words at each step, and one row with the counts when it's done.",
      },
      {
        kind: "new",
        area: "Screen readers",
        text: "Screen readers (VoiceOver, Narrator and Orca) name the controls in Settings' Sync, Links, About and import views, by what each one does.",
      },
      {
        kind: "fixed",
        area: "Little ilo",
        text: "A desktop app's sign-in that opens in Little ilo hands back to the app when it's done.",
      },
      {
        kind: "fixed",
        area: "Window",
        text: 'The open-app dialog\'s "Always allow" line wraps inside the dialog instead of running past its edge.',
      },
    ],
  },
  {
    version: "0.1.6",
    date: "5 November 2026",
    summary:
      "Settings you can use from the keyboard alone, a media player that stacks in the sidebar, a switch for automatic update checks, and, on the Mac, scripts that can read and open ilo's windows, Spaces and tabs once you let them.",
    changes: [
      {
        kind: "new",
        area: "Settings",
        text: "Settings works from the keyboard alone. ⇥ and ⇧⇥ move through it in reading order, ↑ and ↓ move along its column, and Space or ↩ acts.",
      },
      {
        kind: "new",
        area: "Settings",
        text: "Typing a letter starts a search, and ⌘F goes to the search field. ⌘W, Esc and ⌘Z still do what they did.",
      },
      {
        kind: "new",
        area: "Updates",
        only: ["mac", "windows"],
        text: 'Settings › About has "Check for updates automatically", on the Mac and on Windows. Turned off, ilo asks for no updates until you choose Check for Updates.',
      },
      {
        kind: "changed",
        area: "Media player",
        text: "The sidebar's media player is a stack. The newest player shows, with up to two older ones peeking behind it. Point at it and they all open, each with its title, a close button, and Picture in Picture where the page allows it.",
      },
      {
        kind: "changed",
        area: "Spaces",
        text: "Edit Theme Color… opens its picker at the window's top corner.",
      },
      {
        kind: "changed",
        area: "Spaces",
        text: "With one Space that has no name or icon of yours, its name row hides. Show Space Header, in the Space's menu, brings it back.",
      },
      {
        kind: "new",
        area: "Agents",
        only: ["mac"],
        text: "Scripts can read ilo's windows, Spaces and tabs, show a Space or a tab, and open windows and tabs, once you turn on \"Let scripts control the browser\" in Settings › Agents. It's off at first.",
      },
      {
        kind: "new",
        area: "Agents",
        only: ["mac"],
        text: "Scripts never see private windows or agents' Spaces, and nothing they can ask reads a page. Every script's request goes in the agents' activity log, and Settings shows them under \"What scripts did\".",
      },
      {
        kind: "new",
        area: "Screen readers",
        only: ["mac"],
        text: "VoiceOver names the controls in Settings' sections, including Shortcuts' keys, and the tab rows' buttons, the delete dialog, the find bar and its count, Site Controls, the icon picker and New Space.",
      },
      {
        kind: "fixed",
        area: "Little ilo",
        only: ["mac", "windows"],
        text: "Closing a Little ilo window that's over another app's window leaves that app in front.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        only: ["linux"],
        text: "On Wayland, holding Esc to close the command bar no longer sends Esc on to the page.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        only: ["linux"],
        text: "On Wayland, a click on the page that closes the command bar gives the page the keyboard.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        only: ["linux"],
        text: "A click outside a menu over the command bar closes both.",
      },
      {
        kind: "fixed",
        area: "Pages",
        only: ["linux"],
        text: "On Wayland, a page takes clicks as soon as what covered it goes, rather than after its next frame.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "On Wayland, going in or out of full screen lays the page out again, even when the window's size stays the same.",
      },
    ],
  },
  {
    version: "0.1.5",
    date: "5 November 2026",
    summary:
      "A security update to Chromium 156.0.8078.79, with Settings in a window of its own you can search, an import that shows what each browser holds, tabs you can drag to another window, and a quiet banner for updates.",
    changes: [
      {
        kind: "security",
        area: "Engine",
        text: "The engine is on Chromium 156.0.8078.79, a security point release.",
      },
      {
        kind: "security",
        area: "Privacy",
        only: ["mac"],
        text: "ilo no longer answers Chromium's AppleScript commands. Before, a script macOS let through, such as one from a terminal you once allowed, could read every tab's address and open, reload or close tabs, outside ilo's rules for agents and its activity log.",
      },
      {
        kind: "changed",
        area: "Settings",
        text: "Settings is a window of its own, its twelve sections in a column beside a card. Search every section from the top, or find a setting from the command bar.",
      },
      {
        kind: "new",
        area: "Settings",
        text: "Settings has new rows for Appearance, the Dock icon, the edge reveal and agent tasks.",
      },
      {
        kind: "new",
        area: "Tabs",
        text: "Drag a tab's row out of its window to move it to another window, or to a window of its own.",
      },
      {
        kind: "changed",
        area: "Import",
        text: "The import reads a browser as you pick it, shows its icon, says how much of each thing will come and offers its favorites as tiles. Spaces arrive as cards in their own colours.",
      },
      {
        kind: "changed",
        area: "Import",
        text: "A stalled import says which profile it's on, a stopped one offers Import Again, and on first launch each step says what it's for.",
      },
      {
        kind: "changed",
        area: "Agents",
        text: "Agents from other apps get the same task card as ⌘T's tasks, and their Spaces stay out of every list.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "A task with two pages or more opens on Pages, a grid of them, and an outside agent's session ends on a receipt.",
      },
      {
        kind: "new",
        area: "Agents",
        text: "A task that waits for you badges the Dock or the taskbar.",
      },
      {
        kind: "changed",
        area: "Updates",
        text: "A ready update waits in a small banner at the sidebar's foot. Point at it to see it, click to restart into it, and your tabs come back. On the Mac, an update ilo won't install by itself shows there too, and a scheduled check no longer puts an alert over your work.",
      },
      {
        kind: "new",
        area: "Downloads",
        text: "A new download flies into the Library button, where the recent downloads show and stay while a download's menu is open.",
      },
      {
        kind: "changed",
        area: "Command bar",
        text: "The command bar puts the commands you pick and the tabs you switch to first, learned per profile on this computer only and forgotten when you clear your browsing data.",
      },
      {
        kind: "new",
        area: "Command bar",
        text: "The command bar has Mute All Tabs, Enter Full Screen and New Profile. ⌘N's window opens with it, and opens in full screen from full screen.",
      },
      { kind: "new", area: "Window", text: "Window › Stay On Top keeps a window over the others." },
      {
        kind: "changed",
        area: "Keyboard",
        text: "Esc in full screen goes to the page first; press it twice, or hold it, to leave.",
      },
      { kind: "changed", area: "Window", text: "A window you're not in dims its header's glyphs." },
      { kind: "new", area: "Pages", text: "⇧⌘C's toast has Share, which shares the page." },
      {
        kind: "new",
        area: "Pages",
        text: "Rest the pointer on a link and its whole address shows.",
      },
      {
        kind: "changed",
        area: "Link rules",
        text: "A Google Meet link from another app opens in the Space you're in.",
      },
      {
        kind: "fixed",
        area: "Pages",
        text: "Until a page paints, it shows black or white to match the appearance, so a dark site no longer flashes white.",
      },
      {
        kind: "changed",
        area: "Sidebar",
        text: "Only the tabs scroll in the sidebar, under the Space's name.",
      },
      {
        kind: "fixed",
        area: "Sidebar",
        text: "With the sidebar hidden, a pointer flung to the window's edge no longer brings it out. It comes out once the pointer moves along the edge.",
      },
      {
        kind: "new",
        area: "Spaces",
        text: "The mouse's back and forward buttons go to the Library and a new Space, as the swipe does.",
      },
      {
        kind: "changed",
        area: "Tabs",
        text: "Close and Keep Pinned shows the tab you used before, and a pinned tab that has left its site names that site.",
      },
      {
        kind: "changed",
        area: "Library",
        text: "A Space card's list starts higher, and the ••• at its foot opens the Space's menu.",
      },
      {
        kind: "fixed",
        area: "Spaces",
        text: "A click on a tab's row while its Space slides in shows that tab, and a tab dropped on the next Space mid-swipe moves there.",
      },
      {
        kind: "fixed",
        area: "Sidebar",
        text: "A name field keeps the keyboard while a toast or a tooltip is up.",
      },
      {
        kind: "fixed",
        area: "Window",
        text: "Opening ilo with a link shows the link's page, and a two-colour Space's swatch reads as its main colour.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        text: "A shortcut pressed right after you switch windows acts on the window you switched to.",
      },
      {
        kind: "changed",
        area: "Link rules",
        text: '"is equal to" compares the host and path, and "contains" ignores the scheme and www.',
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["mac"],
        text: "A display going away under a full-screen window leaves ilo the active app.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["mac"],
        text: "The Dock menu's New Window and New Incognito Window open ilo's windows.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["windows", "linux"],
        text: "Key chips spell their shortcuts as the menus do.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "The caption no longer shows under the sidebar, ilo can quit with a file dialog open, and the sidebar's list no longer shows its edge.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["windows"],
        text: "Ctrl and the other modifier keys no longer seem stuck after a menu or the command bar takes the keyboard, and windows over a page keep their size and place as they move.",
      },
      {
        kind: "fixed",
        area: "Capture",
        only: ["windows"],
        text: "Resizing the window ends a capture, as on the Mac and Linux.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "On Wayland, a save or open panel no longer closes ilo.",
      },
    ],
  },
  {
    version: "0.1.4",
    date: "3 November 2026",
    summary:
      "Agents on Windows: tasks start without flashing windows and find Claude Code and Codex however you installed them.",
    changes: [
      {
        kind: "fixed",
        area: "Agents",
        only: ["windows"],
        text: "Agents no longer flash console windows.",
      },
      {
        kind: "fixed",
        area: "Agents",
        only: ["windows"],
        text: "Tasks find Claude Code and Codex however each was installed, and Claude tasks start through npm's claude.cmd.",
      },
      { kind: "fixed", area: "Agents", text: "Quitting while a task runs no longer crashes." },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["linux"],
        text: "A ⌃Tab walk that starts in the page keeps going while ⌃ is held, on Wayland.",
      },
    ],
  },
  {
    version: "0.1.3",
    date: "3 November 2026",
    summary:
      "Hand a task to Claude or Codex from ⌘T, a reworked sidebar, text fields that move by words, and a crash fix for pages that ask for a device.",
    changes: [
      {
        kind: "new",
        area: "Agents",
        text: "⇥ in ⌘T hands Claude or Codex a task. Its card sits at the foot of the sidebar while it works.",
      },
      {
        kind: "changed",
        area: "Sidebar",
        clip: "sidebar",
        text: "The Space's name row lights like a tab row. Pressing it folds the pinned tabs, and its menu is reordered.",
      },
      {
        kind: "changed",
        area: "Sidebar",
        text: "Favorites are larger tiles, at most four to a row.",
      },
      {
        kind: "changed",
        area: "Spaces",
        text: "With more Spaces than fit, they share the switcher, and the Library's Space cards scroll sideways.",
      },
      {
        kind: "changed",
        area: "Sidebar",
        text: "The sidebar's list ends with room to spare and a foot line, and glides to the row you go to.",
      },
      {
        kind: "new",
        area: "Split view",
        clip: "split-view",
        text: "Drag a tab to either side, ⌥ on a pane's ✕, add a favorite to a split, and a split's row has its own menu.",
      },
      {
        kind: "new",
        area: "Split view",
        text: "⌘W closes a pinned split whole, and ⌘Z brings a closed split back.",
      },
      {
        kind: "new",
        area: "Text fields",
        text: "Text fields move, select and delete by words, and a double- or triple-click selects a word or the line.",
      },
      {
        kind: "new",
        area: "Window",
        text: "Tooltips show after the pointer rests, and name a button's keyboard shortcut, in the find bar, the command bar and the save-password offer.",
      },
      {
        kind: "new",
        area: "Sidebar",
        text: "Removing a pinned tab offers Undo, and deleting a Space says its tabs go with it and that ⌘Z brings them back.",
      },
      {
        kind: "new",
        area: "Settings",
        text: "Each profile can be renamed and says why it can't be deleted, the Widevine row offers to restart ilo once it's downloaded, and Settings and Site Controls work from the keyboard.",
      },
      {
        kind: "new",
        area: "Import",
        text: "Imports count what comes in as it comes, can be cancelled, and offer favorites worked out from your history.",
      },
      {
        kind: "new",
        area: "Accessibility",
        text: "Under Reduce Motion nothing moves or scales; fades stay.",
      },
      {
        kind: "fixed",
        area: "Tabs",
        only: ["mac"],
        text: "Pages no longer flash blank when you switch Spaces or tabs on the Mac.",
      },
      { kind: "new", area: "Updates", text: "Settings › About can take nightly builds." },
      {
        kind: "changed",
        area: "Updates",
        text: "What's New opens as a tab in the main window after an update, on every OS.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "The windows over a page survive Alt+F4, keep the keyboard and move with their window.",
      },
      {
        kind: "fixed",
        area: "Notifications",
        only: ["windows"],
        text: "A click on a site's notification reaches the page.",
      },
      {
        kind: "fixed",
        area: "Agents",
        only: ["windows"],
        text: "Claude Code and Codex keep working with the Microsoft Store copy, across its updates.",
      },
      {
        kind: "fixed",
        area: "Menus",
        only: ["linux"],
        text: "Menus look right with no compositor, and a menu item that opens the command bar keeps it open.",
      },
      {
        kind: "fixed",
        area: "Window",
        text: "Quitting no longer crashes while a window is closing.",
      },
      {
        kind: "fixed",
        area: "Devices",
        text: "A page that asks for a USB, HID, serial or Bluetooth device no longer crashes ilo.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "A minimized window's page is hidden, as on the Mac.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "Back and forward say the page's title, tabs an agent closes stay out of your archive, a link to an app stops in the agent's tab, and a click in another site's frame reaches it.",
      },
      {
        kind: "changed",
        area: "Command bar",
        text: "The command bar puts the open tabs your typing names first, a renamed tab before the rest, and shows one row per title for web apps like Figma and Notion.",
      },
      {
        kind: "changed",
        area: "Spaces",
        text: "A Space swipe's detent comes while your fingers are still down.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "The zoom readout sits under the find bar.",
      },
      {
        kind: "fixed",
        area: "Library",
        text: "A page's full screen in a window that already fills its screen no longer leaves the page over the Library's list.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["linux"],
        text: "A floating call moves by its top bar and sizes from its corner, and the Cast, permission and agent cards share one ink.",
      },
    ],
  },
  {
    version: "0.1.2",
    date: "2 November 2026",
    summary:
      "Little ilo for quick pages and links from other apps, Peek for any link, sidebar backups and more fixes.",
    changes: [
      {
        kind: "new",
        area: "Little ilo",
        clip: "little-ilo",
        text: "A small window for a quick page, from ⌥⌘N (Ctrl+Alt+N on Windows and Linux), ⌥⌘-click on a tab, the Dock's menu or an optional menu bar button. It opens where the pointer is, at the size the last one closed at, and its bar picks the page's profile. ⌘O moves its page to the main window, and ⇧⌘T reopens one you closed.",
      },
      {
        kind: "new",
        area: "Little ilo",
        text: "Links from other apps can open in Little ilo, chosen in Settings › Links. The same link brings back the window it opened.",
      },
      {
        kind: "new",
        area: "Peek",
        clip: "peek",
        text: "⇧-click a link, or choose Open Link in Peek from its menu, to open it in Peek over the page you're on.",
      },
      {
        kind: "changed",
        area: "Peek",
        text: "Peek's page is now the card, with its site and buttons floating over a lighter dim.",
      },
      {
        kind: "new",
        area: "Sidebar",
        clip: "sidebar-backups",
        text: "ilo keeps backups of your sidebar: Help › Restore Earlier Sidebar, or the command bar, brings one back.",
      },
      {
        kind: "new",
        area: "Sidebar",
        text: "A pinned folder moves to another Space from its menu, the command bar or a drag.",
      },
      {
        kind: "new",
        area: "Tabs",
        text: "An unloaded pinned tab's row has a button that removes it from the pinned tabs.",
      },
      {
        kind: "changed",
        area: "Updates",
        text: "Updates download in the background and say when they're ready, on every OS. Their notes and What's New open the release's entry here.",
      },
      {
        kind: "fixed",
        area: "Keyboard",
        only: ["windows"],
        text: "Shortcuts keep working after a window is minimized and restored.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "A link from another app opens once, and opening ilo again brings its window forward.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "A window closed maximized opens maximized again.",
      },
      {
        kind: "fixed",
        area: "Little ilo",
        only: ["windows"],
        text: "Quitting after a browser window closed under Little ilo no longer crashes.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "Menus, the command bar and other overlays stay under another app's window in front of ilo.",
      },
      {
        kind: "fixed",
        area: "Window",
        text: "A long note at the top of the page shortens its words instead of running past the window's edge.",
      },
      {
        kind: "fixed",
        area: "Sidebar",
        text: "A sidebar name field keeps what's typed when it loses the keyboard.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "Connect finds Claude Code wherever it was installed.",
      },
    ],
  },
  {
    version: "0.1.1",
    date: "2 November 2026",
    summary: "Fixes for the 0.1.0 beta, most of them on Windows.",
    changes: [
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "ilo no longer quits when a window moves to another monitor, or is resized after its last tab closed.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "No empty boxes left over the page after a window is minimized and restored.",
      },
      {
        kind: "fixed",
        area: "Window",
        only: ["windows"],
        text: "The app, its windows and their taskbar thumbnails show ilo's icon.",
      },
      {
        kind: "fixed",
        area: "Profiles",
        only: ["windows"],
        text: "Microsoft sign-ins that need a compliant device work on PCs joined to a work or school account.",
      },
      {
        kind: "fixed",
        area: "Window",
        text: "A window no longer shows a leftover page after its last tab closes.",
      },
      {
        kind: "fixed",
        area: "Split view",
        text: "A pinned tab in a split view keeps its pin, and closing its pane only takes it out of the split.",
      },
      {
        kind: "fixed",
        area: "Command bar",
        text: "The command bar always takes the keyboard when it opens.",
      },
      {
        kind: "new",
        area: "First launch",
        text: "Onboarding offers an ilo account, so sync is found on the first run.",
      },
      {
        kind: "fixed",
        area: "Agents",
        text: "A key an agent presses goes to the page only, never to ilo's shortcuts.",
      },
    ],
  },
  {
    version: "0.1.0",
    date: "1 November 2026",
    summary: "The first beta, for people ready to live in it and tell us what breaks.",
    changes: [
      {
        kind: "new",
        area: "Spaces",
        clip: "spaces",
        text: "Spaces, each with its own colour and its own tabs. Swipe with two fingers to move between them.",
      },
      {
        kind: "new",
        area: "Command bar",
        clip: "command-bar",
        text: "The command bar on ⌘T, for addresses, searches, open tabs and every command.",
      },
      {
        kind: "new",
        area: "Sidebar",
        text: "A sidebar with favorites, pinned tabs, folders and Today tabs, which move to the Library's archive when you leave them alone.",
      },
      {
        kind: "new",
        area: "Profiles",
        text: "Profiles that keep logins, cookies and extensions apart.",
      },
      {
        kind: "new",
        area: "Import",
        text: "Bring your tabs, bookmarks and history in from the other browsers on your computer, and passwords from a file you export.",
      },
      {
        kind: "new",
        area: "Sync",
        text: "Sync with an ilo account, end-to-end encrypted, so the server can't read what it keeps. You can run your own.",
      },
      { kind: "new", area: "Passwords", text: "Passwords saved and filled in." },
      { kind: "new", area: "Pages", text: "Split view, find in page, downloads and DevTools." },
      {
        kind: "new",
        area: "Extensions",
        text: "Extensions from the Chrome Web Store, and ads and trackers blocked with uBlock Origin built in.",
      },
      { kind: "new", area: "Pages", text: "Protected video, such as Netflix." },
      {
        kind: "new",
        area: "Updates",
        text: "Updates that install themselves, and crash reports only if you choose to send them.",
      },
    ],
  },
];
