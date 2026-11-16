#include "browser_core/board_store.h"

#include <algorithm>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "base/files/file_enumerator.h"
#include "base/files/file_util.h"
#include "base/files/scoped_temp_dir.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::board {
namespace {

constexpr char kId[] = "0123456789abcdef0123456789abcdef";
constexpr char kPng[] = "\x89PNG\r\n\x1a\n made-up picture bytes";

base::span<const uint8_t> Bytes(std::string_view text) {
  return base::as_byte_span(text);
}

// What's in |dir|, by name, in order.
std::vector<std::string> Names(const base::FilePath& dir) {
  std::vector<std::string> names;
  base::FileEnumerator each(dir, false,
                            base::FileEnumerator::FILES | base::FileEnumerator::DIRECTORIES);
  for (base::FilePath path = each.Next(); !path.empty(); path = each.Next()) {
    names.push_back(path.BaseName().AsUTF8Unsafe());
  }
  std::sort(names.begin(), names.end());
  return names;
}

class BoardStore : public testing::Test {
 protected:
  void SetUp() override {
    ASSERT_TRUE(profile_.CreateUniqueTempDir());
    board_ = BoardDir(profile_.GetPath(), kId);
    ASSERT_TRUE(base::CreateDirectory(board_));
  }

  base::FilePath files() const { return board_.Append(FILE_PATH_LITERAL("files")); }

