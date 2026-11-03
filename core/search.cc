#include "browser_core/search.h"

#include <memory>
#include <string>
#include <string_view>
#include <vector>

#include "base/callback_list.h"
#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/supports_user_data.h"
#include "browser_core/search_default.h"
#include "chrome/browser/autocomplete/autocomplete_classifier_factory.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "chrome/browser/search_engines/template_url_service_factory.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/omnibox/browser/autocomplete_classifier.h"
#include "components/omnibox/browser/autocomplete_match.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "components/search_engines/template_url.h"
#include "components/search_engines/template_url_data.h"
#include "components/search_engines/template_url_service.h"
#include "third_party/metrics_proto/omnibox_event.pb.h"

namespace browser_core {

namespace {

// ungoogled-chromium's engine in Google's place, which "searches" by going to http://<the words>.
constexpr char16_t kNoSearch[] = u"nosearch";

// Whether a profile has had kSiteSearches, once, so one the user deletes stays deleted.
constexpr char kSiteSearchesSeeded[] = "browser_core.site_searches_seeded";

// A site search a new profile has, so Tab after the start of its host searches it.
struct SiteSearch {
  std::string_view name;
  // The site's host, as Chrome names a site's search, and what Tab is offered for.
  std::string_view keyword;
  std::string_view url;
};

// Arc's 19, which it seeds active into Chromium's keywords table (#1397). X has twitter.com's too,
// so a search typed by its old name finds it.
constexpr SiteSearch kSiteSearches[] = {
    {"Google Drive", "drive.google.com", "https://drive.google.com/drive/search?q={searchTerms}"},
    {"Gmail", "mail.google.com", "https://mail.google.com/mail/u/0/#search/{searchTerms}"},
    {"Google Maps", "maps.google.com", "https://www.google.com/maps/search/{searchTerms}"},
    {"Google Calendar", "calendar.google.com",
     "https://calendar.google.com/calendar/r/search?q={searchTerms}"},
    {"Google Docs", "docs.google.com", "https://docs.google.com/document/u/0/?q={searchTerms}"},
    {"Dropbox", "dropbox.com", "https://www.dropbox.com/search/personal?query={searchTerms}"},
    {"Amazon", "amazon.com", "https://www.amazon.com/s?k={searchTerms}"},
    {"GitHub", "github.com", "https://github.com/search?q={searchTerms}"},
    {"Figma", "figma.com", "https://www.figma.com/files/search?q={searchTerms}"},
    {"X", "x.com", "https://x.com/search?q={searchTerms}"},
    {"X", "twitter.com", "https://x.com/search?q={searchTerms}"},
    {"YouTube", "youtube.com", "https://www.youtube.com/results?search_query={searchTerms}"},
    {"Wikipedia", "wikipedia.org", "https://en.wikipedia.org/wiki/Special:Search?search={searchTerms}"},
    {"Reddit", "reddit.com", "https://www.reddit.com/search/?q={searchTerms}"},
    {"SoundCloud", "soundcloud.com", "https://soundcloud.com/search?q={searchTerms}"},
    {"Asana", "asana.com", "https://app.asana.com/0/search?q={searchTerms}"},
    {"LinkedIn", "linkedin.com", "https://www.linkedin.com/search/results/all/?keywords={searchTerms}"},
    {"Apple Music", "music.apple.com", "https://music.apple.com/search?term={searchTerms}"},
    {"Spotify", "spotify.com", "https://open.spotify.com/search/{searchTerms}"},
};

// A profile's wait for its engines to load, held on the profile.
struct SeedOnLoad : base::SupportsUserData::Data {
  base::CallbackListSubscription loaded;
};
constexpr char kSeedOnLoad[] = "browser_core.seed_site_searches";

// Builds no service: Chrome registers a profile's prefs through its factories.
class PrefsFactory : public ProfileKeyedServiceFactory {
 public:
  static PrefsFactory* GetInstance() {
    static base::NoDestructor<PrefsFactory> instance;
    return instance.get();
  }

  PrefsFactory() : ProfileKeyedServiceFactory("BrowserCoreSearch") {}

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return nullptr;
  }

