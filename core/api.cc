// The C API in public/browser_core.h, over the core's C++.

#include <algorithm>
#include <iterator>
#include <map>
#include <optional>
#include <string>
#include <vector>

#include "base/files/file_path.h"
#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/numerics/safe_conversions.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/ad_block.h"
#include "browser_core/address_import.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/agent_page.h"
#include "browser_core/agent_session.h"
#include "browser_core/archive.h"
#include "browser_core/auto_archive.h"
#include "browser_core/autofill_data.h"
#include "browser_core/capture.h"
#include "browser_core/cast_dialog.h"
#include "browser_core/clear_site.h"
#include "browser_core/crash_reports.h"
#include "browser_core/developer_mode.h"
#include "browser_core/downloads.h"
#include "browser_core/drm_request.h"
#include "browser_core/extensions.h"
#include "browser_core/favicon.h"
#include "browser_core/favorites.h"
#include "browser_core/find.h"
#include "browser_core/held_keys.h"
#include "browser_core/history_import.h"
#include "browser_core/host.h"
#include "browser_core/idle_unload.h"
#include "browser_core/install_question.h"
#include "browser_core/main_delegate.h"
#include "browser_core/notes.h"
#include "browser_core/page_menu.h"
#include "browser_core/page_text.h"
#include "browser_core/autofill_offer.h"
#include "browser_core/password_offer.h"
#include "browser_core/passwords.h"
#include "browser_core/page_dialog.h"
#include "browser_core/permission_question.h"
#include "browser_core/platform/playready.h"
#include "browser_core/platform/window_views.h"
#include "browser_core/profiles.h"
#include "browser_core/public/browser_core.h"
#include "browser_core/search.h"
#include "browser_core/session.h"
#include "browser_core/site_controls.h"
#include "browser_core/space.h"
#include "browser_core/sync.h"
#include "browser_core/system_engine.h"
#include "browser_core/tab.h"
#include "browser_core/widevine.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "chrome/browser/devtools/devtools_toggle_action.h"
#include "chrome/browser/devtools/devtools_window.h"
#include "chrome/browser/download/download_prefs.h"
#include "chrome/browser/lifetime/browser_shutdown.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/ui/browser_commands.h"
#include "chrome/browser/ui/browser_window/public/browser_window_interface.h"
#include "chrome/browser/ui/navigator/browser_navigator.h"
#include "chrome/browser/ui/navigator/browser_navigator_params.h"
#include "components/autofill/core/common/autofill_prefs.h"
#include "components/password_manager/core/common/password_manager_pref_names.h"
#include "components/prefs/pref_service.h"
#include "components/tabs/public/tab_interface.h"
#include "components/zoom/page_zoom.h"
#include "components/zoom/zoom_controller.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "content/public/common/page_zoom.h"
#include "ui/display/display.h"
#include "ui/display/screen.h"
#include "ui/gfx/geometry/point_f.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/rect_conversions.h"
#include "ui/gfx/geometry/rect_f.h"
#include "ui/native_theme/native_theme.h"
#include "ui/views/widget/widget.h"
#include "ui/views/widget/widget_delegate.h"
#include "url/gurl.h"

#if BUILDFLAG(IS_LINUX)
#include "browser_core/platform/linux/wayland.h"
#include "ui/views/widget/widget.h"
#endif

#if BUILDFLAG(IS_LINUX) || BUILDFLAG(IS_WIN)
#include "browser_core/platform/focus_ring_aura.h"
#endif

namespace {

gfx::Rect ToRect(bc_rect rect) {
  return gfx::ToEnclosingRect(gfx::RectF(rect.x, rect.y, rect.width, rect.height));
}

// The window's widget while it's open. As the browser quits, Chromium can destroy its native window
// before the core's Window goes, leaving a closed widget with no native widget behind it (#474).
views::Widget* OpenWidget(bc_id id) {
  browser_core::Window* window = browser_core::Window::FromId(id);
  views::Widget* widget = window ? window->widget() : nullptr;
  return widget && !widget->IsClosed() ? widget : nullptr;
}

// Copies up to |capacity| bytes of |text| into |to|, ending in a NUL, and returns its length.
size_t CopyString(const std::string& text, char* to, size_t capacity) {
  if (capacity > 0) {
    const size_t length = std::min(capacity - 1, text.size());
    std::copy_n(text.begin(), length, to);
    to[length] = '\0';
  }
  return text.size();
}

// The page |id| is at, which pinning it pins it at.
GURL PageUrlOf(bc_id id) {
  const browser_core::Tab* tab = browser_core::Tab::Get(id);
  return tab ? tab->Url() : GURL();
}

// After |id| was pinned, unpinned or moved, with the rest of its split.
void SplitMoved(browser_core::Space& space, bc_id id) {
  for (bc_id pane : space.Panes(id)) {
    if (browser_core::Tab* tab = browser_core::Tab::Get(pane)) {
      // Handled now, so a pinned tab unpinned unseen has 12 hours in Today before it's archived.
      tab->set_last_seen(base::Time::Now());
      // Whether it's away from its pinned URL.
      tab->StateChanged();
    }
  }
}

// The profile of the Space the window shows, which what it opens and suggests comes from: for a
// private window's, the off-the-record one, or null once its session has gone, as only opening a
// private window starts one.
Profile* ProfileOf(const browser_core::Window& window) {
  const browser_core::Space* space = browser_core::Space::FromId(window.space());
  Profile* profile = browser_core::ProfileFor(space ? space->profile() : std::string());
  return space && space->off_the_record() && profile ? browser_core::PrivateProfileOf(profile)
                                                     : profile;
}

// Loads |url| in the tab, as Chrome loads what's typed in its location bar or a bookmark, giving a
// tab without its page one first.
void LoadInTab(bc_id id, const GURL& url, ui::PageTransition transition) {
  content::WebContents* contents = browser_core::Window::PageOf(id);
  tabs::TabInterface* tab = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  if (!tab) {
    return;
  }
  // Not a link: the user starts something else in the tab, so closing a tab no longer leads back
  // to where it came from, as Chrome's tab strip forgets on a typed or bookmark navigation.
  if (browser_core::Space* space = browser_core::Space::Holding(id)) {
    space->ForgetOpeners();
  }
  browser_core::Tab::FromWebContents(contents)->Load(url, [&] {
    NavigateParams params(tab->GetBrowserWindowInterface(), url, transition);
    params.disposition = WindowOpenDisposition::CURRENT_TAB;
    params.source_contents = contents;
    Navigate(&params);
  });
}

// The loaded profile |profile_id| names, or null.
Profile* LoadedProfile(bc_id profile_id) {
  const std::string dir = browser_core::ProfileDir(profile_id);
  // ProfileFor gives the first profile for one that isn't loaded.
  Profile* profile = dir.empty() ? nullptr : browser_core::ProfileFor(dir);
  return profile && browser_core::DirOf(profile) == dir ? profile : nullptr;
}

}  // namespace

int bc_main(int argc, const char** argv, const bc_host* host) {
  return browser_core::Run(argc, argv, host ? *host : bc_host{});
}

void bc_quit(void) {
  browser_core::Quit();
}

void bc_restart(void) {
  browser_core::Restart();
}

bool bc_quitting(void) {
  return browser_shutdown::IsTryingToQuit();
}

bc_id bc_window_open(bc_rect bounds, void* ui_view) {
  browser_core::Window* window = browser_core::Window::Open(ToRect(bounds), ui_view);
  return window ? window->id() : 0;
}

void* bc_wayland_display(void (*dispatched)(void)) {
#if BUILDFLAG(IS_LINUX)
  return browser_core::platform::JoinWaylandDisplay(dispatched);
#else
  return nullptr;
#endif
}

double bc_display_scale(void) {
  display::Screen* screen = display::Screen::Get();
  return screen ? screen->GetPrimaryDisplay().device_scale_factor() : 1;
}

void* bc_window_wayland_surface(bc_id id, double* width, double* height) {
#if BUILDFLAG(IS_LINUX)
  views::Widget* widget = OpenWidget(id);
  gfx::Size size;
  void* surface = widget ? browser_core::platform::WaylandSurfaceOf(widget, &size) : nullptr;
  *width = size.width();
  *height = size.height();
  return surface;
#else
  return nullptr;
#endif
}

void bc_window_ui_takes_keyboard(bc_id id) {
#if BUILDFLAG(IS_LINUX) || BUILDFLAG(IS_WIN)
  browser_core::platform::UITakesKeyboard(OpenWidget(id), /*until_page=*/false);
#endif
}

void bc_window_ui_holds_keyboard(bc_id id) {
#if BUILDFLAG(IS_LINUX) || BUILDFLAG(IS_WIN)
  browser_core::platform::UITakesKeyboard(OpenWidget(id), /*until_page=*/true);
#endif
}

bool bc_window_keyboard_is_ui(bc_id id) {
#if BUILDFLAG(IS_LINUX)
  return browser_core::platform::KeyboardIsUI(OpenWidget(id));
#else
  return false;
#endif
}

bool bc_window_keyboard_in_child(bc_id id) {
#if BUILDFLAG(IS_LINUX)
  return browser_core::platform::KeyboardInChild(OpenWidget(id));
#else
  return false;
#endif
}

