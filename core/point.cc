#include "browser_core/point.h"

#include <algorithm>
#include <string_view>
#include <utility>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/logging.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/task/thread_pool.h"
#include "browser_core/agent_page.h"
#include "browser_core/arrange.h"
#include "browser_core/ax_text.h"
#include "browser_core/boosts.h"
#include "browser_core/host.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "content/public/browser/browser_context.h"
#include "content/public/browser/devtools_agent_host.h"
#include "content/public/browser/host_zoom_map.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/render_widget_host_view.h"
#include "content/public/browser/web_contents.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "third_party/blink/public/common/page/page_zoom.h"
#include "third_party/skia/include/core/SkBitmap.h"
#include "ui/gfx/codec/png_codec.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/rect_conversions.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

using Type = blink::WebInputEvent::Type;

// The label's name, as Arrange cuts it.
constexpr size_t kMaxName = 60;
// The element's text, enough for a section's, and no page's whole.
constexpr size_t kMaxText = 2000;
// The selector, enough for a deep path; a longer one is cut at its last step that fits.
constexpr size_t kMaxSelector = 400;
// The selector's steps up from the element, short of an id.
constexpr size_t kMaxSteps = 8;
// The picture holds the element's box and this many points around it.
constexpr float kAround = 8;

// |event|'s point in the page's view: an out-of-process frame's widget has it in its own.
gfx::PointF InPage(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event) {
  content::RenderWidgetHostView* view = widget->GetView();
  return view ? view->TransformPointToRootCoordSpaceF(event.PositionInWidget())
              : event.PositionInWidget();
}

// |text| cut to |size| characters, whole ones.
std::string Cut(std::string_view text, size_t size) {
  std::u16string wide = base::UTF8ToUTF16(text);
  if (wide.size() <= size) {
    return std::string(text);
  }
  wide.resize(size);
  // Never half a surrogate pair.
  if (!wide.empty() && (wide.back() & 0xFC00) == 0xD800) {
    wide.pop_back();
  }
  return base::UTF16ToUTF8(wide);
}

// A DevTools node's attribute, from its flat list of names and values.
std::string Attribute(const base::DictValue& node, std::string_view name) {
  const base::ListValue* attributes = node.FindList("attributes");
  for (size_t i = 0; attributes && i + 1 < attributes->size(); i += 2) {
    const std::string* each = (*attributes)[i].GetIfString();
    const std::string* value = (*attributes)[i + 1].GetIfString();
    if (each && value && *each == name) {
      return *value;
    }
  }
  return std::string();
}

std::string TagOf(const base::DictValue& node) {
  const std::string* name = node.FindString("nodeName");
  return name ? base::ToLowerASCII(*name) : std::string();
}

// Arrange's kind: its ARIA role, else its tag.
std::string KindOf(const base::DictValue& node) {
  std::string role = Attribute(node, "role");
  return role.empty() ? TagOf(node) : role;
}

// An id or role a selector can carry as it is: letters, digits, - and _ alone, so nothing in it is
// read as more selector.
bool Plain(std::string_view word) {
  return !word.empty() && !base::IsAsciiDigit(word.front()) &&
         std::ranges::all_of(
             word, [](char c) { return base::IsAsciiAlphaNumeric(c) || c == '-' || c == '_'; });
}

// The element with |backend| id in the DevTools document under |node|, and the elements it's in,
// the page's root first.
bool Find(const base::DictValue& node, int backend, std::vector<const base::DictValue*>& path) {
  path.push_back(&node);
  if (node.FindInt("backendNodeId") == backend) {
    return true;
  }
  if (const base::ListValue* children = node.FindList("children")) {
    for (const base::Value& child : *children) {
      if (child.is_dict() && Find(child.GetDict(), backend, path)) {
        return true;
      }
    }
  }
  path.pop_back();
  return false;
}

