# FreeSurfer 8.2.0's core services: behaviour spec (`fs.core`)

Written by the replica author of queue item 3.1-22 as its spec writer (`docs/porting-guide.md` §1):
behaviour, conventions, messages and the validation plan, from FreeSurfer 8.2.0's source and the
official programs' output, never code or routines. A clean-room implementer writes the clean room
from this file, black-box runs of the official programs and the cited standards only.

Status: **parts 1 and 2 of 2.** Part 1 (§1-8): the messages, the fatal errors, the diagnostics,
the byte swaps, the command-line helpers. Part 2 (§9-19): the random numbers, the string and
file-name helpers, the numbers and lists, the fast arctangent, the version options and stamps, the
environment record, the times and dates, the memory and usage reports, the logic-problem reports,
the temporary files, the thread limit, the debugging stack and the licence check (for reference).

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

## 9. Random numbers

Every FreeSurfer program draws from **one** generator: VXL's `vnl_random` as ITK 4.13.2 carries it
(Marsaglia and Zaman's subtract-with-borrow seeded by a linear congruential generator; the main
package's `larmorx_core::vnl_random`, its own spec and record), through a draw function that keeps
the last seed it was given and **reseeds** the generator only when called with a different seed
(it starts at seed 0, its generator made with seed 0), and returns `drand64(0, 1)` **rounded to
float**.

- **Seeding** with a seed s (a 64-bit signed integer): the C library's `rand` and `random` are seeded
  with s's low 32 bits; when s equals the current seed, one draw is made with s - 1 first (so the
  sequence restarts); the current seed becomes s; **one draw is made with s and discarded**; the
  draw count becomes 1. A program's first number after seeding is therefore the generator's second.
- **A number between low and hi** (swapped when low > hi): the float draw, widened to double, times
  `hi - low`, plus `low` (double arithmetic, no fused multiply-add); the draw count goes up by one.
  A result outside [low, hi] (a draw rounded up to 1.0f) is reported as a C-style error (§2) with
  code -4 and the message `randomNumber(<low>, <hi>) - <value>` + `\n`, each as `%2.1f` of the value
  rounded to float, and returned anyway.
- **No seed yet** (current seed 0; seeding with 0 also leaves it 0): when the variable
  `FREESURFER_SEED` is set and not empty, `Setting seed for random number genererator to <value>`
  (sic, the raw value) + `\n` on standard output, then seeding with it read as C++'s `std::stol`
  (white space, a sign, decimal digits; the rest ignored: ` 12abc` is 12, `-0x10` is 0). Otherwise
  `<name> supposed to be reproducible but seed not set` + `\n` on standard error, once per run, where
  `<name>` is the program file's first 15 bytes (`/` written as `@`; empty where the system has no
  `/proc`), and **FS-04**: FreeSurfer seeds with `-|t|`, t the clock's seconds truncated to 32 bits
  (so no two runs agree). **Default:** the seed -1; **reproduce:** the clock. The line is written in
  both modes.
- **FS-05.** A `FREESURFER_SEED` without digits, or beyond the 64-bit range, ends FreeSurfer with
  `terminate called after throwing an instance of 'std::invalid_argument'` (beyond the range:
  `'std::out_of_range'`) + `\n` + `  what():  stol` + `\n` on standard error and an abort (exit status
  134), after the `Setting seed` line. **Default:** `error: FREESURFER_SEED must be an integer in the
  range of a 64-bit long, not '<value>'` + `\n`, exit status 1; **reproduce:** FreeSurfer's lines and
  134.
- **A permutation of 0 .. n-1:** start from the identity; for each place i in order, swap it with the
  place given by a number between 0 and `n - 0.0001` truncated toward zero.

## 10. Strings and file names

Strings are bytes up to the first NUL; white space is the C locale's (space, `\t`, `\n`, `\v`, `\f`,
`\r`), case changes touch ASCII letters only.

