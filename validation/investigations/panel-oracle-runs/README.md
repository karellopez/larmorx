# The data panel's fMRIPrep oracle runs: their BIDS inputs

The data panel (larmorx-testdata, `docs/panel.md`) adds nine datasets to the five that already
had an fMRIPrep 25.2.6 oracle run. Each new run reads a **BIDS view**: the dataset's panel files
hard-linked from the test-data cache in their BIDS layout, so fMRIPrep sees exactly the files the
catalog lists and verified, and nothing is copied or altered.

```bash
# one view per dataset, under the workspace's oracles/ (not committed: image data)
python -P bids_view.py ds004884 <workspace>/oracles/bids-views/ds004884
# then the run, as the five earlier ones (scripts/run_fmriprep_oracle.sh), at 6 threads:
NPROCS=6 OMP=6 scripts/run_fmriprep_oracle.sh <workspace>/oracles/bids-views/ds004884 \
    M2079 ds004884-sub-M2079-noreconall --fs-no-reconall
```

`bids_view.py` needs `larmorx_testdata` (the panel's catalog, `LARMORX_TESTDATA_ROOT` if the
checkout is not the installed one) and nibabel.

## The one file a view adds: ds004884's lesion mask on the T1w's grid

fMRIPrep 25.2.6 takes a lesion mask as `anat/sub-<label>[_ses-<label>]_label-lesion_roi.nii.gz`
in the T1w's space; sMRIPrep subtracts it voxel by voxel from a mask on the T1w's grid
(niworkflows' `create_cfm`) to keep the lesion out of the template registration's cost function.
The Aphasia Recovery Cohort's masks (`derivatives/lesion_masks/`) are all drawn on a T2w and
stored on that T2w's grid (each mask's sidecar names it as `SpatialReference`); in none of the
189 sessions that have a mask, a T1w and BOLD does a mask share the T1w's grid (the survey:
larmorx-testdata `tools/panel-survey/arc_lesions.py`).

The view therefore carries the mask **resampled onto the T1w's grid**, nearest neighbour through
both files' scanner coordinates, without a registration (`mask_on_grid`). For the chosen session
(sub-M2079, ses-217) a rigid registration of the T2w to the T1w, started from the headers, finds
0.38 degrees and 0.4 mm of residual motion between the two scans (at most 1.1 mm at the corners
of a brain-sized box; `arc_align.sh`, `arc_motion.py`; the best of the six sessions measured): the
headers alone align them to about one voxel. The resampled mask holds 176,205 voxels (176.2 mL,
the original's volume; SHA-256 `02440da7ebdb5625ce344e4997bd52545a656dc81b544c78c14514574cf962d0`)
and lies on the infarct in the T1w. The orchestrator accepted this derived input on 2026-10-24
(the alternative was a run without the mask); the test-data catalog keeps the original mask
unchanged.

## The runs

```bash
nohup bash validation/investigations/panel-oracle-runs/run_panel_oracles.sh <larmorx main checkout> &
```

runs the nine one after the other (`NPROCS=6 OMP=6`), logs to
`<workspace>/oracles/logs/panel-oracles.log` (one line per run with its exit status; the last line
`PANEL ORACLES FINISHED`) and each run's console to `panel-oracle-<run>.log` beside it. After each
run it appends to the run's `RUN_INFO.txt` the BIDS view it read and the commit of this directory,
and for ds004884 the derived lesion mask: its SHA-256, the upstream mask it was made from and how.
The checkout given supplies `scripts/run_fmriprep_oracle.sh` (the five earlier runs' script); its
parent is the workspace.
