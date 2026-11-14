#include "browser_core/suggest.h"

#include <algorithm>
#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/memory/ref_counted.h"
#include "base/notreached.h"
#include "base/strings/escape.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/favorites.h"
#include "browser_core/history_removal.h"
#include "browser_core/host.h"
#include "browser_core/popular_sites.h"
#include "browser_core/search.h"
#include "browser_core/site_searches.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/autocomplete/chrome_autocomplete_provider_client.h"
#include "chrome/browser/autocomplete/chrome_autocomplete_scheme_classifier.h"
#include "chrome/browser/history/history_service_factory.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/search_engines/template_url_service_factory.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_types.h"
#include "components/keyed_service/core/service_access_type.h"
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

// When the profile's history last saw a visit to |match|'s page, in microseconds since 1970, or 0
// (#2337). engine/patches/ours/chromium-omnibox-match-last-visit.patch fills last_visit on the
// matches HistoryURL and HistoryQuick make from a history row, and a bookmark's or a shortcut's
// match that won over one has it among its duplicates. No history lookup: the bar is a hot path.
int64_t LastVisit(const AutocompleteMatch& match) {
  base::Time last = match.last_visit;
  for (const AutocompleteMatch& duplicate : match.duplicate_matches) {
    last = std::max(last, duplicate.last_visit);
  }
  return last.is_null() ? 0 : (last - base::Time::UnixEpoch()).InMicroseconds();
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

Suggestions::Removal::Removal() = default;
Suggestions::Removal::Removal(Removal&&) = default;
Suggestions::Removal& Suggestions::Removal::operator=(Removal&&) = default;
Suggestions::Removal::~Removal() = default;

bc_id Suggestions::Remove(const GURL& url) {
  const AutocompleteResult& result = controller_.result();
  auto match = std::ranges::find_if(result, [&](const AutocompleteMatch& match) {
    return match.destination_url == url && match.SupportsDeletion();
  });
  // Its row stays until Chrome's deletion, which waits for what it takes out to be read.
  const bool being_taken = std::ranges::any_of(removals_, [&](const auto& removal) {
    return removal.second.url == url && (removal.second.state == Removal::State::kTaking ||
                                         removal.second.state == Removal::State::kTakingThenBack);
  });
  if (match == result.end() || being_taken) {
    return 0;
  }
  // A copy, as removing it changes the result it's in.
  const AutocompleteMatch removing(*match);
  // What Chrome's deletion takes out, the match's and those of its duplicates: a search's term with
  // the pages it searched, or a page, as each provider's DeleteMatch does.
  TemplateURLService* engines = TemplateURLServiceFactory::GetForProfile(profile_);
  Removal removal;
  removal.url = url;
  std::vector<const AutocompleteMatch*> taken = {&removing};
  for (const AutocompleteMatch& duplicate : removing.duplicate_matches) {
    taken.push_back(&duplicate);
  }
  for (const AutocompleteMatch* taking : taken) {
    if (!taking->deletable || !taking->provider) {
      continue;
    }
    if (taking->provider->type() != AutocompleteProvider::Type::kSearch) {
      removal.pages.push_back(taking->destination_url);
    } else if (const TemplateURL* engine = taking->GetTemplateURL(engines)) {
      removal.searches.emplace_back(engine->id(), taking->contents);
    }
  }
  const bc_id id = NextRemoval();
  // Chrome's deletion once what it takes out has been read, so it's read as it was.
  base::OnceClosure take_out = base::BindOnce(&Suggestions::TakeOut, weak_factory_.GetWeakPtr(),
                                              id, removing);
  history::HistoryService* history =
      HistoryServiceFactory::GetForProfile(profile_, ServiceAccessType::EXPLICIT_ACCESS);
  if (history) {
    removal.removed = base::MakeRefCounted<RemovedHistory>();
    history->ScheduleDBTask(
        FROM_HERE,
        std::make_unique<KeepHistory>(removal.removed, removal.pages, removal.searches,
                                      removal.begin, removal.end, /*take_out=*/false,
                                      std::move(take_out)),
        &history_tasks_);
  }
  Hold(id, std::move(removal));
  if (!history) {
    std::move(take_out).Run();
  }
  return id;
}

void Suggestions::TakeOut(bc_id id, const AutocompleteMatch& match) {
  controller_.DeleteMatch(match);
  auto removal = removals_.find(id);
  if (removal == removals_.end()) {
    return;
  }
  const bool back = removal->second.state == Removal::State::kTakingThenBack;
  removal->second.state = Removal::State::kOut;
  if (back) {
    Restore(id);
  }
}

bool Suggestions::Restore(bc_id id) {
  auto removal = removals_.find(id);
  history::HistoryService* history =
      HistoryServiceFactory::GetForProfile(profile_, ServiceAccessType::EXPLICIT_ACCESS);
  if (removal == removals_.end() || !removal->second.removed || !history) {
    return false;
  }
  // A ⌘Z before the history service has read it, as while it loads, puts it back once it's out,
  // since Chrome's deletion is posted only then.
  switch (removal->second.state) {
    case Removal::State::kTaking:
      removal->second.state = Removal::State::kTakingThenBack;
      return true;
    case Removal::State::kOut:
      break;
    case Removal::State::kTakingThenBack:
    case Removal::State::kBack:
      return false;
  }
  removal->second.state = Removal::State::kBack;
  history->ScheduleDBTask(FROM_HERE,
                          std::make_unique<PutBackHistory>(removal->second.removed, ListAgain()),
                          &history_tasks_);
  return true;
}

bool Suggestions::RemoveAgain(bc_id id) {
  auto removal = removals_.find(id);
  history::HistoryService* history =
      HistoryServiceFactory::GetForProfile(profile_, ServiceAccessType::EXPLICIT_ACCESS);
  if (removal == removals_.end() || !history) {
    return false;
  }
  if (removal->second.state == Removal::State::kTakingThenBack) {
    removal->second.state = Removal::State::kTaking;
    return true;
  }
  if (removal->second.state != Removal::State::kBack) {
    return false;
  }
  // Read as it is now, the visits since it came back with it, then taken out in the same task, so
  // a Restore after it, posted after it, puts back what it read.
  removal->second.state = Removal::State::kOut;
  removal->second.removed = base::MakeRefCounted<RemovedHistory>();
  history->ScheduleDBTask(
      FROM_HERE,
      std::make_unique<KeepHistory>(removal->second.removed, removal->second.pages,
                                    removal->second.searches, removal->second.begin,
                                    removal->second.end, /*take_out=*/true, ListAgain()),
      &history_tasks_);
  return true;
}

bc_id Suggestions::RemoveVisits(const GURL& url, base::Time begin, base::Time end) {
  // As the Library's History lists them: none for a private profile, whose pages nothing keeps.
  history::HistoryService* history =
      profile_->IsOffTheRecord()
          ? nullptr
          : HistoryServiceFactory::GetForProfile(profile_, ServiceAccessType::EXPLICIT_ACCESS);
  if (!history || !history->BackendLoaded() || !url.is_valid() || begin >= end) {
    return 0;
  }
  Removal removal;
  removal.url = url;
  removal.pages = {url};
  removal.begin = begin;
  removal.end = end;
  // Read and taken out in one task, so it's out at once: no list, search or suggestion asked after
  // shows the visits, nothing but this removal keeps them, and a Restore, posted after it, puts
  // back what it read.
  removal.state = Removal::State::kOut;
  removal.removed = base::MakeRefCounted<RemovedHistory>();
  history->ScheduleDBTask(
      FROM_HERE,
      std::make_unique<KeepHistory>(removal.removed, removal.pages, removal.searches, begin, end,
                                    /*take_out=*/true, ListAgain()),
      &history_tasks_);
  const bc_id id = NextRemoval();
  Hold(id, std::move(removal));
  return id;
}

// static
bc_id Suggestions::NextRemoval() {
  static bc_id last = 0;
  return ++last;
}

void Suggestions::Hold(bc_id id, Removal removal) {
  removals_[id] = std::move(removal);
  if (removals_.size() > kKept) {
    removals_.erase(removals_.begin());
  }
}

base::OnceClosure Suggestions::ListAgain() {
  return base::BindOnce(
      [](base::WeakPtr<Suggestions> suggestions) {
        if (suggestions && !suggestions->text_.empty()) {
          suggestions->Start(suggestions->keyword_, suggestions->text_, /*complete=*/false);
        }
      },
      weak_factory_.GetWeakPtr());
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
  TemplateURLService* engines = TemplateURLServiceFactory::GetForProfile(profile_);
  struct Row {
    std::string url, text, detail, completion;
    bc_id tab = 0;
    std::string keyword, site;
    bool typed_search = false;
    bool typed_url = false;
    bool removable = false;
    int64_t last_visit = 0;
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
        .last_visit = search ? 0 : LastVisit(match),
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
  // A word typed hints the site search it starts on the first row, where Tab looks, whatever that
  // row is; one with a space keeps Chrome's, whose first word is the keyword itself.
  if (!rows.empty() && keyword_.empty() &&
      std::ranges::none_of(text_, [](char16_t c) { return base::IsUnicodeWhitespace(c); })) {
    const TemplateURL* engine = SiteStarted(engines, text_);
    rows.front().keyword = engine ? base::UTF16ToUTF8(engine->keyword()) : std::string();
    rows.front().site = engine ? base::UTF16ToUTF8(engine->short_name()) : std::string();
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
                           .site = row.site.c_str(),
                           .last_visit = row.last_visit});
  }
  const std::string text =
      base::UTF16ToUTF8(keyword_.empty() ? text_ : keyword_ + u' ' + text_);
  Host().window_suggestions(Host().context, window_, text.c_str(), suggestions.data(),
                            suggestions.size());
}

}  // namespace browser_core
