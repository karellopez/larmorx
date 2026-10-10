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
| `dataset` | `src/edt_fullcopy.c` | `EDIT_full_copy` (the order of copying and loading) | MCW 1994-2000 |
| `dataset` | `src/3ddata.h` | `DSET_UNMSEC` | MCW 1994-2000 |
| `tshift` | `src/3dTshift.c` | the slice and voxel-pair loop of `main` | MCW 1994-2000 |
| `tshift` | `src/thd_dsetto1D.c` | `THD_extract_series`, `THD_extract_array` | MCW 1994-2000 |
| `tshift` | `src/thd_1Dtodset.c` | `THD_insert_series` | MCW 1994-2000 |
| `tshift` | `src/thd_initdblk.c` | `THD_need_brick_factor` | MCW 1994-2000 |
| `tshift` | `src/mrilib.h` | `SHORTIZE`, `BYTEIZE` | MCW 1994-2000 |
| `cli::tshift` | `src/3dTshift.c` | `main` (option scan, checks, messages), `TS_copy_input_to_output` | MCW 1994-2000 |
| `cli::tshift` | `src/thd_filestuff.c` | `THD_filename_ok` | MCW 1994-2000 |
| `cli::tshift` | `src/debugtrace.c` | the message prefixes of `INFO_message`, `WARNING_message`, `ERROR_message`, `ERROR_exit` | NIH, public domain |

## glibc files translated (`glibc_sincosf`)

| glibc 2.35 file | What |
|---|---|
| `sysdeps/ieee754/flt-32/s_sinf.c`, `s_cosf.c` | `sinf`, `cosf` |
| `sysdeps/ieee754/flt-32/s_sincosf.h` | `abstop12`, `reduce_fast`, `reduce_large`, `pi63`, `pio4` |
| `sysdeps/x86/fpu/sincosf_poly.h` | `sinf_poly` (x86 version) |
| `sysdeps/x86/fpu/s_sincosf_data.c` | `__sincosf_table`, `__inv_pio4` |
| `sysdeps/x86_64/fpu/multiarch/s_sinf-fma.c`, `s_cosf-fma.c`, `s_sinf-sse2.c`, `s_cosf-sse2.c` | the FMA and SSE2 builds selected at run time |

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
  output depends on are read (time axis, matrix, space); none is written. The `AFNI_*`
  environment variables are not read.
- **Output** (the larmorx conventions shared with `larmorx afni 3dTshift`): NIfTI only, and
  an existing output file is an error.

## Verification

| What | How | Result (2026-10-10) |
|---|---|---|
| `csfft` | `tools/csfft-verify/run.sh`: AFNI's compiled `csfft_cox` (from the oracle build's `libmri.a`) and its general radix-2 loop on 3,336 + 160 input records, every length AFNI computes itself | 36,950,784 + 2,621,400 float values, 0 differ |
| `glibc_sincosf` | `tools/sincosf-verify`: all 2^32 float inputs against glibc 2.35 on an x86-64 CPU with FMA | 0 differ for `sinf` and for `cosf` |
| 3dTshift | `python -m larmorx_validation parity afni-tshift --implementation replica` against the AFNI 25.2.09 binary | `docs/validation/afni-tshift-replica.md` |
