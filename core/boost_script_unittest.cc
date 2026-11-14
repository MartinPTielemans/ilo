#include "browser_core/boost_script.h"

#include <string>

#include "components/sync_preferences/testing_pref_service_syncable.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::boost_script {
namespace {

constexpr char kScript[] = "document.title = 'boosted';\n";

class BoostScriptTest : public testing::Test {
 protected:
  void SetUp() override { RegisterPrefs(prefs_.registry()); }

  sync_preferences::TestingPrefServiceSyncable prefs_;
};

TEST_F(BoostScriptTest, AScriptNewHereStaysOffUntilTurnedOn) {
  // Whatever another computer did: the record is this profile's alone.
  EXPECT_EQ(Check(prefs_, "a.test", kScript), State::kNew);
  ASSERT_TRUE(TurnOn(prefs_, "a.test", kScript));
  EXPECT_EQ(Check(prefs_, "a.test", kScript), State::kOn);
  // Another site's script is still new.
  EXPECT_EQ(Check(prefs_, "b.test", kScript), State::kNew);
}

TEST_F(BoostScriptTest, AnyChangedByteTurnsItOff) {
  ASSERT_TRUE(TurnOn(prefs_, "a.test", kScript));
  const std::string script = kScript;
  const std::string changed_ones[] = {script + " ", "document.title = 'boosted';\r\n",
                                      script.substr(0, script.size() - 1), std::string(),
                                      "document.title = 'Boosted';\n"};
  for (const std::string& changed : changed_ones) {
    EXPECT_EQ(Check(prefs_, "a.test", changed), State::kChanged) << changed;
  }
  // The file back as it was runs again: the record is the bytes, not a version.
  EXPECT_EQ(Check(prefs_, "a.test", kScript), State::kOn);
}

TEST_F(BoostScriptTest, TurnOffAndDeleteForgetIt) {
  ASSERT_TRUE(TurnOn(prefs_, "a.test", kScript));
  Forget(prefs_, "a.test");
  EXPECT_EQ(Check(prefs_, "a.test", kScript), State::kNew);
}

TEST_F(BoostScriptTest, OverTheLimitIsOffAndCantBeTurnedOn) {
  const std::string at_limit(kMaxBytes, 'x');
  const std::string over(kMaxBytes + 1, 'x');
  ASSERT_TRUE(TurnOn(prefs_, "a.test", at_limit));
  EXPECT_EQ(Check(prefs_, "a.test", at_limit), State::kOn);
  EXPECT_FALSE(TurnOn(prefs_, "a.test", over));
  EXPECT_EQ(Check(prefs_, "a.test", over), State::kTooLarge);
  // The script turned on before keeps its record.
  EXPECT_EQ(Check(prefs_, "a.test", at_limit), State::kOn);
}

TEST_F(BoostScriptTest, OnlyASiteHasARecord) {
  for (const char* not_site : {"", "../a.test", "a..test", ".a.test", "A.test", "a.test/x",
                               "a test", "a.test\\x", "a.test."}) {
    EXPECT_FALSE(TurnOn(prefs_, not_site, kScript)) << not_site;
    EXPECT_EQ(Check(prefs_, not_site, kScript), State::kNew) << not_site;
  }
}

TEST_F(BoostScriptTest, TheRecordNeverSyncs) {
  ASSERT_TRUE(prefs_.FindPreference("browser_core.boost_scripts"));
  // No flag at all, so neither the Settings choice nor priority preferences carry it.
  EXPECT_EQ(prefs_.registry()->GetRegistrationFlags("browser_core.boost_scripts"), 0u)
      << "a syncable record would let a server turn a script on";
}

}  // namespace
}  // namespace browser_core::boost_script
