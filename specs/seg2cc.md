# `seg2cc` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-28: every option, message, exit status and path, the defects with
their switches, the forms not reproduced, the validation plan, §9). Written by the replica's author
(track A, relay 58) as the spec writer: behaviour and messages, for the clean room
(`larmorx-fs-volumes`), which works from this file and black-box runs of the official script only.
§2 (the shell's behaviours) is shared by `specs/mergeseg.md` and `specs/fscalc.md`. Every statement
about the shell and the system programs is measured on the official scripts (`/bin/tcsh` 6.21.00 as
Ubuntu 22.04 builds it, GNU coreutils 8.32, GNU time 1.9, procps 3.3.17, the C locale) and on small
test scripts; **(observed)** marks them.

## 1. What it is for

`seg2cc --s <subject>` adds the corpus callosum's five labels (251 to 255) to a subject's
segmentation: recon-all runs it at its stage "CC Seg" in both streams, on
`$SUBJECTS_DIR/<subject>/mri/aseg.auto_noCCseg.mgz`, writing `aseg.auto.mgz`,
`transforms/cc_up.lta` and `scripts/seg2cc.log`. It runs FreeSurfer's programs `mri_info`,
`mri_cc`, `mri_convert`, `mri_label2vol`, `mri_binarize` and the script `mergeseg`
(`specs/mergeseg.md`); their outputs are theirs (each program's own spec).

## 2. The shell's behaviours (shared with mergeseg and fscalc) (observed)

- **Lines.** Every line a script prints is the shell's `echo` of its words joined by one blank and
  a new line. tcsh's `echo` also reads the words: a first word `-n` is dropped with the line's end;
  `\a \b \c \e \f \n \r \t \v \\` and `\0` with up to three octal digits are characters (`\c` ends
  the output there, without a new line); any other backslash stays (FS-1278).
- **Logs.** `echo ... | tee -a <log>` writes the line to standard output and appends it to the log
  (created when missing); a log that cannot be opened: `tee: <name>: <reason>` on standard error
  and the pipeline fails (status 1), standard output is still written. `echo ... >> <log>` that
  cannot be opened: `<name>: <reason>.` on standard error and the script ends, status 1. `<reason>`
  is the C library's text for Linux's error code (`No such file or directory`, `Is a directory`,
  `Permission denied`, ...). The log `/dev/null` discards.
- **A pipeline's status** is the status of the last of its commands that failed, else 0 (tcsh's
  `anyerror`).
- **Undefined variables.** A script that reads a variable the environment does not set prints
  `<NAME>: Undefined variable.` on standard error and ends, status 1.
- **The current folder.** The shell's folder starts as `PWD` when that names the folder the script
  starts in, else that folder's physical name (links resolved). `pushd <dir>` makes it `<dir>`
  (relative to it), `.` and `..` taken off as text, links kept, and prints the directory stack on
  standard output: the current folder, then the saved ones, each followed by one blank, then a new
  line; a folder that is `HOME` or inside it is shown as `~` / `~/...`. `popd` returns and prints
  the stack the same way. A folder that cannot be entered: `<dir>: No such file or directory.`
  (`Not a directory.`, `Permission denied.`) and the end of the script. A program the script runs
  starts in the folder's physical name, with `PWD` set to the shell's name of it. `pwd` prints the
  physical name.
- **`date`** prints `%a %b %e %H:%M:%S %Z %Y` in the C locale (`Tue Nov 24 17:46:31 UTC 2026`),
  the time zone from `TZ` (unset: `/etc/localtime`; empty: UTC; a TZif file by name under
  `/usr/share/zoneinfo` or `TZDIR`; else a POSIX `TZ` string; anything unreadable: UTC), `%Z` the
  zone's abbreviation then in effect. Other locales print other texts (not reproduced: the C
  locale's form).
- **`uname -a`** (Ubuntu's coreutils on Linux): `<kernel name> <host> <release> <version> <machine>
  <machine> <machine> GNU/Linux`, the version as `/proc/sys/kernel/version` holds it (its trailing
  blank kept).
- **`cat <file>`** that cannot be read: `cat: <name>: <reason>` on standard error, nothing on
  standard output.
- **`mkdir -p <dir>`**: the folders made; a part that is a file: `mkdir: cannot create directory
  '<the name up to it>': Not a directory` (`File exists`, `Permission denied`) on standard error,
  status 1 (the scripts do not look at it). `rm -f <file>`: nothing for a missing file; a folder:
  `rm: cannot remove '<name>': Is a directory`. `rm -rf`: the folder and its contents.
- **The help test.** Before their options, the scripts ask whether the echo of all their arguments
  holds the text `-help` (then the usage and the help, status 1) or `-version` (then the version
  line, status 0): anywhere, inside a subject or a file name too (FS-1274).
- **Names read again (FS-1277).** The scripts never quote their variables: an argument, the
  subjects folder, `FREESURFER_HOME` or (mergeseg) the output's folder holding blanks, new lines,
  `*`, `?`, `[...]`, `{`, a leading `~` or `=`, or an empty word, is read again as shell words
  (split, matched against files, expanded, dropped). Not reproduced (§8).
- **`fs_time <program> <args>`** (FreeSurfer's front end of GNU `time`): the program's standard
  output, then, when it failed, `Command exited with non-zero status <n>`, then
  `@#@FSTIME  <date '+%Y:%m:%d:%H:%M:%S'> <program> N <number of args> e <elapsed s, 2 places,
  truncated> S <system s> U <user s> P <(U+S)*100/elapsed, an integer>% M <peak resident kB> F
  <major faults> R <minor faults> W 0 c <involuntary switches> w <voluntary switches> I <512-byte
  blocks read> O <blocks written> L <1-min load> <5-min load> <15-min load>` (two blanks after the
  key; the loads as `uptime` prints them, `%.2f`), then `@#@FSLOADPOST <date> <program> N <n>
  <loads>`, all on standard output; the status is the program's. Only on Linux with `/usr/bin/time`
  (GNU); elsewhere the program alone. `FS_TIME_ALLOW=0` runs the program alone; `FSTIME_LOAD=0`
  leaves the loads out (the resource line then ends with `O <n> `); fs_time sets `FS_TIME_ALLOW=1`
  and `FSTIME_LOAD=1` for the program when they are unset. The figures describe the run; they are
  never compared.
- **`UpdateNeeded <target> <source>...`** (FreeSurfer's csh script) prints `1` when the target does
  not exist, when a source does not exist (then status 1), or when a source's modification time is
  later than the target's (nanoseconds), else `0`.
- **The end lines.** `echo " "`, `Started at <date at the start> ` (a blank at the end), `Ended   at
  <date>`, `<Name>-Run-Time-Sec <whole seconds>`, `<Name>-Run-Time-Hours <h>` (`<h>`: the seconds
  divided by 3600 with 20 decimals, truncated, then rounded to 2 places as a long double: 18 s
  gives `0.01`), `echo " "`, `<name> Done`, each through `tee -a` the log.
- **Process numbers.** `$$` is the script's process number.

## 3. The command line

`seg2cc` without arguments: the usage (§7), status 1. Then the help test (§2). Before the help
test, `FREESURFER_HOME` is read (unset: `FREESURFER_HOME: Undefined variable.`, status 1, even for
`--help`); `$FREESURFER_HOME/sources.csh` is sourced when it exists (on Linux it does nothing).

The options, in any order, each read with its value from the next argument (none left: `ERROR: flag
<flag> requires one argument`, status 1):

| Option | Effect |
|---|---|
| `--s <subject>` | the subject (required) |
| `--cc <name>` | the output segmentation's name, default `aseg.auto` (`<name>.mgz`) |
| `--nocc <name>` | the input segmentation's name, default `aseg.auto_noCCseg` |
| `--n <name>` | the norm volume's name, default `norm` |
| `--ctab <ctab>` | accepted and ignored (FS-1276) |
| `--threads <n>` | accepted and ignored |
| `--expert <file>` | FS-1275 |
| `-lh`, `-lh-only`, `--lh-only` / `-rh`, `-rh-only`, `--rh-only` | `-lh` / `-rh` given to mri_cc |
| `--log <file>` | the log, default `$SUBJECTS_DIR/<subject>/scripts/seg2cc.log` |
| `--nolog`, `--no-log` | the log `/dev/null` |
| `--force-update`, `--no-force-update` | run every step whatever the files' ages (default not) |
| `--tmp <dir>`, `--tmpdir <dir>` | the temporary folder, and no cleanup |
| `--nocleanup`, `--cleanup` | keep / remove the temporary folder (default remove) |
| `--debug` | the shell traces each command it runs on standard error (not reproduced, §8) |

Any other argument: `ERROR: Flag <flag> unrecognized.`, then all the arguments, status 1.

## 4. The checks

1. No `--s`: `ERROR: must supply a subject name`, status 1.
2. `SUBJECTS_DIR` unset: `SUBJECTS_DIR: Undefined variable.`, status 1.
3. `<segvol>` = `$SUBJECTS_DIR/<subject>/mri/<nocc>.mgz`, `<normvol>` the same with `<n>`: the
   first that does not exist: `ERROR: cannot find <it>`, status 1.
4. Both made full names with getfullpath (`specs/getfullpath.md`, its default).
5. Without `--force-update`: when UpdateNeeded of `<cc>.mgz` (in the subject's mri folder) against
   the two says `0`: `  seg2cc: update not needed. Run with --force-update to force an update`
   (two blanks first), status 0.

## 5. The run

1. `mkdir -p` of `<subject>/scripts`, `<subject>/mri/transforms`, `<subject>/mri/tmp` (under
   `$SUBJECTS_DIR`).
2. The temporary folder: `--tmpdir`'s, else `/scratch/tmpdir.seg2cc.<process>` when `/scratch` is a
   folder the user may write in, else `<subject>/mri/tmp/tmpdir.seg2cc.<process>`; `mkdir -p` it.
3. The log: unless `/dev/null`, `rm -f` it; then `Log file for seg2cc` `>>` the log (the line is not
   on standard output); then through `tee -a`: the date, an empty line, `setenv SUBJECTS_DIR
   <SUBJECTS_DIR>`, `cd <pwd>`, `<the script's full name> <the arguments>` (the script's name as
   FreeSurfer's installation holds it: `$FREESURFER_HOME/bin/seg2cc`), an empty line,
   `$FREESURFER_HOME/build-stamp.txt`'s text, `seg2cc 8.2.0`, `uname -a`. With `PBS_JOBID` set:
   `pbsjob <PBS_JOBID>` `>>` the log.
4. `pushd <subject>/mri` (the stack printed on standard output, not in the log).
5. `mri_info --o <tmp>/isconformed --conformed-to-min <segvol>` (its standard output discarded,
   its standard error passed); the file's first line's words: `IsConformed <words>` through `tee`.
   No file: cat's message, `IsConformed` alone, and the second path below. Two or more words: `if:
   Expression Syntax.`, status 1.
6. **Conformed** (the word `yes`): when UpdateNeeded `<cc>.mgz <nocc>.mgz` says 1 or
   `--force-update`: the date (tee), then the step `mri_cc -aseg <nocc>.mgz -o <cc>.mgz -lta
   transforms/cc_up.lta [-lh] [-rh] <subject>`, then the date.
7. **Otherwise**, each step only when UpdateNeeded of its output against its inputs says 1 (or
   `--force-update`), in the subject's mri folder: `mri_convert <normvol> <n>.conf.mgz
   --conform_min` (inputs `<normvol>`); `mri_label2vol --seg <segvol> --temp <n>.conf.mgz
   --regheader --o <nocc>.conf.mgz` (inputs `<n>.conf.mgz`, `<segvol>`); `mri_cc -norm
   <n>.conf.mgz -aseg <nocc>.conf.mgz -o <cc>.conf.mgz -lta transforms/cc_up.lta [-lh] [-rh]
   <subject>` (input `<nocc>.conf.mgz`); then always `mri_label2vol --seg <cc>.conf.mgz --temp
   <normvol> --regheader --o <cc>.noconf.mgz`; then for each label 251, 252, 253, 254, 255:
   `mri_binarize --i <cc>.noconf.mgz --match <label> --o <tmp>/ccbin.mgh` and `mergeseg --src
   <src> --merge <tmp>/ccbin.mgh --o <cc>.mgz --segid <label> --tmpdir <tmp>/tmp.mergeseg --ctab
   $FREESURFER_HOME/FreeSurferColorLUT.txt [--cleanup]` (`<src>`: `<nocc>.mgz` for 251, then
   `<cc>.mgz`; `--cleanup` unless no cleanup).
8. A **step**: its command line through `tee`, then `fs_time` of it through `tee`; a failure (of
   the program or the tee): `ERROR: <the command line>` on standard output, status 1.
9. `popd` (printed), `rm -rf` the temporary folder unless no cleanup, the end lines (§2), status 0.

## 6. The defects (FreeSurfer's behaviour with `reproduce`; the default corrects)

- **FS-1274** (the help test, §2): by default only the arguments `-help` and `--help` (`-version`,
  `--version`) ask.
- **FS-1275**: `--expert <file>` runs FreeSurfer's check of the file (`ERROR: cannot find <file>`,
  or `ERROR: multiple occurrences of a command in expert options file <file>` when two of its lines
  without `#` have the same first word, blank lines counting as an empty word; standard output) and
  then always ends: after a failed check `cmd: Undefined variable.`, after a passed one
  `XOptsFiles: Undefined variable.` (standard error, status 1). By default the check alone: status 1
  after its message when it fails, else seg2cc goes on (the file is not used).
- **FS-1276**: `--ctab` is ignored; by default its table is given to mergeseg's `--ctab`.
- **FS-1279**: a relative `--log` or `--tmpdir` (or a relative `SUBJECTS_DIR`, from which their
  defaults come) is read again in the subject's mri folder after step 4: log lines after it go to
  another file (or fail with tee's message, and a failing step's pipeline then fails), the
  conformation answer (step 5) is looked for there (not found: the second path). By default both
  names are taken from the folder seg2cc starts in.
- **FS-1280**: a relative `SUBJECTS_DIR` reaches the programs run in the subject's mri folder as it
  is (mri_cc does not find the subject). By default the programs get its absolute name (the log's
  `setenv` line keeps the name as given).
- **FS-1278**, **FS-1277** (§2).

Each notes itself on standard error (`larmorx: upstream defect FS-... `) when it changed the result.

## 7. The usage and the help

The usage, each line with its new line: an empty line; `seg2cc`; `  --s subject : output`;
`  --nocc noccsegvol : name of aseg-type volume with out CC (default is aseg.auto_noCCseg) `;
`  --cc ccsegvol : output name of aseg-type volume with CC (default is aseg.auto) `; `  --normvol :
name of norm volume (default is norm)`; `  --lh-only, --rh-only`; `  --ctab ctab  : ctab for the seg
(will use embedded if there or FreeSurferColorLUT.txt if not spec)`; `  --force-update : force
regeneration of files whether needed or not (default is --no-force-update)`; an empty line. The
help adds `seg2cc 8.2.0` and four empty lines. Version: `seg2cc 8.2.0`.

## 8. Not reproduced (both modes alike, with a notice)

The shell's reading of names (FS-1277: the names as given), `--debug`'s trace (a line `larmorx:
seg2cc --debug: ...` says so), `sources.csh`'s macOS branch, `date`'s and `printf`'s texts in
locales other than C.

## 9. Validation plan

- The official script on synthetic subjects (32^3 volumes; one segmentation holding a callosum
  label makes mri_cc stop at once with status 77) in folders of their own, a stand-in
  `FREESURFER_HOME` (`sources.csh`, `build-stamp.txt`, a colour table) and the reference
  installation's `bin` first on `PATH`, `LC_ALL=C`, `TZ=UTC`, `HOME` the case's folder: every
  option, every failure above, the environment's variables; compared: status, both streams and
  every file written, with the folder, dates, fs_time's lines, run times, `uname -a`, host and user
  names, process numbers and `tmp.XXXXXX` names masked (fs_time's lines compared apart, their
  figures aside).
- recon-all's recorded steps (18 runs, conformed heads): the subject rebuilt before the step, the
  step run, `aseg.auto.mgz`, `transforms/cc_up.lta` and the log compared (FreeSurfer's provenance
  and the log's dates and times aside).
- The second path on real heads made not conformed (a slice taken off).
