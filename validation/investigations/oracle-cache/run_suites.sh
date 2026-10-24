#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Every parity suite (or the ones named) at one tier, in one oracle-cache mode, its records and a
# timing line per suite: the measurements of docs/validation/oracle-cache.md.
# Usage: run_suites.sh <checkout> <out> <tier> <use|refresh|off> [--jobs N] [suite ...]
#   <out>/records/<suite>.{md,json}, <out>/<suite>.log, <out>/timings.tsv
#   (suite, exit status, wall seconds, runs from the cache, runs that ran).
# A suite is "<name>" or "<name>:replica" (the GPL replica, for the AFNI suites).
# The cache is LARMORX_ORACLE_CACHE (default <workspace>/oracles/cache).
R=$1; OUT=$2; TIER=$3; MODE=$4; shift 4
JOBS=4
if [ "$1" = "--jobs" ]; then JOBS=$2; shift 2; fi
SUITES=("$@")
if [ ${#SUITES[@]} -eq 0 ]; then
  SUITES=(nifti-io itk-geometry ants-apply-transforms itk-hdf5 afni-tshift afni-tshift:replica
          afni-automask afni-automask:replica afni-unifize afni-unifize:replica resample-series
          numpy-linalg mri-hmc ants-image-math ants-threshold-image ants-multiply-images
          ants-smooth-image ants-resample-image-by-spacing ants-resample-image ants-n4
          ants-denoise ants-registration ants-registration-corrections ants-ai
          ants-affine-initializer ants-atropos)
fi
D=$(cd "$(dirname "$0")/../../../scripts/orchestration" && pwd)
mkdir -p "$OUT/records"
printf "suite\texit\tseconds\tserved\tran\n" > "$OUT/timings.tsv"
for s in "${SUITES[@]}"; do
  name=${s%%:*}; impl=original; [ "$s" != "$name" ] && impl=${s#*:}
  dir=$OUT/records; [ "$impl" != original ] && dir=$OUT/records-$impl
  mkdir -p "$dir"
  start=$(date +%s.%N)
  bash "$D/lxv.sh" "$R" parity "$name" --tier "$TIER" --implementation "$impl" --jobs "$JOBS" \
    --oracle-cache "$MODE" --out "$dir" > "$OUT/$name-$impl.log" 2>&1
  code=$?
  end=$(date +%s.%N)
  line=$(grep -m1 "^original programs:" "$OUT/$name-$impl.log")
  served=$(echo "$line" | sed -n 's/^original programs: \([0-9]*\) runs from.*/\1/p')
  ran=$(echo "$line" | sed -n 's/.*saved), \([0-9]*\) run;.*/\1/p')
  printf "%s\t%s\t%.1f\t%s\t%s\n" "$s" "$code" "$(echo "$end - $start" | bc)" "$served" "$ran" \
    | tee -a "$OUT/timings.tsv"
done
echo "ALLDONE $(date -Is)" | tee -a "$OUT/timings.tsv"
