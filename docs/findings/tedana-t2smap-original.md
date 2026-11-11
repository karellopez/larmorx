# tedana's t2smap, the clean-room original: findings

Written by the clean-room implementers of `specs/t2smap.md` (the original in
`crates/larmorx-tedana`, 2026-11-11). Every statement here comes from the spec, tedana's
documentation, the papers the spec names, black-box runs of tedana 26.0.3's program (through the
oracle-output cache), of the replica's program (`larmorx-gpl` 0.1.0rc1 from PyPI, a black box), of
SciPy and numpy (permissive libraries, run as black boxes) and nibabel 5.4.2's MIT source: no
tedana source and no replica code were read. Unrestricted. The tools behind them are in
`validation/investigations/t2smap-original/` (its README) and the parity suite
`tedana-t2smap-original` (`validation/src/larmorx_validation/parity/tedana_t2smap_original.py`;
the record `docs/validation/tedana-t2smap-original.md`).

## What the original computes differently (and why the results stay within §17.3)

- **The log-linear fit per voxel in closed form** (§7.2, §16): the centred 2 × 2 normal equations
  from the per-echo sums of the observations, lstsq's rank rule for equal echo times (the
  minimum-norm solution), in float64, in any voxel order. tedana's batched SVD of each group gives
  other last bits (and depends on its BLAS threads, §13.4); the original gives the one-thread
  result at any thread count. On fMRIPrep's ds003592 call with `--fittype loglin`, `reproduce`
  against tedana: every value identical but 1 of 10,850,000 combined values (one float32 unit).
- **The curve fit is its own trust-region reflective solver** (§7.3, §16): SciPy's method as its
  documentation and the main package's SciPy port define it, with the model's own derivatives and
  the problem reduced to the per-echo means (the cost ½(T Σₑ(pₑ − μₑ)² + C), so every evaluation is
  O(echoes), not O(echoes × volumes)); the covariance from the final Jacobian. On fMRIPrep's four
  curve-fit calls the share of fit voxels judged by §17.3's objective criteria (b)/(c) rather than
  by value is 0 to 0.305 % (`reproduce` against tedana), the rest within the value thresholds.
  On 1,500 noisy random voxels against the SciPy port (bit for bit SciPy 1.18.1, forward
  differences): 1,367 by value, 5 by the objective (0.33 %), 0 failing, 128 failing in both.
- **A robustness choice (D11) in the curve fit**: a trust-region step that cancels a parameter onto
  its lower bound to within 4 ulps of the parameter is taken as crossing the bound, so the reflected
  and anti-gradient steps are weighed. SciPy's in-or-out decision there rests on its last bits.
  Found on noise voxels outside the head of a probe case (S0 about 1e28, T2* about 0.19 ms, echo
  times 0.05 ms apart): without it the original collapsed S0 to one ulp; with it the voxel is
  SciPy's (2 evaluations, xtol). Results stay within §17.3 (the orchestrator, 2026-11-11).
- **`reproduce` is held to §17.3's thresholds for the fitted numbers** (the orchestrator,
  2026-11-11, as for 3dvolreg's original): the TED defects' behaviours (what is computed, refused
  or written) are exact; the fits are the original's solvers'. The replica is the exact one.
- **Every NaN written is x86-64's default NaN** (the orchestrator, 2026-11-11): the same bits on
  every platform; `check.py` compares NaN with NaN.

## Behaviours observed and followed (black-box, against tedana and the replica)

- **A NaN observation of a voxel with one good echo makes only that voxel NaN** (T2* 1 NaN), unlike
  an infinite one, which poisons its group's batch in tedana (TED-09).
