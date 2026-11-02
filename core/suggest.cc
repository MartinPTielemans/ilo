#include "browser_core/suggest.h"

#include <algorithm>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/notreached.h"
#include "base/strings/escape.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/favorites.h"
#include "browser_core/host.h"
#include "browser_core/popular_sites.h"
#include "browser_core/search.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/autocomplete/chrome_autocomplete_provider_client.h"
#include "chrome/browser/autocomplete/chrome_autocomplete_scheme_classifier.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/search_engines/template_url_service_factory.h"
#include "components/omnibox/browser/autocomplete_enums.h"
#include "components/omnibox/browser/autocomplete_input.h"
#include "components/omnibox/browser/autocomplete_match.h"
#include "components/omnibox/browser/autocomplete_provider.h"
#include "components/omnibox/browser/autocomplete_result.h"
#include "components/search_engines/template_url.h"
#include "components/search_engines/template_url_service.h"
#include "components/url_formatter/url_formatter.h"
#include "third_party/metrics_proto/omnibox_event.pb.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// Chrome's address bar's sources that read only what's on the device. Search, with search
// suggestions off, adds just the search for what was typed, and Keyword the search engines' site
// searches.
constexpr int kProviders = static_cast<int>(
    AutocompleteProvider::Type::kBookmark | AutocompleteProvider::Type::kBuiltin |
    AutocompleteProvider::Type::kHistoryQuick | AutocompleteProvider::Type::kHistoryUrl |
    AutocompleteProvider::Type::kKeyword | AutocompleteProvider::Type::kSearch |
    AutocompleteProvider::Type::kShortcuts);

// As Chrome matches a suggestion to an open tab: by URL, without what doesn't change the page.
GURL Stripped(const GURL& url, const TemplateURLService* engines) {
  return AutocompleteMatch::GURLToStrippedGURL(url, AutocompleteInput(), engines,
                                               /*keyword=*/std::u16string(),
                                               /*keep_search_intent_params=*/true);
}

// A tab at |url|, stripped, other than |shown|: a favorite first, which every Space of its profile
// shows, then one in the Space |window| shows. A private window's own tabs alone, as it shows no
// other. 0 if none.
bc_id TabAt(const GURL& url, const Window& window, bc_id shown, const TemplateURLService* engines) {
  std::vector<Space*> spaces = Space::All();
  std::ranges::stable_partition(spaces, [&](Space* space) { return space->id() == window.space(); });
  // Selecting a favorite of another profile shows a Space of that profile, as selecting a tab of
  // another Space does.
  std::vector<bc_id> favorites = Favorites::All();
  if (window.own_space()) {
    Space* own = Space::FromId(window.space());
    spaces = own ? std::vector<Space*>{own} : std::vector<Space*>();
    favorites.clear();
  }
  if (const Space* in = Space::FromId(window.space())) {
    std::ranges::stable_partition(
        favorites, [&](bc_id tab) { return Favorites::ShownIn(tab, *in); });
  }
  std::vector<const std::vector<bc_id>*> lists = {&favorites};
  for (Space* space : spaces) {
    lists.push_back(&space->pinned_rows());
    lists.push_back(&space->today());
  }
  for (const std::vector<bc_id>* tabs : lists) {
    for (bc_id tab : *tabs) {
      // With its page or without, which it gets back as the window shows it.
      const Tab* open = Tab::Get(tab);
      if (tab != shown && open && Stripped(open->Url(), engines) == url) {
        return tab;
      }
    }
  }
  return 0;
}

}  // namespace

Suggestions::Suggestions(bc_id window, Profile* profile)
    : window_(window),
      profile_(profile),
      classifier_(std::make_unique<ChromeAutocompleteSchemeClassifier>(profile)),
      controller_(std::make_unique<ChromeAutocompleteProviderClient>(profile),
                  AutocompleteControllerConfig{.provider_types = kProviders}) {
  observation_.Observe(&controller_);
  profile_observation_.Observe(profile);
}

Suggestions::~Suggestions() = default;

void Suggestions::OnProfileWillBeDestroyed(Profile* profile) {
  NOTREACHED() << "browser: window " << window_ << "'s suggestions outlived their profile";
}

void Suggestions::Start(const std::u16string& keyword,
                        const std::u16string& text,
                        bool complete) {
  keyword_ = keyword;
  text_ = text;
  if (text.empty()) {
    controller_.Stop(AutocompleteStopReason::kClobbered);
    Changed();
    return;
  }
  // In a site search, Chrome's address bar asks for the keyword, a space and what was typed.
  AutocompleteInput input(keyword.empty() ? text : keyword + u' ' + text,
                          metrics::OmniboxEventProto::OTHER, *classifier_);
  input.set_prevent_inline_autocomplete(!complete);
  input.set_in_keyword_mode(!keyword.empty());
  input.set_allow_exact_keyword_match(!keyword.empty());
  controller_.Start(input);
}

void Suggestions::Remove(const GURL& url) {
  const AutocompleteResult& result = controller_.result();
  auto match = std::ranges::find_if(result, [&](const AutocompleteMatch& match) {
    return match.destination_url == url && match.SupportsDeletion();
  });
  if (match != result.end()) {
    // A copy, as removing it changes the result it's in.
    controller_.DeleteMatch(AutocompleteMatch(*match));
  }
}

