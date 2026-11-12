#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Run a list of recorded recon-all jobs, <parallel> at a time, each with run_recon.sh.
#
#   run_queue.sh <freesurfer home> <licence> <jobs.tsv> <runs dir> <parallel> <threads per job>
#
# jobs.tsv: one job per line, tab-separated: <run name> <T1w> <subject id> [recon-all args ...]
# (e.g. "-no-v8" for the classic stream). Each run goes to <runs dir>/<run name>/; a run whose
# RUN_INFO.txt already ends with "EXIT 0" is skipped. Prints one line per finished run and
# "QUEUE-DONE <failed count>" at the end.
set -u
FS=$1 LIC=$2 JOBS=$3 RUNS=$4 PAR=$5 TH=$6
D=$(cd "$(dirname "$0")" && pwd)
export FS LIC RUNS TH D
one() {
  IFS=$'\t' read -r name t1 sid rest <<< "$1"
  out=$RUNS/$name
  if tail -n 1 "$out/RUN_INFO.txt" 2>/dev/null | grep -qx "EXIT 0"; then
    echo "skip $name (done)"; return 0
  fi
  mkdir -p "$out"
  # shellcheck disable=SC2086
  nice -n 5 bash "$D/run_recon.sh" "$FS" "$LIC" "$t1" "$sid" "$out" "$TH" $rest
  echo "done $name: $(tail -n 1 "$out/RUN_INFO.txt")"
}
export -f one
grep -v '^\s*$' "$JOBS" | grep -v '^#' | tr '\n' '\0' | xargs -0 -P "$PAR" -I{} bash -c 'one "$@"' _ {}
failed=$(for r in "$RUNS"/*/RUN_INFO.txt; do tail -n 1 "$r" | grep -vx "EXIT 0"; done | wc -l)
echo "QUEUE-DONE $failed"
