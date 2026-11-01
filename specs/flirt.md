# Spec: `flirt` and `convert_xfm` (behaviour of FSL FLIRT, FSL 6.0.7.23)

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
The initial matrix is converted in (`S M S⁻¹` with `S = diag(1/b, 1/b, 1/b, 1)`, the product of
three as §12 says), and the final matrix out: its translation column (rows 1-3 of column 4)
multiplied by `b`. The output image is computed from unscaled images.

**Parameters.** As `specs/mcflirt.md` §4.4: three Euler rotations (radians), three translations,
three scales, three skews, composed about a centre (`R = Rx·Ry·Rz`, then skew and scale, about
the centre). The centre is the input volume's intensity-weighted centre of mass (`specs/mcflirt.md`
§6.1) **of the volume the current image pair was built with** (§5.4), in scaled mm. Decomposition
(matrix → parameters) is the inverse, about the same centre. With `-anglerep quaternion` the three
rotation parameters are the vector part `(x, y, z)` of a unit quaternion (`w = √(1 − x² − y² −
z²)` in single precision; parameters with `x² + y² + z² > 1` are refused with `Parameters do not
form a valid axis - greater than unity`); the search (§8) always uses Euler angles.

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

### 4.2 The base scale

Unless `-basescale` is given, it is decided from the two images' headers: with `M` the largest and
`m` the smallest absolute voxel size over both images (single precision), `b = M/8` if `M > 12`;
then `b = m` if `m < 0.75` (the second wins). With `-applyxfm` the base scale is 1.

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
smoothing 0 the centred correlation with the true count), `corratio` (appendix A; cost `1 −
η²`-like value `Σ n_b var_b / (n var)`; with smoothing 0 the unweighted counts), `woods` (appendix
A), `mutualinfo` and `normmi` (appendix B), `leastsq` (appendix A; with smoothing 0 the unweighted
mean). FLIRT's histogram costs use the schedule's fuzzy fraction (default 0.5, §7.3); **with
smoothing 0 and fuzzy fraction 0** the mutual-information costs use plain histograms: each sample
adds 1 to the cell of its reference bin and test bin (`trunc(y·b1 + b0)` clamped to `0..B−1`), and
the entropies are `Σ −p ln p` with `p = n/N` in single precision, the terms for counts `n` up to
`min(10000, trunc(10N/(B+1)))` taken from a table computed once per pair (`−p ln p` in single
precision, stored in double, then added to the single-precision sum in double and rounded), the
rest computed directly; the overlap correction and the empty case as appendix B.

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
  product); its normal `n` is minus the 3×3×3 gradient (the gradient masks of `specs/bet.md`'s
  smoothing step, each component a single-precision running sum over the 27 neighbours in z, y, x
  order) of the segmentation smoothed by a Gaussian of σ = 2 mm (the separable Gaussian of
  `specs/mcflirt.md`/`specs/bet.md`), **each component divided by the voxel size of the current
  reference level** (not the segmentation's; FSL-67), normalised by `√(nx² + ny² + nz²)` in single
  precision. The pair's two points are `p + 2n` (the grey-matter side) and `p − 2n` (white
  matter), each coordinate `p_k ± n_k·2` with `n_k·2` in single and the sum in double, stored as
  single. No boundary voxel: `ERROR::set_bbr_seg: could not find any boundary points!` on standard
  error (at each evaluation, which then is NaN, §6.5 below).
- *From `-wmcoords`/`-wmnorms`*: rows of `x y z` (mm) and normals; the points `c ± n·2` in double,
  stored as single.

