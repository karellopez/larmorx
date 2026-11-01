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
| FSL-67 | BBR normals divided by the reference level's voxel sizes | c? | D | kept; measured in part 2 |
| FSL-68 | every error after the command line exits with status 0 (`main` catches all exceptions) | e | B | exit status 1 by default, with FSL's message; `reproduce` exits 0 |
| FSL-69 | an older image pair compares with the unblurred input after a `setscale` the data are too coarse for | c? | C | kept (pipelines' results depend on it), documented |
| FSL-70 | a schedule matrix name FSL cannot interpret (`Uq`, an empty word) or `U[` (a 28th list): an uninitialised or out-of-bounds pointer | a | — | refused, both modes, after FSL's message |
| FSL-71 | `setoption bbrstep` below 1: the boundary loop never ends (0) or wraps (negative) | a | — | refused, both modes |
| FSL-72 | `read` of a file with a word that is not a number: an endless loop | a | — | refused, both modes |
| FSL-73 | `-pedir` without a field map read (no `-fieldmap`, or `-applyxfm`, which never reads it): an empty field map interpolated | a | — | refused, both modes |
| FSL-74 | the usage lists `-displayinit` (the option is `-printinit`) and leaves `woods` out of `-cost` | f | — | the same text (messages kept) |

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
- **Open (part 2): FSL's heap corruption.** 281 findings: FSL aborts (`munmap_chunk(): invalid
  pointer`, `free(): invalid size`, `double free or corruption`) or segfaults on small or constant
  images (from one voxel up to some 30 per axis; with the default options too, e.g. a 1 x 1 x 1
  input against a 9 x 1 x 1 reference), where the replica finishes. gdb shows the crash in a later
  `free` (reading the next image), so the corruption happened earlier; it is not yet traced
  (`crash_probe.py` narrows the inputs). Once traced, the replica refuses those inputs in both modes.
- **Open (part 2): 66 matrices.** Registrations (45) and BBR (21) whose matrix differs, 64 with
  voxels below 0.5 or above 5 mm.
- **Open (part 2): the fix pass.** 8 cases where `fix` changes a result without naming a defect:
  infinite or NaN reference voxels (FSL-14's notice) and `-setbackground` with spline.
- **Open (part 2):** one stderr count (`Erroneous dof 3 : using 6 instead` once more than FSL,
  trial-flirt-01277), one more heap corruption with `-bins 16` (`corrupted size vs. prev_size`), and
  the replica over 3 GiB on two cases with 0.01 mm and 25 mm voxels (FSL's peak not yet measured).

## Not replicated yet (part 2)

Each is refused with a message (`larmorx-nc: error: ... not replicated yet`), never computed
differently:
- `-anglerep quaternion` (`construct_rotmat_quat`, `rotmat2quat`).
- The schedule command `alignpaxes` (`principleaxes_mat`: Armadillo's `eig_sym`, LAPACK `dsyevd`).
- General parameter-subset masks' pseudo-inverse (`dgesdd`).
- `-debug` with the BBR cost (the `debug` 4D volume of `set_debug_mode`).

Not refused but not yet compared with FSL: `-debug`'s other volumes (the pyramid, the weights; the
1 × 1 × 1 weight volumes FSL filters from empty ones).
- The other programs of fsl-flirt: `avscale`, `makerot`, `midtrans`, `rmsdiff`, `img2imgcoord`,
  `img2stdcoord`, `std2imgcoord`, `applyxfm4D`, `pointflirt`.
