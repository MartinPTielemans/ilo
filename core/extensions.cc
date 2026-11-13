#include "browser_core/extensions.h"

#include <algorithm>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <vector>

#include "base/command_line.h"
#include "base/functional/bind.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/process/kill.h"
#include "base/scoped_multi_source_observation.h"
#include "base/scoped_observation.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/values.h"
#include "browser_core/ad_block.h"
#include "browser_core/host.h"
#include "browser_core/profiles.h"
#include "chrome/browser/extensions/commands/command_service.h"
#include "chrome/browser/extensions/extension_action_runner.h"
#include "chrome/browser/extensions/extension_tab_util.h"
#include "chrome/browser/extensions/extension_view_host.h"
#include "chrome/browser/extensions/extension_view_host_factory.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/ui/extensions/extension_popup_types.h"
#include "chrome/browser/ui/views/extensions/extension_popup.h"
#include "chrome/common/webui_url_constants.h"
#include "components/sessions/content/session_tab_helper.h"
#include "content/public/browser/child_process_termination_info.h"
#include "content/public/browser/render_process_host.h"
#include "content/public/browser/render_process_host_creation_observer.h"
#include "content/public/browser/render_process_host_observer.h"
#include "content/public/browser/web_contents.h"
#include "extensions/browser/disable_reason.h"
#include "extensions/browser/event_router.h"
#include "extensions/browser/extension_action.h"
#include "extensions/browser/extension_action_icon_factory.h"
#include "extensions/browser/extension_action_manager.h"
#include "extensions/browser/extension_host.h"
#include "extensions/browser/extension_host_observer.h"
#include "extensions/browser/extension_prefs.h"
#include "extensions/browser/extension_registrar.h"
#include "extensions/browser/extension_registry.h"
#include "extensions/browser/extension_registry_observer.h"
#include "extensions/browser/extension_system.h"
#include "extensions/browser/extension_util.h"
#include "extensions/browser/management_policy.h"
#include "extensions/browser/permissions/active_tab_permission_granter.h"
#include "extensions/browser/process_map.h"
#include "extensions/browser/ui_util.h"
#include "extensions/browser/uninstall_reason.h"
#include "extensions/common/api/commands/commands_handler.h"
#include "extensions/common/api/extension_action/action_info.h"
#include "extensions/common/command.h"
#include "extensions/common/extension.h"
#include "extensions/common/manifest.h"
#include "extensions/common/manifest_handlers/options_page_info.h"
#include "extensions/common/mojom/context_type.mojom.h"
#include "ui/gfx/codec/png_codec.h"
#include "ui/gfx/image/image_skia.h"
#include "ui/gfx/image/image_skia_rep.h"
#include "ui/views/bubble/bubble_anchor.h"
#include "ui/events/keycodes/keyboard_codes.h"
#include "ui/views/bubble/bubble_border.h"

namespace browser_core {

namespace {

constexpr char kOnCommand[] = "commands.onCommand";

// In an extension's entry of the profile's extension prefs: how many times its process has crashed
// since it was last turned on, and that the browser turned it off for that and the user hasn't been
// told.
constexpr char kCrashesPref[] = "browser_core_crashes";
constexpr char kCrashedOffPref[] = "browser_core_crashed_off";

// The crashes that turn an extension off, as Arc turned off one that crashed it.
constexpr int kCrashesToTurnOff = 2;

// Whether the browser turned extension |id| off in |profile| as it crashed, and the user hasn't
// been told.
bool CrashedOff(Profile* profile, const std::string& id) {
  bool crashed = false;
  extensions::ExtensionPrefs::Get(profile)->ReadPrefAsBoolean(id, kCrashedOffPref, &crashed);
  return crashed;
}

// Counts the crashes of each extension's own process, its background page or service worker, its
// popup and its pages in tabs, and turns it off after the second, across launches. Chromium can't
// tell which extension, if any, crashed the browser itself: its crash reports name no extension,
// and an extension's own code never runs in the browser's process. So a crash of its own process
// is the closest honest sign, as it's the extension's code that crashed there.
class CrashWatch : public content::RenderProcessHostCreationObserver,
                   public content::RenderProcessHostObserver {
 public:
  CrashWatch() {
    for (auto hosts = content::RenderProcessHost::AllHostsIterator(); !hosts.IsAtEnd();
         hosts.Advance()) {
      OnRenderProcessHostCreated(hosts.GetCurrentValue());
    }
  }

 private:
  // content::RenderProcessHostCreationObserver, which hears a host again for each new process:
  void OnRenderProcessHostCreated(content::RenderProcessHost* host) override {
    if (!observations_.IsObservingSource(host)) {
      observations_.AddObservation(host);
    }
  }

  // content::RenderProcessHostObserver:
  void RenderProcessReady(content::RenderProcessHost* host) override {
    // Kept, as the process map may forget the process before its exit is heard.
    if (std::optional<extensions::ExtensionId> id = ExtensionIn(host)) {
      extensions_[host] = *id;
    }
  }
  void RenderProcessExited(content::RenderProcessHost* host,
                           const content::ChildProcessTerminationInfo& info) override {
    std::optional<extensions::ExtensionId> id = ExtensionIn(host);
    if (auto kept = extensions_.find(host); kept != extensions_.end()) {
      id = id.value_or(kept->second);
      extensions_.erase(kept);
    }
    // A process the browser or the system ended isn't the extension's crash.
    if (id && (info.status == base::TERMINATION_STATUS_PROCESS_CRASHED ||
               info.status == base::TERMINATION_STATUS_OOM)) {
      // Posted: turning it off tears down its hosts, which the process's observers may be.
      Profile* profile = Profile::FromBrowserContext(host->GetBrowserContext());
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(&CrashWatch::Crashed,
                                    profile->GetOriginalProfile()->GetWeakPtr(), *id));
    }
  }
  void RenderProcessHostDestroyed(content::RenderProcessHost* host) override {
    extensions_.erase(host);
    observations_.RemoveObservation(host);
  }

