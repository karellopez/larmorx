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

## Part 2 (queue item 3.1-22, piece 2): FS-04 to FS-12

## FS-04: unseeded random numbers come from the clock (nondeterminism is never reproduced by default)

`utils/utils.cpp` 106-162 (`randomNumber`): when `idum` is 0 (no `setRandomSeed` yet, or
`setRandomSeed(0L)`, which `mris_flatten`, `mris_rf_label`, `mris_rf_train`,
`mris_make_face_parcellation` and `mris_transmantle_dysplasia_paths` call) and `FREESURFER_SEED` is
unset or empty, it prints `<comm> supposed to be reproducible but seed not set` once (from
`/proc/self/comm`, `/` written `@`, nothing where `/proc` is missing) and sets
`idum = -1L * (long) abs((int) time(NULL))`. Two runs a second apart differ. Default: the seed -1
(33 of FreeSurfer's 64 `setRandomSeed` calls pass `-1L`), with FS-04's notice; `reproduce`: the
clock, as FreeSurfer (and `abs(INT_MIN)` stays `INT_MIN`). The line is printed in both modes.

## FS-05: a bad `FREESURFER_SEED` aborts (rule B)

`utils/utils.cpp` 119-125: `std::stol(seed)` throws `std::invalid_argument` without digits and
`std::out_of_range` beyond a 64-bit `long`; nothing catches it, so libstdc++'s terminate handler
writes `terminate called after throwing an instance of 'std::invalid_argument'` and `  what():  stol`
and aborts (exit status 134; measured with a GCC build of the same call,
`validation/investigations/freesurfer-core/stol_probe.cpp`: `abc` and `99999999999999999999`). `Setting seed for random number
genererator to <value>` (FreeSurfer's spelling) is printed first. `stol` takes leading white space,
a sign and decimal digits and ignores what follows (`" 12abc"` is 12, `"-0x10"` is 0). The input
makes no sense and the original crashes: rule B. Default: `error: FREESURFER_SEED must be an
integer in the range of a 64-bit long, not '<value>'`, exit status 1; `reproduce`: libstdc++'s two
lines, exit status 134.

## FS-06: the fast arctangent reads outside its table (out-of-bounds reads are never reproduced)

`utils/utils.cpp` 1702-1744 (`fastApproxAtan2fWkr`, `fastApproxAtan2f`): the index is
`(int)((100000 * ay) / ax)` evaluated in `float` (`tableLast` is an `int` promoted to `float`). With
a NaN argument, both arguments infinite, or `100000 * ay` beyond `FLT_MAX` (`ay` above about
3.4e33), the quotient is NaN or infinite, `cvttss2si` gives `INT_MIN`, and the lookup reads 8 GB
before the table (a segmentation fault or garbage). Both modes: for an overflowing product, the
quotient of both magnitudes scaled by 2^-64 (exact for numbers that large, so it is the quotient an
unbounded exponent gives, the index FreeSurfer means); for NaN and infinite pairs, `atan2f`'s value;
a notice. The table itself is `(float) atan2(i, 100000)` for i = 0 to 100000 with glibc's `atan2`;
all 100,001 entries equal the correctly rounded `atan2` rounded to `float` (checked with mpmath at
200 bits; SHA-256 `4bb06d9e...addf` in the unit test). Signs are decided by `x == fabsf(x)`, so
`-0.0` counts as positive: `fastApproxAtan2f(0, -0) = 0` where `atan2f` gives pi (kept: the
approximation's own convention).

## FS-07: the version options skip and repeat arguments (rule A)

`utils/version.cpp` 218-247 (`handleVersionOption`): after handling an option at `narg` it copies
`argv[narg+1 ..]` one place back but continues the loop at `narg + 1`, so the argument moved into
`narg` is never examined, while the stale copies at the end (`argc` is not reduced inside the loop)
are examined again. Measured on the official `mri_info` (`blackbox.sh`): `--versionx -version
--all-info` prints the version once and the `--all-info` report twice, with `ProgramArguments:
mri_info -version --all-info --all-info`; `-version` is skipped. `p --version --version x` returns 1
and leaves `p --version x`, so the program's own parser meets `--version`. `--version` and
`-version` are prefix matches (`strncmp` over 9 and 8 bytes: `--versionx` counts), `--all-info` and
`-all-info` exact (11 and 10 bytes, the NUL included). Rule A. Default: each argument examined once,
the reports showing the command line as given; `reproduce`: FreeSurfer's loop (the notice is added
only when the two differ).

## FS-08: `FSENVgetenv` reads memory it never wrote (memory-dependent: filled deterministically)

`utils/fsenv.cpp` 35-97: `int b; sscanf(pc, "%d", &b);` leaves `b` unset when
`FS_DESIRED_BVEC_SPACE` has no number, then compares it with 1 and 2; and when `getcwd` fails it
prints `ERROR: getcwd returned no path` (no newline) and copies the unset buffer. Both modes: `b` = 0
(so the error line and `NULL` follow, as for any wrong number) and the directory empty, with a
notice. `sscanf`'s `%d` keeps the low 32 bits of `strtol`'s value (`4294967298` gives 2, accepted).
Also kept: `user` is set only when `SUBJECTS_DIR` is.

## FS-09 and FS-10: empty lists (out-of-bounds reads)

`utils/utils.cpp` 1386-1480: `median(t, 0)` takes the even branch and `quick_select(t, 0, 0)` returns
`arr[-1]`. 1212-1285: `nunqiue_int_list(l, 0)` returns 1 after reading `idlist[0]` (past the list),
`unqiue_int_list` returns a one-item list of that value, `most_frequent_int_list` that value with a
count of 0. FS-09, both modes: NaN, with a notice. FS-10: default 0 items (`most_frequent`: value 0,
count 0); `reproduce`: one item (FreeSurfer's count is independent of the value read), the value
filled with 0, with a notice. No caller on the default paths is known to pass an empty list; the
cards will say where one can.

## FS-11: fixed buffers (stack overflows are never reproduced)

`utils/utils.cpp` 711-720 (`stricmp`, two `char buf[STR_LEN]`) and 982-992 (`FileNameExtension`,
one) `strcpy` their argument into 1,024 bytes with no check: a string of 1,024 bytes or more
overwrites the stack. Both modes: the whole string, with a notice. `FileNameRemoveExtension` and
`FileNamePath` copy into the caller's buffer, whose size the callers decide (their items check it).

## FS-12: `nint` and `nint2` (the 2026-10-11 rule: saturate and warn)

`utils/utils.cpp` 1521-1522: `(int)(f + 0.5)` (`f - 0.5` below 0; `nint2`: `f + 0.49999999`) with
no range check; beyond the `int` range or NaN `cvttsd2si` gives `INT_MIN`. Default: saturated (NaN
0), with a notice; `reproduce`: `INT_MIN`.

## Part 2: behaviours kept (not defects)

- `setRandomSeed` (77-101) seeds `srand` and `srandom` with the seed's low 32 bits, and re-seeding
  with the current seed first draws once with `seed - 1` (`OpenRan1` reseeds only on a new seed),
  then once with the seed: every `setRandomSeed` consumes one draw, so a program's first
  `randomNumber` is the generator's second number. `nrgcalls` starts at 0 when no seed was set and at
  1 after one.
- `randomNumber` widens `OpenRan1`'s `float` (`drand64` rounded) to `double` before `* range + low`;
  a draw rounded up to `1.0f` can give a value above `hi`, reported with `ErrorPrintf(ERROR_BADPARM,
  "randomNumber(%2.1f, %2.1f) - %2.1f\n", ...)` (FS-01's NUL, two newlines) and returned anyway.
- `OpenRan1` (`utils/numerics.cpp` 1256-1279, item 3.1-24 brought forward) constructs its
  `vnl_random` with seed 0 at its first call and stops a program that calls it from an OpenMP thread
  other than 0.
- `spendTimeCheckingForLogicProblem` reports true at a place's first 32 calls, then at the 64th,
  128th, ... (the threshold grows with the count, so the comment's "elide" starts only after 32).
- `getMemoryUsed` runs `grep -i vmdata /proc/<pid>/status | cut -f 2` through a shell; its failure
  message has no newline. `GetMemUsage` leaves its values unchanged where `/proc` is missing
  (macOS). `PrintRUsage` is `getrusage`'s; the replica reads the same counters from `/proc`
  (Linux's `getrusage` fields that are always 0 stay 0).
- `getBaseTempDir` (1921-1936) keeps the value of the last variable it looked at (`TEMPDIR`) when
  none is set and writable, so an unwritable `TEMPDIR` ends the program even though `/tmp` would do.
- `FileNamePath("/name")` is the empty string; `FileNamePath` and `FileNameAbsolute` use `.` for the
  directory on Linux (the `getcwd` calls are compiled out).
- `xDbg_Printf` flushes its stream even with output off; the stream is then NULL, so it flushes
  every output stream (`fflush(NULL)`). `ROMP_SUPPORT_ENABLED` is off in the official build.
- `chklc` (the licence key) prints the key file's lines on standard output when `DIAG_NO` is above 0;
  `cmp_glib_version` compares the major and minor numbers separately, so with glibc 2.35 a 3-line
  key file is always refused. Not reproduced: no check (decision log 2026-10-19).
- `getAllInfo`'s `BuildTime` is `version.cpp`'s compile time (`Mar 14 2026 12:21:06` in every
  official program); `COMPILER_VERSION`'s first branch tests `__GNU_PATCHLEVEL__`, a macro GCC never
  defines (it would not compile: `ver_string` takes three numbers), so the version has three parts
  (`8.4.0`). `VERcurTimeStamp` writes `20` and the year's last two digits (1970 gives 2070).
- The C library's `localtime` (`currentDateTime`'s `%c`, `VERfileTimeStamp`), measured on 180,240
  cases in 24 zones (`ctime_probe.py`): the replica's agrees on every one; glibc applies a POSIX
  `TZ` rule to the UTC year and, before 1970, uses 1970's change times (northern rules never in
  daylight time, southern ones always).

## FS-261 to FS-263 (track E relay 1's report; the replica's side, track A relay 12)

From track E's handoff (the source and its open probe `validation/investigations/freesurfer-core-probe/`,
whose prebuilt `fs_core_probe` this relay ran as a black box for the unit tests' numbers):

- **FS-261** (rule A). `utils/machine.cpp` 240-317: `ByteSwap2`, `ByteSwap4`, `ByteSwap8` loop
  `for (n = 0; n < nitems; n += k)` while moving the pointer one item per pass, so they swap
  `ceil(nitems / k)` items; `ByteSwapBuf` (215-234) passes the item count. Callers: `utils/imautils.cpp`
  77 (`imaLoadVal`, one item: correct) and 623 (`imaReadPixelData`, `npixels` shorts: only the first
  half of the pixels swapped). The replica (`machine::byte_swap_buf`, `byte_swap2/4/8`): every item by
  default, FreeSurfer's first `ceil(n / k)` with `reproduce`, noted where they differ (more than one
  item); items beyond the buffer are never touched (FreeSurfer would write past it). Measured with
  `fs_core_probe batch` (2026-11-13): `byteswapbuf 2 4 0102030405060708` gives `0201040305060708`,
  `4 3` of 12 bytes swaps the first item only, `8 2` the first, `2 5` the first three; `k = 3` prints
  `ERROR: ByteSwapBuf: nBytesPerItem = 3 not supported` and returns 1; counts 0 and -1 swap nothing:
  the unit test `machine::tests::fs261_a_buffer_swap` holds these lines.
- **FS-262 is FS-176.** `swapLong64` (37-53) stores the reversed 64-bit integer in a `double` before
  returning it; the replica has had it since relay 10 (`machine::swap_long64`, with FS-176's switch and
  notice, the readers' and writers' defect: `freadLong`, `fwriteLong`, `znzreadLong`, `znzwriteLong`
  are its only callers). Track E's FS-262 describes the same behaviour from the function's side (its
  branch predated relay 10's merge): one ID should be retired (recommendation: keep FS-176, which the
  register's handoff of relay 10 and the replica's notices already use; FS-262's text could become
  FS-176's). The replica keeps noting FS-176.
- **FS-263** (rule A). `include/log.h` 45-54: `fs::fatal`'s destructor calls `detail::errorExit`
  (`utils/log.cpp` 99-106), which with exceptions on throws the code; a destructor is `noexcept` in
  C++11, so `std::terminate` runs: libstdc++'s verbose handler prints `terminate called after throwing
  an instance of 'int'` on standard error and aborts (status 134). `ErrorExit` ends through
  `fs::fatal` too, so both paths abort. Code 0 neither throws nor exits. The replica
  (`log::error_exit_code`): by default `Fatal::Throw(code)` reaches the program's handler; `reproduce`
  writes the terminate line and ends with 134 (`Fatal::Exit(134)`, as FS-03's 139). Measured with
  `fs_core_probe` (2026-11-13): `fatal 7 f7 throw` gives `error: f7`, the terminate line, status 134;
  `errorexit 2 3 bad throw` gives `error: No such file or directory`, `error: bad` with FS-01's NUL,
  the terminate line, 134; `fatal 0 f7 throw` returns (`error: f7`, status 0). Programs that switch
  exceptions on: `mri_ca_train` (its `catch(...)` writes the done file) and the Python bindings; none on
  recon-all's or fMRIPrep's default paths.

## The register's texts before the open-files rewrite (2026-11-13)

The orchestrator's rule of 2026-11-13 ("open files carry behaviour only", the foundations' brief):
`crates/larmorx-core/src/defects.rs` is open to FreeSurfer's clean-room implementers, so its texts
describe only what a caller sees (inputs, outputs, messages, exit status). The texts it held
before, with the mechanisms they named (pointers, which function passes what, the libraries
underneath, the casts), are kept here verbatim; the sections above give their evidence.

- **FS-01**: the C-style error messages (ErrorPrintf, ErrorExit, ErrorReturn and what calls them) end with a NUL byte before their newline, on standard error and in the error log, so recon-all.log and other logs that capture them read as binary files to grep and text tools
- **FS-02**: DiagInit converts the PROFILE variable to an integer with no range check: a value beyond the int range (or NaN) is an undefined conversion, which gives -2147483648 on x86-64
- **FS-03**: a failed internal assertion (assertFailed) ends the program by writing to address -1 on purpose, a segmentation fault, after its message; with FREESURFER_HANG_ON_ASSERT set it first loops for ever
- **FS-04**: a program that draws random numbers before setting a seed (or after setting seed 0), with FREESURFER_SEED unset, seeds the generator from the clock: no two runs agree
- **FS-05**: a FREESURFER_SEED that is not an integer, or is beyond a 64-bit integer, ends the program with an uncaught C++ exception (abort, exit status 134) at its first random number
- **FS-06**: the fast arctangent's table lookup (fastApproxAtan2f) reads outside its table when an argument is NaN, both are infinite, or the larger magnitude is so large that 100000 times the smaller overflows a float
- **FS-07**: the --version and --all-info handling skips the argument that follows each option it removes and examines the last arguments again: a second version option can be missed, another handled twice
- **FS-08**: FSENVgetenv reads memory it never wrote: the b-vector space when FS_DESIRED_BVEC_SPACE holds no number, the working directory when it cannot be read
- **FS-09**: the median (and quick_select) of an empty list reads the value before the list
- **FS-10**: the unique-integer helpers (count, list and most frequent value of a list) given an empty list read the integer past its end and report one item
- **FS-11**: the case-insensitive string comparison (stricmp) and the file-name extension helper copy their argument into a 1,024-byte buffer on the stack with no check: a longer string overflows it
- **FS-12**: nint and nint2 (FreeSurfer's rounding to an integer) convert values beyond the int range, and NaN, with no check, an undefined conversion that gives -2147483648 on x86-64
