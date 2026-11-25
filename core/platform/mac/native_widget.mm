#include "browser_core/platform/window_views.h"

#import <AppKit/AppKit.h>

#include <cstdio>
#include <utility>

#include "base/functional/callback.h"
#import "components/remote_cocoa/app_shim/native_widget_mac_nswindow.h"
#include "components/remote_cocoa/app_shim/native_widget_ns_window_bridge.h"
#include "components/remote_cocoa/common/native_widget_ns_window.mojom.h"
#include "ui/base/cocoa/window_size_constants.h"
#include "ui/views/widget/native_widget_mac.h"

namespace {

// The window's buttons sit in the app's first row, centred in the 44 points it's drawn in, the
// first 13 from the left edge as Arc's is, so their centres are 20, 43 and 66 across and 22 down
// (docs/ux-spec.md, Header and URL field).
constexpr CGFloat kButtonRowHeight = 44;
constexpr CGFloat kButtonInset = 13;

}  // namespace

// AppKit's private calls that place the window's buttons, which Chrome's browser window overrides
// too (components/remote_cocoa/app_shim/browser_native_widget_window_mac.mm).
@interface NSWindow (BrowserCorePrivate)
+ (Class)frameViewClassForStyleMask:(NSWindowStyleMask)windowStyle;
@end

@interface NSThemeFrame (BrowserCorePrivate)
- (CGFloat)_titlebarHeight;
- (void)setStyleMask:(NSUInteger)styleMask;
@end

// The window's frame view, which puts the buttons in the header row.
@interface BrowserCoreWindowFrame : NativeWidgetMacNSWindowTitledFrame
@end

@implementation BrowserCoreWindowFrame {
  BOOL _inFullScreen;
}

- (CGFloat)_titlebarHeight {
  // Full screen keeps AppKit's title bar, which comes down with the menu bar.
  return _inFullScreen ? [super _titlebarHeight] : kButtonRowHeight;
}

- (CGFloat)_minXTitlebarWidgetInset {
  return kButtonInset;
}

- (BOOL)_shouldCenterTrafficLights {
  return YES;
}

- (void)setStyleMask:(NSUInteger)styleMask {
  _inFullScreen = (styleMask & NSWindowStyleMaskFullScreen) != 0;
  [super setStyleMask:styleMask];
}

@end

@interface BrowserCoreNSWindow : NativeWidgetMacNSWindow
// Ends a way out of full screen AppKit took the window and didn't finish, as it says it has.
- (void)finishFullScreenExit;
// Ends a way into full screen AppKit gave up, leaving the window windowed, as an exit's.
- (void)finishFullScreenEnter;
@end

@implementation BrowserCoreNSWindow {
  // AppKit said it's taking the window out of full screen and hasn't said it's done, from which
  // display, and whether the window has gone to another meanwhile.
  BOOL _leaving;
  NSNumber* _leavingFrom;
  BOOL _leavingMoved;
  // AppKit said it's taking the window into full screen and hasn't said it's done.
  BOOL _entering;
}

- (instancetype)initWithContentRect:(NSRect)contentRect
                          styleMask:(NSUInteger)windowStyle
                            backing:(NSBackingStoreType)bufferingType
                              defer:(BOOL)deferCreation {
  if ((self = [super initWithContentRect:contentRect
                               styleMask:windowStyle
                                 backing:bufferingType
                                   defer:deferCreation])) {
    NSNotificationCenter* center = NSNotificationCenter.defaultCenter;
    [center addObserver:self
               selector:@selector(willEnterFullScreen:)
                   name:NSWindowWillEnterFullScreenNotification
                 object:self];
    [center addObserver:self
               selector:@selector(didEnterFullScreen:)
                   name:NSWindowDidEnterFullScreenNotification
                 object:self];
    [center addObserver:self
               selector:@selector(willExitFullScreen:)
                   name:NSWindowWillExitFullScreenNotification
                 object:self];
    [center addObserver:self
               selector:@selector(didExitFullScreen:)
                   name:NSWindowDidExitFullScreenNotification
                 object:self];
    [center addObserver:self
               selector:@selector(didChangeScreen:)
                   name:NSWindowDidChangeScreenNotification
                 object:self];
  }
  return self;
}

- (void)willEnterFullScreen:(NSNotification*)notification {
  _entering = YES;
}

- (void)didEnterFullScreen:(NSNotification*)notification {
  _entering = NO;
}

- (void)willExitFullScreen:(NSNotification*)notification {
  _leaving = YES;
  _leavingFrom = self.screen.deviceDescription[@"NSScreenNumber"];
  _leavingMoved = NO;
}

- (void)didExitFullScreen:(NSNotification*)notification {
  _leaving = NO;
  _entering = NO;
}

- (void)didChangeScreen:(NSNotification*)notification {
  // AppKit says the screen changed on its way out on the same display too.
  _leavingMoved |=
      _leaving && ![_leavingFrom isEqual:self.screen.deviceDescription[@"NSScreenNumber"]];
  [self finishFullScreenExit];
}