  static std::optional<extensions::ExtensionId> ExtensionIn(content::RenderProcessHost* host) {
    return extensions::ProcessMap::Get(host->GetBrowserContext())
        ->GetExtensionIdForProcess(host->GetID());
  }

  static void Crashed(base::WeakPtr<Profile> profile, const extensions::ExtensionId& id) {
    if (!profile) {
      return;
    }
    const extensions::Extension* extension =
        extensions::ExtensionRegistry::Get(profile.get())->enabled_extensions().GetByID(id);
    // The ad blocker the browser comes with is the browser's, and Settings turns it off.
    if (!extension || !extensions::ui_util::ShouldDisplayInExtensionSettings(*extension) ||
        IsAdBlocker(id)) {
      return;
    }
    extensions::ExtensionPrefs* prefs = extensions::ExtensionPrefs::Get(profile.get());
    int crashes = 0;
    prefs->ReadPrefAsInteger(id, kCrashesPref, &crashes);
    ++crashes;
    LOG(ERROR) << "browser: extension " << id << " crashed, " << crashes << " times";
    if (crashes < kCrashesToTurnOff) {
      prefs->UpdateExtensionPref(id, kCrashesPref, base::Value(crashes));
      return;
    }
    prefs->UpdateExtensionPref(id, kCrashesPref, std::nullopt);
    extensions::ExtensionRegistrar::Get(profile.get())->DisableExtension(
        id, {extensions::disable_reason::DISABLE_USER_ACTION});
    // Policy may keep it on.
    if (!extensions::ExtensionRegistry::Get(profile.get())->enabled_extensions().Contains(id)) {
      LOG(ERROR) << "browser: extension " << id << " turned off for crashing";
      prefs->UpdateExtensionPref(id, kCrashedOffPref, base::Value(true));
    }
  }

