#include "browser_core/pdf_read.h"

#include <string>
#include <vector>

#include "base/containers/span.h"
#include "base/strings/stringprintf.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

// A page with a line of text, and one with only a picture on it, as a scan is.
constexpr char kTextPage[] = "BT /F1 12 Tf 72 700 Td (Taxable income for the year: 41,250) Tj ET";
constexpr char kScanPage[] = "q 500 0 0 700 50 50 cm /Im1 Do Q";

// A US Letter PDF of |pages|, each a content stream, with the offsets PDFium reads, so nothing is
// rebuilt. |encrypt| is an encryption dictionary's entries, for a locked one.
std::string Pdf(const std::vector<std::string>& pages, const std::string& encrypt = "") {
  std::vector<std::string> objects = {
      "<< /Type /Catalog /Pages 2 0 R >>",
      "",  // The page tree, once the pages' numbers are known.
      "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
      // Two by two grey pixels, as hex.
      "<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray "
      "/BitsPerComponent 8 /Filter /ASCIIHexDecode /Length 9 >>\nstream\n00FFFF00>\nendstream",
  };
  std::string kids;
  for (const std::string& content : pages) {
    const size_t page = objects.size() + 1;
    kids += base::StringPrintf("%zu 0 R ", page);
    objects.push_back(base::StringPrintf(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R "
        ">> /XObject << /Im1 4 0 R >> >> /Contents %zu 0 R >>",
        page + 1));
    objects.push_back(base::StringPrintf("<< /Length %zu >>\nstream\n%s\nendstream", content.size(),
                                         content.c_str()));
  }
  objects[1] =
      base::StringPrintf("<< /Type /Pages /Kids [%s] /Count %zu >>", kids.c_str(), pages.size());
  std::string trailer;
  if (!encrypt.empty()) {
    objects.push_back("<< /Filter /Standard " + encrypt + " >>");
    trailer = base::StringPrintf(
        " /Encrypt %zu 0 R /ID [<00112233445566778899AABBCCDDEEFF> "
        "<00112233445566778899AABBCCDDEEFF>]",
        objects.size());
  }
  std::string pdf = "%PDF-1.7\n";
  std::vector<size_t> offsets;
  for (size_t i = 0; i < objects.size(); ++i) {
    offsets.push_back(pdf.size());
    pdf += base::StringPrintf("%zu 0 obj\n%s\nendobj\n", i + 1, objects[i].c_str());
  }
  const size_t xref = pdf.size();
  pdf += base::StringPrintf("xref\n0 %zu\n0000000000 65535 f \n", objects.size() + 1);
  for (size_t offset : offsets) {
    pdf += base::StringPrintf("%010zu 00000 n \n", offset);
  }
  pdf += base::StringPrintf("trailer\n<< /Size %zu /Root 1 0 R%s >>\nstartxref\n%zu\n%%%%EOF\n",
                            objects.size() + 1, trailer.c_str(), xref);
  return pdf;
}

PdfRead Read(const std::string& pdf, int first = 0, int count = kMostPdfPages) {
  return ReadPdf(base::as_byte_span(pdf), first, count);
}

bool IsJpeg(const std::vector<uint8_t>& picture) {
  return picture.size() > 2 && picture[0] == 0xFF && picture[1] == 0xD8;
}

// A page's text comes as PDFium reads it, with no picture, as the words are what's read.
TEST(PdfReadTest, APageWithTextIsReadAsText) {
  const PdfRead read = Read(Pdf({kTextPage}));
  ASSERT_EQ(read.result, PdfResult::kRead);
  EXPECT_EQ(read.page_count, 1);
  ASSERT_EQ(read.pages.size(), 1u);
  EXPECT_NE(read.pages[0].text.find("Taxable income for the year: 41,250"), std::string::npos);
  EXPECT_TRUE(read.pages[0].picture.empty());
}

