# Spec: `lx.afni.automask` (behaviour of AFNI 3dAutomask 25.2.09)

The input for a clean-room **original** of larmorx's brain-mask tool (to be written in
`crates/larmorx-afni`, Apache-2.0). AFNI's 3dAutomask program file is NIH public domain, but
its results depend on library routines copyrighted by the Medical College of Wisconsin (GPL-2:
the average of the sub-bricks, the clip level's maximum and sub-volumes, the counting; see
`docs/licensing.md`, "Notes", and the licence table of `docs/findings/afni-automask.md`). So
larmorx has, or will have, two implementations (CLAUDE.md rule 1):
- a bit-exact replica of AFNI's code, in the separate `larmorx-gpl` package (done:
  `larmorx-gpl afni 3dAutomask`, `docs/validation/afni-automask.md`);
- the original, in the main package, written from this file, AFNI's documentation and
  black-box runs of the AFNI binary only (`oracles/afni-25.2.09/bin/3dAutomask`, built by
  `scripts/build_afni_oracle.sh --add 3dAutomask`).

**Whoever writes the original must not open** AFNI's source (`tags/AFNI-25.2.09/`, the `afni/`
clone, `oracles/afni-25.2.09/build/`), the replica crates (`crates-gpl/`) or the findings
written from the source (`docs/findings/afni-automask.md`, the sections of
`docs/findings/upstream-defects.md` marked restricted).

This spec describes behaviour: what goes in, what comes out, the maths and the numeric
conventions that decide the bits. It holds no code. Statements marked **(observed)** were
confirmed by running the AFNI binary; the rest come from AFNI's help, documentation and analysis
of the program's behaviour.