bool bc_window_keyboard_page(bc_id id, bc_rect* frame) {
#if BUILDFLAG(IS_LINUX)
  views::Widget* widget = OpenWidget(id);
  const std::optional<gfx::Rect> page =
      widget ? browser_core::platform::KeyboardPageFrame(widget) : std::nullopt;
  if (!page) {
    return false;
  }
  *frame = {static_cast<double>(page->x()), static_cast<double>(page->y()),
            static_cast<double>(page->width()), static_cast<double>(page->height())};
  return true;
#else
  return false;
#endif
}

bool bc_window_focus_ring(bc_id id, bc_rect* frame) {
#if BUILDFLAG(IS_LINUX) || BUILDFLAG(IS_WIN)
  views::Widget* widget = OpenWidget(id);
  const std::optional<gfx::Rect> ring =
      widget ? browser_core::platform::FocusRingFrame(widget) : std::nullopt;
  if (!ring) {
    return false;
  }
  *frame = {static_cast<double>(ring->x()), static_cast<double>(ring->y()),
            static_cast<double>(ring->width()), static_cast<double>(ring->height())};
  return true;
#else
  return false;
#endif
}

bool bc_window_dialog(bc_id id, bc_rect* frame) {
#if BUILDFLAG(IS_LINUX) || BUILDFLAG(IS_WIN)
  views::Widget* widget = OpenWidget(id);
  const std::optional<gfx::Rect> dialog =
      widget ? browser_core::platform::DialogFrame(widget) : std::nullopt;
  if (!dialog) {
    return false;
  }
  *frame = {static_cast<double>(dialog->x()), static_cast<double>(dialog->y()),
            static_cast<double>(dialog->width()), static_cast<double>(dialog->height())};
  return true;
#else
  return false;
#endif
}

size_t bc_window_child_title(bc_id id, char* title, size_t capacity) {
#if BUILDFLAG(IS_LINUX)
  views::Widget* widget = OpenWidget(id);
  return CopyString(widget ? base::UTF16ToUTF8(browser_core::platform::ChildTitle(widget))
                           : std::string(),
                    title, capacity);
#else
  return CopyString(std::string(), title, capacity);
#endif
}

void bc_answer_clipboard_requests(void) {
#if BUILDFLAG(IS_LINUX)
  browser_core::platform::AnswerClipboardRequests();
#endif
}

void bc_window_overlay_takes_keyboard(bc_id id, bool takes) {
#if BUILDFLAG(IS_LINUX)
  if (views::Widget* widget = OpenWidget(id)) {
    browser_core::platform::OverlayTakesKeyboard(widget, takes);
    browser_core::Window::FromId(id)->OverlayTakesKeyboard(takes);
  }
#endif
}

void bc_window_minimize(bc_id id) {
#if BUILDFLAG(IS_LINUX)
  if (views::Widget* widget = OpenWidget(id)) {
    widget->Minimize();
  }
#endif
}

void bc_window_toggle_maximized(bc_id id) {
#if BUILDFLAG(IS_LINUX)
  views::Widget* widget = OpenWidget(id);
  if (!widget) {
    return;
  }
  if (widget->IsFullscreen()) {
    browser_core::Window::FromId(id)->LeaveFullScreen();
  } else if (widget->IsMaximized()) {
    widget->Restore();
  } else {
    widget->Maximize();
  }
#endif
}

void bc_window_ui_text_input(bc_id id, bool focused, double x, double y, double height) {
#if BUILDFLAG(IS_LINUX)
  if (views::Widget* widget = OpenWidget(id)) {
    std::optional<gfx::Rect> caret;
    if (focused) {
      caret = gfx::ToEnclosingRect(gfx::RectF(x, y, 0, height));
    }
    browser_core::platform::SetUITextInput(widget, caret);
  }
#endif
}

void bc_hold_keys(bool in_core) {
  browser_core::held_keys::Hold(in_core);
}

void bc_give_held_keys(void (*take)(void* context, const bc_key* keys, size_t count),
                       void* context) {
  browser_core::held_keys::Give(take, context);
}

bool bc_window_restore(bc_id id) {
  browser_core::Window* window = browser_core::Window::FromId(id);
  return window && browser_core::RestoreSession(*window);
}

size_t bc_saved_window_frames(bc_rect* frames, size_t count) {
  const std::vector<gfx::Rect> saved = browser_core::SavedWindowFrames();
  for (size_t i = 0; i < std::min(count, saved.size()); ++i) {
    frames[i] = {static_cast<double>(saved[i].x()), static_cast<double>(saved[i].y()),
                 static_cast<double>(saved[i].width()), static_cast<double>(saved[i].height())};
  }
  return saved.size();
}

bc_id bc_window_open_saved(size_t index, bc_rect bounds, void* ui_view) {
  if (index == 0 || index >= browser_core::SavedWindowFrames().size()) {
    return 0;
  }
  browser_core::Window* window =
      browser_core::Window::Open(ToRect(bounds), ui_view, /*behind=*/true);
  if (!window) {
    return 0;
  }
  if (!browser_core::RestoreSavedWindow(*window, index) &&
      !browser_core::Space::All().empty()) {
    window->ShowSpace(browser_core::Space::All().front()->id());
  }
  return window->id();
}

void bc_window_close(bc_id id) {
  if (browser_core::Window* window = browser_core::Window::FromId(id)) {
    window->Close();
  }
}

void bc_window_set_title(bc_id id, const char* title) {
  views::Widget* widget = OpenWidget(id);
  if (widget && widget->widget_delegate() && title) {
    widget->widget_delegate()->SetTitle(base::UTF8ToUTF16(title));
  }
}

void bc_window_activate(bc_id id) {
  views::Widget* widget = OpenWidget(id);
  if (!widget) {
    return;
  }
  if (widget->IsMinimized()) {
    widget->Restore();
  }
  widget->Activate();
}

bc_id bc_space_create(const char* name, bc_id after) {
  const std::vector<browser_core::Space*> all = browser_core::Space::All();
  auto it = std::ranges::find(all, after, &browser_core::Space::id);
  const size_t at = it == all.end() ? SIZE_MAX : static_cast<size_t>(it - all.begin()) + 1;
  browser_core::Space* space = browser_core::Space::Create(name ? name : "", 0, std::nullopt, at);
  space->SetProfile(browser_core::DirOf(browser_core::StartProfile()));
  return space->id();
}

bc_id bc_space_create_for_agent(const char* name, const char* agent, const char* agent_name) {
  if (!agent || !*agent || !agent_name || !*agent_name) {
    return 0;
  }
  browser_core::Space* space = browser_core::Space::Create(
      name ? name : "", 0, browser_core::Space::Agent{.id = agent, .name = agent_name});
  space->SetProfile(browser_core::DirOf(browser_core::StartProfile()));
  return space->id();
}

size_t bc_space_agent(bc_id id, char* agent, size_t capacity) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  return CopyString(space && space->agent() ? space->agent()->id : std::string(), agent, capacity);
}

size_t bc_space_agent_name(bc_id id, char* name, size_t capacity) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  return CopyString(space && space->agent() ? space->agent()->name : std::string(), name,
                    capacity);
}

bool bc_agent_may_act(const char* agent,
                      bc_id tab,
                      const char* action,
                      char* reason,
                      size_t capacity) {
  content::WebContents* contents = browser_core::Tab::FromId(tab);
  // An action no agent may do is refused whatever the tab, so that's the reason given.
  std::string refused = browser_core::RefusedAction(action ? action : "");
  if (refused.empty() && (!agent || !*agent || !browser_core::Space::AgentMayAct(agent, tab))) {
    refused = "The tab isn't in the agent's own Space.";
  }
  if (refused.empty() && browser_core::IsHeld(tab)) {
    refused = "The user has this tab until they hand it back. Ask for it with ask_for_tab.";
  }
  if (refused.empty() && contents &&
      browser_core::Tab::FromWebContents(contents)->system_page()) {
    // Its Space never switches a tab there, but the rule is the core's to keep, not the UI's.
    refused =
        "The tab shows its page in the system's engine, where the browser can't keep an agent's "
        "rules.";
  }
  CopyString(refused, reason, capacity);
  if (browser_core::Host().agent_activity) {
    const bc_agent_activity entry = {
        .agent = agent ? agent : "",
        .tab = tab,
        .action = action ? action : "",
        .refused = refused.c_str(),
    };
    browser_core::Host().agent_activity(browser_core::Host().context, &entry);
  }
  if (refused.empty()) {
    browser_core::AgentActed(agent, tab);
  }
  return refused.empty();
}

bool bc_agent_release_tab(const char* agent, bc_id tab) {
  return agent && browser_core::EndAgentSession(agent, tab, BC_AGENT_SESSION_RELEASED);
}

size_t bc_agent_disconnected(const char* agent) {
  return agent ? browser_core::EndAgentSessions(agent, BC_AGENT_SESSION_DISCONNECTED) : 0;
}

bool bc_tab_take_back(bc_id tab) {
  browser_core::Hold(tab);
  return browser_core::EndTabSession(tab, BC_AGENT_SESSION_TAKEN_BACK);
}

