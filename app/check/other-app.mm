// Another app's window on the Mac, for a check that goes from the app to another and back. macOS
// lets an app give its activation away but no process take it, so both ways are a yield: the
// driver's yield hands the app's to this one, and a line "back <pid>" on stdin hands this one's
// back to the app of process <pid>. It shows a small window, says "ready <pid>" once it can be
// yielded to, "active" each time it becomes the active app, with its window key, and "yielded to
// <pid>" after each "back". It ends at the end of stdin.
// Usage: other-app. Built with clang++ -fobjc-arc -framework Cocoa.
#import <Cocoa/Cocoa.h>

#include <stdio.h>
#include <unistd.h>

namespace {

void Say(NSString* line) {
  printf("%s\n", line.UTF8String);
  fflush(stdout);
}

}  // namespace

int main() {
  @autoreleasepool {
    NSApplication* app = NSApplication.sharedApplication;
    app.activationPolicy = NSApplicationActivationPolicyRegular;
    // At the screen's bottom left, clear of the browser window's top and a Mini Player's corner.
    NSWindow* window = [[NSWindow alloc] initWithContentRect:NSMakeRect(20, 20, 240, 120)
                                                   styleMask:NSWindowStyleMaskTitled
                                                     backing:NSBackingStoreBuffered
                                                       defer:NO];
    window.title = @"Other app";
    window.releasedWhenClosed = NO;
    [NSNotificationCenter.defaultCenter
        addObserverForName:NSApplicationDidBecomeActiveNotification
                    object:nil
                     queue:nil
                usingBlock:^(NSNotification*) {
                  [window makeKeyAndOrderFront:nil];
                  Say(@"active");
                }];
    [NSNotificationCenter.defaultCenter
        addObserverForName:NSApplicationDidFinishLaunchingNotification
                    object:nil
                     queue:nil
                usingBlock:^(NSNotification*) {
                  [window orderFront:nil];
                  Say([NSString stringWithFormat:@"ready %d", getpid()]);
                }];
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
      char line[64];
      while (fgets(line, sizeof line, stdin)) {
        int pid = 0;
        if (sscanf(line, "back %d", &pid) != 1) {
          continue;
        }
        dispatch_async(dispatch_get_main_queue(), ^{
          NSRunningApplication* to =
              [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
          if (!to) {
            Say([NSString stringWithFormat:@"no app of process %d", pid]);
            return;
          }
          [NSApp yieldActivationToApplication:to];
          [to activateFromApplication:NSRunningApplication.currentApplication options:0];
          Say([NSString stringWithFormat:@"yielded to %d", pid]);
        });
      }
      dispatch_async(dispatch_get_main_queue(), ^{
        [NSApp terminate:nil];
      });
    });
    [app run];
  }
  return 0;
}
