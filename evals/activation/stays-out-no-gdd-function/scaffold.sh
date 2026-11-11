#!/usr/bin/env bash
# Builds the project this case runs in. The harness runs it in the empty workspace of each run.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$here/../../fixtures/shop.sh" plain .
