# BET (`bet`, `bet2`, `betsurf`)

`lx.fsl.bet` and `lx.fsl.betsurf` (command line `larmorx fsl bet`, `larmorx fsl bet2`,
`larmorx fsl betsurf`): FSL 6.0.7.23's brain extraction and its skull and scalp surfaces.

The ports come first. One section per program follows (`bet`, `bet2`, `betsurf`); each is the
source of the program's reference page on the documentation site (`python -m larmorx_validation
reference`), with its options table checked against the code. What the three share comes after
them, and the page's dates are in its [History](#history).

## Ports

FSL's licence allows non-commercial use only, so the two ports of `bet`, `bet2` and `betsurf`, a
replica and a clean room, live in different packages ([licensing](../licensing.md) says what each
package holds):

- **The replica** is translated from FSL's source. The package `larmorx-nc` holds it, under FSL's
  licence, and `larmorx` runs it as a separate program, `larmorx-nc fsl bet|bet2|betsurf`, when
  `larmorx-nc` is installed (`pip install larmorx-nc`).
- **The clean room** is written from [`specs/bet.md`](../../specs/bet.md), the BET paper and runs
  of FSL, without FSL's source or the replica. It is part of `larmorx`, under Apache-2.0
  (`crates/larmorx-fsl`, with its [PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md)).

`implementation="auto"` runs the replica when `larmorx-nc` is installed, and the clean room
otherwise. `"replica"` and `"clean-room"` choose one (`"original"`, 0.1.0rc1's spelling of
`"clean-room"`, still works). Each image's `implementation` names the port that ran. On the
command line, `--implementation auto|replica|clean-room` before the family chooses alike, from the
next release; in 0.1.0rc1 the variable `LARMORX_IMPLEMENTATION` chooses (`original` for the clean
room).

## `bet`

<!-- reference
program: bet
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.bet
command-line: crates/larmorx-fsl/src/cli/bet.rs
records: {replica: [fsl-bet-replica, fsl-bet-replica-full], clean-room: [fsl-bet]}
evidence: fsl-bet
evidence-sections: {data: "The standard images the variations read"}
usage-only: ["-h"]
-->

`bet` finds the brain in a head image. It writes the brain and, on request, its mask, its
outline, an approximate skull and the brain's surface as a mesh. It runs FSL's brain extraction,
`bet2`, alone or inside one of seven variations (below).

### Python (`bet`)

`lx.fsl.bet(image, *, frac=0.5, gradient=0.0, radius=None, centre=None, smoothness=1.0, threshold=False, head_ct=False, variation=None, t2=None, brain=True, mask=False, outline=False, skull=False, mesh=False, tanh="glibc", fsl_data=None, n_threads=1, implementation="auto", upstream_defects="fix")`

`image` is a path (any image FSL reads: NIfTI-1 or -2, single file or pair, gzipped or not,
ANALYZE) or an in-memory image. Without `variation` it runs `bet2`. With `"robust"` (`-R`),
`"small_fov"` (`-Z`) or `"fmri"` (`-F`) it runs the `bet` script. `"eyes"` (`-S`), `"bias"`
(`-B`), `"surfaces"` (`-A`) and `"surfaces_t2"` (`-A2`, with `t2`) run the script with FLIRT,
standard_space_roi, FAST, fslmaths, fslstats and betsurf; each port runs its own copies of those
programs, in its own process. `"surfaces"` and `"surfaces_t2"` also return betsurf's surfaces
(`BetResult.surfaces`, a `BetsurfResult` with the masks, the skull's mask and the outlines).

These four read FSL's standard images, which are FSL's data under the FSL Licence and which this
package never ships. They come from `fsl_data` (a directory laid out as `$FSLDIR/data`), else from
the variable `LARMORX_FSL_DATA`; when given, that is the only place looked. Otherwise the clean
room reads them from the copy `larmorx-nc` installs (as data), else `$FSLDIR/data`, else
larmorx's data cache, filled once by a checksum-verified download of FSL's `fsl-data_standard`
package (`larmorx fetch fsl-standard` fetches it ahead; [fsl-roi.md](fsl-roi.md), "FSL's standard
images"). The replica reads `$FSLDIR/data`, else its own copy. An image found nowhere is refused
before anything runs, naming the image and every place looked (a `RuntimeError` from the clean
room).

The arguments are checked first (`ValueError`). FSL's settings belong to the call: the outputs are
written as NIfTI-1 and read back; `FSLOUTPUTTYPE`, `FSL_LOAD_NIFTI_EXTENSIONS` and every other
`FSL*` variable of the process are never passed; and no FSL program is looked for on the `PATH`.

### Options of `bet`

| FSL option | Python | Notes |
|---|---|---|
| `<input> <output>` | `image`; the result | outputs `<output>`, `_mask`, `_overlay`, `_skull`, `_mesh.vtk` |
| `-f <f>` | `frac` | 0 to 1; smaller gives a larger brain |
| `-g <g>` | `gradient` | -1 to 1; positive gives a larger brain at the bottom |
| `-r <r>` | `radius` | mm; the initial sphere is half of it |
| `-c <x y z>` | `centre` | voxels of the file as stored; `0 0 0` is "not given" (the centre of gravity), as in FSL |
| `-t` | `threshold=True` | also thresholds the brain and mask |
| `-m` | `mask=True` | |
| `-o` | `outline=True` | |
| `-s` | `skull=True` | |
| `-n` | `brain=False` | |
| `-e` | `mesh=True` | VTK; also switches the self-intersection test to the exact one |
| `-v` | (command line only) | |
| `-h` | (command line only) | |
| `--tanh=correctly-rounded` (LARMORX's own option; also `--tanh correctly-rounded`, and passed on by `bet`) | `tanh="correctly-rounded"` | computes the surface's smoothness weight with the correctly rounded `tanh` (CORE-MATH's, `larmorx_core::math::tanh`) instead of glibc 2.35's, which FSL calls and which is the default (`glibc`; decision GLIBC-06). No notice: it is asked for. Where a surface self-intersects and is found again, the last bit can change the result (4 of the 13 real images of the clean room's standard comparison: [its findings](../findings/fsl-bet-clean-room.md)). Clean room only: `implementation="replica"` refuses it, and `"auto"` runs the clean room |
| (`bet2`'s `-w`) | `smoothness` | bet2 only (`bet` does not pass it) |
| (`bet2`'s `--ct`) | `head_ct=True` | bet2 only: the input in Hounsfield units |
| `LARMORX_FSL_DATA` | `fsl_data` | where `-S`, `-B`, `-A` and `-A2` read FSL's standard images ([`bet`'s Python call](#python-bet)) |
| `FSLOUTPUTTYPE` | (always NIfTI-1 in Python) | on the command line read from the environment, as FSL does (unset: `NIFTI_GZ`, FSL-11) |

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

### Examples of `bet`

<!-- inputs: boldref.nii.gz=head -->

```python example
import larmorx as lx

