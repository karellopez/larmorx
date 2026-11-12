# Spec writer's notes, 2026-11-11 (replica-side role; the clean-room relays never read this file)

Brief: scratch SPEC-WRITER-BRIEF.md. Worktree agent-ad7e01a7198215645, branch
worktree-agent-ad7e01a7198215645 (start a9ba3851).

## The four points, against tedana 26.0.3's source and program

1. check.py's default (TED-05). Replica: crates-gpl/larmorx-gpl-tedana/src/maps.rs caps every
   full value (count-1 voxels too) > 10c to c when `fix` and c > 0 (TED-18: no cap). check.py
   read capped voxels as limited != full (only tedana's files) and evaluated (b)'s F at the
   written capped T2*. Fix: cap_state() per run (tedana's reading, else "default" when no full
   value > 10c(1+2^-22): at-cap = value == c, c = percentile_lower of the written limited
   values over count>=1), the cap row on both maps, (b)'s F at the cap = min(F(S0,c),
   min_{T2*>=10c} F(S0,T2*)), (a) at the cap = S0 + cap row, variances not judged there.
   Data: sw/analyse-ds003592.txt, sw/analyse-ds000258.txt (35 / 15 voxels at the cap; the
   profile F'/F-1 within 2e-10, the order-stat voxel 53897 / 25170 has F(c) < profile).
   Proof: recheck logs oracles/logs/t2smap-cr-sw-recheck-2.log and -3.log (final checker):
   113 PASS->PASS, 3 FAIL->PASS, 3 ERROR->ERROR. Old checker copy: sw/oldcheck/.
   Known theoretical gap (stated in the spec): in the default's files the order statistic's own
   voxel is indistinguishable from capped ones; excluding it from the lip makes the bound not
   strictly valid; tol*c and (b)/(c) cover it.
2. §7.3 +inf start: tedana's per-voxel fit catches (RuntimeError, ValueError) (decay.py:119);
   SciPy 1.18.1: +inf T2* start -> ValueError "array must not contain infs or NaNs" (raised by
   numpy's asarray_chkfinite inside scipy.linalg.svd on the NaN 2-point Jacobian). Also: the
   bounds error is unreachable (curve_fit's check_finite on ydata first, _minpack_py.py:938);
   make_strictly_feasible(rstep=1e-10) at least_squares.py:908 / common.py:440. sw/inf_start.py
   and the harness's scipy_edges.py run with oracles/python-newest.
   Found on the way: the bounds' np.min of an empty sample is outside tedana's try (decay.py:233,
   inside the Parallel generator) -> uncaught ValueError "zero-size array ..." for TED-16 with
   --masktype none --fittype curvefit (observed directly: sw/excl/out-all-curvefit.log);
   loglin writes 500 ms / S0 1 (sw/excl/out-all-loglin).
3. TED-08: larmorx_core::defects Notices::add -> push_once: one notice per defect; the
   replica adds the failures text (count1_failed > 0) before the variances text (verbose,
   count1_fitted > 0), workflow.rs:762-803.
4. §3: the brief's requested correction is NOT what tedana does. t2smap.py:355-360 makes the
   directory (os.mkdir) before parse_volume_indices; tedana run directly through the launcher
   (sw/excl/out-*: four --exclude errors) leaves the empty directory. The clean-room relays'
   "no output directory" came from compare.py's runs through the oracle cache, which records
   files only (oracle_cache/core.py walks files): an empty directory is not replayed. So: the
   replica (makes it) is right; the original (relay 2a, ec495196: error before the directory)
   now differs from tedana; NOT a replica deviation. Spec §3 kept and made explicit.

## Also stated
- §7.3 DEBUG count: N = len(voxel_idx) (adaptive_mask <= 2 for group 2), f counts all failures
  in it (decay.py:208-263): count-1 voxels in both; observed 54/74 (cmp3 tedana.txt).
- §12.3: the divide-by-zero warning of §7.2 now observed (TED-16 runs).

# Spec writer 2, 2026-11-11 (brief SPEC-WRITER-BRIEF-2.md; same worktree and branch, from 7a2f3b95)

Work folder: <scratch>/sw2/ (replays f<n>/ with tedana, reproduce, default, replica outputs and
model/model2 outputs; scripts degen2.py, signs.py, classes.py, fmri_const.py, dump_groups.py,
model_vs_tedana.py, vox.py; oldcheck/ = check.py + model.py at 7a2f3b95). Logs:
oracles/logs/t2smap-cr-sw2-{replay45,replay-rest,recheck-probe,recheck-trial,build-lstsq}.log.

## 1. Observations without a slope (the trial's 45 "divide by zero" cases)
- tedana (decay.py fit_loglinear): per echo-count group ONE np.linalg.lstsq(iv_arr float64 (m x 2),
  log_data float32 (m x N), rcond=None) -> numpy casts b to float64, one DGELSD call with N RHS
  (umath_linalg gufunc, no loop over columns). t2s = 1/betas[1], s0 = exp(betas[0]).
- All 45 cases: the degenerate voxels are EQUAL observations (uint8 saturated 255 -> y = log 256),
  1,800-odd exact-zero slopes in the completed ones, every one constant (Fraction test of
  m*sum(t*y) - sum(t)*sum(y) == 0). Our numpy reproduction of tedana's batched call gives
  tedana's written T2* at every one (0 mismatches).
- tedana's b1 there: never 0 (2,587 voxels incl. failing cases), 1,881 negative / 706 positive,
  |b1| 4.9e-19 .. 9.5e-17. Negative -> T2* < 0 -> 1 ms; positive -> 1e16..2e18 ms (limited capped
  when > 10c; if > 0.5 % of limited values the cap itself becomes huge: f00869 cap 1.9e14 s).
  Original (closed form, centred): exactly +0 -> +inf -> 500 ms, divide warning; curvefit: +inf
  start fails (SciPy, §7.3 item 4) -> keeps 500 ms / S0 256 where tedana converged (positive) or
  failed (negative, TED-06).
- What decides the noise (from the port's doc, larmorx_transform::numpy_linalg::lstsq, and
  black-box): the column's values, the design (m, TEs), the column's kernel class (the last
  NRHS mod 4 columns: dgemv_t 4x2 / 4x1 kernels vs the Haswell microkernel), ILADLC (trailing
  all-zero columns; impossible here unless |x| < 6e-8 everywhere), LAPACK release (DLARF vs
  DLARF1F), OpenBLAS kernels, BLAS threads (threaded dgemv splits columns).
  Measured sign changes on the 2,587: shared env (LAPACK 3.12.0) 48 (2 cases), SkylakeX kernels
  122 (3 cases), Prescott generic 716 (11 cases), Zen 0, 4 threads 0. Place in batch: 210
  random tedana designs, NRHS 1..8, every column: value changes in 66, sign in 19, never 0.
- fMRIPrep's four calls: 0 equal-observation voxels; min |b1| / (eps*max|y|/TE range) = 7.8e9.
- Cost (shared machine; lstsq_check with LSTSQ_CHECK_TIMES, 548 cases all identical):
  ds003592 main group 52,231 x 600: port 0.489 s at 1 thread, 0.211 s at 8; numpy lstsq 0.33 s;
  numpy closed form 0.04 s. ds000210-rest main group 48,066 x 612: port 0.453 / 0.185 s.
  The original's whole run (docs/benchmarks): ds003592 loglin 3.04 s (1 thread) / 1.52 s (8);
  curvefit calls 2.7-3.7 s / 1.3-2.0 s. So the port on every voxel adds ~15 % at one thread.

### Recommendation to the orchestrator (NOT in the spec)
Options:
(a) Threshold: accept at voxels whose exact slope is 0 (recognised exactly: all observations
    equal, or in general m*sum(t*y) == sum(t)*sum(y) in exact arithmetic over the float32
    observations and float64 TEs, with a cheap prefilter |b1| <= ~1e3*eps*max|y|/range(TE)):
    any of 1 ms / 500 ms / |rate| below the noise bound in T2*, failures and S0 by anything, those
    voxels out of the (b)/(c) share, and the cap judged per run (each run's limited map = its full
    map capped by its own c) since c can move by orders of magnitude (f00422 0.022 vs 0.5 s;
    f00869 1.9e14 s vs 0.55 s), which then frees every other voxel near ten caps. Pros: no code in
    the original. Cons: a complex, weak check (it accepts the cap moving for all voxels);
    reproduce does not give tedana's (deterministic, reference-setup) result there; the default
    keeps whatever its solver gives.
(b) The original uses larmorx_transform::numpy_linalg::lstsq for the log-linear fit:
    - in reproduce only: (A) exact on the whole log-linear fit (a bonus: §17.3's loglin row then
      identical), +0.45 s at one thread (+15 %) on fMRIPrep-sized calls in reproduce; (B) still
      diverges (the default's closed form vs the replica's default = tedana's arithmetic), so (B)
      needs (a) or (c) anyway;
    - in both modes: (A) and (B) exact; the default keeps tedana's machine-dependent noise result
      (1 ms for a no-decay voxel), against D13 (correct by default), +15 % in the default too.
(c) RECOMMENDED: a new defect TED-46 (rule A: the input makes sense, a voxel without decay; the
    result is wrong: 1 ms or ~1e17 ms by the sign of rounding noise that the batch place, the
    LAPACK release and the CPU kernels decide; the correct value is the exact slope 0, T2* = +inf,
    500 ms after tedana's own floor):
    - default (original AND replica): the exact slope of equal observations is +0 (T2* = +inf,
      500 ms; S0 = exp(y)); the original's closed form already does it; numpy's divide warning
      printed as for any slope of 0 (§12.3); notice once per run when such voxels are fitted,
      e.g. `<k> voxel(s) whose log-linear observations are all equal (no decay) fitted with slope
      0: T2* = +inf, 500 ms after the floor; tedana's least squares leaves a slope of rounding
      noise there, whose sign gives 1 ms or a T2* above ten caps` (k summed over groups and
      volumes; named when k > 0: tedana's noise was never 0 on 2,587 voxels and six setups);
    - reproduce: tedana's noise exactly: the port on the whole batch (simplest; +15 % at one
      thread in reproduce only) or only on the candidate columns at their kernel class (cheaper,
      needs care with the class rule); then (A) is exact there;
    - (B): both defaults give +0 -> agree; the replica needs TED-46 in its default (replica-side
      relay; its reproduce unchanged);
    - the general exact-zero case with distinct observations (symmetric arrangements; finding 20
      of the dry run, none in the trial): keep it outside TED-46 (rule C/D: measure first) and let
      §17.3 accept it by an exactly-recognised rule (the Fraction test), or include it in TED-46
      with an exact test in the default (cheap: only candidates with tiny |b1|). My preference:
      equal observations only in TED-46 (robust, trivially exact), the symmetric coincidence
      accepted by §17.3 with the exact test.
    Trade-off summary: (c) is the only option where both modes are right by D13 (default correct
    and machine-independent, reproduce exact), at the cost of one defect row, a notice, the
    replica's small fix and the port in reproduce (+15 % there). fMRIPrep's calls are unaffected
    in every option (no such voxel).

## 2. The checker reads every format
- model.load_inputs: as_nifti1 (tedana's conversion: NIfTI-1/2 single files kept, anything else
  an in-memory Nifti1Image of np.asanyarray(dataobj) with affine and zooms; the mask with
  max_dim=3 squeeze); the fast reading (shape check, float32) else nilearn's apply_mask
  (non-finite -> 0) like tedana's fallback; the outputs' class from the mask image (NIfTI-2 for a
  mask computed from a NIfTI-2 echo); images written on the mask's 3 spatial axes.
- Found on the way, model fixes (spec text already said them or now says them): the RMSE map's
  empty voxels are numpy's +NaN (0x7FC00000), not 0/0's default NaN; the table's rmse_mean with
  ONE volume is a pairwise sum over the voxels (§13.2's "with one volume every sum is pairwise").
- Proof: recheck-probe (cmp2..cmp5; cmp = relay 2a's program, call file differs everywhere):
  116 PASS->PASS, 17 ERROR->PASS (ANALYZE, MGH, pairs, 4D masks), 1 FAIL->FAIL (cmp5 underflow
  (B), unchanged), 1 ERROR->ERROR (TED-01 NaN (B): outside the model by design); no PASS->FAIL.
  recheck-trial (the trial's 328 replayed cases, (A) and (B)): 38 PASS->PASS, 265 ERROR->PASS,
  21 ERROR->FAIL (newly judged: the degenerate voxels' share/cap rows and variance-map
  tolerances: the original's, not the readers'), 62 FAIL->FAIL, 250 ERROR->ERROR (tedana or the
  model fails: no outputs to judge); no PASS->FAIL.
  Model vs tedana on the 202 replays tedana completed: see model_vs_tedana3.txt (below).

## 3. Other spec points from the triage (tedana 26.0.3 source + replays)
- Masks of another grid (io.load_data_nilearn): fast path ValueError("Image/mask shape mismatch")
  or "Expected 4D image" -> except Exception -> nilearn apply_mask per echo
  (_convert_to_nifti1(dtype=float32)): check_niimg_3d(mask), load_mask_img (empty -> "The mask is
  invalid as it is empty: it masks all data."), allclose affine ("Mask affine:\n...\n is different
  from img affine:\n..."), shape ("Mask shape: (..)\n is different from img shape:(..)"),
  safe_get_data (non-finite -> 0), m.ndim != 2 -> "Expected 4D image". Chained traceback.
  Original: no shape check (findings 130, 141, 162, 222, 938: IndexError / goes on).
  Replica: one traceback (known gap).
- Empty user mask on the grid: dropout -> IndexError (spec had it); none/decay -> the adaptive
  mask image's masking.unmask -> load_mask_img -> empty-mask ValueError (observed 13, 207, 718,
  1013, 1059). Spec §6.6 now says it.
- TED-13's default check: at step 12, after reading (the replica's order; finding 141: the original
  refuses -1 before reading). §15 row says it.
- §6.5: two places of numpy's scalar-divide warning (lines 213 and 229 of utils.py); the original
  prints one (13, 718, 1013).
- §12.3: "Mean of empty slice" then "invalid value encountered in divide" only when S > 0 (an
  empty array has nothing to divide): tedana prints the first alone for an empty mask (24, 252,
  289, ...: 17 cases); the original prints both.
- Notices: the replica's push_once keeps the first text: TED-15 counts the first group's voxels
  (281: 1 vs 181), TED-09 the first volume's in ts (223: 1 vs 3). The spec's "first of its texts"
  was already the rule; made explicit (first group ascending, first volume in ts). §17.3 (B)
  already reports, not fails, count differences. Option for the orchestrator: whole-run counts
  (more informative; then both implementations change, the replica too). Not done.
- f00162 (TED-16 named by the original only): the original went on past an empty other-grid
  mask (§4.4), not a notice question.

## Final (spec writer 2)
- Model vs tedana: 202/202 replayed completed cases bit-identical (sw2/model_vs_tedana3.txt):
  NIfTI-1 78 (incl. z-cat 11, 4D masks 11), NIfTI-2 78 (6 computed masks -> NIfTI-2 outputs),
  Nifti1Pair 22, Spm2Analyze 18, MGH 34. Two model fixes on the way: RMSE map's empty voxels +NaN
  (tedana: np.full(..., np.nan), decay.py:705) and rmse_mean with one volume pairwise (the (S, 1)
  array's axis-0 reduction is contiguous).
- check.py: + variance_floor (max(P00,P11) m (2^-24 max|y|)^2/(m-2); the 23 variance-only trial
  failures used <= 2.5e-4 of it; never used on fMRIPrep), + caps beyond allowance fail the rows
  (14 PASS->FAIL, all equal-observation cases), + equal-observation note.
- Final recheck (oracles/logs/t2smap-cr-sw2-recheck-final.log): probe 116 P->P, 17 E->P, 1 F->F,
  1 E->E; fMRIPrep 22 P->P, 8 F->F (fmri2 figures/ from the cache); trial 264 E->P, 24 P->P,
  19 F->P, 14 P->F, 22 E->F, 43 F->F, 250 E->E; every trial FAIL (79) has equal observations.
- Commit ca8e98f0 on worktree-agent-ad7e01a7198215645.
- Follow-ups for the orchestrator: (1) the decision on open point 5 (recommendation (c) above);
  (2) whole-run notice counts (TED-15/TED-09) instead of the first place's, if wanted (both
  implementations change); (3) the original's own fixes found (for clean relay 3c, already black
  box facts in the spec now): refuse masks of another grid at step 11 (§4.4), the second
  scalar-divide warning (§6.5), no 'invalid value in divide' with no voxel (§12.3), the RMSE
  NaN fill (§9), TED-13's place (§15), the notices' first-place numbers (§15).
