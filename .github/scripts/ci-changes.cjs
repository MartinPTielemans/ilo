// Which CI jobs a change needs, from the paths it touches. A path no rule knows runs every job, so a
// new directory is checked until a rule here says what it affects. The idea is T3 Code's
// .github/scripts/ci-changes.cjs (https://github.com/pingdotgg/t3code, MIT) at commit
// 570d4b03eef879bd60be3d74444046858c94dacd, with ilo's jobs in place of its packages.
const fs = require("node:fs");

// Each job ci.yml can run, and the paths it covers. app_linux and engine_windows are ready for
// app-linux.yml and engine-windows.yml; ci.yml's comment says how to hook them in.
const JOBS = {
  core_linux: [/^core\//, /^engine\//, /^\.github\/workflows\/core-linux\.yml$/],
  app_linux: [/^app\//, /^core\/public\//, /^\.github\/workflows\/app-linux\.yml$/],
  engine_windows: [
    /^core\//,
    /^engine\//,
    /^\.github\/workflows\/engine-windows\.yml$/,
    /^\.github\/actions\/engine-windows-stage\//,
  ],
};

// The gate itself: a change to it could hide what it skips.
const RUNS_EVERYTHING = [
  /^\.github\/workflows\/ci\.yml$/,
  /^\.github\/scripts\/ci-changes\.cjs$/,
  /^\.github\/actions\/ci-changes\//,
];

// Nothing a job builds reads these, nor Markdown anywhere. Spikes stand alone (AGENTS.md), and
// tools/ holds scripts for working on the repo.
const INERT = [
  /^docs\//,
  /^(LICENSE|NOTICE|\.gitignore)$/,
  /^spikes\//,
  /^tools\//,
  /^\.github\/(ISSUE_TEMPLATE|triage)\//,
  /^\.github\/[^/]+\.td$/,
  /^\.github\/workflows\//,
];

function decide(files) {
  const needed = Object.fromEntries(Object.keys(JOBS).map((job) => [job, files === null]));
  for (const file of files ?? []) {
    if (file.endsWith(".md")) continue;
    const owners = Object.keys(JOBS).filter((job) => JOBS[job].some((rule) => rule.test(file)));
    if (owners.length > 0) {
      for (const job of owners) needed[job] = true;
    } else if (RUNS_EVERYTHING.some((rule) => rule.test(file)) || !INERT.some((rule) => rule.test(file))) {
      console.error(`Runs everything: ${file}`);
      for (const job of Object.keys(needed)) needed[job] = true;
    }
  }
  return needed;
}

function main() {
  const args = process.argv.slice(2);
  const pathsIndex = args.indexOf("--paths");
  const files =
    pathsIndex === -1
      ? null
      : fs.readFileSync(args[pathsIndex + 1], "utf8").split("\n").filter(Boolean);
  for (const [job, needed] of Object.entries(decide(files))) console.log(`${job}=${needed}`);
}

module.exports = { decide };

if (require.main === module) main();
