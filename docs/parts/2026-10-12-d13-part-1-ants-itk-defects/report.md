# Final report: D13 part 1: ANTs/ITK defects

Agent `a448db6a134de1e65`, 2026-10-12. The agent's report to the orchestrator, verbatim.

## D13 part 1: done. Branch `worktree-agent-a448db6a134de1e65`, not pushed

The defaults are corrected. With the defects reproduced, parity against `oracles/ants-2.6.5` (GCC 14.3) gives the same bit-identity counts as before. Every gate passed (both Rust workspaces, `maturin develop`, `pytest tests/python tests/golden`, ruff, SPDX). The smoke-tier parity passed: 1746 cases.

Commits (all with the configured identity, no AI attribution):
- `0620c05` WIP: the code
- `6d0cfb0` merge of main
- `50509e3` WIP: the parity harness and the docs
- `f82dd82` the records and the docs
- `6b8e211` the golden checksums

Main had moved by one CLAUDE.md commit; I merged it.

### Entries
**Fixed by default, with a warning and the reproduce switch:**
- ANTS-03, 04, 05, 08 to 16, 19, 20, 24, 25, 27, 30, 31, 33, 35, 36, 38, 39, 43.
- ITK-01, 02, 03, 10.
- ANTS-06 and SCI-02 are clamped (NaN becomes 0) with a warning; the switch gives x86-64's wraparound.
- ANTS-47 is new: DenoiseImage treats a mask it cannot read as no mask.

**Failure and garbage cases now have a switch:**
- ANTS-29 (size 1) and ITK-08 are reproduced byte for byte.
- ANTS-07, ANTS-19 and ANTS-28 depend on memory ANTs never wrote. The switch reproduces the exit code, header and printed text, fills the undefined part with zeros and warns. For ANTS-28, ITK's printed exception has two addresses and a build directory, written as `0x0` and `<ITK source>`.

**Left alone:**
- ANTS-02 and ANTS-26 (crashes and a hang) and ResampleImage's wrong-length spacing list define nothing beyond the failure, so they stay errors (exit 1) in both modes.
- ANTS-04 is wired, but no operation ported so far reports a failure, so it never fires yet.
- The questionable (c?) entries are unchanged.
- New entry ITK-11: ITK's Gaussian kernel loses accuracy above a variance of about 50 voxels² (1.5e-3 relative at 100, 52 % at 700). It is an imprecision, not D13, so it is reproduced.

**Warnings fire only where the result changes:**
- ITK-01 and ANTS-39 change the last bits of a value at most, so larmorx computes both ways and warns only where a stored value differs (`IndexFloor::checked`).
- ITK-08 warns only when the two files' voxels differ.
- ANTS-03's read warning comes only when the short-named file exists.

### Parity, standard tier, reproduce mode (records from the clean commit `50509e30b3e8`)
| Suite | Bit-identical | Change from before |
|---|---|---|
| ImageMath | 296 of 301 | 303 pass (was 302 plus 1 expected divergence) |
| ThresholdImage | 35 of 35 | none |
| MultiplyImages | 12 of 12 | none |
| SmoothImage | 30 of 30 | none |
| ResampleImageBySpacing | 28 of 28 | 30 pass (was 29) |
| ResampleImage | 47 of 47 | was 46 of 46; size-1 now compared, headers included |
| N4 | 29 of 45; 45 of 45 with CORE-MATH | none |
| DenoiseImage | 7 of 40; 40 of 40 with CORE-MATH | none |
| antsApplyTransforms | 144 of 158; 158 of 158 with CORE-MATH | none |

Three former expected divergences now pass:
- ResampleImage `size-1`, byte for byte.
- ResampleImageBySpacing `thin-smoothed-axis`, compared in everything but the uninitialised values.
- ImageMath `fill-holes/hole-first-slice-0.5`, likewise.

