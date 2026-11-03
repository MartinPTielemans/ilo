// Another app's window for little.sh on the Mac (#1689), as app-windows.py's cover is on Windows:
// a plain window at X, Y, WIDTH by HEIGHT points from the main display's top-left corner, in front
// of every app's window without taking the activation, as a window another app puts up while the
// user is in ilo is. It says "cover <window number>" once it's on screen and ends at the end of its
// input, handing the activation back to process PID first if it has it, as the app the user goes
// back to.
// Usage: cover PID X Y WIDTH HEIGHT
#import <Cocoa/Cocoa.h>

int main(int argc, const char* argv[]) {
  @autoreleasepool {
    if (argc != 6) {
      fprintf(stderr, "usage: cover PID X Y WIDTH HEIGHT\n");
      return 2;
    }
    const pid_t ilo = atoi(argv[1]);
    NSApplication* app = NSApplication.sharedApplication;
    // No Dock icon, and nothing makes it active as it starts, but it can be.
    app.activationPolicy = NSApplicationActivationPolicyAccessory;
    const CGFloat top = NSHeight(NSScreen.screens.firstObject.frame);
    const double width = atof(argv[4]), height = atof(argv[5]);
    const NSRect frame = NSMakeRect(atof(argv[2]), top - atof(argv[3]) - height, width, height);
    NSWindow* window = [[NSWindow alloc] initWithContentRect:frame
                                                   styleMask:NSWindowStyleMaskTitled
                                                     backing:NSBackingStoreBuffered
                                                       defer:NO];
    window.releasedWhenClosed = NO;
    window.title = @"cover";
    window.backgroundColor = [NSColor colorWithSRGBRed:0.25 green:0.38 blue:0.5 alpha:1];
    [window setFrame:frame display:YES];
    [window orderFrontRegardless];
    NSFileHandle* input = NSFileHandle.fileHandleWithStandardInput;
    [NSNotificationCenter.defaultCenter
        addObserverForName:NSFileHandleReadCompletionNotification
                    object:input
                     queue:nil
                usingBlock:^(NSNotification* notification) {
                  if ([notification.userInfo[NSFileHandleNotificationDataItem] length]) {
                    [input readInBackgroundAndNotify];
                    return;
                  }
                  if (app.active) {
                    NSRunningApplication* to =
                        [NSRunningApplication runningApplicationWithProcessIdentifier:ilo];
                    [app yieldActivationToApplication:to];
                    [to activateFromApplication:NSRunningApplication.currentApplication options:0];
                  }
                  printf("cover done, active %d\n", app.active);
                  fflush(stdout);
                  exit(0);
                }];
    [input readInBackgroundAndNotify];
    dispatch_async(dispatch_get_main_queue(), ^{
      printf("cover %ld\n", (long)window.windowNumber);
      fflush(stdout);
    });
    [app run];
  }
  return 0;
}
