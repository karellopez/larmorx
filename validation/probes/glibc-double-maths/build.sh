#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds the probe and CORE-MATH's C cr_tan and cr_atan as one shared library.
# Usage: build.sh <out-dir> [<core-math clone>]   (default: <workspace>/tags/core-math)
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=${1:?usage: build.sh <out-dir> [<core-math clone>]}
cm=${2:-$here/../../../../tags/core-math}
mkdir -p "$out"
gcc -O2 -ffp-contract=off -fPIC -shared -o "$out/libcrmath.so" \
  "$cm/src/binary64/tan/tan.c" "$cm/src/binary64/atan/atan.c" -lm
CARGO_TARGET_DIR="$out/target" cargo build --release --offline --manifest-path "$here/Cargo.toml"
cp "$out/target/release/glibc-double-maths" "$out/"
echo "built $out/glibc-double-maths and $out/libcrmath.so"
