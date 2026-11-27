#include "browser_core/pdf_read_wait.h"

#include <utility>

#include "base/functional/bind.h"
#include "base/location.h"

namespace browser_core {

PdfReadWait::PdfReadWait(mojo::PendingRemote<mojom::PdfReader> reader,
                         base::ReadOnlySharedMemoryRegion pdf,
                         int first,
                         int count,
                         Done done)
    : reader_(std::move(reader)), done_(std::move(done)) {
  reader_.set_disconnect_handler(base::BindOnce(&PdfReadWait::Broken, weak_.GetWeakPtr()));
  deadline_.Start(FROM_HERE, kPdfReadDeadline,
                  base::BindOnce(&PdfReadWait::TimedOut, weak_.GetWeakPtr()));
  reader_->Read(std::move(pdf), first, count,
                base::BindOnce(&PdfReadWait::Answered, weak_.GetWeakPtr()));
}

PdfReadWait::~PdfReadWait() = default;

void PdfReadWait::Answer(PdfReadTime time,
                         mojom::PdfReadResult result,
                         int32_t page_count,
                         std::vector<mojom::PdfPagePtr> pages) {
  if (!done_) {
    return;
  }
  reader_.reset();
  deadline_.Stop();
  weak_.InvalidateWeakPtrs();
  // Last, as what it runs may destroy this.
  std::move(done_).Run(time, result, page_count, std::move(pages));
}

void PdfReadWait::Answered(mojom::PdfReadResult result,
                           int32_t page_count,
                           std::vector<mojom::PdfPagePtr> pages) {
  Answer(PdfReadTime::kInTime, result, page_count, std::move(pages));
}

void PdfReadWait::Broken() {
  Answer(PdfReadTime::kInTime, mojom::PdfReadResult::kBroken, 0, {});
}

void PdfReadWait::TimedOut() {
  Answer(PdfReadTime::kTimedOut, mojom::PdfReadResult::kBroken, 0, {});
}

}  // namespace browser_core
