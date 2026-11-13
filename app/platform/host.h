// The platform side of the app, called from Rust: what the core's API (core/public/browser_core.h)
// leaves to the app. The core makes the browser window and puts the page in it; GPUI draws in a
// view of the app's, under the page.
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Starts the test driver (drive.mm), which takes commands on stdin.
void drive_start(void);

// Where a window of |width| by |height| points goes to be centered on the main display, or on the
// Mac's own with BROWSER_DISPLAY=builtin, in bc_window_open's terms: from the main display's
// top-left corner.
void host_centered(double width, double height, double* x, double* y);

// Puts a window on screen at |x|, |y|, |width| by |height| in host_centered's terms, where the
// first browser window will open, with the window's material and, unless |picture_width| is 0,
// the sidebar's picture from |profile_dir| that a |picture_width| by |picture_height| window took
// (host_save_launch_picture). Its part above |foot_top| points from the window's top shows at the
// window's top and the rest at its bottom, the room just above |foot_top| cropped or repeated to
// fit (launch_picture.rs says when that draws the sidebar as the UI will). For the moment between
// NSApp being made and the browser starting. With |maximized|, Windows' alone, it's maximized on
// that frame's screen, as the first browser window will be (host_maximize_window), with no picture.
void host_show_launch_window(const char* profile_dir, double x, double y, double width,
                             double height, bool maximized, double picture_width,
                             double picture_height, double foot_top);

// The visible frames of the screens a window may open on, in host_centered's terms, as x, y,
// width and height, four to a screen: the main display's first, or with BROWSER_DISPLAY=builtin
// the Mac's own alone. Copies up to |count| screens into |frames| and returns how many there are.
size_t host_screens(double* frames, size_t count);

// Where the pointer is, in host_centered's terms, for Little ilo to open on its screen. False where
// the host doesn't say, as on Linux, whose window manager places windows.
bool host_pointer(double* x, double* y);

// The frame of the browser window of |ui_view|, in host_centered's terms. False, leaving them be,
// while it's full screen.
bool host_window_frame(void* ui_view, double* x, double* y, double* width, double* height);

// Takes the launch window's sidebar picture off the browser window, once the UI has drawn under
// it, and the launch window off screen if the browser window never came.
void host_close_launch_window(void);

// Calls |finished| once AppKit has finished launching the app, and so has handed over the links
// the launch carried (application:openURLs:). At once for a launch that carried none. Call before
// the run loop starts.
void host_watch_finished_launching(void (*finished)(void));

// Keeps the first |width| points of |ui_view|'s window, its sidebar, in |profile_dir| for the next
// launch's window, unless the window is hidden or full screen, and says whether it took it. The
// file is written in the background.
bool host_save_launch_picture(void* ui_view, const char* profile_dir, double width);
// Windows: the same, from the sidebar's pixels in the frame the UI last drew, |columns| by |rows|
// top-down BGRA. A picture of the window would have Windows repaint its frame, where its theme
// paints the caption under the UI, on screen too (#1262).
bool host_save_launch_pixels(void* ui_view, const char* profile_dir, const uint8_t* pixels,
                             size_t columns, size_t rows);

// Mac and Windows: draws the icon of |app|, if it's installed, |size| pixels square into |rgba|,
// size * size * 4 bytes of premultiplied RGBA, for the import's rows (#1673). On the Mac |app| is
// a bundle id; on Windows the shell's icon for an exe, by its UTF-8 path, or by a bare name such as
// chrome.exe looked up in the registry's App Paths, which is all it reads of the other browser.
bool host_app_icon(const char* app, size_t size, uint8_t* rgba);

// Makes the view the UI draws in, for bc_window_open, which sizes it to fill the window. The
// functions below that take a |ui_view| act on the browser window it's in.
void* host_ui_view(void);

// Linux only: calls |ready| from Chromium's loop whenever |fd| is readable, for GPUI's loop, which
// runs inside it (gpui_linux::embed).
void host_watch_fd(int fd, void (*ready)(void));

// Linux and Windows: |window| is the core's id for the browser window of |ui_view|. The window is
// Chromium's, so the calls that act on it, such as the driver's, go through the core.
void host_window_opened(void* ui_view, uint64_t window);

// Linux only: the browser window of |ui_view| has closed, for the watches below.
void host_window_closed(void* ui_view);

// Linux only: GPUI's window in |ui_view| became active, or stopped being, for the watches below.
// With |whole_window|, as under Wayland, that's the browser window's having the keyboard; on X11
// GPUI hears only of its own window's, which the page's taking the keyboard ends.
void host_window_active(void* ui_view, bool active, bool whole_window);

// A capture's file name for now, as macOS names a screenshot:
// "Capture 2026-10-29 at 18.04.12.png".
void host_capture_name(char* name, size_t capacity);

// The browser window of |ui_view| is Little ilo's, as Arc's Little Arc: each time it comes on
// screen it comes to the desktop shown rather than taking the user to the one it was on, and opened
// while a full-screen browser window has that desktop, it opens over it there. With |grows|,
// host_show_window first shows it small, at a rectangle under the pointer, or at its middle when a
// key opened it, until host_grow_little_window. Before host_show_window. Off the Mac, where the
// window manager places windows and animates them its own way, it doesn't grow; Windows records
// it, for closing it over another app's window (#1692), and Linux does nothing.
void host_set_window_little(void* ui_view, bool grows);

// Grows the Little ilo window of |ui_view| in from where host_show_window showed it small, once
// its UI has drawn on screen, so the motion's start is seen. Nothing once it has grown or wasn't
// held, as off the Mac.
void host_grow_little_window(void* ui_view);

// Shows the window holding |ui_view| in front, with its title bar over the UI, and makes the
// app the active one.
void host_show_window(void* ui_view);

// Shows the window holding |ui_view| as host_show_window does, but right behind the browser window
// of |above_ui_view|, neither key nor activating the app, as the windows a relaunch brings back
// after the first.
void host_show_window_behind(void* ui_view, void* above_ui_view);

// Where the next browser window goes after the one of |ui_view|, in host_centered's terms: as
// big, down and to the right as AppKit cascades windows, kept on its screen.
void host_cascaded(void* ui_view, double* x, double* y, double* width, double* height);

// Names the browser window of |ui_view| in the Window menu's list of windows and in Mission
// Control. The window doesn't draw it.
void host_set_window_title(void* ui_view, const char* title);

// Draws the material under the UI of |ui_view|'s browser window light, Dia's material 13, or
// dark, its material 29 (docs/ux-spec.md › Window material).
void host_set_window_light(void* ui_view, bool light);