bool bc_tab_give_to_agent(bc_id tab, bc_id space) {
  browser_core::Space* from = browser_core::Space::Holding(tab);
  browser_core::Space* to = browser_core::Space::FromId(space);
  // The page goes as it is, with its profile's logins, so never into another profile's Space,
  // where it would load again.
  if (!from || from->agent() || !to || !to->agent() || from->profile() != to->profile()) {
    return false;
  }
  browser_core::Window::MoveToSpace(tab, space, /*to_agent=*/true);
  if (browser_core::Space::Holding(tab) != to) {
    return false;
  }
  // One the user took back from an agent before is the agent's now.
  browser_core::HandBack(tab);
  return true;
}

bool bc_agent_hand_off(const char* agent, bc_id tab) {
  if (!agent || !browser_core::Space::AgentMayAct(agent, tab) || browser_core::IsHeld(tab)) {
    return false;
  }
  browser_core::Hold(tab);
  return true;
}

bool bc_tab_hand_back(bc_id tab) {
  return browser_core::HandBack(tab);
}

bool bc_tab_held(bc_id tab) {
  return browser_core::IsHeld(tab);
}

bc_id bc_agent_open_tab(const char* agent, const char* url) {
  const GURL target(url ? url : "");
  if (!agent || !*agent || !target.SchemeIsHTTPOrHTTPS()) {
    return 0;
  }
  browser_core::Space* space = nullptr;
  for (browser_core::Space* each : browser_core::Space::All()) {
    if (each->agent() && each->agent()->id == agent) {
      space = each;
    }
  }
  // The window showing the agent's Space, or else the first of the user's, for its Browser.
  browser_core::Window* window = nullptr;
  for (browser_core::Window* each : browser_core::Window::All()) {
    if (!each->own_space() && (!window || (space && each->space() == space->id()))) {
      window = each;
    }
  }
  Profile* profile = space ? browser_core::ProfileFor(space->profile()) : nullptr;
  content::WebContents* contents =
      window && profile ? window->OpenTab(profile, target, /*when_shown=*/false) : nullptr;
  if (!contents) {
    return 0;
  }
  const bc_id tab = browser_core::Tab::IdOf(contents);
  space->Add(tab, /*opener=*/0, /*foreground=*/false);
  browser_core::AgentActed(agent, tab);
  if (browser_core::Host().tab_opened) {
    browser_core::Host().tab_opened(browser_core::Host().context, window->id(), tab, 0, false);
  }
  return tab;
}

bool bc_agent_navigate(const char* agent, bc_id tab, const char* url) {
  const GURL target(url ? url : "");
  if (!agent || !browser_core::Space::AgentMayAct(agent, tab) || browser_core::IsHeld(tab) ||
      !target.SchemeIsHTTPOrHTTPS() || !browser_core::Window::PageOf(tab)) {
    return false;
  }
  LoadInTab(tab, target, ui::PAGE_TRANSITION_TYPED);
  return true;
}

size_t bc_agent_list_tabs(const char* agent, bc_id* tabs, size_t capacity) {
  if (!agent || !*agent) {
    return 0;
  }
  // The agent's Space, found here as bc_agent_open_tab finds it, so no other Space is listed.
  const browser_core::Space* space = nullptr;
  for (const browser_core::Space* each : browser_core::Space::All()) {
    if (each->agent() && each->agent()->id == agent) {
      space = each;
    }
  }
  if (!space) {
    return 0;
  }
  std::vector<bc_id> listed = space->pinned();
  std::ranges::copy(space->today(), std::back_inserter(listed));
  std::copy_n(listed.begin(), std::min(capacity, listed.size()), tabs);
  return listed.size();
}

void bc_agent_page(const char* agent,
                   bc_id tab,
                   uint64_t request,
                   int action,
                   int64_t element,
                   const char* text) {
  const bool may =
      agent && browser_core::Space::AgentMayAct(agent, tab) && !browser_core::IsHeld(tab);
  // A tab the browser unloaded to save memory gets its page back, not shown.
  content::WebContents* contents = may ? browser_core::Window::PageOf(tab) : nullptr;
  if (!contents) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](bc_id tab, uint64_t request) {
                         if (browser_core::Host().agent_page_done) {
                           browser_core::Host().agent_page_done(
                               browser_core::Host().context, tab, request,
                               "The tab isn't in the agent's own Space.", true);
                         }
                       },
                       tab, request));
    return;
  }
  browser_core::AgentPage::CreateForWebContents(contents);
  browser_core::AgentPage::FromWebContents(contents)->Act(request, action, element,
                                                          text ? text : "");
}

bc_id bc_space_profile(bc_id id) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  return space ? browser_core::ProfileId(space->profile()) : 0;
}

bool bc_space_set_profile(bc_id id, bc_id profile) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  const std::string dir = browser_core::ProfileDir(profile);
  if (!space || space->own() || dir.empty() || !space->pinned_rows().empty() ||
      !space->today().empty()) {
    return false;
  }
  // At once for a profile that's loaded, as a Space's is.
  browser_core::LoadProfiles({dir},
                             base::BindOnce(&browser_core::Window::SetSpaceProfile, id, dir));
  return true;
}

size_t bc_profiles(bc_id* profiles, size_t capacity) {
  const std::vector<bc_id> all = browser_core::Profiles();
  std::copy_n(all.begin(), std::min(capacity, all.size()), profiles);
  return all.size();
}

size_t bc_profile_name(bc_id profile, char* name, size_t capacity) {
  return CopyString(browser_core::ProfileName(profile), name, capacity);
}

bool bc_profile_rename(bc_id profile, const char* name) {
  return browser_core::RenameProfile(profile, name ? name : "");
}

bc_id bc_profile_create(const char* name) {
  return browser_core::CreateProfile(name ? name : "", base::DoNothing());
}

bool bc_profile_delete(bc_id profile) {
  return browser_core::DeleteProfile(profile);
}

size_t bc_profile_download_folder(bc_id profile_id, char* path, size_t capacity) {
  Profile* profile = browser_core::ProfileOrLoad(profile_id);
  return CopyString(
      profile ? DownloadPrefs::FromBrowserContext(profile)->DownloadPath().AsUTF8Unsafe() : "",
      path, capacity);
}

bool bc_profile_set_download_folder(bc_id profile_id, const char* path) {
  Profile* profile = browser_core::ProfileOrLoad(profile_id);
  const base::FilePath folder = base::FilePath::FromUTF8Unsafe(path ? path : "");
  if (!profile || !folder.IsAbsolute()) {
    return false;
  }
  DownloadPrefs* prefs = DownloadPrefs::FromBrowserContext(profile);
  if (prefs->IsDownloadPathManaged()) {
    return false;
  }
  // As Chrome's Location setting, which sets where Save Page As starts too.
  prefs->SetDownloadPath(folder);
  return true;
}

// The pref of a BC_SAVE_ value, or null.
static const char* OfferPref(int what) {
  switch (what) {
    case BC_SAVE_PASSWORDS:
      return password_manager::prefs::kCredentialsEnableService;
    case BC_SAVE_ADDRESSES:
      return autofill::prefs::kAutofillProfileEnabled;
    case BC_SAVE_CARDS:
      return autofill::prefs::kAutofillCreditCardEnabled;
  }
  return nullptr;
}

bool bc_profile_offers_to_save(bc_id profile_id, int what, bool* offers) {
  Profile* profile = browser_core::ProfileOrLoad(profile_id);
  const char* pref = OfferPref(what);
  if (!profile || !pref || !offers) {
    return false;
  }
  *offers = profile->GetPrefs()->GetBoolean(pref);
  return true;
}

bool bc_profile_set_offers_to_save(bc_id profile_id, int what, bool offer) {
  Profile* profile = browser_core::ProfileOrLoad(profile_id);
  PrefService* prefs = profile ? profile->GetPrefs() : nullptr;
  const char* pref = OfferPref(what);
  if (!prefs || !pref || !prefs->IsUserModifiablePreference(pref)) {
    return false;
  }
  prefs->SetBoolean(pref, offer);
  return true;
}

size_t bc_profile_search_engines(bc_id profile, char* engines, size_t capacity) {
  return CopyString(browser_core::DefaultEngineChoices(browser_core::ProfileOrLoad(profile)),
                    engines, capacity);
}

size_t bc_profile_search_engine(bc_id profile, char* keyword, size_t capacity) {
  return CopyString(
      base::UTF16ToUTF8(browser_core::DefaultEngine(browser_core::ProfileOrLoad(profile))),
      keyword, capacity);
}

bool bc_profile_set_search_engine(bc_id profile, const char* keyword) {
  return keyword && browser_core::SetDefaultEngine(browser_core::ProfileOrLoad(profile),
                                                   base::UTF8ToUTF16(keyword));
}

double bc_profile_archive_after(bc_id profile_id) {
  Profile* profile = browser_core::ProfileOrLoad(profile_id);
  return profile ? browser_core::ProfileArchiveAfter(profile).InSecondsF() : 0;
}

bool bc_profile_set_archive_after(bc_id profile, double seconds) {
  return seconds > 0 && browser_core::SetProfileArchiveAfter(browser_core::ProfileOrLoad(profile),
                                                              base::Seconds(seconds));
}

size_t bc_profile_notes_service(bc_id profile_id, char* service, size_t capacity) {
  Profile* profile = LoadedProfile(profile_id);
  return CopyString(profile ? browser_core::notes::Service(profile) : std::string(), service,
                    capacity);
}

