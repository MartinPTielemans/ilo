#include "browser_core/board_store.h"

#include <algorithm>
#include <array>
#include <utility>

#include "base/files/file_util.h"
#include "base/files/important_file_writer.h"
#include "base/memory/ptr_util.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"

namespace browser_core::board {

namespace {

// The bytes Sniff reads: WebP's is the longest, "RIFF", a size, "WEBP".
constexpr size_t kSniffed = 12;

bool IsLowerHex(std::string_view text, size_t size) {
  if (text.size() != size) {
    return false;
  }
  for (char c : text) {
    if (!base::IsAsciiDigit(c) && !(c >= 'a' && c <= 'f')) {
      return false;
    }
  }
  return true;
}

const char* Extension(Image image) {
  switch (image) {
    case Image::kPng:
      return "png";
    case Image::kJpeg:
      return "jpg";
    case Image::kGif:
      return "gif";
    case Image::kWebp:
      return "webp";
    case Image::kNone:
      return "";
  }
  return "";
}

bool StartsWith(base::span<const uint8_t> bytes, std::string_view magic, size_t at = 0) {
  if (bytes.size() < at + magic.size()) {
    return false;
  }
  for (size_t i = 0; i < magic.size(); ++i) {
    if (bytes[at + i] != static_cast<uint8_t>(magic[i])) {
      return false;
    }
  }
  return true;
}

}  // namespace

bool IsBoardId(std::string_view id) {
  return IsLowerHex(id, 32);
}

Image Sniff(base::span<const uint8_t> head) {
  if (StartsWith(head, "\x89PNG\r\n\x1a\n")) {
    return Image::kPng;
  }
  if (StartsWith(head, "\xff\xd8\xff")) {
    return Image::kJpeg;
  }
  if (StartsWith(head, "GIF87a") || StartsWith(head, "GIF89a")) {
    return Image::kGif;
  }
  if (StartsWith(head, "RIFF") && StartsWith(head, "WEBP", 8)) {
    return Image::kWebp;
  }
  return Image::kNone;
}

std::string FileName(std::string_view hash, Image image) {
  return std::string(hash) + "." + Extension(image);
}

Image FileImage(std::string_view name) {
  const size_t dot = name.find('.');
  if (dot == std::string_view::npos || !IsLowerHex(name.substr(0, dot), 64)) {
    return Image::kNone;
  }
  for (Image image : {Image::kPng, Image::kJpeg, Image::kGif, Image::kWebp}) {
    if (name.substr(dot + 1) == Extension(image)) {
      return image;
    }
  }
  return Image::kNone;
}

const char* MimeType(Image image) {
  switch (image) {
    case Image::kPng:
      return "image/png";
    case Image::kJpeg:
      return "image/jpeg";
    case Image::kGif:
      return "image/gif";
    case Image::kWebp:
      return "image/webp";
    case Image::kNone:
      return "application/octet-stream";
  }
  return "application/octet-stream";
}

base::FilePath BoardsDir(const base::FilePath& profile_dir) {
  return profile_dir.Append(FILE_PATH_LITERAL("Boards"));
}

base::FilePath BoardDir(const base::FilePath& profile_dir, std::string_view id) {
  return IsBoardId(id) ? BoardsDir(profile_dir).AppendASCII(id) : base::FilePath();
}

bool SaveBoardJson(const base::FilePath& board, std::string_view json) {
  return !board.empty() && json.size() <= kMaxBoardJson && base::DirectoryExists(board) &&
         base::ImportantFileWriter::WriteFileAtomically(
             board.Append(FILE_PATH_LITERAL("board.json")), json);
}

bool SaveThumb(const base::FilePath& board, base::span<const uint8_t> png) {
  return !board.empty() && png.size() <= kMaxThumb && Sniff(png) == Image::kPng &&
         base::DirectoryExists(board) &&
         base::ImportantFileWriter::WriteFileAtomically(
             board.Append(FILE_PATH_LITERAL("thumb.png")), base::as_string_view(png));
}

// static
std::unique_ptr<FileAdd> FileAdd::Start(const base::FilePath& board, uint64_t size) {
  if (board.empty() || size == 0 || size > kMaxFile || !base::DirectoryExists(board)) {
    return nullptr;
  }
  // files/ is the only folder made here, and only inside a Board that's there.
  base::FilePath files = board.Append(FILE_PATH_LITERAL("files"));
  base::FilePath temp;
  if (!base::CreateDirectory(files) || !base::CreateTemporaryFileInDir(files, &temp)) {
    return nullptr;
  }
  base::File file(temp, base::File::FLAG_OPEN | base::File::FLAG_WRITE);
  if (!file.IsValid()) {
    base::DeleteFile(temp);
    return nullptr;
  }
  return base::WrapUnique(new FileAdd(std::move(files), std::move(temp), std::move(file), size));
}

FileAdd::FileAdd(base::FilePath files, base::FilePath temp, base::File file, uint64_t size)
    : files_(std::move(files)), temp_(std::move(temp)), file_(std::move(file)), size_(size) {}

FileAdd::~FileAdd() {
  Fail();
}

void FileAdd::Fail() {
  failed_ = true;
  file_.Close();
  if (!temp_.empty()) {
    base::DeleteFile(temp_);
    temp_.clear();
  }
}

bool FileAdd::Append(base::span<const uint8_t> bytes) {
  if (failed_ || bytes.size() > size_ - written_) {
    Fail();
    return false;
  }
  // The first bytes say what it is, before any is kept.
  if (written_ == 0) {
    if (bytes.size() < std::min<uint64_t>(kSniffed, size_) ||
        (image_ = Sniff(bytes)) == Image::kNone) {
      Fail();
      return false;
    }
  }
  if (!file_.WriteAtCurrentPosAndCheck(bytes)) {
    Fail();
    return false;
  }
  hasher_.Update(bytes);
  written_ += bytes.size();
  return true;
}

std::optional<std::string> FileAdd::Finish() {
  if (failed_ || written_ != size_ || image_ == Image::kNone) {
    Fail();
    return std::nullopt;
  }
  std::array<uint8_t, crypto::hash::kSha256Size> digest;
  hasher_.Finish(digest);
  file_.Close();
  std::string name = FileName(base::ToLowerASCII(base::HexEncode(digest)), image_);
  base::FilePath stored = files_.AppendASCII(name);
  // The same bytes stored before are the same file.
  if (base::PathExists(stored) ? !base::DeleteFile(temp_)
                               : !base::ReplaceFile(temp_, stored, nullptr)) {
    Fail();
    return std::nullopt;
  }
  temp_.clear();
  failed_ = true;  // Done: nothing more comes.
  return name;
}

}  // namespace browser_core::board
