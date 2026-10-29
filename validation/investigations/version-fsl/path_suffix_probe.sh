#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# NewNifti 5.0.0's replaceSuffix (FSL 6.0.7.23) on paths that hold ".img" or ".hdr" before the
# file's own extension: mcflirt of FSL 6.0.7.17 and 6.0.7.23 on the same series, read from and
# written to such directories (bet2 strips ".hdr"/".img" from its names itself, FSL-40, so it
# cannot show this). Usage: path_suffix_probe.sh <a small 4D series .nii.gz> <scratch dir>
set -u
img=$1
out=$2
rm -rf "$out"
for v in 6.0.7.17 6.0.7.23; do
  if [ "$v" = 6.0.7.17 ]; then fsldir=$HOME/fsl; else fsldir=$HOME/fsl-$v; fi
  d=$out/$v
  mkdir -p "$d/a.img.d" "$d/a.hdr.d" "$d/plain" "$d/x.hdr.y.img.d"
  cp "$img" "$d/a.img.d/in.nii.gz"
  cp "$img" "$d/plain/in.nii.gz"
  cp "$img" "$d/a.hdr.d/in.nii.gz"
  run() {
    echo "== FSL $v: mcflirt $* (FSLOUTPUTTYPE=${OTYPE:-NIFTI_GZ})"
    (cd "$d" && env -i HOME="$HOME" PATH=/usr/bin:/bin FSLDIR="$fsldir" \
      FSLOUTPUTTYPE="${OTYPE:-NIFTI_GZ}" "$fsldir/bin/mcflirt" "$@" 2>&1 | head -4)
    echo "exit ${PIPESTATUS[0]}"
  }
  run -in a.img.d/in.nii.gz -out plain/o1
  run -in plain/in.nii.gz -out a.img.d/o2
  run -in a.hdr.d/in.nii.gz -out plain/o3
  OTYPE=NIFTI_PAIR run -in plain/in.nii.gz -out a.hdr.d/o4
  OTYPE=NIFTI_PAIR run -in plain/in.nii.gz -out x.hdr.y.img.d/o5
  (cd "$d" && find . -type f | sort | tr '\n' ' ')
  echo
done
