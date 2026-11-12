# larmorx-freesurfer: FreeSurfer's replicas

This directory holds `larmorx-freesurfer`, the package of **replicas** of FreeSurfer 8.2.0's
programs: Rust translations of FreeSurfer's source code, written to give its output bit for bit,
for every program `recon-all` runs in its two streams and the FreeSurfer programs fMRIPrep and
sMRIPrep run outside it ([docs/freesurfer-plan.md](../docs/freesurfer-plan.md)). It is being
built: the workspace, the program and the wheel exist (queue item 3.1-01), the crates are empty
until their queue items bring the code ([docs/freesurfer/queue.md](../docs/freesurfer/queue.md)).

The rest of larmorx is Apache-2.0. Why this directory is separate is in
[docs/licensing.md](../docs/licensing.md):

- FreeSurfer is distributed under the FreeSurfer Software License (version 1.0, February 2011).
  A translation of its source is a modified version of FreeSurfer, so it carries the licence's
  Part B, prefaced with the words its paragraph 1(b) requires (`LICENSE`; the complete agreement
  is `LICENSES/FreeSurfer-LICENSE.txt`), keeps FreeSurfer's notices and is marked as modified
  (`NOTICE`, each crate's `PROVENANCE.md`). Its users receive those terms: research use only, no
  clinical use, as is.
- The main `larmorx` package will have a **clean room** of every program (Apache-2.0, the crates
  `crates/larmorx-fs-*`), written from the specs (`specs/`) and black-box runs without seeing
  FreeSurfer's source or this directory.
- Licence families never mix in one binary. These crates use the Apache crates by path (Apache-2.0
  code may be included in a work under other terms), but no Apache crate may depend on them, and
  they never link the FSL replicas (`crates-nc/`) or the GPL ones (`crates-gpl/`). The third-party
  code inside FreeSurfer goes where its own licence puts it (`docs/freesurfer/third-party.md`
  section 9): the permissive pieces are ported into the main package's crates from their own
  upstreams and used here by path.

## Contents

| Crate | What (`docs/freesurfer/waves.md` section 4) |
|---|---|
| `larmorx-freesurfer-core` | `fs.core` (messages, diagnostics, program name, environment, version stamps, ROMP, timers) and `fs.maths` (matrices, the numerics wrappers, `fs_powell`, `fs_lbfgs`, statistics); `run`, what the program gives a program's replica |
| `larmorx-freesurfer-io` | `fs.io` (`MRIread`/`MRIwrite`, tags, the format probe) and `fs.labels` (colour tables, annotations, labels, `register.dat`, point sets) |
| `larmorx-freesurfer-volume` | `fs.volume` (the `MRI` structure, transforms, resampling, warp fields, CMA) and `fs.bspline` (EPFL's B-splines as Reuter's `mriBSpline.cpp` writes them) |
| `larmorx-freesurfer-filter` | `fs.filter` (filters, normalisation, segmentation helpers, flood fill) |
| `larmorx-freesurfer-surf` | `fs.surfio` (the surface structure and its I/O), `fs.surf` (the rest of the surface library) |
| `larmorx-freesurfer-deform` | `fs.deform` (surface deformation) |
| `larmorx-freesurfer-defect` | `fs.defect` (the topology fixer's defects) |
| `larmorx-freesurfer-gca` | `fs.gca` (GCA atlases and morphs) |
| `larmorx-freesurfer-robust` | `fs.robust` (Reuter's robust registration) |
| `larmorx-freesurfer-volumes`, `-atlas`, `-topology`, `-cortex`, `-classic`, `-networks`, `-recon` | the programs of waves 3.2, 3.3, 3.4, 3.5, 3.6, N and 3.7, one module per program; each crate's `PROGRAMS` registers them |
| `larmorx-freesurfer-cli` | the `larmorx-freesurfer` command line: `larmorx-freesurfer [--n-threads N] [--upstream-defects fix\|reproduce] [--implementation replica] freesurfer <program> <the original's arguments>` (`larmorx-freesurfer --help`; each option wins over the variable it replaces, `OMP_NUM_THREADS`, `LARMORX_UPSTREAM_DEFECTS`) |
| `pyproject.toml`, `NOTICE`, `LICENSE`, `LICENSES/` | the `larmorx-freesurfer` wheel and the licences and notices it carries (below) |

## Building and testing

This is its own Cargo workspace; the root workspace does not build it.

```bash
cd crates-freesurfer
cargo build --release -p larmorx-freesurfer-cli
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

The build writes the program `target/release/larmorx-freesurfer`.
`scripts/orchestration/gates-rust.sh <checkout> freesurfer` runs the workspace's format, lint and
tests (`all` includes it).

## The wheel

`maturin build --release --locked` in this directory builds the `larmorx-freesurfer` wheel: the
program only (maturin's `bin` bindings), no Python module. Its licence files are FreeSurfer's
licence with its preface (`LICENSE`), the notices (`NOTICE`), the complete FreeSurfer agreement
(`LICENSES/FreeSurfer-LICENSE.txt`), the Apache-2.0 text and notices of the larmorx crates it
includes (`LICENSES/Apache-2.0.txt`, `LICENSES/NOTICE-larmorx.txt`, a copy of the repository's
`NOTICE`), the licence texts of the Rust crates it is built with (`LICENSES/third-party-rust.txt`,
written by `scripts/release/third_party.py`) and this workspace's source. It is not published yet.
