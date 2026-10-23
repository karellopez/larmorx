# Final report: A5.2: BSplineSyN and displacement fields

Agent `a8e1b12f07e8dd9e9`, 2026-10-21. The agent's report to the orchestrator, verbatim.

## A5 part 2 is finished: BSplineSyN, GaussianDisplacementField, BSplineDisplacementField and BSpline stages, validated against ANTs with 0 failures

**Where the work is.** The harness put this agent in a worktree of `larmorx_documentation`, so larmorx was cloned to `/home/karelo/VScode_projects/super_fmriprep/larmorx_documentation/.claude/worktrees/agent-a8e1b12f07e8dd9e9/larmorx`, branch `a5-part2-bspline`, built on main `05a8d04`. Head is `84f913e` and the tree is clean. To merge, from the main checkout: `git fetch <that path> a5-part2-bspline:a5-part2-bspline`. There are 18 commits, all with the configured noreply identity and no AI attribution. Nothing was pushed or merged, and CLAUDE.md is untouched.

### What was built
- **BSplineSyN.** SyN with B-spline smoothing of the update and total fields. It includes ITK's B-spline fit of fields and scattered points (`larmorx_registration::bspline_field`), masks used as weights, the point-set-only path that fits each point's derivative, knot spacings turned into meshes, and the stage's messages and observer. Its output transform is a `BSplineSmoothingOnUpdateDisplacementFieldTransform`.
- **GaussianDisplacementField and BSplineDisplacementField.** Each optimises one field with the linear stages' optimizer, smoothing every update and the total field. This needed:
  - metrics with local support evaluated through the composite transform;
  - the scale and step-size estimation ITK uses for fields;
  - ITK's in-place smoothing of the gradient (and the one place where it does not apply);
  - the per-level field adaptors, plus Demons and GC with fields.
- **BSpline.** The cubic `BSplineTransform` (`transform/bspline.rs`):
  - its domain setters, and the per-level adaptor (resampling the coefficients, then re-decomposing them);
  - ITK's special scale rule for B-spline transforms;
  - ITK's exception when a later stage needs the transform's Jacobian with respect to position;
  - `BSpline.txt`, `.h5` and composite output, and reading B-spline files back (`-r`, `-q`, `-k`).
- **Speed-up that keeps ANTs' bits.** For B-spline transforms the metrics compute only the derivative columns that are non-zero at each point. ITK adds a zero to every other parameter's compensated sum at every point, which can change the sum's state, so larmorx applies those zeros later (`CompensatedSums`). The sums are bit-identical to ITK's; unit tests and every parity case confirm it.
- **Python API.** `RegistrationStage(transform="BSplineSyN" | "GaussianDisplacementField" | "BSplineDisplacementField" | "BSpline", mesh_size=..., total_mesh_size=..., spline_order=...)`, with tests.
- **Bug fixes along the way:**
  - Wrong exception line numbers for `ObjectToObjectOptimizerBase` (118, 130 and 158), also in `multistart.rs` from A4.
  - The ITK-30 notice was missing in the field stages.

### Coverage, and what is not covered
- Covered: every image metric, the point-set metrics, masks, sampling, several metrics per stage, 2D, 3D and 4D, float, `-g`, `-i`, `-a`, `-z`, `--save-state` and `--restore-state`, `--print-similarity-measure-interval` and `--write-interval-volumes`, initial B-spline transforms, and every message and failure.
- Still refused with a message: the velocity-field and exponential stages (A5 part 3).
- Not covered: `antsApplyTransforms` (and so `lx.ants.apply_transforms`) cannot read `BSplineTransform` files yet. It refuses with a clear message. Users can get the warped image from the registration itself (`warped=True`).

### Parity numbers (ANTs 2.6.5, GCC 14.3 oracle, one thread, seed 42)
- **Standard tier,** from `9a4d65a`: 403 cases. 273 identical to the unmodified ANTs, 36 identical with the CORE-MATH preload, 77 rejected by both, 17 expected divergences, 0 failures.
  - The 106 new cases split 80 identical, 4 with CORE-MATH, 20 rejected by both, 2 expected divergences (ANTS-77).
