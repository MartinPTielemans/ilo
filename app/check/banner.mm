// A real notification banner on the Mac, for notifications.sh in a check VM (#685): it finds the
// banner whose text has TEXT among Notification Center's windows, through the accessibility API,
// and presses it as a click does, or the button named BUTTON in it, as on the banner macOS shows to
// ask whether the app may notify. It waits up to 15 s for it and says "pressed <its text>". A
// banner shows for a few seconds and then waits in Notification Center's list, so after 2 s it
// opens the list, as a click on the menu bar's clock does, and presses it there. Without one it
// prints what Notification Center's banners show, for the run's log, and exits 1. The run's process
// needs Accessibility, which check-mac.yml grants. Usage: banner TEXT [BUTTON] Built with clang++
// -fobjc-arc -framework Cocoa.
#import <Cocoa/Cocoa.h>

#include <initializer_list>

namespace {

NSString* Attribute(AXUIElementRef element, CFStringRef name) {
  CFTypeRef value = NULL;
  if (AXUIElementCopyAttributeValue(element, name, &value) != kAXErrorSuccess || !value) {
    return nil;
  }
  id object = CFBridgingRelease(value);
  return [object isKindOfClass:NSString.class] ? object : nil;
}

NSArray* Children(AXUIElementRef element) {
  CFTypeRef value = NULL;
  if (AXUIElementCopyAttributeValue(element, kAXChildrenAttribute, &value) != kAXErrorSuccess ||
      !value) {
    return @[];
  }
  id object = CFBridgingRelease(value);
  return [object isKindOfClass:NSArray.class] ? object : @[];
}

NSArray<NSString*>* Actions(AXUIElementRef element) {
  CFArrayRef names = NULL;
  if (AXUIElementCopyActionNames(element, &names) != kAXErrorSuccess || !names) {
    return @[];
  }
  return CFBridgingRelease(names);
}

// What the element says of itself, as VoiceOver would read it.
NSString* Text(AXUIElementRef element) {
  NSMutableArray* parts = [NSMutableArray array];
  for (CFStringRef name : {kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute}) {
    NSString* part = Attribute(element, name);
    if (part.length) {
      [parts addObject:part];
    }
  }
  return [parts componentsJoinedByString:@" | "];
}

// Every element under |element|, with the text of it and of all its descendants.
NSString* AllText(AXUIElementRef element, int depth) {
  NSMutableString* text = [Text(element) mutableCopy];
  if (depth < 12) {
    for (id child in Children(element)) {
      [text appendFormat:@" %@", AllText((__bridge AXUIElementRef)child, depth + 1)];
    }
  }
  return text;
}

void Print(AXUIElementRef element, int depth) {
  printf("%*s%s %s [%s]\n", depth * 2, "", Attribute(element, kAXRoleAttribute).UTF8String ?: "?",
         Text(element).UTF8String, [Actions(element) componentsJoinedByString:@","].UTF8String);
  if (depth < 12) {
    for (id child in Children(element)) {
      Print((__bridge AXUIElementRef)child, depth + 1);
    }
  }
}

// The innermost pressable element whose text, with its descendants', has |text|.
AXUIElementRef Find(AXUIElementRef element, NSString* text, int depth) {
  if (depth > 12 || ![AllText(element, depth) containsString:text]) {
    return NULL;
  }
  for (id child in Children(element)) {
    if (AXUIElementRef found = Find((__bridge AXUIElementRef)child, text, depth + 1)) {
      return found;
    }
  }
  return [Actions(element) containsObject:(__bridge NSString*)kAXPressAction] ? element : NULL;
}

// A button named |button| under |element|.
AXUIElementRef ButtonIn(AXUIElementRef element, NSString* button, int depth) {
  if ([Attribute(element, kAXRoleAttribute) isEqualToString:(__bridge NSString*)kAXButtonRole] &&
      [Text(element) isEqualToString:button]) {
    return element;
  }
  for (id child in Children(element)) {
    if (depth < 12) {
      if (AXUIElementRef found = ButtonIn((__bridge AXUIElementRef)child, button, depth + 1)) {
        return found;
      }
    }
  }
  return NULL;
}

// The button named |button| in the innermost element whose text has |text| and that holds one.
AXUIElementRef FindButton(AXUIElementRef element, NSString* text, NSString* button, int depth) {
  if (depth > 12 || ![AllText(element, depth) containsString:text]) {
    return NULL;
  }
  for (id child in Children(element)) {
    if (AXUIElementRef found =
            FindButton((__bridge AXUIElementRef)child, text, button, depth + 1)) {
      return found;
    }
  }
  return ButtonIn(element, button, depth);
}

// The action named |button| that the innermost element whose text has |text| offers, as a banner
// offers its buttons: "Name:Allow" and its target and selector.
NSString* NamedAction(AXUIElementRef element,
                      NSString* text,
                      NSString* button,
                      int depth,
                      AXUIElementRef* owner) {
  if (depth > 12 || ![AllText(element, depth) containsString:text]) {
    return nil;
  }
  for (id child in Children(element)) {
    if (NSString* found =
            NamedAction((__bridge AXUIElementRef)child, text, button, depth + 1, owner)) {
      return found;
    }
  }
  NSString* named = [NSString stringWithFormat:@"Name:%@\n", button];
  for (NSString* action in Actions(element)) {
    if ([action hasPrefix:named] || [action isEqualToString:button]) {
      *owner = element;
      return action;
    }
  }
  return nil;
}

// Opens Notification Center's list, as a click on the menu bar's clock does. Returns whether the
// clock took the press.
bool OpenList() {
  bool pressed = false;
  for (NSRunningApplication* app in NSWorkspace.sharedWorkspace.runningApplications) {
    if (![app.bundleIdentifier isEqualToString:@"com.apple.controlcenter"]) {
      continue;
    }
    AXUIElementRef control = AXUIElementCreateApplication(app.processIdentifier);
    CFTypeRef bar = NULL;
    if (AXUIElementCopyAttributeValue(control, CFSTR("AXExtrasMenuBar"), &bar) == kAXErrorSuccess &&
        bar) {
      for (id item in Children((AXUIElementRef)bar)) {
        if ([Attribute((__bridge AXUIElementRef)item, kAXIdentifierAttribute)
                isEqualToString:@"com.apple.menuextra.clock"]) {
          pressed = AXUIElementPerformAction((__bridge AXUIElementRef)item, kAXPressAction) ==
                    kAXErrorSuccess;
        }
      }
      CFRelease(bar);
    }
    CFRelease(control);
  }
  return pressed;
}
}  // namespace