void bc_profile_set_notes_service(bc_id profile_id, const char* service) {
  if (Profile* profile = LoadedProfile(profile_id); profile && service) {
    browser_core::notes::SetService(profile, service);
  }
}

bool bc_profile_add_history(bc_id profile, const bc_history_page* pages, size_t count) {
  return (pages || !count) &&
         browser_core::AddImportedHistory(LoadedProfile(profile), profile, pages, count);
}

bool bc_profile_add_addresses(bc_id profile, const bc_address* addresses, size_t count) {
  return (addresses || !count) &&
         browser_core::AddImportedAddresses(LoadedProfile(profile), profile, addresses, count);
}

size_t bc_spaces(bc_id* spaces, size_t capacity) {
  std::vector<browser_core::Space*> all = browser_core::Space::All();
  for (size_t i = 0; i < std::min(capacity, all.size()); ++i) {
    spaces[i] = all[i]->id();
  }
  return all.size();
}

size_t bc_space_name(bc_id id, char* name, size_t capacity) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  return space ? CopyString(space->name(), name, capacity) : 0;
}

void bc_space_set_name(bc_id id, const char* name) {
  if (browser_core::Space* space = browser_core::Space::FromId(id)) {
    space->SetName(name ? name : "");
  }
}

bool bc_space_color(bc_id id, uint32_t* color) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  if (!space || !space->color()) {
    return false;
  }
  *color = *space->color();
  return true;
}

void bc_space_set_color(bc_id id, bool has_color, uint32_t color) {
  if (browser_core::Space* space = browser_core::Space::FromId(id)) {
    space->SetColor(has_color ? std::optional(color & 0xffffff) : std::nullopt);
  }
}

size_t bc_space_icon(bc_id id, char* icon, size_t capacity) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  return space ? CopyString(space->icon(), icon, capacity) : 0;
}

void bc_space_set_icon(bc_id id, const char* icon) {
  if (browser_core::Space* space = browser_core::Space::FromId(id)) {
    space->SetIcon(icon ? icon : "");
  }
}

size_t bc_space_theme(bc_id id, char* theme, size_t capacity) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  return space ? CopyString(space->theme(), theme, capacity) : 0;
}

void bc_space_set_theme(bc_id id, const char* theme) {
  if (browser_core::Space* space = browser_core::Space::FromId(id)) {
    space->SetTheme(theme ? theme : "");
  }
}

bool bc_space_delete(bc_id id) {
  return browser_core::Window::DeleteSpace(id);
}

size_t bc_space_today(bc_id id, bc_id* tabs, size_t capacity) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  if (!space) {
    return 0;
  }
  const std::vector<bc_id>& today = space->today();
  std::copy_n(today.begin(), std::min(capacity, today.size()), tabs);
  return today.size();
}

size_t bc_space_pinned(bc_id id, bc_id* tabs, size_t capacity) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  if (!space) {
    return 0;
  }
  const std::vector<bc_id>& pinned = space->pinned_rows();
  std::copy_n(pinned.begin(), std::min(capacity, pinned.size()), tabs);
  return pinned.size();
}

bool bc_folder(bc_id id, bool* open) {
  browser_core::Space* space = browser_core::Space::HoldingFolder(id);
  if (space && open) {
    *open = space->IsOpen(id);
  }
  return space;
}

size_t bc_folder_name(bc_id folder, char* name, size_t capacity) {
  browser_core::Space* space = browser_core::Space::HoldingFolder(folder);
  return space ? CopyString(space->FolderName(folder), name, capacity) : 0;
}

bc_id bc_folder_of(bc_id id) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  if (!space) {
    space = browser_core::Space::HoldingFolder(id);
  }
  return space ? space->FolderOf(id) : 0;
}

bc_id bc_folder_create(bc_id id, const char* name, bc_id folder, size_t index) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  return space ? space->AddFolder(name ? name : "", folder, index) : 0;
}

void bc_folder_rename(bc_id folder, const char* name) {
  if (browser_core::Space* space = browser_core::Space::HoldingFolder(folder)) {
    space->RenameFolder(folder, name ? name : "");
  }
}

bool bc_space_pinned_open(bc_id id) {
  browser_core::Space* space = browser_core::Space::FromId(id);
  return !space || space->pinned_open();
}

void bc_space_set_pinned_open(bc_id id, bool open) {
  if (browser_core::Space* space = browser_core::Space::FromId(id)) {
    space->SetPinnedOpen(open);
  }
}

size_t bc_folder_icon(bc_id folder, char* icon, size_t capacity) {
  browser_core::Space* space = browser_core::Space::HoldingFolder(folder);
  return space ? CopyString(space->FolderIcon(folder), icon, capacity) : 0;
}

void bc_folder_set_icon(bc_id folder, const char* icon) {
  if (browser_core::Space* space = browser_core::Space::HoldingFolder(folder)) {
    space->SetFolderIcon(folder, icon ? icon : "");
  }
}

bc_id bc_folder_duplicate(bc_id folder) {
  browser_core::Space* space = browser_core::Space::HoldingFolder(folder);
  if (!space) {
    return 0;
  }
  // A copy of the rows, which the copies go in among.
  const std::vector<bc_id> rows = space->pinned_rows();
  const auto at = std::ranges::find(rows, folder);
  const bc_id holder = space->FolderOf(folder);
  // Where |folder| is in its holder, counted as bc_folder_create counts.
  const size_t index = std::ranges::count_if(
      rows.begin(), at, [&](bc_id row) { return space->FolderOf(row) == holder; });
  // Each folder copied, to its copy, and how many rows each copy holds so far.
  std::map<bc_id, bc_id> copies;
  std::map<bc_id, size_t> held;
  const auto copy_folder = [&](bc_id from, bc_id into, size_t to) {
    const bc_id copy = space->AddFolder(space->FolderName(from), into, to);
    space->SetFolderIcon(copy, space->FolderIcon(from));
    space->SetOpen(copy, space->IsOpen(from));
    copies[from] = copy;
    return copy;
  };
  const bc_id copy = copy_folder(folder, holder, index + 1);
  if (!copy) {
    return 0;
  }
  // What's in |folder| follows it, each row after the folder holding it.
  for (auto row = std::next(at); row != rows.end(); ++row) {
    auto in = copies.find(space->FolderOf(*row));
    if (in == copies.end()) {
      break;
    }
    const bc_id into = in->second;
    if (space->IsFolder(*row)) {
      copy_folder(*row, into, held[into]++);
      continue;
    }
    const browser_core::Tab* tab = browser_core::Tab::Get(*row);
    // Unloaded, as a tab restored after a quit is, to load once shown.
    const bc_id copied = tab && tab->profile()
                             ? browser_core::Tab::Restore(tab->profile(), tab->Save())
                             : 0;
    if (copied) {
      space->AddPinned(copied, into, held[into]++, space->PinnedUrl(*row));
    }
  }
  return copy;
}

void bc_folder_set_open(bc_id folder, bool open) {
  if (browser_core::Space* space = browser_core::Space::HoldingFolder(folder)) {
    space->SetOpen(folder, open);
  }
}

void bc_folder_move(bc_id folder, bc_id to, size_t index) {
  if (browser_core::Space* space = browser_core::Space::HoldingFolder(folder)) {
    space->Move(folder, /*pinned=*/true, to, index, &PageUrlOf);
  }
}

void bc_folder_delete(bc_id folder) {
  if (browser_core::Space* space = browser_core::Space::HoldingFolder(folder)) {
    space->DeleteFolder(folder);
  }
}

bc_id bc_tab_profile(bc_id id) {
  const browser_core::Tab* tab = browser_core::Tab::Get(id);
  Profile* profile = tab ? tab->profile() : nullptr;
  return profile ? browser_core::ProfileId(browser_core::DirOf(profile)) : 0;
}

size_t bc_favorites(bc_id* tabs, size_t capacity) {
  const std::vector<bc_id> favorites = browser_core::Favorites::All();
  std::copy_n(favorites.begin(), std::min(capacity, favorites.size()), tabs);
  return favorites.size();
}

bc_id bc_favorite_space(bc_id tab) {
  return browser_core::Favorites::SpaceOf(tab);
}

int bc_favorites_scope(void) {
  return browser_core::Favorites::scope();
}

void bc_set_favorites_scope(int scope) {
  browser_core::Favorites::SetScope(scope);
  // Each window stops showing a favorite its Space no longer has.
  browser_core::Window::HiddenFavoritesGone();
}

size_t bc_tab_split(bc_id id, bc_id* tabs, size_t capacity) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  if (!space) {
    return 0;
  }
  const std::vector<bc_id> panes = space->Panes(id);
  std::copy_n(panes.begin(), std::min(capacity, panes.size()), tabs);
  return panes.size();
}

bool bc_tab_split_with(bc_id id, bc_id with, bool left) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  if (!space || browser_core::Space::Holding(with) != space ||
      !space->Split(id, with, PageUrlOf(id), left)) {
    return false;
  }
  SplitMoved(*space, id);
  browser_core::Window::PanesChanged();
  return true;
}