**Defects (D13).** Some behaviours below are defects that larmorx corrects by default and
reproduces with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`): AFNI-43
(`-peels 0`, §5.3), AFNI-01 (a negative `scl_slope` in a series, §3), AFNI-40 (an option without
its value, §8.2), and those of AFNI's dataset reading and writing shared with 3dTshift (AFNI-02,
-07, -11, -12, -16, -18, -20, -22, -23, -26; `specs/3dTshift.md` §6, §7.1). Three behaviours were
questionable; the user decided them on 2026-10-23: AFNI-41 refused by default (rule B), AFNI-42
and AFNI-44 kept as AFNI's and documented (rule C) (§5.3, §5.5, §8.1).
§9 lists what the default does for each.

---

## 1. Purpose

From an EPI time series, or an anatomical image that has already been skull-stripped, make a
brain mask: 1 inside, 0 outside, on the input's grid. fMRIPrep uses it on the unifized BOLD
reference: `3dAutomask -apply_prefix uni_xform_masked.nii.gz -dilate 1 -prefix
uni_xform_mask.nii.gz uni_xform.nii.gz` (niworkflows, `init_enhance_and_skullstrip_bold_wf`,
"skullstrip_second_pass").

## 2. Inputs and outputs

- **Input:** a NIfTI dataset, read with AFNI's rules (`specs/3dTshift.md` §6): uint8 and int16
  stay integers (with a brick factor from `scl_slope`), float32 stays float32, the other types
  become float32; non-finite float values become 0. One difference from 3dTshift's reading:
  3dAutomask reads the dataset itself, not a copy, so **a `scl_slope` of exactly 1 with
  `scl_inter` 0 on uint8 or int16 data is kept as a brick factor of 1** (3dTshift's copy has
  none). A factor of 1 scales nothing, but it is copied to `-apply_prefix`'s output, whose
  `scl_slope` is then 1 (observed; 0 for the copy of 3dTshift).
- **Outputs**, each optional:
  - the mask (`-prefix`): uint8, 0 and 1, one volume, no time axis, `scl_slope` 0, the input's
    geometry. With neither `-prefix` nor `-apply_prefix`, AFNI writes it with the prefix
    `automask` in its own format (larmorx refuses: NIfTI only);
  - the masked input (`-apply_prefix`): every sub-brick of the input, in the input's storage
    type and with its brick factors, every voxel outside the mask 0, the input's time axis
    kept. With `-apply_prefix` and no `-prefix`, this is the only output;
  - the depth (`-depth`): int16, one volume: for each mask voxel, how many erosions (§6.3)
    remove it; 0 outside.
- Masks of 2D images (one slice) are computed by another method (§7).

## 3. The image the mask is made from

The **average of absolute values** over the sub-bricks, in float:
- one sub-brick: `|f * x|` for each voxel, where `x` is the stored value as float and `f` the
  brick factor (none when 0 or 1); the product in float, then non-finite results 0;
- several sub-bricks: at each voxel, its series as floats (non-finite values 0), each value
  multiplied by its sub-brick's factor when that factor is **positive** (and only when some
  factor is neither 0 nor 1), then the sum of `|value|` in float in sub-brick order, times
  `c = (float)(1.0 / n)` (the double quotient rounded to float, then a float product).

AFNI-01: a negative factor is dropped in the series case (with its magnitude) but applied in
the one-sub-brick case. The mask hardly changes (§4 scales with the data), but the clip levels
it prints do. The default applies negative factors too; `reproduce` drops them.

## 4. The clip level

`clip(v, frac)` of a float image `v` (the clip level fraction `frac`, default 0.5; values
outside `(0, 0.99)` give 0.5):

1. `top` = the largest value, found by keeping `m = -1e38` and replacing it by each value `x`
   with `m < x`; as a double. If `top < 1e-100`, the level is 0.
2. A histogram `h[0..10000]` (10001 bins): `s = 10000 / top` in double; every **positive** value
   `x` goes to bin `k = trunc(s * x + 0.499)` (double arithmetic, `x` converted to double),
   if `k <= 10000`; also `npos` counts them and `d` (double) accumulates `k * k` (as doubles).
3. If `npos <= 222`, the level is 0.
4. `q = trunc(0.65f * (float)npos)` (a float product); `b = rint(0.5 * sqrt(d / npos))`
   (double; `rint` rounds half to even). Starting with `i = 9999` and `kk = 0`: while `i >= b`
   and `kk < q`: `kk += h[i]`, `i -= 1`. Then `cut = i`. **The bin 10000 (the maximum's) is never
   counted** in this step or the next (observed: two groups of values, 1000 voxels of 100 and
   1000 of 10, give a level of 5).
5. Repeat at most 66 times, stopping when the cut does not change: `n` = the sum of `h[cut ..
   9999]`; `half = n / 2` (integer division); from `i = cut`, `kk = 0`: while `i < 10000` and
   `kk < half`: `kk += h[i]`, `i += 1`; then `cut = trunc(frac * (float)i)` (a float product).
6. The level is `(float)(cut / s)` (double quotient, at most 1e38, rounded to float).

(A bin index below 0 can only arise with more than 100 million positive voxels; larmorx counts
such a bin as empty.)

### 4.1 The gradual clip level (the default; `-nograd` turns it off)

1. The centre of mass of `|v|` in voxel indices: sums in double over the voxels in storage
   order (x fastest) of `|x|`, `|x| i`, `|x| j`, `|x| k`; the three quotients (or, when the sum
   is 0, the grid's centre `(n - 1) / 2`), each rounded to float. `ic`, `jc`, `kc` = `rint` of
   those floats (half to even).
2. `g = 0.333f * clip(v, frac)` (float).
3. The overlap `o = max(1, rint(0.01 n))` per axis (double, half to even). With `it = nx - 1`
   etc., the lower half-range is `[0, ic + o]` and the upper `[ic - o, it]`, clipped to the grid
   (likewise for y and z). The clip level of each of the 8 octant boxes (lower/upper per axis)
   is `clip` of that sub-volume (the voxels of the box, in storage order), raised to `g` when
   below it.
4. The octants' centres: `x0 = (float)(0.5 * ic)`, `x1 = (float)(0.5 * (ic + it))` (double
   products rounded to float); `dx = (x1 > x0) ? (float)(1.0 / (double)(x1 - x0)) : 0` (the
   difference in float, the quotient in double, rounded to float); likewise for y and z.
5. At voxel `(i, j, k)`: `u = ((float)i - x0) * dx` in float, clamped to `[0, 1]`; `1 - u` in
   float; likewise for y and z; the level is the float sum, in this order, of
   `c000 * (1-u) * (1-v) * (1-w)`, `c100 * u * (1-v) * (1-w)`, `c010 * (1-u) * v * (1-w)`,
   `c110 * u * v * (1-w)`, `c001 * (1-u) * (1-v) * w`, `c101 * u * (1-v) * w`,
   `c011 * (1-u) * v * w`, `c111 * u * v * w`, each product evaluated left to right in float
   (`c` with the bits of the x, y, z halves: 0 lower, 1 upper).

## 5. The mask of a 3D image (`nz >= 2`)

The steps below run in this order on the averaged image `v`. Neighbourhoods, unless stated
otherwise, treat a voxel at the grid's edge as having **itself** in place of each missing
neighbour (planar duplication).

### 5.1 Above the clip level

`mask = (v >= level)` voxel by voxel, with the gradual level (§4.1) or, with `-nograd`, the
single level `clip(v, frac)`. If no voxel is set, the empty mask is the result. If `nx < 2` or
`ny < 2`, the thresholded mask is the result.

### 5.2 The largest cluster

Keep only the largest **6-connected** cluster of set voxels (faces). When several clusters
have the largest size, the one whose first voxel comes first in storage order wins.

### 5.3 Peeling and restoring (`-peels`, `-nbhrs`)

With `p` peels (default 1) and threshold `t` (default 17):
- `p` is 1 for any value below 1 (AFNI-43: the help says `-peels 0` turns the peeling off; the
  default does that, `reproduce` peels once) (observed);
- `-nbhrs` outside 6..26: by default a fatal error naming AFNI-41 (exit 1; decided 2026-10-23,
  rule B: the value makes no sense, AFNI silently replaces it); with `reproduce`, `t` is 17 as in
  AFNI (`-nbhrs 40` acts as 17) (observed);
  the threshold used is `min(18, t)`;
- `-NN1`, `-NN2`, `-NN3` set `t` to 6, 18 and 26 (so 18 with the minimum), whatever `-nbhrs`
  says and whichever comes first (AFNI-42, kept and documented, decided 2026-10-23, rule C; note that an explicit `-NN2` gives 18, not the
  default 17) (observed).

Nothing happens when the grid has fewer than 27 voxels. Otherwise, with the 18 neighbours of a
voxel (the 6 faces and 12 edges of its 3 × 3 × 3 cube; edge voxels duplicated as above):
1. For each layer `l = 1 .. p`: every set voxel with fewer than `min(18, t)` set neighbours is
   marked with `l` (counting on the mask as it was at the start of the layer), then all marked
   voxels are cleared.
2. For each layer `l = p, p - 1, ..., 1`: every cleared voxel whose mark is `l` or more and that
   is still clear is restored when it has more than `r` set neighbours, `r = 0` for `l = p` and
   1 otherwise (the counts taken on the mask at the start of the layer).

Then 5.2 again.

### 5.4 Small holes

"Fill in once" with side `s`: with `sx = min((nx - 1) / 2, s)` (integer division) and likewise
`sy`, `sz`, nothing happens if all three are 0. Otherwise, every clear voxel with
`sx <= i < nx - sx`, `sy <= j < ny - sy`, `sz <= k < nz - sz` is filled when, along x, a set
voxel lies within `sx` steps in both directions (and `sx > 0`), or likewise along y or z. The
tests use the mask as it was before the pass; the filled voxels are set together after it.
**Voxels outside those index ranges are never filled, along any axis.**

Fill in once with side 1; if anything was filled, again; if again, a third time.

### 5.5 Large holes

`m = max(1, rint(0.016 nx), rint(0.016 ny), rint(0.016 nz))` (double, half to even). AFNI
runs this step only when `m > 1` **or `rint(0.016 nz) > 0`** (AFNI-44, kept: the test reads the
z term only, so a volume with fewer than 32 slices and no axis of 94 voxels or more skips it,
and one with 32 to 93 slices and no wide axis runs it with `m = 1`) (observed). The step: fill
in once with each side `2 .. m - 1`, then fill in with side `m` repeatedly until a pass fills
nothing.

### 5.6 The final peel, and the outside

1. One peel and restore (§5.3 with `p = 1` and the same threshold), then 5.2.
2. Invert the mask; keep the largest 6-connected cluster of the inverted mask (the outside of
   the head; 5.2's rule); then:
3. With `-eclip`: with `T = (float)(9999.9 * (double)level)` (`level` the single clip level of
   §4, not the gradual one), repeat until a sweep adds nothing: in storage order over the
   interior voxels (`1 <= i < nx - 1`, likewise `j`, `k`), a voxel not in the (outside) mask and
   whose value is below `level` or above `T` joins it when one of its 6 face neighbours is in
   it; **each voxel added counts for the voxels after it in the same sweep** (in place).
4. Invert again: the mask is the brain.
5. With `-eclip`, if step 3 added any voxel: one more peel and restore, then 5.2.

## 6. The program's own steps

### 6.1 `-dilate` and `-erode`

With `d` dilations and `e` erosions (default 0; negative values are refused, §8.2), when `d`
or `e` is not 0: `w = max(1, rint(0.032 nx), rint(0.032 ny), rint(0.032 nz))`;
1. `d` times: a **dilation**: every clear voxel with at least 3 set neighbours of the chosen
   level (`-NN1`: the 6 faces; `-NN2`, the default: faces and edges, 18; `-NN3`: all 26; edge
   voxels duplicated as above) is set (counts on the mask before the pass); then fill in with
   side `w` until nothing is filled (§5.4);
2. `e` times: an **erosion** (§6.3), then the same filling;
3. fill every hole: invert, keep the largest 6-connected cluster, invert.

With both options, AFNI warns "Combining dilate,erode options is like peels option" (at the
option scan).

### 6.2 Counts, empty planes and `-SI`

- `++ <n> voxels in the mask [out of <N>: <p>%]` with `p = 100 n / N` in double, `%.2f`.
  If `n` is 0: the fatal error "No voxels? Quitting without saving mask" (exit 1, nothing
  written).
- For x, then y, then z: `++ first <a> x-planes are zero [from <c>]` and `++ last  <b> x-planes
  are zero [from <c>]` (`%3d`), `a` the number of leading planes without a mask voxel and `b`
  the trailing ones, `c` the first and second letter of the axis's orientation code (`R` and `L`
  for an axis running right to left, `L`/`R`, `P`/`A`, `A`/`P`, `I`/`S`, `S`/`I`; `??` for an
  axis AFNI could not orient).
- `-SI h` (millimetres): on the axis whose orientation's first letter is `S` (the first planes
  are superior), every plane from `a + trunc(h / |step| + 0.5)` to the last is cleared; on an
  axis whose second letter is `S`, every plane from 0 to `L - trunc(h / |step| + 0.5)`, `L` the
  last non-empty plane (`h / |step|` in double, `step` the axis's voxel size with its sign). If
  both ends of different axes qualify, the later test in the order above wins. When anything is
  to clear (the range is not empty): `++ SI clipping mask along axis <x|y|z> from <lo>..<hi>`
  and `++ <n> voxels left [out of <N>]`. The clipping happens after the report and before the
  outputs.

### 6.3 Erosion and `-depth`

An **erosion** of level `nn`: every set voxel on the grid's border is cleared; every other set
voxel is cleared when one of its 6 face neighbours is clear, or (`nn >= 2`) one of its 12 edge
neighbours, or (`nn = 3`) one of its 8 corner neighbours (counts on the mask before the pass).

`-depth`: on a copy of the mask computed in §5 (before `-dilate`, `-erode` and `-SI`): repeat
until it is empty: add 1 to every set voxel's depth; erode (level `-NN`, default 2). (If an
erosion cleared nothing while voxels remained, AFNI would warn "Nothing left to peel" and stop;
it cannot happen, since the border of any mask erodes.)

## 7. The mask of a 2D image (`nz = 1`)

Settings of §4 and §5 other than `-clfrac` play no part.
1. If `nx < 16` or `ny < 16`: no mask (the fatal error "Mask creation fails for unknown
   reasons!", exit 1) (observed for 12 × 12 and 64 × 1 inputs).
2. `w = |v|` (a float copy, non-finite values 0); `mask = (w >= clip(w, frac))`.
3. No voxel set: no mask (the same error). 1 or 2 voxels, or every voxel, set: that mask.
4. Three peels and restores in the plane: the 8 in-plane neighbours (edge pixels duplicated),
   threshold 6 (fewer than 6 set neighbours peels), restored as in §5.3.
5. Keep every **4-connected** cluster with at least `trunc(0.5f * (float)big)` pixels (`big` the
   largest cluster's size; when that is 0 or more than `big`, `big`).
6. Invert; keep the clusters with at least `trunc(0.9f * (float)big)` pixels (the outside, and
   any hole nearly as large); invert.

## 8. The command line (`larmorx afni 3dAutomask [options] dataset`)

### 8.1 Options (before the dataset; scanning stops at the first argument not starting with `-`)

| Option | Effect |
|---|---|
| `-prefix p` | the mask (§2) |
| `-apply_prefix p` | the masked input (§2) |
| `-depth p` | the depth (§6.3) |
| `-clfrac c`, `-mfrac c` | clip level fraction: `c` read as a double (C `strtod`), rounded to float; outside `[0.1, 0.9]` (float comparisons): "-clfrac value %f is illegal!" |
| `-nograd` | the single clip level; **any option starting with `-nogr`** is taken as it (5 characters compared) (observed: `-nogrxyz`) |
| `-peels p`, any option starting with `-peel` | `p` = trunc of `strtod` (§5.3) |
| `-nbhrs n`, any option starting with `-nbhr` | `n` = trunc of `strtod` (§5.3) |
| `-NN1`, `-NN2`, `-NN3` | neighbourhood of §6.1 and §6.3, and the peel threshold (§5.3, AFNI-42) |
| `-eclip` | §5.6 step 3 |
| `-dilate d`, `-erode e` | C `strtol` base 10 (leading blanks, sign, digits; 0 when none), the `long` converted to `int` (low 32 bits); negative: "-dilate %s is illegal!" with the argument as given |
| `-SI h` | `strtod` to float; `h <= 0`: "-SI value %f is illegal!" |
| `-q` | no progress messages (`++`/` + ` lines) |

An unknown option: the fatal error "ILLEGAL option: <arg>". Option names are case-sensitive.
After the options, the dataset; the arguments after it are ignored (observed: a `-prefix` after
the dataset has no effect).

### 8.2 Errors and their messages

All on standard error, exit status 1 (AFNI's fatal-error format, `specs/3dTshift.md` §7.1):
- `-prefix`, `-apply_prefix` or `-depth` as the last argument: "-prefix (null) is illegal!"
  (the missing name is printed as `(null)`) (observed);
- `-peels`, `-nbhrs`, `-clfrac`, `-mfrac`, `-SI`, `-dilate` or `-erode` as the last argument:
  **AFNI crashes** (a segmentation fault; its crash banner, exit 1) (observed). AFNI-40: larmorx
  refuses with one fatal line naming the missing value, in both modes, after the messages AFNI
  prints before the crash;
- no dataset: "Can't open dataset '(null)'"; a dataset that cannot be read: "Can't open dataset
  '<name>'" (observed);
- a name `THD_filename_ok` refuses (blanks, shell metacharacters, non-ASCII; AFNI-12 as in
  3dTshift): "-prefix <name> is illegal!" (likewise for the other names).

### 8.3 Messages of a successful run (on standard error, unless `-q`)

In order (observed on every parity case, byte for byte):
1. (AFNI's version banner, which larmorx does not print;) `++ Authored by: Emperor Zhark` (also
   with `-q`; not with `AFNI_QUIET_STARTUP=YES`);
2. the messages of opening the dataset (`specs/3dTshift.md` §7.1: conversion, obliquity);
3. `++ Loading dataset <name as given>`;
4. `++ Forming automask`, then, from §4 and §5: ` + Fixed clip level = %f`, with the gradual
   level ` + Used gradual clip level = %f .. %f` (the smallest and largest voxel levels),
   ` + Number voxels above clip level = %d`, and for each clustering ` + Clustering voxels ...`
   and ` + Largest cluster has %d voxels`; ` + Filled %5d voxels in small holes; now have %d
   voxels` (when any), ` + Filled %5d voxels in large holes; now have %d voxels` (when any),
   ` + Clustering non-brain voxels ...` before the outside's clustering, ` + Removed  %d exterior
   voxels below clip level` (with `-eclip`, when any), ` + Mask now has %d voxels`;
5. with `-depth`: for each erosion ` + Eroded   %d voxels` and `++ Peeled %d voxels from mask of
   %d (now %d)`;
6. `++ Dilating automask` before the dilations, `++ Eroding automask` before the erosions (with
   ` + Eroded   %d voxels` for each), and the clustering lines of the final hole filling;
7. the count, the planes and `-SI` (§6.2);
8. `++ Output dataset ./<name>` after writing the mask (the name with `./` when it has no
   directory; printed when that file exists, written or not); with `-apply_prefix`: `++ applying
   mask to original data`, `++ Writing masked data`, `++ Output dataset ./<name>`; with `-depth`:
   `++ Writing depth dataset`, `++ Output dataset ./<name>`;
9. `++ CPU time = 0.000000 sec` (AFNI 25.2.09's CPU clock reads 0 on Linux).

`%f` is C's six decimals; `%d` and `%5d` integers.

### 8.4 AFNI's global options and environment variables

As for 3dTshift (`specs/3dTshift.md` §7.1: `-overwrite`, `-Dname=value`, `-ok_1D_text`,
`-skip_afnirc`; the variables of reading, writing, output names and messages), with these
differences: 3dAutomask reads no slice-time file (`AFNI_1D_ZERO_TEXT` has no effect);
`AFNI_QUIET_STARTUP=YES` hides the author line and the `Output dataset` lines (observed). An
existing output follows `AFNI_DECONFLICT` per output (AFNI-11: AFNI writes nothing for that
output, prints "output dataset name ... conflicts with existing file" and "dataset NOT written
to disk!", goes on and exits 0).

## 9. Defects and what larmorx does by default

| Defect | AFNI | larmorx's default | `reproduce` |
|---|---|---|---|
| AFNI-01 | a negative factor dropped when averaging several sub-bricks (§3) | applied; named whenever such a series is averaged, with the number of mask voxels it changes | AFNI's |
| AFNI-40 | an option's value missing: a crash (§8.2) | a fatal error, exit 1 | the same refusal |
| AFNI-41 | `-nbhrs` outside 6..26 silently 17 | a fatal error naming AFNI-41, exit 1 (rule B, decided 2026-10-23) | AFNI's |
| AFNI-42 | `-NN1/2/3` reset the peel threshold to 6/18/26 | AFNI's, documented (rule C, decided 2026-10-23) | AFNI's |
| AFNI-43 | `-peels 0` peels once | no peeling; named when the mask differs | AFNI's |
| AFNI-44 | the large-hole step decided by the z size | AFNI's, documented (rule C, decided 2026-10-23: it shapes fMRIPrep's BOLD masks) | AFNI's |
| AFNI-02, -07, -11, -12, -16, -18, -20, -22, -23, -26 | dataset reading and writing (`specs/3dTshift.md` §6.5, §7.1) | as for 3dTshift; with `AFNI_NIFTI_SCALE=NO` (AFNI-23) the data are read as without the variable (float32, `slope * x + inter`) for the mask and the masked copy | AFNI's |

Every correction is named on standard error as `*+ WARNING: upstream defect <ID> (<tool>)
corrected: ...` (the notice line of `docs/api/upstream-defects.md`).

## 10. Validation

- **Bit-identity** of every output (data and header) with AFNI's binary, exit status and
  messages, on the parity cases of `validation/.../parity/afni_automask.py` (synthetic phantoms
  covering every option, type, geometry and error; fMRIPrep's call on every oracle run's BOLD
  reference; AFNI's default on BOLD series and an anatomical), in `reproduce` mode; the replica
  meets it (`docs/validation/afni-automask.md`). A clean-room original should meet it too: the
  method is integer and comparison arithmetic except the average (§3) and the clip levels (§4),
  whose float and double steps are stated exactly.
- **AFNI's own variability**: none measured (the program is single-threaded and fMRIPrep's
  build gives the same masks; see the record).
- **The defaults** must agree with the replica's on every case where a correction applies.
- Golden cases: one per code path (the replica's, `crates-gpl/larmorx-gpl-cli/tests/golden.rs`).
