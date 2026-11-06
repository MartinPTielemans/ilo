// Updates on the Mac (docs/ux-spec.md › Updates): Sparkle 2, from a release build that carries it
// with its feed and key, loaded once the first window shows so launch never waits for it. Around it,
// what keeps an update quick and whole: macOS's check of a new version runs while the user waits for
// the restart, not on its first launch (#389), and Chromium never starts without its cookie key
// after a change of signature.

#import <Cocoa/Cocoa.h>
#import <Security/Security.h>
#import <Sparkle/Sparkle.h>

#include <signal.h>

#include "../host.h"

namespace {

// Ardia's: after 90 s a check that hasn't finished never will, and Install and Relaunch waits for
// one at most 15 s.
constexpr int64_t kPrewarmLimit = 90;
constexpr int64_t kInstallWait = 15;
// The release notes of the update being installed: its version and their URL.
NSString* const kNotes = @"UpdateNotes";
// The designated requirement Chromium last read its cookie key under.
NSString* const kKeySignature = @"KeychainKeySignature";
// The launches that tried to make the cookie key's item again under a signature, and failed. Each
// try may have macOS ask, so a failure is tried again on only a few launches.
NSString* const kKeyTries = @"KeychainKeyTries";
constexpr NSInteger kKeyTriesLimit = 3;

void Log(NSString* line) {
  fprintf(stderr, "browser: %s\n", line.UTF8String);
}

// The running app's designated requirement, which macOS grants Keychain items to: alike across
// versions signed by the same Developer ID, new for another identity, and for every ad hoc build.
SecRequirementRef CopyOwnRequirement() {
  SecCodeRef running = nullptr;
  SecStaticCodeRef code = nullptr;
  SecRequirementRef requirement = nullptr;
  if (SecCodeCopySelf(kSecCSDefaultFlags, &running) == errSecSuccess &&
      SecCodeCopyStaticCode(running, kSecCSDefaultFlags, &code) == errSecSuccess) {
    SecCodeCopyDesignatedRequirement(code, kSecCSDefaultFlags, &requirement);
  }
  if (code) CFRelease(code);
  if (running) CFRelease(running);
  return requirement;
}

NSString* OwnSignature() {
  SecRequirementRef requirement = CopyOwnRequirement();
  if (!requirement) return nil;
  CFStringRef text = nullptr;
  SecRequirementCopyString(requirement, kSecCSDefaultFlags, &text);
  CFRelease(requirement);
  return CFBridgingRelease(text);
}

// The cookie key's item, Chromium's, in the login keychain, or with |suffix|, the spare that holds
// the key while the item is made again.
NSDictionary* KeyItem(NSString* name, NSString* suffix = @"") {
  NSString* service = [NSString stringWithFormat:@"%@ Safe Storage%@", name, suffix];
  return @{
    (__bridge id)kSecClass : (__bridge id)kSecClassGenericPassword,
    (__bridge id)kSecAttrService : service,
    (__bridge id)kSecAttrAccount : name,
  };
}
NSString* const kSpare = @" (moving)";

OSStatus CopyKey(NSDictionary* item, NSData** key) {
  NSMutableDictionary* query = [item mutableCopy];
  query[(__bridge id)kSecReturnData] = @YES;
  CFTypeRef data = nullptr;
  OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &data);
  *key = CFBridgingRelease(data);
  return status;
}

// Adds |item| with |key|. macOS grants an item to the designated requirement of the app that
// adds it, and to no other.
OSStatus AddKey(NSDictionary* item, NSData* key) {
  NSMutableDictionary* attributes = [item mutableCopy];
  attributes[(__bridge id)kSecAttrLabel] = item[(__bridge id)kSecAttrService];
  attributes[(__bridge id)kSecValueData] = key;
  return SecItemAdd((__bridge CFDictionaryRef)attributes, nullptr);
}

// Whether |item| holds |key|, read back.
OSStatus ReadBack(NSDictionary* item, NSData* key) {
  NSData* held = nil;
  OSStatus status = CopyKey(item, &held);
  return status == errSecSuccess && ![held isEqual:key] ? errSecIO : status;
}

