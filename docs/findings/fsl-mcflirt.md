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
