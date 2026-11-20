#include "browser_core/overlay_lend.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using Goes = OverlayLend::Goes;

constexpr uint64_t kTab = 1;
constexpr uint64_t kBoard = 2;

bool Open(uint64_t) {
  return true;
}

// Who has the keyboard as the window does: the UI's ask takes it at once, and the let-go moves it
// as the lend says.
enum class Keyboard { kOverlay, kUI, kPage };

struct Window {
  void Take() {
    lend.Take(selected);
    keyboard = Keyboard::kOverlay;
  }
  void Ask() {
    lend.UIAsked();
    keyboard = Keyboard::kUI;
  }
  void LetGo() {
    switch (lend.LetGo(selected, /*page_shown=*/true, Open)) {
      case Goes::kToPage:
        keyboard = Keyboard::kPage;
        break;
      case Goes::kToUI:
        keyboard = Keyboard::kUI;
        break;
      case Goes::kBack:
        keyboard = lent_by;
        break;
    }
  }

  OverlayLend lend;
  uint64_t selected = kTab;
  Keyboard lent_by = Keyboard::kPage;
  Keyboard keyboard = Keyboard::kPage;
};

TEST(OverlayLend, TheUIKeepsWhatItAskedForAsTheLetGoComesAfter) {
  // New Board from the + menu under Wayland: the Board's tab shows and its name field asks, and the
  // menu's let-go comes a compositor roundtrip later (#3572).
  Window window;
  window.Take();
  window.selected = kBoard;
  window.Ask();
  window.LetGo();
  EXPECT_EQ(window.keyboard, Keyboard::kUI);
}

TEST(OverlayLend, TheUIKeepsWhatItAskedForAsTheLetGoComesFirst) {
  // The same on X11, whose hide lets go at once.
  Window window;
  window.Take();
  window.selected = kBoard;
  window.LetGo();
  window.Ask();
  EXPECT_EQ(window.keyboard, Keyboard::kUI);
}

TEST(OverlayLend, AnotherTabShownGivesItsPageTheKeyboard) {
  // The command bar going to a page, from the UI.
  Window window;
  window.lent_by = Keyboard::kUI;
  window.Take();
  window.selected = kBoard;
  window.LetGo();
  EXPECT_EQ(window.keyboard, Keyboard::kPage);
}

TEST(OverlayLend, TheSameTabGivesItBack) {
  Window window;
  window.lent_by = Keyboard::kUI;
  window.Take();
  window.LetGo();
  EXPECT_EQ(window.keyboard, Keyboard::kUI);
}

TEST(OverlayLend, AnAskBeforeTheLendDoesntCount) {
  Window window;
  window.Ask();
  window.Take();
  window.selected = kBoard;
  window.LetGo();
  EXPECT_EQ(window.keyboard, Keyboard::kPage);
}

TEST(OverlayLend, AnAskCountsForItsLendAlone) {
  Window window;
  window.Take();
  window.Ask();
  window.LetGo();
  window.Take();
  window.selected = kBoard;
  window.LetGo();
  EXPECT_EQ(window.keyboard, Keyboard::kPage);
}

TEST(OverlayLend, ATabClosedUnderTheOverlayGivesItBack) {
  // A Space the dialog deleted: what shows comes of that.
  OverlayLend lend;
  lend.Take(kTab);
  EXPECT_EQ(lend.LetGo(kBoard, /*page_shown=*/true, [](uint64_t) { return false; }), Goes::kBack);
}

TEST(OverlayLend, NoPageShownGivesItBack) {
  OverlayLend lend;
  lend.Take(kTab);
  EXPECT_EQ(lend.LetGo(kBoard, /*page_shown=*/false, Open), Goes::kBack);
}

}  // namespace
}  // namespace browser_core
