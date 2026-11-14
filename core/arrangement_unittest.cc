#include "browser_core/arrangement.h"

#include <limits>
#include <string>

#include "base/strings/string_util.h"
#include "components/sync_preferences/testing_pref_service_syncable.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::arrangement {
namespace {

constexpr char kFile[] = R"({"version": 1, "changes": [
  {"kind": "settle", "name": "The video", "role": "video",
   "target": {"path": "main > video", "fp": {"tag": "video", "landmark": "main"}},
   "before": {"name": "the article", "role": "article", "target": {"path": "main > article"}}},
  {"kind": "trade", "name": "Trending", "role": "aside", "target": {"fp": {"tag": "aside", "nth": 1}},
   "with": {"name": "Sign up", "role": "form", "target": {"path": "#signup"}}},
  {"kind": "float", "name": "More like this", "role": "section", "target": {"path": "#more"},
   "at": {"x": 24, "y": 96.5}, "pin": "screen", "width": 320}
]})";

// |kFile| with |from| put in place of |to|, once.
std::string With(std::string_view from, std::string_view to) {
  std::string file = kFile;
  const size_t at = file.find(from);
  EXPECT_NE(at, std::string::npos) << from;
  return at == std::string::npos ? file : file.replace(at, from.size(), to);
}

TEST(ArrangementTest, AFileOfTheSchemaIsReadWhole) {
  const std::optional<base::DictValue> file = Parse(kFile);
  ASSERT_TRUE(file);
  const base::ListValue* changes = file->FindList("changes");
  ASSERT_TRUE(changes);
  ASSERT_EQ(changes->size(), 3u);
  EXPECT_EQ(*(*changes)[0].GetDict().FindStringByDottedPath("before.name"), "the article");
  EXPECT_EQ((*changes)[1].GetDict().FindIntByDottedPath("target.fp.nth"), 1);
  EXPECT_EQ((*changes)[2].GetDict().FindDoubleByDottedPath("at.y"), 96.5);
  // What the core writes, it reads back as it was.
  EXPECT_EQ(Parse(Write(*file)), file);
}

TEST(ArrangementTest, AnythingTheSchemaDoesntNameRefusesTheFile) {
  const std::string refused[] = {
      With(R"("version": 1)", R"("version": 2)"),
      With(R"("version": 1)", R"("version": "1")"),
      With(R"("kind": "trade")", R"("kind": "script")"),
      With(R"("kind": "trade")", R"("kind": "hide")"),
      // A field of its own, at each depth.
      With(R"("version": 1,)", R"("version": 1, "css": "body{display:none}",)"),
      With(R"("pin": "screen")", R"j("pin": "screen", "onclick": "alert(1)")j"),
      With(R"("tag": "video",)", R"("tag": "video", "style": "x",)"),
      With(R"("path": "#signup")", R"("path": "#signup", "script": "x")"),
      // A settle against both neighbours, or neither.
      With(R"("before": {"name": "the article")", R"("after": {"name": "x", "role": "x",
           "target": {"path": "p"}}, "before": {"name": "the article")"),
      With(R"("before": {"name": "the article", "role": "article", "target": {"path": "main > article"}})",
           R"("width": 3)"),
      // A trade's other element in a settle's field.
      With(R"("with":)", R"("before":)"),
      // Names and kinds are strings; places are finite numbers within reach.
      With(R"("name": "Trending")", R"("name": 7)"),
      With(R"("x": 24)", R"("x": "24")"),
      With(R"("x": 24)", R"("x": 1e300)"),
      With(R"("width": 320)", R"("width": 0)"),
      With(R"("pin": "screen")", R"("pin": "fixed")"),
      // A target finds by something.
      With(R"({"path": "#more"})", R"({})"),
      With(R"({"path": "#more"})", R"({"path": ""})"),
      // A tag name is a tag name.
      With(R"("tag": "aside")", R"("tag": "a b")"),
      With(R"("tag": "aside")", R"("tag": "<script>")"),
      With(R"("nth": 1)", R"("nth": -1)"),
      With(R"("nth": 1)", R"("nth": 0.5)"),
      // Not JSON as RFC 8259 has it: no comments, no trailing commas.
      With(R"("version": 1,)", R"("version": 1, /* x */)"),
      With(R"("width": 320})", R"("width": 320,})"),
  };
  for (const std::string& file : refused) {
    EXPECT_FALSE(Parse(file)) << file;
  }
}

TEST(ArrangementTest, AHostileFileIsRefusedWholeAndNothingOfItRidesAlong) {
  // Markup, script and CSS in every string field, as data, are kept as data: the page's script only
  // ever compares them or gives a path to querySelector, which throws on these and finds nothing.
  const std::string markup = R"(</script><script>alert(1)</script><style>*{display:none}</style>)";
  std::string hostile = kFile;
  for (std::string_view field : {"The video", "the article", "main > video", "Trending", "#more"}) {
    base::ReplaceSubstringsAfterOffset(&hostile, 0, field, markup);
  }
  const std::optional<base::DictValue> file = Parse(hostile);
  ASSERT_TRUE(file);
  // Rebuilt, it has the schema's fields and no others, whatever they hold.
  EXPECT_EQ(*file->FindList("changes")->front().GetDict().FindString("name"), markup);
  EXPECT_EQ(file->size(), 2u);

  // Over each limit, the whole file is refused, never the part within it.
  std::string many = R"({"version": 1, "changes": [)";
  for (size_t i = 0; i <= kMaxChanges; ++i) {
    many += std::string(i ? "," : "") +
            R"({"kind": "float", "name": "", "role": "div", "target": {"path": "p"},
                "at": {"x": 0, "y": 0}, "pin": "page"})";
  }
  many += "]}";
  ASSERT_LE(many.size(), kMaxBytes);
  EXPECT_FALSE(Parse(many));
  EXPECT_FALSE(Parse(With("Trending", std::string(kMaxString + 1, 'x'))));
  EXPECT_TRUE(Parse(With("Trending", std::string(kMaxString, 'x'))));
  std::string big = With("More like this", std::string(kMaxString, 'x'));
  while (big.size() <= kMaxBytes) {
    big.insert(big.size() - 2, R"(, {"kind": "float", "name": ")" + std::string(kMaxString, 'y') +
                                   R"(", "role": "div", "target": {"path": "p"},
                                   "at": {"x": 0, "y": 0}, "pin": "page"})");
  }
  EXPECT_FALSE(Parse(big));
  // Deep nesting, where the schema has none.
  EXPECT_FALSE(Parse(With(R"({"path": "#more"})",
                          std::string(500, '[') + std::string(500, ']'))));
  EXPECT_FALSE(Parse(With(R"("tag": "video",)", R"("tag": "video", "text": {"a": {"b": "c"}},)")));
  // Not a file at all.
  for (std::string_view junk : {"", "null", "[]", "1", "\"x\"", "{", "\xff\xfe"}) {
    EXPECT_FALSE(Parse(junk)) << junk;
  }
}

