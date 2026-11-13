#ifndef BROWSER_CORE_EXTENSIONS_H_
#define BROWSER_CORE_EXTENSIONS_H_

#include <map>
#include <memory>
#include <string>
#include <vector>

#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/scoped_observation.h"
#include "browser_core/public/browser_core.h"
#include "chrome/browser/ui/toolbar/toolbar_actions_model.h"
#include "ui/gfx/image/image.h"

class BrowserWindowInterface;
class Profile;

namespace content {
class WebContents;
}

namespace views {
class View;
}

namespace browser_core {

// The extension actions a window shows: those pinned in the profile of its tab shown, as they are
// for that tab, and the other turned-on extensions' while its Site Controls show. Tells the app
// with window_extension_actions whenever they differ from what it last told, so a tab switch that
// changes nothing costs nothing.
class WindowActions : public ToolbarActionsModel::Observer {
 public:
  explicit WindowActions(bc_id window);
  ~WindowActions() override;

  // Shows the actions for |contents|, or none.
  void Show(content::WebContents* contents);

  // Shows every turned-on extension's action, the unpinned ones after the pinned, by name, or the
  // pinned alone, as until this is called.
  void ShowEvery(bool every);

 private:
  class Icon;
  // What the app was told of one action. The icon is compared by its pixels' storage, which
  // Chrome keeps while the icon doesn't change.
  struct Told {
    std::string id;
    std::string title;
    gfx::Image icon;
    std::string badge;
    uint32_t badge_color;
    bool enabled;
    bool pinned;
  };

  // ToolbarActionsModel::Observer:
  void OnToolbarActionAdded(const ToolbarActionsModel::ActionId& id) override;
  void OnToolbarActionRemoved(const ToolbarActionsModel::ActionId& id) override;
  void OnToolbarActionUpdated(const ToolbarActionsModel::ActionId& id) override;
  void OnToolbarModelInitialized() override;
  void OnToolbarPinnedActionsChanged() override;
  void OnToolbarActionsModelShutdown() override;

  void Changed();
  void Report();

  const bc_id window_;
  base::WeakPtr<content::WebContents> contents_;
  raw_ptr<Profile> profile_ = nullptr;
  base::ScopedObservation<ToolbarActionsModel, ToolbarActionsModel::Observer> observation_{this};
  std::map<std::string, std::unique_ptr<Icon>> icons_;
  std::vector<Told> told_;
  bool every_ = false;
  bool report_posted_ = false;
  base::WeakPtrFactory<WindowActions> weak_factory_{this};
};

// Tells the app of |profile|'s extensions from now on, those Chrome's extensions page lists. Then
// extensions_changed.
void ObserveExtensions(Profile* profile);

// Then extensions_changed again, for each profile observed.
void ReportExtensions();

// Pins extension |id|'s action in |profile|, or unpins it.
void SetExtensionPinned(Profile* profile, const std::string& id, bool pinned);

// The user was told the browser turned extension |id| off in |profile| as it crashed.
void ExtensionCrashTold(Profile* profile, const std::string& id);

// Turns extension |id| on again in |profile|, as Chrome's extensions page does. Returns whether
// it's on.
bool TurnOnExtension(Profile* profile, const std::string& id);

// After two crashes at launch in a row, the app starts the browser safe (docs/ux-spec.md › Safe
// start): |profile|'s extensions, the ad blocker too, are held for this launch, before any of them
// runs, and its next launch loads them as they were.
void HoldExtensionsForSafeStart(Profile* profile);

// Removes extension |id| from |profile|, as the user does. Returns false if it can't be.
bool RemoveExtension(Profile* profile, const std::string& id);

// Runs extension |id|'s action on |contents|, the tab |window| shows in |browser|, as a click on its
// toolbar button does in Chrome: it's granted the tab, and its popup opens from |anchor| or its
// action.onClicked fires. Returns false if the extension isn't turned on in the tab's profile.
bool RunExtension(bc_id window,
                  BrowserWindowInterface* browser,
                  content::WebContents* contents,
                  const std::string& id,
                  views::View* anchor);

// Runs extension |id|'s keyboard command |name| on |contents|, the tab a window shows, as its key
// does in Chrome: it's granted the tab, and its commands.onCommand hears the name and the tab.
// Returns false if the extension isn't turned on in the tab's profile or isn't listening.
bool RunExtensionCommand(content::WebContents* contents,
                         const std::string& id,
                         const std::string& name);

}  // namespace browser_core

#endif  // BROWSER_CORE_EXTENSIONS_H_
