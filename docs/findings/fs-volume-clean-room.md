# fs.volume's clean room: what black-box runs showed (unrestricted)

The clean room of FreeSurfer 8.2.0's volume functions, parts 1 to 4 of the spec
(`crates/larmorx-fs-core/src/volume/`, the rest of queue item C3.1-05), was written from
[`specs/freesurfer-volume.md`](../../specs/freesurfer-volume.md) §1 to §22 and checked only against
black boxes: the prebuilt probe `fs_volume_probe` (FreeSurfer's compiled library and the C library,
glibc 2.35) run through the open folder
[`validation/investigations/freesurfer-volume-probe/`](../../validation/investigations/freesurfer-volume-probe/README.md).
Nothing here comes from FreeSurfer's source; this file is open to everyone. The comparisons and the
tools are in
[`validation/investigations/freesurfer-cleanroom-volume/`](../../validation/investigations/freesurfer-cleanroom-volume/README.md).

## The comparison

- **What:** the 15 groups of the spec's §69.2 (9,489 calls, seed 7: parts 1 to 3's ten, the fixed
  group `vrules` and part 4's `vsample`, `vmove`, `vskull`, `vltaread`), on the probe (plainly,
  again, and in two guard runs; every reference hash the spec's) and on the clean room's candidate,
  in both modes. `reproduce`: 8,753 identical, 647 where FreeSurfer did not finish, 89 memory lines,
  none other. `fix`: 6,342 identical, 647 not finished, 396 memory, 2,104 corrected by a noted
  defect, none other. Every line of `vgblock`, `vgcopy`, `vcoords`, `vheader` and `vskull` is
  identical in `reproduce`; part 4's groups have no memory lines that differ. fs.mri's 16 groups
  (9,511 calls, its new group `novox` included) likewise: none unexplained in either mode.
- **How we know:** *validated* (`compare.py compare`, both modes, 2026-11-17, relay 8).

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

## Part 4: behaviours the spec's text does not state (observed)

- **`MRIcubicCoeff` with an index outside 0 to 3** gives -3 (`c008000000000000`), not 3, after its
  error (`coef`, 10 lines).
- **`MRIcubicSampleVolumeFrame`'s window points outside the volume** (FS-300) read as the double of
  all bits set (`ffffffffffffffff`, a NaN): on every such line of `vsample` (7 lines) the result is
  that NaN, the same in the plain run, the run with the stack below the call zeroed and both guard
  runs. The clean room's `reproduce` adds that value there (deterministic; the official build's
  memory contents as observed); the default adds 0 (the spec's text), both noted.
- **`ham_sinc`'s last product** keeps the window's NaN where both factors are NaN (`hsinc` of -inf
  with a NaN full width: `7ff8000000000000`, the full width's NaN): the window is the left operand.
  And an infinite argument of `sin` or `cos` gives x86-64's default NaN (`fff8000000000000`), as
  §18 says, in both modes (larmorx's correctly rounded `sin` and `cos` give a positive NaN there,
  so the clean room gives the default NaN itself).
- **`MRIrotate_I`** sets the destination's `ras_good_flag` to 0 (as `MRItranslate`; §19 says it
  only of the axis rotations). Its vector, product and point are 3 x 1 matrices made once: a
  rotation matrix of another size (2 x 2, 4 x 4) gives `MatrixMultiply`'s size error once per voxel,
  the product stays 0 and every voxel is sampled at the centre (`vmove`, 3 lines).
- **Reading an `.xfm`:** the rows are numbered from 1 in `ltMNIreadEx: could not read row <r> from
  <name> (<last line read>)`; the named volumes are looked up after the rows (a cut file leaves
  `errno` 0); a `%Generated by` second line names volumes only through its `src` and `dst` words
  (neither its `minctracc` nor the second-word rule applies then, and the third line is not read
  for a name); `USE_AVERAGE305` applies only when the second line is an information line (a file
  whose second line is `Transform_Type` gets no average geometry and no INFO lines). Every
  `minctracc` line of the probe is a `%Generated by` line, so the clean room's `minctracc` rule
  (§21's text: the first two words holding `.mnc`, the source and the destination; with one, no
  destination) is not checked by the probe.
- **The transforms the MINC, FSL and register.dat readers make** have `sigma` 1 (an `.lta`'s is
  the file's, an allocation's 10,000) and, for MINC and FSL, `fscale` 0; `identity.nofile`'s
  transform is RAS-to-RAS itself (type 1), with the allocation's `sigma`.
- **`TransformFileNameType`:** the `#` cut applies to an `@` part too (`a.lta@xfm#y` is MINC); an
  empty extension (a name ending in `.` or `@`) gives REGISTER_DAT, a name without a `.` an LTA.
- **A `register.dat` that cannot be opened** leaves `errno` EINVAL (22) after
  `regio_read_register(): No such file or directory` (the probe's standard error is not oriented),
  so `ltaReadRegisterDat`'s error starts with `Invalid argument`.
- **An `.lta` whose type or count does not match** (FS-308): in the probe's plain runs the unset
  values were 0 (memory lines); the clean room's `reproduce` takes 0, its default refuses the file
  (`ltaReadFile(<name>): can't read data`).
- **How we know:** *measured* (`vsample`, `vmove`, `vltaread`; `models.py skull` of the open probe
  folder agrees with the clean room's walks on all 160 `vskull` lines).

## Part 4: the defects' modes (D13)

| Defect | Default | `reproduce` |
|---|---|---|
| FS-298 | a 64-bit volume's own values | 32-bit integers at the voxels' places |
| FS-299 | the cubic sum divided by 8 | divided by the points added |
| FS-300 | outside window points 0 | the official build's all-ones NaN (observed) |
| FS-301 | a moved sample written as the destination's type stores it (§5) | its byte at the voxel's byte place |
| FS-302 | a window over 128 voxels: an error, `ERROR_BADPARM`, the value unchanged | the same |
| FS-303 | a sample the sampler refuses: the one before (0 at first) | the same (FreeSurfer's first is memory contents) |
| FS-304 | an axis rotation without an inverse: no volume after the error | the same (FreeSurfer crashes) |
| FS-305 | a centre of mass that is not a number: the program ends | the walks as FreeSurfer's (runs outside taken at once); the end where it never returns |
| FS-306 | a name of 500 bytes or more classified whole | the same (FreeSurfer writes past its memory) |
| FS-307 | the `%` lines' skip ends at the end of the file | the same (FreeSurfer never returns) |
| FS-308 | an `.lta` without its two header lines refused | the unset values 0 |
| FS-309 | an `.lta` of one line refused | the same (FreeSurfer crashes) |
| FS-310 | a `src`/`dst` word last on its line names no volume | the same (FreeSurfer crashes) |
| FS-258 | a register.dat name of 1,000 bytes kept whole | the same |

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
  both modes (28 SHA-256 over 14 groups, part 4's samplers, moves, head's box and transform reader
  included, the transform files in memory; 0.09 s in a debug build), identical in debug and release
  builds on x86-64 and under the AArch64 NaN emulation of both
  (`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`).
- **How we know:** *measured* (2026-11-17).
