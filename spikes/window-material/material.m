// The window-material spike (#4087): shows one candidate material in a sidebar-sized window over
// a plain window of our own, and reads the captures back. Nothing in app/ uses it.
//
//   material serve           windows driven by lines on stdin (below), answers on stdout
//   material read <png>      the ground patch's and the glow patch's mean colour in one capture
//   material check <png> <rrggbb>   whether every pixel of a proof capture is that colour
//   material fit <dir>       the tint alpha that brings each candidate nearest its targets
//   material table <dir>     the table of grounds and contrasts, in Markdown
//
// serve's lines:
//   show <material> <tint> <dark|light> <white|grey|black|picture> <active|inactive> <plain|glow>
//     <tint> is "none" or <rrggbb>a<alpha in thousandths>, such as 141417a480.
//     <behind> may also be ring: black, with white in the outer half of the margin, to show that
//     nothing past the backdrop reaches the patch the ground is read from.
//     Answers "shown reduce <0|1> at <x>,<y>,<w>,<h>" once the window is on screen with its
//     material drawn and no other window meets the backdrop: the candidate window's frame, in
//     points from the main screen's top-left, for `screencapture -R`. Else "error: ..." naming
//     what's in the way.
//   proof <rrggbb>
//     Shows the backdrop alone, all that colour, and answers "proved at <x>,<y>,<w>,<h>": its
//     frame, for a capture `material check` reads. Every pixel the colour proves the spot shows
//     the backdrop and nothing else: no Dock, banner, notification or dialog.
//   quit
//
// The backdrop goes where no window that says by its bounds where it draws meets it (an app's,
// an alert, a desktop widget), clear of the menu bar, the Dock and the top right where banners
// come, nearest the screen's foot and left. The candidate's window sits kMargin in from its edges.
// A capture is the candidate window's frame as the screen composes it, so every patch below is in
// points from the window's top-left.

#import <AppKit/AppKit.h>
#import <ImageIO/ImageIO.h>
#import <QuartzCore/QuartzCore.h>
#include <math.h>
#include <poll.h>
#include <unistd.h>

// Macros, as C takes only constant expressions in the patches' initializers.
#define kWidth 360.0
#define kHeight 520.0
// More than the blur's reach, so what's past the backdrop never shows in the material: the ring
// backdrop checks it. Was 60.
#define kMargin 160.0
#define kBackWidth (kWidth + 2 * kMargin)
#define kBackHeight (kHeight + 2 * kMargin)
// Clear of the text, the edges and the rounded corners.
static const NSRect kGround = {{264, 240}, {80, 80}};
// At the foot's left corner, where the glow is brightest.
static const NSRect kGlow = {{20, kHeight - 60}, {40, 40}};

// The brief's targets (docs on #4087): dark's grounds over white, grey and black, and light's
// ends today. Grey has no light target.
typedef struct {
  double r, g, b;
} RGB;
static const RGB kDarkTint = {20, 20, 23}, kLightTint = {255, 255, 255};
static const RGB kDarkTarget[3] = {{63, 63, 64}, {36, 36, 38}, {10, 10, 11}};
static const RGB kLightTarget[3] = {{253, 253, 253}, {-1, -1, -1}, {231, 231, 231}};
static const RGB kPaper = {243, 240, 232}, kInk = {22, 20, 15};
static const char* kBehind[4] = {"white", "grey", "black", "picture"};

static void Fail(NSString* message) {
  fprintf(stderr, "material: %s\n", message.UTF8String);
  exit(1);
}

#pragma mark - serve

// The public materials, and the one the app uses today in dark, which AppKit doesn't name.
static BOOL MaterialNamed(NSString* name, NSVisualEffectMaterial* material) {
  NSDictionary<NSString*, NSNumber*>* named = @{
    @"underWindowBackground" : @(NSVisualEffectMaterialUnderWindowBackground),
    @"sidebar" : @(NSVisualEffectMaterialSidebar),
    @"hudWindow" : @(NSVisualEffectMaterialHUDWindow),
    @"fullScreenUI" : @(NSVisualEffectMaterialFullScreenUI),
    @"windowBackground" : @(NSVisualEffectMaterialWindowBackground),
    @"today" : @29,
  };
  NSNumber* found = named[name];
  if (!found) {
    return NO;
  }
  *material = (NSVisualEffectMaterial)found.integerValue;
  return YES;
}

// The plain window behind: one colour, or a busy picture of slanted bands and a checker.
@interface Backdrop : NSView
@property(nonatomic, copy) NSString* kind;
// A proof's colour, for the kind flat.
@property(nonatomic, strong) NSColor* flat;
@end

