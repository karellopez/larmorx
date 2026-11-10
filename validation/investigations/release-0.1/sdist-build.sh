#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 1: do the three sdists build a wheel again (maturin build in the unpacked sdist)?
set -u
INV=${1:?usage: $0 <output dir of build.sh>}
source "$(dirname "$0")/../../../scripts/orchestration/env.sh"
export CARGO_BUILD_JOBS=28
rm -rf "$INV/sdist-unpacked"; mkdir -p "$INV/sdist-unpacked" "$INV/wheels/from-sdist"
for s in "$INV/sdist/larmorx-0.0.1.tar.gz:target-plain" "$INV/sdist-staged/larmorx_nc-0.0.1.tar.gz:target-nc-plain" "$INV/sdist-staged/larmorx_gpl-0.0.1.tar.gz:target-gpl-plain"; do
  f=${s%%:*}; td=${s##*:}
  tar xzf "$f" -C "$INV/sdist-unpacked"
  d="$INV/sdist-unpacked/$(basename "$f" .tar.gz)"
  echo "== $(basename "$f")"
  (cd "$d" && CARGO_TARGET_DIR=$INV/$td maturin build --release -o "$INV/wheels/from-sdist" 2>&1 | grep -E "Built wheel|error|warning: unused|failed" | head -5)
done
ls -la "$INV/wheels/from-sdist"
echo "SDIST-BUILD DONE"