- (void)finishFullScreenExit {
  // A window whose display goes away can come out of full screen on another with AppKit's will
  // and no did (#415), or with neither. Chrome's widget then waits for the end of a transition
  // that never comes, holding the page full screen and every request to go in or out, and the
  // app keeps its full-screen layout. Any windowed window that Chrome still holds full screen, or
  // that AppKit started out and moved without finishing, gets AppKit's missing notifications, as
  // AppKit would have sent them, so Chrome's widget, its observers and the app all hear one end.
  // A window AppKit is still taking out on its own screen is left to finish.
  if (self.styleMask & NSWindowStyleMaskFullScreen) {
    return;
  }
  remote_cocoa::NativeWidgetNSWindowBridge* bridge = self.bridge;
  const bool held =
      bridge && bridge->target_fullscreen_state() && !bridge->in_fullscreen_transition();
  if (!held && !(_leaving && _leavingMoved)) {
    return;
  }
  fprintf(stderr, "browser: window full screen exit finished for AppKit\n");
  NSNotificationCenter* center = NSNotificationCenter.defaultCenter;
  if (!_leaving) {
    [center postNotificationName:NSWindowWillExitFullScreenNotification object:self];
  }
  [center postNotificationName:NSWindowDidExitFullScreenNotification object:self];
}

- (void)finishFullScreenEnter {
  // AppKit tells only the window's delegate of a way into full screen it gave up, as when a press
  // comes as a way out ends, and the window ends windowed with AppKit's will and no did. Chrome's
  // occlusion checker passes over a window from AppKit's will until its did, so it never looks at
  // this one again: a page shown in it later stays occluded, its keys going nowhere (#2248). The
  // window gets the did AppKit sometimes sends after a failure, its exit's, which Chrome's widget
  // takes as it is already windowed, so the checker, the widget and the app all hear one end.
  if (!_entering || (self.styleMask & NSWindowStyleMaskFullScreen)) {
    return;
  }
  remote_cocoa::NativeWidgetNSWindowBridge* bridge = self.bridge;
  if (bridge && (bridge->target_fullscreen_state() || bridge->in_fullscreen_transition())) {
    return;
  }
  _entering = NO;
  fprintf(stderr, "browser: window full screen enter ended windowed, finished for AppKit\n");
  [NSNotificationCenter.defaultCenter postNotificationName:NSWindowDidExitFullScreenNotification
                                                    object:self];
}

+ (Class)frameViewClassForStyleMask:(NSWindowStyleMask)windowStyle {
  // Nil if a future macOS has no NSThemeFrame, which leaves AppKit's own.
  if ((windowStyle & NSWindowStyleMaskTitled) && [BrowserCoreWindowFrame class]) {
    return [BrowserCoreWindowFrame class];
  }
  return [super frameViewClassForStyleMask:windowStyle];
}

@end

namespace browser_core::platform {

namespace {

class NativeWidget : public views::NativeWidgetMac {
 public:
  NativeWidget(views::Widget* widget, base::RepeatingClosure leaving_full_screen)
      : views::NativeWidgetMac(widget), leaving_full_screen_(std::move(leaving_full_screen)) {}

  void SetFullscreen(bool fullscreen, int64_t target_display_id) override {
    // A way out AppKit didn't finish is over first, so the request isn't kept waiting for it.
    NSWindow* window = GetNativeWindow().GetNativeNSWindow();
    if ([window isKindOfClass:[BrowserCoreNSWindow class]]) {
      [(BrowserCoreNSWindow*)window finishFullScreenExit];
    }
    views::NativeWidgetMac::SetFullscreen(fullscreen, target_display_id);
  }

  void OnWindowFullscreenTransitionStart() override {
    // Chrome's widget counts the window windowed from here, though its observers hear only once
    // AppKit is done, and AppKit may never be, as when the window's display goes away (#415).
    if (!IsFullscreen()) {
      leaving_full_screen_.Run();
    }
  }

  void OnWindowFullscreenTransitionComplete() override {
    // A way in AppKit gave up ends here, windowed, as Chrome's widget handles the failure. Its
    // missing did comes on the next turn, out of that handling.
    NSWindow* window = GetNativeWindow().GetNativeNSWindow();
    if (IsFullscreen() || ![window isKindOfClass:[BrowserCoreNSWindow class]]) {
      return;
    }
    __weak BrowserCoreNSWindow* weak_window = (BrowserCoreNSWindow*)window;
    dispatch_async(dispatch_get_main_queue(), ^{
      [weak_window finishFullScreenEnter];
    });
  }

 protected:
  // As NativeWidgetNSWindowBridge::CreateNSWindow makes a default window, with the frame above.
  NativeWidgetMacNSWindow* CreateNSWindow(
      const remote_cocoa::mojom::CreateWindowParams* params) override {
    // The app's UI fills the window, title bar and all.
    NativeWidgetMacNSWindow* window = [[BrowserCoreNSWindow alloc]
        initWithContentRect:ui::kWindowSizeDeterminedLater
                  styleMask:params->style_mask | NSWindowStyleMaskFullSizeContentView
                    backing:NSBackingStoreBuffered
                      defer:NO];
    window.releasedWhenClosed = NO;
    window.titlebarAppearsTransparent = YES;
    window.titleVisibility = NSWindowTitleHidden;
    if (params->animation_enabled) {
      window.animationBehavior = NSWindowAnimationBehaviorDocumentWindow;
    }
    return window;
  }

 private:
  base::RepeatingClosure leaving_full_screen_;
};

}  // namespace

views::NativeWidget* NewNativeWidget(views::Widget* widget,
                                     base::RepeatingClosure leaving_full_screen) {
  // It deletes itself as its window closes.
  return new NativeWidget(widget, std::move(leaving_full_screen));
}

}  // namespace browser_core::platform
