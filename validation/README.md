# larmorx-validation

Development-only package (not shipped in the wheel) that checks larmorx against the tools it re-implements, and measures its speed. It produces the **validation record** of the per-tool contract (PLAN.md §4) in `docs/validation/`, the benchmark reports in `docs/benchmarks/`, and the figures of the documentation site ([docs/figures.md](../docs/figures.md)).

```bash
pip install -e ../larmorx-testdata -e "validation[oracles]"     # from the larmorx repo
python -m larmorx_validation parity nifti-io --tier standard --out docs/validation
python -m larmorx_validation bench nifti-io --tier full --out docs/benchmarks
python -m larmorx_validation figures nifti-io --out ../larmorx_documentation/static/figures   # the [figures] extra
pytest tests/parity                                             # the parity suites as tests (tier from LARMORX_PARITY_TIER)
```

## Parity

A **suite** compares larmorx with a reference implementation (an *oracle*) on every relevant file of the test-data catalog (`larmorx-testdata`, a separate repository next to this one). Each case ends as:

| Status | Meaning |
|---|---|
| `pass` | every check is within its threshold |
| `both-error` | larmorx and the reference both reject the input |
| `expected-divergence` | a deliberate, documented difference (the reason is in the report) |
| `fail` | a check is outside its threshold, or only one side rejects the input |

Thresholds follow PLAN.md §11.3; the report lists them. Suites so far:

| Suite | larmorx | Reference |
|---|---|---|
| `nifti-io` | `larmorx.io.load` / `save` | nibabel |
| `resample-series` | `lx.transforms.resample_series`, `lx.ndimage` | fMRIPrep's `ResampleSeries`, nitransforms, SciPy (in-process) |
| `itk-geometry` | ITK's reading of NIfTI geometry | ITK 5.4.5 (through ANTsPy) |
| `ants-apply-transforms` | `larmorx ants antsApplyTransforms` | antsApplyTransforms (ANTsPy) |
| `afni-tshift` | `larmorx afni 3dTshift` | the AFNI 25.2.09 binary |
| `ants-image-math`, `ants-threshold-image`, `ants-multiply-images`, `ants-smooth-image`, `ants-resample-image-by-spacing`, `ants-resample-image`, `ants-n4`, `ants-denoise` | `larmorx ants ImageMath` / `ThresholdImage` / `MultiplyImages` / `SmoothImage` / `ResampleImageBySpacing` / `ResampleImage` / `N4BiasFieldCorrection` / `DenoiseImage` | the ANTs 2.6.5 binaries (`parity/ants_programs.py`, a harness for every ANTs program run as a binary; `parity/ants_threaded.py` for the programs whose result depends on ANTs' thread count) |

### The oracle-output cache

Every run of an original program goes through a content-addressed cache
(`oracle_cache/`, [docs/validation/oracle-cache.md](../docs/validation/oracle-cache.md)): a run
seen before (the same oracle build, inputs, arguments and settings) is served from
`<workspace>/oracles/cache` (`LARMORX_ORACLE_CACHE`) instead of running again, so a re-run of a
record runs only larmorx. Runs whose result the original itself varies (the clock, races, memory
it never wrote, thread bands of programs that vary) always run. `--oracle-cache use|refresh|off`
chooses the mode; `python -m larmorx_validation oracle-cache verify --sample N` re-runs cached
entries to check them. A suite's runner calls `oracle_cache.run(argv, env=..., policy=...)` in
place of `subprocess.run`, with the `Policy` that says the run's thread count, its family of
environment variables and, from the case, why it must never be cached (`case_never`).

```bash
python -m larmorx_validation parity ants-ai --tier standard --out docs/validation   # uses the cache
python -m larmorx_validation parity ants-ai --tier standard --oracle-cache refresh    # every run again, compared
python -m larmorx_validation oracle-cache verify --sample 20 --jobs 4                 # the drift detector
python -m larmorx_validation oracle-cache stats
pytest validation/tests                                                               # the cache's own tests
```

## Benchmarks

Median of repeated runs after a warm-up, with the environment recorded. Inputs are the files tagged `benchmark` in the catalog plus a typical anatomical, functional and template image. Build larmorx in release mode first (`maturin develop --release`); debug builds are many times slower.

## Layout

```
src/larmorx_validation/
  environment.py      versions, platform, CPU, commits of larmorx and larmorx-testdata
  report.py           Markdown/JSON helpers
  parity/             harness (cases, checks, statuses), comparisons, rendering, suites
  oracle_cache/       the original programs' runs, computed once: key, store, policy, replays, verify
  bench/              timing harness, benchmark definitions
  figures/            the documentation site's figures: style, records, charts, brain images, cache, one module per page
tests/                the package's own tests (the oracle cache), in the Python gates
investigations/       the tools behind findings and measurements (oracle-cache/: run every suite in a mode, compare records)
```
