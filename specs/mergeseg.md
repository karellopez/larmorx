# `mergeseg` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-29). Written by the replica's author (track A, relay 58) as the
spec writer, for the clean room (`larmorx-fs-volumes`). The shell's behaviours (lines, logs,
`tee`, pipelines' status, undefined variables, `pushd`, `date`, `uname -a`, `cat`, `mkdir`, `rm`,
the help test, names read again, the end lines) are `specs/seg2cc.md` §2; **(observed)** marks
measurements on the official script.

## 1. What it is for

`mergeseg --src <seg> --merge <seg> --o <out>` merges one segmentation into another: the output is
the source where the merge volume is 0 (below 0.5), the merge volume's value elsewhere; with
`--segid <id>` the value `<id>` there; with `--segid-only <id>` only the merge volume's voxels equal
to `<id>` are merged (as `<id>`), eroded `--segid-erode <n>` times first. seg2cc runs it five times
for a segmentation not conformed to its smallest voxel (`specs/seg2cc.md` §5.7). It runs
`mri_binarize`, `mri_mask`, `fscalc` (`specs/fscalc.md`) and `mri_convert`.

## 2. The command line

No argument: the usage, status 1. Then the help test (`specs/seg2cc.md` §2: `-help` or `-version`
in any argument; the help: the usage, `mergeseg 8.2.0` and the text after it, §6). Then
`$FREESURFER_HOME/sources.csh` is sourced: `FREESURFER_HOME` unset: `FREESURFER_HOME: Undefined
variable.`; the file missing: `<FREESURFER_HOME>/sources.csh: No such file or directory.` (both
standard error, status 1). `FS_MRIMASK_ALLOW_DIFF_GEOM` is set to `0` for the programs it runs.

Options (a value missing: `ERROR: flag <flag> requires one argument`, status 1):

| Option | Effect |
|---|---|
| `--src <seg>` | the source; checked at once: `ERROR: cannot find <seg>`, status 1 |
| `--merge <seg>` | the merge volume; checked at once the same way |
| `--o <out>` | the output |
| `--segid <id>`, `--i <id>` | the value written where the merge volume is not 0 |
| `--segid-only <id>` | only the merge volume's voxels of value `<id>` (and `--segid <id>`) |
| `--segid-erode <n>` | with `--segid-only`, erode them `<n>` times (`mri_binarize --erode`) |
| `--ctab <ctab>` | a colour table embedded in the output (`mri_convert --ctab`) |
| `--log <file>` | the log, default `<out>.log`; `--nolog`, `--no-log`: `/dev/null` |
| `--tmp <dir>`, `--tmpdir <dir>` | the temporary folder, and no cleanup |
| `--nocleanup`, `--cleanup` | keep / remove the temporary folder (default remove) |
| `--debug` | the shell's trace of each command (not reproduced) |

Any other: `ERROR: Flag <flag> unrecognized.` and all the arguments, status 1. Then: no `--src`:
`ERROR: must spec src seg`; no `--merge`: `ERROR: must spec merge seg`; no `--o`: `ERROR: must spec
out seg` (status 1).

`--segid-erode`'s value is compared with 0 by the shell (only with `--segid-only`) (observed):
digits (a decimal number; a leading 0 is not octal; past 64 bits it wraps), or `-` and digits, are
numbers; a word starting with a digit and holding anything else: `if: Badly formed number.`; any
other word: `if: Expression Syntax.` (standard error, status 1, when the comparison runs). Above 0:
`--erode <n>` is passed as written.

## 3. The run

1. The output's folder: GNU `dirname` of `<out>`, `mkdir -p` it, its physical name.
2. The temporary folder: `--tmpdir`'s, else FreeSurfer's `fs_temp_dir --scratch --base <folder>`:
   a new folder `<folder>/tmp.XXXXXX` (six letters or digits, mode 0700), made in the output's
   folder; `mkdir -p` it.
3. The log: unless `/dev/null`, `rm -f` it; `Log file for mergeseg` `>>` it; through `tee -a`: the
   date, an empty line, `setenv SUBJECTS_DIR <SUBJECTS_DIR>` (unset: FS-1341), `cd <pwd>`, `<the
   script's full name, $FREESURFER_HOME/bin/mergeseg> <the arguments>`, an empty line,
   `build-stamp.txt`'s text, `mergeseg 8.2.0`, `uname -a`.
4. The steps, each: its command line through `tee`, then the program's standard output through
   `tee` (no `fs_time`); a failure: `ERROR:` on standard output, status 1:
   1. `mri_binarize --i <merge> --inv --o <tmp>/not.mergeseg.mgh` and `--match <id> [--erode <n>]`
      with `--segid-only`, else `--min 0.5`;
   2. `mri_mask <src> <tmp>/not.mergeseg.mgh <tmp>/srcsegnotmerge.mgh`; then, unless no cleanup,
      `rm <tmp>/not.mergeseg.mgh`;
   3. with a segid: `mri_binarize --i <merge> --o <tmp>/mergeseg.mgh --binval <id>` and the same
      `--match`/`--min` options; the merge volume is then `<tmp>/mergeseg.mgh`;
   4. `fscalc <merge volume> sum <tmp>/srcsegnotmerge.mgh -o <out>`;
   5. with `--ctab`: `mri_convert <out> --ctab <ctab> <out>`.
5. `rm -rf` the temporary folder unless no cleanup; the end lines (`Mergeseg-Run-Time-Sec`,
   `Mergeseg-Run-Time-Hours`, `mergeseg Done`); status 0.

## 4. The defects (`reproduce`: FreeSurfer's)

- **FS-1341**: `SUBJECTS_DIR` unset: `SUBJECTS_DIR: Undefined variable.` at step 3 (after the
  temporary folder is made), status 1. By default the line `setenv SUBJECTS_DIR ` and the merge.
- **FS-1274**, **FS-1277** (also the output folder's physical name), **FS-1278**
  (`specs/seg2cc.md`).

## 5. Not reproduced

As `specs/seg2cc.md` §8.

## 6. The usage

An empty line; `mergeseg --help`; ` --src source segmentation`; ` --merge merge seg volume ` (a
blank at the end); ` --o outseg`; ` --segid segindex (optional)`; ` --segid-only segindex : only
take segindex from merge`; ` --segid-erode Nerode : erode seg-only segindex before merge`; ` --ctab
ctab : color table to embed in the output`; an empty line. The help then prints `mergeseg 8.2.0`,
an empty line, and: `Merges one segmentation into another. The output is the same as the source` /
`except in voxels where the merge seg is non-zero in which case those voxels` / `are replaced with the values in the merge segmentation volume. ` / an empty line / `If you
specify the segindex, then all the voxels in the merge seg will be set` / `to segindex.` / an empty
line.

## 7. Validation plan

As `specs/seg2cc.md` §9: 32^3 sources (integers) and merge volumes (bytes; floats holding NaNs and
infinities), a volume of another extent (mri_mask's error), an empty file, every option and failure,
`SUBJECTS_DIR` and `FREESURFER_HOME` unset; the output's voxels, extent, type, geometry and embedded
colour table compared, and the log.
