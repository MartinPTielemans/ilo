#ifndef BROWSER_CORE_SYSTEM_ENGINE_CHOICES_H_
#define BROWSER_CORE_SYSTEM_ENGINE_CHOICES_H_

#include <string>
#include <string_view>
#include <vector>

#include "base/memory/raw_ref.h"

class GURL;
class PrefRegistrySimple;
class PrefService;

namespace browser_core::system_engine {

// The sites a profile keeps for the system's engine.
inline constexpr char kSitesPref[] = "browser_core.system_engine.sites";
// The sites whose protected-media note the user closed.
inline constexpr char kClosedNotesPref[] = "browser_core.system_engine.closed_notes";
// The listed services the user turned off, by their entries in ListedServices().
inline constexpr char kListedOffPref[] = "browser_core.system_engine.listed_off";

void RegisterChoicePrefs(PrefRegistrySimple* registry);

// The streaming services whose main content is protected video or audio, which open in the system's
// engine from the first visit, in order. Each matches its host and subdomains.
std::vector<std::string_view> ListedServices();

// The site of |url| as a profile keeps it: its registrable domain, else its host.
std::string SiteOf(const GURL& url);

// What a profile chose about sites for the system's engine, over its prefs alone, apart from
// system_engine.cc so browser_core_unittests tests it without a profile or an engine. Its rules are
// those system_engine.h gives for the calls of the same names, less the engine's own checks.
class Choices {
 public:
  // |test_host|, for the app's check only, is listed as the services are.
  explicit Choices(PrefService& prefs, std::string_view test_host = {});

  // |url|'s entry among the listed services, or |test_host|, else empty.
  std::string_view ListedEntry(const GURL& url) const;

  bool Remembered(const GURL& url) const;
  void Remember(const GURL& url);
  void Forget(const GURL& url);
  bool NoteClosed(const GURL& url) const;
  void CloseNote(const GURL& url);
  bool KeptInChromium(const GURL& url) const;
  std::vector<std::string> Sites() const;
  std::vector<std::string> KeptSites() const;
  // Whether Settings' list gave |site|, so UseSite may change it.
  bool Offered(std::string_view site) const;
  void ForgetChoices(const GURL& url);
  // Every site back to the listed default, as Clear Browsing Data does.
  void Clear();

 private:
  bool Holds(const char* pref, std::string_view value) const;
  std::vector<std::string> Strings(const char* pref) const;

  const raw_ref<PrefService> prefs_;
  const std::string_view test_host_;
};

}  // namespace browser_core::system_engine

#endif  // BROWSER_CORE_SYSTEM_ENGINE_CHOICES_H_