// Reads the cookie key as Chromium will, so macOS asks for it now if it's going to. A launch that
// stopped while the item was made again left the key in the spare, which goes back first. Not
// found means neither has it: with the spare there, Chromium would make a new key and lose the
// profile's cookies and passwords, so a spare that can't be read or put back leaves both as they
// are. |stranded| says the key is in the spare and couldn't be put back.
OSStatus ReadKey(NSString* name, NSData** key, bool* stranded) {
  *stranded = false;
  OSStatus status = CopyKey(KeyItem(name), key);
  if (status != errSecItemNotFound) return status;
  OSStatus spare = CopyKey(KeyItem(name, kSpare), key);
  if (spare == errSecItemNotFound) return status;
  if (spare != errSecSuccess) {
    Log([NSString stringWithFormat:@"the cookie key's spare couldn't be read: %d", (int)spare]);
    return spare;
  }
  status = AddKey(KeyItem(name), *key);
  if (status == errSecSuccess) status = ReadBack(KeyItem(name), *key);
  Log([NSString stringWithFormat:@"the cookie key came back from its spare: %d", (int)status]);
  if (status == errSecItemNotFound) status = errSecIO;
  *stranded = status != errSecSuccess;
  return status;
}

// Makes the cookie key's item again, holding the same key, so that it's granted to this app's
// designated requirement instead of the one that made it. Signed with the team's requirement
// (tools/requirement.sh), every build the team signs reads it after that, whichever of its
// identities signed it, and the move from Apple Development to Developer ID asks nothing. Changing
// the item's access in place would ask for the keychain's password; making it again doesn't.
// Each step waits for the one before to read back, so one of the two always holds the key, and
// success means the item does.
OSStatus Regrant(NSString* name, NSData* key) {
  NSDictionary* spare = KeyItem(name, kSpare);
  SecItemDelete((__bridge CFDictionaryRef)spare);
  OSStatus status = AddKey(spare, key);
  if (status == errSecSuccess) status = ReadBack(spare, key);
  if (status == errSecSuccess) status = SecItemDelete((__bridge CFDictionaryRef)KeyItem(name));
  if (status != errSecSuccess) {
    // The item is as it was.
    SecItemDelete((__bridge CFDictionaryRef)spare);
    return status;
  }
  status = AddKey(KeyItem(name), key);
  if (status == errSecSuccess) status = ReadBack(KeyItem(name), key);
  // Otherwise the spare stays, for ReadKey to put back.
  if (status == errSecSuccess) SecItemDelete((__bridge CFDictionaryRef)spare);
  return status;
}

// Where Sparkle unpacks an update before installing it, as Sparkle 2.10's installer does: the app
// is a few folders down in ~/Library/Caches/<bundle id>/org.sparkle-project.Sparkle/Installation.
NSBundle* StagedApp(NSString* version) {
  NSString* bundle = NSBundle.mainBundle.bundleIdentifier;
  NSURL* caches = [NSFileManager.defaultManager URLsForDirectory:NSCachesDirectory
                                                       inDomains:NSUserDomainMask]
                      .firstObject;
  NSURL* root = [[caches URLByAppendingPathComponent:bundle]
      URLByAppendingPathComponent:@"org.sparkle-project.Sparkle/Installation"];
  NSDirectoryEnumerator* walk = [NSFileManager.defaultManager
                 enumeratorAtURL:root
      includingPropertiesForKeys:nil
                         options:NSDirectoryEnumerationSkipsHiddenFiles |
                                 NSDirectoryEnumerationSkipsPackageDescendants
                    errorHandler:nil];
  for (NSURL* url in walk) {
    if (walk.level > 4) {
      [walk skipDescendants];
      continue;
    }
    if (![url.pathExtension isEqual:@"app"]) continue;
    NSBundle* app = [NSBundle bundleWithURL:url];
    if ([app.bundleIdentifier isEqual:bundle] &&
        [[app objectForInfoDictionaryKey:@"CFBundleVersion"] isEqual:version]) {
      return app;
    }
  }
  return nil;
}

