#include "browser_core/window.h"

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <iterator>
#include <map>
#include <numeric>
#include <optional>
#include <set>
#include <utility>
#include <vector>

#include "base/auto_reset.h"
#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/logging.h"
#include "base/memory/ptr_util.h"
#include "base/no_destructor.h"
#include "base/numerics/safe_conversions.h"
#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/agent_session.h"
#include "browser_core/archive.h"
#include "browser_core/auto_archive.h"
#include "browser_core/browser_window_stand_in.h"
#include "browser_core/dialog_keys.h"
#include "browser_core/extensions.h"
#include "browser_core/favorites.h"
#include "browser_core/host.h"
#include "browser_core/idle_unload.h"
#include "browser_core/install_question.h"
#include "browser_core/mini_player.h"
#include "browser_core/password_offer.h"
#include "browser_core/platform/window_views.h"
#include "browser_core/profiles.h"
#include "browser_core/session.h"
#include "browser_core/side_panel.h"
#include "browser_core/space.h"
#include "browser_core/suggest.h"
#include "browser_core/sync.h"
#include "browser_core/tab.h"
#include "browser_core/window_frames.h"
#include "build/build_config.h"
#include "chrome/app/chrome_command_ids.h"
#include "chrome/browser/devtools/devtools_contents_resizing_strategy.h"
#include "chrome/browser/devtools/devtools_window.h"
#include "chrome/browser/extensions/api/side_panel/side_panel_service.h"
#include "chrome/browser/extensions/extension_action_runner.h"
#include "chrome/browser/lifetime/application_lifetime_desktop.h"
#include "chrome/browser/lifetime/browser_shutdown.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "chrome/browser/profiles/profiles_state.h"
#include "chrome/browser/renderer_host/chrome_navigation_ui_data.h"
#include "chrome/browser/tab_contents/tab_util.h"
#include "chrome/browser/tab_list/tab_removed_reason.h"
#include "chrome/browser/ui/browser_active_state_manager/browser_active_state_manager.h"
#include "chrome/browser/ui/browser_tabrestore.h"
#include "chrome/browser/ui/browser_window/public/browser_window_interface.h"
#include "chrome/browser/ui/browser_window/public/create_browser_window.h"
#include "chrome/browser/ui/browser_window/public/desktop_browser_window_capabilities.h"
#include "chrome/browser/ui/exclusive_access/exclusive_access_manager.h"
#include "chrome/browser/ui/exclusive_access/fullscreen_controller.h"
#include "chrome/browser/ui/extensions/extension_side_panel_utils.h"
#include "chrome/browser/ui/interaction/browser_elements.h"
#include "chrome/browser/ui/navigator/browser_navigator.h"
#include "chrome/browser/ui/navigator/browser_navigator_params.h"
#include "chrome/browser/ui/navigator/browser_navigator_params_utils.h"
#include "chrome/browser/ui/tabs/tab_enums.h"
#include "chrome/browser/ui/tabs/tab_model.h"
#include "chrome/browser/ui/tabs/tab_strip_model.h"
#include "chrome/browser/ui/views/interaction/browser_elements_views_impl.h"
#include "chrome/common/webui_url_constants.h"
#include "components/sessions/content/session_tab_helper.h"
#include "components/sessions/core/serialized_navigation_entry.h"
#include "components/sessions/core/serialized_user_agent_override.h"
#include "components/tabs/public/tab_interface.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_user_data.h"
#include "content/public/common/referrer.h"
#include "content/public/common/url_constants.h"
#include "extensions/browser/extension_registry.h"
#include "extensions/common/extension.h"
#include "extensions/common/mojom/api_permission_id.mojom-shared.h"
#include "extensions/common/permissions/permissions_data.h"
#include "ui/base/page_transition_types.h"
#include "ui/base/window_open_disposition.h"
#include "ui/gfx/codec/png_codec.h"
#include "ui/gfx/image/image_skia.h"
#include "ui/gfx/image/image_skia_rep.h"
#include "ui/menus/simple_menu_model.h"
#include "ui/views/view.h"
#include "ui/views/view_tracker.h"
#include "ui/views/widget/widget.h"
#include "ui/views/widget/widget_delegate.h"
#include "ui/views/window/client_view.h"

#if BUILDFLAG(IS_LINUX)
#include "browser_core/platform/linux/wayland.h"
#endif

namespace browser_core {

namespace {

// Whether |contents| is a tab in a Browser's tab strip. One Chrome is taking out isn't, from
// within its change to the tabs, where Chrome tells its observers once the tab has left.
bool InTabStrip(content::WebContents* contents) {
  tabs::TabInterface* tab = tabs::TabInterface::MaybeGetFromContents(contents);
  return tab && tab->GetBrowserWindowInterface();
}

std::map<bc_id, std::unique_ptr<Window>>& Windows() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<Window>>> windows;
  return *windows;
}

// The windows' ids, the most recently active first.
std::vector<bc_id>& Order() {
  static base::NoDestructor<std::vector<bc_id>> order;
  return *order;
}

// Moves |id| to the front of Order, and says whether it wasn't there.
bool ToFront(bc_id id) {
  if (!Order().empty() && Order().front() == id) {
    return false;
  }
  std::erase(Order(), id);
  Order().insert(Order().begin(), id);
  return true;
}

// A Today tab the user or its page closed.
struct ClosedTab {
  // The id it had.
  bc_id id = 0;
  bc_id space = 0;
  // Its place in the Space's Today tabs.
  size_t index = 0;
  SavedTab saved;
  // A tab of the split view it was in, else 0, its place in the split, and whether the split was
  // stacked, with its name and icon.
  bc_id split_with = 0;
  size_t split_index = 0;
  bool stacked = false;
  std::string split_name;
  std::string split_icon;
  // Whether it closed in a Little ilo window, which ⇧⌘T reopens it in a new one of, in the profile
  // in the directory |profile| (window_reopen_little).
  bool little = false;
  std::string profile;
};

// As many as Chrome keeps.
constexpr size_t kMaxClosedTabs = 25;

// How many Clears a window can undo.
constexpr size_t kMaxClears = 10;

// Between the pages of a split view, as in Arc.
constexpr int kSplitGap = 10;

// What Expand Current Split leaves each of the other pages of a split view, as Arc's.
constexpr int kExpandedOther = 220;

// The shares of |count| pages across |length|, with the |focused| one taking what the others
// don't: kExpandedOther each, no more than an even share, so it's never the smaller one on a card
// too short for both. The app works out the same in expanded_shares (app/src/shell/panes.rs),
// since it draws the split's gaps and rings.
std::vector<double> ExpandedShares(size_t count, size_t focused, int length) {
  const double span = length + kSplitGap;
  const double other = std::min<double>(kExpandedOther + kSplitGap, span / count) / span;
  std::vector<double> shares(count, other);
  shares[focused] = 1 - other * (count - 1);
  return shares;
}

// What shows with |tab|, in its order: its split's tabs, a Space's or a favorite's, else |tab|.
std::vector<bc_id> PanesWith(bc_id tab) {
  if (const Space* space = Space::Holding(tab)) {
    return space->Panes(tab);
  }
  if (std::vector<bc_id> panes = Favorites::Panes(tab); !panes.empty()) {
    return panes;
  }
  return {tab};
}

// How |tab|'s split shows its pages: stacked, each page's share, and the page that took its card.
struct SplitLook {
  bool stacked = false;
  std::vector<double> shares;
  bc_id expanded = 0;
};

SplitLook LookOf(bc_id tab) {
  if (const Space* space = Space::Holding(tab)) {
    return {space->Stacked(tab), space->Shares(tab), space->Expanded(tab)};
  }
  if (const SplitView* split = Favorites::SplitOf(tab)) {
    return {split->stacked, split->shares, split->expanded};
  }
  return {};
}

// The Spaces going once their last tab closes.
std::set<bc_id>& DeletingSpaces() {
  static base::NoDestructor<std::set<bc_id>> deleting;
  return *deleting;
}

// Newest last.
std::vector<ClosedTab>& ClosedTabs() {
  static base::NoDestructor<std::vector<ClosedTab>> closed;
  return *closed;
}

// A Space deleted, for UndoDeleteSpace.
struct DeletedSpace {
  bc_id id = 0;
  SavedSpace saved;
  // The Today tabs saved, which went to the archive.
  std::vector<bc_id> today;
  // The tab or favorite the window showed there, or 0.
  bc_id shown = 0;
  // Its place among the Spaces, which it comes back to.
  size_t at = 0;
};

// The Spaces deleted, newest last, as many as the closed tabs.
std::vector<DeletedSpace>& DeletedSpaces() {
  static base::NoDestructor<std::vector<DeletedSpace>> deleted;
  return *deleted;
}

// The pages Window::ForgetPage named, by their tab, until it closes.
std::map<bc_id, GURL>& ForgottenPages() {
  static base::NoDestructor<std::map<bc_id, GURL>> forgotten;
  return *forgotten;
}

// Keeps |id|, a tab closing, for ReopenClosedTab, and archives it.
void KeepClosed(bc_id id) {
  Tab* tab = Tab::Get(id);
  // A sign-in's callback page, whose address carries a single-use code, leaves nothing to reopen
  // (#2527); a tab that has gone on from it closes as any does.
  if (auto forgotten = ForgottenPages().extract(id);
      forgotten && tab && tab->Url() == forgotten.mapped()) {
    return;
  }
  Space* space = Space::Holding(id);
  // One not in a Space never showed, a pinned tab closes only when its page closes itself, and a
  // private window's leaves nothing behind. An agent's pages aren't the user's to find again
  // (#1408). A Little ilo window's goes in the archive, as Arc's Little Arc's do, and ⇧⌘T reopens
  // it in another.
  if (!tab || !space || space->IsPinned(id) || space->off_the_record() || space->agent()) {
    return;
  }
  SavedTab saved = tab->Save();
  if (saved.history.empty()) {
    return;
  }
  // A Blank Window's goes in the archive, so nothing closed is lost, but not in the closed tabs
  // ⇧⌘T reopens, whose Space went with its window.
  if (space->blank()) {
    Archive(id, saved.history[saved.current].virtual_url(), tab->Title(), space->node().id,
            /*little=*/false);
    return;
  }
  Archive(id, saved.history[saved.current].virtual_url(), tab->Title(), space->node().id,
          space->little());
  const size_t index = std::ranges::find(space->today(), id) - space->today().begin();
  const std::vector<bc_id> panes = space->Panes(id);
  const size_t split_index = std::ranges::find(panes, id) - panes.begin();
  ClosedTabs().push_back({.id = id,
                          .space = space->id(),
                          .index = index,
                          .saved = std::move(saved),
                          .split_with = panes.size() < 2 ? 0 : panes[split_index == 0 ? 1 : 0],
                          .split_index = split_index,
                          .stacked = space->Stacked(id),
                          .split_name = space->SplitName(id),
                          .split_icon = space->SplitIcon(id),
                          .little = space->little(),
                          .profile = space->profile()});
  if (ClosedTabs().size() > kMaxClosedTabs) {
    ClosedTabs().erase(ClosedTabs().begin());
  }
}

// The closed tab |id|, taken from those ReopenClosedTab brings back, and from the archive.
std::optional<ClosedTab> TakeClosed(bc_id id) {
  auto closed = std::ranges::find(ClosedTabs(), id, &ClosedTab::id);
  if (closed == ClosedTabs().end()) {
    return std::nullopt;
  }
  ClosedTab tab = std::move(*closed);
  ClosedTabs().erase(closed);
  Unarchive(id);
  return tab;
}

// Gives the split |tab| is back in what it had when |closed| closed: stacked, a name, an icon.
void RemakeSplit(Space& space, bc_id tab, const ClosedTab& closed) {
  if (closed.stacked) {
    space.SetStacked(tab, true);
  }
  if (!closed.split_name.empty()) {
    space.SetSplitName(tab, closed.split_name);
  }
  if (!closed.split_icon.empty()) {
    space.SetSplitIcon(tab, closed.split_icon);
  }
}

// A new tab in |dir|'s profile with |tab|'s history, name and mute, in no Space and without its
// page, or 0 if the profile isn't loaded.
bc_id CopyToProfile(bc_id tab, const std::string& dir) {
  const Tab* from = Tab::Get(tab);
  Profile* profile = ProfileFor(dir);
  if (!from || !profile || DirOf(profile) != dir) {
    return 0;
  }
  return Tab::Restore(profile, from->Save());
}

// Whether |tab| is a private window's, in an off-the-record profile.
bool OffTheRecord(const Tab* tab) {
  return tab && tab->profile() && tab->profile()->IsOffTheRecord();
}

// |tab|, leaving a window, was seen until now.
void Seen(bc_id tab) {
  if (Tab* seen = Tab::Get(tab)) {
    seen->set_last_seen(base::Time::Now());
    seen->LeftWindow();
  }
  ScheduleAutoArchive();
}

// Has |contents|'s page, and its docked DevTools, stretch the frame they last drew to fill their
// views as these resize, or not. Chrome shows that frame at its old size in the top-left corner
// until the renderer draws at the new one, which moves a page's right edge and scrollbar with the
// edge the app moves (issue #166). Not for a page full screen: the window resizes that.
void StretchOnResize(content::WebContents* contents, bool stretch) {
  stretch = stretch && !contents->IsFullscreen();
  for (content::WebContents* each :
       {contents, DevToolsWindow::GetInTabWebContents(contents, nullptr)}) {
    if (each) {
      platform::StretchOnResize(each->GetRenderWidgetHostView(), stretch);
    }
  }
}

// The pages on their way between windows, each held as captured, visibly, from when a window lets
// go of it until the window with its picture has taken it (AdoptFreedPages). Chrome hides a page
// out of every window, which has its renderer let go of what it drew, so a heavy page would draw
// again from nothing in the window taking it, and show nothing until it had.
std::map<bc_id, base::ScopedClosureRunner>& MovingPages() {
  static base::NoDestructor<std::map<bc_id, base::ScopedClosureRunner>> moving;
  return *moving;
}

void HoldWhileMoving(content::WebContents* contents) {
  if (!contents || MovingPages().contains(Tab::IdOf(contents))) {
    return;
  }
  MovingPages().emplace(Tab::IdOf(contents),
                        contents->IncrementCapturerCount(gfx::Size(), /*stay_hidden=*/false,
                                                         /*stay_awake=*/false,
                                                         /*is_activity=*/false));
}

// When the app last asked a window to select a tab.
base::TimeTicks g_handover_click;
// The tab whose page Adopt last moved, until the window taking it has shown it, and the size of the
// pane it had in the window it left, for the handover lines.
bc_id g_adopted = 0;
gfx::Size g_left_size;

std::string Handing(bc_id window, bc_id tab) {
  return base::StrCat(
      {"window ", base::NumberToString(window), ", tab ", base::NumberToString(tab)});
}

// What a tab MakeHeldBy made loads, and the Space holding it, kept on its page until its tab strip
// has it.
class HeldLoad : public content::WebContentsUserData<HeldLoad> {
 public:
  // Starts the page of |contents|, held by its Space first, if MakeHeldBy made it and it hasn't
  // started. Chrome's tab helpers are on it by now: its tab strip took it, or Navigate, which
  // inserts it, has returned.
  static void Start(content::WebContents* contents) {
    HeldLoad* held = contents ? FromWebContents(contents) : nullptr;
    if (!held) {
      return;
    }
    const bc_id space_id = held->space_;
    content::NavigationController::LoadURLParams load = std::move(held->load_);
    const bool https_default = held->https_default_;
    const bool no_https_upgrade = held->no_https_upgrade_;
    const bool from_sync = held->from_sync_;
    contents->RemoveUserData(UserDataKey());
    // A Space gone before its tab had a page leaves the tab without one.
    Space* space = Space::FromId(space_id);
    if (!space) {
      return;
    }
    const bc_id tab = Tab::IdOf(contents);
    if (!Space::Holding(tab)) {
      space->Expect(tab);
    }
    // As LoadURLParamsFromNavigateParams gives it, from the helpers now on the page.
    if (load.frame_tree_node_id.is_null()) {
      std::unique_ptr<ChromeNavigationUIData> ui_data =
          ChromeNavigationUIData::CreateForMainFrameNavigation(contents, https_default,
                                                               no_https_upgrade);
      ui_data->set_navigation_initiated_from_sync(from_sync);
      load.navigation_ui_data = std::move(ui_data);
    }
    contents->GetController().LoadURLWithParams(load);
  }

 private:
  friend class content::WebContentsUserData<HeldLoad>;

  HeldLoad(content::WebContents* contents, bc_id space, NavigateParams& params)
      : content::WebContentsUserData<HeldLoad>(*contents),
        space_(space),
        load_(LoadURLParamsFromNavigateParams(&params)),
        https_default_(params.is_using_https_as_default_scheme),
        no_https_upgrade_(params.url_typed_with_http_scheme ||
                          params.captive_portal_window_type !=
                              captive_portal::CaptivePortalWindowType::kNone),
        from_sync_(params.navigation_initiated_from_sync) {}