  std::map<content::RenderProcessHost*, extensions::ExtensionId> extensions_;
  base::ScopedMultiSourceObservation<content::RenderProcessHost, content::RenderProcessHostObserver>
      observations_{this};
};

// Extension |id| if it's turned on in |profile| and runs there: in a private window's profile only
// if the user allowed it in Incognito, as in Chrome, whose off-the-record registry is the
// original's and lists every extension. Never the ad blocker, whose popup and commands are uBlock
// Origin's own UI, which Settings › Ad blocking stands for.
const extensions::Extension* RunsIn(Profile* profile, const std::string& id) {
  if (IsAdBlocker(id)) {
    return nullptr;
  }
  const extensions::Extension* extension =
      extensions::ExtensionRegistry::Get(profile)->enabled_extensions().GetByID(id);
  if (extension && profile->IsOffTheRecord() &&
      !extensions::util::IsIncognitoEnabled(extension, profile)) {
    return nullptr;
  }
  return extension;
}

// The key a command's |code| is, as bc_key names one, or "" for a media key, which no window's
// keys are.
std::string KeyName(ui::KeyboardCode code) {
  if ((code >= ui::VKEY_A && code <= ui::VKEY_Z) || (code >= ui::VKEY_0 && code <= ui::VKEY_9)) {
    return std::string(1, base::ToLowerASCII(static_cast<char>(code)));
  }
  switch (code) {
    case ui::VKEY_OEM_COMMA:
      return ",";
    case ui::VKEY_OEM_PERIOD:
      return ".";
    case ui::VKEY_SPACE:
      return " ";
    case ui::VKEY_HOME:
      return "Home";
    case ui::VKEY_END:
      return "End";
    case ui::VKEY_PRIOR:
      return "PageUp";
    case ui::VKEY_NEXT:
      return "PageDown";
    case ui::VKEY_INSERT:
      return "Insert";
    case ui::VKEY_DELETE:
      return "Delete";
    case ui::VKEY_UP:
      return "ArrowUp";
    case ui::VKEY_DOWN:
      return "ArrowDown";
    case ui::VKEY_LEFT:
      return "ArrowLeft";
    case ui::VKEY_RIGHT:
      return "ArrowRight";
    default:
      return "";
  }
}

// One of an extension's commands as the app is told of it, owning its strings.
struct KeyedCommand {
  std::string name;
  std::string description;
  std::string key;
  uint32_t modifiers;
  bool suggested;
};

// |extension|'s commands that have a key, its action's first. Global ones too: the window runs
// them while it's in front, as Chrome's do.
std::vector<KeyedCommand> KeyedCommands(Profile* profile, const extensions::Extension& extension) {
  std::vector<KeyedCommand> keyed;
  // |suggested| is the key the manifest gives |command|, if it gives one.
  auto add = [&keyed](const ui::Command& command, const ui::Accelerator* suggested) {
    const ui::Accelerator& accelerator = command.accelerator();
    std::string key = KeyName(accelerator.key_code());
    if (key.empty()) {
      return;
    }
    const uint32_t modifiers = (accelerator.IsShiftDown() ? BC_MODIFIER_SHIFT : 0) |
                               (accelerator.IsCtrlDown() ? BC_MODIFIER_CONTROL : 0) |
                               (accelerator.IsAltDown() ? BC_MODIFIER_ALT : 0) |
                               (accelerator.IsCmdDown() ? BC_MODIFIER_META : 0);
    keyed.push_back({command.command_name(), base::UTF16ToUTF8(command.description()),
                     std::move(key), modifiers, suggested && *suggested == accelerator});
  };
  extensions::CommandService* service = extensions::CommandService::Get(profile);
  const extensions::ActionInfo* action = extensions::ActionInfo::GetExtensionActionInfo(&extension);
  extensions::Command command;
  if (action && service->GetExtensionActionCommand(extension.id(), action->type,
                                                   extensions::CommandService::ACTIVE, &command,
                                                   nullptr)) {
    using Type = extensions::ActionInfo::Type;
    const extensions::Command* manifest =
        action->type == Type::kAction ? extensions::CommandsInfo::GetActionCommand(&extension)
        : action->type == Type::kBrowser
            ? extensions::CommandsInfo::GetBrowserActionCommand(&extension)
            : extensions::CommandsInfo::GetPageActionCommand(&extension);
    add(command, manifest ? &manifest->accelerator() : nullptr);
  }
  ui::CommandMap named;
  service->GetNamedCommands(extension.id(), extensions::CommandService::ACTIVE,
                            extensions::CommandService::ANY_SCOPE, &named);
  const ui::CommandMap* manifest = extensions::CommandsInfo::GetNamedCommands(&extension);
  for (const auto& [name, each] : named) {
    const auto suggested = manifest ? manifest->find(name) : ui::CommandMap::const_iterator();
    add(each, manifest && suggested != manifest->end() ? &suggested->second.accelerator()
                                                       : nullptr);
  }
  return keyed;
}

// Chrome gives an extension the key it suggests only if Chrome's own table of shortcuts doesn't
// hold it, but here the app's keys are the browser's, and the app keeps them over an extension's.
// So a key that table alone refused, such as Linux's Alt+Shift+P for Chrome's tab groups, is the
// extension's, as on the Mac. One the user changed or took away stays as they left it, and one
// another extension holds stays its.
void AssignKeysChromeRefused(Profile* profile, const extensions::Extension& extension) {
  extensions::CommandService* service = extensions::CommandService::Get(profile);
  extensions::ExtensionPrefs* prefs = extensions::ExtensionPrefs::Get(profile);
  auto assign = [&](const ui::Command& command) {
    // Read again for each, as assigning one rewrites them.
    const base::DictValue* suggested = prefs->ReadPrefAsDict(extension.id(), "commands");
    const base::DictValue* pref = suggested ? suggested->FindDict(command.command_name()) : nullptr;
    if (command.global() || command.accelerator().key_code() == ui::VKEY_UNKNOWN ||
        (pref && pref->FindBool("was_assigned").value_or(false)) ||
        service->FindCommandByName(extension.id(), command.command_name())
                .accelerator()
                .key_code() != ui::VKEY_UNKNOWN) {
      return;
    }
    service->AddKeybindingPref(command.accelerator(), extension.id(), command.command_name(),
                               /*allow_overrides=*/false, /*global=*/false);
  };
  if (const ui::CommandMap* named = extensions::CommandsInfo::GetNamedCommands(&extension)) {
    for (const auto& [name, command] : *named) {
      assign(command);
    }
  }
  using extensions::CommandsInfo;
  for (const extensions::Command* action : {CommandsInfo::GetActionCommand(&extension),
                                            CommandsInfo::GetBrowserActionCommand(&extension),
                                            CommandsInfo::GetPageActionCommand(&extension)}) {
    if (action) {
      assign(*action);
    }
  }
}

// The page Chrome's Options item opens for |extension|: its own, or Chrome's extensions page
// showing it, as Chrome embeds one that doesn't ask for a tab. "" if it has none.
std::string OptionsPage(const extensions::Extension& extension) {
  if (!extensions::OptionsPageInfo::HasOptionsPage(&extension)) {
    return "";
  }
  if (extensions::OptionsPageInfo::ShouldOpenInTab(&extension)) {
    return extensions::OptionsPageInfo::GetOptionsPage(&extension).spec();
  }
  return std::string(chrome::kChromeUIExtensionsURL) + "?options=" + extension.id();
}

// Whether a safe start holds |extension| in |profile|: all but the browser's own, a theme and one a
// policy keeps on.
bool Holdable(Profile* profile, const extensions::Extension& extension) {
  return !extensions::Manifest::IsComponentLocation(extension.location()) &&
         !extension.is_theme() &&
         !extensions::ExtensionSystem::Get(profile)->management_policy()->MustRemainEnabled(
             &extension, nullptr);
}

// A safe start's hold on one profile's extensions, for the launch. Each is terminated, as Chrome
// leaves an extension whose process crashed: not loaded, nothing kept in its prefs, and listed on
// Chrome's extensions page with its switch as the user left it and Remove. The profile's saved ones
// are held before any runs. The command line's load a task later, as their installer checks them
// first (#2629), so each is held as it loads, once: the user can still load it again there, and one
// they install or turn on in the launch is theirs.
class SafeStartHold : public extensions::ExtensionRegistryObserver {
 public:
  // |count| are held already.
  SafeStartHold(Profile* profile, size_t count) : profile_(profile), count_(count) {
    observation_.Observe(extensions::ExtensionRegistry::Get(profile));
    Said();
  }

