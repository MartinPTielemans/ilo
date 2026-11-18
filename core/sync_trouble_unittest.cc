#include "browser_core/sync_trouble.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::sync_trouble {
namespace {

using syncer::SyncerError;

const base::Time kStart = base::Time::FromMillisecondsSinceUnixEpoch(1'700'000'000'000);

bc_sync_activity Shown(const Run& run) {
  bc_sync_activity activity = {};
  run.Read(&activity);
  return activity;
}

Refused Of(int kind, int count) {
  Refused refused;
  for (int i = 0; i < count; ++i) {
    refused.Add(kind);
  }
  return refused;
}

TEST(SyncTroubleTest, EachErrorIsItsKind) {
  const SyncerError ok = SyncerError::Success();
  const Refused none;
  EXPECT_EQ(Classify(ok, ok, none), BC_SYNC_TROUBLE_NONE);
  EXPECT_EQ(Classify(SyncerError::NetworkError(-106), ok, none), BC_SYNC_TROUBLE_OFFLINE);
  EXPECT_EQ(Classify(SyncerError::HttpError(net::HTTP_INTERNAL_SERVER_ERROR), ok, none),
            BC_SYNC_TROUBLE_SERVER);
  EXPECT_EQ(Classify(SyncerError::HttpError(net::HTTP_BAD_GATEWAY), ok, none),
            BC_SYNC_TROUBLE_SERVER);
  EXPECT_EQ(Classify(SyncerError::HttpError(net::HTTP_UNAUTHORIZED), ok, none),
            BC_SYNC_TROUBLE_SIGN_IN);
  EXPECT_EQ(Classify(SyncerError::ProtocolViolationError(), ok, none), BC_SYNC_TROUBLE_SERVER);
  EXPECT_EQ(Classify(SyncerError::ProtocolError(syncer::THROTTLED), ok, none),
            BC_SYNC_TROUBLE_THROTTLED);
  EXPECT_EQ(Classify(SyncerError::ProtocolError(syncer::NOT_MY_BIRTHDAY), ok, none),
            BC_SYNC_TROUBLE_SERVER);
  // A commit's.
  EXPECT_EQ(Classify(ok, SyncerError::NetworkError(-106), none), BC_SYNC_TROUBLE_OFFLINE);
  EXPECT_EQ(Classify(ok, SyncerError::ProtocolError(syncer::CONFLICT), none),
            BC_SYNC_TROUBLE_NONE);
  EXPECT_EQ(Classify(ok, SyncerError::ProtocolError(syncer::TRANSIENT_ERROR), none),
            BC_SYNC_TROUBLE_SERVER);
  const Refused three = Of(BC_SYNC_KIND_SPACES, 3);
  EXPECT_EQ(Classify(ok, SyncerError::ProtocolError(syncer::TRANSIENT_ERROR), three),
            BC_SYNC_TROUBLE_REFUSED);
  EXPECT_EQ(Classify(ok, SyncerError::ProtocolError(syncer::INVALID_MESSAGE), three),
            BC_SYNC_TROUBLE_REFUSED);
  // The download's says what stopped a cycle that couldn't download.
  EXPECT_EQ(Classify(SyncerError::NetworkError(-106),
                     SyncerError::ProtocolError(syncer::TRANSIENT_ERROR), three),
            BC_SYNC_TROUBLE_OFFLINE);
}

TEST(SyncTroubleTest, KindsAreSettingsNames) {
  EXPECT_EQ(KindOfField("ilo_space"), BC_SYNC_KIND_SPACES);
  EXPECT_EQ(KindOfField("bookmark"), BC_SYNC_KIND_BOOKMARKS);
  EXPECT_EQ(KindOfField("password"), BC_SYNC_KIND_PASSWORDS);
  EXPECT_EQ(KindOfField("session"), BC_SYNC_KIND_OPEN_TABS);
  EXPECT_EQ(KindOfField("ilo_setting"), BC_SYNC_KIND_SETTINGS);
  EXPECT_EQ(KindOfField("preference"), BC_SYNC_KIND_SETTINGS);
  EXPECT_EQ(KindOfField("extension"), BC_SYNC_KIND_EXTENSIONS);
  EXPECT_EQ(KindOfField("device_info"), BC_SYNC_KIND_OTHER);
  EXPECT_EQ(Of(BC_SYNC_KIND_BOOKMARKS, 2).kind(), BC_SYNC_KIND_BOOKMARKS);
  Refused mixed = Of(BC_SYNC_KIND_BOOKMARKS, 2);
  mixed.Add(BC_SYNC_KIND_PASSWORDS);
  EXPECT_EQ(mixed.count(), 3);
  EXPECT_EQ(mixed.kind(), BC_SYNC_KIND_OTHER);
}

// One failed cycle, or two, then one that goes through, shows nothing (#3435's report).
TEST(SyncTroubleTest, AMissedCycleShowsNothing) {
  Run run;
  EXPECT_FALSE(run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart));
  EXPECT_FALSE(run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart + base::Seconds(30)));
  EXPECT_EQ(Shown(run).trouble, BC_SYNC_TROUBLE_NONE);
  EXPECT_FALSE(run.Ended(BC_SYNC_TROUBLE_NONE, Refused(), kStart + base::Seconds(60)));
  // The count starts again.
  EXPECT_FALSE(run.Ended(BC_SYNC_TROUBLE_SERVER, Refused(), kStart + base::Seconds(90)));
  EXPECT_FALSE(run.Ended(BC_SYNC_TROUBLE_SERVER, Refused(), kStart + base::Seconds(100)));
  EXPECT_EQ(Shown(run).trouble, BC_SYNC_TROUBLE_NONE);
}

