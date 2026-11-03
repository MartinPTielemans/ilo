#include "browser_core/agent_page.h"

#include <algorithm>
#include <deque>
#include <functional>
#include <map>
#include <string_view>
#include <utility>
#include <vector>

#include "base/containers/span.h"
#include "base/functional/bind.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "build/build_config.h"
#include "components/autofill/content/browser/content_autofill_client.h"
#include "components/autofill/core/browser/filling/filling_product.h"
#include "components/autofill/core/browser/suggestions/suggestion_hiding_reason.h"
#include "content/public/browser/devtools_agent_host.h"
#include "content/public/browser/host_zoom_map.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_entry.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "third_party/blink/public/common/page/page_zoom.h"
#include "ui/gfx/geometry/size.h"
#include "url/gurl.h"
#include "url/origin.h"
#include "url/url_constants.h"

namespace browser_core {

namespace {

constexpr base::TimeDelta kLoadTimeout = base::Seconds(15);
// What an input starts, such as a navigation or a menu, has started by then.
constexpr base::TimeDelta kSettle = base::Milliseconds(400);
// How long a hidden page keeps drawing after an agent's last action, for its next one.
constexpr base::TimeDelta kRest = base::Seconds(30);

// Select All's modifier: ⌘ on the Mac, Ctrl elsewhere, as DevTools' Input domain numbers them.
#if BUILDFLAG(IS_MAC)
constexpr int kSelectAllModifier = 4;
#else
constexpr int kSelectAllModifier = 2;
#endif

// The keys an agent may press, with their key codes and the text they type.
struct Key {
  std::string_view name;
  int code;
  std::string_view text;
};
constexpr Key kKeys[] = {
    {"Enter", 13, "\r"},   {"Tab", 9, ""},         {"Escape", 27, ""},  {"Backspace", 8, ""},
    {"Delete", 46, ""},    {"Space", 32, " "},     {"ArrowUp", 38, ""}, {"ArrowDown", 40, ""},
    {"ArrowLeft", 37, ""}, {"ArrowRight", 39, ""}, {"PageUp", 33, ""},  {"PageDown", 34, ""},
    {"Home", 36, ""},      {"End", 35, ""},
};

// Whether the node is a field, whose value is the user's: a text field of any kind, a select, a
// date, time or number input, or an editable region such as a mail's body.
bool IsField(const base::DictValue& node) {
  if (const base::ListValue* properties = node.FindList("properties")) {
    for (const base::Value& property : *properties) {
      const base::DictValue* dict = property.GetIfDict();
      const std::string* name = dict ? dict->FindString("name") : nullptr;
      if (name && *name == "editable") {
        return true;
      }
    }
  }
  const std::string* role = node.FindStringByDottedPath("role.value");
  if (!role) {
    return false;
  }
  static constexpr std::string_view kFields[] = {
      "textbox", "searchbox", "combobox",  "spinbutton",
      "date",    "datetime",  "inputtime", "colorwell",
  };
  const std::string lower = base::ToLowerASCII(*role);
  for (std::string_view field : kFields) {
    if (lower == field) {
      return true;
    }
  }
  return false;
}

// An element's id: the slot of the session its frame is in, above its DOM node's id there, as each
// renderer numbers its own nodes. The main frame's elements have their DOM nodes' ids.
int64_t ElementId(int slot, int node) {
  return (int64_t{slot} << 32) | static_cast<uint32_t>(node);
}
int SlotOfElement(int64_t element) {
  return static_cast<int>(element >> 32);
}
int NodeOfElement(int64_t element) {
  return static_cast<int>(element & 0xffffffff);
}

// The element id of a node of the tree, a double above an int's range.
std::optional<int64_t> ElementOf(const base::DictValue& node) {
  const std::optional<double> id = node.FindDouble("backendDOMNodeId");
  return id ? std::optional<int64_t>(static_cast<int64_t>(*id)) : std::nullopt;
}

// A frame's tree, read through the session at |slot|, with its ids made the snapshot's: |prefix|
// before each node's, as each frame's tree numbers its own, and each DOM node's its element's.
base::ListValue Tagged(base::ListValue nodes, const std::string& prefix, int slot) {
  for (base::Value& node : nodes) {
    base::DictValue* dict = node.GetIfDict();
    if (!dict) {
      continue;
    }
    for (const char* key : {"nodeId", "parentId"}) {
      if (const std::string* id = dict->FindString(key)) {
        dict->Set(key, prefix + *id);
      }
    }
    if (base::ListValue* children = dict->FindList("childIds")) {
      for (base::Value& child : *children) {
        if (const std::string* id = child.GetIfString()) {
          child = base::Value(prefix + *id);
        }
      }
    }
    if (const std::optional<int> backend = dict->FindInt("backendDOMNodeId")) {
      dict->Set("backendDOMNodeId", static_cast<double>(ElementId(slot, *backend)));
    }
  }
  return nodes;
}

void Report(bc_id tab, uint64_t request, std::string text, bool error) {
  if (Host().agent_page_done) {
    Host().agent_page_done(Host().context, tab, request, text.c_str(), error);
  }
}

}  // namespace

bool IsWebFrame(content::RenderFrameHost* frame) {
  const std::string& scheme =
      frame->GetLastCommittedOrigin().GetTupleOrPrecursorTupleIfOpaque().scheme();
  return scheme == url::kHttpScheme || scheme == url::kHttpsScheme;
}

base::ListValue AgentTree(base::ListValue nodes, const std::set<int64_t>& passwords) {
  std::map<std::string, const base::DictValue*> by_id;
  for (const base::Value& node : nodes) {
    const base::DictValue* dict = node.GetIfDict();
    const std::string* id = dict ? dict->FindString("nodeId") : nullptr;
    if (id) {
      by_id[*id] = dict;
    }
  }
  // Every node under a field, which goes, so no step down from another node reaches it either.
  std::set<std::string> under_fields;
  std::vector<std::string> steps;
  const auto children_of = [&steps](const base::DictValue& node) {
    if (const base::ListValue* children = node.FindList("childIds")) {
      for (const base::Value& child : *children) {
        if (const std::string* id = child.GetIfString()) {
          steps.push_back(*id);
        }
      }
    }
  };
  for (const auto& [id, node] : by_id) {
    if (IsField(*node)) {
      children_of(*node);
    }
  }
  while (!steps.empty()) {
    const std::string id = std::move(steps.back());
    steps.pop_back();
    if (!under_fields.insert(id).second) {
      continue;
    }
    if (auto found = by_id.find(id); found != by_id.end()) {
      children_of(*found->second);
    }
  }
  by_id.clear();
  base::ListValue kept;
  for (base::Value& node : nodes) {
    base::DictValue* dict = node.GetIfDict();
    const std::string* id = dict ? dict->FindString("nodeId") : nullptr;
    if (!id || under_fields.contains(*id)) {
      continue;
    }
    dict->Remove("value");
    if (IsField(*dict)) {
      dict->Remove("childIds");
    }
    if (base::ListValue* properties = dict->FindList("properties")) {
      properties->EraseIf([](const base::Value& property) {
        const base::DictValue* each = property.GetIfDict();
        const std::string* name = each ? each->FindString("name") : nullptr;
        return !name || *name == "valuetext";
      });
    }
    const std::optional<int64_t> backend = ElementOf(*dict);
    if (backend && passwords.contains(*backend)) {
      dict->Set("password", true);
    }
    kept.Append(std::move(node));
  }
  return kept;
}

AgentPage::AgentPage(content::WebContents* contents)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<AgentPage>(*contents),
      tab_(Tab::IdOf(contents)) {}

AgentPage::~AgentPage() {
  Detach();
}

void AgentPage::Act(uint64_t request, int action, int64_t element, std::string text) {
  jobs_.push_back({request, action, element, std::move(text)});
  Next();
}

// static
void AgentPage::SessionEnded(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  if (AgentPage* page = contents ? AgentPage::FromWebContents(contents) : nullptr) {
    page->LetGo();
  }
}

void AgentPage::Next() {
  if (job_ || jobs_.empty()) {
    return;
  }
  job_ = std::move(jobs_.front());
  jobs_.pop_front();
  rest_.Stop();
  // A hidden tab otherwise stops drawing, and its page then takes no input and gives no picture.
  if (!drawing_) {
    drawing_ = GetWebContents().IncrementCapturerCount(gfx::Size(), /*stay_hidden=*/true,
                                                       /*stay_awake=*/false,
                                                       /*is_activity=*/false);
  }
  AfterLoad(base::BindOnce(&AgentPage::Run, weak_factory_.GetWeakPtr()));
}

bool AgentPage::IsWebPage() {
  return GetWebContents().GetLastCommittedURL().SchemeIsHTTPOrHTTPS();
}

void AgentPage::Run() {
  const int action = job_->action;
  const bool history = action == BC_AGENT_PAGE_BACK || action == BC_AGENT_PAGE_FORWARD;
  if (!history && !IsWebPage()) {
    Fail("The tab's page isn't an http or https page, the only pages agents may use.");
    return;
  }
  // An element in a frame that's gone, or now holds what isn't a web page's, is off limits.
  const int slot = SlotOfElement(job_->element);
  content::RenderFrameHost* frame = slot ? FrameOf(slot) : nullptr;
  if (slot && (!frame || !IsWebFrame(frame))) {
    Fail("The element isn't on the page any more. Take a new snapshot.");
    return;
  }
  HideSuggestions();
  switch (action) {
    case BC_AGENT_PAGE_SNAPSHOT:
      Snapshot();
      return;
    case BC_AGENT_PAGE_CLICK:
      Click();
      return;
    case BC_AGENT_PAGE_TYPE:
    case BC_AGENT_PAGE_TYPE_SUBMIT:
      Type(action == BC_AGENT_PAGE_TYPE_SUBMIT);
      return;
    case BC_AGENT_PAGE_KEY:
      PressKey(job_->text, base::BindOnce(&AgentPage::Settle, weak_factory_.GetWeakPtr()));
      return;
    case BC_AGENT_PAGE_SCROLL_DOWN:
    case BC_AGENT_PAGE_SCROLL_UP:
      Scroll(action == BC_AGENT_PAGE_SCROLL_DOWN);
      return;
    case BC_AGENT_PAGE_BACK:
    case BC_AGENT_PAGE_FORWARD:
      History(action == BC_AGENT_PAGE_FORWARD);
      return;
    case BC_AGENT_PAGE_SCREENSHOT:
      Screenshot();
      return;
  }
  Fail("There's no such action.");
}

void AgentPage::AfterLoad(base::OnceClosure then) {
  if (!GetWebContents().IsLoading()) {
    std::move(then).Run();
    return;
  }
  loaded_ = std::move(then);
  // A page that never stops loading, as one that streams, is used as it is after a while.
  wait_.Start(FROM_HERE, kLoadTimeout,
              base::BindOnce(&AgentPage::DidStopLoading, base::Unretained(this)));
}

void AgentPage::Settle() {
  wait_.Start(
      FROM_HERE, kSettle,
      base::BindOnce(
          [](AgentPage* page) {
            page->AfterLoad(base::BindOnce(&AgentPage::Snapshot, page->weak_factory_.GetWeakPtr()));
          },
          base::Unretained(this)));
}

void AgentPage::Click() {
  Center(base::BindOnce(
      [](base::WeakPtr<AgentPage> page, std::optional<gfx::RectF> box) {
        if (!page) {
          return;
        }
        if (!box) {
          page->Fail("The element isn't on the page any more, or is hidden. Take a new snapshot.");
          return;
        }
        page->Point(*box);
        const std::pair<double, double> at(box->CenterPoint().x(), box->CenterPoint().y());
        base::WeakPtr<AgentPage> weak = page;
        page->Mouse("mouseMoved", at,
                    base::BindOnce(
                        [](base::WeakPtr<AgentPage> page, std::pair<double, double> at) {
                          if (!page) {
                            return;
                          }
                          page->Mouse(
                              "mousePressed", at,
                              base::BindOnce(
                                  [](base::WeakPtr<AgentPage> page, std::pair<double, double> at) {
                                    if (page) {
                                      page->Mouse("mouseReleased", at,
                                                  base::BindOnce(&AgentPage::Settle, page));
                                    }
                                  },
                                  page, at));
                        },
                        weak, at));
      },
      weak_factory_.GetWeakPtr()));
}

void AgentPage::Mouse(const char* type, std::pair<double, double> at, base::OnceClosure then) {
  base::DictValue params =
      base::DictValue().Set("type", type).Set("x", at.first).Set("y", at.second);
  if (std::string_view(type) != "mouseMoved") {
    params.Set("button", "left");
    params.Set("clickCount", 1);
  }
  Send("Input.dispatchMouseEvent", std::move(params),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, base::OnceClosure then, const base::DictValue* done) {
             if (!page) {
               return;
             }
             if (!done) {
               page->Fail("The page didn't take the click.");
               return;
             }
             std::move(then).Run();
           },
           weak_factory_.GetWeakPtr(), std::move(then)));
}

