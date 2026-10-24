# Provenance: larmorx-afni

**Clean-room implementation. No AFNI code was used.**

AFNI's `3dTshift` and the library routines it calls are copyrighted by the Medical College
of Wisconsin and licensed under the GPL-2, so larmorx re-implements them (CLAUDE.md rule 1);
so are the library routines `3dAutomask` and `3dUnifize` depend on. This crate was written by
implementers who never opened AFNI's source code, the replica's source (`crates-gpl/`) or notes
derived from them (`docs/findings/afni-tshift.md`, `afni-automask.md`, `afni-unifize.md`, the
restricted sections of `docs/findings/upstream-defects.md`, `docs/parts/`, the table of AFNI
routines in `docs/licensing.md`). The inputs of 3dTshift's original were:

| Input | What it gave |
|---|---|
| [`specs/3dTshift.md`](../../specs/3dTshift.md) | the behaviour spec: inputs, the maths of each step, AFNI's NIfTI rules, the command line; since 2026-10-12 also §6.1 to §6.5 (the geometry AFNI writes, the slice fields of a copy, the output names, AFNI-03's notice) and §7.1 (AFNI's global options and environment variables) |
| `3dTshift -help` (AFNI 25.2.09) | AFNI's published documentation of the options and slice patterns |
| Black-box runs of the AFNI 25.2.09 binary (`oracles/afni-25.2.09/bin/3dTshift`) | every rule marked *(observed)* in the spec, and the ones found while implementing (listed in `docs/api/afni-tshift.md`); for D13, float32 data with a slope and an intercept, metre and micron units, and the search for AFNI-08's trigger (spec §6); for part 2b (2026-10-12), AFNI's messages of a run, word for word and in order, its 1D parsing, the separators of AFNI-18's list, the template-space codes, the plumb boundary of a 0.8 mm column (all added to the spec, marked observed by the clean-room implementer); for AFNI-26's switch (2026-10-12), 32 scenarios of failed and refused writes (missing and read-only directories, read-only files, directories of the output's name, `AFNI_DECONFLICT`, `-overwrite`, message prefix and colours, copies, `.nii.gz`, NIfTI-2, one-row datasets; spec §7.1, "A failed write") |
| Black-box runs of the replica program (`larmorx-gpl afni 3dTshift`, built in `crates-gpl/target/release/`; its source not opened) | the wording of larmorx's own lines that the replica prints (the defect notices of AFNI-11, -12, -18, -19, -20, -22, -23 and -26; the warnings about `-D` options ending in `(larmorx)`), so that both implementations say the same |
| The defect register's summary table and "D13 gaps" list (`docs/findings/upstream-defects.md`, which describe the AFNI entries by observable behaviour only; the restricted AFNI section was not opened) | which of 3dTshift's behaviours are defects, and the corrections larmorx makes by default (`larmorx_core::defects`, `upstream_defects`): AFNI-01, -02, -03, -05, -07, -08, -11, -12, -16, -18, -19, -20, -22, -23, -26 |
| Textbook mathematics | the FFT (`fft.rs`: Stockham autosort, Van Loan 1992 §1.7), least squares, Lagrange interpolation |
| Public descriptions of IEEE 754 and of the x87 80-bit extended format; GCC's `long double` on x86-64 Linux, run as a black box | `x87`: an operation rounded to a 64-bit significand, then to 53 bits, in integer arithmetic; checked against 6 million operations computed with GCC 13.3 |
| larmorx's own Apache code: `larmorx_io::nifti::itk::write` (its single-precision port of the NIfTI reference library's `nifti_mat44_to_quatern` and polar decomposition, public domain, from ITK 5.4.5) and `larmorx_io::nifti::itk` (`nifti_quatern_to_mat44`) | the method of the quaternion and of the qform's matrix, which the spec describes (§6.1) and `geometry.rs` carries out in double precision, as the spec says AFNI does |
| C's `printf` (`%g`, `%f`), as documented | `cfmt`, AFNI's numbers in messages; the same logic as larmorx's `larmorx_ants::cli::cstd::format_g` |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `tshift` (detrend, shift, clip, retrend; skipped slices; `-ignore`; parallel driver) | AFNI 25.2.09 `3dTshift` | spec §4, black-box runs |
| `tshift::series` (Fourier, Lagrange and weighted-sinc shifts) | AFNI 25.2.09 `3dTshift` | spec §5, black-box runs (heptic summation order, weighted-sinc edges) |
| `fft` | none (a generic FFT; AFNI's own FFT is not reproduced) | textbook |
| `timing` (named patterns, header slice timing, default origin, the slice fields of a copy) | AFNI 25.2.09 `3dTshift`, `-tpattern` | spec §3, §6.2, AFNI's help table, black-box runs |
| `oned` (`-tpattern @file`, `1D:` text, `AFNI_1D_ZERO_TEXT`, `AFNI_USE_FGETS`, AFNI's messages) | AFNI's 1D text format, as `3dTshift` reads it | spec §3.1, §7.1, black-box runs |
| `dataset` (datum and brick factor on reading, `AFNI_NIFTI_SCALE`, conversion messages, storing back, the output header) | AFNI 25.2.09's NIfTI reading and writing | spec §6, §7.1, black-box runs |
| `geometry` (the matrix AFNI reads and writes, the plumb test, the cardinal matrix, the quaternion, the xform codes, `AFNI_NIFTI_PRIORITY`, the obliquity report) | AFNI 25.2.09's NIfTI geometry | spec §6.1, §7.1, black-box runs, larmorx-io's port of the NIfTI reference library |
| `x87` | x86-64's x87 extended precision (AFNI-24's arithmetic) | public format descriptions; checked against GCC |
| `input` (`~/`, AFNI-18's list, the `.gz` fallback, the input's name in messages) | AFNI 25.2.09's opening of a dataset | spec §7.1, black-box runs |
| `output` (the output-name rules, `AFNI_DECONFLICT`'s renaming, what AFNI's writer cannot open, the write through a temporary file) | AFNI 25.2.09's output names and failed writes (AFNI-26); larmorx's own write (spec §7.1, "the cross-platform design") | spec §6.3, §7.1, black-box runs |
| `settings` (the table of AFNI's variables, the global options, `-D` read as AFNI reads it) | AFNI 25.2.09's global options and environment | spec §7.1, black-box runs |
| `msg`, `cfmt` (labels, prefix, colours; numbers) | AFNI 25.2.09's messages | spec §7.1, black-box runs |
| `cli::tshift` (options, errors, "copy of input" cases, the order of messages) | AFNI 25.2.09 `3dTshift` command line | spec §7, §7.1, `-help`, black-box runs |

Transcendental functions come from `larmorx_core::math` (correctly rounded CORE-MATH ports),
except the arc cosine of the obliquity report (the `libm` crate, pure Rust; `larmorx_core::math`
has no `acos` yet; the angle is rounded to float32 and printed with 6 decimals).
The parity record is [docs/validation/afni-tshift.md](../../docs/validation/afni-tshift.md).

## 3dAutomask and 3dUnifize (2026-10-23)

Written by one implementer who had never opened AFNI's source, the replica's crates or the
restricted findings, from these inputs only:

| Input | What it gave |
|---|---|
| [`specs/3dAutomask.md`](../../specs/3dAutomask.md), [`specs/3dUnifize.md`](../../specs/3dUnifize.md) (commit 30c50b0; their rule-B refusals in detail from commit 60c963d) | the behaviour specs: the average, the clip level and the gradual clip level, the mask's steps, the program's steps, the local white-matter intensity, the half-size path, the T2 inversion, `-GM`, the edge clipping, `-EPI`, the command lines, the messages, the defects and the user's decisions of 2026-10-23 (AFNI-41 and AFNI-48 refused by default; AFNI-42 and AFNI-44 kept) |
| [`specs/3dTshift.md`](../../specs/3dTshift.md) §6 and §7.1 | AFNI's dataset reading and writing, global options and environment variables (already carried out in `dataset`, `geometry`, `input`, `output`, `settings`, `msg`) |
| `3dAutomask -help`, `3dUnifize -help` (AFNI 25.2.09) | AFNI's documentation of the options and of the method (the progress letters' meanings, the defaults) |
| Black-box runs of the AFNI 25.2.09 binaries (`oracles/afni-25.2.09/bin/3dAutomask`, `3dUnifize`) | the messages of every path, word for word and in order (the progress line, where each automask's lines fall, `-quiet`, `-verb`, `-EPI`'s lines and dots, the outputs' lines, the conflict, refused-name and failed-write paths), the 2D outputs' `dim[0]`, the unit brick factor of float32 inputs, the count line of `-eclip`; every rule found so is in the specs, marked observed by the clean-room implementer |
| Black-box runs of the replica program (`larmorx-gpl afni 3dAutomask`, `3dUnifize`, built in `crates-gpl/target/release/`, and on 2026-10-24 its corrected build of the decisions part, `.claude/worktrees/agent-af2255d42fc4f41d1/crates-gpl/target/release/larmorx-gpl`; its source not opened) | the wording of larmorx's own lines, so that both implementations say the same (the notices of AFNI-01, -11, -12, -18, -23, -26, -43, -45, -47; the refusals of AFNI-40, -41, -46, -48, with their notices and where they are printed) |
| The defect register's summary table (`docs/findings/upstream-defects.md`, rows AFNI-40 to AFNI-49, which describe them by observable behaviour; the restricted section not opened) | which behaviours are defects, and the corrections larmorx makes by default |
| The parity modules of the two tools (`validation/.../parity/afni_automask.py`, `afni_unifize.py`, `afni_replica.py`, `afni_phantoms.py`), run as black boxes; edited only to select the original and record its results | the cases and phantoms of the comparison |
| CORE-MATH (MIT), `src/binary32/tanh/tanhf.c` and `src/binary32/cbrt/cbrtf.c` at commit 040ee482a8ca | `larmorx_core::math::{tanhf, cbrtf}`, correctly rounded, for the squash and `-EPI`'s radius (checked against the C on all 2^32 floats) |
| Textbook methods | run-based connected-component labelling with union-find, breadth-first searches, order statistics (selection), counts per rank with block counts for a sliding window |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `automask::clip` (the clip level, the gradual clip level) | AFNI 25.2.09 `3dAutomask` | spec §4, §4.1; the histogram's integer sums proved equal to the spec's double sums below 2^53 |
| `automask::cluster` (the largest face-connected cluster, the clusters of a plane) | AFNI 25.2.09 `3dAutomask` | spec §5.2, §7; run-based labelling |
| `automask::morph` (peeling and restoring, filling holes, erosion, dilation, the exterior clip, the depth) | AFNI 25.2.09 `3dAutomask` | spec §5.3 to §5.6, §6.1, §6.3; the exterior clip and the depth by breadth-first searches with the same results |
| `automask` (the average, the mask of a volume and of a plane, AFNI-01, -43, the outputs) | AFNI 25.2.09 `3dAutomask` | spec §3, §5, §7, §9; black-box runs |
| `automask::program` (the depth's messages, `-dilate`, `-erode`, the count, the empty planes, `-SI`) | AFNI 25.2.09 `3dAutomask` | spec §6; black-box runs |
| `unifize::wmi` (the local white-matter intensity) | AFNI 25.2.09 `3dUnifize` | spec §4.3; a sliding window of counts per rank (larmorx's own method; the same bits as the spec's sorted balls) |
| `unifize::duplo` (the half-size image and back, AFNI-47) | AFNI 25.2.09 `3dUnifize` | spec §4.2, §4.4 |
| `unifize` (the T2 inversion, AFNI-45, the scaling and its squash, `-GM`, the edge clipping, `-EPI`, AFNI-01, AFNI-46, the progress letters) | AFNI 25.2.09 `3dUnifize` | spec §5 to §7; black-box runs |
| `cli::automask`, `cli::unifize`, `cli::common` (options, errors, messages, outputs, AFNI-40, -41, -48) | AFNI 25.2.09 `3dAutomask`, `3dUnifize` command lines | spec §8 of each, `-help`, black-box runs |
| `dataset::read_as` (a unit `scl_slope` kept as a brick factor), `dataset::written_shape` (a plane written 2D) | AFNI 25.2.09's reading and writing in these programs | spec 3dAutomask §2, black-box runs |

The parity records are [docs/validation/afni-automask.md](../../docs/validation/afni-automask.md)
and [docs/validation/afni-unifize.md](../../docs/validation/afni-unifize.md). The black-box tools
the implementer used (running and comparing AFNI, the original and the replica program, the
phantoms, the CORE-MATH checksums, the informal timings) are in
[validation/investigations/afni-masks-original/](../../validation/investigations/afni-masks-original/).

One exposure, recorded for the audit: after both programs were complete (2026-10-23), a text
search over `docs/licensing.md` printed a few rows of its table of
AFNI routine names (names only, no code), a table the implementer had not opened before. No
line of this crate was written or changed from it; the implementer did not open the table.
