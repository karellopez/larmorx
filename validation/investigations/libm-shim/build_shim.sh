#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds libm_shim.so (LD_PRELOAD) from libm_shim.c and CORE-MATH's correctly rounded functions (MIT; <workspace>/tags/core-math, the commit of
# crates/larmorx-core/src/math/UPSTREAM.md). Usage: build_shim.sh <workspace> <output folder>
# (the objects stay in <output folder>/obj: ../freesurfer-libm-inventory/README.md links libm_rates
# against them)
set -euo pipefail
W=${1:?workspace}
OUT=${2:?output folder}
HERE=$(cd "$(dirname "$0")" && pwd)
C="$W/tags/core-math/src"
CC=/usr/bin/gcc
mkdir -p "$OUT/obj"
for f in binary64/acos/acos binary64/asin/asin binary64/atan/atan binary64/atan2/atan2 \
         binary64/tan/tan binary64/log10/log10 binary64/log2/log2 binary64/hypot/hypot \
         binary64/erf/erf binary64/erfc/erfc binary64/expm1/expm1 binary64/log1p/log1p \
         binary64/tgamma/tgamma binary32/acos/acosf binary32/asin/asinf binary32/log10/log10f \
         binary32/log2/log2f binary32/tanh/tanhf binary32/hypot/hypotf; do
  "$CC" -O2 -fPIC -fno-builtin -I"$C/$(dirname "$f")" -c "$C/$f.c" -o "$OUT/obj/$(basename "$f").o"
done
"$CC" -O2 -fPIC -fno-builtin -shared -o "$OUT/libm_shim.so" "$HERE/libm_shim.c" "$OUT"/obj/*.o -ldl -lm
echo "built $OUT/libm_shim.so"
