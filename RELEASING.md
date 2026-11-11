# Releasing larmorx

The steps of a release, for the maintainer. A release publishes the three packages, `larmorx`
(Apache-2.0), `larmorx-gpl` (the GNU GPL version 3) and `larmorx-nc` (the FSL Licence,
non-commercial use only), at **one version**, written as PyPI writes it (PEP 440: `0.1.0rc1`,
`0.1.0`); its git tag adds a `v` (`v0.1.0rc1`). Only final releases and alpha, beta and
release-candidate pre-releases are possible (`X.Y.Z`, `X.Y.ZaN`, `X.Y.ZbN`, `X.Y.ZrcN`).

Everything happens in GitHub Actions, in the workflow
[`.github/workflows/release.yml`](.github/workflows/release.yml): a pushed tag `v<version>` is a
release; every other run is a **dry run**, which builds and tests every file and uploads
nothing. Nobody holds a PyPI token: PyPI trusts the workflow itself (trusted publishing), and
nothing is uploaded before the maintainer approves it in GitHub.

**PyPI never accepts a version twice**, not even after a deletion. A mistake found after an
upload is mended by the next version (for example `0.1.0rc2`), never by a second upload.

## What a release publishes

- **On PyPI**, 21 files: for each package six wheels (Linux x86-64 and aarch64 `manylinux_2_28`,
  macOS 11 Intel and Apple silicon, Windows x64 and arm64) and its source distribution.
- **The GitHub Release** of the tag: those 21 files, the 18 standalone archives (`larmorx`,
  `larmorx-nc` and `larmorx-gpl` for each of the six platforms) and `SHA256SUMS`, with the
  version's section of [CHANGELOG.md](CHANGELOG.md) as its text. A pre-release (`aN`, `bN`,
  `rcN`) is marked so and never becomes "latest".
- **Two container images**, linux/amd64 and linux/arm64, at `ghcr.io/karellopez/larmorx`:
  `<version>` (larmorx with `[prep]`, without the replica packages) and
  `<version>-all` (also larmorx-gpl and larmorx-nc). A final release also moves `latest` and
  `latest-all`.
