# NOTES: field maps part A (sdcflows' field-map estimation port)

Worktree: <workspace>/larmorx/.claude/worktrees/agent-ad5d6c85d74dc98ba
(branch of main 2b453c1; relay 1 started there, no fast-forward needed)
Scratch: <scratch>/fmap-a-1/
(helper scripts in `scripts/` there: find_nodes.py, node_inputs.py, fndiff.py, probe_colmat.py;
commit the ones behind findings under validation/investigations/sdcflows-fieldmap/ in relay 5)

Part's agents (ids for the audit's archive step): relay 1 = ad5d6c85d74dc98ba (the harness's
worktree agent-ad5d6c85d74dc98ba); relay 2 = a52fb97af12fd7a98 (worked in relay 1's worktree);
relay 3 = ab2c4c87b023ea94e (same worktree; its transcript's meta says "Field maps A, relay 3");
relay 4 = aed354163dc359170 (same worktree; meta "Field maps A, relay 4 (apply, writer)");
relay 5 = add71b54eddc33c55 (same worktree); relay 6 = a12f04869a7af7743 (same worktree; meta
"Field maps A, relay 6 (records, gates)"); relay 7 = abf1e3da6428896bb (same worktree; meta
"Field maps A, relay 7 (fix findings)"); relay 8 = a3f6fcb919202dd47 (same worktree; meta
"Field maps A, relay 8 (records)"); relay 9 = a7b32ff0ee5bd279b; relay 10 = a1349b2efb577f2fe (both
from the orchestrator); relay 11 = the handoff and the final report (same worktree; its id not visible
to the agent: the orchestrator's to add).

## Status by relay
- **Relay 1 (done, commits 71991275, eefadced):** pins, inventory, design, relay plan (below);
  crate `crates/larmorx-sdcflows` with the B-spline grid and its weights, their Python wrappers
  and bindings, bit-identical to sdcflows in both environments (see "Where relay 1 stopped").
- **Relay 2 (done, commits a0cdaaca, bf21e596, bc7f0d1d, d38c12bc):**
  `blas` (OpenBLAS Haswell sdot, numpy's dot incl. its one-element scalar path, norm), `ridge`
  (SciPy cg; scikit-learn sparse_cg, both systems; Collocation and stored-CSR operators),
  `approx` (BSplineApprox's core: recentring, the fit, field, error, extrapolation, log, the
  failed-fit error; resize via a caller-given resampled grid), `npy::py_repr`/`py_format_2e`;
  `lx.sdcflows.ridge_sparse_cg` (+ RidgeResult); `_core` bindings; check_ridge.py and
  check_approx.py bit-identical in both envs (details below).
- **Relay 3 (done, commits 09fb3f66, ccc347a5):** `lx.sdcflows.bspline_approx` complete
  (files, headers, logs, failures), `resample_to_zooms`, `rescale_affine`,
  `topup_coeff_reorient`, `transform_coefficients`, `reorient_pedir`, `B0FieldTransform.fit`,
  `deoblique_and_zooms`; Rust `tools`; `_core.nifti_set_xform`; three checks, all
  byte-identical in both envs and to fMRIPrep's own files (see "Where relay 3 stopped").
- **Relay 4 (done, commits 0f91e274, 1c48bc82, 3c536a51, f0347e61, e323b7f9, 38965c9a,
  af14b02b):** nibabel's scaled writer (`save_nibabel`), `B0FieldTransform.apply` +
  `to_displacements`, `apply_coeffs_field`, `fmap_to_disp`/`disp_to_fmap`/
  `displacements_field_to_fieldmap`, the whole phase-difference preparation (`phase.py`) with
  `phdiff_workflow`; SDC-07 measured (rule D -> A); switches SDC-04, 05, 07, 08, 09; all checks
  byte-identical in both envs (see "Where relay 4 stopped").
- **Relay 5 (done, commits f9698a49, d6523175, d870ebac, d537b523, fc7e195f):** save_nibabel
  in larmorx.io; SDC-01, -02 (measured), -06, -10 (new) switched; API page, findings doc,
  PROVENANCE; register texts drafted (`<scratch>/HANDOFF-register-draft.md`); parity suite
  `sdcflows-fieldmap` wrapping the nine checks, all ALL OK in both envs (see "Where relay 5
  stopped").
- **Relay 6 (done in part, commits 18e0418c, 0d7b77c6, c397728e, 8aa567ed, 9e39e0ab,
  f236aedb):** the originals' step in the suite (STEP_SUITES, stepdigest), 15 golden
  cases (no checksums yet), memory calls (pass), the difftest descriptor + runner, a 2-minute
  trial (12,661 cases, **2,296 differ**: larmorx mistakes in nibabel's header arithmetic, mostly
  NIfTI quaternions and qform affines), the benchmark. **Not done (on purpose): the records, the
  golden checksums, the full gates**: the trial's fixes change outputs, so they come after them.
  See "Where relay 6 stopped".
- **Relay 7 (done, commits 129854e9, ab801550, 18756aff, e9817463, d47952d1, 75e4bebf,
  c5bfc440, 7a2fda55):** the trial's findings fixed (A-E: nibabel's header arithmetic,
  scipy's float64 stack, numpy's NaN, notices), the stored collocation matrix (default; within
  the memory rule), 28 permanent cases, the 0.5 h trial (335,471 cases: 335,456 agree, 14 =
  10 new SDC-11, 4 larmorx's (fixed), 1 SCI-02 notice), triage.toml. See "Where relay 7 stopped".
- **Relay 8 (done, commits f99a809b, d0bd51da, 14ff7709, 2f6d6744, 3c9c10af, 2ed5b887,
  a31e70bb, 7f47bf81):** the record (37 cases, 37 pass, 821 checks; the originals' step: 4 changed, all
  LAPACK-3.12.1), golden checksums (16 new cases, none changed), the stored matrix at one thread
  only (3c9c10af), the benchmark, the full gates. See "Where relay 8 stopped".
- **Relay 9 (done, larmorx commits a2b9395c, 61185255, e2f73918, 25120b6b; site clone commit
  1663e0ec on branch fmap-a-pages):** the evidence module `evidence/sdcflows_fieldmap.py` (+ its
  runner `sdcflows_fieldmap_run.py`), 11 cache entries computed from clean 61185255, exported
  into `<scratch>/site` (19.3 MB, 56 blobs). See "Where relay 9 stopped".
- **Relay 10 (done, larmorx commits 9f488a11, a6cc7fad, 4f0fcc85, 0da6be22, 030dc188; site
  clone commits 3d286c32, 7130949f on branch fmap-a-pages; evidence-blobs 96bca689):** the page
  `content/tools/sdcflows-fieldmap.md`, the JSON re-exported (a viewer bug fixed: the estimators'
  examples drew nothing), every site check passed. See "Where relay 10 stopped".
- **Relay 11 (done, larmorx commit eca11c3a):** HANDOFF.toml (48 operations, 17 files) and HANDOFF.md
  written against a trial merge with main ec557ff5 and the site's 69662a86 (scratch worktrees
  `r11/merge`, `r11/site-merge`): `--check` and the real apply pass there, tables, counts (432), the
  evidence summary, the site's build (0 problems), the trial merge's wheel and serial gates' pytest
  (1,106 passed); the part's final report. See "Where relay 11 stopped".
- **The part is complete** (relays 1 to 11); waiting for the orchestrator's merge.
- **(was) relay 10** = the site page and its checks (see "Next relay (10)").
- **(was) relay 9** = the site's evidence (see "Next relay (9)");
- **(was) relay 8** = records, golden checksums, full gates, benchmark report (see "Next relay (8)");
- **(was) relay 7** = the trial's findings fixed, permanent cases, the 0.5 h trial, triage.toml;
  then relay 8 = records, golden checksums, gates; relay 9 = site; relay 10 = handoff + report
  (see "Next relay (7)").

## Pins (tags under <workspace>/tags/, sdists from PyPI, SHA-256 checked against PyPI's JSON)
| Tag dir | What | SHA-256 of the sdist |
|---|---|---|
| sdcflows-2.17.1 (+ -download) | newest sdcflows (PyPI 2026-09-29), the port's source | 0365de256edd71481ab49497ee6c8c6688f4a99f4eb11819d4770e95630e5614 |
| sdcflows-2.15.1 (+ -download) | fMRIPrep 25.2.6's sdcflows (checked in the .sif) | a7776211cdcfbab018f4e16ccd96fd9bd504e205a616051815efbf6ebfaafe05 |
| scikit-learn-1.9.1 (+ -download) | newest scikit-learn (both kept envs have it) | 629cada3e33e2b9bf376cdc7614a47a4140b8aedc1d836579e359736fbd82977 |
| scikit-learn-1.6.1 (+ -download) | fMRIPrep 25.2.6's scikit-learn | b4fc2525eca2c69a59260f583c56a7557c6ccdf8deafdba6e060f94c1c59738e |
| scipy-1.15.3 (+ -download) | the shared env's SciPy (full sdist; scipy-v1.15.2 holds only ndimage) | eae3cf522bc7df64b42cad3925c876e1b0b6c35c1337c93e12c0f366f55b0eaf |
Already there: scipy-1.18.1, numpy-2.3.5, numpy-2.5.4, OpenBLAS-0.3.30, OpenBLAS-0.3.34, nitransforms-25.1.0.
upstream.tsv rows (through the handoff, format of rows 54-56):
`sdcflows-2.17.1  https://files.pythonhosted.org/packages/source/s/sdcflows/sdcflows-2.17.1.tar.gz  sha256:0365de25...5614  Apache-2.0  tags`,
the same for sdcflows-2.15.1, scikit-learn-1.9.1 (`.../s/scikit-learn/scikit_learn-1.9.1.tar.gz`, BSD-3-Clause),
scikit-learn-1.6.1, scipy-1.15.3 (`.../s/scipy/scipy-1.15.3.tar.gz`, BSD-3-Clause).

Environments (library versions on part A's paths):
| env | sdcflows | scikit-learn | SciPy | numpy (BLAS) |
|---|---|---|---|---|
| newest `oracles/python-newest` | 2.17.1 | 1.9.1 | 1.18.1 | 2.5.4 (scipy-openblas 0.3.34) |
| shared `.venv` | 2.17.1 | 1.9.1 | 1.15.3 | 2.3.5 (scipy-openblas 0.3.30) |
| fMRIPrep 25.2.6 image | 2.15.1 | 1.6.1 | 1.15.2 | 2.2.6 (conda-forge libopenblasp 0.3.30, DYNAMIC_ARCH) |
- scikit-learn is already in both kept envs (no install needed; record that in the record's environment).
- **The step 2.15.1 -> 2.17.1 on part A's paths: none in sdcflows' code.** transform.py,
  interfaces/bspline.py, interfaces/fmap.py, utils/phasemanip.py, utils/tools.py,
  workflows/fit/fieldmap.py are byte-identical; the release diff is only in fieldmaps.py, wrangler,
  interfaces/epi.py, interfaces/utils.py (UniformGrid's warning, PEPOLAR: part B),
  workflows/fit/pepolar.py (TOPUP config/head-motion coupling: part B), viz.
- Libraries on these paths, read side by side: sklearn 1.6.1 vs 1.9.1 `_solve_sparse_cg` identical,
  solver resolution the same (sparse, no intercept -> sparse_cg); SciPy 1.15.3 vs 1.18.1 `cg` same
  arithmetic (only `postprocess` removed), `make_system` same dtypes, `_get_atol_rtol` same,
  `csr_matvec`/`csc_matvec`/`csr_matvecs` identical, `_find_interval` identical, `_deBoor_D`
  differs only in the derivative recursion with repeated knots (`h[m]` -> `h[n]`, not reached:
  nu=0, distinct knots), `_evaluate_spline` adds an early return for nu > k+1 (not reached);
  OpenBLAS 0.3.30 vs 0.3.34 `sdot.c` and `sdot_microk_haswell-2.c` identical.
  Expected: no step on part A's paths; the records check it case by case (relay 5).
- **BLAS kernel matters.** The ridge solver's dot products are OpenBLAS `sdot`: Haswell/Zen kernel
  = 4 ymm x 8 lanes of FMA chains (32 partial sums), lanes reduced (hi+lo 128, 4+5, 6+7, (4+5)+(6+7),
  two hadds), tail elements `dot(double) += (float)(y*x)`, `dot += mydot`, returned as float; numpy's
  FLOAT_dot adds it to a double and casts to float. No SMP split in sdot (unlike ddot). The fMRIPrep
  image's OpenBLAS picks **Cooperlake** on the main machine (AVX-512: `sdot_microk_skylakex-2.c`,
  other bits) and Haswell on the old laptop, where the oracle runs ran (hostname in every report):
  run the image's in-process oracle with `--env OPENBLAS_CORETYPE=Haswell`. larmorx follows the
  Haswell kernel (the convention of `docs/findings/numpy-blas.md`) on every platform.
- fMRIPrep's nodes ran with OMP_NUM_THREADS=1, MKL_NUM_THREADS=1 (reports' environment).

## Inventory (sdcflows 2.17.1; every function, option, message, failure case)
### sdcflows/transform.py
- `grid_bspline_weights(target_nii, ctrl_nii, dtype='float32')` -> scipy sparse (kron of 3 per-axis
  CSRs; COO or BSR by scipy's density rule). Warning (UserWarning, stacklevel 2):
  "Image's and B-Spline's grids are not aligned." when `allclose(norm(cross(ctrl[:3,:3].T,
  target[:3,:3].T), axis=1), 0, atol=1e-3)` fails (continues anyway). Per axis: coords float32 zeros
  with arange on the axis row; `locs = apply_affine(inv(ctrl.affine) @ target.affine, coords.T)[:, axis]`
  (float64, dgemm order); knots `arange(-3, K+3, float32)`; `BSpline(knots, eye(K+2), 3)(locs)` =
  de Boor in float64 (`_find_interval`, `_deBoor_D`, extrapolate=True), columns 1:-1; kept where
  `|locs - knot| < 2.0`, cast to float32 into a lil_array (lil drops exact zeros), `.tocsr()`;
  `kron(kron(wd0, wd1), wd2)`. **Measured (probe_colmat.py, both envs):** final CSR (after
  `sparse_hstack(...).tocsr()`) has sorted indices, int32 indices, float32 data; a kron whose B has
  `2*nnz >= rows*cols` goes through BSR with B densified, so **explicit zeros** are stored
  (e.g. 20x18x7 image, 5x5x4 knots: 71,136 explicit zeros of 191,520): stored pattern per row =
  axis-0 stored entries x (axis 1: all K1 if the inner kron densified, else stored) x (axis 2: all
  K2 if the outer kron densified, else stored); value = fl32(fl32(w0*w1)*w2) (0 where densified).
- `B0FieldTransform` (attrs, slots: `coeffs`, `mapped`):
  - `fit(target_reference, xfm_data2fmap=None, approx=True) -> True`: str/Path loaded; approx only
    if xfm given AND grids not aligned (same cross test, coeffs[-1] vs target); projected reference
    `xfm @ target.affine`, `ensure_positive_cosines`; approx: `deoblique_and_zooms(coeffs[-1],
    target)` (factor 4, padding 1); colmat = hstack of every level's weights; `colmat @ coeffs`
    (float32 coefficients) reshaped; header: target's copy, intent 'estimate' name 'fieldmap Hz',
    float32, cal_max = max(|min|, max), cal_min = -cal_max; approx: nitransforms `Affine(reference=
    zeros target grid).apply(mapped)` (order 3, mode constant, nitransforms' defaults). Returns True
    always (the "cache valid" False path no longer exists).
  - `apply(moving, pe_dir, ro_time, xfms=None, jacobian=True, xfm_data2fmap=None, approx=True,
    order=3, mode='constant', cval=0.0, prefilter=True, output_dtype=None, num_threads=None,
    allow_negative=False)`: ensure_positive_cosines(moving) -> axcodes; warns "The fieldmap has been
    already fit, the user is responsible for ensuring the parameters of the EPI target are
    consistent." if mapped is set, else fit(moving, ...); squeeze dims>3 of size 1; n_volumes;
    output_dtype default the moving's on-disk dtype; pe_dir str -> list, ro_time **float** -> list
    (an int ro_time is not wrapped: zip over int -> TypeError), replicated to n_volumes; per volume
    `pe_axis = 'ijk'.index(pe[0])`, flip = (axcodes[pe_axis] in 'LPI') ^ pe.endswith('-'),
    ro = -ro if flip; voxcoords float32 index grid (nitransforms ndindex); xfms given -> ignored
    with warning "Head-motion compensating (realignment) transforms are ignored when applying the
    unwarp with SDCFlows. This feature will be enabled as soon as unit tests are implemented for its
    quality assurance." (stacklevel 1); `unwarp_parallel` (asyncio, semaphore max_concurrent =
    num_threads or min(cpu, 12)): per volume `_sdc_unwarp`: vsm = fmap_hz(float32) * ro;
    coords[pe] += vsm; `map_coordinates(volume (raw dtype), coords, output='float32', order, mode,
    cval, prefilter)`; jacobian: `*= 1 + np.gradient(vsm, axis=pe)`; stacked on the last axis;
    `allow_negative=False` -> values < 0 set to cval; header dtype output_dtype; `reorient_image(moved,
    axcodes)` (niworkflows) back to the original orientation.
  - `to_displacements(ro_time, pe_dir, itk_format=True)` = `fmap_to_disp(self.mapped, ...)`.
- `fmap_to_disp(fmap_nii, ro_time, pe_dir, itk_format=True)`: vsm float64 = data * (+-ro);
  ijk float32 (N,3) with the pe column; `apply_affine(aff without translation, ijk)` (dgemm float64);
  ITK: x,y *= -1; (X,Y,Z,1,3) float64 image with fmap's affine, intent 'vector' name 'SDC', units mm.
- `disp_to_fmap(xyz_nii, epi_nii, ro_time, pe_dir, itk_format=True)`: float32 deltas squeezed,
  (-1,3); ITK x,y *= -1; `(deltas @ inv(epi.affine)[:3,:3].T).astype(float32)` (float64 dgemm);
  vsm = pe column; `vsm / (+-ro)`; intent 'estimate' name 'Delta_B0 [Hz]', units mm, cal_max/min.
  ro_time 0 -> inf/NaN (no check).
- `_move_coeff(in_coeff, fmap_ref, transform, fmap_target=None)`: ITK linear -> RAS (nitransforms),
  `Affine(.., reference=fmap_ref)`; debug branch writes `debug_fmapref.nii.gz` in the CWD (resampled
  fmap_ref through ~xfm onto fmap_target, q/sform code 1); newaff = inv(inv(coeff.affine) @ (~xfm).matrix);
  q/sform code 1; cal_max/min; returns the image with the new affine (data unchanged).
- `unwarp_parallel`, `worker`, `_sdc_unwarp`: above.
### sdcflows/interfaces/bspline.py
- Constants: LOW_MEM_BLOCK_SIZE 1000 (unused), DEFAULT_ZOOMS_MM (40,40,20), DEFAULT_LF_ZOOMS_MM
  (100,100,40), DEFAULT_HF_ZOOMS_MM (16,16,10).
- `BSplineApprox` inputs: in_data (mandatory), in_mask, bs_spacing [HF] (list of 3-tuples),
  ridge_alpha 1e-4, recenter 'median'|'mode'|'mean'|False (default median), extrapolate True,
  zooms_min 1.0 (float or 3-tuple; 0 = never resize), debug False (unused in the interface body).
  Outputs: out_intercept (model.intercept_ = 0.0), out_field (`<in>_field.nii.gz`), out_coeff
  (`<in>_coeff000.nii.gz`, ...), out_error (`<in>_error.nii.gz`), out_extrapolated
  (`<in>_extra.nii.gz`, or out_field when the mask is everything; absent when extrapolate=False).
  Steps: grids from the ORIGINAL image (`bspline_grid`) before any resize; need_resize =
  any(zooms < zooms_min) -> info "Resampling image with resolution exceeding 'zooms_min' (AxBxC →
  DxExF)." and `resample_to_zooms(fmap, max(zooms, zooms_min))` (and the mask, order 3); data float32;
  mask = ones or `asanyarray(mask.dataobj) > 1e-4` (raw stored values, scaling ignored); recenter
  (scipy.stats.mode(...).mode squeezed / np.median / np.mean of data[mask]); data -= center;
  data[~mask] = 0 (the fit uses EVERY voxel, zeros outside the mask); colmat = hstack(weights).tocsr();
  info "Approximating B-Splines grids (and 17x17x20 [knots]) on a grid of 64x64x44 (180224) voxels,
  of which N fall within the mask." (the "and" also with one grid: cosmetic); `Ridge(alpha,
  fit_intercept=False)` fit up to 3 times on identical inputs (deterministic: the retries change
  nothing), debug log "Model fit attempt {i}: max(|coeffs|) = {e}", break when max|coef| < 1e4,
  else RuntimeError "Spline fit of input file {in_data} failed. Extreme value {e:.2e} detected in
  spline coefficients."; coefficients float32 per level with the grid's affine (and grid header);
  resize: reload original, data - center (no zeroing outside the mask), mask reloaded, new colmat;
  field = colmat[mask] @ coef inside the mask, 0 outside; header float32, cal_max = max|field|,
  cal_min = -cal_max; error = (data - field) * mask, cal_min 0, cal_max max; extrapolated:
  colmat[~mask] @ coef outside, with the field's header copy (cal values from the input header).
- Ridge path (sklearn 1.9.1, sparse X, fit_intercept=False): float32 kept (validate_data dtype
  [f64, f32]); `_preprocess_data` copies X, offsets 0, scale 1; solver auto -> **sparse_cg**;
  alpha float32 array; `_solve_sparse_cg`: X1 = aslinearoperator(X); n_features > n_samples ->
  kernel ridge (C = X X^T + a I on samples, coef = X^T sol) else C = X^T X + a I, b = X^T y;
  `scipy.sparse.linalg.cg(C, b, rtol=1e-4, maxiter=None)`: make_system (float32, x0 = zeros),
  bnrm2 = norm(b) (sqrt(sdot(b,b)) float32), atol = max(0, 1e-4 * float(bnrm2)) (double),
  bnrm2 == 0 -> returns b (!), maxiter = 10 n, loop: norm(r) < atol -> done; z = r (identity
  preconditioner returns r itself); rho = dot(r, z); p = z (first) / p *= beta; p += z; q = C p;
  alpha = rho / dot(p, q); x += alpha p; r -= alpha q (each a float32 temporary then the in-place
  op); info > 0 when exhausted (sklearn warns only if verbose: never here); info < 0 impossible.
  Matvecs: CSR row sums in ascending column order (float32, from +0); X^T: per column, ascending
  sample order (CSC scatter); `mv = X1.rmatvec(X1.matvec(x)) + alpha * x` (float32).
- `ApplyCoeffsField`: in_data, in_coeff (list), fmap2data_xfm xor data2fmap_xfm (ITK files, first of
  a list; nitransforms `Affine.from_filename(fmt='itk').matrix`, inverted for fmap2data), in_xfms
  (list of 4x4; ignored by apply with its warning), ro_time (list), pe_dir (list of i,i-,j,j-,k,k-),
  jacobian False, num_threads, approx True; outputs out_corrected `<in>_sdc`, out_field `<in>_field`
  (unwarp.mapped).
- `TransformCoefficients`: in_coeff (list), fmap_ref, transform, fmap_target (debug) -> `<coeff>_space-target`.
- `TOPUPCoeffReorient` -> `_fix_topup_fieldcoeff(in_coeff, fmap_ref, pe_dir, out_file)`: pe_dir in
  i,i-,j,j-,k,k-,x,x-,y,y-,z,z-; ref reoriented to RAS (io_orientation, as_reoriented),
  `reorient_pedir(pe, ornt)`; coefficients flipped on axis 0 and multiplied by -1 if the RAS pe axis
  is i; ValueError "Shape of coefficients file {shape} does not meet the expected shape {exp}
  (toupup factors are {factors})." unless shape == ref_shape // factors + 3*(factors > 1);
  newaff = diag(ref_ras[:3,:3] * factors) centred on the reference; q/sform code 1; cal_max/min;
  intent 'estimate' name 'B-Spline coefficients'; out `<coeff>_fixed` (default
  `coefficients.nii.gz` absolute in the CWD).
- `bspline_grid(img, control_zooms_mm=(40,40,20))`: zooms from the header (all of them: a 4D header
  breaks the broadcast), dir_cos = affine[:3,:3] / zooms, bs_affine = control_zooms * dir_cos,
  bs_zooms = voxel_sizes, bs_shape = (zooms*(shape-1) // bs_zooms + 3).astype(int), centred
  translation; zeros float32 image of the input's class.
- `_split_itk_file`, `_b0_resampler`: private, unused by any workflow (`_b0_resampler` with
  hmc_xfm raises AttributeError: `unwarp.xfm` on a slots class). Not ported (documented).
### sdcflows/interfaces/fmap.py
- `PhaseMap2rads` -> `au2rads(in_file, newpath)`: float64 data (scaling applied), (d - min) *
  (2pi / (max - min)), float32, clip [0, 2pi]; header copy, float32, xyzt units mm,
  `Nifti1Image(data, None, hdr)` (affine from the header's s/qform), `<in>_rads`. A constant phase
  map -> 0/0 = NaN everywhere (RuntimeWarning only).
- `SubtractPhases`: lengths of in_phases and in_meta differ -> ValueError "Length of input
  phase-difference maps and metadata files should match."; one input -> passed through (file and
  metadata unchanged); two -> `subtract_phases`: EchoTime popped from copies of both metadata
  (missing -> None -> `None > x` TypeError), ordered by echo time, phase2 - phase1 float32, negative
  += 2pi (float32), clip [0, 2pi]; metadata = meta2 updated by meta1, EchoTime1/EchoTime2 set;
  header of phase1 (the shorter TE) copy, float32, units mm, its affine; `<phase1>_phdiff`.
- `Phasediff2Fieldmap` -> `phdiff2fmap(in_file, delta_te(metadata))`: float32 / (2 pi dTE)
  (the double scalar rounded to float32 under NEP 50), image header, float32, `<in>_fmap`.
- `delta_te(meta)`: EchoTime1/2 absent -> EchoTimeDifference (abs; None -> ValueError
  "Phase/phase-difference fieldmaps: no echo-times information."; not a number -> ValueError
  "Could not interpret metadata <EchoTimeDifference={value}>."); else float(te or 'unknown') each ->
  ValueError "Could not interpret metadata <EchoTime(1,2)={(te1, te2)}>." (0 or missing count as
  unknown); abs(te2 - te1).
- `CheckB0Units` (units 'Hz' | 'rad/s'): Hz -> pass-through; rad/s -> **raw** `dataobj / (2 pi)`
  (scl_slope/inter ignored), saved with the input header (its on-disk dtype: an int16 map is
  re-quantised by nibabel's scaling), `<in>_Hz`.
- `DisplacementsField2Fieldmap`: transform, epi, ro_time, pe_dir (j-,j,i,i-,k,k-), demean False
  (subtract the median), itk_transform True -> `disp_to_fmap`; `<transform>_Hz`.
- `CheckRegister`: mag_files (1-2), fmap_files (1-2), rot_thresh 0.02, trans_thresh 1.0. Each
  (mag, fmap) pair and the two mags / two fmaps: `_check_gross_geometry` -> ValueError (and
  LOGGER.critical) "Images have shape mismatch: {f1} {shape1}, {f2} {shape2}" or "Images have
  orientation mismatch: {f1} {codes1}, {f2} {codes2}"; two mags with non-allclose affines ->
  FreeSurfer `mri_robust_register --auto_sens` (nipype RobustRegister), LTA -> RAS, euler angles,
  any |angle| > rot or any **signed** translation component > trans (a negative shift is never
  "too big") -> critical message (its first part lacks the f-prefix: prints "{mag_files}") and
  ValueError "Magnitude 1/2 orientation mismatch too big to ignore."; outputs: every image whose
  affine is not allclose to mag1's is rewritten into the CWD with mag1's affine (info "Copying
  affine to {basename}"), else its path. No oracle run reached the registration branch.
### sdcflows/utils/phasemanip.py: au2rads, subtract_phases, phdiff2fmap, delta_te (above).
### sdcflows/utils/tools.py (reached by part A)
- `ensure_positive_cosines` (ported already: `lx.transforms.ensure_positive_cosines`, nibabel's
  io_orientation with vnl's SVD), `reorient_pedir(pe_dir, source_ornt, target_ornt=None)`
  (doctests in the source), `deoblique_and_zooms(in_reference, oblique, factor=4, padding=1,
  factor_tol=1e-4)` (nibabel `rescale_affine`, `strided_scalar`), `resample_to_zooms(in_file,
  zooms, order=3, prefilter=True)` (nitransforms `Affine(reference=newref).apply`).
  `brain_masker` is part B.
### sdcflows/workflows/fit/fieldmap.py
- `init_fmap_wf(omp_nthreads, sloppy, debug, mode='phasediff'|other)`: unzip -> CheckRegister ->
  magnitude_wf (IntraModalMerge + init_brainextraction_wf: part B) -> bs_filter (BSplineApprox
  bs_spacing [HF], zooms_min 4.0 if sloppy, extrapolate = not debug; fmap = out_extrapolated, or
  out_field in debug); phasediff: init_phdiff_wf; else (direct): IntraModalMerge(zero_based_avg=
  False, hmc=False, to_ras=False) (part B) -> CheckB0Units(units from metadata 'Units') -> bs_filter;
  method strings 'FMB (fieldmap-based) - phase-difference map' / '... - directly measured B0 map'.
- `init_phdiff_wf`: PhaseMap2rads (MapNode) -> SubtractPhases -> PRELUDE (nipype FSL: phase_file,
  magnitude_file, mask_file) -> Phasediff2Fieldmap. In larmorx: `lx.fsl.prelude` (auto).
- `_get_file`, `_get_units`: trivial helpers.

## The paths fMRIPrep runs (oracle runs' working directories, fMRIPrep 25.2.6, ran on the old laptop)
| run | estimator(s) | part A nodes (durations) |
|---|---|---|
| ds005454-sub-16 | phasediff (wf_gre16), 1 mag? | check_register, phmap2rads, calc_phdiff (1 input), prelude, compfmap, bs_filter **57.9 s**; then boldref_fmap x2, fmap_recon (T1w, MNI res-2) |
| ds002603-sub-01 | phasediff, 1 magnitude, 128^3 2 mm | bs_filter **117.9 s**; recon in 2 BOLD runs x 4 |
| ds002422-sub-02 | 2 phasediff estimators (64x64x24 3x3x3.3; 64x64x36 3.59x3.59x3.78), TEs 4.76/9.52 ms | bs_filter 9.1 s, 15.4 s; recon x 12 |
| ds001600-sub-1 | 2 phase1/phase2 (acq-v1, v2: two phmap2rads, real subtraction), 1 phasediff (v4), all 64x64x44 3.75x3.75x4, 2 mags each; 1 PEPOLAR (TOPUP) | bs_filter ~10 s each; fix_coeff (TOPUPCoeffReorient, pe_dir y, coeff 80x80x60 "zooms" 1,1,1 RAS, ref 80x80x60 3 mm LAS) |
| ds005040-sub-002 | PEPOLAR (TOPUP) | fix_coeff; recon x 4 (boldref_fmap, fmap_recon) |
Every bs_filter: bs_spacing [(16,16,10)], recenter median, ridge_alpha 1e-4, zooms_min 1.0,
extrapolate True (fMRIPrep removes the unused outputs out_field/out_error). Every check_register
took <= 0.1 s (no registration). Not in any run: direct field maps (CheckB0Units: ds005752's is not
linked), SyN (DisplacementsField2Fieldmap, BSplineApprox recenter=False), ApplyCoeffsField
(fMRIPrep uses its own ReconstructFieldmap + ResampleSeries: part B; their kernel is
`grid_bspline_weights` + `ensure_positive_cosines`), TransformCoefficients (no workflow uses it).
Synthetic cases cover those (relay 5).

## Defect candidates (SDC-01..30; decide by rules A-D in the relay that ports each; register via handoff)
- SDC-a `grid_bspline_weights` outside the knot grid: BSpline(extrapolate=True) continues the end
  polynomial pieces, so a sample beyond the outer knots (loc < 0 or > K-1, within distance 2) gets
  a non-B-spline weight (loc -1.5 on knot 0: -3.27 instead of 0.0208). Reached when the sampling
  grid extends beyond the knots (B0FieldTransform.fit's approx reference, fMRIPrep's direct
  reconstruction on a larger target). Rule A likely (measure how often fMRIPrep meets it: D);
  fix: the true basis (extended uniform knots give identical bits inside [0, K-1]).
- SDC-b `grid_bspline_weights` on non-aligned grids: only warns, then builds a separable product that
  is wrong for rotated grids. Rule B (clear error) or A (exact non-separable weights); measure.
- SDC-c `CheckB0Units` rad/s: divides the raw stored values (ignores scl_slope/inter) and saves with
  the input's on-disk dtype (int16 re-quantised). Rule A.
  **Relay 3: dropped** (the scaling is applied; rule C for the re-quantisation; see "Where relay 3
  stopped").
- SDC-d `CheckRegister` translation test uses signed components (`vec > trans_thresh`): a large
  negative shift passes. Rule A. Its critical message prints "{mag_files}" literally (cosmetic, A).
- SDC-e `subtract_phases` missing EchoTime -> TypeError; `au2rads` constant map -> NaN map; rule B
  (clear errors). `B0FieldTransform.apply` int ro_time -> TypeError (B or accept ints: A).
- SDC-f `bspline_grid` zooms from the header (a 4D header breaks it; header zooms vs affine).
  Likely C (keep, document) / B. **Correction (relay 2):** the mask is NOT from raw values:
  `np.asanyarray(masknii.dataobj)` goes through nibabel's `ArrayProxy.__array__`, which APPLIES
  scl_slope/inter (probed: int16 with slope 1e-5 -> float64 1e-5, below 1e-4: out of the mask).
  The threshold's type follows the scaled array's dtype (float32 data: `x > fl32(1e-4)`; float64
  or ints: in float64). Re-check SDC-c (`CheckB0Units`' `dataobj / (2 pi)`) the same way: an
  ArrayProxy in a ufunc is converted with its scaling, so SDC-c may not exist as written.
  **Relay 3: settled**: 4D-singleton header -> rule A (first three zooms, `reproduce` = the
  ValueError); header zooms vs affine -> rule C (measured equal). See "Where relay 3 stopped".
- SDC-g (relay 3) `TOPUPCoeffReorient` negates only for `i`, never for FSL's `x` (fMRIPrep passes
  `y`-style letters): rule D (measure on LR/RL PEPOLAR data first); ported as sdcflows does.
  **Relay 4: measured (synthetic, no LR PEPOLAR data in the cache) -> SDC-07, rule A: the
  negation inverts the field; switched.** IDs of every candidate: "Where relay 4 stopped".
- `cg` returns b itself when norm(b) == 0 (all-zero field): coefficients = b = 0: harmless.
- Not defects: the 3 identical retries (larmorx fits once, same outcome and message); "and" with a
  single grid in the log message (cosmetic, keep the text).

## Design
- **Crate** `crates/larmorx-sdcflows` (Apache-2.0, sdcflows' notice + BSD notices of SciPy,
  scikit-learn, OpenBLAS where their arithmetic is followed; PROVENANCE.md). The later clean-room
  original goes elsewhere (e.g. `larmorx-fieldmap` / `larmorx-mri`), never in this crate.
  Modules: `bspline` (grid, per-axis weights, the collocation operator: matvec, rmatvec, masked
  rows, materialised CSR), `blas` (OpenBLAS sdot Haswell, numpy FLOAT_dot), `ridge` (SciPy cg +
  scikit-learn sparse_cg incl. the kernel branch), `approx` (BSplineApprox), `phase`
  (phasemanip), `fmap` (CheckB0Units, disp<->fmap, CheckRegister geometry), `transform`
  (B0FieldTransform fit/apply, unwarp), `topup` (TOPUPCoeffReorient), `coeff` (TransformCoefficients).
  Depends on larmorx-core, -transform (openblas 4x4, apply_affine rows, nitransforms mapping),
  -interp (ndimage map_coordinates), rayon, thiserror.
- **No materialised matrix in the fit** (memory): the collocation matrix is applied on the fly from
  the per-axis weights, with exactly scipy's stored pattern (explicit zeros of densified axes
  included, so 0*inf = NaN stays) and summation order (rows: ascending column; X^T: ascending
  sample). The original holds X (8 bytes/nnz: 11 M nnz for 64x64x44, ~134 M for 128^3) at least 3
  times (validate_data/_preprocess_data copies, the adjoint's conj copy); larmorx holds O(voxels +
  knots). `to_csr()` materialises it only when a caller asks for the matrix.
- **Python family `lx.sdcflows`** (`python/larmorx/sdcflows/`), names after sdcflows':
  `bspline_grid`, `grid_bspline_weights` (-> `CsrMatrix` dataclass: data/indices/indptr/shape,
  `.to_scipy()` if SciPy is installed; numpy stays the only dependency), `bspline_approx`
  (BSplineApprox; dataclass result with field, coefficients, error, extrapolated, intercept),
  `B0FieldTransform` (fit/apply/to_displacements, dataclass state), `apply_coeffs_field`,
  `transform_coefficients`, `topup_coeff_reorient`, `fmap_to_disp`, `disp_to_fmap`,
  `displacements_field_to_fieldmap`, `au2rads`, `subtract_phases`, `phdiff2fmap`, `delta_te`,
  `check_b0_units`, `check_register`, `reorient_pedir`, `deoblique_and_zooms`,
  `resample_to_zooms`, `phdiff_workflow` (init_phdiff_wf with `lx.fsl.prelude`); `fmap_workflow`
  (init_fmap_wf) needs part B's magnitude preparation: part A takes the magnitude reference and mask
  as inputs, part B completes it. In-memory images and paths, like `lx.transforms`; file names as
  sdcflows' (`_field`, `_coeff000`, `_rads`, ...) where a function writes files (`out_dir=`).
- **Command line**: none for the interfaces, as the resampler's (nipype interfaces have no program
  and no original arguments). sdcflows' own BIDS-App command (`sdcflows bids_dir out participant`)
  runs whole workflows with part B's masks and wrangling: proposed for part B or later (question
  for the orchestrator).
- **Threads**: `n_threads` on every call (0 = all); deterministic: the matvecs split rows / columns
  in fixed chunks, each output a sequential sum; sdot is sequential by definition (its 32 lanes);
  B0FieldTransform.apply's per-volume work in parallel (fixed volumes per thread), num_threads maps
  to n_threads.
- **Memory**: the memory rule's case in tests/parity/test_memory_peak.py: bspline_approx on a
  64x64x44 field vs sdcflows' BSplineApprox (expected far below).
- **Exactness plan**: weights: de Boor f64 without contraction (`_find_interval`, `_deBoor_D`),
  locs through `openblas::rows3`-style dgemm order, inv4/matmul4 for the 4x4s, f32 casts; kron
  pattern/value rule above; CG in f32 with the Haswell sdot replica (unit-tested against numpy in
  both envs), double atol, numpy NEP-50 scalar rules; median/mean/mode as numpy/scipy compute them
  (median: partition + fl32(fl32(a+b)/2); mean: numpy pairwise float32 sum); nitransforms apply
  through the existing chain mapping + `lx.ndimage.map_coordinates`.
- **Records** (relay 5): `validation/.../parity/sdcflows_fieldmap.py`, in-process sdcflows as the
  oracle (newest default, shared for the step; fMRIPrep's image for the oracle runs' real calls with
  OPENBLAS_CORETYPE=Haswell), PRELUDE through the oracle cache (`use`).

## Relay plan (one sub-task per relay; commit; update this file; stop)
1. (done, relay 1) Pins, inventory, design; crate, `bspline_grid`, per-axis weights, collocation
   operator + CSR, Rust unit tests; Python binding + wrappers (CsrMatrix), bit-checks vs sdcflows
   in both envs (orientations, odd grids, BSR/COO patterns, oracle runs' field maps).
2. `blas::sdot` + numpy FLOAT_dot (bit-checked vs numpy); `ridge` (cg, sparse_cg both branches)
   bit-checked vs sklearn on the oracle runs' bs_filter inputs.
3. `bspline_approx` complete (recenter modes, masks, zooms_min resize -> needs nitransforms' image
   apply port + nibabel rescale_affine: `resample_to_zooms`), outputs/headers/messages; check vs
   BSplineApprox on ds005454, ds002603, ds002422 x2, ds001600 x3.
4. (done, relay 4, with relay 5's phasemanip/fmap interfaces/phdiff workflow and SDC-07's
   measurement; relay 5 now = the remaining switches, API page, findings: see "Next relay (5)")
   B0FieldTransform fit/apply (deoblique_and_zooms, unwarp, jacobian, reorient_image),
   ApplyCoeffsField, fmap_to_disp/disp_to_fmap/DisplacementsField2Fieldmap, TransformCoefficients,
   TOPUPCoeffReorient (+ reorient_pedir) vs ds001600/ds005040's fix_coeff.
5. phasemanip + fmap interfaces + phdiff workflow (lx.fsl.prelude); D13 decisions and switches;
   `.pyi`, API page `docs/api/sdcflows-fieldmap.md`, PROVENANCE, SPDX, findings doc.
6. Parity suite + records (both envs, the step, fMRIPrep's real calls), golden cases, memory case,
   difftest descriptor + trial (boxed), benchmark; gates; records from a clean commit.
7. Site page (`fmap-a-pages`), 8. handoff + final report.
Relays 2-3 or 4-5 may need splitting; say so here.

## Where relay 1 stopped (2026-11-08 ~01:30)
Commits on the worktree's branch (from main 2b453c1): `71991275` (crate larmorx-sdcflows: bspline,
npy), `eefadced` (lx.sdcflows wrappers + _core bindings/stubs + tests/python/test_sdcflows.py +
validation/investigations/sdcflows-fieldmap/{README.md, check_weights.py, probe_colmat.py,
oracle_nodes.py} + crate PROVENANCE.md). Working tree clean.

Done and verified:
- Rust `larmorx_sdcflows::bspline`: `bspline_grid`, `AxisWeights::from_locations` (SciPy's
  `_find_interval` + `_deBoor_D` for the uniform knots), `grid_bspline_weights` (-> `LevelWeights`
  with the per-axis weights, the dense-axis flags of scipy's kron, the alignment flag),
  `Collocation` (levels side by side): `matvec_f32(v, rows, n_threads)`, `rmatvec_f32(y, n_threads)`
  (scipy's summation orders, no stored matrix), `to_csr`. `npy`: floor_divide (npy_divmod),
  allclose_zero, cross, norm3, grids_aligned. 7 Rust unit tests pass; fmt/clippy clean on the two
  crates.
- Python `lx.sdcflows`: `bspline_grid(img, control_zooms_mm)` (-> lx.Image zeros float32),
  `grid_bspline_weights(target, ctrl, dtype)`, `collocation_matrix(target, levels, dtype)` (->
  `CsrMatrix`: data/indices/indptr/shape, `to_scipy()`, `toarray()`; int32 indices while they fit;
  UserWarning NOT_ALIGNED per misaligned level), constants. `_core`: `sdcflows_bspline_grid`,
  `sdcflows_collocation_csr`, `sdcflows_collocation_apply` (X @ v / X.T @ v / X[rows] @ v float32).
  Family registered in `python/larmorx/__init__.py` (lazy `_SUBMODULES`, `__all__`).
- **Bit-identity**: `check_weights.py`: 15 cases, every CSR array byte-identical and `X @ v`,
  `X.T @ y` identical to scipy's `aslinearoperator(X)`, in newest (SciPy 1.18.1 / numpy 2.5.4) AND
  shared (SciPy 1.15.3 / numpy 2.3.5): orientations (LAS, permuted PSL, oblique-aligned), two
  levels, single slice, single row, dense axes with explicit zeros, float64, a rotated target (the
  warning matches), a target beyond the knots (extrapolated weights: SDC-a reproduced as the
  original does), the field maps of ds005454, ds002422 x2, ds001600 (up to 53 M entries).

Not done yet (relay 2's sub-task, in this order):
1. `blas` module: OpenBLAS 0.3.34 `sdot` Haswell kernel + numpy `FLOAT_dot` (double accumulate,
   float cast) exactly as written in "Pins" above (kernel: 4 x 8 lanes of `vfmadd231ps` over
   `n & -32` elements; reduce: per acc hi128+lo128, (4+5), (6+7), sum, hadd, hadd; tail
   `double dot += (float)(y[i]*x[i])`; `dot += mydot`; return (float)dot; numpy: `double sum = 0;
   sum += sdot; (float)sum`). Bit-check against `np.dot` of float32 vectors of many lengths
   (0, 1, 31, 32, 33, 63, 64, 1000, 10^5, 10^6) in both envs with OPENBLAS_CORETYPE=Haswell.
2. `ridge` module: SciPy `cg` (as read: rtol 1e-4, atol 0 -> max(0, rtol*float(norm(b))) double,
   maxiter 10n, x0 zeros, identity preconditioner returning r itself, the float32 temporaries,
   bnrm2 == 0 returns b) + scikit-learn `_solve_sparse_cg` both branches (n_features > n_samples:
   kernel ridge on samples; else normal equations), on `Collocation` (matvec/rmatvec), float32,
   alpha float32(alpha). Return coef + iteration count + info. Bit-check: `Ridge(alpha=1e-4,
   fit_intercept=False).fit(colmat, y).coef_` on the four field maps (data recentred and zeroed
   outside the masks the oracle runs give: `.../magnitude_wf/brainextraction_wf/masker/
   clipped_mask.nii.gz`), plus a tiny kernel-branch case, in both envs.
3. Then relay 3 = `bspline_approx` (see the relay plan).

Sandbox notes for the next relay (they cost time here):
- The sandbox refuses `source`, `git -C <other repo>`, `cd` to the workspace root, variables in
  loops with find/tar/sed, heredocs piped into python. Write scripts to `<scratch>/scripts/` with
  the Write tool and run them with `bash <script>`; run boxed jobs as
  `scripts/orchestration/boxed.sh <GiB> bash <scratch>/scripts/<x>.sh ...` from the worktree.
- `python -I` ignores PYTHONPATH: use `python -P`.
- Helpers in `<scratch>/scripts/`: `build_wheel.sh` (maturin release build, CARGO_BUILD_JOBS=14,
  copies `_core.abi3.so` into the worktree's python/larmorx; ~1 min incremental; run boxed 16 GiB,
  log `oracles/logs/fmap-a-1-build.log`), `run_check.sh <script.py>` (runs it in newest then
  shared with OPENBLAS_CORETYPE=Haswell and the worktree's python/ on PYTHONPATH),
  `pytest_wt.sh <tests...>` (ruff format + pytest with the worktree's larmorx), `fndiff.py <old>
  <new> <relpath> [func ...]` (diff of named functions between two source trees).
- The golden coverage test (`tests/golden/registry.py` NAMESPACES) does not list `lx.sdcflows`
  yet: add it with the cases in relay 6 (CsrMatrix.to_scipy/toarray are public methods too).
- Nothing yet for the handoff beyond what NOTES lists (upstream.tsv rows, the register family
  SDC-, NOTICE entries for sdcflows/scikit-learn): HANDOFF.toml/md are written in relay 8.

## Where relay 2 stopped (2026-11-08 ~02:00)
Commits (worktree branch): a0cdaaca (blas, ridge, bindings, lx.sdcflows.ridge_sparse_cg),
bf21e596 (numpy's one-element dot; check_ridge.py), bc7f0d1d (approx core,
`_core.sdcflows_bspline_approx`, check_approx.py, ridge tests, PROVENANCE rows). Working tree
clean. Helper scripts added in `<scratch>/scripts/`: `run_both_checks.sh` (build + both checks
in both envs); the probes `nep50.py`, `sk_msgs.py`, `probe_np.py` are committed (d38c12bc) as
`validation/investigations/sdcflows-fieldmap/probe_nep50.py`, `probe_ridge_messages.py`,
`probe_mask_scaling.py`.

Done and verified (both kept envs, OPENBLAS_CORETYPE=Haswell, LARMORX_WORKSPACE set; logs
`oracles/logs/fmap-a-2-check-ridge.log`, `fmap-a-2-checks.log`):
- `check_ridge.py` 88 cases ALL OK: 80 np.dot/norm vectors (lengths 0..10^6, signed zeros,
  subnormals, inf, NaN; a one-element vector is numpy's plain float product, `cblasfuncs.c`);
  SciPy cg on 8 systems (maxiter 0/1/5, atol, rtol 0, zero b); sklearn Ridge on 10 sparse
  matrices (normal, kernel with max_iter ignored, unsorted/duplicate indices, CSC input, float64
  y, alpha 0) + 13 messages; B-spline fits of 5 synthetic + the 7 oracle bs_filter field maps
  at 1 and 14 threads, identical to sklearn AND to fMRIPrep 25.2.6's own coeff000 files (old
  laptop, sklearn 1.6.1 / SciPy 1.15.2 / numpy 2.2.6: the step to the newest is nil on this
  path). Speed of the fit (weights + ridge): sklearn 1.9-28 s, larmorx 0.3-3.8 s at 14 threads
  (ds002603: 2,097,152 x 9,072, nnz 126 M: 28.1 s vs 3.8 s).
- `check_approx.py`: sdcflows' BSplineApprox interface vs `_core.sdcflows_bspline_approx` on 16
  synthetic cases (median/mean/mode/none, no mask, full mask, scaled int16 mask, two levels,
  alpha, no extrapolation, kernel-system grid, single slice, odd LAS, resize with zooms_min 4
  and (2,2,3) without a mask, a failed fit's RuntimeError) + the 7 oracle calls: coefficients,
  grids, field, error, extrapolated (stored arrays), intercept, every INFO/DEBUG log record:
  ALL OK, 23 cases in each env (the 7 oracle calls incl. ds002603's 2 M voxels).
- Formats: sdcflows' f-string of a numpy float32 goes through Python's float repr
  ("max(|coeffs|) = 1319.5245361328125"); `npy::py_repr` = CPython's shortest repr with its
  tie rule (Rust's `{:?}` differs on ties: 791.0652465820313 vs Python's ...312).
- numpy details ported in `approx`: median = mean of the middle one/two values (NaN if any NaN);
  mean = numpy pairwise float32 sum (8 lanes, blocks <= 128, +0 initial) / count as **float64**
  (rcount is np.intp, so `ret / rcount` is float64, then cast to float32); SciPy mode = smallest
  of the most frequent, NaNs one value after the numbers (1.15.3 np.unique and 1.18.1
  unique_counts agree). Edge noted, not decided: the sign of a zero mode depends on numpy's
  unstable sort (only zero signs of the error map can differ).
- No "least-squares option" exists in sdcflows 2.17.1 (or 2.15.1): BSplineApprox always uses
  `Ridge(alpha, fit_intercept=False)`; with sparse X that is always sparse_cg (alpha = 0 is
  plain least squares through the same CG, covered by `tall-alpha0`). No other solver reachable.
- Gates run on the two crates only: fmt, clippy -D warnings, cargo test (larmorx-sdcflows 14
  tests); ruff check/format on the new Python; tests/python/test_sdcflows.py 5 pass. The full
  gates are relay 6's.

Next relay (3), in this order:
1. Python `lx.sdcflows.bspline_approx(in_data, in_mask=None, bs_spacing=[HF], ridge_alpha=1e-4,
   recenter="median", extrapolate=True, zooms_min=1.0, out_dir=None, n_threads=1)` returning a
   dataclass (out_field, out_coeff list, out_error, out_extrapolated, out_intercept + arrays),
   on top of `_core.sdcflows_bspline_approx` (see check_approx.py's `run_larmorx`: it is the
   wrapper's logic, verified): load as nibabel does (`get_fdata(dtype=float32)`: check
   larmorx's NIfTI reader gives the same float32 values for scaled ints, else port nibabel's
   scaling order), mask `np.asanyarray(dataobj) > 1e-4` WITH scaling (dtype rule above),
   header zooms; the resize message (format in run_larmorx: `str()` of the header zooms and of
   `np.maximum(zooms, zooms_min)` elements) and `resample_to_zooms` (sdcflows/utils/tools.py:
   rescale_affine(nibabel), new_shape = rint(shape * pre_zooms / zooms), sform/qform codes kept,
   `Affine(reference=newref).apply(in_file, order=3, prefilter=True)`; lx.transforms has the
   nitransforms mapping + lx.ndimage.map_coordinates; check against sdcflows' function on the
   two resize cases of check_approx and a mask); files: `<in>_field.nii.gz` (header copy,
   float32, cal_max = max|field|, cal_min = -cal_max), `<in>_coeff000.nii.gz`... (the grid's
   image: zeros float32 with its affine; header = nibabel's new header for it), `<in>_error`
   (the INPUT header as is: its on-disk dtype! cal_min 0, cal_max = np.max(error) incl. NaN),
   `<in>_extra` (the field's header; or out_field itself when the mask is everything);
   log records through `logging.getLogger("nipype.interface")`? (check docs/architecture.md's
   logging convention; keep sdcflows' texts); RuntimeError text from the core.
   Compare files byte-for-byte (gzip aside: decompressed) with sdcflows' outputs.
2. Then relay 4 as planned (B0FieldTransform, ...).
3. Leftovers for relay 6: memory case (Collocation holds O(voxels + knots); sklearn holds X 3x),
   golden cases for `ridge_sparse_cg` and `RidgeResult` (and add `lx.sdcflows` to NAMESPACES),
   the API page rows (ridge_sparse_cg: sklearn options alpha/tol/max_iter, fit_intercept=False
   only, float32 only).

## Where relay 3 stopped (2026-11-08 ~02:40)
Commits (worktree branch): 09fb3f66 (the code and the checks), ccc347a5 (PROVENANCE rows,
investigation README). Working tree clean. Build: `_core.abi3.so` in the worktree is current.
Logs: `oracles/logs/fmap-a-3-checks.log` (both checks, both envs), `fmap-a-3-pytest.log`.
Helpers added in `<scratch>/scripts/`: `run_check_quiet.sh <script.py>` (both envs, result lines
only: nipype echoes its log records on stderr), `run_r3_checks.sh` (relay 3's two big checks;
run boxed: `scripts/orchestration/boxed.sh 24 bash <scratch>/scripts/run_r3_checks.sh`, ~6 min).
`check_b0field.py` and `check_coeffs.py` take ~1 min each per env.

Done and verified (both kept envs, OPENBLAS_CORETYPE=Haswell):
- **Rust `tools`** (larmorx-sdcflows): `rescale_affine` (nibabel; `rows3_single` for its two
  one-point products), `voxel_sizes`, `resample_to_zooms_grid`, `affine_coordinates` /
  `identity_coordinates` (nitransforms' `Affine(matrix, reference).apply` coordinates:
  ndcoords, f32, map, f32, inv(moving)), `apply_affine` (nibabel: rows3 for >= 2 points,
  rows3_single for one), `IDENTITY`. `approx::bspline_approx_logged` (the log also on failure;
  the binding sets `err.log` on the exception). Bindings: `sdcflows_rescale_affine`,
  `sdcflows_resample_to_zooms_grid`, `sdcflows_affine_coordinates`, `sdcflows_apply_affine`,
  and in nifti.rs `nifti_set_xform(header, "sform"|"qform", affine, code)` (nibabel's
  set_sform/set_qform with codes). Stubs in `_core.pyi`.
- **Python** (`python/larmorx/sdcflows/`): `approx.py` (`bspline_approx` ->
  `BSplineApproxResult`: images + `out_*` paths with `out_dir`; nipype's split_filename /
  fname_presuffix; logger `larmorx.sdcflows`; `name=` for in-memory inputs; `debug` accepted and
  ignored like sdcflows), `tools.py` (`resample_to_zooms`, `rescale_affine`; 3D and 4D-singleton
  only, documented), `coeffs.py` (`topup_coeff_reorient`, `transform_coefficients` incl. the
  debug `debug_fmapref.nii.gz` in out_dir or the CWD like sdcflows, `reorient_pedir`,
  `CoefficientsResult`), `transform.py` (`B0FieldTransform` dataclass with `fit` only,
  `deoblique_and_zooms`). Exported in `__init__`.
- **Checks** (committed, `validation/investigations/sdcflows-fieldmap/`):
  `check_bspline_approx.py` 51 cases per env (300 random rescale_affine; 15 resample_to_zooms;
  36 approximations incl. every option and failure; the 7 oracle bs_filter calls; the oracle
  runs' own `*_coeff000`/`*_extra` files byte-identical too), `check_coeffs.py`
  (reorient_pedir 1,728 combos; topup 44 cases + ds001600/ds005040 fix_coeff and their own
  `*_fixed` files; transform_coefficients text/mat/debug/two-transforms error),
  `check_b0field.py` (deoblique 12 cases; fit 12 cases incl. ds005454 coeff000 and ds005040
  TOPUP coefficients on their BOLD refs, approx and not, warnings). ALL OK in both envs.
- Gates run on what changed: fmt, clippy -D warnings (larmorx-sdcflows, larmorx-py), cargo test
  (16), ruff check/format, check_spdx, `pytest tests/python tests/golden` (pass).
- Details learnt: nibabel's in-memory header has scl NaN and vox_offset 0 (set when written):
  compare written files or skip those fields; numpy's 16-byte `intent_name` truncates
  "B-Spline coefficients" to "B-Spline coeffic" (reproduced); `deoblique_and_zooms` returns
  float64 zeros (strided_scalar) with the reference's header dtype; after `fit(approx)` the
  `mapped` header is the target's (float32, no intent, no cal: nitransforms' apply);
  `str.replace('_field.', ...)` is applied to the file name only (sdcflows: the whole path;
  differs only when a directory name holds "_field.": an API choice, no defect).
- **Gap left (documented in code):** `bspline_approx`'s `out_error` for an integer-typed input
  header is written as float32; sdcflows (nibabel) scales it into the input's integer type.
  Needs nibabel's float->int writer (below).

Defect candidates re-checked (SDC-c, SDC-f, and two new):
- **SDC-c dropped as a defect** (CheckB0Units rad/s): `np.asanyarray(img.dataobj)` applies the
  scaling (relay 2's probe), so the division is on Hz-correct values; the output keeps the
  input header's stored type and nibabel's writer re-scales into it (int16: ~range/65535
  steps, ~0.015 Hz for +-500 Hz). Rule C (nibabel's documented writer behaviour; loss
  negligible): keep. Needs the scaled writer port to be exact (relay 4/5).
- **SDC-f split and settled** (`bspline_grid` takes every header zoom): (1) a 4D header with one
  volume fails with numpy's broadcast ValueError; fMRIPrep's own `desc-preproc_fieldmap`
  outputs are such images (9 of 9 in the oracle runs, `probe_header_zooms.py`), so the input
  makes sense and the result is wrong: **rule A** (fix: the first three zooms; `reproduce`:
  sdcflows' ValueError; applies to `bspline_grid`, `bspline_approx`, `B0FieldTransform`).
  Register it as an SDC ID in relay 5 with the switches. (2) header zooms vs the affine's voxel
  sizes: measured equal on all 34 field-map images of the oracle runs (largest relative
  difference 7e-8): **rule C** (keep, document).
- **New, SDC-g candidate (rule D, measure first):** `TOPUPCoeffReorient` negates the
  coefficients only when the reoriented direction starts with `i`; fMRIPrep's PEPOLAR passes
  FSL letters (`y` in both oracle runs: `to_las`'s `pe_dir_fsl`), so an `x` (left-right) phase
  encoding is never negated although the comment says "x/i". Whether negation is right for x
  needs a measurement on an LR/RL PEPOLAR dataset (corrected blips' agreement with and
  without; part B or the final phase). Ported as sdcflows does.
- Not defects: the intent-name truncation (numpy's field width; kept, rule C, cosmetic).

Next relay (4), in this order:
1. **nibabel's scaled float->int writer** (`nibabel/arraywriters.py` SlopeInterArrayWriter:
   calc_scale, _range_scale, finite_range, nan2zero; `volumeutils.array_to_file`,
   `_write_data`, `best_write_scale_ftype`, `int_scinter_ftype`; nibabel 5.4.2, MIT) in Python
   with numpy (exact by construction), e.g. `python/larmorx/sdcflows/_nibabel_write.py` or in
   `larmorx.io` (ask: `lx.save` deliberately stores the array's type; a `nibabel_scaling=True`
   option keeps that default). Then: `bspline_approx`'s `out_error` for integer headers,
   `B0FieldTransform.apply`'s output in the moving image's stored type (int16 BOLD).
2. `B0FieldTransform.apply` (ensure_positive_cosines on the data, the "already fit" warning,
   `reshape_dataobj` squeeze, pe/ro lists (an int ro_time -> TypeError, SDC-e), per-volume
   `_sdc_unwarp`: vsm = fmap(f32) * ro; coords[pe] += vsm; map_coordinates(volume raw dtype,
   output float32, order, mode, cval, prefilter); jacobian `*= 1 + np.gradient(vsm, axis=pe)`
   (check larmorx_transform::resample_series::jacobian's float32 rule); `allow_negative`;
   output_dtype; niworkflows' `reorient_image` back to the original axcodes), the xfms warning,
   `to_displacements`; `ApplyCoeffsField` (data2fmap_xfm / fmap2data_xfm through
   `Affine.from_filename(fmt='itk')` (`lx.transforms.load_itk_linear`) and `~` (inv4), the
   `_sdc` and `_field` files); `fmap_to_disp`, `disp_to_fmap`, `DisplacementsField2Fieldmap`.
   Check against sdcflows on synthetic and the oracle runs' BOLD references (ds005454 coeff000,
   ds005040 TOPUP `*_fixed`), every option, 3D and 4D, int16 and float32 BOLD.
3. Then relay 5 as planned (phasemanip, fmap interfaces, the phdiff workflow, D13 switches for
   SDC-a, b, d, e, f(1), and the register entries; API page, findings doc), relay 6 (suite,
   records, golden incl. `lx.sdcflows` in NAMESPACES, memory case, difftest, benchmark).

## Where relay 4 stopped (2026-11-08 ~03:25)
Commits (worktree branch): 0f91e274 (writer + approx's error map), 1c48bc82 (labelled "lint" but
it also holds the unwarp binding, the crate's `larmorx-interp` dependency and `pub mod unwarp`),
3c536a51 (apply, displacements, ApplyCoeffsField, SDC-05/08), f0347e61 (SDC-07 measured and
switched; check_coeffs with the switch), e323b7f9 + 38965c9a (probe_sdc07_topup.py), af14b02b
(phase.py, SDC-04/09, check_phase.py, tests, PROVENANCE, README). Working tree clean; the
worktree's `_core.abi3.so` is current (built 03:18 with every relay-4 Rust change). Logs:
`oracles/logs/fmap-a-4-{approx,apply,coeffs,phase,sdc07,build}.log`. Helpers added in
`<scratch>/scripts/`: `rust_checks.sh`, `gates_r4.sh` (relay 4's gates; boxed 24 GiB, ~3 min),
`dbg_w*.py`, `probe_sdcg*.py`, `probe_prelude_hdr.py` (scratch probes; the ones behind findings
are committed: `probe_sdc07_topup.py`).

Done and verified (both kept environments, OPENBLAS_CORETYPE=Haswell):
- **nibabel's scaled writer** `python/larmorx/sdcflows/_nibabel_write.py` (`save_nibabel`,
  `nibabel_stored`, `WriterError`/`ScalingError`/`HeaderDataError`; exported as
  `lx.sdcflows.save_nibabel`): a Python port of nibabel 5.4.2's `SlopeInterArrayWriter` +
  `array_to_file` (numpy's own arithmetic) with **x87 extended precision emulated exactly**
  (`_Extended`: Fraction arithmetic rounded to 64-bit significands, signed zeros, one rounding to
  float32) for `_range_scale`'s slope/intercept (nibabel's `best_float()` is longdouble on x86-64
  Linux, float64 elsewhere: the emulation gives Linux's bits on every platform). Reused: the
  Rust writer (`_core.nifti_header_for_image` + `_core.nifti_write`, scl fields set in the dict);
  `lx.save` untouched (it stores the array's type by design). Possible later move into
  `larmorx.io` (`save(..., scaling="nibabel")`): a handoff note. `check_nibabel_write.py`:
  60,000 extended ops vs np.longdouble + 782 writes, ALL OK. `bspline_approx`'s `out_error` now
  written in the input's stored type (the gap relay 3 left): 3 new integer-input cases in
  check_bspline_approx (54 per env, ALL OK).
- **`B0FieldTransform.apply`** (transform.py) + Rust `larmorx_sdcflows::unwarp::unwarp_volume`
  (binding `_core.sdcflows_unwarp_volume`, stub in `_core.pyi`): per volume, numpy's NEP-50 types
  (Python-float readout: float32 shift/coords/gradient/Jacobian; numpy float64 readout: float64,
  rounded into the float32 coordinate and output), `larmorx_interp::ndimage::map_coordinates`
  (the resampler's SciPy replica, reused as is: same code path as fMRIPrep's resampler; the
  Jacobian is re-expressed here because larmorx-transform's `jacobian` is private and float32
  only), volumes one after the other (memory: one volume's coordinates and prefilter at a time;
  sdcflows holds up to 12). Python: ensure_positive_cosines (lx.transforms), "already fit" and
  head-motion warnings, squeeze, pe/ro lists and their errors, 4D output always (a 3D input gives
  (X,Y,Z,1), as sdcflows), `allow_negative`, `output_dtype`, header chain as nibabel's
  constructors (`_rebuilt_header`), niworkflows' `reorient_image` (`_reoriented`,
  `_axcodes2ornt`). `num_threads` accepted (no effect), `n_threads` threads.
- **`apply_coeffs_field`** (ApplyCoeffsField: data2fmap/fmap2data ITK files via
  `load_itk_linear` + nitransforms' last-row normalisation + `inv`, `_sdc`/`_field` files),
  **`fmap.py`**: `fmap_to_disp`, `disp_to_fmap`, `displacements_field_to_fieldmap` (demean keeps
  the pre-demean cal_max, as sdcflows).
- `check_apply.py`: 74 cases per env ALL OK (synthetic every option/failure; real: ds001600
  acq-v1 (phase1/phase2 coeffs) and acq-PA (TOPUP), ds005040 scaled int16 63 vols + its
  coregistration both directions, ds005454 first 10 vols with fmap2data). Note: SciPy 1.15.3's
  SCI-03 did not show on these 'reflect'/'nearest' cases (long axes).
- **Phase-difference preparation** `python/larmorx/sdcflows/phase.py`: `au2rads`,
  `subtract_phases` (1 input passed through, 2 subtracted, metadata merge), `delta_te` (every
  message), `phdiff2fmap` (delta_te or metadata), `check_b0_units` (rad/s re-quantised into the
  input's type through save_nibabel: SDC-c's rule C kept), `check_register` (geometry checks with
  sdcflows' messages, `_conform_img` rewrites; **the two-magnitudes registration branch raises
  NotImplementedError**: it needs FreeSurfer's mri_robust_register, no oracle run reaches it),
  `phdiff_workflow` (init_phdiff_wf with `lx.fsl.prelude`; PRELUDE's output written as
  `<phase>_unwrapped` with `Image.save`). `check_phase.py`: 43 synthetic + 21 fMRIPrep files
  (phmap2rads x7, calc_phdiff x7, compfmap x7 from fMRIPrep's own PRELUDE outputs) ALL OK in both
  envs; the whole workflow with lx.fsl.prelude (clean-room) gives fMRIPrep's field in every voxel
  of the 7 estimators; only `descrip` differs (the PRELUDE implementation's own text).
- Gates on what changed (gates_r4.sh): cargo fmt; clippy -D warnings (larmorx-sdcflows,
  larmorx-py, larmorx-core); cargo test (core 136, sdcflows 19); ruff check/format (repo);
  check_spdx; pytest tests/python tests/golden (675 passed, 13 skipped). Cargo.lock refreshed
  (larmorx-sdcflows -> larmorx-interp); crates-gpl/crates-nc locks not affected (they do not
  build larmorx-sdcflows: relay 6 confirms with gates-rust.sh all).

Defect IDs (assigned now; relay 5 registers them through the handoff, keep these numbers):
| ID | Candidate | Rule | State |
|---|---|---|---|
| SDC-01 | SDC-a weights beyond the knot grid (extrapolated polynomial) | D (measure how often fMRIPrep meets it) then A | not switched yet (relay 5) |
| SDC-02 | SDC-b non-aligned grids: separable product, only a warning | B or A, measure | not switched yet (relay 5) |
| SDC-03 | SDC-d CheckRegister signed translation test (and "{mag_files}" literal) | A | in the FreeSurfer branch (NotImplementedError now): switch when it is ported |
| SDC-04 | au2rads constant phase map -> NaN map | B | **switched** (fix: ValueError; reproduce: NaN + warning) |
| SDC-05 | apply int / float32 readout time -> TypeError | A (crash on sensible input) | **switched** (fix: works + warning; reproduce: sdcflows' TypeError texts) |
| SDC-06 | SDC-f(1) 4D one-volume header zooms -> ValueError in bspline_grid | A | not switched yet (relay 5; relay 3 decided it) |
| SDC-07 | SDC-g TOPUPCoeffReorient negates a BIDS `i` direction | D measured -> A | **switched** (fix: never negate + warning; reproduce: negate) |
| SDC-08 | apply/ApplyCoeffsField on a positive-axes 4D series from a file -> AttributeError (numpy rollaxis on nibabel's ArrayProxy; also in fMRIPrep's sdcflows 2.15.1/numpy 2.2.6) | A | **switched** (fix: works + warning; reproduce: AttributeError) |
| SDC-09 | SubtractPhases without EchoTime -> TypeError of None comparison | B | **switched** (fix: clear ValueError; reproduce: TypeError) |
`larmorx_core::defects` holds SDC-04, 05, 07, 08, 09 (after SCI-03). SDC-07's evidence:
`measure_sdc_g.py` (log `oracles/logs/fmap-a-4-sdc07.log`): BIDS-letter negation r = -0.999
(RMS 55 Hz, unwarping worsens r by 0.18), FSL letter r = +0.999 (0.9 Hz), in LAS and RAS;
`probe_sdc07_topup.py`: lx.fsl.topup's coefficients = FSL topup's (r = 1.0000, via the oracle
cache). fMRIPrep impact: none (it passes FSL letters); users of the interface with BIDS letters
get an inverted field.

Next relay (5), in this order:
1. Switches SDC-06 (bspline_grid / bspline_approx / B0FieldTransform: first three zooms by
   default; reproduce: numpy's broadcast ValueError), SDC-01 (measure how often fMRIPrep meets
   weights beyond the knots: the B0FieldTransform.fit approx grid of the oracle runs' BOLD
   references, fMRIPrep's reconstruction grids; then A or C) and SDC-02 (measure the rotated-grid
   error vs an exact non-separable evaluation; then B or A); add them to `larmorx_core::defects`
   (after SDC-09, same block), re-run check_weights/check_b0field/check_bspline_approx.
2. `docs/api/sdcflows-fieldmap.md` (every function: options mapped to sdcflows' interface inputs,
   files, messages, switches; `save_nibabel`; `ridge_sparse_cg` rows from relay 2's note), the
   findings doc `docs/findings/sdcflows-fieldmap.md` (BLAS kernel, NEP 50 rules, nibabel's
   extended precision, the explicit zeros of scipy's kron, SDC-01..09 with evidence), and the
   handoff texts for the register's new SDC family (HANDOFF.md/toml are relay 8's: keep the texts
   in NOTES or a draft file in the scratch).
3. Then relay 6 (parity suite + records in both envs and fMRIPrep's image, golden cases incl.
   `lx.sdcflows` in NAMESPACES: CsrMatrix methods, save_nibabel, apply, phase functions; memory
   case; difftest descriptor + trial; benchmark vs sdcflows: apply on ds005454's full 280 volumes
   is a good benchmark), relay 7 (site), relay 8 (handoff + final report).
Open questions for the orchestrator (relay 8's report): `save_nibabel`'s place (sdcflows family
now; `larmorx.io` later?); check_register's FreeSurfer branch (waits for the FreeSurfer part).

## FROM THE ORCHESTRATOR (2026-11-08)
- Relay 1's agent id: ad5d6c85d74dc98ba (the part's agents so far). Each later relay: add your own id if you can find
  it (your worktree is relay 1's; the orchestrator adds them at the merge otherwise).
- sdcflows' own command line (the BIDS app): ported in part B, as its last step once the workflows it runs exist
  (full completeness: every tool's original command line); part A's interfaces stay without one, as the resampler's.
- `lx.sdcflows` into `tests/golden/registry.py`'s NAMESPACES with the golden cases (relay 6), as noted.

## FROM THE ORCHESTRATOR (2026-11-08, after relay 4)
- `save_nibabel` is nibabel's writer, not sdcflows': move it to `larmorx.io` (e.g. `lx.io.save_nibabel`, its Rust
  side in `larmorx-io` where it fits), its callers updated, its docs on the I/O API page; no alias in `lx.sdcflows`.
  fMRIPrep's other Python steps (part B, larmorprepx) will write through it too.
- `check_register`'s two-magnitude branch (FreeSurfer's `mri_robust_register`) waits for the FreeSurfer part:
  keep the clear NotImplementedError, and name it in next-steps' FreeSurfer item through the handoff.
- Commit 1c48bc82's label: say what it holds in the part's final report (no history rewrite).

## Where relay 5 stopped (2026-11-08 ~05:00)
Agent id: **add71b54eddc33c55** (relay 5; same worktree). Commits: f9698a49 (save_nibabel ->
larmorx.io), d6523175 (SDC-01/02/06/10 switches, API page, findings), d870ebac (parity suite),
d537b523 (clippy), and the final one of this relay (checks with reproduce/fix, part 2b,
NOTES). Working tree clean after it. The worktree's `_core.abi3.so` was built at 03:43 from
d6523175's Rust (d537b523 only rewrote one comparison equivalently and a test): relay 6 rebuilds.

Done and verified:
1. **`save_nibabel` in `larmorx.io`** (orchestrator's item): `python/larmorx/io/nibabel_writer.py`
   (moved with git mv), exported as `lx.io.save_nibabel`, `lx.io.nibabel_stored`,
   `lx.io.WriterError`, `ScalingError`, `HeaderDataError`; no alias in `lx.sdcflows`; callers
   (approx, fmap, phase, transform) and the checks updated; its tests in
   `tests/python/test_io.py` (scaling into int16, a constant image stored as zeros with the
   value as intercept, the header type overridden, an infinity clipped: values probed with
   nibabel itself); section on `docs/api/io.md`; provenance section in
   `crates/larmorx-io/PROVENANCE.md` (the sdcflows PROVENANCE points there). **Golden case
   `io.save_nibabel` is in `tests/golden/registry.py` WITHOUT checksums**: `test_golden
   [io.save_nibabel]` fails ("no recorded checksums") until relay 6 runs `record_golden.py`.
2. **Switches** (larmorx_core::defects SDC-01, -02, -06, -10 added beside 04, 05, 07, 08, 09):
   - SDC-01 (rule D measured -> A): Rust `bspline::SampleWeights` (the recursion on the knots
     continued beyond the grid, `extended_interval`; inside [0, K-1] the original's intervals,
     same bits), `grid_bspline_weights(..., defects)`, `LevelWeights.beyond`.
   - SDC-02 (rule D measured -> A): `bspline::ExactLevel` (per-voxel weights at its position:
     nibabel's apply_affine, per-axis rounding, `(w0*w1)*w2`, ascending columns), used when the
     alignment test fails in fix mode; `Collocation::rmatvec_f32` scatters rows in sample order
     when a level is exact (same bits as the column sums). fix: no "not aligned" UserWarning,
     the SDC-02 notice; reproduce: sdcflows' warning + notice.
   - SDC-06 (rule A): `bspline_grid`/`bspline_approx` use the first three zooms (several
     volumes: a clear ValueError); reproduce: numpy's broadcast ValueError (Python-side switch).
   - **SDC-10 (new, rule A, measured)**: `B0FieldTransform.fit(approx=True)` on a target not
     aligned with the coefficients: sdcflows builds the deobliqued grid around the target as
     given and samples it at the target's own coordinates (the transform ignored). fix: grid
     around `xfm @ target.affine`, sampled there (`_resample_onto(..., sample_affine)`); notice
     whenever the transform is not the identity.
   - Bindings: `sdcflows_collocation_csr/_apply` take `upstream_defects` (default "fix") and
     return notices (apply now returns `(values, notices)`); `sdcflows_bspline_approx` takes it
     and returns `beyond`; `sdcflows_ridge_collocation` uses Reproduce (parity primitive).
     `_core.pyi` updated. Tests: 4 new in `tests/python/test_sdcflows.py` (SDC-01 closed-form
     B-spline rows and same bits inside; SDC-02 exact rows; SDC-06 both modes and multi-volume
     error; SDC-10 approx vs exact field within 0.5 Hz, reproduce > 2 Hz off). Rust: 2 new
     unit tests (21 pass). **Permanent cases** (difftest) for SDC-01/02/06/10: with relay 6's
     descriptor (no sdcflows descriptor exists yet); the checks hold the cases meanwhile.
3. **Measurements** (`validation/investigations/sdcflows-fieldmap/measure_sdc01_02.py`, parts
   0, 1, 1b, 2, 2b, 3; logs `oracles/logs/fmap-a-5-sdc01-02.log`, `fmap-a-5-sdc01b.log`,
   `fmap-a-5-sdc02b.log`, part 3 in the session; run with the shared .venv: needs fMRIPrep's
   load_transforms): fMRIPrep's 28 reconstructions never evaluate on a target beyond the knots
   (always the field map's own grid); sdcflows' own fit: ds005040 BOLD ref 5.5 % beyond (outside
   the brain, up to 70 Hz), ds002603's deobliqued grids 19 % beyond (no BOLD voxel there);
   SDC-02 rotations: separable 0.23 Hz RMS at 0.05 deg ... 39 Hz at 45 deg, approx within 0.13 Hz
   of exact; part 2b: **SDC-02 is reached by sdcflows' own fit with every real registration**
   (the direct path on the projected grid, rotated 0.89 to 1.50 deg): 1.5 to 4.6 Hz RMS, up to
   49 Hz, in the BOLD brain masks of ds002422 x3, ds005040, ds005454; SDC-10 on ds002603: 7.7 /
   8.5 Hz RMS in the brain (part 3's numbers are in the findings doc; its run printed to the
   session only).
4. **Docs**: `docs/api/sdcflows-fieldmap.md` (every function, option-mapping tables per
   interface, files, messages, the defects table, Rust API, command line, limits);
   `docs/findings/sdcflows-fieldmap.md` (versions/step, BLAS kernel, cg/sparse_cg, kron's
   explicit zeros, NEP 50, numpy reductions, Python repr, nibabel's scaled mask and x87 writer,
   SDC-01..10 with evidence and rules); crate PROVENANCE complete (switches, ExactLevel, not
   ported); investigation README (measure_sdc01_02.py; the suite).
   **Register texts for the handoff**: `<scratch>/HANDOFF-register-draft.md` (the families
   sentence, the 10 summary rows, the new section "## sdcflows: field-map estimation (SDC-)"
   with all 10 entries, the 9 rows of docs/api/upstream-defects.md, the next-steps FreeSurfer
   line for check_register's branch and SDC-03); part 2b's numbers are in SDC-02's row/entry.
5. **Parity suite `sdcflows-fieldmap`** (`validation/src/larmorx_validation/parity/
   sdcflows_fieldmap.py`, `tests/parity/test_sdcflows_fieldmap.py`): one case per check script
   (smoke: weights, coeffs, phase, nibabel-writer, b0field; standard: apply, approx-core,
   bspline-approx, ridge), one check per reported OK/FAIL line, skipped unless numpy's OpenBLAS
   runs Haswell/Zen; registered in `__main__.PARITY_MODULES` and `python_versions.SUITES`
   (newest by default; NOT in STEP_SUITES: the scripts record no digests; the step is shown by
   running the record in both envs). The checks compare with `upstream_defects="reproduce"`
   and add corrected cases against sdcflows with the corrections made in its pieces
   (`measure_sdc01_02.fixed_weights`, the SDC-10 projected fit): check_weights (SDC-01, -02,
   -06), check_b0field (SDC-01, -02, -10 incl. ds002603's oblique BOLD refs with fMRIPrep's
   registration), check_bspline_approx (SDC-06), check_apply (SDC-01, SDC-02 through apply).
   **Results: all nine checks ALL OK in both kept environments** (logs
   `oracles/logs/fmap-a-5-checks.log`, `fmap-a-5-checks2.log`); the suite itself has not been
   run through pytest/the CLI yet (relay 6's record run is its first).

Gates run in relay 5 (on what changed): cargo fmt --all --check; clippy -D warnings
(larmorx-sdcflows, larmorx-py, larmorx-core, all targets); cargo test larmorx-sdcflows (21);
ruff check/format (python, tests, validation); check_spdx; pytest tests/python/test_io.py
tests/python/test_sdcflows.py (pass). Not run: the full gates, the smoke tier (relay 6).

## Next relay (6), in this order
1. Rebuild the wheel (`<scratch>/scripts/build_wheel.sh`, boxed 16 GiB).
2. `lx.sdcflows` into `tests/golden/registry.py` NAMESPACES with golden cases for every public
   function and method (CsrMatrix.to_scipy/toarray, B0FieldTransform.fit/apply/
   to_displacements, the phase functions, bspline_approx, ridge_sparse_cg, the coefficients'
   functions, the conversions; one per code path, <= 32^3 inputs, seeded plain arithmetic;
   include a corrected SDC-01/02 case); then the records and checksums from a clean commit:
   `python -m larmorx_validation parity sdcflows-fieldmap --tier standard` (newest, boxed;
   then `--python shared` for the step), `scripts/record_golden.py` (runs the smoke tier first;
   records io.save_nibabel's and the new cases' checksums).
3. Memory case in `tests/parity/test_memory_peak.py`: bspline_approx on a 64x64x44 field map
   (ds001600's or ds002422's) against sdcflows' BSplineApprox (expected far below; the rule:
   <= 110 % + 8 MiB).
4. difftest descriptor `validation/src/larmorx_validation/difftest/tools/sdcflows_fieldmap.py`
   (grids, orientations, masks, recenter modes, zooms_min, rotated/larger targets, readout
   times, pe directions, both modes) + a short boxed trial (`--budget 0.5`) + triage; permanent
   cases for SDC-01, -02, -06, -10 (and 04, 05, 07, 08, 09) in `validation/difftest/cases/
   sdcflows-fieldmap/`.
5. Benchmark vs sdcflows' Python (bspline_approx on the 7 oracle calls; apply on ds005454's 280
   volumes), the validation record `docs/validation/sdcflows-fieldmap.md` and tools.toml's entry
   (through the handoff), gates (`gates-rust.sh <wt> all`, `gates-python.sh <wt> --no-build`,
   `tables --check`, `check_spdx.py`, the parity smoke tier).
Then relay 7 (site page, branch fmap-a-pages), relay 8 (HANDOFF.toml/md from
`<scratch>/HANDOFF-register-draft.md` and the other items, the final report: say what commit
1c48bc82 holds).

Open question for the orchestrator (relay 8's report): the step of the Python environments for
this suite (STEP_SUITES needs digest-recording comparisons; proposed: two records, newest and
shared, both all-pass, as the checks already are).

## FROM THE ORCHESTRATOR (2026-11-08, after relay 5)
- The two environments' record: follow the resampler's pattern (`resample-series`: the record made with
  `--python newest`, the step from the shared environment per case, through `python_versions`' `STEP_SUITES`), giving
  the suite's cases the digests the step needs; only if that cannot be done cleanly, two records (one per environment),
  with the reason in the record's notes.

## Where relay 6 stopped (2026-11-08 ~05:30)
Agent id **a12f04869a7af7743**. Commits (worktree branch, after fc7e195f): 18e0418c (the step),
0d7b77c6 (golden cases), c397728e (difftest descriptor + runner, memory calls, bench script),
8aa567ed + 9e39e0ab (triage_trial.py), f236aedb (benchmark report). Working tree clean. `_core.abi3.so` rebuilt at 04:10 from fc7e195f (no Rust changed in relay 6).

Done and verified:
1. **The originals' step, the resampler's pattern** (the orchestrator's choice of "after relay 5"):
   `sdcflows-fieldmap` is in `python_versions.STEP_SUITES`. Each check script passes sdcflows',
   scikit-learn's, SciPy's, numpy's and nibabel's side of every comparison through
   `validation/investigations/sdcflows-fieldmap/stepdigest.py`'s `oracle()` (digests of the
   content: arrays with NaN alike, decompressed files by content not path, nibabel images by
   data+affine+header bytes, sparse matrices by their arrays, messages without directories or
   addresses); the suite (`parity/sdcflows_fieldmap.py`, `_Lines`) takes the digests gathered
   since the previous reported line when the script prints its OK/FAIL line, and adds one check
   per line: `the originals' own step: ...: <line>` (identical / changed: no known reason /
   varies), `_step` as resample_series' (`CHANGES` empty: none expected). `_highlights` uses
   `resample_series.step_summary`. check_ridge's report now writes `name: detail` (its timings
   were in the check names). Tested on the `nibabel-writer` case (newest with the step: 1 case,
   6 lines compared, identical). **The record itself is not made yet** (relay 8).
2. **Golden** (`tests/golden/registry.py`, end of file, "sdcflows' field-map estimation"):
   `lx.sdcflows` in NAMESPACES; 15 cases covering every public function and method
   (`test_every_tool_operation_and_function_has_a_case` passes): py.sdcflows.weights (+
   .reproduce), csr_scipy (importorskip scipy), ridge, bspline_approx, tools, b0field (+
   .reproduce), coeffs (+ .reproduce), displacements, phase (+ .reproduce), phdiff_workflow
   (PRELUDE `implementation="original"` explicitly: auto would pick the replica where larmorx-nc
   is installed). 3 s in all here (bspline_approx 2 s). Platform care: no NaN in outputs (SDC-04's
   NaN map hashed as `isnan`: NaN bits differ between x86 and ARM), paths replaced by names in
   messages. **No checksums yet**: `test_golden[py.sdcflows.*]` and `[io.save_nibabel]` fail
   "no recorded checksums" until relay 8 runs `scripts/record_golden.py` (after the fixes).
3. **Memory** (`tests/parity/test_memory_peak.py`, `SDCFLOWS_CASES`,
   `test_sdcflows_port_needs_no_more_memory_than_sdcflows`; one call per process through
   `validation/investigations/sdcflows-fieldmap/memory_call.py`; sdcflows in the newest env
   through `oracle_cache.run` (Policy family None; the releases and OPENBLAS_CORETYPE passed as
   `--env`, refused if not the running ones, because the cache keys the interpreter but not its
   packages); larmorx in both D13 modes, its files = sdcflows' in reproduce; allowance 1.1 x
   sdcflows + 8 MiB): **3 passed** (log `oracles/logs/fmap-a-6-memory.log`): fit ds002422 larmorx
   40 MiB vs sdcflows 367; fit ds005454 70 vs 1,474; ApplyCoeffsField ds005454 20 volumes 829 vs
   3,741. The first run left two oracle-cache disputes (nipype's INFO lines carried time stamps;
   the script now silences nipype after importing it, so its key changed; the two disputed
   entries, keys fac9bd9a... and ba61bf75..., are under the old script's key and never served
   again; the orchestrator may resolve them).
4. **Random testing**: `validation/src/larmorx_validation/difftest/tools/sdcflows_fieldmap.py`
   (TOOL `sdcflows-fieldmap`, implementation `port`, family python; registered in
   `difftest/tools/__init__.py`, `permanent.SUITES`, `python_versions.TOOLS`): calls
   bspline_approx (field maps 3D or 4D-of-one, float32/float64/scaled int16, masks, spacings,
   alpha, recenter, extrapolate, zooms_min), fit and apply (coefficients built by the runner on
   the field map's control grids; own/aligned/rotated targets; no/identity/rigid transform;
   approx; apply's pe (per volume too), readout incl. int, jacobian, order, mode, cval, prefilter,
   allow_negative, output_dtype), topup (factors, wrong shape, BIDS and FSL letters),
   displacements (ITK/RAS+, ro 0), phase (au2rads, subtract_phases with echo times / missing).
   The suite's runner (`_random_case`, `_compare`, `_fix_pass`) runs each call through the
   investigation's comparators (`check_bspline_approx.case`, `check_b0field.fit_case`,
   `check_apply.apply_case`/`disp_case`, `check_coeffs.topup_case`, `check_phase.rads_case`/
   `sub_case`) with larmorx in reproduce; one check per reported line named `<line>: every
   output bit-identical` (value 0/1: the tool's exact check), the step, and the D13 fix pass
   (`defects.add_fix_check`, larmorx fix vs reproduce, notices + IDs in error texts).
   **Permanent cases: none written yet** (relay 7: after the fixes, promote shrunk findings or
   write recipes for SDC-01, -02, -04, -05, -06, -07, -08, -09, -10).
5. **The 2-minute trial** (`<scratch>/dt-try/`, seed `sdc-try`, budget 0.03 h, 6 jobs, box 16 GiB,
   commit 0d7b77c6-dirty; not a record: a probe of the descriptor): 12,661 cases, 9,837 agree,
   528 notice, **2,296 differ**. `triage_trial.py <trial> --per-call 50` re-ran 250 of them field
   by field (`<scratch>/dt-try/causes.json`):
   - **A. NIfTI quaternions written (>= 60 %: topup 50/50, displacements 34, bspline_approx 28,
     apply 23, fit 22 of the sample)**: `quatern_b/c/d` of headers larmorx builds from an affine
     differ from nibabel's: signed zeros (e.g. a coefficient grid of a permuted, qfac -1 field
     map: sdcflows -0.0, larmorx +0.0) and last bits. Cause: `larmorx_io::nifti::header::
     NiftiHeader::set_qform` uses `larmorx_core::rotation::matrix_to_quaternion` (Shepperd's
     closed form); nibabel 5.4.2's `set_qform` takes `npl.svd(R)` (polar) then
     `quaternions.mat2quat` (numpy `eigh` of the 4x4 K matrix, LAPACK dsyevd; `q *= -1` if
     q[0] < 0). A larmorx mistake for a port (rule 8): fix in the nibabel-facing writer only
     (the headers lx.sdcflows and lx.io.save_nibabel build through `_core.nifti_header_for_image`
     / `nifti_set_xform`), a nibabel-exact quaternion (port mat2quat with numpy's eigh replica
     or prove a closed form gives eigh's bits incl. zero signs); do not change `lx.save`'s or
     ITK's paths (golden checksums of other tools) without the orchestrator.
   - **B. qform-only inputs (sform_code 0): lx.load's affine is not nibabel's** (216 of 717
     images of the findings' inputs: signed zeros, and last bits up to 3e-16), which then
     changes the output affines and, through them, data (displacements' vectors, #250, #414).
     Cause: `NiftiHeader::qform()` (header.rs:728) computes w and the matrix in f64 with its own
     order; nibabel's `get_qform`: `fillpositive(bcd)` (`np.asarray` of the float32 fields:
     float32 dot under NEP 50?), `quat2mat`, `np.dot(R, np.diag(vox))` (vox float32, `vox[-1] *=
     qfac`; a BLAS product with exact zeros: zero signs from +0/-0 sums). Port it exactly
     (where lx.sdcflows loads: `_as_loaded`) and check on these files
     (`<scratch>/scripts/dbg_qform.py` compares lx.load with nibabel on every finding input).
   - **C. sdcflows' `mapped` held as float64** in some fits (#49: approx=False, rigid xfm,
     rotated target, sheared field-map sform; values and files identical, the in-memory dtype
     differs): find the path (sdcflows' fit: `fmap = colmat @ coeffs` types) and give `mapped`
     the same dtype.
   - **D. bspline_approx data zero signs** (10 of 50: `data(zero signs or NaN bits)`; #48, #163):
     likely the error map's or the field's -0.0 (relay 2's note on the mode's zero sign), check.
   - **E. D13 notices with no visible change**: SDC-01 353 cases, SDC-02 324, SDC-10 57, SDC-07 7
     (`report.md`, "D13 notices without a visible change"): make each notice fire exactly where
     fix and reproduce differ (or declare it in `silent_ok` with the reason), e.g. SDC-02 on a
     pure axis permutation (the separable product is exact there), SDC-01 where the samples
     beyond the grid get weights equal in both bases, SDC-07 when the coefficients are all zero.
   - Others in the sample: apply 'affine last bits' (B through the target), fit 'affine last
     bits' (#98: qform-only target, B), bspline_approx 'affine signed zeros + srow' (coefficient
     grids of qform-only maps: B), topup's quaternion (A).
   Debug helpers (scratch): `scripts/dbg_finding.py`, `dbg_approx.py`, `dbg_cases.py`,
   `dbg_disp.py`, `dbg_qform.py`, `classify.py` (results.jsonl by detail), `triage.sh`.
6. **Benchmark**: `validation/investigations/sdcflows-fieldmap/bench_sdcflows.py` (newest env,
   OPENBLAS_NUM_THREADS=1 as fMRIPrep's nodes; median of 3; outputs checked identical), log
   `oracles/logs/fmap-a-6-bench.log`, JSON `oracles/logs/fmap-a-6-bench.json`; report
   `docs/benchmarks/sdcflows-fieldmap.md` (+ `.json`), every output identical: the 7 bs_filter
   fits at 1 thread 0.62-0.75x sdcflows' speed (ds002603: 48.1 s vs 29.8 s), at 14 threads
   5.3-6.8x faster (4.65 s); ApplyCoeffsField on 60 volumes of ds005454 1.7x faster at 1 and 14
   threads (19.7 vs 33.5 s; 7.3 vs 12.3 s; 280 volumes would need ~50 GiB in sdcflows). Finding: **larmorx's fit at one thread is
   slower than scikit-learn's** (ds005454 24.7 s vs 17.6 s; ds002422 3.0 vs 2.0 s): the
   collocation applied on the fly from the per-axis weights (the memory design: 70 vs 1,474 MiB)
   costs more per CG iteration than scikit-learn's stored CSR; at 14 threads 6.8x faster (2.6 s).
   Within the memory rule (1.1 x + 8 MiB) a stored CSR (8 bytes per entry, ~56 M entries for
   ds005454 = 450 MB, below sdcflows' 1.47 GB) would be allowed: a speed option for relay 7 or
   the clean-room original (decide with the orchestrator; bit-identical either way).
7. Gates run in relay 6 on what changed: ruff check/format (validation, tests, python),
   check_spdx; pytest tests/golden -k sdcflows (the coverage test passes; the cases fail only
   for missing checksums); the memory tests. Not run: gates-rust (no Rust changed), the full
   gates, the smoke tier.

## Next relay (7), in this order
1. Fix A, B, C, D (above) in larmorx's nibabel-facing code (lx.sdcflows' loading and header
   building, `lx.io.save_nibabel`), each with a unit test and the investigation's checks re-run
   in both envs (`<scratch>/scripts/run_all_checks.sh`, boxed); make E's notices exact.
   Rebuild (`<scratch>/scripts/build_wheel.sh`, boxed 16 GiB) if Rust changes.
2. Re-run the 2-minute probe (`lxv.sh . difftest --tools sdcflows-fieldmap --budget 0.03 --jobs 6
   --seed sdc-try --out <scratch>/dt-try2`, boxed 16) until no difference is left that is
   larmorx's; then permanent cases in `validation/difftest/cases/sdcflows-fieldmap/` (promote the
   shrunk findings that were larmorx's: `difftest --promote <finding> --id <name> --status fixed`;
   and one recipe per defect SDC-01, -02, -04, -05, -06, -07, -08, -09, -10 with `--status
   register --defect SDC-0x --rule A`), the short trial `--budget 0.5` (boxed, 12 jobs at most:
   14 threads) on a clean commit, every remaining difference triaged into
   `validation/difftest/triage.toml` (new block "sdcflows-fieldmap (2026-11-08)").
3. Then relay 8: the record `python -m larmorx_validation parity sdcflows-fieldmap --tier
   standard --out <dir>` from a clean commit (newest, boxed 24 GiB, jobs <= 6: the step runs the
   shared env twice first; ~20-30 min), its `docs/validation/sdcflows-fieldmap.md` (+ .json),
   `scripts/record_golden.py` (boxed; runs the smoke tier first: check its worker count against
   the 14-thread budget), the full gates; relay 9 the site, relay 10 the handoff and report.

## FROM THE ORCHESTRATOR (2026-11-08, after relay 6)
- The plan split is accepted: relay 7 fixes the trial's findings (permanent cases, the 0.5 h trial, triage), relay 8
  the records, golden checksums and full gates, relay 9 the site, relay 10 the handoff and the final report.
- nibabel's header arithmetic stays nibabel-specific: the quaternions as nibabel writes them (`mat2quat` and numpy's
  `eigh` as nibabel reaches them) in `lx.io.save_nibabel` and the headers `lx.sdcflows` builds; the affine of a
  qform-only image as nibabel computes it through a nibabel-exact loader beside `lx.load` (e.g. `lx.io.load_nibabel`,
  or a `nibabel` mode of the reader) that `lx.sdcflows` uses. `lx.load`, `lx.save` and the ITK path stay unchanged, so
  no other tool's record or golden checksum moves.
- The stored collocation matrix: yes, now (the port must not be slower than sdcflows' Python at one thread): the stored
  path by default where its peak keeps the memory rule (sdcflows' peak plus 10 %, plus 8 MiB, on the same call), the
  low-memory path where it would not (or as an option); the same bits both ways, tested.
- The two oracle-cache disputes of the first memory run (nipype's printed timestamps, the old script's key): leave
  them in the cache's findings, as the cache's rule says (never overwritten); name them in the part's report.

## Where relay 7 stopped (2026-11-08 ~06:45)
Agent id **abf1e3da6428896bb**. Commits (after f236aedb): 129854e9 (nibabel's arithmetic:
numpy_linalg, nibabel_qform, load_nibabel), ab801550 (float64 field, numpy's NaN, notices),
18756aff (stored collocation, io_orientation, '<f4', 3D mask, deferred apply notices, SDC-02
test), e9817463 (tests, permanent cases, golden case, docs, PROVENANCE), d47952d1 (format), and
75e4bebf (SDC-11, triage), c5bfc440 (the trial's last four), 7a2fda55 (the stored matrix at one
thread). `_core.abi3.so` in the worktree is current (built 07:04 from 7a2fda55's Rust). Gates
run in relay 7 (on 7a2fda55): cargo fmt --check (all), clippy -D warnings --locked and cargo
test on larmorx-core, -transform, -io, -sdcflows, -py; ruff check/format; check_spdx;
`pytest tests/python validation/tests` 837 passed; `tests/golden`: only the 16 cases without
recorded checksums fail (io.load_nibabel, io.save_nibabel, 14 py.sdcflows.*). Not run: the
crates-gpl/crates-nc workspaces, the parity smoke tier (relay 8).

Done and verified:
1. **The trial's findings (relay 6's causes A-E), fixed** (all in nibabel-facing code; `lx.load`,
   `lx.save`, the ITK path and `lx.transforms` unchanged):
   - **A. nibabel's quaternions**: `crates/larmorx-transform/src/numpy_linalg.rs` = numpy's
     `svd` (3x3, dgesdd path 5a), `eigh` (4x4, dsyevd), `det > 0` (getf2 + slogdet), with
     LAPACK 3.12.1 (OpenBLAS 0.3.34, default) or 3.12.0 (0.3.30) (`LapackRelease`: DLARF1F vs
     DLARF) on the Haswell kernels (disassembly: `drot` and `dgemv_t`'s tails fused, `dnrm2`
     x87, the rest plain); `larmorx_io::nifti::nibabel_qform` / `set_qform_nibabel` /
     `header_for_image_nibabel`; `_core.nifti_header_for_image(..., nibabel=True, lapack=)`,
     `nifti_set_xform(..., nibabel=True, lapack=)`, `_core.nibabel_qform`,
     `_core.numpy_linalg_small(kind)` (svd, eigh, det_positive, io_polar).
     `check_numpy_linalg.py`: 4,048 matrices bit-identical in both envs. **The LAPACK step is
     real**: nibabel's float32 quaternions differ between numpy 2.5.4 and 2.3.5 in ~2-3 % of
     oblique affines (findings doc); the checks select 3.12.0 in the shared env
     (`stepdigest._match_numpy_lapack` -> `lx.io.nibabel_header.use_lapack`, a ContextVar).
   - **B. qform-only / no-code affines**: `python/larmorx/io/nibabel_header.py`:
     `nibabel_affine(header)` (get_best_affine: get_qform in exact x87 emulation, base affine
     with -0.0), `load_nibabel` (exported `lx.io.load_nibabel`, `lx.io.nibabel_affine`),
     `io_orientation` (numpy's SVD polar factor: ties!), `ensure_positive_cosines`; lx.sdcflows
     uses them everywhere (`_as_loaded`, phase.py loads, au2rads' header affine, transform/
     coeffs/phase orientations).
   - **C. float64 field**: scipy's `kron` of an empty axis is float64 -> `Collocation::
     scipy_upcasts`, `matvec_f64_upcast`; `sdcflows_collocation_apply` returns float64 then.
   - **D. NaN signs**: numpy's median/mean of nothing = x86 default NaN (0xffc00000); a NaN
     among values returned as is; scipy mode of nothing np.nan (approx.rs).
   - **E. notices**: SDC-02 only when a rotated level stores a weight in either mode and some
     index moves another axis' coordinate (`LevelWeights::sdc02_changes`); SDC-07 named after the
     shape check; apply defers fit's D13 notices until the series is unwarped; the suite's apply
     fix pass also compares `mapped` (that made SDC-01/SDC-10's apply notices visible).
   - Also found and fixed on the way: scikit-learn's "dtype('<f4')" in its infinity message
     (nibabel's raw LE float32 array); a 3D mask with a 4D-of-one field map in fix mode
     (SDC-06's correction completed); check_b0field writes larmorx's field with save_nibabel.
2. **Stored collocation matrix** (orchestrator's decision): `bspline::StoredCollocation`
   (`Collocation::store`: CSR with u32 indices, 8 B/weight; with n_threads > 1 also its
   transpose, 16 B/weight; row counts from the patterns without evaluating weights; at one
   thread the adjoint scatters the rows in order as scipy's csc_matvec on X.T does) is the
   fit's operator by default and also evaluates the field on the original grid;
   `low_memory=True` (Python `bspline_approx`, `ApproxParams.low_memory`, the binding) keeps the
   on-the-fly path. Same bits (Rust test, Python test, all checks). **Speed at one thread**
   (`<scratch>/r7/quick_fit_time.py`, one run each, newest env, OPENBLAS_NUM_THREADS=1) against
   relay 6's scikit-learn medians: ds005454 15.5 s vs 17.6; ds002422 1.73 vs 2.02 and 3.12 vs
   3.74; ds002603 24.4 vs 29.8 (low_memory: 23.8, 2.76, 4.90, 45.9). Memory test before the
   one-thread change (`oracles/logs/fmap-a-7-memory.log`, CSR + transpose): fit ds002422
   larmorx 127 vs sdcflows 366 MiB; fit ds005454 877 vs 1,473 MiB; apply 828 vs 3,740 MiB;
   after it (`oracles/logs/fmap-a-7-memory2.log`, one thread, rows only): ds002422 83 vs 366
   MiB, ds005454 468 vs 1,473 MiB, apply 828 vs 3,740 MiB (3 passed). (sdcflows holds >= 24 B/weight: its matrix,
   `_preprocess_data`'s copy, the adjoint operator's conjugate transpose.)
3. **Permanent cases**: 23 in `validation/difftest/cases/sdcflows-fieldmap/` (14 fixed
   findings; SDC-01, -02, -04, -05, -06, -07, -08, -09, -10 registered with their rules), every
   one `agree` replayed (`oracles/logs/fmap-a-7-replays.log`).
4. **Checks**: all ten investigation checks ALL OK in both envs after the changes
   (`oracles/logs/fmap-a-7-checks.log`); unit tests: test_io.py (5 nibabel tests), test_sdcflows.py
   (7 new), Rust (stored matrix bits, nibabel quaternions, NaN rules); cargo fmt/clippy clean on
   the touched crates.
5. **Golden**: case `io.load_nibabel` added (covers lx.io.load_nibabel, nibabel_affine); NO
   checksums yet (relay 8). The checksums of io.save_nibabel and the py.sdcflows.* cases will
   also be recorded then (header quaternions now nibabel's).
6. **The 0.5 h trial** (`oracles/difftest/2026-11-08-sdcflows-fieldmap`, seed sdc-trial, 12
   jobs, 3 GiB per case, box 28 GiB, from clean d47952d1): **335,471 cases, 335,456 agree, 1
   notice, 14 differ** (report.md there). The 14, three classes: (i) 10 cases, a resize onto a
   single voxel (nitransforms' bare `AssertionError`) -> **SDC-11 (new, rule A)**: resampled and fitted by
   default, named; `reproduce` the bare assertion (`tools.resample_to_zooms(...,
   upstream_defects=)`, `larmorx_core::defects::SDC_11`; register texts added to
   `<scratch>/HANDOFF-register-draft.md`: summary row, section, api row). The notice: SCI-02
   (lx.ndimage's padding-condition notice, harness: silent_ok in apply's fix pass). (ii) 2
   cases: the error map's cal_max = numpy's max of sdcflows' Fortran-ordered array (NaN bits,
   zero sign by memory order) -> larmorx keeps the error map in Fortran order. (iii) 2 cases: a
   zero mode with zeros of both signs -> the zeros' more frequent sign (numpy's unstable sort
   leaves it undefined; numpy's on the cases). All triaged in `validation/difftest/triage.toml`
   (block "sdcflows-fieldmap (2026-11-08)"), all with permanent cases (28 in all). Final
   replays with the final build 7a2fda55 (14 trial findings + 28 permanent cases, all 42 agree):
   `oracles/logs/fmap-a-7-final-replays3.log`. (75e4bebf's message says "14 SDC-11": 10 are.)
7. Not done in relay 7 (relay 8): the benchmark report (`docs/benchmarks/sdcflows-fieldmap.md` +
   `.json`) still shows relay 6's on-the-fly fit numbers; see the quick numbers below.

Defect IDs now: SDC-01..11 (SDC-03 still waiting for the FreeSurfer branch). Register texts for
the handoff: `<scratch>/HANDOFF-register-draft.md` (SDC-11 included).

## Next relay (8), in this order
1. Rebuild the wheel first (`<scratch>/scripts/build_wheel.sh`, boxed 16 GiB) if anything Rust
   changed; at 75e4bebf the worktree's `_core.abi3.so` is current (built 06:39).
2. **The record** from a clean commit: `scripts/orchestration/lxv.sh . parity sdcflows-fieldmap
   --tier standard --python newest --out <dir>` (boxed 24 GiB, jobs <= 6; the step runs the
   shared env twice first, ~20-30 min) -> `docs/validation/sdcflows-fieldmap.md` (+ .json).
   Expect the originals' step to show header changes from nibabel's LAPACK 3.12.0 vs 3.12.1
   (quaternions of oblique headers, ~2-3 % of them) and not elsewhere: give that reason in
   `CHANGES` of `parity/sdcflows_fieldmap.py` if the step flags them ("no known reason"
   otherwise), with the findings doc's paragraph ("LAPACK 3.12.0 to 3.12.1").
3. `scripts/record_golden.py` (boxed; it runs the smoke tier first: check its workers against
   14 threads): records io.save_nibabel, io.load_nibabel and the 15 py.sdcflows.* cases.
4. The benchmark (`bench_sdcflows.py --threads 1,14 --repeats 3`, boxed; newest env;
   OPENBLAS_NUM_THREADS=1): the stored fit at one thread must not be slower than scikit-learn
   (relay 6: 0.62-0.75x with the on-the-fly operator); update the report and its JSON.
5. Full gates: `gates-rust.sh <wt> all` (crates-gpl/crates-nc locks: larmorx-transform and
   larmorx-io changed, refresh if their builds need it), `gates-python.sh <wt> --no-build`,
   `tables --check`, `check_spdx.py`, the parity smoke tier.
6. Then relay 9 (site: the evidence page; mention nibabel's arithmetic and the stored matrix),
   relay 10 (HANDOFF.toml/md from the register draft + upstream.tsv rows + tools.toml entry +
   golden.md + next-steps; the final report: say what commit 1c48bc82 holds; the two
   oracle-cache disputes of relay 6; the LAPACK step for the orchestrator).
Open for the orchestrator (relay 10's report): the nibabel-exact helpers live in
`larmorx.io.nibabel_header` (header_for_image, set_xform, io_orientation,
ensure_positive_cosines, use_lapack); `lx.transforms.ensure_positive_cosines` (the resampler)
still uses vnl's SVD: ties of a rotation between two axes may pick another axis than nibabel
does there (a follow-up for the resampler's own records, not changed here by the rule "no other
tool's record moves").

## FROM THE ORCHESTRATOR (2026-11-08 07:20, after relay 7)
- Relay 7's agent id: abf1e3da6428896bb. Its plan for relay 8 above is agreed, in that order.
- numpy 2.3.5 against 2.5.4 (LAPACK 3.12.0 against 3.12.1) on oblique headers' float32 quaternions: a measured step
  between the environments, named with its reason in the record's `CHANGES` and the findings doc, as relay 7 says.
- `lx.transforms.ensure_positive_cosines` (vnl's SVD in the resampler) stays unchanged in this part: relay 10 puts it
  in the handoff as a next-steps follow-up (the resampler's own records, measured first).
- Beside you: 3dvolreg's spec writer (its own worktree, within 32 GiB / 14 threads). Keep to your 32 GiB / 14.

## Where relay 8 stopped (2026-11-08 ~10:15)
Agent id **a3f6fcb919202dd47**. Commits (after 7a2fda55): f99a809b (the step's masked digest),
d0bd51da (LAPACK-free inputs, the orientation reason, bench --low-memory, findings), 14ff7709
(the first record, from d0bd51da), 2f6d6744 (golden checksums), 3c9c10af (the stored matrix at
one thread only), 2ed5b887 (the record re-made from 3c9c10af), a31e70bb (the benchmark report,
bench_fit_paths.py, the README rows, the API page's speed), 7f47bf81 (the step's probes
probe_step_explain.py, probe_qr_environments.py: scripts and a README row only, after the smoke
tier; ruff and SPDX pass on it). Working tree clean. `_core.abi3.so` in the worktree is current (built 08:50 from 3c9c10af's Rust).

Done and verified:
1. **The originals' step made exact** (relay 7's expectation of header changes). Three causes,
   each with a rule, never a list of cases:
   - `stepdigest` keeps a second digest per line with the NIfTI quaternion fields zeroed
     (`mask_quaternions`, NIfTI-1 256:268, NIfTI-2 352:376; headers, files, raw bytes); a line
     whose full digest changed and whose masked digest is identical in every run has the reason
     **`LAPACK-3.12.1`** (`python_versions.CHANGES`, text there; the suite's `CHANGES` names it).
   - A random case whose input images hold an orientation numpy's SVD decides
     (`_orientation_step`: nibabel's `io_orientation` of each input's affine under LAPACK 3.12.0
     and 3.12.1 in larmorx's replica, `lx.io.nibabel_header`, differs) has the same reason for
     any change of its lines. Found on the permanent case `sdc02-single-voxel-no-notice`
     (quaternion (3, 3, 0, 5)): under 3.12.1 sdcflows sees positive axes, does not reorient and
     fails with SDC-08's AttributeError; under 3.12.0 it reorients and unwarps (probe:
     `<scratch>/r8/probe/step_explain.py`, which describes every digested object per env).
   - **Inputs from `numpy.linalg.qr` differ between the environments** (102 of 150 matrices of
     check_bspline_approx's `rescale_affine` inputs; check_coeffs' seed-5 oblique reference too):
     the step compared the originals on other inputs. Both scripts now draw their oblique
     matrices with `stepdigest.orthogonal(rng)` (a unit quaternion of four normal draws, plain
     float arithmetic, a column negated half of the time). Their cases changed (new inputs): all
     pass in both environments (the record).
   Tests: `validation/tests/test_sdcflows_step.py` (4: the mask, the masked reason, an inputs'
   reason, the tie detector on the (3, 3, 0, 5) rotation). Findings doc: the LAPACK paragraph.
2. **The record** `docs/validation/sdcflows-fieldmap.md` (+ .json), from 3c9c10af (standard
   tier, `--python newest`, box 24 GiB, 6 jobs, ~14 min): **37 cases, 37 pass, 0 both, 0
   expected, 0 fail (821 checks)**; the step: 33 cases identical, **4 changed, all
   LAPACK-3.12.1** (`difftest/nibabel-quaternion-apply`, `-displacements`, `-topup`: quaternions
   only; `difftest/sdc02-single-voxel-no-notice`: the orientation tie); 381 reported lines
   compared under both releases. (The first record from d0bd51da, same numbers, is kept in
   `<scratch>/r8/record-d0bd51da/`; the failed first try in `record-try1/`.) API page: status
   `validated` with the record's link.
3. **Golden** (`scripts/record_golden.py --parity-workers 12`, box 28 GiB, from 14ff7709): the
   smoke tier first (2,880 passed: 2,651 in parallel in 321 s, then the memory and thread tests
   serially in 1,106 s; marker `parity-smoke-14ff7709...json`), then **16 new cases**
   (io.load_nibabel, io.save_nibabel, 14 py.sdcflows.*), **no other case changed** (256 cases,
   1,967 outputs; the cases 13.5 s in all, py.sdcflows.bspline_approx 0.93 s the longest of the
   new ones). Committed 2f6d6744. 3c9c10af changes no bits (tests/golden passes after it).
4. **The benchmark** (`bench_sdcflows.py --repeats 3 --volumes 60 --threads 1,14 --low-memory`,
   newest env, OPENBLAS_NUM_THREADS=1, box 24 GiB, from 2ed5b887; report
   `docs/benchmarks/sdcflows-fieldmap.md` + `.json`; the earlier run kept as
   `sdcflows-fieldmap-stored-every-thread-count.json`): every output byte-identical; the seven
   bs_filter fits at **one thread 1.09-1.14x scikit-learn's speed** (ds005454 15.26 vs 16.72 s,
   ds002603 24.87 vs 27.48 s), **14 threads 6.5-7.8x** (2.15 s, 4.04 s), low_memory at one thread
   0.60-0.72x; ApplyCoeffsField 60 volumes 1.7x at 1 and 14 threads (18.68 vs 31.83 s; 6.56 vs
   11.29 s). **Open follow-up (in the report, footnote 1):** the stored matrix's one-thread time
   depends on what the process ran before: ds001600 2.52-2.61 s in a fresh process, 3.3 s in this
   benchmark run, 5.5-5.7 s after the benchmark's first three calls (sdcflows' runs included) in
   the same process (`bench_fit_paths.py process`, reproduced; not after larmorx-only calls; not
   glibc's dynamic mmap threshold: `GLIBC_TUNABLES=glibc.malloc.mmap_threshold=65536` changes
   nothing; THP is `madvise` here, numpy madvises its arrays, larmorx's are never marked);
   low_memory varies < 1 %; bits never change. Matters before larmorprepx runs fits in a
   long-lived process. Probes: `<scratch>/r8/probe/` (fit_threads.py, fit_one.py, fit_order.py,
   bench_first4.py; committed as `bench_fit_paths.py`).
   Found on the way: at 2f6d6744 the stored matrix was also used at 14 threads, where it was 2 to
   3.3 times SLOWER than evaluating the weights (ds005454 7.6 s against 2.3 s; ds002422 0.62
   against 0.30 s: products bound by memory bandwidth, the transpose built serially): **3c9c10af
   stores the matrix at one thread only** (`approx.rs`: `low_memory || n_threads != 1` -> the
   weights evaluated; same bits: Rust and Python tests, the record re-made). `low_memory=True`
   now forces the on-the-fly path at one thread. The orchestrator's decision ("the stored path by
   default") is kept where it was made for (one thread); say so in relay 10's report.
5. **Gates** (on a31e70bb): `gates-rust.sh all` (root, crates-gpl, crates-nc: fmt, clippy -D
   warnings --locked, tests; no lock refresh needed: crates-gpl/nc do not build larmorx-sdcflows;
   build caches seeded from the main checkout's targets), `gates-python.sh --no-build` (1,100
   passed, 12 skipped; 714 without FSL; collection without larmorx_validation; ruff; format;
   SPDX), `tables --check`: all pass. The new golden cases take 1.9 s here (bspline_approx 0.94
   s). The parity smoke tier on a31e70bb (`parity_smoke.py --workers 12`, box 28 GiB): **2,880
   passed**, 0 failed (2,651 in parallel in 294 s, then the memory and thread tests serially in
   1,063 s; marker `parity-smoke-a31e70bb...json`; the first one, on 14ff7709, before
   record_golden: 2,880 passed too).

## Next relay (9), in this order
The site's evidence page (brief sub-task 4; `docs/figures.md`, "Add a tool"). Large: split it
in two relays if needed (9: the evidence module and its export; 10: the site page, its checks;
11: the handoff and the final report), and say so here.
1. `validation/src/larmorx_validation/evidence/sdcflows_fieldmap.py` (`export(ctx)`, in
   `evidence/__init__.py`'s `TOOLS`; models: `resample_series.py` (a Python port, both
   environments, the step), `fsl_prelude.py` (fMRIPrep's phase-difference calls, sdcflows' brain
   masks)). Examples from fMRIPrep's real calls: the phase-difference preparation (raw phase and
   magnitude, sdcflows' and larmorx's field map, ds005454 / ds002603 / ds002422), the B-spline fit
   (field map, coefficients, the fitted field and error map; fMRIPrep's own `bs_filter` files),
   the unwarping (ApplyCoeffsField on ds005454's BOLD volumes); differences shown as exact zeros
   with the reason; the D13 corrections where a real input carries them (SDC-10 on ds002603's
   oblique references: 7.7-8.5 Hz RMS; SDC-07 on ds005040's TOPUP coefficients; SDC-06 on the
   4D-of-one field maps); the step (LAPACK 3.12.0 -> 3.12.1) and the speed (this benchmark:
   `docs/benchmarks/sdcflows-fieldmap.json`), the memory (`test_memory_peak`'s numbers); mention
   nibabel's arithmetic and the stored matrix at one thread. Cases' verdicts from the record
   (3c9c10af). Export from a clean commit.
2. The site clone `<scratch>/site` (site `main` and `evidence-blobs` as they are when the relay
   starts; branch `fmap-a-pages`; the no-reply identity first): `content/tools/<tool>.md`
   (`evidence: true`, `wide: true`), the checks (build 0 problems, number and viewer checks,
   inventory after >= before on every page, screenshots 1440 and 390, light and dark).
3. Then relay 10 (or 11): HANDOFF.toml/md (from `<scratch>/HANDOFF-register-draft.md`: the
   register's SDC family SDC-01..11 with SDC-03 reserved, upstream.tsv rows, tools.toml entry with
   status `validated` and the record, golden.md's 16 cases, next-steps; the follow-ups:
   `lx.transforms.ensure_positive_cosines`' vnl SVD (the resampler's records, measured first),
   the stored transpose kept only for tests and a clean-room original), the part's agents' ids,
   the final report (what commit 1c48bc82 holds; the two oracle-cache disputes of relay 6; the
   LAPACK step; relay 8's change of the stored matrix's default to one thread).

## FROM THE ORCHESTRATOR (2026-11-08 10:08, after relay 8)
- Relay 8's agent id: a3f6fcb919202dd47.
- **Confirmed:** the stored collocation matrix by default at one thread only, the weights on the fly at more (the
  same bits; the faster path at each thread count, within the memory rule). The API page and the benchmark say so.
- **Agreed:** the split: relay 9 = the evidence module and its export (step 1), relay 10 = the site page and its
  checks (step 2), relay 11 = the handoff and the final report (step 3, with relay 8's open follow-up: the one-thread
  fit's speed depending on what the process ran before, for larmorprepx's long-lived processes, in next-steps).
- Beside you: the orchestrator's merge verification of 3dvolreg's clean-room original (8 threads, 32 GiB); keep to
  32 GiB / 14 threads.

## Where relay 9 stopped (2026-11-08 ~11:40)
Agent id: not visible to the agent (the orchestrator has it; transcript meta "Field maps A, relay
9" if set). Commits (worktree branch, after 7f47bf81): **a2b9395c** (the evidence module, its
runner, `TOOLS`, the summary's page), **61185255** (speed chart's note on ds001600), **e2f73918**
(SDC-06's table compares values with the singleton volume axis dropped), **25120b6b** (the
unwarped series compared in the image's units where the scale factors differ; the coefficients'
view in the middle slice). Working tree clean. `_core.abi3.so` unchanged (no Rust changed).
Site clone: `<scratch>/site` (cloned from the local site checkout's `main` **b9d118cc** = the
3dvolreg clean-room merge, which origin did not have yet at 10:27 (origin/main e4b3757);
`evidence-blobs` **a4fa3b63**, 1236 blobs fetched and verified; no-reply identity and the
commit-msg hook set), branch **fmap-a-pages**, commit **1663e0ec** (only
`static/evidence/sdcflows-fieldmap.json` and `static/evidence/blobs.sha256`; blobs verify: 1275
blobs, 0 problems; the 39 new blobs are only in `static/evidence/blobs/`, not yet in the
evidence-blobs branch). Logs: `oracles/logs/fmap-a-9-{try1,try2,export,export2,export3}.log`.

Done and verified:
1. **`validation/src/larmorx_validation/evidence/sdcflows_fieldmap.py`** (`TOOL =
   "sdcflows-fieldmap"`, in `evidence/__init__.py`'s `TOOLS`; `summary.py`'s `PAGES` has
   `/tools/sdcflows-fieldmap/`, eco `larmorx`, the port `validated`). Every sdcflows and larmorx
   run is a process of **`evidence/sdcflows_fieldmap_run.py`** (tasks estimator, apply, fit,
   refit) in a kept environment (newest = reference, shared = the step), OPENBLAS_CORETYPE=Haswell,
   OPENBLAS_NUM_THREADS=1. Cache entries (`oracles/figures-cache/sdcflows-fieldmap/`, all
   computed at clean 61185255, ~6 min in all): `estimator-<key>` x7 (sdcflows newest + shared,
   larmorx 1 thread reproduce, 14 reproduce, 14 default, `phdiff_workflow` with lx.fsl.prelude's
   original), `apply-ds005454` (first 10 volumes; sdcflows num_threads 8), `sdc10-ds002603` (both
   oblique BOLD refs: sdcflows x2 envs, larmorx both modes, the exact field via
   `measure_sdc01_02.exact_field`), `sdc06-refit` (fMRIPrep's 4 own 4D-of-one field maps of
   ds005454, ds002603, ds002422 x2), `memory` (memory_call.py under /usr/bin/time -v).
2. **The JSON** (`python -m larmorx_validation evidence sdcflows-fieldmap --site <scratch>/site`,
   from 25120b6b, cache served): **examples** `estimator-ds005454`, `estimator-ds002603`,
   `estimator-ds002422-run01` (inputs: raw phase, magnitude, mask over it, fMRIPrep's unwrapped
   phase; outputs rads, fmap, extrapolated, error; panels sdcflows, larmorx 1/14, default,
   sdcflows-shared; fMRIPrep's own files and the workflow compared over their files only: all
   **bit-identical**, the step identical), `coefficients-ds005454` (fMRIPrep's own panel;
   identical), `apply-ds005454` (4 of 10 volumes shown; larmorx = sdcflows bit for bit; **the
   default corrects SDC-02 on this real call**: fMRIPrep's registration rotates the series' grid,
   field 3.65 Hz RMS / 31.3 Hz largest in the brain, series 8.73 RMS (range 2417): verdict d13),
   `sdc10-ds002603` (sdcflows vs the exact field 8.48 Hz RMS, largest 82.9 Hz; larmorx default =
   exact to 9.2e-5 Hz; reproduce = sdcflows; d13, names SDC-01 and SDC-10). **Tables**
   `estimators` (all 7 estimators: every output identical to sdcflows and fMRIPrep's own files,
   the workflow, threads, modes, the step), `sdc10` (both refs: 8.48 / 7.67 Hz RMS against 5e-6),
   `sdc06` (4 maps: sdcflows ValueError, reproduce the same, default = the 3D map's fit, d13),
   `corrections` (SDC-01..11 from the findings doc with rule, where shown, the record's case),
   `step` (381 lines, 4 changed, LAPACK-3.12.1), `memory` (fit ds002422 367 vs 82/81 MiB, fit
   ds005454 1,474 vs 468, apply 20 vols 3,741 vs 828/829), `trial` and `triage` (335,471 cases;
   the 4 triage entries). **speed** (the benchmark JSON's own format, built here: 7 bs_filter
   fits + ApplyCoeffsField; note on ds001600's one-thread fits 0.86x after earlier calls in the
   process, the report's footnote 1). **cases** 37 (33 bit-identical, 4 d13: sdc01, sdc02, sdc07-bids
   and sdc10: a nonzero "D13 fix pass"), implementation `port` validated.
   Credits: ds001600, ds002422, ds002603, ds005454 (attach passed). Size: page 19.31 MB (budget
   40), site 351.2 MB (500).
3. Checks: ruff check/format (validation, python, tests), check_spdx, `pytest validation/tests`
   (384 + the 75 oracle-cache tests serially; under `-n 8` two oracle-cache tests fail,
   test_above_one_thread_a_run_is_served_after_three_identical_runs and
   test_exit_status_and_failures_are_cached: parallel runs only, serially all pass; this part
   does not touch the cache's code). Posters looked at (`<scratch>/r9/posters/montage.png`).

Found on the way (for relay 11's report / the orchestrator):
- **PRELUDE's page calls ds005454 "a GRE phase difference (Siemens, 3 T)"**
  (`evidence/fsl_prelude.py`, `FMRIPREP`): its metadata says 7 T (MagneticFieldStrength 7,
  MAGNETOM Terra.X). Not this part's file: a follow-up.
- SDC-02 is met on a real call of sdcflows' own unwarping (ApplyCoeffsField on ds005454 with
  fMRIPrep's registration), as relay 5's part 2b measured for the fit (1.5 to 4.6 Hz RMS).
- The unwarped series' two int16 files (default and reproduce) have their own scale factors:
  compared in image units.

## Next relay (10), in this order
The site page and its checks (brief sub-task 4), in `<scratch>/site` on branch `fmap-a-pages`:
0. If the orchestrator has published another site `main` or `evidence-blobs` since b9d118cc /
   a4fa3b63, rebase `fmap-a-pages` on it, `python3 tools/blobs.py fetch`, and re-export
   (`scripts/orchestration/boxed.sh 16 scripts/orchestration/lxv.sh . evidence
   sdcflows-fieldmap --site <scratch>/site --no-compute`: seconds, the cache is complete).
1. `content/tools/sdcflows-fieldmap.md` (`evidence: true`, `wide: true`; model:
   `content/tools/resample-series.md`, a port with the step): facts (port of sdcflows 2.17.1,
   status validated, records `docs/validation/sdcflows-fieldmap.md`, benchmark, API page
   `docs/api/sdcflows-fieldmap.md`, findings), what it is (with a short `lx.sdcflows` code
   example), `ev.agreement('sdcflows-fieldmap', 'port')`, the three estimator examples
   (`ev.example('sdcflows-fieldmap', 'estimator-ds005454')`, ...), `coefficients-ds005454`,
   `ev.table(..., 'estimators')`, `apply-ds005454` (SDC-02), `sdc10-ds002603` + table `sdc10`,
   table `sdc06`, table `corrections`, table `step`, `ev.speed`, table `memory`, tables `trial`
   and `triage`, `ev.cases`. Mention nibabel's header arithmetic (lx.io.save_nibabel /
   load_nibabel: quaternions with numpy's eigh, LAPACK 3.12.0/3.12.1) and the stored matrix at
   one thread.
2. `python3 tools/blobs.py commit` (evidence-blobs as one parentless commit of the manifest's
   blobs), `tools/build.py` (0 problems), the number check and the viewers' check (site README,
   "The validation evidence"), inventory after >= before on every page (the new page only adds),
   screenshots 1440 and 390, light and dark (several outputs, planes, compare modes).
   Shared files (`data/status.yml` is generated from tools.toml; `site.yml`'s navigation,
   `content/validation.md`, `static/evidence/summary.json`): only through the handoff (relay 11).
3. Then relay 11: HANDOFF.toml/md (from `<scratch>/HANDOFF-register-draft.md` and "Next relay
   (9)" item 3 above), the part's agents' ids, the final report (with relay 8's open follow-up,
   the PRELUDE page's 3 T, the xdist-only oracle-cache test failures).


## FROM THE ORCHESTRATOR (2026-11-08 10:44, after relay 9)
- Relay 9's agent id: a7b32ff0ee5bd279b. Its plan for relay 10 (the page, `tools/blobs.py commit`, the build and
  every check, the screenshots) is agreed; relay 11 the handoff and the final report.
- The site's main b9d118cc and evidence-blobs a4fa3b63 (your clone's base) are now published (3dvolreg's clean-room
  original merged, 2026-11-08 10:50); larmorx main is befa2f36. Your branch needs nothing from them.
- PRELUDE's page's ds005454 label (3 T against the metadata's 7 T): the orchestrator fixes it outside this part.
  The two oracle-cache tests that fail only under `-n 8`: relay 11 names them in the report (a follow-up).

## Where relay 10 stopped (2026-11-08 ~14:30)
Agent id: not visible to the agent (transcript meta "Field maps A, relay 10" if set).
**larmorx worktree** (after 25120b6b; working tree clean):
- **9f488a11**: two captions (the estimators' table said "five oracle runs": the seven estimators
  come from four; the triage note named "relays 6 and 7", internal names).
- **a6cc7fad**: **the three estimator examples drew no image.** Their default view named
  `lx-fmriprep` and `wf-fmriprep`, comparisons over fMRIPrep's files that have no panels (no
  `display`), so the viewer's `render` threw inside its animation frame, silently. The site's
  viewer check passed all the same (it checks only comparisons with images). Now the default is
  `lx-sdc`, `lx14-lx1`, `fix-lx`, and `_check_default` refuses an export whose default names a
  missing panel or a comparison without images.
- **4f0fcc85**: `validation/investigations/sdcflows-fieldmap/site_checks.sh` (numbers, viewers,
  `drawn`: every viewer loaded and no canvas left at the browser's default 300 x 150; element
  screenshots) + README row. Note: shoot.mjs puts Chrome's profile in TMPDIR; a long TMPDIR (the
  scratch path) makes Chrome's socket path too long and Chrome does not start.
- **0da6be22**: two panel captions without Markdown code marks (panel captions are plain text).
- **030dc188**: the estimators' table fits a desktop frame (it was 1,175 px in a 1,016 px frame,
  its verdicts out of view; no table of any other page overflows at 1440): larmorx's threads,
  modes and sdcflows' step in one column "Threads, modes, step" that names any that differs;
  each row's grid once (it was repeated).
**Site clone** `<scratch>/site`, branch **fmap-a-pages** on b9d118cc (the site's main; unchanged
since): 1663e0ec (relay 9's JSON) + **3d286c32** (the page `content/tools/sdcflows-fieldmap.md`
and the JSON from a6cc7fad) + **7130949f** (the JSON re-exported from 030dc188, `--no-compute`:
the table, the captions, the exporter fingerprint; no blob changed). **evidence-blobs 96bca689**
(local branch in the clone, `tools/blobs.py commit`: 1275 blobs = 1236 + 39 new, none replaced
or removed; manifest SHA-256 8565bf68d65fe9ca; no-reply identity). The orchestrator pushes
evidence-blobs first, then main. Logs: `oracles/logs/fmap-a-10-export{1,2,3}.log`; the checks'
outputs in `<scratch>/r10/` (site-before/, inv-before/, inv-after/, shots/, checks/, union/;
shots-old*/ the superseded shots). NOTES before this relay: `<scratch>/r10/NOTES.before-relay10.md`.

The page `/tools/sdcflows-fieldmap/` (models: resample-series, fsl-prelude): facts (port of
sdcflows 2.17.1 with scikit-learn 1.9.1, SciPy 1.18.1, numpy 2.5.4 / OpenBLAS 0.3.34, nibabel
5.4.2; validated; record, findings, benchmark, API), what it is (field maps, init_phdiff_wf,
BSplineApprox, fMRIPrep's and sdcflows' unwarping, a `lx.sdcflows` example, how the port is exact:
SciPy's sparse storage, sparse_cg/cg, OpenBLAS's sdot order, nibabel's x87 scale factors and
numpy's SVD/eigh quaternions; threads; the stored matrix at one thread), the agreement chart (37:
33 bit-identical, 4 d13), the three estimator examples + table `estimators`,
`coefficients-ds005454`, `apply-ds005454` (SDC-02 on a real call: field 3.65 Hz RMS, series 8.73
RMS), `sdc10-ds002603` + table, table `sdc06`, corrections (8 rule A switched, 2 rule B, SDC-03
waits for FreeSurfer) + table, the step (381 lines, 4 changed, LAPACK-3.12.1) + table, speed
(chart; 6.5-7.8x at 14 threads, 1.09-1.14x at one in a fresh process, ApplyCoeffsField 1.7x),
memory (22-32 % of sdcflows') + table, random testing (trial + triage tables), every case, known
limits (part B; SDC-03; the one-thread stored fit's speed after earlier calls in a process).

Checks (on the final page and JSON, 7130949f):
- `tools/build.py` 0 problems, 29 pages, 167 links into the main repository checked against a
  union of larmorx main (ec557ff5) and this branch (`<scratch>/r10/union`: `git archive` of both
  and an empty `.git`, so the build checks it; against the worktree alone 2 problems: 3dvolreg's
  clean-room files, on main only).
- Before/after: `_site` of b9d118cc against the branch: every existing page's HTML byte-identical,
  sitemap.xml and blobs.sha256 only add (1 URL, 39 lines); `site_inventory.py` (cleanroom-bet) on
  the 23 existing evidence pages: 0 losses, every count identical. The new page: 6 examples
  (9/9/9/6/6/6 panels, 24/24/24/6/6/6 arrays), 37 cases, 8 speed rows, 19.31 MB (budget 40).
- Number check 158 of 158 agree; viewers' check 62, none failing; `drawn`: all 6 viewers drawn.
- Screenshots 1440 and 390, light and dark (`<scratch>/r10/shots/`): the top and 17 components
  (the six examples, the agreement chart, the eight tables, the speed chart, the case explorer),
  each scrolled into view first; looked at: every example (both widths, both themes among them),
  the chart, the speed chart, the estimators', SDC-10's, corrections', step's and memory tables,
  the case explorer. At 390 the wide tables scroll inside their frames, as on every page; a tall
  element photographed on the phone repeats its top below its end (the capture beyond the
  viewport, not the page).
- larmorx side: ruff check/format, check_spdx, `tables --check`, `pytest validation/tests -k
  "evidence or sdcflows or summary"` (47 passed).

## Next relay (11), in this order: the handoff and the part's final report
1. `<scratch>/HANDOFF.toml` (`scripts/orchestration/apply_handoff.py --check`, with `--site
   <scratch>/site`) and `<scratch>/HANDOFF.md`, from `<scratch>/HANDOFF-register-draft.md` and
   "Next relay (9)" item 3: the register's SDC family (SDC-01..11; SDC-03 not switched yet) and its
   legend row; `upstream.tsv` rows (sdcflows 2.17.1 and 2.15.1, scikit-learn 1.9.1 / 1.6.1, SciPy
   1.15.3 tags); `validation/tools.toml` entry (status `validated`, the record, the page, run
   `tables`); `docs/licensing.md` (sdcflows Apache-2.0, a port in the main package);
   `docs/validation/golden.md` (16 cases); `docs/next-steps.md` (part A done; follow-ups:
   `lx.transforms.ensure_positive_cosines`' vnl SVD, measured first; the stored transpose kept only
   for tests; a clean-room original later; the one-thread stored fit's speed after earlier calls in
   a long-lived process (the benchmark's footnote 1), before larmorprepx; the site's viewer check
   misses a viewer that never draws (`site_checks.sh drawn` shows how; a check in the site's own
   `checkViewers` would cover every page); the two oracle-cache tests failing only under `-n 8`).
   The site's shared files: `site.yml` tools entry after the resampler, e.g. `- { title: sdcflows'
   field-map estimation, url: "/tools/sdcflows-fieldmap/", eco: larmorx, summary: "sdcflows'
   B-spline field fit, the field evaluated and applied, and the phase-difference preparation: a
   port of sdcflows 2.17.1, fMRIPrep's seven phase-difference estimators bit for bit, the fit 6.5
   to 7.8 times faster at 14 threads. Port." }`; `content/roadmap.md` (row 1 "Field maps": part A
   done, part B next; a completed row with the page's link); `data/status.yml` (generated from
   tools.toml); `static/evidence/summary.json` (`evidence summary` at the merge; summary.py's PAGES
   has the page since relay 9).
2. The part's agents' ids (top of this file; relays 9 and 10's from the orchestrator).
3. The final report as the brief's "The final report" asks (the port, each record, fMRIPrep's
   calls, the step from fMRIPrep's sdcflows to the newest, defects with rules, golden, memory, the
   trial, speed, what was run, gates, commits, the site's checks, the handoff files, open
   questions), with: relay 8's change of the stored matrix's default to one thread (confirmed); the
   two oracle-cache disputes of relay 6; the LAPACK step; PRELUDE's page's 3 T (fixed by the
   orchestrator, larmorx ec557ff5); the xdist-only oracle-cache test failures; relay 10's viewer
   bug and the `drawn` check.

## FROM THE ORCHESTRATOR (2026-11-08 11:25, after relay 10)
- Relay 10's agent id: a1349b2efb577f2fe. Its plan for relay 11 (the handoff and the part's final report) is agreed.
- **main has moved:** larmorx main is now ec557ff5 (3dvolreg's clean-room original merged: its handoff changed
  `validation/tools.toml`, the register's summary rows and D13 paragraphs and counts line, `docs/validation/golden.md`,
  `docs/next-steps.md`, `docs/status.md`, `docs/api/upstream-defects.md`, the decision log; and the audit's code),
  the site's main is 69662a86 (`data/status.yml`, `site.yml`, `content/roadmap.md` changed; PRELUDE's page relabelled).
  Write HANDOFF.toml's `old` texts against a **trial merge** of this branch with main ec557ff5 (and of `fmap-a-pages`
  with the site's 69662a86), in scratch worktrees only (never merge into this branch or main), and check
  `apply_handoff.py --check --site` there; list in HANDOFF.md every conflict the trial merge meets and how you would
  resolve it (the shared registries: keep both sides).
- The site's viewer check missing a view that draws nothing (relay 10's find): name it in the report as a follow-up for
  the site's own checks (with your `site_checks.sh` `drawn` check as its model).
- The part's agents for the audit's archive: relays 1-10 (ad5d6c85d74dc98ba, a52fb97af12fd7a98, ab2c4c87b023ea94e,
  aed354163dc359170, add71b54eddc33c55, a12f04869a7af7743, abf1e3da6428896bb, a3f6fcb919202dd47, a7b32ff0ee5bd279b,
  a1349b2efb577f2fe) and yourself.

## Where relay 11 stopped (2026-11-08 ~12:00): the part is complete
larmorx worktree: **eca11c3a** (the findings doc: SDC-11 was 10 of the trial's 14 differences, not
14; no other change of the branch). Working tree clean; no Rust changed since a31e70bb (relay 8's
full gates); `tables --check` and `check_spdx.py` pass on the branch.

The handoff (in `<scratch>`): **HANDOFF.toml** (48 operations, 17 files: validation/tools.toml, the
register (11 summary rows, the SDC section, the counts 432 / (c) 216 / sixteen with a second class,
SDC-03's **switch**, the D13 paragraph; PIPE-14 kept by rule C with sdcflows' warning, NP-02 and NIB-04
updated), docs/api/upstream-defects.md (10 rows, a paragraph), upstream.tsv (5 rows), UPSTREAM.md,
NOTICE (sdcflows, scikit-learn, numpy, LAPACK in full; SciPy, OpenBLAS, nibabel extended),
docs/licensing.md (2 rows), golden.md (262 cases, 2,053 outputs, the recording, the I/O and lx.sdcflows
rows), difftest.md, next-steps.md (the order's (6) done and (7) named, item 8, a follow-ups block),
status.md, README.md, CLAUDE.md (Where things are; rule 2's pins), decision-log.md (a proposed entry),
site.yml, data/status.yml, content/roadmap.md) and **HANDOFF.md** (the reasons, the conflicts, what to
verify, the audit's note, the agents). Generator: `r11/make_handoff.py <merge> <site-merge> <out>` (reads
every `old` text from the trial merge's files).

The trial merges (scratch worktrees, to remove: `git worktree remove` from the larmorx repository for
`r11/merge`, from the site clone for `r11/site-merge`; site-merge's `static/evidence/blobs` is a symlink
to the clone's blobs): larmorx main ec557ff5 + the branch (commits dbb85d51, ffadfef9; three conflicts,
both sides kept: golden.json's header, test_memory_peak.py and triage.toml appended at the same place)
with the handoff applied, `tables --site`, the evidence summary; the site's 69662a86 + fmap-a-pages
(no conflict) likewise. Checks there: `apply_handoff.py --check --site` and the apply: 48 of 48;
`tables --check` exit 0; `register_counts.py` 432 entries, no decide/yes; `evidence summary` (the page:
37 cases, 33 bit-identical, 4 d13); `tools/build.py --main-repo r11/merge`: 29 pages, 0 problems, 167
links; before/after builds (`r11/site-before`, `r11/site-after`): every page only gains. The merged
wheel (`r11/build_and_test_merge.sh`, the worktree's target as cache, 1 min) and the gates' pytest
serially (`r11/serial_tests.sh`, log `oracles/logs/fmap-a-11-merge-serial.log`): 1,106 passed, 15
skipped; ruff check/format and SPDX pass. Under `-n 12` (log `fmap-a-11-merge-test.log`) one more
xdist-only failure, test_difftest's memory-limit night (passes alone and serially): in next-steps'
harness item with the two oracle-cache ones. Previews: `r11/shared-preview.diff`,
`r11/site-shared-preview.diff`. NOTES before this relay: `r11/NOTES.before-relay11.md`.

