#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# fMRIPrep's and sMRIPrep's FreeSurfer calls (the programs recon-all's recordings do not hold:
# `docs/freesurfer/programs.toml`, stream "fmriprep", run "bare") under the libm shim, on a
# finished subject: one folder per mode, the outputs kept for the comparison.
#
# Usage: fmriprep_calls.sh <freesurfer home> <subjects dir> <subject> <boldref> <t1w ref> \
#            <bold in T1w space> <libm_shim.so> <out folder> <count|cr> [functions]
#   <boldref>             fMRIPrep's coregistration reference (desc-coreg_boldref)
#   <t1w ref>             fMRIPrep's T1w reference (desc-preproc_T1w)
#   <bold in T1w space>   a volume on the T1w reference's grid (mri_vol2surf needs the LTA's source
#                         geometry: the T1w reference itself stands in for the BOLD series, the
#                         same code path per volume)
#   [functions]           LARMORX_LIBM_SHIM_FUNCS for cr (comma-separated; all when empty)
# The subject is copied into <out>/<mode>/subjects (the original is never written). One thread
# everywhere (OMP_NUM_THREADS=1, as the 1-thread recordings), so two runs are comparable.
# Writes <out>/<mode>/shim.log (the shim's counts) and <out>/<mode>/calls.log (each call, its exit).
# FMRIPREP_CALLS_ONLY (space-separated call names, e.g. "mri_coreg") runs only those calls, in a
# folder <out>/<mode>[-<functions>]-only (the calls depend on earlier ones: name them too).
set -u
[ $# -ge 9 ] || { sed -n '6,18p' "$0"; exit 2; }
FSHOME=$1 SRCSD=$2 SUBJ=$3 BOLDREF=$4 T1WREF=$5 BOLDT1W=$6 SHIM=$7 OUT=$8 MODE=$9 FUNCS=${10:-}
ONLY=${FMRIPREP_CALLS_ONLY:-}
RUN=$OUT/$MODE${FUNCS:+-${FUNCS//,/-}}${ONLY:+-only}
[ -e "$RUN" ] && { echo "fmriprep_calls.sh: $RUN exists" >&2; exit 2; }
mkdir -p "$RUN/subjects" "$RUN/work"
cp -a "$SRCSD/$SUBJ" "$RUN/subjects/"
for s in fsaverage fsaverage5; do ln -s "$FSHOME/subjects/$s" "$RUN/subjects/$s"; done
SD=$RUN/subjects
export FREESURFER_HOME=$FSHOME FREESURFER=$FSHOME SUBJECTS_DIR=$SD
export FS_LICENSE=${FS_LICENSE:-/usr/local/freesurfer/license.txt}
export PATH=$FSHOME/bin:$PATH LANG=C LC_ALL=C
export OMP_NUM_THREADS=1 FS_OMP_NUM_THREADS=1 ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1
export LARMORX_LIBM_SHIM=$MODE LARMORX_LIBM_SHIM_LOG=$RUN/shim.log
[ -n "$FUNCS" ] && export LARMORX_LIBM_SHIM_FUNCS=$FUNCS
W=$RUN/work
call() {  # call <name> <program> <args...>: under the shim, from the work folder
  local name=$1; shift
  [ -n "$ONLY" ] && [[ " $ONLY " != *" $name "* ]] && return 0
  local t0=$SECONDS
  (cd "$W" && env LD_PRELOAD="$SHIM" "$@" >"$W/$name.out" 2>&1)
  echo "$name exit $? $((SECONDS - t0)) s" | tee -a "$RUN/calls.log"
}
S=$SD/$SUBJ
# fMRIPrep's init_bbreg_wf (bold2anat_dof 6, no T2w): mri_coreg, then bbregister from it
call mri_coreg mri_coreg --s "$SUBJ" --sd "$SD" --mov "$BOLDREF" --lta "$W/coreg.lta" --dof 6 \
  --sep 4 --ftol 1.000000e-04 --linmintol 1.000000e-02
call bbregister bbregister --t2 --6 --init-reg "$W/coreg.lta" --initcost "$W/bbregister.initcost" \
  --lta "$W/bbreg.lta" --mov "$BOLDREF" --reg "$W/bbreg.dat" --s "$SUBJ"
# sMRIPrep: fsnative2t1w (RobustRegister auto_sens, est_int_scale), midthickness, GIFTI surfaces
call mri_robust_register mri_robust_register --satit --iscale --mov "$S/mri/T1.mgz" --dst "$T1WREF" \
  --lta "$W/rr.lta"
call mris_expand mris_expand -thickness "$S/surf/lh.white" 0.5 "$W/lh.midthickness"
call mris_convert_pial mris_convert --to-scanner "$S/surf/lh.pial" "$W/lh.pial.gii"
call mris_convert_thickness mris_convert -c "$S/surf/lh.thickness" "$S/surf/lh.white" \
  "$W/lh.thickness.gii"
call mris_convert_sphere mris_convert "$S/surf/lh.sphere.reg" "$W/lh.sphere.reg.gii"
# fMRIPrep's init_bold_surf_wf: the T1w-to-fsnative LTA, then mri_vol2surf (fsnative, fsaverage5)
call lta_convert lta_convert --inlta "$W/rr.lta" --outlta "$W/rr_inv.lta" --invert
call mri_vol2surf_fsnative mri_vol2surf --mov "$BOLDT1W" --reg "$W/rr_inv.lta" --hemi lh \
  --interp trilinear --projfrac-avg 0 1 0.2 --trgsubject "$SUBJ" --srcsubject "$SUBJ" \
  --out_type gii --o "$W/v2s_fsnative.gii"
call mri_vol2surf_fsaverage5 mri_vol2surf --mov "$BOLDT1W" --reg "$W/rr_inv.lta" --hemi lh \
  --interp trilinear --projfrac-avg 0 1 0.2 --trgsubject fsaverage5 --srcsubject "$SUBJ" \
  --out_type gii --o "$W/v2s_fsaverage5.gii"
# fMRIPrep's several-T1w template (two images of one head: the raw average and the T1w reference)
call mri_robust_template mri_robust_template --satit --mov "$S/mri/rawavg.mgz" "$T1WREF" --inittp 1 \
  --iscale --subsample 200 --fixtp --noit --lta "$W/rt1.lta" "$W/rt2.lta" --template "$W/rt.mgz"
echo "CALLS-DONE" | tee -a "$RUN/calls.log"
