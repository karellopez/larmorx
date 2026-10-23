# Final report: Clean-room implement lx.afni.tshift

Agent `a044c62bbab60d293`, 2026-10-09. The agent's report to the orchestrator, verbatim.

I implemented `lx.afni.tshift` / `larmorx afni 3dTshift` clean-room from `specs/3dTshift.md`. It agrees with the AFNI 25.2.09 binary on all 236 standard-tier parity cases and is 2–3× faster on one thread, 5.6–6.2× on 12. All lint and test gates passed before each code commit; nothing was pushed, and commits carry no AI attribution. The parity record and the benchmark report were both generated from the clean commit `7708720`.

**Clean-room statement:** I never opened, read, grepped or listed any forbidden path: `tags/AFNI-25.2.09/`, `afni/`, `oracles/afni-25.2.09/build/`, `docs/findings/afni-tshift.md`, or other tasks' `/tmp/claude-1000/` files. I did no web searches. My inputs were the spec, `3dTshift -help`, and black-box runs of `oracles/afni-25.2.09/bin/3dTshift`.
- Fast-forwarding my branch to `main` brought a newer `docs/findings/afni-tshift.md` into the worktree. I saw only its filename in the merge summary and never opened it.
- I read the test-data catalog descriptions in `synthetic-tshift.toml`, but not the generator script.
- `CLAUDE.md` changed on disk mid-task to "read, don't translate" for AFNI's code. I stayed strictly clean-room, as you instructed.

**Worktree and branch:** `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a044c62bbab60d293`, branch `worktree-agent-a044c62bbab60d293`. I first fast-forwarded it to `main` (`9eb2f31`) to get the spec and the correctly rounded `sin`/`cos`.

**Commits:**
- `c0d53d8` Add larmorx-afni: clean-room 3dTshift
- `a3a8839` larmorx CLI: the afni family
- `56853b5` Python: lx.afni.tshift
- `385027c` -no_detrend: reproduce AFNI's voxel pairs
- `6aeef0b` Add the 3dTshift parity suite
- `8444c57` parity: per-category summary in the record
- `f67c289` weighted sinc: one fraction per slice
- `fa28125` parity: name the known causes of last-bit differences
- `cc0f8ba` larmorx-afni: name the stored type in the float32 conversion warning
- `7708720` Add the 3dTshift benchmark suite
- `df2821f` specs/3dTshift.md: behaviour found by black-box runs
- `5dbe305` validation record, API page and benchmark report

