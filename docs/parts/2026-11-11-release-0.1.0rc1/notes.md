# The first release (0.1.0rc1, then 0.1.0): relay notes

**Worktree for relay 2 onwards:** `<workspace>/larmorx/.claude/worktrees/agent-aad7eab002a56be61`
(branch `worktree-agent-aad7eab002a56be61`, from main `459a5a21`; relay 2a, agent of 2026-11-10).

## FROM THE ORCHESTRATOR
- (2026-11-10, during relay 1) Trusted-publishing names changed (BRIEF.md updated): PyPI refuses two
  pending publishers with the same owner, repository, workflow and environment for different project
  names. So: **one GitHub environment per package**: `pypi` for `larmorx`, `pypi-gpl` for
  `larmorx-gpl`, `pypi-nc` for `larmorx-nc` (workflow still `release.yml`, owner `karellopez`,
  repository `larmorx`). The workflow needs **one publish job per package**, each in its own
  environment and uploading only that package's files; the user approves the three in GitHub's one
  review dialog.
- (2026-11-10, after relay 1) **The user's side is done and checked:** PyPI's three pending publishers
  (the user's screenshot) and GitHub's three environments (public API: reviewer karellopez, admins'
  bypass off, one *tag* rule `v*` each).
- (2026-11-10, after relay 1) **The answers to D1-D19: the recommendations, with these precisions.**
  - D1 `dist` profile with fat LTO. D2, D4, D5 (two images), D6, D7, D10, D11, D12, D14, D15 (the
    config file), D16, D18: as recommended. D8: the `release/dry-run` branch trigger (the orchestrator
    pushes it).
  - D9: the compound licence expression for the three packages, kept equal to NOTICE by a test; the
    project's own licence stays Apache-2.0 (the user, 2026-10-19), and the README says so.
  - D3 and D19: **a separate relay, 2c, in the replica-side role** (a fresh agent; may open the
    companion programs' test harnesses `crates-gpl/larmorx-gpl-cli/tests/` and
    `crates-nc/larmorx-nc-cli/tests/` and what those harnesses call in the two CLI crates, never a
    tool's algorithm source, never any clean-room code): the version normalisation in the gpl
    harness (its 9 cases re-recorded once, from a clean commit) and the `LARMORX_GOLDEN_PROGRAM`
    option in both. Relays 2a and 2b do the Python harness's normalisation (4 cases) and stay out of
    those test files. Relay order: 2a, 2b, 2c, 3a, 3b, 4, 5a, 5b, 6.
  - D13: CITATION.cff without an e-mail address (public repository); the orchestrator asks the user
    for the name's spelling, an ORCID and an affiliation and puts the answer here before relay 5a.
  - D17 and Windows Sandbox: the orchestrator asks the user; until then the PE import-table check is
    the Windows evidence.
- (2026-11-10, the user's answers) **No Windows Sandbox now** ("We do not do the windows sandbox now,
  maybe in the future"): the PE import-table check is the Windows evidence for 0.1. **No Zenodo for
  now** (no DOI, no `.zenodo.json`). **CITATION.cff "similar" to the user's BIDS Manager fork's**
  (github.com/karellopez/BIDS-Manager, `CITATION.cff`, cff-version 1.2.0, type software, a `message`,
  an `abstract`, `keywords`, `license`, `repository-code`, `repository-artifact`, `version`,
  `date-released`). **One author only** (the user: "For now, Jochem out, and yes add my email"):
  given-names `Karel Mauricio`, family-names `López Vilaret`, email
  `karel.mauricio.lopez.vilaret@uni-oldenburg.de` (the user asked for it), affiliation `Applied
  Neurocognitive Psychology Lab, University of Oldenburg`, orcid
  `https://orcid.org/0009-0007-0504-5258`.
  For larmorx: license `Apache-2.0` (the project's own; the bundled licences are in NOTICE),
  repository-code `https://github.com/karellopez/larmorx`, repository-artifact
  `https://pypi.org/project/larmorx/`, url the documentation site, version and date-released set by
  the release (kept equal to the package version by the version script's check).
- (2026-11-10) **Relay 2b also measures speed**, `dist` against `release`, on a few calls of the
  benchmark suites (one thread and eight; e.g. antsRegistration's, 3dvolreg's, the resampler's, the
  B-spline fit's), boxed, and records the numbers: the user asked whether the small builds differ in
  completeness or speed.
- (2026-11-10, after relay 2a) **D20: yes, in relay 2b.** `scripts/release/third_party.py` writes
  `LICENSES/third-party-rust.txt` per package (each crate of the shipped programs' normal-dependency
  closure for the union of the six release targets (`cargo metadata --filter-platform` per target;
  dev and build dependencies out unless their code is linked in), its version, licence expression
  and the full text of its licence files from Cargo's registry sources); **committed**, listed in
  each package's `license-files` (so the sdists carry it and the wheels built from them too), with a
  test that the file's crate list equals the closure in `Cargo.lock` (no registry or network needed
  in the test). Pure Python, no new dependency. bzip2's binary-copy notice is the reason; MIT's and
  Apache-2.0's notices are covered by the same file.
- (2026-11-10, after relay 2a) **Relay 2c also switches `larmorx-gpl --version` and `larmorx-nc
  --version` to the PEP 440 form** (`0.1.0rc1`), as `larmorx --version` already is.
- (2026-11-10, after relay 2a) **PyPI's author field and e-mail: asked the user; leave pyproject's
  `authors` as it is until relay 5a**, which applies the answer written here (if none is written by
  then, 5a leaves it and lists it as open).
- (2026-11-10, after relay 2a) Every relay: run Python in the worktree only with
  `scripts/orchestration/env.sh`'s variables set (`LARMORX_WORKSPACE`; relay 2a's stray `oracles/`
  finding); check before each smoke run that `<workspace>/larmorx/.claude/worktrees/oracles` does
  not exist. The fallback in the 3dvolreg suite is a follow-up for the next-steps, not for this part.
- (2026-11-10, after relay 2c) **For relay 3a:**
  - **Standalone archives hold one binary, not two 25 MB copies:** the Unix `.tar.gz` holds
    `larmorx` and `lx` as a symlink to it; the Windows `.zip` holds `larmorx.exe` and a one-line
    `lx.cmd` (`@"%~dp0larmorx.exe" %*`). Each archive also holds the package's licence files (LICENSE,
    NOTICE, LICENSES/, its third-party-rust.txt) and a short README. Measure the archives' sizes.
    The companions' archives (`larmorx-nc`, `larmorx-gpl`) are one program each, with their own
    licences (the nc archive with what the FSL Licence requires, as its wheel).
  - **Relay 2c's two notes:** the companions' installed-program golden tests run in the job that
    has Rust (`LARMORX_GOLDEN_PROGRAM`, NOTES "Relay 2c: findings"); the Linux companion wheels need
    `--compatibility manylinux_2_28` (built from their sdists, as relay 2a advised).
  - **Accepted for rc1, equal from 0.1.0 on:** the sdists carry no `.cargo/config.toml`, so a source
    build on Windows links the C runtime dynamically (say so in RELEASING.md, relay 5a); the AFNI
    clean-room `implementation:` lines and tedana's description print Cargo's `0.1.0-rc.1`.
  - The first CI run on main after the merge is CI's first Windows run with `+crt-static`; the
    dry run (D8) on the `release/dry-run` branch is the proof before that: its Windows golden tests
    must pass on the dist builds.
  - `ci.yml` stays as it is (CLAUDE.md: "CI stays as it is"); the release workflow reuses its
    steps by copying, not by changing it. actionlint and zizmor (in the scratch folder, from PyPI
    or their releases) both clean, every action pinned by commit SHA, minimal permissions per job.

- (2026-11-11, after relay 5a) **Decided:** `skip-existing: true` in the three publish steps (relay 6;
  RELEASING.md's "yank and the next version" then only for a broken upload, not a partial one).
  The README's 5a text is reviewed by the orchestrator: keep it. Dry run 3 (93154c93) goes to relay
  6, not to 5b.
- (2026-11-11) **Dry run 3 PASSED** (run 42942601741 at 93154c93, 37 minutes): every job but the release-only ones green on the six platforms (wheels, programs, sdist-install, containers amd64 and arm64, test x6, assemble); only the expected notices (CHANGELOG before 5a, not on origin/main, not on PyPI yet). Nothing to fix from it.
- (2026-11-11, after relay 5a) **For relay 5b, also the site page "Why Rust and Python"**
  (`docs/next-steps.md`, its follow-up "The site: a page 'Why Rust and Python'": the user,
  2026-10-31, "Before the first release at the latest; written for researchers, in plain words",
  with the points listed there; facts from the projects' history and documentation and published
  papers only, never from their source). Do the installation page and the index's stale notes
  first; if the second page does not fit in the relay, commit at a clean boundary and leave it as
  relay 5c in NOTES.md.

## State
- **Relay 1 (inventory and design): DONE 2026-11-10, agent `a39d00af2f5ba5858`** (harness worktree
  `.claude/worktrees/agent-a39d00af2f5ba5858` at `459a5a21`; nothing committed; its temporary edits
  to the Cargo manifests and `crates-nc/pyproject.toml` were reverted, `git status` clean).
- **Relay 2a (packaging: versions, metadata, sdists): DONE 2026-11-10** in the worktree named at the
  top (branch `worktree-agent-aad7eab002a56be61`, from `459a5a21`). Commits: `61c7dec5` (the
  packaging), `42d88287` (PROVENANCE of the version code, CITATION.cff in the version script),
  `13bbc7e7` (golden.json: the 4 `/OSVersion` cases re-recorded, nothing else). Details in
  "Relay 2a: what was done" below.
- **Relay 2b (profile and runtime, the speed of `dist`, D20): DONE 2026-11-10** in the same worktree.
  Commits: `a2704b60` (the `dist` profile, `.cargo/config.toml`, `scripts/release/pe_imports.py`,
  `scripts/release/third_party.py` and the three `LICENSES/third-party-rust.txt`, their tests),
  `f190b951` (`validation/investigations/release-0.1/dist_speed.py`, the README's proofs, sizes and
  speed), `b36f7578` (a README sentence). Details in "Relay 2b: what was done" below.
- **Relay 2c (replica-side role: D3's gpl normalisation, D19, the companions' PEP 440 `--version`):
  DONE 2026-11-10** in the same worktree. Commits: `b5bc14d6` (build.rs, `--version`,
  `LARMORX_GOLDEN_PROGRAM` in both harnesses, the gpl `<version>` normalisation, companion_sdist's
  REQUIRED), `13013f21` (`gpl_golden_version_only.py`), `8ffd5448` (gpl golden.tsv re-recorded from
  13013f21: the 9 cases only), `d912a342` (`installed_golden.sh`, the README's relay 2c section).
  Details in "Relay 2c: what was done" below. **All three Rust workspaces are now fully green**:
  the branch can be merged at any relay boundary from here.
- **Relay 3a (the release workflow, all but the container): DONE 2026-11-11** in the same
  worktree. Commits: `46ed4455` (`.github/workflows/release.yml`, `scripts/release/{changelog,
  archives,smoke,assemble,pypi_state,ci_green}.py`, their tests, `workflow_rehearsal.sh`),
  `4b5c5d4d` (the `test_mri_replica.py` fix, `watch_ci.py`'s branch argument, the rehearsal's
  test-job steps, the README's relay 3a section), `d5421dac` (the installed programs' path
  lookups made safe for Git Bash on Windows; the second rehearsal in the README). The worktree is
  clean at `d5421dac`. Details in "Relay 3a: what was done" below;
  the orchestrator's dry-run plan in "Relay 3a: the dry run (for the orchestrator)".
- **Relay 3b (the container images): DONE 2026-11-11** in the same worktree. Commits: `feb3ad79`
  (`docker/Dockerfile` and `docker/README.txt`, `scripts/release/container.py`, release.yml's
  `container` x2 and `container-push`, `smoke.py --image`, the notes' container line, the tests,
  `rootless_docker.sh` and `container_rehearsal.sh`), `14f7cc2b` (the rehearsal's image names by
  digest only), `67d00142` (no `--only-binary :all:` for the dependencies: pybids 0.18.1 instead
  of 0.22.0), `5d1c328b` (the investigation README's relay 3b section), `79935496` (**dry run 1's
  two failures fixed**, below), `5ef26ea9` (the README: the last rehearsal, the fixes). The
  worktree is clean at `5ef26ea9`. Details in "Relay 3b: what was done" below.
- **The first dry run** (`d5421dac` on `release/dry-run`, pushed by the orchestrator during relay
  3b; run 42927413161; log `oracles/logs/release-dryrun-1-d5421dac.log`): check, sdist, wheel x6
  and programs on 5 platforms passed; **2 failures, both fixed in `79935496`**: (1)
  `sdist-install`: the smoke's glibc 2.28 check ran on the packages pip built there from the
  sdists, which link the runner's glibc (GLIBC_2.39, 2.34): new `smoke.py --source-build` (with
  `--installed`) leaves the binaries' platform checks out; `sdist-install` passes it; the
  published files keep the check (test, verify-pypi, a test asserts where the option is); shown
  here on relay 3a's host-built venv (3 failures of 21 without, 17 of 17 with). The pytest step of
  that job did not run (the smoke comes first in its step): dry run 2 shows it. (2) `programs
  (linux-aarch64)`: `cargo test` in the checkout read `rust-toolchain.toml` and rustup added
  rustfmt and clippy to the 1.99.0 dtolnay had installed: "detected conflict: 'bin/cargo-clippy'"
  on ubuntu-24.04-arm (why only there is not visible without the job log; ci.yml installs 1.99.0
  fresh from the toolchain file, with the components in one go). The golden-test step now sets
  `RUSTUP_TOOLCHAIN: ${{ env.RUST_TOOLCHAIN }}`, which rustup puts before the toolchain file
  (checked here: `rustup show active-toolchain` says "overridden by environment variable"), so no
  component is installed; the tests need neither. Not reproducible here (an arm runner image);
  dry run 2 is the proof. The test, assemble and later jobs were skipped (nothing published).
  **Dry run 2** (the orchestrator, from `5ef26ea9` or later): the first with the test job on six
  platforms and the two container jobs.
- **Relay 4 (clean machines): DONE 2026-11-11** in the same worktree. Commits: `c02045bd`
  (**dry run 2's one failure fixed**: `test (macos-x64)` and `test (macos-arm64)`, "only the
  system's libraries: ['@rpath/larmorx._core.abi3.so']": `otool -L` prints a library's own
  install name first; smoke.py leaves out what `otool -D` gives, refuses every other non-system
  library; a test), `ebf9dde6` (`tests/python/test_sdcflows_app.py`: two tests made `./work` in
  the folder pytest runs from; they run in `tmp_path` now), `0d858b2e` (smoke.py reads the glibc
  versions from the ELF file itself where objdump is missing), `2e5c21f4` (the clean-machine
  helpers, `scripts/release/check_install.py` and `check_install_windows.ps1`, the README's relay
  4 section, a test), `93154c93` (the README: dry run 2). The worktree is clean at `93154c93`. Details in "Relay 4: what
  was done" below.
- **Relay 5a (the documents in the repository): DONE 2026-11-11** in the same worktree. Commits:
  `ccd3325f` (README.md's installation and status, CHANGELOG.md, CITATION.cff, RELEASING.md;
  `changelog.py`'s statuses table and the notes' unwrapping; release.yml's dry-run warning text;
  the two PYPI.md macOS lines; tests), `c36f7b0d` (the account name and the host names out of the
  current tracked files), `96a050a1` (the plain image's licence in precise words). The worktree
  is clean at `96a050a1`. Details in "Relay 5a: what was done" below. **Dry run 3** (`93154c93`,
  the orchestrator's): its result had not arrived when relay 5a stopped.
- **Relay 5b (the site): DONE 2026-11-11** in the site clone `<scratch>/site`, branch
  `release-pages` (from the published main `d9f67d1`): `5650433` (the installation page, the home
  page's install notes and stale cards, mri-hmc's line), `b8edd27` (the page "Why Rust and Python",
  so no relay 5c), `af701b6` (the layout after the screenshots). The clone is clean at `af701b6`;
  nothing changed in the larmorx worktree. The site's shared files (site.yml, data/status.yml) and
  the roadmap through the handoff: "Relay 5b: findings". **Publish the site only after PyPI has
  0.1.0rc1, and after the larmorx branch (its CHANGELOG.md and CITATION.cff) is on main.**
- **Relay 6 (the last changes, the handoff, the final report): DONE 2026-11-11** in the same worktree
  and in the scratch folder. One commit, `8f2d2bd7` (`skip-existing` in the three publish steps with a
  test, RELEASING.md's partial-upload advice, the site's lines that name a release and its
  publication after PyPI, the words a final release changes; README's links to the site's
  installation page and "Why Rust and Python"; relay 5b's helpers in
  `validation/investigations/release-0.1/site/` with the README's section). The handoff:
  `<scratch>/HANDOFF.toml` (30 operations, at the merge), `<scratch>/HANDOFF-claude-machines.py`
  (CLAUDE.md's host names, at the merge), `<scratch>/HANDOFF-site.toml` (7 operations, at the
  site's publication after PyPI), `<scratch>/HANDOFF.md` (how, in which order, why, what is not
  scriptable). Details in "Relay 6: what was done" below. **The part is complete: next, the
  orchestrator's merge** (the final report's plan).
- **The part's agents, in order (for the clean-room audit):** relay 1 `a39d00af2f5ba5858`
  (inventory and design; its harness worktree removed, nothing committed); relay 2a
  `aad7eab002a56be61` (its harness worktree is the part's worktree); relay 2b `a289f4bfef580629b`;
  relay 2c `a542f9fcb3b589521` (the replica-side role: the companions' test harnesses and CLI
  crates); relay 3a `a136e13aaa1567c52`; relay 3b `adb5f6f3b49a3445d`; relay 4 `a43da72427cc28e44`;
  relay 5a `a674e7e1421ce3748`; relay 5b `a0e2cf0f342d12de9` (the site clone `<scratch>/site`);
  relay 6 `af288e71603a8c60e`. Relays 2b to 6 worked in relay 2a's worktree (5b only in the site
  clone). No relay edited clean-room code.
- **The exact final state of both branches (2026-11-11, relay 6):**
  - larmorx: branch `worktree-agent-aad7eab002a56be61` at **`8f2d2bd7`**
    (`8f2d2bd708f3fc80695bf48a02d05557644fbaeb`; from main `459a5a21`, which is also origin's main), worktree
    `<workspace>/larmorx/.claude/worktrees/agent-aad7eab002a56be61`,
    `git status` clean; 28 commits over main (`61c7dec5` .. `8f2d2bd7`); `release/dry-run` on
    origin at `93154c93` (dry run 3, passed); no tag. In the worktree (git-ignored):
    `python/larmorx/_core.abi3.so` relay 2c's release build, `crates-nc/target/release/larmorx-nc`
    and `crates-gpl/target/release/larmorx-gpl` release builds printing `0.1.0rc1`.
  - Site: the clone `<scratch>/site`, branch **`release-pages`** at **`af701b6`** (3 commits over
    the site's main `d9f67d1`, also origin's), clean, the no-reply identity set;
    `static/evidence/blobs/` (git-ignored, 1,424 files) populated.

## Relay 2a: what was done (2026-11-10)
- **Version 0.1.0rc1** (Cargo `0.1.0-rc.1`) in the three workspaces, their locks and the `[all]` pins
  (`larmorx-nc==0.1.0rc1`, `larmorx-gpl==0.1.0rc1`), by `scripts/release/set_version.py`
  (`<pep440> [--date D]`, `--check [tag]`, `--print`; only X.Y.Z, aN, bN, rcN; also CITATION.cff's
  `version` and `date-released` once relay 5a writes the file: `--check` then demands it equal).
  Verified with `cargo metadata --locked --offline` in the three workspaces.
- **`larmorx_core::VERSION` in PEP 440 form** (`crates/larmorx-core/build.rs` includes
  `src/pep440.rs`; unit tests; PROVENANCE section): `larmorx --version`, `_core.__version__`,
  `larmorx.__version__` and pip's version are `0.1.0rc1`. larmorx-io's HDF5 `/OSVersion` default
  uses it; `crates/larmorx-cli/tests/{binaries,afni_replica_cli}.rs` compare with it. **Still Cargo's
  form (not mine to edit):** `larmorx-nc --version` and `larmorx-gpl --version` print
  `0.1.0-rc.1` (replica `src`: relay 2c may switch them to PEP 440 if the orchestrator wants, D2),
  the clean-room AFNI crate's `implementation:` lines (a clean-room agent), the AFNI replicas'
  (`env!("CARGO_PKG_VERSION")`). Identical from 0.1.0 on.
- **Metadata** of the three: Development Status 3 - Alpha, `authors = [{ name = "Karel Lopez
  Vilaret" }]` (NOTICE's form, no e-mail), Homepage and Documentation (the site), Repository,
  Issues, Changelog (`blob/main/CHANGELOG.md`, written by 5a), Source for the companions;
  classifiers Environment Console, Python 3.12/3.13/3.14, CPython, Image Processing; summaries of what
  exists today; `rust-version = "1.99"` in the three `[workspace.package]` and taken by the four
  leaf crates (larmorx-py, larmorx-cli, larmorx-nc-cli, larmorx-gpl-cli); larmorx-gpl-cli's
  licence field `GPL-3.0-only`. `twine check --strict` passes on all six files.
- **Licence expressions (D9)**, derived and checked by `scripts/release/licences.py` (`--check`,
  `--write`; needs cargo): own licence first, then NOTICE's licence phrases (sections "never
  distributed" left out; tedana's LGPL converted), the SPDX headers of the Apache crates in the
  program's closure (`crates/*/src`; the AFNI replicas' crate by check_spdx's allowed expressions,
  its files not opened; larmorx-hdf5 named HDF5), the path crates' Cargo licences, and the
  crates.io crates (normal deps, no proc-macros, all targets) none of whose choices is named:
  - larmorx: `Apache-2.0 AND BSD-2-Clause AND BSD-3-Clause AND bzip2-1.0.6 AND CC-BY-4.0 AND HDF5
    AND MIT AND MPL-2.0 AND PSF-2.0 AND SunPro AND Zlib`
  - larmorx-nc: `LicenseRef-FSL AND Apache-2.0 AND BSD-2-Clause AND BSD-3-Clause AND bzip2-1.0.6 AND
    HPND-sell-variant AND MIT AND MPL-2.0 AND PSF-2.0 AND SunPro AND Zlib` (HPND-sell-variant: the
    SGI STL's notice, checked against SPDX's template: its optional clauses absent)
  - larmorx-gpl: `GPL-3.0-only AND Apache-2.0 AND BSD-3-Clause AND HDF5 AND MIT AND MPL-2.0 AND
    PSF-2.0 AND SunPro AND Zlib`
  `packaging.licenses` accepts all three (canonical form unchanged). README's "Licence" (5a) should
  say: larmorx's own licence is Apache-2.0; the expression lists the bundled works' licences.
- **PyPI descriptions (D10):** `scripts/release/pypi_readme.py --ref <tag|sha> FILE...` (in place;
  `--check`): relative links and anchors -> GitHub at the ref (`blob`/`tree`; images
  raw.githubusercontent), `blob/main` links pinned to the ref, code left alone, a missing target an
  error. Main README: 52 lines rewritten, 0 relative links left; rendered with readme_renderer (GFM)
  without error. `crates-nc/PYPI.md` ("Non-commercial use only" first; the tools and FSL package
  versions; that installing it makes `implementation="auto"` run the replicas; how to avoid that;
  the licence and source) and `crates-gpl/PYPI.md`; `readme = "PYPI.md"`; the directory READMEs'
  "Not on PyPI" paragraphs updated (and crates-nc/README.md's stale "BET has no clean-room original
  yet").
- **larmorprepx (D14):** `larmorprepx` (no arguments) prints "larmorprepx is not ready yet in larmorx
  <version>: its field-map stages exist as Python functions (...); the anatomical (FreeSurfer) and
  BOLD stages, the confounds and the final validation come in later releases (<next-steps URL>)",
  then the `[prep]` hint if modules are missing; exit 1; `--help` says the same; a smoke test.
- **Golden (D3, Python harness only):** `tests/golden/registry.py`'s `_h5_structure` records
  `/OSVersion` as `<version>` when it is the running version. Exactly the 4 predicted cases changed,
  and by that string only (`validation/investigations/release-0.1/golden_version_only.py`: with
  `<version>` written back as `0.0.1` the fingerprints equal the 0.0.1 record, 4 of 4). Re-recorded
  from a clean commit by `record_golden.py` (`13bbc7e7`).
- **Relay 1's helpers committed** in `validation/investigations/release-0.1/` (with relay 2a's
  `golden_version_only.py`, `sdists-install.sh`, `wheelinfo.py`; a README; this machine's paths
  replaced by arguments and env.sh's variables, nothing of the account name in them).
- **compare_records.py (D4):** `strip()` replaces `larmorx(-nc|-gpl)? <version>` (PEP 440 or Cargo
  form) and `package='larmorx...', version='...'` by `<version>`.
- **Companion sdists:** `scripts/release/companion_sdist.py {nc,gpl} --out DIR [--maturin PATH]`:
  stages the tracked files (`git ls-files`, working-tree contents, so a description rewritten for
  PyPI is the one packaged) of the root manifests, `crates/`, licences and the companion's
  directory, the companion's pyproject at the root with prefixed paths, `maturin sdist` there, then
  checks completeness (licences, notices, every program crate's `Cargo.toml` and `src/`, FSL's
  original source and the wheel data for nc; nothing of `crates-gpl/tools/` for gpl).
  `crates-nc/pyproject.toml` got `include = [{ path = "wheel-data/**/*", format = "sdist" }]`.
- **Measured (Linux x86-64, `oracles/logs/release-2a-sdists.log`):** sdists larmorx 3.21 MB, nc 3.26
  MB (393 entries; the wheel data now in it), gpl 0.89 MB (188 entries); `twine check --strict` on
  the sdists and the wheels built from them: PASSED. Wheels built by pip from the three sdists in a
  fresh venv of the system's CPython 3.12.16 (build isolation, maturin from PyPI, rustc 1.99): 21 s
  with cached dependencies; larmorx 15.2 MB, nc 6.39 MB (the FSL data in it: relay 1's sdist-built
  4.96), gpl 1.27 MB. Installed: the five programs' versions right (nc and gpl in Cargo's form, above),
  `share/larmorx-nc/fsl-data` present, `pytest tests/python tests/golden` without FSL: 879 passed,
  53 skipped, only the 4 golden cases before their re-recording.
- **Gates (at 61c7dec5 + the follow-up edits):** Rust root OK, crates-nc OK, crates-gpl: everything
  but `larmorx-gpl-cli --test golden`, which fails exactly the 9 expected `dataset_description.json`
  cases (tedana's `larmorx-gpl <version>` line; relay 2c, D3) — CI's `gpl` jobs would fail the same
  way until 2c: **merge only after 2c**. Python: gates-python.sh --no-build at 13bbc7e7: 1337 passed, 12 skipped; 7 failed, all in tests/python/test_smoke.py (the shared venv's metadata says 0.0.1, below: expected in a worktree; they pass on the installed packages); the same 7 without FSLDIR and LARMORX_NC_BIN (917 passed); tests/golden all pass; the collection without larmorx_validation passes. ruff, ruff format, check_spdx, tables
  --check: pass (set_version --check v0.1.0rc1, licences --check and pypi_readme --check too). `validation/tests/test_release.py`: 23 passed.
- **Parity smoke tier** (record_golden's, 16 workers): first run at 61c7dec5 1 failure of 2731:
  `test_sdcflows_estimation_parity[contract-audit]` (field maps B's relay-22 audit: every source
  file the branch added or changed under `crates/` and `python/larmorx` must be named in a crate's
  PROVENANCE.md: `larmorx-core/build.rs`, `src/pep440.rs` and the two larmorx-cli test files);
  fixed by larmorx-core's PROVENANCE section. The second run (19:20) failed 162 AFNI cases: not a
  larmorx change but a stray `larmorx/.claude/worktrees/oracles/` the first run had made (below,
  findings); moved to `<workspace>/archive/2026-11-10-release-2a-stray-oracles/` (with a README),
  the wrapper fixed, the failing cases then pass. The third run: the third (LARMORX_WORKSPACE set) and fourth runs were stopped: the second run was still going beside them (its serial memory tests re-made the stray folder at 19:51, also archived, as oracles-2) and the worktree had only debug builds of larmorx-nc and larmorx-gpl (the suites take them when no release build exists: topup's and t2smap's memory tests ran 3 times slower); the fifth run, after build-nc.sh and build-gpl.sh, alone, with LARMORX_ORACLE_CACHE named: 2968 passed, 0 failed (16 workers 320 s, then serial 1316 s), marker parity-smoke-42d88287...; golden.json recorded from it (13bbc7e7: 6 outputs of the 4 cases, each 4 bytes longer, nothing else).

## Relay 2a: findings and follow-ups for the orchestrator
- **For relay 2b's smoke tier in this worktree:** build the companions' release programs first
  (`scripts/orchestration/build-nc.sh <worktree>`, `build-gpl.sh <worktree>`; with 2b's `dist`
  profile the suites still look in `target/release`: build `dist` into place or set
  `LARMORX_NC_BIN`/`LARMORX_GPL_BIN`), run Python through `<scratch>/r2a/py.sh` (env.sh,
  `LARMORX_ORACLE_CACHE`), one smoke run at a time; and `git status` clean before
  `record_golden.py` (it refuses otherwise). The release extension of the main wheel goes into
  `python/larmorx/_core.abi3.so` (`<scratch>/r2a/build-main.sh`, cache `<scratch>/inv/target-plain`).
- **Run Python in a worktree only with `scripts/orchestration/env.sh` sourced (`LARMORX_WORKSPACE`).**
  Without it, `parity/afni_volreg_original.workspace()` falls back to `Path(__file__).parents[5]`,
  the worktree's parent (`larmorx/.claude/worktrees`), and creates `oracles/inputs/` there; from then
  on `oracle_cache.store.workspace()` (the first parent holding `oracles/`) and every suite's
  `<workspace>/oracles/...` find that folder: an empty cache, no AFNI oracle builds (AFNI then comes
  from elsewhere), 162 AFNI cases failing (relay 2a's second smoke run). The folder (774 MB) is in
  `<workspace>/archive/2026-11-10-release-2a-stray-oracles/` (its cache entries record wrong runs:
  never merge them into `oracles/cache`; deletable when the user agrees). Follow-up for whoever owns
  the suite: the fallback should be `oracle_cache.store.workspace()` (or refuse), never a fixed
  parent count. Relay 2a's `<scratch>/r2a/py.sh` now sources env.sh.
- **In the agents' worktree setup `tests/python/test_smoke.py`'s version tests fail** (the shared
  venv's `larmorx` metadata is the main checkout's editable install, 0.0.1, while the worktree's
  extension says 0.1.0rc1): expected until the orchestrator's `maturin develop` in the main checkout
  after the merge; they pass on the installed packages (above) and will in CI.
- **The companion wheels built from their sdists hold their licence files under
  `.dist-info/licenses/crates-nc/...` (`crates-gpl/...`)** (their paths follow the staged
  pyproject); built in their directories they sit at `.dist-info/licenses/...`. Relay 3a: build the
  published companion wheels from their sdists (relay 1's advice), so wheel and source build agree.
- **D20 (new, for the orchestrator): the licence texts of the bundled crates.io crates are not in the
  wheels** (only the SBOM's identifiers): MIT's and Apache-2.0's notice conditions, and
  bzip2-1.0.6's "Redistributions in binary form must reproduce the above copyright notice" (libbz2-rs-
  sys, in larmorx and larmorx-nc), Zlib (zlib-rs, source distributions only). Recommendation: a
  `scripts/release/third_party.py` that writes `LICENSES/third-party-rust.txt` (each crate of the
  program's closure, its licence expression and the text of its licence files from Cargo's registry
  cache) before the release build, listed in each package's `license-files` (relay 2b or 3a; or
  committed, with a test that it matches Cargo.lock).
- **The contract audit's `GLUE` misses integration tests** (`crates/*/tests/*.rs` must be named in a
  PROVENANCE.md, though its docstring excludes tests): relay 2a named the two larmorx-cli tests in
  larmorx-core's PROVENANCE section instead of changing field maps B's audit; a one-line fix
  (`|/tests/` in GLUE) is for whoever owns that audit.
- **The author's name:** pyproject says "Karel Lopez Vilaret" (NOTICE's copyright line);
  CITATION.cff (5a) will say given-names "Karel Mauricio", family-names "López Vilaret". Whether
  PyPI's `authors` should use the full form (and whether the e-mail address goes into PyPI's
  metadata too; it would be public there) is the user's call; not changed.
- **This shell's default PATH has FSL's `python3` first** (`~/fsl/bin`): use the venv's Python
  explicitly (relay 2a's first runs of the stdlib-only release scripts used it; no effect). The
  harness refuses many compound commands: write scripts (relay 2a's are in `<scratch>/r2a/`:
  `py.sh` (the venv's Python with the worktree first), `build-main.sh`, `gates-rust.sh`,
  `gpl-tests.sh`, `sdists.sh`, `record.sh`).

## Relay 2b: what was done (2026-11-10)
- **`[profile.dist]`** in `Cargo.toml`, `crates-nc/Cargo.toml`, `crates-gpl/Cargo.toml`
  (`inherits = "release"`, `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`; a comment says
  why `release` stays the everyday profile). No `[profile.release]` exists anywhere (Cargo's
  defaults), so CI, verify-merge and the agents' builds are unchanged.
- **`.cargo/config.toml`** (repository root, one file for the three workspaces: Cargo searches from
  the working directory upwards): `[target.'cfg(all(windows, target_env = "msvc"))'] rustflags =
  ["-C", "target-feature=+crt-static"]`. Checked with `cargo check --target
  aarch64-pc-windows-msvc -v` started in `crates-nc/` (the flag on the target's crates, not on the
  host's build scripts) and on a Linux build (0 occurrences). It applies to CI's Windows builds too
  (D15): the next CI run of main after the merge is the first Windows evidence (golden tests on
  x64 and arm64). **Not in the sdists**: maturin's main sdist leaves `.cargo/` out, the companions'
  staging copies only manifests, `crates/`, licences and their directory; a source build on Windows
  (it has MSVC) then links the C runtime dynamically. Recommendation: leave it so (an `include` in
  `[tool.maturin]` would add it, if the orchestrator wants source builds to match).
- **`scripts/release/pe_imports.py`** `[--machine x86_64|aarch64] [--quiet] PATH...` (files,
  directories, wheels, zips): the import and delay-load tables of every `.exe`/`.pyd`/`.dll`
  (PE32 and PE32+, stdlib only); refuses `vcruntime*`, `msvcp*`, `msvcr*`, `concrt*`, `vccorlib*`,
  `vcomp*`, `ucrtbase*`, `api-ms-win-crt-*`; exit 0/1/2; last line `PE-IMPORTS ok|FAILED`. Tests on
  synthetic images (x86-64, i386, arm64; both tables, the old delay form; in a wheel; the machine
  check; broken and missing files). Real files: setuptools' launchers (`cli-64.exe`,
  `gui-arm64.exe`, `cli-32.exe`) are refused (VCRUNTIME140 and the api-ms-win-crt DLLs); ruff
  0.16.10's `ruff.exe` from PyPI (Rust with `+crt-static`) passes. **For relay 3a:** run it in the
  Windows build and test jobs on every wheel and standalone archive (`--machine x86_64` /
  `aarch64`).
- **D20: `scripts/release/third_party.py`** (`--write`, `--check`; cargo needed, offline) and the
  three committed files `LICENSES/third-party-rust.txt` (larmorx: programs larmorx-py and
  larmorx-cli; 36 crates compiled in, 9 procedural-macro crates, 35 in Cargo.lock's graph but not
  built in; 38 different licence texts; 140,046 bytes), `crates-nc/LICENSES/third-party-rust.txt`
  (20 / 5 / 24; 60,824 bytes), `crates-gpl/LICENSES/third-party-rust.txt` (19 / 5 / 22; 82,857
  bytes). The closure: `cargo tree -e normal,no-proc-macro --target <t>` per program and target
  (features resolved as that crate's build does), the macros from `-e normal`; section 3's reasons
  from `--target all` and `-e normal,build` (other platforms; build scripts; tests or features not
  enabled). Each licence file of a crate's published source (LICENSE*, LICENCE*, COPYING*,
  COPYRIGHT*, NOTICE*, UNLICENSE*, the manifest's license-file), each text once. Only r-efi (UEFI,
  not built) has no licence file. In `license-files`: main and nc by their `LICENSES/*`, gpl added
  explicitly (and to `companion_sdist.py`'s REQUIRED); checked in the three `dist` wheels and the
  three sdists (PKG-INFO's License-File). Tests (`validation/tests/test_release.py`, now 30): sections
  1-3 equal the registry crates Cargo.lock's graph reaches from the programs (no cargo needed),
  disjoint, every text named exists; each package ships its file (bzip2's notice present where
  libbz2-rs-sys is linked); `--check` up to date (skipped without cargo). **Whenever a Cargo.lock
  changes:** `python -P scripts/release/third_party.py --write` (the test fails until then). The
  set agrees with `licences.py`'s closure (the same crates add identifiers: libbz2-rs-sys, zlib-rs);
  unicode-ident's Unicode-3.0 is only in section 2 (a macro's dependency) and, as licences.py says,
  not in the expressions.
- **Sizes** (Linux x86-64, 0.1.0rc1, `release` -> `dist`): larmorx wheel 15.24 -> 13.95 MB
  (`_core.abi3.so` 78.1 -> 41.2 MB); larmorx-nc wheel 6.39 -> 6.20 MB (program 8.63 -> 6.79);
  larmorx-gpl wheel 1.29 -> 1.06 MB (program 3.35 -> 2.00); standalone `larmorx` 48.4 -> 25.1 MB
  (`lx` the same; gzip -9 of one 9.96 MB, both in one tar.gz 20.1 MB: relay 3a may prefer a format
  with a long window, `.tar.xz`, or one binary). Build times at 28 threads, dependencies cached:
  main extension 340 s (`release` 95 s), the two standalone binaries 179 s (43), nc 46 s, gpl 19 s.
- **The same bits under `dist`** (log `oracles/logs/release-2b-golden.log`): Python golden 305/305
  on the `dist` extension; AFNI replicas' golden and larmorx-nc's golden pass (`cargo test
  --profile dist`); larmorx-gpl's fails only its 9 known version cases in both profiles, and the
  outputs the harness saved for them are byte for byte the same under `dist` and `release` (109 of
  118 files; the 9 `stderr.txt` differ only in the temporary folder's name and progress-bar
  speeds). **The parity smoke tier on `dist` builds** (`oracles/logs/release-2b-smoke.log`; the dist
  extension, the dist nc and gpl programs swapped into `crates-*/target/release/` and restored):
  **2,968 passed, 0 failed** (16 workers 314 s, serial 1,214 s). No golden checksum changed; nothing
  recorded.
- **Speed, `dist` against `release`** (`dist_speed.py`, boxed 24 GiB, quiet machine, 4 rounds ABBA,
  3 repeats; `oracles/logs/release-2b-speed.log`; table in the investigation README): dist/release
  = antsRegistration (ds005003's coregister) 0.930 at 1 thread, 0.884 at 8; N4 (ds000005 T1w)
  0.893, 0.958; 3dvolreg (ds000005 run 1; `auto` ran the replica, one thread) 1.001, 1.002; the
  resampler (ds000005 run 1, motion and field map) 1.007, 1.029; the B-spline fit (ds005454) 0.946,
  0.980. **Outputs identical in every run of both builds.** So the small builds are complete (the
  same crates and features) and as fast or faster (up to 12 %); 3dvolreg and the resampler within
  the runs' spread (about 3 %).
- **Gates** (at `b36f7578`; `oracles/logs/release-2b-gates.log`): Rust root OK, crates-nc OK,
  crates-gpl everything but the golden test's 9 known cases (`--no-fail-fast`: every other target
  passes); Python 1,344 passed, 12 skipped, 7 failed: the same 7 `test_smoke.py` version tests as
  relay 2a (the shared venv's metadata says 0.0.1 in a worktree); without FSLDIR 917 passed, the
  same 7; ruff, ruff format, check_spdx, tables --check, set_version --check, licences --check,
  third_party --check, pypi_readme --check: pass.
- **The worktree's state:** clean at `b36f7578`. `python/larmorx/_core.abi3.so` (git-ignored) is the
  `dist` build (sha256 199bc87d...); `crates-nc/target/release/larmorx-nc` and
  `crates-gpl/target/release/larmorx-gpl` are the release builds again (restored), the dist ones
  in `crates-*/target/dist/`. Main extension builds: `<scratch>/inv/target-plain/{release,dist}`;
  package trees for the speed runs `<scratch>/r2b/trees/{release,dist}`; wheels
  `<scratch>/r2b/wheel-*`; sdists `<scratch>/r2b/sdist/`; relay 2b's runners in `<scratch>/r2b/`
  (`build.sh`, `golden.sh`, `gpl-compare.sh`, `smoke.sh`, `speed.sh`, `gates.sh`; NOTES before 2b
  in `NOTES-before-2b.md`).

## Relay 2b: findings and follow-ups for the orchestrator
- **HANDOFF material (relay 6):** next-steps' release items "Windows' C runtime" (done: one
  `.cargo/config.toml` at the root serves the three workspaces, not one per workspace as the item
  says; evidence `scripts/release/pe_imports.py` and, after the merge, CI's Windows golden tests) and
  "Size" (done by the `dist` profile for the released files, not by changing `release`; the numbers
  above); `docs/licensing.md` could name `LICENSES/third-party-rust.txt` beside the SBOM (the
  third-party crates' licence texts in every wheel and sdist).
- **For relay 3a:** build with `--profile dist` (maturin and cargo); run `pe_imports.py` on the
  Windows files; run `third_party.py --check` in the `check` job after `cargo fetch --locked` (it
  needs the registry's sources, not the network beyond the fetch); the dist build of the main
  extension is single-threaded LTO for about 4-6 minutes per platform (340 s here).
- **CI's Windows builds change with this merge** (`+crt-static` from `.cargo/config.toml`, D15):
  the extension and the test programs link the C runtime statically; the golden tests there are
  the proof that the bits stay the same; nothing else in CI changes (Linux and macOS untouched).
- **The 3dvolreg benchmark row ran the replica** (the default `implementation="auto"` with the
  AFNI replicas in the main package): no threaded path, so its 8-thread row repeats the 1-thread
  one. Enough for dist against release; a benchmark of the original's threads is in
  `docs/benchmarks/afni-volreg.md`.
- **fMRIPrep's SyN distortion correction call** (ds005003-syn `fmap_preproc_wf/wf_auto_00000/syn`)
  ran over 6 minutes at 8 threads in larmorx (one run, stopped): too long for this comparison, so
  the BOLD-to-field-map coregistration (0.2-0.8 s) stands for antsRegistration; a measured SyN
  time belongs in the final validation's benchmarks.

## Relay 2c: what was done (2026-11-10, replica-side role)
- **The companions' `--version` in PEP 440 form:** `larmorx-nc --version` and `larmorx-gpl
  --version` print `larmorx-nc 0.1.0rc1` / `larmorx-gpl 0.1.0rc1` (and their `--help`'s first
  line the same): a `build.rs` in each CLI crate (`crates-*/larmorx-*-cli/build.rs`) converts
  `CARGO_PKG_VERSION` with `include!("../../crates/larmorx-core/src/pep440.rs")` (relay 2a's code,
  unchanged) into `LARMORX_NC_PEP440_VERSION` / `LARMORX_GPL_PEP440_VERSION`; an unsupported version
  form fails the build, as for larmorx-core. Unit tests check the form independently (Cargo's
  `-alpha.N`/`-beta.N`/`-rc.N` written `aN`/`bN`/`rcN`). `companion_sdist.py`'s REQUIRED lists the
  two build scripts (both sdists carry them and `crates/larmorx-core/src/pep440.rs`). No golden
  output carries the program's `--version`, so no checksum changed by it. **Still Cargo's form:**
  tedana's `dataset_description.json` (`"Python": "larmorx-gpl 0.1.0-rc.1 (...)"`, the replica's
  `src`, not opened): the harness normalises both forms; identical from 0.1.0 on.
- **D3, the gpl harness:** `tests/golden.rs`'s `without_version` writes `"Python": "larmorx-gpl
  <v> ` as `"Python": "larmorx-gpl <version> ` for `<v>` = this crate's version in Cargo's or
  PEP 440's form only (another version still differs); a unit test. The 9 cases re-recorded once
  from the clean commit `13013f21` (`LARMORX_GOLDEN_RECORD=1 cargo test --locked -p
  larmorx-gpl-cli --test golden`; log `oracles/logs/release-2c-record.log`): 203 outputs, exactly
  the 9 `dataset_description.json` lines changed (and the "Recorded from" line).
  `validation/investigations/release-0.1/gpl_golden_version_only.py`: each saved output (the
  harness's mismatch folder from the run just before) with the version written back as `0.0.1`
  equals the old record and with `<version>` the new one, 9 of 9 (`GPL-VERSION-ONLY ok`).
- **D19, `LARMORX_GOLDEN_PROGRAM`** in both harnesses (`crates-nc/larmorx-nc-cli/tests/golden.rs`,
  `crates-gpl/larmorx-gpl-cli/tests/golden.rs`): unset or empty, cargo's build
  (`CARGO_BIN_EXE_...`) as before; else a path (relative ones from the crate's directory, where
  cargo runs tests) or a bare name looked up on `PATH` (`.exe` tried on Windows), made absolute
  (`std::path::absolute`), and its `--version` must print exactly `larmorx-nc <VERSION>` /
  `larmorx-gpl <VERSION>` of the checkout, else the test fails naming what it printed. Every case
  (and nc's `every_tool_has_a_golden_case`, which reads `--help`) then runs that program; with
  `--nocapture` the harness prints `golden: running <path> (<version>), not cargo's build`; a
  mismatch's message names the program. **Proof** (`installed_golden.sh`, boxed 32 GiB; logs
  `oracles/logs/release-2c-d19.log`, `release-2c-installed-golden.log`): the two sdists made by
  `companion_sdist.py` (complete), a wheel built from each by pip with `MATURIN_PEP517_ARGS="--profile
  dist --locked"` (stripped programs; 61 s with the LTO seeds), installed with `--no-index` in a
  fresh venv of `/usr/bin/python3.12`: the programs print `0.1.0rc1`; **larmorx-nc's golden test
  passes on the installed program** (2 of 2 tests, every case; 21 s against about 60 s on cargo's
  debug build; the installed program finds its own `share/larmorx-nc/fsl-data`, which changes no
  case), **larmorx-gpl's passes** by path and by its name on `PATH`; negative controls fail as they
  must: a missing program (both harnesses), relay 2b's larmorx-gpl that prints `0.1.0-rc.1`.
- **Gates at `d912a342`** (`oracles/logs/release-2c-gates.log`, boxed 64 GiB): the three Rust
  workspaces **all OK** (fmt, clippy, test: crates-gpl's golden test passes now); Python 1,344
  passed, 12 skipped, 7 failed: the same 7 `test_smoke.py` version tests as relays 2a and 2b (the
  shared venv's metadata says 0.0.1 in a worktree); without FSLDIR 917 passed, the same 7; the
  collection without larmorx_validation, ruff, ruff format, check_spdx, tables --check,
  set_version --check v0.1.0rc1, licences --check, third_party --check, pypi_readme --check: pass.
- **Parity smoke tier at `d912a342`**, alone, boxed 64 GiB, env.sh's variables and
  `LARMORX_ORACLE_CACHE` set, after `build-nc.sh` and `build-gpl.sh` (release programs, `0.1.0rc1`)
  and the release main extension (`<scratch>/inv/target-plain`, now in `python/larmorx`), no stray
  `larmorx/.claude/worktrees/oracles` before or after (`oracles/logs/release-2c-smoke.log`): **2,968
  passed, 0 failed** (16 workers 321 s, serial 1,318 s); marker
  `parity-smoke-d912a342b51eb03c314f438d6c9109712d80e9ca.json`. The worktree is clean at `d912a342`.
- Relay 2c's runners in `<scratch>/r2c/` (`quick.sh`, `record.sh`, `d19.sh`, `gates.sh`,
  `smoke.sh`; the saved mismatch outputs `mism-13013f21/`, the old record `golden-old.tsv`; the D19
  venvs `venv-d19/`, `installed/`); NOTES before 2c in `r2c/NOTES-before-2c.md`.

## Relay 2c: findings and follow-ups for the orchestrator
- **For relay 3a:** the companions' golden tests on the installed programs are cargo tests: run
  them where Rust is (e.g. the `build` job, after the wheels are installed into a venv there), as
  `LARMORX_GOLDEN_PROGRAM=<venv>/bin/larmorx-nc cargo test --profile dist --locked -p
  larmorx-nc-cli --test golden` in `crates-nc` (`<venv>\Scripts\larmorx-nc.exe` on Windows;
  larmorx-gpl the same in `crates-gpl`); the harness still compiles cargo's own program
  (`CARGO_BIN_EXE_*` is always built) but runs the named one and checks its `--version` against the
  checkout. The `test` job (no Rust) keeps the version checks and `LARMORX_NC_BIN`/`LARMORX_GPL_BIN`
  through larmorx.
- **For relay 3a:** wheels pip builds from the companions' sdists are tagged `linux_x86_64` (maturin's
  PEP 517 build here), which PyPI refuses: the release's Linux companion wheels need
  `--compatibility manylinux_2_28` (e.g. in `MATURIN_PEP517_ARGS`, or `maturin build --sdist`-style
  from the unpacked sdist inside the manylinux_2_28 container, as `ci.yml` builds the main wheel).
- **HANDOFF material (relay 6), `docs/validation/golden.md`** (shared): in the larmorx-gpl row of the
  cases table, "`dataset_description.json` without its `Node` block" -> "`dataset_description.json`
  without its `Node` block and with larmorx-gpl's version written `<version>`"; after the paragraph
  ending "(rule 10's estimate, 2026-11-08).": "Re-recorded on 2026-11-10 from 13013f21 (the first
  release's relay 2c): the harness writes larmorx-gpl's own version, in Cargo's form or PEP 440's,
  as `<version>` in `dataset_description.json`'s `"Python"` field, so a release changes no
  checksum; exactly the 9 `dataset_description.json` outputs changed
  (`validation/investigations/release-0.1/gpl_golden_version_only.py`: with `<version>` written
  back as 0.0.1 each equals the record before, 9 of 9), every other checksum unchanged." and a
  new paragraph: "**The installed programs** (since 2026-11-10): the larmorx-nc and larmorx-gpl
  harnesses take `LARMORX_GOLDEN_PROGRAM` (a path, or a name found on `PATH`; `.exe` added on
  Windows): the cases then run that program instead of cargo's build, e.g. the one a wheel installs
  (`LARMORX_GOLDEN_PROGRAM=<venv>/bin/larmorx-nc cargo test --locked -p larmorx-nc-cli --test
  golden` in `crates-nc`), after checking that its `--version` is the checkout's. The release
  workflow runs them on the wheels it built (`validation/investigations/release-0.1/
  installed_golden.sh`: every case passes on the `dist` wheels' programs)." Optionally one sentence
  each in `crates-nc/README.md` and `crates-gpl/README.md`'s golden paragraph (not edited: outside
  relay 2c's files).
- **CHANGELOG (relay 5a):** `larmorx-nc --version` and `larmorx-gpl --version` print the PEP 440
  version (`0.1.0rc1`), as `larmorx --version`.
- The harness note of relay 2a about compare_records: records that hold `larmorx-nc <version>` /
  `larmorx-gpl <version>` from `--version` (fsl_bet's `replica_version()` and the like) change form
  from `0.1.0-rc.1` to `0.1.0rc1`; `compare_records.py`'s `strip()` covers both (D4).

## Relay 3a: what was done (2026-11-11)
- **`.github/workflows/release.yml`** (833 lines; `ci.yml` unchanged). Triggers: `push: tags:
  ["v*"]` (a release), `push: branches: ["release/dry-run"]` (D8: a dry run the orchestrator
  starts with `git push`), `workflow_dispatch` with the boolean `dry_run` (default true; off is
  accepted only on a `v*` tag ref: the way to restart a release whose tag run was lost; off on a
  branch stops `check` with an error). Only a release publishes: every job after `assemble` has
  `if: needs.check.outputs.publish == 'true'`. Top-level `permissions: {}`, each job its own
  (`contents: read`; `check` also `actions: read`; the publish jobs `id-token: write` only
  (larmorx's also `contents: read` for its sparse checkout); `github-release` `contents: write`,
  `id-token: write`, `attestations: write`). Every action pinned by commit SHA with its version in a
  comment (checkout v7.0.1, setup-python v7.1.0, upload-artifact v7.0.2, download-artifact v8.0.2,
  maturin-action v1.51.0, gh-action-pypi-publish v1.14.3, attest-build-provenance v4.2.2,
  dtolnay/rust-toolchain master of 2026-10-16 with `toolchain: 1.99.0`); the manylinux_2_28
  images pinned by tag and digest (`2026.10.26-1`, x86_64 `sha256:e0adf185...`, aarch64
  `sha256:a3e02308...`); no caches (zizmor's cache-poisoning; a release builds from scratch);
  `persist-credentials: false` everywhere; every `${{ }}` reaches a script through `env:`.
  maturin 1.15.0 (CI's), twine 7.0.0, Rust 1.99.0. Concurrency: a dry run gives way to a newer
  one; a release is never cancelled.
  The installed programs' paths (for `LARMORX_GOLDEN_PROGRAM`, `LARMORX_NC_BIN`,
  `LARMORX_GPL_BIN`) come from a small Python helper that writes them with forward slashes and no
  newline (Git Bash on Windows would keep `print`'s carriage return in `$(...)`) and fails the
  step when the program is not installed.
- **The jobs:**
  - `check` (ubuntu): mode and version (`set_version.py --print`; on a release `--check <tag>`,
    else `--check`); `ci_green.py` (the commit on `origin/main` and CI green for it or for its
    nearest green ancestor with documentation-only changes since, as ci.yml's `paths-ignore`;
    strict on a release, a notice on a dry run); `pypi_state.py absent` (the version on none of the
    three projects; strict on a release); the release notes (`changelog.py`: strict on a release;
    on a dry run a missing section is a warning annotation and placeholder notes); `cargo fetch
    --locked` of the three workspaces, `third_party.py --check`, `licences.py --check`,
    `pypi_readme.py --check`. Outputs: version, publish, prerelease, ref (the tag, or the commit for
    a dry run: PyPI's links point there), tag, epoch (the commit's time: `SOURCE_DATE_EPOCH`).
  - `sdist` (ubuntu): PyPI's descriptions rewritten at the ref, `maturin sdist`, `companion_sdist.py
    nc` and `gpl`, `twine check --strict`; artifact `sdist`.
  - `wheel` x6 (CI's runners and targets; Linux in the pinned manylinux_2_28 image;
    `MACOSX_DEPLOYMENT_TARGET=11.0` on macOS): larmorx's wheel, `--profile dist --locked`; artifact
    `wheels-<platform>`.
  - `programs` x6: the two companion sdists (the very files the `sdist` job made) unpacked into
    `_build/` inside the checkout (so the build finds `.cargo/config.toml` and
    `rust-toolchain.toml`: maturin-action reads the toolchain upwards to the workspace; given
    explicitly too), their wheels built there by maturin-action (`working-directory`, `--profile
    dist --locked --out ../../programs`; on Linux in the container, `--manylinux 2_28`, so the
    companion wheels are `manylinux_2_28`: relay 2c's note), the standalone larmorx and lx as a
    program wheel of `crates/larmorx-cli` (`--bindings bin`, not published; on Linux maturin's
    manylinux check covers the binaries too), the companions' golden tests on the programs their
    wheels install in a fresh venv (`LARMORX_GOLDEN_PROGRAM=<path> cargo test --locked -p
    larmorx-{nc,gpl}-cli --test golden`, dev profile for the harness; failures as annotations by
    `ci_cargo_failures.py`, mismatches uploaded on failure), the archives (`archives.py`);
    artifacts `companions-<platform>`, `archives-<platform>`.
  - `test` x6 (no Rust): the three wheels installed by path (numpy from PyPI), `--group test`,
    `python -m pytest` as CI with `LARMORX_NC_BIN`/`LARMORX_GPL_BIN` at the installed programs (so
    the replicas' own tests run: `test_the_replica_itself` of hmc and t2smap), mismatches uploaded on
    failure; `smoke.py --installed --archives --target`; on Windows `pe_imports.py --machine
    x86_64|aarch64 --quiet dist archives`; a fresh venv with `pip install --only-binary :all:
    --find-links dist "larmorx[all]==<v>"`, `pip check`, the smoke, the `[prep]` modules present;
    linux-x64 also Python 3.13 and 3.14 (fresh venvs, `pytest --ignore=tests/golden`, as CI).
  - `sdist-install` (ubuntu): the three sdists built from source by pip (build isolation, Rust
    1.99.0 the default, `MATURIN_PEP517_ARGS=--locked`, release profile), the smoke, `pytest
    tests/python tests/golden` with the installed programs.
  - `assemble` (ubuntu; needs everything above): `assemble.py` (exactly the 39 files: 18 wheels
    with the promised tags, 3 sdists, 18 archives; none over PyPI's 100 MiB; `release/pypi/` 21,
    `release/archives/` 18, `release/SHA256SUMS`; the table as a notice annotation), `twine check
    --strict` on the 21; artifact `release` (30 days).
  - `publish-nc`, `publish-gpl`, `publish-larmorx` (release only; environments **`pypi-nc`,
    `pypi-gpl`, `pypi`**, each with its PyPI URL): each moves exactly its 7 files (6 wheels, the
    sdist; the count checked) into `upload/` and runs gh-action-pypi-publish (`attestations: true`,
    `print-hash: true`). All three need only `[check, assemble]`, so their approvals appear together
    in one dialog; `publish-larmorx` first runs `pypi_state.py wait` (at most 20 minutes: PyPI must
    list every companion file of the version with this run's SHA-256s; another digest is an error
    at once), so a failed companion upload leaves larmorx unpublished (D6).
  - `github-release` (release only; needs the three publish jobs): GitHub's build provenance for
    the 39 files (`attest-build-provenance`), `gh release create <tag> --verify-tag --title
    "larmorx <v>" --notes-file <check's notes>`, `--prerelease --latest=false` for aN/bN/rcN, else
    `--latest`; the 39 files and `SHA256SUMS` (D7: after the uploads only).
  - `verify-pypi` x6 (release only; needs publish-larmorx): `pip install --only-binary :all:
    "larmorx[all]==<v>"` from PyPI (20 tries, 30 s apart), the smoke, `pytest tests/golden
    tests/python/test_smoke.py` with the installed programs.
- **Helpers** (`scripts/release/`, stdlib only, `python -P`): `changelog.py` (the version's
  section: headings `## [0.1.0rc1] - DATE` or `## 0.1.0rc1 (DATE)`, optional `v`; to the next
  level-2 heading, fences respected; Keep a Changelog's trailing link definitions dropped; links
  absolute at the ref through `pypi_readme.rewrite`; a footer with the install commands,
  `SHA256SUMS` and `gh attestation verify`; no file, no section, two sections or an empty one:
  exit 1 naming it and the versions the file has), `archives.py` (below), `smoke.py`, `assemble.py`,
  `pypi_state.py` (`absent`, `wait`, `--report`), `ci_green.py` (`--sha`, `--report`).
  `scripts/orchestration/watch_ci.py` takes an optional branch or tag (`release/dry-run`,
  `v0.1.0rc1`; default main) and prints each failed job's annotations.
- **The standalone archives** (FROM THE ORCHESTRATOR's form): made from the program wheels of the
  same run (`larmorx_cli`, `larmorx_nc`, `larmorx_gpl`), each `<package>-<v>-<target triple>`
  with a top folder of that name and a README.txt: `larmorx-...` holds `bin/larmorx` and `bin/lx`
  as a symbolic link to it (Windows: `bin\larmorx.exe` and `bin\lx.cmd` = `@"%~dp0larmorx.exe"
  %*`), the repository's LICENSE, NOTICE, LICENSES/ (as the larmorx wheel; third-party-rust.txt
  included); `larmorx-nc-...` holds `bin/larmorx-nc`, `share/larmorx-nc/fsl-data/` (beside
  `bin/`, where the program looks), its wheel's LICENSE (the FSL Licence), NOTICE, LICENSES/ at
  the top and the rest of its wheel's licence files under `source/` (FSL's original source, the
  Rust translation, Cargo files), `sbom/`; `larmorx-gpl-...` likewise (`source/` the program's
  crates; its README names the release's `larmorx_gpl-<v>.tar.gz` as the Corresponding Source).
  Sorted entries, owner 0, dated by `SOURCE_DATE_EPOCH` (the commit's time), gzip mtime 0: the same
  files give the same bytes (tested). Sizes (Linux x86-64, dist): larmorx 10.0 MB (the 25 MB
  program once), larmorx-nc 5.82 MB, larmorx-gpl 1.01 MB. `lx` through the link prints `Usage: lx`.
- **Tests:** `validation/tests/test_release.py` 30 -> 47 (the notes' sections, refusals, links and
  footer, the command line; the archives on Unix and Windows: layout, link, `lx.cmd`, modes, owner,
  dates, the same bytes twice, refusals; the whole set: 39 names, layout, SHA256SUMS, missing,
  foreign and doubled files; PyPI's project names and the comparison; ci_green's paths equal
  ci.yml's `paths-ignore`, its verdict on a temporary git history; the workflow: the three
  environment names exactly, every `uses:` pinned by a 40-hex SHA, the publish permissions,
  `permissions: {}`, the triggers, the five release-only jobs).
- **Checked here:** actionlint 1.7.12 with shellcheck 0.11.0 (`release.yml` and `ci.yml`: clean);
  zizmor 1.32.0 `--offline` (no findings; `--persona=auditor`: only 5 "info" superfluous-actions
  for dtolnay/rust-toolchain, kept: it installs rustup where a runner has none);
  `validation/investigations/release-0.1/workflow_rehearsal.sh` (the workflow's Linux steps on a
  fresh clone of `46ed4455` with maturin 1.15.0; log `oracles/logs/release-3a-rehearsal.log`):
  every step passed (README "Relay 3a"); the smoke 47 of 53, the 6 others the expected glibc 2.28
  failures of host-built files (they need `GLIBC_2.34`: the check works).
  The test job's steps on those files (fresh venvs of CPython 3.12; log
  `oracles/logs/release-3a-testjob.log`; now part of the rehearsal): `pytest tests/python
  tests/golden` with `LARMORX_NC_BIN`/`LARMORX_GPL_BIN` at the installed programs first failed
  `tests/python/test_mri_replica.py::test_the_replica_itself` ("LARMORX_NC_BIN is set to 'None'":
  the test read the variable after its `no_replica` fixture had removed it, so it worked only
  beside a repository build; CI sets no variable and skipped it). **Fixed in the test** (read once
  at import, as `test_tedana.py` does; `4b5c5d4d`; not clean-room code: the audit's clean-room
  paths are the three crates); then 885 passed, 51 skipped. `larmorx[all]` from the built files:
  21 s, `pip check` clean, the smoke passes but the glibc checks, the `[prep]` modules present.
- **Gates at `4b5c5d4d`** (`oracles/logs/release-3a-gates.log`, boxed 64 GiB; no Rust changed, so
  relay 2c's extension and release programs stayed in place): the three Rust workspaces OK (fmt,
  clippy, test; 121 s); Python 1,361 passed, 12 skipped, 7 failed: the same 7 `test_smoke.py`
  version tests as relays 2a-2c (the shared venv's metadata says 0.0.1 in a worktree; on the
  installed wheels they pass: the test job's steps above); without FSLDIR 917 passed, the same 7;
  ruff, ruff format, check_spdx, tables --check, set_version --check v0.1.0rc1, licences --check,
  third_party --check, pypi_readme --check, actionlint (both workflows), zizmor: pass. No parity
  smoke tier: nothing under `crates/` or `python/larmorx` changed (relay 2c's run at `d912a342`
  stands), no golden checksum changed. `d5421dac` changed only `release.yml` and the README:
  actionlint, zizmor and `test_release.py` (47 passed) again after it. The second rehearsal at
  `4b5c5d4d` (`oracles/logs/release-3a-rehearsal-2.log`, with the test job's steps): every step
  passed, pytest 885 passed / 51 skipped, the smoke 47 of 53 (the 6 expected glibc checks).
- **Job-minutes per run (estimates from CI run 42766636707's times and the local builds; the first
  dry run measures them):** check 3, sdist 3, wheel 6 x 15-25 (about 120), programs 6 x 25-35
  (about 180: the companions' dist builds 5-8, the CLI's two LTO links 12-16, the golden tests'
  dev builds and runs 5-7, three container starts on Linux), test 6 x 6-10 + 5 (about 55),
  sdist-install about 20, assemble 2: **a dry run about 380 job-minutes, about 55 minutes of wall
  time** (check, sdist, programs, test, assemble in a row). A release adds the publish jobs (about
  10, larmorx's waiting included), github-release 3 and verify-pypi 6 x 6: **about 430
  job-minutes**, plus the approval. Four runs (dry run, rc1, dry run, 0.1.0) about 1,600
  job-minutes; standard runners are free for a public repository; at most 13 jobs at once (4 on
  macOS), within GitHub Free's 20 (5 macOS). Relay 3b's two container jobs add about 20.

## Relay 3a: findings and follow-ups (for 3b and the orchestrator)
- **For relay 3b (the container):** add the `container` jobs (amd64 on ubuntu-24.04, arm64 on
  ubuntu-24.04-arm) after `[check, wheel, programs]` (the Linux files: artifacts
  `wheels-linux-x64`/`-aarch64` and `companions-linux-x64`/`-aarch64`; or from `release` after
  `assemble`), build and smoke-test the two images on every run, push nothing on a dry run; make
  `assemble` (or the publish jobs) need them so nothing is published before the images passed;
  push by digest and tag after the three publish jobs (`packages: write` there only), and make
  `github-release` need the tag job (D7). Then update `test_release_workflow_names_and_pins`
  (it counts five release-only jobs) and, if wanted, `changelog.footer` (an `apptainer pull
  docker://ghcr.io/karellopez/larmorx:<v>` line). docker/login-action v4.8.0 was 5 days old on
  2026-11-10 (relay 1: take the release before it). actionlint and zizmor as above (the tools in
  `<scratch>/r3a/tools/`: `al/actionlint`, `venv/bin/zizmor`, `venv/bin/shellcheck`).
- **Only the dry run can show** (not testable here): maturin-action's container builds of the
  unpacked sdists (`working-directory` under the workspace, `--out ../../programs`; its source at
  the pinned SHA mounts the workspace at the same path and runs in `<workspace>/<working-directory>`),
  the dist builds' golden bits on the five other platforms (Windows' first `+crt-static` builds),
  the wheel tags `assemble` expects (`manylinux_2_28_*`, `macosx_11_0_*`, `win_*`), macOS's
  minimum 11.0 in the binaries (smoke's `otool -l`), rustup on windows-11-arm (dtolnay installs it
  if missing), and the replicas' own Python tests on all six platforms (`test_the_replica_itself`
  of hmc and t2smap: CI never ran them on its runners, which have no companion programs).
- **Expected annotations of a dry run before relay 5a:** a warning "CHANGELOG.md: no usable
  section for 0.1.0rc1: a release would stop here", a notice "CI-GREEN NO: ... is not on
  origin/main" (a dry run of the branch before its merge), a notice "0.1.0rc1 is not on PyPI yet".
- **Possible change after the first dry run:** the CLI's build in its own job (`cli` x6, needs
  only `check`; the archives then made in `test`) would take about 10 minutes off the wall time
  for about 30 more job-minutes; decide from the dry run's job times.
- **For relay 5a (RELEASING.md):** approve the three deployments together (larmorx's job waits
  20 minutes for the companions, then fails without uploading); a failed job after an approval is
  re-run with "Re-run failed jobs" (the `check` job is not re-run, so the PyPI-absent check does
  not block it); PyPI never takes a version twice, so a broken upload means the next version; the
  dispatch with `dry_run` off on the tag is the way to restart a lost tag run.
- **HANDOFF material (relay 6):** `docs/orchestration.md`'s release procedure: the dry run is
  `git push origin +<sha>:refs/heads/release/dry-run` and `python3 scripts/orchestration/watch_ci.py
  <sha> release/dry-run`; a release `git tag -a v<v> -m "larmorx <v>" <main's sha>`, `git push
  origin v<v>`, `watch_ci.py <sha> v<v>`; `assemble`'s notice (the files and sizes) at
  `https://api.github.com/repos/karellopez/larmorx/check-runs/<job id>/annotations`.

## Relay 3a: the dry run (for the orchestrator)
1. **When:** the workflow runs as it is now, without the container jobs; a first dry run right
   after relay 3a would shake out the platform problems (Windows' first `+crt-static` builds, the
   container builds of the unpacked sdists, the six platforms' wheel tags) while relays 3b-5b
   work, and costs nothing on a public repository (nothing is published: the publish, release
   and verify jobs are skipped; the environments, which allow only `v*` tags, are not touched;
   CI's own workflow does not run on that branch). Then again after relay 3b, and on main's merge
   commit before the tag.
2. **Start:** `git push origin +<sha>:refs/heads/release/dry-run` (the branch head of the part,
   e.g. `d5421dac` or later; `+` replaces an earlier dry run's head; pushing works through VS
   Code's credential socket).
3. **Watch:** `python3 scripts/orchestration/watch_ci.py <sha> release/dry-run` (relay 3a's
   version: the branch as second argument; it prints each job's result and a failed job's
   annotations); the run page https://github.com/karellopez/larmorx/actions/workflows/release.yml.
   About 55 minutes, about 380 job-minutes.
4. **Pass:** every job green: check, sdist, wheel x6, programs x6 (the companions' golden tests
   on their installed programs), test x6 (the golden tests on the dist builds, the replicas' own
   tests, the smoke, the PE check, `larmorx[all]`), sdist-install, container x2 (relay 3b: both
   images built on amd64 and arm64 and checked inside, `CONTAINER check ok: 27 checks`), assemble
   (39 files; then `CONTAINER compare ok`: a difference between the architectures' packages is a
   warning annotation). Expected
   annotations: the CHANGELOG warning (until relay 5a), "CI-GREEN NO ... is not on origin/main"
   (a notice), "0.1.0rc1 is not on PyPI yet". The files and their sizes: assemble's notice at
   `https://api.github.com/repos/karellopez/larmorx/check-runs/<assemble's job id>/annotations`.
   Failures: pytest and cargo failures and smoke failures are error annotations; golden
   mismatches are uploaded as artifacts (a login needed to download them).
5. **The release, later** (after the part's merge, CI green on main, a dry run of main's commit,
   relay 5a's CHANGELOG and RELEASING.md): re-check the three environments by the API, `git tag -a
   v0.1.0rc1 -m "larmorx 0.1.0rc1" <main's sha>`, `git push origin v0.1.0rc1`, `watch_ci.py <sha>
   v0.1.0rc1`; the user approves the three deployments in one dialog; the GitHub Release and the
   PyPI install on six platforms follow by themselves.

## Relay 3b: what was done (2026-11-11)
- **`docker/Dockerfile`** (D5: two targets from the release's own Linux wheels, base
  `docker.io/library/python:3.12.16-slim-trixie@sha256:5e2bdb1b...` (the multi-architecture index,
  updated on Docker Hub 2026-11-08), no `# syntax` line (BuildKit's built-in frontend), wheels by a
  `RUN --mount=type=bind` so they stay out of the layers):
  - `larmorx` -> `ghcr.io/karellopez/larmorx:<v>`: the release's wheel with `[prep]` from PyPI
    as a user's `pip install` gets it (no `--only-binary :all:`: below), `pip check`, `/opt/larmorx/pip-freeze.txt` (`pip list
    --format=freeze`: `pip freeze` writes `larmorx @ file:///wheels/...` for a wheel installed by
    path), `/opt/larmorx/README.txt` (`docker/README.txt`: the two images, Apptainer and Docker
    commands, caches, where the licences and sources are). OCI labels title, description,
    version, revision, created (the commit's time), source, url, documentation, licenses
    `Apache-2.0`, base.name. ENV `MPLCONFIGDIR=/tmp/larmorx-matplotlib`, `PIP_NO_CACHE_DIR` etc.
    No ENTRYPOINT, `CMD ["larmorx", "--help"]`.
  - `larmorx-all` (FROM larmorx) -> `:<v>-all`: the two companion wheels (`--no-index --no-deps`),
    `pip check`, the freeze again, their sdists in `/opt/larmorx/source/` (the GPL's Corresponding
    Source, FSL's original and amended source); title `larmorx-all`, a description that says
    "NON-COMMERCIAL USE ONLY" and why (implementation="auto" runs the FSL replicas), licenses
    `Apache-2.0 AND GPL-3.0-only AND LicenseRef-FSL`.
  - `latest`, `latest-all` only for a final release.
- **`scripts/release/container.py`** (stdlib, `python -P`; every step the workflow runs, so the
  local rehearsal runs the same code): `stage` (this architecture's three wheels of the version,
  the two sdists, the README; anything missing, twice or of another version an error), `build`
  (`docker buildx create --driver docker-container --driver-opt image=moby/buildkit:v0.34.1@sha256:
  99368f3e...` when the builder `larmorx-release` is missing; per target `docker buildx build
  --platform linux/<arch> --provenance false --sbom false --output type=oci,dest=<image>-<v>-<arch>.oci.tar
  --output type=docker,name=larmorx-release/<image>:<v>`, build args VERSION, REVISION, CREATED,
  SOURCE_DATE_EPOCH; each archive's blobs checked against their digests; `images.json` with each
  image's manifest and config digests), `check` (per image: the loaded image is the archive's
  (Docker's id is the config, the containerd store's the manifest: both accepted), its labels, no
  entrypoint, `smoke.py --installed --image <image>` as root and as user 65534 with `--read-only
  --tmpfs /tmp` (Apptainer's conditions), the default command, the freeze (`larmorx==<v>`; the
  companions present only in -all; -all = larmorx's set plus exactly the two), the sdists in -all
  only, the golden tests and `tests/python/test_smoke.py` inside (the checkout mounted read-only,
  `pip install --group test` in the throwaway container, mismatches to
  `golden-mismatches-<image>/`)), `compare` (each image's packages equal on both architectures;
  a difference is a warning), `index` (one OCI layout with every blob once and per image a
  multi-architecture index, sorted, compact JSON, so the same inputs give the same digest;
  `index.json` names them `<v>` and `<v>-all`; `digests.json`), `push` (`skopeo copy --all
  --preserve-digests --retry-times 3` of each index to `<v>`/`<v>-all` and, with `--latest`, to
  `latest`/`latest-all`; each tag read back with `skopeo inspect --raw` and its SHA-256 compared;
  `digest=`/`digest-all=` into `GITHUB_OUTPUT`).
- **`scripts/release/smoke.py --image larmorx|larmorx-all`** (with `--installed`): the plain
  image must hold larmorx alone (larmorx-nc and larmorx-gpl neither installed nor on the path, no
  FSL data), -all all three; `/opt/larmorx/README.txt`, `pip-freeze.txt`, `source/` (two sdists in
  -all, nothing in the plain one); the binaries' platform checks left out (the image brings its own
  glibc). The default behaviour is unchanged.
- **`release.yml`** (actionlint 1.7.12 with shellcheck and zizmor 1.32.0 `--offline`: clean;
  `--persona=auditor`: the same 5 "info" superfluous-actions as relay 3a, nothing new):
  - `container` x2 (`amd64` on `ubuntu-24.04`, `arm64` on `ubuntu-24.04-arm`; needs `[check,
    sdist, wheel, programs]`; `contents: read`; every run): the three wheels (artifacts
    `wheels-<platform>`, `companions-<platform>`) and the sdists into `files/`, `container.py
    stage`, `build`, `check`; mismatches uploaded on failure; artifacts `images-<arch>` (the two
    OCI archives, `images.json`, the freezes; `compression-level: 0`) and `image-meta-<arch>`
    (`images.json` and the freezes). Nothing is pushed.
  - `assemble` now needs `container` too (nothing is published before both images passed) and
    runs `container.py compare` on the two `image-meta-*` artifacts.
  - `container-push` (release only: `if: needs.check.outputs.publish == 'true'`; needs `[check,
    container, publish-larmorx, publish-nc, publish-gpl]`; `contents: read`, `packages: write`
    (the only job with it), `id-token: write`, `attestations: write`): `container.py index` of the
    two `images-*` artifacts, skopeo (apt-get install if the runner lacks it), `skopeo login
    ghcr.io` with the job's token by stdin into `$RUNNER_TEMP/ghcr-auth.json` (removed in an
    `always()` step), `container.py push` (`--latest` unless a pre-release), then
    `attest-build-provenance` (the same pinned v4.2.2) for both index digests (`subject-name
    ghcr.io/karellopez/larmorx`, not pushed to the registry: GitHub keeps them; `gh attestation
    verify oci://ghcr.io/karellopez/larmorx:<v> --repo karellopez/larmorx`). Outputs `digest`,
    `digest-all`.
  - `github-release` needs `container-push` too (D7) and appends "**Container images**
    (linux/amd64, linux/arm64): `ghcr.io/karellopez/larmorx:<v>` (`<digest>`) and `...:<v>-all`
    (`<digest>`)." to the notes.
  - The header comment names the new jobs. Release-only jobs now 6.
  - The test job's `larmorx[all]` install and `verify-pypi`'s: `--only-binary
    larmorx,larmorx-nc,larmorx-gpl` instead of `:all:` (the finding below).
- **`changelog.py`'s footer:** a **Container** paragraph: `apptainer pull
  docker://ghcr.io/karellopez/larmorx:<v>` or `docker run --rm ghcr.io/karellopez/larmorx:<v>
  larmorx --version` (Apache-2.0 and permissive licences only); `...:<v>-all` adds larmorx-gpl
  and larmorx-nc (non-commercial use only).
- **Tests** (`validation/tests/test_release.py` 47 -> 55): the count of release-only jobs 6; the
  footer's container lines; `stage` (one Linux wheel per package and architecture, others left
  alone, refusals); `index_layout` on synthetic OCI archives (refs, platforms, every blob its
  digest, the shared layer once, the same digests from the folders in either order) and its
  refusals (missing architecture, other version, an image of the wrong architecture,
  `images.json` naming another manifest, a changed blob); `image_manifests` following a nested
  index and leaving out an attestation manifest; `compare_freezes`; the tag names and labels, the
  BuildKit pin; the Dockerfile (base pinned by digest, the two targets in order, each target's
  licence and title equal to `container.IMAGES`, the companions only in -all, no ENTRYPOINT, the
  CMD); the workflow's container jobs (runners, no push and no `packages:` in `container`, the
  push job's needs, `packages: write` exactly once and only there, `assemble` needs `container`,
  `github-release` needs `container-push`).
- **Proved here** (`validation/investigations/release-0.1/container_rehearsal.sh`, README "Relay
  3b"; log `oracles/logs/release-3b-container-rehearsal-3.log`, at `5d1c328b`): a rootless Docker
  daemon with no administrator rights (`rootless_docker.sh`: Docker's rootless extras, the
  account's subordinate ids; data in the scratch folder, in its own 32 GiB box), relay 3a's
  rehearsal files (host-built wheels, `manylinux_2_34`), BuildKit 0.34.1 pinned: **every step
  passed** in about 5 minutes: both images built (49 packages, pybids 0.22.0; OCI archives 226.7
  and 237.9 MB; about 680 and 700 MB unpacked), the 27 checks inside (golden + test_smoke 317 passed in each image), Apptainer SIF
  images from the archives (213, 224 MB) with `apptainer exec --containall` of `larmorx
  --version`, `larmorx-nc --version`, `larmorx-gpl --version`, `smoke.py --image` and the golden
  tests (305 passed in each), the push into a local registry with skopeo 1.13.3 (the runner's
  Ubuntu 24.04 version) of the four tags with the indexes' own digests (the manifests pushed are
  the ones built and checked), and `apptainer pull docker://...` of the four tags running the
  programs. Not here (only GitHub): arm64, the runners' Docker, the manylinux_2_28 wheels inside,
  GHCR's login and first push, the attestations.
- **Gates** (at `5ef26ea9`; `oracles/logs/release-3b-gates-2.log`, boxed 64 GiB; the first run
  before the dry-run fixes in `release-3b-gates.log`, the same results): the three Rust
  workspaces OK (no Rust changed); Python 1,370 passed, 12 skipped, 7 failed: the same 7
  `test_smoke.py` version tests as relays 2a-3a (the shared venv's metadata says 0.0.1 in a
  worktree; inside the images they pass: 317 of 317 with `test_smoke.py`); without FSLDIR 917
  passed, the same 7; ruff, ruff format, check_spdx, tables --check, set_version --check,
  licences --check, third_party --check, pypi_readme --check, actionlint (both workflows),
  zizmor, shellcheck of the two new scripts: pass. `test_release.py` 56 passed. No parity smoke
  tier: nothing under `crates/` or `python/larmorx` changed; no golden checksum changed.
- Relay 3b's runners in `<scratch>/r3b/` (`build-try.sh`, `check-try.sh`, `rehearse.sh`,
  `gates.sh`, `hubdigest.py` (Docker Hub digests); the rehearsal folders `rehearsal/` (the first
  run: the push failed on the rehearsal script's tag-and-digest image names, fixed in
  `14f7cc2b`; then `FROM=push` passed), `rehearsal-2/` (every step passed at `14f7cc2b`, pybids
  0.18.1) and `rehearsal-3/` (the record, at `5d1c328b`); the trial images in
  `try/`; Docker's data in `docker/`, `rehearsal/docker/`, `rehearsal-2/docker/`,
  `rehearsal-3/docker/`: 1.9 GB each; `r3b/` 18 GB in all, deletable build outputs; Docker's
  folders hold files of the account's subordinate ids, which a plain `rm` cannot remove:
  `rootlesskit rm -rf <folder>` can); NOTES before
  3b in `r3b/NOTES-before-3b.md`.

## Relay 3b: findings and follow-ups (for 4, 5a, 6 and the orchestrator)
- **The dry run's container jobs:** the next dry run (a commit from `14f7cc2b` on) shows the
  arm64 image and the runners' Docker; `container-push` runs only on a release, so the rc1 tag's
  run is the first push to GHCR. If it fails after the PyPI uploads, "Re-run failed jobs" repeats
  it (the images are artifacts of the run, kept 14 days); `github-release` waits for it.
- **For the user (D17, RELEASING.md, relay 5a):** after rc1's push the package
  `ghcr.io/karellopez/larmorx` is private: github.com/users/karellopez/packages/container/larmorx
  -> Package settings -> Change visibility -> Public (once); until then `apptainer pull` and
  `docker pull` without a login fail. Check after: `apptainer pull
  docker://ghcr.io/karellopez/larmorx:0.1.0rc1` on this machine (relay 4 or the orchestrator).
- **For relay 4 (clean machines):** the rehearsal's SIF images are here
  (`<scratch>/r3b/rehearsal-2/sif/`); relay 4 can re-run `container_rehearsal.sh` with the
  wheels it builds in the manylinux_2_28 container (the real release tags), and after rc1 pull
  the real images.
- **For relay 5a (README, RELEASING.md, CHANGELOG):** the two images and what each holds
  (`docker/README.txt` has the user text), `apptainer pull docker://ghcr.io/karellopez/larmorx:<v>`,
  `docker run --rm -v "$PWD:/data" -w /data ghcr.io/karellopez/larmorx:<v> larmorx ...`, `latest`
  only for final releases, TemplateFlow's cache (`$HOME/.cache/templateflow`; a volume with
  Docker), `gh attestation verify oci://ghcr.io/karellopez/larmorx:<v> --repo karellopez/larmorx`;
  the release's job list now has `container` x2 and `container-push`.
- **Job-minutes:** the two container jobs about 8-12 minutes each (build about 1-2, checks 2-3,
  the artifacts about 450 MB per architecture), in parallel with `test` after `programs`: a dry
  run about 400 job-minutes, wall time unchanged or a few minutes more; `container-push` about 5
  minutes (downloads about 900 MB, pushes about 470 MB of layers once: the -all image shares the
  plain one's).
- **Finding, fixed (`67d00142`): `--only-binary :all:` changed what `[prep]` resolves to.**
  docopt has no wheel (an sdist only), num2words 0.5.10 and newer need it, pybids 0.19 and newer
  need those: with `--only-binary :all:` pip backtracked (about 20 s) to **pybids 0.18.1** (with
  formulaic 0.5.2), while a user's plain `pip install "larmorx[all]"` builds docopt's pure-Python
  sdist and gets pybids 0.22.0. So relay 3a's `larmorx[all]` checks (the test job, `verify-pypi`,
  `workflow_rehearsal.sh`) tested another set of packages than users get: they now say
  `--only-binary larmorx,larmorx-nc,larmorx-gpl` (our packages must still come as the built
  wheels), and the images install `[prep]` without the option. Nothing of larmorx imports pybids
  (templateflow does).
- **HANDOFF material (relay 6):** `docs/licensing.md` (what each image holds and under which
  terms: the plain image Apache-2.0 and permissive only, `-all` with the FSL Licence's
  non-commercial terms, the sources in `/opt/larmorx/source/`); `docs/orchestration.md`'s release
  procedure (the images are pushed by the tag's run after the approval; D17 once).

## Relay 4: what was done (2026-11-11)
Record: `validation/investigations/release-0.1/README.md`, "Relay 4: clean machines" (every
environment with its image digest, versions, times, results). Runners and outputs in
`<scratch>/r4/` (28 GB: `build/` the clean build with its clone, Rust, maturin and the Windows
GNU files in `build/wgnu/`; `env-*/` the three environments; `container/` the rehearsal (its
Docker data has files of the account's subordinate ids: `rootlesskit rm -rf` removes them);
`sif/` the four images; NOTES before relay 4 in `r4/NOTES-before-4.md`).
- **The Linux files as the release makes them** (`clean_build.sh`, in release.yml's pinned
  manylinux_2_28 image through Apptainer, `--containall --cleanenv`, a fresh clone of `5ef26ea9`,
  Rust 1.99.0, maturin 1.15.0): every step passed; the tags `manylinux_2_28_x86_64` (abi3 and
  py3-none), the sizes equal to relays 2b/3a's; the companions' golden tests pass on the
  programs their wheels install.
- **Three clean environments** (`clean_env.sh`, user 1000, nothing of the machine's on the
  paths): RHEL 8.10 (UBI 8 python-312, glibc 2.28, Python 3.12.15, venv pip 23.2.1, GCC 8.5),
  Debian 12 slim (Python 3.12.16, no compiler, no binutils) and Debian 13 with Python 3.14.10:
  `larmorx`, `larmorx[prep]`, `larmorx[all]` from the wheels, `pip check` clean, smoke 52 of 52
  (the glibc 2.28 floor holds and the programs run on 2.28), pytest `tests/python tests/golden`
  934 passed, 2 skipped everywhere (after `ebf9dde6`; the first runs failed only that test);
  from the sdists (RHEL 8 with Rust 1.99.0, Debian 13 with stable 1.99.1): `[all]` 150-170 s,
  smoke `--source-build` 17 of 17, pytest 934 passed (the golden suite on a newer Rust: the same
  bits), the companions' golden tests pass on both installs; in the slim image the sdist fails as
  it must: maturin fetched Rust itself (puccinialin), then "linker `cc` not found".
- **The container rehearsal** with the manylinux_2_28 wheels: every step passed (27 checks
  inside, 317 passed in each image; the SIF images, 305 passed; push, pull of the four tags).
- **Windows** (win-laptop, no administrator rights asked for): `win_check.sh <v>` copies
  `check_install.py` and `check_install_windows.ps1` into `%USERPROFILE%\larmorx-check-<v>`, runs
  the check (uv's Python 3.12.14 installed into that folder, a venv, `larmorx[all]==<v>` from
  PyPI, the tag's tests from GitHub's archive, the GitHub Release's `.exe` archives, smoke with the
  PE checks, pytest), and checks the folder is gone. Now: from PyPI it fails at the install, as
  it must (not uploaded yet); with Windows files cross-built here by zig/MinGW (no MSVC, no
  Microsoft SDK: `win_gnu_build.sh`) pytest **934 passed on Windows**, the smoke passes but the
  three C-runtime checks (those files import the UCRT's DLLs, which the check refuses, as it must
  for the release's MSVC files). **The folder was made and removed four times; nothing is left**:
  the first run's uv shim (`.local\bin\python3.12.exe`) and registry entry
  (`HKCU\Software\Python\Astral\CPython3.12.14`), both pointing into the removed folder, were
  removed and the wrapper now prevents them (checked after the last run).
- **Gates at `2e5c21f4`** (`oracles/logs/release-4-gates.log`, boxed 64 GiB; no Rust changed, so
  relay 2c's extension and release programs stayed in place): the three Rust workspaces OK (fmt,
  clippy, test; 121 s); Python 1,373 passed, 12 skipped, 7 failed: the same 7 `test_smoke.py`
  version tests as relays 2a-3b (the shared venv's metadata says 0.0.1 in a worktree; on the
  installed files they pass: 934 of 934 in every clean environment); without FSLDIR 917 passed,
  the same 7; ruff, ruff format, check_spdx, tables --check, set_version --check, licences
  --check, third_party --check, pypi_readme --check, actionlint (both workflows), zizmor,
  shellcheck `-S warning` of the release-0.1 scripts: pass; `test_release.py` 59 passed. No parity
  smoke tier: nothing under `crates/` or `python/larmorx` changed; no golden checksum changed.

## Relay 4: findings and follow-ups (for 5a, 6 and the orchestrator)
- **Dry run 3** (the orchestrator, from this relay's head): the first with the macOS `test` jobs
  past their smoke step (their `larmorx[all]` from the built files never ran yet). The other
  changes it carries: `smoke.py` (macOS install name; glibc without objdump: on the runners
  objdump exists, so the Linux path is as before), `test_sdcflows_app.py`, the new
  `check_install*` scripts (not run by the workflow).
- **For relay 5a (README's installation, RELEASING.md):** a build from source needs a C compiler
  (gcc or clang; on Windows MSVC's build tools) and Rust 1.99 or newer, which maturin's build
  backend downloads by itself into `~/.cache/puccinialin` when none is installed; checked: RHEL 8
  (glibc 2.28, GCC 8.5), Debian 12 and 13, Python 3.12 to 3.14 (abi3). RELEASING.md's checks
  after the upload: `validation/investigations/release-0.1/win_check.sh <v>` (the Windows
  helper; or on any Windows machine `powershell -NoProfile -ExecutionPolicy Bypass -File
  check_install_windows.ps1 -Version <v>`), and on Linux or macOS `python -P
  scripts/release/check_install.py --version <v> --work <new folder>` (e.g. inside `apptainer
  exec --containall docker://python:3.12-slim-bookworm`), then `apptainer pull
  docker://ghcr.io/karellopez/larmorx:<v>` once the package is public (D17).
- **Informational:** on Python 3.13 and newer pip installs **wrapt 2.6.0.dev1**, a pre-release:
  formulaic 1.2.2 (pybids' dependency) asks for `wrapt>=1.17.0rc1` there, and a pre-release in a
  specifier lets pip take pre-releases (Python 3.12 gets wrapt 2.5.2); every test passes with it.
  Nothing of larmorx's to change; the orchestrator may note it for the users' page.
- **Informational:** `maturin build --zig --target x86_64-pc-windows-gnu` of the extension
  writes `PyInit_larmorx._core` into its `.def` (the dotted module name) and the link fails; only
  zig cross-builds meet it (`win_gnu_build.sh` works around it). An upstream report could be
  drafted (maturin), if the user wants.
- **HANDOFF material (relay 6):** next-steps' "clean-machine check": done on Linux (RHEL 8 with
  glibc 2.28, Debian 12 slim, Debian 13 with Python 3.14; the wheels, the sdists, the images),
  prepared for Windows (`win_check.sh` after rc1; the PE check the evidence for the C runtime;
  Windows Sandbox later, the user); `docs/orchestration.md`'s release procedure: after the
  upload, `win_check.sh <v>` and `check_install.py` from PyPI in a clean Linux image.

## Relay 5a: what was done (2026-11-11)
Runners and outputs in `<scratch>/r5a/` (`gates.sh`, `cffvenv/` (cffconvert 2.0.0), `notes.md`
(the release notes as the workflow makes them), the edit scripts; NOTES before 5a in
`r5a/NOTES-before-5a.md`).
- **README.md** (edited in the branch, as the brief says; the orchestrator reviews it at the
  merge): the status line (0.1.0rc1, a pre-release; larmorprepx not ready, only its field-map
  stages exist); "Installation and usage" split into **Installation** (pip and the two extras;
  how pip treats a pre-release; a table of the three packages with each licence in plain words,
  larmorx-nc's effect on `implementation="auto"`; the requirements as the release's files make
  them: Python 3.12 or newer (tested on 3.12 everywhere, 3.13 and 3.14 on Linux; no
  free-threaded builds), Linux x86-64/aarch64 with glibc 2.28 (`manylinux_2_28`; no musl),
  macOS 11 as the wheels' tags say (12 on Apple silicon for `[prep]`/`[all]`: SciPy's and
  scikit-learn's wheels), Windows 10/11 x64 and arm64 (the C runtime built in); a source build
  needs a C compiler and Rust 1.99, which maturin downloads by itself; the standalone archives;
  the two images with Apptainer and Docker commands, TemplateFlow's cache under each) and
  **Usage** (the examples, unchanged); Findable/Accessible (releases, CITATION.cff, the images
  offered); the roadmap's item 5; two rows (CHANGELOG, RELEASING) in the documents table; the
  Licence section (larmorx's own licence Apache-2.0; PyPI's expression lists the included works'
  licences; larmorx-gpl named; tedana among the projects not affiliated; CITATION.cff). The
  generated tools table is untouched (`tables --check` passes). Anchors changed:
  `#installation-and-usage` is now `#installation` and `#usage` (nothing in the repository links
  it; relay 5b: check the site). `pypi_readme.py --check`: 59 lines rewritten, no relative link
  left; readme_renderer (GFM) renders README, CHANGELOG and RELEASING without error.
- **CHANGELOG.md** (Keep a Changelog form): `## [0.1.0rc1] - 2026-11-12` with "What is in it"
  (the three packages and the extras, the six platforms' wheels, the standalone programs, the two
  images, SHA256SUMS and the attestations, the PEP 440 `--version` of the three programs),
  "The tools and their status" (the status words in plain words, what `-replica`/`-full` mean,
  and the **statuses table generated from `validation/tools.toml`**: each of the 26 tools with
  the records its validation text links and each record's status word; a data panel's record is
  part of its tool's main record) and "Known limitations" (larmorprepx; not yet ported:
  FreeSurfer, surfaces, AFNI's other tools, the confounds; t2smap needs larmorx-gpl; larmorx-nc
  changes the FSL tools' default; Python's free-threaded builds; no musl or glibc < 2.28; macOS
  12 on Apple silicon for `[prep]`; wrapt's pre-release on Python 3.13+ (relay 4); source builds
  need Rust and a C compiler, Windows source builds link the C runtime dynamically; the two
  Cargo-form version messages); the link definition at the end. `changelog.py 0.1.0rc1 --ref
  v0.1.0rc1` passes (15,405 characters of notes).
- **`scripts/release/changelog.py`:** `statuses(tools_toml)` makes the table;
  `--write-statuses` (`changelog.py <v> --write-statuses`) writes it between the section's two
  marker lines (`<!-- BEGIN statuses: validation/tools.toml (...) -->`, `<!-- END statuses -->`);
  `notes()` refuses a table that differs from `tools.toml` (so the tag's `check` job stops a
  release whose table is stale, and a dry run warns) and leaves the marker lines out; **the
  notes' paragraphs and list items are joined into one line each** (`unwrapped()`: GitHub shows a
  release's text as a comment, where a line break in a paragraph stays one; lists, tables,
  headings, fences, quotations and hard breaks kept). release.yml's dry-run warning now says
  "the release notes of <v> failed (this step's log says why)" (actionlint and zizmor clean).
- **CITATION.cff** as "FROM THE ORCHESTRATOR" gives it (cff-version 1.2.0, a message, the title
  "LARMORX: Library of Algorithmic Reimplementations for MRI through Open Rust eXecution", an
  abstract, one author with the university e-mail, affiliation and ORCID, version "0.1.0rc1",
  date-released "2026-11-12", license Apache-2.0, url the site, repository-code, the PyPI
  artifact, keywords); `cffconvert --validate`: valid for schema 1.2.0; `set_version.py --check
  v0.1.0rc1` passes (it covers the version; `set_version.py <v> --date D` sets the date).
- **RELEASING.md** (the maintainer's steps): what a release publishes; the once-only setup
  (PyPI's pending publishers, the environments, GHCR's visibility); 1 the release commit
  (`set_version.py <v> --date`, the CHANGELOG section with `--write-statuses`, the link
  definition, README's status line, the checks); 2 the dry run (`git push origin
  +<commit>:refs/heads/release/dry-run` or a dispatch; what must be green, the normal notices);
  3 the tag (re-check the environments; what `check` refuses); 4 the approval (all three
  together; why; reject leaves nothing); 5 a failure after the approval ("Re-run failed jobs",
  never "Re-run all jobs" after an upload began; a partial upload: yank and the next version;
  container-push, github-release (delete a half-made release, keep the tag), verify-pypi; a lost
  tag run: the dispatch with dry run off on the tag); 6 after the release (D17: the package
  public once; `check_install.py` in a clean Linux image through Apptainer, the Windows script
  or `win_check.sh`; the images' pull and attestation; `sha256sum -c`, `gh attestation
  verify`); 7 0.1.0 after rc1 (and rc2 for a problem); "Good to know" (Windows source builds
  link the C runtime dynamically; the Cargo-form messages; the rehearsals' README; the
  job-minutes).
- **The account name and the host names** out of the current tracked files listed in the
  relay plan: `docs/helper-machines.md` (the remote's path from the home folder there),
  `docs/machine-move.md` (placeholders; the host names are in the workspace's
  ORCHESTRATOR-STATE.md), `scripts/orchestration/move/phase4-clean.sh` (the host name from
  `OLD_LAPTOP_HOSTNAME`, refused without it or on another machine: both refusals run here;
  Claude Code's scratch folder computed from the workspace path and the user id: the same name as
  before here), `pull.sh` (`$HOME`; the old machine's `$HOME` must be the same, by SSH: the same
  user and paths, as before), `second-pass.sh` (`$HOME`),
  `validation/investigations/sdcflows-fieldmap-b/relay11/fetch_pybids020.sh` and
  `relay9/probe_obliquity.py` (`LARMORX_WORKSPACE`, default `~/VScode_projects/super_fmriprep`;
  probe_obliquity's names taken relative to the runs' folder: the same 5 lines as the old script
  printed, compared byte for byte). `bash -n` and shellcheck `-S warning`: only two warnings
  older than this relay (pull.sh SC2054 on rsync's `--info=progress2,stats1`, second-pass.sh
  SC2034 on an unused loop variable). `git grep` of the account name outside `docs/parts/` now
  finds only CLAUDE.md line 228 and the four records (below).
- **The PYPI.md files** (crates-nc, crates-gpl): macOS "11 or newer" as the wheels' tags say,
  with `larmorx[all]` needing 12 on Apple silicon (they said 12).
- **The plain image's licence wording** (`96a050a1`): "without the replica packages: commercial
  use allowed" instead of "Apache-2.0 and permissive licences only" (larmorx includes Eigen's
  MPL-2.0 routines, `[prep]` brings certifi's MPL-2.0, the Debian base its GPL tools) in README,
  CHANGELOG, RELEASING, the notes' footer, `docker/README.txt`, the Dockerfile's comment and
  description label, and `container.py`'s docstring. The next dry run builds the images with it.
- **Tests** (`validation/tests/test_release.py` 59 -> 63): the statuses table from a synthetic
  tools.toml (order, a panel record left out, `|` escaped, three refusals) and from the
  repository's (26 rows, every linked record's page exists); the table's check in the notes,
  `--write-statuses` (only the version's section, idempotent, refusals, the command line); the
  unwrapping; the repository's documents (CHANGELOG has the current version's section with the
  table and its link definition, dated as CITATION.cff; CITATION.cff's author (one), ORCID,
  licence and URLs; RELEASING.md exists and README and CHANGELOG link it).
- **Gates at `c36f7b0d`** (`oracles/logs/release-5a-gates.log`, boxed 64 GiB; no Rust changed, so
  relay 2c's extension and release programs stayed in place): the three Rust workspaces OK (fmt,
  clippy, test; 121 s); Python 1,377 passed, 12 skipped, 7 failed: the same 7 `test_smoke.py`
  version tests as relays 2a-4 (the shared venv's metadata says 0.0.1 in a worktree); without
  FSLDIR 917 passed, the same 7; ruff, ruff format, check_spdx, tables --check, set_version
  --check, licences --check, third_party --check, pypi_readme --check, actionlint, zizmor,
  shellcheck of the release-0.1 scripts, cffconvert, `changelog.py`'s notes: pass;
  `test_release.py` 63 passed. After `96a050a1` (text only): ruff, check_spdx, tables --check,
  set_version, pypi_readme, the notes and `test_release.py` (63) again: pass. No parity smoke tier:
  nothing under `crates/` or `python/larmorx` changed; no golden checksum changed.

## Relay 5a: findings and follow-ups (for 5b, 6 and the orchestrator)
- **Open for the user: PyPI's author field and e-mail.** No answer arrived during relay 5a:
  `pyproject.toml`'s `authors = [{ name = "Karel Lopez Vilaret" }]` (the three packages) is
  unchanged, with no e-mail. CITATION.cff has the full name and the university e-mail (asked for
  by the user). If the user wants PyPI to match: `authors = [{ name = "Karel Mauricio López
  Vilaret", email = "..." }]` in the three pyproject files (the e-mail is then public on PyPI).
- **The release date:** CITATION.cff's `date-released` and the CHANGELOG heading say
  **2026-11-12**. If the tag goes out on another day: `python -P scripts/release/set_version.py
  0.1.0rc1 --date <day>` and the heading's date in the same commit (`test_the_release_documents`
  checks they agree). CITATION.cff is not documentation-only for ci.yml (`**.md`, `docs/**`), so
  that commit runs CI.
- **The statuses table follows `validation/tools.toml`:** if a merge changes tools.toml before
  the tag (a handoff), run `python -P scripts/release/changelog.py 0.1.0rc1 --write-statuses`;
  the dry run of main's commit warns and the tag's `check` job stops otherwise.
- **Recommendation (the orchestrator's decision):** `skip-existing: true` in the three
  `gh-action-pypi-publish` steps would let "Re-run failed jobs" finish a publish job that failed
  after uploading part of its files (a re-run takes the same artifacts, so the same files);
  without it RELEASING.md says: yank and the next version. Not changed in 5a (a workflow change
  for a dry run to prove).
- **For relay 5b (the site):** README's anchors changed (`#installation`, `#usage`); the site's
  installation page can follow README's "Installation" (the same requirements and commands); the
  plain image's wording "without the replica packages: commercial use allowed".
- **HANDOFF material (relay 6):** `docs/overview.md` (its install lines and "not on PyPI") and
  `docs/licensing.md`'s "Nothing is on PyPI yet" (shared documents); CLAUDE.md line 228 (the two
  host names; the orchestrator's); next-steps: the "Releases on PyPI" items now done by this part
  (README, CHANGELOG, CITATION, RELEASING).
- **For the orchestrator, not edited (records and history):** the home path in
  `docs/validation/ants-registration-panel.json` (11 lines),
  `validation/investigations/ants-40-n4-nonpositive/analysis.json` and `results.json` (25
  each), `validation/investigations/itk-06-nonfinite/results.json` (6); `docs/parts/` (81
  files); git history.
- **Informational:** should GitHub show a release's text without keeping its line breaks after
  all, the unwrapping changes nothing visible (it only joins lines of one paragraph or item).

## Relay 5b: what was done (2026-11-11)
The site clone: `<scratch>/site` (from github.com/karellopez/larmorx_documentation at `d9f67d1`, the
published main), **branch `release-pages`**, the no-reply identity set in the clone. Three commits:
`5650433` (the installation page, the home page's install notes, mri-hmc's one line), `b8edd27` (the page
"Why Rust and Python" and the home page's link to it), `af701b6` (the installation page after its
screenshots: lists instead of tables, shorter code lines; the replica card's licences on two lines).
The clone is clean at `af701b6`. Runners and outputs in `<scratch>/r5b/` (NOTES before 5b:
`r5b/NOTES-before-5b.md`).
- **`content/installation.md`** (new, `/installation/`; `{%- set v = "0.1.0rc1" %}` at its top is the
  one place of the version, used 23 times): the release candidate and what is not ready (larmorprepx,
  FreeSurfer), links to the changelog and the roadmap; **pip** and the two extras (README's three
  lines, shorter comments), how pip treats a pre-release, the quotation marks, a virtual environment on
  Linux and macOS and in Windows' Command Prompt, `--version`, `lx.__version__`, the API pages; **the
  three packages and their licences in plain words** (README's table as a list: larmorx Apache-2.0 with
  NOTICE's MPL-2.0 line; larmorx-nc the FSL Licence, non-commercial use only, and `implementation`'s
  effect; larmorx-gpl the GNU GPL version 3); **what your computer needs** (README's requirements word
  for word, source builds); **standalone programs** (the six archive names as `archives.py` makes them,
  `larmorx-nc-`/`larmorx-gpl-` the same way, the folder layout, download and unpack with `v` and `t` on
  Linux and macOS, `Expand-Archive` on Windows, PATH; the programs are not signed by Apple or Microsoft:
  `xattr -d com.apple.quarantine` / `Unblock-File` after checking the file; **not tried on a Mac or
  through a browser download**: standard macOS and Windows behaviour, worded with "may"); **container
  images** (the two images as a list, `latest` only for a final release, Apptainer pull and exec, Docker
  and Podman, TemplateFlow's cache, `/opt/larmorx/README.txt`); **check a download** (`sha256sum -c
  --ignore-missing SHA256SUMS` and `shasum -a 256 -c --ignore-missing SHA256SUMS`, both run here on a
  test SHA256SUMS; the PowerShell check, run twice on the Windows helper with Windows PowerShell 5.1
  (True for the file, False after changing it; its temporary folder removed); `gh attestation verify`
  for files and images; PyPI's attestations); **updating and removing**.
- **`content/index.md`:** the hero's line (`Apache-2.0 · Linux, macOS and Windows · pip install
  larmorx`); the "Status" margin note (0.1.0rc1 a pre-release, a link to the installation page); the
  closing status note (`pip install larmorx`, `[prep]`, `[all]` with larmorx-nc non-commercial and
  larmorx-gpl GPL-3.0, a link; no more "not on PyPI", "build it from source" or `larmorx[exact]`);
  Findable and Accessible as README's FAIR lines (releases on PyPI and GitHub, CITATION.cff; wheels for
  six platforms, container images offered, linked); **also stale and corrected after README and the
  status table:** the three ways' cards (restrictive: FSL, tedana, Workbench; the replicas larmorx-nc
  for FSL and larmorx-gpl for t2smap, the AFNI replicas in larmorx since AFNI 26.3.05; the card's
  licences "larmorx-nc · FSL Licence / larmorx-gpl · GPL-3.0", it said GPL-3.0-or-later), the six
  ecosystem cards (AFNI: the four tools with both implementations in larmorx, it said 3dTshift with a
  replica in larmorx-gpl; FSL: the eight tools, it said head-motion correction only; ANTs: antsAI and
  Atropos added; LARMORX: sdcflows and t2smap added), the status section's versions (AFNI 26.3.05, FSL
  6.0.7.23, sdcflows 2.17.1, tedana 26.0.3; it said AFNI 25.2.09, FSL 6.0.7); the Rust card links "Why
  Rust and Python".
- **`content/tools/mri-hmc.md`:** larmorx-nc "by name, or `pip install "larmorx[all]"`", with a link to
  the installation page (it said by name only). The other tool pages' `pip install larmorx-nc` /
  `larmorx-gpl` lines stay true.
- **`content/why-rust-and-python.md`** (new, `/why-rust-and-python/`; next-steps' follow-up "The site:
  a page 'Why Rust and Python'", every point of it): the original tools are Unix programs (born on
  Unix workstations, Cox 1996 quoted from its abstract; the scripts, FSL's BET page and afni_proc.py's
  help quoted; Unix features; `long`, `long double`, GCC extensions; X11 and Motif, FSL's X-server note;
  small teams; WSL, which the three projects' Windows notes recommend, each page fetched 2026-11-11),
  Glatard et al. 2015 quoted from its abstract (Europe PMC); what Rust gives; what LARMORX's rules add
  (the 18 CORE-MATH ports as `crates/larmorx-core/PROVENANCE.md` lists them (exp, log, pow, sin, cos,
  tan, asin, acos, atan2, cbrt, tanh, expf, logf, powf, sinf, cosf, tanhf, cbrtf; next-steps' list of
  2026-10-31 had 17, without tan), the glibc functions for `reproduce` as `docs/architecture.md`
  gives them, matrix order, vnl/Eigen/SciPy/HDF5 in Rust, threads, seeds, golden tests); why Python;
  the limits; six references, **every DOI checked against Crossref's record** (title, journal, volume,
  pages, year, authors). Facts only from documentation, papers and the repository's docs; no upstream
  source opened. "Rust made the solution practical; the rules made it work." kept.
- **Checks:** `tools/build.py --main-repo <the part's worktree>`: 33 pages, **0 problems**, for the
  branch and for a scratch copy with the handoff applied (`<scratch>/r5b/site-handoff`, below).
  **Inventory** (`validation/investigations/afni-volreg-original/site_inventory.py`, every built page)
  against main `d9f67d1`'s build (`r5b/before_site`): the branch: the home 4,961 -> 5,060 words,
  mri-hmc 27,823 -> 27,829, two new pages, the other 28 pages unchanged and **byte-identical**; with
  the handoff: every page's words up (the footer's two new links), the roadmap one table row more, no
  count down anywhere (exit 0). **Screenshots** (`r5b/shots/`, 1440 and 390, light and dark): both new
  pages whole (`after-installation-*` the first round, `after2-installation-*` after the fixes,
  `after-why-rust-and-python-*`), the home page's changed parts before and after (`before-home1..6`,
  `after-home1..6`: the hero's line, "What LARMORX is" with its cards, the features, FAIR, the status
  note, the header with "Install"), the roadmap's release paragraph with the handoff
  (`after-roadmap-release-*`). Looked at: the installation page's tables broke `larmorx`/image names
  inside the word in narrow cells (now lists), three code lines ran past the column at 1440 (shortened);
  at 390 the code blocks scroll sideways as the home page's do. The evidence was not exported again
  (no JSON, no blobs changed): no number or viewer check needed.

## Relay 5b: findings and follow-ups (for relay 6 and the orchestrator)
- **The order of publication:** the site's `release-pages` links `repo:CHANGELOG.md` and
  `repo:CITATION.cff`, which exist only on the larmorx branch: built against main `459a5a21` the site
  has 2 problems (those two files), against the branch 0. So: **merge and push the larmorx branch
  first; merge and push the site only when 0.1.0rc1 is on PyPI** (the pages say `pip install larmorx`
  and give the release's download addresses and image names, which do not exist before the upload).
  The handoff below applies at the site's merge.
- **HANDOFF for the site's shared files (relay 6 writes them into HANDOFF.md/toml; the orchestrator
  applies them at the site's merge).** Exact replacements, as `<scratch>/r5b/handoff_site.py <site>`
  applies them (tested on the scratch copy, 0 problems):
  - `site.yml`, `nav`: after `  - { title: Status, url: "/#status" }` insert
    `  - { title: Install, url: "/installation/" }` (the header at 1440 holds the eight; at 390 the
    menu).
  - `site.yml`, `docs`: after `  - { title: Overview, url: "/" }` insert
    `  - { title: Installation, url: "/installation/" }` and
    `  - { title: Why Rust and Python, url: "/why-rust-and-python/" }` (sidebar and footer).
  - `data/status.yml`, `planned`: the item `title: Releases on PyPI` / `text: for all six platforms.`
    becomes `title: Release 0.1.0` / `text: the final release, once the release candidate 0.1.0rc1
    installs cleanly on all six platforms.` (eco `[larmorx]` kept; remove the item at 0.1.0).
  - `content/roadmap.md` (the orchestrator's page, at the publication): as
    `<scratch>/r5b/handoff_roadmap.py <site> <release date>` does: "## The first release {#release}"'s
    four bullets become "The release candidate 0.1.0rc1 is out ([installation](/installation/)).
    Release 0.1.0 follows once it installs cleanly on all six platforms."; a Completed row at the top
    of the table: `| <date> | **The first release: 0.1.0rc1, a release candidate.** The three packages
    on PyPI at one version: `larmorx` (Apache-2.0), `larmorx-nc` (the FSL replicas; the FSL Licence,
    non-commercial use only) and `larmorx-gpl` (tedana's `t2smap`; the GNU GPL version 3), `pip install
    "larmorx[all]"` for all three; wheels for Linux, macOS and Windows on x86-64 and ARM, built and
    tested on every platform by a release workflow on the version's tag, with Windows' C runtime built
    into the programs and smaller binaries; the standalone programs for each platform and two container
    images for Apptainer, Docker and Podman; every file with its SHA-256 and GitHub's record of its
    build; the files rehearsed on clean Linux systems and on Windows before the upload. With it, the
    pages [Installation](/installation/) and [Why Rust and Python](/why-rust-and-python/) |`; "Last
    updated: <date>."; the front matter's description "the first release" -> "the release after
    0.1.0rc1".
- **For relay 6 in the larmorx branch (suggestions):** RELEASING.md's step 7 (rc1 -> 0.1.0) could
  name the site's version lines: `content/installation.md`'s `{%- set v = "0.1.0rc1" %}`, its first
  paragraph ("a release candidate") and the pip pre-release sentence; `content/index.md`'s "Status"
  margin note; the roadmap's release section; status.yml's "Release 0.1.0" item (removed); and the
  publication order above. README's Installation could link the site's page
  (https://karellopez.github.io/larmorx_documentation/installation/) in one line. Commit 5b's helpers
  under `validation/investigations/release-0.1/site/` (with README lines): `r5b/handoff_site.py`,
  `r5b/handoff_roadmap.py`, `r5b/edit_index.py` (the home page's first edit), `r5b/shots.sh`,
  `r5b/shots2.sh`, `r5b/crop.py`, `r5b/sumtest.ps1`, `r5b/sumtest2.ps1` (run with
  `validation/investigations/release-0.1/winps.sh`).
- **next-steps (HANDOFF):** the follow-up "The site: a page 'Why Rust and Python'" is done (site
  `b8edd27`); its list said 17 CORE-MATH ports, there are 18 (`tan` since 2026-11-03).
- **Not tried:** the macOS and Windows download-mark advice (no Mac here; the Windows helper's runs
  are over SSH, without a browser's mark): check it on rc1 if a Mac or a browser download is at hand.
- **Not changed (true as they are):** the tool pages' `pip install larmorx-nc` and `pip install
  larmorx-gpl` lines; the AFNI pages' history of larmorx-gpl until 2026-10-29.

## Relay 6: what was done (2026-11-11)
Runners and outputs in `<scratch>/r6/` (`quick.sh` the quick checks, `gates.sh` the Python gates,
`make_handoff.py` the handoff's builder (each `old` cut from the files by unique markers, the TOML
checked to parse back to the same operations), `claude_machines.py`, the comparison folders
`same/`, `same2/`; NOTES before relay 6 in `r6/NOTES-before-6.md`).
- **`skip-existing: true`** in the three `gh-action-pypi-publish` steps (FROM THE ORCHESTRATOR,
  2026-11-11), with the workflow's comment saying why it is safe: "Re-run failed jobs" takes the
  same files (the run's artifact) and uploads only those PyPI does not have; files built again
  cannot reach PyPI, since "Re-run all jobs" runs `check` again, whose `pypi_state.py absent` is
  strict on a release and stops the run once any file of the version is up. `test_release.py`'s
  workflow test checks each publish step has `attestations: true` and `skip-existing: true`
  (negative control: with one removed the check fails). RELEASING.md step 5: a partial upload is
  finished by the re-run; yanking (its menu path moved to the `verify-pypi` bullet) only for a
  broken upload; "Re-run all jobs" stopped by `check` anyway.
- **RELEASING.md:** step 6.5, the documentation site after the upload (why; the lines that name
  a release: `content/installation.md`'s `{%- set v = ... %}`, the home page's Status note, the
  roadmap's release section and table, `status.yml`'s planned item; `tools/build.py --main-repo`
  0 problems); step 7, the words a final release changes (README's status line and pre-release
  sentence; on the site the installation page's first paragraph and pre-release sentence, the
  Status note, the roadmap's section; `status.yml`'s "Release 0.1.0" removed).
- **README.md:** the Rust bullet links "Why Rust and Python", the installation section the site's
  installation page (absolute links: `pypi_readme.py --check` passes, 59 lines rewritten as before).
- **Relay 5b's helpers** in `validation/investigations/release-0.1/site/` (`handoff_site.py`,
  `handoff_roadmap.py`, `edit_index.py`, `shots.sh`, `shots2.sh`, `crop.py`, `sumtest.ps1`,
  `sumtest2.ps1`): SPDX headers, ruff's form, functions with a usage line, the screenshot scripts'
  folders as arguments (`<site checkout> <work folder>`); no local path, host name or account
  name in them. On the same inputs the three edit scripts write the same files as relay 5b's
  originals (cmp, also after ruff's formatting); `edit_index.py` on d9f67d1's home page gives site
  commit `5650433`'s byte for byte; a second application is refused. The investigation README's
  section "Relay 5b: the documentation site".
- **Checks** (`oracles/logs/release-6-gates.log`, boxed 64 GiB): actionlint 1.7.12 (both
  workflows) and zizmor 1.32.0 `--offline` clean (`--persona=auditor`: the same 5 informational
  findings as before); `test_release.py` 63 passed; ruff, ruff format, check_spdx, tables --check,
  set_version --check v0.1.0rc1, licences --check, third_party --check, pypi_readme --check, the
  release notes (15,405 characters), shellcheck of the new scripts: pass; `gates-python.sh
  --no-build`: 1,377 passed, 12 skipped, 7 failed: the same 7 `test_smoke.py` version tests as every
  relay since 2a (the shared venv's metadata says 0.0.1 in a worktree); without FSLDIR 917 passed,
  the same 7; no stray `larmorx/.claude/worktrees/oracles`. No Rust changed (no Rust gates needed),
  no golden checksum changed, the site unchanged by relay 6.
- **The handoff** (HANDOFF.md says what, when and why): `apply_handoff.py --check` against a trial
  merge of `8f2d2bd7` into `459a5a21` (scratch worktree, `--no-commit`): 30 of 30; the site's
  against a trial merge of `release-pages` into `d9f67d1`: 7 of 7. Applied in the trial trees: the
  site's files equal relay 5b's scripts' output, the site builds with 0 problems against the merged
  larmorx tree and its `_site` is byte-identical to relay 5b's handoff build (so 5b's inventory and
  screenshots hold); `tables --check`, `check_spdx.py` pass; the ten changed documents' relative
  links resolve but nine older ones of `docs/status.md` (written from the repository's root; in
  HANDOFF.md). The trial worktrees removed; the main checkout untouched (`git status` clean).
- **For the audit, noted:** one `git grep` whose pathspec (`docs/*.md`) also matched `docs/parts/`
  printed matching lines of some briefs and handoffs (install hints, a brief's oracle paths);
  relay 6 then left `docs/parts/` out and edited no clean-room file.

## Inventory (measured 2026-11-10 on the main machine, Linux x86-64, at 459a5a21, version 0.0.1)
Logs: `oracles/logs/release-1-inventory-build.log`, `release-1-sdist-build.log`,
`release-1-golden-experiments.log`. Local builds are tagged `manylinux_2_34` (the host's glibc
2.35); CI and the release build in the manylinux_2_28 container, as `ci.yml` does.

**Wheels (Linux x86-64; all far below PyPI's 100 MB per file):**

| Package | today's release profile | strip only | thin LTO + strip | fat LTO + 1 CGU + strip |
|---|---|---|---|---|
| `larmorx` (abi3 wheel; `_core.abi3.so` unpacked) | 15.2 MB (78.1 MB) | 13.4 (51.0) | 13.3 (50.2) | 13.9 (41.2) |
| `larmorx-nc` (bin wheel; program) | 6.4 MB (8.6) | | | 6.2 (6.8) |
| `larmorx-gpl` (bin wheel; program) | 1.3 MB (3.3) | | | 1.05 (2.0) |

Build times at 28 threads: main from the seed (deps cached) 95 s; main fat LTO from scratch 345 s
(about 100 s of parallel crates, then about 245 s of single-threaded LTO of `_core`), thin LTO from
scratch 90 s; nc fat LTO 48 s, gpl 19 s. CI's main wheel (cached deps, release) on the six
platforms: 13.0-15.6 MB (CI run 42766636707, artifacts `wheel-<platform>`).
- The main wheel holds `larmorx/` (124 files), `LICENSE`, `NOTICE`, `LICENSES/*` (3) and a CycloneDX
  SBOM (maturin's, `sboms/larmorx-py.cyclonedx.json`: 65 crates with their licences). The nc wheel:
  the program, `data/share/larmorx-nc/fsl-data/` (13 files) and 283 licence files (FSL Licence,
  notices, `fsl-source/`, the Rust translation). The gpl wheel: the program and 22 licence files.
- METADATA: `Development Status :: 2 - Pre-Alpha`; no Author; URLs Repository, Issues (+ Source for
  the companions); `License-Expression` Apache-2.0 / GPL-3.0-only / LicenseRef-FSL (PEP 639,
  Metadata 2.4; LicenseRef-* is valid SPDX). The summary promises FreeSurfer ports and
  Workbench-like tools that do not exist yet. `twine check --strict` passes for all six files.
- The SBOMs: `larmorx-gpl-cli`'s crate licence is `GPL-3.0-or-later` (the workspace's) while the
  pyproject says the program is GPL-3.0-only: set `license = "GPL-3.0-only"` in
  `crates-gpl/larmorx-gpl-cli/Cargo.toml` (a manifest, not source). The main SBOM's
  `larmorx-afni-replica` is `Apache-2.0 AND CC-BY-4.0 AND SunPro` (see D9).
- **README on PyPI:** renders, but its 98 relative links (`docs/...`, `NOTICE`, `CLAUDE.md`, ...)
  and 3 anchors would point into pypi.org (404). The companions' READMEs are directory pages for
  developers ("This directory holds ...", "Not on PyPI", build commands), with 3 relative links.

**sdists:**
- `larmorx`: `maturin sdist` works: 3.2 MB (12.0 MB unpacked, 660 files): `crates/` (only the 16
  crates `larmorx-py` needs), `python/larmorx`, licences, `Cargo.toml`/`Cargo.lock`, README; no
  `crates-nc/`, `crates-gpl/`, tests or validation. It builds a wheel identical in file list
  (`maturin build` in the unpacked sdist). It does not carry `rust-toolchain.toml` (a source build
  uses the user's Rust; no `rust-version` is declared).
- **`larmorx-nc` and `larmorx-gpl`: `maturin sdist` FAILS** (1.15.0 and 1.16.0): "File
  larmorx_nc-0.0.1/Cargo.toml was already added from <repo>/Cargo.toml, can't add it from
  <repo>/crates-nc/Cargo.toml": their path dependencies (`../crates/larmorx-core` ...) belong to
  the root workspace, so maturin puts the sdist's root at the repository root and then places both
  workspaces' manifests at the same spot. `sdist-generator = "git"` works but leaves out
  `../crates/` (not buildable). **Working approach (measured):** a staging copy of the tracked tree
  with the companion's `pyproject.toml` at its root (every path prefixed `crates-nc/`, `manifest-path
  = "crates-nc/larmorx-nc-cli/Cargo.toml"`); `maturin sdist` then gives nc 1.84 MB (380 files:
  `crates-nc/` with `fsl-source/`, `crates/larmorx-core`) and gpl 0.89 MB (`crates-gpl/` with the
  two crates, `crates/larmorx-core`, `-hdf5`, `-interp`, `-io`, `-optim`, `-transform`), and both
  build a wheel. Two defects of that sdist to fix: (1) **the wheel data is missing** (only the two
  licence-listed files of `wheel-data/` are in it; the 11 images and settings are not: the wheel
  built from it is 4.96 MB instead of 6.39): add them with `[tool.maturin] include = [{ path =
  "...wheel-data/**/*", format = "sdist" }]`; (2) the licence files then sit under `crates-nc/` in
  that wheel's `.dist-info/licenses/` (their paths follow the pyproject): build the published
  companion wheels **from their sdists** too, so wheel and source build agree. No maturin issue
  matches (searched "was already added from": 9 closed issues, all README duplicates): an upstream
  report could be drafted (`docs/upstream-reports/`, the user files it).

**The version in the outputs (experiment B: the three workspaces at `0.1.0-rc.1`, then reverted):**
- Python golden suite: **4 cases change** (`cli.ants.antsApplyTransforms.outputs`,
  `cli.ants.antsRegistration`, `.fields`, `.syn`; outputs `composite.structure`,
  `inverse.structure`, `state.structure`): larmorx's ITK HDF5 writer records the crate version as
  `/OSVersion` (`larmorx_io::itk_hdf5::WriteOptions::default`, `env!("CARGO_PKG_VERSION")`), and
  `_h5_structure` hashes the strings. 301 others unchanged.
- `larmorx-gpl` golden: **9 cases change**: tedana's `dataset_description.json` field `"Python":
  "larmorx-gpl <version> (...)"`.
- AFNI replicas' golden and `larmorx-nc`'s golden: unchanged; `larmorx-core`'s version test passes
  with `0.1.0-rc.1`.
- Predicted, not run (the shared venv's larmorx is an editable install): `tests/python/test_smoke.py`
  fails 4-5 tests with Cargo's form, as it compares `_core.__version__` and `larmorx --version` with
  `importlib.metadata.version("larmorx")` (`0.1.0rc1`).
- Records: about every parity suite writes `larmorx {larmorx.__version__}` into its record's tool
  line, so verify-merge's `compare_records.py` would report "details differ" across the board after
  the bump (it strips times, `/tmp` paths and addresses only).

**The size profile at today's version (experiment A, fat LTO + 1 codegen unit + strip):** the
Python golden suite (305 passed), the AFNI replicas' golden, `larmorx-nc`'s golden and
`larmorx-gpl`'s golden **all pass** on Linux x86-64: early evidence for sub-task 2 (its formal proof
still needs the parity smoke tier on that build, and the six platforms in the release workflow).

**[prep]'s dependencies (pip dry runs, wheels only, `<scratch>/inv/deps/`):** all 46 packages
(numpy 2.5.4, scipy 1.18.1, scikit-learn 1.9.1, pandas 3.1.1, matplotlib 3.11.2, pillow 13.0.0,
nilearn, templateflow with pybids, sqlalchemy 2.1.8, ...) have wheels on all six platforms for
CPython 3.12 and 3.14, **Windows arm64 included**; Linux needs at most manylinux_2_28; macOS needs
**12+ on Apple silicon** (scipy, scikit-learn: `macosx_12_0_arm64`; macOS 11 fails to resolve) and
**10.15+ on Intel** (scipy). larmorx itself needs numpy only (macOS 11 / 10.13).

**CI today (`ci.yml`, run 42766636707 at 8f5eff7f):** 13 jobs, 112 job-minutes: `lint` 0.8;
`gpl` x6 2.1-5.2 (cargo fmt/clippy/test of crates-nc and crates-gpl; no wheels); `test` x6
11.7-16.6 (cargo test 3.6-6.0, the wheel build 4.6-9.1 with maturin-action 1.15.0 and
manylinux 2_28 on Linux, `pytest` 1.1-1.8, Python 3.14 on linux-x64). It never builds the companion
wheels or any sdist. Its actions are tag-pinned (`@v7`), not SHA-pinned. A tag push does not
trigger it (`branches: [main]`). The release workflow reuses its matrix (runners
`ubuntu-24.04`, `ubuntu-24.04-arm`, `macos-15`, `macos-15-intel`, `windows-2025`,
`windows-11-arm`; targets; manylinux 2_28) and its test commands, in a separate file: `ci.yml`
unchanged.

**Names:** `larmorx`, `larmorx-gpl`, `larmorx-nc`, `larmorx-freesurfer`, `larmorprepx` all free on
PyPI (404, 2026-11-10). A pending publisher does not reserve a name: upload rc1 soon after the user
adds them. `ghcr.io/karellopez/larmorx` does not exist yet.

**This machine:** Apptainer 1.4.5 (runs images as the user); Docker 28.3 is installed but the
account is not in the `docker` group (no daemon access); no podman or buildah; no actionlint. So an
OCI image cannot be built here; an Apptainer image from the same base and the same pip commands
can. Network to PyPI and GitHub's API works without a login (60 API requests per hour); CI's
artifacts cannot be downloaded without a login.

**win-laptop (ssh works):** Windows 11 Education 25H2 (build 26200), x64 only (arm64 only in CI);
the SSH session has an elevated administrator token; **Visual C++ 14.50 runtime is installed
system-wide** (`vcruntime140.dll`, `msvcp140.dll` in System32), so a missing runtime cannot show
there; Windows Sandbox is supported by the edition but not enabled (Hyper-V's hypervisor is
running); `uv` is installed (`C:\Program Files\uv`), `python` on the PATH is the Store alias, git
present, no Rust; 165 GB free.

## Design

**Versions (D2, D3, D4).** One version for the three packages, set in one place per workspace:
`[workspace.package] version` in `Cargo.toml`, `crates-gpl/Cargo.toml`, `crates-nc/Cargo.toml`
(Cargo form `0.1.0-rc.1`; maturin turns it into PEP 440 `0.1.0rc1`; the final `0.1.0`), plus the
`version = "0.0.1"` of every path dependency in the three `[workspace.dependencies]` (16 + 6 + 8),
the three `Cargo.lock` files (workspace members' versions; refresh with `cargo update -w`), and the
`[all]` pins (`"larmorx-nc==0.1.0rc1", "larmorx-gpl==0.1.0rc1"`, static in `pyproject.toml`). A
script does all of it and checks it: `scripts/release/set_version.py <pep440>` and `--check
[<tag>]` (used by the release workflow's first job and by a test). Tags `v0.1.0rc1`, `v0.1.0`.
- `larmorx_core::VERSION` in PEP 440 form (a `build.rs` in larmorx-core, or a const fn), so
  `larmorx --version`, `_core.__version__` and `larmorx.__version__` equal pip's version
  (`test_smoke.py` stays as it is); `larmorx-io`'s `/OSVersion` default uses it too. Left in Cargo's
  form until someone allowed edits them (identical from 0.1.0 on): the clean-room AFNI crate's
  `implementation:` lines (`crates/larmorx-afni/src/cli/{tshift,automask,volreg}.rs`, a clean-room
  agent) and the `larmorx-nc --version` / `larmorx-gpl --version` lines and tedana's description
  (replica crates' `src`, a replica-side agent).
- The golden harnesses made independent of the version: `tests/golden/registry.py`'s
  `_h5_structure` writes `<version>` for the running version (then the 4 cases are re-recorded once,
  from a clean commit, `record_golden.py`; nothing else may change); `crates-gpl/larmorx-gpl-cli/
  tests/golden.rs` likewise for the description's `larmorx-gpl <version>` (replica test code: the
  orchestrator or a replica-side agent; then its 9 cases re-recorded once). Without it, every
  version bump re-records 13 cases.

**The size profile (D1).** A new profile `[profile.dist]` (`inherits = "release"`, `lto = "fat"`,
`codegen-units = 1`, `strip = "symbols"`) in the three workspaces, used by the release workflow
(`maturin build --profile dist`, `cargo build --profile dist`) and by local release rehearsals; the
everyday `release` profile, CI, verify-merge and the agents' builds stay as they are (from scratch
the main crate took 345 s with fat LTO against 90 s with thin LTO, mostly one single-threaded LTO
step that cached dependencies do not shorten; today's release build from cached dependencies 95 s). Proof in sub-task 2: the four golden suites (done here on
Linux, above) and the parity smoke tier on a dist build; the release workflow's test job then runs
the golden tests on the six platforms on the dist builds.

**Windows' C runtime (D15).** `.cargo/config.toml` at the repository root:
`[target.'cfg(all(windows, target_env = "msvc"))'] rustflags = ["-C",
"target-feature=+crt-static"]`. Cargo looks for its config from the working directory upwards, so
this one file serves the three workspaces whenever a build starts inside the repository (a copy in
`crates-nc/` and `crates-gpl/` would only matter for builds started from outside with
`--manifest-path`; `RUSTFLAGS`, if ever set, would override it). It applies to CI's Windows builds too (the golden tests there prove
the bits; the maths is larmorx's own). Not in the sdists (a source build on Windows has MSVC and
its runtime). Checked in the release workflow by a PE import-table check of every `.exe` and
`.pyd` (no `VCRUNTIME140*.dll`, `MSVCP140*.dll`, `api-ms-win-crt-*`, `ucrtbase.dll`): a small
Python script, no MSVC tools needed, so it can also run here on downloaded files.

**Metadata (D9, D10, D13).** All three: `Development Status :: 3 - Alpha`, `authors = [{ name =
"Karel Lopez Vilaret" }]` (the copyright line of NOTICE; no e-mail address), URLs Homepage and
Documentation `https://karellopez.github.io/larmorx_documentation/`, Repository, Issues, Changelog
(`.../blob/main/CHANGELOG.md`), Source for the companions; classifiers `Programming Language ::
Python :: 3.12`, `3.13`, `3.14`, `Implementation :: CPython`, `Environment :: Console`; summaries
that state what exists today (no FreeSurfer or Workbench promise); `rust-version` in the three
`[workspace.package]` (the pinned 1.99, unless measured lower) for source builds. READMEs for PyPI:
the main README's relative links and anchors made absolute at build time, pinned to the tag
(`scripts/release/pypi_readme.py`, run on the runner's checkout before `maturin build`/`sdist`);
the companions get user-facing PyPI descriptions of their own (`crates-nc/PYPI.md`,
`crates-gpl/PYPI.md`; `readme =` points there): what it is, **"non-commercial use only" first** for
larmorx-nc, that `larmorx` runs it as a separate program and that `implementation="auto"` then
chooses the replica (so installing larmorx-nc puts FSL's terms on the default results), the tools
and upstream versions, the licence in plain words, absolute links; their directory READMEs keep the
developer text ("Not on PyPI" updated).

**The release workflow (`.github/workflows/release.yml`; D5-D8, D11, D16, D18).** Triggers:
`push: tags: ["v*"]` (a release), `workflow_dispatch` with `dry_run` (default true), and (D8) a
push to the branch `release/dry-run` (a dry run the orchestrator can start with `git push`: a
dispatch needs a login or a token, which the orchestrator does not have). Anything that is not a
`v*` tag push is a dry run: build and test, nothing uploaded, pushed or released. Top-level
`permissions: {}`; each job its own; every action pinned by commit SHA with its version in a
comment (newest releases, 2026-11-10: checkout v7.0.1 `3d3c42e5...`, setup-python v7.1.0
`6d4fd22b...`, upload-artifact v7.0.2 `cf430e03...`, download-artifact v8.0.2 `9000827c...`,
attest-build-provenance v4.2.2 `4d101475...`, maturin-action v1.51.0 `e83996d1...`,
gh-action-pypi-publish v1.14.3 `7245906f...`, setup-buildx v4.4.1 `f87e5991...`, login v4.8.0
`8aa5831f...` (5 days old: take the one before it), build-push v7.4.0 `c3c9e263...`, metadata
v6.2.0 `dc802804...`; full SHAs in `<scratch>/inv/action_pins.py`'s output: re-run it); `actionlint`
(a release binary in scratch) and `zizmor` (pip) checked locally.

| Job | Runs on | Needs | What | Est. minutes |
|---|---|---|---|---|
| `check` | ubuntu | | `set_version.py --check <tag>` (tag = the three versions = the `[all]` pins), a `CHANGELOG.md` entry, the tag's commit is on `main`, CI passed for it (API, `actions: read`) | 1 |
| `build` x6 | CI's six runners | check | the three wheels (`--profile dist`; Linux in manylinux_2_28; the companion wheels from their sdists), the standalone `larmorx`/`lx` (Linux: maturin `-b bin -m crates/larmorx-cli/Cargo.toml` in the same container, binaries taken from that wheel; elsewhere `cargo build --profile dist`), the PyPI READMEs prepared first; `MACOSX_DEPLOYMENT_TARGET` set (11.0) | 6 x 25-45 = about 200 |
| `sdist` | ubuntu | check | the three sdists (companions through the staging script, from `git archive`), `twine check --strict` on every file; then each sdist installed from source in a fresh venv (default release profile) and its smoke checks | 25 |
| `test` x6 | the six runners, Python 3.12, no Rust | build | `pip install --no-index --find-links dist larmorx larmorx-gpl larmorx-nc`, then `pip install --find-links dist "larmorx[all]==<v>"`; `python -m pytest` (as CI: tests/python + golden; parity skips) with `LARMORX_NC_BIN`/`LARMORX_GPL_BIN` at the installed programs; `larmorx --version`, `lx --version`, `larmorprepx --version`, `larmorx-gpl --version`, `larmorx-nc --version` (normalised compare); a `larmorx` call that runs each replica; larmorx-nc finds its FSL data; the standalone programs run; the PE import check (Windows), `otool -L` (macOS), the glibc symbol versions <= 2.28 (Linux); linux-x64 also 3.13 and 3.14 (no golden, as CI) | 6 x 6 + 4 = 40 |
| `container` x2 | ubuntu-24.04, ubuntu-24.04-arm | build, sdist | two images from the Linux wheels (D5), `docker run` smoke (versions, the golden tests inside with the checkout's `tests/` mounted), push by digest only on a tag | 2 x 10 = 20 |
| `publish-gpl`, `publish-nc`, `publish-larmorx` | ubuntu | check, build, sdist, test, container; tag only | environments `pypi-gpl`, `pypi-nc`, `pypi`; `id-token: write`; each downloads only its package's wheels and sdist; `gh-action-pypi-publish` (attestations on); `publish-larmorx` first waits (at most 15 min) until both companions' version is on PyPI's JSON API (D6) | 3 x 1-5 |
| `container-tags` | ubuntu | the three publish jobs | the multi-arch tags from the pushed digests (`buildx imagetools create`): `<v>`, `<v>-all`; `latest`, `latest-all` for a final release only; `packages: write` | 1 |
| `github-release` | ubuntu | the publish jobs, container-tags | `gh release create v<v>` (`contents: write`; `--prerelease` for an rc), notes from CHANGELOG.md's section, every wheel and sdist, the standalone archives, `SHA256SUMS`; build-provenance attestations of those files (D16) | 3 |
| `verify-pypi` x6 | the six runners | the publish jobs | `pip install "larmorx[all]==<v>"` from PyPI itself (retries while the CDN catches up), the golden tests and the smoke checks | 6 x 5 = 30 |

About 290 job-minutes for a dry run (`check`, `build`, `sdist`, `test`, `container`) and 330 for a
release, about 60-75 minutes of wall time plus the approval; four runs for rc1 and 0.1.0 (a dry
run before each tag): about 1,250 job-minutes in all. Standard runners are free for a public
repository; GitHub Free runs 20 jobs at once (5 on macOS), within which this fits. (Splitting `build`
into a main job and a programs job per platform, 12 jobs, would halve its wall time at about 15
more job-minutes: relay 3 decides from the dry run's times.)

**Standalone programs (D11).** Per platform and program an archive (`.tar.gz`; `.zip` on Windows):
`larmorx-<v>-<target>` (`larmorx`, `lx`, LICENSE, NOTICE, LICENSES/, a short README: the Rust
tools only; `larmorx sdcflows sdcflows` needs the Python package); `larmorx-gpl-<v>-<target>` (the
program, crates-gpl's LICENSE, NOTICE, LICENSES/ and its sdist, the Corresponding Source);
`larmorx-nc-<v>-<target>` (`bin/larmorx-nc`, `share/larmorx-nc/fsl-data/` as the wheel installs it,
the FSL Licence and notices, and its sdist: the FSL Licence wants "all original and amended source
code" in every transmitted product). The companions' programs are taken from their tested wheels
(the same files); Linux binaries need glibc 2.28 (not musl: Rust's `std` maths would come from musl).

**The container (D5, D7, D17).** `ghcr.io/karellopez/larmorx`, linux/amd64 and linux/arm64, built
natively on the two Linux runners from the released wheels (`Dockerfile`, base
`python:3.12-slim-trixie` pinned by digest: the Python CI's golden tests run on), **two images per
release**: `:<v>` = `larmorx[prep]` (Apache-2.0 and permissive code only: safe for commercial use)
and `:<v>-all` = also `larmorx-gpl` and `larmorx-nc` (labelled non-commercial; with the two
companions' sdists under `/opt/larmorx/source/`); `latest` and `latest-all` move only on a final
release. Why two: with larmorx-nc installed, `implementation="auto"` runs the FSL replicas, so a
single image would put FSL's non-commercial terms on every default result. OCI labels: source,
version, revision, documentation, title, description, `licenses` (`Apache-2.0` / `Apache-2.0 AND
GPL-3.0-only AND LicenseRef-FSL`). No ENTRYPOINT; `CMD ["larmorx", "--help"]`, so `docker run
<image> larmorx ...` and `apptainer exec larmorx.sif larmorx ...` both work; `MPLCONFIGDIR` and
caches under `/tmp`; templates downloaded on first use (TemplateFlow into the user's home under
Apptainer; a mounted volume with Docker, as the docs say); `pip freeze` saved in the image and as a
release file. The images are pushed only after PyPI accepted the files (by digest, tagged after).
Usable as `apptainer pull docker://ghcr.io/karellopez/larmorx:0.1.0rc1`, `docker run`, `podman run`.

**larmorprepx shown as not ready (D14).** The `larmorprepx` command stays (its tests check
`--version` and the `[prep]` message): its description and final message say "not ready yet in
larmorx 0.1: its field-map stages exist as Python functions; the anatomical (FreeSurfer) and BOLD
stages, the confounds and the final validation come in later releases", with the roadmap's URL; exit
status 1 as now. README, CHANGELOG, the site and the PyPI description call it a preview; the
`[prep]` extra is "what larmorprepx will need". No import-time warning.

**Clean machines (sub-task 4).** Here: (a) Linux wheels built as CI builds them, in the
manylinux_2_28 image through Apptainer (`docker://quay.io/pypa/manylinux_2_28_x86_64`, rustup in
scratch); (b) installed and tested in `docker://python:3.12-slim-trixie` (no compiler, no Rust) and
in the manylinux_2_28 image's own CPython 3.12 (AlmaLinux 8, **glibc 2.28: the floor we claim**),
with `--containall`, a scratch home: wheels, `larmorx[all]`, the golden tests, the smoke checks, the
standalone programs; (c) the sdists built from source in `docker://python:3.12` (gcc present) with
rustup in scratch (a source build needs Rust and a C linker: say so in the README); (d) the image:
an Apptainer definition with the Dockerfile's base and pip commands (`spython` can convert the
Dockerfile), the same smoke. Not here: linux-aarch64, macOS, Windows arm64 (the release workflow's
`test` jobs, then `verify-pypi` from PyPI). The real OCI image is first pulled here after rc1's push.
win-laptop: the dry run's Windows wheels cannot be fetched without a login, so its check is on rc1
itself (`uv venv`, `uv pip install "larmorx[all]==0.1.0rc1"`, the golden tests from a clone of the
tag, the standalone `.exe` archives from the GitHub Release). **For a truly clean Windows** the user
enables Windows Sandbox once (an administrator PowerShell: `Enable-WindowsOptionalFeature -Online
-FeatureName Containers-DisposableClientVM -All`, then a restart; or "Turn Windows features on or
off" -> Windows Sandbox): then the `.exe` programs run where no Visual C++ runtime exists (25H2's
`wsb` command line may let a script drive it over SSH; else the user starts one prepared `.wsb`
file). Without the sandbox, the PE import check is the evidence.

## Decisions for the orchestrator (each with a recommendation)
- **D1 size profile:** a separate `dist` profile (fat LTO, 1 CGU, strip) for releases only, vs
  changing `release` (every CI and agent build of the main crate then pays about 4 minutes of
  single-threaded LTO here, more on CI's runners). Thin LTO + strip gives the same download size
  (13.3 MB) at a quarter of the time but a larger installed library (50 MB against 41).
  **Recommend `dist` with fat LTO, as the brief asks.**
- **D2 version forms:** Cargo `0.1.0-rc.1`, PyPI and tag `0.1.0rc1`; `larmorx_core::VERSION` in PEP
  440 form; the AFNI clean-room `implementation:` lines and the companions' `--version` keep Cargo's
  form until the owners of those crates switch them (identical from 0.1.0). **Recommend so.**
- **D3 golden and the version:** normalise the version in the two harnesses and re-record the 4 + 9
  cases once (from a clean commit), vs re-recording 13 cases at every release. **Recommend
  normalising**; the gpl harness's change needs a replica-side agent or the orchestrator.
- **D4 records and the version:** `compare_records.py`'s `strip()` replaces `larmorx(-nc|-gpl)?
  <version>` by `<version>` (the orchestrator's script). **Recommend yes**, before the bump merges.
- **D5 container:** two images per release (Apache-only `:<v>`, everything `:<v>-all`), vs one.
  **Recommend two.** Base `python:3.12-slim-trixie` pinned by digest.
- **D6 publish order in one approval dialog:** three parallel publish jobs (one dialog);
  `publish-larmorx` waits for the companions to appear on PyPI before uploading (so `[all]` never
  points at missing versions). **Recommend so.**
- **D7 what happens after approval only:** the image tags and the GitHub Release come after the PyPI
  uploads; a rejected approval leaves nothing public. **Recommend so.**
- **D8 how the orchestrator starts a dry run:** add `push: branches: [release/dry-run]` (git push
  works; a dispatch needs a login or a token). **Recommend adding it** (or the user clicks "Run
  workflow" each time).
- **D9 licence expressions:** PEP 639's `License-Expression` should cover the code a distribution
  contains: the main wheel contains code under Apache-2.0, BSD-2-Clause, BSD-3-Clause, CC-BY-4.0,
  MIT, MPL-2.0 (Eigen's routines: file-level copyleft, their source stays available in the sdist and
  the repository), PSF-2.0, SunPro (fdlibm) and the HDF5 licence (no SPDX identifier:
  `LicenseRef-HDF5`) (NOTICE), plus permissive Rust crates (in the shipped SBOM). Keep `Apache-2.0`, or write the compound expression
  of NOTICE's licences (as numpy does for what it bundles), with a test keeping it equal to NOTICE.
  **Recommend the compound expression for the three packages** (e.g. larmorx-nc `LicenseRef-FSL AND
  Apache-2.0 AND BSD-3-Clause AND MIT ...`); the project's own licence stays Apache-2.0 (README
  says so). Possibly the user's call: a licence statement on PyPI.
- **D10 PyPI READMEs:** main README's links made absolute at build time (pinned to the tag), the
  companions get their own user-facing PyPI descriptions. **Recommend so.**
- **D11 standalone programs:** archives as above; Linux built for glibc 2.28 (no musl).
  **Recommend so.**
- **D12 platforms stated:** Python 3.12+ (CPython; free-threaded builds not supported by abi3
  wheels); Linux x86-64/aarch64 with glibc 2.28+; macOS 12 or newer, Intel and Apple silicon (the
  library alone would run on 11, but `[prep]`'s SciPy and scikit-learn need 12 on Apple silicon;
  one simple statement; wheels built with `MACOSX_DEPLOYMENT_TARGET=11.0`); Windows 10/11 x64 and
  arm64. No musllinux wheels in 0.1 (the container is the
  answer for odd Linux systems). **Recommend so.**
- **D13 metadata:** as in "Metadata"; the author's ORCID and an e-mail address (CITATION.cff) are
  the user's to give (none is invented). **Recommend asking the user in the final report.**
- **D14 larmorprepx:** keep the command, say "not ready" plainly (as above). **Recommend so.**
- **D15 Windows C runtime:** `.cargo/config.toml` at the root for every MSVC build (CI included),
  vs only in release.yml through `RUSTFLAGS` (CI unchanged, but release builds then differ from what
  CI tested). **Recommend the config file.**
- **D16 attestations:** PyPI's (default with trusted publishing) and GitHub's build provenance for
  the release files and the image. **Recommend yes.**
- **D17 GHCR visibility:** a new user-owned package may start private: the user makes
  `ghcr.io/karellopez/larmorx` public once, after the first push (browser). **Needs the user.**
- **D19 the companions' golden tests on the installed programs:** their harnesses
  (`crates-*/larmorx-*-cli/tests/golden.rs`) run cargo's own build of the program; to check the
  files users install, they need an option naming the program (e.g. `LARMORX_GOLDEN_PROGRAM`), in
  replica test code the release engineer may not open. **Recommend the orchestrator (or a
  replica-side agent) adds it** (with D3's gpl normalisation); until then the `build` job runs
  `cargo test --profile dist --locked -p larmorx-{nc,gpl}-cli --test golden` beside the wheel build
  on each platform (same profile and target), and the `test` job runs the installed programs through
  larmorx (`LARMORX_NC_BIN`, `tests/python/test_mri_replica.py::test_the_replica_itself`).
- **D18 dry-run-before-tag rule:** the orchestrator tags only a commit whose dry run passed and whose
  CI is green; the workflow's `check` job enforces CI-green and on-main. **Recommend so.**

## What the user does in the browser (exact values; for the final report)
1. PyPI -> Account settings -> Publishing -> "Add a new pending publisher" (GitHub), three times:
   project `larmorx`, owner `karellopez`, repository `larmorx`, workflow `release.yml`, environment
   `pypi`; project `larmorx-gpl`, environment `pypi-gpl`; project `larmorx-nc`, environment
   `pypi-nc` (the rest the same).
2. GitHub -> karellopez/larmorx -> Settings -> Environments -> New environment, three times (`pypi`,
   `pypi-gpl`, `pypi-nc`): "Required reviewers" = karellopez; "Prevent self-review" **off** (the
   user is the only reviewer and the tag is pushed from their account); "Deployment branches and
   tags" -> "Selected branches and tags" -> add a tag rule `v*`.
3. After the first image push: github.com/users/karellopez/packages/container/larmorx -> Package
   settings -> Change visibility -> Public (if it is not public already).
4. Approve the three deployments in the run's "Review deployments" dialog when the tag's run asks.
5. Optional, once: Windows Sandbox on win-laptop (above).

## Relay plan (one sub-task per relay; split marked a/b)
- **2a packaging, versions and metadata (DONE 2026-11-10, commits 61c7dec5, 42d88287, 13bbc7e7):** `scripts/release/set_version.py` (+ `--check`, a test),
  version 0.1.0rc1 in the three workspaces and locks, `[all]` pins, `larmorx_core::VERSION` PEP 440
  (+ larmorx-io's default), metadata (D9, D13), `crates-gpl/larmorx-gpl-cli` licence field, PyPI
  descriptions (D10: `scripts/release/pypi_readme.py`, `crates-*/PYPI.md`), larmorprepx's wording
  (D14), the golden harness's version normalisation and the 4 cases re-recorded (D3), the companion
  sdists' staging script (`scripts/release/companion_sdist.py`, from `git archive`, wheel data
  included) and the three sdists built and installed from source on Linux; commit the relay-1
  helpers under `validation/investigations/release-0.1/` (README). Gates.
- **2b packaging, profile and runtime (DONE 2026-11-10, commits a2704b60, f190b951, b36f7578):**
  `[profile.dist]` in the three workspaces, `.cargo/config.toml` (crt-static), the PE import check
  script; proof: the four golden suites and the parity smoke tier on dist builds (boxed), sizes
  recorded; the speed of dist against release; D20 (`third_party.py`, the three
  `third-party-rust.txt`). Gates.
- **2c replica-side (DONE 2026-11-10, commits b5bc14d6, 13013f21, 8ffd5448, d912a342):** D3's gpl
  normalisation and its 9 cases re-recorded, D19's `LARMORX_GOLDEN_PROGRAM` in both companion
  harnesses (proved on the dist wheels' installed programs), the companions' PEP 440 `--version`.
  Gates and the parity smoke tier (2,968 passed).
- **3a release workflow (DONE 2026-11-11, commits 46ed4455, 4b5c5d4d, d5421dac):** `release.yml` (all jobs but the container), `CHANGELOG` notes extraction,
  `SHA256SUMS`, the wait-for-PyPI step, standalone archives; actionlint and zizmor locally.
- **3b container (DONE 2026-11-11, commits feb3ad79, 14f7cc2b, 67d00142, 5d1c328b, 79935496 (dry
  run 1's fixes), 5ef26ea9):**
  `docker/Dockerfile` (two targets), `scripts/release/container.py`, the container jobs and the
  push after PyPI, the checks inside, the rehearsal here (rootless Docker, Apptainer, a local
  registry with skopeo).
- **4 clean machines (DONE 2026-11-11, commits c02045bd (dry run 2's fix), ebf9dde6, 0d858b2e,
  2e5c21f4, 93154c93):** as above (here; win-laptop prepared for rc1: `win_check.sh`).
- **5a documents in the repository (DONE 2026-11-11, commits ccd3325f, c36f7b0d, 96a050a1):**
  README's installation, CHANGELOG.md (0.1.0rc1: what is in it,
  each tool's status from `validation/tools.toml`), CITATION.cff, RELEASING.md (dry run, tag,
  approval, checks after, 0.1.0 after rc1), placeholders for the account name (current files only:
  `docs/helper-machines.md` lines 31-32, `docs/machine-move.md` 8, 71, 84-85, 150,
  `scripts/orchestration/move/{phase4-clean,pull,second-pass}.sh`,
  `validation/investigations/sdcflows-fieldmap-b/relay11/fetch_pybids020.sh`,
  `.../relay9/probe_obliquity.py`; keep the scripts working: `$HOME`-based paths, hostnames from a
  variable). For the orchestrator, not edited: CLAUDE.md line 228 (hostnames), records
  `docs/validation/ants-registration-panel.json` (11), `validation/investigations/ants-40-n4-
  nonpositive/{analysis,results}.json` (25 each), `itk-06-nonfinite/results.json` (6),
  `docs/parts/` (81 files), git history.
- **5b the site (DONE 2026-11-11, site commits 5650433, b8edd27, af701b6 on `release-pages`):** `content/installation.md` in a clone (`<scratch>/site`, branch `release-pages`,
  no-reply identity first); `content/index.md`'s stale note (it still names `larmorx[exact]` and "not
  on PyPI"; "no ... containers" in the Accessible card); nav and docs entries in `site.yml` through
  the handoff (shared); build 0 problems, inventory after >= before, screenshots 1440 and 390, light
  and dark.
- **6 handoff and final report (DONE 2026-11-11, commit 8f2d2bd7 and the scratch handoff):** HANDOFF.toml (`apply_handoff.py --check` on a trial merge),
  HANDOFF.md (next-steps' release items, `docs/licensing.md`'s "Nothing is on PyPI yet",
  `docs/overview.md` lines 19 and 60-84, the decision log, CLAUDE.md's lines, `docs/orchestration.md`'s
  release procedure), the dry-run plan, the user's browser steps.

## Helpers and seeds (relay 1, `<scratch>/inv/`; committed by relay 2a, local paths replaced, in
`validation/investigations/release-0.1/`; relay 2a's own runners in `<scratch>/r2a/`)
- `build.sh` (sizes and times), `sdist-build.sh`, `staging-sdist.py` (the companion sdists),
  `sdist_summary.py`, `golden-experiments.sh` (A: profile, B: version), `deps/resolve.sh` +
  `deps/summarize.py` (pip dry runs per platform), `action_pins.py`, `winps.sh` +
  `win-inventory.ps1` (win-laptop over SSH), `venv-tools/` (twine 7.0.0, readme_renderer, maturin
  1.16.0).
- Build seeds: `target-lto/` (fat LTO, main, 665 MB), `target-nc-lto/`, `target-gpl-lto/`,
  `target-plain/`, `target-nc-plain/`, `target-gpl-plain/`, `target-thin/`; wheels in `wheels/`
  (`plain`, `strip`, `thin`, `lto`, `bump`, `from-sdist`), sdists in `sdist/`, `sdist-staged/`.
- The harness refuses compound shell commands (pipes with `$(...)`, `source`, `cd` into another
  checkout before git): put multi-step work in a script file and run `bash <file>`; `source
  scripts/orchestration/env.sh` is refused, so set PATH, PYO3_PYTHON and OPENBLAS_CORETYPE in the
  script.
