#include "browser_core/tile.h"

#include <cstdio>
#include <map>
#include <memory>
#include <string>
#include <utility>

#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/agent_session.h"
#include "browser_core/checked.h"
#include "browser_core/platform/window_views.h"
#include "browser_core/tab.h"
#include "browser_core/tile_rule.h"
#include "browser_core/window.h"
#include "chrome/browser/ui/tabs/public/tab_dialog_manager.h"
#include "content/public/browser/web_contents.h"
#include "ui/views/widget/widget.h"
#include "ui/views/widget/widget_delegate.h"
#include "ui/views/widget/widget_observer.h"

namespace browser_core::tiles {

namespace {

TileRule& Rule() {
  static base::NoDestructor<TileRule> rule;
  return *rule;
}

// Keeps the dialog of a tile's tab, such as Chrome's warning before http, off the screen and
// from taking the keyboard (TileRule::HoldsDialog). Chrome shows it again as its page becomes
// visible, so each time it does, it's hidden again. The dialog may close while it's held, so
// it's held weakly.
class DialogHold : public views::WidgetObserver {
 public:
  explicit DialogHold(bc_id tab) : tab_(tab) { Take(); }
  DialogHold(const DialogHold&) = delete;
  DialogHold& operator=(const DialogHold&) = delete;
  ~DialogHold() override { Let(/*show=*/false); }

  // Takes the dialog the tab has now, if this holds no other.
  void Take() {
    if (dialog_ && !dialog_->IsClosed()) {
      return;
    }
    views::Widget* dialog = Window::TabDialog(Tab::FromId(tab_));
    if (!dialog || dialog->IsClosed() || !dialog->widget_delegate()) {
      return;
    }
    observation_.Reset();
    could_ = dialog->widget_delegate()->CanActivate();
    // A dialog that can't be activated shows without the keyboard, for the moment it shows.
    dialog->widget_delegate()->SetCanActivate(false);
    dialog_ = dialog->GetWeakPtr();
    observation_.Observe(dialog);
    dialog->Hide();
  }

  // Lets the dialog go: where |show|, Chrome shows it again over the page, without the keyboard,
  // as it does as a page becomes visible, which it has been all along in the tile.
  void Let(bool show) {
    observation_.Reset();
    views::Widget* dialog = dialog_.get();
    dialog_.reset();
    if (!dialog || dialog->IsClosed() || !dialog->widget_delegate()) {
      return;
    }
    tabs::TabDialogManager* dialogs = show ? Window::DialogsOf(Tab::FromId(tab_)) : nullptr;
    if (dialogs && dialogs->UpdateDialogVisibility()) {
      dialogs->UpdateModalDialogBounds();
    }
    dialog->widget_delegate()->SetCanActivate(could_);
  }

  // views::WidgetObserver:
  void OnWidgetVisibilityChanged(views::Widget* widget, bool visible) override {
    if (visible) {
      widget->Hide();
    }
  }
  void OnWidgetDestroying(views::Widget* widget) override {
    observation_.Reset();
    dialog_.reset();
  }

 private:
  const bc_id tab_;
  base::WeakPtr<views::Widget> dialog_;
  bool could_ = true;
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
};

std::map<bc_id, std::unique_ptr<DialogHold>>& DialogHolds() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<DialogHold>>> holds;
  return *holds;
}

// |tab|'s page left its tile for a window or a card, which shows its dialog again where |show|,
// or is going.
void LetDialog(bc_id tab, bool show) {
  auto hold = DialogHolds().extract(tab);
  if (!hold.empty()) {
    hold.mapped()->Let(show);
  }
}

// What each tile's line last said (SayTile).
std::map<bc_id, std::string>& Said() {
  static base::NoDestructor<std::map<bc_id, std::string>> said;
  return *said;
}

// For the checks: whether |tab|'s page draws at its tile's size and its window's scale (#4441).
// Once a change, as a tile's motion changes it each frame.
void SayTile(bc_id tab, content::WebContents* contents) {
  std::string now = platform::DescribeTile(contents);
  if (now.empty() || Said()[tab] == now) {
    return;
  }
  fprintf(stderr, "browser: tile %llu %s\n", static_cast<unsigned long long>(tab), now.c_str());
  Said()[tab] = std::move(now);
}

void LogEnded(bc_id tab) {
  Said().erase(tab);
  fprintf(stderr, "browser: tile %llu ended\n", static_cast<unsigned long long>(tab));
}

// The page of |tab|, which was a tile in |view| or |parked|, leaves both: out of its window as well
// where it |leaves_window|. Returns whether it was either.
bool Out(bc_id tab, const void* view, bool parked, bool leaves_window) {
  if (!view && !parked) {
    return false;
  }
  if (view) {
    LogEnded(tab);
  }
  if (content::WebContents* contents = Tab::FromId(tab)) {
    platform::EndTile(contents, leaves_window);
  }
  return true;
}

}  // namespace

