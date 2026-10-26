#!/bin/bash
# End-to-end test of the GDD kit with fake agents (spends no tokens).
# Copies the repository to a temporary folder and runs four cases:
#   t0 · a Feature waits for the red zone approval (end of Discover).
#   t1 · yellow without scenarios: finishes on its own (Build -> Reviewer -> Merge -> Release).
#   t2 · with scenarios: human G1, pauses at G4 and resumes.
#   t3 · the Reviewer says KO twice: the change is parked.
#   t5 · planned red paths keep the change red.
#   t6 · a relaunch clears earlier requests from the inbox.
#   t7 · a higher-color path outside the package fails G2 (scope).
#   t8 · the replan sends a step bug back to test mode.
#   t9 · a green relaunch skips the Builder.
#   t10 · build mode is not blamed for the steps test mode wrote.
#   t11 · deleting the rules does not disable Path Guard.
#   t12 · deleting a protected step written in test mode is caught.
#   t13 · G1 refuses hidden steps with unknown phrasing.
#   t4 · the Builder touches the harness: Path Guard stops it and the change is parked.
# Usage: gdd-kit/tests/e2e.sh
set -euo pipefail
# grep -c, not grep -q, after a pipe: -q stops reading at the first match, the writer gets
# SIGPIPE and pipefail turns a passing check into a failure when the output is long.
SRC="$(cd "$(dirname "$0")/../.." && pwd)"
TMP="$(mktemp -d "${TMPDIR:-/var/tmp}/gdd-e2e.XXXXXX")"
trap '[ -n "${KEEP:-}" ] || rm -rf "$TMP"' EXIT
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
git switch -q change/t0; (GDD_CHANGE=t0 bin/gdd prepare || true) | grep -c "Red zones not approved" >/dev/null || fail "t0 built before red zones"
git switch -q main
bin/gdd gate zones ok --by human >/dev/null
echo "OK t0 no Feature before the red zones are approved"
# t1
bin/gdd new-change t1 --title "Greeting" --paths src/hello.txt >/dev/null
bin/gdd launch t1 | grep -c "completed" >/dev/null || fail "t1 did not finish"
echo "OK t1 finishes on its own"
# t2
bin/gdd new-change t2 --title "Greeting with name" --paths features/greeting.feature >/dev/null
git switch -q change/t2; printf 'Feature: greeting\n  Scenario: greets\n    Given a name\n' > features/greeting.feature
git add -A; git commit -qm G1; bin/gdd gate G1 ok t2 --by human >/dev/null; git switch -q main
bin/gdd launch t2 | grep -c "paused" >/dev/null || fail "t2 did not pause at G4"
git log -1 --format=%B main | grep -c "^Approvals: G1 ok (human" >/dev/null || fail "t2 merge commit does not record the G1 approval"
git log -1 --format=%B main | grep -c "^G4: human approval required" >/dev/null || fail "t2 merge commit does not say G4 is required"
git log -1 --format=%B main | grep -c "^Reviewer: OK" >/dev/null || fail "t2 merge commit does not record the Reviewer"
git log -1 --format=%B main | grep -c "^Tokens: in" >/dev/null || fail "t2 merge commit does not record the tokens"
bin/gdd resume t2 --g4 ok | grep -c "completed" >/dev/null || fail "t2 did not finish after G4"
echo "OK t2 pauses at G4 and continues with the OK"
# t3
bin/gdd new-change t3 --title "KO" --paths src/n.txt >/dev/null
FAKE_VERDICT=KO bin/gdd launch t3 > "$TMP/t3.out" 2>&1 || true; grep -q "parkreviewend" "$TMP/t3.out" || { tail -15 "$TMP/t3.out"; fail "t3 was not parked"; }
echo "OK t3 is parked after two Reviewer KOs"
git stash -u -q; git switch -q main
# t4
bin/gdd new-change t4 --title "Cheat" --paths src/n.txt >/dev/null
FAKE_PROTECTED=1 bin/gdd launch t4 | grep -c "parkg2end" >/dev/null || fail "t4 was not parked"
grep -q "protected path" .gdd/changes/t4/g2.json || fail "t4 without Path Guard"
echo "OK t4: Path Guard stops the Builder that touches the harness"
# t5 · planned red paths keep the change red even if the branch only touched scenarios so far
bin/gdd new-change t5 --title "Planned red" --paths bin/new-tool >/dev/null
git switch -q change/t5; printf 'Feature: t5\n  Scenario: s\n    Given x\n' > features/t5.feature; git add -A; git commit -qm t5
bin/gdd classify t5 | grep -q '"color": "red"' || fail "t5 lost its planned red color"
git switch -q main
echo "OK t5 planned red paths keep the color"
# t6 · a relaunch clears earlier requests from the inbox
bin/gdd new-change t6 --title "Stale request" --paths src/t6.txt >/dev/null
bin/gdd note t6 needs "STALE-REQUEST" >/dev/null
bin/gdd launch t6 | grep -c "completed" >/dev/null || fail "t6 did not finish"
grep -q "STALE-REQUEST" .gdd/changes/t6/inbox.md && fail "t6 inbox still shows a resolved request"
grep -q "resolved before the last launch" .gdd/changes/t6/inbox.md || fail "t6 inbox does not count resolved requests"
echo "OK t6 a relaunch clears earlier requests from the inbox"
# t7 · the Builder of a green change writes a yellow file outside the package: G2 fails on scope
bin/gdd new-change t7 --title "Docs only" --paths docs/t7.md --resumable >/dev/null
bin/gdd launch t7 > "$TMP/t7.out" 2>&1 || true; grep -q "parkg2end" "$TMP/t7.out" || { cat "$TMP/t7.out"; cat .gdd/changes/t7/g2.json 2>/dev/null | head -c 800; fail "t7 was not parked"; }
grep -q "outside the package" .gdd/changes/t7/g2.json || fail "t7 without a scope failure"
git stash -u -q; git switch -q main
echo "OK t7 a higher-color path outside the package fails G2 (scope)"
# t8 · the fault is in the test-mode steps: the replan sends the change back to test mode
bin/gdd new-change t8 --title "Step bug" --paths src/hello.txt src/n.txt >/dev/null
FAKE_STEP_BUG=1 FAKE_REPLAN_NEXT=test bin/gdd launch t8 | grep -c "completed" >/dev/null || fail "t8 did not finish after going back to test mode"
grep -q "gdd-build-test.*after=replan" "$FAKELOG" || fail "t8 never went back to test mode"
grep -q "accepted by rule" .gdd/changes/t8/inbox.md || fail "t8 inbox does not list accepted out-of-package paths"
echo "OK t8 the replan sends a step bug back to test mode"
# t9 · a relaunch whose code is already green skips the Builder
bin/gdd new-change t9 --title "Relaunch" --paths src/hello.txt src/n.txt >/dev/null
FAKE_VERDICT=KO bin/gdd launch t9 | grep -c "parkreviewend" >/dev/null || fail "t9 was not parked"
N=$(grep -c "gdd-build" "$FAKELOG")
bin/gdd launch t9 > "$TMP/t9.out" 2>&1 || true; grep -q "completed" "$TMP/t9.out" || { tail -20 "$TMP/t9.out"; fail "t9 relaunch did not finish"; }
[ "$(grep -c "gdd-build" "$FAKELOG")" = "$N" ] || fail "t9 relaunch called the Builder again"
grep -q "Builder skipped" .gdd/log.jsonl || fail "t9 prepare did not log the skip"
echo "OK t9 a green relaunch skips the Builder"
# t10 · test mode writes steps that are protected from build mode: build mode is judged from after test mode
bin/gdd new-change t10 --title "Protected steps" --paths src/hello.txt src/n.txt tests/acceptance/steps.txt >/dev/null
FAKE_PROTECTED_STEPS=1 bin/gdd launch t10 > "$TMP/t10.out" 2>&1 || true
grep -c "completed" "$TMP/t10.out" >/dev/null || { tail -15 "$TMP/t10.out"; fail "t10 did not finish (Path Guard blamed build mode for test-mode steps)"; }
echo "OK t10 build mode is not blamed for the steps test mode wrote"
# t11 · build mode deletes the rules file: Path Guard still uses the rules from when the build started
bin/gdd new-change t11 --title "Delete rules" --paths src/hello.txt src/n.txt >/dev/null
FAKE_DELETE_RULES=1 bin/gdd launch t11 > "$TMP/t11.out" 2>&1 || true
grep -c "parkg2end" "$TMP/t11.out" >/dev/null || { tail -15 "$TMP/t11.out"; fail "t11 was not parked (deleting the rules disabled Path Guard)"; }
bin/gdd guard t11 --role builder-build | grep -c "gdd/rules.toml: protected path" >/dev/null || fail "t11 Path Guard itself let the deleted rules through"
git stash -u -q; git checkout -q -- . ; git switch -q main
echo "OK t11 deleting the rules does not disable Path Guard"
# t12 · build mode deletes an untracked step file written in test mode: still caught
bin/gdd new-change t12 --title "Delete steps" --paths src/hello.txt src/n.txt tests/acceptance/t12.txt >/dev/null
FAKE_PROTECTED_STEPS=tests/acceptance/t12.txt FAKE_DELETE_STEPS=tests/acceptance/t12.txt bin/gdd launch t12 > "$TMP/t12.out" 2>&1 || true
grep -c "parkg2end" "$TMP/t12.out" >/dev/null || { tail -15 "$TMP/t12.out"; fail "t12 was not parked (a deleted protected step slipped through)"; }
bin/gdd guard t12 --role builder-build | grep -c "tests/acceptance/t12.txt: protected path" >/dev/null || fail "t12 Path Guard itself let the deleted step through"
git stash -u -q; git switch -q main
echo "OK t12 deleting a protected step written in test mode is caught"
# t13 · G1 refuses hidden examples whose step sentences no visible scenario has
bin/gdd new-change t13 --title "Hidden phrasing" --paths features/t13.feature >/dev/null
git switch -q change/t13; mkdir -p features/hidden
printf 'Feature: t13\n  Scenario: s\n    Given a name "Ana"\n' > features/t13.feature
printf '@hidden\nFeature: h\n  Scenario: s\n    Given a brand new sentence\n' > features/hidden/t13.feature
git add -A; git commit -qm t13
(bin/gdd gate G1 ok t13 --by human 2>&1 || true) | grep -c "G1 refused" >/dev/null || fail "t13 G1 accepted a hidden step with an unknown phrasing"
printf '@hidden\nFeature: h\n  Scenario: s\n    Given a name "Luz \\"la\\" buena"\n' > features/hidden/t13.feature
git add -A; git commit -qm t13b
bin/gdd gate G1 ok t13 --by human >/dev/null || fail "t13 G1 refused a hidden step that reuses a visible sentence"
git rm -rq features/hidden; git commit -qm t13c; git switch -q main
echo "OK t13 G1 refuses hidden steps with unknown phrasing"
bin/gdd report --by role >/dev/null
echo "All green."
