#include "browser_core/search_default.h"

#include <memory>

#include "base/strings/utf_string_conversions.h"
#include "base/values.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "components/search_engines/default_search_manager.h"
#include "components/search_engines/search_engines_pref_names.h"
#include "components/search_engines/template_url.h"
#include "components/search_engines/template_url_data.h"
#include "components/search_engines/template_url_data_util.h"
#include "components/search_engines/template_url_service.h"
#include "third_party/search_engines_data/resources/definitions/prepopulated_engines.h"

namespace browser_core {

namespace {

constexpr char kPickForgotten[] = "browser_core.search_pick_forgotten";
constexpr char kGoogleAdded[] = "browser_core.search_google_added";

constexpr char16_t kGoogleKeyword[] = u"google.com";
constexpr char kGoogleUrl[] = "https://www.google.com/search?q={searchTerms}";

}  // namespace

void RegisterSearchDefaultPrefs(user_prefs::PrefRegistrySyncable* registry) {
  registry->RegisterBooleanPref(kPickForgotten, false);
  registry->RegisterBooleanPref(kGoogleAdded, false);
}

void ForgetPickedDefault(PrefService* prefs) {
  if (prefs->GetBoolean(kPickForgotten)) {
    return;
  }
  prefs->SetBoolean(kPickForgotten, true);
  const base::Value* picked =
      prefs->GetUserPrefValue(DefaultSearchManager::kDefaultSearchProviderDataPrefName);
  std::unique_ptr<TemplateURLData> engine =
      picked && picked->is_dict() ? TemplateURLDataFromDictionary(picked->GetDict()) : nullptr;
  if (!engine || engine->prepopulate_id != TemplateURLPrepopulateData::duckduckgo.id) {
    return;
  }
  // Chrome reads a cleared choice as one it reset, and would tell the user so.
  const bool reset = prefs->GetBoolean(prefs::kUnacknowledgedDefaultSearchEngineResetOccurred);
  prefs->ClearPref(DefaultSearchManager::kDefaultSearchProviderDataPrefName);
  prefs->SetBoolean(prefs::kUnacknowledgedDefaultSearchEngineResetOccurred, reset);
}

TemplateURLData GoogleChoice() {
  TemplateURLData data;
  data.SetShortName(u"Google");
  data.SetKeyword(kGoogleKeyword);
  data.SetURL(kGoogleUrl);
  // As one the user added, which Chrome never replaces with the site's own.
  data.safe_for_autoreplace = false;
  data.is_active = TemplateURLData::ActiveStatus::kTrue;
  return data;
}

bool IsGoogleChoice(const TemplateURL& engine) {
  return engine.keyword() == kGoogleKeyword && engine.url() == kGoogleUrl &&
         engine.type() == TemplateURL::NORMAL && engine.prepopulate_id() == 0;
}

void AddGoogleChoice(PrefService* prefs, TemplateURLService* engines) {
  if (prefs->GetBoolean(kGoogleAdded)) {
    return;
  }
  prefs->SetBoolean(kGoogleAdded, true);
  const TemplateURL* existing = engines->GetTemplateURLForKeyword(kGoogleKeyword);
  if (existing && !existing->safe_for_autoreplace()) {
    return;
  }
  engines->Add(std::make_unique<TemplateURL>(GoogleChoice()));
}

}  // namespace browser_core