  void RegisterProfilePrefs(user_prefs::PrefRegistrySyncable* registry) override {
    RegisterSearchDefaultPrefs(registry);
    registry->RegisterBooleanPref(kSiteSearchesSeeded, false);
  }
};

// |profile|'s engines once they've loaded.
TemplateURLService* LoadedEngines(Profile* profile) {
  TemplateURLService* engines = profile ? TemplateURLServiceFactory::GetForProfile(profile) : nullptr;
  return engines && engines->loaded() ? engines : nullptr;
}

// Whether the user can make |engine| the default, or has: none while a policy or an extension
// picks it. Chrome's CanMakeDefault is false for the default itself, which the user's list keeps.
bool IsChoice(TemplateURLService* engines, const TemplateURL* engine) {
  if (engine->keyword() == kNoSearch || !engines->ShowInDefaultList(engine)) {
    return false;
  }
  return engine == engines->GetDefaultSearchProvider()
             ? engines->CanDefaultSearchProviderBeModifiedByUser()
             : engines->CanMakeDefault(engine);
}

// Gives |profile| kSiteSearches, once its engines have loaded. One whose keyword it has already keeps
// its own page, and one Chrome made as the site's own search was used, which it keeps inactive
// until the user turns it on, is turned on, as Arc's are; one the user turned off stays off.
void SeedSiteSearches(Profile* profile) {
  PrefService* prefs = profile->GetPrefs();
  TemplateURLService* engines = LoadedEngines(profile);
  if (!engines || prefs->GetBoolean(kSiteSearchesSeeded)) {
    return;
  }
  prefs->SetBoolean(kSiteSearchesSeeded, true);
  for (const SiteSearch& site : kSiteSearches) {
    const std::u16string keyword = base::UTF8ToUTF16(site.keyword);
    if (TemplateURL* engine = engines->GetTemplateURLForKeyword(keyword)) {
      if (engine->is_active() == TemplateURLData::ActiveStatus::kUnspecified &&
          !engine->CanBeUsedForKeywordMatching()) {
        engines->SetIsActiveTemplateURL(engine, true);
      }
      continue;
    }
    TemplateURLData data;
    data.SetShortName(base::UTF8ToUTF16(site.name));
    data.SetKeyword(keyword);
    data.SetURL(std::string(site.url));
    // As one the user added, which Chrome never replaces with the site's own.
    data.safe_for_autoreplace = false;
    data.is_active = TemplateURLData::ActiveStatus::kTrue;
    engines->Add(std::make_unique<TemplateURL>(data));
  }
}

}  // namespace

void RegisterSearchPrefs() {
  PrefsFactory::GetInstance();
}

void SetUpSearch(Profile* profile) {
  ForgetPickedDefault(profile->GetPrefs());
  TemplateURLService* engines = TemplateURLServiceFactory::GetForProfile(profile);
  if (!engines) {
    return;
  }
  engines->Load();
  // A private profile's engines are its original's, which keeps its own pref.
  if (profile->IsOffTheRecord()) {
    return;
  }
  if (engines->loaded()) {
    SeedSiteSearches(profile);
    return;
  }
  auto wait = std::make_unique<SeedOnLoad>();
  wait->loaded =
      engines->RegisterOnLoadedCallback(base::BindOnce(&SeedSiteSearches, base::Unretained(profile)));
  profile->SetUserData(kSeedOnLoad, std::move(wait));
}

GURL TypedUrl(Profile* profile, std::string_view text) {
  AutocompleteMatch match;
  // As Chrome's paste and go does: the address bar's choice without completing what was typed.
  AutocompleteClassifierFactory::GetForProfile(profile)->Classify(
      base::UTF8ToUTF16(text), /*in_keyword_mode=*/false, /*allow_exact_keyword_match=*/false,
      metrics::OmniboxEventProto::OTHER, &match, /*alternate_nav_url=*/nullptr);
  if (AutocompleteMatch::IsSearchType(match.type)) {
    if (GURL url = TypedWithScheme(text); url.is_valid()) {
      return url;
    }
  }
  return match.destination_url;
}

GURL TypedWithScheme(std::string_view text) {
  const GURL url(base::TrimWhitespaceASCII(text, base::TRIM_ALL));
  return url.is_valid() && url.SchemeIsHTTPOrHTTPS() ? url : GURL();
}

GURL SearchUrl(Profile* profile, const std::u16string& keyword, std::string_view text) {
  TemplateURLService* engines = TemplateURLServiceFactory::GetForProfile(profile);
  const TemplateURL* engine = !engines         ? nullptr
                              : keyword.empty() ? engines->GetDefaultSearchProvider()
                                                : engines->GetTemplateURLForKeyword(keyword);
  if (!engine) {
    return GURL();
  }
  return GURL(engine->url_ref().ReplaceSearchTerms(
      TemplateURLRef::SearchTermsArgs(base::UTF8ToUTF16(text)), engines->search_terms_data()));
}

std::string DefaultEngineChoices(Profile* profile) {
  TemplateURLService* engines = LoadedEngines(profile);
  std::string choices;
  if (!engines) {
    return choices;
  }
  for (const TemplateURL* engine : engines->GetTemplateURLs()) {
    if (IsChoice(engines, engine)) {
      choices += base::UTF16ToUTF8(engine->keyword()) + '\t' +
                 base::UTF16ToUTF8(engine->short_name()) + '\n';
    }
  }
  return choices;
}

std::u16string DefaultEngine(Profile* profile) {
  TemplateURLService* engines = LoadedEngines(profile);
  const TemplateURL* engine = engines ? engines->GetDefaultSearchProvider() : nullptr;
  return engine ? engine->keyword() : std::u16string();
}

bool SetDefaultEngine(Profile* profile, const std::u16string& keyword) {
  TemplateURLService* engines = LoadedEngines(profile);
  TemplateURL* engine = engines ? engines->GetTemplateURLForKeyword(keyword) : nullptr;
  if (!engine || !IsChoice(engines, engine)) {
    return false;
  }
  engines->SetUserSelectedDefaultSearchProvider(engine);
  return true;
}

}  // namespace browser_core
