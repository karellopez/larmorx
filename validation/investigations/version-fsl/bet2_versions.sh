#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# bet2 of FSL 6.0.7.17 and 6.0.7.23 on one input with several option sets: for each, whether
# the two versions' brain masks hold the same values and their verbose self-intersection lines
# (to bisect which option makes the versions differ; the version trial's case 80).
# Usage: bet2_versions.sh <input image> <work dir> "<options 1>" ["<options 2>" ...]
set -u
input=$1
work=$2
shift 2
mkdir -p "$work"
n=0
for opts in "$@"; do
  n=$((n + 1))
  for v in 17 23; do
    if [ "$v" = 17 ]; then fsldir=$HOME/fsl; else fsldir=$HOME/fsl-6.0.7.23; fi
    d="$work/$n-$v"
    rm -rf "$d"
    mkdir -p "$d"
    # shellcheck disable=SC2086
    (cd "$d" && env -i HOME="$HOME" PATH=/usr/bin:/bin FSLDIR="$fsldir" FSLOUTPUTTYPE=NIFTI \
      "$fsldir/bin/bet2" "$input" out -m -v $opts >stdout.txt 2>stderr.txt)
  done
  a=$(tail -c +353 "$work/$n-17/out_mask.nii" | sha256sum | cut -c1-12)
  b=$(tail -c +353 "$work/$n-23/out_mask.nii" | sha256sum | cut -c1-12)
  s17=$(grep -m1 self-intersection "$work/$n-17/stdout.txt")
  s23=$(grep -m1 self-intersection "$work/$n-23/stdout.txt")
  if [ "$a" = "$b" ]; then same=same; else same=DIFFER; fi
  echo "[$opts] mask $same | 6.0.7.17: $s17 | 6.0.7.23: $s23"
done
