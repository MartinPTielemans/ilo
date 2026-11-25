// The window's material under a floating window's plate (app/src/glass.rs, #4090): toasts and the
// zoom HUD. The UI draws the plate's translucent fill and its shadow; the material goes under the
// plate alone, as the menu's goes under its panels (menu_window.mm), so the room round it for the
// shadow stays clear.

#import <Cocoa/Cocoa.h>
#import <objc/runtime.h>

#include "glass.h"

namespace {

// The key the view's glass is kept under.
char g_glass_key;

// The plate's shape its glass's mask was drawn for, in the view's size, kept with the glass.
struct Shape {
  NSRect plate;
  double radius;
  NSSize size;
};
char g_shape_key;

bool SameShape(const Shape& a, const Shape& b) {
  return NSEqualRects(a.plate, b.plate) && a.radius == b.radius && NSEqualSizes(a.size, b.size);
}

}  // namespace

bool host_set_glass(void* ui_view,
                    double left,
                    double top,
                    double width,
                    double height,
                    double radius,
                    double opacity,
                    bool light) {
  NSView* view = (__bridge NSView*)ui_view;
  NSView* holder = view.superview;
  const NSSize size = view.bounds.size;
  if (!holder || size.width <= 0 || size.height <= 0) {
    return false;
  }
  NSVisualEffectView* glass = objc_getAssociatedObject(view, &g_glass_key);
  if (!glass) {
    // The HUD window's material, its tint kept, as under the overlays (host.mm). AppKit makes it
    // opaque with Reduce Transparency or Increase Contrast, as it does the window's.
    glass = [[NSVisualEffectView alloc] initWithFrame:view.frame];
    glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    glass.blendingMode = NSVisualEffectBlendingModeBehindWindow;
    glass.material = NSVisualEffectMaterialHUDWindow;
    glass.state = NSVisualEffectStateActive;
    [holder addSubview:glass positioned:NSWindowBelow relativeTo:view];
    objc_setAssociatedObject(view, &g_glass_key, glass, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
  }
  const NSRect plate = NSMakeRect(left, top, width, height);
  // The mask is drawn again only as the plate moves, grows or rounds otherwise: a fade changes
  // the alpha alone, every frame.
  const Shape shape{plate, radius, size};
  NSValue* drawn = objc_getAssociatedObject(view, &g_shape_key);
  Shape was{};
  if (drawn) {
    [drawn getValue:&was size:sizeof(was)];
  }
  if (!drawn || !SameShape(was, shape)) {
    // Drawn at the view's size from its top left, as GPUI's coordinates go.
    NSImage* mask = [NSImage imageWithSize:size
                                   flipped:YES
                            drawingHandler:^BOOL(NSRect) {
                              [NSColor.blackColor setFill];
                              [[NSBezierPath bezierPathWithRoundedRect:plate
                                                               xRadius:radius
                                                               yRadius:radius] fill];
                              return YES;
                            }];
    mask.resizingMode = NSImageResizingModeStretch;
    glass.maskImage = mask;
    objc_setAssociatedObject(view, &g_shape_key,
                             [NSValue valueWithBytes:&shape objCType:@encode(Shape)],
                             OBJC_ASSOCIATION_RETAIN_NONATOMIC);
  }
  glass.alphaValue = opacity;
  // The window's look, which the UI's fill over it is drawn for.
  glass.appearance =
      [NSAppearance appearanceNamed:light ? NSAppearanceNameAqua : NSAppearanceNameDarkAqua];
  return true;
}
