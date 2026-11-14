#include "browser_core/agent_page.h"

#include <algorithm>
#include <deque>
#include <functional>
#include <iterator>
#include <map>
#include <string_view>
#include <utility>
#include <vector>

#include "base/base64.h"
#include "base/containers/adapters.h"
#include "base/containers/span.h"
#include "base/functional/bind.h"
#include "base/i18n/case_conversion.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/strings/strcat.h"
#include "base/scoped_multi_source_observation.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/bind_post_task.h"
#include "base/task/single_thread_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/time/time.h"
#include "browser_core/host.h"
#include "browser_core/platform/window_views.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/typing_keys.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "chrome/browser/shell_integration.h"
#include "components/autofill/content/browser/content_autofill_client.h"
#include "components/autofill/core/browser/filling/filling_product.h"
#include "components/autofill/core/browser/suggestions/suggestion_hiding_reason.h"
#include "content/public/browser/browser_accessibility_state.h"
#include "components/viz/common/frame_sinks/copy_output_result.h"
#include "mojo/public/cpp/bindings/callback_helpers.h"
#include "content/public/browser/devtools_agent_host.h"
#include "content/public/browser/host_zoom_map.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_entry.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/scoped_accessibility_mode.h"
#include "content/public/browser/render_widget_host.h"
#include "content/public/browser/render_widget_host_observer.h"
#include "content/public/browser/render_widget_host_view.h"
#include "content/public/browser/web_contents.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "third_party/blink/public/common/page/page_zoom.h"
#include "ui/accessibility/ax_action_data.h"
#include "ui/accessibility/ax_enums.mojom.h"
#include "ui/accessibility/ax_mode.h"
#include "third_party/blink/public/mojom/input/input_event_result.mojom-shared.h"
#include "ui/gfx/geometry/point_f.h"
#include "ui/gfx/geometry/rect.h"
#include "third_party/skia/include/core/SkBitmap.h"
#include "ui/gfx/codec/jpeg_codec.h"
#include "ui/gfx/geometry/size.h"
#include "url/gurl.h"
#include "url/origin.h"
#include "url/url_constants.h"

