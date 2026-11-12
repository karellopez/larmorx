# tedana's `t2smap`: probes and numerics (the replica part, 2026-11-08)

What tedana 26.0.3's `t2smap` computes, and the numpy and SciPy routines it reaches, measured for
the replica (`larmorx-gpl`'s tedana replica, later) and the spec (`specs/t2smap.md`, later). The
findings these scripts produced are in `docs/findings/tedana-t2smap.md` (source-derived) and the
part's notes; the numbers below are this machine's (Linux x86-64, AMD, AVX-512).

## `probes/` (the inventory, relay 1)

| Script | What it shows |
|---|---|
| `versions.py` | the library versions and OpenBLAS builds of a Python environment |
| `run_fmriprep_call.sh <env-bin> <out> <fittype>` | tedana's `t2smap` on fMRIPrep's real call (ds003592-sub-280's working directory: the three resampled echoes, the dilated mask, TEs 14, 29.96, 45.92 ms), with `/usr/bin/time -v` (time, peak memory) |
| `compare.py <a> <b> ...` | two outputs' data: identical voxels, largest differences |
| `compare_envs.py` | the library step: tedana 26.0.3 under the shared environment (numpy 2.3.5, SciPy 1.15.3) against `oracles/python-newest` (numpy 2.5.4, SciPy 1.18.1) on that call: `loglin` bit-identical, `curvefit` not (SciPy's TRF) |
| `headers.py <dir>` | every NIfTI output's type, shape and header fields (`t2smap` writes fresh nibabel headers: sform code 2, qform code 0) |
| `edge_probe.py` | failure cases on tiny synthetic echoes (a NaN in the first echo: `argmax of an empty sequence`; an `--exclude` index equal to the number of volumes: `IndexError`; no mask found) |
| `log32_all.py`, `simd_probe.py` | first look at numpy's `float32` `log` and `float64` `exp` per dispatch path (superseded by `numerics/`) |

- `probes/numpy_average_orders.py` (relay 5b): numpy's orders behind the optimal combination
  (PAID's `divide(f32, f32, out=f64)` runs in float32; `average`'s weights' sum is pairwise for
  `fitmode ts`'s echo-innermost weights, the products in index order). `probes/epi_mask_failures.py`
  (relay 5b): how t2smap ends without `--mask` when nilearn finds no mask (TED-03).

## `numerics/` (relay 2)

`check_numerics.py` writes seeded cases (inputs and numpy's or SciPy's results) and runs the Rust
program `numpy_check` (`crates/larmorx-core/examples/numpy_check.rs`) on them, which computes
each with `larmorx_core::numpy` and `larmorx_core::math` and compares bit for bit:

```
cargo build --release -p larmorx-core --example numpy_check
OPENBLAS_CORETYPE=Haswell scripts/orchestration/boxed.sh 16 <env>/bin/python -P \
    validation/investigations/t2smap/numerics/check_numerics.py \
    --checker <target>/release/examples/numpy_check --work <empty dir> --logf --threads 12
```

(about 4 minutes per environment with `--logf`, 3 GB.) It covers:

- the reductions `t2smap` reaches, over the shapes and axes it uses (the temporal mean of
  `(voxels, echoes, volumes)`, the means over echoes, the RMSE table's `nansum`/`nanstd` over
  voxels and volumes, the combination's weighted `average` over echoes) and edge cases: lengths
  around numpy's pairwise blocks (7, 8, 9, 127, 128, 129, 8193, 50000), axes of length 1 (a single
  volume makes the sum over echoes or voxels pairwise), NaN, signed zeros, infinities; `mean`,
  `var`, `std`, `nansum`, `nanstd` in `float32` and `float64`; `average` with weights (and its
  `ZeroDivisionError`); `percentile(method="higher")` (the adaptive mask's 33rd percentile);
  `nanpercentile` (linear; its `float32` result when the first column has no number); SciPy's
  `scoreatpercentile(..., "lower")` (the T2* cap);
- `float64` `exp` and `log` with numpy's AVX-512 kernels off (`NPY_DISABLE_CPU_FEATURES`): glibc
  2.35's, through the old symbols numpy's wheels bind (`exp@GLIBC_2.2.5`, `log@GLIBC_2.2.5`: a
  negative logarithm is C's positive `NAN`); with them on (the default here), how far numpy's
  SVML kernels are from glibc's and from the correctly rounded results;
- numpy's `float32` `log` on all 2^32 inputs, AVX-512 and AVX2/FMA3 paths (its own kernel), and the
  counts of differences among numpy's kernel, glibc's `logf` and the correctly rounded `logf`.

Results (2026-11-08, `results.txt`; the same in both environments, numpy 2.3.5 / SciPy 1.15.3 and
numpy 2.5.4 / SciPy 1.18.1):

- every reduction and percentile case identical (109 cases each of `sum`, `mean`, `std` in both
  types, `var`, `nansum`, `nanstd` in `float32`; 7 `average`; 51 `percentile`; 6 `nanpercentile`;
  26 `scoreatpercentile`);
- `float64` `exp` and `log`, AVX-512 off: identical to `exp_glibc` and to `log_glibc` with C's `NAN`
  for negative arguments, on 6,000,008 and 4,000,009 arguments;
- AVX-512 on (numpy's SVML kernels): `exp` differs from glibc's on 81,649 of 2,000,000 arguments in
  `[-800, 0]` (4.1 %; tedana's `exp(-TE/T2*)`), 92,295 in `[-20, 20]` (4.6 %), and from the
  correctly rounded one on as many; `log` on 2,083 of 2,000,000 in `[0, 10]` (0.10 %);
- numpy's `float32` `log`: `logf_numpy` identical on all 2^32 inputs, AVX-512 and AVX2/FMA3 paths;
  it differs from the correctly rounded `logf` on 78,427,614 of the 2,139,095,039 positive finite
  floats (3.7 %, at most 4 ulp) and on 10,947,841 of the 201,326,593 floats in `[1, 2^24]` (5.4 %,
  at most 3 ulp: tedana's `log(|x| + 1)`); glibc's `logf` differs from the correctly rounded one on
  416,908 and 167,180 of them.

`lstsq_columns.py` measures whether `numpy.linalg.lstsq`'s solution for one column of B (one voxel
of tedana's log-linear fit, all voxels of one adaptive-mask value solved at once by LAPACK's
`dgelsd`) depends on the other columns and on BLAS threads. It does: about a fifth of the columns
solved alone differ in their last bits from the same columns in the full solve (239 and 235 of 300
identical, m = 600 rows), a shift of the columns by 1 to 3 places changes 2 of 5,003, and 2, 4 or
14 BLAS threads change 4 to 14 of 5,003 (tedana's `--n-threads`); with one volume (m = echoes) every
column is the same in every arrangement. So the replica follows `dgelsd` on the whole batch with
OpenBLAS's kernels at one thread (fMRIPrep's call), column blocking included.

### numpy's `lstsq` (relay 3)

`larmorx_transform::numpy_linalg::lstsq` follows `numpy.linalg.lstsq` for tedana's two-column
design: LAPACK's `dgelsd` (the QR path for `m >= 3` rows, the direct bidiagonalization for two
rows, `dlalsd`'s small path with `dbdsqr` on the right-hand sides) and OpenBLAS's Haswell kernels
on long vectors (`dgemv_t` in row blocks of 2048 with its 4×4, 4×2 and 4×1 column kernels,
`dger`/`daxpy` with the fused first `16·⌊m/16⌋` rows, `drot`, `dgemm`, the x87 `dnrm2`). A
column's result depends only on its own values and on its kernel class (the last `nrhs mod 4`
columns), which is why tedana's results change with the batch and with BLAS threads; larmorx gives
numpy's one-thread result at any thread count.

| Script | What it shows |
|---|---|
| `check_lstsq.py --checker <lstsq_check> --work <empty dir> [--real <dir> ...]` | numpy's `lstsq` at one BLAS thread against larmorx's at one and four threads, bit for bit: `tedana` (2 to 5 echoes, 1 to 1366 volumes, 1 to 2001 voxels), `random` (2 to 4099 rows around the kernels' blocks), `edge` (zero columns and rows, collinear and all-zero designs, scalings below `SMLNUM` and above `BIGNUM`, NaN, infinities, signed zeros, `rcond` values, a non-finite design, no column) and `real` (tedana's own calls) |
| `dump_lstsq_calls.py --out <dir> --workdir <scratch> -- <t2smap arguments>` | runs tedana's `t2smap` command line in-process and records every `numpy.linalg.lstsq` call it makes (inputs, numpy's results, BLAS threads) |
| `run_lstsq_check.sh <lstsq_check> <scratch>` | both of the above in both environments, on fMRIPrep's three multi-echo `t2smap` calls (ds003592 sub-280; ds000210 sub-02's cuedSGT and rest runs) |

```
cargo build --release -p larmorx-transform --example lstsq_check
scripts/orchestration/boxed.sh 24 validation/investigations/t2smap/numerics/run_lstsq_check.sh \
    <target>/release/examples/lstsq_check <empty scratch dir>
```

(about 6 minutes, 3 GB.) Results (2026-11-08, from 7c3fa5f0's code; the same in both
environments, numpy 2.3.5 with LAPACK 3.12.0 and numpy 2.5.4 with LAPACK 3.12.1): every case
identical, 550 per environment: `tedana` 274/274, `random` 217/217, `edge` 53/53 (numpy 2.3.5
raises `LinAlgError` for a non-finite design too, through `DLASCL`'s argument check on a NaN norm,
printing OpenBLAS's `** On entry to DLASCL parameter number 4 had an illegal value`), `real` 6/6:
tedana's two calls per run (the voxels fitted with two echoes, then with three: 595 and 52,231 on
ds003592, 3,169 and 28,356 on ds000210 cuedSGT, 9,718 and 47,486 on rest), made at one BLAS thread.
Speed on ds003592's 600 × 52,231 call: numpy 0.35 s; larmorx about 0.4 s at one thread, 0.2 s at
14 (each fused multiply-add is a correctly rounded `mul_add`; no SIMD in this replica).

### SciPy's `curve_fit` (relay 4)

tedana's `--fittype curvefit` (fMRIPrep's default) calls `scipy.optimize.curve_fit(monoexponential,
te, y, p0=loglin start, bounds=((min(y), 0), (inf, inf)))` per voxel: with bounds that is
`least_squares`' trust-region reflective method with 2-point differences, the SVD of the augmented
Jacobian at every iteration and the covariance from the SVD of the final Jacobian. larmorx's port
is `larmorx_optim::curve_fit2` (with `larmorx_transform::numpy_linalg::{gesdd, blas}`).

| Script | What it shows |
|---|---|
| `check_curve_fit.py --checker <curve_fit_check> --work <empty dir> [--real <dir> ...]` | SciPy's and numpy's own results against larmorx's, bit for bit: `dot` and `matvec` (numpy's `dot` of vectors and of C- and F-contiguous matrices on `least_squares`' shapes), `svd` (`scipy.linalg.svd` and `numpy.linalg.svd` of `m × 2` matrices, `full_matrices=False`: 2 to 4099 rows, scalings below `SMLNUM` and above `BIGNUM`, rank one, zero columns, signed zeros, not finite), `fit` (tedana-like voxels: 2 to 5 echoes, 1 to 600 volumes, three noise levels, the log-linear start), `edge` (every exit: starts outside, on or far from the bounds, infinite and NaN starts and data, residuals not finite at the start, flat, rising, tiny, huge, negative and zero signals, T2* from 0.05 ms to 10^7 ms, two and three residuals, perfect fits, the evaluation budget) and `real` (tedana's own calls); it prints the exits SciPy reached |
| `dump_curve_fit_calls.py --out <dir> --workdir <scratch> -- <t2smap arguments>` | runs tedana's `t2smap` command line in-process and records every `scipy.optimize.curve_fit` call (echo times, float32 signal, start, bounds, SciPy's result as tedana got it) and the Python warnings they raised |
| `run_curve_fit_check.sh <curve_fit_check> <scratch> ["shared newest"]` | both of the above in both environments, on fMRIPrep's four multi-echo `t2smap` calls (ds003592 sub-280; ds000210 sub-02's cuedSGT and rest runs; ds000258 sub-21262's rest run, four echoes) |
| `compare_curve_fit_envs.py <scratch>` | the library step on those calls: SciPy 1.15.3/numpy 2.3.5 against SciPy 1.18.1/numpy 2.5.4 |
| `search_rare_exits.py --fits N --seed S` | random voxels and starts, looking for `curve_fit`'s rare exits (`intersect_trust_region`'s errors, an SVD of a Jacobian that is not finite) |

```
cargo build --release -p larmorx-optim --example curve_fit_check
scripts/orchestration/boxed.sh 16 validation/investigations/t2smap/numerics/run_curve_fit_check.sh \
    <target>/release/examples/curve_fit_check <empty scratch dir>
```

(about 6 minutes per environment, 4 GB.) Results (2026-11-08, from 7bb40be4's code; numpy's
AVX-512 kernels off, OpenBLAS's Haswell kernels, one BLAS thread):

- every record identical in both environments, 169,785 each: `dot` 13/13, `matvec` 41/41, `svd`
  69/69 (SciPy) and 67/67 (numpy: LAPACK 3.12.0 with 2.3.5, 3.12.1 with 2.5.4), `fit` 144/144,
  `edge` 98/98, and **tedana's own calls 169,344/169,344**: ds003592 52,826, ds000210 cuedSGT
  31,525 and rest 57,204, ds000258 27,789 (every voxel's fit with two to four echoes);
- the exits SciPy reached (SciPy 1.18.1): status 2 on 150,148 fits, 4 on 13,114, 3 on 3,046, 1 on
  1,326 (7 of them with the covariance warning: two residuals), a start outside the bounds on
  1,925 (1,919 of them tedana's: the log-linear start below the voxel's smallest signal or a
  T2* that is not positive), the evaluation budget on 13 (11 of them tedana's), an SVD of a
  Jacobian that is not finite on 5, residuals not finite at the start on 1; no voxel reached
  `intersect_trust_region`'s two errors (none of 30,000 random fits either);
- the Python warnings tedana's calls raised: none, on all four calls;
- the library step (`compare_curve_fit_envs.py`): the two environments give different last bits
  in most fits (parameters differ in 39,211 of ds003592's 52,826 fits, median relative 9e-10,
  largest 0.70; 31,007, 55,377 and 23,038 on the other three, largest 3.5 on ds000258), never a
  different outcome: SciPy 1.18.1 returns C-ordered SVD factors and 1.15.3 Fortran-ordered ones,
  so numpy's products with them run `dgemv_n` or `dgemv_t` (`larmorx_optim::Scipy`);
- speed: larmorx fits the 169,785 records in 33.8 s at one thread (0.2 ms a fit; SciPy about 1
  ms), 4.6 s at 12, with the same bits.

## `replica/` (the replica program, relays 5a and 5b)

The replica (`crates-gpl/larmorx-gpl-tedana`, the `larmorx-gpl tedana t2smap` program) against
tedana 26.0.3 itself:

```
cd crates-gpl && cargo build --release -p larmorx-gpl-tedana --example t2smap_stages \
    --example t2smap_parse && cd ..
OPENBLAS_CORETYPE=Haswell scripts/orchestration/boxed.sh 32 <env>/bin/python -P \
    validation/investigations/t2smap/replica/check_stages.py --stages \
    <target>/release/examples/t2smap_stages --work <empty dir> --libraries newest --jobs 8
<env>/bin/python -P validation/investigations/t2smap/replica/check_parser.py \
    --parse <target>/release/examples/t2smap_parse --work <empty dir>
```

- `check_stages.py` runs tedana's `t2smap` in-process through `tedana_stages.py` (which wraps
  `utils.make_adaptive_mask`, `decay.fit_decay` and `decay.fit_decay_ts` and saves what they
  return) and the replica's `t2smap_stages` (`LARMORX_UPSTREAM_DEFECTS=reproduce`, numpy's AVX-512
  kernels off: `NPY_DISABLE_CPU_FEATURES`, one BLAS thread), and compares the adaptive mask, the
  T2* and S0 maps, the curve fit's failures, variances and covariance bit for bit (tedana's
  variance maps swapped back, TED-07) and the messages up to "Computing T2* map" line by line.
  Cases: fMRIPrep's four multi-echo calls of the oracle runs (their `t2smap_node/command.txt`;
  curvefit, and loglin for ds003592) and 13 synthetic series (seeded; float32 and scaled int16,
  options and data types). `--libraries shared` runs the shared environment's tedana (numpy
  2.3.5, SciPy 1.15.3) against `LARMORX_TEDANA_LIBRARIES=shared`. Results (2026-11-08): 18 of 18
  identical in both environments (logs `oracles/logs/t2smap-1-relay5a-stages-*.log`).
