#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Whether FSL 6.0.7.23's mcflirt depends on the CPU through OpenBLAS: the same run under each
# OpenBLAS core this CPU can execute (OPENBLAS_CORETYPE; FSL's conda OpenBLAS 0.3.33 picks one at
# start-up, Cooperlake on an AMD Zen 4) and FSL 6.0.7.17 (Armadillo 10.5.1's closed-form 4 x 4
# inverse, no LAPACK), compared output by output.
# Usage: openblas_core_probe.sh <scratch dir> <mcflirt arguments without -out...>
set -u
out=$1
shift
mkdir -p "$out"
for core in default Haswell SkylakeX Cooperlake Zen SandyBridge Prescott; do
  d=$out/6.0.7.23-$core
  mkdir -p "$d"
  extra=()
  if [ "$core" != default ]; then extra=(OPENBLAS_CORETYPE=$core); fi
  env -i HOME="$HOME" PATH=/usr/bin:/bin FSLDIR="$HOME/fsl-6.0.7.23" FSLOUTPUTTYPE=NIFTI_GZ \
    OPENBLAS_VERBOSE=2 "${extra[@]}" "$HOME/fsl-6.0.7.23/bin/mcflirt" "$@" -out "$d/out" \
    >"$d/log" 2>&1
  echo "$core: $(head -1 "$d/log")"
done
d=$out/6.0.7.17
mkdir -p "$d"
env -i HOME="$HOME" PATH=/usr/bin:/bin FSLDIR="$HOME/fsl" FSLOUTPUTTYPE=NIFTI_GZ \
  "$HOME/fsl/bin/mcflirt" "$@" -out "$d/out" >"$d/log" 2>&1
for f in "$out"/*/out*.rms "$out"/*/out.par; do
  [ -f "$f" ] && echo "$(md5sum <"$f" | cut -c1-8) $f"
done | sed "s|$out/||" | sort -k2
