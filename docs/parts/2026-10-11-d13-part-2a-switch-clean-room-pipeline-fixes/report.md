# Final report: D13 part 2a: switch + clean-room/pipeline fixes

Agent `ac15f1af9c8d17145`, 2026-10-11. The agent's report to the orchestrator, verbatim.

## D13 implementation report (worktree `agent-ac15f1af9c8d17145`, branch `worktree-agent-ac15f1af9c8d17145`; not pushed)

D13 is implemented. Every fix is on by default, and `reproduce` gives the original's result bit for bit. Parity runs in `reproduce` mode and is bit-identical as before. Gates pass on the final merged tree. One gap: the full `maturin develop` + pytest gate ran on the pre-merge tree. After the merge I rebuilt with `maturin develop --release` and ran pytest separately (323 passed).

### Commits
- `4a087a7` D13: correct upstream defects by default, reproduce them on request (mechanism, all fixes, tests, docs).
- `5164ff5` Measurements (PIPE-03, FSL-02 with ground truth, FSL-01) and the regenerated parity records.
- `1977b24` Shorter fix-pass summary; FSL-01's measured effect on the mri-hmc page.
- `2ef5e9c` Golden checksums, recorded with `--skip-parity`. The 33 failing smoke cases were all ANTs N4, during the other agent's oracle rebuild. Superseded by `1b0a6f0`.
- `0955589` Regenerated 3dTshift and mri-hmc records; `golden.md` case count.
- `0311a80` Merge of `main` (`8a5bca3`, the ANTs/N4 work). Conflicts in `decision-log.md` (kept both sections) and `golden.json` (re-recorded).
- `1b0a6f0` Golden checksums re-recorded from the merged tree. The full smoke-tier parity passed: 1670 passed, all ANTs suites included. Against main's `golden.json`, only my entries differ; the N4 cases match main.

Identity is `karellopez <49761113+…@users.noreply.github.com>`. No attribution lines anywhere.

### Mechanism
- **Rust** (`larmorx_core::defects`):
  - `enum UpstreamDefects { Fix (default), Reproduce }`, plus `ENV = "LARMORX_UPSTREAM_DEFECTS"` and parse/env helpers. A bad value exits with code 2.
  - `Defect` constants, gathered in the list `DEFECTS`.
  - `Notice { defect, mode, detail }` with `message()` and `line()`.
  - Each tool's options struct carries the switch, and tools return `Vec<Notice>`.
- **Python**:
  - `upstream_defects="fix"|"reproduce"` on every affected function.
  - `lx.UpstreamDefectWarning(UserWarning)` with attributes `entry`, `tool`, `mode` and `detail`. It fires when a defect changed the result (fix), or would have been corrected (reproduce).
  - `lx.mri.hmc` also takes `normcorr_count="mcflirt"|"weights"`; `weights` cannot be combined with `reproduce`.
- **Command line**: the environment variable `LARMORX_UPSTREAM_DEFECTS`, following the `LARMORX_IMPLEMENTATION` pattern. Each defect prints one stderr line: `upstream defect <ID> (<tool>) corrected|reproduced: … [LARMORX_UPSTREAM_DEFECTS=…]`. Replicas receive the variable through `_replica.run`, and their stderr lines are parsed back into warnings.
- **Parity** compares in `reproduce`. A fix pass then runs the default and reports what changed. It checks that a defect is named exactly when the outputs differ (with a "silent ok" list where a notice can fire without a visible change).
- **Golden**: the cases record the default output, plus seven tiny `*.reproduce` / `composite_order` cases.
- **Docs**: `docs/api/upstream-defects.md`; the D13 sections of the three API pages; the register's "larmorx today", Summary rows and gap list; `porting-guide.md` §8; the decision-log entry.

### Each fix
Parity in `reproduce` mode is bit-identical as before for every suite. Both modes are unit-tested and the warnings are tested. All golden `*.reproduce` checksums equal the pre-D13 checksums of the same computations (15 of 15).

**Resampler (resample-series: 136 of 136 bit-identical)**
- **PIPE-03:**
  - Fix pass: 9 cases changed, largest difference 871 in output units.
  - Realistic case (`docs/validation/resample-series-jacobian.md`): ds000005's real grids and transforms with a synthetic 3 T-like field map, shifts up to 3.6 voxels.
    - Native grid: bit-identical.
    - T1w: fMRIPrep's factor differs from the corrected one by more than 0.01 in 40.5 % of brain voxels, by more than 0.05 in 21 %, by more than 0.2 in 2.2 %. At the 99th percentile it is 0.227 off the exact factor; larmorx's is 0.002 off.
    - MNI res-2: 32.5 %, 13.9 % and 0.2 %; 99th-percentile error 0.163 for fMRIPrep against 0.008 for larmorx.
  - The ds001600 fMRIPrep oracle skipped its BOLD runs as too short (≤ 5 volumes), so no real field-map run exercises PIPE-03 yet.
- **PIPE-04:** latent in fMRIPrep (its composites have 2 or 3 transforms). Covered by a golden case with an 11-affine composite and by unit tests.
- **SCI-03:** 15 cases named it, 10 changed, largest difference 5.

**3dTshift original (241 cases, 222 pass, 0 failures, 162 of 222 bit-identical)**

