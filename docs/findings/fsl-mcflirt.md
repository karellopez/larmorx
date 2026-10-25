# FSL mcflirt (FSL 6.0.7.17)

> **Forbidden to the clean-room implementer.** These notes were written while reading FSL's
> source, which is under the FSL Licence (non-commercial). Whoever writes the clean-room
> original of `lx.mri.hmc` (`crates/larmorx-mri`) must not open this file, FSL's source or
> `crates-nc/`; they work from `specs/mcflirt.md`, papers, FSL's documentation and black-box
> runs only (CLAUDE.md rule 1, `docs/licensing.md`). These notes are for the bit-exact
> replica in `larmorx-nc` and for whoever maintains the spec. They describe behaviour with
> file:line references and quote only short expressions and identifiers, never routines.

**Sources read** (`~/fsl/src/`, the conda build of FSL 6.0.7.17; versions in
`upstream.tsv`): `fsl-mcflirt` 2111.0 (`mcflirt.cc`, `Globaloptions.{h,cc}`, `Log.{h,cc}`),
`fsl-newimage` 2203.12 (`costfns.cc`, `newimage.{h,cc}`, `newimagefns.{h,cc}`, `newimageio.h`,
`generalio.cc`), `fsl-miscmaths` 2203.2 (`optimise.{h,cc}`, `miscmaths.cc`, `kernel.cc`,
`splinterpolator.h`), `fsl-armawrap` 0.6.0 (`operator_stream.hpp`), `fsl-newnifti` 4.1.0,
`fsl-utils` 2203.5 (`fslStartup.cc`). Line numbers refer to those files.

*Read* = from the source; *verified* = confirmed with the binary (oracle runs in
`<workspace>/oracles/fsl-6.0.7/mcflirt/runs/`, `scripts/run_mcflirt_oracle.py`).

## The program (`mcflirt.cc`)

- **Schedule** (`main`, `mcflirt.cc:622-827`). `float current_scale=8.0, new_tolerance=0.8`
  (:628). Stage 1: `new_tolerance = 8*0.2*0.5`, scale 8 (:653); stage 2: `4*0.2`, scale 4
  (:717); stage 3: `0.1` at the stage-2 grid (:746); stage 4 only if `no_stages >= 4`, with
  `NormCorr` switched to `NormCorrSinc` (:758-764). Unrun stages copy the matrices through
  (:711-713, :739-741, :751-753, :765-767). `new_tolerance` is a float. *Read; stage order
  verified with `-report`.*
- **Reference** (:648): `refnum = no_volumes/2` when `-refvol` is absent (`-1`), so
  `-refvol -1` means the default (*verified*). `-reffile` reads the image with
  `read_volume` (legacy mode: a 4D file is cut to its first volume with a warning,
  `newimageio.h:311-322`; *verified*) and sets `refnum = -1` (:656-660).
- **Reference grids** are `isotropic_resample(anisorefvol, scale)` (:691, :721) of the raw
  reference: no blurring (`newimagefns.cc:295-343`). The test volume handed to the cost is
  `timeseries[i]` at full resolution (`correct`, :428). *Read.*
- **Visiting order** (`correct`, :401-458): `i = refnum + direction`, forward to
  `no_volumes`, backward to `-1 - mean_cond`; with `refnum = -1` the backward pass would start
  at −2 and is suppressed by the special case at :422-423. *Read; order verified with
  `-report`.*
- **Previous-volume initialisation** (:453-455): after volume `i` the next index `i'` gets
  `mat_array_in[i'] = finalmat` only if `scaling == 8.0`, `i' < no_volumes - 1`,
  `i' > -1` and no `-fudge`. The `< no_volumes - 1` excludes the **last volume**, which starts
  from the identity: an off-by-one. *Read; verified with six identical displaced volumes
  (`syn/motion/identical-copies/reffile`): matrices 0 and 5 are bit-identical, 1–4 differ;
  with `-fudge` all six are identical.*
- **Optimiser call chain**: `usroptimise(offsettrans, dof, 1, new_tolerance)` (:446) →
  `optimise_strategy1` (:298-326, dof clamped to 6–12 with "Erroneous dof", *verified*) →
  `optimise` (:221-246) → `powell_opt` (:199-218) → `MISCMATHS::optimise(..., itmax=1,
  boundguess)` with the default type `"brent"` (`optimise.h:86-89`). So: one sweep of
  coordinate line searches, no Powell direction update, despite the function's name.
  `powell_opt` resets the parameters to the identity vector when their maximum absolute
  value is below 0.001 (:204, :191-195); with rigid starts the scales are 1, so this only
  fires for an all-NaN start (as we read armadillo's `max`, NaN entries never win the
  comparison). *Read; consistent with the NaN run, where only the NaN volume has a NaN
  matrix.*
