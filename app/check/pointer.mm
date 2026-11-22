// The real pointer, for what the window server decides itself and the driver's events, posted to
// the app's own windows, never reach: which window a click at a point goes to (#3611), and the
// moves Chrome's Picture in Picture window is dragged and sized by. It posts mouse events at the
// HID level, as a mouse would, so it moves the desktop's pointer and presses on whatever is under
// it: it runs only in a GitHub Actions job (GITHUB_ACTIONS=true) or the check VM (ILO_CHECK_VM=1),
// and only in a virtual machine (kern.hv_vmm_present 1), so a stray variable on someone's Mac posts
// nothing. Its process needs Accessibility, which check-mac.yml grants to the job's.
// Usage, in points from the main display's top-left, as drive.mm's place:
//   pointer drag X Y X2 Y2   presses at (X, Y), drags to (X2, Y2), lets go, and puts the pointer
//                            back where it found it. It says "dragged X Y to X2 Y2".
//   pointer click X Y PID    presses and lets go at (X, Y), with no move there first, so the
//                            window server picks the window the press goes to, and leaves the
//                            pointer there. It says "clicked X Y".
//   pointer move X Y PID     moves the pointer to (X, Y), as a mouse's move. It says "moved X Y"
//                            once the pointer is there.
//   pointer windows PID      lists the windows of process PID on screen that can be seen, front
//                            first, a line each: "window N level L at X Y W H". It posts nothing.
// click and move post nothing unless the app of process PID is the frontmost one and no other
// app's window is over the point, each checked right before each event they post.
// Built with clang++ -fobjc-arc -framework Cocoa.
#import <Cocoa/Cocoa.h>

#include <math.h>
#include <sys/sysctl.h>
#include <unistd.h>

namespace {

// Whether macOS runs in a virtual machine, as the hosted runners and the check VM do, and a Mac
// someone works on doesn't.
bool InVirtualMachine() {
  int present = 0;
  size_t size = sizeof(present);
  return sysctlbyname("kern.hv_vmm_present", &present, &size, nullptr, 0) == 0 && present == 1;
}

void Post(CGEventType type, CGPoint at) {
  CGEventRef event = CGEventCreateMouseEvent(NULL, type, at, kCGMouseButtonLeft);
  if (type != kCGEventMouseMoved) {
    CGEventSetIntegerValueField(event, kCGMouseEventClickState, 1);
  }
  CGEventPost(kCGHIDEventTap, event);
  CFRelease(event);
}

// Where the desktop's pointer is, in points from the main display's top-left.
CGPoint Now() {
  CGEventRef now = CGEventCreate(NULL);
  const CGPoint at = CGEventGetLocation(now);
  CFRelease(now);
  return at;
}

// Waits up to a second for the pointer to be at |at|, as a posted event moves it some time after
// CGEventPost returns, and one posted as the process exits can be lost. It answers where it is if
// it never gets there, or nil.
NSString* Reached(CGPoint at) {
  CGPoint now = Now();
  for (int i = 0; i < 100 && (fabs(now.x - at.x) > 0.5 || fabs(now.y - at.y) > 0.5); ++i) {
    usleep(10000);
    now = Now();
  }
  if (fabs(now.x - at.x) <= 0.5 && fabs(now.y - at.y) <= 0.5) {
    return nil;
  }
  return [NSString stringWithFormat:@"the pointer stayed at %.0f %.0f", now.x, now.y];
}

// Waits up to a second for the left button to be up, so a let-go posted last isn't lost with the
// process, which would leave the button held for the next check.
void Released() {
  for (int i = 0;
       i < 100 && CGEventSourceButtonState(kCGEventSourceStateHIDSystemState, kCGMouseButtonLeft);
       ++i) {
    usleep(10000);
  }
}

// Whether |bounds| spans one of the displays, its menu bar's strip aside.
bool AsBigAsAScreen(CGRect bounds) {
  for (NSScreen* screen in NSScreen.screens) {
    const NSSize size = screen.frame.size;
    if (bounds.size.width == size.width && bounds.size.height >= size.height - 50) {
      return true;
    }
  }
  return false;
}

// Why the pointer mustn't go to |at| for |pid|'s app now, or nil: another app is in front, or
// another app's window is over the point, such as a notification's.
NSString* Refusal(pid_t pid, CGPoint at) {
  NSRunningApplication* front = NSWorkspace.sharedWorkspace.frontmostApplication;
  if (front.processIdentifier != pid) {
    return [NSString stringWithFormat:@"%@ (process %d) is in front, not process %d",
                                      front.localizedName, front.processIdentifier, pid];
  }
  NSArray* windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
      kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
  for (NSDictionary* window in windows) {
    CGRect bounds;
    if (!CGRectMakeWithDictionaryRepresentation(
            (__bridge CFDictionaryRef)window[(__bridge NSString*)kCGWindowBounds], &bounds) ||
        !CGRectContainsPoint(bounds, at) ||
        [window[(__bridge NSString*)kCGWindowAlpha] doubleValue] == 0) {
      continue;
    }
    // The Dock's window as big as a screen, over every app's, lets their clicks through, and so
    // does Notification Center's, which macOS 26 keeps over a display in the check VM. A smaller
    // window of Notification Center's is a banner, which still refuses. A banner drawn inside the
    // screen-wide window can't be told from this list, as it has no window of its own.
    NSString* name = window[(__bridge NSString*)kCGWindowOwnerName];
    if (([name isEqualToString:@"Dock"] || [name isEqualToString:@"Notification Center"]) &&
        AsBigAsAScreen(bounds)) {
      continue;
    }
    // Front to back: the first window there that's seen decides.
    const pid_t owner = [window[(__bridge NSString*)kCGWindowOwnerPID] intValue];
    if (owner == pid) {
      return nil;
    }
    return [NSString stringWithFormat:@"%@'s window, level %@ at %.0f,%.0f %.0fx%.0f, is over "
                                      @"%.0f %.0f",
                                      window[(__bridge NSString*)kCGWindowOwnerName],
                                      window[(__bridge NSString*)kCGWindowLayer], bounds.origin.x,
                                      bounds.origin.y, bounds.size.width, bounds.size.height, at.x,
                                      at.y];
  }
  return [NSString stringWithFormat:@"no window of process %d is at %.0f %.0f", pid, at.x, at.y];
}

// What another app sees of |pid|'s windows: those on screen with any alpha, front first.
void ListWindows(pid_t pid) {
  NSArray* windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
      kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
  for (NSDictionary* window in windows) {
    CGRect bounds;
    if ([window[(__bridge NSString*)kCGWindowOwnerPID] intValue] != pid ||
        [window[(__bridge NSString*)kCGWindowAlpha] doubleValue] == 0 ||
        !CGRectMakeWithDictionaryRepresentation(
            (__bridge CFDictionaryRef)window[(__bridge NSString*)kCGWindowBounds], &bounds)) {
      continue;
    }
    printf("window %d level %d at %.0f %.0f %.0f %.0f\n",
           [window[(__bridge NSString*)kCGWindowNumber] intValue],
           [window[(__bridge NSString*)kCGWindowLayer] intValue], bounds.origin.x,
           bounds.origin.y, bounds.size.width, bounds.size.height);
  }
}

}  // namespace

