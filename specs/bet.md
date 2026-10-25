# Spec: `lx.fsl.bet`, `lx.fsl.betsurf` (behaviour of FSL 6.0.7.17's `bet`, `bet2` and `betsurf`)

The input for a clean-room **original** of larmorx's brain extraction (to be written in the main
package, Apache-2.0). FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx has, or will have, two implementations (CLAUDE.md rule 1, `docs/licensing.md`):
- a bit-exact replica of FSL's code, in the separate `larmorx-nc` package (done:
  `larmorx-nc fsl bet|bet2|betsurf`, `docs/validation/fsl-bet-replica.md`);
- the original, in the main package, written from this file, FSL's documentation, the BET papers
  and black-box runs of the FSL programs only (`~/fsl/bin/bet`, `bet2`, `betsurf`, FSL 6.0.7.17,
  run in a clean environment: FSL never on the `PATH`, `FSLDIR=~/fsl`, `FSLOUTPUTTYPE` set).

**Whoever writes the original must not open** FSL's source (`~/fsl/src/`, `crates-nc/fsl-source/`),
the replica crates (`crates-nc/`), the findings written from the source
(`docs/findings/fsl-bet.md`, the sections of `docs/findings/upstream-defects.md` marked
restricted) or `docs/parts/` (porting guide §1).

This spec describes behaviour: what goes in, what comes out, the maths and the conventions that
decide the results. It holds no code. Statements marked **(observed)** were confirmed by running
FSL's programs; the rest come from FSL's documentation, the papers and the analysis of the
programs' behaviour. Two references describe the method: S. M. Smith, "Fast robust automated brain
extraction", Human Brain Mapping 17(3):143-155, 2002 (BET); M. Jenkinson, M. Pechaud, S. Smith,
"BET2: MR-based estimation of brain, skull and scalp surfaces", OHBM 2005 (betsurf).

**The original is built for speed** (user, 2026-10-19): the behaviour below within the thresholds
of §11, designed to be much faster than FSL (§10 lists where the time goes and what can run in
parallel deterministically). Bit-identity is not required of it; where a convention below "decides
the bits", following it keeps the original closest to FSL.

**Defects (D13).** Some behaviours below are defects that larmorx corrects by default and
reproduces with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`): FSL-40,
-42 to -48, -50, -51, -53 to -56. §9 lists what the default does for each. Where FSL crashes
on what it leaves undefined (FSL-41, -52, and FSL-43's fill far outside the image), the default
refuses with a clear message, and `reproduce` keeps everything FSL defines and fills the rest
deterministically, with a warning (D13's failure cases). Two questionable behaviours are kept as
FSL's (rule C): FSL-49, FSL-57.

---

## 1. Purpose and use

BET finds the brain in a head image: a tessellated sphere is placed inside the head and deformed,
vertex by vertex, onto the brain's surface; the brain is what the surface encloses. It works on
T1-, T2- and proton-density-weighted images and on EPI (one volume), at any voxel size.

fMRIPrep 25.2.6 calls it on the BOLD reference, twice per BOLD run (niworkflows'
`init_enhance_and_skullstrip_bold_wf`, "skullstrip_first_pass", and the `skullstrip_bold_wf` of the
co-registration reference): **`bet <boldref> <boldref>_brain.nii.gz -f 0.20 -m`** with
`FSLOUTPUTTYPE=NIFTI_GZ`; it uses the mask. 29 such calls run in the twelve oracle runs with
BOLD runs, the data panel's included
(`oracles/fmriprep-25.2.6/*/work/**/skullstrip_first_pass/command.txt`).

`betsurf` (normally run by `bet -A` or `-A2`) then finds the inner skull, the outer skull and the
scalp from the brain's surface.

## 2. The three programs

- **`bet2`** is the brain extraction (§4).
- **`bet`** is a shell script (`#!/bin/bash -e`, `LC_ALL=C`) that runs `bet2` with the options it
  is given, or one of its variations (§5): `-R` (bet2 repeated from the brain's centre), `-F` (4D
  data), `-Z` (small field of view in z), and `-S`, `-B`, `-A`, `-A2`, which run FSL programs
  larmorx does not have yet (standard_space_roi, FLIRT, FAST, betsurf after FLIRT): larmorx refuses
  these four, naming what is missing (exit 1, "pending").
- **`betsurf`** finds the skull and scalp surfaces (§6).

All three take FSL's image names: a file name with or without its extension (§3.1).

## 3. Images

### 3.1 Names and formats

- A name is resolved by trying, in order, the name as given and the name with `.nii.gz`, `.nii`,
  `.hdr.gz`, `.hdr`, `.img.gz`, `.img` (`.hdr`/`.img` pairs: the header names the pair), and the
  NIfTI-2 variants; a name ending in one of these extensions is used as is. Images FSL reads:
  NIfTI-1 and NIfTI-2 single files and pairs, gzipped or not, and ANALYZE 7.5 pairs.
- When no file matches, FSL's image layer prints `Image Exception : #63 :: No image files match:
  <name>` and then `Image Exception : #22 :: Failed to read volume <name>` followed by
  `Error : No image files match: <name>` on standard error, and the program aborts on the
  uncaught exception (exit status 134, `terminate called after throwing an instance of
  'std::runtime_error'` and `what():  ...` lines) **(observed)**. larmorx prints the same
  `Image Exception` lines, then one line of its own saying that FSL aborts here, and exits 1.
- Outputs are written in the type `FSLOUTPUTTYPE` names: `NIFTI` (`.nii`), `NIFTI_GZ`
  (`.nii.gz`), `NIFTI_PAIR` (`.hdr`/`.img`), `NIFTI_PAIR_GZ`, `NIFTI2`, `NIFTI2_GZ`,
  `NIFTI2_PAIR`, `NIFTI2_PAIR_GZ`, `ANALYZE`, `ANALYZE_GZ`. Unset, FSL prints `ERROR::
  Environment variable FSLOUTPUTTYPE is not set!` and `Please make sure that the appropriate
  configuration file is sourced by your shell (e.g. by putting it in .profile).` and `e.g. bash or
  sh users add the line ". ${FSLDIR}/etc/fslconf/fsl.sh"` and `e.g. tcsh or csh users add the line
  "source ${FSLDIR}/etc/fslconf/fsl.csh"` and exits 1; an unknown value: `ERROR:: Unrecognised
  value (<value>) of environment variable FSLOUTPUTTYPE` and `Legal values are: NIFTI, NIFTI_PAIR,
  NIFTI_GZ, NIFTI_PAIR_GZ`, exit 1 **(observed)**. These are printed when the first output is
  written, after the computation.
  The Python API always writes NIfTI-1 (`FSLOUTPUTTYPE=NIFTI`) and reads the outputs back.

