#include "browser_core/space.h"

#include <map>
#include <string>
#include <vector>

#include "base/test/task_environment.h"
#include "browser_core/auto_archive.h"
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

}  // namespace
}  // namespace browser_core
