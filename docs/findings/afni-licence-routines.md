# AFNI's MCW routines behind 3dAutomask and 3dUnifize (restricted)

> **Restricted: clean-room implementers must not read this file** (CLAUDE.md rule 1). It names the
> library routines of AFNI 25.2.09 that decide the results of 3dAutomask and 3dUnifize and what each
> decides; it was written from AFNI's source by the replica author (2026-10-23) and moved here from
> `docs/licensing.md` on 2026-10-24, so that the licence page holds no source-derived detail.

Why it matters for the licence: `docs/licensing.md`, "AFNI: one implementation or two is decided
per routine".

| MCW routine (GPL-2) | File | What it decides | Program |
|---|---|---|---|
| `THD_extract_array` | `thd_dsetto1D.c` | a voxel's series and its scaling (positive factors only: AFNI-01) | both (the average of a series; `-EPI`'s median) |
| `mri_to_float`, `mri_scale_to_float` | `mri_to_float.c` | one sub-brick as float, its scaling | both |
| `mri_max` | `mri_max.c` | the clip level histogram's top | both |
| `mri_cut_3D` | `mri_cut.c` | the octants' boxes of the gradual clip level | both |
| `THD_countmask` | `thd_makemask.c` | the voxel counts printed, "No voxels?" | 3dAutomask |
| `thd_floatscan` | `thd_floatscan.c` | non-finite values set to 0 | both |
| `THD_need_brick_factor` | `thd_initdblk.c` | whether factors are applied | both |
| `mri_new_vol`, `mri_new_conforming` | `mri_new.c` | zero-filled images | both |
| `MCW_spheremask`, `MCW_build_mask` | `edt_buildmask.c` | the ball of the local white-matter level | 3dUnifize |
| `qsort_float` (with `cs_qsort_small.h`) | `mri_percents.c` | the sorted ball values; `-T2`'s percentile | 3dUnifize |
| `qmed_float` | `cs_qmed.c` | `-GM`'s medians, `-EPI`'s median | 3dUnifize |
| `AFNI_prefilter_args`, `THD_filename_ok`, `THD_write_3dim_dataset`, ... | `afni_environ.c`, `thd_filestuff.c`, `thd_writedset.c` | global options, output names, existing outputs | both |
