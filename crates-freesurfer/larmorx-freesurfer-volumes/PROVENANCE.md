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
| `mri_normalize/mri_normalize.cpp` 162-994 (queue item 3.2-25, track D relay 47) | `main`: the version options, the options, `-atlas`'s mask, the label's points, `-renorm`, `-surface`, `-conform`, `-mask`, `-r`, `-l`, `-grad`, the float input, `-aseg`, the first pass, the aseg and 1-D normalisations, the gentle and 3-D passes, `-w`'s bias, the byte result, the timing line (FS-1017); the program `mri_normalize` of `larmorx-freesurfer` | `src/mri_normalize/main.rs` |
| `include/csv.h` (queue item 3.2-04, track A relay 46) | `CSVState`, `readCSVRow`, `readCSV` | `src/mri_convert/csv.rs` |
| `utils/stats.cpp` 53-415 (3.2-04, taken from 3.2-07) | `LoadStatTableFS`, `LoadStatTableCSV`, `LoadStatTableTSV`, `LoadStatTable`, `AllocStatTable`, `WriteStatTable`, `PrintStatTable`, `InitStatTableFromMRI` (FS-949, FS-950, FS-1031 to FS-1037), `STAT_TABLE` (`include/stats.h`) | `src/mri_convert/stat_table.rs` |
| `utils/fsgdf.cpp` 868-886 (3.2-04) | `gdfCountItemsInString` (FS-948) | `src/mri_convert/stat_table.rs` |
| `mri_convert/mri_convert.cpp` 57-73, 79-1882, 3889-4001 (3.2-04 and 3.2-05, first half) | the file statics, `main` from its start to the check of a GDF image stem (the option loop, its checks, the parse-only report; FS-1038 to FS-1042), `get_ints`, `get_floats`, `get_string`, `usage_message`, `usage`; `include/DICOMRead.h`'s globals the options set (`DicomSettings`) | `src/mri_convert/options.rs` |
| `mri_convert/mri_convert.help.xml` | the help text `usage` renders (FreeSurfer's file, copied unchanged) | `src/mri_convert/mri_convert.help.xml` |

Third-party code: none in `mri_normalize.cpp` (FreeSurfer's own file, MGH's notice). Its library
calls are the other crates' (`larmorx-freesurfer-filter`'s normalisation, `-volume`, `-surf`,
`-gca`; `GCAsourceVoxelToPrior` added to `larmorx-freesurfer-gca` for `-atlas`).

Third-party code: none in `csv.h`, `stats.cpp`, `fsgdf.cpp` and `mri_convert.cpp` (FreeSurfer's own, MGH's notice; `csv.h` carries no notice
and is FreeSurfer's). Their library calls are the other crates' (`larmorx-freesurfer-io`'s TSV
reader, file names, format identification and `MRIwrite`; `-core`'s matrices, version and help;
`-volume`'s orientation check) and `larmorx-core`'s `strtod`/`strtof` (C's conversions, black box).

## Changes from the originals

- Each function takes the volumes as `main` passes them: a control volume that is both a
  function's source and its destination is one `&mut Mri` (`main` never passes two).
- `remove_surface_outliers` and `remove_outliers_near_surface` do nothing (FreeSurfer's return at
  their start; the clone `remove_surface_outliers` makes first is never used).
- `main` (relay 47): its file statics are `Options`, `mrinorm.cpp`'s state (`NormState`), `mni` and
  `mri_not_control`, one struct (`Prog`) for the run; `Progname` is the name recon-all runs it by
  (`mri_normalize`); volumes FreeSurfer leaks are dropped when the C code stops using them (a
  lower peak: `MRIcopy`'s copy before `MRInormalize`, the bias field `-l` reads); the timer is the
  monotonic clock (fs.core's `Timer`). `MRIcombineDistanceTransforms(one, dist, dist)` (a source
  that is also the destination) is computed into a new volume of the same geometry, which holds
  the same values (it is voxel by voxel). FS-1017 (a step that gives no volume where FreeSurfer
  goes on with none, a crash: both modes end with an error).
- FS-1014 (the white-matter peak of an empty histogram: 0, as glibc's heap gives it), FS-1015 (a
  walk of `-T2`'s outlier removal past 10,000,000 steps ends the program), FS-1016 (an option
  short of its arguments: an error where FreeSurfer crashes; `(null)` where it reads the
  environment).
- `main`'s variables are one struct (`Options`), the unset ones `None` until an option sets them;
  the DICOM reader's globals the options set are kept in it (`DicomSettings`) until the DICOM
  reader is ported (queue item 3.7-14); `setenv`/`unsetenv` act on the program's environment
  table (`run::Environment`).
- `std::stof`'s `ERANGE` comes from fs.core's scanner on the characters `strtof` read (glibc's
  `strtof` and `%f` share the conversion).
- FS-948 (the item count's read past the line: the count kept, no notice), FS-949 and FS-950 (rule
  A), FS-1031 to FS-1042 (crashes and memory: both modes as `specs/mri_convert.md` says).
