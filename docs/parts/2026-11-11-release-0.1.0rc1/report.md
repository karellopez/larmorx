# The first release (0.1.0rc1): the final report

Relay 6's report (2026-11-11), verbatim but for local paths replaced by placeholders; the
orchestrator's merge notes follow it.

## Relay 6 (the last relay of the first release): report

Sub-task 6 is done. The branch and the site clone are clean, and both are ready for the merge. One
commit is new in this relay, `8f2d2bd7`; the handoff files are in the scratch folder.

### 1. What the part built

**larmorx**: branch `worktree-agent-aad7eab002a56be61`, head
`8f2d2bd708f3fc80695bf48a02d05557644fbaeb`. It has 28 commits over main `459a5a21`, which is also
origin's main.
- **2a** (`61c7dec5`, `42d88287`, `13bbc7e7`): the packaging.
  - Version 0.1.0rc1 everywhere, set by `set_version.py`; `--version` in PEP 440 form.
  - Metadata, the compound licence expressions and PyPI's descriptions.
  - The companions' sdists.
  - `larmorprepx` says "not ready yet".
  - The Python golden harness writes the version as `<version>`; its 4 cases were re-recorded.
- **2b** (`a2704b60`, `f190b951`, `b36f7578`):
  - The `dist` profile: the same bits, as fast or faster, smaller files.
  - `.cargo/config.toml` with `+crt-static` for Windows.
  - `pe_imports.py`, `third_party.py` and the three `LICENSES/third-party-rust.txt` files.
- **2c**, replica-side role (`b5bc14d6`, `13013f21`, `8ffd5448`, `d912a342`):
  - The companions print `0.1.0rc1`.
  - The gpl golden harness writes the version as `<version>`; its 9 cases were re-recorded.
  - `LARMORX_GOLDEN_PROGRAM`, so the companions' golden tests can run the installed programs.
- **3a** (`46ed4455`, `4b5c5d4d`, `d5421dac`): `release.yml` and its helpers in `scripts/release/`.
- **3b** (`feb3ad79`, `14f7cc2b`, `67d00142`, `5d1c328b`, `79935496`, `5ef26ea9`): the two images,
  `container.py`, and the fixes for dry run 1.
- **4** (`c02045bd`, `ebf9dde6`, `0d858b2e`, `2e5c21f4`, `93154c93`): the clean-machine checks,
  `check_install.py`, `check_install_windows.ps1`, and the fix for dry run 2.
- **5a** (`ccd3325f`, `c36f7b0d`, `96a050a1`): README's installation section, `CHANGELOG.md`,
  `CITATION.cff`, `RELEASING.md`, and placeholders for the account name.
- **6** (`8f2d2bd7`):
  - `skip-existing: true` in the three publish steps, with a test and a negative control.
  - RELEASING.md: a partial upload is finished by "Re-run failed jobs"; yanking is only for a
    broken upload. New step 6.5 (the site, after the upload) and the words a final release changes.
  - README links the site's installation page and "Why Rust and Python".
  - Relay 5b's helpers are now in `validation/investigations/release-0.1/site/`, with a README
    section. On the same inputs they write byte-identical files to the ones relay 5b ran.

