#include "browser_core/ad_block_off_sites.h"

#include <utility>

#include "base/values.h"
#include "components/prefs/pref_registry_simple.h"
#include "components/prefs/testing_pref_service.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

// Where ExtensionPrefs keeps uBlock Origin's entry, and the list in it.
constexpr char kExtensions[] = "extensions.settings";
constexpr char kOldList[] = "oghkpboejbohceocapipmnjifoklonij.browser_core_off_sites";

// A profile's prefs, with the sites uBlock Origin kept in its entry of the extension prefs.
class AdBlockOffSitesTest : public testing::Test {
 protected:
  void SetUp() override {
    RegisterAdBlockOffSitesPrefs(prefs_.registry());
    prefs_.registry()->RegisterDictionaryPref(kExtensions);
    base::DictValue extensions;
    extensions.SetByDottedPath(kOldList, Sites());
    prefs_.SetDict(kExtensions, std::move(extensions));
  }

  // uBlock Origin's list, as ExtensionPrefs::ReadPrefAsList gives it.
  const base::ListValue* Old() const {
    return prefs_.GetDict(kExtensions).FindListByDottedPath(kOldList);
  }

  static base::ListValue Sites() {
    return base::ListValue()
        .Append("*://*.example.com/*")
        .Append("*://127.0.0.1/*")
        .Append("*://*.news.example/*");
  }

  const base::ListValue& Kept() const { return prefs_.GetList(kAdBlockOffSitesPref); }

  TestingPrefServiceSimple prefs_;
};

TEST_F(AdBlockOffSitesTest, CarriesTheOldListOverOnceInItsOrder) {
  CarryOverAdBlockOffSites(&prefs_, Old());
  EXPECT_EQ(Kept(), Sites());
  EXPECT_TRUE(prefs_.GetBoolean(kAdBlockOffSitesCarriedPref));
  // The old list is left as it was, the way back for a release.
  ASSERT_TRUE(Old());
  EXPECT_EQ(*Old(), Sites());
}

TEST_F(AdBlockOffSitesTest, AnEmptiedListStaysEmpty) {
  CarryOverAdBlockOffSites(&prefs_, Old());
  // The user turns ad blocking back on on every site, and the profile loads
  // again.
  prefs_.SetList(kAdBlockOffSitesPref, base::ListValue());
  CarryOverAdBlockOffSites(&prefs_, Old());
  EXPECT_TRUE(Kept().empty());
  ASSERT_TRUE(Old());
  EXPECT_EQ(*Old(), Sites());
}

TEST_F(AdBlockOffSitesTest, ALaterOldListIsNotTakenUp) {
  // A profile with no old list is done at once, so nothing it writes there
  // later comes back.
  CarryOverAdBlockOffSites(&prefs_, nullptr);
  EXPECT_TRUE(prefs_.GetBoolean(kAdBlockOffSitesCarriedPref));
  CarryOverAdBlockOffSites(&prefs_, Old());
  EXPECT_TRUE(Kept().empty());
}

TEST_F(AdBlockOffSitesTest, TheProfilesOwnListWins) {
  prefs_.SetList(kAdBlockOffSitesPref, base::ListValue().Append("*://*.other.example/*"));
  CarryOverAdBlockOffSites(&prefs_, Old());
  EXPECT_EQ(Kept(), base::ListValue().Append("*://*.other.example/*"));
  ASSERT_TRUE(Old());
  EXPECT_EQ(*Old(), Sites());
}

}  // namespace
}  // namespace browser_core
