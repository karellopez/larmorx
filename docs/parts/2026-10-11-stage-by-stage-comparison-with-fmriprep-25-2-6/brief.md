# Brief: Stage-by-stage comparison with fMRIPrep 25.2.6

As given to agent `a443c5a76b071f29f` on 2026-10-11, verbatim (paths are this machine's).

You are building larmorx's first comparison with a real fMRIPrep run: **stage by stage**. For each processing step that larmorx already implements, take fMRIPrep's own inputs and parameters for that step, run larmorx on them, and compare with fMRIPrep's own output for that step. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first: `CLAUDE.md` (all rules, including rule 8, no equivalence claims without numbers, and rule 10, never in CI), `docs/overview.md`, PLAN.md §9 and §11, `docs/analysis/01-fmriprep-bold-pipeline.md` and `02-anatomical-smriprep-niworkflows.md` (they give the steps, tools and parameters with file:line), and the decision-log entry "fMRIPrep oracles through Apptainer, not Docker".

## The oracle run (read-only; never modify it)
`oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall/`:
- `RUN_INFO.txt`: image digest, version, exact arguments, seeds, threads (8/8);
- `out/`: the derivatives;
- `work/`: nipype's working directory with every node's inputs and outputs, and the `command.txt` of command-line nodes;
- `fmriprep.log`.
The input dataset is `~/.cache/larmorx-testdata/data/openneuro/ds000005` (sub-01: one T1w, three BOLD runs, no field maps, no slice timing). fMRIPrep's container is `oracles/images/fmriprep-25.2.6.sif` (Apptainer, runs as the user, no sudo). You may run its tools as a black-box reference where useful, e.g. ANTs at one thread to separate thread effects. The repo's own oracle builds are in `oracles/ants-2.6.5/bin`, `oracles/afni-25.2.09/bin` and `~/fsl/bin`.

## Stages to compare
Compare every stage where larmorx has the tool; locate each in `work/` by node name and `command.txt`:
1. **Anatomical:**
   - DenoiseImage (fMRIPrep ran it with 8 threads, where ANTs is not reproducible run to run; see `docs/findings/ants-denoise.md`);
   - every N4 call (pre-template, brain-extraction passes, final);
   - the brain-extraction ImageMath, ThresholdImage and MultiplyImages steps (TruncateImageIntensity, Laplacian, MD/ME/GetLargestComponent/FillHoles/PadImage, ...);
   - antsApplyTransforms of the template priors and masks into T1w space and of the outputs into MNI (Gaussian, MultiLabel, Lanczos, inverse flags, `.h5` composites).
2. **Functional:**
   - head-motion correction: `lx.mri.hmc` on the raw BOLD with fMRIPrep's HMC reference, against mcflirt's matrices and fMRIPrep's ITK HMC transforms and motion confounds;
   - N4 of the coregistration reference;
   - the one-shot resampling: `lx.transforms.resample_series` with fMRIPrep's own transform files, against `desc-preproc_bold` in boldref, T1w and MNI res-2 space, plus boldrefs and masks where fMRIPrep resamples them the same way.
   No slice timing was run for this dataset; note it.
3. **List the stages larmorx cannot run yet** and which tool each needs (antsRegistration, antsAI, Atropos, 3dvolreg, 3dUnifize, 3dAutomask, BET, FAST, FLIRT-BBR, mri_coreg, lta_convert, the confounds). This becomes the roadmap table.

## Metrics per stage
- Bit-identical or not.
- For intensity images: max |diff|, max relative diff, NCC.
- For masks and labels: Dice and changed voxels.
- For transforms and motion: parameter differences in mm/degrees, displacement, FD correlation.
- Timing: fMRIPrep's node time from the logs or the node's `_report` against larmorx's time, with threads stated.
- **Separate the causes:**
  - thread effects: fMRIPrep's thread count against ANTs at one thread, which you can run;
  - glibc's maths: the CORE-MATH preload, `scripts/build_crmath_preload.sh`, `run_ants(..., correctly_rounded=True)` in the validation harness;
  - version differences.
  Apply PLAN.md §11.3's thresholds and say which pass.

## What to deliver
- **A reusable command** in the validation package, e.g. `python -m larmorx_validation pipeline-stages --oracle <run dir> --out docs/validation`. It finds the stages in any fMRIPrep work directory of this layout, so it can be re-run on future oracle runs. It reads nipype's text files (`command.txt`, `_report`), not pickles where avoidable.
- **The record** `docs/validation/fmriprep-stages.{md,json}`. Its first section is a table, one row per stage, with the result. It states the fMRIPrep version, digest and the oracle run, and its headline numbers are against fMRIPrep as it ran.
- **Findings** in `docs/findings/fmriprep-oracle.md`: anything surprising about fMRIPrep 25.2.6's actual behaviour (parameters nipype passes or drops, thread settings, nondeterminism, differences from `docs/analysis/`), with file:line or `work/` paths.
- Updates to `docs/overview.md`, if a number there changes, and to the "what is still needed" roadmap in a short doc section.

## Rules (from CLAUDE.md; non-negotiable)
- **Never in CI:** this needs real data and the oracle run. Any pytest wrapper must skip cleanly without them. Add no golden cases unless a new public function appears.
- **Commit early and often;** make a "WIP checkpoint" commit if you must stop mid-piece.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `maturin develop --release`, then `pytest tests/python tests/golden`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- If a stage shows a larmorx **bug** (a difference not explained by threads, glibc maths or versions), do not hide it. Record it, and fix it if it is small and clearly within an existing tool, with tests. Otherwise describe it precisely in the report.
- Write intermediate larmorx outputs under `oracles/compare/fmriprep-25.2.6/` (outside git), never into the oracle run directory. Never commit image data.
- **Wait loops must not match themselves** (CLAUDE.md). Use `[x]`-bracket patterns or file checks.
- Upstream sources and the oracles are read-only. Never open FSL source, `reference_src/`, `crates-gpl/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- **CPU:** you are the only agent running. A parity re-run of DenoiseImage (`python -m larmorx_validation parity ants-denoise`) may still be running when you start; do not kill it. Timings you report must come from a quiet machine (`uptime`), or be marked as shared.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- The per-stage table, with headline numbers.
- Every unexplained difference, and any larmorx bug found or fixed.
- The roadmap list of missing tools.
- Findings about fMRIPrep 25.2.6.
- Commits.
