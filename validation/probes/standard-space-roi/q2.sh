#!/usr/bin/env bash
# probe: FSL's standard_space_roi vs the replica: $1 tag, $2 input, then args after "in out"
set -u
P=${PROBE_DIR:-/tmp/ssroi-probe}
NC=${LARMORX:-.}/crates-nc/target/release/larmorx-nc
tag=$1; IN=$2; OUT=$3; shift 3
rm -rf $P/$tag && mkdir -p $P/$tag/fsl $P/$tag/rep
export FSLDIR=${FSLDIR_ORACLE:-$HOME/fsl-6.0.7.23} FSLOUTPUTTYPE=${OT:-NIFTI_GZ} PATH=/usr/bin:/bin OPENBLAS_CORETYPE=Haswell
(cd $P/$tag/fsl && $FSLDIR/bin/standard_space_roi $IN "$OUT" "$@" > o.txt 2> e.txt; echo $? > s.txt)
(cd $P/$tag/rep && LARMORX_UPSTREAM_DEFECTS=${MODE:-reproduce} $NC fsl standard_space_roi $IN "$OUT" "$@" > o.txt 2> e.txt; echo $? > s.txt)
for d in fsl rep; do echo "== $d status $(cat $P/$tag/$d/s.txt): $(ls $P/$tag/$d | tr '\n' ' ')"; echo "-- out:"; head -c 400 $P/$tag/$d/o.txt; echo "-- err:"; head -c 1500 $P/$tag/$d/e.txt; done
