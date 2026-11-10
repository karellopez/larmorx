# The first release (0.1.0rc1): the measurements behind its packaging

The helpers of the release engineer's relays (2026-11-10), kept so that every number of the
release part's notes can be made again on another machine. The release's own tools are in
`scripts/release/` (`set_version.py`, `licences.py`, `pypi_readme.py`, `companion_sdist.py`,
tested by `validation/tests/test_release.py`); these are the experiments that led to them.
Each shell script takes the checkout and an output folder and sources
`scripts/orchestration/env.sh` (FSL off the path, the workspace's venv); run the heavy ones in a
memory box (`scripts/orchestration/boxed.sh 32 bash <script> ...`).

## Relay 1: inventory (at 459a5a21, version 0.0.1, Linux x86-64)

| Helper | What it measures |
|---|---|
| `build.sh <checkout> <out>` | the three wheels as built today and with the size settings set through the environment only (strip; thin LTO; fat LTO with one codegen unit): sizes and build times; the main sdist; `maturin sdist` failing for both companions |
| `staging-sdist.py <checkout> <nc\|gpl> <out>` | the experiment that found the working way to the companions' sdists: the tracked tree copied with the companion's `pyproject.toml` at its root (superseded by `scripts/release/companion_sdist.py`) |
| `sdist-build.sh <out>`, `sdist_summary.py <sdist>...` | whether each sdist builds a wheel again, and what each holds (top-level layout, size, trees that must not be in it) |
| `golden-experiments.sh <checkout> <out>` | A: the four golden suites (Python, AFNI replicas, larmorx-nc, larmorx-gpl) on fat-LTO, one-codegen-unit, stripped builds at 0.0.1 (all pass); B: which golden cases depend on the version (the workspaces at 0.1.0-rc.1, reverted at the end): 4 Python cases (the HDF5 writer's `/OSVersion`) and 9 of larmorx-gpl (tedana's `dataset_description.json`) |
| `deps/resolve.sh`, `deps/summarize.py <dir>` | pip dry runs (wheels only, nothing installed) of larmorx's and `[prep]`'s dependencies for the six platforms and CPython 3.12 and 3.14: every package has wheels everywhere, Windows arm64 included; macOS needs 12 on Apple silicon (SciPy, scikit-learn) and 10.15 on Intel |
| `action_pins.py` | the newest release of each GitHub Action a release workflow uses, with the commit SHA its tag points to (GitHub's public API, no login) |
| `winps.sh <script.ps1>`, `win-inventory.ps1` | the Windows helper's state over SSH (the helper's SSH alias, `docs/helper-machines.md`): Windows version, administrator token, Visual C++ runtime present, Python, uv, Windows Sandbox |

## Relay 2a: versions, metadata, sdists (version 0.1.0rc1)

| Helper | What it shows |
|---|---|
| `golden_version_only.py [case ...]` | run at relay 2a's commit before the golden re-recording (`golden.json` then recorded at 0.0.1): the four Python golden cases that change when `/OSVersion` is recorded as `<version>` (`tests/golden/registry.py`, `_h5_structure`) differ by that string only: with `<version>` written back as `0.0.1`, their fingerprints equal the recorded ones (result 2026-11-10: 4 of 4 identical) |
| `sdists-install.sh <checkout> <out>` | the three sdists made (`maturin sdist`; `companion_sdist.py nc` and `gpl`, each checked complete), `twine check --strict` on them, a wheel built from each with pip in a fresh venv of the system's CPython 3.12 (build isolation: maturin from PyPI, the machine's Rust), installed there, the versions of the five programs, larmorx-nc's FSL data, and `pytest tests/python tests/golden` on the installed packages without FSL (result 2026-11-10: 879 passed, 53 skipped, the 4 golden cases above before their re-recording; the three wheels from the sdists in 21 s with cached dependencies) |
| `wheelinfo.py <wheel dir> <sdist dir>` | what the wheels and sdists hold: licence files and where they sit, data files, and the metadata PyPI shows (version, summary, licence expression, authors, URLs, classifiers, requirements) |