namespace browser_core {

namespace {

constexpr base::TimeDelta kLoadTimeout = base::Seconds(15);
// How long an action waits for its element to be ready, and then for the page to settle.
constexpr base::TimeDelta kBusy = base::Seconds(5);
// A page with no request for this long has settled: what an input starts, such as a navigation
// or a menu's fetch, has started by then.
constexpr base::TimeDelta kQuiet = base::Milliseconds(500);
// A frame, at the rate pages draw, between the looks that tell whether something holds still.
constexpr base::TimeDelta kFrame = base::Hertz(60);
// What the wheel starts, a scroll or nothing to scroll, has started by then (#1627).
constexpr base::TimeDelta kSettle = base::Milliseconds(400);
// What an agent hears when the input would reach an extension's control, which the activity log
// has too.
constexpr char kExtensionCovers[] =
    "An extension's control covers that spot on the page, and agents can't act on an "
    "extension's controls. Ask the user if it's in the way.";
constexpr char kExtensionKeyboard[] =
    "An extension's control has the keyboard in this tab, and agents can't act on an "
    "extension's controls. Click the page outside it first, or ask the user.";
// How long a hidden page keeps drawing after an agent's last action, for its next one.
constexpr base::TimeDelta kRest = base::Seconds(30);
// Ours: how long a navigation that became a download keeps its action's answer for the download.
// Its item starts within a few tasks of the navigation's end.
constexpr base::TimeDelta kDownloadStart = base::Seconds(5);

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

// Whether an accessibility node is disabled, as a control, or one in a disabled fieldset, is.
bool IsDisabled(const base::Value& node) {
  const base::DictValue* dict = node.GetIfDict();
  const base::ListValue* properties = dict ? dict->FindList("properties") : nullptr;
  if (!properties) {
    return false;
  }
  for (const base::Value& property : *properties) {
    const base::DictValue* each = property.GetIfDict();
    const std::string* name = each ? each->FindString("name") : nullptr;
    if (name && *name == "disabled") {
      return each->FindBoolByDottedPath("value.value").value_or(false);
    }
  }
  return false;
}

// A node's name, as the page shows it, its runs of spaces one.
std::string NameOf(const base::DictValue& node) {
  const std::string* name = node.FindStringByDottedPath("name.value");
  return name ? base::CollapseWhitespaceASCII(*name, /*trim_sequences_with_line_breaks=*/true)
              : std::string();
}

// A name as it compares in any case, as "Open" and "OPEN" do.
std::u16string Folded(std::string_view name) {
  return base::i18n::FoldCase(base::UTF8ToUTF16(name));
}

// Whether the node has the boolean |property|, such as "selected", and it's true.
bool Has(const base::DictValue& node, std::string_view property) {
  if (const base::ListValue* properties = node.FindList("properties")) {
    for (const base::Value& each : *properties) {
      const base::DictValue* dict = each.GetIfDict();
      const std::string* name = dict ? dict->FindString("name") : nullptr;
      if (name && *name == property) {
        return dict->FindBoolByDottedPath("value.value").value_or(false);
      }
    }
  }
  return false;
}

// What a node of the tree holds, as text: a field's value, or a number field's number.
std::optional<std::string> ValueOf(const base::DictValue& node) {
  const base::Value* value = node.FindByDottedPath("value.value");
  if (!value) {
    return std::nullopt;
  }
  if (const std::string* text = value->GetIfString()) {
    return *text;
  }
  if (const std::optional<double> number = value->GetIfDouble()) {
    return base::NumberToString(*number);
  }
  return std::nullopt;
}

// The attribute |name| of a node as DOM.describeNode gives it, if it has one.
std::optional<std::string> AttributeOf(const base::DictValue& node, std::string_view name) {
  const base::ListValue* attributes = node.FindList("attributes");
  // Name, value, name, value.
  for (size_t i = 0; attributes && i + 1 < attributes->size(); i += 2) {
    const std::string* each = (*attributes)[i].GetIfString();
    const std::string* value = (*attributes)[i + 1].GetIfString();
    if (each && value && base::EqualsCaseInsensitiveASCII(*each, name)) {
      return *value;
    }
  }
  return std::nullopt;
}

// The text of a described node's text children, as a textarea's or an option's is.
std::string ChildText(const base::DictValue& node) {
  std::string text;
  if (const base::ListValue* children = node.FindList("children")) {
    for (const base::Value& child : *children) {
      const base::DictValue* dict = child.GetIfDict();
      const std::string* value = dict ? dict->FindString("nodeValue") : nullptr;
      if (value && dict->FindInt("nodeType") == 3) {
        text += *value;
      }
    }
  }
  return text;
}

// An option's label as a select shows it: its label attribute or else its text, its runs of
// spaces one.
std::u16string LabelOf(const base::DictValue& option) {
  const std::optional<std::string> label = AttributeOf(option, "label");
  return base::CollapseWhitespace(
      base::UTF8ToUTF16(label && !label->empty() ? *label : ChildText(option)),
      /*trim_sequences_with_line_breaks=*/true);
}

// A described field's value in the page's markup: an input's value attribute, a textarea's text,
// or a select's option marked selected, else its first. None for anything else, as an editable
// region's.
std::optional<std::u16string> MarkupOf(const base::DictValue& node) {
  const std::string* name = node.FindString("nodeName");
  if (!name) {
    return std::nullopt;
  }
  if (*name == "INPUT") {
    return base::UTF8ToUTF16(AttributeOf(node, "value").value_or(std::string()));
  }
  if (*name == "TEXTAREA") {
    return base::UTF8ToUTF16(ChildText(node));
  }
  if (*name != "SELECT") {
    return std::nullopt;
  }
  std::vector<const base::DictValue*> options;
  const auto add = [&options](const base::DictValue& parent, const auto& add) -> void {
    const base::ListValue* children = parent.FindList("children");
    for (size_t i = 0; children && i < children->size(); ++i) {
      const base::DictValue* child = (*children)[i].GetIfDict();
      const std::string* tag = child ? child->FindString("nodeName") : nullptr;
      if (tag && *tag == "OPTION") {
        options.push_back(child);
      } else if (tag && *tag == "OPTGROUP") {
        add(*child, add);
      }
    }
  };
  add(node, add);
  if (options.empty()) {
    return std::nullopt;
  }
  for (const base::DictValue* option : options) {
    if (AttributeOf(*option, "selected")) {
      return LabelOf(*option);
    }
  }
  return LabelOf(*options.front());
}

// The box around a quad's corners, as DOM.getBoxModel and DOM.getContentQuads give them.
std::optional<gfx::RectF> BoundsOf(const base::ListValue& quad) {
  if (quad.size() < 8) {
    return std::nullopt;
  }
  double xs[4];
  double ys[4];
  for (size_t corner = 0; corner < 4; ++corner) {
    xs[corner] = quad[2 * corner].GetIfDouble().value_or(0);
    ys[corner] = quad[2 * corner + 1].GetIfDouble().value_or(0);
  }
  const auto [left, right] = std::ranges::minmax(xs);
  const auto [top, bottom] = std::ranges::minmax(ys);
  return gfx::RectF(static_cast<float>(left), static_cast<float>(top),
                    static_cast<float>(right - left), static_cast<float>(bottom - top));
}

// Network.enable's parameters: no request's or response's body is kept.
base::DictValue Hearing() {
  return base::DictValue()
      .Set("maxTotalBufferSize", 0)
      .Set("maxResourceBufferSize", 0)
      .Set("maxPostDataSize", 0);
}

bool IsRole(const base::DictValue& node, std::string_view role) {
  const std::string* value = node.FindStringByDottedPath("role.value");
  return value && *value == role;
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

// Why there's no picture, for the agent.
constexpr char kNoPicture[] = "The page couldn't be pictured.";
constexpr char kPictureChanged[] = "The page changed while it was pictured. Try again.";

// DevTools attaches to each frame in another process below a session's, in a session of its own.
base::DictValue AutoAttaching() {
  return base::DictValue()
      .Set("autoAttach", true)
      .Set("waitForDebuggerOnStart", false)
      .Set("flatten", true)
      .Set("filter", base::ListValue().Append(base::DictValue().Set("type", "iframe")));
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
    // What an extension's content script made is the extension's, not the page's: no ref, and no
    // text read as the page's (engine/patches/ours/chromium-agent-extension-nodes.patch marks it,
    // and each node inside it).
    if (!id || under_fields.contains(*id) || Has(*dict, "fromExtension")) {
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

// Whether a wheel moved anything, as the widgets of the page's frames answer it: a scroll it
// started that scrolled, in the frame under it or one it went on to, or a page that took the wheel
// itself, as an editor or a map may.
class AgentPage::WheelWatch : public content::RenderWidgetHost::InputEventObserver,
                              public content::RenderWidgetHostObserver {
 public:
  explicit WheelWatch(content::WebContents& contents) {
    contents.GetPrimaryMainFrame()->ForEachRenderFrameHost(
        [this](content::RenderFrameHost* frame) {
          content::RenderWidgetHost* widget = frame->GetRenderWidgetHost();
          if (widget && !acks_.IsObservingSource(widget)) {
            acks_.AddObservation(widget);
            gone_.AddObservation(widget);
          }
        });
  }

  bool moved() const { return moved_; }

 private:
  // content::RenderWidgetHost::InputEventObserver:
  void OnInputEventAck(const content::RenderWidgetHost&,
                       blink::mojom::InputEventResultSource,
                       blink::mojom::InputEventResultState state,
                       const blink::WebInputEvent& event) override {
    const blink::WebInputEvent::Type type = event.GetType();
    if (state == blink::mojom::InputEventResultState::kConsumed &&
        (type == blink::WebInputEvent::Type::kGestureScrollUpdate ||
         type == blink::WebInputEvent::Type::kMouseWheel)) {
      moved_ = true;
    }
  }

  // content::RenderWidgetHostObserver:
  void RenderWidgetHostDestroyed(content::RenderWidgetHost* widget) override {
    acks_.RemoveObservation(widget);
    gone_.RemoveObservation(widget);
  }

  bool moved_ = false;
  base::ScopedMultiSourceObservation<content::RenderWidgetHost,
                                     content::RenderWidgetHost::InputEventObserver>
      acks_{this};
  base::ScopedMultiSourceObservation<content::RenderWidgetHost, content::RenderWidgetHostObserver>
      gone_{this};
};

AgentPage::AgentPage(content::WebContents* contents)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<AgentPage>(*contents),
      tab_(Tab::IdOf(contents)) {}

AgentPage::~AgentPage() {
  Detach();
}

void AgentPage::Act(uint64_t request, int action, int64_t element, std::string text) {
  acted_ = true;
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

// static
uint64_t AgentPage::TakeJob(content::WebContents* contents) {
  AgentPage* page = AgentPage::FromWebContents(contents);
  if (!page) {
    return 0;
  }
  page->download_starting_.Stop();
  if (page->held_) {
    return std::exchange(page->held_, std::nullopt)->request;
  }
  if (page->job_ && !page->taken_) {
    page->taken_ = true;
    return page->job_->request;
  }
  return 0;
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

std::string AgentPage::NotWebPage() {
  content::WebContents& contents = GetWebContents();
  // A tab whose first page hasn't committed is still on its empty start page, which isn't the
  // page's fault: the agent can wait for it (#1461).
  const content::NavigationEntry* pending = contents.GetController().GetPendingEntry();
  const GURL loading = pending ? pending->GetURL() : GURL();
  if (contents.IsLoading() && loading.SchemeIsHTTPOrHTTPS()) {
    std::string_view site = loading.host();
    if (base::StartsWith(site, "www.")) {
      site.remove_prefix(4);
    }
    return base::StrCat({site, " is still loading after ",
                         base::NumberToString(kLoadTimeout.InSeconds()),
                         " s. Wait, or try again."});
  }
  return "The tab's page isn't an http or https page, the only pages agents may use.";
}

void AgentPage::Run() {
  const int action = job_->action;
  const bool history = action == BC_AGENT_PAGE_BACK || action == BC_AGENT_PAGE_FORWARD;
  if (!history && !IsWebPage()) {
    Fail(NotWebPage());
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
      Listen(base::BindOnce(
          [](base::WeakPtr<AgentPage> page) {
            if (page) {
              page->PressKey(page->job_->text, base::BindOnce(&AgentPage::Settle, page));
            }
          },
          weak_factory_.GetWeakPtr()));
      return;
    case BC_AGENT_PAGE_SCROLL_DOWN:
    case BC_AGENT_PAGE_SCROLL_UP:
      Scroll(action == BC_AGENT_PAGE_SCROLL_DOWN);
      return;
    case BC_AGENT_PAGE_SCROLL_TO:
      ScrollTo();
      return;
    case BC_AGENT_PAGE_BACK:
    case BC_AGENT_PAGE_FORWARD:
      Listen(base::BindOnce(&AgentPage::History, weak_factory_.GetWeakPtr(),
                            action == BC_AGENT_PAGE_FORWARD));
      return;
    case BC_AGENT_PAGE_SCREENSHOT:
      Screenshot();
      return;
    case BC_AGENT_PAGE_QUIET:
      Listen(base::BindOnce(&AgentPage::Settle, weak_factory_.GetWeakPtr()));
      return;
    case BC_AGENT_PAGE_SELECT:
      if (!job_->element || job_->text.empty()) {
        Fail("Choosing needs an element and an option.");
        return;
      }
      field_ = job_->element;
      choosing_ = Choosing::kOptions;
      Snapshot();
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

void AgentPage::Ready(bool enabled, Then then) {
  job_->deadline = base::TimeTicks::Now() + kBusy;
  Look(enabled, std::nullopt, 0, std::move(then));
}

void AgentPage::Look(bool enabled, std::optional<gfx::RectF> last, int still, Then then) {
  const int64_t element = job_->element;
  Send("Accessibility.getPartialAXTree",
       base::DictValue().Set("backendNodeId", NodeOfElement(element)).Set("fetchRelatives", false),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, bool enabled, std::optional<gfx::RectF> last,
              int still, Then then, const base::DictValue* result) {
             if (!page || !page->job_) {
               return;
             }
             // DevTools knows no node by an id whose node is gone, nor one whose page is.
             const base::ListValue* nodes = result ? result->FindList("nodes") : nullptr;
             if (!nodes || nodes->empty()) {
               page->Fail("The element isn't on the page any more. Take a new snapshot.");
               return;
             }
             const bool disabled = enabled && IsDisabled(nodes->front());
             page->Center(base::BindOnce(
                 [](base::WeakPtr<AgentPage> page, bool enabled, bool disabled,
                    std::optional<gfx::RectF> last, int still, Then then,
                    std::optional<gfx::RectF> box) {
                   if (!page || !page->job_) {
                     return;
                   }
                   still = box && box == last ? still + 1 : 0;
                   if (still >= 2 && !disabled) {
                     std::move(then).Run(box);
                     return;
                   }
                   if (base::TimeTicks::Now() >= page->job_->deadline) {
                     if (disabled) {
                       page->Fail(
                           "The element is still disabled after 5 s: the page may want something "
                           "else first. Take a new snapshot.");
                       return;
                     }
                     // One still moving is acted on where it is, and the caller says what one
                     // that's hidden can't do.
                     std::move(then).Run(box);
                     return;
                   }
                   page->wait_.Start(FROM_HERE, kFrame,
                                     base::BindOnce(&AgentPage::Look, page, enabled, box, still,
                                                    std::move(then)));
                 },
                 page, enabled, disabled, last, still, std::move(then)));
           },
           weak_factory_.GetWeakPtr(), enabled, last, still, std::move(then)),
       SlotOfElement(element));
}

void AgentPage::Listen(base::OnceClosure then) {
  job_->deadline = base::TimeTicks::Now() + kBusy;
  last_request_ = base::TimeTicks::Now();
  listening_ = true;
  // Each frame in a process of its own makes its own requests. The main frame's session last,
  // as the first command starts DevTools' sessions, and its answer starts the input.
  for (size_t slot = 1; slot < targets_.size(); ++slot) {
    if (!targets_[slot].session.empty()) {
      Send("Network.enable", Hearing(), base::DoNothing(), static_cast<int>(slot));
    }
  }
  Send("Network.enable", Hearing(),
       base::BindOnce([](base::OnceClosure then, const base::DictValue*) { std::move(then).Run(); },
                      std::move(then)));
}

void AgentPage::Unlisten() {
  if (!listening_) {
    return;
  }
  listening_ = false;
  settling_ = false;
  requests_.clear();
  streams_.clear();
  if (!agent_host_) {
    return;
  }
  for (size_t slot = 0; slot < targets_.size(); ++slot) {
    if (slot == 0 || !targets_[slot].session.empty()) {
      Send("Network.disable", base::DictValue(), base::DoNothing(), static_cast<int>(slot));
    }
  }
}

void AgentPage::Heard(const std::string& session,
                      const std::string& method,
                      const base::DictValue& event) {
  const std::string* id = event.FindString("requestId");
  if (!id) {
    return;
  }
  const std::string request = session + " " + *id;
  if (method == "Network.requestWillBeSent") {
    // A stream, as an EventSource's, is open as long as the page is.
    const std::string* type = event.FindString("type");
    (type && *type == "EventSource" ? streams_ : requests_).insert(request);
  } else if (method == "Network.loadingFinished" || method == "Network.loadingFailed") {
    requests_.erase(request);
  } else if (method != "Network.responseReceived") {
    return;
  }
  if (streams_.contains(request)) {
    return;
  }
  // One that started before the job listened counts when it ends too.
  last_request_ = base::TimeTicks::Now();
  if (settling_) {
    Quiet();
  }
}

void AgentPage::Settle() {
  settling_ = true;
  // The quiet counts from the action's end: the requests it starts come after its last event's
  // answer, which on a loaded machine is more than kQuiet after Listen began.
  last_request_ = base::TimeTicks::Now();
  Quiet();
}

void AgentPage::Quiet() {
  const base::TimeTicks now = base::TimeTicks::Now();
  base::TimeTicks next = job_->deadline;
  if (now >= next) {
    settling_ = false;
    // A document still loading is busy. Requests alone are said apart, as what the page shows
    // may well have held still, which Still finds out, and then it's busy too.
    if (GetWebContents().IsLoading()) {
      job_->busy = true;
    } else {
      job_->requests = true;
    }
    Still(base::ListValue());
    return;
  }
  if (!GetWebContents().IsLoading() && requests_.empty()) {
    if (now >= last_request_ + kQuiet) {
      settling_ = false;
      Still(base::ListValue());
      return;
    }
    next = std::min(next, last_request_ + kQuiet);
  }
  // Else the load or a request ending looks again, as does the deadline.
  wait_.Start(FROM_HERE, next - now, base::BindOnce(&AgentPage::Quiet, base::Unretained(this)));
}

void AgentPage::Still(base::ListValue last) {
  // After an action, the page may have gone somewhere agents may not.
  if (!IsWebPage()) {
    Fail("The tab's page isn't an http or https page, the only pages agents may use.");
    return;
  }
  Send("Accessibility.getFullAXTree", base::DictValue(),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, base::ListValue last, const base::DictValue* result) {
             if (!page || !page->job_) {
               return;
             }
             const base::ListValue* nodes = result ? result->FindList("nodes") : nullptr;
             if (!nodes) {
               page->Fail("The page's tree couldn't be read.");
               return;
             }
             // The first read is always compared with a second, even past the deadline.
             const bool same = *nodes == last;
             if (!same && (last.empty() || base::TimeTicks::Now() < page->job_->deadline)) {
               page->wait_.Start(FROM_HERE, kFrame,
                                 base::BindOnce(&AgentPage::Still, page, nodes->Clone()));
               return;
             }
             page->job_->busy |= !same;
             page->Read(nodes->Clone());
           },
           weak_factory_.GetWeakPtr(), std::move(last)));
}

void AgentPage::Click() {
  Press(/*focused=*/false);
}

void AgentPage::Press(bool focused) {
  // Once it's on screen, still and enabled (Ready). Then in the session of the element's frame:
  // DevTools then gives the click to that frame even while the window's hit-test data lacks it, as
  // in a background tab, where through the main frame's session the page around the frame took the
  // click (#1175). The click's answer waits for the requests it starts (Listen).
  Ready(/*enabled=*/true,
        base::BindOnce(
            [](base::WeakPtr<AgentPage> page, bool focused, std::optional<gfx::RectF> ready) {
              if (!page) {
                return;
              }
              if (!ready) {
                page->Fail(
                    "The element isn't on the page any more, or is hidden. Take a new snapshot.");
                return;
              }
              page->Send(
                  "Page.getLayoutMetrics", base::DictValue(),
                  base::BindOnce(
                      [](base::WeakPtr<AgentPage> page, bool focused, gfx::RectF ready,
                         const base::DictValue* metrics) {
                        if (!page || !page->job_) {
                          return;
                        }
                        const base::DictValue* viewport =
                            metrics ? metrics->FindDict("cssLayoutViewport") : nullptr;
                        const gfx::RectF view(
                            static_cast<float>(
                                viewport ? viewport->FindDouble("clientWidth").value_or(0) : 0),
                            static_cast<float>(
                                viewport ? viewport->FindDouble("clientHeight").value_or(0) : 0));
                        if (view.IsEmpty() || view.Contains(ready.CenterPoint())) {
                          page->PressInView();
                          return;
                        }
                        // Scrolled to and still outside the page, where no press lands: a skip
                        // link, which focus brings on screen. So it's focused first, as a
                        // keyboard user's Tab would, then pressed (#2174).
                        if (focused) {
                          page->Fail(
                              "The element stays outside the page even with focus, where a "
                              "click can't reach it. Take a new snapshot.");
                          return;
                        }
                        const int64_t element = page->job_->element;
                        page->Send(
                            "DOM.focus",
                            base::DictValue().Set("backendNodeId", NodeOfElement(element)),
                            base::BindOnce(
                                [](base::WeakPtr<AgentPage> page, const base::DictValue* done) {
                                  if (!page) {
                                    return;
                                  }
                                  if (!done) {
                                    page->Fail(
                                        "The element is outside the page, where a click can't "
                                        "reach it, and can't take focus to come into view.");
                                    return;
                                  }
                                  page->Press(/*focused=*/true);
                                },
                                page),
                            SlotOfElement(element));
                      },
                      page, focused, *ready));
            },
            weak_factory_.GetWeakPtr(), focused));
}

void AgentPage::PressInView() {
  const int slot = SlotOfElement(job_->element);
  Locate(base::BindOnce(
      [](base::WeakPtr<AgentPage> page, int slot, std::optional<gfx::RectF> box) {
        if (!page) {
          return;
        }
        if (!box) {
          page->Fail("The element isn't on the page any more, or is hidden. Take a new snapshot.");
          return;
        }
        // Where that is in the main frame's viewport, for the agent's pointer.
        page->Offset(
            slot, *box,
            base::BindOnce(
                [](base::WeakPtr<AgentPage> page, int slot, gfx::PointF at,
                   std::optional<gfx::RectF> shown) {
                  if (!page) {
                    return;
                  }
                  if (!shown) {
                    page->Fail(
                        "The element isn't on the page any more, or is hidden. Take a "
                        "new snapshot.");
                    return;
                  }
                  page->Point(*shown);
                  // From the top frame down, as the click goes, so what an extension put over
                  // the element in any frame above it is found.
                  page->Covered(
                      shown->CenterPoint(),
                      base::BindOnce(
                          [](base::WeakPtr<AgentPage> page, int slot, gfx::PointF at,
                             bool covered) {
                            if (!page || !page->job_) {
                              return;
                            }
                            if (covered) {
                              page->RefuseExtension(kExtensionCovers);
                              return;
                            }
                            page->Listen(
                                base::BindOnce(&AgentPage::ClickAt, page, slot, at, size_t{0}));
                          },
                          page, slot, at));
                },
                page, slot, box->CenterPoint()));
      },
      weak_factory_.GetWeakPtr(), slot));
}

void AgentPage::Choose(const base::ListValue& nodes, const std::vector<std::string>& frames) {
  const std::string want =
      base::CollapseWhitespaceASCII(job_->text, /*trim_sequences_with_line_breaks=*/true);
  if (want.empty()) {
    Fail("Choosing needs an element and an option.");
    return;
  }
  std::map<std::string, const base::DictValue*> by_id;
  std::map<int64_t, std::string> of_element;
  for (const base::Value& node : nodes) {
    const base::DictValue* dict = node.GetIfDict();
    const std::string* id = dict ? dict->FindString("nodeId") : nullptr;
    if (!id) {
      continue;
    }
    by_id[*id] = dict;
    const std::optional<int64_t> element = ElementOf(*dict);
    if (element && !dict->FindBool("ignored").value_or(false)) {
      of_element.emplace(*element, *id);
    }
  }
  const auto field = of_element.find(field_);
  if (field == of_element.end()) {
    Fail("The element isn't on the page any more, or is hidden. Take a new snapshot.");
    return;
  }
  // The options to choose from, in the page's order: those in the element, those in what it says
  // it controls, as a combobox does its list, and once it's clicked, those that showed then. A
  // select's are in a popup the page doesn't draw while it's closed, so they count hidden too.
  struct Option {
    const base::DictValue* node;
    bool native;
  };
  std::vector<Option> options;
  std::set<std::string> walked;
  const auto walk = [&](const std::string& root) {
    std::vector<std::pair<std::string, bool>> steps = {{root, false}};
    while (!steps.empty()) {
      auto [id, native] = std::move(steps.back());
      steps.pop_back();
      const auto found = by_id.find(id);
      if (found == by_id.end() || !walked.insert(id).second) {
        continue;
      }
      const base::DictValue& node = *found->second;
      native = native || IsRole(node, "MenuListPopup");
      if (IsRole(node, "option") && (native || !node.FindBool("ignored").value_or(false))) {
        options.push_back({&node, native});
      }
      if (const base::ListValue* children = node.FindList("childIds")) {
        for (const base::Value& child : base::Reversed(*children)) {
          if (const std::string* each = child.GetIfString()) {
            steps.emplace_back(*each, native);
          }
        }
      }
    }
  };
  walk(field->second);
  if (const base::ListValue* properties = by_id[field->second]->FindList("properties")) {
    for (const base::Value& property : *properties) {
      const base::DictValue* dict = property.GetIfDict();
      const std::string* name = dict ? dict->FindString("name") : nullptr;
      const base::ListValue* related =
          name && *name == "controls" ? dict->FindListByDottedPath("value.relatedNodes") : nullptr;
      if (!related) {
        continue;
      }
      for (const base::Value& each : *related) {
        const std::optional<int> node =
            each.is_dict() ? each.GetDict().FindInt("backendDOMNodeId") : std::nullopt;
        const auto controlled =
            node ? of_element.find(ElementId(SlotOfElement(field_), *node)) : of_element.end();
        if (controlled != of_element.end()) {
          walk(controlled->second);
        }
      }
    }
  }
  // Options on screen, in the tree's order, with their elements.
  const auto each_shown = [&nodes](auto then) {
    for (const base::Value& node : nodes) {
      const base::DictValue* dict = node.GetIfDict();
      const std::string* id = dict ? dict->FindString("nodeId") : nullptr;
      const std::optional<int64_t> element = dict ? ElementOf(*dict) : std::nullopt;
      if (id && element && IsRole(*dict, "option") && !dict->FindBool("ignored").value_or(false)) {
        then(*id, *dict, *element);
      }
    }
  };
  if (choosing_ == Choosing::kOpened) {
    each_shown([&](const std::string& id, const base::DictValue& node, int64_t element) {
      if (!shown_.contains(element) && walked.insert(id).second) {
        options.push_back({&node, false});
      }
    });
  }
  if (options.empty()) {
    if (choosing_ == Choosing::kOpened) {
      Fail("Clicking the element showed no options. Take a new snapshot to see what it did.");
      return;
    }
    // Those hidden now that show then count as the element's, as a list it shows does.
    each_shown([this](const std::string&, const base::DictValue&, int64_t element) {
      shown_.insert(element);
    });
    choosing_ = Choosing::kOpened;
    Click();
    return;
  }
  std::vector<const Option*> exact;
  std::vector<const Option*> folded;
  for (const Option& option : options) {
    const std::string name = NameOf(*option.node);
    if (name == want) {
      exact.push_back(&option);
    } else if (Folded(name) == Folded(want)) {
      folded.push_back(&option);
    }
  }
  const std::vector<const Option*>& matches = exact.empty() ? folded : exact;
  if (matches.size() != 1) {
    constexpr size_t kListed = 50;
    std::string why = matches.empty() ? "There's no option “" + want + "” there."
                                      : "Several options are named “" + want + "”.";
    why += " Its options are ";
    for (size_t each = 0; each < options.size() && each < kListed; ++each) {
      std::string name;
      base::TruncateUTF8ToByteSize(NameOf(*options[each].node), 80, &name);
      why += (each ? ", “" : "“") + name + "”";
    }
    if (options.size() > kListed) {
      why += " and " + base::NumberToString(options.size() - kListed) + " more";
    }
    why += ".";
    if (choosing_ == Choosing::kOpened) {
      why += " Clicking the element showed them, so they may still be open.";
    }
    Fail(std::move(why));
    return;
  }
  const Option& option = *matches.front();
  if (Has(*option.node, "disabled")) {
    Fail("The option “" + want + "” is disabled.");
    return;
  }
  chosen_ = ElementOf(*option.node).value_or(0);
  chosen_name_ = NameOf(*option.node);
  choosing_ = Choosing::kChosen;
  if (option.native) {
    // A frame's nodes have its index in the snapshot's list before theirs, as "2:41".
    const std::string* id = option.node->FindString("nodeId");
    const size_t colon = id ? id->find(':') : std::string::npos;
    size_t frame = 0;
    if (colon != std::string::npos && !base::StringToSizeT(id->substr(0, colon), &frame)) {
      frame = frames.size();
    }
    ChooseNative(frames, frame, *option.node);
    return;
  }
  if (!chosen_) {
    Fail("The option “" + want + "” can't be clicked. Take a new snapshot.");
    return;
  }
  job_->element = chosen_;
  Click();
}

void AgentPage::ChooseNative(const std::vector<std::string>& frames,
                             size_t frame,
                             const base::DictValue& option) {
  // The action toggles an option, so choosing the one a select holds would clear it. It's left
  // as it is, and the answer reads as it would had it been chosen.
  if (Has(option, "selected")) {
    Settle();
    return;
  }
  content::RenderFrameHost* host = nullptr;
  if (frame == 0) {
    host = GetWebContents().GetPrimaryMainFrame();
  } else if (frame < frames.size()) {
    GetWebContents().GetPrimaryMainFrame()->ForEachRenderFrameHost(
        [&host, &id = frames[frame]](content::RenderFrameHost* each) {
          if (each->GetDevToolsFrameToken().ToString() == id) {
            host = each;
          }
        });
  }
  // DevTools gives a node's accessibility id, after its frame's index.
  const std::string* id = option.FindString("nodeId");
  const size_t colon = id ? id->rfind(':') : std::string::npos;
  int node = 0;
  if (!host || !IsWebFrame(host) || !id ||
      !base::StringToInt(colon == std::string::npos ? *id : id->substr(colon + 1), &node)) {
    Fail("The element isn't on the page any more. Take a new snapshot.");
    return;
  }
  if (!accessibility_) {
    accessibility_ =
        content::BrowserAccessibilityState::GetInstance()->CreateScopedModeForWebContents(
            &GetWebContents(), ui::AXMode::kWebContents);
  }
  // Content gives a hidden tab, as an agent's is, the mode only once it's shown, unless it was
  // asked for that tab alone (engine/patches/ours/chromium-core-agent-accessibility.patch).
  // Without it the renderer never hears of the action.
  if (!GetWebContents().GetAccessibilityMode().has_mode(ui::AXMode::kWebContents)) {
    Fail("This browser's engine can't choose in a select in a tab that isn't shown.");
    return;
  }
  ui::AXActionData action;
  action.action = ax::mojom::Action::kDoDefault;
  action.target_tree_id = host->GetAXTreeID();
  action.target_node_id = node;
  host->AccessibilityPerformAction(action);
  Settle();
}

bool AgentPage::Shows(const base::ListValue& nodes) {
  const std::u16string chosen = Folded(chosen_name_);
  for (const base::Value& node : nodes) {
    const base::DictValue* dict = node.GetIfDict();
    const std::optional<int64_t> element = dict ? ElementOf(*dict) : std::nullopt;
    if (!element || chosen.empty()) {
      continue;
    }
    if (*element == chosen_ && Has(*dict, "selected")) {
      return true;
    }
    if (*element != field_) {
      continue;
    }
    // What a field holds is never told: only whether it's the option the agent chose. A
    // dropdown's button may show it as its name instead.
    const std::string* value = dict->FindStringByDottedPath("value.value");
    const std::string held =
        value ? base::CollapseWhitespaceASCII(*value, /*trim_sequences_with_line_breaks=*/true)
              : std::string();
    if (Folded(held) == chosen || Folded(NameOf(*dict)) == chosen) {
      return true;
    }
  }
  return false;
}

void AgentPage::EndChoosing() {
  choosing_ = Choosing::kNo;
  shown_.clear();
  field_ = 0;
  chosen_ = 0;
  chosen_name_.clear();
  shows_ = false;
  accessibility_.reset();
}

void AgentPage::ClickAt(int slot, gfx::PointF at, size_t step) {
  static constexpr const char* kSteps[] = {"mouseMoved", "mousePressed", "mouseReleased"};
  if (step == std::size(kSteps)) {
    Settle();
    return;
  }
  Mouse(kSteps[step], at, slot,
        base::BindOnce(&AgentPage::ClickAt, weak_factory_.GetWeakPtr(), slot, at, step + 1));
}

void AgentPage::Mouse(const char* type, gfx::PointF at, int slot, base::OnceClosure then) {
  base::DictValue params = base::DictValue()
                               .Set("type", type)
                               .Set("x", static_cast<double>(at.x()))
                               .Set("y", static_cast<double>(at.y()));
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
           weak_factory_.GetWeakPtr(), std::move(then)),
       slot);
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
                   page->Ready(
                       /*enabled=*/true,
                       base::BindOnce(
                           [](base::WeakPtr<AgentPage> page, int64_t element, bool submit,
                              std::optional<gfx::RectF> box) {
                             if (!page) {
                               return;
                             }
                             if (box) {
                               page->Point(*box);
                             }
                             page->Listen(base::BindOnce(&AgentPage::Focus, page, element, submit));
                           },
                           page, element, submit));
                 },
                 weak_factory_.GetWeakPtr(), element, submit));
}

