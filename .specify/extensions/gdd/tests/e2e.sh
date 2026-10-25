#!/bin/bash
# End-to-end test of the GDD kit with fake agents (spends no tokens).
# Copies the repository to a temporary folder and runs four cases:
#   t0 · a Feature waits for the red zone approval (end of Discover).
#   t1 · yellow without scenarios: finishes on its own (Build -> Reviewer -> Merge -> Release).
#   t2 · with scenarios: human G1, pauses at G4 and resumes.
#   t3 · the Reviewer says KO twice: the change is parked.
#   t5 · planned red paths keep the change red.
#   t4 · the Builder touches the harness: Path Guard stops it and the change is parked.
# Usage: gdd-kit/tests/e2e.sh
set -euo pipefail
SRC="$(cd "$(dirname "$0")/../.." && pwd)"
TMP="$(mktemp -d "${TMPDIR:-/var/tmp}/gdd-e2e.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
export PATH="$SRC/gdd-kit/tests/fake:$PATH" FAKELOG="$TMP/fake.log"
export GIT_AUTHOR_NAME=gdd GIT_AUTHOR_EMAIL=gdd@local GIT_COMMITTER_NAME=gdd GIT_COMMITTER_EMAIL=gdd@local
chmod +x "$SRC"/gdd-kit/tests/fake/*
mkdir -p "$TMP/repo"
(cd "$SRC" && tar --exclude=./.git --exclude='./.gdd/log.jsonl' --exclude='./.gdd/changes' \
  --exclude='./.gdd/*.json' --exclude='./.gdd/*.txt' --exclude='__pycache__' --exclude='./.specify/workflows/runs' \
  --exclude='./features' -cf - .) | (cd "$TMP/repo" && tar xf -)
cd "$TMP/repo"; git init -q -b main
python3 - <<'PY'
p="gdd/commands.toml"; s=open(p).read()
s=s.replace('unit = ""','unit = "test -f src/hello.txt && ! test -f tests/failing"',1)
s=s.replace('security = ""','security = "true"',1).replace('simulated = false','simulated = true',1)
s=s.replace('file = ""','file = "tests/failing"',1).replace('content = ""','content = "x"',1)
open(p,"w").write(s)
PY
mkdir -p features docs; printf 'Feature: smoke\n  Scenario: starts\n    Given nothing\n' > features/smoke.feature
printf '# Project Brief\n\nTest project.\n' > docs/brief.md
git add -A; git commit -qm init
fail() { echo "KO $1"; exit 1; }
bin/gdd gate G0 ok --by machines >/dev/null 2>&1 && fail "G0 opened without a self-test"
bin/gdd selftest >/dev/null || fail "selftest"
bin/gdd gate G0 ok --by machines >/dev/null
# t0 · a Feature cannot be built before the red zones are approved (end of Discover)
bin/gdd new-change t0 --title "Too early" --paths src/hello.txt >/dev/null
git switch -q change/t0; (GDD_CHANGE=t0 bin/gdd prepare || true) | grep -q "Red zones not approved" || fail "t0 built before red zones"
git switch -q main
bin/gdd gate zones ok --by human >/dev/null
echo "OK t0 no Feature before the red zones are approved"
# t1
bin/gdd new-change t1 --title "Greeting" --paths src/hello.txt >/dev/null
bin/gdd launch t1 | grep -q "completed" || fail "t1 did not finish"
echo "OK t1 finishes on its own"
# t2
bin/gdd new-change t2 --title "Greeting with name" --paths features/greeting.feature >/dev/null
git switch -q change/t2; printf 'Feature: greeting\n  Scenario: greets\n    Given a name\n' > features/greeting.feature
git add -A; git commit -qm G1; bin/gdd gate G1 ok t2 --by human >/dev/null; git switch -q main
bin/gdd launch t2 | grep -q "paused" || fail "t2 did not pause at G4"
bin/gdd resume t2 --g4 ok | grep -q "completed" || fail "t2 did not finish after G4"
echo "OK t2 pauses at G4 and continues with the OK"
# t3
bin/gdd new-change t3 --title "KO" --paths src/n.txt >/dev/null
FAKE_VERDICT=KO bin/gdd launch t3 | grep -q "parkreviewend" || fail "t3 was not parked"
echo "OK t3 is parked after two Reviewer KOs"
git stash -u -q; git switch -q main
# t4
bin/gdd new-change t4 --title "Cheat" --paths src/n.txt >/dev/null
FAKE_PROTECTED=1 bin/gdd launch t4 | grep -q "parkg2end" || fail "t4 was not parked"
grep -q "protected path" .gdd/changes/t4/g2.json || fail "t4 without Path Guard"
echo "OK t4: Path Guard stops the Builder that touches the harness"
# t5 · planned red paths keep the change red even if the branch only touched scenarios so far
bin/gdd new-change t5 --title "Planned red" --paths bin/new-tool >/dev/null
git switch -q change/t5; printf 'Feature: t5\n  Scenario: s\n    Given x\n' > features/t5.feature; git add -A; git commit -qm t5
bin/gdd classify t5 | grep -q '"color": "red"' || fail "t5 lost its planned red color"
git switch -q main
echo "OK t5 planned red paths keep the color"
bin/gdd report --by role >/dev/null
echo "All green."
