#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Compare the -help text of 3dTshift, 3dAutomask and 3dUnifize between two AFNI oracle builds
# (no GUI or browser options). Usage: help_diff.sh <workdir> [old] [new]
#   old, new: oracle directory names under <workspace>/oracles (default afni-25.2.09, afni-26.3.05)
set -euo pipefail
work=${1:?usage: help_diff.sh <workdir> [old] [new]}
old=${2:-afni-25.2.09}
new=${3:-afni-26.3.05}
ws=${LARMORX_WORKSPACE:-}
if [ -z "$ws" ]; then  # the first parent holding oracles/ (a worktree sits deeper)
  ws=$(cd "$(dirname "$0")" && pwd)
  while [ "$ws" != / ] && [ ! -d "$ws/oracles" ]; do ws=$(dirname "$ws"); done
fi
mkdir -p "$work/home"
for p in 3dTshift 3dAutomask 3dUnifize; do
  for v in "$old" "$new"; do
    status=0
    env -i PATH=/usr/bin:/bin HOME="$work/home" AFNI_DONT_LOGFILE=YES \
      "$ws/oracles/$v/bin/$p" -help >"$work/$p-$v.help" 2>&1 || status=$?
    echo "$p $v: exit $status, $(wc -l <"$work/$p-$v.help") lines"
  done
  diff "$work/$p-$old.help" "$work/$p-$new.help" >"$work/$p.helpdiff" || true
  echo "$p: $(grep -c '^[<>]' "$work/$p.helpdiff" || true) lines differ"
done