void AgentPage::Focus(int64_t element, bool submit) {
  // Focus moves to the element's frame, where the main frame's keys then go.
  Send("DOM.focus", base::DictValue().Set("backendNodeId", NodeOfElement(element)),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, int64_t element, bool submit,
              const base::DictValue* focused) {
             if (!page) {
               return;
             }
             if (!focused) {
               page->Fail("The element can't take typing. Take a new snapshot.");
               return;
             }
             // Focusing it can bring an extension's menu, which may take the keyboard.
             if (page->KeysInExtension()) {
               page->RefuseExtension(kExtensionKeyboard);
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
             page->TypeInto(element, submit);
           },
           weak_factory_.GetWeakPtr(), element, submit),
       SlotOfElement(element));
}

void AgentPage::TypeInto(int64_t element, bool submit) {
  std::u16string text = base::UTF8ToUTF16(job_->text);
  base::ReplaceSubstringsAfterOffset(&text, 0, u"\r\n", u"\n");
  std::ranges::replace(text, u'\r', u'\n');
  if (text.find(u'\n') == std::u16string::npos) {
    TypeKeys(std::move(text), submit);
    return;
  }
  // An input holds one line, where Enter would submit its form: its line breaks go, as they
  // would had the text been pasted.
  Send("DOM.describeNode", base::DictValue().Set("backendNodeId", NodeOfElement(element)),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, std::u16string text, bool submit,
              const base::DictValue* described) {
             if (!page) {
               return;
             }
             const std::string* name =
                 described ? described->FindStringByDottedPath("node.nodeName") : nullptr;
             if (name && base::EqualsCaseInsensitiveASCII(*name, "input")) {
               std::erase(text, u'\n');
             }
             page->TypeKeys(std::move(text), submit);
           },
           weak_factory_.GetWeakPtr(), std::move(text), submit),
       SlotOfElement(element));
}

