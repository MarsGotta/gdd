#!/bin/sh
# What the blueprint's limits still allow one change, counted from its log, so nobody counts by hand.
#
# Usage: sh gdd/limits.sh <id>
#
# It reads the caps from the front matter of gdd/blueprint.md, those of the change's color before
# those that hold for every color, and the events of gdd/log/<id>.jsonl, and prints one line for
# each cap: what was used and what the blueprint allows. The last line is
#   gdd-limits {"color":"<color>","stop":[<each cap with no room left>],"slow":[<each tier past its seconds>],"judge":"<gates, review, ship or empty>"}
# The color is the result of the last brief step. A round is each launch of gdd-build or of
# gdd-checks after the first of each. Rounds, minutes, tokens and the runs of the checks that run
# before the review have no room left when what was used reaches the cap; the checks and the review,
# when the times they sent the change back pass fix_rounds_per_gate. Minutes leave out every wait
# for a person, from an ask to the event after it. A tier of checks past its seconds in a run of the
# gates stops nothing: it counts as it came out, and goes to the backlog for hardening. The
# builder's lines are not counted here: git diff --shortstat gives them.
#
# A cap with no room left stops the next round, never the judging of a build already finished. When
# the last of the build, gates and review steps is a build that didn't stop, its gates come next;
# gates that passed, its review; a review that passed, shipping it. Then the caps on rounds, on the
# times sent back, on minutes and on tokens are printed with what was used but left out of stop, a
# line says what to run and that no new round starts after it while a cap has no room left, and
# judge names what comes next. The runs before the review still stop a build that waits for its
# gates, because they would run once more. A brief or checks step after them starts over.
#
# A waiver of a limit that names it in "limit", by its key, and its new value in "to" sets that cap
# for this change only, the last waiver of each cap winning, and the line of that cap says so.
#
# It belongs to the project, like gdd/check.sh: the person approves it with the blueprint, and
# raising a cap is changing the blueprint. It reads two files and writes nothing.
# Exit code: 0 when stop is empty, 1 when it isn't, 2 when it is called wrong or a file is missing.
if [ "$#" -ne 1 ]; then
  echo "usage: sh gdd/limits.sh <id>" >&2
  exit 2
fi
case "$1" in
  *[!a-z0-9-]*|'') echo "limits.sh: an id is lowercase letters, digits and hyphens" >&2; exit 2 ;;
esac
log="gdd/log/$1.jsonl"
for file in gdd/blueprint.md "$log"; do
  if [ ! -f "$file" ]; then
    echo "limits.sh: $file is missing" >&2
    exit 2
  fi