@implementation Backdrop
- (void)drawRect:(NSRect)dirty {
  NSRect bounds = self.bounds;
  if ([self.kind isEqual:@"flat"]) {
    [self.flat setFill];
    NSRectFill(bounds);
    return;
  }
  if ([self.kind isEqual:@"ring"]) {
    [NSColor.whiteColor setFill];
    NSRectFill(bounds);
    [NSColor.blackColor setFill];
    NSRectFill(NSInsetRect(bounds, kMargin / 2, kMargin / 2));
    return;
  }
  if ([self.kind isEqual:@"white"] || [self.kind isEqual:@"black"] || [self.kind isEqual:@"grey"]) {
    double v = [self.kind isEqual:@"white"] ? 1 : [self.kind isEqual:@"black"] ? 0 : 128 / 255.0;
    [[NSColor colorWithSRGBRed:v green:v blue:v alpha:1] setFill];
    NSRectFill(bounds);
    return;
  }
  [[NSColor colorWithSRGBRed:0.5 green:0.5 blue:0.5 alpha:1] setFill];
  NSRectFill(bounds);
  const CGFloat band = 36;
  const CGFloat reach = NSWidth(bounds) + NSHeight(bounds);
  for (int i = 0; i * band < 2 * reach; ++i) {
    double hue = fmod(i * 0.137, 1.0);
    double brightness = (i % 3 == 0) ? 0.25 : (i % 3 == 1) ? 0.95 : 0.6;
    [[NSColor colorWithHue:hue saturation:0.8 brightness:brightness alpha:1] setFill];
    CGFloat x = i * band - reach;
    NSBezierPath* path = [NSBezierPath bezierPath];
    [path moveToPoint:NSMakePoint(x, 0)];
    [path lineToPoint:NSMakePoint(x + band, 0)];
    [path lineToPoint:NSMakePoint(x + band + reach, reach)];
    [path lineToPoint:NSMakePoint(x + reach, reach)];
    [path closePath];
    [path fill];
  }
  const CGFloat square = 24;
  for (int row = 0; row * square < NSHeight(bounds); ++row) {
    for (int column = 0; column * square < NSWidth(bounds); ++column) {
      if ((row + column) % 2) {
        continue;
      }
      [[NSColor colorWithSRGBRed:(row % 2) green:(row % 2) blue:(row % 2) alpha:0.3] setFill];
      NSRectFillUsingOperation(NSMakeRect(column * square, row * square, square, square),
                               NSCompositingOperationSourceOver);
    }
  }
}
@end

// The Space's glow from the mock: a radial gradient from the foot's left corner,
// rgba(122,112,244,.36) to clear, 560 by 460 pt.
@interface Glow : NSView
@end

@implementation Glow
- (void)drawRect:(NSRect)dirty {
  CGContextRef context = NSGraphicsContext.currentContext.CGContext;
  CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
  const CGFloat components[8] = {122 / 255.0, 112 / 255.0, 244 / 255.0, 0.36,
                                 122 / 255.0, 112 / 255.0, 244 / 255.0, 0};
  const CGFloat locations[2] = {0, 1};
  CGGradientRef gradient = CGGradientCreateWithColorComponents(srgb, components, locations, 2);
  CGContextSaveGState(context);
  // Unflipped, so the origin is the foot's left corner; the circle stretched to 560 wide.
  CGContextScaleCTM(context, 560.0 / 460.0, 1);
  CGContextDrawRadialGradient(context, gradient, CGPointZero, 0, CGPointZero, 460, 0);
  CGContextRestoreGState(context);
  CGGradientRelease(gradient);
  CGColorSpaceRelease(srgb);
}
@end

static NSColor* Colour(RGB rgb, double alpha) {
  return [NSColor colorWithSRGBRed:rgb.r / 255 green:rgb.g / 255 blue:rgb.b / 255 alpha:alpha];
}

// <rrggbb>a<thousandths>, or none.
static BOOL ParseTint(NSString* text, RGB* rgb, double* alpha) {
  if ([text isEqual:@"none"]) {
    *alpha = 0;
    return YES;
  }
  unsigned hex = 0, thousandths = 0;
  if (sscanf(text.UTF8String, "%6xa%u", &hex, &thousandths) != 2 || thousandths > 1000) {
    return NO;
  }
  *rgb = (RGB){(hex >> 16) & 0xff, (hex >> 8) & 0xff, hex & 0xff};
  *alpha = thousandths / 1000.0;
  return YES;
}

static NSWindow* g_backdrop;
static NSWindow* g_window;

// A sidebar's column of text samples: the text colour at 1.0, 0.6 and 0.42, at 13 and 11 pt.
static NSView* TextColumn(BOOL light) {
  NSView* column = [[NSView alloc] initWithFrame:NSMakeRect(20, kHeight - 240, 240, 200)];
  const double steps[3] = {1.0, 0.6, 0.42};
  const CGFloat sizes[2] = {13, 11};
  CGFloat y = 200;
  for (int s = 0; s < 2; ++s) {
    for (int i = 0; i < 3; ++i) {
      y -= 30;
      NSString* words = [NSString stringWithFormat:@"%@ at %.2g · %g pt · Reading list",
                                                   light ? @"Ink" : @"Paper", steps[i], sizes[s]];
      NSTextField* label = [NSTextField labelWithString:words];
      label.font = [NSFont systemFontOfSize:sizes[s]];
      label.textColor = Colour(light ? kInk : kPaper, steps[i]);
      label.frame = NSMakeRect(0, y, 240, 20);
      [column addSubview:label];
    }
  }
  return column;
}

