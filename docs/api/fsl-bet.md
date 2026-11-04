# BET (`bet`, `bet2`, `betsurf`)

`lx.fsl.bet` and `lx.fsl.betsurf` (command line `larmorx fsl bet`, `larmorx fsl bet2`,
`larmorx fsl betsurf`): FSL 6.0.7.23's brain extraction and its skull and scalp surfaces (FSL
6.0.7.17's until 2026-10-29: what changed, `bet2 --binaryMask` among it, in
[findings/fsl-bet.md](../findings/fsl-bet.md), "FSL 6.0.7.23").

**Two implementations.** FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx keeps its FSL tools in two implementations ([licensing](../licensing.md)): the bit-exact
**replica**, `larmorx-nc fsl bet|bet2|betsurf` (package `larmorx-nc`, `crates-nc/`, under the FSL
Licence's terms; translated from FSL's source:
[PROVENANCE](../../crates-nc/larmorx-nc-bet/PROVENANCE.md)), and the clean-room **original** in
the main package (Apache-2.0, `crates/larmorx-fsl`), written from
[`specs/bet.md`](../../specs/bet.md), the BET paper and black-box runs of FSL
([PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md); since 2026-10-30 `bet2` and the `bet`
script, since 2026-10-31 `betsurf`). `implementation="auto"` runs the replica, as a separate
program, when `larmorx-nc` is installed (`pip install larmorx-nc`), else the original;
`"replica"` and `"original"` choose.
Each image's `implementation` names the one that ran. The command line chooses alike
(`LARMORX_IMPLEMENTATION=auto|replica|original`).