- **Tolerances** (`set_param_tols`, :173-188): `{0.005 ×3, 0.2 ×3, 0.002 ×3, 0.001 ×3}`
  (the comment says "0.02 scale"; the code says 0.002), rotations divided by `-rotation`;
  multiplied by `new_tolerance` (:318). *Verified* through the in-plane mode's printout and
  `-rotation 2`.
- **Parameterisation** (`vector2affine`/`affmat2vector`, :99-162): Euler, `compose_aff` /
  `decompose_aff` about `impair->testCog`, the test volume's `cog("scaled_mm")` (costfns
  constructor, `costfns.cc:282-294`): a new centre for every volume and stage.
- **`costfn`** (:253-281): `affmat * initmat`, with `initmat` reset to the identity at
  :707. So `-init` (read at :769-774) only enters the final resampling (:787-804), as
  `mat_array0[i]*init_trans`. *Verified:* matrices identical with and without `-init`.
- **In-plane mode** (`fix2D`, :388-398; `double_end_slices`, :368-386): triggered by
  `zsize < 3 || zsize*zdim < fov || twodcorrect`; sets the global `twodcorrect = 1` and
  `smoothsize = 0.1` for the rest of the run. `double_end_slices` builds a new volume with one
  replicated slice at each end and `setdims(xdim, ydim, 8.0f)`; the sform/qform are not
  carried. Main applies `fix2D` to the stage-1 8 mm grid (:693) and `correct` applies it again
  to its copy (:411-412), so the stage-1 reference is padded twice; stages 2–3 once. Only
  params 3–5 (rz, tx, ty) are optimised; `optimise` prints `Params:` and `Tolerances:` to
  stderr for every volume, unconditionally (:238-244). `-fov` is parsed with `atoi`
  (`Globaloptions.cc:228-231`). *Verified* on a 4-slice slab (also with `-fov 10`).
- **`-gdt`** (:434-440, :700-705, :728-734): test volumes are replaced by `gradient(...)`,
  but for the reference the gradient is computed into `gtempvol` and discarded; the raw
  reference is saved as `grefvol_<out>` and used. Broken. *Read.* The name is the prefix
  `grefvol_` glued to the whole `-out` string, so an absolute `-out` path gives an invalid
  file name and mcflirt aborts. *Verified* (`real/ds000005/opt-gdt`).
- **`-edge`** is never parsed (`Globaloptions.cc` has no such option; `edgeflag` stays 0),
  so the `fixed_edge_detect` branches (:430-433, :695-699) are dead. nipype's
  `use_contour` emits `-edge`, which mcflirt rejects. *Verified.*
- **`-scaling`** sets `globalopts.scaling`, which nothing reads; `estimate_scaling`
  (:286-294) is never called. `-hist` sets an unused flag. `eval_costs` (:556-566, a
  hard-coded `/usr/people/prb/...` path) is unreachable (`costmeas` is never set). *Read.*
- **`-meanvol`** (:663-686): mean of `affine_transform(timeseries[i], ..., mat_array1[i],
  1.0)` with trilinear/extraslice, summed in float and divided by `(float)no_volumes`, saved
  as `outputfname + "_mean_reg"`; then `refnum = -1`, `mat_array0` reset, `mean_cond = 1`.
  *Read; naming verified.*
- **Final resampling** (:776-807): `testvol` = `extrefvol` (with `-reffile`) or the volume
  itself, `affine_transform(timeseries[i], testvol, mat*init, 1.0)`; precedence sinc → nn →
  spline → trilinear; extraslice extrapolation. `timeseries[i] = testvol` copies data only
  (`ShadowVolume::equals` → `copydata`, `newimage.cc:362-369`), which throws "Attempted to
  copydata with non-matching sizes" when the reference has another voxel count. This happens
  after the whole estimation and before any matrix is written. *Verified.*
