#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Run probe_numpy_exp.py in both kept environments with numpy's default dispatch and with the
# AVX-512 features disabled (as on an AVX2 machine), and in the fMRIPrep image.
S=${OUT:?set OUT to an output folder}
W=${LARMORX_WORKSPACE:?}
mkdir -p "$S/probe-exp"
for py in "$W/.venv/bin/python" "$W/oracles/python-newest/bin/python"; do
  "$py" -I "$S/scripts/probe_numpy_exp.py" "$S/probe-exp"
  NPY_DISABLE_CPU_FEATURES="AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR" "$py" -I "$S/scripts/probe_numpy_exp.py" "$S/probe-exp"
done
timeout 300 apptainer exec --cleanenv -B "$S":/s "$W/oracles/images/fmriprep-25.2.6.sif" python -I /s/scripts/probe_numpy_exp.py /s/probe-exp
timeout 300 apptainer exec --cleanenv --env "NPY_DISABLE_CPU_FEATURES=AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR" -B "$S":/s "$W/oracles/images/fmriprep-25.2.6.sif" python -I /s/scripts/probe_numpy_exp.py /s/probe-exp
