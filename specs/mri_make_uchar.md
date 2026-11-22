# `mri_make_uchar` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-22: the command line, what the program computes and prints,
every message, the exit statuses, the defects with their switches, and the validation plan, §7).
Written by the replica's author (track D, relay 51) as the spec writer: behaviour, conventions
and messages, for the clean room (`larmorx-fs-volumes`), which works from this file, the library
modules' specs (`specs/freesurfer-io.md` for reading and writing volumes, `freesurfer-volume.md`
for the volume functions and linear transform arrays, `freesurfer-maths.md` for matrices and
histograms, `freesurfer-mri.md` for voxels and geometry, `freesurfer-core.md` for the version
options, diagnostics and error messages) and black-box runs of the official program only.

Conventions: `%d`, `%ld`, `%f`, `%lf`, `%g` and `%2.0f` are C's formats; "standard output" unless
said otherwise. An **error** is FreeSurfer's error message (`specs/freesurfer-core.md`: `error: `
and the text, with the system's message of `errno` as an error first when `errno` is not 0, and
FS-01's NUL byte before the newline); `errno` is never cleared by the program. **`ErrorExit(code)`**
is that message and the end of the program with `code`'s exit status (`ERROR_NO_FILE` 255).
Arithmetic on `double` and `float` is IEEE round to nearest without fused operations; where an
operation's result is a NaN, the NaN passed on is SSE's (`specs/freesurfer-maths.md`: a NaN made
from numbers is the default NaN with the sign bit set; a NaN operand is passed on quieted, of two
the first operand's, in the operand order given here).

## 1. The command line

`mri_make_uchar [options] <input volume> <talairach xform> <output volume>`.

1. The version options (`specs/freesurfer-core.md`, the version report: `--version` prints
   `mri_make_uchar freesurfer 8.2.0`, `--all-info` its line) are taken out of the arguments; when
   one was given and nothing else is left, status 0; otherwise the program goes on.
2. The program's name in its messages is `mri_make_uchar` (its first argument, as
   `mri_nu_correct.mni` runs it).
3. The options: while at least one argument is left and the next one starts with `-`, it is an
   option, chosen by the character after the dash in any ASCII case (the rest is ignored: `-help`
   is `-h`, `-verbose` is `-v`):
   - `n`: nothing;
   - `u`, `?`: the usage (§1.1), status 0;
   - `f`, `w`, `r`: the next argument scanned as C's `sscanf("%lf")` (leading white space skipped;
     decimal, hexadecimal, `inf`, `infinity`, `nan`, `nan(...)` in any case, with a sign; the
     longest number at its start: `0.5x` is 0.5) into the first percentile (default 0.01), the
     white-matter percentile (default 0.9) or the ball's radius in mm (default 50); a text that
     does not start with a number (`abc`, the empty text) leaves the setting as it was. The next
     argument is taken either way. Given last (no next argument): FS-1128;
   - `h`: the next argument is the cumulative histogram's file name (given last: no file, and the
     argument count then ends the options);
   - `v`: the next argument is the ball volume's file name (likewise);
   - anything else (a dash alone, `-x`, `--foo`, `-1`): `unknown option <argument>` on standard
     error, status 1.
   A later option of the same letter replaces the earlier setting.
4. Fewer than two arguments left: the usage, status 1. Two left (no output name): FS-1127. Three
   or more: input, transform, output; any after them ignored.

### 1.1 The usage

On standard output, the defaults printed as the options left them (`%g`):

```
Uses the Tal xform to find a ball of voxels that are mostly brain.
The top of the intensity histogram in this ball will then be white
matter, which allows us to center it at the desired value,
approximately (110).

usage: mri_make_uchar [options] <input volume> <talairach xform> <output volume>
-f FIRST_PERCENTILE (default <first>)
-w WM_PERCENTILE (default <wm>)
-r MAX_R (default <radius>)
-h cumulative histo file
-v vradvol : volume with everthing outside of MAX_R set to 0
```

## 2. What it does

1. Standard output: `FIRST_PERCENTILE <first>`, `WM_PERCENTILE    <wm>` (four spaces) and
   `MAX_R <radius>`, each `%lf` on a line.
2. The input volume read (`MRIread`, `specs/freesurfer-io.md` §42: every format; non-finite voxels
   of a `float` volume made 0 with their `NaN found` lines; a transform its header names loaded
   when the file exists). No volume: `ErrorExit(ERROR_NO_FILE)` with `mri_make_uchar: could not
   read input volume <input>`.
3. The transform read (`LTAread`, `specs/freesurfer-volume.md` §21: an `.xfm` as one scanner-to-
   scanner transform of centre (0, 0, 0) and spread 1, an `.lta` as written, any type kept as
   read). None: `ErrorExit(ERROR_NO_FILE)` with `mri_make_uchar: could not read <transform>`.
