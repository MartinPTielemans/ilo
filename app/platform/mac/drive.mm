// Test driver for check.sh. With BROWSER_DRIVE=1 the app reads one command per line on stdin
// and posts the matching AppKit events to its own window. They go through the same AppKit path
// as real input (hit testing, first responder changes, key equivalents) without touching the
// owner's mouse or keyboard. Input methods are system-wide, so instead of switching the owner's,
// the driver has a scripted one of its own.
//
//   click <x> <y> [cmd] [shift] [alt] [ctrl]
//                                         left click, in points from the window's top-left
//                                         corner, with those modifiers held
//   doubleclick <x> <y>                   two left clicks, the second with a click count of 2
//   rightclick <x> <y>                    a right click
//   middleclick <x> <y>                   a click of the middle button, a mouse's wheel
//   clickkey <x> <y>                      a left click in the key window, such as the command bar
//   doubleclickkey <x> <y>                two left clicks in the key window, as doubleclick
//   rightclickkey <x> <y>                 a right click in the key window
//   drag <x> <y> <x2> <y2>                presses at (x, y), moves to (x2, y2) and lets go
//   dragkey <x> <y> <x2> <y2>             as drag, in the key window, such as Settings
//   press <x> <y> [cmd] [shift] [alt]     presses at (x, y), for a drag in steps, with those keys
//                                         held, as dragto and release take them too
//   rightpress, rightdragto, rightrelease <x> <y>
//                                         as press, dragto and release, with the right button,
//                                         as a press-drag-release through a context menu
//   dragto <x> <y>                        moves the pointer pressed to (x, y)
//   release <x> <y>                       lets go of it at (x, y)
//   move <x> <y>                          moves the pointer to (x, y), without a button down
//   key <key> [cmd] [shift] [alt] [ctrl]  <key> is a lowercase letter, a digit, ".", "-", "+",
//                                         "[", "]", return, escape, tab, space, backspace,
//                                         delete (forward), pageup, pagedown, up, down, left,
//                                         right, f12 or f10. "+" is on "-"'s key, as on a Danish
//                                         keyboard
//   keydown <key> [repeat]                <key>, as key names it, going down, or with repeat the
//                                         repeat the keyboard sends while it's held, to the key
//                                         window then, as a held key's repeats go
//   keyup <key>                           <key> let go
//   flags [cmd] [shift] [alt] [ctrl]      the modifiers held from now, as a modifier key going
//                                         down or up; key holds them only for its own key
//   type <text>                           lowercase letters, dots and spaces
//   insert <text>                         commits <text>, any characters, to the key window's
//                                         text input, as the character palette does for an emoji.
//                                         The app's asking for the palette only logs "browser:
//                                         character palette shown"
//   scroll <x> <y> <dy> <count>           <count> trackpad scroll events at (x, y), each moving
//                                         the content up by <dy> points, at a trackpad's pace
//   swipe <x> <y> <dx> <dy> <count> [<rest>]
//                                         two fingers at (x, y): <count> trackpad scroll events,
//                                         each <dx> points right and <dy> down, from the touch's
//                                         start, then its end after <rest> ms more, else at once
//   pageswipe <x> <y> back|forward        a swipe between pages at (x, y), as three fingers or
//                                         a mouse's swipe send one, handed to the host's monitor
//   wheel <x> <y> <dx>                    a mouse wheel's tick across at (x, y), <dx> lines right
//   dragover <x> <y> [url|file|text <s>]  a drag from another app at (x, y), carrying the URL,
//                                         the file at the path or the text <s>, handed to the view
//                                         AppKit would hand it to, which it logs with whether it
//                                         takes it, as "browser: drag over <view class> taken"
//   dragdrop <x> <y> [url|file|text <s>]  lets go of that drag there, as dragover, then logs
//                                         "browser: drag dropped" or "browser: drag refused"
//   dragleave                             takes the drag away, as cancelled
//   resize <width> <height>               resizes the window, keeping its top-left corner
//   fullscreen                            enters or leaves full screen, like the green button
//   display add <width> <height> [main]   plugs in a second display of that size in points, a
//                                         virtual one, the main one with the menu bar if main
//                                         says so, as an external monitor often is, logging "browser: display added at <x>
//                                         <y> <width> <height>" in place's terms, or "browser:
//                                         display not added: <why>"
//   display remove                        pulls it out again, logging "browser: display removed"
//                                         once AppKit's screens no longer have it
//   minimize                              minimizes the window to the Dock, or brings it back
//   hide                                  hides the app, as ⌘H does, or brings it back
//   place <x> <y> <width> <height>        moves and resizes the window to that frame, in points
//                                         from the main display's top-left corner
//   screen                                logs the screen the app opens windows on, the main
//                                         display or with BROWSER_DISPLAY=builtin the Mac's own
//                                         (host_screens), as "browser: screen at <x> <y> <width>
//                                         <height>" in place's terms, so a check places windows
//                                         there and not on the main display it may be kept off
//   order                                 logs the browser windows on screen front to back and
//                                         the key one, as "browser: order window <n> window <m>
//                                         …, key window <n>|none", numbered as window numbers
//                                         them, the driven one too, after each one's frame, as
//                                         "browser: frame window <n> <x> <y> <width> <height>"
//                                         in place's terms
//   layout                                logs the frames of the window's views and child
//                                         windows, the centres of its close, minimize and
//                                         zoom buttons, the frame of the page that has the
//                                         keyboard, as "browser: layout keyboard <x>,<y>
//                                         <w>x<h>" or "none", and whether it's full screen, as
//                                         "browser: layout full screen <0|1>"
//   state                                 nothing, for the done line's report
//   title [<text>]                        logs the key window's title, as "browser: key window
//                                         titled <title>", such as a window of Chrome's. With
//                                         <text>, once the title has it, for up to 10 s, as a
//                                         window Chrome opens takes the keyboard and its title a
//                                         moment after the command that opens it; else "browser:
//                                         no key window titled <text>, windows <title> | <title>",
//                                         the app's windows shown, the key one marked "(key)"
//   pasteboard                            logs the text on the run's pasteboard, which stands in
//                                         for the clipboard, as "browser: pasteboard text <text>"
//   savepanel <path>                      picks <path> in the save panel the app began, once it
//                                         has, through the panel's completion handler, logging
//                                         "browser: save panel answered", or "browser: no save
//                                         panel" if none begins within 5 s. The driver keeps each
//                                         save panel the app begins off screen, logging "browser:
//                                         save panel offers <name> in <folder>"
//   front                                 logs which of the app's windows other apps take for
//                                         the one in front, as "browser: front <first>, main
//                                         <window>, key <window>, ax focused <window>, ax main
//                                         <window>". <first> is its first window on screen at
//                                         the normal level in the window server's order, which
//                                         T3 Code's window capture takes. Each is "driven",
//                                         "window <n>" for another browser window, "none", or
//                                         "<class> <width>x<height> <accessibility role>"
//   backdrop <grey>|off                   a window of the driver's in <grey> (0–255, sRGB) behind
//                                         the browser window and 100 points past its edges, for
//                                         the window's material to blur something known. Each
//                                         frame it draws logs "browser: backdrop <grey> drawn,
//                                         window at <x>,<y> <w>x<h> on <w>x<h>": the browser
//                                         window's frame and its screen's size, in points from
//                                         that screen's top left
//   menu <title>                          chooses the main menu's item titled <title>, in a
//                                         submenu too, or in the submenu <title> names first
//                                         as "Submenu › Item"
//   menubar <menu>                        logs the items of the main menu's <menu>, as
//                                         "browser: menu bar <menu>: <titles>", enabled as
//                                         AppKit would show them now, written as "menu shown"
//                                         writes a menu's, and chooses none
//   dock [<title>]                        chooses the Dock menu's item titled <title>, the menu
//                                         the Dock icon's right click opens, or with none logs
//                                         its items, as "browser: dock menu <titles>"
//   openmenu <menu>                       opens the main menu's <menu>, the same menu the menu
//                                         bar opens, near the window's top left, tracked until
//                                         dismiss or choose closes it. Commands still come in
//                                         while it's open, as with any menu
//   choose <title>                        chooses the item titled <title> in the menu a right
//                                         click showed, the page's too, or the button in the
//                                         alert shown, Chrome's too
//   dismiss                               closes the menu a right click or openmenu showed,
//                                         choosing none
//   ime on|off                            the scripted input method, for keys to the UI or the
//                                         page, Chromium's or Safari's engine's: letters add to a
//                                         composition, space converts it to 日本語, return
//                                         commits it and escape cancels it
//   window <n>                            drives the n-th browser window opened, from 1, from
//                                         now, bringing it to the front with the keyboard as a
//                                         click on it does, logging "browser: drive window <n>"
//   agentspace <agent> <name>             makes a Space named <name> for the agent with the id
//                                         <agent> and the name <name>, as agent code will once
//                                         there is some, logging "browser: made space <space> for
//                                         agent <agent>"
//   agentmayact <agent> <tab> [action]    logs whether the agent may do the action, "navigate" if none,
//                                         to the tab, as "browser: agent <agent> may act on
//                                         <tab>: <0|1>", and if not "browser: agent <agent>
//                                         refused <tab>: <reason>"
//   agentrelease <agent> <tab>            the agent releases the tab, as agent code will, logging
//                                         "browser: agent <agent> released <tab>: <0|1>", whether
//                                         it had a session there
//   agentdisconnect <agent>               the agent's client disconnects, logging "browser: agent
//                                         <agent> disconnected, <n> sessions ended"
//   takeback <tab>                        the user takes the tab back from its agent, logging
//                                         "browser: took back <tab>: <0|1>"
//   age <seconds>                         makes every tab look <seconds> longer unseen, as hours
//                                         later, for the next tab shown to unload those
//                                         bc_unload_idle_tabs lets go. From then each check logs
//                                         "browser: idle tabs checked" once it's done
//
// Clicks, drags and moves go to the app's window on top at that point of the browser window
// driven, such as the sidebar reveal over its edge, scrolls and layout to the browser window
// driven, and keys and clickkey to the key window. It logs "browser: drive <n>: <command>" when
// command n arrives and "browser: drive <n> done, key <window class>, focus <view class>, active
// <0|1>, marked <location,length|none>" once AppKit has handled its events, "browser: menu tracked
// <titles>" as a menu AppKit tracks opens, such as DevTools' page's, "browser: modal shown <texts
// and buttons>" as an alert does, and "browser: desktop space changed" once macOS has slid to
// another Space. A page ignores clicks while the app isn't the active app, and macOS doesn't let a
// background process make it active.
#import <Cocoa/Cocoa.h>
#import <objc/runtime.h>

