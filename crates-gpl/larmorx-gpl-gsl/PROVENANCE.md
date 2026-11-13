# Provenance: larmorx-gpl-gsl

**Replica: translated from FreeSurfer's transcription of GSL.** This crate is a statement-by-statement
Rust translation of the "GSL library replacement for Freesurfer" in FreeSurfer 8.2.0's
`utils/numerics.cpp` (Silvester Czanner, 2006): GSL 1.8's RANLUX389 generator and the random
distributions FreeSurfer draws from, written to give FreeSurfer's official build's numbers bit for bit
(queue item 3.1-53, `gpl.gsl`; `docs/freesurfer/third-party.md` TP-03 and the placement P-08). It is
part of the `larmorx-gpl` package, alone there among FreeSurfer's pieces: GPL code cannot share a
program with FreeSurfer's licence, so no FreeSurfer replica links it; `larmorx-freesurfer`'s
`mri_coreg` and `mris_expand` draw from larmorx's clean room, `larmorx_core::ranlux` (queue item
3.1-05, written from `specs/ranlux.md` alone), and this crate is that clean room's bit-exact reference
(the user's decision of 2026-11-12, "No waiting for permissions"). No program and no wheel's binary
includes it; it is a library of the GPL workspace, with its probe (`examples/gsl_probe.rs`) and tests.

## The upstreams

- **FreeSurfer 8.2.0** (GitHub's `v8.2.0` archive, SHA-256
  `bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`, extracted in
  `<workspace>/tags/freesurfer-8.2.0`): `utils/numerics.cpp` (SHA-256
  `a40443aad70cd37f1ccd6c75a7891d2bf8760a16dafe594ef03c50c0a0039009`; "Copyright © 2021 The General
  Hospital Corporation (Boston, MA)", the FreeSurfer Software License Agreement) and `include/numerics.h`
  (SHA-256 `6c1754b2b3072ee88a86811ceacd937c3846fdc31cce80974bff66e0b042b43b`). The text that defines
  the bits is FreeSurfer's (rule 8: the source is the specification).
- **GSL 1.8** (the GNU Scientific Library, 2006; "Copyright (C) 1996, 1997, 1998, 1999, 2000 James
  Theiler, Brian Gough"; GNU GPL version 2 or any later version), whose `rng/ranlux.c`
  (`increment_state`, `ranlux_get`, `ranlux_set_lux`) and `randist/gamma.c`, `gauss.c`,
  `exponential.c`, `chisq.c`, `fdist.c`, `tdist.c`, `flat.c` FreeSurfer's functions transcribe
  expression by expression. Read beside FreeSurfer's file: GSL 1.8's `randist/gamma.c` and `tdist.c`
  from the `release-1-8` tag of `git.savannah.gnu.org/cgit/gsl.git` (SHA-256
  `f031873222756fdd2fdfe0c15c6d33fe46b7ea6913dbc3021783fd19e4063bf9`,
  `1bbb7abaf021e316c8e6f2db8f9d65bce84cb9a620d0f823e63d086f51c1f4c4`) and the current `rng/ranlux.c`
  and `randist/{gauss,exponential,chisq,fdist,flat}.c` of the `ampl/gsl` mirror (`ranlux.c` SHA-256
  `60dcf5fe8ea5e9eb5815c063da7f8a29a0ce1258a0310b578016ba0d99b8f3a1`); copies outside the repository.
- **The official build's code** (GCC 8.4.0, `-O3 -fno-builtin`, x86-64 SSE2, no FMA;
  `docs/freesurfer/variation.md`): the unit oracle's `numerics.cpp.o`
  (`<workspace>/oracles/freesurfer-8.2.0-unit`, the official programs' code, queue item 3.1-02),
  disassembled, decides what C leaves open: the order in which `sc_gamma_int(r, na) +
  sc_gamma_frac(r, a - na)` draws (left to right), the operand order of each instruction (which NaN
  passes on: `sse.rs`), and the conversion `(unsigned int)floor(a)` (`cvttsd2si` to 64 bits, the low
  32 kept).

## Files translated

| larmorx | FreeSurfer 8.2.0 | GSL 1.8 |
|---|---|---|
| `src/rng.rs`: `ScStatus` | `include/numerics.h` 109-118 (`sc_status_t`) | `ranlux_state_t` |
| `src/rng.rs`: constants | `utils/numerics.cpp` 1870-1872 (`SC_MASK_LO`, `SC_MASK_HI`, `two_over_24`) | `mask_lo`, `mask_hi`, `two24` |
| `src/rng.rs`: `sc_inc_status` | 1928-1963 | `increment_state` |
| `src/rng.rs`: `ScRng::get` | 1965-1982 (`sc_rng_get`) | `ranlux_get` |
| `src/rng.rs`: `ScRng::set` | 1984-2024 (`sc_rng_set`, luxury 389) | `ranlux_set_lux` |
| `src/rng.rs`: `ScRng::alloc`, `new` | 2026-2041 (`sc_rng_alloc`; `sc_rng_free` 2043-2050 is the drop), `include/numerics.h` 120-141 (`sc_rng_type`, `sc_rng`, `intern_rng_type`) | `gsl_rng_alloc` |
| `src/rng.rs`: `uni`, `uni_pos` | 2052-2069 (`sc_uni`, `sc_uni_pos`) | `gsl_rng_uniform`, `gsl_rng_uniform_pos` |
| `src/randist.rs`: `ran_flat` | 2073-2080 | `gsl_ran_flat` |
| `src/randist.rs`: `ran_gaussian`, `ugauss` | 2082-2103 | `gsl_ran_gaussian` (the polar method), `gsl_ran_ugaussian` |
| `src/randist.rs`: `gamma_int`, `gamma_large`, `gamma_frac`, `ran_gamma` | 2105-2179 | `gsl_ran_gamma_int`, `gamma_large`, `gamma_frac`, `gsl_ran_gamma` |
| `src/randist.rs`: `ran_fdist`, `ran_chisq` | 2181-2198 | `gsl_ran_fdist`, `gsl_ran_chisq` |
| `src/randist.rs`: `ran_tdist`, `ran_exponential` | 2200-2230 | `gsl_ran_tdist`, `gsl_ran_exponential` |
| `src/sse.rs` | (larmorx's own: SSE's NaNs and `cvttsd2si`, on every platform) | |
| `src/tests.rs`, `examples/gsl_probe.rs` | (larmorx's own: the tests and the probe's commands) | |

Not here: `sc_err_msg` (its one message, allocation failure, does not arise in Rust), the Cephes
wrappers that follow in the same block (`sc_ran_binomial_pdf`, `sc_cdf_*`, 2232-2352: FreeSurfer's
own calls of Cephes, no GSL code; `fs.maths`'s), and `sc_linalg_cholesky_decomp` (VNL's).

## The functions the translation calls

- `log`, `exp`: glibc 2.35's (x86-64), as the official programs get them on the reference machine:
  `larmorx_core::math::log_glibc`, `exp_glibc` (Apache-2.0, from Arm's Optimized Routines).
- `tan`: **correctly rounded** (`larmorx_core::math::tan`, CORE-MATH's), not glibc's: the user's
  decision of 2026-11-04 (no glibc data in larmorx; GLIBC-07). glibc 2.35's `tan` differs from it on
  41,708 of the 2²⁴ arguments `sc_gamma_large` passes (`specs/ranlux.md` §6), so a gamma draw of
  shape 12 or more may differ from FreeSurfer's in its last bits where it meets one of them.
  `ScRng::set_tan` replaces it, for validation only.
- `sqrt`, `floor`: IEEE's (exact or correctly rounded on every platform), with SSE's NaNs.

## larmorx's own additions (D13)

- **FS-100** (`sc_ran_gamma`'s conversion of shapes outside [0, 2³²)): by default a finite shape of
  2³² or more uses its whole integer part (rule A), NaN, ±∞ and negative shapes are errors (rule B);
  `reproduce` gives FreeSurfer's value, except for −∞ and negative shapes, where FreeSurfer's loop
  does not end (the same errors). **FS-101** (the seeds whose seeded state is all zero): by default
  that state starts with a carry of 1 (James's 1994 rule, rule A); `reproduce` keeps the zero
  stream, and the draws that would never return are errors. Both as the clean room decides them
  (`specs/ranlux.md` §8), with the clean room's notice and error texts, so the two implementations
  print the same lines.

## Verification

- `cargo test -p larmorx-gpl-gsl`: `specs/ranlux.md`'s reference numbers (the probe's, from
  FreeSurfer's compiled functions): §2.5's 11 seeds (first values, the second group, the SHA-256 of
  100,000 values), §10.2's 12 distribution hashes of 20,000 draws and first draws, §4.2's `R = 1`
  factor, §7's NaNs, FS-100's and FS-101's values and errors; on Linux x86-64 with glibc, the two
  large-shape hashes with the C library's `tan`; and the **clean room's 28 golden checksums**, the
  same cases text for text (every platform checks that replica and clean room agree there).
- `validation/investigations/freesurfer-gsl-replica-check/` (`compare_replica.py`): the replica
  against FreeSurfer's functions (`fs_rng_probe`) and the official `mri_volsynth`, and against the
  clean room, byte for byte; the results are in its `result-*.json` and README.