// Gives the whole app, its menus, alerts and pages, an appearance of its own, 1 light or 2 dark, in
// place of the system's, or with 0 the system's again (View › Appearance).
void host_set_app_appearance(int appearance);

// Makes the browser window of |ui_view| key and puts it in front, out of the Dock if it's
// minimized, as a Switch to Tab row for a page it shows does. The user's: agents get none, as it
// takes the user's focus (decision 8).
void host_activate(void* ui_view);

// Hides the browser window of |ui_view| without closing it, or with |shown| puts it back in front,
// out of the Dock if it's minimized, as the Dock's Hide All and Show All Little ilo Windows do.
// Whether it's on screen now: host_window_shown. The user's: agents get none.
void host_set_window_shown(void* ui_view, bool shown);
bool host_window_shown(void* ui_view);

// Puts ilo's items in the Dock icon's menu, the engine's, in place of its New Window and New
// Incognito Window: those two, as File's run them (|chosen| 3 and 4), then Little ilo's, as Arc's:
// New Little ilo Window, then, while one is open, Hide All Little ilo Windows, or Show All Little
// ilo Windows once they're all hidden. |little| is asked as the menu opens: 0 with no Little ilo
// window open, 1 with one on screen, 2 with all hidden, and -1 with Little ilo off in Settings,
// which leaves out all of Little ilo's items. |chosen| gets the item picked: 0 New, 1
// Show All, 2 Hide All, 3 New Window, 4 New Incognito Window. Once the app has finished launching, as the engine's delegate is set then.
// Off the Mac, which has no Dock menu: on Windows the taskbar's jump list gets New window and New
// incognito window, which launch ilo again with --new-window and --incognito (bc_host's
// app_new_window), once a window is shown; on Linux nothing, as the desktop file has them.
void host_add_dock_items(int (*little)(void), void (*chosen)(int item));

// Puts a button in the menu bar whose click calls |clicked|, which opens a Little ilo window, as
// Arc's optional one does, or with NULL takes it out again. Off until the user turns it on in
// Settings › General. It draws once and then waits for a click, so an idle browser stays asleep.
// Off the Mac, nothing, and Settings offers no switch there.
void host_set_little_status_item(void (*clicked)(void));

// Puts a task called |title| in the taskbar's jump list, which runs the browser with |arguments|,
// as Windows' jump lists hold an app's tasks: New Little ilo Window, which the core hands the app.
// Windows keeps the list, so it's written once a launch, as the first window opens with the
// AppUserModelID the list goes under, and nothing waits for it. Off Windows,
// whose Dock menu and launcher have their own, nothing.
void host_set_little_task(const char* title, const char* arguments);

// Calls |changed| with the view of each browser window that becomes the key window.
void host_watch_key_window(void (*changed)(void* ui_view));

// Mac only: whether the window of |ui_view|, any of the app's, is the key window now. GPUI hears
// it in a task of its own, after a menu's window has handed the keyboard back (#2019).
bool host_window_is_key(void* ui_view);

// Calls |changed| with the view of a browser window as it becomes the window the user is in, or
// stops being, and with each one's state as the watch starts and as it shows. That's the active
// app's main window on the Mac, so its own overlays, menus and sheets over it don't count; the
// foreground window on Windows, or one it owns, as an overlay; on X11 the window manager's active
// window (_NET_ACTIVE_WINDOW), or one transient for it, read as it changes; under Wayland the
// browser window's having the keyboard, which the compositor gives the activated toplevel, the
// page's or the UI's alike. Linux's is active_window.rs's.
void host_watch_window_main(void (*changed)(void* ui_view, bool main));

// Calls |changed| with the view of each browser window that has gone full screen, or left it, once
// macOS's transition is over.
void host_watch_full_screen(void (*changed)(void* ui_view, bool entered));

// Asks |swiped| about each swipe between pages in a browser window, at its point in |ui_view|
// from the top left, in points; one it takes, by returning true, goes no further.
void host_watch_swipes(bool (*swiped)(void* ui_view, double x, double y, bool back));

// Calls |tapped| with the view of the browser window with the keyboard each time ⌥ is pressed and
// let go with no other key, modifier, press or scroll between, the page's keyboard or the UI's.
void host_watch_option_taps(void (*tapped)(void* ui_view));

// Calls |left| with the view of a browser window whose UI loses the keyboard to its own page,
// which GPUI doesn't see, so a field in the sidebar ends as it does for a click elsewhere: on the
// Mac at a press in the page, on Windows as GPUI's window loses the focus, to the page or anything
// else. Linux's UI hears it itself, through GPUI's patch and the overlays it shows.
void host_watch_ui_keyboard(void (*left)(void* ui_view));

// Calls |pressed| with the UI's view of a browser window whose page the user presses a mouse
// button in, as the press goes on to the page: a click in an agent's page takes it over (#1080).
// Never for a press in the UI or the window's frame, nor for the wheel. On Linux the app hears
// them itself, from GPUI under Wayland and from the X server while an agent works (page_presses.rs).
void host_watch_page_presses(void (*pressed)(void* ui_view));

// Whether a menu is open, the menu bar's or a context menu, and if so calls |apply| once none is,
// instead of any function an earlier call left waiting. AppKit can crash when the main menu is
// replaced while it tracks one, and the core's callbacks still come in while it does.
bool host_after_menus(void (*apply)(void));

// Calls |resigning| as the app is about to stop being the active one, its windows still shown.
void host_watch_resigning_active(void (*resigning)(void));

// Calls |became_active| each time the app becomes the active one, such as back from System
// Settings.
void host_watch_active(void (*became_active)(void));

// Whether the system asks apps to reduce motion: the Mac's Reduce Motion, Windows' "Show
// animations in Windows" turned off. Linux's desktop says so through its settings portal, which
// platform/linux/reduce_motion.rs reads instead.
bool host_reduce_motion(void);

// Calls |changed| on the UI thread each time that setting changes.
void host_watch_reduce_motion(void (*changed)(void));

// Whether the system opens http and https links with this app: whether it's the default browser.
bool host_is_default_browser(void);

// Asks the system to open http and https links with this app, which asks the user to confirm
// first, then calls |answered| once they have, either way. The way back is the system's own
// setting, in System Settings › Desktop & Dock. The user's alone: agents get none, as it's the
// system's setting, not the browser's.
void host_make_default_browser(void (*answered)(void));

// Milliseconds since the system started the app's process, for its launch times.
double host_since_launch(void);

// The browser window of |ui_view| has closed: what showed over it hides, and its windows go.
void host_forget_window(void* ui_view);

