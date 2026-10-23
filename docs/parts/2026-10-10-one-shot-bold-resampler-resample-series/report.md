# Final report: One-shot BOLD resampler (resample_series)

Agent `a5152712e192ab1b7`, 2026-10-10. The agent's report to the orchestrator, verbatim.

Paused at a safe checkpoint, as you asked. All gates passed on the last code commit: fmt, clippy, cargo test, `maturin develop --release` + pytest tests/python (183 passed), ruff, check_spdx. The notes-only commit after it needed no gates. The worktree is clean and nothing of mine is running: I stopped my benchmark and its wait script the moment it started.

**Where the work stands:**
- **Done:** the resampler, its Python wrapper and the parity record. Every compared output is bit-identical to fMRIPrep.
- **Not done:** the benchmark report. A preliminary run shows 4.9–7.8× faster than fMRIPrep on 4 threads.
- **Blocked:** the `larmorx-testdata` commit. The harness won't let me write or commit outside my worktree.

**Committed on branch `worktree-agent-a5152712e192ab1b7`** (main merged in; one conflict in `upstream.tsv`, resolved by keeping both sets of pins):
- `02453f6`: the code.
  - `larmorx_interp::ndimage` is a replica of SciPy 1.15.2's `map_coordinates`, `spline_filter` and `spline_filter1d`, all orders and modes.
  - `larmorx_core::math::powi` is a correctly rounded integer power. It is original code, not a port, and `PROVENANCE.md` says so. It matched exact arithmetic on 26,000 checked values; glibc's `pow` differs on 6 of them.
  - `larmorx_transform::nitransforms` and `::resample_series` hold the chain mapping and the fused resampler.
  - Python: `lx.ndimage`, and `lx.transforms` is now a package (`itk.py` unchanged) with `load_transforms`, the chain types and `resample_series`.
  - Also in it: bindings, `.pyi` stubs, tests, the parity suite, NOTICE, PROVENANCE, and UPSTREAM pins for SciPy v1.15.2 and nitransforms 25.1.0.
- `78e7057`: the validation record, `docs/api/resample-series.md`, two findings pages (`scipy-ndimage.md`, `fmriprep-resampling.md`), the decision-log entry and the benchmark suite. The choice recorded there: the API is `lx.transforms.resample_series` (no `lx.xfm`), and SciPy's interpolation is `lx.ndimage`.
- `f094729`: `NOTES-resampler.md`, saying what is done and what is next.

**Parity (standard tier):** 139 cases.
- 136 pass, and all 136 are bit-identical to SciPy, nitransforms or fMRIPrep's `ResampleSeries`. That includes real ds000005 runs resampled to native, T1w and MNI res-2 through fMRIPrep's own `.h5` warp.
- 2 cases are rejected by both, as expected.
- 1 is an expected divergence: for coordinates around 1e19 and beyond, SciPy's index arithmetic overflows and it reads outside its array. larmorx returns NaN there instead.

**What remains:**
1. **Benchmark report.** Run `python -m larmorx_validation bench resample-series --repeats 3 --threads 1 4 0 --out docs/benchmarks` on a quiet machine. It takes about 40 minutes and up to about 12 GB of RAM.
   - The preliminary run, on a shared machine, was bit-identical except one MNI row. That row was a bug in the benchmark itself, not the resampler; it is fixed.
   - One-thread rough timing: about 4.6× faster.
   - Then the measured numbers go into the API page and `docs/overview.md`.
2. **`larmorx-testdata` commit (needs you or the user).** The new 24-file `synthetic-series-resampling` collection is staged in the session scratchpad (`scratchpad/td/larmorx-testdata/`).
   - To apply it: `python scratchpad/td/apply_to_larmorx_testdata.py /home/karelo/VScode_projects/super_fmriprep/larmorx-testdata`. It copies the generator, regenerates the data deterministically, adds the index entries and updates two listing lines; then run pytest and commit there.
   - I tested it on a fresh copy of the checkout: the testdata repo's pytest passes (10 passed).
   - Until it lands, the parity suite needs `LARMORX_TESTDATA_ROOT` pointing at the staged copy. The record names the test data as "`5434909ddf09` + staged collection" and should be re-run after the commit.
3. Merge `main` again before the final report.
4. **Later, outside this brief:**
   - `ReconstructFieldmap` (rebuilding the field map in Hz from fMRIPrep's B-spline coefficients) is not ported yet.
   - Multi-echo: let all echoes share one coordinate mapping.
   - Per-volume readout times in the Python API.
   - CIFTI and surface sampling.
   - Non-ITK transform formats (X5, FSL, AFNI, LTA).

One correction: earlier I treated an ANTs oracle build as running. My `pgrep` was matching its own shell command; no build was running. The load was another agent's AFNI parity run.