// A selector that finds the element at the end of |path| again, from its id, its role and its
// place among its parent's elements of its tag: words for the agent, never run.
std::string SelectorOf(const std::vector<const base::DictValue*>& path) {
  std::vector<std::string> steps;
  for (size_t i = path.size(); i-- > 0 && steps.size() < kMaxSteps;) {
    const base::DictValue& node = *path[i];
    if (node.FindInt("nodeType") != 1) {
      continue;
    }
    const std::string tag = TagOf(node);
    if (tag == "html") {
      break;
    }
    const std::string id = Attribute(node, "id");
    if (Plain(id)) {
      steps.push_back("#" + id);
      break;
    }
    std::string step = tag;
    const std::string role = Attribute(node, "role");
    if (Plain(role)) {
      step += "[role=\"" + role + "\"]";
    }
    if (i > 0) {
      int index = 0;
      int count = 0;
      if (const base::ListValue* siblings = path[i - 1]->FindList("children")) {
        for (const base::Value& sibling : *siblings) {
          if (!sibling.is_dict() || TagOf(sibling.GetDict()) != tag ||
              sibling.GetDict().FindInt("nodeType") != 1) {
            continue;
          }
          ++count;
          if (&sibling.GetDict() == &node) {
            index = count;
          }
        }
      }
      if (count > 1 && index > 0) {
        step += ":nth-of-type(" + base::NumberToString(index) + ")";
      }
    }
    steps.push_back(std::move(step));
  }
  std::string selector;
  for (auto step = steps.rbegin(); step != steps.rend(); ++step) {
    std::string next = selector.empty() ? *step : selector + " > " + *step;
    if (next.size() > kMaxSelector) {
      break;
    }
    selector = std::move(next);
  }
  return selector;
}

// A computed style's value by its name, from DevTools' list.
std::string StyleOf(const base::ListValue& styles, std::string_view name) {
  for (const base::Value& each : styles) {
    const base::DictValue* style = each.GetIfDict();
    const std::string* named = style ? style->FindString("name") : nullptr;
    const std::string* value = style ? style->FindString("value") : nullptr;
    if (named && value && *named == name) {
      return *value;
    }
  }
  return std::string();
}

// The page's address as the agent gets it: its origin and path, without the query or fragment
// that can carry a session's token, nor a username or password.
std::string AddressOf(const GURL& url) {
  GURL::Replacements without;
  without.ClearUsername();
  without.ClearPassword();
  without.ClearQuery();
  without.ClearRef();
  return url.ReplaceComponents(without).spec();
}

void Ended(bc_id tab) {
  if (Host().tab_point_ended) {
    Host().tab_point_ended(Host().context, tab);
  }
}

}  // namespace

struct PagePoint::Picked {
  int turn = 0;
  int node = 0;
  std::string kind;
  std::string name;
  std::string address;
  std::string title;
  std::string tag;
  std::string text;
  std::string selector;
  std::string color;
  std::string background;
  std::string font;
  std::string font_size;
  std::string padding;
  std::string frame;
  gfx::RectF box;
  bool withheld = false;
  std::vector<uint8_t> png;
  int width = 0;
  int height = 0;
};

PagePoint::PagePoint(content::WebContents* contents, bc_id tab)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<PagePoint>(*contents),
      tab_(tab) {}

PagePoint::~PagePoint() {
  Unhook();
  Detach();
}

// static
int PagePoint::May(content::WebContents& contents, std::string_view agent) {
  content::RenderFrameHost* page = contents.GetPrimaryMainFrame();
  Tab* tab = Tab::FromWebContents(&contents);
  // A page in the system's engine has no DevTools to point through.
  if (!page || Boosts::SiteOf(page->GetLastCommittedURL()).empty() || (tab && tab->system_page())) {
    return BC_POINT_NOT_WEB;
  }
  if (contents.GetBrowserContext()->IsOffTheRecord()) {
    return BC_POINT_PRIVATE;
  }
  // The asking agent's own Space is allowed: it already sees all of that page.
  Space* space = Space::Holding(Tab::IdOf(&contents));
  if (space && space->agent() && space->agent()->id != agent) {
    return BC_POINT_OTHER_AGENT;
  }
  if (const Arrange* arrange = Arrange::FromWebContents(&contents); arrange && arrange->on()) {
    return BC_POINT_ARRANGING;
  }
  return BC_POINT_CAN;
}