// The overlays: the command bar, the Space's icon and theme pickers, a closed folder's preview and
// the Boost editor. One shows at a time.
#define HOST_COMMAND_BAR 0
#define HOST_ICON_PICKER 1
#define HOST_THEME_PICKER 2
#define HOST_FOLDER_PREVIEW 3
#define HOST_BOOST_EDITOR 4
// The Site Controls take the same elevation call, though they aren't one of the overlays.
#define HOST_SITE_CONTROLS 5

// Gives the |overlay| (or the Site Controls) the shadow its elevation level draws, in Core
// Animation's terms: a black line round its shape and a black drop shadow, each with an opacity
// and a blur |radius|, which is half CSS's blur, and the drop shadow falling |shadow_drop| points.
// The shadow reaches three radii and the drop past the window. Call it before the overlay is added,
// and again before a show that takes another level, as the command bar's over the URL field does.
void host_set_overlay_elevation(int overlay,
                                float outline_opacity,
                                double outline_radius,
                                float shadow_opacity,
                                double shadow_radius,
                                double shadow_drop);

// Makes the window of |overlay_view| the |overlay| (one of the HOST_ overlays above), which
// floats over a browser window when shown, for UI that has to draw over the page, translucent over
// a blur of what's behind it, with a shadow, in a rounded shape of |radius|. It starts hidden.
// |dismissed| runs, after the event, when the user leaves the overlay without choosing anything in
// it.
void host_add_overlay(int overlay, void* overlay_view, double radius, void (*dismissed)(void));

// Gives the overlay's blur and shadow the shape the UI draws when that isn't a rounded rectangle:
// the polygon through the |count| corners, each three numbers in |corners|: x and y in points from
// the window's top left, and the radius it's rounded by there.
void host_set_overlay_shape(int overlay, const double* corners, size_t count);

// Rounds the overlay's blur and shadow by |radius| from its next show on, as the command bar's
// panel over the URL field is rounded less than its centred one.
void host_set_overlay_radius(int overlay, double radius);

// Makes the overlay grow in as it shows, as a system popover does, from |x| and |y| points from
// its window's top left, such as the tip of its arrow. It goes in one frame.
void host_set_overlay_growth(int overlay, double x, double y);

// Makes the overlay, once it grows in, shrink back to where it grew from as it hides, fading.
void host_set_overlay_shrink(int overlay);

// Makes the overlay that grows in (host_set_overlay_growth) grow from |from| of its size, rather
// than near nothing, over |seconds|, fading in alongside, and fade out over |out| seconds as it
// hides, shrinking back to |from| only if host_set_overlay_shrink asked: a folder's preview. Each
// show takes the motion last set.
void host_set_overlay_motion(int overlay, double from, double seconds, double out);
// Under Reduce Motion the overlays that grow in and shrink back only fade, at their size. The UI
// says so as its own Reduce Motion changes (main.rs, set_reduce_motion).
void host_set_overlays_still(bool still);

// Shows the overlay over |ui_view|'s window and gives it the keyboard, |width| points wide: with
// its top left at |left|, |top| in the window's content and ending at least 8 points inside it, or
// if |centered|, centred across it with its top |top| points above its middle. The other overlay
// closes, as the user leaving it. Shown there already, it moves to the new place. Keys typed
// before it has the keyboard go to it once it does.
void host_show_overlay(int overlay,
                       void* ui_view,
                       bool centered,
                       double left,
                       double top,
                       double width);

// Makes the overlay |height| points tall, keeping its top edge, and draws it at that size.
void host_set_overlay_height(int overlay, double height);

// Whether this system's emoji font draws |emoji|, UTF-8, as the one emoji it is. A newer emoji than
// the system knows draws as a missing glyph, and a newer sequence as the emoji it is made of. For
// the icon picker's list. emoji.mm.
bool host_can_draw_emoji(const char* emoji);

// Hides the overlay. The browser window gets the keyboard back, with the focus where it was.
void host_hide_overlay(int overlay);

// Hides the overlay as the user leaving it, as escape does.
void host_dismiss_overlay(int overlay);

// Hides the overlay by folding it into |x|, |y|, |width| and |height| points from the top left of
// the content of the browser window it shows over: its drawing fades in |fade| seconds, as its
// plate, rounded by |radius|, goes there in |duration| on the motion's ease, its fill turning
// from |from| to |to| (0xRRGGBBAA) and its shadow going. With |fade_only| it only fades where it
// is, in |fade|. Returns false where the host can't, having hidden it. host_hide_overlay's
// keyboard rule holds.
bool host_fold_overlay(int overlay,
                       double x,
                       double y,
                       double width,
                       double height,
                       double radius,
                       uint32_t from,
                       uint32_t to,
                       double duration,
                       double fade,
                       bool fade_only);

// Gives the overlay the keyboard back if it shows, the user hasn't left it, and the browser window
// it shows over or no window has it: for the UI, once it finds the overlay's window isn't key.
// The Mac's alone: Linux's and Windows' hosts close an overlay whose keyboard goes elsewhere.
void host_take_overlay_keyboard(int overlay);

// Holds the keys typed from now until the command bar or the find bar takes the keyboard, and then
// gives them to it: for a shortcut that opens one but reaches the page first, which may keep it,
// and on Windows for the find bar, which shows a frame before it takes the keyboard
// (find_bar::show).
// Showing either bar with the keyboard holds them by itself. The core says when the page has
// answered the shortcut and no bar opens (host_release_held_keys).
void host_hold_keys(void);

// No bar opens for the shortcut host_hold_keys held the keys after, so they go to the page now.
void host_release_held_keys(void);

// Makes the window of |find_bar_view| the find bar of |ui_view|'s window, floating over it when
// shown, |right| and |top| points from the top right corner of the window's content. It starts
// hidden.
void host_add_find_bar(void* ui_view, void* find_bar_view, double right, double top);

// Shows |ui_view|'s find bar, and gives it the keyboard if |keyboard|, with the keys typed until
// then. Unlike the command bar, it stays when the user clicks the page, which takes the keyboard.
void host_show_find_bar(void* ui_view, bool keyboard);

// Hides |ui_view|'s find bar. If it had the keyboard, its browser window gets it back.
void host_hide_find_bar(void* ui_view);

// Makes the window of |offer_view| the offer to save a password of |ui_view|'s window, floating
// over it when shown, |right| and |top| points from the top right corner of the window's content,
// or under the find bar while it shows. It starts hidden.
void host_add_offer(void* ui_view, void* offer_view, double right, double top);

