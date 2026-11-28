#include "browser_core/platform/keyboard.h"

#import <AppKit/AppKit.h>
#include <Carbon/Carbon.h>

#include "base/apple/foundation_util.h"
#include "base/apple/owned_objc.h"
#include "base/apple/scoped_cftyperef.h"
#include "base/functional/bind.h"
#include "base/location.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "components/input/native_web_keyboard_event.h"
#include "ui/events/event.h"
#include "ui/events/keycodes/dom/dom_keyboard_layout_map.h"
#include "ui/events/keycodes/keyboard_code_conversion_mac.h"

// What AppKit's key bindings make of a key, read by having them interpret it for nothing that
// edits text.
@interface BrowserCoreKeyBinding : NSResponder
@property(nonatomic) BOOL noop;
@end

@implementation BrowserCoreKeyBinding
@synthesize noop = _noop;
- (void)doCommandBySelector:(SEL)selector {
  _noop = selector == @selector(noop:);
}
- (void)insertText:(id)text {
}
@end

namespace browser_core::platform {

namespace {

// The split keys' layout, made again after the system says the keyboard's input source changed.
struct SplitLayouts {
  SplitLayout layout;
  bool stale = true;
  bool hearing = false;
};

SplitLayouts& Split() {
  static base::NoDestructor<SplitLayouts> layouts;
  return *layouts;
}

void LayoutChanged(CFNotificationCenterRef, void*, CFNotificationName, const void*, CFDictionaryRef) {
  Split().stale = true;
}

}  // namespace

const SplitLayout& SplitLayoutOf(const input::NativeWebKeyboardEvent&) {
  SplitLayouts& split = Split();
  // Heard from the first key that asks, once; the notice comes only when the user changes layout.
  if (!split.hearing) {
    CFNotificationCenterAddObserver(CFNotificationCenterGetDistributedCenter(), &split,
                                    &LayoutChanged, kTISNotifySelectedKeyboardInputSourceChanged,
                                    nullptr, CFNotificationSuspensionBehaviorDeliverImmediately);
    split.hearing = true;
  }
  if (split.stale) {
    // The current Latin layout's keys. GPUI's patch reads the current layout's for the UI, which
    // on a Latin layout is the same one.
    split.layout = SplitLayout(ui::GenerateDomKeyboardLayoutMap());
    split.stale = false;
  }
  return split.layout;
}

void StopHearingLayouts() {
  SplitLayouts& split = Split();
  if (split.hearing) {
    CFNotificationCenterRemoveObserver(CFNotificationCenterGetDistributedCenter(), &split,
                                       kTISNotifySelectedKeyboardInputSourceChanged, nullptr);
    split.hearing = false;
  }
}

std::string LatinKey(const input::NativeWebKeyboardEvent& event) {
  NSEvent* key = event.os_event.Get();
  if (!key || key.type != NSEventTypeKeyDown) {
    return std::string();
  }
  base::apple::ScopedCFTypeRef<TISInputSourceRef> layout(TISCopyCurrentKeyboardLayoutInputSource());
  CFBooleanRef latin_layout =
      layout ? base::apple::CFCast<CFBooleanRef>(TISGetInputSourceProperty(
                   layout.get(), kTISPropertyInputSourceIsASCIICapable))
             : nullptr;
  if (!latin_layout || CFBooleanGetValue(latin_layout)) {
    return std::string();
  }
  // The Latin layout the user last typed with, which is what AppKit's menus match by.
  base::apple::ScopedCFTypeRef<TISInputSourceRef> latin(
      TISCopyCurrentASCIICapableKeyboardLayoutInputSource());
  if (!latin) {
    return std::string();
  }
  // ⇧ alone, as Chromium names a page's key by its character with ⇧: ⌘ and ⌥ would give the
  // layout's other characters.
  const UInt32 shift = (key.modifierFlags & NSEventModifierFlagShift) ? (shiftKey >> 8) & 0xFF : 0;
  UInt32 dead = 0;
  const UniChar typed = ui::TranslatedUnicodeCharFromKeyCode(
      latin.get(), key.keyCode, kUCKeyActionDown, shift, LMGetKbdType(), &dead);
  // Printable ASCII only: a Latin layout's own letters, such as "é", and its function keys, which
  // Chromium names by the key, come from the page's name.
  if (typed <= 0x20 || typed >= 0x7F) {
    return std::string();
  }
  return std::string(1, static_cast<char>(typed));
}

std::unique_ptr<ui::KeyEvent> KeyBehindCharacter(const ui::KeyEvent& character) {
  // views makes the character in AppKit's dispatch of the key, with no NSEvent of its own.
  NSEvent* key = NSApp.currentEvent;
  if (!character.is_char() || character.HasNativeEvent() || !key ||
      key.type != NSEventTypeKeyDown || key.characters.length != 1 ||
      [key.characters characterAtIndex:0] != character.GetCharacter()) {
    return nullptr;
  }
  return std::make_unique<ui::KeyEvent>(base::apple::OwnedNSEvent(key));
}

struct UnheardKeys::Watch {
  // A key going down in AppKit's queue, before AppKit dispatches it.
  void GoingDown(NSEvent* event) {
    const NSEventModifierFlags held = event.modifierFlags;
    if (event.window != window || !(held & NSEventModifierFlagControl) ||
        (held & NSEventModifierFlagCommand)) {
      return;
    }
    BrowserCoreKeyBinding* binding = [[BrowserCoreKeyBinding alloc] init];
    [binding interpretKeyEvents:@[ event ]];
    if (!binding.noop) {
      return;
    }
    going_down = event;
    menu_took = false;
    // Run once AppKit has dispatched the key, as the next task.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Watch::Dispatched, weak.GetWeakPtr(),
                                  base::apple::OwnedNSEvent(event)));
  }

  // AppKit has dispatched |event|: it's unheard unless views heard it or a menu took it.
  void Dispatched(base::apple::OwnedNSEvent event) {
    if (going_down != event.Get()) {
      return;
    }
    going_down = nil;
    if (menu_took) {
      return;
    }
    const ui::KeyEvent key(event);
    // Run from a copy: |unheard| can close the dialog, and this with it.
    const base::RepeatingCallback<void(const ui::KeyEvent&)> run = unheard;
    run.Run(key);
  }

  NSWindow* __weak window = nil;
  id monitor = nil;
  id menus = nil;
  NSEvent* going_down = nil;
  bool menu_took = false;
  base::RepeatingCallback<void(const ui::KeyEvent&)> unheard;
  base::WeakPtrFactory<Watch> weak{this};
};

UnheardKeys::UnheardKeys(gfx::NativeWindow window,
                         base::RepeatingCallback<void(const ui::KeyEvent&)> unheard)
    : watch_(std::make_unique<Watch>()) {
  Watch* watch = watch_.get();
  watch->window = window.GetNativeNSWindow();
  watch->unheard = std::move(unheard);
  watch->monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown
                                                         handler:^NSEvent*(NSEvent* event) {
                                                           watch->GoingDown(event);
                                                           return event;
                                                         }];
  watch->menus = [NSNotificationCenter.defaultCenter
      addObserverForName:NSMenuDidSendActionNotification
                  object:nil
                   queue:nil
              usingBlock:^(NSNotification*) {
                if (watch->going_down) {
                  watch->menu_took = true;
                }
              }];
}

UnheardKeys::~UnheardKeys() {
  [NSEvent removeMonitor:watch_->monitor];
  [NSNotificationCenter.defaultCenter removeObserver:watch_->menus];
}

void UnheardKeys::Heard() {
  // Only from the key going down: a key going up or another in the queue isn't it.
  if (watch_->going_down && NSApp.currentEvent == watch_->going_down) {
    watch_->going_down = nil;
  }
}

}  // namespace browser_core::platform