static NSString* Show(NSArray<NSString*>* words) {
  NSVisualEffectMaterial material;
  RGB tint = {0, 0, 0};
  double alpha = 0;
  if (words.count != 7 || !MaterialNamed(words[1], &material) ||
      !ParseTint(words[2], &tint, &alpha)) {
    return @"error: show <material> <tint> <dark|light> <behind> <active|inactive> <plain|glow>";
  }
  BOOL light = [words[3] isEqual:@"light"];
  BOOL active = [words[5] isEqual:@"active"];
  BOOL glow = [words[6] isEqual:@"glow"];
  NSString* behind = words[4];
  if (![@[ @"white", @"grey", @"black", @"picture", @"ring" ] containsObject:behind]) {
    return @"error: behind is white, grey, black, picture or ring";
  }

  ((Backdrop*)g_backdrop.contentView).kind = behind;
  g_backdrop.contentView.needsDisplay = YES;

  NSAppearance* appearance =
      [NSAppearance appearanceNamed:light ? NSAppearanceNameAqua : NSAppearanceNameDarkAqua];
  g_window.appearance = appearance;
  NSRect bounds = NSMakeRect(0, 0, kWidth, kHeight);
  // As the app's WindowMaterial makes it, but with its state held: a window of a process started
  // from a script is seldom the key one, and following that would draw every capture inactive.
  NSVisualEffectView* effect = [[NSVisualEffectView alloc] initWithFrame:bounds];
  effect.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  effect.blendingMode = NSVisualEffectBlendingModeBehindWindow;
  effect.state = active ? NSVisualEffectStateActive : NSVisualEffectStateInactive;
  effect.emphasized = YES;
  effect.material = material;
  effect.appearance = appearance;

  // The tint the UI draws over the material.
  NSView* tinted = [[NSView alloc] initWithFrame:bounds];
  tinted.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
  tinted.wantsLayer = YES;
  tinted.layer.backgroundColor = Colour(tint, alpha).CGColor;
  [effect addSubview:tinted];
  if (glow) {
    Glow* glowing = [[Glow alloc] initWithFrame:bounds];
    glowing.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    [effect addSubview:glowing];
  }
  [effect addSubview:TextColumn(light)];
  g_window.contentView = effect;

  [g_backdrop orderFrontRegardless];
  [g_window orderFrontRegardless];
  [g_backdrop displayIfNeeded];
  [g_window displayIfNeeded];
  return nil;
}

// |frame|, in AppKit's screen coordinates, in the window list's: points from the main screen's
// top-left.
static CGRect ListRect(NSRect frame) {
  CGFloat top = NSMaxY(NSScreen.screens.firstObject.frame);
  return CGRectMake(NSMinX(frame), top - NSMaxY(frame), NSWidth(frame), NSHeight(frame));
}

// Other processes' windows on screen that say by their bounds where they draw: apps' windows,
// alerts and desktop widgets. The Dock's, Notification Center's and the window server's cover the
// whole screen, see-through, as the wallpaper's and the desktop icons' do, and the cursor is above
// them all: their bounds say nothing, so they're left to the pixel proof.
static NSArray<NSDictionary*>* OthersOnScreen(void) {
  NSArray<NSDictionary*>* list = CFBridgingRelease(
      CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID));
  CGRect screen = NSRectToCGRect(NSScreen.screens.firstObject.frame);
  screen.origin = CGPointZero;
  const int dock = CGWindowLevelForKey(kCGDockWindowLevelKey);
  NSMutableArray<NSDictionary*>* others = [NSMutableArray array];
  for (NSDictionary* info in list) {
    CFDictionaryRef frame = (__bridge CFDictionaryRef)info[(__bridge id)kCGWindowBounds];
    CGRect bounds;
    if ([info[(__bridge id)kCGWindowOwnerPID] intValue] == getpid() ||
        [info[(__bridge id)kCGWindowAlpha] doubleValue] <= 0 ||
        [info[(__bridge id)kCGWindowLayer] intValue] >= dock || !frame ||
        !CGRectMakeWithDictionaryRepresentation(frame, &bounds) ||
        CGRectContainsRect(bounds, screen)) {
      continue;
    }
    [others addObject:info];
  }
  return others;
}

// The names of those of |others| that meet |area|, in the window list's coordinates, each grown by
// |clear| on every side: a window's bounds leave out its shadow.
static NSArray<NSString*>* InTheWay(NSArray<NSDictionary*>* others, CGRect area, CGFloat clear) {
  NSMutableArray<NSString*>* names = [NSMutableArray array];
  for (NSDictionary* info in others) {
    CFDictionaryRef frame = (__bridge CFDictionaryRef)info[(__bridge id)kCGWindowBounds];
    CGRect bounds;
    if (!frame || !CGRectMakeWithDictionaryRepresentation(frame, &bounds) ||
        CGRectIsEmpty(CGRectIntersection(CGRectInset(bounds, -clear, -clear), area))) {
      continue;
    }
    NSString* owner = info[(__bridge id)kCGWindowOwnerName] ?: @"a process";
    NSString* title = info[(__bridge id)kCGWindowName];
    [names addObject:title.length ? [NSString stringWithFormat:@"%@ (“%@”)", owner, title] : owner];
  }
  return names;
}