// Shows |ui_view|'s offer, without the keyboard: as the find bar, it takes it when clicked, and the
// page takes it back when clicked.
void host_show_offer(void* ui_view);

// Hides |ui_view|'s offer. If it had the keyboard, its browser window gets it back.
void host_hide_offer(void* ui_view);

// Moves |ui_view|'s find bar, its offer, and the HUD and the toast over it |right| points further
// in from the window's right edge and |top| points further down, to the focused page's top right
// corner when a split view's focused page isn't the top right one. The focused page's left edge
// is |left| points from the window's, where the toast's window starts.
void host_set_pane_corner(void* ui_view, double left, double right, double top);

// Makes the window of |hud_view| float over a browser window when shown, |right| and |top| points
// from the top right corner of the window's content, for the zoom HUD, which draws its pill
// |margin| inside the window. It starts hidden, and the mouse goes through it.
void host_add_hud(void* hud_view, double right, double top, double margin);

// Shows the HUD over |ui_view|'s window as the UI draws it now, again for a change, until
// host_hide_hud.
void host_show_hud(void* ui_view);

// Hides the HUD, once it has drawn its way out.
void host_hide_hud(void);

// Makes the window of |view| float over a browser window's content when shown, covering it, for
// the split view's drop targets over the page. It starts hidden, and the mouse goes through it.
void host_add_drop_targets(void* view);

// Shows the drop targets' window as the UI draws it now, at the content of |ui_view|'s window.
void host_show_drop_targets(void* ui_view);

// Hides the drop targets' window.
void host_hide_drop_targets(void);

// Makes the window of |dialog_view| the dialog's, which covers a browser window when shown, with
// the keyboard until it's hidden. It starts hidden.
void host_add_dialog(void* dialog_view);

// Shows the dialog's window over |ui_view|'s window, as the UI draws it now, with the keyboard:
// over all of it, or with a |width|, over that rect of its content, from its top left, such as a
// tab's card. Shown already, it moves to the rect.
void host_show_dialog(void* ui_view, double x, double y, double width, double height);

// Hides the dialog's window. The browser window gets the keyboard back, with the focus where it
// was.
void host_hide_dialog(void);

// Makes the window of |view| the Site Controls', which float over a browser window as its child
// when shown, translucent over a blur in their shape, with a shadow. It starts hidden. |dismissed|
// runs, after the event, when the user leaves them: a click outside them, another window taking
// the keyboard, or switching apps.
void host_add_site_controls(void* view, void (*dismissed)(void));

// The Site Controls' shape from their window's top left, |count| corners clockwise, each as x, y
// and the radius it's rounded by, which their blur and shadow take at their next show or height.
void host_set_site_controls_shape(const double* corners, size_t count);

// Shows the Site Controls over |ui_view|'s window, as the UI draws them now, with the keyboard:
// |width| by |height| points, their top left at |left|, |top| in the window's content.
void host_show_site_controls(void* ui_view, double left, double top, double width, double height);

// Makes the Site Controls |height| points tall, keeping their top edge, and draws them at that
// size.
void host_set_site_controls_height(double height);

// The UI drew a frame of the Site Controls' motion, at |scale| about |x|, |y| from their window's
// top left and at |opacity|, which their blur and shadow follow.
void host_site_controls_drew(double scale, double opacity, double x, double y);

// The Site Controls take no more clicks or keys as they leave: the mouse goes through them, and the
// browser window gets the keyboard back, with the focus where it was, if they had it.
void host_release_site_controls(void);

// Hides the Site Controls once they've left.
void host_hide_site_controls(void);

// Makes the window of |view| the link preview's, which covers a browser window when shown, as its
// child, with the keyboard, and returns the view the preview's page goes in, over |view|. It
// starts hidden. |dismissed| runs, after the event, when the user leaves it for another window.
void* host_add_link_preview(void* view, void (*dismissed)(void));

// Shows the link preview's window over |ui_view|'s window, as the UI draws it now, with the
// keyboard, its page coming in with the UI's first frame.
void host_show_link_preview(void* ui_view);

// The UI drew a frame of the link preview's motion, with its page at |left|, |top|, |width| by
// |height| from the window's top left, with corners of |radius|, and at |opacity|, which the page's
// view follows. Where the page shows under the preview's window, it's the hole the window has.
void host_link_preview_drew(double left,
                            double top,
                            double width,
                            double height,
                            double radius,
                            double opacity);

// The link preview takes no more clicks or keys as it leaves: the mouse goes through it, and the
// browser window gets the keyboard back, with the focus where it was, if it had it.
void host_release_link_preview(void);

// Hides the link preview once it has left.
void host_hide_link_preview(void);

// Makes the window of |view| Capture's, which covers a browser window's page when shown, as its
// child, with the keyboard and a camera for the pointer. It starts hidden. |dismissed| runs, after
// the event, when the user leaves it: a click outside it, another window taking the keyboard,
// switching apps, or the browser window changing size, which the picture of the page no longer
// fits.
void host_add_capture(void* view, void (*dismissed)(void));

// Shows Capture's window over |ui_view|'s window, as the UI draws it now, with the keyboard:
// |width| by |height| points, its top left at |left|, |top| in the window's content.
void host_show_capture(void* ui_view, double left, double top, double width, double height);

// The pointer over Capture's window is the camera, or with |camera| false the arrow, as over its
// menu.
void host_capture_camera(bool camera);

// Hides Capture's window. The browser window gets the keyboard back, with the focus where it was.
void host_hide_capture(void);

// Puts the PNG |png|, |size| bytes long, on the clipboard, or on host_copy's pasteboard of its own
// in a driven run.
void host_copy_png(const uint8_t* png, size_t size);

// Calls |png| with the desktop picture of the display the browser is on as a PNG, at most the
// display's size in pixels, before it returns, for Portrait Mode (#673), and says whether it did:
// false where the system gives none, as on Linux, whose sandbox can't read it (#2682).
bool host_desktop_picture(void* context,
                          void (*png)(void* context, const uint8_t* data, size_t size));

// Asks where to save the PNG |png|, |size| bytes long, in a save panel on |ui_view|'s window, named
// for when it was taken, and saves it there. It returns at once, before the panel shows, since the
// UI calls it from an event its panel can't wait in.
void host_save_png(void* ui_view, const uint8_t* png, size_t size);

// Shares the PNG |png|, |size| bytes long, as a file named for when it was taken: in the system's
// share menu at |x|, |y|, |width| by |height| points from the top left of |view|, a window's view,
// or with |messages| in a new message in Messages.
void host_share_png(const uint8_t* png,
                    size_t size,
                    void* view,
                    double x,
                    double y,
                    double width,
                    double height,
                    bool messages);