int PagePoint::Start(std::string agent) {
  const int may = May(*web_contents(), agent);
  if (may != BC_POINT_CAN) {
    return may;
  }
  if (on_ && agent == agent_) {
    return BC_POINT_CAN;
  }
  End(/*tell=*/false);
  // Zap takes the page's presses too.
  if (Boosts* boosts = Boosts::FromWebContents(web_contents()); boosts && boosts->zapping()) {
    boosts->EndZap(/*tell=*/true);
  }
  on_ = true;
  agent_ = std::move(agent);
  ++turn_;
  web_contents()->ForEachRenderFrameHost(
      [this](content::RenderFrameHost* frame) { Hook(frame->GetRenderWidgetHost()); });
  LOG(ERROR) << "browser: tab " << tab_ << " point on";
  return BC_POINT_CAN;
}

void PagePoint::End(bool tell) {
  if (!on_) {
    return;
  }
  on_ = false;
  agent_.clear();
  ++turn_;
  pressed_ = false;
  asking_ = false;
  waiting_.reset();
  chain_.clear();
  at_ = 0;
  shown_.reset();
  picking_ = false;
  Unhook();
  Detach();
  LOG(ERROR) << "browser: tab " << tab_ << " point off";
  if (tell) {
    // Posted, so the app never hears inside a call of its own that ended it.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE,
                                                                base::BindOnce(&Ended, tab_));
  }
}

void PagePoint::Do(int action) {
  // A step while a question is in the page waits for none: the next move asks again.
  if (!on_ || chain_.empty() || asking_) {
    return;
  }
  if (action == BC_POINT_DO_OUTER && at_ + 1 < chain_.size()) {
    ++at_;
  } else if (action == BC_POINT_DO_INNER && at_ > 0) {
    --at_;
  } else {
    return;
  }
  asking_ = true;
  Show();
}

void PagePoint::Hook(content::RenderWidgetHost* widget) {
  // A frame in its parent's process shares its widget.
  if (!widget || widgets_.IsObservingSource(widget)) {
    return;
  }
  widgets_.AddObservation(widget);
  auto callback =
      base::BindRepeating(&PagePoint::OnMouse, base::Unretained(this), base::Unretained(widget));
  widget->AddMouseEventCallback(callback);
  hooked_.emplace(widget, std::move(callback));
}

void PagePoint::Unhook() {
  for (auto& [widget, callback] : hooked_) {
    widget->RemoveMouseEventCallback(callback);
  }
  hooked_.clear();
  widgets_.RemoveAllObservations();
}

bool PagePoint::OnMouse(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event) {
  if (!on_) {
    return false;
  }
  const bool left = event.button == blink::WebPointerProperties::Button::kLeft;
  switch (event.GetType()) {
    case Type::kMouseMove:
      // Moves reach the page, so a hover menu opens as the user reaches for it; a drag after a
      // press Point took is Point's.
      Ask(InPage(widget, event));
      return pressed_;
    case Type::kMouseDown:
      if (!left) {
        return false;
      }
      // A left press and its release are Point's, so the page neither follows a link nor starts a
      // selection.
      pressed_ = true;
      Pick();
      return true;
    case Type::kMouseUp:
      if (!left || !pressed_) {
        return false;
      }
      pressed_ = false;
      return true;
    case Type::kMouseLeave:
      chain_.clear();
      at_ = 0;
      Hover(std::nullopt);
      return false;
    default:
      return false;
  }
}

void PagePoint::Ask(gfx::PointF point) {
  if (asking_) {
    waiting_ = point;
    return;
  }
  asking_ = true;
  // Read where the page has scrolled with each question, as the user may scroll while pointing.
  Send("Page.getLayoutMetrics", base::DictValue(),
       base::BindOnce(&PagePoint::HitTest, weak_factory_.GetWeakPtr(), point));
}

void PagePoint::HitTest(gfx::PointF point, const base::DictValue* metrics) {
  const base::DictValue* viewport = metrics ? metrics->FindDict("cssLayoutViewport") : nullptr;
  if (!viewport) {
    Asked();
    return;
  }
  // DevTools' hit test takes the page's CSS pixels from the top of the document.
  const double zoom = Zoom();
  const double x = point.x() / zoom + viewport->FindDouble("pageX").value_or(0);
  const double y = point.y() / zoom + viewport->FindDouble("pageY").value_or(0);
  Send("DOM.getNodeForLocation",
       base::DictValue()
           .Set("x", static_cast<int>(x))
           .Set("y", static_cast<int>(y))
           .Set("ignorePointerEventsNone", true),
       base::BindOnce(&PagePoint::Ancestors, weak_factory_.GetWeakPtr()));
}

