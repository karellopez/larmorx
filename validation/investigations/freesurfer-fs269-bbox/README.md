# FS-269 measured: FreeSurfer 8.2.0's bounding boxes on the recorded heads (closed)

**Closed to FreeSurfer clean-room implementers**: these tools model FreeSurfer's box from its source
and run the replica. The findings: `docs/findings/freesurfer-volume.md` §32.

FreeSurfer's `REGIONgetBoundingBox` and `REGIONgetBoundingBoxM` give a box one voxel short of the
voxels from `min - npad` to `max + npad` (FS-269). Track E's spec relay 16 asked for the measurement
before a decision (rule D). What each tool shows:

- `boxes.py` (numpy, no program run): on the six recorded heads, the v8 stream's `mri_mask -bb 3
  orig.mgz orig.mgz invol.crop.nii.gz` (the SynthMorph step): FreeSurfer's box against the full one
  (they differ on all six heads, so correcting would change the network's input); and `mri_binarize
  --bb <0..3>` of three masks per head and stream: the mask voxels FreeSurfer's box leaves out (with a
  pad of 0: 7 to 390 per mask, 3,005 of 25,531,217 over 36 masks; with a pad of 1 or more: none).
- `binarize.py`: the official `mri_binarize` against the port in both modes (track D's
  `validation/probes/fs-binarize/real.py` with five bounding-box calls), 3 heads x 2 streams: 30 of 30
  identical with `reproduce`; `fix` identical with pads 1 and 2, FS-269 noted with a pad of 0.
- `vsinus.py`: the v8 stream's `mri_vsinus_seg` crop, `mri_mask -T .001 -crop 0 <prior> <prior> <mincrop>` (a pad of 0),
  on the six v8 heads: the prior made again by the script's own official steps (`mri_vol2vol` of FreeSurfer's MNI152
  prior by SynthMorph's recorded affine onto the recorded `nu.mgz`), `mincrop.mgz` from the official program and the
  port in both modes, then the rest of the step with the official programs (`mri_binarize --crop-around-ras`, the
  network `mri_sclimbic_seg` at recon-all's 4 threads, `mri_vol2vol --regheader`, the cortex mask) for FreeSurfer's box
  and for the corrected one, and recon-all's `mri_mask -oval 1 -invert` of `brain.finalsurfs.mgz` by each `vsinus.mgz`.
  FreeSurfer's chain gives the recorded files again (6 of 6); the corrected box moves the network's crop by a voxel and
  changes 553 to 1,105 voxels of `vsinus.mgz` on 6 of 6 heads (rule C for `mri_mask`'s boxes). The corrected box is
  made by the port's `mri_binarize --min .001 --bb 0` (`REGIONgetBoundingBox`, rule A): before the decision it was
  `mri_mask`'s own `fix`, and on the six heads the two give the same grid (size and centre, bit for bit), the same
  network input and the same `vsinus.mgz` (the network at 4 threads gave the same file in both runs). After the
  decision the port's `mri_mask` gives the official crop in both modes (6 of 6, FS-269 noted). Inputs and outputs in
  `<workspace>/oracles/inputs/fs269-vsinus/<run>/`; logs `oracles/logs/fs-found-c-r46-vsinus-6.log` (before the
  decision) and `fs-found-c-r46-vsinus-6-decided.log`.
- `fill_interior_full_box.patch`: track D's `MRISfillInterior` box made the full one, for one run of
  `validation/probes/fs-surfio/real.py` (24 recorded calls: 24 of 24 identical with FreeSurfer in both
  modes), then reverted; it is not part of the port.

Run from the repository, each in a memory box:

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 8 python -P validation/investigations/freesurfer-fs269-bbox/boxes.py --json boxes.json
    scripts/orchestration/boxed.sh 16 python -P validation/investigations/freesurfer-fs269-bbox/binarize.py --json bb.json
    scripts/orchestration/boxed.sh 24 python -P validation/investigations/freesurfer-fs269-bbox/vsinus.py --json vsinus.json
    git apply validation/investigations/freesurfer-fs269-bbox/fill_interior_full_box.patch
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-surfio/real.py --json fill.json
    git apply -R validation/investigations/freesurfer-fs269-bbox/fill_interior_full_box.patch

Measured 2026-11-23 (relay 45 of track C; `vsinus.py` relay 46); logs `oracles/logs/fs-found-c-r45-binarize-bb.log`,
`fs-found-c-r45-fill-full-box.log`, `fs-found-c-r46-vsinus-6.log`.