void AgentPage::Type(bool submit) {
  const int64_t element = job_->element;
  IsPassword(element,
             base::BindOnce(
                 [](base::WeakPtr<AgentPage> page, int64_t element, bool submit,
                    std::optional<bool> password) {
                   if (!page) {
                     return;
                   }
                   if (!password) {
                     page->Fail("The element isn't on the page any more. Take a new snapshot.");
                     return;
                   }
                   if (*password) {
                     page->Fail(
                         "Password fields are off limits to agents: signing in is the "
                         "user's to do.");
                     return;
                   }
                   // Where it is, for the agent's pointer, then the typing, which focusing
                   // scrolls to whether or not it has a box.
                   page->Center(base::BindOnce(
                       [](base::WeakPtr<AgentPage> page, int64_t element, bool submit,
                          std::optional<gfx::RectF> box) {
                         if (!page) {
                           return;
                         }
                         if (box) {
                           page->Point(*box);
                         }
                         page->Focus(element, submit);
                       },
                       page, element, submit));
                 },
                 weak_factory_.GetWeakPtr(), element, submit));
}

void AgentPage::Focus(int64_t element, bool submit) {
  // Focus moves to the element's frame, where the main frame's keys then go.
  Send("DOM.focus", base::DictValue().Set("backendNodeId", NodeOfElement(element)),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, bool submit, const base::DictValue* focused) {
             if (!page) {
               return;
             }
             if (!focused) {
               page->Fail("The element can't take typing. Take a new snapshot.");
               return;
             }
             // What the field holds is selected, so the text replaces it.
             const base::DictValue select = base::DictValue()
                                                .Set("key", "a")
                                                .Set("code", "KeyA")
                                                .Set("windowsVirtualKeyCode", 65)
                                                .Set("modifiers", kSelectAllModifier);
             page->Send("Input.dispatchKeyEvent",
                        select.Clone()
                            .Set("type", "rawKeyDown")
                            .Set("commands", base::ListValue().Append("selectAll")),
                        base::DoNothing());
             page->Send("Input.dispatchKeyEvent", select.Clone().Set("type", "keyUp"),
                        base::DoNothing());
             page->Send(
                 "Input.insertText", base::DictValue().Set("text", page->job_->text),
                 base::BindOnce(
                     [](base::WeakPtr<AgentPage> page, bool submit, const base::DictValue* typed) {
                       if (!page) {
                         return;
                       }
                       if (!typed) {
                         page->Fail("The page didn't take the text.");
                         return;
                       }
                       if (submit) {
                         page->PressKey("Enter", base::BindOnce(&AgentPage::Settle, page));
                       } else {
                         page->Settle();
                       }
                     },
                     page, submit));
           },
           weak_factory_.GetWeakPtr(), submit),
       SlotOfElement(element));
}