**Files created:**
- `crates/larmorx-afni/` (FFT, tshift core, AFNI's NIfTI rules, slice timing, 1D parser, CLI, `PROVENANCE.md`)
- `crates/larmorx-py/src/afni.rs`
- `python/larmorx/afni/__init__.py` and `tshift.py`
- `tests/python/test_afni.py` and `tests/parity/test_afni_tshift.py`
- `validation/src/larmorx_validation/parity/afni_tshift.py` and `bench/afni_tshift.py`
- `docs/api/afni-tshift.md`, `docs/validation/afni-tshift.{md,json}`, `docs/benchmarks/afni-tshift.{md,json}`
- I also edited the workspace `Cargo.toml`, `larmorx-cli`, `larmorx-py`, `python/larmorx/__init__.py`, `_core.pyi`, the validation `__main__.py`, both docs READMEs, `docs/architecture.md` and the spec.

The FFT is my own (Stockham, radices 2/3/4/5), not `rustfft`. With `-no_detrend`, larmorx reproduces AFNI's voxel-pair behaviour (described below) and warns, as the spec already does for the negative-slope defect.

**Parity, standard tier: 236 cases; 217 pass, 17 rejected by both, 2 expected divergences, 0 fail.** 158 of the 217 compared cases are bit-identical: 117 of 120 for Lagrange, weighted sinc and copies, and 41 of 97 for Fourier.

| Category | Compared | Bit-identical | Worst float diff (× max AFNI) | Worst integer diff |
|---|---|---|---|---|
| copy | 6 | 6 | 0 | 0 |
| header-timing | 12 | 11 | 8.5e-8 | – |
| methods | 84 | 84 | 0 | 0 |
| options | 42 | 26 | 4.3e-7 | 0 |
| real | 20 | 9 | 5.6e-8 | 1 |
| synthetic | 33 | 20 | 3.7e-7 | 1 |
| tpattern | 20 | 2 | 8.5e-8 | – |

- The thresholds are 1e-5 relative for float outputs and ±1 for integer outputs; the worst cases sit far inside them.
- Real int16 runs differ by ±1 in about 3–7 values per million.
- Headers, TR, `toffset`, units and xform codes are identical, and the affines show no difference.
- Fourier differences are expected: AFNI's FFT is float32, mine is double.
- The two expected divergences are `-voxshift` (not supported) and a prefix without `.nii`/`.nii.gz` (larmorx writes NIfTI only).

**Benchmarks** (median of 3, uncompressed output):

| Run | AFNI | larmorx, 1 thread | larmorx, 12 threads |
|---|---|---|---|
| ds000210 | 2.53 s | 1.27 s (2.0×) | 0.45 s (5.6×) |
| ds006010 | 7.25 s | 2.34 s (3.1×) | 1.18 s (6.2×) |
| ds005454 (96 slices) | 48.3 s | 24.3 s (2.0×) | 8.67 s (5.6×) |

The Python API without file output reaches up to 7.4×. With all threads, about half of larmorx's time is single-threaded gzip decompression of the input.

**Spec gaps settled by black-box runs** (all now in the spec, marked *(observed)*):
- **Header slice timing (no `-tpattern`):**
  - It is used only when the slice axis in `dim_info` is the third, `slice_code` (read as a signed byte) is positive, the duration is positive, and `0 ≤ slice_start < slice_end < nz`.
  - Codes 1, 3, 4, 5 and 6 follow the NIfTI orders; code 2 and codes 7–127 give every slice time 0.
  - Times are built by repeated float32 addition, and ms/µs durations are converted.
  - Times beyond the TR make the output a copy, with the timing written back to the header.
- **Copy cases:** the output keeps the input's TR and `toffset`, even when `-TR` is given. A one-volume input is written as 3D.
- **Output header:** this corrects the spec. `slice_end` is nz−1 and `dim_info` is 48. Inputs in NIfTI-2, big-endian or metre/micron units come out as little-endian NIfTI-1 in millimetres.
- **Geometry:**
  - The sform wins whenever `sform_code` > 0; otherwise the qform, otherwise the voxel sizes.
  - Both output forms come from that one matrix, with codes mapped {1,2}→1, {3,5}→3, 4→4.
- **Scaling to float32:** the stored value is converted to float32 first, then `slope·x + inter` is computed in double.
- **Brick factor on store:** multiply by `1/factor` in double; this confirms the spec.
- **Heptic:** the float32 sum adds the terms of samples −2…+3 first, then −3, then +4. A brute-force search found this order; no other order matches.
- **Weighted sinc:**
  - The fraction is taken once per slice from the slice's offset, not per output sample.
  - Near the edges the sum is in double; it is float32 only where the whole ±R window is inside the series.
  - The remaining differences come only from glibc's float `sinf`/`cosf`, which are not correctly rounded. Calling them through ctypes in a model reproduces AFNI exactly; larmorx keeps its correctly rounded functions.
- **`-no_detrend`:** voxels are paired in index order within each slice. The first of each pair has its mean removed and restored; the second is shifted as raw values. A unit test pins AFNI's exact output for a designed input.
- **Option parsing:**
  - Only the method options (4 characters, case-sensitive), `-wsinc5`/`-wsinc9` (7 characters, any case) and `-verb` (5 characters) may be abbreviated; everything else must match exactly.
  - Values are read with C's `atof`/`atoi`. `-slice` beats `-tzero`, and the last `-rlt`/`-rlt+` wins.
  - A `-Fourier` given after `-no_detrend` is allowed, with a warning.
  - `-TR …ms` or `…msec` switches every time to milliseconds; output is written in seconds.
  - Arguments after the dataset are ignored.
  - AFNI exits 0 without writing when the output exists; larmorx exits 1.
- **`@file` format:** read as an AFNI 1D matrix, column by column. `#` comments, commas and `N@v` repeats work; the first row sets the number of columns; text and `nan` are errors.

**Left undone:**
- **Unresolved last-bit residues, all within thresholds:**
  - quintic and heptic weights for some real fractions (several hypotheses tested, none matched);
  - the exact rounding of `-no_detrend`;
  - weighted sinc's glibc `sinf`/`cosf` differences, left by design.
- **Shared venv:** `maturin develop` installed larmorx into `.venv` as editable, pointing at this worktree. Re-run it from the main checkout after merging. The new parity test needs `validation/src` from this branch on the path; the venv's `larmorx_validation` is the main checkout's.
- **Docs I did not touch:** the `CLAUDE.md` status and repo map, and `validation/README.md`'s suites table (it was already out of date). `docs/findings/afni-tshift.md` is forbidden to me, so the findings live in the spec.
- **First-commit lockfile:** commit `c0d53d8` includes a `Cargo.lock` that already lists the CLI's `larmorx-afni` dependency added in the next commit.
- **Logs:** the full parity and bench logs are in `oracles/logs/afni-tshift-*.log`.
