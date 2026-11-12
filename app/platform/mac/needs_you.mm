// Quiet until it needs you, outside ilo (app/src/needs_you.rs): the Dock icon's badge counts the
// tasks waiting for the user, and a need that comes while another app is active posts one
// notification through UNUserNotificationCenter, whose click brings ilo forward and opens its task.

#import <Cocoa/Cocoa.h>
#import <UserNotifications/UserNotifications.h>

#include "../host.h"

namespace {

void (*g_clicked)(uint64_t id);
// The system's leave is asked once a run, as the first notification is posted, never at launch.
bool g_asked;

NSString* const kNeed = @"need";

}  // namespace

@interface NeedsYouDelegate : NSObject <UNUserNotificationCenterDelegate>
@end

@implementation NeedsYouDelegate

// A click on one: the app comes forward, as a banner's click makes it active, and opens the task.
- (void)userNotificationCenter:(UNUserNotificationCenter*)center
    didReceiveNotificationResponse:(UNNotificationResponse*)response
             withCompletionHandler:(void (^)(void))completionHandler {
  NSNumber* need = response.notification.request.content.userInfo[kNeed];
  if (need && [response.actionIdentifier isEqualToString:UNNotificationDefaultActionIdentifier]) {
    const uint64_t id = need.unsignedLongLongValue;
    dispatch_async(dispatch_get_main_queue(), ^{
      [NSApp activateIgnoringOtherApps:YES];
      if (g_clicked) {
        g_clicked(id);
      }
    });
  }
  completionHandler();
}

// One that comes as ilo becomes active still shows, in the list alone, as ilo has it in view.
- (void)userNotificationCenter:(UNUserNotificationCenter*)center
       willPresentNotification:(UNNotification*)notification
         withCompletionHandler:(void (^)(UNNotificationPresentationOptions))completionHandler {
  completionHandler(UNNotificationPresentationOptionList);
}

@end

namespace {

UNUserNotificationCenter* Center() {
  static NeedsYouDelegate* delegate = [[NeedsYouDelegate alloc] init];
  UNUserNotificationCenter* center = UNUserNotificationCenter.currentNotificationCenter;
  if (!center.delegate) {
    center.delegate = delegate;
  } else if (center.delegate != delegate) {
    // A click on one would reach no task.
    fprintf(stderr, "browser: needs you clicks are another's\n");
  }
  return center;
}

void Post(UNUserNotificationCenter* center, uint64_t id, NSString* title, NSString* body) {
  UNMutableNotificationContent* content = [[UNMutableNotificationContent alloc] init];
  content.title = title;
  content.body = body;
  content.userInfo = @{kNeed : @(id)};
  // No sound of ilo's, nor the system's.
  content.sound = nil;
  NSString* name = [NSString stringWithFormat:@"needs-you-%llu", id];
  UNNotificationRequest* request = [UNNotificationRequest requestWithIdentifier:name
                                                                        content:content
                                                                        trigger:nil];
  [center addNotificationRequest:request
           withCompletionHandler:^(NSError* error) {
             // check.sh reads this line.
             fprintf(stderr, "browser: needs you posted %llu%s%s\n", id, error ? ": " : "",
                     error ? error.localizedDescription.UTF8String : "");
           }];
}

}  // namespace

bool host_app_active(void) {
  return NSApp.isActive;
}

void host_needs_you_badge(int count) {
  NSApp.dockTile.badgeLabel = count > 0 ? [NSString stringWithFormat:@"%d", count] : nil;
  // check.sh reads this line.
  fprintf(stderr, "browser: dock badge %s\n",
          NSApp.dockTile.badgeLabel ? NSApp.dockTile.badgeLabel.UTF8String : "none");
}

void host_watch_needs_you(void (*clicked)(uint64_t id)) {
  g_clicked = clicked;
}

void host_needs_you_notify(uint64_t id, const char* title, const char* body) {
  UNUserNotificationCenter* center = Center();
  NSString* heading = @(title);
  NSString* words = @(body);
  [center getNotificationSettingsWithCompletionHandler:^(UNNotificationSettings* settings) {
    switch (settings.authorizationStatus) {
      case UNAuthorizationStatusNotDetermined: {
        if (g_asked) {
          return;
        }
        g_asked = true;
        // Banners and the list alone: the badge is the Dock tile's, and no sound.
        [center requestAuthorizationWithOptions:UNAuthorizationOptionAlert
                              completionHandler:^(BOOL granted, NSError*) {
                                fprintf(stderr, "browser: needs you asked: %s\n",
                                        granted ? "allowed" : "refused");
                                if (granted) {
                                  Post(center, id, heading, words);
                                }
                              }];
        return;
      }
      case UNAuthorizationStatusDenied:
        fprintf(stderr, "browser: needs you not allowed\n");
        return;
      default:
        Post(center, id, heading, words);
    }
  }];
}
