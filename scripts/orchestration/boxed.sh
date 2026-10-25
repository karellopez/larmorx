#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Run a heavy job in its own memory box, so a runaway process cannot take the machine down.
# Usage: boxed.sh <GiB> <command> [args...]
#
# Each heavy job (parity tiers, differential random testing, fMRIPrep oracle runs, benchmarks,
# record_golden.py, verify-merge.sh) runs in a systemd user scope with a hard memory limit and no
# swap: a process that exceeds the limit is killed inside the box, and nothing outside it is
# touched. On 2026-10-25 one random N4BiasFieldCorrection case grew to 28 GB; the machine swapped
# until VS Code's terminal host stopped answering and the editor, with every agent running in
# it, went down. The command's exit status is returned; 137 means the box killed it.
#
# Example: scripts/orchestration/boxed.sh 10 python -m larmorx_validation difftest --budget 6h
set -u
[ $# -ge 2 ] || { sed -n '3,4p' "$0"; exit 2; }
GIB=$1; shift
case $GIB in ''|*[!0-9]*) echo "boxed.sh: <GiB> must be a whole number, got '$GIB'" >&2; exit 2;; esac
exec systemd-run --user --scope --quiet --collect \
  -p MemoryMax="${GIB}G" -p MemorySwapMax=0 -- "$@"
