# Final report: antsApplyTransforms --float + speed

Agent `a689dd2fa349688de`, 2026-10-11. The agent's report to the orchestrator, verbatim.

`--float 1` is done: larmorx now computes it in single precision exactly as ANTs does. All 81 `--float 1` parity cases and all 14 of fMRIPrep's `--float 1` outputs are bit-identical. The template-prior step is now faster than ANTs on one thread, but the benchmark report is pending a quiet machine. All gates pass, golden checksums are recorded, nothing is pushed. Branch `worktree-agent-a689dd2fa349688de` has main merged in; main has not moved since.

**What the float path covers** (read from ANTs 2.6.5 and ITK 5.4.5; written up in `docs/findings/ants-float.md`):
- **Points and indices:** each output point is computed in double and rounded to float before the first transform. Continuous indices are computed in double and stored as float. On the scan-line path the line vector is a float difference and each index `start + α·vector` is rounded to float. The inside test compares floats.
- **Output:** interpolated values stay double, then are clamped to ±FLT_MAX and rounded to float (ITK's bounds-checked cast).
- **Transforms:** affine, Rigid3D, Euler (correctly rounded `sinf`/`cosf`, newly ported from CORE-MATH and bit-identical to its C code on all 2^32 inputs), versor rigid and similarity (their mix of float and double), translation, and identity.
  - Inverses are computed in float with vnl's float SVD inverse.
  - Displacement fields are looked up in float; composites and `.h5` composites are read into float transforms.
  - 4D (`-e 3`) is covered.
- **Reading in float:** text transforms are read as float (both parameter lines rounded once from the text, as ITK does); `.mat` and `.h5` are cast from their stored values; `-f` is parsed as float.
- **Interpolators:**
  - Nearest neighbour rounds `2x + 0.5` in float.
  - Gaussian and MultiLabel store their support region's ends as float.
  - The windowed sincs store their window's argument and value as float.
  - Linear, B-spline and GenericLabel need only the float index. The B-spline prefilter is double in ITK, not float, so it stays double.
- **Interfaces:** CLI `--float 1`; Python `lx.ants.apply_transforms(..., precision="float32"|"float64")`, which returns float32 data in float mode. `_core.pyi`, the API page with its option table, the findings, the provenance files and `CLAUDE.md` are updated.
- **Precision is a type parameter** (`larmorx_core::ItkReal`), so float and double share one code path. The double path is unchanged, checked bit for bit against the old code on 2,000 random transforms of each kind, inverses included.

**Parity, standard tier** (`docs/validation/ants-apply-transforms.md`): 166 cases, every old case again with `--float 1`, plus two float-parsing cases.

| | ANTs as built | ANTs with CORE-MATH preloaded |
|---|---|---|
| All 158 compared cases | 144 bit-identical | 158 bit-identical |
| The 81 `--float 1` cases | 81 bit-identical | 81 bit-identical |

- The other 8 cases are errors that both programs reject.
- The 14 cases that differ as built are all double-precision Gaussian and windowed-sinc cases; glibc's maths is the only cause.
- The CORE-MATH preload now also replaces `sinf`, `cosf` and `sincosf`. I rebuilt it in place in `oracles/core-math-preload`. No other suite's result can change, because no other port computes with those functions.
- The smoke-tier parity run before recording golden: 1746 passed.

**The 14 fMRIPrep nodes** (`docs/validation/fmriprep-stages-float.md`, stages A06, A11, F08, F09 and F12 re-run):
- Before: 3 of 8 A11 outputs bit-identical, and 3 of 6 each in F08, F09 and F12. Three probability maps failed the 1e-4 threshold.
- After: all 14 `--float 1` outputs are bit-identical to fMRIPrep's own (A11 8 of 8, F08, F09 and F12 6 of 6 each). Overall 26 of 28 nodes; the other two (A06, in double) differ only through glibc's maths. No case fails any more.
- The full `fmriprep-stages.md` is not regenerated; that is queue item 4.

**Template-prior speed (A06):** it was slower because of vnl's `erf`, which recomputed log Γ(1/2) (two logarithms) on every call.
- Fixes: that value is now computed once, and the Gaussian weights live on the stack instead of being allocated per voxel. The output bits are unchanged: still identical to ANTs with CORE-MATH preloaded, and all double golden cases unchanged.
- One thread, larmorx and ANTs run alternately on the busy machine: larmorx 10.5–11.3 s, ANTs 12.1–13.9 s. With threads, rows are resampled in parallel.
- The benchmark report is pending: load was 7–18 throughout. I added a `--float 1` Gaussian job and a Gaussian-through-warp job to the benchmark suite.

**Golden**, recorded from fb6cc8d after smoke parity passed:
- New cases: `cli.ants.antsApplyTransforms.float` (float nonlinear and scan-line paths) and `py.ants.apply_transforms.float32` (every interpolator plus the scan-line path).
- Changed: `cli.ants.antsApplyTransforms.4d`, which uses `--float` and so now computes in single precision.
- The other 88 cases are unchanged; 91 cases in all.

**Shared-type changes** (additive unless noted):
- **larmorx-core:**
  - new `itk_real` module (`ItkReal`, `Precision`);
  - `math::{sinf, cosf}`;
  - `vnl_inverse3_f32` and `vnl_determinant3_f32`.
- **larmorx-transform:**
  - new `MatrixOffset<R>`, `LinearTransform::with_precision`, `TransformChain::with_precision` returning `PrecisionChain`, and `DisplacementField::transform_point_as`;
  - `TransformChain` gained a private field;
  - `LinearTransform::from_itk` now goes through the generic code (same bits);
  - behaviour changes, all following the ITK source:
    - translations map `p + offset` (differs only in zero signs);
    - a translation's inverse stays a translation;
    - the Euler ZYX flag counts only with exactly 4 fixed parameters;
    - the field lookup uses ITK's floor.
- **larmorx-interp:** `Interpolator::with_precision` and `precision()`, `nearest_as`. The sinc base index now uses ITK's floor (tiny negative indices only).
- **larmorx-io:** `parse_itk_text_with` and `read_itk_transform_with`.
- **larmorx-ants (breaking):** a new `precision` field on `ApplyTransformsOptions`, so code building it as a struct literal must add it; all callers in the repo are updated. Also new: `ApplyTransformsError::Transform`, and `TransformLoader::load_with`, a default method.
- **Python:** `precision=` on `apply_transforms`, `_core.itk_transform_read` and `_core.ants_apply_transforms`.
- **To mechanically copy the updated `NOTICE` (Apache) into `crates-gpl/LICENSES/NOTICE-larmorx.txt`, I edited inside `crates-gpl/`, which the brief limits to the lockfile refresh.** Its test requires the copy; I read no GPL code.

**D13:** no defaults changed. One new register entry, ITK-10: a valid versor whose axis has norm ≥ 1 can fail to load in float. ANTs exits 1 with `--float 1` and 0 with `--float 0`; larmorx refuses the same way. ITK-01 (ITK's floor of tiny negative indices) now also covers the field and sinc lookups. The options struct can take an `upstream_defects` field later; I did not add one.

**Not reproduced, documented:** Rigid3DTransform's orthogonality check, and GenericLabel treating −0 and +0 as one label.

**Commits** (oldest first): 4d15201 (pause checkpoint), 1078486, e917ba6, 30ebfbb, merge 90f9685, a2cdf3e, 9110347, fb6cc8d, 60a580d.

`NOTES-float.md` is archived in `<workspace>/archive/2026-10-11-float-notes/` and removed from the repo.