  const bc_id space_;
  content::NavigationController::LoadURLParams load_;
  const bool https_default_;
  const bool no_https_upgrade_;
  const bool from_sync_;

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(HeldLoad);

// Makes the new tab |params| asks for as Chrome's Navigate makes it (CreateTargetContents), for
// |space| to hold from when its tab strip has it until the window places it, and leaves it to
// Navigate to insert. Its page starts once inserted (HeldLoad::Start), as Chrome's Navigate starts
// a tab's page once it has made the tab and its helpers. Chrome's Navigate would start it before
// the tab strip had the tab, and the window places a tab a task after that, so a tab an agent
// opens would be in no Space as its first request starts (#2660).
void MakeHeldBy(Space& space, NavigateParams& params) {
  Profile* profile = params.browser->GetProfile();
  scoped_refptr<content::SiteInstance> site =
      params.opener ? scoped_refptr<content::SiteInstance>(params.opener->GetSiteInstance())
                    : tab_util::GetSiteInstanceForNewTab(profile, params.url);
  content::WebContents::CreateParams create(profile, std::move(site));
  create.main_frame_name = params.frame_name;
  if (params.opener) {
    create.opener_id = params.opener->GetGlobalId();
  }
  create.opened_by_another_window = params.opened_by_another_window;
  create.initially_hidden = params.disposition == WindowOpenDisposition::NEW_BACKGROUND_TAB;
#if defined(USE_AURA)
  // In the window's compositor, which draws its page out of sight while an agent acts on it, as
  // Chrome's tabs are. A page in none draws no frame, and its input waits for one.
  if (params.browser->GetWindow() && params.browser->GetWindow()->GetNativeWindow()) {
    create.context = params.browser->GetWindow()->GetNativeWindow();
  }
#endif
  std::unique_ptr<content::WebContents> made = content::WebContents::Create(create);
  HeldLoad::CreateForWebContents(made.get(), space.id(), params);
  // A tab strip no window watches still loads it, once Navigate has inserted it.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](base::WeakPtr<content::WebContents> contents) {
                       HeldLoad::Start(contents.get());
                     },
                     made->GetWeakPtr()));
  params.contents_to_insert = std::move(made);
}

}  // namespace

void HandoverClicked() {
  g_handover_click = base::TimeTicks::Now();
}

void LogHandover(const char* stage, const std::string& detail) {
  const base::TimeDelta since = base::TimeTicks::Now() - g_handover_click;
  if (g_handover_click.is_null() || since > base::Seconds(3)) {
    return;
  }
  fprintf(stderr, "browser: handover %s +%.1f ms (%s)\n", stage, since.InMillisecondsF(),
          detail.c_str());
}

// Makes the window's close button Window::Close, so pages get their unload handlers first.
class Window::ClientView : public views::ClientView {
 public:
  ClientView(views::Widget* widget, views::View* contents, base::WeakPtr<Window> window)
      : views::ClientView(widget, contents), window_(std::move(window)) {}

  views::CloseRequestResult OnWindowCloseRequested() override {
    // Posted, since a window without Browsers closes its widget at once, and the widget is asking.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Window::Close, window_));
    return views::CloseRequestResult::kCannotClose;
  }

 private:
  base::WeakPtr<Window> window_;
};

// static
Window* Window::Open(const gfx::Rect& bounds, void* ui_view, bool behind) {
  if (browser_shutdown::IsTryingToQuit()) {
    return nullptr;
  }
  const bc_id id = NewId();
  Window* window = new Window(id, bounds, ui_view, behind);
  Windows()[id] = base::WrapUnique(window);
  // It opens active, unless behind.
  if (behind) {
    Order().push_back(id);
  } else {
    ToFront(id);
  }
  return window;
}

// static
Window* Window::OpenOwn(const gfx::Rect& bounds, void* ui_view, Profile* profile, Space::Own own) {
  Window* window = Open(bounds, ui_view);
  if (!window) {
    return nullptr;
  }
  window->own_space_ = true;
  if (own == Space::Own::kPrivate) {
    window->off_the_record_ = true;
    // Its Browser now, which holds the private session for as long as the window is open.
    window->BrowserFor(StartPrivateSession(profile));
  }
  window->ShowSpace(Space::CreateOwn(DirOf(profile), own)->id(), /*page=*/false);
  return window;
}

// static
Window* Window::FromId(bc_id id) {
  auto it = Windows().find(id);
  return it == Windows().end() ? nullptr : it->second.get();
}

// static
std::vector<Window*> Window::All() {
  std::vector<Window*> all;
  for (bc_id id : Order()) {
    if (Window* window = FromId(id)) {
      all.push_back(window);
    }
  }
  return all;
}

// static
Window* Window::ForBrowser(const BrowserWindowInterface* browser) {
  for (auto& [id, window] : Windows()) {
    for (auto& [profile, held] : window->browsers_) {
      if (held.browser == browser) {
        return window.get();
      }
    }
  }
  return nullptr;
}

// static
void Window::CloseAll() {
  while (!Windows().empty()) {
    Windows().begin()->second->Destroy();
  }
}

// static
Window* Window::Showing(bc_id tab) {
  for (auto& [id, window] : Windows()) {
    if (std::ranges::contains(window->panes_, tab)) {
      return window.get();
    }
  }
  return nullptr;
}

// static
void Window::MiniPlayerTookKeyboard() {
  for (auto& [id, window] : Windows()) {
    window->left_ = false;
  }
}

// static
void Window::PageViewChanged(bc_id tab) {
  for (Window* window : All()) {
    if (std::ranges::contains(window->panes_, tab)) {
      window->LayOutPage();
      if (window->selected_ == tab) {
        window->FocusPageSoon();
      }
    }
  }
}

// static
Window* Window::Picturing(bc_id tab) {
  for (Window* window : All()) {
    if (!window->let_go_ || window->closing_ || !window->selected_) {
      continue;
    }
    // A split view's pages move together, so the picture of one stands for them all.
    if (std::ranges::contains(PanesWith(window->selected_), tab)) {
      return window;
    }
  }
  return nullptr;
}

void Window::HoldPictured() {
  for (bc_id pane : panes_) {
    if (Picturing(pane)) {
      HoldWhileMoving(Tab::FromId(pane));
    }
  }
}

// static
void Window::AdoptFreedPages() {
  // A page left as the browser quits isn't shown again.
  if (!browser_shutdown::IsTryingToQuit()) {
    for (Window* window : All()) {
      if (Picturing(window->selected_) == window && !Showing(window->selected_)) {
        window->Select(window->selected_);
      }
    }
  }
  // Each page held is in a window again, or in none, which hides it now. Let go of once out of the
  // map, as their visibility changing may come back here.
  std::map<bc_id, base::ScopedClosureRunner> moved;
  moved.swap(MovingPages());
}

// static
Window* Window::Holding(bc_id tab) {
  const Tab* held = Tab::Get(tab);
  if (!held) {
    return nullptr;
  }
  if (content::WebContents* contents = held->web_contents()) {
    tabs::TabInterface* in = tabs::TabInterface::MaybeGetFromContents(contents);
    return in ? ForBrowser(in->GetBrowserWindowInterface()) : nullptr;
  }
  const Space* space = Space::Holding(tab);
  for (Window* window : All()) {
    if (window->closing_ || window->off_the_record_ != OffTheRecord(held) ||
        window->own_space_ != (space && space->own())) {
      continue;
    }
    if (!window->own_space_ || space->id() == window->space_) {
      return window;
    }
  }
  return nullptr;
}

// static
content::WebContents* Window::PageOf(bc_id tab) {
  Tab* held = Tab::Get(tab);
  if (!held || held->web_contents()) {
    return held ? held->web_contents() : nullptr;
  }
  Window* window = Holding(tab);
  return window ? held->Wake(*window) : nullptr;
}

// static
void Window::PanesChanged() {
  // The window the user was in first, so a split it changed takes its pages from the others rather
  // than the other way round. One whose page another took waits for the user.
  for (Window* window : All()) {
    if (window->let_go_) {
      continue;
    }
    const bc_id selected = window->selected_;
    if (!selected) {
      continue;
    }
    const SplitLook look = LookOf(selected);
    if (PanesWith(selected) != window->panes_) {
      window->Select(selected);
    } else if (look.stacked != window->stacked_ || look.expanded != window->expanded_ ||
               (!window->expanded_ && look.shares != window->shares_)) {
      window->LayOutPage();
    }
  }
}

// static
bool Window::SplitTogether(content::WebContents* contents, content::WebContents* other) {
  const Tab* tab = Tab::FromWebContents(contents);
  const Tab* with = Tab::FromWebContents(other);
  return tab && with && tab != with && std::ranges::contains(PanesWith(tab->id()), with->id());
}

// static
void Window::LayOutShowing(bc_id tab) {
  for (auto& [id, window] : Windows()) {
    if (std::ranges::contains(window->panes_, tab)) {
      window->LayOutPage();
    }
  }
}

// static
void Window::PageCommittedInNewView(bc_id tab) {
  for (Window* window : All()) {
    if (window->selected_ == tab) {
      window->FocusPageSoon();
    }
  }
}

// static
void Window::OnPageClicked(bc_id tab) {
  for (auto& [id, window] : Windows()) {
    if (window->selected_ != tab && std::ranges::contains(window->panes_, tab)) {
      window->Select(tab);
    }
  }
}

// static
void Window::ActivateContents(content::WebContents* contents) {
  const bc_id tab = Tab::IdOf(contents);
  // A tab the window shows already stays as it is: a notification's click brings its tab forward,
  // and the page's own window.focus() in its click handler then asks again (#685).
  if (Window* window = tab ? Holding(tab) : nullptr; window && window->selected_ != tab) {
    window->Select(tab);
  }
}

// static
bool Window::CanDockDevTools(BrowserWindowInterface* browser) {
  return ForBrowser(browser);
}

// static
void Window::UpdateDevTools(BrowserWindowInterface* browser, content::WebContents* inspected) {
  Window* window = ForBrowser(browser);
  if (window && std::ranges::contains(window->panes_, Tab::IdOf(inspected))) {
    window->LayOutPage();
    // DevTools closed with the keyboard leave it with the window, and the page takes it back.
    window->FocusPageSoon();
  }
}

// static
void Window::ShowDevTools(BrowserWindowInterface* browser, content::WebContents* inspected) {
  Window* window = ForBrowser(browser);
  content::WebContents* devtools = DevToolsWindow::GetInTabWebContents(inspected, nullptr);
  if (!window || !devtools) {
    return;
  }
  // The keyboard stays where it is until LayOutPage has them past the page, so a key typed while
  // the frontend's first bounds are on their way goes to the page.
  window->devtools_to_show_ = devtools->GetWeakPtr();
  UpdateDevTools(browser, inspected);
}

// static
void Window::TrimPageMenu(BrowserWindowInterface* browser, ui::SimpleMenuModel& menu) {
  if (!ForBrowser(browser)) {
    return;
  }
  // Arc's menu is Chrome's without Lens, Reading Mode, Listen, QR Code, Send to Your Devices, Copy
  // Link Text and the items whose UI is Chrome's side panel or Gemini's (ux-spec.md › Page menu).
  // Translate is the URL field's (#671): Chrome's needs Google's servers. Cast… opens the app's card
  // (CastDialog).
  static constexpr int kTrimmed[] = {
      IDC_CONTENT_CONTEXT_SEARCHLENSFORIMAGE,
      IDC_CONTENT_CONTEXT_SEARCHLENSFORVIDEOFRAME,
      IDC_CONTENT_CONTEXT_LENS_REGION_SEARCH,
      IDC_CONTENT_CONTEXT_WEB_REGION_SEARCH,
      IDC_CONTENT_CONTEXT_LENS_OVERLAY,
      IDC_CONTENT_CONTEXT_OPEN_IN_READING_MODE,
      IDC_CONTENT_CONTEXT_LISTEN_TO_THIS_PAGE,
      IDC_CONTENT_CONTEXT_GENERATE_QR_CODE,
      IDC_SEND_TAB_TO_SELF,
      IDC_CONTENT_CONTEXT_SHARING_SUBMENU,
      IDC_CONTENT_CONTEXT_COPYLINKTEXT,
      IDC_CONTENT_CONTEXT_ADD_LINK_TO_READING_LIST,
      IDC_CONTENT_CONTEXT_SAVE_TO_MEMORY_BANKS,
      IDC_CONTENT_CONTEXT_GLIC,
      IDC_CONTENT_CONTEXT_GLICSHAREIMAGE,
      IDC_CONTENT_CONTEXT_RELOAD_GLIC,
      IDC_CONTENT_CONTEXT_ARCHIVE_GLIC,
      IDC_CONTENT_CONTEXT_TRANSLATE,
      IDC_CONTENT_CONTEXT_PARTIAL_TRANSLATE,
  };
  for (int id : kTrimmed) {
    if (std::optional<size_t> index = menu.GetIndexOfCommandId(id)) {
      menu.RemoveItemAt(*index);
    }
  }
  // A section left empty leaves a separator at the top or two together. Chrome takes off the last.
  for (size_t i = 0; i < menu.GetItemCount();) {
    if (menu.GetTypeAt(i) == ui::MenuModel::TYPE_SEPARATOR &&
        (i == 0 || menu.GetTypeAt(i - 1) == ui::MenuModel::TYPE_SEPARATOR)) {
      menu.RemoveItemAt(i);
    } else {
      ++i;
    }
  }
}

// static
Window* Window::ForUnplaced(const NavigateParams& params) {
  // A page's own new tab or window comes with its Browser, and so does whatever a window does.
  if (params.browser || params.source_contents || params.contents_to_insert) {
    return nullptr;
  }
  // Only what makes a tab: a navigation in "the current tab" of no Browser would take whichever
  // tab a window's Browser has active, which isn't the one it shows.
  switch (params.disposition) {
    case WindowOpenDisposition::NEW_FOREGROUND_TAB:
    case WindowOpenDisposition::NEW_BACKGROUND_TAB:
    case WindowOpenDisposition::NEW_WINDOW:
    case WindowOpenDisposition::NEW_POPUP:
      break;
    default:
      return nullptr;
  }
  Profile* profile = params.initiating_profile;
  if (!profile || profile->IsOffTheRecord()) {
    return nullptr;
  }
  // The user's own window active last whose Space is one of the user's in the navigation's
  // profile, so its tab goes into a Space of that profile, as the tab opened then goes to the Space
  // shown: not an agent's, which a window shows while a task's page is watched.
  const std::string dir = DirOf(profile);
  for (Window* window : All()) {
    const Space* shown = Space::FromId(window->space_);
    if (!window->closing_ && !window->off_the_record_ && !window->own_space_ && shown &&
        !shown->agent() && shown->profile() == dir) {
      return window;
    }
  }
  return nullptr;
}

// static
void Window::AdjustNavigate(NavigateParams& params) {
  // A navigation with no Browser, such as DevTools' PUT /json/new (#1831), Chrome opens in the
  // profile's tabbed Browser it activated last, else in a window of its own with a tab strip. A
  // window makes its profile's Browser only once one of its tabs gets a page there, and a launch
  // puts tabs back without one, so a window showing no page has none for Chrome to find. The
  // user's window takes it instead, in the Space it shows, as ⌘T's tab would go there.
  if (Window* front = ForUnplaced(params)) {
    if (BrowserWindowInterface* browser = front->BrowserFor(params.initiating_profile)) {
      params.browser = browser;
    }
  }
  Window* window = params.browser ? ForBrowser(params.browser) : nullptr;
  if (!window) {
    return;
  }
  // What a pop-up asked is this navigation's alone, so none is left from one that opened nothing.
  window->popup_.reset();
  window->popup_placed_ = false;
  // The page menu's Open Link in New Window and in Incognito Window: the app opens its windows, as
  // for ⌘N and ⇧⌘N. A private window's link stays private either way, as Chrome's does.
  if (params.started_from_context_menu &&
      (params.disposition == WindowOpenDisposition::NEW_WINDOW ||
       params.disposition == WindowOpenDisposition::OFF_THE_RECORD)) {
    Profile* profile = params.browser->GetProfile();
    const bool private_window = profile->IsOffTheRecord() ||
                                params.disposition == WindowOpenDisposition::OFF_THE_RECORD;
    params.disposition = WindowOpenDisposition::IGNORE_ACTION;
    // Posted: the app opens a window and a tab in it, which mustn't happen inside a navigation.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE,
        base::BindOnce(
            [](base::WeakPtr<Window> window, const std::string& url, bc_id profile_id,
               bool in_private) {
              if (window && Host().window_open_link) {
                Host().window_open_link(Host().context, window->id(), url.c_str(), profile_id,
                                        in_private);
              }
            },
            window->weak_factory_.GetWeakPtr(), params.url.spec(),
            ProfileId(DirOf(profile->GetOriginalProfile())), private_window));
    return;
  }
  Tab* source = params.source_contents ? Tab::FromWebContents(params.source_contents) : nullptr;
