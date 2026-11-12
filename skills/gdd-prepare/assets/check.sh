#!/bin/sh
# The project's runner: it runs one check, times it and says what it took.
#
# Usage: sh gdd/check.sh <check id> -- <command> [<argument>...]
#
# The command runs as it is, with its own output and its own exit code. The last line is the
# runner's, and the seconds of a gate in the log come from it:
#   gdd-check {"check":"<check id>","seconds":<whole seconds>,"exit":<exit code>}
#
# The commands of the constitution go through it, and so do the ones of the blueprint's checks,
# all but seal. It belongs to the project: the person approves it with the blueprint, and a
# project that can't run it leaves it out, and then nothing says what a check took.
# Exit code: the command's own, or 2 when it is called wrong.
if [ "$#" -lt 3 ] || [ "$2" != "--" ]; then
  echo "usage: sh gdd/check.sh <check id> -- <command> [<argument>...]" >&2
  exit 2
fi
id="$1"
shift 2
start="$(date +%s)"
"$@"
code="$?"
end="$(date +%s)"
printf 'gdd-check {"check":"%s","seconds":%s,"exit":%s}\n' "$id" "$((end - start))" "$code"
exit "$code"
