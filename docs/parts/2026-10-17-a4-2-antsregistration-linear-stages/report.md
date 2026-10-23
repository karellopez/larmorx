# Final report: A4.2: antsRegistration linear stages

Agent `a1a3657e76367601a`, 2026-10-17. The agent's report to the orchestrator, verbatim.

## A4 part 2: report

All three parts are delivered. Part 1 is complete except for three refused features, listed at the end. The branch is `worktree-agent-a1a3657e76367601a`: 24 commits on top of `95e77d9`, HEAD `39f456f`. The tree is clean and nothing was pushed or merged. No commit carries AI attribution, and CLAUDE.md was not edited.

### Parts 0a and 0b (done earlier in this session)
- **0a, HDF5 fixtures:** `b3b8fe5`, golden checksums in `8c72fef`.
  - `scripts/make_hdf5_fixtures.py` makes five small files in `tests/golden/hdf5/`.
  - Golden case `hdf5.fixtures`; documented in `docs/validation/golden.md`.
- **0b, `antsApplyTransforms` transform outputs:** `32c5356`, record `7b6c77e`, API page `7b65469`, golden `75bf5ab`.
  - Every form of `-o Linear[...]`, `CompositeTransform[...]` and `DisplacementField[...]` is implemented.
  - `ants-apply-transforms` record: 220 cases, 202 pass, 14 rejected by both, 4 expected divergences (all ANTs crashes). One new register entry, ANTS-48.

### Part 1: `antsRegistration` linear stages
**What was built:**
- A new crate, `larmorx-registration`, porting ITK's v4 registration framework.
- `larmorx_ants::registration` (ANTs' registration helper) and the CLI `larmorx ants antsRegistration`.
- `lx.ants.registration` in Python, with `.pyi` stubs.
- `docs/api/ants-registration.md` with the option table, and `docs/findings/ants-registration.md`.
- `PROVENANCE.md` updated in the registration, ants, image, interp and io crates.

**What it covers:**
- **Command line:** every option, message and exit status of ANTs 2.6.5, with verbose output byte for byte. Deformable stages are parsed and then refused, naming A5.
- **Transforms:** Translation, Rigid, Similarity, Affine and CompositeAffine, in 2D, 3D and 4D, double and float.
- **Metrics:** Mattes/MI, MI2, MeanSquares, GC, CC (dense and sampled); Demons fails in a linear stage, as in ANTs; several metrics per stage.
- **Optimisation inputs:** the three sampling strategies with ANTs' seeding, masks (including `NULL`/`NOMASK`), shrink factors and smoothing sigmas, convergence, the optimiser with its scales and learning rate, winsorizing, histogram matching and the gradient filter.
- **Initial transforms:** transform files in any dimension, including inverted ones (`-r [file,1]`), and the centre, centre-of-mass and origin initialisations (`-r`/`-q`); also `-i 1`, `-k` and `-j`.
- **Outputs:** collapsing (`-z`), the composite `.h5` files (`-a 1`), every warped-image interpolator in 2D, 3D and 4D (new ND interpolators in `larmorx_interp::nd`), `--write-interval-volumes` and `--print-similarity-measure-interval`.

**Parity against the GCC 14.3 oracle**, run at one thread with `ANTS_RANDOM_SEED=42`. Both records were run from the same clean commit (`36aa668`):

| Tier | Cases | Identical to unmodified ANTs | Identical with CORE-MATH preload | Rejected by both | Expected divergences | Failures |
|---|---|---|---|---|---|---|
| standard | 166 | 102 | 24 | 31 | 9 | 0 |
| full | 175 | 102 | 33 | 31 | 9 | 0 |

- Records: `docs/validation/ants-registration.md` and `docs/validation/ants-registration-full.md`.
- Everything that differs from the unmodified ANTs differs only because glibc's `exp`/`log`/`sin`/`cos` are not correctly rounded.
- **Real data:**
  - four fMRIPrep 25.2.6 nodes as they ran;
  - in the full tier, the linear stages (SyN left out) of sMRIPrep's brain extraction and template registration on all five oracle runs. All 10 pass.
  - nipype did not keep some intermediates, so these cases use stand-ins: the run's `desc-preproc_T1w` and masked template/T1w images. One registration's starting transform was also missing; those cases start from a centre-of-mass alignment instead.
- **ANTs' own thread band** (14 cases, 2/4/8 threads against 1): about 1e-13 to 1e-12 in a transform parameter in double, 3.5e-9 on fMRIPrep's BOLD-reference affine, and 5e-3 on a float affine. larmorx gives the one-thread result at any thread count and is within 3.5e-18 of ANTs at one thread.
- **The 9 expected divergences:**
  - ANTs without a seed (it seeds from the clock);
  - the ANTS-51 crash;
  - SyN;
  - `--version`;
  - three cases where ANTs exits 1 without a message and larmorx explains why (ANTS-45);
  - the point-set metrics and MINC transform files, which larmorx refuses.

**Upstream defects** (register and switches):
- New: ANTS-49 (a failed stage is dropped and the run still exits 0), ANTS-50 (`--save-state` after linear stages exits 1 without writing anything), ANTS-51 (transform file names miscounted, then a crash) and ITK-18 (MI2's marginals are swapped). All are fixed by default, with a `reproduce` switch.
- ANTS-46, fMRIPrep's "early stop", is explained as ITK's signed convergence test, not a defect.
- ANTS-02 and ANTS-45 now include antsRegistration.

**Golden tests:** five new cases (`cli.ants.antsRegistration` plus `.metrics`, `.dimensions`, `.reproduce`, and `py.ants.registration`). They were recorded from `7b125d5` after the parity smoke tier passed; no existing checksum changed.

**Gates on HEAD:** both Rust workspaces pass `fmt`, `clippy -D warnings` and `test --locked`. `ruff check`, `ruff format`, `check_spdx.py` and `pytest tests/python tests/golden` (366 passed) all pass, and the parity smoke tier passed. `crates-gpl/Cargo.lock` did not need refreshing.

### Not done (refused with a message, documented in the API page and NOTES)
1. **Point-set metrics (ICP, PSE, JHCT, IGDM).** This is a large separate port, about 3,800 lines of ITK/ANTs C++: an exact kd-tree, four metrics, point-set readers for label images, VTK and text files, and ANTs' intensity point-set filter. Below 100 % sampling, ANTs seeds its point sampling from the clock, so it cannot be compared there. fMRIPrep, sMRIPrep and sdcflows never use these metrics. I recommend scheduling them as their own task.
2. **Displacement fields among the initial transforms.** These need A5's displacement-field transform.
3. **MINC transform files.** The oracle itself has no MINC support (it prints ITK's error and exits 0).

Notes for whoever continues: `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/a4-registration/NOTES.md`. Logs are in `/home/karelo/VScode_projects/super_fmriprep/oracles/logs/a4p2-*`.
