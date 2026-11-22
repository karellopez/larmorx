# `mri_add_xform_to_header` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-10: the command line, the input and output paths, every message,
the exit statuses, the defects with their switches, and the validation plan, §6). Written by the
replica's author (track A, relay 50) as the spec writer: behaviour, conventions and messages, for
the clean room (`larmorx-fs-volumes`), which works from this file, the library modules' specs
(`specs/freesurfer-io.md` for reading and writing volumes, COR folders and `.xfm` files,
`freesurfer-mri.md` for the volume's transform fields, `freesurfer-core.md` for the version options,
diagnostics, the help renderer and error messages) and black-box runs of the official program only.

Conventions: `%d` and `%f` are C's formats (a `float` printed with `%f` is first widened to
`double`); "standard output" unless said otherwise. An **error** is FreeSurfer's error message
(`specs/freesurfer-core.md`: `error: ` and the text, with the system's message of `errno` as an
error first when `errno` is not 0, and FS-01's NUL byte before the newline); **`ErrorExit(code)`**
is that message and the end of the program with `code`'s exit status (`ERROR_BADPARM` 252,
`ERROR_NO_FILE` 255). A volume is a FreeSurfer volume (`specs/freesurfer-mri.md`), with its
transform name (`transform_fname`, room for 1,023 bytes), its transform (an MNI transform, empty in
a new volume), the transform's forward and inverse matrices (none in a new volume) and the flag
that frees the transform.

## 1. The command line

`mri_add_xform_to_header [options] <xfm> <in> [<out>]`.

1. The version options (`specs/freesurfer-core.md`, the version report: `--version` prints
   `mri_add_xform_to_header freesurfer 8.2.0`, `--all-info` its line); they are taken out of the
   arguments and the program goes on (with nothing else: the help, status 1).
2. The program's name in its messages is `mri_add_xform_to_header` (its first argument, as the
   scripts run it).
3. No argument left: the help (§1.2), status 1.
4. The options: while the next argument starts with `-`, it is an option (an empty argument is not
   one: it ends the options). The text after the dash compared without regard to ASCII case with
   `-help` and `-usage` (that is, `--help` and `--usage` in any case): the help, status 1.
   Otherwise its first letter after the dash, in any case (the rest is ignored: `-copy` is `-c`):
   - `c`: copy the name only: the transform file is not read (§2.2);
   - `v`: verbose, toggled at each (`-v -v` is off);
   - `n`: takes the next argument and ignores it (`-n` last: nothing is taken; the program then
     finds no transform name);
   - `?`, `h`, `u`: the help, status 1;
   - anything else (a dash alone, `--x`, `-x`): `unknown option <argument>` on standard error,
     status 1.
