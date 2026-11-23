# fs.volume's clean room: what black-box runs showed (unrestricted)

The clean room of FreeSurfer 8.2.0's volume functions, parts 1 to 9 and 10a of the spec and §54 to §57
(`crates/larmorx-fs-core/src/volume/`, the rest of queue item C3.1-05), was written from
[`specs/freesurfer-volume.md`](../../specs/freesurfer-volume.md) §1 to §48 and §54 to §58 (part 9: volume-to-volume
resampling, frame covariance and PCA, the false discovery rate, regions, voxel neighbourhood measures;
part 10a: the Talairach helpers, dilation and the segment helpers;
the operations on a transform array; segmentations onto another grid, SPM warps, segment statistics and
shifts, crops), and checked only against
black boxes: the prebuilt probe `fs_volume_probe` (FreeSurfer's compiled library and the C library,
glibc 2.35) run through the open folder
[`validation/investigations/freesurfer-volume-probe/`](../../validation/investigations/freesurfer-volume-probe/README.md).
Nothing here comes from FreeSurfer's source; this file is open to everyone. The comparisons and the
tools are in
[`validation/investigations/freesurfer-cleanroom-volume/`](../../validation/investigations/freesurfer-cleanroom-volume/README.md).
fs.bspline's clean room, which the cubic B-spline paths of §29 and §31 use, has its own file:
[`fs-bspline-clean-room.md`](fs-bspline-clean-room.md).

## The comparison

- **What:** the 58 groups of the spec's §69.2 and of `specs/freesurfer-bspline.md` §9 (18,639 calls,
  seed 7: parts 1 to 3's ten, the fixed groups `vrules` and `vrules4` to `vrules20`, part 4's
  `vsample`, `vmove`, `vskull`, `vltaread`, part 5's `vcma`, part 6a's `vres`, part 6b's `vframes`,
  `vderiv`, `vlinear`, part 6c's `vresamp`, the cubic B-spline paths' `vbsvol`, part 7a's `vstat7`, part
  7b's `vimage` and `vnbhd`, §54's `vltapv` and `vlta54`, part 8's `vpvlab`, `vfr8` and `vmri2`, §55 to
  §57's `vaseg`, `vsegs` and `vcrops`, part 9's `vv2v`, `vregion`, `vcov` and `vvoxel`, part 10a's
  `vtal` and `vdil`, and fs.bspline's `vbspline` and `vbsrules`), on the probe (plainly, again, and in
  two guard runs; every reference hash the spec's, the probe of commit `2a612716`) and on the clean
  room's candidate, in both modes. `reproduce`: 17,467 identical, 924 where FreeSurfer did not finish (9
  of them FS-314's aborts, which the candidate gives too and the loop checks), 221 memory lines, 27 lines
  noting a defect whose value in FreeSurfer is memory or whose end the loop allows in both modes (§69.3:
  FS-300 in `vsample` and `vresamp`, FS-325 in `vlinear`, FS-469 in `vpvlab`, FS-509 and FS-517 in
  `vrules15`, FS-517 in `vrules16`), none unexplained. `fix`: 13,216 identical, 924 not finished, 605
  memory, 3,894 corrected by a noted defect, none other. Every line of `vgblock`, `vgcopy`, `vcoords`,
  `vheader`, `vskull`, `vrules4`, `vrules7`, `vrules10`, `vrules11`, `vaseg`, `vsegs`, `vcrops`,
  `vregion`, `vrules18`, `vtal` and `vdil` is identical in `reproduce`. A few lines of earlier groups
  moved between "did not finish", "memory" and "identical" against relay 17's run (the rebuilt probe's
  four runs classify its memory lines anew; its reference hashes are unchanged). fs.mri (16 groups, 9,511
  calls) was run again: none unexplained in either mode; fs.maths was not touched.
- **How we know:** *validated* (`compare.py compare`, both modes, 2026-11-23, relay 18; log
  `oracles/logs/fs-cleanroom-2-relay18-full.log`).

## FS-291 on volumes of other types (resolved by the spec's tensor rows)

- **What:** relay 7's 9 lines (`MRIinvertContrast` and `MRIscaleAndMultiply` under `reproduce` with
  volumes that are not of bytes) were tensor volumes: the spec now states (§6) that every row of a
  tensor volume walked directly starts at its first voxel, and (§9, §10) that the default refuses
  only when the source's width, height and depth are all positive, naming the first type that is
  not `uchar`. The clean room follows both: every line of `vscale`, `vtwo` and `vrules` identical
  in `reproduce`, none unexplained in `fix`.
- **How we know:** *validated* (the same comparison).

## Behaviours relay 7 observed, now stated by the spec

The spec writer answered relay 7's items (2026-11-17): the reads and writes of the types without a
value without voxels (`specs/freesurfer-mri.md` §5, FS-58's exceptions), `MRIclear` of a bitmap and
of volumes without voxels (§7), `MRIlinearScale`'s other voxels (§9), sums and differences of types
without direct access (§10), the frame copies' messages (§6, §7), FS-265 (§8: both modes the whole
volume, noted) and the nearest sample's order of checks (§15). The clean room's behaviour did not
change with them but FS-265's notice; the observations as relay 7 recorded them:

- **Voxel reads and writes of the types without a value** (tensor, bitmap, double complex; fs.mri's
  §5): `MRIgetVoxVal` gives -1e10 and `MRIsetVoxVal` returns 1 without touching voxels, so a volume
  of such a type without voxels gives -1e10 and does not crash (`MRIvalRange` of a header-only
  tensor volume: minimum -1e10, maximum -10000). FS-58 applies only to the types read and written;
  negative indices give `outside_val` before it. The clean room's fs.mri follows that order (the fs.mri
  comparison, both modes, unchanged: none unexplained).
