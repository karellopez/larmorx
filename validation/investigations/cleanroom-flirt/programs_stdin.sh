#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# programs_stdin.sh <dir> <larmorx binary>: the coordinate programs reading standard input, FSL
# 6.0.7.23 (run directly in a 4 GiB memory box: the oracle cache does not key standard input)
# against the clean-room original (reproduce mode). <dir> holds the fsl-flirt-replica suite's
# epi.nii and t1.nii (oracles/inputs/fsl-flirt-replica/); outputs go to <dir>/out. Prints the
# cases whose exit status or streams differ (FSL's usage names the program by its path) and a
# count. Part 2 of FLIRT's clean-room original (2026-11-02): the findings' "FLIRT's other
# programs".
D=$1
LX=$2
FSLDIR=${FSLDIR:-$HOME/fsl-6.0.7.23}
O=$D/out
mkdir -p "$O"
n=0
bad=0
box() { systemd-run --user --scope --quiet --collect -p MemoryMax=4G -p MemorySwapMax=0 -- "$@"; }
run() {
  n=$((n + 1))
  (cd "$D" && printf "$1" | box env -i HOME="$HOME" PATH=/usr/bin:/bin FSLDIR="$FSLDIR" \
    FSLOUTPUTTYPE=NIFTI_GZ OPENBLAS_CORETYPE=Haswell "$FSLDIR/bin/$2" "${@:3}" \
    >"$O/f$n.out" 2>"$O/f$n.err")
  fc=$?
  (cd "$D" && printf "$1" | box env -i HOME="$HOME" PATH=/usr/bin:/bin FSLOUTPUTTYPE=NIFTI_GZ \
    LARMORX_IMPLEMENTATION=original LARMORX_UPSTREAM_DEFECTS=reproduce "$LX" fsl "${@:2}" \
    >"$O/o$n.out" 2>"$O/o$n.err")
  oc=$?
  grep -v '^larmorx' "$O/o$n.err" >"$O/o$n.err2"
  if [ "$fc" != "$oc" ] && ! { [ "$fc" -gt 128 ] && [ "$oc" = 1 ]; }; then
    echo "DIFF exit $fc vs $oc: ${*:2}"
    bad=$((bad + 1))
  elif ! cmp -s "$O/f$n.out" "$O/o$n.out" || ! cmp -s "$O/f$n.err" "$O/o$n.err2"; then
    echo "DIFF streams: ${*:2}"
    bad=$((bad + 1))
  fi
}
P='10 20 30\n10 20 30\n5 5 5\n'
Q='10 20 30\n5 5 5\n10 20 30\n'
run "$P" std2imgcoord -img t1.nii -std epi.nii -
run "$P" std2imgcoord -img t1.nii -std epi.nii -v -
run "$Q" std2imgcoord -img t1.nii -std epi.nii ''
run "$Q" std2imgcoord -img t1.nii -std epi.nii
run "$Q" img2imgcoord -src epi.nii -dest t1.nii -
run "$Q" img2imgcoord -src epi.nii -dest t1.nii -v
run "$Q" img2stdcoord -img t1.nii -std epi.nii -v -
run "$Q" img2stdcoord -img t1.nii -std epi.nii -flirt -
run "$Q" img2stdcoord -img t1.nii -std epi.nii -mm -verbose -
run "$Q" std2imgcoord -img t1.nii -std epi.nii -vox -v -
run '1 2 3 4 5 6 7 8 9\n10 11\n' img2stdcoord -img t1.nii -std epi.nii -
run '1e400 2 3\n4 5 6\n' img2stdcoord -img t1.nii -std epi.nii -
run 'nan 2 3\ninf 5 6\n0x10 1 1\n' img2stdcoord -img t1.nii -std epi.nii -
run '' img2imgcoord -src epi.nii -dest t1.nii -help
run '' img2imgcoord -src epi.nii -dest t1.nii -frob
run '' img2imgcoord -src epi.nii -warp
run '0 0 0\n1 2 3\n' std2imgcoord -img t1.nii -std epi.nii -
run '1 2 3\n1 2 3.0000001\n4 5 6\n' std2imgcoord -img t1.nii -std epi.nii -
run '1 2 3\n4 5 6\n' std2imgcoord -img t1.nii -std epi.nii -verbose
echo "$n cases, $bad differ"
