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
both builds (one SHA-256 over every output file, decompressed, the same in every run of both
builds, warm-ups included). Nothing is left out of a `dist` build: the same crates and features, only the code layout
(cross-crate inlining) and the symbol table differ.

## Relay 2c: the companions' version and their golden tests on installed programs

`larmorx-nc --version` and `larmorx-gpl --version` (and their `--help`'s first line) print the
version in PEP 440's form, `0.1.0rc1`, as pip, PyPI and `larmorx --version` do: a `build.rs` in
each CLI crate converts Cargo's version with larmorx-core's `src/pep440.rs` (included as it is).
tedana's `dataset_description.json` still names `larmorx-gpl 0.1.0-rc.1` (Cargo's form; the same
string from 0.1.0 on). The gpl golden harness writes larmorx-gpl's own version, in either form,
as `<version>` in that file's `"Python"` field, so a release changes no checksum; its 9 cases
were re-recorded once, from the clean commit `13013f21`. Both companion harnesses take
`LARMORX_GOLDEN_PROGRAM` (a path, or a name found on `PATH`): the golden cases then run that
program instead of cargo's build, after checking that its `--version` is the checkout's.

| Helper | What it shows |
|---|---|
| `gpl_golden_version_only.py --old OLD.tsv --new NEW.tsv --outputs DIR` | the gpl golden record before (`--old`, recorded at 0.0.1) and after the re-recording (`--new`) hold the same outputs and differ in exactly the 9 `dataset_description.json` lines; each of those saved files (`DIR`: the harness's `LARMORX_GOLDEN_MISMATCHES` folder from the run just before the re-recording), its `Node` block dropped, hashes to the old record with the version it names written as `0.0.1` and to the new one with `<version>` (result 2026-11-10: 203 outputs, 9 changed, 9 of 9 equal to both). To make it again: at `13013f21`, `LARMORX_GOLDEN_MISMATCHES=DIR cargo test --locked -p larmorx-gpl-cli --test golden` in `crates-gpl` (it fails with the 9 cases and saves them), `--old` from `git show 13013f21:crates-gpl/larmorx-gpl-cli/tests/golden.tsv`, `--new` from `8ffd5448` |
| `installed_golden.sh <checkout> <out> [<larmorx-gpl of another version>]` | the companions' sdists made, a wheel built from each with the `dist` profile (pip, build isolation), installed in a fresh venv of the system's CPython 3.12, and both golden harnesses run on the installed programs through `LARMORX_GOLDEN_PROGRAM`, with the negative controls (result 2026-11-10 at `8ffd5448`: the programs print `larmorx-nc 0.1.0rc1` and `larmorx-gpl 0.1.0rc1`; every golden case of both passes on them, larmorx-gpl's also by its name on `PATH`; larmorx-nc's golden test in 21 s on the installed program against about 60 s on cargo's debug build; a missing program and relay 2b's larmorx-gpl, which prints `0.1.0-rc.1`, fail with the reason named) |

## Relay 3a: the release workflow

`.github/workflows/release.yml` builds, tests and (on a `v*` tag only, after the maintainer's
approval) publishes the release; its helpers are `scripts/release/changelog.py` (the release
notes), `archives.py` (the standalone archives), `smoke.py` (the installed packages' and the
archives' checks), `assemble.py` (the whole set and `SHA256SUMS`), `pypi_state.py` (the version
absent from PyPI before, every file there after) and `ci_green.py` (the tagged commit on main,
CI green), tested by `validation/tests/test_release.py`. Checked here with actionlint 1.7.12
(shellcheck 0.11.0 on every `run:` script) and zizmor 1.32.0 (`--offline`; no findings, also none
above "info" with `--persona=auditor`).

| Helper | What it shows |
|---|---|
| `workflow_rehearsal.sh <checkout> <out> [<maturin>]` | the workflow's Linux steps on a fresh clone of the commit, outside GitHub, with the workflow's maturin (1.15.0): the three sdists (PyPI's descriptions rewritten at the commit, `twine check --strict`), the companions' wheels built from their unpacked sdists inside the clone, the standalone `larmorx` as a program wheel, the companions' golden tests on the programs their wheels install, the archives, the main wheel, and the smoke checks in a fresh venv of the system's CPython 3.12. The host's glibc (2.35) tags the wheels and makes the smoke's glibc 2.28 check fail (the workflow builds in the manylinux_2_28 container): that failure is expected there, every other check must pass |

**The rehearsal (2026-11-11, at `46ed4455`, maturin 1.15.0, 28 threads, build caches of relays 1
and 2b; log `oracles/logs/release-3a-rehearsal.log`):** every step passed: the three sdists
(3 s; larmorx 3.22 MB, larmorx-nc 3.27 MB, larmorx-gpl 0.90 MB; `twine check --strict` passed),
larmorx-nc's wheel from its unpacked sdist (44 s, 6.21 MB), larmorx-gpl's (15 s, 1.06 MB), the
standalone `larmorx` and `lx` as a program wheel (271 s, two LTO links; 20.5 MB, not published),
the companions' golden tests on the programs their wheels install (42 s: larmorx-nc 2 of 2 tests,
larmorx-gpl 3 of 3, every case), the archives (4 s: `larmorx-0.1.0rc1-x86_64-unknown-linux-gnu.tar.gz`
10.0 MB holding the 25 MB program once and `lx` as a link to it, `larmorx-nc-...` 5.82 MB with
300 files (the FSL data, FSL's original source and the amended source), `larmorx-gpl-...`
1.01 MB), larmorx's wheel (335 s); the smoke checks on a fresh venv with the three wheels: 47 of
53 passed, the other 6 the glibc 2.28 check of the six programs and extensions, built against this
host's glibc 2.35 (they need `GLIBC_2.34`: the check works; the workflow builds them in the
manylinux_2_28 container).

**The test job's steps** (on the rehearsal's files, in fresh venvs of CPython 3.12; 2026-11-11, log
`oracles/logs/release-3a-testjob.log`; since then part of `workflow_rehearsal.sh`): `pytest
tests/python tests/golden` with `LARMORX_NC_BIN` and `LARMORX_GPL_BIN` at the installed programs
failed one test, `tests/python/test_mri_replica.py::test_the_replica_itself` ("LARMORX_NC_BIN is
set to 'None'"): the test read the variable after its `no_replica` fixture had removed it, so it
ran only beside a repository build (CI sets no variable and has no build: it skipped there).
Fixed in the test (the variable read once, at import, as `test_tedana.py` does); then 885 passed,
51 skipped (that test with the variable 5 of 5 in its file; without it and without a build:
skipped, as in CI). `larmorx[all]` from the built files: 21 s, `pip check` clean, the smoke's
checks all pass but the glibc ones, the `[prep]` modules present.

**The rehearsal again, with the test job's steps (2026-11-11, at `4b5c5d4d`; log
`oracles/logs/release-3a-rehearsal-2.log`):** every step passed, the same times within seconds;
`pytest tests/python tests/golden` on the installed wheels 885 passed, 51 skipped; the smoke 47 of
53 (the 6 expected glibc checks); `larmorx[all]` from the built files 18 s, `pip check` clean.