// Makes the window of |toast_view| the toast's, which floats over a browser window's top right
// corner when shown, as its child, and lets the mouse through. It starts hidden. Shown, it's as
// wide as the focused page, from that page's left edge to its corner (host_set_pane_corner), and
// keeps its own height.
void host_add_toast(void* toast_view);

// The toast takes clicks, while it has buttons, or lets the mouse through to the page. Even then
// only its plate does (host_set_toast_plate), and the margin round it lets them through.
void host_toast_takes_clicks(bool takes);

// Where the toast's plate is in its window, from the window's top left corner, in points.
void host_set_toast_plate(double x, double y, double width, double height);

// Shows the toast's window over |ui_view|'s window, at its focused page's corner
// (host_set_pane_corner), as the UI draws it now.
void host_show_toast(void* ui_view);

// Hides the toast's window.
void host_hide_toast(void);

// Makes the window of |foot_bar_view| the foot bar: the bar a mode of the page, such as Arrange's
// and pointing's, shows over the page's bottom centre, as a child of the browser window. A click on
// it reaches its buttons, but it never takes the keyboard, which stays with the browser window and
// so with the page. It starts hidden.
void host_add_foot_bar(void* foot_bar_view);

// Shows the foot bar's window over |ui_view|'s window, or moves it there: |width| by |height|
// points, centred between |left| and |right| points in from the edges of the window's content,
// with its bottom |bottom| points over the content's. It keeps those as the window resizes.
void host_show_foot_bar(void* ui_view,
                        double left,
                        double right,
                        double bottom,
                        double width,
                        double height);

// Hides the foot bar's window.
void host_hide_foot_bar(void);

// Makes the window of |tooltip_view| the tooltip's, which floats over a browser window as its
// child when shown, and lets the mouse through and never takes the keyboard. It starts hidden.
// |away| is called when that window stops being key or the app stops being active, which hide it,
// and when the overlay it shows over hides, which hides it at once.
// The window reaches |margin| points past the plate on every side, for its shadow, which the UI
// draws; |left| and |top| below are the plate's.
void host_add_tooltip(void* tooltip_view, double margin, void (*away)(void));

// Shows the tooltip's window as the UI draws it now, fading in over |fade| seconds from where it
// is, or moves it, with its top left at (|left|, |top|) points in the content of |ui_view|'s
// window. The UI has placed it inside the window, so the host doesn't clamp it. With |overlay| not
// -1, what it names is in that overlay, which shows over |ui_view|'s window, such as a command bar
// row's button: |left| and |top| are in the overlay's content, and the tooltip goes over it,
// though an overlay otherwise goes over the tooltip.
void host_show_tooltip(void* ui_view, int overlay, double left, double top, double fade);

// Fades the tooltip's window out over |fade| seconds, then hides it.
void host_hide_tooltip(double fade);

// Makes the window of |card_view| the hover card's, a sidebar tab's, which floats over a browser
// window as its child when shown and never takes the keyboard. Only its plate takes the mouse, for
// its buttons; the margin round it lets the mouse through. It starts hidden. |away| is called when
// that browser window stops being key, the app stops being active, or a press or a scroll lands
// anywhere but the card, which still goes where it was aimed. The UI ends it on a key itself. The
// window reaches |margin| points past the plate on every side, for its shadow.
void host_add_hover_card(void* card_view, double margin, void (*away)(void));

// Shows the hover card's window as the UI draws it now, or moves it, with its plate's top left at
// (|left|, |top|) points in the content of |ui_view|'s window, |width| by |height|. The UI has
// placed it inside the window.
void host_show_hover_card(void* ui_view, double left, double top, double width, double height);

// Hides the hover card's window at once. The UI draws its motion in and out, so the window stays
// opaque.
void host_hide_hover_card(void);

// Calls |pressed| each time the system says it's short of memory, for the UI to let go of what it
// can do without, such as the hover cards' pictures.
void host_watch_memory_pressure(void (*pressed)(void));

// Makes the window of |link_view| the link target's, which floats over a browser window as its
// child when shown, and lets the mouse through and never takes the keyboard. It starts hidden.
// |pointer| is called with the pointer's place in the content of the browser window, in points
// from its top left, at every move of it while that window shows. As the tooltip's, its window
// reaches |margin| points past the plate.
void host_add_link_target(void* link_view, double margin, void (*pointer)(double x, double y));

// Shows the link target's window as the UI draws it now, fading in over |fade| seconds from where
// it is, or moves it, with its top left at (|left|, |top|) points in the content of |ui_view|'s
// window. The UI has placed it inside the window, so the host doesn't clamp it.
void host_show_link_target(void* ui_view, double left, double top, double fade);

// Fades the link target's window out over |fade| seconds, then hides it.
void host_hide_link_target(double fade);

// Gives |view| the keyboard in its window, from the page if the page has it.
void host_focus(void* view);

// Asks |message| in an alert, with |detail| under it and the |count| |buttons|, a "Cancel" one
// taking escape, and returns the index of the one chosen once it closes. The first is the default,
// which return chooses, if |returns|; otherwise only a click chooses it, for one nothing undoes.
int host_ask(const char* message, const char* detail, const char* const* buttons, size_t count,
             bool returns);

// Shows the close, minimize and zoom buttons of |ui_view|'s window, or hides them, as Arc does
// while its sidebar is hidden.
void host_show_window_buttons(void* ui_view, bool shown);

// Puts |ui_view|'s window's close, minimize and zoom buttons at its right, mirrored, so close is at
// the corner, and the reveal's window and the edge that brings it out at the right too, or back on
// the left: Sidebar position (#2375). The UI lays out the rest.
void host_set_sidebar_right(void* ui_view, bool right);

// The Mac's close, minimize and zoom buttons as AppKit draws them in |ui_view|'s window, for the
// copies the UI draws in their place: each one's centre across from the window's left edge, their
// row's centre down from its top, and their circles' diameter. False until a window shows all
// three out of full screen.
bool host_window_buttons(void* ui_view, double centres[3], double* y, double* diameter);

// Does what a click on the close (0), minimize (1) or zoom (2) button of |ui_view|'s window does.
void host_press_window_button(void* ui_view, int button);

// The Mac's: puts the browser window of |ui_view| full screen as its green button does, unless
// it's full screen already or on its way, as AppKit takes a window opened from a full-screen one
// there itself. Returns whether it was.
bool host_enter_full_screen(void* ui_view);