**Status: `validated`** (the replica), against FSL 6.0.7.23 itself
([record](../validation/fsl-bet-replica.md), standard tier, re-made 2026-11-04 with `-S`, `-B`,
`-A`, `-A2`, standard_space_roi and fslroi: 263 cases, 201 compared, every one bit-identical, 600
of 600 files byte for byte, FSL's own step from 6.0.7.17 in every case;
[full tier](../validation/fsl-bet-replica-full.md), against FSL 6.0.7.17: 173 cases, 129
compared, 364 of 364 files byte for byte): every compared case gives FSL's exit status, messages and files byte
for byte (images: header and data; meshes as text), fMRIPrep's 29 BET calls on its twelve oracle
runs with BOLD runs included (the data panel's), whose outputs also have the values of fMRIPrep's
own (and of fMRIPrep's build of FSL in its image). 47 cases are rejected by both alike; in 13, FSL
crashes on what it leaves undefined and `reproduce` fills it (below); 1 waits for fslpy's `imcp`
conversion (standard_space_roi, below). The records include random testing's permanent cases (17 in the standard tier) (`difftest/...`; BET's
random testing: [docs/validation/difftest.md](../validation/difftest.md)), and the replica needs no
more memory than FSL in any measured call (`tests/parity/test_memory_peak.py`).

## Quick start

```python
import larmorx as lx

# As fMRIPrep calls BET on its BOLD reference.
r = lx.fsl.bet("boldref.nii.gz", frac=0.2, mask=True)
r.brain, r.mask
r.mask.implementation   # Implementation(kind='replica', package='larmorx-nc', ...)

# The skull and scalp: bet2's mesh, then betsurf with the T1's registration to MNI152
b = lx.fsl.bet("T1w.nii.gz", mesh=True)
s = lx.fsl.betsurf("T1w.nii.gz", b.mesh, "T1w_to_MNI152.mat", skull_mask=True)
s.inskull_mask, s.outskull_mask, s.outskin_mask, s.skull_mask
```

```bash
larmorx fsl bet boldref.nii.gz boldref_brain.nii.gz -f 0.20 -m
larmorx fsl bet2 T1w.nii.gz T1w_brain -e
larmorx fsl betsurf --t1only -o -m -s T1w.nii.gz T1w_brain_mesh.vtk T1w_to_MNI152.mat T1w
```

## `lx.fsl.bet(image, *, frac=0.5, gradient=0.0, radius=None, centre=None, smoothness=1.0, threshold=False, head_ct=False, variation=None, t2=None, brain=True, mask=False, outline=False, skull=False, mesh=False, tanh="glibc", fsl_data=None, n_threads=1, implementation="auto", upstream_defects="fix")`

Returns a `BetResult`: `brain` (float32: the input times the mask; 4D with `"fmri"`), `mask`
(int16, 1 inside), `outline` (float32), `skull` (int16) and `mesh` (a `Surface`: `vertices`
`(n, 3)` in FSL's millimetre coordinates, `triangles` `(m, 3)`), each `None` unless asked for.
`image` is a path (any image FSL reads: NIfTI-1 or -2, single file or pair, gzipped or not,
ANALYZE) or an in-memory image. Without `variation` it runs `bet2`; with `"robust"` (`-R`),
`"small_fov"` (`-Z`) or `"fmri"` (`-F`) the `bet` script; `"eyes"` (`-S`), `"bias"` (`-B`),
`"surfaces"` (`-A`) and `"surfaces_t2"` (`-A2`, with `t2`) run the script with FLIRT,
standard_space_roi, FAST, fslmaths, fslstats and betsurf, and have only their replica yet
(since 2026-11-04; `implementation="original"`, or `"auto"` without `larmorx-nc`:
`NotImplementedError`); `"surfaces"` and `"surfaces_t2"` also return betsurf's surfaces
(`BetResult.surfaces`, a `BetsurfResult` with the masks, the skull's mask and the outlines). They
read FSL's standard images from `fsl_data` (a directory laid out as `$FSLDIR/data`; the variable
`LARMORX_FSL_DATA` otherwise), else `$FSLDIR/data`, else the copy `larmorx-nc` carries. The
arguments are checked first (`ValueError`); FSL's
settings are the call's: the outputs are written as NIfTI-1 and read back, `FSLOUTPUTTYPE`,
`FSL_LOAD_NIFTI_EXTENSIONS` and every other `FSL*` variable of the process are never passed, and no
FSL program is looked for on the `PATH` (CLAUDE.md rule 6).

### Option mapping (`bet2`, and `bet` without a variation)

| FSL option | Python | Notes |
|---|---|---|
| `<input> <output>` | `image`; the result | outputs `<output>`, `_mask`, `_overlay`, `_skull`, `_mesh.vtk` |
| `-f <f>` | `frac` | 0 to 1; smaller gives a larger brain |
| `-g <g>` | `gradient` | -1 to 1; positive gives a larger brain at the bottom |
| `-r <r>`, `--radius` | `radius` | mm; the initial sphere is half of it |
| `-c <x y z>` | `centre` | voxels of the file as stored; `0 0 0` is "not given" (the centre of gravity), as in FSL (observed) |
| `-w <r>`, `--smooth` | `smoothness` | bet2 only (`bet` does not pass it) |
| `-t`, `--threshold` | `threshold=True` | also thresholds the brain and mask |
| `--ct` | `head_ct=True` | bet2 only: the input in Hounsfield units |
| `-m`, `--mask` | `mask=True` | |
| `-o`, `--outline` | `outline=True` | |
| `-s`, `--skull` | `skull=True` | |
| `-n`, `--nooutput` | `brain=False` | |
| `-e`, `--mesh` | `mesh=True` | VTK; also switches the self-intersection test to the exact one |
| `-v`, `--verbose` | (command line only) | |
| `--parameter_d1=`, `--parameter_d2=` | (command line only) | hidden search depths (7 and 3) |
| `--tanh=correctly-rounded` (larmorx's own; also `--tanh correctly-rounded`, and passed on by `bet`) | `tanh="correctly-rounded"` | the surface's smoothness weight with the correctly rounded `tanh` (CORE-MATH's, `larmorx_core::math::tanh`) instead of glibc 2.35's bits, which FSL calls (`glibc`, the default). **GLIBC-06, the user's decision of 2026-10-31**: glibc's bits by default, the correctly rounded one on request (no notice: it is asked for). Where a surface self-intersects and is found again, the last bit can change the result (4 of the standard tier's 13 real images: findings). The clean-room original only (`implementation="replica"` refuses it; `"auto"` runs the original) |
| `FSLOUTPUTTYPE` | (always NIfTI-1 in Python) | on the command line read from the environment, as FSL does |

### `bet`'s variations

| `bet` option | Python `variation` | What it does |
|---|---|---|
| (none) | `None` | bet2 |
| `-R` | `"robust"` | bet2 again from the brain's centre until it moves less than a voxel (at most 11 runs); `-n` ignored |
| `-Z` | `"small_fov"` | pads the image to 100 mm in z with copies of its end slices, crops back (FSL-45) |
| `-F` | `"fmri"` | the first volume's mask (`-f 0.3`, then the user's `-f`), dilated, applied to every volume |
| `-S` | `"eyes"` | SIENA's eye and optic nerve cleanup: `bet2 -s`; `standard_space_roi -b -d` and `bet2 -n -m -f 0.4` on its result; MNI152's eye mask taken into the input (FLIRT `-applyxfm` through standard_space_roi's inverse matrix), dilated (`-dilF`, 7 more in 2D) and removed from the brain mask, the premask's brain added; what is brighter than 3.5 standard deviations above the brain's mean (`fslstats -M`, `-S`, GNU `dc`: FSL-216) removed with a little morphology; the mask (`_mask`) and the skull without the eyes (`_skull`) always written; no outline or mesh (`-o`, `-e` only in its temporary run); with `-n` it fails (`brain=False` refused) |
| `-B` | `"bias"` | SIENA's bias field and neck cleanup: `standard_space_roi -b -d`, FAST's bias field (`fast -b`, its messages discarded), the input divided by it; with `-s` the skull from `standard_space_roi -maskFOV -roiNONE -d` and `bet2`, moved by fslpy's `immv` (and `-s` removed from the options through `echo`: FSL-63); `standard_space_roi -b -d` again and `bet -m -R -d` on its result (the brain and mask always written: `-R` removes `-n`) |
| `-A` | `"surfaces"` | FLIRT to `MNI152_T1_2mm` (the matrix), `bet2 -e`, `betsurf --t1only -o -m -s`: the brain, its mesh, the three surfaces, their masks and outlines, the skull's mask |
| `-A2 <T2>` | `"surfaces_t2"`, `t2` | the T2 registered to the input by FLIRT (`-interp sinc`), then as `-A` with `betsurf -o -m -s` on the registered T2 |
| (the last of `-R`, `-S`, `-Z`, `-F`, `-A`, `-A2`, `-B` given) | | the variation run |
| `-d` | (command line only) | keeps the temporary files |

## `lx.fsl.betsurf(t1, brain_surface, t1_to_standard, *, t2=None, masks=True, skull_mask=False, outlines=False, precision=0, n_threads=1, implementation="auto", upstream_defects="fix")`

Returns a `BetsurfResult`: the `inskull`, `outskull` and `outskin` surfaces, their masks
(`masks=True`, `-m`), the skull's mask (`skull_mask=True`, `-s`) and the surfaces drawn as images
(`outlines=True`, `-o`), all int16 on the T1's grid. `brain_surface` is `bet(..., mesh=True).mesh`
or a VTK file; `t1_to_standard` FLIRT's 4 x 4 matrix to MNI152 (a path or an array; it only decides
which profiles cross the eyes); without `t2` the surfaces come from the T1 alone (`--t1only`).
`precision` is `-p`. `lx.fsl.read_vtk(path)` reads a VTK mesh as FSL writes it.

### Option mapping (`betsurf`)

| FSL option | Python | Notes |
|---|---|---|
| `<t1> [<t2>] <bet_mesh.vtk> <t1_to_standard.mat> <output>` | `t1`, `t2`, `brain_surface`, `t1_to_standard`; the result | outputs `<output>_inskull_mesh.vtk`, `_outskull_mesh.vtk`, `_outskin_mesh.vtk` (always), their images |
| `-1`, `--t1only` | `t2=None` | the surfaces from the T1 alone |
| `-m`, `--mask` | `masks=True` (default) | `<output>_inskull_mask`, `_outskull_mask`, `_outskin_mask` |
| `-s`, `--skullmask` | `skull_mask=True` | `<output>_skull_mask` (the masks are made without `-m` too) |
| `-o`, `--outline` | `outlines=True` | `<output>_inskull_mesh`, `_outskull_mesh`, `_outskin_mesh` images: each surface drawn as 1 |
| `-p <int>`, `--increased_precision=<int>` | `precision` | each surface retessellated that many times, 100 more steps each (the meshes written are the retessellated ones); FSL's int: `strtol` of base 0 (`0x10` reads, `08` does not); larmorx refuses more than 10 (the meshes would not fit any memory) |
| `-v`, `--verbose` | (command line only) | `extracting profiles`, the profiles cast and the percentage set aside (the dropped ones and those of which only the scalp counts), then each phase |
| `-h`, `--help` | (command line only) | the usage, exit 0 |
| the mesh name `manual_input` | (command line only) | the name read from standard input (larmorx refuses at its end, where FSL waits forever); the name `c` cancels (an empty mesh: refused, FSL-52) |
| `FSLOUTPUTTYPE` | (always NIfTI-1 in Python) | read when the first image is written, as FSL does |

## Upstream defects (D13)

Corrected by default, reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); each correction that changes a result is named as an
`lx.UpstreamDefectWarning` (on the command line `larmorx-nc: upstream defect <ID> ...`). Decided by
the project's rules A to D ([register](../findings/upstream-defects.md)):

- **FSL-40** (A): bet2 removes the first `.hdr` of a name wherever it stands, and a name with
  `.img` but no `.hdr` aborts FSL; the default removes a trailing extension only.
- **FSL-42** (B): a NaN `-f` or `-g`, a negative, NaN or infinite `-r`, `-w` of 0 or less, a
  non-finite `-c` or search depth: FSL computes with them; the default refuses (exit 1, naming the
  value).
- **FSL-43** (A): the mask's fill starts at the surface's centroid without a bounds check; on a
  single slice that voxel lies below the image. From a start outside, FSL reaches each point by
  the image's linear index: a point outside is another voxel of the volume, or memory beyond it,
  which FSL reads and writes (and may crash on). The default starts at the nearest voxel inside;
  `reproduce` does FSL's fill point by point, the volume exactly and the memory beyond it read as
  not part of the mask, and warns with the number of those reads (since 2026-10-29). Where the
  centroid is not a number, FSL writes far outside the volume and crashes: the default refuses;
  `reproduce` skips the fill (the mask holds the drawn surface only) and warns.
- **FSL-44** (A): `bet` splits names with blanks into words and stops its options at an empty
  argument; the default keeps each argument whole.
- **FSL-63** (A): `bet -R` passes bet2's options through bash's `echo`, which takes a leading
  `-e` (`-n`, `-E`) as its own option: `bet <in> <out> -R -e` writes no mesh in FSL. The default
  passes the options as given (the mesh written); `reproduce` drops them as FSL does.
- **FSL-45** (A): `bet -Z` crops its outputs with `imglob` called without `${FSLDIR}/bin`: with
  FSL off the `PATH` (always so in Python) FSL leaves them padded; the default crops exactly its
  outputs.
- **FSL-46** (A): `bet`'s cleanup removes every `<output>_tmp*` file; the default only its own.
- **FSL-47** (A): `betsurf --t1only` fills the scalp mask from the outer skull surface's centre in
  its padded grid; the default from the scalp's (named only when the mask changes).
- **FSL-48** (B): a T2 on another grid than the T1 is sampled at the T1's voxel coordinates; the
  default refuses.
- **FSL-50** (A): one skull profile with an empty search range ends betsurf (status 255, no
  message, no output); the default drops that profile.
- **FSL-51**: a profile search can read past the profile's samples, into memory FSL never set;
  the default drops such profiles, `reproduce` reads "nothing there" and warns.
- **FSL-53** (B): a matrix file with fewer than the 12 numbers betsurf uses; the default refuses.
- **FSL-54** (A): `betsurf --t1only` exits 0 when it fails; the default exits with the failure's
  status.
- **FSL-55** (B): a NaN in the T1 inside the brain surface makes the 98th percentile undefined; the
  default refuses.
- **FSL-56** (A): an image saved with two or more NIfTI extensions of different sizes (only with
  `FSL_LOAD_NIFTI_EXTENSIONS`) gets a wrong data offset; the default writes the right one.
- **FSL-216** (A; B for a number that is not one): `bet -S` computes its eye threshold with GNU
  `dc` from what `fslstats` prints, which `dc` misreads for a number with an exponent, a minus
  sign, `nan` or `inf`; the default reads the numbers as numbers and refuses a mean or deviation
  that is not a number; `reproduce` gives what `dc` prints, its messages too.
- **FSL-217** (A): `bet -F` and `-Z` cut the input with fslroi (and `-Z` joins the slices with
  fslmerge), which write an int8 image as uint8 with the same bytes: bet2 then runs on negative
  values turned into 128 to 255. The default keeps int8 and its values, named where the result
  differs (a negative value, or `-d` keeping the int8 temporary files); `reproduce` gives FSL's
  uint8.

Where FSL crashes on what it leaves undefined, the default refuses with the reason and
`reproduce` keeps everything FSL defines, fills the rest deterministically and warns (D13's
failure cases): **FSL-41** (no voxel strictly between the 2nd and 98th percentiles inside the
initial sphere, a constant or binary image: FSL reads the median from an empty list; `reproduce`
takes it as 0) and **FSL-52** (a mesh file FSL reads without checks: `reproduce` takes 0 for the
values FSL never sets and leaves out the faces that name missing vertices) and **FSL-58** (no voxel of
the T1 inside the brain surface: betsurf reads its 98th percentile from an empty list; `reproduce`
takes it as 0). Inside `bet`, where FSL's
bet2 crashes, the script's ERR trap reports the failure and exits 1: that is FSL's defined outcome,
and `reproduce` gives it (bet2 alone fills instead). Kept as FSL's and
documented (rule C):
**FSL-49** (betsurf keeps the T1's scalp estimate when the T2's lies further out) and **FSL-57**
(betsurf's smoothness weight set to 0 after it was used: no effect).

### glibc's maths functions (GLIBC-07)

FSL's bet2 takes its initial radius and the fractional threshold's power from glibc 2.35's `pow`,
and betsurf its blurring's weights from glibc's `exp` (`specs/bet.md` §8), which are not correctly
rounded. The clean-room original computes them correctly rounded by default and takes glibc's with
`upstream_defects="reproduce"` (`larmorx_core::math`'s `*_glibc` functions, proved bit for bit
against glibc 2.35; the user's decision of 2026-11-04: glibc's bits for `reproduce` only). The
surface is an iterative fit: one power's last bit can move it, and bet2 cannot tell without fitting
twice, so neither mode names GLIBC-07: the default's correctly rounded functions can move a
surface's last bits. bet2's `tanh` is a separate choice (GLIBC-06, `tanh=`, above).

## Deliberate differences

- Where FSL aborts on an uncaught exception (a missing input, FSL-40's `.img` name), the
  replica prints FSL's messages before the abort and one line of its own, and exits 1 (FSL: 134).
  Where it crashes on what it leaves undefined (FSL-41, FSL-52, FSL-43 at a centre that is not a
  number), the default
  refuses (exit 1) and `reproduce` finishes with the undefined part filled (exit 0, the defect
  named); FSL ends by a signal. FSL's buffered standard output before such a crash may be lost
  (with `-v`); the replica's goes on.
- The `bet` script's messages name the script by its path (`$0`: `<FSLDIR>/bin/bet failed during
  command:...`); the replica names it `bet`.
- The replica needs no FSL installation and runs none of FSL's programs: what `bet -S`, `-B`,
  `-A` and `-A2` run (FLIRT, standard_space_roi, FAST, fslmaths, fslstats, betsurf, fslpy's
  `immv`, GNU `dc`, the script itself for `-B`'s robust run) are the replicas', in its process.
  The standard images they read (`MNI152_T1_2mm`, `MNI152_T1_2mm_brain_mask_dil`,
  `MNI152_T1_2mm_eye_mask`) are looked for in `LARMORX_FSL_DATA` (a directory laid out as
  `$FSLDIR/data`; when set, only there), else `$FSLDIR/data`, else `larmorx-nc`'s own copy
  (`<prefix>/share/larmorx-nc/fsl-data`, which the wheel installs: FSL 6.0.7.23's three files,
  byte for byte, with FSL's Licence, MNI's notice and their manifest, so no FSL installation is
  needed); when one is nowhere, the script refuses (exit 1) before it runs anything, naming it
  and every place looked.
- A program the script runs that aborts (fslmaths on an image that is not there, as `bet -S -n`
  meets) prints FSL's text and bash's report (`bet: line 399: <pid> Aborted ...`), then the
  `ERR` trap's line, exit 1, as FSL's script.

## Known last-bit differences

None: the replica carries the C library functions FSL's programs call as this machine's glibc
computes them (`pow`, `exp`, `tanh`, and the x87 `long double` arithmetic of FSL's NIfTI layer;
`crates-nc/larmorx-nc-core/tools/libm-verify`), on every platform; NaN and infinite voxels give
SSE's NaN bits on every platform (`larmorx-nc-core`'s `sse`; `validation/probes/fsl-bet-nan`).

## Speed

[benchmarks/fsl-bet.md](../benchmarks/fsl-bet.md) (2026-10-31, the median of 3 runs, FSL
6.0.7.23, the replica and the clean-room original on the same calls): fMRIPrep's call on
ds000005's BOLD reference took 0.67 s in FSL, 0.39 s in the replica and 0.24 s in the original
(0.06 s on 24 threads); `bet2 -m` on four T1w images 1.25 to 3.06 s, 0.72 to 1.75 s and 0.49 to
1.21 s; betsurf on four real heads 6.5 to 18.4 s, 3.9 to 9.9 s (2.6 to 5.8 s on 24 threads: the
replica smooths in parallel, with the same result) and 2.1 to 5.3 s (0.6 to 1.4 s on 24). Earlier
informal timings of the replica:
[validation/investigations/fsl-bet](../../validation/investigations/fsl-bet/README.md).

## Rust

The replica's Rust API is in the crate `larmorx-nc-bet` (FSL Licence): `bet`, `bet2`, `betsurf`
(the `larmorx-nc` program's entry points: the original arguments, the settings, D13's mode, the
thread count), `bet2::main`, `bet::main`, `betsurf::main`; `bet2::compute` runs the extraction on
a `larmorx_nc_core::volume::Volume<f32>`. FSL's libraries are the shared core's,
`larmorx-nc-core` (its [PROVENANCE](../../crates-nc/larmorx-nc-core/PROVENANCE.md) lists the
API). The Apache-2.0 crates cannot use them.


## The clean-room original (`crates/larmorx-fsl`)

Written from [`specs/bet.md`](../../specs/bet.md), S. M. Smith's paper (Human Brain Mapping
17(3), 2002) and black-box runs of FSL 6.0.7.23's programs, without FSL's source or the replica
([PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md); what the runs showed beyond the spec:
[findings/fsl-bet-original.md](../findings/fsl-bet-original.md)). Since 2026-10-30: `bet2`
complete (every option, message, exit status and failure case of the spec, with the D13
switches of FSL-40, -42, -43, -56, -60, -61 and -62) and the `bet` script's default, `-R`
(FSL-63), `-F` and `-Z` (FSL-45), FSL-44 and FSL-46; `-S`, `-B`, `-A`, `-A2` refused as pending
(the replica runs them since 2026-11-04; their original, from `specs/bet.md` §5.3-5.4, is the
clean-room catch-up part's). Since 2026-10-31: **`betsurf`** complete (spec §6: every option of §6.1,
`-p`'s retessellation, the VTK, OFF and FreeSurfer meshes, `manual_input`, every message; the
D13 switches of FSL-43, -47, -48, -50 to -55 and -58, FSL-49 and FSL-57 kept as FSL's), so
`implementation="auto"` without `larmorx-nc` runs the original for every BET program.

**Status: `experimental`** ([record](../validation/fsl-bet.md), standard tier against FSL
6.0.7.23, re-made 2026-10-31 in part 2b: **127 of the 128 compared cases within the thresholds**, the
other missing one threshold (below); of its 168 cases 30 more are rejected by both alike, 6 are where
`reproduce` fills what FSL leaves undefined, 4 pending): exit statuses and messages FSL's byte
for byte; 217 of 217
brains, masks and outlines of `bet2` and `bet` bit-identical to FSL's (part 2a's record
counted 12 reported Dice values with phantoms' known brains among them, as "within the
thresholds"), 13 of 13 skull images bit-identical, the meshes FSL's to the file's 6 decimals, the
surfaces that rerun after a self-intersecting first pass (a 75-year-old's T1w, 7 T, a tumour)
bit for bit, fMRIPrep's 29 BET calls on its oracle runs all bit-identical (and with fMRIPrep's
own outputs' values); headers FSL's byte for byte but for `descrip` (`larmorx fsl bet2`), the
qform's quaternion included. The 142 cases of part 2a's record are unchanged; the 26 new ones
are betsurf's (its options, messages and failure cases, FSL-48, -52, -53, -54 among them, and
five real heads with bet2's meshes and FLIRT's matrices made by FSL).

**betsurf, what agrees and what does not.** The profiles' decisions (§6.4) are FSL's: on 330 of
330 single profiles probed one at a time (a tiny triangle that casts exactly one profile; FSL's
`-v` says whether it set it aside), 127 of 128 triangles of a T1 and T2 and 64 of 64 of a T1
alone; the share of profiles set aside within 0.06 % of FSL's on the record's real images. Where
the votes are dense the surfaces are FSL's (the phantom's inner and outer skull to the file's 6
decimals, also with `-p 1`); where few profiles vote (a skull base where nearly every profile is
set aside) the inner skull settles elsewhere by up to 11 mm on a few vertices, and the scalp
differs a little everywhere (95 % of the vertices within 0.2 to 3 mm on real heads). The record
holds betsurf to its masks (Dice >= 0.99 and the boundary distance's 95th percentile <= 1 voxel,
specs/bet.md §11): 14 of its 67 masks and outlines
are FSL's bit for bit and 51 more within these thresholds; on the five real heads the masks'
Dice coefficients are 0.9907 to 0.9997 (the inner skull 0.9989 to 0.9997, the scalp 0.9938 to
0.9987), and the scalp of ds002603 with its T2 misses the boundary threshold (its 95th
percentile 2 voxels): the record's one failure. The questions for the spec writer are in the findings.

**`tanh`** (GLIBC-06, the user's decision of 2026-10-31): the smoothness weight's `tanh` is glibc
2.35's by default (`larmorx_core::math::tanh_glibc`, fdlibm's, proved equal to the system's on
12.9 billion inputs), so the surfaces are FSL's; `--tanh correctly-rounded` (`tanh=
"correctly-rounded"`) takes the correctly rounded `larmorx_core::math::tanh` (CORE-MATH's),
which gives another, equally valid surface on 4 of the standard tier's 13 real images (the
findings give the measurement). The surface follows `specs/bet.md` §8.1 (the deformation's
arithmetic, operation by operation), §8.2 (the quaternion's precision path) and §4.7 (the skull
search).

**Random testing** (a 30-minute trial, 2026-10-31, `oracles/difftest/2026-10-31-cleanroom-betsurf`,
1,846 cases, both implementations; the first, 2026-10-30, in the findings): the replica agreed in
every case it finished (3 timeouts). The original agreed in 957 of 961 `bet2` cases and 513 of
532 `bet` cases: the 21 others are, all but one, neurologically stored images (`bet -R` and
`-c`; case 1457 stored the other way along x is FSL's exactly), the class of cases 622 and 677,
open with the spec writer. betsurf: 221 of 353 agree, 6 are FSL-51's (undefined); 126 differ,
on small synthetic images (11 to 32 voxels per side) where the profiles' decisions differ (case
36: FSL sets aside 96.8 % of its profiles, the original 87.8 %), and in 2 FSL-50's `reproduce`
stops where FSL runs on: both for the spec writer (findings, "Random testing, part 2b").

**Built for speed:** bet2's vertices of each iteration are updated in parallel (each from the
previous iteration's positions only), the self-intersection measure finds close pairs in a grid
of cells instead of over all pairs, the fill is a stack fill over the grid, and no image is held
twice; betsurf casts its profiles in parallel (each vertex's into its own list, counted in
vertex order), smooths each axis in parallel with exact weights in two buffers, and deforms each of
its three surfaces with every vertex's step computed in parallel from the previous positions. Every parallel step runs in the call's own pool of `n_threads` (the
command line: `OMP_NUM_THREADS`, else every core) with the same result at any thread count.
Memory: betsurf's peak is below FSL's on the memory test's call (144 MiB against 175: the T1
clamped in place, the images freed after the profiles). Timings:
[benchmarks/fsl-bet.md](../benchmarks/fsl-bet.md) (the original 2.5 to 2.9 times FSL's speed on one thread for bet2 and bet, 3.0 to 3.6 for betsurf; 6.5 to 11.6 and 10.7 to 13.5 times on 24 threads).

**Deliberate differences of the original:** its images' `descrip` is `larmorx fsl bet2` (betsurf's
too); where FSL aborts or crashes it prints FSL's messages before that point and one line
of its own ending in `(larmorx)`, and exits 1; `bet`'s messages name the script `bet`; betsurf's
`manual_input` refuses at the end of its input, where FSL waits forever; `-p` above 10 is
refused (the meshes would not fit any memory).

**Rust:** `larmorx_fsl::bet::find_surface` (the surface on a `larmorx_fsl::bet::Vol`: the initial
parameters, the passes and their self-intersection), `brain_mask`, `outline`, `masked`,
`skull::skull`, `cormack`; betsurf's steps in `larmorx_fsl::betsurf`: `read_matrix`,
`robust_maximum` and `clamp` (§6.3), `WorkGrid` (the padded working grid), `votes` (the profiles
and their counting images, §6.4), `smooth`, `deform`, `sphere`, `retessellate` and
`surface_mask` (§6.5), the profile searches in `larmorx_fsl::betsurf::profile`; the command
lines `larmorx_fsl::cli::run` / `run_in` (an environment table and a directory given, as
`lx.fsl.bet` and `lx.fsl.betsurf` run them in process), `larmorx_fsl::cli::betsurf::main`.
