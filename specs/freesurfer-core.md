# FreeSurfer 8.2.0's core services: behaviour spec (`fs.core`)

Written by the replica author of queue item 3.1-22 as its spec writer (`docs/porting-guide.md` §1):
behaviour, conventions, messages and the validation plan, from FreeSurfer 8.2.0's source and the
official programs' output, never code or routines. Made ready for the clean room by the spec
finisher (FreeSurfer's foundations, track E, relay 1, 2026-11-13): the black-box validation plan
(§20) with its open probe, a read-through as a clean-room reader, the gaps filled from the source as
behaviour (§0, §2, §3, §6, §9, §10, §11, §19), three new defects (FS-261 to FS-263). What black-box
runs confirmed is marked **(observed)**, with the probe command or the comparison group behind it.

**Whoever implements it must not open:** FreeSurfer's source (`<workspace>/tags/freesurfer-8.2.0`,
any other copy, the installed package's scripts, Python and libraries), the replica crates
(`crates-freesurfer/`, among them `larmorx-freesurfer-core`), the documents written from FreeSurfer's
source (`docs/freesurfer/`, `docs/findings/freesurfer-*.md`), the replica's probes and checks
(`validation/probes/fs-*`, `validation/probes/x87-extended/`, `validation/investigations/freesurfer-core/`,
whose scripts run the replica), the unit oracle's build tree and tools
(`<workspace>/oracles/freesurfer-8.2.0-unit/` except running `probes/fs_core_probe`;
`validation/investigations/freesurfer-unit-oracle/`), the replay tool's and the unit oracle's Python
(`validation/src/larmorx_validation/freesurfer/`, `freesurfer_unit.py`: run, never read),
`docs/parts/` and the scratch folders of FreeSurfer's parts. **It may read** this spec, the standards
and papers of §21, the main package's open code and documentation (for example `larmorx_core::vnl_random`,
`larmorx_core::glibc_random`, `larmorx_core::x87`, `larmorx_core::math`, `larmorx_core::cscan`,
`larmorx_core::cfmt`, `larmorx_core::defects`), the open probe folder
`validation/investigations/freesurfer-core-probe/` (calls only: its README says how to run it), and
run as black boxes the prebuilt probe `<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_core_probe`,
the official FreeSurfer 8.2.0 programs and the C library.

## 0. Conventions and the functions covered

- **The official build** is x86-64 Linux, GCC 8.4.0 at `-O3`: SSE2 arithmetic for `float` and
  `double`, no fused multiply-add, no reassociation; every expression below is evaluated operation by
  operation, each result rounded to its type (to nearest, ties to even). "In float" means every
  operation of the expression is a `float` operation; a float combined with a double is widened
  first. A NaN made from numbers (`0/0`, `∞ − ∞`, the square root of a negative number) is SSE's
  default NaN, sign bit set (`0xffc00000` in float, `0xfff8000000000000` in double); a NaN operand
  passes on quieted with its sign and payload; of two NaN operands the left one of the expression as
  written. Conversions of a float or double to an integer truncate toward zero; beyond the target's
  range, or NaN, x86-64 gives the "integer indefinite" value (`-2147483648` for 32 bits).
- **The C library** is glibc 2.35 in the C locale, as the probe and the official programs run on the
  reference machine. Strings are bytes up to the first NUL. White space is the C locale's (space,
  `\t`, `\n`, `\v`, `\f`, `\r`); a byte of 0x80 or more is never white space, a letter, or a digit;
  case changes touch ASCII letters only. `printf` formats are C11 7.21.6.1's (glibc rounds a decimal
  conversion correctly: `larmorx_core::cfmt`).
- **The process state** (§1) belongs to one program run: an implementation gives each run its own.

FreeSurfer's names, as the probe calls them (§20), and where they are specified:

| Section | FreeSurfer's functions |
|---|---|
| §1 | `Progname`, the diagnostic globals (`Gdiag`, `Gdiag_no`, `Gx` ... `Gvz`, `IMAGE_SIZE`, `Gprofile`), `Gerror` |
| §2, §3 | `ErrorPrintf`, `ErrorExit`, `ErrorReturn` (an `ErrorPrintf` and a return), `fs::error`, `fs::warning`, `fs::debug`, `fs::fatal`, `setErrorLog`, `throwExceptions`, `assertFailed` (`cheapAssert`), `CMDargNErr`, `CMDusageExit` |
| §4, §5 | `FSinit`, `DiagInit`, `DiagPrintf`, `DiagBreak`, `DiagHeartbeat`, `DiagShowPctDone`, `check_finite` |
| §6 | `swapShort`, `swapUShort`, `swapInt`, `swapLong32`, `swapLong64`, `swapFloat`, `swapDouble`, `Arch486`, `ByteSwapBuf` (`ByteSwap2`, `ByteSwap4`, `ByteSwap8`) |
| §7 | `CMDisFlag`, `CMDsingleDash`, `CMDnthIsArg`, `CMDstringMatch` |
| §9 | `setRandomSeed`, `getRandomSeed`, `getRandomCalls`, `randomNumber`, `OpenRan1`, `compute_permutation` |
| §10 | `fgetl`, `StrUpper`, `StrLower`, `str_toupper`, `FileNameOnly`, `FileName`, `FileNamePath`, `FileNameRemoveExtension`, `FileNameExtension`, `FileNameFromWildcard`, `FileExists`, `stricmp`, `ItemsInString`, `CountItemsInString`, `GetNthItemFromString`, `deblank`, `strcpyalloc`, `FindFirstMatch`, `getEnvironVar`, `stringEndsWith` |
| §11 | `devIsinf`, `devIsnan`, `devFinite`, `deltaAngle`, `nint`, `nint2`, `sum2stddev`, `compare_ints`, `compare_floats`, `nunqiue_int_list`, `unqiue_int_list`, `most_frequent_int_list`, `kth_smallest`, `quick_select`, `median`, `DListStats`, `squaref`, `squared` |
| §12 | `fastApproxAtan2f` |
| §13 | `getVersion`, `getBuildStamp`, `handleVersionOption`, `getAllInfo`, `argv2cmdline`, `VERuser`, `VERfileTimeStamp`, `VERcurTimeStamp` |
| §14 | `currentDateTime`, `Timer`, `TIMER_INTERVAL_BEGIN`/`END` |
| §15 | `FSENVgetenv`, `FSENVfree`, `FSENVgetSUBJECTS_DIR`, `FSENVsetSUBJECTS_DIR` |
| §16 | `getMemoryUsed`, `GetMemUsage`, `GetVmPeak`, `PrintRUsage`, `WriteRUsage` |
| §17 | `spendTimeCheckingForLogicProblem`, `copeWithLogicProblem` (`copeWithLogicProblem2`) |
| §18 | `directoryExists`, `directoryIsWritable`, `getBaseTempDir`, `makeTempFile`, `makeTempDir` |
| §19 | the thread limit (`romp_omp_get_thread_num`), the parallel loops' levels and the reproducible sums (`ROMP_if_parallel1`, `ROMP_Distributor_begin`/`_end`, `ROMP_PARTIALSUM`), the debugging stack (`xDbg_PushStack`, `xDbg_PopStack`, `xDbg_SetStackDesc`, `xDbg_SetCurrentNote`, `xDbg_GetCurrentFunction`, `xDbg_GetCurrentNote`, `xDbg_Printf`, `DebugEnterFunction`, `DebugExitFunction`), the HIPS functions, the licence check (`chklc`, reference only) |

**Not covered** (no program on the default paths reaches them; a later item specifies one if a
program does): `IntSqrt`, `StrReplace`, `StrSkipNumber`, `StrRemoveSpaces`, `FileType`, `FileNumber`,
`FileNumberOfEntries`, `FileFullName`, `FileTmpName`, `FileRename`, `FileNameAbsolute`,
`AppendString`, `normAngle`, `calcDeltaPhi`, `latan2`, `QuadEqual`, `angleDistance`,
`fComplementCode`, `mad`, `FindMatches`, `GetVmSize`, `PrintMemUsage`, `printMemoryUsed`,
`SetProgressCallback`, `exec_progress_callback` (no callback is ever installed), `FSENVprintenv`,
`DiagFprintf`, `DiagShowImage`, `DiagDrawBox`, `DiagCreateWindow`, `DiagCloseWindow`, `xDbg_Init` and
the other `xDbg_*` functions not named above, ROMP's statistics (off in the official build), the
`TSV` table's reading and writing (with `fs.io`).