### Fix pass (what the new defaults change)
- **ImageMath:** 12 entries in 38 cases. For example, `/` on a real T1w; ANTS-16 in 11 cases, up to 2.1e4; ANTS-06 contour labels near 2^64 now 0.
- **ResampleImage:** ANTS-30 in 7 cases, ANTS-06 in 5 (up to 4.29e9).
- **N4:** ANTS-38 in 4 cases on a generated phantom and a ramp input, up to 3.9e3. No fMRIPrep command line is affected. ANTS-35 in 1 case.
- **DenoiseImage:** ANTS-43 in 25 of 40 cases. I measured it on sMRIPrep's command with two real T1w images. It changes 7.4 % and 8.4 % of the voxels: median 0.58 and 2.9, at most 2.8 and 17.7 (0.7 % of the 99th percentile), always lower. ANTs' own result moves by up to 271 between thread counts, so this is well inside that.
- **antsApplyTransforms:** only the `-u` integer outputs (ANTS-06).
- **Other suites:** errors where ANTs accepted bad input (ANTS-24, 27, 28, 31, 15). ITK-02 changes header directions when a 4D file is read as 3D.

### ResampleImage's command line (ANTS-30), for your review
`ResampleImage d in out MxNxO [size=1,spacing=0] [interpolate type [its parameters]] [pixeltype]`

| Interpolation | Its parameters | Pixel type |
|---|---|---|
| linear, nearest neighbour | none | argument 7, as in ANTs |
| windowed sinc | window letter in argument 7 | argument 8 |
| B-spline | order in argument 7 | argument 8 |
| Gaussian | sigma in argument 7, alpha in 8 | argument 9 |

- The window `b` now means Blackman, as the usage says; ANTs builds a second Lanczos there.
- A warning names ANTS-30 whenever the two readings differ; reproduce mode reads the arguments as ANTs does.
- Documented in `docs/api/ants-resample-image.md`, "The seventh argument".

### Golden (97 cases, 395 outputs, recorded from `f82dd82`)
- **Defaults re-recorded:**
  - `cli.ants.ImageMath./`, `.total`, `.mean`
  - `cli.ants.N4BiasFieldCorrection` and `cli.ants.DenoiseImage` (data only)
  - `py.ants.arithmetic`, `py.ants.n4`, `py.ants.denoise_image`
  - `py.ants.image_math_operations` (the usage text changed)
- **Renamed:** `cli.ants.ImageMath.vtotal` is now `.vtotal.reproduce`, since larmorx refuses `vtotal` by default.
- **New reproduce cases**, each equal to the checksums those cases had before:
  - `cli.ants.ImageMath.reproduce`
  - `cli.ants.N4BiasFieldCorrection.reproduce`
  - `cli.ants.DenoiseImage.reproduce`
  - `py.ants.arithmetic.reproduce`, `py.ants.n4.reproduce`, `py.ants.denoise_image.reproduce`

### Updated documents
- The register's entries, summary table, counts and gap section; the gap column has no "yes" left.
- The D13 sections of the nine ANTs API pages and `docs/api/upstream-defects.md`.
- The resample-series page (SCI-02 row), `docs/validation/golden.md`, the two PROVENANCE.md files.
- The decision log, porting guide §8 and CLAUDE.md.

### One problem along the way
My first standard-tier run wrote its records straight into the repository, so all but the first were stamped "-dirty". I reran every suite into the scratchpad and copied the results in. The summaries are identical, and all records carry the clean `50509e30b3e8`. The first run's records are kept in `scratchpad/d13p1/records-first-run/`.

Key files are in `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a448db6a134de1e65`:
- `crates/larmorx-core/src/{defects.rs,itk_real.rs}`
- `crates/larmorx-ants/src/cli/resample_image.rs`
- `validation/src/larmorx_validation/parity/ants_programs.py`
- `tests/python/test_defects_ants.py`
- `docs/findings/upstream-defects.md`
