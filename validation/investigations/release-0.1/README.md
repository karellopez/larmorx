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

## Relay 2b: the `dist` profile, Windows' C runtime, the third-party licence texts

The release files are built with `[profile.dist]` (the three workspaces' `Cargo.toml`: `release`
with fat LTO, one codegen unit and the symbols stripped); everyday builds, CI and the agents keep
`release`. `.cargo/config.toml` links Windows' C runtime statically for the MSVC targets
(`+crt-static`; `cargo check --target aarch64-pc-windows-msvc -v` shows the flag on the target's
crates and not on the host's build scripts, and no Linux build gets it).
`scripts/release/pe_imports.py` checks the Windows files' import tables (on this machine:
setuptools' launchers `cli-64.exe`, `gui-arm64.exe`, `cli-32.exe` import `VCRUNTIME140.dll` and
the `api-ms-win-crt-*` DLLs and are refused; ruff 0.16.10's `ruff.exe` from PyPI, a Rust program
built with `+crt-static`, imports none and passes). `scripts/release/third_party.py` writes
each package's `LICENSES/third-party-rust.txt`.

| Helper | What it shows |
|---|---|
| `dist_speed.py run --tree release=DIR --tree dist=DIR` | the speed of the two profiles on five real calls (fMRIPrep's antsRegistration of the BOLD reference and N4 of the T1w, its 3dvolreg, the BOLD resampler, sdcflows' B-spline fit) at one and eight threads, each build in fresh processes, alternating, and whether their outputs are the same bytes |

**Sizes (Linux x86-64, 0.1.0rc1, bytes; the wheels as maturin builds them here):**

| File | `release` | `dist` |
|---|---|---|
| `larmorx` wheel (`_core.abi3.so` unpacked) | 15,242,110 (78,071,352) | 13,946,489 (41,185,208) |
| `larmorx-nc` wheel (the program) | 6,394,099 (8,633,856) | 6,203,548 (6,788,256) |
| `larmorx-gpl` wheel (the program) | 1,285,227 (3,347,088) | 1,059,288 (1,995,256) |
| standalone `larmorx` (`lx` the same size) | 48,449,568 | 25,065,352 |

Build times at 28 threads from cached dependencies: the main extension 340 s with `dist` (one
single-threaded LTO step; `release` about 95 s), the standalone programs (two binaries) three
minutes more (179 s with `dist`, 43 s with `release`), larmorx-nc 46 s, larmorx-gpl 19 s.

**The same bits (Linux x86-64, at `a2704b60`):** the Python golden suite on the `dist` extension
305 of 305; the AFNI replicas' golden (`cargo test --profile dist -p larmorx-cli --test
afni_replica_golden`) and larmorx-nc's (`-p larmorx-nc-cli --test golden`) pass; larmorx-gpl's
fails, in both profiles alike, only its 9 cases whose `dataset_description.json` names the
version (relay 2c re-records them): their outputs saved by the harness are byte for byte the same
under `dist` and `release` (109 of 118 files identical, the other 9 the cases' `stderr.txt`,
which differ only in the temporary folder's name and the progress bars' speeds).

**The parity smoke tier on the `dist` builds** (the `dist` extension in `python/larmorx`, the
`dist` programs of larmorx-nc and larmorx-gpl in the place the suites take them from,
`crates-*/target/release/`, restored afterwards; boxed, 64 GiB): 2,968 passed, 0 failed (16
workers 314 s, then the memory and thread tests alone 1,214 s), as relay 2a's run on the
`release` builds.

**Speed** (`dist_speed.py run --threads 1,8 --rounds 4 --repeats 3`, boxed, nothing else running;
the median of 12 timed runs per row, 4 for the rows that take over 5 s per run; 703 s in all):

| Call | Threads | `release` | `dist` | `dist` / `release` | Same outputs |
|---|---|---|---|---|---|
| antsRegistration, ds005003's `fmapreg_wf/coregister` | 1 | 0.787 s | 0.732 s | 0.930 | yes |
| | 8 | 0.211 s | 0.186 s | 0.884 | yes |
| N4BiasFieldCorrection, ds000005's T1w (`inu_n4`) | 1 | 5.905 s | 5.275 s | 0.893 | yes |
| | 8 | 1.196 s | 1.146 s | 0.958 | yes |
| 3dvolreg, ds000005 run 1 (RobustAverage; `auto` runs the replica, one thread) | 1 | 2.333 s | 2.335 s | 1.001 | yes |
| | 8 | 2.247 s | 2.252 s | 1.002 | yes |
| `lx.transforms.resample_series`, ds000005 run 1, motion and field map | 1 | 3.114 s | 3.134 s | 1.007 | yes |
| | 8 | 0.948 s | 0.976 s | 1.029 | yes |
| `lx.sdcflows.bspline_approx`, ds005454's field map | 1 | 19.085 s | 18.055 s | 0.946 | yes |
| | 8 | 3.690 s | 3.617 s | 0.980 | yes |

`dist` is as fast or faster: 5 to 12 % on the ITK-based calls and the B-spline fit at one thread,
within the runs' spread (about 3 %) on 3dvolreg and the resampler; every output the same bytes in
both builds (the SHA-256 of every output file, decompressed, equal across the 160 runs of a row
pair). Nothing is left out of a `dist` build: the same crates and features, only the code layout
(cross-crate inlining) and the symbol table differ.