void AgentPage::PressKey(const std::string& name, base::OnceClosure then) {
  const Key* key = nullptr;
  for (const Key& each : kKeys) {
    if (each.name == name) {
      key = &each;
    }
  }
  if (!key) {
    Fail(
        "There's no such key. Press Enter, Tab, Escape, Backspace, Delete, Space, an arrow, "
        "PageUp, PageDown, Home or End.");
    return;
  }
  HideSuggestions();
  // No nativeVirtualKeyCode, as select-all has none: DevTools then marks the key skip_if_unhandled,
  // so it goes to the page alone and never to the app's key hooks, ilo's shortcuts or the menus.
  base::DictValue event = base::DictValue()
                              .Set("key", key->name == "Space" ? " " : std::string(key->name))
                              .Set("code", std::string(key->name))
                              .Set("windowsVirtualKeyCode", key->code);
  base::DictValue down = event.Clone().Set("type", key->text.empty() ? "rawKeyDown" : "keyDown");
  if (!key->text.empty()) {
    down.Set("text", std::string(key->text));
  }
  Send("Input.dispatchKeyEvent", std::move(down), base::DoNothing());
  Send("Input.dispatchKeyEvent", event.Clone().Set("type", "keyUp"),
       base::BindOnce([](base::OnceClosure then, const base::DictValue*) { std::move(then).Run(); },
                      std::move(then)));
}