// Where the backdrop goes: the first spot on the main screen, clear of the menu bar, the Dock and
// the top right's 400 by 200 pt, where a banner may slide in mid-run, that none of OthersOnScreen
// comes within 64 pt of, so their shadows fall short of it too, from the foot and the left. Else
// none, and |error| names what's in the way of the screen's middle.
static BOOL PlaceBackdrop(NSRect* back, NSString** error) {
  NSRect screen = NSScreen.screens.firstObject.visibleFrame;
  const CGRect banners =
      CGRectMake(NSWidth(NSScreen.screens.firstObject.frame) - 400, 0, 400, 200);
  NSArray<NSDictionary*>* others = OthersOnScreen();
  for (CGFloat x = ceil(NSMinX(screen)); x + kBackWidth <= NSMaxX(screen); x += 10) {
    for (CGFloat y = ceil(NSMinY(screen)); y + kBackHeight <= NSMaxY(screen); y += 10) {
      NSRect spot = NSMakeRect(x, y, kBackWidth, kBackHeight);
      if (!CGRectIntersectsRect(ListRect(spot), banners) &&
          !InTheWay(others, ListRect(spot), 64).count) {
        *back = spot;
        return YES;
      }
    }
  }
  NSRect middle = NSMakeRect(round(NSMidX(screen) - kBackWidth / 2),
                             round(NSMidY(screen) - kBackHeight / 2), kBackWidth, kBackHeight);
  *error = [NSString
      stringWithFormat:@"no spot %g by %g pt on the screen is clear of other windows; in the "
                       @"middle: %@",
                       kBackWidth, kBackHeight,
                       [InTheWay(others, ListRect(middle), 64) componentsJoinedByString:@", "]];
  return NO;
}

static NSString* MakeWindows(void) {
  NSRect back;
  NSString* error;
  if (!PlaceBackdrop(&back, &error)) {
    return error;
  }
  g_backdrop = [[NSWindow alloc] initWithContentRect:back
                                           styleMask:NSWindowStyleMaskBorderless
                                             backing:NSBackingStoreBuffered
                                               defer:NO];
  g_backdrop.releasedWhenClosed = NO;
  g_backdrop.hasShadow = NO;
  g_backdrop.opaque = YES;
  Backdrop* backdrop = [[Backdrop alloc] initWithFrame:NSMakeRect(0, 0, kBackWidth, kBackHeight)];
  backdrop.kind = @"grey";
  g_backdrop.contentView = backdrop;

  // As the browser window: titled, its content the full window, its title bar clear and empty.
  g_window = [[NSWindow alloc]
      initWithContentRect:NSInsetRect(back, kMargin, kMargin)
                styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                          NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable |
                          NSWindowStyleMaskFullSizeContentView
                  backing:NSBackingStoreBuffered
                    defer:NO];
  g_window.releasedWhenClosed = NO;
  g_window.titlebarAppearsTransparent = YES;
  g_window.titleVisibility = NSWindowTitleHidden;
  g_window.title = @"Window material";
  [g_window setFrame:NSInsetRect(back, kMargin, kMargin) display:NO];
  return nil;
}

// Runs the app's events for |seconds|, or until |until| says so.
static BOOL Pump(double seconds, BOOL (^until)(void)) {
  NSDate* end = [NSDate dateWithTimeIntervalSinceNow:seconds];
  while (end.timeIntervalSinceNow > 0) {
    if (until && until()) {
      return YES;
    }
    NSEvent* event = [NSApp nextEventMatchingMask:NSEventMaskAny
                                        untilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]
                                           inMode:NSDefaultRunLoopMode
                                          dequeue:YES];
    if (event) {
      [NSApp sendEvent:event];
    }
  }
  return until ? until() : YES;
}

