# FreeSurfer 8.2.0's core services, as a black box

The probe behind the **(observed)** marks and the validation plan of
[`specs/freesurfer-core.md`](../../../specs/freesurfer-core.md) (`fs.core`: messages, fatal errors,
diagnostics, byte order, command-line helpers, the seeded random numbers, strings and file names,
numbers and lists, the fast arctangent, version stamps, times, the environment record, memory
reports, logic-problem reports, temporary files, threads, the debugging stack). **Open to the
clean-room implementer of that spec**: the files here hold only calls of FreeSurfer's functions (and
of the C library's) by name, the reading and printing of their arguments and results, and a
comparison loop; no FreeSurfer code, nothing written from its source. The implementer runs the
prebuilt probe (below) and the comparison loop; building the probe reads the unit oracle's build
tree, which is closed to FreeSurfer's clean-room implementers, so they never build it.

| File | What it is |
|---|---|
| `fs_core_probe.cpp` | the probe: one call (or one sequence of calls) of FreeSurfer's core functions per run, or a batch of calls from a file; its commands below |
| `build.sh` | builds the probe with the unit oracle's compiler (Ubuntu's GCC 8.4.0) and FreeSurfer's flags against `<workspace>/oracles/freesurfer-8.2.0-unit` (queue item 3.1-02): `build.sh <workspace> <output>` (the spec writer's or the orchestrator's step, not the implementer's) |
| `compare.py` | the comparison loop of the spec's §20: 22 groups of cases (about 162,700 calls in 368 runs), run on the probe (through the oracle-output cache) and on a candidate program that takes the same commands; `list`, `refs`, `compare` |
| `official.sh` | the official `mri_info` as a black box (the spec's §20.4): a missing file, the diagnostic variables, the version options: `official.sh <FREESURFER_HOME>` |
| `guards.py` | whether the selections and the median depend on memory outside the list, on lists with NaNs and infinities (three guards): `python -P guards.py <fs_core_probe> <scratch folder>` |

The prebuilt probe: `<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_core_probe`. Its build (the
larmorx commit, this folder's `build.sh`, its SHA-256) is recorded in that folder's `BUILD_INFO.txt`
for the orchestrator's check; the implementer only runs the probe (every other access to the unit
oracle's folder, reading or hashing a file in it included, counts as opening FreeSurfer's build
tree).

## Encodings

Every argument and every printed value uses one of these forms, so any byte string can pass:

| Kind | Form |
|---|---|
| string | `x` followed by the hex digits of its bytes, two per byte (`x2f746d70` is `/tmp`; `x` alone is the empty string); `null` is no string |
| float | the 8 hex digits of its IEEE 754 bits (`3f800000` is 1.0) |
| double | the 16 hex digits of its bits (`3ff0000000000000` is 1.0) |
| integer | decimal |

## Batch commands

`fs_core_probe batch <file>` reads one call per line (`<command> <arguments>`, separated by single
spaces) and prints one result line per call on standard output, in order; what the called function
itself writes (on standard output or standard error) comes before that line. `fs_core_probe single
<command> <arguments>` makes one such call in a run of its own (for the calls that end a run).

| Command | Calls | Prints |
|---|---|---|
| `swapshort i`, `swapushort i`, `swapint i`, `swaplong32 i`, `swaplong64 i` | `swapShort`, `swapUShort`, `swapInt`, `swapLong32`, `swapLong64` | the integer |
| `swapfloat f`, `swapdouble d` | `swapFloat`, `swapDouble` | the bits |
| `byteswapbuf k n buffer` | `ByteSwapBuf(buffer, n, k)` | its result and the buffer after the call |
| `arch486` | `Arch486` | the integer |
| `isflag s`, `singledash s`, `stringmatch s1 s2`, `nthisarg nth s...` | `CMDisFlag`, `CMDsingleDash`, `CMDstringMatch`, `CMDnthIsArg(count, strings, nth)` | the integer (`stringmatch null null` is not called: `skip`) |
| `fgetl n content` | `fgetl(buffer, n, file)` on a file holding `content`, until it returns nothing | each returned string, then `end` |
| `strupper s`, `strlower s`, `str_toupper s` | `StrUpper`, `StrLower`, `str_toupper` | the string |
| `filenameonly s`, `filenamepath s`, `removeext s`, `extension s` | `FileNameOnly`, `FileNamePath`, `FileNameRemoveExtension`, `FileNameExtension` (into a buffer of the caller) | the string |
| `filename s` | `FileName(s)` | the result, then `s` as the call left it |
| `fileexists s` | `FileExists` (with `errno` set to 1234 before) | the result, then 1 if `errno` is still 1234 |
| `fromwildcard s` | `FileNameFromWildcard` | the string |
| `stricmp a b`, `itemsinstring s`, `countitems s`, `nthitem n s`, `deblank s` | `stricmp`, `ItemsInString`, `CountItemsInString`, `GetNthItemFromString(s, n)`, `deblank` | the integer or the string |
| `deltaangle f f`, `devisinf f`, `devisnan f`, `devfinite f` | `deltaAngle`, `devIsinf`, `devIsnan`, `devFinite` | the float or the integer |
| `nint d`, `nint2 d`, `sum2stddev d d n`, `cmpints i i`, `cmpfloats f f` | `nint`, `nint2`, `sum2stddev`, `compare_ints`, `compare_floats` | the integer or the double |
| `nunique i...`, `unique i...`, `mostfrequent i...` | `nunqiue_int_list`, `unqiue_int_list`, `most_frequent_int_list` | the result (`unique`: the count and the values; `mostfrequent`: the value and its count), `\|`, the list as the call left it |
| `kth k f...`, `qselect k f...`, `median f...`, `mad f...` | `kth_smallest`, `quick_select`, `median`, `mad` | the result, `\|`, the list as the call left it |
| `atan2f y x` | `fastApproxAtan2f(y, x)` | the float |
| `dliststats n d...` (or `n null`) | `DListStats` | the five doubles, or `null` |
| `findfirst v i...`, `argv2cmdline s...`, `checkfinite place d` | `FindFirstMatch`, `argv2cmdline`, `check_finite` | the integer or the string |
| `rompsum d0 d...` | a reproducible ROMP loop summing the values into `d0` | the double |
| `rompparts lo hi` | `ROMP_Distributor_begin` on `[lo, hi)` | the number of pieces, then each `lo:hi` |
| `rompparallel` | `ROMP_if_parallel1` for the levels serial, experimental, fast, assume_reproducible, shown_reproducible | five integers |
| `strerror n`, `atoi s`, `atof s`, `scanlx s`, `scand s`, `stol s` | the C library's `strerror`, `atoi`, `atof`, `sscanf(s, "0x%lx")`, `sscanf(s, "%d")`, C++'s `std::stol` | the string, the integer, the double; `scanlx`/`scand`: the count matched and the value (`-` when none); `stol`: the value or `invalid_argument`/`out_of_range` |
| `glob s` | the C library's `glob(s, 0)` | its result, then every path |
| `gmtime t`, `localtime t`, `strftimec t` | `gmtime_r`, `localtime_r`, `strftime("%c")` of the local time | year-1900, month, day, hour, minute, second, weekday, day of the year, daylight flag, offset, zone name (a string); or the `%c` string |

The lists of `nunique` ... `mad` lie between two guard values (7777, as an integer or a float; the
variable `FS_CORE_PROBE_GUARD` chooses another integer), so a result read from just outside the list
shows as the guard.

## Run commands

One call (or one sequence) per run; the environment is as the caller gives it.

| Command | What it does |
|---|---|
| `init` | `DiagInit(NULL, NULL, NULL)`, then `return=<bits>` and the diagnostic state |
| `errorprintf errno code message [log file] [throw] [twice]` | `errno` set, then `ErrorPrintf(code, "%s", message)` (twice with `twice`), then `Gerror` and `errno`; `log` sets the error log first |
| `errorexit errno code message [log file] [throw]` | `ErrorExit(code, "%s", message)`; with `throw`, exceptions on (`throwExceptions(true)`) and a handler that prints `caught <code>`; `returned` when it returns |
| `fatal code message [throw]`, `error message [log file]`, `warning message`, `debug Gdiag message` | `fs::fatal(code)`, `fs::error()`, `fs::warning()`, `fs::debug()` with the message (`Gdiag` in hex) |
| `argnerr flag n`, `usageexit` | `CMDargNErr(flag, n)`, `CMDusageExit` |
| `assert file line test` | `assertFailed(file, line, test)` |
| `diagprintf Gdiag bits message` | `DiagPrintf(bits, "%s", message)`, then a newline and `return=<n>` |
| `heartbeat Gdiag f...`, `pctdone Gdiag n f...` | `DiagHeartbeat(f)`, `DiagShowPctDone(f, n)` for each value |
| `random step...` | steps `seed=<long>` (`setRandomSeed`), `draw=<double>,<double>` (`randomNumber(low, hi)`: prints the draw), `perm=<n>` (`compute_permutation(n, NULL)`: prints `perm` and the list), `state` (prints `getRandomSeed`, `getRandomCalls`) |
| `version progname argv0 argument...` | `handleVersionOption(argc, argv, progname)`, then `n=<result>` and the first `argc - result` arguments |
| `allinfo progname argv0 argument...`, `versionstrings`, `curstamp`, `filestamp path`, `user`, `datetime 0\|1` | `getAllInfo`; `getVersion` and `getBuildStamp`; `VERcurTimeStamp`; `VERfileTimeStamp`; `VERuser`; `currentDateTime(allowOverride)` |
| `fsenv`, `getsubjectsdir`, `setsubjectsdir s` | `FSENVgetenv` (then its fields, or `NULL`), `FSENVgetSUBJECTS_DIR`, `FSENVsetSUBJECTS_DIR` (then the variable's value) |
| `logic file line n` | `spendTimeCheckingForLogicProblem(file, line)` n times: one digit per call |
| `cope fixer message file line function n` | `copeWithLogicProblem2` n times (`fixer` may be `null`): each call's response (0 old, 1 fix) and whether it reported |
| `tempdir`, `tempfile suffix`, `mktempdir`, `dirwritable s` | `getBaseTempDir`, `makeTempFile`, `makeTempDir`; `directoryExists` and `directoryIsWritable` |
| `memused`, `memusage`, `rusage prefix`, `writerusage prefix file` | `getMemoryUsed`, `GetMemUsage` and `GetVmPeak`, `PrintRUsage` on standard output, `WriteRUsage` |
| `xdbg step...` | steps `push=<title>,<note>`, `pop`, `enter=<title>` (entering a function), `exit`, `desc=<text>`, `note=<text>`, `cur` (prints the current function, note and description) |
| `maxthreads`, `threadnum n` | the threads OpenMP gives after FreeSurfer's start-up; n threads asking their number |
| `hips` | one HIPS function with a result and one without, then the result |

## The comparison loop

```sh
# the cases, one line each (and the runs and calls per group)
python -P compare.py list --workspace <workspace> --scratch <scratch>/core
# the probe's reference hashes per group (the spec's §20 quotes them)
PYTHONPATH=validation/src python -P compare.py refs --workspace <workspace> \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_core_probe --scratch <scratch>/core
# a candidate against the probe, in each mode
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 4 python -P \
  validation/investigations/freesurfer-core-probe/compare.py compare --workspace <workspace> \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_core_probe \
  --candidate <the candidate> --scratch <scratch>/core --mode reproduce   # then --mode fix
```

**The candidate** is the clean room's own program taking the commands above with the same
arguments and printing the same forms (its library behind them). The loop runs it under the name
`fs_core_probe` (a link in the scratch folder: FreeSurfer's messages name the running program) with
`LARMORX_UPSTREAM_DEFECTS` set to the mode, in the same working directory and environment as the
probe. The cases are generated from `--seed` (default 7) into `<scratch>/batch`; the files they read
into `<scratch>/work`.

**The masks** (both programs' outputs alike): the scratch folder, the FreeSurfer package's folder,
the user's and the machine's names and the kernel's release become `<scratch>`, `<fs-home>`,
`<user>`, `<machine>`, `<release>` (also in hex); `20YY/MM/DD-hh:mm:ss-GMT` stamps become `<now>`;
the six characters of temporary names become `XXXXXX`; the memory and time reports' numbers become
`N`; the draws and seeds of the clock-seeded runs (FS-04), `<draw>` and `<clock>`.

**What may differ.** A run whose case names a defect (`[FS-xx]` in `list`) may differ in `--mode
fix`: there larmorx corrects FreeSurfer. For FS-261 and FS-262 the loop checks the corrected values
itself (the byte reversal of every item). A run marked `memory` (FS-08, FS-09, FS-10) shows what
FreeSurfer read outside the memory it wrote (the guard, or whatever lay there): it may differ in both
modes. Every other run must be identical, byte for byte, in both modes. Exit status 1 when a
difference is unexplained; the first three of each group are printed.

**The probe's runs** go through the oracle-output cache (one thread, the family `freesurfer`), except
those that vary from run to run (the clock, the time, temporary names, memory reports, the user and
the machine, the time-zone files, a directory tree), those whose results are files, and those that
set a variable the cache's key for FreeSurfer's family leaves out (two such runs would otherwise
share an entry; since 2026-11-13 the key holds every variable FreeSurfer reads, `DIAG`, `DIAG_NO`,
`PROFILE`, `IMAGE_SIZE`, `TMPDIR` and the others, so none of the loop's cases is left out for
that). `--no-cache` runs them directly.

## Results (2026-11-13, the reference machine: glibc 2.35, Linux 6.8, x86-64)

- `refs`, seed 7, the prebuilt probe (commit `bd50220e`): the hashes quoted in the spec's §20.2;
  identical through the cache and without it, and on a second run.
- `compare` with the probe as its own candidate: every run identical in both modes (the loop's own
  check).
- `guards.py`: 3,000 lists with NaNs and infinities through `kth`, `qselect` and `median` with three
  different guards (7777, -7777, 0): no result and no order depends on memory outside the list.
- `official.sh`: the official `mri_info` prints the C-style error with its NUL byte and exits 1,
  prints the profiling and verbose lines as the probe's `init` does, and handles the version options
  as the probe's `version` does (FS-07); `--version --version x` leaves `--version` to `mri_info`'s
  own parser, which prints `8.2.0` and exits 1.
- `threadnum 130` (more than FreeSurfer's 128 threads) is not among the cases: each thread numbered
  128 or more prints the limit's line, and the run ends with status 1 or, at random, a segmentation
  fault (status 139); six runs gave 139, 139, 1, 139, 139, 139.
- The facts the spec marks (observed) come from these runs; the commands are beside each.