void bc_tab_separate(bc_id id) {
  if (browser_core::Space* space = browser_core::Space::Holding(id)) {
    space->Separate(id);
    browser_core::Window::PanesChanged();
  }
}

bool bc_tab_move_in_split(bc_id id, size_t index) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  if (!space || !space->MoveInSplit(id, index)) {
    return false;
  }
  browser_core::Window::PanesChanged();
  return true;
}

bool bc_tab_split_stacked(bc_id id) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  return space && space->Stacked(id);
}

double bc_tab_split_share(bc_id id) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  std::vector<bc_id> panes = space ? space->Panes(id) : std::vector<bc_id>();
  if (panes.size() < 2) {
    return 0;
  }
  std::vector<double> shares = space->Shares(id);
  return shares.empty() ? 1.0 / panes.size() : shares[std::ranges::find(panes, id) - panes.begin()];
}

bool bc_tab_set_split_shares(bc_id id, const double* shares, size_t count) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  if (!space ||
      !space->SetShares(id, shares ? std::vector<double>(shares, shares + count)
                                   : std::vector<double>())) {
    return false;
  }
  browser_core::Window::PanesChanged();
  return true;
}

bc_id bc_tab_split_expanded(bc_id id) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  return space ? space->Expanded(id) : 0;
}

bool bc_tab_set_split_expanded(bc_id id, bool expanded) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  if (!space || !space->SetExpanded(id, expanded)) {
    return false;
  }
  browser_core::Window::PanesChanged();
  return true;
}

bool bc_tab_set_split_stacked(bc_id id, bool stacked) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  if (!space || !space->SetStacked(id, stacked)) {
    return false;
  }
  browser_core::Window::PanesChanged();
  return true;
}

void bc_window_show_space(bc_id window_id, bc_id space) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->ShowSpace(space);
  }
}

void bc_window_show_space_empty(bc_id window_id, bc_id space) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->ShowSpace(space, false);
  }
}

double bc_window_sidebar_width(bc_id window_id) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->sidebar_width() : 0;
}

void bc_window_set_sidebar_width(bc_id window_id, double width) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->SetSidebarWidth(width);
  }
}

bc_id bc_window_space_tab(bc_id window_id, bc_id space) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->TabToShowIn(space) : 0;
}

void bc_window_set_page_frame(bc_id window_id, bc_rect frame, double corner_radius) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->SetPageFrame(ToRect(frame), corner_radius);
  }
}

void bc_window_set_focus_color(bc_id window_id, uint32_t color) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->SetFocusColor(color & 0xffffff);
  }
}

void bc_window_set_page_color(bc_id window_id, uint32_t color) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->SetPageColor(color & 0xffffff);
  }
}

void bc_window_page_moving(bc_id window_id, bool moving) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->SetPageMoving(moving);
  }
}

void bc_window_select(bc_id window_id, bc_id tab) {
  browser_core::HandoverClicked();
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    // The user's switch, which Chrome's tab strip judges by where the two tabs came from. The
    // core's own, as after a close, leave the openers be.
    const bc_id was = window->selected();
    browser_core::Space* from = browser_core::Space::Holding(was);
    browser_core::Space* to = browser_core::Space::Holding(tab);
    if (to) {
      to->UserShowed(was, tab);
    }
    if (from && from != to) {
      from->UserShowed(was, tab);
    }
    window->Select(tab);
  }
}

void bc_window_focus_page(bc_id window_id) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->GiveKeyboardToPage();
  }
}

bc_id bc_window_clear(bc_id window_id) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->Clear() : 0;
}

size_t bc_window_undo_clear(bc_id window_id, bc_id clear) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->UndoClear(clear) : 0;
}

bc_id bc_window_undo_delete_space(bc_id window_id, bc_id space) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->UndoDeleteSpace(space) : 0;
}

bc_id bc_window_reopen_closed_tab(bc_id window_id) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->ReopenClosedTab() : 0;
}

bc_id bc_window_reopen_tab(bc_id window_id, bc_id closed) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->ReopenClosed(closed, /*show=*/false) : 0;
}

bc_id bc_window_restore_archived(bc_id window_id, bc_id archived) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->RestoreArchived(archived) : 0;
}

void bc_window_answer_downloads(bc_id window_id, bool cancel) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->AnswerDownloads(cancel);
  }
}

void bc_extension_answer(bc_id question, bool accept) {
  browser_core::InstallQuestion::Answer(question, accept);
}

void bc_extensions_report() {
  browser_core::ReportExtensions();
}

void bc_extension_set_pinned(bc_id profile_id, const char* id, bool pinned) {
  if (Profile* profile = LoadedProfile(profile_id); profile && id) {
    browser_core::SetExtensionPinned(profile, id, pinned);
  }
}

bool bc_extension_remove(bc_id profile_id, const char* id) {
  Profile* profile = LoadedProfile(profile_id);
  return profile && id && browser_core::RemoveExtension(profile, id);
}

void bc_extension_crash_told(bc_id profile_id, const char* id) {
  if (Profile* profile = LoadedProfile(profile_id); profile && id) {
    browser_core::ExtensionCrashTold(profile, id);
  }
}

bool bc_extension_turn_on(bc_id profile_id, const char* id) {
  Profile* profile = LoadedProfile(profile_id);
  return profile && id && browser_core::TurnOnExtension(profile, id);
}

bool bc_window_run_extension(bc_id window_id, const char* id, bc_rect anchor) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window && id && window->RunExtension(id, ToRect(anchor));
}

bool bc_window_run_extension_command(bc_id window_id, const char* id, const char* name) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window && id && name && window->RunExtensionCommand(id, name);
}

bc_id bc_archive_clear(void) {
  return browser_core::RemoveAllArchived();
}

bc_id bc_archive_remove(bc_id archived) {
  return browser_core::RemoveArchived(archived);
}

size_t bc_archive_undo_remove(bc_id removal) {
  return browser_core::UndoRemoved(removal);
}

double bc_archive_after(void) {
  return browser_core::UserSettings().archive_after.InSecondsF();
}

void bc_set_archive_after(double seconds) {
  if (!(seconds > 0)) {
    return;
  }
  browser_core::UserSettings().archive_after = base::Seconds(seconds);
  browser_core::ScheduleAutoArchive();
  browser_core::SessionChanged();
}

double bc_little_archive_after(void) {
  return browser_core::UserSettings().little_archive_after.InSecondsF();
}

void bc_set_little_archive_after(double seconds) {
  if (!(seconds > 0)) {
    return;
  }
  browser_core::UserSettings().little_archive_after = base::Seconds(seconds);
  browser_core::ScheduleAutoArchive();
  browser_core::SessionChanged();
}

bool bc_haptics(void) {
  return browser_core::UserSettings().haptics;
}

void bc_set_haptics(bool haptics) {
  browser_core::UserSettings().haptics = haptics;
  browser_core::SessionChanged();
}

void bc_set_appearance(int appearance) {
  // Chromium's own override of the system's scheme, which every page's color provider follows.
  using Scheme = ui::NativeTheme::PreferredColorScheme;
  ui::NativeTheme::SetPreferredColorSchemeOverride(
      appearance == 1   ? std::optional(Scheme::kLight)
      : appearance == 2 ? std::optional(Scheme::kDark)
                        : std::nullopt);
}

bool bc_link_previews(void) {
  return browser_core::UserSettings().link_previews;
}

void bc_set_link_previews(bool previews) {
  browser_core::UserSettings().link_previews = previews;
  browser_core::SessionChanged();
}

bool bc_option_reveals_sidebar(void) {
  return browser_core::UserSettings().option_reveals_sidebar;
}

void bc_set_option_reveals_sidebar(bool reveals) {
  browser_core::UserSettings().option_reveals_sidebar = reveals;
  browser_core::SessionChanged();
}

int bc_hover_kinds(void) {
  return browser_core::UserSettings().hover_kinds;
}

void bc_set_hover_kinds(int kinds) {
  browser_core::UserSettings().hover_kinds = kinds;
  browser_core::SessionChanged();
}

bool bc_archive_note(void) {
  return browser_core::ArchiveNoteShown();
}

void bc_dismiss_archive_note(void) {
  browser_core::DismissArchiveNote();
}

bool bc_warn_before_quitting(void) {
  return browser_core::UserSettings().warn_before_quitting;
}

void bc_set_warn_before_quitting(bool warn) {
  browser_core::UserSettings().warn_before_quitting = warn;
  browser_core::SessionChanged();
}

bool bc_restore_windows(void) {
  return browser_core::UserSettings().restore_windows;
}

void bc_set_restore_windows(bool restore) {
  browser_core::UserSettings().restore_windows = restore;
  browser_core::SessionChanged();
}

bool bc_warn_moving_profiles(void) {
  return browser_core::UserSettings().warn_moving_profiles;
}

void bc_set_warn_moving_profiles(bool warn) {
  browser_core::UserSettings().warn_moving_profiles = warn;
  browser_core::SessionChanged();
}

bool bc_unload_idle_tabs(void) {
  return browser_core::UserSettings().unload_idle_tabs;
}

void bc_set_unload_idle_tabs(bool unload) {
  browser_core::UserSettings().unload_idle_tabs = unload;
  browser_core::SessionChanged();
  browser_core::UnloadIdleTabsSoon();
}

