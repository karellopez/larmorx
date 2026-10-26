# BET (`bet`, `bet2`, `betsurf`)

`lx.fsl.bet` and `lx.fsl.betsurf` (command line `larmorx fsl bet`, `larmorx fsl bet2`,
`larmorx fsl betsurf`): FSL 6.0.7.17's brain extraction and its skull and scalp surfaces.

**Two implementations, one so far.** FSL is distributed under the FSL Licence (non-commercial use
only), so larmorx keeps its FSL tools in two implementations ([licensing](../licensing.md)): the
bit-exact **replica**, `larmorx-nc fsl bet|bet2|betsurf` (package `larmorx-nc`, `crates-nc/`,
under the FSL Licence's terms; translated from FSL's source:
[PROVENANCE](../../crates-nc/larmorx-nc-bet/PROVENANCE.md)), done; and the clean-room
**original** in the main package (Apache-2.0), to be written from
[`specs/bet.md`](../../specs/bet.md). Until the original exists, `implementation="auto"` and
`"replica"` run the replica, as a separate program, when `larmorx-nc` is installed
(`pip install larmorx-nc`), and raise `lx.ReplicaNotFoundError` naming that command otherwise;
`implementation="original"` raises `NotImplementedError`. Each image's `implementation` names the
replica that ran.