// Whether |app| is signed as this app is, whole: what's run from it is only ever ours. Reading every
// file's hash is also most of what macOS does before a new version's first launch.
bool SignedLikeThisApp(NSURL* app) {
  SecRequirementRef requirement = CopyOwnRequirement();
  SecStaticCodeRef code = nullptr;
  bool alike = requirement &&
               SecStaticCodeCreateWithPath((__bridge CFURLRef)app, kSecCSDefaultFlags, &code) ==
                   errSecSuccess &&
               SecStaticCodeCheckValidity(code,
                                          kSecCSStrictValidate | kSecCSCheckNestedCode |
                                              kSecCSCheckAllArchitectures,
                                          requirement) == errSecSuccess;
  if (code) CFRelease(code);
  if (requirement) CFRelease(requirement);
  return alike;
}

// Whether the updater reads the nightly feed (host_follow_nightly).
bool g_follow_nightly = false;

// Whether Sparkle checks and downloads on its schedule (host_set_automatic_update_checks).
bool g_automatic = true;

// The nightly feed the build carries, if it carries one.
NSString* NightlyFeed() {
  id feed = [NSBundle.mainBundle objectForInfoDictionaryKey:@"NightlyFeedURL"];
  return [feed isKindOfClass:NSString.class] && [feed length] ? feed : nil;
}

}  // namespace

// Sparkle's delegate, for both its updater and its alerts.
@interface Updates : NSObject <SPUUpdaterDelegate, SPUStandardUserDriverDelegate>
@property(nonatomic) SPUStandardUpdaterController* controller;
// Told where the update stands, for Settings › About.
@property(nonatomic) void (*changed)(int stage, const char* version);
// Sparkle's install now, of the update downloaded to install on quit.
@property(nonatomic, copy) dispatch_block_t installNow;
// Restarts the browser, for Install and Relaunch.
@property(nonatomic) void (*restart)(void);
// Sparkle's Install and Relaunch, and the version it's of, held until the restart goes ahead.
@property(nonatomic, copy) dispatch_block_t relaunch;
@property(nonatomic, copy) NSString* relaunchVersion;
- (void)restartOnceWarm;
@end

@implementation Updates {
  // Versions whose check macOS has done, the one it's doing and what waits for it.
  NSMutableSet<NSString*>* _warmed;
  NSString* _warming;
  NSTask* _task;
  NSMutableArray<dispatch_block_t>* _afterWarming;
  // The stage last told.
  int _stage;
}

- (instancetype)init {
  if ((self = [super init])) {
    _warmed = [NSMutableSet set];
    _afterWarming = [NSMutableArray array];
    NSNotificationCenter* center = NSNotificationCenter.defaultCenter;
    [center addObserver:self
               selector:@selector(terminating:)
                   name:NSApplicationWillTerminateNotification
                 object:nil];
  }
  return self;
}

// The release notes the first launch of |item| opens, kept until then.
- (void)keepNotes:(SUAppcastItem*)item {
  NSURL* url = item.fullReleaseNotesURL ?: item.releaseNotesURL;
  if (![url.scheme isEqual:@"https"]) return;
  [NSUserDefaults.standardUserDefaults
      setObject:@{@"version" : item.versionString, @"url" : url.absoluteString}
         forKey:kNotes];
}

