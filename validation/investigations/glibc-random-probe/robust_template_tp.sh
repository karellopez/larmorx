#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The official mri_robust_template's random first time point (specs/glibc-random.md §7, §10): for
# each seed and number of inputs N, the program's line "Will use TP k as random initial target" and
# the value the probe predicts, (the first rand() after srand(seed)) mod N + 1. The inputs are
# 8 x 8 x 8 volumes made by the official mri_volsynth; each run is stopped after 30 s (the line comes
# before the registrations). Usage:
#   robust_template_tp.sh <FREESURFER_HOME> <FS_LICENSE> <glibc_random_probe> <output folder>
set -u
export FREESURFER_HOME=${1:?FREESURFER_HOME}
export FS_LICENSE=${2:?FS_LICENSE}
PROBE=$(realpath "${3:?probe}")
OUT=${4:?output folder}
export OMP_NUM_THREADS=1
mkdir -p "$OUT" && cd "$OUT" || exit 1
B="$FREESURFER_HOME/bin"
for i in 1 2 3 4 5 6 7; do
  [ -f "v$i.mgz" ] || "$B/mri_volsynth" --pdf z --dim 8 8 8 1 --seed "$i" --vol "v$i.mgz" > /dev/null 2>&1
done
same=0; total=0
for n in 2 3 5 7; do
  movs=$(for i in $(seq 1 "$n"); do printf 'v%d.mgz ' "$i"; done)
  for seed in 1 2 53 1234 99991 2147483647 -1; do
    # shellcheck disable=SC2086
    line=$(timeout 30 "$B/mri_robust_template" --mov $movs --template "t-$n.mgz" --satit \
      --seed "$seed" 2>&1 | grep -m1 "as random initial target")
    tp=$(sed -n 's/.*Will use TP \([0-9]*\) as random.*/\1/p' <<< "$line")
    first=$("$PROBE" rand "$seed" 1)
    want=$(( first % n + 1 ))
    total=$((total + 1)); [ "$tp" = "$want" ] && same=$((same + 1))
    echo "N=$n seed=$seed: program TP ${tp:-none}, probe $want   [$line]"
  done
done
echo "identical: $same of $total"
