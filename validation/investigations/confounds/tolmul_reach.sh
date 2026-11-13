#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# The records that DBDSQR's tolerance constant (TOLMUL, corrected in the confounds' part: random-testing
# finding 54131) can reach, re-made from a checkout into <out>/<name>/, one after the other, each in its
# own memory box; then compare them with the committed ones (compare_remade_records.py).
# Usage: tolmul_reach.sh <checkout> <out> [<GiB> [<name>...]]   (default: 20 GiB, every record below)
# The reach (grep of numpy_linalg's dbdsqr_upper_ncc): DBDSDC/DLASDQ (numpy's svd, pinv), DGESVD (SciPy's
# gesvd, nipype's fallback_svd and _full_rank), gesdd_s2 (larmorx-optim's curve_fit and least_squares:
# tedana), lstsq (tedana's replica and clean room), svd3 (larmorx-io's nibabel_qform: save_nibabel,
# set_xform, io_orientation: sdcflows, niworkflows, the confounds). Needs the checkout's extension built,
# LARMORX_GPL_BIN at the checkout's larmorx-gpl (tedana's replica), FSLDIR set, a clean tree.
set -f
R=$(cd "$1" && pwd); OUT=$2; GIB=${3:-20}; shift 3 2>/dev/null || shift $#
D=$R/scripts/orchestration
declare -A CMD=(
  [confounds-linalg]="confounds-linalg --tier standard --jobs 2"
  [confounds-nipype]="confounds-nipype --tier standard --jobs 2"
  [confounds-fmriprep]="confounds-fmriprep --tier standard --jobs 2"
  [sdcflows-fieldmap]="sdcflows-fieldmap --tier standard --python newest --jobs 2"
  [sdcflows-estimation]="sdcflows-estimation --tier standard --python newest --jobs 2"
  [sdcflows-estimation-full]="sdcflows-estimation --tier full --case ^(pepolar-workflow|pepolar-image|syn-preprocessing-workflow|syn-sdc-workflow|estimators-image|derivatives-preproc|app-run-.*)$ --python newest --jobs 2"
  [sdcflows-estimation-app-shared]="sdcflows-estimation --tier full --case ^app-run- --python shared --jobs 2"
  [niworkflows-helpers]="niworkflows-helpers --tier standard --python newest --jobs 2"
  [tedana-t2smap-replica]="tedana-t2smap-replica --tier standard --jobs 2"
  [tedana-t2smap-original]="tedana-t2smap-original --tier full --jobs 2"
)
ORDER=(confounds-linalg confounds-nipype confounds-fmriprep sdcflows-fieldmap sdcflows-estimation sdcflows-estimation-full sdcflows-estimation-app-shared niworkflows-helpers tedana-t2smap-replica tedana-t2smap-original)
[ $# -gt 0 ] && NAMES=("$@") || NAMES=("${ORDER[@]}")
if [ -n "$(git -C "$R" status --porcelain --untracked-files=no)" ]; then echo "the checkout is not clean: records only from a clean commit"; exit 1; fi
for n in "${NAMES[@]}"; do
  mkdir -p "$OUT/$n"
  # shellcheck disable=SC2086 # the suite's arguments are words
  "$D/boxed.sh" "$GIB" "$D/lxv.sh" "$R" parity ${CMD[$n]} --out "$OUT/$n" > "$OUT/$n.log" 2>&1
  echo "$n exit $?"
done