  base::ScopedTempDir profile_;
  base::FilePath board_;
};

TEST(BoardNames, AnIdIs32LowercaseHexDigitsAndNeverAPath) {
  EXPECT_TRUE(IsBoardId(kId));
  EXPECT_FALSE(IsBoardId("0123456789ABCDEF0123456789ABCDEF"));
  EXPECT_FALSE(IsBoardId("0123456789abcdef"));
  EXPECT_FALSE(IsBoardId("../../../../Cookies"));
  EXPECT_FALSE(IsBoardId("0123456789abcdef0123456789abcde/"));
  EXPECT_TRUE(BoardDir(base::FilePath(FILE_PATH_LITERAL("p")), "..").empty());
}

TEST(BoardNames, AStoredFileIsAHashAndAPicturesExtension) {
  const std::string hash(64, 'a');
  EXPECT_EQ(FileImage(FileName(hash, Image::kPng)), Image::kPng);
  EXPECT_EQ(FileImage(FileName(hash, Image::kWebp)), Image::kWebp);
  EXPECT_EQ(FileImage(hash + ".svg"), Image::kNone);
  EXPECT_EQ(FileImage(hash + ".html"), Image::kNone);
  EXPECT_EQ(FileImage(hash), Image::kNone);
  EXPECT_EQ(FileImage("../board.json"), Image::kNone);
  EXPECT_EQ(FileImage(std::string(63, 'a') + ".png"), Image::kNone);
}

TEST(BoardNames, OnlyFourPicturesAreKept) {
  EXPECT_EQ(Sniff(Bytes(kPng)), Image::kPng);
  EXPECT_EQ(Sniff(Bytes("\xff\xd8\xff\xe0 jfif")), Image::kJpeg);
  EXPECT_EQ(Sniff(Bytes("GIF89a......")), Image::kGif);
  EXPECT_EQ(Sniff(Bytes(std::string_view("RIFF\x10\0\0\0WEBPVP8 ", 16))), Image::kWebp);
  EXPECT_EQ(Sniff(Bytes("<svg xmlns='http://www.w3.org/2000/svg'/>")), Image::kNone);
  EXPECT_EQ(Sniff(Bytes("<!doctype html><script>")), Image::kNone);
  EXPECT_EQ(Sniff(Bytes(std::string_view("RIFF\x10\0\0\0WAVEfmt ", 16))), Image::kNone);
  EXPECT_STREQ(MimeType(Image::kWebp), "image/webp");
}

TEST_F(BoardStore, APictureIsStoredByTheHashOfItsBytes) {
  const std::string bytes = kPng;
  std::unique_ptr<FileAdd> add = FileAdd::Start(board_, bytes.size());
  ASSERT_TRUE(add);
  // In two slices, as the page sends them.
  ASSERT_TRUE(add->Append(Bytes(bytes).first(16u)));
  ASSERT_TRUE(add->Append(Bytes(bytes).subspan(16u)));
  std::optional<std::string> name = add->Finish();
  ASSERT_TRUE(name);
  EXPECT_EQ(FileImage(*name), Image::kPng);
  EXPECT_EQ(Names(files()), std::vector<std::string>{*name});
  std::string stored;
  ASSERT_TRUE(base::ReadFileToString(files().AppendASCII(*name), &stored));
  EXPECT_EQ(stored, bytes);

  // The same bytes again are the same file.
  add = FileAdd::Start(board_, bytes.size());
  ASSERT_TRUE(add && add->Append(Bytes(bytes)));
  EXPECT_EQ(add->Finish(), name);
  EXPECT_EQ(Names(files()), std::vector<std::string>{*name});
}

TEST_F(BoardStore, WhatIsntAPictureIsRefusedAtItsFirstSliceAndLeavesNothing) {
  const std::string page = "<!doctype html><script>alert(1)</script>";
  std::unique_ptr<FileAdd> add = FileAdd::Start(board_, page.size());
  ASSERT_TRUE(add);
  EXPECT_FALSE(add->Append(Bytes(page)));
  EXPECT_FALSE(add->Finish());
  EXPECT_TRUE(Names(files()).empty());
}

TEST_F(BoardStore, TheCapIsCountedAcrossSlices) {
  EXPECT_FALSE(FileAdd::Start(board_, kMaxFile + 1));
  EXPECT_FALSE(FileAdd::Start(board_, 0));
  // Slices that run past the size the add started with end it.
  const std::string bytes = kPng;
  std::unique_ptr<FileAdd> add = FileAdd::Start(board_, bytes.size());
  ASSERT_TRUE(add && add->Append(Bytes(bytes)));
  EXPECT_FALSE(add->Append(Bytes("more")));
  EXPECT_FALSE(add->Finish());
  EXPECT_TRUE(Names(files()).empty());
}

TEST_F(BoardStore, AnAbandonedAddLeavesNothing) {
  const std::string bytes = kPng;
  std::unique_ptr<FileAdd> add = FileAdd::Start(board_, bytes.size() * 2);
  ASSERT_TRUE(add && add->Append(Bytes(bytes)));
  // Short of its size, it isn't stored.
  EXPECT_FALSE(add->Finish());
  EXPECT_TRUE(Names(files()).empty());
  // Nor when the page goes away mid-add.
  add = FileAdd::Start(board_, bytes.size() * 2);
  ASSERT_TRUE(add && add->Append(Bytes(bytes)));
  EXPECT_EQ(Names(files()).size(), 1u);
  add.reset();
  EXPECT_TRUE(Names(files()).empty());
}

TEST_F(BoardStore, NothingIsWrittenToABoardThatIsntThere) {
  ASSERT_TRUE(base::DeletePathRecursively(board_));
  EXPECT_FALSE(FileAdd::Start(board_, 64));
  EXPECT_FALSE(SaveBoardJson(board_, "{}"));
  EXPECT_FALSE(SaveThumb(board_, Bytes(kPng)));
  // Not even its folder, which only the app's New Board makes.
  EXPECT_FALSE(base::PathExists(board_));
  EXPECT_FALSE(SaveBoardJson(base::FilePath(), "{}"));
}

TEST_F(BoardStore, TheBoardsFileAndItsPictureAreWrittenWhole) {
  ASSERT_TRUE(SaveBoardJson(board_, R"({"cards":[]})"));
  ASSERT_TRUE(SaveBoardJson(board_, R"({"cards":[1]})"));
  std::string json;
  ASSERT_TRUE(base::ReadFileToString(board_.Append(FILE_PATH_LITERAL("board.json")), &json));
  EXPECT_EQ(json, R"({"cards":[1]})");

  EXPECT_TRUE(SaveThumb(board_, Bytes(kPng)));
  EXPECT_FALSE(SaveThumb(board_, Bytes("GIF89a......")));
  EXPECT_EQ(Names(board_), (std::vector<std::string>{"board.json", "thumb.png"}));
}

TEST_F(BoardStore, ABoardsFileOverItsCapIsRefusedAndTheLastOneStays) {
  ASSERT_TRUE(SaveBoardJson(board_, R"({"cards":[1]})"));
  EXPECT_FALSE(SaveBoardJson(board_, std::string(kMaxBoardJson + 1, ' ')));
  std::string json;
  ASSERT_TRUE(base::ReadFileToString(board_.Append(FILE_PATH_LITERAL("board.json")), &json));
  EXPECT_EQ(json, R"({"cards":[1]})");
  EXPECT_TRUE(SaveBoardJson(board_, std::string(kMaxBoardJson, ' ')));
}

TEST_F(BoardStore, ACaptureIsStoredWholeAndWaitsInTheArrivalsInTheOrderItCame) {
  const std::string other = std::string(kPng) + " another";
  const std::optional<std::string> first = AddFile(board_, Bytes(kPng));
  const std::optional<std::string> second = AddFile(board_, Bytes(other));
  ASSERT_TRUE(first && second);
  EXPECT_EQ(Names(files()),
            (std::vector<std::string>{std::min(*first, *second), std::max(*first, *second)}));
  EXPECT_TRUE(Arrived(board_).empty());
  ASSERT_TRUE(Arrive(board_, *second));
  ASSERT_TRUE(Arrive(board_, *first));
  // The same picture twice is two cards to make.
  ASSERT_TRUE(Arrive(board_, *second));
  EXPECT_EQ(Arrived(board_), (std::vector<std::string>{*second, *first, *second}));
}

TEST_F(BoardStore, WhatTheBoardsPageTookLeavesTheArrivalsAndWhatCameSinceStays) {
  const std::string name = *AddFile(board_, Bytes(kPng));
  const std::string later = *AddFile(board_, Bytes(std::string(kPng) + " later"));
  ASSERT_TRUE(Arrive(board_, name) && Arrive(board_, name) && Arrive(board_, later));
  // The page took the two it read; the third came after its read.
  ASSERT_TRUE(TakeArrived(board_, {name, name}));
  EXPECT_EQ(Arrived(board_), (std::vector<std::string>{later}));
  // A name it never had takes nothing.
  ASSERT_TRUE(TakeArrived(board_, {name}));
  EXPECT_EQ(Arrived(board_), (std::vector<std::string>{later}));
  ASSERT_TRUE(TakeArrived(board_, {later}));
  EXPECT_TRUE(Arrived(board_).empty());
  // The pictures stay: nothing removes a stored file.
  EXPECT_EQ(Names(files()).size(), 2u);
}

TEST_F(BoardStore, OnlyAStoredFilesNameArrivesAndOnlyInABoardThatsThere) {
  EXPECT_FALSE(AddFile(board_, Bytes("not a picture's bytes")));
  EXPECT_FALSE(Arrive(board_, "../../Cookies"));
  EXPECT_FALSE(Arrive(board_, "board.json"));
  EXPECT_FALSE(Arrive(board_, std::string(64, 'a') + ".svg"));
  EXPECT_TRUE(Arrived(board_).empty());
  // A line the browser didn't write is passed over.
  const std::string name = FileName(std::string(64, 'a'), Image::kPng);
  ASSERT_TRUE(base::WriteFile(board_.Append(FILE_PATH_LITERAL("arrived")),
                              "../../Cookies\n" + name + "\n\nthumb.png\n"));
  EXPECT_EQ(Arrived(board_), (std::vector<std::string>{name}));

  ASSERT_TRUE(base::DeletePathRecursively(board_));
  EXPECT_FALSE(AddFile(board_, Bytes(kPng)));
  EXPECT_FALSE(Arrive(board_, name));
  EXPECT_FALSE(TakeArrived(board_, {name}));
  EXPECT_FALSE(base::PathExists(board_));
}

TEST_F(BoardStore, AtMostKMaxArrivedWaitForThePage) {
  const std::string name = *AddFile(board_, Bytes(kPng));
  for (size_t i = 0; i < kMaxArrived; ++i) {
    ASSERT_TRUE(Arrive(board_, name)) << i;
  }
  EXPECT_FALSE(Arrive(board_, name));
  EXPECT_EQ(Arrived(board_).size(), kMaxArrived);
  ASSERT_TRUE(TakeArrived(board_, {name}));
  EXPECT_TRUE(Arrive(board_, name));
}

}  // namespace
}  // namespace browser_core::board
