# Brief: Porting guide + upstream defect register

As given to agent `a6cbcf2eeebad0df0` on 2026-10-11, verbatim (paths are this machine's).

You are writing two reference documents for the larmorx project, so that every future port, by an agent or a person, starts from what has been learned. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. This is a **documentation-only** task: no code changes.

Read first: `CLAUDE.md` (especially rule 1 on licences, rule 5, the new rule 8 "the source is the specification; an oracle binary is only a check", rule 10, and D11/D13 in "The user's direction"), `docs/overview.md`, `docs/licensing.md`, `docs/decision-log.md` (all of it; the 2026-10-10 and 2026-10-11 entries matter most), PLAN.md §4 and §11.

Then read **every** file in `docs/findings/`, every `specs/*.md`, every `docs/validation/*.md`, `docs/validation/golden.md`, and `docs/analysis/01-04`.

**Licence boundary:** you may read all of these because you write no implementation. But the porting guide is for clean-room implementers as well, so it must not contain source-derived details of FSL, AFNI-MCW or Workbench internals. It covers method only. The defect register may describe upstream behaviour at the level the existing findings do. Mark the entries that come from source-derived findings (e.g. `docs/findings/fsl-mcflirt.md`, `afni-tshift.md`) so a clean-room implementer knows not to read them. Never open FSL source, `reference_src/`, `crates-gpl/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.

## Document 1: `docs/porting-guide.md`, the method
Written for whoever ports the next tool: a precise checklist with the reasons behind it, drawn from what actually happened. Cover at least:
- **The two tracks and when each applies** (link `docs/licensing.md`); the clean-room process and what each role may read.
- **The source is the specification** (rule 8). The N4 case: GCC 11 `-O3` dropped two float roundings, larmorx copied the binary, fMRIPrep's build follows the source. Rules:
  - how to tell a compiler artefact from source semantics;
  - prefer oracle builds that honour the source;
  - record the oracle's exact build (compiler, flags, versions);
  - compare with a second build of the original (e.g. the one inside fMRIPrep's image) when results disagree.
- **Measure the original's own variability first:** thread counts (ITK's work units; N4's and DenoiseImage's thread dependence; DenoiseImage not reproducible at ≥ 8 threads), repeated runs, seeds, small input perturbations (mcflirt's band). This defines what "equivalent" can mean.
- **Separate the causes of every difference, one link at a time:**
  - glibc's not-correctly-rounded maths: the CORE-MATH `LD_PRELOAD`, `scripts/build_crmath_preload.sh`;
  - compiler artefacts;
  - versions;
  - threads;
  - BLAS kernels (numpy's OpenBLAS; `docs/findings/numpy-blas.md`).
  Use the chain the stage comparison used.
- **Validation records lead with the result against the unmodified original.** Explanatory comparisons (preloaded, other builds) come second. Statuses `experimental`/`validated`/`stable` follow PLAN.md §4 and rule 8 (no equivalence claims while a case fails).
- **D11 and D13**, with examples:
  - improvements by default within the band;
  - deterministic upstream bugs fixed by default with a warning and a compatibility switch;
  - undefined behaviour, races and nondeterminism never reproduced;
  - how parity tests use the compatibility switch.
- **Read the pipeline's real parameters,** not only the docs:
  - nipype drops arguments given to `Node` (e.g. `shrink_factor`);
  - threads arrive as `NSLOTS`;
  - `--random-seed` becomes `ANTS_RANDOM_SEED`;
  - niworkflows rewrites headers;
  - fMRIPrep's image sets `OMP_NUM_THREADS=1`.
  Confirm every one on an oracle run's `work/` directory.
- **Determinism and platforms:**
  - correctly rounded maths;
  - fixed operation order, and deterministic scatter-to-gather where the original races;
  - no platform BLAS for results;
  - golden tests;
  - the CI budget (rule 10).
- **Practical lessons:**
  - commit early (agents were killed by usage limits);
  - one agent at a time;
  - wait loops must not match themselves;
  - never delete work products;
  - the shared venv (`PYTHONPATH` first);
  - the crates-gpl lockfile;
  - test data generated deterministically (gzip mtime 0);
  - SPDX headers and notices for translated permissive code.
- **The per-tool contract checklist,** with links to the models that did it best (e.g. `docs/validation/ants-n4.md`, `docs/api/afni-tshift.md`, `crates/larmorx-afni/PROVENANCE.md`).

Keep it a guide, not a history. Point to the decision log and findings for details instead of repeating them.

## Document 2: `docs/findings/upstream-defects.md`, the register
One entry per defect or questionable behaviour found in an original tool or pipeline (ANTs/ITK, AFNI, FSL, fMRIPrep/niworkflows/sMRIPrep/nipype, nitransforms, SciPy, numpy/OpenBLAS, glibc), with:
- the tool and version;
- what is wrong;
- the evidence (file:line where the existing findings give it; or how it was observed);
- the class: (a) crash / undefined behaviour / out-of-bounds read, (b) race or nondeterminism, (c) deterministic bug that changes results, (d) imprecision (e.g. not correctly rounded), (e) CLI or parsing quirk, (f) documentation or message only;
- its impact on fMRIPrep users;
- **what larmorx does today**;
- **what D13 requires** for classes (c) and (e): fixed by default, with a warning and a compatibility switch;
- **the gap,** if larmorx today reproduces a class-(c) bug by default.

Collect every one already in the findings, specs and records. Examples to look for, not an exhaustive list:
- AFNI 3dTshift: the negative-slope datum bug, `fft_shift2`'s NULL write, the sequential-decreasing slice-code bug, double scaling of float data with slope and intercept, the 1D reader's two roundings, the error message wording, `-no_detrend` pairing;
- mcflirt: the last volume not initialised from the previous one, normcorr's inflated count (effectively uncentred), the abort cases;
- ANTs:
  - FillHoles reading outside the image, ClusterThresholdVariate, `Otsu 0` hanging;
  - ResampleImageBySpacing on 3-voxel axes returning garbage, ResampleImage size 1 with infinite spacing, ResampleImage's 7th argument read twice;
  - ImageMath Laplacian reading its normalize flag from the sigma argument;
  - N4's and DenoiseImage's thread dependence, DenoiseImage's races;
  - the GD/GC signed zeros;
- ITK: the histogram min/max `else if`, the SSE2 `floor` returning 0 for tiny negatives;
- fMRIPrep: the ResampleSeries OpenBLAS race; nipype dropping `shrink_factor`; the DataSink's wrong dtype log message;
- SciPy: index overflow for huge coordinates;
- numpy/OpenBLAS: kernel-dependent last bits;
- glibc: `logf`/`expf`/`sin`/`cos` not correctly rounded.

At the top, put a summary table, and a short list of **D13 gaps**: the class-(c) and (e) defects larmorx reproduces by default today, which the next task will switch to "correct by default". For each, say which larmorx tool and code path it touches, so that task can be briefed precisely.

## Also
- Link both documents from `CLAUDE.md` (the repo map and the hard rules where relevant), `docs/overview.md`, `docs/findings/README.md`, and `docs/architecture.md` ("Adding a tool" should send porters to the guide first).
- Do not change any validation record's numbers.

## Rules
- **Commit early and often** (a "WIP checkpoint" commit if you must stop).
- **Gates before committing:** `ruff check && ruff format --check` and `python scripts/check_spdx.py` (the venv is `/home/karelo/VScode_projects/super_fmriprep/.venv`). Code gates are not needed for docs-only changes, but run `python -m pytest -q tests/golden` once, to confirm nothing imports docs.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.**
- You are the only agent running. A long validation run (`python -m larmorx_validation pipeline-stages`) is going on the machine; do not touch it.
- Write in plain, direct English, in the style of the existing docs: short sentences, concrete numbers, file:line where known.

## Report back
- The structure of both documents.
- The number of defects by class.
- The precise list of D13 gaps for the next task.
- Commits.