## 1. What every FreeSurfer program shares

Every FreeSurfer program keeps a small process-wide state: its name for messages (`"no name"` until
it sets one), a set of diagnostic bits (64 bits, 0 at start), a diagnostic index (-1), the
coordinates of a diagnostic voxel, a second point and a vertex (each -1), an image size (512), a
profiling level (0), the code of its last error (0), the C library's `errno`, an optional error-log
file, and a switch that turns fatal errors into exceptions (off). An implementation must give each
program run its own copy of these, starting from those values, so that programs run one after the
other in one process (as `recon-all`'s port will) start alike.

## 2. Messages

All on **standard error**, each ending with one newline:

| Kind | Bytes written |
|---|---|
| error | `error: ` + message + `\n` (the message also appended, with a newline, to the error log when one is set) |
| warning | `warning: ` + message + `\n` (not in the log) |
| debug | `debug: ` + message + `\n`, only when the diagnostic bit `0x10000000` (verbose) is set |
| fatal | as an error, then the program ends (§3) |

No colour codes are ever written. Messages are formatted by the caller with the C library's `printf`
rules; the caller's own trailing newline, when its format has one, is kept, so such a message ends
with two newlines. Standard error is unbuffered: each message reaches it as it is written.

**The error log.** Every error line's message (the `errno` text of a C-style error included, below)
is appended to the log's file followed by `\n`; the file is opened for appending at each message. A
log that cannot be opened is skipped without a message, **but the failed open leaves `errno` set**
(2 for a missing folder, **(observed)**: `errorprintf 0 -4 x78 log x2f6e6f6e6578697374656e742f6469722f6c6f67`
prints `Gerror=-4 errno=2`), so the next C-style error starts with `error: No such file or directory`.

**The C-style error calls** (FreeSurfer's older API: "print an error" and "print an error and exit",
and the "print and return a value" form built on the first), used by most library code:

1. record the error code as the last error;
2. if `errno` is not 0, write an error whose message is the C library's text for it (step 2 never
   changes `errno`, and nothing here resets it: a second C-style error with the same `errno` prints
   its text again, **(observed)** `errorprintf 13 -4 x61 twice`);
3. write the caller's message as an error;
4. for "print and exit": end the program with the error code (§3).

