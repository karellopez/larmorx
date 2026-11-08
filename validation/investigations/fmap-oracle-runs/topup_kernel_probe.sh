#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Does fMRIPrep's own topup depend on OpenBLAS's kernels? Re-runs the topup node of a finished fMRIPrep
# 25.2.6 oracle run in its image, once with OPENBLAS_CORETYPE=Haswell and once with the CPU's own kernels,
# and compares both field maps with the run's. nipype removed the node's encoding file, so it is written
# again by the image's nipype from the node's inputs.
# Usage: topup_kernel_probe.sh <workspace> <run dir> <topup node dir, relative to the run's work/> <out dir>
# (run it inside scripts/orchestration/boxed.sh). Found on 2026-11-08 on ds004958-sub-fsm64ps-noreconall-pepolar's
# first run (now in <workspace>/archive/2026-11-08-fmriprep-runs-avx512-kernels/), node wf_auto_00001: the CPU's
# own kernels (AVX-512) give the run's field bit for bit, Haswell's differ by up to 0.197 Hz.
set -euo pipefail
ws="${1:?usage: topup_kernel_probe.sh <workspace> <run dir> <node dir> <out dir>}"
run="$(cd "${2:?run dir}" && pwd)"
node="${3:?node dir}"
out="${4:?out dir}"
img="$ws/oracles/images/fmriprep-25.2.6.sif"
mkdir -p "$out/enc" "$out/haswell" "$out/default"
cmd="$(cat "$run/work/$node/command.txt")"
apptainer exec --cleanenv -B "$run/work:/work:ro" -B "$out/enc:/out" --pwd /out "$img" python -c "
from nipype.utils.filemanip import loadpkl
from nipype.interfaces.fsl import TOPUP
d = loadpkl('/work/$node/_inputs.pklz')
names = TOPUP.input_spec().copyable_trait_names()
TOPUP(**{k: v for k, v in d.items() if v is not None and k in names}).cmdline
"
cmd="${cmd//--datain=\/work\/$node\//--datain=/enc/}"
for k in haswell default; do
  env=(APPTAINERENV_OMP_NUM_THREADS=4)
  [ "$k" = haswell ] && env+=(APPTAINERENV_OPENBLAS_CORETYPE=Haswell)
  # shellcheck disable=SC2086
  env "${env[@]}" apptainer exec --cleanenv -B "$run/work:/work:ro" -B "$out/enc:/enc:ro" \
    -B "$out/$k:/out" --pwd /out "$img" $cmd > "$out/$k.log" 2>&1
done
field="$(basename "$(ls "$run/work/$node/"*_field.nii.gz)")"
"$ws/.venv/bin/python" -I - "$run/work/$node/$field" "$out/haswell/$field" "$out/default/$field" <<'EOF'
import sys

import nibabel as nb
import numpy as np

ref, *runs = (np.asanyarray(nb.load(p).dataobj).astype(float) for p in sys.argv[1:])
for name, a in zip(("haswell", "default"), runs):
    d = np.abs(a - ref)
    print(f"{name}: max |difference| from the run's field {d.max():.6g} Hz, equal voxels {np.mean(d == 0):.2%}")
EOF