void bc_age_tabs_for_testing(double seconds) {
  browser_core::AgeTabsForTesting(base::Seconds(seconds));
}

bool bc_ad_blocking(void) {
  return browser_core::UserSettings().ad_blocking;
}

void bc_set_ad_blocking(bool on) {
  browser_core::UserSettings().ad_blocking = on;
  browser_core::ApplyAdBlocking();
  browser_core::SessionChanged();
}

bool bc_auto_picture_in_picture(void) {
  return browser_core::UserSettings().auto_picture_in_picture;
}

void bc_set_auto_picture_in_picture(bool on) {
  browser_core::UserSettings().auto_picture_in_picture = on;
  browser_core::ApplyAutoPictureInPicture(/*changed=*/true);
  browser_core::SessionChanged();
}

size_t bc_crash_reports_collector(char* host, size_t capacity) {
  return CopyString(browser_core::crash_reports::CollectorHost(), host, capacity);
}

bool bc_send_crash_reports(void) {
  return browser_core::crash_reports::Sending();
}

void bc_set_send_crash_reports(bool send) {
  browser_core::crash_reports::SetSending(send);
}

void bc_answer_crash_report(bool send) {
  browser_core::crash_reports::Answer(send);
}

void bc_page_favicon(const char* url) {
  if (url) {
    browser_core::FindFavicon(url);
  }
}

void bc_window_suggest(bc_id window_id, const char* keyword, const char* text, bool complete) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  Profile* profile = window ? ProfileOf(*window) : nullptr;
  if (window && profile) {
    window->Suggest(profile, keyword ? base::UTF8ToUTF16(keyword) : std::u16string(),
                    text ? base::UTF8ToUTF16(text) : std::u16string(), complete);
  }
}

void bc_window_remove_suggestion(bc_id window_id, const char* url) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  if (window && url) {
    window->RemoveSuggestion(GURL(url));
  }
}

bc_id bc_tab_open(bc_id window_id, const char* url, bool when_shown) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  Profile* profile = window ? ProfileOf(*window) : nullptr;
  content::WebContents* contents =
      window && profile ? window->OpenTab(profile, GURL(url), when_shown) : nullptr;
  if (!contents) {
    return 0;
  }
  const bc_id tab = browser_core::Tab::IdOf(contents);
  if (browser_core::Space* space = browser_core::Space::FromId(window->space())) {
    space->Add(tab, /*opener=*/0, /*foreground=*/false);
  }
  return tab;
}

// The profile |window_id| shows, else the one the core started with.
static Profile* SearchProfile(bc_id window_id) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  Profile* profile = window ? ProfileOf(*window) : nullptr;
  return profile ? profile : browser_core::StartProfile();
}

size_t bc_typed_url(bc_id window, const char* text, char* url, size_t capacity) {
  Profile* profile = SearchProfile(window);
  const GURL typed = profile && text ? browser_core::TypedUrl(profile, text) : GURL();
  return typed.is_valid() ? CopyString(typed.spec(), url, capacity) : 0;
}

size_t bc_search_url(bc_id window,
                     const char* keyword,
                     const char* text,
                     char* url,
                     size_t capacity) {
  Profile* profile = SearchProfile(window);
  const std::u16string engine = keyword ? base::UTF8ToUTF16(keyword) : std::u16string();
  const GURL search = profile && text ? browser_core::SearchUrl(profile, engine, text) : GURL();
  return search.is_valid() ? CopyString(search.spec(), url, capacity) : 0;
}

bc_id bc_tab_open_preview(bc_id opener_id, const char* url) {
  content::WebContents* opener = browser_core::Tab::FromId(opener_id);
  tabs::TabInterface* tab = opener ? tabs::TabInterface::MaybeGetFromContents(opener) : nullptr;
  browser_core::Window* window =
      tab ? browser_core::Window::ForBrowser(tab->GetBrowserWindowInterface()) : nullptr;
  content::WebContents* preview = window && url ? window->OpenPreview(*opener, GURL(url)) : nullptr;
  return preview ? browser_core::Tab::IdOf(preview) : 0;
}

void bc_tab_show_in(bc_id id, void* view, bc_rect frame, double corner_radius) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  // A page a window shows stays there.
  if (!contents || !view || browser_core::Window::Showing(id)) {
    return;
  }
  browser_core::Window* window = browser_core::Window::Holding(id);
  browser_core::platform::ShowPageIn(window ? window->widget() : nullptr,
                                     contents->GetNativeView(), view, ToRect(frame),
                                     corner_radius);
  contents->Focus();
}

void bc_window_keep_preview(bc_id window_id, bc_id id) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  browser_core::Space* space = window ? browser_core::Space::FromId(window->space()) : nullptr;
  if (!space || !browser_core::Tab::FromId(id) || browser_core::Space::Holding(id) ||
      browser_core::Favorites::Has(id)) {
    return;
  }
  space->Add(id, /*opener=*/0, /*foreground=*/true);
  window->Select(id);
}

void bc_tab_show(bc_id id, bc_rect frame) {
  content::WebContents* contents = browser_core::Window::PageOf(id);
  tabs::TabInterface* tab = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  browser_core::Window* window =
      tab ? browser_core::Window::ForBrowser(tab->GetBrowserWindowInterface()) : nullptr;
  if (window) {
    window->Show(contents, ToRect(frame));
  }
}

void bc_tab_navigate(bc_id id, const char* url) {
  const GURL to(url);
  const browser_core::Tab* tab = browser_core::Tab::Get(id);
  browser_core::Window* window = tab ? browser_core::Window::Holding(id) : nullptr;
  browser_core::Space* space = browser_core::Space::Holding(id);
  // As in Arc, a pinned tab keeps what it was pinned as: another address opens in a tab of its own.
  if (window && space && space->IsPinned(id) && tab->Url() != to) {
    window->OpenFromPinned(id, to);
    return;
  }
  LoadInTab(id, to, ui::PAGE_TRANSITION_TYPED);
}

void bc_tab_back_to_pinned_url(bc_id id) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  const browser_core::Tab* tab = browser_core::Tab::Get(id);
  const GURL url = space ? space->PinnedUrl(id) : browser_core::Favorites::Url(id);
  if (!url.is_valid() || !tab) {
    return;
  }
  // A tab already there doesn't load it again.
  if (tab->Url() != url) {
    LoadInTab(id, url, ui::PAGE_TRANSITION_AUTO_BOOKMARK);
  }
}

void bc_tab_go_back(bc_id id) {
  if (content::WebContents* contents = browser_core::Window::PageOf(id)) {
    browser_core::Tab::FromWebContents(contents)->GoBack();
  }
}

void bc_tab_go_forward(bc_id id) {
  if (content::WebContents* contents = browser_core::Window::PageOf(id)) {
    browser_core::Tab::FromWebContents(contents)->GoForward();
  }
}

void bc_tab_history(bc_id id, bool forward) {
  if (browser_core::Tab* tab = browser_core::Tab::Get(id)) {
    tab->ListHistory(forward);
  }
}

void bc_tab_go_to_history(bc_id id, int offset) {
  if (content::WebContents* contents = browser_core::Window::PageOf(id)) {
    browser_core::Tab::FromWebContents(contents)->GoToOffset(offset);
  }
}

bc_id bc_tab_open_history(bc_id id, int offset) {
  browser_core::Window* window = browser_core::Window::Holding(id);
  return window ? window->OpenHistory(id, offset) : 0;
}

void bc_tab_reload(bc_id id, bool bypass_cache) {
  if (content::WebContents* contents = browser_core::Window::PageOf(id)) {
    browser_core::Tab::FromWebContents(contents)->Reload(bypass_cache);
  }
}

void bc_tab_wait(bc_id id) {
  if (content::WebContents* contents = browser_core::Tab::FromId(id)) {
    browser_core::Tab::FromWebContents(contents)->Wait();
  }
}

void bc_tab_clear_site(bc_id id, bool cookies) {
  browser_core::ClearSiteAndReload(id, cookies);
}

bool bc_system_engine_available(void) {
  return browser_core::system_engine::Available();
}

bool bc_tab_set_system_engine(bc_id id, bool on) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  const GURL url = contents ? browser_core::Tab::PageUrl(contents) : GURL();
  if (!url.SchemeIsHTTPOrHTTPS() || !browser_core::system_engine::Allowed(id)) {
    return false;
  }
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  if (on) {
    if (!browser_core::Tab::FromWebContents(contents)->UseSystemEngine(url)) {
      return false;
    }
    browser_core::system_engine::Remember(profile, url);
    return true;
  }
  // Forgotten first, so Chromium doesn't hand the pages straight back once they go there.
  browser_core::system_engine::Forget(profile, url);
  // Off keeps the site offered in Site Controls, the way back, as the closed note's ✕ does, and
  // keeps the note from asking again about a site the user just turned off.
  browser_core::CloseMediaNote(contents);
  browser_core::system_engine::ReturnToChromium(profile, url);
  return true;
}

void bc_tab_close_media_note(bc_id id) {
  if (content::WebContents* contents = browser_core::Tab::FromId(id)) {
    browser_core::CloseMediaNote(contents);
  }
}

int bc_widevine(void) {
  return browser_core::widevine::State();
}

