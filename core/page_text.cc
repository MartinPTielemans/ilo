#include "browser_core/page_text.h"

#include <algorithm>
#include <string_view>
#include <utility>

#include "base/containers/flat_map.h"
#include "base/functional/bind.h"
#include "base/strings/string_util.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/agent_page.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "ui/accessibility/ax_enums.mojom.h"
#include "ui/accessibility/ax_mode.h"
#include "ui/accessibility/ax_node_data.h"
#include "ui/accessibility/ax_tree_update.h"

namespace browser_core {

namespace {

// Enough for a long article, and few enough that a huge page's snapshot stays quick. The renderer
// stops at either, giving what it has.
constexpr size_t kMaxNodes = 20000;
constexpr base::TimeDelta kSnapshotTimeout = base::Seconds(5);
constexpr base::TimeDelta kLoadTimeout = base::Seconds(15);

// The roles whose text stands on lines of its own, as the page lays them out.
bool IsBlock(ax::mojom::Role role) {
  switch (role) {
    case ax::mojom::Role::kArticle:
    case ax::mojom::Role::kBanner:
    case ax::mojom::Role::kBlockquote:
    case ax::mojom::Role::kCaption:
    case ax::mojom::Role::kContentInfo:
    case ax::mojom::Role::kDialog:
    case ax::mojom::Role::kFigcaption:
    case ax::mojom::Role::kFigure:
    case ax::mojom::Role::kFooter:
    case ax::mojom::Role::kForm:
    case ax::mojom::Role::kHeader:
    case ax::mojom::Role::kHeading:
    case ax::mojom::Role::kList:
    case ax::mojom::Role::kListItem:
    case ax::mojom::Role::kMain:
    case ax::mojom::Role::kNavigation:
    case ax::mojom::Role::kParagraph:
    case ax::mojom::Role::kRegion:
    case ax::mojom::Role::kRow:
    case ax::mojom::Role::kSection:
    case ax::mojom::Role::kTable:
      return true;
    default:
      return false;
  }
}

// A field the user types, picks or has filled in, whose value is theirs, as autofill's is: a text
// field or text area of any kind, an editable region such as a mail's body, a select, and a date,
// time, number or colour input. Its subtree too, where Chromium puts the text typed into it.
bool IsField(const ui::AXNodeData& node) {
  if (node.IsTextField() || node.HasState(ax::mojom::State::kEditable) ||
      node.HasState(ax::mojom::State::kRichlyEditable)) {
    return true;
  }
  switch (node.role) {
    case ax::mojom::Role::kColorWell:
    case ax::mojom::Role::kComboBoxSelect:
    case ax::mojom::Role::kDate:
    case ax::mojom::Role::kDateTime:
    case ax::mojom::Role::kInputTime:
    case ax::mojom::Role::kSpinButton:
      return true;
    default:
      return false;
  }
}

void NewLine(std::string& text) {
  while (!text.empty() && text.back() == ' ') {
    text.pop_back();
  }
  if (!text.empty() && text.back() != '\n') {
    text += '\n';
  }
}

// Pieces of a line are separate nodes, such as a link inside a sentence, which a space joins.
void Append(std::string& text, std::string_view piece) {
  piece = base::TrimWhitespaceASCII(piece, base::TRIM_ALL);
  if (piece.empty()) {
    return;
  }
  if (!text.empty() && text.back() != '\n' && text.back() != ' ') {
    text += ' ';
  }
  text += piece;
}

// Blink cuts a node's URL at this length, so a longer one is matched by what's left of it.
constexpr size_t kMaxUrlLength = 10000;

bool IsReadable(std::string_view url, const std::set<std::string, std::less<>>& readable) {
  if (url.size() < kMaxUrlLength) {
    return readable.contains(url);
  }
  return std::ranges::any_of(readable,
                             [url](const std::string& each) { return each.starts_with(url); });
}

// The URLs of the frames an agent may read: each one IsWebFrame admits, inside frames it admits, as
// agent_page.cc outlines them. A URL a frame it doesn't admit has too is left out, as the snapshot
// can't tell those frames apart.
std::set<std::string, std::less<>> ReadableUrls(content::WebContents& contents) {
  std::set<std::string, std::less<>> readable;
  std::set<std::string, std::less<>> refused;
  contents.GetPrimaryMainFrame()->ForEachRenderFrameHostWithAction(
      [&](content::RenderFrameHost* frame) {
        std::string url = frame->GetLastCommittedURL().spec();
        if (!IsWebFrame(frame)) {
          refused.insert(std::move(url));
          // Nor what's in it, whose own documents a refused one's subtree in the snapshot has.
          frame->ForEachRenderFrameHost([&refused](content::RenderFrameHost* inner) {
            refused.insert(inner->GetLastCommittedURL().spec());
          });
          return content::RenderFrameHost::FrameIterationAction::kSkipChildren;
        }
        readable.insert(std::move(url));
        return content::RenderFrameHost::FrameIterationAction::kContinue;
      });
  for (const std::string& url : refused) {
    readable.erase(url);
  }
  return readable;
}

void Report(bc_id tab, uint64_t request, std::optional<std::string> text) {
  if (Host().tab_text) {
    Host().tab_text(Host().context, tab, request, text ? text->c_str() : nullptr);
  }
}

}  // namespace

std::string TextOf(const ui::AXTreeUpdate& update,
                   const std::set<std::string, std::less<>>& readable) {
  if (update.nodes.empty()) {
    return std::string();
  }
  base::flat_map<ui::AXNodeID, const ui::AXNodeData*> nodes;
  for (const ui::AXNodeData& node : update.nodes) {
    nodes[node.id] = &node;
  }
  auto root = nodes.find(update.root_id);
  // A step back up out of a block ends its line. A stack of its own, as a page nests deeper than
  // the thread's.
  struct Step {
    const ui::AXNodeData* node;
    bool leaving;
  };
  std::vector<Step> steps = {{root != nodes.end() ? root->second : &update.nodes.front(), false}};
  std::string text;
  while (!steps.empty()) {
    const Step step = steps.back();
    steps.pop_back();
    const ui::AXNodeData& node = *step.node;
    if (step.leaving) {
      NewLine(text);
      continue;
    }
    // A frame's document an agent may not read, an extension's above all, with all that's in it.
    // Every document's root is a kRootWebArea, a frame's too.
    if (node.role == ax::mojom::Role::kRootWebArea &&
        !IsReadable(node.GetStringAttribute(ax::mojom::StringAttribute::kUrl), readable)) {
      continue;
    }
    // Hidden, or a field, a password field above all, whose value is the user's alone.
    if (node.IsInvisible() || node.HasState(ax::mojom::State::kProtected) || IsField(node)) {
      continue;
    }
    // An ignored node, such as a plain wrapper, has no text of its own, but its children may.
    const bool shown = !node.IsIgnored();
    if (shown && node.role == ax::mojom::Role::kStaticText) {
      Append(text, node.GetStringAttribute(ax::mojom::StringAttribute::kName));
      continue;
    }
    if (shown && node.role == ax::mojom::Role::kLineBreak) {
      NewLine(text);
      continue;
    }
    if (shown && IsBlock(node.role)) {
      NewLine(text);
      steps.push_back({&node, true});
    }
    for (auto child = node.child_ids.rbegin(); child != node.child_ids.rend(); ++child) {
      if (auto found = nodes.find(*child); found != nodes.end()) {
        steps.push_back({found->second, false});
      }
    }
  }
  return std::string(base::TrimWhitespaceASCII(text, base::TRIM_ALL));
}

PageText::PageText(content::WebContents* contents)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<PageText>(*contents),
      tab_(Tab::IdOf(contents)) {}

PageText::~PageText() = default;

void PageText::Read(uint64_t request) {
  // A tab without its page, or one opened behind and not yet shown, has nothing to read: it loads
  // once it's shown.
  if (!Tab::Loaded(tab_)) {
    Answer(request, std::nullopt);
    return;
  }
  waiting_.push_back(request);
  if (!GetWebContents().IsLoading()) {
    Snapshot();
  } else if (!give_up_.IsRunning()) {
    give_up_.Start(FROM_HERE, kLoadTimeout, this, &PageText::Snapshot);
  }
}

void PageText::Snapshot() {
  give_up_.Stop();
  for (uint64_t request : std::exchange(waiting_, {})) {
    asked_.insert(request);
    GetWebContents().RequestAXTreeSnapshot(
        base::BindOnce(&PageText::Snapshotted, weak_factory_.GetWeakPtr(), request),
        ui::kAXModeWebContentsOnly, kMaxNodes, kSnapshotTimeout,
        content::WebContents::AXTreeSnapshotPolicy::kAll);
  }
}

void PageText::Snapshotted(uint64_t request, ui::AXTreeUpdate& update) {
  asked_.erase(request);
  Answer(request, TextOf(update, ReadableUrls(GetWebContents())));
}

void PageText::Answer(uint64_t request, std::optional<std::string> text) {
  // Bound to the tab's id rather than this, which a closing tab deletes before the task runs.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Report, tab_, request, std::move(text)));
}

void PageText::DidStopLoading() {
  if (!waiting_.empty()) {
    Snapshot();
  }
}

void PageText::WebContentsDestroyed() {
  // The snapshots in the renderer never answer once the page is gone, so the app hears it here.
  give_up_.Stop();
  for (uint64_t request : std::exchange(waiting_, {})) {
    Answer(request, std::nullopt);
  }
  for (uint64_t request : std::exchange(asked_, {})) {
    Answer(request, std::nullopt);
  }
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(PageText);

}  // namespace browser_core
