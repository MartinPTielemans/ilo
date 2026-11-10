// macOS's own question in a check VM or a hosted Mac, as "Allow … to find devices on local
// networks?", which UserNotificationCenter shows over the app and which takes its activation: it
// finds the alert whose text has TEXT, or any with TEXT empty, among UserNotificationCenter's
// windows, through the accessibility API, and presses its button named BUTTON. It waits up to
// 15 s for it and says "pressed BUTTON on TEXT", never the rest of the alert, which names the
// process that asked. Without one it says which app is in front and exits 1. With no arguments it
// waits for any of its alerts and says only what it asks about, from a fixed list of topics, and
// its buttons. It reads UserNotificationCenter's windows alone, never another app's. The run's
// process needs Accessibility, which check-mac.yml grants.
// Usage: alert [TEXT BUTTON]. Built with clang++ -fobjc-arc -framework Cocoa.
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

// The titles of the buttons under |element|.
void Buttons(AXUIElementRef element, NSMutableArray* titles, int depth) {
  if ([Attribute(element, kAXRoleAttribute) isEqual:NSAccessibilityButtonRole]) {
    id title = Attribute(element, kAXTitleAttribute);
    if ([title isKindOfClass:NSString.class]) {
      [titles addObject:title];
    }
  }
  if (depth < 8) {
    for (id child in Children(element)) {
      Buttons((__bridge AXUIElementRef)child, titles, depth + 1);
    }
  }
}

}  // namespace

int main(int argc, char** argv) {
  @autoreleasepool {
    if (argc != 1 && argc != 3) {
      fprintf(stderr, "usage: alert [TEXT BUTTON]\n");
      return 2;
    }
    if (argc == 1) {
      for (int tries = 0; tries < 75; tries++) {
        for (NSRunningApplication* app in [NSRunningApplication
                 runningApplicationsWithBundleIdentifier:@"com.apple.UserNotificationCenter"]) {
          id application = CFBridgingRelease(AXUIElementCreateApplication(app.processIdentifier));
          for (id window in Children((__bridge AXUIElementRef)application, kAXWindowsAttribute)) {
            NSString* said = AllText((__bridge AXUIElementRef)window, 0);
            NSMutableArray* topics = [NSMutableArray array];
            for (NSString* topic in @[ @"camera", @"microphone", @"local network", @"screen",
                                       @"record", @"keystrokes", @"accessibility",
                                       @"notifications", @"bluetooth", @"location" ]) {
              if ([said rangeOfString:topic options:NSCaseInsensitiveSearch].location !=
                  NSNotFound) {
                [topics addObject:topic];
              }
            }
            NSMutableArray* buttons = [NSMutableArray array];
            Buttons((__bridge AXUIElementRef)window, buttons, 0);
            printf("UserNotificationCenter asks about %s, with %s\n",
                   topics.count ? [topics componentsJoinedByString:@", "].UTF8String
                                : "none of the known topics",
                   [buttons componentsJoinedByString:@", "].UTF8String);
            return 0;
          }
        }
        [NSThread sleepForTimeInterval:0.2];
      }
      NSRunningApplication* front = NSWorkspace.sharedWorkspace.frontmostApplication;
      printf("UserNotificationCenter shows nothing; in front: %s\n",
             (front.bundleIdentifier ?: @"none").UTF8String);
      return 1;
    }
    NSString* text = @(argv[1]);
    NSString* title = @(argv[2]);
    for (int tries = 0; tries < 75; tries++) {
      for (NSRunningApplication* app in [NSRunningApplication
               runningApplicationsWithBundleIdentifier:@"com.apple.UserNotificationCenter"]) {
        id application = CFBridgingRelease(AXUIElementCreateApplication(app.processIdentifier));
        for (id window in Children((__bridge AXUIElementRef)application, kAXWindowsAttribute)) {
          NSString* said = AllText((__bridge AXUIElementRef)window, 0);
          if (text.length &&
              [said rangeOfString:text options:NSCaseInsensitiveSearch].location == NSNotFound) {
            continue;
          }
          AXUIElementRef button = Button((__bridge AXUIElementRef)window, title, 0);
          if (button && AXUIElementPerformAction(button, kAXPressAction) == kAXErrorSuccess) {
            printf("pressed %s on \"%s\"\n", title.UTF8String, text.UTF8String);
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
