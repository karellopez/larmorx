#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# FSL 6.0.7.23's fast on its command-line, option-value and failure paths: exit status, the
# head of standard output and error, the files written. Usage: probe.sh <work dir> (inputs from
# <workspace>/oracles/inputs/fsl-fast-replica, made by the fsl-fast-replica suite). Run it in a
# memory box: scripts/orchestration/boxed.sh 8 validation/probes/fsl-fast-cli/probe.sh <dir>.
set -u
# the workspace: LARMORX_WORKSPACE (scripts/orchestration/env.sh), else the checkout's parent
WS=${LARMORX_WORKSPACE:-$(cd "$(dirname "$0")/../../../.." && pwd)}
IN=$WS/oracles/inputs/fsl-fast-replica
export FSLDIR=${LARMORX_FSLDIR:-$HOME/fsl-6.0.7.23} FSLOUTPUTTYPE=NIFTI_GZ PATH=/usr/bin:/bin
export OPENBLAS_CORETYPE=Haswell
F=$FSLDIR/bin/fast
mkdir -p "$1" && cd "$1" || exit 1
run() {
    echo "=== ${*#"$F"}"
    rm -rf w && mkdir w
    (cd w && ulimit -v 8000000 && timeout 300 "$@") > o.txt 2> e.txt
    echo "exit $?"
    echo "--- stdout: $(head -c 600 o.txt | tr '\n' '|')"
    echo "--- stderr: $(head -c 600 e.txt | tr '\n' '|')"
    echo "--- files: $(ls w | tr '\n' ' ')"
}
# the usage, the option errors, the checks before any image is read
run $F
run $F -h
run $F --bogus x
run $F -n abc x
run $F -n 2.5 "$IN/const.nii.gz"
run $F -n
run $F --pvestep=10 x
run $F -H -1 x
run $F -S 0 x
# images: missing, constant, all zero; an option after the input; -S with too few images
run $F nothere
run $F -o c "$IN/const.nii.gz"
run $F -o z "$IN/zeros.nii.gz"
run $F -o s "$IN/mni4_t1.nii.gz" -n 2
run $F -S 2 "$IN/t1_half.nii.gz"
# values without meaning (FSL-99): every voxel class 1, NaN statistics, aborts
for o in "-H nan" "-H inf" "-l 0" "-l -5" "-f nan" "-R nan"; do
    run $F $o -o seg "$IN/mni4_t1.nii.gz"
done
run $F -o a -n 0 -N --nopve "$IN/t1_half.nii.gz"
run $F -o a -n 0 -N -W 1 -O 1 "$IN/t1_half.nii.gz"
run $F -o a -n -1 -N --nopve "$IN/t1_half.nii.gz"
run $F -o a -n -2 -N --nopve "$IN/t1_half.nii.gz"
run $F -o a -S 2 -n 0 -N --nopve -W 1 -O 1 -I 0 "$IN/mni4_t2.nii.gz" "$IN/mni4_t1.nii.gz"
# a singular covariance (two identical channels); one class
run $F -o a -S 2 -N --nopve -W 1 -O 1 -I 0 "$IN/mni4_t1.nii.gz" "$IN/mni4_t1.nii.gz"
run $F -o a -n 1 -N -W 1 -O 1 "$IN/t1_half.nii.gz"
# verbose texts: -s with no iterations (NaN volumes), -t other than 1 to 3, -O 1's beta
run $F -o s -n 2 -s "$IN/t1_means_two.txt" --nopve -N -I 0 -O 0 -W 0 -v "$IN/t1_half.nii.gz"
run $F -o t -t 7 -v -N --nopve -I 0 -O 1 -W 1 "$IN/t1_half.nii.gz"
# priors with a class count other than 2 or 3; FSLDIR unset (FSL-100)
run $F -o u -a "$IN/ident.mat" -n 4 -N --nopve -O 1 -W 1 "$IN/t1_half.nii.gz"
run env -u FSLDIR $F -o seg "$IN/mni4_t1.nii.gz"
