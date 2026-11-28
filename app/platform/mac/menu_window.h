// ilo's own context menu's window (app/src/menu.rs), over a browser window. Coordinates are
// points from the top left of the browser window's content, or of the reveal's panel with
// |in_reveal|.

#pragma once

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// The menu's GPUI window, opened hidden. While it shows, every pointer event in the app goes to
// |pointer| first, as |kind| (0 moved, 1 pressed, 2 right-pressed, 3 released, 4 scrolled) at |x|,
// |y| from the menu window's top left, which returns whether the menu takes it. |dismissed| is
// called when the user leaves the menu for another window or app, or its browser window resizes,
// moves or goes full screen; |choose| chooses an item by title, for the driver.
// |undo| is asked when ⌘Z is typed at the menu, and returns whether the menu took it as an undo of
// a reorder it made; otherwise the shortcut closes the menu.
void host_menu_window_on_undo(bool (*undo)(void));

void host_add_menu_window(void* view,
                          bool (*pointer)(int kind, double x, double y),
                          void (*dismissed)(void),
                          bool (*choose)(const char* title));

// Puts the menu's window at |left|, |top|, |width|, |height| over the window whose UI draws in
// |ui_view|, as it draws now, with the keyboard: a browser window, or another of the app's, such
// as Settings, the command bar or the site controls, for their menus.
void host_show_menu_window(void* ui_view,
                           bool in_reveal,
                           double left,
                           double top,
                           double width,
                           double height);

// Puts the window's material under the menu's open panels, |count| of them in |panels| as left,
// top, width, height and corner radius each, in points from the menu window's top left, in a
// |light| or dark look. Asked after each show and as a submenu opens or closes.
void host_set_menu_glass(const double* panels, size_t count, bool light);

// Takes it away, giving the keyboard back to the window that had it.
void host_hide_menu_window(void);

// The screen's work area around the browser window, as left, top, right and bottom in |area|.
void host_menu_area(void* ui_view, bool in_reveal, double* area);

// For the driver: chooses |title|, or "Submenu › Item", in the menu that shows, or closes it.
// Each returns whether a menu was showing.
bool host_menu_window_choose(const char* title);
bool host_menu_window_dismiss(void);

#ifdef __cplusplus
}
#endif
