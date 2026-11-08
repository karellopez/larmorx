# t2smap (multi-echo T2* maps and optimal combination)

`lx.tedana.t2smap` (command line `larmorx tedana t2smap`): tedana 26.0.3's `t2smap` (TE-Dependent
ANAlysis; DuPre et al. 2021, "TE-dependent analysis of multi-echo fMRI with tedana", JOSS
6:3669). For multi-echo fMRI it estimates, voxel by voxel, the T2* and S0 of the signal decay
S(TE) = S0 · exp(-TE / T2*) across the echo times, and combines the echoes into one series
weighted by TE · exp(-TE / T2*) (Posse et al. 1999, "Enhancement of BOLD-contrast sensitivity by
single-shot multi-echo functional MR imaging", MRM 42:87-97) or by each echo's temporal
signal-to-noise ratio (PAID; Poser et al. 2006). An adaptive mask first counts each voxel's echoes
with good signal; the fit uses those echoes (at least two), log-linear (a least-squares line
through log(|S| + 1) against TE) or monoexponential (a bounded nonlinear least-squares fit started
from the log-linear one).

fMRIPrep 25.2.6 runs it for every multi-echo BOLD run (`init_bold_t2s_wf`: nipype's `T2SMap`) as
`t2smap -d <echoes> -e <TEs in ms> --mask <BOLD mask dilated by 2 voxels> --fittype curvefit`
(`--me-t2s-fit-method`, `curvefit` by default, or `loglin`), in its node directory with one
thread, and keeps `T2starmap.nii.gz` and `desc-optcom_bold.nii.gz`.

**Implementations.** tedana is licensed under the GNU LGPL version 2.1, so larmorx keeps `t2smap`
in two implementations ([licensing](../licensing.md)):
- the bit-exact **replica** (2026-11-08), `larmorx-gpl tedana t2smap` (package `larmorx-gpl`,
  under the GNU GPL version 3 only: tedana's LGPL-2.1 converted under its section 3;
  [PROVENANCE](../../crates-gpl/larmorx-gpl-tedana/PROVENANCE.md)), run as a separate program;
- the **clean-room original**, `crates/larmorx-tedana` in the main package (Apache-2.0, from
  `specs/t2smap.md`): still to be written.

Until the original exists, the replica is the only implementation: `implementation="auto"` and
`"replica"` run it when `larmorx-gpl` is installed (`pip install larmorx-gpl`, or `pip install
"larmorx[all]"`) and raise `ReplicaNotFoundError` with that command otherwise; `"original"` raises
`NotImplementedError`. The command line does the same (`LARMORX_IMPLEMENTATION`; exit 127 with
the command that installs it). The program is found through `LARMORX_GPL_BIN`, then the Python
environment's scripts directory, then `PATH`.

**Status: `experimental`** (the record `tedana-t2smap-replica`, random testing; below).

## Usage

```python
import larmorx as lx

# fMRIPrep's call: three echoes, their times (seconds, or ms as fMRIPrep passes them), a mask
res = lx.tedana.t2smap(
    ["echo-1_bold.nii.gz", "echo-2_bold.nii.gz", "echo-3_bold.nii.gz"],
    [0.014, 0.02996, 0.04592],
    mask="brainmask_dilated.nii.gz",
    fittype="curvefit",
)
res.t2star          # lx.Image, the full T2* map in seconds (fMRIPrep's)
res.optcom          # lx.Image, the optimally combined series (fMRIPrep's)
res.adaptive_mask   # int32: the number of good echoes per voxel
res.fit_failures    # uint8: the voxels whose curve fit failed (they keep the log-linear estimate)
res.confounds["rmse_mean"]   # the fit's error per volume

# tedana's defaults (log-linear fit), the files kept, tedana's original names
res = lx.tedana.t2smap(echoes, tes, mask=mask, out_dir="derivatives/t2smap", convention="orig")
```

```bash
larmorx tedana t2smap -d echo-1.nii.gz echo-2.nii.gz echo-3.nii.gz -e 14.0 29.96 45.92 \
    --mask mask.nii.gz --fittype curvefit --out-dir t2smap
```

Inputs are paths or in-memory images (an `lx.Image`, a nibabel image, an `(array, affine)` pair),
written as NIfTI-1 in their own data type to the call's temporary directory. The files are written
to a temporary directory and read back, unless `out_dir` is given (then they stay there, and
tedana refuses to overwrite an existing one unless `overwrite=True`). The result is a
`T2smapResult`: `t2star`, `s0`, `t2star_limited`, `s0_limited`, `optcom`, `adaptive_mask`,
`rmse` (images carrying `implementation`), `confounds` (the table's columns as float64 arrays,
NaN for tedana's `n/a`), `fit_failures`, `t2star_variance`, `s0_variance`,
`t2star_s0_covariance` (or `None`), `dataset_description`, `files` (tedana's registry: each
file's name by its description), `out_dir`, `log` (tedana's log, its standard error without
larmorx's notice lines) and `implementation`. A run that fails raises `ReplicaError` (a
`RuntimeError`) with tedana's messages and exit status.

## Options

| `t2smap` | Python (`lx.tedana.t2smap(...)`) | What |
|---|---|---|
| `-d FILE [FILE ...]` | `echoes` (first argument) | the echoes' series, in ascending order of echo time; one file is a z-concatenated series (the echoes stacked along the third axis: tedana warns that it is deprecated, issue 1313). NIfTI-1 and -2 (`.nii`, `.nii.gz`, pairs), ANALYZE and MGH; other formats nibabel reads (AFNI, MINC, PAR/REC) are refused with a clear message (a documented gap) |
| `-e TE [TE ...]` | `echo_times` (second argument) | the echo times: seconds when all are below 1 (converted to ms, an INFO line), milliseconds when all are at least 1 (tedana's deprecation warning); otherwise tedana's ValueError |
| `--out-dir PATH` | `out_dir=None` | the output directory (created, one level); by default the wrapper's temporary directory |
| `--prefix PREFIX` | `prefix=""` | a prefix of every file name (`_` appended unless it ends with one) |
| `--convention {orig,bids}` | `convention="bids"` | the files' names: BIDS derivatives 1.5.0, or tedana's original ones (below) |
| `--verbose` | `verbose=False` | with `curvefit`: also the fit's variance and covariance maps |
| `--overwrite`, `-f` | `overwrite=False` | overwrite existing files (else an existing output stops the run: RuntimeError) |
| `--mask FILE` | `mask=None` | the analysis mask (its voxels above 0); without one, nilearn's `compute_epi_mask` of the first echo (a warning: tedana recommends a mask) |
| `--masktype {dropout,decay,none} ...` | `masktype="dropout"` (one or several) | the adaptive mask's methods: `dropout` (an echo whose mean signal is below a third of a reference voxel's), `decay` (signal that stops decaying), `none` |
| `--dummy-scans INT` | `dummy_scans=0` | volumes removed from the start (also from the combined series) |
| `--exclude EXCLUDE` | `exclude=None` (tedana's text, `"0,5:10,15"`, or volume indices) | volumes left out of the adaptive mask and the fit, kept in the combination (0-based, ranges end-exclusive; not with `--fitmode ts`) |
| `--fittype {loglin,curvefit}` | `fittype="loglin"` | the fit: log-linear least squares, or the monoexponential curve fit (fMRIPrep's) |
| `--fitmode {all,ts}` | `fitmode="all"` | one fit per voxel over all volumes, or one per voxel and volume (4D maps) |
| `--combmode {t2s,paid}` | `combmode="t2s"` | the combination's weights: TE · exp(-TE / T2*), or PAID's temporal SNR |
| `--n-independent-echos INT` | `n_independent_echos=None` | the number of independent echoes (EPTI): only tedana's warnings change |
| `--n-threads INT` | `n_threads=1` (0: all logical CPUs, passed as -1) | threads; tedana's limit for its BLAS and its curve-fit processes. The replica's results never depend on it (it gives tedana's one-thread results; tedana's own log-linear maps change with BLAS threads in a few voxels' last bits) |
| `--debug` | `debug=False` | the log at DEBUG level (the help's TSV log file is never written: tedana's, kept) |
| `--quiet` (hidden) | `quiet=False` | the log at WARNING level |
| `-h`, `--help` | - | the usage (argparse's, exit 0) |
| `-v`, `--version` | - | `t2smap v26.0.3` (exit 0) |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects="fix"` | `"reproduce"` gives tedana's files bit for bit (its defects and its maths, below) |
| `LARMORX_TEDANA_LIBRARIES` | `libraries="newest"` | the numpy and SciPy whose arithmetic is reproduced: `newest` (numpy 2.5.4, SciPy 1.18.1) or `shared` (numpy 2.3.5, SciPy 1.15.3); the curve fit's last bits differ between them |
| - | `implementation="auto"` | `"auto"` or `"replica"` (the replica), `"original"` (none yet) |

argparse's errors exit 2 with tedana's usage and message; any other failure exits 1 with Python's
traceback lines (its first and last; the stack itself is not reproduced). The Python wrapper
checks the choices and `n_threads` before running anything (`ValueError`).

## Outputs

| tedana's description | BIDS name (`bids`) | Original name (`orig`) | `T2smapResult` |
|---|---|---|---|
| adaptive mask img | `desc-adaptiveGoodSignal_mask.nii.gz` (int32) | `adaptive_mask.nii.gz` | `adaptive_mask` |
| fit failures img (`curvefit`) | `desc-decayFitFailures_mask.nii.gz` (uint8) | `fit_failures.nii.gz` | `fit_failures` |
| t2star variance img (`curvefit`, `--verbose`) | `stat-variance_desc-t2star_statmap.nii.gz` | `t2s_var.nii.gz` | `t2star_variance` |
| s0 variance img (`curvefit`, `--verbose`) | `stat-variance_desc-s0_statmap.nii.gz` | `s0_var.nii.gz` | `s0_variance` |
| t2star-s0 covariance img (`curvefit`, `--verbose`) | `stat-covariance_desc-t2star+s0_statmap.nii.gz` | `t2s_s0_covar.nii.gz` | `t2star_s0_covariance` |
| s0 img | `S0map.nii.gz` | `s0vG.nii.gz` | `s0` |
| rmse img | `desc-rmse_statmap.nii.gz` | `rmse.nii.gz` | `rmse` |
| confounds tsv | `desc-confounds_timeseries.tsv` | `confounds.tsv` | `confounds` |
| limited s0 img | `desc-limited_S0map.nii.gz` | `s0v.nii.gz` | `s0_limited` |
| limited t2star img | `desc-limited_T2starmap.nii.gz` (s) | `t2sv.nii.gz` | `t2star_limited` |
| combined img | `desc-optcom_bold.nii.gz` | `ts_OC.nii.gz` | `optcom` |
| t2star img | `T2starmap.nii.gz` (s) | `t2svG.nii.gz` | `t2star` |
| data description json | `dataset_description.json` | `dataset_description.json` | `dataset_description` |
| registry json | `desc-tedana_registry.json` | `registry.json` | `files` |

Also `t2smap_call.sh` (the command, as tedana writes it) and an empty `figures/` directory. The
images are float32 (unless noted), gzip-compressed, with nibabel's fresh header from the mask's
affine (sform code 2, qform code 0), the 4D ones with the first echo's voxel sizes and TR; the
table is pandas' (tab-separated, `n/a` for missing values, the shortest digits that read back);
the JSON files are Python's `json.dump(indent=4, sort_keys=True)`, all as tedana writes them.

**One documented difference (both modes).** `dataset_description.json` names its producer
honestly: where tedana writes the host's Python version and the versions of the libraries
installed with it, the replica writes `larmorx-gpl <version>` (tedana 26.0.3's t2smap translated
to Rust, with the results of Python 3.12.16 and the libraries listed) and the library versions
whose arithmetic it reproduces; every other key is tedana's (the host's `uname` included).

## Upstream defects

Corrected by default (D13; [the register](upstream-defects.md)), each with a notice
(`larmorx-gpl: upstream defect TED-.. (tedana) corrected: ...` on standard error, an
`UpstreamDefectWarning` in Python) where the correction changed the result, and reproduced with
`upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`):

| ID | tedana 26.0.3 | larmorx by default |
|---|---|---|
| TED-01 | a NaN among the first echo's temporal means (a NaN sample, or no volume left) makes the dropout mask's 33rd percentile NaN: `ValueError: attempt to get argmax of an empty sequence` | the percentile and the reference voxels from the finite means; a clear error when none is finite |
| TED-02 | `--exclude` accepts an index equal to the number of volumes, then fails with numpy's IndexError | refused with the check's own message, as a larger index is |
| TED-03 | without `--mask`, an empty mask from nilearn (or too few voxels for its threshold) fails later with IndexError or ValueError | refused with a clear error |
| TED-04 | a user mask's affine is never compared with the data's | refused with nilearn's message when they differ |
| TED-05 | only the limited T2* map is capped; the full map (fMRIPrep's T2* map, the combination's weights) keeps values of up to days (up to 239,794 s on fMRIPrep's calls), where tedana 25.0.1, fMRIPrep's own, capped it | the full map gets the limited map's cap (the notice names how many values) |
| TED-07 | `--verbose`'s T2* and S0 variance maps are swapped | each written under its own name |
| TED-08 | a voxel with one good echo is fitted with two, but its curve-fit failure and variances are not reported | its failure marked, its variances written |
| TED-09 | one infinite sample makes the T2* and S0 of every voxel with the same number of good echoes NaN (one least-squares problem for all) | that voxel NaN, the others solved without it |
| TED-10 | an infinite echo time fails in numpy's least squares (LinAlgError) | refused with a clear error |
| TED-11 | `--n-independent-echos 0` fails with ZeroDivisionError | refused with a clear error |
| TED-12 | a single echo fails later with an unrelated numpy error | refused with a clear error |
| TED-13 | `--dummy-scans` that removes every volume fails after numpy's warnings; a negative count shifts `--exclude`'s indices | refused with a clear error |
| TED-14 | `--debug`'s message on the T2* floor counts echoes times voxels as voxels | the voxels counted |
| TED-15 | `--combmode paid` with a voxel whose good echoes are constant (every voxel of a one-volume series) fails with `ZeroDivisionError: Weights sum to zero` | those voxels weighted equally; a one-volume series refused with a clear error |
| TED-16 | `--exclude` that removes every volume: nothing is fitted, and every voxel's T2* (500 ms) and S0 (1) are written with exit status 0 | refused with a clear error |
| TED-17 | a single voxel left for the fit fails with `IndexError: too many indices for array` | fitted |
| TED-18 | when more than 99.5 % of the limited T2* map's values are 0 (voxels of one good echo), its cap is 0 and every limited T2* is set to 0 | no cap of 0 or less, on either map |

TED-06 (a curve fit whose log-linear start lies outside its bounds, T2* <= 0 or S0 below the
voxel's lowest signal, is a silent failure keeping the log-linear estimate: 1,919 of the 1,930
failures on fMRIPrep's four calls) is kept as tedana does it (rule C: refits from a start moved
inside the bounds beat the log-linear residuals in only 15 to 29 % of those voxels), documented
in the register.

**Maths.** By default the replica's mathematical functions are correctly rounded
(`larmorx_core::math`): the float32 logarithm of the log-linear fit and the float64 `exp` and `log`
of the fit's model, the floors, the RMSE, the weights and nilearn's smoothing kernel, and the
powers of SciPy's curve fit. With `reproduce` they are tedana's: numpy's own float32 `log` kernel
(on x86-64 CPUs with AVX2/FMA3, which differs from the correctly rounded one in about 5 % of the
values: NP-03) and the C library's `exp`, `log` and `pow` (glibc 2.35, numpy's path without
AVX-512: GLIBC-07; on AVX-512 CPUs numpy uses its own SVML kernels, NP-04, so tedana's last bits
depend on the CPU and the oracle runs with them off, `NPY_DISABLE_CPU_FEATURES`).

The notices are exact, in both modes: while the run computes, a witness per family of functions
(numpy's float32 log; the float64 functions and the powers) notes whether the other mode's choice
would have given another value on any argument. If one did, the stages from the mask to the
combination run again with the other mode's functions, writing nothing: nothing is named when no
written value and no printed line changes; otherwise each family that changes something alone is
named (NP-03, GLIBC-07) with the values and lines it changes, or both together when only the two
together change something (numpy's float32 log moves a curve fit's start, and on that path glibc's
`exp` differs). The default run therefore takes up to three passes more than the computation
itself (three on fMRIPrep's calls, where each family alone changes values), so that the default
and `reproduce` (which names them the same way) take about as long as tedana at one thread and
3 to 4 times less at 14 (below).

## Validation

- **The record** [`tedana-t2smap-replica`](../validation/tedana-t2smap-replica.md) (the parity
  suite, tedana 26.0.3 as a program through the oracle-output cache, in both library
  environments: numpy 2.5.4 / SciPy 1.18.1 and numpy 2.3.5 / SciPy 1.15.3, OpenBLAS on its
  Haswell kernels, numpy's AVX-512 kernels off, one BLAS thread): 69 cases, 44 bit-identical
  (every output file, header, log line and Python warning, the exit status and the last error
  line), 25 rejected by both alike, 0 failures: fMRIPrep's four multi-echo calls (ds003592,
  ds000210's two tasks and ds000258 with curvefit, ds003592's with loglin), 32 synthetic cases
  (every option and path: no mask, scaled int16, z-concatenated, PAID, `--fitmode ts`,
  `--verbose`, nine echoes, one volume, existing outputs) and the failure cases of every defect;
  each case's D13 fix pass (a defect named exactly where the default changes an output); tedana's
  own step between the releases and its spread (numpy's AVX-512 kernels, threads, tedana 25.0.1
  in fMRIPrep's image) reported; fMRIPrep's own files compared with tedana 26.0.3's and with the
  default's; and random testing's 18 permanent cases (below).
- **Random testing** (`difftest --tools tedana-t2smap`, [the descriptor](../validation/difftest.md)):
  the trial (30 minutes, 3,886 random cases: two to six echoes, every option, data type, file form,
  geometry and special sample) found 59 differences and 11 imprecise notices, all fixed (nibabel's
  extended-precision qform and its headers of new images, NIfTI-2 outputs, series with no volume
  left, numpy's warnings on infinite samples and empty arrays, numpy 2.3.5's float32 percentile
  rank, the maths notices) or registered (TED-16, TED-17, TED-18); every one agrees with the final
  build, 17 kept as permanent cases (`validation/difftest/cases/tedana-t2smap/`); a 10-minute check of
  the final build: 1,268 cases, no difference (one notice, made exact: an 18th permanent case).
- **Beneath**: the command line on 109 cases (help at 19 widths, every error), the numerics bit
  for bit (numpy's reductions, percentiles and float32 log, `lstsq` on tedana's own calls, SciPy's
  `curve_fit` on tedana's own 169,344 calls), nibabel's qform on 4,000 headers.
- **Golden**: larmorx-gpl's 10 cases (`crates-gpl/larmorx-gpl-cli/tests/golden.rs`: each fit type
  and mode, both combinations and naming conventions, the variance maps, no mask, both mask
  methods, dummy scans and an excluded volume, float32 and int16, a z-concatenated file, echo
  times in ms, both library environments, both modes, a failure) on every platform (CI's `gpl`
  job).
- **Memory** (`tests/parity/test_memory_peak.py`, fMRIPrep's ds003592 call): 545 MiB (curvefit)
  and 543 MiB (loglin) at the peak, against tedana's 1,243 and 1,224 MiB.
- **Speed** ([benchmarks](../benchmarks/tedana-t2smap.md); fMRIPrep's four curvefit calls and
  ds003592's loglin call, best of two runs, a shared machine): tedana 26.0.3 takes 29 to 46 s on
  the curve-fit calls and 6 s on the log-linear one at one thread; the replica, whose both modes
  run the maths notices' passes (four in all on these calls), 31 to 45 s at one thread (0.8 to 1.1
  times tedana's speed) and 9 to 13 s at 14 threads (3 to 4 times), and 10 s / 8 s on the
  log-linear call (tedana is faster there). The clean-room original is to be the fast one.
