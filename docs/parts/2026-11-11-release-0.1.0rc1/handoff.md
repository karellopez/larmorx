# The first release (0.1.0rc1): the handoff (relay 6, 2026-11-11)

Three files beside this one, in the part's scratch folder:

| File | What | When |
|---|---|---|
| `HANDOFF.toml` | 30 operations on larmorx's shared documents (`docs/licensing.md`, `docs/overview.md`, `docs/validation/golden.md`, `docs/next-steps.md`, `docs/status.md`, `docs/orchestration.md`, `docs/decision-log.md`, two tools' API pages, and 8 on `CLAUDE.md`, the last block) | at the merge |
| `HANDOFF-claude-machines.py` | the two laptops' host names out of `CLAUDE.md`'s "Machines" line (kept out of the TOML, so that no file you may archive into `docs/parts/`, which is public, carries them) | at the merge, if you want it (your file) |
| `HANDOFF-site.toml` | 7 operations on the site's shared files (`site.yml`'s navigation and documentation list, `data/status.yml`'s planned item, four on `content/roadmap.md`, dated 2026-11-12) | at the site's publication, after PyPI has 0.1.0rc1 |

**Checked** (2026-11-11, relay 6): `apply_handoff.py --check` against a trial merge of the branch at
`8f2d2bd7` into main as it is (`459a5a21`; origin's main the same), in a scratch worktree: 30 of 30
apply; `HANDOFF-site.toml --site` against a trial merge of `release-pages` (`af701b6`) into the
site's main (`d9f67d1`, origin's the same): 7 of 7. Then applied for real in the two trial trees:
the site's three files are byte for byte what relay 5b's `handoff_site.py` and
`handoff_roadmap.py <site> 2026-11-12` write; the site builds with 0 problems (33 pages) against the
merged larmorx tree, and its `_site` is byte-identical to relay 5b's build of its handoff copy, so
relay 5b's inventory (no count down on any page) and screenshots hold for it exactly; against
main as it is the site has the 2 expected problems (`CHANGELOG.md`, `CITATION.cff`: only on main
after the merge). In the larmorx trial tree with the handoff: `tables --check` and `check_spdx.py`
pass; every relative link in the ten changed files resolves but nine old ones of `docs/status.md`
(below). `HANDOFF-claude-machines.py` was run on a copy of the trial's `CLAUDE.md` (no host name
left). The trial worktrees are removed.

## How to apply, in order

1. **At the merge** (docs/orchestration.md, step 6), in the merged checkout:
   `python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml` (all or nothing; the
   CLAUDE.md block is operations 22 to 29: drop them from a copy of the file if you prefer to edit
   CLAUDE.md by hand), then `python3 -I <scratch>/HANDOFF-claude-machines.py CLAUDE.md`. Nothing of
   the handoff touches `validation/tools.toml` or the defect register, so `tables` and
   `register_counts.py` give what they gave before (run them anyway, as step 6 says). If anything
   in the merge changes `validation/tools.toml` before the tag: `python -P
   scripts/release/changelog.py 0.1.0rc1 --write-statuses` (the tag's `check` job stops a release
   whose statuses table differs).
2. **After 0.1.0rc1 is on PyPI** (the upload, the images and the GitHub Release done): merge the
   site's `release-pages` (clone `<scratch>/site`, branch `release-pages`, `af701b6`; the no-reply
   identity is set in that clone), then from a larmorx checkout `python -P
   scripts/orchestration/apply_handoff.py <scratch>/HANDOFF-site.toml --site <the merged site>`; if
   the release is published on another day than 2026-11-12, use
   `python -P validation/investigations/release-0.1/site/handoff_roadmap.py <site> <day>` instead of
   the four roadmap operations (and the date of CHANGELOG and CITATION.cff moves too, below). Then
   `tools/build.py` (0 problems; in a fresh worktree link or fetch `static/evidence/blobs/`, which
   lives on the `evidence-blobs` branch: without it every evidence image is a "problem") and push.

## Why each change

