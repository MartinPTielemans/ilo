// A drag with the real pointer, for what macOS moves with it alone, such as Chrome's Picture in
// Picture window, which the window server moves and sizes itself and the driver's events, posted
// to the app's own windows, never reach. It posts mouse events at the HID level, as a mouse
// would, so it moves the desktop's pointer and presses on whatever is under it: it runs only in a
// GitHub Actions job (GITHUB_ACTIONS=true), never on someone's Mac, and puts the pointer back
// where it found it. Its process needs Accessibility, which check-mac.yml grants to the job's.
// Usage: pointer drag X Y X2 Y2, in points from the main display's top-left, as drive.mm's place.
// It says "dragged X Y to X2 Y2". Built with clang++ -fobjc-arc -framework Cocoa.
#import <Cocoa/Cocoa.h>

#include <unistd.h>

namespace {

void Post(CGEventType type, CGPoint at) {
  CGEventRef event = CGEventCreateMouseEvent(NULL, type, at, kCGMouseButtonLeft);
  if (type != kCGEventMouseMoved) {
    CGEventSetIntegerValueField(event, kCGMouseEventClickState, 1);
  }
  CGEventPost(kCGHIDEventTap, event);
  CFRelease(event);
}

}  // namespace

int main(int argc, char** argv) {
  @autoreleasepool {
    // Probe for #3354, never to land: "pointer move X Y X2 Y2" moves the real pointer from X Y to
    // X2 Y2 with no button down, as a hand's hover, and leaves it there; "pointer to X Y" moves it
    // straight there.
    const bool move = argc == 6 && strcmp(argv[1], "move") == 0;
    const bool to_only = argc == 4 && strcmp(argv[1], "to") == 0;
    if (!move && !to_only && (argc != 6 || strcmp(argv[1], "drag") != 0)) {
      fprintf(stderr, "usage: pointer drag X Y X2 Y2\n");
      return 2;
    }
    const char* actions = getenv("GITHUB_ACTIONS");
    if (!actions || strcmp(actions, "true") != 0) {
      fprintf(stderr, "pointer moves the real pointer, so it runs only in a GitHub Actions job\n");
      return 2;
    }
    if (!AXIsProcessTrusted()) {
      fprintf(stderr, "pointer's process hasn't Accessibility, so macOS would drop its events\n");
      return 1;
    }
    CGEventRef now = CGEventCreate(NULL);
    const CGPoint was = CGEventGetLocation(now);
    CFRelease(now);
    if (to_only) {
      Post(kCGEventMouseMoved, CGPointMake(atof(argv[2]), atof(argv[3])));
      printf("pointer to %s %s, was %.0f %.0f\n", argv[2], argv[3], was.x, was.y);
      return 0;
    }
    const CGPoint from = CGPointMake(atof(argv[2]), atof(argv[3]));
    const CGPoint to = CGPointMake(atof(argv[4]), atof(argv[5]));
    if (move) {
      Post(kCGEventMouseMoved, from);
      usleep(50000);
      const int moves = 10;
      for (int step = 1; step <= moves; step++) {
        Post(kCGEventMouseMoved, CGPointMake(from.x + (to.x - from.x) * step / moves,
                                             from.y + (to.y - from.y) * step / moves));
        usleep(16000);
      }
      printf("moved %.0f %.0f to %.0f %.0f, was %.0f %.0f\n", from.x, from.y, to.x, to.y, was.x,
             was.y);
      return 0;
    }
    // Paced as a hand is, a frame apart, so the window server sees a press, a drag and a let-go.
    Post(kCGEventMouseMoved, from);
    usleep(100000);
    Post(kCGEventLeftMouseDown, from);
    usleep(150000);
    const int steps = 15;
    for (int step = 1; step <= steps; step++) {
      Post(kCGEventLeftMouseDragged,
           CGPointMake(from.x + (to.x - from.x) * step / steps,
                       from.y + (to.y - from.y) * step / steps));
      usleep(16000);
    }
    usleep(150000);
    Post(kCGEventLeftMouseUp, to);
    usleep(150000);
    Post(kCGEventMouseMoved, was);
    printf("dragged %.0f %.0f to %.0f %.0f\n", from.x, from.y, to.x, to.y);
    return 0;
  }
}
