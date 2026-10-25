#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# A CORE-MATH preload with the double-precision functions only (exp, log, sin, cos, sincos,
# asin, acos, atan2, cbrt, pow): glibc's float functions stay. For an FSL oracle whose replica
# carries glibc's float functions (larmorx-nc-core's glibc_float.rs) and correctly rounded
# double ones: mcflirt run with this preload computes what the replica computes, so a
# difference that disappears with it is glibc's double rounding.
#
# Reuses the objects of scripts/build_crmath_preload.sh (<workspace>/oracles/core-math-preload/obj,
# read only). Usage: build_crmath_double_preload.sh <out dir>   -> <out>/libcrmath-double.so
set -euo pipefail
WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
[[ -d "$WS/oracles" ]] || WS="${LARMORX_WORKSPACE:?set LARMORX_WORKSPACE}"
OBJ="$WS/oracles/core-math-preload/obj"
OUT="$1"
mkdir -p "$OUT"
cat > "$OUT/preload-double.c" <<'EOF'
/* glibc's double-precision names for CORE-MATH's correctly rounded functions (LD_PRELOAD). */
double cr_exp(double);
double cr_log(double);
double cr_sin(double);
double cr_cos(double);
double cr_asin(double);
double cr_acos(double);
double cr_atan2(double, double);
double cr_cbrt(double);
double cr_pow(double, double);
double exp(double x) { return cr_exp(x); }
double log(double x) { return cr_log(x); }
double sin(double x) { return cr_sin(x); }
double cos(double x) { return cr_cos(x); }
void sincos(double x, double *s, double *c) { *s = cr_sin(x); *c = cr_cos(x); }
double asin(double x) { return cr_asin(x); }
double acos(double x) { return cr_acos(x); }
double atan2(double y, double x) { return cr_atan2(y, x); }
double cbrt(double x) { return cr_cbrt(x); }
double pow(double x, double y) { return cr_pow(x, y); }
EOF
/usr/bin/gcc -O2 -fPIC -fno-builtin -c "$OUT/preload-double.c" -o "$OUT/preload-double.o"
/usr/bin/gcc -shared -o "$OUT/libcrmath-double.so" "$OUT/preload-double.o" \
  "$OBJ"/exp.o "$OBJ"/log.o "$OBJ"/sin.o "$OBJ"/cos.o "$OBJ"/asin.o "$OBJ"/acos.o \
  "$OBJ"/atan2.o "$OBJ"/cbrt.o "$OBJ"/pow.o -lm
echo "built $OUT/libcrmath-double.so"