static int Serve(void) {
  [NSApplication sharedApplication];
  [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
  [NSApp finishLaunching];
  NSString* error = MakeWindows();
  if (error) {
    printf("error: %s\n", error.UTF8String);
    fflush(stdout);
    return 1;
  }
  int reduce = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency;
  printf("serving reduce %d\n", reduce);
  fflush(stdout);

  NSMutableData* pending = [NSMutableData data];
  for (;;) {
    struct pollfd input = {STDIN_FILENO, POLLIN, 0};
    if (poll(&input, 1, 0) <= 0) {
      Pump(0.02, nil);
      continue;
    }
    char buffer[1024];
    ssize_t got = read(STDIN_FILENO, buffer, sizeof buffer);
    if (got <= 0) {
      return 0;
    }
    [pending appendBytes:buffer length:(NSUInteger)got];
    for (;;) {
      const char* bytes = pending.bytes;
      const char* newline = memchr(bytes, '\n', pending.length);
      if (!newline) {
        break;
      }
      NSUInteger length = (NSUInteger)(newline - bytes);
      NSString* line = [[NSString alloc] initWithBytes:bytes
                                                length:length
                                              encoding:NSUTF8StringEncoding];
      [pending replaceBytesInRange:NSMakeRange(0, length + 1) withBytes:NULL length:0];
      NSArray<NSString*>* words =
          [[line stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceCharacterSet]
              componentsSeparatedByCharactersInSet:NSCharacterSet.whitespaceCharacterSet];
      if ([words.firstObject isEqual:@"quit"]) {
        return 0;
      }
      if ([words.firstObject isEqual:@"proof"]) {
        unsigned hex = 0;
        if (words.count != 2 || sscanf(words[1].UTF8String, "%6x", &hex) != 1) {
          printf("error: proof <rrggbb>\n");
          fflush(stdout);
          continue;
        }
        [g_window orderOut:nil];
        Backdrop* backdrop = (Backdrop*)g_backdrop.contentView;
        backdrop.kind = @"flat";
        backdrop.flat = Colour((RGB){(hex >> 16) & 0xff, (hex >> 8) & 0xff, hex & 0xff}, 1);
        backdrop.needsDisplay = YES;
        [g_backdrop orderFrontRegardless];
        [g_backdrop displayIfNeeded];
        BOOL visible = Pump(5, ^{
          return (BOOL)((g_backdrop.occlusionState & NSWindowOcclusionStateVisible) != 0);
        });
        Pump(0.6, nil);
        CGRect back = ListRect(g_backdrop.frame);
        if (visible) {
          printf("proved at %.0f,%.0f,%.0f,%.0f\n", back.origin.x, back.origin.y,
                 back.size.width, back.size.height);
        } else {
          printf("error: the backdrop isn't visible\n");
        }
        fflush(stdout);
        continue;
      }
      if (![words.firstObject isEqual:@"show"]) {
        printf("error: show, proof or quit\n");
        fflush(stdout);
        continue;
      }
      NSString* error = Show(words);
      if (error) {
        printf("%s\n", error.UTF8String);
        fflush(stdout);
        continue;
      }
      // On screen and uncovered, then a moment for the window server's blur of what's behind,
      // which it draws in frames of its own after the window's.
      BOOL visible = Pump(5, ^{
        return (BOOL)((g_window.occlusionState & NSWindowOcclusionStateVisible) != 0);
      });
      Pump(0.6, nil);
      reduce = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency;
      // The capture is the screen as composed, so another window over the backdrop, or under the
      // material, would be in it: a dialog, a banner, an app that came forward.
      NSArray<NSString*>* inWay = InTheWay(OthersOnScreen(), ListRect(g_backdrop.frame), 0);
      CGRect window = ListRect(g_window.frame);
      if (!visible) {
        printf("error: the window isn't visible\n");
      } else if (inWay.count) {
        printf("error: in the way of the backdrop: %s\n",
               [inWay componentsJoinedByString:@", "].UTF8String);
      } else {
        printf("shown reduce %d at %.0f,%.0f,%.0f,%.0f\n", reduce, window.origin.x,
               window.origin.y, window.size.width, window.size.height);
      }
      fflush(stdout);
    }
  }
}

#pragma mark - reading

typedef struct {
  RGB mean;
  double spread;  // the largest channel's standard deviation, to tell a clean patch
} Patch;

// A capture, in sRGB whatever the display's colour space, 8 bits a channel, top row first.
typedef struct {
  NSMutableData* pixels;
  size_t width, height;
} Image;

static BOOL Load(NSString* path, Image* image) {
  CGImageSourceRef source =
      CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:path], NULL);
  if (!source) {
    return NO;
  }
  CGImageRef picture = CGImageSourceCreateImageAtIndex(source, 0, NULL);
  CFRelease(source);
  if (!picture) {
    return NO;
  }
  image->width = CGImageGetWidth(picture);
  image->height = CGImageGetHeight(picture);
  image->pixels = [NSMutableData dataWithLength:image->width * image->height * 4];
  CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
  CGContextRef context =
      CGBitmapContextCreate(image->pixels.mutableBytes, image->width, image->height, 8,
                            image->width * 4, srgb, kCGImageAlphaPremultipliedLast);
  CGContextDrawImage(context, CGRectMake(0, 0, image->width, image->height), picture);
  CGContextRelease(context);
  CGColorSpaceRelease(srgb);
  CGImageRelease(picture);
  return YES;
}