5. Then, counting the arguments left: none: `ErrorExit(ERROR_BADPARM)` with `mri_add_xform_to_header:
   no transform name specified`; one (the transform's name): `... no input name specified`; two:
   the output is the input; three or more: the third is the output, any after it ignored.

### 1.1 The help

`--help`, `-h`, `-?`, `-u` and no argument print the help text of the program's XML help
(`specs/freesurfer-core.md`, the help renderer) on standard output: the name, the synopsis
`mri_add_xform_to_header <options> xfmfile invol outvol`, the description `this program adds
specified xform to the volume header.`, the positional arguments `xfm file` and `input volume`,
the required flagged arguments' line of stars, the optional ones `-v` (`Be verbose`) and `-c` (`Do
not try to load the xfmfile, just copy name`), the output `output volume`, an example of stars,
`None` as bugs, the reporting line `Report bugs to <freesurfer@nmr.mgh.harvard.edu>` and a see-also
line of stars (the XML is the program's `mri_add_xform_to_header.help.xml`, rendered as every
FreeSurfer help is).

## 2. What it does

1. Verbose: `reading from <in>...` on standard error (no newline).
2. The input:
   - a folder (it opens for reading and is a directory): read as a COR folder's header without its
     slices (`MRIreadInfo`, `specs/freesurfer-io.md` §104: its `COR-.info`, warnings for
     non-standard values, its transform read when its `xform` line names a file that exists);
   - else a file that opens for reading: its extension is the text after the last `.` of the whole
     name, folders included (`a.b/c` has `b/c`; a name without a `.`, or with the `.` last, has
     none: `ErrorExit(ERROR_BADPARM)` with `mri_add_xform_to_header: no extension found`). Printed
     as `INFO: extension is <ext>` on standard output. `mgz` or `mgh` (exactly, lower case): the
     volume read whole (`MRIread`, `specs/freesurfer-io.md` §42: its transform read when its
     header names a file that exists, non-finite voxels made 0); any other:
     `ErrorExit(ERROR_BADPARM)` with `mri_add_xform_to_header: currently only .mgz or .mgh saves
     transform name`;
   - neither, or a read that gives no volume: `ErrorExit(ERROR_NO_FILE)` with
     `mri_add_xform_to_header: could not open source file <in>` (after a missing file, the system's
     message `No such file or directory` first).
3. Without `-c` the transform file is read into the volume's transform with FreeSurfer's `.xfm`
   reader (`specs/freesurfer-io.md` §45). The read assigns the transform as it goes: a file that
   fails after one or more transforms leaves those (concatenated) in the volume, one that fails
   before any (a missing file, a wrong header) leaves the volume's own transform as the input's
   read made it. A read that fails: its messages, then the error `mri_add_xform_to_header: could
   not read xform file '<xfm>'` (a newline in the text, so the NUL byte and a second newline
   follow); the program goes on. Then the transform's forward and inverse matrices are taken from
   whatever transform the volume holds (none: a volume whose transform was never read has none,
   and no message), and the flag that frees it set. `errno` after the read is what its own file
   operations leave: the code of the last existence check, open or read that failed (a missing
   file: `ENOENT`; a folder: `EISDIR` from its first read), unchanged when none failed.
4. The name: `<xfm>` as given becomes the volume's transform name (FS-1154 for names of 1,024 bytes
   or more).
5. Verbose: `done.` and a newline, then `writing to <out>...` on standard error.
6. The output:
   - a folder (it opens for reading and is a directory): its `COR-.info` rewritten from the
     volume's header (§3); the slices untouched;
   - any other name: the whole volume written (`MRIwrite`, `specs/freesurfer-io.md`: the format
     its name gives; an MGZ or MGH file keeps the transform name in its header tags; a volume read
     without its slices, a COR folder's, cannot be written as a file: FreeSurfer crashes, FS-58).
     A name whose format FreeSurfer does not know, a folder that does not exist: the writer's
     messages, and the program goes on (FS-1155).
7. Verbose: `done.` and a newline on standard error. Exit status 0.

## 3. `COR-.info` (`MRIwriteInfo`)

The file `<out>/COR-.info` is created (truncated); one that cannot be (`<out>/COR-.info` is itself
a folder, a folder without the right to write): the error `MRIwriteInfo(<out>): could not open
<out>/COR-.info.` (`errno` cleared before it, so no system message; a newline in the text), and
the program goes on (FS-1155). Its lines, each `<name> <value>` and a newline, in this order:

| line | value |
|---|---|
| `imnr0`, `imnr1` | the first and last slice numbers, `%d` |
| `ptype` | 2 for a coronal slice direction, 0 for an axial one, 1 otherwise (sagittal, or a volume whose RAS flag is 0) (`specs/freesurfer-volume.md`, the slice direction) |
| `x`, `y` | the width and height, `%d` |
| `fov` | the field of view divided by 1000 in `float`, `%f` |
| `thick` | the pixel size divided by 1000 in `float`, `%f` (FS-1157) |
| `psiz` | the pixel size divided by 1000 in `float`, `%f` |
| `locatn` | the location, `%f` |
| `strtx`, `endx`, `strty`, `endy`, `strtz`, `endz` | the volume's start and end per axis divided by 1000 in `float`, `%f` |
| `tr`, `te`, `ti` | `%f` |
| `xform <name>` | only when the volume has a forward transform matrix (§2.3) (FS-1158) |
| `ras_good_flag` | `%d` |
| `x_ras`, `y_ras`, `z_ras`, `c_ras` | three `%f` each: the column, row and slice direction cosines and the centre |

No `flip angle` line (the COR writer of `MRIwrite` writes one; this file has none). FS-1156 for an
output name of 1,014 bytes or more.

## 4. Exit statuses

0 when the program ends normally, whatever its writer did (FS-1155); 1 for the help (every way it
is asked), an unknown option, and a transform that is not linear (its matrices cannot be taken:
`get_linear_transform_ptr` on standard error; FreeSurfer's reader reads only linear ones, so no
file reaches it); 252 (`ERROR_BADPARM`) for no transform name, no input name, no extension and
an extension other than `mgz` or `mgh`; 255 (`ERROR_NO_FILE`) for an input that gives no volume;
134 for FS-1154 and FS-1156 with `reproduce`.

## 5. The defects (`larmorx_core::defects`)

| ID | FreeSurfer | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-1154 | a transform name of 1,024 bytes or more: the official build stops at once with `*** buffer overflow detected ***: terminated` on standard error (status 134 from the abort; nothing written; what standard output held is lost when it is not a terminal) | the whole name written | that line, status 134 |
| FS-1155 | an output that cannot be written (the writer's or `MRIwriteInfo`'s messages): status 0 | status 1 after the messages | status 0 |
| FS-1156 | `COR-.info`'s path (the output's name with `/COR-.info`) of 1,024 bytes or more: the abort of FS-1154, before the file is opened | the file written | that line, status 134 |
| FS-1157 | `thick` written from the pixel size | the slice thickness (`thick` of the volume, divided by 1000 in `float`) | the pixel size |
| FS-1158 | the `xform` line only with a forward transform matrix: with `-c` the name is lost unless the folder's own transform loaded; with a matrix and an empty name: `xform ` and nothing after it | the line whenever the name is not empty | FreeSurfer's rule |

The library's defects the program reaches: FS-01 (the NUL byte of error messages), FS-58 (a COR
folder's header written as a volume file: FreeSurfer crashes; larmorx ends with a message), FS-167
and FS-441 (memory contents FreeSurfer reads in damaged or frameless volumes).

## 6. Validation plan (for the clean room)

- **Black box.** The official `mri_add_xform_to_header` (`<workspace>/oracles/freesurfer-8.2.0/
  root/usr/local/freesurfer/8.2.0/bin`, `FREESURFER_HOME` and `FS_LICENSE` set,
  `FS_DISABLE_LANG=1` so that no machine-dependent `errno` prefixes its first error message, one
  thread) through the oracle-output cache, each case in a folder of its own, relative names, the
  folder's absolute name masked in standard output and error and in the files (`<DIR>`). Never set
  `DIAG_NO` (or `diag`): the licence check then prints the licence file. The program reads a
  format from the text after the last dot of a whole path: the cache's key keeps that text
  (`oracle_cache.policy.EXTENSION_FROM_WHOLE_PATH`), or two inputs of the same bytes under
  different names would share a key.
- **Cases.** The command line: no argument, `--version` (alone and before a call), the help in
  every spelling (`--help`, `--usage`, `--HELP`, `-h`, `-H`, `-?`, `-u`, `-U`, `-help`), unknown
  options (`-x`, `--x`, `-`), `-c`, `-C`, `-copy`, `-v` once and twice, `-n` with and without its
  argument, one to five positional arguments, an empty transform name. Inputs: MGZ and MGH volumes
  whose header names a transform that loads, one that is missing, one that does not read, one
  that fails after its first transform, none; the same bytes named `U.MGZ`, `noext`, `trail.` (where names may end in a dot: Windows drops a name's last dots),
  `a.b/c`, `v.nii.gz`; a NIfTI volume; a damaged MGZ; an MGH volume cut in its voxels; COR folders
  (4 x 4 x 3 bytes) with their transform beside them, without one and with slices 2 mm thick, with
  a missing one; `cor/`; empty folders; a missing name; an empty name. Transform files: a good
  linear one, `Invert_Flag = True`, two concatenated, one good then one of another type, a wrong
  header, a word for a number, a singular matrix inverted, a header only, an empty file, a name
  completed by `.xfm`, a missing one with and without extension, a folder, one in a folder.
  Outputs: `.mgz`, `.mgh`, `.nii`, `.nii.gz`, `.img`, an unknown format, a missing folder, an
  empty folder (with and without a trailing `/`), a folder whose `COR-.info` is a folder, a COR
  folder, the input itself. The environment: `FS_SKIP_TAGS=1`, `DIAG_VERBOSE=1`. A transform name
  of 1,104 bytes into an MGZ and an MGH file; an output folder 1,059 bytes deep (not where paths
  stop at 1,024 bytes, as on macOS). Then seeded random volumes (1 to 5 voxels a side, 1 to 3
  frames, four voxel types, every kind of axes, header tags) with zero to three options.
  Compare exit status, both streams and every file and folder after the run, case by case;
  `reproduce` must match everywhere but FS-1154 and FS-1156 (aborts), FS-58 (a crash), FS-167 and
  FS-441 (memory contents).
- **Recorded steps.** recon-all's two calls on each of the 12 recorded runs (`<workspace>/oracles/
  freesurfer-8.2.0/runs/*-{classic,v8}`: `-c <subject>/mri/transforms/talairach.xfm orig.mgz
  orig.mgz` and the same for `nu.mgz`), replayed on the subject folder as it was before each step:
  the written volume identical to the recorded one but for the subjects folder's path in the
  transform name.