- **`-stages 0` with `-reffile`**: `extrefvol` is read only inside the stage-1 block
  (:655-660), so the final resampling gets an empty volume and aborts ("Attempted to use
  affine transform with no voxels in vout"). *Verified.*
- **Outputs** (`decompose_mats`, :460-554):
  - `.par` centre: `refvol.cog("scaled_mm")` of `extrefvol`, `meanvol`, or
    `timeseries[refnum]` *after* the final resampling (:810-821); the identity resampling of
    the reference volume returns it unchanged. *Verified* (centre of mass with the minimum
    subtracted).
  - RMS centre `0.5*(n-1)*dim` of the same volume, `rmax = 80` (:467, :481-483);
    `rms_deviation(mat[i-1], mat[i])` and `rms_deviation(I, mat[i])` (:496-517); means over
    `N-1` and `N`. *Verified.*
  - The reference index writes `IdentityMatrix(4)` if `tmpmatflag` (set by `-mats`, `-plots`,
    `-rmsrel`, `-rmsabs`), even without `-mats`; when the `.mat` directory was not created
    (only `-plots`) the logger's directory is empty and the file goes to `"/MAT_xxxx"`, which
    fails silently (:519-536). *Read; verified:* `-rmsrel` alone leaves only `MAT_0006` (the
    reference) in the directory, and `-plots` alone creates none (`syn/cli/{rmsrel,plots}-only`).
  - `.par` line: six values each followed by `"  "`, default stream format (:532-534).
- **Exit paths** (`Globaloptions.cc:77-284`): `argc < 3` → usage, exit 1 (so `-help` alone
  exits 1; *verified*); missing input → exit 2; "Lacking argument" (checked before the
  unknown-option test, so an unknown option in last position reports a missing argument) and
  "Unrecognised option" → `exit(-1)` = 255; unknown `-cost` names keep NormCorr, with a
  message only when `-report` came earlier (:232-253). *Verified.*
- **Default output name** (:275-280): `make_basename(input) + "_mcf"`. *Verified.*

## The cost function (`fsl-newimage/costfns.cc`)

- **Dispatch** (`Costfn::cost`, :336-402): NormCorr → `1 - fabs(normcorr_smoothed)` when
  `smoothsize > 0` (always in mcflirt unless `-smooth 0`), else `1 - fabs(normcorr)`;
  NormCorrSinc → `1 - fabs(normcorr_smoothed_sinc)`. Woods never uses the smoothed variant.
- **The count bug** (`p_normcorr_smoothed`, :1788-1902; the same in
  `p_normcorr_smoothed_sinc`, :1908-2018). The weighted sums use three float accumulators
  per quantity (row, slice, total), and row/slice partial sums are reset after being added on
  (:1878-1885). The weight count `num` is added to `numA` after each row (:1878) and `numA` to
  `numB` after each slice (:1883), but **neither `num` nor `numA` is ever reset**. So the
  final `num = numB` (:1889) is Σ over slices of the running sum of the running sum of row
  weights, about 10⁴ times the true weight sum on a 4 mm grid. In
  `varxy = sumxy/(num-1.0) - (sumx*sumy)/(num*num)` (:1894) the mean terms become negligible
  and the "normalised correlation" is effectively uncentred. The unsmoothed `p_normcorr`
  (:1691-1782) counts correctly (`long num`, no partial-sum mix-up), and so does
  `p_leastsquares_smoothed` (:2302-2306). *Read; verified indirectly:* a ramp series shifted
  along x is realigned by default and not with `-smooth 0`
  (`syn/content/ramp/reffile*`). The same functions serve FLIRT's `-cost normcorr`.
- **Mixed precision** in that formula: `num-1.0` is double (the literal), so
  `sumxy/(num-1.0)` is a double division; `(sumx*sumy)/(num*num)` is float; the difference
  is stored in a float. `corr = varxy/sqrt(varx)/sqrt(vary)` in float. *Read.*
- **Sampling** (`findrangex`, :583-675; loops, e.g. :1829-1886): `iaffbig =
  vtest.sampling_mat().i() * aff.i() * vref.sampling_mat()` in double, entries cast to
  float `a11..a34`; row start `o = y*a12 + z*a13 + a14`; analytic range with
  `xb2 = (float)n - 1.0001`, `ceil`/`floor`, then a brute-force walk that drops leading
  out-of-bounds points and truncates at the first later one; end points additionally pass
  `in_interp_bounds` (:116-124). *Read.*
- **Trilinear** (`q_tri_interpolation`, :127-158): `(int)` truncation, bounds check against
  `n-1` (`SAFE_FLIRT`, :87), x-then-y-then-z lerps of the form `(b - a)*d + a`; pad value
  outside. *Read.*
- **Edge weights** (:1817-1820, :1854-1861): `smoothx = smoothsize / vtest.xdim()`, weight
  `o1/smoothx` or `(xb2-o1)/smoothx`, product over axes, clamped at 0. *Read.*
- **Stage-4 sinc** (`q_sinc_interpolation`, :229-277; kernel table :186-226): Hanning
  half-width 3 in a static 201-entry table built lazily on first use (`Globalkernelwidth`
  starts at 0); window clipped to the volume; normalised by the kernel sum; `backgroundval()`
  if `|kersum| ≤ 1e-9`. *Read.*
- **Histogram costs**: bins from `set_no_bins` (:3622-3666, `bin_a1 = no_bins/(max-min)`,
  `bin_a0 = -min*no_bins/(max-min)`, index `(int)(v*a1 + a0)` clamped); corr ratio
  :1176-1318; Woods :1456-1561 (`Sqr(num[b])*stdev/sum[b]`, i.e. n_b², likely meant n_b);
  MI/NMI with fuzzy binning `fuzzyfrac = 0.5` and the overlap correction :3149-3372. *Read.*

## The optimiser (`fsl-miscmaths/optimise.cc`)

- `optimise` (:311-385): `fval = 0` per call; per direction `optimise1d(pt, e_n, tol, ...,
  100, fval, boundguess(min(it, 2)))`; with `max_iter = 1` (mcflirt) the convergence test
  (`avtol < 1`) and the Powell correction (`type == "powell"` only) never matter. *Read.*
- `optimise1d` (:213-306): `unittol = |1/Σ|dir_n/tol_n||` in float; `x1 = boundguess *
  unittol` with `boundguess = 10` (`Globaloptions.h:190-191`); `init_value == 0.0` triggers a
  re-evaluation (:242). Refinement while `++it <= 100` and `|(x2-x1)/unittol| > 1` (:254);
  the `it > 0` test makes `nextpt` always used (:258-262); the `min_dist = 0.1*unittol` and
  `0.4/0.5*unittol` adjustments in that order (:267-285). *Read.*
- `findinitialbound` (:147-210): extrapolation factor 1.6, maximum 3.2 (×(x2 − xmid));
  prints "findinitialbound failed to bracket" when it ends unbracketed. *Read.*
- `estquadmin` (:91-111), `extrapolatept` (:114-126, `0.3819660`), `nextpt` (:130-143). *Read.*

## Geometry and maths (`fsl-miscmaths/miscmaths.cc`, `fsl-newimage`)

- `construct_rotmat_euler` (:799-837) composes `make_rot` about the centre for x, then y,
  then z, then adds translations. `make_rot` (:899-939) stores `theta = norm2(angl)` in a
  **float** (:905-906) while `axis = angl/theta` is double, so the axis is not exactly unit
  length and `R` is orthogonal only to ~1e−8; the in-plane mode's `.par` shows `tz ≈ 7e−6`
  where the matrix has none (*verified*, `syn/motion/small/2d`).
- `rotmat2euler` (:960-988): float `cz, sz, cy, sy, cx, sx`; gimbal branch for `cy < 1e-4`.
  `decompose_aff` (:1021-1070): float scales and skews; translation `A·c + a − c` in double.
- `rms_deviation` (:1127-1154): `isodiff = a1*a2.i() - I`; `sqrt(tr·tr +
  rmax²/5·trace(AᵀA))`, returned as float.
- `volume::cog` (`newimage.cc:1464-1477`) and `calc_cog` (:1537-1594): `val = v - min`
  (float subtraction, then double), partial sums flushed every `max(1000, √nvox)` voxels,
  `total < 1e-5` → 1 with a warning; scaled by `sampling_mat` (pixdim only, no origin, no
  flip, :1230-1239). *Verified* (minimum subtracted).
- **Left–right order**: `NiftiGetLeftRightOrder` (`generalio.cc:190-222`) and
  `volume::left_right_order` (`newimage.cc:2132-2156`): determinant of the sform if set, else
  the qform; inconsistent signs or `|det| < 1e-12` give codes that are neither
  radiological nor neurological. `readGeneralVolume` swaps neurological volumes to
  radiological (:324) and `save_basic_volume` swaps them back (:408-417). `pixdim` made
  absolute, 0 → 1 (:311-312). *Verified* (RAS/LAS invariance).

## Resampling (`fsl-newimage/newimagefns.{cc,h}`)

- `isotropic_resample` (`newimagefns.cc:295-343`): float `step = scale/dim`, sizes
  `(int)Max(1.0, n/step)`, positions accumulated with `fx += stepx`, values by
  `aniso.interpolate` with the volume's methods (trilinear, zeropad by default,
  `newimage.cc:229-234`). *Read.*
