#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Relay 2c (D19): the companions' golden tests on the programs their wheels install. Makes the two
# sdists (scripts/release/companion_sdist.py), builds a wheel from each with the dist profile (pip,
# build isolation: maturin from PyPI; MATURIN_PEP517_ARGS="--profile dist --locked"), installs them
# in a fresh venv of the system's CPython 3.12, prints the programs' versions, then runs both
# golden harnesses with LARMORX_GOLDEN_PROGRAM at the installed programs (a path; for larmorx-gpl
# also a name found on PATH) and the negative controls: a missing program, and (if given) a
# larmorx-gpl of another version, each of which must fail naming the reason.
#
# Usage: installed_golden.sh <checkout> <out> [<a larmorx-gpl program of another version>]
# Optional: NC_TARGET, GPL_TARGET (CARGO_TARGET_DIR for the two wheel builds, to reuse a cache).
# Run it in a memory box: scripts/orchestration/boxed.sh 32 bash installed_golden.sh ...
set -u
[ $# -ge 2 ] || { sed -n '3,13p' "$0"; exit 2; }
W=$(cd "$1" && pwd)
mkdir -p "$2" && R=$(cd "$2" && pwd)
OLD=${3:-}
source "$W/scripts/orchestration/env.sh"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-28}
unset FSLDIR LARMORX_NC_BIN LARMORX_GPL_BIN PYTHONPATH VIRTUAL_ENV
cd "$W" || exit 1
git log --oneline -1
git status --short
rm -rf "$R/sdist" "$R/wheels" "$R/venv"
mkdir -p "$R/sdist" "$R/wheels"
echo "== sdists"
python -P scripts/release/companion_sdist.py nc --out "$R/sdist" 2>&1 | tail -1
python -P scripts/release/companion_sdist.py gpl --out "$R/sdist" 2>&1 | tail -1
echo "== wheels from the sdists, dist profile"
/usr/bin/python3.12 -m venv "$R/venv"
V=$R/venv/bin
"$V/pip" install -q --upgrade pip
s=$SECONDS
for f in "$R"/sdist/*.tar.gz; do
  case $(basename "$f") in
    larmorx_nc*) td=${NC_TARGET:-} ;;
    *) td=${GPL_TARGET:-} ;;
  esac
  CARGO_TARGET_DIR=${td:-$R/target} MATURIN_PEP517_ARGS="--profile dist --locked" \
    "$V/pip" wheel -q --no-deps -w "$R/wheels" "$f" 2>&1 | tail -3
done
echo "seconds $((SECONDS - s))"
ls -l "$R/wheels" | awk 'NR > 1 {print $5, $9}'
"$V/pip" install -q --no-index "$R"/wheels/*.whl || exit 1
for c in larmorx-nc larmorx-gpl; do
  "$V/$c" --version
  file -b "$V/$c" | cut -d, -f1,2 | cut -c1-80
done
fail=0
check() { # <label> <expected: pass|fail> <command...>
  local label=$1 want=$2 log
  shift 2
  log=$("$@" 2>&1)
  local rc=$?
  echo "$log" | grep -E "^test |golden: running|test result|LARMORX_GOLDEN_PROGRAM=|mismatches|differs" | cut -c1-200 | head -8
  if { [ "$want" = pass ] && [ $rc -ne 0 ]; } || { [ "$want" = fail ] && [ $rc -eq 0 ]; }; then
    echo "UNEXPECTED: $label (exit status $rc)"
    fail=1
  else
    echo "as expected: $label"
  fi
}
echo "== larmorx-nc's golden test on the installed program"
cd "$W/crates-nc" || exit 1
check "larmorx-nc golden, installed program" pass \
  env LARMORX_GOLDEN_PROGRAM="$V/larmorx-nc" cargo test --locked -p larmorx-nc-cli --test golden -- --nocapture
check "larmorx-nc golden, a missing program" fail \
  env LARMORX_GOLDEN_PROGRAM="$R/no-such-program" cargo test --locked -p larmorx-nc-cli --test golden
echo "== larmorx-gpl's golden test on the installed program"
cd "$W/crates-gpl" || exit 1
check "larmorx-gpl golden, installed program (a path)" pass \
  env LARMORX_GOLDEN_PROGRAM="$V/larmorx-gpl" cargo test --locked -p larmorx-gpl-cli --test golden -- --nocapture
check "larmorx-gpl golden, installed program (a name on PATH)" pass \
  env PATH="$V:$PATH" LARMORX_GOLDEN_PROGRAM=larmorx-gpl cargo test --locked -p larmorx-gpl-cli --test golden -- --nocapture
check "larmorx-gpl golden, a missing program" fail \
  env LARMORX_GOLDEN_PROGRAM="$R/no-such-program" cargo test --locked -p larmorx-gpl-cli --test golden
if [ -n "$OLD" ]; then
  "$OLD" --version
  check "larmorx-gpl golden, a program of another version" fail \
    env LARMORX_GOLDEN_PROGRAM="$OLD" cargo test --locked -p larmorx-gpl-cli --test golden
fi
echo "INSTALLED-GOLDEN FINISHED status $fail"
exit $fail
