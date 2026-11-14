#include "browser_core/platform/window_views.h"

#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

#include <cmath>
#include <cstdio>
#include <memory>
#include <utility>

#include "base/apple/foundation_util.h"
#include "base/functional/callback.h"
#include "base/task/sequenced_task_runner.h"
#include "base/time/time.h"
#import "components/remote_cocoa/app_shim/bridged_content_view.h"
#include "content/public/browser/frame_shown.h"
#include "content/public/browser/render_widget_host_view.h"
#include "content/public/browser/stretch_content_on_resize_mac.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/views/widget/widget.h"

// A page the window shows, with its DevTools when they're docked.
@interface BrowserCorePane : NSObject
@property(weak, nonatomic) NSView* page;
@property(weak, nonatomic) NSView* devtools;
// The distance from each edge of the window to the page's frame, which it keeps as the window
// resizes.
@property(nonatomic) NSEdgeInsets insets;
// With DevTools docked, where the page goes in its frame, from the frame's top-left.
@property(nonatomic) NSRect pageInDevTools;
@end

@implementation BrowserCorePane
@synthesize page = _page;
@synthesize devtools = _devtools;
@synthesize insets = _insets;
@synthesize pageInDevTools = _pageInDevTools;
@end

// The ring inside the focused page's edge in a split view, over the page. Clicks go through it.
@interface BrowserCoreFocusRing : NSView
@end

@implementation BrowserCoreFocusRing
- (NSView*)hitTest:(NSPoint)point {
  return nil;
}
@end

// Chromium's occlusion checker, and the flag it keeps on each window, which content doesn't export
// in a component build.
@protocol BrowserCoreOcclusionChecker
+ (instancetype)sharedInstance;
- (void)performOcclusionStateUpdates;
@end

@protocol BrowserCoreOccludedWindow
- (BOOL)isOccluded;
@end

// Holds the app's UI view and, over it, the pages shown, left to right, with the focused one's ring
// in a split view, and an extension's side panel. Flipped, so frames run from the top-left as the
// core's do.
@interface BrowserCoreWindowView : NSView
@property(strong, nonatomic) NSView* uiView;
@property(copy, nonatomic) NSArray<BrowserCorePane*>* panes;
@property(nonatomic) NSUInteger focused;
@property(strong, nonatomic) BrowserCoreFocusRing* ring;
@property(strong, nonatomic) BrowserCorePane* sidePanel;
// The views of the pages a switch replaced, kept over the pages it shows until those have drawn,
// and those pages. A page shown again shows only its background until its first frame, 60 to 230 ms
// on GitHub's hosted Mac, which over the old page would leave no page in the page area (#1438).
@property(strong, nonatomic) NSHashTable<NSView*>* leaving;
@property(strong, nonatomic) NSHashTable<NSView*>* awaited;
// How many switches have kept pages, so the limit on keeping them ends only the last one's.
@property(nonatomic) NSUInteger holds;
// What the panes are drawn at, about the centre of their frames (ScalePages); 0 is 1.
@property(nonatomic) double pageScale;
- (void)layOutPage;
- (void)scalePages;
- (void)letLeavingGo;
@end

@implementation BrowserCoreWindowView
@synthesize uiView = _uiView;
@synthesize panes = _panes;
@synthesize focused = _focused;
@synthesize ring = _ring;
@synthesize sidePanel = _sidePanel;
@synthesize leaving = _leaving;
@synthesize awaited = _awaited;
@synthesize holds = _holds;
@synthesize pageScale = _pageScale;

- (BOOL)isFlipped {
  return YES;
}

// Through the pages a switch is leaving, as if they had gone: they only stand in for the pages it
// shows until those draw.
- (NSView*)hitTest:(NSPoint)point {
  NSView* hit = [super hitTest:point];
  if (!hit || self.leaving.count == 0) {
    return hit;
  }
  BOOL leaving = NO;
  for (NSView* page in self.leaving) {
    leaving = leaving || [hit isDescendantOf:page];
  }
  if (!leaving) {
    return hit;
  }
  NSPoint local = [self convertPoint:point fromView:self.superview];
  for (NSView* subview in [self.subviews reverseObjectEnumerator]) {
    if (![self.leaving containsObject:subview]) {
      if (NSView* under = [subview hitTest:local]) {
        return under;
      }
    }
  }
  return self;
}

- (void)letLeavingGo {
  for (NSView* page in self.leaving.allObjects) {
    if (page.superview == self) {
      [page removeFromSuperview];
    }
  }
  [self.leaving removeAllObjects];
}

