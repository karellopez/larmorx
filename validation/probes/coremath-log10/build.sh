#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the probe and CORE-MATH's C cr_log10 as a shared library.
# Usage: build.sh <out-dir> [<core-math clone>]   (default: <workspace>/tags/core-math)
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=${1:?usage: build.sh <out-dir> [<core-math clone>]}
cm=${2:-$here/../../../../tags/core-math}
mkdir -p "$out"
gcc -O2 -ffp-contract=off -fPIC -shared -o "$out/libcrlog10.so" \
  "$cm/src/binary64/log10/log10.c" -lm
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/coremath-log10" "$out/"
echo "built $out/coremath-log10 and $out/libcrlog10.so"
