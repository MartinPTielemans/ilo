#!/bin/bash
# Proves tools/update-order.py: its port of Sparkle's comparator on Sparkle 2.10.1's own cases
# (Tests/SUVersionComparisonTest.m), then the release shapes it meets, each a scratch history:
# 0.3.0 tagged before version.sh, then a build that shows 0.1.0, which every updater is offered;
# that release renamed ilo-v0.3.0, which is still the one before; a later release after version.sh;
# and a build whose four-part version went down, as packaging the shown version would have, which
# it refuses. It changes nothing here. On a Mac with SPARKLE_FRAMEWORKS naming the folder that
# holds the Sparkle.framework app/setup.sh pins, every case also goes through that framework's own
# comparator, which has to agree with the port.
# Usage: [SPARKLE_FRAMEWORKS=<folder>] tools/update-order-test.sh
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=check GIT_AUTHOR_EMAIL=check@example.test
export GIT_COMMITTER_NAME=check GIT_COMMITTER_EMAIL=check@example.test
failed=0

# Sparkle's cases: a < b, a = b, a > b. Each pair, both ways round, goes in pairs.tsv with the
# port's answer.
python3 -I - "$here/tools" "$scratch/pairs.tsv" <<'EOF' || failed=1
import sys
sys.path.insert(0, sys.argv[1])
import importlib.util
spec = importlib.util.spec_from_file_location("order", sys.argv[1] + "/update-order.py")
order = importlib.util.module_from_spec(spec)
spec.loader.exec_module(order)
ascending = [
    ("1.0", "1.1"), ("0.1", "0.1.2"),
    ("1.5.5-335d3e2", "1.5.6-b252311"),
    ("1.5.5", "1.5.6a1"), ("1.1.0b1", "1.1.0b2"), ("1.1.1b2", "1.1.2b1"), ("1.1.1b2", "1.1.2a1"),
    ("1.0a1", "1.0b1"), ("1.0b1", "1.0"), ("0.9", "1.0a1"), ("1.0b", "1.0b2"),
    ("1.0b10", "1.0b11"), ("1.0b9", "1.0b10"), ("1.0rc", "1.0"), ("1.0b", "1.0"),
    ("1.0pre1", "1.0"),
    ("1.0 (1234)", "1.0 (1235)"), ("1.0b1 (1234)", "1.0 (1234)"), ("1.0b5 (1234)", "1.0b5 (1235)"),
    ("1.0b5 (1234)", "1.0.1b5 (1234)"), ("1.0.1b5 (1234)", "1.0.1b6 (1234)"),
    ("2.0.0.2429", "2.0.0.2430"), ("1.1.1.1818", "2.0.0.2430"), ("3.3 (5847)", "3.3.1b1 (5902)"),
    ("201210251627", "201211051041"),
    # The builds this release crosses: 0.3.0's, and one past it.
    ("4741", "6404"), ("999", "1000"),
]
equal = [
    ("1.0", "1.0"), ("1.0", "1.0.0"), ("1.0.0", "1.0"),
    ("1.5.5-335d3e2", "1.5.5-a655360"), ("1.5-335d3e2", "1.5.0-335d3e2"),
    ("1.5.0-335d3e2", "1.5-335d3e2"),
    ("1.0pre1", "1.0.0pre1"), ("1.0.0pre1", "1.0pre1"),
    ("3.3 (5847)", "3.3.0 (5847)"), ("3.3.0 (5847)", "3.3 (5847)"),
    ("1.1.1.1818", "1.1.1.1818.0"), ("1.1.1.1818.0", "1.1.1.1818"),
    ("3.3b1 (5902)", "3.3.0b1 (5902)"), ("3.3.0b1 (5902)", "3.3b1 (5902)"),
]
descending = [("2.0", "1.1"), ("0.1", "0.0.1"), ("1.5.6-b252311", "1.5.5-335d3e2")]
wrong = []
pairs = open(sys.argv[2], "w")
for cases, want in ((ascending, -1), (equal, 0), (descending, 1)):
    for a, b in cases:
        pairs.write(f"{a}\t{b}\t{order.compare(a, b)}\n{b}\t{a}\t{order.compare(b, a)}\n")
        got = order.compare(a, b)
        if got != want:
            wrong.append(f"{a} against {b}: {got}, Sparkle says {want}")
        if want and order.compare(b, a) != -want:
            wrong.append(f"{b} against {a}: not the reverse")
for line in wrong:
    print(f"FAIL comparator: {line}")
print(f"comparator: {sum(map(len, (ascending, equal, descending)))} of Sparkle's cases, {len(wrong)} wrong")
sys.exit(1 if wrong else 0)
EOF