- `affine_transform(vin, vout, aff, 1.0)` (`newimagefns.h:862-889`) sets the pad value to
  `vin.backgroundval()` and extraslice extrapolation, calls `raw_affine_transform`
  (`newimagefns.cc:130-223`: loops z, x, then y innermost; LR swaps only for neurological
  volumes, which never occur after reading) and `affine_transform_mask` (:232-274, positions
  outside `[-1, n]` get the pad value). *Verified* (fill value).
- `backgroundval` = `calc_bval(vol, 2)` (`newimage.cc:1481-1532`): sorted 2-voxel border
  shell, element `numbins/10`. *Verified* (90 on the background case).
- extraslice (`newimage.cc:1087-1095`): index −1 → 0 and n → n−1, other outside indices →
  pad value. `in_extraslice_bounds` (`newimage.h:463-469`) guards spline interpolation.
- `-sinc_final`: `setinterpolationmethod(sinc)` defines a Blackman kernel of full width 7
  with 1201 samples on first use (`newimage.cc:579-582`, `newimage.h:597-600`,
  `kernel.cc:81-156`); `kernelinterpolation` (`newimage.cc:620-683`).
- `-spline_final`: `Splinterpolator` with `Constant` extrapolation (`newimage.cc:1186-1199`),
  order 3, prec 1e-8 (`splinterpolator.h:109`); the causal initialisation is the mirror sum
  (:1633-1653), the anti-causal one `-z/(1-z²)·(2c[n-1] - lv)` (:1664-1688); `Constant`
  clamps coefficient indices (:1209-1212). *Read.*