#include <cerrno>
#include <cstdio>
#include <map>
#include <sstream>
#include <string>
#include <thread>

#include <unistd.h>

#include "browser_core.h"
#include "../host.h"
#include "menu_window.h"

// A drag from another app, as AppKit describes it to the view under the pointer: what it carries
// and where it is.
@interface DriveDrag : NSObject
@property(nonatomic) NSPasteboard* draggingPasteboard;
@property(nonatomic) NSPoint draggingLocation;
@end

@implementation DriveDrag
- (id)draggingSource {
  return nil;
}

- (NSDragOperation)draggingSourceOperationMask {
  return NSDragOperationEvery;
}
@end

// The backdrop command's grey. It logs each frame it draws, since the browser window's material
// blurs it only once that frame is on screen.
@interface DriveBackdropView : NSView
@property(nonatomic) int level;
// Where the browser window is, for the log.
@property(nonatomic, copy) NSString* place;
@end

@implementation DriveBackdropView
- (void)drawRect:(NSRect)rect {
  [[NSColor colorWithSRGBRed:self.level / 255.0
                       green:self.level / 255.0
                        blue:self.level / 255.0
                       alpha:1] setFill];
  NSRectFill(rect);
  // On the main queue's next turn, once AppKit has handed the frame to the window server.
  NSString* line = [NSString stringWithFormat:@"browser: backdrop %d drawn, %@", self.level,
                                              self.place];
  dispatch_async(dispatch_get_main_queue(), ^{
    fprintf(stderr, "%s\n", line.UTF8String);
  });
}
@end

// CoreGraphics' virtual display, a private class display tools such as DeskPad use: the only way
// to plug a second display in and pull it out again without a cable, as when a window's display
// goes away (#415). No SDK header has it, so it's declared here, and the display command looks
// the classes up at run time, so a macOS without them fails the command, not the launch.
@interface CGVirtualDisplayDescriptor : NSObject
@property(retain, nonatomic) dispatch_queue_t queue;
@property(retain, nonatomic) NSString* name;
@property(nonatomic) unsigned int maxPixelsWide;
@property(nonatomic) unsigned int maxPixelsHigh;
@property(nonatomic) CGSize sizeInMillimeters;
@property(nonatomic) unsigned int vendorID;
@property(nonatomic) unsigned int productID;
@property(nonatomic) unsigned int serialNum;
@end

@interface CGVirtualDisplayMode : NSObject
- (instancetype)initWithWidth:(unsigned int)width
                       height:(unsigned int)height
                  refreshRate:(double)refreshRate;
@end

@interface CGVirtualDisplaySettings : NSObject
@property(retain, nonatomic) NSArray* modes;
@property(nonatomic) unsigned int hiDPI;
@end

@interface CGVirtualDisplay : NSObject
@property(readonly, nonatomic) unsigned int displayID;
- (instancetype)initWithDescriptor:(CGVirtualDisplayDescriptor*)descriptor;
- (BOOL)applySettings:(CGVirtualDisplaySettings*)settings;
@end

namespace {

// A character typed with ⇧, such as + or ?, has its key's code on a US keyboard.
const std::map<std::string, unsigned short> kKeyCodes = {
    {"a", 0},  {"s", 1},  {"d", 2},  {"f", 3},  {"h", 4},  {"g", 5},       {"z", 6},
    {"x", 7},  {"c", 8},  {"v", 9},  {"b", 11}, {"q", 12}, {"w", 13},      {"e", 14},
    {"r", 15}, {"y", 16}, {"t", 17}, {"o", 31}, {"u", 32}, {"i", 34},      {"p", 35},
    {"l", 37}, {"j", 38}, {"k", 40}, {"n", 45}, {"m", 46}, {"return", 36}, {"tab", 48},
    {" ", 49}, {"space", 49}, {".", 47}, {"escape", 53}, {"1", 18},   {"2", 19},
    {"3", 20}, {"4", 21}, {"5", 23}, {"6", 22}, {"7", 26}, {"8", 28},      {"9", 25},
    {"0", 29}, {"-", 27}, {"+", 27}, {"[", 33}, {"]", 30}, {"up", 126}, {"down", 125},
    {"left", 123}, {"right", 124}, {"backspace", 51}, {",", 43}, {"delete", 117},
    {"pageup", 116}, {"pagedown", 121}, {"/", 44}, {"?", 44}, {"=", 24}, {"&", 26}, {"_", 27},
    {":", 41}, {"@", 19}, {"f12", 111}, {"f10", 109}, {"f6", 97},
};
// AppKit's characters for the arrows are NSUpArrowFunctionKey to NSRightArrowFunctionKey, and for
// the others NSDeleteFunctionKey, NSPageUpFunctionKey, NSPageDownFunctionKey, NSF12FunctionKey,
// NSF10FunctionKey and NSF6FunctionKey.
const std::map<std::string, std::string> kCharacters = {
    {"return", "\r"},
    {"tab", "\t"},
    {"space", " "},
    {"escape", "\x1b"},
    {"backspace", "\x7f"},
    {"up", "\xEF\x9C\x80"},
    {"down", "\xEF\x9C\x81"},
    {"left", "\xEF\x9C\x82"},
    {"right", "\xEF\x9C\x83"},
    {"delete", "\xEF\x9C\xA8"},
    {"pageup", "\xEF\x9C\xAC"},
    {"pagedown", "\xEF\x9C\xAD"},
    {"f12", "\xEF\x9C\x8F"},
    {"f10", "\xEF\x9C\x8D"},
    {"f6", "\xEF\x9C\x89"},
};

// Marks the end of a command's events. AppKit handles events in the order they were posted.
constexpr short kDoneSubtype = 0x5350;

// The window the backdrop command puts behind the browser window.
NSWindow* g_backdrop;

// The menu AppKit tracks, such as the context menu of DevTools' page, which Chrome shows. choose
// picks from it when none of the app's own menus is showing.
__weak NSMenu* g_tracked;

// The window a full-screen transition AppKit announced is moving, and which way, until it's done.
__weak NSWindow* g_full_screen_window;
bool g_full_screen_entering = false;

// One step of that transition, on the uptime clock the core's and the app's lines read. A "resized"
// step has the window's content size.
void LogFullScreenStep(bool entering, const char* step, NSWindow* window) {
  const double ms = clock_gettime_nsec_np(CLOCK_UPTIME_RAW) / 1e6;
  g_full_screen_entering = entering;
  if (window) {
    const NSSize size = [window contentRectForFrameRect:window.frame].size;
    fprintf(stderr, "browser: full screen %s %s %.0fx%.0f at %.1f ms\n", entering ? "enter" : "exit",
            step, size.width, size.height, ms);
  } else {
    fprintf(stderr, "browser: full screen %s %s at %.1f ms\n", entering ? "enter" : "exit", step,
            ms);
  }
}

// The display the display command added, until it takes it away. Letting go of it removes it.
CGVirtualDisplay* g_display;

// The screen AppKit has for display |display|, or nil.
NSScreen* ScreenOf(unsigned display) {
  for (NSScreen* screen in NSScreen.screens) {
    if ([screen.deviceDescription[@"NSScreenNumber"] unsignedIntValue] == display) {
      return screen;
    }
  }
  return nil;
}

// Calls |then| once AppKit's screens are as |ready| wants, with true, or with false after 10 s.
void AwaitScreens(bool (^ready)(void), void (^then)(bool)) {
  __block bool finished = false;
  __block id observer;
  void (^check)(bool) = ^(bool last) {
    const bool there = ready();
    if (finished || (!there && !last)) {
      return;
    }
    finished = true;
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    observer = nil;
    then(there);
  };
  observer = [NSNotificationCenter.defaultCenter
      addObserverForName:NSApplicationDidChangeScreenParametersNotification
                  object:nil
                   queue:nil
              usingBlock:^(NSNotification*) {
                check(false);
              }];
  check(false);
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
    check(true);
  });
}

// Makes display |display| the main one, with the menu bar, and the main one before it its right
// neighbour, until the app quits, as an external monitor set as the main display is.
void MakeMain(unsigned display, void (^then)(bool)) {
  const CGDirectDisplayID before = CGMainDisplayID();
  CGDisplayConfigRef config;
  if (CGBeginDisplayConfiguration(&config) != kCGErrorSuccess) {
    then(false);
    return;
  }
  CGConfigureDisplayOrigin(config, display, 0, 0);
  CGConfigureDisplayOrigin(config, before, (int32_t)CGDisplayPixelsWide(display), 0);
  if (CGCompleteDisplayConfiguration(config, kCGConfigureForAppOnly) != kCGErrorSuccess) {
    then(false);
    return;
  }
  AwaitScreens(
      ^{
        return [NSScreen.screens.firstObject.deviceDescription[@"NSScreenNumber"]
                   unsignedIntValue] == display;
      },
      then);
}

// Adds a display of |width| × |height| points, beside the others, the main one if |main|, and logs
// where, as "browser: display added at <x> <y> <width> <height>" in place's terms, or why it
// couldn't.
void AddDisplay(unsigned width, unsigned height, bool main, void (^done)(void)) {
  Class descriptor_class = NSClassFromString(@"CGVirtualDisplayDescriptor");
  Class settings_class = NSClassFromString(@"CGVirtualDisplaySettings");
  Class mode_class = NSClassFromString(@"CGVirtualDisplayMode");
  Class display_class = NSClassFromString(@"CGVirtualDisplay");
  if (!descriptor_class || !settings_class || !mode_class || !display_class) {
    fprintf(stderr, "browser: display not added: this macOS has no CGVirtualDisplay\n");
    done();
    return;
  }
  CGVirtualDisplayDescriptor* descriptor = [[descriptor_class alloc] init];
  descriptor.queue = dispatch_get_main_queue();
  descriptor.name = @"Drive";
  descriptor.maxPixelsWide = width;
  descriptor.maxPixelsHigh = height;
  // About 100 points an inch, as an external monitor's.
  descriptor.sizeInMillimeters = CGSizeMake(width * 0.254, height * 0.254);
  descriptor.vendorID = 0xb0c;
  descriptor.productID = 0x415;
  descriptor.serialNum = 1;
  CGVirtualDisplay* display = [[display_class alloc] initWithDescriptor:descriptor];
  CGVirtualDisplaySettings* settings = [[settings_class alloc] init];
  settings.hiDPI = 0;
  settings.modes = @[ [[mode_class alloc] initWithWidth:width height:height refreshRate:60] ];
  if (!display || !display.displayID || ![display applySettings:settings]) {
    fprintf(stderr, "browser: display not added: CoreGraphics refused the virtual display\n");
    done();
    return;
  }
  g_display = display;
  const unsigned id = display.displayID;
  void (^log)(bool) = ^(bool added) {
    if (!added) {
      fprintf(stderr, "browser: display not added: AppKit has no screen for display %u%s\n", id,
              main ? " as the main one" : "");
    } else {
      const CGFloat top = NSHeight(NSScreen.screens.firstObject.frame);
      const NSRect frame = ScreenOf(id).frame;
      fprintf(stderr, "browser: display added at %.0f %.0f %.0f %.0f\n", NSMinX(frame),
              top - NSMaxY(frame), NSWidth(frame), NSHeight(frame));
    }
    done();
  };
  AwaitScreens(
      ^{
        return ScreenOf(id) != nil;
      },
      ^(bool added) {
        added && main ? MakeMain(id, log) : log(added);
      });
}