- **`docs/licensing.md`** (relays 2a, 2b, 3b, 5a): its "Nothing is on PyPI yet" bullet becomes what
  a release publishes and under which terms: the three packages on PyPI from 0.1.0rc1 (the user's
  decisions), the compound licence expressions (D9) and that LARMORX's own licence stays
  Apache-2.0, `LICENSES/third-party-rust.txt` beside maturin's SBOM in every wheel and sdist (D20:
  the crates.io crates' licence texts that MIT's, Apache-2.0's and bzip2's binary notices require),
  FSL's source in larmorx-nc's files, larmorx-gpl's sdist as its Corresponding Source; the GitHub
  Release's standalone programs with their licences; **the two images** and why two (the plain one
  without the replica packages, commercial use allowed, in relay 5a's precise words; `-all` under
  the FSL Licence's non-commercial terms, the companions' sdists in `/opt/larmorx/source/`); the
  local build commands kept (larmorx-gpl's added), with a sentence on the `dist` profile. Its
  "Publishing" section's "Once wheels ship" made present.
- **`docs/overview.md`** (relay 5a's list): the section loses "(planned)", "Nothing is on PyPI yet"
  and "Until the packages are on PyPI"; the pip lines say what each installs (the doubled `pip
  install larmorx` line merged), surfaces and `larmorprepx` marked planned or not ready; the
  standalone programs exist; the published GPL source; the e-mail line made true again
  (CITATION.cff has the university address, which the user asked for); the release workflow in
  "Working on every platform". Wording "on PyPI from 0.1.0rc1" (as relay 2a's `crates-nc/README.md`
  says it), true before and after the upload, so the larmorx handoff need not wait for it.
- **`docs/validation/golden.md`** (relay 2c's text, with relay 2a's and 2b's facts): the larmorx-gpl
  row says the version is written `<version>`; the two re-recordings of 2026-11-10 (gpl from
  `13013f21`, 9 outputs; main from `42d88287`, the 4 HDF5 cases' 6 outputs; each "by that string
  only", proved by the two `*_version_only.py` helpers); a paragraph on the release's builds (which
  jobs run which suite on which files, `LARMORX_GOLDEN_PROGRAM`, the same checksums under `dist`);
  "Recording": a new version re-records nothing.
- **`docs/next-steps.md`**: item 10 of the order; the six release items marked done with their
  evidence (clean machines, the C runtime, size, dependencies, `[all]`, publishing) and a new item
  "0.1.0 after 0.1.0rc1" with what remains; the follow-up "Why Rust and Python" marked done (relay
  5b's note: CORE-MATH has 18 ports, `tan` included, where the list says 17); a follow-ups bullet of
  this part (the two Cargo-form messages, Windows source builds, the workflow's wall time, the
  contract audit's `GLUE`, the 3dvolreg suite's workspace fallback (the orchestrator, 2026-11-10:
  "a follow-up for the next-steps"), two possible maturin reports, the macOS and browser download
  advice not tried, the account name left in records and history); "Waiting for the user": PyPI's
  author field (still unanswered) and D17's one click.
- **`docs/status.md`**: an "In place" bullet for releases.
- **`docs/api/afni-tshift.md`, `docs/api/mri-hmc.md`**: the tools' own pages still said "not on PyPI
  yet: build it from ..." in their install lines; not shared documents, but not this role's files
  either, so through the handoff. (Their example outputs that print `0.0.1` are left: examples.)
- **`docs/orchestration.md`**: the release procedure as the orchestrator runs it (relay 3a's
  commands, relay 3b's image push and D17, relay 4's checks after the upload, relay 5b's
  publication order), pointing to `RELEASING.md` for the maintainer's full steps.
- **`docs/decision-log.md`**: one entry, "2026-11-10 and 11: The first release, 0.1.0rc1": the user's
  decisions quoted as BRIEF.md and NOTES.md give them, the orchestrator's D1 to D20 answers, what was
  decided during the part (pybids and `--only-binary`, the plain image's wording, `skip-existing`,
  the README's review), the three dry runs. ORCHESTRATOR-STATE.md's "Into the decision log and
  next-steps at the next merge" is done by it.
- **`CLAUDE.md`** (your file; exact text in operations 22 to 29): the release in "Distribution"; "No
  Docker needed (images offered)"; the part in "Next"; the GitHub organisation answered (the user,
  2026-11-10); rule 2's FreeSurfer "pending D5" (D5 was decided on 2026-10-26: the decision log's
  table says so); the gates: `third_party.py --write` after a `Cargo.lock` change and `licences.py
  --write` for a new licence (`validation/tests/test_release.py` fails otherwise, which every
  agent's gates now meet); "Docker is not installed" (docker-ce is active and enabled here, its
  socket `root:docker`, and the account is not in the `docker` group: checked 2026-11-11); the repo
  map's release files. And, by the snippet, the host names (relay 5a's list).
- **The site** (`HANDOFF-site.toml`, relay 5b's): "Install" in the header's navigation, the two new
  pages in the documentation list (sidebar and footer), `status.yml`'s planned "Releases on PyPI"
  becomes "Release 0.1.0" (remove it at 0.1.0), the roadmap's release section and completed row,
  "Last updated", its description.

## Not scriptable, or for your judgement

- **The order of publication:** larmorx's branch first (the site links its `CHANGELOG.md` and
  `CITATION.cff`); the site only after PyPI has the version. The README (already in the branch)
  links the site's two new pages, which exist only from the site's push: push the site soon after
  the upload.
- **The release date:** CHANGELOG's heading and CITATION.cff's `date-released` say 2026-11-12. On
  another day: `python -P scripts/release/set_version.py 0.1.0rc1 --date <day>` and the heading in
  one commit (CITATION.cff is not documentation-only for ci.yml, so it runs CI), then the dry run of
  that commit; and `handoff_roadmap.py` with that day for the site.
- **After the merge:** step 2's `maturin develop --release` brings the shared venv's metadata to
  0.1.0rc1 (`tests/python/test_smoke.py`'s 7 version tests fail in agents' worktrees until then, as
  every relay reported; they pass on the installed files). The first CI run of main after the merge
  is CI's first Windows run with `+crt-static` (the dry runs' Windows jobs passed on the `dist`
  builds). `compare_records.py` strips the versions (D4), so verify-merge's record comparison is not
  disturbed by 0.0.1 -> 0.1.0rc1. No golden checksum changed after relay 2c; main has not moved since
  the branch's base, so nothing to merge in `golden.json`.
- **The clean-room audit** (merge step 5a): the part's agents are in NOTES.md ("The part's agents").
  No relay edited clean-room code (the role's rules; relay 3a's `tests/python/test_mri_replica.py`
  fix is a test, not one of the audit's clean-room crates). Relay 6, for the record: one `git grep`
  whose pathspec also matched `docs/parts/` printed matching lines of some briefs and handoffs
  there (install hints: `larmorx[exact]`, `pip install larmorx-nc`, a brief's oracle paths); relay 6
  then excluded `docs/parts/` and edited no clean-room file.
- **Found, older than this part:** nine links in `docs/status.md` are written from the
  repository's root (`docs/status-log.md`, `docs/decision-log.md`, `docs/next-steps.md`,
  `docs/figures.md`, `docs/validation/oracle-cache.md`, `docs/validation/difftest.md`,
  `docs/findings/machine-move-proof.md`, `docs/machine-move.md`) and do not resolve from `docs/`
  (GitHub's page of the file); a `docs/` prefix too many. The handoff's new bullet links
  `../RELEASING.md` correctly. Also older: shellcheck's SC2034 (an unused variable) in relay 2a's
  `validation/investigations/release-0.1/sdists-install.sh`, which relay 5a's shellcheck list left
  out; harmless, left as it ran.
- **ORCHESTRATOR-STATE.md:** after the merge its release items move to the decision log and
  next-steps (this handoff); the stray `oracles` folder in
  `archive/2026-11-10-release-2a-stray-oracles/` is deletable when the user agrees (its cache
  entries record wrong runs).
