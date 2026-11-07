#include "browser_core/agent_download_hold.h"

#include "base/files/file_path.h"
#include "base/test/mock_callback.h"
#include "components/download/public/common/mock_download_item.h"
#include "testing/gmock/include/gmock/gmock.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using ::testing::NiceMock;
using ::testing::Return;
using ::testing::ReturnRefOfCopy;

constexpr base::FilePath::CharType kHalfway[] =
    FILE_PATH_LITERAL("/downloads/report.docx.crdownload");

// A download the user starts, or any not an agent's, is never held.
TEST(AgentDownloadHoldTest, HoldsOnlyAnAgentsDownload) {
  NiceMock<download::MockDownloadItem> item;
  base::MockOnceClosure again;
  EXPECT_CALL(again, Run).Times(0);
  EXPECT_FALSE(HoldCompletion(&item, again.Get()));
}

// A small file comes whole before the user answers: its completion waits, and the yes lets it
// complete, asking Chrome again, after which nothing holds it.
TEST(AgentDownloadHoldTest, AYesCompletesAFileThatCameWhileItAsked) {
  NiceMock<download::MockDownloadItem> item;
  MarkAgentDownload(&item, "claude-code", 20);
  base::MockOnceClosure again;
  EXPECT_CALL(again, Run).Times(0);
  EXPECT_TRUE(HoldCompletion(&item, again.Get()));
  ::testing::Mock::VerifyAndClearExpectations(&again);

  EXPECT_CALL(item, Resume(true));
  EXPECT_CALL(item, GetState()).WillRepeatedly(Return(download::DownloadItem::IN_PROGRESS));
  EXPECT_CALL(again, Run);
  AllowAgentDownload(&item);
  base::MockOnceClosure later;
  EXPECT_CALL(later, Run).Times(0);
  EXPECT_FALSE(HoldCompletion(&item, later.Get()));
}

// A no cancels a file that came whole while it asked, whose completion never runs, and takes it
// off the list, which leaves nothing on disk.
TEST(AgentDownloadHoldTest, ANoCancelsAFileThatCameWhileItAsked) {
  NiceMock<download::MockDownloadItem> item;
  MarkAgentDownload(&item, "claude-code", 20);
  base::MockOnceClosure again;
  EXPECT_CALL(again, Run).Times(0);
  EXPECT_TRUE(HoldCompletion(&item, again.Get()));
  EXPECT_CALL(item, Resume).Times(0);
  EXPECT_CALL(item, Cancel(true));
  EXPECT_CALL(item, Remove());
  RefuseAgentDownload(&item);
}

// Chrome lets no download be paused before it's put where it goes: it must enter IN_PROGRESS
// unpaused. So an agent's waits, unpaused, until its file has a path, and then asks to pause once.
// The update itself never pauses it, as a pause inside an update tells the observers again.
TEST(AgentDownloadHoldTest, AsksToPauseOnlyOnceItsTargetIsResolved) {
  NiceMock<download::MockDownloadItem> item;
  MarkAgentDownload(&item, "claude-code", 20);
  EXPECT_CALL(item, Pause()).Times(0);
  ON_CALL(item, GetState()).WillByDefault(Return(download::DownloadItem::IN_PROGRESS));
  ON_CALL(item, GetFullPath()).WillByDefault(ReturnRefOfCopy(base::FilePath()));
  EXPECT_FALSE(ResolvedToPause(&item));
  ON_CALL(item, GetFullPath()).WillByDefault(ReturnRefOfCopy(base::FilePath(kHalfway)));
  EXPECT_TRUE(ResolvedToPause(&item));
  EXPECT_FALSE(ResolvedToPause(&item));
  ::testing::Mock::VerifyAndClearExpectations(&item);

  ON_CALL(item, GetState()).WillByDefault(Return(download::DownloadItem::IN_PROGRESS));
  EXPECT_CALL(item, Pause()).Times(1);
  PauseHeld(&item);
}

// A download the user starts is never paused, nor one the user allowed, nor one that has ended by
// the time its pause comes.
TEST(AgentDownloadHoldTest, NeverPausesTheUsersOneAllowedOrOneEnded) {
  NiceMock<download::MockDownloadItem> users;
  ON_CALL(users, GetState()).WillByDefault(Return(download::DownloadItem::IN_PROGRESS));
  ON_CALL(users, GetFullPath()).WillByDefault(ReturnRefOfCopy(base::FilePath(kHalfway)));
  EXPECT_CALL(users, Pause()).Times(0);
  EXPECT_FALSE(ResolvedToPause(&users));
  PauseHeld(&users);

  NiceMock<download::MockDownloadItem> allowed;
  MarkAgentDownload(&allowed, "claude-code", 20);
  ON_CALL(allowed, GetState()).WillByDefault(Return(download::DownloadItem::IN_PROGRESS));
  ON_CALL(allowed, GetFullPath()).WillByDefault(ReturnRefOfCopy(base::FilePath(kHalfway)));
  EXPECT_TRUE(ResolvedToPause(&allowed));
  AllowAgentDownload(&allowed);
  EXPECT_CALL(allowed, Pause()).Times(0);
  PauseHeld(&allowed);

  NiceMock<download::MockDownloadItem> ended;
  MarkAgentDownload(&ended, "claude-code", 20);
  ON_CALL(ended, GetState()).WillByDefault(Return(download::DownloadItem::CANCELLED));
  EXPECT_CALL(ended, Pause()).Times(0);
  PauseHeld(&ended);
}

// Its agent and tab, for its report, until it ends.
TEST(AgentDownloadHoldTest, KnowsItsAgentUntilItEnds) {
  NiceMock<download::MockDownloadItem> item;
  std::string agent;
  uint64_t tab = 0;
  EXPECT_FALSE(AgentDownloadOf(&item, &agent, &tab));
  MarkAgentDownload(&item, "claude-code", 20);
  ASSERT_TRUE(AgentDownloadOf(&item, &agent, &tab));
  EXPECT_EQ(agent, "claude-code");
  EXPECT_EQ(tab, 20u);
  ForgetAgentDownload(&item);
  EXPECT_FALSE(AgentDownloadOf(&item, &agent, &tab));
}

}  // namespace
}  // namespace browser_core
