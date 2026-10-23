# Final report: Clean-room lx.mri.hmc from spec

Agent `af23415ec4fb32914`, 2026-10-10. The agent's report to the orchestrator, verbatim.

## lx.mri.hmc: report (worktree branch `worktree-agent-af23415ec4fb32914`, not pushed)

**Status:** finished and committed, with all gates passing. The parity suite runs 134 cases: 121 pass, 11 are rejected by both programs, 1 is an expected divergence and **1 fails by a small margin** (details under Parity).

### What was built
- **Crate `crates/larmorx-mri`** (Apache-2.0, `#![forbid(unsafe_code)]`), module `hmc`.
  - Volumes, rigid compose/decompose, subsampled reference grids and trilinear/sinc/spline/nearest kernels.
  - Cost functions: normcorr as in mcflirt, with float32 sums in three levels and mcflirt's count N; also leastsquares, corratio, woods, mutualinfo and normmi.
  - Line-search sweep and the 8/4/4 mm schedule (optional sinc stage 4), with the stage-1 chain initialisation.
  - Options: `-meanvol` (also with `-reffile`), in-plane mode (`-2d`/`-fov`), `-dof` 6–12, `-init`, `-fudge`.
  - Outputs: `.mat`, `.par`, `.rms` files, the resampled series, `-stats` and `mean_reg`. Output naming and headers follow mcflirt's rules.
  - World transforms: RAS matrices as nitransforms expects (checked against nitransforms 25.1.0) and ITK/LPS matrices.
  - Threads: stage 1 is parallel over the slices of each cost evaluation, stages 2–4 over volumes. Results are bit-identical for any thread count (there is a test for this).
- **CLI** `larmorx mri hmc` accepts mcflirt-style arguments and follows mcflirt's exit-code rules. `-gdt` is rejected with a message.
- **Python** `lx.mri.hmc(...)` returns an `HmcResult`: `matrices`, `affines`, `itk`, `params`, `rms_abs/rel`, `fd`, `image`, `mean`, plus `save_mats/par/itk` and `lx.mri.read_mats`. The GIL is released during the work, and `.pyi` stubs are included.
- **Docs and records:**
  - `docs/api/mri-hmc.md` (option-mapping table, status `validated`)
  - `docs/validation/mri-hmc.{md,json}`
  - `docs/benchmarks/mri-hmc.{md,json}`
  - `crates/larmorx-mri/PROVENANCE.md`
  - README, overview, architecture and index entries
- **Validation code:** parity suite `validation/src/larmorx_validation/parity/mri_hmc.py`, band runner `mri_hmc_band.py` and benchmark `bench/mri_hmc.py`.
- **Tests:** `tests/python/test_mri.py` and `tests/parity/test_mri_hmc.py`.
- **Band data:** 127 perturbation runs of mcflirt, saved in `oracles/fsl-6.0.7/mcflirt/band/` with `band.json` (not committed). Logs are `oracles/logs/fsl-6.0.7-mcflirt-band-*.log`.

### Parity (standard tier, record `docs/validation/mri-hmc.md`)

**fMRIPrep's command on nine real runs, larmorx against mcflirt:**
- 95th-percentile matrix RMS deviation: 0.014–0.030 mm. mcflirt against itself with slightly perturbed input (the band) gives 0.016–0.10 mm. larmorx is below the band on every run.
- Median parameter differences: 0.001–0.003 mm and 0.0005–0.005°.
- FD correlation: 0.854–0.999, against the band's 0.794–0.998.

**All 116 runs with `-mats`:**

| Category | Runs | Worst 95th pct (mm) | Worst median Δparam (mm / °) | Lowest FD r |
|---|---|---|---|---|
| options (ds000005) | 23 | 0.066 | 0.0103 / 0.0083 | 0.9991 |
| real | 28 | 0.030 | 0.0029 / 0.0053 | 0.9820 |
| synthetic/geometry | 10 | 0.113 | – | – |
| synthetic/motion | 31 | 0.281 | – | – |
| other synthetic categories | – | ≤ 0.096 | – | – |

- The thresholds are 0.1 mm and 0.05 mm / 0.05°, or mcflirt's band where that is wider (25 runs use the band).
- 49 of the 11,271 `.mat` files are identical to mcflirt's as text.
- Against the synthetic ground truth, larmorx and mcflirt are equally accurate: mean deviation 0.409 mm and 0.414 mm.

