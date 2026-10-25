#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Rust gates of a checkout: fmt, clippy -D warnings, tests, in the root workspace, crates-gpl and/or crates-nc.
# Usage: gates-rust.sh <checkout> [root|gpl|nc|both|all]   (both: root and gpl; all: the three)
set -o pipefail
source "$(dirname "$0")/env.sh"
W=$1; WHICH=${2:-both}; status=0
run() {
  local dir=$1; echo "== $dir"
  (cd "$dir" && cargo fmt --all --check) || { echo "FMT FAILED in $dir"; return 1; }
  (cd "$dir" && cargo clippy --workspace --all-targets --locked -- -D warnings) || { echo "CLIPPY FAILED in $dir"; return 1; }
  (cd "$dir" && cargo test --workspace --locked) || { echo "TEST FAILED in $dir"; return 1; }
  echo "OK $dir"
}
if [ "$WHICH" = root ] || [ "$WHICH" = both ] || [ "$WHICH" = all ]; then run "$W" || status=1; fi
if [ "$WHICH" = gpl ] || [ "$WHICH" = both ] || [ "$WHICH" = all ]; then run "$W/crates-gpl" || status=1; fi
if [ "$WHICH" = nc ] || [ "$WHICH" = all ]; then run "$W/crates-nc" || status=1; fi
echo "GATES-RUST exit $status"; exit $status
