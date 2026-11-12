#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# One head's recon-all (FreeSurfer 8.2.0's default stream, -all), every step recorded by
# fs_time_recorder.py (installed with install_recorder.sh), with its RUN_INFO.txt.
#
#   run_recon.sh <freesurfer home> <licence file> <T1w> <subject id> <out dir> <threads> [recon-all args]
#
# <out dir>/subjects/<id>   recon-all's subject directory
# <out dir>/record/         steps/NNNN-<program>.json and blobs/ (fs_time_recorder.py)
# <out dir>/recon-all.out   recon-all's terminal output
# <out dir>/RUN_INFO.txt    what ran, where, with what; its last line "EXIT <status>"
# No -parallel: the steps run one after the other, so each file change belongs to one step.
set -u
FS=$1 LIC=$2 T1=$3 SID=$4 OUT=$5 TH=$6
shift 6
mkdir -p "$OUT/subjects"
export FREESURFER_HOME=$FS FS_LICENSE=$LIC SUBJECTS_DIR=$OUT/subjects
set +u
source "$FS/SetUpFreeSurfer.sh" > /dev/null 2>&1
set -u
export LARMORX_FS_RECORD=$OUT/record LARMORX_FS_SUBJDIR=$SUBJECTS_DIR/$SID
CMD=(recon-all -s "$SID" -i "$T1" -all -openmp "$TH" -itkthreads "$TH" "$@")
{
  echo "what: FreeSurfer recon-all, every step recorded (tools/freesurfer-oracle)"
  echo "command: ${CMD[*]}"
  echo "subject: $SID"
  echo "T1w: $T1"
  echo "T1w sha256: $(sha256sum "$T1" | cut -d' ' -f1)"
  echo "freesurfer: $(cat "$FS/build-stamp.txt" 2>/dev/null)"
  echo "freesurfer package sha256: $(cat "$FS/../../../../../download/SHA256SUMS" 2>/dev/null | cut -d' ' -f1)"
  echo "recorder sha256: $(sha256sum "$FS/bin/fs_time" | cut -d' ' -f1)"
  echo "threads: $TH (-openmp, -itkthreads)"
  echo "cpu: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //'), $(nproc) logical"
  echo "os: $(. /etc/os-release; echo "$PRETTY_NAME")"
  echo "start: $(date -Is)"
} > "$OUT/RUN_INFO.txt"
"${CMD[@]}" > "$OUT/recon-all.out" 2>&1
status=$?
"$FS/bin/fs_time" --larmorx-final
{
  echo "end: $(date -Is)"
  echo "steps recorded: $(ls "$OUT/record/steps" 2>/dev/null | wc -l)"
  echo "EXIT $status"
} >> "$OUT/RUN_INFO.txt"
exit $status
