# FreeSurfer 8.2.0's volume structure, as a black box

The probe behind the **(observed)** marks and the validation plan of
[`specs/freesurfer-mri.md`](../../../specs/freesurfer-mri.md) (`fs.mri`: the geometry and its
matrices, the volume, its allocation, voxels, header and colour-table copies, the tkregister and FSL
matrices, world and voxel coordinates, the geometry's printing and comparisons, the interpolation
and precision names, the full copy, the volume's MINC transform, shapes, the bounding box and the
linear transform arrays). **Open to the clean-room implementer of that spec**: the files here hold
only calls of FreeSurfer's functions (and of the C library's) by name, the reading and printing of
their arguments, results and the structures' fields, and a comparison loop; no FreeSurfer code,
nothing written from its source. The implementer runs the prebuilt probe (below) and the comparison
loop; building the probe reads the unit oracle's build tree, which is closed to FreeSurfer's
clean-room implementers, so they never build it.

| File | What it is |
|---|---|
| `fs_mri_probe.cpp` | the probe: a batch of calls from a file, each in a child process of its own; its commands below |
| `build.sh` | builds it with the unit oracle's compiler (Ubuntu's GCC 8.4.0) and FreeSurfer's flags against `<workspace>/oracles/freesurfer-8.2.0-unit` (queue item 3.1-02): `build.sh <workspace> <output>` (the spec writer's or the orchestrator's step, not the implementer's) |
| `compare.py` | the comparison loop of the spec's §23: 15 groups of seeded cases (9,245 calls in 22 batches), run on the probe (four times: plainly, plainly again in another process and in two guard runs, through the oracle-output cache) and on a candidate program that takes the same commands; `list`, `refs`, `compare` |

The prebuilt probe: `<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_mri_probe`; its guard runs
use the fs.maths probe's guard allocator, `guard_malloc.so` beside it
([`../freesurfer-maths-probe/`](../freesurfer-maths-probe/README.md), "The guard runs"). Their
builds (the larmorx commit, the `build.sh`, the SHA-256s) are recorded in that folder's
`BUILD_INFO.txt` for the orchestrator's check; the implementer only runs them (every other access to
the unit oracle's folder, reading or hashing a file in it included, counts as opening FreeSurfer's
build tree).

## Running it

```sh
fs_mri_probe batch <file>      # or - for standard input
```

Each line of the file is one call: `[@<seconds>] [+gdiag=<hex>] <command> <arguments>`. The probe
reads every line first, then runs each in a child process of its own, in the working directory,
with standard output and standard error captured and an alarm of 2 seconds (`@<seconds>` sets
another). `+gdiag=<hex>` sets FreeSurfer's diagnostic bits before the call (`10000`: `DIAG_INFO`;
`10000040`: `DIAG_SHOW` and verbose). One line is printed per call (an empty line for an empty one):

```
<result> | <standard output> <standard error>
```

the two streams as strings (below). `<result>` is the command's printed values, or `EXIT <status>`
when FreeSurfer's code ended the program (its error functions; the streams hold its messages),
`CRASH <signal>` when it died of a signal, `TIMEOUT` when the alarm ended it. With `MALLOC_PERTURB_`
set, the child fills 256 KiB of its stack with that byte before the call.

## Encodings

