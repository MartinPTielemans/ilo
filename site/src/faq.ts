import { PLATFORMS, STAGE } from "./downloads";

// The questions people ask about ilo, answered in a sentence or two that stands on its own, so a
// search engine or an AI answer can quote it whole. Help shows them, and gives them to search
// engines as an FAQPage. Every answer says no more than the rest of the site does.
export const FAQ: { question: string; answer: string }[] = [
  {
    question: "What is ilo?",
    answer:
      "ilo is a free, open-source web browser for Mac, Windows and Linux, built on Chromium. It keeps each part of your life in its own Space, down a sidebar that tidies itself, with one command bar for addresses, searches, open tabs and commands.",
  },
  {
    question: "Is ilo free?",
    answer:
      "Yes. The whole browser is free on every computer it runs on, with no ads, no licence checks and no paid features. It's GPL-3.0, and it makes no money. If it ever does, the browser itself stays free; any charge would probably be for hosting a service you can also run yourself, such as sync.",
  },
  {
    question: "Is ilo open source?",
    answer:
      "Yes. All of ilo that runs on your computer is under the GPL-3.0, and its source is on GitHub at github.com/MartinPTielemans/ilo.",
  },
  {
    question: "What is ilo built on?",
    answer:
      "ilo is built on ungoogled-chromium, which cuts Chromium's calls to Google, with an interface of its own written in Rust with GPUI. Sites work as they do in Chrome.",
  },
  {
    question: "Which computers does ilo run on?",
    answer: `${PLATFORMS.map((platform) => `${platform.name}: ${platform.needs.charAt(0).toLowerCase()}${platform.needs.slice(1)}`).join(". ")}. ilo is ${STAGE} on all three.`,
  },
  {
    question: "Does ilo work with Chrome extensions?",
    answer:
      "Yes. Chrome's extensions work in ilo: install them from the Chrome Web Store, and find them in the Extensions menu.",
  },
  {
    question: "Does ilo block ads?",
    answer:
      "Yes. ilo blocks ads and trackers from the first page with uBlock Origin, built in. You can turn blocking off for a site in Site Controls, or everywhere in Settings.",
  },
  {
    question: "Does ilo collect my data?",
    answer:
      "Only one anonymous count a day, and only if you turn it on. ilo asks once during setup, and you can change your choice in Settings › Privacy. The count says which version of ilo you have, your operating system and chip, and whether it's the copy's first run or its first count this week or month. It has no ID and nothing about the pages you visit or what you do in ilo. ilo needs no account and has no ads. Crash reports leave your computer only if you choose to send them. ilobrowser.com/privacy lists every service ilo talks to.",
  },
  {
    question: "Can I move my tabs and bookmarks to ilo?",
    answer:
      "Yes. Choose Import from Other Browsers… in the ilo menu, or type “import” in the command bar, to bring your tabs, bookmarks and history from another browser.",
  },
  {
    question: "Does ilo sync between computers?",
    answer:
      "Yes, with an ilo account, end-to-end encrypted. It syncs your Spaces and your first profile's bookmarks, passwords and open tabs, encrypted with 24 recovery words only your devices have. It uses ilo's server unless you run your own, and needs no Google account.",
  },
  {
    question: "Who makes ilo?",
    answer:
      "ilo is made by Martin P. Tielemans, in the open. Because it's GPL, if he ever stops, anyone can carry on from the code.",
  },
];

/** The questions as schema.org's FAQPage, for search engines and answer engines. */
export const faqData = () => ({
  "@context": "https://schema.org",
  "@type": "FAQPage",
  mainEntity: FAQ.map(({ question, answer }) => ({
    "@type": "Question",
    name: question,
    acceptedAnswer: { "@type": "Answer", text: answer },
  })),
});