  // Whether it's about to hold |id|, as soon as the registry's notification is done.
  bool Holding(const extensions::ExtensionId& id) const { return holding_.contains(id); }

 private:
  // extensions::ExtensionRegistryObserver:
  void OnExtensionLoaded(content::BrowserContext* context,
                         const extensions::Extension* extension) override {
    if (extension->location() != extensions::mojom::ManifestLocation::kCommandLine ||
        !Holdable(profile_, *extension) || !held_.insert(extension->id()).second) {
      return;
    }
    holding_.insert(extension->id());
    // Not inside the registry's notification.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE,
        base::BindOnce(&SafeStartHold::Hold, weak_factory_.GetWeakPtr(), extension->id()));
  }
  // The profile is going. This deletes this.
  void OnShutdown(extensions::ExtensionRegistry* registry) override;

  void Hold(const extensions::ExtensionId& id) {
    holding_.erase(id);
    if (!extensions::ExtensionRegistry::Get(profile_)->enabled_extensions().Contains(id)) {
      return;
    }
    extensions::ExtensionRegistrar::Get(profile_)->TerminateExtension(id);
    ++count_;
    Said();
  }

  // check.sh reads this line, the launch's count so far.
  void Said() { LOG(ERROR) << "browser: safe start holds " << count_ << " extensions"; }

  const raw_ptr<Profile> profile_;
  // The command line's it has held, and those of them it's about to.
  std::set<extensions::ExtensionId> held_;
  std::set<extensions::ExtensionId> holding_;
  size_t count_;
  base::ScopedObservation<extensions::ExtensionRegistry, extensions::ExtensionRegistryObserver>
      observation_{this};
  base::WeakPtrFactory<SafeStartHold> weak_factory_{this};
};

std::map<Profile*, std::unique_ptr<SafeStartHold>>& SafeStartHolds() {
  static base::NoDestructor<std::map<Profile*, std::unique_ptr<SafeStartHold>>> holds;
  return *holds;
}

void SafeStartHold::OnShutdown(extensions::ExtensionRegistry* registry) {
  SafeStartHolds().erase(profile_);
}

// Whether a safe start is about to hold extension |id| in |profile|, one that loaded late.
bool HoldingLate(Profile* profile, const extensions::ExtensionId& id) {
  auto hold = SafeStartHolds().find(profile);
  return hold != SafeStartHolds().end() && hold->second->Holding(id);
}

