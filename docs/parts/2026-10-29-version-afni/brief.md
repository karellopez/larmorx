You are bringing larmorx's AFNI tools from **AFNI 25.2.09 to AFNI 26.3.05**, the newest release (github.com/karellopez/larmorx; your worktree is a checkout of main). This is part 1 of the version update (the user's decision of 2026-10-27: larmorx follows the newest upstream releases, updated in planned steps; decision log "Newest upstream versions, updated in steps" and "How versions are handled from now on"). Aim for 6 to 8 hours.

Read first: the repo's `CLAUDE.md` (rules 1-10, "Agent working rules", "Working on this machine"); `docs/porting-guide.md` §1 (who may read what), its D13 section and its per-tool checklist; `scripts/orchestration/README.md`; `docs/licensing.md`; `docs/validation/oracle-cache.md`; `docs/validation/difftest.md` ("Running", "Permanent cases"); `docs/figures.md`; next-steps "The order from here" item 3; the decision-log entries named above; `crates-gpl/larmorx-gpl-afni/PROVENANCE.md` and `crates/larmorx-afni/PROVENANCE.md`; the AFNI findings `docs/findings/afni-tshift.md`, `afni-automask.md`, `afni-unifize.md`, `afni-licence-routines.md`, and the AFNI rows of `docs/findings/upstream-defects.md`.

**Testing depth (the user, 2026-10-28):** smart, not exhaustive. Every changed code path once, fMRIPrep's real calls on the oracle runs the standard tier already holds, the edge cases of any defect found, a short random-testing trial, the standard and smoke tiers. No data-panel re-runs beyond what the standard tier holds, no full tiers.

## Your role: replica author, validation, and spec writer for AFNI
You may read AFNI's source (`<workspace>/tags/AFNI-25.2.09`, `<workspace>/tags/AFNI-26.3.05`), the GPL replica (`crates-gpl/`) and the restricted AFNI findings. Because you read the source:
- you **never edit the clean-room crate** `crates/larmorx-afni` (nor `crates/larmorx-mri`);
- you edit `specs/3dTshift.md`, `specs/3dAutomask.md` and `specs/3dUnifize.md` **as the spec writer only** (porting guide §1): behaviour, maths, conventions, command line and validation plan, never code, routine names or line references; each new or changed line marked "(AFNI 26.3.05)".
- Where the clean-room originals must change to follow 26.3.05, describe the change by behaviour in the spec and list it in your report for a later clean-room agent.

## Tasks

