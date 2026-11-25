#include "browser_core/handed_back.h"

#include <algorithm>
#include <map>
#include <optional>
#include <string>
#include <vector>

#include "base/test/task_environment.h"
#include "browser_core/space.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core::handed_back {
namespace {

using Rows = std::vector<bc_id>;

// What the core keeps of a tab as it's given (handed::Leaving).
struct Left {
  Space::Node node;
  Rows split;
  std::optional<Pin> pin;
  bc_id today_folder = 0;
};

// Space alone, with what it asks of the rest of the core as space_unittest.cc stubs it.
class HandedBackTest : public testing::Test {
 protected:
  // In Work: a pinned tab, then the folder Docs with a tab, the folder Notes holding a split view,
  // and a tab, then a pinned tab. Three Today tabs, the lower two in a split view.
  void SetUp() override {
    work_ = Space::Create("Work");
    work_->AddPinned(1, 0, 0, Url(1));
    docs_ = work_->AddFolder("Docs", 0, 1);
    work_->AddPinned(2, docs_, 0, Url(2));
    notes_ = work_->AddFolder("Notes", docs_, 1);
    work_->AddPinned(3, notes_, 0, Url(3));
    work_->AddPinned(4, notes_, 1, Url(4));
    ASSERT_TRUE(work_->Split(4, 3, GURL()));
    work_->AddPinned(5, docs_, 2, Url(5));
    work_->AddPinned(6, 0, 2, Url(6));
    for (bc_id tab : {11, 12, 13}) {
      work_->AddAt(tab, SIZE_MAX);
    }
    ASSERT_TRUE(work_->Split(13, 12, GURL()));
    ASSERT_EQ(work_->pinned_rows(), (Rows{1, docs_, 2, notes_, 3, 4, 5, 6}));
    ASSERT_EQ(work_->today(), (Rows{11, 12, 13}));
  }

  void TearDown() override { Space::Delete(work_->id()); }

  // |tab| leaves Work as a given tab does: what's kept of it, and then out of the Space.
  Left Give(bc_id tab) {
    Left left{.node = *work_->NodeOf(tab),
              .split = work_->Panes(tab),
              .pin = PinOf(*work_, tab),
              .today_folder = TodayFolderOf(*work_, tab)};
    if (left.split.size() < 2) {
      left.split.clear();
    }
    work_->Remove(tab);
    return left;
  }

  // |tab| comes back to Work as a returned tab does: at the top of Today, then put back.
  void Return(bc_id tab, const Left& left) {
    work_->AddAt(tab, 0);
    Put(*work_, tab, left.node, left.split, left.pin, left.today_folder);
  }

  // Four more Today tabs under the split view, the upper three in the today's folder Trip, which
  // is returned.
  bc_id AddTrip() {
    for (bc_id tab : {21, 22, 23, 24}) {
      work_->AddAt(tab, SIZE_MAX);
    }
    const bc_id trip = work_->AddTodayFolder("Trip", {21, 22, 23});
    EXPECT_TRUE(trip);
    EXPECT_EQ(work_->today(), (Rows{11, 12, 13, 21, 22, 23, 24}));
    return trip;
  }

  // What the session saves of Work's Today tabs while |tab| is away with |left|.
  Today SavedToday(bc_id tab, const Left& left) const {
    return TodayWith(*work_, {{.tab = tab, .node = left.node, .folder = left.today_folder}});
  }

  // The rows |saved| has in |folder|, which must be one unbroken run: a launch brings back only a
  // today's folder whose tabs come back together.
  Rows RunOf(const Today& saved, bc_id folder) const {
    Rows in;
    std::vector<size_t> at;
    for (size_t i = 0; i < saved.rows.size(); ++i) {
      auto handed = saved.folders.find(saved.rows[i]);
      if ((handed == saved.folders.end() ? TodayFolderOf(*work_, saved.rows[i]) : handed->second) ==
          folder) {
        in.push_back(saved.rows[i]);
        at.push_back(i);
      }
    }
    EXPECT_TRUE(at.empty() || at.back() - at.front() + 1 == at.size());
    return in;
  }

  // Whether |tab| has the node it left with, its id and its place.
  bool HasNode(bc_id tab, const Left& left) const {
    const Space::Node* node = work_->NodeOf(tab);
    return node && node->id == left.node.id && node->position.Equals(left.node.position);
  }

