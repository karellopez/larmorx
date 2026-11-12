# FreeSurfer 8.2.0's core services: behaviour spec (`fs.core`)

Written by the replica author of queue item 3.1-22 as its spec writer (`docs/porting-guide.md` §1):
behaviour, conventions, messages and the validation plan, from FreeSurfer 8.2.0's source and the
official programs' output, never code or routines. A clean-room implementer writes the clean room
from this file, black-box runs of the official programs and the cited standards only.

Status: **part 1 of 2** (the messages, the fatal errors, the diagnostics, the byte swaps, the
command-line helpers). Part 2 (the program name and version options, the environment helpers,
the random numbers, the file-name and string helpers, the timers, the temporary files, the thread
switch) follows in the same file.

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
| error | `error: ` + message + `\n` (the message also appended, with a newline, to the error log when one is set; a log that cannot be opened is skipped silently) |
| warning | `warning: ` + message + `\n` (not in the log) |
| debug | `debug: ` + message + `\n`, only when the diagnostic bit `0x10000000` (verbose) is set |
| fatal | as an error, then the program ends (§3) |

No colour codes are ever written. Messages are formatted by the caller with the C library's `printf`
rules (C11 7.21.6.1); the caller's own trailing newline, when its format has one, is kept, so such a
message ends with two newlines.

**The C-style error calls** (FreeSurfer's older API: "print an error" and "print an error and exit",
and the "print and return a value" form built on the first), used by most library code:

1. record the error code as the last error;
2. if `errno` is not 0, write an error whose message is the C library's text for it (glibc's English
   texts: 2 `No such file or directory`, 13 `Permission denied`, 21 `Is a directory`, ...; a code
   glibc has no text for: `Unknown error <n>`);
3. write the caller's message as an error;
4. for "print and exit": end the program with the error code (§3).

**FS-01.** FreeSurfer's step 3 writes one **NUL byte** (0x00) after the message, before the newline,
on standard error and in the error log (official `mri_info` on a missing file:
`error: mghRead(/nonexistent.mgz, -1): could not open file` + NUL + `\n`, exit status 1). A file
holding such lines (`recon-all.log` captures them) reads as binary to `grep` and other text tools.
**Default:** no NUL byte; **reproduce:** the NUL byte, exactly as above. Only these C-style calls
add it; the plain error, warning, debug and fatal messages never do.

## 3. Ending a program

- **Fatal error with code c:** the program exits with status `c mod 256` (the code's low 8 bits:
  -1 gives 255, -4 gives 252). FreeSurfer's error codes: 0 none, -1 no file, -2 no memory,
  -3 unsupported, -4 bad parameter, -5 bad file, -6 size, -7 bad loop, -8 out of bounds.
- **With exceptions on** (a program can switch them on): a fatal error with a code other than 0
  throws that code instead of exiting; **a fatal error with code 0 does nothing**: the program goes on
  after the message (a quirk kept: callers rely on code 0 never stopping them in this mode).
