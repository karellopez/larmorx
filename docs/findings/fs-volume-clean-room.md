# fs.volume's clean room: what black-box runs showed (unrestricted)

The clean room of FreeSurfer 8.2.0's volume functions, parts 1 to 5 of the spec
(`crates/larmorx-fs-core/src/volume/`, the rest of queue item C3.1-05), was written from
[`specs/freesurfer-volume.md`](../../specs/freesurfer-volume.md) §1 to §24 and checked only against
black boxes: the prebuilt probe `fs_volume_probe` (FreeSurfer's compiled library and the C library,
glibc 2.35) run through the open folder
[`validation/investigations/freesurfer-volume-probe/`](../../validation/investigations/freesurfer-volume-probe/README.md).
Nothing here comes from FreeSurfer's source; this file is open to everyone. The comparisons and the
tools are in
[`validation/investigations/freesurfer-cleanroom-volume/`](../../validation/investigations/freesurfer-cleanroom-volume/README.md).

## The comparison

- **What:** the 17 groups of the spec's §69.2 (9,960 calls, seed 7: parts 1 to 3's ten, the fixed
  groups `vrules` and `vrules4`, part 4's `vsample`, `vmove`, `vskull`, `vltaread` and part 5's
  `vcma`), on the probe (plainly, again, and in two guard runs; every reference hash the spec's,
  the probe of commit `9319981f`) and on the clean room's candidate, in both modes. `reproduce`:
  9,194 identical, 660 where FreeSurfer did not finish, 99 memory lines, 7 `vsample` lines noting
  FS-300 (§69.3 allows them in both modes), none other. `fix`: 6,697 identical, 660 not finished,
  389 memory, 2,214 corrected by a noted defect, none other. Every line of `vgblock`, `vgcopy`,
  `vcoords`, `vheader`, `vskull` and `vrules4` is identical in `reproduce`. fs.mri's 16 groups
  (9,511 calls) and fs.maths' 24 (28,708 calls, the new fixed batches `melements-nan` and
  `mvectors-nan` included) likewise: none unexplained in either mode.
- **How we know:** *validated* (`compare.py compare`, both modes, 2026-11-17, relay 9; log
  `oracles/logs/fs-cleanroom-2-relay9-cmp.log`).

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
  same in all four probe runs (`nswaps 0`, no voxel swapped). §23 says only "an error here"; the
  clean room's default refuses such lists (an error, no volume) and its `reproduce` takes the
  numbers past a list's end as NaN, equal to no label (D13's deterministic fill), which gives those
  two lines. Reported to the spec writer.
- **`ReadCachedBrainVolumeStats` without a cache** prints `computing (and caching) brain volume
  statistics` and calls the caller's `ComputeBrainVolumeStats`, which the spec leaves out (it reads
  a subject's surfaces): the probe's one such line ends there with status 255
  (`FREESURFER_HOME` is not defined). The clean room takes that function from its caller; the
  candidate's stand-in prints the probe's two lines.
- **How we know:** *validated* (`vcma`, both modes, 2026-11-17, relay 9; every `cmalab` line, 95,
  identical, the clean room's label tests written from §23's tables).

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

## Golden cases and platforms

- **What:** `tests/golden_volume.rs`, every command of the probe's grammar on seeded arguments in
  both modes (30 SHA-256 over 15 groups: part 4's samplers, moves, head's box and transform reader,
  the transform files in memory, several in one process; part 5's labels, segmentation functions
  and the statistics cache's text and reading in memory; 0.05 s in a debug build), identical in
  debug and release builds on x86-64 and under the AArch64 NaN emulation of both
  (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`).
- **How we know:** *measured* (2026-11-17, relay 9).
