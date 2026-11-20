#include "browser_core/quit_stores_wait.h"

#include <optional>
#include <string>
#include <utility>

#include "base/functional/bind.h"
#include "base/test/task_environment.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

class QuitStoresWaitTest : public testing::Test {
 protected:
  // Starts a wait whose engine answers through |answer|, kept to answer later.
  void Start() {
    WaitForStores(base::BindOnce(
                      [](base::OnceCallback<void(StoresWritten)>* kept,
                         base::OnceCallback<void(StoresWritten)> answer) {
                        *kept = std::move(answer);
                      },
                      &answer_),
                  base::BindOnce(&QuitStoresWaitTest::Ended, base::Unretained(this)));
  }

  void Ended(std::optional<std::string> line) {
    ++ends_;
    line_ = std::move(line);
  }

  base::test::TaskEnvironment task_environment_{
      base::test::TaskEnvironment::TimeSource::MOCK_TIME};
  base::OnceCallback<void(StoresWritten)> answer_;
  int ends_ = 0;
  std::optional<std::string> line_;
};

TEST_F(QuitStoresWaitTest, AQuitWithNoStoreWaitsForNothingAndSaysNothing) {
  WaitForStores(base::BindOnce([](base::OnceCallback<void(StoresWritten)> answer) {
                  std::move(answer).Run(StoresWritten::kNone);
                }),
                base::BindOnce(&QuitStoresWaitTest::Ended, base::Unretained(this)));
  EXPECT_EQ(ends_, 1);
  EXPECT_FALSE(line_);
  // Nothing left to run: no limit was posted.
  EXPECT_EQ(task_environment_.GetPendingMainThreadTaskCount(), 0u);
}

TEST_F(QuitStoresWaitTest, TheEngineAnsweringFirstEndsTheWaitWithItsTime) {
  Start();
  EXPECT_EQ(ends_, 0);
  task_environment_.FastForwardBy(base::Milliseconds(40));
  std::move(answer_).Run(StoresWritten::kWritten);
  EXPECT_EQ(ends_, 1);
  EXPECT_EQ(line_, "system stores written at 40 ms");
  // The limit passing after changes nothing.
  task_environment_.FastForwardBy(kStoresWriteLimit);
  EXPECT_EQ(ends_, 1);
}

TEST_F(QuitStoresWaitTest, TheLimitEndsAWaitTheEngineNeverAnswers) {
  Start();
  task_environment_.FastForwardBy(kStoresWriteLimit - base::Milliseconds(1));
  EXPECT_EQ(ends_, 0);
  task_environment_.FastForwardBy(base::Milliseconds(1));
  EXPECT_EQ(ends_, 1);
  EXPECT_EQ(line_, "system stores not written, gave up at 1000 ms");
  // A late answer changes nothing.
  std::move(answer_).Run(StoresWritten::kWritten);
  EXPECT_EQ(ends_, 1);
  EXPECT_EQ(line_, "system stores not written, gave up at 1000 ms");
}

TEST_F(QuitStoresWaitTest, AnEngineWithNoWayToBeAskedSaysSoAtOnce) {
  WaitForStores(base::BindOnce([](base::OnceCallback<void(StoresWritten)> answer) {
                  std::move(answer).Run(StoresWritten::kNoWay);
                }),
                base::BindOnce(&QuitStoresWaitTest::Ended, base::Unretained(this)));
  EXPECT_EQ(ends_, 1);
  EXPECT_EQ(line_, "system stores not written, no way to write them");
  EXPECT_EQ(task_environment_.GetPendingMainThreadTaskCount(), 0u);
}

}  // namespace
}  // namespace browser_core
