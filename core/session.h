#ifndef BROWSER_CORE_SESSION_H_
#define BROWSER_CORE_SESSION_H_

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

#include "base/functional/callback_forward.h"
#include "base/time/time.h"
#include "browser_core/public/browser_core.h"
#include "browser_core/space.h"
#include "components/sessions/core/serialized_navigation_entry.h"
#include "ui/gfx/geometry/rect.h"
#include "url/gurl.h"

class Profile;

namespace content {
class WebContents;
}

namespace browser_core {

class Window;

// A tab's history, as the session file keeps it.
struct SavedTab {
  // Up to Chrome's count of navigations each side of the current one, as its sessions keep.
  std::vector<sessions::SerializedNavigationEntry> history;
  int current = 0;
  // A pinned tab's pinned URL, or a favorite's URL.
  GURL pinned_url;
  // The name the user gave it, if any.
  std::string name;
  // The emoji the user gave it to show in place of its favicon, if any.
  std::string icon;
  // When a window last stopped showing a Today tab, if it was saved with one.
  base::Time last_seen;
  // Whether it's in a split view with the tab saved above it, and whether that split is stacked.
  bool split = false;
  bool stacked = false;
  // Its share of its split view, or 0 while the split's shares are even.
  double share = 0;
  // Whether its page took its split view's card, the other pages keeping 220 pt, which the shares
  // aren't kept for.
  bool expanded = false;
  // A pinned tab's folder, among its Space's saved folders, else -1.
  int folder = -1;
  // A pinned tab's node, or none if it was saved before pinned tabs had them.
  Space::Node node;
  // Whether it was muted, for a tab closed and reopened. The session file doesn't keep it.
  bool muted = false;
  // A favorite's profile's directory. A Space's tabs are in the Space's.
  std::string profile;
  // What names a favorite's site, shared by its tabs in each profile under Every Space, or 0.
  int site = 0;
};

// |contents|'s history, with the page it's loading, or waiting to load, if it has no page yet.
SavedTab SaveTab(content::WebContents& contents);

// A folder of pinned tabs, as the session file keeps it.
struct SavedFolder {
  std::string name;
  std::string icon;
  bool open = true;
  // The folder it's in, among its Space's saved folders, which is saved before it, else -1.
  int folder = -1;
  // The pinned tab saved under it, counted among them.
  size_t at = 0;
  // Its node, or none if it was saved before folders had them.
  Space::Node node;
};

// A Space, as the session file keeps it.
struct SavedSpace {
  std::string name;
  // Its node, or none if it was saved before Spaces had them.
  Space::Node node;
  std::optional<uint32_t> color;
  // The emoji the user gave it, if any.
  std::string icon;
  // The theme the UI gave it, if any.
  std::string theme;
  // Space::touched(), which a session saved before Spaces had it can't say, so it was.
  bool touched = true;
  // Its profile's directory, or empty for the profile the core started with.
  std::string profile;
  // The agent it belongs to, or none for the user's own, as every Space saved before agents had
  // Spaces is.
  std::optional<Space::Agent> agent;
  // Each top to bottom.
  std::vector<SavedTab> pinned;
  std::vector<SavedFolder> folders;
  // Whether its pinned tabs and folders showed, or were collapsed.
  bool pinned_open = true;
  std::vector<SavedTab> today;
  // The tab the window showed there, counting the pinned tabs then the Today tabs, or -1.
  int shown = -1;
  // The favorite the window showed there instead, among the session's favorites, or -1.
  int shown_favorite = -1;
  // The favorites that belong to the Space under Each Space, left to right, and the one the window
  // showed there among them, or -1.
  std::vector<SavedTab> favorites;
  int shown_own_favorite = -1;
};

// |space|, where a window shows |shown|, without its shown favorite unless that's one of its own.
// Puts the Today tabs it saved in |today| if it isn't null.
SavedSpace SaveSpace(const Space& space, bc_id shown, std::vector<bc_id>* today = nullptr);

// Makes |saved| a Space, its tabs unloaded in |window| and |profile|, with the id |id|, a deleted
// Space's, if it isn't 0. The window shows its shown tab there. Returns it.
Space* RestoreSpace(Window& window,
                    Profile* profile,
                    const SavedSpace& saved,
                    bc_id id = 0,
                    size_t at = SIZE_MAX);

// The Spaces as the user left them: their names, their tabs with each tab's history, the tab each
// showed and the Space the window showed, and the favorites. The core saves them a while after
// they change, in the user data directory since Spaces aren't one profile's, and the next launch
// puts them back.

// Reads what the last launch saved, then runs |loaded|.
void LoadSession(base::OnceClosure loaded);

// Saves the Spaces soon, as something in them changed.
void SessionChanged();

// Saves the Spaces now and stops saving: the tabs closing go with their window or with the quit,
// and come back at the next launch.
void KeepSession();

// Writes a save still waiting, as Chrome shuts down.
void FlushSession();

// The docked sidebar's width the last launch saved for its front window, or 0.
double SavedSidebarWidth();

// The frames of the regular windows open when the browser last quit, front to back, as they were
// out of full screen. One saved before frames were kept is empty. Read with the Spaces.
std::vector<gfx::Rect> SavedWindowFrames();

// Has |window| show what the saved window |index| showed, the first being RestoreSession's: its
// Space, its sidebar's width, and full screen if it was the front one. Returns whether there's such a window
// and its Space came back.
bool RestoreSavedWindow(Window& window, size_t index);

// What the user chose in the app's Settings. The core keeps it with the Spaces, and reads it back
// with them, before the app opens its window.
struct Settings {
  // How long a Today tab no window shows waits before it goes in the archive.
  base::TimeDelta archive_after = base::Hours(12);
  // How long a Little ilo window that isn't active waits before it closes into the archive (Arc's
  // default).
  base::TimeDelta little_archive_after = base::Hours(6);
  // Whether the app taps the trackpad at its detents.
  bool haptics = true;
  // Whether the app asks before the user's Quit quits.
  bool warn_before_quitting = true;
  // Whether the user has dismissed the note on the first tabs auto-archive took.
  bool archive_noted = false;
  // Whether a launch brings back the last session's windows with their Today tabs, else one window
  // showing no tab (RestoreSession).
  bool restore_windows = true;
  // Whether the app warns before moving a tab to another profile's Space, which it does once.
  bool warn_moving_profiles = true;
  // Who shares the favorites: a BC_FAVORITES_ value.
  int favorites_scope = BC_FAVORITES_PROFILE;
  // Whether a tab no window has shown for a while lets go of its page (UnloadIdleTabsSoon).
  bool unload_idle_tabs = true;
  // Whether the browser blocks ads and trackers, in every profile.
  bool ad_blocking = true;
  // Whether the browser sends its crash reports (crash_reports.h).
  bool send_crash_reports = false;
  // Whether a link to another site in a pinned tab or a favorite opens in a link preview.
  bool link_previews = true;
  // Whether a page may take its video or call into Picture in Picture as its tab is hidden, in
  // every profile (ApplyAutoPictureInPicture).
  bool auto_picture_in_picture = true;
  // Whether ⌥ pressed alone brings the hidden sidebar out over the page.
  bool option_reveals_sidebar = true;
  // What the Library button's hover lists, as bc_hover_kinds' bits.
  int hover_kinds = 15;
  // When the newest crash report the user was offered was written, in seconds since 1970, so no
  // report is offered twice, nor one older.
  double crash_report_offered = 0;
  // Whether the user said yes to Google's Widevine module, which the component updater then keeps
  // up to date (widevine.h), until they remove it.
  bool widevine = false;
};

// The user's settings. Call SessionChanged after changing them.
Settings& UserSettings();

// Puts the favorites and Spaces the last launch saved in |window|, which shows what the front
// window showed (RestoreSavedWindow), unless there are Spaces already. Returns whether it did.
bool RestoreSession(Window& window);

}  // namespace browser_core

#endif  // BROWSER_CORE_SESSION_H_