void AgentPage::Scroll(bool down) {
  if (job_->element) {
    Center(base::BindOnce(
        [](base::WeakPtr<AgentPage> page, std::optional<gfx::RectF> box) {
          if (!page) {
            return;
          }
          if (!box) {
            page->Fail(
                "The element isn't on the page any more, or is hidden. Take a new snapshot.");
            return;
          }
          page->Settle();
        },
        weak_factory_.GetWeakPtr()));
    return;
  }
  Send("Page.getLayoutMetrics", base::DictValue(),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, bool down, const base::DictValue* metrics) {
             if (!page) {
               return;
             }
             const base::DictValue* viewport =
                 metrics ? metrics->FindDict("cssLayoutViewport") : nullptr;
             const double width = viewport ? viewport->FindDouble("clientWidth").value_or(0) : 0;
             const double height = viewport ? viewport->FindDouble("clientHeight").value_or(0) : 0;
             if (width <= 0 || height <= 0) {
               page->Fail("The page has no view to scroll.");
               return;
             }
             page->Send("Input.dispatchMouseEvent",
                        base::DictValue()
                            .Set("type", "mouseWheel")
                            .Set("x", width / 2)
                            .Set("y", height / 2)
                            .Set("deltaX", 0)
                            .Set("deltaY", (down ? 0.8 : -0.8) * height),
                        base::BindOnce(
                            [](base::WeakPtr<AgentPage> page, const base::DictValue*) {
                              if (page) {
                                page->Settle();
                              }
                            },
                            page));
           },
           weak_factory_.GetWeakPtr(), down));
}

void AgentPage::History(bool forward) {
  content::NavigationController& controller = GetWebContents().GetController();
  if (forward ? !controller.CanGoForward() : !controller.CanGoBack()) {
    Fail(forward ? "There's no page to go forward to." : "There's no page to go back to.");
    return;
  }
  content::NavigationEntry* entry = controller.GetEntryAtOffset(forward ? 1 : -1);
  if (!entry || !entry->GetURL().SchemeIsHTTPOrHTTPS()) {
    Fail("That page isn't an http or https page, the only pages agents may open.");
    return;
  }
  [[maybe_unused]] auto navigations = forward ? controller.GoForward() : controller.GoBack();
  Settle();
}

void AgentPage::Screenshot() {
  Send("Page.captureScreenshot", base::DictValue().Set("format", "jpeg").Set("quality", 75),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, const base::DictValue* result) {
             if (!page) {
               return;
             }
             const std::string* data = result ? result->FindString("data") : nullptr;
             if (!data) {
               page->Fail("The page couldn't be pictured.");
               return;
             }
             page->Answer(*data, /*error=*/false);
           },
           weak_factory_.GetWeakPtr()));
}

void AgentPage::Snapshot() {
  // After an action, the page may have gone somewhere agents may not.
  if (!IsWebPage()) {
    Fail("The tab's page isn't an http or https page, the only pages agents may use.");
    return;
  }
  Send("Accessibility.getFullAXTree", base::DictValue(),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, const base::DictValue* result) {
             if (!page) {
               return;
             }
             const base::ListValue* nodes = result ? result->FindList("nodes") : nullptr;
             if (!nodes) {
               page->Fail("The page's tree couldn't be read.");
               return;
             }
             // Then the trees of its frames, read after it, as only then has DevTools told
             // of the sessions of those in other processes.
             page->frames_ = page->Frames();
             page->frames_[0].nodes = nodes->Clone();
             page->ReadTree(1);
           },
           weak_factory_.GetWeakPtr()));
}

