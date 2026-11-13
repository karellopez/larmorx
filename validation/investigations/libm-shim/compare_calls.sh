#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Compares the outputs of two fmriprep_calls.sh folders (e.g. count and cr-log2-only) with the
# FreeSurfer replay tool's comparison (provenance aside; the two folders' paths taken as one place).
# Usage: compare_calls.sh <larmorx checkout> <python> <folder A> <folder B>
# Prints one line per output file present in both work folders (the programs' logs *.out skipped).
set -u
[ $# -eq 4 ] || { sed -n '3,6p' "$0"; exit 2; }
L=$1 PY=$2 A=$3 B=$4
for f in "$A"/work/*; do
  n=$(basename "$f")
  case $n in *.out|*.log|*~|bbreg.dat.sum) continue;; esac
  [ -e "$B/work/$n" ] || continue
  r=$(PYTHONPATH=$L/validation/src "$PY" -m larmorx_validation freesurfer-replay compare \
      --pair "$A=$B" "$f" "$B/work/$n" 2>&1 | tail -1)
  echo "$n: $r"
done