### 3.2 Reading

- Every stored type is read and converted to 32-bit float; `scl_slope` and `scl_inter` are
  applied (when the slope is neither 0 nor 1, or the intercept is not 0) as `value * slope +
  intercept` in float. Voxel sizes are the absolute values of `pixdim[1..3]`.
- A 4D input given where one volume is expected is truncated to its first volume, with the
  warning on standard error: `Warning: An input intended to be a single 3D volume has multiple
  timepoints. Input will be truncated to first volume, but this functionality is deprecated and
  will be removed in a future release.` **(observed)**.
- **Radiological storage.** FSL's programs work on images whose voxel x axis runs from the
  subject's right to left in the voxel-to-world matrix with a negative determinant ("radiological"
  storage). An input whose matrix (the sform when its code is set, else the qform) has a positive
  determinant is mirrored along x when read (voxel `i` becomes `nx - 1 - i`, and the matrices are
  rewritten to match), and every output is mirrored back when written, so the outputs line up with
  the input voxel for voxel. All of BET's geometry (§4-§6) is in this radiological voxel frame.
- **The algorithm's coordinates** are millimetres along the voxel axes: a voxel `(i, j, k)` is the
  point `(i dx, j dy, k dz)`, where `dx, dy, dz` are the voxel sizes. World matrices play no part
  in the computation (only in the radiological decision and in the outputs' headers). BET's mesh
  file (`-e`) is written in these coordinates.
- Options given in voxels (`bet2 -c`) are voxel indices of the file as stored; they are converted
  to the radiological frame (`i` becomes `nx - 1 - i` when the file was mirrored).

### 3.3 Writing

Each output image has the input's grid and geometry and a fresh FSL header:
- dimensions, voxel sizes (and `pixdim[0]`, the qform's sign), the sform and qform with their
  codes, the intent code and parameters, `cal_min`/`cal_max` (the input's display range, kept; the mask's set to
  1 and 0, **observed**), `aux_file` copied from the input; `xyzt_units` 10 (mm and s);
  `descrip` is FSL's build string, `2203.12-dirty 2024-02-01T16:17:47+00:00` for FSL 6.0.7.17's
  image library; `scl_slope` 1 and `scl_inter` 0; `slice_*` fields and `toffset` 0;
  `vox_offset` 352 for single files without extensions **(observed)**;
- extensions are written only when `FSL_LOAD_NIFTI_EXTENSIONS` is set to a non-zero number
  (then the input's extensions are kept); FSL computes the data offset of two or more extensions
  wrongly (FSL-56, §9);
- the data in the output's own type: the brain and the outline float32, the masks and the skull
  int16 (§4.7).

The header of an image FSL writes can be compared byte for byte with the original's: the replica
does (`docs/validation/fsl-bet-replica.md`); the original should write the same fields.

## 4. `bet2`

### 4.1 Command line

`bet2 <input_fileroot> <output_fileroot> [options]`.

- With no argument: the usage (below) on standard error, exit 1. With one argument: if it
  contains `-h` or `--help` anywhere, the usage and exit 0; else `error: not enough arguments, use
  bet -h for help` and exit 255. If the first or the second argument starts with `-`: `error : two
  first arguments should be input and output names, see bet -h for help`, exit 255.
- The options follow the two names. FSL's option parser: a word starting with `--` is a long
  option whose value follows `=` in the same word (`--parameter_d1=5`; `--parameter_d1 5` is
  `Missing non-optional argument`); any other word starting with `-` is a run of one-letter
  options (`-mn` is `-m -n`), each taking its values from the following words; numbers are read
  as C's `strtod` reads them and then rounded to float (`-f`, `-g`, `-r`, `-w`, `-c`), and must
  use the whole word; an option given twice keeps the last value. A word that does not start with
  `-` is `<word>:  is an unrecognised token!`. An unknown option is `<option>: Option doesn't
  exist!`; a missing value `<option>: Missing non-optional argument!`; a value that does not read
  `<option>: Couldn't set_value! valstr="<value> <the next words…>"!` (the values the option takes,
  joined by blanks). Each of these errors prints the usage, an empty line and the message on
  standard error, exit 1 **(observed)**.
- After parsing: `-f` outside [0, 1]: an empty line and `Fractional intensity threshold must be
  between 0 and 1!`, exit 1; `-g` outside [-1, 1]: an empty line and `Vertical gradient must be
  between -1 and 1!`, exit 1 (NaN passes both checks: FSL-42). Then `-h` prints the usage and
  exits 0.
- Names: bet2 removes the first `.hdr` from each name wherever it stands, and a name containing
  `.img` loses its first `.hdr` again (FSL-40, §9; a name with `.img` and no `.hdr` aborts). The
  output name `default__default` means `<input>_brain`.

The usage (standard error), exactly:

```

Part of FSL (ID: "")
BET (Brain Extraction Tool) v2.1 - FMRIB Analysis Group, Oxford

Usage: 
bet2 <input_fileroot> <output_fileroot> [options]

Optional arguments (You may optionally specify one or more of):
	-o,--outline	generate brain surface outline overlaid onto original image
	-m,--mask	generate binary brain mask
	-s,--skull	generate approximate skull image
	-n,--nooutput	don't generate segmented brain image output
	-f <f>		fractional intensity threshold (0->1); default=0.5; smaller values give larger brain outline estimates
	-g <g>		vertical gradient in fractional intensity threshold (-1->1); default=0; positive values give larger brain outline at bottom, smaller at top
	-r,--radius <r>	head radius (mm not voxels); initial surface sphere is set to half of this
	-w,--smooth <r>	smoothness factor; default=1; values smaller than 1 produce more detailed brain surface, values larger than one produce smoother, less detailed surface
	-c <x y z>	centre-of-gravity (voxels not mm) of initial mesh surface.
	-t,--threshold	-apply thresholding to segmented brain image and mask
	--ct	-treat input as CT image (default is MRI)
	-e,--mesh	generates brain surface as mesh in vtk format
	-v,--verbose	switch on diagnostic messages
	-h,--help	displays this help, then exits


```

(two hidden options, `--parameter_d1=<d1>` (default 7) and `--parameter_d2=<d2>` (default 3), are
the search depths of §4.4; they are not listed). **(observed)**

Defaults: `-f 0.5`, `-g 0`, `-r` 0 (estimated), `-w 1`, `-c` unset (the centre of gravity), no
outputs but the brain.

### 4.2 The robust range

`t2` and `t98`, the 2nd and 98th percentiles of the image's intensities, are found from
histograms, iteratively (every voxel counts, in float; the arithmetic in double):
1. `lo, hi` = the image's minimum and maximum. A histogram of 1000 bins over `[lo, hi]`: a value
   `x` goes to bin `trunc(1000 x / (hi - lo) + 1000 (-lo) / (hi - lo))`, clamped to `[0, 999]`
   (values outside the range go to the end bins); `n` = the number of voxels.
2. From the bottom, add bins until the count reaches `n / 50` (integer division): the last bin
   added is `b`; `t2 = lo + b (hi - lo) / 1000` (the product rounded to float, added in float).
   From the top likewise, bin `u`; `t98 = lo + (u + 1) (hi - lo) / 1000`.
3. While `t98 - t2 < (hi - lo) / 10` (the range is dominated by a few voxels), repeat with the
   range narrowed to the bins found, widened by one bin on each side (`lo' = lo + b' (hi - lo) /
   1000` with `b' = max(b - 1, 0)`, `hi' = lo + (u' + 1) (hi - lo) / 1000` with
   `u' = min(u + 1, 999)`), at most 10 passes; the tenth pass uses the full range again and leaves
   out its lowest and highest bins (the counts start one bin in from each end).
4. If `hi == lo`, `t2 = lo` and `t98 = hi`.

The verbose line (§4.9) prints these values.

### 4.3 Initial parameters

- **The threshold** `t = t2 + 0.1 (t98 - t2)`.
- **The radius** (unless `-r` gives it): `N` = the number of voxels above `t`;
  `r = (0.75 N dx dy dz / π)^(1/3)` mm, the radius of a sphere of that volume. If the field of
  view along an axis is smaller than `r` (`n_x dx < r`, and so on), the volume is treated as a slab
  along that axis: `r = (N dy dz / (π n_x))^(1/2)` (for x; likewise for y and z), and when this
  happens for two axes the warning `Warning: Image appears to be 1D` is printed on standard error.
- **The centre of gravity** (unless `-c` gives it): over the voxels with `v - t2 > t - t2`, the
  weights `w = min(v - t2, t98 - t2)`; `c = Σ w (i dx, j dy, k dz) / Σ w` mm. With `-c x y z` the
  centre is `(x dx, y dy, z dz)` in the radiological frame (§3.2).
- **The median within-brain intensity** `tm`: of the voxels with `t2 < v < t98` whose point is
  within the radius `r` of the centre (squared distance `< r²`), in storage order, `tm` is the
  largest of the `floor(N_m / 2)` smallest values (the lower median); with no such voxel FSL reads
  an empty list and crashes (FSL-41): larmorx refuses, and in `reproduce` takes `tm = 0` and goes
  on, with a warning.
- **The fractional threshold** is used as `b_t = f^0.275`.

### 4.4 The initial surface and its deformation

The surface is an icosahedron subdivided 4 times: 2562 vertices, 5120 triangles. The icosahedron's
12 vertices are `(±τ, ±o, 0)`, `(±o, 0, ±τ)`, `(0, ±τ, ±o)` with `τ = 0.8506508084` and
`o = 0.5257311121`; each subdivision splits every triangle into four at its edges' midpoints and
then projects every vertex onto the unit sphere. The triangles are oriented so that their normals
point outwards. The sphere is scaled to radius `r / 2` and centred on `c`.

1000 iterations then move every vertex at once (each vertex's new position is computed from the
positions of the previous iteration). For each vertex `p`:
- `n` = its normal: the normalised sum of the normals (cross products, not normalised) of its
  triangles; `m` = the mean of its neighbours' positions; `s = m - p`; the normal part
  `s_n = (s · n) n`, the tangential part `s_t = s - s_n`.
- `l` = the mean, over all vertices, of the mean distance from a vertex to its neighbours,
  recomputed at iterations 0, 50, 100, 200, 300, … (`i == 50` or `i % 100 == 0`).
- **Tangential term** `u1 = s_t / 2`.
- **Smoothness term** `u2 = f2 s_n`, with the local inverse radius of curvature
  `1/R = 2 |s_n · n| / l²` and `f2 = (1 + tanh(F (1/R - E))) / 2`, where `E = (1/rmin + 1/rmax)/2`,
  `F = 6 / (1/rmin - 1/rmax)`, `rmin = 3.33 w`, `rmax = 10 w` (mm; `w` is `-w`). In a rerun (§4.5)
  of pass `P ≥ 1`, where `s · n > 0` (concave), `f2` is multiplied by the pass's factor and capped
  at 1.
- **Intensity term** `u3 = f3 n`, with `f3` from the intensities inward along `-n`:
  - the voxel grid's spacing used for sampling is `(dx, dy, dz)`, except that an axis whose field
    of view is smaller than `r` uses `2 r` instead; `ds = min(1, the smallest of the three)`;
  - `Imin = tm`, `Imax = t`. The point `q = p - n` (1 mm inside) is converted to voxel indices by
    `trunc(q / d + 0.5)` per axis (`d` the spacing above, `trunc` toward zero). If that voxel is
    inside the image: its value `I` gives `Imin = min(Imin, I)`, `Imax = max(Imax, I)`; then the
    voxel `trunc(q/d + 0.5 - (d1 - 1) n/d)` (the deepest point, `d1 - 1` mm further in; `d1 = 7`
    by default) must be inside the image too, else `f3 = 0`; its value lowers `Imin`. Then, for
    `k = 1, 2, ...` while `g = 2 + (k - 1) ds` stays below `d1`, the voxel
    `trunc(q/d + 0.5 - k ds n/d)` (the position is accumulated, one step of `ds n/d` at a time)
    lowers `Imin`, and raises `Imax` while `g < d2` (`d2 = 3`): with 1 mm voxels, the samples at
    2 to 6 mm inside `p`, `Imax` from the one at 2 mm;
  - `Imin = max(t2, Imin)`, `Imax = min(tm, Imax)`;
  - the local threshold `tl = (Imax - t2) b + t2`, where `b = b_t`, or with `-g G`:
    `b = clamp(b_t + G (p_z - c_z) / r, 0, 1)` (larger `G` grows the brain at the bottom);
  - `f3 = 2 (Imin - tl) / (Imax - t2)` when `Imax - t2 > 0`, else `2 (Imin - tl)`;
  - `u3 = 0.5 · 0.1 · l · f3 · n`.
  With the point `q` outside the image, `f3 = 0`.
- The new position is `p + u1 + u2 + u3`.

Notes that decide the bits: the sampling truncates (toward zero) rather than rounding, and the
inward samples advance from the previous one (an accumulated position) rather than being computed
afresh; the sums are in double.

### 4.5 Self-intersection and reruns

After the 1000 iterations, the surface's self-intersection is measured against the initial
sphere: with `ml` and `mlo` the mean neighbour distances of the deformed and the initial surfaces,
the sum over ordered pairs of distinct, non-adjacent vertices closer than `ml` of
`(d / ml - d_o / mlo)²` (`d`, `d_o` the pair's distances on each surface). A total above 4000
counts as self-intersecting. With `-e`, the test is instead an exact one: whether any two
non-adjacent triangles intersect.

When the surface self-intersects, it is deformed again from the initial sphere, pass `P = 1, 2, …`,
each with the factor `10^(P+1)` for the smoothness term (§4.4), lowered linearly over the last
quarter of the iterations: for iteration `i > 750`, `4 (1 - i/1000) (10^(P+1) - 1) + 1`. After
pass 10 the result is kept whatever its self-intersection.

### 4.6 The brain mask

The mask is what the surface encloses: every voxel 1, the surface drawn as 0 (each triangle
rasterised: points along one edge every `min(dx, dy, dz)/2` mm, and from each of them to the
opposite vertex at the same spacing, each point to the voxel `floor(p/d + 0.5)` when inside the
image), then a flood fill of 0 through 6-connected voxels of value 1 from the voxel holding the
centroid of the vertices (`trunc` of the centroid in voxels); the mask is `1 - that`. A centroid
outside the image (a single slice: the surface's centroid falls below it) makes FSL write and
read outside the volume (FSL-43): the default starts the fill at the nearest voxel inside the
image, which on a single slice gives FSL's mask **(observed)**; a centroid that is not a number
(the surface diverged) is an error.

### 4.7 Outputs

- **brain** (`<out>`, unless `-n`): the input (as float, after `--ct`'s conversion is undone: the
  original CT values) times the mask; float32.
- **mask** (`<out>_mask`, `-m`): int16, 1 inside; its display range 1 and 0.
- **`-t`**: voxels below `t` are removed from the mask (and so from the brain) as well.
- **outline** (`<out>_overlay`, `-o`): the input with the surface drawn at the input's maximum
  (rasterised as above, but each point to the voxel `trunc(p/d + 0.5)`); float32.
- **mesh** (`<out>_mesh.vtk`, `-e`): the surface as VTK ASCII polydata, exactly:
  `# vtk DataFile Version 3.0`, `surface file`, `ASCII`, `DATASET POLYDATA`,
  `POINTS <n>  float` (two blanks), one line `x y z` per vertex in fixed notation with 6 decimals,
  `POLYGONS <m> <4m>`, one line `3 a b c ` (a trailing blank) per triangle; no point data. When
  the file cannot be written: `error opening file <name>` on standard error, and the run goes on.
- **skull** (`<out>_skull`, `-s`): int16, 100 at the skull's estimated outer edge and 0 elsewhere:
  for each surface voxel near each vertex (as in §6.4's choice of voxels), a profile outward along
  the normal over 30 mm sampled every `min(d)/2` (trilinear interpolation), the position of its
  maximum `d_max` (when above `t`), the point before it where `d/30 - I(d)/(t98 - t2)` is largest
  (from -3 mm), and from there the position of the steepest rise towards `d_max`: the skull voxel
  (`trunc` of the point).

### 4.8 CT images (`--ct`)

The input in Hounsfield units is first converted to Cormack units (C. Rorden et al., NeuroImage
2012): values below -1024 set to -1024; `v ← v + 1024`; `e1 = max(v - 900, 0)`;
`e9 = min(e1, 200) · 9`; `v ← v + e1 + e9`; values above 3000 then set to 100. BET runs on the
converted image; the brain output carries the original CT values.

### 4.9 Messages (`-v`)

On standard output, C++'s default formatting (`%g` with 6 significant digits):
- `min <min> thresh2 <t2> thresh <t> thresh98 <t98> max <max>`;
- `c-of-g <cx> <cy> <cz> mm`;
- `radius <r> mm`;
- `median within-brain intensity <tm>`;
- after each pass: `self-intersection total <total> (threshold=4000.0) ` (a trailing blank; not
  with `-e`); with `-e`, when it self-intersects: `the mesh is self-intersecting ` after the first
  pass, `the mesh is self-intersecting` after a rerun;
- before each rerun: `thus will rerun with higher smoothness constraint`.

## 5. `bet`

### 5.1 Command line

`bet <input> <output> [options]`. Without a second argument (or an empty one) the usage (below)
on **standard output**, exit 1. Both names lose their image extension (fslpy's `remove_ext`: the
first of `.nii.gz`, `.nii`, `.hdr.gz`, `.hdr`, `.img.gz`, `.img`, `.mnc`, `.mnc.gz` that ends the
name). If no image of the input's name exists: an empty line, `Error: input image <input> not
valid`, an empty line, exit 1 (standard output).

The options, in any order: `-o -m -s -n -t -e` are passed to bet2; `-f <f>`, `-g <g>`, `-r <r>`
and `-c <x> <y> <z>` too (a missing value prints the usage); `-R`, `-S`, `-Z`, `-F`, `-A`, `-B`,
`-A2 <T2>` choose the variation (the last one given); `-v` sets verbose and passes `-v` to bet2;
`-d` keeps the temporary files; anything else prints the usage. With `-v`, before running, on
standard output: `IN=<input>`, `OUT=<output>`, `bet2opts=<the options passed, each preceded by a
blank>`, `verbose=1`, `debug=<0|1>`, `variation=<0..7>`.

When a program the script runs fails, the script prints `<the script's path> failed during
command:<the script's arguments, as words joined by blanks>` on standard output and exits 1 (bash's
`ERR` trap; the replica prints `bet` for its own path). The script expands names unquoted
(FSL-44). It ends by removing every file whose name starts with `<output>_tmp` unless `-d`
(FSL-46).

The usage, exactly (standard output):

```

Usage:    bet <input> <output> [options]

Main bet2 options:
  -o          generate brain surface outline overlaid onto original image
  -m          generate binary brain mask
  -s          generate approximate skull image
  -n          don't generate segmented brain image output
  -f <f>      fractional intensity threshold (0->1); default=0.5; smaller values give larger brain outline estimates
  -g <g>      vertical gradient in fractional intensity threshold (-1->1); default=0; positive values give larger brain outline at bottom, smaller at top
  -r <r>      head radius (mm not voxels); initial surface sphere is set to half of this
  -c <x y z>  centre-of-gravity (voxels not mm) of initial mesh surface.
  -t          apply thresholding to segmented brain image and mask
  -e          generates brain surface as mesh in .vtk format

Variations on default bet2 functionality (mutually exclusive options):
  (default)   just run bet2
  -R          robust brain centre estimation (iterates BET several times)
  -S          eye & optic nerve cleanup (can be useful in SIENA - disables -o option)
  -B          bias field & neck cleanup (can be useful in SIENA)
  -Z          improve BET if FOV is very small in Z (by temporarily padding end slices)
  -F          apply to 4D FMRI data (uses -f 0.3 and dilates brain mask slightly)
  -A          run bet2 and then betsurf to get additional skull and scalp surfaces (includes registrations)
  -A2 <T2>    as with -A, when also feeding in non-brain-extracted T2 (includes registrations)

Miscellaneous options:
  -v          verbose (switch on diagnostic messages)
  -h          display this help, then exits
  -d          debug (don't delete temporary intermediate images)

```

### 5.2 The variations

- **default**: bet2 with the options.
- **`-R`** (robust centre): `-n` is removed from the options (the brain is always written); bet2
  runs; then, up to 10 more times, bet2 runs again with `-c x y z`, the centre of gravity in voxels
  of the previous brain (the mean voxel coordinates of the voxels above 0.001, weighted by their
  value, as stored, printed with 6 decimals and read back), until the centre moves by less than 1
  voxel: the squared distance computed with `dc` at scale 0 (truncated to an integer) is below 1.
- **`-F`** (4D data): the first volume (`<out>_tmp`) is extracted with bet2 `-m -n -f 0.3` and the
  user's options (a later `-f` overrides); its mask is dilated by one voxel (the maximum over the
  3 x 3 x 3 neighbourhood, of the non-zero voxels: `fslmaths -dilF`) into `<out>_mask`, and every
  volume of the input is multiplied by it into `<out>` (float, as `fslmaths -mas` writes).
- **`-Z`** (small field of view in z): `npad = (100 / dz - nz) / 2 + 1` in `dc`'s arithmetic
  at scale 0 (each division truncated toward zero to an integer; `dz` as `fslval` prints it, the
  sign removed); if `npad ≤ 0`, the message `Image already has a greater Z FOV than 100.0 so not
  using padding` on standard output and plain bet2; else the bottom slice is repeated `npad` times below
  and the top slice `npad` times above (`fslroi`, `fslmerge -z`), bet2 runs on the padded image,
  and each output whose name starts with `<out>` is cropped back to the original slices. The crop
  finds the outputs with `imglob`, which FSL's script calls without `${FSLDIR}/bin`: with FSL's
  programs off the `PATH` it fails (`<script>: line 329: imglob: command not found`) and the
  outputs stay padded (FSL-45).
- **`-S`, `-B`, `-A`, `-A2`**: pending in larmorx (they need standard_space_roi, FLIRT, FAST):
  refused with exit 1 and a message naming the missing program.

## 6. `betsurf`

### 6.1 Command line

`betsurf [options] <t1> <t2> <bet_mesh.vtk> <t1_to_standard.mat> <output>` and
`betsurf --t1only [options] <t1> <bet_mesh.vtk> <t1_to_standard.mat> <output>`.

Options: `-h,--help`, `-v,--verbose`, `-1,--t1only`, `-o,--outline` (the surfaces drawn as images),
`-m,--mask` (their masks), `-s,--skullmask` (the skull's mask), `-p,--increased_precision <int>`
(retessellate the surfaces that many times for 100 more iterations each). The option parser is
bet2's (§4.1), except that the first word not starting with `-` ends the options. Errors print the
usage, an empty line and the message on standard error and exit 1. `-h` prints the usage and
exits 0. A word starting with `-` among the remaining arguments prints the usage and exits 255.
Too few arguments: `too few arguments` and the usage, exit 255 (exit 0 with `--t1only`: FSL-54).
Extra arguments are ignored.

The usage (standard error), exactly:

```

Part of FSL (ID: "")
BETSURF (BET Surface Finder) v2.1 - FMRIB Analysis Group, Oxford

Usage: 
 betsurf          [options] <t1> <t2> <bet_mesh.vtk> <t1_to_standard.mat> <output>
 betsurf --t1only [options] <t1>      <bet_mesh.vtk> <t1_to_standard.mat> <output>

Optional arguments (You may optionally specify one or more of):
	-h,--help	displays this help, then exits
	-v,--verbose	switch on diagnostic messages
	-1,--t1only	extraction with t1 only
	-o,--outline	generates all surface outlines
	-m,--mask	generates binary masks from the meshes
	-s,--skullmask	generates skull binary mask
	-p,--increased_precision <int>	retessellates the meshes the indicated number of times


```

### 6.2 Inputs

- **The mesh**: read in the order of the steps below; a file is VTK when its first line contains
  `# vtk DataFile Version 3.0` (three more lines are skipped, then `POINTS <n> <type>`, `n`
  points, `POLYGONS <m> <size>`, `m` faces `3 a b c`), OFF when it contains `OFF` (an optional
  dimension when the line contains `n`, which must be 3; then `n m e`, points, faces `k a b c`),
  else FreeSurfer's ASCII surface (a header line, `n m`, points `x y z value`, faces
  `a b c value`). The name `manual_input` asks for a file name on standard input (the prompt
  `loading mesh : enter file name / c to cancel: ` on standard output); `c` cancels. A file that
  cannot be opened: `error opening file`, `could not open the following file: <name>`,
  `terminating.` on standard error, exit 255. A mesh FSL cannot read (numbers that do not read,
  faces naming missing vertices, an empty mesh) makes FSL compute with memory it never set or
  crash (FSL-52): larmorx refuses; in `reproduce` it keeps the values C++ defines (a number that
  does not read is stored as 0), takes 0 for every value the stream never sets, leaves out the
  faces that name missing vertices, and warns. An empty mesh's centre is not a number: its mask's
  fill is skipped (FSL-43).
- **The matrix**: 16 numbers (FLIRT's 4 x 4 matrix, row by row, from the T1's FSL coordinates in
  mm to MNI152's); only the first 12 are used. A file that cannot be opened: `unable to open
  <name>` on standard error, exit 255 (0 with `--t1only`). Fewer than 12 readable numbers:
  FSL-53.
- **The images**: the T1 and the T2 (registered to the T1, on its grid: FSL-48), read as in §3.2.
  The order: mesh, matrix, T1, T2.

### 6.3 The robust maximum

The brain mask of the input mesh is made on the T1's grid (§4.6's rasterisation and fill, but
drawing the surface as 1 into an image of 0s and filling with 1 from the centroid: the mask
includes the surface voxels). `thr = 1.1 ×` the value of rank `ceil(0.98 N)` (the
`ceil(0.98 N)`-th smallest) of the T1's values in the mask (`N` voxels); T1 values above `thr` are
set to `thr` (as float). A NaN among them makes FSL's selection undefined (FSL-55).

### 6.4 The profiles

The working grid is the T1's, padded so that the mesh stays 3 voxels from every face: per axis,
`below = max(0, ceil(3 - min_vertex / d))` and `above = max(0, ceil(3 - n + max_vertex / d))`
voxels, each plus 2 (plus 1 with `--t1only`); three counting images on that grid start at 0 (inner
skull, outer skull, scalp).

The input mesh is drawn as 1 on the T1's grid. For each vertex `p`, in order: `n` = its normal;
`h = ceil(max distance to its neighbours / 2)` mm; every voxel `v` within `h` of `p` per axis (from
`floor(p/d - h/d)` to `floor(p/d + h/d)`) that is on the drawn surface and no closer to any of the
vertex's neighbours than to `p` (in mm: the nearest neighbour's distance `≥` `p`'s; not for NaN)
casts one profile, from the voxel along `n` (in voxels: `n/d`), sampled by trilinear interpolation
of the T1 (and the T2) every 0.5 mm: positions `-30 ≤ s < 60` (T1 and T2) or `-3 ≤ s < 60`
(`--t1only`), and up to 100 mm for the profiles that pass near the eyes. A profile passes near the
eyes when, for any of the points `v + k n` mm (`k = 0, 5, 10`), mapped by the matrix and
translated by `(-88, -128, -74)`, `0.0117213 y - 0.021433355 z - 0.6 > 0`, unless one of those
points has `y > 62` and `|x| < 16` (the nose region).

On a profile, the searches below use thresholds between the profile's minimum and maximum over a
region `[L, R)` of its samples: `th(a) = min + a (max - min)`. "The first sample at or after `s`
not above `th`": scanning forward from the first sample at or after position `s`, within the
region's end; "the last sample before `s` not below `th`": scanning backward from the last sample
at or before `s`, down to the region's start. A search that finds nothing gives -500.

**T1 and T2, ordinary profiles** (positions -30 to 60):
1. Regions start at -3 mm on both profiles.
2. The scalp `S`: the last T1 sample before the end not below `th1(0.2)`; `S'` the last one before
   `S - 1.5`; if `S - S' > 2`, `S = S'`. The T2's scalp `S2`: the last T2 sample before its end not
   below `th2(0.25)`.
3. If `|S - S2| > 5` (an artefact on one of them): when `S ≥ S2`, the T1's last sample not below
   `th1(0.2)` before `min(S, S2) + 15` replaces `S` if it lies within 10 mm of `S2`, else the
   profile is dropped; when `S < S2`, the T2's search from `min + 15` must lie within 10 mm of `S`,
   else the profile is dropped, and `S` stays (FSL-49).
4. Both regions end at `S`. The inner skull `IS`: the first T1 sample from the T2 region's start
   not above `th1(0.25)`; the first T2 sample from `IS` not above `th2(0.30)`, if later, replaces
   it. The T2's region starts at `IS`.
5. The gaps: from `IS`, alternately the next T2 sample not below `th2(0.40)` and the next not above
   `th2(0.30)`, up to 6 of the latter (stopping at -500); if more than one gap was found and the
   first bright sample is within 10 mm of `IS`, the outer skull search starts at the first gap,
   else at `IS + 0.5`. The T1's region starts there.
6. The outer skull from the scalp side `OS2`: the last T1 sample before `S` not below `th1(0.75)`,
   then the last before that not above `th1(0.30)` (the search start if none).
7. The outer skull from the brain side: `a` = the first T1 sample from the region's start not
   above `th1(0.30)` (none: FSL stops the program, FSL-50); scanning the samples from `a` (not the
   first), stopping at the first above `th1(0.25)`, `b` = the last one below `th1(0.15)` (else
   `a`); `OS` = the first T1 sample from `b` not below `th1(0.15)`.
8. The profile is kept, with (scalp `S`, outer skull `OS`, inner skull `IS`), when
   `OS2 - OS < 3`, `OS - OS2 < 2`, `S - OS ≥ 2` and `OS ≥ IS`; else only its scalp counts.

**T1 and T2, near the eyes** (positions -30 to 100): `IS` from the T1 (`th1(0.25)` from the
region's start at -3 mm), later T2 (`th2(0.30)`) replacing it, 0 when beyond 15 mm; the scalp `S`
as in 2 (the profile is dropped when `S - S' > 2`); the T1 region ends at `S`; `w` = the first T1
sample from `IS` not above `th1(0.25)`; `OS` = the first from `w` not below `th1(0.35)`, but
`IS + 0.5` when `w - IS ≥ 7` or `OS < IS`. Kept as (S, OS, IS).

**T1 only, ordinary profiles** (positions -3 to 60): the scalp `S` (as in 2, `S = S'` when the
check fails); the region ends at `S`; `IS` = the first sample from the start not above `th(0.25)`;
the region starts at `IS`; `OS2` = the last sample before `S` not below `th(0.75)`, then the last
before it not above `th(0.20)`; `a` = the first sample from the region's start not above
`th(0.30)` (none: dropped), at least -3; scanning from `a` with a counter (up on samples above
`th(0.15)`, down on samples below, never below 0), stopping at a sample above `th(0.60)` or when
the counter reaches 12, `b` = the last sample below `th(0.15)`; `OS` = the first from `b` not below
`th(0.15)`. Kept, as (IS, OS2, S), when `-2 ≤ OS2 - OS < 2` and `S - OS2 ≥ 2`; else only the
scalp counts.

**T1 only, near the eyes** (positions -3 to 100): `S` as in 2 (no check), the region ends at `S`,
`IS` = the first sample from -3 not above `th(0.25)` (0 when beyond 5), `OS` = the first from `IS`
not below `th(0.35)`; kept as (IS, OS, S) always.

Each kept estimate `e` adds 1 to its counting image at the voxel `floor(v + e n/d + 0.5)` plus the
padding (points outside the grid are dropped). Profiles whose searches read past their last
sample are FSL-51.

With `-v`, on standard output: `extracting profiles` before; ` nb of profiles : <N>` and
` removed profiles : <100 × dropped / N>%` after (C++'s `%g`; `-nan` when `N` is 0).

### 6.5 The surfaces

The inner skull and scalp counting images are smoothed by a separable Gaussian of σ = 3 mm (per
axis σ in voxels `3/d`, the kernel of half-width `2 trunc(σ - 0.001) + 3` voxels, weights
`exp(-j²/(2σ²))` normalised to sum 1; outside the grid counts as 0) (`blurring`). Then, with
`E` and `F` as in §4.4 for `w = 1` (`rmin = 3.33`, `rmax = 10`), each deformation step moves every
vertex at once by `u1 + u2 + u3`: `u1` and `u2` as in §4.4 (no rerun factor; `u2` uses the
smoothness factor 1), and:
- every 100 iterations (from 0) each vertex is marked "outside" when, sampling the counting image
  along its normal from `p - n` every 2 mm up to 149 mm, no value exceeds 0.1 and no rise between
  successive samples exceeds 0.001 (`100 ×` the rise `> 0.1`);
- for a vertex not marked: `g = I(p + n) - I(p - n)` (trilinear, in the working grid);
  `f3 = clamp(100 g, -1, 1)`, but `f3 = v_s` (the step's speed) when `0 ≤ 100 g < 0.1` and
  `I(p + n) < 0.1`; when a mask of the previous surface is given and its trilinear value `m` at
  `p` is positive, `f3 = max(m/2, 0.1, f3)`;
- for a marked vertex: `f3 = max(m/2, 0.1)` where `m > 0`, else 0;
- `u3 = 0.05 f3 n`;
- `l` is recomputed every 50 iterations.

(FSL also sets the smoothness weight to 0 where the mask applies, after the smoothness term has
been computed, so it has no effect: FSL-57, kept.)

The phases (`computation`; each vertex's mark starts as "not outside"):
1. **Inner skull** (` inner skull`): the input mesh, translated into the working grid, 800 steps on
   the smoothed inner-skull image, speed 0.5, no mask. With `-p k`: a copy retessellated `k` times
   (each triangle into four at its edges' midpoints), 100 more steps (`  increased precision`).
   The result (translated back) is `<out>_inskull_mesh.vtk`.
2. The inner skull's mask on the working grid: the smoothed inner-skull image converted to integer
   (truncated: values of 1 or more stay as barriers), the surface drawn as 1, filled with 1 from
   the centroid (§6.3's fill). The outer-skull counting image is set to 0 where this mask is 1,
   then smoothed.
3. **Outer skull** (` outer skull`): the same mesh, 800 more steps on the smoothed outer-skull
   image, speed 0.5, with the inner skull's mask; `-p` as above (no mask). `<out>_outskull_mesh.vtk`.
4. The outer skull's mask on the working grid: as in 2, but from the smoothed **inner-skull** image
   and the outer skull's surface.
5. **Scalp** (` skin`): a new icosahedron (2562 vertices) centred on the outer skull mesh's centroid
   with radius 0.75 × its mean vertex distance from the centroid, 1500 steps on the smoothed scalp
   image, speed 1.5, with the outer skull's mask; `-p`: the mesh itself retessellated, 100 steps.
   `<out>_outskin_mesh.vtk`.

### 6.6 Outputs

On the T1's grid, int16 (§3.3):
- `-o`: `<out>_inskull_mesh`, `<out>_outskull_mesh`, `<out>_outskin_mesh`: each surface drawn as 1
  (`  outline`);
- `-m`: `<out>_inskull_mask`, `<out>_outskull_mask`, `<out>_outskin_mask`: each surface's mask
  (drawn as 1 and filled with 1 from its centroid) (`  mask`); with `--t1only` FSL fills the scalp
  mask from the outer skull surface's centroid in the working grid (FSL-47);
- `-s`: `<out>_skull_mask` (` skull mask`): `min(1, outer mask × (1 - inner mask) + edge)`, where
  the edge is the inner skull's drawn surface (T1 and T2) or the outer skull's (`--t1only`); the
  inner and outer masks are made with `-s` even without `-m`.
The three VTK meshes are always written (§4.7's format). The order of the verbose lines follows
the phases above.

## 7. Messages and exit statuses (summary)

| Program | Case | Output | Exit |
|---|---|---|---|
| bet2 | no argument | usage (stderr) | 1 |
| bet2 | one argument with `-h` | usage | 0 |
| bet2 | one argument | `error: not enough arguments, use bet -h for help` | 255 |
| bet2 | a name starting with `-` | `error : two first arguments should be input and output names, see bet -h for help` | 255 |
| bet2 | option errors | usage, empty line, the parser's message | 1 |
| bet2 | `-f`, `-g` out of range | empty line, the message | 1 |
| bet2 | input not found | `Image Exception` lines; FSL aborts (134); larmorx exits 1 | 1 |
| bet2 | an output with `.img` and no `.hdr` | FSL aborts (std::out_of_range); larmorx exits 1 | 1 |
| bet2 | a constant image (no voxel for `tm`) | FSL crashes; larmorx refuses (FSL-41); `reproduce` goes on with `tm = 0` | 1 (`reproduce`: 0) |
| bet | no output name | usage (stdout) | 1 |
| bet | input not an image | empty line, `Error: input image <in> not valid`, empty line | 1 |
| bet | a failing step | `<path> failed during command:<args>` (stdout) | 1 |
| bet | `-S`, `-B`, `-A`, `-A2` | larmorx: pending | 1 |
| betsurf | too few arguments | `too few arguments`, usage | 255 (`--t1only`: 0) |
| betsurf | matrix not found | `unable to open <name>` | 255 (`--t1only`: 0) |
| betsurf | mesh not found | `error opening file` / `could not open the following file: <name>` / `terminating.` | 255 |
| all | `FSLOUTPUTTYPE` unset or unknown | §3.1 | 1 |

## 8. Numeric conventions that decide the bits

For an original that wants to stay as close as possible to FSL (not required):
- intensities are float32; the thresholds `t2`, `t98`, `t`, `tm` and all geometry are double;
  trilinear interpolation is in float (its coordinates rounded to float first);
- `pow` (`f^0.275`, the radius), `tanh` and `exp` (the Gaussian weights) are glibc's: not correctly
  rounded; a correctly rounded `pow` differs in the last bit for about 0.09 % of BET's arguments,
  which can change the surface after 1000 iterations by a voxel in rare places;
- C's conversions of double to int truncate toward zero; `(int)` of a value out of range or NaN is
  `INT_MIN` on x86-64;
- the sums over a vertex's triangles and neighbours are in the order the triangles were created
  (§4.4's construction, each subdivision's new triangles in order);
- the NIfTI headers' quaternions are computed as FSL's NIfTI library does, in x87 extended
  precision.

## 9. Defects (D13): what the default does

| ID | Behaviour of FSL | Default (`fix`) | `reproduce` |
|---|---|---|---|
| FSL-40 | bet2 removes the first `.hdr` of each name wherever it stands (a directory `x.hdr` is read as `x`); a name with `.img` but no `.hdr` aborts | removes a trailing extension only | FSL's names (and the refusal where FSL aborts) |
| FSL-41 | no voxel for the median intensity: FSL reads an empty list and crashes | refuses | `tm = 0`, goes on, warns |
| FSL-42 | NaN `-f`/`-g`, a negative, NaN or infinite `-r`, `-w ≤ 0`, a non-finite `-c` or search depth are used as given | refuses, naming the value | uses them |
| FSL-43 | the mask's fill starts outside the image (a single slice): FSL writes and reads outside the volume; further outside, or at a centroid that is not a number, it writes far outside the volume and crashes | starts at the nearest voxel inside (a centroid that is not a number: refuses) | one voxel outside: only FSL's defined steps (the start's neighbours inside the image); further: no fill (the drawn surface only); warns |
| FSL-44 | bet expands its arguments unquoted: a name with a blank becomes several words; an empty argument ends the options | quotes them | FSL's word splitting |
| FSL-45 | `-Z` crops with `imglob`, called without `${FSLDIR}/bin`: off the PATH the outputs stay padded; on it, other images named like the output are cropped too | crops exactly its outputs | FSL's behaviour for the PATH given |
| FSL-46 | the cleanup removes every `<out>_tmp*` file, also files the run did not create | removes only its own | removes all |
| FSL-47 | `--t1only`'s scalp mask is filled from the outer skull surface's centre in the working grid | fills from the scalp surface's | FSL's |
| FSL-48 | a T2 on another grid is sampled at the T1's voxel coordinates | refuses | samples it so |
| FSL-49 | `S < S2` keeps the T1's scalp (`outskin = outskin`) | kept (rule C) | kept |
| FSL-50 | one profile with an empty search range ends betsurf with 255, silently, before any output | drops the profile | exits 255 |
| FSL-51 | a profile search reads past its last sample (memory never set) | drops the profile | reads "nothing there" (+∞, NaN) and warns |
| FSL-52 | a mesh file FSL cannot read (values never set, missing vertices, empty) | refuses | the values C++ stores, 0 for those never set, faces naming missing vertices left out; warns |
| FSL-53 | a matrix with fewer than 12 readable numbers leaves entries never set (the first failing one 0) | refuses | uses 0 for them, warns |
| FSL-54 | `--t1only` exits 0 also when it fails | exits with the failure's status | exits 0 |
| FSL-55 | a NaN in the T1 inside the brain surface: the 98th percentile is undefined | refuses | libstdc++'s selection, warns |
| FSL-56 | two or more extensions of different sizes: the data offset counts the first one's size once per extension | the correct offset | FSL's offset |
| FSL-57 | betsurf sets the smoothness weight to 0 after it was used (no effect) | kept (rule C) | kept |

Each correction that changed a result is named on standard error
(`larmorx: upstream defect <ID> (<tool>) corrected: ...`) and in Python as an
`UpstreamDefectWarning`.

## 10. Built for speed

Where FSL spends its time (one 1 mm T1, about 2 s for bet2 and 40 s for betsurf on one core):
- bet2: 1000 (to 11 000) iterations over 2562 vertices, each sampling 8-10 voxels along the
  normal; the self-intersection measure (an all-pairs scan of 2562 vertices, 6.5 million pairs).
  Every vertex's update reads only the previous iteration's positions: the vertices can be updated
  in parallel, deterministically. The all-pairs scan can use a spatial grid of cell `ml`.
- betsurf: the profiles (tens of thousands, each 180-260 trilinear samples of two images): fully
  parallel; the three smoothings of padded volumes (separable, parallel per line); 3100 deformation
  steps of 2562 vertices (parallel per vertex, as above).
- The masks' fills: a breadth-first fill or a scanline fill is far faster than FSL's stack of
  single voxels, with the same result.
- Reading and writing: `.nii.gz` compression dominates for large images (parallel deflate).

## 11. Validation plan

Against FSL 6.0.7.17 (`~/fsl`), through `python -m larmorx_validation parity fsl-bet` (the
replica's suite, `docs/validation/fsl-bet-replica.md`, is the model; the original's suite runs the
same cases with `implementation="original"`):
- **Cases**: fMRIPrep's 29 calls; every option of bet2, bet and betsurf; every storage type,
  scaling, orientation (radiological and neurological storage, qform or sform only, oblique) and
  file format; 2D and 4D inputs; the messages and failures of §7; the defects of §9 in both modes;
  synthetic heads with a known brain; real T1w, T2w and EPI images of the data panel (children,
  older adults, 1.5 T, 3 T, 7 T, a tumour, a stroke, motion).
- **Metrics**, for each output against FSL's:
  - exit status and messages: identical;
  - masks: Dice ≥ 0.995 on real data and phantoms, and the boundary distance (the symmetric
    distance between the two masks' surfaces, in voxels): 95th percentile ≤ 0.5, maximum ≤ 2;
  - the brain image: identical values inside both masks (it is the input times the mask);
  - meshes: per-vertex distance to FSL's vertices (the same vertex numbering): 95th percentile
    ≤ 0.1 mm, maximum ≤ 1 mm;
  - betsurf's masks: Dice ≥ 0.99, boundary distance 95th percentile ≤ 1 voxel;
  - the verbose values (`t2`, `t98`, `t`, `tm`, the centre, the radius): relative difference
    ≤ 1e-6.
- **FSL's own variability**: FSL is deterministic (the same input gives the same bits, run after
  run, at any load); between builds (fMRIPrep 25.2.6's FSL, with newer image libraries) the record
  reports the differences. The thresholds above are about one voxel at the boundary, the scale of
  a last-bit difference in `pow` after 1000 iterations (§8).
- **Speed**: against FSL and the replica on the real data (single thread and all cores),
  reported, not gated.
