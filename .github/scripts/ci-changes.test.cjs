// Which jobs ci-changes.cjs starts for a few paths whose answer isn't plain from their directory.
// CI's Changes job runs it before it decides: node --test .github/scripts/ci-changes.test.cjs
const assert = require("node:assert/strict");
const test = require("node:test");
const { decide } = require("./ci-changes.cjs");

// The jobs a change to these paths starts, by name.
function started(...paths) {
  return Object.entries(decide(paths))
    .filter(([, needed]) => needed)
    .map(([job]) => job)
    .sort();
}

test("the split keys' table runs the core's jobs and the UI's tests on each OS", () => {
  // The core's unit tests read it, and so do the app's (app/src/shell/keys.rs), #3800.
  assert.deepEqual(started("core/split_keys_layouts.txt"), [
    "app_linux",
    "app_mac",
    "app_windows",
    "core_linux",
    "engine_windows",
  ]);
});

test("another core file runs the core's jobs alone", () => {
  assert.deepEqual(started("core/tab.cc"), ["core_linux", "engine_windows"]);
});
