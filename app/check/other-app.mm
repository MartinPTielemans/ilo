// Another app's window on the Mac, for a check that goes from the app to another and back. macOS
// lets an app give its activation away but no process take it, so both ways are a yield: the
// driver's yield hands the app's to this one, and a line "back <pid>" on stdin hands this one's
// back to the app of process <pid>. A line "fullscreen" makes its window full screen on the
// screen BROWSER_DISPLAY picks, as the app's host puts the browser's window there, and "windowed"
// takes it out again, with each said once done: the Mac keeps a window off the Space another app
// has made full screen unless it may join one, which a check watches the Mini Player over. It
// shows a small window, says "ready <pid>" once it can be yielded to, "active" each time it
// becomes the active app, with its window key, and "yielded to <pid>" after each "back". It ends
// at the end of stdin.
// Usage: other-app. Built with clang++ -fobjc-arc -framework Cocoa.
#import <Cocoa/Cocoa.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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
    // Resizable, as full screen takes it: AppKit leaves a window that isn't resizable out of
    // full screen, and toggleFullScreen on it is a no-op that says nothing back, so the check's
    // wait for the entered-full-screen line ran out.
    NSWindow* window = [[NSWindow alloc] initWithContentRect:NSMakeRect(20, 20, 240, 120)
                                                   styleMask:NSWindowStyleMaskTitled |
                                                            NSWindowStyleMaskResizable
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
    [NSNotificationCenter.defaultCenter
        addObserverForName:NSWindowDidEnterFullScreenNotification
                    object:window
                     queue:nil
                usingBlock:^(NSNotification*) {
                  Say(@"fullscreen");
                }];
    [NSNotificationCenter.defaultCenter
        addObserverForName:NSWindowDidExitFullScreenNotification
                    object:window
                     queue:nil
                usingBlock:^(NSNotification*) {
                  Say(@"windowed");
                }];
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
      char line[64];
      while (fgets(line, sizeof line, stdin)) {
        int pid = 0;
        if (sscanf(line, "back %d", &pid) == 1) {
          dispatch_async(dispatch_get_main_queue(), ^{
            NSRunningApplication* to =
                [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
            if (!to) {
              Say([NSString stringWithFormat:@"no app of process %d", pid]);
              return;
            }
            // Always, as the check runs on macOS 15, but this builds for older SDKs too.
            if (@available(macOS 14, *)) {
              [NSApp yieldActivationToApplication:to];
              [to activateFromApplication:NSRunningApplication.currentApplication options:0];
            }
            Say([NSString stringWithFormat:@"yielded to %d", pid]);
          });
        } else if (strncmp(line, "fullscreen", 10) == 0) {
          dispatch_async(dispatch_get_main_queue(), ^{
            // The browser's own screen, as the host picks it (platform/mac/host.mm), so the full
            // screen takes the Space the Mini Player is on and not the main display's.
            NSScreen* screen = NSScreen.screens.firstObject;
            if (strcmp(getenv("BROWSER_DISPLAY") ?: "", "builtin") == 0) {
              for (NSScreen* each in NSScreen.screens) {
                if (CGDisplayIsBuiltin([each.deviceDescription[@"NSScreenNumber"]
                                        unsignedIntValue])) {
                  screen = each;
                }
              }
            }
            [window setFrameOrigin:NSMakePoint(screen.frame.origin.x + 20,
                                               screen.frame.origin.y + 20)];
            [window toggleFullScreen:nil];
          });
        } else if (strncmp(line, "windowed", 8) == 0) {
          dispatch_async(dispatch_get_main_queue(), ^{
            [window toggleFullScreen:nil];
          });
        }
      }
      dispatch_async(dispatch_get_main_queue(), ^{
        [NSApp terminate:nil];
      });
    });
    [app run];
  }
  return 0;
}
