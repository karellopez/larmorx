# larmorx-validation

Development-only package (not shipped in the wheel) that checks larmorx against the tools it re-implements, and measures its speed. It produces the **validation record** of the per-tool contract (PLAN.md §4) in `docs/validation/` and the benchmark reports in `docs/benchmarks/`.

```bash
pip install -e ../larmorx-testdata -e "validation[oracles]"     # from the larmorx repo
python -m larmorx_validation parity nifti-io --tier standard --out docs/validation
python -m larmorx_validation bench nifti-io --tier full --out docs/benchmarks
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

## Benchmarks

Median of repeated runs after a warm-up, with the environment recorded. Inputs are the files tagged `benchmark` in the catalog plus a typical anatomical, functional and template image. Build larmorx in release mode first (`maturin develop --release`); debug builds are many times slower.

## Layout

```
src/larmorx_validation/
  environment.py      versions, platform, CPU, commits of larmorx and larmorx-testdata
  report.py           Markdown/JSON helpers
  parity/             harness (cases, checks, statuses), comparisons, rendering, suites
  bench/              timing harness, benchmark definitions
```