// The Mac's: whether the browser window of |ui_view| is full screen or on its way, from the start
// of its transition, before the window hears it's over.
bool host_window_full_screen(void* ui_view);

// Whether the browser window of |ui_view| is on the macOS desktop shown, as a window AppKit takes
// full screen isn't until macOS has moved to its desktop. Off the Mac, always.
bool host_window_on_active_space(void* ui_view);

// Keeps the browser window of |ui_view| over other apps' windows, its overlays over it as before,
// or with |on| false lets it go back among them (Window › Stay On Top). The Mac sets it aside
// while the window is full screen, and takes it up again after. Under Wayland, which gives a client
// no way, nothing.
void host_set_window_on_top(void* ui_view, bool on);

#ifdef _WIN32
// Windows' alone, whose browser window has no title bar for these: moves |ui_view|'s window with
// the pointer until the button that's down comes up, as a drag on a title bar does, and whether
// it's maximized. Linux has the core's (bc_window_start_move, bc_window_is_maximized).
void host_start_window_move(void* ui_view);
bool host_window_maximized(void* ui_view);
// While |ui_view|'s window is maximized, or if it closed maximized, the frame it un-maximizes to,
// in host_window_frame's terms, which leaves a maximized window's be. False, leaving them be,
// otherwise.
bool host_window_restored_frame(void* ui_view, double* x, double* y, double* width, double* height);
// Maximizes |ui_view|'s window on the screen of the frame it has, which it then un-maximizes to, as
// a window comes back when it quit maximized (bc_saved_window_maximized).
void host_maximize_window(void* ui_view);
#endif

// Makes the window of |reveal_view| the sidebar reveal of |ui_view|'s window, which shows over its
// left edge without taking the keyboard. It starts hidden. While it shows, |pointer_moved| hears,
// with |ui_view|, where the pointer is across the window's content, in points from its left edge,
// as it moves anywhere in the app's windows, and an x past them all when the app stops being the
// active one.
void host_add_reveal(void* ui_view,
                     void* reveal_view,
                     void (*pointer_moved)(void* ui_view, double x));

// Shows |ui_view|'s reveal window, |width| points wide from its browser window's left edge and as
// tall as its content, or makes it that wide if it shows.
void host_show_reveal(void* ui_view, double width);

// Hides |ui_view|'s reveal window.
void host_hide_reveal(void* ui_view);

// Makes the window of |settings_view| the Settings window: a standard one, dark, that can show
// over the browser window in full screen. It starts hidden.
void host_add_settings(void* settings_view);

// Shows the Settings window, as the UI draws it now, in front and with the keyboard. The first
// time, it's centred over the browser window in front, on its display.
void host_show_settings(void);

// Hides the Settings window, which keeps its place for the next host_show_settings.
void host_hide_settings(void);

// Closes the window with the keyboard if it's one of Chrome's with a close button, such as its
// Task Manager or DevTools undocked, as ⌘W does in Chrome, and says whether it did. A tab's dialog
// isn't one: ⌘W on it closes its tab.
bool host_close_other_window(void);

// A detent on the trackpad, as a swipe's nearest Space changes.
void host_detent(void);

// Gives the Dock icon's dot the color it had when the app last quit, kept in |profile_dir|, and
// follows the user's icon style from now on (dock.mm). For the launch, before the browser starts.
void host_dock_dot_launch(const char* profile_dir);

// Gives the Dock icon's dot |color|, as 0xRRGGBB, or without |colored| the icon's own green, once
// it's drawn off the main thread. In the Tinted and Clear icon styles the icon stays as macOS draws
// it. Asked again for the same color, it checks the style again.
void host_dock_dot(bool colored, uint32_t color);

// Whether the app is the active one, its windows taking the keyboard.
bool host_app_active(void);

// |count| tasks wait for the user, or none at 0: the Dock icon's badge on the Mac, the taskbar
// button's overlay on Windows, with the count up to 9 and then 9+ in place of the Space's dot, and
// on Linux the browser windows' urgency hint, with no count. It stays while the app is active, and
// nothing bounces, flashes or sounds.
void host_needs_you_badge(int count);

// Posts a system notification, |title| over |body|, with no sound, which comes back as
// |clicked|(|id|) once the user clicks it, the app brought forward. The first asks the system's
// leave for them on the Mac; refused, it posts nothing and asks no more. Linux's are the app's own,
// in Rust, over D-Bus.
void host_needs_you_notify(uint64_t id, const char* title, const char* body);
void host_watch_needs_you(void (*clicked)(uint64_t id));

// Calls |released| once, after the event that shows ⌃ is no longer held: ⌃ going up, a key or a
// button pressed without it, or the app going to the background. Watches the whole app, whichever
// view has the keyboard.
void host_watch_control(void (*released)(void));

// A link or a file that a drag from the page or another app carries.
typedef struct {
  // Its URL, UTF-8.
  const char* url;
  // Whether it's a file's, dragged as a file, rather than a link or text.
  bool file;
  // A tab of this app's that host_drag_tab drags, with an empty |url|, else 0.
  uint64_t tab;
} host_dragged;

// What host_take_drops hears of a drag.
enum { HOST_DRAG_OVER, HOST_DRAG_LEFT, HOST_DRAG_DROP };

// Hands drags over the UI's views, from the page or another app, to |dragged| with the view:
// HOST_DRAG_OVER as the pointer comes or moves to (x, y), in points from the view's top-left
// corner, HOST_DRAG_LEFT once it leaves or the drag ends elsewhere, and HOST_DRAG_DROP as it's let
// go there. |items| are what the drag carries. |dragged| returns whether a drop there opens any of
// them, or opened them.
void host_take_drops(bool (*dragged)(void* ui_view, int phase, double x, double y,
                                     const host_dragged* items, size_t count));

// Moves the file at |path|, a full path in UTF-8, to the system's Trash, and says whether it's gone
// from there, as it is if it was gone already.
bool host_trash(const char* path);

// Starts the system's drag of the file at |path|, a full path in UTF-8, from the pointer, during
// the event of the left button held and moved. Anywhere it's let go may take a copy of it, the
// UI's view as host_take_drops says. Calls |ended| once it's let go, with whether something took
// it. Returns false, and never calls |ended|, if it didn't start, as when there's no file there.
bool host_drag_file(const char* path, void (*ended)(bool taken));