- `-nn_final`: `MISCMATHS::round` (`miscmaths.cc:470-480`), half away from zero.

## Writing (`fsl-newimage/newimageio.h`, `generalio.cc`)

- Output type: `dtype(inputfname)` (`generalio.cc:565-575`): any `scl_slope != 1.0 ||
  scl_inter != 0.0` gives `DT_FLOAT` unless the slope is 0 or the type is double (a NaN
  slope gives float); then `closestTemplatedType` (:525-551): int8 → uchar, uint16 → int32,
  uint32/int64/uint64 → float. *Verified* for uint16, float64 and scaled int16.
- `save_volume_dtype` (`newimageio.h:402-416`) → `copyconvert` → `convertbuffer`
  (`newimage.h:884-892`): a plain C cast, so truncation toward zero and no clamping.
  *Verified* (int16 = trunc(float32 output)).
- Header: `set_fsl_hdr` (`newimageio.h:331-374`: `scl_slope = 1`, `scl_inter = 0`, units 10,
  `sliceOrdering = 0`, `cal_min/max` = display range); `descrip = BUILDSTRING`
  (`generalio.cc:388`, here `2203.12-dirty 2024-02-01T16:17:47+00:00`); mcflirt sets the
  display range to the series' max/min before saving (`mcflirt.cc:825`). *Verified.*
- `FSLOUTPUTTYPE` is required (`generalio.cc:150-174`); unset → exit 1 at the first save,
  i.e. after the `.mat` files are written. *Verified.*
- `.mat` files: `Log::out(..., false)` (`Log.h:112-122`) streams the matrix with armawrap's
  `doPrint` (`operator_stream.hpp:13-31`): fixed notation, the stream's default precision 6,
  one space after each value. `Log::establishDir` (`Log.cc:79-103`) appends `+` until
  `mkdir` succeeds (*verified*); a log file named by an empty `logfilename` is opened in it and
  never written.
- `-stats` (`mcflirt.cc:569-618`): mean/variance/sigma loops run `z = 1 … nz-2`, so the end
  slices stay 0. Names are `outputfname + "_meanvol"` etc., with the image extension added
  by the writer, e.g. `x.nii.gz_meanvol.nii.gz` (*verified*); nipype's `MCFLIRT` expects
  `x_variance.nii.gz` for FSL ≥ 6 and does not find them.

## Threads and determinism

`fslStartup.cc:70-85` disables OpenBLAS threading before `main`; newimage volumes default to
one thread. No random numbers. *Verified:* three runs of fMRIPrep's command on ds000005 give byte-identical outputs (`real/ds000005/fmriprep-repeat*`).

## The replica (`larmorx-nc`, 2026-10-24)

The bit-exact replica (`crates-nc/larmorx-nc-mcflirt` and `larmorx-nc-core`) was translated
from the files above. What the translation had to get right, beyond the source's reading:

**The binary follows the source (rule 8).** The conda build's libraries and `mcflirt` were
compiled by conda-forge GCC 13.2.0 (producer strings `GCC: (conda-forge gcc 13.2.0-3|4)`).
`objdump -d` finds no fused multiply-add in `mcflirt`, `libfsl-newimage.so`,
`libfsl-miscmaths.so`, `libfsl-NewNifti.so`, `libfsl-znz.so` or `libfsl-utils.so`, so every
`float` and `double` operation is rounded where the source rounds it; x87 instructions occur
only in `libfsl-NewNifti.so` (102: its `long double` quaternion code, emulated exactly in
`x87.rs`). With the translation following the source operation by operation, all 137
recorded runs are bit-identical (below), so no source-built oracle is needed. *Verified*
(2026-10-24).

**Precision decided by the compiled code** where the source is ambiguous (each checked in the
disassembly): `q_kernelval`'s second product `kernel[n+1]*dn` is a float product (`mulss`)
while the first is double (`1.0 - dn` is a double literal; `costfns.cc`); `sincfn`'s small-`x`
branch is the float `1.0f - |x|`; `kernelval`'s index arithmetic is float throughout and its
interpolation double (`ColumnVector`); `hanning` and `blackman` evaluate in double and return
float; `MISCMATHS::dot` returns float; `Min`/`Max` take the second argument's type;
`round(float)` adds 0.5 in double and truncates; `make_rot` calls `sincosf` on a float angle
(GCC pairs `sin`/`cos`); `rotmat2euler` calls `atan2f`.

**Armadillo's evaluation (fsl-armawrap 0.6.0, Armadillo 10.5.1).** armawrap's NEWMAT
expressions become Armadillo expressions whose evaluation order sets the bits: `inv(A)*B*C`
is `solve(A, B*C)` (for 4 × 4: `inv_tiny(A)` then a large-matrix product with
`direct_dot_arma`'s two accumulators), `A*inv(B)*C` is `A*solve(B, C)`, three plain factors
`(A*B)*C`, four `((A*B)*C)*D`; products of at most 4 × 4 use `gemm_emul_tinysq` (left-to-right
sums); `accu` and `dot` use two accumulators up to 32 elements; `max` keeps one running
maximum from `-inf`; `inv` of a 4 × 4 uses the cofactor formula when the determinant is in
`[eps, 1/eps]` and the result checks within 1e-10, else falls back. Larger products go to
BLAS (OpenBLAS 0.3.26 at run time); mcflirt never reaches them, and the replica panics if it
ever does.

**The C library's maths: mcflirt as built calls glibc 2.35.** On the oracle machine (a CPU
with FMA) glibc selects `__sinf_fma`, `__cosf_fma`, `__sincosf_fma` and `__logf_fma`; `atan2f`
and `atanf` are fdlibm's. None is correctly rounded: over all 2^32 floats `sinf` misrounds
29,362,812 inputs (0.68 %), `cosf` 28,209,642, `logf` 416,908 (GLIBC-02, -03); at the angles
mcflirt's searches use (|x| ≤ 0.5) `sinf` misrounds 0.063 %, `cosf` 0.0015 %, and `logf`
0.020 % of probabilities. mcflirt calls `sincosf` at every cost evaluation (`make_rot`), `logf`
in the mutual-information costs and `sinf` for the sinc kernels: of the Blackman kernel's 1201
samples (`-sinc_final`, the fourth stage) 24 `sinf` arguments are misrounded and 20 samples
differ from correctly rounded ones; glibc's double `cos` agrees with the correctly rounded one
on all 2402 window arguments, and so do the 201 samples of the stage-4 cost's Hanning table
and `log` of every bin count up to 4096 (`crates-nc/larmorx-nc-core/tools/glibc-float-verify`).
With correctly rounded functions (`larmorx_core::math`), 12 of the 79 synthetic recorded runs
differed from mcflirt as built, and each was bit-identical to mcflirt run with the CORE-MATH
preload (`oracles/core-math-preload`; the probe is
`validation/investigations/fsl-mcflirt-replica/crm_compare.py`). The replica therefore
carries glibc 2.35's float functions as the FMA build computes them, translated from Arm's
Optimized Routines v19.11 (MIT; glibc's are the same code: identical expressions and
constants) with every `a + b*c` a fused multiply-add (`larmorx-nc-core/src/glibc_float.rs`):
bit-identical to the C library on all 2^32 inputs of `sinf`, `cosf`, both outputs of
`sincosf`, and `logf`. *Verified* (2026-10-24). On a CPU without FMA glibc selects its SSE2
build, which differs on 12 `sinf` and 22 `cosf` inputs (GLIBC-03): there mcflirt's own results
can differ from the replica's (and from mcflirt's on this machine).

