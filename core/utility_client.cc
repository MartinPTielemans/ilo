#include "browser_core/utility_client.h"

#include <memory>
#include <optional>
#include <utility>
#include <vector>

#include "base/memory/read_only_shared_memory_region.h"
#include "base/memory/shared_memory_mapping.h"
#include "browser_core/pdf_read.h"
#include "browser_core/pdf_reader.mojom.h"
#include "mojo/public/cpp/base/big_buffer.h"
#include "mojo/public/cpp/bindings/pending_receiver.h"
#include "mojo/public/cpp/bindings/receiver.h"
#include "mojo/public/cpp/bindings/service_factory.h"

namespace browser_core {

namespace {

mojom::PdfReadResult ResultOf(PdfResult result) {
  switch (result) {
    case PdfResult::kRead:
      return mojom::PdfReadResult::kRead;
    case PdfResult::kLocked:
      return mojom::PdfReadResult::kLocked;
    case PdfResult::kBroken:
      return mojom::PdfReadResult::kBroken;
  }
  return mojom::PdfReadResult::kBroken;
}

// The reader's side of bc_pdf_read, in its utility process: one read a process, which the browser
// ends as it answers.
class PdfReaderService : public mojom::PdfReader {
 public:
  explicit PdfReaderService(mojo::PendingReceiver<mojom::PdfReader> receiver)
      : receiver_(this, std::move(receiver)) {}
  PdfReaderService(const PdfReaderService&) = delete;
  PdfReaderService& operator=(const PdfReaderService&) = delete;
  ~PdfReaderService() override = default;

  // mojom::PdfReader:
  void Read(base::ReadOnlySharedMemoryRegion pdf,
            int32_t first,
            int32_t count,
            ReadCallback callback) override {
    base::ReadOnlySharedMemoryMapping mapping = pdf.Map();
    if (!mapping.IsValid()) {
      std::move(callback).Run(mojom::PdfReadResult::kBroken, 0, {});
      return;
    }
    PdfRead read = ReadPdf(mapping.GetMemoryAsSpan<uint8_t>(), first, count);
    std::vector<mojom::PdfPagePtr> pages;
    for (PdfPageRead& page : read.pages) {
      std::optional<mojo_base::BigBuffer> picture;
      if (!page.picture.empty()) {
        picture.emplace(base::span<const uint8_t>(page.picture));
      }
      pages.push_back(mojom::PdfPage::New(std::move(page.text), std::move(picture)));
    }
    std::move(callback).Run(ResultOf(read.result), read.page_count, std::move(pages));
  }

 private:
  mojo::Receiver<mojom::PdfReader> receiver_;
};

auto RunPdfReader(mojo::PendingReceiver<mojom::PdfReader> receiver) {
  return std::make_unique<PdfReaderService>(std::move(receiver));
}

}  // namespace

UtilityClient::UtilityClient() = default;

UtilityClient::~UtilityClient() = default;

void UtilityClient::RegisterMainThreadServices(mojo::ServiceFactory& services) {
  ChromeContentUtilityClient::RegisterMainThreadServices(services);
  services.Add(RunPdfReader);
}

}  // namespace browser_core
