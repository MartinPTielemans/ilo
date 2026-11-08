// macOS's own question in a check VM or a hosted Mac, as "Allow “ilo” to find devices on local
// networks?", which UserNotificationCenter shows over the app and which takes its activation: it
// finds the alert whose text has TEXT among UserNotificationCenter's windows, through the
// accessibility API, and presses its button named BUTTON. It waits up to 15 s for it and says
// "pressed <the alert's text>". Without one it says which app is in front and exits 1. It reads
// UserNotificationCenter's windows alone, never another app's. The run's process needs
// Accessibility, which check-mac.yml grants. Usage: alert TEXT BUTTON Built with clang++
// -fobjc-arc -framework Cocoa.
#import <Cocoa/Cocoa.h>

#include <initializer_list>

namespace {

id Attribute(AXUIElementRef element, CFStringRef name) {
  CFTypeRef value = NULL;
  if (AXUIElementCopyAttributeValue(element, name, &value) != kAXErrorSuccess || !value) {
    return nil;
  }
  return CFBridgingRelease(value);
}

NSArray* Children(AXUIElementRef element, CFStringRef name = kAXChildrenAttribute) {
  id children = Attribute(element, name);
  return [children isKindOfClass:NSArray.class] ? children : @[];
}

// What the element and everything in it say, as VoiceOver would read them.
NSString* AllText(AXUIElementRef element, int depth) {
  NSMutableArray* parts = [NSMutableArray array];
  for (CFStringRef name : {kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute}) {
    id part = Attribute(element, name);
    if ([part isKindOfClass:NSString.class] && [part length]) {
      [parts addObject:part];
    }
  }
  if (depth < 8) {
    for (id child in Children(element)) {
      NSString* text = AllText((__bridge AXUIElementRef)child, depth + 1);
      if (text.length) {
        [parts addObject:text];
      }
    }
  }
  return [parts componentsJoinedByString:@" "];
}

// The button titled |title| under |element|.
AXUIElementRef Button(AXUIElementRef element, NSString* title, int depth) {
  if ([Attribute(element, kAXRoleAttribute) isEqual:NSAccessibilityButtonRole] &&
      [Attribute(element, kAXTitleAttribute) isEqual:title]) {
    return element;
  }
  if (depth < 8) {
    for (id child in Children(element)) {
      if (AXUIElementRef found = Button((__bridge AXUIElementRef)child, title, depth + 1)) {
        return found;
      }
    }
  }
  return NULL;
}

}  // namespace

int main(int argc, char** argv) {
  @autoreleasepool {
    if (argc != 3) {
      fprintf(stderr, "usage: alert TEXT BUTTON\n");
      return 2;
    }
    NSString* text = @(argv[1]);
    NSString* title = @(argv[2]);
    for (int tries = 0; tries < 75; tries++) {
      for (NSRunningApplication* app in [NSRunningApplication
               runningApplicationsWithBundleIdentifier:@"com.apple.UserNotificationCenter"]) {
        id application = CFBridgingRelease(AXUIElementCreateApplication(app.processIdentifier));
        for (id window in Children((__bridge AXUIElementRef)application, kAXWindowsAttribute)) {
          NSString* said = AllText((__bridge AXUIElementRef)window, 0);
          if ([said rangeOfString:text options:NSCaseInsensitiveSearch].location == NSNotFound) {
            continue;
          }
          AXUIElementRef button = Button((__bridge AXUIElementRef)window, title, 0);
          if (button && AXUIElementPerformAction(button, kAXPressAction) == kAXErrorSuccess) {
            printf("pressed %s\n", said.UTF8String);
            return 0;
          }
        }
      }
      [NSThread sleepForTimeInterval:0.2];
    }
    NSRunningApplication* front = NSWorkspace.sharedWorkspace.frontmostApplication;
    printf("no alert with \"%s\" and %s; in front: %s\n", text.UTF8String, title.UTF8String,
           (front.bundleIdentifier ?: @"none").UTF8String);
    return 1;
  }
}
