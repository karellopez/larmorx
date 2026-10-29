# larmorx: project context

Read this first. The full plan is [PLAN.md](PLAN.md) (v0.3.2). Why the plan looks the way it does is in [docs/decision-log.md](docs/decision-log.md). A plain-language summary of the concepts (replica, clean-room original, oracle), the packages, installation and what is published is in [docs/overview.md](docs/overview.md).

## What we are building
One package, **`larmorx`** (repo `github.com/karellopez/larmorx`), with two layers:

1. **The tool library**: `pip install larmorx`, then `import larmorx as lx`.
   - ANTs/ITK, AFNI and FreeSurfer functions are **ported** to Rust.
   - FSL-like and Connectome-Workbench-like functions have **both** a replica (licence-segregated package) and a clean-room original (Apache-2.0); see `docs/licensing.md`.
   - Every tool has three interfaces, plus documentation and a validation record (PLAN.md §4):
     - a Rust API (crates `larmorx-*`)
     - an idiomatic **Python wrapper** (`lx.ants.registration(...)`)
     - a CLI that accepts the original program's arguments (`larmorx ants antsRegistration ...`, short alias `lx`)
2. **`larmorprepx`**, the preprocessing pipeline, **inside the package**.
   - Module `larmorx.pipelines.larmorprepx`; command `larmorprepx`; installed via the `[prep]` extra.
   - An fMRIPrep-like pipeline in **plain Python functions** on top of the library. BIDS in, fMRIPrep-compatible derivatives and HTML reports out.
   - `larmorx.pipelines/` leaves room for future pipelines.

The FreeSurfer port is a separate wheel, `larmorx-freesurfer` (exposed as `lx.freesurfer`), so its licence terms stay isolated. **Distribution** (user, 2026-10-16): companion wheels per licence family, `pip install larmorx` Apache-only, `pip install "larmorx[all]"` everything; one `import larmorx` and one `larmorx` command either way, `implementation="auto" | "replica" | "original"` where a tool has both.

**Naming rules:**
- Always lowercase `larmorx` / `larmorprepx` in code and commands.
- In prose the project may be written **LARMORX**, the acronym of *Library of Algorithmic Reimplementations for MRI through Open Rust eXecution* (user, 2026-10-19); tagline: "LARMORX — Bringing Neuroimaging Tools into Resonance. Reimplemented in Rust. Accessible through Python. Built for FAIR research." Its identity and FAIR4RS philosophy are set out in README.md.
- Never use plain `larmor` for anything of ours: an unrelated NMR package already ships a `larmor` command.

**Targets:** native wheels for Windows x64/arm64, Linux x64/aarch64 and macOS x64/arm64. No Docker, no external neuroimaging installs, no admin rights.

## Where things are (2026-10-28)
Phase L1 in progress. History: [docs/status-log.md](docs/status-log.md) (to 2026-10-26) and the merge commits; decisions: [docs/decision-log.md](docs/decision-log.md); **the plan, its order, the follow-ups and the decisions waiting for the user: [docs/next-steps.md](docs/next-steps.md)** (keep it current).

**Ported and validated** (one record per tool in `docs/validation/`, each leading with the results against the unmodified original):

