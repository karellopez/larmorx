#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The permanent cases of the FSL version trial's BET findings (triaged 2026-10-29, README.md):
# one per distinct cause, written into validation/difftest/cases/fsl-bet/ by `difftest
# --promote` (the shrunk recipe; case 141 from its drawn recipe, whose shrunk one also meets
# Armadillo 14.2.1's sign of zero, which waits for the Armadillo part).
# Usage: make_permanent_cases.sh <the trial's findings/fsl-bet directory> <141's drawn finding>
set -eu
here=$(cd "$(dirname "$0")/../../.." && pwd)
f=$1
drawn141=$2
lxv() { "$here/scripts/orchestration/lxv.sh" "$here" "$@"; }
lxv difftest --promote "$f/20261029-00051-replica.json" --id bet2-n-crash-nothing-asked \
  --tier smoke --status fixed \
  --description "bet2 -n alone (no output asked) where FSL crashes in the mask's fill (a constant image: the surface's centre is not a number, FSL-43): reproduce fills and writes nothing, as the call asks; the suite had required an output (found by the FSL version trial, 2026-10-29)"
lxv difftest --promote "$drawn141" --id bet-robust-mesh-echo \
  --tier smoke --status register --defect FSL-63 --rule A \
  --description "bet -R -e: the script passes bet2's options through bash's echo, which takes the leading -e as its own option, so FSL writes no mesh (FSL-63); the default writes it, reproduce drops -e as FSL does (found by the FSL version trial, 2026-10-29)"
lxv difftest --promote "$f/20261029-00661-replica.json" --id bet2-fill-by-index \
  --tier smoke --status fixed --defect FSL-43 \
  --description "bet2 -r 60 on a 3 x 6 x 4 head: the mask's fill starts four rows before the image; FSL reaches each point by its linear index, other voxels of the volume, and fills every voxel; reproduce had skipped the fill as for a crash (FSL-43; found by the FSL version trial, 2026-10-29)"
lxv difftest --promote "$f/20261029-01297-replica.json" --id bet-fill-by-index \
  --tier smoke --status fixed --defect FSL-43 \
  --description "bet -f 0.7 -g -0.3 -m -o on a 22 x 18 x 21 image of 0.01 mm slices: the mask's fill starts 86 slices above the image and reaches the volume by FSL's linear index; inside bet, reproduce had corrected FSL-43 instead (found by the FSL version trial, 2026-10-29)"
lxv difftest --promote "$f/20261029-01308-replica.json" --id bet-robust-crash-filled \
  --tier smoke --status fixed --defect FSL-43 \
  --description "bet -R -g 0.5 -e: FSL's second bet2 crashes in a fill that starts far above the image, and the script's ERR trap prints its failure after the crash; reproduce goes on (FSL-43's fill by its linear index) and the suite compares what FSL printed before its crash (found by the FSL version trial, 2026-10-29)"
lxv difftest --promote "$f/20261029-00080-replica.json" --id bet2-fill-memory-beyond \
  --tier smoke --status register --defect FSL-43 \
  --description "bet2 -c 22 19 8 on a 3 x 4 x 8 ramp: the fill starts outside the image and FSL reads memory beyond the volume (6.0.7.17 crashes, 6.0.7.23 writes values that depend on that memory); reproduce reads it as not part of the mask and names it, the suite judges FSL's values there undefined (found by the FSL version trial, 2026-10-29)"
echo "PERMANENT-DONE"
