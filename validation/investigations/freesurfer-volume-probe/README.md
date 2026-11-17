# FreeSurfer 8.2.0's volume functions, as a black box (parts 1 to 3)

The probe behind the **(observed)** marks and the validation plan (§69) of
[`specs/freesurfer-volume.md`](../../../specs/freesurfer-volume.md) for its parts 1 to 3 (§1 to §17:
the volume-geometry text block, its copies, matrices, comparison and printer; the voxel
arithmetic; coordinates, orientation, the Talairach point, centre of mass, principal components,
the running average, the header comparison, determinant and dump). **Open to the clean-room
implementer of that spec**: the files here hold only calls of FreeSurfer's functions (and of the C
library's) by name, the reading and printing of their arguments, results and the structures'
fields, and a comparison loop; no FreeSurfer code, nothing written from its source. The implementer
runs the prebuilt probe (below) and the comparison loop; building the probe reads the unit oracle's
build tree, which is closed to FreeSurfer's clean-room implementers, so they never build it.

| File | What it is |
|---|---|
| `fs_volume_probe.cpp` | the probe: a batch of calls from a file, each in a child process of its own; its commands below |
| `build.sh` | builds it with the unit oracle's compiler (Ubuntu's GCC 8.4.0) and FreeSurfer's flags against `<workspace>/oracles/freesurfer-8.2.0-unit` (queue item 3.1-02): `build.sh <workspace> <output>` (the spec writer's or the orchestrator's step, not the implementer's) |
| `compare.py` | the comparison loop of the spec's §69: 10 groups of seeded cases and one of fixed cases (`vrules`; 8,180 calls), run on the probe (four times: plainly, plainly again in another process and in two guard runs, through the oracle-output cache) and on a candidate program that takes the same commands; `list`, `refs`, `compare` |
| `models.py` | a statement of the spec checked as a model of the probe's printed voxels: `fs291` (§9's byte walk of FS-291 under `reproduce`, a tensor volume's rows all at its first voxel: every `invc` and `sam` line of `vscale`, `vtwo` and `vrules`) |

The prebuilt probe: `<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe`; its guard
runs use the fs.maths probe's guard allocator, `guard_malloc.so` beside it
([`../freesurfer-maths-probe/`](../freesurfer-maths-probe/README.md)). Their builds (the larmorx
commit, the `build.sh`, the SHA-256s) are recorded in that folder's `BUILD_INFO.txt` for the
orchestrator's check; the implementer only runs them (every other access to the unit oracle's
folder, reading or hashing a file in it included, counts as opening FreeSurfer's build tree).

## Running it

```sh
fs_volume_probe batch <file>      # or - for standard input
```

Each line of the file is one call: `[@<seconds>] [+gdiag=<hex>] <command> <arguments>`, run as the
fs.mri probe runs its lines ([`../freesurfer-mri-probe/README.md`](../freesurfer-mri-probe/README.md),
"Running it": a child process each, an alarm of 2 seconds unless `@` sets another, the line
`<result> | <standard output> <standard error>`, `EXIT <status>`, `CRASH <signal>`, `TIMEOUT`). The
commands that write or read a file use `vg.out` and `vg.in` in the working directory.

## Encodings