void Suggestions::OnResultChanged(AutocompleteController* controller, bool default_match_changed) {
  Changed();
}

void Suggestions::Changed() {
  if (report_posted_) {
    return;
  }
  report_posted_ = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Suggestions::Report, weak_factory_.GetWeakPtr()));
}

void Suggestions::Report() {
  report_posted_ = false;
  Window* window = Window::FromId(window_);
  if (!window || !Host().window_suggestions) {
    return;
  }
  const TemplateURLService* engines = TemplateURLServiceFactory::GetForProfile(profile_);
  struct Row {
    std::string url, text, detail, completion;
    bc_id tab = 0;
    std::string keyword, site;
    bool typed_search = false;
    bool typed_url = false;
    bool removable = false;
  };
  std::vector<Row> rows;
  const AutocompleteResult& result = controller_.result();
  for (const AutocompleteMatch& match : result) {
    if (text_.empty() || !match.destination_url.is_valid()) {
      continue;
    }
    const bool search = AutocompleteMatch::IsSearchType(match.type);
    // A page by its title, then its URL; a search by its words, then its engine.
    const bool titled = !search && !match.description.empty();
    // An extension's keyword needs its events, which the bar doesn't send.
    const TemplateURL* engine = match.associated_keyword.empty()
                                    ? nullptr
                                    : engines->GetTemplateURLForKeyword(match.associated_keyword);
    const bool site_search = engine && engine->type() == TemplateURL::NORMAL;
    rows.push_back({
        .url = match.destination_url.spec(),
        .text = base::UTF16ToUTF8(titled ? match.description : match.contents),
        .detail = base::UTF16ToUTF8(titled ? match.contents : match.description),
        .completion = &match == result.default_match()
                          ? base::UTF16ToUTF8(match.inline_autocompletion)
                          : std::string(),
        // The first goes where what was typed goes, as ↩ does in Chrome's address bar.
        .tab = search || rows.empty() ? 0
                                      : TabAt(Stripped(match.destination_url, engines), *window,
                                              window->selected(), engines),
        .keyword = site_search ? base::UTF16ToUTF8(engine->keyword()) : std::string(),
        .site = site_search ? base::UTF16ToUTF8(engine->short_name()) : std::string(),
        .typed_search = match.type == omnibox::AutocompleteMatchType::kSearchWhatYouTyped,
        .typed_url = match.type == omnibox::AutocompleteMatchType::kUrlWhatYouTyped,
        .removable = match.SupportsDeletion(),
    });
  }
  // Typed with its scheme, the address goes first, where ↩ goes, though Chrome's address bar puts
  // the search first for a host that can't resolve (TypedWithScheme).
  if (auto typed = std::ranges::find_if(rows, &Row::typed_url);
      keyword_.empty() && typed != rows.end() && rows.front().typed_search &&
      TypedWithScheme(base::UTF16ToUTF8(text_)).is_valid()) {
    typed->tab = 0;
    std::rotate(rows.begin(), typed, typed + 1);
  }
  // As Dia lists them: the search for what was typed right after the top row.
  if (auto search = std::ranges::find_if(rows, &Row::typed_search);
      search != rows.end() && search != rows.begin()) {
    std::rotate(rows.begin() + 1, search, search + 1);
  }
  // Then well-known sites, in the rows Chrome's address bar leaves empty, so a new profile lists
  // more than the search. They never go above the profile's own, or first, where ↩ goes, and a
  // site search lists only searches.
  const std::vector<PopularSite> sites = rows.empty() || !keyword_.empty()
                                             ? std::vector<PopularSite>()
                                             : PopularSitesFor(text_);
  for (const PopularSite& site : sites) {
    if (rows.size() >= AutocompleteResult::GetMaxMatches()) {
      break;
    }
    const GURL url(site.url);
    const GURL stripped = Stripped(url, engines);
    if (std::ranges::any_of(rows, [&](const Row& row) {
          return Stripped(GURL(row.url), engines) == stripped;
        })) {
      continue;
    }
    rows.push_back({
        .url = url.spec(),
        .text = std::string(site.name),
        // As Chrome shows a page's URL in its rows.
        .detail = base::UTF16ToUTF8(url_formatter::FormatUrl(
            url, AutocompleteMatch::GetFormatTypes(/*preserve_scheme=*/false,
                                                   /*preserve_subdomain=*/false),
            base::UnescapeRule::SPACES, nullptr, nullptr, nullptr)),
        .tab = TabAt(stripped, *window, window->selected(), engines),
    });
  }
  std::vector<bc_suggestion> suggestions;
  for (const Row& row : rows) {
    suggestions.push_back({.url = row.url.c_str(),
                           .text = row.text.c_str(),
                           .detail = row.detail.c_str(),
                           .tab = row.tab,
                           .completion = row.completion.c_str(),
                           .removable = row.removable,
                           .keyword = row.keyword.c_str(),
                           .site = row.site.c_str()});
  }
  const std::string text =
      base::UTF16ToUTF8(keyword_.empty() ? text_ : keyword_ + u' ' + text_);
  Host().window_suggestions(Host().context, window_, text.c_str(), suggestions.data(),
                            suggestions.size());
}

}  // namespace browser_core