void AgentPage::TypeKeys(std::u16string text, bool submit) {
  auto typed = base::BindOnce(
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
      weak_factory_.GetWeakPtr(), submit);
  // Nothing to type clears what's selected, as Delete would.
  if (text.empty()) {
    const base::DictValue key = base::DictValue()
                                    .Set("key", "Delete")
                                    .Set("code", "Delete")
                                    .Set("windowsVirtualKeyCode", 46);
    Send("Input.dispatchKeyEvent", key.Clone().Set("type", "rawKeyDown"), base::DoNothing());
    Send("Input.dispatchKeyEvent", key.Clone().Set("type", "keyUp"), std::move(typed));
    return;
  }
  // DevTools' key events rather than its insertText, as a key carries DevTools' mark of input
  // that isn't the user's, which keeps it from an extension's listeners
  // (engine/patches/ours/chromium-agent-extension-listeners.patch), and an IME's commit doesn't.
  std::vector<base::DictValue> keys = TypingKeys(text);
  if (keys.empty()) {
    const base::DictValue none;
    std::move(typed).Run(&none);
    return;
  }
  for (size_t each = 0; each < keys.size(); ++each) {
    if (each + 1 < keys.size()) {
      Send("Input.dispatchKeyEvent", std::move(keys[each]), base::DoNothing());
    } else {
      Send("Input.dispatchKeyEvent", std::move(keys[each]), std::move(typed));
    }
  }
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
  if (KeysInExtension()) {
    RefuseExtension(kExtensionKeyboard);
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
  // The page's requests from the wheel on are heard, so the answer waits for what it starts.
  Listen(base::BindOnce(&AgentPage::TurnWheel, weak_factory_.GetWeakPtr(), down));
}

void AgentPage::TurnWheel(bool down) {
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
             const gfx::RectF view(static_cast<float>(width), static_cast<float>(height));
             if (view.IsEmpty()) {
               page->Fail("The page has no view to scroll.");
               return;
             }
             if (!page->job_->element) {
               page->Wheel(view, down);
               return;
             }
             page->Center(base::BindOnce(
                 [](base::WeakPtr<AgentPage> page, bool down, gfx::RectF view,
                    std::optional<gfx::RectF> box) {
                   if (!page) {
                     return;
                   }
                   // Its part of the view, where a user's pointer would be.
                   const gfx::RectF shown = box ? gfx::IntersectRects(*box, view) : gfx::RectF();
                   if (shown.IsEmpty()) {
                     page->Fail(
                         "The element isn't on the page any more, or is hidden. Take a new "
                         "snapshot.");
                     return;
                   }
                   page->Wheel(shown, down);
                 },
                 page, down, view));
           },
           weak_factory_.GetWeakPtr(), down));
}

