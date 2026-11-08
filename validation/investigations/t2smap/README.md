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