std::vector<AgentPage::Frame> AgentPage::Frames() {
  content::RenderFrameHost* main = GetWebContents().GetPrimaryMainFrame();
  std::vector<content::RenderFrameHost*> hosts;
  main->ForEachRenderFrameHost(
      [&hosts](content::RenderFrameHost* host) { hosts.push_back(host); });
  const auto depth = [](content::RenderFrameHost* host) {
    int depth = 0;
    for (; host->GetParent(); host = host->GetParent()) {
      ++depth;
    }
    return depth;
  };
  std::ranges::stable_sort(hosts, std::ranges::less(), depth);
  std::vector<Frame> frames;
  frames.push_back({std::string(), 0, 0, base::ListValue()});
  std::map<content::RenderFrameHost*, size_t> index = {{main, 0}};
  for (content::RenderFrameHost* host : hosts) {
    const auto parent = host->GetParent() ? index.find(host->GetParent()) : index.end();
    if (parent == index.end() || !host->IsRenderFrameLive() || !IsWebFrame(host)) {
      continue;
    }
    const int slot = SlotOf(host);
    if (slot < 0) {
      continue;
    }
    index[host] = frames.size();
    frames.push_back(
        {host->GetDevToolsFrameToken().ToString(), slot, parent->second, base::ListValue()});
  }
  return frames;
}

void AgentPage::ReadTree(size_t next) {
  if (next >= frames_.size()) {
    StitchTree(1);
    return;
  }
  const Frame& frame = frames_[next];
  Send("Accessibility.getFullAXTree", base::DictValue().Set("frameId", frame.id),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, size_t next, const base::DictValue* result) {
             if (!page || next >= page->frames_.size()) {
               return;
             }
             // A frame that went meanwhile is left out, and so is what's in it.
             if (const base::ListValue* nodes = result ? result->FindList("nodes") : nullptr) {
               Frame& frame = page->frames_[next];
               frame.nodes =
                   Tagged(nodes->Clone(), base::NumberToString(next) + ":", frame.slot);
             }
             page->ReadTree(next + 1);
           },
           weak_factory_.GetWeakPtr(), next),
       frame.slot);
}

void AgentPage::StitchTree(size_t next) {
  if (next >= frames_.size()) {
    std::vector<Frame> frames = std::exchange(frames_, {});
    base::ListValue nodes = std::move(frames[0].nodes);
    for (size_t each = 1; each < frames.size(); ++each) {
      for (base::Value& node : frames[each].nodes) {
        nodes.Append(std::move(node));
      }
    }
    // Its text fields, each checked for whether it's for a password.
    std::vector<int64_t> fields;
    for (const base::Value& node : nodes) {
      const base::DictValue* dict = node.GetIfDict();
      const std::string* role = dict ? dict->FindStringByDottedPath("role.value") : nullptr;
      const std::optional<int64_t> backend = dict ? ElementOf(*dict) : std::nullopt;
      if (role && backend && (*role == "textbox" || *role == "searchbox")) {
        fields.push_back(*backend);
      }
    }
    CheckPasswords(std::move(nodes), std::move(fields), {});
    return;
  }
  const Frame& frame = frames_[next];
  const Frame& parent = frames_[frame.parent];
  if (frame.nodes.empty() || parent.nodes.empty()) {
    frames_[next].nodes.clear();
    StitchTree(next + 1);
    return;
  }
  Send("DOM.getFrameOwner", base::DictValue().Set("frameId", frame.id),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, size_t next, const base::DictValue* result) {
             if (!page || next >= page->frames_.size()) {
               return;
             }
             Frame& frame = page->frames_[next];
             Frame& parent = page->frames_[frame.parent];
             const std::optional<int> owner =
                 result ? result->FindInt("backendNodeId") : std::nullopt;
             const base::DictValue* root = frame.nodes.front().GetIfDict();
             const std::string* root_id = root ? root->FindString("nodeId") : nullptr;
             bool stitched = false;
             for (base::Value& node : parent.nodes) {
               base::DictValue* dict = node.GetIfDict();
               if (owner && root_id && dict &&
                   ElementOf(*dict) == ElementId(parent.slot, *owner)) {
                 dict->EnsureList("childIds")->Append(*root_id);
                 stitched = true;
                 break;
               }
             }
             // A frame whose element isn't in the tree, as a hidden one's isn't, is left out.
             if (!stitched) {
               frame.nodes.clear();
             }
             page->StitchTree(next + 1);
           },
           weak_factory_.GetWeakPtr(), next),
       parent.slot);
}