void PagePoint::Ancestors(const base::DictValue* node) {
  std::optional<int> id = node ? node->FindInt("backendNodeId") : std::nullopt;
  const std::string* frame = node ? node->FindString("frameId") : nullptr;
  frame_ = frame ? *frame : std::string();
  if (!id) {
    chain_.clear();
    at_ = 0;
    Hover(std::nullopt);
    Asked();
    return;
  }
  // Still over the same element: only its box may have moved, as the page scrolled.
  if (!chain_.empty() && chain_.front().node == *id) {
    Show();
    return;
  }
  // The accessibility tree answers once it's on in this client, in order, before what's asked next.
  if (!accessible_) {
    accessible_ = true;
    Send("Accessibility.enable", base::DictValue(), base::DoNothing());
  }
  Send("Accessibility.getAXNodeAndAncestors", base::DictValue().Set("backendNodeId", *id),
       base::BindOnce(
           [](base::WeakPtr<PagePoint> point, int hit, const base::DictValue* result) {
             if (!point) {
               return;
             }
             // Without the accessibility tree, the hit element alone, unnamed.
             if (!result) {
               point->chain_ = {Link{hit, std::string()}};
               point->at_ = 0;
               point->Show();
               return;
             }
             point->Chain(result);
           },
           weak_factory_.GetWeakPtr(), *id));
}

void PagePoint::Chain(const base::DictValue* result) {
  chain_.clear();
  at_ = 0;
  // The hit node first, then each it's in up to the page's, as the tree has them; one the tree
  // ignores, such as a plain wrapper, is skipped, and the page's own root isn't pointed at.
  if (const base::ListValue* nodes = result->FindList("nodes")) {
    for (const base::Value& each : *nodes) {
      const base::DictValue* node = each.GetIfDict();
      std::optional<int> backend = node ? node->FindInt("backendDOMNodeId") : std::nullopt;
      const std::string* role = node ? node->FindStringByDottedPath("role.value") : nullptr;
      if (!backend || node->FindBool("ignored").value_or(false) ||
          (role && *role == "RootWebArea") ||
          std::ranges::any_of(chain_, [&](const Link& link) { return link.node == *backend; })) {
        continue;
      }
      const std::string* name = node->FindStringByDottedPath("name.value");
      chain_.push_back(Link{*backend, name ? Cut(*name, kMaxName) : std::string()});
    }
  }
  if (chain_.empty()) {
    Hover(std::nullopt);
    Asked();
    return;
  }
  Show();
}

void PagePoint::Show() {
  const Link link = chain_[at_];
  Send("DOM.describeNode", base::DictValue().Set("backendNodeId", link.node),
       base::BindOnce(&PagePoint::Described, weak_factory_.GetWeakPtr(), link));
}

void PagePoint::Described(Link link, const base::DictValue* result) {
  const base::DictValue* node = result ? result->FindDict("node") : nullptr;
  std::string kind = node ? KindOf(*node) : std::string();
  Send("DOM.getBoxModel", base::DictValue().Set("backendNodeId", link.node),
       base::BindOnce(&PagePoint::Boxed, weak_factory_.GetWeakPtr(), std::move(link),
                      std::move(kind)));
}

void PagePoint::Boxed(Link link, std::string kind, const base::DictValue* result) {
  const base::DictValue* model = result ? result->FindDict("model") : nullptr;
  const base::ListValue* quad = model ? model->FindList("border") : nullptr;
  if (!quad || quad->size() != 8) {
    Hover(std::nullopt);
    Asked();
    return;
  }
  // The border's quad, in the view's CSS pixels, is a rectangle unless the page transforms it, so
  // its bounds are what the outline goes around.
  double left = (*quad)[0].GetIfDouble().value_or(0), right = left;
  double top = (*quad)[1].GetIfDouble().value_or(0), bottom = top;
  for (size_t i = 2; i < 8; i += 2) {
    const double x = (*quad)[i].GetIfDouble().value_or(0);
    const double y = (*quad)[i + 1].GetIfDouble().value_or(0);
    left = std::min(left, x);
    right = std::max(right, x);
    top = std::min(top, y);
    bottom = std::max(bottom, y);
  }
  const double zoom = Zoom();
  Hover(Shown{link.node, std::move(kind), std::move(link.name),
              gfx::RectF(left * zoom, top * zoom, (right - left) * zoom, (bottom - top) * zoom)});
  Asked();
}