# As fMRIPrep calls BET on its BOLD reference.
r = lx.fsl.bet("boldref.nii.gz", frac=0.2, mask=True)
r.brain, r.mask
# The port that ran: the clean room of larmorx,
# or the replica once larmorx-nc is installed.
r.mask.implementation

# The same call, the clean room chosen.
c = lx.fsl.bet("boldref.nii.gz", frac=0.2, mask=True, implementation="clean-room")
c.mask.implementation.kind  # -> 'clean-room'
```

```bash example
larmorx fsl bet boldref.nii.gz boldref_brain.nii.gz -f 0.20 -m
```

### Ports of `bet`

The replica gives FSL 6.0.7.23's exit status, messages and files byte for byte in 201 of the 263
cases of its record, which holds `bet`, `bet2`, `betsurf`, `standard_space_roi` and `fslroi`; 47
more are rejected by both. The clean room is within the thresholds in 198 of the 200 compared
cases of its record, which holds the same programs. One of the two others is `bet`'s: a
3.5-year-old's `bet -B`. The boundaries of its brain and mask lie up to 2.24 voxels from FSL's,
where the threshold is 2, and the mask's boundary distance at the 95th percentile is 1.0 voxel,
where the threshold is 0.5. Their Dice coefficients, 0.9989 and 0.9983, are within the threshold
of 0.995.

## `bet2`

<!-- reference
program: bet2
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.bet
command-line: crates/larmorx-fsl/src/cli/bet2.rs
records: {replica: [fsl-bet-replica, fsl-bet-replica-full], clean-room: [fsl-bet]}
evidence: fsl-bet
-->

`bet2` is FSL's brain extraction itself, which the `bet` script runs. It deforms a tessellated
sphere onto the brain's surface in a head image and writes the brain and, on request, its mask,
its outline, an approximate skull and the surface as a mesh.

### Python (`bet2`)

`lx.fsl.bet` without `variation` runs `bet2` (the function's description: [`bet`](#bet)).

### Options of `bet2`

| FSL option | Python | Notes |
|---|---|---|
| `<input> <output>` | `image`; the result | outputs `<output>`, `_mask`, `_overlay`, `_skull`, `_mesh.vtk` |
| `-f <f>` | `frac` | 0 to 1; smaller gives a larger brain |
| `-g <g>` | `gradient` | -1 to 1; positive gives a larger brain at the bottom |
| `-r <r>`, `--radius` | `radius` | mm; the initial sphere is half of it |
| `-c <x y z>` | `centre` | voxels of the file as stored; `0 0 0` is "not given" (the centre of gravity), as in FSL |
| `-w <r>`, `--smooth` | `smoothness` | bet2 only (`bet` does not pass it) |
| `-t`, `--threshold` | `threshold=True` | also thresholds the brain and mask |
| `--ct` | `head_ct=True` | bet2 only: the input in Hounsfield units |
| `-m`, `--mask` | `mask=True` | |
| `-o`, `--outline` | `outline=True` | |
| `-s`, `--skull` | `skull=True` | |
| `-n`, `--nooutput` | `brain=False` | |
| `-e`, `--mesh` | `mesh=True` | VTK; also switches the self-intersection test to the exact one |
| `-v`, `--verbose` | (command line only) | |
| `--binaryMask <image>` | (command line only) | |
| `-h`, `--help` | (command line only) | |
| `--parameter_d1=`, `--parameter_d2=` | (command line only) | hidden search depths (7 and 3) |
| `--tanh=correctly-rounded` (LARMORX's own option; also `--tanh correctly-rounded`, and passed on by `bet`) | `tanh="correctly-rounded"` | computes the surface's smoothness weight with the correctly rounded `tanh` (CORE-MATH's, `larmorx_core::math::tanh`) instead of glibc 2.35's, which FSL calls and which is the default (`glibc`; decision GLIBC-06). No notice: it is asked for. Where a surface self-intersects and is found again, the last bit can change the result (4 of the 13 real images of the clean room's standard comparison: [its findings](../findings/fsl-bet-clean-room.md)). Clean room only: `implementation="replica"` refuses it, and `"auto"` runs the clean room |
| `FSLOUTPUTTYPE` | (always NIfTI-1 in Python) | on the command line read from the environment, as FSL does (unset: `NIFTI_GZ`, FSL-11) |
| (`bet`'s variations) | `variation`, `t2`, `fsl_data` | `bet` only ([its options](#options-of-bet)) |

### Examples of `bet2`

<!-- inputs: T1w.nii.gz=head -->

```python example
import larmorx as lx