void AgentPage::Wheel(const gfx::RectF& area, bool down) {
  const gfx::PointF at = area.CenterPoint();
  // From the main frame, where the wheel goes, down through the frames it goes into.
  Covered(at,
          base::BindOnce(
              [](base::WeakPtr<AgentPage> page, gfx::PointF at, bool down, float height,
                 bool covered) {
                if (!page || !page->job_) {
                  return;
                }
                if (covered) {
                  page->RefuseExtension(kExtensionCovers);
                  return;
                }
                page->TurnWheelAt(at, down, height);
              },
              weak_factory_.GetWeakPtr(), at, down, area.height()));
}

void AgentPage::TurnWheelAt(gfx::PointF at, bool down, float height) {
  wheel_ = std::make_unique<WheelWatch>(GetWebContents());
  Send("Input.dispatchMouseEvent",
       base::DictValue()
           .Set("type", "mouseWheel")
           .Set("x", at.x())
           .Set("y", at.y())
           .Set("deltaX", 0)
           .Set("deltaY", (down ? 0.8 : -0.8) * height),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, bool down, const base::DictValue*) {
             if (page) {
               // By then the scroll the wheel started has scrolled, or found nothing to.
               page->wait_.Start(FROM_HERE, kSettle,
                                 base::BindOnce(&AgentPage::Scrolled, page, down));
             }
           },
           weak_factory_.GetWeakPtr(), down));
}

void AgentPage::Scrolled(bool down) {
  const bool moved = wheel_ && wheel_->moved();
  wheel_.reset();
  if (!moved) {
    Fail(down ? "Nothing scrolled: it's as far down as it goes, or nothing there scrolls."
              : "Nothing scrolled: it's as far up as it goes, or nothing there scrolls.");
    return;
  }
  // Then it waits for the page to settle, as after any input.
  Settle();
}

void AgentPage::ScrollTo() {
  // Readiness scrolls it into view, as each of its looks does.
  Ready(/*enabled=*/false,
        base::BindOnce(
            [](base::WeakPtr<AgentPage> page, std::optional<gfx::RectF> box) {
              if (!page) {
                return;
              }
              if (!box) {
                page->Fail(
                    "The element isn't on the page any more, or is hidden. Take a new snapshot.");
                return;
              }
              page->Listen(base::BindOnce(&AgentPage::Settle, page));
            },
            weak_factory_.GetWeakPtr()));
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
  if (!GetWebContents().GetRenderWidgetHostView()) {
    Fail("The page couldn't be pictured.");
    return;
  }
  // Should no frame come, the agent hears so after as long as a page that won't load is waited for
  // (#1461), rather than never.
  const uint64_t request = job_->request;
  wait_.Start(FROM_HERE, kLoadTimeout,
              base::BindOnce(
                  [](AgentPage* page, uint64_t request) {
                    if (page->job_ && page->job_->request == request) {
                      page->Fail(base::StrCat(
                          {"The tab didn't draw a picture in ",
                           base::NumberToString(kLoadTimeout.InSeconds()),
                           " s. Try again, or ask the user to bring it to the front."}));
                    }
                  },
                  base::Unretained(this), request));
  Draw(std::nullopt,
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, uint64_t request,
              base::expected<SkBitmap, std::string> picture) {
             if (!page || !page->job_ || page->job_->request != request) {
               return;
             }
             if (!picture.has_value()) {
               page->wait_.Stop();
               page->Fail(std::move(picture.error()));
               return;
             }
             // A page in a big window is millions of pixels, too many to encode on the UI thread.
             base::ThreadPool::PostTaskAndReplyWithResult(
                 FROM_HERE, {base::TaskPriority::USER_VISIBLE},
                 base::BindOnce(
                     [](const SkBitmap& picture) {
                       std::optional<std::vector<uint8_t>> jpeg =
                           gfx::JPEGCodec::Encode(picture, /*quality=*/75);
                       return jpeg ? base::Base64Encode(*jpeg) : std::string();
                     },
                     std::move(*picture)),
                 base::BindOnce(
                     [](base::WeakPtr<AgentPage> page, uint64_t request, std::string data) {
                       if (!page || !page->job_ || page->job_->request != request) {
                         return;
                       }
                       page->wait_.Stop();
                       if (data.empty()) {
                         page->Fail("The page couldn't be pictured.");
                         return;
                       }
                       page->Answer(std::move(data), /*error=*/false);
                     },
                     page, request));
           },
           weak_factory_.GetWeakPtr(), request));
}

// static
void AgentPage::Picture(content::WebContents& contents,
                        std::optional<gfx::Rect> area,
                        Pictured done) {
  CreateForWebContents(&contents);
  AgentPage* page = FromWebContents(&contents);
  // Posted, so the caller never hears inside its own call, and heard even if the page goes or its
  // agent's session ends first.
  done = mojo::WrapCallbackWithDefaultInvokeIfNotRun(
      base::BindPostTaskToCurrentDefault(std::move(done)),
      base::expected<SkBitmap, std::string>(base::unexpected(kNoPicture)));
  ++page->pictures_;
  page->Draw(area, base::BindOnce(
                       [](base::WeakPtr<AgentPage> page, Pictured done,
                          base::expected<SkBitmap, std::string> picture) {
                         if (page && --page->pictures_ == 0 && !page->acted_) {
                           // Posted, as this may be inside DevTools' own reply.
                           base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
                               FROM_HERE, base::BindOnce(&AgentPage::Rest, page));
                         }
                         std::move(done).Run(std::move(picture));
                       },
                       page->weak_factory_.GetWeakPtr(), std::move(done)));
}

void AgentPage::Draw(std::optional<gfx::Rect> area, Pictured done) {
  if (agent_host_) {
    WithheldBoxes(
        base::BindOnce(&AgentPage::Copy, weak_factory_.GetWeakPtr(), area, std::move(done)));
    return;
  }
  // DevTools tells of the page's frames in other processes as it attaches to each level of them,
  // and a frame not heard of yet would be hatched whole.
  Send("Target.setAutoAttach", AutoAttaching(),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, std::optional<gfx::Rect> area, Pictured done,
              const base::DictValue* result) {
             if (!page) {
               return;
             }
             if (!result) {
               std::move(done).Run(base::unexpected(kNoPicture));
               return;
             }
             page->Attach(1, base::BindOnce(&AgentPage::Draw, page, area, std::move(done)));
           },
           weak_factory_.GetWeakPtr(), area, std::move(done)),
       0);
}

