# Spec: `flirt`, `convert_xfm` and FLIRT's other programs (behaviour of FSL FLIRT, FSL 6.0.7.23)

The input for the clean-room **original** of larmorx's FLIRT (Apache-2.0, in the main package,
written later by an implementer who has never read FSL's source). FSL is under the FSL Licence
(non-commercial); the bit-exact replica is `larmorx-nc fsl flirt` (`crates-nc/larmorx-nc-flirt`).

**Whoever works on the original must not open:** FSL's source code (`~/fsl-*/src/`, the copies in
`crates-nc/fsl-source/`), the replica crates (`crates-nc/`), `docs/findings/fsl-flirt.md`,
`docs/findings/fsl-mcflirt.md`, `docs/findings/fsl-bet.md`, `docs/parts/`, and the restricted (†)
sections of the defect register (`docs/porting-guide.md` §1).

This spec describes behaviour: what goes in, what comes out, the maths and the numeric
conventions that decide the results, the command line, the messages and the failure cases.
Statements marked **(observed)** were confirmed by running FSL 6.0.7.23's `flirt` as a black box
(the `fsl-flirt-replica` parity suite's runs, `docs/validation/fsl-flirt-replica.md`). FLIRT shares
its cost functions, interpolation and line search with mcflirt: where this spec says "as
`specs/mcflirt.md` §n", the rule there holds unchanged, with the differences stated here.

**Version.** FSL 6.0.7.23, Linux x86-64 conda build (fsl-flirt 2111.4, fsl-newimage 2601.0,
fsl-miscmaths 2412.6, fsl-newnifti 5.0.0, fsl-armawrap 0.7.0 with Armadillo 14.2.1, OpenBLAS 0.3.33
on its Haswell kernels). `flirt -version` prints `FLIRT version 6.0`.

**Built for speed.** The original must give FLIRT's results within the thresholds of §13 and is
built to be much faster (`docs/porting-guide.md`, "Clean-room originals"): FLIRT spends almost all
its time in cost evaluations (§6), which are independent across grid rows (sums in fixed blocks,
§12) and across the search's grid points (§8.1), and in the pyramid's filters (§5), which are
separable. FSL takes 4 s for a default 12-dof run of a 3 mm BOLD reference to a 0.8 mm T1w and 46 s
for fMRIPrep's boundary-based call on the same pair, on one thread (observed).

---

## 1. Purpose and fMRIPrep's use

FLIRT finds the affine matrix that maps an input volume onto a reference volume by minimising a
cost function over 6 to 12 parameters, through a multi-resolution schedule (8, 4, 2, 1 mm) that
starts with a search over rotations. It writes the matrix (`-omat`) and the input resampled onto
the reference's grid (`-out`), or applies a given matrix (`-applyxfm`).

fMRIPrep 25.2.6 calls it once per BOLD run without FreeSurfer, for the boundary-based registration
of the BOLD reference to the T1w (`bold_reg_wf/fsl_bbr_wf/flt_bbr`):

    flirt -in <boldref> -ref <T1w brain> -out <name>.nii.gz -omat <name>.mat -basescale 1
          -searchcost bbr -dof 6 -init <initial matrix> -schedule $FSLDIR/etc/flirtsch/bbr.sch
          -wmseg <white-matter mask>

The initial matrix comes from FreeSurfer's `mri_coreg` (converted to FLIRT's convention). The
schedule `bbr.sch` (§7.4) switches the cost to `bbr` itself; `-searchcost bbr` is therefore only
what the cost is reset to by that schedule. This call is tier 1; the default schedule and the
other options are tier 2; the schedule language's less used commands tier 3.

## 2. Command line

`flirt [options] -in <inputvol> -ref <refvol> ...` Options are scanned left to right; each word
must start with `-`, except the arguments of the option before it.

**Without arguments** the usage text (§2.4) is printed on standard output, exit status 1.

**No-argument options:** `-help` (usage on standard output, status 0), `-version` (`FLIRT version
6.0`, status 0), `-applyxfm` and its alias `-applynonisoxfm` (apply `-init`, no optimisation, no
isotropic resampling of the reference), `-i` (accepted, no effect), `-nosearch` (every rotation
search range set to `0 0`), `-nosave` (accepted; the default), `-noresample`, `-forcescaling`,
`-2D`, `-noclamp`, `-noresampblur`, `-usesqform`, `-printinit`, `-debugsave` (debug volumes of the
search are saved, §8.1), `-debug`, `-v` (verbose 1).

**One-argument options:** `-o`/`-out`, `-ref`, `-in`, `-init` (a matrix file; also turns
`-usesqform` off), `-schedule`, `-refweight` and `-inweight` (turn weighting on, §6.6), `-wmseg`,
`-fieldmap`, `-fieldmapmask`, `-wmcoords` and `-wmnorms` (turn the coordinate form of BBR on),
`-omat`, `-bins` (integer, C `atoi`), `-dof` (integer), `-sincwidth` (integer read, then used as a
float), `-paddingsize`, `-applyisoxfm <scale>` (as `-applyxfm` with isotropic resampling of the
reference to `scale` mm), `-basescale`, `-minsampling`, `-setbackground`, `-coarsesearch` and
`-finesearch` (degrees, stored as radians in single precision), `-echospacing`, `-pedir`
(integer; 0 is an error), `-bbrtype`, `-bbrslope`, `-verbose` (integer), `-datatype`
(`char|short|int|float|double`), `-cost` and `-searchcost`
(`mutualinfo|corratio|normcorr|normmi|leastsq|labeldiff|bbr`, and `woods`, which the usage text
does not list), `-interp` (`trilinear|nearestneighbour|sinc|spline`), `-sincwindow`
(`rectangular|hanning|blackman`), `-anglerep` (`quaternion|euler`). Numbers are read with C's
`atof`/`atoi` (a word that is not a number reads as 0).

**Two-argument options:** `-searchrx`, `-searchry`, `-searchrz <a> <b>`: the range `[min(a,b),
max(a,b)]` in degrees, stored in radians (double).

**Defaults:** cost and search cost `corratio`, 12 dof, 256 bins, trilinear final interpolation,
sinc width 7 with a Hanning window, rotation ranges `[-90°, 90°]` on each axis, coarse step 60°,
fine step 18°, minimum sampling 1 mm, base scale 1 (unless §4.2 changes it), echo spacing 0.0005
s, BBR type `signed`, BBR slope −0.5, padding 0, verbose 0, Euler angles.

### 2.1 Messages and exit statuses of the scan (observed)

| Condition | Standard error | Status |
|---|---|---|
| a word not starting with `-` | `Unrecognised option <its first character>` | 255 |
| an option needing a value is the last word | `Lacking argument to option <opt>` | 255 |
| an unknown option with one word after it | `Lacking argument to option <opt>` | 255 |
| an unknown option with two or more words after it | `Unrecognised option <opt>` | 255 |
| a two-argument option with one word after it | `Lacking argument to option <opt>` | 255 |
| `-pedir 0` (or a non-number) | `Unrecognised argument to pedir (<word>) - it should be a number between -3 and 3 (not 0)` | 255 |
| `-datatype`, `-cost`/`-searchcost`, `-interp`, `-sincwindow`, `-anglerep` with an unknown value | `Unrecognised data type: <v>`, `Unrecognised cost function type: <v>`, `Unrecognised interpolation method: <v>`, `Unrecognised sinc window: <v>`, `Unrecognised angle representation: <v>` | 255 |
| no `-in` after the scan | `ERROR:: Input volume filename not found` and a blank line; the usage on standard output | 2 |
| no `-ref` | `ERROR:: Reference volume filename not found`, a blank line; the usage | 2 |

An empty word is skipped. The checks for `-in` and `-ref` come after the whole scan.

### 2.2 Errors after the scan

Every later error that FSL raises as an exception (a missing or unreadable image, a malformed
matrix file, a product of matrices of wrong sizes, an unknown BBR type) ends the program with
**status 0**: the message is printed on standard error, preceded by the image library's own line
where it prints one (`Image Exception : #<n> :: <message>`, then the message again). Outputs not
yet written are not written. (observed)

- An image name that cannot be found: the image library's message for it, status 0.
- `-init` naming a file that cannot be opened: `Could not open matrix file <name>`, then the
  matrix is empty and the first product with it fails (`matrix multiplication: incompatible
  matrix dimensions: 4x4 and 0x0`), status 0.
- A matrix file with fewer numbers on a line than its first numeric line: `Could not find enough
  numbers in matrix file`, status 0.
- `-schedule` naming a file that cannot be opened: `Could not open file<name>` (no blank), status
  255, nothing written.
- `-wmcoords`/`-wmnorms` empty or of different numbers of rows: `Coordinate and Normal matrices
  <a> and <b> are either zero size or different sizes`, status 1.
- `-bbrtype` other than `signed|global_abs|local_abs`, when BBR is first used: `Image Exception :
  #30 :: Unrecognised BBR type: <v>` and `Valid types are: signed, global_abs, local_abs`, then the
  same two lines again, status 0.
- `FSLOUTPUTTYPE` unset when an image is written: FSL's message about the variable, status 1 (as
  mcflirt, `specs/mcflirt.md` §10.1; larmorx's default writes `.nii.gz`, FSL-11).
- A matrix a cost evaluation must invert that is singular (a scale of 0 reached by an
  optimisation, e.g. through a parameter subset on degenerate images): `inv(): matrix is
  singular`, status 0, nothing written, wherever in the schedule it happens (observed, part 3).
- An image the program cannot allocate (an isotropic reference of voxels of 0.01 mm: hundreds of
  GB): `Image Exception : #99 :: Out of memory`, then `Out of memory`, status 0 (observed under an
  address-space limit, part 3). The original reports it alike where its own allocation fails, and
  never starts a computation it cannot hold.

### 2.3 Verbose output