bool Show(bc_id tab, void* view, const gfx::Rect& frame, double corner_radius, bool takes_wheel) {
  content::WebContents* contents = Tab::FromId(tab);
  if (!contents) {
    return false;
  }
  const TileRule::Shown shown = Rule().Show(tab, view, IsAgentTab(contents),
                                            Window::Showing(tab), platform::CanTile());
  // Not for each frame the app moves it by.
  if (shown != TileRule::Shown::kFramed) {
    fprintf(stderr, "browser: tile %llu %s\n", static_cast<unsigned long long>(tab),
            TileRule::Name(shown));
  }
  switch (shown) {
    case TileRule::Shown::kNew:
    case TileRule::Shown::kFramed:
    case TileRule::Shown::kMoved:
      // Held before the page becomes visible in the tile, which is when Chrome shows the dialog.
      if (Rule().HoldsDialog(tab, IsAgentTab(contents), IsHeld(tab)) &&
          !DialogHolds().contains(tab)) {
        DialogHolds().emplace(tab, std::make_unique<DialogHold>(tab));
      }
      Rule().SetTakesWheel(tab, takes_wheel);
      platform::ShowTile(contents, view, frame, corner_radius, Rule().TakesWheel(tab));
      // Into a tile, the page takes the size a watch gives it, whatever size the place it came from
      // gave it, such as a window's pages', so the tile shows it at its own size (#4441).
      if (shown != TileRule::Shown::kFramed) {
        Window::GiveAgentPageItsSize(contents);
      }
      if (Checked()) {
        SayTile(tab, contents);
      }
      // A tile made for a page that died draws nothing, as one whose page dies in it: one a window
      // let go of, handed back with no reload. Not for each frame the app moves it by.
      if (shown != TileRule::Shown::kFramed) {
        const Tab* shows = Tab::Get(tab);
        platform::HideTile(contents, shows && shows->crashed());
      }
      return true;
    default:
      return false;
  }
}

void End(bc_id tab) {
  const bool parked = Rule().Parked(tab);
  const bool was_tile = Out(tab, Rule().Left(tab), parked, /*leaves_window=*/true);
  // A page that died in its tile or the park stayed crashed there, and no tile has it now.
  if (Tab* left = was_tile ? Tab::Get(tab) : nullptr) {
    left->LeftSight();
  }
  LetDialog(tab, /*show=*/false);
}

void TakenByWindow(bc_id tab) {
  const bool parked = Rule().Parked(tab);
  Out(tab, Rule().Left(tab), parked, /*leaves_window=*/false);
  LetDialog(tab, /*show=*/true);
}

void EndTile(bc_id tab) {
  if (!Rule().End(tab)) {
    return;
  }
  LogEnded(tab);
  if (content::WebContents* contents = Tab::FromId(tab)) {
    platform::ParkTile(contents);
  }
  if (Tab* left = Tab::Get(tab)) {
    left->LeftSight();
  }
}

void ShowInCard(bc_id tab) {
  const bool parked = Rule().Parked(tab);
  Out(tab, Rule().ShowInCard(tab), parked, /*leaves_window=*/true);
  LetDialog(tab, /*show=*/true);
  if (Tab* left = Tab::Get(tab)) {
    left->LeftSight();
  }
}

void PageViewChanged(bc_id tab) {
  if (!Rule().InTile(tab)) {
    return;
  }
  if (content::WebContents* contents = Tab::FromId(tab)) {
    platform::TilePageChanged(contents);
  }
}

void DialogComing() {
  for (auto& [tab, hold] : DialogHolds()) {
    hold->Take();
  }
}

bool Holds(bc_id tab) {
  return Rule().InTile(tab);
}

bool KeepsCrashed(bc_id tab) {
  return Rule().KeepsCrashed(tab, Window::Showing(tab) != nullptr);
}

void Died(bc_id tab, bool died) {
  if (!Rule().InTile(tab)) {
    return;
  }
  if (content::WebContents* contents = Tab::FromId(tab)) {
    platform::HideTile(contents, died);
  }
}

}  // namespace browser_core::tiles
