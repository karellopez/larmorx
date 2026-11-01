# Findings: FSL FLIRT (FSL 6.0.7.23)

**Restricted: written from FSL's source.** A clean-room implementer must not read this file
(`docs/porting-guide.md` §1); what it learns about behaviour is in `specs/flirt.md`.

What the replica (`crates-nc/larmorx-nc-flirt`, part 1, 2026-11-01) found in FLIRT 2111.4 (FSL
6.0.7.23) and the libraries it calls: how FLIRT computes its results, where FSL's code does
something undefined or questionable, and what larmorx does. Each entry says how it is known:
*read* (the source), *verified* (a probe or a black-box run), *validated* (the parity record).

## How FLIRT works, for the replica

- **The image pair holds references** (*read*, *validated*). `Costfn` keeps `const volume<float>&`
  to `main`'s `refvol`, `refvol_2/4/8` and `testvol` and to the globals `global_refweight` and
  `global_testweight`. `usrsetscale` first re-reads the test volume into `main`'s `testvol`
  (`get_testvol`), then, only if the scale is coarse enough (`min_sampling <= 1.25·scale`, or
  `force`), blurs it and builds a new pair. When the condition fails, the previous pair stays and
  now sees the clamped, unblurred input: with 3 mm BOLD data (`min_sampling = 3`) the default
  schedule's `setscale 2` leaves the 4 mm pair in place with the unblurred input, and every cost of
  the 2 mm stage compares the 4 mm reference with the full-resolution input. This decides FLIRT's
  results on every 3 mm input; the replica resolves the pair's volumes at each evaluation to give
  the same. The pair's centre of mass (`testCog`, the centre of the rotations) is computed when it
  is built, from the input of that moment. FSL-69 (rule C: kept and documented).
- **`final_transform`'s BBR pair** (*read*): with `-pedir`, the final image is resampled through
  `Costfn::bbr_resamp` on a new pair of the final volumes, whose `setup_costfn` recomputes the
  boundary points (and prints the "no boundary points" error again when there are none).