- `check_parser.py`: tedana's parser in-process (`COLUMNS` per case) against `t2smap_parse`: 109
  cases (help at 19 widths, every argparse and `is_valid_file` error, abbreviations, `=` forms,
  short-option clusters, negative numbers, `--`): all identical.
- `check_failures.py --t2smap <env>/bin/t2smap --replica <target>/release/larmorx-gpl --work
  <dir>`: tedana's failure cases on tiny seeded echoes (TED-01, 02, 04, 10, 11, 12, 13 and three
  clear errors of tedana's own): with `reproduce` the replica ends as tedana does (status, last
  line of standard error), by default it names the defect. 11 cases, all as expected; 24 since
  the clean-room part's relay RS1 (2026-11-12: TED-31, TED-46, the chained tracebacks, the
  temporal means' and the cast's warnings, the thresholds' printing, `--fitmode ts` with no
  volume), which the parity suite's `fail/` cases run against tedana in full.
- `rs1_probe.py <work dir> --t2smap <launcher-newest>/t2smap --replica <larmorx-gpl>
  [--only <case>,...] [--show]` (relay RS1, 2026-11-12; run with the newest environment's Python,
  boxed): tedana's program run **directly** (not through the oracle-output cache) and the replica
  in both modes on a small seeded phantom: every line of standard error (warnings with their
  source lines, chained tracebacks), the exit status and every file's bytes, tedana against the
  replica's `reproduce`, and the default's notices. Result: 21 of 21 identical (log
  `oracles/logs/t2smap-cr-rs1-probe2.log`; before the fixes 13 differed, `...-probe1.log`).
  `--registry` runs tedana in process with `warnings`' filter changes traced (`##FILTERS`
  lines): Python's warnings registry is reset when the libraries' versions are gathered and when
  the confounds table is written (log `oracles/logs/t2smap-cr-rs1-registry.log`).
