# FreeSurfer's core services: black-box checks

Closed to FreeSurfer clean-room implementers only in what `docs/findings/freesurfer-core.md`
says of the source; the scripts themselves run the official program or the C library and read
nothing of FreeSurfer's source.

`blackbox.sh <FREESURFER_HOME>` runs the official FreeSurfer 8.2.0 `mri_info` (from the unpacked
`freesurfer_ubuntu22-8.2.0_amd64.deb`, `docs/freesurfer-plan.md`) on a file that does not exist,
with the diagnostic variables of `specs/freesurfer-core.md` §4, and with the version options
(§11), and prints each case's standard output, its standard error as `od -c` shows it, and its
exit status (the user's and machine's names masked).

What it showed (2026-11-12, the main machine): every missing-file case exits 1; standard error is
`error: mghRead(/nonexistent.mgz, -1): could not open file` followed by a NUL byte and a newline
(FS-01); `PROFILE=7.9` prints `turning profiling diagnostics on (7)...`, `PROFILE=1e10` and
`PROFILE=nan` print `(-2147483648)` (FS-02); `DIAG_VERBOSE=` (empty) and `DIAG=0x10000000` print
`verbose diagnostics enabled...` on standard output. `--version` prints `mri_info freesurfer
8.2.0`; `--all-info` the one-line report with `BuildTime: Mar 14 2026 12:21:06`, `BuildStamp:
freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45`, `CompilerVersion: 8.4.0`;
`--versionx -version --all-info` prints the version once and the report twice with
`ProgramArguments: <mri_info> -version --all-info --all-info` (FS-07). The replica's unit tests
(`crates-freesurfer/larmorx-freesurfer-core`, modules `log`, `diag`, `version`) hold the same bytes.

`ctime_probe.py <probe>` compares the replica's `localtime` and `strftime("%c")`
(`ctime::localtime`, through the example `ctime_probe` of `larmorx-freesurfer-core`: `cargo build
--release --example ctime_probe` in `crates-freesurfer/`) with the C library's (CPython's
`time.localtime` and `time.strftime`, glibc 2.35 on the main machine) in 24 zones (TZ unset,
empty, zone files with and without footers, POSIX strings with and without rules, unreadable
names) at seeded times from 1900 to 2100 and every 7 hours over two years. Result (2026-11-12,
`--cases 5000`): 180,240 cases, 0 differ (before the two measured rules were added, 1,187 POSIX-rule
cases differed: the rule's year is the UTC year, and before 1970 glibc uses 1970's change times).

`stol_probe.cpp` shows what an uncaught `std::stol` exception does (FS-05): built with the
machine's `g++` and run on `abc` and `99999999999999999999`, it prints its first line, then
`terminate called after throwing an instance of 'std::invalid_argument'` (or `'std::out_of_range'`)
and `  what():  stol` on standard error, and aborts (exit status 134); `" 12x"` gives 12.
