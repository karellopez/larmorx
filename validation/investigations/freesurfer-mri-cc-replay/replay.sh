#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# replay.sh <larmorx-freesurfer> <mode> <scratch> [run-pattern]: recon-all's recorded seg2cc steps replayed by
# the replay tool with FreeSurfer's own seg2cc script and a PATH on which mri_cc is the port in <mode>
# (reproduce or fix). One JSON report and log per run in <scratch>/replay-<mode>/; the rebuilt subjects in
# <scratch>/work-<mode>/ (removed at the end). Run it in a memory box (each step needs about 0.5 GiB).
set -u
[ $# -ge 3 ] || { sed -n '3,6p' "$0"; exit 2; }
CLI=$(realpath "$1"); MODE=$2; OUT=$(realpath -m "$3"); PAT=${4:--(classic|v8)$}
REPO=$(cd "$(dirname "$0")/../../.." && pwd)
mkdir -p "$OUT/shim-$MODE" "$OUT/replay-$MODE"
printf '#!/bin/sh\nexec %s --upstream-defects %s freesurfer mri_cc "$@"\n' "$CLI" "$MODE" > "$OUT/shim-$MODE/mri_cc"
printf '#!/bin/sh\nPATH=%s:$PATH\nexport PATH\nexec "$@"\n' "$OUT/shim-$MODE" > "$OUT/wrap-$MODE.sh"
chmod +x "$OUT/shim-$MODE/mri_cc" "$OUT/wrap-$MODE.sh"
cd "$REPO"
export PYTHONPATH=$REPO/python:$REPO/validation/src
RUNS=$(python -c 'from larmorx_validation.oracle_cache.store import workspace; print(workspace())')/oracles/freesurfer-8.2.0/runs
for run in $(ls "$RUNS" | grep -E -- "$PAT"); do
  python -m larmorx_validation freesurfer-replay run "$run" seg2cc --threads 1 \
    --command "$OUT/wrap-$MODE.sh {program}" --work "$OUT/work-$MODE/$run" \
    --json "$OUT/replay-$MODE/$run.json" > "$OUT/replay-$MODE/$run.log" 2>&1
  grep -h ": REPRODUCED\|: DIFFERENT\|: FAILED" "$OUT/replay-$MODE/$run.log"
done
rm -rf "$OUT/work-$MODE"
