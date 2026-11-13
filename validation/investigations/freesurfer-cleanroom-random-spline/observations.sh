#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The clean-room implementer's black-box observations behind docs/findings/ranlux-clean-room.md and
# cubic-spline-clean-room.md (2026-11-13), from the prebuilt probes and the clean room's probe:
#  1. which multiples m * 2147483563 (m = -12 to 12) leave RANLUX's seeded state all zero;
#  2. the draws that return with the all-zero stream (integer gamma shapes of 12 or more);
#  3. the input forms of fs_spline_probe's `cases` and `d3`, read from its outputs.
# One-off observations outside the oracle-output cache (the suites, compare_*.py, use it).
# Usage: observations.sh <workspace> <cleanroom-probe>
set -u
W=${1:?workspace}
C=${2:?cleanroom-probe}
P=$W/oracles/freesurfer-8.2.0-unit/probes/fs_rng_probe
Q=$W/oracles/freesurfer-8.2.0-unit/probes/fs_spline_probe
echo "== 1. all-zero seeded states (probe, clean room under --reproduce)"
for m in $(seq -12 12); do
  s=$((m * 2147483563))
  a=$("$P" raw "$s" 48 | sort -u | tr '\n' ' ')
  b=$("$C" --reproduce raw "$s" 48 2>/dev/null | sort -u | tr '\n' ' ')
  echo "m=$m seed=$s probe_zero=$([ "$a" = "0 " ] && echo yes || echo no) cleanroom_zero=$([ "$b" = "0 " ] && echo yes || echo no)"
done
echo "== 2. seed 2147483563: draws that return (each stopped after 5 s)"
while read -r c; do
  echo "$c: probe $(timeout 5 "$P" dist $c | tr '\n' ' ')| clean room $("$C" --reproduce dist $c 2>&1 | tr '\n' ' ')"
done <<'X'
gamma 2147483563 2 12 1
gamma 2147483563 2 25 0.5
gamma_large 2147483563 2 30
chisq 2147483563 2 24
fdist 2147483563 2 24 24
gamma 2147483563 1 12.5 1
X
echo "== 3. fs_spline_probe's input forms"
echo "cases: n ibcbeg ybcbeg ibcend ybcend m t[n] y[n] x[m] (one case per line; floats as bits)"
echo "2 1 00000000 1 00000000 3 3f800000 40a00000 40c00000 00000000 3f000000 40000000 40c00000" | "$Q" cases
echo "d3: n a[3n] b[n] (a in band storage: column j = super above, diagonal, sub below)"
echo "3 3f800000 40000000 40400000 40800000 40a00000 40c00000 40e00000 41000000 41100000 41200000 41300000 41400000" | "$Q" d3
echo "2 3f800000 00000000 40400000 40800000 40a00000 40c00000 40e00000 41000000" | "$Q" d3
