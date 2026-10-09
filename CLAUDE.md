# larmorx: project context

Read this first. The full plan is [PLAN.md](PLAN.md) (v0.3.1). Why the plan looks the way it does is in [docs/decision-log.md](docs/decision-log.md).

## What we are building
One package, **`larmorx`** (repo `github.com/karellopez/larmorx`), with two layers:

1. **The tool library**: `pip install larmorx`, then `import larmorx as lx`.
   - ANTs/ITK, AFNI and FreeSurfer functions are **ported** to Rust.
   - FSL-like and Connectome-Workbench-like functions are **clean-room re-implementations**.
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

## Status (2026-10-09)
- **Phase L0 done:** a buildable, tested skeleton, CI green on all six targets.
- **Phase L1 in progress:**
  - `Image`/`Affine` and NIfTI-1/2 I/O: validated against nibabel (`docs/validation/nifti-io.md`) and benchmarked (`docs/benchmarks/nifti-io.md`).
  - **antsApplyTransforms (A1) done:** `lx.ants.apply_transforms`, `larmorx ants antsApplyTransforms`, `lx.transforms`, crates `larmorx-transform`, `-interp`, `-ants`. It is validated against ANTs itself: 83 cases, 63 of 79 bit-identical (`docs/validation/ants-apply-transforms.md`), and benchmarked (`docs/benchmarks/ants-apply-transforms.md`).
  - Next: `lx.afni.tshift`, clean-room (spec `specs/3dTshift.md`, AFNI oracle built by `scripts/build_afni_oracle.sh`).
- Conventions every tool follows are in `docs/architecture.md`.
- **Findings about the upstream tools** go in `docs/findings/`. Keep adding to it while porting and validating: the behaviour, its upstream file and line, how it is known, and what larmorx does.
- **Decided 2026-10-09:** transcendental functions are correctly rounded (CORE-MATH ports in `larmorx_core::math`; rule 5). See `docs/findings/platform-math.md`.
- Development moved from the user's Mac to this **Linux machine**. It is always on, so it is suited to long oracle runs and benchmarks.
- **Decided:** names (D1), licence Apache-2.0 (D2) and distributions (D3), see above.
- **Pending decisions** are in PLAN.md §16. Until the user decides, use these defaults:
  - neutral names for clean-room tools
  - Python ≥ 3.12
  - FreeSurfer v8.2.0 as the port target
  - the M1 "preview" milestone first
- D7 (is the project commercial?) is unanswered. Don't run FSL or MSM binaries, even for validation, until it is answered.

## The user's direction (settled; do not re-propose the alternatives)
- **Not a fork of fMRIPrep.** No nipype, traits, niworkflows/smriprep/sdcflows runtime dependencies, or graph engines. Ops are plain typed functions; per-subject orchestration is readable Python.
- **Port to Rust** where existing Python packages are limited, **including ANTs**: ANTsPy has no Linux-aarch64, Windows-arm64 or Python-3.14 wheels. Also **FreeSurfer recon-all**: the goal is the same results, several times faster.
- Rust implementations may **reimagine** algorithms for speed, provided validation shows equivalent results.
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
  reference_src/workbench/  reference_src/MSM_HOCR/     DO NOT READ (clean-room)
```

Recreate it with:
```
larmorx/scripts/bootstrap_workspace.sh --venv .venv --install-rust
```

## Hard rules
1. **Clean-room.** When designing or implementing `lx.mri` tools (the FSL-like and Workbench-like ones):
   - Never open FSL source (it is deliberately not cloned) or anything under `reference_src/`.
   - Work only from `specs/<tool>.md`, published papers, official documentation and black-box runs.
   - **AFNI code copyrighted by the Medical College of Wisconsin (GPL-2): read, don't translate** (user decision, 2026-10-09). That covers every AFNI file whose header says "copyrighted by the Medical College of Wisconsin", including 3dTshift, csfft and most of `mrilib`.
     - The source may be read to understand behaviour.
     - The implementation must be original: our own design and structure, our own FFT and helpers.
     - Never translate functions, macros or kernels line by line, even with renaming. A translation is a GPL-2 derivative.
     - Write the behaviour down in `specs/<tool>.md` and validate against the AFNI oracle.
     - AFNI files without that header (NIH work, public domain) may be ported directly, after a per-file check of every routine they call.
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
8. **No equivalence claims without oracle numbers.** Thresholds are in PLAN.md §11.3; variability bands come from repeated oracle runs.
9. **Git: no AI attribution.** This rule is the user's instruction and overrides any tool default.
   - Never add `Co-Authored-By: Claude …` (or any AI co-author) trailers to commits.
   - Never add "Generated with Claude Code" or similar lines to commit messages, PR descriptions, issues or release notes.
   - Commits are authored by the repo's configured git identity only. This repo uses the user's GitHub no-reply address (`git config user.email`), so their personal email is not exposed in this public repo.

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
| `Cargo.toml`, `crates/` | Rust workspace: `larmorx-core`, `-io`, `-transform`, `-interp`, `-ants`, `-cli` (standalone `larmorx`/`lx`), `-py` (`larmorx._core`) |
| `pyproject.toml`, `python/larmorx/` | maturin package (abi3, CPython >= 3.12), including `pipelines/larmorprepx/` |
| `tests/python/` | Python tests of the installed package |
| `.github/workflows/ci.yml` | lint + wheel build and tests on the six targets |
| `validation/` | parity suites and benchmarks (dev-only package `larmorx-validation`) |
| `docs/architecture.md`, `docs/api/`, `docs/validation/`, `docs/benchmarks/`, `docs/findings/` | conventions, user docs per tool, validation records, benchmark reports, findings about the upstream tools |
| `../larmorx-testdata/` (separate repo) | test-data catalog, generated edge cases, hash-verified downloader (`pip install -e`) |
