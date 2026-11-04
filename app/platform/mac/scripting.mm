// AppleScript (ilo.sdef, #704). AppKit's Cocoa scripting resolves a script's `tab 2 of window 1`
// through the keys ilo.sdef names, on the objects here, which hold an id each and nothing else:
// every read and every command goes to the app (host_scripting), which answers from what its UI
// shows and carries out a command as a click would, and logs each Apple Event once.

#import <Cocoa/Cocoa.h>

#include <libproc.h>

#include <algorithm>
#include <string>
#include <vector>

#include "../host.h"

namespace {

const host_scripting* g_scripting = nullptr;

NSAppleEventDescriptor* CurrentEvent() {
  return NSAppleEventManager.sharedAppleEventManager.currentAppleEvent;
}

pid_t SenderPID() {
  return [CurrentEvent() attributeDescriptorForKeyword:keySenderPIDAttr].int32Value;
}

// The Apple Event handled now, as its sender's pid and its return ID.
uint64_t EventID() {
  const uint64_t pid = static_cast<uint32_t>(SenderPID());
  return pid << 32 | static_cast<uint32_t>(CurrentEvent().returnID);
}

NSString* AppName(pid_t pid) {
  return [NSRunningApplication runningApplicationWithProcessIdentifier:pid].localizedName;
}

// Who sent it, by its app's name, or a tool's with the app it runs in, as osascript in Terminal.
std::string Sender() {
  const pid_t pid = SenderPID();
  if (NSString* name = AppName(pid)) {
    return name.UTF8String;
  }
  char tool[2 * MAXCOMLEN + 1] = "";
  if (proc_name(pid, tool, sizeof(tool)) <= 0) {
    return "an app";
  }
  pid_t parent = pid;
  for (int level = 0; level < 16; level++) {
    proc_bsdinfo info;
    if (proc_pidinfo(parent, PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != sizeof(info)) {
      break;
    }
    parent = static_cast<pid_t>(info.pbi_ppid);
    if (parent <= 1) {
      break;
    }
    if (NSString* app = AppName(parent)) {
      return std::string(tool) + " in " + app.UTF8String;
    }
  }
  return tool;
}

// The script's command fails with |why|, as macOS fails one it may not send.
void Refuse(NSString* why) {
  NSScriptCommand* command = [NSScriptCommand currentCommand];
  command.scriptErrorNumber = errAEEventNotPermitted;
  command.scriptErrorString = why;
}

// Whether scripts may read, which the app asks Settings › Agents' switch and logs.
bool Allowed() {
  if (!g_scripting) {
    return false;
  }
  if (g_scripting->allowed(EventID(), Sender().c_str(), "read")) {
    return true;
  }
  Refuse(@"Scripts are off in ilo's Settings › Agents.");
  return false;
}

NSString* Text(const std::vector<char>& buffer) {
  return [NSString stringWithUTF8String:buffer.data()] ?: @"";
}

// What |read| copies for |id|, as bc_space_name copies.
NSString* Read(size_t (*read)(uint64_t, char*, size_t), uint64_t id) {
  std::vector<char> buffer(1024);
  const size_t length = read(id, buffer.data(), buffer.size());
  if (length >= buffer.size()) {
    buffer.resize(length + 1);
    read(id, buffer.data(), buffer.size());
  }
  return Text(buffer);
}

template <typename List>
std::vector<uint64_t> Ids(List list) {
  std::vector<uint64_t> ids(64);
  size_t count = list(ids.data(), ids.size());
  if (count > ids.size()) {
    ids.resize(count);
    count = list(ids.data(), ids.size());
  }
  ids.resize(std::min(count, ids.size()));
  return ids;
}

// A command's why, if the app refused it.
struct Why {
  std::vector<char> buffer = std::vector<char>(512, 0);
  NSString* text() const { return Text(buffer); }
};

}  // namespace

void host_set_scripting(const host_scripting* scripting) {
  g_scripting = scripting;
}

// What a script names: an id, in its container under the key ilo.sdef gives.
@interface ILOScriptObject : NSObject
@property(nonatomic, readonly) uint64_t ident;
@property(nonatomic, readonly, strong) id container;
- (instancetype)initWithID:(uint64_t)ident container:(id)container key:(NSString*)key;
@end

@interface ILOScriptWindow : ILOScriptObject
@end

@interface ILOScriptSpace : ILOScriptObject
@property(nonatomic, readonly) uint64_t window;
@end

@interface ILOScriptTab : ILOScriptObject
@end

@implementation ILOScriptObject {
  NSString* _key;
}

- (instancetype)initWithID:(uint64_t)ident container:(id)container key:(NSString*)key {
  if ((self = [super init])) {
    _ident = ident;
    _container = container;
    _key = key;
  }
  return self;
}

- (NSNumber*)uniqueID {
  return @(_ident);
}

- (NSScriptObjectSpecifier*)objectSpecifier {
  NSScriptClassDescription* kind = static_cast<NSScriptClassDescription*>(
      [NSScriptClassDescription classDescriptionForClass:[_container class]]);
  NSScriptObjectSpecifier* within =
      _container == NSApp ? nil : [(ILOScriptObject*)_container objectSpecifier];
  return [[NSUniqueIDSpecifier alloc] initWithContainerClassDescription:kind
                                                     containerSpecifier:within
                                                                    key:_key
                                                               uniqueID:@(_ident)];
}

@end

@implementation ILOScriptWindow

- (NSNumber*)orderedIndex {
  if (!Allowed()) {
    return @0;
  }
  const std::vector<uint64_t> windows = Ids(g_scripting->windows);
  const auto at = std::find(windows.begin(), windows.end(), self.ident);
  return @(at == windows.end() ? 0 : at - windows.begin() + 1);
}

- (NSString*)name {
  return Allowed() ? Read(g_scripting->window_name, self.ident) : @"";
}

- (ILOScriptSpace*)activeSpace {
  const uint64_t space = Allowed() ? g_scripting->window_space(self.ident) : 0;
  return space ? [[ILOScriptSpace alloc] initWithID:space container:self key:@"iloSpaces"] : nil;
}

- (ILOScriptTab*)activeTab {
  const uint64_t tab = Allowed() ? g_scripting->window_tab(self.ident) : 0;
  return tab ? [[ILOScriptTab alloc] initWithID:tab container:self key:@"iloTabs"] : nil;
}

- (NSArray<ILOScriptSpace*>*)iloSpaces {
  NSMutableArray* spaces = [NSMutableArray array];
  if (!Allowed()) {
    return spaces;
  }
  for (uint64_t space : Ids(g_scripting->spaces)) {
    [spaces addObject:[[ILOScriptSpace alloc] initWithID:space container:self key:@"iloSpaces"]];
  }
  return spaces;
}

- (NSArray<ILOScriptTab*>*)iloTabs {
  NSMutableArray* tabs = [NSMutableArray array];
  if (!Allowed()) {
    return tabs;
  }
  const uint64_t window = self.ident;
  for (uint64_t tab : Ids([window](uint64_t* ids, size_t capacity) {
         return g_scripting->tabs(window, 0, ids, capacity);
       })) {
    [tabs addObject:[[ILOScriptTab alloc] initWithID:tab container:self key:@"iloTabs"]];
  }
  return tabs;
}

@end

@implementation ILOScriptSpace

- (uint64_t)window {
  return [(ILOScriptObject*)self.container ident];
}

- (NSString*)name {
  return Allowed() ? Read(g_scripting->space_name, self.ident) : @"";
}

- (NSArray<ILOScriptTab*>*)iloTabs {
  NSMutableArray* tabs = [NSMutableArray array];
  if (!Allowed()) {
    return tabs;
  }
  const uint64_t window = self.window, space = self.ident;
  for (uint64_t tab : Ids([window, space](uint64_t* ids, size_t capacity) {
         return g_scripting->tabs(window, space, ids, capacity);
       })) {
    [tabs addObject:[[ILOScriptTab alloc] initWithID:tab container:self key:@"iloTabs"]];
  }
  return tabs;
}

- (id)handleFocusCommand:(NSScriptCommand*)command {
  Why why;
  if (!g_scripting || !g_scripting->focus_space(EventID(), Sender().c_str(), self.window,
                                                self.ident, why.buffer.data(), why.buffer.size())) {
    Refuse(g_scripting ? why.text() : @"Scripts are off in ilo's Settings › Agents.");
  }
  return nil;
}

@end

@implementation ILOScriptTab

- (NSString*)title {
  return Allowed() ? Read(g_scripting->tab_title, self.ident) : @"";
}

- (NSString*)URL {
  return Allowed() ? Read(g_scripting->tab_url, self.ident) : @"";
}

- (id)handleSelectCommand:(NSScriptCommand*)command {
  id container = self.container;
  const uint64_t window = [container isKindOfClass:[ILOScriptSpace class]]
                              ? [(ILOScriptSpace*)container window]
                              : [(ILOScriptObject*)container ident];
  Why why;
  if (!g_scripting || !g_scripting->select_tab(EventID(), Sender().c_str(), window, self.ident,
                                               why.buffer.data(), why.buffer.size())) {
    Refuse(g_scripting ? why.text() : @"Scripts are off in ilo's Settings › Agents.");
  }
  return nil;
}

@end

// The application's windows, front to back.
@interface NSApplication (ILOScripting)
- (NSArray<ILOScriptWindow*>*)iloWindows;
@end

@implementation NSApplication (ILOScripting)

- (NSArray<ILOScriptWindow*>*)iloWindows {
  NSMutableArray* windows = [NSMutableArray array];
  if (!Allowed()) {
    return windows;
  }
  for (uint64_t window : Ids(g_scripting->windows)) {
    [windows addObject:[[ILOScriptWindow alloc] initWithID:window container:NSApp
                                                       key:@"iloWindows"]];
  }
  return windows;
}

@end

// make new window, and make new tab with properties {URL:…} at a window, at a Space, or at the
// application, which opens as a link from another app does.
@interface ILOMakeCommand : NSCreateCommand
@end

@implementation ILOMakeCommand

- (id)performDefaultImplementation {
  if (!g_scripting) {
    Refuse(@"Scripts are off in ilo's Settings › Agents.");
    return nil;
  }
  NSString* kind = self.createClassDescription.className;
  const uint64_t event = EventID();
  const std::string sender = Sender();
  Why why;
  if ([kind isEqualToString:@"window"]) {
    const uint64_t window =
        g_scripting->new_window(event, sender.c_str(), why.buffer.data(), why.buffer.size());
    if (!window) {
      Refuse(why.text());
      return nil;
    }
    return [[[ILOScriptWindow alloc] initWithID:window container:NSApp key:@"iloWindows"]
        objectSpecifier];
  }
  if (![kind isEqualToString:@"tab"]) {
    Refuse(@"ilo makes windows and tabs only.");
    return nil;
  }
  id url = self.resolvedKeyDictionary[@"URL"];
  if (![url isKindOfClass:[NSString class]]) {
    Refuse(@"A new tab needs its URL, an http or https page.");
    return nil;
  }
  uint64_t window = 0, space = 0;
  id where = self.arguments[@"Location"];
  ILOScriptObject* container = nil;
  if ([where isKindOfClass:[NSPositionalSpecifier class]]) {
    NSPositionalSpecifier* position = where;
    [position evaluate];
    id inside = position.insertionContainer;
    if ([inside isKindOfClass:[ILOScriptSpace class]]) {
      container = inside;
      window = [(ILOScriptSpace*)inside window];
      space = [(ILOScriptSpace*)inside ident];
    } else if ([inside isKindOfClass:[ILOScriptWindow class]]) {
      container = inside;
      window = [(ILOScriptWindow*)inside ident];
    }
  }
  const uint64_t tab = g_scripting->new_tab(event, sender.c_str(), window, space,
                                            [url UTF8String], why.buffer.data(), why.buffer.size());
  if (!tab) {
    if (why.buffer[0]) {
      Refuse(why.text());
    }
    return nil;
  }
  if (!container) {
    return nil;
  }
  return [[[ILOScriptTab alloc] initWithID:tab container:container key:@"iloTabs"] objectSpecifier];
}

@end
