# Decision log

Newest entries last. Each entry records what was decided, by whom, and why.

## 2026-10-08: Analysis of fMRIPrep and its dependencies
- Cloned fMRIPrep and its NiPreps/Python dependencies, plus the sources of the binaries it calls (ANTs, AFNI, FreeSurfer, Workbench, MSM_HOCR). The commits are pinned in `upstream.tsv`.
- FSL was **not** cloned: it has a non-commercial licence, and its tools must be clean-room re-implementations.
- The analysis reports are in `docs/analysis/01–04`.

## 2026-10-08: Plan v0.1, fork fMRIPrep (rejected)
- **Proposal:** fork the NiPreps code, keep the nipype graph node-for-node ("parity mode"), and swap external binaries for Rust kernels behind a backend registry.
- **User decision: rejected.** In the user's words: they want "a similar program as fmriprep, but more convenient and straightforward". fMRIPrep "has a lot of unnecessary complexity with nipype and some other already deprecated libraries". They want "functions that can be orchestrated with Python". Existing Python packages such as ANTs can be used, but where they have limitations we "can port them or even reimagine them for better performance in Rust". Example given: recon-all is very slow, so port it to Rust "with a better logic, so we have the same results and make it x times faster".

## 2026-10-08: Plan v0.2, new pipeline using ANTsPy, Rust for the gaps
- A new pipeline built from plain Python functions with a stage cache instead of a graph engine.
- ANTsPy, nibabel, nitransforms and scikit-image are used where they are adequate.
- Rust kernels cover the gaps: resampling, head-motion correction (HMC), slice-timing correction (STC), BBR, fieldmaps, multi-echo, surfaces.
- recon-all is ported as a separate track ("fx-recon").

## 2026-10-09: Plan v0.3, port ANTs too; a tool library with per-tool Python wrappers (current)
- **User decision:** "since ants also have this limitation of OS systems, lets port it too". "At the end we should have a library with the best functions from other softwares ported into rust. And then, the prep pipeline will be through python". "Each binary should also have its own python wrapper, so other developers can build functions using the wrapper."
- **Result:**
  - the library `fx` (placeholder): ports of ANTs/ITK, AFNI and FreeSurfer, plus clean-room re-implementations of FSL- and Workbench-like tools
  - per-tool contract: Rust API + Python wrapper + compatible CLI
  - the pyfmriprepx pipeline in Python on top
  - ANTsPy becomes a test oracle and an interim backend only
- **Supporting facts:**
  - ANTsPy has no aarch64, win-arm64 or Python 3.14 wheels.
  - All other core Python dependencies have wheels on all six targets.

## 2026-10-09: Development moves to the user's Linux machine
- **User decision:** develop on the Linux machine, because it is always on (long, uninterrupted work and oracle runs).
- The Mac workspace (`~/VScode_project/super_fmriprep`) keeps the original clones and `.venv`. It is no longer the main development host.
- The workspace on Linux is recreated with `scripts/bootstrap_workspace.sh`.

## 2026-10-09: Names decided (D1) and the pipeline moves inside the package (D3)
- **User decision:** the project and package are named **larmorx**, and the repo has the same name. The future pipeline is **larmorprepx**, "inside the package".
- **Why larmorx:** the Larmor precession is the physics at the heart of every MRI signal, and the "x" suggests cross-platform. It is a unique token: no exact web results, and no GitHub repos used the name that day.
- **Availability checked 2026-10-09 (all free):**
  - PyPI and crates.io: `larmorx`, `larmorx-freesurfer`, `larmorx-core`/`-ants`/`-afni`/`-mri`/`-cli`/`-py`, `larmorprepx`
  - GitHub accounts `larmorx` and `larmorprepx`
  - domains `larmorx.org`, `.io` and `.com` showed as unregistered
- **Avoid plain "larmor"**: an unrelated solid-state NMR package already ships a `larmor` command.
- **Resulting layout:**
  - GitHub repo `github.com/karellopez/larmorx` (created by the user 2026-10-09, public; a `larmorx` org can be reserved later and the repo transferred)
  - wheel `larmorx`, imported as `import larmorx as lx`, with the CLI `larmorx` (alias `lx`)
  - crates `larmorx-*`
  - pipeline module `larmorx.pipelines.larmorprepx`, with the command `larmorprepx` (installed with the `[prep]` extra)
  - FreeSurfer port in a separate wheel, `larmorx-freesurfer` (licence isolation)
