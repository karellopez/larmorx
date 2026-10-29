#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# One `python -m larmorx_validation <args>` run of this checkout in its own memory box, its output
# in a log whose last line is "LXV-EXIT <status>" (so that one wait on the log's last line
# follows it). Usage: boxed_lxv.sh <GiB> <log> <args...>
set -u
gib=$1
log=$2
shift 2
here=$(cd "$(dirname "$0")/../../.." && pwd)
"$here/scripts/orchestration/boxed.sh" "$gib" "$here/scripts/orchestration/lxv.sh" "$here" "$@" >"$log" 2>&1
echo "LXV-EXIT $?" >>"$log"