- **Full tier,** from `fc87e0e`: 441 cases. 278 identical, 68 with CORE-MATH, 77 rejected by both, 18 expected divergences, 0 failures.
  - It adds 4D cases of all four stages.
- **Real data.** sMRIPrep's MNI template registration with its SyN stage replaced by antsRegistrationSyN.sh's `BSplineSyN[0.1,26,0,3]`:
  - on every third voxel for all five oracle runs (ds000005 in the standard tier);
  - at full size for ds000005 (7.3 hours in the harness).
  - All are identical to ANTs with CORE-MATH; without it, glibc's `log` in the Mattes stages makes them differ.
- **ANTs' own thread band** (2, 4 and 8 threads against 1):
  - 8.9e-16 for a BSpline transform;
  - 6.7e-7 to 2.3e-6 mm for BSplineSyN and the B-spline field (ITK's B-spline fit depends on the thread count);
  - 1.18 to 1.29 mm on the real BSplineSyN case on every third voxel.
  - larmorx gives the one-thread result at any thread count.
- **Speed, informal** (the benchmark report is pending; the machine was not quiet): on a 4D BSpline case ANTs took 461 s and larmorx 23 s at one thread, bit-identical.

### Golden tests and gates
- Golden: 5 new cases (`cli…bsplinesyn`, `.fields`, `.bspline`, `.bspline.reproduce`, `py.ants.registration.bspline`), recorded from `f5aec9a` after the parity smoke tier passed. Now 130 cases and 598 outputs; the whole suite runs in about 6 s.
- All gates pass at head: fmt, clippy and tests in both Rust workspaces, ruff, SPDX, `pytest tests/python tests/golden` (396 passed), `pytest tests/parity` (1989 passed).

### Findings and register entries
- New findings file: `docs/findings/ants-bspline.md`.
- **ANTS-75**, extended: the observer's transform copies also fail for a B-spline transform whose mesh is not 1. Corrected by default; `reproduce` gives ANTs' failure.
- **ANTS-76:** `-z 1` collapse loses transforms around B-spline transforms, or crashes. Corrected by default; `reproduce` gives ANTs' result.
- **ANTS-77:** a mesh size of 0, or fewer mesh sizes than dimensions, makes ANTs divide by zero and fail on garbage values. larmorx refuses; these are the 2 expected divergences.
- Also found: a coarse B-spline mesh makes ANTs fail with scales below epsilon. This is ITK's design (the pinned boundary dominates the fit), not a defect; larmorx fails the same way.

### Decisions for you
- **ANTS-78** (questionable): BSplineDisplacementField's "base level" mesh is already doubled at the base level. larmorx reproduces it.
- **ITK-35** (questionable): B-spline transforms have no Jacobian with respect to position in ITK, so later linear, field and BSpline stages fail. larmorx fails the same way. Implementing the Jacobian would make those stages run.

### Follow-ups
- `antsApplyTransforms` with `BSplineTransform` files, with parity cases.
- Benchmark report for A5 parts 1 and 2.
- Figures, once the figure framework exists.

### Docs updated
API page with the option table and a section on the four stages; both validation records; validation index; `golden.md`; README; `PROVENANCE.md` for registration, ants and interp; porting-guide note on skipping zeros only where it is exact; `upstream-defects.md`.

### For `docs/next-steps.md` (you asked me not to edit it)
My earlier commit `04880b8` did change §4 Part 2 and Smaller items, before your message; reconcile as you see fit. Suggested text:

"**A5 part 2 done (2026-10-21, branch `a5-part2-bspline`).** BSplineSyN, GaussianDisplacementField, BSplineDisplacementField and BSpline to full completeness, plus the Python API. ants-registration standard 403 cases (273 / 36 / 77 / 17 / 0, from `9a4d65a`), full 441 (278 / 68 / 77 / 18 / 0, from `fc87e0e`), including sMRIPrep's template registration with antsRegistrationSyN.sh's `BSplineSyN[0.1,26,0,3]`. Register: ANTS-75 extended, ANTS-76, ANTS-77; to decide: ANTS-78 and ITK-35. Follow-ups: antsApplyTransforms with BSplineTransform files; benchmark report for A5 parts 1 and 2. Velocity-field and exponential stages: A5 part 3."

Scratch notes are in `<scratch>/a5-bspline/NOTES.md`.