  // Each of Work's rows and today's folders with its node, its id and its place, as text to
  // compare.
  std::map<bc_id, std::string> Nodes() const {
    std::map<bc_id, std::string> nodes;
    const Rows today_folders = work_->TodayFolders();
    for (const Rows* rows : {&work_->pinned_rows(), &work_->today(), &today_folders}) {
      for (bc_id row : *rows) {
        const Space::Node* node = work_->NodeOf(row);
        nodes[row] = node ? node->id.AsLowercaseString() + " " + node->position.ToDebugString()
                          : std::string();
      }
    }
    return nodes;
  }

  // What the session saves of Work's pinned rows while |tab| is away with |left|.
  Pinned SavedWith(bc_id tab, const Left& left) const {
    return PinnedWith(*work_, {{.tab = tab, .node = left.node, .pin = *left.pin}});
  }

  size_t RowOf(bc_id row) const {
    return std::ranges::find(work_->pinned_rows(), row) - work_->pinned_rows().begin();
  }

  static GURL Url(bc_id tab) { return GURL("https://example.com/" + std::to_string(tab)); }

  base::test::TaskEnvironment task_environment_;
  Space* work_ = nullptr;
  bc_id docs_ = 0;
  bc_id notes_ = 0;
};

TEST_F(HandedBackTest, OnlyAPinnedTabHasAPin) {
  EXPECT_FALSE(PinOf(*work_, 11));
  EXPECT_FALSE(PinOf(*work_, docs_));
  const std::optional<Pin> pin = PinOf(*work_, 3);
  ASSERT_TRUE(pin);
  EXPECT_EQ(pin->url, Url(3));
  ASSERT_EQ(pin->folders.size(), 2u);
  EXPECT_EQ(pin->folders[0].id, notes_);
  EXPECT_EQ(pin->folders[1].id, docs_);
  EXPECT_TRUE(PinOf(*work_, 1)->folders.empty());
}

TEST_F(HandedBackTest, APinnedTabComesBackPinnedWhereItWas) {
  for (bc_id tab : {1, 2, 5, 6}) {
    const Left left = Give(tab);
    EXPECT_FALSE(work_->Has(tab));
    Return(tab, left);
    EXPECT_EQ(work_->pinned_rows(), (Rows{1, docs_, 2, notes_, 3, 4, 5, 6})) << tab;
    EXPECT_EQ(work_->today(), (Rows{11, 12, 13})) << tab;
    EXPECT_EQ(work_->PinnedUrl(tab), Url(tab));
    EXPECT_TRUE(HasNode(tab, left)) << tab;
  }
  EXPECT_EQ(work_->FolderOf(2), docs_);
  EXPECT_EQ(work_->FolderOf(6), 0u);
}

// Whatever was pinned, unpinned or closed around it meanwhile.
TEST_F(HandedBackTest, APinnedTabComesBackBetweenTheRowsItWasBetween) {
  const Left left = Give(5);
  work_->AddPinned(7, docs_, 0, Url(7));
  work_->Remove(2);
  Return(5, left);
  EXPECT_EQ(work_->pinned_rows(), (Rows{1, docs_, 7, notes_, 3, 4, 5, 6}));
  EXPECT_EQ(work_->PinnedUrl(5), Url(5));
  EXPECT_TRUE(HasNode(5, left));
}

// The pin's node and its address are as they were, from either side of the split.
TEST_F(HandedBackTest, APinnedTabOfASplitViewRejoinsItWithItsNodeAndItsAddress) {
  for (bc_id tab : {3, 4}) {
    const Left left = Give(tab);
    EXPECT_EQ(work_->Panes(tab == 3 ? 4 : 3).size(), 1u);
    Return(tab, left);
    EXPECT_EQ(work_->pinned_rows(), (Rows{1, docs_, 2, notes_, 3, 4, 5, 6})) << tab;
    EXPECT_EQ(work_->Panes(tab), (Rows{3, 4})) << tab;
    EXPECT_EQ(work_->FolderOf(tab), notes_);
    EXPECT_EQ(work_->PinnedUrl(tab), Url(tab));
    EXPECT_TRUE(HasNode(tab, left)) << tab;
  }
}

// Unpinned meanwhile, the other tab of its split is no pinned tab's to join.
TEST_F(HandedBackTest, APinnedTabStaysLooseOnceItsSplitsOtherTabIsUnpinned) {
  const Left left = Give(4);
  work_->Unpin(3);
  Return(4, left);
  EXPECT_EQ(work_->pinned_rows(), (Rows{1, docs_, 2, notes_, 4, 5, 6}));
  EXPECT_EQ(work_->Panes(4), Rows{4});
  EXPECT_EQ(work_->PinnedUrl(4), Url(4));
}

TEST_F(HandedBackTest, OnceItsFolderIsGoneItsInTheOneThatHeldItWhereTheFolderWas) {
  const Left left = Give(3);
  work_->DeleteFolder(notes_);
  ASSERT_EQ(work_->pinned_rows(), (Rows{1, docs_, 2, 4, 5, 6}));
  Return(3, left);
  EXPECT_EQ(work_->FolderOf(3), docs_);
  // Beside the tab of its split, which is where its folder was.
  EXPECT_EQ(work_->pinned_rows(), (Rows{1, docs_, 2, 3, 4, 5, 6}));
  EXPECT_EQ(work_->Panes(3), (Rows{3, 4}));
  EXPECT_EQ(work_->PinnedUrl(3), Url(3));
  EXPECT_EQ(work_->NodeOf(3)->id, left.node.id);
}

TEST_F(HandedBackTest, OutsideEveryFolderOnceAllItWasInAreGone) {
  const Left left = Give(2);
  work_->DeleteFolder(docs_);
  Return(2, left);
  EXPECT_TRUE(work_->IsPinned(2));
  EXPECT_EQ(work_->FolderOf(2), 0u);
  // Where its folder was: under the tab that was above the folder, over the one that was below.
  EXPECT_LT(RowOf(1), RowOf(2));
  EXPECT_LT(RowOf(2), RowOf(6));
  EXPECT_EQ(work_->PinnedUrl(2), Url(2));
  EXPECT_EQ(work_->NodeOf(2)->id, left.node.id);
}

// The account's other devices see no move, of the tab or of a row beside it: every node is as it
// was before the give.
TEST_F(HandedBackTest, NoRowsNodeChangesWithAGiveAndAReturn) {
  const std::map<bc_id, std::string> before = Nodes();
  for (bc_id tab : {1, 2, 3, 4, 5, 6, 11, 12, 13}) {
    const Left left = Give(tab);
    Return(tab, left);
    EXPECT_EQ(Nodes(), before) << tab;
  }
}

// The session saves a handed pin among the Space's pinned rows where it would be pinned again, so
// a quit while an agent has it brings it back as the pin it was.
TEST_F(HandedBackTest, AHandedPinIsSavedWhereItComesBack) {
  for (bc_id tab : {1, 2, 3, 4, 5, 6}) {
    const bc_id folder = work_->FolderOf(tab);
    const Left left = Give(tab);
    const Pinned saved = SavedWith(tab, left);
    EXPECT_EQ(saved.rows, (Rows{1, docs_, 2, notes_, 3, 4, 5, 6})) << tab;
    EXPECT_EQ(saved.folders, (std::map<bc_id, bc_id>{{tab, folder}})) << tab;
    Return(tab, left);
    EXPECT_EQ(work_->pinned_rows(), saved.rows) << tab;
  }
}

TEST_F(HandedBackTest, HandedPinsOfOneFolderAreSavedInTheirOrder) {
  const Left two = Give(2);
  const Left five = Give(5);
  ASSERT_EQ(work_->pinned_rows(), (Rows{1, docs_, notes_, 3, 4, 6}));
  for (const std::vector<Held>& held :
       {std::vector<Held>{{.tab = 2, .node = two.node, .pin = *two.pin},
                          {.tab = 5, .node = five.node, .pin = *five.pin}},
        std::vector<Held>{{.tab = 5, .node = five.node, .pin = *five.pin},
                          {.tab = 2, .node = two.node, .pin = *two.pin}}}) {
    const Pinned saved = PinnedWith(*work_, held);
    EXPECT_EQ(saved.rows, (Rows{1, docs_, 2, notes_, 3, 4, 5, 6}));
    EXPECT_EQ(saved.folders, (std::map<bc_id, bc_id>{{2, docs_}, {5, docs_}}));
  }
}

// In the folder that held its folder, past what that one holds, as it would be pinned again.
TEST_F(HandedBackTest, AHandedPinWhoseFolderIsGoneIsSavedInTheOneThatHeldIt) {
  const Left left = Give(5);
  work_->DeleteFolder(docs_);
  const Pinned saved = SavedWith(5, left);
  EXPECT_EQ(saved.folders, (std::map<bc_id, bc_id>{{5, 0}}));
  Return(5, left);
  EXPECT_EQ(work_->pinned_rows(), saved.rows);
  EXPECT_EQ(work_->FolderOf(5), 0u);
}

TEST_F(HandedBackTest, ATodayTabComesBackWhereItWasAndIntoItsSplit) {
  Left left = Give(11);
  EXPECT_FALSE(left.pin);
  Return(11, left);
  EXPECT_EQ(work_->today(), (Rows{11, 12, 13}));
  EXPECT_TRUE(HasNode(11, left));
  for (bc_id tab : {12, 13}) {
    left = Give(tab);
    Return(tab, left);
    EXPECT_EQ(work_->today(), (Rows{11, 12, 13})) << tab;
    EXPECT_EQ(work_->Panes(tab), (Rows{12, 13})) << tab;
    EXPECT_FALSE(work_->IsPinned(tab));
    EXPECT_TRUE(HasNode(tab, left)) << tab;
  }
}

// Pinned meanwhile, the other tab of its split is no Today tab's to join.
TEST_F(HandedBackTest, ATodayTabStaysLooseOnceItsSplitsOtherTabIsPinned) {
  const Left left = Give(13);
  work_->Pin(12, [](bc_id tab) { return Url(tab); });
  Return(13, left);
  EXPECT_EQ(work_->today(), (Rows{11, 13}));
  EXPECT_EQ(work_->Panes(13), Rows{13});
}

TEST_F(HandedBackTest, OnlyATodayTabInATodaysFolderHasOne) {
  const bc_id trip = AddTrip();
  EXPECT_EQ(TodayFolderOf(*work_, 22), trip);
  EXPECT_EQ(TodayFolderOf(*work_, 24), 0u);
  // A pinned tab's folder is its pin's.
  EXPECT_EQ(TodayFolderOf(*work_, 2), 0u);
}

TEST_F(HandedBackTest, ATodayTabComesBackIntoItsTodaysFolderFromBetweenItsTabs) {
  const bc_id trip = AddTrip();
  const Left left = Give(22);
  EXPECT_EQ(work_->TodayFolderTabs(trip), (Rows{21, 23}));
  Return(22, left);
  EXPECT_EQ(work_->today(), (Rows{11, 12, 13, 21, 22, 23, 24}));
  EXPECT_EQ(work_->TodayFolderTabs(trip), (Rows{21, 22, 23}));
  EXPECT_TRUE(HasNode(22, left));
}

// At the folder's edge its place alone is beside the folder: what's kept of it puts it inside.
TEST_F(HandedBackTest, ATodaysFoldersFirstTabAndItsLastComeBackIntoIt) {
  const bc_id trip = AddTrip();
  for (bc_id tab : {21, 23}) {
    const Left left = Give(tab);
    Return(tab, left);
    EXPECT_EQ(work_->today(), (Rows{11, 12, 13, 21, 22, 23, 24})) << tab;
    EXPECT_EQ(work_->TodayFolderTabs(trip), (Rows{21, 22, 23})) << tab;
    EXPECT_TRUE(HasNode(tab, left)) << tab;
  }
}

TEST_F(HandedBackTest, OnceItsTodaysFolderIsDeletedItsLooseAtItsPlace) {
  const bc_id trip = AddTrip();
  const Left left = Give(22);
  work_->DeleteFolder(trip);
  Return(22, left);
  EXPECT_EQ(work_->today(), (Rows{11, 12, 13, 21, 22, 23, 24}));
  // No folder is made for it.
  EXPECT_TRUE(work_->TodayFolders().empty());
  EXPECT_EQ(work_->FolderOf(22), 0u);
  EXPECT_TRUE(HasNode(22, left));
}

// A today's folder goes with its last tab, so one whose tabs were all given isn't there to come
// back to.
TEST_F(HandedBackTest, OnceItsTodaysFolderWentWithItsLastTabItsLooseAtItsPlace) {
  const bc_id trip = AddTrip();
  std::map<bc_id, Left> left;
  for (bc_id tab : {21, 22, 23}) {
    left[tab] = Give(tab);
  }
  EXPECT_FALSE(work_->IsTodayFolder(trip));
  for (bc_id tab : {22, 21, 23}) {
    Return(tab, left[tab]);
    EXPECT_TRUE(work_->TodayFolders().empty()) << tab;
  }
  EXPECT_EQ(work_->today(), (Rows{11, 12, 13, 21, 22, 23, 24}));
}

TEST_F(HandedBackTest, NoTodaysFoldersNodeChangesWithAGiveAndAReturn) {
  AddTrip();
  for (bc_id tab : {31, 32}) {
    work_->AddAt(tab, SIZE_MAX);
  }
  ASSERT_TRUE(work_->AddTodayFolder("Plans", {31, 32}));
  const std::map<bc_id, std::string> before = Nodes();
  for (bc_id tab : {21, 22, 23, 24, 31, 32}) {
    const Left left = Give(tab);
    Return(tab, left);
    EXPECT_EQ(Nodes(), before) << tab;
  }
}

TEST_F(HandedBackTest, AHandedTodayTabIsSavedInItsTodaysFolderWhereItComesBack) {
  const bc_id trip = AddTrip();
  for (bc_id tab : {21, 22, 23}) {
    const Left left = Give(tab);
    const Today saved = SavedToday(tab, left);
    EXPECT_EQ(saved.rows, (Rows{11, 12, 13, 21, 22, 23, 24})) << tab;
    EXPECT_EQ(RunOf(saved, trip), (Rows{21, 22, 23})) << tab;
    Return(tab, left);
  }
  // One that was in none is saved in none.
  const Left left = Give(24);
  const Today saved = SavedToday(24, left);
  EXPECT_EQ(saved.rows, (Rows{11, 12, 13, 21, 22, 23, 24}));
  EXPECT_TRUE(saved.folders.empty());
}

TEST_F(HandedBackTest, HandedTodayTabsOfOneTodaysFolderAreSavedInItInTheirOrder) {
  const bc_id trip = AddTrip();
  const Left first = Give(21), second = Give(22);
  const Today saved =
      TodayWith(*work_, {{.tab = 22, .node = second.node, .folder = second.today_folder},
                         {.tab = 21, .node = first.node, .folder = first.today_folder}});
  EXPECT_EQ(saved.rows, (Rows{11, 12, 13, 21, 22, 23, 24}));
  EXPECT_EQ(RunOf(saved, trip), (Rows{21, 22, 23}));
}

// The user moved the folder meanwhile: the tab's own place is where the folder was, and it's saved
// with the folder's tabs all the same, or a launch would find them apart and bring back none.
TEST_F(HandedBackTest, AHandedTodayTabIsSavedWithItsTodaysFolderWhereverThatIsNow) {
  const bc_id trip = AddTrip();
  const Left left = Give(22);
  work_->MoveFolder(trip, /*pinned=*/false, 0, 0, &Url);
  ASSERT_EQ(work_->today(), (Rows{21, 23, 11, 12, 13, 24}));
  const Today saved = SavedToday(22, left);
  EXPECT_EQ(RunOf(saved, trip).size(), 3u);
  EXPECT_EQ(saved.folders.at(22), trip);
  Return(22, left);
  EXPECT_EQ(work_->today(), saved.rows);
  EXPECT_EQ(work_->TodayFolderTabs(trip).size(), 3u);
}

TEST_F(HandedBackTest, AHandedTodayTabWhoseTodaysFolderIsGoneIsSavedLoose) {
  const bc_id trip = AddTrip();
  const Left left = Give(22);
  work_->DeleteFolder(trip);
  const Today saved = SavedToday(22, left);
  EXPECT_EQ(saved.rows, (Rows{11, 12, 13, 21, 22, 23, 24}));
  EXPECT_TRUE(saved.folders.empty());
}

// A tab that was in no today's folder, whose place is now between two tabs the user made a folder
// of meanwhile, comes back in that folder: the user's folder stays whole, and none is made.
TEST_F(HandedBackTest, ATodayTabComesBackIntoATodaysFolderMadeAroundItsPlaceMeanwhile) {
  for (bc_id tab : {21, 22, 23}) {
    work_->AddAt(tab, SIZE_MAX);
  }
  const Left left = Give(22);
  EXPECT_EQ(left.today_folder, 0u);
  const bc_id trip = work_->AddTodayFolder("Trip", {21, 23});
  ASSERT_TRUE(trip);
  Return(22, left);
  EXPECT_EQ(work_->today(), (Rows{11, 12, 13, 21, 22, 23}));
  EXPECT_EQ(work_->TodayFolderTabs(trip), (Rows{21, 22, 23}));
  EXPECT_EQ(work_->TodayFolders(), Rows{trip});
  EXPECT_TRUE(HasNode(22, left));
}

// And it's saved in that folder meanwhile, so after a quit it's there as after a return.
TEST_F(HandedBackTest, ATodayTabIsSavedInATodaysFolderMadeAroundItsPlaceMeanwhile) {
  for (bc_id tab : {21, 22, 23}) {
    work_->AddAt(tab, SIZE_MAX);
  }
  const Left left = Give(22);
  const bc_id trip = work_->AddTodayFolder("Trip", {21, 23});
  ASSERT_TRUE(trip);
  const Today saved = SavedToday(22, left);
  EXPECT_EQ(saved.rows, (Rows{11, 12, 13, 21, 22, 23}));
  EXPECT_EQ(RunOf(saved, trip), (Rows{21, 22, 23}));
  EXPECT_EQ(saved.folders.at(22), trip);
  // Beside the folder, not between two of its tabs, it's in none.
  const Left beside = Give(13);
  EXPECT_TRUE(SavedToday(13, beside).folders.empty());
}

}  // namespace
}  // namespace browser_core::handed_back