int main(int argc, const char* argv[]) {
  @autoreleasepool {
    if (argc != 2 && argc != 3) {
      fprintf(stderr, "usage: banner TEXT [BUTTON]\n");
      return 2;
    }
    NSString* text = @(argv[1]);
    NSString* button = argc == 3 ? @(argv[2]) : nil;
    if (!AXIsProcessTrusted()) {
      printf("banner: not trusted for accessibility\n");
    }
    AXUIElementRef center = NULL;
    bool listed = false;
    for (int tries = 0; tries < 150; ++tries) {
      if (tries == 20 && !button) {
        listed = OpenList();
        printf("banner: %s\n", listed ? "opened Notification Center's list"
                                      : "no clock to open Notification Center's list");
      }
      NSRunningApplication* found_app = nil;
      for (NSRunningApplication* app in NSWorkspace.sharedWorkspace.runningApplications) {
        if ([app.bundleIdentifier isEqualToString:@"com.apple.notificationcenterui"] ||
            [app.executableURL.lastPathComponent isEqualToString:@"NotificationCenter"]) {
          found_app = app;
        }
      }
      if (found_app) {
        if (center) {
          CFRelease(center);
        }
        center = AXUIElementCreateApplication(found_app.processIdentifier);
        NSString* action = (__bridge NSString*)kAXPressAction;
        AXUIElementRef found = button ? FindButton(center, text, button, 0) : Find(center, text, 0);
        if (!found && button) {
          action = NamedAction(center, text, button, 0, &found) ?: action;
        }
        if (found) {
          const AXError error = AXUIElementPerformAction(found, (__bridge CFStringRef)action);
          printf("pressed %s with %s: %s\n", AllText(found, 0).UTF8String,
                 [action componentsSeparatedByString:@"\n"][0].UTF8String,
                 error ? [@(error) stringValue].UTF8String : "done");
          return error ? 1 : 0;
        }
      }
      usleep(100000);
    }
    printf("banner: none with \"%s\"%s%s. Notification Center shows:\n", text.UTF8String,
           button ? ", button " : "", button ? button.UTF8String : "");
    if (listed) {
      // Not the list, which holds its widgets too.
      printf("banner: (its list isn't printed)\n");
    } else if (center) {
      // Its banners' window alone: the others are its widgets, and the menu bar the system's.
      for (id window : Children(center)) {
        if ([Text((__bridge AXUIElementRef)window) isEqualToString:@"Notification Center"]) {
          Print((__bridge AXUIElementRef)window, 0);
        }
      }
    } else {
      printf("banner: Notification Center isn't running\n");
    }
    return 1;
  }
}