# The framework the Mac app ships, on the same pairs: -1, 0 or 1, as NSComparisonResult is.
if [ -n "${SPARKLE_FRAMEWORKS:-}" ]; then
  cat > "$scratch/sparkle.m" <<'EOF'
#import <Foundation/Foundation.h>
#import <Sparkle/Sparkle.h>

int main(void) {
  @autoreleasepool {
    NSData *data = [[NSFileHandle fileHandleWithStandardInput] readDataToEndOfFile];
    NSString *input = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    SUStandardVersionComparator *comparator = [SUStandardVersionComparator defaultComparator];
    for (NSString *line in [input componentsSeparatedByString:@"\n"]) {
      NSArray<NSString *> *parts = [line componentsSeparatedByString:@"\t"];
      if (parts.count < 2) continue;
      printf("%s\t%s\t%ld\n", parts[0].UTF8String, parts[1].UTF8String,
             (long)[comparator compareVersion:parts[0] toVersion:parts[1]]);
    }
  }
  return 0;
}
EOF
  if ! xcrun clang -fobjc-arc -F"$SPARKLE_FRAMEWORKS" -framework Foundation -framework Sparkle \
      -Wl,-rpath,"$SPARKLE_FRAMEWORKS" "$scratch/sparkle.m" -o "$scratch/sparkle"; then
    echo "FAIL comparator: Sparkle's own didn't build against $SPARKLE_FRAMEWORKS"
    failed=1
  elif ! "$scratch/sparkle" < "$scratch/pairs.tsv" > "$scratch/sparkle.tsv" ||
    ! diff "$scratch/pairs.tsv" "$scratch/sparkle.tsv" > "$scratch/differ"; then
    echo "FAIL comparator: the port and Sparkle's framework differ (< the port, > Sparkle's):"
    sed 's/^/  /' "$scratch/differ"
    failed=1
  else
    echo "ok comparator: Sparkle's framework agrees on all $(wc -l < "$scratch/pairs.tsv" | tr -d ' ') pairs"
  fi
fi

# A commit on the scratch history, showing `$1` and holding `$2`'s version.sh: none, today's, or
# one that packs the shown version, as package.sh did before.
commit() {
  mkdir -p app/platform/windows/package
  printf '[package]\nname = "browser"\nversion = "%s"\n' "$1" > app/Cargo.toml
  case $2 in
    none) rm -f app/platform/windows/package/version.sh ;;
    today) cp "$here/app/platform/windows/package/version.sh" app/platform/windows/package/ ;;
    shown) printf '#!/bin/bash\necho "%s.0"\n' "$1" > app/platform/windows/package/version.sh ;;
  esac
  git add -A && git commit -q --allow-empty -m "$1"
}

# Runs update-order.py in the scratch history, and fails unless it exits `$1` and says `$2`.
expect() {
  local out rc
  out=$(python3 -I "$here/tools/update-order.py" ${3:+"$3"} 2>&1)
  rc=$?
  if [ "$rc" != "$1" ] || ! grep -qF -- "$2" <<<"$out"; then
    echo "FAIL $name: exited $rc, wanting $1 and a line with \"$2\". It said:"
    sed 's/^/  /' <<<"$out"
    failed=1
  else
    echo "ok $name"
  fi
}

cd "$scratch" && git init -q -b main
for n in 1 2 3; do commit 0.2.$n none; done
git tag v0.2.3
commit 0.3.0 none && git tag -a -m 0.3.0 v0.3.0
for n in 1 2 3 4; do commit 0.1.0 today; done

name="0.3.0 to a build showing 0.1.0" expect 0 "offered: Windows, MSIX and the installer's four-part version, 0.3.0.0 → 1.0.8.0"
name="Sparkle's build" expect 0 "offered: Mac, Sparkle's comparator on the build, 4 → 8"
name="winget is said, not required" expect 0 "not offered: winget under the same id, by the shown version, 0.3.0 → 0.1.0"
git tag v0.1.0
name="the release being made isn't the one before" expect 0 "v0.3.0 (0.3.0) to this build (0.1.0)" v0.1.0

git tag ilo-v0.3.0 v0.3.0 && git tag -d v0.3.0 > /dev/null && git tag -d v0.2.3 > /dev/null
name="the one before, renamed" expect 0 "ilo-v0.3.0 (0.3.0) to this build (0.1.0)" v0.1.0
commit 0.1.1 today
name="a later release after version.sh" expect 0 "1.0.8.0 → 1.0.9.0"

git tag -d v0.1.0 > /dev/null
commit 0.1.0 shown
name="a four-part version that went down" expect 1 "REFUSED: Windows, MSIX and the installer's four-part version, 0.3.0.0 → 0.1.0.0"

git clone -q --depth 1 "file://$scratch" "$scratch/shallow" && cd "$scratch/shallow" || exit 1
name="a shallow clone" expect 1 "shallow clone"

exit "$failed"
