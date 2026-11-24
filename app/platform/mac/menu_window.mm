// ilo's own context menu's window (app/src/menu.rs). GPUI draws the menu in a window of its own,
// as nothing can draw over the page's native view, which goes over the browser window as its
// child and takes the keyboard while it shows, as the command bar does (AGENTS.md › Over the
// page). The menu sees every pointer event in the app first, so a press outside only closes it,
// the click going no further, but for a right press, which goes on and opens the menu of what
// it's on in its place (menu.rs has the rule).

#import <Cocoa/Cocoa.h>

#include <initializer_list>

#include "menu_window.h"

// host.mm's, for the levels and title bar its overlays have.
void HostAddOver(NSWindow* parent, NSWindow* child);
void HostMakeOverlay(NSWindow* window);
NSView* HostRevealView(void* ui_view);

namespace {

NSWindow* g_window;
NSView* g_view;
bool (*g_pointer)(int, double, double);
void (*g_dismissed)(void);
bool (*g_choose)(const char*);
bool g_shown;
// Where the menu's window goes, on screen, which the pointer is measured from before it's there.
NSRect g_frame;
// The browser window the menu is over, and the window it's the child of: that one, or its
// reveal's panel.
NSWindow* g_browser_window;
NSWindow* g_parent;
// The window that had the keyboard, which gets it back unless the user went to another.
__weak NSWindow* g_had_keyboard;
bool g_give_back;
// Bumped by each show and hide, so a show posted before a hide doesn't put the menu back.
unsigned g_showing;
// The button whose press the menu took, or -1. A press outside closes the menu and goes no
// further, and the menu is hidden by the time that button comes up.
NSInteger g_press_taken = -1;

enum Pointer { kMoved, kPressed, kRightPressed, kReleased, kScrolled };

// The view's own coordinates for a point |x|, |y| from its top left.
NSPoint InView(NSView* view, double x, double y) {
  return NSMakePoint(x, view.flipped ? y : NSHeight(view.bounds) - y);
}

NSPoint ToScreen(NSView* view, double x, double y) {
  NSPoint in_window = [view convertPoint:InView(view, x, y) toView:nil];
  return [view.window convertPointToScreen:in_window];
}

NSPoint FromScreen(NSView* view, NSPoint screen) {
  NSPoint in_view = [view convertPoint:[view.window convertPointFromScreen:screen] fromView:nil];
  return NSMakePoint(in_view.x, view.flipped ? in_view.y : NSHeight(view.bounds) - in_view.y);
}

// What the menu's coordinates are from: the browser window's content or the reveal's panel.
NSView* PlacedIn(void* ui_view, bool in_reveal) {
  NSView* ui = (__bridge NSView*)ui_view;
  NSView* reveal = in_reveal ? HostRevealView(ui_view) : nil;
  return reveal ?: ui.window.contentView;
}

// The user left the menu. Posted: the UI may be handling what made them leave.
void Dismiss(bool give_back) {
  if (!g_shown) {
    return;
  }
  g_give_back = g_give_back && give_back;
  dispatch_async(dispatch_get_main_queue(), ^{
    if (g_dismissed) {
      g_dismissed();
    }
  });
}

NSEvent* Track(NSEvent* event) {
  if (!g_shown) {
    // The rest of a click whose press the menu took goes no further either: a page that heard
    // the release, or the drag before it, would have half of the click that closed the menu.
    switch (event.type) {
      case NSEventTypeLeftMouseDown:
      case NSEventTypeRightMouseDown:
      case NSEventTypeOtherMouseDown:
        g_press_taken = -1;
        break;
      case NSEventTypeLeftMouseUp:
      case NSEventTypeRightMouseUp:
      case NSEventTypeOtherMouseUp:
        if (event.buttonNumber == g_press_taken) {
          g_press_taken = -1;
          return nil;
        }
        break;
      case NSEventTypeLeftMouseDragged:
      case NSEventTypeRightMouseDragged:
      case NSEventTypeOtherMouseDragged:
        if (event.buttonNumber == g_press_taken) {
          return nil;
        }
        break;
      default:
        break;
    }
    return event;
  }
  if (event.type == NSEventTypeKeyDown) {
    // The menu shows shortcuts but doesn't run them: one closes it, and goes no further, as it
    // was typed at the menu.
    const NSEventModifierFlags flags = event.modifierFlags;
    if (flags & (NSEventModifierFlagCommand | NSEventModifierFlagControl)) {
      Dismiss(true);
      return nil;
    }
    return event;
  }
  int kind;
  switch (event.type) {
    case NSEventTypeMouseMoved:
    case NSEventTypeLeftMouseDragged:
    case NSEventTypeRightMouseDragged:
    case NSEventTypeOtherMouseDragged:
      kind = kMoved;
      break;
    case NSEventTypeRightMouseDown:
      kind = kRightPressed;
      break;
    case NSEventTypeLeftMouseDown:
    case NSEventTypeOtherMouseDown:
      kind = kPressed;
      break;
    case NSEventTypeLeftMouseUp:
    case NSEventTypeRightMouseUp:
    case NSEventTypeOtherMouseUp:
      kind = kReleased;
      break;
    default:
      kind = kScrolled;
      break;
  }
  NSPoint at = event.locationInWindow;
  if (event.window) {
    at = [event.window convertPointToScreen:at];
  }
  const bool taken = g_pointer(kind, at.x - NSMinX(g_frame), NSMaxY(g_frame) - at.y);
  if (kind == kPressed || kind == kRightPressed) {
    g_press_taken = taken ? event.buttonNumber : -1;
  } else if (kind == kReleased && event.buttonNumber == g_press_taken) {
    g_press_taken = -1;
    return nil;
  }
  return taken ? nil : event;
}

}  // namespace

