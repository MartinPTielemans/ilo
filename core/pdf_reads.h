#ifndef BROWSER_CORE_PDF_READS_H_
#define BROWSER_CORE_PDF_READS_H_

#include <cstddef>
#include <cstdint>
#include <string_view>

#include "base/containers/span.h"
#include "browser_core/public/browser_core.h"

namespace browser_core {

// The browser's side of bc_pdf_read (#4367): each read starts a reader, a utility process of its
// own that ends as it answers, so no reader is left running while nothing reads.
bc_id StartPdfRead(base::span<const uint8_t> bytes, int first, int count);
// The read's |index|th page's text and picture: empty before its answer, past what it read, or for
// a page with no picture.
std::string_view PdfPageText(bc_id read, int index);
base::span<const uint8_t> PdfPagePicture(bc_id read, int index);
void DropPdfRead(bc_id read);

}  // namespace browser_core

#endif  // BROWSER_CORE_PDF_READS_H_
