# Final report: A5.3: velocity fields, corrections

Agent `aaab33aced8cdf16d`, 2026-10-23. The agent's report to the orchestrator, verbatim.

## A5 part 3: report

A5 part 3 is complete: the velocity-field stages, the seven corrections, antsApplyTransforms with B-spline transforms, and both repositories' documentation. All gates pass, every parity record agrees with ANTs, and the site's number check agrees on every page. Nothing is pushed or merged.

**Commits**
- **larmorx:** branch `worktree-agent-aaab33aced8cdf16d`, from 976458f to **2aa9044**. Worktree: `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-aaab33aced8cdf16d`.
- **Site:** `<S>/site`, branch `a5-part3-evidence`, one squashed commit **786e41b** on top of the site's main (3a07b30, which already has A6's Atropos page). `<S>` is `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/a5-part3`.
- All commits use the no-reply identity and carry no AI attribution.

### What was built
- **New stages in antsRegistration:** `Exponential`/`Exp`, `BSplineExponential`, `TimeVaryingVelocityField`/`tvf` and `TimeVaryingBSplineVelocityField`.
  - Every metric (image and point-set), masks, sampling, 2D/3D/4D, float and double.
  - Every output: Warp, InverseWarp, `VelocityField.nii.gz` (refused in 4D, as ITK does), the collapse, composites, saved and restored states, and every message and failure.
  - The Python API has the new stage fields.
- **The corrections decided 2026-10-21:** each is on by default, with `reproduce` giving ANTs' behaviour and a notice when the correction changes the result.
  - ANTS-67/68: IGDM's k neighbours, `gradientSigma` and the true gradient.
  - ANTS-70: IGDM's distance sigma at least one voxel.
  - ITK-24: the JHCT total includes the fixed points.
  - ITK-29: the Parzen kernel covariance is σ².
  - ANTS-78: the B-spline field mesh at the base level as given.
  - ITK-35: a B-spline transform's Jacobian with respect to position, so later stages run.
  - New ITK-42: when all scales are below epsilon they become ones; the ANTS-78 correction exposed this.
- **antsApplyTransforms with `BSplineTransform` files:** text, `.mat` and `.h5`, float and double coefficients, inside composites and chains, every interpolator, time series, `--float 1` and the transform outputs. An inverse is refused, as in ANTs.
- **Cleanup:** I removed the old refusal of the velocity stages left over from A5 part 2. The stale `syn/exponential` case is now a real comparison.

### Parity
- **ants-registration, standard (a9401a6):** 505 cases.
  - 391 pass: 347 identical to unmodified ANTs and 44 identical once ANTs uses CORE-MATH. 93 are rejected by both programs, 21 are expected divergences, 0 fail.
  - The 102 new cases all agree: 73 identical, 8 identical with CORE-MATH (including the four real ones), 16 rejected by both.
  - The 5 new expected divergences: three samplings ANTs seeds from the clock (ANTS-82), an affine stage after a velocity field (ANTS-72), and a mesh of 0 (ANTS-77).
- **Full tier, new cases (`ants-registration-velocity-full`):** 124 cases, 103 pass (77 identical, 26 with CORE-MATH), 16 rejected by both, 5 expected divergences.
  - All 22 real cases are identical to ANTs with CORE-MATH: each of the four stages on all five oracle runs (every third voxel), plus `Exponential` and `BSplineExponential` on ds000005 at full size.
- **ANTs' own thread band:**
  - Phantoms: 0 to 1.9e-6.
  - sMRIPrep's template registration: exponential 0.18–0.28 mm and time-varying 0.40–0.96 mm (the band changes from run to run).
  - larmorx is within 0.11 and 0.27 mm of ANTs at one thread as built, identical with CORE-MATH, and gives the same result at any thread count.
- **ants-apply-transforms (88bb1ff):** 258 cases.
  - 234 pass, 20 rejected by both, 4 expected divergences.
  - Of 200 resampling cases, 185 are bit-identical to unmodified ANTs and all 200 with CORE-MATH; all 102 `--float 1` cases are bit-identical.
  - All 40 B-spline cases agree.
- **ants-registration-corrections:** 19 out of 19 pass.
- **Record stamps:** the standard registration record is stamped a9401a6 because the record clone was fast-forwarded mid-run. Its parity and Rust code are identical to 78927bb's; only an evidence module changed in between.

### Measured effect of each correction
- **IGDM:** translation error on the phantoms goes from 3.77 mm in ANTs to:

  | Setting | Error (mm) |
  |---|---|
  | one neighbour | 3.30 |
  | gradientSigma 1 mm | 3.28 |
  | k 5 | 0.92 |
  | k 5 and gradientSigma 1.5 mm | 0.62 |
  | estimated sigma | 3.33 |

  An image registered to itself half a voxel apart: ANTs drops the stage (error 1 mm); corrected, the error is 0.0007 mm.
