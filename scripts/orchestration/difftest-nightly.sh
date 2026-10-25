#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# A night of differential random testing (docs/validation/difftest.md): every finished tool, random
# valid inputs and options through each original program and larmorx, differences shrunk.
# Writes <workspace>/oracles/difftest/<date>/ (results.jsonl, findings/, report.md, summary.json)
# and logs to <workspace>/oracles/logs/difftest-<date>.log; the log's last line is
# "DIFFTEST NIGHTLY FINISHED <status>". Run from cron or by hand; never in CI.
#
# Usage: difftest-nightly.sh <checkout> [hours (6)] [jobs (4)] [seed (YYYYMMDD)] [tools (all)]
# The checkout's python/larmorx needs its built _core (maturin build; never `maturin develop`
# in an agent's worktree) and the replica programs (build-gpl.sh, build-nc.sh).
# Example crontab line (22:30 every night, 6 hours, 4 jobs):
#   30 22 * * * /path/to/larmorx/scripts/orchestration/difftest-nightly.sh /path/to/larmorx 6 4
set -o pipefail
source "$(dirname "$0")/env.sh"
W=$1
HOURS=${2:-6}
JOBS=${3:-4}
SEED=${4:-$(date +%Y%m%d)}
TOOLS=${5:-all}
DAY=$(date +%Y-%m-%d)
OUT=$LARMORX_WORKSPACE/oracles/difftest/$DAY
LOG=$LARMORX_WORKSPACE/oracles/logs/difftest-$DAY.log
mkdir -p "$OUT" "$(dirname "$LOG")"
export PYTHONPATH=$W/python:$W/validation/src
cd "$W" || exit 1
{
  echo "difftest nightly: $(date -Is) checkout $W ($(git -C "$W" describe --always --dirty --abbrev=12)) hours $HOURS jobs $JOBS seed $SEED tools $TOOLS"
  python -m larmorx_validation difftest --tools "$TOOLS" --budget "$HOURS" --jobs "$JOBS" --seed "$SEED" --out "$OUT"
  status=$?
  echo "report: $OUT/report.md"
  echo "DIFFTEST NIGHTLY FINISHED $status"
} >> "$LOG" 2>&1
