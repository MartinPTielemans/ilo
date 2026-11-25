#include "browser_core/handed.h"

#include <algorithm>
#include <utility>

#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/agent_session.h"
#include "browser_core/handed_place.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "components/sync/base/unique_position.h"
#include "content/public/browser/history_traversal.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_entry.h"
#include "content/public/browser/web_contents.h"
#include "url/gurl.h"

namespace browser_core::handed {

namespace {

std::map<bc_id, Mark>& Marks() {
  static base::NoDestructor<std::map<bc_id, Mark>> marks;
  return *marks;
}

// The place |row| has among |home|'s Today tabs: its own there, or the one it kept if it's a
// handed tab that left |home|. Null if it has none.
const syncer::UniquePosition* PlaceOf(const Space& home, bc_id row) {
  if (const Space::Node* node = home.NodeOf(row)) {
    return &node->position;
  }
  const Mark* mark = Of(row);
  return mark && mark->home == home.id() ? &mark->node.position : nullptr;
}

// What the page of |tab| asks before it's taken to the entry at |index| of its history.
bool MayTraverse(bc_id tab, int index) {
  content::WebContents* contents = Tab::FromId(tab);
  content::NavigationEntry* entry =
      contents ? contents->GetController().GetEntryAtIndex(index) : nullptr;
  return !entry || MayGoTo(tab, entry->GetUniqueID());
}

}  // namespace

void Given(bc_id tab, bc_id home, Space::Node node) {
  Marks().insert_or_assign(tab, Mark{.home = home, .node = std::move(node)});
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
  // An agent may pin a tab in its own Space; the user's was a Today tab.
  if (in->IsPinned(tab)) {
    in->Unpin(tab);
  }
  Window::MoveToSpace(tab, mark.home);
  if (Space::Holding(tab) != home) {
    return false;
  }
  std::vector<bc_id> others = home->today();
  std::erase(others, tab);
  home->Move(tab, /*pinned=*/false, 0, PlaceAmong(*home, others, mark.node),
             [](bc_id) { return GURL(); });
  // Its node as it was, so the account's other devices see no move.
  if (mark.node.id.is_valid()) {
    home->RestoreNodes(home->node(), {{tab, mark.node}});
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

bool MayGoTo(bc_id tab, int entry) {
  const Mark* mark = Of(tab);
  return !mark || mark->history.MayGoTo(entry, IsHeld(tab));
}

size_t PlaceAmong(const Space& home, const std::vector<bc_id>& rows, const Space::Node& node) {
  std::vector<handed_place::Row> today;
  today.reserve(rows.size());
  for (size_t i = 0; i < rows.size(); ++i) {
    const syncer::UniquePosition* place = PlaceOf(home, rows[i]);
    today.push_back({
        .before =
            place && place->IsValid() && node.position.IsValid() && place->LessThan(node.position),
        .joined_above = i > 0 && std::ranges::contains(home.Panes(rows[i]), rows[i - 1]),
    });
  }
  return handed_place::Among(today);
}

}  // namespace browser_core::handed
