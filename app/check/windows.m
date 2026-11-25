// The windows process PID has on screen, for rules.sh on the Mac: one a line, as "<level> <width>
// <height>" in points. A list, a picker, a sheet or a dialog that a page gets drawn outside itself
// is a window of the app's more, whatever draws it, so a run that lists them after a page's ask
// sees one without knowing which it would be. It asks the window server, which names no window and
// shows nothing of one's contents, so it needs no permission from the user.
// Usage: windows PID
#import <Cocoa/Cocoa.h>

int main(int argc, const char* argv[]) {
  @autoreleasepool {
    if (argc != 2) {
      fprintf(stderr, "usage: windows PID\n");
      return 2;
    }
    const pid_t app = atoi(argv[1]);
    NSArray* windows = CFBridgingRelease(
        CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID));
    if (!windows) {
      fprintf(stderr, "the window server listed no windows\n");
      return 1;
    }
    for (NSDictionary* window in windows) {
      if ([window[(id)kCGWindowOwnerPID] intValue] != app) {
        continue;
      }
      CGRect bounds = CGRectZero;
      CGRectMakeWithDictionaryRepresentation((CFDictionaryRef)window[(id)kCGWindowBounds], &bounds);
      printf("%d %.0f %.0f\n", [window[(id)kCGWindowLayer] intValue], bounds.size.width,
             bounds.size.height);
    }
  }
  return 0;
}
