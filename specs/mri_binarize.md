# `mri_binarize` (FreeSurfer 8.2.0): behaviour spec

Status: complete (queue item 3.2-19: the options, the three stand-alone options, the program's
order of steps, its outputs and exit statuses, the volume tessellation `--surf` and
`--fix-vol-topo` use, and the validation plan with the open probe, §6). Written by the replica's
author (track D) as the spec writer: behaviour, maths, conventions and messages, for the clean
room (`larmorx-fs-volumes`, queue item C3.2-13), which works from this file, the library modules'
specs (`specs/freesurfer-*.md`) and black-box runs of the official program only.

Conventions: volumes are FreeSurfer volumes (`specs/freesurfer-mri.md`,
`specs/freesurfer-volume.md`); a voxel's value is read as a `float` (the volume's type converted
as `specs/freesurfer-mri.md` reads it) and written from a `float` (converted to the volume's type
as it writes it); voxels are visited with the column index outermost and the slice index
innermost unless said otherwise. Messages go to standard output unless "standard error" is said;
`%g`, `%lf` and their kin are C's formats. Numbers on the command line are read as C's `sscanf`
reads them (`%d`, `%lf`): a number that cannot be read leaves the setting as it was (`nan` and
`inf` are numbers for `%lf`).

## 1. The command line

`mri_binarize [options]`: the version options first (`specs/freesurfer-core.md`, the version
report: `--version` and `--all-info` print their report; with only version options the program
ends with status 0); then, with no argument left, the usage (§1.4) and status 1. Every other
argument is an option, compared without case, read with the arguments after it; an argument that
is no option prints `ERROR: Option <arg> unknown` on standard error (and, for an argument of two
or more characters starting with one dash not followed by another, `       Did you really mean
-<arg> ?`) and ends with status 255. An option short of its arguments prints `ERROR: <option> flag
needs <n> argument` (`arguments` for more than one) on standard error and ends with status 255.

### 1.1 The options