- **"Flag needs arguments"** (the older programs' option parser): `ERROR: <flag> flag needs <n>
  argument` (`arguments` unless n is 1) + `\n` on standard error, exit status 255.
- **A failed internal assertion** writes `ASSERTION FAILED: <file>:<line> <test>` + `\n` on
  standard output and on standard error, then ` break <BUILD_ROOT>/utils/diag.cpp:85` + `\n` on
  standard output, where BUILD_ROOT is the official build's directory,
  `/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0` (the `<file>` of an
  assertion in the library starts with it too). **FS-03:** FreeSurfer then dies of a segmentation
  fault on purpose (status 139 as a shell reports it); with the variable `FREESURFER_HANG_ON_ASSERT`
  set it first loops for ever printing `FREESURFER_HANG_ON_ASSERT` on both streams. **Default:** exit
  status 1 after the messages; **reproduce:** exit status 139. The endless loop is never reproduced.

## 4. Diagnostics set from the environment

At start most programs read these variables (when set):

| Variable | Sets | Read as |
|---|---|---|
| `DIAG_NO` | the diagnostic index | `atoi` |
| `IMAGE_SIZE` | the image size | `atoi` |
| `DIAGX`, `DIAGY`, `DIAGZ` | the diagnostic voxel | `atoi` |
| `DIAGVX`, `DIAGVY`, `DIAGVZ` | the diagnostic vertex | `atoi` |
| `PROFILE` | the profiling level, then prints `turning profiling diagnostics on (<level>)...\n` on standard output | `atof`, converted to a 32-bit int |
| `diag`, else `DIAG` | or-ed into the diagnostic bits | the literal `0x` (lower case), then `scanf`'s `%lx`; anything else leaves the bits alone |
| `DIAG_VERBOSE` (any value, even empty) | the verbose bit `0x10000000` | presence |

Then, when the verbose bit is set, `verbose diagnostics enabled...\n` on standard output (after the
profiling line). `atoi` is glibc's: white space, a sign, decimal digits, a 64-bit `long` clamped to
its range, then its low 32 bits (`99999999999999999999` gives -1). `%lx` skips white space, takes a
sign and an optional `0x`/`0X`, reads hexadecimal digits (beyond 64 bits: all ones), a minus negates
modulo 2^64.

**FS-02.** The `PROFILE` level is the value truncated toward zero; FreeSurfer's conversion of a value
beyond the 32-bit range, or NaN, is undefined in C and gives -2147483648 on x86-64. **Default:** the
value saturated to the 32-bit range (NaN gives 0); **reproduce:** -2147483648.

The same start-up sets the numeric locale (to `en_US` or `C.UTF-8`, a warning `Please install the
en_US locale for numeric consistency` when the machine has none of them, not when `FS_DISABLE_LANG`
is set): numbers are always written as in the C locale, and **the warning is never written** by
LARMORX (it only reports a missing locale on the machine). It also makes FreeSurfer's OpenMP loops run
on **one thread when `OMP_NUM_THREADS` is unset**: where a program's results depend on its thread
count, the result to reproduce is that thread count's.

## 5. Other diagnostics

- **Diagnostic print** (a message and a set of bits): nothing, returning -1, when the bits are not 0
  and share none with the diagnostic bits; otherwise the message on standard output, returning its
  length in bytes.
- **Heartbeat** (a fraction done, a float): remembers the last fraction printed (start -10). A smaller
  fraction than the remembered one resets it to -20. When the fraction exceeds the remembered one by
  more than 0.10 (float subtraction, compared in double), it is remembered and, with the heartbeat bit
  `0x4000` set, `<100*fraction>% finished\n` is written on standard error (`%2.1f`, the product in
  float). **Percent done** (a fraction and a count n) does the same with its own memory and the step
  `1/n` in float (n = 0: never prints).
- **Finite check** (a place name and a double): 1 when finite; else the C-style error (§2) with code
  -4 and the message `<place> not finite!` + `\n`, and 0.

## 6. Byte order

Swapping a 16-bit, 32-bit (integer or float) or 64-bit float value reverses its bytes; bit patterns
are kept (NaN payloads included).

## 7. Command-line helpers of the older programs

A *flag* is an argument starting with `--`; a *single dash* is an argument of at least two characters
starting with one `-` not followed by another; "the n-th remaining argument is a value" when it
exists and is not a flag.

## 8. Validation plan

- Unit checks of every behaviour above (the replica's tests in `crates-freesurfer/larmorx-freesurfer-core`),
  against the unit oracle (queue item 3.1-02) when it exists: the C-style error output with and without
  `errno`, the exit statuses, the exceptions mode, `DiagInit` on the variables above (including the
  `atoi`/`%lx` edge cases and FS-02's values), the heartbeat sequences, the swaps.
- Black-box runs of the official programs: `mri_info` on a missing file (FS-01's bytes and status 1);
  `DIAG_VERBOSE=1 mri_info <volume>` (the verbose line on standard output); `PROFILE=3` (the profiling
  line).
- The replays of the first programs that use these services (every recorded step prints through §2).
