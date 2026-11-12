# FreeSurfer 8.2.0's core services (`fs.core`): findings

**Closed to FreeSurfer clean-room implementers** (source-derived: it names FreeSurfer's files and
lines; the behaviour is in `specs/freesurfer-core.md`). Written by the replica author of queue item
3.1-22 (FreeSurfer's foundations, track A). Defect IDs FS-01 to FS-03, decided by rules A to D
(CLAUDE.md, D13); the register's entries are in the orchestrator's handoff.

## FS-01: a NUL byte at the end of every C-style error message (rule A, corrected by default)

`utils/log.cpp` 110-120 (the macro `vargsToString`, used by `ErrorPrintf` at 127 and `ErrorExit` at 140) formats `ErrorPrintf`'s and `ErrorExit`'s message into a buffer of
`vsnprintf`'s length plus one and builds `std::string(zc.data(), zc.size())` from all of it, so the
string ends with the buffer's terminating NUL; `fs::error()` (`include/log.h`) then writes `error: `,
the string and `\n`, and `writeToErrorLog` writes the same string to the error log. Measured on the
official `mri_info` (`validation/investigations/freesurfer-core/blackbox.sh`): `... could not open
file\0\n`. Every library call that reports through these functions (`ErrorReturn`, `ErrorPrintf`,
`ErrorExit`: most of `utils/`) does it. Why it matters: `recon-all.log` and the scripts' logs capture
standard error, and a NUL byte makes `grep` and similar tools treat the log as a binary file
(`Binary file recon-all.log matches`). The input makes sense and the output is wrong: rule A.
Default: the message without the NUL; `reproduce`: with it.

## FS-02: an undefined conversion of `PROFILE` (the 2026-10-11 rule: saturate and warn)

`utils/diag.cpp` 147-150 stores `atof(getenv("PROFILE"))` in `int Gprofile` with no range check. A
value beyond the `int` range, or NaN, is undefined behaviour in C; GCC's `cvttsd2si` gives
`INT_MIN`, measured: `PROFILE=1e10` and `PROFILE=nan` print `turning profiling diagnostics on
(-2147483648)...`. Default: saturated (NaN 0), with FS-02's notice; `reproduce`: `INT_MIN`.

## FS-03: a failed assertion crashes on purpose (crashes are not reproduced by default)

`utils/diag.cpp` 82-100 (`assertFailed`, `include/base.h`'s `cheapAssert`): after its messages it
loops for ever while `FREESURFER_HANG_ON_ASSERT` is set, then writes to address -1, a segmentation
fault (a core dump for a debugger). Default: exit status 1 after the messages; `reproduce`: exit
status 139 (what a shell reports for the signal; a Rust program cannot raise it without unsafe code).
The endless loop is never reproduced.

## Behaviours kept (not defects)

- `fs::fatal(0)` with exceptions on (`throwExceptions(true)`) neither throws nor exits: the program
  continues after the message (`utils/log.cpp` 99-106). Kept (rule C: callers may rely on a code 0
  never stopping them).
- `FSinit` (`utils/fsinit.cpp` 25-31) sets OpenMP to one thread when `OMP_NUM_THREADS` is unset: the
  official programs run single-threaded by default, so the results to reproduce, where a program's
  arithmetic depends on its thread count (`docs/freesurfer/variation.md`), are those of one thread
  unless the variable says otherwise. LARMORX's own thread count stays independent of it.
- `setLocale` (`utils/fsinit.cpp` 10-22) only changes `LC_NUMERIC` to `en_US` or `C.UTF-8`, whose
  number formats are the C locale's for FreeSurfer's `printf` calls (no grouping flag is used); the
  warning it prints when no such locale is installed describes the machine, not the data, and is not
  reproduced.
- `swapDouble` (`utils/machine.cpp` 19-35) exchanges bytes 0-7, 2-5, 4-3 and 6-1: a full reversal
  after all. `swapLong64` (not reached by any default-path program) returns its result through a
  `double` and so loses the bits beyond 53: to register if a program ever reaches it.
- The official programs' `__FILE__` strings start with the build directory
  `/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0` (`strings mri_info`); the
  messages that print `__FILE__` (`assertFailed`'s ` break` line, the library's assertions) carry it.