void AgentPage::CheckPasswords(base::ListValue nodes,
                               std::vector<int64_t> fields,
                               std::set<int64_t> passwords) {
  if (fields.empty()) {
    // The ids of elements in frames of their own are doubles, written as the integers they are.
    std::optional<std::string> json =
        base::WriteJsonWithOptions(base::Value(AgentTree(std::move(nodes), passwords)),
                                   base::JSONWriter::OPTIONS_OMIT_DOUBLE_TYPE_PRESERVATION);
    if (json) {
      Answer(std::move(*json), /*error=*/false);
    } else {
      Fail("The page's tree couldn't be read.");
    }
    return;
  }
  const int64_t field = fields.back();
  fields.pop_back();
  IsPassword(field,
             base::BindOnce(
                 [](base::WeakPtr<AgentPage> page, base::ListValue nodes,
                    std::vector<int64_t> fields, std::set<int64_t> passwords, int64_t field,
                    std::optional<bool> password) {
                   if (!page) {
                     return;
                   }
                   // One that couldn't be read is taken for one.
                   if (password.value_or(true)) {
                     passwords.insert(field);
                   }
                   page->CheckPasswords(std::move(nodes), std::move(fields), std::move(passwords));
                 },
                 weak_factory_.GetWeakPtr(), std::move(nodes), std::move(fields),
                 std::move(passwords), field));
}

void AgentPage::Center(Then then) {
  const int slot = SlotOfElement(job_->element);
  const int element = NodeOfElement(job_->element);
  Send("DOM.getDocument", base::DictValue().Set("depth", 0),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, int slot, int element, Then then,
              const base::DictValue*) {
             if (!page) {
               return;
             }
             page->Send(
                 "DOM.scrollIntoViewIfNeeded", base::DictValue().Set("backendNodeId", element),
                 base::BindOnce(
                     [](base::WeakPtr<AgentPage> page, int slot, int element, Then then,
                        const base::DictValue*) {
                       if (!page) {
                         return;
                       }
                       page->Send(
                           "DOM.getContentQuads", base::DictValue().Set("backendNodeId", element),
                           base::BindOnce(
                               [](base::WeakPtr<AgentPage> page, int slot, Then then,
                                  const base::DictValue* result) {
                                 if (!page) {
                                   return;
                                 }
                                 const base::ListValue* quads =
                                     result ? result->FindList("quads") : nullptr;
                                 const base::ListValue* quad =
                                     quads && !quads->empty() ? (*quads)[0].GetIfList() : nullptr;
                                 if (!quad || quad->size() < 8) {
                                   std::move(then).Run(std::nullopt);
                                   return;
                                 }
                                 // The box around its corners.
                                 double xs[4];
                                 double ys[4];
                                 for (size_t corner = 0; corner < 4; ++corner) {
                                   xs[corner] = (*quad)[2 * corner].GetIfDouble().value_or(0);
                                   ys[corner] = (*quad)[2 * corner + 1].GetIfDouble().value_or(0);
                                 }
                                 const auto [left, right] = std::ranges::minmax(xs);
                                 const auto [top, bottom] = std::ranges::minmax(ys);
                                 const gfx::RectF box(
                                     static_cast<float>(left), static_cast<float>(top),
                                     static_cast<float>(right - left),
                                     static_cast<float>(bottom - top));
                                 page->Offset(slot, box, std::move(then));
                               },
                               page, slot, std::move(then)),
                           slot);
                     },
                     page, slot, element, std::move(then)),
                 slot);
           },
           weak_factory_.GetWeakPtr(), slot, element, std::move(then)),
       slot);
}

void AgentPage::Offset(int slot, gfx::RectF box, Then then) {
  if (slot == 0) {
    std::move(then).Run(box);
    return;
  }
  content::RenderFrameHost* frame = FrameOf(slot);
  content::RenderFrameHost* parent = frame ? frame->GetParent() : nullptr;
  const int up = parent ? SlotOf(parent) : -1;
  if (up < 0) {
    std::move(then).Run(std::nullopt);
    return;
  }
  Send("DOM.getFrameOwner", base::DictValue().Set("frameId", targets_[slot].frame),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, int up, gfx::RectF box, Then then,
              const base::DictValue* result) {
             if (!page) {
               return;
             }
             const std::optional<int> owner =
                 result ? result->FindInt("backendNodeId") : std::nullopt;
             if (!owner) {
               std::move(then).Run(std::nullopt);
               return;
             }
             // The frame's element in view in its own frame, where the click then lands.
             page->Send("DOM.scrollIntoViewIfNeeded",
                        base::DictValue().Set("backendNodeId", *owner), base::DoNothing(), up);
             page->Send(
                 "DOM.getBoxModel", base::DictValue().Set("backendNodeId", *owner),
                 base::BindOnce(
                     [](base::WeakPtr<AgentPage> page, int up, gfx::RectF box, Then then,
                        const base::DictValue* result) {
                       if (!page) {
                         return;
                       }
                       const base::ListValue* content =
                           result ? result->FindListByDottedPath("model.content") : nullptr;
                       if (!content || content->size() < 2) {
                         std::move(then).Run(std::nullopt);
                         return;
                       }
                       // The frame's page starts at the top left of its element's content box.
                       box.Offset(static_cast<float>((*content)[0].GetIfDouble().value_or(0)),
                                  static_cast<float>((*content)[1].GetIfDouble().value_or(0)));
                       page->Offset(up, box, std::move(then));
                     },
                     page, up, box, std::move(then)),
                 up);
           },
           weak_factory_.GetWeakPtr(), up, box, std::move(then)),
       up);
}

