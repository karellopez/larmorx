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
| FSL-75 | newimage's `calc_bval` (the background value, edge width 2) sizes its buffer too small for an image 3 voxels wide along x (with y or z ≥ 2), or along y (z ≥ 2, x not 2 or 4), and empty for 1 × 1 × 1: heap overflow (the percentile of the values that fit), or a read of an empty vector's first element | a | — | rule A: the whole border's percentile by default, FSL's values that fit with `reproduce`, named in both; 1 × 1 × 1 refused in both modes (rule B; part 2) |
| FSL-76 | makerot divides its axis by the square root of its length (`ax / sqrt(norm2(ax))`, `norm2` being the length): for an axis that is not of unit length the rotation is by `θ·√|a|` (30 degrees about 0,0,4 give 60; the help calls the axis "unnormalised") | a | — | rule A: divided by its length by default, FSL's with `reproduce`, named (part 2) |
| FSL-77 | makerot reads `axis.value()[0..2]` and `centre.value()[0..2]` whatever their sizes: fewer than three values read past the vector (its buffer: the default's zeros, or an earlier occurrence's values: `-a 1,2,3 -a 7` is 7,2,3), more are ignored | a | — | rule B: refused by default (a message naming FSL-77); FSL's reading with `reproduce`, named (part 2) |
| FSL-78 | midtrans' `main` catches every `std::exception` of `do_work` (a singular `AAᵀ`, a singular input's solve, a non-square matrix's `det()`), prints `what()` and returns 0 with nothing written | e | B | exit status 1 by default with FSL's message, 0 with `reproduce`, named in both (part 3) |
| FSL-79 | applyxfm4D's spline on an integer image: `Splinterpolator<T>` stores each deconvolution pass as `static_cast<T>(c + 0.5)` (`SplineColumn::Set` for non-float `T`: rounded half up, toward zero below −0.5) and returns `static_cast<T>(value_at(...))` (truncated) before newimage's float | c | A | the float image's coefficients and values by default, FSL's with `reproduce`; named where the output changes (part 3) |
| FSL-80 | applyxfm4D `-3D` sinc: `setinterpolationmethod(sinc)` defines newimage's default kernel (blackman, width 7) first, and the kernel cache (`std::set<kernelstorage*, kernelstorage::comparer>`) orders its kernels by `comparer`, an equality test (true for equal kernels, false otherwise): two different kernels are "not less" both ways, so the set holds them equivalent and `definesincinterpolation("r", 9)` gets the cached blackman 7 back | c | A | the rectangular width-9 kernel by default, blackman 7 with `reproduce`; named where the output changes (part 3) |
| FSL-81 | aff2rigid (FSL 6.0.7.23's script) calls `mat(...)` after `from numpy import *`: NumPy 2 removed `numpy.mat`, so the script stops with `NameError` before writing | b | A | the matrix computed by default (identical to FSL's script with `mat` restored, 22 matrices: `validation/probes/fsl-flirt-programs/aff2rigid_mat.py`), FSL's traceback and exit 1 with `reproduce`, named in both (part 3) |

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
  its files; the `-bins 16` case is the same. The values FSL sorts are defined: the first
  `numbins` written (the later writes land past the buffer), so the background value FSL uses is
  the percentile of those. The short trial of part 2's new paths found an ordinary image where FSL
  survives (trial-flirt-programs-11900: 25 × 30 × 27 at 0.5 × 1.2 × 1 mm, a 3 × 3 × 3 level, FSL
  exit 0 and a matrix): a refusal would fail where FLIRT works. Decided by rule A (the input makes
  sense, the result comes from a part of the border while the heap is corrupted): by default the
  percentile of the whole border; with `reproduce`, the values that fit FSL's buffer (the D13 rule
  for undefined cases: the defined part reproduced, with a notice); FSL-75 named in both modes
  wherever the buffer is too small (`_SILENT` in the suite: the notice is about FSL's heap, whose
  effect the comparison may not see). The 1 × 1 × 1 case (no border; FSL reads an empty vector, a
  null read) stays refused in both modes (rule B). The core's `Volume::background_buffer` tells
  the three cases apart, `background_value_with(whole_border)` computes both values;
  `Volume::backgroundval` is FSL's (mcflirt and BET reach it too: they now get FSL's values that
  fit, without a notice yet: a follow-up). Suite cases `edge/background-overflow` (FSL killed, the
  replica finishes in both modes naming FSL-75), `edge/background-empty` (refused); permanent case
  `background-level-overflow-survives` (FSL finishes; the replica's matrix is FSL's).
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
  0.01 mm or 25-50 mm make FLIRT's isotropic levels huge; `memory_findings.py`, one thread, each
  program alone): -01374 FSL 8.0 GiB (the replica 12.1 GiB before part 2's memory fixes, below);
  -03718 FSL 6.3 GiB, the replica 5.9 (identical matrices); -03183 both run over 30 minutes;
  -01768 and -03192 both stopped by a 64 GiB box at 24 to 43 GiB; -02317 FSL segfaults at 8.9 GiB
  (a 3-voxel level: FSL-75). The second trial's quaternion runs over 3 GiB need more than 20 GiB in
  FSL (each stopped by a 20 GiB box), and in the replica on two of them (-2-01831, -2-02240); on
  -2-01944 the replica finishes at 19.0 GiB. Not a difference:
  both programs need more than the trial's budget; they belong to the final validation phase's
  heavy cases.
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
- **FSL-22 in FLIRT** (trial-flirt-programs-00931: a 3 × 1 × 1 input holding 5000, 100 and NaN
  against a 1 × 1 × 1 reference; the matrix is NaN, so every output voxel is the padding, the
  input's background value; -00902 likewise with a 2 × 2 × 4 input holding NaN voxels). `calc_bval` sorts the border with `std::sort` and `operator<`:
  libstdc++'s order put the NaN first (FLIRT's output NaN); the replica's volumes sorted NaN last
  in both modes (100): mcflirt's replica had FSL-22's switch, FLIRT's did not use it. FLIRT's
  background values now take the NaN order by mode (last by default, libstdc++'s with
  `reproduce`) and name FSL-22 wherever a border holds NaN (`_SILENT`: the padding may not show).
  Permanent case `background-nan-order`.
- **makerot** (part 2): FSL-76 and FSL-77 above; every other path of the 50 probe calls (the
  usage, `-h`, the compulsory `-t`, parse errors, long options after `=`, `:` as separator, `atof`'s
  pieces, `--cov`, a zero axis, `-o` and an unwritable file, an image that cannot be read: signal 6)
  gives FSL's text and files. Left open: three calls with a NaN or infinite angle or axis, where
  FSL prints `nan` and the replica `-nan` in some rows (the NaN's sign through `make_rot`'s
  products; part 3).
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

## Part 3's findings (2026-11-01)

**FSL's LAPACK and BLAS** (*verified*, `validation/investigations/fsl-flirt/lapack_eig_svd_probe.py`:
ctypes on `~/fsl-6.0.7.23/lib/liblapack.so.3`, every word identical over 400 records: the BLAS
kernels, `dsyevd` 3 × 3 of every family, `dgesdd` 3 × 3, 12 × 12 and 12 × n). FSL's
`liblapack.so.3` is OpenBLAS 0.3.33 with its bundled reference LAPACK (towards 3.13: `dlarf1f` and
`dlarf1l`, the Fortran 90 `dlartg`), dispatched to the Haswell kernels (`OPENBLAS_CORETYPE=Haswell`):
- `ddot` (n < 16 sequential; strided with two accumulators), `daxpy`, `dscal` (the interface
  returns for alpha = 1; `x·0` kept for 0), `drot` (FMA: `x' = fma(c, x, s·y)`, `y' = fma(c, y,
  −(s·x))`, unit and strided alike), `dnrm2` is x87 `nrm2.S` (squares and sums in the 64-bit
  significand, four accumulators added `((a2 + a0) + a1) + a3`, `fsqrt`, rounded to double: the
  rounding of the square root needs `v − r·r > r` checked, not a plain `sqrt`).
