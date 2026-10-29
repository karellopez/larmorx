#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Replays the BET findings of the FSL version trial (oracles/difftest/2026-10-29-version-fsl-2)
# against FSL 6.0.7.23 and 6.0.7.17, each in its own memory box (boxed_lxv.sh), one log per
# replay in <log dir>, and prints each replay's last lines.
# Usage: replay_bet_findings.sh <log dir> <finding.json>...
set -u
here=$(cd "$(dirname "$0")" && pwd)
logs=$1
shift
mkdir -p "$logs"
for f in "$@"; do
  n=$(basename "$f" .json)
  for v in 6.0.7.23 6.0.7.17; do
    log="$logs/$n-$v.log"
    "$here/boxed_lxv.sh" 8 "$log" difftest --replay "$f" --fsl-version "$v"
    echo "== $n against FSL $v"
    grep -v "^\s*$" "$log" | tail -4
  done
done