// The third failure in a row shows, since the first, and the next good cycle clears it.
TEST(SyncTroubleTest, ThreeInARowShowSinceTheFirst) {
  Run run;
  run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart);
  run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart + base::Seconds(10));
  EXPECT_TRUE(run.Ended(BC_SYNC_TROUBLE_SERVER, Refused(), kStart + base::Seconds(20)));
  bc_sync_activity shown = Shown(run);
  EXPECT_EQ(shown.trouble, BC_SYNC_TROUBLE_SERVER);
  EXPECT_EQ(shown.since, kStart.InMillisecondsFSinceUnixEpoch());
  EXPECT_EQ(shown.refused, 0);
  // Another of the same changes nothing.
  EXPECT_FALSE(run.Ended(BC_SYNC_TROUBLE_SERVER, Refused(), kStart + base::Seconds(40)));
  EXPECT_TRUE(run.Ended(BC_SYNC_TROUBLE_NONE, Refused(), kStart + base::Seconds(80)));
  shown = Shown(run);
  EXPECT_EQ(shown.trouble, BC_SYNC_TROUBLE_NONE);
  EXPECT_EQ(shown.since, 0);
}

// Cycles back off as they fail, so two minutes of failures can be two cycles.
TEST(SyncTroubleTest, TwoMinutesOfFailuresShow) {
  Run run;
  run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart);
  EXPECT_FALSE(run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart + base::Seconds(119)));
  Run later;
  later.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart);
  EXPECT_TRUE(later.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart + base::Minutes(2)));
  EXPECT_EQ(Shown(later).trouble, BC_SYNC_TROUBLE_OFFLINE);
}

TEST(SyncTroubleTest, ARefusalSaysHowManyOfWhat) {
  Run run;
  const Refused two = Of(BC_SYNC_KIND_SPACES, 2);
  run.Ended(BC_SYNC_TROUBLE_REFUSED, two, kStart);
  run.Ended(BC_SYNC_TROUBLE_REFUSED, two, kStart + base::Seconds(30));
  EXPECT_TRUE(run.Ended(BC_SYNC_TROUBLE_REFUSED, Of(BC_SYNC_KIND_SPACES, 3),
                        kStart + base::Seconds(60)));
  bc_sync_activity shown = Shown(run);
  EXPECT_EQ(shown.trouble, BC_SYNC_TROUBLE_REFUSED);
  EXPECT_EQ(shown.refused, 3);
  EXPECT_EQ(shown.refused_kind, BC_SYNC_KIND_SPACES);
  EXPECT_TRUE(run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart + base::Seconds(90)));
  shown = Shown(run);
  EXPECT_EQ(shown.trouble, BC_SYNC_TROUBLE_OFFLINE);
  EXPECT_EQ(shown.refused, 0);
  EXPECT_EQ(shown.since, kStart.InMillisecondsFSinceUnixEpoch());
}

// Being told to wait is the server's answer: it shows from its cycle and ends a run of failures.
TEST(SyncTroubleTest, ThrottledShowsAtOnce) {
  Run run;
  run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart);
  run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart + base::Seconds(10));
  EXPECT_TRUE(run.Ended(BC_SYNC_TROUBLE_THROTTLED, Refused(), kStart + base::Seconds(20)));
  EXPECT_EQ(Shown(run).trouble, BC_SYNC_TROUBLE_THROTTLED);
  EXPECT_EQ(Shown(run).since, 0);
  EXPECT_FALSE(run.Ended(BC_SYNC_TROUBLE_OFFLINE, Refused(), kStart + base::Seconds(30)));
  EXPECT_EQ(Shown(run).trouble, BC_SYNC_TROUBLE_THROTTLED);
  EXPECT_TRUE(run.Ended(BC_SYNC_TROUBLE_NONE, Refused(), kStart + base::Seconds(40)));
  EXPECT_EQ(Shown(run).trouble, BC_SYNC_TROUBLE_NONE);
}

TEST(SyncTroubleTest, ResetForgetsTheRun) {
  Run run;
  for (int i = 0; i < 3; ++i) {
    run.Ended(BC_SYNC_TROUBLE_SIGN_IN, Refused(), kStart + base::Seconds(i));
  }
  EXPECT_EQ(Shown(run).trouble, BC_SYNC_TROUBLE_SIGN_IN);
  run.Reset();
  EXPECT_EQ(Shown(run).trouble, BC_SYNC_TROUBLE_NONE);
  EXPECT_FALSE(run.Ended(BC_SYNC_TROUBLE_SIGN_IN, Refused(), kStart + base::Seconds(9)));
}

}  // namespace
}  // namespace browser_core::sync_trouble
