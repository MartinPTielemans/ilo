#include "browser_core/page_translate.h"

#include <optional>
#include <utility>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "chrome/browser/translate/chrome_translate_client.h"
#include "chrome/common/chrome_isolated_world_ids.h"
#include "components/translate/core/common/language_detection_details.h"
#include "content/public/browser/page.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

namespace {

// The browser's script, run in Chrome's translate world, which shares the page's document but none
// of its script: called with what to do ("read", "show" or "revert"), the read's request and the
// texts to show. What it read stays in that world's global until the page goes. Each run keeps the
// text the page had and the one shown, so a run the page has changed since is left as the page
// has it either way.
constexpr char16_t kScript[] =
    uR"js((function (op, request, texts) {
  const state = globalThis.__iloTranslate;
  const revert = () => {
    if (!state) return false;
    if (state.shownTitle !== null && document.title === state.shownTitle) {
      document.title = state.title;
    }
    for (const run of state.runs) {
      if (run.shown !== null && run.node.data === run.shown) run.node.data = run.original;
    }
    globalThis.__iloTranslate = undefined;
    return true;
  };
  if (op === 'revert') return revert();
  if (op === 'read') {
    revert();
    const root = document.body || document.documentElement;
    const runs = [];
    if (root && !root.closest('[translate="no"], .notranslate')) {
      const skipped = new Set(['script', 'style', 'noscript', 'template', 'textarea', 'select',
                               'code', 'kbd', 'samp']);
      const letter = /\p{L}/u;
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT,
        { acceptNode(node) {
          if (node.nodeType === Node.TEXT_NODE) {
            return letter.test(node.data) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP;
          }
          return skipped.has(node.localName) || node.getAttribute('translate') === 'no' ||
                  node.classList.contains('notranslate') || node.isContentEditable
              ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_SKIP;
        } });
      for (let node; runs.length < 5000 && (node = walker.nextNode());) {
        const [, lead, words, trail] = /^(\s*)([\s\S]*?)(\s*)$/.exec(node.data);
        runs.push({ node, lead, words, trail, original: node.data, shown: null });
      }
    }
    globalThis.__iloTranslate = { request, title: document.title, shownTitle: null, runs };
    return [document.title.trim(), ...runs.map((run) => run.words)];
  }
  if (op !== 'show' || !state || state.request !== request || !Array.isArray(texts) ||
      texts.length !== state.runs.length + 1 || texts.some((text) => typeof text !== 'string')) {
    return false;
  }
  if (texts[0] && document.title === state.title) {
    document.title = state.shownTitle = texts[0];
  }
  state.runs.forEach((run, index) => {
    const text = texts[index + 1];
    if (!text || run.node.data !== run.original) return;
    run.shown = run.lead + text + run.trail;
    run.node.data = run.shown;
  });
  return true;
}))js";

void ReportTexts(bc_id tab, uint64_t request, std::optional<std::string> texts) {
  if (Host().tab_translate_texts) {
    Host().tab_translate_texts(Host().context, tab, request, texts ? texts->c_str() : nullptr);
  }
}

}  // namespace

PageTranslate::PageTranslate(content::WebContents* contents, bc_id tab)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<PageTranslate>(*contents),
      tab_(tab) {
  ObserveDriver();
}

void PageTranslate::ObserveDriver() {
  if (driver_observation_.IsObserving()) {
    return;
  }
  if (auto* client = ChromeTranslateClient::FromWebContents(web_contents())) {
    driver_observation_.Observe(client->GetTranslateDriver());
  }
}

PageTranslate::~PageTranslate() = default;

// static
void PageTranslate::ReportNoTexts(bc_id tab, uint64_t request) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&ReportTexts, tab, request, std::optional<std::string>()));
}

void PageTranslate::Read(uint64_t request) {
  reading_.insert(request);
  Run("read", request, "null",
      base::BindOnce(&PageTranslate::Gathered, weak_factory_.GetWeakPtr(), request));
}

void PageTranslate::Show(uint64_t request, std::string_view texts) {
  // Only an array goes into the script, as the page's words came out of it: JSON, so nothing in a
  // translation can be read as script.
  std::optional<base::Value> parsed = base::JSONReader::Read(texts, base::JSON_PARSE_RFC);
  if (!parsed || !parsed->is_list()) {
    return;
  }
  std::optional<std::string> json = base::WriteJson(*parsed);
  if (json) {
    Run("show", request, *json, base::DoNothing());
  }
}

void PageTranslate::Revert() {
  Run("revert", 0, "null", base::DoNothing());
}

void PageTranslate::Run(std::string_view op,
                        uint64_t request,
                        std::string_view texts,
                        content::RenderFrameHost::JavaScriptResultCallback done) {
  content::RenderFrameHost* frame = GetWebContents().GetPrimaryMainFrame();
  if (!frame || !frame->IsRenderFrameLive()) {
    std::move(done).Run(base::Value());
    return;
  }
  // The request as a string, as a JavaScript number can't hold every 64-bit one.
  frame->ExecuteJavaScriptInIsolatedWorld(
      base::StrCat({kScript, u"(\"", base::UTF8ToUTF16(op), u"\", \"",
                    base::NumberToString16(request), u"\", ", base::UTF8ToUTF16(texts), u")"}),
      std::move(done), ISOLATED_WORLD_ID_TRANSLATE);
}

void PageTranslate::Gathered(uint64_t request, base::Value texts) {
  if (!reading_.erase(request)) {
    // The page went first, and the app has heard.
    return;
  }
  std::optional<std::string> json;
  if (texts.is_list()) {
    json = base::WriteJson(texts);
  }
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&ReportTexts, tab_, request, std::move(json)));
}

void PageTranslate::ReportLanguage(const std::string& language) {
  language_ = language;
  if (Host().tab_language) {
    Host().tab_language(Host().context, tab_, language.c_str());
  }
}

void PageTranslate::PrimaryPageChanged(content::Page& page) {
  // A new document has none of what the script showed, nor its words to answer a read with.
  for (uint64_t request : std::exchange(reading_, {})) {
    ReportNoTexts(tab_, request);
  }
  // Told even when the last page's was empty, so the app forgets a translation shown on it.
  ReportLanguage(std::string());
}

void PageTranslate::WebContentsDestroyed() {
  for (uint64_t request : std::exchange(reading_, {})) {
    ReportNoTexts(tab_, request);
  }
  driver_observation_.Reset();
}

void PageTranslate::OnTranslateDriverDestroyed(translate::TranslateDriver* driver) {
  driver_observation_.Reset();
}

void PageTranslate::OnLanguageDetermined(const translate::LanguageDetectionDetails& details) {
  // Each page is registered as it loads, and again after; only the page shown counts.
  if (details.url != GetWebContents().GetLastCommittedURL()) {
    return;
  }
  std::string language = details.adopted_language.empty() ? "und" : details.adopted_language;
  if (language != language_) {
    ReportLanguage(language);
  }
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(PageTranslate);

}  // namespace browser_core
