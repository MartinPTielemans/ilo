#include "browser_core/pdf_read.h"

#include <algorithm>
#include <cmath>
#include <optional>

#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "third_party/pdfium/public/cpp/fpdf_scopers.h"
#include "third_party/pdfium/public/fpdf_text.h"
#include "third_party/pdfium/public/fpdfview.h"
#include "third_party/skia/include/core/SkBitmap.h"
#include "third_party/skia/include/core/SkImageInfo.h"
#include "ui/gfx/codec/jpeg_codec.h"

namespace browser_core {

namespace {

constexpr size_t kFewestCharacters = 16;
constexpr float kLongestSide = 1568;
constexpr float kMostDotsAnInch = 200;
constexpr float kPointsAnInch = 72;
constexpr int kJpegQuality = 80;

// Once a process: the reader's is its own, so nothing else there sets PDFium up. No V8 isolate, so
// a document's JavaScript never runs.
void InitializePdfium() {
  static const bool initialized = [] {
    FPDF_LIBRARY_CONFIG config = {};
    config.version = 2;
    FPDF_InitLibraryWithConfig(&config);
    return true;
  }();
  (void)initialized;
}

std::u16string PageText(FPDF_PAGE page) {
  ScopedFPDFTextPage text(FPDFText_LoadPage(page));
  const int count = text ? FPDFText_CountChars(text.get()) : 0;
  if (count <= 0) {
    return std::u16string();
  }
  // UTF-16LE, with a NUL after, which the count it returns takes in.
  std::u16string buffer(static_cast<size_t>(count) + 1, u'\0');
  const int written =
      FPDFText_GetText(text.get(), 0, count, reinterpret_cast<unsigned short*>(buffer.data()));
  buffer.resize(written > 0 ? static_cast<size_t>(written) - 1 : 0);
  return buffer;
}

std::vector<uint8_t> PagePicture(FPDF_PAGE page) {
  const gfx::Size size = PdfPictureSize(FPDF_GetPageWidthF(page), FPDF_GetPageHeightF(page));
  if (size.IsEmpty()) {
    return {};
  }
  SkBitmap bitmap;
  if (!bitmap.tryAllocPixels(SkImageInfo::Make(size.width(), size.height(), kBGRA_8888_SkColorType,
                                               kOpaque_SkAlphaType))) {
    return {};
  }
  // PDFium draws into the bitmap's own pixels, white first, as paper is.
  ScopedFPDFBitmap target(FPDFBitmap_CreateEx(size.width(), size.height(), FPDFBitmap_BGRx,
                                              bitmap.getPixels(),
                                              static_cast<int>(bitmap.rowBytes())));
  if (!target) {
    return {};
  }
  FPDFBitmap_FillRect(target.get(), 0, 0, size.width(), size.height(), 0xFFFFFFFF);
  FPDF_RenderPageBitmap(target.get(), page, 0, 0, size.width(), size.height(), /*rotate=*/0,
                        FPDF_ANNOT);
  return gfx::JPEGCodec::Encode(bitmap, kJpegQuality).value_or(std::vector<uint8_t>());
}

}  // namespace

bool PageWantsPicture(std::u16string_view text) {
  size_t shown = 0;
  for (char16_t character : text) {
    if (!base::IsUnicodeWhitespace(character) && ++shown >= kFewestCharacters) {
      return false;
    }
  }
  return true;
}

gfx::Size PdfPictureSize(float width, float height) {
  if (!(width > 0) || !(height > 0)) {
    return gfx::Size();
  }
  const float scale =
      std::min(kLongestSide / std::max(width, height), kMostDotsAnInch / kPointsAnInch);
  return gfx::Size(std::max(1, static_cast<int>(std::lround(width * scale))),
                   std::max(1, static_cast<int>(std::lround(height * scale))));
}

PdfRead ReadPdf(base::span<const uint8_t> bytes, int first, int count) {
  PdfRead read;
  if (bytes.empty() || first < 0 || count < 1) {
    return read;
  }
  InitializePdfium();
  ScopedFPDFDocument document(FPDF_LoadMemDocument64(bytes.data(), bytes.size(), nullptr));
  if (!document) {
    read.result =
        FPDF_GetLastError() == FPDF_ERR_PASSWORD ? PdfResult::kLocked : PdfResult::kBroken;
    return read;
  }
  read.result = PdfResult::kRead;
  read.page_count = std::max(0, FPDF_GetPageCount(document.get()));
  // In 64 bits, as a |first| near the largest int would overflow.
  const int end = static_cast<int>(
      std::min<int64_t>(int64_t{first} + std::min(count, kMostPdfPages), int64_t{read.page_count}));
  for (int index = first; index < end; ++index) {
    PdfPageRead page_read;
    ScopedFPDFPage page(FPDF_LoadPage(document.get(), index));
    if (page) {
      const std::u16string text = PageText(page.get());
      if (PageWantsPicture(text)) {
        page_read.picture = PagePicture(page.get());
      }
      page_read.text = base::UTF16ToUTF8(text);
    }
    read.pages.push_back(std::move(page_read));
  }
  return read;
}

}  // namespace browser_core
