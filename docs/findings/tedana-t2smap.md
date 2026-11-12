# Findings: tedana's `t2smap` (tedana 26.0.3)

**Restricted: written from tedana's source.** A clean-room implementer must not read this file
(`docs/porting-guide.md` §1); what it learns about behaviour is in `specs/t2smap.md`. The last
section, "Black-box facts", holds only what runs of tedana's program show; the spec writer copies
from it what the spec needs.

What the replica (`crates-gpl/larmorx-gpl-tedana`, the `larmorx-gpl` program, 2026-11-08, from
tedana 26.0.3) found in `t2smap`: how the program works where that is not obvious, where tedana's
code does something questionable, how each case was decided (D13's rules A to D) and what larmorx
does, and the facts of the libraries it reaches (numpy, SciPy, nibabel, nilearn) that the replica
had to match. Each entry says how it is known: *read* (the source), *history* (tedana's
repository, its pull requests and release notes), *measured*
(`validation/investigations/t2smap/`, the parity suite `tedana-t2smap-replica`, random testing).
The defects' register rows are TED-01 to TED-18 (`docs/findings/upstream-defects.md`), with NP-03
and NP-04 (numpy) and GLIBC-07 (glibc's maths, as numpy calls it).

## Versions

- **Ported from tedana 26.0.3** (2026-04-03; tag `26.0.3` = `e0a18e33`; PyPI sdist SHA-256
  `3d767a09...a5b2f`), the newest release, installed in both kept Python environments: the newest
  (numpy 2.5.4 with OpenBLAS 0.3.34, SciPy 1.18.1 with its own OpenBLAS 0.3.31.dev, nibabel 5.4.2,
  nilearn 0.14.1, pandas 3.0.6) and the shared (numpy 2.3.5 with OpenBLAS 0.3.30, SciPy 1.15.3).
  The replica reproduces either (`LARMORX_TEDANA_LIBRARIES=newest|shared`; newest by default).
- **fMRIPrep 25.2.6's image has tedana 25.0.1** (numpy 2.2.6, SciPy 1.15.2, conda-forge's OpenBLAS
  0.3.30): the reference of the final comparisons only (decision D4); no "25" mode. On fMRIPrep's
  path 25.0.1 differs from 26.0.3 in: the data loaded as float64 (26: float32, so the adaptive
  mask's threshold line prints float32 digits), the full T2* map capped (TED-05), `nan_to_num` and
  the clamps of negative values (removed in 26), the T2* floors applied to the limited map only
  (26: the full map), no check of the echo times' units, no `figures/` directory, the RMSE from the
  full maps (26: the limited ones); numpy 2.2.6 also sums arrays of more than 8,192 elements in
  blocks (numpy 2.3 and later in one pass: any sum, mean or deviation over more voxels can differ in
  its last bits).
- **Licence** (*read*): `LICENSE` is the GNU LGPL version 2.1 (502 lines, SHA-256 `d716613a...`),
  with no "or any later version" applied to tedana (the text's "How to Apply" template only); no
  source file carries a header; `__about__.py`: "Copyright 2023, The ME-ICA Developers". So
  LGPL-2.1-only; the replica uses its section 3 (the GNU GPL version 3 instead, for that copy):
  `larmorx-gpl` (`docs/licensing.md`).

## How `t2smap` works, for the replica

### The command line

*Read*: `tedana.workflows.t2smap._get_parser`, argparse with `ArgumentDefaultsHelpFormatter`.

- `-d FILE [FILE ...]` (required; each must exist: "The file X does not exist!", exit 2): one file
  per echo, ascending; a single file is a z-concatenated series (echoes stacked along the third
  axis; deprecated, a warning naming tedana's issue 1313).
- `-e TE [TE ...]` (required): seconds when every value is in (0, 1) (multiplied by 1,000, an INFO
  line), milliseconds when every value is at least 1 (a deprecation WARNING), else `ValueError: TE
  values must be positive and either all in seconds ...` (`utils.check_te_values`).
- `--out-dir` (`.`; created with `os.mkdir`, one level), `--prefix` (`_` appended unless it ends
  with one), `--convention {orig,bids}` (`bids` = BIDS 1.5.0 names, from
  `resources/config/outputs.json`), `--verbose` (curvefit only: the variance and covariance maps),
  `--overwrite`/`-f` (removes the confounds table first; without it an existing output is a
  `RuntimeError: File X already exists. In order to allow overwrite please use the --overwrite
  option in the command line or the overwrite parameter in the Python API.`), `--mask` (none:
  nilearn's `compute_epi_mask` of the first echo, with a WARNING), `--masktype
  {dropout,decay,none} ...` (`dropout`), `--dummy-scans` (0), `--exclude` (`0,5:10,15`: 0-based,
  end-exclusive; negative numbers, steps and open ranges are ValueErrors; not with `--fitmode ts`),
  `--fittype {loglin,curvefit}` (`loglin`), `--fitmode {all,ts}` (`all`), `--combmode {t2s,paid}`
  (`t2s`), `--n-independent-echos` (only two warnings, in the adaptive mask), `--n-threads` (1;
  `threadpool_limits` and joblib's processes for the curve fit; -1: no limit), `--debug` (its help
  promises a TSV log file that is never written: kept, rule C), `--quiet` (hidden), `-v`/`--version`
  (`t2smap v26.0.3`). argparse's abbreviations, `--option=value` and grouped short flags all work;
  errors exit 2 with the usage; any exception exits 1 with Python's traceback.
- The replica's parser is tedana's argparse behaviour line for line: `check_parser.py`, 109 cases
  (the help at 19 widths of `COLUMNS`, errors, abbreviations, clusters, `--`), identical.

### The workflow, in order (*read*: `t2smap_workflow`)

The output directory; `--exclude` parsed; the loggers (stderr, `%(levelname)-8s
%(module)s:%(funcName)s:%(lineno)d %(message)s`, INFO); `t2smap_call.sh` (`"t2smap " + "
".join(argv)`, no newline); the system information; the `--fitmode ts` with `--exclude` error; the
echo times' check; the reference image (the first echo, or the z-concatenated file's first slab);
`OutputGenerator` (26.0.3 creates an empty `figures/` directory: it no longer passes
`make_figures=False`); the mask; the data (float32, masked voxels by echoes by volumes; a fast path
by boolean indexing and, on any exception, nilearn's `apply_mask`; a 3D file is "Expected 4D
image"); the dummy scans; the excluded volumes (offset by the dummy scans; an index above the
number of volumes refused, one equal to it not: TED-02); the adaptive mask, **written at once**
(an existing adaptive mask stops a run without `--overwrite` there, before the fit); "Computing
T2* map" and the fit; for curvefit the failures map and, with `--verbose`, the variance maps;
`modify_t2s_s0_maps`; the maps unmasked; S0 (full); the RMSE; the RMSE map, the confounds table,
the limited S0, the limited T2* (ms / 1000: seconds); the optimal combination with the **full** T2*
over every volume (excluded ones included); the full T2* (seconds) and the combined series;
`dataset_description.json`, the registry; the newsletter's two INFO lines; the loggers torn down.

### The adaptive mask (*read*; `utils.make_adaptive_mask`, threshold 1)

- **base**: per voxel the number of leading echoes with no NaN and no value <= 0 over time.
- **dropout**: the 33rd percentile (`interpolation="higher"`) of the first echo's non-zero temporal
  means; the voxels whose mean equals it are the exemplars (several: the one whose echo means have
  the largest sum); each echo's threshold is the exemplar's mean of that echo divided by 3; per
  voxel the last echo above its threshold. A NaN mean makes the percentile NaN and no voxel equal
  to it: TED-01.
- **decay**: per voxel the first echo whose temporal mean does not decrease.
- The adaptive mask is the minimum over the methods (`dropout` appended twice to the list:
  harmless); voxels below 1 are removed from the mask with the WARNING "N voxels in user-defined
  mask do not have good signal. Removing voxels from mask."; the thresholds are printed with
  numpy's float32 array formatting ("Echo-wise intensity thresholds for adaptive mask: [180.95544
  65.59678  18.5366 ]").
- With `--exclude`, `data_cat[:, :, use_volumes]` makes numpy put the indexed axis first in memory,
  so the temporal means are summed in index order instead of numpy's pairwise order: their last
  bits differ from the means without `--exclude` (`adaptive::temporal_means`).
- `--n-independent-echos` only adds its two warnings (EPTI): the counts are not changed.

### The fits (*read*; `decay.py`)

- **loglin** (`fit_loglinear`): per adaptive-mask value n (1 is fitted with 2 echoes): `log(|data|
  + 1)` in float32 (numpy's own kernel: NP-03), the design `[1, -TE]` repeated per volume, and
  `numpy.linalg.lstsq(X, Y, rcond=None)` over every voxel of that value at once (one LAPACK
  `dgelsd` call, the voxels as right-hand sides); T2* = 1 / slope, S0 = exp(intercept) (float64
  exp). One call for the whole group is why one infinite sample makes every voxel of its group NaN
  (TED-09): `log(inf + 1) = inf`, `dgelsd` scales B by `BIGNUM / inf = 0` (`inf · 0 = NaN`) and
  back by `inf`.
- **curvefit** (`fit_monoexponential`): loglin first, as the start; per voxel
  `scipy.optimize.curve_fit(s0 · exp(-TE / t2s), TE per volume, data, p0=(S0, T2*), bounds=((min
  of the voxel's data, 0), (inf, inf)))`, which with bounds is SciPy's **trust-region reflective**
  `least_squares` (`jac="2-point"`, `x_scale` 1, the exact solver: a dense SVD, `ftol = xtol = gtol
  = 1e-8`, `max_nfev` 200), not MINPACK (only an unbounded `curve_fit` reaches `leastsq`). Data or
  a start that is not finite (`asarray_chkfinite`), a start outside the bounds (a log-linear S0
  below the voxel's minimum, a T2* <= 0) and a fit without convergence are ValueErrors or
  RuntimeErrors, all caught: the voxel is a "failure", keeps the log-linear estimate and is marked
  in the failures map (TED-06, TED-08). The covariance: `svd(res.jac)`, the singular values above
  `eps · max(shape) · s0`, `(VT.T / s²) · VT · 2 · cost / (m - 2)`, an `OptimizeWarning:
  Covariance of the parameters could not be estimated` when it is indeterminate (tedana's
  `decay.py:111`, once per place, printed after the group's fits; per-volume fits of three echoes
  reach it). joblib's `Parallel(n_jobs=n_threads)` over voxels; tqdm's bars "{n}-echo
  monoexponential" on stderr.
- **fitmode ts**: `fit_decay` per volume (each a one-volume fit), 4D maps.
- **modify_t2s_s0_maps**: T2* infinite -> 500, <= 0 -> 1, the floor where `exp(-TE / T2*)`
  underflows (`min(-TE) / log(eps)`; its `--debug` message miscounts: TED-14), S0 NaN -> 0; the
  limited copies with the one-echo voxels set to 0; the cap of the limited T2* only (TED-05: values
  above ten times the 99.5th percentile, `scipy.stats.scoreatpercentile(..., "lower")` over the
  whole limited map, zeros included, set to it).
- **RMSE** (`rmse_of_fit_decay_ts`, with the limited maps): per voxel the mean over volumes of the
  square root of the mean over the good echoes of the squared residuals (float32 predictions from
  float64 `s0 · exp(-te / t2s)`); the confounds table: `rmse_mean`, `rmse_std`, `rmse_min`, the
  2nd, 25th, 50th, 75th and 98th percentiles and `rmse_max` per volume (`nanpercentile`, linear).
  `nanpercentile` with tedana's list of quantiles returns float64 (the interpolation in float64,
  `b - a` in float32), so the percentile columns are float64 while `rmse_mean` and `rmse_std` are
  float32 values cast by `column_stack`; but when volume 0 has no number the whole percentile block
  is float32 (`apply_along_axis` takes the first column's type): the replica's
  `Percentiles::float32`.

### The optimal combination (*read*; `combine.make_optcom`)

- `t2s`: weights `TE · exp(-TE / T2*)` (Posse et al. 1999) with the full T2* map in ms; `paid`:
  `TE · mean / std` over time (PAID's `divide(f32, f32, out=f64)` is a float32 division; a zero
  deviation in every good echo makes the weights sum to zero: TED-15); echoes 1..n per adaptive-mask
  value (1 uses 2); `np.average` along the echoes: in index order, the weights' sum pairwise for
  `--fitmode ts` (echoes innermost in memory), both pairwise for one volume.
- The message is "voxel- and volume-wise" whenever the T2* map is 4D (`t2s.ndim`), that is with
  `--fitmode ts` even for a single volume.

### The outputs (*read*, *measured*)

- Names (`bids` / `orig`): `desc-adaptiveGoodSignal_mask` / `adaptive_mask` (int32),
  `desc-decayFitFailures_mask` / `fit_failures` (uint8, curvefit), `stat-variance_desc-t2star_statmap`,
  `stat-variance_desc-s0_statmap`, `stat-covariance_desc-t2star+s0_statmap` / `t2s_var`, `s0_var`,
  `t2s_s0_covar` (curvefit + `--verbose`), `S0map` / `s0vG`, `desc-rmse_statmap` / `rmse`,
  `desc-confounds_timeseries.tsv` / `confounds`, `desc-limited_S0map` / `s0v`,
  `desc-limited_T2starmap` / `t2sv` (seconds), `T2starmap` / `t2svG` (seconds), `desc-optcom_bold`
  / `ts_OC`, `dataset_description.json`, `desc-tedana_registry.json` / `registry`,
  `t2smap_call.sh`, an empty `figures/`.
- Images: `.nii.gz`, float64 written as float32, int64 as int32, through nilearn's
  `masking.unmask` and `new_img_like(..., copy_header=False)`: a **fresh** nibabel header from the
  mask's affine (sform code 2, qform code 0 with its quaternion filled by nibabel's `set_qform`,
  `xyzt_units` 0, `descrip` empty, `pixdim[0]` = qfac); nibabel keeps the codes 0 when the fresh
  header's own affine is "close" (`allclose`) to the given one; 4D: the first echo's zooms
  (`set_zooms`, the TR in `pixdim[4]`; converted references have float32 zooms; its two
  HeaderDataErrors); gzip with mtime 0, no name, level 1.
- **The affine nibabel reads** (*measured*, random testing 2026-11-08): `img.affine` of an image
  with only a qform (`sform_code` 0) is `get_qform`, computed in numpy's `longdouble` (x87's
  extended format on x86-64 Linux): `fillpositive`, `quat2mat` and `np.dot(R, diag(vox))` through
  `LONGDOUBLE_dot`, which sums from `+0`. Every zero of the result is `+0` (a plain float64 product
  `0 · (-vox)` gives `-0.0`, which the outputs' `srow` then hold), and oblique rotations' entries
  can differ from float64 arithmetic in the last bit. The replica reads it as nibabel does
  (`NiftiHeader::best_affine_nibabel`, larmorx-io; `check_affine.py`: 4,000 generated headers
  identical to nibabel). (nibabel's `get_best_affine` tests the codes with `!= 0`.)
- The table: pandas' `to_csv(sep="\t", lineterminator="\n", na_rep="n/a", index=False)`, float64
  in Python's shortest repr (with ties to the even digit, as numpy's Dragon4 and Python's dtoa:
  920.45343017578125 prints `...812`; Rust's `{:e}` may pick the other: the replica's
  `shortest_ties_even`). JSON: `json.dump(indent=4, sort_keys=True)`; `dataset_description.json`
  names the host's `platform.uname()`, `sys.version` and twelve library versions (the replica
  names itself there: decision D6).
- `_convert_to_nifti1` (pairs, NIfTI-2) returns NIfTI-1 single files unchanged: a 4D mask of one
  volume is not squeezed, the fast data path fails and nilearn's path runs, which zeroes non-finite
  data. The z-concatenated reference is C-ordered in memory (`get_fdata()` of a slab), the others
  Fortran-ordered (as nibabel holds them): nilearn's `compute_epi_mask` sees them so.
- Two echo-count errors (files against echo times) are tedana's own `fit_decay` ValueError, after
  the adaptive mask.

### Messages (*read*, *measured*)

- tedana's log lines carry their module, function and line (`t2smap:t2smap_workflow:365 Using
  output directory: <abs>`, `utils:check_te_values:796 TE values appear to be in milliseconds.
  ...`, `io:__init__:164 Generating figures directory: ...`, `utils:load_mask:916 Using
  user-defined mask`, `t2smap:t2smap_workflow:414 Loading input data: [...]` (a Python list's
  repr), `utils:make_adaptive_mask:167` (the thresholds), `:195` (removed voxels), `combine:make_optcom:202
  Optimally combining data with voxel-wise T2* estimates`, `utils:log_newsletter_info:812, 816`);
  `--debug` adds shapes, counts, the floor and the cap (`{cap*10:.5f}`), the curve fit's failure
  counts. TED-02: tedana logs "Excluding N volumes from adaptive mask and T2*/S0 estimation" before
  its IndexError.
- Python's warnings print the installed file and line; numpy's differ per release: "Mean of empty
  slice" (2.5.4) / "Mean of empty slice." (2.3.5) at `_methods.py` 134 / 136, `nanstd`'s
  "Degrees of freedom <= 0 for slice." at 1997 / 2015, `_nanquantile_unchecked`'s "All-NaN slice
  encountered" at 1593 / 1617, "invalid value encountered in divide"; each printed once per place
  (`decay::Libraries`).
- tqdm's bars print the run's own timings (not compared; the oracle runs with `TQDM_DISABLE=1`).
  tqdm 4.70.1 takes its `disable` default from `TQDM_DISABLE` (`tqdm.utils.envwrap`: names
  `TQDM_` + the parameter in any case, the last winning; the text converted with `bool()`, so
  any non-empty value disables the bars, `0` included): observed on tedana with `1`, `0`, an
  empty value and `TQDM_disable`. The replica printed its bars whatever the variable said until
  2026-11-08 (relay 8b: `log::tqdm_disabled`, the same rule; the parity suite had normalised the
  bars away, so its record did not see it).

## Defects (D13; the register's TED-01 to TED-18, TED-31 and TED-46)

Every one verified black-box (`validation/investigations/t2smap/replica/check_failures.py` and the
parity suite's `fail/` cases): with `LARMORX_UPSTREAM_DEFECTS=reproduce` the replica ends as tedana
does (exit status, last error line, every message and file); by default it applies the rule's
correction and names the defect (`larmorx-gpl: upstream defect TED-..` on stderr) exactly where the
result changes.

| ID | What tedana does (*read*, *measured*) | Rule | larmorx's default |
|---|---|---|---|
| TED-01 | a NaN first-echo temporal mean (a NaN sample, or no volume left) makes the dropout method's 33rd percentile NaN: `ValueError: attempt to get argmax of an empty sequence` | A | the percentile and the exemplars over the means that are not NaN (an infinite mean sorts and matches as in tedana: since 2026-11-12, it was dropped with the NaN before); none left: a clear error |
| TED-02 | `--exclude` accepts an index equal to the number of volumes (`>` for `>=`), then numpy's IndexError | B | a clear error, as for larger indices |
| TED-03 | without `--mask`, nilearn's `compute_epi_mask` may find nothing (MaskWarning, then `IndexError: index -1 ...` in the adaptive mask) or too few voxels (the argmax ValueError) | B | a clear error |
| TED-04 | a user mask is applied by voxel index, its affine never compared with the data's (tedana's TODO); measured: the four real calls' masks have the data's affine | B | nilearn's own "Mask affine ... is different from img affine" error, mask against the first echo |
| TED-05 | only the limited T2* map is capped (below) | A | the full map gets the limited map's cap |
| TED-06 | a curve fit whose log-linear start is outside the bounds is a silent "failure" keeping the log-linear estimate; measured (relay 4, `defects/ted06_feasible_start.py`): of 1,930 failures on fMRIPrep's four calls 1,919 are such starts (every one a T2* <= 0, a signal rising with the echo time), 11 the evaluation budget; refits from the clipped start beat the log-linear residuals in only 15 to 29 % | C | kept, documented (no switch) |
| TED-07 | `--verbose` writes the variance of S0 as the T2* variance map and the T2* variance as the S0 map (the covariance diagonal unpacked in the wrong order) | A | the true names; `reproduce` swaps them on output; the notice with `--verbose` |
| TED-08 | a voxel with one good echo is fitted with two and kept in the full maps, but its failure is not marked in the failures map, nor its variances in the verbose maps | A | marked and written; the notice when such a fit failed (or, with `--verbose`, was fitted) |
| TED-09 | one infinite sample (or an integer scaled beyond float32) makes T2* and S0 NaN for every voxel of its adaptive-mask group (one `lstsq` per group) | A | that voxel NaN alone, the rest solved without it |
| TED-10 | an infinite echo time is accepted (taken as milliseconds) and fails in `lstsq`: `LinAlgError: SVD did not converge in Linear Least Squares` (numpy 2.3.5 after OpenBLAS's `DLASCL` parameter messages on stdout) | B | a clear error |
| TED-11 | `--n-independent-echos 0`: Python's ZeroDivisionError | B | a clear error |
| TED-12 | a single echo is accepted, then an unrelated numpy error (IndexError in dropout, column_stack's ValueError in the log-linear fit) | B | a clear error |
| TED-13 | `--dummy-scans` removing every volume (the argmax ValueError after "Mean of empty slice") or negative (shifts `--exclude`'s indices up) | B | a clear error |
| TED-14 | `--debug`'s T2* floor message counts echoes times voxels as voxels | A | the voxel count (a message only) |
| TED-15 | `--combmode paid` with a zero temporal deviation in every good echo of a voxel (every voxel of a one-volume series): `ZeroDivisionError: Weights sum to zero, can't be normalized` for the whole run | A (B for one volume) | those voxels get equal weights; a one-volume series a clear error |
| TED-16 | `--exclude` removing every volume: the adaptive mask from no volume, the log-linear fit of no rows, T2* = 1/0 (then 500 ms) and S0 = 1 in every voxel, written with exit status 0 (random testing) | B | a clear error |
| TED-17 | a single voxel left for the fit: `fit_loglinear`'s squeezed mask column is 0-dimensional, `IndexError: too many indices for array` (both fit types and modes; random testing) | A | fitted |
| TED-18 | the limited T2* map's cap is its 99.5th percentile, 0 when more than 99.5 % of its values are 0 (voxels of one good echo): every positive limited T2* set to 0, the RMSE computed with T2* = 0 (random testing) | A | no cap of 0 or less, on either map (TED-05's full-map cap included) |
| TED-31 | no voxel left for the fit (the adaptive mask removed every voxel of the mask): `fit_loglinear`'s count-1 step indexes column 0 of an array of no column, `IndexError: index 0 is out of bounds for axis 1 with size 0`, after the adaptive mask image and the echo-time check (both fit types and modes; found by the clean-room original's black-box probe, 2026-11-11) | B | a clear error (the text both implementations share); the replica completed a log-linear run of no fit voxel and stopped a curve fit with nilearn's empty-mask text until 2026-11-12, in both modes |
| TED-46 | a voxel whose log-linear observations are all equal (saturated samples: no decay) gets the batched `lstsq`'s rounding noise as its slope: T2* 1 ms, or 1e15-1e18 ms, never 500 ms (found by the clean-room original's random testing, 2026-11-11; spec §7.2, §13.4) | A | the exact solution (y, 0) in a rank-2 group, the batch's other columns kept (tedana's bits); `reproduce` keeps `lstsq`'s noise (since 2026-11-12) |