static Patch Read(const Image* image, NSRect rect) {
  double scale = image->width / kWidth;
  size_t x0 = (size_t)round(NSMinX(rect) * scale), y0 = (size_t)round(NSMinY(rect) * scale);
  size_t x1 = (size_t)round(NSMaxX(rect) * scale), y1 = (size_t)round(NSMaxY(rect) * scale);
  const uint8_t* bytes = image->pixels.bytes;
  double sum[3] = {0, 0, 0}, squares[3] = {0, 0, 0};
  double n = 0;
  for (size_t y = y0; y < y1 && y < image->height; ++y) {
    for (size_t x = x0; x < x1 && x < image->width; ++x) {
      const uint8_t* p = bytes + (y * image->width + x) * 4;
      for (int c = 0; c < 3; ++c) {
        sum[c] += p[c];
        squares[c] += (double)p[c] * p[c];
      }
      ++n;
    }
  }
  Patch patch = {{0, 0, 0}, 0};
  if (n == 0) {
    return patch;
  }
  double mean[3];
  for (int c = 0; c < 3; ++c) {
    mean[c] = sum[c] / n;
    patch.spread = fmax(patch.spread, sqrt(fmax(0, squares[c] / n - mean[c] * mean[c])));
  }
  patch.mean = (RGB){mean[0], mean[1], mean[2]};
  return patch;
}

static BOOL ReadFile(NSString* path, Patch* ground, Patch* glow) {
  Image image;
  if (!Load(path, &image)) {
    return NO;
  }
  // The capture is the candidate window's frame; any other shape means it took something else.
  double ratio = (double)image.width / image.height;
  if (fabs(ratio - kWidth / kHeight) > 0.01) {
    fprintf(stderr, "material: %s is %zux%zu, not the window's shape\n", path.UTF8String,
            image.width, image.height);
    return NO;
  }
  *ground = Read(&image, kGround);
  if (glow) {
    *glow = Read(&image, kGlow);
  }
  return YES;
}

// Whether every pixel of |path|, a capture of the backdrop's frame, is |hex| give or take 2 a
// channel, the most a display's colour space moves a flat sRGB colour. Else it prints the
// rectangle the other pixels are in, in points from the backdrop's top-left, and the first one.
static int Check(NSString* path, NSString* hex) {
  unsigned want = 0;
  Image image;
  if (sscanf(hex.UTF8String, "%6x", &want) != 1 || !Load(path, &image)) {
    fprintf(stderr, "material: can't check %s\n", path.UTF8String);
    return 2;
  }
  const int target[3] = {(want >> 16) & 0xff, (want >> 8) & 0xff, want & 0xff};
  const uint8_t* bytes = image.pixels.bytes;
  size_t off = 0, left = SIZE_MAX, top = SIZE_MAX, right = 0, bottom = 0;
  size_t first_x = 0, first_y = 0;
  for (size_t y = 0; y < image.height; ++y) {
    for (size_t x = 0; x < image.width; ++x) {
      const uint8_t* p = bytes + (y * image.width + x) * 4;
      if (abs(p[0] - target[0]) <= 2 && abs(p[1] - target[1]) <= 2 && abs(p[2] - target[2]) <= 2) {
        continue;
      }
      if (!off++) {
        first_x = x;
        first_y = y;
      }
      left = MIN(left, x);
      top = MIN(top, y);
      right = MAX(right, x + 1);
      bottom = MAX(bottom, y + 1);
    }
  }
  if (!off) {
    printf("clear: every pixel #%06x\n", want);
    return 0;
  }
  const double scale = image.width / kBackWidth;
  const uint8_t* p = bytes + (first_y * image.width + first_x) * 4;
  printf("not clear: %zu pixels aren't #%06x, in %.0f,%.0f,%.0f,%.0f pt from the backdrop's "
         "top-left; the first is #%02x%02x%02x at %.0f,%.0f\n",
         off, want, left / scale, top / scale, (right - left) / scale, (bottom - top) / scale, p[0],
         p[1], p[2], first_x / scale, first_y / scale);
  return 1;
}

static NSString* Hex(RGB rgb) {
  return [NSString
      stringWithFormat:@"#%02x%02x%02x", (int)round(rgb.r), (int)round(rgb.g), (int)round(rgb.b)];
}

static double Linear(double channel) {
  double c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}

static double Luminance(RGB rgb) {
  return 0.2126 * Linear(rgb.r) + 0.7152 * Linear(rgb.g) + 0.0722 * Linear(rgb.b);
}

// Text at |alpha| over |ground|, blended as the UI blends it, against the ground: WCAG's ratio.
static double Contrast(RGB text, double alpha, RGB ground) {
  RGB over = {alpha * text.r + (1 - alpha) * ground.r, alpha * text.g + (1 - alpha) * ground.g,
              alpha * text.b + (1 - alpha) * ground.b};
  double a = Luminance(over), b = Luminance(ground);
  return (fmax(a, b) + 0.05) / (fmin(a, b) + 0.05);
}

#pragma mark - fit and table

// One shot's file name: <material>-<tint>-<appearance>-<behind>[-<extra>].png.
typedef struct {
  NSString *material, *tint, *appearance, *behind, *extra;
} Name;

static BOOL ParseName(NSString* file, Name* name) {
  if (![file.pathExtension isEqual:@"png"]) {
    return NO;
  }
  NSArray<NSString*>* parts = [file.stringByDeletingPathExtension componentsSeparatedByString:@"-"];
  if (parts.count < 4 || parts.count > 5) {
    return NO;
  }
  *name = (Name){parts[0], parts[1], parts[2], parts[3], parts.count == 5 ? parts[4] : @""};
  return YES;
}

