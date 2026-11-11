#include "browser_core/suggest.h"

#include <algorithm>
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
#include "browser_core/favorites.h"
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
#include "components/history/core/browser/history_backend.h"
#include "components/history/core/browser/history_database.h"
#include "components/history/core/browser/history_db_task.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_types.h"
#include "components/history/core/browser/keyword_search_term.h"
#include "components/history/core/browser/url_row.h"
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

// A page Remove took out of history, as it was: its row, its visits oldest first and where each
// came from, and the term it searched for, if it was a search's results.
struct Page {
  history::URLRow row;
  history::VisitVector visits;
  history::VisitSourceMap sources;
  // Chromium 157 makes this an IdType, whose default is the invalid ID, 0.
  history::KeywordID keyword{};
  std::u16string term;
};

}  // namespace

// Filled and read only on the history database's thread, in the order the tasks were posted there,
// so the main thread holds it without reading it.
struct Suggestions::Removed : base::RefCountedThreadSafe<Removed> {
  std::vector<Page> pages;

 private:
  friend class base::RefCountedThreadSafe<Removed>;
  ~Removed() = default;
};

namespace {

// Reads what a removal takes out: |pages|, and the pages of each search for a term with an engine's
// keyword, as Chrome's DeleteMatchingURLsForKeyword finds them. With |take_out|, takes them out
// then, as Chrome's providers do. Then |done|. Another task queued ahead of it on the history
// sequence runs it later than what was posted after it, so what reads before a removal does the
// removal or has it done.
class Keep : public history::HistoryDBTask {
 public:
  Keep(scoped_refptr<Suggestions::Removed> removed,
       std::vector<GURL> pages,
       std::vector<std::pair<history::KeywordID, std::u16string>> searches,
       bool take_out,
       base::OnceClosure done)
      : removed_(std::move(removed)),
        pages_(std::move(pages)),
        searches_(std::move(searches)),
        take_out_(take_out),
        done_(std::move(done)) {}

  bool RunOnDBThread(history::HistoryBackend* backend, history::HistoryDatabase* db) override {
    if (!db) {
      return true;
    }
    Read(db);
    if (take_out_) {
      if (!pages_.empty()) {
        backend->DeleteURLs(pages_);
      }
      for (const auto& [keyword, term] : searches_) {
        backend->DeleteMatchingURLsForKeyword(keyword, term);
      }
    }
    return true;
  }

  void DoneRunOnMainThread() override { std::move(done_).Run(); }

 private:
  void Read(history::HistoryDatabase* db) {
    std::vector<history::URLID> ids;
    for (const GURL& url : pages_) {
      history::URLRow row;
      if (history::URLID id = db->GetRowForURL(url, &row)) {
        ids.push_back(id);
      }
    }
    for (const auto& [keyword, term] : searches_) {
      std::vector<history::KeywordSearchTermRow> rows;
      db->GetKeywordSearchTermRows(term, &rows);
      for (const history::KeywordSearchTermRow& row : rows) {
        if (row.keyword_id == keyword) {
          ids.push_back(row.url_id);
        }
      }
    }
    std::ranges::sort(ids);
    ids.erase(std::ranges::unique(ids).begin(), ids.end());
    for (history::URLID id : ids) {
      Page page;
      if (!db->GetURLRow(id, &page.row)) {
        continue;
      }
      db->GetVisitsForURL(id, &page.visits);
      std::ranges::sort(page.visits, {}, &history::VisitRow::visit_time);
      db->GetVisitsSource(page.visits, &page.sources);
      if (history::KeywordSearchTermRow term; db->GetKeywordSearchTermRow(id, &term)) {
        page.keyword = term.keyword_id;
        page.term = term.term;
      }
      removed_->pages.push_back(std::move(page));
    }
  }

  const scoped_refptr<Suggestions::Removed> removed_;
  const std::vector<GURL> pages_;
  const std::vector<std::pair<history::KeywordID, std::u16string>> searches_;
  const bool take_out_;
  base::OnceClosure done_;
};

// Puts the pages a removal took out back: each visit again, at its time with its transition, so
// the history service counts the page's visits and typed visits as it did the first time, then its
// title and the term it searched for. Then |done|.
class PutBack : public history::HistoryDBTask {
 public:
  PutBack(scoped_refptr<Suggestions::Removed> removed, base::OnceClosure done)
      : removed_(std::move(removed)), done_(std::move(done)) {}

  bool RunOnDBThread(history::HistoryBackend* backend, history::HistoryDatabase* db) override {
    for (const Page& page : removed_->pages) {
      const GURL& url = page.row.url();
      // A page with no visits left, as one only bookmarked, comes back as a row with one.
      if (page.visits.empty()) {
        backend->AddPagesWithDetails({page.row}, history::SOURCE_BROWSED);
      }
      for (const history::VisitRow& visit : page.visits) {
        const auto source = page.sources.find(visit.visit_id);
        backend->AddPage(history::HistoryAddPageArgs(
            url, visit.visit_time, history::ContextID(), /*nav_entry_id=*/0,
            /*local_navigation_id=*/std::nullopt, /*referrer=*/GURL(), history::RedirectList(),
            visit.transition, page.row.hidden(),
            source == page.sources.end() ? history::SOURCE_BROWSED : source->second,
            history::VisitResponseCodeCategory::kNot404, /*did_replace_entry=*/false,
            visit.consider_for_ntp_most_visited));
      }
      backend->SetPageTitle(url, page.row.title());
      if (page.keyword) {
        backend->SetKeywordSearchTermsForURL(url, page.keyword, page.term);
      }
    }
    return true;
  }

  void DoneRunOnMainThread() override { std::move(done_).Run(); }

 private:
  const scoped_refptr<Suggestions::Removed> removed_;
  base::OnceClosure done_;
};

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
  static bc_id last = 0;
  const bc_id id = ++last;
  // Chrome's deletion once what it takes out has been read, so it's read as it was.
  base::OnceClosure take_out = base::BindOnce(&Suggestions::TakeOut, weak_factory_.GetWeakPtr(),
                                              id, removing);
  history::HistoryService* history =
      HistoryServiceFactory::GetForProfile(profile_, ServiceAccessType::EXPLICIT_ACCESS);
  if (history) {
    removal.removed = base::MakeRefCounted<Removed>();
    history->ScheduleDBTask(FROM_HERE,
                            std::make_unique<Keep>(removal.removed, removal.pages, removal.searches,
                                                   /*take_out=*/false, std::move(take_out)),
                            &history_tasks_);
  }
  removals_[id] = std::move(removal);
  if (removals_.size() > kKept) {
    removals_.erase(removals_.begin());
  }
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
  history->ScheduleDBTask(FROM_HERE, std::make_unique<PutBack>(removal->second.removed, ListAgain()),
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
  removal->second.removed = base::MakeRefCounted<Removed>();
  history->ScheduleDBTask(
      FROM_HERE,
      std::make_unique<Keep>(removal->second.removed, removal->second.pages,
                             removal->second.searches, /*take_out=*/true, ListAgain()),
      &history_tasks_);
  return true;
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
                           .site = row.site.c_str()});
  }
  const std::string text =
      base::UTF16ToUTF8(keyword_.empty() ? text_ : keyword_ + u' ' + text_);
  Host().window_suggestions(Host().context, window_, text.c_str(), suggestions.data(),
                            suggestions.size());
}

}  // namespace browser_core