**Site**: clone `<scratch>/site`, branch `release-pages` at `af701b6`, on top of the site's main
`d9f67d1` (also origin's). Commits: `5650433` (the installation page and the home page),
`b8edd27` ("Why Rust and Python"), `af701b6` (the layout).

**Checks at `8f2d2bd7`** (log `oracles/logs/release-6-gates.log`):
- Pass: actionlint, zizmor, `test_release.py` (63 passed), ruff, the SPDX check, `tables --check`,
  and every release script's `--check`.
- Python gates: 1,377 passed, 12 skipped, 7 failed. The 7 are the same `test_smoke.py` version
  tests every relay has seen: they fail only because the shared venv still says 0.0.1, and they
  pass after `maturin develop`.
- No Rust, golden checksum or site file changed in relay 6.

### 2. The handoff
All four files were in the scratch folder (filed here: `handoff.md`, `handoff.toml`,
`handoff-site.toml`, `handoff-claude-machines.py.txt`).
- **`HANDOFF.toml`**, 30 operations, applied at the merge: `docs/licensing.md`, `docs/overview.md`,
  `docs/validation/golden.md` (relay 2c's text), `docs/next-steps.md` (the release items marked
  done, the step to 0.1.0, the part's follow-ups, the items waiting for the user), `docs/status.md`,
  `docs/orchestration.md` (the release procedure), the decision-log entry (the user's decisions
  quoted, and the orchestrator's D1 to D20), two API pages that still said "not on PyPI yet", and 8
  operations on CLAUDE.md ("Distribution" and "Targets"; the release part in "Next"; the GitHub
  organisation question answered; rule 2's "FreeSurfer v8.2.0 (pending D5)" becomes "decided
  2026-10-26"; the gates: `third_party.py --write` after a `Cargo.lock` change; "Docker is not
  installed" corrected: docker-ce runs, but the account is not in the `docker` group; the repo
  map's release files).
- **`HANDOFF-claude-machines.py`**: removes the two host names from CLAUDE.md's "Machines" line.
- **`HANDOFF-site.toml`**, 7 operations, applied only after PyPI has the version: `site.yml`,
  `data/status.yml`, and the roadmap.
- **Verified:** `--check` passed in scratch trial merges: the branch into `459a5a21` (30 of 30) and
  `release-pages` into `d9f67d1` (7 of 7). Applied for real in those trial trees, the site gives
  the same files as relay 5b's scripts and builds with 0 problems; its build is byte-identical to
  the one relay 5b checked, so relay 5b's page inventory and screenshots still hold. Against main as
  it was, the site build shows the 2 expected problems (`CHANGELOG.md` and `CITATION.cff` exist
  only after the merge).

### 3. The part's agents (for the clean-room audit)

| Relay | Agent id |
|---|---|
| 1 | `a39d00af2f5ba5858` |
| 2a | `aad7eab002a56be61` |
| 2b | `a289f4bfef580629b` |
| 2c (replica-side role) | `a542f9fcb3b589521` |
| 3a | `a136e13aaa1567c52` |
| 3b | `adb5f6f3b49a3445d` |
| 4 | `a43da72427cc28e44` |
| 5a | `a674e7e1421ce3748` |
| 5b | `a0e2cf0f342d12de9` |
| 6 | `af288e71603a8c60e` |

For the audit: one `git grep` relay 6 ran also matched `docs/parts/` and printed some matching
lines there (install hints, a brief's oracle paths); after that it left `docs/parts/` out. No relay
edited clean-room code.

### 4. The plan from the merge to 0.1.0rc1 on PyPI
1. Merge `8f2d2bd7` with the full checklist (step 2's `maturin develop --release` makes the 7
   version tests pass; D4 strips the version from the record comparison; step 5a the clean-room
   audit; step 6 the handoff, then the machines snippet).
2. CI green on all 13 jobs (CI's first Windows run with `+crt-static`).
3. The final dry run on main's head, the exact commit to tag.
4. Re-check the three environments by the public API.
5. The date: CHANGELOG and CITATION.cff must name the tag's day.
6. The tag: `git tag -a v0.1.0rc1 -m "larmorx 0.1.0rc1" <sha>`, `git push origin v0.1.0rc1`,
   `watch_ci.py <sha> v0.1.0rc1`; when `assemble` passes, the user approves.
7. After the approval: the three uploads (larmorx's waits for its two companions),
   `container-push`, `github-release`, `verify-pypi` on six platforms; a failed job: "Re-run failed
   jobs" (RELEASING.md, step 5).
8. Checks after (D17): once the image package is public, `apptainer pull
   docker://ghcr.io/karellopez/larmorx:0.1.0rc1`; `check_install.py` from PyPI in a clean Linux
   image; `validation/investigations/release-0.1/win_check.sh 0.1.0rc1` on the Windows helper.
9. The site after PyPI: merge `release-pages`, apply `HANDOFF-site.toml`, `tools/build.py` 0
   problems, inventory at or above the one before, push, watch Pages.

### 5. What the user does in the browser
- Already done and checked: the three PyPI pending publishers and the three GitHub environments.
- At the tag's run, once `assemble` is green: "Review deployments", tick `pypi`, `pypi-gpl` and
  `pypi-nc`, then "Approve and deploy".
- If a job fails after that: "Re-run jobs", then "Re-run failed jobs". Never "Re-run all jobs".
- Once, after the images are pushed: github.com/users/karellopez/packages/container/larmorx,
  "Package settings", "Change visibility", "Public", and type `larmorx` to confirm.

### 6. Still open
- PyPI's author field: the user has not answered, so `authors` is unchanged; to settle before
  0.1.0 (a version's metadata cannot change after its upload).
- Windows Sandbox: later, as the user decided.
- Not tried: the site's advice on the download marks of macOS and of a browser on Windows.
- Two messages still in Cargo's version form (`0.1.0-rc.1`): the AFNI clean-room `implementation:`
  line (a clean-room agent's file) and tedana's `dataset_description.json` (a replica-side agent's
  file); identical from 0.1.0 on; in the next-steps follow-ups.
- Older problems found: nine links in `docs/status.md` written from the repository's root;
  shellcheck's one warning in relay 2a's `sdists-install.sh`; the home path in four record files
  and in `docs/parts/`.

## The orchestrator's merge (2026-11-11)

- The three dry runs on `release/dry-run`: run 42927413161 at `d5421dac` (two failures: the glibc
  check on a from-source build, rustup's clippy on linux-aarch64; fixed by relay 3b), run
  42934352316 at `5ef26ea9` (one: the macOS check counted the extension's own install name; fixed
  by relay 4), run 42942601741 at `93154c93`: every job green on the six platforms in 37 minutes,
  the release-only jobs skipped as designed.
- At the merge: `HANDOFF.toml` applied (30 of 30), the machines snippet run, the release date set
  to the tag's day (2026-11-11: `set_version.py 0.1.0rc1 --date 2026-11-11` and CHANGELOG's
  heading).