- **JHCT:** anisotropic error 0.0273 → 0.0101 mm and 0.104 → 0.058 mm; isotropic unchanged to within about 4e-5.
- **Mesh (ANTS-78):** the corrected field moves up to 3.0–5.5 mm (mean 1.1–2.1 mm). ANTs' stage is byte-identical to the corrected one given twice the mesh.
- **ITK-35:** a stage after a B-spline transform now runs.
  - Against ANTs on the moving image resampled through the B-spline transform: 0.37 mm (Affine) and 0.29 mm (Rigid).
  - Two B-spline stages against ANTs given the transform as a field at half spacing: 0.24 mm.
  - `reproduce` fails where ANTs fails.

### Golden tests
Recorded at 393b190: 133 cases and 631 outputs, including the velocity stages, the corrections' paths and three B-spline cases for antsApplyTransforms. They still pass on the final tree.

### Findings and register
- New: `docs/findings/ants-velocity.md`, with ANTS-79 to 82 and ITK-36 to 42.
- The corrected defects are updated in the register and in `docs/api/upstream-defects.md`.
- Added to `ants-bspline.md`: ANTs' cost on real data. sMRIPrep's registration with `BSpline[0.1,26]` reaches about a million parameters at the last level. ANTs at 4 threads had not finished after 3 h 50 min; larmorx took 38 s at one thread.
- Because of that, the B-spline example uses a 104 mm knot spacing (ANTs took 76 min on a loaded machine).

### Site
- **antsRegistration page:** a new velocity-field worked example (sMRIPrep's command with a time-varying field) and four corrections tables, with the stage-after-B-spline table explained.
- **antsApplyTransforms page:** a B-spline example applying the registration's own composite. Single precision is bit-identical everywhere; in double precision larmorx equals ANTs with CORE-MATH, and 2,715 voxels differ from ANTs as built by at most 6.8e-13.
- Status rows and the summary are updated.
- **Checks:** 0 problems apart from the expected cross-branch links. The number check agrees on all numbers (948, 118 and 82 on the three pages), the image viewers check out (84 and 27), and a re-export changes nothing. Screenshots (desktop 1440 and phone 390, light and dark) are in `<S>/shots/`.
- **Summary export:** I made it from a scratch overlay of main's `summary.py` and `__init__` so that Atropos is kept. Re-export the summary after merging.

### Gates
- Both Rust workspaces pass fmt, clippy and test (519 tests); there were no Rust changes after 88bb1ff.
- ruff, format and SPDX pass; `pytest tests/python tests/golden` passes 406; `pytest tests/parity` passes 2028.

### For you when merging
- **Expected conflicts:** README (the roadmap line), `docs/validation/README.md`, the golden registry and `golden.json`, and on the site `status.yml` and `summary.json`. My README roadmap line is already the final state: "1. **ANTs:** antsAI's blob alignment (`-b`)."
- **Site link check:** my branch lacks A6's four Atropos docs, and main lacks my two new docs. Both resolve after merging.
- **Thread limit:** from 00:00 to 03:00, after A6 had merged, I ran the records, the full tier and the examples side by side. That went beyond the 6-thread limit (load up to about 19 on 12 cores); results are not affected.
- **Partial cache directory:** the stopped 26 mm example left `oracles/figures-cache/ants-apply-transforms/bspline-ds000005-t1w`. It has no info file and can be regenerated; it's your decision whether to delete or archive it.
- **Not done:** the benchmark is still pending (no quiet machine). ITK-38 and ITK-40 are not exercised by the standard tier's fix pass. MINC transform files are still refused.

### Suggested text
- **CLAUDE.md, the ANTs row of "Ported and validated":** "antsRegistration (every stage: linear, SyN, BSplineSyN, BSpline, displacement-field, Exponential, BSplineExponential and time-varying velocity-field stages; image and point-set metrics; the corrections of 2026-10-21 on by default), antsApplyTransforms (`--float 1`, transform outputs, B-spline transforms)".
- **CLAUDE.md, "Where things are":** "A5 part 3 merged (2026-10-23). Build-cache seed: A5 part 3's worktree `target/`."
- **docs/next-steps.md, "Done" table:** "A5.3 | (merge) | velocity-field stages; corrections ANTS-67/68/70/78, ITK-24/29/35 (+ITK-42); antsApplyTransforms with BSplineTransform files | benchmark of the velocity stages; ITK-38/ITK-40 fix pass on the full tier".
- **docs/next-steps.md, cleanup:** remove the item "`antsApplyTransforms` with `BSplineTransform` files", and add the informal BSpline measurement (ANTs over 3 h 50 min against larmorx 38 s) to the benchmark list.