void AgentPage::Point(const gfx::RectF& box) {
  if (!Host().agent_page_element || !job_) {
    return;
  }
  // The page's CSS pixels, at its zoom, are the view's points.
  const float zoom = static_cast<float>(
      blink::ZoomLevelToZoomFactor(content::HostZoomMap::GetZoomLevel(&GetWebContents())));
  const gfx::RectF points = gfx::ScaleRect(box, zoom);
  Host().agent_page_element(Host().context, tab_, job_->request,
                            bc_rect{points.x(), points.y(), points.width(), points.height()});
}

void AgentPage::IsPassword(int64_t element, base::OnceCallback<void(std::optional<bool>)> then) {
  Send("DOM.describeNode", base::DictValue().Set("backendNodeId", NodeOfElement(element)),
       base::BindOnce(
           [](base::OnceCallback<void(std::optional<bool>)> then, const base::DictValue* result) {
             const base::DictValue* node = result ? result->FindDict("node") : nullptr;
             if (!node) {
               std::move(then).Run(std::nullopt);
               return;
             }
             const base::ListValue* attributes = node->FindList("attributes");
             bool password = false;
             // Name, value, name, value.
             for (size_t i = 0; attributes && i + 1 < attributes->size(); i += 2) {
               const std::string* name = (*attributes)[i].GetIfString();
               const std::string* value = (*attributes)[i + 1].GetIfString();
               if (name && value && base::EqualsCaseInsensitiveASCII(*name, "type") &&
                   base::EqualsCaseInsensitiveASCII(*value, "password")) {
                 password = true;
               }
             }
             std::move(then).Run(password);
           },
           std::move(then)),
       SlotOfElement(element));
}

void AgentPage::HideSuggestions() {
  if (autofill::ContentAutofillClient* client =
          autofill::ContentAutofillClient::FromWebContents(&GetWebContents())) {
    client->HideSuggestions(autofill::SuggestionHidingReason::kHiddenByCaller, std::nullopt);
  }
}

void AgentPage::Answer(std::string text, bool error) {
  if (!job_) {
    return;
  }
  const uint64_t request = job_->request;
  job_.reset();
  frames_.clear();
  wait_.Stop();
  loaded_.Reset();
  // Posted, so the app never hears inside its own bc_agent_page.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Report, tab_, request, std::move(text), error));
  Next();
  if (!job_) {
    // A hidden page left drawing would keep its animations waking the browser while the agent
    // thinks or has gone quiet.
    rest_.Start(FROM_HERE, kRest,
                base::BindOnce([](AgentPage* page) { page->drawing_.RunAndReset(); },
                               base::Unretained(this)));
  }
}

void AgentPage::Fail(std::string why) {
  Answer(std::move(why), /*error=*/true);
}

void AgentPage::LetGo() {
  std::deque<Job> owed = std::exchange(jobs_, {});
  if (job_) {
    owed.push_front(*std::move(job_));
    job_.reset();
  }
  for (const Job& job : owed) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Report, tab_, job.request,
                                  std::string("The agent's session with the tab ended."), true));
  }
  weak_factory_.InvalidateWeakPtrs();
  wait_.Stop();
  rest_.Stop();
  loaded_.Reset();
  drawing_.RunAndReset();
  Detach();
}

int AgentPage::SlotOf(content::RenderFrameHost* frame) {
  while (frame->GetParent() && !frame->IsCrossProcessSubframe()) {
    frame = frame->GetParent();
  }
  if (!frame->GetParent()) {
    return frame == GetWebContents().GetPrimaryMainFrame() && agent_host_ ? 0 : -1;
  }
  const std::string id = frame->GetDevToolsFrameToken().ToString();
  for (size_t slot = 1; slot < targets_.size(); ++slot) {
    if (targets_[slot].frame == id) {
      return targets_[slot].session.empty() ? -1 : static_cast<int>(slot);
    }
  }
  return -1;
}

content::RenderFrameHost* AgentPage::FrameOf(int slot) {
  if (slot <= 0 || static_cast<size_t>(slot) >= targets_.size()) {
    return nullptr;
  }
  content::RenderFrameHost* found = nullptr;
  GetWebContents().GetPrimaryMainFrame()->ForEachRenderFrameHost(
      [this, slot, &found](content::RenderFrameHost* host) {
        if (host->GetDevToolsFrameToken().ToString() == targets_[slot].frame) {
          found = host;
        }
      });
  return found;
}