bool HostIsMenuWindow(NSWindow* window) {
  return window && window == g_window;
}

void host_add_menu_window(void* view,
                          bool (*pointer)(int kind, double x, double y),
                          void (*dismissed)(void),
                          bool (*choose)(const char* title)) {
  g_view = (__bridge NSView*)view;
  g_window = g_view.window;
  g_pointer = pointer;
  g_dismissed = dismissed;
  g_choose = choose;
  HostMakeOverlay(g_window);
  // Borderless, as GPUI made it titled, whose edge AppKit draws as a light line along the top of
  // the whole window, which reaches past the panels for their shadows (#643). The UI draws the
  // panels' shadows, inside the window.
  g_window.styleMask = NSWindowStyleMaskBorderless;
  g_window.hasShadow = NO;
  constexpr NSEventMask kEvents =
      NSEventMaskMouseMoved | NSEventMaskLeftMouseDragged | NSEventMaskRightMouseDragged |
      NSEventMaskOtherMouseDragged | NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown |
      NSEventMaskOtherMouseDown | NSEventMaskLeftMouseUp | NSEventMaskRightMouseUp |
      NSEventMaskOtherMouseUp | NSEventMaskScrollWheel | NSEventMaskKeyDown;
  [NSEvent addLocalMonitorForEventsMatchingMask:kEvents handler:^NSEvent*(NSEvent* event) {
    return Track(event);
  }];
  NSNotificationCenter* center = NSNotificationCenter.defaultCenter;
  [center addObserverForName:NSApplicationDidResignActiveNotification
                      object:nil
                       queue:nil
                  usingBlock:^(NSNotification*) {
                    Dismiss(false);
                  }];
  // Chromium makes its window key by itself, such as when a page commits; that isn't the user
  // leaving, so the menu takes the keyboard back. Another window is.
  [center addObserverForName:NSWindowDidBecomeKeyNotification
                      object:nil
                       queue:nil
                  usingBlock:^(NSNotification* notification) {
                    NSWindow* window = notification.object;
                    if (!g_shown || window == g_window) {
                      return;
                    }
                    // A tab's dialog is its browser window's too, such as Chrome's warning
                    // showing under the menu (#3209).
                    const bool tab_dialog =
                        window.parentWindow == g_browser_window &&
                        [window isKindOfClass:NSClassFromString(@"NativeWidgetMacNSWindow")];
                    if (window == g_browser_window || window == g_parent || tab_dialog) {
                      [g_window makeKeyWindow];
                    } else {
                      Dismiss(false);
                    }
                  }];
  // The menu stays where it opened, so it closes when its window changes under it.
  for (NSNotificationName name : {NSWindowWillStartLiveResizeNotification,
                                  NSWindowDidResizeNotification, NSWindowDidMoveNotification,
                                  NSWindowWillEnterFullScreenNotification,
                                  NSWindowWillExitFullScreenNotification,
                                  NSWindowWillCloseNotification,
                                  NSWindowDidMiniaturizeNotification}) {
    [center addObserverForName:name
                        object:nil
                         queue:nil
                    usingBlock:^(NSNotification* notification) {
                      if (g_shown && notification.object == g_browser_window) {
                        Dismiss(name != NSWindowWillCloseNotification);
                      }
                    }];
  }
}