**Failure (`real/ds003345/default`, mcflirt's default middle-volume reference):**
- The FD correlation is r = 0.9885; the band's threshold is 0.9893. The three noise realisations give 0.9893–0.9903 and the cropped slice 0.9888.
- On that run the matrices are at the edge of the band: 0.028 mm, against 0.025–0.026 mm for noise and 0.029 mm for crop. The 0.1 mm threshold still passes.
- The status is still marked `validated` and the API page and README describe this case. **You may prefer `experimental` until it is resolved.**

### Benchmarks (`docs/benchmarks/mri-hmc.md`)
fMRIPrep's command on a laptop i7-8750H with 12 logical CPUs, against mcflirt on one thread:

| Measure | Speed-up |
|---|---|
| CLI, 1 thread | 1.2–2.3× |
| CLI, 4 threads | 3.6–7.1× |
| CLI, 12 threads | 4.9–9.4× |
| Matrices only (Python, `resample=False`), 12 threads | 5.7–16× |
| `-stages 4` on ds000005, 12 threads | 110 s vs 531 s |

A 240-volume run takes 5.4 s instead of 31 s.

### Spec gaps settled by black-box runs (added to `specs/mcflirt.md`, marked observed)
- Each elementary rotation's sine and cosine is rounded to float32.
- Square roots are float32. Sums are in exactly three levels, and N must be exact: one ulp breaks agreement.
- `-report` prints `refnum`/`Original_refvol` only when one of `-mats`, `-plots`, `-rmsrel` or `-rmsabs` is given. The exact message text for an unknown cost was also recorded.
- `-verbose 20` trace format.
- A single-volume output is 3D and keeps `pixdim[4]`. The qform quaternion is rewritten and normalised.
- `-stats` and mean images keep the input's `cal_min`/`cal_max`.
- `-meanvol` with `-reffile` registers the first pass to that file and forms the mean on its grid.
- Band numbers (§14), and which cost-arithmetic variants were ruled out (§17).

### Clean-room statement
- **Opened:**
  - `specs/mcflirt.md`, PLAN.md and the docs on overview, licensing and architecture
  - the oracle manifest and each run's `run.json`, outputs, stdout and stderr
  - `mcflirt -help`; black-box runs of `~/fsl/bin/mcflirt`, including `-verbose 20` traces
  - the larmorx-testdata catalog and data
  - nitransforms (MIT) through its API only
  - glibc `sinf`/`cosf` values queried through ctypes
  - larmorx repo files outside the forbidden ones
- **Never opened, listed or grepped:** `~/fsl/src` or anything else under `~/fsl` apart from running its binaries; `docs/findings/` (the directory was never listed); `crates-nc/`; `crates-gpl/`; `scripts/run_mcflirt_oracle.py`; `oracles/.../analysis/`; other tasks' `/tmp` files.
- No web searches.

### Commits (no AI attribution)
fbdfe36 (WIP checkpoint), 423c0af, 823c465, f8a8972, fc0405c, a694e83, 2a06738, 3b155ec, fc392f5, 7c3b038, 5e7fb83, c64b950. `main` is unchanged at 81d707f and already merged.

### Left undone or open
- **Bit-exactness:** the first parabola step matches mcflirt for 140 of 240 volumes on ds000005, and for 50–70 % of volumes on other data. Cost values still differ by 1–2 float32 quanta once the test volume is interpolated; the cause is unknown.
- **Single-thread speed** is only 1.2–2.3× mcflirt's. A microbenchmark showed that unchecked float-to-int conversions and loads would gain about 20 % on the cost loop, but that needs audited `unsafe`, which the brief excluded. Several other attempts gave no gain.
- **Histogram costs** (`corratio`, `mutualinfo`, `normmi`) and `leastsquares` are within thresholds on ds000005. Woods' cost registers no BOLD run for either program.
- **Not done:**
  - `-gdt` is unsupported.
  - There is no `implementation="auto"` replica switch, because there is no larmorx-nc replica yet.
  - The band has no second FSL build.
- CLAUDE.md status lines are left to you.
- No new test data was needed.