int main(int argc, char** argv) {
  @autoreleasepool {
    const bool drag = argc == 6 && strcmp(argv[1], "drag") == 0;
    const bool click = argc == 5 && strcmp(argv[1], "click") == 0;
    const bool move = argc == 5 && strcmp(argv[1], "move") == 0;
    if (argc == 3 && strcmp(argv[1], "windows") == 0) {
      ListWindows(atoi(argv[2]));
      return 0;
    }
    if (!drag && !click && !move) {
      fprintf(stderr,
              "usage: pointer drag X Y X2 Y2 | click X Y PID | move X Y PID | windows PID\n");
      return 2;
    }
    const char* actions = getenv("GITHUB_ACTIONS");
    const char* vm = getenv("ILO_CHECK_VM");
    if (!(actions && strcmp(actions, "true") == 0) && !(vm && strcmp(vm, "1") == 0)) {
      fprintf(stderr,
              "pointer moves the real pointer, so it runs only in a GitHub Actions job or the "
              "check VM\n");
      return 2;
    }
    if (!InVirtualMachine()) {
      fprintf(stderr,
              "pointer moves the real pointer, so it runs only in a virtual machine, and "
              "kern.hv_vmm_present isn't 1 here\n");
      return 2;
    }
    if (!AXIsProcessTrusted()) {
      fprintf(stderr, "pointer's process hasn't Accessibility, so macOS would drop its events\n");
      return 1;
    }
    const CGPoint from = CGPointMake(atof(argv[2]), atof(argv[3]));
    if (click || move) {
      const pid_t pid = atoi(argv[4]);
      NSString* refused = Refusal(pid, from);
      if (refused) {
        fprintf(stderr, "pointer posted nothing: %s\n", refused.UTF8String);
        return 1;
      }
      if (move) {
        Post(kCGEventMouseMoved, from);
        if ((refused = Reached(from))) {
          fprintf(stderr, "pointer moved to %.0f %.0f, but %s\n", from.x, from.y,
                  refused.UTF8String);
          return 1;
        }
        printf("moved %.0f %.0f\n", from.x, from.y);
        return 0;
      }
      Post(kCGEventLeftMouseDown, from);
      if ((refused = Reached(from))) {
        Post(kCGEventLeftMouseUp, from);
        fprintf(stderr, "pointer pressed at %.0f %.0f, but %s\n", from.x, from.y,
                refused.UTF8String);
        return 1;
      }
      // A hand's press and let-go, a few frames apart.
      usleep(80000);
      if ((refused = Refusal(pid, from))) {
        // The press is down: it's let go anyway, so no button stays held, and the run fails.
        Post(kCGEventLeftMouseUp, from);
        fprintf(stderr, "pointer let go at once: %s\n", refused.UTF8String);
        return 1;
      }
      Post(kCGEventLeftMouseUp, from);
      Released();
      printf("clicked %.0f %.0f\n", from.x, from.y);
      return 0;
    }
    const CGPoint to = CGPointMake(atof(argv[4]), atof(argv[5]));
    const CGPoint was = Now();
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
    Reached(was);
    printf("dragged %.0f %.0f to %.0f %.0f\n", from.x, from.y, to.x, to.y);
    return 0;
  }
}
