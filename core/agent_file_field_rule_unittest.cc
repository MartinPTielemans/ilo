#include "browser_core/agent_file_field_rule.h"

#include <utility>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

FileFieldFacts Input(std::vector<std::string> attributes) {
  return {"INPUT", std::move(attributes)};
}

// The card's field, and the same field as the user picks, nothing changed.
FileFieldPlace Shown() {
  return {"F00D", "https://jobs.example"};
}
FileFieldNow Unchanged() {
  FileFieldNow now;
  now.same_document = true;
  now.agents = true;
  now.held = false;
  now.in_page = true;
  now.place = Shown();
  now.facts = Input({"type", "file", "name", "resume"});
  return now;
}

// Neither kind of a page's file dialog opens in an agent's tab nobody holds; in one the user
// holds, or one of theirs, the page's dialog is theirs as in any tab.
TEST(AgentFileFieldRuleTest, APagesDialogIsRefusedOnlyInAnAgentsTabNobodyHolds) {
  EXPECT_FALSE(PageMayOpenFileDialog(/*agents=*/true, /*held=*/false));
  EXPECT_TRUE(PageMayOpenFileDialog(/*agents=*/true, /*held=*/true));
  EXPECT_TRUE(PageMayOpenFileDialog(/*agents=*/false, /*held=*/false));
}

TEST(AgentFileFieldRuleTest, AFileInputMayBeAskedFor) {
  EXPECT_EQ(FileFieldRefusal(Input({"type", "file"})), std::nullopt);
  EXPECT_EQ(FileFieldRefusal(Input({"id", "cv", "type", "FILE"})), std::nullopt);
}

// The card is never shown for a folder field: the agent hears to hand the tab off.
TEST(AgentFileFieldRuleTest, AFolderFieldIsRefusedBeforeTheCard) {
  const std::optional<std::string> refused =
      FileFieldRefusal(Input({"type", "file", "webkitdirectory", ""}));
  ASSERT_TRUE(refused);
  EXPECT_NE(refused->find("hand_off"), std::string::npos);
}

TEST(AgentFileFieldRuleTest, AnythingButAFileInputIsRefused) {
  EXPECT_TRUE(FileFieldRefusal(Input({"type", "text"})));
  EXPECT_TRUE(FileFieldRefusal(Input({})));
  EXPECT_TRUE(FileFieldRefusal({"LABEL", {"for", "cv"}}));
  EXPECT_TRUE(FileFieldRefusal({"BUTTON", {"type", "file"}}));
}

TEST(AgentFileFieldRuleTest, MultipleLetsTheUserPickMore) {
  EXPECT_FALSE(FileFieldTakesMany(Input({"type", "file"})));
  EXPECT_TRUE(FileFieldTakesMany(Input({"type", "file", "multiple", ""})));
}

// Accept only filters what the dialog shows: its valid types, lowercased, and nothing it can't
// filter by.
TEST(AgentFileFieldRuleTest, AcceptListsTheTypesToFilterBy) {
  EXPECT_EQ(
      FileFieldAccepts(Input({"type", "file", "accept", " image/PNG, .PDF ,, bogus,image/*"})),
      (std::vector<std::string>{"image/png", ".pdf", "image/*"}));
  EXPECT_TRUE(FileFieldAccepts(Input({"type", "file"})).empty());
}

TEST(AgentFileFieldRuleTest, TheFilesGoIntoTheFieldTheCardNamed) {
  EXPECT_TRUE(FilesLand(Shown(), Unchanged(), 1));
}

// The page put another field, with the same label, in its place: the card's node isn't in the
// page, and the new one isn't the card's.
TEST(AgentFileFieldRuleTest, AFieldReplacedSinceTheCardGetsNothing) {
  FileFieldNow now = Unchanged();
  now.in_page = false;
  EXPECT_FALSE(FilesLand(Shown(), now, 1));
}

TEST(AgentFileFieldRuleTest, AFrameNavigatedSinceTheCardGetsNothing) {
  FileFieldNow navigated = Unchanged();
  navigated.same_document = false;
  EXPECT_FALSE(FilesLand(Shown(), navigated, 1));
  FileFieldNow other_origin = Unchanged();
  other_origin.place.origin = "https://elsewhere.example";
  EXPECT_FALSE(FilesLand(Shown(), other_origin, 1));
}

// The page moved the node into another of its frames.
TEST(AgentFileFieldRuleTest, AFieldInAnotherFrameGetsNothing) {
  FileFieldNow now = Unchanged();
  now.place.frame = "BEEF";
  EXPECT_FALSE(FilesLand(Shown(), now, 1));
}

TEST(AgentFileFieldRuleTest, ATabTheUserTookOrThatLeftTheSpaceGetsNothing) {
  FileFieldNow held = Unchanged();
  held.held = true;
  EXPECT_FALSE(FilesLand(Shown(), held, 1));
  FileFieldNow left = Unchanged();
  left.agents = false;
  EXPECT_FALSE(FilesLand(Shown(), left, 1));
}

