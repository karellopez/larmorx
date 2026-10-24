#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The data panel's nine new fMRIPrep 25.2.6 oracle runs, one after the other, at 6 threads
# (NPROCS=6 OMP=6: the machine is shared; the five earlier runs used 8). Each run reads its BIDS
# view (bids_view.py) and goes through scripts/run_fmriprep_oracle.sh of the checkout given, with
# the options of the earlier runs (--fs-no-reconall; ds004173 --anat-only). A run that fails does
# not stop the others; every run's status is in the log.
#
# Usage: run_panel_oracles.sh <larmorx checkout> [run name ...]   (default: all nine)
# Log: <workspace>/oracles/logs/panel-oracles.log, last line "PANEL ORACLES FINISHED".
set -uo pipefail
REPO=$(cd "$1" && pwd); shift
WS=$(cd "$REPO/.." && pwd)
VIEWS=$WS/oracles/bids-views
LOG=$WS/oracles/logs/panel-oracles.log
RUNS=(
  "ds000228 pixar008 ds000228-sub-pixar008-noreconall"
  "ds005003 RS001 ds005003-sub-RS001-noreconall"
  "ds004173 603309 ds004173-sub-603309-noreconall --anat-only"
  "ds002422 02 ds002422-sub-02-noreconall"
  "ds000256 CTS203 ds000256-sub-CTS203-noreconall"
  "ds004884 M2079 ds004884-sub-M2079-noreconall"
  "ds003592 280 ds003592-sub-280-noreconall"
  "ds002603 01 ds002603-sub-01-noreconall"
  "ds004958 fsm64ps ds004958-sub-fsm64ps-noreconall"
)
mkdir -p "$(dirname "$LOG")"
echo "panel oracles started $(date -u +%Y-%m-%dT%H:%M:%SZ) from $REPO" >> "$LOG"
for spec in "${RUNS[@]}"; do
  read -r ds label name extra <<< "$spec"
  if [[ $# -gt 0 ]] && [[ ! " $* " == *" $name "* ]]; then continue; fi
  start=$(date +%s)
  echo "RUN $name: started $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOG"
  NPROCS=6 OMP=6 "$REPO/scripts/run_fmriprep_oracle.sh" "$VIEWS/$ds" "$label" "$name" \
    --fs-no-reconall ${extra:-} > "$WS/oracles/logs/panel-oracle-$name.log" 2>&1
  status=$?
  echo "RUN $name: exit $status after $(( ($(date +%s) - start) / 60 )) min" >> "$LOG"
done
echo "PANEL ORACLES FINISHED $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOG"
