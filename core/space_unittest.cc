#include "browser_core/space.h"

#include <map>
#include <set>
#include <string>
#include <vector>

#include "base/no_destructor.h"
#include "base/test/task_environment.h"
#include "browser_core/auto_archive.h"
#include "browser_core/closing_way_back.h"
#include "browser_core/favorites.h"
#include "browser_core/host.h"
#include "browser_core/session.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core {

// What space.cc asks of the rest of the core, which the test leaves out.
bc_id NewId() {
  static bc_id last = 1000;
  return ++last;
}

const bc_host& Host() {
  static const bc_host host{};
  return host;
}

void SessionChanged() {}

void ScheduleAutoArchive() {}

void BoostSpacesChanged() {}
void BoostSpaceGone(bc_id) {}

// The favorites every Space shows, as the test sets them.
std::set<bc_id>& ShownFavorites() {
  static base::NoDestructor<std::set<bc_id>> tabs;
  return *tabs;
}

bool Favorites::ShownIn(bc_id tab, const Space&) {
  return ShownFavorites().contains(tab);
}

namespace {

using Rows = std::vector<bc_id>;

class SpaceTest : public testing::Test {
 protected:
  void SetUp() override {
    work_ = Space::Create("Work");
    home_ = Space::Create("Home");
  }

  void TearDown() override {
    Space::Delete(work_->id());
    Space::Delete(home_->id());
    ShownFavorites().clear();
  }

  // The tab Work shows once |tab| closes, every tab loaded.
  bc_id NextInWork(bc_id tab) const {
    return work_->NextAfterLeaving(tab, [](bc_id) { return true; });
  }

  // In Work: a pinned tab, then the folder Docs with a tab, the folder Notes holding a split view,
  // and a tab, then a pinned tab. Home has a pinned tab of its own.
  void Fill() {
    work_->AddPinned(1, 0, 0, Url(1));
    docs_ = work_->AddFolder("Docs", 0, 1);
    work_->AddPinned(2, docs_, 0, Url(2));
    notes_ = work_->AddFolder("Notes", docs_, 1);
    work_->AddPinned(3, notes_, 0, Url(3));
    work_->AddPinned(4, notes_, 1, Url(4));
    ASSERT_TRUE(work_->Split(4, 3, GURL()));
    work_->AddPinned(5, docs_, 2, Url(5));
    work_->AddPinned(6, 0, 2, Url(6));
    work_->SetFolderIcon(docs_, "star");
    work_->SetOpen(notes_, false);
    home_->AddPinned(7, 0, 0, Url(7));
    ASSERT_EQ(work_->pinned_rows(), (Rows{1, docs_, 2, notes_, 3, 4, 5, 6}));
  }

  static GURL Url(bc_id tab) { return GURL("https://example.com/" + std::to_string(tab)); }

