import { PLATFORM_LIST } from "./downloads";
import { SITE } from "./seo";

// A phone can't run ilo, so there the download button sends ilo's address to the visitor's computer
// instead: through the phone's own share sheet, or as a mail to themselves where there's none.
// Nothing leaves the phone but what the visitor sends.

/** What the button says, and `brief` where there's little room. */
export const SEND_LABEL = { full: "Send ilo to my computer", brief: "Send to my computer" };

/** What's sent: the address to open on the computer, and a line saying what it is. */
export const SEND = {
  title: "ilo",
  text: `ilo, a free, open-source browser for ${PLATFORM_LIST}`,
  url: `${SITE}/`,
};

export type Sent = typeof SEND;

/**
 * A mail with `sent` in it, addressed to no one, for the visitor to send themselves. The reading
 * pages' script carries this function's source (src/reading.ts), so it uses nothing outside itself.
 */
export function mailFor(sent: Sent) {
  return `mailto:?subject=${encodeURIComponent(sent.title)}&body=${encodeURIComponent(`${sent.text}\n${sent.url}`)}`;
}