| Helper | Result |
|---|---|
| read a line (with an n-byte buffer) | the next piece the C library's `fgets` gives (at most n - 1 bytes, through `\n`) that is not empty, blank or starting with `#` after its leading white space; returned without that white space, cut at its first `#`, without its final `\n`; nothing at the end of the file. A line longer than n - 1 bytes comes in pieces |
| file name only | the part after the last `/`, cut at its last `#`, then at its last `@` |
| file name | the same after skipping one leading `@` of that part |
| file path | everything before the last `/` (empty for `/name`); `.` without a `/` |
| remove extension | cut at the last `.` (even inside a directory name) |
| extension | after the last `.`; empty without one |
| from wildcard | a name containing `*` becomes the first (byte order) existing path the C library's `glob` finds with no flags (`*`, `?`, `[...]`, `\` escapes; a leading `.` matched only literally); no match leaves it as it is |
| file exists | whether the file opens for reading (a directory does, on Linux); `errno` unchanged |
| case-insensitive compare | the byte difference of the first differing upper-case bytes, 0 when equal |
| items in string | runs of non-space bytes; -1 for an empty string |
| count items | runs of bytes other than space and tab; the count stops where an item would start with `\n` or `\r` |
| n-th item | the n-th white-space separated item (from 0; any negative n: the last); else `error: asking for item <n>, but <count> items in string` (n shown as -1 for a negative n with no items) and nothing |
| deblank | the string without its white space |
| first match | the first index of a value in a list, or -1 |

**FS-11.** FreeSurfer's case-insensitive compare and extension helper copy their argument into a
1,024-byte buffer: a string of 1,024 bytes or more overflows it. Both modes work on the whole string,
with a notice.

## 11. Numbers and lists

- **IEEE tests on floats** (from the bits): infinite (+1 for +inf, -1 for -inf, else 0), NaN, finite.
- **Angle difference** (floats): `a - b` in float; above π (compared in double) it becomes
  `2·(float)π - d`, below -π `-2·(float)π - d` (once, not a full reduction).
- **Rounding** `nint(x)`: x + 0.5 (x - 0.5 below 0) truncated toward zero; `nint2(x)`: the same with
  x + 0.49999999 at or above 0. **FS-12:** beyond the 32-bit range, or NaN, FreeSurfer's conversion
  is undefined (-2147483648 on x86-64). **Default:** saturated (NaN 0), with a notice;
  **reproduce:** -2147483648.
- **Standard deviation from sums** (Σx, Σx², n): mean = Σx/n; `sqrt((Σx² - 2·mean·Σx + n·mean·mean)
  / (n - 1))`, evaluated left to right in double.
- **Comparisons** for sorting: integers -1, 0, 1; floats -1, 1, or 0 when equal **or either is NaN**.
- **Integer lists** (each sorts the list in place first, ascending): the count of distinct values;
  the distinct values; the most frequent value (the smallest of a tie) with its count. **FS-10:** for
  an empty list FreeSurfer reads past it and reports one item. **Default:** no items (most frequent:
  value 0, count 0); **reproduce:** one item, its value 0, with a notice.
- **k-th smallest** (k from 1; reorders the list): Wirth's selection (*Algorithms + Data Structures =
  Programs*, 1976): repeatedly partition the active range around the value at place k - 1, scanning
  from both ends with strict comparisons and swapping, until the range is one place. **Quick select**
  (k from 1; reorders the list): *Numerical Recipes in C*, 2nd ed., §8.5's `select` (median of the
  ends and the middle moved to the low end, the partition scanning with strict comparisons). **Median**
  of n floats: odd n, Wirth's selection of the (n+1)/2-th; even n, quick select of the n/2-th, then of
  the n/2+1-th on the reordered list, and `0.5 × (q1 + q2)` (the sum in float, the product in double,
  rounded to float). Callers that read the list afterwards see these algorithms' orders. **FS-09:**
  FreeSurfer's median of an empty list reads the value before it. Both modes: NaN, with a notice.