// Runs the staged copy of |version| for a moment, with BROWSER_PREWARM, which ends it before it
// does anything, so macOS checks it now, as it would before its first launch. Then |then|.
- (void)warm:(NSString*)version then:(dispatch_block_t)then {
  if ([_warmed containsObject:version]) {
    if (then) then();
    return;
  }
  if (then) [_afterWarming addObject:then];
  if ([version isEqual:_warming]) return;
  _warming = version;
  NSDate* began = NSDate.date;
  dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
    NSBundle* app = StagedApp(version);
    NSString* outcome = nil;
    // The signature's check and the run apart, so the log says which of them macOS's check is in.
    NSDate* checking = NSDate.date;
    if (!app) {
      outcome = @"not found";
    } else if (!SignedLikeThisApp(app.bundleURL)) {
      outcome = @"not signed as this app is";
    } else {
      Log([NSString stringWithFormat:@"update %@ signature checked in %.0f ms", version,
                                     -checking.timeIntervalSinceNow * 1000]);
      NSTask* task = [[NSTask alloc] init];
      task.executableURL = app.executableURL;
      NSMutableDictionary* environment = [NSProcessInfo.processInfo.environment mutableCopy];
      environment[@"BROWSER_PREWARM"] = @"1";
      task.environment = environment;
      task.standardInput = NSFileHandle.fileHandleWithNullDevice;
      task.standardOutput = NSFileHandle.fileHandleWithNullDevice;
      task.standardError = NSFileHandle.fileHandleWithNullDevice;
      NSError* error = nil;
      if ([task launchAndReturnError:&error]) {
        dispatch_sync(dispatch_get_main_queue(), ^{
          self->_task = task;
        });
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, kPrewarmLimit * NSEC_PER_SEC),
                       dispatch_get_main_queue(), ^{
                         if (task.running) kill(task.processIdentifier, SIGKILL);
                       });
        [task waitUntilExit];
        outcome = [NSString stringWithFormat:@"status %d", task.terminationStatus];
      } else {
        outcome = error.localizedDescription;
      }
    }
    dispatch_async(dispatch_get_main_queue(), ^{
      Log([NSString stringWithFormat:@"update %@ prewarmed in %.0f ms, %@", version,
                                     -began.timeIntervalSinceNow * 1000, outcome]);
      self->_task = nil;
      self->_warming = nil;
      [self->_warmed addObject:version];
      NSArray<dispatch_block_t>* waiting = self->_afterWarming;
      self->_afterWarming = [NSMutableArray array];
      for (dispatch_block_t next in waiting) next();
    });
  });
}

- (void)terminating:(NSNotification*)notification {
  if (_task.running) kill(_task.processIdentifier, SIGKILL);
}

// Tells Settings › About the update is at |stage|, of |item| if it's named.
- (void)tell:(int)stage item:(SUAppcastItem*)item {
  _stage = stage;
  if (self.changed) self.changed(stage, (item.displayVersionString ?: @"").UTF8String);
}

// The feed each check reads: the nightly one while the user takes nightly builds, else
// Info.plist's, which nil leaves Sparkle to read. The log names it for the nightly run.
- (NSString*)feedURLStringForUpdater:(SPUUpdater*)updater {
  NSString* nightly = g_follow_nightly ? NightlyFeed() : nil;
  NSString* feed = nightly ?: [NSBundle.mainBundle objectForInfoDictionaryKey:@"SUFeedURL"];
  Log([NSString stringWithFormat:@"update feed %@", feed]);
  return nightly;
}

// Before each check, Sparkle's own every 8 hours too.
- (BOOL)updater:(SPUUpdater*)updater
    mayPerformUpdateCheck:(SPUUpdateCheck)updateCheck
                    error:(NSError* __autoreleasing*)error {
  [self tell:HOST_UPDATE_CHECKING item:nil];
  return YES;
}

// An update downloaded in the background, to install when the app quits: macOS checks it now,
// while the user goes on browsing, and Settings › About offers to restart into it. Taking the
// install over (YES) is what gives Restart to Update Sparkle's install now; Sparkle still installs
// it when the app quits, and runs no more checks until then, as there's nothing newer to show.
- (BOOL)updater:(SPUUpdater*)updater
    willInstallUpdateOnQuit:(SUAppcastItem*)item
    immediateInstallationBlock:(void (^)(void))immediateInstallHandler {
  [self keepNotes:item];
  [self warm:item.versionString then:nil];
  self.installNow = immediateInstallHandler;
  [self tell:HOST_UPDATE_READY item:item];
  return YES;
}

