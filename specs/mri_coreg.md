# `mri_coreg` (FreeSurfer 8.2.0): behaviour spec

Status: queue item 3.2-17 (track C relay 52): the whole program but `--landmarks` and
`--landmarks-2d` (not ported: "is not ported yet", status 1). Written by the replica's author as
the spec writer: behaviour, conventions and messages, for the clean room (`larmorx-fs-volumes`),
which works from this file, the library modules' specs (`specs/freesurfer-core.md` for the version
options, `FSENVgetenv`, `nint` and the C library's numbers; `freesurfer-maths.md` for the matrices
and `OpenPowell2`; `freesurfer-volume.md` for the transform files, `TranformAffineParams2Matrix`,
`TranformExtractAffineParams`, `LTAmat2RotMat`, `RMSregDiffMJ`, `MRIconform`, `MRIresample` and
`MRIcomputeCentroid`; `freesurfer-filter.md` for `MRIgaussianSmoothNI`; `freesurfer-io.md` for the
volumes), `specs/ranlux.md` (the random draws), `specs/glibc-qsort.md` (the sort) and black-box runs
of the official program only.

Conventions: `%d`, `%f`, `%g`, `%e` are C's `printf` conversions (`%lf` is `%f`); "prints" means
standard output unless said otherwise; numbers are IEEE binary64 (`double`) unless named `float`;
every sum is from 0 in the order written. `log`, `log2`, `exp` and `pow` are the C library's
(glibc 2.35's bits with `reproduce`, correctly rounded by default: GLIBC-07, no notice: the program
fits a registration); `sqrt`, `floor`, `ceil` and `round` are exact.

## 1. The command line

Options are read one by one; each name is compared without case but `--sd`, `-SDIR`,
`--no-coord-dither`, `--no-intensity-dither` and `--no-dither`, which are exact. With `--debug`
on, each later option is first printed as `<arguments left, the option included> <option>`.

- No argument: the usage (the end of §10), status 1. `--help`: the usage (with the settings read so
  far: `--dof 9 --help` prints "default is 9") and three lines, status 1. `--version`: the version
  line (`$Id: mri_coreg.c,v 1.27 2016/04/30 15:11:49 greve Exp $`), status 1. The version options of
  `freesurfer-core.md` (`--all-info`, ...) come first, as in every FreeSurfer program.
- An option missing its arguments: `ERROR: <option> flag needs <n> argument(s)` on standard error,
  status 255; `<n>` is the count the option takes but `--rms`, which needs 2 and names 4.
- An unknown option: `ERROR: Option <option> unknown` on standard error, and for a single dash
  `       Did you really mean -<option> ?`; status 255.
- A number that `sscanf` does not read leaves the setting as it was (FS-1316 for the two that
  FreeSurfer reads from a variable it never set: §10).