| Defect | Cases | What the fix changes |
|---|---|---|
| AFNI-01 | 7 | data, up to 8,540 (int16 with negative slope) |
| AFNI-02 | 2 | header scaling only |
| AFNI-03 | 1 | data, up to 27.8 |
| AFNI-05 | 4 | data, up to 122 |
| AFNI-07 | 2 | header origin (`qoffset`/`srow`) |

- AFNI-08: black-box runs did not find its trigger, so it has no switch.
- The unit cases now carry a real origin.

**Head-motion original (mri-hmc: 134 cases, 121 pass; the one failure is the known ds003345 FD threshold, unchanged)**
- **FSL-01:** 110 runs changed.
  - On the 52 real runs, a run's largest matrix change has a median of 0.024 mm.
  - On fMRIPrep's command for ds000005 the last volume moves by 0.06 mm.
  - The largest real change, 72 mm, is a `-cost woods` run where mcflirt and larmorx both fail on every volume.
  - Synthetic `background` series: mcflirt's last volume converges to a 180° rotation, 106 mm from the true motion. The corrected one is 0.36 mm from it.
- **FSL-03:** 5 runs, matrices up to 0.30 mm.
- **FSL-14:** 2 runs; mcflirt's NaN matrices become finite.
- **FSL-05, -06, -08, -09, -10, -13:** 1, 1, 5, 102, 3 and 1 runs respectively (command line, names, integer rounding, units).
- **FSL-07:** covered by CLI unit tests; no parity case triggers it.

### FSL-02 (measured, not switched)
Record: `docs/validation/mri-hmc-normcorr-count.md`.
- The corrected count moves larmorx's matrices by 0.014–0.033 mm at the 95th percentile on 19 real runs. mcflirt's own variability band there is 0.016–0.10 mm.
- Against mcflirt: `weights` stays within the band on 24 of 28 band runs; `reproduce` on 28 of 28 and `default` on 26 of 28 (its two misses are FSL-01's last volume).
- Against the synthetic ground truth it is no more accurate: better on 5 of 8 series, worse on 3, by 0.001–0.16 mm.
- **Recommendation:** keep it opt-in; the user decides.

### Golden entries I changed
- **Changed:** `cli.mri.hmc`, `py.afni.tshift.options`, `py.mri.hmc.options`, `py.ndimage.map_coordinates`, `py.ndimage.spline_filter`, `py.transforms.resample_series.sdc`.
- **New:** `cli.afni.3dTshift.reproduce`, `py.afni.tshift.reproduce`, `cli.mri.hmc.reproduce`, `py.mri.hmc.reproduce`, `py.ndimage.reproduce`, `py.transforms.resample_series.reproduce`, `py.transforms.composite_order`.
- Totals: 89 cases, 355 outputs; the suite runs in 0.9 s.

### Found along the way
- **Bug in the 3dTshift replica, left for its owner:** it writes a metre or micrometre input's sform unconverted, where AFNI converts the linear part to mm; its qform matches AFNI's. I did not touch the replica. The two cases are marked as expected divergences in the replica suite (`REPLICA_DIVERGENCES` in `validation/.../parity/afni_tshift.py`), with the reason in its record.

### Clean-room statement
**Opened:**
- Rules and docs: the CLAUDE.md files, the decision log, the porting guide and `architecture.md`.
- The register: only its Summary, "D13 gaps", "Decisions needed" and non-restricted entries. I edited only the FSL-01/FSL-02 Summary rows and decision item 4.
- `specs/3dTshift.md` and `specs/mcflirt.md`; I added notes marked **(observed)**.
- larmorx's own code, including the validation harness. Of `tests/parity/test_afni_tshift_replica.py` I grepped only its import and CASES lines.
- fMRIPrep, nitransforms, SciPy and nipype sources, for the PIPE and SCI entries.
- Oracle data: mcflirt's `band.json`, `runs/*/run.json` and the `.mat` files; testdata sidecars; fMRIPrep oracle outputs and logs.

**Run as black boxes:**
- AFNI's 3dTshift oracle binary and `~/fsl/bin/mcflirt`, including `-help`.
- The `larmorx-gpl` program, on two unit inputs (headers compared) and through its parity suite.
- `cargo fmt`/`clippy`/`test` in `crates-gpl`, seeing their output only.
- Of the regenerated replica record I read only its 3-line headline.

**Never opened:** FSL source (nothing under `~/fsl` beyond running `bin` programs); AFNI source (`tags/AFNI-25.2.09`, `afni/`, the oracle build directory); `docs/findings/fsl-mcflirt.md` and `afni-tshift.md`; the register's † sections; the sources in `crates-gpl/` and `crates-nc/`; `scripts/run_mcflirt_oracle.py`; `oracles/fsl-6.0.7/mcflirt/analysis/`.

### Files
In `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-ac15f1af9c8d17145/`:
- `crates/larmorx-core/src/defects.rs`
- `python/larmorx/_defects.py`
- `docs/api/upstream-defects.md`
- `docs/validation/resample-series-jacobian.md`
- `docs/validation/mri-hmc-normcorr-count.md`
- `validation/src/larmorx_validation/parity/{defects.py, resample_jacobian.py, mri_hmc_count.py}`
- `tests/golden/golden.json`
