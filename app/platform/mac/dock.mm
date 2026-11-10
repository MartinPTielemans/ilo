// The Dock icon's dot in a Space's color (docs/ux-spec.md › Dock dot), painted over macOS's own
// drawing of the app's icon, so it keeps the icon style's glass and shading. Drawing that takes
// from 10 ms to, right after a style change, 3 s, so it's done off the main thread, and only the
// swap, 10–45 ms in the Dock, runs on it.

#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>

#include <atomic>
#include <cstdlib>

#include "../host.h"

namespace {

// The icon's drawing is this many pixels square, with the dot centred at (kDotX, kDotY) from the
// top-left corner, as macOS draws the Icon Composer icon in branding/.
constexpr int kSize = 1024;
constexpr int kDotX = 512, kDotY = 328;
// A little past the dot's edge, so its antialiased rim takes the color too.
constexpr int kDotRadius = 79;

// Not public: AppKit exports it but doesn't declare it.
NSString* const kIconStyleChanged = @"NSWorkspaceIconAppearanceConfigurationDidChangeNotification";

NSString* g_saved;
// On the main thread: what the dot was last asked for, so a style change can ask again.
bool g_colored;
uint32_t g_color;
// Bumped by each ask, so a drawing a later ask has made pointless isn't made.
std::atomic<uint64_t> g_asked;
// On Queue(): macOS's drawing the Dock's icon was last decided on, and the color it shows, or nil
// while it shows macOS's own.
NSData* g_drawn;
NSNumber* g_shown;

dispatch_queue_t Queue() {
  static dispatch_queue_t queue = dispatch_queue_create(
      "dock-dot", dispatch_queue_attr_make_with_qos_class(DISPATCH_QUEUE_SERIAL, QOS_CLASS_UTILITY, 0));
  return queue;
}

// macOS's drawing of the app's icon in the user's icon style.
NSBitmapImageRep* Drawn() {
  NSBitmapImageRep* rep = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr
                                                                  pixelsWide:kSize
                                                                  pixelsHigh:kSize
                                                               bitsPerSample:8
                                                             samplesPerPixel:4
                                                                    hasAlpha:YES
                                                                    isPlanar:NO
                                                              colorSpaceName:NSDeviceRGBColorSpace
                                                                 bytesPerRow:0
                                                                bitsPerPixel:0];
  [NSGraphicsContext saveGraphicsState];
  NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:rep];
  [[NSWorkspace.sharedWorkspace iconForFile:NSBundle.mainBundle.bundlePath]
      drawInRect:NSMakeRect(0, 0, kSize, kSize)];
  [NSGraphicsContext restoreGraphicsState];
  return rep;
}

// Whether macOS drew the dot in our green, as in the Default and Dark styles. Tinted and Clear
// draw it in the style's colors, and there the icon stays as macOS draws it.
bool DotIsOurs(NSBitmapImageRep* rep) {
  NSColor* dot = [[rep colorAtX:kDotX y:kDotY] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
  if (!dot) {
    return false;
  }
  const double r = dot.redComponent * 255, g = dot.greenComponent * 255,
               b = dot.blueComponent * 255;
  return g > r + 50 && g > b + 25;
}

// A color blend keeps macOS's shading of the dot and comes within 4–6 per channel of macOS drawing
// a dot of that color itself; a hue blend misses by 12–14.
void Recolor(NSBitmapImageRep* rep, uint32_t color) {
  [NSGraphicsContext saveGraphicsState];
  NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:rep];
  [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(kDotX - kDotRadius, kSize - kDotY - kDotRadius,
                                                     2 * kDotRadius, 2 * kDotRadius)] setClip];
  [[NSColor colorWithSRGBRed:(color >> 16 & 0xff) / 255.
                       green:(color >> 8 & 0xff) / 255.
                        blue:(color & 0xff) / 255.
                       alpha:1] setFill];
  NSRectFillUsingOperation(NSMakeRect(0, 0, kSize, kSize), NSCompositingOperationColor);
  [NSGraphicsContext restoreGraphicsState];
}

// On Queue(): decides what the Dock shows for the ask, and swaps it on the main thread if that
// changed.
void Decide(uint64_t asked, bool colored, uint32_t color) {
  if (asked != g_asked.load()) {
    return;
  }
  NSBitmapImageRep* rep = Drawn();
  NSData* drawn = [NSData dataWithBytes:rep.bitmapData length:rep.bytesPerRow * rep.pixelsHigh];
  const bool ours = DotIsOurs(rep);
  NSNumber* shown = colored && ours ? @(color) : nil;
  // A colored icon is redrawn when macOS's drawing changed under it, as from Default to Dark.
  const bool same = shown ? [shown isEqual:g_shown] && [drawn isEqualToData:g_drawn] : !g_shown;
  g_drawn = drawn;
  if (same) {
    return;
  }
  g_shown = shown;
  NSImage* image = nil;
  NSString* name = ours ? @"green" : @"system";
  if (shown) {
    Recolor(rep, color);
    image = [[NSImage alloc] initWithSize:NSMakeSize(kSize, kSize)];
    [image addRepresentation:rep];
    name = [NSString stringWithFormat:@"%06x", color];
  }
  // The next launch starts in this color, before the browser can say which Space shows.
  if (shown && g_saved) {
    [name writeToFile:g_saved atomically:YES encoding:NSUTF8StringEncoding error:nil];
  } else if (g_saved) {
    [NSFileManager.defaultManager removeItemAtPath:g_saved error:nil];
  }
  dispatch_async(dispatch_get_main_queue(), ^{
    const CFTimeInterval start = CACurrentMediaTime();
    // Nil gives the Dock back macOS's live icon, which follows the style. macOS's drawing set as
    // the icon, or the named application icon, would stay in the style it was drawn in.
    NSApp.applicationIconImage = image;
    fprintf(stderr, "browser: dock dot %s in %.1f ms\n", name.UTF8String,
            (CACurrentMediaTime() - start) * 1000);
  });
}

void Ask() {
  const uint64_t asked = ++g_asked;
  const bool colored = g_colored;
  const uint32_t color = g_color;
  dispatch_async(Queue(), ^{
    Decide(asked, colored, color);
  });
}

}  // namespace

void host_dock_dot_launch(const char* profile_dir) {
  g_saved = [NSString stringWithFormat:@"%s/dock-dot", profile_dir];
  // macOS draws the icon anew in the new style, which a colored one has to follow, and one handed
  // back to macOS has to stay handed back.
  [NSWorkspace.sharedWorkspace.notificationCenter addObserverForName:kIconStyleChanged
                                                              object:nil
                                                               queue:NSOperationQueue.mainQueue
                                                          usingBlock:^(NSNotification*) {
                                                            Ask();
                                                          }];
  NSString* saved = [NSString stringWithContentsOfFile:g_saved
                                              encoding:NSUTF8StringEncoding
                                                 error:nil];
  if (saved.length == 6) {
    host_dock_dot(true, (uint32_t)strtoul(saved.UTF8String, nullptr, 16));
  }
}

void host_dock_dot(bool colored, uint32_t color) {
  g_colored = colored;
  g_color = color;
  Ask();
}