| Option | Effect |
|---|---|
| `--mov <v>` | the moving volume |
| `--ref <v>`, `--targ <v>` | the reference volume |
| `--reg <f>`, `--lta <f>` | the output registration |
| `--regdat <f>` | also the registration as `register.dat` |
| `--s <subject>` | the subject; also the reference mask `aparc+aseg.mgz` |
| `--sd <dir>`, `-SDIR <dir>` | sets `SUBJECTS_DIR` for the rest of the program |
| `--ref-mask <v>`, `--no-ref-mask` | the reference's mask, or none (put after `--s`) |
| `--mov-mask <v>` | the moving volume's mask |
| `--dof <n>` | the parameters optimised (default 6), schema 1 |
| `--6`, `--9`, `--12` | the parameter count only (schema unchanged) |
| `--zscale` | schema 3, 7 parameters |
| `--xztrans+yrot`, `--2dz` | schema 2, 3 parameters |
| `--xytrans+zrot` | schema 4, 3 parameters |
| `--xytrans+zrot+xyscale+xyshear` | schema 5, 6 parameters |
| `--sep <n>` (repeatable) | the separations in voxels (default 4 then 2) |
| `--ftol <x>`, `--linmintol <x>` | Powell's tolerances (defaults 1e-7, 1e-3) |
| `--nitersmax <n>` | Powell's limit (default 4) |
| `--bf`, `--no-bf` | the brute-force search on or off (default on) |
| `--bf-lim <x>`, `--bf-nsamp <n>` | its range (default 30) and samples (default 30); both turn it on |
| `--sat <pct>` | the saturation percentile (default 99.99) |
| `--seed <n>` | the dithers' seed (default 53) |
| `--no-coord-dither`, `--no-intensity-dither`, `--no-dither` | the dithers off |
| `--mov-idither <v>` | the moving volume's intensity dither read from a volume |
| `--no-smooth` | no smoothing of either volume |
| `--ref-fwhm <x>` | the reference smoothed by `x` mm on each axis before all, the rest unsmoothed |
| `--mov-oob`, `--no-mov-oob` | samples outside the moving volume counted as 0, or skipped (default) |
| `--cras0` / `--centroid` / `--no-cras0`, `--regheader` | the initial translation: the centres (default), the centroids, none |
| `--trans x y z`, `--rot x y z`, `--scale x y z`, `--shear x y z` | the initial parameters; `--trans` also `--no-cras0` |
| `--init-reg <lta>` | the initial parameters from a registration; also `--no-cras0` |
| `--init-reg-save <lta>`, `--init-reg-save-only <lta>` | the initial registration written (and the end) |
| `--ras2ras` | the output as RAS to RAS (default voxel to voxel) |
| `--conf-ref` | the reference conformed without rescaling |
| `--params <f>`, `--final-cost <f>`, `--log-cost <f>` | the optimised parameters, the final cost, every cost |
| `--init-cost-only <f>` | the initial cost written, then the end |
| `--movout <v>` | the moving volume after the masks written |
| `--threads <n>`, `--nthreads <n>` | OpenMP's thread count (printed; no result depends on it) |
| `--rusage <f>` | read and kept; nothing is written |
| `--checkopts`, `--nocheckopts` | stop after the checks (status 0) |
| `--debug` | the options printed (above) and the brute force's lines (§6) |
| `--diag <n>`, `--diag-show`, `--diag-verbose` | FreeSurfer's diagnostic level and flags (`--diag-show` and `--diag-verbose` AND the flags: they can only clear bits) |

The stand-alone options (§8) do their work at once and end the program: `--mat2par`,
`--mat2par-vox`, `--mat2rot`, `--par2mat`, `--lrrev`, `--rms`, `--landmarks`, `--landmarks-2d`.

**The checks**, after FreeSurfer's environment record is made (`FSENVgetenv`: its messages,
`FREESURFER_HOME not defined`, `SUBJECTS_DIR not defined`, `ERROR: reading <file>` for its colour
table, on standard output), in order: no `--mov`: `ERROR: must spec --mov`, status 1; with `--s`,
the reference defaults to `brainmask.mgz` and the reference and the reference mask, each when no
file of that name can be read, become `<SUBJECTS_DIR>/<subject>/mri/<name>` (`(null)` for an
unset `SUBJECTS_DIR`; FS-1315 without a record or for a name of 3,000 bytes or more); no
reference: `ERROR: must spec --ref`; no `--reg`: `ERROR: must spec --reg` (status 1); no `--sep`:
4 then 2.

## 2. The report

Unless `--checkopts`, printed after the checks (each line `<key><value>` exactly so; the floats `%f`):

```

<version line>
cwd <working directory>
cmdline <the command line, the program's name first>
sysname  <uname's system>
hostname <uname's node>
machine  <uname's machine>
user     <the user>
dof    <n>
nsep    <n>
cras0    <0|1>
ftol    <f>
linmintol    <f>
bf       <0|1>
bflim    <f>
bfnsamp    <n>
SmoothRef <0|1>
SatPct    <f>
MovOOB <0|1>
optschema <n>
```