// Starts the system's drag of |tab|, a tab's row dragged out of its window, from the pointer, during
// the event of the left button held and moved. It shows the row: the part of |ui_view|'s window at
// (|x|, |y|), |width| by |height| points from its content's top-left corner, with the pointer at
// (|grab_x|, |grab_y|) in it. It carries a type of the app's own, which only the app's UI views
// take, with the tab in the item host_take_drops hands them. Calls |ended| once it ends: taken by
// one, let go where none took it, or cancelled, as escape does, with where on the screen it was let
// go, in host_window_frame's terms, |placed| false where the system doesn't say, as under Wayland.
// Returns false, and never calls |ended|, if it didn't start, as on Windows and Linux so far
// (#1798), where the row stays in its window.
enum { HOST_TAB_TAKEN, HOST_TAB_LET_GO, HOST_TAB_CANCELLED };
bool host_drag_tab(void* ui_view, uint64_t tab, double x, double y, double width, double height,
                   double grab_x, double grab_y,
                   void (*ended)(uint64_t tab, int how, bool placed, double x, double y));

// Looks up a preview of the file at |path|, a full path in UTF-8, off the main thread: for an
// image, or a video's first frame, a thumbnail at most 96 pixels on its longer side, its aspect
// kept, else the system's icon for the file, 52 pixels square. Then calls |done| on the main thread
// with |id|, the preview as a PNG |size| bytes long, |width| by |height| pixels, and whether it's a
// thumbnail, or with a null |png| if there's no file there.
void host_file_preview(const char* path, uint64_t id,
                       void (*done)(uint64_t id, const uint8_t* png, size_t size, double width,
                                    double height, bool image));

// Plays the video at |path|, a full path in UTF-8, without sound and from its start again at its
// end, for a tile |side| pixels square. Returns it for host_video_frame, or null if this platform
// can't play one; a file that isn't a video the system plays gives no frames.
void* host_video_open(const char* path, int side);

// Copies the frame |video| shows now into |pixels|, |side| by |side| BGRA pixels with the frame's
// middle filling them, and returns true, or returns false if there's no frame since the last.
bool host_video_frame(void* video, uint8_t* pixels);

// Stops |video| and frees it.
void host_video_close(void* video);

// Puts |text| on the clipboard. A driven run puts it on a pasteboard of its own instead, so checks
// never touch the user's clipboard.
void host_copy(const char* text);
// Puts |text|, the account's secret, on the clipboard of this device alone, marked for clipboard
// histories and managers to skip, as a password manager copies a password, and clears it after
// |seconds| unless something replaced it. A driven run uses host_copy's pasteboard of its own.
// False, doing nothing, where the clipboard can't be told to keep it to itself, and the app then
// copies it through GPUI and takes it off after |seconds| itself.
bool host_copy_secret(const char* text, uint32_t seconds);
// Clears the secret host_copy_secret put on the clipboard if it's still there, as the app quits
// before its |seconds| are up.
void host_forget_secret(void);

// Whether the key event being handled now is the key repeating as it's held, not a press. False
// where the host can't tell.
bool host_key_repeated(void);

// Copies the text host_copy's clipboard or pasteboard holds into |text|, |capacity| bytes with its
// NUL, and returns its whole length in bytes without the NUL, 0 if it holds none.
size_t host_paste(char* text, size_t capacity);

// Opens |url| in the browser the protected-playback card offers for video the engine can't play:
// Safari on the Mac, and the system's default browser on Windows, which the card offers only while
// that isn't this app (#2468). Linux offers none. A driven run only logs it, so checks never launch
// another browser.
void host_open_elsewhere(const char* url);

// Shows System Settings' list of apps with Full Disk Access, which an import from Safari needs. A
// driven run only logs it, so checks never open System Settings.
void host_open_full_disk_access(void);

// Shows the system's share menu for the page at |url| titled |title|, hanging from the frame at
// |x|, |y|, |width| by |height| points in |ui_view|'s window's content.
void host_share(void* ui_view,
                double x,
                double y,
                double width,
                double height,
                const char* url,
                const char* title);

// Gives |each| the name of each of the system's share services for the page at |url|, as its
// share menu lists them, with |context|.
void host_share_services(const char* url,
                         void (*each)(void* context, const char* name),
                         void* context);

// Shares the page at |url| titled |title| with the service at |index| in host_share_services's.
void host_share_with(const char* url, const char* title, size_t index);

// The starts of today, yesterday, this week and last week in the user's calendar and time zone, in
// milliseconds since the Unix epoch.
void host_day_starts(double starts[4]);

// Updates: Sparkle on the Mac (platform/mac/updates.mm), ilo's own on Windows
// (platform/windows/updates.cc), both from the same appcast and key. Linux's are Flatpak's, which
// the app asks itself.

// On the Mac, before Chromium starts: makes sure Chromium can read its cookie key, "<name> Safe
// Storage" in the Keychain, when the app's signature is new, as after a move to another signing
// identity, and makes the item again so it's granted to the new signature (tools/requirement.sh
// says why that carries over from Apple Development to Developer ID). Where macOS asks and the
// user doesn't allow it, an alert offers Try Again until it can, or Quit, when this returns false
// and the app quits before Chromium starts. Without the key, Chromium can't read the profile's
// cookies and passwords.
bool host_keep_keychain_key(void);

// Whether this build updates itself: a release build that carries the updater, its feed and its
// key, and on Windows not an MSIX from the Microsoft Store, which the Store updates.
bool host_updates_available(void);

// Whether the Microsoft Store updates this copy: on Windows, an MSIX from it.
bool host_updated_by_store(void);

// Whether this build is a nightly, each night's build of main (APP_TRACK=nightly at build time),
// rather than a release.
bool host_nightly_build(void);

// Whether this build's updater can take nightly builds instead of its own: it updates itself and
// carries a nightly feed beside its feed (branding/update-feed-nightly).
bool host_nightly_feed(void);

// Has the updater read the nightly feed, or with false its own, from its next check on: before
// host_start_updater, from its first. Either way it installs only a build numbered above the one
// running, so a nightly that goes back to its own feed keeps its build until a release passes it.
void host_follow_nightly(bool nightly);

// Where host_start_updater's |changed| says the update stands, for Settings › About's row and
// Restart to Update at the sidebar's foot.
enum {
  // A check ended with nothing to say: on the Mac, the user closed Sparkle's window, cancelled
  // the download or skipped the version.
  HOST_UPDATE_IDLE = 0,
  HOST_UPDATE_CHECKING = 1,
  // No newer version.
  HOST_UPDATE_CURRENT = 2,
  // A newer version, which on the Mac Sparkle's own window offers.
  HOST_UPDATE_FOUND = 3,
  HOST_UPDATE_DOWNLOADING = 4,
  // Downloaded and its signature checked, to install when the app quits, or now with
  // host_install_update.
  HOST_UPDATE_READY = 5,
  HOST_UPDATE_FAILED = 6,
  // A newer version the updater won't download by itself, as Sparkle leaves one that needs the
  // user's go-ahead: host_check_for_updates shows it in Sparkle's window. The Mac's alone.
  HOST_UPDATE_AVAILABLE = 7,
};