- (void)updater:(SPUUpdater*)updater didFindValidUpdate:(SUAppcastItem*)item {
  Log([NSString stringWithFormat:@"update %@ found", item.versionString]);
  [self tell:HOST_UPDATE_FOUND item:item];
}

- (void)updater:(SPUUpdater*)updater
    willDownloadUpdate:(SUAppcastItem*)item
           withRequest:(NSMutableURLRequest*)request {
  [self tell:HOST_UPDATE_DOWNLOADING item:item];
}

- (void)updater:(SPUUpdater*)updater
    failedToDownloadUpdate:(SUAppcastItem*)item
                     error:(NSError*)error {
  Log([NSString stringWithFormat:@"update %@ download failed: %@", item.versionString,
                                 error.localizedDescription]);
  [self tell:HOST_UPDATE_FAILED item:item];
}

- (void)userDidCancelDownload:(SPUUpdater*)updater {
  [self tell:HOST_UPDATE_IDLE item:nil];
}

// Skipped, or put off from Sparkle's alert: the row offers the check again. An update dismissed
// while installing installs on quit, which the next check finds ready.
- (void)updater:(SPUUpdater*)updater
    userDidMakeChoice:(SPUUserUpdateChoice)choice
            forUpdate:(SUAppcastItem*)item
                state:(SPUUserUpdateState*)state {
  if (choice != SPUUserUpdateChoiceInstall) [self tell:HOST_UPDATE_IDLE item:nil];
}

// A check that finds no update says why, with Sparkle's reason (SPUNoUpdateFoundReason) and the
// newest build the feed has, as Sparkle is only loaded, not linked, its keys are spelled out here.
- (void)updaterDidNotFindUpdate:(SPUUpdater*)updater error:(NSError*)error {
  SUAppcastItem* latest = error.userInfo[@"SULatestAppcastItemFound"];
  Log([NSString stringWithFormat:@"update none: reason %@, feed's newest %@, running %@",
                                 error.userInfo[@"SUNoUpdateFoundReason"],
                                 latest.versionString ?: @"none",
                                 [NSBundle.mainBundle objectForInfoDictionaryKey:@"CFBundleVersion"]]);
  [self tell:HOST_UPDATE_CURRENT item:nil];
}

// Any other end to a check, such as a feed that doesn't parse, with what Sparkle says went wrong.
- (void)updater:(SPUUpdater*)updater didAbortWithError:(NSError*)error {
  // SUNoUpdateError, which updaterDidNotFindUpdate:error: has told.
  if (error.code == 1001) return;
  NSError* cause = error.userInfo[NSUnderlyingErrorKey];
  Log([NSString stringWithFormat:@"update failed: %@ %ld, %@%@", error.domain, (long)error.code,
                                 error.localizedDescription,
                                 cause ? [@" " stringByAppendingString:cause.description] : @""]);
  // SUInstallationCanceledError: the user said no to macOS's authorization.
  [self tell:error.code == 4007 ? HOST_UPDATE_IDLE : HOST_UPDATE_FAILED item:nil];
}

// The feed parsed and the check's end, which a check that shows no outcome says it got to.
- (void)updater:(SPUUpdater*)updater didFinishLoadingAppcast:(SUAppcast*)appcast {
  Log([NSString stringWithFormat:@"update feed loaded, %lu items",
                                 (unsigned long)appcast.items.count]);
}

- (void)updater:(SPUUpdater*)updater
    didFinishUpdateCycleForUpdateCheck:(SPUUpdateCheck)updateCheck
                                 error:(NSError*)error {
  NSString* outcome = error ? [NSString stringWithFormat:@"%@ %ld, %@", error.domain,
                                                         (long)error.code,
                                                         error.localizedDescription]
                            : @"no error";
  Log([NSString stringWithFormat:@"update cycle ended: %@", outcome]);
  // A check that ended without saying what it found, so the row doesn't stay at Checking….
  if (_stage == HOST_UPDATE_CHECKING) [self tell:HOST_UPDATE_IDLE item:nil];
}

