# tedana's t2smap, the clean-room original: findings

Written by the clean-room implementers of `specs/t2smap.md` (the original in
`crates/larmorx-tedana`, 2026-11-11). Every statement here comes from the spec, tedana's
documentation, the papers the spec names, black-box runs of tedana 26.0.3's program (through the
oracle-output cache), of the replica's program (`larmorx-gpl` 0.1.0rc1 from PyPI, a black box), of
SciPy and numpy (permissive libraries, run as black boxes) and nibabel 5.4.2's MIT source: no
tedana source and no replica code were read. Unrestricted. The tools behind them are in
`validation/investigations/t2smap-original/` (its README) and the parity suite
`tedana-t2smap-original` (`validation/src/larmorx_validation/parity/tedana_t2smap_clean_room.py`;
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
  or written) are exact; the fits are the original's solvers'. The replica is the exact one. The
  one exception (relay C4, 2026-11-12, below): TED-46's voxels, where tedana's values are its
  rounding noise, are computed with the main package's exact ports of numpy's `lstsq` and SciPy's
  `curve_fit`.
- **Every NaN written is x86-64's default NaN** (the orchestrator, 2026-11-11): the same bits on
  every platform; `check.py` compares NaN with NaN.

## Behaviours observed and followed (black-box, against tedana and the replica)

- **A NaN observation of a voxel with one good echo makes only that voxel NaN** (T2* 1 NaN), unlike
  an infinite one, which poisons its group's batch in tedana (TED-09).
- **tedana chains two tracebacks** for a 3D echo file (`Expected 4D image` twice) and for a mask of
  another grid (`ValueError: Image/mask shape mismatch`, then "During handling of the above
  exception, another exception occurred:" and nilearn's `Mask shape: ...` error); the original
  prints the chain in both modes. The replica of 0.1.0rc1 printed only the last traceback; the
  replica built from this branch prints tedana's chain (2026-11-12: `three-d` and `x-mask-grid`
  pass (B) in the record).
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
  The replica of 0.1.0rc1 did not print two of these; the replica built from this branch prints
  both (2026-11-12: `x-underflow-debug` and `x-inf-both-paid` pass (B) in the record).