- The earlier entries above use the old working names (`pyfmriprepx`, `fx`) as they were at the time.

## 2026-10-09: Repository created; no AI co-authorship in git history
- **User:** created the public repo `https://github.com/karellopez/larmorx.git`.
- **User decision:** every commit and push must be made **without Claude as co-author**: no `Co-Authored-By` trailers and no "Generated with Claude Code" lines.
- **Where the rule is recorded:** CLAUDE.md (hard rule 9) and the README's contributing section.
- **Privacy:** the repo's local git config uses the user's GitHub no-reply email, so their personal address is not exposed in public commits.

## 2026-10-09: Licence decided (D2)
- **User decision:** our code is licensed under **Apache-2.0**.
- `LICENSE` holds the Apache-2.0 text and `NOTICE` the project notice. Ported ANTs/ITK code will add its upstream NOTICE entries there.
- FreeSurfer-derived code stays in the separate `larmorx-freesurfer` wheel under its own licence terms (D3).

## 2026-10-09: Phase L1 foundations: images, NIfTI I/O, test data, validation
- **User direction:** start the implementation with a high-quality architecture; keep test data in a separate, comprehensive and well-organised repository; prove parity with the replicated software with reports; measure performance.
- **Test data:** a separate repository, `larmorx-testdata` (next to `larmorx/` in the workspace; to be published as `github.com/karellopez/larmorx-testdata`). It holds a TOML catalog (source, licence, SHA-256, size, tier, tags), generated edge cases committed in the repo, and a standard-library downloader into a content-addressed cache. Tiers: `smoke` (≤ 50 MB, every CI run), `standard` (≤ 2 GB), `full` (benchmarks).
- **Validation framework:** `validation/` (package `larmorx-validation`, development only) runs parity suites (statuses `pass`, `both-error`, `expected-divergence`, `fail`) and benchmarks. Reports are committed in `docs/validation/` (the per-tool validation record) and `docs/benchmarks/`; `tests/parity/` runs the suites in CI.
- **Image model:** one `lx.Image` (data, affine, optional header) for every dimensionality, instead of separate `Image`/`Image4D` classes; 4D specifics (`tr`, slice timing) come from the header. A frozen dataclass over numpy; headers cross the Rust boundary as dicts, so no Rust object reaches Python.
- **NIfTI I/O semantics = nibabel 5.x**, validated case by case. Deliberate differences, each documented in `docs/api/io.md`:
  - arrays are returned in native byte order (values identical);
  - `save` stores the array's own type; nibabel rescales into the header's type;
  - gzip is detected from the content, not the name;
  - a single file with `vox_offset` 0 is an error (nibabel reads garbage);
  - `get_fdata`-style reads of complex data raise (nibabel drops the imaginary part);
  - the default gzip level is 2: zlib-rs's level 2 matches the file size of zlib's level 1 (nibabel's default) and is faster; zlib-rs's level 1 compresses about 15% less;
  - RGB24/RGBA32 data are not supported yet.
- **Finding (needs a decision):** no ANTs 2.6.x release pins ITK 5.4.7, contrary to CLAUDE.md; fMRIPrep's image runs ANTs 2.6.2 with ITK 5.4.4. See `UPSTREAM.md`.

## Open decisions (PLAN.md §16)

| # | Decision | Recommended default (used until decided) | Status |
|---|---|---|---|
| D1 | Names | `larmorx` (package, repo, CLI `larmorx`/`lx`, crates `larmorx-*`); pipeline `larmorprepx` | **decided 2026-10-09** |
| D2 | Licence of our code | Apache-2.0 | **decided 2026-10-09** |
| D3 | Distributions | wheel `larmorx` (library + larmorprepx via the `[prep]` extra), wheel `larmorx-freesurfer`, standalone CLI | **decided 2026-10-09** |
| D4 | Names of clean-room tools | neutral names with documented compatibility | open |
| D5 | FreeSurfer version to port | v8.2.0, also validated against 7.4.x | open |
| D6 | FreeSurfer atlas data | download at first use after licence acceptance | open |
| D7 | Commercial project or users? | unknown. Until answered, do not run FSL/MSM binaries | open |
| D8 | Output compatibility | fMRIPrep-compatible derivative names and confound columns | open |
| D9 | Minimum Python | 3.12 | open |
| D10 | First milestone | M1 Preview (pipeline on x64 with interim ANTsPy) | open |
