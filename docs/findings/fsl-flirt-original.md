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
- `Erroneous dof <d> : using 6 instead` (a `-dof` below 6): printed twice for every row a
  schedule's `optimise` optimises, once for every starting point of the search's free
  optimisations, never by `measurecost` (73 lines for the default schedule with `-nosearch`:
  36 optimised rows twice and one minimum).
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