// The browser window the window command picked, from 0.
size_t g_driven = 0;

// The browser window driven, once it's on screen.
NSWindow* MainWindow() {
  NSWindow* window = (__bridge NSWindow*)host_browser_window(g_driven);
  return window.visible ? window : nil;
}

void PostMouse(NSWindow* window,
               NSEventType type,
               double x,
               double y,
               int count,
               NSEventModifierFlags flags = 0) {
  NSEvent* event = [NSEvent mouseEventWithType:type
                                      location:NSMakePoint(x, NSHeight(window.frame) - y)
                                 modifierFlags:flags
                                     timestamp:NSProcessInfo.processInfo.systemUptime
                                  windowNumber:window.windowNumber
                                       context:nil
                                   eventNumber:0
                                    clickCount:count
                                      pressure:1];
  // AppKit gives every event it makes button 0, and GPUI takes the button from the number.
  const bool right = type == NSEventTypeRightMouseDown || type == NSEventTypeRightMouseUp;
  if (right || type == NSEventTypeOtherMouseDown || type == NSEventTypeOtherMouseUp) {
    CGEventRef numbered = CGEventCreateCopy(event.CGEvent);
    CGEventSetIntegerValueField(numbered, kCGMouseEventButtonNumber,
                                right ? kCGMouseButtonRight : kCGMouseButtonCenter);
    event = [NSEvent eventWithCGEvent:numbered];
    CFRelease(numbered);
  }
  [NSApp postEvent:event atStart:NO];
}

// Hands |move| to the owner of every tracking area in |view| and under it that asks for moves and
// holds the pointer, as AppKit does with the window server's moves and not with posted ones, but
// to |hit|, which has it.
void TrackMove(NSView* view, NSEvent* move, NSView* hit) {
  if (view.hiddenOrHasHiddenAncestor) {
    return;
  }
  NSPoint at = [view convertPoint:move.locationInWindow fromView:nil];
  for (NSTrackingArea* area in view.trackingAreas) {
    NSTrackingAreaOptions options = area.options;
    NSRect rect = options & NSTrackingInVisibleRect ? view.visibleRect : area.rect;
    bool active = options & NSTrackingActiveAlways ||
                  (options & NSTrackingActiveInActiveApp && NSApp.active) ||
                  (options & NSTrackingActiveInKeyWindow && view.window.keyWindow) ||
                  (options & NSTrackingActiveWhenFirstResponder &&
                   view.window.firstResponder == view);
    if (options & NSTrackingMouseMoved && active && NSPointInRect(at, rect) &&
        area.owner != hit && [area.owner respondsToSelector:@selector(mouseMoved:)]) {
      [area.owner mouseMoved:move];
    }
  }
  for (NSView* subview in view.subviews) {
    TrackMove(subview, move, hit);
  }
}

// (x, y) in |from| as a point in |to|, each from its window's top-left corner.
void Convert(NSWindow* from, NSWindow* to, double* x, double* y) {
  NSPoint screen = [from convertPointToScreen:NSMakePoint(*x, NSHeight(from.frame) - *y)];
  NSPoint at = [to convertPointFromScreen:screen];
  *x = at.x;
  *y = NSHeight(to.frame) - at.y;
}

// The window of the app's that the pointer at (x, y) in |browser| is over, as the window server
// picks it: the top one there, such as the sidebar reveal over the browser window's edge. A window
// that clicks pass through, such as the command bar's shadow, is skipped, as the server skips it.
// Another app's window is skipped too, such as a notification over the top right corner, since the
// driver posts only to the app's own windows.
NSWindow* WindowAt(NSWindow* browser, double x, double y) {
  NSPoint screen = [browser convertPointToScreen:NSMakePoint(x, NSHeight(browser.frame) - y)];
  NSInteger number = 0;
  while ((number = [NSWindow windowNumberAtPoint:screen belowWindowWithWindowNumber:number])) {
    NSWindow* window = [NSApp windowWithWindowNumber:number];
    if (!window) {
      continue;
    }
    if (!window.visible) {
      break;
    }
    if (!window.ignoresMouseEvents) {
      return window;
    }
  }
  return browser;
}

void Click(NSWindow* window, double x, double y, int count, NSEventModifierFlags flags = 0) {
  PostMouse(window, NSEventTypeLeftMouseDown, x, y, count, flags);
  PostMouse(window, NSEventTypeLeftMouseUp, x, y, count, flags);
}

// The modifiers named in the rest of |words|: cmd, shift, alt and ctrl.
NSEventModifierFlags Modifiers(std::istringstream& words) {
  NSEventModifierFlags flags = 0;
  std::string modifier;
  while (words >> modifier) {
    flags |= modifier == "cmd"     ? NSEventModifierFlagCommand
             : modifier == "shift" ? NSEventModifierFlagShift
             : modifier == "alt"   ? NSEventModifierFlagOption
             : modifier == "ctrl"  ? NSEventModifierFlagControl
                                   : 0;
  }
  return flags;
}

// Posts |key| to |window| as |types|, each a repeat if |repeat| says so.
bool Key(NSWindow* window,
         const std::string& key,
         NSEventModifierFlags flags,
         std::initializer_list<NSEventType> types = {NSEventTypeKeyDown, NSEventTypeKeyUp},
         bool repeat = false) {
  auto code = kKeyCodes.find(key);
  if (code == kKeyCodes.end()) {
    return false;
  }
  auto special = kCharacters.find(key);
  NSString* characters = @((special == kCharacters.end() ? key : special->second).c_str());
  // A keyboard reports ⇧ with a letter as the capital, even ignoring modifiers, and AppKit matches
  // the menu's key equivalents by it.
  if ((flags & NSEventModifierFlagShift) && key.size() == 1 && key[0] >= 'a' && key[0] <= 'z') {
    characters = characters.uppercaseString;
  }
  // The keyboard marks arrow keys as function and keypad keys, and the keys above them as function
  // keys.
  if (key == "up" || key == "down" || key == "left" || key == "right") {
    flags |= NSEventModifierFlagFunction | NSEventModifierFlagNumericPad;
  } else if (key == "delete" || key == "pageup" || key == "pagedown" || key == "f12" ||
             key == "f10" || key == "f6") {
    flags |= NSEventModifierFlagFunction;
  }
  for (NSEventType type : types) {
    [NSApp postEvent:[NSEvent keyEventWithType:type
                                      location:NSZeroPoint
                                 modifierFlags:flags
                                     timestamp:NSProcessInfo.processInfo.systemUptime
                                  windowNumber:window.windowNumber
                                       context:nil
                                    characters:characters
                   charactersIgnoringModifiers:characters
                                     isARepeat:repeat
                                       keyCode:code->second]
             atStart:NO];
  }
  return true;
}

// AppKit has no maker for scroll events, so they start as Quartz events, and AppKit reads their
// window and their position in it from data no public call sets. Without both, AppKit drops the
// event. Field 51 is where an NSEvent made with a window number keeps it; CGEventSetWindowLocation
// is a private CoreGraphics call. Fine for a test driver, never for the browser.
constexpr CGEventField kWindowNumberField = (CGEventField)51;
extern "C" void CGEventSetWindowLocation(CGEventRef event, CGPoint location);

}  // namespace

// host.mm's, as AppKit has no way to make a swipe event of one's own.
bool SwipeBetweenPages(NSWindow* window, NSPoint in_window, bool back);

