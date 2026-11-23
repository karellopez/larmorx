#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# How glibc 2.35's x86-64 log2 was found to be Arm's log2.c without fused operations (black box).
# Usage: build.sh <out-dir> <optimized-routines-v19.11>/math   (Arm's MIT sources: log2.c, log2_data.c)
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=${1:?usage: build.sh <out-dir> <arm math folder>}; arm=${2:?arm math folder}
mkdir -p "$out"; cp "$here"/*.c "$here"/math_config.h "$out"/; cp "$arm"/log2.c "$arm"/log2_data.c "$out"/
cd "$out"
# the non-FMA form (HAVE_FAST_FMA 0, no FMA code): the one equal to glibc's
gcc -O2 -fPIC -DHAVE_FAST_FMA=0 -c log2.c -o log2_sse2.o && objcopy --redefine-sym log2=arm_log2 log2_sse2.o
gcc -O2 -fPIC -DHAVE_FAST_FMA=0 -c log2_data.c -o log2_data_nofma.o
gcc -O2 -fPIC -c err.c -o err.o
gcc -shared -o libarmlog2.so log2_sse2.o log2_data_nofma.o err.o
gcc -O2 -o cmp cmp.c -L. -larmlog2 -Wl,-rpath,"$out" -lm
# the FMA form, for the counter-check (175,963 of 402,653,199 differ)
gcc -O2 -mfma -mavx2 -fPIC -c log2.c -o log2_fma.o && objcopy --redefine-sym log2=arm_log2 log2_fma.o
gcc -O2 -fPIC -c log2_data.c -o log2_data_fma.o
gcc -shared -o libarmlog2fma.so log2_fma.o log2_data_fma.o err.o
gcc -O2 -o cmp_fma cmp.c -L. -l:libarmlog2fma.so -Wl,-rpath,"$out" -lm
# the searches over implicit fusion sites (near 1; main path in both forms)
gcc -O2 -ffp-contract=off -mfma -o search search.c log2_data_fma.o -lm
gcc -O2 -ffp-contract=off -mfma -o search2 search2.c log2_data_fma.o -lm
gcc -O2 -ffp-contract=off -mfma -o search3 search3.c log2_data_nofma.o -lm
gcc -O2 -ffp-contract=off -mfma -o near1 near1.c log2_data_fma.o -lm
echo "built in $out: ./cmp 67108864 (0 differ), ./cmp_fma 67108864 (175,963 differ), ./search3 400000"