// Tells the app of one profile's extensions, once after each call that changed them.
class ProfileExtensions : public extensions::ExtensionRegistryObserver,
                          public ToolbarActionsModel::Observer,
                          public extensions::CommandService::Observer {
 public:
  explicit ProfileExtensions(Profile* profile) : profile_(profile) {
    observation_.Observe(extensions::ExtensionRegistry::Get(profile));
    pinned_observation_.Observe(ToolbarActionsModel::Get(profile));
    commands_observation_.Observe(extensions::CommandService::Get(profile));
    Changed();
  }

  void Changed() {
    if (report_posted_) {
      return;
    }
    report_posted_ = true;
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&ProfileExtensions::Report, weak_factory_.GetWeakPtr()));
  }

 private:
  // extensions::ExtensionRegistryObserver:
  void OnExtensionLoaded(content::BrowserContext* context,
                         const extensions::Extension* extension) override {
    AssignKeysChromeRefused(profile_, *extension);
    // Turned on again, as on Chrome's extensions page, it needs no telling.
    if (CrashedOff(profile_, extension->id())) {
      extensions::ExtensionPrefs::Get(profile_)->UpdateExtensionPref(
          extension->id(), kCrashedOffPref, std::nullopt);
    }
    Changed();
  }
  void OnExtensionUnloaded(content::BrowserContext* context,
                           const extensions::Extension* extension,
                           extensions::UnloadedExtensionReason reason) override {
    Changed();
  }
  void OnExtensionInstalled(content::BrowserContext* context,
                            const extensions::Extension* extension,
                            bool is_update) override {
    Changed();
  }
  void OnExtensionUninstalled(content::BrowserContext* context,
                              const extensions::Extension* extension,
                              extensions::UninstallReason reason) override {
    Changed();
  }
  // The profile is going, and its extensions with it.
  void OnShutdown(extensions::ExtensionRegistry* registry) override { observation_.Reset(); }

  // ToolbarActionsModel::Observer, for which are pinned:
  void OnToolbarActionAdded(const ToolbarActionsModel::ActionId& id) override {}
  void OnToolbarActionRemoved(const ToolbarActionsModel::ActionId& id) override {}
  void OnToolbarActionUpdated(const ToolbarActionsModel::ActionId& id) override {}
  void OnToolbarModelInitialized() override { Changed(); }
  void OnToolbarPinnedActionsChanged() override { Changed(); }
  void OnToolbarActionsModelShutdown() override { pinned_observation_.Reset(); }

  // extensions::CommandService::Observer, for the commands' keys:
  void OnExtensionCommandAdded(const extensions::ExtensionId& id,
                               const std::string& command) override {
    Changed();
  }
  void OnExtensionCommandRemoved(const extensions::ExtensionId& id,
                                 const std::string& command) override {
    Changed();
  }
  void OnCommandServiceDestroying() override { commands_observation_.Reset(); }

  void Report() {
    report_posted_ = false;
    if (!Host().extensions_changed || !observation_.IsObserving()) {
      return;
    }
    extensions::ExtensionRegistry* registry = extensions::ExtensionRegistry::Get(profile_);
    std::vector<scoped_refptr<const extensions::Extension>> listed;
    for (const scoped_refptr<const extensions::Extension>& extension :
         registry->GenerateInstalledExtensionsSet()) {
      // The ad blocker is Settings › Ad blocking, not an extension of the user's.
      if (extensions::ui_util::ShouldDisplayInExtensionSettings(*extension) &&
          !IsAdBlocker(extension->id())) {
        listed.push_back(extension);
      }
    }
    std::ranges::sort(listed, {}, [](const scoped_refptr<const extensions::Extension>& extension) {
      return extension->name();
    });
    ToolbarActionsModel* model = pinned_observation_.GetSource();
    std::vector<std::vector<KeyedCommand>> keyed;
    std::vector<std::vector<bc_extension_command>> commands;
    keyed.reserve(listed.size());
    commands.reserve(listed.size());
    std::vector<std::string> options;
    options.reserve(listed.size());
    std::vector<bc_extension> extensions;
    extensions.reserve(listed.size());
    for (const scoped_refptr<const extensions::Extension>& extension : listed) {
      // Not one a safe start holds as soon as it can, which the menu would list until then.
      const bool enabled = registry->enabled_extensions().Contains(extension->id()) &&
                           !HoldingLate(profile_, extension->id());
      std::vector<bc_extension_command>& told = commands.emplace_back();
      for (const KeyedCommand& command : keyed.emplace_back(KeyedCommands(profile_, *extension))) {
        told.push_back({command.name.c_str(), command.description.c_str(), command.key.c_str(),
                        command.modifiers, command.suggested});
      }
      extensions.push_back({.id = extension->id().c_str(),
                            .name = extension->name().c_str(),
                            .enabled = enabled,
                            .pinned = model && model->IsActionPinned(extension->id()),
                            .options = options.emplace_back(OptionsPage(*extension)).c_str(),
                            .commands = told.data(),
                            .command_count = told.size(),
                            .removable = !IsAdBlocker(extension->id()),
                            .incognito = extensions::util::IsIncognitoEnabled(extension->id(),
                                                                              profile_),
                            .crashed = !enabled && CrashedOff(profile_, extension->id())});
    }
    Host().extensions_changed(Host().context, ProfileId(DirOf(profile_)), extensions.data(),
                              extensions.size());
  }

  raw_ptr<Profile> profile_;
  base::ScopedObservation<extensions::ExtensionRegistry, extensions::ExtensionRegistryObserver>
      observation_{this};
  base::ScopedObservation<ToolbarActionsModel, ToolbarActionsModel::Observer> pinned_observation_{
      this};
  base::ScopedObservation<extensions::CommandService, extensions::CommandService::Observer>
      commands_observation_{this};
  bool report_posted_ = false;
  base::WeakPtrFactory<ProfileExtensions> weak_factory_{this};
};