#if BUILDFLAG(IS_LINUX)
  // ⇧⌥-click on a link opens it in a split view on Linux, as ⌥-click does, since Openbox and Xfce
  // take every Alt-click for themselves but leave one with Shift (the owner's call, #2755). Blink
  // asks for a new window, as for ⇧-click, so the press it came from tells them apart.
  if (source && params.disposition == WindowOpenDisposition::NEW_WINDOW &&
      !params.started_from_context_menu && params.user_gesture && !params.contents_to_insert &&
      !params.post_data && source->last_press_held_alt()) {
    params.disposition = WindowOpenDisposition::NEW_SPLIT_VIEW;
  }
#endif
  // A page in an agent's Space the user isn't looking at opens its tabs and windows there, behind,
  // loading at once, as the agent's own: it works in its own tabs and never changes what the user
  // sees (#1470). Chrome gives a tab opened from a link the tab it shows as its opener, so the
  // window keeps which. One the user is looking at opens them as any page does.
  const bc_id source_id = source ? Tab::IdOf(params.source_contents) : 0;
  const Space* source_space = source ? Space::Holding(source_id) : nullptr;
  const bool for_agent = source_space && source_space->agent() && !Showing(source_id);
  window->agent_opener_ = 0;
  if (for_agent && (params.disposition == WindowOpenDisposition::NEW_FOREGROUND_TAB ||
                    params.disposition == WindowOpenDisposition::NEW_BACKGROUND_TAB ||
                    params.disposition == WindowOpenDisposition::NEW_POPUP ||
                    params.disposition == WindowOpenDisposition::NEW_WINDOW ||
                    params.disposition == WindowOpenDisposition::NEW_SPLIT_VIEW)) {
    params.disposition = WindowOpenDisposition::NEW_BACKGROUND_TAB;
    window->agent_opener_ = source_id;
  }
  // ⌥⌘-click on a link opens it in a Little ilo window, as Arc's does in Little Arc (the owner's
  // call over Chrome's split, 2026-10-29). The page asks for a split view, as for ⌥-click, so the
  // press it came from tells them apart. A private window's opens beside its page, as its links
  // stay private, and the link menu's Open Link in Split View splits.
  Profile* clicked_in = params.browser->GetProfile();
  const bool little_click = source &&
                            params.disposition == WindowOpenDisposition::NEW_SPLIT_VIEW &&
                            source->last_press_held_command() &&
                            !params.started_from_context_menu && !clicked_in->IsOffTheRecord();
  // With Little ilo off (bc_little_windows) it opens as ⌘-click does, in a tab behind.
  if (little_click && !UserSettings().little_windows) {
    params.disposition = WindowOpenDisposition::NEW_BACKGROUND_TAB;
  } else if (little_click && Host().window_open_little) {
    params.disposition = WindowOpenDisposition::IGNORE_ACTION;
    // Posted, as for Open Link in New Window above.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](base::WeakPtr<Window> window, const std::string& url, bc_id profile_id) {
                         if (window && Host().window_open_little) {
                           Host().window_open_little(Host().context, window->id(), url.c_str(),
                                                     profile_id);
                         }
                       },
                       window->weak_factory_.GetWeakPtr(), params.url.spec(),
                       ProfileId(DirOf(clicked_in))));
    return;
  }
  // ⇧-click on a web link, which Chrome opens in a new window: the app may open it in a link
  // preview instead (the owner, 2026-11-02, #1370). Not a page's window.open(), which holds the
  // page it made, or a form sent.
  if (source && params.disposition == WindowOpenDisposition::NEW_WINDOW &&
      !params.started_from_context_menu && params.user_gesture && !params.contents_to_insert &&
      !params.post_data && params.url.SchemeIsHTTPOrHTTPS() && Host().tab_link_to_preview) {
    const gfx::PointF click = source->last_click();
    // The app only answers here: it opens the preview after this call, outside the navigation.
    if (Host().tab_link_to_preview(Host().context, Tab::IdOf(params.source_contents),
                                   params.url.spec().c_str(), click.x(), click.y())) {
      params.disposition = WindowOpenDisposition::IGNORE_ACTION;
      return;
    }
  }
  // ⌥-click on a link opens it beside its page in a split view, as Arc's does
  // (chromium-core-alt-click-split.patch).
  const bool split = source && params.disposition == WindowOpenDisposition::NEW_SPLIT_VIEW;
  window->split_with_ = split ? Tab::IdOf(params.source_contents) : 0;
  // A page's pop-up, as a sign-in's: the app hears what it asked for with its tab (tab_popup).
  if (source && params.disposition == WindowOpenDisposition::NEW_POPUP) {
    const blink::mojom::WindowFeatures& features = params.window_features;
    const gfx::Rect& asked = features.bounds;
    window->popup_ = bc_rect{
        static_cast<double>(asked.x()), static_cast<double>(asked.y()),
        features.has_width ? static_cast<double>(asked.width()) : 0.,
        features.has_height ? static_cast<double>(asked.height()) : 0.};
    window->popup_placed_ = features.has_x && features.has_y;
  }
  // Popups become tabs (decision 3). So do new windows: the app opens its own.
  if (split || params.disposition == WindowOpenDisposition::NEW_POPUP ||
      params.disposition == WindowOpenDisposition::NEW_WINDOW) {
    params.disposition = WindowOpenDisposition::NEW_FOREGROUND_TAB;
  }
  // A link a page opens behind, as by ⌘-click, loads only once its tab is first shown (Dia). A
  // form sent to a tab behind goes at once, when the user sent it, and a window the page made
  // itself already has its page and the page's hold on it.
  if (source && !for_agent && params.disposition == WindowOpenDisposition::NEW_BACKGROUND_TAB &&
      params.url.SchemeIsHTTPOrHTTPS() && !params.post_data && !params.contents_to_insert) {
    content::WebContents* contents = window->OpenBehind(params);
    // Posted, as for the tabs Chrome opens.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Window::ReportTabOpened, window->weak_factory_.GetWeakPtr(),
                                  Tab::IdOf(contents), Tab::IdOf(params.source_contents),
                                  /*foreground=*/false, /*split_with=*/0,
                                  /*popup=*/std::nullopt, /*popup_placed=*/false));
    params.disposition = WindowOpenDisposition::IGNORE_ACTION;
  }
  // Chrome's downloads page, which it opens after the user stopped a quit to let downloads finish,
  // is the Library's Downloads here.
  if (params.url.SchemeIs(content::kChromeUIScheme) &&
      params.url.host() == chrome::kChromeUIDownloadsHost) {
    params.disposition = WindowOpenDisposition::IGNORE_ACTION;
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](base::WeakPtr<Window> window) {
                         if (window && Host().window_show_downloads) {
                           Host().window_show_downloads(Host().context, window->id());
                         }
                       },
                       window->weak_factory_.GetWeakPtr()));
  }
  // A tab an agent's tab opens, which goes in the agent's Space, is the agent's before its page
  // starts, as every rule for an agent's page asks of it then (#2660). A pop-up's page, which
  // content made, the opener's Tab held as content made it (Tab::DidOpenRequestedURL).
  if (source_space && source_space->agent() && !params.contents_to_insert &&
      (params.disposition == WindowOpenDisposition::NEW_FOREGROUND_TAB ||
       params.disposition == WindowOpenDisposition::NEW_BACKGROUND_TAB) &&
      !params.url.is_empty() && !params.url.SchemeIs(url::kAboutScheme) &&
      !params.url.SchemeIs(content::kChromeUIScheme)) {
    MakeHeldBy(*Space::Holding(source_id), params);
  }
}

// static
void Window::OnTabClosing(bc_id tab) {
  Space* space = Space::Holding(tab);
  if (!space && !Favorites::Has(tab)) {
    return;
  }
  // One closing with the last regular window, or as the browser quits, comes back at the next
  // launch. A window's own Space's never does.
  const auto regular_closing = [](const auto& entry) {
    return entry.second->closing_ && !entry.second->own_space_;
  };
  if (!(space && space->own()) &&
      (browser_shutdown::IsTryingToQuit() || std::ranges::any_of(Windows(), regular_closing))) {
    KeepSession();
  }
  const bc_id next = space ? space->NextAfterLeaving(tab, &Tab::Loaded) : 0;
  // A favorite split's page closing leaves the rest of it to show, the one on its right first.
  bc_id beside = 0;
  if (!space) {
    const std::vector<bc_id> panes = Favorites::Panes(tab);
    const auto at = std::ranges::find(panes, tab);
    auto right = std::find_if(at == panes.end() ? at : at + 1, panes.end(), &Tab::Loaded);
    auto left = std::find_if(std::make_reverse_iterator(at), panes.rend(), &Tab::Loaded);
    beside = right != panes.end() ? *right : left != panes.rend() ? *left : 0;
  }
  if (space) {
    space->Remove(tab);
  } else {
    Favorites::Remove(tab);
  }
  // The window the user was in picks first, as another passes over the tab that one shows.
  for (Window* window : All()) {
    if (window->selected_ == tab) {
      window->selected_ = 0;
      Space* shown = space || beside ? nullptr : Space::FromId(window->space_);
      window->next_after_closing_ = beside ? beside
                                    : shown ? shown->NextAfterLeaving(tab, &Tab::Loaded)
                                            : next;
#if BUILDFLAG(IS_LINUX)
      // Until the next page shows, keys in nothing reach no shortcut, as ⌘T straight after ⌘W
      // would, where the Mac's menu bar takes them: the UI holds them for it, as with no page
      // (#1178).
      views::Widget* widget = window->widget_.get();
      if (!window->closing_ && widget->IsActive() && platform::KeyboardForPage(widget)) {
        platform::UITakesKeyboard(widget, /*until_page=*/true);
      }
#endif
      // Posted: Chrome is part way through closing the tab, and its tab strip can't change yet.
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE,
          base::BindOnce(&Window::SelectAfterClosing, window->weak_factory_.GetWeakPtr()));
    } else if (!window->selected_ && window->next_after_closing_ == tab) {
      // The tab it was to show is closing too, as a split view's tabs close together.
      window->next_after_closing_ = next;
    }
  }
  // A window showing it beside the tab it shows shows the rest of the split.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Window::PanesChanged));
  if (space && space->pinned().empty() && space->today().empty() &&
      DeletingSpaces().erase(space->id())) {
    Space::Delete(space->id());
  }
}

// static
void Window::Unload(bc_id tab, bool split) {
  Space* space = Space::Holding(tab);
  if ((space ? !space->IsPinned(tab) : !Favorites::Has(tab)) || !Tab::Get(tab)) {
    return;
  }
  const std::vector<bc_id> going = split ? PanesWith(tab) : std::vector<bc_id>{tab};
  const auto staying = [&going](bc_id other) {
    return !std::ranges::contains(going, other) && Tab::Loaded(other);
  };
  const bc_id next = space ? space->NextAfterLeaving(tab, staying) : 0;
  // Out of its split, so showing the rest of it doesn't load it again.
  if (space && !split) {
    space->Separate(tab);
  } else if (!split) {
    Favorites::Separate(tab);
  }
  // Another tab first, so the pages unload out of sight.
  for (Window* window : All()) {
    if (std::ranges::contains(going, window->selected_)) {
      Space* shown = space ? nullptr : Space::FromId(window->space_);
      if (const bc_id its_next = shown ? shown->NextAfterLeaving(tab, staying) : next) {
        window->Pick(its_next);
      } else {
        window->ShowNothing();
      }
    }
    // Nor does showing another Space that showed it last.
    std::erase_if(window->selected_in_space_, [&going](const auto& entry) {
      return std::ranges::contains(going, entry.second);
    });
  }
  PanesChanged();
  for (bc_id each : going) {
    if (Tab* unloading = Tab::Get(each)) {
      unloading->Unload();
    }
  }
}

// static
void Window::MoveToFavorites(bc_id tab, size_t index) {
  if (Favorites::Has(tab)) {
    Favorites::Move(tab, index);
    return;
  }
  Tab* moving = Tab::Get(tab);
  Space* space = Space::Holding(tab);
  // A private window's tabs are never saved, as the favorites are.
  if (!moving || !space || space->own()) {
    return;
  }
  // Under Each Space the favorites are the user's Spaces' own, and an agent's has none.
  if (space->agent() && Favorites::scope() == BC_FAVORITES_SPACE) {
    return;
  }
  // In the favorites before it leaves its Space, so the UI never hears of it in neither and
  // forgets its title.
  Favorites::Add(tab, space->IsPinned(tab) ? space->PinnedUrl(tab) : moving->Url(), index,
                 space->id());
  space->Remove(tab);
  // A window showing it beside the tab it shows shows the rest of the split.
  PanesChanged();
  // Whether it's away from its URL.
  moving->StateChanged();
}

// static
void Window::MoveSplitToFavorites(bc_id tab, size_t index) {
  if (Favorites::Has(tab)) {
    Favorites::Move(tab, index);
    return;
  }
  Space* space = Space::Holding(tab);
  const std::vector<bc_id> panes = space ? space->Panes(tab) : std::vector<bc_id>();
  if (panes.size() < 2) {
    MoveToFavorites(tab, index);
    return;
  }
  // As MoveToFavorites: never a private window's, nor an agent's under Each Space.
  if (space->own() || (space->agent() && Favorites::scope() == BC_FAVORITES_SPACE) ||
      std::ranges::any_of(panes, [](bc_id pane) { return !Tab::Get(pane); })) {
    return;
  }
  SplitView split{.tabs = panes,
                  .stacked = space->Stacked(tab),
                  .shares = space->Shares(tab),
                  .expanded = space->Expanded(tab),
                  .name = space->SplitName(tab),
                  .icon = space->SplitIcon(tab),
                  .focused = tab};
  // Focused where a window shows it focused.
  for (Window* window : All()) {
    if (std::ranges::contains(panes, window->selected_)) {
      split.focused = window->selected_;
      break;
    }
  }
  std::vector<GURL> urls;
  for (bc_id pane : panes) {
    urls.push_back(space->IsPinned(pane) ? space->PinnedUrl(pane) : Tab::Get(pane)->Url());
  }
  // In the favorites before they leave their Space, as MoveToFavorites.
  Favorites::AddSplit(std::move(split), urls, index, space->id());
  for (bc_id pane : panes) {
    space->Remove(pane);
  }
  PanesChanged();
  for (bc_id pane : panes) {
    Tab::Get(pane)->StateChanged();
  }
}

// static
void Window::Unfavorite(bc_id tab, bool pinned, bc_id folder, size_t index) {
  Window* window = Holding(tab);
  Space* shown = window ? Space::FromId(window->space_) : nullptr;
  // A favorite is the user's, so it doesn't move into an agent's Space.
  if (!shown || shown->agent() || shown->own() || !Favorites::ShownIn(tab, *shown)) {
    return;
  }
  // A split goes whole, a split there too, as it looked.
  const std::vector<bc_id> panes = Favorites::Panes(tab);
  const SplitView* split = Favorites::SplitOf(tab);
  const SplitView look = split ? *split : SplitView();
  // In the Space before it leaves the favorites, as above.
  for (size_t i = 0; i < panes.size(); ++i) {
    const bc_id pane = panes[i];
    if (pinned) {
      shown->AddPinned(pane, folder, index, Favorites::Url(pane));
    } else {
      shown->AddAt(pane, index);
    }
    if (i > 0) {
      shown->Split(pane, panes[i - 1], Favorites::Url(pane), /*left=*/false);
    }
  }
  if (panes.size() > 1) {
    shown->SetStacked(tab, look.stacked);
    shown->SetShares(tab, look.shares);
    if (look.expanded) {
      shown->SetExpanded(look.expanded, true);
    }
    shown->SetSplitName(tab, look.name);
    shown->SetSplitIcon(tab, look.icon);
  }
  for (bc_id pane : panes) {
    // Under Every Space, the site's tabs in the other profiles close.
    Favorites::Unfavorite(pane);
    if (Tab* moving = Tab::Get(pane)) {
      // So it isn't archived at once.
      moving->set_last_seen(base::Time::Now());
      moving->StateChanged();
    }
  }
  PanesChanged();
}

// static
void Window::SetSpaceProfile(bc_id space, const std::string& profile) {
  Space* in = Space::FromId(space);
  if (!in || in->own() || !in->pinned_rows().empty() || !in->today().empty() ||
      in->profile() == profile) {
    return;
  }
  in->SetProfile(profile);
  // Its own favorites go with it, and the profile gets the sites it lacks.
  Favorites::SpaceMoved(space);
  HiddenFavoritesGone();
}

