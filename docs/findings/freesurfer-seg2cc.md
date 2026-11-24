# FreeSurfer 8.2.0's `seg2cc`, `mergeseg` and `fscalc`: findings of the replica (queue items 3.2-28, 3.2-29, 3.5-29)

**Restricted** (source-derived): closed to FreeSurfer clean-room implementers, who work from
`specs/seg2cc.md`, `specs/mergeseg.md` and `specs/fscalc.md`. Track A, relay 58 (2026-11-24). The
replica: `crates-freesurfer/larmorx-freesurfer-volumes/src/seg2cc/`, `src/mergeseg/`,
`src/fscalc/` on the shell model `src/script.rs` (with `src/csh.rs`'s word rules); the probe:
`validation/probes/fs-scripts/`.

## The scripts

`scripts/seg2cc` (362 lines), `scripts/mergeseg` (303) and `scripts/fscalc` (342) are tcsh scripts
installed with `@FS_VERSION@` replaced by `8.2.0`. They run FreeSurfer's programs by name
(`mri_info`, `mri_cc`, `mri_convert`, `mri_label2vol`, `mri_binarize`, `mri_mask`, `mris_calc`,
`mergeseg`, `fscalc`, `getfullpath`), FreeSurfer's helpers `UpdateNeeded` (csh: `test <source> -nt
<target>`), `fsr-checkxopts` (tcsh: `grep -v \# | awk '{print $1}' | sort [| uniq] | wc -l`),
`fs_temp_dir` (C++: `FS_TMPDIR=/scratch` without overwriting, then `--base` overwriting it, then
`makeTempDir()`), `fs_time` (tcsh around GNU `/usr/bin/time -o /dev/stdout -f <format>`, `uptime |
sed 's/,/ /g'`), `fname2ext` (csh, fscalc's), and the system's `date`, `uname -a`, `pwd`, `mkdir`,
`rm`, `cat`, `head`, `bc -l`, `printf`, `cut`. The port runs the ported programs in its own
process (`Sh::run`, the crate's `PROGRAMS`) and reproduces what the helpers and system programs
print and decide (`src/script.rs`), which `docs/freesurfer/programs.md` §3 planned for recon-all's
helpers. fscalc lives in the volumes crate (queue item 3.5-29 placed it in `larmorx-freesurfer-cortex`):
mergeseg runs it, and the volumes crate cannot depend on the cortex crate.

recon-all runs `seg2cc --s <subject>` in both streams; every recorded head is conformed (256^3 at
1 mm), so only the first path (`mri_cc -aseg ...`) is recorded: mergeseg and fscalc never run in
the recordings (fscalc does in label-cortex's `--fix-ga`, v8).

## Measured facts of tcsh 6.21.00 and the system's programs

- `pushd`/`popd` print the stack (current folder first, every entry followed by a blank, `$HOME`
  as `~`); tcsh's folder starts as `PWD` when it names the same folder, else the physical name; a
  relative `pushd` name is joined and `..` removed as text (links kept).
- `if($v == yes)` with `$v` empty is false without an error; with two words `if: Expression
  Syntax.`. `if($v > 0)`: `x` and `+2` `Expression Syntax.`, `1.5`, `3a`, `0x1` `Badly formed
  number.`, `08` and `010` positive (no octal), `9999999999999999999` wraps negative.
- A pipeline's `$status` is the last failing command's (`anyerror`): `false | true` gives 1.
- `$0` is the script's absolute name (a relative invocation is made absolute).
- An undefined variable inside double quotes (`echo "ERROR: $cmd"`, `echo "setenv SUBJECTS_DIR
  $SUBJECTS_DIR"`) ends the script: `<name>: Undefined variable.`, status 1.
- GNU `time` prints `Command exited with non-zero status <n>` on its output (`-o /dev/stdout`)
  before the format line when the command fails.
- `printf %5.2f` of `bc -l`'s `.00500000000000000000` (18 s) gives `0.01` in the C locale (the long
  double above 0.005); the recordings' es_ES time locale gave `0,00` (an invalid number for that
  locale's decimal comma).
- `date` in the recordings is es_ES's (`jue 12 nov 2026 12:33:22 CET`): the port prints the C
  locale's form (the logs' dates are masked by the replay tool's comparison).

## The defects (rules A to D; `larmorx_core::defects`)

1. **FS-1274** (class e; rule A). `set n = `echo $argv | grep -e -help | wc -l``: any argument
   holding `-help` prints the usage and help and exits 1 (`seg2cc --s sub-helper`); `-version`
   prints the version and exits 0 without doing anything. The default asks only `-help`/`--help`
   (`-version`/`--version`).
2. **FS-1275** (class a-like failure, an option that always fails; rule A). `--expert`: after
   `fsr-checkxopts $XOptsFile; if($status) goto error_exit;` (`error_exit` echoes `"ERROR: $cmd"`,
   and `cmd` is not set during the parsing: `cmd: Undefined variable.`), `set XOptsFiles =
   ($XOptsFiles ...)` reads an unset variable: `XOptsFiles: Undefined variable.`. The file is not
   used ("# Not used"). Default: the check, and on.
3. **FS-1276** (class c; rule A). `--ctab` sets `ctab`, which nothing reads; the merge gets
   `$ctabdefault`. Default: `--ctab`'s table.
4. **FS-1277** (class e; rule A, D13 gap). No variable is quoted. Not reproduced: both modes take
   the names as given, with a notice (the probe counts these cases as not reproduced).
5. **FS-1278** (class e; rule A). Lines through tcsh's `echo` (`echo_style` both).
6. **FS-1279** (class c; rule A). `pushd $mdir` comes after `--log`/`--tmpdir`'s names and before
   their later uses (`tee -a $LF`, `$tmpdir/isconformed`): a relative name is read from two
   folders. With `--tmp t2` the official answer of `mri_info --o t2/isconformed` is written into
   `<subject>/mri/t2/`, which does not exist: no answer, and the second path runs on a conformed
   head (measured, probe case `--s s --tmp t2`).
7. **FS-1280** (class c; rule A). `mri_cc ... <subject>` inside `<subject>/mri` reads
   `$SUBJECTS_DIR/<subject>/mri/...` relative to that folder.
8. **FS-1341** (class c; rule A). mergeseg echoes `"setenv SUBJECTS_DIR $SUBJECTS_DIR"` and never
   uses the variable otherwise.

Kept (rule C, no ID): mergeseg's `error_exit` prints only `ERROR:`; fscalc's default log lives in
its temporary folder and is removed with it; fsr-checkxopts counts blank lines as a repeated empty
command (two blank lines fail the check); the usage's claim that `--ctab`'s default is the
segmentation's embedded table (the script never does that; the default path's table stays
`FreeSurferColorLUT.txt`).

## Seen through seg2cc: FS-1053

seg2cc's second path runs `mri_convert <norm> <n>.conf.mgz --conform_min`, which prints the new
geometry with a validity flag FreeSurfer never sets (FS-1053, mri_convert's: memory contents; the
port prints 0 with the notice). The probe masks the line `valid   : <n>`.

## A finding of this relay: a short text named `.mgz` in the default mode (not yet corrected)

`mri_binarize --i text.mgz ...` (and every reader of a volume) with a 13-byte text file named
`.mgz`: zlib reads it as plain data, the MGH header's integers are the text's bytes (width
1,629,517,423, height 1,819,635,045), the depth is cut (FS-167: 0 by default, memory contents
with `reproduce`), the frames 0 by default. With `reproduce` the frame count from memory contents
is refused (FS-760, status 254, as the official program). By default the frames are 0, the voxels'
`calloc` is of 0 bytes, and the index tables (`(extent + 1000) * 4` bytes each, filled) take about
14.5 GB: the 2 GiB box killed the run (status 137), and an in-process run of the probe's cases
reached the 8 GiB box. FreeSurfer with those values would fill the same tables (machine-dependent,
as FS-760). Not corrected in this relay: the probe's unreadable volumes are empty files (the header's
first read fails in both modes). The correction (next relay): a header the file cuts before its
type (rule B, `mghRead(<name>, <frame>): read error`) and index tables past the machine's memory
refused as FS-760's frames.

## Speed and memory

The recorded steps (`mri_cc`'s path): the port 17.6-19.1 s against FreeSurfer's recorded 14.4-27.1
s (one thread). The port runs the programs in its own process: its peak is the largest program's
plus the shell's; FreeSurfer's tcsh tree's peak is its largest program's.
