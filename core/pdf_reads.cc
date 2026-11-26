#include "browser_core/pdf_reads.h"

#include <map>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/memory/read_only_shared_memory_region.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/pdf_read.h"
#include "browser_core/pdf_reader.mojom.h"
#include "content/public/browser/service_process_host.h"
#include "mojo/public/cpp/bindings/remote.h"

namespace browser_core {

namespace {

// A read's bytes are copied once, into memory its reader maps read-only.
constexpr size_t kMostBytes = size_t{256} << 20;

static_assert(kMostPdfPages == BC_PDF_MOST_PAGES);

struct Read {
  mojo::Remote<mojom::PdfReader> reader;
  bool answered = false;
  int result = BC_PDF_BROKEN;
  std::vector<mojom::PdfPagePtr> pages;
};

std::map<bc_id, std::unique_ptr<Read>>& Reads() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<Read>>> reads;
  return *reads;
}

bc_id g_next_read = 1;

int ResultOf(mojom::PdfReadResult result) {
  switch (result) {
    case mojom::PdfReadResult::kRead:
      return BC_PDF_READ;
    case mojom::PdfReadResult::kLocked:
      return BC_PDF_LOCKED;
    case mojom::PdfReadResult::kBroken:
      return BC_PDF_BROKEN;
  }
  return BC_PDF_BROKEN;
}

void Tell(bc_id id, int result, int page_count, int pages_read) {
  if (Host().pdf_read_done) {
    Host().pdf_read_done(Host().context, id, result, page_count, pages_read);
  }
}

// The reader answered, or its process went, a crash in it too, which is BROKEN: the read keeps
// what it read, and its process ends.
void Answered(bc_id id,
              mojom::PdfReadResult result,
              int32_t page_count,
              std::vector<mojom::PdfPagePtr> pages) {
  auto found = Reads().find(id);
  if (found == Reads().end() || found->second->answered) {
    return;
  }
  Read& read = *found->second;
  read.answered = true;
  read.reader.reset();
  read.result = ResultOf(result);
  if (read.result == BC_PDF_READ) {
    read.pages = std::move(pages);
  } else {
    page_count = 0;
  }
  const int pages_read = static_cast<int>(read.pages.size());
  LOG(ERROR) << "browser: pdf read " << id << " ended " << read.result << ", " << pages_read
             << " of " << page_count << " pages";
  Tell(id, read.result, page_count, pages_read);
}

const mojom::PdfPage* PageOf(bc_id id, int index) {
  auto found = Reads().find(id);
  if (found == Reads().end() || index < 0 ||
      static_cast<size_t>(index) >= found->second->pages.size()) {
    return nullptr;
  }
  return found->second->pages[index].get();
}

}  // namespace

bc_id StartPdfRead(base::span<const uint8_t> bytes, int first, int count) {
  if (bytes.empty() || bytes.size() > kMostBytes || first < 0 || count < 1 ||
      count > kMostPdfPages) {
    return 0;
  }
  base::MappedReadOnlyRegion memory = base::ReadOnlySharedMemoryRegion::Create(bytes.size());
  if (!memory.IsValid()) {
    return 0;
  }
  memory.mapping.GetMemoryAsSpan<uint8_t>().copy_from(bytes);
  const bc_id id = g_next_read++;
  auto read = std::make_unique<Read>();
  read->reader = content::ServiceProcessHost::Launch<mojom::PdfReader>(
      content::ServiceProcessHost::Options().WithDisplayName(u"PDF Reader").Pass());
  read->reader.set_disconnect_handler(base::BindOnce(&Answered, id, mojom::PdfReadResult::kBroken,
                                                     0, std::vector<mojom::PdfPagePtr>()));
  read->reader->Read(std::move(memory.region), first, count, base::BindOnce(&Answered, id));
  Reads()[id] = std::move(read);
  LOG(ERROR) << "browser: pdf read " << id << " started, pages " << first << " to "
             << first + count - 1;
  return id;
}

std::string_view PdfPageText(bc_id id, int index) {
  const mojom::PdfPage* page = PageOf(id, index);
  return page ? std::string_view(page->text) : std::string_view();
}

base::span<const uint8_t> PdfPagePicture(bc_id id, int index) {
  const mojom::PdfPage* page = PageOf(id, index);
  return page && page->picture ? base::span<const uint8_t>(*page->picture)
                               : base::span<const uint8_t>();
}

void DropPdfRead(bc_id id) {
  auto found = Reads().find(id);
  if (found == Reads().end()) {
    return;
  }
  const bool answered = found->second->answered;
  // Its remote goes with it, which ends a reader still reading.
  Reads().erase(found);
  if (!answered) {
    // Posted, so the app never hears inside its own call.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Tell, id, BC_PDF_CLOSED, 0, 0));
  }
}

}  // namespace browser_core
