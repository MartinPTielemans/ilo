#include "browser_core/boost_colors.h"

#include <string>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

constexpr bc_boost_colors kColors{0x1c2a3b, 0xf4f4f4, 0x3366ff, 0xffffff};
constexpr char kDeclared[] =
    ":root{--ilo-space-background: #1c2a3b; --ilo-space-text: #f4f4f4; "
    "--ilo-space-accent: #3366ff; --ilo-space-accent-text: #ffffff}\n";

TEST(BoostColors, ABoostThatNamesThemTakesTheSpacesColoursFirst) {
  constexpr char kCss[] = "body { background: var(--ilo-space-background) !important; }";
  EXPECT_EQ(BoostSheet(&kColors, kCss), std::string(kDeclared) + kCss);
}

TEST(BoostColors, ABoostThatNamesNoneTellsTheSiteNothing) {
  // The site's scripts could read them on the root, so a Boost that doesn't use them gets none.
  constexpr char kCss[] = "#ad { display: none !important; } /* Zap */";
  EXPECT_EQ(BoostSheet(&kColors, kCss), kCss);
}

TEST(BoostColors, NoColoursLeavesTheBoostAsItIs) {
  constexpr char kCss[] = "body { color: var(--ilo-space-text); }";
  EXPECT_EQ(BoostSheet(nullptr, kCss), kCss);
}

TEST(BoostColors, OnlyTheColoursValuesGoIn) {
  // Bits above 0xRRGGBB are dropped, so each value is six hex digits and nothing else.
  constexpr bc_boost_colors kWide{0xff1c2a3b, 0xfff4f4f4, 0xff3366ff, 0xffffffff};
  constexpr char kCss[] = "a { color: var(--ilo-space-accent); }";
  EXPECT_EQ(BoostSheet(&kWide, kCss), std::string(kDeclared) + kCss);
}

}  // namespace
}  // namespace browser_core
