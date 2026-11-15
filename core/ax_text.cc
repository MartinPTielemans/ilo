#include "browser_core/ax_text.h"

#include <algorithm>
#include <iterator>
#include <map>
#include <string_view>
#include <vector>

#include "base/containers/adapters.h"
#include "base/containers/flat_map.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "ui/accessibility/ax_enums.mojom.h"
#include "ui/accessibility/ax_node_data.h"
#include "ui/accessibility/ax_tree_update.h"

namespace browser_core {

namespace {

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

bool FromExtension(const ui::AXNodeData& node) {
  return std::ranges::any_of(node.html_attributes,
                             [](const auto& each) { return each.first == "fromExtension"; });
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

// IsBlock's roles as DevTools names them, the landmark ones by their ARIA names.
bool IsBlock(std::string_view role) {
  static constexpr std::string_view kBlocks[] = {
      "article", "banner", "blockquote", "caption", "contentinfo", "dialog", "figcaption",
      "figure", "footer", "footerasnonlandmark", "form", "header", "headerasnonlandmark",
      "heading", "list", "listitem", "main", "navigation", "paragraph", "region", "row",
      "section", "table"};
  const std::string lower = base::ToLowerASCII(role);
  return std::ranges::find(kBlocks, lower) != std::end(kBlocks);
}

// Whether DevTools' tree has the node as not drawn, as the snapshot's IsInvisible: display: none,
// or visibility: hidden.
bool NotDrawn(const base::DictValue& node) {
  if (const base::ListValue* reasons = node.FindList("ignoredReasons")) {
    for (const base::Value& reason : *reasons) {
      const base::DictValue* dict = reason.GetIfDict();
      const std::string* name = dict ? dict->FindString("name") : nullptr;
      if (name && (*name == "notRendered" || *name == "notVisible")) {
        return true;
      }
    }
  }
  return false;
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

using Nodes = base::flat_map<ui::AXNodeID, const ui::AXNodeData*>;

Nodes NodesOf(const ui::AXTreeUpdate& update) {
  Nodes nodes;
  for (const ui::AXNodeData& node : update.nodes) {
    nodes[node.id] = &node;
  }
  return nodes;
}

// The text from |start| down, and in |withheld| whether it left out a field holding a value or a
// frame an agent may not read.
std::string Walk(const Nodes& nodes,
                 const ui::AXNodeData& start,
                 const std::set<std::string, std::less<>>& readable,
                 bool& withheld) {
  // A step back up out of a block ends its line. A stack of its own, as a page nests deeper than
  // the thread's.
  struct Step {
    const ui::AXNodeData* node;
    bool leaving;
  };
  std::vector<Step> steps = {{&start, false}};
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
      withheld = true;
      continue;
    }
    // Hidden, or a field, a password field above all, whose value is the user's alone. Or what an
    // extension's content script made, which is the extension's, not the page's
    // (engine/patches/ours/chromium-agent-extension-nodes.patch marks it, and each node inside it).
    if (node.IsInvisible() || node.HasState(ax::mojom::State::kProtected) || IsField(node) ||
        FromExtension(node)) {
      if ((node.HasState(ax::mojom::State::kProtected) || IsField(node)) &&
          !node.GetStringAttribute(ax::mojom::StringAttribute::kValue).empty()) {
        withheld = true;
      }
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

}  // namespace

std::string TextOf(const ui::AXTreeUpdate& update,
                   const std::set<std::string, std::less<>>& readable) {
  if (update.nodes.empty()) {
    return std::string();
  }
  const Nodes nodes = NodesOf(update);
  auto root = nodes.find(update.root_id);
  bool withheld = false;
  return Walk(nodes, root != nodes.end() ? *root->second : update.nodes.front(), readable,
              withheld);
}

ElementText TextUnder(const base::ListValue& nodes, int node) {
  std::map<std::string, const base::DictValue*, std::less<>> by_id;
  const base::DictValue* start = nullptr;
  for (const base::Value& each : nodes) {
    const base::DictValue* dict = each.GetIfDict();
    const std::string* id = dict ? dict->FindString("nodeId") : nullptr;
    if (!id) {
      continue;
    }
    by_id[*id] = dict;
    if (!start && dict->FindInt("backendDOMNodeId") == node) {
      start = dict;
    }
  }
  ElementText text;
  if (!start) {
    return text;
  }
  // As Walk goes: a stack of its own, and a step back up out of a block ends its line. Each node
  // once, whatever the tree says.
  struct Step {
    const base::DictValue* node;
    bool leaving;
  };
  std::vector<Step> steps = {{start, false}};
  std::set<const base::DictValue*> seen;
  std::string out;
  while (!steps.empty()) {
    const Step step = steps.back();
    steps.pop_back();
    if (step.leaving) {
      NewLine(out);
      continue;
    }
    const base::DictValue& each = *step.node;
    if (!seen.insert(&each).second || NotDrawn(each) || Has(each, "fromExtension")) {
      continue;
    }
    // A field's value is the user's, and what's under it is that value again: none goes, as none
    // goes into the outline (#2372), a filled email or card number as much as a password.
    if (IsField(each)) {
      const std::optional<std::string> value = ValueOf(each);
      text.withheld |= value && !value->empty();
      continue;
    }
    const std::string* role = each.FindStringByDottedPath("role.value");
    const std::string_view kind = role ? std::string_view(*role) : std::string_view();
    const bool shown = !each.FindBool("ignored").value_or(false);
    if (shown && kind == "StaticText") {
      const std::string* name = each.FindStringByDottedPath("name.value");
      Append(out, name ? *name : std::string());
      continue;
    }
    if (shown && kind == "LineBreak") {
      NewLine(out);
      continue;
    }
    if (shown && IsBlock(kind)) {
      NewLine(out);
      steps.push_back({&each, true});
    }
    if (const base::ListValue* children = each.FindList("childIds")) {
      for (const base::Value& child : base::Reversed(*children)) {
        const std::string* id = child.GetIfString();
        if (auto found = id ? by_id.find(*id) : by_id.end(); found != by_id.end()) {
          steps.push_back({found->second, false});
        }
      }
    }
  }
  text.text = std::string(base::TrimWhitespaceASCII(out, base::TRIM_ALL));
  return text;
}

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

}  // namespace browser_core
