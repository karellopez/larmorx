# Provenance: larmorx-gpl-afni

**Replica: translated from AFNI's source.** This crate is a step-by-step Rust translation of
AFNI 25.2.09 (tag `AFNI_25.2.09`, commit `b1e12b26dae2`, see `upstream.tsv`), written to give
AFNI's output bit for bit. It is distributed under the GNU General Public License, version 3
or later (`../LICENSE`), as part of the `larmorx-gpl` package (`docs/licensing.md`). The
Apache-2.0 clean-room original of the same tool is `crates/larmorx-afni`; neither crate uses
the other's code.

## Licences of the translated sources

- **AFNI files copyrighted by the Medical College of Wisconsin.** Their header reads "Major
  portions of this software are copyrighted by the Medical College of Wisconsin, 1994-2000,
  and are released under the Gnu General Public License, Version 2", and
  `doc/README/README.copyright` says: "The MCW-copyrighted part of this software is released
  to the public under the GNU General Public License, Version 2 (or any later edition)." AFNI's
  `LICENSE.txt` lists them as an exception to its public-domain notice ("Major portions of
  this software are copyrighted by the Medical College of Wisconsin; Authors: Robert W. Cox,
  et al.; License: GNU General Public License 2").
- **AFNI files written at the NIH** (no MCW header): public domain, a "United States
  Government Work" (`LICENSE.txt`). The NIfTI library (`src/nifti/`) states "This code is
  released to the public domain."
- **The GNU C Library 2.35** (`glibc_sincosf.rs`): "Copyright (C) 2018-2022 Free Software
  Foundation, Inc.", licensed under the GNU Lesser General Public License version 2.1 or any
  later version. Section 3 of the LGPL-2.1 allows applying the GNU GPL (version 2 or later)
  instead, so the translation is part of this GPL-3.0-or-later crate. Source: the release
  tarball `https://ftp.gnu.org/gnu/glibc/glibc-2.35.tar.xz` (SHA-256 in `upstream.tsv`).

Every source file starts with an SPDX header naming its licence and the files it translates.
Changes from the originals are listed below and in each module's documentation.

## AFNI files and functions translated

| Rust module | AFNI 25.2.09 file | Functions, macros | Copyright |
|---|---|---|---|
| `csfft` | `src/csfft.c` | `csfft_cox` (with its `fftn` routing test), `csfft_trigconsts`, the `fft2`/`fft4` macros, `fft8`, `fft16`, `fft32` (generated code, as one loop), `fft64`, `fft128`, `fft256`, `fft512`, `fft_4dec`, `fft_3dec`, `fft_5dec`, `csfft_nextup`, `csfft_nextup_one35`, `csfft_nextup_even` | MCW 1994-2000 (radix-2 code by Andrzej Jesmanowicz, the rest by Robert W. Cox) |
| `shift` | `src/thd_shift2.c` | `SHIFT_set_method`, `SHIFT_get_method`, `SHIFT_two_rows`, `fft_shift2` (with `ZFILL`, `RECUR`), `lin_shift`, `cub_shift`, `quint_shift`, `hept_shift`, `wsinc5_shift`, `wsinc9_shift`, `nn_shift`, `ts_shift` and their `*_shift2` wrappers; the `P_*`, `Q_*`, `S_*`, `FINS`, `sinc`, `M3`, `wwsinc5`, `wwsinc9` macros | MCW 1994-2000 |
| `shift` | `src/mrilib.h` | `CMULT`, `CEXPIT`, `PI` | MCW 1994-2000 |
| `detrend` | `src/thd_detrend.c` | `THD_const_detrend`, `get_linear_trend`, `THD_linear_detrend` | MCW 1994-2000 |
| `tpattern` | `src/thd_timeof.c` | `TS_parse_tpattern` | MCW 1994-2000 |
| `oned` | `src/mri_read.c` | `my_fgets`, `iznogood_1D`, `decode_linebuf`, `mri_read_ascii`, `mri_read_1D` (files and `1D:` strings) | MCW 1994-2000 |
| `oned` | `src/cs_fgets.c` | `afni_fgets` | NIH, public domain |
| `oned` | `src/mri_fromstring.c` | `mri_1D_fromstring` | NIH, public domain |
| `oned` | `src/niml/niml_header.c` | `NI_decode_string_list` | NIH, public domain |
| `afni_ext` | `src/thd_niftiread.c` | `THD_nifti_process_afni_ext` | NIH, public domain |
| `afni_ext` | `src/thd_nimlatr.c` | `THD_dblkatr_from_niml` | NIH, public domain |
| `afni_ext` | `src/thd_initdblk.c` | `THD_datablock_apply_atr` (the `IJK_TO_DICOM_REAL`, `TAXIS_*` and `TEMPLATE_SPACE` parts) | MCW 1994-2000 |
| `afni_ext` | `src/thd_zblock.c` | `THD_unzblock` | MCW 1994-2000 |
| `afni_ext` | `src/niml/niml_elemio.c` | the text decoding of `NI_read_element`, `NI_decode_one_double`, `NI_decode_one_string`, `header_stuff_is_group` | NIH, public domain |
| `dataset` | `src/thd_atlas.c`, `src/AFNI_atlas_spaces.niml` | `THD_get_generic_space` (the generic space of each standard template space) | NIH, public domain |
| `dataset` | `src/thd_niftiread.c` | `THD_open_nifti` (datum, brick factors, time axis, slice offsets, the choice of qform or sform, the geometry it keeps), `THD_load_nifti` (conversion to float, scaling, float scan), `NIFTI_code_to_view`, `NIFTI_code_to_space` | NIH, public domain |
| `dataset` | `src/thd_niftiwrite.c` | `populate_nifti_image` (datum, scaling, geometry, dimensions, time and slice-timing fields, units, codes), `get_slice_timing_pattern`, `space_to_NIFTI_code` | NIH, public domain |
| `dataset` | `src/nifti/nifti2/nifti2_io.c` | the header conversion of `nifti_image_read` (`nifti_convert_n1hdr2nim`, `nifti_convert_n2hdr2nim`: dimension and spacing fixes, `FIXED_FLOAT`), `nifti_quatern_to_dmat44` | NIH, public domain |
| `dataset` | `src/thd_floatscan.c` | `thd_floatscan` | MCW 1994-2000 |
| `axes` | `src/thd_mattor.c` | `THD_matrix_to_orientation` | MCW 1994-2000 (no header; written by Robert W. Cox) |
| `axes` | `src/thd_coords.c` | `THD_dicom_card_xform`, `THD_compute_oblique_angle` (with its `MINNUM`, `MAXNUM`, `MIN3`, `MAX3`) | MCW 1994-2000 |
| `axes` | `src/thd_dsetatr.c` | the obliquity update of `THD_set_dataset_attributes` (a plumb `ijk_to_dicom_real` replaced by the cardinal matrix) | MCW 1994-2000 (no header) |
| `axes` | `src/thd_niftiwrite.c` | the spatial transforms of `populate_nifti_image` (sform, quaternion, qform offsets, voxel sizes) | NIH, public domain |
| `axes` | `src/3ddata.h`, `src/vecmat.h` | `ORIENT_xyz`, `ORIENT_sign`, `ORIENT_xyzint`; `DMAT_DET`, `DMAT_MUL` | MCW 1994-2000 |
| `axes` | `src/nifti/nifti2/nifti2_io.c` | `nifti_dmat44_to_orientation`, `nifti_dmat44_to_quatern`, `nifti_dmat33_polar`, `nifti_dmat33_inverse`, `nifti_dmat33_determ`, `nifti_dmat33_rownorm`, `nifti_dmat33_colnorm`, `nifti_dmat33_mul` | NIH, public domain |
| `dataset` | `src/thd_niftiread.c` | the axes of `THD_open_nifti` (orientation, steps and origins for the qform, sform and form-less cases) | NIH, public domain |
| `cli::tshift` | `src/thd_writedset.c` | the existing-output part of `THD_write_3dim_dataset`: `AFNI_DECONFLICT`'s overwrite, rename and refusal, with their messages and exit status (AFNI-11 reproduced) | MCW 1994-2000 |
| `cli::tshift` | `src/thd_opendset.c` | `THD_deconflict_prefix`, `THD_deconflict_nifti` | MCW 1994-2000 |
| `cli::tshift` | `src/afni_environ.c` | `THD_deathcon`, `THD_ok_overwrite` | MCW 1994-2000 |
| `cli::tshift` | `src/afni_logger.c` | the `AFNI_ECHO_COMMANDLINE` part of `AFNI_logger` (no log file is written) | NIH, public domain |
| `cli::tshift` | `src/thd_notes.c` | `tross_commandline` | MCW 1994-2000 |
| `cli::tshift` | `src/thd_mastery.c` | the blank (`AFNI_PATH_SPACES_OK`) and `~/` cases of `THD_open_dataset` | MCW 1994-2000 |
| `cli::tshift` | `src/thd_opentcat.c` | the opening of each listed dataset in `THD_open_tcat`, and its warning (joining several in time is not translated) | NIH, public domain |
| `environ` | `src/afni_environ.c` | `AFNI_prefilter_args` (the `-Dname=val`, `-overwrite`, `-ok_1D_text` and `-skip_afnirc` options), `AFNI_setenv`, `AFNI_yesenv`, `AFNI_noenv`, `AFNI_numenv` | MCW 1994-2000 |
| `environ` | `src/afni_environ.h` | `YESSISH`, `NOISH` | MCW 1994-2000 |
| `message` | `src/debugtrace.c` | `output_message` (the labels, `AFNI_MESSAGE_PREFIX`, `AFNI_MESSAGE_COLORIZE`), `INFO_message`, `ININFO_message`, `WARNING_message`, `ERROR_message`, `ERROR_exit` | NIH, public domain |
| `dataset` | `src/thd_coords.c` | `THD_report_obliquity` | MCW 1994-2000 |
| `dataset` | `src/thd_writedset.c` | the `AFNI_ALLOW_MILLISECONDS` and one-row (`AFNI_WRITE_1D_AS_PREFIX`) tests of `THD_write_3dim_dataset` | MCW 1994-2000 |
| `dataset` | `src/edt_fullcopy.c` | `EDIT_full_copy` (the order of copying and loading) | MCW 1994-2000 |
| `dataset` | `src/3ddata.h` | `DSET_UNMSEC` | MCW 1994-2000 |
| `tshift` | `src/3dTshift.c` | the slice and voxel-pair loop of `main` | MCW 1994-2000 |
| `tshift` | `src/thd_dsetto1D.c` | `THD_extract_series`, `THD_extract_array` | MCW 1994-2000 |
| `tshift` | `src/thd_1Dtodset.c` | `THD_insert_series` | MCW 1994-2000 |
| `tshift` | `src/thd_initdblk.c` | `THD_need_brick_factor` | MCW 1994-2000 |
| `tshift` | `src/mrilib.h` | `SHORTIZE`, `BYTEIZE` | MCW 1994-2000 |
| `cli::tshift` | `src/3dTshift.c` | `main` (option scan, checks, messages), `TS_copy_input_to_output` | MCW 1994-2000 |
| `cli::tshift` | `src/thd_filestuff.c` | `THD_filename_ok` (to name AFNI-12, and to refuse a `-prefix` when it is reproduced) | MCW 1994-2000 |
| `cli::tshift` | `src/nifti/nifti2/nifti2_io.c` | the message of `nifti_image_write_engine` when the output cannot be opened (AFNI-26, reproduced) | NIH, public domain |

### 3dAutomask and 3dUnifize (2026-10-23)

The licence of every routine that shapes their results is in
`docs/findings/afni-automask.md` and `afni-unifize.md` ("The licence of every routine"): MCW
library code is involved, so both are replicas here, and clean-room originals are to follow
from `specs/3dAutomask.md` and `specs/3dUnifize.md`.

| Rust module | AFNI 25.2.09 file | Functions, macros | Copyright |
|---|---|---|---|
| `mask` | `src/thd_automask.c` | `THD_automask_verbose`, `_extclip`, `_set_clipfrac`, `_set_peelcounts`, `_set_gradualize` (as `MaskSettings`), `mask_count`, `mri_automask_image`, `mri_automask_image2D`, `THD_mask_clip_neighbors`, `THD_mask_fillin_once`, `THD_mask_fillin_completely`, `THD_mask_clust`, `THD_mask_clust2D`, `THD_mask_erode` (without `redilate`), `THD_mask_erodemany`, `THD_mask_erodemany2D`, `THD_mask_dilate`, `THD_mask_depth` | NIH, public domain |
| `mask` | `src/thd_cliplevel.c` | `THD_cliplevel` (float images), `THD_cliplevel_partial`, `get_octant_clips`, `pointclip`, `THD_cliplevel_gradual` | NIH, public domain |
| `mask` | `src/mri_get_cmass.c` | `mri_get_cmass_3D` | NIH, public domain |
| `mask` | `src/mri_max.c`, `src/mri_cut.c`, `src/thd_makemask.c` | `mri_max` (float), `mri_cut_3D`, `THD_countmask` | MCW 1994-2000 |
| `automask` | `src/thd_automask.c` | `THD_automask`, `thd_apply_mask` | NIH, public domain |
| `automask` | `src/thd_median.c` | `THD_aveabs_brick` | NIH, public domain |
| `automask` | `src/3dAutomask.c` | the dilation and erosion loop, the cut-plane report and the `-SI` clipping of `main` | NIH, public domain |
| `automask` | `src/thd_dsetto1D.c`, `src/mri_to_float.c`, `src/thd_initdblk.c` | `THD_extract_array`, `mri_to_float`, `mri_scale_to_float`, `THD_need_brick_factor` | MCW 1994-2000 |
| `cli::automask` | `src/3dAutomask.c` | `main` (option scan, checks, messages, outputs); the `AUTHOR`, `WROTE_DSET`, `CHECK_OPEN_ERROR` macros of `src/mrilib.h` | NIH, public domain (mrilib.h: MCW 1994-2000) |
| `unifize` | `src/3dUnifize.c` | `median7`, `mri_double_down`, `mri_double_up`, `mri_local_percmean`, `mri_WMunifize`, `mri_GMunifize`, `mri_clipedges_inplace`, `PKVAL`, `PKMID`, `WMCUT`, `WMSCL` | NIH, public domain |
| `unifize` | `src/mri_invert.c` | `mri_invertcontrast_inplace` (float images) | NIH, public domain |
| `unifize` | `src/thd_dset_nbhd.c` | `mri_get_nbhd_array` with `SetSearchAboutMaskedVoxel(1)` | NIH, public domain |
| `unifize` | `src/edt_buildmask.c` | `MCW_build_mask`, `MCW_spheremask` (unit voxels) | MCW 1994-2000 |
| `unifize` | `src/cs_qmed.c` | `qmed_float`, `median_float4`, `median_float5`, `median_float7`, `median_float9`, `MED3` | MCW 1994-2000 |
| `unifize` | `src/mri_percents.c` | `qsort_float` (as a sort: any correct sort gives its result on finite values) | MCW 1994-2000 |
| `cli::unifize` | `src/3dUnifize.c` | `main` (option scan, checks, messages, the `-EPI` path, outputs) | NIH, public domain |
| `cli::unifize` | `src/thd_dsetto3D.c`, `src/thd_median.c` | `THD_extract_float_brick`, `THD_median_brick` | NIH, public domain |
| `cli::common` | (moved from `cli::tshift`, 2026-10-23) | the shared command-line pieces listed under `cli::tshift` above | as above |
| `environ` | `src/afni_environ.c`, `src/Aomp.h` | the variables of 3dAutomask and 3dUnifize (`environ::AUTOMASK`, `UNIFIZE`; `AFNI_SETUP_OMP`'s `OMP_NUM_THREADS`) | MCW 1994-2000 |
| `dataset` | `src/thd_niftiread.c` | `THD_load_nifti`'s brick factor of a loaded dataset (`read_loaded`) | NIH, public domain |

Not translations (written for larmorx-gpl, GPL-3.0-or-later): `x87` (the arithmetic of x86-64's
`long double`, for the formulas of `nifti2_io.c` that use it) and `cli::output` (writing the
output through a temporary file and a rename; `cannot_open`, whether AFNI's NIfTI writer could
open the output, for AFNI-26).

## glibc files translated (`glibc_sincosf`)

| glibc 2.35 file | What |
|---|---|
| `sysdeps/ieee754/flt-32/s_sinf.c`, `s_cosf.c` | `sinf`, `cosf` |
| `sysdeps/ieee754/flt-32/s_sincosf.h` | `abstop12`, `reduce_fast`, `reduce_large`, `pi63`, `pio4` |
| `sysdeps/x86/fpu/sincosf_poly.h` | `sinf_poly` (x86 version) |
| `sysdeps/x86/fpu/s_sincosf_data.c` | `__sincosf_table`, `__inv_pio4` |
| `sysdeps/x86_64/fpu/multiarch/s_sinf-fma.c`, `s_cosf-fma.c`, `s_sinf-sse2.c`, `s_cosf-sse2.c` | the FMA and SSE2 builds selected at run time |

## glibc files translated (`glibc_floatmath`, 2026-10-23)

| glibc 2.35 file | What |
|---|---|
| `sysdeps/ieee754/flt-32/s_tanhf.c` | `tanhf` (from fdlibm: "Copyright (C) 1993 by Sun Microsystems, Inc. ... Permission to use, copy, modify, and distribute this software is freely granted, provided that this notice is preserved", kept in the module's header) |
| `sysdeps/ieee754/flt-32/s_expm1f.c` | `expm1f` (fdlibm, the same notice) |
| `sysdeps/ieee754/flt-32/s_cbrtf.c` | `cbrtf` (Copyright (C) 1997-2022 Free Software Foundation, Inc.) |

On x86-64 these have one build each (no FMA variant). `frexpf` and `ldexpf` are written as
their exact definitions.

## Changes from the originals

- **No global state.** AFNI keeps work arrays and FFT tables in static variables; here they
  are per-thread values (`ShiftWork`, `CsfftPlan`), so voxel pairs run in parallel. The
  arithmetic of each pair is unchanged, so results do not depend on the number of threads.
- **`fft8`, `fft16`, `fft32`** are one loop instead of AFNI's generated code;
  `tools/csfft-verify/check_unrolled.py` checks that the loop performs exactly the generated
  butterflies.
- **Double `cos`/`sin`** come from `larmorx_core::math` (correctly rounded) instead of glibc's;
  only their values rounded to float are used, and they agree with glibc's on every argument
  the FFT tables use (`tools/csfft-verify`).
- **`fftn`** (AFNI's FFT for other lengths) is not translated: 3dTshift needs it only for
  series longer than 32764 points, which are refused.
- **Undefined behaviour in C** gets a defined result: `fft_shift2` with a too-large shift and
  no second row clears only the first (AFNI writes through a NULL pointer); `(int)` of a float
  beyond the `int` range saturates.
- **Inputs not supported** (an error instead): datasets that are not NIfTI, complex and RGB
  data, sub-brick selectors, `-voxshift`, 1D `.tsv`/`.csv` tables, standard input, and the
  1D `[...]`/`{...}` selectors. Of AFNI's header extension only the attributes 3dTshift's
  output depends on are read (time axis, matrix, space); none is written.
- **AFNI's environment** (2026-10-12). AFNI changes its process environment (`putenv`) for
  each `-Dname=val`; the replica never does (`std::env::set_var` is unsafe in a multithreaded
  program): it reads the environment once into a table of settings, applies the global
  options to it, and passes it to every routine that reads a variable (`environ::Settings`).
  `~/.afnirc` is not read (as with `-skip_afnirc`). Of AFNI's global options, `-overwrite`,
  `-Dname=val`, `-ok_1D_text` and `-skip_afnirc` are taken; the others (`-Vname=`,
  `-echo_edu`, `-np`, `-all_opts`, `-h_view`, `<<` inclusions, ...) are unknown options. Not
  reproduced: AFNI's version banner and the compile date after a fatal error (they name
  AFNI's build), the log file `~/.afni.log` (and its size warning; the command line is echoed
  when `HOME` is a directory), AFNI's debugging output (`AFNI_TRACE`, `AFNI_DEBUG`, ...).
- **Writing the output** (2026-10-12): to a temporary file in the output's directory, renamed
  over the output (`cli::output::replace_atomically`), so an existing output is replaced whole
  or left as it was; on Windows the rename is retried for about 2 s while another program
  holds the file. AFNI writes in place.
- **Output** (the larmorx conventions shared with `larmorx afni 3dTshift`): NIfTI only, and
  an existing output file is an error. Any `-prefix` path is accepted (2026-10-11), so that
  paths with blanks or non-ASCII characters work on every platform whichever implementation
  `larmorx afni 3dTshift` runs. AFNI refuses the names `THD_filename_ok` rejects unless
  `AFNI_ALLOW_ARBITRARY_FILENAMES` is set. Both are AFNI defects corrected by default and
  reproduced with the switch (AFNI-11, AFNI-12; below).
- **`-verbose`** starts with a line naming the implementation (the replica, with its version)
  and one naming the D13 mode.
- **AFNI's deterministic defects are corrected by default** (D13, 2026-10-12;
  `docs/findings/upstream-defects.md`), as the clean-room original corrects them, and named in
  a warning on standard error; `LARMORX_UPSTREAM_DEFECTS=reproduce` restores AFNI's code
  paths, bit for bit:
  - AFNI-01 (`tshift::extract`): a negative brick factor is applied when the series is
    extracted, as `THD_insert_series` divides by it; AFNI applies only a positive one.
  - AFNI-02 (`dataset::read`): float32 data with a slope and an intercept, scaled when loaded,
    get no brick factor; AFNI keeps the slope as one.
  - AFNI-03 (`dataset::slice_offsets`): header slice code 2 visits `slice_end` down to
    `slice_start`; AFNI's loop visits `slice_end` only.
  - AFNI-05 (`tshift::shift_pair`): `-no_detrend` de-means the second voxel of each pair with
    its own mean; AFNI de-means the first voxel twice.
  - AFNI-07 (`dataset::geometry`): a metre or micrometre header's matrix and origins are
    converted to millimetres with its steps; AFNI converts only the steps.
  - AFNI-08 (`dataset::slice_fields`): the "all zeros" test of `populate_nifti_image` is what
    it was meant to be (no time away from 0); AFNI's can never be true.
  - AFNI-11, AFNI-12 (`cli::tshift`): the larmorx conventions above. Reproduced, an existing
    output is reported with AFNI's messages and not written (exit 0), and a name
    `THD_filename_ok` rejects is refused. Since 2026-10-12 `-overwrite`, `AFNI_DECONFLICT` and
    `AFNI_ALLOW_ARBITRARY_FILENAMES` are read in both modes, as AFNI reads them.
  - AFNI-16 (`axes::written`, `axes::snaps`; decided 2026-10-12): a matrix that
    `THD_compute_oblique_angle` calls plumb (within about 0.01° to 0.024° of plumb, depending on
    the step) but that has a
    non-zero entry off its axes' rows keeps its obliquity: it is written as AFNI writes an
    oblique matrix (the sform as carried, its quaternion, the axes' origins and steps), where
    `THD_set_dataset_attributes` replaces it by the cardinal matrix of the axes. A matrix
    without such an entry is written from its axes in both modes.
  - AFNI-18 (`cli::tshift::open_dataset`; 2026-10-12): an input name with a blank is one file;
    AFNI reads a list of datasets unless `AFNI_PATH_SPACES_OK` is yes. Reproduced, a list of
    one dataset is that dataset; several are an error (AFNI joins them in time).
  - AFNI-19 (`dataset::write`): with `AFNI_ALLOW_MILLISECONDS`, `toffset` is converted to
    seconds with the TR; AFNI skips `DSET_UNMSEC` and writes it in milliseconds.
  - AFNI-20 (`dataset::write`): a dataset of one row is written as NIfTI in both modes; AFNI
    writes a 1D text file (with a random identifier) unless `AFNI_WRITE_1D_AS_PREFIX` is yes.
  - AFNI-22 (`environ::prefilter`): `-Dname=value`'s value is the text after the first `=`;
    AFNI's `sscanf` reading (`environ::afni_setenv`) is used in `reproduce` mode.
  - AFNI-23 (`dataset::read`, `dataset::write`, `cli::tshift::rlt_dropped_intercept`; user
    decision 2026-10-12): with `AFNI_NIFTI_SCALE=NO`, `THD_open_nifti` keeps uint8 and int16
    data with a slope and an intercept as stored, with the slope as brick factor, and the
    intercept is lost (`THD_load_nifti` scales only float bricks). Both modes read them as AFNI
    does; by default the output's `scl_inter` is the input's intercept (`scl_slope` 1 when
    there is no factor), so it reads back as the input's values shifted. With `-rlt` the trend
    takes the intercept with it: without `-ignore` nothing changes; with `-ignore` the data are
    read as `THD_load_nifti` reads them without the variable (float, `slope * x + inter`).
  - AFNI-21 (`message::Style`; messages only): AFNI's terminal colour codes around warning and
    error labels are written in `reproduce` mode only.
- **`long double`** (2026-10-12; until then the translation computed these in double).
  `nifti_dmat44_to_quatern`, `nifti_quatern_to_dmat44` and `nifti_dmat33_inverse` mix
  `long double` constants into double formulas (`0.25l * (r32-r23) / a`,
  `r11 + r22 + r33 + 1.0l`, `1.0l - (b*b+c*c+d*d)`, `2.0l * (b*c-a*d) * yd`, `1.0l / deti`).
  On x86-64, AFNI's platform, these are x87 operations rounded to 64 bits, then to double;
  `x87::add`, `mul` and `div` compute the same doubles with integer arithmetic, on every
  platform (AFNI-24). NIfTI-2 output shows the difference (its quaternion fields are doubles).
- **The axes' orientation** where `nifti_dmat44_to_orientation` fails (a degenerate sform, a
  zero or repeated column): AFNI indexes its orientation tables out of bounds there; the
  replica writes the matrix as read.

## Verification

| What | How | Result (2026-10-10) |
|---|---|---|
| `csfft` | `tools/csfft-verify/run.sh`: AFNI's compiled `csfft_cox` (from the oracle build's `libmri.a`) and its general radix-2 loop on 3,336 + 160 input records, every length AFNI computes itself | 36,950,784 + 2,621,400 float values, 0 differ |
| `glibc_sincosf` | `tools/sincosf-verify`: all 2^32 float inputs against glibc 2.35 on an x86-64 CPU with FMA | 0 differ for `sinf` and for `cosf` |
| `glibc_floatmath` (2026-10-23) | `tools/floatmath-verify`: `tanhf` on all 2^32 inputs against glibc 2.35, input by input; both functions as a hash of all 2^32 results, against `glibc_reference.c` (a C program calling glibc) | 0 `tanhf` inputs differ; the hashes equal (`3c6bad00ea34c387`, `c679cb92d4402e33`) |
| 3dAutomask, 3dUnifize (2026-10-23) | `python -m larmorx_validation parity afni-automask` and `afni-unifize` against the AFNI 25.2.09 binary | see `docs/validation/afni-automask.md`, `afni-unifize.md` |
| 3dTshift | `python -m larmorx_validation parity afni-tshift --implementation replica` against the AFNI 25.2.09 binary | 217 of 217 compared cases bit-identical (`docs/validation/afni-tshift-replica.md`) |
| 3dTshift with D13 (2026-10-12) | the same suite, in `reproduce` mode, with a fix pass and the replica's default against the clean-room original's | see `docs/validation/afni-tshift-replica.md` (from `c479c1a`: 279 of 279 compared cases bit-identical, 3 expected divergences) |
| `x87` (2026-10-12) | the emulated operations against GCC's `long double` on x86-64: 3 million quotients of the kind `nifti_dmat44_to_quatern` forms, and 2 million random sums, products and quotients (scratch C programs) | 0 differ (1,071 of them differ from plain double arithmetic) |
| Global options and environment variables (2026-10-12) | the parity categories `global` and `environment`: exit status, standard output and error byte for byte, and the output files, against the AFNI binary in a clean environment; NIfTI-2 headers of 120 random oblique inputs and 160 qform-only, near-180° and trace-near-0 ones | 60 of 60 identical: exit status, messages and outputs (from `c479c1a`, AFNI-23's cases included; `environment/one-row-dataset`'s output is an expected divergence, AFNI-20); the headers identical except `vox_offset` |
| The geometry AFNI writes (2026-10-12) | AFNI's and the replica's (`reproduce`) outputs compared byte for byte on plumb, oblique, nearly plumb, qform-only, sform-only and form-less inputs, in millimetres, metres and micrometres, on AFNI-01, -02, -03, -05, -08 inputs, and on five real BOLD runs (ds000210 with AFNI's extension, ds001600, ds003345, ds005454, ds006736) | the 348-byte headers identical except `vox_offset` (AFNI's extension), the data identical; `tests/oracle.rs` checks 20 cases the same way |
