#include "browser_core/settings_values.h"

#include <map>
#include <string>

#include "browser_core/agent_tabs_close_rule.h"
#include "browser_core/sync_settings.pb.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::settings_values {
namespace {

using Entity = sync_settings_pb::SettingEntity;

// A new profile has Chromium's own: third-party cookies allowed (blocked in private windows),
// spell check on, the languages Chromium picks, and a thread's tabs closing after 15 minutes.
TEST(SettingsValuesTest, DefaultsAreChromiumsAndFifteenMinutes) {
  const Values values = Read(base::DictValue());
  EXPECT_FALSE(values.block_third_party_cookies);
  EXPECT_TRUE(values.spell_check);
  EXPECT_FALSE(values.languages);
  EXPECT_EQ(values.agent_tabs_close_after, 900);
}

TEST(SettingsValuesTest, EachRoundTripsThroughTheSessionFile) {
  Values values;
  values.block_third_party_cookies = true;
  values.spell_check = false;
  values.languages = "nl,en-US";
  values.agent_tabs_close_after = 3600;
  base::DictValue settings;
  Write(values, settings);
  const Values read = Read(settings);
  EXPECT_TRUE(read.block_third_party_cookies);
  EXPECT_FALSE(read.spell_check);
  EXPECT_EQ(read.languages, "nl,en-US");
  EXPECT_EQ(read.agent_tabs_close_after, 3600);
}

// A time the pop-up doesn't offer, as an edited file may hold, reads as the nearest it does.
TEST(SettingsValuesTest, ACloseTimeReadsAsThePopUpsNearest) {
  base::DictValue settings;
  settings.Set("agent_tabs_close_after", 1000.0);
  EXPECT_EQ(Read(settings).agent_tabs_close_after, 900);
  settings.Set("agent_tabs_close_after", -5.0);
  EXPECT_EQ(Read(settings).agent_tabs_close_after, 0);
  settings.Set("agent_tabs_close_after", 1e9);
  EXPECT_EQ(Read(settings).agent_tabs_close_after, 7200);
}

TEST(SettingsValuesTest, LanguagesKeepTheirOrderWithoutRepeatsOrSpaces) {
  EXPECT_EQ(Languages("en-US, nl ,fr"), "en-US,nl,fr");
  EXPECT_EQ(Languages("nl,en,NL,en"), "nl,en");
  EXPECT_EQ(Languages("zh-Hant-TW"), "zh-Hant-TW");
}

TEST(SettingsValuesTest, LanguagesRefuseAnEmptyListOrATagThatIsntOne) {
  EXPECT_FALSE(Languages(""));
  EXPECT_FALSE(Languages(" , "));
  EXPECT_FALSE(Languages("en,,nl"));
  EXPECT_FALSE(Languages("english"));
  EXPECT_FALSE(Languages("en_US"));
  EXPECT_FALSE(Languages("e"));
  EXPECT_FALSE(Languages("en-toolongsubtag"));
  EXPECT_FALSE(Languages("en;q=0.9"));
}

// A language list the file holds that isn't one leaves Chromium's.
TEST(SettingsValuesTest, AnUnreadableLanguageListInTheFileLeavesChromiums) {
  base::DictValue settings;
  settings.Set("languages", "en_US");
  EXPECT_FALSE(Read(settings).languages);
}

// Cookies, spell check and the languages follow the account; the close time stays here, and
// languages go up only once the user has chosen them, so Chromium's own never overwrite theirs.
TEST(SettingsValuesTest, ThreeFollowTheAccountAndCloseTimeStays) {
  Values values;
  std::map<std::string, Entity> all;
  ToEntities(values, all);
  EXPECT_TRUE(all.contains(kBlockThirdPartyCookies));
  EXPECT_TRUE(all.contains(kSpellCheck));
  EXPECT_FALSE(all.contains(kLanguages));
  EXPECT_FALSE(all.contains("agent_tabs_close_after"));
  values.languages = "nl";
  ToEntities(values, all);
  EXPECT_EQ(all[kLanguages].text(), "nl");
}

TEST(SettingsValuesTest, EachRoundTripsThroughItsEntity) {
  Values values;
  values.block_third_party_cookies = true;
  values.spell_check = false;
  values.languages = "fr,de";
  std::map<std::string, Entity> all;
  ToEntities(values, all);
  Values other;
  for (const auto& [name, entity] : all) {
    Entity sent;
    ASSERT_TRUE(sent.ParseFromString(entity.SerializeAsString()));
    FromEntity(sent, other);
  }
  EXPECT_TRUE(other.block_third_party_cookies);
  EXPECT_FALSE(other.spell_check);
  EXPECT_EQ(other.languages, "fr,de");
}

// An entity from a version before the text field, or one whose text isn't a language list,
// changes nothing.
TEST(SettingsValuesTest, ALanguagesEntityWithoutItsTextChangesNothing) {
  Values values;
  values.languages = "nl";
  Entity old;
  old.set_name(kLanguages);
  Entity read;
  ASSERT_TRUE(read.ParseFromString(old.SerializeAsString()));
  EXPECT_FALSE(read.has_text());
  FromEntity(read, values);
  EXPECT_EQ(values.languages, "nl");
  Entity bad;
  bad.set_name(kLanguages);
  bad.set_text("not a list");
  FromEntity(bad, values);
  EXPECT_EQ(values.languages, "nl");
}

// A newer version's field this one doesn't know is kept, and the rest still reads, as an older
// version reads the text field.
TEST(SettingsValuesTest, AFieldFromANewerVersionIsKeptAndTheRestReads) {
  Entity entity;
  entity.set_name(kSpellCheck);
  entity.set_on(false);
  // Field 9, a string: tag (9 << 3) | 2, then its length and bytes.
  const std::string bytes = entity.SerializeAsString() + std::string("\x4a\x03" "abc", 5);
  Entity read;
  ASSERT_TRUE(read.ParseFromString(bytes));
  EXPECT_EQ(read.name(), kSpellCheck);
  EXPECT_FALSE(read.on());
  EXPECT_EQ(read.SerializeAsString(), bytes);
}

}  // namespace
}  // namespace browser_core::settings_values