static int BehindIndex(NSString* behind) {
  for (int i = 0; i < 4; ++i) {
    if ([behind isEqual:@(kBehind[i])]) {
      return i;
    }
  }
  return -1;
}

// Every plain shot's ground, by "<material>-<tint>-<appearance>" and then by backdrop.
static NSDictionary<NSString*, NSMutableDictionary<NSNumber*, NSValue*>*>* Grounds(NSString* dir) {
  NSMutableDictionary* grounds = [NSMutableDictionary dictionary];
  NSArray<NSString*>* files = [[NSFileManager.defaultManager contentsOfDirectoryAtPath:dir
                                                                                 error:nil]
      sortedArrayUsingSelector:@selector(compare:)];
  for (NSString* file in files) {
    Name name;
    if (!ParseName(file, &name) || name.extra.length || BehindIndex(name.behind) < 0) {
      continue;
    }
    Patch ground;
    if (!ReadFile([dir stringByAppendingPathComponent:file], &ground, NULL)) {
      continue;
    }
    NSString* key = [@[ name.material, name.tint, name.appearance ] componentsJoinedByString:@"-"];
    if (!grounds[key]) {
      grounds[key] = [NSMutableDictionary dictionary];
    }
    grounds[key][@(BehindIndex(name.behind))] = [NSValue valueWithBytes:&ground
                                                               objCType:@encode(Patch)];
  }
  return grounds;
}

static Patch PatchOf(NSValue* value) {
  Patch patch;
  [value getValue:&patch size:sizeof patch];
  return patch;
}

// The alpha a of |tint| over the untinted grounds G nearest the targets T, least squares over
// the channels and backdrops that have a target: a = Σ(C−G)(T−G) / Σ(C−G)².
static double Fit(NSDictionary<NSNumber*, NSValue*>* byBehind, BOOL light, double* worst) {
  RGB tint = light ? kLightTint : kDarkTint;
  const RGB* targets = light ? kLightTarget : kDarkTarget;
  double top = 0, bottom = 0;
  for (int i = 0; i < 3; ++i) {
    if (targets[i].r < 0 || !byBehind[@(i)]) {
      continue;
    }
    RGB g = PatchOf(byBehind[@(i)]).mean;
    const double c[3] = {tint.r, tint.g, tint.b}, gg[3] = {g.r, g.g, g.b},
                 t[3] = {targets[i].r, targets[i].g, targets[i].b};
    for (int k = 0; k < 3; ++k) {
      top += (c[k] - gg[k]) * (t[k] - gg[k]);
      bottom += (c[k] - gg[k]) * (c[k] - gg[k]);
    }
  }
  double a = bottom > 0 ? fmin(1, fmax(0, top / bottom)) : 0;
  // How far the tinted grounds would still be from their targets, the worst channel.
  *worst = 0;
  for (int i = 0; i < 3; ++i) {
    if (targets[i].r < 0 || !byBehind[@(i)]) {
      continue;
    }
    RGB g = PatchOf(byBehind[@(i)]).mean;
    *worst = fmax(*worst, fabs(a * tint.r + (1 - a) * g.r - targets[i].r));
    *worst = fmax(*worst, fabs(a * tint.g + (1 - a) * g.g - targets[i].g));
    *worst = fmax(*worst, fabs(a * tint.b + (1 - a) * g.b - targets[i].b));
  }
  return a;
}

static int FitAll(NSString* dir) {
  NSDictionary* grounds = Grounds(dir);
  for (NSString* key in [grounds.allKeys sortedArrayUsingSelector:@selector(compare:)]) {
    NSArray<NSString*>* parts = [key componentsSeparatedByString:@"-"];
    if (![parts[1] isEqual:@"none"]) {
      continue;
    }
    BOOL light = [parts[2] isEqual:@"light"];
    double worst = 0;
    double a = Fit(grounds[key], light, &worst);
    RGB tint = light ? kLightTint : kDarkTint;
    printf("%s %s %02x%02x%02xa%03d %.1f\n", parts[0].UTF8String, parts[2].UTF8String, (int)tint.r,
           (int)tint.g, (int)tint.b, (int)round(a * 1000), worst);
  }
  return 0;
}

// The worst channel's distance from the targets, over the backdrops that have one.
static double Off(NSDictionary<NSNumber*, NSValue*>* byBehind, BOOL light) {
  const RGB* targets = light ? kLightTarget : kDarkTarget;
  double worst = -1;
  for (int i = 0; i < 3; ++i) {
    if (targets[i].r < 0 || !byBehind[@(i)]) {
      continue;
    }
    RGB g = PatchOf(byBehind[@(i)]).mean;
    worst = fmax(worst, fmax(fabs(g.r - targets[i].r),
                             fmax(fabs(g.g - targets[i].g), fabs(g.b - targets[i].b))));
  }
  return worst;
}