- `dgemv_n` (`dgemv_n_4.c`): the 4 × 4 and 4 × 2 microkernels use FMA, the tails do not; `lda == m3`
  takes the pair branch. `dgemv_t` (`dgemv_t_4.c`): **the C tails are FMA-contracted by the
  compiler** (`m3 = 1`: `fma(a0, xt0, y)`; `m3 = 2`: `y + fma(a0, xt0, a1·xt1)`; `m3 = 3`:
  `y + fma(a2, xt2, fma(a0, xt0, a1·xt1))`; `*y += yb·alpha` an FMA).
- `dger` through `axpy`, `dsymv_U`'s small path, `dsyr2` through `axpy`; `dgemm` has no small-matrix
  path on Haswell: each element `vmulpd` then a chain of `vfmadd231pd` over k ascending, in blocks of
  `GEMM_Q = 256`, then `C + acc·alpha` (beta 0: zeros first).
- LAPACK, unblocked for n ≤ 12: `dsyevd('V', 'U')` = `dlansy`, scaling, `dsytd2`, `dstedc` →
  `dsteqr('I')` (N ≤ 25), `dorm2l` through `dormtr`/`dormql`; `dgesdd('S')` with m ≥ n: MNTHR =
  `int(n·11/6)`; path 3 (m ≥ MNTHR: `dgeqr2`, `dorg2r`, `dgebd2` on R, `dbdsdc`, `dorm2r`/`dorml2`,
  `dgemm`) and path 5 (`dgebd2`, `dbdsdc` → `dlasdq` → `dbdsqr` with its sorts, `dormbr`);
  `dbdsqr`'s TOLMUL uses glibc's `pow(eps, −0.125)`.

**Armadillo 14.2.1's dispatch** (*read*, fsl-armawrap 0.7.0): `eig_sym` (a rudimentary symmetry check
of the corners, `\nwarning: eig_sym(): given matrix is not symmetric`; non-finite → failure, outputs
emptied; the `dsyev` fallback), `svd_econ` (`dgesdd`, `V = VTᵀ`), `pinv` (empty; the diagonal path
with `tol = max(m, n)·max|a_ii|·ε`; the symmetric path above 40 rows; the general path with `tol =
max(m, n)·s₀·ε`, `tmp = V·diag(1/s)` element by element, then `tmp·Uᵀ`), `glue_times` (row and
column vectors through `gemv`; a product of an object with its own transpose through `syrk`:
`syrk_vec` for vectors, `syrk_emul` up to 48 elements with Armadillo's own dot, BLAS `dsyrk`
beyond; `tinysq` up to 4 × 4; three and four factors ordered by the storage cost; `+=` evaluates
the left `Glue` first, `apply_inplace_plus`; negation as alpha = −1; an `inv()` operand evaluated
first), `AWDiagonalMatrix` and `AWSymmetricMatrix` are full matrices (`symmatl` on assignment).

**alignpaxes** (*validated*, 5 cases): `principleaxes_mat` (`calc_principleaxes`) in double with
`calc_cog`'s flushing; `eig_sym` as above; FSL's `etemp` is a `float`, so each swap stores the moved
eigenvalue in single precision, which the second comparison reads. A non-finite image: `eig_sym`
fails, the empty `paxes` is read (`Mat::operator(): index out of bounds`, caught: exit 0).

**applyxfm4D**: `volume<T>::interpolate` returns `float` for every `T`, so a double image's
interpolated values pass through single precision; but `affine_transform_mask<T>` writes the
`T` padding value, so the masked voxels of a double output hold the double background value
(*verified*: 80 % of an output's voxels exact doubles, the rest single-precision values; finding
probe-part3b-00089). `kernelinterpolation` adds `operator()(...)·kerfac` as `double·float` into a
float sum for `T = double` (`convsum = (float)((double)convsum + v·kerfac)`), `float` arithmetic for
the integer types. `Splinterpolator<double>` keeps double coefficients. The background value is
`T`'s (`calc_bval<T>`, the double vector sorted with libstdc++'s introsort: `stdsort::sort_checked`
generic). FSL-80's kernel cache: the comparison holds kernels of different widths equal (*verified*:
`-3D` sinc output identical to blackman 7, not rectangular 9). The replica holds the series in its
type and converts each volume on its own (as FSL's peak).

**midtrans and pointflirt** (*validated*, 23 cases): `avg_affine_with_procrustes` follows
Armadillo's expression rules (above): `uxP_av += u_i*x_i.t()*P_i.t()` is `(u xᵀ) Pᵀ`; `Pᵀ P`-like
products through `syrk`; `-ba*inv(AAt)` the inverse first, then the product with alpha −1; the
four-factor `vv*dd*uu.t()*q` is `vv·(dd·(uuᵀ·q))`. pointflirt's 3-point layout (rule C): a 3 × 3
file is not transposed (`if (x.Nrows() > x.Ncols())`), so its rows are read as coordinates.
pointflirt's shape errors: the submatrix reads of `x·yᵀ` are unchecked (past the elements with
fewer than three rows), then `q.SubMatrix(10,12,1,1) = ybar` throws outside `main`'s try.

**-debug** (*validated*, 3 cases): the BBR cost's `debugvol` (`refvol` with a second volume, zeroed)
gets, at the rounded mean of each pair's two reference-voxel coordinates, `w12·update + (1 − w12)`
and `w12`; out-of-grid writes go through the reference's `extrapolate`-reference (nowhere for
zero/constant padding; the edge for extraslice one voxel out; mirrored/periodic). Without weights
FLIRT filters its empty `global_refweight` (the resampling of a volume of no voxels has one voxel:
`max(1, 0/step)`), so `global_refweight1` is 1 × 1 × 1, 0, with the new voxel size.
`Pointer to impair is 0x...` prints an address (masked as `(set)` by the suite).

**makerot's NaN signs** (*verified*, `validation/probes/fsl-flirt-programs/makerot_nan.py`, 10 of 10):
GCC compiled `make_rot`'s inlined `gemv_emul_tinysq` for row 0 with the vector's element as the
first factor and the new product as the first addend; rows 1 and 2 matrix-first, sum-first.