Not defects: `--debug`'s help promising a TSV log file (C, documented); the dropout mask appended
twice; RepLGR lines that go nowhere; warnings and bars naming paths and timings.

Library defects met on tedana's path (the register's rows): **NP-03**, numpy's float32 `log` is
numpy's own kernel on x86-64 CPUs with AVX2/FMA3 or AVX-512 (not correctly rounded: 10,947,841 of
the 201,326,593 floats in [1, 2^24] differ from the correctly rounded, at most 3 ulp; 78.4 million
of all positive floats, at most 4 ulp), the C library's `logf` elsewhere: tedana's log-linear fit
depends on the CPU; by default larmorx uses the correctly rounded `logf`, `reproduce` numpy's kernel
(`logf_numpy`). **NP-04**, numpy's float64 `exp` and `log` on AVX-512 CPUs are Intel's SVML kernels
(`exp` differs from glibc's on 4.1 % of the arguments in [-800, 0], tedana's `exp(-TE/T2*)`; `log`
on 0.10 % in [0, 10]); glibc's elsewhere: the record's spread measures it (22,899 values of
fMRIPrep's ds003592 call); the reference is the non-AVX-512 path (decision D2:
`NPY_DISABLE_CPU_FEATURES`), a register row only. **GLIBC-07**: numpy's float64 `exp`/`log` (glibc
2.35's on CPUs without AVX-512) and SciPy's powers (C's `pow`) are not correctly rounded; the
default uses the correctly rounded functions.

The default's maths notices are exact: the program keeps the first pass's written bits and printed
lines, and for each family of functions whose other choice gave another value on some argument (a
witness per family, the OR over every argument: deterministic at any thread count) runs the stages
from the mask to the combination again with that choice, writing nothing; the defect is named with
the written values and printed lines that differ, or not at all (both modes).


## TED-05: the full T2* map is no longer capped (rule A)

- **What tedana 26.0.3 does** (*read*). `decay.modify_t2s_s0_maps` applies the floors (infinite
  T2* to 500 ms, T2* <= 0 to 1 ms, the floor where `exp(-TE / T2*)` underflows) to the full map,
  copies it into the limited map with the one-echo voxels set to 0, and caps **only the limited
  map**: values above ten times its 99.5th percentile (`scipy.stats.scoreatpercentile`,
  `interpolation_method="lower"`) are set to that percentile. `t2smap_workflow` writes the full
  map (`T2starmap`, which fMRIPrep keeps as its T2* map) and weights the optimal combination with
  it, uncapped.
- **What tedana 25.0.1 did** (fMRIPrep 25.2.6's tedana; *read*). `t2smap_workflow` capped the
  full map itself after the fit, with the same rule (its comment: "set a hard cap for the T2*
  map/timeseries ... anything that is 10x higher than the 99.5 %ile will be reset to 99.5 %ile"),
  besides `modify_t2s_s0_maps`' cap of the limited map; tedana's main workflow (`tedana.py`) did
  the same.
- **The change** (*history*). Pull request ME-ICA/tedana#1290, "Write out T2* & S0 variance +
  T2*-S0 covariance" (merged 2026-01-08, released in 26.0.0 on 2026-02-25; closes #1268, "Track
  and write out S0 and T2* estimate variance (and maybe covariance?) maps"), removed the
  workflow's cap of the full map (`tedana/workflows/t2smap.py`: the four lines of `cap_t2s`) while
  restructuring the fit: its description lists "Create new function to create the limited maps
  from the full maps and apply the ceilings and floors", that is, the floors and caps were meant
  to move into `modify_t2s_s0_maps`, which caps the limited map only. Neither the description, its
  seven comments and review, nor the 26.0.0 release notes (which list the pull request only by its
  title) mention the full map's cap or a reason to remove it; tedana's documentation
  (`docs/outputs.rst`) describes `T2starmap` as the "Full estimated T2* 3D map" without one. The
  current main branch (checked 2026-11-08) still caps only the limited map.
- **Measured** (`validation/investigations/t2smap/defects/ted05_full_map_cap.py`, relay 5b): on
  fMRIPrep's four multi-echo t2smap calls (ds003592 sub-280, ds000210 sub-02 cuedSGT and rest,
  ds000258 sub-21262), 2 to 34 voxels per call hold full-map values above ten times the cap, up
  to 239,794,244 ms (239,794 s: not a T2*), where tedana 25.0.1's t2smap, fMRIPrep's own, wrote
  the capped value.
- **Decision: rule A** (the input makes sense and the result is wrong), not rule C: the removal
  came with a restructuring whose description says the new function applies "the ceilings and
  floors", with no word of dropping the full map's; it is not a design pipelines rely on (fMRIPrep's own tedana capped the full map). By default the full map gets the
  limited map's cap: values above ten times the limited map's 99.5th percentile are set to it
  (tedana 25.0.1 took the full map's own percentile, which differs from it only by the one-echo
  voxels the limited map holds as 0); the notice names how many values were capped, and the
  threshold. `LARMORX_UPSTREAM_DEFECTS=reproduce` (`upstream_defects="reproduce"`) writes
  tedana 26.0.3's uncapped map and weights.
- **A cap of 0** (random testing, 2026-11-08): when the limited map's 99.5th percentile is 0,
  tedana's cap sets every positive limited value to 0 (TED-18); the default applies no cap of 0
  or less to either map, so TED-05's full-map cap is then not applied either.

## The libraries, as tedana's path reaches them (*read*, *measured*)

- **numpy's reductions** (`larmorx_core::numpy`): every reduction starts at `+0.0`; along the
  innermost remaining axis (or an outer axis whose later axes all have length 1) `0 + pairwise`
  (blocks of 8 accumulators up to 128, halves rounded down to multiples of 8; short sums start at
  `-0.0`); along an outer axis in index order; no chunking up to 100,000 elements in 1-D (numpy
  2.2.6: blocks of 8,192). `mean` and `var` divide in float64 (NEP 50: float32 / intp).
  `percentile(..., "higher")`, `nanpercentile` (linear), `scoreatpercentile(..., "lower")`,
  `average` (its ZeroDivisionError), `nanstd` (its warning); verified on 109 x 9 reduction groups
  and 90 percentile cases in both environments.
- **numpy's float32 log** (NP-03): `logf_numpy` equals numpy's on all 2^32 inputs, AVX-512 and
  AVX2/FMA3 paths, both releases. numpy's float64 `exp`/`log` with AVX-512 off are glibc 2.35's
  `exp@GLIBC_2.2.5` and `log@GLIBC_2.2.5` (numpy's wheels bind the old symbols), except that
  numpy's log of a negative is C's `+NaN`. `NPY_DISABLE_CPU_FEATURES="X86_V4 AVX512F AVX512CD
  AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"` turns AVX-512 off in both numpy 2.3.5 and
  2.5.4 (2.5.4's dispatch targets are `X86_V3`, `X86_V4`, `AVX512_ICL`, `AVX512_SPR`); turning off
  `X86_V3` or `FMA3 AVX2` too makes numpy's float32 log glibc's `logf`.
- **numpy's `lstsq`** (`larmorx_transform::numpy_linalg::lstsq`): `dgelsd` for an `m x 2` design
  and one right-hand side per voxel: `MNTHR = 3` (m >= 3: `dgeqr2`, `dorm2r` through `dlarf` (LAPACK
  3.12.0, numpy 2.3.5) or `dlarf1f` (3.12.1, numpy 2.5.4), `dgebd2`, `dormbr`; m < 3: `dgebrd`
  directly), `dlalsd` with n <= SMLSIZ (`dlasdq` sorts the singular values up after `dbdsqr` sorted
  them down), the threshold `rcond · max(d)` with `rcond = eps · max(m, n)`, `dlascl`'s scaling when
  a norm leaves [smlnum, bignum] (and its argument check: INFO < 0 without scaling for a NaN norm,
  TED-10); OpenBLAS's Haswell kernels on these shapes, main loops included: `dgemv_t` (rows in
  blocks of 2,048, the tail contracted with FMA; columns in groups of 4 through the microkernel, a
  pair through SSE2 4x2, a single through 4x1), `dger` -> `daxpy` (FMA for the first 16⌊m/16⌋ rows),
  `dgemm`'s 2 x N kernel, `nrm2.S`'s four x87 accumulators. A column's result depends on its place in
  the kernels' groups (about a fifth of columns solved alone differ in their last bits) and on BLAS
  threads (below). Verified: 550 cases per environment, tedana's own calls on fMRIPrep's three
  multi-echo runs among them, identical at 1 and 4 threads.
- **SciPy's `curve_fit`** (`larmorx_optim::curve_fit2`): `least_squares` TRF with bounds,
  `approx_derivative` ('2-point', bounds-adjusted steps), `scipy.linalg.svd` (`dgesdd`, paths 3 and
  5s, SciPy's own OpenBLAS with LAPACK 3.12.0's routines and GCC 10.2 kernels), numpy's `dot`
  (`cblas_matrixproduct`: row-major `dgemv` for a C-contiguous matrix, column-major otherwise).
  SciPy 1.15.3 -> 1.18.1 changed only the memory order of the SVD's factors on this path (1.18.1's
  batched `_svd` returns C-ordered `U`, `VT`), which sends `U.T.dot(f)` and `V.dot(x)` to `dgemv_n`
  or `dgemv_t`: the curve fit's last bits (ds003592: 39,211 of 52,826 fits, median relative 9e-10,
  largest 0.70), evaluation counts and statuses of 186 to 2,168 fits per call, never the outcome.
  Verified: tedana's own 169,344 `curve_fit` calls on fMRIPrep's four multi-echo calls identical in
  both environments; every exit status (1 to 4, the covariance warning, the budget, a start outside
  the bounds, data or Jacobian not finite) reached; `intersect_trust_region`'s two ValueErrors
  ported but unreached in 30,000 random fits.
- **nilearn 0.14.1's `compute_epi_mask`** (`larmorx_interp::nilearn`): the mean over time with
  `smooth=1` (`gaussian_filter1d` per axis, SciPy's `correlate1d`, mode reflect, truncate 4.0; no FMA
  in SciPy's `_nd_image`), the sorted values' largest gap between 0.2 and 0.85, opening 2
  (`binary_erosion`, the largest connected component, `binary_dilation`), int8.
- **nibabel's header arithmetic**: `set_qform` through numpy's `svd`, `det` and `eigh`
  (`set_affine_nibabel`, the field maps' part), `get_qform` in extended precision (above).

## Random testing (2026-11-08)

The descriptor `tedana-t2smap` (`validation/src/larmorx_validation/difftest/tools/tedana_t2smap.py`)
draws multi-echo series (two to six echoes, 1 to 12 volumes, grids of 6 to 24 voxels across,
drawn geometries and stored types, special samples) and every option, and runs each case through
the replica suite's comparison in both environments with the D13 fix pass. Two 2-minute probes
(seed `t2smap-probe`) and the trial (`oracles/difftest/2026-11-08-t2smap-replica`, seed
`t2smap-trial`, 30 minutes, 12 jobs at one thread, 1.5 GiB a case, larmorx `a519ad06`: 3,886
cases, 3,816 agree, 11 notices, 59 differences) found, all fixed in the replica or registered
(`validation/difftest/triage.toml`; the permanent cases in `validation/difftest/cases/tedana-t2smap/`):

- **nibabel's `get_qform` in extended precision** (*measured*, above): the outputs' `srow` held
  `-0.0` where tedana's hold `+0.0` for qform-only images.
- **nibabel sets the sform and qform of every new image** (*read*, *measured*):
  `Nifti1Image(data, affine)` without a header calls `_affine2header` unconditionally; only the
  update at writing compares the affine with the header's (`allclose`). The replica had skipped
  the fresh header's update when the affine equalled its own base affine.
- **NIfTI-2 outputs** (*read*, *measured*): `_convert_to_nifti1` returns any `Nifti1Image` as it
  is, and a `Nifti2Image` is one (a subclass), so a NIfTI-2 first echo stays the reference image;
  without `--mask`, nilearn's mask and every output are then `Nifti2Image`s (NIfTI-2 files). A
  user mask is always made a `Nifti1Image` (`load_mask`); pairs are converted.
- **No volume left** (*read*, *measured*): with every volume a dummy scan (TED-13, refused by
  default) and no dropout method, tedana goes on: numpy's `lstsq` of no rows returns zeros (`if m
  == 0: x[...] = 0`), T2* = 1/0 (its divide warning, `decay.py:371`), and the confounds table fails
  in pandas (`Shape of passed values is (0, 3), indices imply (0, 9)`: `nanpercentile` of an empty
  array returns `nanmean`'s `(0,)`); with `--fittype curvefit` the first voxel's bound,
  `np.min(data_2d[:, voxel])` of an empty column, raises outside `curve_fit`'s `try` (`zero-size
  array to reduction operation minimum which has no identity`). With `--exclude` covering every
  volume the same fit runs on nothing and tedana writes T2* = 500 ms, S0 = 1 everywhere with exit
  status 0: **TED-16** (rule B: a clear error by default).
- **A single voxel to fit** (*read*, *measured*): `fit_loglinear`'s `np.squeeze(echo_masks[...,
  i_echo])` of one voxel is 0-dimensional, and indexing it fails (`IndexError: too many indices for
  array: array is 0-dimensional, but 1 were indexed`) in both fit types and modes: **TED-17**
  (rule A: fitted by default).
- **A cap of 0** (*read*, *measured*): the limited map's 99.5th percentile is 0 when more than
  99.5 % of its values are 0 (voxels of one good echo); every positive limited T2* is then set to
  0, and the RMSE divides by it (`decay.py:83`'s divide warning): **TED-18** (rule A: no cap of 0
  or less by default, on either map; TED-05's full-map cap followed it before).
- **numpy's warnings** (*read*, *measured*): `mean` over an empty axis warns "Mean of empty slice"
  whatever the result's size (no voxel included), the division 0/0 only where values exist;
  `--n-independent-echos` with no good voxel divides numpy int64 scalars 0/0 ("invalid value
  encountered in scalar divide", `utils.py:213` and `:230`); PAID's `mean` and `std` of a series
  holding both infinities warn "invalid value encountered in reduce" (`_methods.py` 132 and 168 in
  numpy 2.5.4, 134 and 170 in 2.3.5), of one holding infinities of one sign "invalid value
  encountered in subtract" (188 / 190; the source line differs too: `x = um.subtract(arr, arrmean,
  out=...)` / `x = asanyarray(arr - arrmean)`).
- **The default's maths notices** (larmorx's own): one family alone can change written values that
  the other family's choice then takes back (the full other mode gives the default's values), or
  change them only together with it (numpy's float32 log moves a curve fit's start, and on that
  path glibc's `exp` differs from the correctly rounded one where it did not on the default's). The
  program now runs the other mode's functions altogether first, names nothing when they change
  nothing, and otherwise names each family that changes something alone, or both with the joint
  effect.
- **Notices without a visible change** (larmorx's own): TED-07 where both variance maps hold the
  same float32 values (named now only where the swap changes one, outside the one-echo voxels,
  which are TED-08's); TED-04 for a mask of another shape, for which tedana's path raises nilearn's
  error itself (named now only for a 3D mask of the data's shape, tedana's silent fast path).
- **tedana's own step between the releases on log-linear fits** (*measured*): numpy 2.3.5 ->
  2.5.4's LAPACK 3.12.0 -> 3.12.1 (`dgelsd`'s QR through `dlarf1f`) moves the last bits of a voxel's
  slope now and then (none of the record's cases), which T2* = 1/slope magnifies where the slope is
  near zero (up to 93 % relative in one noise voxel); the suite reports it with that reason.

## Black-box facts (open: runs of tedana's program and of numpy only)

What runs show without tedana's source; the spec writer may copy from here.

- **The log-linear fit's last bits depend on the batch and on BLAS threads** (*measured*, relay
  2): numpy's `lstsq` solves every voxel of an echo count at once; about a fifth of the columns
  solved alone differ in their last bits, a shift of the batch by 1 to 3 columns changes 2 of
  5,003, and `--n-threads` 2, 4 or 14 (OpenBLAS's threads) change 4 to 14 of 5,003 voxels' fits.
  tedana's log-linear maps therefore depend on `--n-threads`; the replica gives tedana's
  one-thread result at any thread count.
- **tedana's own spread** (the record `tedana-t2smap-replica`, its `spread/` cases; values that
  differ from the reference setup: newest libraries, numpy's AVX-512 kernels off, one thread):
  numpy's AVX-512 kernels on: 22,899 values on fMRIPrep's ds003592 call (of 35.6 million), 8 on a
  synthetic curve fit, 0 on a synthetic log-linear fit; `--n-threads 4`: 1 / 0 / 0; the shared
  releases (numpy 2.3.5, SciPy 1.15.3): 28,808 / 5 / 0; tedana 25.0.1 in fMRIPrep 25.2.6's image:
  176,938 / 447 / 1,646. The replica at four threads gives its one-thread values.
- **fMRIPrep's own files against tedana 26.0.3** (values of `T2starmap` and `desc-optcom_bold`
  that differ; fMRIPrep's runs used tedana 25.0.1): ds000210 cuedSGT 12,295, ds000210 rest 12,565,
  ds000258 35,226, ds003592 41,897; against larmorx's default 12,117 / 12,602 / 34,918 / 42,180
  (the default caps the full map as 25.0.1 did, TED-05, and its functions are correctly rounded).
- **The library step on a log-linear fit** (random testing): tedana 26.0.3's log-linear T2* can
  differ between numpy 2.3.5 and 2.5.4 in a noise voxel (none of fMRIPrep's calls).
- **Speed and memory** (fMRIPrep's ds003592 call; `docs/benchmarks/tedana-t2smap.md`): tedana
  26.0.3 takes about 55 s (curvefit) and 1.24 GB at its peak.

## The clean-room part's replica gaps (relay RS1, 2026-11-12)

The clean-room original's record, probe and random testing (2026-11-11) found where larmorx-gpl
0.1.0rc1 departed from tedana; each was checked against tedana 26.0.3's source and its program
run **directly** (not through the oracle-output cache, which keeps files but not directories:
`validation/investigations/t2smap/replica/rs1_probe.py`, 21 cases, every line of standard error
with the warnings' source lines, every file's bytes), fixed, and kept as a permanent case of the
parity suite (`check_failures.py`'s cases 11 to 23, each failing on the PyPI 0.1.0rc1 replica and
passing from this branch, in both environments):

- **Chained tracebacks** (*read*: `io.load_data_nilearn`'s `try: <fast path> except Exception:
  <nilearn's apply_mask>`): when the fallback fails too, Python prints the fast path's error,
  `During handling of the above exception, another exception occurred:`, then the fallback's. The
  replica printed only the last. Now `log::PyError` carries the handled error (`during_handling_of`)
  and `data::load_data` chains them: a 3D echo (`Expected 4D image` twice), a mask of another grid
  (`Image/mask shape mismatch`, then nilearn's `Mask shape: ...`), an empty mask of another grid
  (the mismatch, then nilearn's empty-mask text: the "message" gap the original's suite saw was
  this chain). The suite now compares the chain (`exceptions`).
- **Python's warnings registry is reset during a run** (*measured*: `rs1_probe.py --registry`
  traces `warnings._filters_mutated`): when `get_system_version_info` asks the libraries' versions
  (before any warning) and when `save_tsv` writes the confounds table (pandas' `pandas_dtype` in
  `infer_objects` under `catch_warnings`), so a warning shown before the table shows again after
  it. `Log::reset_registry` at both places.
- **Two warnings the replica never printed**: numpy's `RuntimeWarning: overflow encountered in
  cast` at `tedana/io.py:352` (`data = np.float32(data)` in `save_img`, a finite float64 value
  beyond float32's range: the full S0 map, then again the limited one after the table; also the
  variance maps, the T2* maps and the combined series), and `RuntimeWarning: invalid value
  encountered in reduce` at `numpy/_core/_methods.py:132` (134 in numpy 2.3.5) from the adaptive
  mask's temporal means (`data.mean(axis=-1)`) of a series whose sum adds +inf and -inf, which
  PAID's means then print again after the reset. The condition is numpy's summation order
  (`adaptive::sum_adds_opposite_infinities`: pairwise with eight accumulators along a contiguous
  axis, in order along an outer one; a NaN met first makes the sum NaN quietly); PAID's warnings
  use it too (they skipped every series with a NaN before).
- **TED-31 and TED-46** (above): the defaults the clean-room original takes, with the same notices
  (`specs/t2smap.md` §15.1); `reproduce` tedana's IndexError and `lstsq`'s noise. Measured: the
  replica's default and the original's default write the same files on the three TED-46 cases of
  `rs1_probe.py` (T2* 0.5 s and S0 256 at the 18 saturated voxels, where tedana writes 7.9e12 s,
  or 2.7e13 s per volume), with the same notice (18 voxels, 3 echoes). In `curvefit` the exact
  slope's start (T2* = +inf) is SciPy's refused start, a failure that keeps the log-linear S0
  (256, from log(255 + 1)), where tedana's positive noise let the fit converge to S0 255.5.
- **TED-01** (spec writer 3, 2026-11-12): the default drops NaN first-echo means only (an infinite
  one sorts and matches as in tedana, the original's rule since its second trial); the notice and
  the error say "not NaN" / "a number".
- **`--fitmode ts` with no volume** (TED-13 in `reproduce`, *read* and observed): `fit_decay_ts`
  calls `fit_decay`, which checks the echo times, once per volume, so with no volume it checks
  nothing; tedana then writes the S0 map and fails at the confounds table. The replica checked the
  echo times first; now it skips that check (and TED-31's) when it fits nothing.
- **numpy's scientific printing of the thresholds**: the shared fix in
  `larmorx_core::numpy::format` (the clean-room part's relay 3a-ii: numpy prints a shorter
  element's own further digits, not zeros) reaches the replica when it is rebuilt: case 22 prints
  `8.8745688e+04` as tedana, where 0.1.0rc1 printed its zero-padded form.

## ANALYZE 7.5, MGH/MGZ and TED-46 under the curve fit (relay RS2, 2026-11-12)

- **ANALYZE 7.5 and MGH/MGZ are read** (*read*: nibabel 5.4.2's `load`, `Spm2AnalyzeHeader`,
  `Spm99AnalyzeImage.from_file_map`, `MGHHeader`, `MGHImage.from_file_map`, `affines.from_matvec`,
  `volumeutils.array_from_file`; tedana's `_convert_to_nifti1`). nibabel tries its classes in order:
  a `.hdr`/`.img` pair whose header is not NIfTI's (348 in `sizeof_hdr` in either byte order, no
  `ni1`/`n+1` magic) is an `Spm2AnalyzeImage` (SPM2's scale factor `funused1` with its intercept
  `funused2`, else the gl/cal ranges; SPM's origin; a MATLAB `.mat` beside it overrides the
  affine); `.mgh`/`.mgz` (no other compressed suffix: `x.mgh.gz` is no image nibabel knows) is an
  `MGHImage`. tedana converts both to an in-memory `Nifti1Image` of nibabel's scaled array.
  The replica reads their bytes through the main package's reader of that loading,
  `larmorx_tedana::formats` (Apache-2.0: the clean-room original's, called as a library; a shared
  crate would be its better home, below), and adds what tedana's path decides:
  - **when the data are read**: `_convert_to_nifti1` reads a converted image's data at once, so a
    reading error (a short `.img`) comes where the image is loaded: at `load_ref_img` for the
    first echo (one traceback), in the fast path before its shape check for a later echo (then
    again in nilearn's fallback, which converts every echo before masking: two chained
    tracebacks), and before the squeeze of a user mask. The replica read the data after those
    checks; it now reads converted images where tedana does (`Header::converted`);
  - **the file a short read names**: nibabel's `array_from_file` names the file object it read
    (`Expected <n> bytes, got <m> bytes from <file>`, then ` - could the file be damaged?`): an
    ANALYZE pair's `.img`, not the `.hdr` the user gave;
  - **MGH's affine is float32** (*read*: `from_matvec(MdcD, Pxyz_c - centre)` makes a 4 × 4 of
    `MdcD`'s type, float32): the translation, computed in float64, is stored rounded to float32.
    It shows only where the affine is printed (nilearn's `Mask affine: ...` refusal); the outputs'
    headers keep float32 fields either way. The main package's reader returns the float64
    translation (an observation for its clean-room owner, in behaviour words: the MGH translation
    is the float32 rounding of `Pxyz_c - centre`);
  - complex64 and RGB ANALYZE data, and headers the reader refuses (negative dimensions, an
    unknown ANALYZE `dim[0]`, an MGH type nibabel does not know), are refused with the replica's
    message (`larmorx.UnsupportedFormat`; D5).
  Measured: the parity suite's 20 `formats/` cases (`check_formats.py`: both byte orders, every
  stored type a fit reads, both scalings, the origin set and unset, `.mat` files of `mat`, `M` and
  neither, compressed pairs, voxel sizes of 0 and below 0, `.img` names, z-concatenated files,
  masks of each format, MGH's oblique affine, its footer or none, `goodRASFlag` 0, three formats at
  once, and nibabel's six failures) agree bit for bit in both environments; the 104 replayed
  random-testing cases of the clean-room part with ANALYZE or MGH inputs
  (`rs2_replayed_formats.py`): every file tedana wrote identical, the 30 it failed failing too; the
  descriptor's ANALYZE and MGH draws: 30 of 30 agree.
- **TED-46 under `--fittype curvefit`** (spec writer 4: rule D, measured first;
  `validation/investigations/t2smap-spec/ted46_curvefit.py`, `specs/t2smap.md` §7.3): with TED-46's
  exact slope 0 the curve fit starts at T2* = +inf, which SciPy refuses (a failure keeping the
  log-linear S0 = x + 1, the +1 of log(|x| + 1)). On the 23 cases that hold such voxels (20
  replayed trial cases, three synthetic series of 0.2 %, 1 % and 5 % saturated voxels), against
  the curve fit's exact solution (S0 = x, T2* = +inf: a residual of 0): that start already gave
  the truth's T2* (500 ms), cap and every other voxel's values, but S0 = x + 1 and a failure at
  each such voxel; a finite start (500 ms) converges at T2* 1.2e9 to 2.1e10 ms and moves the cap
  in 20 of the 23 cases (not adopted); tedana writes 1 ms (negative noise, 12 trial cases and the
  synthetic ones) or about 1e16 to 3e17 ms with S0 = x + 0.5 (positive noise, 7 cases; the cap one
  of them). The default (rule A) now takes the exact solution where the samples are all equal: S0
  = x, T2* +inf (500 ms), not a failure, variances 0 (+inf with two samples, SciPy's warning)
  (`decay::fit_monoexponential`; the unit test `equal_samples_take_the_curve_fits_exact_solution_
  by_default`); `reproduce` is tedana's fit from its noise.
- **fMRIPrep's own files, reproduced exactly** (*measured*: `rs2_fmriprep_t25.py`): on the oracle
  runs made again with OpenBLAS's Haswell kernels (2026-11-09), tedana 25.0.1 run here in
  fMRIPrep 25.2.6's image gives fMRIPrep's `T2starmap` and `desc-optcom_bold` of ds000210's two
  tasks and ds000258 **bit for bit** with numpy's AVX-512 kernels on (as fMRIPrep's run had them:
  only `OPENBLAS_CORETYPE` was set), and differs in 4,015, 7,609 and 25,542 values with them off
  (NP-04). The earlier runs' remaining differences were OpenBLAS's AVX-512 kernels. The replica
  record's comparison of fMRIPrep's files with tedana 26.0.3's counts the version step from 25.0.1
  (6,364, 13,164 and 38,876 values; ds003592, an earlier run on another machine, 41,897).
- **The reader's home** (for the orchestrator): `larmorx_tedana::formats` (ANALYZE, MGH, MATLAB
  files) reads what nibabel reads for any tool, not only t2smap; moved to `larmorx-io` (a clean
  relay's or the orchestrator's move: the replica-side relays never edit the clean-room crate),
  both implementations and later tools would call it from there.