void host_show_menu_window(void* ui_view,
                           bool in_reveal,
                           double left,
                           double top,
                           double width,
                           double height) {
  NSView* in = PlacedIn(ui_view, in_reveal);
  if (!g_window || !in.window) {
    return;
  }
  if (!g_shown) {
    NSWindow* key = NSApp.keyWindow;
    g_had_keyboard = key == g_window ? nil : key;
  }
  g_shown = true;
  g_give_back = true;
  g_parent = in.window;
  g_browser_window = ((__bridge NSView*)ui_view).window;
  const NSPoint origin = ToScreen(in, left, top);
  const NSRect frame = NSMakeRect(origin.x, origin.y - height, width, height);
  g_frame = frame;
  // The press that opened it, for how long it took.
  NSEvent* press = NSApp.currentEvent;
  const unsigned showing = ++g_showing;
  // It goes on screen once it has drawn this menu, which it can't while the UI is still handling
  // the click that opened it. Until then it holds the last menu it drew.
  dispatch_async(dispatch_get_main_queue(), ^{
    if (showing != g_showing) {
      return;
    }
    [g_window setFrame:frame display:NO];
    [g_view.layer setNeedsDisplay];
    [g_view.layer displayIfNeeded];
    if (g_window.parentWindow != g_parent) {
      [g_window.parentWindow removeChildWindow:g_window];
      HostAddOver(g_parent, g_window);
      // Over an overlay, such as the command bar, which isn't the main window but is a level
      // above the normal one, it goes at the overlay's level.
      if (g_window.level < g_parent.level) {
        g_window.level = g_parent.level;
      }
    }
    [g_window makeKeyAndOrderFront:nil];
    // check.sh reads these lines.
    fprintf(stderr, "browser: menu window at %.0f,%.0f %.0fx%.0f\n", left, top, width, height);
    if (press) {
      const double ms = (NSProcessInfo.processInfo.systemUptime - press.timestamp) * 1000;
      fprintf(stderr, "browser: menu window shown %.1f ms after the press\n", ms);
    }
  });
}

void host_hide_menu_window(void) {
  if (!g_shown) {
    return;
  }
  g_shown = false;
  ++g_showing;
  [g_window.parentWindow removeChildWindow:g_window];
  [g_window orderOut:nil];
  NSWindow* back = g_had_keyboard.visible ? g_had_keyboard : g_browser_window;
  g_had_keyboard = nil;
  if (g_give_back && NSApp.active && back.visible) {
    [back makeKeyWindow];
  }
  fprintf(stderr, "browser: menu window hidden\n");
}

void host_menu_area(void* ui_view, bool in_reveal, double* area) {
  NSView* in = PlacedIn(ui_view, in_reveal);
  NSScreen* screen = in.window.screen ?: NSScreen.mainScreen;
  const NSRect visible = screen.visibleFrame;
  const NSPoint top_left = FromScreen(in, NSMakePoint(NSMinX(visible), NSMaxY(visible)));
  const NSPoint bottom_right = FromScreen(in, NSMakePoint(NSMaxX(visible), NSMinY(visible)));
  area[0] = top_left.x;
  area[1] = top_left.y;
  area[2] = bottom_right.x;
  area[3] = bottom_right.y;
}

bool host_menu_window_choose(const char* title) {
  return g_shown && g_choose && g_choose(title);
}

bool host_menu_window_dismiss(void) {
  if (!g_shown || !g_dismissed) {
    return false;
  }
  g_dismissed();
  return true;
}
