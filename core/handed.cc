#include "browser_core/handed.h"

#include <utility>

#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/agent_session.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "components/sync/base/unique_position.h"
#include "content/public/browser/history_traversal.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_entry.h"
#include "content/public/browser/web_contents.h"

namespace browser_core::handed {

namespace {

std::map<bc_id, Mark>& Marks() {
  static base::NoDestructor<std::map<bc_id, Mark>> marks;
  return *marks;
}

// The place |row| has among |home|'s rows: its own there, or the one it kept if it's a handed tab
// that left |home|. Null if it has none.
const syncer::UniquePosition* PlaceOf(const Space& home, bc_id row) {
  if (const Space::Node* node = home.NodeOf(row)) {
    return &node->position;
  }
  const Mark* mark = Of(row);
  return mark && mark->home == home.id() ? &mark->node.position : nullptr;
}

// What the page of |tab| asks before it's taken to the entry at |index| of its history, by a page
// (|from_page|) or by the browser.
bool MayTraverse(bc_id tab, int index, bool from_page) {
  content::WebContents* contents = Tab::FromId(tab);
  content::NavigationEntry* entry =
      contents ? contents->GetController().GetEntryAtIndex(index) : nullptr;
  return !entry || MayGoTo(tab, entry->GetUniqueID(), from_page);
}

}  // namespace

Mark Leaving(const Space& from, bc_id tab) {
  const Space::Node* node = from.NodeOf(tab);
  std::vector<bc_id> split = from.Panes(tab);
  if (split.size() < 2) {
    split.clear();
  }
  return {.home = from.id(),
          .node = node ? *node : Space::Node(),
          .split = std::move(split),
          .pin = handed_back::PinOf(from, tab)};
}

void Given(bc_id tab, Mark left) {
  Marks().insert_or_assign(tab, std::move(left));
  BoundHistory(tab);
}

const Mark* Of(bc_id tab) {
  auto it = Marks().find(tab);
  return it == Marks().end() ? nullptr : &it->second;
}

const std::map<bc_id, Mark>& All() {
  return Marks();
}

void Left(bc_id tab) {
  if (!Marks().erase(tab)) {
    return;
  }
  // Its history is all the user's again, and they go through it as they like.
  if (content::WebContents* contents = Tab::FromId(tab)) {
    content::SetHistoryTraversalFilter(contents, {});
  }
}

bool Return(bc_id tab) {
  const Mark* kept = Of(tab);
  Space* in = Space::Holding(tab);
  if (!kept || !in || !in->agent()) {
    return false;
  }
  // The move takes the mark off.
  const Mark mark = *kept;
  Space* home = Space::FromId(mark.home);
  // The Space it left may be gone, or in another profile now, where the page couldn't go as it is.
  if (!home || home->agent() || home->own() || home->profile() != in->profile()) {
    return false;
  }
  // An agent may pin a tab in its own Space; the user's goes back as it left, with the pin it had
  // or none.
  if (in->IsPinned(tab)) {
    in->Unpin(tab);
  }
  Window::MoveToSpace(tab, mark.home);
  if (Space::Holding(tab) != home) {
    return false;
  }
  handed_back::Put(*home, tab, mark.node, mark.split, mark.pin);
  if (!mark.split.empty()) {
    Window::PanesChanged();
  }
  // The user's again, as a tab moved out of an agent's Space is (bc_tab_move_to_space).
  FillForUser(tab);
  return true;
}

void BoundHistory(bc_id tab) {
  auto mark = Marks().find(tab);
  content::WebContents* contents = Tab::FromId(tab);
  if (mark == Marks().end() || !contents) {
    return;
  }
  content::NavigationController& controller = contents->GetController();
  std::vector<int> entries;
  entries.reserve(controller.GetEntryCount());
  for (int i = 0; i < controller.GetEntryCount(); ++i) {
    entries.push_back(controller.GetEntryAtIndex(i)->GetUniqueID());
  }
  // No entry has the id 0, so a page on none yet keeps all it has for the user.
  const content::NavigationEntry* on = controller.GetLastCommittedEntry();
  mark->second.history.Set(entries, on ? on->GetUniqueID() : 0);
  // By the tab's id, so it holds nothing that could be gone when it's asked.
  content::SetHistoryTraversalFilter(contents, base::BindRepeating(&MayTraverse, tab));
}

bool MayGoTo(bc_id tab, int entry, bool from_page) {
  const Mark* mark = Of(tab);
  return !mark || mark->history.MayGoTo(entry, IsHeld(tab), from_page);
}

bool AgentMayGoTo(bc_id tab, int entry) {
  const Mark* mark = Of(tab);
  return !mark || mark->history.MayGoTo(entry, /*held=*/false, /*from_page=*/false);
}

bool MayStep(bc_id tab, int offset) {
  if (!Of(tab)) {
    return true;
  }
  content::WebContents* contents = Tab::FromId(tab);
  if (!contents) {
    return IsHeld(tab);
  }
  const content::NavigationEntry* entry = contents->GetController().GetEntryAtOffset(offset);
  return entry && MayGoTo(tab, entry->GetUniqueID(), /*from_page=*/false);
}

size_t PlaceAmong(const Space& home, const std::vector<bc_id>& rows, const Space::Node& node) {
  return handed_back::Among(home, rows, node, [&](bc_id row) { return PlaceOf(home, row); });
}

}  // namespace browser_core::handed
