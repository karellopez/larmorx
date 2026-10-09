# Provenance: larmorx-afni

**Clean-room implementation. No AFNI code was used.**

AFNI's `3dTshift` and the library routines it calls are copyrighted by the Medical College
of Wisconsin and licensed under the GPL-2, so larmorx re-implements them (CLAUDE.md rule 1).
This crate was written by an implementer who never opened AFNI's source code or notes
derived from it. Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/3dTshift.md`](../../specs/3dTshift.md) | the behaviour spec: inputs, the maths of each step, AFNI's NIfTI rules, the command line |
| `3dTshift -help` (AFNI 25.2.09) | AFNI's published documentation of the options and slice patterns |
| Black-box runs of the AFNI 25.2.09 binary (`oracles/afni-25.2.09/bin/3dTshift`) | every rule marked *(observed)* in the spec, and the ones found while implementing (listed in `docs/api/afni-tshift.md`) |
| Textbook mathematics | the FFT (`fft.rs`: Stockham autosort, Van Loan 1992 §1.7), least squares, Lagrange interpolation |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `tshift` (detrend, shift, clip, retrend; skipped slices; `-ignore`; parallel driver) | AFNI 25.2.09 `3dTshift` | spec §4, black-box runs |
| `tshift::series` (Fourier, Lagrange and weighted-sinc shifts) | AFNI 25.2.09 `3dTshift` | spec §5, black-box runs (heptic summation order, weighted-sinc edges) |
| `fft` | none (a generic FFT; AFNI's own FFT is not reproduced) | textbook |
| `timing` (named patterns, header slice timing, default origin) | AFNI 25.2.09 `3dTshift`, `-tpattern` | spec §3, AFNI's help table, black-box runs |
| `oned` (`-tpattern @file`, `1D:` text) | AFNI's 1D text format, as `3dTshift` reads it | black-box runs |
| `dataset` (datum and brick factor on reading, storing back, output header and geometry) | AFNI 25.2.09's NIfTI reading and writing | spec §6, black-box runs |
| `cli::tshift` (options, errors, "copy of input" cases) | AFNI 25.2.09 `3dTshift` command line | spec §7, `-help`, black-box runs |

Transcendental functions come from `larmorx_core::math` (correctly rounded CORE-MATH ports).
The parity record is [docs/validation/afni-tshift.md](../../docs/validation/afni-tshift.md).