- (void)updater:(SPUUpdater*)updater willScheduleUpdateCheckAfterDelay:(NSTimeInterval)delay {
  Log([NSString stringWithFormat:@"update check in %.0f s", delay]);
}

- (void)updater:(SPUUpdater*)updater willInstallUpdate:(SUAppcastItem*)item {
  [self keepNotes:item];
}

// Install and Relaunch, Restart to Update's or Sparkle's own window's: Sparkle's installer learns
// it relaunches only once the restart goes ahead (host_relaunch_into_update), as it can't be told
// otherwise after, so a quit a page stops leaves it to install on quit alone. Sparkle asks this
// once, so Restart to Update, which About and the sidebar offer from now on, tries it again.
- (BOOL)updater:(SPUUpdater*)updater
    shouldPostponeRelaunchForUpdate:(SUAppcastItem*)item
                 untilInvokingBlock:(void (^)(void))installHandler {
  [self keepNotes:item];
  self.relaunch = installHandler;
  self.relaunchVersion = item.versionString;
  if (_stage != HOST_UPDATE_READY) [self tell:HOST_UPDATE_READY item:item];
  [self restartOnceWarm];
  return YES;
}

// The restart waits for macOS's check of the new version, up to 15 s.
- (void)restartOnceWarm {
  __block BOOL restarted = NO;
  dispatch_block_t restart = ^{
    if (restarted) return;
    restarted = YES;
    if (self.restart) self.restart();
  };
  [self warm:self.relaunchVersion then:restart];
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, kInstallWait * NSEC_PER_SEC),
                 dispatch_get_main_queue(), restart);
}

// A scheduled check shows no alert of Sparkle's own (#678): the sidebar's banner says what it
// found, quietly, and Sparkle's window shows it only once the user asks, from the banner or Check
// for Updates…, which Sparkle shows in immediate focus. An update Sparkle downloads by itself
// becomes ready through willInstallUpdateOnQuit; one it won't is the banner's Update available.
- (BOOL)supportsGentleScheduledUpdateReminders {
  return YES;
}

- (BOOL)standardUserDriverShouldHandleShowingScheduledUpdate:(SUAppcastItem*)update
                                         andInImmediateFocus:(BOOL)immediateFocus {
  return NO;
}

- (void)standardUserDriverWillHandleShowingUpdate:(BOOL)handleShowingUpdate
                                        forUpdate:(SUAppcastItem*)update
                                            state:(SPUUserUpdateState*)state {
  if (handleShowingUpdate) {
    Log([NSString stringWithFormat:@"update %@ shown", update.versionString]);
    return;
  }
  Log([NSString stringWithFormat:@"update %@ in the banner", update.versionString]);
  if (state.stage == SPUUserUpdateStageNotDownloaded) [self tell:HOST_UPDATE_AVAILABLE item:update];
}

// Any of Sparkle's alerts, an error's too.
- (void)standardUserDriverWillShowModalAlert {
  Log(@"update alert shows");
}

- (void)standardUserDriverDidReceiveUserAttentionForUpdate:(SUAppcastItem*)update {
  Log([NSString stringWithFormat:@"update %@ alert has the keyboard", update.versionString]);
}

// The notes the alert shows, as they are: the log has their first line.
- (NSAttributedString*)standardUserDriverWillShowReleaseNotesText:(NSAttributedString*)notes
                                                        forUpdate:(SUAppcastItem*)update
                                         withBundleDisplayVersion:(NSString*)displayVersion
                                                    bundleVersion:(NSString*)version {
  NSString* first = [notes.string componentsSeparatedByString:@"\n"].firstObject;
  Log([NSString stringWithFormat:@"update %@ notes %@", update.versionString, first]);
  return nil;
}

// The user closed Sparkle's window, skipped the version or installed it: an update the banner
// offered goes with it.
- (void)standardUserDriverWillFinishUpdateSession {
  if (_stage == HOST_UPDATE_AVAILABLE) [self tell:HOST_UPDATE_IDLE item:nil];
}