// After AppKit's pass rather than by autoresizing, which would resize the page twice: resizing the
// UI view tells the app, which shows the page again at its new frame before AppKit reaches it.
- (void)resizeSubviewsWithOldSize:(NSSize)oldSize {
  [super resizeSubviewsWithOldSize:oldSize];
  [self layOutPage];
}

- (NSRect)frameOf:(BrowserCorePane*)pane {
  NSRect bounds = self.bounds;
  NSEdgeInsets insets = pane.insets;
  return NSMakeRect(insets.left, insets.top, NSWidth(bounds) - insets.left - insets.right,
                    NSHeight(bounds) - insets.top - insets.bottom);
}

- (void)layOutPage {
  for (BrowserCorePane* pane in self.panes) {
    NSView* page = pane.page;
    if (page.superview != self) {
      continue;
    }
    NSRect frame = [self frameOf:pane];
    NSView* devtools = pane.devtools;
    if (devtools.superview != self) {
      page.frame = frame;
      continue;
    }
    devtools.frame = frame;
    // The core places the page again for each new frame, and DevTools once they've laid out at it.
    page.frame = NSIntersectionRect(
        NSOffsetRect(pane.pageInDevTools, NSMinX(frame), NSMinY(frame)), frame);
  }
  if (self.ring.superview == self && self.focused < self.panes.count) {
    self.ring.frame = [self frameOf:self.panes[self.focused]];
  }
  if (self.sidePanel.page.superview == self) {
    self.sidePanel.page.frame = [self frameOf:self.sidePanel];
  }
  [self scalePages];
}

// By each view's layer transform, so the pages are never laid out again for it. AppKit keeps each
// layer's anchor point and which way up it is, so where a transform puts it is measured in this
// view's layer rather than worked out.
- (void)scalePages {
  NSMutableArray<CALayer*>* layers = [NSMutableArray array];
  for (BrowserCorePane* pane in self.panes) {
    for (NSView* view in @[ pane.page ?: NSNull.null, pane.devtools ?: NSNull.null ]) {
      if ([view isKindOfClass:NSView.class] && view.superview == self && view.layer) {
        [layers addObject:view.layer];
      }
    }
  }
  if (self.ring.superview == self && self.ring.layer) {
    [layers addObject:self.ring.layer];
  }
  CALayer* space = self.layer;
  auto frame = [space](CALayer* layer) { return [layer convertRect:layer.bounds toLayer:space]; };
  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  CGRect frames = CGRectNull;
  for (CALayer* layer in layers) {
    layer.transform = CATransform3DIdentity;
    frames = CGRectUnion(frames, frame(layer));
  }
  const double scale = self.pageScale > 0 ? self.pageScale : 1;
  if (scale != 1 && !CGRectIsNull(frames)) {
    const CGPoint about = CGPointMake(CGRectGetMidX(frames), CGRectGetMidY(frames));
    for (CALayer* layer in layers) {
      const CGRect own = frame(layer);
      const CGPoint want = CGPointMake(about.x + scale * (CGRectGetMinX(own) - about.x),
                                       about.y + scale * (CGRectGetMinY(own) - about.y));
      const CATransform3D scaled = CATransform3DMakeScale(scale, scale, 1);
      layer.transform = scaled;
      const CGRect at = frame(layer);
      const double dx = want.x - CGRectGetMinX(at);
      const double dy = want.y - CGRectGetMinY(at);
      layer.transform = CATransform3DConcat(scaled, CATransform3DMakeTranslation(dx, dy, 0));
      // A layer upside down to this one moves the other way.
      if (std::abs(CGRectGetMinY(frame(layer)) - want.y) > 0.5) {
        layer.transform = CATransform3DConcat(scaled, CATransform3DMakeTranslation(dx, -dy, 0));
      }
    }
  }
  [CATransaction commit];
}
@end

