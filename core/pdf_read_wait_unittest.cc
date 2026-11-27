#include "browser_core/pdf_read_wait.h"

#include <memory>
#include <optional>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/memory/read_only_shared_memory_region.h"
#include "base/test/task_environment.h"
#include "browser_core/mojo_once_for_testing.h"
#include "browser_core/pdf_reader.mojom.h"
#include "mojo/public/cpp/bindings/pending_receiver.h"
#include "mojo/public/cpp/bindings/receiver.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using mojom::PdfReadResult;

// A reader in its process: it holds the read until the test answers it, or goes as a crash
// would take its process.
class Reader : public mojom::PdfReader {
 public:
  explicit Reader(mojo::PendingReceiver<mojom::PdfReader> receiver)
      : receiver_(this, std::move(receiver)) {
    receiver_.set_disconnect_handler(base::BindOnce([](bool* left) { *left = true; }, &left_));
  }

  void Read(base::ReadOnlySharedMemoryRegion, int32_t, int32_t, ReadCallback answer) override {
    answer_ = std::move(answer);
  }

  void Answer(PdfReadResult result, int32_t page_count, std::vector<mojom::PdfPagePtr> pages) {
    std::move(answer_).Run(result, page_count, std::move(pages));
  }

  void Crash() {
    receiver_.reset();
    answer_.Reset();
  }

  bool asked() const { return !answer_.is_null(); }
  // Whether the read let it go, which ends its process.
  bool left() const { return left_; }

 private:
  mojo::Receiver<mojom::PdfReader> receiver_;
  ReadCallback answer_;
  bool left_ = false;
};

struct Heard {
  PdfReadTime time;
  PdfReadResult result;
  int32_t page_count;
  size_t pages;
};

class PdfReadWaitTest : public testing::Test {
 protected:
  static void SetUpTestSuite() { StartMojoOnceForTesting(); }

  void Start() {
    mojo::PendingRemote<mojom::PdfReader> remote;
    reader_ = std::make_unique<Reader>(remote.InitWithNewPipeAndPassReceiver());
    wait_ = std::make_unique<PdfReadWait>(
        std::move(remote), base::ReadOnlySharedMemoryRegion::Create(8).region, 0, 30,
        base::BindOnce(
            [](std::vector<Heard>* heard, PdfReadTime time, PdfReadResult result,
               int32_t page_count, std::vector<mojom::PdfPagePtr> pages) {
              heard->push_back({time, result, page_count, pages.size()});
            },
            &heard_));
    task_environment_.RunUntilIdle();
    ASSERT_TRUE(reader_->asked());
  }

  base::test::TaskEnvironment task_environment_{base::test::TaskEnvironment::TimeSource::MOCK_TIME};
  std::unique_ptr<Reader> reader_;
  std::unique_ptr<PdfReadWait> wait_;
  std::vector<Heard> heard_;
};

// A reader that never answers, as a hung one, doesn't keep the read waiting: at its deadline it's
// timed out, with no pages, and the reader's process is let go.
TEST_F(PdfReadWaitTest, AReadItsReaderNeverAnswersTimesOutAtItsDeadline) {
  Start();
  task_environment_.FastForwardBy(kPdfReadDeadline - base::Milliseconds(1));
  EXPECT_TRUE(heard_.empty());
  task_environment_.FastForwardBy(base::Milliseconds(1));
  ASSERT_EQ(heard_.size(), 1u);
  EXPECT_EQ(heard_[0].time, PdfReadTime::kTimedOut);
  EXPECT_EQ(heard_[0].result, PdfReadResult::kBroken);
  EXPECT_EQ(heard_[0].page_count, 0);
  EXPECT_EQ(heard_[0].pages, 0u);
  task_environment_.RunUntilIdle();
  EXPECT_TRUE(reader_->left());
}

// A reader whose process goes, a crash too, breaks the read at once, in time, and the deadline
// adds no second answer.
TEST_F(PdfReadWaitTest, AReadWhoseReaderGoesIsBrokenAtOnce) {
  Start();
  reader_->Crash();
  task_environment_.RunUntilIdle();
  ASSERT_EQ(heard_.size(), 1u);
  EXPECT_EQ(heard_[0].time, PdfReadTime::kInTime);
  EXPECT_EQ(heard_[0].result, PdfReadResult::kBroken);
  task_environment_.FastForwardBy(kPdfReadDeadline);
  EXPECT_EQ(heard_.size(), 1u);
}

// An answer in time is the reader's, its pages too, and the deadline adds none.
TEST_F(PdfReadWaitTest, AnAnswerInTimeIsTheReadersAlone) {
  Start();
  std::vector<mojom::PdfPagePtr> pages;
  pages.push_back(mojom::PdfPage::New("Annual statement", std::nullopt));
  pages.push_back(mojom::PdfPage::New("Page two", std::nullopt));
  reader_->Answer(PdfReadResult::kRead, 7, std::move(pages));
  task_environment_.RunUntilIdle();
  ASSERT_EQ(heard_.size(), 1u);
  EXPECT_EQ(heard_[0].time, PdfReadTime::kInTime);
  EXPECT_EQ(heard_[0].result, PdfReadResult::kRead);
  EXPECT_EQ(heard_[0].page_count, 7);
  EXPECT_EQ(heard_[0].pages, 2u);
  EXPECT_TRUE(reader_->left());
  task_environment_.FastForwardBy(kPdfReadDeadline);
  EXPECT_EQ(heard_.size(), 1u);
}

// A read dropped before its answer answers nothing here, and lets its reader go.
TEST_F(PdfReadWaitTest, ADroppedReadAnswersNothingAndLetsItsReaderGo) {
  Start();
  wait_.reset();
  task_environment_.FastForwardBy(kPdfReadDeadline);
  EXPECT_TRUE(heard_.empty());
  EXPECT_TRUE(reader_->left());
}

}  // namespace
}  // namespace browser_core
