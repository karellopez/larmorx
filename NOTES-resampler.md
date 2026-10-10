# Resampler work: checkpoint (2026-10-10, paused on request)

Status notes for `lx.transforms.resample_series` (fMRIPrep's one-shot BOLD resampler). Not part
of the docs; remove when the work is merged.

## Done (committed on this branch)

- **Rust.** `larmorx_interp::ndimage` (SciPy 1.15.2 `map_coordinates` / `spline_filter` /
  `spline_filter1d`, orders 0-5, every mode); `larmorx_core::math::powi` (correctly rounded
  integer powers, original code); `larmorx_transform::nitransforms` (ndcoords, `Affine.map`,
  `DenseFieldTransform.map`, chains) and `larmorx_transform::resample_series`
  (`SeriesResampler`: coordinates once per run, voxel-shift map and Jacobian once per readout,
  one prefilter per volume, parallel over volumes or voxels, `resample_volume` for streaming).
- **Python.** `lx.ndimage`; `lx.transforms` is now a package (`itk.py` unchanged) with
  `load_transforms`, `Affine`, `AffineSeries`, `DenseField`, `TransformChain`,
  `resample_series`, `ensure_positive_cosines`; `.pyi` stubs.
- **Validation.** Suite `resample-series`; record `docs/validation/resample-series.{md,json}`:
  139 cases (standard tier), 136 pass, **all 136 bit-identical** to SciPy, nitransforms or
  fMRIPrep's `ResampleSeries` (real ds000005 runs to native, T1w and MNI res-2 through
  fMRIPrep's own `.h5` warp included), 2 rejected by both, 1 expected divergence (coordinates
  where SciPy's integer index arithmetic overflows: undefined behaviour in SciPy).
- **Docs.** `docs/api/resample-series.md`, `docs/findings/{scipy-ndimage,fmriprep-resampling}.md`,
  `platform-math.md` notes, decision-log entry, NOTICE, PROVENANCE (core, interp, transform),
  UPSTREAM.md / upstream.tsv pins (SciPy v1.15.2, nitransforms 25.1.0), `check_spdx.py` MIXED.
- Gates pass (fmt, clippy, cargo test, maturin develop --release + pytest tests/python, ruff,
  check_spdx).

## Next

1. **Benchmark report** `docs/benchmarks/resample-series.{md,json}` (suite written:
   `python -m larmorx_validation bench resample-series --repeats 3 --threads 1 4 0 --out
   docs/benchmarks`). Run only on a quiet machine; it takes about 40 minutes and up to ~12 GB
   RAM (ds005454). Stopped at its start when the pause was requested. A preliminary
   `--quick --threads 4` run (machine shared with another parity run) gave 4.9-7.8× faster
   than fMRIPrep's `resample_image`, bit-identical; one thread ~4.6×. Then put the measured
   numbers in `docs/api/resample-series.md` ("Speed"), `docs/overview.md` ("Faster and
   better") and the report. (A benchmark bug was fixed after that quick run: fMRIPrep's path
   reoriented the source without a field map, which `ResampleSeries` does not do.)
2. **Test data (larmorx-testdata).** The new collection `synthetic-series-resampling`
   (generator `generators/series_resampling_synthetic.py`, 24 files, 1.2 MB, catalog TOML,
   `docs/sources` page, index entries) could not be written or committed from this worktree
   (the harness blocks writes outside it). It is staged in the session scratchpad
   (`.../scratchpad/td/larmorx-testdata/`) with `.../scratchpad/td/apply_to_larmorx_testdata.py
   <checkout>`, which copies the generator and page, regenerates (deterministic, `--check`
   passes), adds the index entries and updates the two listing lines; then `pytest` and
   commit there. Until then the parity suite needs `LARMORX_TESTDATA_ROOT` pointing at the
   staged copy, and the record names the test data as "5434909ddf09 + staged collection";
   re-run the record after the commit.
3. Then merge `main` again and report.

## Later (not in this brief)

- `ReconstructFieldmap` (B-spline coefficients to Hz on the target grid), multi-echo plan
  reuse (one coordinate mapping for all echoes), per-volume readout in the Python API,
  CIFTI/surface sampling, X5/FSL/AFNI/LTA transform formats.
