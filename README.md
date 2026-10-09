# larmorx

**Status: phase L0, a buildable project skeleton; no tools yet.**

**larmorx** is a library of neuroimaging tools in Rust, with a Python wrapper and a CLI for every tool:
- tools from ANTs/ITK, AFNI and FreeSurfer, ported to Rust
- FSL- and Workbench-like tools, re-implemented clean-room

It also ships **larmorprepx**, an fMRIPrep-like fMRI preprocessing pipeline written in plain Python on top of the library.

Both will install as native wheels on Windows, Linux and macOS (x64 and arm64), with no Docker and no external neuroimaging software. Outputs will be fMRIPrep-compatible.

```bash
pip install larmorx                  # the tool library:  import larmorx as lx
uv tool install "larmorx[prep]"      # plus the larmorprepx pipeline command   (planned)
```

| Document | Contents |
|---|---|
| [PLAN.md](PLAN.md) | the plan |
| [CLAUDE.md](CLAUDE.md) | project context and rules for contributors |
| [docs/decision-log.md](docs/decision-log.md) | decisions so far |
| [docs/analysis/](docs/analysis/) | what fMRIPrep computes, in detail |

## Setting up a development workspace
The repo expects to live inside a workspace directory, next to read-only clones of the upstream projects it is based on:

```bash
mkdir -p ~/work/super_fmriprep && cd ~/work/super_fmriprep
git clone https://github.com/karellopez/larmorx.git
python3.12 -m venv .venv           # Python >= 3.12
larmorx/scripts/bootstrap_workspace.sh --venv .venv --install-rust
```

The script:
- shallow-clones every upstream listed in [upstream.tsv](upstream.tsv) at its pinned commit (about 2 GB; `--skip freesurfer,afni` saves about 1.1 GB)
- installs Rust with rustup (user-level)
- installs the evaluation packages into the venv

## Building and testing
You need Rust (stable, via rustup) and Python ≥ 3.12. From the repo, with the workspace venv active:

```bash
source ../.venv/bin/activate
pip install --group dev          # maturin, pytest, packaging, ruff (pip >= 25.1)
maturin develop                  # build the larmorx._core extension into the venv
cargo test --workspace           # Rust tests
pytest                           # Python tests of the installed package
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
ruff check && ruff format --check
```

| Path | Contents |
|---|---|
| `crates/larmorx-core`, `crates/larmorx-io` | shared Rust foundation (stubs for now) |
| `crates/larmorx-cli` | the multicall CLI; standalone `larmorx` and `lx` binaries |
| `crates/larmorx-py` | the `larmorx._core` extension module (PyO3, abi3 for CPython ≥ 3.12) |
| `python/larmorx` | the Python package, including `pipelines/larmorprepx` |
| `tests/python` | Python tests |
| `.github/workflows/ci.yml` | lint, plus wheel build and tests on the six target platforms |

## Contributing
- Commit messages, PR descriptions and release notes must not contain AI co-author trailers (`Co-Authored-By: Claude …`) or "Generated with …" lines.
- Follow the rules in [CLAUDE.md](CLAUDE.md): clean-room policy, provenance records, and the per-tool contract.
