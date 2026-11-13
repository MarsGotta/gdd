#!/usr/bin/env bash
# Builds the project this case runs in. The harness runs it in the empty workspace of each run.
# A yellow change briefed and sealed with its scenarios, whose approval was asked for and not given yet.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$here/../fixtures/shop.sh" asked .