**List statistics** (a list of n doubles; FreeSurfer's `DListStats`): n ≤ 0: `ERROR: DListStats()
nlist = <n>` + `\n` on standard output, nothing returned; no list: `ERROR: DListStats() dlist is
NULL` + `\n`. Else `[n, mean, sd, min, max]`: the sum S and the sum Q of the double squares in x87
extended precision (each double widened exactly, then added), `mean = (double)(S / n)`, `sd =
(double) sqrt_ext(Q / n - (extended)(mean mean))` (the mean's square in double, the square root
in extended precision), min and max from the first element with strict comparisons. Checked
against the official code: 1,000 lists in the fs.maths probe (`specs/freesurfer-maths.md` §6), all
identical.

## 12. The fast arctangent

A table T of 100,001 floats: `T[i]` = `atan2(i, 100000)` in double (correctly rounded; glibc's
equals it on every entry), rounded to float. For (y, x): with `ax = |x|`, `ay = |y|`, the base angle is
`T[(int)((100000·ay)/ax)]` when ax ≥ ay (float product and quotient; `T[0]` when ax is 0), else
`(float)(π/2) - T[(int)((100000·ax)/ay)]` in float. Then, held in double: unchanged when x = |x| and
y = |y|; negated when only y is negative; `(float)π - r` when only x is negative; `r - (float)π` when
both are; returned as float. A negative zero counts as positive (atan2(0, -0) gives 0). With the
variable `FREESURFER_fastApproxAtan2f_use_atanf2` set (read once, at the first call), the C library's
`atan2f` is used instead and the first call writes `<build root>/utils/utils.cpp:1727 using slow
atan2f` + `\n` on standard output (§3's build root).

**FS-06.** For a NaN argument, two infinite ones, or `100000·ay` beyond the float range, FreeSurfer's
index is not a number and its lookup reads outside the table. Both modes: an overflowing product is
computed on both magnitudes scaled by 2^-64 (exact, the quotient an unbounded exponent gives); NaN
and infinite pairs give `atan2f`'s value; with a notice.

## 13. Version options and stamps

The official 8.2.0 build's stamps: version `8.2.0`; build stamp
`freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45`; build time `Mar 14 2026 12:21:06`;
platform `Linux`; compiler `GCC` `8.4.0`.

- **Version options** (most programs handle them first): `--version` and `-version` match as
  **prefixes** (`--versionx` counts), `--all-info` and `-all-info` exactly. A version option writes
  `<program> freesurfer 8.2.0` + `\n` on standard output; an all-info option writes one line:
  `ProgramName: <program>  ProgramArguments: <the arguments, the program's name first, one space
  apart>  ProgramVersion: 8.2.0  TimeStamp: <now, below>  BuildTime: Mar 14 2026 12:21:06  BuildStamp:
  <stamp>  User: <user>  Machine: <host name>  Platform: Linux  PlatformVersion: <kernel release>
  CompilerName: GCC  CompilerVersion: 8.4.0` (pairs two spaces apart; `UNKNOWN` for the user, or for
  the machine and the release, when they cannot be had). The options are removed; the program learns
  how many.
- **FS-07.** FreeSurfer's removal moves the later arguments one place back but goes on at the next
  place: the argument moved into a removed one's place is never examined, the copies left at the end
  are examined again, and the all-info line shows the arguments as moved so far. Official `mri_info
  --versionx -version --all-info`: one version line, then two all-info lines with `ProgramArguments:
  mri_info -version --all-info --all-info`; `p --version --version x` leaves `p --version x`.
  **Default:** each argument examined once, in order, the all-info line showing the command line as
  given; **reproduce:** FreeSurfer's way. A notice when the two differ.
- **Time stamps:** "now" is `20YY/MM/DD-hh:mm:ss-GMT` in UTC, YY the year's last two digits (1970
  shows 2070); a file's stamp is its modification time in local time (§14), `YYYY/MM/DD hh:mm:ss`, or
  `ERROR: stating file <name>` + `\n` on standard output and nothing. The command line as one string:
  each argument followed by one space. The user is the effective user's account name.

## 14. Times and dates

- **The current date string:** the variable `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` when set
  (unless the caller forbids the override), else the local time in the C locale's `%c` form:
  `Thu Nov 12 20:55:19 2026` (day and month abbreviations, the day of the month space-padded to two
  characters).
- **Local time** follows the program's `TZ`: unset, the zone file `/etc/localtime`; empty, UTC;
  otherwise (one leading `:` dropped) a zone file (an absolute path, or a name under `TZDIR`, else
  `/usr/share/zoneinfo`; RFC 8536's format, its footer rule after the last transition, before the
  first transition the first standard-time type) and, when no file can be read, a POSIX `TZ` string
  (a `dst` name without a rule takes `M3.2.0,M11.1.0`, change times 02:00 by default), else UTC. A
  POSIX rule applies to the **UTC** year, and **before 1970 the change times of 1970** are used
  (glibc 2.35, measured: northern rules are then never in daylight time, southern ones always).
- **Timers:** elapsed time from a monotonic clock in nanoseconds; milliseconds truncated, seconds,
  minutes and hours as `ns / 1e9`, `/ 6e10`, `/ 3.6e12` in double. An interval line:
  `<file>:<line> interval took <ms> msec`. (FreeSurfer's clock is the wall clock; timings never reach
  a result.)

## 15. The environment record

Built from the environment, with messages on standard output: `FREESURFER_HOME` (unset: `FREESURFER_HOME
not defined` + `\n`, no record); `SUBJECTS_DIR` (unset: `SUBJECTS_DIR not defined` + `\n`, kept unset;
the user's name is recorded only when it is set); the working directory; the system's name, machine
type and host name; the colour table `$FREESURFER_HOME/FreeSurferColorLUT.txt` (unreadable: `ERROR:
reading <file>` + `\n`, no record); the "now" stamp of §13; and `FS_DESIRED_BVEC_SPACE` read as `%d`
(a number's low 32 bits; default 2), which must be 1 (scanner) or 2 (voxel), else `ERROR:
FS_DESIRED_BVEC_SPACE = <value>, must be 1 or 2` + `\n` and no record. Reading `SUBJECTS_DIR` alone:
its value or `FSENVgetSUBJECTS_DIR: SUBJECTS_DIR not defined` + `\n`; setting it overwrites the
variable. **FS-08:** with no number in `FS_DESIRED_BVEC_SPACE`, or a working directory that cannot be
read (`ERROR: getcwd returned no path`, no newline), FreeSurfer uses memory it never wrote. Both modes:
the space 0 (so the error above and no record), the directory empty, with a notice.

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
  the run, never a result.

## 17. Logic-problem reports

A table of counters per (file, line); a line of one million or more is a failed assertion (§3:
`line < 1000000`, at `<build root>/utils/utils.cpp:1760`).

- **Whether to check a place:** true at its first 32 calls, then at the 64th, 128th, 256th, ...
- **Coping with a problem** (an optional fixer variable, a message, file, line, function): the
  response is "fix" when the fixer variable is set, else "old". Only when `FREESURFER_reportLogicErrors`
  is set (read once per run) is it reported, with the same thinning on its own count: `<file>:<line>
  <function> - probable logic error: <message>, fixing it` (or `, not fixing it`) + `\n`, then, with a
  fixer variable, `.  Set <variable> to fix it` + `\n`, on standard output; whether it reported is
  returned.

## 18. Temporary files

- **The base directory:** the first of `FS_TMPDIR`, `TMPDIR`, `TMP`, `TEMP`, `TEMPDIR` that is set and
  writable; when none is, the value of `TEMPDIR` (the last looked at), or `/tmp` when that is empty;
  not writable: `error: Can not write to temporary directory <dir>` + `\n`, exit status 1. Writable
  means a new file `tmp.XXXXXX` (six letters or digits) can be made in it, and is then removed.
- **A temporary file** `<base>/tmp.XXXXXX<suffix>` (new, empty) or **directory** `<base>/tmp.XXXXXX`;
  failure: `error: Could not create temporary file in <base>` (or `directory`), exit status 1. The six
  characters are unpredictable; their choice never reaches a result.

## 19. Threads, the debugging stack and the licence check

- **Threads:** a FreeSurfer program never runs on more than 128 threads (asked for more, it uses 128);
  a thread numbered 128 or more would end it with `Freesurfer supports a maximum of 128 threads, set
  OMP_NUM_THREADS accordingly` + `\n` on standard error, exit status 1.
- **The debugging stack:** functions may push their title and note (each up to 512 bytes; at most 49
  kept, further pushes print `Stack limit reached, can't store name.`) and pop them (the popped title
  and note, up to 511 bytes, become the current ones; popping an empty stack prints `ERROR:
  xDbg_PopStack call when stack is empty.`); titles and notes set directly are cut to 511 bytes. Its
  prints write nothing (its output is never switched on) but **flush every output stream**.
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

