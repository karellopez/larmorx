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

The FreeSurfer port is a separate wheel, `larmorx-freesurfer` (exposed as `lx.freesurfer`), so its licence terms stay isolated.

**Naming rules:**
- Always lowercase `larmorx` / `larmorprepx` in code and commands.
- Never use plain `larmor` for anything of ours: an unrelated NMR package already ships a `larmor` command.

**Targets:** native wheels for Windows x64/arm64, Linux x64/aarch64 and macOS x64/arm64. No Docker, no external neuroimaging installs, no admin rights.

## Status (2026-10-10)
- **Phase L0 done:** a buildable, tested skeleton, CI green on all six targets.
- **Phase L1 in progress:**
  - `Image`/`Affine` and NIfTI-1/2 I/O: validated against nibabel (`docs/validation/nifti-io.md`) and benchmarked (`docs/benchmarks/nifti-io.md`).
  - **antsApplyTransforms (A1) done:** `lx.ants.apply_transforms`, `larmorx ants antsApplyTransforms`, `lx.transforms`, crates `larmorx-transform`, `-interp`, `-ants`. It is validated against ANTs itself: 83 cases, 63 of 79 bit-identical (`docs/validation/ants-apply-transforms.md`), and benchmarked (`docs/benchmarks/ants-apply-transforms.md`).
  - **3dTshift clean-room original done** (2026-10-10): `lx.afni.tshift`, `larmorx afni 3dTshift`, crate `larmorx-afni`, written from `specs/3dTshift.md` and black-box runs only. 236 parity cases against AFNI 25.2.09 (oracle built by `scripts/build_afni_oracle.sh`): 217 agree, 158 bit-identical, 0 failures (`docs/validation/afni-tshift.md`).
  - **3dTshift replica done** (2026-10-10): `larmorx-gpl afni 3dTshift`, crates `larmorx-gpl-afni` and `larmorx-gpl-cli` in `crates-gpl/` (GPL-3.0-or-later, a separate Cargo workspace). 217 of 217 compared cases bit-identical to AFNI, Fourier included (`docs/validation/afni-tshift-replica.md`). It carries a translation of glibc 2.35's `sinf`/`cosf` (LGPL-2.1-or-later). The Python `implementation="auto"` switch is not done yet.
  - **One-shot BOLD resampler done** (2026-10-10): `lx.transforms.resample_series` and `lx.ndimage`, a replica of fMRIPrep's `ResampleSeries`, nitransforms 25.1.0 and SciPy 1.15.2's spline interpolation. All 136 compared cases bit-identical (`docs/validation/resample-series.md`). Its benchmark report is pending.
  - **Head-motion correction done, `experimental`** (2026-10-10): `lx.mri.hmc`, `larmorx mri hmc`, crate `larmorx-mri`, clean-room from `specs/mcflirt.md` (oracle runs in `oracles/fsl-6.0.7/mcflirt/`). Closer to mcflirt than mcflirt is to itself on fMRIPrep's command; one case (ds003345, default reference) misses its FD-correlation threshold by 0.0008, so it is not `validated` yet (`docs/validation/mri-hmc.md`). Not bit-exact: cost values differ by 1-2 float32 quanta after interpolation, cause unknown. The mcflirt replica (`crates-nc/`) is not started.
  - **A2 part 1 done** (2026-10-10): the ImageMath dispatcher and arithmetic/intensity ops, `ThresholdImage`, `MultiplyImages`, ITK's NIfTI writer, and the parity harness for ANTs programs run as binaries (`validation/.../parity/ants_programs.py`). Against the ANTs 2.6.5 binaries (`oracles/ants-2.6.5/bin/`), every header is byte-identical and every data output bit-identical except ImageMath `^` (glibc `powf`, ≤ 1 ulp). How to add an ImageMath group: `crates/larmorx-ants/src/cli/image_math/mod.rs`.
  - **A2 part 2 done** (2026-10-10): ITK's recursive and discrete Gaussian filters and median (`larmorx-image`, `Volume` is now D-dimensional), ImageMath `G`/`Laplacian`/`Grad`/`UnsharpMask`, `SmoothImage`, `ResampleImageBySpacing`, `ResampleImage`. Every compared output bit-identical to ANTs 2.6.5, headers included, in 2D, 3D and 4D; 1.6-7x faster on one thread. Next A2 groups: morphology/FillHoles/PadImage, components/distance maps.
