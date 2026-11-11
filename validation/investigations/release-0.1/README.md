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

**The rehearsal (2026-11-11, at `5d1c328b`, with relay 3a's rehearsal files: the three wheels
built on this host, tagged `manylinux_2_34_x86_64`, and the companions' sdists; rootless Docker
28.3.0, buildx 0.25.0, BuildKit 0.34.1, Apptainer 1.4.5; log
`oracles/logs/release-3b-container-rehearsal-3.log`): every step passed, about 5 minutes from a
fresh daemon.** The build: about 50 s for both images (pip resolves `larmorx[prep]` from PyPI: 49
packages, numpy 2.5.4, scipy 1.18.1, matplotlib 3.11.2, pandas 3.1.1, nilearn 0.14.1,
templateflow 25.1.2, pybids 0.22.0); OCI archives 226.7 MB (`larmorx`) and 237.9 MB
(`larmorx-all`), about 680 and 700 MB unpacked. The checks inside (27 of 27): the labels
(licences `Apache-2.0`, and `Apache-2.0 AND GPL-3.0-only AND LicenseRef-FSL`), no entrypoint,
`smoke.py --image` as root and as user 65534 on a read-only file system (the `larmorx` image
without larmorx-nc, larmorx-gpl and FSL's data; the `-all` image with both programs at 0.1.0rc1
and the 9 FSL data files with their SHA-256s), `larmorx --help` as the default command, the
`-all` image's packages exactly `larmorx`'s plus larmorx-nc and larmorx-gpl, its two sdists; the
golden tests and `tests/python/test_smoke.py` inside each image 317 passed (24 s). Apptainer:
each OCI archive a SIF image (213 MB, 224 MB), `apptainer exec --containall` of `larmorx
--version`, `larmorx-nc --version` and `larmorx-gpl --version` (the -all image), `smoke.py
--image`, and the golden tests (a venv over the image's packages for pytest): 305 passed in each.
The push: one OCI layout with both multi-architecture indexes (amd64 alone here), `skopeo copy
--all --preserve-digests` of `0.1.0rc1`, `latest`, `0.1.0rc1-all` and `latest-all` into the local
registry, each tag read back with the index's own digest (9 of 9 checks; the per-architecture
manifests pushed are the ones built and checked), then `apptainer pull
docker://127.0.0.1:5055/karellopez/larmorx:<tag>` of the four tags: each runs `larmorx --version`,
the -all image `larmorx-nc --version`, the plain image has no `larmorx-nc`. The earlier runs (logs
`release-3b-container-rehearsal.log`, `-2.log`, `release-3b-push-try.log`): the first stopped at
the push on this script's own image names (Apptainer takes a digest without a tag; fixed), the
second passed every step at `14f7cc2b` with pybids 0.18.1: with `--only-binary :all:` pip cannot
take docopt (an sdist only), which num2words 0.5.10 and newer need for pybids 0.19 and newer, and
backtracked for about 20 s; a user's plain `pip install` builds docopt and gets pybids 0.22.0, so
the images (and the workflow's `larmorx[all]` checks, now `--only-binary
larmorx,larmorx-nc,larmorx-gpl`) install the dependencies as a user's pip does (`67d00142`).

**What only GitHub shows:** the arm64 images (built on the arm64 runner; this machine has no
arm64 emulation), the runners' Docker (the check takes an image's id as Docker's own store or
the containerd store gives it), the manylinux_2_28 wheels in the images, GHCR's login and first
push with the workflow's token, the new package's visibility (private at first: the user makes it
public once, D17), skopeo on `ubuntu-24.04` (installed by apt if missing), and the images'
provenance attestations. Nothing of that runs on a dry run: `container-push` runs on a release
only.

**Dry run 1 (GitHub run 42927413161 at `d5421dac`, the workflow before the container jobs):**
check, sdist, the six wheels and five of the six programs jobs passed; two failures, fixed in
`79935496`. (1) `sdist-install`: the smoke's glibc 2.28 check on the packages pip built there
from the sdists (they link the runner's glibc: `GLIBC_2.39`, `2.34`): `smoke.py --source-build`
leaves the binaries' platform checks out for a build from source; the published files keep them
(the test and verify-pypi jobs). Shown here on relay 3a's host-built venv: without the option
3 failures of 21 (the same glibc checks), with it 17 of 17. (2) `programs (linux-aarch64)`:
`cargo test` in the checkout read `rust-toolchain.toml`, and rustup added its rustfmt and clippy
components to the 1.99.0 the setup step had installed: "detected conflict: 'bin/cargo-clippy'" on
`ubuntu-24.04-arm`. The step now sets `RUSTUP_TOOLCHAIN` to the installed toolchain, which rustup
puts before the toolchain file (`rustup show active-toolchain`: "overridden by environment
variable RUSTUP_TOOLCHAIN"), so no component is added; the tests need neither. The next dry run
is the first to show the test job on the six platforms, and the container jobs.

## Relay 4: clean machines

| Helper | What it shows |
|---|---|
| `clean_build.sh <checkout> <out> <manylinux_2_28.sif>` | the release's Linux x86_64 files built as `release.yml` builds them, inside the manylinux_2_28 image it pins (by digest), through Apptainer, with no administrator rights and nothing of this machine's on the paths (`--containall --cleanenv`, a fresh clone of the commit, Rust 1.99.0 by rustup, maturin 1.15.0 and twine 7.0.0 by pip, all in `<out>`): the sdist job, the wheel job (`--profile dist --locked --manylinux 2_28`), the programs job (the companions from their unpacked sdists, the standalone program wheel, the companions' golden tests on their installed programs, the archives), `twine check --strict`, `SHA256SUMS` |
| `clean_env.sh <name> <image.sif> <python> <build out> <out>` | those files installed and tested in one clean environment (an image run by Apptainer as this unprivileged user, an empty home, every venv, cache and toolchain in `<out>`, the files and the tests read-only): `pip install larmorx`, `larmorx[prep]`, `larmorx[all]` from the wheels, each in a fresh venv with the venv's own pip; `smoke.py --installed --archives`; the test group and `pytest tests/python tests/golden` with the installed programs; with `SDIST_RUST` the three sdists built from source by pip as a user's `pip install --no-binary` (no `MATURIN_PEP517_ARGS`), the same checks with `--source-build`, and the companions' golden tests (cargo) on both installs' programs; `SDIST_RUST=none` (no compiler): the sdist's build must fail, and what pip says is kept. `RERUN=1` and `SRC=<checkout>` repeat the smoke checks and pytest with another checkout's tests |
| `../../../scripts/release/check_install.py` (with `check_install_windows.ps1`) | the check after a release on a machine of one's own (below): a fresh venv, `pip install "larmorx[all]==<v>"` from PyPI (or `--find-links`), `pip check`, the tag's source tree from GitHub, `smoke.py --installed --archives` on the GitHub Release's archives of this platform, `pytest tests/python tests/golden`; the Windows wrapper keeps everything (uv's Python, every cache) in `%USERPROFILE%\larmorx-check-<v>` and removes it |
| `win_check.sh <version> [--files <dir>] [-- <wrapper arguments>]` | the Windows wrapper run on the Windows helper over SSH (its alias, `docs/helper-machines.md`): the folder made (it must not exist), the two scripts (and `--files`) copied in, the check run, the folder's removal checked |
| `win_gnu_build.sh <clean_build out> <manylinux_2_28.sif>` | Windows x86_64 files for a trial of the wrapper before the release, cross-built with Rust's `x86_64-pc-windows-gnu` target and zig's MinGW-w64 linker (ziglang 0.14.1 from PyPI, maturin's cargo-zigbuild wrappers): no MSVC and no Microsoft SDK, so no licence terms accepted for anyone. `maturin build --zig` of the extension fails (its `.def` names `PyInit_larmorx._core`, the dotted module name, which the link cannot find): built without `--zig`, with the zig wrapper as linker and pyo3's `generate-import-lib` |

**The build (2026-11-11, at `5ef26ea9`, the commit of dry run 2; log
`oracles/logs/release-4-clean-build.log`):** `quay.io/pypa/manylinux_2_28_x86_64:2026.10.26-1`
(`sha256:e0adf185...`; AlmaLinux 8.10, glibc 2.28, CPython 3.12.16, GCC 14.2.1 of gcc-toolset-14),
28 threads, every step passed: the three sdists 7 s (larmorx 3,224,497 bytes, larmorx-nc 3,274,952,
larmorx-gpl 904,181; `twine check --strict` passed); larmorx's wheel 338 s (fat LTO;
`larmorx-0.1.0rc1-cp312-abi3-manylinux_2_28_x86_64.whl`, 13,937,192 bytes); larmorx-nc's wheel
from its sdist 47 s (`py3-none-manylinux_2_28_x86_64`, 6,212,535), larmorx-gpl's 19 s (1,060,330),
the standalone program wheel 273 s (20,528,126); the companions' golden tests on the programs
their wheels install: larmorx-nc 2 of 2 tests (every case, 21 s), larmorx-gpl 3 of 3; the archives
(larmorx 9,997,705, larmorx-nc 5,817,953, larmorx-gpl 1,012,437 bytes). The sizes equal relay
2b's and 3a's host builds; the tags are the ones `assemble.py` expects.

**The clean environments** (logs `oracles/logs/release-4-env-<name>.log` and
`-<name>-rerun.log`; every image pinned by digest; user 1000, no administrator rights):

| Environment | System | Python, pip of its venvs | The wheels | From the sdists |
|---|---|---|---|---|
| `rhel8-py3.12`: `registry.access.redhat.com/ubi8/python-312@sha256:5a65be9c...` | RHEL 8.10, glibc 2.28 (the oldest supported), GCC 8.5.0, objdump | `/usr/bin/python3.12` 3.12.15, pip 23.2.1 | `larmorx` 4 s, `[prep]` 29 s, `[all]` 32 s; `pip check` clean; smoke 52 of 52 (53 with the `objdump` check of the first run), the glibc floor holds: every program and the extension need GLIBC_2.28 at most and run on 2.28; pytest 934 passed, 2 skipped (after the test fix below) | Rust 1.99.0: `[all]` 170 s, `larmorx` 105 s, `[prep]` 133 s (pip 23.2 builds again each time); smoke `--source-build` 17 of 17; pytest 934 passed, 2 skipped; the companions' golden tests (cargo) pass on both installs' programs |
| `debian12-slim-py3.12`: `python:3.12.16-slim-bookworm@sha256:0f086103...` | Debian 12, glibc 2.36, no compiler, no binutils | 3.12.16, pip 25.0.1 | `larmorx` 7 s, `[prep]` 26 s, `[all]` 20 s; `pip check` clean; smoke 52 of 52 (the glibc floor read from the files themselves: no objdump, below); pytest 934 passed, 2 skipped | fails, as it must: maturin's build backend found no Rust, fetched Rust 1.99.1 itself (puccinialin, into the home's cache), then `error: linker 'cc' not found`: **a build from source needs a C compiler** (and Rust 1.99 or newer, which maturin fetches when none is installed) |
| `debian13-py3.14`: `python:3.14-trixie@sha256:e683e47f...` | Debian 13, glibc 2.41, GCC 14.2.0, objdump | 3.14.10 (the abi3 wheel), pip 26.2.2 | `larmorx` 6 s, `[prep]` 28 s, `[all]` 20 s; smoke 52 of 52; pytest 934 passed, 2 skipped | Rust stable (1.99.1, newer than the pin): `[all]` 150 s, then `larmorx` 6 s and `[prep]` 19 s from pip 26's wheel cache; smoke 17 of 17; pytest 934 passed, 2 skipped, **the golden suite included: a newer compiler gives the same bits**; the companions' golden tests pass |

In all three `larmorx[all]` resolves to the same 47 packages besides ours and pip (numpy 2.5.4,
scipy 1.18.1, scikit-learn 1.9.1, pandas 3.1.1, matplotlib 3.11.2, nilearn 0.14.1, templateflow
25.1.2, pybids 0.22.0), but for one: on Python 3.14 pip takes wrapt 2.6.0.dev1, a pre-release,
because formulaic 1.2.2 (pybids') asks for `wrapt>=1.17.0rc1` on Python 3.13 and newer (a
pre-release in a specifier lets pip take pre-releases; on 3.12 it takes wrapt 2.5.2);
`larmorprepx` says it is not ready yet (exit status 1); `larmorx` alone installs neither companion.
**Two findings, fixed:** (1) the first runs failed one test everywhere,
`tests/python/test_sdcflows_app.py::test_exit_statuses` (`OSError: [Errno 30] Read-only file
system: '/src/work'`): its calls without `-w` take the BIDS app's default work folder, `./work`,
in the folder pytest runs from (in a checkout a git-ignored `work/`); the two tests now run in
`tmp_path` (`ebf9dde6`), and the re-runs with the tests of `0d858b2e` pass. (2) In the slim image
`smoke.py` failed its `objdump` check (binutils is not there): it now reads the files'
version-needs tables itself when objdump is missing (`0d858b2e`; the same answer as `objdump -T`
on the release's five Linux binaries and three system ones). `check_install.py` itself ran in the
slim image on the same files (`--find-links`, `--source`, `--archives`; log
`release-4-check-install-linux.log`): 8 of 8 steps (smoke 52 checks, pytest 934 passed).

**The container rehearsal with these files** (`container_rehearsal.sh` with the
manylinux_2_28 wheels and the clone of `5ef26ea9`; log `release-4-container-rehearsal.log`):
every step passed: both images built (OCI archives 226,685,440 and 237,855,232 bytes), the 27
checks inside (golden and `test_smoke.py` 317 passed in each), the SIF images with `apptainer exec`
of the versions, `smoke.py --image` and the golden tests (305 passed in each), the push of the four
tags into a local registry with their indexes' digests, `apptainer pull` of each and its programs.

**Windows (the Windows helper: Windows 11 Education 25H2, build 26200, x64; uv 0.12.13; Visual
C++'s runtime installed system-wide, so its absence cannot be shown there: the PE check of the
release's files is the evidence, and it passed in dry run 2's two Windows test jobs).** The check
for after rc1's upload is `win_check.sh 0.1.0rc1` (PyPI, the GitHub Release's `.exe` archives,
the tag's tests). Proved now (logs `release-4-win-pypi-before.log`,
`release-4-win-gnu-trial-2.log`):
uv's Python 3.12.14 installed into the check's folder, the venv, pip upgraded; from PyPI the
install fails as it must today ("No matching distribution found for larmorx==0.1.0rc1"); with
the zig-built Windows files (`-FindLinks`, the source tree of `5ef26ea9` from GitHub's archive,
`-NoArchives`): `larmorx[all]` from the files 98 s, `pip check` clean, the smoke checks pass but the
3 C-runtime checks (these files import the Universal C Runtime's `api-ms-win-crt-*` DLLs, which
the check refuses, as it must for the release's files), **pytest 934 passed, 2 skipped on
Windows, the golden suite included**; the folder removed, nothing left. The first run left uv's
`python3.12.exe` link in the profile's `.local\bin` and a registry entry
(`HKCU\Software\Python\Astral\CPython3.12.14`) pointing into the removed folder: both removed
(they were the check's own, made minutes before); the wrapper now sets `UV_PYTHON_INSTALL_BIN=0`,
`UV_PYTHON_INSTALL_REGISTRY=0` and `UV_PYTHON_BIN_DIR` inside its folder and removes any registry
entry that points into it; after the second run neither exists.

**Dry run 2 (GitHub run 42934352316 at `5ef26ea9`, the first with the container jobs and the
test job on six platforms):** check, sdist, the six wheels, the six programs jobs (linux-aarch64's
fixed), sdist-install, both container jobs and the test jobs of linux-x64, linux-aarch64,
windows-x64 and windows-arm64 passed (so the Windows `+crt-static` dist builds pass their golden
tests and the PE check); one cause failed `test (macos-x64)` and `test (macos-arm64)`: "only the
system's libraries: ['@rpath/larmorx._core.abi3.so']". That is the extension's own install name
(its `LC_ID_DYLIB`, which `otool -L` prints first for a library), not a dependency: smoke.py now
leaves out the name `otool -D` gives and still refuses every other library outside `/usr/lib` and
`/System/Library` (`c02045bd`, with a test on `otool` outputs). Nothing was published (the
release-only jobs and assemble were skipped).

## Relay 5b: the documentation site (`site/`)

The site's pages for the first release are on the site's branch `release-pages` (from its main
`d9f67d1`: `5650433` the installation page and the home page's install notes, `b8edd27` the page
"Why Rust and Python", `af701b6` the installation page after its screenshots). Its shared files
(`site.yml`, `data/status.yml`) and the roadmap change when the site is published, after PyPI
has the version, through the release part's site handoff (`apply_handoff.py --site`). Relay 6
put relay 5b's helpers here (SPDX headers, ruff's form, the screenshot scripts' folders as
arguments); on the same inputs the three edit scripts write the same files as the ones relay 5b
ran, byte for byte.

| Helper | What it shows |
|---|---|
| `site/handoff_site.py <site checkout>` | the changes to the site's shared files: `site.yml`'s navigation (Install, after Status) and documentation list (Installation and Why Rust and Python, after Overview), `data/status.yml`'s planned item "Releases on PyPI" become "Release 0.1.0" |
| `site/handoff_roadmap.py <site checkout> <date>` | the roadmap at the publication: the section "The first release" says the release candidate is out, the release's row at the top of the completed steps, "Last updated", the front matter's description; the release part's handoff has the same changes dated 2026-11-12, this script takes the actual day |
| `site/edit_index.py <site checkout at d9f67d1>` | the home page's first edit (12 replacements: the "not yet on PyPI" lines and the cards that had gone stale); its result is site commit `5650433`'s `content/index.md`, byte for byte |
| `site/shots.sh <site checkout> <work>`, `site/shots2.sh <site checkout> <work>` | the screenshots (1440 and 390 px wide, light and dark, the site's `tools/review/shoot.mjs`) of the two builds served from `<work>/serve-before/larmorx_documentation` and `serve-after/larmorx_documentation` (each a build's `_site`): both new pages whole, the home page's changed parts before and after, the roadmap's release paragraph |
| `site/crop.py <png> <out> <y0> <y1>` | a close view of a full-page screenshot |
| `site/sumtest.ps1`, `site/sumtest2.ps1` (with `winps.sh`) | the installation page's PowerShell check of a download against `SHA256SUMS`, on the Windows helper with Windows PowerShell 5.1: True for the file, False after changing it (`sumtest2.ps1` with the file's line second in `SHA256SUMS`), the temporary folder removed |

**The checks (2026-11-11):** the site's `tools/build.py --main-repo <the part's worktree>`: 33
pages, 0 problems, for the branch and for a copy with the handoff applied; against larmorx's main
at `459a5a21` the branch has 2 problems, its links to `CHANGELOG.md` and `CITATION.cff`, which
reach main with this part's merge: so the larmorx branch is merged first, and the site published
only after PyPI has 0.1.0rc1 (its pages give the release's install lines, download addresses and
image names). The inventory (`../afni-volreg-original/site_inventory.py <before>/_site
<after>/_site`, every built page) against `d9f67d1`'s build: the home page 4,961 -> 5,060 words,
mri-hmc 27,823 -> 27,829, two new pages, the other 28 pages byte-identical; with the handoff
every page's words up (the footer's two new links), the roadmap one table row more, no count
down (exit 0). The page's checks of a download for Linux and for macOS (`sha256sum -c
--ignore-missing SHA256SUMS`, `shasum -a 256 -c --ignore-missing SHA256SUMS`) were run by hand,
both on this Linux machine, on a test `SHA256SUMS`. **Not tried:** the advice on the download marks of macOS and of a browser on
Windows (`xattr -d com.apple.quarantine`, `Unblock-File`): no Mac here, and the Windows helper's
runs are over SSH, without a browser's mark; worded with "may" on the page.