// A page with no text layer, as a scan, is a picture to look at, as JPEG.
TEST(PdfReadTest, AScannedPageIsAPicture) {
  const PdfRead read = Read(Pdf({kScanPage}));
  ASSERT_EQ(read.result, PdfResult::kRead);
  ASSERT_EQ(read.pages.size(), 1u);
  EXPECT_TRUE(read.pages[0].text.empty());
  EXPECT_TRUE(IsJpeg(read.pages[0].picture));
}

// Each page is judged alone, in the document's order.
TEST(PdfReadTest, AMixedDocumentsPagesComeInOrder) {
  const PdfRead read = Read(Pdf({kTextPage, kScanPage, kTextPage}));
  ASSERT_EQ(read.result, PdfResult::kRead);
  ASSERT_EQ(read.pages.size(), 3u);
  EXPECT_TRUE(read.pages[0].picture.empty());
  EXPECT_TRUE(IsJpeg(read.pages[1].picture));
  EXPECT_TRUE(read.pages[2].picture.empty());
  EXPECT_FALSE(read.pages[2].text.empty());
}

// A read takes 30 pages at most, from where it's asked, and says how many the document has, so
// the thread can ask for the rest; past the end it reads none.
TEST(PdfReadTest, AReadTakesThirtyPagesFromWhereItsAsked) {
  const std::string pdf = Pdf(std::vector<std::string>(35, kTextPage));
  const PdfRead first = Read(pdf);
  ASSERT_EQ(first.result, PdfResult::kRead);
  EXPECT_EQ(first.page_count, 35);
  EXPECT_EQ(first.pages.size(), 30u);
  EXPECT_EQ(Read(pdf, 30, 30).pages.size(), 5u);
  EXPECT_EQ(Read(pdf, 10, 100).pages.size(), 30u);
  const PdfRead past = Read(pdf, 40, 30);
  EXPECT_EQ(past.result, PdfResult::kRead);
  EXPECT_TRUE(past.pages.empty());
  EXPECT_TRUE(Read(pdf, 2147483600, 30).pages.empty());
}

// One that needs a password is locked, and nothing tries one.
TEST(PdfReadTest, ALockedDocumentIsLocked) {
  const std::string garbled(64, 'A');
  const PdfRead read =
      Read(Pdf({kTextPage}, "/V 1 /R 2 /P -4 /O <" + garbled + "> /U <" + garbled + ">"));
  EXPECT_EQ(read.result, PdfResult::kLocked);
  EXPECT_TRUE(read.pages.empty());
}

TEST(PdfReadTest, WhatIsntAPdfIsBroken) {
  EXPECT_EQ(Read("%PDF-1.7\nnot a document").result, PdfResult::kBroken);
  EXPECT_EQ(Read("").result, PdfResult::kBroken);
  EXPECT_EQ(Read(Pdf({kTextPage}), -1, 1).result, PdfResult::kBroken);
}

// Only next to no text is a picture: a stamp's few letters are, a line of words isn't.
TEST(PdfReadTest, APictureIsForPagesWithNextToNoText) {
  EXPECT_TRUE(PageWantsPicture(u""));
  EXPECT_TRUE(PageWantsPicture(u"  PAID \r\n 12/03  "));
  EXPECT_TRUE(PageWantsPicture(u"abcdefg hijklmno"));
  EXPECT_FALSE(PageWantsPicture(u"abcdefgh ijklmnop"));
}

// 1568 px on the long side, or 200 dots an inch for a page too small to need that.
TEST(PdfReadTest, APicturesLongSideIs1568OrTwoHundredDotsAnInch) {
  EXPECT_EQ(PdfPictureSize(612, 792), gfx::Size(1212, 1568));
  EXPECT_EQ(PdfPictureSize(792, 612), gfx::Size(1568, 1212));
  EXPECT_EQ(PdfPictureSize(144, 72), gfx::Size(400, 200));
  EXPECT_TRUE(PdfPictureSize(0, 792).IsEmpty());
}

}  // namespace
}  // namespace browser_core
