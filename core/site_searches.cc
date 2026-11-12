#include "browser_core/site_searches.h"

#include <algorithm>
#include <memory>
#include <string_view>

#include "base/i18n/case_conversion.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "components/search_engines/template_url.h"
#include "components/search_engines/template_url_data.h"
#include "components/search_engines/template_url_service.h"
#include "components/search_engines/template_url_starter_pack_data.h"

namespace browser_core {

namespace {

constexpr char kSeeded[] = "browser_core.site_searches_seeded";

struct SiteSearch {
  std::string_view name;
  // The site's host, as Chrome names a site's search, which Tab is offered for.
  std::string_view keyword;
  std::string_view url;
};

// Arc's, which it seeds active into Chromium's keywords table. X has twitter.com's too, so a search
// typed by its old name finds it. The list is the owner's to trim.
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

}  // namespace

void RegisterSiteSearchesPrefs(user_prefs::PrefRegistrySyncable* registry) {
  registry->RegisterBooleanPref(kSeeded, false);
}

void SeedSiteSearches(PrefService* prefs, TemplateURLService* engines) {
  if (prefs->GetBoolean(kSeeded)) {
    return;
  }
  prefs->SetBoolean(kSeeded, true);
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

const TemplateURL* SiteStarted(TemplateURLService* engines, const std::u16string& text) {
  std::u16string prefix = base::i18n::ToLower(text);
  base::TrimWhitespace(prefix, base::TRIM_ALL, &prefix);
  if (prefix.size() < 2) {
    return nullptr;
  }
  TemplateURLService::TemplateURLVector started;
  engines->AddMatchingKeywords(prefix, /*supports_replacement_only=*/true, &started);
  // Chrome's @ keywords search its own pages, and the engines it adds by itself stay inactive.
  auto engine = std::ranges::find_if(started, [](const TemplateURL* engine) {
    return engine->type() == TemplateURL::NORMAL && engine->CanBeUsedForKeywordMatching() &&
           engine->starter_pack_id() == template_url_starter_pack_data::StarterPackId::kNone &&
           !engine->featured_by_policy();
  });
  return engine == started.end() ? nullptr : *engine;
}

}  // namespace browser_core