// static
void Window::TabsArrived(const std::vector<bc_id>& tabs) {
  for (auto& [id, window] : Windows()) {
    const bc_id waiting = window->waiting_;
    const Space* shown = Space::FromId(window->space_);
    if (!waiting || !std::ranges::contains(tabs, waiting) || !shown ||
        !(shown->Has(waiting) || Favorites::ShownIn(waiting, *shown))) {
      continue;
    }
    window->Select(waiting);
  }
}

// static
void Window::HiddenFavoritesGone() {
  for (auto& [id, window] : Windows()) {
    // A favorite a Space no longer shows isn't the tab it comes back to.
    std::erase_if(window->selected_in_space_, [](const auto& entry) {
      const Space* in = Space::FromId(entry.first);
      return Favorites::Has(entry.second) && !(in && Favorites::ShownIn(entry.second, *in));
    });
    const Space* shown = Space::FromId(window->space_);
    if (!shown || !Favorites::Has(window->selected_) ||
        Favorites::ShownIn(window->selected_, *shown)) {
      continue;
    }
    const auto free = [&window](bc_id tab) { return !window->ShownElsewhere(tab); };
    const std::vector<bc_id> pinned = shown->pinned();
    auto today = std::ranges::find_if(shown->today(), free);
    auto first = std::ranges::find_if(pinned, free);
    if (today != shown->today().end()) {
      window->Pick(*today);
    } else if (first != pinned.end()) {
      window->Pick(*first);
    } else {
      window->ShowNothing();
    }
  }
}

// static
void Window::ForgetPage(bc_id tab, const GURL& url) {
  ForgottenPages()[tab] = url;
}

// static
void Window::CloseTab(bc_id tab, bool discard) {
  // One without its page has no unload handlers to keep it.
  if (Tab* closing = Tab::Get(tab); closing && !closing->web_contents()) {
    KeepClosed(tab);
    closing->Close();
    return;
  }
  content::WebContents* contents = Tab::FromId(tab);
  tabs::TabInterface* in = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  if (!in) {
    return;
  }
  TabStripModel* tabs = in->GetBrowserWindowInterface()->GetTabStripModel();
  if (discard) {
    tabs->DetachAndDeleteWebContentsAt(tabs->GetIndexOfWebContents(contents));
    return;
  }
  tabs->CloseWebContentsAt(tabs->GetIndexOfWebContents(contents),
                           TabCloseTypes::CLOSE_USER_GESTURE);
}

// static
void Window::CloseBelow(bc_id tab) {
  Space* space = Space::Holding(tab);
  if (!space || space->IsPinned(tab)) {
    return;
  }
  const std::vector<bc_id>& today = space->today();
  // Below its split, if it's in one.
  const std::vector<bc_id> below(std::ranges::find(today, space->Panes(tab).back()) + 1,
                                 today.end());
  for (Window* window : All()) {
    if (std::ranges::contains(below, window->selected_)) {
      window->Pick(tab);
    }
  }
  for (auto id = below.rbegin(); id != below.rend(); ++id) {
    CloseTab(*id);
  }
}

bc_id Window::TakeTab(bc_id tab) {
  Space* from = Space::Holding(tab);
  Space* to = Space::FromId(space_);
  // Only a Little ilo window's empty Space takes one, of the user's own Today tabs in its profile:
  // a pinned tab or a favorite keeps its place, an agent's stays out of the user's window, and a
  // page can't take its cookies to another profile or out of incognito.
  if (!own_space_ || off_the_record_ || !from || !to || from == to || !to->today().empty() ||
      from->own() || from->off_the_record() || from->agent().has_value() ||
      from->profile() != to->profile() || from->IsPinned(tab) || Favorites::Has(tab) ||
      from->Panes(tab).size() > 1 || DeletingSpaces().contains(from->id())) {
    return 0;
  }
  const bc_id next = from->NextAfterLeaving(tab, &Tab::Loaded);
  // In its new Space before it leaves the old, as MoveToSpace does.
  to->AddAt(tab, 0);
  from->Remove(tab);
  // Its page comes here, which lets go of it in the window that showed it.
  Select(tab);
  for (auto& [id, window] : Windows()) {
    if (id == id_) {
      continue;
    }
    if (window->selected_ == tab) {
      window->selected_ = next;
      if (next) {
        window->Pick(next);
      } else {
        window->ShowNothing();
      }
    } else if (window->SelectedIn(from->id()) == tab) {
      window->SetSelectedIn(from->id(), next);
    }
  }
  PanesChanged();
  return tab;
}

// static
void Window::MoveToSpace(bc_id tab, bc_id space, bool to_agent) {
  Space* from = Space::Holding(tab);
  Space* to = Space::FromId(space);
  // No tab moves into an agent's Space, which would put it in the agent's reach, unless the user
  // gives it, and Chrome never moves a page between incognito and regular windows. A tab leaves a
  // Little ilo window's Space for the user's, as ⌘O moves it, and none goes into one.
  if (!from || !to || from == to || to->agent().has_value() != to_agent ||
      from->off_the_record() || to->own() || DeletingSpaces().contains(space)) {
    return;
  }
  // A page can't take its cookies and logins to another profile, so it opens there again.
  const bc_id moved = from->profile() == to->profile() ? tab : CopyToProfile(tab, to->profile());
  if (!moved) {
    return;
  }
  const bool pinned = from->IsPinned(tab);
  const GURL url = from->PinnedUrl(tab);
  const bc_id next = from->NextAfterLeaving(tab, &Tab::Loaded);
  // In a Space it's the user's or the agent's, and its page closes it no longer.
  for (bc_id each : {tab, moved}) {
    if (Tab* page = Tab::Get(each)) {
      page->SetPageMayClose(false);
    }
  }
  // A Today tab keeps its node's id, so sync moves it on the account's other devices rather than
  // closing it into their archives and opening it again there.
  const Space::Node* node = pinned ? nullptr : from->NodeOf(tab);
  const base::Uuid kept = node ? node->id : base::Uuid();
  // In its new Space before it leaves the old, so the UI never hears of it in neither and forgets
  // its title.
  if (pinned) {
    to->AddPinned(moved, 0, to->pinned_rows().size(), url);
  } else {
    to->AddAt(moved, 0);
  }
  from->Remove(tab);
  // Out of an agent's Space, it's the user's: the agent's session there ends, and with it what
  // the agent started in it, which would land in the user's page.
  if (from->agent()) {
    EndTabSession(tab, BC_AGENT_SESSION_RELEASED);
  }
  if (kept.is_valid()) {
    // Its place was among the old Space's tabs, so it gets one at the top of the new one's.
    to->RestoreNodes(to->node(), {{moved, Space::Node{kept, syncer::UniquePosition()}}});
  }
  FollowMove(tab, moved, from->id(), space, next);
  // Out of its Space, it isn't kept for ReopenClosedTab or archived.
  if (moved != tab) {
    CloseTab(tab);
  }
  PanesChanged();
}

// static
void Window::MoveFolderToSpace(bc_id folder, bc_id space) {
  Space* from = Space::HoldingFolder(folder);
  Space* to = Space::FromId(space);
  // Where MoveToSpace takes a pinned tab the user doesn't give to an agent.
  if (!from || !to || from == to || to->agent() || to->own() || DeletingSpaces().contains(space)) {
    return;
  }
  std::vector<bc_id> tabs;
  for (bc_id row : from->pinned_rows()) {
    if (!from->IsFolder(row) && from->Within(row, folder)) {
      tabs.push_back(row);
    }
  }
  std::map<bc_id, bc_id> copies;
  if (from->profile() != to->profile()) {
    for (bc_id tab : tabs) {
      const bc_id copy = CopyToProfile(tab, to->profile());
      if (!copy) {
        // The folder goes whole or not at all.
        for (const auto& [tab_copied, made] : copies) {
          CloseTab(made);
        }
        return;
      }
      copies[tab] = copy;
    }
  }
  to->TakeFolder(*from, folder, copies);
  // What the old Space shows next is none of the folder's.
  const bc_id next = from->NextAfterLeaving(folder, &Tab::Loaded);
  for (bc_id tab : tabs) {
    auto copy = copies.find(tab);
    FollowMove(tab, copy == copies.end() ? tab : copy->second, from->id(), space, next);
  }
  for (const auto& [tab, made] : copies) {
    CloseTab(tab);
  }
  PanesChanged();
}

// static
void Window::FollowMove(bc_id tab, bc_id moved, bc_id from, bc_id space, bc_id next) {
  for (auto& [id, window] : Windows()) {
    if (window->selected_ == tab && window->let_go_) {
      // Its page went to another window, which takes it along.
      window->Pick(next);
    } else if (window->selected_ == tab && window->own_space_) {
      // Only its own window shows a window's own Space, so the page waits for the window the app
      // shows it in next (bc_window_select), as ⌘O does.
      window->selected_ = next;
      if (next) {
        window->Pick(next);
      } else {
        window->ShowNothing();
      }
    } else if (window->selected_ == tab) {
      // ShowSpace keeps |next| as the tab the old Space shows.
      window->selected_ = next;
      window->SetSelectedIn(space, moved);
      window->ShowSpace(space);
    } else if (window->SelectedIn(from) == tab) {
      window->SetSelectedIn(from, next);
    }
  }
}

// static
bool Window::DeleteSpace(bc_id space) {
  std::vector<Space*> staying = Space::All();
  std::erase_if(staying, [](Space* each) { return DeletingSpaces().contains(each->id()); });
  auto it = std::ranges::find(staying, space, &Space::id);
  if (it == staying.end() || staying.size() < 2) {
    return false;
  }
  Space* neighbour = it != staying.begin() ? *(it - 1) : *(it + 1);
  // Before the windows leave it, so it keeps the tab the one active last showed there.
  DeletedSpace& deleted = DeletedSpaces().emplace_back();
  deleted.id = space;
  const std::vector<Space*> all = Space::All();
  deleted.at = static_cast<size_t>(std::ranges::find(all, space, &Space::id) - all.begin());
  // With no window left, as at quit once the last has closed, it shows nothing (#1687).
  const std::vector<Window*> windows = All();
  deleted.shown = windows.empty() ? 0 : windows.front()->SelectedIn(space);
  deleted.saved = SaveSpace(**it, deleted.shown, &deleted.today);
  if (DeletedSpaces().size() > kMaxClosedTabs) {
    // |deleted| is the last, which stays.
    DeletedSpaces().erase(DeletedSpaces().begin());
  }
  for (auto& [id, window] : Windows()) {
    if (window->space_ == space) {
      window->ShowSpace(neighbour->id());
    }
    window->selected_in_space_.erase(space);
  }
  // Under Each Space, its favorites close with it.
  Favorites::SpaceDeleting(space);
  std::vector<bc_id> tabs = (*it)->pinned();
  std::ranges::copy((*it)->today(), std::back_inserter(tabs));
  if (tabs.empty()) {
    Space::Delete(space);
    return true;
  }
  DeletingSpaces().insert(space);
  // As the user closes them, so ⇧⌘T reopens the Today tabs. The last one deletes the Space.
  for (bc_id tab : tabs) {
    CloseTab(tab);
  }
  return true;
}

Window::Window(bc_id id, const gfx::Rect& bounds, void* ui_view, bool behind)
    : id_(id), sidebar_width_(SavedSidebarWidth()) {
  delegate_ = std::make_unique<views::WidgetDelegate>();
  delegate_->SetCanFullscreen(true);
  delegate_->SetCanResize(true);
  delegate_->SetCanMaximize(true);
  delegate_->SetCanMinimize(true);
  delegate_->SetClientViewFactory(base::BindOnce(
      [](base::WeakPtr<Window> window, views::Widget* widget,
         views::View* contents) -> std::unique_ptr<views::ClientView> {
        return std::make_unique<ClientView>(widget, contents, std::move(window));
      },
      weak_factory_.GetWeakPtr()));

  widget_ = std::make_unique<views::Widget>();
  views::Widget::InitParams params(views::Widget::InitParams::CLIENT_OWNS_WIDGET,
                                   views::Widget::InitParams::TYPE_WINDOW);
  params.delegate = delegate_.get();
  params.native_widget = platform::NewNativeWidget(
      widget_.get(), base::BindRepeating(&Window::LeaveFullScreen, weak_factory_.GetWeakPtr()));
  params.bounds = bounds;
  params.name = "BrowserCoreWindow";
#if BUILDFLAG(IS_LINUX)
  platform::SetUpWindow(params, *delegate_);
#endif
  widget_->Init(std::move(params));
  widget_observation_.Observe(widget_.get());
#if BUILDFLAG(IS_LINUX)
  // Where it opens, for a window manager that maximizes it before it ever moves.
  KeepNormalFrame(normal_frames_, widget_->GetWindowBoundsInScreen());
#endif
  platform::AddWindowViews(widget_.get(), ui_view);
  if (behind) {
    widget_->ShowInactive();
  } else {
    widget_->Show();
  }
}

Window::~Window() {
  for (web_modal::ModalDialogHostObserver& observer : dialog_observers_) {
    observer.OnHostDestroying();
  }
}

BrowserWindowInterface* Window::BrowserFor(Profile* profile) {
  if (closing_ || browser_shutdown::IsTryingToQuit()) {
    return nullptr;
  }
  if (auto it = browsers_.find(profile); it != browsers_.end()) {
    BrowserWindowInterface* browser = it->second.browser;
    // One closes once its last tab has, and the profile's next tab goes in a new one.
    if (!browser->IsDeleteScheduled() && !browser->capabilities()->IsAttemptingToCloseBrowser()) {
      return browser;
    }
  }
  if (GetBrowserWindowCreationStatusForProfile(*profile) !=
      BrowserWindowInterface::CreationStatus::kOk) {
    return nullptr;
  }

  // The Browser owns its window and deletes it as it goes.
  auto* stand_in = new BrowserWindowStandIn(weak_factory_.GetWeakPtr());
  BrowserWindowCreateParams params(BrowserWindowInterface::TYPE_NORMAL, *profile,
                                   /*from_user_gesture=*/false);
  params.window = stand_in;
  // The app keeps the tabs it restores, in Spaces, so Chrome's sessions stay out of it.
  params.omit_from_session_restore = true;
  params.should_trigger_session_restore = false;
  BrowserWindowInterface* browser = CreateBrowserWindow(std::move(params));
  stand_in->SetBrowser(browser);
  browser->GetTabStripModel()->AddObserver(this);
  // Chrome's bubbles find the widget to anchor to through this.
  if (auto* elements = BrowserElements::From(browser)->AsA<BrowserElementsViewsImpl>()) {
    elements->Init(widget_->GetRootView());
  }
  browsers_[profile] = Held{browser};
  return browser;
}

content::WebContents* Window::RestoreTab(
    Profile* profile,
    base::span<const sessions::SerializedNavigationEntry> history,
    int current,
    content::SessionStorageNamespaceHandle* storage) {
  BrowserWindowInterface* browser = BrowserFor(profile);
  if (!browser) {
    return nullptr;
  }
  base::AutoReset<bool> opening(&opening_tab_, true);
  return chrome::AddRestoredTab(
      browser, history, browser->GetTabStripModel()->count(), current, /*extension_app_id=*/"",
      /*group=*/std::nullopt, /*select=*/false, /*pin=*/false, base::TimeTicks(), base::Time(),
      /*storage_namespace=*/storage, sessions::SerializedUserAgentOverride(),
      /*extra_data=*/{}, /*from_session_restore=*/true, /*is_active_browser=*/std::nullopt);
}

void Window::Suggest(Profile* profile,
                     const std::u16string& keyword,
                     const std::u16string& text,
                     bool complete) {
  // Closing, it may have let its profile go.
  if (closing_) {
    return;
  }
  if (!suggestions_ || suggestions_->profile() != profile) {
    suggestions_ = std::make_unique<Suggestions>(id_, profile);
  }
  suggestions_->Start(keyword, text, complete);
}

// static
void Window::LetProfileGo(Profile* profile) {
  for (auto& [id, window] : Windows()) {
    if (window->suggestions_ && window->suggestions_->profile() == profile) {
      window->suggestions_.reset();
    }
  }
}

bc_id Window::RemoveSuggestion(const GURL& url) {
  return suggestions_ ? suggestions_->Remove(url) : 0;
}

bool Window::UndoRemoveSuggestion(bc_id removal) {
  return suggestions_ && suggestions_->Restore(removal);
}

bool Window::RedoRemoveSuggestion(bc_id removal) {
  return suggestions_ && suggestions_->RemoveAgain(removal);
}

bc_id Window::RemoveHistory(Profile* profile, const GURL& url, base::Time begin, base::Time end) {
  // Closing, it may have let its profile go.
  if (closing_ || !profile) {
    return 0;
  }
  // The removals live with the window's suggestions, so one ⌘Z path undoes both.
  if (!suggestions_ || suggestions_->profile() != profile) {
    suggestions_ = std::make_unique<Suggestions>(id_, profile);
  }
  return suggestions_->RemoveVisits(url, begin, end);
}