## 20. Validation plan, part 2

- Unit checks of §9-19 (the replica's tests in `crates-freesurfer/larmorx-freesurfer-core`): the
  seeding sequence (reseeds, discarded draws, counts), the scaled draw, `FREESURFER_SEED` and FS-04,
  FS-05 in both modes; every string and file-name helper with its edge cases; the IEEE tests, the
  angle fold, rounding with FS-12, the lists with FS-10, the selections and median with FS-09 and
  NaNs; the arctangent table's SHA-256 (`4bb06d9e2f3a1b01bf4256f7befbd5f9842036095048023174106d0ef7f8addf`,
  the floats' little-endian bytes), its sign rules and FS-06; the version options against the official
  `mri_info` (`validation/investigations/freesurfer-core/blackbox.sh`) and FS-07; the environment
  record with FS-08; the logic-problem thinning; the temporary files.
- Local time against the C library: `validation/investigations/freesurfer-core/ctime_probe.py`, 24
  zones, 180,240 times, 0 differ (2026-11-12).
- Against the unit oracle (queue item 3.1-02) when it exists: `randomNumber`'s sequences for the seeds
  the programs set (-1, 1234, 53, ...), with `larmorx_core::vnl_random` installed; the selections'
  reorderings on random lists with ties and NaNs; `fastApproxAtan2f` on random pairs.
- The replays of the first programs that use these services (the seeded programs: `mri_normalize`,
  `mris_smooth`, `mris_ca_label -seed 1234`, `mri_coreg`; every program's `--version` and
  `--all-info`, with the time stamps, user and host masked).