  base::test::TaskEnvironment task_environment_;
  Space* work_ = nullptr;
  Space* home_ = nullptr;
  bc_id docs_ = 0;
  bc_id notes_ = 0;
};

// What an archived tab goes back to: the user's Space it closed in, while there's one.
// A tab made for a Space whose window places it a task later, as one an agent's tab opens, is that
// Space's from when it's made, so its page's first request finds it there, and only until it's
// placed or gone (#2660).
TEST_F(SpaceTest, AnExpectedTabIsTheSpacesFromWhenItsMadeUntilItsPlacedOrGone) {
  work_->Expect(7);
  EXPECT_EQ(Space::Holding(7), work_);
  EXPECT_FALSE(work_->Has(7));
  EXPECT_EQ(work_->today(), Rows{});
  work_->Add(7, /*opener=*/0, /*foreground=*/false);
  EXPECT_EQ(Space::Holding(7), work_);
  EXPECT_EQ(work_->today(), Rows{7});
  work_->Remove(7);
  EXPECT_EQ(Space::Holding(7), nullptr);
  // One that closes before its window placed it.
  work_->Expect(8);
  work_->Remove(8);
  EXPECT_EQ(Space::Holding(8), nullptr);
  // A Space holding a tab is where it is, whichever expected it.
  home_->Expect(9);
  work_->Add(9, /*opener=*/0, /*foreground=*/false);
  EXPECT_EQ(Space::Holding(9), work_);
}

// A tab a page opens is its opener's Space's until it's listed, whatever becomes of the opener in
// between: the Space holds it, not the opener.
TEST_F(SpaceTest, ATabAPageOpenedIsItsOpenersSpacesAfterTheOpenerClosedOrLeft) {
  work_->Add(5, /*opener=*/0, /*foreground=*/false);
  work_->Expect(7);
  work_->Remove(5);
  EXPECT_EQ(Space::Holding(7), work_);
  work_->Add(6, /*opener=*/0, /*foreground=*/false);
  work_->Expect(8);
  work_->Remove(6);
  home_->Add(6, /*opener=*/0, /*foreground=*/false);
  EXPECT_EQ(Space::Holding(8), work_);
}

TEST_F(SpaceTest, ASpaceIsFoundByItsNodeWhileItIsTheUsers) {
  const base::Uuid work = work_->node().id;
  ASSERT_TRUE(work.is_valid());
  EXPECT_EQ(Space::WithNode(work), work_);
  EXPECT_EQ(Space::WithNode(base::Uuid()), nullptr);

  Space* agent = Space::Create("Claude Code", 0, Space::Agent{.id = "claude", .name = "Claude"});
  const base::Uuid agents = agent->node().id;
  EXPECT_EQ(Space::WithNode(agents), nullptr);
  Space::Delete(agent->id());

  Space::Delete(work_->id());
  EXPECT_EQ(Space::WithNode(work), nullptr);
  work_ = Space::Create("Work");
}

TEST_F(SpaceTest, AFolderMovesWholeAndInOrderToTheBottomOfThePinnedTabs) {
  Fill();
  const base::Uuid node = work_->NodeOf(docs_)->id;

  home_->TakeFolder(*work_, docs_, {});

  EXPECT_EQ(home_->pinned_rows(), (Rows{7, docs_, 2, notes_, 3, 4, 5}));
  EXPECT_EQ(work_->pinned_rows(), (Rows{1, 6}));
  EXPECT_EQ(Space::HoldingFolder(docs_), home_);
  EXPECT_EQ(Space::HoldingFolder(notes_), home_);
  EXPECT_EQ(Space::Holding(3), home_);

  EXPECT_EQ(home_->FolderOf(docs_), 0u);
  EXPECT_EQ(home_->FolderOf(2), docs_);
  EXPECT_EQ(home_->FolderOf(notes_), docs_);
  EXPECT_EQ(home_->FolderOf(3), notes_);
  EXPECT_EQ(home_->FolderOf(4), notes_);
  EXPECT_EQ(home_->FolderOf(5), docs_);

  EXPECT_EQ(home_->FolderName(docs_), "Docs");
  EXPECT_EQ(home_->FolderIcon(docs_), "star");
  EXPECT_TRUE(home_->IsOpen(docs_));
  EXPECT_EQ(home_->FolderName(notes_), "Notes");
  EXPECT_FALSE(home_->IsOpen(notes_));

  for (bc_id tab : Rows{2, 3, 4, 5}) {
    EXPECT_EQ(home_->PinnedUrl(tab), Url(tab));
    EXPECT_FALSE(work_->Has(tab));
  }
  EXPECT_EQ(home_->Panes(3), (Rows{3, 4}));
  EXPECT_TRUE(work_->Panes(3).empty());

  // A new node in Home, as sync carries a row moved to another Space, and none left in Work.
  ASSERT_TRUE(home_->NodeOf(docs_));
  EXPECT_NE(home_->NodeOf(docs_)->id, node);
  EXPECT_TRUE(home_->NodeOf(3));
  EXPECT_FALSE(work_->NodeOf(docs_));
  EXPECT_FALSE(work_->NodeOf(3));
  EXPECT_TRUE(home_->NodeOf(7)->position.LessThan(home_->NodeOf(docs_)->position));
}

// A split's name and icon are the split's, given by any of its tabs: they go where it goes, and
// they go once it does, so a split made again of the same tabs has none.
TEST_F(SpaceTest, ASplitsNameAndIconAreItsOwnAndGoWithIt) {
  Fill();
  EXPECT_TRUE(work_->SetSplitName(4, "Reading"));
  EXPECT_TRUE(work_->SetSplitIcon(3, "📚"));
  EXPECT_FALSE(work_->SetSplitName(1, "Alone"));
  EXPECT_EQ(work_->SplitName(3), "Reading");
  EXPECT_EQ(work_->SplitIcon(4), "📚");
  EXPECT_EQ(work_->SplitName(1), "");

  home_->TakeFolder(*work_, docs_, {});
  EXPECT_EQ(home_->SplitName(3), "Reading");
  EXPECT_EQ(home_->SplitIcon(4), "📚");

  home_->Separate(4);
  EXPECT_EQ(home_->SplitName(3), "");
  ASSERT_TRUE(home_->Split(4, 3, GURL()));
  EXPECT_EQ(home_->SplitName(3), "");
  EXPECT_EQ(home_->SplitIcon(4), "");
}

// Into another profile's Space each tab goes as the new one standing in for it.
TEST_F(SpaceTest, TabsCopiedToAnotherProfileTakeTheirPlaces) {
  Fill();

  home_->TakeFolder(*work_, docs_, {{2, 12}, {3, 13}, {4, 14}, {5, 15}});

  EXPECT_EQ(home_->pinned_rows(), (Rows{7, docs_, 12, notes_, 13, 14, 15}));
  EXPECT_EQ(home_->FolderOf(13), notes_);
  EXPECT_EQ(home_->PinnedUrl(14), Url(4));
  EXPECT_EQ(home_->Panes(13), (Rows{13, 14}));
  EXPECT_FALSE(home_->Has(3));
  EXPECT_EQ(work_->pinned_rows(), (Rows{1, 6}));
}

TEST_F(SpaceTest, OnlyAFolderMovesAndNeverIntoAWindowsOwnSpace) {
  Fill();
  Space* little = Space::CreateOwn("Default", Space::Own::kLittle);

  little->TakeFolder(*work_, docs_, {});
  home_->TakeFolder(*work_, 2, {});
  work_->TakeFolder(*work_, docs_, {});

  EXPECT_EQ(work_->pinned_rows(), (Rows{1, docs_, 2, notes_, 3, 4, 5, 6}));
  EXPECT_TRUE(little->pinned_rows().empty());
  EXPECT_EQ(home_->pinned_rows(), (Rows{7}));
  Space::Delete(little->id());
}

// A Live Folder is the user's: a Space has one of each source, an agent's Space and a window's own
// none, and one moved to a Space with its own comes plain (#2979).
TEST_F(SpaceTest, ASpaceHasOneLiveFolderOfASourceAndOnlyTheUsersHaveAny) {
  Fill();
  work_->SetFolderLive(docs_, BC_LIVE_PULL_REQUESTS);
  EXPECT_EQ(work_->FolderLive(docs_), BC_LIVE_PULL_REQUESTS);
  EXPECT_EQ(work_->LiveFolderOf(BC_LIVE_PULL_REQUESTS), docs_);
  // A second of the source, and a source the core doesn't know, stay plain.
  work_->SetFolderLive(notes_, BC_LIVE_PULL_REQUESTS);
  work_->SetFolderLive(notes_, 7);
  EXPECT_EQ(work_->FolderLive(notes_), 0);

  Space* agent = Space::Create("Claude Code", 0, Space::Agent{.id = "claude", .name = "Claude"});
  const bc_id agents = agent->AddFolder("Pull Requests", 0, 0);
  agent->SetFolderLive(agents, BC_LIVE_PULL_REQUESTS);
  EXPECT_EQ(agent->FolderLive(agents), 0);
  Space::Delete(agent->id());

  // Home has one of its own, so Docs comes plain, its tabs with it.
  const bc_id homes = home_->AddFolder("Pull Requests", 0, 1);
  home_->SetFolderLive(homes, BC_LIVE_PULL_REQUESTS);
  home_->TakeFolder(*work_, docs_, {});
  EXPECT_EQ(home_->FolderLive(docs_), 0);
  EXPECT_EQ(home_->LiveFolderOf(BC_LIVE_PULL_REQUESTS), homes);
  EXPECT_EQ(work_->LiveFolderOf(BC_LIVE_PULL_REQUESTS), 0u);

  // A Live Folder taken into an agent's Space comes plain there, as one made there would be.
  Space* agents_space = Space::Create("Codex", 0, Space::Agent{.id = "codex", .name = "Codex"});
  agents_space->TakeFolder(*home_, homes, {});
  EXPECT_TRUE(agents_space->IsFolder(homes));
  EXPECT_EQ(agents_space->FolderLive(homes), 0);
  EXPECT_FALSE(agents_space->TakesLiveFolder(BC_LIVE_PULL_REQUESTS));
  EXPECT_EQ(home_->LiveFolderOf(BC_LIVE_PULL_REQUESTS), 0u);
  EXPECT_TRUE(home_->TakesLiveFolder(BC_LIVE_PULL_REQUESTS));
  home_->TakeFolder(*agents_space, homes, {});
  // Back in a user's Space it stays plain: what was live went with the move into the agent's.
  EXPECT_EQ(home_->FolderLive(homes), 0);
  Space::Delete(agents_space->id());
  home_->SetFolderLive(homes, BC_LIVE_PULL_REQUESTS);

  // Ended, it's a plain folder with what's in it.
  home_->SetFolderLive(homes, 0);
  EXPECT_EQ(home_->LiveFolderOf(BC_LIVE_PULL_REQUESTS), 0u);
  EXPECT_TRUE(home_->IsFolder(homes));
}

// A Space moved goes right after the one named, or first, and only its place changes, so sync
// sends it alone. Where it is already, or after a Space that's gone, changes nothing.
TEST_F(SpaceTest, ASpaceMovedTakesANewPlaceAndTheOthersKeepTheirs) {
  Space* play = Space::Create("Play");
  auto order = [] {
    Rows ids;
    for (Space* space : Space::All()) {
      ids.push_back(space->id());
    }
    return ids;
  };
  const syncer::UniquePosition work = work_->node().position;
  const syncer::UniquePosition home = home_->node().position;

  Space::MoveAfter(play->id(), 0);
  EXPECT_EQ(order(), (Rows{play->id(), work_->id(), home_->id()}));
  EXPECT_TRUE(work_->node().position.Equals(work));
  EXPECT_TRUE(home_->node().position.Equals(home));
  EXPECT_TRUE(play->node().position.LessThan(work));

  Space::MoveAfter(play->id(), work_->id());
  EXPECT_EQ(order(), (Rows{work_->id(), play->id(), home_->id()}));
  Space::MoveAfter(play->id(), home_->id());
  EXPECT_EQ(order(), (Rows{work_->id(), home_->id(), play->id()}));
  const syncer::UniquePosition last = play->node().position;
  EXPECT_TRUE(home.LessThan(last));
  Space::MoveAfter(play->id(), home_->id());
  Space::MoveAfter(play->id(), play->id());
  Space::MoveAfter(play->id(), 999999);
  EXPECT_EQ(order(), (Rows{work_->id(), home_->id(), play->id()}));
  EXPECT_TRUE(play->node().position.Equals(last));
  Space::Delete(play->id());
}

// Each Today tab is a node, as sync knows it, apart from the pinned tabs, and keeps its id pinned
// and unpinned, so other devices move it rather than closing one and opening another (#2098). A
// window's own Space isn't synced, so its tabs have none.
TEST_F(SpaceTest, ATodayTabIsANodeOfItsOwnThatKeepsItsIdPinnedAndUnpinned) {
  Fill();
  work_->AddAt(20, 0);
  work_->AddAt(21, 1);
  work_->AddAt(22, 0);
  ASSERT_EQ(work_->today(), (Rows{22, 20, 21}));
  const Space::Node* top = work_->NodeOf(22);
  ASSERT_TRUE(top && work_->NodeOf(20) && work_->NodeOf(21));
  EXPECT_TRUE(top->position.LessThan(work_->NodeOf(20)->position));
  EXPECT_TRUE(work_->NodeOf(20)->position.LessThan(work_->NodeOf(21)->position));
  const base::Uuid id = work_->NodeOf(21)->id;
  EXPECT_NE(id, work_->NodeOf(20)->id);

  work_->Move(21, /*pinned=*/true, 0, 0, [](bc_id) { return Url(21); });
  ASSERT_TRUE(work_->NodeOf(21));
  EXPECT_EQ(work_->NodeOf(21)->id, id);
  EXPECT_TRUE(work_->NodeOf(21)->position.LessThan(work_->NodeOf(1)->position));
  work_->Move(21, /*pinned=*/false, 0, 0, [](bc_id) { return GURL(); });
  ASSERT_TRUE(work_->NodeOf(21));
  EXPECT_EQ(work_->NodeOf(21)->id, id);
  EXPECT_TRUE(work_->NodeOf(21)->position.LessThan(work_->NodeOf(22)->position));

  work_->Remove(20);
  EXPECT_FALSE(work_->NodeOf(20));

  Space* little = Space::CreateOwn("Default", Space::Own::kLittle);
  little->AddAt(30, 0);
  EXPECT_FALSE(little->NodeOf(30));
  Space::Delete(little->id());
}

// A tab the user opens over another, as the command bar's ⌘T opens it, goes back to that one when
// it closes, a Today tab, a pinned tab or a favorite the Space shows, not to the tab below (#3813).
TEST_F(SpaceTest, ATabOpenedOverAnotherClosesBackToIt) {
  Fill();
  work_->Add(20, /*opener=*/0, /*foreground=*/false);
  work_->Add(21, /*opener=*/0, /*foreground=*/false);
  // Over the Today tab at the bottom, it goes at the top.
  work_->AddOpenedOver(22, 20);
  EXPECT_EQ(work_->today(), (Rows{22, 21, 20}));
  EXPECT_EQ(NextInWork(22), 20);
  work_->Remove(22);
  // Over a pinned tab.
  work_->AddOpenedOver(23, 1);
  EXPECT_EQ(work_->today(), (Rows{23, 21, 20}));
  EXPECT_EQ(NextInWork(23), 1);
  work_->Remove(23);
  // Over a favorite, while the Space shows it.
  ShownFavorites() = {30};
  work_->AddOpenedOver(24, 30);
  EXPECT_EQ(NextInWork(24), 30);
  ShownFavorites().clear();
  EXPECT_EQ(NextInWork(24), 21);
}

// The tab it opened over is the way back until the user shows an unrelated tab, as Chrome's tab
// strip has it, and the one opened last over another takes the way back from those before it.
TEST_F(SpaceTest, ATabOpenedOverAnotherForgetsItOnceTheUserGoesElsewhere) {
  work_->Add(20, /*opener=*/0, /*foreground=*/false);
  work_->Add(21, /*opener=*/0, /*foreground=*/false);
  work_->AddOpenedOver(22, 20);
  work_->UserShowed(20, 22);
  EXPECT_EQ(NextInWork(22), 20);
  work_->UserShowed(22, 21);
  work_->UserShowed(21, 22);
  EXPECT_EQ(NextInWork(22), 21);
  work_->AddOpenedOver(23, 20);
  work_->AddOpenedOver(24, 20);
  EXPECT_EQ(work_->today(), (Rows{24, 23, 22, 21, 20}));
  EXPECT_EQ(NextInWork(24), 20);
  EXPECT_EQ(NextInWork(23), 22);
}

// A tab the user closes over a favorite, the Space's only other tab, as the command bar's search
// opens it, gives way to the favorite whether its page lets it go before the window leaves the
// Space or after, as Window::CloseTab, the switch and Window::OnTabClosing call the rule (#3890).
TEST_F(SpaceTest, AClosedTabGivesWayToTheTabUnderItWhicheverLandsFirst) {
  ShownFavorites() = {30};
  ClosingWayBack way_back;
  // The close lands first: the Space still knows where the tab came from.
  work_->AddOpenedOver(24, 30);
  way_back.Closing(24);
  EXPECT_EQ(way_back.Closed(24, NextInWork(24)), 30);
  work_->Remove(24);
  // The switch lands first and forgets that, so the way back is kept as the window leaves.
  work_->AddOpenedOver(25, 30);
  way_back.Closing(25);
  way_back.Left(25, NextInWork(25));
  work_->ForgetOpeners();
  EXPECT_EQ(NextInWork(25), 0);
  EXPECT_EQ(way_back.Closed(25, NextInWork(25)), 30);
  work_->Remove(25);
}

// Stay on the tab's beforeunload keeps it, a tab like any other, so a close after, an agent's
// too, goes where the Space has it then. A second leaving keeps the first's way back.
TEST_F(SpaceTest, AClosedTabKeptByItsPageForgetsTheWayBack) {
  ShownFavorites() = {30};
  ClosingWayBack way_back;
  work_->AddOpenedOver(24, 30);
  way_back.Closing(24);
  way_back.Left(24, NextInWork(24));
  work_->ForgetOpeners();
  way_back.Stayed(24);
  EXPECT_EQ(way_back.Closed(24, NextInWork(24)), 0);
  work_->AddOpenedOver(25, 30);
  way_back.Closing(25);
  way_back.Left(25, NextInWork(25));
  work_->ForgetOpeners();
  way_back.Left(25, NextInWork(25));
  EXPECT_EQ(way_back.Closed(25, 0), 30);
  // One the user didn't close keeps the Space's pick.
  way_back.Left(26, 30);
  EXPECT_EQ(way_back.Closed(26, 21), 21);
}

}  // namespace
}  // namespace browser_core
