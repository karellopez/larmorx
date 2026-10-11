# larmorx

**Status: early development.** Available now:
- `lx.Image` and NIfTI-1/2 reading and writing, [validated against nibabel](docs/validation/nifti-io.md) (252 files, bit-identical data) and [faster than nibabel and SimpleITK](docs/benchmarks/nifti-io.md) (compressed writes 4–8× faster with threads);
- `lx.ants.apply_transforms` / `larmorx ants antsApplyTransforms`, [validated against ANTs](docs/validation/ants-apply-transforms.md) (63 of 79 cases bit-identical, all agree);
- `lx.afni.tshift` / `larmorx afni 3dTshift`, a clean-room implementation [validated against AFNI](docs/validation/afni-tshift.md) (217 of 217 compared cases agree, 158 bit-identical) and [2–6× faster](docs/benchmarks/afni-tshift.md); with the separate GPL package `larmorx-gpl` installed (`larmorx[exact]`, built from `crates-gpl/`), it runs a bit-exact replica instead, as a separate program ([217 of 217 bit-identical](docs/validation/afni-tshift-replica.md), [licensing](docs/licensing.md));
- `lx.mri.hmc` / `larmorx mri hmc`, head-motion correction accepting mcflirt-style options, a clean-room implementation, `experimental`, [compared with FSL's mcflirt](docs/validation/mri-hmc.md) (with fMRIPrep's command, closer to mcflirt than mcflirt is to itself under small input perturbations; 121 of 122 compared cases within thresholds) and [5–16× faster with threads](docs/benchmarks/mri-hmc.md).

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
| [docs/overview.md](docs/overview.md) | the project in plain terms: replicas and clean-room originals, packages, installation, what is published |
| [docs/licensing.md](docs/licensing.md) | how each upstream's licence is handled |
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
pip install --group dev          # maturin, pytest, packaging, nibabel, ruff (pip >= 25.1)
maturin develop --release        # build the larmorx._core extension into the venv
cargo test --workspace           # Rust tests
pytest tests/python              # Python tests of the installed package
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
ruff check && ruff format --check
```

Parity checks and benchmarks need the test data (`larmorx-testdata`, a separate repository kept next to this one; not published yet) and the validation package:

```bash
pip install -e ../larmorx-testdata -e "validation[oracles]"
pytest tests/parity                                             # parity with the reference tools (smoke tier)
python -m larmorx_validation parity nifti-io --tier standard --out docs/validation
python -m larmorx_validation bench nifti-io --out docs/benchmarks
```

| Path | Contents |
|---|---|
| `crates/larmorx-core`, `crates/larmorx-io` | shared Rust foundation: images, grids, linear algebra, NIfTI and transform files |
| `crates/larmorx-transform`, `crates/larmorx-interp`, `crates/larmorx-ants` | ITK transforms and interpolators; ANTs tools (antsApplyTransforms) |
| `crates/larmorx-cli` | the multicall CLI; standalone `larmorx` and `lx` binaries |
| `crates/larmorx-py` | the `larmorx._core` extension module (PyO3, abi3 for CPython ≥ 3.12) |
| `python/larmorx` | the Python package, including `pipelines/larmorprepx` |
| `tests/python`, `tests/parity` | Python tests; parity suites under pytest |
| `validation/` | parity suites and benchmarks against the reference tools |
| `docs/` | [architecture](docs/architecture.md), [API pages](docs/api/), validation records, benchmark reports, [findings](docs/findings/) about the tools we replicate |
| `.github/workflows/ci.yml` | lint, plus wheel build and tests on the six target platforms |

## Contributing
- Commit messages, PR descriptions and release notes must not contain AI co-author trailers (`Co-Authored-By: Claude …`) or "Generated with …" lines.
- Follow the rules in [CLAUDE.md](CLAUDE.md): clean-room policy, provenance records, and the per-tool contract.