## Relay 3b: the container images

`docker/Dockerfile` has two targets, built from the release's own Linux wheels on top of
`python:3.12.16-slim-trixie` (pinned by its multi-architecture digest): `larmorx`
(`ghcr.io/karellopez/larmorx:<v>`: larmorx with its `[prep]` dependencies, Apache-2.0 and
permissive licences only) and `larmorx-all` (`:<v>-all`: also larmorx-gpl and larmorx-nc, labelled
non-commercial, their sdists in `/opt/larmorx/source/`); `docker/README.txt` is copied into both.
`scripts/release/container.py` does every step the workflow runs (`stage`, `build`, `check`,
`compare`, `index`, `push`); the jobs are `container` (amd64 on `ubuntu-24.04`, arm64 on
`ubuntu-24.04-arm`; every run; nothing pushed) and `container-push` (a release only, after the
three PyPI uploads; the only job with `packages: write`). Tested by
`validation/tests/test_release.py` (the build context, the multi-architecture index on synthetic
OCI archives, the Dockerfile's targets, labels and pins, the jobs' order and permissions).

| Helper | What it shows |
|---|---|
| `rootless_docker.sh start\|stop\|env <state>` | a rootless Docker daemon with no administrator rights (Docker's rootless extras, the account's subordinate ids), its images in `<state>`, in a memory box of its own |
| `container_rehearsal.sh <checkout> <wheels> <sdists> <out>` | the two container jobs' steps for linux/amd64, outside GitHub: `container.py stage`, `build` (buildx's docker-container builder, BuildKit pinned by digest), `check` (inside each image: labels, `smoke.py --image` as root and as an unprivileged user on a read-only file system, the default command, the packages, the companions' sources, the golden tests and `test_smoke.py`); each OCI archive turned into a SIF image by Apptainer, with `apptainer exec` of the versions, `smoke.py` and the golden tests; `container.py index` and `push --latest` into a local registry (distribution 3.0.0, run by Apptainer) with skopeo 1.13.3 (Ubuntu 24.04's, the runner's), each tag read back; `apptainer pull docker://...` of the four pushed tags and the programs in what came back. `FROM=push` repeats only the push part |

**The rehearsal (2026-11-11, at `14f7cc2b`, with relay 3a's rehearsal files: the three wheels
built on this host, tagged `manylinux_2_34_x86_64`, and the companions' sdists; rootless Docker
28.3.0, buildx 0.25.0, BuildKit 0.34.1, Apptainer 1.4.5; log
`oracles/logs/release-3b-container-rehearsal-2.log`): every step passed, about 5 minutes from a
fresh daemon.** The build: 50 s for both images (pip resolves `larmorx[prep]` from PyPI: 48
packages, numpy 2.5.4, scipy 1.18.1, matplotlib 3.11.2, pandas 3.1.1, nilearn 0.14.1,
templateflow 25.1.2; pip backtracks for about 20 s and settles on pybids 0.18.1); OCI archives
226.4 MB (`larmorx`) and 237.5 MB (`larmorx-all`), 681 MB and 701 MB unpacked. The checks inside
(27 of 27): the labels (licences `Apache-2.0`, and `Apache-2.0 AND GPL-3.0-only AND
LicenseRef-FSL`), no entrypoint, `smoke.py --image` as root and as user 65534 on a read-only file
system (the `larmorx` image without larmorx-nc, larmorx-gpl and FSL's data; the `-all` image
with both programs at 0.1.0rc1 and the 9 FSL data files with their SHA-256s), `larmorx --help` as
the default command, the `-all` image's packages exactly `larmorx`'s plus larmorx-nc and
larmorx-gpl, its two sdists; the golden tests and `tests/python/test_smoke.py` inside each image
317 passed (24 s). Apptainer: each OCI archive a SIF image (213 MB, 224 MB), `apptainer exec
--containall` of `larmorx --version`, `larmorx-nc --version` and `larmorx-gpl --version` (the -all
image), `smoke.py --image`, and the golden tests (a venv over the image's packages for pytest):
305 passed in each. The push: one OCI layout with both multi-architecture indexes (amd64 alone
here), `skopeo copy --all --preserve-digests` of `0.1.0rc1`, `latest`, `0.1.0rc1-all` and
`latest-all` into the local registry, each tag read back with the index's own digest (9 of 9
checks; the per-architecture manifest pushed is the one built and checked), then `apptainer pull
docker://127.0.0.1:5055/karellopez/larmorx:<tag>` of the four tags: each runs `larmorx --version`,
the -all image `larmorx-nc --version`, the plain image has no `larmorx-nc`.

**What only GitHub shows:** the arm64 images (built on the arm64 runner; this machine has no
arm64 emulation), the runners' Docker (the check takes an image's id as Docker's own store or
the containerd store gives it), the manylinux_2_28 wheels in the images, GHCR's login and first
push with the workflow's token, the new package's visibility (private at first: the user makes it
public once, D17), skopeo on `ubuntu-24.04` (installed by apt if missing), and the images'
provenance attestations. Nothing of that runs on a dry run: `container-push` runs on a release
only.