void AgentPage::Attach(size_t slot, base::OnceClosure then) {
  for (; slot < targets_.size(); ++slot) {
    if (!targets_[slot].session.empty()) {
      // Its reply comes after DevTools has told of the frames it attached to.
      Send("Target.setAutoAttach", AutoAttaching(),
           base::BindOnce(
               [](base::WeakPtr<AgentPage> page, size_t slot, base::OnceClosure then,
                  const base::DictValue*) {
                 if (page) {
                   page->Attach(slot + 1, std::move(then));
                 }
               },
               weak_factory_.GetWeakPtr(), slot, std::move(then)),
           static_cast<int>(slot));
      return;
    }
  }
  std::move(then).Run();
}

void AgentPage::Copy(std::optional<gfx::Rect> area,
                     Pictured done,
                     std::optional<Boxes> before) {
  content::RenderWidgetHostView* view = GetWebContents().GetRenderWidgetHostView();
  const gfx::Rect source =
      !view ? gfx::Rect()
            : gfx::IntersectRects(area.value_or(gfx::Rect(view->GetViewBounds().size())),
                                  gfx::Rect(view->GetViewBounds().size()));
  if (!before || source.IsEmpty()) {
    std::move(done).Run(base::unexpected(before ? kNoPicture : kPictureChanged));
    return;
  }
  // A copy of the page's own surface, as the app's pictures of a page are (capture.cc). DevTools'
  // capture waits for a redraw to be presented, which a page out of sight gets only as its window
  // draws something new, so with the UI holding the keyboard and the window still it never came. A
  // page no window shows is held in its tab's window, unseen, which gives it a compositor.
  base::ScopedClosureRunner held;
  if (!view->IsShowing()) {
    Window* window = Window::Holding(tab_);
    held = platform::DrawUnseen(window ? window->widget() : nullptr,
                                GetWebContents().GetNativeView());
  }
  view->CopyFromSurface(
      source, gfx::Size(), base::TimeDelta(),
      base::BindOnce(
          [](base::WeakPtr<AgentPage> page, gfx::Rect source, Pictured done, Boxes before,
             base::ScopedClosureRunner held, const content::CopyFromSurfaceResult& result) {
            if (!page) {
              return;
            }
            if (!result.has_value() || result->bitmap.drawsNothing()) {
              std::move(done).Run(base::unexpected(kNoPicture));
              return;
            }
            // Where the boxes are after the copy too, as the page may have moved one meanwhile,
            // as a smooth scroll does: each is painted over all the way from one box to the other.
            page->WithheldBoxes(base::BindOnce(&AgentPage::Paint, page, source, std::move(done),
                                               std::move(before), result->bitmap));
          },
          weak_factory_.GetWeakPtr(), source, std::move(done), std::move(*before),
          std::move(held)));
}

void AgentPage::Paint(gfx::Rect source,
                      Pictured done,
                      Boxes before,
                      SkBitmap bitmap,
                      std::optional<Boxes> after) {
  if (!after) {
    std::move(done).Run(base::unexpected(kPictureChanged));
    return;
  }
  for (const auto& [element, box] : *after) {
    const auto [field, added] = before.emplace(element, box);
    if (!added) {
      field->second.Union(box);
    }
  }
  // The page's CSS pixels, at its zoom, are the view's points (Point), and the picture has the
  // screen's pixels.
  const float zoom = static_cast<float>(
      blink::ZoomLevelToZoomFactor(content::HostZoomMap::GetZoomLevel(&GetWebContents())));
  const gfx::Size size(bitmap.width(), bitmap.height());
  std::vector<gfx::Rect> boxes;
  for (const auto& [element, box] : before) {
    const gfx::Rect pixels = InPicture(box, zoom, source, size);
    if (!pixels.IsEmpty()) {
      boxes.push_back(pixels);
    }
  }
  const float scale =
      zoom * static_cast<float>(size.width()) / static_cast<float>(source.width());
  // A page in a big window is millions of pixels, too many to paint on the UI thread.
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::TaskPriority::USER_VISIBLE},
      base::BindOnce(
          [](const SkBitmap& bitmap, std::vector<gfx::Rect> boxes,
             float scale) -> std::optional<SkBitmap> {
            SkBitmap picture;
            // Painted on a copy of its own, as the copy's pixels may be shared.
            if (!picture.tryAllocPixels(bitmap.info()) || !bitmap.readPixels(picture.pixmap())) {
              return std::nullopt;
            }
            Hatch(picture, boxes, scale);
            return picture;
          },
          std::move(bitmap), std::move(boxes), scale),
      base::BindOnce(
          [](Pictured done, std::optional<SkBitmap> picture) {
            if (!picture) {
              std::move(done).Run(base::unexpected(kNoPicture));
              return;
            }
            std::move(done).Run(std::move(*picture));
          },
          std::move(done)));
}

void AgentPage::Rest() {
  if (!acted_ && !pictures_ && !job_ && jobs_.empty()) {
    Detach();
  }
}

AgentPage::Withholding::Withholding() = default;
AgentPage::Withholding::~Withholding() = default;

void AgentPage::WithheldBoxes(base::OnceCallback<void(std::optional<Boxes>)> then) {
  auto withholding = std::make_unique<Withholding>();
  withholding->frames = Frames();
  withholding->then = std::move(then);
  // A frame below one whose fields are read that can't be read itself, as an extension's own, one
  // not live or one whose session hasn't started, is hatched whole, as the outline leaves it out.
  // What's in it is in its box.
  content::RenderFrameHost* main = GetWebContents().GetPrimaryMainFrame();
  std::map<std::string, int> read;
  for (const Frame& frame : withholding->frames) {
    read[frame.id] = frame.slot;
  }
  main->ForEachRenderFrameHost([&](content::RenderFrameHost* host) {
    content::RenderFrameHost* parent = host->GetParent();
    if (!parent) {
      return;
    }
    const std::string id = host->GetDevToolsFrameToken().ToString();
    const auto above =
        read.find(parent == main ? std::string() : parent->GetDevToolsFrameToken().ToString());
    if (above != read.end() && !read.contains(id)) {
      withholding->unread.emplace_back(above->second, id);
    }
  });
  ReadFields(std::move(withholding), 0);
}

void AgentPage::ReadFields(std::unique_ptr<Withholding> withholding, size_t next) {
  if (next >= withholding->frames.size()) {
    Cover(std::move(withholding), 0);
    return;
  }
  const Frame& frame = withholding->frames[next];
  const int slot = frame.slot;
  base::DictValue params;
  if (next > 0) {
    params.Set("frameId", frame.id);
  }
  Send("Accessibility.getFullAXTree", std::move(params),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, std::unique_ptr<Withholding> withholding, size_t next,
              int slot, const base::DictValue* result) {
             if (!page) {
               return;
             }
             const base::ListValue* nodes = result ? result->FindList("nodes") : nullptr;
             // A frame whose fields can't be read may show anything: no picture, rather.
             if (!nodes) {
               std::move(withholding->then).Run(std::nullopt);
               return;
             }
             for (const base::Value& node : *nodes) {
               const base::DictValue* dict = node.GetIfDict();
               const std::optional<std::string> value = dict ? ValueOf(*dict) : std::nullopt;
               const std::optional<int> backend =
                   dict ? dict->FindInt("backendDOMNodeId") : std::nullopt;
               if (!value || value->empty() || !backend || !IsField(*dict)) {
                 continue;
               }
               const int64_t element = ElementId(slot, *backend);
               FieldFacts facts;
               facts.value = base::UTF8ToUTF16(*value);
               facts.secret = Has(*dict, "wasPassword");
               facts.autofilled = Has(*dict, "autofilled");
               facts.fixed = IsDisabled(node) || Has(*dict, "readonly");
               if (const auto typed = page->typed_.find(element); typed != page->typed_.end()) {
                 facts.typed = typed->second;
               }
               withholding->fields.emplace_back(element, std::move(facts));
             }
             page->ReadFields(std::move(withholding), next + 1);
           },
           weak_factory_.GetWeakPtr(), std::move(withholding), next, slot),
       slot);
}

void AgentPage::Cover(std::unique_ptr<Withholding> withholding, size_t next) {
  if (next >= withholding->unread.size()) {
    NextField(std::move(withholding), 0);
    return;
  }
  const auto& [slot, frame] = withholding->unread[next];
  const int in = slot;
  Send("DOM.getFrameOwner", base::DictValue().Set("frameId", frame),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, std::unique_ptr<Withholding> withholding, size_t next,
              int slot, const base::DictValue* result) {
             if (!page) {
               return;
             }
             const std::optional<int> owner =
                 result ? result->FindInt("backendNodeId") : std::nullopt;
             // A frame whose element can't be found may show anything: no picture, rather.
             if (!owner) {
               std::move(withholding->then).Run(std::nullopt);
               return;
             }
             page->Send(
                 "DOM.getBoxModel", base::DictValue().Set("backendNodeId", *owner),
                 base::BindOnce(
                     [](base::WeakPtr<AgentPage> page, std::unique_ptr<Withholding> withholding,
                        size_t next, int slot, int64_t element, const base::DictValue* result) {
                       if (!page) {
                         return;
                       }
                       // Its content box, where the frame's page shows.
                       const base::ListValue* content =
                           result ? result->FindListByDottedPath("model.content") : nullptr;
                       page->Keep(std::move(withholding), slot, element,
                                  content ? BoundsOf(*content) : std::nullopt, &AgentPage::Cover,
                                  next + 1);
                     },
                     page, std::move(withholding), next, slot, ElementId(slot, *owner)),
                 slot);
           },
           weak_factory_.GetWeakPtr(), std::move(withholding), next, in),
       in);
}