void PagePoint::Hover(std::optional<Shown> shown) {
  const bool changed = (shown ? shown->node : 0) != (shown_ ? shown_->node : 0);
  shown_ = std::move(shown);
  if (changed) {
    LOG(ERROR) << "browser: tab " << tab_ << " point over "
               << (shown_ ? (shown_->kind.empty() ? "element" : shown_->kind) : "nothing");
  }
  if (!Host().tab_point_hover) {
    return;
  }
  const bc_arrange_element element = {shown_ ? shown_->kind.c_str() : "",
                                      shown_ ? shown_->name.c_str() : ""};
  const gfx::RectF box = shown_ ? shown_->box : gfx::RectF();
  Host().tab_point_hover(Host().context, tab_, &element,
                         bc_rect{box.x(), box.y(), box.width(), box.height()});
}

void PagePoint::Asked() {
  asking_ = false;
  if (waiting_) {
    Ask(*std::exchange(waiting_, std::nullopt));
  }
}

void PagePoint::Pick() {
  // A click with nothing outlined, or while the last is still gathered, picks nothing.
  if (!shown_ || picking_) {
    return;
  }
  picking_ = true;
  Picked picked;
  picked.turn = turn_;
  picked.node = shown_->node;
  picked.frame = frame_;
  picked.kind = shown_->kind;
  picked.name = shown_->name;
  picked.box = shown_->box;
  content::WebContents* contents = web_contents();
  picked.address = AddressOf(contents->GetPrimaryMainFrame()->GetLastCommittedURL());
  picked.title = base::UTF16ToUTF8(contents->GetTitle());
  LOG(ERROR) << "browser: tab " << tab_ << " point picks "
             << (picked.kind.empty() ? "element" : picked.kind);
  // DOM and CSS on in this client, which reads the page and changes nothing in it; then the
  // document's tree, for the element's tag, the path the selector names, and the id CSS asks its
  // styles by. The page's own frame's alone: an element inside another frame goes without.
  Send("DOM.enable", base::DictValue(),
       base::BindOnce(
           [](base::WeakPtr<PagePoint> point, Picked picked, const base::DictValue*) {
             if (!point) {
               return;
             }
             point->Send(
                 "CSS.enable", base::DictValue(),
                 base::BindOnce(
                     [](base::WeakPtr<PagePoint> point, Picked picked, const base::DictValue*) {
                       if (!point) {
                         return;
                       }
                       point->Send("DOM.getDocument", base::DictValue().Set("depth", -1),
                                   base::BindOnce(&PagePoint::Document, point, std::move(picked)));
                     },
                     point, std::move(picked)));
           },
           weak_factory_.GetWeakPtr(), std::move(picked)));
}

void PagePoint::Document(Picked picked, const base::DictValue* result) {
  const base::DictValue* root = result ? result->FindDict("root") : nullptr;
  std::vector<const base::DictValue*> path;
  if (!root || !Find(*root, picked.node, path)) {
    Styled(std::move(picked), nullptr);
    return;
  }
  picked.tag = TagOf(*path.back());
  picked.selector = SelectorOf(path);
  const int node = path.back()->FindInt("nodeId").value_or(0);
  Send("CSS.getComputedStyleForNode", base::DictValue().Set("nodeId", node),
       base::BindOnce(&PagePoint::Styled, weak_factory_.GetWeakPtr(), std::move(picked)));
}

