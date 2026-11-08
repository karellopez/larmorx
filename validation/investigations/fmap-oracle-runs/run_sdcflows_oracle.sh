#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# One oracle run of sdcflows 2.17.1's own BIDS app (the field maps' part B, decision D8 (d)): the field-map
# estimators of one participant, without field-map-less ones (`--no-fmapless`), in
# <workspace>/oracles/python-sdcflows-2.17 (make_python_sdcflows.sh), with the workspace's oracle builds
# on the PATH: ANTs 2.6.5 (GCC 14.3), AFNI 26.3.05 and FSL 6.0.7.23 (`$FSLDIR_ORACLE`, by default
# `~/fsl-6.0.7.23`). Writes <workspace>/oracles/sdcflows-2.17.1/<run>/ (out/, work/, sdcflows.log and
# RUN_INFO.txt: the command, the environment, the programs' SHA-256, start, end, exit status, and the
# BIDS view's `<view>.DERIVED.txt` when there is one). Never overwrites a run.
# Usage: run_sdcflows_oracle.sh <workspace> <run> <BIDS dir> <participant> [more sdcflows options]
# (NPROCS, OMP: the threads, 4 by default)
set -uo pipefail
ws="${1:?usage: run_sdcflows_oracle.sh <workspace> <run> <BIDS dir> <participant> [options]}"
name="${2:?run name}"
bids="${3:?BIDS dir}"
part="${4:?participant}"
shift 4
venv="$ws/oracles/python-sdcflows-2.17"
fsl="${FSLDIR_ORACLE:-$HOME/fsl-6.0.7.23}"
root="$ws/oracles/sdcflows-2.17.1/$name"
if [ -e "$root" ]; then
  echo "$root exists: never overwritten" >&2
  exit 1
fi
mkdir -p "$root"
nprocs="${NPROCS:-4}"
omp="${OMP:-$nprocs}"
args=("$bids" "$root/out" participant --participant-label "$part" -w "$root/work"
  --nprocs "$nprocs" --omp-nthreads "$omp" --no-fmapless --notrack -v "$@")
export PATH="$ws/oracles/ants-2.6.5/bin:$ws/oracles/afni-26.3.05/bin:$venv/bin:/usr/bin:/bin:$fsl/bin"
export FSLDIR="$fsl" FSLOUTPUTTYPE=NIFTI_GZ OPENBLAS_CORETYPE=Haswell OMP_NUM_THREADS="$omp"
export TEMPLATEFLOW_HOME="$ws/oracles/templateflow"
unset PYTHONPATH
info="$root/RUN_INFO.txt"
{
  echo "sdcflows oracle run: $name"
  echo "sdcflows: $("$venv/bin/sdcflows" --version 2>&1 | tail -1)"
  echo "environment: $venv ($(grep -E '^(niworkflows|nipype|numpy|scipy|nibabel)==' "$venv/ENVIRONMENT.txt" | tr '\n' ' '))"
  echo "BIDS input: $bids"
  echo "participant: $part"
  echo "arguments: ${args[*]}"
  echo "threads: --nprocs $nprocs --omp-nthreads $omp"
  echo "programs:"
  for p in topup applytopup 3dvolreg 3dAutomask N4BiasFieldCorrection antsRegistration antsApplyTransforms \
    DenoiseImage ImageMath; do
    exe="$(command -v "$p" || true)"
    if [ -n "$exe" ]; then echo "  $p $(sha256sum "$exe" | cut -c1-64) $exe"; else echo "  $p (not found)"; fi
  done
  echo "host: $(uname -srm), $(nproc) CPUs, $(free -g | awk '/Mem:/{print $2}') GB RAM"
  echo "started: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$info"
view="${bids%/}.DERIVED.txt"
[ -f "$view" ] && { echo "BIDS view: $bids"; cat "$view"; } >> "$info"
"$venv/bin/sdcflows" "${args[@]}" > "$root/sdcflows.log" 2>&1
status=$?
{
  echo "finished: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "exit status: $status"
} >> "$info"
exit $status
