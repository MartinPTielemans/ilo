// Another app's click on the Mac, for check/spaces.sh: a left click the window server delivers
// at <x> <y>, in points from the main display's top left, as the user's mouse does, wherever the
// app is and whether or not it's the active one. With "moved", the pointer first comes in from
// 120 points to the left, as a hand moves it there; with "bare", it goes straight to the point.
// It never runs on a Mac someone is using: the click goes to whatever is under the point.
// Build: cc -o click click.c -framework ApplicationServices
#include <ApplicationServices/ApplicationServices.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void post(CGEventType type, CGPoint at) {
  CGEventRef event = CGEventCreateMouseEvent(NULL, type, at, kCGMouseButtonLeft);
  if (type != kCGEventMouseMoved) {
    CGEventSetIntegerValueField(event, kCGMouseEventClickState, 1);
  }
  CGEventPost(kCGHIDEventTap, event);
  CFRelease(event);
}

int main(int argc, char** argv) {
  if (argc != 4 || (strcmp(argv[3], "moved") && strcmp(argv[3], "bare"))) {
    fprintf(stderr, "usage: click <x> <y> moved|bare\n");
    return 2;
  }
  const CGPoint at = CGPointMake(atof(argv[1]), atof(argv[2]));
  // Without the right to post events, macOS drops them without a word.
  printf("click: trusted %d, may post %d\n", AXIsProcessTrusted(), CGPreflightPostEventAccess());
  if (!strcmp(argv[3], "moved")) {
    for (int step = 12; step >= 0; step--) {
      post(kCGEventMouseMoved, CGPointMake(at.x - 10 * step, at.y));
      usleep(16000);
    }
    usleep(300000);
  }
  post(kCGEventLeftMouseDown, at);
  usleep(80000);
  post(kCGEventLeftMouseUp, at);
  return 0;
}
