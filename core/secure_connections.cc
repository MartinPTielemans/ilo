#include "browser_core/secure_connections.h"

#include <string>

#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/agent_space_page.h"
#include "browser_core/host.h"
#include "browser_core/secure_connections_rule.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/ssl/ask_before_http_dialog_controller.h"
#include "chrome/browser/ssl/https_first_mode_settings_tracker.h"
#include "chrome/common/pref_names.h"
#include "components/prefs/pref_service.h"
#include "components/tabs/public/tab_interface.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/web_contents.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

using secure_connections_rule::Mode;

Mode ModeOf(HttpsFirstModeSetting setting) {
  switch (setting) {
    case HttpsFirstModeSetting::kDisabled:
      return Mode::kOff;
    case HttpsFirstModeSetting::kEnabledBalanced:
      return Mode::kBalanced;
    case HttpsFirstModeSetting::kEnabledFull:
      return Mode::kStrict;
  }
}

HttpsFirstModeSetting SettingOf(Mode mode) {
  switch (mode) {
    case Mode::kOff:
      return HttpsFirstModeSetting::kDisabled;
    case Mode::kBalanced:
      return HttpsFirstModeSetting::kEnabledBalanced;
    case Mode::kStrict:
      return HttpsFirstModeSetting::kEnabledFull;
  }
}

void Enter(std::string agent, bc_id tab, std::string host, const char* action) {
  if (!Host().agent_activity) {
    return;
  }
  const bc_agent_activity entry = {
      .agent = agent.c_str(),
      .tab = tab,
      .action = action,
      .refused = secure_connections_rule::kEntryReason,
      .site = host.c_str(),
  };
  Host().agent_activity(Host().context, &entry);
}

// The browser's own step, not the agent's: back to the page the tab showed, which of a tab the
// user handed over is the page they handed it at, or closed. The entry goes first, so the app
// ends the step that waits on the page before the tab's own end answers it.
void PutBack(bc_id id, std::string agent, std::string host, const char* action) {
  Enter(std::move(agent), id, std::move(host), action);
  if (content::WebContents* contents = Window::PageOf(id)) {
    content::NavigationController& history = contents->GetController();
    switch (secure_connections_rule::PutBackTo(history.CanGoBack())) {
      case secure_connections_rule::PutBack::kBack:
        history.GoBack();
        break;
      case secure_connections_rule::PutBack::kClosed:
        Window::CloseTab(id);
        break;
    }
  }
}

// TakesHttpWarning, with the |action| its entry has: one of the rule's two, which outlive the
// posted call.
bool Takes(content::WebContents* contents, const GURL& url, const char* action) {
  // A page inside another's has no warning of its own: only a tab's page does.
  const Tab* tab = contents ? Tab::FromWebContents(contents) : nullptr;
  if (!tab || !secure_connections_rule::NoWarning(RuleTabOf(contents))) {
    return false;
  }
  const Space* space = AgentSpaceOf(tab->id());
  LOG(ERROR) << "browser: tab " << tab->id() << " not secure, not opened: an agent's tab";
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&PutBack, tab->id(), space->agent()->id, std::string(url.host()),
                                action));
  return true;
}

}  // namespace

bool SecureConnections(Profile* profile, bool* on) {
  HttpsFirstModeService* service =
      profile ? HttpsFirstModeServiceFactory::GetForProfile(profile) : nullptr;
  if (!service || !on) {
    return false;
  }
  *on = secure_connections_rule::IsOn(ModeOf(service->GetCurrentSetting()));
  return true;
}

bool SetSecureConnections(Profile* profile, bool on) {
  HttpsFirstModeService* service =
      profile ? HttpsFirstModeServiceFactory::GetForProfile(profile) : nullptr;
  if (!service) {
    return false;
  }
  PrefService* prefs = profile->GetPrefs();
  for (const char* pref : {prefs::kHttpsOnlyModeEnabled, prefs::kHttpsFirstBalancedMode,
                           prefs::kHttpsFirstModeIncognito}) {
    if (!prefs->IsUserModifiablePreference(pref)) {
      return false;
    }
  }
  // Written only where the answer changes: a profile that never chose keeps Chrome's own default
  // until the user chooses otherwise, and the stricter choice isn't lost to a row that read on.
  const Mode now = ModeOf(service->GetCurrentSetting());
  const Mode after = secure_connections_rule::After(now, on);
  if (after != now && !service->UpdatePrefs(SettingOf(after))) {
    return false;
  }
  // A private window warns by a pref of its own, which follows the row.
  if (prefs->GetBoolean(prefs::kHttpsFirstModeIncognito) != on) {
    prefs->SetBoolean(prefs::kHttpsFirstModeIncognito, on);
  }
  return true;
}

bool TakesHttpWarning(content::WebContents* contents, const GURL& url) {
  return Takes(contents, url, secure_connections_rule::kEntryAction);
}

namespace {

// Chrome's warning in |tab| if it is up.
AskBeforeHttpDialogController* WarningIn(bc_id tab) {
  content::WebContents* contents = Window::PageOf(tab);
  tabs::TabInterface* page =
      contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  AskBeforeHttpDialogController* warning =
      page ? AskBeforeHttpDialogController::From(page) : nullptr;
  return warning && warning->HasOpenDialog() ? warning : nullptr;
}

}  // namespace

bool HasHttpWarning(bc_id tab) {
  return WarningIn(tab) != nullptr;
}

void LeaveNoHttpWarning(bc_id tab) {
  AskBeforeHttpDialogController* warning = WarningIn(tab);
  if (!warning) {
    return;
  }
  content::WebContents* contents = Window::PageOf(tab);
  if (Takes(contents, contents->GetLastCommittedURL(),
            secure_connections_rule::kHandedBackAction)) {
    warning->CloseDialog();
  }
}

}  // namespace browser_core
