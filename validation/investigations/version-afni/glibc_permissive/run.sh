#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Does a permissive source give glibc 2.35's sinf and cosf bit for bit? Arm's Optimized Routines
# v19.11 (MIT), compiled as glibc compiles its two x86-64 builds, against the C library on all
# 2^32 inputs of each: (1) without FMA, against glibc's SSE2 build (forced by GLIBC_TUNABLES);
# (2) with -mfma -mavx2, against glibc's FMA build. Linux x86-64 with glibc 2.35 and an
# FMA/AVX2 CPU only. Usage: run.sh <work dir>   (fetches the sources with fetch_aor.sh)
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=${1:?usage: run.sh <work dir>}
src=$work/aor-v19.11
[ -f "$src/sinf.c" ] || bash "$here/fetch_aor.sh" "$src"
[ -f "$src/math_errf.c" ] || curl -sfL -o "$src/math_errf.c" \
  https://raw.githubusercontent.com/ARM-software/optimized-routines/v19.11/math/math_errf.c
echo "glibc: $(ldd --version | head -1)"
for build in sse2 fma; do
  flags="-O2 -fno-builtin -ffp-contract=fast"
  [ "$build" = fma ] && flags="$flags -mfma -mavx2"
  out=$work/$build
  mkdir -p "$out"
  for f in sinf cosf sincosf_data math_errf; do
    gcc $flags -I"$src" -Dsinf=aor_sinf -Dcosf=aor_cosf -c "$src/$f.c" -o "$out/$f.o"
  done
  gcc -O2 -fno-builtin -fopenmp "$here/compare.c" "$out"/*.o -o "$out/compare" -lm
  tun=""
  [ "$build" = sse2 ] && tun="glibc.cpu.hwcaps=-FMA,-AVX2"
  for fn in sin cos; do
    echo -n "$build build: "
    GLIBC_TUNABLES=$tun "$out/compare" "$fn" || true
  done
done