4. **The ball.** The output: a `uchar` volume of the input's extent, frames and header
   (`MRIcloneDifferentType`, `specs/freesurfer-volume.md`), all zeros. For each voxel `(x, y, z)`
   of the input's extent, `x` outermost, `z` innermost:
   1. When the input's slice direction is not coronal (`specs/freesurfer-volume.md` §14: axial,
      sagittal, or none), the error `MRIvoxelToTalairachVoxel: unsupported slice direction <d>`
      (`<d>` 1 sagittal, 2 axial, -1 none; FS-1130). Nothing else of that call is used.
   2. The voxel's scanner RAS (`MRIvoxelToWorld`, `specs/freesurfer-mri.md`: `float` arithmetic,
      widened), stored as three `float`s `p = (px, py, pz)`.
   3. The transform's matrix at `p` (`LTAtransformAtPoint`, `specs/freesurfer-volume.md` §63: the
      weights with the C library's `exp`; an `exp` whose finite exponent gives 0 or an infinity
      sets `errno` to `ERANGE`, **(observed)** of glibc 2.35: for an `.xfm`'s spread of 1, every
      point farther than about 38.6 mm from the origin), then that matrix times `(px, py, pz, 1)`
      as `MatrixMultiply` (`specs/freesurfer-maths.md`: `float`) gives `q`; `r = sqrt((qx qx + qy
      qy) + qz qz)` with each `q` widened to `double`.
   4. `r < radius` (false for a NaN): the output voxel takes the input's voxel of frame 0
      (`MRIgetVoxVal`, then `MRIsetVoxVal` into `uchar`: clamped to 0 and 255, rounded with `nint`;
      `specs/freesurfer-mri.md` §5).
   With an input that is not coronal, the error of step 1 at the first voxel shows the `errno`
   the reads left (often 0, then no system message), and from the next voxel on `Numerical
   result out of range` once any earlier voxel's weight underflowed.
