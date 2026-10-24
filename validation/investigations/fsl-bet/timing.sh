#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Informal timings of FSL 6.0.7.17's bet, bet2 and betsurf against the larmorx-nc replica (one run
# each, wall seconds), and whether fMRIPrep 25.2.6's image can run `bet -Z` (it has no `dc`).
# Not a benchmark report. Usage (from the repository):
#   validation/investigations/fsl-bet/timing.sh <boldref> <T1w> <betsurf dir: t1.nii.gz,
#       t1_mesh.vtk, t1_to_std.mat> [<slab for -Z>]
# FSL from ~/fsl (never on the PATH); the replica from crates-nc/target/release.
set -u
NC=${LARMORX_NC_BIN:-crates-nc/target/release/larmorx-nc-bet}
SIF=${FMRIPREP_SIF:-../oracles/images/fmriprep-25.2.6.sif}
IN=$1; T1=$2; C=$3; SLAB=${4:-}
OUT=$(mktemp -d)
E="env -i HOME=$OUT PATH=/usr/bin:/bin FSLDIR=$HOME/fsl FSLOUTPUTTYPE=NIFTI_GZ"
t() { local s; s=$(date +%s.%N); (cd "$OUT" && "$@" > /dev/null 2>&1); echo "$(date +%s.%N) - $s" | bc; }
echo "fMRIPrep's call, FSL: $(t $E "$HOME/fsl/bin/bet" "$IN" fa -f 0.20 -m)"
echo "fMRIPrep's call, replica: $(t $E OMP_NUM_THREADS=1 "$NC" fsl bet "$IN" ra -f 0.20 -m)"
echo "T1w bet2 -m, FSL: $(t $E "$HOME/fsl/bin/bet2" "$T1" ft -m)"
echo "T1w bet2 -m, replica: $(t $E OMP_NUM_THREADS=1 "$NC" fsl bet2 "$T1" rt -m)"
echo "betsurf --t1only -m, FSL: $(t $E "$HOME/fsl/bin/betsurf" --t1only -m "$C/t1.nii.gz" "$C/t1_mesh.vtk" "$C/t1_to_std.mat" fs)"
echo "betsurf --t1only -m, replica 1 thread: $(t $E OMP_NUM_THREADS=1 "$NC" fsl betsurf --t1only -m "$C/t1.nii.gz" "$C/t1_mesh.vtk" "$C/t1_to_std.mat" rs)"
echo "betsurf --t1only -m, replica 3 threads: $(t $E OMP_NUM_THREADS=3 "$NC" fsl betsurf --t1only -m "$C/t1.nii.gz" "$C/t1_mesh.vtk" "$C/t1_to_std.mat" rs3)"
if [ -n "$SLAB" ] && [ -f "$SIF" ]; then
  echo "fMRIPrep's image, bet -Z:"
  apptainer exec --cleanenv --no-home --bind "$OUT:$OUT" --bind "$(dirname "$SLAB"):$(dirname "$SLAB")" \
    --env FSLDIR=/app/.pixi/envs/fmriprep --env FSLOUTPUTTYPE=NIFTI_GZ --env PATH=/usr/bin:/bin \
    "$SIF" /app/.pixi/envs/fmriprep/bin/bet "$SLAB" "$OUT/outz" -Z -m 2>&1 | grep -v '^WARNING' | tail -2
  apptainer exec --cleanenv --no-home "$SIF" bash -c 'command -v dc || echo "no dc on the image'"'"'s PATH"'
fi
uptime