void PagePoint::Styled(Picked picked, const base::DictValue* result) {
  if (const base::ListValue* styles = result ? result->FindList("computedStyle") : nullptr) {
    picked.color = StyleOf(*styles, "color");
    picked.background = StyleOf(*styles, "background-color");
    picked.font = StyleOf(*styles, "font-family");
    picked.font_size = StyleOf(*styles, "font-size");
    // The computed style has each side's; as CSS writes them, one where all four are the same.
    const std::string sides[] = {StyleOf(*styles, "padding-top"), StyleOf(*styles, "padding-right"),
                                 StyleOf(*styles, "padding-bottom"),
                                 StyleOf(*styles, "padding-left")};
    picked.padding =
        std::ranges::all_of(sides, [&](const std::string& side) { return side == sides[0]; })
            ? sides[0]
            : base::JoinString(sides, " ");
  }
  // Its text from the tree the agent's outline reads, by the same rule, before DOM goes off.
  // Nothing of a frame an agent may not read.
  if (!Readable(picked.frame)) {
    Texted(std::move(picked), std::string(), true);
  } else {
    base::DictValue params;
    if (!picked.frame.empty()) {
      params.Set("frameId", picked.frame);
    }
    Send("Accessibility.getFullAXTree", std::move(params),
         base::BindOnce(
             [](base::WeakPtr<PagePoint> point, Picked picked, const base::DictValue* result) {
               if (!point) {
                 return;
               }
               const base::ListValue* nodes = result ? result->FindList("nodes") : nullptr;
               ElementText text = nodes ? TextUnder(*nodes, picked.node) : ElementText();
               point->Texted(std::move(picked), std::move(text.text), text.withheld);
             },
             weak_factory_.GetWeakPtr(), std::move(picked)));
  }
  // Off again, so the page's changes aren't followed while the user only points.
  Send("CSS.disable", base::DictValue(), base::DoNothing());
  Send("DOM.disable", base::DictValue(), base::DoNothing());
}

bool PagePoint::Readable(const std::string& frame) {
  content::RenderFrameHost* main = web_contents()->GetPrimaryMainFrame();
  content::RenderFrameHost* found = frame.empty() ? main : nullptr;
  main->ForEachRenderFrameHost([&](content::RenderFrameHost* each) {
    if (!found &&
        base::EqualsCaseInsensitiveASCII(each->GetDevToolsFrameToken().ToString(), frame)) {
      found = each;
    }
  });
  if (!found) {
    return false;
  }
  for (content::RenderFrameHost* up = found; up; up = up->GetParent()) {
    if (!IsWebFrame(up)) {
      return false;
    }
  }
  return true;
}

void PagePoint::Texted(Picked picked, std::string text, bool withheld) {
  if (!on_ || picked.turn != turn_) {
    return;
  }
  picked.text = Cut(text, kMaxText);
  picked.withheld = withheld;
  // The picture an agent's screenshot is, by the same function: the box and a little around it,
  // cut to the view. It hatches every field holding a value, as the text has none of them, where a
  // screenshot shows a value the page's own markup has, a read-only field's or the agent's typing.
  gfx::RectF area = picked.box;
  area.Outset(kAround);
  AgentPage::Picture(
      *web_contents(), gfx::ToEnclosingRect(area), AgentPage::Hatched::kEveryValue,
      base::BindOnce(&PagePoint::Drawn, weak_factory_.GetWeakPtr(), std::move(picked)));
}

void PagePoint::Drawn(Picked picked, base::expected<SkBitmap, std::string> picture) {
  if (!on_ || picked.turn != turn_) {
    return;
  }
  if (!picture.has_value() || picture->drawsNothing()) {
    Encoded(std::move(picked), std::nullopt);
    return;
  }
  picked.width = picture->width();
  picked.height = picture->height();
  // Encoded on a worker, as a big element is many pixels.
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::TaskPriority::USER_VISIBLE},
      base::BindOnce(
          [](SkBitmap bitmap) -> std::optional<std::vector<uint8_t>> {
            return gfx::PNGCodec::EncodeBGRASkBitmap(bitmap, /*discard_transparency=*/false);
          },
          std::move(picture).value()),
      base::BindOnce(&PagePoint::Encoded, weak_factory_.GetWeakPtr(), std::move(picked)));
}

void PagePoint::Encoded(Picked picked, std::optional<std::vector<uint8_t>> png) {
  picking_ = false;
  if (!on_ || picked.turn != turn_) {
    return;
  }
  if (png) {
    picked.png = std::move(*png);
  } else {
    picked.width = 0;
    picked.height = 0;
  }
  Report(picked);
}