std::map<std::string, std::unique_ptr<ProfileExtensions>>& Observed() {
  static base::NoDestructor<std::map<std::string, std::unique_ptr<ProfileExtensions>>> observed;
  return *observed;
}

// The popup opening or shown, one at a time as in Chrome, which tells the app as it shows and as
// it goes. Chrome's popup closes itself when it loses the keyboard or its window shows another tab.
class Popup : public extensions::ExtensionHostObserver {
 public:
  void Open(extensions::ExtensionHost* host, bc_id window, const std::string& extension) {
    observation_.Reset();
    Report(window_, "");
    observation_.Observe(host);
    window_ = window;
    extension_ = extension;
  }

  void Shown(extensions::ExtensionHost* host) {
    if (host && observation_.IsObservingSource(host)) {
      Report(window_, extension_);
    }
  }

 private:
  // extensions::ExtensionHostObserver:
  void OnExtensionHostDestroyed(extensions::ExtensionHost* host) override {
    observation_.Reset();
    Report(window_, "");
    window_ = 0;
  }

  static void Report(bc_id window, const std::string& extension) {
    if (window && Host().window_extension_popup) {
      Host().window_extension_popup(Host().context, window, extension.c_str());
    }
  }

  bc_id window_ = 0;
  std::string extension_;
  base::ScopedObservation<extensions::ExtensionHost, extensions::ExtensionHostObserver>
      observation_{this};
};

Popup& ThePopup() {
  static base::NoDestructor<Popup> popup;
  return *popup;
}

}  // namespace

// One action's icon, which tells the window's actions once it has loaded.
class WindowActions::Icon : public extensions::ExtensionActionIconFactory::Observer {
 public:
  Icon(const extensions::Extension* extension,
       extensions::ExtensionAction* action,
       WindowActions* actions)
      : factory_(extension, action, this), actions_(actions) {}

  gfx::Image Get(int tab) { return factory_.GetIcon(tab); }

 private:
  // extensions::ExtensionActionIconFactory::Observer:
  void OnIconUpdated() override { actions_->Changed(); }

  extensions::ExtensionActionIconFactory factory_;
  raw_ptr<WindowActions> actions_;
};

WindowActions::WindowActions(bc_id window) : window_(window) {}

WindowActions::~WindowActions() = default;

void WindowActions::Show(content::WebContents* contents) {
  contents_ = contents ? contents->GetWeakPtr() : nullptr;
  Profile* profile =
      contents ? Profile::FromBrowserContext(contents->GetBrowserContext()) : nullptr;
  if (profile != profile_) {
    observation_.Reset();
    icons_.clear();
    profile_ = profile;
    if (profile) {
      observation_.Observe(ToolbarActionsModel::Get(profile));
    }
  }
  Changed();
}

void WindowActions::ShowEvery(bool every) {
  if (every != every_) {
    every_ = every;
    Changed();
  }
}

void WindowActions::OnToolbarActionAdded(const ToolbarActionsModel::ActionId& id) {
  Changed();
}
void WindowActions::OnToolbarActionRemoved(const ToolbarActionsModel::ActionId& id) {
  icons_.erase(id);
  Changed();
}
void WindowActions::OnToolbarActionUpdated(const ToolbarActionsModel::ActionId& id) {
  Changed();
}
void WindowActions::OnToolbarModelInitialized() {
  Changed();
}
void WindowActions::OnToolbarPinnedActionsChanged() {
  Changed();
}
// The profile is going.
void WindowActions::OnToolbarActionsModelShutdown() {
  observation_.Reset();
  icons_.clear();
  profile_ = nullptr;
  contents_ = nullptr;
  Changed();
}