namespace {

// Trackpads report about every 8 ms while a finger moves. Each event carries pixel deltas, as a
// trackpad's do, so Chromium scrolls by exactly that much instead of animating a wheel tick.
// The drag dragover holds, on a pasteboard of its own, and the view it's over.
DriveDrag* g_drag;
NSView* g_drag_view;

// The view AppKit hands a drag at |location|, in the window's coordinates, to: the deepest one
// there registered for something the drag carries.
NSView* DragDestination(NSWindow* window, NSPoint location, NSPasteboard* pasteboard) {
  NSView* view = [window.contentView.superview hitTest:location];
  NSSet* carried = [NSSet setWithArray:pasteboard.types];
  for (; view; view = view.superview) {
    if ([carried intersectsSet:[NSSet setWithArray:view.registeredDraggedTypes]]) {
      return view;
    }
  }
  return nil;
}

// Ends the drag, at the view it's over, and lets go of its pasteboard.
void EndDrag() {
  id<NSDraggingDestination> view = (id<NSDraggingDestination>)g_drag_view;
  if ([view respondsToSelector:@selector(draggingEnded:)]) {
    [view draggingEnded:(id<NSDraggingInfo>)g_drag];
  }
  [g_drag.draggingPasteboard releaseGlobally];
  g_drag = nil;
  g_drag_view = nil;
}

// Moves the drag to (x, y), from the window's top-left corner, starting it with what |words| say
// it carries if there's none, and returns whether the view there takes it.
bool DragOver(NSWindow* window, double x, double y, std::istringstream& words) {
  if (!g_drag) {
    std::string kind, text;
    words >> kind;
    std::getline(words >> std::ws, text);
    NSString* value = @(text.c_str());
    NSPasteboard* pasteboard = [NSPasteboard pasteboardWithUniqueName];
    if (kind == "url" || kind == "file") {
      NSURL* url = kind == "url" ? [NSURL URLWithString:value] : [NSURL fileURLWithPath:value];
      [pasteboard writeObjects:@[ url ]];
    } else if (kind == "text") {
      [pasteboard setString:value forType:NSPasteboardTypeString];
    } else {
      [pasteboard releaseGlobally];
      return false;
    }
    g_drag = [[DriveDrag alloc] init];
    g_drag.draggingPasteboard = pasteboard;
  }
  g_drag.draggingLocation = NSMakePoint(x, NSHeight(window.frame) - y);
  NSView* view = DragDestination(window, g_drag.draggingLocation, g_drag.draggingPasteboard);
  id<NSDraggingInfo> info = (id<NSDraggingInfo>)g_drag;
  NSDragOperation operation = NSDragOperationNone;
  if (view != g_drag_view) {
    id<NSDraggingDestination> left = (id<NSDraggingDestination>)g_drag_view;
    if ([left respondsToSelector:@selector(draggingExited:)]) {
      [left draggingExited:info];
    }
    g_drag_view = view;
    if ([view respondsToSelector:@selector(draggingEntered:)]) {
      operation = [(id<NSDraggingDestination>)view draggingEntered:info];
    }
  } else if ([view respondsToSelector:@selector(draggingUpdated:)]) {
    operation = [(id<NSDraggingDestination>)view draggingUpdated:info];
  }
  fprintf(stderr, "browser: drag over %s %s\n", view ? object_getClassName(view) : "nothing",
          operation ? "taken" : "refused");
  return operation != NSDragOperationNone;
}

void Scroll(NSWindow* window, double x, double y, double dy, int count, void (^done)(void)) {
  long window_number = window.windowNumber;
  __block int left = count;
  dispatch_source_t timer =
      dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
  dispatch_source_set_timer(timer, DISPATCH_TIME_NOW, 8 * NSEC_PER_MSEC, NSEC_PER_MSEC);
  dispatch_source_set_event_handler(timer, ^{
    CGEventRef event =
        CGEventCreateScrollWheelEvent2(nullptr, kCGScrollEventUnitPixel, 1, (int32_t)-dy, 0, 0);
    CGEventSetIntegerValueField(event, kWindowNumberField, window_number);
    CGEventSetWindowLocation(event, CGPointMake(x, y));
    [NSApp postEvent:[NSEvent eventWithCGEvent:event] atStart:NO];
    CFRelease(event);
    if (--left == 0) {
      dispatch_source_cancel(timer);
      done();
    }
  });
  dispatch_resume(timer);
}

// Two fingers, as a trackpad reports them: a touch that begins, moves, rests for |rest_ms| and
// ends, with the phases AppKit reads from Quartz's field.
void Swipe(NSWindow* window, double x, double y, double dx, double dy, int count, int rest_ms,
           void (^done)(void)) {
  long window_number = window.windowNumber;
  void (^post)(double, double, int64_t) = ^(double across, double along, int64_t phase) {
    CGEventRef event = CGEventCreateScrollWheelEvent2(nullptr, kCGScrollEventUnitPixel, 2,
                                                      (int32_t)along, (int32_t)across, 0);
    CGEventSetIntegerValueField(event, kCGScrollWheelEventScrollPhase, phase);
    CGEventSetIntegerValueField(event, kWindowNumberField, window_number);
    CGEventSetWindowLocation(event, CGPointMake(x, y));
    [NSApp postEvent:[NSEvent eventWithCGEvent:event] atStart:NO];
    CFRelease(event);
  };
  __block int sent = 0;
  dispatch_source_t timer =
      dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
  dispatch_source_set_timer(timer, DISPATCH_TIME_NOW, 8 * NSEC_PER_MSEC, NSEC_PER_MSEC);
  dispatch_source_set_event_handler(timer, ^{
    post(dx, dy, sent == 0 ? kCGScrollPhaseBegan : kCGScrollPhaseChanged);
    if (++sent < count) {
      return;
    }
    dispatch_source_cancel(timer);
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)rest_ms * NSEC_PER_MSEC),
                   dispatch_get_main_queue(), ^{
                     post(0, 0, kCGScrollPhaseEnded);
                     done();
                   });
  });
  dispatch_resume(timer);
}

// The scripted input method makes the calls a Japanese one makes, at the same point: while the
// view handles the key. GPUI hands keys to the input context, Chromium's page view interprets them
// itself, and WebKit's asks the input context without waiting, so each gets its own hook. The
// composition is nil when there isn't one.
bool g_ime_on = false;
NSString* g_composition;
IMP g_handle_event;
IMP g_interpret_key_events;
IMP g_handle_event_by_input_method;

// Returns false for a key the input method leaves to the view.
bool ImeKey(id<NSTextInputClient> client, NSEvent* event) {
  constexpr NSEventModifierFlags kShortcut =
      NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption;
  if (!g_ime_on || event.type != NSEventTypeKeyDown || (event.modifierFlags & kShortcut)) {
    return false;
  }
  NSString* characters = event.characters;
  bool letter = characters.length == 1 && [NSCharacterSet.lowercaseLetterCharacterSet
                                              characterIsMember:[characters characterAtIndex:0]];
  if (!g_composition && !letter) {
    return false;
  }
  NSRange none = NSMakeRange(NSNotFound, 0);
  switch (event.keyCode) {
    case 36:  // return
      [client insertText:g_composition replacementRange:none];
      g_composition = nil;
      return true;
    case 53:  // escape
      [client setMarkedText:@"" selectedRange:NSMakeRange(0, 0) replacementRange:none];
      g_composition = nil;
      return true;
    case 49:  // space selects the whole conversion, as a real one does
      g_composition = @"日本語";
      [client setMarkedText:g_composition
              selectedRange:NSMakeRange(0, g_composition.length)
           replacementRange:none];
      return true;
  }
  if (!letter) {
    return true;
  }
  g_composition = [g_composition ?: @"" stringByAppendingString:characters];
  [client setMarkedText:g_composition
          selectedRange:NSMakeRange(g_composition.length, 0)
       replacementRange:none];
  return true;
}

BOOL HandleEvent(NSTextInputContext* context, SEL selector, NSEvent* event) {
  if ([(id)context.client isKindOfClass:NSClassFromString(@"GPUIView")] &&
      ImeKey(context.client, event)) {
    return YES;
  }
  return ((BOOL (*)(id, SEL, NSEvent*))g_handle_event)(context, selector, event);
}

// WebKit's page view, which the core loads only when a tab first shows a page in Safari's engine,
// so it's looked up as each key comes.
void HandleEventByInputMethod(NSTextInputContext* context,
                              SEL selector,
                              NSEvent* event,
                              void (^handled)(BOOL)) {
  if ([(id)context.client isKindOfClass:NSClassFromString(@"WKWebView")] &&
      ImeKey(context.client, event)) {
    handled(YES);
    return;
  }
  ((void (*)(id, SEL, NSEvent*, void (^)(BOOL)))g_handle_event_by_input_method)(context, selector,
                                                                                event, handled);
}

void InterpretKeyEvents(NSView* view, SEL selector, NSArray<NSEvent*>* events) {
  if (events.count == 1 && ImeKey((id<NSTextInputClient>)view, events[0])) {
    return;
  }
  ((void (*)(id, SEL, NSArray*))g_interpret_key_events)(view, selector, events);
}

void HookIme() {
  if (g_handle_event) {
    return;
  }
  g_handle_event =
      method_setImplementation(class_getInstanceMethod(NSTextInputContext.class,
                                                       @selector(handleEvent:)),
                               (IMP)HandleEvent);
  g_interpret_key_events = method_setImplementation(
      class_getInstanceMethod(NSClassFromString(@"RenderWidgetHostViewCocoa"),
                              @selector(interpretKeyEvents:)),
      (IMP)InterpretKeyEvents);
  if (Method method =
          class_getInstanceMethod(NSTextInputContext.class,
                                  NSSelectorFromString(@"handleEventByInputMethod:completionHandler:"))) {
    g_handle_event_by_input_method =
        method_setImplementation(method, (IMP)HandleEventByInputMethod);
  }
}

// The system's character palette would go on the owner's screen, as their input method would
// switch, so the driver stands in for it: the app logs that it asked for it, and insert commits
// what the palette would.
void OrderFrontCharacterPalette(id, SEL, id) {
  fprintf(stderr, "browser: character palette shown\n");
}

// The run's pasteboard, which stands in for the clipboard, so a page's or DevTools' copy and paste
// never touch the owner's. Chromium asks for the clipboard both ways.
NSPasteboard* g_pasteboard;
IMP g_pasteboard_with_name;

// The save panel the app began and its completion handler, which the driver answers as the user
// would pick in it. A save panel runs out of process, where its -ok: raises "not implemented" and
// its window can't be driven, so the driver keeps it off screen and ends it the way the panel
// itself does, through the handler, with the URL it says the user picked.
NSSavePanel* g_save_panel;
void (^g_save_panel_done)(NSModalResponse);
NSURL* g_save_panel_url;
IMP g_save_panel_class;
IMP g_begin_panel;
IMP g_panel_url;

void BeginPanel(NSSavePanel* self, SEL selector, void (^done)(NSModalResponse)) {
  // An open panel, such as the import's, is the user's to answer as before.
  if ([self isKindOfClass:NSOpenPanel.class]) {
    ((void (*)(id, SEL, void (^)(NSModalResponse)))g_begin_panel)(self, selector, done);
    return;
  }
  g_save_panel = self;
  g_save_panel_done = done;
  fprintf(stderr, "browser: save panel offers %s in %s\n",
          self.nameFieldStringValue.UTF8String ?: "", self.directoryURL.path.UTF8String ?: "");
}

NSURL* PanelURL(NSSavePanel* self, SEL selector) {
  if (self == g_save_panel && g_save_panel_url) {
    return g_save_panel_url;
  }
  return ((NSURL * (*)(id, SEL)) g_panel_url)(self, selector);
}

// The panel's class may be one AppKit picks for it, so it's hooked as the app makes its first.
NSSavePanel* SavePanel(id self, SEL selector) {
  NSSavePanel* panel = ((NSSavePanel * (*)(id, SEL)) g_save_panel_class)(self, selector);
  Class kind = object_getClass(panel);
  IMP begin = method_setImplementation(
      class_getInstanceMethod(kind, @selector(beginWithCompletionHandler:)), (IMP)BeginPanel);
  if (begin != (IMP)BeginPanel) {
    g_begin_panel = begin;
  }
  IMP url = method_setImplementation(class_getInstanceMethod(kind, @selector(URL)), (IMP)PanelURL);
  if (url != (IMP)PanelURL) {
    g_panel_url = url;
  }
  return panel;
}

