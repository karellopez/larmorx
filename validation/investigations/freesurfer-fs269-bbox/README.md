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
- `fill_interior_full_box.patch`: track D's `MRISfillInterior` box made the full one, for one run of
  `validation/probes/fs-surfio/real.py` (24 recorded calls: 24 of 24 identical with FreeSurfer in both
  modes), then reverted; it is not part of the port.

Run from the repository, each in a memory box:

    source scripts/orchestration/env.sh
    scripts/orchestration/boxed.sh 8 python -P validation/investigations/freesurfer-fs269-bbox/boxes.py --json boxes.json
    scripts/orchestration/boxed.sh 16 python -P validation/investigations/freesurfer-fs269-bbox/binarize.py --json bb.json
    git apply validation/investigations/freesurfer-fs269-bbox/fill_interior_full_box.patch
    scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-surfio/real.py --json fill.json
    git apply -R validation/investigations/freesurfer-fs269-bbox/fill_interior_full_box.patch

Measured 2026-11-23 (relay 45 of track C); logs `oracles/logs/fs-found-c-r45-binarize-bb.log`,
`fs-found-c-r45-fill-full-box.log`.