@end

namespace {
Updates* g_updates;
}

bool host_keep_keychain_key(void) {
  NSString* name = [NSBundle.mainBundle objectForInfoDictionaryKey:@"CFBundleName"];
  NSString* signature = OwnSignature();
  NSUserDefaults* defaults = NSUserDefaults.standardUserDefaults;
  // Under a signature the key was read under, macOS doesn't ask, so launch doesn't wait for it.
  if (name.length == 0 || !signature || [[defaults stringForKey:kKeySignature] isEqual:signature]) {
    return true;
  }
  auto done = ^{
    [defaults setObject:signature forKey:kKeySignature];
    [defaults removeObjectForKey:kKeyTries];
  };
  NSInteger tries = [[defaults dictionaryForKey:kKeyTries][signature] integerValue];
  for (;;) {
    NSData* key = nil;
    bool stranded = false;
    OSStatus status = ReadKey(name, &key, &stranded);
    // Not there: a first launch, and Chromium makes one.
    if (status == errSecItemNotFound) {
      done();
      return true;
    }
    if (status == errSecSuccess && tries >= kKeyTriesLimit) {
      Log([NSString stringWithFormat:@"the cookie key is left as it is, after %d tries to regrant it",
                                     (int)tries]);
      // The item holds the key, so a spare a try left is only a copy.
      SecItemDelete((__bridge CFDictionaryRef)KeyItem(name, kSpare));
      done();
      return true;
    }
    if (status == errSecSuccess) {
      // Counted first, so a launch that stops partway counts too.
      [defaults setObject:@{signature : @(++tries)} forKey:kKeyTries];
      OSStatus regranted = Regrant(name, key);
      Log([NSString stringWithFormat:@"the cookie key regranted to %@: %d", signature, (int)regranted]);
      if (regranted == errSecSuccess) {
        done();
        return true;
      }
      // The item still has the key, or the spare does, which goes back.
      status = ReadKey(name, &key, &stranded);
      if (status == errSecSuccess) return true;
    }
    Log([NSString stringWithFormat:@"the cookie key couldn't be read: %d", (int)status]);
    // AppKit isn't up before Chromium starts, so the alert is the system's.
    NSString* title = [NSString stringWithFormat:@"%@ needs its key in your Keychain", name];
    NSString* message =
        stranded
            ? [NSString stringWithFormat:@"%@ keeps the key to your cookies and saved passwords in "
                                         @"your Keychain. It couldn't put the key back as \"%@ "
                                         @"Safe Storage\", and keeps it as \"%@ Safe Storage "
                                         @"(moving)\" until it can. Without it, websites would "
                                         @"sign you out. Make sure your login keychain is "
                                         @"unlocked in Keychain Access, then try again.",
                                         name, name, name]
            : [NSString stringWithFormat:@"%@ keeps the key to your cookies and saved passwords in "
                                         @"your Keychain, as \"%@ Safe Storage\". Without it, "
                                         @"websites would sign you out. When macOS asks, enter "
                                         @"your password and choose Always Allow.",
                                         name, name];
    CFOptionFlags response = kCFUserNotificationOtherResponse;
    CFUserNotificationDisplayAlert(0, kCFUserNotificationCautionAlertLevel, nullptr, nullptr,
                                   nullptr, (__bridge CFStringRef)title,
                                   (__bridge CFStringRef)message, CFSTR("Try Again"),
                                   CFSTR("Quit"), nullptr, &response);
    if (response != kCFUserNotificationDefaultResponse) return false;
  }
}

bool host_updates_available(void) {
  NSBundle* main = NSBundle.mainBundle;
  NSString* sparkle = [main.privateFrameworksPath stringByAppendingPathComponent:@"Sparkle.framework"];
  return [main objectForInfoDictionaryKey:@"SUFeedURL"] &&
         [main objectForInfoDictionaryKey:@"SUPublicEDKey"] &&
         [NSFileManager.defaultManager fileExistsAtPath:sparkle];
}

