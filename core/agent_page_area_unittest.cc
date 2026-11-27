#include "browser_core/agent_page_area.h"

#include <vector>

#include "testing/gtest/include/gtest/gtest.h"
#include "ui/gfx/geometry/size.h"

namespace browser_core {
namespace {

TEST(AgentPageAreaTest, TheFrontWindowsPageArea) {
  EXPECT_EQ(AgentPageArea({gfx::Size(1440, 860), gfx::Size(900, 700)}, gfx::Size(1000, 600)),
            gfx::Size(1440, 860));
}

TEST(AgentPageAreaTest, AWindowNotLaidOutYetIsPassedOver) {
  EXPECT_EQ(AgentPageArea({gfx::Size(), gfx::Size(900, 700)}, gfx::Size(1000, 600)),
            gfx::Size(900, 700));
}

TEST(AgentPageAreaTest, WithNoWindowTheLastOnesArea) {
  EXPECT_EQ(AgentPageArea({}, gfx::Size(1000, 600)), gfx::Size(1000, 600));
  EXPECT_EQ(AgentPageArea({gfx::Size()}, gfx::Size(1000, 600)), gfx::Size(1000, 600));
}

TEST(AgentPageAreaTest, NoneBeforeAnyWindowLaidOutAPage) {
  EXPECT_TRUE(AgentPageArea({}, gfx::Size()).IsEmpty());
}

}  // namespace
}  // namespace browser_core