- `check_formats.py` (relay RS2, 2026-11-12): the inputs and cases of the parity suite's
  `formats/` category (ANALYZE 7.5 pairs of SPM2 in both byte orders and every stored type a fit
  reads, the scale factor and intercept, the gl/cal ranges, the origin, a `.mat` beside the pair
  (`mat`, `M`, neither), compressed pairs, voxel sizes of 0 and below 0, `.img` names, a
  z-concatenated file and masks of the format; MGH/MGZ of uint8, int16, uint16, int32, float32,
  an oblique affine, the footer's repetition time or none, `goodRASFlag` 0, a z-concatenated
  file, three formats at once; and nibabel's failures: a short data file first and later, an
  unknown ANALYZE type, MGH dimensions of 0, an MGH version other than 1, a `.mgh.gz` name; a
  head-shaped series for nilearn's computed mask of an ANALYZE and an MGZ reference):
  `make(<folder>)` writes them, `CASES` names them (20; the suite runs them in both environments,
  `reproduce` bit for bit, the default pass). Result: 20 of 20 agree, 12 complete and 8 refused
  alike (log `oracles/logs/t2smap-cr-rs2-formats4.log`).
- `rs2_replayed_formats.py <work> <larmorx-gpl> <scratch>` (relay RS2): the replica
  (`reproduce`) against tedana's files on the replayed random-testing cases of the clean-room
  part whose inputs are ANALYZE or MGH (`<work>/f<n>/{inputs,tedana/out}` as
  `../t2smap-original/replay.py --keep` leaves them): 104 cases, every file tedana wrote
  identical (the host's `dataset_description.json` apart), the 30 tedana failed failing too (log
  `oracles/logs/t2smap-cr-rs2-replayed-formats.log`).
- `rs2_descriptor_formats.py --cases N` (relay RS2): the replica's random-testing descriptor on
  its ANALYZE and MGH draws only, each through the descriptor's own comparison: 30 of 30 agree
  (log `oracles/logs/t2smap-cr-rs2-descriptor-formats.log`).
- `check_outputs.py --replica <target>/release/larmorx-gpl --work <empty dir> --libraries
  newest|shared` (relay 5b; build with `cargo build --release --manifest-path crates-gpl/Cargo.toml
  -p larmorx-gpl-cli`): tedana's whole `t2smap` program (`t2smap._main(args)` in the environment's
  Python) against `larmorx-gpl tedana t2smap` with `reproduce`, every output file: each image's
  decompressed bytes (every header field and value bit for bit; gzip's own bytes are not
  compared), the confounds table's text, the registry, `dataset_description.json` except the
  fields that describe the host and Python (decision D6), `t2smap_call.sh`; the logger's lines,
  the exit status and the last error line. Cases: those of `check_stages.py` and 15 more for the
  paths after the fit (no `--mask`: nilearn's `compute_epi_mask` on float32, scaled int16 and
  z-concatenated data; `--combmode paid` in both fit modes; `--verbose` with `--convention orig`
  and `--prefix`, and with `--fitmode ts`; nine echoes per volume and over all volumes; one
  volume; constant voxels with PAID, TED-15; an image nilearn finds no mask in, TED-03; an
  existing output without and with `--overwrite`). Results (2026-11-08): 33 of 33 identical in
  the shared environment, the 28 synthetic and the 5 real in the newest (logs
  `oracles/logs/t2smap-1-relay5b-outputs-*.log`).

