# FLIRT's clean-room original: black-box observations

What the clean-room implementer of FLIRT (`crates/larmorx-fsl/src/flirt/`, part 1, 2026-11-02)
learnt from running FSL 6.0.7.23's `flirt` and `convert_xfm` as black boxes (`~/fsl-6.0.7.23/bin`,
FSL off the `PATH`), beyond `specs/flirt.md`. Nothing here comes from FSL's source, the replica or
the restricted findings. The tools that produced each observation are in
[`validation/investigations/cleanroom-flirt/`](../../validation/investigations/cleanroom-flirt/README.md);
what the spec should say is in the part's spec questions (its handoff).

## The command line and the schedule

- **The default schedules' text.** `flirt -v` prints every line of the schedule it runs: the 3D
  default (85 lines, a trailing blank after `clear UA`) and, with `-2D`, the 2D default (blank
  lines printed as ` >> `, a trailing blank after the 8 mm `optimise`). The original runs and
  prints those lines (`flirt::schedule::DEFAULT_3D`, `DEFAULT_2D`), so `-v` prints FSL's text.
- `WARNING: Both reference and input images have an sform matrix set` is printed only with `-v`.
- `Erroneous dof <d> : using 6 instead` (a `-dof` below 6): printed once for every row an
  `optimise` or `measurecost` command processes and once for every starting point of the
  search's free optimisations, after that start's own messages (corrected by relay 6 of part 2,
  2026-11-02, from FSL's merged streams: the 73 lines of the default schedule with `-nosearch`
  are 72 rows and one minimum; part 1 had read "twice per optimised row, never by
  `measurecost`").
- `dualsort` of lists of different lengths prints `Cannot dual sort matrices of unequal size`
  with no newline; `setoption` with fewer than three words (also `setoption nosubset`) is
  `Wrong number of args to SETOPTION` (exit 255); a line of blanks prints nothing.
- `printparams` prints each row followed by an empty line; `saveparams` the same and one more
  empty line at the end; `save` the rows and one empty line.
- A schedule with no `setscale` (FSL's `measurecost1.sch`, `pairreg2.sch`, `pairreg3.sch`) runs
  on a first pair FLIRT makes before the schedule: the 8 mm level and the input blurred to 8 mm.
- The base scale (§4.2) is decided from the voxel sizes as read, **before** a single slice is
  tripled: an 8 x 1 x 1 reference of 1 x 1.5 x 0.5 mm voxels gives a base scale of 0.5 (its
  0.5 mm slice), which puts a 3-voxel-wide level in the pyramid (FSL-75's case).

## The costs

`validation/investigations/cleanroom-flirt/costs.sch` measures two matrices at the 8, 4 and 2 mm
pairs (and with smoothing 0 and fuzzy fraction 0) for every cost; `costs3.sch` at the 4 mm pair
and the two finer scales that only reset the input (3 mm data); FSL against the original
(`costs.py`):

- `normcorr`, `mutualinfo`, `normmi`, `labeldiff`: the same printed values (6 decimals) at every
  scale, also weighted (`-refweight`, `-inweight`); `leastsq` to 7 significant digits.
- `corratio` and `woods`: **the points of a reference bin holding at most two points are left out
  altogether**: of the bins' sums, of the count `n` and of the total variance (`corratio`), of the
  count (`woods`). Found on synthetic images (`corratio_synth.py`, `corratio_sparse.py`): on a
  10 x 10 x 10 ramp FSL's `corratio` was 0.6 to 2 % above appendix A's formula (the sparse bins
  left out of the sum only) and its `woods` 0.14 % (`n/(n − 1)`), at 8, 16 and 32 bins alike;
  leaving the sparse bins' points out everywhere gives FSL's printed values in every case. With
  it the original's `corratio` and `woods` are FSL's at every pair (the 8 mm pair, whose few
  voxels make sparse bins, included) and with `-noresample` (whose brightest voxels make one).
- `woods` is `Σ_b n_b·σ_b/μ_b / n` (`specs/mcflirt.md` appendix A's `n_b²` is not FLIRT's: 1.23
  against 2119 at 8 mm), `σ_b` with `n_b − 1`.
- The bin sums are single-precision running sums in the sampling order (one running sum per bin
  over the whole grid); FSL's printed digits agree with them at every pair.
- `-noresample` still blurs the input at each `setscale` (only so do the `normcorr` values agree).

## The image pair's centre

- A `setscale` that only resets the input (§7.2: the data too coarse for the scale) **keeps the
  pair's centre**, the centre of mass of the input the pair was built with (blurred to its
  scale): `centre.sch` prints the parameters of two rotations (`printparams`) after each
  `setscale`; their translations, `(R − I)·c`, give the centre: FSL's stays the 4 mm pair's at
  the 2 and 1 mm scales of 3 mm data. (A fresh centre at each reset moves the default schedule's
  result on the suite's 3 mm BOLD reference by 0.1 mm.)
- The search (§8.1) measures both centres of mass again (the reference level's and the input's):
  with an empty level (a weight of zeros) FSL prints `WARNING::in calculating COG, total = 0.0`
  once more there than once per pair (`-refweight` of zeros: once; `-inweight` of zeros: one per
  pair and one more), also with `-nosearch` (the search runs with no rotations).

## The search and the optimisation (`-verbose 7`)

`flirt -verbose 7` prints every cost evaluation (`<count> : <cost> :: <parameters>`; inside the
search the level is lowered by 2, so the grids' evaluations show at 7). The original's
evaluations, written the same way by a development hook (since removed), were compared with
FSL's one by one on the suite's `-dof 6` registration (`validation/investigations/cleanroom-flirt/`:
`stages.py` compares the rows the default schedule's stages print). What this showed:

- **The grids' parameters ignore `-dof`.** The `search` command's own dof (12 when it has none)
  chooses the coarse grid's subset and the fine grid's scale rule: with `-dof 6` FSL's coarse
  grid optimises the common scale with the translations, its fine grid keeps the median scale;
  `-dof` applies to the minima's free optimisations only (6 parameters there).
- **The grid points and minima start from their sampled parameters** (the angles as sampled,
  e.g. `-1.570796 -1.570796 -1.570796`), not from a decomposition of their matrix (which gives an
  equivalent but different triple of angles, and so different line-search directions), and the
  coarse grid keeps the optimised translations and scale as they come.
- **The convergence test of a subset's optimisation counts the subset's tolerances** (four for
  the search's), not 12: a coarse point whose first sweep moves the scale by 0.072 and two
  translations by 1.3 and 1.8 mm (tolerances 0.016 and 1.6 mm at 8 mm) sweeps again in FSL (sum
  6.4 / 4 > 1), where a count of 12 would stop.
- **The minima are listed z outer, then y, then x** (FSL's `COST minima at : a,b,c` lines), which
  decides which of two near-duplicate minima is kept.
- **`P` holds each start with its own cost** (the cost of the start's matrix, an evaluation FSL
  does not print), not the optimised one.

With these, the two traces agree evaluation by evaluation through the whole 8 mm stage (4,029
evaluations) within 1e-3 in every parameter; they part from the 9th evaluation in the 5th
decimal of a parabolic step (the costs agree in their printed digits; their last bits do not:
neither double sums nor single-precision variants of the correlation ratio's final arithmetic
reproduce the 9th and 10th steps both), and the drift grows to 0.02 mm in the 4 mm stage.

## The pyramid and the outputs

- The pyramid's levels (`-debug`'s `refvol*`, read back): the original's differ from FSL's in the
  last bit of 2.5 % of the voxels (the blur and the isotropic resampling); the rule of §5 kept
  (reversed tap order, float products, double sums: each further from FSL's).
- `-applyxfm` with trilinear and nearest-neighbour interpolation: FSL's images bit for bit. With
  `-interp sinc`, a third of the voxels differ in their last bit (about 1e-7 relative); the
  variants tried (double sums, the product order, a kernel over ±3.5 voxels, the table read
  another way) are each further from FSL's.
- `-paddingsize p`: voxels up to `p` voxels outside the input are interpolated by the extraslice
  rule (a neighbour one voxel beyond an edge reads the edge voxel, further ones the padding
  value), not set to the padding value.
- FSL writes the sform's negative zeros as zeros, and a `NIFTI_PAIR` header of 352 bytes (the
  four extension bytes).
- `-applyisoxfm` with an unset sform (code 0) keeps the reference's sform rows but for the x
  flips of the two grids (`W·F_n·F_n'`, no scaling: 57.75 became 64.75 for 16 voxels of 0.5 mm
  resampled to 3.5 mm on a flipped image); the quaternion's fields follow only a set qform (an
  oblique qform's `quatern_b` and `quatern_c` then differ from FSL's in their last float bit).
- `-applyxfm` writes the image before the `-omat` matrix: with an output type FSL cannot write
  (`FSLOUTPUTTYPE=ANALYZE`) FSL stops with its message and writes neither.
- `convert_xfm -inverse` (and every `inv(A)`, §12) leaves LAPACK's signs of zero: `-0` where the
  inverse of an upper triangle through `dtrti2` scales a zero by minus the inverse of a diagonal
  (the translation of a matrix with no z translation, observed); the original computes the
  inverse in that order (`dgetrf`, then `dgetri`).
- A level of the reference pyramid of 1 x 1 x 1 voxels (a reference of 1 or 2 voxels along every
  axis) is halved from a border-less volume (§10a, FSL-75's empty buffer): refused in both modes
  like the single voxel of the output's padding.
- `-usesqform`'s matrix (`real/applyxfm-usesqform`): FSL's translation differs from the double
  products of §8.5 by 4.5e-6 mm (-13.74984741 against -13.74984288), whichever of the sform and
  qform is used: FSL computes part of it in single precision.
- `convert_xfm -fixscaleskew`: FSL's matrix differs from §10's decomposition and composition (the
  Euler angles from single-precision intermediates, sines and cosines rounded to single) in the
  8th significant digit; no combination of single and double precision reproduces it.

## The boundary-based cost

- With `-wmcoords`/`-wmnorms` the original's matrix is FSL's within 0.0004 mm (`bbr/coords`): the
  cost, the schedule and the optimisation agree; the differences of the `-wmseg` cases (0.13 mm on
  the suite's `bbr/default`, fMRIPrep's three calls 0.01 to 0.05 mm) come from the boundary points
  and their normals.
- `bbrcost.sch` measures the cost of the initial matrix with `bbrstep` 1, 2, 200 and 201 on
  `bbr/default`'s images: FSL 0.537036, 0.540203, 0.566266, 0.554291; the original (the
  segmentation smoothed by `specs/bet.md`'s Gaussian of σ = 2 mm, `[1, 2, 1]` across each
  derivative) 0.537732, 0.541060, 0.568640, 0.556265. Variants tried, each with `[1, 2, 1]`,
  `[1, 1, 1]` and plain central differences: the Gaussian `exp(−k²/(2σ²))` over `2⌊σ − 0.001⌋ + 3`
  taps (σ in voxels; closest: 0.536692, 0.539858, 0.564964, 0.553858 with `[1, 1, 1]`), FLIRT's
  pyramid kernel, no smoothing, σ of 1, 3 mm and 2 voxels: none gives FSL's value. The spec's
  "gradient masks of `specs/bet.md`'s smoothing step" are not in `specs/bet.md` (spec question).
- `-bbrtype local_abs`: 1.4 mm from FSL's (`min(u, 2 − u)` of §6.5); the other types as the
  default.

## The principal axes

- A non-finite moment (`schedule/alignpaxes-nan`): before FLIRT's `Mat::operator(): index out of
  bounds` FSL prints an empty line and Armadillo's `warning: eig_sym(): given matrix is not
  symmetric` on standard error.

## FLIRT's other programs (part 2, relay 1, 2026-11-02)

`avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff` and `makerot`: what black-box
runs of FSL 6.0.7.23's programs showed beyond `specs/flirt.md` §15 (the `fsl-flirt` suite's
`programs/` cases through the oracle cache; further command lines through it; the coordinate
programs reading standard input run directly, as the cache does not key standard input). The
original reproduces each of these.

**`makerot`** (`-o` writes 10 significant digits, which show the last bits):
- The rotation is `(B·R)·Bᵀ` with `B = [u, v, n]`: `n = w/θ` where `w` is the axis scaled by the
  angle (§15.4) and `θ` its length **rounded to single precision**, `n` not normalised again (30
  degrees about z give `R33 = 0.9999999443 = n_z²`, and the translation `(1 − R33)·c_z`); `u =
  (−n_y, n_x, 0)` divided by its length, or `(1, 0, 0)` when that length is exactly 0 (an axis of
  `1e-30, 0, 1` uses the first); `v = n × u` divided by its length; `R` the rotation by `θ` (its
  single-precision cosine and sine) in the `(u, v)` plane, `[[c, s, 0], [−s, c, 0], [0, 0, 1]]`.
  Every matrix of the suite's cases and seven more command lines are FSL's to the last printed
  digit, the signs of zeros (`-0.000000` from a translation of `−2.08e-07`) included; a NaN axis
  component and an infinite angle give FSL's `nan`/`-nan` rows with §15.4's operand orders.
- An angle `θ` below 1e-8 radians (the single-precision value) gives the identity (5.7e-7 degrees
  about z: the identity; 5.75e-7: a rotation).
- The axis and centre values are kept in **single precision**: an axis of `1e-30, 0, 1` behaves as
  `1.0000000032e-30, 0, 1`.

**`avscale`:**
- The **scales and skews** are kept in **single precision**, the rotations and translations in
  double: `Average scaling` of `-1.1 0.1 0 3 / 0.05 0.9 0.2 -1 / 0 -0.1 1.2 2` prints 1.074986 (the
  double mean, 1.0749865469, would print 1.074987), and the half transforms' and the inverted eddy
  matrix's last printed digits on three matrices follow only this choice (the rotation-and-
  translation matrix's entries in double: 0.998610, not 0.998609).
- `--inverteddies`' check matrix of a reflection (the matrix above): FSL prints its third row
  `0.000000 0.000000 1.000000 0.000000`, residuals of about 1e-10 whose signs follow FSL's own last
  bits; the original prints `-0.000000` for the first (every other entry and every other case
  identical). No variant of the rotation's precision gives FSL's signs on both this matrix and the
  suite's (spec question).
- A matrix file that cannot be parsed (a row shorter than the first): FSL prints `Could not find
  enough numbers in matrix file` (its catch) and aborts with it; a volume that cannot be read: the
  image layer's three lines, the catch's two, the abort. A matrix of 4 rows that is not 4 × 4 (4 × 3,
  5 × 5): FSL prints its output up to `Determinant = ` or the half transforms and aborts in
  Armadillo's `det()` or a product (`std::logic_error`); the original refuses it before printing
  (§15). A singular last row (`0 0 0 0`): `Determinant = 0.000000`, `Left-Right orientation:
  swapped`, then `inv(): matrix is singular` (the catch's line) and the abort; an infinite entry, as
  a NaN or a zero column, gives the decomposition's message.

**`rmsdiff`:** a matrix that cannot be parsed aborts without a catch's line; a 4 × 3 matrix aborts
in `det()` (the original refuses it); a second matrix with a singular last row prints the low
determinant warning, then `RMS_DEVIATION ERROR:: Could not invert matrix`, exit 251; as the first
matrix it is used as given. With a mask that equals the reference: `0` and `0`.

**The coordinate programs:**
- An image read whose header has **neither sform nor qform** gets the millimetre matrix of the
  voxel sizes with x reversed about the field of view (`V = F·S`: a 5 × 6 × 7 image of 2 × 3 × 4 mm
  prints ` stdvox2world =` `-2 0 0 8 / 0 3 0 0 / 0 0 4 0`), and **counts as having a coordinate
  space** (no warning); only the empty standard volume (no `-std`) has `V = S` (the identity, 1 mm
  voxels) and the warning (§15.2 says "else its voxel sizes" for both: spec question).
- `-` is an option of `std2imgcoord` and `img2stdcoord` only: `img2imgcoord ... -` prints `Lacking
  argument to option -` (the scan's end) and reads standard input as for no file; an unknown
  option as the last word prints `Lacking argument to option <o>` and the run goes on, with a word
  after it `Unrecognised option <o>` (exit 255); `img2imgcoord -verbose` is unknown; `-flirt`'s
  warning is on standard error.
- Standard input: the prompt (verbose) is printed after the heading, before the terminal refusal,
  which is on standard error (`... | img2imgcoord, OR supply a coordinate file`, the program's bare
  name), exit 1; `std2imgcoord` reads a terminal (no refusal). Its "triple equal to the previous one"
  is compared exactly (`1 2 3` then `1 2 3.0000001` both print) and never to the first triple (`0 0
  0` first prints).
- Numbers are read as libstdc++'s streams read doubles: `nan`, `inf` and `1e400` (out of range) end
  the input; `0x10` reads `0`, then `x10` ends it; triples run across lines (`1 2 3 4 5 6 7 8 9 / 10
  11`: three triples).
- `-warp` of a readable image that is not a warp field (a 3D image): FSL prints `An error occured
  while reading file: <name>`, exit 1; the original refuses every readable `-warp` (§14.1) with its
  own message, exit 1.
- A missing source and destination: `ERROR:: Source volume filename not found` (a blank line), the
  run goes on and the image layer's exception for the empty name aborts it.

## FLIRT's other programs and scripts (part 2, relay 2, 2026-11-02)

Black-box runs of FSL 6.0.7.23 (through the oracle cache; probes in
`validation/investigations/cleanroom-flirt/programs2_*.py`) that the spec does not say, or says
otherwise:

**FLIRT's matrix files** (every program: flirt's `-init`, `convert_xfm`, `avscale`, ...; observed
through `convert_xfm -omat`): a line counts when its first word is a number for C's `strtod`
(`inf`, `nan`, `0x1p3` included; `.x`, `x1`, `# 1 2` and blank lines are passed over); the first
such line's count of words is the number of columns (`1 0 0 0 junk` makes 5, and the next line of
four then throws `Could not find enough numbers in matrix file`); each value is read as libstdc++'s
`operator>>` reads a double: the characters a number may hold are taken (a sign, digits, one point,
an exponent after a digit), and when they are not a whole number (`inf`, `nan`, `2.5e`, `.`, the
`x10` left of `0x10`) the value is 0 and the line's stream fails, so **every later value on the line
repeats the last stored one** (`1 inf 0 0` reads `1 0 0 0`, `0 0 inf 1` reads `0 0 0 0`; after
`1e400`, which reads the largest double, `1 0 1e400 5` reads `1 0 1.797693135e+308
1.797693135e+308`); a line that ends before its values are read throws (`0 1 0` and `0 1 0 ` with
four columns), one whose stream failed does not (`0 1 nan` reads `0 1 0 0`). Part 1's reader took
`inf` and `nan` as numbers and counted the first line's numbers; corrected in this relay (the
programs' 145 suite cases unchanged). Spec question: §2 and §15 say only "as FLIRT reads them".

**`midtrans`:** the transforms are read, each checked by its determinant, before anything is
printed (`-v`'s `Number of matrices` comes after); a matrix with a non-finite determinant counts as
unreadable (`Could not read matrix <name>`, exit 1). `x_n` is computed for every transform after
`Initialising averages` and before `Stage 1` (a 3 × 3 transform's `matrix multiplication:
incompatible matrix dimensions: 3x3 and 4x1` comes there); `--debug` prints `P_<n>` (1-based) and
`Pxx_av` inside stage 1, before `A` (so `Pxx_av` is stage 1's `avg(P_n·(r·I + x_n·x_nᵀ))`, not stage
3's `Pxx`); without a template `Centre of template = ` prints four values (`90 110 90 0`, or zeros
with `--debug`), with one three; `Subject number` counts from 0. A translation of `1e300` makes the
averages infinite: FSL's SVD fails and its run stops at `matrix multiplication: incompatible matrix
dimensions: 0x0 and 3x1` after `Stage 1` (exit 0; refused by the original, rule B, as the spec
says). A template that cannot be read: the image layer's lines, then the exception caught (FSL-78:
exit 0). `--separate` to a directory that does not exist: `Could not open file <p>0001.mat for
writing` per matrix, exit 0. The original's matrices are FSL's to the printed digits in every case
(its SVD is larmorx's, not LAPACK's: within 1e-9).

**`pointflirt`:** both files are transposed when the **input's** has more rows than columns (an
input of 3 × 3 and a reference of 4 × 3: points as columns, then `copy into submatrix:
incompatible matrix dimensions: 3x1 and 4x1`); a missing file reads as empty (`Could not open
matrix file`, then `ERROR: Different number of coordinates`, exit 255: the spec's "FSL aborts" is
not what happens); with `-v` the coordinates are printed (scaled by `--vox`'s voxel sizes, read in
single precision) before the size checks; a NaN coordinate does **not** stop FSL: its SVD returns
an arbitrary matrix (`1 -0 -0 0 / -0 1 -0 -0 / -1 2 -0 0` for one NaN in four points; the spec says
it aborts), refused by the original (rule B). `-o` to a directory that does not exist: `Could not
open file <name> for writing`, exit 0.

**`applyxfm4D`:** the volumes are **not blurred** before resampling (a 1.7 mm T1w onto a 3 mm grid
with trilinear: FSL's image bit for bit without the blur §9.2 describes for `flirt -out`); the
output's sform keeps the reference's negative zeros unless the reference is stored
neurologically (FSL flips it, and the products make its zeros positive); `-v` prints the method as
given (`0, nn`); an input that cannot be read prints only `Image Exception : #63 :: No image files
match: <stem>` before the abort (in 4D and `-3D` alike), the reference the volume's lines too
(`#22 :: Failed to read volume`, `Error : ...`); a matrix is inverted before its product: a
non-square one aborts with `inv(): given matrix must be square sized`, a square one of another size
with `matrix multiplication: incompatible matrix dimensions: RxR and 4x4`. On int16 and uint16
series the original's trilinear images are FSL's bit for bit (FSL-09 truncation with `reproduce`),
its spline with FSL-79 reproduced differs from FSL's in 11 of 327,680 voxels by one unit (the
coefficients' passes in single precision against FSL's arithmetic), its sinc in 17 by one unit;
float series: sinc and spline differ in the last bits of single precision (correlation 1.0000000
with FSL's). A uint16 series is written as int32 (FSL's grouping).

**`extracttxt`:** a negative relative start is subtracted without dc's message (`end$ t.txt 4 -3`
prints lines 10 to 13, no `dc: stack empty`), while `-0` and `+0` as starts, and any sign on a
count, print it once and count as their digits (the script tests the start's sign as a number:
`-0` is not negative); the system's GNU dc 1.4.1 reads `13 -3 + p` as 16 with `dc: stack empty`.
A range read as sed's options prints GNU sed 4.8's usage text after `sed: invalid option -- '<d>'`:
GPL text, not reproduced by the Apache original (one larmorx line instead; the suite leaves the
usage out of FSL's standard error). The script's words reach the shell unquoted: the original
refuses a word that is a shell pattern matching a name in the directory (the shell would replace
it).

**`aff2rigid`:** with one argument the script does not print its usage: it reads the matrix, then
stops at line 105 (FSL-81) like the full call; a missing, non-square or singular matrix stops it
with NumPy's tracebacks (`FileNotFoundError`, `LinAlgError`), which the original does not
reproduce (its own message, exit 1). Its corrected matrix is the replica's in every printed digit
(the suite's two cases).

**`pairreg`:** the schedules are FSL's files in `$FSLDIR/etc/flirtsch`; the original reads them
there and refuses without `FSLDIR` (spec question: their texts, so the original can carry its own).

**The oracle cache** keys FSL's runs without `PATH`: an `aff2rigid` run without FSL's `bin` on the
`PATH` (`/usr/bin/env: 'fslpython': No such file or directory`, exit 127) is served for the same
inputs with it. Not a suite problem (its cases always set it), noted for the cache's policy.

## The verbose levels and `-debug` (part 2, relay 3, 2026-11-02)

Black-box runs of FSL 6.0.7.23's `flirt` at every level from 1 to 7 and with `-debug` and
`-debugsave` (the fsl-flirt suite's cases `verbose/*`, `debug/*`, `bbr/debug`, and the probe
schedules they came from) show where each line is printed and at which level; the original prints
the same lines in the same order (`crates/larmorx-fsl/src/flirt/trace.rs`, the engine, the search
and the optimiser). Every line below is on standard output unless it says otherwise; the levels
inside the search are the run's lowered by 2 (§2.3), the minima's free optimisations at the same
lowered level (not 2 more: their evaluations show at 7).

- **Reading (level 2):** `Refvol intensity clamped between <lo> and <hi>` (the reference's
  clamping range; `Refvol intensity between <min> and <max>`, its raw range, with `-noclamp`),
  `Testvol sampling matrix =` and the input's voxel sizes as a matrix (a single slice's third
  8), `Testvol Data Type = <the input's NIfTI code>`, the input's clamping line, then `Init
  Matrix =` and the initial matrix a first time; level 1's lines follow (with `-2D` neither
  `The output image will use ...` line nor the sform warning, whatever the images hold). The
  first matrix switches the stream to fixed notation, so the reference's line is the only one in
  `%g`. Level 3 adds `CoG for refvol is:  x y z ` and the input's, of the prepared images.
- **Preparation (level 2):** `Subsampling the volumes`, `Resampling refvol isotropically: <the
  minimum sampling>` and four lines describing that level (`Refvol:: Size = (54,80,80,1)`, `Dims`,
  `ROI Size`, `Minimum and maximum intensities are: ...`); then the input as prepared
  (`TESTVOL::`, blurred to 8 mm; with `-noresample` none of the reference's lines and the input
  unblurred).
- **Each `setscale` (level 2):** `Testvol sampling matrix =` and the matrix again, `Testvol Data
  Type = 0`, `Testvol intensity clamped between 0.000000 and 0.000000` (with `-noclamp` the input's
  raw range); at level 3, when the scale makes a new image pair (forced, or the minimum sampling at
  most 1.25 times the scale), `Previous scale used <n> cost function evaluations`: the evaluations
  since the last such line (an input-only reset neither prints nor resets the count; the pair made
  before the schedule resets it silently).
- **`setoption` (level 3):** `setoptions: <name> <each value read as a number> ` and an empty line,
  for every option, known or not (a word reads 0: `costfunction normcorr` echoes `0.000000`).
- **`measurecost`, `gridmeasurecost`, `optimise` (level 4) and the search (level 6):** once per
  command, `Coarse rotation samplings are:` with the three axes' coarse angles, ` and fine
  rotation samplings are:` with the fine ones, an empty line.
- **An optimisation:** at 7 `Starting with <its 12 parameters>` and `  and tolerance <the 12
  tolerances at the scale>` (with a parameter subset its own variables and tolerances, after
  `Using subset cost function`, which shows from level 4); at 5 `Starting with : ` and the matrix
  it starts from, then every evaluation, `<count> : <cost> :: <the optimised parameters>` (with a
  subset all twelve, and at 7 each printed twice and followed by an empty line). With Powell's
  method, at 5, `Applying POWELL correction` and `finit, fend, fextrap = a , b , c` where the
  correction is made, `fval = <cost>` at the end of each iteration. `measurecost` and
  `gridmeasurecost` evaluate without printing their evaluations (they count).
- **The search** (its level, the run's minus 2): at 2 a `*` after each row of the coarse grid and
  a newline, `Median scale = <s>`, a `*` after each row of the fine grid and a newline; at 3
  `(a,b,c) => <cost> with <12 parameters>` for each fine point below the threshold after its
  optimisation, `Costs (1st column) are:` with those points again, `COST minima at : a,b,c` for
  each minimum and the same list as `Cost minima at`, `Chosen parameters:` with their parameters;
  at 1 `After free optimisation, parameters are:` and each minimum's `<cost> ::: <parameters>`,
  then `Parameters (1st column costs) are:` with each kept result (its cost, its matrix by
  columns, its start's cost, the start's matrix by columns); at 4 each coarse point's `Starting
  with <12 parameters>`, `  and tolerance <the subset's 4>` and ` dearranged: <12 parameters>`,
  and `Cost threshold = <t> and minimum is <c>`; at 5 each point's evaluations (the fine grid's
  too) and its `Starting with : ` (the identity for the grid points, the minimum's start for the
  free optimisations).
- **The end (level 2):** the input and the reference as read (`testvol::`, `refvol::`, their raw
  ranges; not when no matrix was found), `Final transform matrix is:` and the matrix; with `-out`
  the output (`Transformed testvol::`) before `Final result:`. `-applyxfm` prints `refvol::`,
  `inputvol::` (as read) and `Resampled volume::` instead.
- **`-debugsave`** writes nothing at level 4: from level 6 (the search's 4) it saves the coarse
  grid's optimised translations (`tx`, `ty`, `tz`) and scales after the median (`sc`), and the fine
  grid's costs after the threshold's optimisations (`costs`), float volumes of the grids' sizes
  (x the first angle), 1 mm voxels, no sform or qform, identity sform rows.
- **`-debug`:** once the images are prepared, the reference pyramid's levels (`refvol_8`,
  `refvol_4`, `refvol_2`, `refvol`: each level's header follows its grid as `-applyisoxfm`'s
  does), their weights (`global_refweight1` to `8`; without weights only `global_refweight1`, one
  voxel of 0, a 1-dimensional image at the minimum sampling's voxel size with no sform or qform),
  the input at 8 mm (`testvol`, the input's header with its sform's negative zeros kept) and its
  weight (`testweight`); at each `setscale`, on standard error `MJ DEBUG OUTPUT: scale = <s>` and
  `MJ DEBUG OUTPUT: min_sampling = <m>`, and on standard output the current pair's input before the
  scale changes it (`testvol DEBUG::` four lines); the boundary-based cost's five lines (`Setting
  bbr seg 1`, ..., `Result (post) of is_bbr_set is 1`, standard error) when a pair's cost becomes
  `bbr` (a new pair with that cost, or `setoption costfunction bbr` on a pair without it); then
  §9.4's final call (the cost of `U`'s first row appended to `U`, before `Failed to calculate any
  transformation matrix` when `U` is empty) and BBR's volume `debug`.
- **Also found:** a `setoption tolerance` with fewer than 12 values makes the next `measurecost`
  or `optimise` FSL's index error (`Mat::operator(): index out of bounds`), with `rel` and `abs`
  alike (§7.3 says "when used"; the original now refuses there too, FSL-68's status).

Every line and file above is the original's, in FSL's order, on the suite's cases (the numbers
aside: at level 5 and more the evaluations follow the optimisation's path, so their count differs
where the matrices differ in their last digits; the suite then compares the lines with each run of
evaluations as one). The pyramid's volumes and the input's are FSL's bit for bit on these cases;
BBR's `debug` volume correlates at 0.9994 with FSL's (it follows the final matrix).

## The spec writer's answers (part 2, relay 4, 2026-11-02)

The spec writer's answers to part 1's questions (`specs/flirt.md` §14, "The clean-room questions of
2026-11-02") are now in the original; what black-box runs of FSL 6.0.7.23 showed while they went
in (the suite's cases, and `-verbose 7` traces of FSL and the original compared evaluation by
evaluation; every FSL run through the oracle cache):

- **The traces now agree.** With §6.2's `corratio` (single-precision sums over the bins with more
  than two points, the two roundings of `η` and of the cost), §3's composition and decomposition
  (the elementary rotations `(B·C)·Bᵀ`, the single-precision scales and skews, `atan2f`), §8.1's
  search (the tolerances of the search-wide `x₀`, the coarse grid held and interpolated in single
  precision, the percentile's single-precision index, the minima's starts from their composed
  matrix, the sorted list's rule), §8.2's line-search unit and convergence weights, and the centres
  of mass in blocks of `max(1000, trunc(√N)) + 1` voxels, `real/dof6`'s `-verbose 7` trace is FSL's
  for its first 3,982 evaluations (part 1: the 9th), the whole 8 mm search and most of the free
  optimisations included; it parts at the 3,983rd, a parabolic step along `t_x` of the free
  optimisation from `rx = −3.118587` (the 6th decimal of the step; the costs agree to their printed
  digits). `real/weights-normcorr`'s trace is FSL's through its 5,449th evaluation.
- **The free optimisations start from the start's decomposition composed again.** FSL's `Starting
  with :` matrix of each minimum, and its row in `P`, is the composition of the decomposed start,
  not the start itself: under gimbal lock (`|r_y| = 90°`, the search's −1.570796) the decomposition
  gives other angles (`−1.570796 −1.570796 −1.570796` becomes `3.141593 −1.570796 0`) and the
  recomposed matrix differs from the start in the 6th significant digit of its translation
  (84.266231 against 84.266223 on `real/weights-normcorr`). The original does the same.
- **Weighted costs multiply the edge factors one by one** (§6.4's "then the edge factors multiply
  it"): `(((w_t·w_r)·w_x)·w_y)·w_z`, not `(w_t·w_r)·((w_x·w_y)·w_z)`; with the product of the
  factors first the weighted `corratio` of `real/weights-normcorr` parts from FSL's at the 2,194th
  evaluation (a scale of 0.248, where most points lie near the input's edges). The shared engine's
  point weight now receives the edge weight (`larmorx_mri::hmc::cost::PointWeight`); `lx.mri.hmc`
  passes none.
- **FSL-90's composition with `-dof 6` happens twice.** With `-anglerep quaternion` and `-dof 6`
  FSL prints `Parameters do not form a valid axis - greater than unity` twice for each starting
  point whose angles' squares sum above 1 (8 lines for the four of `real/anglerep-quaternion-dof6`;
  once each with 12 dof): the 7 parameters are composed first, then the scale dropped. The original
  in `reproduce` composes the same two (§8.1 step 9 says only "with `-dof 6` the median scale
  dropped": spec question).
- **FSL-22 in FLIRT's padding** (`real/applyxfm-nan-border`): with `reproduce`, the order FSL's build
  gives the border's NaN values is libstdc++'s `std::sort` with `<`; the original sorts them so
  (`schedule::std_sort`, an introsort written from its documented behaviour: median of three moved
  to the front, unguarded partition, a heap sort when the depth runs out, an insertion sort over
  the first 16 values and an unguarded one after) and FSL's background value, so every padded voxel,
  is reproduced bit for bit. The schedule's `sort` and `dualsort` use the same order (§7.1: rows of
  equal cost keep the order libstdc++ gives them).
- **The output image.** With §9.3's rules (the edge rule before the blur with `-applyxfm`, the
  spline's coefficients and weights exactly, `-setbackground`'s zero coefficients outside the grid
  and its constant in the trilinear padding) `real/applyisoxfm-4d-blur-setbackground` and every
  spline case are FSL's bit for bit; with §8.5's swapped forms (a single-precision field of view)
  `real/applyxfm-usesqform`'s matrix is FSL's to its last printed digit (part 1: 4.5e-6 mm); with
  §3's decomposition and composition `convert_xfm -fixscaleskew` prints FSL's ten digits (part 1:
  the 8th). With §9.2's sinc table and lookup the sinc outputs differ from FSL's in the last bit of
  1.6 % of the voxels (`real/interp-sinc`: 32,052 of 1,990,656; part 1: a third); a single-precision
  `π` in the sinc's argument is further (646,586), `1 − f` in single precision changes nothing:
  spec question (what else §9.2's model does).
- **BBR's normals** as §6.5 states them (a Gaussian of 2 mm, the gradient's weights 3, 1, 1/3,
  neighbours outside the image 0): every `bbr/` case and fMRIPrep's three calls within thresholds,
  `bbr/type-local_abs` included (part 1: 1.4 mm).
- **Left open** (the record's 8 failures of 305 on the standard tier): `real/cost-leastsq` (its
  trace parts at the 3,952nd evaluation, the first free optimisation's start, `leastsq`'s last
  bits: §6.2 gives no exact order for it), `real/cost-normmi`, `real/weights-normmi`,
  `real/weights-normcorr` (traces part late; FSL's own bands 1.2 to 46.8 mm: unstable
  registrations, a last bit decides), `real/weights-woods` (36 more evaluations than FSL, each
  printing Woods' warning: the search's weighted `corratio` parts as above), and FSL-91's and
  FSL-92's `reproduce` runs (`real/weights-empty-refweight`, `-inweight`, `edge/one-voxel-input`):
  every cost constant, the matrix FLIRT ends at is decided by the ties of its line searches, its
  sorts and its list (the 8 mm search's trace is FSL's but for a few 6th digits of the
  translations of the free optimisations' results, then the 4 mm stage's choices part). By default
  the original refuses all three (FSL-91, FSL-92, rule B).

## Random testing of every program (part 2, relay 5, 2026-11-02)

The trial `difftest --tools fsl-flirt-original --budget 0.4 --jobs 8 --threads 1 --seed 20261103
--shrink-budget 240` (24.4 minutes, realistic cases only, a 24 GiB box, 3 GiB a case; the
descriptor now draws every program of the original, a registration's `-verbose` 2 to 7 and
`-debug`), on `4ba5ada`: 9,854 cases, 7,093 agree, 2,712 differ, 49 notices
(`<workspace>/oracles/difftest/2026-11-02-cleanroom-flirt-2`). extracttxt, midtrans, pointflirt
and rmsdiff agree in every case; aff2rigid in 169 of 170. Found and corrected (black-box runs,
each checked on the trial's cases and the suite's 168 affected cases: no change but the two
FSL-91 cases already open):

- **`-applyisoxfm`'s header** as §9.3 states it: `st = s/d` in single precision, each form swapped
  by the read grid (`W·D⁻¹·F⁻¹·D`, the field of view in single precision) and back by the new
  grid's `n_x` and `s`, every element rounded to single precision (a model of 12 cases: 12 of 12
  of FSL's sform rows, where the voxel-flip form gave 9; `1.5 × 2.3333333` writes `3.4999998`,
  not the tie's `3.5`); an unset qform is swapped likewise (its x offset moves, as the unset
  sform's); the quaternion from the composed single-precision matrix (§3's conversion). NIfTI-2
  outputs carry FSL's single-precision values.
- **An unset form completed** (observed): a registration's `-out` image and applyxfm4D's output
  write an unset sform as the qform's matrix with the qform's code, an unset qform as the sform's
  quaternion and offsets with its code (130 of 135 applyxfm4D runs, every `-nosearch`
  registration whose matrix is FSL's); `-applyxfm` keeps it unset (0 of 443).
- **zstd and bzip2 outputs of every integer type** (char and int images were refused with
  `unexpected output type`).
- **FSL-76, FSL-79, FSL-80 named only where they change a value** (D13): makerot with a zero
  angle or an axis whose two scalings give the same matrix, applyxfm4D's spline of an integer
  image and its `-3D` sinc kernel where the other mode's volume is the same.
- **The coordinate programs: an image without sform or qform** has its voxel sizes as its world
  coordinates, x not reversed (the source in millimetres as well as the destination or standard
  image); `std2imgcoord` and `img2stdcoord` then print their warning of a standard volume without
  coordinates. 20 of the trial's 37 coordinate findings agree now; the 17 others differ only in
  a printed 6th significant digit (the last bits of the mapping).
- **BBR without a boundary voxel**: `ERROR::set_bbr_seg: could not find any boundary points!` once
  each time a pair is made with the cost (a `setscale` makes it twice) and at `setoption
  costfunction bbr`, and at each evaluation the error then `WARNING:: Found NaN in BBR cost (2)`
  (a NaN cost, also `Σ w12 = 0`'s, is warned at the end): FSL's lines on five schedules, line for
  line.
- **Memory**: the coordinate programs and pointflirt's `--vox` read only the header's bytes (89 MiB
  for a 0.8 mm T1w's header, FSL 10 MiB).

Still open (for the next relay; triage as below):
- *avscale and midtrans with a 0.8 mm volume*: 197 MiB against FSL's 130 MiB (the whole file
  decompressed and converted while the decompressed bytes are held: the shared reader's two
  copies);
- *uint8 images in applyxfm4D (sinc, spline) and `-applyxfm`*: values that differ by up to 255
  (9 of 9 `-3D` sinc and 10 of 11 spline uint8 cases): FSL's handling of char data (the type
  group, its conversion) to be measured;
- *the quaternion's signs* of a completed or composed qform (`quatern_b/c/d` of `-0` against `0`,
  or a half turn's sign) in 8 of the replayed applyxfm4D and `-applyisoxfm` findings;
- *`-dof 3`*: FSL prints `Erroneous dof 3 : using 6 instead` more often (95 of 108 cases): 73 times
  for the default schedule with `-nosearch` (part 1's count in the first section: twice per
  optimised row, once per search start, never for `measurecost`), 1599 and 2753 times with the
  search on two of the trial's pairs, where the original prints it once per `optimise` or
  `measurecost` command (19); the search's count to be measured;
- *avscale's skews* printed `-0.000000` by FSL where the original's are `0.000000` (the sign of a
  residual of about 1e-17, relay 1's spec question 2), a rotation angle's 6th digit;
- *registrations* (1,808 of 3,616 differ): paths that part in the costs' last bits and ill-posed
  random pairs (displacement beyond the thresholds with FSL's own variability often 0 on these
  small images), `Parameters do not form a valid axis` counts that follow the path (FSL-90),
  COG-warning counts; pairreg (204 of 230: its three registrations' paths; FSL's own heap
  corruption, FSL-75, in 30 of them);
- *convert_xfm* (7 of 861): the text of six matrices differs (to be looked at).

Part 1's 128 open findings replayed (`difftest --replay`, 8 at a time in a 32 GiB box, memory
findings left out): 47 agree, 81 differ (registrations 76, `-applyxfm` 4, convert_xfm 1); a
stratified sample of 180 of this trial's findings: 50 agree since these corrections.

The thread probes (`tests/parity/test_thread_pools.py`): a one-thread rmsdiff's pool thread was
seen once under its process's name (`python`), before it named itself: the probe now reads such
a thread's name again 2 ms later (`thread_pools.probe`); every original FLIRT program then stays
in its pool (6 runs each).

## The open items of the trial closed (part 2, relay 6, 2026-11-02)

Every FSL run below went through the oracle-output cache (`use`); probes in the scratch helpers
`u8probe.py`, `fprobe.py`, `fprobe4d.py`, `mprobe.py` (standard output and error merged through
`sh`, cached as their own entries) and `dloop.py` (a trial case by hand), committed as
`validation/investigations/cleanroom-flirt/relay6_*.py`.

- **applyxfm4D reads a uint8 image as signed char (new defect FSL-111, rule A).** FSL's applyxfm4D
  keeps an image of the char group in a char image, signed on x86-64: a uint8 value of 128 to 255
  reads `v − 256`. A profile 100, 140 interpolated 0.4 of the way gives 13 (`100·0.6 − 116·0.4`),
  not 116; the background value (the border's 10th percentile) is taken over the negative values
  (180 for a border of 10s and 140 to 250s); nearest neighbour writes the stored bytes back (the
  output's conversion wraps a signed char to its byte) but for the padding voxels. `flirt
  -applyxfm` reads uint8 as stored (the same probes agree). By default the original reads the
  values as stored; with `reproduce` as FSL, named where a written value changes. All 29 uint8
  applyxfm4D findings of the trial are now FSL's bytes (sinc, spline, trilinear, nearest, 3D and
  4D).
- **FSL-79's conversions are x86-64's**: `(T)(c + 0.5)` and `(T)v` truncate through a 32-bit
  integer and wrap to the type's bits (a char coefficient of 160 reads −96; int16 profiles at
  30000 to 32767 wrap), not clamped as the original had them: the int16 and uint8 spline probes
  are FSL's values.
- **A reference with neither sform nor qform**: a registration's `-out` image and applyxfm4D's
  output (not `-applyxfm`'s) take the input's voxel-to-world matrix (its sform when set, else its
  qform, as FSL holds it: swapped for a neurological input) times `D_in⁻¹·A⁻¹·D_ref` (`A` the
  matrix in mm; applyxfm4D the first volume's), products left to right, rounded to single
  precision; written as the sform and as the qform (the quaternion of that matrix, its offsets
  the matrix's), both with the code of the input's form. 25 probes (five inputs: sform and qform
  of different codes, qform only, sform only, neurological, none; two matrices, one a reflection)
  bit for bit; the reflection's positive determinant is written as it is (no flip of the data).
- **Quaternions computed from a matrix are written once**: FSL writes a completed qform (an unset
  qform from the sform) and `-applyisoxfm`'s from their matrix; the original converted them to
  their matrix and back a second time, which moved a last bit (`quatern_c` 0.012488983571529388
  against 0.01248898170888424) and the sign of a zero (`-0` from the negated third column).
- **`-dof 3`'s messages**: once per row of `optimise` and `measurecost` and once per free
  optimisation start (above); 15 of 16 sampled trial cases print FSL's count, the 16th (and the
  trial's #76) differ by the rows their search's list keeps (paths). With `-anglerep quaternion`
  and FSL-90 reproduced, `-dof` below 6 composes the start once (twice only at 6): FSL's message
  order start by start.
- **The 4 × 4 inverse as §12 says**: an upper triangle by the triangle's inverse (its zeros above
  the diagonal `−0`), the LU factorisation left-looking with the pivot's reciprocal, the solve
  against `L` with the columns in pairs: the 7 convert_xfm `-inverse` texts of the trial are
  FSL's. The vector solves (coordinates, avscale's translation, midtrans) use the same
  factorisation.
- **avscale on §3's decomposition** (single-precision scales and skews, `atan2f`): 11 of 12
  sampled avscale findings agree now (the skews' `−0.000000`, the reflection's check row); the
  12th differs in the sign of one residual of the check `M⁻¹·R·K·S` (`−0.000000` against
  `0.000000`).
- **The coordinate programs' qform** is FSL's (the NIfTI-1 library's conversion, x87 steps,
  single-precision elements, `bet::quatern`): 18 of 18 sampled coordinate findings agree (the 6th
  digits of relay 5's list).
- **Nearest neighbour outside the input** (`-applyxfm`, `-applyisoxfm`, `-out`, with a padding
  size): the nearest index first, then the extraslice rule on it (an index one voxel beyond an
  edge reads the edge voxel, further ones the background value): positions 1 to 1.5 voxels out
  take the edge's value (trial #8481: 20 of 120 voxels).

Still open (what the spec does not settle, as spec questions in the handoff):
- the sinc and spline outputs' last bits of float images (spec question 22; applyxfm4D's spline
  of float images likewise, 1.7 to 23 % of voxels' last bit in five trial findings);
- avscale's check residual sign in one of twelve cases;
- registrations' paths (displacement on ill-posed random pairs, FSL-90's message counts during
  quaternion line searches, COG-warning counts, pairreg) as relay 5 lists them.

## `-applyxfm`'s TR (FAST's clean-room original, relay 3, 2026-11-03)

`-applyxfm` and `-applyisoxfm` write the input's TR (`pixdim[4]`) for a 3D input too, as §9.3
says ("the output has the input's TR"), and as FSL reads it: its **absolute value**, and **1 for
an input with a time axis** (`dim[0]` 4 or more, one volume or several) **whose TR is 0**; the
format (NIfTI-1 or ANALYZE 7.5) does not matter; `pixdim[5..7]` are the reference's. The original
wrote the reference's TR for a 3D input and a 4D input's raw `pixdim[4]`. Found by FAST's suite
(`priors/A-a`: FSL's registered priors carry the template prior's `pixdim[4] = 0`, a 3D NIfTI;
FSL's standard priors, ANALYZE with `dim[0] = 4` and `pixdim[4] = 0`, give 1).

Black box (`validation/investigations/cleanroom-fast/applyxfm_tr.py`: FSL through the oracle
cache, every input file keyed; a NIfTI reference with `pixdim[4..7] = 1.7, 1.3, 1.3, 1.3` and the
identity), FSL's output `pixdim[4]`:

| input | TR 0 | TR 2.5 | TR -2 |
|---|---|---|---|
| NIfTI, `dim[0] = 3` | 0 | 2.5 | 2 |
| NIfTI, `dim[0] = 4`, one volume | 1 | 2.5 | 2 |
| NIfTI, `dim[0] = 4`, two volumes | 1 | | |
| ANALYZE, `dim[0] = 3` | 0 | 2.5 | |
| ANALYZE, `dim[0] = 4`, one volume | 1 | 2.5 | |

After a registration (`-dof 6 -out`) the output keeps the reference's TR (unchanged;
`applyxfm_tr_inputs.py`'s images with FLIRT's `probe.py`). The original now reads the TR so in
the apply path (`flirt::engine::apply`): the eleven outputs' headers are FSL's; FAST's 14 priors
cases pass (12, and 2 both-error as before), and FLIRT's 28 suite cases that apply a matrix keep
their statuses (25 pass, 2 both-error, 1 expected divergence). `-verbose 2`'s "Resampled volume"
line keeps its TR as before (not observed otherwise).

## glibc's functions in `reproduce` (the glibc maths part, 2026-11-04)

With `reproduce` the original now calls glibc 2.35's `sinf`, `cosf`, `atan2f`, `exp`, `cos` and
`logf` where `specs/flirt.md` names them (`larmorx_core::math::Libm`; the default keeps the
correctly rounded functions, unchanged). The standard record, made again from `be3db91` and
compared with main's case by case:

- **Toward FSL.** Most registrations' matrices are now FSL's text: `real/dof6`, `dof7`, `dof9`,
  `real/default` (its image too), `default-full`, `init`, `coarse-fine`, `minsampling`,
  `noclamp`, `noresample`, `nosearch`, `usesqform`, `searchcost-normmi`, the four `basescale`
  cases (`basescale-small` was 0.673 mm), `-cost mutualinfo` (0.62 mm), `woods` (8.72 mm),
  `normmi`, the schedules, every verbose level (their standard output now FSL's numbers too),
  the sinc cases' and applyxfm4D's images (FSL's values; `real/interp-sinc` was 32,052 voxels one
  last bit off). Two failures of the list above are closed: `real/cost-normmi` (1.60 mm) and
  `real/weights-normmi` (10.15 mm) were decided by the C library's last bits. fMRIPrep's three
  calls: 0.006, 0.0011, 0.0101 mm before, 0.0000, 0.0002, 0.0065 now. `real/weights-woods`'s matrix
  is FSL's text; it still fails on the line where Woods' warning is printed (one evaluation
  fewer).
- **Away from FSL, within the thresholds.** `bbr/default` (with `bbr/debug`,
  `bbr/fmriprep-form`, `bbr/type-global_abs`) 0.0197 to 0.0693 mm, `bbr/coords` 0.0016 to
  0.0040, `bbr/fieldmap-mask` 0.0386 to 0.0518 (threshold 0.1), `real/bins64` 0.0601 to 0.0692,
  `real/searchr` 0.0330 to 0.1349, `real/weights` 0.2100 to 0.3059 (threshold 0.5).
  `validation/investigations/cleanroom-glibc-dbl/record_moves.py` measures, for each, the default
  against `reproduce` (one program, only the C library's last bits differ): 0.0651, 0.0033,
  0.0415, 0.0984, 0.1463, 0.4251 mm, the size of each move. glibc's functions make the C
  library's bits FSL's, but the rest of the arithmetic (BBR's boundary points and normals, the
  sums' order) is not FSL's bit for bit, so these paths still part and end on either side of the
  cost's flat minimum. The default stays where it was: FSL against the default is main's number
  in every case.
- **The fix passes** of every registration and of applyxfm4D now change values (the default's
  correctly rounded functions against `reproduce`'s glibc ones), reported as "GLIBC-07 (not
  named)" (`parity/defects.py`, `UNNAMED_GLIBC`: the iterative programs name nothing, the user's
  decision of 2026-11-04).
- **Unchanged failures:** `real/cost-leastsq` (81.81 to 82.08 mm; FSL's own 46.76),
  `real/weights-normcorr` (0.5785 to 0.5083, toward FSL; FSL's own 0.1879), FSL-91's and FSL-92's
  three. New in the record (main's suite gained it after main's record; the same with main's
  code): `programs/pointflirt-no-points`, FSL-204, not yet in the original (a follow-up).

## The clean-room catch-up: programs and messages (relay 3, 2026-11-05)

`specs/flirt.md` §14's answers of 2026-11-03, items 1 to 8 of `docs/next-steps.md`'s list for the
original, and what black-box runs of FSL 6.0.7.23 showed on the way. Every FSL run went through
the oracle-output cache (`use`); the probes are committed in
`validation/investigations/cleanroom-flirt/` (`catchup_probe.py`: FLIRT's programs on given
files and seeded images; `catchup_finding.py`: a random-testing finding's shrunk case, FSL against
the original in both modes, with extra words such as `-verbose 7`; `catchup_merged.py`: the same
with standard output and error merged through `sh`, to place a warning between verbose lines;
`catchup_replay.py`: a trial's findings replayed by program).

- **The free optimisations' verbose level** (§2.3): the code already ran them at the search's
  lowered level; `verbose/5` and `verbose/7` confirm it (FSL's lines).
- **Quaternion starts** (§8.1 step 9, FSL-90 with `reproduce`): a composition whose vector part
  lies outside the unit ball keeps only its scales and skews about the centre (no translations: the
  invalid starts at 6 dof start from the identity), composed once (the second composition at 6 dof
  is gone). FSL's messages at every such composition, evaluations included, were already the
  original's; their **order** was not: FSL composes a start, prints its dof message (`-dof` below 6)
  and runs its optimisation, start by start, where the original composed every start first. Each
  free optimisation now writes its own log, passed on in the starts' order (deterministic at any
  thread count; the grids' evaluations, whose lines are all alike, still write to the run's log).
  With `-dof 3`: the 8 validity messages and the 102 dof messages interleaved exactly as FSL's
  (`real/anglerep-quaternion-dof3`); with 6 and 12 dof FSL's 8 and 4; the matrices FSL's text at 3
  and 6 dof. The `-verbose 7` traces at 6 and 12 dof are FSL's through about 5,050 lines and part at
  the 4 mm stage, at a near-zero skew's sign (`0.000000` against FSL's `-0.000000`): item 13's
  question.
- **avscale's check signs** (§15.1, §12): the 82 avscale findings of the 2026-11-02 trial are FSL's
  text with the current code (§12's inverse and glibc's `atan2f`, both in place since part 2 and the
  glibc maths part): nothing left.
- **midtrans' determinant test** is `|det| < 1e-6`: a determinant that overflows (a scale of 1e200)
  passes it and the transform is used (the original read it as unreadable). FSL then writes rows of
  NaN with exit status 0, and a translation of 1e300 makes its decomposition fail, reported as
  `matrix multiplication: incompatible matrix dimensions: 0x0 and 3x1` with exit status 0: **FSL-241**
  (new, rule B: the transforms cannot be averaged; FSL's values are not specified, so the original
  refuses in both modes). The replica, run as a black box on the same files, writes FSL's NaN rows
  and refuses the failed decomposition, in both modes.
- **pointflirt** (§15.6): no points at all (both files empty, missing or of words only) is FSL-204:
  FSL prints its `Could not open matrix file` lines and, with `-v`, its two headings with empty
  matrices, then crashes; the original prints the same and refuses (exit status 1), in both modes;
  `--vox` without its volumes still gives `--vox`'s own error first (FSL's order). A `nan` word reads
  0 (the fit of the points as read: the same bytes as with 0). An overflow (`1e400` times a voxel
  size above 1, or `1e200`, whose square overflows) stops as FSL's SVD does (`Mat::operator():
  index out of bounds`, FSL killed by signal 6, the original exit 1): the original refused before.
- **`-warp`** (§15.2): FSL accepts exactly three volumes; a 3D image, one, two or four volumes print
  `An error occured while reading file: <name>`, exit 1, after the verbose lines so far (observed on
  all four). The original reads the volume count from the header; three volumes are refused until
  FNIRT's warp fields are ported.
- **FSL's 4 × 4 inverse fails only on an exact zero** (§12): NaN and infinite matrices are inverted
  (the LU's `(−1)·t + y` and `x_k·(−1)` as products, so a NaN keeps its sign there), the subsets'
  `x₀ − x₀` computed as written (a NaN parameter's own NaN, x86-64's default NaN for an infinite
  one), and with `reproduce` FLIRT goes on with its NaN centres and matrices and writes them: 37 of
  the 57 findings of the two 2026-11-02 trials that stopped at `inv(): matrix is singular` now agree
  (three permanent cases, `validation/difftest/cases/fsl-flirt-original/nan-matrices-*.json`). The
  signs of the NaN matrices still differ (FSL `-nan` in every element, the original `nan` in the top
  rows: finding 2905's trace parts first at a subset optimisation's start; §14's open question 2).
  Of the other 20: 16 registrations with NaN or infinite input voxels whose matrices or images
  still differ (the one studied, finding 5009: FSL's free optimisations end at rotations 0,
  translations NaN, and FLIRT's final matrix is its `-init` matrix, where the original's
  compositions carry the NaN centre into every element: how FSL's arithmetic keeps the 3 × 3 part
  finite is a question for a spec writer or for item 13), 3 ill-posed paths (displacement beyond the
  thresholds) and pairreg's finding 2381 (FSL's heap corruption, FSL-75, in its first run).
- **`convert_xfm -fixscaleskew`** aborts where its decomposition does not solve, as avscale's (a
  zero scale or a non-finite entry: `matrix multiplication: problem with matrix inverse; suggest to
  use solve() instead`, signal 6): a matrix file of `-nan` words reads as zeros (§2.2), which FLIRT's
  NaN matrices in pairreg's third step are (observed: both argument orders). In `pairreg` a step that
  aborts is followed by the shell's `Aborted` line, as FSL's script prints it (findings 3569, 5082,
  9483 now agree).
- **pairreg's schedules** (§15.8) are carried by the original (`flirt::schedule::PAIRREG_1` to `_3`,
  their text taken from FSL's `-v` echo, one black-box run: every line, blanks and comments as FSL
  prints them, `pairreg3.sch` with two final empty lines); `pairreg` runs without `$FSLDIR` (FSL's
  matrix and `-v` text, `programs/pairreg-verbose`).
- **Verbose lines found on the way** (FSL's observed, now the original's): `-verbose 3` measures
  each centre for its `CoG for ... is:` line and prints the COG warning there (an all-zero
  reference: `verbose/3-empty-reference`); a level's fourth size in `Refvol::`, `TESTVOL::` and
  `testvol DEBUG::` is the image's `pixdim[4]` as read (FSL prints 0 for images without a TR; the
  original printed 1); the `Testvol sampling matrix` and `Init Matrix` print in scaled millimetres
  (0.5 mm voxels and a base scale of 0.5 print 1 and an `-init` translation of 1.5 as 3); a 4D image
  in a registration is warned about twice (`An input intended to be a single 3D volume has multiple
  timepoints ...`), the second time after the sform warning (FSL reads both images twice), the
  reference included (`verbose/2-basescale-tr`).

## The clean-room catch-up: numerics, `-debug` and NaN centres (relay 4, 2026-11-05)

Items 9 to 13 of `docs/next-steps.md`'s list for the original (`specs/flirt.md` §14's answers of
2026-11-03), and what black-box runs of FSL 6.0.7.23 showed on the way. Every FSL run went through
the oracle-output cache (`use`); the probes are the catch-up's (`catchup_finding.py`,
`catchup_probe.py`, `catchup_replay.py`) and `catchup_schedule.py` (FLIRT on a finding's inputs
with a schedule of the probe's own, its lines written from FSL's `-v` echo, `print` lines added to
see a list), all in `validation/investigations/cleanroom-flirt/`.

- **`leastsq`** (§6.2, item 12): three levels of single-precision sums (row, slice, total) of
  `δ·δ` and the count without smoothing or weights, of `(w·δ)·δ` and `w` otherwise; the quotient
  single when the weight (count) is above 1, else `(max − min)²`. `real/cost-leastsq` passes (it
  failed at 82 mm with double sums).
- **applyxfm4D's spline** (§15.7, item 11) is FLIRT's own (§9.3) instead of mcflirt's: a float
  image's passes stored in single precision, a double image's values read and filtered in double
  (each value still rounded to single). The five trial findings the spec writer modelled (#368,
  #4718, #3494, #6140 float, #6013 double) differed in 47, 4,430, 177, 342 and 181 voxels, 5,177
  in all: 0 now (permanent cases `applyxfm4d-spline-float`, `-double`).
- **`-debug`'s volumes** (§9.4, item 9): each level's header from the reference by the
  resampling's steps, `level/d` per axis in single precision (`d` the voxel sizes as FLIRT holds
  them, scaled by the base scale), composing only a set form, every form of a neurological
  reference (an unset one too) swapped back with the level's grid, the elements in single
  precision and the quaternion computed from them, as `-applyisoxfm`'s header (one function now
  serves both); `global_refweight1` without weights has `pixdim[0]` (qfac) and `pixdim[4..7]` 1.
  Observed beyond the spec: with `-noresample` the levels are the reference itself (its forms as
  read, the held voxel sizes as `pixdim`) and, without weights, no `global_refweight1` is written;
  `testvol`'s `pixdim` are the input's held voxel sizes (scaled by the base scale; trial #175).
  With the boundary-based cost and no boundary point, `-debug`'s five lines surround the set-up
  and its error (`Result (post) of is_bbr_set is 0`), the boundary counts as not set, so the
  set-up, its lines and its error come again before every evaluation (then the NaN warning); the
  final call writes `debug` (zeros), and FSL goes on to `-omat` (the `-init` matrix) and `-out`.
  One evaluation's lines are pushed together (the grids' evaluations run in parallel). The first
  pair's set-up comes after `-verbose 3`'s centres (their COG warnings first; trial #1308).
  The 161 `-debug` findings of the 2026-11-02 trial: 52 agreed before, 123 now.
- **A perturbation is composed with all twelve parameters** (observed, `schedule/perturb-scaled`):
  `measurecost 6` of a scaled and skewed row keeps its scales and skew, `measurecost 7` with a
  scale perturbation changes only the first scale (`1.2 → 1.3`, the others kept): the row is
  decomposed, perturbed and composed again whatever the command's dof (the original composed the
  command's dof, at least 6, before: a row's non-rigid parameters were lost). An optimisation's
  `Starting with :` matrix is its start's first `dof` parameters composed (a scaled start at 6
  dof shows the rigid matrix, at 7 its first scale), its `Starting with` line the start's twelve.
- **The search's `P`** keeps each start as composed once (§8.1 step 9: "the starting matrix kept
  in `P` is the composed start itself"), its cost that matrix's; the free optimisation starts from
  its decomposition. Under gimbal lock the two differ (FSL's 84.266223 against the original's
  84.266231 before, on `real/weights-normcorr`, whose `-verbose 7` trace now follows FSL's to line
  7,556 instead of 7,467).
- **NaN centres** (FSL-14 reproduced: NaN or infinite input voxels), item 13: FSL prints an
  optimisation's NaN starting parameters, then evaluates from the identity's (rotations,
  translations and skews 0, scales 1: `1 : -nan :: 0.000000 ...`), so a 6-dof optimisation
  composes the identity exactly and a 12-dof one (its skew and scale about the NaN centre) all
  NaN; the original now does the same. The coarse and threshold grids' `Starting with :` shows the
  identity's parameters composed about the centre (the identity, all NaN about a NaN centre); a NaN
  fine-grid scale is not outside `[0.5, 2]`, so it is kept; a NaN pre-image (a NaN matrix) is
  outside the input: the output voxel is the background value (FSL's whole image one value,
  the original's NaN before). Of the 57 findings of the two 2026-11-02 trials that stopped at
  `inv(): matrix is singular` (37 agreed after relay 3), 52 agree now; left: #1760 (an
  ill-posed path: FSL's own variability 22 mm), #3788 (a registration 211 mm away, not studied),
  #7221 (the
  `normmi` cost of a translation whose overlap reaches the NaN voxels, `-1.305830` against FSL's
  `-1.000000` at evaluation 119,956 of 120,000, every earlier line FSL's), #4404 (its image's
  correlation, not studied) and pairreg's #2381 (FSL's heap corruption, FSL-75). Finding #2424 (BBR, no
  boundary, infinite voxels, `-debug`) is FSL's in every file and line but the cost object's
  address (permanent case `debug-bbr-no-boundary`).
- **The C library's functions in `reproduce`** (item 10): wired since the glibc maths part
  (`sinf`, `cosf`, `atan2f`, `exp`, `cos`, `logf`); the spline's horizon now uses the correctly
  rounded logarithm (`larmorx_core::math::log`; the platform's `ln` before; the same 15 terms).
- **Still open** (for a spec writer; the record's failures): FSL-91's and FSL-92's `reproduce`
  traces (`real/weights-empty-refweight`, `-inweight`, `edge/one-voxel-input`, every cost equal)
  are FSL's through line 66,124 of 140,494 and part at a free optimisation's final line, the sign
  of a zero third skew under gimbal lock (`r_y = −π/2`: FSL `-0.000000`, the original `0.000000`),
  after which the next start's angle differs in its 6th digit (`-0.327782` against `-0.327783`);
  the compositions and decompositions of 50 gimbal-lock angle triples through `setrowparams`
  and `measurecost` print FSL's rows exactly (probes `gimbal.sch`, `gimbal2.sch`), so the sign
  comes from a step the spec does not state. FSL's first evaluated vector also differs from its
  printed start in a zero's sign (`Starting with 0 -0 -0 ...`, first evaluation `0 0 -0`; probe
  `shown.sch`). `real/weights-normcorr` (0.5083 mm, FSL's own 0.1879) and `real/weights-woods`
  (Woods' warning one line more) part at the 4 mm stage: evaluation 47 (`0.050021` against
  `0.050056`, the costs equal to their 6 printed digits) and 356 (`0.948010` against `0.947916` at
  the same printed parameters), the weighted costs' last bits.

**The record** (`fsl-flirt`, re-made from `23710fe`, standard tier; `compare_records.py` against
main's, made from `be3db91`): 333 cases (308 before): 229 pass, 89 rejected by both, 10 expected
divergences, 5 failures. `real/cost-leastsq` passes (82 mm before); `programs/pointflirt-no-points`
is an expected divergence (FSL-204, relay 3); `real/cost-leastsq`, `real/bins64` (0.0692 mm before)
and `real/weights` (0.3059 mm) now write FSL's matrices as text; fMRIPrep's three calls 0.0000,
0.0002 and 0.0113 mm (ds005040's 0.0065 before: its boundary-based path, whose costs are not FSL's
to the last bit, parts elsewhere since a perturbed row keeps its scales); 25 new cases (relay 3's
14, relay 4's 3 and its 8 permanent cases, all passing but `nan-matrices-forcescaling`, rejected by
both). The five failures are the "Still open" list above. Memory: the changed paths
(`applyxfm4D-spline`, the registrations) stay within the memory test's bounds (5 calls); golden:
`cli.fsl.flirt`, `.costs`, `.reproduce`, `.schedule` and `cli.fsl.applyxfm4D` move (the
perturbations' twelve parameters, `leastsq`'s sums, the spline), to be re-recorded once with the
part's other moves.

## The catch-up's random-testing trial (relay 8, 2026-11-05)

`oracles/difftest/2026-11-05-catchup-flirt` (seed `20261103`, the 2026-11-02 trial's, so its first
4,240 cases are that trial's drawn again; 30 minutes, 14 jobs at one thread, 3 GiB a case, the
oracle cache in `use` mode): 13,907 cases, 12,334 agree, 1,560 differ, 13 notices (FSL-09, -79,
-80, -111, -187). Of the 1,140 differences of the cases drawn again, 696 agree now; 46 cases that
agreed then differ now (36 registrations' paths, 7 the headers below, 2 `labeldiff`, 1 pairreg).
Fixed (each with a permanent case, `validation/difftest/cases/fsl-flirt-original/`):

- **The search's minima on a one-point grid** (17 cases, `search-cost-minima-fallback`): with no
  local minimum (`-nosearch`: the grid's only point is the last index on every axis) the overall
  minimum is listed only on FSL's `Cost minima at` line, without a `COST minima at` line
  (observed, `-verbose 7`); the original listed both.
- **A sform completed from a qform-only reference keeps its negative zeros** (25 cases,
  `out-completed-sform-negative-zero`): with `qfac` −1 the qform's zero entries times −1 are
  negative zeros, which FSL's `-out` writes; a stored sform's negative zeros are still written as
  zeros (relay 3's observation), and a neurologically stored reference's are positive (its swap's
  products).
- **applyxfm4D's padding of an int32 image** (1 case, `applyxfm4d-int32-padding`): FSL writes the
  background value exactly in the type (2147483647) and the interpolated voxels through single
  precision (2³¹, wrapped to −2147483648: FSL-09), observed with `catchup8_int_nn.py` (FLIRT's
  `-applyxfm` pads through single precision: unchanged); the original keeps an int32 image's
  values when some lie beyond 2²⁴ (`bet::image::read_i32_series`) and writes the padding from them.

Open (in `validation/difftest/triage.toml`): 941 registrations ending elsewhere on the trial's
random, often ill-posed pairs and 51 with `labeldiff`; messages whose count follows the path (166
COG warnings, 172 FSL-90 quaternion messages); 110 pairreg paths and 12 runs where FSL crashes
(FSL-75); and, for the spec writer, 9 searches whose test volume has no intensity range at a scale
(`Testvol intensity clamped between 0 and 0`): FSL prints `WARNING: Found 0 or less
sub-threshold costs`, its fine grid's costs and parameters print as 0 and every point becomes a
minimum, where §8.1 steps 6 and 7 give the cost of no variance (1) and an optimisation of every
point (`20261103-08267`, `03733`).


## The search's starting points of equal cost (relay 10, 2026-11-05)

Found through BET's trial (standard_space_roi's matrices 37 to 303 mm from FSL's on small heads):
FLIRT's search builds its list of starting points as §8.1 step 9 says (each new entry after every
entry of lower or equal cost), and FSL then **sorts that list by cost with libstdc++'s `std::sort`**
before appending it to `S` and `P`; `std::sort` is an introsort, not stable, so starting points of
equal cost take its order, not the order found. Observed (`trial_ssroi_matrices.py`, a schedule of
our own: `search`, then `print S` and `print P`): on a pair whose 829 starting points all cost 1
(a test volume clamped between 0 and 0 at every scale; FSL's own `measurecost` gives 1 for both
programs' matrices and for the identity), FSL's `S` and `P` are exactly the introsort's permutation
of the list as `-verbose` prints it (`P` permuted alike); with 20 rows put in `S` before the search,
those 20 stay first and only the search's rows are permuted. The original kept the order found;
it now sorts the search's results with `schedule::std_sort` after printing them
(`flirt/search.rs`; unit test `std_sort_orders_equal_costs_as_fsl_orders_its_search`, the observed
first 16 and last 20 places; permanent cases `search-equal-costs-sorted`,
`pairreg-search-equal-costs-sorted`, which differ with the build before).

What it changes: on random pairs that carry little information at the coarse scales many costs
tie. Replayed (`catchup_replay.py`, the trial's findings of the three path classes with the final
build and with the build before the fix; `trial_catchup_replay_sorted.txt`,
`trial_catchup_replay_before_sort.txt`): 873 of the 1,471 findings agree now and differed before
(755 registrations ending elsewhere, 101 pairreg runs, 17 message counts; one agreed before); 338
registrations, 236 message counts and 23 pairreg runs still differ. The record `fsl-flirt` re-made
from `a835ddd`: 339 cases, 238 pass, 89 both refuse, 10 expected divergences, 2 fail; three cases
move from fail to pass (`real/weights-empty-refweight`, `real/weights-empty-inweight`,
`edge/one-voxel-input`: an empty weight or a one-voxel input makes every cost equal: the order of the equal
costs decided their matrices, whatever the zero's sign where relay 4's traces part), and two difftest cases'
displacement checks are no longer needed (their matrices FSL's). The two failures left are the
weighted `normcorr` and `woods` costs' last bits (relay 4). FAST's record (its `-a` priors
registered by this FLIRT) and BET's are unchanged.
