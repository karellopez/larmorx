# freesurfer-mri-mask-singular: transforms singular only as voxel to voxel (FS-1135)

`search.py` finds linear transform arrays whose matrix inverts in single precision (so
`TransformRead`'s and `LTAchangeType`'s inversions pass) but whose voxel-to-voxel form for two
oblique volumes does not, the inputs of `mri_mask`'s FS-1135 (`MRIlinearTransform: xform is
singular`, then no mask). It holds no FreeSurfer code: it runs a program (the official `mri_mask`
or a candidate) as a black box on seeded inputs and counts the outcomes.

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 4 python -P validation/investigations/freesurfer-mri-mask-singular/search.py \
        --out <folder> --program '<FREESURFER_HOME>/bin/mri_mask' --type 0 --seed 11
    # a candidate: --program '<larmorx-freesurfer> --upstream-defects reproduce freesurfer mri_mask'

Run the official program with `FREESURFER_HOME`, `FS_LICENSE` and `FS_DISABLE_LANG=1` in the
environment and no diagnostic level. Relay 52 of track D (2026-11-22): with the port, seed 11 voxel
to voxel 25 of 400, seed 12 scanner to scanner 19 of 400 singular as voxel to voxel; the official
program on seed 11's first 100: 84 status 0, 9 `TransformInvert: xform noninvertible`, 7 singular
as voxel to voxel (cases 4, 7, 27, 63, 83, 87, 89: the port's cases). The probe's `SINGULAR_V2V` is
seed 11's case 4, `SINGULAR_RAS` seed 12's case 44.