void WindowActions::Changed() {
  if (report_posted_) {
    return;
  }
  report_posted_ = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&WindowActions::Report, weak_factory_.GetWeakPtr()));
}

void WindowActions::Report() {
  report_posted_ = false;
  if (!Host().window_extension_actions) {
    return;
  }
  std::vector<Told> shown;
  ToolbarActionsModel* model = observation_.GetSource();
  if (contents_ && model && profile_) {
    const int tab = sessions::SessionTabHelper::IdForTab(contents_.get()).id();
    extensions::ExtensionRegistry* registry = extensions::ExtensionRegistry::Get(profile_);
    auto tell = [&](const extensions::Extension* extension, bool pinned) {
      extensions::ExtensionAction* action =
          extension ? extensions::ExtensionActionManager::Get(profile_)->GetExtensionAction(
                          *extension)
                    : nullptr;
      // Nor its action, pinned before it left the lists or not.
      if (!action || IsAdBlocker(extension->id())) {
        return;
      }
      std::unique_ptr<Icon>& icon = icons_[extension->id()];
      if (!icon) {
        icon = std::make_unique<Icon>(extension, action, this);
      }
      const std::string title = action->GetTitle(tab);
      shown.push_back({.id = extension->id(),
                       .title = title.empty() ? extension->name() : title,
                       .icon = icon->Get(tab),
                       .badge = action->GetDisplayBadgeText(tab),
                       .badge_color = action->HasBadgeBackgroundColor(tab)
                                          ? action->GetBadgeBackgroundColor(tab)
                                          : 0,
                       .enabled = action->GetIsVisible(tab),
                       .pinned = pinned});
    };
    for (const ToolbarActionsModel::ActionId& id : model->pinned_action_ids()) {
      tell(registry->enabled_extensions().GetByID(id), true);
    }
    if (every_) {
      // The model has an action for each turned-on extension that Chrome's extensions menu lists.
      std::vector<const extensions::Extension*> others;
      for (const ToolbarActionsModel::ActionId& id : model->action_ids()) {
        const extensions::Extension* extension = registry->enabled_extensions().GetByID(id);
        if (extension && !model->IsActionPinned(id)) {
          others.push_back(extension);
        }
      }
      std::ranges::sort(others, {}, &extensions::Extension::name);
      for (const extensions::Extension* extension : others) {
        tell(extension, false);
      }
    }
  }
  auto same = [](const Told& a, const Told& b) {
    return a.id == b.id && a.title == b.title && a.badge == b.badge &&
           a.badge_color == b.badge_color && a.enabled == b.enabled && a.pinned == b.pinned &&
           a.icon.IsEmpty() == b.icon.IsEmpty() &&
           (a.icon.IsEmpty() || a.icon.AsImageSkia().BackedBySameObjectAs(b.icon.AsImageSkia()));
  };
  if (std::ranges::equal(shown, told_, same)) {
    return;
  }
  told_ = std::move(shown);
  std::vector<std::vector<uint8_t>> pngs;
  std::vector<bc_extension_action> actions;
  pngs.reserve(told_.size());
  for (const Told& told : told_) {
    std::optional<std::vector<uint8_t>> png;
    if (!told.icon.IsEmpty()) {
      png = gfx::PNGCodec::EncodeBGRASkBitmap(
          told.icon.AsImageSkia().GetRepresentation(2).GetBitmap(), /*discard_transparency=*/false);
    }
    pngs.push_back(png.value_or(std::vector<uint8_t>()));
    actions.push_back({.id = told.id.c_str(),
                       .title = told.title.c_str(),
                       .icon = pngs.back().empty() ? nullptr : pngs.back().data(),
                       .icon_size = pngs.back().size(),
                       .badge = told.badge.c_str(),
                       .badge_color = told.badge_color,
                       .enabled = told.enabled,
                       .pinned = told.pinned});
  }
  Host().window_extension_actions(Host().context, window_, actions.data(), actions.size());
}

void ObserveExtensions(Profile* profile) {
  static base::NoDestructor<CrashWatch> crashes;
  std::unique_ptr<ProfileExtensions>& observed = Observed()[DirOf(profile)];
  observed = std::make_unique<ProfileExtensions>(profile);
}

void ReportExtensions() {
  for (auto& [dir, observed] : Observed()) {
    observed->Changed();
  }
}

void SetExtensionPinned(Profile* profile, const std::string& id, bool pinned) {
  ToolbarActionsModel* model = ToolbarActionsModel::Get(profile);
  if (model && model->HasAction(id) && model->IsActionPinned(id) != pinned) {
    model->SetActionVisibility(id, pinned);
  }
}

void ExtensionCrashTold(Profile* profile, const std::string& id) {
  if (CrashedOff(profile, id)) {
    extensions::ExtensionPrefs::Get(profile)->UpdateExtensionPref(id, kCrashedOffPref,
                                                                  std::nullopt);
    ReportExtensions();
  }
}