void bc_widevine_download(void) {
  browser_core::widevine::Download();
}

void bc_widevine_remove(void) {
  browser_core::widevine::Remove();
}

int bc_playready_needs() {
  return browser_core::platform::PlayReadyNeeds();
}

void bc_tab_stop(bc_id id) {
  if (content::WebContents* contents = browser_core::Tab::FromId(id)) {
    browser_core::Tab::FromWebContents(contents)->Stop();
  }
}

size_t bc_tab_find_open(bc_id id, char* text, size_t capacity) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  const std::u16string open =
      contents ? browser_core::FindInPage::GetOrCreateForWebContents(contents)->Open()
               : std::u16string();
  return CopyString(base::UTF16ToUTF8(open), text, capacity);
}

void bc_tab_find(bc_id id, const char* text, bool forward) {
  if (content::WebContents* contents = browser_core::Tab::FromId(id); contents && text) {
    browser_core::FindInPage::GetOrCreateForWebContents(contents)->Find(base::UTF8ToUTF16(text),
                                                                        forward);
  }
}

void bc_tab_find_close(bc_id id) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  if (browser_core::FindInPage* find =
          contents ? browser_core::FindInPage::FromWebContents(contents) : nullptr) {
    find->Close();
  }
}

void bc_tab_element_at(bc_id id, double x, double y) {
  if (content::WebContents* contents = browser_core::Tab::FromId(id)) {
    browser_core::PageCapture::GetOrCreateForWebContents(contents)->ElementAt(gfx::PointF(x, y));
  }
}

void bc_tab_capture(bc_id id, uint64_t request, bool full_page, bc_rect rect) {
  if (content::WebContents* contents = browser_core::Tab::FromId(id)) {
    browser_core::PageCapture::GetOrCreateForWebContents(contents)->Capture(
        request, full_page, gfx::RectF(rect.x, rect.y, rect.width, rect.height));
  }
}

void bc_tab_picture(bc_id id, uint64_t request, int width, int height) {
  if (content::WebContents* contents = browser_core::Tab::FromId(id)) {
    browser_core::PicturePage(contents, request, gfx::Size(std::max(width, 0), std::max(height, 0)));
  }
}

void bc_tab_read_text(bc_id id, uint64_t request) {
  if (content::WebContents* contents = browser_core::Tab::FromId(id)) {
    browser_core::PageText::GetOrCreateForWebContents(contents)->Read(request);
  } else if (browser_core::Host().tab_text) {
    // A tab without its page, or none: nothing to read, told from a task of its own.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](bc_id tab, uint64_t request) {
                         if (browser_core::Host().tab_text) {
                           browser_core::Host().tab_text(browser_core::Host().context, tab,
                                                         request, nullptr);
                         }
                       },
                       id, request));
  }
}

void bc_tab_capture_end(bc_id id) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  if (browser_core::PageCapture* capture =
          contents ? browser_core::PageCapture::FromWebContents(contents) : nullptr) {
    capture->End();
  }
}

void bc_tab_answer_permission(bc_id id, int answer) {
  browser_core::PermissionQuestion::Answer(id, answer);
}

void bc_tab_answer_dialog(bc_id tab,
                          bc_id dialog,
                          bool accept,
                          const char* text,
                          const char* password,
                          bool checked) {
  browser_core::PageDialog::Answer(
      tab, dialog,
      {.accept = accept,
       .text = base::UTF8ToUTF16(text ? text : ""),
       .password = base::UTF8ToUTF16(password ? password : ""),
       .checked = checked});
}

void bc_tab_site_controls(bc_id id) {
  browser_core::SiteControls::Open(id);
}

void bc_site_controls_close(void) {
  browser_core::SiteControls::Close();
}

void bc_tab_cast(bc_id id) {
  browser_core::CastDialog::Open(id);
}

void bc_cast_close(void) {
  browser_core::CastDialog::Close();
}

void bc_tab_cast_to(bc_id id, const char* device) {
  browser_core::CastDialog::To(id, device);
}

void bc_tab_set_site_permission(bc_id id, int type, int value) {
  browser_core::SiteControls::SetPermission(id, type, value);
}

void bc_tab_reset_site_permissions(bc_id id) {
  browser_core::SiteControls::Reset(id);
}

void bc_tab_set_ad_blocking(bc_id id, bool on) {
  browser_core::SiteControls::SetAdBlocking(id, on);
}

size_t bc_ad_blocking_off_sites(bc_id profile_id, char* sites, size_t capacity) {
  std::string lines;
  if (Profile* profile = LoadedProfile(profile_id)) {
    for (const std::string& site : browser_core::AdBlockingOffSites(profile)) {
      lines += (lines.empty() ? "" : "\n") + site;
    }
  }
  return CopyString(lines, sites, capacity);
}

void bc_ad_blocking_turn_on_site(bc_id profile_id, const char* site) {
  if (Profile* profile = LoadedProfile(profile_id); profile && site) {
    browser_core::TurnOnAdBlockingOnSite(profile, site);
  }
}

void bc_tab_set_developer_mode(bc_id id, bool on) {
  if (browser_core::Tab* tab = browser_core::Tab::Get(id)) {
    browser_core::developer_mode::Set(tab->profile(), tab->Url(), on);
  }
}

void bc_tab_picture_in_picture(bc_id id) {
  browser_core::SiteControls::PictureInPicture(id);
}

void bc_tab_show_certificate(bc_id id) {
  browser_core::SiteControls::ShowCertificate(id);
}

void bc_tab_answer_offer(bc_id id, int answer, const char* username) {
  if (browser_core::AutofillOffer::Answer(id, answer)) {
    return;
  }
  browser_core::PasswordOffer::Answer(id, answer, username);
}

void bc_tab_show_offer(bc_id id) {
  browser_core::PasswordOffer::ShowAgain(id);
}

void bc_tab_close_page_menu(bc_id id, int command) {
  browser_core::PageMenu::Close(id, command);
}

void bc_profile_read_logins(bc_id profile) {
  browser_core::Passwords::Read(profile);
}

void bc_profile_remove_login(bc_id profile, const char* key, const char* username, bool never) {
  browser_core::Passwords::Remove(profile, key ? key : "", username ? username : "", never);
}

void bc_profile_read_autofill(bc_id profile) {
  browser_core::autofill_data::Read(profile);
}

void bc_profile_remove_autofill(bc_id profile, const char* key) {
  browser_core::autofill_data::Remove(profile, key ? key : "");
}

bool bc_profile_import_passwords(bc_id profile, const char* path) {
  return path && browser_core::Passwords::Import(profile, base::FilePath::FromUTF8Unsafe(path));
}

void bc_tab_jump_to_selection(bc_id id) {
  if (content::WebContents* contents = browser_core::Tab::FromId(id)) {
    contents->CenterSelection();
  }
}

void bc_tab_set_pinned(bc_id id, bool pinned) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  if (!space || !browser_core::Tab::Get(id)) {
    return;
  }
  if (pinned) {
    space->Pin(id, &PageUrlOf);
  } else {
    space->Unpin(id);
  }
  SplitMoved(*space, id);
}

void bc_tab_move_to_favorites(bc_id id, size_t index) {
  browser_core::Window::MoveToFavorites(id, index);
}

void bc_tab_move(bc_id id, bool pinned, bc_id folder, size_t index) {
  if (browser_core::Favorites::Has(id)) {
    browser_core::Window::Unfavorite(id, pinned, folder, index);
    return;
  }
  browser_core::Space* space = browser_core::Space::Holding(id);
  if (!space || !browser_core::Tab::Get(id)) {
    return;
  }
  space->Move(id, pinned, folder, index, &PageUrlOf);
  SplitMoved(*space, id);
}

int bc_tab_zoom(bc_id id, int step) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  zoom::ZoomController* zoom = contents ? zoom::ZoomController::FromWebContents(contents) : nullptr;
  if (!zoom) {
    return 100;
  }
  zoom::PageZoom::Zoom(contents, static_cast<content::PageZoom>(std::clamp(step, -1, 1)));
  return zoom->GetZoomPercent();
}

void bc_tab_devtools(bc_id id, int what) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  tabs::TabInterface* tab = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  if (!tab) {
    return;
  }
  // As Chrome's ⌥⌘I, ⌥⌘J and ⌥⌘C, on the Browser's active tab, which is the one its window shows.
  switch (what) {
    case BC_DEVTOOLS_CONSOLE:
      DevToolsWindow::ToggleDevToolsWindow(tab->GetBrowserWindowInterface(),
                                           DevToolsToggleAction::ShowConsolePanel(),
                                           DevToolsOpenedByAction::kConsoleShortcut);
      break;
    case BC_DEVTOOLS_INSPECT:
      DevToolsWindow::ToggleDevToolsWindow(tab->GetBrowserWindowInterface(),
                                           DevToolsToggleAction::Inspect(),
                                           DevToolsOpenedByAction::kInspectorModeShortcut);
      break;
    default:
      DevToolsWindow::ToggleDevToolsWindow(tab->GetBrowserWindowInterface(),
                                           DevToolsToggleAction::Toggle(),
                                           DevToolsOpenedByAction::kToggleShortcut);
  }
}