- **SciPy's internal floating-point warnings** in degenerate fits (`overflow encountered in scalar
  multiply`, `in dot`; `invalid value encountered in subtract`, `in multiply`) are not reproduced:
  the solver is the original's own, and §12.3 lists only the OptimizeWarning for the curve fit.
- **ANALYZE 7.5 and MGH/MGZ** are read as nibabel 5.4.2 loads them: every non-NIfTI `.hdr`/`.img`
  as SPM2's ANALYZE (byte order by nibabel's guess, its header fixes, SPM2's scaling in float64:
  `funused1` and `funused2` when the slope is finite and not 0, else the gl/cal ranges' ratio; the
  origin affine; a `.mat` sidecar); a NIfTI pair's scaling is float32 arithmetic (observed: 664 of
  10,752 values would differ in float64, and none differ). The original and the replica built
  from this branch both read them as tedana does (2026-11-12: the record's 12 ANALYZE and MGH
  cases pass (A) and (B), every value identical); only the replica of 0.1.0rc1 (PyPI) refused
  both formats (`larmorx.UnsupportedFormat`).
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

## Relay 3c (2026-11-11): the spec's points of spec writer 2, two defects, the second trial

- **nilearn's second reading (§4.4)**, probed again (`validation/investigations/t2smap-original/
  mask_probe.py`, 22 cases, tedana 26.0.3 through the oracle-output cache): tedana's first reading
  checks the echoes in order, each one's grid against the mask's shape and then its four axes, so
  the first error is `Image/mask shape mismatch` (a 4D mask of one volume fails at the first echo;
  an echo both 3D and of another grid too) or `Expected 4D image`; any of them sends every echo
  through nilearn's masking, whose checks come per echo, in nilearn's order: the mask's values
  (an empty mask's `The mask is invalid as it is empty: it masks all data.`), the affines, the
  shapes, then tedana's own refusal of a 3D echo (a 3D first echo stops before a second echo of
  another grid is looked at). A 4D mask of one volume holding other values than 0 and 1 (0/1/2,
  1/2, 0/0.5, −1/1) is read as tedana reads it (its `> 0`), with no nilearn refusal: the values
  check concerns only an empty mask. The original had read such masks on (the trial's findings
  130, 141, 162, 222, 242); it now follows tedana in both modes, and with it the default's TED-13
  check comes after these errors, at step 12.
- **No voxel left for the fit (TED-31, new; rule B).** When the adaptive mask removes every voxel
  (here every first echo ≤ 0: §6.1's base count 0), tedana writes the adaptive mask image, prints
  `Computing T2* map`, checks the echo times' count, and fails with `IndexError: index 0 is out of
  bounds for axis 1 with size 0`, with both fit types and both fit modes (observed, after §6.5's
  warnings when `--n-independent-echos` is given). The input holds no estimate to make, and the
  failure is an unrelated numpy error: by default a clear `ValueError: no voxel of the mask has
  good signal (the adaptive mask removed all <S>), so there is nothing to fit (tedana fails with
  IndexError)`, notice `no voxel is left for the fit (the adaptive mask removed all <S>)`;
  `reproduce` gives tedana's error. Black-box facts of the replica (larmorx-gpl 0.1.0rc1) for the
  replica-side relay: its default completes the log-linear fit's run (every map 0, the RMSE NaN,
  exit 0) and stops a curve-fit run with nilearn's empty-mask text, no notice.
- **Equal observations (TED-46, the orchestrator's decision under rule A, 2026-11-11).** Where a
  voxel's log-linear observations are all equal (in a design of two distinct echo times or more)
  the original takes the least squares' exact solution (y, 0) in both modes, so T2* = +∞, numpy's
  `divide by zero` warning, then §8's floor: 500 ms (TED-05 then caps the full map in the default
  where 500 ms lies above ten caps). Observations all 0 are not TED-46's: tedana's slope there is
  an exact 0 too (a zero right-hand side). The default names the voxels counted at the first place
  that holds one (the first group; in `ts` the first volume): `<k> voxel(s) whose log-linear
  observations have no slope fitted with the slope 0 (no decay, T2* 500 ms); tedana's <n>-echo
  least squares leaves rounding noise there (T2* 1 ms, or 1e16 ms and more)`. `reproduce` kept
  the exact zero (its notice said tedana's noise was not computed yet) until relay C4
  (2026-11-12, below), which computes it with the main package's exact port of numpy's
  `lstsq`.
- **Warnings (§12.3, §6.5):** the temporal means of no volume warn `Mean of empty slice` alone when
  the mask has no voxel; with no fit voxel `invalid value encountered in scalar divide` comes once
  for each of §6.5's two percentages (two places: twice when k > 3) (both observed).
- **The RMSE map's voxels without a value** are written as numpy's `nan` (`0x7FC00000`, §9, §13.1),
  the one NaN the original does not write as x86-64's default NaN (golden `zcat-int16`'s RMSE
  checksum changed with it).
- **Notices' numbers (§15):** TED-09's and TED-15's count the first place that holds such voxels
  (TED-09: in `ts` the first volume, then the first echo-count group; TED-15: the first of the
  fit's echo-count groups, ascending, count 1 with count 2: observed against the replica, 829
  voxels of count 1 where the run had 831, 1 voxel of count 3 where it had 181, 98 = 16 of count
  1 and 82 of count 2 in a two-echo run).
- **TED-01 is a NaN mean's** (the trial's findings 455, 588, 761): the default had dropped every
  non-finite first-echo mean from the dropout percentile, so an infinite sample changed the
  thresholds and named TED-01; tedana sorts and matches an infinite mean without failing, and the
  replica's default keeps it (observed). The default now drops the NaN means only (the spec's
  §15.1 row said "finite"; spec writer 3 made it "not NaN", 2026-11-12).

## Relay C4 (2026-11-12): TED-46 in `reproduce` and in the curve fit, the spec writers' answers

- **TED-46 in `reproduce`: tedana's rounding noise, computed.** A voxel whose log-linear
  observations are all equal has the exact slope 0, but tedana's batched least squares leaves
  noise there whose sign depends on the voxel's place in its group's batch (§13.4). `reproduce`
  now solves each group (and volume, in `ts`) that holds such voxels as tedana does: the m × 2
  design (rows (1, −TE), echo by echo) and one right-hand side per voxel of the group in voxel
  order, through the main package's exact port of numpy's `lstsq`
  (`larmorx_transform::numpy_linalg::lstsq`, LAPACK 3.12.1 or 3.12.0 as `LARMORX_TEDANA_LIBRARIES`
  names numpy's release), and takes its solution at those voxels only (every other voxel keeps
  the closed form). The whole batch is solved because a column's last bits depend on where it
  stands in it (the kernel class of the batch's last columns, §13.4). Memory: the batch, m × N
  float64, only in such a group in `reproduce` (none of fMRIPrep's calls holds one). The notice
  no longer says the noise is not computed.
- **TED-46 in the curve fit, `reproduce`: SciPy's own fit, through its exact port.** From
  tedana's start at those voxels (T2* about 1e13 s, a positive noise slope, in the suite's cases),
  SciPy's forward difference along T2* (a step of √eps · T2*) changes the model S0 · exp(−TE /
  T2*) by less than half a unit in the last place of float64, so its Jacobian's T2* column is
  exactly 0 and only S0 moves (to about x + 0.5, the spec's measurement). The original's own
  derivatives are not 0 there, so its trust-region fit went on along T2* to an objective of
  about 0 (T2* 8.4e8 s instead of tedana's 2.7e13 s): the suite's `x-ted46-curvefit-ts` failed
  (A) on §17.3's (b)/(c) share (108 of 3,216 fits, 3.36 %) and on the run's cap (one of those
  values; the caps 2.7e13 s and 8.4e8 s apart). `reproduce` therefore fits TED-46's voxels with
  `larmorx_optim::curve_fit2` (SciPy 1.18.1, or 1.15.3 with `shared`), the model with glibc's
  exponential, from tedana's start; every other voxel keeps the original's solver.
- **TED-46 in the curve fit, the default** (spec writer 4's measurement, §7.3's last paragraph):
  where the samples themselves are all equal (x), the exact solution S0 = x, T2* = +∞ (500 ms
  after §8's floor), not a failure (0 in the failures map, left out of the DEBUG count), its
  variances and covariance 0 when m = n · T_u > 2, +∞ with SciPy's `OptimizeWarning` when m ≤ 2
  (two echoes in `--fitmode ts`). Observations equal with samples that are not (x = ±c) keep the
  start +∞ and its failure.
- **The result**, value by value at the 18 voxels the suite's TED-46 inputs saturate at 255 in
  every echo and volume (`validation/investigations/t2smap-original/ted46_values.py`; cases
  `x-ted46-loglin`, `-loglin-ts`, `-curvefit`, `-curvefit-ts`, `-curvefit-ts-2e`): every map
  (T2*, S0, the limited maps, the failures, both variances, the RMSE) identical to tedana's in
  `reproduce`, and the default's identical to the replica's default (larmorx-gpl built from the
  same branch). tedana's T2* there: 7.3e13 s (all mode), 2.7e13 s per volume (ts), 1.6e14 s with
  two echoes; the default's 500 ms.
- **TED-46's notice counts equal observations only** (spec writer 3): an exact zero slope from
  distinct observations (rare arrangements, §7.2) is the closed form's own value (judged by
  §17.3), no longer counted.
- **TED-01's texts** (spec writer 3): the notice reads "not NaN"; with no first-echo mean that is a
  number the default's error comes with the notice `no voxel of the mask has a first-echo mean
  that is a number` (a correction that changes the message is named).
- **`--fitmode ts` with no volume** (every volume a dummy scan, TED-13 in `reproduce`): tedana
  checks the echo times in each volume's fit, so with no volume a count that does not match is
  never met (nor TED-31): it writes the adaptive mask and the S0 map and fails at the confounds
  table (spec writer 3); the original now does the same (`x-dummy-all-none-ts-te2`).
- **MGH's translation is float32** (nibabel 5.4.2's `MGHHeader.get_affine` and
  `affines.from_matvec`, MIT): the translation Pxyz_c − (MdcD · dims) / 2 is computed in float64
  and stored in an array of MdcD's type, float32; the image's affine is that array widened. The
  original kept the float64 value. It shows where nilearn prints an affine in a refusal; the
  outputs' headers store float32 values anyway (golden `mgh` unchanged).
- **The suite against a replica of this branch.** The replica built from the same branch
  (`crates-gpl`, `cargo build --release -p larmorx-gpl-cli`, run as a program) reads ANALYZE and
  MGH, prints tedana's chained tracebacks and both missing warnings, and has TED-31's and TED-46's
  corrections: the suite's expected divergences `three-d`, `x-mask-grid`, `x-underflow-debug`,
  `x-inf-both-paid`, TED-46's (A) and (B) and TED-31's (B), and the handling of the replica's
  refusal of ANALYZE and MGH are gone; every one of those cases passes. (B) is run only against
  such a replica (`LARMORX_GPL_BIN`).

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