**fdlibm's constants are its decimal literals, not its comments (corrected 2026-10-25).**
`rotmat2euler`'s `atan2f` and the `atanf` it calls are fdlibm's code in glibc 2.35 (the
disassembly of `__atan2f_finite` and `atanf` follows `e_atan2f.c` and `s_atanf.c` operation by
operation, without fused multiply-adds). The first translation took `aT[0]` from the source's
hex comment, `0x3eaaaaaa`; the compiler reads the decimal literal `3.3333334327e-01`, which
rounds to `0x3eaaaaab`, and that is the value in glibc's `.rodata` (every other constant of the
two functions matches its comment). The comment's value changes `atanf` by one ulp on
1,037,658 of the 2^32 inputs (882,244 of them below 0.4375, the branch small rotation angles
take). Two fMRIPrep calls added to the oracle panel on 2026-10-25 reached such an input:
ds002422's rest run (1.5 T, a 79 mm slab), where one of mcflirt's 600 rotation decompositions
(volume 166, the start of the third stage: `atan2f(0x3c7c2fbe, 0x3f7ff83d)`, glibc
`0x3c7c324a`, the port `0x3c7c324b`) moved that volume's matrix, and ds000256 CTS203's
rest-fixation-no-feedback run (mean framewise displacement 4.97 mm), where the first stage's
chain carried the difference from volume 122 to the last. The cause was isolated with
LD_PRELOAD interposers on FSL's own functions (`validation/probes/fsl-mcflirt-affine-trace`):
volume 166's second stage was identical evaluation by evaluation (26 matrices and costs, as
bits), FSL's cost function gave the replica's costs at the replica's matrices, and the replay
of mcflirt's 600 `decompose_aff` calls through the replica
(`crates-nc/larmorx-nc-core/tools/affine-trace-replay`) found the one call and its `atan2f`.
With the corrected constant `atanf` is bit-identical to glibc on all 2^32 inputs and `atan2f`
on every pair of 38 special values and on 3 × 2^30 seeded random pairs (random bits,
rotation-like pairs, pairs in [-1, 1]; `tools/glibc-float-verify atanf atan2f`), and both
calls give mcflirt's matrices. mcflirt itself is deterministic on both (two runs identical);
with the CORE-MATH preload 1 of ds002422's 200 and 64 of ds000256's 164 matrices change, so
glibc's own rounding is what the replica must carry. glibc's `atanf` is not correctly rounded
on 21,089,462 inputs (0.49 %), 7,060 of them with |x| ≤ 0.05. *Verified* (2026-10-25).

**Corrections and reproductions found while translating** (register entries FSL-20, FSL-21;
FSL-04 now reproduced):
- `-gdt` (FSL-04) is reproduced with `reproduce`: each test volume is replaced by its 3×3×3
  gradient magnitude (`make_grad_masks`, float sums in mask order), the reference stays plain,
  and `grefvol_<-out>` is the plain reference; with a relative `-out` it runs to the end.
- `Log::establishDir` (`Log.cc`) creates `<out>.mat`, adding `+` while the name exists; a
  permission error prints "Error! Could not create directory: ... No write permission" and goes
  on; any other error (a missing parent directory) prints "Error!: Unknown Log directory return
  value" and `exit(0)`s before the `.par`, `.rms` and image are written (FSL-20).