b = lx.fsl.bet("T1w.nii.gz", mesh=True)
b.brain, b.mesh
```

```bash example
larmorx fsl bet2 T1w.nii.gz T1w_brain -e
```

### Ports of `bet2`

The replica gives FSL 6.0.7.23's exit status, messages and files byte for byte in 201 of the 263
cases of its record, which holds `bet`, `bet2`, `betsurf`, `standard_space_roi` and `fslroi`; 47
more are rejected by both. The clean room's record holds the same programs. Of its 380 brains,
masks and outlines (`betsurf`'s counted apart), 378 are FSL's bit for bit; the two others are
`bet -B`'s. 11 of 11 meshes have FSL's vertices to the file's 6 decimals.

## `betsurf`

<!-- reference
program: betsurf
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.betsurf
command-line: crates/larmorx-fsl/src/cli/betsurf.rs
records: {replica: [fsl-bet-replica, fsl-bet-replica-full], clean-room: [fsl-bet]}
evidence: fsl-bet#betsurf
-->

`betsurf` finds the inner skull, the outer skull and the scalp of a head, from `bet2`'s brain
surface (`-e`), the T1 image (and a T2) and the T1's registration to MNI152.

### Python (`betsurf`)

`lx.fsl.betsurf(t1, brain_surface, t1_to_standard, *, t2=None, masks=True, skull_mask=False, outlines=False, precision=0, n_threads=1, implementation="auto", upstream_defects="fix")`

`brain_surface` is `bet(..., mesh=True).mesh` or a VTK file; `t1_to_standard` is FLIRT's 4 x 4
matrix to MNI152 (a path or an array; it only decides which profiles cross the eyes). Without `t2`
the surfaces come from the T1 alone (`--t1only`). `precision` is `-p`. `lx.fsl.read_vtk(path)`
reads a VTK mesh as FSL writes it.

### Options of `betsurf`

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
| `FSLOUTPUTTYPE` | (always NIfTI-1 in Python) | read when the first image is written, as FSL does (unset: `NIFTI_GZ`, FSL-11) |

### Examples of `betsurf`

<!-- inputs: T1w.nii.gz=head T1w_to_MNI152.mat=matrix -->

```python example
import larmorx as lx