void AgentPage::Send(const char* method, base::DictValue params, Reply reply, int slot) {
  if (!agent_host_) {
    scoped_refptr<content::DevToolsAgentHost> host =
        content::DevToolsAgentHost::GetOrCreateFor(&GetWebContents());
    // The profile's policy may keep DevTools from the page, which leaves agents without it too.
    if (!host || !host->AttachClient(this)) {
      std::move(reply).Run(nullptr);
      return;
    }
    agent_host_ = std::move(host);
    targets_.assign(1, Target());
    AutoAttach(0);
  }
  if (slot < 0 || static_cast<size_t>(slot) >= targets_.size() ||
      (slot && targets_[slot].session.empty())) {
    std::move(reply).Run(nullptr);
    return;
  }
  const int id = next_command_++;
  base::DictValue command =
      base::DictValue().Set("id", id).Set("method", method).Set("params", std::move(params));
  if (slot) {
    command.Set("sessionId", targets_[slot].session);
  }
  std::optional<std::string> json = base::WriteJson(command);
  if (!json) {
    std::move(reply).Run(nullptr);
    return;
  }
  replies_[id] = {slot, std::move(reply)};
  agent_host_->DispatchProtocolMessage(this, base::as_byte_span(*json));
}

void AgentPage::AutoAttach(int slot) {
  // Asked again after its host closed, it would bring DevTools back to a page no agent acts on.
  if (!agent_host_) {
    return;
  }
  Send("Target.setAutoAttach",
       base::DictValue()
           .Set("autoAttach", true)
           .Set("waitForDebuggerOnStart", false)
           .Set("flatten", true)
           .Set("filter", base::ListValue().Append(base::DictValue().Set("type", "iframe"))),
       base::DoNothing(), slot);
}

void AgentPage::Detach() {
  replies_.clear();
  targets_.clear();
  frames_.clear();
  if (scoped_refptr<content::DevToolsAgentHost> host = std::move(agent_host_)) {
    host->DetachClient(this);
  }
}

void AgentPage::DispatchProtocolMessage(content::DevToolsAgentHost* agent_host,
                                        base::span<const uint8_t> message) {
  std::optional<base::DictValue> dict = base::JSONReader::ReadDict(
      std::string_view(reinterpret_cast<const char*>(message.data()), message.size()),
      base::JSON_PARSE_RFC);
  std::optional<int> id = dict ? dict->FindInt("id") : std::nullopt;
  // Events have no id. Of them, only a frame's session starting or ending matters here.
  if (!id) {
    const std::string* method = dict ? dict->FindString("method") : nullptr;
    const base::DictValue* params = dict ? dict->FindDict("params") : nullptr;
    const std::string* session = params ? params->FindString("sessionId") : nullptr;
    if (method && session && *method == "Target.attachedToTarget") {
      Attached(*session, *params);
    } else if (method && session && *method == "Target.detachedFromTarget") {
      Detached(*session);
    }
    return;
  }
  auto reply = replies_.find(*id);
  if (reply == replies_.end()) {
    return;
  }
  Reply run = std::move(reply->second.second);
  replies_.erase(reply);
  // An error has no result, which each reply takes for a failure.
  std::move(run).Run(dict->FindDict("result"));
}

void AgentPage::AgentHostClosed(content::DevToolsAgentHost* agent_host) {
  agent_host_ = nullptr;
  // The page's renderer went: what was asked is answered as nothing.
  auto replies = std::move(replies_);
  replies_.clear();
  targets_.clear();
  for (auto& [id, reply] : replies) {
    std::move(reply.second).Run(nullptr);
  }
}

void AgentPage::Attached(const std::string& session, const base::DictValue& event) {
  const std::string* frame = event.FindStringByDottedPath("targetInfo.targetId");
  const std::string* type = event.FindStringByDottedPath("targetInfo.type");
  if (!frame || !type || *type != "iframe") {
    return;
  }
  auto target = std::ranges::find(targets_, *frame, &Target::frame);
  if (target == targets_.end()) {
    targets_.push_back({*frame, std::string()});
    target = targets_.end() - 1;
  }
  target->session = session;
  // Posted, as DevTools tells of a frame inside the command that attached it. The frame's own
  // frames in other processes come in sessions of their own too.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&AgentPage::AutoAttach, weak_factory_.GetWeakPtr(),
                                static_cast<int>(target - targets_.begin())));
}

void AgentPage::Detached(const std::string& session) {
  auto target = std::ranges::find(targets_, session, &Target::session);
  if (target == targets_.end()) {
    return;
  }
  target->session.clear();
  const int slot = static_cast<int>(target - targets_.begin());
  // What was asked of it is answered as nothing, as it never will be.
  std::vector<int> owed;
  for (const auto& [command, reply] : replies_) {
    if (reply.first == slot) {
      owed.push_back(command);
    }
  }
  for (int command : owed) {
    auto reply = replies_.find(command);
    if (reply == replies_.end()) {
      continue;
    }
    Reply run = std::move(reply->second.second);
    replies_.erase(reply);
    std::move(run).Run(nullptr);
  }
}

void AgentPage::DidStopLoading() {
  // Only a wait for the load ends here. A load that stops while an action settles, as a page from
  // the cache can within the settle's time, leaves the settle to snapshot the page it loaded.
  if (loaded_) {
    wait_.Stop();
    std::move(loaded_).Run();
  }
}

void AgentPage::WebContentsDestroyed() {
  // The tab is closing: what's asked is answered now, as the replies never will be.
  LetGo();
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(AgentPage);

}  // namespace browser_core
