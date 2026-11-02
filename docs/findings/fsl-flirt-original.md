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