**The cost of a matrix `A`** (reference mm → input mm through `A⁻¹`): with `T = S_in⁻¹ A⁻¹` (input
voxel sizes `S_in`; computed as the solution of `S_in·T = A⁻¹`, LU with partial pivoting, after
`A⁻¹` by LU), for every pair `k = 0, step, 2·step, …` (`step` from `setoption bbrstep`, default 1):
`g = I(T·p_gm)`, `w = I(T·p_wm)` (4 × 4 times 4-vector products in double, the input interpolated
at the single-precision coordinates by its own interpolation: trilinear with zero outside),
`q = 200(g − w)/(g + w)` in double (0 when `|g + w| ≤ 1e-6`), `u = 1 + tanh(slope·(q − offset))`
(the slope and offset 0 single, converted to double; glibc's `tanh`). With `-bbrtype local_abs`
each term is `min(u, 2 − u)`. The cost is `Σ u / Σ 1` (double sums in pair order; with weights,
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
| `alignpaxes` | list[:rows] | each row's matrix replaced by the principal-axes alignment of the two images (tier 3; not specified further here) |
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
   column has three ones, on `sx, sy, sz`); else the three translations.
3. The reference parameters start at the translation that aligns the centres of mass (`c_ref −
   M_init·c_in`, both in scaled mm, `M_init` applied as a 4 × 4 times 4-vector product), unit scales.
4. **Coarse grid**: for every coarse `(rx, ry, rz)` (x outer, z inner), the subset optimised (§8.3,
   4 iterations) from the reference parameters with those rotations; the resulting translations
   and scale stored on a small grid.
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
   grid's.
8. **Minima**: the grid points (excluding the last index on each axis) none of whose 26
   neighbours (inside the grid) has a strictly smaller cost; if none, the overall minimum (the
   grid scanned z outer, then y, then x, starting from point (0, 0, 0) and replaced on a strictly
   smaller cost). Their interpolated parameters (skews 0) are the search's starting points.
9. Each starting point, as a matrix of `min(dof, 7)` parameters, is optimised freely (§8.2, dof
   `min(dof, 7)`, 4 iterations, with the search cost); the optimised and starting matrices, with
   their costs, enter a list sorted by the optimised cost, from which a new entry is dropped if its
   RMS deviation (§8.6) from an existing one is below the reference level's smallest voxel size,
   unless it is better, which replaces that one.

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
  even and odd indices, then added) below 1 ends the optimisation.
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
LAPACK); the products `P·y` and `pinv(P)·d` are plain sums of rounded products. For other masks FSL
uses an SVD pseudo-inverse whose last bits are not specified here. With `setoption paramsubset`
active, `optimise` uses the subset form with the row's own parameters as `x₀`.

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
(§4.1) and that the input used is the current (blurred) one, whose sform is kept.

### 8.6 The RMS deviation

`√(tᵀt + (80²/5)·tr(AᵀA))` with `D = M1·M2⁻¹ − I`, `A` its 3 × 3 part, `t` its translation (about the
origin), as `specs/mcflirt.md` §10.4.

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
values are resampled through the BBR mapping instead. **Data type**: the input's (§4.1), except that
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

## 11. Defects (D13) and larmorx's defaults

The register (`docs/findings/upstream-defects.md`) holds the entries; what the original must do by
default and with `reproduce`:
- **FSL-09** (output types: truncation, int8 as uint8; §9.2): rounded and clamped by default.
- **FSL-11** (`FSLOUTPUTTYPE` unset): `.nii.gz` by default.
- **FSL-14** (a NaN voxel makes the cost NaN): non-finite values left out of the cost sums by default.
- **FSL-21**, **FSL-66** (no or negative bins: `-bins` below 8 gives 0 bins at the 8 mm scale; FSL
  aborts or crashes): refused with a message in both modes.
- **FSL-56** (NIfTI extensions' offset): corrected by default.
- **FSL-67** (BBR normals scaled by the reference level's voxel sizes, §6.5): kept for now; the
  effect is nil on isotropic levels and is to be measured on anisotropic segmentations (rule D)
  before it is corrected or kept.
- **FSL-68** (every error after the command line ends with exit status 0, nothing written; §2):
  exit status 1 with the same message by default; `reproduce` exits 0.
- Undefined behaviour, refused in both modes with a message: an uninitialised matrix name (§7.1),
  `bbrstep` below 1, `-pedir` without a field map read (§6.5), a `read` of an unparsable file (FSL
  loops forever).

## 12. Numerics and operation order

- Matrix arithmetic is Armadillo 14.2.1's (`specs/mcflirt.md` §13): 4 × 4 products in its small-
  matrix order (row by column, left to right); `inv(A)·B` as an LU solve with partial pivoting
  (`dgesv`, OpenBLAS's Haswell kernels); `A·B·C` as `(A·B)·C` for equal square sizes; `A·inv(B)·C·
  inv(D)` evaluates each inverse (LU) and then the products of four; `inv(A)` alone by LU (`dgetrf`
  + `dgetri`), with the 2×2 and 3×3 closed forms and the diagonal shortcut of Armadillo.
- Every cost evaluation's sums are single precision in the visiting order (§6); the line search is
  single precision (`specs/mcflirt.md` §7). BBR's sums are double.
- glibc's `exp`, `tanh`, `logf` give the last bits (CORE-MATH agrees in more than 99.8% of calls).
- Speed (built-for-speed original): the cost sums may be computed in blocks of rows in parallel only
  if the block sums are combined in the visiting order with FSL's three-level summation (row, slice,
  volume) preserved — the row sums are independent, so rows can run in parallel and be folded per
  slice in order; the search's coarse grid points are independent (each is optimised from the
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

## 14. Open questions

1. The principal-axes alignment (`alignpaxes`) is not specified (no shipped schedule uses it).
2. The SVD pseudo-inverse of general parameter-subset masks (§8.3).
3. Quaternion parameters in the schedule's decomposition (`-anglerep quaternion` outside the search).