void AgentPage::NextField(std::unique_ptr<Withholding> withholding, size_t next) {
  if (next >= withholding->fields.size()) {
    base::OnceCallback<void(std::optional<Boxes>)> then = std::move(withholding->then);
    std::move(then).Run(std::move(withholding->boxes));
    return;
  }
  const int64_t element = withholding->fields[next].first;
  // Its attributes, then a textarea's text or a select's options, for its value in the markup.
  Send("DOM.describeNode", base::DictValue().Set("backendNodeId", NodeOfElement(element)),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, std::unique_ptr<Withholding> withholding, size_t next,
              const base::DictValue* result) {
             if (!page) {
               return;
             }
             const int64_t element = withholding->fields[next].first;
             const base::DictValue* node = result ? result->FindDict("node") : nullptr;
             const std::string* name = node ? node->FindString("nodeName") : nullptr;
             const int depth = !name ? 0 : *name == "SELECT" ? 3 : *name == "TEXTAREA" ? 1 : 0;
             if (!depth) {
               page->Withhold(std::move(withholding), next, result);
               return;
             }
             page->Send("DOM.describeNode",
                        base::DictValue()
                            .Set("backendNodeId", NodeOfElement(element))
                            .Set("depth", depth),
                        base::BindOnce(&AgentPage::Withhold, page, std::move(withholding), next),
                        SlotOfElement(element));
           },
           weak_factory_.GetWeakPtr(), std::move(withholding), next),
       SlotOfElement(element));
}

void AgentPage::Withhold(std::unique_ptr<Withholding> withholding,
                         size_t next,
                         const base::DictValue* described) {
  const int64_t element = withholding->fields[next].first;
  FieldFacts& facts = withholding->fields[next].second;
  if (const base::DictValue* node = described ? described->FindDict("node") : nullptr) {
    const std::optional<std::string> type = AttributeOf(*node, "type");
    const std::optional<std::string> autocomplete = AttributeOf(*node, "autocomplete");
    facts.secret |= (type && base::EqualsCaseInsensitiveASCII(*type, "password")) ||
                    (autocomplete && SecretAutocomplete(*autocomplete));
    facts.markup = MarkupOf(*node);
    // A select's value is its option's label, as the tree reads it, its runs of spaces one.
    const std::string* name = node->FindString("nodeName");
    if (name && *name == "SELECT") {
      facts.value = base::CollapseWhitespace(facts.value, /*trim_sequences_with_line_breaks=*/true);
    }
  } else {
    // One that couldn't be read is withheld.
    facts.secret = true;
  }
  if (!Withheld(facts)) {
    NextField(std::move(withholding), next + 1);
    return;
  }
  const int slot = SlotOfElement(element);
  Send("DOM.getBoxModel", base::DictValue().Set("backendNodeId", NodeOfElement(element)),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, std::unique_ptr<Withholding> withholding, size_t next,
              int slot, const base::DictValue* result) {
             if (!page) {
               return;
             }
             // Its padding box, inside its border, where it shows what it holds.
             const base::ListValue* padding =
                 result ? result->FindListByDottedPath("model.padding") : nullptr;
             const int64_t element = withholding->fields[next].first;
             page->Keep(std::move(withholding), slot, element,
                        padding ? BoundsOf(*padding) : std::nullopt, &AgentPage::NextField,
                        next + 1);
           },
           weak_factory_.GetWeakPtr(), std::move(withholding), next, slot),
       slot);
}

void AgentPage::Lift(int slot, gfx::RectF box, Then then) {
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
             page->Send(
                 "DOM.getBoxModel", base::DictValue().Set("backendNodeId", *owner),
                 base::BindOnce(
                     [](base::WeakPtr<AgentPage> page, int up, gfx::RectF box, Then then,
                        const base::DictValue* result) {
                       if (!page) {
                         return;
                       }
                       // The frame's page shows in its element's content box; a frame with none
                       // isn't drawn.
                       const base::ListValue* content =
                           result ? result->FindListByDottedPath("model.content") : nullptr;
                       const std::optional<gfx::RectF> frame =
                           content ? BoundsOf(*content) : std::nullopt;
                       if (!frame) {
                         std::move(then).Run(gfx::RectF());
                         return;
                       }
                       page->Lift(up, IntoParent(box, *frame), std::move(then));
                     },
                     page, up, box, std::move(then)),
                 up);
           },
           weak_factory_.GetWeakPtr(), up, box, std::move(then)),
       up);
}

void AgentPage::Keep(std::unique_ptr<Withholding> withholding,
                     int slot,
                     int64_t element,
                     std::optional<gfx::RectF> box,
                     void (AgentPage::*go_on)(std::unique_ptr<Withholding>, size_t),
                     size_t next) {
  if (!box) {
    (this->*go_on)(std::move(withholding), next);
    return;
  }
  Lift(slot, *box,
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, std::unique_ptr<Withholding> withholding,
              int64_t element, void (AgentPage::*go_on)(std::unique_ptr<Withholding>, size_t),
              size_t next, std::optional<gfx::RectF> lifted) {
             if (!page) {
               return;
             }
             if (!lifted) {
               std::move(withholding->then).Run(std::nullopt);
               return;
             }
             if (!lifted->IsEmpty()) {
               withholding->boxes[element] = *lifted;
             }
             ((*page).*go_on)(std::move(withholding), next);
           },
           weak_factory_.GetWeakPtr(), std::move(withholding), element, go_on, next));
}

void AgentPage::KeepTyped(const base::ListValue& nodes, int64_t element) {
  for (const base::Value& node : nodes) {
    const base::DictValue* dict = node.GetIfDict();
    if (dict && ElementOf(*dict) == element) {
      typed_[element] = base::UTF8ToUTF16(ValueOf(*dict).value_or(std::string()));
      return;
    }
  }
  typed_.erase(element);
}

void AgentPage::Snapshot() {
  // After an action, the page may have gone somewhere agents may not.
  if (!IsWebPage()) {
    Fail(NotWebPage());
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
             page->Read(nodes->Clone());
           },
           weak_factory_.GetWeakPtr()));
}