// Logs the key window's title once it has |text|, for up to 10 s, or the app's windows if none
// does: a window Chrome opens, such as undocked DevTools, takes the keyboard a moment after the
// command that opens it, and its page's title after that.
void AwaitKeyTitle(NSString* text, int tries, void (^done)(void)) {
  NSString* title = NSApp.keyWindow.title ?: @"";
  if (text.length == 0 || [title containsString:text]) {
    fprintf(stderr, "browser: key window titled %s\n", title.UTF8String);
    done();
    return;
  }
  if (tries == 0) {
    NSMutableArray<NSString*>* shown = [NSMutableArray array];
    for (NSWindow* window in NSApp.windows) {
      if (window.visible) {
        [shown addObject:[NSString stringWithFormat:@"%@%@", window.title,
                                                    window.keyWindow ? @" (key)" : @""]];
      }
    }
    fprintf(stderr, "browser: no key window titled %s, windows %s\n", text.UTF8String,
            [shown componentsJoinedByString:@" | "].UTF8String);
    done();
    return;
  }
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 100 * NSEC_PER_MSEC), dispatch_get_main_queue(),
                 ^{
                   AwaitKeyTitle(text, tries - 1, done);
                 });
}

// Picks |path| in the save panel the app began, once it has, which may be a moment after the app
// asked for it: for up to 5 s.
void AnswerSavePanel(NSString* path, int tries, void (^done)(void)) {
  if (g_save_panel_done) {
    void (^answer)(NSModalResponse) = g_save_panel_done;
    g_save_panel_done = nil;
    g_save_panel_url = [NSURL fileURLWithPath:path isDirectory:NO];
    fprintf(stderr, "browser: save panel answered\n");
    // The app's handler reads the panel's URL as it runs.
    answer(NSModalResponseOK);
    g_save_panel = nil;
    g_save_panel_url = nil;
    done();
    return;
  }
  if (tries == 0) {
    fprintf(stderr, "browser: no save panel\n");
    done();
    return;
  }
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 100 * NSEC_PER_MSEC), dispatch_get_main_queue(),
                 ^{
                   AnswerSavePanel(path, tries - 1, done);
                 });
}

NSPasteboard* GeneralPasteboard(id, SEL) {
  return g_pasteboard;
}

NSPasteboard* PasteboardWithName(id self, SEL selector, NSPasteboardName name) {
  if ([name isEqualToString:NSPasteboardNameGeneral]) {
    return g_pasteboard;
  }
  return ((NSPasteboard * (*)(id, SEL, NSPasteboardName)) g_pasteboard_with_name)(self, selector,
                                                                                  name);
}

// Frames are in points from the content view's bottom-left corner. A plain NSView and the core's
// window view only hold others, so the views in them are listed instead.
void AppendViews(NSMutableString* line, NSView* content, NSView* parent) {
  for (NSView* view in parent.subviews) {
    // Views that only hold others: the core's, and the UI's that GPUI's is in.
    NSString* name = NSStringFromClass(view.class);
    if (view.class == NSView.class || [name isEqual:@"BrowserCoreWindowView"] ||
        [name isEqual:@"HostUIView"]) {
      AppendViews(line, content, view);
      continue;
    }
    NSRect frame = [parent convertRect:view.frame toView:content];
    [line appendFormat:@", %@ %.0f,%.0f %.0fx%.0f", NSStringFromClass(view.class),
                       NSMinX(frame), NSMinY(frame), NSWidth(frame), NSHeight(frame)];
    if (view.layer.masksToBounds && view.layer.cornerRadius > 0) {
      [line appendFormat:@" radius %.0f", view.layer.cornerRadius];
    }
  }
}

// A window as the front command names it.
NSString* Describe(id element) {
  if (!element) {
    return @"none";
  }
  for (size_t index = 0; index < 64; index++) {
    if ((__bridge void*)element == host_browser_window(index)) {
      return index == g_driven ? @"driven" : [NSString stringWithFormat:@"window %zu", index + 1];
    }
  }
  if ([element isKindOfClass:NSString.class]) {
    return element;
  }
  if (![element isKindOfClass:NSWindow.class]) {
    return NSStringFromClass([element class]);
  }
  NSWindow* window = element;
  return [NSString stringWithFormat:@"%@ %.0fx%.0f %@", NSStringFromClass(window.class),
                                    NSWidth(window.frame), NSHeight(window.frame),
                                    window.accessibilityRole];
}

// The app's first window on screen at the normal level, front to back, as another app finds it.
id FirstNormalWindow() {
  NSArray* windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
      kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
  for (NSDictionary* info in windows) {
    if ([info[(id)kCGWindowOwnerPID] intValue] == getpid() &&
        [info[(id)kCGWindowLayer] intValue] == 0) {
      NSInteger number = [info[(id)kCGWindowNumber] integerValue];
      return [NSApp windowWithWindowNumber:number]
                 ?: [NSString stringWithFormat:@"number %ld", (long)number];
    }
  }
  return nil;
}

void Layout(NSWindow* window) {
  NSView* content = window.contentView;
  NSMutableString* line = [NSMutableString
      stringWithFormat:@"browser: layout content %.0fx%.0f", NSWidth(content.bounds),
                       NSHeight(content.bounds)];
  AppendViews(line, content, content);
  // As the spec gives them: points from the window's top-left corner.
  [line appendString:@"\nbrowser: layout buttons"];
  for (NSWindowButton type :
       {NSWindowCloseButton, NSWindowMiniaturizeButton, NSWindowZoomButton}) {
    NSButton* button = [window standardWindowButton:type];
    if (button.hidden) {
      [line appendString:@" hidden"];
      continue;
    }
    NSRect frame = [button convertRect:button.bounds toView:nil];
    [line appendFormat:@" %.1f,%.1f", NSMidX(frame), NSHeight(window.frame) - NSMidY(frame)];
  }
  // Child windows, such as the find bar: from the content's top-left corner, and their size.
  NSRect bounds = [window contentRectForFrameRect:window.frame];
  for (NSWindow* child in window.childWindows) {
    // The command bar's shadow stays a child while the bar is hidden, at alpha 0, showing nothing.
    if (child.alphaValue == 0) {
      continue;
    }
    NSRect frame = child.frame;
    [line appendFormat:@"\nbrowser: layout child %.0f,%.0f %.0fx%.0f",
                       NSMinX(frame) - NSMinX(bounds), NSMaxY(bounds) - NSMaxY(frame),
                       NSWidth(frame), NSHeight(frame)];
  }
  // The page whose view has the keyboard, by its frame as the views' above, which tells a split
  // view's pages apart where the done line's class can't.
  NSResponder* responder = window.firstResponder;
  NSView* page = [responder isKindOfClass:NSView.class] ? (NSView*)responder : nil;
  while (page && ![NSStringFromClass(page.class) isEqual:@"WebContentsViewCocoa"]) {
    page = page.superview;
  }
  if (page) {
    NSRect frame = [page.superview convertRect:page.frame toView:content];
    [line appendFormat:@"\nbrowser: layout keyboard %.0f,%.0f %.0fx%.0f", NSMinX(frame),
                       NSMinY(frame), NSWidth(frame), NSHeight(frame)];
  } else {
    [line appendString:@"\nbrowser: layout keyboard none"];
  }
  [line appendFormat:@"\nbrowser: layout full screen %d",
                     (window.styleMask & NSWindowStyleMaskFullScreen) != 0];
  fprintf(stderr, "%s\n", line.UTF8String);
}

// Chooses the item titled |title| in |menu| or its submenus, such as View › Developer, or with
// "Submenu › Item", the item in the submenu so titled, where the same title is elsewhere too.
bool ChooseMenuItem(NSMenu* menu, NSString* title) {
  NSRange split = [title rangeOfString:@" › "];
  if (split.location != NSNotFound) {
    NSString* parent = [title substringToIndex:split.location];
    NSString* item_title = [title substringFromIndex:NSMaxRange(split)];
    for (NSMenuItem* item in menu.itemArray) {
      if (!item.submenu) {
        continue;
      }
      if ([item.title isEqualToString:parent] ? ChooseMenuItem(item.submenu, item_title)
                                               : ChooseMenuItem(item.submenu, title)) {
        return true;
      }
    }
    return false;
  }
  NSInteger index = [menu indexOfItemWithTitle:title];
  if (index >= 0) {
    [menu performActionForItemAtIndex:index];
    return true;
  }
  for (NSMenuItem* item in menu.itemArray) {
    if (item.submenu && ChooseMenuItem(item.submenu, title)) {
      return true;
    }
  }
  return false;
}

// Chooses the enabled item titled |title| in the menu AppKit tracks, and closes the menu.
bool ChooseTracked(NSString* title) {
  NSMenu* menu = g_tracked;
  NSInteger index = [menu indexOfItemWithTitle:title];
  if (index < 0 || ![menu itemAtIndex:index].enabled) {
    return false;
  }
  [menu performActionForItemAtIndex:index];
  [menu cancelTracking];
  return true;
}

// The text fields' and buttons' views in |view|, in the order AppKit lays them out.
void ModalTexts(NSView* view, NSMutableArray<NSView*>* texts) {
  if ([view isKindOfClass:NSTextField.class] || [view isKindOfClass:NSButton.class]) {
    [texts addObject:view];
  }
  for (NSView* subview in view.subviews) {
    ModalTexts(subview, texts);
  }
}

// Presses the button titled |title| in the window AppKit runs modally, such as Chrome's own alerts.
bool ChooseModal(NSString* title) {
  NSMutableArray<NSView*>* texts = [NSMutableArray array];
  ModalTexts(NSApp.modalWindow.contentView, texts);
  for (NSView* view in texts) {
    if ([view isKindOfClass:NSButton.class] && [((NSButton*)view).title isEqual:title]) {
      [(NSButton*)view performClick:nil];
      return true;
    }
  }
  return false;
}

