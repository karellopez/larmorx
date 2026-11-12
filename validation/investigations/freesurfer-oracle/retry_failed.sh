#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# After a queue: move each run whose RUN_INFO.txt does not end "EXIT 0" into <runs>/../failed/<run>-<time>/
# (kept, never deleted: recon-all refuses an existing subject folder), then run the queue again for them.
#   retry_failed.sh <freesurfer home> <licence> <jobs.tsv> <runs dir> <parallel> <threads>
set -u
FS=$1 LIC=$2 JOBS=$3 RUNS=$4 PAR=$5 TH=$6
F=$(dirname "$RUNS")/failed
for r in "$RUNS"/*/; do
  r=${r%/}
  if ! tail -n 1 "$r/RUN_INFO.txt" 2>/dev/null | grep -qx "EXIT 0"; then
    mkdir -p "$F" && mv "$r" "$F/$(basename "$r")-$(date +%Y%m%dT%H%M%S)" && echo "moved $(basename "$r")"
  fi
done
bash "$(dirname "$0")/run_queue.sh" "$FS" "$LIC" "$JOBS" "$RUNS" "$PAR" "$TH"