content::WebContents* Window::OpenTab(Profile* profile,
                                      const GURL& url,
                                      bool when_shown,
                                      bool typed_http) {
  BrowserWindowInterface* browser = BrowserFor(profile);
  if (!browser) {
    return nullptr;
  }
  NavigateParams params(browser, url, ui::PAGE_TRANSITION_TYPED);
  params.disposition = WindowOpenDisposition::NEW_BACKGROUND_TAB;
  params.url_typed_with_http_scheme = typed_http;
  if (when_shown && url.SchemeIsHTTPOrHTTPS()) {
    return OpenBehind(params);
  }
  // The app hears only of the tabs it didn't open.
  base::AutoReset<bool> opening(&opening_tab_, true);
  Navigate(&params);
  return params.navigated_or_inserted_contents;
}

content::WebContents* Window::OpenHeld(Space& space, Profile* profile, const GURL& url) {
  BrowserWindowInterface* browser = BrowserFor(profile);
  if (!browser) {
    return nullptr;
  }
  NavigateParams params(browser, url, ui::PAGE_TRANSITION_TYPED);
  params.disposition = WindowOpenDisposition::NEW_BACKGROUND_TAB;
  MakeHeldBy(space, params);
  // The app hears only of the tabs it didn't open.
  base::AutoReset<bool> opening(&opening_tab_, true);
  Navigate(&params);
  HeldLoad::Start(params.navigated_or_inserted_contents);
  return params.navigated_or_inserted_contents;
}

content::WebContents* Window::OpenFromPinned(bc_id pinned, const GURL& url, bool typed_http) {
  // A pinned tab without its page opens it too.
  const Tab* from = Tab::Get(pinned);
  content::WebContents* contents =
      from && from->profile() ? OpenTab(from->profile(), url, /*when_shown=*/false, typed_http)
                              : nullptr;
  if (contents) {
    ReportTabOpened(Tab::IdOf(contents), pinned, /*foreground=*/true, /*split_with=*/0,
                    /*popup=*/std::nullopt, /*popup_placed=*/false);
  }
  return contents;
}

content::WebContents* Window::OpenPreview(content::WebContents& opener,
                                          const GURL& url,
                                          const gfx::Size& size) {
  Profile* profile = Profile::FromBrowserContext(opener.GetBrowserContext());
  BrowserWindowInterface* browser = BrowserFor(profile);
  if (!browser) {
    return nullptr;
  }
  // Made here and loaded once it's sized, rather than by Navigate, which makes a background tab,
  // sizes it to the active tab's page and loads it at once, so the preview's page read the window's
  // page's size and drew a frame at it before the app's size came (#2539).
  content::WebContents::CreateParams create(profile,
                                            tab_util::GetSiteInstanceForNewTab(profile, url));
  create.initially_hidden = true;
#if defined(USE_AURA)
  create.context = widget_->GetNativeWindow();
#endif
  NavigateParams params(browser, content::WebContents::Create(create));
  params.disposition = WindowOpenDisposition::NEW_BACKGROUND_TAB;
  // No Space takes it until the app keeps it, so the app isn't told of it as a tab opened.
  base::AutoReset<bool> opening(&opening_tab_, true);
  Navigate(&params);
  content::WebContents* preview = params.navigated_or_inserted_contents;
  if (!preview) {
    return nullptr;
  }
  // The app shows it in its card until it draws, so it shows the window's page colour until then,
  // as the panes do, not Chromium's white (#2742).
  if (Tab* tab = Tab::FromWebContents(preview); tab && page_color_) {
    tab->SetPageColor(
        SkColorSetRGB(*page_color_ >> 16, *page_color_ >> 8 & 0xff, *page_color_ & 0xff));
  }
  // Inserting a background tab sized it to the active tab's page.
  preview->Resize(gfx::Rect(size));
  NavigateParams load(browser, url, ui::PAGE_TRANSITION_LINK);
  // What the page would have sent with the link, by the policy Chrome's pages default to.
  load.referrer = content::Referrer(
      opener.GetLastCommittedURL(), network::mojom::ReferrerPolicy::kStrictOriginWhenCrossOrigin);
  preview->GetController().LoadURLWithParams(LoadURLParamsFromNavigateParams(preview, &load));
  return preview;
}

void Window::Show(content::WebContents* contents, const gfx::Rect& frame) {
  // A tab that has left its Browser has no strip to be active in, nor a window to be shown in.
  if (!InTabStrip(contents)) {
    page_frame_ = frame;
    return;
  }
  // The Browser's active tab is the one Chrome's keyboard shortcuts, find and fullscreen act on.
  TabStripModel* tabs =
      tabs::TabInterface::GetFromContents(contents)->GetBrowserWindowInterface()->GetTabStripModel();
  // A page another window has a picture of goes there once this one has shown the next
  // (AdoptFreedPages), and its offer, which Chrome hides here, with it.
  content::WebContents* leaving = tabs->GetActiveWebContents();
  const bc_id left = leaving && leaving != contents ? Tab::IdOf(leaving) : 0;
  const bc_id freed = left && Picturing(left) ? left : 0;
  PasswordOffer::Moving moving(freed);
  HoldPictured();
  // A full-screen page's tab leaving takes the window out with it, within ActivateTabAt. The tab
  // active already isn't activated again: off the Mac the window leaves a page's full screen as
  // Chrome asks, so the app's frame for the new size comes within Chrome's change to the tabs that
  // ended the page's full screen, as closing them all does, where activating one fails a CHECK
  // (#1991).
  if (leaving != contents) {
    tabs->ActivateTabAt(tabs->GetIndexOfWebContents(contents));
  }
  const bc_id tab = Tab::IdOf(contents);
  std::vector<bc_id> panes = PanesWith(tab);
  for (bc_id pane : panes_) {
    if (!std::ranges::contains(panes, pane)) {
      Seen(pane);
    }
  }
  panes_ = std::move(panes);
  shown_ = contents->GetWeakPtr();
  SetLetGo(false);
  page_frame_ = frame;
  if (freed) {
    g_left_size = focused_frame_.size();
  }
  // If the page it replaces had the keyboard, the window has it once that page leaves, and the
  // new page takes it. Not at a frame for a window that has just left full screen, whose pages
  // wait for its new size: the app lays them out once it has it (LeaveFullScreen).
  if (!platform::PagesHeldForResize(widget_.get())) {
    LayOutPage();
  }
  if (freed) {
    LogHandover("giver let go", Handing(id_, freed));
  }
  if (tab == g_adopted) {
    g_adopted = 0;
    const std::string detail = base::StrCat({Handing(id_, tab), ", ", g_left_size.ToString(),
                                             " to ", focused_frame_.size().ToString()});
    LogHandover("taker laid out the page", detail);
    contents->GetPrimaryMainFrame()->InsertVisualStateCallback(base::BindOnce(
        [](const std::string& detail, bool) { LogHandover("taker's page drew a frame", detail); },
        detail));
  }
  FocusPageSoon();
  // A page of another profile makes its Browser the active one.
  ReportActiveBrowser();
}

void Window::LayOutPage() {
  // As the browser quits, Chromium closes every native window itself (Widget::CloseAllWidgets)
  // before the core's Window goes, and the app, still drawing, may set its colours then (#1478).
  if (widget_->IsClosed()) {
    return;
  }
  std::vector<content::WebContents*> shown;
  for (bc_id pane : panes_) {
    if (content::WebContents* contents = Tab::FromId(pane)) {
      shown.push_back(contents);
    }
  }
  // A page full screen takes the window, as in Chrome, and the rest of its split view waits.
  if (shown_ && shown_->IsFullscreen()) {
    shown = {shown_.get()};
  }
  const SplitLook look = panes_.empty() ? SplitLook() : LookOf(panes_.front());
  stacked_ = look.stacked;
  shares_ = look.shares;
  std::vector<platform::Pane> panes;
  size_t focused = 0;
  std::optional<gfx::Rect> focused_frame;
  const int count = static_cast<int>(shown.size());
  // Side by side or stacked, each page takes its share of the frame with the gap after it, else
  // is as wide or as tall as the others. A page full screen, alone, takes the whole frame.
  const int start = stacked_ ? page_frame_.y() : page_frame_.x();
  const int span = (stacked_ ? page_frame_.height() : page_frame_.width()) + kSplitGap;
  // An expanded split keeps the others at 220 pt whatever the frame, so its shares come from it.
  expanded_ = look.expanded;
  if (const auto at = std::ranges::find(panes_, expanded_);
      expanded_ && at != panes_.end() && shown.size() == panes_.size()) {
    shares_ = ExpandedShares(shown.size(), at - panes_.begin(), span - kSplitGap);
  }
  const bool even = shares_.size() != shown.size();
  auto edge = [&](int index) {
    return start + (even ? span * index / count
                         : base::ClampRound(span * std::accumulate(shares_.begin(),
                                                                   shares_.begin() + index, 0.0)));
  };
  for (int i = 0; i < count; ++i) {
    content::WebContents* contents = shown[i];
    Tab* tab = Tab::FromWebContents(contents);
    const int from = edge(i);
    const int to = edge(i + 1) - kSplitGap;
    const gfx::Rect frame =
        stacked_ ? gfx::Rect(page_frame_.x(), from, page_frame_.width(), to - from)
                 : gfx::Rect(from, page_frame_.y(), to - from, page_frame_.height());
    const bool died = tab && tab->crashed();
    if (contents == shown_.get()) {
      // Past the last pane while it's a page that died, so the ring stays where it was.
      focused = died ? shown.size() : panes.size();
      focused_frame = frame;
    }
    // A page that died leaves its pane to the app's card under it until it loads again.
    if (died) {
      continue;
    }
    if (tab && page_color_) {
      tab->SetPageColor(SkColorSetRGB(*page_color_ >> 16, *page_color_ >> 8 & 0xff,
                                      *page_color_ & 0xff));
    }
    // At each layout, for a page whose view changed or that comes back, before it resizes. One
    // moving from another window shows the frame it drew there, filling its view here, until it
    // draws at its new size.
    StretchOnResize(contents, page_moving_ || MovingPages().contains(Tab::IdOf(contents)));
    platform::Pane& pane = panes.emplace_back(Tab::PageViewOf(contents), frame);
    pane.color = page_color_;
    if (!tab || !tab->system_page()) {
      pane.widget = contents->GetRenderWidgetHostView();
    }
    DevToolsContentsResizingStrategy strategy;
    if (content::WebContents* devtools = DevToolsWindow::GetInTabWebContents(contents, &strategy)) {
      // As Chrome's window does: DevTools fill the frame, and the page goes where they leave it.
      platform::DockedDevTools docked = {devtools->GetNativeView(), gfx::Rect(),
                                         strategy.hide_inspected_contents()};
      gfx::Rect devtools_frame;
      ApplyDevToolsContentsResizingStrategy(strategy, gfx::Rect(pane.frame.size()),
                                            &devtools_frame, &docked.page_frame);
      // DevTools in a window of their own leave their toolbox in the tab, under the page, where it
      // draws only around a page device mode makes smaller. Under one filling the frame it would
      // show nothing, so it stays out, and its renderer hidden, until device mode leaves it room.
      const bool toolbox = !DevToolsWindow::AsDevToolsWindow(devtools);
      const bool covered = !docked.page_under && docked.page_frame == gfx::Rect(pane.frame.size());
      if (!toolbox || !covered) {
        pane.devtools = docked;
      }
      if (!covered && devtools == devtools_to_show_.get()) {
        devtools_to_show_ = nullptr;
        devtools_to_focus_ = devtools->GetWeakPtr();
        FocusPageSoon();
      }
    }
  }
  SidePanel* side_panel = ShownSidePanel();
  content::WebContents* panel_page = side_panel ? side_panel->contents() : nullptr;
  if (panel_page) {
    StretchOnResize(panel_page, page_moving_);
  }
  std::optional<platform::SidePanelPage> panel;
  // A page full screen takes the window from the side panel too.
  if (panel_page && !side_panel_frame_.IsEmpty() && !(shown_ && shown_->IsFullscreen())) {
    panel = platform::SidePanelPage{panel_page->GetNativeView(), side_panel_frame_};
  }
  platform::ShowPages(widget_.get(), panes, page_corner_radius_, focused, focus_color_, panel);
  if (panel_page != side_panel_laid_out_.get()) {
    side_panel_laid_out_ = panel_page ? panel_page->GetWeakPtr() : nullptr;
    ReportSidePanelSoon();
  }
  if (!focused_frame) {
    focused_frame = panes.empty() ? page_frame_ : panes.front().frame;
  }
  if (*focused_frame != focused_frame_) {
    focused_frame_ = *focused_frame;
    for (web_modal::ModalDialogHostObserver& observer : dialog_observers_) {
      observer.OnPositionRequiresUpdate();
    }
  }
}

void Window::ShowSpace(bc_id space, bool page) {
  const Space* in = Space::FromId(space);
  // A window with a Space of its own shows it alone, and no other window shows it.
  if (!in || space == space_ || in->own() != own_space_ || (own_space_ && space_)) {
    return;
  }
  const bc_id tab = page ? TabToShowIn(space) : 0;
  if (space_) {
    selected_in_space_[space_] = selected_;
  }
  space_ = space;
  // Posted before the tab is, so the app knows the Space when it hears which tab shows.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](base::WeakPtr<Window> window, bc_id space) {
                       if (window && Host().window_space_shown) {
                         Host().window_space_shown(Host().context, window->id(), space);
                       }
                     },
                     weak_factory_.GetWeakPtr(), space));
  if (tab) {
    Select(tab);
  } else {
    ShowNothing();
  }
}

bc_id Window::TabToShowIn(bc_id space) const {
  Space* in = Space::FromId(space);
  if (!in || space == space_) {
    return in ? selected_ : 0;
  }
  // Never a page another window shows, which would leave that window.
  const auto free = [this](bc_id tab) { return !ShownElsewhere(tab); };
  auto last = selected_in_space_.find(space);
  // A favorite split it showed shows focused where it was focused last, here or in another Space.
  const bc_id was = last == selected_in_space_.end() ? 0
                    : Favorites::Has(last->second)   ? Favorites::Focused(last->second)
                                                     : last->second;
  if (was && free(was) && (in->Has(was) || Favorites::ShownIn(was, *in))) {
    return was;
  }
  if (auto today = std::ranges::find_if(in->today(), free); today != in->today().end()) {
    return *today;
  }
  const std::vector<bc_id> pinned = in->pinned();
  auto first = std::ranges::find_if(pinned, free);
  return first == pinned.end() ? 0 : *first;
}

bc_id Window::SelectedIn(bc_id space) const {
  if (space == space_) {
    return selected_;
  }
  auto it = selected_in_space_.find(space);
  return it == selected_in_space_.end() ? 0 : it->second;
}

void Window::SetSelectedIn(bc_id space, bc_id tab) {
  selected_in_space_[space] = tab;
}

void Window::SetSidebarWidth(double width) {
  sidebar_width_ = width;
  SessionChanged();
}

void Window::ShowNothing() {
  HoldPictured();
  selected_ = 0;
  waiting_ = 0;
  for (bc_id pane : std::exchange(panes_, {})) {
    Seen(pane);
  }
  shown_ = nullptr;
  SetLetGo(false);
  LayOutPage();
  ReportSelected();
}

void Window::SetPageFrame(const gfx::Rect& frame, double corner_radius) {
  page_frame_ = frame;
  page_corner_radius_ = corner_radius;
  if (!shown_) {
    return;
  }
  // The app's frame at the window's size, so pages held for its resize are laid out here, which
  // lets them go, though a tab switch's Show leaves them waiting for it.
  const bool held = platform::PagesHeldForResize(widget_.get());
  // Off the Mac, a full-screen page's tab leaving the window, as closing it or all the window's
  // tabs does, takes the window out of full screen at once, so this frame comes from within
  // Chrome's change to the tabs, after the tab has left them (#2068). The window shows what's left
  // of the split, if anything, and the app shows the next tab: the frame never comes again for it,
  // so the pages held for the resize go now.
  if (!InTabStrip(shown_.get())) {
    std::erase_if(panes_, [](bc_id pane) {
      content::WebContents* contents = Tab::FromId(pane);
      return !contents || !InTabStrip(contents);
    });
    shown_ = nullptr;
    if (held) {
      LayOutPage();
    }
    return;
  }
  Show(shown_.get(), frame);
  if (held) {
    LayOutPage();
  }
}

void Window::SetFocusColor(uint32_t color) {
  if (focus_color_ != color) {
    focus_color_ = color;
    LayOutPage();
  }
}

void Window::SetPageColor(uint32_t color) {
  if (page_color_ != color) {
    page_color_ = color;
    LayOutPage();
  }
}