// Calls |done| once the command's events are posted. Returns false, without calling it, for a
// command it can't do.
bool Run(const std::string& line, void (^done)(void)) {
  NSWindow* window = MainWindow();
  std::istringstream words(line);
  std::string action;
  words >> action;
  if (action == "window") {
    size_t number;
    if (!(words >> number) || number == 0 || !host_browser_window(number - 1)) {
      return false;
    }
    g_driven = number - 1;
    fprintf(stderr, "browser: drive window %zu\n", number);
    [(__bridge NSWindow*)host_browser_window(g_driven) makeKeyAndOrderFront:nil];
    done();
    return true;
  }
  // Before the window's lookup, which finds no window out of sight.
  if (action == "minimize") {
    NSWindow* driven = (__bridge NSWindow*)host_browser_window(g_driven);
    if (driven.miniaturized) {
      [driven deminiaturize:nil];
    } else {
      [driven miniaturize:nil];
    }
    done();
    return true;
  }
  if (action == "hide") {
    if (NSApp.hidden) {
      [NSApp unhide:nil];
    } else {
      [NSApp hide:nil];
    }
    done();
    return true;
  }
  if (action == "display") {
    std::string what;
    unsigned width, height;
    if (!(words >> what)) {
      return false;
    }
    if (what == "add" && !g_display && words >> width >> height) {
      std::string main;
      words >> main;
      AddDisplay(width, height, main == "main", done);
      return true;
    }
    if (what == "remove" && g_display) {
      const unsigned id = g_display.displayID;
      g_display = nil;
      AwaitScreens(
          ^{
            return ScreenOf(id) == nil;
          },
          ^(bool removed) {
            fprintf(stderr, "browser: display %s\n", removed ? "removed" : "not removed");
            done();
          });
      return true;
    }
    return false;
  }
  if (!window) {
    return false;
  }
  if (action == "click") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    NSWindow* target = WindowAt(window, x, y);
    // A click that another window of the app's takes, such as one AppKit puts over a full-screen
    // window, says which, since a check's click that does nothing logs nothing else (#1090).
    if (target != window) {
      const NSRect frame = target.frame;
      fprintf(stderr, "browser: drive click went to %s %ld at %.0f,%.0f %.0fx%.0f\n",
              NSStringFromClass([target class]).UTF8String, (long)target.windowNumber,
              frame.origin.x, frame.origin.y, frame.size.width, frame.size.height);
    }
    Convert(window, target, &x, &y);
    Click(target, x, y, 1, Modifiers(words));
    done();
    return true;
  }
  if (action == "clickkey") {
    double x, y;
    if (!NSApp.keyWindow || !(words >> x >> y)) {
      return false;
    }
    Click(NSApp.keyWindow, x, y, 1);
    done();
    return true;
  }
  if (action == "doubleclickkey") {
    double x, y;
    if (!NSApp.keyWindow || !(words >> x >> y)) {
      return false;
    }
    Click(NSApp.keyWindow, x, y, 1);
    Click(NSApp.keyWindow, x, y, 2);
    done();
    return true;
  }
  if (action == "rightclickkey") {
    double x, y;
    if (!NSApp.keyWindow || !(words >> x >> y)) {
      return false;
    }
    PostMouse(NSApp.keyWindow, NSEventTypeRightMouseDown, x, y, 1);
    PostMouse(NSApp.keyWindow, NSEventTypeRightMouseUp, x, y, 1);
    done();
    return true;
  }
  if (action == "rightclick") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    NSWindow* target = WindowAt(window, x, y);
    Convert(window, target, &x, &y);
    PostMouse(target, NSEventTypeRightMouseDown, x, y, 1);
    PostMouse(target, NSEventTypeRightMouseUp, x, y, 1);
    done();
    return true;
  }
  if (action == "middleclick") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    NSWindow* target = WindowAt(window, x, y);
    Convert(window, target, &x, &y);
    PostMouse(target, NSEventTypeOtherMouseDown, x, y, 1);
    PostMouse(target, NSEventTypeOtherMouseUp, x, y, 1);
    done();
    return true;
  }
  if (action == "doubleclick") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    NSWindow* target = WindowAt(window, x, y);
    Convert(window, target, &x, &y);
    Click(target, x, y, 1);
    Click(target, x, y, 2);
    done();
    return true;
  }
  if (action == "dragkey") {
    double x, y, x2, y2;
    NSWindow* key = NSApp.keyWindow;
    if (!key || !(words >> x >> y >> x2 >> y2)) {
      return false;
    }
    // Halfway first, so the view sees the drag start before it ends.
    PostMouse(key, NSEventTypeLeftMouseDown, x, y, 1);
    PostMouse(key, NSEventTypeLeftMouseDragged, (x + x2) / 2, (y + y2) / 2, 1);
    PostMouse(key, NSEventTypeLeftMouseDragged, x2, y2, 1);
    PostMouse(key, NSEventTypeLeftMouseUp, x2, y2, 1);
    done();
    return true;
  }
  if (action == "drag") {
    double x, y, x2, y2;
    if (!(words >> x >> y >> x2 >> y2)) {
      return false;
    }
    NSWindow* target = WindowAt(window, x, y);
    Convert(window, target, &x, &y);
    Convert(window, target, &x2, &y2);
    PostMouse(target, NSEventTypeLeftMouseDown, x, y, 1);
    PostMouse(target, NSEventTypeLeftMouseDragged, x2, y2, 1);
    PostMouse(target, NSEventTypeLeftMouseUp, x2, y2, 1);
    done();
    return true;
  }
  // A drag in steps goes to the window the press went to, as AppKit sends it.
  static NSWindow* pressed;
  if (action == "rightpress" || action == "rightdragto" || action == "rightrelease") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    if (action == "rightpress") {
      pressed = WindowAt(window, x, y);
    }
    NSWindow* target = pressed ?: window;
    Convert(window, target, &x, &y);
    NSEventType type = action == "rightpress"    ? NSEventTypeRightMouseDown
                       : action == "rightdragto" ? NSEventTypeRightMouseDragged
                                                 : NSEventTypeRightMouseUp;
    PostMouse(target, type, x, y, 1);
    if (action == "rightrelease") {
      pressed = nil;
    }
    done();
    return true;
  }
  if (action == "press" || action == "dragto" || action == "release") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    if (action == "press") {
      pressed = WindowAt(window, x, y);
    }
    NSWindow* target = pressed ?: window;
    Convert(window, target, &x, &y);
    NSEventType type = action == "press"    ? NSEventTypeLeftMouseDown
                       : action == "dragto" ? NSEventTypeLeftMouseDragged
                                            : NSEventTypeLeftMouseUp;
    PostMouse(target, type, x, y, 1, Modifiers(words));
    if (action == "release") {
      pressed = nil;
    }
    done();
    return true;
  }
  if (action == "move") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    NSWindow* target = WindowAt(window, x, y);
    Convert(window, target, &x, &y);
    NSEvent* move = [NSEvent mouseEventWithType:NSEventTypeMouseMoved
                                       location:NSMakePoint(x, NSHeight(target.frame) - y)
                                  modifierFlags:0
                                      timestamp:NSProcessInfo.processInfo.systemUptime
                                   windowNumber:target.windowNumber
                                        context:nil
                                    eventNumber:0
                                     clickCount:0
                                       pressure:0];
    // It goes to the view under the pointer and to the tracking areas holding it, as a real move
    // does, which is how GPUI's view, under the page's, hears the pointer over the page. It's also
    // posted, for the app's event monitors.
    NSView* content = target.contentView;
    NSView* hit = [content hitTest:[content.superview convertPoint:move.locationInWindow
                                                          fromView:nil]];
    [hit mouseMoved:move];
    TrackMove(content, move, hit);
    [NSApp postEvent:move atStart:NO];
    done();
    return true;
  }
  if (action == "scroll") {
    double x, y, dy;
    int count;
    if (!(words >> x >> y >> dy >> count) || count < 1) {
      return false;
    }
    Scroll(window, x, y, dy, count, done);
    return true;
  }
  if (action == "swipe") {
    double x, y, dx, dy;
    int count, rest = 0;
    if (!(words >> x >> y >> dx >> dy >> count) || count < 1) {
      return false;
    }
    words >> rest;
    Swipe(window, x, y, dx, dy, count, rest, done);
    return true;
  }
  if (action == "pageswipe") {
    double x, y;
    std::string way;
    if (!(words >> x >> y >> way) || (way != "back" && way != "forward")) {
      return false;
    }
    SwipeBetweenPages(window, NSMakePoint(x, NSHeight(window.frame) - y),
                      way == "back");
    done();
    return true;
  }
  if (action == "dragover" || action == "dragdrop") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    bool taken = DragOver(window, x, y, words);
    if (!g_drag) {
      return false;
    }
    if (action == "dragdrop") {
      id<NSDraggingDestination> view = (id<NSDraggingDestination>)g_drag_view;
      bool dropped = taken && [view respondsToSelector:@selector(performDragOperation:)] &&
                     [view performDragOperation:(id<NSDraggingInfo>)g_drag];
      fprintf(stderr, "browser: drag %s\n", dropped ? "dropped" : "refused");
      EndDrag();
    }
    done();
    return true;
  }
  if (action == "dragleave") {
    id<NSDraggingDestination> view = (id<NSDraggingDestination>)g_drag_view;
    if ([view respondsToSelector:@selector(draggingExited:)]) {
      [view draggingExited:(id<NSDraggingInfo>)g_drag];
    }
    EndDrag();
    done();
    return true;
  }
  if (action == "wheel") {
    double x, y, dx;
    if (!(words >> x >> y >> dx)) {
      return false;
    }
    CGEventRef event =
        CGEventCreateScrollWheelEvent2(nullptr, kCGScrollEventUnitLine, 2, 0, (int32_t)dx, 0);
    CGEventSetIntegerValueField(event, kWindowNumberField, window.windowNumber);
    CGEventSetWindowLocation(event, CGPointMake(x, y));
    [NSApp postEvent:[NSEvent eventWithCGEvent:event] atStart:NO];
    CFRelease(event);
    done();
    return true;
  }
  if (action == "resize") {
    double width, height;
    if (!(words >> width >> height)) {
      return false;
    }
    NSRect frame = window.frame;
    [window setFrame:NSMakeRect(NSMinX(frame), NSMaxY(frame) - height, width, height) display:YES];
    done();
    return true;
  }
  if (action == "fullscreen") {
    [window toggleFullScreen:nil];
    done();
    return true;
  }
  if (action == "place") {
    double x, y, width, height;
    if (!(words >> x >> y >> width >> height)) {
      return false;
    }
    const CGFloat top = NSHeight(NSScreen.screens.firstObject.frame);
    [window setFrame:NSMakeRect(x, top - y - height, width, height) display:YES];
    done();
    return true;
  }
  if (action == "screen") {
    double visible[4];
    if (host_screens(visible, 1) < 1) {
      return false;
    }
    // host_screens gives the screen's visible area, from the main display's top-left; the screen
    // holding its middle is the one.
    const CGFloat top = NSHeight(NSScreen.screens.firstObject.frame);
    const NSPoint middle =
        NSMakePoint(visible[0] + visible[2] / 2, top - visible[1] - visible[3] / 2);
    for (NSScreen* screen in NSScreen.screens) {
      const NSRect frame = screen.frame;
      if (NSPointInRect(middle, frame)) {
        fprintf(stderr, "browser: screen at %.0f %.0f %.0f %.0f\n", NSMinX(frame),
                top - NSMaxY(frame), NSWidth(frame), NSHeight(frame));
        done();
        return true;
      }
    }
    return false;
  }
  if (action == "order") {
    NSMutableString* order = [NSMutableString string];
    const CGFloat top = NSHeight(NSScreen.screens.firstObject.frame);
    // Each by the order it opened in, the driven one too.
    const auto name_of = [](NSWindow* each) -> NSString* {
      for (size_t index = 0; index < 64; index++) {
        if ((__bridge void*)each == host_browser_window(index)) {
          return [NSString stringWithFormat:@"window %zu", index + 1];
        }
      }
      return nil;
    };
    for (NSWindow* each in NSApp.orderedWindows) {
      NSString* name = each.visible ? name_of(each) : nil;
      if (!name) {
        continue;
      }
      [order appendFormat:@" %@", name];
      const NSRect frame = each.frame;
      fprintf(stderr, "browser: frame %s %.0f %.0f %.0f %.0f\n", name.UTF8String, NSMinX(frame),
              top - NSMaxY(frame), NSWidth(frame), NSHeight(frame));
    }
    fprintf(stderr, "browser: order%s, key %s\n", order.UTF8String,
            (name_of(NSApp.keyWindow) ?: @"none").UTF8String);
    done();
    return true;
  }
  if (action == "layout") {
    Layout(window);
    done();
    return true;
  }
  if (action == "state") {
    done();
    return true;
  }
  if (action == "agentspace") {
    std::string agent, name;
    if (!(words >> agent) || !std::getline(words >> std::ws, name) || name.empty()) {
      return false;
    }
    const bc_id space = bc_space_create_for_agent(name.c_str(), agent.c_str(), name.c_str());
    fprintf(stderr, "browser: made space %llu for agent %s\n", (unsigned long long)space,
            agent.c_str());
    done();
    return true;
  }
  if (action == "agentmayact") {
    std::string agent;
    bc_id tab;
    if (!(words >> agent >> tab)) {
      return false;
    }
    std::string what;
    if (!(words >> what)) {
      what = "navigate";
    }
    char reason[256];
    const bool may = bc_agent_may_act(agent.c_str(), tab, what.c_str(), reason, sizeof reason);
    fprintf(stderr, "browser: agent %s may act on %llu: %d\n", agent.c_str(),
            (unsigned long long)tab, may);
    if (!may) {
      fprintf(stderr, "browser: agent %s refused %llu: %s\n", agent.c_str(),
              (unsigned long long)tab, reason);
    }
    done();
    return true;
  }
  if (action == "agentrelease") {
    std::string agent;
    bc_id tab;
    if (!(words >> agent >> tab)) {
      return false;
    }
    const bool released = bc_agent_release_tab(agent.c_str(), tab);
    fprintf(stderr, "browser: agent %s released %llu: %d\n", agent.c_str(),
            (unsigned long long)tab, released);
    done();
    return true;
  }
  if (action == "agentdisconnect") {
    std::string agent;
    if (!(words >> agent)) {
      return false;
    }
    const size_t ended = bc_agent_disconnected(agent.c_str());
    fprintf(stderr, "browser: agent %s disconnected, %zu sessions ended\n", agent.c_str(), ended);
    done();
    return true;
  }
  if (action == "takeback") {
    bc_id tab;
    if (!(words >> tab)) {
      return false;
    }
    const bool took = bc_tab_take_back(tab);
    fprintf(stderr, "browser: took back %llu: %d\n", (unsigned long long)tab, took);
    done();
    return true;
  }
  if (action == "age") {
    double seconds;
    if (!(words >> seconds)) {
      return false;
    }
    bc_age_tabs_for_testing(seconds);
    done();
    return true;
  }
  if (action == "front") {
    fprintf(stderr, "browser: front %s, main %s, key %s, ax focused %s, ax main %s\n",
            Describe(FirstNormalWindow()).UTF8String, Describe(NSApp.mainWindow).UTF8String,
            Describe(NSApp.keyWindow).UTF8String,
            Describe(NSApp.accessibilityFocusedWindow).UTF8String,
            Describe(NSApp.accessibilityMainWindow).UTF8String);
    done();
    return true;
  }
  if (action == "title") {
    AwaitKeyTitle(@(line.substr(std::min(line.size(), action.size() + 1)).c_str()), 100, done);
    return true;
  }
  if (action == "savepanel") {
    AnswerSavePanel(@(line.substr(std::min(line.size(), action.size() + 1)).c_str()), 50, done);
    return true;
  }
  if (action == "pasteboard") {
    fprintf(stderr, "browser: pasteboard text %s\n",
            [g_pasteboard stringForType:NSPasteboardTypeString].UTF8String ?: "");
    done();
    return true;
  }
  if (action == "backdrop") {
    std::string grey;
    if (!(words >> grey)) {
      return false;
    }
    if (grey == "off") {
      [g_backdrop orderOut:nil];
    } else {
      if (!g_backdrop) {
        g_backdrop = [[NSWindow alloc] initWithContentRect:window.frame
                                                 styleMask:NSWindowStyleMaskBorderless
                                                   backing:NSBackingStoreBuffered
                                                     defer:NO];
        g_backdrop.releasedWhenClosed = NO;
        g_backdrop.hasShadow = NO;
        g_backdrop.contentView = [[DriveBackdropView alloc] initWithFrame:NSZeroRect];
      }
      DriveBackdropView* view = (DriveBackdropView*)g_backdrop.contentView;
      view.level = std::stoi(grey);
      // In points from the top left of the browser window's screen, which a capture of that
      // screen is read by.
      NSRect screen = window.screen.frame;
      view.place = [NSString
          stringWithFormat:@"window at %.0f,%.0f %.0fx%.0f on %.0fx%.0f",
                           NSMinX(window.frame) - NSMinX(screen),
                           NSMaxY(screen) - NSMaxY(window.frame), NSWidth(window.frame),
                           NSHeight(window.frame), NSWidth(screen), NSHeight(screen)];
      view.needsDisplay = YES;
      // Past the edges by more than the material's blur, so it blurs nothing else.
      [g_backdrop setFrame:NSInsetRect(window.frame, -100, -100) display:YES];
      [g_backdrop orderWindow:NSWindowBelow relativeTo:window.windowNumber];
    }
    done();
    return true;
  }
  if (action == "menu") {
    NSString* title = @(line.substr(std::min(line.size(), action.size() + 1)).c_str());
    for (NSMenuItem* top in NSApp.mainMenu.itemArray) {
      if (top.submenu && ChooseMenuItem(top.submenu, title)) {
        done();
        return true;
      }
    }
    return false;
  }
  if (action == "openmenu") {
    NSString* title = @(line.substr(std::min(line.size(), action.size() + 1)).c_str());
    NSMenu* menu = [NSApp.mainMenu itemWithTitle:title].submenu;
    if (!menu) {
      return false;
    }
    // Tracking it doesn't return until it closes, so it opens after this command is done.
    NSView* view = window.contentView;
    dispatch_async(dispatch_get_main_queue(), ^{
      [menu popUpMenuPositioningItem:nil
                          atLocation:NSMakePoint(40, NSHeight(view.bounds) - 40)
                              inView:view];
    });
    done();
    return true;
  }
  if (action == "dock") {
    NSString* title = @(line.substr(std::min(line.size(), action.size() + 1)).c_str());
    id<NSApplicationDelegate> delegate = NSApp.delegate;
    NSMenu* menu = [delegate respondsToSelector:@selector(applicationDockMenu:)]
                       ? [delegate applicationDockMenu:NSApp]
                       : nil;
    if (!menu) {
      return false;
    }
    if (title.length) {
      NSInteger index = [menu indexOfItemWithTitle:title];
      if (index < 0) {
        return false;
      }
      [menu performActionForItemAtIndex:index];
    } else {
      NSMutableArray<NSString*>* titles = [NSMutableArray array];
      for (NSMenuItem* item in menu.itemArray) {
        if (!item.separatorItem && !item.hidden) {
          [titles addObject:item.title];
        }
      }
      fprintf(stderr, "browser: dock menu %s\n", [titles componentsJoinedByString:@", "].UTF8String);
    }
    done();
    return true;
  }
  if (action == "menubar") {
    NSString* title = @(line.substr(std::min(line.size(), action.size() + 1)).c_str());
    NSMenu* menu = [NSApp.mainMenu itemWithTitle:title].submenu;
    if (!menu) {
      return false;
    }
    NSMutableArray<NSString*>* titles = [NSMutableArray array];
    auto add = ^(NSMenuItem* item, NSString* name) {
      if (item.state == NSControlStateValueOn) {
        name = [name stringByAppendingString:@" (checked)"];
      }
      [titles addObject:item.enabled ? name : [name stringByAppendingString:@" (disabled)"]];
    };
    // Enabled as the menu would open now, which validates its items.
    [menu update];
    for (NSMenuItem* item in menu.itemArray) {
      if (item.separatorItem || item.hidden) {
        continue;
      }
      if (!item.submenu) {
        add(item, item.title);
        continue;
      }
      [item.submenu update];
      for (NSMenuItem* inner in item.submenu.itemArray) {
        if (!inner.separatorItem && !inner.hidden) {
          add(inner, [NSString stringWithFormat:@"%@ › %@", item.title, inner.title]);
        }
      }
    }
    fprintf(stderr, "browser: menu bar %s: %s\n", title.UTF8String,
            [titles componentsJoinedByString:@", "].UTF8String);
    done();
    return true;
  }
  if (action == "choose") {
    std::string title = line.substr(std::min(line.size(), action.size() + 1));
    if (!host_menu_window_choose(title.c_str()) && !host_choose(title.c_str()) &&
        !ChooseTracked(@(title.c_str())) && !ChooseModal(@(title.c_str()))) {
      return false;
    }
    // Once the menu or alert has closed, which GCD's main queue waits for.
    dispatch_async(dispatch_get_main_queue(), done);
    return true;
  }
  if (action == "dismiss") {
    if (!host_menu_window_dismiss()) {
      NSMenu* menu = g_tracked;
      if (!menu) {
        return false;
      }
      [menu cancelTracking];
    }
    dispatch_async(dispatch_get_main_queue(), done);
    return true;
  }
  if (action == "ime") {
    std::string state;
    words >> state;
    if (state != "on" && state != "off") {
      return false;
    }
    HookIme();
    g_ime_on = state == "on";
    g_composition = nil;
    done();
    return true;
  }
  NSWindow* key_window = NSApp.keyWindow ?: window;
  if (action == "keydown" || action == "keyup") {
    std::string key, repeat;
    words >> key >> repeat;
    const NSEventType type = action == "keydown" ? NSEventTypeKeyDown : NSEventTypeKeyUp;
    if (!Key(key_window, key, 0, {type}, repeat == "repeat")) {
      return false;
    }
    done();
    return true;
  }
  if (action == "key" || action == "flags") {
    std::string key;
    if (action == "key") {
      words >> key;
    }
    NSEventModifierFlags flags = Modifiers(words);
    if (action == "flags") {
      // As the left ⌃ key would report it.
      [NSApp postEvent:[NSEvent keyEventWithType:NSEventTypeFlagsChanged
                                        location:NSZeroPoint
                                   modifierFlags:flags
                                       timestamp:NSProcessInfo.processInfo.systemUptime
                                    windowNumber:key_window.windowNumber
                                         context:nil
                                      characters:@""
                     charactersIgnoringModifiers:@""
                                       isARepeat:NO
                                         keyCode:59]
               atStart:NO];
    } else if (!Key(key_window, key, flags)) {
      return false;
    }
    done();
    return true;
  }
  if (action == "type") {
    std::string text = line.substr(std::min(line.size(), action.size() + 1));
    for (char character : text) {
      // A capital is its letter with shift, as the keyboard types it.
      const bool capital = character >= 'A' && character <= 'Z';
      const std::string key(1, capital ? character - 'A' + 'a' : character);
      if (!Key(key_window, key, capital ? NSEventModifierFlagShift : 0)) {
        return false;
      }
    }
    done();
    return true;
  }
  if (action == "insert") {
    std::string text = line.substr(std::min(line.size(), action.size() + 1));
    id responder = key_window.firstResponder;
    if (![responder conformsToProtocol:@protocol(NSTextInputClient)]) {
      return false;
    }
    [(id<NSTextInputClient>)responder insertText:@(text.c_str())
                                replacementRange:NSMakeRange(NSNotFound, 0)];
    done();
    return true;
  }
  return false;
}