- Conventions every tool follows are in `docs/architecture.md`.
- **Findings about the upstream tools** go in `docs/findings/`. Keep adding to it while porting and validating: the behaviour, its upstream file and line, how it is known, and what larmorx does.
- **Decided 2026-10-09:** transcendental functions are correctly rounded (CORE-MATH ports in `larmorx_core::math`; rule 5). See `docs/findings/platform-math.md`.
- Development moved from the user's Mac to this **Linux machine**. It is always on, so it is suited to long oracle runs and benchmarks.
- **Decided:** names (D1), licence Apache-2.0 (D2) and distributions (D3), see above.
- **Pending decisions** are in PLAN.md §16. Until the user decides, use these defaults:
  - neutral names for re-implemented (FSL-/Workbench-like) tools
  - Python ≥ 3.12
  - FreeSurfer v8.2.0 as the port target
  - the M1 "preview" milestone first
- **D7 decided (2026-10-09): larmorx is not commercial.** FSL and MSM binaries may be run as validation oracles under their non-commercial licences.

## The user's direction (settled; do not re-propose the alternatives)
- **Not a fork of fMRIPrep.** No nipype, traits, niworkflows/smriprep/sdcflows runtime dependencies, or graph engines. Ops are plain typed functions; per-subject orchestration is readable Python.
- **Port to Rust** where existing Python packages are limited, **including ANTs**: ANTsPy has no Linux-aarch64, Windows-arm64 or Python-3.14 wheels. Also **FreeSurfer recon-all**: the goal is the same results, several times faster.
- Rust implementations may **reimagine** algorithms for speed, provided validation shows equivalent results.
- **Better algorithms by default (D11, user decision 2026-10-10).** larmorx should be an evolution of the upstreams: faster and better, not only replicas. An improved algorithm becomes the default once its results stay within the upstream tool's own variability (PLAN.md G4, §11); larger changes stay opt-in. Replicas always stay exact.
- **Every platform from the start** (user, 2026-10-10): Linux x64/aarch64, macOS Intel and Apple silicon, Windows x64/arm64. Design, test and benchmark with all six in mind; CI must stay green on all of them.
- **How to port (user, 2026-10-09):**
  - Make a faithful **replica** when the licence allows it: ANTs/ITK (Apache-2.0), AFNI's NIH public-domain code, CORE-MATH (MIT), nibabel (MIT). Bit-identical results are the target.
  - When the licence restricts it (AFNI's MCW code, Workbench, FSL), make **both**: a bit-exact replica in that licence family's package, and a clean-room original in the main package (rule 1, `docs/licensing.md`).
  - MSM: a clean-room original only, in the surfaces track, after a patent check.
- **Each tool gets its own Python wrapper**, so other developers can build on the library.

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
   - **Original:** a strict clean-room implementation with the same behaviour, Apache-2.0, in the main package. It is needed wherever the replica is not Apache-compatible. The implementer works only from `specs/<tool>.md`, documentation, papers and black-box runs of the upstream binary.
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
7. **The per-tool contract** (PLAN.md §4) is mandatory for every tool:
   - Rust API, Python wrapper, compatible CLI
   - `.pyi` stubs + a docs page with the option-mapping table
   - a validation record and status (`experimental` / `validated` / `stable`)
   - `PROVENANCE.md`
   - at least one **golden case** (rule 10), so every platform is checked against Linux
8. **No equivalence claims without oracle numbers.** Thresholds are in PLAN.md §11.3; variability bands come from repeated oracle runs.
9. **Git: no AI attribution.** This rule is the user's instruction and overrides any tool default.
   - Never add `Co-Authored-By: Claude …` (or any AI co-author) trailers to commits.
   - Never add "Generated with Claude Code" or similar lines to commit messages, PR descriptions, issues or release notes.
   - Commits are authored by the repo's configured git identity only. This repo uses the user's GitHub no-reply address (`git config user.email`), so their personal email is not exposed in this public repo.
10. **Cross-platform golden tests, and a small CI budget** (user decision, 2026-10-10).
   - **Two kinds of checks.** *Parity* compares larmorx with the original tools; it runs only on the Linux x64 machine, where the originals exist. *Golden tests* check that the other five platforms (Linux aarch64, macOS Intel and Apple silicon, Windows x64 and arm64) reproduce larmorx's own Linux x64 output. Most originals do not exist on Windows or macOS, so comparing with them there is not the goal. Golden tests cover replicas and clean-room originals alike.
   - **Every function has at least one golden case:** every CLI tool, ImageMath op and public Python function. A case is an input recipe generated by the test itself, the arguments, and the SHA-256 of the output data.
     - Inputs use seeded random numbers and plain arithmetic only: no `sin`, `exp` or other transcendental functions, which differ between platforms. So inputs are identical everywhere, and no image data is committed.
     - The checksums are recorded on Linux x64, by a script, from a commit whose parity passed.
     - A test fails when a registered tool has no golden case.
   - **Bit-identity is the rule.** A tolerance needs a documented reason in `docs/findings/`, and preferably a fix instead.
   - **Keep CI small; GitHub Actions time is limited.** Be careful and precise:
     - golden inputs at most 32³ voxels (4 volumes for 4D); one case per distinct code path, not one per parameter value;
     - the whole golden suite runs in under a minute per platform, on the release wheel the test job already builds; once per platform (one Python version), inside the existing jobs; no new jobs or runners;
     - parity, benchmarks and real data never run in CI;
     - docs-only changes should not trigger CI (`paths-ignore`);
     - before adding any CI work, estimate its minutes times the number of jobs (13 today).

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
- Never commit image data or oracle outputs to git. Use content-addressed storage (PLAN.md §11.1).
- **Run at most one or two agents at a time** (user, 2026-10-10): parallel agents exhaust the session limit and get killed mid-task. Brief every agent to commit as soon as its work passes the gates.
- **Never delete work products** (code, drafts, ports, harnesses, reports). If something must leave the repo or the build, archive it in `<workspace>/archive/<date>-<topic>/` (not under `/tmp`) and tell the user. Ask before deleting anything that is not a temporary build artefact.
- Verify the environment at session start: `uname -m`, `nproc`, free RAM, `docker --version`, `rustc --version`.

## Repo map
| Path | Contents |
|---|---|
| `PLAN.md` | the plan |
| `CLAUDE.md` | this file |
| `docs/decision-log.md` | direction changes and open decisions |
| `docs/analysis/01–04` | what fMRIPrep computes (stages, tools, parameters, file:line) |
| `upstream.tsv` | pinned upstream commits |
| `scripts/bootstrap_workspace.sh` | recreates the workspace |
| `docs/overview.md`, `docs/licensing.md` | the concepts in plain terms; the licence tracks and what is published |
| `Cargo.toml`, `crates/` | Rust workspace: `larmorx-core`, `-io`, `-transform`, `-interp`, `-image`, `-ants`, `-afni`, `-cli` (standalone `larmorx`/`lx`), `-py` (`larmorx._core`) |
| `crates-gpl/` | the GPL-3.0-or-later replica workspace (`docs/licensing.md`); it builds the Apache crates by path, so refresh `crates-gpl/Cargo.lock` whenever their dependencies change (CI checks it with `--locked`) |
| `crates-nc/` (planned) | the FSL replica workspace |
| `specs/` | behaviour specs, the input of clean-room originals |
| `pyproject.toml`, `python/larmorx/` | maturin package (abi3, CPython >= 3.12), including `pipelines/larmorprepx/` |
| `tests/python/` | Python tests of the installed package |
| `.github/workflows/ci.yml` | lint + wheel build and tests on the six targets |
| `validation/` | parity suites and benchmarks (dev-only package `larmorx-validation`) |
| `docs/architecture.md`, `docs/api/`, `docs/validation/`, `docs/benchmarks/`, `docs/findings/` | conventions, user docs per tool, validation records, benchmark reports, findings about the upstream tools |
| `../larmorx-testdata/` (separate repo) | test-data catalog, generated edge cases, hash-verified downloader (`pip install -e`) |