- The histogram costs take `no_bins / scaling` bins at each stage (`mcflirt.cc` `usrsetscale`):
  with `-bins` below 8 the first stage has none. The mutual-information costs and Woods' then
  compute a degenerate cost and the search ends at arbitrary matrices (deterministic:
  reproduced); the correlation ratio adds bin 0 into bin -1 (`costfns.cc` `numy[no_bins-1] +=
  numy[no_bins]`), before its arrays, and glibc aborts when they are freed ("double free or
  corruption (out)"). The replica reproduces the abort when the written values are non-zero
  (FSL-21).
- An `-out` whose `.mat` directory cannot be made, an absent `-in`, a missing or malformed
  `-init` (`Could not open matrix file`, armawrap's `AWException`, Armadillo's "matrix
  multiplication: incompatible matrix dimensions"), `-dof` out of range ("Erroneous dof"),
  `-verbose 20` (every evaluated matrix, after which `cerr` stays in fixed notation:
  armawrap's `operator<<` sets it and never restores it): all reproduced to the byte.

**Threads in the replica.** Within a pass of a stage that does not chain its volumes (stages
2-4; the first with `-fudge`), each volume's search reads only the reference, its volume and
its start, so the replica runs them in parallel and prints their messages in mcflirt's order;
the final resampling is parallel over volumes. The first stage's chain and the mean volume's
sum stay sequential. Outputs are byte-identical at 1 and 3 threads (golden and parity tests).

**fMRIPrep's mcflirt is not FSL 6.0.7.17's, but gives the same matrices.** fMRIPrep 25.2.6's
image has fsl-mcflirt 2111.0 with fsl-newimage 2501.4, fsl-miscmaths 2412.4 and fsl-armawrap
0.7.0 (FSL 6.0.7.17: 2203.12, 2203.2, 0.6.0), and glibc 2.35. On the 22 mcflirt calls of the
12 fMRIPrep oracle runs that have BOLD runs (the five first runs and the data panel's: a child's
extreme motion, 1.5 T, multiband-8 with 600 volumes, multi-echo, a stroke, a glioma), its `.mat`
files are identical as text to FSL 6.0.7.17's and to the replica's: 4,517 of 4,517 matrices (the
replica's record, `docs/validation/fsl-mcflirt-replica.md`, the `fmriprep/` cases). *Verified*
(2026-10-24; the panel's calls 2026-10-25).

**Random testing (2026-10-25).** The differential random tests (`larmorx_validation.difftest`,
small generated phantoms, 4-8 volumes, at most 32 × 32 × 20 voxels) found the replica differing
from mcflirt in 61 of their first cases, each shrunk to a minimal recipe
(`oracles/difftest/2026-10-25/findings/fsl-mcflirt/`). Replayed with a build that has fdlibm's
comment value of `aT[0]` (`0x3eaaaaaa`), 58 of the first 59 shrunk recipes still differ; with the
corrected constant all 61, shrunk and original recipes alike (122 runs), are bit-identical to
mcflirt in every file, both streams and the exit status. The other one (`:616`, `-meanvol` with
`-out out_mcf.nii.gz`) was the replica's own: its corrected default names the `-meanvol` image
after the stem (`out_mcf_mean_reg.nii.gz`, as the clean-room original does under FSL-08) but did
not name FSL-08 in a notice; it does now, whenever the correction changes the name. Five of the
findings and that one are permanent cases (`validation/difftest/cases/fsl-mcflirt/`), and three
moving phantoms of the suite's own (`edge/atanf-phantom-*`, smoke tier) reach the same `atanf`
arguments: they fail with the comment's constant and pass with glibc's.

**Memory.** The record compares files and counts changed values in chunks (16 MB), so a
600-volume series (ds004958's multiband-8 run, 1.7 GiB as float) is never held whole (before, the
suite's own process reached 7.75 GiB on it); the replica peaks at about 7 GiB there, and the full
tier runs in a 16 GiB memory box
(`scripts/orchestration/boxed.sh`).

**What the replica does not reproduce:** the abort's signal (it prints what mcflirt prints and
one line of its own, then exits with status 1), FSL's build string in `descrip`, and mcflirt's
single thread.

## fMRIPrep, nipype, niworkflows, nitransforms (Apache-2.0 / MIT; not restricted)

- nipype 1.12.0 `fsl.MCFLIRT(save_mats=True)` with `in_file` and `ref_file` builds
  `mcflirt -in <in> -out <cwd>/<stem>_mcf.nii.gz -reffile <ref> -mats` (*verified* with the
  workspace venv). `_list_outputs` predicts `<out>.mat/MAT_%04d` for every volume.
- niworkflows `MCFLIRT2ITK` (`niworkflows/interfaces/itk.py:58-79`) and nitransforms'
  `FSLLinearTransform.to_ras` (`nitransforms/io/fsl.py:81-104`, `_fsl_aff_adapt` :211-224):
  zooms from the affine's column norms (FSL uses `pixdim`) and a flip when `det(affine) > 0`.
- fMRIPrep's confounds (`fmriprep/interfaces/confounds.py:102-229`) recompute motion
  parameters, RMS deviation and FD from the ITK transforms; the parameter centre is
  `zooms × scipy.ndimage.center_of_mass(boldref)`, with no minimum subtracted (FSL subtracts
  it) and in file order (FSL works in radiological order).