| Family | Tools | Records |
|---|---|---|
| I/O and transforms | NIfTI-1/2 I/O (`lx.load`, `lx.save`, `lx.Image`); ITK transform files, HDF5 included, read and written in pure Rust (`larmorx-hdf5`); fMRIPrep's one-shot BOLD resampler (`lx.transforms.resample_series`) | `nifti-io`, `itk-hdf5`, `resample-series` |
| ANTs and ITK (ports) | antsApplyTransforms (`--float 1`, transform outputs, B-spline transforms), N4BiasFieldCorrection, DenoiseImage, ImageMath (every operation fMRIPrep, sMRIPrep and sdcflows call), ThresholdImage, MultiplyImages, SmoothImage, ResampleImage(BySpacing), antsRegistration (every stage: linear, SyN, BSplineSyN, BSpline, displacement-field, Exponential, BSplineExponential and time-varying velocity-field; image and point-set metrics; the corrections of 2026-10-21 on by default), antsAI (every option, the blob alignment included), antsAffineInitializer, Atropos (every option; sMRIPrep's call bit-identical with correctly rounded maths on the five oracle runs); **re-validated on the data panel** (2026-10-27: every ANTs node of fMRIPrep's fourteen oracle runs and the panel's own images, records `ants-*-panel`; the fourteen whole template registrations at full size, 2026-10-28: 14 of 14 identical with correctly rounded maths) | `ants-*` |
| AFNI | 3dTshift, 3dAutomask, 3dUnifize, complete: clean-room originals (`crates/larmorx-afni`) and GPL replicas (`crates-gpl/`) | `afni-tshift`, `afni-tshift-replica`, `afni-automask`, `afni-automask-replica`, `afni-unifize`, `afni-unifize-replica` |
| FSL | head-motion correction with mcflirt-style options, clean-room original, `experimental` (`lx.mri.hmc`); the mcflirt replica (`crates-nc/`, package `larmorx-nc`, FSL Licence: non-commercial, FSL's source in `crates-nc/fsl-source/`), bit-identical to mcflirt as built (173 of 173 compared cases of its record, fMRIPrep's 22 calls on the twelve oracle runs with BOLD runs included, and fMRIPrep's own matrices identical); `larmorx-nc-core` is the shared core of the next FSL replicas; the **BET** replica (`larmorx-nc fsl bet`, `bet2`, `betsurf`; `lx.fsl.bet`, `lx.fsl.betsurf`; `bet -S`, `-B`, `-A`, `-A2` pending FLIRT and FAST): every file FSL 6.0.7.17 writes byte for byte in every compared case (129 of 129, 364 files, fMRIPrep's 29 BET calls on the twelve oracle runs with BOLD runs included); its FSL libraries are the core's, shared with mcflirt | `mri-hmc`, `fsl-mcflirt-replica`, `fsl-bet-replica`, `fsl-bet-replica-full` |

Golden tests cover every tool on all six platforms (`docs/validation/golden.md`); CI has been green at every merge. Stage-by-stage comparisons with fMRIPrep 25.2.6 were run on four oracle runs (`docs/validation/fmriprep-stages*.md`); further fMRIPrep comparisons wait until every tool is ported (user, 2026-10-16).

**In place** (how each came about: [docs/status-log.md](docs/status-log.md), 2026-10-21 to 26; why: the decision log):
- **Interactive evidence** for every tool: `python -m larmorx_validation evidence <tool>|summary|all --site <site>` ([docs/figures.md](docs/figures.md)); the site's images on its orphan branch `evidence-blobs` (`tools/blobs.py`), a size guard that never removes content, credits from the test-data catalog.
- **The oracle-output cache** ([docs/validation/oracle-cache.md](docs/validation/oracle-cache.md)): every original program's run computed once; `--oracle-cache use|refresh|off`; runs the original varies always run.
- **The data panel:** fourteen OpenNeuro subjects (larmorx-testdata `docs/panel.md`, `fetch --tier full --tag panel`), and fourteen fMRIPrep oracle runs (below).
- **Differential random testing** ([docs/validation/difftest.md](docs/validation/difftest.md)): `python -m larmorx_validation difftest`, nightly by `scripts/orchestration/difftest-nightly.sh`, in memory boxes; heavy cases queued (25 wait for a larger machine, 11 are beyond any machine and checked as guard cases).
- **Generated tables:** `python -m larmorx_validation tables [--check]` writes the README's tool table, the validation index and the site's status from `validation/tools.toml` and the records.
- **Memory:** heavy jobs in boxes (`scripts/orchestration/boxed.sh`); larmorx's peak similar to the original's (at most 10 % above plus 8 MiB, `tests/parity/test_memory_peak.py`, antsAI and antsAffineInitializer included since 2026-10-28: their images in float); the thread pools exact (every call's parallel work in its own pool of `n_threads`: `tests/parity/test_thread_pools.py`, 74 paths); guards stop impossible sizes before allocating (ANTS-162, AFNI-75). The originals the harness runs (and larmorx's in-process runs of the ANTs image programs) run under a 64 GiB address-space limit, so an impossible allocation is refused alike on every machine (`validation/src/larmorx_validation/address_space.py`, 2026-10-28; random testing: twice each case's memory budget).
- **FSL:** `crates-nc/` (`larmorx-nc`, FSL Licence) with its shared core `larmorx-nc-core`; the mcflirt and BET replicas.
**Next:** the move to the new main machine is done (2026-10-28: the copy, the build and the proof
that results do not depend on the machine, [docs/findings/machine-move-proof.md](docs/findings/machine-move-proof.md));
the old laptop is a helper (phase 4 of [docs/machine-move.md](docs/machine-move.md)). Ports first,
with smart testing per part; the very extensive testing in one final validation phase (user,
2026-10-28; decision log). One agent at a time (`docs/next-steps.md`, "The order from here").
Done: antsAI and antsAffineInitializer's memory and speed (2026-10-28), the clean-room fixes
(2026-10-29, `-cost normmi` left open). Next: the version update (AFNI 26.3.05 and FSL 6.0.7.23,
installed beside the old ones), then BET's clean-room original (from `specs/bet.md`) and the next FSL tools (FLIRT with BBR, FAST, topup),
field maps, FreeSurfer, surfaces, confounds, larmorprepx; each a bounded agent run with a stop
after it, each with its site page from its tests. Then the final validation phase (the data
panel's part b and every new tool, full tiers, heavy and nightly random testing, the extensive
evidence, reviews, the scripts triage) and the comparisons with fMRIPrep. Machine-only runs
continue meanwhile: the 25 heavy random-testing cases (17 running here, 8 for the lab's server).
**Build-cache seed** for the next agent: on the new machine, the main checkout's `target/`, `crates-gpl/target/` and `crates-nc/target/` (rebuilt by `verify-merge.sh` at each merge, release and test profiles).

**fMRIPrep oracle runs** (`oracles/fmriprep-25.2.6/<run>/`, fMRIPrep 25.2.6 through Apptainer, `scripts/run_fmriprep_oracle.sh`; settings in each run's `RUN_INFO.txt`): `ds000005-sub-01-noreconall` (no field maps, no slice timing), `ds001600-sub-1-noreconall` (anatomy only: its BOLD runs have 5 volumes), `ds005040-sub-002-noreconall` (PEPOLAR, slice timing), `ds005454-sub-16-noreconall` (phase difference, multiband, 280 volumes), `ds005752-sub-ON01016-noreconall` (a direct field map fMRIPrep does not use: no `IntendedFor`); and the data panel's nine (2026-10-24 and 25, `validation/investigations/panel-oracle-runs/`, every one finished with exit status 0): `ds000228-sub-pixar008-noreconall` (a 3.5-year-old), `ds005003-sub-RS001-noreconall` (a glioma), `ds004173-sub-603309-noreconall` (anatomy only, strong motion), `ds002422-sub-02-noreconall` (1.5 T, two field-map estimators), `ds000256-sub-CTS203-noreconall` (a child's three motion levels), `ds004884-sub-M2079-noreconall` (a stroke, with its lesion mask resampled onto the T1w's grid), `ds003592-sub-280-noreconall` (multi-echo), `ds002603-sub-01-noreconall` (Philips multiband, a field map), `ds004958-sub-fsm64ps-noreconall` (multiband-8, 600 volumes; field maps not linked).


## The user's direction (settled; do not re-propose the alternatives)
- **Not a fork of fMRIPrep.** No nipype, traits, niworkflows/smriprep/sdcflows runtime dependencies, or graph engines. Ops are plain typed functions; per-subject orchestration is readable Python.
- **Port to Rust** where existing Python packages are limited, **including ANTs**: ANTsPy has no Linux-aarch64, Windows-arm64 or Python-3.14 wheels. Also **FreeSurfer recon-all**: the goal is the same results, several times faster.
- Rust implementations may **reimagine** algorithms for speed, provided validation shows equivalent results.
- **Better algorithms by default (D11, user decision 2026-10-10).** larmorx should be an evolution of the upstreams: faster and better, not only replicas. An improved algorithm becomes the default once its results stay within the upstream tool's own variability (PLAN.md G4, §11); larger changes stay opt-in. Replicas always stay exact.
- **Correct by default, faithful on request (D13, user decision 2026-10-11).** "What I like at the end is to have the best and methodically correct implementation." When an original has a *deterministic* bug that changes results (e.g. AFNI's negative-slope scaling, mcflirt's last-volume initialisation), larmorx fixes it by default and warns when the bug would have changed the result; a documented compatibility switch reproduces the original bit for bit, and parity tests use it. Crashes, out-of-bounds reads, races and nondeterminism are never reproduced by default. Details decided 2026-10-11: undefined float-to-integer conversions clamp and warn; replicas share the corrected default, and their switch gives exactly the original's result; even failure and garbage cases get a switch (exact where the original is deterministic; where it depends on memory contents, the defined parts are reproduced and the rest is filled deterministically with a warning); mcflirt's weight-count bug is measured before deciding. Every defect found goes into `docs/findings/upstream-defects.md`, and the method into `docs/porting-guide.md`. **Questionable behaviours (c?) are decided by four rules, without asking the user** (user, 2026-10-23; decision log): **A** the input makes sense and the result is wrong: correct it; **B** the input makes no sense and the original silently does something arbitrary: a clear error; **C** possibly a deliberate design that pipelines rely on, or a correction measured to bring no gain: keep and document; **D** unknown whether correcting helps: measure first, then A or C. Every case keeps the `reproduce` switch. Only a case the rules cannot settle goes to the user.
- **Every platform from the start** (user, 2026-10-10): Linux x64/aarch64, macOS Intel and Apple silicon, Windows x64/arm64. Design, test and benchmark with all six in mind; CI must stay green on all of them.
- **How to port (user, 2026-10-09):**
  - Make a faithful **replica** when the licence allows it: ANTs/ITK (Apache-2.0), AFNI's NIH public-domain code, CORE-MATH (MIT), nibabel (MIT). Bit-identical results are the target.
  - When the licence restricts it (AFNI's MCW code, Workbench, FSL), make **both**: a bit-exact replica in that licence family's package, and a clean-room original in the main package (rule 1, `docs/licensing.md`).
  - **FreeSurfer: both as well** (user, 2026-10-26): a replica of every program in `larmorx-freesurfer` and a clean-room original of every program in the main package, built for speed (FreeSurfer is the slowest stage), every program complete; version 8.2.0 (decision log 2026-10-26).
  - MSM: a clean-room original only, in the surfaces track, after a patent check.
- **Each tool gets its own Python wrapper**, so other developers can build on the library.
- **Clean-room originals are built for speed** (user, 2026-10-19): the original's behaviour within the validated thresholds, but designed to be much faster than a replica, with the best methods (efficient algorithms, cache-friendly memory, deterministic multithreading, SIMD with runtime dispatch), benchmarked against the original and the replica (`docs/porting-guide.md`, "Clean-room originals: the same behaviour, built for speed"). Replicas stay bit-exact first.
- **Full completeness for every tool** (user, 2026-10-12, asked whether to scope new tools to fMRIPrep's code paths first): every option, message and failure case, with D13 switches, as for 3dTshift. One agent at a time; A4 after the stage comparisons (decision log 2026-10-12).

- **Newest upstream versions, updated in steps** (user, 2026-10-27): each tool is ported from the newest release; a newer release is brought in by a planned part (source changes ported, records re-made), the old oracle kept. fMRIPrep's own versions are only a reference for the final comparisons (the user: "I would not like larmorx to be limited by fmriprep. The idea was that it reproduces fmriprep, but also to bring a solution in which user can simple use whatever they want to make software"). AFNI (25.2.09, newest 26.3.x) and FSL (6.0.7.17, newest 6.0.7.23) are updated after the move (decision log 2026-10-27).
- **Port every tool first, compare with fMRIPrep at the end** (user, 2026-10-16). Each port is validated against its own original program; stage-by-stage and end-to-end comparisons with fMRIPrep wait until all tools are ported.
- **One agent at a time, in bounded parts with a stop after each** (user, 2026-10-10 and 2026-10-17): the user watches the weekly usage limit (only they can see it, with `/usage`), and starts each next part. A second agent in parallel **only when the user explicitly asks** for it (user, 2026-10-22: "Lets keep the one agent at the time in the docs. But you can use 2 agents only when I tell"; that day A5 part 3 and A6 ran together); never more than two unless the user says so (2026-10-24: four at once, for the evidence infrastructure and FSL, "I will tell you if something gets wrong"). **From 2026-10-26: one agent at a time** (user: "I would like to continue the development but with 1 agent at a time"), until the user says otherwise. On the main machine (32 threads) one agent may use up to 28 threads (cargo `-j 28`, parity `--jobs` to match); parallel agents split them: two at most 14 each, four at most 7 each; the orchestrator merges them one after the other.
- **Licence: Apache-2.0 kept** (user, 2026-10-19). Royalty or source-available schemes declined; the replicas keep their upstream licences. **GitHub organisation:** not now (2026-10-19), to reconsider before the first public release.
- **Figures and a documentation site** (user, 2026-10-20): every tool gets elegant figures comparing it with the original (rule 7), drawn by code in this repository from the validation records and published on the site, repository `larmorx_documentation` (`github.com/karellopez/larmorx_documentation`, live at https://karellopez.github.io/larmorx_documentation/ since 2026-10-20, deployed by GitHub Pages on every push; style of the lab's `bids_manager_documentation`, hero: spins coming into resonance).
- **The site's evidence only ever improves** (user, 2026-10-24: "I do not really want to have less quality than now, always better quality and information"): nothing the site shows is removed, downsampled or turned into slices; saving space only where nothing is lost (cropping margins where every array of a viewer is exactly constant, lossless encodings); space-saving forms only for new content, added on top; a page over its size budget gets a larger budget, never less content. Every part that touches the site proves it with a before/after inventory per page (after >= before) and before/after screenshots, checked at the merge.
- **CI stays as it is** (user, 2026-10-16): every push to `main` that is not docs-only runs the full six-platform workflow.
- **Very strong evidence** (user, 2026-10-23: "reliable programs replicas, clean room, and very very strong evidence"): besides the parity suites against the originals (as built, with correctly rounded maths, in fMRIPrep's image, at several thread counts) and the explained differences, (1) shared documents go through the orchestrator (above), (2) the tables that repeat facts (README tools, validation index, the site's status) are generated from the records, (3) every tool is validated on one fixed **data panel** (3T and 7T, multiband and single-band, children and older adults, a lesion, low quality and motion), (4) **differential random testing** against the original runs overnight for every tool, and (5) large ports get an **independent review** before merging (decision log 2026-10-23). (3) to (5), with the full tiers and the extensive evidence, run in **one final validation phase** once every tool is ported (user, 2026-10-28: "port tools to the last version first with a decent but not heavily extensive testing ... Those repeated tests may be worth it at the very end as the cherry in the cake"); each part meanwhile tests smartly ("Every part, in full", below).
- **Speed at the same quality** (user, 2026-10-23): oracle outputs are cached by content and never computed twice (the oracle-output cache, since 2026-10-25: verification re-runs only larmorx; variation is still measured fresh), no duplicate runs (the golden recording reuses the parity smoke result through its marker; the long tier once, at the end), small questions decided in batches. **Agents run on the model the user chooses** (now the session's): never launch an agent with a different or cheaper model.
- **Working with the user** (user, 2026-10-23 to 26): explain in plain, simple words; report at milestones (a merge published, a decision needed), not at every agent wake-up; decide what the rules settle and bring only the rest, in one batch with a recommendation. To spare the weekly usage limit (2026-10-25: 13 % in 13 hours with five agents and a crash): one agent at a time (2026-10-26); agents read a lean CLAUDE.md (history in `docs/status-log.md`); a fresh orchestrator session at each clean stop (the state is in the files: `ORCHESTRATOR-STATE.md`, this file, the decision log); large handoffs integrated by a helper agent with a small context, then reviewed. Never ask for passwords, tokens or sudo; GitHub sign-in happens through VS Code only.
- **Memory similar to the original's** (user, 2026-10-25, refined 2026-10-27: "similar ... not exact"): for the same call larmorx's peak
  memory is at most 10 % above the original program's plus the interpreter's 8 MiB; no full-size
  copies where the original holds none, and no speed cache that raises the peak.
  `tests/parity/test_memory_peak.py` holds the measured calls; every new port adds its own
  (`validation/investigations/difftest/memory_pair.py` measures one).
- **Settled technical choices** (decision log): `.h5` files compared by content, not compressed bytes (2026-10-16; byte-identical deflate deferred); pretrained models re-implemented as Rust inference with the original weights, validated as equivalence (2026-10-19); FreeSurfer's licence key is a runtime check, not a licence term, so `larmorx-freesurfer` needs none; Washington University's Talairach step gets a clean-room original with its atlas fetched from FreeSurfer on first use (2026-10-19; `docs/licensing.md`).

## Workspace layout
```
<workspace>/                     e.g. ~/.../super_fmriprep
  CLAUDE.md                      imports this file (written by the bootstrap script)
  .venv/                         Python venv (evaluation: fmriprep, antspyx, SimpleITK)
  larmorx/                       THIS REPO (github.com/karellopez/larmorx)
  fmriprep/ smriprep/ niworkflows/ sdcflows/ nireports/ nitransforms/ nipype/ nibabel/
  nitime/ pybids/ python-client/ tedana/ acres/ migas-py/
  ANTs/ ITK/ afni/ freesurfer/   read-only upstream references, pinned in upstream.tsv
  reference_src/workbench/  reference_src/MSM_HOCR/     licence-restricted sources (docs/licensing.md)
  larmorx_documentation/         the documentation site (github.com/karellopez/larmorx_documentation; its own CLAUDE.md)
  larmorx-testdata/              the test-data catalog and downloader (github.com/karellopez/larmorx-testdata, public since 2026-10-23)
  tags/                          the pinned upstream releases ported from (ANTs v2.6.5, ITK v5.4.5, AFNI 25.2.09, ...)
  oracles/                       oracle builds (ANTs, AFNI, the CORE-MATH preload), fMRIPrep runs, logs, templates
  archive/                       work moved out of the way, never deleted (see "Working on this machine")
  ORCHESTRATOR-STATE.md          the orchestrator's in-flight state, imported by the workspace CLAUDE.md; kept current at every step
```

Recreate it with:
```
larmorx/scripts/bootstrap_workspace.sh --venv .venv --install-rust
```

## Hard rules
1. **Licences: two tracks, kept apart** (user decision, 2026-10-09). See `docs/licensing.md`.
   - **Replica:** a bit-for-bit port of the upstream source, for the most accurate output. It carries the upstream's licence and lives in a package per licence family:
     - permissive upstreams (ANTs/ITK, AFNI's NIH files, CORE-MATH, nibabel): the main `larmorx` package (Apache-2.0);
     - AFNI's MCW files and Workbench (GPL-2.0-or-later): `larmorx-gpl` (GPL-3.0-or-later, `crates-gpl/`);
     - FSL: `larmorx-nc` (the FSL Licence's non-commercial terms, `crates-nc/`);
     - FreeSurfer: `larmorx-freesurfer`.
   - **Original:** a strict clean-room implementation with the same behaviour, Apache-2.0, in the main package. It is needed wherever the replica is not Apache-compatible. The implementer works only from `specs/<tool>.md`, documentation, papers and black-box runs of the upstream binary; never the upstream source, the replica crates or source-derived findings (`docs/findings/afni-tshift.md`, `afni-automask.md`, `afni-unifize.md`, `afni-licence-routines.md`, `fsl-mcflirt.md`, `fsl-bet.md`, the sections of `upstream-defects.md` marked restricted), nor `docs/parts/` (other parts' briefs, reports and handoffs, some source-derived). Who may read what: `docs/porting-guide.md` §1.
   - **Never** move replica code into the main package, even renamed. Never link code from different licence families into one binary: `larmorx-gpl` and `larmorx-nc` are incompatible with each other.
   - **MSM_HOCR:** no replica can be published (its ELC library forbids public distribution of derivatives, and its FastPD optimiser is patented). Its original must avoid those optimisation methods.
   - **Every file records its origin** with an SPDX header and in its crate's `PROVENANCE.md`.
2. **Ported code records its provenance.** Each crate's `PROVENANCE.md` lists the upstream files and versions it was ported from. Port from the pinned **release tags**, not the local master/dev clones:
   - ANTs v2.6.5
   - ITK v5.4.5 (the version ANTs 2.6.5 pins; decided 2026-10-09)
   - AFNI 25.2.09
   - FreeSurfer v8.2.0 (pending D5)
3. **FreeSurfer-derived code lives only in `crates/larmorx-freesurfer`** (separate wheel, carries the FreeSurfer licence text and preface). Audit each ported file for third-party code first.
4. **Upstream clones are read-only.** Never modify them, and never vendor code from them without attribution.
5. **Rust rules:**
   - Pure-Rust dependencies only: no C, C++, Fortran or HDF5 libraries.
   - Deterministic reductions, independent of thread count.
   - Correctly rounded transcendental functions from `larmorx_core::math` (pure-Rust ports of CORE-MATH, decided 2026-10-09). They give the same bits on every platform and match glibc, and so ANTs/AFNI on Linux, in more than 99.8 % of calls. Use the `libm` crate only for functions not yet ported, and say so in the code.
   - Seeded RNG; no fast-math; no `target-cpu=native` in release builds.
   - Explicit `n_threads` on every call; release the GIL.
   - `#![forbid(unsafe_code)]` outside audited hot loops.
6. **Python rules:**
   - ≥ 3.12.
   - Typed dataclass inputs and outputs; no global config; no pickles in caches.
   - UTF-8 I/O; no `shell=True`; `spawn` only.
7. **The per-tool contract** (PLAN.md §4) is mandatory for every tool. Start every port from `docs/porting-guide.md`: the method, and the contract as a checklist with the models to copy (§11).
   - Rust API, Python wrapper, compatible CLI
   - `.pyi` stubs + a docs page with the option-mapping table
   - a validation record and status (`experimental` / `validated` / `stable`)
   - `PROVENANCE.md`
   - at least one **golden case** (rule 10), so every platform is checked against Linux
   - **figures** (user, 2026-10-20): visual evidence of how the replica and the clean-room original compare with the original (agreement overview, difference distributions, image comparisons such as registration overlays and difference maps, the original's own variability, speed), elegant and consistent, generated by code in this repository from the validation records and real-data runs (`larmorx_validation.evidence`, [docs/figures.md](docs/figures.md): how to add a tool's evidence; interactive worked examples with a verdict from the site's taxonomy, `data/verdicts.yml`, and its reason for every difference) and published on the documentation site, repository `larmorx_documentation` (style of the lab's `bids_manager_documentation`)
8. **The source is the specification; an oracle binary is only a check** (user, 2026-10-11). A replica reproduces what the upstream source defines, not what one compiler made of it. When larmorx and an oracle binary disagree and the cause is the binary's code generation (e.g. GCC 11 at `-O3` dropping two float roundings in N4's B-spline fit), follow the source, record the binary's behaviour in `docs/findings/`, and prefer an oracle build that honours the source.
   **No equivalence claims without oracle numbers.** Thresholds are in PLAN.md §11.3; variability bands come from repeated oracle runs.
9. **Git: no AI attribution.** This rule is the user's instruction and overrides any tool default.
   - Never add `Co-Authored-By: Claude …` (or any AI co-author) trailers to commits.
   - Never add "Generated with Claude Code" or similar lines to commit messages, PR descriptions, issues or release notes.
   - Commits are authored by the repo's configured git identity only. This repo uses the user's GitHub no-reply address (`git config user.email`), so their personal email is not exposed in this public repo.
   - **Enforced** (user, 2026-10-27: "make sure that the new computer don't include Claude as coauthor"): Claude Code's attribution is off in `~/.claude/settings.json` on every machine (`"attribution": {"commit": "", "pr": "", "sessionUrl": false}`), and a `commit-msg` guard in each repository refuses any AI co-author or "Generated with" line (`scripts/orchestration/git-hooks/`, installed by `install.sh` into the three repositories and any new clone; it covers worktrees). Never bypass it (`--no-verify`). The no-reply identity is set in each repository's own `.git/config` (the global one is personal).
10. **Cross-platform golden tests, and a small CI budget** (user decision, 2026-10-10).
   - **Two kinds of checks.** *Parity* compares larmorx with the original tools; it runs only on the Linux x64 machine, where the originals exist. *Golden tests* check that the other five platforms (Linux aarch64, macOS Intel and Apple silicon, Windows x64 and arm64) reproduce larmorx's own Linux x64 output. Most originals do not exist on Windows or macOS, so comparing with them there is not the goal. Golden tests cover replicas and clean-room originals alike.
   - **Every function has at least one golden case:** every CLI tool, ImageMath op and public Python function. A case is an input recipe generated by the test itself, the arguments, and the SHA-256 of the output data.
     - Inputs use seeded random numbers and plain arithmetic only: no `sin`, `exp` or other transcendental functions, which differ between platforms. So inputs are identical everywhere, and no image data is committed.
     - One exception (user, 2026-10-16): tiny synthetic **format fixtures** that CI cannot generate (e.g. HDF5 files using storage features only the HDF5 C library writes) may be committed: a few KB, no image data, made by a committed deterministic script, documented in `docs/validation/golden.md`.
     - The checksums are recorded on Linux x64, by a script, from a commit whose parity passed.
     - A test fails when a registered tool has no golden case.
   - **Bit-identity is the rule.** A tolerance needs a documented reason in `docs/findings/`, and preferably a fix instead.
   - **Keep CI small; GitHub Actions time is limited.** Be careful and precise:
     - golden inputs at most 32³ voxels (4 volumes for 4D); one case per distinct code path, not one per parameter value;
     - the whole golden suite runs in under a minute per platform, on the release wheel the test job already builds; once per platform (one Python version), inside the existing jobs; no new jobs or runners;
     - parity, benchmarks and real data never run in CI;
     - docs-only changes should not trigger CI (`paths-ignore`);
     - before adding any CI work, estimate its minutes times the number of jobs (13 today).

## Agent working rules
Every agent reads this file first, then `docs/porting-guide.md`, its section of `docs/next-steps.md`, `docs/architecture.md` and the records and findings its task builds on. The orchestrator's brief adds only the task; these rules hold for every task.
- **Roles.** A replica author may read the upstream source; a clean-room implementer may not (rule 1; `docs/porting-guide.md` §1 says who may read what). An agent that has read upstream source (replica or validation work) never edits a clean-room crate (`crates/larmorx-afni`, `crates/larmorx-mri`, ...); it edits `specs/` only when its brief names it the **spec writer** (porting guide §1: behaviour, maths, conventions, command line and validation plan, never code or routines). Otherwise it describes the change in its report. The clean-room original is then written by a different agent that has never read the source.
- **Shared documents through a handoff** (user, 2026-10-23): an agent edits only its own files (its code, its tool's API pages, findings files, validation records, `PROVENANCE.md`, evidence module, and in the site clone its tool's pages and evidence data). It does not edit the shared documents: `README.md`, `docs/findings/upstream-defects.md`, `docs/api/upstream-defects.md`, `docs/validation/README.md`, `docs/validation/golden.md`, `docs/licensing.md`, `docs/next-steps.md`, `docs/decision-log.md`, this file, and in the site `data/status.yml`, `site.yml`, `content/validation.md`, `static/evidence/summary.json`. It writes `<scratch>/HANDOFF.md` with the exact text for each, which the orchestrator integrates and harmonizes at the merge. Code that must change for the tool to work (the CLI program registry, family `__init__.py`, `_core.pyi`, the golden registry and checksums, `larmorx_core::defects`) changes in the branch, minimally. Each agent uses only the defect-ID range its brief gives it.
- **Every part, in full** (user, 2026-10-22, standing): an excellent replica, and a clean-room original where the licence requires one; very comprehensive tests; very good data examples that cover every use case (real data from the oracle runs and OpenNeuro, templates, synthetic edge cases), for the best comparisons; important findings documented (`docs/findings/`, the register); and when the work is done, **both repositories' documentation updated**: this repository's (API page, records, findings, PROVENANCE, README) and `larmorx_documentation`'s, where every ported program's page is interactive evidence in the approved form (raw inputs, the original oracles' outputs, larmorx's outputs from replica and clean-room original, comparison metrics and views showing how they differ, why, and whether the difference is important, expected or can still be improved; the site's style; accurate, high-quality, elegant media whose every shot carries meaningful information). `docs/figures.md` says how. **Testing depth** (user, 2026-10-28): the tests are smart, not repeated: every code path once (each option, image and data type, odd header and size), fMRIPrep's real calls on about three distinct oracle runs, the edge cases of the defects found, a short random-testing trial, the standard and smoke tiers; the site page is published from those tests (figures and metrics), and the final validation phase adds the extensive evidence to it (`docs/next-steps.md`). Fixes and version updates are not postponed: they use the same testing, aligned with the newest originals.
- **Scope.** Full completeness: every option, message, exit status and failure case of the original, with D13 switches for its defects; every defect found goes into `docs/findings/` and the register; the per-tool contract (rule 7), figures included (`docs/figures.md`; the site's pages are in a clone of `larmorx_documentation` in the agent's scratch directory, on a branch the orchestrator fetches; set the clone's git identity to the no-reply address first, as the global one is personal).
- **Environment.** `export PATH=$HOME/.cargo/bin:$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)` (FSL off the path), `export PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, `export OPENBLAS_CORETYPE=Haswell` (numpy's reference kernels on the main machine's AVX-512 CPU, `docs/findings/numpy-blas.md`; `scripts/orchestration/env.sh` sets all three). **Never run `maturin develop`** (it repoints the shared venv at the agent's worktree): `maturin build --release -o <scratch>/wheel`, copy the wheel's `larmorx/_core.abi3.so` into the worktree's `python/larmorx/` (git-ignored), and run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`. Copying the build-cache seed's `target/` (see "Where things are") saves the first build.
- **Gates before the final commits:** the three Rust workspaces (root, `crates-gpl`, `crates-nc`; refresh their `Cargo.lock` when dependencies change; `scripts/orchestration/gates-rust.sh <checkout> all`): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`; `ruff check`, `ruff format --check`, `python scripts/check_spdx.py`, `pytest tests/python tests/golden validation/tests`, `pytest tests/parity`.
- **Records only from a clean commit** (a record stamped `-dirty` is redone). Golden checksums with `scripts/record_golden.py` from a clean commit (it runs the parity smoke tier first, about 10 minutes, unless that tier passed for the same clean commit and build: then it reuses its marker). Validation against the oracles named here only (the ANTs oracle is the GCC 14.3 build).
- **The oracle-output cache** ([docs/validation/oracle-cache.md](docs/validation/oracle-cache.md)): new suites run original programs through `oracle_cache.run` (never `subprocess.run`), with a `Policy` naming the thread count, the family of environment variables and, from the case, why a run must never be cached (`case_never`: a clock seed, memory never written, a defect of `VARYING_DEFECTS`, a case that measures the original's own spread). A difference between a fresh run and its entry is a finding (`<cache>/findings.jsonl`), never overwritten. Suites keep derived inputs outside the cache's directory (`oracles/inputs/<suite>/`).
- **Random testing** ([docs/validation/difftest.md](docs/validation/difftest.md)): a new tool gets a difftest descriptor (`validation/src/larmorx_validation/difftest/tools/`, one short module: its option grammar, input generators, implementations, exact checks) in the same part, and a short trial night (`difftest --tools <tool> --budget 0.5`, boxed as every night) before its gates; every difference is triaged by the user's rule (a larmorx mistake fixed with a permanent case, a defect of the original registered by rules A-D, a clean-room original's reported with its reproducer) into `validation/difftest/triage.toml`. Tables that repeat facts are generated: edit `validation/tools.toml` and run `python -m larmorx_validation tables`, never the generated blocks.
- **No permission prompts.** Temporary clones, copies and build outputs go in the agent's scratch directory (`/tmp/claude-1000/.../scratchpad/<task>/`), never under a `.claude/` folder: writes there raise a permission prompt the user has to answer, which stalls the run (2026-10-21: a denied copy into `<worktree>/.claude/s/`). Prefer approaches that need no approval; report any that does.
- **Portable tests.** No test may assume a temporary path free of `~` or blanks (GitHub's Windows runners: `C:\Users\RUNNER~1\...`). Helper scripts kept beside others run with `python -P` (a script named like a standard module, e.g. `types.py`, shadows it otherwise).
- **Commits.** Early and often (WIP checkpoints are fine), with the configured git identity and no AI attribution (rule 9). Agents do not push, do not merge into `main` and do not edit this file; the orchestrator does.
- **Keep every tool that produced a finding or a record** (user, 2026-10-23: helpers belong in the repository, for reproducibility on other machines): commit it under `validation/investigations/<topic>/` (with a README: what it shows, how to run it) or `validation/probes/`, never only in the scratch directory. The orchestrator's own helpers are in `scripts/orchestration/`; use them instead of rewriting them.
- **Usage limits and long runs.** Keep `<scratch>/NOTES.md` current (done, running, next) so an interrupted run can be resumed. Long jobs run in the background with logs in `oracles/logs/`; wait with one background wait on the log's final line, not repeated short checks, and with wait loops that cannot match themselves. When a long run ends, act on it and finish: never stop silently. Stop at the end of the briefed part and report concisely (what was built, what is covered and not, the numbers, findings, gates, commits).

## Facts worth knowing (from docs/analysis/)
- fMRIPrep's heaviest compute, 4D BOLD resampling, is Python: `fmriprep/fmriprep/interfaces/resampling.py`.
- fMRIPrep/ANTs registration settings are in `niworkflows/niworkflows/data/*.json` and `sdcflows/sdcflows/data/*.json`. They are regression tests for `lx.ants.registration`.
- **ANTsPy as an in-process oracle:**
  - `ants.internal.get_lib_fn('antsRegistration')(args)` runs raw antsRegistration arguments.
  - Wheels exist for linux-x64, macOS and win-x64, Python 3.8–3.13.
  - `ants.motion_correction` is a slow per-volume Python loop (one reason `lx.mri.hmc` exists).
- fMRIPrep Docker images are linux/amd64. Generate fMRIPrep and FreeSurfer oracles **on this machine**.
- Sizes:
  - ANTs ≈ 160k lines; the ITK modules it relies on ≈ 200k
  - FreeSurfer's recon-all path ≈ 200–250k lines (surface library 82k, GCA + GCA morph 44k)
- Upstream fMRIPrep CI only compares output file *lists*. Our numeric oracle harness is new work.

## Working on this machine
- Run long jobs (fMRIPrep and FreeSurfer oracles, benchmarks) in the background with `tmux` or `nohup`.
  - Log to `oracles/logs/`.
  - Record the exact command, image tag/version, seeds and thread counts next to the outputs.
- **Every heavy job runs in a memory box** (2026-10-25): `scripts/orchestration/boxed.sh <GiB> <command>` (a systemd user scope, hard memory limit, no swap), for parity tiers, random testing, fMRIPrep oracle runs, benchmarks, `record_golden.py` and `verify-merge.sh`; a harness that runs original programs on generated inputs also caps each process. On 2026-10-25 one random N4BiasFieldCorrection case grew to 28 GB; the machine swapped until VS Code's terminal host stopped answering, and the editor and every agent in it went down. Boxes together stay under about 100 GiB on the main machine (124 GiB, 199 GiB of swap; a box has no swap), under about 24 GiB on the old laptop (31 GiB).
- Never commit image data or oracle outputs to git. Use content-addressed storage (PLAN.md §11.1).
- The FreeSurfer licence file (`/usr/local/freesurfer/license.txt`) is only mounted read-only into containers; never copied or committed.
- **Machines** (user, 2026-10-25; moved 2026-10-27 and 28): the main machine is the new laptop (`karelo-Alienware-m18-R1-AMD`: AMD, 16 cores / 32 threads, 124 GiB, AVX-512, RTX 4090, Ubuntu 22.04); every agent and every record runs there. The old laptop (`karelo-GE75-Raider-8SF`: Intel i7-8750H, 12 threads, 31 GiB, AVX2) is the helper `old-laptop`: the quiet benchmark machine and a second Linux reference, reached by `ssh old-laptop` and the git remote `old-laptop` (`docs/helper-machines.md`); no development starts there. A Windows laptop serves Windows checks (`win-laptop`); the lab's server (1 TB RAM, shared, Ubuntu 24.04: originals in an Ubuntu 22.04 container) only giant jobs. Helpers are reached by key-based SSH from the main machine, never with passwords; macOS stays with CI.
- **Run one agent at a time** (user, 2026-10-10; a two-agent trial on 2026-10-11 was stopped the same evening to stay within usage limits). Four parallel agents once exhausted the session limit and were killed mid-task. Brief every agent to commit as soon as its work passes the gates; when pausing one, ask for a WIP checkpoint commit and a NOTES file first.
- **The ANTs oracle is built with conda-forge GCC 14.3** (`oracles/toolchains/gcc-14.3`, user-level). Ubuntu 22.04's GCC 11.4 and 12.3 miscompile float roundings at `-O3` (GCC bug 110182); `scripts/build_ants_oracle.sh` probes for it and refuses such a compiler. The old GCC 11 build is kept as evidence in `oracles/ants-2.6.5-gcc11-O3`; validate registration (A4/A5) against the new build only.
- **Wait loops must not match themselves.** `until ! pgrep -f "larmorx_validation bench"` also matches its own command line, so it never ends (four such loops ran for a day, 2026-10-11). Use a bracket: `pgrep -f "[l]armorx_validation bench"`.
- **Never delete work products** (code, drafts, ports, harnesses, reports). If something must leave the repo or the build, archive it in `<workspace>/archive/<date>-<topic>/` (not under `/tmp`) and tell the user. Ask before deleting anything that is not a temporary build artefact.
- Verify the environment at session start: `uname -m`, `nproc`, free RAM, `apptainer --version`, `rustc --version`. Docker is not installed; Apptainer replaces it.
- **Merging an agent's work: the checklist** (the user: "Make sure everything works before merging"). Nothing reaches `main` or GitHub before steps 1-5 pass on the merged tree in the main checkout:
  The steps as scripts: `scripts/orchestration/` (README: one command per step). Each part's brief, report and handoff go into `docs/parts/` at its merge.
  0. Start from a clean tree; chain the merge with `&&` and check `.git/MERGE_HEAD`. On 2026-10-11 an uncommitted file made `git merge` abort, and the checks then ran on the old tree.
  1. The three Rust workspaces: `cargo fmt --check`, `clippy -D warnings`, `cargo test --locked`, in the root, `crates-gpl` and `crates-nc`.
  2. `maturin develop --release`, then `pytest tests/python tests/golden validation/tests`.
  3. `ruff check`, `ruff format --check`, `scripts/check_spdx.py`.
  4. `pytest tests/parity` (the smoke tier against the real oracles).
  5. Re-run the new or changed tool's standard-tier parity yourself, from the committed merge (a clean tree, so the record's commit is clean), and compare with the agent's record. The re-run serves the agent's runs of the originals from the oracle-output cache (the record's header says how many); `--oracle-cache refresh` re-runs and compares them.
  6. **Integrate and harmonize the documents** (user, 2026-10-23: "I just want the implementations and docs correct after the agents finished their job"): put each agent's `HANDOFF.md` into the shared documents; re-read every shared document the merge touched so it tells one consistent story (the defect register's table, history paragraph and counts, `docs/api/upstream-defects.md`, README, the validation index, next-steps, "Where things are" here, the site's status, navigation and Validation page); recount the register's **decide** entries and list every one under "Waiting for the user". After merging two parallel branches, re-record the golden checksums from the merge (`scripts/record_golden.py`; the values must not change) and re-export the site's summary.
  7. Commit, check for AI attribution, push, then watch CI on all six platforms.
  8. Once CI is green, delete that agent worktree's Rust build caches (`target/`, `crates-gpl/target/`; regenerable), keeping only the newest worktree's cache as a seed for the next agent (user, 2026-10-19: 30 merged worktrees had grown to 190 GB). Never delete a worktree's source or branch.
- **Reading CI results without a login:** GitHub serves job logs only to logged-in users. Golden mismatches, failing `cargo test` tests and build errors (`scripts/ci_cargo_failures.py`, since 2026-10-12) are printed as workflow annotations, which `https://api.github.com/repos/karellopez/larmorx/check-runs/<job id>/annotations` serves without a login. Each job's step results: `https://api.github.com/repos/karellopez/larmorx/actions/jobs/<job id>`. GitHub's Windows runners give the temporary directory as an 8.3 short path (`C:\Users\RUNNER~1\...`): tests must not assume a temporary path free of `~` or blanks.
- **Detached jobs (`nohup ... &`) send no notice when they end.** Watch their log for a final line in a tracked background command.
- **Launch worktree agents from inside this repository** (`cd /home/karelo/VScode_projects/super_fmriprep/larmorx` right before the Agent call): the harness creates the worktree in the repository of the current directory. On 2026-10-20 A5 part 2 was launched from `larmorx_documentation` and its worktree landed there.
- **Oracle builds:** `scripts/build_ants_oracle.sh` (GCC 14.3; `oracles/ants-2.6.5`, with `antsAI`, `antsAffineInitializer`, `Atropos`, ...), `scripts/build_afni_oracle.sh` (`oracles/afni-25.2.09`), `scripts/build_crmath_preload.sh` (`oracles/core-math-preload`, correctly rounded `exp`, `log`, `sin`, `cos`, `pow`, `asin`, `acos`, `atan2`, `cbrt` and their float versions; older preloads archived).

## Repo map
| Path | Contents |
|---|---|
| `PLAN.md` | the plan |
| `CLAUDE.md` | this file |
| `docs/decision-log.md` | direction changes and open decisions |
| `docs/status-log.md` | the step-by-step status until 2026-10-20 (moved from this file) |
| `docs/analysis/01–04` | what fMRIPrep computes (stages, tools, parameters, file:line) |
| `upstream.tsv` | pinned upstream commits |
| `scripts/bootstrap_workspace.sh` | recreates the workspace |
| `docs/overview.md`, `docs/licensing.md` | the concepts in plain terms; the licence tracks and what is published |
| `docs/porting-guide.md` | how to port a tool: tracks and who may read what, the source as the specification, the original's own variability, oracle coverage, the pipeline's real parameters, separating causes, validation records, D11/D13, determinism, working safely, the per-tool checklist |
| `Cargo.toml`, `crates/` | Rust workspace: `larmorx-core`, `-io`, `-hdf5`, `-transform`, `-interp`, `-image`, `-registration`, `-ants`, `-afni` (clean-room), `-mri` (clean-room), `-cli` (standalone `larmorx`/`lx`), `-py` (`larmorx._core`) |
| `crates-gpl/` | the GPL-3.0-or-later replica workspace (`docs/licensing.md`); it builds the Apache crates by path, so refresh `crates-gpl/Cargo.lock` whenever their dependencies change (CI checks it with `--locked`) |
| `crates-nc/` | the FSL replica workspace (`larmorx-nc`, FSL Licence; `larmorx-nc-core` the shared FSL core; `fsl-source/`, FSL's original source; `scripts/fsl_source.py` copies and checks it); it builds the Apache crates by path, never `crates-gpl` |
| `specs/` | behaviour specs, the input of clean-room originals |
| `docs/figures.md` | the figures: the command, the cache, provenance, style, data rules, how to add a tool |
| `docs/next-steps.md` | the ordered plan of the next tasks, their scopes and validation, and the decisions waiting for the user |
| `docs/upstream-reports/` | drafts of bug reports for the upstream projects (marked "draft, not yet filed" until the user files them) |
| `pyproject.toml`, `python/larmorx/` | maturin package (abi3, CPython >= 3.12), including `pipelines/larmorprepx/` |
| `tests/python/`, `tests/golden/`, `tests/parity/` | Python tests of the installed package; the cross-platform golden tests (and their HDF5 fixtures); the parity suites under pytest |
| `.github/workflows/ci.yml` | lint + wheel build and tests on the six targets |
| `validation/` | parity suites and benchmarks (dev-only package `larmorx-validation`) |
| `validation/src/larmorx_validation/oracle_cache/`, `validation/tests/` | the oracle-output cache (key, store, policy, replays, `verify`) and the validation package's own tests ([docs/validation/oracle-cache.md](docs/validation/oracle-cache.md)) |
| `validation/src/larmorx_validation/difftest/`, `validation/difftest/` | differential random testing: the harness and one descriptor per tool; the permanent cases and `triage.toml` ([docs/validation/difftest.md](docs/validation/difftest.md)) |
| `validation/tools.toml`, `validation/src/larmorx_validation/tables.py` | one description per tool, from which the README's tool table, the validation index and the site's status table are generated (`python -m larmorx_validation tables`) |
| `docs/architecture.md`, `docs/api/`, `docs/validation/`, `docs/benchmarks/`, `docs/findings/` | conventions, user docs per tool, validation records, benchmark reports, findings about the upstream tools |
| `docs/findings/upstream-defects.md` | the register of every defect found in the originals (class, evidence, fMRIPrep impact, what larmorx does, D13 gap), with the list of defects larmorx still reproduces by default |
| `../larmorx-testdata/` (github.com/karellopez/larmorx-testdata) | test-data catalog, generated edge cases, hash-verified downloader (`pip install -e`) |
| `scripts/orchestration/` | the merge checklist as scripts: gates, `verify-merge.sh`, record comparison, golden merge, register recount, handoff blocks, CI and Pages watchers, oracle records (its README) |
| `docs/parts/` | every part's brief, final report and handoff, verbatim |
| `docs/validation/oracles.md`, `oracle-runs.md` | how the oracles were built (BUILD_INFO, program checksums) and how the fMRIPrep runs were made (RUN_INFO) |
| `validation/investigations/`, `validation/probes/` | the tools behind findings |
| `validation/requirements-lock.txt`, `rust-toolchain.toml` | the exact Python packages and Rust compiler the records were made with |
| `../larmorx_documentation/` (separate repo) | the documentation site: builder, design, the landing page, per-tool pages and figures (its own CLAUDE.md) |