5. `-v`: the ball volume written to its name (`MRIwrite`, `specs/freesurfer-io.md` §84: the format
   its name gives; a name it cannot write: the writer's messages, FS-1129).
6. **The histogram.** `MRIhistogram(output, 100)` (`specs/freesurfer-volume.md`: every frame, 100
   bins between the volume's minimum and maximum, a maximum equal to the minimum becoming 255);
   unless its first bin's value is above the bin size, the count of the first bin whose value is
   at least 0 cleared (`HISTOclearZeroBin`); made a probability density in place (`HISTOmakePDF`); its cumulative
   distribution, a new histogram (`HISTOmakeCDF`: a sum of 0 makes every count a NaN). `-h`: the
   cumulative histogram written to its name as text (`HISTOwriteTxt`, `specs/freesurfer-maths.md`:
   `%4d %10.5f %10.5f` of each bin's number, value and count); a file it cannot open:
   `ERROR: HISTOwrite(): opening <name>` on standard output (FS-1129).
7. `i1` and `i2`: the first bin of the cumulative histogram whose count is nearest the first
   percentile and the white-matter percentile, each as a `float` (`HISTOfindBinWithCount`: NaN
   counts are never nearer, so bin 0). Standard output: `i1 = <i1>, i2 = <i2>`.
8. **The line.** `x1` and `x2` the values of bins `i1` and `i2` (widened); `y1 = 255 * first`;
   `y2 = 110`; `m = (y2 - y1) / (x2 - x1)`; `b = y2 - x2 * m` (in that operand order). Bins of one
   value (`x1 = x2`) or a NaN percentile give a slope that is not finite (FS-1131).
9. **The mapping.** For each voxel, `x` outermost, `z` innermost: `v = (input voxel of frame 0,
   widened) * m + b`; `v < 0`: 0, counted in `nzero`; else `v > 255`: 255, counted in `nsat`; else
   `v` as it is (a NaN neither, not counted); stored as a `float` through `MRIsetVoxVal` into the
   output's frame 0 (a NaN becomes `nint`'s -2147483648, so the byte 0: FS-12). Other frames keep
   the ball's zeros.
10. Standard output: `#mri_make_uchar# mapping %2.0f %2.0f to %2.0f %2.0f  :  b %g m %g : thresh %g
    maxsat %g : nzero %ld nsat %ld` with `x1, x2, y1, y2, b, m, (-b) / m, (255 - b) / m, nzero,
    nsat` (`-b` the sign flipped, a NaN's too).
11. The output written (`MRIwrite`, the format its name gives; a name it cannot write: FS-1129).
12. Standard error: `type change took <m> minutes and <s> seconds.` (the time since the program
    started, after its version options: `nint` of the milliseconds / 1000 as `float`, whole
    seconds; minutes and the rest). Status 0.

Standard output is a file's or a pipe's buffer (4,096 bytes) until the end; standard error is
written at once, so in a combined log the timing line comes before the program's lines.

## 3. Exit statuses

0 when the program ends normally, whatever its writers did (FS-1129) and whatever its line
(FS-1131); 0 for `-u` and `-?`; 1 for no or one argument, an unknown option; 255
(`ERROR_NO_FILE`) for an input or a transform that does not read; 139 (signal 11) for FS-1127 and
FS-1128 with `reproduce`.

## 4. The defects (`larmorx_core::defects`)

| ID | FreeSurfer | larmorx by default | `reproduce` |
|---|---|---|---|
| FS-1127 | two arguments: the whole computation (its lines, the `-h` and `-v` files), then a crash writing to no name (signal 11; standard output, held in its buffer, lost unless more than 4,096 bytes were printed) | the usage, status 1, before anything is read | the crash, status 139, standard output cut as FreeSurfer's |
| FS-1128 | `-f`, `-w` or `-r` given last: a crash at once (signal 11, nothing printed but what the version options printed) | the usage, status 1 (as `-h` and `-v` given last) | status 139 |
| FS-1129 | an output, `-v`'s volume or `-h`'s file that cannot be written: status 0 | status 1 after the writer's messages (no timing line) | status 0 |
| FS-1130 | an input that is not coronal: the error of §2.4.1 for every voxel (`errno`'s message first), although nothing is computed from it | no such error | every error |
| FS-1131 | a line that is not finite (§2.8): the mapping line with `inf` and `nan`, a volume of zeros, status 0 | after the `i1` line, `mri_make_uchar: no finite mapping to uchar (bins <i1> and <i2>: <x1> and <x2>; slope <m>): nothing written` on standard error (`%g`), status 1, no output | FreeSurfer's |

The library's defects the program reaches: FS-01 (the NUL byte of error messages), FS-12 (`nint` of
a NaN), FS-308 and FS-504 (a file that is no transform array read as one: memory contents), FS-841
(the `NaN found` lines of a read on several threads), GLIBC-07 (`exp`'s last bits; no output
changes, the weights of one transform being normalised).

## 5. Speed and memory

The port reads and writes as FreeSurfer does; its peak memory is the input, the output and the
reads' buffers (85 MiB for a 256^3 `float` input, FreeSurfer 599 MiB **(observed)**).

## 6. What `mri_nu_correct.mni` runs

`mri_make_uchar nu.mgz transforms/talairach.xfm nu.mgz` (in place, in the subject's `mri` folder),
on the `float` volume its N3 steps and rescale made, as its last step when called with `--uchar`.

## 7. Validation plan (for the clean room)

- **Black box.** The official `mri_make_uchar` (`<workspace>/oracles/freesurfer-8.2.0/root/usr/
  local/freesurfer/8.2.0/bin`, `FREESURFER_HOME` and `FS_LICENSE` set, `FS_DISABLE_LANG=1`, one
  thread) through the oracle-output cache, each case in a folder of its own, the folder's
  absolute name masked in the messages, the timing line's figures masked. Never set `DIAG_NO`
  (or `diag`): the licence check then prints the licence file. The program takes a file's format
  from the text after the last dot of a whole name: the cache's key keeps that text
  (`oracle_cache.policy.EXTENSION_FROM_WHOLE_PATH`).
- **The open probe** `validation/investigations/freesurfer-mri-make-uchar-probe/compare.py` (its
  README: 103 command lines on seeded inputs: every option and value form, the files, outputs,
  input types and shapes, coronal and other slice directions, transforms near the identity, far
  away, with a NaN, cut short, linear transform arrays of both types, the failures). Compare exit
  status, both streams and every file written; `reproduce` must match everywhere but the cases
  that read a volume as a transform (FS-308, FS-504: memory contents).
- **Golden cases** (`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_make_uchar_golden.rs`'s
  inputs): seven cases on 32^3 and 8^3 volumes; the official value is in the replica's test.
- **Real calls.** `mri_nu_correct.mni`'s call on the recorded heads (`<workspace>/oracles/
  freesurfer-8.2.0/runs/*-{classic,v8}`) with stand-ins (the `float` input is not kept): in place
  on the run's `orig.mgz` times the run's recorded rescale ratio (the `mris_calc ... mul <ratio>`
  line of its `recon-all.log`) as `float`, with the run's `transforms/talairach.xfm`; on the run's
  `orig.mgz`; on the run's `T1.mgz` with `-h`, `-v` and `-r 60`. Each output identical to the
  official program's (FreeSurfer's provenance aside).
