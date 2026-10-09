# larmorx

**Status: planning; no code yet.**

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
python3 -m venv .venv
larmorx/scripts/bootstrap_workspace.sh --venv .venv --install-rust
```

The script:
- shallow-clones every upstream listed in [upstream.tsv](upstream.tsv) at its pinned commit (about 2 GB; `--skip freesurfer,afni` saves about 1.1 GB)
- installs Rust with rustup (user-level)
- installs the evaluation packages into the venv

## Contributing
- Commit messages, PR descriptions and release notes must not contain AI co-author trailers (`Co-Authored-By: Claude …`) or "Generated with …" lines.
- Follow the rules in [CLAUDE.md](CLAUDE.md): clean-room policy, provenance records, and the per-tool contract.