| Option | Arguments | Sets |
|---|---|---|
| `--help` | | the usage and the help (§1.4), status 1 |
| `--version` | | (handled first by the version options) |
| `--debug` | | every later option printed as `<n> <option>` (n: the arguments left, the option included) |
| `--noverbose` | | no report, no progress lines (§3) |
| `--checkopts`, `--nocheckopts` | | end after the checks with status 0 / not |
| `--i`, `--o`, `--surf`, `--merge`, `--copy`, `--mask`, `--count` | name | the input, output volume, output surface, merge volume, copy volume, mask, count file (`--count` also turns counting on) |
| `--surf-smooth` | n | the surface's smoothing passes |
| `--mask-thresh` | t | the mask threshold (0.5) |
| `--min`, `--max` | t | the threshold (and that it is set) |
| `--rmin`, `--rmax` | r | the threshold as `r` times the input's mean (§3) |
| `--pct` | p | the minimum from the top `p` % (and the minimum set) |
| `--fdr` | q | the threshold of a false discovery rate (§3) |
| `--fdr-pos`, `--fdr-neg`, `--fdr-abs` | | its sign: +1, -1, 0 (the default) |
| `--threads`, `--nthreads` | n | FreeSurfer's thread count (1 if unread; larmorx computes as one thread) |
| `--abs`, `--neg` | | the input's absolute value / negated before thresholding |
| `--bincol` | | the binarised voxels replaced by their column number (§3) |
| `--uchar` | | the output as `uchar` (else `int`) |
| `--no-count` | | no second count (§3) |
| `--reverse` | | the surface's faces reversed |
| `--zero-edges` | | the first and last column, row and slice planes get the "not" value |
| `--zero-slice-edges` | | the first and last slice planes only |
| `--fill-holes`, `--no-fill-holes`, `--remove-islands`, `--no-remove-islands`, `--fix-vol-topo`, `--no-fix-vol-topo`, `--erode-edges`, `--no-erode-edges` | | on / off |
| `--match` | v ... | match values: every following argument that does not start with `--` (a value that cannot be read is 0) |
| `--ctx-wm`, `--wm` | | adds 2 41 77 251 252 253 254 255 |
| `--all-wm` | | adds those and 7 46 |
| `--ventricles` | | adds 4 5 14 43 44 72 31 63 |
| `--wm+vcsf` | | adds 2 41 77 251 252 253 254 255 7 46 4 5 14 43 44 72 31 63 |
| `--gm` | | adds 2 41 77 251 252 253 254 255 7 46 4 5 14 43 44 15 72 31 63 0 24, and sets the values to 0 and 1 (as `--inv`) |
| `--subcort-gm` | | adds 10 11 12 13 17 18 26 28 27 8 49 50 51 52 53 54 58 60 59 47 |
| `--scm-lh` | | adds 26 11 2 4 13 12 10 28 30 34 78, islands removed and holes filled |
| `--scm-rh` | | adds 58 50 41 43 52 51 49 60 62 66 79, islands removed and holes filled |
| `--match-ctab` | ctab [x ...] | adds every valid entry's index of the colour table (read as `specs/freesurfer-io.md`'s ASCII colour tables; unread: status 1) except the values following it |
| `--ctab` | ctab | the colour table embedded in the output (unread: none, no error) |
| `--copy-ctab` | | the input's colour table copied into the output |
| `--replace`, `--replaceonly` | v1 v2 | a replacement (unread numbers 0); `--replaceonly` also replaces only (§3) |
| `--replace-nn`, `--replaceonly-nn` | v w | a nearest-neighbour replacement within a window of `w` voxels; `--replaceonly-nn` also replaces only |
| `--no-transitive-replace` | | replacements applied to the input at once (§3) |
| `--binval`, `--binvalnot` | n | the value in range (1) and out of it (0) |
| `--inv` | | the values 0 and 1 |
| `--frame` | f | that frame only (and no loop over the frames) |
| `--frame-sum` | | the frames summed into frame 0 (no loop) |
| `--frame-and` | | as `--frame-sum`, the minimum `nframes - 0.5` (and set) |
| `--dilate` | n | 3-D dilations |
| `--erode-face`, `--erode-edge`, `--erode-corner` (`--erode`) | n | erosions by face, edge or corner neighbours |
| `--erode2d` | n | 2-D erosions |
| `--bb`, `--crop` | npad | the output cropped to the bounding box of its nonzero voxels, padded |

FS-1023: FreeSurfer keeps at most 1,000 match values, 1,000 replacements and 1,000
nearest-neighbour replacements; more are written over its other settings (other options change,
or it crashes: `--match-ctab` with FreeSurfer's whole `FreeSurferColorLUT.txt` crashes). larmorx
keeps every entry in both modes and notes FS-1023 (not reproduced: a write past a list).

### 1.2 The stand-alone options

They run where they stand in the command line (the options before them read, those after not)
and end the program with the status of their write (0 when written):

- `--ohe seg ohe id ...` (three arguments or more): the segmentation read (unread: status 1), every
  argument after the two names read as C's `atoi`, the one-hot encoding (`specs/freesurfer-volume.md`'s
  `MRIoneHotEncode`, forced) written to `ohe`.
- `--crop-around-ras out in lta|nolta rc ac sc fr fa fs [keep ...]` (nine arguments or more): `in`
  read; `lta` read as a linear transform unless it is `nolta` (unread: status 1); the centre: with
  `rc` `cras`, `Getting cras from <ac>` and that volume's centre (`c_r`, `c_a`, `c_s`; `sc` ignored),
  else the three numbers; the field of view three integers; the values to keep every following
  argument that does not start with `--`; `MRIcropAroundRAS` (`specs/freesurfer-volume.md`) written
  to `out` (none: status 1).
- `--dilate-vertex-sum vno surf measure|nofile|area target outmask` (fewer than five: `USAGE:
  --dilate-vertex-sum vno surf measure target outmask`, status 1) and `--dilate-vertex vno surf
  radius outmask` (fewer than four: the usage line, status 1; FS-1024: FreeSurfer prints
  `USAGE: --dilate-vertex-sum vno surf radius outmask`, larmorx `--dilate-vertex`): the surface read
  (`specs/freesurfer-surfio.md`, unread: status 1), the measure read unless `nofile` or `area`
  (status 1 unread), the target `pi * r * r` (`double`, `pi` times `r` first) or the number;
  `MRISdilateVertexToSum`'s mask (`specs/freesurfer-surfio.md`) written (none: status 1).