- **The schedule's word splitting** (*read*, *validated*): `striptrailingspace` does nothing when the
  last non-blank character is the string's first (`if (i2!=0)`), so `clear U  ` gives the words
  `clear`, `U` and an empty word, and a line of blanks one empty word (an unrecognised command).
  Reproduced (it is only parsing; the messages are FSL's).
- **`copy X X`** re-reads the list's size at every row (`crow<=Min(usrsrcrow2,usrsrcmat->size())`),
  so it appends the list's own rows until row `r2` (999999 rows by default). Reproduced.
- **`usrread`** loops `while (!fptr.eof())`: a token that is not a number sets the fail bit, never
  the end of file, and the loop pushes rows for ever (memory exhausted). Refused (FSL-72); a file
  ending without a blank after its last number loses its last row (eof set while reading it):
  reproduced.
- **The pseudo-inverse of the parameter-subset masks** (*verified*: `validation/probes/fsl-flirt-pinv`).
  `params12toN` calls `pinv(parammask)` (MISCMATHS → `arma::pinv(A, 0)`, Armadillo 14.2.1's
  divide-and-conquer SVD, LAPACK `dgesdd` on FSL's OpenBLAS). For the search's 12 × 4 mask (a
  column of three ones for the common scale, three unit columns) the scale row is
  `0x1.5555555555557p-2` (two units above the rounded 1/3) = `(1/√3)²` rounded; for `sch2D_4dof`'s
  column of two ones (one scale for x and y, beside rz, tx, ty) `0x1.ffffffffffffep-2` = `(1/√2)²`;
  unit columns (FLIRT's identity, the other shipped schedules' `paramsubset`s, the 2D subset) give
  exactly 1: `check_masks.py` confirms 105 such masks. Other masks (a single column of several ones, such a
  column beside others, values other than 0 and 1) give other last bits and small off-support
  values (17 rows of 21 such masks differ from the closed form), so they need `dgesdd` itself: the
  replica refuses them at `setoption paramsubset` until part 2. The products `pinv·d` and `parammask·p` go to
  OpenBLAS's `dgemv_n` (Haswell's 4 × 4 microkernel with FMA), but for these masks every sum has at
  most the products of one mask column, added left to right, and the FMA with α = 1 is an addition:
  plain sums give the same bits.
- **The optimiser** (*read*, *validated*): FLIRT uses `MISCMATHS::optimise` with up to 4 (or the
  schedule's) iterations, the convergence test `SP(initpt - pt, inv_tol).SumAbsoluteValue() < 1`
  (Armadillo's `accu`: two interleaved accumulators) and Powell's update when
  `optimisationtype powell` (bbr.sch). mcflirt's one-iteration use never reached the test or Powell;
  the core's `optimise` now translates all of it (mcflirt's results unchanged: its standard-tier
  record re-made, 236 of 236 cases the same).
- **`labeldiff`'s corner weights** (*verified*, `validation/probes/fsl-flirt-costs`): the source's
  types decide the arithmetic: `(1.0-dx)*(1.0-dy)*(1.0-dz)` is double, but `dx*dy*(1.0-dz)` takes
  the float product `dx*dy` first and `dx*dy*dz` is a float product added in float (`addss`, the
  disassembly of `p_labeldiff`). A first translation computed all eight in double: one float unit
  apart on rotated matrices; corrected, bit-identical on every probed matrix.
- **Weighted normalised correlation** (*read*): `cost(affmat, refweight, testweight)` returns `1.0 -
  normcorr_fully_weighted`, without `fabs` (the unweighted path is `1 - |r|`): an anticorrelation is
  rewarded only without weights. Reproduced (a design question, not a bug: rule C).
- **`set_bbr_seg`'s normals** (*read*): the gradient of the smoothed segmentation (voxel units of the
  segmentation's grid) is divided by `refvol.xdim()`, `ydim()`, `zdim()` — the *pair's reference*,
  which FLIRT has resampled isotropically to `min_sampling` — before normalisation. For an isotropic
  reference level the division changes nothing after normalising, so the normal is the gradient in
  the segmentation's voxel units, not in mm: on an anisotropic segmentation (fMRIPrep's T1w masks,
  0.85 × 0.83 × 0.83 mm) the directions tilt slightly. FSL-67 (rule D: to be measured, part 2; kept
  meanwhile, both modes).
- **FSL 6.0.7.17's step** (*verified*): fsl-flirt 2111.2 and 2111.4 differ only in
  `configure_interpolation` (moved out of `final_transform`, same settings); FSL 6.0.7.17's and
  6.0.7.23's flirt give the same matrices and images byte for byte on ds005040's default and
  fMRIPrep BBR calls (Armadillo 10.5.1 against 14.2.1 changes no bit there).

## Defects (D13; register entries through the part's handoff)

| ID | What | Class | Rule | larmorx |
|---|---|---|---|---|
| FSL-09 | output types: truncation toward zero, int8 written as uint8 (`save_volume_dtype`, as mcflirt) | c | A | corrected by default; `reproduce` truncates |
| FSL-11 | `FSLOUTPUTTYPE` unset | e | (decided 2026-10-23) | `.nii.gz` |
| FSL-14 | a NaN voxel makes a cost NaN | c | A | non-finite points left out by default |
| FSL-21, FSL-66 | `-bins` giving no or negative bins at a scale (`no_bins/8` at 8 mm: `-bins` below 8) | a | — | refused (heap corruption, division by zero, `bad_array_new_length`), both modes |
| FSL-56 | NIfTI extensions' offset | c | A | corrected by default |
| FSL-67 | BBR normals divided by the reference level's voxel sizes | c? | D, then C (measured 2026-11-01) | kept in both modes: no measurable gain |
| FSL-68 | every error after the command line exits with status 0 (`main` catches all exceptions) | e | B | exit status 1 by default, with FSL's message; `reproduce` exits 0 |
| FSL-69 | an older image pair compares with the unblurred input after a `setscale` the data are too coarse for | c? | C | kept (pipelines' results depend on it), documented |
| FSL-70 | a schedule matrix name FSL cannot interpret (`Uq`, an empty word) or `U[` (a 28th list): an uninitialised or out-of-bounds pointer | a | — | refused, both modes, after FSL's message |
| FSL-71 | `setoption bbrstep` below 1: the boundary loop never ends (0) or wraps (negative) | a | — | refused, both modes |
| FSL-72 | `read` of a file with a word that is not a number: an endless loop | a | — | refused, both modes |
| FSL-73 | `-pedir` without a field map read (no `-fieldmap`, or `-applyxfm`, which never reads it): an empty field map interpolated | a | — | refused, both modes |
| FSL-74 | the usage lists `-displayinit` (the option is `-printinit`) and leaves `woods` out of `-cost` | f | — | the same text (messages kept) |
| FSL-75 | newimage's `calc_bval` (the background value, edge width 2) sizes its buffer too small for an image 3 voxels wide along x (with y or z ≥ 2), or along y (z ≥ 2, x not 2 or 4), and empty for 1 × 1 × 1: heap overflow, or a read of an empty vector's first element | a | — | refused, both modes, where FLIRT computes it (part 2) |

FSL-21 and FSL-66 are mcflirt's entries; FLIRT meets them through `-bins` (each scale has
`trunc(bins/scale)` bins). FSL-09, FSL-11, FSL-14, FSL-56 are the shared ones of FSL's libraries.

## Random testing: the trial of 2026-11-01

`difftest --tools fsl-flirt --budget 0.5 --jobs 8 --seed trial-flirt` (images up to 32 voxels per
axis, every stored type, odd geometries; `<workspace>/oracles/difftest/2026-11-01-fsl-flirt`):
3945 cases, 3009 agree, 928 differ, 8 over the 3 GiB limit. Triage in
`validation/difftest/triage.toml`; `validation/investigations/fsl-flirt/recheck_findings.py` re-ran
the 936 findings after the fixes: 577 agree in both their shrunk and drawn forms.

- **Fixed: the blur of `-applyxfm`.** `do_applyxfm` calls `configure_interpolation(testvol)`
  (extraslice with `backgroundval()`, or constpad with `-setbackground`'s value) *before* it blurs
  each volume (`filter_image(tempvol, tempvol, tempvol, min_sampling_ref, false, filter_blur)` on a
  `ShadowVolume` of `testvol[t0]`), and `convolve` reads `source(x, y, z)` out of bounds through the
  volume's extrapolation. The replica blurred with zero outside: wrong wherever the input's edges
  are not zero and the reference is coarser (the blur only runs then). The core's convolution now
  takes the volume's extrapolation (zero for the default zeropad, as before); the main path blurs
  `testvol` before configuring it, so it is unchanged. Parity cases `real/applyisoxfm-4d-blur` and
  `-setbackground` (the raw series' edges are not zero); permanent case `applyxfm-blur-extraslice`.
- **Fixed: BBR's NaN without boundary points.** `bbrval /= bbrwsum` is 0 / 0, SSE's default NaN
  (`-nan` when printed); the compiler had folded the empty loop's `0.0 / 0.0` into a positive NaN.
  The division now follows SSE's NaN rules (`sse::div_f64`). Permanent case
  `bbr-no-boundary-points-nan`.
Part 2 (2026-11-01) settled the 359 open findings (`recheck_findings.py` with each build; logs
`<workspace>/oracles/logs/fsl-flirt-2-recheck{2,3}.log`): every one of the 930 findings that fit
the trial's memory agrees, in its shrunk and drawn forms; the 6 that did not are FSL's own memory
(below).
- **FSL-75: FSL's heap corruption.** Traced with a hardware watchpoint (`gdb`, `watch -l` on the
  corrupted chunk header found by a first run; `<scratch>/py/gdb_watch.py`, now
  `validation/probes/fsl-flirt-bval/`): the write is in `calc_bval` (newimage.cc:1432), reached
  from `subsample_by_2` (`halfvol = refvol.backgroundval()`). `numbins = 2(xb−2ewx)(yb−2ewy)ewz +
  2(xb−2ewx)zb·ewy + 2yb·zb·ewx` with `ew = min(2, n−1)`; for `n = 3`, `n − 2ew = −1`: the
  negative terms shrink the vector while their loops write nothing, and the yz faces' loop writes
  past it. Exactly: `numbins < written` when x = 3 (and y or z ≥ 2) or y = 3 (z ≥ 2, x ∉ {2, 4});
  `numbins = 0` at 1 × 1 × 1 (`hist[0]` on an empty vector: a null read, segfault). FLIRT computes
  it in `subsample_by_2` (the reference pyramid), `configure_interpolation` (without
  `-setbackground`: the final resampling's padding) and `q_sinc_interpolation`'s fallback (kernel
  sum ≈ 0; on the input, never met in the trial). The levels' sizes follow from the images': e.g. a
  1 × 1 × 1 input against an 8 × 1 × 1 reference reaches a 3 × 1 × 4 level. FSL then aborts at a
  later `free` (`malloc(): invalid size (unsorted)`, `double free or corruption (!prev)`, `free():
  invalid next size`, `munmap_chunk(): invalid pointer`, `corrupted size vs. prev_size`: the
  message depends on the heap's layout), segfaults, or finishes and crashes at exit after writing
  its files; the `-bins 16` case is the same. The replica refuses in both modes at the first such
  computation (`Volume::background_undefined`), before writing anything; the suite's comparison
  takes "FSL killed, the replica refusing FSL-75" as agreement (FSL's later messages and files are
  those of a corrupted heap). Suite cases `edge/background-overflow`, `edge/background-empty`.
  mcflirt reaches `calc_bval` through `affine_transform`'s padding and `subsample_by_2` too: its
  replica does not refuse yet (a follow-up; no record case meets it).
- **The 66 matrices: `scalemat`'s single-precision quotient.** `orig2scaled_coord(1,1) =
  1/globaloptions::get().basescale` divides an `int` by a `float`: a single-precision quotient
  (exactly 100 for a base scale of 0.01), stored in the double matrix; the replica divided in
  double. With an identity initial matrix `S·I·S⁻¹` hides it; with `-init` (or `-usesqform`) and a
  base scale other than 1 (voxels below 0.75 mm or above 12 mm: every one of the 66) the initial
  matrix differed in its last digits (−200.000004 against −200) and the search went elsewhere.
  Found by comparing `-verbose 5` traces. Fixed; suite case `real/basescale-small-init` (the old
  build differs from FSL there), permanent case `basescale-init-float`. The extra `Erroneous dof 3`
  line and the stdout differences were the same cause.
- **The fix pass's unnamed changes (8).** FSL-14 was never named by FLIRT's replica: it is now
  named whenever the pair's volumes hold NaN or infinite values (silent where they lie outside the
  cost's points: `_SILENT`); FSL-09 was named only for values that were not integers: also for
  values out of the type's range (a negative `-setbackground` into uint8: 251 against 0).
- **The 6 runs over 3 GiB** (trial-flirt-01374, -01768, -02317, -03183, -03192, -03718: voxels of
  0.01 mm or 25-50 mm make FLIRT's isotropic levels huge): FSL's own peak is 8.2 GiB on one and
  above 25 GiB on others (`memory_findings.py`; the box stopped two FSL runs at 26 and 39 GiB). Not a
  difference: both programs need more than the trial's budget; they belong to the final validation
  phase's heavy cases.
- **The replica's peak memory above FSL's on large references** (found measuring trial-flirt-01374,
  12.1 GiB against FSL's 8.0; then on ordinary calls: `-in epi -ref t1full -omat out.mat` with a
  191 × 288 × 288 float reference, 262 MiB against 192, and with `-inweight`, 503 against 375).
  Located with an RSS timeline and `gdb` catching every allocation of the reference's size
  (`validation/investigations/fsl-flirt/rsstrace.py`, `gdb_allocations.py`): FSL's blur holds the source and two volumes
  (`convolve_separable` assigns each pass over its one copy); the core's kept all three passes'
  results alive (shadowed bindings live to the end of the scope); `isotropic_resample` and
  `raw_affine_transform` copied their input to change its extrapolation (FSL changes it in place),
  and FLIRT's weighted `filter_image` kept its binarised weight and weighted product while it
  filtered the weight. Each copy is now released once used (the copies made only when the
  extrapolation changes): 201 MiB against 192, 322 against 374, every matrix still FSL's.
  `tests/parity/test_memory_peak.py` gains both calls (`omat-only`, `inweight`).
- **Quaternion messages: one missing** (trial-flirt-programs-00219 and the like: 1720 lines of
  `Parameters do not form a valid axis` against FSL's 1721). FLIRT's `optimise` builds the starting
  matrix (`vector2affine(params, no_params, affmattst)`) at every verbosity and prints it only from
  `-verbose 5`; the replica built it only then, so an invalid starting axis printed nothing. Fixed.

## Part 2's other findings (2026-11-01)

- **`-anglerep quaternion`.** `construct_rotmat_quat` computes `w2 = 1.0 - Sqr(p1) - Sqr(p2) -
  Sqr(p3)` in double and stores it in a `float`; `w = sqrt(w2)` (float), `x, y, z` floats, every
  entry a float expression (`2*y*y` is `(2y)·y`); with `n == 3` it does not return before the
  translations (it adds `params(4..6)`); with `n < 3` it prints `Can only do 3 or more, not <n>` and
  goes on; `w2 < 0` prints `Parameters do not form a valid axis - greater than unity` and returns
  the identity (scales and skews still applied by `compose_aff`). FLIRT prints these from inside
  the cost function, so the replica collects them and writes them to standard error before
  anything else written there. `rotmat2quat` fills a `mat44` (float) with the 3 × 3 rotation only
  (the rest uninitialised: it reaches only discarded outputs) and calls the legacy
  `nifti_mat44_to_quatern` (x87 precision in FSL's build; the core's `mat44_to_quatern`), keeping
  `(qb, qc, qd)` (the fourth output, `qfac`, lands in `quaternion(4)`). The search keeps Euler
  angles (`flirt.cc:695`).
- **FSL-67's measurement** (rule D → C): `validation/investigations/fsl-flirt/fsl67_measure.py`
  at commit c9f0aa6 (whose default computed the normals with the segmentation's voxel sizes),
  results in `fsl67-results.json`: fMRIPrep's three BBR calls move by 0.016 mm (ds005040, 0.85 ×
  0.83 × 0.83 mm segmentation), 0 (ds003592, isotropic) and 0.025 mm (ds000228, whose 0.9999998 mm
  voxel differs from 1 only in the float's last bit: BBR's own sensitivity) on average over the
  brain; on ds005040's T1w taken every third slice (0.85 × 0.83 × 2.5 mm) and moved by a known
  rigid motion, three starts converge 0.060 mm (corrected) and 0.063 mm (FLIRT's) from the truth on
  average, the 95th percentiles 0.092 and 0.092 mm: no gain above the noise. Kept in both modes.
- **avscale** (avscale.cc): `catch (Exception)` catches `std::runtime_error` (armawrap's and
  newimage's exceptions: it prints `what()`), `catch (...)` the rest (`Image error`), and both
  `throw;` again: `std::terminate`. `optionarg[0]` of an empty first argument reads the string's
  terminator (not `-`: no option). A missing matrix argument after an option makes
  `std::string(nullptr)` (`std::logic_error`). `affmat.i() * rotmat * skew * scale` is a chain of
  four (each operand evaluated, `((A⁻¹R)K)S`); `(mat*scale.i()*skew.i()*rot.i() + id4)` in
  `sqrtaff` likewise; `.SubMatrix(1,3,1,3).i() * trans` is a solve. 400 random matrices
  (rotations, scales, skews, reflections, any matrix) give FSL's text.
- **The coordinate programs**: `read_volume_hdr_only` neither repairs zero voxel sizes nor builds
  ANALYZE's sform (unlike a full read) and allocates a one-element buffer; `std2imgcoord` prints
  `affmat.i()` as an Armadillo expression (`arma_ostream::print`: its own widths, the stream's
  state restored), while every `Matrix` prints through armawrap's `doPrint` (fixed, 6 decimals,
  for good); `img2imgcoord`'s `verbose` is a zero-initialised global (no constructor sets it);
  `-` is an option for img2imgcoord (`Lacking argument to option -` when last) and the standard
  input for the others; the `-warp` reader's failure prints the image exception and `An error
  occured while reading file: <name>`, exit 1.
- **rmsdiff**: `rms_deviation` catches its inverse's exception (`RMS_DEVIATION ERROR:: Could not
  invert matrix`, `exit(-5)`); the low-determinant warning prints the matrix on `cerr` (which then
  stays fixed); `Max(dist, maxdist)` keeps `maxdist` for a NaN distance; an empty mask divides 0 by
  0 (`-nan`).

## Not replicated yet (part 3)

Each is refused with a message (`larmorx-nc: error: ... not replicated yet`), never computed
differently:
- The schedule command `alignpaxes` (`principleaxes_mat`: Armadillo's `eig_sym`, LAPACK `dsyevd`).
- General parameter-subset masks' pseudo-inverse (`dgesdd`).
- `-debug` with the BBR cost (the `debug` 4D volume of `set_debug_mode`: `refvol` doubled, the
  points' rounded mean of millimetre coordinates used as voxel indices, writes out of bounds going
  through `extrapolate`'s reference).
- The coordinate programs' `-warp` with a readable warp field (FNIRT's `FnirtFileReader`: field or
  coefficient files, the latter needing fsl-basisfield's splines).

Not refused but not yet compared with FSL: `-debug`'s other volumes (the pyramid, the weights; the
1 × 1 × 1 weight volumes FSL filters from empty ones).
- The other programs of fsl-flirt: `applyxfm4D` (templated over the input's type, sinc `b`/7 in
  4D and `r`/9 in 3D), `makerot` (fsl-utils' `Option<vector<float>>`; its axis is divided by the
  square root of its norm, `ax / sqrt(norm2(ax))`: a wrong angle unless the axis is unit length, a
  defect to register), `midtrans` and `pointflirt` (Armadillo's `svd`: LAPACK `dgesdd`, shared with
  the pseudo-inverse above).
