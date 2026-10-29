#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The permanent cases of the two BET findings of the FSL version trial that waited for Armadillo
# 14.2.1's LAPACK path (validation/investigations/version-fsl/README.md): case 80 as drawn (bet2 -c
# on an image stored neurologically: the centre goes through the solve of the nifti-to-newimage
# voxel matrix) and case 141 shrunk (fslstats -C prints -0.000000, which bet -R's dc reads as a
# subtraction). Written into validation/difftest/cases/fsl-bet/ by `difftest --promote`.
# Usage: make_permanent_cases.sh <the trial's findings/fsl-bet directory> <scratch directory>
set -eu
here=$(cd "$(dirname "$0")/../../.." && pwd)
f=$1
scratch=$2
mkdir -p "$scratch"
lxv() { "$here/scripts/orchestration/lxv.sh" "$here" "$@"; }
# --promote writes the finding's shrunk recipe: case 80's drawn one goes in its place.
python3 -I - "$f/20261029-00080-replica.json" "$scratch/80-drawn.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
d["shrunk"], d["shrunk_checks"], d["shrunk_detail"], d["shrink_moves"] = (
    d["recipe"], d["checks"], d["detail"], [])
json.dump(d, open(sys.argv[2], "w"), indent=1)
PY
lxv difftest --promote "$scratch/80-drawn.json" --id bet2-centre-neurological-solve \
  --tier smoke --status fixed \
  --description "bet2 -c 22 19 8 -m -w 0.5 on a 30 x 32 x 16 image of 1.2 x 0.8 x 1.2 mm stored neurologically: the centre goes through the solve of the nifti-to-newimage voxel matrix, which FSL 6.0.7.23's Armadillo 14.2.1 does by LAPACK (18 of 15,360 voxels differ from 6.0.7.17); the replica follows it since 2026-10-29 (found by the FSL version trial, case 80 as drawn)"
lxv difftest --promote "$f/20261029-00141-replica.json" --id bet-robust-cog-minus-zero \
  --tier smoke --status fixed \
  --description "bet -R -e on a 1 x 1 x 2 image stored neurologically: FSL 6.0.7.23's fslstats -C prints the centre -0.000000 (Armadillo 14.2.1's solve through LAPACK), which the script's dc reads as a subtraction (dc: stack empty, twice); the replica's fslstats and its dc do the same since 2026-10-29 (found by the FSL version trial, case 141 shrunk)"
echo "PERMANENT-DONE"
