#!/bin/sh
# Builds the layer outside the repository and prints where the binary is; `sh build.sh test` runs
# its tests there too.
# Cargo hard-links what it builds, and a plugin's files can't have two names: with a target/ folder
# in here, the eval harness refuses every case of the plugin.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
dir="${GDD_TARGET_DIR:-${TMPDIR:-/tmp}/gdd-engine-target}"
if [ "${1:-}" = "test" ]; then
  exec cargo test --manifest-path "$here/Cargo.toml" --target-dir "$dir"
fi
cargo build --release --manifest-path "$here/Cargo.toml" --target-dir "$dir"
echo "$dir/release/gdd"
