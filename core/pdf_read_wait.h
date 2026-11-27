#ifndef BROWSER_CORE_PDF_READ_WAIT_H_
#define BROWSER_CORE_PDF_READ_WAIT_H_

#include <cstdint>
#include <vector>

#include "base/functional/callback.h"
#include "base/memory/read_only_shared_memory_region.h"
#include "base/memory/weak_ptr.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "browser_core/pdf_reader.mojom.h"
#include "mojo/public/cpp/bindings/pending_remote.h"
#include "mojo/public/cpp/bindings/remote.h"

namespace browser_core {

// How long a read may take, its reader's start included. A reader still reading then is taken
// as stuck, so the read never waits on it for good.
inline constexpr base::TimeDelta kPdfReadDeadline = base::Seconds(60);

// Whether a read ended before its deadline, or the deadline passed first.
enum class PdfReadTime { kInTime, kTimedOut };

// One read in its reader (bc_pdf_read, #4367), apart so browser_core_unittests tests its ending
// alone. It answers once: in time with what the reader read, or kBroken with no pages as the
// reader's process goes, a crash in it too; or timed out, kBroken with no pages, as the deadline
// passes first. Its remote goes as it answers, which ends the reader's process, and as it's
// destroyed, which then answers nothing.
class PdfReadWait {
 public:
  using Done = base::OnceCallback<void(PdfReadTime time,
                                       mojom::PdfReadResult result,
                                       int32_t page_count,
                                       std::vector<mojom::PdfPagePtr> pages)>;

  PdfReadWait(mojo::PendingRemote<mojom::PdfReader> reader,
              base::ReadOnlySharedMemoryRegion pdf,
              int first,
              int count,
              Done done);
  PdfReadWait(const PdfReadWait&) = delete;
  PdfReadWait& operator=(const PdfReadWait&) = delete;
  ~PdfReadWait();

 private:
  void Answer(PdfReadTime time,
              mojom::PdfReadResult result,
              int32_t page_count,
              std::vector<mojom::PdfPagePtr> pages);
  void Answered(mojom::PdfReadResult result,
                int32_t page_count,
                std::vector<mojom::PdfPagePtr> pages);
  void Broken();
  void TimedOut();

  mojo::Remote<mojom::PdfReader> reader_;
  base::OneShotTimer deadline_;
  Done done_;
  base::WeakPtrFactory<PdfReadWait> weak_{this};
};

}  // namespace browser_core

#endif  // BROWSER_CORE_PDF_READ_WAIT_H_