// Starts the updater, which checks every 8 hours and downloads a newer version in the background
// with no window of its own, checks its EdDSA signature against the build's key, and installs it
// when the app quits. On the Mac that's Sparkle; on Windows, ilo's own, which runs the installer
// silently once the app has gone (host_finish_update). Calls |changed| on the main thread with a
// HOST_UPDATE_* each time the update moves on, with the version it's of, else "", and |restart| on
// the main thread once Restart to Update, or on the Mac Sparkle's own Install and Relaunch, wants
// the browser to restart (bc_restart). Does nothing where host_updates_available is false.
void host_start_updater(void (*restart)(void), void (*changed)(int stage, const char* version));

// On Windows, before host_start_updater: if a quit since the last launch started installing an
// update, calls |installed| with the update's build number and this copy's own, once. Where it
// answers that the update didn't go in, the updater's first check waits its 8 hours, so the app
// can say so and offer Try Again until the user asks.
void host_take_install(bool (*installed)(uint64_t build, uint64_t own));

// The app menu's Check for Updates…, which checks now: on the Mac, Sparkle's check, which shows
// what it finds in Sparkle's window; on Windows, the same quiet check as Settings › About's,
// which the app shows there, as the updater has no window.
void host_check_for_updates(void);

// Settings › About's check, which |changed| follows: on the Mac, Sparkle's check in the
// background, which downloads an update to install on quit with no window of its own, or while
// Sparkle shows one, Check for Updates…; on Windows, the updater's check, at once.
void host_check_for_updates_quietly(void);

// Settings › About's "Check for updates automatically" (#1793). Off, the updater asks the feed
// nothing on its own: on the Mac, Sparkle neither checks nor downloads on its schedule; on
// Windows, the updater's 8-hour timer doesn't fire. Check for Updates… still checks, once, when
// asked. Given before host_start_updater, the updater starts that way; after, a change takes
// effect at once, and turned on, the schedule runs again from the last check, which checks at
// once if one is due. Not on Linux, whose Flatpak's updates are the system's.
void host_set_automatic_update_checks(bool on);

// Restart to Update: restarts through host_start_updater's |restart|, to relaunch into the update
// that's HOST_UPDATE_READY. On the Mac, once macOS has checked the new version as it would at its
// first launch, or after 15 s. Nothing installs or relaunches before the restart goes ahead
// (host_relaunch_into_update), so a quit a page stops leaves nothing armed. Nothing if none is
// ready.
void host_install_update(void);

// The restart goes ahead (bc_host's quit_going_ahead): the ready update installs as the app quits
// and relaunches ilo, and this returns true. On the Mac, Sparkle's install, once Restart to Update
// or Sparkle's Install and Relaunch asked for it; on Windows, the installer's relaunch
// (host_finish_update). False with none to relaunch into, for the core to start the browser again.
bool host_relaunch_into_update(void);

// Chromium is quitting. On Windows, a ready update's installer starts, silently, and waits for
// this process to end before it installs, then relaunches ilo if host_relaunch_into_update asked.
// On the Mac, nothing: Sparkle installs on quit by itself.
void host_finish_update(void);

// On the Mac and Windows: calls |notes| with the release notes' URL of the update this launch is
// the first of, once, as the updater kept them when it installed it. Windows' are kept with the
// build host_take_install reads, so they're taken before it.
void host_take_update_notes(void (*notes)(const char* url));

// For the test driver: presses the button titled |title| in the alert shown. False if none shows.
bool host_choose(const char* title);

// For the test driver: the NSWindow of the browser window whose view host_ui_view made |index|th,
// from 0, once it's shown, else null.
void* host_browser_window(size_t index);

// AppleScript, the Mac's alone (scripting.mm, ilo.sdef): what a script may ask of the app's windows,
// Spaces and tabs, which the app answers from what its UI shows, and the commands it may give,
// which the app carries out as a click would. A script asks with one Apple Event at a time,
// |event| (its sender's pid and return ID), from |sender|, the app that sent it by its name. The
// strings are UTF-8, copied as bc_space_name copies them.
typedef struct {
  // Whether scripts may control the browser, asked once for each Apple Event that reads, with
  // |action| "read". The app logs it, refused if not.
  bool (*allowed)(uint64_t event, const char* sender, const char* action);
  // The windows, front to back, never a private one; the Space each shows, 0 for none of the
  // user's, and the tab it shows, 0 for none; and its name, the shown tab's title.
  size_t (*windows)(uint64_t* windows, size_t capacity);
  uint64_t (*window_space)(uint64_t window);
  uint64_t (*window_tab)(uint64_t window);
  size_t (*window_name)(uint64_t window, char* name, size_t capacity);
  // The user's Spaces, as the switcher lists them, never an agent's.
  size_t (*spaces)(uint64_t* spaces, size_t capacity);
  size_t (*space_name)(uint64_t space, char* name, size_t capacity);
  // The tabs of |space|, pinned then Today, or with 0 of the Space |window| shows.
  size_t (*tabs)(uint64_t window, uint64_t space, uint64_t* tabs, size_t capacity);
  size_t (*tab_title)(uint64_t tab, char* title, size_t capacity);
  size_t (*tab_url)(uint64_t tab, char* url, size_t capacity);
  // The commands, each logged: |window| shows |space|, or |tab|; a window opens as ⌘N's does; a
  // tab opens loading |url|, an http or https page, in |space| of |window|, or with |window| 0 as a
  // link from another app does. Each returns false or 0, and copies why into |why|, if refused.
  bool (*focus_space)(uint64_t event, const char* sender, uint64_t window, uint64_t space,
                      char* why, size_t capacity);
  bool (*select_tab)(uint64_t event, const char* sender, uint64_t window, uint64_t tab, char* why,
                     size_t capacity);
  uint64_t (*new_window)(uint64_t event, const char* sender, char* why, size_t capacity);
  uint64_t (*new_tab)(uint64_t event, const char* sender, uint64_t window, uint64_t space,
                      const char* url, char* why, size_t capacity);
} host_scripting;

// Answers scripts with |scripting|, which the app keeps for as long as it runs.
void host_set_scripting(const host_scripting* scripting);

#ifdef __cplusplus
}
#endif