`-v` (verbose 1) prints, on standard output: ` >> <line>` for every line of the schedule before it
runs (comments and blank lines included); `The output image will use the sform from the reference
image` when the reference has an sform, `The output image will use the transformed sform from the
input image` when the input has one (both can appear; with `-applyxfm` only the first that
applies); `Init Matrix = ` and the initial matrix (§2.5's matrix printing) followed by an empty
line (also with `-printinit`); on standard error `WARNING: Both reference and input images have
an sform matrix set` when both have one. Higher levels add (each level includes the lower):
- 2: the input's sampling matrix, `Testvol Data Type = <code>` (0 after the first reading), the
  intensity range after clamping, each isotropic resampling of the reference (`Resampling refvol
  isotropically: <mm>` and four lines of image information), `Subsampling the volumes`, a `*` per
  row of the search grids and a newline, `Median scale = <v>`, the final matrix (`Final transform
  matrix is:`);
- 3: centres of mass, the search's minima and chosen parameters, cost evaluation counts per
  scale (`Previous scale used <n> cost function evaluations`);
- 4 and more: the rotation samplings, the search's starting points and tolerances, and with
  `-debugsave` the search's volumes `tx`, `ty`, `tz`, `sc`, `costs` written in the input's data
  type;
- 5: every cost evaluation (`<count> : <cost> :: <parameters>`), 7: every subset evaluation.
Inside the search the verbose level is lowered by 2 (and by 2 more during the free optimisation of
its minima), so the levels above apply there with that offset. The exact text of each line, in
FSL's order, is in the parity record's verbose cases (observed).

### 2.4 Usage text (observed)

The usage prints `FLIRT version 6.0`, a blank line, three `Usage:` lines naming the program as it
was invoked, and the option list; the defaults of `-bins` and `-dof` it shows are the current
values (so `flirt -bins 64 -help` shows 64). The parity record holds the exact text.

### 2.5 Printing numbers

- **Matrix files** (`-omat`, schedule `save`): each element in C++ stream default notation with
  10 significant digits (`printf("%.10g")`), followed by two blanks, a newline after each row.
  `-omat` with an empty name writes nothing.
- **Matrices on standard output** (`Init Matrix`, `Final result`, `print`): each element in fixed
  notation with 6 decimals, followed by one blank, a newline per row. **The stream stays in fixed
  notation afterwards**: every later number printed on standard output (verbose lines, Powell's
  lines, `printparams`' costs) uses `%.6f`; before the first matrix, numbers print as `%g` with 6
  significant digits. (observed)
- Without `-omat`, after a registration: an empty line, `Final result: `, the matrix and an empty
  line.

## 3. Coordinates

FLIRT's matrices map **FLIRT millimetres of the input** to FLIRT millimetres of the reference:
voxel index times voxel size, after each image is put in radiological order, exactly as mcflirt's
`.mat` files (`specs/mcflirt.md` §4.2, §4.3). Images are read as `specs/mcflirt.md` §4.1 (a 4D
input to a 3D reading keeps its first volume with FSL's warning).

**The base scale.** Internally the voxel sizes of every image read are divided by the base scale
`b` (§4.2) when `|b − 1| > 1e-5`; the matrices of the schedule are in these scaled millimetres.
The initial matrix is converted in (`S M S⁻¹` with `S = diag(1/b, 1/b, 1/b, 1)`, `1/b` a single-precision quotient (exactly 100 for b = 0.01), the product of
three as §12 says), and the final matrix out: its translation column (rows 1-3 of column 4)
multiplied by `b`. The output image is computed from unscaled images.

**Parameters.** As `specs/mcflirt.md` §4.4: three Euler rotations (radians), three translations,
three scales, three skews, composed about a centre (`R = Rx·Ry·Rz`, then skew and scale, about
the centre). The centre is the input volume's intensity-weighted centre of mass (`specs/mcflirt.md`
§6.1) **of the volume the current image pair was built with** (§5.4), in scaled mm. Decomposition
(matrix → parameters) is the inverse, about the same centre.

**The composition exactly** (spec writer, 2026-11-02; every composition of FLIRT and `convert_xfm`,
the search's included; observed: the model of `validation/investigations/flirt-spec-answers/`
gives FSL's ten significant digits). With `n` parameters only the first `n` are used (7: one scale
for the three axes). The rotations are three elementary rotations, about x by `r_x`, then y by
`r_y`, then z by `r_z`, each a 4 × 4 matrix about the centre `c`, multiplied in that order
(`((I·R_x)·R_y)·R_z`, §12's small products). An elementary rotation, with `a` the angle vector (the
angle in its axis' place, zeros elsewhere): `θ = √(a·a)` (in double: the squares summed as §12's
dot products, the square root) rounded to single precision; the identity if `θ < 1e-8`. Otherwise
`u = a/θ` (double quotients: `u` is exactly ±1 on its axis only when the angle is a
single-precision number), `v = (−u_y, u_x, 0)` (or `(1, 0, 0)` if `√(v·v)` is not positive) divided
by `√(v·v)`, `w = u × v` (each component `p₂q₃ − p₃q₂` and its cyclic forms, the products rounded
first) divided by `√(w·w)`; `B` the 3 × 3 matrix whose columns are `v`, `w`, `u`; `C = [c s 0; −s c 0;
0 0 1]` with `c` and `s` the single-precision cosine and sine of `θ` (glibc's); the rotation is `(B·C)·Bᵀ`
(§12's small products) and its translation `(I − R)·c` (the difference first, then the
matrix-vector product). So the x rotation's (1, 1) entry is `u_x²`, not 1: for angles that are not
single-precision numbers (the optimiser's, `setrowparams`') FLIRT's matrices differ from the
textbook rotation of single-precision sines and cosines in their 8th significant digit (observed:
`1.018201124` against `1.018201155`). The translations are then added to the fourth column (`t_x`,
`t_y`, `t_z` in turn), and with more than 6 parameters the matrix is multiplied by the skew and then
by the scale, both about the centre: `(M·K)·S` with `K = [1 k₁ k₂; 0 1 k₃; 0 0 1]`, `S = diag(s_x,
s_y, s_z)` and their translations `c − K·c`, `c − S·c` (the products first).

**The decomposition exactly** (spec writer, 2026-11-02). Of a matrix `M` about `c`, with `x`, `y`,
`z` its first three columns and every dot product summed as §12's (two interleaved accumulators:
`(p₁ + p₃) + p₂` for three terms): `s_x = √(x·x)` in double, rounded to single; then in single
precision, each dot product rounded to single first: `d = x·y`, `s_y = √(y·y − d²/s_x²)`, `k₁ =
d/(s_x·s_y)`; in double `x₀ = x/s_x` and `y₀ = y/s_y − k₁·x₀`; in single `s_z = √((z·z − (x₀·z)²) −
(y₀·z)²)`, `k₂ = (x₀·z)/s_z`, `k₃ = (y₀·z)/s_z`. The rotation is `R = A·(S⁻¹·K⁻¹)` with `A` the 3 × 3
part, `K⁻¹` §12's triangular inverse of the unit upper triangle (its (1, 3) entry `−(k₂ − k₁k₃)`, the
product rounded first) and `S⁻¹·K⁻¹` each row of `K⁻¹` times the reciprocal `1/s` of its scale (the
reciprocal rounded first); the angles from `R` as `specs/mcflirt.md` §4.4 (`cy` the double square
root of `R₁₁² + R₁₂²` rounded to single; `R₁₁/cy` and the other quotients double quotients rounded
to single; glibc's `atan2f`), and the translations `A·c + m − c` (double, the product first, `m`
the fourth column).

**Quaternion parameters** (`-anglerep quaternion`; observed, the replica identical to FSL on the
record's cases). The three rotation parameters are the vector part `(x, y, z)` of a unit
quaternion; the search (§8) always uses Euler angles, every other composition and decomposition
uses the quaternion:
- *Composition.* `w² = 1 − x² − y² − z²` in double (left to right), stored in single precision; if
  it is negative, `Parameters do not form a valid axis - greater than unity` on standard error and
  the rotation is the identity (scales and skews are still applied). Otherwise `w = √w²` in single
  precision and, with `x, y, z` rounded to single precision, the rotation's entries in single
  precision, each product left to right (`2·y·y` is `(2y)·y`): `R11 = 1 − 2y² − 2z²`, `R22 = 1 −
  2x² − 2z²`, `R33 = 1 − 2x² − 2y²`, `R12 = 2xy − 2wz`, `R21 = 2xy + 2wz`, `R13 = 2xz + 2wy`, `R31 =
  2xz − 2wy`, `R23 = 2yz − 2wx`, `R32 = 2yz + 2wx`. The centre's translation `c − R·c` (double, a
  3 × 3 times 3-vector product), then the translations added; with exactly 3 parameters the
  translations are added too (with 4, 5 or 6, as many as there are). With 1 or 2 parameters FLIRT
  prints `Can only do 3 or more, not <n>` on standard error at each composition and still uses the
  three components. Messages printed during the optimisation appear on standard error as they
  arise, every time.
- *Decomposition.* The rotation part of §3's decomposition, its 3 × 3 entries stored in single
  precision, converted to a quaternion as the NIfTI-1 reference library converts a matrix
  (its polar decomposition when the columns are not orthonormal, the sign chosen so that the scalar
  part is non-negative), computed as FSL's x86-64 build computes it: the library's `long double`
  constants make some of its operations x87 extended-precision operations, rounded once to 64
  significant bits and again when stored; the parameters are its `(b, c, d)`.

## 4. Reading and preparing the images

### 4.1 The input and the reference

The input is read once (later readings reuse the first). Its data-type code (from the file; a
scaled file counts as float, `specs/mcflirt.md` §4.1) becomes the output data type unless
`-datatype` is given. Then:
1. **Single slices**: an image of one slice becomes three slices of the same values (the slice
   repeated above and below), with the slice thickness set to 8 mm and **every other property
   dropped** (no sform or qform, default interpolation); this applies to the input, the reference
   (unless `-applyxfm`), the weight volumes and the white-matter segmentation.
2. **Clamping** (unless `-noclamp`): every value is clamped to the image's robust range (the
   2%-98% thresholds of `specs/mcflirt.md`'s robust limits: FSL's `robustmin`/`robustmax`
   definition, the same as BET's, `specs/bet.md`), values above the upper limit set to it, else
   values below the lower set to it; NaN stays NaN.
3. **Weights**: with `-inweight` (`-refweight`) the weight image is read (scaled voxels, single
   slices tripled); with weighting on and no weight file, a volume of ones on the image's grid.
4. With `-fieldmap`: the field map (rad/s, on the reference's grid) and `-fieldmapmask` (default:
   `0·fmap + 1`, so ones except where the map is NaN). With `-wmseg`: the white-matter
   segmentation; with `-wmcoords`: the two text matrices (§9.2).

**Neurological files** (spec writer, 2026-11-02; questions 9 and 15, observed with the model). Every
image read is put in radiological order, and back when it is written: a file whose sform (if set,
else its qform) has a positive 3 × 3 determinant (a sform and a qform of opposite signs, or a zero
determinant: left as stored) has its data reversed along x, and its sform and qform (both, whatever
their codes) replaced by `W·D⁻¹·F⁻¹·D` (the four products in §12's order; `D = diag(d_x, d_y, d_z,
1)`; `D⁻¹` by reciprocals; `F = [−1 0 0 f; 0 1 0 0; 0 0 1 0; 0 0 0 1]`, whose §12 triangular inverse
is `F` itself), with the field of view **`f = (n_x − 1)·d_x` computed in single precision** (95 ×
1.7 gives 161.5, not 161.500004530). Writing does the same with the written grid's `n_x` and `d_x`.
The forms are read as single-precision matrices: the sform's rows as stored, the qform from its
quaternion by the NIfTI-1 reference library's conversion (its long double constants as x87
operations, each element stored in single precision).

### 4.2 The base scale

Unless `-basescale` is given, it is decided from the two images' headers: with `M` the largest and
`m` the smallest absolute voxel size over both images (single precision), `b = M/8` if `M > 12`;
then `b = m` if `m < 0.75` (the second wins). With `-applyxfm` the base scale is 1. (spec writer,
2026-11-02) The voxel sizes are the headers' as stored, read before anything else and before a
single slice is tripled (§4.1): a single slice counts with its own thickness (observed by the
implementer: an 8 × 1 × 1 reference of 1 × 1.5 × 0.5 mm voxels gives `b = 0.5`, FSL-75's case).

### 4.3 The minimum sampling

`s_min = ceil(max(min voxel size of the reference, min voxel size of the input))` (scaled sizes,
after the single-slice tripling, so a single slice counts 8 mm); unless `-forcescaling`, the
working minimum sampling is `max(-minsampling, s_min)`.

## 5. The pyramid

All filters below keep the image's properties except where said.

**Blur to a resolution `r`** (`blur(I, r)`): a separable convolution along x, y, z with, per axis,
`ratio = r / d` (`d` that axis' voxel size, single precision): no blur (a kernel `[1]`) if `|d| <
1e-8`, `ratio < 1.1`, or `σ = 0.85·ratio/2 < 0.5` (single precision); otherwise `n = 2·trunc(σ −
0.001) + 3` taps `exp(−(k − c)² / (4σ²))`, `k = 1..n`, `c = n/2 + 1` (integer), the square in
integers converted to single, `4σ²` as `(σ·σ)·4.0` with `σ·σ` single, the quotient and `exp` in
double (glibc's `exp`), normalised by their sum (two interleaved accumulators, even and odd taps,
then added). The convolution is the same as mcflirt's smoothing passes (each output value the
single-precision running sum, in kernel order, of the double products of value and tap; values
outside the image count 0; the kernel centred at `(n − 1)/2`). Note the Gaussian's `4σ²`: this is
a Gaussian of standard deviation `√2·σ`.

**Isotropic resampling to `s` mm**: `specs/mcflirt.md` §5.3's resampling (trilinear, the volume's
extrapolation; grid `max(1, trunc(n·d/s))` points per axis, positions accumulated in single-
precision steps `s/d`), the voxel size set to `s`, the sform and qform composed with the scaling.

**Halving** (`subsample by 2`): a grid of `⌈n/2⌉` points per axis at the even voxels; each value
`0.125·v(c) + 0.0625·Σ6 faces + 0.0312·Σ12 edges + 0.0156·Σ8 corners` of the 3×3×3 neighbourhood,
each group summed in single precision (in the order +x, −x, +y, −y, +z, −z for the faces; for the
edges xy±±, then xz±±, then yz±± with the first offset's + before −; the corners +++, ++−, +−+,
+−−, −++, −+−, −−+, −−−), the four weighted groups added in double left to right, stored as single;
neighbours outside the image by the image's extrapolation (zero by default); voxel sizes doubled;
the sform/qform composed with `diag(2, 2, 2, 1)`; the other voxels' initial value (overwritten) is
the background value.

**With weights** (§6.6), an image `I` with weight `W` is filtered as `f(I·B) / f(B)` with `B = [W
≥ 0.01]` (1 where the weight is at least 0.01 and at most the weight's maximum, else 0), the
division only where `f(B) ≠ 0` (0 elsewhere); a weight is filtered as `f(W)·[f(B) ≥ 0.9]`.

**The reference pyramid.** Unless `-noresample`: the reference (weighted when weighting is on) is
blurred to `s_min` and resampled isotropically to `s_min` mm ("1 mm" level); the "2 mm" level is
its halving if `s_min < 1.9`, else the same image; "4 mm" the halving of the 2 mm level if `s_min
< 3.9`, else the 2 mm image; "8 mm" likewise with 7.9. So with 3 mm data (`s_min = 3`) the four
levels have grid spacings 3, 3, 6 and 12 mm. With `-noresample` all four levels are the reference
as read.

**The input** is blurred to 8 mm before the first scale (unless `-noresample`), and at every
`setscale s` it is taken again from its reading (clamped, unblurred) and blurred to `s` (§7.2).

## 6. The costs

### 6.1 The image pair

A cost compares a **reference** (one of the pyramid's levels) with the **input** (the current
blurred input at its own grid), mapping each reference grid point into the input through the
inverse of the trial matrix, exactly as `specs/mcflirt.md` §6.1 (voxel-to-voxel map in double
rounded to single, row ranges, the bounds test `0 ≤ p ≤ n − 1.0001`), interpolating the input
trilinearly (`specs/mcflirt.md` §6.2), with the edge weight of `specs/mcflirt.md` §6.3 (`smooth`
the schedule's current smoothing in mm, §7.3). The matrix the cost sees is the trial matrix times
the initial matrix (`T·M_init`, 4 × 4 product, §12).

The pair's **reference bins** (histogram costs) are computed when the pair is built, from the
reference level's minimum and maximum (`specs/mcflirt.md` appendix B, "Reference bins"), with `B =
trunc(bins / s)` bins at scale `s` (the quotient in single precision).

### 6.2 The cost functions without weights

As `specs/mcflirt.md`: `normcorr` (§6.4, the count `N` quirk included; cost `1 − |r|`; with
smoothing 0 the centred correlation with the true count), `corratio` (cost `Σ n_b var_b / (n
var)`, exactly below; with smoothing 0 the unweighted counts), `woods` (exactly below), `mutualinfo`
and `normmi` (appendix B), `leastsq` (appendix A; with smoothing 0 the unweighted mean). FLIRT's histogram costs use the schedule's fuzzy fraction (default 0.5, §7.3); **with
smoothing 0 and fuzzy fraction 0** the mutual-information costs use plain histograms: each sample
adds 1 to the cell of its reference bin and test bin (`trunc(y·b1 + b0)` clamped to `0..B−1`), and
the entropies are `Σ −p ln p` with `p = n/N` in single precision, the terms for counts `n` up to
`min(10000, trunc(10N/(B+1)))` taken from a table computed once per pair (`−p ln p` in single
precision, stored in double, then added to the single-precision sum in double and rounded), the
rest computed directly; the overlap correction and the empty case as appendix B.

**`corratio` exactly** (spec writer, 2026-11-02; reproduced from this text by
`validation/investigations/flirt-spec-answers/corratio_model.py`, below). For every contributing
grid point (§6.1), in the sampling order (z outer, then y, then x along each row), with its
reference bin `b`, the interpolated input value `y` and its weight `w` (the edge weight of
`specs/mcflirt.md` §6.3; 1 with smoothing 0; with weighting, §6.4's `wt·wr` times the edge
factors):
- **three single-precision running sums per bin, over the whole grid in that order** (one level:
  no row or slice partial sums, unlike `normcorr`): `n_b += w`, `S_b += w·y`, `Q_b += (w·y)·y`,
  each product rounded to single precision before the next operation (`w·y` once, then times `y`);
  no fused multiply-add. With smoothing 0 and no weights the counts are integers and the sums
  `S_b += y`, `Q_b += y·y`: the same values as with `w = 1`;
- the bins are clamped to `0 … B−1` (§6.1), so the extra bin `B` never fills (FLIRT folds it into
  bin `B − 1`, which changes nothing);
- then over the bins in index order, **only the bins with `n_b > 2`** (a weighted count above 2;
  more than two points with smoothing 0) take part, each in this order, all single precision:
  `n += n_b`, `S += S_b`, `Q += Q_b`, `v_b = (Q_b − (S_b·S_b)/n_b) / (n_b − 1)` (`S_b·S_b` rounded,
  then divided, then subtracted; `n_b − 1` in single), `A += v_b·n_b`. **The points of a bin with at
  most two points are left out of everything**: of `n`, `S`, `Q` and so of the total variance, not
  only of `A` (the clean-room observation of 2026-11-02, right; `specs/mcflirt.md` appendix A's
  reading, sparse bins out of `A` only, gives 0.982248 where FSL prints 0.984415 on `real/dof6`'s
  8 mm pair at the identity, observed);
- `A ← A/n` if `n > 0`; `V = (Q − (S·S)/n) / (n − 1)` if `n > 1` (single precision; `n − 1` in
  single); `A ← A/V` if `V > 0`;
- if `n ≤ 1` or `V ≤ 0` (no bin with more than two points, or no variance), `η = 0`; otherwise
  `η = 1 − A`, **computed in double and rounded to single**;
- the cost is `1 − η`, **again computed in double and rounded to single**. The two roundings are
  not an identity: for `A < 0.5` the cost is `1 − η` with `η` the single-precision rounding of
  `1 − A` (to nearest, ties to even), so a multiple of 2⁻²⁴, up to 3·10⁻⁸ from `A`; for `A ≥ 0.5`
  both steps are exact and the cost is `A`; this is what decides the last bits of the
  costs a parabolic step sees (spec question 3: the 9th and 10th evaluations of `real/dof6`'s
  `-verbose 7` trace). (Model, observed: with this text all 23 evaluations of the search's first
  sweep at 8 mm on `real/dof6` print FSL's costs and parameters; returning `A` itself parts from
  FSL's trace at the 5th evaluation, double-precision bin sums at the 2nd.)

**`woods` exactly** (spec writer, 2026-11-02; the same model). Always the unsmoothed, unweighted
form, whatever the smoothing (with weights FLIRT prints its warning and uses it, §6.4): per bin, in
the sampling order, the integer count `n_b` and the single-precision running sums `S_b += y`,
`Q_b += y·y` (`y·y` rounded). Then over the bins `b = 0 … B` in index order (bin `B` is always
empty), only the bins with `n_b > 2`, all single precision (`n_b` converted to single): `n += n_b`
(integer); `v_b = (Q_b − (S_b·S_b)/n_b) / (n_b − 1)`; `σ_b = √v_b` if `v_b > 0`, else 0 (correctly
rounded); the term `t_b = ((n_b·n_b)·σ_b) / S_b` if `S_b > 0`, else `(n_b·n_b)·σ_b`; `W += t_b`.
The cost is `W/n` (single) if `n > 0`, else `1e10`. For a bin with a positive sum `t_b = n_b·σ_b/μ_b`
(`μ_b = S_b/n_b`): Woods' criterion weighted by the bins' counts (FSL-18: not a defect), so the
clean-room observation `Σ n_b·σ_b/μ_b / n` with `σ_b` from `n_b − 1` is right for those bins and
`specs/mcflirt.md` appendix A's `n_b²·σ_b/μ_b` was not; **a bin whose sum is not positive adds
`n_b²·σ_b`**, not `n_b·σ_b` (observed: on the suite's BOLD reference minus 30000, FSL's `woods` at
the identity is 37174512, which `n_b·σ_b` would make 20844; the model gives FSL's six printed
`corratio` and `woods` values on both images).

### 6.3 `labeldiff`

For label images. Each contributing grid point (as §6.1) with reference value `x` and the input's
eight cell values `v` around the sample (or the pad value eight times outside `[0, n−1)` on any
axis): the mismatch is the sum, over the cell corners in the order 000, 001, 011, 010, 110, 100,
101, 111 (xyz offsets), of the corner's trilinear weight (`(1−dx)(1−dy)(1−dz)` and so on, in
double, `dx = x − trunc(x)` in single) where `|v − x| > 0.5`, each weight added in double to the
single-precision running sum and rounded. The cost is the mean mismatch over the points (the count
exact) without smoothing; with smoothing the edge-weighted mean (`Σ w·mismatch / Σ w`, sums per
row, per slice, then over slices, each reset after it is added: no `N` quirk). With at most one
point (or a weight sum ≤ 1), `(max − min)²` over both images.

### 6.4 Weighted costs (`-refweight`, `-inweight`)

With weighting on, every cost uses the **fully weighted** form: each point's weight starts from
`wt·wr` (the input weight interpolated trilinearly at the sample with the cost's interpolation
rule, times the reference weight at the grid point, single precision), then the edge factors of
`specs/mcflirt.md` §6.3 multiply it, whatever the smoothing (smoothing 0 gives no edge factors).
The sums are those of the smoothed forms. Differences from §6.2:
- `normcorr`: the cost is `1 − r`, **not** `1 − |r|` (an anticorrelation is not rewarded);
- `woods`: prints `WARNING: Woods is not implemented with cost function weighting` on standard
  error at every evaluation and uses the unweighted `woods`;
- the mutual-information costs always take the weighted fuzzy form.
The weight volumes follow the pyramid (§5): the reference weight's level matches the reference
level; the input weight is blurred with the input (§7.2).

### 6.5 The boundary-based cost (`bbr`)

**The boundary points**, computed the first time the cost is BBR for a pair (and again for later
pairs):
- *From `-wmseg`*: a voxel `(x, y, z)` of the segmentation is a boundary voxel when its value is
  above 0.5 and the 3×3×3 box sum of the segmentation there (the separable convolution of §5 with
  taps `[1, 1, 1]`) is below 26.5. Boundary voxels are taken in z, y, x order. Its point is the
  voxel's position in the segmentation's scaled mm (index times voxel size, 4 × 4 times 4-vector
  product); its normal `n` is minus the 3×3×3 gradient of the segmentation smoothed by a Gaussian of
  σ = 2 mm (both exactly below; (spec writer, 2026-11-02) not `specs/bet.md`'s, which has none),
  **each component divided by the voxel size of the current reference level** (not the
  segmentation's; FSL-67), normalised by `√(nx² + ny² + nz²)` in single
  precision. The pair's two points are `p + 2n` (the grey-matter side) and `p − 2n` (white
  matter), each coordinate `p_k ± n_k·2` with `n_k·2` in single and the sum in double, stored as
  single. No boundary voxel: `ERROR::set_bbr_seg: could not find any boundary points!` on standard
  error (at each evaluation, which then is NaN, §6.5 below).

  **The smoothing, the gradient and the normals exactly** (spec writer, 2026-11-02; with this text
  `validation/investigations/flirt-spec-answers/bbr_model.py` reproduces FSL's printed cost of the
  initial matrix of `bbr/default`, 0.537036, 0.540203, 0.566266, 0.554291 with `bbrstep` 1, 2, 200
  and 201, and the same four costs with `-noclamp`, `-bbrtype local_abs` and `global_abs`: 16 of 16
  values, observed):
  - *The Gaussian.* Per axis `σ_k = 2/d_k` voxels (a single-precision quotient, `d_k` the
    segmentation's voxel size as read, after the base scale), a kernel of **radius**
    `r_k = 2·trunc(σ_k − 0.001) + 3` (the difference in double), so `2r_k + 1` taps
    `g_j = exp(−j²/(2σ_k²))`, `j = −r_k … r_k` (`j²` an integer; the denominator `(2·σ_k)·σ_k` in
    double; glibc's `exp`), each rounded to single and summed in single precision in `j` order; the
    taps applied are `g_j·(1/Σ)` in double (`1/Σ` a double quotient). This is a Gaussian of standard
    deviation `σ_k` voxels (§5's blur, with its `4σ²`, is √2 wider): the suite's 1.7 × 1.67 × 1.67 mm
    segmentation has `σ` = 1.18, 1.2, 1.2 voxels, radius 5, 11 taps per axis.
  - *Its convolution:* separable, along x, then y, then z, each pass as §5's (each output value the
    single-precision running sum, in tap order from `−r_k` to `+r_k`, of the double products of value
    and tap; values outside the image count 0). The box sum of the boundary test is the same
    convolution with the taps `[1, 1, 1]`.
  - *The gradient:* at every voxel, three components, each a single-precision running sum over the
    27 neighbours (offsets `(dx, dy, dz)`, `dz` outer, then `dy`, then `dx`, each from −1 to +1) of
    `v·m`, the smoothed value times the mask weight rounded to single (no fused multiply-add);
    neighbours outside the image count 0. The x component's weight is `dx·3^(1 − |dy| − |dz|)`
    (computed in double, stored in single): ±3 for the two neighbours along x, ±1 for the eight that
    also move along one other axis, ±1/3 (0.33333334) for the eight corners, 0 when `dx = 0`; the y
    and z components the same with the axes exchanged (`dy·3^(1 − |dx| − |dz|)`,
    `dz·3^(1 − |dx| − |dy|)`). Not a `[1, 2, 1]` smoothing across a central difference.
  - *The normal:* `n_k = −G_k/d^ref_k` (single), `|n| = √((n_x·n_x + n_y·n_y) + n_z·n_z)` in single
    precision, then `n_k/|n|` (single). A zero gradient (a perfectly symmetric neighbourhood) makes
    the pair's points NaN, and the cost NaN with the warnings below. `d^ref` is the voxel size of the
    pair's reference level when its cost first becomes BBR: with `bbr.sch`'s `setscale 1 force` the
    "1 mm" level, isotropic at `s_min` (3 mm on the suite's images), where the division changes no
    direction; it matters only with an anisotropic level (`-noresample`).
  - *The order:* boundary voxels with z outer, then y, then x (the stored order of the points, and
    so the order of the cost's terms and of `bbrstep`'s subsampling).
- *From `-wmcoords`/`-wmnorms`*: rows of `x y z` (mm) and normals; the points `c ± n·2` in double,
  stored as single.

**The cost of a matrix `A`** (reference mm → input mm through `A⁻¹`): with `T = S_in⁻¹ A⁻¹` (input
voxel sizes `S_in`; computed as the solution of `S_in·T = A⁻¹`, LU with partial pivoting, after
`A⁻¹` by LU), for every pair `k = 0, step, 2·step, …` (`step` from `setoption bbrstep`, default 1):
`g = I(T·p_gm)`, `w = I(T·p_wm)` (4 × 4 times 4-vector products in double, the input interpolated
at the single-precision coordinates by its own interpolation: trilinear with zero outside;
(spec writer, 2026-11-02) each of the eight neighbours outside the grid counts 0 on its own, so a
point less than one voxel outside still takes the neighbours inside),
`q = 200(g − w)/(g + w)` in double (0 when `|g + w| ≤ 1e-6`), `u = 1 + tanh(slope·(q − offset))`
(the slope, default −0.5, and the offset 0 single, converted to double; glibc's `tanh`). With
`-bbrtype local_abs` each term is `min(u, 2 − u)` (in double, before the weight multiplies it).
(spec writer, 2026-11-02) This is FLIRT's rule (spec question 5: the clean-room observation was
right): the model reproduces FSL's `local_abs` costs of `bbr/default`'s initial matrix, 0.080241,
0.080231, 0.090327 and 0.084639 with `bbrstep` 1, 2, 200 and 201, with the normals above; the
clean-room original's 1.4 mm on `bbr/type-local_abs` come from its normals, amplified by this cost,
which rewards a contrast of either sign and is flat where the default is not (FSL's own band there is
0.90 mm against 0.015 mm for the default). The cost is `Σ u / Σ 1` (double sums in pair order; with weights,
each term times `w12 = (wt(T p_gm)·wr(V p_gm))·(wt(T p_wm)·wr(V p_wm))`, the products of single
interpolated weights, in double, `V = S_ref⁻¹`, and the denominator `Σ w12`), then with
`global_abs` `min(c, 2 − c)`; returned as single. A NaN sum prints `WARNING:: Found NaN in BBR
cost` once per remaining pair and `WARNING:: Found NaN in BBR cost (2)` at the end.

**With a field map** (`-pedir d`, `|d| ∈ {1, 2, 3}`, and `-fieldmap`): the reference point is also
converted to reference voxels `r = S_ref⁻¹ p`; if the mask at `r` (trilinear) is below 0.95, the map
value is taken from the nearest point along the phase-encoding direction where the mask exceeds
0.95 (stepping 0, 1, 2, … voxels, at each distance first backwards then forwards, up to the map's
largest extent in mm; 0 if none), else the map at `r`; the input coordinate along axis `|d|` is
shifted by `σ·fmap`, with `σ = echo spacing × N_d / (2π)` (single precision; `N_d` the input's size
along that axis; negative for negative `d`). The phase-encoding direction in reference voxels is
`S_ref⁻¹ A (0, 1, 0, 1)ᵀ` divided by its 4-vector norm. The final image (§10.2) is then resampled
through the same mapping. (FLIRT reads the field map only in registration runs: `-pedir` with
`-applyxfm`, or without `-fieldmap`, is undefined in FSL; larmorx refuses it with a message.)

## 7. The schedule

### 7.1 The language

A schedule is a list of lines (a file read line by line, the empty piece after the last newline
included; or the default, §7.4). Each line is split into words at blanks and tabs, as follows:
leading blanks and tabs removed; trailing ones removed **unless the last non-blank character is the
line's first** (so `"U  "` keeps its blanks); the first word up to the next blank or tab; repeat on
the rest. Consequences (observed): a line of blanks gives one empty word (an unrecognised command);
`clear U  ` gives `clear`, `U` and an empty word. A line whose first word starts with `#` is a
comment; a line with no words does nothing.

**Matrix variables**: 27 user lists `U` (also `U` followed by a digit) and `UA` … `UZ`, and `S`, `P`
(the search's results and starting points). Each holds rows of 17 numbers: a cost, then a 4 × 4
matrix column by column. A name may carry rows: `NAME:r1-r2`, `NAME:r` (`r1 = r2 = r`), default
`1-999999` (rows clipped to the list's length; `r1` at least 1, `r2` at least `r1`; row numbers
read as scalars, below). Any other name (`Cannot interpret <x> as a Matrix name`, or `Can only
accept UA to UZ, not <x>` for `U` followed by anything but a capital or a digit) is undefined in
FSL (it uses an uninitialised list); larmorx prints FSL's message and refuses (status 1). `U[` names
a 28th list (undefined); refused.

**Scalars**: `MAXDOF` (the `-dof` value), `MINSAMPLING` (the working minimum sampling), a word
starting with a letter is 0 (with `Cannot interpret <x> as a scalar variable name` at verbose above
20), anything else `atof`. Integer scalars are the value rounded half away from zero (`(int)(x ±
0.5)` in double).

**Commands** (the first word; anything else prints `Unrecognised command <w>` and ` ... ignoring`
and continues). "Wrong number" below prints `Wrong number of args to <COMMAND>` and ends with
status 255.

| Command | Arguments | Effect |
|---|---|---|
| `clear` | a list | empties it (≥ 2 words) |
| `copy` | src[:rows] dest | appends the rows of src to dest; when src is dest, the length is re-read after each row, so the list keeps appending its own rows until row r2 (≥ 3 words) |
| `print` | list[:rows] | prints the rows (fixed notation, §2.5) |
| `save` | list[:rows] file | writes the rows (fixed, one blank after each number) and an empty line |
| `read` | list file | the list replaced by the file's rows of 17 numbers (C++ stream input; a row is kept only if the end of file was not reached while reading it) |
| `printparams` | list[:rows] | each row's cost, a blank, its 12 parameters (§3, centre §5.4) as a fixed row, an empty line |
| `saveparams` | list[:rows] file | the same into a file (its first cost in `%g`, later ones fixed), an empty line at the end |
| `readparams` | list file | rows of a cost (single precision) and 12 parameters, composed into matrices |
| `sort` | list | sorted by cost, ascending (an unstable introsort: rows of equal cost keep the order libstdc++'s `std::sort` gives them) |
| `dualsort` | list1 list2 | both sorted by list1's costs, rows kept in pairs; different lengths print `Cannot dual sort matrices of unequal size` (no newline) and do nothing |
| `search` | [dof] | the search (§8.1) with the given dof (default 12); appends its results to `S` and their starting points to `P`, sorted by cost (> 2 words: wrong number) |
| `optimise` | dof list[:rows] p1 … [rel\|abs] [maxit] | for each row: perturb, optimise (§8.2), append the result to `U` (≥ 5 words) |
| `measurecost` | dof list[:rows] p1 … [rel\|abs] | for each row: perturb, append the cost of the matrix to `U` (≥ 5 words) |
| `gridmeasurecost` | dof list[:rows] (lo step hi)… [abs\|rel] | for each row, the costs on a grid of perturbations (§8.4) appended to `U` (≥ 7 words) |
| `aligncog` | list[:rows] | each row's matrix with its translation replaced so that the input's centre of mass maps onto the reference's (`t = c_ref − L·c_in`, `L` the linear part); appended to `U` |
| `alignpaxes` | list[:rows] | for each row, the principal-axes alignment of the two images (§8.7; the row's matrix is not used), appended to `U` with the row's cost (< 2 words: wrong number) |
| `setscale` | s [force] | §7.2 |
| `setrow` | list p1 … p16 | appends the matrix whose **columns** are p1-p4, p5-p8, ... (the order in which rows store a matrix), cost 0 (≥ 18 words) |
| `setrowqsform` | list | appends the sform/qform initialisation (§8.5) relative to the initial matrix, cost 0 |
| `setrowparams` | list p1 … p12 | appends the matrix of 12 parameters (§3), cost 0 (≥ 14 words) |
| `setoption` | name values | §7.3 (≥ 3 words) |
| `if` | a op b | skips the next line unless `a op b` (`> < == != <= >=`, single precision); another operator prints `Cannot recognise operator <op> in IF statement` (no extra newline beyond its own) and ends with status 255 (≥ 4 words) |
| `exit` | | ends the program immediately, status 0, nothing written |

**Perturbations** (`optimise`, `measurecost`): up to 12 numbers before `rel`/`abs` (more is an
error of FSL: an index out of bounds, status 0 with its message); default `rel`. The row's matrix is
decomposed (§3), each parameter `i` increased by `p_i·δ_i` (`rel`) or `p_i` (`abs`), and composed
again. The sizes `δ`: rotations half the fine search step of each axis if that axis has more than
one fine angle (§8.1), else 3 times the rotation tolerance; translations 0.05; scales 0.1 when
`-dof ≥ 7`, else 0; skews 0.05. The dof used is `min(-dof, the command's dof)`.

### 7.2 `setscale s [force]`

If `s > 0` it becomes the requested scale (tolerances scale with it, §8.2). The input is taken
again as first read (clamped, its weight as first read). Then, if `force`, or `-forcescaling`, or
the working minimum sampling is at most `1.25·s`:
- the input is blurred to `s` (weighted when weighting is on), and its weight filtered likewise;
- the reference level is the pyramid's 8, 4, 2 or 1 mm level when `|s − 8|`, `|s − 4|`, `|s − 2|`
  or `|s − 1|` is below 0.01 (1 mm only while no other scale has replaced it); for any other
  scale the reference (and its weight, its field map, its segmentation, as in §4.1) is read
  again, blurred and resampled isotropically to `s` mm, and **this replaces the 1 mm level** for
  the rest of the run;
- a new image pair (§6.1, built with the new input, so its centre of mass is the new input's) with
  `trunc(bins/s)` bins, the current cost, smoothing and fuzzy fraction.

Otherwise (the scale is too fine for the data) **only the input is reset**: the existing pair now
compares its previous reference level with the clamped, unblurred input (observed: with 3 mm data
the default schedule's `setscale 2` leaves the 4 mm pair in place with the unblurred input).
(spec writer, 2026-11-02; observed by the implementer, confirmed) The pair keeps everything else:
its reference level, bins, cost, smoothing, fuzzy fraction and **its centre** (the centre of mass of
the input it was built with: the rotations stay about it); the requested scale, and so the
tolerances, is the new one. A later `search` (§8.1 step 3) or `aligncog` measures the centre of
mass of the input the pair now holds (the unblurred input) afresh.

### 7.3 `setoption name values`

The values are scalars (§7.1). `smoothing s` (the edge-weight width in mm, also the current pair's),
`fuzzyfraction f` (also the current pair's), `optimisationtype t` (`powell` for Powell's method,
anything else line searches only, §8.2), `costfunction c` (the current, main and search cost;
unknown: `Unrecognised cost function: <c>`, status 255), `tolerance t1 … t12` (the tolerances
divided by the requested scale, so that they hold at this scale; fewer than 12 values leave a short
list: an index error when used), `rescaletolerance k` (one value: all tolerances times `k`; more
values: a size error, status 0), `boundguess b1 …` (the line search's first steps per iteration,
§8.2), `nosubset`, `paramsubset n m1 … m12n` (the parameter subset of §8.3: `n` columns of 12;
fewer values print `Must specify at least <12n> for a <n> parameter subset mask`), `minsampling s`,
`bbrstep k` (the BBR pair step, truncated to an integer; larmorx refuses a step below 1, which never
ends in FSL). Too few words: `Must specify an option and a value`. Anything else: `Option <name>
is unrecognised - ignoring`.

### 7.4 The default schedules and `bbr.sch`

**The first pair** (spec writer, 2026-11-02; observed by the implementer, confirmed). Before the
first line of any schedule FLIRT builds an image pair from the 8 mm reference level (§5; with
`-noresample` the reference as read) and the input blurred to 8 mm (with `-noresample` as read;
the weights filtered likewise), with `trunc(bins/8)` bins, the main cost (`-cost`), the command
line's smoothing and fuzzy fraction, its centre the centre of mass of that input. The requested
scale is 1 until a `setscale`, so the tolerances (§8.2) are the unscaled ones. A schedule with no
`setscale` (FSL's `measurecost1.sch`, `pairreg2.sch`, `pairreg3.sch`) runs on this pair.

The 3D default (no `-schedule`, no `-2D`):
1. **8 mm**: smoothing 8; the search (§8.1) with dof 12 into `S` and `P`.
2. **4 mm**: smoothing 4; the costs of `S` and of `P` (dof 7) into `US` and `UP`, sorted together
   by `US`'s costs; optimise (dof 7, 4 iterations) the first 3 of `US` and of `UP` and the identity;
   sort; the best 4 (`UA`) optimised again from 10 perturbations each (rx ±1, ry ±1, rz ±1
   relative; the common scale ±0.1 and ±0.2 absolute), sorted into `UB`.
3. **2 mm**: smoothing 2; `UB`'s costs, sorted (`UC`); the best optimised at dof 7 (4 iterations)
   → `UD`; bound guess 1; if `-dof > 7` the best of `UD` optimised at dof 9 (1 iteration) → `UE`; if
   `-dof > 9`, at dof 12 (2 iterations); sorted → `UF`.
4. **1 mm**: smoothing 1, bound guess 1; the sform/qform initialisation appended to `UF`; its first
   two rows optimised at dof 12 (1 iteration); the sform/qform matrix itself measured as a final
   candidate; sorted.
The result is the first row of `U` (§9.1). With `-dof 6`, `MAXDOF` is 6, so step 3's dof-9 and
dof-12 optimisations are skipped, and the dof of every `optimise 7/9/12` is capped at 6 (§7.1).

The 2D default (`-2D`), every scale with the parameter subset `rz, tx, ty` (three unit columns on
parameters 3, 4, 5) and no search:
1. **8 mm**, smoothing 8: the identity optimised (dof 12 in the subset, 4 iterations) → `UA`.
2. **4 mm**, smoothing 4: `UA`'s cost (dof 12), sorted → `UL`; its best 3 optimised (4 iterations),
   and the identity at dof 7 (4); sorted → `UM`; its best 4 again from rz ±1 (relative, 4
   iterations each); sorted → `UB`.
3. **2 mm**, smoothing 2: `UB`'s costs, sorted → `UC`; the best optimised (4) → `UD`; bound guess 1;
   if `MAXDOF > 7`, `UD`'s best at dof 9 (1) → `UE`; if `MAXDOF > 9`, at dof 12 (2); sorted → `UF`.
4. **1 mm**, smoothing 1, bound guess 1: the sform/qform initialisation appended to `UF`; its first
   two rows optimised (dof 12, 1 iteration); sorted.
Since the subset is active throughout, every `optimise` takes the subset form (§8.3), whatever its
dof. The final padding is at least 1 voxel (§9.2).

`bbr.sch` (fMRIPrep's): `setscale 1 force`; cost `bbr`; Brent; tolerances 0.0005 rad and 0.02 mm (and
0.002, 0.001 for scales and skews); bound guess 1; `bbrstep 200`; the sform/qform matrix and the
identity (relative to `-init`, §8.5) measured, and a grid of perturbations of both (±0.07 rad in
three steps per rotation, ±4 mm per translation: 3⁶ = 729 matrices each); the best optimised (dof
6, 8 iterations), then Powell from it (8), then the second best (Brent, 4); sorted; tolerance 0.0002
rad; `bbrstep 2`; a fine grid (±0.0017 rad, ±0.1 mm) around the best; the best optimised (Brent, 8),
then dof 12 Powell (8) and dof 12 Brent (4) (capped at `-dof`); sorted; printed (`print U:1`, which
fMRIPrep's log shows as one row of 17 numbers). FSL's other shipped schedules (`simple3D.sch`,
`xyztrans.sch`, `ytransonly.sch`, `ztransonly.sch`, `sch3Dtrans_3dof`, `sch2D_3dof` to `6dof`,
`measurecost1.sch`, `pairreg1-3.sch`) use only the commands above; the parity suite runs each.

## 8. Searching and optimising

### 8.1 The search

1. The rotation samples per axis: `m = round((max − min)/δ) + 1` points (double quotient, half away
   from zero), at `min + (k − 1)·(max − min)/(m − 1)` computed in single precision from the single
   ends (`(max + min)/2` when `m = 1`), for the coarse step (60°) and the fine step (18°): with the
   defaults 4 coarse angles (−90°, −30°, 30°, 90°) and 11 fine angles per axis.
2. The search's parameter subset: with dof > 6, a common scale and three translations (the scale
   column has three ones, on `sx, sy, sz`); else the three translations. (spec writer, 2026-11-02;
   observed by the implementer, confirmed) The dof of steps 2 to 8 is the `search` command's own
   (12 when it has none), not `-dof`: with `-dof 6` the coarse grid still optimises the common
   scale and the fine grid keeps the median scale; `-dof` enters step 9 only. The grids'
   tolerances are §8.3's for this subset with `x₀` the reference parameters in force when the
   search starts (the identity's `(0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0)`, unless an earlier search or
   a subset `optimise` left others), computed once before step 3; all the search's optimisations
   use them.
3. The reference parameters start at the translation that aligns the centres of mass (`c_ref −
   M_init·c_in`, both in scaled mm, `M_init` applied as a 4 × 4 times 4-vector product), unit scales.
   (spec writer, 2026-11-02) `c_ref` is the centre of the pair's reference level and `c_in` of the
   pair's input (blurred to the scale), the same as the rotations' centre; every centre of mass of
   FLIRT (and `specs/mcflirt.md` §6.1's) is computed with `v − v_min` **in single precision**, then
   converted to double, and double running sums `Σ(v − v_min)·i` (per axis) and `Σ(v − v_min)` over
   the voxels in z, y, x order, flushed into the totals after every `max(1000, trunc(√N)) + 1`
   voxels (`N` the number of voxels; each block's sums restart at 0 and are added to the totals,
   which are added to the last block's at the end); each coordinate is its total over the total
   (a total below 1e-5 in magnitude counts 1, with `WARNING::in calculating COG, total = 0.0` on
   standard error), times the voxel size. (With
   it the model gives the search's printed reference translations on `real/dof6`, -15.257971
   13.083505 76.451790; with the subtraction in double, 13.083506.) (spec writer, 2026-11-02) Both
   centres are measured afresh at every search (their warnings printed again, observed by the
   implementer); after a `setscale` that only reset the input (§7.2) `c_in` is the unblurred
   input's, no longer the rotations' centre.
4. **Coarse grid**: for every coarse `(rx, ry, rz)` (x outer, z inner), the subset optimised (§8.3,
   4 iterations) from the reference parameters with those rotations; the resulting translations
   and scale stored on a small grid. (spec writer, 2026-11-02) The reference parameters `x₀` are
   these parameters (the sampled angles exactly) and the subset starts from `y = 0`, so the first
   evaluation is at them; the grid holds the results in single precision.
5. The scale grid is replaced everywhere by its median (the element at `trunc(0.5·n)` of the sorted
   values).
6. **Fine grid**: for every fine `(rx, ry, rz)`, the translations and scale interpolated
   trilinearly on the coarse grid at `(i·f_x, j·f_y, k·f_z)`, `f = (n_coarse − 1)/max(1, n_fine −
   1)` in single precision (zero outside the grid); a scale outside `[0.5, 2]`, or any scale when
   dof ≤ 6, becomes 1; the cost there (no optimisation; skews 0 only for the first point: later
   points keep the reference parameters' skews, which are 0).
7. The threshold `t = min(c_min + 0.2(c_max − c_min), 20th percentile)`; if `t ≤ c_min`, `t =
   max(1.0001 c_min, 0.9999 c_min)`. The points with cost strictly below `t` are optimised
   (subset, 4 iterations) from their interpolated parameters (skews 0); their costs replace the
   grid's. (spec writer, 2026-11-02) The percentile is the element at `trunc(n·p)` (`n` the number
   of fine points converted to single, `p = 0.2` in single precision, the product in single) of the
   sorted costs, as the median of step 5 with `p = 0.5`; each point's interpolated parameters (the
   sampled angles) become the subset's reference parameters `x₀` and it starts from `y = 0`; the
   points are visited x outer, z inner.
8. **Minima**: the grid points (excluding the last index on each axis) none of whose 26
   neighbours (inside the grid) has a strictly smaller cost; if none, the overall minimum (the
   grid scanned z outer, then y, then x, starting from point (0, 0, 0) and replaced on a strictly
   smaller cost). Their interpolated parameters (skews 0) are the search's starting points.
   (spec writer, 2026-11-02; observed by the implementer, confirmed) The minima are listed z outer,
   then y, then x (`COST minima at : a,b,c`, then `Cost minima at : a,b,c` in the same order).
9. Each starting point, as a matrix of `min(dof, 7)` parameters, is optimised freely (§8.2, dof
   `min(dof, 7)`, 4 iterations, with the search cost); the optimised and starting matrices, with
   their costs, enter a list sorted by the optimised cost, from which a new entry is dropped if its
   RMS deviation (§8.6) from an existing one is below the reference level's smallest voxel size,
   unless it is better, which replaces that one. (spec writer, 2026-11-02) Here `dof` is `-dof`.
   Exactly: the starting point's first `min(dof, 7)` parameters are composed (§3, about the pair's
   centre; with `-dof 6` the median scale is dropped) and **that matrix** is optimised: its
   decomposition (§3) is the start, which gives back the sampled angles to single precision
   (other angles under gimbal lock, `|r_y| = 90°`). With `-anglerep quaternion` this composition
   reads the three Euler angles as a quaternion's vector part (§3): a starting point whose angles'
   squares sum above 1 prints `Parameters do not form a valid axis - greater than unity` on
   standard error and keeps only its translations and scale (observed: FSL's four lines on
   `real/anglerep-quaternion`, one per such minimum of its five; nothing else in the search
   prints it). The cost stored with the starting matrix (in `P`) is the search cost of that matrix,
   evaluated after its optimisation (counted, not printed: observed by the implementer). The
   list: a new entry is compared with the entries in their (sorted) order and the first one within
   the distance decides: a strictly better new entry replaces it (removed, the new one inserted
   after every entry of lower or equal cost), otherwise the new one is dropped.

### 8.2 One optimisation

From a matrix: its 12 parameters (§3, about the pair's centre), the tolerances `t_i·s_req` (default
0.005, 0.005, 0.005 rad, 0.2, 0.2, 0.2 mm, 0.002 × 3, 0.001 × 3, times the requested scale),
optimising the first `dof` parameters (dof clamped to 6..12 with `Erroneous dof <d> : using 6
instead` or `12`). The method is mcflirt's line search (`specs/mcflirt.md` §7.2, §7.3), with these
differences:
- **Iterations.** Up to `maxit` sweeps (the command's last number; default 4). Each sweep performs
  the line searches along the current directions in order, each with the bound guess `b_k` of
  sweep `k` (the last value for later sweeps; default 10 then 1) as its first step multiple
  (`a1 = b_k·u`). A sweep's starting value is the previous line search's best cost (0 means: not
  known, evaluated).
- **Convergence.** After a sweep, `Σ_i |(x_i^start − x_i^end) / t_i| / n_t` (terms with `|t_i| ≤
  1e-15` count 0; `n_t` the number of tolerances, 12; the sum with two interleaved accumulators,
  even and odd indices, then added) below 1 ends the optimisation. (spec writer, 2026-11-02;
  observed by the implementer, confirmed) Exactly: `n_t` is the length of the tolerance list, 12
  for every optimisation of the 12 parameters whatever its dof, and the subset's number of columns
  in the subset form (§8.3: 4, or 3, in the search); the weights `w_i = |1/t_i| / n_t` are computed
  first (double), each term is `|(x_i^start − x_i^end)·w_i|`, and the sum is rounded to single
  precision before the comparison.
- **The tolerance unit of a line search** (spec writer, 2026-11-02). Along a direction `d` (a
  coordinate axis, or Powell's `x_end − x_start`): `û = d/√(d·d)` (double: §12's sum of squares,
  its square root, each component divided), `τ = Σ_i |û_i / t_i|` accumulated in single precision
  over the tolerances with `|t_i| > 1e-15` (each double quotient's magnitude added to the single
  running sum in double, rounded back to single), and the unit `u = |1/τ|` (a single-precision
  quotient); mcflirt's line search (`specs/mcflirt.md` §7.2) runs with this `u`, its trial points
  `p + a·û` (`a` single, the product and the sum double). Along a coordinate axis `u =
  fl(1/fl(1/t_k))`, which differs from `fl(t_k)` for some tolerances (for 29 % of random ones;
  never for the defaults at the scales 8, 4, 2 and 1); along Powell's direction it is one tolerance
  unit in this L1 sense (question 19: the implementer's `1/‖d ⊘ t‖` holds with the L1 norm of the
  unit direction).
- **Powell** (`optimisationtype powell`): after a sweep that did not converge, let `Δ_m` be each
  line search's cost change (new minus start, single), `m*` the most negative, `f0` the sweep's
  first starting cost, `f1` its final cost, `fe` the cost at `x_start + 2(x_end − x_start)`; if
  `2(f0 − 2f1 + fe)(f0 − f1 − |Δ_m*|)² < (f0 − fe)²|Δ_m*|` (single precision, left to right) and
  `fe < f0`, it prints `Applying POWELL correction` and `finit, fend, fextrap = <f0> , <f1> ,
  <fe>`, searches along `x_end − x_start`, prints `fval = <f>`, and replaces direction `m*` by that
  difference.
The result is the matrix composed from the final parameters (first `dof`), and the last cost.

### 8.3 Parameter subsets

With a subset mask `P` (12 × n) and reference parameters `x₀`, the optimised variable is `y` with
`x = P·y + x₀`; the start is `y = pinv(P)·(x_start − x₀)` and the tolerances `pinv(P)·((t + x₀) −
x₀) − pinv(P)·(x₀ − x₀)`. For masks of zeros and ones with at most one non-zero per row (the
search's, and every shipped schedule's), `pinv(P)` has, for a column of `k` ones, the value
`(1/√k)²` (square root and reciprocal correctly rounded, then the product rounded: `0x1.5555555555557p-2`
for `k = 3`, `0x1.ffffffffffffep-2` for `k = 2`) at its rows, zero elsewhere (observed, FSL's
LAPACK); the products `P·y` and `pinv(P)·d` are plain sums of rounded products.

**Every other mask** (several non-zeros in a row, values other than 0 and 1, dependent columns:
any finite 12 × n mask; observed bit for bit, part 3) has Armadillo 14.2.1's pseudo-inverse:
- a **diagonal** mask (non-zeros only at `(i, i)`): `1/p_ii` at `(i, i)` for each
  `|p_ii| ≥ tol`, `tol = 12·max_i |p_ii|·ε` (ε = 2⁻⁵²), zeros elsewhere;
- otherwise the thin SVD `P = U·diag(s)·Vᵀ` (`s` descending) by LAPACK's divide-and-conquer
  driver `dgesdd` with `jobz = 'S'` (reference LAPACK as bundled in OpenBLAS 0.3.33, unblocked
  below 13 columns, on OpenBLAS's Haswell kernels: the transposed matrix-vector product and the
  plane rotation contract into fused multiply-adds, the matrix product accumulates each element
  by fused multiply-adds over the inner index in order, the Euclidean norm sums its squares in
  x87 extended precision); the singular values `s_k ≥ tol = max(12, n)·s₁·ε` are kept (none:
  `pinv(P)` is zero), and `pinv(P) = W·Uᵀ` over them, `W(i, k) = V(i, k)·(1/s_k)` (the reciprocal
  rounded first), the product `W·Uᵀ` as the matrix product above.

LAPACK (modified BSD) and OpenBLAS (BSD-3-Clause) are permissive: the original may port those
routines to reproduce FSL's last bits; any accurate SVD is within §13's thresholds. A mask with a
NaN or infinite value is refused by the original in both modes (rule B: FSL's SVD fails and its
run stops on the empty result). The products `P·y` and `pinv(P)·d` are Armadillo's
matrix-vector products (§12). With `setoption paramsubset` active, `optimise` uses the subset
form with the row's own parameters as `x₀`.

### 8.4 `gridmeasurecost`

For each component given (lo, step, hi, up to 12), `n_c = ceil(max((hi − lo)/step, 1e-6) +
0.000001)` (a NaN quotient counts 1e-6); the grid has `Π round(n_c)` points, visited as an odometer
whose first component runs fastest, **advanced before each point** (so the first point already has
the first component at `lo + step`, and the counter wraps to 0 when it reaches `n_c`); each point
`lo + count·step` per component is a perturbation, relative (times `δ`, §7.1) unless a word `abs`
appears anywhere after the triples. The triples are read while at least three words remain and the
next is neither `rel` nor `abs`.

### 8.5 The sform/qform initialisation

When both images have a sform or qform (codes summed positive), `M_q = S_ref · W_ref⁻¹ · W_in ·
S_in⁻¹` (the four 4 × 4 products in Armadillo's order, §12; `W` the sform if set, else the qform,
else the voxel-size diagonal; `S` the voxel-size matrices), converted to scaled mm (§3); otherwise
the identity. `setrowqsform` appends `M_q · M_init⁻¹` (or the identity with `-usesqform`).
`-usesqform` makes `M_q` the initial matrix. Note that single-slice images lose their sform/qform
(§4.1) and that the input used is the current (blurred) one, whose sform is kept. (spec writer,
2026-11-02; question 15, observed with the model: FSL's `-13.74984741`) `W` is the form as FLIRT
holds it after reading: for a neurological file the swapped one of §4.1, whose single-precision
field of view is the 4.5e-6 mm the implementer found (the double product gives `-13.74984288`);
`W_ref⁻¹` by §12's inverse, `S_in⁻¹` by reciprocals, and the products `((S_ref·W_ref⁻¹)·W_in)·S_in⁻¹`.

### 8.6 The RMS deviation

`√(tᵀt + (80²/5)·tr(AᵀA))` with `D = M1·M2⁻¹ − I`, `A` its 3 × 3 part, `t` its translation (about the
origin), as `specs/mcflirt.md` §10.4.

### 8.7 The principal-axes alignment (`alignpaxes`)

The schedule command `alignpaxes list[:rows]` appends, for each row of the list, the matrix that
carries the input level's principal axes onto the current reference level's, with the row's cost;
the row's own matrix is not used.

**The principal axes of an image** (volume 0, every voxel, x fastest): with `m` the image's
minimum and `w = v − m` per voxel, the double-precision sums `Σw`, `Σw·x`, `Σw·y`, `Σw·z`,
`Σw·x·x`, `Σw·x·y`, `Σw·x·z`, `Σw·y·y`, `Σw·y·z`, `Σw·z·z` (voxel coordinates; each term
`(w·a)·b` from the left), accumulated in partial sums that are added to running totals after every
`max(1000, ⌊√N⌋) + 1` voxels (N the voxel count) and once at the end (the same blocking as the
centre of mass, §6.1). A total below 1e-5 in magnitude prints `WARNING::in calculating Principle
Axes, total = 0.0` on standard error and counts as 1. With `M` the second moments divided by the
total, `μ` the first moments divided by it and `S` the 3 × 3 diagonal of the voxel sizes, the
covariance in millimetres is `C = (S·M)·S − (S·μ)(S·μ)ᵀ`, each of the two steps made symmetric from
its lower triangle. Its eigenvectors (LAPACK `dsyevd`, `jobz = 'V'`, `uplo = 'U'`, eigenvalues
ascending, as §8.3's library) are the columns of `R`, then reordered: the column of the smallest
eigenvalue (the first strictly smaller one, scanning the second and the third) exchanged with the
first, then the second and the third exchanged when the third's eigenvalue is below the second's;
each exchange stores the eigenvalue it moves into the other place rounded to single precision
(which the second comparison reads). A non-finite moment (a NaN or infinite voxel): the
decomposition fails and FSL stops with `Mat::operator(): index out of bounds` (status 0, nothing
written: FSL-68; the original exits 1 by default as §11). (spec writer, 2026-11-02; observed by
the implementer, confirmed) Both images' axes are computed (the reference level's first) before
either is used, and for each image whose covariance is not finite (a NaN or infinite voxel makes
every entry NaN) standard error gets an empty line and `warning: eig_sym(): given matrix is not
symmetric` (the symmetry check before the decomposition compares entries within a tolerance, which
NaN fails); then FLIRT's message.

**The alignment** of the reference level's axes `R_r` and the input level's `R_i` (each computed
as above, the reference's first): for the first and the second column `n`, when the dot product
`R_r(:, n)·R_i(:, n)` (summed in order) is negative, `R_r(:, n)` is negated; then each of the two
matrices whose determinant (§12's closed form) is negative has its third column negated. The
linear part is `L = R_r·R_iᵀ`, the translation `aligncog`'s (§7.1: `t = c_r − L·c_i`, the centres
of mass in scaled millimetres, computed after the axes).

## 9. Outputs

### 9.1 The matrix

The first row of `U` after the schedule, its matrix times the initial matrix, the translation scaled
by the base scale (§3), written to `-omat` (§2.5). If `U` is empty: `Failed to calculate any
transformation matrix` on standard error, nothing written, status 0.

### 9.2 The image (`-out`)

The input and the reference are read again (unscaled, unclamped, single slices **not** tripled); if
the interpolation is not nearest neighbour and `-noresampblur` is not given, the input is blurred to
the reference's smallest voxel size (§5). The interpolation (`-interp`; sinc with a kernel of full
width `round(-sincwidth)` voxels, the window `-sincwindow`, 1201 stored samples, as mcflirt's
`-sinc_final` kernel construction, `specs/mcflirt.md` §9.3) and the edge rule: with
`-setbackground v`, outside values are `v`; otherwise the input's background value (the 10th
percentile of its 2-voxel border, `specs/mcflirt.md` §9.2) and the extraslice rule, **taken from the
blurred input**. The output grid is the reference's (its header kept: sform, qform, dims, datatype
replaced); each voxel is the input interpolated at `M⁻¹` of its position (`specs/mcflirt.md` §9.1's
voxel-to-voxel mapping), then voxels whose pre-image lies more than `-paddingsize` voxels (at least
1 with `-2D`) outside the input are set to the background/pad value. With a field map (§6.5) the
values are resampled through the BBR mapping instead. (spec writer, 2026-11-02; observed by the
implementer, confirmed) Exactly: the pre-image tested is the interpolation's own, in single
precision (`x·a₁₁ + z·a₁₃ + a₁₄` at the start of each line along y, then `a₁₂` added at each step;
the same for the other two coordinates; `a` the voxel-to-voxel matrix rounded to single), and a
voxel keeps its interpolated value when every coordinate lies in `[−p, n − 1 + p]`; within `p`
voxels outside the input the value is therefore interpolated with the neighbours outside read by
the edge rule: the extraslice rule by default (one voxel beyond an edge reads the edge voxel,
further ones the background value), the constant `v` with `-setbackground v`. With a field map
there is no padding test: every voxel is interpolated (the edge rule outside the input).

**The windowed sinc exactly** (spec writer, 2026-11-02; question 13, observed with the model: 40000
of 40000 sampled voxels of `-applyxfm -interp sinc` bit for bit; the two sums in double, the weight
as `w_x·(w_y·w_z)` or the table's position in double each part from FSL in 38 to 91 % of them).
The table (full width `w`, half-width `h = (w − 1)/2`): 1201 samples, `m = 600` in single
precision; sample `k = 1 … 1201` at `x = ((k − m − 1)/m)·h` (single precision, left to right); the
sinc `1 − |x|` when `|x| < 1e-7`, else `sin(y)/y` with `y = π·x` rounded to single and the
single-precision sine (glibc's) and quotient; the window in double, rounded to single (Hanning
`0.5 + 0.5·cos(π·x/h)`, Blackman `0.42 + 0.5·cos(π·x/h) + 0.08·cos(2π·x/h)`, rectangular 1; 0 for
`|x| > h`; glibc's `cos`); the sample is the single product of the sinc and the window. The table
read at `t` (single): 0 if `|t| > h`; `q = (t/h)·m + m` in single, plus 1 in double, rounded to
single; `n = ⌊q⌋`, `f = q − n` (single); 0 if `n < 1` or `n > 1200`; otherwise `k_n·(1 − f) +
k_{n+1}·f` in double, rounded to single. At a position `p` (single), with `i₀ = ⌊p⌋` per axis: each
axis' `2h + 1` weights, the neighbour `i₀ + e` weighted by the table at `(p − i₀) − e` (the
difference in single precision, then `−e` added in single); over the neighbours inside the grid,
z outer from `i₀ − h`, then y, then x, the weight `(w_x·w_y)·w_z` in single, and the single
running sums `S += v·w` and `W += w`; the value `S/W` (single) when `|W| > 1e-9`, otherwise the
edge rule's value at `i₀`. **Data type**: the input's (§4.1), except that
a non-float type becomes float when the output's values span less than 1.5 (unless `-datatype`);
conversion to integers as mcflirt's FSL-09 rule (`specs/mcflirt.md` §9.4: truncation; larmorx's
default rounds and clamps). `descrip` is the writer's; units mm and s.

### 9.3 `-applyxfm` and `-applyisoxfm`

No optimisation: base scale 1; the reference read (a 4D reference cut to its first volume with the
warning); the input read as 4D; the initial matrix (`-init`, or the sform/qform with `-usesqform`,
else identity) is the result. With `-applyisoxfm s` the reference is first blurred and resampled
isotropically to `s` mm (§5). For each volume `t` of the input, in order: blurred to the reference's
smallest voxel size (unless nearest neighbour or `-noresampblur`), resampled as §9.2 — with the
**pad value taken from the unblurred first volume**, once — into volume `t` of the output; the output
has the input's TR. `-omat` writes the initial matrix. The program ends with status 0 after this.

(spec writer, 2026-11-02; question 10, observed by the implementer, confirmed) The image is
written before the matrix: an image FSL cannot write (an output type it does not know) ends the
run before `-omat` is written. (After a registration it is the other way round: `-omat`, §9.1,
then the image.)

(spec writer, 2026-11-02; question 14, observed with the model: FSL's 172800 values of
`real/applyisoxfm-4d-blur-setbackground` bit for bit; a blur reading zeros parts in 43441 of them,
edge coefficients repeated in 5654) **The edge rule comes before the blur here**: the input's edge rule (the constant `v` with `-setbackground v`,
else the extraslice rule with the first volume's background value) is set before each volume is
blurred, so the blur reads the neighbours beyond the image by that rule (`v`, or the edge voxel one
voxel out and the background value further), not zeros as after a registration (§9.2, where the
blur comes first and reads zeros). With `-interp spline` and `-setbackground v` the spline's
coefficients are computed as for the extraslice rule (`specs/mcflirt.md` §9.3's prefilter, its
mirror-symmetric initialisation), but at evaluation the coefficients outside the grid count 0 (the
extraslice rule repeats the edge coefficient instead), and a position outside `[0, n − 1]` on any
axis gives `v`. The spline exactly: the coefficients along x, then y, then z (axes of one voxel
skipped), each column in double and the results stored in single precision after each axis; pole
`z = √3 − 2`; the causal start `c₀ + Σ_{i=1}^{N−1} zⁱ·cᵢ` with `N = trunc(ln(1e-8)/ln|z| + 1.5)`
(15) capped at the column's length, the powers by repeated products; the causal sweep `cᵢ += z·cᵢ₋₁`;
the anti-causal start `c_last = (−z/(1 − z²))·(2·c_last − l)` with `l` the column's last value
before the causal sweep; the anti-causal sweep `cᵢ = z·(cᵢ₊₁ − cᵢ)` downwards; every coefficient times
6. At a position `p`, the start index `⌈p⌉ − 2` per axis and the four cubic weights of the distances
`p − (s + i)` (`2/3 + ((0.5·a)·a)·(a − 2)` for `a < 1`, `(1/6)·((b·b)·b)` with `b = 2 − a` for `a < 2`,
in double); the value `Σ (c·w_x)·(w_z·w_y)` in double, z outer, then y, then x, rounded to single.
The input's values are those read (a scaled integer file: `raw·slope` then `+ intercept`, in
single precision).

(spec writer, 2026-11-02; question 9, observed with the model: FSL's `quatern_b`, `quatern_c`,
`quatern_d`, offsets and sform rows on a neurological reference with an oblique qform and no
sform) **`-applyisoxfm`'s header.** The resampling (§5) composes a set sform or qform (code ≠ 0)
with `diag(st_x, st_y, st_z, 1)` (`st = s/d`, single precision) and keeps an unset one as read
(for a neurological file: swapped by the read grid, §4.1); writing swaps both back with the new
grid's `n_x` and `s` (§4.1). So an unset sform keeps its rows but for its x translation, which
moves by the two fields of view (observed by the implementer: 57.75 became 64.75 for 16 voxels of
0.5 mm resampled to 3.5 mm). The written qform: every element of the matrix rounded to single
precision first, then the quaternion of §3's conversion (the NIfTI-1 reference library's, x87
operations included), its fields stored in single precision.

### 9.4 `-debug`

With `-debug`, once the images are prepared (the reference pyramid, its weights, the input at
8 mm), FLIRT saves in single precision, in the output type of `FSLOUTPUTTYPE`, in the working
directory: `refvol_8`, `refvol_4`, `refvol_2`, `refvol` (the reference's levels), `global_refweight1`,
`global_refweight2`, `global_refweight4`, `global_refweight8` (the reference weight's levels:
without `-refweight` the first is a 1 × 1 × 1 volume holding 0 with the base scale's voxel size,
the filtering of an empty weight, and the others are FSL's empty volumes), `testvol` and
`testweight` (observed). After the schedule, on standard error, `Final DEBUG call in FLIRT`,
`Pointer to impair is <the cost object's address>` (a run-dependent address; the original prints
any non-null value), `Final DEBUG call in FLIRT 2`, `Scale is:<the requested scale>`, then one cost
evaluation of the matrix (as the schedule's `measurecost 12 U:1 0 0 0 0 0 0 0 rel 8`, its result
appended to `U`), `Final DEBUG call in FLIRT 3` and `Final DEBUG call in FLIRT 4`. With the
boundary-based cost (§6.5) that evaluation also writes `debug`: two volumes on the reference
level's grid (its header), zero, where each pair of points of the boundary (the two sides' points
in reference voxels) writes, at the rounded midpoint of the two points, `w·u + (1 − w)` in volume 0
and `w` in volume 1 (`w` the pair's weight, `u` its cost term, both in single precision; later
pairs over earlier ones); a midpoint outside the grid is written where the reference's
extrapolation would read it: nowhere for zero and constant padding, the nearest edge voxel when it
is one voxel outside with extraslice (nowhere beyond), the mirrored or periodic voxel otherwise.

## 10. `convert_xfm`

`convert_xfm [options] <input-matrix>`: `-omat <file>`, `-concat <second>` (result `second · input`),
`-fixscaleskew <second>` (the input's rotations and translations with the second's scales and skews:
both decomposed about the origin, combined, composed with 12 parameters), `-inverse` (applied last),
`-v` (verbose 5: prints `Initial matrix:`, the matrix, an empty line, `Second matrix:` or `Fix
Scale-Skew matrix:`, the matrix, an empty line, before each operation), `-verbose n`, `-help`. The
result is written as §2.5 (`-omat`) and printed (fixed, then an empty line) when verbose is above 0.
Errors: no arguments (usage on standard output, status 1); `Unknown option <w>` and a blank line for
a second non-option word (status 2); `Lacking argument to option <o>` and a blank line (2);
`Unrecognised option <o>` and a blank line (255); `Input matrix filename not found` and a blank
line (2); a matrix that cannot be opened prints `Could not open matrix file <f>`; fewer than 4 rows:
`Cannot read input-matrix` (status 254), `Cannot read fixscaleskew-matrix` or `Cannot read
concat-matrix` (253); the inverse of a singular matrix aborts (`inv(): matrix is singular`, signal 6).
The usage text names the program as invoked: `convert_xfm (Version 2.1)`, the copyright line, the
usage lines and options (exact text in the parity record).

## 10a. The background value and FSL-75

FLIRT computes an image's **background value** (the 10th percentile of a border shell, as
`specs/mcflirt.md` §9.2) in three places: each subsampling of the reference pyramid fills the
half-size level with the background value of the level it halves (§5); the final resampling
(`-out`, `-applyxfm`) pads the input with its background value unless `-setbackground` is given
(§9.2); the windowed-sinc cost's interpolation falls back to it where its kernel sums to about 0.
The shell is `e = min(2, n − 1)` voxels deep along each axis of size `n`; FSL sizes its buffer as
`2(x − 2eₓ)(y − 2e_y)e_z + 2(x − 2eₓ)·z·e_y + 2·y·z·eₓ` values, which is smaller than the shell it
fills (the terms with a negative factor count against it while their loops write nothing) exactly
when **the size along x is 3 and the size along y or z is at least 2, or the size along y is 3,
the size along z at least 2 and the size along x neither 2 nor 4**; and is empty for a **1 × 1 × 1**
image, whose first value FSL then reads. FSL fills the buffer in loop order (the two xy faces
for each depth `e < e_z`, x from `eₓ` to `x − eₓ − 1`, y from `e_y` to `y − e_y − 1`, the voxel at
depth `e` then its mirror at `z − 1 − e`; then the xz faces, x as before, z over the whole size;
then the yz faces, y and z over the whole size), keeps the values that fit, and takes the value at
position `⌊numbins / 10⌋` of those sorted; the rest are written past the buffer (the heap is
corrupted: FSL may abort or crash later, or finish, with the same result where the corruption
touched nothing it reads again; all observed on small random images, and one ordinary 25 × 30 × 27
image at 0.5 × 1.2 × 1 mm whose pyramid reaches 3 × 3 × 3, where FSL finished). The levels' sizes
follow from the images' sizes (§5), so images with 5 or 6 voxels along x or y meet it in the
pyramid. The original, by default (rule A), takes the 10th percentile (`⌊n / 10⌋` of the `n`
values sorted) of **every** value the loops write; with `reproduce`, the values FSL's buffer holds
and its position; in both modes it names FSL-75 wherever the buffer is too small (a notice: FSL
corrupts its heap there). A **1 × 1 × 1** image has no border (FSL reads an empty buffer): refused
in both modes (rule B), before writing anything, with a message naming the size and FSL-75, exit
status 1. NaN values in the border have no defined order in FSL's sort (FSL-22, as for mcflirt,
`specs/mcflirt.md` §9.2): the original sorts them last by default and names FSL-22; with
`reproduce` it gives them the order FSL's build gives them (libstdc++'s introsort, insertion sort
below 16 values), except where that sort would read outside the vector (sorted last there).

## 11. Defects (D13) and larmorx's defaults

The register (`docs/findings/upstream-defects.md`) holds the entries; what the original must do by
default and with `reproduce`:
- **FSL-09** (output types: truncation, int8 as uint8; §9.2): rounded and clamped by default.
- **FSL-11** (`FSLOUTPUTTYPE` unset): `.nii.gz` by default.
- **FSL-14** (a NaN voxel makes the cost NaN): non-finite values left out of the cost sums by default.
- **FSL-21**, **FSL-66** (no or negative bins: `-bins` below 8 gives 0 bins at the 8 mm scale; FSL
  aborts or crashes): refused with a message in both modes.
- **FSL-56** (NIfTI extensions' offset): corrected by default.
- **FSL-67** (BBR normals scaled by the reference level's voxel sizes, §6.5): **kept in both
  modes** (rule C, measured 2026-11-01): computing the normals in millimetres (the segmentation's
  own voxel sizes) moves fMRIPrep's BBR results by 0.016 mm (ds005040, 0.85 × 0.83 × 0.83 mm) and
  0.025 mm (ds000228, voxel sizes 1 mm up to a float's last bit) on average over the brain, and on a
  strongly anisotropic synthetic case with a known motion (0.85 × 0.83 × 2.5 mm, three starts) it
  brings the result 0.003 mm closer to the truth on average (0.060 against 0.063 mm) and not closer
  at the 95th percentile: below the size of BBR's own sensitivity to its last bits
  (`validation/investigations/fsl-flirt/fsl67_measure.py`).
- **FSL-68** (every error after the command line ends with exit status 0, nothing written; §2):
  exit status 1 with the same message by default; `reproduce` exits 0.
- **FSL-78** (`midtrans`: an exception of its computation, a singular or non-square matrix, is
  printed and the program exits with status 0, nothing written; §15.5): exit status 1 by default
  (rule B), FSL's 0 with `reproduce`, named in both modes.
- **FSL-79** (`applyxfm4D`'s spline interpolation of an integer image keeps its spline
  coefficients and each interpolated value in the image's type; §15.7): the float image's
  coefficients and values by default (rule A: the coefficients are a filter's output, not image
  values), FSL's with `reproduce`; named where the result differs.
- **FSL-80** (`applyxfm4D -3D` with sinc: the program asks for a rectangular window of full width
  9 but gets the blackman window of width 7 it defined first; §15.7): the rectangular width-9
  kernel by default (rule A), the blackman width-7 one with `reproduce`; named where the result
  differs.
- **FSL-81** (`aff2rigid` of FSL 6.0.7.23 stops with a Python `NameError` before writing; §15.8):
  the matrix computed and written by default (rule A), FSL's traceback and exit status 1 with
  `reproduce`, named in both modes.
- Kept (rule C): `pointflirt` reads a file of exactly three points as three coordinates per
  column (a 3 × 3 file is taken as given, not transposed; §15.6); documented.
- Undefined behaviour, refused in both modes with a message: an uninitialised matrix name (§7.1),
  `bbrstep` below 1 (FSL-71: FSL never finishes), `-pedir` without a field map read (§6.5), a `read`
  of an unparsable file (FSL-72: FSL reads for ever, its memory growing until an allocation fails:
  `std::bad_alloc`, exit status 0 under an address-space limit), the background value of a single
  voxel (§10a, FSL-75; the buffer too small for the border: corrected by default, FSL's values with
  `reproduce`).

## 12. Numerics and operation order

- Matrix arithmetic is Armadillo 14.2.1's (`specs/mcflirt.md` §13): 4 × 4 products in its small-
  matrix order (row by column, left to right); `inv(A)·B` as an LU solve with partial pivoting
  (`dgesv`, OpenBLAS's Haswell kernels); `A·B·C` as `(A·B)·C` for equal square sizes; `A·inv(B)·C·
  inv(D)` evaluates each inverse (LU) and then the products of four; `inv(A)` alone by LU (`dgetrf`
  + `dgetri`), with the 2×2 and 3×3 closed forms and the diagonal shortcut of Armadillo.
- (spec writer, 2026-11-02; question 11, observed with the model: FSL's text of `convert_xfm
  -inverse`, its `-0`s included) **The 4 × 4 inverse exactly.** A diagonal matrix: reciprocals.
  An upper triangle (the left-right flip of §4.1, the skew of §3): the triangle's inverse below.
  Otherwise the LU factorisation with partial pivoting, column by column (left-looking): for
  column `j`, the earlier row interchanges; the entries above the diagonal `a_ij − Σ_{k<i} l_ik
  a_kj` (the sum from 0 in order, then `+ 0`); the entries on and below it `y − A·x` (below); the
  pivot the first entry of largest magnitude, the rows interchanged, the entries below it
  multiplied by the pivot's reciprocal. Then the inverse of `U`, column by column: `d = 1/u_jj`;
  the column above the diagonal multiplied by the part already inverted (for each earlier column
  `i` in order: `x_k = x_i·u_ki + x_k` for `k < i`, then `x_i = x_i·u_ii`), then each entry
  multiplied by `−d` (so a zero becomes `−0`). Then from the last column to the first: the
  column's entries below the diagonal set aside and zeroed, and the column replaced by `y − A·x`
  with the columns to its right and the entries set aside; then the column interchanges in
  reverse order. Every `y − A·x` (OpenBLAS's matrix-vector kernel with `α = −1`): with four rows,
  the columns in pairs, `y − (a_{r,k+1}·x_{k+1} + a_{r,k}·x_k)` (the second product first, then one
  subtraction), a last single column as `y + a_{r,k}·(x_k·(−1))`; with fewer rows, `t` summed from
  0 in order, then `(−1)·t + y`. The kernels' fused multiply-adds occur only with `α = −1`, where
  they give the rounded subtraction above: no FMA changes these inverses.
- Every cost evaluation's sums are single precision in the visiting order (§6); the line search is
  single precision (`specs/mcflirt.md` §7). BBR's sums are double. (spec writer, 2026-11-02) No
  fused multiply-add in FLIRT's own arithmetic (the costs, the filters, the boundary points, the
  line search: FSL's x86-64 build has none there): every product is rounded before it is added;
  glibc's functions and OpenBLAS's kernels compute as their own builds do.
- glibc's `exp`, `tanh`, `logf` give the last bits (CORE-MATH agrees in more than 99.8% of calls).
- Speed (built-for-speed original): the cost sums may be computed in blocks of rows in parallel only
  if the block sums are combined in the visiting order with FSL's three-level summation (row, slice,
  volume) preserved — the row sums are independent, so rows can run in parallel and be folded per
  slice in order; (spec writer, 2026-11-02) this holds for the costs summed in three levels
  (`normcorr`, `specs/mcflirt.md` §6.4; `labeldiff`, §6.3), **not** for `corratio`, `woods` and the
  mutual-information histograms, whose per-bin sums are single running sums over the whole grid in
  the visiting order (§6.2, appendix B): their rows can be interpolated in parallel, but the values
  must be added to the bins in the visiting order to keep FSL's bits; the search's coarse grid points are independent (each is optimised from the
  reference parameters) and can run in parallel; the fine grid's costs too; the threshold step's
  optimisations too. The pyramid's convolutions are separable and parallel over slices.

## 13. Validation plan for the original

Cases (the replica's `fsl-flirt-replica` cases are the model; their inputs are made by the suite
from ds005040's fMRIPrep run): fMRIPrep's 22 BBR calls (3 in the standard tier); the default schedule
on a BOLD reference to T1w at 6, 7, 9, 12 dof; every cost function; every search option; weights;
`-applyxfm` 3D and 4D with every interpolation and data type; the boundary-based options; every
shipped schedule; the 2D mode; the verbose levels; every failure of §2 and §10.

Metrics and thresholds (PLAN.md §11.3):
- **Matrices**: the RMS deviation from FSL's matrix (§8.6, radius 80 mm about the origin) and the
  95th-percentile displacement over a brain mask of the reference: at most 0.1 mm for fMRIPrep's BBR
  calls and 6-dof runs, 0.5 mm for 12-dof runs; exact for `-applyxfm` and `convert_xfm` (identical
  text).
- **Images** (`-out`): with the same matrix (fed by `-applyxfm`), bit-identical; from the original's
  own matrix, NCC with FSL's image ≥ 0.999 inside the reference's mask.
- **Messages, statuses, files**: identical.
- **Band**: FLIRT is deterministic; its band comes from perturbed inputs (±0.5 of the last stored
  unit of the input; one slice cropped, reported) and from FSL 6.0.7.17 against 6.0.7.23; the
  record reports the original's distances as ratios to that band.

FLIRT's other programs (§15): every case of the record's `programs/` category (every option, the
usages, the files that cannot be read, the singular matrices, the aborts) identical in text and
exit status; avscale also on random matrices (rotations, scales, skews, reflections, any matrix:
identical text). `midtrans` and `pointflirt` (SVD-based: §15.5, §15.6) and `aff2rigid`: matrices
within 1e-9 of FSL's in every element (bit-identical if the original ports §8.3's library
routines), messages and statuses identical; `applyxfm4D`: bit-identical images for nearest
neighbour and trilinear in every data type, sinc and spline as §9.2's images (NCC ≥ 0.9999 with
FSL's, identical where both are exact); `extracttxt` and `pairreg`: identical text and matrices
(`pairreg` as FLIRT's). The parameter subsets and `alignpaxes` (§8.3, §8.7): the matrices'
thresholds above.

## 14. Open questions

Closed in part 3: the principal-axes alignment (§8.7), every parameter-subset mask (§8.3),
`midtrans`, `pointflirt`, `applyxfm4D` (§15.5 to §15.7), the scripts (§15.8), `-debug` (§9.4) and
`makerot`'s NaN signs (§15.4). Open:
1. The coordinate programs' `-warp` (FNIRT's warp fields): refused by the original (and by the
   replica) until FSL's warp-field libraries are ported (`docs/next-steps.md`).
2. The sign of NaN results from non-finite inputs in `reproduce` mode (FSL-14 lets them in): an
   all-NaN `-omat` matrix when a schedule optimises relative to a row (`optimise ... rel`, with or
   without a parameter subset), and the NaN voxels of `applyxfm4D`'s spline interpolation of an
   image with NaN and infinite voxels: FSL writes `nan` or `-nan` by an operand order not specified
   here (the values are NaN either way; six findings of the part-3 trial's 209,657 cases and one
   of its pre-check's).
3. Not ported, by decision: `standard_space_roi`, `flirt_average`, `epi_reg` (they need FSL's other
   programs: `fslmaths`, `fslroi`, FAST, `fugue`).

### The clean-room questions of 2026-11-02 (spec writer)

The questions of FLIRT's clean-room original, part 1 (its black-box observations:
`docs/findings/fsl-flirt-original.md`), answered in the sections named. "Right", "incomplete" or
"wrong" is the observation as the implementer stated it. The models that reproduce FSL's numbers
from this spec's text alone are in `validation/investigations/flirt-spec-answers/` (its README).

| # | Question | Answered in | The observation | Confidence and check |
|---|---|---|---|---|
| 1 | `corratio` and `woods`: the points of a bin with at most two (weighted) points | §6.2 (`specs/mcflirt.md` appendix A for mcflirt) | right: left out of the count, the sums and the total variance (`corratio`), of the count (`woods`) | high: the model gives FSL's six printed `corratio` (smoothing 8 and 0) and `woods` values on two images; the sparse bins out of `A` only give 0.982248 for FSL's 0.984415 |
| 2 | `woods`' formula | §6.2 | incomplete: `Σ n_b σ_b/μ_b / n` with `σ_b` from `n_b − 1` holds for bins with a positive sum; a bin whose sum is not positive adds `n_b²·σ_b`; appendix A's `n_b²·σ_b/μ_b` corrected | high: FSL's 37174512 on the BOLD reference minus 30000, where `n_b·σ_b` gives 20844 |
| 3 | The correlation ratio's sums and precisions | §6.2, §12 | incomplete: the per-bin single-precision running sums in the sampling order are right (one level, products rounded, no fused multiply-add); missing were the two double-to-single roundings of the result (`η = 1 − A`, then the cost `1 − η`) | high: the model prints FSL's costs and parameters at all 23 evaluations of the search's first sweep on `real/dof6` (the 9th and 10th included); without the two roundings it parts at the 5th, with double sums at the 2nd. Also stated: the centres of mass' single-precision `v − v_min` and block sums (§8.1), needed for the printed translations |
| 4 | BBR's normals: the Gaussian, the gradient's weights, the voxels' order | §6.5 | wrong reference (`specs/bet.md` has no such masks): a Gaussian of σ = 2 mm (radius `2·trunc(σ − 0.001) + 3` voxels, not that many taps) and weights 3, 1, 1/3 (not `[1, 2, 1]`); the order z, y, x was right | high: the model gives FSL's 0.537036 (and the other 15 printed costs of the initial matrix: `bbrstep` 1, 2, 200, 201; default, `-noclamp`, `local_abs`, `global_abs`) |
| 5 | `-bbrtype local_abs` | §6.5 | right: `min(u, 2 − u)` per term; the 1.4 mm come from the normals (question 4), which this cost amplifies | high for the formula (the model's four `local_abs` costs are FSL's); medium that the record's case then passes (FSL's own band 0.90 mm) |
| 6 | The search: (a) the grids follow the `search` command's dof; (b) the starts are the sampled parameters; (c) the minima z outer; (d) `P` holds each start with its own cost; (e) both centres measured again | §8.1 steps 2, 3, 4, 7, 8, 9 | (a), (c), (d), (e) right; (b) right for the grids and the threshold step, incomplete for the minima: their first `min(-dof, 7)` parameters are composed and that matrix's decomposition is the start (the same angles to single precision; with `-dof 6` the median scale dropped); also stated: the grids' tolerances (`x₀` the parameters in force when the search starts), the percentile's element, the list's replacement rule, and `-anglerep quaternion`'s minima read as quaternions (`real/anglerep-quaternion`'s four messages) | high (source; FSL's four messages counted on the five minima of the `-verbose 7` trace) |
| 7 | The convergence test's `n_t` in the subset form | §8.2 | right: the tolerance list's length (the subset's columns; 12 for every free optimisation); also the weights `|1/t|/n_t` first and the sum rounded to single | high (source) |
| 8 | A `setscale` that only resets the input keeps the pair's centre | §7.2 | right; a later `search` or `aligncog` measures the current (unblurred) input's centre afresh | high (source) |
| 9 | `-applyisoxfm`'s unset sform and the qform's last float bit | §4.1, §9.3 | right about the unset sform (both forms swapped by the two grids' single-precision fields of view); incomplete for the qform: its matrix is rounded to single precision before §3's conversion | high: the model gives FSL's six quaternion and offset fields and twelve sform values on an oblique neurological reference (a double field of view moves the sform's x translation, 1.803835 for 1.803831) |
| 10 | `-applyxfm` writes the image before `-omat` | §9.3 | right (a registration writes `-omat` first) | high (source) |
| 11 | `inv(A)`'s `-0` and OpenBLAS's fused multiply-adds | §12 | right about the order; the fused multiply-adds occur only with `α = −1`, where they equal the rounded subtraction: none changes the inverse | high: the model prints FSL's `convert_xfm -inverse` text, its two `-0`s included |
| 12 | `alignpaxes` with a non-finite moment | §8.7 | right: an empty line and the warning for each non-finite image (the reference level's first), then the error | high (source; the implementer's observation) |
| 13 | The windowed sinc's last bits | §9.2 | incomplete: the table's single-precision samples and lookup (the position's last addition in double), the per-axis weights at `(p − i₀) − e`, the weight `(w_x·w_y)·w_z` and both sums in single precision, z outer | high: the model gives 40000 of 40000 sampled voxels of `-applyxfm -interp sinc` bit for bit; double sums, `w_x·(w_y·w_z)` and a double lookup position part in 38 to 91 % |
| 14 | The spline with `-setbackground` | §9.3 | incomplete: with `-applyxfm` the edge rule is set before the blur, which reads `-setbackground`'s constant beyond the image; the spline's coefficients are the extraslice rule's but count 0 outside the grid; the prefilter and the evaluation stated exactly | high: the model gives FSL's 172800 values of `real/applyisoxfm-4d-blur-setbackground` bit for bit (a blur reading zeros parts in 43441, edge coefficients in 5654) |
| 15 | `-usesqform`'s 4.5e-6 mm | §4.1, §8.5 | the single precision found: the left-right swap of a neurological file, whose field of view `(n_x − 1)·d_x` is a single-precision product | high: the model gives FSL's `-13.74984741` (a double field of view gives `-13.74984288`) |
| 16 | `convert_xfm -fixscaleskew`'s 8th digit | §3 | incomplete: the decomposition's single-precision steps, the skew's triangular inverse and the scales' reciprocals, and the composition's elementary rotations `(B·C)·Bᵀ` (whose (1, 1) entry is `u_x²`) are FLIRT's | high: the model gives FSL's ten digits on the suite's `-fixscaleskew` and on a composition of double angles, where the textbook rotation parts in the 8th digit (1.018201155 for 1.018201124) |
| 17 | A schedule without `setscale` runs on a first pair | §7.4 | right: the 8 mm level, the input blurred to 8 mm, `trunc(bins/8)` bins, the main cost; the requested scale 1 | high (source) |
| 18 | The base scale before the single slices' tripling | §4.2 | right (the headers as stored) | high (source) |
| 19 | Powell's direction's tolerance | §8.2 | right in the L1 sense: `u = |1/Σ|û_i/t_i||`, summed in single precision; also along the axes `u = fl(1/fl(1/t))`, not `fl(t)` | high (source) |
| 20 | The paddings | §9.2 | right: the mask's single-precision pre-image is the interpolation's; within `p` voxels the edge rule (extraslice, or `-setbackground`'s constant); no test with a field map | high (source; the implementer's observation) |

## 15. FLIRT's other programs

All print with C++'s stream formatting: numbers with 6 significant digits (`%g`) until a matrix
is printed, which switches the stream to fixed notation with 6 decimals for good; a matrix is its
rows, each value in fixed notation with 6 decimals followed by a blank (`-0.000000` for small
negatives). An **Armadillo expression** printed (`std2imgcoord`'s inverse matrix) uses Armadillo's
own layout and restores the stream's state: 4 decimals, right-aligned in 9 columns (13 in
scientific notation `d.dddde±XX` when a finite value is at least 100 in magnitude, or non-zero and
at most 1e-4; 10 in fixed notation when one is at least 10 in magnitude), zeros as `0`, `nan`,
`inf`, `-inf`. An uncaught exception ends with the C++ runtime's two lines (`terminate called after
throwing an instance of '<type>'` and `  what():  <message>`) and signal 6; the original prints
them and exits 1 (`docs/porting-guide.md`, failure cases). Matrix files are read as FLIRT reads
them (§2): a missing file prints `Could not open matrix file <name>` and
gives an empty matrix; fewer numbers than the first numeric line announces throw `Could not find
enough numbers in matrix file`. Images that cannot be read throw newimage's exception
(`Image Exception : #63 :: No image files match: <stem>`, then `#22 :: Failed to read volume
<name>` with `Error : ...`). Matrices other than 4 × 4 where FSL would compute with them beyond
4 × 4 (a determinant, an inverse) are refused by the original with a message (FSL computes part
of the output first).

### 15.1 `avscale [--allparams/--inverteddies] matrixfile [non-reference-volume]`

- Fewer than one argument: `Usage: avscale [--allparams/--inverteddies] matrixfile
  [non-reference-volume]` on standard error, exit 255. A first argument starting with `-` is the
  option (any such word; only `--allparams` and `--inverteddies` change anything), the next the
  matrix; a missing matrix argument throws a `std::logic_error` (`basic_string: construction from
  null is not valid`): `Image error`, then the runtime's lines, signal 6.
- With a volume argument: the volume is read first (an empty name is not read: the centre of an
  empty volume, `WARNING::in calculating COG, total = 0.0` on standard error after the matrix is
  read, centre 0), then the matrix; the centre is the volume's centre of gravity in scaled mm
  (`specs/mcflirt.md` §6.1). A matrix of fewer than 4 rows: exit 254 (no message besides the
  reader's).
- The decomposition of §3 (Euler) about the centre; any exception is caught, its message printed
  on standard error (`std::runtime_error`s; `Image error` for others), and thrown again (signal 6):
  a zero scale makes the solve singular (`matrix multiplication: problem with matrix inverse;
  suggest to use solve() instead`), as does a NaN entry.
- Prints `Rotation & Translation Matrix:` and the rotation-and-translation matrix (the first six
  parameters composed about the origin) and a blank line; with `--allparams` `Rotation Angles
  (x,y,z) [rads] = ` and `Translations (x,y,z) [mm] = ` each followed by the three values as a row
  and a blank line; `Scales (x,y,z) = `, `Skews (xy,xz,yz) = ` likewise; `Average scaling = ` the
  mean of the scales (double sum over 3, stored in single precision) and two newlines;
  `Determinant = ` the determinant (Armadillo's), `Left-Right orientation: preserved` (determinant
  above 0) or `swapped`, a blank line.
- The half transforms: with `M' = M` times `diag(−1, 1, 1, 1)` when the determinant is negative,
  the **square root** of `M'`: its quaternion decomposition (§3) with the rotation halved (the
  vector part times `1/(2 cos(t/2))`, `t = asin(|v|)`, both in double), the square roots of the
  scales and half the skews; its rotation `R` (3 parameters, translations 0), `K` the half skews,
  `S` the root scales; its translation solves `((M'·S⁻¹·K⁻¹·R⁻¹ + I)'s 3 × 3 part)·t = M's
  translation` (an LU solve; the four-term product with each inverse evaluated); the forward half
  is `R·K·S` with `t` (the products left to right), the backward half the forward times `M⁻¹`.
  Printed after `Forward half transform =` and `Backward half transform =`, each with a blank line.
  A singular `M` throws `inv(): matrix is singular` (signal 6, after the printed lines).
- `--inverteddies`: after a blank line, `Matrix check: mat^-1 * rotmat * skew * scale =` and
  `M⁻¹·R·K·S` (left to right), a blank line, then `Inverted eddy matrix:` and `R·K'·S'` with `S'`
  the scales with `1/s_y`, `K'` the skews with the xy and yz skews negated.

### 15.2 `img2imgcoord`, `std2imgcoord`, `img2stdcoord`

Command lines (options anywhere; a word not starting with `-` is the coordinate file; an empty
word skipped; `-help` prints the usage on standard output and exits 0; no argument prints it and
exits 1): `-src`/`-dest` (img2imgcoord) or `-img`/`-std` (the others), `-xfm` or `-premat` (the
matrix), `-warp` (a FNIRT warp field), `-vox`, `-mm`, `-v` (img2imgcoord: verbose level 5; the
others: 1), `-verbose` (the others: 5), `-flirt` (accepted; the others print `WARNING::Using
outdated options, please update to new usage`), `-` (the others: read standard input). An option
that needs a value and is last: `Lacking argument to option <o>` and the scan ends (the run goes
on); an unknown option: `Unrecognised option <o>`, exit 255. A missing source (input) name:
`ERROR:: Source volume filename not found` (img2imgcoord), `ERROR:: image filename not found`
(std2imgcoord), `ERROR:: input image filename not found` (img2stdcoord), followed by a blank line;
likewise for the destination (`ERROR:: Destination volume filename not found`) and, with `-std`
given empty, `ERROR:: standard image filename not found`; the run goes on (the image read then
throws). Usage texts are FSL's, verbatim (the record compares them).

Images are read **header only**: sizes below 1 count 1; voxel sizes as stored (magnitudes, no
repair of zero sizes, no ANALYZE origin); swapped to radiological order when stored neurologically
(the sform and qform follow). Defaults: img2imgcoord in voxels, std2imgcoord output in mm,
img2stdcoord input in voxels.

Per program: img2imgcoord reads `-src`, then `-dest`; the others `-img`, then `-std` when given
(without it an empty standard volume: no sform or qform, 1 mm voxels). With verbose level above
3: `<name>:: SIZE = x x y x z` and `<name>:: DIMS = dx x dy x dz mm` (a blank line after), for the
destination then the source (`Destination Volume`, `Source Volume`; the others `standard image`
when given, then `input image`). The matrix (`-xfm`; the identity without one) must have 4 rows
(`Cannot read transform file`, exit 254); verbose above 3 prints ` affmat =`, the matrix and two
newlines (std2imgcoord also ` Inverse affmat =` and the inverse as an Armadillo expression).
`-warp`: the file is read as an image; when it cannot be, its exception's lines and `An error
occured while reading file: <name>`, exit 1 (with a readable warp field the original's behaviour is
open, §14). The others then warn `WARNING:: standard coordinates not set in standard image`
(std2imgcoord) or `WARNING:: mm coordinate space not set in standard volume` (img2stdcoord) when
the standard volume has neither sform nor qform; with verbose above 3, ` stdvox2world =` and its
millimetre matrix. Then img2imgcoord always prints `Coordinates in Destination volume (in mm)` or
`(in voxels)`; std2imgcoord with verbose prints `Coordinates in input image (in mm):` or `(in
voxels):`; img2stdcoord with verbose `Coordinates in standard image (in mm):`. Coordinates come
from the file (`Could not open matrix file <name>`, exit 255, when it cannot be opened) or from
standard input when the name is `-` or empty (verbose: a prompt line, `Please type in input image
coordinates (in voxels) :`, `(in mm) :`, std2imgcoord `Please type in standard coordinates :`;
img2imgcoord and img2stdcoord refuse a terminal with `Interactive mode has been removed. Please
pipe input such as: cat coordinate_file.txt | <program>, OR supply a coordinate file`, exit 1).
Triples are read as C++ streams read doubles (`std::istream >> double`, "C" locale); the first triple that
does not read ends the input; std2imgcoord on standard input also ends at a triple equal to the
previous one. Each output line is the three coordinates separated by two blanks.

The mapping (`V` an image's millimetre matrix: its sform if set, else its qform, else its voxel
sizes; `S` its voxel-size diagonal; `N` its NIfTI-to-newimage voxel matrix: the identity, or for an
image stored neurologically `S⁻¹·F·S` with `F` the reversal of x about the field of view (`x' =
(nx − 1)·dx − x`, the field of view in single precision), evaluated as a solve; every `A⁻¹·v` is an
LU solve of `A` against `v`, every `A·v` a 4 × 4 times 4-vector product): through the matrix `M`
from source to target, `n(u) = S_t⁻¹·(M·(S_s·u))` (the fourth element of `u` kept):
- img2imgcoord: in voxels `N_d⁻¹·n(N_s·c)`, in mm `V_d·n(V_s⁻¹·c)`;
- std2imgcoord: `n` from the standard to the input through `M⁻¹` (the inverse evaluated at each
  coordinate: a singular `M` throws `inv(): matrix is singular`, signal 6) of `V_std⁻¹·c`, then
  `V_img·…` (mm) or `N_img⁻¹·…` (voxels);
- img2stdcoord: `V_std·n(V_img⁻¹·c)` (input in mm) or `V_std·n(N_img·c)` (voxels).
A matrix that is not 4 × 4 at the mapping throws `NewimageCoord2NewimageCoord: M must be a 4x4
matrix` (newimage's exception #11; signal 6, after the lines printed so far).

### 15.3 `rmsdiff matrixfile1 matrixfile2 refvol [mask]`

Fewer than 3 arguments: `Usage: rmsdiff matrixfile1 matrixfile2 refvol [mask]` and `        Outputs
rms deviation between matrices (in mm)` on standard error, exit 255. Each matrix must have 4 rows
(`Could not read matrix <name>`, exit 254). A matrix whose determinant's magnitude is below 0.1:
`WARNING:: matrix <1|2> has low determinant`, the matrix (standard error), a blank line. The
reference is read in full (a 4D image: its first volume, with FSL's warning). Without a mask: the
RMS deviation (§8.6) over 80 mm **about the reference's centre of gravity** (scaled mm), printed in
single precision; a singular second matrix: `RMS_DEVIATION ERROR:: Could not invert matrix`, exit
251. With a mask: over the mask's voxels above 0.5 (its own indices, x fastest; the coordinates
`(i·dx, j·dy, k·dz)` with the reference's voxel sizes, each product in single precision), the
distance `|(M1 − M2)·(x, y, z, 1)|` (double, the norm as `√` of the sum of squares); printed are
the largest distance (a NaN distance never replaces it) and `√(Σ d² / n)` (an empty mask: NaN,
printed `-nan`).

### 15.4 `makerot --theta=angle [options]`

FSL's general option parser (as `bet2`'s, `specs/bet.md`): `-t,--theta` (compulsory, a float
read whole: `Couldn't set_value! valstr="<word>"`), `-a,--axis` and `-c,--centre` (one word each,
split at `,`, or at `:` when the word holds one, each piece read as `atof` reads it: `x` is 0, an
empty piece 0), `--cov` (an image name), `-o,--out` (a file name), `-v,--verbose` (no effect),
`-h,--help`; long options take their value after `=` only (`--theta 30`: `--theta: Missing
non-optional argument!`); one-letter options group (`-vt 30`); a word that is not an option:
`<word>:  is an unrecognised token!`. A parse error prints the usage, a blank line and the message
to standard error, exit 1. `-h` prints the usage, exit 1 (help is checked first: no compulsory
message). Without `-t`: the block `***************************************************`, `The
following COMPULSORY options have not been set:`, each missing option's usage line, the rule again,
then the usage, exit 1. The usage: `Part of FSL (ID: 2412.6-dirty)`, the title `makerot `,
`Copyright(c) 2003, University of Oxford (Mark Jenkinson)`, a blank line, `Usage: `, `makerot
[options] --theta=angle`, then the compulsory and optional options in the order `-t`, `-a`,
`--cov`, `-c`, `-o`, `-v`, `-h` (a help text beginning `~` prints after one blank: `-a,--axis
<ax,ay,az>	unnormalised axis vector (comma separated)`).

The axis is (0, 0, 1) unless given; a zero axis (its length exactly 0): `ERROR:: Axis cannot be
zero`, exit 1. FSL then divides the axis by **the square root of its length** (FSL-76), so the
rotation is by `θ·√|a|`: 30 degrees about (0, 0, 4) give 60. The original divides by the length
(rule A: the help calls the axis "unnormalised") and names FSL-76 wherever the length is not
exactly 1; `reproduce` gives FSL's matrix. An axis or centre of **other than three values**
(FSL-77): FSL reads past the values it was given (the vector's buffer: the default's zeros, or the
values an earlier occurrence of the same option left there: `-a 1,2,3 -a 7` is (7, 2, 3)) and
ignores values beyond the third; the original refuses it by default (rule B, a message naming
FSL-77, exit 1) and reads as FSL does with `reproduce` (named). The centre is (0, 0, 0) unless
given; `--cov` replaces it by `((n − 1)·d·0.5)` along each axis of the image (size `n`, voxel size
`d`, in double), whatever `-c` says; an image that cannot be read throws newimage's exception
(signal 6; the original exits 1 after the same text). The matrix is §8.5's rotation
about the axis scaled by `θ·π/180` (in double, element by element: `((a·θ)·π)/180`) through the
centre. With `-o` it is written as FLIRT writes matrices (§9.1; a file that cannot be opened:
`Could not open file <name> for writing`, exit 0); otherwise printed (§15's matrix layout) and
followed by a blank line. Exit 0.

**NaN signs** (a NaN or infinite angle, axis or centre; observed, part 3, every bit of FSL's
output: `validation/probes/fsl-flirt-programs/makerot_nan.py`). FSL's arithmetic follows x86-64's
SSE rules: an invalid operation (`∞ − ∞`, `0·∞`, `∞/∞`) gives the default NaN, whose sign bit is
set (printed `-nan`); an operation with one NaN operand returns that NaN (its sign kept); with two,
the first operand's. The order of the operands is therefore part of the result: in each 3 × 3
matrix-vector product of the rotation (each column of `(B·R)·Bᵀ`, and `(I − R)·c` for the
translation), the **first row** is formed as `x₀·a₀₀`, then `(x_k·a₀ₖ) + s` for `k = 1, 2` (the
vector's element first, the new product before the running sum `s`); the **other rows** as
`a_r0·x₀`, then `s + a_rk·x_k` (the matrix element first, the running sum first). Every other step
(the norms, the divisions, the cross product) passes NaNs on by the same rules; the angle's single-
precision sine and cosine of a non-finite angle are `θ − θ` (glibc's: the default NaN for an
infinite angle, the NaN itself for a NaN).

### 15.5 `midtrans [options] transform1 transform2 ... transformN`

The transform from the space the inputs map into (a template) to their "middle". FSL's general
option parser (§15.4's rules: long options take `=value`, one-letter options group): `-o,--out`
(a file name), `--template` (an image), `--separate` (a name prefix), `--debug`, `-v,--verbose`,
`-h,--help`; the first word that is not an option and every word after it are the transforms
(matrix files). Usage: `Part of FSL (ID: 2412.6-dirty)`, the title `midtrans ` and `Copyright(c)
2010, University of Oxford (Mark Jenkinson)`, a blank line, `Usage: `, then `midtrans [options]
transform1 transform2 ... transformN`, `  e.g. midtrans -o temp2mid.mat A2temp.mat B2temp.mat
C2temp.mat`, `       midtrans -o C2mid.mat A2C.mat B2C.mat ident.mat`, then the options (help texts:
`output filename for matrix`, `input filename for template image (needed for fix origin)`,
`basename for output of separate matrices (final name includes a number; e.g. img2mid0001.mat)`,
`switch on debugging output`, `switch on diagnostic messages`, `display this message`). A parse
error: the usage, a blank line, the message, exit 1; `-h`: the usage, exit 1. Fewer than two
transforms: `Must specify at least 2 transforms to find the mid-transform`, then the usage, exit 1.

Each transform is read as §15's matrix files (a missing file: its message and an empty matrix,
which has determinant 1); a matrix that is not square: `det(): given matrix must be square sized`
(FSL-78); a determinant below 1e-6 in magnitude: `Could not read matrix <name>`, exit 1. With `-v`:
`Number of matrices = <N>`. The centre `o`: the template's centre of mass in scaled millimetres
(§6.1) with `--template`, else `Assuming MNI origin as no template image specified` on standard
output and `o = (90, 110, 90)`, `(0, 0, 0)` with `--debug`; with `-v`, `Centre of template = ` and
`o` as a row. With `-v` the stages print `Initialising averages`, `Stage 1`, `A = `/`s = `,
`Stage 2`, `Subject number <n>` with `R = `/`u = `, `Stage 3`, `M = `/`w = `, `Final calculations`
(each matrix in §15's layout); `--debug` prints `P_<n> = ` and `Pxx_av = `.

The computation, for `N` transforms `T_n = [P_n t_n; 0 1]`, with `x_n` the first three components
of `T_n⁻¹·(o, 1)` (an LU solve), `r = (80·80)/5` (single precision) and every average a sum in
order divided by `N` as a single-precision number:
1. `M₁ = avg(P_n·(r·I + x_n·x_nᵀ)) + avg(t_n·x_nᵀ) − avg(P_n·x_n)·avg(x_n)ᵀ − avg(t_n)·avg(x_n)ᵀ`;
   its SVD `U·D·Vᵀ` (§8.3's driver) gives the rotation `A = V·Uᵀ` and `s = avg(x_n) −
   A·(avg(P_n·x_n) + avg(t_n))`.
2. For each `n`: `P'_n = A·P_n`, `t'_n = A·t_n + s`, `R_n = V·Uᵀ` from the SVD of `P'_nᵀ`, and
   `u_n = (P'_n − R_n)·x_n + t'_n`.
3. With the averages over `n` of `x`, `u`, `t'`, `t'·t'ᵀ`, `u·t'ᵀ`, `P'·x`, `R·x`, `(P'·x)·t'ᵀ`,
   `(R·x)·t'ᵀ`, `(u·xᵀ)·P'ᵀ`, `P'·P'ᵀ`, `R·P'ᵀ`, and the sums `Σ((P'x)·xᵀ)·P'ᵀ`, `Σ((Rx)·xᵀ)·P'ᵀ`
   divided by `N` (in double): `Pxx = r·avg(P'P'ᵀ) + Σ(...)/N`, `Rxx = r·avg(RP'ᵀ) + Σ(...)/N`;
   `AAᵀ = Pxx − avg(P'x)avg(P'x)ᵀ + avg((P'x)t'ᵀ) + avg((P'x)t'ᵀ)ᵀ − avg(t')avg(P'x)ᵀ −
   avg(P'x)avg(t')ᵀ + avg(t't'ᵀ) − avg(t')avg(t')ᵀ` and `BAᵀ = −(Rxx − avg(Rx)avg(P'x)ᵀ +
   avg((Rx)t'ᵀ) − avg(Rx)avg(t')ᵀ − avg((ux)P'ᵀ) + avg(u)avg(P'x)ᵀ − avg(ut'ᵀ) + avg(u)avg(t')ᵀ)`,
   summed left to right; `M = −(BAᵀ·inv(AAᵀ))` (the inverse by LU, then the product) and
   `w = −(M·avg(P'x)) − M·avg(t') + avg(Rx) + avg(u)`.
The result is `[M·A, w + M·s; 0 0 0 1]`. Products of a matrix with its own transpose are formed
as symmetric rank-k updates (each element `Σ a_ik·a_jk` in order, both triangles alike). With
`-o` it is written (§9.1's matrix file: a file that cannot be opened prints `Could not open file
<name> for writing`); printed (§15's layout, then a blank line) with `-v` or without `-o`. With
`--separate p`, each `result·T_n` is written to `p0001.mat`, `p0002.mat`, ... Exit 0. A singular
`AAᵀ` (`inv(): matrix is singular`) or a singular input (`matrix multiplication: problem with matrix
inverse; suggest to use solve() instead`) prints the message (FSL-78); a non-finite matrix makes the
SVD fail: refused by the original with a message (rule B).

### 15.6 `pointflirt -i <file> -r <file> [options]`

The affine matrix that maps the input points onto the reference points in the least-squares
sense. FSL's general option parser: `-i` and `-r` (compulsory, text files of coordinates),
`-o,--out` (a matrix file), `--vox`, `--invol`, `--refvol` (images), `-v,--verbose`, `-h,--help`
(the usage: the title `pointflirt `, `Copyright(c) 2001, University of Oxford (Mark Jenkinson)`,
the examples `pointflirt -i <invol coords>  -r <refvol coords> -o <output matrix>` and `pointflirt
-i <invol coords>  -r <refvol coords> -o <output matrix> --vox --invol=<input vol> --refvol=<ref
vol>`; parse errors, `-h` and missing compulsory options as §15.4). The files are read as matrix
files (§15); a matrix with more rows than columns is transposed (both files are then transposed),
so points are columns: a file of exactly three points (3 × 3) is taken as three points **per
column** (rule C, §11). Different numbers of points: `ERROR: Different number of coordinates in
the two files`, exit 255. With `--vox`, both `--invol` and `--refvol` are needed (`ERROR: need to
specify --invol and --refvol to use --vox`, exit 2); their headers' voxel sizes multiply each
coordinate (x, y, z). With `-v`: `Input volume coordinates are (in mm)`, the matrix, a blank line,
`Reference volume coordinates are (in mm)`, the matrix, a blank line. Coordinates other than three
per point stop the run (`copy into submatrix: incompatible matrix dimensions: 3x1 and <k>x1`,
signal 6; the original exits 1 after the text).

With `X` (3 × n) and `Y` the points, `S = X·Xᵀ` (a symmetric rank-k update), `x̄`, `ȳ` the means
of the rows (each value divided by `n`, then summed in order), `Q = X·Yᵀ`: the 12 × 12 system `B` has `S` in its
three diagonal 3 × 3 blocks, the column block `W·n` at rows 1-9 and columns 10-12 (`W` 9 × 3 with
`x̄` in rows `3k+1..3k+3` of column `k`), `Wᵀ` at rows 10-12 and columns 1-9, and the identity at
rows and columns 10-12; the right-hand side `q = (Q(:,1), Q(:,2), Q(:,3), ȳ)`. Its SVD
`B = U·diag(d)·Vᵀ` (§8.3's driver): each `d_k > 1e-8` replaced by `1/d_k`, the others by 0; any
zero printed as `WARNING: nullspace of dimension <k>`, `  ... lacks sufficient independent
coordinates`, `Finding non-unique solution` (standard output); `p = V·(D·(Uᵀ·q))`. The matrix rows
are `(p1 p2 p3 p10)`, `(p4 p5 p6 p11)`, `(p7 p8 p9 p12)`, `(0 0 0 1)`; written with `-o`, else
printed (§15's layout, then a blank line). Exit 0. A non-finite coordinate makes the SVD fail
(`Mat::operator(): index out of bounds`, signal 6; the original exits 1).

### 15.7 `applyxfm4D <input> <ref> <output> <transform file or directory> [options]`

Every volume of the input resampled onto the reference's grid. Fewer than four arguments: the usage
(`Usage: applyxfm4D <input volume> <ref volume> <output volume> <transformation matrix file/dir>`, a
blank line, `\t--interp, -interp <nearestneighbour (or nn), trilinear, spline, sinc (default)>`,
`\t--singlematrix, -singlematrix (flag option, do not provide an argument)`, `\t--fourdigit,
-fourdigit (flag option, do not provide an argument)`, `\t--userprefix, -userprefix <prefix>`, two
blank lines) on standard error, exit 255. The words after the fourth, in order: `-singlematrix`
(one matrix file for every volume), `-fourdigit` (the matrices `MAT_nnnn`), `-userprefix p` (the
matrices `p` followed by the four digits), `-interp m` (`nearestneighbour` or `nn`, `trilinear`,
`sinc`, `spline`; another word: `interpolation method: <m> is not a valid choice. Please choose
from nearestneighbour, trilinear, sinc, or spline.`, exit 1), `-3D`/`--3D`/`-3d`/`--3d` (one 3D
volume), `-v`/`-verbose`/`--verbose`; every option has its `--` form; other words are ignored; an
option needing a value as the last word stops the run (`basic_string: construction from null is
not valid`, signal 6; the original exits 1). Without `-singlematrix` the matrix of volume `m` is
`<dir>/MAT_0` followed by the four digits of `m` (each `'0' + digit`). The type: the input's,
grouped as FSL's programs group types (char, short, int, float, double; int8 as char): the values
are interpolated in single precision from the type's values, each converted to single precision
where it is read, except as below; the background value is the type's; each result is converted
to the type (FSL-09: toward zero, int8 written as uint8; rounded and clamped by default).

**4D** (the default): the input read whole, the reference as one volume; `-v` prints `using
interpolation method (enum, string): <0|1|2|4>, <m>`; with more than 10000 volumes `WARNING:: More
than 10000 volumes - only doing first 10000` (volumes up to 10000 done); for each volume, its
matrix file's name printed (standard output) unless `-singlematrix`, then the volume resampled as
§9.2 onto a copy of the reference: its background value (§10a, FSL-75: also named on every
overflow, the whole border by default; NaN voxels in the border ordered as §10a, FSL-22) as the
padding, the extraslice rule, padding size 0, the
sinc kernel blackman with full width 7. The output: the volumes in order, the input's TR, display
range 0, 0. **-3D**: the input and the reference read as volumes; the input padded with its
background value; sinc with the rectangular window of full width 9 (FSL-80: FSL uses blackman of
width 7, `reproduce`); the output on the reference's grid with the input's TR.

**Spline**: cubic B-spline coefficients computed by recursive filtering along x, then y, then z
(as `specs/mcflirt.md`'s spline, the constant rule at the edges for extraslice); for float images
each pass's results stored in single precision; for **double** images in double precision; for
**integer** images (FSL-79, `reproduce`) each pass's results stored as `(T)(c + 0.5)` (the type's
conversion toward zero) and each interpolated value converted to the type before the output's
conversion; by default as float images. **Double** images: the sinc kernel's sum adds each product
`value·kernel` (the value in double, the kernel factor in single precision) in double precision to
the single-precision running sum, rounding it back to single precision at each step; the voxels
the padding mask sets (pre-images outside the input) hold the background value **in double
precision**, while every interpolated voxel (extrapolated ones included) passes through single
precision. Matrix files that cannot be read: their message and an empty matrix, which stops the
run (`matrix multiplication: incompatible matrix dimensions: 0x0 and 4x4`, signal 6; the original
exits 1 after the text); a missing input: newimage's exception (signal 6). Exit 0.

### 15.8 The scripts: `pairreg`, `extracttxt`, `aff2rigid`

`pairreg brain1 brain2 skull1 skull2 outputmatrix [extra flirt args]` (FSL's shell script, run with
`LC_ALL=C`): fewer than five arguments print `Usage: pairreg brain1 brain2 skull1 skull2
outputmatrix [extra flirt args]`, a blank line and `  NB: brain1 and skull1 are used as -ref
arguments internally` (standard output), exit 1. Then, the extra arguments split at blanks and
appended to each FLIRT run: `flirt -in brain2 -ref brain1 -schedule pairreg1.sch -omat
<out>.1.<pid>`; `flirt -in skull2 -ref skull1 -init <out>.1.<pid> -schedule pairreg2.sch -omat
<out>.2.<pid>`; `convert_xfm -fixscaleskew <out>.2.<pid> <out>.1.<pid> -omat <out>.3.<pid>` (the
second matrix with the first's scales and skews, §10); `flirt -in brain2 -ref brain1 -init
<out>.3.<pid> -schedule pairreg3.sch -omat <out>` (FSL's three shipped schedules, in
`$FSLDIR/etc/flirtsch`); the three temporary matrices removed; exit 0 whatever the runs did (each
run's messages as its own). A glob character in an argument is refused by the original (the shell
would expand it).

`extracttxt search_word file [num_trailing_lines [relative_start]]` (a `/bin/sh` script run with
`LC_ALL=C` on GNU grep, dc and sed): one argument or none: `Usage: extracttxt search_word file
[num_trailing_lines [relative_start]]` on standard output and dash's `extracttxt: 68: exit: Illegal
number: -1` on standard error, exit 2. `num` is the line number of the first line matching the
basic regular expression `search_word` (none: nothing printed, exit 0; a missing file: `grep:
<file>: No such file or directory`, exit 0). With a relative start `k`: `num1 = num + k` (dc's
arithmetic on the words as given); with a count `c`: `num2 = num1 + c − 1` and `num1 = num2` when
`num2 < num1`; the lines `num1` to `num2` (to the end without a count) printed by sed. dc reads a
`+` or `-` sign as a command: with fewer than two numbers on its stack it prints `dc: stack empty`
on standard error and does nothing, so a signed argument (`+2`, `-0`, a negative count) and a
negative `num1` read back (`num2`'s computation) each print it and count as their digits. A range
starting with `-` (a negative `num1`) is read by sed as options: `sed: invalid option -- '<its first
digit>'` and GNU sed 4.8's usage on standard error, exit 1; a range starting at 0: `sed: -e
expression #1, char <length of the range>: invalid usage of line address 0`, exit 1. A word or file
with blanks, a word starting with `-`, an empty word, a file with a NUL byte, a non-integer count or
start: refused by the original (the shell passes them unquoted; grep would read them otherwise).

`aff2rigid <input2standard mat> <output mat>` (FSL's Python script, on its NumPy 2.4.6): without
arguments, `Usage: aff2rigid <input2standard mat> <output mat>`, a line holding one blank, `
First argument is the FLIRT transform (12 DOF) from the input image to standard`, `       Second
argument is the output matrix which will go from the input image to standard space (6 DOF)` and
`          aligning the AC, the AC-PC line and the mid-sagittal plane (in order of decreasing
accuracy)` (standard output), exit 1. FSL 6.0.7.23's script then stops with `NameError: name 'mat'
is not defined` (FSL-81: NumPy 2 has no `mat`; its traceback `Traceback (most recent call last):`,
`  File "aff2rigid", line 105, in <module>`, `    v41=mat(zeros((4,1)))`, `        ^^^`, exit 1).
The original computes the matrix the script defines (rule A): with `A` the input (4 × 4, an
input-to-standard matrix) and the standard-space points `x1 = (91, 129, 67)`, `x2 = (91, 100,
70)`, `x3 = (91, 129, 117)` (homogeneous, last component 1): `v2 = (x2 − x1)/‖x2 − x1‖`, `v3 = x3 −
x1 − (v3ᵀv2)·v2` normalised, `v4 = v2 × v3` (as 4-vectors with last component 0); `w2 = A⁻¹·v2`
normalised, `w3 = A⁻¹·v3` made orthogonal to `w2` the same way and normalised, `w4 = w2 × w3`; `R1`
the identity with columns 1-3 replaced by `w2`, `w3`, `w4` (4 components each), `R2` the identity
with rows 1-3 replaced by `v2ᵀ`, `v3ᵀ`, `v4ᵀ`; `R = R2ᵀ·R1ᵀ`, its translation `x1 − R·(A⁻¹·x1)`
(rows 1-3 of column 4). The numbers as NumPy forms them on OpenBLAS's Haswell kernels: `A⁻¹` by LU
(`dgesv` on the identity), norms as the square root of a dot product, products of a matrix and a
vector or of two matrices by the library's kernels (§8.3), cross products in the textbook order.
The output is written with each number in `%14.10f`, four per row, separated by a blank.