namespace browser_core::platform {

namespace {

BridgedContentView* ContentView(views::Widget* widget) {
  return base::apple::ObjCCastStrict<BridgedContentView>(
      widget->GetNativeWindow().GetNativeNSWindow().contentView);
}

BrowserCoreWindowView* WindowView(views::Widget* widget) {
  return base::apple::ObjCCastStrict<BrowserCoreWindowView>(ContentView(widget).embeddedView);
}

// Shows a page's view, or its DevTools', which Chrome's views host leaves hidden while their tab is
// in the background, unless |hidden| as its window is out of sight. Hidden, a page never draws, and
// Chromium drops the keys it doesn't use instead of passing them to the window.
void Unhide(NSView* view, bool hidden) {
  view.autoresizingMask = NSViewNotSizable;
  view.hidden = hidden;
  // Clipped by Core Animation, which rounds the compositor's layers under it without redrawing
  // them.
  view.wantsLayer = YES;
}

// On the continuous curve the Mac rounds its windows with, so the card's corners, the window's
// radius less the frame, are concentric with the window's (the app's window-radius-mac token).
void RoundCorners(NSView* view, double radius, CACornerMask corners) {
  view.layer.cornerCurve = kCACornerCurveContinuous;
  view.layer.cornerRadius = radius;
  view.layer.maskedCorners = corners;
  view.layer.masksToBounds = radius > 0 && corners;
}

// The corners of |frame| that |rect|, in it from its top-left, reaches.
CACornerMask CornersReached(NSView* view, NSRect rect, NSSize frame) {
  const bool left = NSMinX(rect) <= 0;
  const bool right = NSMaxX(rect) >= frame.width;
  const bool top = NSMinY(rect) <= 0;
  const bool bottom = NSMaxY(rect) >= frame.height;
  // A layer's minimum Y is its view's top if the view is flipped.
  const bool min_y = view.isFlipped ? top : bottom;
  const bool max_y = view.isFlipped ? bottom : top;
  return (left && min_y ? kCALayerMinXMinYCorner : 0) |
         (right && min_y ? kCALayerMaxXMinYCorner : 0) |
         (left && max_y ? kCALayerMinXMaxYCorner : 0) |
         (right && max_y ? kCALayerMaxXMaxYCorner : 0);
}

// Chromium's occlusion checker passes over a window with no page in it, so a window that comes
// back from full screen or from behind another while the page is out can stay marked occluded,
// and a page put in it then never draws or hands back the keys it doesn't use. Chrome's windows
// always hold a page. Asked again only for a window marked occluded: the checker asks the window
// server about every window, which takes milliseconds.
void UpdateOcclusion(NSWindow* window) {
  if (![window respondsToSelector:@selector(isOccluded)] ||
      ![static_cast<id<BrowserCoreOccludedWindow>>(window) isOccluded]) {
    return;
  }
  Class<BrowserCoreOcclusionChecker> checker = NSClassFromString(@"WebContentsOcclusionCheckerMac");
  [[checker sharedInstance] performOcclusionStateUpdates];
}

// How long the pages a switch left stay over one that hasn't drawn, such as one whose renderer is
// stuck.
constexpr base::TimeDelta kLeavingAtMost = base::Seconds(1);

// Keeps the pages leaving |view| until each of |arrivals| has drawn in it. Asked once they're in
// the window, so the frame each waits for is one drawn there. Not at the first frame of the page's
// compositor, which can come before the page's own and, with the new layer it draws into, leaves
// a frame of neither page on screen.
void HoldLeaving(BrowserCoreWindowView* view,
                 base::span<const Pane> panes,
                 NSSet<NSView*>* arrivals) {
  const NSUInteger hold = ++view.holds;
  const base::TimeTicks since = base::TimeTicks::Now();
  // check.sh's switch-frames run reads these lines.
  fprintf(stderr, "browser: switch keeps the page it left\n");
  __weak BrowserCoreWindowView* weak_view = view;
  for (const Pane& pane : panes) {
    NSView* page_view = pane.page.GetNativeNSView();
    if (![arrivals containsObject:page_view]) {
      continue;
    }
    [view.awaited addObject:page_view];
    __weak NSView* weak_page = page_view;
    content::CallWhenFrameShown(pane.widget, base::BindOnce(^{
      BrowserCoreWindowView* shown_in = weak_view;
      if (NSView* drawn = weak_page) {
        [shown_in.awaited removeObject:drawn];
      }
      if (shown_in.awaited.count == 0 && shown_in.leaving.count > 0) {
        fprintf(stderr, "browser: switch let the page it left go after %.1f ms\n",
                (base::TimeTicks::Now() - since).InMillisecondsF());
        [shown_in letLeavingGo];
      }
    }));
  }
  base::SequencedTaskRunner::GetCurrentDefault()->PostDelayedTask(
      FROM_HERE, base::BindOnce(^{
        BrowserCoreWindowView* shown_in = weak_view;
        if (shown_in.holds == hold && shown_in.leaving.count > 0) {
          fprintf(stderr, "browser: switch let the page it left go at the limit\n");
          [shown_in.awaited removeAllObjects];
          [shown_in letLeavingGo];
        }
      }),
      kLeavingAtMost);
}

constexpr CACornerMask kAllCorners = kCALayerMinXMinYCorner | kCALayerMaxXMinYCorner |
                                     kCALayerMinXMaxYCorner | kCALayerMaxXMaxYCorner;

}  // namespace

void AddWindowViews(views::Widget* widget, void* ui_view) {
  BridgedContentView* content = ContentView(widget);
  BrowserCoreWindowView* view = [[BrowserCoreWindowView alloc] initWithFrame:content.bounds];
  view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  if (NSView* ui = (__bridge NSView*)ui_view) {
    ui.frame = view.bounds;
    ui.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    [view addSubview:ui];
    view.uiView = ui;
  }
  [content addSubview:view];
  // Without this, the content view's hit test and accessibility would skip the page and the UI
  // (engine/patches/ours/chromium-core-embedded-view.patch).
  content.embeddedView = view;
}

void ShowPages(views::Widget* widget,
               base::span<const Pane> panes,
               double corner_radius,
               size_t focused,
               uint32_t ring_color,
               std::optional<SidePanelPage> side_panel) {
  BrowserCoreWindowView* view = WindowView(widget);
  const bool had_page = view.panes.count > 0;
  NSMutableSet<NSView*>* staying = [NSMutableSet set];
  for (const Pane& pane : panes) {
    [staying addObject:pane.page.GetNativeNSView()];
    if (pane.devtools) {
      [staying addObject:pane.devtools->view.GetNativeNSView()];
    }
  }
  NSView* panel_view = side_panel ? side_panel->page.GetNativeNSView() : nil;
  if (panel_view) {
    [staying addObject:panel_view];
  }
  const bool hidden = !widget->IsVisible();
  if (!view.leaving) {
    view.leaving = [NSHashTable weakObjectsHashTable];
    view.awaited = [NSHashTable weakObjectsHashTable];
  }
  // A page left that shows again is no longer leaving, and one awaited that no longer shows is no
  // longer awaited.
  for (NSView* page in view.leaving.allObjects) {
    if ([staying containsObject:page]) {
      [view.leaving removeObject:page];
    }
  }
  for (NSView* page in view.awaited.allObjects) {
    if (![staying containsObject:page]) {
      [view.awaited removeObject:page];
    }
  }
  // The pages put in the window, each showing only its background until its renderer has drawn
  // there, even one that was drawing unseen for a picture or in another window.
  NSMutableSet<NSView*>* arrivals = [NSMutableSet set];
  for (const Pane& pane : panes) {
    NSView* page_view = pane.page.GetNativeNSView();
    if (!hidden && pane.widget && page_view.superview != view) {
      [arrivals addObject:page_view];
    }
  }
  const bool arriving = arrivals.count > 0;
  // The pages that leave stay where they are, over the pages a switch shows, until those have
  // drawn, as do the ones an earlier switch left while its pages haven't.
  NSWindow* window = view.window;
  for (NSView* subview in [view.subviews copy]) {
    if (subview == view.uiView || subview == view.ring || [staying containsObject:subview]) {
      continue;
    }
    const bool kept = [view.leaving containsObject:subview];
    if (!arriving && !(kept && view.awaited.count > 0)) {
      [view.leaving removeObject:subview];
      [subview removeFromSuperview];
      continue;
    }
    if (!kept) {
      [view.leaving addObject:subview];
      // The window has the keyboard once the page that had it leaves, as if it had gone.
      NSView* keyboard = base::apple::ObjCCast<NSView>(window.firstResponder);
      if ([keyboard isDescendantOf:subview]) {
        [window makeFirstResponder:nil];
      }
    }
  }
  const bool holding = arriving && view.leaving.count > 0;
  // The pages put in the window go under the ones leaving it.
  NSView* lowest_leaving = nil;
  for (NSView* subview in view.subviews) {
    if ([view.leaving containsObject:subview]) {
      lowest_leaving = subview;
      break;
    }
  }
  NSRect bounds = view.bounds;
  auto insets = [&](const gfx::Rect& frame) {
    NSRect rect = frame.ToCGRect();
    return NSEdgeInsetsMake(NSMinY(rect), NSMinX(rect), NSHeight(bounds) - NSMaxY(rect),
                            NSWidth(bounds) - NSMaxX(rect));
  };
  NSMutableArray<BrowserCorePane*>* shown = [NSMutableArray array];
  for (const Pane& pane : panes) {
    NSView* page_view = pane.page.GetNativeNSView();
    NSView* devtools_view = pane.devtools ? pane.devtools->view.GetNativeNSView() : nil;
    BrowserCorePane* record = [[BrowserCorePane alloc] init];
    record.page = page_view;
    record.devtools = devtools_view;
    Unhide(page_view, hidden);
    if (devtools_view) {
      Unhide(devtools_view, hidden);
      RoundCorners(devtools_view, corner_radius, kAllCorners);
      record.pageInDevTools = pane.devtools->page_frame.ToCGRect();
      RoundCorners(page_view, corner_radius,
                   CornersReached(page_view, record.pageInDevTools, pane.frame.size().ToCGSize()));
      if (devtools_view.superview != view) {
        [view addSubview:devtools_view positioned:NSWindowAbove relativeTo:view.uiView];
      }
    } else {
      RoundCorners(page_view, corner_radius, kAllCorners);
    }
    if (page_view.superview != view) {
      if (lowest_leaving) {
        [view addSubview:page_view positioned:NSWindowBelow relativeTo:lowest_leaving];
      } else {
        [view addSubview:page_view];
      }
    }
    if (devtools_view) {
      // Only when it's out of order: a view moved loses the keyboard.
      NSArray<NSView*>* subviews = view.subviews;
      const bool page_over =
          [subviews indexOfObject:page_view] > [subviews indexOfObject:devtools_view];
      if (page_over == pane.devtools->page_under) {
        [view addSubview:page_view
              positioned:pane.devtools->page_under ? NSWindowBelow : NSWindowAbove
              relativeTo:devtools_view];
      }
    }
    record.insets = insets(pane.frame);
    [shown addObject:record];
  }
  if (panel_view) {
    BrowserCorePane* record = [[BrowserCorePane alloc] init];
    record.page = panel_view;
    record.insets = insets(side_panel->frame);
    Unhide(panel_view, hidden);
    RoundCorners(panel_view, corner_radius,
                 panel_view.isFlipped ? kCALayerMinXMaxYCorner | kCALayerMaxXMaxYCorner
                                      : kCALayerMinXMinYCorner | kCALayerMaxXMinYCorner);
    if (panel_view.superview != view) {
      [view addSubview:panel_view];
    }
    view.sidePanel = record;
  } else {
    view.sidePanel = nil;
  }
  view.panes = shown;
  view.focused = focused;
  if (shown.count > 1) {
    if (!view.ring) {
      view.ring = [[BrowserCoreFocusRing alloc] init];
      view.ring.wantsLayer = YES;
      // Arc's: 2 pt, inside the page's edge.
      view.ring.layer.borderWidth = 2;
    }
    view.ring.layer.cornerCurve = kCACornerCurveContinuous;
    view.ring.layer.cornerRadius = corner_radius;
    view.ring.layer.borderColor = [NSColor colorWithSRGBRed:((ring_color >> 16) & 0xff) / 255.0
                                                      green:((ring_color >> 8) & 0xff) / 255.0
                                                       blue:(ring_color & 0xff) / 255.0
                                                      alpha:1]
                                      .CGColor;
    // Moving the ring, which never has the keyboard, rather than a page.
    if (view.subviews.lastObject != view.ring) {
      [view addSubview:view.ring positioned:NSWindowAbove relativeTo:nil];
    }
  } else {
    [view.ring removeFromSuperview];
  }
  [view layOutPage];
  if (!had_page && !panes.empty()) {
    UpdateOcclusion(view.window);
  }
  if (holding) {
    HoldLeaving(view, panes, arrivals);
  }
}

void ScalePages(views::Widget* widget, double scale) {
  BrowserCoreWindowView* view = WindowView(widget);
  view.pageScale = scale;
  [view scalePages];
}

// AppKit gives a view's points from its frame, which the layer's scale doesn't move.
gfx::PointF LaidOutPagePoint(gfx::NativeView, const gfx::PointF& point) {
  return point;
}

void HoldPagesForResize(views::Widget*) {}

bool PagesHeldForResize(views::Widget*) {
  return false;
}

void HidePages(views::Widget* widget) {
  BrowserCoreWindowView* view = WindowView(widget);
  for (BrowserCorePane* pane in view.panes) {
    pane.page.hidden = YES;
    pane.devtools.hidden = YES;
  }
  view.sidePanel.page.hidden = YES;
  [view.awaited removeAllObjects];
  [view letLeavingGo];
}

void ShowPageIn(views::Widget*,
                gfx::NativeView page,
                void* view,
                const gfx::Rect& frame,
                double corner_radius) {
  NSView* holder = (__bridge NSView*)view;
  NSView* page_view = page.GetNativeNSView();
  if (!holder || !page_view) {
    return;
  }
  Unhide(page_view, /*hidden=*/false);
  RoundCorners(page_view, corner_radius, kAllCorners);
  if (page_view.superview != holder) {
    [holder addSubview:page_view];
  }
  NSRect rect = frame.ToCGRect();
  if (!holder.isFlipped) {
    rect.origin.y = NSHeight(holder.bounds) - NSMaxY(rect);
  }
  page_view.frame = rect;
  UpdateOcclusion(holder.window);
}

bool FocusSettledOnActivation() {
  // AppKit can be part way through moving the keyboard, as when a committing page hides the view
  // that had it and Chromium makes the window key: a page view focused inside that move believes
  // it has the keyboard after AppKit has given it elsewhere.
  return false;
}

void PassPointerThrough(views::Widget*) {
  // Views' accept_events makes the window ignore mouse events.
}

bool PageDialogsInOwnWindow() {
  // A child NSWindow either way.
  return false;
}

bool KeyboardForPage(views::Widget* widget) {
  NSWindow* window = widget->GetNativeWindow().GetNativeNSWindow();
  // AppKit gives the keyboard to the window when the view that had it leaves, and views gives it to
  // the content view as the window opens. Neither holds anything of the core's that takes it.
  if (window.firstResponder == window || window.firstResponder == window.contentView) {
    return true;
  }
  BrowserCoreWindowView* view = WindowView(widget);
  NSView* focused = base::apple::ObjCCast<NSView>(window.firstResponder);
  if (![focused isDescendantOf:view] || [focused isDescendantOf:view.uiView]) {
    return false;
  }
  for (BrowserCorePane* pane in view.panes) {
    if (pane.devtools && [focused isDescendantOf:pane.devtools]) {
      return false;
    }
  }
  return !view.sidePanel || ![focused isDescendantOf:view.sidePanel.page];
}

void ReleaseKeyboard(views::Widget* widget) {
  [widget->GetNativeWindow().GetNativeNSWindow() makeFirstResponder:nil];
}

bool OverlayHasKeyboard(views::Widget* widget) {
  if (!widget) {
    return false;
  }
  NSWindow* window = widget->GetNativeWindow().GetNativeNSWindow();
  // A submenu's window is the menu's child.
  for (NSWindow* over = NSApp.keyWindow.parentWindow; over; over = over.parentWindow) {
    if (over == window) {
      return true;
    }
  }
  return false;
}

void WhenOverlayLetsGo(views::Widget* widget, base::OnceClosure let_go) {
  NSWindow* overlay = NSApp.keyWindow;
  if (!overlay) {
    return;
  }
  auto held = std::make_shared<base::OnceClosure>(std::move(let_go));
  NSNotificationCenter* center = NSNotificationCenter.defaultCenter;
  __block id observer = [center addObserverForName:NSWindowDidResignKeyNotification
                                            object:overlay
                                             queue:nil
                                        usingBlock:^(NSNotification*) {
                                          [center removeObserver:observer];
                                          observer = nil;
                                          // Posted, as the next key window takes it after.
                                          base::SequencedTaskRunner::GetCurrentDefault()->PostTask(
                                              FROM_HERE, std::move(*held));
                                        }];
}

void KeepKeyboard(views::Widget* widget, base::OnceClosure change) {
  NSWindow* window = widget->GetNativeWindow().GetNativeNSWindow();
  NSResponder* had = window.firstResponder;
  std::move(change).Run();
  NSView* had_view = base::apple::ObjCCast<NSView>(had);
  if (window.firstResponder != had && (had == window || had_view.window == window)) {
    [window makeFirstResponder:had];
  }
}

void StretchOnResize(content::RenderWidgetHostView* view, bool stretch) {
  content::SetStretchContentOnResize(view, stretch);
}

base::ScopedClosureRunner DrawUnseen(views::Widget*, gfx::NativeView) {
  return base::ScopedClosureRunner();
}

}  // namespace browser_core::platform
