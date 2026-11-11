#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 1 of the first release: build the three packages as they are today (Linux x86-64) and
# with the size settings (strip, LTO, one codegen unit), from the env only (no file changes),
# to measure wheel and sdist sizes, contents and build times. Kept for reproducibility (relay 1 ran it from its scratch folder).
set -u
WT=${1:?usage: $0 <checkout> <output dir>}
INV=${2:?usage: $0 <checkout> <output dir>}
source "$WT/scripts/orchestration/env.sh"   # PATH (FSL off), PYO3_PYTHON, OPENBLAS_CORETYPE, LARMORX_VENV
SEED=$LARMORX_WORKSPACE/larmorx/target/release
export CARGO_BUILD_JOBS=28
export OPENBLAS_CORETYPE=Haswell
t() { local s=$SECONDS; "$@"; local rc=$?; echo "TIME $(( SECONDS - s )) s rc=$rc: $*"; return $rc; }

mkdir -p "$INV/target-plain" "$INV/wheels" "$INV/sdist"
[ -d "$INV/target-plain/release" ] || cp -a "$SEED" "$INV/target-plain/release"

echo "== main, plain release (today's settings)"
cd "$WT" || exit 1
CARGO_TARGET_DIR=$INV/target-plain t maturin build --release --locked -o "$INV/wheels/plain" 2>&1 | grep -E "Built wheel|error|warning: |TIME|📦|🔗" | tail -5
echo "== main sdist"
t maturin sdist -o "$INV/sdist" 2>&1 | tail -2

echo "== main, strip only (maturin --strip)"
CARGO_TARGET_DIR=$INV/target-plain t maturin build --release --locked --strip -o "$INV/wheels/strip" 2>&1 | tail -2

echo "== main, fat LTO, 1 codegen unit, strip (fresh target)"
CARGO_TARGET_DIR=$INV/target-lto CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  CARGO_PROFILE_RELEASE_STRIP=symbols t maturin build --release --locked -o "$INV/wheels/lto" 2>&1 | tail -2

echo "== main, thin LTO, 16 codegen units, strip (fresh target)"
CARGO_TARGET_DIR=$INV/target-thin CARGO_PROFILE_RELEASE_LTO=thin \
  CARGO_PROFILE_RELEASE_STRIP=symbols t maturin build --release --locked -o "$INV/wheels/thin" 2>&1 | tail -2

for pkg in nc gpl; do
  cd "$WT/crates-$pkg" || exit 1
  seed=$LARMORX_WORKSPACE/larmorx/crates-$pkg/target/release
  mkdir -p "$INV/target-$pkg-plain"
  [ -d "$INV/target-$pkg-plain/release" ] || cp -a "$seed" "$INV/target-$pkg-plain/release"
  echo "== $pkg, plain release"
  CARGO_TARGET_DIR=$INV/target-$pkg-plain t maturin build --release --locked -o "$INV/wheels/plain" 2>&1 | tail -2
  echo "== $pkg sdist"
  t maturin sdist -o "$INV/sdist" 2>&1 | tail -2
  echo "== $pkg, fat LTO, 1 codegen unit, strip (fresh target)"
  CARGO_TARGET_DIR=$INV/target-$pkg-lto CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    CARGO_PROFILE_RELEASE_STRIP=symbols t maturin build --release --locked -o "$INV/wheels/lto" 2>&1 | tail -2
done
ls -la "$INV/wheels"/* "$INV/sdist"
echo "BUILD-INVENTORY DONE"