void AgentPage::Read(base::ListValue nodes) {
  // Then the trees of its frames, read after it, as only then has DevTools told of the sessions
  // of those in other processes.
  frames_ = Frames();
  frames_[0].nodes = std::move(nodes);
  ReadTree(1);
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
    if (choosing_ == Choosing::kOptions || choosing_ == Choosing::kOpened) {
      std::vector<std::string> ids;
      for (const Frame& frame : frames) {
        ids.push_back(frame.id);
      }
      Choose(nodes, ids);
      return;
    }
    if (choosing_ == Choosing::kChosen) {
      shows_ = Shows(nodes);
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
    // What the agent's typing or choosing left, read once the page settled, which a field then
    // shows only while it still holds exactly that.
    if (job_ && (job_->action == BC_AGENT_PAGE_TYPE || job_->action == BC_AGENT_PAGE_TYPE_SUBMIT)) {
      KeepTyped(nodes, job_->element);
    } else if (choosing_ == Choosing::kChosen && field_) {
      KeepTyped(nodes, field_);
    }
    base::DictValue answer = base::DictValue().Set("nodes", AgentTree(std::move(nodes), passwords));
    if (job_ && job_->busy) {
      answer.Set("busy", true);
    } else if (job_ && job_->requests) {
      answer.Set("requests", true);
    }
    if (choosing_ == Choosing::kChosen && shows_) {
      answer.Set("chosen", chosen_name_);
    }
    // The ids of elements in frames of their own are doubles, written as the integers they are.
    std::optional<std::string> json = base::WriteJsonWithOptions(
        base::Value(std::move(answer)), base::JSONWriter::OPTIONS_OMIT_DOUBLE_TYPE_PRESERVATION);
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
  Locate(base::BindOnce(
      [](base::WeakPtr<AgentPage> page, int slot, Then then, std::optional<gfx::RectF> box) {
        if (!page) {
          return;
        }
        if (!box) {
          std::move(then).Run(std::nullopt);
          return;
        }
        page->Offset(slot, *box, std::move(then));
      },
      weak_factory_.GetWeakPtr(), SlotOfElement(job_->element), std::move(then)));
}

void AgentPage::Locate(Then then) {
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
                               [](base::WeakPtr<AgentPage> page, Then then,
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
                                 std::move(then).Run(box);
                               },
                               page, std::move(then)),
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

void AgentPage::Covered(gfx::PointF at, base::OnceCallback<void(bool)> then) {
  CoveredIn(0, at, 0, std::move(then));
}

void AgentPage::CoveredIn(int slot,
                          gfx::PointF at,
                          int depth,
                          base::OnceCallback<void(bool)> then) {
  Send("DOM.getNodeForLocation",
       base::DictValue().Set("x", static_cast<int>(at.x())).Set("y", static_cast<int>(at.y())),
       base::BindOnce(
           [](base::WeakPtr<AgentPage> page, int slot, gfx::PointF at, int depth,
              base::OnceCallback<void(bool)> then, const base::DictValue* found) {
             if (!page) {
               return;
             }
             const std::optional<int> node = found ? found->FindInt("backendNodeId") : std::nullopt;
             // Nothing there the page can say, as over its margin: what's found next is the page's.
             if (!node) {
               std::move(then).Run(false);
               return;
             }
             page->Send(
                 "DOM.describeNode", base::DictValue().Set("backendNodeId", *node),
                 base::BindOnce(&AgentPage::CoveredBy, page, slot, at, depth, *node,
                                std::move(then)),
                 slot);
           },
           weak_factory_.GetWeakPtr(), slot, at, depth, std::move(then)),
       slot);
}

void AgentPage::CoveredBy(int slot,
                          gfx::PointF at,
                          int depth,
                          int node,
                          base::OnceCallback<void(bool)> then,
                          const base::DictValue* described) {
  // A frame's element: the frame it holds takes the input, and one that isn't a web page's is an
  // extension's, or the browser's.
  const std::string* frame =
      described ? described->FindStringByDottedPath("node.frameId") : nullptr;
  content::RenderFrameHost* inner = nullptr;
  if (frame) {
    GetWebContents().GetPrimaryMainFrame()->ForEachRenderFrameHost(
        [&inner, frame](content::RenderFrameHost* each) {
          if (each->GetDevToolsFrameToken().ToString() == *frame) {
            inner = each;
          }
        });
  }
  if (inner && !IsWebFrame(inner)) {
    std::move(then).Run(true);
    return;
  }
  // A page's frame in a process of its own, whose own session finds what's under the point there,
  // which is its frame's element's content box's top left on. One in the same process the first
  // look went into.
  const int below = inner ? SlotOf(inner) : -1;
  constexpr int kDeepest = 8;
  if (below >= 0 && below != slot && depth < kDeepest) {
    Send("DOM.getBoxModel", base::DictValue().Set("backendNodeId", node),
         base::BindOnce(
             [](base::WeakPtr<AgentPage> page, int below, gfx::PointF at, int depth,
                base::OnceCallback<void(bool)> then, const base::DictValue* result) {
               if (!page) {
                 return;
               }
               const base::ListValue* content =
                   result ? result->FindListByDottedPath("model.content") : nullptr;
               if (!content || content->size() < 2) {
                 std::move(then).Run(false);
                 return;
               }
               at.Offset(-static_cast<float>((*content)[0].GetIfDouble().value_or(0)),
                         -static_cast<float>((*content)[1].GetIfDouble().value_or(0)));
               page->CoveredIn(below, at, depth + 1, std::move(then));
             },
             weak_factory_.GetWeakPtr(), below, at, depth, std::move(then)),
         slot);
    return;
  }
  Send("Accessibility.getPartialAXTree",
       base::DictValue().Set("backendNodeId", node).Set("fetchRelatives", false),
       base::BindOnce(
           [](base::OnceCallback<void(bool)> then, const base::DictValue* tree) {
             const base::ListValue* nodes = tree ? tree->FindList("nodes") : nullptr;
             bool made = false;
             for (size_t i = 0; nodes && i < nodes->size(); ++i) {
               if (const base::DictValue* dict = (*nodes)[i].GetIfDict()) {
                 made = made || Has(*dict, "fromExtension");
               }
             }
             std::move(then).Run(made);
           },
           std::move(then)),
       slot);
}

bool AgentPage::KeysInExtension() {
  content::RenderFrameHost* focused = GetWebContents().GetFocusedFrame();
  return focused && !IsWebFrame(focused);
}

void AgentPage::RefuseExtension(std::string why) {
  static constexpr std::pair<int, const char*> kActions[] = {
      {BC_AGENT_PAGE_CLICK, "click"},           {BC_AGENT_PAGE_TYPE, "type"},
      {BC_AGENT_PAGE_TYPE_SUBMIT, "type"},      {BC_AGENT_PAGE_KEY, "press_key"},
      {BC_AGENT_PAGE_SCROLL_DOWN, "scroll"},    {BC_AGENT_PAGE_SCROLL_UP, "scroll"},
      {BC_AGENT_PAGE_SELECT, "select_option"},
  };
  const char* action = "click";
  for (const auto& [each, name] : kActions) {
    if (job_ && job_->action == each) {
      action = name;
    }
  }
  const Space* space = Space::Holding(tab_);
  if (space && space->agent() && Host().agent_activity) {
    // check.sh reads it in the log, as Settings › Agents lists it.
    const bc_agent_activity entry = {
        .agent = space->agent()->id.c_str(),
        .tab = tab_,
        .action = action,
        .refused = why.c_str(),
    };
    Host().agent_activity(Host().context, &entry);
  }
  Fail(std::move(why));
}

// static
void AgentPage::AppRefused(content::WebContents* contents, const GURL& url) {
  AgentPage* page = contents ? FromWebContents(contents) : nullptr;
  if (!page || !page->job_) {
    return;
  }
  page->app_link_ = url;
  page->app_name_.clear();
  // As Chrome finds it for its own question, off the UI thread: on Linux it reads desktop files.
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::MayBlock(), base::TaskPriority::USER_VISIBLE},
      base::BindOnce(&shell_integration::GetApplicationNameForScheme, url),
      base::BindOnce(
          [](base::WeakPtr<AgentPage> page, const GURL& url, const std::u16string& name) {
            if (page && page->app_link_ == url) {
              page->app_name_ = name;
            }
          },
          page->weak_factory_.GetWeakPtr(), url));
}

void AgentPage::Answer(std::string text, bool error) {
  if (!job_) {
    return;
  }
  // A link to an app, which the core stopped, is what the action did, whatever the page shows
  // after (#1443).
  if (std::optional<GURL> app = std::exchange(app_link_, std::nullopt); app && !error) {
    const std::string opens = app_name_.empty() ? "an app" : base::UTF16ToUTF8(app_name_);
    text = base::StrCat({"That link opens ", opens, " (", app->GetScheme(),
                         ":), outside the browser, and agents can't open apps. ",
                         "The tab stayed on “", base::UTF16ToUTF8(GetWebContents().GetTitle()),
                         "”. Ask the user, or use the page's web link if it has one."});
    error = true;
  }
  app_name_.clear();
  const uint64_t request = job_->request;
  job_.reset();
  frames_.clear();
  EndChoosing();
  wheel_.reset();
  wait_.Stop();
  loaded_.Reset();
  Unlisten();
  if (std::exchange(taken_, false)) {
    // The download it started answers it.
  } else if (download_starting_.IsRunning()) {
    // Its page didn't change, or the tab has none, as the navigation became a download.
    AnswerHeld();
    held_ = Held{request, std::move(text), error};
  } else {
    // Posted, so the app never hears inside its own bc_agent_page.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Report, tab_, request, std::move(text), error));
  }
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

void AgentPage::AnswerHeld() {
  if (std::optional<Held> held = std::exchange(held_, std::nullopt)) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE,
        base::BindOnce(&Report, tab_, held->request, std::move(held->text), held->error));
  }
}

void AgentPage::LetGo() {
  std::deque<Job> owed = std::exchange(jobs_, {});
  if (job_ && !std::exchange(taken_, false)) {
    owed.push_front(*std::move(job_));
  }
  job_.reset();
  download_starting_.Stop();
  AnswerHeld();
  for (const Job& job : owed) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Report, tab_, job.request,
                                  std::string("The agent's session with the tab ended."), true));
  }
  weak_factory_.InvalidateWeakPtrs();
  acted_ = false;
  pictures_ = 0;
  wheel_.reset();
  wait_.Stop();
  rest_.Stop();
  loaded_.Reset();
  EndChoosing();
  drawing_.RunAndReset();
  Detach();
  // With DevTools gone, its sessions are too, and what they heard.
  Unlisten();
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
  Send("Target.setAutoAttach", AutoAttaching(), base::DoNothing(), slot);
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
  // Events have no id. Of them, a frame's session starting or ending matters here, and while a
  // job listens, a request starting or ending.
  if (!id) {
    const std::string* method = dict ? dict->FindString("method") : nullptr;
    const base::DictValue* params = dict ? dict->FindDict("params") : nullptr;
    const std::string* session = params ? params->FindString("sessionId") : nullptr;
    if (method && session && *method == "Target.attachedToTarget") {
      Attached(*session, *params);
    } else if (method && session && *method == "Target.detachedFromTarget") {
      Detached(*session);
    } else if (method && params && listening_ && method->starts_with("Network.")) {
      const std::string* from = dict->FindString("sessionId");
      Heard(from ? *from : std::string(), *method, *params);
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
  // frames in other processes come in sessions of their own too, and while a job listens, so do
  // its requests.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](base::WeakPtr<AgentPage> page, int slot) {
                       if (!page) {
                         return;
                       }
                       page->AutoAttach(slot);
                       if (page->listening_) {
                         page->Send("Network.enable", Hearing(), base::DoNothing(), slot);
                       }
                     },
                     weak_factory_.GetWeakPtr(), static_cast<int>(target - targets_.begin())));
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
  if (loaded_) {
    wait_.Stop();
    std::move(loaded_).Run();
  } else if (settling_) {
    Quiet();
  }
}

void AgentPage::DidFinishNavigation(content::NavigationHandle* navigation) {
  // A new document's fields hold nothing the agent typed.
  if (navigation->IsInPrimaryMainFrame() && navigation->HasCommitted() &&
      !navigation->IsSameDocument()) {
    typed_.clear();
  }
  if (navigation->IsDownload() && job_) {
    download_starting_.Start(FROM_HERE, kDownloadStart,
                             base::BindOnce(&AgentPage::AnswerHeld, base::Unretained(this)));
  }
}

void AgentPage::WebContentsDestroyed() {
  // The tab is closing: what's asked is answered now, as the replies never will be.
  LetGo();
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(AgentPage);

}  // namespace browser_core