done
awk -v now="$(date -u +%Y-%m-%dT%H:%M:%SZ)" '
function secs(s,   y, m, d, yy, era, yoe, mp, doy, doe) {
  y = substr(s, 1, 4) + 0; m = substr(s, 6, 2) + 0; d = substr(s, 9, 2) + 0
  yy = (m <= 2) ? y - 1 : y
  era = int(yy / 400); yoe = yy - era * 400
  mp = (m > 2) ? m - 3 : m + 9
  doy = int((153 * mp + 2) / 5) + d - 1
  doe = yoe * 365 + int(yoe / 4) - int(yoe / 100) + doy
  return (era * 146097 + doe - 719468) * 86400 + substr(s, 12, 2) * 3600 + substr(s, 15, 2) * 60 + substr(s, 18, 2)
}
function key_of(l) { sub(/^ */, "", l); sub(/:.*/, "", l); return l }
function number_of(l) { sub(/^[^:]*: */, "", l); return l + 0 }
function cap_of(key) {
  if (key in raised) return raised[key]
  if (color != "" && ((color, key) in own)) return own[color, key]
  return (key in all) ? all[key] : ""
}
function report(name, used, key, past, judged,   c, full) {
  c = cap_of(key)
  if (c == "") { printf "%s %d (no cap)\n", name, used; return }
  full = past ? used > c : used >= c
  printf "%s %d of %d%s%s\n", name, used, c, (key in raised) ? ", raised by a waiver" : "", full ? ", no room left" : ""
  if (full && judged) spent = 1
  else if (full) stop = stop (stop == "" ? "" : ",") "\"" name "\""
}
function tier(name, used, key,   c, past) {
  c = cap_of(key)
  if (c == "") return
  past = used > c
  printf "%s %d of %d%s%s\n", name, used, c, (key in raised) ? ", raised by a waiver" : "", past ? ", past it: to the backlog for hardening" : ""
  if (past) slow = slow (slow == "" ? "" : ",") "\"" name "\""
}
FNR == NR {
  if ($0 ~ /^---[ \t]*$/) { fence++; next }
  if (fence != 1) next
  line = $0
  sub(/[ \t]+#.*$/, "", line)
  if (line ~ /^[ \t]*(#|$)/) next
  if (line ~ /^[a-z_]+:/) { section = line; sub(/:.*/, "", section); next }
  if (section == "limits" && line ~ /^  [a-z_]+: *[0-9]+ *$/) all[key_of(line)] = number_of(line)
  if (section == "colors" && line ~ /^  [a-z]+: *$/) { hue = key_of(line); inside = 0 }
  if (section == "colors" && line ~ /^    [a-z_]+:/) inside = (line ~ /^    limits: *$/)
  if (section == "colors" && inside && line ~ /^      [a-z_]+: *[0-9]+ *$/) own[hue, key_of(line)] = number_of(line)
  if (section == "checks" && line ~ /^  - id: */) { id = line; sub(/^  - id: */, "", id); gsub(/[ "\047]/, "", id) }
  if (section == "checks" && line ~ /^    runs: *before review *$/) late[id] = 1
  next
}
{
  t = -1
  if (match($0, /"ts" *: *"[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]/)) {
    t = secs(substr($0, RSTART + RLENGTH - 19, 19))
    if (first == "") first = t
    if (waiting) { waited += t - asked; waiting = 0 }
  }
  if ($0 ~ /"event" *: *"ask"/ && t >= 0) { asked = t; waiting = 1 }
  if ($0 ~ /"event" *: *"waiver"/ && match($0, /"limit" *: *"[a-z_]+"/)) {
    cap = substr($0, RSTART, RLENGTH); sub(/^"limit" *: *"/, "", cap); sub(/"$/, "", cap)
    if (match($0, /"to" *: *"?[0-9]+/)) { v = substr($0, RSTART, RLENGTH); sub(/^"to" *: *"?/, "", v); raised[cap] = v + 0 }
  }
  if ($0 ~ /"step" *: *"brief"/ && match($0, /"result" *: *"(green|yellow|red)"/)) {
    color = substr($0, RSTART, RLENGTH); sub(/^"result" *: *"/, "", color); sub(/"$/, "", color)
  }
  if ($0 ~ /"step" *: *"(brief|checks|ship)"/) next_up = ""
  if ($0 ~ /"step" *: *"build"/) { builds++; next_up = ($0 ~ /"result" *: *"stopped"/) ? "" : "gates" }
  if ($0 ~ /"step" *: *"checks"/) checks++
  if ($0 ~ /"step" *: *"review"/) next_up = ($0 ~ /"verdict" *: *"pass"/) ? "ship" : ""
  if ($0 ~ /"step" *: *"review"/ && $0 ~ /"verdict" *: *"fix"/) review++
  if ($0 ~ /"step" *: *"gates"/) {
    next_up = ($0 ~ /"result" *: *"fail"/) ? "" : "review"
    if ($0 ~ /"result" *: *"fail"/) gates++
    each = 0; before = 0; late_ran = 0
    n = split($0, part, /"check" *: *"/)
    for (i = 2; i <= n; i++) {
      c = part[i]; sub(/".*/, "", c)
      s = 0
      if (match(part[i], /"seconds" *: *[0-9.]+/)) { s = substr(part[i], RSTART, RLENGTH); sub(/^[^:]*: */, "", s) }
      if (c in late) { late_ran = 1; before += s } else each += s
    }
    if (late_ran) late_runs++
    if (each > most_each) most_each = each
    if (before > most_before) most_before = before
  }
  rest = $0
  while (match(rest, /"tokens" *: *[0-9]+/)) {
    v = substr(rest, RSTART, RLENGTH); sub(/^[^:]*: */, "", v)
    tokens += v
    rest = substr(rest, RSTART + RLENGTH)
  }
}
END {
  end = waiting ? asked : secs(now)
  minutes = (first == "") ? 0 : int((end - first - waited) / 60)
  if (color != "") printf "color %s\n", color
  judged = (next_up != "")
  report("rounds", (builds > 1 ? builds - 1 : 0) + (checks > 1 ? checks - 1 : 0), "rounds_per_change", 0, judged)
  report("sent back by the checks", gates + 0, "fix_rounds_per_gate", 1, judged)
  report("sent back by the review", review + 0, "fix_rounds_per_gate", 1, judged)
  report("minutes", minutes, "minutes_per_change", 0, judged)
  if (cap_of("runs_before_review") != "") report("runs before the review", late_runs + 0, "runs_before_review", 0, judged && next_up != "gates")
  if (cap_of("tokens_per_change") != "") report("tokens", tokens + 0, "tokens_per_change", 0, judged)
  tier("seconds of the checks of a round", most_each + 0, "seconds_each_round")
  tier("seconds of the checks before the review", most_before + 0, "seconds_before_review")
  if (stop != "") next_up = ""
  after = spent ? "; a cap above has no room left, so no new round starts after it" : ""
  if (next_up == "gates") printf "the last build is waiting for its gates: run them%s\n", after
  if (next_up == "review") printf "the last build passed its gates and is waiting for its review: launch it%s\n", after
  if (next_up == "ship") printf "the last build passed its review and is waiting to ship%s\n", after
  printf "gdd-limits {\"color\":\"%s\",\"stop\":[%s],\"slow\":[%s],\"judge\":\"%s\"}\n", color, stop, slow, next_up
  exit (stop == "") ? 0 : 1
}' gdd/blueprint.md "$log"