void PagePoint::Report(const Picked& picked) {
  LOG(ERROR) << "browser: tab " << tab_ << " point picked "
             << (picked.kind.empty() ? "element" : picked.kind)
             << (picked.withheld ? " withheld" : "") << (picked.png.empty() ? " no picture" : "");
  if (!Host().tab_point_picked) {
    return;
  }
  bc_point_element element = {};
  element.element = {picked.kind.c_str(), picked.name.c_str()};
  element.address = picked.address.c_str();
  element.title = picked.title.c_str();
  element.tag = picked.tag.c_str();
  element.text = picked.text.c_str();
  element.selector = picked.selector.c_str();
  element.color = picked.color.c_str();
  element.background = picked.background.c_str();
  element.font = picked.font.c_str();
  element.font_size = picked.font_size.c_str();
  element.padding = picked.padding.c_str();
  element.box = {picked.box.x(), picked.box.y(), picked.box.width(), picked.box.height()};
  element.withheld = picked.withheld;
  element.png = picked.png.empty() ? nullptr : picked.png.data();
  element.size = picked.png.size();
  element.width = picked.width;
  element.height = picked.height;
  Host().tab_point_picked(Host().context, tab_, &element);
}

void PagePoint::Send(const char* method, base::DictValue params, Reply reply) {
  if (!agent_host_) {
    scoped_refptr<content::DevToolsAgentHost> host =
        content::DevToolsAgentHost::GetOrCreateFor(web_contents());
    // The profile's policy may keep DevTools from the page, which leaves Point outlining nothing.
    if (!host || !host->AttachClient(this)) {
      std::move(reply).Run(nullptr);
      return;
    }
    agent_host_ = std::move(host);
  }
  const int id = next_command_++;
  std::optional<std::string> json = base::WriteJson(
      base::DictValue().Set("id", id).Set("method", method).Set("params", std::move(params)));
  if (!json) {
    std::move(reply).Run(nullptr);
    return;
  }
  replies_[id] = std::move(reply);
  agent_host_->DispatchProtocolMessage(this, base::as_byte_span(*json));
}

void PagePoint::Detach() {
  // Answers still owed go unanswered: Point ended, or the tab is closing.
  replies_.clear();
  accessible_ = false;
  if (scoped_refptr<content::DevToolsAgentHost> host = std::move(agent_host_)) {
    host->DetachClient(this);
  }
}

double PagePoint::Zoom() {
  return blink::ZoomLevelToZoomFactor(content::HostZoomMap::GetZoomLevel(web_contents()));
}

void PagePoint::RenderFrameCreated(content::RenderFrameHost* frame) {
  if (on_) {
    Hook(frame->GetRenderWidgetHost());
  }
}

void PagePoint::PrimaryPageChanged(content::Page& page) {
  // Point was the last page's.
  End(/*tell=*/true);
}

void PagePoint::WebContentsDestroyed() {
  End(/*tell=*/true);
}

void PagePoint::RenderWidgetHostDestroyed(content::RenderWidgetHost* widget) {
  hooked_.erase(widget);
  widgets_.RemoveObservation(widget);
}

void PagePoint::DispatchProtocolMessage(content::DevToolsAgentHost* agent_host,
                                        base::span<const uint8_t> message) {
  std::optional<base::DictValue> dict = base::JSONReader::ReadDict(
      std::string_view(reinterpret_cast<const char*>(message.data()), message.size()),
      base::JSON_PARSE_RFC);
  std::optional<int> id = dict ? dict->FindInt("id") : std::nullopt;
  // Events, which the client doesn't ask for, have no id.
  if (!id) {
    return;
  }
  auto reply = replies_.find(*id);
  if (reply == replies_.end()) {
    return;
  }
  Reply run = std::move(reply->second);
  replies_.erase(reply);
  std::move(run).Run(dict->FindDict("result"));
}

void PagePoint::AgentHostClosed(content::DevToolsAgentHost* agent_host) {
  agent_host_ = nullptr;
  accessible_ = false;
  // The page's renderer went: what was asked is answered as nothing, so nothing waits on it.
  base::flat_map<int, Reply> replies = std::move(replies_);
  replies_.clear();
  for (auto& [id, reply] : replies) {
    std::move(reply).Run(nullptr);
  }
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(PagePoint);

}  // namespace browser_core
