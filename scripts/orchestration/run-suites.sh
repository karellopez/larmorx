#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Standard-tier parity records of several suites, side by side (verify-merge.sh's step 5).
# Usage: run-suites.sh <checkout> <outdir> [--serial] [--main-log <file>] "<parity args>"...
#   each "<parity args>" is one suite as verify-merge's --suite takes it, e.g. "ants-ai --jobs 6";
#   the suites run at the same time (each with its own --jobs, so their threads add up: choose
#   them within the machine's share), each logged in <outdir>/<args with blanks as _>.log and
#   recorded in <outdir>/records/<suite>.{json,md} (two suites must not write the same record);
#   --serial runs them one after the other, as before 2026-10-30.
# When all have ended, one line per suite in the order given, "STEP5 <args> exit <status>",
# on standard output and appended to --main-log; the exit status is the worst suite's.
R=$1; OUT=$2; shift 2
D=$(cd "$(dirname "$0")" && pwd)
SERIAL=0; MAIN=""; SUITES=()
while [ $# -gt 0 ]; do
  case $1 in
    --serial) SERIAL=1; shift;;
    --main-log) MAIN=$2; shift 2;;
    *) SUITES+=("$1"); shift;;
  esac
done
mkdir -p "$OUT/records"
declare -a PIDS CODES
run_one() {
  local a=$1 n
  n=$(echo "$a" | tr ' ' '_')
  # shellcheck disable=SC2086 # the suite's arguments are words
  timeout 21600 bash "$D/lxv.sh" "$R" parity $a --tier standard --out "$OUT/records" > "$OUT/$n.log" 2>&1
}
for i in "${!SUITES[@]}"; do
  if [ $SERIAL = 1 ]; then
    run_one "${SUITES[$i]}"; CODES[$i]=$?
  else
    run_one "${SUITES[$i]}" & PIDS[$i]=$!
  fi
done
if [ $SERIAL = 0 ]; then
  for i in "${!SUITES[@]}"; do wait "${PIDS[$i]}"; CODES[$i]=$?; done
fi
worst=0
for i in "${!SUITES[@]}"; do
  line="STEP5 ${SUITES[$i]} exit ${CODES[$i]}"
  echo "$line"; [ -n "$MAIN" ] && echo "$line" >> "$MAIN"
  [ "${CODES[$i]}" -gt "$worst" ] && worst=${CODES[$i]}
done
exit "$worst"