**The scripts** (*verified*): `extracttxt` under dash with GNU dc 1.4.1 and sed 4.8: dc's `-` and
`+` with fewer than two numbers print `dc: stack empty` and do nothing (a printed negative number
read back is `-` then its digits); a range beginning with `-` is parsed by sed's getopt (`invalid
option -- '<digit>'`, the usage, exit 1) (`validation/probes/fsl-flirt-programs/extracttxt_ranges.py`,
390 of 390); `exit -1` is dash's `Illegal number`, exit 2. `aff2rigid`: FSL-81 above; its numbers
are NumPy's on OpenBLAS (`gesv` for `linalg.inv`, `ddot` norms, `dgemv_t` for C-order
matrix-vector products, `dgemm`), `savetxt('%14.10f')`. `pairreg`: three flirt runs and
`convert_xfm -fixscaleskew`, in-process; temporary names `<out>.k.<pid>`.

**mcflirt's FSL-75** (*validated*, `edge/fsl75-narrow`): mcflirt's `affine_transform` pads with
`calc_bval` too; on a series 3 voxels wide FSL aborts (double free) or finishes; the replica names
FSL-75 in both modes and uses the whole border by default.

**Part 3's random testing** (*validated*): a pre-check (2,805 cases, seed `probe-part3b`) and one
half-hour trial of the new paths (`difftest --tools fsl-flirt-part3 --budget 0.5 --jobs 20 --seed
part3-trial`, `<workspace>/oracles/difftest/2026-11-01-fsl-flirt-part3`: 209,657 cases, 209,455
agree, 136 notices, 19 differ, 47 over the memory limit). What they found, each fixed with a suite
case or a permanent case (`validation/difftest/cases/fsl-flirt/`), re-checked boxed with the final
build (`recheck_findings.py`):
- applyxfm4D's double template (≈ 100 pre-check cases): `fmrib_main<double>` was refused; its
  arithmetic is above (interpolate's float return, the mask's double padding value, the sinc sum's
  `double·float`, double spline coefficients).
- FSL-79 exactly: `SplineColumn::Set` adds 0.5 before the integer cast, and `Splinterpolator<T>`
  returns `static_cast<T>(value_at)`.
- FSL-22 in applyxfm4D (`calc_bval<T>` sorts the border with libstdc++'s introsort: NaN voxels in
  the border move the 10th percentile; trial 39242, 152078).
- FLIRT's exceptions inside computations whose Rust signature returns a value: the cost
  function's `aff.i()` of a singular matrix (`inv(): matrix is singular`; trial 57755: a parameter
  subset reaching scale 0 on one-voxel images; the replica had used a NaN matrix there) and
  newimage's `Out of memory` (#99) for an isotropic reference of 417 or 585 GB (voxels of 0.01 mm;
  trial 70320, 165530; the replica's allocator aborted). `larmorx-nc-core::exception` throws them as
  an unwinding payload (`resume_unwind`, nothing printed) that FLIRT's `main` catches (FSL-68) and
  the program entry point otherwise reports as `std::terminate`; the volume is allocated with
  `try_reserve` (`exception::try_volume`).
- extracttxt's dc and sed behaviour for ranges before the first line (dash, dc 1.4.1, sed 4.8:
  `validation/probes/fsl-flirt-programs/extracttxt_ranges.py`, 390 of 390).
- The notices: FSL-79 and FSL-80 are named only where the written output changes (both
  conversions compared, the masked voxels left out), FSL-09 only for voxels the mask does not
  overwrite; 131 of the trial's 136 notice cases agree since; in 5 (FSL-79 or FSL-80 with FSL-09 in
  one voxel) the two corrections cancel (the float spline's 2.6 rounded to 3, FSL's integer 3 truncated to 3: the combined
  output equals FSL's): left as notices.
- The 47 memory cases are flirt schedules on extreme voxel sizes: FSL needs as much (finding
  probe-part3b-01970: FSL 8,966 MiB, the replica 8,342, the matrices identical; `memory_findings.py`):
  heavy cases of the final validation phase.

**Open** (left by decision of part 3; `specs/flirt.md` §14.2): the sign of NaN results from
non-finite inputs under FSL-14's `reproduce`: an all-NaN `-omat` matrix when a schedule optimises
relative to a row (pre-check 496, trial 65393, 83738, 97568; also without `paramsubset`, so part
2's path: the starting matrix from `testCog` already differs in sign; FSL `-nan` against the
replica's `nan` in some, the reverse in others), and the NaN voxels of applyxfm4D's spline of an
image with NaN and infinite voxels (trial 102752, 122789, 191754: `deconv`'s or `value_at`'s
operand order in FSL's build for two NaN operands). Not traced: which of `calc_cog`'s sums, the tiny
`sampling_mat()·v` product, `compose_aff` or the spline's sweeps passes the other operand's NaN.

## The spec answers of 2026-11-02 (the costs and BBR, from the source)

What FSL 6.0.7.23's source says behind `specs/flirt.md`'s answers to the clean-room questions 1
to 5 (§6.2, §6.5, §8.1, §12, §14); the models that reproduce FSL's numbers from the spec's text are
in `validation/investigations/flirt-spec-answers/`.

- **The correlation ratio** (`p_corr_ratio`, `p_corr_ratio_smoothed`, `p_corr_ratio_fully_weighted`
  in `costfns.cc`): per-bin `float` arrays `numy`, `sumy`, `sumy2` filled in the sampling loop
  (`sumy2[b] += weight*val*val`, left to right), the extra bin folded into `no_bins − 1`, then the
  loop over bins with `numy[b] > 2` accumulating `numtoty`, `totsumy`, `totsumy2` and `corr_ratio`
  inside the same test: the sparse bins never reach the totals. The function returns
  `(1.0 − corr_ratio)` (a double expression returned as `float`) and `Costfn::cost` takes `1.0 −`
  it into the `float` `retval`: the two roundings of §6.2. With `numtoty <= 1 || var <= 0` it returns
  0.0, so the cost is 1.
- **Woods** (`p_woods_fn`, the only form `Costfn::woods_fn` calls; `p_woods_fn_smoothed` is never
  used): `woods += Sqr((float) num[b])*stdev/sum[b]` when `sum[b] > 0`, else
  `Sqr((float) num[b])*stdev`; the loop runs over `b = 0 … no_bins`; `1e+10` when `numtot` is 0.
- **BBR's points** (`Costfn::set_bbr_seg`): `wmsum = convolve_separable(wmseg, box3, box3, box3)`,
  `wmsmooth = smooth(wmseg, 2.0)` and `gradient(wmsmooth, wmgrad)` (`newimagefns.h`):
  `smooth()` passes `2·(int)(σ − 0.001) + 3` to `gaussian_kernel1D(σ, radius)` as the **radius**
  (`exp(−(j*j)/(2.0*sigma*sigma))` into a `float`, summed in `float`, `kern *= 1.0/sum`);
  `make_grad_masks()` builds `(x − 1)·pow(3, 1 − |y − 1| − |z − 1|)` and its permutations as
  `float` volumes; `gradient()` sums `source(...)*mask(...)` (`float` × `float`) over `mz`, `my`,
  `mx`; the normal `−wmgrad/refvol.?dim()` and `std::sqrt(norm2sq(...))` (`float`). The volumes
  extrapolate with zeros (the default `zeropad`). `bbr_slope` defaults to −0.5, `bbr_dist` to 2.0;
  `local_abs` is `Min(bbrupdate, 2.0 − bbrupdate)` inside the sum (`Costfn::bbr`).
- **The centres of mass** (`calc_cog` in `newimage.cc`): `val = (double)(vol(x,y,z) − vmin)` (the
  subtraction in the volume's type, `float`), double sums flushed every `nlim + 1` voxels with
  `nlim = max(1000, (int64_t) sqrt(nvoxels))`.
- **The rotations' sines and cosines** (`make_rot` in `miscmaths.cc`): `float theta = norm2(angl)`
  and `cos(theta)`, `sin(theta)` on the `float` (`std::` overloads: single precision); the model
  of the search's first sweep reproduces FSL's trace with either precision, so §3 was left as it is.
- FSL's x86-64 build has no fused multiply-adds in this code (the replica, bit-identical, uses
  none there).

## The spec answers of 2026-11-02 (the search, the outputs and their precision, from the source)

What FSL 6.0.7.23's source says behind `specs/flirt.md`'s answers to the clean-room questions 6
to 20 (§3, §4.1, §4.2, §7.2, §7.4, §8.1, §8.2, §8.5, §8.7, §9.2, §9.3, §12, §14); the models are
`validation/investigations/flirt-spec-answers/matrices_model.py` and `resample_model.py`.

- **The search** (`search_cost`, `optimise_strategy3`, `usrsearch` in `flirt.cc`): `search_cost`
  sets `dof = searchdof` (the `search` command's argument, default 12) for the subset mask and
  the `dof <= 6` scale rule, and restores it at the end; `optimise_strategy3` then uses `Min(dof,
  7)` of `-dof`, `vector2affine(params_8, dof, matresult)` (so the composition of the first
  `min(dof, 7)` parameters, with the user's `anglerep`: the `Euler` hack covers `search_cost`
  only) and `optimise_strategy1`, whose `affmat2vector` decomposes the matrix. `matrow(1,18) =
  costfn(premat)` (the matrix form of `costfn` prints nothing). `param_tol` is computed at the
  start of `search_cost` from the `refparams` in force (`params12toN` of `refparams` and of `tol +
  refparams`). The coarse points: `refparams` = the sampled parameters, `params12toN` gives 0;
  `tx`, `ty`, `tz`, `scale` are `volume<float>`. The threshold step sets `refparams = params_8`.
  `find_cost_minima`'s loops are z, y, x. `percentile` (`percentile_vec`, `newimage.cc`) takes
  `hist[(unsigned int)(((float) numbins) * p)]`. `add2list` scans the list in order, breaks at the
  first close entry (`delete_row` when better, else `return -1`), `sorted_posn` inserts before the
  first strictly greater cost.
- **The optimiser** (`MISCMATHS::optimise`, `optimise1d`, `optimise.cc`): `inv_tol /= (float)
  tol.Nrows()` and `float avtol = SP(initpt - pt, inv_tol).SumAbsoluteValue()`; `optimise1d`'s
  `unitdir = dir/std::sqrt(dir.SumSquare())`, `float dir_tol += fabs(unitdir(n)/tol(n))`, `float
  unittol = fabs(1/dir_tol)`, trial points `x*unitdir + pt`.
- **The composition** (`make_rot`, `construct_rotmat_euler`, `compose_aff` in `miscmaths.cc`):
  `float theta = norm2(angl)`, `axis = angl/theta`, the basis `x2`, `x3 = cross(x1, x2)` normalised
  by `norm2` (double), `rotcore` with `cos(theta)`, `sin(theta)` of the float (`sincosf`),
  `basischange * rotcore * basischange.t()`; `aff = aff * newaff` per axis; `aff * skew * scale`.
  **The decomposition** (`decompose_aff`, `rotmat2euler`): `float sx = norm2(x)`, `MISCMATHS::dot`
  returns `float`, `aff3 * scales.i() * (skew.SubMatrix(1,3,1,3)).i()` (Armadillo: `aff3 *
  solve(scales, inv(skew))`, the skew's inverse by `dtrtri` as a triangular matrix, the solve by
  `dgesv`'s packed reciprocals).
- **The left-right swap** (`basic_swapdimensions`, `swapmat`, `setrow` in `newimage.cc`):
  `swapmat(-1, 2, 3)`'s translation `(xsize - 1)·xdim` is computed in `float` by `setrow`;
  `newS = sform_mat() * sampling_mat().i() * swapmat.i() * swappedSamplingMatrix` (and `newQ`),
  both whatever the codes; `readGeneralVolume` swaps when `RadiologicalFile` is false and
  `makeradiological` finds `FSL_NEUROLOGICAL`; the writer (`save_basic_volume`) swaps back. The
  forms are `mat44` (float) from `getSForm`/`getQForm` (`nifti_quatern_to_mat44`, x87 long
  double constants). `set_fsl_hdr` writes `setQForm(newmat2mat44(qform_mat()))`
  (`nifti_mat44_to_quatern` of the float matrix). `qsform_init_mat` uses `newimagevox2mm_mat()`
  (the swapped sform).
- **`-applyisoxfm`** (`isotropic_resample`, `newimagefns.cc`): `iso.copyproperties(aniso)` keeps an
  unset form unchanged; a set one becomes `form * iso2aniso` with the `float` steps.
- **The inverse** (`arma::inv` → `dgetrf` (OpenBLAS `getrf_single` → `getf2_k`) and `dgetri`
  (reference LAPACK, `dtrtri` → OpenBLAS `trti2_U`)): every `dgemv_n` call there has `α = −1`; the
  Haswell microkernel's `fma(alpha, s, y)` with `α = −1` equals `y − s`; `dscal` with `−1/u_jj`
  turns a zero into `−0`. The replica's `larmorx-nc-core::lapack` is the reference.
- **The output order** (`do_applyxfm`, `main` in `flirt.cc`): `do_applyxfm` saves the volume
  before `save_matrix_data`; after a registration `main` calls `save_matrix_data` first.
- **The paddings** (`affine_transform`, `affine_transform_mask`, `raw_affine_transform` in
  `newimagefns.*`): the mask's `o1 = x*a11 + z*a13 + a14; o1 += a12` in `float`, the same
  accumulation as the interpolation; the mask omits `raw_affine_transform`'s neurological
  `swapmat` products, which never apply in FLIRT (every volume is radiological once read).
  `Costfn::bbr_resamp` (the field-map output) has no mask.
- **`alignpaxes`** (`flirt.cc`): `principleaxes_mat()` of the reference, then of the input, then
  the `SubMatrix` reads; Armadillo's `eig_sym_helper` prints `arma_warn(1, "eig_sym():", ": given
  matrix is not symmetric")` when `rudimentary_sym_check` fails (`"\nwarning: "` first).
- **The sinc** (`kernelinterpolation` in `newimage.cc`, `sinckernel1D`, `sincfn`, `hanning`,
  `blackman`, `kernelval` in `kernel.cc`): the store loops `storez[d+wz] = kernelval((z-iz0+d), wz,
  kernelz)`, `zj = iz0 - z1 + wz`, `float kerfac = storex[xj] * storey[yj] * storez[zj]`, `float
  convsum`, `kersum`; `kernelval`'s `float dn = x/w*halfnk + halfnk + 1.0`.
- **The spline with `-setbackground`** (`configure_interpolation` before `filter_image` in
  `do_applyxfm`, `convolve` reading `source(...)` through the extrapolation;
  `translate_extrapolation_type(constpad) = Zeros`; `Splinterpolator::coef` returns 0 for Zeros
  outside; `init_fwd_sweep`/`init_bwd_sweep` treat Zeros as Mirror; `SplineColumn::Set` stores
  `static_cast<T>`; `value_at`'s loops and `get_wgt`). After a registration `main` blurs first and
  configures afterwards (zeros outside).
- **The first pair, the base scale** (`main`, `set_basescale`): `new Costfn(refvol_8, testvol)`
  with `no_bins/8`; `requestedscale = 1.0` by default; `set_basescale` reads the headers only
  (`read_volume_hdr_only`) before `get_refvol`/`get_testvol` (whose `double_end_slices` triples).

## The refusals and leads of 2026-11-02 (FSL-90 to FSL-93, from the source)

What FSL 6.0.7.23's source says behind `specs/flirt.md` §4.1, §4.2, §6.4, §8.1 step 9 and §11's
FSL-90 to FSL-93, decided by rules A-D; FSL's runs are `validation/investigations/flirt-spec-answers/
fsl_runs.py`'s `default12`, `quat`, `zero-*`, `onevox-*` and `zero-voxsize` (through the oracle
cache), the replica's in both modes `replica_runs.py`.

- **FSL-90, `-anglerep quaternion`'s search** (`optimise_strategy3`, `flirt.cc`): `search_cost`
  sets `anglerep = Euler` for its own compositions and decompositions and restores the user's
  before returning `paramlist` (Euler angles); `optimise_strategy3` then calls `vector2affine(
  params_8, dof, matresult)` with the user's `anglerep`, so `construct_rotmat_quat` reads the three
  angles as `(x, y, z)` (`w² = 1 − x² − y² − z²` negative: its message and the identity). Rule A
  (a documented option, the search's minima misread); the replica's `search_start` composes them
  with Euler angles by default, FSL's call with `reproduce`. Measured on `real/anglerep-quaternion`:
  FSL's matrix 0.3685 mm (`rmsdiff`'s 80 mm form) from FSL's `-anglerep euler` run (`default12`), the
  corrected one 0.2333 mm, 0.192 mm from FSL's; four of the five starting points lose their rotation
  in FSL (its four messages).
- **FSL-91, an empty weighting mask** (`filter_image`, `filter_weight`, `flirt.cc`; `binarise`,
  `newimagefns.h`; the `*_fully_weighted` costs, `costfns.cc`; `calc_cog`, `newimage.cc`):
  `binarise(weight, 0.01)` is `binarise(vol, 0.01, vol.max(), inclusive)`, empty when the maximum
  is below 0.01; `filter_image` gives `divide(f(I·B), f(B), f(B))` = 0 (the safe division) and
  `filter_weight` `f(W)·[f(B) ≥ 0.9]` = 0, at every level and at every `setscale` (the input from
  `global_init_testweight`). Each weighted cost has a guard for no weight: `corr_ratio_fully_
  weighted` returns 0 when `numtoty <= 1` (cost 1), `normcorr_fully_weighted` `corr = 0` unless
  `num > 2` (cost 1), `leastsquares_fully_weighted` and `labeldiff_fully_weighted` the squared range
  of the two (zero) images, `calc_fully_weighted_entropy` the maximum entropies when `noverlap` is 0
  (`mutualinfo` 0, `normmi` −1); `woods` ignores the weights (its warning) on zero images; BBR's
  `bbrval /= bbrwsum` with `bbrwsum = 0` (NaN, its `(2)` warning at every evaluation). `calc_cog`
  prints `WARNING::in calculating COG, total = 0.0` when `|total| < 1e-5` and divides by 1. Every
  value is defined, so FSL is deterministic (the replica's `reproduce` identical in the four runs).
  Rule B; the replica's `check_registrable` refuses after `get_testvol` (only `main`'s registration
  path: `-applyxfm` returns before).
- **FSL-92, an image of one voxel**: no defect in the arithmetic (a constant 1 × 1 × 3 image after
  `double_end_slices`); FSL's three runs give rotations of the search's grid (95, 131 and 143
  degrees). Rule B, the same check (recorded before `double_end_slices`).
- **FSL-93, a base scale of 0** (`set_basescale`, `flirt.cc`; `read_volume_hdr_only` against
  `NiftiIO`'s reading in `generalio.cc`): the full reading sets `pixdim[i] = pixdim[i] == 0 ? 1 :
  fabs(pixdim[i])` for `i = 1 .. dim[0]`; `read_volume_hdr_only` keeps the stored magnitudes, so a
  zero makes `Min(mindimA, mindimB) = 0 < 0.75` and `basescale = 0`. `FLIRT_read_volume`'s
  `setxdim(xdim() / basescale)` gives infinite sizes, `set_initmat`'s `orig2scaled_coord(1,1) =
  1/basescale` a NaN initial matrix, `resample_refvol`'s grid `(int)` of an infinite size
  −2147483648 (x86-64's `cvttss2si`, undefined in C++), and the costs' loops over `unsigned int
  zb1 = vref.zsize() − 1` (2147483647) never end in practice (the replica traced under a debugger:
  inside the search's first optimisations, `p_corr_ratio_weighted`). FSL ran for 60 minutes at one
  core with 19 MB resident before the cache's timeout. Rule A for the header (FLIRT's own reading
  counts the size as 1; the replica counts it so in the base scale by default, refuses with
  `reproduce`); `-basescale 0` (`atof`, `force_basescale`) is refused in both modes; `-basescale
  -1` finishes (the replica: the default registration's matrix with its translation negated).
- **A zero BBR normal** (relay 1's lead): unchanged (spec §6.5 states FSL's NaN), not registered:
  unrealistic on a segmentation.

## Not replicated (by decision)

- The coordinate programs' `-warp` (FNIRT's warp fields): refused until FSL's warp-field libraries
  are ported (`docs/next-steps.md`).
- `standard_space_roi`, `flirt_average`, `epi_reg` (scripts of FSL's other programs).

## The spec answers of 2026-11-03 (part 2's questions 1 to 20, from the source)

Source facts behind `specs/flirt.md` §14's table of 2026-11-03 (FSL 6.0.7.23; the models in
`validation/investigations/spec-answers-3/flirt_models.py`):

- **makerot** calls miscmaths' `make_rot(ax*theta*M_PI/180, ctr, mat)` with the whole scaled axis:
  `float theta = norm2(angl)`, `axis = angl/theta` (not renormalised), `x2 = (-axis(2), axis(1), 0)`
  or `(1,0,0)` when `norm2(x2) <= 0`, `x3 = cross(x1,x2)/norm2`, `basischange * rotcore *
  basischange.t()` with `cos(theta)`/`sin(theta)` of the float. The options are `Option<float>` and
  `Option<std::vector<float>>`, so the angle, axis and centre are floats; FSL-76 is `ax /
  sqrt(norm2(ax))`. FLIRT's own compositions use the same `make_rot` per axis (§3).
- **avscale**: `decompose_aff(params, affmat, cor, rotmat2euler)`, `construct_rotmat_euler(params, 6,
  rotmat)` (no centre: about the origin), `ans = affmat.i() * rotmat * skew * scale` (Armadillo's
  four-factor chain evaluates `((A*B)*C)*D` for equal sizes), `newxfm = rotmat * skew * scale` with
  `scale(2,2) = 1.0/params(8)`.
- **The matrix reader** (miscmaths `read_ascii_matrix`): `skip_alpha` finds the first line whose
  first token passes `isNumber` (`strtod` to the token's end); the columns are the token count of
  that line plus a blank, minus 1; the rows the count of such lines from there; each row read by
  `istringstream >> double` with a throw only on `eof()` (a failed stream leaves `newnum` as it is,
  so the last value repeats; libstdc++'s `__convert_to_v` gives 0 and failbit for a partial parse,
  `numeric_limits::max()` and failbit for `±HUGE_VAL`).
- **Coordinate programs**: `parse_command_line` treats any `-` word that is not a no-argument
  option as needing one (`if (n+1>=argc) { "Lacking argument"; break; }`); the interactive refusal's
  text hard-codes the program name; std2imgcoord has no `isatty` test and its `oldstd == stdcoord`
  (initially 0 with a fourth component 0 against 1) only on standard input. `-warp` goes through
  `FnirtFileReader::Read` (`common_read`: `read_volume4D`, `tsize() != 3` throws), caught by
  `catch(...)`.
- **midtrans**: `if (fabs(trans.Determinant()) < 1e-6)` (false for NaN and inf);
  `ColumnVector temp_orig(4)` (Armadillo zero-initialises) printed whole without a template,
  replaced by `cog()`'s 3-vector with one; `Pxx_av` printed in `avg_affine_with_procrustes`' stage
  1.
- **pointflirt**: `if (x.Nrows() > x.Ncols()) { x = x.t(); y = y.t(); }`; with two 0 × 0 matrices
  `n = 0` passes the count test and the later submatrix copies of the empty products crash
  (FSL-204).
- **applyxfm4D**: `call_fmrib_main(dtype(iname), ...)` reads the input's header first;
  `affine_transform(invol[m], dummy, affmat, 0, true)` directly (no `filter_image` blur).
- **flirt**: `search_cost` lowers `verbose` by 2 and restores it; `optimise_strategy3` lowers it by 2
  again from the restored value for the free optimisations (so the same lowered level);
  `safe_save_volume` saves only when `!nosave` (`-debugsave`), `costs` at the lowered `verbose>=4`
  after the fine grid and again at `>=3` after the threshold step; `Previous scale used` inside
  `usrsetscale`'s new-pair branch; `set_param_tols` (all 12 `tolerance(i)`) is called by
  `set_perturbations` (measurecost, optimise), `search_cost` and both optimise strategies;
  `measure_cost` calls `costfn(Matrix)`, which prints nothing. In `optimise_strategy3`, `premat`
  is the matrix after `vector2affine(params_8, dof, matresult)`, before `optimise_strategy1`
  decomposes it (`affmat2vector`) and `optimise()` composes it again for `Starting with :` and the
  evaluations. `construct_rotmat_quat` returns the identity before adding any translation when `w2
  < 0`; `compose_aff` then applies the scale (n > 6).
- **-debug**: `setcostfntype`'s five lines; `set_bbr_seg` returns 1 without boundary points
  (`no_coords = 0`, so `is_bbr_set()` is false and the post line prints 0); the final call runs
  `measurecost 12 U:1 ... rel 8`.
- **-2D**: no `mode2D` condition on the sform messages; `double_end_slices` (one-slice images)
  builds a new volume without the forms.
- **Questions 21 to 37 (part, relay 3b's first half)**: the weight product in `costfns.cc`
  (`weight=wval*refweight(x,y,z); weight*=o1/smoothx; ...; if (weight<0.0) weight=0.0;`);
  `calc_cog` (newimage) is the one centre of mass for FLIRT and mcflirt (`Costfn`'s `testCog`,
  mcflirt's `refvol.cog`), blocks of `nlim = max(1000, (int64)sqrt(nvoxels))` flushed when `n >
  nlim`; `raw_affine_transform` completes `vout`'s forms (sform from qform, qform from sform,
  both from `vin.sform_mat()` (else qform) `* iaffbig`); `-applyxfm` resamples into a
  `ShadowVolume` of the 4D output, whose header the completion does not reach; `call_fmrib_main`
  maps uint8 to `fmrib_main<char>`; `measure_cost` and `optimise_strategy1` print `Erroneous dof`,
  `optimise_strategy0` does not; nearest neighbour is `MISCMATHS::round` then `operator()(ix,iy,iz,
  ep)` with `extraslice`'s `-1`/`size()` clamp; Armadillo 14.2.1's `inv()` (`op_inv_gen_full::
  apply_direct`) fails only on `getrf`/`trtri` `info != 0` or a zero diagonal (no rcond test in the
  default path; the symmetric path needs 100 rows); `inv(A)*v` is Armadillo's `solve` (`dgesv`).
  BBR: `costfn()` calls `setcostfntype(currentcostfn)` before every evaluation, which calls
  `set_bbr_seg` again while `is_bbr_set()` (`no_coords > 0`) is false.

## The spec answers of 2026-11-03 (part 2's questions 22 to 25 and 38, from the source)

Relay 3b of the spec answers (`validation/investigations/spec-answers-3/flirt_models_3b.py`,
README "FLIRT, questions 22 to 25 and 38"); FSL 6.0.7.23's source:

- **Q22, the sinc's sine.** `kernel.cc`'s `sincfn(float x)` computes `sin(y)/y` with `float y =
  M_PI*x`: C++'s `sin(float)` is `sinf`, glibc 2.35's (`__sinf_fma` on this CPU), the code the nc
  core already carries (`glibc_float::sinf`, Arm v19.11). The FLIRT replica is FSL's (it calls it);
  the clean-room original calls a correctly rounded `sinf`, which accounts exactly for its 32,052
  voxels on `real/interp-sinc` and for its applyxfm4D sinc differences on float images.
- **Q22, the spline.** `volume<T>::splineinterpolate` → `Splinterpolator<T>` (`splinterpolator.h`:
  `calc_coef` copies the data as `T`, `deconv_along` per non-singleton axis, `SplineColumn::Deconv`
  in double, `Set` back to `T`; `value_at` sums `coef·w_x·wgt2` in double with `wgt2 = (w_z)·w_y`),
  `static_cast<T>` then `static_cast<float>`; `extraslice` maps to `Constant` (edge coefficient
  repeated). So `volume<double>` (applyxfm4D's double images) filters in double and rounds the value
  to float; `volume<float>` stores each pass in float. applyxfm4D's 4D path interpolates
  `invol[m]`, a `ShadowVolume` whose interpolation settings delegate to the owner; no difference
  from FLIRT's `-applyxfm` path but the type.
- **Q23, `leastsq`.** `costfns.cc`'s `p_leastsquares`, `p_leastsquares_smoothed` and
  `p_leastsquares_fully_weighted`: `sum`/`num` per row, `sumA`/`numA` per slice, `sumB`/`numB`
  total (all `float`; the unsmoothed count a `long int`), `sum += weight*(valx-valy)*(valx-valy)`
  (left to right), the weighted form's `weight = wval*refweight(x,y,z)` then the edge factors
  (`weight *= o1/smoothx`, ...), `lsq = sum/num` when `num > 1`, else `(Max(max,max) − Min(min,
  min))²`; chosen by `Costfn::cost` (`LeastSq`: smoothed when `smoothsize > 0`, the fully weighted
  form whenever weights are given).
- **Q24, the ties.** miscmaths' line search compares with `<` and `>` only; with every cost equal or
  NaN no step is accepted. Where the clean-room original leaves FSL's trace on the FSL-91 and FSL-92
  cases is `rotmat2euler`'s `atan2f` (the nc core's `fdlibm::atan2f`, glibc's
  `__ieee754_atan2f`): measured with a development hook on the replica
  (`crates-nc/larmorx-nc-flirt/dev-patches/crtrig_variant.patch`, `LXNC_CRTRIG=sin|atan|all`, never
  committed in the source): with a correctly rounded `atan2f` the replica leaves FSL's trace at the
  same line as the original (66,128 of `real/weights-empty-refweight`'s) and then follows the
  original for 21,353 more lines; with correctly rounded `sinf`/`cosf` only it stays FSL's. On
  `real/dof6` the correctly rounded `sinf`/`cosf` (`make_rot`'s `sincosf`) part at line 4,656 (the
  original's 3,983rd evaluation), `atan2f` alone at 4,814. The skew's print-only sign three lines
  before (FSL `-0.000000`, the original `0.000000`, `decompose_aff`'s `c = dot(y0,z)/sz` near 0) is
  not from these functions (the hooked replica keeps FSL's sign): the original's composition or
  decomposition arithmetic differs from `compose_aff`/`decompose_aff`'s somewhere.
- **Q25, mcflirt's centres.** mcflirt's centres are `volume<T>::cog("scaled_mm")` → `calc_cog`
  (`newimage.cc`): `val = (double)(vol(x,y,z) - vmin)` (the subtraction in `T`), `nlim =
  max(1000, (int64_t)sqrt(nvoxels))`, the flush when `++n > nlim`, `WARNING::in calculating COG,
  total = 0.0` below 1e-5; then `sampling_mat() * v`. Both the cost function's `testCog` (the
  rotations' centre, `Costfn`'s constructor) and `refvol.cog` (`mcflirt.cc`'s `.par`
  decompositions) use it.
- **Q38, the levels.** `flirt.cc`'s pyramid: `resample_refvol(refvol, min_sampling)` →
  `filter_resamp_blur` → `isotropic_resample` (`iso2aniso = diag(step)`, set forms only), then
  `refvol_2 = subsample_by_2(refvol)` when `min_sampling < 1.9` (else a copy), `refvol_4` from
  `refvol_2` below 3.9, `refvol_8` from `refvol_4` below 7.9 (`subsample_by_2`: sizes `(n+1)/2`,
  `setdims(2·d)`, `sub2mat = diag(2,2,2,1)` centred, set forms only; `copyproperties` keeps the
  unset ones); the debug saves follow (`save_volume(refvol_8, "refvol_8")`, ...). Without weights
  `global_refweight` is an empty `volume<float>`, filtered to the 1 × 1 × 1 zero volume whose
  `pixdim[4..7]` are newimage's defaults, 1. BBR without boundary points: `costfn()` re-runs
  `set_bbr_seg` before each evaluation, the cost is NaN, `optimise` keeps the start, the final
  matrix is the `-init` one, `final_transform` resamples with it (`affine_transform`; `bbr_resamp`
  only with a field map); the debug call's `bbr` evaluation writes `debugvol` (zeros).

## The replica's switches (2026-11-03, sub-task 4 of the spec answers)

- **FSL-111** (`applyxfm4d.rs`): a `DT_UNSIGNED_CHAR` file (not int8) is templated on `u8` in the
  default mode (`typed_main::<u8>`): read as stored, resampled with float spline coefficients (no
  FSL-79 comparison in that type), FSL-09's rounding clamped to 0..255. Beside it, volume by
  volume (`signed_bytes`), the signed reading FSL's `char` template makes is resampled as the
  default resamples an int8 image (its FSL-79 and FSL-09 notices kept); FSL-111 is named when the
  bytes written differ. `reproduce` keeps the `i8` template (FSL's). The cost: twice the
  resampling for uint8 inputs in the default mode only.
- **FSL-204** (`pointflirt.rs`, `procrustes`): `n == 0` with both matrices 0 x 0 (after the
  `--vox` and `-v` steps, whose lines FSL prints first) is refused in both modes (`End::Refuse`,
  `larmorx-nc: error: pointflirt: no points in the coordinate files (FSL-204)`, exit 1), where
  FSL's later submatrix copies of the empty products crash (signal 11).

## standard_space_roi and fslroi (2026-11-04, BET's -S/-B/-A/-A2 part, sub-task 2)

`standard_space_roi` (fsl-flirt 2111.4, a bash script, `#!/usr/bin/env bash`, `LC_ALL=C`
exported, no `-e`) is replicated in `crates-nc/larmorx-nc-flirt/src/ssroi.rs`; FSL's `fslroi`
(fsl-avwutils 2209.6 `fslroi.cc`) as a program in `crates-nc/larmorx-nc-avwutils/src/fslroi.rs`
(its region the core's `avwutils::fslroi`, now with all three argument forms); fslpy 3.27.0's
`imcp`/`immv` in `crates-nc/larmorx-nc-core/src/fslpy.rs`; where the script's data images are
found in `larmorx-nc-core/src/fsldata.rs`.

**How the script runs** (lines of FSL 6.0.7.23's script): `[ _$2 = _ ] && Usage` (112: the usage
heredoc on standard output with `${FSLDIR}` expanded, exit 1); `in=`remove_ext $1`` (114),
`imtest $in` (115: "Input image $in not valid", exit 1, on standard output); `out=`remove_ext $2``
(121); the option loop `while [ _$1 != _ ]` (133: an empty argument ends it; an unknown option
ends it too and goes, with the rest, to FLIRT as `$@`); `-ssref`, `-altinput`, `-roiMASK`,
`-maskMASK` check their value with `[ _$2 = _ ] && Usage` and then `imtest` ("standard space
reference image not valid", "Input image $2 not valid", "Image $2 not valid"); `-b` sets
`maskthr="-thr 0.5"`, `mask=2`, `maskmask=${FSLDIR}/data/standard/MNI152_T1_2mm_brain_mask_dil`,
`roi=3`. The work: `flirt -in $in -ref $standard -o ${out}_tmp_to_std -omat ${out}_tmp_to_std.mat
$@` (211), `convert_xfm -inverse` (212), `X=$(fslval $standard dim1)` then `dc` of `"$X 2 - p"`
(214 to 216); masking (mask 1 or 2): the FOV image `fslmaths $standard -mul 0 -add 1 -roi 1 $X 1 $Y
1 $Z 0 1 ... -odt float` (221: the reference's grid less its outer voxel on each side) or
`fslmaths $maskmask -bin ... -odt float` (223), `flirt -applyxfm -init ..._inv.mat` into the
input's space (225), `fslmaths ... $maskthr ...` (226), `fslmaths $inapply -mas ...` (227); then
the cut (roi 1 or 2): the same FOV or `$roimask -bin` (234, 236), its `flirt -applyxfm` (238) and
`fslroi $inapply $out `fslstats ..._roi_to_in -w`` (239), else `imcp $inapply $out` (241); the
cleanup `/bin/rm ${out}_tmp*` unless `-d` (248). The exit status is the last command's: `rm`'s
(1 when one of its names is missing or a directory), or 0 with `-d` (the `if` that runs no branch).
A program that crashes is reported by bash (`$0: line N: PID Aborted                 <the line's
text>`, N the command's own line at the top level but the `fi` of the `if` block it runs in:
229 for lines 219 to 229, 242 for 232 to 242) and the script goes on; `/bin/rm`'s messages name
it so (`/bin/rm: cannot remove 'x': No such file or directory`). bash's `[` on split words is
bash 5.1's `test` (`shtest.rs`: POSIX's rules up to four arguments, the `-a`/`-o`/`!`/`(`
grammar beyond; `[: too many arguments`, ``[: syntax error: `-x' unexpected``, `[: _a: integer
expression expected`, by which word follows), in `reproduce` only.

**The programs in-process.** Every program is the replica's own (`larmorx_nc_flirt::flirt`,
`convert_xfm`, `larmorx_nc_avwutils::fslmaths`, `fslstats`, `fslroi`, the core's `fslval`, `imcp`),
with its own messages, statuses and notices; an exception nothing in one catches is caught by
the script's runner, which prints the C++ runtime's text and bash's report (status 134) and goes
on. GNU `dc` of `"$X 2 - p"`: `scripts::dc`. First probe (`probe/quick.sh` in the relay's
scratch, ds000005's T1w to FSL's MNI152_T1_2mm, `reproduce`): the defaults, `-b -d` and
`-maskFOV -roiNONE -d` bit for bit, every temporary file included but for FLIRT's `descrip`:
the FLIRT replica wrote `larmorx-nc fsl flirt` where FLIRT writes newimage's build string; it
writes FSL's since (`run.rs`'s `DESCRIP`; FLIRT's golden header checksums move, data unchanged).

**The BET replica suite's new cases** (`fsl-bet --implementation replica`, standard tier, the
categories `fslroi/`, `ssroi/` and six of `defects/`, against FSL 6.0.7.23 through the oracle
cache, 2026-11-04, not a record): 55 cases, 42 identical (exit status, both streams with bash's
process IDs masked, every file byte for byte, the D13 fix pass), 12 refused by both alike, 1
expected divergence (`phantom-none-convert`, imcp's nibabel conversion, below); 0 fail. Real
T1w images (an adult, a child of 3.5 with `-b -d` as bet runs it, an 82-year-old with
`-maskFOV -roiNONE -d`, a tumour with `-roiMASK`, a Philips T1w with `-b -maskMASK`, a T2w) with
FSL's MNI152_T1_2mm, and the phantoms with `-ssref` for every option, output type and failure.

**fslroi's statuses (black-box runs of FSL 6.0.7.23).** `main` catches `std::exception` (newimage's
`Exception` is one), prints `what()` after the `Image Exception : #n :: msg` line, and falls off
its end: exit 0 after `Invalid ROI dimensions` (#21), `Out of Bounds (time index)` (#5: the ROI's
first read of a time point outside the series, through the const 4D accessor) and `No image
files match` (#63); the usage (argument counts other than 4, 8 and 10 after the program) exits 1.
A box reaching outside the image in x, y or z is filled with the extrapolation's zeros (a written
output, exit 0). The 32-bit `int` sums `max += min - 1` wrap as FSL's do. Note: the core's
`ToolFailure` gave status 1 for these exceptions, which the `bet` script's uses of `fslroi`
(`-F`, `-Z`) never reach (their regions are always valid); it now carries `exception`, and the
fslroi program applies FSL-215.

**Defects (D13; FSL-211 to FSL-215, register rows through the handoff):**

| ID | What | Rule | Default / `reproduce` |
|---|---|---|---|
| FSL-211 | the script expands `$1`, `$2`, `$in`, `$out`, `$standard`, `$inapply`, the masks and `$@` unquoted: a name with a blank is split (an input, reference or mask is then "not valid": `imtest` prints 0 for more than one word; an output writes and removes other files; an option with a blank makes `[` print `too many arguments` and ends the options; FLIRT's options are split again); an empty input makes `remove_ext` print its usage, which becomes the input's name | A | names as given / split as bash does, the `[` messages with their line numbers; a name with `*`, `?` or `[` is refused in `reproduce` (the shell's pathname expansion is not reproduced) |
| FSL-212 | `rm ${out}_tmp*` removes every file named so, also the user's | A | only this run's temporary files / the glob, rm's messages and status |
| FSL-213 | `maskthr` is never initialised: bash takes it from the environment, and line 226 applies it to the transformed mask (unless `-b` sets it) | A | ignored (named when set) / applied |
| FSL-214 | no step's status is checked: after a failure (FLIRT refusing an option, an image that cannot be written) the script runs the rest on missing files and exits with the cleanup's status, 0 when its temporary files exist, mostly without an output | B | stops at the first failed program, exit 1, temporary files removed unless `-d` / goes on |
| FSL-215 | fslroi exits 0 after the errors it reports | A | exit 1 / exit 0 |

**FSL-217 (found 2026-11-04, sub-task 5, rule A; the replica's correction pending):** fslroi
reads an image in newimage's closest templated type, which for int8 (`DT_INT8`) is
`DT_UNSIGNED_CHAR` (`closestTemplatedType`), and saves it so: the bytes are kept and the type
becomes uint8, so every negative value becomes 128 to 255 (the replica, a translation, gives
−64 → 192 in both modes today; FSL's own output to be confirmed by a parity case). The same
mapping is in `fslmerge` (the first input's type), which `bet -Z` runs on the slices fslroi cut,
and in `bet -F`'s first-volume fslroi: bet2 then runs on the wrapped values. fslmaths has the
same mapping for `-odt input` (FSL-157, corrected there). The default should keep int8 and its
values (named when a value is negative), as FSL-157's correction does; `reproduce` FSL's uint8.
uint16 → int32 keeps every value (rule C, FSL's design, as for fslmaths); uint32, int64 and uint64
→ float32 round integers beyond 2^24 (rule C, the type's design; documented in
`specs/standard_space_roi.md` §3.2).

**Not replicated yet:** fslpy's `imcp` of a source whose file type (nibabel's class and the
extension: NIfTI-1, NIfTI-2 or ANALYZE, single or pair, compressed or not) is not
`$FSLOUTPUTTYPE`'s: fslpy loads it with nibabel 5.4.2 and saves it in the output type (the header
copied, slope and intercept set explicitly). Only `standard_space_roi -maskNONE -roiNONE` (and
`-b -maskNONE`) on such an input reaches it; the replica refuses it in both modes (exit 1, "pending").