- **`MRIclear` of a bitmap volume:** no error (another type: the error); a bitmap volume has no
  voxels, so with any row FreeSurfer crashes (the clean room: FS-58's end), with none it returns 0.
- **`MRIlinearScale` with `only_nonzero`:** the voxels with `|v| < 1e-15` are written with their own
  value (a subnormal stays, a destination's other bytes are replaced), not left as they were.
- **`MRIadd` of two sources of one type without direct access** (RGB, complex): the destination is
  not written (`MRIsubtract` writes the previous difference, 0 at first, as the spec says).
- **The messages of `MRIcopyFrame` and `MRIcopyFrames`** end with a new line of their own (before the
  C-style error's NUL, FS-01): `MRIcopyFrame: dst frame #<df> out of range (nframes=<n>)\n`.
- **`MRIclone(src, src)`** with a colour table: FreeSurfer frees the destination's table (the
  source's own) before copying the source's, prints `CTABdeepCopy: ct was NULL` and leaves the
  volume without a table (5 lines of `vmake`). **FS-332** (new, rule A): by default the volume keeps
  its table; `reproduce` gives FreeSurfer's.
- **`MRIvalRangeRegion` without a region:** FreeSurfer crashes (every such line of `vrange`); the
  clean room takes the whole volume (FS-265 now, noted in both modes).
- **The nearest sample** (`MRIsampleVolumeType`): the type check comes after the outside check; an
  unsupported type returns -3 (`ERROR_UNSUPPORTED`) and the sample 0.
- **How we know:** *measured* (the probe's `vmake`, `vrange`, `vscale`, `vtwo`, `vstats`).

## FS-54's default, noted against FreeSurfer's rounding (fs.mri)

- **What:** a long voxel written with a NaN or a value beyond `int`: FreeSurfer's `nint` gives
  -2^31 (FS-12), the clean room's default stores the value rounded in the long range (a NaN: 0). The
  default now notes FS-54 wherever its stored bytes differ from FreeSurfer's (before, it compared
  with its own corrected `nint`, so 2 lines of `vscale` differed in `fix` without a notice).
- **How we know:** *validated* (`vscale` in `fix`: none unexplained; the fs.mri comparison unchanged).

## Part 4: what relay 8 observed, and the amended spec (relay 9)

Relay 8's observations went to the spec writer, who amended §18 to §22 (spec commit `53a939af`);
the clean room now follows the amended text, which states them:

- **`MRIcubicCoeff` with an index outside 0 to 3** gives -3 (`c008000000000000`) after its error.
- **`MRIcubicSampleVolumeFrame`'s window points outside the volume** (FS-300): FreeSurfer never
  reads them and adds whatever the stack holds; the probe's four runs showed the all-ones NaN
  (`ffffffffffffffff`) there, left by the probe's own earlier work. The amended spec takes those
  points as 0 in both modes, noted, and the loop lets such lines differ (relay 8's `reproduce`
  gave the all-ones NaN; relay 9 follows the amended text).
- **`ham_sinc`'s last product** keeps the window's NaN where both factors are NaN; an infinite
  argument of `sin` or `cos` gives x86-64's default NaN in both modes (larmorx's correctly rounded
  `sin` and `cos` give a positive NaN there, so the clean room gives the default NaN itself).
- **`MRIrotate_I`**: the offset and the point are 3 x 1 matrices made once; the point is the
  product's output and then the sum's (`MatrixAdd(X, O, X)`), so a rotation matrix of another size
  than 3 x 3 leaves the product out and each voxel adds the centre to the previous voxel's point,
  and a centre of another size than 3 x 1 leaves the sum out (`vrules4`'s 43 `rot` lines, all
  identical). The sum into its own operand keeps the centre's NaN (`specs/freesurfer-maths.md`
  §8.2).
- **Reading an `.xfm`:** the line tests read `fgetl`'s buffer (its leading white space kept, the
  piece of a skipped `#` or blank line at the end of the file), 900-byte pieces; rows with fewer
  numbers keep the identity's; the volumes' names from `%Generated by` (anywhere in the line),
  `minctracc` (a count kept for the whole program run, so a line with one `.mnc` word names the
  next file's destination) or the second words; a named volume that does not exist leaves `errno`
  2. `vrules4`'s 40 `ltaread` and 8 `ltareads` lines (several files in one process) are identical.
- **`TransformFileNameType`:** the name cut at its first `#`, upper-cased; the extension after the
  last `@` of the whole name (one after the `#` compared as written: `a#b@xfm` is not MINC,
  `a#b@XFM` is) or after the last `.` of the cut name; a cut name ending in `nii.gz` a morph.
- **A `register.dat` that cannot be opened** leaves `errno` 2 once the program has written to its
  standard error (the probe's build of commit `9319981f` orients it before each call; the earlier
  build showed EINVAL, 22, the C library's `perror` on a stream not yet oriented).
- **How we know:** *measured* (`vsample`, `vmove`, `vltaread`, `vrules4`; `models.py skull` agrees
  with the clean room's walks on all 160 `vskull` lines).

## Part 4: the defects' modes (D13, §22's table)

| Defect | Default | `reproduce` |
|---|---|---|
| FS-298 | a 64-bit volume's own values | 32-bit integers at the voxels' places |
| FS-299 | the cubic sum divided by 8 | divided by the points added |
| FS-300 | outside window points 0 | the same |
| FS-301 | a moved sample written by the destination's own write (fs.mri §5's `MRIsetVoxVal`) | its byte at the voxel's byte place |
| FS-302 | a window over 128 voxels ends the program (code -4) | the same |
| FS-303 | a sample the sampler refuses: the one before (0 at first) | the same (FreeSurfer's first is memory contents) |
| FS-304 | an axis rotation without an inverse ends the program (code -4) after `MRIrotate_I`'s errors | the same (FreeSurfer crashes) |
| FS-305 | a centre of mass that is not a number ends the program (code -4) before any walk | the same |
| FS-306 | a name of 500 bytes or more classified whole | the same (FreeSurfer writes past its memory) |
| FS-307 | an `.xfm` whose last piece is a `%` line in the skip ends the program (code -5) | the same (FreeSurfer never returns) |
| FS-308 | an `.lta`'s unset type or count taken as 0 | the same |
| FS-309 | an `.lta` of one line ends the program (code -5) | the same (FreeSurfer crashes) |
| FS-310 | a `src` or `dst` word with no word after it ends the program (code -5) | the same (FreeSurfer crashes) |
| FS-960 | a tensor volume's rows walked directly each at its own place | every row at the first voxel |
| FS-258 | a register.dat name of 1,000 bytes kept whole | the same |

The ends of the program carry the table's messages; their C-style error lines differ between the
modes only by FS-01's NUL. **FS-960** is met only under `reproduce` in parts 1 to 5: the default
never walks a tensor volume directly there (FS-291's default refuses those walks and FS-301's
writes through `MRIsetVoxVal`); `MRIextractInto` refuses a tensor source. 6 lines of `vscale`,
`vtwo` and `vrules` note it, all identical.

## Part 5: the CMA labels (§23)

- **What:** every `vcma` line is identical in `reproduce` but FreeSurfer's crashes (13: header
  copies of segmentations without their cached matrices, FS-252, and the statistics cache's aborts,
  FS-314) and memory lines (10); in `fix` 58 lines are corrected by a noted defect (FS-311 to FS-315,
  FS-01, FS-12, FS-54, FS-56). The walks of the segmentation functions (columns outermost, slices
  innermost) give their messages in FreeSurfer's order; `MRIoneHotEncode`'s line comes after the
  allocation's message.
- **FS-315 (label lists of 0 columns):** of `vcma`'s 13 such lines, 11 are memory lines; 2 read the
  same in all four probe runs (`nswaps 0`, no voxel swapped). The spec now states both modes (relay
  9's question, answered 2026-11-18): the default ends the program (code -4) after the row counts'
  check, before any voxel is written; `reproduce` reads the numbers past a list's end as NaN, equal
  to no label. The clean room follows it; `vrules5`'s fixed lists (one or both lists of 0 columns,
  lists of no rows, `n x 1` and `1 x n` lists on three segmentation types) are identical in both
  modes but the memory lines. The default's message has no final new line (the spec gives none).
- **FS-314 under `reproduce`** (answered 2026-11-18): FreeSurfer's abort, the C++ runtime's two lines
  (`std::length_error` with `basic_string::_M_create` when a word of the line is longer than 15
  bytes, else `std::bad_alloc`) and status 134; all 9 such lines of `vcma` and `vrules5` end the
  same in the candidate (the loop's abort check). The default gives no number (noted).
- **`ReadCachedBrainVolumeStats` without a cache** prints `computing (and caching) brain volume
  statistics` and calls the caller's `ComputeBrainVolumeStats`, which the spec leaves out (it reads
  a subject's surfaces): the probe's one such line ends there with status 255
  (`FREESURFER_HOME` is not defined). The clean room takes that function from its caller; the
  candidate's stand-in prints the probe's two lines.
- **How we know:** *validated* (`vcma`, both modes, 2026-11-17, relay 9; every `cmalab` line, 95,
  identical, the clean room's label tests written from §23's tables).

## The `.xfm` reader's line 3 (answered 2026-11-18)

- **What:** a line 2 that is neither an information line (`%`) nor a `Transform_Type` line goes on to
  step 3 like a `%` line: line 3 is read and passed over before the rows (relay 9 had read such a
  line 2 as going straight to the matrix, which `vltaread` and `vrules4` could not tell apart). A
  word holding both `src` and `dst` is a `src` word (tested first); a later `src` word replaces the
  source; a `dst` word first ends the search without a source. `vrules5`'s eleven `.xfm` texts are
  identical in both modes.
- **How we know:** *validated* (`vrules5`, `vltaread`, `vrules4`, both modes, 2026-11-18).

## Part 6a: resolution, planes, fills, whole-factor resampling, changes of type (§25)

- **What:** `vres`'s 830 calls are identical with `reproduce` but where FreeSurfer did not finish
  (77) and the memory lines (37); in `fix` 157 lines are corrected by a noted defect (FS-316 to
  FS-318 and the voxel access's FS-52 to FS-58, FS-289, FS-290). Measured beyond §25's text, all
  confirmed on `vres`'s lines where aliasing makes the order visible:
  - `MRIextractPlane` walks the plane's x outermost and its y innermost (a smaller destination, or
    the source itself, is written through `MRIsetVoxVal`'s places, so the order decides which value
    a voxel keeps).
  - `MRIfillPlane` with an orientation that is not 0, 1 or 2 fills the coronal plane, as
    `MRIextractPlane` reads it (5 lines of orientation 3).
  - `MRIchangeType`'s histogram ends: the low end adds bins from 0 while their total is below the
    target, up to all 1000 of them (relay 10 stopped at 999: no `vres` line tells them apart; the
    spec's `vrules6` cases with `f_low` 1 and 2 add all 1000, relay 11); the high end adds bins from
    999 down and never adds bin 0 (6 lines of `chtype` and `ups 2` whose ranges would otherwise be
    empty).
  - `MRIdownsample2` into the source itself doubles the voxel sizes before it takes the source's
    voxel-to-RAS matrix for the centre (the matrix then has the doubled sizes).
  - `MRIfillFG`: a byte written where the destination holds no voxel is read back at the same
    coordinates (§25 states it); without that a fill into a smaller destination never ends. Under
    FS-316's `reproduce` a seed past the depth writes its byte into a later frame's voxel and the
    sweeps' box, clamped to the extent on every axis, holds the last slice (one past) or nothing
    (further): `npix` 1 (`vrules6`, relay 11).
- **FS-319 is not reachable:** `MRIupsampleNConserve` changes a source to floats with `f_low` and
  `f_high` 0; the low end then keeps the minimum and the high end's maximum is the minimum plus a
  bin count (0 or more) times a size that is not below 0, so the range is never empty (a NaN range
  never compares as empty). The clean room keeps FS-319's end of the program for a failed change;
  no input reaches it. No `vres` line shows it either (the probe's three `ups 2` crashes are a
  plain conversion of a volume with a registration matrix, one with a colour table, and an
  upsampling into the source itself).
- **How we know:** *validated* (`vres`, both modes, 2026-11-18, relay 10).

## Parts 6b and 6c: frame-range samplers, splatting, derivatives, neighbour counts, linear transforms, resampling onto a template (§27 to §31)

- **What:** `vframes`, `vderiv`, `vlinear` and `vresamp` (1,418 calls) are identical with
  `reproduce` but where FreeSurfer did not finish (54), the memory lines (4) and the two lines below
  (FS-325, FS-300), which §69.3 now allows in both modes;
  in `fix` 267 lines are corrected by a noted defect (FS-320 to FS-328, FS-461, FS-298 and the voxel
  access's). Measured beyond the text:
  - **`MRIindexNotInVolume` rounds the double:** its "ambiguous" test (§15: the coordinates rounded
    to the nearest integers, ties to even, as floats, in `[0, extent)`) takes the double's rounding,
    then the float: a sample at row 2.500000119 of a volume 3 rows high is unambiguously outside
    (`vderiv`'s line 81: the derivative's sample at `y + 1.5` gives `outside_val`; rounding the float
    first, 2.5 to 2, would sample it and give 18.2116 for the probe's 7.7501). Every earlier group is
    identical either way (no earlier line has a coordinate within half a float step of a half).
  - **FS-325's value never written:** an `lxf` with the sinc method at points that are not whole
    (`vlinear`'s line 235: every sample fails) stores in FreeSurfer the bytes its own earlier steps
    leave on the stack: in a float destination the NaN with every bit set (`ffffffff`; relay 11 read it
    as an int's -1), in an `int` one -2147483648 (the NaN's `nint`), the same in all four of the probe's
    runs; the clean room stores 0 as §29 states (FS-325, noted in both modes), and §69.3 now lets such a
    line differ in both modes (answered 2026-11-18).
  - **FS-300 in a cubic resampling:** `MRIresampleFill` with `SAMPLE_CUBIC` reads the cubic sampler's
    window points FreeSurfer never reads (`vresamp`'s line 31: one voxel 0x7f in the probe, 0x8d in the
    clean room, whose `reproduce` takes those points as 0, §18); §69.3 now allows FS-300 in `vresamp`
    in both modes, as in `vsample`.
  - **FS-326 reached** (relay 11 found no `vresamp` line past 255 votes; the spec writer's fixed group
    `vrules7`, 2026-11-18): byte sources of 513 x 3 and 511 x 3 voxels and 100 to 130 frames voting onto
    one 512 mm voxel; with `reproduce` FreeSurfer's labels (7, 0, 0, 0: the counts stop at 255), by
    default the label with the most votes (7, 1, 5, 0), all 12 lines identical in `reproduce`.
- **How we know:** *validated* (`vframes`, `vderiv`, `vlinear`, `vresamp`, both modes, 2026-11-18,
  relay 11).

## Part 7a: statistics, orientation strings, voxel-to-RAS matrices (§33)

- **What:** `vstat7` (481 calls) and `vrules8` (32) are identical with `reproduce` but their memory lines
  (7 and 3: FS-330's printed values, FS-463's strings of fewer than two characters); in `fix` 44 lines
  are corrected by a noted defect (FS-329, FS-330, FS-463, FS-01, FS-55). Measured beyond the text:
  - **`MRIsetVoxelToRasXform`'s column scales** keep the later row's NaN where two meet: the sum of the
    three squares in `double` takes each new square as its first operand (`sq + t`; `vstat7`'s one line
    whose column has NaNs in rows 2 and 3 prints `-nan`, row 3's default NaN, not row 2's payload);
    finite sums are the same in either order.
  - **`MRIvoxelXformToRasXform` and `MRIgetVoxelToVoxelXform`'s `vox2ras` and `ras2vox`** are the
    matrices made from the geometry fields (`extract_i_to_r`, `extract_r_to_i`), not the cached ones:
    every `vx2rx` and `v2v` line identical, among them volumes whose cache was left as allocated.
  - **`stuff_four_by_four` into a complex 4 x 4 matrix** writes the floats at the real elements'
    places of its storage, the probe printing its 32 floats after the two unused ones.
  - The defects the probe cannot show, as the spec now states them (relay 13): FS-462 (a sampled
    volume's extent of 0, or an overflowing quotient) and FS-464 (a matrix of fewer than 3 rows or 4
    columns, or none) end the program in both modes (status 252) with their errors, before anything is
    made, read or printed (the crate's own tests).
- **How we know:** *validated* (`vstat7`, `vrules8`, both modes, 2026-11-19, relay 12).

## Part 7b: slices as 2-D images, neighbourhoods and labels (§34, §35)

- **What:** `vimage` (290 calls), `vnbhd` (441) and `vrules9` (34) are identical with `reproduce` but
  their memory and crash lines (frames outside the volume, FS-290; given images without pixels or too
  small, FS-465; negative labels whose stack counts FreeSurfer's runs see differently, FS-467; FS-323's
  windows; second volumes read outside, FS-53); in `fix` 199 lines are corrected by a noted defect
  (FS-01, FS-12, FS-52 to FS-56, FS-289, FS-465, FS-466, FS-468). Measured beyond the text:
  - **The label sums keep the LATER value's NaN where two NaNs meet**: `MRIcomputeLabelNbhd`'s `float`
    sums of the values and the label means' `double` sums (`MRImeanInLabel`, `MRImeanAndStdInLabel` and
    their squares) take each new value as the first operand of the addition (`v + sum`). Four `vnbhd`
    lines (an infinity's sum with the opposite infinity, then a NaN payload) and a 64^3 timing line
    differed with the running sum first; a discriminating check on 2 x 2 x 1 volumes of chosen payloads
    (`validation/investigations/freesurfer-cleanroom-volume/nan-order/`, 12 lines, every one identical)
    tells the two orders and the two loop orders apart: the spec's loop order (x outermost) with the new
    value first. Finite sums are the same either way.
  - **`MRIlabelsInNbhd` and `MRIlabelsInPlanarNbhd` take a voxel's label by `nint`** (the spec's "the
    label of a voxel in the means and counts"); every `linbhd` line identical, labels 2.5 and 1.4999
    among them.
  - The cases the probe does not reach, the implementation's choices (the crate's own tests):
    `MRIcomputeLabelNbhd` with values and no means counts the labels and reads no value; its label count
    is the counts array's length (FreeSurfer's negative count clears an enormous array).
- **How we know:** *validated* (`vimage`, `vnbhd`, `vrules9`, both modes, 2026-11-21, relay 13;
  `nan-order/`, the same day).

## §54: the inverses' fill and the changes between the voxel, RAS-to-RAS and physical-voxel types

- **What:** `vltapv` (135 calls: 110 `LTAchangeType`, 25 `LTAtransformBSpline` with physical-voxel
  arrays) and `vrules10` (65 fixed calls: the six changes with two transforms and with none, singular
  geometries, sizes of 0 and NaN, the validity checks, the targets -1, 3, 5, 12, 21 and 99, arrays of the
  types -1, 3, 5, 10, 11, 12 and 99, the diagnostic level 1) are identical with `reproduce` but one line
  where FreeSurfer did not finish; in `fix` every line is identical or corrected by a noted defect
  (FS-505, FS-506). The products of every change are FreeSurfer's bit for bit. Measured beyond the text:
  an outer product that gives nothing (its operands' sizes do not match: a transform of a 3 x 4 matrix)
  leaves the transform's matrix as it was: a 3 x 4 physical-voxel matrix changed to RAS-to-RAS prints
  both outer products' size errors, then the inverse's `(3 x 4) is not square` and the fill's end of the
  program. Relay 13 had counted that line with the changes not yet ready; it was this.
- **How we know:** *validated* (`vltapv`, `vrules10`, both modes, 2026-11-22, relay 14).

## §54: the changes to and from REGISTER_DAT, FSL and coronal RAS, the inversions, concatenations, reductions, tests and prints

- **What:** `vlta54` (225 seeded calls: 90 type changes of arrays of 1 to 3 transforms between six
  geometries from every type to every type, with `FSLOUTPUTTYPE` unset, `NIFTI_GZ`, `NIFTI` and
  `ANALYZE` and a diagnostic level of 1; 30 inversions; 40 concatenations, some through `LTAconcat2`;
  25 reductions; 20 source and target tests; 20 prints) and `vrules14` (94 fixed calls) are identical
  with `reproduce` but where FreeSurfer did not finish (10 lines) or read memory (7): FS-504's
  transforms an array does not hold, FS-508's freed matrix, FS-198's copies. `fix`: every other line
  identical or corrected by a noted defect (FS-195, FS-198, FS-504 to FS-508). The conversions'
  products (tkregister, native, FSL, surface RAS) are FreeSurfer's bit for bit, their messages
  (`FSLOUTPUTTYPE`, the determinants and flips) included. Measured with the probe, within the text:
  - the changes made through RAS-to-RAS or REGISTER_DAT are two whole calls, each with its own
    diagnostic print, then the outer call's: the change from FSL to voxels prints four matrices (FSL to
    REGISTER_DAT, REGISTER_DAT to RAS-to-RAS, the outer REGISTER_DAT to voxels, the outer FSL call);
  - the reduction compares each next transform with the previous transform's own destination, as §54
    writes it; comparing with the end of the chain reduced so far (the source of a transform taken
    inverted) gives the same 319 lines of `vlta54` and `vrules14`, so the probe's groups do not tell
    the two readings apart (a chain a to b, then c to b taken inverted, then c to d would): asked of the
    spec writer, who added two fixed cases to `vrules14` (relay 15) that confirm the previous
    transform's own destination.
- Choices beyond the probe (§54: the implementations' own tests): the error lines FreeSurfer has no
  counterpart for (FS-504's, FS-508's) end the program with status 252 where §54 says. FS-508 (as §54
  states it since commit `895c5c0d`): a conversion that gives no matrix marks the transform's matrix
  freed; the later transforms are converted (their messages and ends first), then the first read of the
  freed matrix ends the program: the diagnostic print when it is the first transform's, else the
  inverses' fill at that transform.
- **How we know:** *validated* (`vlta54`, `vrules14` of 109 calls, `vltapv`, `vrules10`, `vrules11`, both
  modes, 2026-11-22, relays 15 and 16; the defect tests in `tests/defects_volume.rs`).

## §55 to §57: segmentations onto another grid, SPM warps, segment statistics and shifts, crops

- **What:** `vaseg` (219 calls), `vsegs` (270), `vcrops` (95) and `vrules15` (70 fixed calls) are
  identical with `reproduce` in every line of the first three, and in `vrules15` but where FreeSurfer
  crashed (9 lines: FS-509's label past the table and empty place, the border frames written past the
  last, the concatenation and shifts written outside, FS-517) or read memory (a concatenation's frame past
  the second volume's last; FS-509's type of 100, which the loop lets differ in both modes). `fix`:
  every other line identical or corrected by a noted defect (FS-195, FS-509, FS-510, FS-512, FS-513,
  FS-515, FS-516, FS-517). Measured with the probe, where the text left a reading open:
  - with a colour table, every voxel's labels have their tissue types read (the warning for a type of
    -2 printed once per label of each voxel the map reaches, FS-509's checks there), not only in the
    voxels that vote;
  - `MRIaseg2volMU` maps onto a header of the caller's first transform's destination when the
    segmentation equals its source, else of its source (the case of a segmentation at neither end: 3
    lines of `vaseg`; stated in §55 since, with `vrules16`'s cases);
  - `MRIapplySpmWarp` with the volume the array's source: its forward matrix inverted on its own
    (`MatrixInverse`), the caller's array left as it was (its inverse not filled). Relay 16 read the
    1-based vox2ras from the array's first source in every case (6 lines of `vaseg` agreed); `vrules16`
    (warp points placed through the array's other end) showed the amended §55's reading: SPM's input
    space is the end that is not the volume (its destination when the volume is its source), and the
    clean room follows it since relay 17 (`vaseg` and `vrules16` identical);
  - `MRIcropAroundRAS`'s fields of view: relay 16's per-RAS-axis reading matched `vcrops`; the amended
    §57 registers FreeSurfer's assignment as FS-268 (`SRA` with `(4 1 3)`: FreeSurfer's `(1 3 4)`, by
    default `(3 4 1)`, each voxel axis taking the entry its letter names), FreeSurfer's later write kept
    where an orientation names a direction twice (FS-515 under `reproduce`: `RRS` with `(4 1 3)` gives
    `(1 0 3)`, the field never set 0 in all four of FreeSurfer's runs); the default ends the program on
    such an orientation (status 252).
- Choices beyond the probe (the implementations' own tests): FS-509's, FS-515's (default) and FS-517's
  errors end the program with status 252 (FS-517 also where a last voxel is exactly `int`'s maximum,
  after the new volume's `P0` and `c_ras` lines, as the amended §57 states); FS-511: a refused trilinear
  sample writes nothing and the voxel takes the call's last written value, the nearest sampler's refusal
  writes 0; before any value is written 0 in both modes, noted (`vrules16`'s tensor and RGB volumes);
  FS-267's tissue type of the previous vote is kept in the process (the defect test runs two calls in
  one process, `a2vtwice`).
- **How we know:** *validated* (`vaseg`, `vsegs`, `vcrops`, `vrules15`, `vrules16`, both modes,
  2026-11-23, relay 17; the defect tests in `tests/defects_volume.rs`).

## Part 10a: the Talairach helpers, dilation and the segment helpers (§46 to §48)

- **What:** `vtal` (240 calls), `vdil` (270) and the fixed group `vrules20` (111) are identical with
  `reproduce` in every line but the three where FreeSurfer crashed (FS-487's points through a matrix that
  could not be made, `MRIvoxelToTalairachEx` and `MRItalairachToVoxelEx` with a 3 x 3 transform matrix;
  FS-488's `MRIfromTalairachEx` without a target), where the candidate gives the spec's error and end.
  `fix`: every other line identical or corrected by a noted defect (FS-487, FS-488, FS-489, FS-490, FS-12,
  and the earlier parts' FS-01, FS-325, FS-52, FS-53). `vrules20`'s fixed cases of part 9 (the room after
  the box of `REGIONgetBoundingBoxEqOdd`; the orders a target or moving volume of width 0 shows in
  `MRIvol2VolTLKernel`, `MRIvol2VolDelta`, `TransformRegDat2LTA` and `MRIvol2VolTkRegVSM`) agree in both
  modes after the code took the orders §41 and §56 now state; `models.py region` takes its `rgbb` line too
  (185 lines, none different).
- Readings the probe decided, where the spec's text allowed two: `ModifyTalairachCRAS` checks the
  array's own type (`LINEAR_RAS_TO_RAS`), not its transform's (the probe's arrays of type 1 hold
  transforms of type 0, and their destination's centre is taken); the Talairach and destination
  volumes' RAS-to-voxel matrices are computed from their geometries (an inverse that fails gives none
  silently, so the product's `m1 is null` ends the program, as `vrules20`'s singular volumes show);
  `MRIsegStats`' first value starts `min` and `max`, so a NaN first value leaves both NaN (with the
  range, mean and spread). Not decided by the probe (every outside value it uses is 0): `MRIdilate`'s
  neighbours past the volume's faces are sampled as the nearest sampler gives them (the outside value)
  rather than clamped as `MRIdilateUchar`'s are.
- **How we know:** *validated* (the three groups, both modes, 2026-11-23, relay 18, log
  `oracles/logs/fs-cleanroom-2-relay18-full.log`; the defect tests in `tests/defects_volume.rs`).

## Part 9: volume-to-volume resampling, covariance and PCA, the false discovery rate, regions, voxel neighbourhood measures (§41 to §44)

- **What:** `vv2v` (415 calls), `vregion` (160), `vcov` (410), `vvoxel` (420) and the fixed groups
  `vrules17` (45), `vrules18` (29) and `vrules19` (49) are identical with `reproduce` but where
  FreeSurfer crashed (FS-270's segmentations at neither end, FS-484's PCA without frames, the kernel's
  writes past a target's frames) or read memory (masks of another extent, frames past the volume's,
  FS-323's look-ups past the index tables); every line of `vregion` and `vrules18` identical. `fix`:
  every other line identical or corrected by a noted defect (FS-269, FS-483, and the earlier parts'
  FS-01, FS-12, FS-52 to FS-54, FS-295, FS-298). The open probe's models agree with the candidate in both modes: `models.py
  region` (every `rgbb` line of `vregion`, `vrules17` and `vrules18`, 184: FreeSurfer's boxes with
  `reproduce`, FS-269's decided boxes with `fix`) and `models.py cov` (111 covariance lines: FreeSurfer's
  count with `reproduce`, FS-483's with `fix`); `vrules18`'s `MRImaskAndUpsample` cuts are §41's (with
  `reproduce` 1 x 1 x 0, 2 x 2 x 0, 3 x 3 x 2, 6 x 6 x 4 and 3 x 3 x 2 voxels; by default 2 x 2 x 1,
  4 x 4 x 2, 3 x 3 x 2, 6 x 6 x 4 and the whole 4 x 4 x 3 source).
- Readings where the text left a choice and the probe agreed (relay 17; the spec states each since
  `3c05edce`): `MRIvol2VolVSM`'s four map corners on the voxel's rounded slice, a corner skipped where
  `|v| < FLT_MIN` (a NaN corner kept); `MRImaskAndUpsample`'s diagnostic ratio is the source's voxel count
  over the box's (`float`; relay 18 took the source's extent multiplied in single precision, as the spec
  now says); `PrintPCAStats` puts two spaces between its two percentages.
- Choices the probe does not decide (the implementations' own tests): `REGIONgetBoundingBoxEqOdd`'s
  fallback compares the start with the room after the box of the smaller side's own size (its four
  `AsymRegion` cases agree either way); `MRIvoxelDirection`'s sixth term pattern takes the fifth's
  `t1`; `MRIfdr2vwth` with a mask list skips the mask of an input that has none. FS-485 (not in the
  probe): the default's full window, through the index tables past the far faces, equals an
  independent computation of §44's text on an exact `float` volume at a far corner and a far face; with
  `reproduce` FreeSurfer's cut window read back as a full cube gives another value; noted in both modes
  only where a window passes a far face. FS-270's and FS-484's ends (status 252) and FS-482's shift never
  written (0 in both modes) as §41, §42 and §55 state.
- **How we know:** *validated* (the seven groups, both modes, and both models, 2026-11-23, relay 17; the
  defect tests in `tests/defects_volume.rs`).

## Part 8: the partial-volume count, random and constant volumes, frames, orders, fractions, difference lists and voxelwise operations (§37 to §39)

- **What:** `vpvlab` (90 calls), `vfr8` (490), `vmri2` (390), `vrules12` (112) and `vrules13` (13) are
  identical with `reproduce` but where FreeSurfer did not finish or read memory, and 3 `vpvlab` lines
  that §69.3 now allows in both modes: in each, a border voxel of another label carries a label outside
  the label table (-1 in one, 250 in two); §37's FS-469 has the clean room use that label's mean over
  the wide window in both modes, where FreeSurfer reads a mean beside its table, the same in its four
  runs. With those labels written as 0 the probe and the candidate give the same line in all three.
  `vrules13`'s six lines where FreeSurfer finishes (FS-471's limits -1 to -3 off the border, a volume
  without the label) are identical in both modes. `fix`: none unexplained (FS-12, FS-469, FS-472 to
  FS-481 and the shared ones noted). Measured beyond the text:
  - `MRIhalfCosBias`: where the weight and the voxel are both NaN (a volume of one slice: the weight is
    0 / 0), the result keeps the voxel's NaN (payloads `7fc12345` and a quieted `7f800001` kept): the
    product is computed as `v w`.
  - The probe's `tiv` does not reset `errno` before `MRIestimateTIV`: its removal of a transform file
    that is not there leaves `ENOENT`, which the transform reader's C-style errors print first (`error:
    No such file or directory`); and a call that ends the program leaves the file for the later lines.
    The candidate does the same (the clean room's `MRIestimateTIV` is unaffected).
  - Beyond the probe, as §37 to §39 now state them (relay 15; the implementations' own tests): FS-470
    (no intensity volume) ends the program at the first border voxel weighed, after its two windows,
    naming the voxel; FS-471 (a negative label limit) ends it at the first counted voxel on the border,
    or at the first counted voxel from -4 down, while -1 to -3 count the voxels off the border as
    FreeSurfer does; FS-471 is noted once, at the first counted voxel, for tables of more than 8 MiB (12
    bytes per label) and counted all the same; FS-479 (an array without a first linear transform) ends
    the program; `mri_framepower` with fewer powers than frames ends it before any frame changes (rule
    B); all status 252 with the spec's error lines, in both modes. `MRInormalizeSequence` reports an
    unsupported type at the first frame of the first voxel (a volume without frames returns
    `NO_ERROR`); §37's write of the border volume to `b.mgz` under the diagnostic bits waits for fs.io's
    clean room (a volume writer): not yet made.
- **How we know:** *validated* (`vpvlab`, `vfr8`, `vmri2`, `vrules12`, `vrules13`, both modes,
  2026-11-22, relays 14 and 15; FS-469's three lines with the labels rewritten, relay 14).

## The cubic B-spline paths (§29 step 5, §31 step 4)

- **What:** `MRIlinearTransformInterp` and `MRIapplyRASlinearTransformInterp` with method 5 and
  `MRIresampleFill` with type 5 sample the source's cubic B-spline (fs.bspline): `vbsvol`'s 70 calls are
  identical with `reproduce` but 4 where FreeSurfer did not finish (sources without their cached
  matrices, FS-252); in `fix` 36 are corrected by a noted defect, none other.
- **How we know:** *validated* (`vbsvol`, both modes, 2026-11-19, relay 12).

## Speed (part 4)

- **What:** the spec's plan names no speed measure; measured anyway on the samplers and the head's
  box, the probe and the candidate each running one batch line end to end (process start, parsing
  and printing included; best of 3, one thread, the reference machine): 20,000 points on a 32^3
  float volume, trilinear 0.020 s against FreeSurfer's 0.030 s (1.5x), cubic 0.032 s against 0.038 s
  (1.2x), nearest 0.018 s against 0.028 s (1.5x), windowed sinc of half-width 3 0.047 s against
  0.036 s (0.75x: its time is the `sin` and `cos` ports of the main package, glibc's under
  `reproduce`, two per weight); the head's box of a 64^3 byte volume 0.0047 s against 0.0053 s.
  Results identical on every line. Reproducer: `<scratch>/vspeed/mk.py` and `run.sh` (relay 8's
  NOTES).
- **How we know:** *measured* (2026-11-17).

## Speed (part 6a)

- **What:** §26 names no speed measure; measured on 64^3 volumes, one batch line each end to end
  (best of 3, one thread, the reference machine; `validation/investigations/freesurfer-cleanroom-volume/speed/`,
  every result line identical): the flood fill of every voxel 0.013 s against FreeSurfer's 0.029 s
  (2.3x: when every byte lies inside both volumes the clean room walks the seed's component on a
  byte copy, which fills the same voxels as the sweeps), with a threshold 0.011 s against 0.017 s
  (1.5x); `MRIdownsampleN` by 2 0.010 s against 0.011 s; `MRIdownsample2` 0.0057 s against 0.0049 s
  (0.85x); `MRIupsampleN` of 32^3 by 2 0.041 s against 0.054 s (1.3x); `MRIchangeType` of 48^3
  floats to bytes with its histogram 0.027 s against 0.031 s; `MRIlimits` 0.0045 s against 0.0050 s.
- **How we know:** *measured* (2026-11-18).

## Speed (parts 6b and 6c)

- **What:** §30 and §32 name no speed measure; measured anyway, one batch line each end to end (process
  start, parsing and printing included, about 0.004 s; best of 3, one thread, the reference machine;
  `validation/investigations/freesurfer-cleanroom-volume/speed/`, every result line identical):
  `MRIsampleSeqVolume` at 20,000 points of a 32^3 float volume 0.0225 s against FreeSurfer's 0.0268 s
  (1.19x); splatting 20,000 values into it 0.0273 s against 0.0377 s (1.38x); the directional
  derivative at 20,000 points 0.0450 s against 0.0473 s (1.05x); the trilinear linear transform of the
  volume by a rotation 0.0102 s against 0.0106 s, the nearest one of a 32^3 byte volume 0.0049 s against
  0.0056 s (1.14x); `MRIresampleFill` onto a 40^3 template of 0.8 mm voxels, trilinear 0.0181 s against
  0.0178 s (0.98x), nearest on bytes 0.0074 s against 0.0080 s (1.09x). The changes that brought these
  (relay 11, results unchanged): `MRIsetVoxVal`'s element in a stack buffer, the 4 x 4 matrix's
  product with a point without allocation, the value replacements of byte and float volumes in one
  walk of the buffers, the nearest resampling's one corner, the stores straight into the buffer.
- **How we know:** *measured* (2026-11-18, relay 11).

## Speed (part 7b)

- **What:** §36 names no speed measure; measured anyway, one batch line each end to end (process start,
  parsing and printing included; best of 3, the probe at one OpenMP thread, the reference machine;
  `speed/mk.py part7b`, every result line identical): `MRIsmoothParcellation` of a 64^3 volume of shorts
  (labels 0 to 5, count 13) 0.037 s against FreeSurfer's 0.230 s (6.2x); `MRImaskZero` of 64^3 floats
  0.038 s against 0.055 s (1.45x); `MRIclipBrightWM` of 64^3 bytes 0.0148 s against 0.0174 s (1.17x); a
  256 x 256 slice as an image 0.0152 s against 0.0193 s (1.27x); `MRImeanAndStdInLabel` of 64^3 floats
  0.0060 s against 0.0055 s (0.92x); 5,000 non-zero counts in windows of 7^3 0.0132 s against 0.0106 s
  (0.80x); `MRImarkLabelBorderVoxels` of 48^3 bytes 0.026 s against 0.016 s (0.61x). The in-range
  reads through `Mri::inside_vox_val` and the byte reads without allocation brought the last four up
  from 0.8x, 0.46x, 0.9x and 0.44x (results unchanged); the label borders and window counts still
  walk every neighbour through the index tables and the `(int)` checks.
- **How we know:** *measured* (2026-11-21, relay 13).

## Speed (part 8)

- **What:** §40 names no speed measure; measured anyway, one batch line each end to end (best of 3, the
  probe at one OpenMP thread; `speed/mk.py part8`, every result line identical): the partial-volume
  count of a 24^3 byte volume 0.019 s against FreeSurfer's 0.011 s (few border voxels: the process start
  dominates); `MRInormalizeSequence` of 48^3 floats x 8 frames 0.192 s against 0.178 s (0.92x);
  `MRIsort` of the same 0.126 s against 0.175 s (1.39x); `MRIvote` of 48^3 bytes x 9 frames 0.062 s
  against 0.073 s (1.18x); `MRIrms` 0.029 s against 0.031 s; `MRInonMaxSuppress` of 48^3 floats 0.034 s
  against 0.032 s; `MRIsquareRoot` of 64^3 floats 0.046 s against 0.055 s (1.20x); `mri_framepower` of
  64^3 floats (power 2, `powf`) 0.069 s against 0.054 s (0.78x, measured while both `powf`s ran for every voxel;
  now only the mode's once GLIBC-07 is noted); `MRIhalfCosBias` 0.047 s against 0.053 s (1.13x).
- **How we know:** *measured* (2026-11-22, relay 14).

## Golden cases and platforms

- **What:** `tests/golden_volume.rs`, every command of the probe's grammar on seeded arguments in
  both modes (66 SHA-256 over 33 groups: part 4's samplers, moves, head's box and transform reader,
  the transform files in memory, several in one process; part 5's labels, segmentation functions
  and the statistics cache's text and reading in memory; part 6a's 11 commands; part 6b's and 6c's
  13, the diagnostic bits' prints included; part 7a's 11, fs.bspline's 10 and the cubic B-spline
  paths; part 7b's 16 commands and §54's changes of type; part 8's 32 commands but `tiv`, which reads files and is in the defect tests; §54's other operations, `lta54`, over pools of three geometries with mostly invertible matrices; §55 to §57's 18 commands, `segs57`, on volumes of labels with ordinary geometries; part 9's 16 commands, `part9`, with §55's upsampling and §56's shift-map resampling; part 10a's 9 commands, `part10`, with `NO_AVERAGE305` and the diagnostic bits; 0.24 s in a debug build), identical in debug and release builds on x86-64 and under the
  AArch64 NaN emulation of both (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`).
- **How we know:** *measured* (2026-11-23, relays 10 to 18).
