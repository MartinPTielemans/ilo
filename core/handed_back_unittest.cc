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
    Left left{.node = *work_->NodeOf(tab), .split = work_->Panes(tab), .pin = PinOf(*work_, tab)};
    if (left.split.size() < 2) {
      left.split.clear();
    }
    work_->Remove(tab);
    return left;
  }

  // |tab| comes back to Work as a returned tab does: at the top of Today, then put back.
  void Return(bc_id tab, const Left& left) {
    work_->AddAt(tab, 0);
    Put(*work_, tab, left.node, left.split, left.pin);
  }

  // Whether |tab| has the node it left with, its id and its place.
  bool HasNode(bc_id tab, const Left& left) const {
    const Space::Node* node = work_->NodeOf(tab);
    return node && node->id == left.node.id && node->position.Equals(left.node.position);
  }

  // Each of Work's rows with its node, its id and its place, as text to compare.
  std::map<bc_id, std::string> Nodes() const {
    std::map<bc_id, std::string> nodes;
    for (const Rows* rows : {&work_->pinned_rows(), &work_->today()}) {
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

}  // namespace
}  // namespace browser_core::handed_back