- **tedana chains two tracebacks** for a 3D echo file (`Expected 4D image` twice) and for a mask of
  another grid (`ValueError: Image/mask shape mismatch`, then "During handling of the above
  exception, another exception occurred:" and nilearn's `Mask shape: ...` error); the original
  prints the chain in both modes. **The replica prints only the last traceback** (a replica gap,
  for the replica-side relay).
- **The output directory is made before `--exclude` is parsed and left empty when it fails** (§3
  step 2; the spec writer checked by running tedana directly). The oracle-output cache stores files,
  not directories, so a replayed run shows no empty directory; the suite makes it again from the
  spec. A permanent case checks the directory itself (`crates/larmorx-tedana/tests/run.rs`,
  `exclude_errors_leave_the_output_directory_empty`).
- **SciPy's curve-fit failures, in order** (§7.3): a non-finite sample; a start outside the bounds
  (−∞ and NaN included); residuals not finite at the start; a T2* start of +∞ (β₁ = +0: SciPy's
  forward differences are NaN there and its SVD refuses them, `ValueError: array must not contain
  infs or NaNs`; probed on SciPy 1.15.3, `scipy_edges.py`); no convergence in 200 evaluations. A
  start within 1e-10 · max(1, |lb|) of a lower bound moves to lb + 1e-10 · max(1, |lb|) (either
  parameter); a T2* start of 1e300 is no failure (SciPy: T2* stays, S0 moves, 2 evaluations).
- **A voxel with no sample** (TED-16, or TED-13 in `reproduce`) stops the curve fit after the
  log-linear fit's warning: `ValueError: zero-size array to reduction operation minimum which has
  no identity`; with `loglin` and no dropout method the maps are 500 ms and S0 = 1 (exit 0).
- **Every volume a dummy scan** (TED-13 in `reproduce`, no dropout method): tedana's table of no row
  stops the run after the S0 image: `ValueError: Shape of passed values is (0, 3), indices imply
  (0, 9)`; with `--fitmode ts` the 4D S0 image of 0 volumes is written first.
- **The curve fit's messages**: SciPy's OptimizeWarning once, then with `--debug` the failure line
  f/N per volume and group in tedana's order; group 2's count counts the voxels with one good echo
  in both f and N (e.g. `54/122`).
- **TED-08 is named once per run**: the failures text when fits of voxels with one good echo
  failed, otherwise (with `--verbose`) the variances text, counting the converged fits of those
  voxels; never both (the replica's form).
- **numpy's warnings** (observed, now printed as tedana prints them): `invalid value encountered in
  subtract` (PAID with infinite samples; nanstd's and the 100th percentile's with an infinite
  RMSE), `invalid value encountered in reduce` (the adaptive mask's means and PAID's with both
  infinities), `overflow encountered in exp` (S0 = exp(β₀) beyond float64), `overflow encountered
  in cast` (float64 maps written as float32). **Python's warnings registry is reset** between the
  S0 image and the limited S0 image and before the combination (a warning of one place shows
  again there); the original resets after the confounds table (where exactly is not observable).
  The replica does not print two of these (`x-underflow-debug`, `x-inf-both-paid`: a replica gap).
- **SciPy's internal floating-point warnings** in degenerate fits (`overflow encountered in scalar
  multiply`, `in dot`; `invalid value encountered in subtract`, `in multiply`) are not reproduced:
  the solver is the original's own, and §12.3 lists only the OptimizeWarning for the curve fit.
- **ANALYZE 7.5 and MGH/MGZ** are read as nibabel 5.4.2 loads them: every non-NIfTI `.hdr`/`.img`
  as SPM2's ANALYZE (byte order by nibabel's guess, its header fixes, SPM2's scaling in float64:
  `funused1` and `funused2` when the slope is finite and not 0, else the gl/cal ranges' ratio; the
  origin affine; a `.mat` sidecar); a NIfTI pair's scaling is float32 arithmetic (observed: 664 of
  10,752 values would differ in float64, and none differ). **The replica refuses both formats**
  (`larmorx.UnsupportedFormat`), a replica gap; the original reads them as tedana.
- **TED-03's two computed-mask cases** (an image whose one-voxel planes the erosions remove; a
  1 × 1 × 1 series) and TED-18 with one positive limited value: observed and equal to tedana's
  (`reproduce`) and the replica's (default).

## The spec's checker and TED-05 (settled by the spec writer, 2026-11-11)

In the default the full T2* map is capped too (TED-05), so the first `check.py` found no capped
voxels in (B) and evaluated the objective at the capped, written T2*: on fMRIPrep's calls (B)
failed at 11 voxels (ds003592) and 1 (ds000258) for that reason alone. The spec writer changed
§17.3's cap row and `check.py` (each run's cap read from its files; a voxel at its cap judged on
S0 and by the least objective over T2* ≥ 10c); every earlier verdict stands and those three runs
pass. The original's code was not the cause.

## Speed and memory (fMRIPrep's calls)

The original is I/O-bound: on ds003592's curve-fit call (64 × 64 × 43, 200 volumes, three echoes)
at one / eight threads, reading the echoes 1.62 / 0.92 s (inflating is sequential per file), the
log-linear fit 0.10 / 0.02, the curve fit 0.09 / 0.014, the RMSE, its map and the table
0.44 / 0.32, the combination 0.10 / 0.06, writing the combined series 0.69 / 0.32 s (relay 2b, a
temporary phase timer, not committed). Peak memory 307 MiB at one thread, 546 MiB at eight
(three echoes decoded at once) against tedana's 1,244 MiB (`tests/parity/test_memory_peak.py`).
The wall times against tedana and the replica: `docs/benchmarks/tedana-t2smap.md`.

## For a spec writer (open points)

1. §12.3's table could add the numpy warnings above, the TED-13 table error's text, and say that
   SciPy's internal floating-point warnings in degenerate fits are not part of the contract.
2. §12.3's "once per place": the two registry resets above (where exactly, inside tedana, is not
   observable from outside: also consistent with a reset at every image write).
3. §4.1: SPM2's scaling is float64 arithmetic while a NIfTI pair's is float32 (both observed).
