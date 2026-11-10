#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 1 of the first release: two measurements for the design, kept for reproducibility.
#  A. The size profile (fat LTO, one codegen unit, strip) at today's version: do the golden tests
#     of the three packages still pass? (the Python golden suite on the LTO wheel's extension; the
#     Rust golden tests of the AFNI replicas, larmorx-nc and larmorx-gpl built with the profile)
#  B. The version: which golden cases depend on the package version? Control at 0.0.1 (release
#     mode), then the three workspaces at 0.1.0-rc.1 (edits reverted at the end).
set -u
WT=${1:?usage: $0 <checkout> <output dir>}
INV=${2:?usage: $0 <checkout> <output dir>}
source "$WT/scripts/orchestration/env.sh"   # PATH (FSL off), PYO3_PYTHON, OPENBLAS_CORETYPE, LARMORX_VENV
export CARGO_BUILD_JOBS=28 OPENBLAS_CORETYPE=Haswell
PY=${LARMORX_VENV:?set LARMORX_VENV (scripts/orchestration/env.sh)}/bin/python
LTO=(CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_STRIP=symbols)
t() { local s=$SECONDS; "$@"; local rc=$?; echo "TIME $(( SECONDS - s )) s rc=$rc: ${*:1:6}"; return $rc; }
so_from() { rm -rf "$INV/unz"; mkdir -p "$INV/unz"; (cd "$INV/unz" && unzip -q "$1" 'larmorx/_core.abi3.so'); cp "$INV/unz/larmorx/_core.abi3.so" "$WT/python/larmorx/_core.abi3.so"; }
pygolden() {
  (cd "$WT" && PYTHONPATH=$WT/python $PY -c "import larmorx, larmorx._core as c; print('imported', larmorx.__file__, c.__version__)")
  (cd "$WT" && LARMORX_GOLDEN_MISMATCHES=$INV/mismatches-$1 PYTHONPATH=$WT/python $PY -m pytest -q -p no:cacheprovider -rf tests/golden 2>&1 | grep -E "^FAILED|passed|failed|error" | sed 's/ - .*//' | tail -40)
}
rgolden() { # <workspace dir> <target dir> <package> <test> [env...]
  local ws=$1 td=$2 pkg=$3 test=$4; shift 4
  (cd "$ws" && env CARGO_TARGET_DIR="$td" "$@" cargo test --release -q -p "$pkg" --test "$test" 2>&1 | grep -E "^test result|FAILED|failed|panicked|mismatch" | head -20)
}

echo "== A1. Python golden + smoke on the LTO wheel (0.0.1)"
so_from "$(ls "$INV"/wheels/lto/larmorx-*.whl)"; t pygolden lto
echo "== A2. Rust golden, AFNI replicas, LTO profile"
t rgolden "$WT" "$INV/target-lto" larmorx-cli afni_replica_golden "${LTO[@]}"
echo "== A3. Rust golden, larmorx-nc, LTO profile"
t rgolden "$WT/crates-nc" "$INV/target-nc-lto" larmorx-nc-cli golden "${LTO[@]}"
echo "== A4. Rust golden, larmorx-gpl, LTO profile"
t rgolden "$WT/crates-gpl" "$INV/target-gpl-lto" larmorx-gpl-cli golden "${LTO[@]}"

echo "== B0. control: Rust golden in release mode at 0.0.1"
t rgolden "$WT" "$INV/target-plain" larmorx-cli afni_replica_golden
t rgolden "$WT/crates-nc" "$INV/target-nc-plain" larmorx-nc-cli golden
t rgolden "$WT/crates-gpl" "$INV/target-gpl-plain" larmorx-gpl-cli golden
echo "== B0. control: Python golden + smoke on the plain wheel (0.0.1)"
so_from "$(ls "$INV"/wheels/plain/larmorx-*.whl)"; t pygolden plain

echo "== B1. bump the three workspaces to 0.1.0-rc.1 (reverted below)"
for f in "$WT/Cargo.toml" "$WT/crates-gpl/Cargo.toml" "$WT/crates-nc/Cargo.toml"; do
  sed -i -e 's/^version = "0\.0\.1"/version = "0.1.0-rc.1"/' -e 's/version = "0\.0\.1" }/version = "0.1.0-rc.1" }/' "$f"
  grep -c '0.1.0-rc.1' "$f"
done
cd "$WT" || exit 1
CARGO_TARGET_DIR=$INV/target-plain t maturin build --release -o "$INV/wheels/bump" 2>&1 | tail -1
so_from "$(ls "$INV"/wheels/bump/larmorx-*.whl)"
echo "== B2. Python golden + smoke at 0.1.0-rc.1 (the extension only; the metadata stays 0.0.1 in this tree)"
t pygolden bump
echo "== B3. Rust golden at 0.1.0-rc.1"
t rgolden "$WT" "$INV/target-plain" larmorx-cli afni_replica_golden
t rgolden "$WT/crates-nc" "$INV/target-nc-plain" larmorx-nc-cli golden
t rgolden "$WT/crates-gpl" "$INV/target-gpl-plain" larmorx-gpl-cli golden
echo "== B4. the version test of larmorx-core"
(cd "$WT" && CARGO_TARGET_DIR=$INV/target-plain cargo test --release -q -p larmorx-core --lib 2>&1 | grep -E "^test result|FAILED" | head -5)

echo "== revert"
git -C "$WT" checkout -- Cargo.toml Cargo.lock crates-gpl/Cargo.toml crates-gpl/Cargo.lock crates-nc/Cargo.toml crates-nc/Cargo.lock
rm -f "$WT/python/larmorx/_core.abi3.so"
git -C "$WT" status --short | head
echo "GOLDEN-EXPERIMENTS DONE"
