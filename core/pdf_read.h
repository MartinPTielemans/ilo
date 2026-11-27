#ifndef BROWSER_CORE_PDF_READ_H_
#define BROWSER_CORE_PDF_READ_H_

#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

#include "base/containers/span.h"
#include "ui/gfx/geometry/size.h"

namespace browser_core {

// A PDF's pages as a thread reads them (bc_pdf_read, #4367): each page's text by PDFium, and a
// picture of a page with next to none, as a scan's. It runs only in the core's reader, a sandboxed
// utility process of its own, as PDFium parses what any file holds; apart so
// browser_core_unittests tests the reading alone.

// The most pages a read reads, BC_PDF_MOST_PAGES.
inline constexpr int kMostPdfPages = 30;

enum class PdfResult { kRead, kLocked, kBroken };

struct PdfPageRead {
  std::string text;
  // The page as JPEG, for one PageWantsPicture; else empty.
  std::vector<uint8_t> picture;
};

struct PdfRead {
  PdfResult result = PdfResult::kBroken;
  int page_count = 0;
  // From |first|, at most |count| of them, fewer past the document's end.
  std::vector<PdfPageRead> pages;
};

// Reads |bytes|' pages |first| (from 0) on, |count| of them, kMostPdfPages at most. No script runs
// and nothing is fetched: PDFium is given no V8 and no loader.
PdfRead ReadPdf(base::span<const uint8_t> bytes, int first, int count);

// Whether a page whose text is |text| gets a picture: under 16 characters but white space, as a
// scan's, or a photo's, with no text layer.
bool PageWantsPicture(std::u16string_view text);

// A page picture's size for a page |width| by |height| points: 1568 px on its long side, as large
// as a model reads a picture whole, but never over 200 dots an inch for a small page.
gfx::Size PdfPictureSize(float width, float height);

}  // namespace browser_core

#endif  // BROWSER_CORE_PDF_READ_H_