void Window::SetPageMoving(bool moving) {
  page_moving_ = moving;
  for (bc_id pane : panes_) {
    if (content::WebContents* contents = Tab::FromId(pane)) {
      StretchOnResize(contents, moving);
    }
  }
  if (SidePanel* side_panel = ShownSidePanel(); side_panel && side_panel->contents()) {
    StretchOnResize(side_panel->contents(), moving);
  }
}

void Window::SetPageScale(double scale) {
  platform::ScalePages(widget_.get(), std::isnan(scale) ? 1.0 : std::clamp(scale, 0.5, 1.0));
}

void Window::Select(bc_id tab) {
  const Tab* selecting = Tab::Get(tab);
  waiting_ = 0;
  // A window with a Space of its own shows its tabs alone, and a regular one never shows them:
  // Chrome never moves a page between incognito and regular windows.
  const Space* holding = Space::Holding(tab);
  if (!selecting ||
      (own_space_ ? !holding || holding->id() != space_
                  : (holding && holding->own()) || OffTheRecord(selecting))) {
    return;
  }
  // One whose profile is still loading, or one of its split's, can't have its page yet: the window
  // shows nothing until it's here (TabsArrived), as it would while a page starts.
  if (std::ranges::any_of(PanesWith(tab), [](bc_id pane) {
        const Tab* in = Tab::Get(pane);
        return in && in->waiting_for_profile();
      })) {
    ShowNothing();
    waiting_ = tab;
    return;
  }
  // A tab without its page, and each of its split's, gets it back here, as it's shown.
  for (bc_id pane : PanesWith(tab)) {
    if (Tab* waking = Tab::Get(pane); waking && !waking->web_contents()) {
      waking->Wake(*this);
    }
  }
  content::WebContents* contents = Tab::FromId(tab);
  tabs::TabInterface* in = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  if (!in || !ForBrowser(in->GetBrowserWindowInterface())) {
    return;
  }
  bc_id other_space = 0;
  if (Space* space = Space::Holding(tab); space && space_ && space->id() != space_) {
    other_space = space->id();
  }
  const Space* shown = Space::FromId(space_);
  if (shown && Favorites::Has(tab) && !Favorites::ShownIn(tab, *shown)) {
    // A favorite shows in the Spaces that have it: the first of them.
    auto spaces = Space::All();
    auto first = std::ranges::find_if(
        spaces, [tab](const Space* each) { return Favorites::ShownIn(tab, *each); });
    if (first == spaces.end()) {
      return;
    }
    other_space = (*first)->id();
  }
  // Here before ShowSpace looks for a tab no other window shows.
  Adopt(tab);
  // A favorite split shows focused on the tab focused last, in whichever Space it shows next.
  Favorites::SetFocused(tab);
  if (other_space) {
    // As the tab its Space shows, so the one it showed before doesn't load on the way.
    SetSelectedIn(other_space, tab);
    ShowSpace(other_space);
    return;
  }
  selected_ = tab;
  Show(contents, page_frame_);
  ReportSelected();
  UnloadIdleTabsSoon();
}

void Window::Adopt(bc_id tab) {
  for (bc_id pane : PanesWith(tab)) {
    content::WebContents* contents = Tab::FromId(pane);
    tabs::TabInterface* in =
        contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
    BrowserWindowInterface* from = in ? in->GetBrowserWindowInterface() : nullptr;
    if (!from || ForBrowser(from) == this || !ForBrowser(from)) {
      continue;
    }
    // Made before the tab leaves, so the profile always has a Browser and none closes as its last.
    BrowserWindowInterface* to =
        BrowserFor(Profile::FromBrowserContext(contents->GetBrowserContext()));
    if (!to) {
      return;
    }
    if (pane == tab) {
      LogHandover("taker takes the tab", Handing(id_, tab));
      g_adopted = tab;
    }
    if (Window* showing = Showing(pane); showing && showing != this) {
      if (pane == tab) {
        g_left_size = showing->focused_frame_.size();
      }
      HoldWhileMoving(contents);
      showing->LetGo();
      LogHandover("giver let go", Handing(showing->id(), pane));
    }
    TabStripModel* tabs = from->GetTabStripModel();
    // Its offer, which Chrome hides as the tab leaves, comes with it.
    PasswordOffer::Moving moving(pane);
    std::unique_ptr<tabs::TabModel> moved =
        tabs->DetachTabAtForInsertion(tabs->GetIndexOfWebContents(contents));
    // Not a tab opened, for the app.
    base::AutoReset<bool> opening(&opening_tab_, true);
    to->GetTabStripModel()->InsertDetachedTabAt(to->GetTabStripModel()->count(), std::move(moved),
                                                AddTabTypes::ADD_NONE);
    LogHandover("tab moved between Browsers", Handing(id_, pane));
  }
}

void Window::LetGo(bool tell) {
  panes_.clear();
  shown_ = nullptr;
  if (tell) {
    SetLetGo(true);
  } else {
    let_go_ = true;
  }
  LayOutPage();
}

void Window::SetLetGo(bool let_go) {
  if (std::exchange(let_go_, let_go) == let_go) {
    return;
  }
  // Posted, like the selection's report, so the app hears of its own call once it has returned.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](base::WeakPtr<Window> window, bc_id tab) {
                       if (window && Host().window_page_elsewhere) {
                         Host().window_page_elsewhere(Host().context, window->id(), tab);
                       }
                     },
                     weak_factory_.GetWeakPtr(), let_go ? selected_ : 0));
}

void Window::HandOver(Window& to, bool others_only) {
  std::vector<BrowserWindowInterface*> browsers;
  for (auto& [profile, held] : browsers_) {
    browsers.push_back(held.browser);
  }
  for (BrowserWindowInterface* browser : browsers) {
    TabStripModel* tabs = browser->GetTabStripModel();
    BrowserWindowInterface* into = to.BrowserFor(browser->GetProfile());
    if (!into) {
      return;
    }
    // Last first: the Browser closes as its last tab leaves, and isn't touched after.
    for (int index = tabs->count() - 1; index >= 0; --index) {
      const Space* holding = Space::Holding(Tab::IdOf(tabs->GetWebContentsAt(index)));
      if (others_only && (!holding || holding->id() == space_)) {
        continue;
      }
      std::unique_ptr<tabs::TabModel> moved = tabs->DetachTabAtForInsertion(index);
      base::AutoReset<bool> opening(&to.opening_tab_, true);
      into->GetTabStripModel()->InsertDetachedTabAt(into->GetTabStripModel()->count(),
                                                    std::move(moved), AddTabTypes::ADD_NONE);
    }
  }
}

bool Window::ShownElsewhere(bc_id tab) const {
  Window* showing = Showing(tab);
  return showing && showing != this;
}

void Window::Pick(bc_id tab) {
  if (!ShownElsewhere(tab)) {
    Select(tab);
    return;
  }
  // Only a Today tab, as a pinned tab may be unloading.
  const Space* space = Space::FromId(space_);
  const std::vector<bc_id> none;
  const std::vector<bc_id>& today = space ? space->today() : none;
  auto free = std::ranges::find_if(today, [this](bc_id each) { return !ShownElsewhere(each); });
  if (free != today.end()) {
    Select(*free);
  } else {
    ShowNothing();
  }
}

bc_id Window::Clear() {
  Space* space = Space::FromId(space_);
  if (!space) {
    return 0;
  }
  std::vector<Cleared> frame;
  const std::vector<bc_id>& today = space->today();
  // While the window shows every Today tab, there's nothing else to clear, so they go too (Arc).
  const bool last = std::ranges::all_of(
      today, [this](bc_id tab) { return std::ranges::contains(panes_, tab); });
  for (size_t index = 0; index < today.size(); ++index) {
    if (last || !Showing(today[index])) {
      const bool split = index > 0 && std::ranges::contains(space->Panes(today[index]),
                                                            today[index - 1]);
      frame.push_back({today[index], index, split, today[index] == selected_});
    }
  }
  // Bottom up, so ReopenClosedTab brings back the top one first, at its place.
  for (auto cleared = frame.rbegin(); cleared != frame.rend(); ++cleared) {
    CloseTab(cleared->tab);
  }
  if (frame.empty()) {
    return 0;
  }
  const bc_id id = NewId();
  cleared_.push_back({id, std::move(frame)});
  if (cleared_.size() > kMaxClears) {
    cleared_.erase(cleared_.begin());
  }
  return id;
}

size_t Window::UndoClear(bc_id clear) {
  auto it = std::ranges::find(cleared_, clear, &ClearedTabs::id);
  if (it == cleared_.end()) {
    return 0;
  }
  std::vector<Cleared> frame = std::move(it->tabs);
  cleared_.erase(it);
  size_t reopened = 0;
  bc_id above = 0;
  bc_id shown = 0;
  // Top down, each at its place before the Clear, which puts back the order with the tab kept.
  for (const Cleared& cleared : frame) {
    // Not one whose page kept it open, or one already reopened.
    std::optional<ClosedTab> tab = TakeClosed(cleared.tab);
    const bc_id id = tab ? Reopen(tab->space, cleared.index, tab->saved) : 0;
    if (id) {
      ++reopened;
      // Back in its split, if the tab beside it came back too.
      if (Space* space = Space::Holding(id);
          space && cleared.split && above && space->Split(id, above, GURL())) {
        RemakeSplit(*space, id, *tab);
      }
      if (cleared.selected) {
        shown = id;
      }
    }
    above = id;
  }
  // A Clear that closed the tab shown shows it again, with its split.
  if (shown) {
    Select(shown);
  }
  return reopened;
}

bc_id Window::UndoDeleteSpace(bc_id id) {
  // The regular windows' Spaces, which a window of its own never shows.
  if (own_space_) {
    return 0;
  }
  std::vector<DeletedSpace>& deleted = DeletedSpaces();
  auto it = std::ranges::find(deleted, id, &DeletedSpace::id);
  Profile* profile = it == deleted.end() ? nullptr : ProfileFor(it->saved.profile);
  // Not while a page keeps one of its tabs open, which keeps the Space.
  if (!profile || Space::FromId(id)) {
    return 0;
  }
  DeletedSpace space = std::move(*it);
  deleted.erase(it);
  // Its Today tabs leave the archive and the closed tabs as they come back, but for those that
  // came back some other way, such as ⇧⌘T, or left the archive.
  const int pinned = static_cast<int>(space.saved.pinned.size());
  const int shown = space.saved.shown;
  std::vector<SavedTab> today;
  bool above = false;
  for (size_t i = 0; i < space.today.size(); ++i) {
    const bc_id tab = space.today[i];
    const int index = pinned + static_cast<int>(i);
    if (!std::ranges::contains(Archived(), tab, &ArchivedTab::tab)) {
      if (index == shown) {
        space.saved.shown = -1;
      } else if (index < shown) {
        --space.saved.shown;
      }
      above = false;
      continue;
    }
    if (!TakeClosed(tab)) {
      Unarchive(tab);
    }
    SavedTab& saved = today.emplace_back(std::move(space.saved.today[i]));
    saved.split = saved.split && above;
    above = true;
  }
  space.saved.today = std::move(today);
  RestoreSpace(*this, DirOf(profile), space.saved, space.id, space.at);
  // Not one of the Space's own, which closed with it and came back as other tabs.
  if (Favorites::Has(space.shown) && !Favorites::SpaceOf(space.shown)) {
    SetSelectedIn(space.id, space.shown);
  }
  ShowSpace(space.id);
  return space.id;
}

bc_id Window::ReopenClosedTab() {
  // A private window keeps none of its own and reopens none of the others'.
  if (ClosedTabs().empty() || off_the_record_) {
    return 0;
  }
  const ClosedTab& last = ClosedTabs().back();
  // A Little ilo window's comes back in a new one, which the app opens, as Arc's ⇧⌘T reopens it in
  // Little Arc. Posted, so the app hears of it once its own call has returned. With Little ilo off
  // it comes back as a Today tab, as a regular window's does.
  if (last.little && UserSettings().little_windows) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](base::WeakPtr<Window> window, bc_id closed, bc_id profile) {
                         if (window && profile && Host().window_reopen_little) {
                           Host().window_reopen_little(Host().context, window->id(), closed,
                                                       profile);
                         }
                       },
                       weak_factory_.GetWeakPtr(), last.id, ProfileId(last.profile)));
    return 0;
  }
  // The rest are the regular windows', which a window of its own never shows.
  if (own_space_) {
    return 0;
  }
  return ReopenClosed(last.id, /*show=*/true);
}

bc_id Window::ReopenClosed(bc_id id, bool show) {
  // A Little ilo window's tab comes back only in a Little ilo window with no tab yet, and a regular
  // window's only in a regular one.
  auto found = std::ranges::find(ClosedTabs(), id, &ClosedTab::id);
  const Space* own = own_space_ ? Space::FromId(space_) : nullptr;
  const bool fits =
      found != ClosedTabs().end() &&
      (own_space_ ? own && own->little() && found->little && own->today().empty()
                  : !found->little);
  std::optional<ClosedTab> taken = fits ? TakeClosed(id) : std::nullopt;
  if (!taken) {
    return 0;
  }
  const ClosedTab closed = std::move(*taken);
  // In this Little ilo window's Space, not the one it closed in, which may still be open.
  const bc_id tab = own_space_ ? Reopen(space_, 0, closed.saved)
                               : Reopen(closed.space, closed.index, closed.saved);
  if (!tab) {
    return 0;
  }
  // Tabs that closed with it in its split view, as its row's ✕ closes them, rejoin it as they come
  // back.
  for (ClosedTab& other : ClosedTabs()) {
    if (other.split_with == closed.id) {
      other.split_with = tab;
    }
  }
  // Back in its split view, at its place, if a tab of it is still open and the split has room.
  Space* space = Space::Holding(tab);
  const bool remade = space && space->Panes(closed.split_with).size() == 1;
  if (space && closed.split_with &&
      space->Split(tab, closed.split_with, Tab::Get(tab)->Url(), /*left=*/false)) {
    space->MoveInSplit(tab, closed.split_index);
    // A split it remakes is as it was. One still open keeps how it's shown now.
    if (remade) {
      RemakeSplit(*space, tab, closed);
    }
    PanesChanged();
  }
  // A Little ilo window shows its one tab, as it has nothing else to show.
  if (show || own_space_) {
    Select(tab);
  }
  return tab;
}

bool Window::RunExtension(const std::string& id, const gfx::Rect& anchor) {
  content::WebContents* contents = Tab::FromId(selected_);
  BrowserWindowInterface* browser =
      contents ? BrowserFor(Profile::FromBrowserContext(contents->GetBrowserContext())) : nullptr;
  if (!browser) {
    return false;
  }
  // Chrome's popup hangs from a view, so a view of no content stands where the app's anchor is.
  if (!popup_anchor_.view()) {
    popup_anchor_.SetView(
        widget_->GetContentsView()->AddChildView(std::make_unique<views::View>()));
  }
  popup_anchor_.view()->SetBoundsRect(anchor);
  // An action that opens the extension's side panel opens it here, as Chrome's toolbar does with
  // what Chrome's runner answers: not granting the tab, as the panel outlives it.
  const extensions::Extension* extension =
      extensions::ExtensionRegistry::Get(contents->GetBrowserContext())
          ->enabled_extensions()
          .GetByID(id);
  extensions::ExtensionActionRunner* runner =
      extensions::ExtensionActionRunner::GetForWebContents(contents);
  extensions::SidePanelService* service =
      extensions::SidePanelService::Get(contents->GetBrowserContext());
  if (!off_the_record_ && extension && runner && service && !runner->GetBlockedActions(id) &&
      service->HasSidePanelActionForTab(*extension,
                                        sessions::SessionTabHelper::IdForTab(contents).id())) {
    extensions::side_panel_util::ToggleExtensionSidePanel(browser, id);
    return true;
  }
  return browser_core::RunExtension(id_, browser, contents, id, popup_anchor_.view());
}

void Window::ShowEveryExtensionAction(bool every) {
  // Made with the first tab shown, before the panel can show for one.
  if (actions_) {
    actions_->ShowEvery(every);
  }
}

void Window::SetSidePanelFrame(const gfx::Rect& frame) {
  if (frame != side_panel_frame_) {
    side_panel_frame_ = frame;
    LayOutPage();
  }
}

void Window::CloseSidePanel() {
  if (SidePanel* side_panel = ShownSidePanel()) {
    side_panel->Close();
  }
}