- **Build provenance** for every file and both images (GitHub's attestations), and PyPI's own
  attestations of its uploads.

## Once, before the first release (done for 0.1.0rc1)

1. **PyPI** (the maintainer's account) → Account settings → Publishing → "Add a new pending
   publisher" (GitHub), three times, with owner `karellopez`, repository `larmorx`, workflow
   `release.yml` and one environment per package: project `larmorx` with environment `pypi`,
   `larmorx-gpl` with `pypi-gpl`, `larmorx-nc` with `pypi-nc`. (PyPI refuses two pending
   publishers with the same owner, repository, workflow and environment.)
2. **GitHub** → the repository's Settings → Environments → New environment, three times (`pypi`,
   `pypi-gpl`, `pypi-nc`): "Required reviewers" the maintainer; "Prevent self-review" off (the
   maintainer is the only reviewer and pushes the tag); "Deployment branches and tags" →
   "Selected branches and tags" → a tag rule `v*`; administrators may not bypass it.
3. After the first release's images are pushed: make the image package public (below, step 6).

## 1. The release commit

On a branch, as any change:

1. **The version**, everywhere at once (the three Cargo workspaces and their `Cargo.lock` files,
   the `[all]` extra's pins, `CITATION.cff`'s `version` and `date-released`):

   ```bash
   python -P scripts/release/set_version.py 0.1.0 --date 2026-12-01
   python -P scripts/release/set_version.py --check v0.1.0
   ```

2. **CHANGELOG.md:** a section headed `## [0.1.0] - 2026-12-01` (the date of `CITATION.cff`),
   in plain words: what is new since the last release, what was fixed, the known limitations.
   Its tools' statuses come from `validation/tools.toml`: copy the two marker lines of the
   statuses table from the last section into the new one, then

   ```bash
   python -P scripts/release/changelog.py 0.1.0 --write-statuses
   python -P scripts/release/changelog.py 0.1.0 --ref v0.1.0     # the release's text, printed
   ```

   Add the link definition `[0.1.0]: https://github.com/karellopez/larmorx/releases/tag/v0.1.0`
   at the end of the file. The README's status line names the newest release; update it too.
3. **The checks** the release's first job runs, here first:

   ```bash
   python -P scripts/release/licences.py --check       # the packages' licence expressions
   python -P scripts/release/third_party.py --check    # LICENSES/third-party-rust.txt (cargo fetch first)
   python -P scripts/release/pypi_readme.py --check --ref v0.1.0 README.md crates-nc/PYPI.md crates-gpl/PYPI.md
   ```

   A changed `Cargo.lock` needs `third_party.py --write`. The golden tests do not depend on the
   version (the harnesses write it as `<version>`), so a version change re-records nothing:
   `pytest tests/golden` and the companions' golden tests pass unchanged.
4. Merge it into `main` the usual way; CI must pass on all six platforms.

## 2. The dry run

A dry run builds and tests every file the release would publish, on the six platforms, and
uploads nothing. Start one on the commit to be released (it must be the exact commit the tag
will name):

```bash
git push origin +<commit>:refs/heads/release/dry-run
```

or in GitHub: Actions → Release → Run workflow, on `main`, with "Dry run" ticked. Pushing to
`release/dry-run` starts it (the `+` replaces an earlier dry run's commit); CI's own workflow
does not run on that branch. A dry run takes about 55 minutes and about 400 job-minutes (free for
a public repository). Follow it on the run's page, or with
`python3 scripts/orchestration/watch_ci.py <commit> release/dry-run`.

It passes when every job is green: `check`, `sdist`, `wheel` and `programs` on six platforms
(the companions' golden tests on the programs their wheels install), `test` on six platforms (the
golden tests on the files built for the release, the replicas' own tests, the smoke checks, the
Windows files' C-runtime check, `larmorx[all]` from the built files), `sdist-install` (the three
source distributions built from source), `container` on amd64 and arm64 (both images built and
checked inside), and `assemble` (the 39 files, their sizes, `SHA256SUMS`). The jobs after
`assemble` are skipped: they run only for a release. Two notices are normal on a dry run: the
version is not on PyPI yet and, when the commit is not on `main` yet, CI has not passed for it.

**Tag only a commit whose dry run passed and whose CI on `main` is green.**

## 3. The tag

First check the three environments once more (Settings → Environments: the reviewer, the tag
rule `v*`). Then:

```bash
git tag -a v0.1.0 -m "larmorx 0.1.0" <commit on main>
git push origin v0.1.0
```

The tag starts the release run. Its first job, `check`, stops it at once, before anything is
built, if the tag is not the packages' version (or `CITATION.cff`'s), the commit is not on `main`
or its CI did not pass, the version is already on PyPI, or `CHANGELOG.md` has no section for the
version or a statuses table that differs from `validation/tools.toml`. Then every file is built
and tested again, as in the dry run (about 55 minutes).

## 4. The approval

When `assemble` has passed, the run waits for the three deployments `pypi`, `pypi-gpl` and
`pypi-nc`. On the run's page: "Review deployments", tick **all three**, "Approve and deploy".

- Approve the three together: `larmorx`'s upload first waits (up to 20 minutes) until PyPI lists
  both companions with this run's files, and stops without uploading anything if one is
  missing, so that `larmorx[all]` never points at a version that is not there.
- "Reject" leaves everything as it was: nothing is public, and the version is still free.
- After the uploads, by themselves: the images are pushed to `ghcr.io`, the GitHub Release is
  made, and `verify-pypi` installs `larmorx[all]==<version>` from PyPI on the six platforms and
  runs the golden and smoke tests there.

## 5. When a job fails after the approval

- **"Re-run failed jobs"** on the run's page repeats the failed jobs and those after them, with
  the files this run built (they are the run's artifacts, kept for 14 days, the assembled set for
  30). `check` is not run
  again, so its "not on PyPI yet" test does not stop the re-run. Never "Re-run all jobs" once an
  upload has begun: it would build every file again while PyPI keeps the first ones, and its
  `check` job, finding the version on PyPI, stops it anyway.
- **A publish job that failed after uploading part of its files** (a network error, PyPI briefly
  unavailable): "Re-run failed jobs" finishes it. The re-run takes the same files and uploads only
  those PyPI does not have yet (the publish steps' `skip-existing`), so the version ends up
  complete, with this run's files. Yanking a version and releasing the next one is for a broken
  upload only (files that do not install or do not work, as `verify-pypi` would show), not for one
  that stopped halfway.
- **`container-push`** (after the PyPI uploads): "Re-run failed jobs"; the GitHub Release waits
  for it.
- **`github-release`**: if the release was made but its files did not all arrive, delete that
  GitHub Release (keep the tag) and "Re-run failed jobs".
- **`verify-pypi`**: it already asks PyPI 20 times, 30 seconds apart, while PyPI's index catches
  up. A real failure there means the published files are broken on that platform: yank the
  version on PyPI (each project's page → Manage → the release → Options → Yank), fix, and release
  the next one.
- **The tag's run is lost** (cancelled before the approval, or a runner failure early on): Actions
  → Release → Run workflow → "Use workflow from" the tag `v<version>`, with "Dry run" **not**
  ticked. Turning the dry run off is accepted only on a `v*` tag.
- A problem found in the files after the release: never move or re-push a tag; the next version
  carries the fix.

## 6. After the release

1. **Make the container images public, once** (the first release only: GitHub makes a new image
   package private): github.com/users/karellopez/packages/container/larmorx → Package settings →
   Change visibility → Public. Until then `apptainer pull` and `docker pull` without a login
   fail.
2. **Check the release as a user installs it**, on machines of one's own (nothing is installed
   outside one new folder):
   - Linux or macOS, with Python 3.12 or newer: download
     [`scripts/release/check_install.py`](scripts/release/check_install.py) from the tag and run
     `python -P check_install.py --version <version> --work <a new folder>`: a fresh venv,
     `pip install "larmorx[all]==<version>"` from PyPI, `pip check`, the smoke checks of the
     installed packages and of this platform's standalone archives from the GitHub Release, and
     the tests (`tests/python`, `tests/golden`) of the tag's source. In a clean Linux, for example:

     ```bash
     mkdir check && cd check
     curl -sSfLO https://raw.githubusercontent.com/karellopez/larmorx/v<version>/scripts/release/check_install.py
     apptainer exec --containall --bind "$PWD:/check" docker://python:3.12-slim-bookworm \
         python -P /check/check_install.py --version <version> --work /check/work
     ```

     `check_install.py` keeps every step's temporary files and pip's cache in its work folder
     (`<work>/tmp`, `<work>/pip-cache`): `--containall` keeps `/tmp` and the home folder in
     Apptainer's session space, 64 MiB by default (`sessiondir max size`), which pip's temporary
     files and cache outgrew at 0.1.0rc1's check ("No space left on device"; until the next
     release's script, rc1's needs `--env TMPDIR=/check/tmp` and a home folder bound from
     `check/`).

   - Windows (10 or 11, no administrator rights needed): download
     [`scripts/release/check_install_windows.ps1`](scripts/release/check_install_windows.ps1) and
     run `powershell -NoProfile -ExecutionPolicy Bypass -File check_install_windows.ps1 -Version
     <version>`. Everything (a Python of its own included) stays in
     `%USERPROFILE%\larmorx-check-<version>`, which it removes at the end. From the main
     development machine, `validation/investigations/release-0.1/win_check.sh <version>` runs it
     on the Windows helper over SSH and checks that nothing is left.
   - Each check prints its steps with their times; its last line is `CHECK-INSTALL ok` or
     `CHECK-INSTALL FAILED: <steps>`.
3. **The images:** `apptainer pull docker://ghcr.io/karellopez/larmorx:<version>`, then
   `apptainer exec larmorx_<version>.sif larmorx --version`; their provenance:
   `gh attestation verify oci://ghcr.io/karellopez/larmorx:<version> --repo karellopez/larmorx`.
4. **The files:** `sha256sum -c SHA256SUMS` in a folder with the release's files;
   `gh attestation verify <file> --repo karellopez/larmorx` for any one of them.
5. **The documentation site** ([larmorx_documentation](https://github.com/karellopez/larmorx_documentation)),
   only now: its pages give the version's `pip install` lines, download addresses and image
   names, which exist only after the upload, and it links this repository's `CHANGELOG.md` and
   `CITATION.cff` on `main`. The lines that name the release: the version at the top of
   `content/installation.md` (`{%- set v = "<version>" %}`, the one place of it on that page),
   the "Status" margin note of `content/index.md`, the release section (`{#release}`) and the
   table of completed steps of `content/roadmap.md`, and the release item in `data/status.yml`'s
   `planned` list. Build it with `python tools/build.py --main-repo <this checkout>` (0 problems)
   before pushing it.

## 7. From a release candidate to the final release

A release candidate (`0.1.0rc1`) is the rehearsal of its final release (`0.1.0`). When it has
installed and passed its checks on the six platforms (`verify-pypi`, and step 6 on Linux and
Windows), the final release repeats steps 1 to 6 with the final version: `set_version.py 0.1.0
--date <day>`, a `## [0.1.0] - <day>` section (what changed since the release candidate, the
statuses table made anew), the dry run, the tag `v0.1.0`, the approval. A final release marks its
GitHub Release "latest" and moves the images' `latest` and `latest-all`.

Besides the version, the final release changes the words that call the release a candidate: the
README's status line and its sentence on how pip treats a pre-release (a final release installs
with a plain `pip install larmorx`), and on the site (step 6.5) the installation page's first
paragraph and its pre-release sentence, the home page's "Status" note, and the roadmap's release
section; `data/status.yml`'s planned item "Release 0.1.0" is removed.

A problem found in a release candidate is fixed, and the next candidate (`0.1.0rc2`) released
the same way, before the final release.

## Good to know

- **Source builds on Windows link the C runtime dynamically.** The published Windows wheels and
  programs have Visual C++'s runtime built in (`.cargo/config.toml`'s `+crt-static`, checked in
  every release by `scripts/release/pe_imports.py`), but the source distributions do not carry
  `.cargo/config.toml`, so a wheel built from them on Windows needs the runtime that Microsoft's
  build tools install. Accepted for 0.1.0rc1; an `include` of that file in `[tool.maturin]`
  (and in `scripts/release/companion_sdist.py`) would make source builds the same.
- **Two messages show Cargo's form of a pre-release version** (`0.1.0-rc.1`): the AFNI programs'
  `++ implementation:` line and tedana's `dataset_description.json` written by `larmorx-gpl`. A
  final version is the same in both forms.
- **The release's own rehearsals**, the clean-machine checks made for 0.1.0rc1 and their results:
  [validation/investigations/release-0.1/](validation/investigations/release-0.1/README.md).
- **What the jobs cost:** a dry run about 400 job-minutes (about 55 minutes), a release about
  450 plus the approval; GitHub's standard runners are free for a public repository.