**Status: `validated`** (the replica), against FSL 6.0.7.17 itself
([record](../validation/fsl-bet-replica.md), standard tier: 152 cases, 113 compared, every one
bit-identical; [full tier](../validation/fsl-bet-replica-full.md): 166 cases, 127 compared, 360
of 360 files byte for byte): every compared case gives FSL's exit status, messages and files byte
for byte (images: header and data; meshes as text), fMRIPrep's 29 BET calls on its twelve oracle
runs with BOLD runs included (the data panel's), whose outputs also have the values of fMRIPrep's
own (and of fMRIPrep's build of FSL in its image). 29 cases are rejected by both alike; in 6, FSL
crashes on what it leaves undefined and `reproduce` fills it (below); 4 variations wait for FLIRT
and FAST.

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

## `lx.fsl.bet(image, *, frac=0.5, gradient=0.0, radius=None, centre=None, smoothness=1.0, threshold=False, head_ct=False, variation=None, brain=True, mask=False, outline=False, skull=False, mesh=False, n_threads=1, implementation="auto", upstream_defects="fix")`

Returns a `BetResult`: `brain` (float32: the input times the mask; 4D with `"fmri"`), `mask`
(int16, 1 inside), `outline` (float32), `skull` (int16) and `mesh` (a `Surface`: `vertices`
`(n, 3)` in FSL's millimetre coordinates, `triangles` `(m, 3)`), each `None` unless asked for.
`image` is a path (any image FSL reads: NIfTI-1 or -2, single file or pair, gzipped or not,
ANALYZE) or an in-memory image. Without `variation` it runs `bet2`; with `"robust"` (`-R`),
`"small_fov"` (`-Z`) or `"fmri"` (`-F`) the `bet` script; `"eyes"` (`-S`), `"bias"` (`-B`),
`"surfaces"` (`-A`) and `"surfaces_t2"` (`-A2`) need FSL programs larmorx does not replicate yet
(`NotImplementedError`, naming them). The arguments are checked first (`ValueError`); FSL's
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
| `-c <x y z>` | `centre` | voxels of the file as stored |
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
| `FSLOUTPUTTYPE` | (always NIfTI-1 in Python) | on the command line read from the environment, as FSL does |

### `bet`'s variations

| `bet` option | Python `variation` | What it does |
|---|---|---|
| (none) | `None` | bet2 |
| `-R` | `"robust"` | bet2 again from the brain's centre until it moves less than a voxel (at most 11 runs); `-n` ignored |
| `-Z` | `"small_fov"` | pads the image to 100 mm in z with copies of its end slices, crops back (FSL-45) |
| `-F` | `"fmri"` | the first volume's mask (`-f 0.3`, then the user's `-f`), dilated, applied to every volume |
| `-S`, `-B`, `-A`, `-A2 <T2>` | `"eyes"`, `"bias"`, `"surfaces"`, `"surfaces_t2"` | pending: standard_space_roi, FLIRT, FAST |
| `-d` | (command line only) | keeps the temporary files |

## `lx.fsl.betsurf(t1, brain_surface, t1_to_standard, *, t2=None, masks=True, skull_mask=False, outlines=False, precision=0, n_threads=1, implementation="auto", upstream_defects="fix")`

Returns a `BetsurfResult`: the `inskull`, `outskull` and `outskin` surfaces, their masks
(`masks=True`, `-m`), the skull's mask (`skull_mask=True`, `-s`) and the surfaces drawn as images
(`outlines=True`, `-o`), all int16 on the T1's grid. `brain_surface` is `bet(..., mesh=True).mesh`
or a VTK file; `t1_to_standard` FLIRT's 4 x 4 matrix to MNI152 (a path or an array; it only decides
which profiles cross the eyes); without `t2` the surfaces come from the T1 alone (`--t1only`).
`precision` is `-p`. `lx.fsl.read_vtk(path)` reads a VTK mesh as FSL writes it.

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
  single slice that voxel lies below the image and FSL writes and reads outside the volume; the
  default starts at the nearest voxel inside (`reproduce` does the part FSL defines and warns).
  Where the centroid is far outside or not a number, FSL writes far outside the volume and
  crashes: the default fills from the nearest voxel for a real position and refuses otherwise;
  `reproduce` skips the fill (the mask holds the drawn surface only) and warns.
- **FSL-44** (A): `bet` splits names with blanks into words and stops its options at an empty
  argument; the default keeps each argument whole.
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

Where FSL crashes on what it leaves undefined, the default refuses with the reason and
`reproduce` keeps everything FSL defines, fills the rest deterministically and warns (D13's
failure cases): **FSL-41** (no voxel strictly between the 2nd and 98th percentiles inside the
initial sphere, a constant or binary image: FSL reads the median from an empty list; `reproduce`
takes it as 0) and **FSL-52** (a mesh file FSL reads without checks: `reproduce` takes 0 for the
values FSL never sets and leaves out the faces that name missing vertices). Inside `bet`, where FSL's
bet2 crashes, the script's ERR trap reports the failure and exits 1: that is FSL's defined outcome,
and `reproduce` gives it (bet2 alone fills instead). Kept as FSL's and
documented (rule C):
**FSL-49** (betsurf keeps the T1's scalp estimate when the T2's lies further out) and **FSL-57**
(betsurf's smoothness weight set to 0 after it was used: no effect).

## Deliberate differences

- Where FSL aborts on an uncaught exception (a missing input, FSL-40's `.img` name), the
  replica prints FSL's messages before the abort and one line of its own, and exits 1 (FSL: 134).
  Where it crashes on what it leaves undefined (FSL-41, FSL-52, FSL-43 far outside), the default
  refuses (exit 1) and `reproduce` finishes with the undefined part filled (exit 0, the defect
  named); FSL ends by a signal. FSL's buffered standard output before such a crash may be lost
  (with `-v`); the replica's goes on.
- The `bet` script's messages name the script by its path (`$0`: `<FSLDIR>/bin/bet failed during
  command:...`); the replica names it `bet`.
- The replica needs no `FSLDIR` and runs none of FSL's programs; `bet -S`, `-B`, `-A`, `-A2`
  are refused as pending.

## Known last-bit differences

None: the replica carries the C library functions FSL's programs call as this machine's glibc
computes them (`pow`, `exp`, `tanh`, and the x87 `long double` arithmetic of FSL's NIfTI layer;
`crates-nc/larmorx-nc-core/tools/libm-verify`), on every platform; NaN and infinite voxels give
SSE's NaN bits on every platform (`larmorx-nc-core`'s `sse`; `validation/probes/fsl-bet-nan`).

## Speed

Informal timings, one run each on a shared machine (not a benchmark report;
[validation/investigations/fsl-bet](../../validation/investigations/fsl-bet/README.md)):
fMRIPrep's call on ds000005's BOLD reference took 1.14 s in FSL and 0.62 s in the replica;
`bet2 -m` on a T1w 2.27 s and 1.06 s; `betsurf --t1only -m` 31.4 s and 15.1 s (13.6 s at 3
threads: the replica smooths in parallel, with the same result). There is no benchmark report
yet.

## Rust

The replica's Rust API is in the crate `larmorx-nc-bet` (FSL Licence): `bet`, `bet2`, `betsurf`
(the `larmorx-nc` program's entry points: the original arguments, the settings, D13's mode, the
thread count), `bet2::main`, `bet::main`, `betsurf::main`; `bet2::compute` runs the extraction on
a `larmorx_nc_core::volume::Volume<f32>`. FSL's libraries are the shared core's,
`larmorx-nc-core` (its [PROVENANCE](../../crates-nc/larmorx-nc-core/PROVENANCE.md) lists the
API). The Apache-2.0 crates cannot use them.
