#include "browser_core/held_keys.h"

#include <cstdint>
#include <cstdio>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "components/input/native_web_keyboard_event.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/render_widget_host.h"
#include "content/public/browser/render_widget_host_observer.h"
#include "content/public/browser/web_contents.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "third_party/blink/public/common/input/web_keyboard_event.h"
#include "third_party/blink/public/mojom/input/input_event_result.mojom-shared.h"

namespace browser_core::held_keys {

namespace {

// The page answers the shortcut and the field it opens takes the keyboard well within it, on any
// machine, so only a page that never answers, or a field that never opens, lasts it. Safari's
// engine's pages say only when they don't keep a key, so one that keeps it lasts it too.
constexpr base::TimeDelta kHoldFor = base::Seconds(1);

// Which key a hold waits on the answer to.
struct Key {
  base::TimeTicks time;
  int code = 0;

  bool Is(const blink::WebInputEvent& event) const {
    return event.GetType() == blink::WebInputEvent::Type::kRawKeyDown &&
           event.TimeStamp() == time &&
           static_cast<const blink::WebKeyboardEvent&>(event).dom_code == code;
  }
};

void Unclaimed();

// Hears the page's answer to the key a hold waits on, from the widget it went to.
class Answer : public content::RenderWidgetHost::InputEventObserver,
               public content::RenderWidgetHostObserver {
 public:
  void Await(content::RenderWidgetHost* widget, Key key) {
    Forget();
    key_ = key;
    if (widget) {
      acks_.Observe(widget);
      gone_.Observe(widget);
    }
  }

  void Forget() {
    acks_.Reset();
    gone_.Reset();
    key_.reset();
    ++generation_;
  }

  // Whether it waits on the answer to |event|.
  bool Awaits(const blink::WebInputEvent& event) const { return key_ && key_->Is(event); }

  void OnInputEventAck(const content::RenderWidgetHost&,
                       blink::mojom::InputEventResultSource,
                       blink::mojom::InputEventResultState,
                       const blink::WebInputEvent& event) override {
    if (!Awaits(event)) {
      return;
    }
    acks_.Reset();
    gone_.Reset();
    // Chromium asks the app about a key the page didn't keep right after this (Answered), unless
    // the page is hidden. A key the page kept, or one the app wasn't asked about, has no field.
    UnclaimedSoon();
  }

  // Ends the hold as unclaimed after the task running, unless it has an answer by then. The keys
  // go to the page from a task of their own, not from inside Chromium's answer to the key.
  void UnclaimedSoon() {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](const Answer* answer, unsigned generation) {
                         if (answer->generation_ == generation && answer->key_) {
                           Unclaimed();
                         }
                       },
                       base::Unretained(this), generation_));
  }

  // A page replaced before it answered, as one a navigation leaves, never will.
  void RenderWidgetHostDestroyed(content::RenderWidgetHost*) override {
    acks_.Reset();
    gone_.Reset();
    UnclaimedSoon();
  }

 private:
  std::optional<Key> key_;
  // Counts the keys waited on, so a check made for one isn't the next one's.
  unsigned generation_ = 0;
  base::ScopedObservation<content::RenderWidgetHost, content::RenderWidgetHost::InputEventObserver>
      acks_{this};
  base::ScopedObservation<content::RenderWidgetHost, content::RenderWidgetHostObserver> gone_{this};
};

struct State {
  // The page whose keys are held, or 0.
  bc_id tab = 0;
  // The core holds them, rather than the host.
  bool in_core = false;
  // The presses held, in the order typed.
  std::vector<input::NativeWebKeyboardEvent> keys;
  // Their keys, whose releases are held too, so the page never gets a release before its press.
  std::set<int> down;
  Answer answer;
  base::OneShotTimer timer;
};

Asking g_asking;

State& Held() {
  static base::NoDestructor<State> state;
  return *state;
}

// Ends the hold. Returns the page whose keys were held and the presses.
std::pair<bc_id, std::vector<input::NativeWebKeyboardEvent>> End() {
  State& held = Held();
  held.timer.Stop();
  held.answer.Forget();
  held.down.clear();
  return {std::exchange(held.tab, 0), std::move(held.keys)};
}

