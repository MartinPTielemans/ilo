#ifndef BROWSER_CORE_SUGGEST_H_
#define BROWSER_CORE_SUGGEST_H_

#include <cstdint>
#include <map>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/callback_forward.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "base/scoped_observation.h"
#include "base/task/cancelable_task_tracker.h"
#include "base/time/time.h"
#include "browser_core/history_removal.h"
#include "browser_core/public/browser_core.h"
#include "chrome/browser/profiles/profile_observer.h"
#include "components/omnibox/browser/autocomplete_controller.h"
#include "components/search_engines/template_url_id.h"
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
  // others again. Returns the removal's id, for Restore, or 0 if there was none to take out, as
  // while the last removal of |url| is still being taken out.
  bc_id Remove(const GURL& url);

  // Puts back what removal |removal| took out, as it was, and lists the suggestions again, once
  // it's out if it's still being taken out. Whether it could: it's one of the last kKept removals,
  // and not back already.
  bool Restore(bc_id removal);

  // Takes out again what Restore put back for |removal|, as the removal did. Whether it could.
  bool RemoveAgain(bc_id removal);

  // Deletes the profile's visits to |url| from |begin| to |end| at once, as the Library's History
  // removes a day's row, and lists the suggestions again. Returns the removal's id, for Restore, as
  // Remove's, or 0 if it can't (bc_window_remove_history).
  bc_id RemoveVisits(const GURL& url, base::Time begin, base::Time end);

 private:
  // A removal: the suggestion's URL, what Chrome's deletion took out, the pages and a search's
  // keyword and term, and the span of their visits it took, or all of them while |end| is null,
  // that as it was, and where it is.
  struct Removal {
    // kTakingThenBack is a removal put back while it was still being taken out.
    enum class State { kTaking, kTakingThenBack, kOut, kBack };

    Removal();
    Removal(Removal&&);
    Removal& operator=(Removal&&);
    ~Removal();

    GURL url;
    std::vector<GURL> pages;
    std::vector<std::pair<TemplateURLID, std::u16string>> searches;
    base::Time begin;
    base::Time end;
    scoped_refptr<RemovedHistory> removed;
    State state = State::kTaking;
  };

  // The removals Restore can put back: the window's last ten, as many as Clear keeps.
  static constexpr size_t kKept = 10;

  // An id for a removal, counted across windows.
  static bc_id NextRemoval();
  // Keeps |removal| as |id|, and lets go of the oldest past kKept.
  void Hold(bc_id id, Removal removal);
  // Removal |id|'s deletion, Chrome's for |match|, once what it takes out has been read.
  void TakeOut(bc_id id, const AutocompleteMatch& match);
  // Lists the suggestions for the text last asked again, run once the history service has done a
  // task, as it has told the address bar's indexes by then.
  base::OnceClosure ListAgain();

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
  // The last kKept removals, by id. Ids are counted across windows, so a window's suggestions made
  // again for another profile never put back one made for the last.
  std::map<bc_id, Removal> removals_;
  base::CancelableTaskTracker history_tasks_;
  base::ScopedObservation<AutocompleteController, AutocompleteController::Observer> observation_{
      this};
  base::ScopedObservation<Profile, ProfileObserver> profile_observation_{this};
  base::WeakPtrFactory<Suggestions> weak_factory_{this};
};

}  // namespace browser_core

#endif  // BROWSER_CORE_SUGGEST_H_