class ArrangementRecordTest : public testing::Test {
 protected:
  void SetUp() override { RegisterPrefs(prefs_.registry()); }

  sync_preferences::TestingPrefServiceSyncable prefs_;
};

TEST_F(ArrangementRecordTest, OneNewHereStaysOffUntilTurnedOn) {
  EXPECT_EQ(Check(prefs_, "a.test", kFile), State::kNew);
  ASSERT_TRUE(TurnOn(prefs_, "a.test", kFile));
  EXPECT_EQ(Check(prefs_, "a.test", kFile), State::kOn);
  EXPECT_EQ(Check(prefs_, "b.test", kFile), State::kNew);
}

TEST_F(ArrangementRecordTest, AnyChangedByteTurnsItOff) {
  ASSERT_TRUE(TurnOn(prefs_, "a.test", kFile));
  // Still the schema, and one moved button is all it takes.
  EXPECT_EQ(Check(prefs_, "a.test", With(R"("x": 24)", R"("x": 25)")), State::kChanged);
  EXPECT_EQ(Check(prefs_, "a.test", std::string(kFile) + "\n"), State::kChanged);
  EXPECT_EQ(Check(prefs_, "a.test", kFile), State::kOn);
}

TEST_F(ArrangementRecordTest, ARefusedFileIsOffAndCantBeTurnedOn) {
  const std::string refused = With(R"("pin": "screen")", R"("pin": "fixed")");
  EXPECT_EQ(Check(prefs_, "a.test", refused), State::kRefused);
  EXPECT_FALSE(TurnOn(prefs_, "a.test", refused));
  EXPECT_EQ(Check(prefs_, "a.test", kFile), State::kNew);
  // Turned on, a file that's then refused is still refused, not changed.
  ASSERT_TRUE(TurnOn(prefs_, "a.test", kFile));
  EXPECT_EQ(Check(prefs_, "a.test", refused), State::kRefused);
}

TEST_F(ArrangementRecordTest, ResetForgetsIt) {
  ASSERT_TRUE(TurnOn(prefs_, "a.test", kFile));
  Forget(prefs_, "a.test");
  EXPECT_EQ(Check(prefs_, "a.test", kFile), State::kNew);
}

TEST_F(ArrangementRecordTest, ASiteThatIsntOneHasNoRecord) {
  for (std::string_view site : {"", "A.test", "a..test", ".a.test", "a.test/", "a test"}) {
    EXPECT_FALSE(TurnOn(prefs_, site, kFile)) << site;
    EXPECT_EQ(Check(prefs_, site, kFile), State::kNew) << site;
  }
}

TEST(ArrangementTest, AMotionValueOutsideItsRangeTakesTheDefault) {
  EXPECT_EQ(Within(0.2, 0.05, 1, 0.3), 0.2);
  EXPECT_EQ(Within(0.05, 0.05, 1, 0.3), 0.05);
  EXPECT_EQ(Within(1, 0.05, 1, 0.3), 1);
  // A zeroed struct, a value past either end, and NaN.
  EXPECT_EQ(Within(0, 0.05, 1, 0.3), 0.3);
  EXPECT_EQ(Within(1.01, 0.05, 1, 0.3), 0.3);
  EXPECT_EQ(Within(-1, 0.05, 1, 0.3), 0.3);
  EXPECT_EQ(Within(std::numeric_limits<double>::quiet_NaN(), 0.05, 1, 0.3), 0.3);
  EXPECT_EQ(Within(std::numeric_limits<double>::infinity(), 5, 100, 25), 25);
}

}  // namespace
}  // namespace browser_core::arrangement
