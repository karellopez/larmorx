# Provenance: larmorx-freesurfer-core

**Replica: translated from FreeSurfer's source.** This crate holds `fs.core` (queue item 3.1-22) and `fs.maths` (3.1-23, 3.1-24) of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): FreeSurfer's messages, diagnostics, program name, environment, version stamps, the ROMP switch, timers, paths, signals and the licence check's messages (`fs.core`); its matrices (float and long double), the numerics wrappers over VNL and Cephes, `fs_powell`, `fs_lbfgs`, histograms, statistics, PDFs and random fields (`fs.maths`). It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

| Rust module | FreeSurfer file | Functions and data |
|---|---|---|
| `globals` | `utils/Progname.cpp`, `utils/diag.cpp` 47-68, `utils/log.cpp` 55-58, `utils/hips.cpp` 4-5 | `Progname`; `Gprofile`, `Gvx` ... `Gvz`, `Gsx` ... `Gsz`, `Gdiag`, `Gdiag_no`, `Gdiag_no2`, `Gx` ... `Gz2`, `IMAGE_SIZE`; `errorlog`, `exceptions`, `Gerror`; `hips_cplxtor`, `hips_rtocplx`; the `static` memories of `DiagHeartbeat` and `DiagShowPctDone`; the C library's `errno` |
| `log` | `include/log.h`, `utils/log.cpp` | the error codes, `fs::fatal`, `fs::error`, `fs::warning`, `fs::debug`, `setErrorLog` and `throwExceptions` (as `Globals` fields), `writeToErrorLog`, `errorExit`, `ErrorPrintf`, `ErrorExit` (`ErrorReturn` is `ErrorPrintf` and a return); glibc's `strerror` texts (from glibc's documented messages); the `term` colours are always empty (`termAllowsColor` returns false) |
| `diag` | `include/diag.h`, `utils/diag.cpp`, `utils/fsinit.cpp` | the `DIAG_*` bits, `DIAG_VERBOSE_ON`, `assertFailed`, `DiagInit`, `DiagPrintf`, `DiagBreak`, `DiagHeartbeat`, `DiagShowPctDone`, `check_finite`; `FSinit` and `setLocale` as far as they are seen (below) |
| `machine` | `utils/machine.cpp` | `swapDouble`, `swapLong32`, `swapFloat`, `swapShort`, `swapInt` |
| `cmdargs` | `utils/cmdargs.cpp` | `CMDargNErr`, `CMDisFlag`, `CMDsingleDash`, `CMDnthIsArg` |
| `base` | `include/base.h` | `squaref`, `squared` |
| `cstd` | none (larmorx's own) | the C library's `atoi`, `atof`, `sscanf`'s `%lx`, `printf`'s `%W.Pf`, x86-64's double-to-int conversion, written from the C standard and glibc's documented behaviour |
| `run` | none (larmorx's own) | what `larmorx-freesurfer` gives a program (queue item 3.1-01) |

Third-party code in these files (CLAUDE.md rule 3): none; each carries MGH's notice
(`Copyright © 2021 The General Hospital Corporation (Boston, MA) "MGH"`) or none (`Progname.cpp`,
`hips.cpp`, `fsinit.cpp`: FreeSurfer's own, Part B as far as MGH holds the rights,
`docs/freesurfer/third-party.md` §2).

Still to translate (queue item 3.1-22, part 2; NOTES of FreeSurfer's foundations, track A):
`utils/utils.cpp` (the random numbers, the file-name and string helpers, `median`/`quick_select`,
the memory reports, the temporary files), `utils/fsenv.cpp`, `utils/version.cpp`,
`utils/timer.cpp` and `include/timer.h`, `utils/romp_support.cpp`, `utils/chklc.cpp`,
`utils/xDebug.cpp`, `include/fio.h`.

## Changes from the originals

- Process-wide globals are fields of `Globals`, one value per program run.
- Where FreeSurfer exits, the functions return `log::Fatal`, which the program turns into its exit
  status (the code's low 8 bits).
- Variadic messages are formatted by the callers; the functions take the formatted text.
- The defects FS-01 (a NUL byte after the C-style error messages), FS-02 (`PROFILE`'s undefined
  conversion) and FS-03 (`assertFailed`'s deliberate segmentation fault) are corrected by default
  and reproduced with `reproduce` (`docs/findings/freesurfer-core.md`).
- `FSinit`'s locale is not set (numbers are written as in the C locale, which FreeSurfer's chosen
  locales keep) and its warning about a missing locale is not printed; its one-thread default is
  reported (`diag::fs_init`) for the programs whose arithmetic depends on their thread count.
- `assertFailed`'s endless loop under `FREESURFER_HANG_ON_ASSERT` is not reproduced.
