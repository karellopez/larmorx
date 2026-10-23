# Brief: Write mcflirt behaviour spec

As given to agent `a15a4b15f33b261f2` on 2026-10-09, verbatim (paths are this machine's).

You are writing the **behaviour spec** for larmorx's head-motion correction, `lx.mri.hmc`: what FSL's `mcflirt` computes, as fMRIPrep uses it. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read `CLAUDE.md`, `docs/overview.md` and `docs/licensing.md` first; they are the rules.

## Why you, and what the spec is for
- FSL is under the FSL Licence (non-commercial). larmorx therefore gets two implementations of mcflirt:
  - a **clean-room original** (Apache-2.0, main package, crate `larmorx-mri`, neutral name `lx.mri.hmc`);
  - later, a bit-exact **replica** in `crates-nc/`.
- The clean-room implementer will work **only** from your spec, the published papers, FSL's documentation and black-box runs of the binary. They will never see FSL's source or notes derived from it. Your spec is their single description of the behaviour.
- You **may** read FSL's source: it is installed at `~/fsl/src/` (FSL 6.0.7.x). The relevant packages are `fsl-mcflirt` (`mcflirt.cc`, `Globaloptions.*`), `fsl-flirt`, `fsl-newimage` and `fsl-miscmaths`, plus whatever mcflirt calls. Record the exact package versions from their conda metadata in the spec, and add rows for them to `upstream.tsv` / `UPSTREAM.md` in the existing style.
- You may run FSL binaries (`~/fsl/bin/mcflirt`, `flirt`, `avscale`, ...) as an oracle (decision D7). Keep `~/fsl/bin` out of PATH for anything else, because FSL ships its own compilers.
- **The model to follow** is `specs/3dTshift.md`. It describes behaviour (inputs, outputs, the maths, edge cases, precision, operation order where it matters) in prose, equations and tables, and marks statements confirmed by black-box runs as **(observed)**.
  - **No code may be transcribed:** no C++ snippets, no line-by-line pseudo-code mirroring the source's structure, no copied identifiers beyond public option names.
  - Describing an algorithm's steps, constants, schedules and numeric conventions is the point. The spec must be complete enough to reproduce mcflirt's results closely, ideally to within float rounding.

## How fMRIPrep uses mcflirt
`docs/analysis/01-fmriprep-bold-pipeline.md` row 2 and `fmriprep/fmriprep/workflows/bold/hmc.py` (in the workspace):
- `mcflirt` on the raw BOLD with `-reffile` = the HMC reference;
- `save_mats`, plus whatever nipype's `fsl.MCFLIRT` interface adds by default (check `nipype/nipype/interfaces/fsl/preprocess.py`);
- the `.mat` files are converted to ITK by niworkflows' `MCFLIRT2ITK` (`niworkflows/niworkflows/interfaces/itk.py`), and the motion parameters feed the confounds (FD and others; check `fmriprep/fmriprep/interfaces/confounds.py` or the equivalent).

The spec must cover the exact command fMRIPrep runs, then mcflirt's other options, marked by importance (tier 1 = what fMRIPrep uses).

## What the spec must contain (at least)
1. **Inputs and outputs:** file types, datatypes, how the reference is chosen (`-refvol`, `-reffile`, `-meanvol`, middle volume), and output naming. The `.mat` convention: FSL scaled-voxel coordinates, the role of the voxel-to-mm and left-right flip (determinant) conventions, how it relates to world/ITK transforms. The `.par` file (order, units, rotation convention, Euler angle decomposition), `-rmsrel`/`-rmsabs` outputs and their exact definitions.
2. **The algorithm, stage by stage:** the default number of stages, the schedule of resolutions/smoothing/subsampling per stage, the cost function (normcorr default; others), the optimiser (method, tolerances, parameter scaling, iteration limits), the initialisation (forward and backward passes from the reference volume, use of the previous volume's estimate), the interpolation used during estimation and for the final resampling (`-spline_final`, `-sinc_final`), the treatment of edges/out-of-FOV samples, masks/weights, centre of rotation, and any re-estimation or mean-volume pass.
3. **Numerics:** float vs double where it affects results, operation order where it matters, and random numbers if any.
4. **Edge cases:** 1-volume inputs, a reference with a different geometry, oblique images, negative-determinant (radiological) storage, NaNs, integer data types, very small images.
5. **Black-box confirmation.** Run mcflirt on real BOLD runs from the test-data catalog and on synthetic series with known motion:
   - the catalog is the local repo `/home/karelo/VScode_projects/super_fmriprep/larmorx-testdata`, installed editable in `/home/karelo/VScode_projects/super_fmriprep/.venv`, with a hash-verified downloader;
   - generate synthetic data with a new generator in its `generators/` (seeded, catalog TOML, `docs/sources/` page, as `tshift_synthetic.py` does), and commit in that repo (local only, never pushed; same git rules).
   Store oracle outputs under `/home/karelo/VScode_projects/super_fmriprep/oracles/fsl-6.0.7/` (outside git) with the exact commands, versions and thread settings next to them. Mark what you confirmed **(observed)**. Note mcflirt's run-to-run determinism.
6. **Validation plan for the original:** metrics and thresholds (parameter differences in mm/degrees, FD, resampled-image differences) and the variability band to measure (repeated runs, different FSL builds if available). PLAN.md §11.3 has the project's thresholds.
7. **A short "paper vs implementation" section:** where FSL's implementation departs from or adds to Jenkinson et al. 2002 (NeuroImage 17:825-841).

## Also write
- `docs/findings/fsl-mcflirt.md`: the source-derived findings (file:line references, how each is known). This file is **forbidden to the clean-room implementer**; say so at its top, as the spec does for 3dTshift.
- At the top of the spec, the list of what the clean-room implementer must not open (FSL source, `crates-nc/`, `docs/findings/fsl-mcflirt.md`), as in `specs/3dTshift.md`.

## Rules
- Do not write any Rust or Python implementation of mcflirt. You only write the spec, the findings, the generator and the oracle runs.
- Upstream sources and clones are read-only. Never open anything under `reference_src/`. Do not touch `crates-gpl/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.**
- **CPU:** an ANTs build and two other agents share this 12-core machine. Run mcflirt jobs with modest parallelism and never kill other processes.
- Python scripts you add to the larmorx repo need `# SPDX-License-Identifier: Apache-2.0` as their first line (`python scripts/check_spdx.py`).
- Write in plain, direct English, in the style of `specs/3dTshift.md`.

## Report back
- The spec's outline and its open questions.
- What you confirmed by black-box runs.
- The oracle data you produced (where, which inputs).
- Anything a clean-room implementer will find hard to match.
- Commits in both repos.
