#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Build libcrmath.so: glibc's expf, logf, exp, log, sin, cos, sincos, sinf, cosf and sincosf
# replaced by CORE-MATH's correctly rounded functions, for LD_PRELOAD into an oracle program.
# (sinf, cosf and sincosf since 2026-10-11: Euler3DTransform<float> calls them under
# antsApplyTransforms --float 1.)
#
# larmorx's transcendental functions are correctly rounded (CLAUDE.md rule 5); glibc's are not
# always, and ITK calls glibc's. Running an ANTs oracle with this library preloaded gives ANTs'
# result with correctly rounded math, which a replica must reproduce bit for bit; the
# difference between that run and the plain one is the effect of glibc's rounding
# (docs/findings/platform-math.md, docs/findings/ants-n4.md).
# - Compiles the pinned CORE-MATH sources (<workspace>/tags/core-math, MIT) read-only, with
#   the system gcc, -O2, no -march.
# - The wrapper defines the glibc names; the dynamic linker binds the oracle's versioned
#   references (expf@GLIBC_2.27, ...) to these unversioned definitions.
#
# Usage:
#   scripts/build_crmath_preload.sh [--core-math <dir>] [--out <dir>]
# Output: <out>/libcrmath.so, <out>/BUILD_INFO.txt. Then:
#   LD_PRELOAD=<out>/libcrmath.so <oracle program> ...
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../.." && pwd)"
CM="$WS/tags/core-math"
OUT="$WS/oracles/core-math-preload"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --core-math) CM="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    -h|--help) sed -n '2,19p' "$0"; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
[[ -f "$CM/src/binary32/exp/expf.c" ]] || { echo "CORE-MATH not found at $CM" >&2; exit 1; }

CLEANPATH="$(echo "$PATH" | tr ':' '\n' | grep -v '/fsl' | paste -sd:)"
export PATH="$CLEANPATH"
CC=/usr/bin/gcc
mkdir -p "$OUT/obj"

# One object per CORE-MATH function (static helpers stay private to their file).
"$CC" -O2 -fPIC -c "$CM/src/binary32/exp/expf.c" -o "$OUT/obj/expf.o"
"$CC" -O2 -fPIC -c "$CM/src/binary32/log/logf.c" -o "$OUT/obj/logf.o"
"$CC" -O2 -fPIC -c "$CM/src/binary64/exp/exp.c" -o "$OUT/obj/exp.o"
"$CC" -O2 -fPIC -I"$CM/src/binary64/log" -c "$CM/src/binary64/log/log.c" -o "$OUT/obj/log.o"
"$CC" -O2 -fPIC -c "$CM/src/binary64/sin/sin.c" -o "$OUT/obj/sin.o"
"$CC" -O2 -fPIC -c "$CM/src/binary64/cos/cos.c" -o "$OUT/obj/cos.o"
"$CC" -O2 -fPIC -c "$CM/src/binary32/sin/sinf.c" -o "$OUT/obj/sinf.o"
"$CC" -O2 -fPIC -c "$CM/src/binary32/cos/cosf.c" -o "$OUT/obj/cosf.o"

cat > "$OUT/obj/preload.c" <<'EOF'
/* glibc's names for CORE-MATH's correctly rounded functions (LD_PRELOAD). */
float cr_expf(float);
float cr_logf(float);
double cr_exp(double);
double cr_log(double);
double cr_sin(double);
double cr_cos(double);
float cr_sinf(float);
float cr_cosf(float);
float expf(float x) { return cr_expf(x); }
float logf(float x) { return cr_logf(x); }
double exp(double x) { return cr_exp(x); }
double log(double x) { return cr_log(x); }
double sin(double x) { return cr_sin(x); }
double cos(double x) { return cr_cos(x); }
void sincos(double x, double *s, double *c) { *s = cr_sin(x); *c = cr_cos(x); }
float sinf(float x) { return cr_sinf(x); }
float cosf(float x) { return cr_cosf(x); }
void sincosf(float x, float *s, float *c) { *s = cr_sinf(x); *c = cr_cosf(x); }
EOF
"$CC" -O2 -fPIC -fno-builtin -c "$OUT/obj/preload.c" -o "$OUT/obj/preload.o"
"$CC" -shared -o "$OUT/libcrmath.so" "$OUT"/obj/*.o -lm

{
  echo "CORE-MATH: $CM ($(git -C "$CM" rev-parse HEAD 2>/dev/null || echo 'no git metadata'))"
  echo "compiler: $($CC --version | head -1)"
  echo "functions: expf logf exp log sin cos sincos sinf cosf sincosf"
  echo "built: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$OUT/BUILD_INFO.txt"
echo "built $OUT/libcrmath.so"