Then `Seed <n>`, `Reading in mov <name>`, the volume read (an unread volume ends with status 1
after the reader's messages), `Reading in ref <name>`, the reference; `--conf-ref`: `Conforming ref
(no scaling or rehistogramming)` and the reference replaced by itself resampled (nearest
neighbour) onto `MRIconform`'s grid, its name `<name>/Conformed` (cut at 1,023 bytes, with
`MRIconformNoScale: Truncation on line 2075` on standard error). The reference's voxel count
(`int`) is kept. `--mov-idither`: `Reading mov intensity dither file <name>`, read.

## 3. The masks, the smoothing and the initial transform

- `--ref-mask`: `Reading in and applying refmask <name>`, read; for every voxel of the reference
  (its extent; the mask read at the same indices), the mask's value converted to `int` (C's
  truncation; NaN and out of range as x86-64: `INT_MIN`) below 0.5 sets the voxel to 0: values
  below 1 mask a voxel out.
- `--ref-fwhm`: `Smoothing ref by FWHM <%5.2f> <%5.2f> <%5.2f>`, the reference smoothed
  (`MRIgaussianSmoothNI`, in place) with standard deviations `fwhm / sqrt(log(256))` mm per axis.
- `--mov-mask`: `Reading in movmask <name>`, applied to the moving volume as the reference mask.
- The initial translation (`--cras0`, the default, or `--centroid`): `Setting cras translation
  parameters to align volume centers` (the centre voxel `((n - 1) / 2.0)` per axis) or `... to align
  centroids` with `Ref CRS centroid <%g> <%g> <%g>` and `Mov CRS centroid ...` (`MRIcomputeCentroid`);
  each centre's RAS point is the volume's voxel-to-RAS matrix (`MRIxfmCRS2XYZ`, base 0) times the
  column `(x, y, z, 1)` stored as `float`, in `float` (`MatrixMultiply`); parameters 0 to 2 become
  the moving point minus the reference point, subtracted in `float`.
- `Init matrix params ` and the twelve parameters (`%7.4f ` each; defaults 0 but the scales 1).
- The initial registration: the parameters' matrix (`TranformAffineParams2Matrix`), inverted in
  place (`MatrixInverse`: a singular matrix leaves its non-finite numbers), as a RAS-to-RAS
  transform from the moving volume to the reference (§7's `LTAcreate`), its subject the `--s`
  subject or `unknown`; `Initial Matrix (RAS2RAS)` and the matrix (`MatrixPrint`).
  `--init-reg-save`: written (a failing writer ends with its status); `--init-reg-save-only`:
  `--init-reg-save-only requested so exiting now`, status 0.
- The schema's parameters taken from the twelve (schema 1: the first `dof`; 2: 0, 2, 4; 3: 0 to 5
  and 8; 4: 0, 1, 5; 5: 0, 1, 5, 6, 7, 9), printed after `Initial parameters to be opt ` (`dof` of
  them, `%7.4f `). A parameter's matrix always uses all twelve (the others keep their initial
  values).

## 4. The dithers

Three uniform random fields, each its own RANLUX389 generator seeded with the seed
(`specs/ranlux.md`; the program's `int` seed as C converts it to `unsigned long`: `-1` is `2^64 - 1`;
seed 0 is the time of day's: FS-29, a fixed seed here, noted), each draw `flat(a, b)`:

- the coordinates' (unless `--no-coord-dither`): `Creating random numbers for coordinate
  dithering`, `flat(0, 1)` for every voxel of the reference in the order columns, rows, slices,
  then three frames innermost, each stored as `float`; or `NOT Creating random numbers for
  coordinate dithering`;
- the intensities' (unless `--no-intensity-dither`): `Performing intensity dithering`; the
  reference's field `flat(0, 1)`; with `--mov-idither` `Performing intensity dithering on mov with
  input dither volume` (no field), else `Performing intensity dithering on mov with computed
  dither` and the moving volume's field `flat(0, 0)` (it draws, and adds 0); or `NOT Performing
  intensity dithering`.

Standard output is flushed here (§9). The smallest separation (from 100) is kept. Then
`COREGprint`:

```
Separation list (<n>): <%2d each, a blank after>  min = <smallest>
DoSmoothing <0|1>
DoCoordDither <0|1>
DoIntensityDither <0|1>
nitersmax <n>
ftol <%5.3e>
linmintol <%5.3e>
SatPct <f>
Hist FWHM <f> <f>                    (7.000000 7.000000)
nthreads <OpenMP's thread count: --threads's (1 for 0 or less), else OMP_NUM_THREADS, else the CPUs; at most 128>
```

## 5. The preprocessing

For the moving volume, then the reference:

- The saturation: the percentile `Pct` of frame 0 (the values in the order slices, columns,
  rows, sorted by `qsort` with FreeSurfer's float comparison: -1, 1, or 0 for equal or a NaN;
  `specs/glibc-qsort.md`); the place `n = round(nvox * Pct / 100)` (an `int`); at `n = 0` or
  `n >= nvox - 1` the value there, else `(v[n] + v[n-1] + v[n+1]) / 3.0` (the three added in
  `float`, then divided in `double`), kept as `float`. FS-1320 for a place past the last value.
  Printed `movsat = <%6.4f>` / `refsat = <%6.4f>`.
- Rescaled to bytes in a float volume (the header copied): the minimum and maximum of frame 0 (from
  `-1e11` and `1e11`; the order columns, rows, slices), the maximum clipped at the saturation; each
  value clipped at the saturation, `nint(255 * (v - min) / (max - min))` as an `unsigned char`
  (wrapping modulo 256; `nint` of NaN as `freesurfer-core.md` says).
- The smoothing: per axis `1.15 sqrt(sep^2 - voxsize^2)` where that is positive, else 0, with `sep`
  the smallest separation; standard deviations `fwhm / sqrt(log(256))`; printed `mov gstd <%6.4f>
  <%6.4f> <%6.4f>` / `ref gstd ...`; when one is above 0 and smoothing is on, `Smoothing mov` /
  `Smoothing ref` and `MRIgaussianSmoothNI` in place, else `NOT Smoothing mov` / `NOT Smoothing ref`.
- The byte vector (columns fastest, then rows, slices): each voxel's value plus the dither (the
  field's draw as `float`, or the dither volume's value at the voxel, frame 0; 0 without either),
  added in `float`, clipped to [0, 255], `nint`.

Standard output is flushed after each volume; then `COREGpreproc() done`. `--movout`: `Saving mov to
<name>` and the moving volume as masked (not rescaled) written. `--log-cost`: the file opened
(else `ERROR: could not open <name> for writing`, status 1).

## 6. The cost and the search

**The cost** of the current parameters: the schema's parameters into their places of the twelve;
`M` (reference RAS to moving RAS) their matrix; `V2V = inverse(vox2ras(mov)) * M * vox2ras(ref)`
(`MatrixMultiplyD`, `float` matrices). The joint histogram (256 x 256, row the moving volume's
bin, column the reference's): the reference's columns `c` from 0 every `sep` while below its width
split in 32 chunks (`niters = (width + sep - 1) / sep`, `chunk = (niters + 31) / 32` columns'
steps each; chunk `k` the columns `[k chunk sep, min((k + 1) chunk sep, width))`), each chunk with
its own histogram, and for each column the rows and slices from 0 every `sep`:

- the point `(c, r, s)`; with the coordinate dither each coordinate plus `sep * d` computed in
  `float` (the dither's frame 0, 1, 2 at the voxel) and clipped at the extent minus 1;
- the moving point `V2V * (c, r, s, 1)` per row as `((V[0] c + V[1] r) + V[2] s) + V[3]` (the
  `float` entries as `double`); outside `[0, extent - 1]` on any axis (the third not checked, and 0,
  for schemas 2, 4, 5): skipped, or with `--mov-oob` (and during the brute force) a moving value 0;
  inside: a hit, the moving value the trilinear interpolation of its bytes;
- the reference value the trilinear interpolation of its bytes at the point;
- trilinear: corners `floor` and `ceil` per axis, `dc = c - floor(c)`, the eight terms
  `((wc wr) ws) v` added in the order (cm rm sm), (cm rm sp), (cm rp sm), (cm rp sp), (cp rm sm),
  (cp rm sp), (cp rp sm), (cp rp sp), `w` being `1 - d` for the lower corner and `d` for the upper;
- bins `i = floor(vf)`, `j = floor(vg + 0.5)`; `H[i][j] += 1 - (vf - i)` and, for `i < 255`,
  `H[i + 1][j] += vf - i`.

The chunks' histograms summed bin by bin in chunk order. The hits counted; `pcthits = pow(sep, 3) *
100.0 * hits / nvoxref` (left to right). The filters: for each axis a Gaussian `g[n] =
exp(-(n^2) / (2 std^2)) / (std sqrt(2 pi))` for `n` from `-14` to `14` (`std = 7 / sqrt(log 256)`;
`-(n^2)` an integer converted), each divided by their sum. Each row of `H` convolved with the
column-bins' filter (full convolution: 256 + 28 outputs, `out[n] = sum_k h[k] g[n - k]` over the
overlap, `k` increasing), then each column of the result with the row-bins' filter (284 x 284);
`FLT_EPSILON` (as a `double`) added to every bin; divided by their sum (taken column by column, the
rows inside); the cost `-(sum_r s1 log2 s1 + sum_c s2 log2 s2) / (sum H log2 H + FLT_EPSILON)` with
`s1` the row sums and `s2` the column sums (each in index order) and `H`'s sum column by column.
With `--log-cost` the line `<sep %2d> <evaluation %4d>  <the schema's parameters %7.5f each, a blank
after>  <cost %9.7f>`. The evaluations are counted from 0 (after the line).

**The overlap test**: `Testing if mov and target overlap`, the cost at separation 4, `Init cost
<%15.10f>`, `nhits = <n> out of <nvoxref>, Percent Overlap: <%5.1f>`; no hit: `ERROR: mov and ref do
not overlap`, status 1. `Initial  RefRAS-to-MovRAS` with `M`, `Initial  RefVox-to-MovVox` with `V2V`
(`MatrixPrint`). `--init-cost-only`: the cost written as `%20.10f\n` (FS-1324), status 0.

For each separation: `sep = <n> -----------------------------------`; at the first, unless
`--no-bf`, the brute force; then Powell.

**The brute force** (`lim0`, one pass, `n1d` samples): `COREGoptBruteForce() <%g lim0> 1 <n1d>`;
the moving volume's outside counted as 0 meanwhile (`Turning on MovOOB for BruteForce Search` when
it was off); the first six parameters (all, when fewer): from `p - lim` while at most `p + lim` in
steps of `(pmax - pmin) / n1d` (accumulated), the cost at each, the lowest over the whole search
(from `1e11`) kept and its parameter value set after the parameter's steps; with `--debug` the
line `iter=<i> n1d=<n> pno=<k> nom=<%g> min=<%g> max=<%g> delta=<%g>` before each parameter and
`<%2d iter> <%2d param> <%3d step>` then `  <%9.5f> ` per parameter and `  <%9.7f cost> <%9.7f
min>` after each cost (flushed). After the pass `#BF# sep=<%2d> iter=0 lim=<%4.1f> delta=<%4.2f>
<parameters %9.5f each, a blank after>  <%9.7f min>` (flushed), `lim` divided by `n1d`; `Turning
MovOOB back off after brute force search` when it was off. FS-1319 for a step that does not advance.

**Powell**: a blank line twice and `---------------------------------`, `Init Powel Params dof = <n>:
` with the parameters `%f ` each, `Starting OpenPowel2(), sep = <n>`; `OpenPowell2` from the
parameters as `float` along the identity's directions with `ftol`, `linmintol` (both `float`) and
`nitersmax` (`freesurfer-maths.md`); each evaluation sets the parameters from Powell's `float`s,
takes the cost (`float`) and keeps the best: at the first evaluation of each separation `InitialCost
<%20.10f> ` and the best set; whenever the cost (as `float`) is below the best, the best replaced;
when the best changed, `#@# <%2d sep> <%4d evaluations so far>  <the best's parameters %7.5f each,
a blank after>  <%9.7f best>` (flushed). Then `Powell done niters total = <n>`, `OptTimeSec <%4.1f>
sec`, `OptTimeMin <%5.2f> min` (wall time since the start of this Powell), `nEvals <total
evaluations>` (flushed), `Final optimized parameters ` with Powell's result `%12.8f ` each, `Final
matrix parameters ` with the twelve `%7.4f `, the cost of the result, `Final cost <%20.15f>` and a
newline and a blank, then `\n\n---------------------------------\n`.

## 7. The outputs

`LTAcreate(src, dst, T, type)`: one transform with the volumes' geometries, a copy of `T`, the
transform's and the array's type; for `REGISTER_DAT` it prints `  ... using LTAcreate() with
REGISTER_DAT, make sure matrix points in the right direction` (no caller of this program passes it).

The output registration: `inverse(V2V)` (FS-1325 when singular) from the moving volume to the
reference as voxel to voxel (`--ras2ras`: RAS to RAS), the subject `--s`'s or `unknown` (1,023
bytes), written (a failure: status 1); `--regdat`: converted to `register.dat` and written (status 1
on failure). Then `Final  RefRAS-to-MovRAS` (`M`), `Final  RefVox-to-MovVox` (`V2V`), `Final matrix
parameters ` with the twelve from the schema's parameters into twelve zeros (not the initial
values: a scale not optimised prints 0.0000), `Final opt parameters ` with the schema's parameters
(`%7.4f ` each); `--params`: those as `%7.4f ` each and a newline; `--final-cost`: `%20.13f \n`
(FS-1324 for both); `nhits = ...` as above; `mri_coreg RunTimeSec <%4.1f> sec`; without
`--conf-ref`: `To check run:` and `   tkregisterfv --mov <mov> --targ <ref> --reg <reg> ` (with
`--s`: `--s <subject> --surfs `); then a blank line twice, `mri_coreg done`, a blank line; status 0.

## 8. The stand-alone options

- `--mat2par reg.lta` / `--mat2par-vox reg.lta`: the transform read (unread: status 1), converted to
  RAS to RAS / voxel to voxel, inverted, its twelve parameters (`TranformExtractAffineParams`)
  printed `%g ` each and a newline; status 0.
- `--mat2rot reg.lta out.lta`: read, `LTAmat2RotMat`, written; the writer's status.
- `--par2mat p1 .. p12 src trg reg.lta`: the parameters printed `%f` one per line (FS-1316 for one
  that does not read), the two volumes read (status 1), the parameters' matrix inverted as a RAS to
  RAS transform from `src` to `trg` (subject from an earlier `--s`, else `unknown`), written; the
  writer's status.
- `--lrrev reg.lta out.lta`: read (status 1), its type kept, converted to RAS to RAS; `Tinv0` its
  matrix inverted, `M = ras2vox(src) * Tinv0` (`float`); the source geometry's orientation string
  printed `Orientation <xyz>` (`???` without valid cosines); the left-right axis `k` (1, 2 or 3; the
  last of the three letters that is `L` or `R`) and its voxel count printed `k=<k> ndim=<n>` (FS-1318
  for none); `K2` the fourth column of `M` with `K2[k] = (n - 1) - K2[k]` (`float`), `P2 =
  vox2ras(src) * K2`; the transform inverted, its parameters printed after `Input Parameters: ` (`%g `
  each); parameter 0 becomes `P2[0]`, parameters 4, 5, 9, 10 change sign; printed after `New
  Parameters:   `; their matrix copied into the transform, which is inverted, converted back to its
  type and written; the writer's status.
- `--rms radius file reg1 reg2`: a radius starting with a letter: `ERROR: first arg to --rms must be
  the radius`, status 1; one at or below 0: `ERROR: radius is <%f>, must be > 0`, status 1 (FS-1316
  for one that does not read); the two registrations read (status 1), each converted to
  `register.dat`; Jenkinson's RMS difference (`RMSregDiffMJ`) written as `%20.10f\n` to `file` unless
  it is `nofile`, printed `rms <%20.10f>`; status 0. FS-1317 for two or three arguments, an output
  that does not open, a registration without a matrix.
- `--landmarks`, `--landmarks-2d` (6 arguments needed): not ported.
- `--init-reg reg0.lta` (not stand-alone): read (status 1 when not), converted to RAS to RAS,
  inverted, its twelve parameters the initial ones; `--no-cras0`.

## 9. Exit statuses, threads, standard output

0; 1 (the checks, a volume or transform that does not read, no overlap, a write that fails); 255
(the command line); a writer's status (`--init-reg-save`, `--mat2rot`, `--par2mat`, `--lrrev`);
FreeSurfer's crashes as §10 says. The official build runs the histogram's 32 chunks and
`MRIgaussianSmoothNI` on OpenMP threads; the chunks are summed in their order (no result depends on
the threads), and the smoothing's `long double` kernel sum is the one-thread order (variation.md
measured the same transforms at 1, 2 and 4 threads).

**Standard output at a crash** (`reproduce`): FreeSurfer's standard output is the C library's
buffer (4,096 bytes when redirected); a crash keeps only what was written out: everything up to the
last flush (the dither lines, each volume's preprocessing, each `#@#` and `#BF#` line, `nEvals`, each
`MatrixPrint`, each error message of FreeSurfer's C++ stream) and the full buffers after it.

## 10. The defects (`larmorx_core::defects`)

FS-1313 (more than 10 `--sep`: an error in both modes), FS-1314 (`--threads` without a number: the
count kept), FS-1315 (`--s` without an environment record, or a name of 3,000 bytes or more:
FreeSurfer crashes), FS-1316 (`--par2mat`'s and `--rms`'s numbers that do not read: 0), FS-1317
(`--rms` with two or three arguments, an output that does not open, a registration without a
matrix: FreeSurfer crashes), FS-1318 (`--lrrev` without a left-right axis: an error in both
modes), FS-1319 (a brute-force step that does not advance: FreeSurfer never ends; an error in both
modes), FS-1320 (the percentile past the sorted values: the last value by default, 0 with
`reproduce`, which is what FreeSurfer read in every case measured), FS-1321 (a NaN coordinate: an
error in both modes), FS-1322 (`--dof` above 12 with schema 1: an error in both modes), FS-1323 (a
separation of 0 or less: an error by default; with `reproduce` `--sep 0` ends with status 136 at
its first histogram as FreeSurfer's division by 0 does, and a negative one runs where no column is
sampled and is an error where FreeSurfer's loop never ends), FS-1324 (`--init-cost-only`,
`--params`, `--final-cost` that do not open: FreeSurfer crashes), FS-1325 (a singular final matrix,
`--conf-ref` on a reference that does not conform: an error in both modes). FreeSurfer's crashes
(FS-1315, FS-1317, FS-1324) give status 139 with `reproduce` after the output FreeSurfer's buffer
had written out (§9); an error (status 1) by default.

The usage text (`print_usage`) is in the replica's `options.rs` as FreeSurfer prints it: the clean
room takes it from the official program's output (`mri_coreg` with no argument, and `--help` for
the three lines after it).

## 11. Validation plan (for the clean room)

- The probe `validation/probes/fs-coreg/` runs the replica (closed); the clean room checks against
  the official program as a black box with the same kinds of cases: the command line's paths and
  failures, the stand-alone options, every registration option on a small "BOLD" volume (20 x 20 x
  14 voxels of 2.4 mm) and a reference (32^3 of 1.5 mm), masks and segmentations, a subject's
  folder with `SUBJECTS_DIR` or `--sd`, FreeSurfer's environment missing; seeded pairs of every type
  and orientation with random options. Each case with `FREESURFER_HOME` a folder holding a small
  colour table and the official program at one thread; the times and the machine's lines masked.
- The recorded heads: fMRIPrep's `MRICoreg` call and `bbregister --init-coreg`'s on four FreeSurfer
  reference subjects with fMRIPrep's BOLD references, byte for byte.