void bc_tab_view_source(bc_id id) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  if (contents && contents->GetController().CanViewSource()) {
    // Content opens it as a tab its page opened, which goes under it.
    contents->GetPrimaryMainFrame()->ViewSource();
  }
}

void bc_tab_print(bc_id id) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  tabs::TabInterface* tab = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  // Chrome's command prints its Browser's active tab, which is the one its window shows.
  if (tab && tab->IsActivated() && chrome::CanPrint(tab->GetBrowserWindowInterface())) {
    chrome::Print(tab->GetBrowserWindowInterface());
  }
}

void bc_tab_save_page(bc_id id) {
  content::WebContents* contents = browser_core::Tab::FromId(id);
  tabs::TabInterface* tab = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  if (tab && tab->IsActivated() && chrome::CanSavePage(tab->GetBrowserWindowInterface())) {
    chrome::SavePage(tab->GetBrowserWindowInterface());
  }
}

void bc_window_task_manager(bc_id window_id) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  content::WebContents* contents = window ? browser_core::Tab::FromId(window->selected()) : nullptr;
  tabs::TabInterface* tab = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  // Chrome's selects its Browser's active tab, the one the window shows.
  chrome::OpenTaskManager(tab ? tab->GetBrowserWindowInterface() : nullptr);
}

bc_id bc_tab_duplicate(bc_id id) {
  browser_core::Window* window = browser_core::Window::Holding(id);
  return window ? window->Duplicate(id) : 0;
}

void bc_download_act(bc_id download, int action) {
  browser_core::ActOnDownload(download, action);
}

void bc_downloads_clear() {
  browser_core::ClearDownloads();
}

void bc_download_icon(bc_id download) {
  browser_core::FindDownloadIcon(download);
}

size_t bc_tab_name(bc_id id, char* name, size_t capacity) {
  const browser_core::Tab* tab = browser_core::Tab::Get(id);
  return CopyString(tab ? tab->name() : std::string(), name, capacity);
}

size_t bc_tab_pinned_url(bc_id id, char* url, size_t capacity) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  const GURL pinned = space ? space->PinnedUrl(id) : browser_core::Favorites::Url(id);
  return CopyString(pinned.spec(), url, capacity);
}

void bc_tab_set_pinned_url(bc_id id, const char* url) {
  browser_core::Tab* tab = browser_core::Tab::Get(id);
  if (!url || !tab) {
    return;
  }
  if (browser_core::Space* space = browser_core::Space::Holding(id)) {
    space->SetPinnedUrl(id, GURL(url));
  } else {
    browser_core::Favorites::SetUrl(id, GURL(url));
  }
  // Whether the page is away from it may have changed.
  tab->StateChanged();
}

void bc_tab_rename(bc_id id, const char* name) {
  if (browser_core::Tab* tab = browser_core::Tab::Get(id)) {
    tab->Rename(name);
  }
}

void bc_tab_set_icon(bc_id id, const char* icon) {
  if (browser_core::Tab* tab = browser_core::Tab::Get(id)) {
    tab->SetIcon(icon ? icon : "");
  }
}

void bc_tab_set_muted(bc_id id, bool muted) {
  if (browser_core::Tab* tab = browser_core::Tab::Get(id)) {
    tab->SetMuted(muted);
  }
}

void bc_tab_share_instead(bc_id id) {
  if (browser_core::Tab* tab = browser_core::Tab::Get(id)) {
    tab->ShareInstead();
  }
}

void bc_tab_media(bc_id id, int action) {
  if (browser_core::Tab* tab = browser_core::Tab::Get(id)) {
    tab->Media(action);
  }
}

void bc_tab_close_below(bc_id id) {
  browser_core::Window::CloseBelow(id);
}

void bc_tab_move_to_space(bc_id id, bc_id space) {
  browser_core::Space* from = browser_core::Space::Holding(id);
  const bool agents = from && from->agent();
  browser_core::Window::MoveToSpace(id, space);
  browser_core::Space* to = browser_core::Space::Holding(id);
  if (agents && to && !to->agent()) {
    browser_core::FillForUser(id);
  }
}

void bc_tab_close(bc_id id, bool split) {
  browser_core::Space* space = browser_core::Space::Holding(id);
  if ((space && space->IsPinned(id)) || browser_core::Favorites::Has(id)) {
    browser_core::Window::Unload(id, split);
    return;
  }
  // Each closes at once unless its page keeps it, and the window shows what its Space picks after
  // the last.
  const std::vector<bc_id> tabs = space && split ? space->Panes(id) : std::vector<bc_id>{id};
  for (bc_id tab : tabs) {
    browser_core::Window::CloseTab(tab);
  }
}

size_t bc_windows(bc_id* windows, size_t capacity) {
  // Private windows are the user's alone.
  std::vector<browser_core::Window*> all = browser_core::Window::All();
  std::erase_if(all, [](const browser_core::Window* each) { return each->own_space(); });
  for (size_t i = 0; i < all.size() && i < capacity && windows; ++i) {
    windows[i] = all[i]->id();
  }
  return all.size();
}

bc_id bc_window_space(bc_id window_id) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->space() : 0;
}

bc_id bc_window_selected(bc_id window_id) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->selected() : 0;
}

bc_id bc_tab_window(bc_id tab) {
  browser_core::Window* window = browser_core::Window::Showing(tab);
  return window ? window->id() : 0;
}

bc_id bc_window_open_private(bc_rect bounds, void* ui_view, bc_id profile_id) {
  Profile* profile = LoadedProfile(profile_id);
  browser_core::Window* window =
      profile ? browser_core::Window::OpenOwn(ToRect(bounds), ui_view, profile,
                                              browser_core::Space::Own::kPrivate)
              : nullptr;
  return window ? window->id() : 0;
}

bc_id bc_window_open_little(bc_rect bounds, void* ui_view, bc_id profile_id) {
  Profile* profile = LoadedProfile(profile_id);
  browser_core::Window* window =
      profile ? browser_core::Window::OpenOwn(ToRect(bounds), ui_view, profile,
                                              browser_core::Space::Own::kLittle)
              : nullptr;
  return window ? window->id() : 0;
}

bc_id bc_window_take_tab(bc_id window_id, bc_id tab) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window ? window->TakeTab(tab) : 0;
}

void bc_window_set_side_panel_frame(bc_id window_id, bc_rect frame) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->SetSidePanelFrame(ToRect(frame));
  }
}

void bc_window_close_side_panel(bc_id window_id) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->CloseSidePanel();
  }
}

bool bc_window_has_side_panel(bc_id window_id, const char* id) {
  browser_core::Window* window = browser_core::Window::FromId(window_id);
  return window && id && window->HasSidePanel(id);
}

void bc_window_toggle_side_panel(bc_id window_id, const char* id) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id); window && id) {
    window->ToggleSidePanel(id);
  }
}

void bc_window_set_side_panel_width(bc_id window_id, double width) {
  if (browser_core::Window* window = browser_core::Window::FromId(window_id)) {
    window->SetSidePanelWidth(base::ClampRound(width));
  }
}

void bc_window_start_move(bc_id id) {
#if BUILDFLAG(IS_LINUX)
  if (views::Widget* widget = OpenWidget(id)) {
    browser_core::platform::StartMove(widget);
  }
#endif
}

bool bc_window_is_maximized(bc_id id) {
#if BUILDFLAG(IS_LINUX)
  views::Widget* widget = OpenWidget(id);
  return widget && widget->IsMaximized();
#else
  return false;
#endif
}

bool bc_window_is_full_screen(bc_id id) {
#if BUILDFLAG(IS_LINUX) || BUILDFLAG(IS_WIN)
  views::Widget* widget = OpenWidget(id);
  return widget && widget->IsFullscreen();
#else
  return false;
#endif
}

void bc_window_toggle_full_screen(bc_id id) {
#if BUILDFLAG(IS_LINUX) || BUILDFLAG(IS_WIN)
  views::Widget* widget = OpenWidget(id);
  if (!widget) {
    return;
  }
  if (widget->IsFullscreen()) {
    browser_core::Window::FromId(id)->LeaveFullScreen();
  } else {
    widget->SetFullscreen(true);
  }
#endif
}

bool bc_sync_set_server(const char* server) {
  return browser_core::sync::SetServer(server ? server : "");
}

void bc_sync_sign_in(void) {
  browser_core::sync::SignIn();
}

void bc_sync_cancel(void) {
  browser_core::sync::Cancel();
}

void bc_sync_sign_out(void) {
  browser_core::sync::SignOut();
}

size_t bc_sync_recovery_words(char* words, size_t capacity) {
  return CopyString(browser_core::sync::RecoveryWords(), words, capacity);
}

bool bc_sync_enter_recovery_words(const char* words) {
  return words && browser_core::sync::EnterRecoveryWords(words);
}

void bc_sync_start_over(void) {
  browser_core::sync::StartOver();
}

void bc_sync_request_pairing(void) {
  browser_core::sync::RequestPairing();
}

void bc_sync_answer_pairing(const char* id, bool add) {
  if (id) {
    browser_core::sync::AnswerPairing(id, add);
  }
}

void bc_sync_read_devices(void) {
  browser_core::sync::ReadDevices();
}

void bc_sync_remove_device(const char* id) {
  if (id) {
    browser_core::sync::RemoveDevice(id);
  }
}
