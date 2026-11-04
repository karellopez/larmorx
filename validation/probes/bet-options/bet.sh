#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# probe: FSL's bet vs the replica: $1 tag, $2 input image; then bet's options after "in out"
# (the word T2 is replaced by the copied T2's name). WHO=fsl|rep|both (default both), MODE
# (default reproduce), T2= a T2 image copied in as t2.nii.gz, OT= FSLOUTPUTTYPE.
set -u
P=${PROBE_RUNS:-$(cd "$(dirname "$0")" && pwd)/runs}
NC=${LARMORX_NC_BIN:-$(cd "$(dirname "$0")/../../.." && pwd)/crates-nc/target/release/larmorx-nc}
tag=$1; IN=$2; shift 2
args=()
for a in "$@"; do if [ "$a" = T2 ]; then args+=(t2.nii.gz); else args+=("$a"); fi; done
export FSLDIR=${LARMORX_FSL_DIR:-$HOME/fsl-6.0.7.23} FSLOUTPUTTYPE=${OT:-NIFTI_GZ} PATH=/usr/bin:/bin OPENBLAS_CORETYPE=Haswell
WHO=${WHO:-both}
R=${REPDIR:-rep}
for d in fsl $R; do
  if [ "$WHO" != both ] && [ "$WHO" != $d ] && ! { [ "$WHO" = rep ] && [ $d = $R ]; }; then continue; fi
  rm -rf $P/$tag/$d && mkdir -p $P/$tag/$d
  cp "$IN" $P/$tag/$d/in.nii.gz
  if [ -n "${T2:-}" ]; then cp "$T2" $P/$tag/$d/t2.nii.gz; fi
  start=$(date +%s)
  if [ $d = fsl ]; then
    (cd $P/$tag/$d && $FSLDIR/bin/bet in out "${args[@]}" > o.txt 2> e.txt; echo $? > s.txt)
  else
    (cd $P/$tag/$d && LARMORX_UPSTREAM_DEFECTS=${MODE:-reproduce} OMP_NUM_THREADS=${THREADS:-4} $NC fsl bet in out "${args[@]}" > o.txt 2> e.txt; echo $? > s.txt)
  fi
  echo "$(( $(date +%s) - start ))" > $P/$tag/$d/time.txt
done
echo "BET-PROBE-DONE $tag"