# The skull and scalp: bet2's mesh, then betsurf with the T1's registration to MNI152
b = lx.fsl.bet("T1w.nii.gz", mesh=True)
s = lx.fsl.betsurf("T1w.nii.gz", b.mesh, "T1w_to_MNI152.mat", skull_mask=True)
s.inskull_mask, s.outskull_mask, s.outskin_mask, s.skull_mask
```

```bash example
larmorx fsl bet2 T1w.nii.gz T1w_brain -e
larmorx fsl betsurf --t1only -o -m -s T1w.nii.gz T1w_brain_mesh.vtk T1w_to_MNI152.mat T1w
```

### Ports of `betsurf`

The replica gives FSL 6.0.7.23's exit status, messages and files byte for byte in 201 of the 263
cases of its record, which holds `bet`, `bet2`, `betsurf`, `standard_space_roi` and `fslroi`; 47
more are rejected by both. The clean room's masks and outlines are within its thresholds (a Dice coefficient of at least 0.99, a
boundary distance of at most 1 voxel at the 95th percentile) in 114 of 116, 34 of them bit for bit.
One of the record's two misses is `betsurf`'s: the scalp of ds002603 with its T2, whose boundary
distance is 2 voxels at the 95th percentile.

## The records

**The replica: `validated`.** Its [record](../validation/fsl-bet-replica.md) is the standard
comparison with FSL 6.0.7.23 itself, `-S`, `-B`, `-A`, `-A2`, standard_space_roi and fslroi
included: 263 cases. In the 201 cases both complete, the replica gives FSL's exit status, messages
and files byte for byte (images: header and data; meshes as text): 600 of 600 files. 47 cases are
rejected by both alike. In 13, FSL crashes on what it leaves undefined and `reproduce` fills it in
([below](#defects-of-the-original-corrected-by-default)). One waits for fslpy's `imcp` conversion
(standard_space_roi's: [fsl-roi.md](fsl-roi.md), "imcp's conversion"). The record also checks
FSL's own step from 6.0.7.17 in every case. The [full comparison](../validation/fsl-bet-replica-full.md),
with FSL 6.0.7.17, holds 173 cases: in its 129 compared cases, 364 of 364 files are byte for
byte.

The compared cases hold fMRIPrep's 29 BET calls on its twelve recorded runs, BOLD runs included
(the data panel's). Their outputs also have the values of fMRIPrep's own outputs, and of fMRIPrep's
build of FSL in its image. The records include random testing's permanent cases, 17 in the
standard comparison (`difftest/...`; BET's random testing:
[docs/validation/difftest.md](../validation/difftest.md)). The replica needs no more memory than
FSL in any measured call (`tests/parity/test_memory_peak.py`).

**The clean room: `experimental`.** Its [record](../validation/fsl-bet.md) is the standard
comparison with FSL 6.0.7.23: 252 cases. **198 of its 200 compared cases are within the
thresholds**; the other two miss thresholds of the boundary distance
([below](#the-clean-room-crateslarmorx-fsl)). 46 more are
rejected by both alike, and in 6 `reproduce` fills in what FSL leaves undefined; none is pending.
The 252 cases are the 164 kept from the second record (168 cases), unchanged in status and
details, and 88 new ones:

- `standard_space_roi`'s and `fslroi`'s 57: 45 within the thresholds, every one FSL's bit for bit,
  and 12 rejected by both ([fsl-roi.md](fsl-roi.md));
- the options' 31: `-S` and `-B` FSL's bit for bit in every case (every output and every `-d`
  file; an adult, a tumour, an 82-year-old with the neck, `-S -n`, a NaN head, a pair output) but a
  3.5-year-old's `-B`; `-A` and `-A2` with bet2's outputs and FLIRT's matrices FSL's, and
  betsurf's masks within its thresholds; `-F` and `-Z` of int8 heads FSL's.

The second record's own numbers:

- exit statuses and messages FSL's byte for byte;
- 217 of 217 brains, masks and outlines of `bet2` and `bet` FSL's bit for bit (the first record
  counted 12 reported Dice values, with phantoms' known brains among them, as "within the
  thresholds");
- 13 of 13 skull images bit for bit, and the meshes FSL's to the file's 6 decimals;
- the surfaces that run again after a self-intersecting first pass (a 75-year-old's T1w, 7 T, a
  tumour) bit for bit;
- fMRIPrep's 29 BET calls on its recorded runs all bit for bit, with fMRIPrep's own outputs'
  values;
- headers FSL's byte for byte but for `descrip` (`larmorx fsl bet2`), the qform's quaternion
  included.

Of the second record's 168 cases, the first record's 142 were unchanged. The 26 new ones were
betsurf's: its options, messages and failure cases, FSL-48, -52, -53 and -54 among them, and five
real heads with bet2's meshes and FLIRT's matrices made by FSL.

## Defects of the original, corrected by default

Corrected by default, reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); each correction that changes a result is named as an
`lx.UpstreamDefectWarning` (on the command line `larmorx-nc: upstream defect <ID> ...`). Decided by
the project's rules A to D ([register](../findings/upstream-defects.md)):

- **FSL-11** (A): with `FSLOUTPUTTYPE` unset FSL's bet2 and betsurf stop with FSL's message at
  their first image (exit 1; `bet`'s script fails with them; betsurf writes its inner skull's mesh
  first). Both ports, the clean room and the replica, write the images as `NIFTI_GZ` (`.nii.gz`:
  the bytes `FSLOUTPUTTYPE=NIFTI_GZ` gives) and name FSL-11 on one line per command (one for
  `bet`'s bet2 runs), in the same words (the replica's line starts `larmorx-nc:`); `reproduce`
  stops as FSL does. Command line only: the Python functions set the type themselves.
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
- **FSL-46** (A): `bet`'s cleanup removes every `<output>_tmp*` file; the default only its own
  (every `<output>_tmp...` file the run's programs created or changed, `standard_space_roi`'s and
  FAST's in `-S` and `-B` included). A step that fails ends the script before its cleanup, in
  both modes (FSL's `ERR` trap: its temporary files are left, as runs of FSL show).
- **FSL-47** (A): `betsurf --t1only` fills the scalp mask from the outer skull surface's centre in
  its padded grid; the default from the scalp's (named only when the mask changes).
- **FSL-48** (B): a T2 on another grid than the T1 is sampled at the T1's voxel coordinates; the
  default refuses.
- **FSL-50** (A): one skull profile with an empty search range ends betsurf (status 255, no
  message, no output); the default drops that profile. (The clean room, since 2026-11-05: FSL
  6.0.7.23 was seen to run on and count such a profile's scalp on all 43 images where it occurred,
  so the clean room counts the scalp in both modes and never stops.)
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
  that is not a number; `reproduce` gives what `dc` prints, its messages too. (The clean room,
  from runs of FSL 6.0.7.23: `fslstats -M` and `-S` print six decimals, never an exponent; a brain
  with a negative mean gives FSL's threshold `|mean| + 3.5 × SD` and one `dc: stack empty` on
  standard error, which `reproduce` gives (case `defects/eyes-negative-mean`); `nan` and `inf`,
  which only NaN or infinite voxels make and which FSL's script never reaches with them, are
  refused in both modes, `reproduce` saying that dc's commands are not reproduced.)
- **FSL-217** (A): `bet -F` and `-Z` cut the input with fslroi (and `-Z` joins the slices with
  fslmerge), which write an int8 image as uint8 with the same bytes: bet2 then runs on negative
  values turned into 128 to 255. The default keeps int8 and its values, named where the result
  differs (a negative value, or `-d` keeping the int8 temporary files); `reproduce` gives FSL's
  uint8.

Where FSL crashes on what it leaves undefined, the default refuses with the reason and
`reproduce` keeps everything FSL defines, fills the rest deterministically and warns: **FSL-41**
(no voxel strictly between the 2nd and 98th percentiles inside the initial sphere, a constant or
binary image: FSL reads the median from an empty list; `reproduce` takes it as 0), **FSL-52** (a
mesh file FSL reads without checks: `reproduce` takes 0 for the values FSL never sets and leaves
out the faces that name missing vertices) and **FSL-58** (no voxel of the T1 inside the brain
surface: betsurf reads its 98th percentile from an empty list; `reproduce` takes it as 0). Inside
`bet`, where FSL's bet2 crashes, the script's ERR trap reports the failure and exits 1: that is
FSL's defined outcome, and `reproduce` gives it (bet2 alone fills instead). Kept as FSL's and
documented (rule C): **FSL-49** (betsurf keeps the T1's scalp estimate when the T2's lies further
out) and **FSL-57** (betsurf's smoothness weight set to 0 after it was used: no effect).

### glibc's maths functions (GLIBC-07)

FSL's bet2 takes its initial radius and the fractional threshold's power from glibc 2.35's `pow`,
and betsurf its blurring's weights from glibc's `exp` (`specs/bet.md` §8), which are not correctly
rounded. The clean room computes them correctly rounded by default and takes glibc's with
`upstream_defects="reproduce"` (`larmorx_core::math`'s `*_glibc` functions, proved bit for bit
against glibc 2.35; decided on 2026-11-04: glibc's bits for `reproduce` only). The
surface is an iterative fit: one power's last bit can move it, and bet2 cannot tell without fitting
twice, so neither mode names GLIBC-07: the default's correctly rounded functions can move a
surface's last bits. bet2's `tanh` is a separate choice (GLIBC-06, `tanh=`, above).

## Deliberate differences

- Where FSL aborts on an uncaught exception (a missing input, FSL-40's `.img` name), the
  replica prints FSL's messages before the abort and one line of its own, and exits 1 (FSL: 134).
  Where it crashes on what it leaves undefined (FSL-41, FSL-52, FSL-43 at a centre that is not a
  number), the default refuses (exit 1) and `reproduce` finishes with the undefined part filled
  (exit 0, the defect named); FSL ends by a signal. FSL's buffered standard output before such a
  crash may be lost (with `-v`); the replica's goes on.
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
6.0.7.23, the replica and the clean room on the same calls): fMRIPrep's call on ds000005's BOLD
reference took 0.67 s in FSL, 0.39 s in the replica and 0.24 s in the clean room (0.06 s on 24
threads); `bet2 -m` on four T1w images 1.25 to 3.06 s, 0.72 to 1.75 s and 0.49 to 1.21 s; betsurf
on four real heads 6.5 to 18.4 s, 3.9 to 9.9 s (2.6 to 5.8 s on 24 threads: the replica smooths in
parallel, with the same result) and 2.1 to 5.3 s (0.6 to 1.4 s on 24). Earlier informal timings of
the replica: [validation/investigations/fsl-bet](../../validation/investigations/fsl-bet/README.md).

## Rust

The replica's Rust API is in the crate `larmorx-nc-bet` (FSL Licence): `bet`, `bet2`, `betsurf`
(the `larmorx-nc` program's entry points: the original arguments, the settings, the mode of the
defects' corrections, the thread count), `bet2::main`, `bet::main`, `betsurf::main`;
`bet2::compute` runs the extraction on a `larmorx_nc_core::volume::Volume<f32>`. FSL's libraries
are the shared core's, `larmorx-nc-core` (its
[PROVENANCE](../../crates-nc/larmorx-nc-core/PROVENANCE.md) lists the API). The Apache-2.0 crates
cannot use them.


## The clean room (`crates/larmorx-fsl`)

Written from [`specs/bet.md`](../../specs/bet.md), S. M. Smith's paper (Human Brain Mapping
17(3), 2002) and black-box runs of FSL 6.0.7.23's programs, without FSL's source or the replica
([PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md); what the runs showed beyond the spec:
[findings/fsl-bet-clean-room.md](../findings/fsl-bet-clean-room.md)). Since 2026-10-30: `bet2`
complete (every option, message, exit status and failure case of the spec, with `fix` and
`reproduce` for FSL-40, -42, -43, -56, -60, -61 and -62) and the `bet` script's default, `-R`
(FSL-63), `-F` and `-Z` (FSL-45), FSL-44 and FSL-46. Since 2026-11-05 (the clean-room catch-up):
**`-S`, `-B`, `-A`, `-A2`** (`specs/bet.md` §5.3, §5.4; `larmorx_fsl::cli::bet_script`), each the
script's sequence of the clean room's own programs in one process (bet2, `standard_space_roi`,
FLIRT, `fslmaths`, `fslstats`, FAST, betsurf, and the script itself for `-B`'s robust run), FSL's
standard images found before anything runs (an image found nowhere refused, naming every place
looked), bash's reports of an abort and the `ERR` trap's line where a program fails (a failed
run's temporary files left, as FSL leaves them), FSL-216 (the eye threshold in dc's exact decimal
arithmetic), FSL-63 for `-B -s` and FSL-46 with the programs' files; FSL-217 inside `-F` and `-Z`
(fslroi's types: an int8 input's temporary images uint8 with the same bytes in `reproduce`). Since
2026-10-31: **`betsurf`** complete (spec §6: every option of §6.1, `-p`'s retessellation, the VTK,
OFF and FreeSurfer meshes, `manual_input`, every message; `fix` and `reproduce` for FSL-43, -47,
-48, -50 to -55 and -58, FSL-49 and FSL-57 kept as FSL's), so `implementation="auto"` without
`larmorx-nc` runs the clean room for every BET program.

**betsurf, what agrees and what does not.** The profiles' decisions (§6.4) are FSL's: on 330 of
330 single profiles probed one at a time (a tiny triangle that casts exactly one profile; FSL's
`-v` says whether it set it aside), 127 of 128 triangles of a T1 and T2 and 64 of 64 of a T1
alone; the share of profiles set aside within 0.06 % of FSL's on the record's real images. Where
the votes are dense the surfaces are FSL's (the phantom's inner and outer skull to the file's 6
decimals, also with `-p 1`); where few profiles vote (a skull base where nearly every profile is
set aside) the inner skull settled elsewhere by up to 11 mm on a few vertices until step 5's
rules were found (2026-11-05: ds002603's inner skull now within 0.026 mm of FSL's, 95th
percentile), and the scalp differs a little everywhere (95 % of the vertices within 0.3 to 3 mm on
real heads). The record holds betsurf to its masks (Dice >= 0.99 and the boundary distance's 95th
percentile <= 1 voxel, specs/bet.md §11): 14 of its 67 masks and outlines are FSL's bit for bit
and 51 more within these thresholds; on the five real heads the masks' Dice coefficients are
0.9935 to 0.9998 (the record of 2026-11-05: the inner skull 0.9990 to 0.9998, the scalp 0.9935 to
0.9991), and the scalp of ds002603 with its T2 misses the boundary threshold (its 95th percentile
2 voxels): one of the record's two failures. The other (since 2026-11-05): `real/child-3y-B`, a
3.5-year-old's `bet -B`, every `-d` file FSL's (both `standard_space_roi` runs, FAST's bias field
and the unbiased images) but the robust run's second bet2 pass, whose `-c` (FSL's centre, given in
voxels) falls on a neurologically stored image. The mask's Dice coefficient, 0.9983, is within
its threshold (at least 0.995), but its boundary distance misses both of its thresholds: 1.0 voxel
at the 95th percentile, where the threshold is 0.5, and 2.24 voxels at its largest, where the
threshold is 2; the brain's largest is 2.24 voxels too. It is the open question of random
testing's cases 622 and 677. The questions for the spec writer are in the findings.

**`tanh`** (GLIBC-06, decided on 2026-10-31): the smoothness weight's `tanh` is glibc
2.35's by default (`larmorx_core::math::tanh_glibc`, fdlibm's, proved equal to the system's on
12.9 billion inputs), so the surfaces are FSL's; `--tanh correctly-rounded` (`tanh=
"correctly-rounded"`) takes the correctly rounded `larmorx_core::math::tanh` (CORE-MATH's),
which gives another, equally valid surface on 4 of the 13 real images of the standard comparison
(the findings give the measurement). The surface follows `specs/bet.md` §8.1 (the deformation's
arithmetic, operation by operation), §8.2 (the quaternion's precision path) and §4.7 (the skull
search).

**Random testing** (a 30-minute run, 2026-10-31, `oracles/difftest/2026-10-31-cleanroom-betsurf`,
1,846 cases, both ports; the first, 2026-10-30, in the findings): the replica agreed in every case
it finished (3 timeouts). The clean room agreed in 957 of 961 `bet2` cases and 513 of 532 `bet`
cases: the 21 others are, all but one, neurologically stored images (`bet -R` and `-c`; case 1457
stored the other way along x is FSL's exactly), the class of cases 622 and 677, open with the spec
writer. betsurf: 221 of 353 agree, 6 are FSL-51's (undefined); 126 differ, on small synthetic
images (11 to 32 voxels per side) where the profiles' decisions differ (case 36: FSL sets aside
96.8 % of its profiles, the clean room 87.8 %), and in 2 FSL-50's `reproduce` stops where FSL runs
on: both for the spec writer (findings, "Random testing, part 2b").

**Random testing, the BET options part's run** (2026-11-04,
`oracles/difftest/2026-11-04-bet-options-6b`, 2,699 cases; its 176 cases where the clean room
differed judged again on 2026-11-05 in the clean-room catch-up, findings: "The BET part's
random-testing differences"): most began at the profiles' decisions on these small heads, which
single profiles (5,290 of 11 images, FSL's `-v` saying whether it set each aside) traced to four
readings of §6.4, now FSL's: the backward searches and the T1-only scan never read a profile's
first sample; step 5's first bright sample lies strictly within 10 mm of the inner skull, and each
of its searches starts after the sample the previous one found; an outer-skull search range with no
sample counts the profile's scalp only, and FSL runs on (**FSL-50's stop**, as the spec states it,
was seen on none of the 43 images where its condition occurs, so the clean room no longer stops in
`reproduce`). With them the share of profiles set aside is FSL's on 162 of the 164 images (74
before), 25 of the 176 cases agree, and ds002603's inner skull is FSL's to 0.026 mm (95th
percentile; it was 0.95 mm, up to 11 mm). What still differs, for the spec writer: the scalp's
deformation (§6.5: 0.24 to 3.4 mm on images whose skulls are FSL's exactly), `-p` on a mesh
collapsed to one point (another sharing of midpoints), and `bet -R`/`-c` on neurologically stored
images (the centre's conversion, cases 622 and 677). `bet -Z -d` of a float64 image now keeps the
doubles in its temporary images (FSL's are float64).

**Built for speed:** bet2's vertices of each iteration are updated in parallel (each from the
previous iteration's positions only), the self-intersection measure finds close pairs in a grid
of cells instead of over all pairs, the fill is a stack fill over the grid, and no image is held
twice; betsurf casts its profiles in parallel (each vertex's into its own list, counted in
vertex order), smooths each axis in parallel with exact weights in two buffers, and deforms each of
its three surfaces with every vertex's step computed in parallel from the previous positions.
Every parallel step runs in the call's own pool of `n_threads` (the command line:
`OMP_NUM_THREADS`, else every core) with the same result at any thread count. Memory: betsurf's
peak is below FSL's on the memory test's call (144 MiB against 175: the T1 clamped in place, the
images freed after the profiles); `bet -S`, `-B`, `-A2` 58 to 62 MiB against FSL's 67 to 115
(their programs one after the other, as the script runs them). `-S`, `-B`, `-A`, `-A2`
(2026-11-05, the main machine, a real adult and a child): 1.4 to 2.9 times FSL's speed on one
thread, 6 to 14 times on 28 (`-B -m` 6.2 s against FSL's 85.8; the findings give each time).
Timings: [benchmarks/fsl-bet.md](../benchmarks/fsl-bet.md) (the clean room 2.5 to 2.9 times
FSL's speed on one thread for bet2 and bet, 3.0 to 3.6 for betsurf; 6.5 to 11.6 and 10.7 to 13.5
times on 24 threads).

**Deliberate differences of the clean room:** its images' `descrip` is `larmorx fsl bet2`
(betsurf's too); where FSL aborts or crashes it prints FSL's messages before that point and one
line of its own ending in `(larmorx)`, and exits 1; `bet`'s messages name the script `bet`;
betsurf's `manual_input` refuses at the end of its input, where FSL waits forever; `-p` above 10
is refused (the meshes would not fit any memory).

**Rust:** `larmorx_fsl::bet::find_surface` (the surface on a `larmorx_fsl::bet::Vol`: the initial
parameters, the passes and their self-intersection), `brain_mask`, `outline`, `masked`,
`skull::skull`, `cormack`; betsurf's steps in `larmorx_fsl::betsurf`: `read_matrix`,
`robust_maximum` and `clamp` (§6.3), `WorkGrid` (the padded working grid), `votes` (the profiles
and their counting images, §6.4), `smooth`, `deform`, `sphere`, `retessellate` and
`surface_mask` (§6.5), the profile searches in `larmorx_fsl::betsurf::profile`; the command
lines `larmorx_fsl::cli::run` / `run_in` (an environment table and a directory given, as
`lx.fsl.bet` and `lx.fsl.betsurf` run them in process), `larmorx_fsl::cli::betsurf::main`.

## History

- **2026-10-29:** FSL 6.0.7.23 became the original (until then FSL 6.0.7.17's programs); what
  changed, `bet2 --binaryMask` among it, is in [findings/fsl-bet.md](../findings/fsl-bet.md),
  "FSL 6.0.7.23".
- **2026-10-30:** the clean room's `bet2` and the `bet` script.
- **2026-10-31:** the clean room's `betsurf`, and its second record (168 cases); decision
  GLIBC-06 (glibc's `tanh` by default, the correctly rounded one on request).
- **2026-11-04:** the replica's `-S`, `-B`, `-A` and `-A2`, which run the replicas of the programs
  they need; the replica's record made again with them, standard_space_roi and fslroi.
- **2026-11-05:** the clean room's `-S`, `-B`, `-A` and `-A2`, which run the clean room's own
  programs (the clean-room catch-up); its record made again (252 cases).
