#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# time.sh <larmorx-freesurfer> <rounds> <ribbon|aseg> <work folder>: FreeSurfer 8.2.0's mri_label2vol and the
# port (reproduce, fix) alternately on one recorded head (ds000005's v8 run), one thread, the wall time of
# each in seconds. ribbon: --fill-ribbon with lh.cortex.label and --label-stat; aseg: --new-aseg2vol --ttype
# --resmm 0.5 onto rawavg.mgz. Run it inside a memory box (scripts/orchestration/boxed.sh). Never a
# diagnostic level above 0 on the official program. LARMORX_WORKSPACE names the workspace (default: the
# folder above the repository; set it in a worktree).
set -u
B=$1; N=${2:-3}; CALL=${3:-ribbon}; T=$4
WS=${LARMORX_WORKSPACE:-$(cd "$(dirname "$0")/../../../../.." && pwd)}
FSH=$WS/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0
S=$WS/oracles/freesurfer-8.2.0/runs/ds000005-sub-01-v8/subjects/sub-01
mkdir -p "$T"; cd "$T"
export FREESURFER_HOME=$FSH FS_LICENSE=${FS_LICENSE:-/usr/local/freesurfer/license.txt} LC_ALL=C LANG=C \
  OMP_NUM_THREADS=1 SUBJECTS_DIR=$(dirname "$S") PATH=$FSH/bin:/usr/bin:/bin FS_DISABLE_LANG=1 USER=probe \
  FREESURFER_SEED=1234
if [ "$CALL" = ribbon ]; then
  A="--label $S/label/lh.cortex.label --fill-ribbon --subject sub-01 --hemi lh --identity --label-stat stat.mgz --o ribbon.mgz"
else
  A="--seg $S/mri/aseg.mgz --temp $S/mri/rawavg.mgz --regheader --new-aseg2vol --ttype --resmm 0.5 --o tt.mgz"
fi
t() { local s e rc; s=$(date +%s.%N); "$@" > out.txt 2>&1; rc=$?; e=$(date +%s.%N); printf "rc=%s %.2f" "$rc" "$(echo "$e - $s" | bc)"; }
for _ in $(seq "$N"); do
  # shellcheck disable=SC2086
  echo "$CALL official $(t "$FSH/bin/mri_label2vol" $A) | reproduce $(t "$B" --upstream-defects reproduce freesurfer mri_label2vol $A) | fix $(t "$B" --upstream-defects fix freesurfer mri_label2vol $A)"
done
