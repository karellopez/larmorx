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
pairs (and with smoothing 0 and fuzzy fraction 0) for every cost; FSL against the original
(`costs.py`):

- `normcorr`, `mutualinfo`, `normmi`, `labeldiff`: the same printed values (6 decimals) at every
  scale, also weighted (`-refweight`, `-inweight`); `leastsq` to 7 significant digits.
- `corratio`: the same values at the 4 and 2 mm pairs and at every smoothing (0 to 16 mm), but
  not at the 8 mm pair with 32 or 64 bins (`0.984415` against `0.982248`; with 16 bins, `-bins
  128`, the same) nor with `-noresample` (relative 3.5e-5). No bin count reproduces FSL's value,
  and none of these variants does: unweighted counts, biased variances, every bin counted,
  appendix A's binning, a top bin of its own, the roles of the two images swapped. Weighted, FSL's
  8 mm value is above 1 (`1.016`): its weighted form is not the weighted moments of §6.2.
- `woods`: FSL's is `Σ_b n_b·σ_b/μ_b / n` (`specs/mcflirt.md` appendix A's `n_b²` is not FLIRT's:
  1.23 against 2119 at 8 mm), with the same 8 mm and `-noresample` mismatch as `corratio`
  (relative 1e-3).
- `-noresample` still blurs the input at each `setscale` (only so do the `normcorr` values agree).

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
- `-usesqform`'s matrix (`real/applyxfm-usesqform`): FSL's translation differs from the double
  products of §8.5 by 4.5e-6 mm (-13.74984741 against -13.74984288), whichever of the sform and
  qform is used: FSL computes part of it in single precision.
- `convert_xfm -fixscaleskew`: FSL's matrix differs from §10's decomposition and composition (the
  Euler angles from single-precision intermediates, sines and cosines rounded to single) in the
  8th significant digit; no combination of single and double precision reproduces it.

## The boundary-based cost

- With `-wmcoords`/`-wmnorms` the original's matrix is FSL's within 0.0004 mm (`bbr/coords`): the
  cost, the schedule and the optimisation agree; the differences of the `-wmseg` cases (0.05 to
  0.11 mm) come from the boundary points and their normals. Three gradient masks tried for the
  normals (central differences, `[1, 1, 1]` and `[1, 2, 1]` across the axis): 0.108, 0.091 and
  0.107 mm; the spec does not name the masks.
- `-bbrtype local_abs`: 1.2 mm from FSL's (`min(u, 2 − u)` of §6.5); the other types within
  0.11 mm.