| Kind | Form |
|---|---|
| integer | decimal |
| float | the 8 hex digits of its IEEE 754 bits (`3f800000` is 1.0) |
| double | the 16 hex digits of its bits (`3ff0000000000000` is 1.0) |
| string | `x` followed by the hex digits of its bytes (`x` alone is the empty string); a field of a fixed size takes at most its size less one byte; a missing string prints `null` |
| M, a matrix | `null`, or `<rows> <cols>` and the rows × cols floats row by row (a real matrix); the same printed. A returned matrix that is the given one itself prints `given` first |
| G, a geometry | `valid width height depth xsize ysize zsize ras_good_flag x_r x_a x_s y_r y_a y_s z_r z_a z_s c_r c_a c_s s_r s_a s_s name` (integers, floats, a string). Printed: the same fields (a volume's own geometry without `valid`, which no allocation sets), then the cached voxel-to-RAS matrix (`cache` and its 16 floats as stored, column by column, or `null`), the cached RAS-to-voxel matrix (M) and the registration matrix (M) |
| Gf | G, a cache flag (1: the cache computed from the fields, as `MRIreInitCache`; 0: none) and a registration matrix (M) |
| V, a volume | `<width> <height> <depth> <type> <nframes> <h>`: allocated as the spec's §4 says, with voxels (`h` 0, `MRIallocSequence`) or without (`h` 1, `MRIallocHeader`). Types: 0 uchar, 1 int, 2 long, 3 float, 4 short, 5 bitmap, 6 tensor, 7 float complex, 8 double complex, 9 RGB, 10 ushort |
| V G | a volume, then its geometry's fields set from G (its cached matrices left as the allocation made them) |
| fill seed | the voxels' byte i set to (seed + 101 i) mod 256; -1 leaves them as allocated |
| voxels (printed) | `novox` for a volume without voxels, else the string of its voxels' bytes (element `c + r W + s W H + f W H D`, each little-endian) |
| volume (printed) | the header: width height depth nframes type, the bytes per voxel, bytes in all, voxels per row, slice, volume and in all, `imnr0` `imnr1`, `xstart` `ystart` `zstart` `xend` `yend` `zend` `fov` `thick` `ps` (floats), `scale`, the region's extent (dx dy dz), `version` `intent` `dof` `brightness` `yinvert` `ptype`, `outside_val` `flip_angle` (doubles), `tr` `te` `ti` `location` `FieldStrength` (floats), the phase-encode direction (string), `bvec_space`, b-values (M), b-vectors (M), the warp field's format, the GCA-morph spacing, `exp_k` (double), the command-line count and each line, the subject, path, format, stem and transform names, `AutoAlign` (M), `origRas2Vox` (M), the transform-ownership flag, 1/0 for a linear transform and for a colour table, then the geometry (printed, without `valid`); then the index tables (`noindex`, or for columns, rows and slices the entries at -500, -1, 0, 1, extent - 1, extent, extent + 499); the frames (`noframes`, or per frame: type, TE, TR, flip, TI, TD (floats), sequence type, echo spacing, echo-train length, read, phase-encode and slice directions (3 floats each), label, name, `dof`, `m_ras2vox` (M), threshold, units, DX DY DZ DR DP DS b-value TM (doubles), diffusion type, the four gradients' ramp and flat times (integers) and amplitudes (doubles)); the voxels |
| H, header fields | version intent dof mean(double) xsize ysize zsize imnr0 imnr1 ptype fov thick ps location xstart xend ystart yend zstart zend flip_angle(double) tr te ti name, the nine direction cosines and the centre (floats), ras_good_flag brightness, the subject, path, format, stem and transform names, a command-line count n and n lines, the registration, `AutoAlign` and `origRas2Vox` matrices (M; `null` keeps the volume's), a colour table (CT; `null` none), then fields the header copy does not copy: valid, the three shears, scale, yinvert, outside_val (double), FieldStrength |
| CT, a colour table | `null`, or `<n> <name> <version> <idbase> <schema>`, then per entry `0` (none) or `1 <name> <ri> <gi> <bi> <ai> <rf> <gf> <bf> <af> <tissue type> <count>`, then its tissue-type table (`null`, or a table of this form whose own tissue-type table is `null`). Printed the same; each entry's count only where the command says |
| X, a MINC transform | `none`, or `<kind> <inverse flag> <16 doubles> <16 doubles>` (the linear matrix and its inverse, row by row; kind 0 linear, 1 a concatenation of no transforms, others a kind FreeSurfer does not support); the volume then owns it and points at its effective matrices (the inverse first when the flag is set). Printed: the ownership flag, the effective linear matrix and its inverse (16 doubles each, row by row, or `null`), and when owned the kind and the inverse flag |
| B, a box's volume | V, a fill seed, a count n, then n settings `<c> <r> <s> <f> <value>` (a float, written by §5's write, mode 0) |
| LT, a linear transform | `x0 y0 z0 sigma` (floats), `m_L`, `m_dL`, `m_last_dL` (M each; `null` none), `type` `label`, the source and destination geometries (Gf each). Printed the same, the geometries printed as G |
| LTA, an array | `<n> <type> <fscale> <subject>`, then n transforms and n inverse transforms (LT): made by `LTAalloc(n, none)` with these fields written over it (its matrices freed where replaced) |

## Commands

The spec's sections in brackets.

| Command | Calls | Prints |
|---|---|---|
| `alloc V` | the allocation [§4] | the volume, then `MRIfree`'s value and 1 when the pointer is cleared |
| `allocvg G type nframes h`, `fromvg G type nframes h` | the volume made from a geometry (`MRI(vg, type, nframes, h)`; `MRIallocFromVolGeom`) [§4] | the volume (`fromvg`: then `MRIfree`'s value) |
| `free2 V` | `MRIfree` twice | both values |
| `sizeof i`, `intent i x` | `MRIsizeof`; `MRI::intentName(i)`, `MRI::intentCode(x)` [§4] | the size; the name and the code |
| `xfm V G base shear M` | `MRIxfmCRS2XYZ(base, shear)`, then `MRImatrixOfDirectionCosines`, `MRImatrixOfVoxelSizes`, `MRImatrixOfTranslations` (each into a copy of M, or a new matrix for `null`), `extract_i_to_r`, `extract_r_to_i` [§3] | the six matrices |
| `setv2r V G M` | `MRIsetVox2RASFromMatrix` [§3] | its value, the geometry |
| `p0 V G d d d` | `MRIp0ToCRAS(r0, a0, s0)` | its value, the geometry |
| `reinit V G` | `MRIreInitCache` | its value, the geometry |
| `vox V seed outside n ops` | the voxels filled, `outside_val` set, then n operations: `get c r s f mode` (`MRIgetVoxVal`), `set c r s f mode value` (`MRIsetVoxVal`), `dx`/`dy`/`dz c r s f` (`MRIgetVoxDx`, `Dy`, `Dz`) [§5] | each result (a float, or the write's value), then the voxels |
| `copyhdr V seed H D`, `copy V seed H D` | `MRIcopyHeader(src, dst)` [§6], `MRIcopy(src, dst)` [§13]; D `none`, or (`copyhdr`) `V seed H`, (`copy`) `V seed ptype` | `given`/`new`/`null`, the result's volume header (as printed above, without index tables and frames), its `mean` (double), its colour table (no counts); `copy`: then its voxels |
| `copyxfm X X'` | the header copy of a 2 × 2 × 2 header carrying X into a new volume (`new`) or into a header carrying X' [§14] | `given`/`new`, the result's transform |
| `pulse d f f f` | `MRIcopyPulseParameters` (flip angle, TR, TE, TI) | its value, the four copied |
| `ctab CT counts` | `CTABdeepCopy`, then `CTABfree` of the copy [§7] | the copy (the counts when `counts` is 1), the free's value, 1 when the pointer is cleared |
| `ctabnull` | `CTABfree` of a missing table, `CTABdeepCopy(NULL)` | the value, the copy |
| `vgcopy Gf` | the geometry's copy into a new one [§2] | the copy |
| `vgassign Gf Gf'` | the second geometry assigned the first [§2] | the second |
| `usevg G V G'`, `vgtomri G V G'` | `useVolGeomToMRI`, `MRIcopyVolGeomToMRI` (G into the volume) [§4, §6] | (`vgtomri`: its value) the volume's geometry |
| `vgfrommri V G G'` | `MRIcopyVolGeomFromMRI` (the volume into G') [§6] | its value, G' |
| `tkreg V G base shear` | `MRIxfmCRS2XYZtkreg`, `MRIxfmCRS2XYZfsl`, the geometry's scanner vox2ras (base, shear) and its inverse, tkregister vox2ras and inverse, scanner RAS to tkregister RAS and back [§9] | the eight matrices |
| `ras2s V G M` | `RASFromSurfaceRAS_` (into a copy of M), `VGtkreg2RAS`, `VGras2tkreg` (with a copy of M), `surfaceRASFromRAS_` | the four matrices |
| `tk2nat`, `nat2tk`, `v2v`, `tkv2v` `V G V G M` | `MRItkReg2Native`, `MRItkRegMtx`, `MRItkRegMtxFromVox2Vox`, `MRIvoxToVoxFromTkRegMtx` (the first volume, the second, the matrix) | the matrix |
| `fixtk V G seed M` | `srand48(seed)`, `MRIfixTkReg`, then `drand48()` | the matrix, the next number drawn (double) |
| `fsl2tk`, `tk2fsl` `T V G V G M` | `FSLOUTPUTTYPE` set to T (`unset`: removed), then `MRIfsl2TkReg`, `MRItkreg2FSL` | the matrix |
| `crs1 M` | `MtxCRS1toCRS0` | the matrix |
| `sphinx G` | `MRIhfs2Sphinx` | its value, the geometry |
| `v2w V G cache n points` | the cache dropped (1), recomputed (2) or left (0); then per point `x y z w` (doubles): w 1 `MRIvoxelToWorld`, w 0 `MRIworldToVoxelIndex` and `MRIworldToVoxel` [§10] | per point (w 0: the three indices) the three doubles, then the geometry |
| `codes x i x i` | `MRIinterpCode`, `MRIinterpString`, `MRIprecisionCode`, `MRIprecisionString` [§12] | the four results |
| `vgprint G nocheck fmt` | `vgprint(nocheck)`, then `geomprint()` or (`fmt` 1) `geomprint("geometry %s %d:\n", "of", 7)` [§11] | nothing (the text on the streams) |
| `vgeq G G' t debug M` | `vg_isEqual_Threshold` set to t, then `==`, `isNotEqualThresh(t)`, `checkgeom(t, debug)`, `checkvox2ras(t, debug, M)` | the four results |
| `diffv2r M M' t x debug` | `VOL_GEOM::diffvox2ras(M, M', t, x, debug)` | the result |
| `shearless V G` | `shearless_components` | the geometry |
| `tomat G M` | `toMatrix` (into M, or a new matrix) | the matrix |
| `shape i×4 i×4` | two `MRI::Shape`s compared with `==` and `!=` [§15] | both |
| `bbox B thresh` | `MRIboundingBox` (the box set to -7 everywhere first) [§17] | its value, the box's six integers |
| `ltaalloc n none`, `ltaalloc n B` | `LTAalloc(n, volume)`, then `LTAfree` [§20, §22] | the array, the free's value, 1 when the pointer is cleared |
| `ltacopy LTA D` | `LTAcopy(src, dst)`, D `none` or an LTA [§21] | `given`/`new`/`null`, the result, then D when it is not the result |
| `ltcopy LTA D` | `LTcopy` of the first array's first transform into D's first (`null`: none) | `given`/`other`/`null`, the result |
| `ltafreenull` | `LTAfree` of a missing array | its value |

## The comparison loop

```sh
# the cases, one line each (and the calls per group)
python -P compare.py list --scratch <scratch>/mri
# the probe's reference hashes per group (the spec's §23 quotes them)
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-mri-probe/compare.py refs \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_mri_probe --scratch <scratch>/mri
# a candidate against the probe, in each mode
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-mri-probe/compare.py compare \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_mri_probe \
  --candidate <the candidate> --scratch <scratch>/mri --mode reproduce   # then --mode fix
```

**The candidate** is the clean room's own program taking `batch <file>` with the lines above and
printing the same lines (its library behind the commands; each line's streams its own, a line that
ends the program `EXIT <status>`). It runs with `LARMORX_UPSTREAM_DEFECTS` set to the mode, in the
same working directory and environment as the probe. Its standard error may hold larmorx's defect
notices (`upstream defect <ID> ...` lines): the loop takes them out before comparing and keeps their
IDs. The cases are generated from `--seed` (default 7) into `<scratch>/batch`, each line given 10
seconds (`@10`); the working directory is `<scratch>/work`.

**The guard runs.** Each batch runs on the probe four times: plainly; plainly again, in another
process, with `MALLOC_PERTURB_` 0; and with `LD_PRELOAD` set to `guard_malloc.so`,
`MALLOC_GUARD_=over` and `MALLOC_PERTURB_` 90, then `MALLOC_GUARD_=under` and 165 (the fs.maths
probe's README says what each shows). A line whose four results are not the same read memory
FreeSurfer did not write or does not own: a **memory** line. The batch `alloc-limits` (extents or
frame counts that are not positive, types without a size) runs plainly only (twice): its wrapped
sizes are more than the guard allocator can map. A memory line, or one where FreeSurfer crashed,
counts in the reference hash by its place only (`<memory>`).

**What may differ.** A line is compared byte for byte (the result, both streams). A difference is
allowed on a line where (1) FreeSurfer did not finish (`TIMEOUT`, `CRASH`) and the candidate did;
(2) a **memory** line; (3) in the batches `matrices-sizes`, `geometry-sizes` and `tkreg-sizes`, a
line where the candidate noted FS-264 (its correction holds in both modes); (4) in `--mode fix`, a
line where the candidate noted a defect. Every other difference is unexplained: exit status 1, the
first three of each group printed. The loop prints, per group, the identical lines and those of
each allowed kind, and the IDs the candidate noted on the allowed lines.

## Results (2026-11-13, the reference machine: glibc 2.35, Linux 6.8, x86-64)

- `refs`, seed 7, the prebuilt probe (commit `2d660905`) with the fs.maths probe's guard allocator:
  the hashes quoted in the spec's §23.2 (9,245 calls; 408 memory or crashed lines, 619 that end the
  program, none where FreeSurfer never returns); identical through the oracle-output cache (cold),
  from it (warm) and without it, and in two more passes in separate processes, which classify every
  line alike. An earlier pass with the probe's own 2-second alarm saw one guard run's call run out
  of time under load: hence `@10`.
- `compare` with the probe as its own candidate, both modes: no unexplained difference; memory lines
  whose contents changed from process to process: `voxels` 18, `headers` 1, `copies` 2, `lta` 88.
- What the batches of other sizes showed (FS-264, the spec's §3): the writers into a given matrix
  and `RASFromSurfaceRAS_` crash with fewer than 4 rows, and change other elements with 4 x 1 and
  4 x 3; `MRIsetVox2RASFromMatrix` crashes on 1 x 4 and reads memory with 3 x 3 and 4 x 1; matrices
  larger than 4 x 4 are used by their top-left 4 x 4.
- About 2 to 3 minutes for a cold `refs` (most of it the guard runs), seconds warm.