## `defects/` (relay 5b: measurements behind rules D)

- `ted05_full_map_cap.py`: with tedana's own functions on fMRIPrep's four multi-echo calls
  (curvefit), the full T2* map's values above ten times the cap tedana 26.0.3 applies to the
  limited map only, and what capping them changes in the combined series. Result (2026-11-08,
  log `oracles/logs/t2smap-1-relay5b-ted05.log`): 2 to 34 voxels per call (full-map maxima 1,574
  to 239,794,244 ms), the combined series changes in those voxels only (median relative change
  0.01 to 0.9 %, at most 2.5 % to 110 %); tedana 25.0.1's t2smap capped the full map too (by its
  own percentile: the same voxels). Decision: rule A (TED-05): by default the full map gets the
  cap.
- `ted06_feasible_start.py`: where tedana's curve fit rejects its start (SciPy's "x0 is
  infeasible" for a log-linear start outside the bounds `((min(data), 0), (inf, inf))`), why, and
  what `curve_fit` from a start clipped into the bounds gives. Result (log
  `oracles/logs/t2smap-1-relay5b-ted06.log`): 1,919 of the 1,930 failures of the four calls are
  infeasible starts, every one with T2* <= 0 (the signal rising with the echo time; 5 also with
  S0 below the smallest sample); refits
  converge in all, but their residuals are below the log-linear estimate's in only 15 to 29 %
  (the data do not decay). Decision: rule C (TED-06: tedana's design, kept and documented: the
  voxels stay failures with the log-linear estimate).

