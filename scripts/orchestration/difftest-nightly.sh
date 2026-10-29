#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# A night of differential random testing (docs/validation/difftest.md): every finished tool, random
# valid inputs and options through each original program and larmorx, differences shrunk.
# Writes <workspace>/oracles/difftest/<date>/ (results.jsonl, findings/, report.md, summary.json)
# and logs to <workspace>/oracles/logs/difftest-<date>.log; the log's last line is
# "DIFFTEST NIGHTLY FINISHED <status>". Run from cron or by hand; never in CI.
#
# Once a week (by the date of its last report) it first runs the upstream watch
# (upstream_watch.py: <workspace>/oracles/logs/upstream-watch/upstream-watch-<date>.md and .json).
#
# Usage: difftest-nightly.sh <checkout> [hours (6)] [jobs (4)] [seed (YYYYMMDD)] [tools (all)]
# The checkout's python/larmorx needs its built _core (maturin build; never `maturin develop`
# in an agent's worktree) and the replica programs (build-gpl.sh, build-nc.sh).
# Example crontab line (22:30 every night, 6 hours, 4 jobs):
#   30 22 * * * /path/to/larmorx/scripts/orchestration/difftest-nightly.sh /path/to/larmorx 6 4
#
# The night runs in a memory box (CLAUDE.md, 2026-10-25: one random N4 case once grew to 28 GB and
# took the machine down): run it as `scripts/orchestration/boxed.sh 8 difftest-nightly.sh ...`, or
# plainly, and it boxes itself (a systemd user scope of LARMORX_DIFFTEST_BOX_GIB GiB, 8, no swap).
# Inside, each worker has a hard limit of LARMORX_DIFFTEST_CASE_GIB GiB (3) and no swap; a case
# that exceeds it is recorded (verdict "memory") and queued in heavy.jsonl, for
# `python -m larmorx_validation difftest --heavy <night>` when the machine is quiet.
set -o pipefail
cgroup=$(sed -n 's/^0:://p' /proc/self/cgroup)
if [ "$(cat "/sys/fs/cgroup$cgroup/memory.max" 2>/dev/null)" = max ] && [ -z "${LARMORX_DIFFTEST_BOXED:-}" ]; then
  export LARMORX_DIFFTEST_BOXED=1
  exec systemd-run --user --scope --quiet --collect -p "MemoryMax=${LARMORX_DIFFTEST_BOX_GIB:-8}G" \
    -p MemorySwapMax=0 -- "$0" "$@"
fi
source "$(dirname "$0")/env.sh"
W=$1
HOURS=${2:-6}
JOBS=${3:-4}
SEED=${4:-$(date +%Y%m%d)}
TOOLS=${5:-all}
DAY=$(date +%Y-%m-%d)
OUT=${LARMORX_DIFFTEST_OUT:-$LARMORX_WORKSPACE/oracles/difftest/$DAY}
LOG=${LARMORX_DIFFTEST_LOG:-$LARMORX_WORKSPACE/oracles/logs/difftest-$DAY.log}
mkdir -p "$OUT" "$(dirname "$LOG")"
export PYTHONPATH=$W/python:$W/validation/src
cd "$W" || exit 1
{
  echo "difftest nightly: $(date -Is) checkout $W ($(git -C "$W" describe --always --dirty --abbrev=12)) hours $HOURS jobs $JOBS seed $SEED tools $TOOLS"
  echo "memory box: $(sed -n 's/^0:://p' /proc/self/cgroup) memory.max $(cat "/sys/fs/cgroup$(sed -n 's/^0:://p' /proc/self/cgroup)/memory.max")"
  # The upstream watch, once a week (decision log 2026-10-27): newer upstream releases and the
  # files of the crates' PROVENANCE.md they changed; no agent, a few network requests. It runs
  # when its newest report is 7 days old or more, and never stops the night.
  WATCH=${LARMORX_UPSTREAM_WATCH_OUT:-$LARMORX_WORKSPACE/oracles/logs/upstream-watch}
  if [ -z "$(find "$WATCH" -name 'upstream-watch-*.md' -mtime -7 2>/dev/null | head -1)" ]; then
    echo "upstream watch: $(date -Is)"
    timeout 1800 python -P "$W/scripts/orchestration/upstream_watch.py" --out "$WATCH" >/dev/null \
      && echo "upstream watch: $WATCH/upstream-watch-$DAY.md" || echo "upstream watch failed ($?)"
  fi
  python -m larmorx_validation difftest --tools "$TOOLS" --budget "$HOURS" --jobs "$JOBS" --seed "$SEED" --out "$OUT" \
    --memory-gib "${LARMORX_DIFFTEST_CASE_GIB:-3}"
  status=$?
  echo "report: $OUT/report.md"
  echo "DIFFTEST NIGHTLY FINISHED $status"
} >> "$LOG" 2>&1