bool Window::HasSidePanel(const std::string& id) {
  content::WebContents* contents = Tab::FromId(selected_);
  // A private window has none.
  if (!contents || off_the_record_) {
    return false;
  }
  const extensions::Extension* extension =
      extensions::ExtensionRegistry::Get(contents->GetBrowserContext())
          ->enabled_extensions()
          .GetByID(id);
  extensions::SidePanelService* service =
      extensions::SidePanelService::Get(contents->GetBrowserContext());
  // As Chrome's extension menu asks before it offers the item.
  return extension && service &&
         extension->permissions_data()->HasAPIPermission(
             extensions::mojom::APIPermissionID::kSidePanel) &&
         service->HasSidePanelContextMenuActionForTab(
             *extension, sessions::SessionTabHelper::IdForTab(contents).id());
}

void Window::ToggleSidePanel(const std::string& id) {
  content::WebContents* contents = Tab::FromId(selected_);
  tabs::TabInterface* tab = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  if (tab && HasSidePanel(id)) {
    extensions::side_panel_util::ToggleExtensionSidePanel(tab->GetBrowserWindowInterface(), id);
  }
}

void Window::SetSidePanelWidth(int width) {
  if (SidePanel* side_panel = ShownSidePanel()) {
    side_panel->SaveWidth(width);
  }
}

void Window::SidePanelChanged(BrowserWindowInterface* browser) {
  SidePanel* side_panel = ShownSidePanel();
  if (side_panel && side_panel->browser() == browser) {
    LayOutPage();
    // A panel closed with the keyboard leaves it with the window, and the page takes it back.
    FocusPageSoon();
  }
  ReportSidePanelSoon();
}

SidePanel* Window::ShownSidePanel() const {
  // A private window shows none, whatever an extension allowed in it asks.
  tabs::TabInterface* tab = shown_ && !off_the_record_
                                ? tabs::TabInterface::MaybeGetFromContents(shown_.get())
                                : nullptr;
  BrowserWindowInterface* browser = tab ? tab->GetBrowserWindowInterface() : nullptr;
  return browser && ForBrowser(browser) == this ? SidePanel::From(browser) : nullptr;
}

void Window::ReportSidePanelSoon() {
  if (side_panel_report_posted_) {
    return;
  }
  side_panel_report_posted_ = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Window::ReportSidePanel, weak_factory_.GetWeakPtr()));
}

void Window::ReportSidePanel() {
  side_panel_report_posted_ = false;
  if (!Host().window_side_panel) {
    return;
  }
  SidePanel* side_panel = ShownSidePanel();
  content::WebContents* page = side_panel ? side_panel->contents() : nullptr;
  std::optional<ToldSidePanel> told;
  if (page) {
    std::vector<uint8_t> icon;
    if (const gfx::Image image = side_panel->icon(); !image.IsEmpty()) {
      icon = gfx::PNGCodec::EncodeBGRASkBitmap(
                 image.AsImageSkia().GetRepresentation(2).GetBitmap(),
                 /*discard_transparency=*/false)
                 .value_or(std::vector<uint8_t>());
    }
    told = ToldSidePanel{.extension = side_panel->extension_id(),
                         .title = base::UTF16ToUTF8(page->GetTitle()),
                         .icon = std::move(icon),
                         .width = side_panel->SavedWidth()};
  }
  if (told == side_panel_told_) {
    return;
  }
  side_panel_told_ = std::move(told);
  if (!side_panel_told_) {
    Host().window_side_panel(Host().context, id_, nullptr);
    return;
  }
  const extensions::Extension* extension =
      extensions::ExtensionRegistry::Get(page->GetBrowserContext())
          ->enabled_extensions()
          .GetByID(side_panel_told_->extension);
  const std::string name = extension ? extension->name() : std::string();
  const bc_side_panel shown = {
      .extension = side_panel_told_->extension.c_str(),
      .name = name.c_str(),
      .icon = side_panel_told_->icon.empty() ? nullptr : side_panel_told_->icon.data(),
      .icon_size = side_panel_told_->icon.size(),
      .title = side_panel_told_->title.c_str(),
      .width = static_cast<double>(side_panel_told_->width)};
  Host().window_side_panel(Host().context, id_, &shown);
}

bool Window::RunExtensionCommand(const std::string& id, const std::string& name) {
  content::WebContents* contents = Tab::FromId(selected_);
  return contents && browser_core::RunExtensionCommand(contents, id, name);
}

bc_id Window::RestoreArchived(bc_id id) {
  // The archive is the regular windows'.
  if (own_space_) {
    return 0;
  }
  std::optional<ArchivedTab> archived = TakeArchived(id);
  if (!archived) {
    return 0;
  }
  // One closed since the browser started comes back with its history, else at its page.
  std::optional<ClosedTab> closed = TakeClosed(archived->tab);
  SavedTab saved;
  if (closed) {
    saved = std::move(closed->saved);
  } else {
    sessions::SerializedNavigationEntry& page = saved.history.emplace_back();
    page.set_virtual_url(archived->url);
    page.set_original_request_url(archived->url);
    page.set_title(base::UTF8ToUTF16(archived->title));
  }
  // Back in the Space it closed in, as Arc's, while that's still one of the user's, which Select
  // then shows. Else in the one shown.
  const Space* closed_in = Space::WithNode(archived->space);
  const bc_id tab = Reopen(closed_in ? closed_in->id() : space_, 0, saved);
  Select(tab);
  return tab;
}

bc_id Window::Duplicate(bc_id tab) {
  const Tab* from = Tab::Get(tab);
  Space* space = Favorites::Has(tab) ? Space::FromId(space_) : Space::Holding(tab);
  if (!from || !from->profile() || !space || DirOf(from->profile()) != space->profile() ||
      OffTheRecord(from) != off_the_record_) {
    return 0;
  }
  content::WebContents* contents = from->web_contents();
  bc_id id = 0;
  if (!contents) {
    // A copy of what it keeps, whose page loads as it's shown.
    id = Tab::Restore(from->profile(), from->Save());
  }
  // In this window's Browser, wherever |tab|'s page is, as it shows here.
  BrowserWindowInterface* browser = contents ? BrowserFor(from->profile()) : nullptr;
  if (browser) {
    id = CloneInto(*browser, *contents);
  }
  if (!id) {
    return 0;
  }
  auto at = std::ranges::find(space->today(), tab);
  space->AddAt(id, at == space->today().end() ? 0 : at - space->today().begin() + 1);
  Select(id);
  return id;
}

bc_id Window::OpenHistory(bc_id tab, int offset) {
  const Tab* from = Tab::Get(tab);
  Space* space = Favorites::Has(tab) ? Space::FromId(space_) : Space::Holding(tab);
  if (!from || !from->profile() || !space || DirOf(from->profile()) != space->profile() ||
      OffTheRecord(from) != off_the_record_) {
    return 0;
  }
  // A system page's history is WebKit's, of which the saved tab keeps only the page.
  if (offset != 0 && from->system_page()) {
    return 0;
  }
  // As the tab would come back after a quit, at the other page: Chrome's background tab loads
  // at once, but here a page loads once it's shown.
  SavedTab saved = from->Save();
  saved.current += offset;
  if (saved.current < 0 || saved.current >= static_cast<int>(saved.history.size())) {
    return 0;
  }
  const bc_id id = Tab::Restore(from->profile(), saved);
  if (!id) {
    return 0;
  }
  // Opened behind by |tab|, so it goes under it and closing it goes back there, as a link's does.
  space->Add(id, tab, /*foreground=*/false);
  return id;
}

bc_id Window::CloneInto(BrowserWindowInterface& browser, content::WebContents& contents) {
  TabStripModel* tabs = browser.GetTabStripModel();
  // With its session storage, as Chrome's duplicate.
  std::unique_ptr<content::WebContents> copy = contents.Clone();
  content::WebContents* copied = copy.get();
  {
    base::AutoReset<bool> opening(&opening_tab_, true);
    tabs->InsertWebContentsAt(tabs->count(), std::move(copy), AddTabTypes::ADD_NONE);
  }
  const bc_id id = Tab::IdOf(copied);
  Tab* from = Tab::FromWebContents(&contents);
  Tab* to = Tab::Get(id);
  to->set_name(from->name());
  to->set_icon(from->icon());
  if (contents.IsAudioMuted()) {
    to->SetMuted(true);
  }
  to->Restored();
  return id;
}

bc_id Window::Reopen(bc_id space_id, size_t index, const SavedTab& saved) {
  Space* space = Space::FromId(space_id);
  if (!space) {
    space = Space::FromId(space_);
    index = 0;
  }
  Profile* profile = space ? ProfileFor(space->profile()) : nullptr;
  // Without its page, which it gets back once it's shown.
  const bc_id tab = space && profile && !closing_ ? Tab::Restore(profile, saved) : 0;
  if (!tab) {
    return 0;
  }
  space->AddAt(tab, index);
  return tab;
}

void Window::SelectAfterClosing() {
  const bc_id next = std::exchange(next_after_closing_, 0);
  if (next && Tab::Get(next)) {
    Pick(next);
  } else {
    // Laid out with no page, as after an unload or in an empty Space, so no pane keeps the closed
    // tab's: on Windows one is a window of its own, which went on drawing over the Space (#1343).
    ShowNothing();
  }
}

void Window::ReportSelected() {
  SessionChanged();
  // Posted, like the Space's changes, so the app hears of its own call once it has returned.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](base::WeakPtr<Window> window, bc_id tab) {
                       // Not for a window that's closing, or whose native window Chromium has
                       // destroyed as the browser quits: the app would set its title (#474).
                       if (window && (window->closing_ || window->widget_->IsClosed())) {
                         window = nullptr;
                       }
                       if (window && Host().window_selected) {
                         Host().window_selected(Host().context, window->id(), tab);
                       }
                       // After the app has heard of the switch, which it times.
                       if (window) {
                         if (!window->actions_) {
                           window->actions_ = std::make_unique<WindowActions>(window->id());
                         }
                         window->actions_->Show(Tab::FromId(tab));
                       }
                       // The page the window left, if another has a picture of it.
                       AdoptFreedPages();
                     },
                     weak_factory_.GetWeakPtr(), selected_));
}

void Window::Close() {
  if (closing_) {
    return;
  }
  // Its tabs are in Spaces every regular window shows, so with another one open they go there,
  // live, and no page closes. A window with a picture of a page this one showed shows it again. A
  // window of its own's tabs close with it.
  Window* to = nullptr;
  for (Window* window : All()) {
    if (window != this && !window->closing_ && !window->own_space_) {
      to = window;
      break;
    }
  }
  const bool hand_over = to && !own_space_ && !browser_shutdown::IsTryingToQuit();
  // A Little ilo window's page opens its tabs behind, in its home Space (the app moves them there),
  // so they stay in its Browser until a window shows them, and outlive it.
  if (to && own_space_ && !off_the_record_ && !browser_shutdown::IsTryingToQuit()) {
    HandOver(*to, /*others_only=*/true);
  }
  if (hand_over) {
    for (bc_id pane : panes_) {
      HoldWhileMoving(Tab::FromId(pane));
    }
    // Its pages out of its view first, as the view goes with the window. The app has no use for a
    // picture of a page for a window that is closing.
    LetGo(false);
    HandOver(*to);
  }
  closing_ = true;
  // Not Chrome's close-cancelled callback: it fires too as the pages' unload handlers hold the
  // close, which then goes on, and the tabs it closed were archived and the window stayed (#870).
  close_stopped_ = chrome::AddClosingAllBrowsersCallback(
      base::BindRepeating(&Window::OnCloseStopped, base::Unretained(this)));
  if (hand_over) {
    AdoptFreedPages();
  }
  if (browsers_.empty()) {
    Destroy();
    return;
  }
  std::vector<BrowserWindowInterface*> browsers;
  for (auto& [profile, held] : browsers_) {
    browsers.push_back(held.browser);
  }
  for (BrowserWindowInterface* browser : browsers) {
    browser->GetWindow()->Close();
  }
}

void Window::OnCloseStopped(bool closing) {
  if (closing || browsers_.empty() || std::ranges::any_of(browsers_, [](const auto& entry) {
        return entry.second.browser->capabilities()->IsAttemptingToCloseBrowser();
      })) {
    return;
  }
  closing_ = false;
  close_stopped_ = {};
}

bool Window::KeepsBrowsers() const {
  return off_the_record_ && !closing_ && !browser_shutdown::IsTryingToQuit();
}

void Window::OnBrowserGone(BrowserWindowInterface* browser) {
  std::erase_if(browsers_, [browser](const auto& entry) { return entry.second.browser == browser; });
  // Chrome destroys an off-the-record profile as the last of its Browsers goes, right after this
  // returns when no page of it is left, so what the window built on it goes now. The core keeps a
  // regular profile loaded, and DeleteProfile lets go of what windows built on it.
  if (Profile* profile = browser->GetProfile();
      profile->IsOffTheRecord() && !browsers_.contains(profile)) {
    LetProfileGo(profile);
  }
  if (closing_ && browsers_.empty()) {
    // Posted: the Browser is part way through its destructor.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Window::Destroy, weak_factory_.GetWeakPtr()));
  }
}

void Window::AskAboutDownloads(int count,
                               bool last_private,
                               base::OnceCallback<void(bool)> answer) {
  // Chrome stops the quit or close that asked before, which is asked again.
  if (downloads_answer_) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(std::move(downloads_answer_), false));
  }
  // With no app to ask, it goes on, cancelling them.
  if (!Host().window_downloads_question) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(std::move(answer), true));
    return;
  }
  downloads_answer_ = std::move(answer);
  Host().window_downloads_question(Host().context, id_, count, last_private);
}

void Window::AnswerDownloads(bool cancel) {
  if (!downloads_answer_) {
    return;
  }
  // Posted, as a yes closes windows, which the app hears of, from inside its own call.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(std::move(downloads_answer_), cancel));
}

void Window::Destroy() {
  // Chrome isn't left waiting for an answer no one can give.
  if (downloads_answer_) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(std::move(downloads_answer_), false));
  }
  // The last regular window stays in the session at its frame, saved while it's listed, for the
  // next window to open there. One that had tabs was kept as they closed (OnTabClosing).
  const bool last_regular = std::ranges::none_of(Windows(), [this](const auto& entry) {
    const Window* other = entry.second.get();
    return other != this && !other->own_space_ && !other->closing_;
  });
  if (!own_space_ && last_regular) {
    KeepSession();
  }
  // Its tabs without pages close as its pages did: a window of its own's, and with the last regular
  // window every regular one. Quitting, they stay for the next launch.
  if ((own_space_ || last_regular) && !browser_shutdown::IsTryingToQuit()) {
    for (Tab* tab : Tab::All()) {
      const Space* space = Space::Holding(tab->id());
      const bool its = own_space_ ? space && space->id() == space_
                                  : !(space && space->own()) && !OffTheRecord(tab);
      if (!tab->web_contents() && its) {
        // A Little ilo window's and a Blank Window's go in the archive, as one with its page does.
        if (space && (space->little() || space->blank())) {
          KeepClosed(tab->id());
        }
        tab->Close();
      }
    }
  }
  // Out of the list first, so the app can't reach the window from its callback.
  std::erase(Order(), id_);
  auto it = Windows().find(id_);
  std::unique_ptr<Window> self = std::move(it->second);
  Windows().erase(it);
  // Its widget can report itself active as it closes, as on Windows when an owned window goes
  // first, which would put the closed window back in Order (#1687).
  widget_observation_.Reset();
  // What it built on its profiles, before the Space goes and ends a private session: not left to
  // the order of its members.
  suggestions_.reset();
  widget_->CloseNow();
  // A window's own Space goes with it, its tabs closed, and the last private window's session
  // ends. A regular one leaves the session while another stays open; the last stays in it, for
  // the next launch to open again.
  if (own_space_) {
    const Space* space = Space::FromId(space_);
    const std::string profile = space ? space->profile() : std::string();
    Space::Delete(space_);
    // A Little ilo window has no session of its own to end.
    if (off_the_record_ && !profile.empty()) {
      EndPrivateSession(ProfileFor(profile));
    } else if (off_the_record_) {
      LOG(ERROR) << "browser: private window " << id_
                 << " closed with no Space to end its session";
    }
  } else if (std::ranges::any_of(Windows(), [](const auto& entry) {
               return !entry.second->own_space_;
             })) {
    SessionChanged();
  }
  InstallQuestion::WindowClosing(id_);
  if (Host().window_closed) {
    Host().window_closed(Host().context, id_);
  }
}

gfx::NativeView Window::GetHostView() const {
  return widget_->GetNativeView();
}

gfx::Point Window::GetDialogPosition(const gfx::Size& size) {
  // As in Chrome's window: a tab's dialog, such as print preview, is centred over the page, at its
  // top. In a split view, over the focused page.
  return gfx::Point(focused_frame_.CenterPoint().x() - size.width() / 2, focused_frame_.y());
}

gfx::Size Window::GetMaximumDialogSize() {
  return focused_frame_.size();
}

bool Window::ShouldConstrainDialogBoundsByHost() {
  // Unconstrained, Chrome gives the dialog a window of its own (constrained_window_views.cc).
  return !platform::PageDialogsInOwnWindow();
}

