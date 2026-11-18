# Provenance: larmorx-freesurfer-volumes

**Replica: translated from FreeSurfer's source.** This crate holds wave 3.2's programs of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): the volume programs both streams run (`mri_convert`, `mri_info`, `mri_normalize`, `mri_segment`, `mri_fill`, ...), fMRIPrep's `mri_coreg`, `mri_robust_register` and `mri_robust_template`, and the scripts around them (`mri_nu_correct.mni`, `seg2cc`, ...). It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

Created empty by queue item 3.1-01 (`docs/freesurfer/queue.md`); each item that brings code adds
its files and functions here, with the third-party code it found in them (CLAUDE.md rule 3: each
ported file is audited for third-party code first).

| FreeSurfer 8.2.0 | What | Here |
|---|---|---|
| `mri_normalize/mri_normalize.cpp` 47-160, 1001-1348 (queue item 3.2-24, track D relay 46) | the file statics the options set, `get_option`, `usage_exit`, `main`'s option loop (FS-1016) | `src/mri_normalize/options.rs` |
| `mri_normalize/mri_normalize.help.xml` | the help text `usage_exit` renders (FreeSurfer's file, copied unchanged) | `src/mri_normalize/mri_normalize.help.xml` |
| `mri_normalize/mri_normalize.cpp` 1350-2167 (3.2-24) | `compute_bias`, `MRIremoveWMOutliersAndRetainMedialSurface` and `MRIremoveWMOutliers` (the `#if 1` forms; FS-1014), `add_interior_points`, `remove_surface_outliers` and `remove_outliers_near_surface` (their bodies after the first `return` never run), `remove_nonwm_voxels`, `build_outside_of_brain_mask`, `remove_surface_outliers_T2` (FS-1015), `threshold_control_points`, `remove_controlpoints_outside_range` | `src/mri_normalize/helpers.rs` |

Third-party code: none in `mri_normalize.cpp` (FreeSurfer's own file, MGH's notice). Its library
calls are the other crates' (`larmorx-freesurfer-filter`'s normalisation, `-volume`, `-surf`,
`-gca`; `GCAsourceVoxelToPrior` added to `larmorx-freesurfer-gca` for `-atlas`).

## Changes from the originals

- Each function takes the volumes as `main` passes them: a control volume that is both a
  function's source and its destination is one `&mut Mri` (`main` never passes two).
- `remove_surface_outliers` and `remove_outliers_near_surface` do nothing (FreeSurfer's return at
  their start; the clone `remove_surface_outliers` makes first is never used).
- FS-1014 (the white-matter peak of an empty histogram: 0, as glibc's heap gives it), FS-1015 (a
  walk of `-T2`'s outlier removal past 10,000,000 steps ends the program), FS-1016 (an option
  short of its arguments: an error where FreeSurfer crashes; `(null)` where it reads the
  environment).