static NSString* TintWords(NSString* tint) {
  if ([tint isEqual:@"none"]) {
    return @"none";
  }
  unsigned hex = 0, thousandths = 0;
  sscanf(tint.UTF8String, "%6xa%u", &hex, &thousandths);
  return [NSString stringWithFormat:@"#%06x at %.3f", hex, thousandths / 1000.0];
}

static int Table(NSString* dir) {
  NSDictionary* grounds = Grounds(dir);
  printf(
      "Ground: the mean of an 80 pt patch clear of the text, as captured (sRGB). Contrast: WCAG's "
      "ratio of the text colour at each step, blended over the ground, against it, over the "
      "backdrop where it's lowest. Off: the worst channel's distance from the targets (dark: "
      "#3f3f40, #242426, #0a0a0b over white, grey, black; light: #fdfdfd, #e7e7e7 over white, "
      "black). A ground whose patch varies (spread over 4) is marked *.\n\n");
  for (int light = 0; light < 2; ++light) {
    printf("### %s\n\n", light ? "Light (Ink 0x16140f)" : "Dark (Paper 0xf3f0e8)");
    printf("| material | tint | over white | over grey | over black | over picture | off | "
           "1.0 | 0.6 | 0.42 |\n");
    printf("|---|---|---|---|---|---|---|---|---|---|\n");
    for (NSString* key in [grounds.allKeys sortedArrayUsingSelector:@selector(compare:)]) {
      NSArray<NSString*>* parts = [key componentsSeparatedByString:@"-"];
      if ([parts[2] isEqual:@"light"] != (BOOL)light) {
        continue;
      }
      NSDictionary<NSNumber*, NSValue*>* byBehind = grounds[key];
      NSMutableString* row =
          [NSMutableString stringWithFormat:@"| %@ | %@ |", parts[0], TintWords(parts[1])];
      double lowest[3] = {INFINITY, INFINITY, INFINITY};
      const double steps[3] = {1.0, 0.6, 0.42};
      for (int i = 0; i < 4; ++i) {
        NSValue* value = byBehind[@(i)];
        if (!value) {
          [row appendString:@" – |"];
          continue;
        }
        Patch patch = PatchOf(value);
        [row appendFormat:@" %@%@ |", Hex(patch.mean), patch.spread > 4 ? @"*" : @""];
        for (int s = 0; s < 3; ++s) {
          lowest[s] = fmin(lowest[s], Contrast(light ? kInk : kPaper, steps[s], patch.mean));
        }
      }
      double off = Off(byBehind, light);
      [row appendString:off < 0 ? @" – |" : [NSString stringWithFormat:@" %.0f |", off]];
      for (int s = 0; s < 3; ++s) {
        [row appendFormat:@" %.2f |", lowest[s]];
      }
      printf("%s\n", row.UTF8String);
    }
    printf("\n");
  }

  // The shots taken once per chosen candidate: inactive, Reduce Transparency, and the glow.
  printf("### Once per chosen candidate\n\n");
  printf("| shot | ground | glow corner | contrast at 0.6 |\n|---|---|---|---|\n");
  NSArray<NSString*>* files = [[NSFileManager.defaultManager contentsOfDirectoryAtPath:dir
                                                                                 error:nil]
      sortedArrayUsingSelector:@selector(compare:)];
  for (NSString* file in files) {
    Name name;
    // The ring's too, which has no column of its own.
    if (!ParseName(file, &name) || (!name.extra.length && BehindIndex(name.behind) >= 0)) {
      continue;
    }
    Patch ground, glow;
    if (!ReadFile([dir stringByAppendingPathComponent:file], &ground, &glow)) {
      continue;
    }
    BOOL light = [name.appearance isEqual:@"light"];
    printf("| %s | %s | %s | %.2f |\n", file.stringByDeletingPathExtension.UTF8String,
           Hex(ground.mean).UTF8String,
           [name.extra isEqual:@"glow"] ? Hex(glow.mean).UTF8String : "–",
           Contrast(light ? kInk : kPaper, 0.6, ground.mean));
  }
  return 0;
}

int main(int argc, const char* argv[]) {
  @autoreleasepool {
    NSString* command = argc > 1 ? @(argv[1]) : @"";
    if ([command isEqual:@"serve"] && argc == 2) {
      return Serve();
    }
    if ([command isEqual:@"read"] && argc == 3) {
      Patch ground, glow;
      if (!ReadFile(@(argv[2]), &ground, &glow)) {
        Fail([NSString stringWithFormat:@"can't read %s", argv[2]]);
      }
      printf("ground %s spread %.1f glow %s\n", Hex(ground.mean).UTF8String, ground.spread,
             Hex(glow.mean).UTF8String);
      return 0;
    }
    if ([command isEqual:@"check"] && argc == 4) {
      return Check(@(argv[2]), @(argv[3]));
    }
    if ([command isEqual:@"fit"] && argc == 3) {
      return FitAll(@(argv[2]));
    }
    if ([command isEqual:@"table"] && argc == 3) {
      return Table(@(argv[2]));
    }
    fprintf(stderr, "usage: material serve | read <png> | check <png> <rrggbb> | fit <dir> | "
                    "table <dir>\n");
    return 2;
  }
}