// The page changed the node since the card: no longer a file input, now a folder's, or no longer
// taking several.
TEST(AgentFileFieldRuleTest, AFieldThePageChangedGetsNothing) {
  FileFieldNow text = Unchanged();
  text.facts = Input({"type", "text"});
  EXPECT_FALSE(FilesLand(Shown(), text, 1));
  FileFieldNow folder = Unchanged();
  folder.facts = Input({"type", "file", "webkitdirectory", ""});
  EXPECT_FALSE(FilesLand(Shown(), folder, 1));
  EXPECT_FALSE(FilesLand(Shown(), Unchanged(), 2));
  FileFieldNow many = Unchanged();
  many.facts = Input({"type", "file", "multiple", ""});
  EXPECT_TRUE(FilesLand(Shown(), many, 2));
}

// A sandboxed or data: frame has no site for the card to name, so it's refused before the card,
// and its "null" origin never matches itself at the pick.
TEST(AgentFileFieldRuleTest, AFrameWithNoSiteIsRefused) {
  EXPECT_EQ(FileFieldPlaceRefusal(Shown()), std::nullopt);
  const std::optional<std::string> refused = FileFieldPlaceRefusal({"F00D", "null"});
  ASSERT_TRUE(refused);
  EXPECT_NE(refused->find("hand_off"), std::string::npos);
  EXPECT_TRUE(FileFieldPlaceRefusal({"", "https://jobs.example"}));
  FileFieldNow opaque = Unchanged();
  opaque.place.origin = "null";
  EXPECT_FALSE(FilesLand({"F00D", "null"}, opaque, 1));
}

DocumentChild Child(int type,
                    std::string name,
                    std::string frame = "",
                    bool holds = false,
                    std::string local_name = "") {
  return {type, std::move(name), std::move(local_name), std::move(frame), holds};
}

// A field's frame is its document's, named on that document's element child, and only when that
// child isn't a frame owner, which DevTools gives its content frame's id too.
TEST(AgentFileFieldRuleTest, TheFrameIsTheDocumentsOwn) {
  const DocumentChild doctype = Child(10, "html");
  EXPECT_EQ(DocumentFrame({doctype, Child(1, "HTML", "F00D")}), "F00D");
  EXPECT_EQ(DocumentFrame({Child(8, "#comment"), Child(1, "svg", "F00D")}), "F00D");
  for (const char* owner : {"IFRAME", "frame", "Object", "EMBED", "fencedframe"}) {
    EXPECT_EQ(DocumentFrame({doctype, Child(1, owner, "BEEF")}), "");
  }
  EXPECT_EQ(DocumentFrame({Child(1, "x-root", "BEEF", /*holds=*/true)}), "");
  EXPECT_EQ(DocumentFrame({Child(1, "h:iframe", "BEEF", /*holds=*/false, "iframe")}), "");
  // A frame id on anything but the element child names no document's frame.
  EXPECT_EQ(DocumentFrame({Child(10, "html", "F00D")}), "");
  EXPECT_EQ(DocumentFrame({}), "");
}

TEST(AgentFileFieldRuleTest, NoFilesAreNoPick) {
  EXPECT_FALSE(FilesLand(Shown(), Unchanged(), 0));
}

// The app's own files for a field it described, nothing changed since.
FileFieldPutNow Puttable() {
  FileFieldPutNow now;
  now.same_document = true;
  now.origin = Shown().origin;
  now.agents = true;
  now.held = false;
  return now;
}

// The app's files start into the field only as it was described: the call is refused, before
// anything begins, for a frame now on another origin or document, a tab the user holds or that
// left the agent's Space, or a count the field doesn't take.
TEST(AgentFileFieldRuleTest, TheAppsFilesGoOnlyIntoTheFieldAsDescribed) {
  EXPECT_TRUE(MayPutFiles(Shown(), Puttable(), 1));

  FileFieldPutNow elsewhere = Puttable();
  elsewhere.origin = "https://other.example";
  EXPECT_FALSE(MayPutFiles(Shown(), elsewhere, 1));

  FileFieldPutNow navigated = Puttable();
  navigated.same_document = false;
  EXPECT_FALSE(MayPutFiles(Shown(), navigated, 1));

  FileFieldPutNow held = Puttable();
  held.held = true;
  EXPECT_FALSE(MayPutFiles(Shown(), held, 1));

  FileFieldPutNow left = Puttable();
  left.agents = false;
  EXPECT_FALSE(MayPutFiles(Shown(), left, 1));

  EXPECT_FALSE(MayPutFiles(Shown(), Puttable(), 0));
  EXPECT_FALSE(MayPutFiles(Shown(), Puttable(), 2));
  FileFieldPutNow many = Puttable();
  many.many = true;
  EXPECT_TRUE(MayPutFiles(Shown(), many, 2));
}

// A frame with no site never matches, even its own opaque origin.
TEST(AgentFileFieldRuleTest, TheAppsFilesNeverGoIntoAFrameWithNoSite) {
  FileFieldPutNow opaque = Puttable();
  opaque.origin = "null";
  EXPECT_FALSE(MayPutFiles({"F00D", "null"}, opaque, 1));
}

}  // namespace
}  // namespace browser_core