// The page's focused frame's widget, which a key goes to, or null once the page went.
content::RenderWidgetHost* WidgetOf(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  if (!contents) {
    return nullptr;
  }
  content::RenderFrameHost* frame = contents->GetFocusedFrame();
  return (frame ? frame : contents->GetPrimaryMainFrame())->GetRenderWidgetHost();
}

// No field takes the keys: the page kept the shortcut, or nothing used it. So the page gets them as
// typed, each press with the character it types and its release, which never reached the page:
// Chromium drops what follows a press the browser kept. A host holding them is told to give them.
void Unclaimed() {
  if (!Held().tab) {
    return;
  }
  auto [tab, keys] = End();
  if (const auto unclaimed = Host().held_keys_unclaimed) {
    unclaimed(Host().context);
  }
  if (!keys.empty()) {
    fprintf(stderr, "browser: held %zu keys went to the page\n", keys.size());
  }
  for (const input::NativeWebKeyboardEvent& press : keys) {
    // A key can close the page.
    content::RenderWidgetHost* widget = WidgetOf(tab);
    if (!widget) {
      return;
    }
    widget->ForwardKeyboardEvent(press);
    const char16_t typed = press.text[0];
    const int shortcut = blink::WebInputEvent::kControlKey | blink::WebInputEvent::kMetaKey;
    if (typed >= 0x20 && typed != 0x7f && !(press.GetModifiers() & shortcut)) {
      input::NativeWebKeyboardEvent character = press;
      character.SetType(blink::WebInputEvent::Type::kChar);
      character.windows_key_code = typed;
      widget->ForwardKeyboardEvent(character);
    }
    input::NativeWebKeyboardEvent release = press;
    release.SetType(blink::WebInputEvent::Type::kKeyUp);
    widget->ForwardKeyboardEvent(release);
  }
}

}  // namespace

base::AutoReset<Asking> AskingAbout(bc_id tab, const input::NativeWebKeyboardEvent& key) {
  return base::AutoReset<Asking>(&g_asking, Asking{tab, &key});
}

void Hold(bool in_core) {
  if (!g_asking.tab || !g_asking.key) {
    return;
  }
  // Another page's hold, which can't be the field's, ends as if nothing took its keys.
  if (Held().tab && Held().tab != g_asking.tab) {
    Unclaimed();
  }
  State& held = Held();
  held.tab = g_asking.tab;
  held.in_core = in_core;
  held.answer.Await(WidgetOf(held.tab), {g_asking.key->TimeStamp(), g_asking.key->dom_code});
  held.timer.Start(FROM_HERE, kHoldFor, base::BindOnce(&Unclaimed));
}

bool Holds(bc_id tab, const input::NativeWebKeyboardEvent& event) {
  State& held = Held();
  if (!held.tab || tab != held.tab || !held.in_core) {
    return false;
  }
  switch (event.GetType()) {
    case blink::WebInputEvent::Type::kRawKeyDown:
      held.keys.push_back(event);
      held.down.insert(event.dom_code);
      return true;
    case blink::WebInputEvent::Type::kKeyUp:
      return held.down.contains(event.dom_code);
    default:
      // The character a press the page got types, such as the shortcut's.
      return false;
  }
}

void Answered(bc_id tab, const input::NativeWebKeyboardEvent& key, bool used) {
  State& held = Held();
  if (tab != held.tab || !held.answer.Awaits(key)) {
    return;
  }
  if (!used) {
    held.answer.UnclaimedSoon();
    return;
  }
  held.answer.Forget();
  if (!held.in_core) {
    // The field opens, and the host holds the keys for it from here.
    End();
  }
  // Otherwise they're held until the field has the keyboard (Give), or for kHoldFor.
}

void Give(void (*take)(void* context, const bc_key* keys, size_t count), void* context) {
  auto [tab, keys] = End();
  if (keys.empty() || !take) {
    return;
  }
  std::vector<std::string> names;
  std::vector<uint32_t> modifiers(keys.size());
  for (size_t i = 0; i < keys.size(); i++) {
    names.push_back(Tab::KeyName(keys[i], &modifiers[i]));
  }
  // Not the page's: they go to the app's field.
  std::vector<bc_key> given;
  for (size_t i = 0; i < keys.size(); i++) {
    given.push_back({names[i].c_str(), modifiers[i], false, ""});
  }
  take(context, given.data(), given.size());
}

}  // namespace browser_core::held_keys