void Window::AddObserver(web_modal::ModalDialogHostObserver* observer) {
  dialog_observers_.AddObserver(observer);
  // A tab's dialog places itself through this as it's made, so its keys are asked about from its
  // first, and again once it shows: the Mac lists a child window only once it's on screen.
  dialog_keys::Watch(widget_.get());
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](base::WeakPtr<Window> window) {
                       if (window) {
                         dialog_keys::Watch(window->widget());
                       }
                     },
                     weak_factory_.GetWeakPtr()));
}

void Window::RemoveObserver(web_modal::ModalDialogHostObserver* observer) {
  dialog_observers_.RemoveObserver(observer);
}

void Window::OnTabStripModelChanged(TabStripModel* tab_strip_model,
                                    const TabStripModelChange& change,
                                    const TabStripSelectionChange& selection) {
  if (change.type() == TabStripModelChange::kRemoved) {
    // Tabs closing with the window, or as the browser quits, come back at the next launch instead.
    // A Little ilo window's and a Blank Window's, which never come back with a launch, go in the
    // archive as they close with their window (KeepClosed).
    const Space* own = own_space_ ? Space::FromId(space_) : nullptr;
    if ((closing_ && !(own && (own->little() || own->blank()))) ||
        browser_shutdown::IsTryingToQuit()) {
      return;
    }
    for (const TabStripModelChange::RemovedTab& removed : change.GetRemove()->contents) {
      // An unloaded tab's page goes, and the tab stays.
      const Tab* tab = Tab::FromWebContents(removed.contents);
      if (removed.remove_reason == TabRemovedReason::kDeleted && !(tab && tab->unloading())) {
        KeepClosed(Tab::IdOf(removed.contents));
      }
    }
    return;
  }
  if (change.type() != TabStripModelChange::kInserted) {
    return;
  }
  // Each tab has Chrome's tab helpers now: a tab made before them observes theirs, and a tab
  // MakeHeldBy made is its Space's and starts its page, one this window opens itself too.
  for (const TabStripModelChange::ContentsWithIndex& inserted : change.GetInsert()->contents) {
    Tab::HelpersArrived(inserted.contents);
    HeldLoad::Start(inserted.contents);
  }
  if (opening_tab_) {
    return;
  }
  const bc_id split_with = std::exchange(split_with_, 0);
  const bc_id agent_opener = std::exchange(agent_opener_, 0);
  const std::optional<bc_rect> popup = std::exchange(popup_, std::nullopt);
  const bool popup_placed = std::exchange(popup_placed_, false);
  for (const TabStripModelChange::ContentsWithIndex& inserted : change.GetInsert()->contents) {
    tabs::TabInterface* opener = tab_strip_model->GetOpenerOfTabAt(inserted.index);
    bc_id opened_by = agent_opener;
    if (!opened_by && opener) {
      opened_by = Tab::IdOf(opener->GetContents());
    }
    // Posted: the app shows the tab, and a tab strip can't change while it reports a change.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE,
        base::BindOnce(&Window::ReportTabOpened, weak_factory_.GetWeakPtr(),
                       Tab::IdOf(inserted.contents), opened_by,
                       !agent_opener && tab_strip_model->active_index() == inserted.index,
                       split_with, popup, popup_placed));
  }
}

void Window::OnWidgetActivationChanged(views::Widget* widget, bool active) {
  // Chrome's focus manager gives the page the keyboard back as its window becomes key, but the
  // core shows pages outside views. Without this, a page that commits while the window isn't key
  // leaves the keyboard with nothing: Chromium gives none to its new view, and the old one is gone.
  if (active) {
    if (std::exchange(left_, false)) {
      CameBackTo(*this);
    }
    // At once where the window's focus has settled by now, so a key read right after the
    // activation, as one typed as a dialog over the window closes, reaches the page (#1711).
    if (platform::FocusSettledOnActivation()) {
      FocusPage();
    } else {
      FocusPageSoon();
    }
    // The session keeps the Space of the window active last.
    if (ToFront(id_)) {
      SessionChanged();
    }
    UnloadIdleTabsSoon();
    if (Host().window_activated) {
      Host().window_activated(Host().context, id_);
    }
    sync::WindowActivated();
  } else {
    // A Little ilo window's auto-archive counts from here.
    inactive_since_ = base::Time::Now();
    if (little()) {
      ScheduleAutoArchive();
    }
    // Posted: on the Mac, an overlay's window is key only once the window has let go.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Window::CheckLeft, weak_factory_.GetWeakPtr()));
  }
  ReportActiveBrowser();
}

void Window::CheckLeft() {
  if (active()) {
    return;
  }
  // A menu open as the user goes to another app or window lets the keyboard go there after.
  if (platform::OverlayHasKeyboard(widget_.get())) {
    platform::WhenOverlayLetsGo(widget_.get(),
                                base::BindOnce(&Window::CheckLeft, weak_factory_.GetWeakPtr()));
    return;
  }
  if (!MiniPlayerHasKeyboard()) {
    left_ = true;
  }
}

bool Window::little() const {
  const Space* own = own_space_ ? Space::FromId(space_) : nullptr;
  return own && own->little();
}

bool Window::active() const {
  return widget_ && widget_->IsActive();
}

void Window::ReportActiveBrowser() {
  // Chrome orders its Browsers by these reports, and picks the window by that order for what has
  // no tab of its own, such as an extension popup's install question or a page's alert.
  const bool key = widget_ && widget_->IsActive();
  if (key && !shown_) {
    // A key window showing no page, as for an empty Space, may hold no Browser of its Space's
    // profile, as a launch puts tabs back without their pages: it makes one, from a task of its
    // own, as this can come while a Browser closes.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Window::ActivateSpaceBrowser, weak_factory_.GetWeakPtr()));
    return;
  }
  tabs::TabInterface* tab = key ? tabs::TabInterface::MaybeGetFromContents(shown_.get()) : nullptr;
  MarkActive(tab ? tab->GetBrowserWindowInterface() : nullptr);
}

void Window::ActivateSpaceBrowser() {
  if (closing_ || shown_ || !widget_ || !widget_->IsActive()) {
    return;
  }
  // Its Space's profile's Browser is the active one, so what Chrome places in the window active
  // last, such as an extension's chrome.tabs.create with no window, finds this window rather than
  // none, or opens one of Chrome's own (#1831). A private or Little ilo window, or one whose
  // Space's profile isn't loaded, keeps the Browser it had active.
  const Space* space = own_space_ ? nullptr : Space::FromId(space_);
  Profile* profile = space && !space->agent() ? ProfileFor(space->profile()) : nullptr;
  if (BrowserWindowInterface* browser =
          profile && DirOf(profile) == space->profile() ? BrowserFor(profile) : nullptr) {
    MarkActive(browser);
  }
}

void Window::MarkActive(BrowserWindowInterface* shown) {
  BrowserWindowInterface* active = nullptr;
  for (auto& [profile, held] : browsers_) {
    if (held.browser->IsDeleteScheduled()) {
      continue;
    }
    if (held.browser == shown) {
      active = held.browser;
    } else if (auto* state = BrowserActiveStateManager::From(held.browser.get())) {
      state->DidBecomeInactive();
    }
  }
  auto* state = BrowserActiveStateManager::From(active);
  if (!state || state->IsActive()) {
    return;
  }
  state->DidBecomeActive();
  // Chrome also makes the active Browser's profile the one the next launch starts with, but the
  // core's start profile is the one new Spaces take and that can't be deleted, so it stays put.
  Profile* start = StartProfile();
  if (start && active->GetProfile()->GetOriginalProfile() != start) {
    profiles::SetLastUsedProfile(start->GetBaseName());
  }
}

void Window::OnWidgetVisibilityChanged(views::Widget* widget, bool visible) {
  // As Chrome's views host does, the pages hide while the window is out of sight, minimized or
  // with the app hidden, so Chromium counts them hidden and they stop drawing and slow their
  // timers. Only the window's being covered reaches them otherwise, as occluded.
  if (visible) {
    LayOutPage();
    FocusPageSoon();
    return;
  }
  // A page view hidden with the keyboard gives it to another view.
  page_had_keyboard_ = page_had_keyboard_ || platform::KeyboardForPage(widget_.get());
  platform::HidePages(widget_.get());
}

void Window::LeaveFullScreen() {
  // The app lays a page leaving out once, at the window's new size, which off the Mac it hears of
  // after the window has resized.
  if (FullScreenForPage()) {
    platform::HoldPagesForResize(widget_.get());
  }
#if !BUILDFLAG(IS_MAC)
  // The page hears it's out before the window resizes, as on the Mac. Each Browser found again:
  // the controller's calls reach code that may change them.
  std::vector<Profile*> profiles;
  for (const auto& [profile, held] : browsers_) {
    profiles.push_back(profile);
  }
  base::WeakPtr<Window> letting_go = weak_factory_.GetWeakPtr();
  for (Profile* profile : profiles) {
    const auto held = browsers_.find(profile);
    if (held == browsers_.end()) {
      continue;
    }
    static_cast<BrowserWindowStandIn*>(held->second.browser->GetWindow())->LetGoOfPage();
    if (!letting_go) {
      return;
    }
  }
  // Off the Mac the window is out as SetFullscreen returns. The window manager's own way out has
  // taken it out already, which Chromium's X11 window ignores, still counting itself full screen.
  if (widget_->IsFullscreen()) {
    base::WeakPtr<Window> alive = weak_factory_.GetWeakPtr();
    widget_->SetFullscreen(false);
    // The window's resize runs the app, which may close it.
    if (!alive) {
      return;
    }
  }
#endif
  // Chrome's controller keeps a page full screen until it hears the window is out, ignoring the
  // page's own exit meanwhile, so a window AppKit took out (its green button, ⌃⌘F, its display
  // going) without finishing kept the page full screen in a window that wasn't. On the Mac Chrome
  // lets go of the page before leaving when it asks itself, so only the window's own exit gets
  // here with one; off the Mac the stand-ins have let go of it above.
  for (const auto& [profile, held] : browsers_) {
    ExclusiveAccessManager* manager = ExclusiveAccessManager::From(held.browser);
    FullscreenController* controller = manager ? manager->fullscreen_controller() : nullptr;
    if (controller && controller->IsWindowFullscreenForTabOrPending()) {
      controller->WindowFullscreenStateChanged();
    }
  }
}

gfx::Rect Window::RestoredFrame() const {
  if (!widget_) {
    return gfx::Rect();
  }
  const gfx::Rect restored = widget_->GetRestoredBounds();
#if BUILDFLAG(IS_LINUX)
  // Chromium knows the frame only when it asked for the maximize itself; for the window manager's,
  // the bounds it gives are the maximized frame.
  if (widget_->IsMaximized() && !widget_->IsFullscreen() &&
      restored == widget_->GetWindowBoundsInScreen()) {
    return FrameBeforeMaximized(normal_frames_, restored);
  }
#endif
  return restored;
}

bool Window::FullScreenForPage() const {
  for (const auto& [profile, held] : browsers_) {
    ExclusiveAccessManager* manager = ExclusiveAccessManager::From(held.browser);
    FullscreenController* controller = manager ? manager->fullscreen_controller() : nullptr;
    if (controller && controller->IsFullscreenCausedByTab()) {
      return true;
    }
  }
  return false;
}

void Window::OnWidgetBoundsChanged(views::Widget* widget, const gfx::Rect& new_bounds) {
#if BUILDFLAG(IS_LINUX)
  if (!widget->IsMaximized() && !widget->IsFullscreen() && !widget->IsMinimized()) {
    KeepNormalFrame(normal_frames_, widget->GetWindowBoundsInScreen());
  }
#endif
  // The session keeps each regular window's frame, saved a while after it moves, so a crash
  // keeps it too.
  if (!own_space_) {
    SessionChanged();
  }
}

void Window::GiveKeyboardToPage() {
  // Held by the window, the keyboard is the page's (KeyboardForPage), as when the page shown before
  // left with it, so FocusPage gives it to the page shown. Given now, as a closing command bar
  // leaves it with the page: a key that came first would reach the window, where nothing takes it.
  // A page still loading has no view to take it yet, and takes it once its view comes.
  platform::ReleaseKeyboard(widget_.get());
  FocusPage();
  FocusPageSoon();
}

void Window::OverlayTakesKeyboard(bool takes) {
  if (takes) {
    selected_under_overlay_ = selected_;
    return;
  }
  // The overlay showed another tab, as the command bar does going to a page: its page takes the
  // keyboard, as a press that shows a tab gives it. On X11 the window isn't active while the
  // overlay has the input focus, so FocusPage passed the page over as it showed, and the window
  // holds the keyboard for it until it's active again. Not when the tab it showed closed, as a
  // Space the dialog deleted did: what shows then comes of that, and the UI keeps the keyboard, as
  // the Mac's does.
  const bc_id under = std::exchange(selected_under_overlay_, 0);
  if (under != selected_ && shown_ && (!under || Tab::Get(under))) {
    page_had_keyboard_ = true;
    GiveKeyboardToPage();
  }
}

void Window::FocusPageSoon() {
  // Posted: AppKit can be part way through moving the keyboard, as when a committing page hides the
  // view that had it and Chromium makes the window key. A page view focused inside that move
  // believes it has the keyboard after AppKit has given it elsewhere (FocusSettledOnActivation).
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Window::FocusPage, weak_factory_.GetWeakPtr()));
}

void Window::FocusPage() {
  if (!shown_ || !widget_->IsActive()) {
    return;
  }
  if (content::WebContents* devtools = devtools_to_focus_.get()) {
    devtools_to_focus_ = nullptr;
    page_had_keyboard_ = false;
    devtools->SetInitialFocus();
    return;
  }
  if (std::exchange(page_had_keyboard_, false) || platform::KeyboardForPage(widget_.get())) {
    Tab::FocusPage(shown_.get());
  }
}

content::WebContents* Window::OpenBehind(NavigateParams& params) {
  content::WebContents::CreateParams create(params.browser->GetProfile());
  create.initially_hidden = true;
  // As for a tab put back at launch: no renderer until it loads.
  create.desired_renderer_state = content::WebContents::CreateParams::kNoRendererProcess;
  std::unique_ptr<content::WebContents> made = content::WebContents::Create(create);
  content::WebContents* contents = made.get();
  {
    base::AutoReset<bool> opening(&opening_tab_, true);
    params.browser->GetTabStripModel()->AddWebContents(std::move(made), -1, params.transition,
                                                       AddTabTypes::ADD_NONE);
  }
  // The load Chrome would have started in it now.
  Tab::Get(Tab::IdOf(contents))->LoadWhenShown(
      LoadURLParamsFromNavigateParams(contents, &params));
  return contents;
}

// static
Space* Window::SpaceOf(content::WebContents* contents) {
  if (Space* space = Space::Holding(Tab::IdOf(contents))) {
    return space;
  }
  tabs::TabInterface* tab = tabs::TabInterface::MaybeGetFromContents(contents);
  BrowserWindowInterface* browser = tab ? tab->GetBrowserWindowInterface() : nullptr;
  Window* window = browser ? ForBrowser(browser) : nullptr;
  if (!window) {
    return nullptr;
  }
  TabStripModel* tab_strip_model = browser->GetTabStripModel();
  const int index = tab_strip_model->GetIndexOfWebContents(contents);
  tabs::TabInterface* opener =
      index == TabStripModel::kNoTab ? nullptr : tab_strip_model->GetOpenerOfTabAt(index);
  return window->SpaceForOpened(opener ? Tab::IdOf(opener->GetContents()) : 0);
}

Space* Window::SpaceForOpened(bc_id opener) const {
  Space* space = opener ? Space::Holding(opener) : nullptr;
  return space ? space : Space::FromId(space_);
}

void Window::ReportTabOpened(bc_id tab,
                             bc_id opener,
                             bool foreground,
                             bc_id split_with,
                             std::optional<bc_rect> popup,
                             bool popup_placed) {
  // A popup can close itself at once.
  if (!Tab::FromId(tab)) {
    return;
  }
  if (Space* space = SpaceForOpened(opener)) {
    space->Add(tab, opener, foreground);
    // Beside the page it came from, unless that page's split view is full.
    const bool split = split_with && space->Split(tab, split_with, Tab::PageUrl(Tab::FromId(tab)),
                                                  /*left=*/false);
    // A page in a Space the window doesn't show doesn't bring its tabs to the front.
    if (foreground && space->id() == space_) {
      Select(tab);
    }
    if (split) {
      PanesChanged();
    }
  }
  if (popup && Host().tab_popup) {
    Host().tab_popup(Host().context, id_, tab, opener, *popup, popup_placed);
  }
  if (Host().tab_opened) {
    Host().tab_opened(Host().context, id_, tab, opener, foreground);
  }
}

}  // namespace browser_core
