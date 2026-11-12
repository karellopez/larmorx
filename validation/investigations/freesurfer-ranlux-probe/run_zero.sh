#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The spec writer's observations for FS-100 and FS-101 (specs/ranlux.md §2.6, §8): the official
# mri_volsynth with a seed whose state is all zero, and with a negative chi-squared degree of
# freedom (a negative gamma shape). Each run is stopped after 20 s; exit status 124 = stopped.
# Usage: run_zero.sh <FREESURFER_HOME> <FS_LICENSE> <output folder>
set -u
export FREESURFER_HOME=${1:?FREESURFER_HOME}
export FS_LICENSE=${2:?FS_LICENSE}
OUT=${3:?output folder}
export OMP_NUM_THREADS=1
cd "$OUT"
V="$FREESURFER_HOME/bin/mri_volsynth"
timeout 20 "$V" --pdf z --dim 2 2 2 1 --seed 2147483563 --vol zero-z.mgz > zero-z.log 2>&1; echo "z seed 2147483563: exit $?"
timeout 20 "$V" --pdf chi2 --dof 4 --dim 2 2 2 1 --seed 2147483563 --vol zero-chi.mgz > zero-chi.log 2>&1; echo "chi2 seed 2147483563: exit $?"
timeout 20 "$V" --pdf z --dim 2 2 2 1 --seed 2147483564 --vol z-m1.mgz > z-m1.log 2>&1; echo "z seed 2147483564: exit $?"
timeout 20 "$V" --pdf z --dim 2 2 2 1 --seed 1 --vol z-1.mgz > z-1.log 2>&1; echo "z seed 1: exit $?"
timeout 20 "$V" --pdf chi2 --dof -3 --dim 1 1 1 1 --seed 53 --vol chi-neg.mgz > chi-neg.log 2>&1; echo "chi2 dof -3 seed 53: exit $?"
