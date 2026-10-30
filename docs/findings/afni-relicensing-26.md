# AFNI 26.3.05's new licence: an inventory of the AFNI replicas (no moves)

**Status:** an inventory for the user's decision (version-update part, 2026-10-29). **Decided the
same day: the AFNI replicas move into the main package** (decision log 2026-10-29; next-steps item
3c). **Done the same day** (below, "The move (2026-10-29)"): the replicas are
`crates/larmorx-afni-replica`, part of `larmorx` (Apache-2.0 with CC BY 4.0's attribution), and
run inside it; `crates-gpl/` keeps only verification tools. Unrestricted: licences, file names and function names only, nothing of the
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

## The move (2026-10-29)

Next-steps item 3c, done by a replica author's part (`validation/investigations/afni-move/`).

### The C library's maths, first

- **`sinf`, `cosf`: re-sourced, proved equal.** `crates/larmorx-afni-replica/src/sincosf.rs` is a
  fresh translation of Arm's Optimized Routines v19.11 (MIT: `math/sincosf.h`, `sinf.c`, `cosf.c`,
  `sincosf_data.c`), with both of glibc's x86-64 builds (FMA, every fused `a + b·c`; SSE2, none).
  `crates-gpl/tools/sincosf-verify` compared it on all 2^32 inputs of each function and build: 0
  differ from the earlier glibc translation (`crates-gpl/tools/glibc-reference`, itself checked
  against glibc), 0 from this machine's glibc 2.35 FMA build, and 0 from its SSE2 build (forced
  with `GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA`; the two builds differ from each other on 12
  `sinf` and 22 `cosf` inputs, so the comparison sees which one runs). No C was compiled.
- **`tanhf`, `expm1f`:** fdlibm's code under Sun's notice (`fdlibm.rs`, SPDX `Apache-2.0 AND
  SunPro`); 0 of the 2^32 `tanhf` inputs differ from glibc (`crates-gpl/tools/floatmath-verify`).
- **`cbrtf`: measured (rule D), and its last bit matters.** `crates-gpl/tools/cbrtf-measure` ran
  3dUnifize `-EPI`'s radius (`18.3 / cbrtf(|dx·dy·dz|)`, at least 4.14) over every non-negative
  float32 voxel volume (2^31), with glibc's `cbrtf` (the earlier translation, whose hash of all
  2^32 results is glibc's) and CORE-MATH's correctly rounded one (MIT, `larmorx_core::math`):

  | | All volumes | From 0.001 mm³ (voxel edges of 0.1 mm and up) | [1, 100) mm³ |
  |---|---|---|---|
  | the cube roots differ (always by one unit in the last place) | 226,746,081 | | |
  | the radius differs | 103,849,703 (82,523,478 above 2364 voxels, a ball no machine holds) | 12,711,708 | 4,943,322 |
  | the printed radius (`-EPI changes -Urad to %.1f voxels`) differs | 65,145,681 | 552 | 43 |
  | the ball of neighbours differs (radius up to 2364 voxels) | 1,449,010 | 8,794 | 80 |

  The ball was compared exactly (while every float sum of three squares it tests is an integer,
  the ball is fixed by the radius's integer part and the integer part of its float square; two
  radii give the same ball unless an integer between those is a sum of three squares, Legendre's
  theorem), and cross-checked by building both balls on 1,303 sampled cases (0 disagree). Example:
  a volume of 11.97 mm³ (`0x413f83e6`, edges of 2.2875 mm): glibc's radius is exactly 8 (2,109
  offsets), CORE-MATH's 7.9999995 (2,103). **No permissive routine with glibc's bits is known**
  (glibc's `cbrtf` is its own LGPL code; fdlibm's and musl's are other algorithms).
- **What the replica does now (until the user decides):** it uses CORE-MATH's `cbrtf`
  (`unifize::epi_radius`), so it is AFNI's bit for bit wherever the last bit cannot matter, and it
  warns (a line ending in `(larmorx)`, which the parity harness sets aside as larmorx's own) on
  every volume where a cube root one unit away would change the ball or the printed radius:
  917,292,437 volumes in all, 206,626 from 0.001 mm³, 2,953 in [1, 100) mm³; every volume where
  glibc's last bit changes something is among them (0 missed). **For the user** (the brief's "do
  not decide that yourself"), the ways to keep AFNI's exact result without LGPL code, the
  recommended first:
  1. **A clean-room `cbrtf` with glibc's bits:** a spec of the method (a short polynomial start,
     one rational correction step in double precision, the scaling by the cube roots of 2), written
     by a spec writer, implemented by an agent that has not read glibc, Apache-2.0, and proved
     equal to glibc's on all 2^32 inputs against `cbrtf-measure`'s reference (a small part, about
     an hour); then the warning goes.
  2. A table of the volumes where glibc's radius differs and changes something, from 0.001 mm³
     (9,346 entries, about 75 kB): exact on every realistic voxel size, not below.
  3. Keep CORE-MATH's `cbrtf` and the warning (the replica is then not exact on the flagged
     volumes).

### The move

- **The crate** `crates-gpl/larmorx-gpl-afni` moved (history kept) to
  `crates/larmorx-afni-replica`; its files' SPDX headers are `Apache-2.0 AND CC-BY-4.0` (every
  file translating MCW code, with CC BY 4.0's attribution in its header: creator, copyright,
  licence and its URI, the disclaimer, the source and its tag, the modification), `Apache-2.0`
  (NIH-derived files and larmorx's own, with AFNI's request to cite), `Apache-2.0 AND MIT`
  (`sincosf.rs`) and `Apache-2.0 AND SunPro` (`fdlibm.rs`); `scripts/check_spdx.py` enforces them
  (CC BY 4.0 only in that crate, and in every file there whose header names MCW's code). The
  attribution is also in `PROVENANCE.md` and the repository's `NOTICE`; CC BY 4.0's legal code and
  Arm's MIT licence are in `LICENSES/`, which the wheel carries. larmorx's own code written for
  `larmorx-gpl` (`x87`, `cnum`, `cli::output`, the dispatch, the tests) is relicensed to Apache-2.0
  by its author.
- **In-process.** The `larmorx` command line runs the replica in its own process
  (`larmorx_cli::replica::Location::Builtin`); the Python wrappers call it through
  `larmorx._core.replica_run_builtin` (without the GIL) with the arguments, the environment table
  and the working directory a separate program would have had, so the files, messages and exit
  status are the same. `implementation="auto"` keeps its rule (the replica when available): it
  now always runs the replica. The `larmorx-gpl` program, its wheel and the `larmorx[exact]` extra
  are gone (the repository's history keeps them).
- **`larmorx-gpl`** holds no package: `crates-gpl/` is the GPL workspace of the verification tools
  that need glibc's LGPL code (`tools/glibc-reference`, `sincosf-verify`, `floatmath-verify`,
  `cbrtf-measure`), reserved for Connectome Workbench's replicas. CI's six `gpl` jobs keep running
  `crates-nc`; their `crates-gpl` steps (4.0 min per run, measured on the three newest runs) are
  gone, and the replicas' tests and golden cases now run in the `test` jobs' `cargo test`.
- **Not changed:** the code's arithmetic. The golden cases moved with their checksums unchanged
  (`crates/larmorx-cli/tests/afni_replica_golden.rs`, 40 cases, 90 outputs); the parity records are
  the evidence (`validation/investigations/afni-move/README.md`).