## Random testing and speed (relay 7b)

- `replica/check_affine.py --example <target>/release/examples/nibabel_affine --work <empty dir>`
  (build: `cargo build --release --manifest-path crates-gpl/Cargo.toml -p larmorx-gpl-tedana
  --example nibabel_affine`): the affine the replica reads (larmorx-io's
  `NiftiHeader::best_affine_nibabel`: nibabel's `get_qform` in x87 extended precision) against
  nibabel's `img.affine` on 4,000 generated NIfTI-1 and NIfTI-2 headers (axis permutations and
  flips, rational and random quaternions, `w` near its threshold, tiny rotations, both `qfac`,
  sform and qform codes): all identical (2026-11-08). Random testing had found the outputs'
  `srow` holding `-0.0` where tedana's hold `+0.0`.
- `recheck_trial.py <night>` (inside `boxed.sh 16`, `LARMORX_GPL_BIN` set): every case of a
  random-testing night of `tedana-t2smap` that did not agree, drawn again from the night's seed
  and run with the current replica: the trial's triage (2026-11-08: 70 of 70 agree with the final
  build; logs `oracles/logs/t2smap-1-relay7b-recheck*.log`).
- `bench_calls.py --work <empty dir> --threads 14 --repeat 3 --json docs/benchmarks/tedana-t2smap.json`
  (inside a box): fMRIPrep's multi-echo calls, tedana 26.0.3 (once, through `oracle_cache.run`,
  never stored) against the replica in `reproduce` and in its default mode at one and 14
  threads (the best of three), every replica run's files checked: `docs/benchmarks/tedana-t2smap.md`.
- `replay_finding.py <finding.json | case:N> [seed] [--original] [--shared] [--all]` and
  `replay_fix_pass.py <finding.json | case:N> [seed] [--original]`: one random-testing case run
  again (tedana against the replica in `reproduce`: statuses, last errors, the messages that
  differ, the files; the replica's default against `reproduce`: the defects named and the values
  that differ), the trial's triage tools (2026-11-08).
