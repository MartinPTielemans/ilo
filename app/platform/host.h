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
// first browser window will open, with the window's material and the sidebar as it was when the
// app last quit at that size, from |profile_dir|. For the moment between NSApp being made and the
// browser starting.
void host_show_launch_window(const char* profile_dir, double x, double y, double width,
                             double height);

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
// while a full-screen browser window has that desktop, it opens over it there. Before
// host_show_window. Off the Mac, where the window manager places windows, nothing.
void host_set_window_little(void* ui_view);

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

// Adds Little ilo's items to the Dock icon's menu, the engine's, after its New Window, as Arc's:
// New Little ilo Window, then, while one is open, Hide All Little ilo Windows, or Show All Little
// ilo Windows once they're all hidden. |little| is asked as the menu opens: 0 with no Little ilo
// window open, 1 with one on screen and 2 with all hidden. |chosen| gets the item picked: 0 New, 1
// Show All, 2 Hide All. Once the app has finished launching, as the engine's delegate is set then.
// Off the Mac, which has no Dock menu, nothing.
void host_add_dock_items(int (*little)(void), void (*chosen)(int item));

// Puts a button in the menu bar whose click calls |clicked|, which opens a Little ilo window, as
// Arc's optional one does, or with NULL takes it out again. Off until the user turns it on in
// Settings › General. It draws once and then waits for a click, so an idle browser stays asleep.
// Off the Mac, nothing, and Settings offers no switch there.
void host_set_little_status_item(void (*clicked)(void));

// Calls |changed| with the view of each browser window that becomes the key window.
void host_watch_key_window(void (*changed)(void* ui_view));

// Calls |changed| with the view of each browser window that has gone full screen, or left it, once
// macOS's transition is over.
void host_watch_full_screen(void (*changed)(void* ui_view, bool entered));

// Asks |swiped| about each swipe between pages in a browser window, at its point in |ui_view|
// from the top left, in points; one it takes, by returning true, goes no further.
void host_watch_swipes(bool (*swiped)(void* ui_view, double x, double y, bool back));

// Calls |tapped| with the view of the browser window with the keyboard each time ⌥ is pressed and
// let go with no other key, modifier, press or scroll between, the page's keyboard or the UI's.
void host_watch_option_taps(void (*tapped)(void* ui_view));

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

// The overlays: the command bar, the Space's icon and theme pickers, and a closed folder's preview.
// One shows at a time.
#define HOST_COMMAND_BAR 0
#define HOST_ICON_PICKER 1
#define HOST_THEME_PICKER 2
#define HOST_FOLDER_PREVIEW 3
// The Site Controls take the same elevation call, though they aren't one of the overlays.
#define HOST_SITE_CONTROLS 4

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

// Gives the overlay the keyboard back if it shows, the user hasn't left it, and the browser window
// it shows over or no window has it: for the UI, once it finds the overlay's window isn't key.
// The Mac's alone: Linux's and Windows' hosts close an overlay whose keyboard goes elsewhere.
void host_take_overlay_keyboard(int overlay);

// Holds the keys typed from now until the command bar or the find bar takes the keyboard, and then
// gives them to it: for a shortcut that opens one but reaches the page first, which may keep it.
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
// corner when a split view's focused page isn't the top right one.
void host_set_pane_corner(void* ui_view, double right, double top);

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

// Asks where to save the PNG |png|, |size| bytes long, in a save panel on |ui_view|'s window, named
// for when it was taken, and saves it there.
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
// corner when shown, as its child, and lets the mouse through. It starts hidden.
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

// Makes the window of |tooltip_view| the tooltip's, which floats over a browser window as its
// child when shown, and lets the mouse through and never takes the keyboard. It starts hidden.
// |away| is called when that window stops being key or the app stops being active, which hide it.
// The window reaches |margin| points past the plate on every side, for its shadow, which the UI
// draws; |left| and |top| below are the plate's.
void host_add_tooltip(void* tooltip_view, double margin, void (*away)(void));

// Shows the tooltip's window as the UI draws it now, fading in over |fade| seconds from where it
// is, or moves it, with its top left at (|left|, |top|) points in the content of |ui_view|'s
// window. The UI has placed it inside the window, so the host doesn't clamp it.
void host_show_tooltip(void* ui_view, double left, double top, double fade);

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

// The Mac's close, minimize and zoom buttons as AppKit draws them in |ui_view|'s window, for the
// copies the UI draws in their place: each one's centre across from the window's left edge, their
// row's centre down from its top, and their circles' diameter. False until a window shows all
// three out of full screen.
bool host_window_buttons(void* ui_view, double centres[3], double* y, double* diameter);

// Does what a click on the close (0), minimize (1) or zoom (2) button of |ui_view|'s window does.
void host_press_window_button(void* ui_view, int button);

#ifdef _WIN32
// Windows' alone, whose browser window has no title bar for these: moves |ui_view|'s window with
// the pointer until the button that's down comes up, as a drag on a title bar does, and whether
// it's maximized. Linux has the core's (bc_window_start_move, bc_window_is_maximized).
void host_start_window_move(void* ui_view);
bool host_window_maximized(void* ui_view);
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
// Task Manager or DevTools undocked, as ⌘W does in Chrome, and says whether it did.
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

// Opens |url| in Safari, which plays the protected video the engine can't. A driven run only logs it,
// so checks never launch Safari.
void host_open_in_safari(const char* url);

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

// Updates: Sparkle on the Mac (platform/mac/updates.mm), WinSparkle on Windows
// (platform/windows/updates.cc).

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

// Starts the updater, which checks every 8 hours. On the Mac it downloads an update in the
// background and installs it when the app quits; on Windows it offers it, and calls |quit| on the
// main thread once the installer it started waits for the browser to quit. Does nothing where
// host_updates_available is false.
void host_start_updater(void (*quit)(void));

// Checks for updates now, showing what it finds: the app menu's Check for Updates….
void host_check_for_updates(void);

// On the Mac: calls |notes| with the release notes' URL of the update this launch is the first of, once.
void host_take_update_notes(void (*notes)(const char* url));

// For the test driver: presses the button titled |title| in the alert shown. False if none shows.
bool host_choose(const char* title);

// For the test driver: the NSWindow of the browser window whose view host_ui_view made |index|th,
// from 0, once it's shown, else null.
void* host_browser_window(size_t index);

#ifdef __cplusplus
}
#endif