bool host_updated_by_store(void) {
  return false;
}

// build.sh marks a nightly's Info.plist.
bool host_nightly_build(void) {
  return [[NSBundle.mainBundle objectForInfoDictionaryKey:@"NightlyBuild"] boolValue];
}

bool host_nightly_feed(void) {
  return host_updates_available() && NightlyFeed();
}

// Sparkle asks for the feed at each check (feedURLStringForUpdater:).
void host_follow_nightly(bool nightly) {
  g_follow_nightly = nightly;
}

void host_start_updater(void (*restart)(void), void (*changed)(int stage, const char* version)) {
  if (g_updates || !host_updates_available()) return;
  NSString* path =
      [NSBundle.mainBundle.privateFrameworksPath stringByAppendingPathComponent:@"Sparkle.framework"];
  NSError* error = nil;
  // Loaded here, not linked, so builds without it run and launch never loads it.
  if (![[NSBundle bundleWithPath:path] loadAndReturnError:&error]) {
    Log([NSString stringWithFormat:@"no updates: %@", error.localizedDescription]);
    return;
  }
  // Sparkle reads these as it starts, over the Info.plist's, so an updater the user turned off asks
  // the feed nothing, not even the check a launch past its due time would make.
  [NSUserDefaults.standardUserDefaults setBool:g_automatic forKey:@"SUEnableAutomaticChecks"];
  [NSUserDefaults.standardUserDefaults setBool:g_automatic forKey:@"SUAutomaticallyUpdate"];
  g_updates = [[Updates alloc] init];
  g_updates.changed = changed;
  g_updates.restart = restart;
  g_updates.controller = [[NSClassFromString(@"SPUStandardUpdaterController") alloc]
      initWithStartingUpdater:YES
              updaterDelegate:g_updates
           userDriverDelegate:g_updates];
  Log(@"updater started");
}

void host_check_for_updates(void) {
  [g_updates.controller checkForUpdates:nil];
}

void host_set_automatic_update_checks(bool on) {
  g_automatic = on;
  Log([NSString stringWithFormat:@"update checks automatic %@", on ? @"on" : @"off"]);
  if (!g_updates) return;
  // Sparkle resets its cycle as either changes: on, it schedules from the last check again.
  SPUUpdater* updater = g_updates.controller.updater;
  updater.automaticallyChecksForUpdates = on;
  updater.automaticallyDownloadsUpdates = on;
}

void host_check_for_updates_quietly(void) {
  SPUUpdater* updater = g_updates.controller.updater;
  // A background check does nothing while Sparkle shows an update, which this brings forward.
  if (updater.sessionInProgress) {
    [g_updates.controller checkForUpdates:nil];
  } else {
    [updater checkForUpdatesInBackground];
  }
}

// Sparkle's install now comes back through shouldPostponeRelaunchForUpdate, which restarts.
void host_install_update(void) {
  if (g_updates.relaunch) {
    Log(@"update installs now");
    [g_updates restartOnceWarm];
  } else if (g_updates.installNow) {
    Log(@"update installs now");
    g_updates.installNow();
  }
}

bool host_relaunch_into_update(void) {
  dispatch_block_t relaunch = g_updates.relaunch;
  if (!relaunch) return false;
  g_updates.relaunch = nil;
  Log(@"update relaunches once installed");
  relaunch();
  return true;
}

void host_take_update_notes(void (*notes)(const char* url)) {
  NSUserDefaults* defaults = NSUserDefaults.standardUserDefaults;
  NSDictionary* kept = [defaults dictionaryForKey:kNotes];
  // Kept until the update they're for is the one running.
  if (![kept[@"version"] isEqual:[NSBundle.mainBundle objectForInfoDictionaryKey:@"CFBundleVersion"]]) {
    return;
  }
  [defaults removeObjectForKey:kNotes];
  NSString* url = kept[@"url"];
  if ([url isKindOfClass:NSString.class]) notes(url.UTF8String);
}

// Sparkle installs a ready update as the app quits by itself.
void host_finish_update(void) {}