The texts are glibc 2.35's `strerror` texts in the C locale: 0 `Success`, 1 `Operation not
permitted`, 2 `No such file or directory`, 13 `Permission denied`, 21 `Is a directory`, ...; a code
glibc has no text for (134 to 140 among the probe's cases, any negative code): `Unknown error <n>`
(**(observed)**: `fs_core_probe single strerror <n>` prints each; the SHA-256 of the 134 lines
`<n>\t<text>\n` for n = 0 to 133 is `f986d0f7191c5b16f925e7bbdffddb87b71196a3058f577eecfb4fc54ecece94`).

**FS-01.** FreeSurfer's step 3 writes one **NUL byte** (0x00) after the message, before the newline,
on standard error and in the error log (official `mri_info` on a missing file:
`error: mghRead(/nonexistent.mgz, -1): could not open file` + NUL + `\n`, exit status 1,
**(observed)** `official.sh`). A message with its own trailing newline gives `error: a\n` + NUL + `\n`
(**(observed)** `errorprintf 0 -4 x610a`). A file holding such lines (`recon-all.log` captures them)
reads as binary to `grep` and other text tools. **Default:** no NUL byte; **reproduce:** the NUL
byte, exactly as above. Only the caller's message of these C-style calls carries it; their `errno`
line and the plain error, warning, debug and fatal messages never do.

Library functions specified elsewhere print through these calls; the environment record (§15), for
example, shows the colour table reader's two lines (`error: No such file or directory`, then
`error: CTABreadASCII(<file>): could not open file` + NUL + `\n`) before its own.

## 3. Ending a program

- **Fatal error with code c:** the program exits with status `c mod 256` (the code's low 8 bits:
  -1 gives 255, -5 gives 251, 256 and -256 give 0, **(observed)**: the `messages` group). A fatal error
  with code 0 therefore ends the program with status 0 (the C-style "print and exit" with code 0
  included). FreeSurfer's error codes: 0 none, -1 no file, -2 no memory, -3 unsupported, -4 bad
  parameter, -5 bad file, -6 size, -7 bad loop, -8 out of bounds. The plain fatal message
  (`fs::fatal(c)`, default code 1) carries no NUL byte; the C-style one does (FS-01).
- **With exceptions on** (a program can switch them on; none on recon-all's or fMRIPrep's default
  paths does): **a fatal error with code 0 does nothing** after its message: the program goes on (a
  quirk kept: callers rely on code 0 never stopping them in this mode; **(observed)** `errorexit 0 0
  x7a65726f throw` prints `returned`).
- **FS-263.** With exceptions on, a fatal error with a code other than 0 is meant to reach the
  program's handler with that code; in the official build it ends the program instead: after the
  message, `terminate called after throwing an instance of 'int'` + `\n` on standard error and an
  abort (status 134), the handler never running (**(observed)** `errorexit 0 -5 x626164 throw` and
  `fatal 7 x6637 throw`). Rule A (the program's handler, for example one writing a "done" file, is the
  intended behaviour). **Default:** the code reaches the caller that switched exceptions on, as an
  error value, after the message; **reproduce:** the terminate line and status 134. Noted either way.
- **"Flag needs arguments"** (the older programs' option parser): `ERROR: <flag> flag needs <n>
  argument` + `\n` when n is 1, `... <n> arguments` + `\n` otherwise (0 and -1 included,
  **(observed)**), on standard error, exit status 255. **The usage exit** prints nothing and ends with
  status 1 (**(observed)** `usageexit`).
- **A failed internal assertion** writes `ASSERTION FAILED: <file>:<line> <test>` + `\n` on
  standard output and on standard error, then ` break <BUILD_ROOT>/utils/diag.cpp:85` + `\n` on
  standard output (both streams flushed), where BUILD_ROOT is the official build's directory,
  `/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0` (the `<file>` of an
  assertion in the library starts with it too). **FS-03:** FreeSurfer then dies of a segmentation
  fault on purpose (status 139 as a shell reports it, **(observed)** `assert x662e637070 12 x61203c2062`);
  with the variable `FREESURFER_HANG_ON_ASSERT` set it first loops for ever printing
  `FREESURFER_HANG_ON_ASSERT` + `\n` on both streams, a few seconds apart. **Default:** exit status 1
  after the messages; **reproduce:** exit status 139 (a status, as a shell reports the signal). The
  endless loop is never reproduced.

## 4. Diagnostics set from the environment

At start most programs read these variables (when set), after setting the numeric locale and the
thread default (below):

| Variable | Sets | Read as |
|---|---|---|
| `DIAG_NO` | the diagnostic index | `atoi` |
| `IMAGE_SIZE` | the image size | `atoi` |
| `DIAGX`, `DIAGY`, `DIAGZ` | the diagnostic voxel | `atoi` |
| `DIAGVX`, `DIAGVY`, `DIAGVZ` | the diagnostic vertex | `atoi` |
| `PROFILE` | the profiling level, then prints `turning profiling diagnostics on (<level>)...\n` on standard output (whatever the value, an empty one included: `(0)`) | `atof`, converted to a 32-bit int |
| `diag`, else `DIAG` | or-ed into the diagnostic bits | the literal `0x` (lower case, first byte of the value), then `scanf`'s `%lx`; anything else leaves the bits alone |
| `DIAG_VERBOSE` (any value, even empty) | the verbose bit `0x10000000` | presence |

Then, when the verbose bit is set (by either variable), `verbose diagnostics enabled...\n` on
standard output (after the profiling line). The start-up returns the diagnostic bits. `atoi` is
glibc's: white space, a sign, decimal digits, a 64-bit `long` clamped to its range, then its low
32 bits (`99999999999999999999` gives -1, `-2147483649` gives 2147483647). `atof` is `strtod`'s
reading (decimal and hexadecimal numbers, `inf`, `nan`; `0x10` is 16; no number is 0). `%lx` skips
white space, takes a sign and an optional `0x`/`0X`, reads hexadecimal digits (beyond 64 bits: all
ones), a minus negates modulo 2^64. **(observed)**, the `init` group: `DIAG=0x0x14000` sets 0x14000,
`0X4000`, ` 0x4000`, `4000` and `-0x1` set nothing, `0x-1` and `0xfffffffffffffffffff` set all 64
bits (and so the verbose line), `diag=0x4000` wins over `DIAG=0x1`; `PROFILE=7.9` gives 7, `-2.5`
gives -2, `abc` gives 0, `2147483647.9` gives 2147483647.

**FS-02.** The `PROFILE` level is the value truncated toward zero; for a value beyond the 32-bit
range, or NaN, FreeSurfer's level is -2147483648 (**(observed)**: `PROFILE=1e10`, `-1e10`, `nan`, `inf`
and `2147483648` all print `turning profiling diagnostics on (-2147483648)...`; the official `mri_info`
alike, `official.sh`). **Default:** the value saturated to the 32-bit range (NaN gives 0);
**reproduce:** -2147483648.

The same start-up sets the numeric locale (to `en_US` or `C.UTF-8`, a warning `Please install the
en_US locale for numeric consistency` when the machine has none of them, not when `FS_DISABLE_LANG`
is set): numbers are always written as in the C locale, and **the warning is never written** by
LARMORX (it only reports a missing locale on the machine). It also makes FreeSurfer's OpenMP loops run
on **one thread when `OMP_NUM_THREADS` is unset**: where a program's results depend on its thread
count, the result to reproduce is that thread count's.

## 5. Other diagnostics

- **Diagnostic print** (a message and a set of bits): nothing, returning -1, when the bits are not 0
  and share none with the diagnostic bits; otherwise the message on standard output, returning its
  length in bytes (**(observed)** `diagprintf`).
- **Heartbeat** (a fraction done, a float): remembers the last fraction printed (start -10). A smaller
  fraction than the remembered one resets it to -20. When the fraction exceeds the remembered one by
  more than 0.10 (float subtraction, compared in double), it is remembered and, with the heartbeat bit
  `0x4000` set, `<100*fraction>% finished\n` is written on standard error (`%2.1f`, the product in
  float). **Percent done** (a fraction and a count n) does the same with its own memory and the step
  `1/n` in float, compared in float (n = 0 gives an infinite step: it never prints). **(observed)**: the fractions 0, 0.1, 0.2, 0.3, 0.1, 1, 0.95, 0.96, 1.2, -0.5, 0, 0.2000001
  (as floats) print `0.0`, `10.0`, `20.0`, `30.0`, `10.0`, `100.0`, `95.0`, `120.0`, `-50.0`, `0.0`,
  `20.0` `% finished`; with n = 4 percent done prints `0.0`, `30.0`, `10.0`, `100.0`, `95.0`, `120.0`,
  `-50.0`, `0.0`.
- **Finite check** (a place name and a double): 1 when finite; else the C-style error (§2) with code
  -4 and the message `<place> not finite!` + `\n` (so `error: <place> not finite!\n` + NUL + `\n`), and
  0.
- **The break point** does nothing.

## 6. Byte order

- **One value:** swapping a 16-bit (signed or unsigned), 32-bit (integer or float) or 64-bit float
  value reverses its bytes; bit patterns are kept (NaN payloads and signalling NaNs included,
  **(observed)**: the `swaps` group). "Is this a little-endian machine" answers 1.
- **FS-176. A 64-bit integer:** FreeSurfer reverses the bytes, then rounds the reversed value (a
  signed 64-bit integer) to the nearest double (ties to even) and converts it back, truncating (a
  value rounded up to 2^63 gives -2^63). The result differs from the exact reversal only when the
  reversed value needs more than 53 significant bits, which never happens for values below 2^40
  (FreeSurfer's MGH and MGZ files hold their tags' lengths as such byte-swapped 64-bit integers, all
  far below). **(observed)**:
  `swaplong64 72057594037927937` (0x0100000000000001) gives 72057594037927936;
  `swaplong64 81985529216486895` (0x0123456789abcdef) gives -1167088121787636992, the exact reversal
  being -1167088121787636991. Rule A. **Default:** the exact reversal; **reproduce:** FreeSurfer's
  rounding. Noted when they differ. (fs.io's 64-bit readers and writers share this rounding and
  this defect, `specs/freesurfer-io.md` §2-4; FS-262, the ID this swap had first, is retired: a
  duplicate of FS-176.)
- **A buffer of n items of k bytes each** (k = 1, 2, 4 or 8): k = 1 leaves it; another k writes
  `ERROR: ByteSwapBuf: nBytesPerItem = <k> not supported` + `\n` on standard output and answers 1,
  the buffer unchanged; otherwise it answers 0.
- **FS-261.** For k = 2, 4 and 8 FreeSurfer reverses the bytes of the **first ⌈n/k⌉ items only**
  (it counts the items in bytes), leaving the others as they were. **(observed)**: `byteswapbuf 2 4
  x0102030405060708` gives `x0201040305060708`, `byteswapbuf 4 2 x0102030405060708` gives
  `x0403020105060708`. FreeSurfer's reading of Siemens IMA files (off the default paths) swaps its
  pixel data this way (from the source; no IMA file was at hand to observe it). Rule A. **Default:** every item reversed; **reproduce:** the first ⌈n/k⌉. Noted
  when they differ (n ≥ 2).

## 7. Command-line helpers of the older programs

A *flag* is an argument of at least two bytes starting with `--`; a *single dash* is an argument of
at least two bytes starting with one `-` not followed by another; "the n-th argument is a value"
(n from 0, of a given count) when n is below the count and that argument is not a flag (a negative
n is not defined). Two strings match when both are given and equal; when exactly one is absent they
do not (both absent: not defined).

## 8. Validation plan, part 1

See §20: one black-box plan for every section.

## 9. Random numbers

Every FreeSurfer program draws from **one** generator: VXL's `vnl_random` as ITK 4.13.2 carries it
(Marsaglia and Zaman's subtract-with-borrow seeded by a linear congruential generator; the main
package's `larmorx_core::vnl_random`, open), through a draw function that keeps the last seed it was
given and **reseeds** the generator (with the seed's 64 bits read as an unsigned number) only when
called with a different seed (it starts at seed 0, its generator made with seed 0), and returns
`drand64(0, 1)` **rounded to float**. The draw function stops a program that calls it from an
OpenMP thread other than the main one: `<BUILD_ROOT>/utils/numerics.cpp:1261 OpenRan1 called from
non-0 thread but this is not conducive to reproducible behavior, nor is this code thread-safe` + `\n`
on standard error, exit status 1.

The program's random state is the current seed (0 at start) and a draw count (0 at start).

- **Seeding** with a seed s (a 64-bit signed integer): the C library's `rand` and `random` are seeded
  with s's low 32 bits (`larmorx_core::glibc_random`); when s equals the current seed, one draw is
  made with s - 1 first (so the sequence restarts); the current seed becomes s; **one draw is made
  with s and discarded**; the draw count becomes 1. A program's first number after seeding is
  therefore the generator's second.
- **A number between low and hi** (swapped when low > hi): the float draw, widened to double, times
  `hi - low`, plus `low` (double arithmetic); the draw count goes up by one. A result outside
  [low, hi] (`< low` or `> hi`: a NaN is never outside; a draw rounded up to 1.0f can be) is reported
  as a C-style error (§2) with code -4 and the message `randomNumber(<low>, <hi>) - <value>` + `\n`,
  each as `%2.1f` of the value rounded to float, and returned anyway.
- **No seed yet** (current seed 0; seeding with 0 also leaves it 0), checked at **every** draw: when
  the variable `FREESURFER_SEED` is set and not empty, `Setting seed for random number genererator to
  <value>` (sic, the raw value) + `\n` on standard output (flushed), then seeding with it read as
  C++'s `std::stol` (white space, a sign, decimal digits; the rest ignored: ` 12abc` is 12, `-0x10` is
  0). A value read as 0 (`0`, `00`, `-0`) therefore prints the line and reseeds at every draw: every
  draw returns the same number and the count stays at 2 (**(observed)**: `FREESURFER_SEED=0`, two
  draws `3fe4815460000000`, then `seed=0 calls=2`). Otherwise (unset or empty) `<name> supposed to be
  reproducible but seed not set` + `\n` on standard error, once per run, where `<name>` is the
  process's name as Linux keeps it (the program file's name without its folder, cut to 15 bytes; `/`
  written as `@`; empty where the system has no `/proc`), and **FS-04**: FreeSurfer makes the current
  seed `-|t|`, t the clock's seconds truncated to 32 bits (so no two runs agree); the draw then
  reseeds the generator (a new seed) and returns its **first** number (no draw is discarded on this
  path); the count goes from 0 to 1. **Default:** the current seed -1, the same way (the first number
  of a generator seeded with -1); **reproduce:** the clock. The line is written in both modes.
- **FS-05.** A `FREESURFER_SEED` without digits (`abc`, ` `, `+`), or beyond the 64-bit range, ends
  FreeSurfer with `terminate called after throwing an instance of 'std::invalid_argument'` (beyond the
  range: `'std::out_of_range'`) + `\n` + `  what():  stol` + `\n` on standard error and an abort (exit
  status 134), after the `Setting seed` line (**(observed)** the `random` group). **Default:** `error:
  FREESURFER_SEED must be an integer in the range of a 64-bit long, not '<value>'` + `\n`, exit
  status 1; **reproduce:** FreeSurfer's lines and 134.
- **The current seed and the count** can be read back.
- **A permutation of 0 .. n-1:** start from the identity; for each place i in order, swap it with the
  place given by a number between 0 and `n - 0.0001` truncated toward zero (n = 0: an empty list).

Reference numbers **(observed)**, `random seed=<s> draw=0000000000000000,3ff0000000000000 ...`, the
first three numbers between 0 and 1 after seeding: seed -1 `3fe779dc20000000 3fbda8dde0000000
3fe9ac30c0000000`; seed 1 `3fe5ed80e0000000 3fcd12cd40000000 3fc0602f00000000`; seed 1234
`3fd1617760000000 3fc1ffe660000000 3fd966ac20000000`; seed 53 `3fe501dd80000000 3fb98e1cc0000000
3fb00f4520000000`. Seeding with -1 twice, then one draw, gives `3fe779dc20000000` and the count 2.
Seed 53, then a number between 5 and -3 (swapped): `400203bb00000000`.

## 10. Strings and file names

| Helper | Result |
|---|---|
| read a line (with an n-byte buffer, n ≥ 2) | the next piece the C library's `fgets` gives (at most n - 1 bytes, through `\n`) that is not empty, blank or starting with `#` after its leading white space; returned without that white space, cut at its first `#`, without its final `\n` (other trailing white space, `\r` included, kept); nothing at the end of the file. A line longer than n - 1 bytes comes in pieces, each judged alone |
| upper case, lower case (in place) | ASCII letters changed, every other byte kept |
| file name only (into the caller's buffer) | the part after the last `/` (all of it without one), cut at its last `#`, then at its last `@` |
| file name (in place) | the part after the last `/`, after skipping one leading `@`, cut at its last `#`, then at its last `@`; **the cuts are made in the caller's string** (it ends where the result ends; **(observed)** `filename` on `/a/b/@c.mgz#3@x` gives `c.mgz` and leaves `/a/b/@c.mgz`) |
| file path | everything before the last `/` (empty for `/name`); `.` without a `/` |
| remove extension | cut at the last `.` (even inside a directory name) |
| extension | after the last `.` (`b/c` for `a.b/c`); empty without one |
| from wildcard | a name containing `*` becomes the first existing path the C library's `glob` finds with no flags (sorted in byte order; `*`, `?`, `[...]`, `[!...]`, `\` escapes; a leading `.` matched only literally, `.*` matching `.` and `..` first); no match, or a name without `*` (even one with `?` or `[`), leaves it as it is |
| file exists | whether the file opens for reading (a directory does, on Linux; a file without read permission does not); `errno` unchanged |
| case-insensitive compare | the difference of the first differing bytes after upper-casing, as unsigned bytes (`\xff` against `a` is 255 - 65 = 190), 0 when equal |
| items in string | runs of non-space bytes; -1 for an empty string |
| count items | runs of bytes other than space and tab (a `\n` inside a run belongs to it); the count stops where a run would start with `\n` or `\r` |
| n-th item | the n-th white-space separated item (from 0; any negative n: the last); else `error: asking for item <n>, but <count> items in string` + `\n` (n shown as -1 for a negative n with no items) and nothing |
| deblank | a new string without the white space |
| copy | a new copy |
| first match | the first index of a value in a list, or -1 |
| an environment variable | its value, or the empty string when unset |
| ends with | whether a string ends with another (the empty string ends every string) |

**FS-11.** FreeSurfer's case-insensitive compare and extension helper copy their argument into a
1,024-byte buffer: a string of 1,024 bytes or more overflows it. Both modes work on the whole string,
with a notice. (Not probed: an overflow would make the probe's own results meaningless.)

## 11. Numbers and lists

- **IEEE tests on floats** (from the bits): infinite (+1 for +inf, -1 for -inf, else 0), NaN, finite.
- **Angle difference** (floats): `a - b` in float; above π (compared in double) it becomes
  `2·(float)π - d`, below -π `-2·(float)π - d` (once, not a full reduction), in float.
- **Squares** of a float in float, of a double in double.
- **Rounding** `nint(x)`: x + 0.5 (x - 0.5 below 0) in double, truncated toward zero; `nint2(x)`: the
  same with x + 0.49999999 at or above 0 (`nint2(0.5)` is 0, `nint(-0.5)` is -1). **FS-12:** beyond the
  32-bit range, or NaN, FreeSurfer gives -2147483648 (**(observed)** the group `numbers`, file
  `numbers-fs12`, which holds every such call of the group; 2^31 - 0.5 is one: x + 0.5 is 2^31).
  **Default:** saturated (NaN 0), with a notice; **reproduce:** -2147483648.
- **Standard deviation from sums** (Σx, Σx², n): mean = Σx/n; `sqrt((Σx² - 2·mean·Σx + n·mean·mean)
  / (n - 1))`, evaluated left to right in double (`2·mean·Σx` is `(2·mean)·Σx`; n = 1 divides by 0).
- **Comparisons** for sorting: integers -1, 0, 1; floats -1, 1, or 0 when equal **or either is NaN**.
- **Integer lists** (each sorts the list in place first, ascending): the count of distinct values;
  the distinct values in order; the most frequent value (the smallest of a tie) with its count.
  **FS-10:** for an empty list FreeSurfer reads the integer just past the list and reports one item
  (the count 1; the list holding the integer read; the most frequent value that integer, with the
  count 0; **(observed)** the probe's guard 7777 appears). **Default:** no items (most frequent: value
  0, count 0); **reproduce:** one item, its value 0, with a notice.
- **k-th smallest** (`kth_smallest(a, n, k)`, k from 1 to n; reorders the list) is Wirth's selection
  (*Algorithms + Data Structures = Programs*, 1976, as N. Devillard's 1998 public-domain note gives
  it), on the places 0 to n − 1 with t = k − 1, in exactly these steps (the order of the swaps is part
  of the result: callers read the list afterwards):
  1. l = 0, m = n − 1.
  2. While l < m: x = a[t]; i = l, j = m; repeat { while a[i] < x: i += 1; while x < a[j]: j −= 1; if
     i ≤ j: swap a[i] and a[j], i += 1, j −= 1 } until i > j; then if j < t: l = i; if t < i: m = j.
  3. The result is a[t].
- **Quick select** (`quick_select(a, n, k)`, k from 1 to n; reorders the list) is the "median of
  three" partition of *Numerical Recipes in C*, 2nd ed., §8.5, as Devillard's note gives it for the
  median, with t = k − 1 in place of the middle place:
  1. low = 0, high = n − 1.
  2. If high ≤ low: the result is a[t]. If high = low + 1: swap a[low] and a[high] when a[low] >
     a[high]; the result is a[t].
  3. middle = ⌊(low + high)/2⌋. Swap a[middle] and a[high] when a[middle] > a[high]; then a[low] and
     a[high] when a[low] > a[high]; then a[middle] and a[low] when a[middle] > a[low]. Then swap
     a[middle] and a[low + 1].
  4. ll = low + 1, hh = high. Repeat { ll += 1 until not a[low] > a[ll] (at least once); hh −= 1 until
     not a[hh] > a[low] (at least once); if hh < ll: stop; swap a[ll] and a[hh] }. Then swap a[low]
     and a[hh].
  5. If hh ≤ t: low = ll. If hh ≥ t: high = hh − 1. Go to step 2.

  Every comparison is a strict float comparison, false when a NaN is involved. **(observed)**: on
  3,000 lists with NaNs and infinities neither selection's result nor its reordering depends on memory
  outside the list (three different guards, README.md); the group `select` compares every k of
  lists up to 12 long, and 400 longer lists.
- **Median** of n floats: odd n, the k-th smallest with k = (n+1)/2; even n, quick select with k =
  n/2, then quick select with k = n/2+1 on the reordered list, and `0.5 × (q1 + q2)` (the sum in
  float, the product in double, rounded to float). **FS-09:** FreeSurfer's median of an empty list
  reads the float just before the list (**(observed)**: the guard). Both modes: NaN, with a notice.

**List statistics** (a list of n doubles): n ≤ 0: `ERROR: DListStats() nlist = <n>` + `\n` on
standard output, nothing returned; no list: `ERROR: DListStats() dlist is NULL` + `\n` (n is checked
first). Else `[n, mean, sd, min, max]`: the sum S and the sum Q of the double squares (each square
computed in double) in x87 extended precision (`larmorx_core::x87`: each double widened exactly,
then added), `mean = (double)(S / n)` (the quotient in extended precision), `sd = (double)
sqrt_ext(Q / n - (extended)(mean mean))` (the mean's square in double, the quotient, the difference
and the square root in extended precision), min and max from the first element with strict
comparisons (a NaN first element stays). **(observed)**: `dliststats 3 3ff0000000000000
4000000000000000 4008000000000000` gives `4008000000000000 4000000000000000 3fea20bd700c2c3e
3ff0000000000000 4008000000000000`; the group `numbers` holds 150 lists with NaNs and infinities.

## 12. The fast arctangent

A table T of 100,001 floats: `T[i]` = `atan2(i, 100000)` in double (correctly rounded; glibc's
equals it on every entry), rounded to float (SHA-256 of the floats' little-endian bytes, computed
from this definition: `4bb06d9e2f3a1b01bf4256f7befbd5f9842036095048023174106d0ef7f8addf`). For
(y, x): with `ax = |x|`, `ay = |y|`, the base angle is `T[(int)((100000·ay)/ax)]` when ax ≥ ay (float
product and quotient; `T[0]` when ax is 0), else `(float)(π/2) - T[(int)((100000·ax)/ay)]` in float.
Then, held in double: unchanged when x = |x| and y = |y|; negated when only y is negative;
`(float)π - r` when only x is negative; `r - (float)π` when both are; returned as float. A negative
zero counts as positive (atan2(0, -0) gives 0). With the variable
`FREESURFER_fastApproxAtan2f_use_atanf2` set (read once, at the first call), the C library's
`atan2f` is used instead and the first call writes `<BUILD_ROOT>/utils/utils.cpp:1727 using slow
atan2f` + `\n` on standard output (**(observed)** `single atan2f 3f800000 bf800000` with the variable
set: that line, then `4016cbe4`).

**FS-06.** For a NaN argument, two infinite ones, or `100000·ay` beyond the float range (ay above
about 3.4·10³³ with ax ≥ ay, or the same with the roles exchanged), FreeSurfer's lookup reads far
outside the table (**(observed)**: `single atan2f 7fc00000 3f800000`, `7f800000 7f800000` and
`7f000000 7f7fffff` end with a segmentation fault). Both modes: an overflowing product is computed on
both magnitudes scaled by 2^-64 (exact, the quotient an unbounded exponent gives); NaN and infinite
pairs give `atan2f`'s value; with a notice. One infinite argument with a finite other is no defect
(`(1, +inf)` gives 0, `(+inf, 1)` π/2).

## 13. Version options and stamps

The official 8.2.0 build's stamps: version `8.2.0`; build stamp
`freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45`; build time `Mar 14 2026 12:21:06`;
platform `Linux`; compiler `GCC` `8.4.0` (**(observed)** `versionstrings`, `allinfo`, `official.sh`).

- **Version options** (most programs handle them first): `--version` and `-version` match as
  **prefixes** (`--versionx` counts), `--all-info` and `-all-info` exactly (`-all-infox` does not). A
  version option writes `<name> freesurfer 8.2.0` + `\n` on standard output, `<name>` the name the
  program passes (`mri_info` for `mri_info`); an all-info option writes one line: `ProgramName:
  <name>  ProgramArguments: <the arguments, the program as it was started (its first argument)
  first, one space apart>
  ProgramVersion: 8.2.0  TimeStamp: <now, below>  BuildTime: Mar 14 2026 12:21:06  BuildStamp:
  <stamp>  User: <user>  Machine: <host name>  Platform: Linux  PlatformVersion: <kernel release>
  CompilerName: GCC  CompilerVersion: 8.4.0` (pairs two spaces apart; `UNKNOWN` for the user, or for
  the machine and the release, when they cannot be had). The options are removed; the program learns
  how many.
- **FS-07.** FreeSurfer's removal moves the later arguments one place back but goes on at the next
  place: the argument moved into a removed one's place is never examined, the copies left at the end
  are examined again, and the all-info line shows the arguments as moved so far. Official `mri_info
  --versionx -version --all-info`: one version line, then two all-info lines with `ProgramArguments:
  <path> -version --all-info --all-info` (**(observed)** `official.sh`; the probe's `version` alike);
  `p --version --version x` prints one version line and leaves `p --version x`. **Default:** each
  argument examined once, in order, the all-info line showing the command line as given;
  **reproduce:** FreeSurfer's way. A notice when the two differ.
- **Time stamps:** "now" is `20YY/MM/DD-hh:mm:ss-GMT` in UTC, YY the year's last two digits (1970
  shows 2070); a file's stamp is its modification time in local time (§14), `YYYY/MM/DD hh:mm:ss`
  (**(observed)**: a file stamped 1700000000 shows `2023/11/14 22:13:20` with `TZ=UTC0`, `23:13:20` in
  `Europe/Berlin`, `17:13:20` in `America/New_York`), or `ERROR: stating file <name>` + `\n` on
  standard output and nothing. The command line as one string: each argument followed by one space.
  The user is the effective user's account name.

## 14. Times and dates

- **The current date string:** the variable `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` when set
  (an empty value gives the empty string; unless the caller forbids the override), else the local time
  in the C locale's `%c` form: `Thu Nov 12 20:55:19 2026` (day and month abbreviations, the day of the
  month space-padded to two characters).
- **Local time** follows the program's `TZ`: unset, the zone file `/etc/localtime`; empty, UTC;
  otherwise (one leading `:` dropped) a zone file (an absolute path, or a name under `TZDIR`, else
  `/usr/share/zoneinfo`; RFC 8536's format, its footer rule after the last transition, before the
  first transition the first standard-time type) and, when no file can be read, a POSIX `TZ` string,
  else UTC. A POSIX rule applies to the **UTC** year, and **before 1970 the change times of 1970** are
  used (glibc 2.35, **(observed)**: `TZ=EST5EDT,M3.2.0,M11.1.0` is never in daylight time before 1970,
  `AEST-10AEDT,M10.1.0,M4.1.0/3` always).
- **A `dst` name without a rule** (`TZ=ABC3DEF`; the daylight offset one hour east of the standard
  one unless given) takes the zone file `posixrules` (found as a zone name is: under `TZDIR`, else
  `/usr/share/zoneinfo`) when it can be read: its transitions, each a change to standard or daylight
  time as the file's type for it is, with the `TZ` string's names and offsets; each transition's
  instant moved by the `TZ` string's offset minus the file's, both those in force just before it (the
  daylight ones when the time before it was daylight time and the file does not mark the transition
  as given in standard time, else the standard ones; a transition the file marks as given in UT not
  moved); before its first transition the `TZ` string's standard time; **after its last transition
  the file's own footer rule, with the file's own names and offsets**. Only where no `posixrules`
  can be read: `M3.2.0,M11.1.0`, change times 02:00 local. **(observed)** on the reference machine
  (its `posixrules` is New York's file, transitions until 2037): `TZ=ABC3DEF` is in daylight time
  from 2006-04-02 09:00 to 2006-10-29 08:00 UTC (New York's 07:00 and 06:00, moved by the 2 hours
  between the offsets) and on 1960-07-07, not on 2006-03-20; in 2040 it changes at New York's 07:00
  and 06:00 UTC and shows `EST`/`EDT` with -5 and -4 hours; with `TZDIR` naming a folder without
  `posixrules`, it follows `M3.2.0,M11.1.0` (daylight time on 2006-03-20, none before 1970). The group
  `time` compares 26 zones (these two included) at 451 times each, `gmtime`, `localtime` and `%c`.
- **Timers:** elapsed time from a clock in nanoseconds; milliseconds truncated, seconds, minutes and
  hours as `ns / 1e9`, `/ 6e10`, `/ 3.6e12` in double. An interval line: `<file>:<line> interval took
  <ms> msec` + `\n` on standard output. (FreeSurfer's clock is the wall clock; timings never reach a
  result.)

## 15. The environment record

Built from the environment, with messages on standard output: `FREESURFER_HOME` (unset: `FREESURFER_HOME
not defined` + `\n`, no record); `SUBJECTS_DIR` (unset: `SUBJECTS_DIR not defined` + `\n`, kept unset;
the user's name is recorded only when it is set); the working directory; the system's name, machine
type and host name; the colour table `$FREESURFER_HOME/FreeSurferColorLUT.txt` (read by `fs.labels`'
reader, whose own errors come first, §2; unreadable: `ERROR: reading <file>` + `\n`, no record); the
"now" stamp of §13; and `FS_DESIRED_BVEC_SPACE` read as `%d` (white space, a sign, decimal digits,
the rest ignored; the number's low 32 bits; default 2), which must be 1 (scanner) or 2 (voxel), else
`ERROR: FS_DESIRED_BVEC_SPACE = <value>, must be 1 or 2` + `\n` (the raw value) and no record
(**(observed)** the `fsenv` group: ` 2`, `2x`, `+2` and `4294967298` give 2, `4294967297` gives 1;
`3`, `0`, `-1` the error). Reading `SUBJECTS_DIR` alone: its value or `FSENVgetSUBJECTS_DIR:
SUBJECTS_DIR not defined` + `\n`; setting it overwrites the variable. **FS-08:** with no number in
`FS_DESIRED_BVEC_SPACE`, or a working directory that cannot be read (`ERROR: getcwd returned no path`,
no newline), FreeSurfer uses memory it never wrote. Both modes: the space 0 (so the error above and no
record), the directory empty, with a notice.

## 16. Memory and usage reports

- **Memory used:** the process's `VmData` (kB) from `/proc/<pid>/status`; when it cannot be read,
  `getting memoryused failed` (no newline) on standard error and -1.
- **Memory usage:** `VmSize`, `VmPeak`, `VmRSS`, `VmData`, `VmStk` (kB; tags matched without regard
  to case), each left as it was when absent (all of them where `/proc` is missing); the peak query
  keeps its five values from one call to the next.
- **Resource usage** (for the process itself): sixteen lines `<prefix><name><value>`: `utimesec    `
  and `stimesec    ` (seconds, `%lf`), then `ru_maxrss   `, `ru_ixrss    `, `ru_idrss    `,
  `ru_isrss    `, `ru_minflt   `, `ru_majflt   `, `ru_nswap    `, `ru_inblock  `, `ru_oublock  `,
  `ru_msgsnd   `, `ru_msgrcv   `, `ru_nsignals `, `ru_nvcsw    `, `ru_nivcsw   ` (integers; the fields
  Linux keeps at 0 are 0); written to a new file, 1 when it cannot be created. These values describe
  the run, never a result (the group `usage` compares the formats only).

## 17. Logic-problem reports

A table of counters per (file, line), a file told by its name's bytes; a line of one million or more
is a failed assertion (§3: `line < 1000000`, at `<BUILD_ROOT>/utils/utils.cpp:1760`).

- **Whether to check a place:** true at its first 32 calls, then at the 64th, 128th, 256th, ...
  (**(observed)** `logic`).
- **Coping with a problem** (an optional fixer variable, a message, file, line, function): the
  response is "fix" when the fixer variable is set (even empty), else "old". Only when
  `FREESURFER_reportLogicErrors` is set (read once per run; even empty) is it reported, with the same
  thinning on its own count: `<file>:<line> <function> - probable logic error: <message>, fixing it`
  (or `, not fixing it`) + `\n`, then, with a fixer variable, `.  Set <variable> to fix it` + `\n`, on
  standard output. The response is returned; whether it reported is given to the caller as well.

## 18. Temporary files

- **The base directory:** the first of `FS_TMPDIR`, `TMPDIR`, `TMP`, `TEMP`, `TEMPDIR` that is set,
  not empty and writable; when none is, the value of `TEMPDIR` (the last looked at), or `/tmp` when
  that is empty or unset; not writable: `error: Can not write to temporary directory <dir>` + `\n`,
  exit status 1. Writable means an existing directory (not a file, not a missing path) in which a new
  file `tmp.XXXXXX` (six letters or digits) can be made, and is then removed. **(observed)** the `temp`
  group: an unwritable, missing or file `TMPDIR` falls through to `/tmp`; the same `TEMPDIR` alone ends
  the program.
- **A temporary file** `<base>/tmp.XXXXXX<suffix>` (new, empty) or **directory** `<base>/tmp.XXXXXX`;
  failure: `error: Could not create temporary file in <base>` (or `directory`), exit status 1 (a
  suffix holding a `/` fails so). The six characters are unpredictable; their choice never reaches a
  result.

## 19. Threads, parallel loops, the debugging stack, HIPS and the licence check

- **Threads:** a FreeSurfer program never runs on more than 128 threads (asked for more, it uses 128,
  **(observed)** `maxthreads` with `OMP_NUM_THREADS` 129 and 1000). A thread numbered 128 or more
  asking its number writes `Freesurfer supports a maximum of 128 threads, set OMP_NUM_THREADS
  accordingly` + `\n` on standard error and ends the program with status 1; with several such threads
  each writes the line and the end races with them (**(observed)**: six runs with 130 threads gave
  status 139, 139, 1, 139, 139, 139). LARMORX never makes more than 128 such threads.
- **The parallel loops' levels.** Each of FreeSurfer's parallel loops is marked serial, experimental,
  fast, assume_reproducible or shown_reproducible; in the official build only the last two run on
  several threads, **the others always run on one thread** (**(observed)** `rompparallel`: `0 0 0 1 1`).
  The specs of the modules that hold the loops name their levels.
- **The reproducible sums.** A loop over the integers lo ≤ i < hi whose sums must not depend on the
  thread count is cut into p = min(hi − lo, 128) consecutive pieces (none when hi ≤ lo): piece j (from
  0) starts where piece j − 1 ended (piece 0 at lo) and has ⌊(hi − start)/(p − j)⌋ members, the
  quotient truncated (so the last pieces are the longest: [0, 300) gives 84 pieces of 2, then 44 of 3,
  **(observed)** `rompparts 0 300`). Each piece adds its terms in loop order into its own sums, which
  start at 0.0 (up to three sums per loop); at the end each sum is the variable's value before the loop
  plus the pieces' sums in piece order, one addition at a time, in double. Any thread count gives the
  same bits (**(observed)**: the group `romp` sums 200 lists with 1 and 8 threads, identical).
- **The debugging stack** (titles and notes of the functions entered, for crash reports): a current
  title and note (each kept to 511 bytes when set) and a stack of at most 49 (title, note) pairs, each
  pair copied to 512 bytes. *Entering a function* pushes the current title and note, then makes the
  given title current and the note empty; *leaving* pops the top pair back into the current title and
  note (to 511 bytes); popping an empty stack and pushing onto a full one change nothing (their
  messages, `ERROR: xDbg_PopStack call when stack is empty.` and `Stack limit reached, can't store
  name.`, go to an output that is never switched on). *The current function* is the title on top of the
  stack, `(No current function)` when it is empty: inside one entered function it is the title that
  was current before (**(observed)** `xdbg`). Its prints write nothing but **flush every output
  stream** (which decides how standard output and standard error interleave in a captured log).
- **HIPS** (an image format FreeSurfer no longer supports): every HIPS function writes `error: hips is
  not supported` + `\n` on standard error and returns 0 (those without a result return nothing;
  **(observed)** `hips`).
- **The licence check (reference only).** FreeSurfer's programs check a licence key file
  (`$FS_LICENSE`, else `$FREESURFER_HOME/license.txt`, else `$FREESURFER_HOME/.license`) and stop
  (exit status 255, or 1 for a failed decryption) with framed messages: `ERROR: FreeSurfer environment
  FREESURFER_HOME is not defined.`, `ERROR: FreeSurfer license file <file> not found.`, `ERROR:
  FreeSurfer license file <file> exists but you do not have read permission.`, `ERROR: FS_LICENSE
  environment variable points to a folder not a file`, `ERROR: Invalid FreeSurfer license key found in
  license file <file>`; with `DIAG_NO` above 0 it prints the file's name and lines on standard output.
  **LARMORX performs no licence check** (the key is a runtime check, not a term of FreeSurfer's
  licence: decision log 2026-10-19), never prints these messages and goes on as FreeSurfer does with
  a valid key.

## 20. Validation plan (black box)

Everything here runs the original as a black box: the probe `fs_core_probe` (calls of FreeSurfer's
compiled core functions and of the C library, built from the unit oracle's libraries, which
reproduce the official programs' code), prebuilt in `<workspace>/oracles/freesurfer-8.2.0-unit/probes/`
(the implementer runs it and opens nothing else there: its build, the larmorx commit and its SHA-256,
is in that folder's `BUILD_INFO.txt` for the orchestrator's check), and the official programs.
`validation/investigations/freesurfer-core-probe/` holds the probe's source (calls only), its
commands and formats (README.md), the comparison loop `compare.py` and `official.sh`.

### 20.1 The candidate and the loop

The clean room's own program (the *candidate*) takes the probe's commands with the same arguments
and prints the same forms, its functions behind them. `compare.py compare` runs every case on both
(the probe through the oracle-output cache, one thread, the family `freesurfer`, except the runs that
vary or set variables the cache's key leaves out; the candidate under the name `fs_core_probe`, with
`LARMORX_UPSTREAM_DEFECTS` set to the mode) and compares exit status, standard output, standard
error and the files a case names, byte for byte, after the masks of the README.

- `--mode reproduce`: **every run identical**, except the runs marked `memory` (FS-08, FS-09, FS-10:
  FreeSurfer read memory it never wrote, the probe's guard or whatever lay there).
- `--mode fix`: every run without a defect identical; a run marked with a defect may differ (the
  notice, the corrected value), and where the corrected output follows from the inputs the loop
  checks it itself: for FS-261 and FS-176 the byte reversal of every item, for FS-12 (`numbers-fs12`)
  the saturated values on standard output (NaN 0), for the finite check (`numbers-fs01`, FS-01) and
  the environment record without a package (`fsenv`, `FREESURFER_HOME=/nonexistent`, FS-01) the
  probe's standard output and its standard error without the NUL byte.
- Then the cases the probe cannot show (both modes defined by larmorx, or a crash): FS-03's exit
  status 1, FS-04's seed -1, FS-05's error line, FS-06's values against `atan2f` on NaN, infinite and
  overflowing pairs, FS-11 on strings of 1,024 bytes and more, FS-263's error value: the clean room's
  own tests, against this spec's text.

### 20.2 The groups and their reference hashes **(observed)**

`compare.py refs --seed 7` on the prebuilt probe (2026-11-13, the reference machine; SHA-256 of
each group's masked results in order; identical through the cache, without it, and on a second
run; the group `numbers` again after its rounding and finite-check calls moved to batches of their
own, the group `time` after its two zones without a rule were added). The hashes hold for the whole list of groups: one random sequence runs through the groups in
order, so `--groups` with a part of the list generates other cases (they compare as well, with other
hashes). The `time` group depends on the machine's `/etc/localtime` (its first zone is TZ unset) and on
the system's time-zone files; the others on nothing but the probe.

| Group | Runs | Calls | What | SHA-256 |
|---|---:|---:|---|---|
| swaps | 3 | 833 | §6: every swap on edge and random values, FS-176 (`swaplong64`), FS-261 (`byteswapbuf`, every k and n) | `62e1e91acc209996f091cd31ae54d99bc3f551f7613e0a63852ce915e66b4581` |
| cmdargs | 1 | 230 | §7 | `5c761bb5079e99b50601d71c3daf5b7debe16c074fc0841b7adafd5e0125841d` |
| strings | 1 | 4,670 | §10's string and file-name helpers on 325 strings (white space, `/`, `#`, `@`, `.`, high bytes) | `c6f74d1c9bafea60c7685682d3019b95f6795530ee815ca88c54d13c660edac1` |
| fgetl | 1 | 300 | §10's line reader: comments, blank lines, long lines, sizes 2 to 200 | `ab086d6391f392b5f1dfe24f8527ce93eed66c4339f8277d42578cd69a8d9f85` |
| numbers | 3 | 1,388 | §11: IEEE tests, angles, rounding, deviations, comparisons, list statistics; FS-12's roundings and the finite check (§5, its C-style errors: FS-01) in batches of their own | `e59cdd00962b3357196dbe7155958bf40255da02e063f1b1d89e67d628c2b7a2` |
| lists | 2 | 303 | §11's integer lists (FS-10 apart) | `d0917c004a3c0998e9c0405265aea5ec52456fbf6b811cc59fcc6b100f763e60` |
| select | 2 | 2,137 | §11's selections and median with ties, NaNs, zeros, infinities (FS-09 apart) | `b932a2116df85719a7baa6a314f4ca18be6b1fd3b51e7e9fd07ce224a3957276` |
| atan2 | 3 | 120,054 | §12: `(i, 100000)` for every table index, 20,000 random pairs, signed zeros, the `atan2f` switch | `2bd50151bb181e72fa2ea8d021c0c57dde8023341f378dd3c0a015e61bdb84ab` |
| romp | 2 | 708 | §19's levels, pieces and sums (1 and 8 threads) | `5ba96f5fd86e169cf448c12364e808b04893ca2d245051053f3c31b237edf853` |
| clib | 1 | 370 | the C library's readings §2, §4, §9 and §15 rely on: `strerror`, `atoi`, `atof`, `%lx`, `%d`, `stol` | `5b4d0b4d2633e5f379d0163ecb6b814f29ea705a34bac2c387ba333c1fbaf96b` |
| time | 26 | 35,178 | §14: `gmtime`, `localtime`, `%c` in 26 zones (zone files, POSIX rules, a daylight name without a rule with and without `posixrules`, unreadable names) | `d532ad87e384da97be0ce8e04af36c97a88983bdd55e6141ef429e5560f557a0` |
| files | 1 | 34 | §10's file existence and wildcards on a small tree | `bdd5c3e1318ad004926129e66839c5c1c43f709b8843218b5f6ba207e9aa18df` |
| init | 27 | 27 | §4 (FS-02 included) | `ea9c12da2d0372d2e27c4ba104db66f2c981c611e6614b8a3ab4404fb202ed6e` |
| messages | 72 | 72 | §2, §3, §5 (FS-01, FS-03, FS-263), the error log, HIPS | `9d47b1773af33de00cafed87333934d73ba3161cb62c84bde1f81e48aaf0666d` |
| random | 36 | 36 | §9: 21 seeds, reseeding, ranges, permutations, `FREESURFER_SEED` (FS-04, FS-05) | `37d801a215a260eb086c55459f5d33f195214a2ac57ebadeb5a19a01151bb4bb` |
| version | 139 | 139 | §13: 127 argument lists (FS-07), stamps, the date string | `07004daeeed7aa67ec6d13bdfa3cd0a76186a42284caf742738b17e98e16701f` |
| fsenv | 18 | 18 | §15 (FS-08; FS-01 in the colour table's error without a package) | `7e85be0e7e40424c9dd2c38fbabc87ae04fd76909dfd1509488f4aeb4d490396` |
| logic | 7 | 7 | §17 | `90d5854f75b15b63a7104fcf977f2d9c0574ec8550e87f3ebae19804cd5b0ec4` |
| temp | 37 | 37 | §18: each variable set to a writable, unwritable, missing, file and empty value | `9de571991793d2afbd6d343dded69b1e9de75f11e90d375ded48118a60835967` |
| usage | 5 | 5 | §16 (formats) | `38a3e55a0bc7847d14e401269dc3ca92d613ab47f9cbb831e97f0c6e8f5a849e` |
| xdbg | 4 | 4 | §19's debugging stack (53 levels, long texts) | `08d1bd69ada015b0f5003963b423ede16292e7efeb5859c0a2195e3d16f5d53f` |
| threads | 6 | 6 | §19's thread limit | `6d33e1f71b0f224b39b3b00a6f9e3b3e6aa2782c453507c77577799daa596fa6` |

### 20.3 Speed

The clean room is built for speed in the usual sense (porting guide, "Clean-room originals: the same
behaviour, built for speed"); most of these services are not hot. The exceptions are the fast
arctangent (called per vertex and voxel: one table, built once, shared without locks), the
selections and the median (called per voxel neighbourhood by some programs), the byte swaps of whole
buffers, and the reproducible sums (their pieces are the unit of parallel work). Measure them against
the probe's batch commands.

### 20.4 The official programs

`official.sh <FREESURFER_HOME>` runs FreeSurfer 8.2.0's own `mri_info` (the unpacked package, one
thread): a missing file (FS-01's bytes, exit status 1), `PROFILE` 7.9, 1e10 and nan (FS-02),
`DIAG_VERBOSE` and `DIAG` (the verbose line), `--version`, `--all-info`, `--versionx -version
--all-info` and `--version --version x` (FS-07) **(observed** 2026-11-13: as §2, §4 and §13 say**)**.
The clean room's program-level checks come with the first programs that use it (`mri_info`'s clean
room): the same cases, byte for byte after the masks.

### 20.5 The replays

The replays of the recorded heads (`<workspace>/oracles/freesurfer-8.2.0/runs/`, the replay tool, run
as a black box) check these services inside the programs once their clean rooms exist: the seeded
programs (`mri_normalize`, `mris_smooth`, `mris_ca_label -seed 1234`, `mri_coreg`), every program's
messages, `--version` and `--all-info` (time stamps, user and host masked).

### 20.6 Golden cases

One case per path in the cross-platform golden suite (CLAUDE.md rule 10): the swaps (FS-261 and
FS-176 defaults), the string and file-name helpers, the rounding and list helpers, the selections
and median, the arctangent table's SHA-256 and a few hundred pairs, the reproducible sums, the
seeded draws for three seeds, the version options' plans (FS-07 default), the messages' bytes
(FS-01 default); no file system, clock or environment beyond what each case sets.

## 21. References

- ISO/IEC 9899:2011 (C11): 7.21.6.1 (`printf`), 7.21.6.2 (`scanf`'s `%d`, `%lx`), 7.22.1 (`atoi`,
  `atof`, `strtol`, `strtod`), 7.24 (`strcmp`); ISO/IEC 14882:2011 (C++11) 21.5 (`std::stol`) and
  15.5.1 (an exception leaving a destructor calls `std::terminate`: FS-263).
- POSIX.1-2017 (IEEE Std 1003.1): `glob`, `fnmatch`, `mkstemp`, `mkstemps`, `mkdtemp`, the `TZ`
  variable (8.3), `localtime`, `strftime` (`%c` in the POSIX locale), `getrusage`, `uname`,
  `getpwuid`.
- RFC 8536, "The Time Zone Information Format (TZif)", 2019.
- The GNU C Library Reference Manual (glibc 2.35): "Error Codes" (the `strerror` texts), "TZ
  Variable".
- IEEE 754-2008; Intel 64 and IA-32 Architectures Software Developer's Manual, vol. 1, §4.8.3.7 (the
  default NaN) and §4.8.4 (the integer indefinite value).
- N. Wirth, *Algorithms + Data Structures = Programs*, Prentice-Hall, 1976 (the k-th smallest);
  N. Devillard, "Fast median search: an ANSI C implementation", 1998, public domain (the selections as
  FreeSurfer's steps take them, with its k); W. H. Press et al., *Numerical Recipes in C*, 2nd ed.,
  Cambridge University Press, 1992, §8.5.
- G. Marsaglia and A. Zaman, "A new class of random number generators", Annals of Applied
  Probability 1 (1991) 462–480 (`vnl_random`'s generator; `larmorx_core::vnl_random`).

## 22. The defects (summary)

| ID | What | Rule | Default | `reproduce` |
|---|---|---|---|---|
| FS-01 | a NUL byte before the newline of C-style errors | A | none | the NUL |
| FS-02 | `PROFILE` beyond the int range or NaN | undefined conversion | saturated, NaN 0 | -2147483648 |
| FS-03 | a failed assertion's deliberate crash | crash | exit status 1 | exit status 139 |
| FS-04 | unseeded draws seeded from the clock | nondeterminism | seed -1 | the clock |
| FS-05 | a `FREESURFER_SEED` that is no 64-bit integer aborts | B | error, status 1 | abort, 134 |
| FS-06 | the fast arctangent's lookup outside its table | out-of-bounds read | defined values (both modes) | defined values |
| FS-07 | the version options skip and repeat arguments | A | each argument once | FreeSurfer's loop |
| FS-08 | unwritten memory for the b-vector space and the working directory | memory | 0 and empty (both modes) | 0 and empty |
| FS-09 | the median of an empty list | memory | NaN (both modes) | NaN |
| FS-10 | the integer lists of an empty list | memory | no items | one item of value 0 |
| FS-11 | 1,024-byte buffers of the compare and extension helpers | overflow | the whole string (both modes) | the whole string |
| FS-12 | `nint`, `nint2` beyond the int range or NaN | undefined conversion | saturated, NaN 0 | -2147483648 |
| FS-176 | 64-bit integer swaps round through a double (fs.io's 64-bit readers and writers too) | A | the exact reversal | the rounding |
| FS-261 | buffer swaps reverse only the first ⌈n/k⌉ items | A | every item | the first ⌈n/k⌉ |
| FS-263 | with exceptions on, a nonzero fatal error aborts | A | the code reaches the caller | abort, 134 |
