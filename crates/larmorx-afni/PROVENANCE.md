# Provenance: larmorx-afni

**Clean-room implementation. No AFNI code was used.**

AFNI's `3dTshift` and the library routines it calls are copyrighted by the Medical College
of Wisconsin and licensed under the GPL-2, so larmorx re-implements them (CLAUDE.md rule 1).
This crate was written by implementers who never opened AFNI's source code, the replica's
source (`crates-gpl/`) or notes derived from them (`docs/findings/afni-tshift.md`, the
restricted sections of `docs/findings/upstream-defects.md`). Its only inputs were:

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
