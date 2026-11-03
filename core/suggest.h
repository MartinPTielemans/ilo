#ifndef BROWSER_CORE_SUGGEST_H_
#define BROWSER_CORE_SUGGEST_H_

#include <map>
#include <memory>
#include <string>

#include "base/memory/raw_ptr.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "base/scoped_observation.h"
#include "base/task/cancelable_task_tracker.h"
#include "browser_core/public/browser_core.h"
#include "chrome/browser/profiles/profile_observer.h"
#include "components/omnibox/browser/autocomplete_controller.h"
#include "url/gurl.h"

class ChromeAutocompleteSchemeClassifier;
class Profile;

namespace browser_core {

// A window's command bar suggestions: what Chrome's address bar lists for the text typed, from the
// profile's history, bookmarks and search engines. Nothing is asked of the search engine, since
// ungoogled-chromium turns search suggestions off, so what's typed stays on the device. Built on
// the profile, so the window lets it go before it lets the profile go (Window::LetProfileGo).
class Suggestions : public AutocompleteController::Observer, public ProfileObserver {
 public:
  Suggestions(bc_id window, Profile* profile);
  Suggestions(const Suggestions&) = delete;
  Suggestions& operator=(const Suggestions&) = delete;
  ~Suggestions() override;

  Profile* profile() const { return profile_; }

  // Lists suggestions for |text|, and again as more are found. Empty text lists none. |complete|
  // lets the first complete |text| inline. Unless |keyword| is empty, they're the searches of the
  // engine with that keyword, as in Chrome's keyword mode.
  void Start(const std::u16string& keyword, const std::u16string& text, bool complete);

  // Takes the removable suggestion going to |url| out of the profile's history, and lists the
  // others again.
  void Remove(const GURL& url);

  // Puts back what Remove took out for |url| as it was, and lists the suggestions again. Start on no
  // text forgets what it took out.
  void Restore(const GURL& url);

  // What Remove took out of the profile's history for a suggestion. Defined where it's used.
  struct Removed;

 private:
  // AutocompleteController::Observer:
  void OnResultChanged(AutocompleteController* controller, bool default_match_changed) override;

  // ProfileObserver: Chrome tells this before it destroys the profile, even when it then waits for
  // the profile's pages, so a window that let it go too late fails here in every build.
  void OnProfileWillBeDestroyed(Profile* profile) override;

  // Tells the app from a task of its own, so a burst of changes is one call, and none comes during
  // the app's call that made them.
  void Changed();
  void Report();

  const bc_id window_;
  const raw_ptr<Profile> profile_;
  std::unique_ptr<ChromeAutocompleteSchemeClassifier> classifier_;
  AutocompleteController controller_;
  std::u16string keyword_;
  std::u16string text_;
  bool report_posted_ = false;
  // What Remove took out, by the suggestion's URL, filled on the history database's thread.
  std::map<GURL, scoped_refptr<Removed>> removed_;
  base::CancelableTaskTracker history_tasks_;
  base::ScopedObservation<AutocompleteController, AutocompleteController::Observer> observation_{
      this};
  base::ScopedObservation<Profile, ProfileObserver> profile_observation_{this};
  base::WeakPtrFactory<Suggestions> weak_factory_{this};
};

}  // namespace browser_core

#endif  // BROWSER_CORE_SUGGEST_H_
