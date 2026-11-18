# fs.volume's clean room: what black-box runs showed (unrestricted)

The clean room of FreeSurfer 8.2.0's volume functions, parts 1 to 6c of the spec
(`crates/larmorx-fs-core/src/volume/`, the rest of queue item C3.1-05), was written from
[`specs/freesurfer-volume.md`](../../specs/freesurfer-volume.md) §1 to §32 and checked only against
black boxes: the prebuilt probe `fs_volume_probe` (FreeSurfer's compiled library and the C library,
glibc 2.35) run through the open folder
[`validation/investigations/freesurfer-volume-probe/`](../../validation/investigations/freesurfer-volume-probe/README.md).
Nothing here comes from FreeSurfer's source; this file is open to everyone. The comparisons and the
tools are in
[`validation/investigations/freesurfer-cleanroom-volume/`](../../validation/investigations/freesurfer-cleanroom-volume/README.md).

## The comparison

- **What:** the 24 groups of the spec's §69.2 (12,270 calls, seed 7: parts 1 to 3's ten, the fixed
  groups `vrules`, `vrules4`, `vrules5` and `vrules6`, part 4's `vsample`, `vmove`, `vskull`,
  `vltaread`, part 5's `vcma`, part 6a's `vres`, part 6b's `vframes`, `vderiv`, `vlinear` and part
  6c's `vresamp`), on the probe (plainly, again, and in two guard runs; every reference hash the
  spec's, the probe of commit `20578b55`) and on the clean room's candidate, in both modes.
  `reproduce`: 11,310 identical, 799 where FreeSurfer did not finish (9 of them FS-314's aborts,
  which the candidate gives too and the loop checks), 152 memory lines, 7 `vsample` lines noting
  FS-300 (§69.3 allows them in both modes), and 2 lines that differ where FreeSurfer uses a value it
  never wrote that happens to be the same in the probe's four runs (FS-325 in `vlinear`, FS-300 in
  `vresamp`: below; both noted by the candidate). `fix`: 8,321 identical, 799 not finished, 508
  memory, 2,642 corrected by a noted defect, none other. Every line of `vgblock`, `vgcopy`,
  `vcoords`, `vheader`, `vskull`, `vrules4` and `vframes` but its 3 crashes is identical in
  `reproduce`. fs.mri (16 groups, 9,511 calls) was run again after relay 11's change of
  `MRIsetVoxVal`'s buffer: none unexplained in either mode; fs.maths was not touched.
- **How we know:** *validated* (`compare.py compare`, both modes, 2026-11-18, relay 11; log
  `oracles/logs/fs-cleanroom-2-relay11-fullb.log`).

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
  `reproduce` but where FreeSurfer did not finish (54), the memory lines (4) and the two lines below;
  in `fix` 267 lines are corrected by a noted defect (FS-320 to FS-328, FS-461, FS-298 and the voxel
  access's). Measured beyond the text:
  - **`MRIindexNotInVolume` rounds the double:** its "ambiguous" test (§15: the coordinates rounded
    to the nearest integers, ties to even, as floats, in `[0, extent)`) takes the double's rounding,
    then the float: a sample at row 2.500000119 of a volume 3 rows high is unambiguously outside
    (`vderiv`'s line 81: the derivative's sample at `y + 1.5` gives `outside_val`; rounding the float
    first, 2.5 to 2, would sample it and give 18.2116 for the probe's 7.7501). Every earlier group is
    identical either way (no earlier line has a coordinate within half a float step of a half).
  - **FS-325's value never written:** an `lxf` with the sinc method into an integer destination
    (`vlinear`'s line 235: every sample fails at points that are not whole) stores FreeSurfer's never
    written double, which is -1 (stored as `0xffffffff`) in all four of the probe's runs, so the line
    is not a memory line; the clean room stores 0 as §29 states (FS-325, noted in both modes): the
    one `vlinear` line that differs under `reproduce`.
  - **FS-300 in a cubic resampling:** `MRIresampleFill` with `SAMPLE_CUBIC` reads the cubic sampler's
    window points FreeSurfer never reads (`vresamp`'s line 31: one voxel 0x7f in the probe, 0x8d in the
    clean room, whose `reproduce` takes the double of all bits set there, §18); the probe's four runs
    agree there by accident of its stack, as in `vsample`, where §69.3 allows such lines; `vresamp`'s
    one differing line under `reproduce`.
  - **FS-326 is not reached by the probe:** no `vresamp` line has a label with more than 255 votes in
    one voxel (the 8 x 8 x 8 sources' fills spread their labels), so no line notes it in either mode;
    the crate's own test gives 280 and 320 votes (`tests/defects_volume.rs`).
- **How we know:** *validated* (`vframes`, `vderiv`, `vlinear`, `vresamp`, both modes, 2026-11-18,
  relay 11).

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

## Golden cases and platforms

- **What:** `tests/golden_volume.rs`, every command of the probe's grammar on seeded arguments in
  both modes (40 SHA-256 over 20 groups: part 4's samplers, moves, head's box and transform reader,
  the transform files in memory, several in one process; part 5's labels, segmentation functions
  and the statistics cache's text and reading in memory; part 6a's 11 commands; part 6b's and 6c's
  13, the diagnostic bits' prints included; 0.05 s in a debug build), identical in
  debug and release builds on x86-64 and under the AArch64 NaN emulation of both
  (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`).
- **How we know:** *measured* (2026-11-18, relays 10 and 11).
