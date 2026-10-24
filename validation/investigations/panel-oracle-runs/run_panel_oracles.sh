#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# The data panel's nine new fMRIPrep 25.2.6 oracle runs, one after the other, at 6 threads
# (NPROCS=6 OMP=6: the machine is shared; the five earlier runs used 8). Each run reads its BIDS
# view (bids_view.py) and goes through scripts/run_fmriprep_oracle.sh of the checkout given, with
# the options of the earlier runs (--fs-no-reconall; ds004173 --anat-only). A run that fails does
# not stop the others; every run's status is in the log.
#
# Usage: run_panel_oracles.sh <larmorx checkout> [run name ...]   (default: all nine)
# Log: <workspace>/oracles/logs/panel-oracles.log, last line "PANEL ORACLES FINISHED". After each
# run, its RUN_INFO.txt gets the BIDS view it read and the commit of this script; ds004884 also
# the derived lesion mask (bids_view.py, mask_on_grid).
set -uo pipefail
REPO=$(cd "$1" && pwd); shift
WS=$(cd "$REPO/.." && pwd)
VIEWS=$WS/oracles/bids-views
LOG=$WS/oracles/logs/panel-oracles.log
# The commit of this script and of bids_view.py beside it, recorded in every run's RUN_INFO.txt.
COMMIT=$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse HEAD 2>/dev/null || echo unknown)
git -C "$(dirname "${BASH_SOURCE[0]}")" diff --quiet HEAD -- . 2>/dev/null || COMMIT="$COMMIT-dirty"
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
  info=$WS/oracles/fmriprep-25.2.6/$name/RUN_INFO.txt
  if [[ -f $info ]] && ! grep -q '^BIDS view:' "$info"; then
    {
      echo "BIDS view: $VIEWS/$ds, the panel's files of $ds hard-linked from the test-data cache"
      echo "  (larmorx validation/investigations/panel-oracle-runs/bids_view.py, commit $COMMIT)"
      if [[ $ds == ds004884 ]]; then
        echo "derived input: sub-M2079/ses-217/anat/sub-M2079_ses-217_label-lesion_roi.nii.gz, sha256"
        echo "  $(sha256sum "$VIEWS/$ds/sub-M2079/ses-217/anat/sub-M2079_ses-217_label-lesion_roi.nii.gz" | cut -d' ' -f1),"
        echo "  the dataset's manual lesion mask (derivatives/lesion_masks/sub-M2079/ses-217/anat/"
        echo "  sub-M2079_ses-217_acq-spc3_run-4_T2w_desc-lesion_mask.nii.gz, drawn on the T2w's grid)"
        echo "  resampled onto the T1w's grid (sub-M2079_ses-217_acq-tfl3p2_run-3_T1w) by nearest"
        echo "  neighbour through both files' scanner coordinates, no registration, by bids_view.py's"
        echo "  mask_on_grid; the test-data catalog keeps the original mask unchanged"
      fi
    } >> "$info"
  fi
done
echo "PANEL ORACLES FINISHED $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LOG"
