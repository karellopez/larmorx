# AFNI 26.3.05's new licence: an inventory of the AFNI replicas (no moves)

**Status:** an inventory for the user's decision (version-update part, 2026-10-29). **Decided the
same day: the AFNI replicas move into the main package** (decision log 2026-10-29; next-steps item
3c, after the version update). Until that part, they stay in `larmorx-gpl`
(`crates-gpl/larmorx-gpl-afni`, GPL-3.0-or-later). Unrestricted: licences, file names and function names only, nothing of the
routines' content. Not legal advice.

The tools: `validation/investigations/version-afni/licence_inventory.py` (every file's licence
statement in both releases, word for word) and `validation/investigations/version-afni/glibc_permissive/`
(the C library's functions against a permissive source).

## What AFNI changed

AFNI 26.3.05's `LICENSE.txt` lists the Medical College of Wisconsin's code with a new licence:

> Location: Major portions of this software are copyrighted by the Medical College of Wisconsin
> Authors: Robert W. Cox, et al.
> License: Creative Commons Attribution License (CC BY 4.0) [doc/README/README.copyright and top of files]

(25.2.09: "License: GNU General Public License 2"). Its `doc/README/README.copyright` says why:
"These portions were originally available publicly with GPL-2.0 license. As of May 12, 2026, MCW
has granted NIH a Creative Commons Attribution License (CC BY 4.0) for all of the code under its
copyright. In brief, the CC-BY-4.0 allows for sharing and redistribution of material, as well as
adapting it, under the required terms of giving appropriate credit and indicating if any changes
were made." The NIH's own code stays in the public domain ("a United States Government Work"),
and `LICENSE.txt` now asks to "provide appropriate attribution and cite the authors in any work or
product based on this material". The header of every MCW file changed from "... released under the
Gnu General Public License, Version 2" to "... released under the Creative Commons Attribution
License (CC BY 4.0)". Several bundled third-party parts that were GPL left the list (gifsicle,
parts of jpeg-6b, RetroTS, mpegtoppm, part of niml_md5.c, svm, XmHTML, SUMA's gts); none is used
by the replicas.

## The AFNI files the replicas derive from

The 55 AFNI files named in `crates-gpl/larmorx-gpl-afni/PROVENANCE.md`, each file's own statement
(`licence_inventory.py`):

| Statement | AFNI 25.2.09 | AFNI 26.3.05 | Files |
|---|---|---|---|
| "Major portions of this software are copyrighted by the Medical College of Wisconsin, 1994-2000, and are released under the Gnu General Public License, Version 2." | 30 | 0 | – |
| "Major portions of this software are copyrighted by the Medical College of Wisconsin, 1994-2000, and are released under the Creative Commons Attribution License (CC BY 4.0)." | 0 | 30 | `3dTshift.c`, `3ddata.h`, `afni_environ.c`, `afni_environ.h`, `cs_qmed.c`, `csfft.c`, `edt_buildmask.c`, `edt_fullcopy.c`, `mri_cut.c`, `mri_max.c`, `mri_percents.c`, `mri_read.c`, `mri_to_float.c`, `mrilib.h`, `thd_1Dtodset.c`, `thd_coords.c`, `thd_detrend.c`, `thd_dsetto1D.c`, `thd_filestuff.c`, `thd_floatscan.c`, `thd_initdblk.c`, `thd_makemask.c`, `thd_mastery.c`, `thd_notes.c`, `thd_opendset.c`, `thd_shift2.c`, `thd_timeof.c`, `thd_writedset.c`, `thd_zblock.c`, `vecmat.h` |
| "This code is released to the public domain." (the NIfTI library) | 1 | 1 | `nifti/nifti2/nifti2_io.c` |
| no notice: written at the NIH, public domain by `LICENSE.txt` | 24 | 24 | `3dAutomask.c`, `3dUnifize.c`, `AFNI_atlas_spaces.niml`, `Aomp.h`, `afni_logger.c`, `cs_fgets.c`, `debugtrace.c`, `mri_fromstring.c`, `mri_get_cmass.c`, `mri_invert.c`, `niml/niml_elemio.c`, `niml/niml_header.c`, `thd_atlas.c`, `thd_automask.c`, `thd_cliplevel.c`, `thd_dset_nbhd.c`, `thd_dsetatr.c`, `thd_dsetto3D.c`, `thd_mattor.c`, `thd_median.c`, `thd_niftiread.c`, `thd_niftiwrite.c`, `thd_nimlatr.c`, `thd_opentcat.c` |

**No file the replicas derive from is GPL in AFNI 26.3.05.** Two of the no-notice files,
`thd_mattor.c` and `thd_dsetatr.c`, are listed in `PROVENANCE.md` as MCW code without a header
(written by Robert W. Cox before 2001): MCW's grant covers "all of the code under its copyright",
so they are CC BY 4.0 if they are MCW's and public domain if they are not; GPL in neither case.

The replicas were translated from 25.2.09, where these 30 files were GPL-2-or-later; the 26.3.05
files are the same text apart from their licence headers and the two changes the replicas now
follow (AFNI-03 in `thd_niftiread.c`, AFNI-05 in `3dTshift.c`; `docs/findings/afni-tshift.md`,
"AFNI 26.3.05"), and MCW's grant covers the code under its copyright whatever release it came in.

## The other code in `crates-gpl/larmorx-gpl-afni`

| Module | What it is | Its licence | A permissive source that gives the same bits |
|---|---|---|---|
| `glibc_sincosf` | glibc 2.35's `sinf` and `cosf`, both of its x86-64 builds (FMA, SSE2) | LGPL-2.1-or-later (glibc's copies: `s_sinf.c`, `s_cosf.c`, `s_sincosf.h`, and `sysdeps/x86/fpu/sincosf_poly.h`, `s_sincosf_data.c`) | **Yes: Arm's Optimized Routines v19.11, MIT**, which glibc's code is. The FMA build: `crates-nc/larmorx-nc-core/src/glibc_float.rs`, translated from Arm's MIT files, is bit-identical to glibc 2.35's FMA build on all 2^32 inputs of each (`tools/glibc-float-verify`), as `glibc_sincosf` is (`tools/sincosf-verify`). The SSE2 build: Arm's `sinf_poly` and glibc's x86 `sinf_poly` are the same expression tree, operation for operation, with the same eleven constants (glibc's x86 table only pairs them for its vector form), and the reductions are line for line Arm's; without contraction the same IEEE operations give the same bits. Not run exhaustively (compiling and running the downloaded source needs the user's permission: `glibc_permissive/run.sh`) |
| `glibc_floatmath`: `tanhf`, `expm1f` | glibc 2.35's `s_tanhf.c`, `s_expm1f.c` | **fdlibm, Sun's notice only** ("Copyright (C) 1993 by Sun Microsystems, Inc. ... Permission to use, copy, modify, and distribute this software is freely granted, provided that this notice is preserved"); glibc's files carry no LGPL header | already permissive (as `crates-nc` treats fdlibm's `atan2f`, `atanf`, `tanh`, `expm1`) |
| `glibc_floatmath`: `cbrtf` | glibc 2.35's `s_cbrtf.c` | LGPL-2.1-or-later ("Copyright (C) 1997-2022 Free Software Foundation, Inc."), glibc's own code | **None known.** CORE-MATH's correctly rounded `cbrtf` (MIT, already in `larmorx_core::math`) differs from glibc's on 453,492,162 of the 2^32 inputs (`tools/floatmath-verify`); fdlibm's `cbrtf` is another algorithm (not compared). 3dUnifize calls it once, for `-EPI`'s radius (`18.3 / cbrtf(|dx·dy·dz|)`, at least 4.14 voxels): whether a different last bit there can change the ball of neighbours or the printed radius is not measured (rule D) |
| `x87` | the arithmetic of x86-64's `long double` for `nifti2_io.c`'s formulas | larmorx's own code, written for larmorx-gpl (GPL-3.0-or-later) | its author may license it as Apache-2.0 |
| `cli::output` | the write through a temporary file and a rename; what AFNI's writer could not open | larmorx's own code, written for larmorx-gpl | as above |
| `crates-gpl/larmorx-gpl-cli` | the `larmorx-gpl` program (dispatch, tests) | larmorx's own code | as above |

## What a move into the main package would need

If the user decides to move the AFNI replicas into `larmorx` (Apache-2.0):
- **Attribution (CC BY 4.0 §3(a)).** For the 30 MCW files: the creator ("Robert W. Cox et al.,
  Medical College of Wisconsin"), the copyright notice ("Major portions Copyright (C) 1994-2000
  Medical College of Wisconsin"), a notice that refers to CC BY 4.0 and its disclaimer, the URI of
  the licence (https://creativecommons.org/licenses/by/4.0/) and of the source (AFNI's repository
  at the tag), and a statement that the material was modified (translated to Rust, with the
  changes `PROVENANCE.md` lists). In each file's header (replacing "released under the GNU GPL
  version 2 or any later version"), in `PROVENANCE.md` and in the main package's `NOTICE`. The
  NIH files keep their public-domain line and AFNI's request to cite the authors.
- **Licence texts.** CC BY 4.0's legal code in `LICENSES/`. CC BY 4.0 lets an adaptation carry
  another licence ("the Adapter's License", here Apache-2.0), provided the attribution stays and
  that licence does not prevent recipients from complying with CC BY 4.0 (§3(a)(4)).
- **The C library's functions:** `sinf` and `cosf` re-sourced from Arm's MIT files (as
  `crates-nc` did), with Arm's MIT licence text and notice; `tanhf` and `expm1f` with Sun's
  notice; `cbrtf` cannot come along as it is: either measured first (does `-EPI`'s radius ever
  depend on glibc's last bit? if not, `larmorx_core::math::cbrtf` serves) or replaced by a
  permissive routine shown to give glibc's bits, or kept in a small LGPL-licensed piece outside
  the main package.
- **The larmorx-gpl-only code** (`x87`, `cli::output`, the program's main): relicensed by its
  author.
- **Packaging:** the crate moves from `crates-gpl/` to `crates/`, its SPDX headers become
  `Apache-2.0 AND CC-BY-4.0` (MCW-derived files) or `Apache-2.0` (NIH-derived, own code), and the
  replica may become part of the main package (no separate process needed);
  `implementation=` keeps choosing between the replica and the clean-room original.
  `crates-gpl/NOTICE`, `LICENSES/LGPL-2.1.txt` and `tests/notices.rs` change with it;
  `cargo deny`'s allowed licences for `crates/` gain CC-BY-4.0 (and MIT for Arm).
- **CI:** the six `gpl` jobs (`cargo test` of `crates-gpl`, its golden cases) keep running for
  whatever stays in `larmorx-gpl`; the moved golden cases join the main package's. The budget
  does not grow (rule 10).
- **Stays in `larmorx-gpl`:** Connectome Workbench's replicas (GPL-2.0-or-later, still to come)
  and, unless measured or replaced, `cbrtf`. If nothing else is left, `larmorx-gpl` has no AFNI
  content.
- **The clean-room originals** (`crates/larmorx-afni`) stay: they are built for speed and are
  larmorx's default where they agree; with the replicas in the main package they would no longer
  be needed for the licence, only for speed.

The decision is the user's (the brief of 2026-10-29; `docs/licensing.md`, rule 1 of CLAUDE.md).