### 1. The upstream watch (small; build it first and use it)
`scripts/orchestration/upstream_watch.py` (decision log 2026-10-27, "How versions are handled from now on"): for every upstream in `UPSTREAM.md` it finds the newest release (git tags of the reference clones and `git ls-remote`, PyPI's JSON API, AFNI's `AFNI.version`, FSL's public changelog) and, for each newer release, lists which of the files named in the crates' `PROVENANCE.md` changed (from the release tags in `<workspace>/tags/` when present; otherwise say what must be fetched). Output: a short Markdown report and a JSON file. Hook it weekly into `scripts/orchestration/difftest-nightly.sh` (no agent, no usage; once a week, by date). A unit test with a fake tags tree. Run it once and put its report in your handoff.

### 2. What changed between AFNI 25.2.09 and 26.3.05
- Every file named in `crates-gpl/larmorx-gpl-afni/PROVENANCE.md` (and in the main package's AFNI-derived parts, if any): diff the two tags. A first look found most diffs are licence headers only; `thd_automask.c` (about 200 changed lines) and `3dTshift.c` (9) change code; `3ddata.h`, `mrilib.h`, `thd_initdblk.c` and `vecmat.h` a few lines. Check each one, and every macro and routine the replicas call, also outside the listed files.
- Classify each change: licence text only; no effect on these programs; a change of results, messages, options, exit status or a defect (one larmorx registered as AFNI-NN that 26.3.05 fixes or changes, or a new one).

### 3. The replicas follow 26.3.05
- Port each change that matters to the GPL replicas (`crates-gpl/`), in full (every option, message, exit status and failure case of 26.3.05, as for 25.2.09). `PROVENANCE.md` names 26.3.05 and its commit.
- **D13 and the register:** a registered defect that 26.3.05 fixes: the default stays correct, `reproduce` now gives 26.3.05's behaviour (say so in the register text, through the handoff); a new defect: decided by rules A-D, with its switch. Defect IDs: only **AFNI-76 to AFNI-85**.
- **The harness picks the AFNI version:** the newest build (`oracles/afni-26.3.05`) by default for the replicas; the old build (`oracles/afni-25.2.09`) still selectable (one option and one environment variable, used by the parity suites, the evidence and random testing alike); every record states the version it ran. Never modify either oracle build.
- **The step from one version to the next:** in each replica record, the cases whose AFNI output changed between 25.2.09 and 26.3.05 (AFNI against AFNI, from the cache), with the reason.

### 4. The clean-room originals against 26.3.05 (run them; never edit their code)
- Run the clean-room originals' suites (`afni-tshift`, `afni-automask`, `afni-unifize`) against 26.3.05.
- Every change of behaviour they must follow: described in the spec by behaviour (above), and listed in your report for the clean-room agent.
- Where they already agree with 26.3.05, move their records to 26.3.05; where not, keep their records and their default oracle on 25.2.09 until the clean-room agent aligns them. Say which.
- `pytest tests/parity` must stay green.

### 5. AFNI's new licence: an inventory, no moves
AFNI 26.3.05's `LICENSE.txt` relicenses the Medical College of Wisconsin portions from GPL-2 to **CC BY 4.0**; files without a header are NIH public domain. This may let the AFNI replicas move into the main Apache package, but that is **the user's decision**. Move nothing. Write `docs/findings/afni-relicensing-26.md` (unrestricted: licences and file names only, no routine content):
- for every AFNI file the replicas derive from: its licence in 25.2.09 and in 26.3.05 (the header's exact words), and any that are still GPL;
- the other code in `crates-gpl/larmorx-gpl-afni`: the glibc 2.35 translations (`glibc_sincosf`, `glibc_floatmath`: `sinf`, `cosf`, `tanhf`, `expm1f`, `cbrtf`, ...), each with its licence and whether a permissive source gives the same bits (Arm's Optimized Routines, MIT, as the FSL replica uses for `pow` and `exp`; fdlibm's float functions under Sun's notice), checked against the existing all-2^32 verifiers where you can;
- what a move would need (notices, `NOTICE`, the CC BY 4.0 attribution and change statement, packaging, CI's `gpl` jobs) and what would stay in `larmorx-gpl`.

### 6. Golden, random testing, site
- Golden checksums: if the replicas' outputs change, re-record from a clean commit (`scripts/record_golden.py`).
- Random testing: the AFNI descriptors run against 26.3.05; a short trial (`difftest --tools afni-tshift,afni-automask,afni-unifize --budget 0.5 --out <workspace>/oracles/difftest/<date>-version-afni`, boxed); triage every difference by the rules.
- The site (a clone of `larmorx_documentation` in your scratch, on a branch; set its git identity to the no-reply address first): the three tools' evidence against 26.3.05 (`python -m larmorx_validation evidence <tool> --site <clone>`; cache entries for the new version are computed once), each page saying which AFNI version each panel ran and what changed from 25.2.09. **Nothing the site shows is removed or reduced**: a before/after inventory per page (every JSON path's count after >= before) and before/after screenshots (`tools/review/shoot.mjs`), and the number check (`LX_EVIDENCE.check()`) on each page.

## Documents
- **Your own files:** the replica code, both PROVENANCE files' AFNI lines (the clean-room crate's only if a version string must change, through the handoff otherwise), the three specs as spec writer, the AFNI findings files, the new relicensing finding, the records, the API pages' version lines, the permanent cases, `validation/investigations/version-afni/` (README and probes), the upstream-watch script and its test.
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:** the register rows and history, `docs/api/upstream-defects.md`, `docs/licensing.md`, `UPSTREAM.md`, `upstream.tsv`, rule 2 of CLAUDE.md (AFNI 26.3.05), next-steps (item 3's AFNI half done; the clean-room alignment's list), the decision log if anything needs a decision, `triage.toml` entries, the site's `data/status.yml` (generated: `validation/tools.toml`, then `python -m larmorx_validation tables`).
- Anything the rules cannot settle: report it with a recommendation, do not decide it.

## Machine (shared: read this)
- **Threads:** up to **24** (`cargo -j 24`, parity `--jobs` to match).
- **Memory:** your boxes together at most **36 GiB** (`scripts/orchestration/boxed.sh`); every parity tier, random-testing run, evidence export and measurement runs in a box.
- **Another job runs all along:** the heavy random-testing cases (tmux `heavy`, from the sibling checkout `../larmorx-heavy`, one case at a time in a 60 GiB box). Never touch that checkout, that tmux session, `oracles/difftest/2026-10-25/` or the main checkout (`<workspace>/larmorx`).
- **Never** run AFNI options that open a GUI or a browser (`-h_view`, `-h_web`, ...).

## Git and where to work
- **Your worktree branch.** Commit after every meaningful step. Never `Co-Authored-By` or any AI attribution: a commit-msg hook refuses it, and never use `--no-verify`. Do not push or merge.
- **Scratch:** `<scratch>/version-afni/`, the folder holding this brief.
  - Keep NOTES.md current; HANDOFF.md, builds, the site clone and outputs go there.
  - Never write under any `.claude/` folder.
  - Commit every helper behind a result under `validation/investigations/version-afni/`.
- **Environment:** `source scripts/orchestration/env.sh`.
  - Never `maturin develop`: build the wheel into `<scratch>/wheel` and copy the `.so` (`scripts/orchestration/lxv.sh`); `scripts/orchestration/build-gpl.sh` builds the replica program.
  - Build-cache seed: copy the main checkout's `target/`, `crates-gpl/target/` and `crates-nc/target/` into your worktree, reading only from there.
- **Gates:**
  - `scripts/orchestration/gates-rust.sh <worktree> all` and `gates-python.sh <worktree> --no-build`;
  - `pytest tests/parity`;
  - `python -m larmorx_validation tables --check`;
  - records and golden checksums from clean commits only.
- **Never:** delete work products; modify the oracle builds, the fMRIPrep runs, the cache's existing entries or any upstream source; ask for sudo; put FSL on PATH.
- **Public repository:** no network addresses, account names, hostnames or keys in anything you commit; write local paths as `<workspace>` and `<scratch>`.
- **Waiting:** wait for long runs with one background wait on the log's final line and end your turn until it notifies you; never stop silently.

## Stop after this part; report (concise)
- The upstream watch: what it found for every upstream.
- Each changed AFNI file: its class, and what the replicas do now.
- The records: per suite, cases, statuses against 26.3.05, compare_records against the 25.2.09 records, the version step (AFNI's own changes).
- The clean-room originals against 26.3.05: what agrees, what must change (the clean-room agent's list), where their records stand.
- The relicensing inventory in five lines, and what a move would need.
- Defects (IDs used), golden, the trial's numbers, the site's inventory and screenshots, gates, commits.
