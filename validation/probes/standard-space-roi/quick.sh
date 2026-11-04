#!/usr/bin/env bash
# quick probe: FSL's standard_space_roi vs the replica on one T1w, args after the two names
set -u
P=${PROBE_DIR:-/tmp/ssroi-probe}
NC=${LARMORX:-.}/crates-nc/target/release/larmorx-nc
T1=${T1:-$HOME/.cache/larmorx-testdata/data/openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz}
tag=$1; shift
rm -rf $P/$tag && mkdir -p $P/$tag/fsl $P/$tag/rep
export FSLDIR=${FSLDIR_ORACLE:-$HOME/fsl-6.0.7.23} FSLOUTPUTTYPE=NIFTI_GZ PATH=/usr/bin:/bin OPENBLAS_CORETYPE=Haswell
cp $T1 $P/$tag/fsl/in.nii.gz; cp $T1 $P/$tag/rep/in.nii.gz
(cd $P/$tag/fsl && $FSLDIR/bin/standard_space_roi in out "$@" > o.txt 2> e.txt; echo $? > s.txt)
(cd $P/$tag/rep && LARMORX_UPSTREAM_DEFECTS=reproduce $NC fsl standard_space_roi in out "$@" > o.txt 2> e.txt; echo $? > s.txt)
for d in fsl rep; do echo "== $d status $(cat $P/$tag/$d/s.txt)"; ls $P/$tag/$d | tr '\n' ' '; echo; head -c 600 $P/$tag/$d/o.txt; head -c 600 $P/$tag/$d/e.txt; done
python - "$P/$tag" <<'PY'
import sys, gzip, pathlib
d=pathlib.Path(sys.argv[1])
for f in sorted((d/'fsl').iterdir()):
    g=d/'rep'/f.name
    if not g.exists(): print('missing in rep', f.name); continue
    a=f.read_bytes(); b=g.read_bytes()
    if f.name.endswith('.gz'): a=gzip.decompress(a); b=gzip.decompress(b)
    if f.suffix=='.gz' and len(a)>348: a=a[:148]+a[228:]; b=b[:148]+b[228:]  # drop descrip
    print(f.name, 'same' if a==b else f'DIFF ({len(a)} vs {len(b)})')
PY
