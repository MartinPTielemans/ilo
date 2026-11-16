// A Board's folder in its profile (#3073): what its names may be, what a stored file may be, and
// the writes, all blocking, run on the Boards' sequence. Apart so browser_core_unittests tests it
// alone.

#ifndef BROWSER_CORE_BOARD_STORE_H_
#define BROWSER_CORE_BOARD_STORE_H_

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <string_view>

#include "base/containers/span.h"
#include "base/files/file.h"
#include "base/files/file_path.h"
#include "crypto/hash.h"

namespace browser_core::board {

// The most bytes a slice of the page's writes carries, the size the page sends (app/board/board.js).
inline constexpr size_t kSlice = 256 * 1024;
// A picture's bytes, the most one may be.
inline constexpr uint64_t kMaxFile = 50 * 1024 * 1024;
// A Board's own file, held whole in memory and written whole at each save. Its cards are text and
// references: a note's words, a link, a picture's hash, a line's points. 4 MiB holds some 2,000
// notes of a page of text each, or about 200,000 points of ink.
inline constexpr uint64_t kMaxBoardJson = 4 * 1024 * 1024;
// The Library's picture of a Board: a PNG of its page, 536 by 376.
inline constexpr uint64_t kMaxThumb = 4 * 1024 * 1024;

// A Board's id: 32 lowercase hex digits, 128 random bits the app makes. Never a path.
bool IsBoardId(std::string_view id);

// The only pictures a Board stores. No SVG: a file is never a document.
enum class Image { kNone, kPng, kJpeg, kGif, kWebp };

// What |head|, a file's first bytes, says it is by its magic, or kNone for anything else.
Image Sniff(base::span<const uint8_t> head);

// A stored file's name: the SHA-256 of its bytes in lowercase hex, and its picture's extension.
std::string FileName(std::string_view hash, Image image);

// The picture a stored file's |name| is, or kNone if |name| isn't one FileName gives.
Image FileImage(std::string_view name);

// The MIME type a stored file is served as.
const char* MimeType(Image image);

// |profile_dir|'s Boards: the folder each Board's folder is in.
base::FilePath BoardsDir(const base::FilePath& profile_dir);

// The folder of the Board |id| in |profile_dir|, or empty for an id that isn't one. It may not
// exist: the app makes it, at New Board, and nothing here ever does.
base::FilePath BoardDir(const base::FilePath& profile_dir, std::string_view id);

// The Board's own file, written whole into |board|, which must exist: a Board deleted while a
// save was on its way stays deleted.
bool SaveBoardJson(const base::FilePath& board, std::string_view json);

// The Library's picture of the Board, a PNG, written the same way.
bool SaveThumb(const base::FilePath& board, base::span<const uint8_t> png);

// A picture on its way into a Board's files, its bytes hashed as they come. The bytes go into a
// temporary file in the Board's files/, which becomes files/<FileName> once all have come and
// they're a picture, and is removed otherwise.
class FileAdd {
 public:
  // Starts adding a file of |size| bytes to |board|, or nullptr if |board| isn't there or |size| is
  // more than a picture may be.
  static std::unique_ptr<FileAdd> Start(const base::FilePath& board, uint64_t size);

  FileAdd(const FileAdd&) = delete;
  FileAdd& operator=(const FileAdd&) = delete;
  ~FileAdd();

  // Takes the next |bytes|. False, and the add is over, if they run past the size it started with,
  // its first bytes aren't a picture's, or the disk refused them.
  bool Append(base::span<const uint8_t> bytes);

  // The stored file's name once all the bytes have come, or nothing if they haven't or the add
  // failed. A file with the same bytes already there is kept, and this one dropped.
  std::optional<std::string> Finish();

 private:
  FileAdd(base::FilePath files, base::FilePath temp, base::File file, uint64_t size);
  void Fail();

  const base::FilePath files_;
  base::FilePath temp_;
  base::File file_;
  const uint64_t size_;
  uint64_t written_ = 0;
  Image image_ = Image::kNone;
  crypto::hash::Hasher hasher_{crypto::hash::kSha256};
  bool failed_ = false;
};

}  // namespace browser_core::board

#endif  // BROWSER_CORE_BOARD_STORE_H_