Integers, floats, doubles, strings, **M** (a matrix), **G** (a geometry), **V** (a volume's shape),
the fill seed, **CT** (a colour table), **X** (a MINC transform) and the printed volume are the
fs.mri probe's (its README, "Encodings"). Added here:

| Kind | Form |
|---|---|
| W, a working volume | V; a fill: a seed (the fs.mri probe's fill seed: byte i is (seed + 101 i) mod 256; -1 leaves the voxels as allocated) or `t<seed>` (for a float volume: float i of the buffer, frames included, is the table's entry (7 i + seed) mod 34; for another type the seed's bytes); the geometry's fields but `valid`, the extent and the name: `xsize ysize zsize ras_good_flag x_r x_a x_s y_r y_a y_s z_r z_a z_s c_r c_a c_s s_r s_a s_s`; the header fields `imnr0 imnr1` (integers) `xstart xend ystart yend zstart zend thick ps tr te ti` (floats) `flip_angle` (a double) `dof` (an integer) `fov location` (floats) `ptype`; the cache: 0 the cached matrices as the allocation left them, 1 recomputed from the fields (`MRIreInitCache`), 2 none; the registration matrix (M); the colour table (CT) |
| the table | the floats `00000000 80000000 3f000000 bf000000 3f800000 bf800000 40200000 437f8000 437e8000 43800000 bfc00000 501502f9 d01502f9 7f7fffff ff7fffff 00000001 7f800000 ff800000 7fc00000 ffc00000 7fc12345 7f800001 40500000 42c80000 1e3ce508 477fff80 46ffff00 c7000080 4f000000 4f800000 41200000 c1200000 3dcccccd 40490fdb` (entry 0 first): zeros, halves, ones, 2.5, 255.5, 254.5, 256, -1.5, ±1e10, the largest floats, the smallest subnormal, infinities, NaNs (both signs, a payload, a signalling one), 3.25, 100, 1e-20, 65535.5, 32767.5, -32768.5, 2^31, 2^32, ±10, 0.1, π |
| D, a destination | `new` (none: the function makes one), `self` (the first source), `other` (the second source), or a W |
| R, a region | `null`, or `x y z dx dy dz` |
| a volume (printed) | `null`, or the fs.mri probe's header (its "volume (printed)" without the index tables and frames), the colour table (CT, each entry with its count), the voxels |
| a result (printed) | `self`, `other` or `given` when the returned volume is that one, then the volume; then, when a given destination is not the result, the destination's state |
| unset | an output the call may leave is 7.0 before the call (`40e00000`, `401c000000000000`) |

## Commands

The spec's sections in brackets.

| Command | Calls | Prints |
|---|---|---|
| `vgwrite G` | `writeVolGeom` to `vg.out` [§2] | the file's bytes (a string) |
| `vgread <string> G` | the string written to `vg.in`; `readVolGeom` of it opened for reading into the geometry G, `errno` set to 0 first [§3] | the geometry (printed G), `ftell`, end-of-file and error flags (1/0), `errno`, the next 64 bytes of the stream (a string) |
| `vgprint G` | `vg_print` [§4] | nothing (standard error) |
| `getvg G G`, `copyvg G G` | `getVolGeom`, `copyVolGeom` (the first into the second) | the second (G) |
| `vgi2r G`, `vgr2i G`, `tkrv2r G` | `vg_i_to_r`, `vg_r_to_i`, `TkrVox2RASfromVolGeom` | M |
| `vgeq G G d` | `vg_isNotEqualThresh` (the threshold), then `vg_isEqual` | two integers |
| `clone W D` | `MRIclone` [§7] | the result |
| `clonesp W i i` | `MRIcloneBySpace` (type, frames) | the volume |
| `clear W` | `MRIclear` | its value, the volume |
| `match W W` | `MRImatch`, `MRImatchDimensions` | two integers |
| `chsize W W\|null i i i` | `MRIcheckSize` (the width, height, depth) | its value |
| `cpframe W D i i` | `MRIcopyFrame` (source frame, destination frame) | the result |
| `cpframes W D i i i` | `MRIcopyFrames` (first, last, first destination frame) | the result |
| `vrange W`, `vrangef W i`, `vrangereg W R` | `MRIvalRange`, `MRIvalRangeFrame`, `MRIvalRangeRegion` [§8] | its value, the minimum, the maximum (floats) |
| `lrange W W i` | `MRIlabelValRange` (the labelled volume, the label) | its value, the minimum, the maximum |
| `linscale W D f f i` | `MRIlinearScale` (scale, offset, only non-zero) [§9] | the result |
| `smul W D f`, `smulf W D f i` | `MRIscalarMul`, `MRIscalarMulFrame` (the frame) | the result |
| `thresh`, `uthresh`, `invc` `W D f` | `MRIthreshold`, `MRIupperthresholdAllFrames`, `MRIinvertContrast` | the result |
| `binz W D f f f` | `MRIbinarize` (threshold, low, high) | the result |
| `abs`, `pos`, `neg` `W D` | `MRIabs`, `MRIpos`, `MRIneg` | the result |
| `mul2`, `sub2`, `add2`, `div2` `W W D` | `MRImultiply`, `MRIsubtract`, `MRIadd`, `MRIdivide` [§10] | the result |
| `sam W f W D` | `MRIscaleAndMultiply` | the result |
| `extract W D i×6` | `MRIextract` (x0 y0 z0 dx dy dz) [§11] | the result |
| `extinto W D i×9` | `MRIextractInto` (and x1 y1 z1) | the result |
| `extreg W D R` | `MRIextractRegion` | the result |
| `cras W W i×6` | `MRIcalcCRASforExtractedVolume` (x0 y0 z0 x1 y1 z1) | the three doubles, then the source's geometry (printed) |
| `sras W k` | k 0 `surfaceRASFromVoxel_`, 1 `voxelFromSurfaceRAS_`, 2 `GetSurfaceRASToVoxelMatrix`, 3 `surfaceRASFromRAS_` of the volume [§13] | M, then the volume's geometry (printed) |
| `spt W k d d d` | k 0 `MRIRASToSurfaceRAS`, 1 `MRIvoxelToSurfaceRAS`, 2 `MRIsurfaceRASToVoxel` | its value, the three doubles, the geometry (printed) |
| `ostr G` | `MRIdircosToOrientationString` into 8 bytes of `Z`, `MRIsliceDirectionName`, `getSliceDirection` [§14] | its value, the first 5 bytes (a string), the name, the direction |
| `vtal W X d d d` | `MRIvoxelToTalairach` of the volume with the MINC transform X | its value, the three doubles, the geometry (printed) |
| `inidx W d d d` | `MRIindexNotInVolume` [§15] | its value |
| `snear W d d d d` | `MRIsampleVolumeType(..., SAMPLE_NEAREST)` with the volume's `outside_val` set to the last double | its value, the sample |
| `com W i` | `MRIcenterOfMass` (the threshold, a byte) | its value, the three means |
| `pca W i` | `MRIprincipleComponents` into a new 3 x 3 matrix | its value, the vectors (M), the three values (floats), the means |
| `avg W i D` | `MRIaverage` (dof) [§15b] | the result |
| `cmph W\|null W\|null\|self` | `MRIcompareHeaders` [§16] | its value |
| `vdet W` | `MRIvolumeDeterminant` | a float |
| `dump W` | `MRIdump` to standard output | its value |
| `dsign d` | `MRIsetSampleVolumeDerivativeSign` | the double returned |

## The models

```sh
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 4 python -P \
  validation/investigations/freesurfer-volume-probe/models.py fs291 \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe --scratch <scratch>/models
```

Prints how many lines match the model (by the types met: `uchar` only, a tensor, other types), how
many it leaves out (FreeSurfer crashed or read memory it does not own) and how many differ; exit
status 1 when one differs.

## The comparison loop

```sh
python -P compare.py list --scratch <scratch>/volume
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-volume-probe/compare.py refs \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe --scratch <scratch>/volume
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-volume-probe/compare.py compare \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe \
  --candidate <the candidate> --scratch <scratch>/volume --mode reproduce   # then --mode fix
```

The candidate, the guard runs, the memory lines and what may differ are as in the fs.mri probe's
loop (its README, "The comparison loop"), without its `FS-264` batches: a difference is allowed on a
line where FreeSurfer did not finish (`TIMEOUT`, `CRASH`) and the candidate did, on a **memory**
line, and in `--mode fix` on a line where the candidate noted a defect. Every call gets 10 seconds
(`@10`) but `pca`'s (2: FreeSurfer's eigensystem never returns on some covariances). The groups:
`vgblock` (§2-3: 60 writes; 500 reads of generated blocks: well-formed, with lines missing,
repeated, reordered or blank, lines of white space or other keys, first words of 63 to 200 bytes,
odd second words, numbers in glibc's other forms, lines longer than 1,039 bytes, NUL bytes, CRLF
endings, no final newline, data after the block, an empty file), `vgcopy` (§4), `vmake` (§7),
`vrange` (§8), `vscale` (§9), `vtwo` (§10), `vextract` (§11), `vcoords` (§13-14), `vstats` (§15,
§15b), `vheader` (§16), and the fixed cases of `vrules` (§6-15: FS-291 with tensor volumes, the
clear of a bitmap and of volumes without voxels, `MRIlinearScale` with `only_nonzero`, sums and
differences of types without direct access, the frame copies' messages, `MRIvalRangeRegion` without
a region, `MRIsampleVolumeType`'s order of checks): volumes of up to 4 x 4 x 4 voxels (5 for
`vextract`, 6 for `vcoords`) and 0 to 3 frames of every type (the types without a voxel size
header-only), headers only too, destinations new, of the same shape, of another shape or a source,
cached matrices as allocated, recomputed or dropped.

## Results (the reference machine: glibc 2.35, Linux 6.8, x86-64)

- Relay 6 (2026-11-16): `refs`, seed 7, the prebuilt probe from commit `77392db5`: the hashes of
  the spec's §69.2, the same through the oracle-output cache cold, warm and without it; the probe
  as its own candidate: no unexplained difference in either mode.
- Relay 7 (2026-11-17): the fixed group `vrules` (84 calls), the same hash from the cache and
  without it, the other groups' hashes unchanged, the probe as its own candidate identical in both
  modes (log `oracles/logs/fs-volume-probe-r7-refs-2026-11-17.log`); `models.py fs291`: 127 lines as
  the model, none different, 50 memory or crash.