void PostDone(long number) {
  [NSApp postEvent:[NSEvent otherEventWithType:NSEventTypeApplicationDefined
                                      location:NSZeroPoint
                                 modifierFlags:0
                                     timestamp:0
                                  windowNumber:0
                                       context:nil
                                       subtype:kDoneSubtype
                                         data1:number
                                         data2:0]
           atStart:NO];
}

}  // namespace

void drive_start(void) {
  NSEvent* (^report_done)(NSEvent*) = ^NSEvent*(NSEvent* event) {
    if (event.subtype != kDoneSubtype) {
      return event;
    }
    NSWindow* key = NSApp.keyWindow;
    NSResponder* focus = (key ?: MainWindow()).firstResponder;
    NSString* marked = @"none";
    if ([focus conformsToProtocol:@protocol(NSTextInputClient)]) {
      NSRange range = [(id<NSTextInputClient>)focus markedRange];
      if (range.location != NSNotFound) {
        marked = [NSString stringWithFormat:@"%lu,%lu", range.location, range.length];
      }
    }
    fprintf(stderr, "browser: drive %ld done, key %s, focus %s, active %d, marked %s\n",
            (long)event.data1, key ? NSStringFromClass(key.class).UTF8String : "none",
            NSStringFromClass(focus.class).UTF8String, NSApp.active, marked.UTF8String);
    return nil;
  };
  [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskApplicationDefined handler:report_done];
  method_setImplementation(class_getInstanceMethod(NSApplication.class,
                                                   @selector(orderFrontCharacterPalette:)),
                           (IMP)OrderFrontCharacterPalette);
  g_pasteboard = [NSPasteboard pasteboardWithUniqueName];
  method_setImplementation(class_getClassMethod(NSPasteboard.class, @selector(generalPasteboard)),
                           (IMP)GeneralPasteboard);
  g_pasteboard_with_name = method_setImplementation(
      class_getClassMethod(NSPasteboard.class, @selector(pasteboardWithName:)),
      (IMP)PasteboardWithName);
  g_save_panel_class = method_setImplementation(
      class_getClassMethod(NSSavePanel.class, @selector(savePanel)), (IMP)SavePanel);
  // A window leaving full screen tells AppKit it has before the desktop's Space slides back, and
  // until it has, the window server still orders the window over the others and drops clicks.
  [NSWorkspace.sharedWorkspace.notificationCenter
      addObserverForName:NSWorkspaceActiveSpaceDidChangeNotification
                  object:nil
                   queue:nil
              usingBlock:^(NSNotification*) {
                fprintf(stderr, "browser: desktop space changed\n");
              }];
  // Full-screen transitions animate, and end well after the command's done.
  for (NSNotificationName name :
       {NSWindowDidEnterFullScreenNotification, NSWindowDidExitFullScreenNotification}) {
    bool entered = [name isEqualToString:NSWindowDidEnterFullScreenNotification];
    [NSNotificationCenter.defaultCenter addObserverForName:name
                                                    object:nil
                                                     queue:nil
                                                usingBlock:^(NSNotification*) {
                                                  fprintf(stderr, "browser: window %s full screen\n",
                                                          entered ? "entered" : "left");
                                                  LogFullScreenStep(entered, "did", nil);
                                                  g_full_screen_window = nil;
                                                }];
  }
  // A display going away moves its windows to another, and says nothing else of it.
  [NSNotificationCenter.defaultCenter
      addObserverForName:NSWindowDidChangeScreenNotification
                  object:nil
                   queue:nil
              usingBlock:^(NSNotification* notification) {
                NSWindow* window = notification.object;
                if (window != (__bridge NSWindow*)host_browser_window(g_driven)) {
                  return;
                }
                const NSSize size = window.screen.frame.size;
                fprintf(stderr, "browser: window on screen %.0fx%.0f, full screen %d\n", size.width,
                        size.height, (window.styleMask & NSWindowStyleMaskFullScreen) != 0);
              }];
  // The same transitions timed, on the uptime clock the core's and the app's lines read: when
  // AppKit starts one, when the window gets its size, and when it ends (above). check.sh reads them.
  for (NSNotificationName name :
       {NSWindowWillEnterFullScreenNotification, NSWindowWillExitFullScreenNotification}) {
    bool entering = [name isEqualToString:NSWindowWillEnterFullScreenNotification];
    [NSNotificationCenter.defaultCenter addObserverForName:name
                                                    object:nil
                                                     queue:nil
                                                usingBlock:^(NSNotification* notification) {
                                                  g_full_screen_window = notification.object;
                                                  LogFullScreenStep(entering, "will", nil);
                                                }];
  }
  [NSNotificationCenter.defaultCenter
      addObserverForName:NSWindowDidResizeNotification
                  object:nil
                   queue:nil
              usingBlock:^(NSNotification* notification) {
                if (notification.object == g_full_screen_window) {
                  NSWindow* window = notification.object;
                  LogFullScreenStep(g_full_screen_entering, "resized", window);
                }
              }];
  // A menu AppKit starts tracking, such as a page's context menu, which Chrome shows: its items,
  // for check.sh to wait for and read, as the app's own menus log theirs.
  [NSNotificationCenter.defaultCenter
      addObserverForName:NSMenuDidBeginTrackingNotification
                  object:nil
                   queue:nil
              usingBlock:^(NSNotification* notification) {
                NSMenu* menu = notification.object;
                if (menu == NSApp.mainMenu) {
                  return;
                }
                g_tracked = menu;
                NSMutableArray<NSString*>* titles = [NSMutableArray array];
                for (NSMenuItem* item in menu.itemArray) {
                  if (!item.separatorItem && !item.hidden) {
                    [titles addObject:item.title];
                  }
                }
                fprintf(stderr, "browser: menu tracked %s\n",
                        [titles componentsJoinedByString:@", "].UTF8String);
              }];
  // An alert AppKit runs modally, such as Chrome's own: its texts and buttons, for check.sh to
  // wait for and read.
  [NSNotificationCenter.defaultCenter
      addObserverForName:NSWindowDidBecomeKeyNotification
                  object:nil
                   queue:nil
              usingBlock:^(NSNotification* notification) {
                NSWindow* window = notification.object;
                if (window != NSApp.modalWindow) {
                  return;
                }
                NSMutableArray<NSView*>* views = [NSMutableArray array];
                ModalTexts(window.contentView, views);
                NSMutableArray<NSString*>* texts = [NSMutableArray array];
                for (NSView* view in views) {
                  NSString* text = [view isKindOfClass:NSButton.class]
                                       ? ((NSButton*)view).title
                                       : ((NSTextField*)view).stringValue;
                  if (text.length && !view.hidden) {
                    [texts addObject:text];
                  }
                }
                fprintf(stderr, "browser: modal shown %s\n",
                        [texts componentsJoinedByString:@", "].UTF8String);
              }];
  // So does a zoom, and the window is zoomed only at its last frame.
  __block BOOL zoomed = NO;
  [NSNotificationCenter.defaultCenter addObserverForName:NSWindowDidResizeNotification
                                                  object:nil
                                                   queue:nil
                                              usingBlock:^(NSNotification* notification) {
                                                NSWindow* window = notification.object;
                                                if (window != MainWindow() ||
                                                    window.zoomed == zoomed) {
                                                  return;
                                                }
                                                zoomed = window.zoomed;
                                                fprintf(stderr, "browser: window zoomed %d\n",
                                                        zoomed);
                                              }];
  // Reads fd 0 with read(2), not stdio: fgets would hold stdin's FILE lock while it waits, and
  // NSXMLNode's XPath lexer, which Sparkle's appcast parse runs on the main thread, takes it too.
  std::thread([] {
    long number = 0;
    auto command = [&number](std::string line) {
      line.erase(line.find_last_not_of("\r\n") + 1);
      long n = ++number;
      // In the run loop's common modes, so a command also runs while a menu is open. GCD's main
      // queue waits until it closes.
      CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopCommonModes, ^{
        fprintf(stderr, "browser: drive %ld: %s\n", n, line.c_str());
        if (!Run(line, ^{
              PostDone(n);
            })) {
          fprintf(stderr, "browser: drive %ld: can't do that\n", n);
          PostDone(n);
        }
      });
      CFRunLoopWakeUp(CFRunLoopGetMain());
    };
    std::string pending;
    char buffer[1024];
    for (;;) {
      ssize_t got = read(STDIN_FILENO, buffer, sizeof buffer);
      if (got < 0 && errno == EINTR) {
        continue;
      }
      if (got <= 0) {
        break;
      }
      pending.append(buffer, got);
      for (size_t end; (end = pending.find('\n')) != std::string::npos;) {
        command(pending.substr(0, end));
        pending.erase(0, end + 1);
      }
    }
    // A last line without its newline still runs, as fgets returned it.
    if (!pending.empty()) {
      command(pending);
    }
  }).detach();
}
