#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Run fMRIPrep as the pipeline-level oracle for larmorprepx (decision log, 2026-10-11).
#
# fMRIPrep runs from its official image through Apptainer, as the current user: no Docker,
# no sudo. Everything that decides the result is fixed and recorded next to the outputs in
# RUN_INFO.txt: the image digest and file checksum, the fMRIPrep version, the command, the
# random seeds, the thread counts, the host and the times. The working directory is kept, so
# intermediate results can be compared step by step.
#
# Usage:
#   scripts/run_fmriprep_oracle.sh <bids_dir> <participant> <run_name> [extra fMRIPrep args...]
# Example (the first oracle, volume only):
#   scripts/run_fmriprep_oracle.sh ~/.cache/larmorx-testdata/data/openneuro/ds000005 01 \
#       ds000005-sub-01-noreconall --fs-no-reconall
# Output: <workspace>/oracles/fmriprep-25.2.6/<run_name>/{out,work,RUN_INFO.txt,fmriprep.log}
# Never commit these outputs (CLAUDE.md, "Working on this machine").
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../.." && pwd)"
VERSION=25.2.6
DIGEST=sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e
IMAGE="$WS/oracles/images/fmriprep-$VERSION.sif"
LICENSE=/usr/local/freesurfer/license.txt
NPROCS="${NPROCS:-8}"
OMP="${OMP:-8}"
SEED="${SEED:-0}"

[[ $# -ge 3 ]] || { sed -n '3,16p' "$0"; exit 2; }
BIDS="$(cd "$1" && pwd)"; PARTICIPANT="$2"; NAME="$3"; shift 3
[[ -f "$IMAGE" ]] || { echo "image not found: $IMAGE (see oracles/images/fmriprep-$VERSION.README.txt)" >&2; exit 1; }
[[ -r "$LICENSE" ]] || { echo "FreeSurfer licence not readable: $LICENSE" >&2; exit 1; }

RUN="$WS/oracles/fmriprep-$VERSION/$NAME"
[[ -e "$RUN/out" ]] && { echo "$RUN exists; choose another run name" >&2; exit 1; }
mkdir -p "$RUN/out" "$RUN/work" "$RUN/home" "$WS/oracles/templateflow"

ARGS=(/data /out participant --participant-label "$PARTICIPANT" -w /work
  --fs-license-file /opt/license.txt --skip-bids-validation --notrack --stop-on-first-crash
  --nprocs "$NPROCS" --omp-nthreads "$OMP" --random-seed "$SEED" --skull-strip-fixed-seed
  --output-spaces MNI152NLin2009cAsym:res-2 T1w func "$@")

{
  echo "fMRIPrep oracle run: $NAME"
  echo "image: docker://nipreps/fmriprep@$DIGEST (tag $VERSION)"
  echo "image file: $IMAGE, sha256 $(sha256sum "$IMAGE" | cut -d' ' -f1)"
  echo "fmriprep: $(apptainer exec --cleanenv "$IMAGE" fmriprep --version 2>/dev/null | tail -1)"
  echo "apptainer: $(apptainer --version)"
  echo "BIDS input: $BIDS"
  echo "participant: $PARTICIPANT"
  echo "arguments: ${ARGS[*]}"
  echo "seeds: --random-seed $SEED --skull-strip-fixed-seed; threads: --nprocs $NPROCS --omp-nthreads $OMP"
  echo "host: $(uname -srm), $(nproc) CPUs, $(free -g | awk '/Mem:/ {print $2}') GB RAM"
  echo "started: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$RUN/RUN_INFO.txt"

set +e
APPTAINERENV_TEMPLATEFLOW_HOME=/templateflow apptainer run --cleanenv --home "$RUN/home:/home/fmriprep" \
  -B "$BIDS:/data:ro" -B "$RUN/out:/out" -B "$RUN/work:/work" \
  -B "$LICENSE:/opt/license.txt:ro" -B "$WS/oracles/templateflow:/templateflow" \
  "$IMAGE" "${ARGS[@]}" > "$RUN/fmriprep.log" 2>&1
STATUS=$?
set -e
{
  echo "finished: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "exit status: $STATUS"
} >> "$RUN/RUN_INFO.txt"
cat "$RUN/RUN_INFO.txt"
exit "$STATUS"