bool TurnOnExtension(Profile* profile, const std::string& id) {
  extensions::ExtensionRegistry* registry = extensions::ExtensionRegistry::Get(profile);
  if (!registry->disabled_extensions().Contains(id)) {
    return registry->enabled_extensions().Contains(id);
  }
  extensions::ExtensionRegistrar::Get(profile)->EnableExtension(id);
  return registry->enabled_extensions().Contains(id);
}

bool SafeStart() {
  // The app's switch, given at every launch (app/src/safe_start.rs).
  static const bool safe =
      base::CommandLine::ForCurrentProcess()->GetSwitchValueASCII("safe-start") == "1";
  return safe;
}

void HoldExtensionsForSafeStart(Profile* profile, size_t not_loaded) {
  if (!SafeStart()) {
    return;
  }
  std::vector<extensions::ExtensionId> held;
  for (const scoped_refptr<const extensions::Extension>& extension :
       extensions::ExtensionRegistry::Get(profile)->enabled_extensions()) {
    if (Holdable(profile, *extension)) {
      held.push_back(extension->id());
    }
  }
  extensions::ExtensionRegistrar* registrar = extensions::ExtensionRegistrar::Get(profile);
  for (const extensions::ExtensionId& id : held) {
    registrar->TerminateExtension(id);
  }
  SafeStartHolds()[profile] = std::make_unique<SafeStartHold>(profile, held.size() + not_loaded);
}

bool RemoveExtension(Profile* profile, const std::string& id) {
  // Settings turns it off: removed, it would only come back at the next launch.
  if (IsAdBlocker(id) ||
      !extensions::ExtensionRegistry::Get(profile)->GetInstalledExtension(id)) {
    return false;
  }
  std::u16string error;
  return extensions::ExtensionRegistrar::Get(profile)->UninstallExtension(
      id, extensions::UNINSTALL_REASON_USER_INITIATED, &error);
}

bool RunExtension(bc_id window,
                  BrowserWindowInterface* browser,
                  content::WebContents* contents,
                  const std::string& id,
                  views::View* anchor) {
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  const extensions::Extension* extension = RunsIn(profile, id);
  extensions::ExtensionActionRunner* runner =
      extensions::ExtensionActionRunner::GetForWebContents(contents);
  if (!extension || !runner) {
    return false;
  }
  if (runner->RunAction(extension, /*grant_tab_permissions=*/true) !=
      extensions::ExtensionAction::ShowAction::kShowPopup) {
    return true;
  }
  extensions::ExtensionAction* action =
      extensions::ExtensionActionManager::Get(profile)->GetExtensionAction(*extension);
  const int tab = sessions::SessionTabHelper::IdForTab(contents).id();
  if (!action || !action->HasPopup(tab)) {
    return true;
  }
  std::unique_ptr<extensions::ExtensionViewHost> host =
      extensions::ExtensionViewHostFactory::CreatePopupHost(*extension, action->GetPopupUrl(tab),
                                                            browser);
  if (!host) {
    return true;
  }
  ThePopup().Open(host.get(), window, id);
  ExtensionPopup::ShowPopup(
      browser, std::move(host), views::BubbleAnchor(anchor), views::BubbleBorder::TOP_LEFT,
      PopupShowAction::kShow,
      base::BindOnce([](extensions::ExtensionHost* host) { ThePopup().Shown(host); }));
  return true;
}

bool RunExtensionCommand(content::WebContents* contents,
                         const std::string& id,
                         const std::string& name) {
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  const extensions::Extension* extension = RunsIn(profile, id);
  extensions::EventRouter* router = extensions::EventRouter::Get(profile);
  if (!extension || !router->ExtensionHasEventListener(id, kOnCommand)) {
    return false;
  }
  // Granted first, so the extension can act on the tab as it hears the command.
  if (auto* granter = extensions::ActiveTabPermissionGranter::FromWebContents(contents)) {
    granter->GrantIfRequested(extension);
  }
  base::ListValue args;
  args.Append(name);
  // The commands API is only for the extension's privileged contexts.
  args.Append(extensions::ExtensionTabUtil::CreateTabObject(
                  contents,
                  extensions::ExtensionTabUtil::GetScrubTabBehavior(
                      extension, extensions::mojom::ContextType::kPrivilegedExtension, contents),
                  extension)
                  .ToValue());
  auto event = std::make_unique<extensions::Event>(extensions::events::COMMANDS_ON_COMMAND,
                                                   kOnCommand, std::move(args), profile);
  event->user_gesture = extensions::EventRouter::UserGestureState::kEnabled;
  router->DispatchEventToExtension(id, std::move(event));
  return true;
}

}  // namespace browser_core