FS-1026: a number of these three options (and of `--match-ctab`'s excluded values) that cannot be
read is memory contents in FreeSurfer; larmorx ends with `mri_binarize: <option>: <arg> is not a
number (FreeSurfer takes memory contents here)` on standard error, status 1 (`reproduce`: 0).

### 1.3 The checks

In this order, each `ERROR: ...` on standard output with status 1: `must specify input volume`;
`must specify output volume or output count file` (none of `--o`, `--count`, `--surf`); no
threshold (`--min`, `--max`, `--rmin`, `--rmax`, `--pct`, `--frame-and`), no match values and no
`--fdr`: with replacements, replace only, else `must specify minimum and/or maximum threshold or
match values`; a threshold with match values: `cannot specify threshold and match values`;
`--min` with `--rmin`: `cannot --rmin and --min ` (a blank at the end), likewise `cannot --rmax and
--max `; without match values, both set and the maximum below the minimum: `max thresh = %g < min
thresh = %g`.

### 1.4 The help

The usage: `USAGE: <program> ` (the name as run, a blank), the usage text, `8.2.0`, an empty
line; `--help` adds the help text. Both texts are FreeSurfer's (`larmorx-freesurfer`'s are the same
bytes; run the official program's `--help` for them).

## 2. The report

Unless `--noverbose`, after the checks: an empty line; `8.2.0`; `cwd <folder>` (the working
folder; empty for a path of 2,000 bytes or more); `cmdline <arguments>` (the program's name and
every argument left after the version options, each followed by a blank); `sysname  <s>`,
`hostname <h>`, `machine  <m>` (`uname`'s); `user     <name>` (the user's name, else `UNKNOWN`);
an empty line; `input      <name>`, `frame      <f>`, `nErode3d   0`, `nErode2d   <n>`, `output
<name>` (`(null)` without one); without match values `Binarizing based on threshold`, `min
<%g>` (or `rmin       <%g>`, or `min        -infinity`), `max        <%g>` (or `rmax       <%g>`,
or `max        +infinity`); with match values `Binarizing based on matching values`, `nMatch <n>`
and each as `%2d  %4d` (its index, its value); `binval        <n>`, `binvalnot     <n>`; with a merge
volume `merge      <name>`; with a mask `mask       <name>` and `maskthresh <%lf>`.

## 3. The program

The steps in order. A volume is read as `specs/freesurfer-io.md` reads one (its not-a-number
voxels made 0); unread: its message, status 1.

1. The input read; a frame at or past its frames: `ERROR: requested frame=<f> >= nframes=<n>`,
   status 1 (a negative frame passes: its voxels are memory FreeSurfer does not own, FS-53).
2. The mask: `Loading mask <name>`, read; its width, height or depth not the input's: `ERROR:
   dimension mismatch between input and mask volumes`, status 1.
3. `--fdr`: the voxel-wise threshold of the false discovery rate `q` over the input's frame inside
   the mask (above 0.5), the values taken as `-log10(p)`, of the sign asked for
   (`specs/freesurfer-volume.md`'s `MRIfdr2vwth`, its debug lines printed); `FDR %g, Sign=%d,
   Thresh = %g`; sign 0: the absolute value taken (step 5) and the minimum the threshold; +1: the
   minimum; -1: the maximum minus the threshold. FS-1020: when no threshold can be computed (an
   input that is not `float`, no voxel in the mask or of the sign), FreeSurfer prints and
   thresholds with memory contents; larmorx ends with `mri_binarize: --fdr: MRIfdr2vwth gave no
   threshold (FreeSurfer goes on with memory contents)` on standard error, status 1 (`reproduce`:
   0).
4. `--frame-sum` or `--frame-and`: `Summing over <n> frames`, each voxel's frames summed in
   `double` and written into its frame 0 (the input's type: a `uchar` sum saturates), the frame 0;
   `--frame-and`: the minimum `nframes - 0.5`.
5. `--abs`: `Removing sign from input`, each voxel its absolute value. `--neg`: `Negating input`,
   each voxel times -1 (in `double`, written back in the input's type).
6. `--rmin`/`--rmax`: `Computing global statistics `, the mean over every voxel and frame (`double`
   sums, columns outermost, then rows, slices, frames), the standard deviation `sqrt(sumsq / n -
   mean^2)` and the maximum (from -1,000,000): `mean = %g, std = %g, max = %g`; `--rmin`: the
   minimum `mean * r`, `Setting min thresh to %g`; `--rmax`: the maximum `mean * r`, `Setting max
   thresh to %g` with the maximum (FS-1019: FreeSurfer prints the minimum there; `reproduce` too).
7. The merge volume and the copy volume read; each of another width, height or depth: `ERROR:
   dimension mismatch between input and merge volumes` (the copy's message names the merge volume
   too), status 1. The copy volume is never used (FreeSurfer's behaviour, kept).
8. `--pct`: `Computing threshold based on top %g percent`, the minimum from
   `specs/freesurfer-volume.md`'s `MRIpercentThresh` (the frame, the mask), `  Threshold set to %g`.
9. The frames: all (first 0, last `nframes - 1`), or the one (`--frame`, `--frame-*`); unless
   `--noverbose`, `fstart = <a>, fend = <b>, nframes = <n>`.
10. The output: `int` (`uchar` with `--uchar`), the input's extent, the frames of step 9, the
    input's header (geometry, colour table, ...). Unless replacing only, for each frame `f`, voxel
    by voxel: the "not" value is the merge volume's voxel at frame `f` (its frames counted from 0:
    a merge volume of fewer frames is read past its memory, FS-53) or `--binvalnot`; a voxel on a
    zeroed edge, or whose mask value (frame 0) is below the mask threshold, gets the "not" value;
    else its value `v` (frame `f`, as a `double`) is in when it matches (`|v - m| < 2 * FLT_MIN`
    for some match value `m`) or, without match values, is not below a set minimum nor above a set
    maximum: it gets `--binval` and is counted; else the "not" value. Written at frame `f - first`.
    FS-1018: with more than one thread FreeSurfer's matching loop shares its counter of match
    values between threads (matches can be missed or made at random); larmorx computes as one
    thread (`OMP_NUM_THREADS=1` gives FreeSurfer's), and notes FS-1018 when FreeSurfer would use
    more.
11. Replacements: with replace only and transitive replacement, `Replacing <n> and propagating
    source list`, the output a copy of the input, then for each pair in order `%2d:  %4d %4d`
    (its number from 1, the two values) and every voxel equal to the first value set to the
    second (in the copy, so later pairs see earlier ones). Otherwise `Replacing <n>`, the pairs
    printed, and the output (the binarisation discarded) the input with each voxel (inside the
    mask) replaced by the target of the first pair whose source it equals (`MRIreplaceList`).
12. Nearest-neighbour replacements: with replace only, `Replacing <n> with nearest neighbors and
    propagating source list` and the output a copy of the input; else `Replacing <n> with nearest
    neighbors` (into the output so far). For each frame and voxel (inside the mask: its value above
    the threshold) whose value (truncated to an `int`) is a source: the window's half `(w - 1) /
    2` (C's integer division) of that source's (first matching) pair; every voxel of the window
    inside the volume (slices outermost, then rows, columns, each from `-half` to `half`) whose
    value is no source is a candidate; its distance `sqrtf((dc*dc + dr*dr) + ds*ds)` (each square an `int` added to the `float` sum in that order);
    the first candidate at a distance below the smallest so far (from 100,000) gives the value; the
    voxel's own value when none; written.
13. Unless `--noverbose` or replacing only: `Found <n> values in range` (the count of step 10).
14. `--dilate n`: `Dilating <n> voxels in 3d`, `n` dilations. `--erode2d n`: `Eroding <n> voxels in
    2d`, `n` 2-D erosions. `--erode-* n`: `Eroding <n> voxels using <type> ErodeEdges=<0|1>` (type
    1 face, 2 edge, 3 corner), `n` erosions by those neighbours, the volume's edges eroded with
    `--erode-edges` (`specs/freesurfer-volume.md`'s `MRIdilate`, `MRIerode2D`, `MRIerodeNN`).
15. The count again (unless `--no-count` or replacing only): `Counting number of voxels in first
    frame` (unless `--noverbose`), the voxels of frame 0 within 0.00001 of `--binval`, `Found <n>
    voxels in final mask`. Otherwise the count is 0.
16. `--bincol`: `Filling mask with column number`, `MRIbinMaskToCol`.
17. `--bb`/`--crop`: `Computing bounding box, npad = <n>`, the bounding box of the nonzero voxels
    padded (`REGIONgetBoundingBox`), printed as `specs/freesurfer-volume.md`'s region report, and
    the output cropped to it (none: status 1).
18. `--remove-islands`: `Removing Volume Islands` (flushed), `MRIremoveVolumeIslands` with the
    threshold `binval - 0.5` and `binval` clusters kept. `--fill-holes`: `Removing Volume Holes`
    (flushed), `MRIremoveVolumeHoles` with the threshold `binval - 0.5`, `binval` kept and the fill
    value `binval`. FS-1025: with `binval` 0 these give no volume (`ERROR:
    MRIremoveVolumeIslands(): nKeep==0`) and FreeSurfer goes on with none and crashes; larmorx
    ends with `mri_binarize: <step> gave no volume (FreeSurfer crashes here)` on standard error,
    status 1 (`reproduce`: status 139).
19. `--fix-vol-topo`: `Fixing volume topology` (flushed), the topology fix (§4) of `binval`.
    FS-1021: when the volume is topologically correct already (its tessellation's Euler number 2)
    the fix gives the volume itself, which FreeSurfer frees and then writes (a crash, or memory
    contents); larmorx keeps it (`reproduce`: status 139). FS-1025 when the fix gives none.
20. The colour table: the input's (a copy) when replacing only or with `--copy-ctab` and the
    input has one; then `--ctab`'s when read.
21. `--o`: `Writing output to <name>`, written (not written: status 1).
22. `--surf`: `Creating surface <name>`, the tessellation (§5) of `binval`, its hemisphere "none";
    with `--surf-smooth n`, `n` passes of `MRISaverageVertexPositions`; `--reverse`: `Reversing face
    order`, the faces reversed and the metric properties computed again; written
    (`specs/freesurfer-surfio.md`'s writer, by the name's type).
23. Unless `--noverbose` or replacing only: `Count: <n> <%lf> <nvox> <%lf>`: the count, the count
    times the voxel volume (the three sizes multiplied as `float`s, then `double`), the output's
    voxels (width times height times depth) and `100 * count / nvox`. `--count <file>`: the same
    four numbers and a newline into the file (unopened: `ERROR: could not open <file>`, status 1).
24. Unless `--noverbose`: `mri_binarize done`; status 0.

## 4. The topology fix

Of a binary volume `B` (on: value `onval`; a voxel is set when its frame-0 value is above 0.5), at
most 1,000 dilations, 1,000 erosions and 1,000 topological erosions:

1. The Euler number `e` of `B`'s tessellation (§5) of `onval` (vertices minus edges plus faces of
   the unripped surface, `specs/freesurfer-surfio.md`). `e = 2`: `B` itself (FS-1021 above).
2. A copy `C` of `B` (with its pulse parameters and colour table); the set voxels listed (columns
   outermost): `Found <n> voxels in binseg`, and for voxels on the volume's faces `WARNING: binseg
   has <n> edge voxels. You may want to add some padding if results are not good`. A second copy
   `S` of `C` (the voxels to keep).
3. `\nDilating ============================`; repeated: `dilate n=<n> eno=<e>`, `C` dilated by its
   6 face neighbours (`MRIdilate6`) with its edge planes set to 0 (`MRIsetEdges`), `e` of the new
   `C`; `e = 2`: `  no defects found, breaking from dilation`; after more than the maximum: `
   ERROR: number of dilations > ndilationsmax=1000` and no volume (FS-1025).
4. `\nEroding ============================`; repeated: `erode n=<n> eno=<e>`, `C` eroded by its
   face neighbours, the volume's edges included (`MRIerodeNN`), every listed voxel set to `onval`
   again, `e` of the result; `e` not 2 (`  defects found, breaking from erode, and reverting to
   previous`) or more than the maximum (`  WARNING: number of erodes > nerodesmax=1000`): `C` stays
   as before this erosion; else `C` is the result. FS-1022: FreeSurfer sets every listed voxel but
   the first again (the first set voxel can be eroded away: one voxel of the input missing from
   the fix); larmorx sets them all (`reproduce`: all but the first), and notes FS-1022 when an
   erosion removed the first.
5. `\nTopoEroding ============================`; repeated: one topological erosion of `C` keeping
   `S`'s voxels (`specs/freesurfer-volume.md`'s `MRIvolTopoErodeOne`), `e` of the result, `n=<n>
   nhits = <h> eno=<e>`; until no voxel was eroded or more than the maximum (`  WARNING: number of
   topoerodes > max=1000`).
6. `C`, with `S`'s pulse parameters.

FS-1027: FreeSurfer's tessellation keeps about 1.25 GB of address space it never releases each
time it runs: the fix, which tessellates at every step, ends where the address space or the memory
limit runs out (`Cannot calloc()` and status 0 without writing anything, or a crash; observed after
55 dilations under the oracle-output cache's limit and 865 in an unlimited 4 GiB box). larmorx
releases its lists and notes FS-1027 from the 32nd tessellation of a fix.

## 5. The tessellation

Of a volume `V` (width `W`, height `H`, depth `D`) and a value `v` (compared with each voxel's
frame-0 value as a `float`; `v` converted to `float`): the surface whose faces are the boundary
faces between voxels of value `v` and voxels of another value (`all` off; with `all` on, between
any two different values). Voxels outside the volume are never compared.

- **The corners.** The lattice corners `(slice k, row i, column j)` with `k` from 0 to `D`, `i`
  from 0 to `H`, `j` from 0 to `W` (slices outermost, then rows, then columns). A corner is a
  vertex when one of the twelve voxel pairs around it, in this order, is a boundary (both voxels
  inside the volume, their values different, one of them `v`): with `(dk, di, dj)` offsets from
  `(k, i, j)` for the two voxels, `(0,-1,-1)-(-1,-1,-1)`, `(0,-1,0)-(-1,-1,0)`,
  `(0,0,0)-(-1,0,0)`, `(0,0,-1)-(-1,0,-1)`, `(-1,0,-1)-(-1,-1,-1)`, `(-1,0,0)-(-1,-1,0)`,
  `(0,0,0)-(0,-1,0)`, `(0,0,-1)-(0,-1,-1)`, `(-1,-1,0)-(-1,-1,-1)`, `(-1,0,0)-(-1,0,-1)`,
  `(0,0,0)-(0,0,-1)`, `(0,-1,0)-(0,-1,-1)` (voxel `(k, i, j)` is column `j`, row `i`, slice `k`).
  Vertices are numbered in the order found.
- **The quadrangles.** Each boundary face is a quadrangle of four corners, numbered in the order
  made. Two tables of quadrangle numbers indexed by `(direction d from 0 to 5, row, column)` (both
  zero at the start: "previous" `P` and "current" `Q`) are kept; after each slice of corners, `P`
  becomes a copy of `Q` (`Q` is not cleared). At a vertex `x`, in order, for each of the 24
  entries `(voxel a, voxel b, d, n, prev)` below (offsets from `(k, i, j)` as above): when both
  voxels are inside the volume and the pair is a boundary *in the direction of `a`* (`a`'s value
  is `v` and `b`'s is not; with `all` on, their values differ), then if `n` is 0 a new quadrangle
  is made and its number stored in `P` (`prev` 1) or `Q` (`prev` 0) at `(d, a's row, a's column)`;
  in any case the quadrangle whose number that table holds there (possibly an earlier one) gets
  `x` as its corner `n` (corners never set stay vertex 0):
  `(0,-1,-1)-(-1,-1,-1) d0 n2 Q`, `(0,-1,0)-(-1,-1,0) d0 n3 Q`, `(0,0,0)-(-1,0,0) d0 n0 Q`,
  `(0,0,-1)-(-1,0,-1) d0 n1 Q`, `(-1,0,-1)-(-1,-1,-1) d2 n2 P`, `(-1,0,0)-(-1,-1,0) d2 n1 P`,
  `(0,0,0)-(0,-1,0) d2 n0 Q`, `(0,0,-1)-(0,-1,-1) d2 n3 Q`, `(-1,-1,0)-(-1,-1,-1) d4 n2 P`,
  `(-1,0,0)-(-1,0,-1) d4 n3 P`, `(0,0,0)-(0,0,-1) d4 n0 Q`, `(0,-1,0)-(0,-1,-1) d4 n1 Q`,
  `(-1,-1,-1)-(0,-1,-1) d1 n2 P`, `(-1,-1,0)-(0,-1,0) d1 n1 P`, `(-1,0,0)-(0,0,0) d1 n0 P`,
  `(-1,0,-1)-(0,0,-1) d1 n3 P`, `(-1,-1,-1)-(-1,0,-1) d3 n2 P`, `(-1,-1,0)-(-1,0,0) d3 n3 P`,
  `(0,-1,0)-(0,0,0) d3 n0 Q`, `(0,-1,-1)-(0,0,-1) d3 n1 Q`, `(-1,-1,-1)-(-1,-1,0) d5 n2 P`,
  `(-1,0,-1)-(-1,0,0) d5 n1 P`, `(0,0,-1)-(0,0,0) d5 n0 Q`, `(0,-1,-1)-(0,-1,0) d5 n3 Q`.
  At most 9,999,999 vertices and 19,999,999 quadrangles: one more ends the program
  (`<program>: max vertices 10000000 exceeded`, `<program>: max faces 20000000 exceeded`,
  `ERROR_NOMEMORY`).
- **The surface.** A triangle surface of the vertices and twice the quadrangles. Vertex `x` at
  the corner `(k, i, j)` has the coordinates of the voxel position `(j - 0.5, i - 0.5, k - 0.5)`
  (each a `float`) through `V`'s tkregister voxel-to-surface matrix (`specs/freesurfer-volume.md`'s
  `surfaceRASFromVoxel_`, a `float` matrix product). Quadrangle `q` with corners `c0..c3` gives
  triangles `2q` and `2q+1`: with `s = nint(sqrt(1.9 c0) + sqrt(3.5 c1))` (`double`; `nint` rounds
  half away from zero) even, `(c0, c1, c3)` and `(c2, c3, c1)`; odd, `(c0, c1, c2)` and `(c0, c2,
  c3)`. Each vertex's face list is its triangles in triangle order (a vertex listed twice in a
  triangle has it twice), each with the vertex's last place in it. The volume's geometry is the
  surface's; then its neighbourhoods are completed and its metric properties computed
  (`specs/freesurfer-surfio.md`).
- With diagnostics (`Gdiag_no` above 0): `slice <k>: <nv> vertices, <nq> faces` before every tenth
  slice once there are any, and `MRIStessellate: using vox2ras matrix:` with the matrix.

## 6. Validation plan

The open probe `validation/investigations/freesurfer-mri-binarize-probe/` (its README) runs the
official program and a candidate on 130 command lines over seeded synthetic inputs (every option,
the stand-alone options, the failures, the topology fix on a shape with a handle, an already
correct one and an empty one) and compares exit status, messages and every file written;
`reproduce` must give FreeSurfer's on every case but those noting FS-53, FS-58, FS-1020, FS-1021,
FS-1023, FS-1025, FS-1026 or FS-1027 (counted); `fix` may differ only with a notice. The replica
(observed, 2026-11-21): seed 1, 122 of 130 identical in `reproduce`, 8 counted, 0 different. A clean
room is also checked against the real calls of recon-all, `mri_nu_correct.mni`, `mri_mcadura_seg`,
`mri_vsinus_seg`, `label-cortex` and `seg2cc` on the recorded heads (the replica's record,
`docs/validation/freesurfer-mri-binarize.md`), and its golden cases (32^3, both modes).
