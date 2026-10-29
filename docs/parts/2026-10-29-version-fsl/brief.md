You are bringing larmorx's FSL tools from **FSL 6.0.7.17 to FSL 6.0.7.23**, the newest release (github.com/karellopez/larmorx; your worktree is a checkout of main). This is part 2 of the version update (the user's decision of 2026-10-27: larmorx follows the newest upstream releases, updated in planned steps; decision log "Newest upstream versions, updated in steps", "How versions are handled from now on", and "2026-10-29: AFNI 26.3.05, the version update's first part", whose design you follow). Part 1 (AFNI) is merged: read `docs/parts/2026-10-29-version-afni/report.md` for the pattern (version option, AFNI's own step in each record, the site's version panels). SciPy, NumPy and OpenBLAS are **not** this part (a later part). Aim for 6 to 8 hours.

Read first: the repo's `CLAUDE.md` (rules 1-10, "Agent working rules", "Working on this machine"); `docs/porting-guide.md` §1 (who may read what), its D13 section and its per-tool checklist; `scripts/orchestration/README.md`; `docs/licensing.md` (the FSL Licence: "all original and amended source code is included"); `docs/validation/oracle-cache.md`; `docs/validation/difftest.md` ("Running", "Permanent cases"); `docs/figures.md`; next-steps "The order from here" item 3; `crates-nc/README.md`, `crates-nc/*/PROVENANCE.md`, `crates-nc/fsl-source/README.md` and `scripts/fsl_source.py`; the FSL findings `docs/findings/fsl-mcflirt.md`, `fsl-bet.md`, and the FSL rows of `docs/findings/upstream-defects.md`; the upstream watch's FSL section (`scripts/orchestration/upstream_watch.py`; its first report in `docs/parts/2026-10-29-version-afni/handoff.md` §11: 15 of the 57 replicated files changed and `fsl-znzlib/znzlib.c` removed; fsl-avwutils 2209.3 → 2209.6, fsl-bet2 2111.8 → 2111.9, fsl-miscmaths 2203.2 → 2412.6, fsl-newimage 2203.12 → 2601.0, fsl-newnifti 4.1.0 → 5.0.0, fsl-utils 2203.5 → 2412.6, fsl-znzlib 2111.0 → 2511.3).

**Testing depth (the user, 2026-10-28):** smart, not exhaustive. Every changed code path once, fMRIPrep's real calls on the oracle runs the standard tier already holds, the edge cases of any defect found, a short random-testing trial, the standard and smoke tiers. No data-panel re-runs beyond what the standard tier holds, no full tiers.

## Your role: replica author, validation, and spec writer for FSL
You may read FSL's source (6.0.7.17's in `crates-nc/fsl-source/` and `~/fsl/src`, 6.0.7.23's in `~/fsl-6.0.7.23/src`), the FSL replicas (`crates-nc/`) and the restricted FSL findings. Because you read the source:
- you **never edit a clean-room crate** (`crates/larmorx-mri`, `crates/larmorx-afni`, nor any future clean-room BET crate);
- you edit `specs/mcflirt.md` and `specs/bet.md` **as the spec writer only** (porting guide §1): behaviour, maths, conventions, command line and validation plan, never code, routine names or line references; each new or changed line marked "(FSL 6.0.7.23)";
- where the clean-room original (`lx.mri.hmc`) must change to follow 6.0.7.23, describe it by behaviour in the spec and list it in your report for a later clean-room agent.

## Tasks

### 1. What changed between FSL 6.0.7.17 and 6.0.7.23
- Every file named in the `crates-nc` crates' `PROVENANCE.md` and `fsl-source/MANIFEST.tsv`: diff the two versions (6.0.7.23's sources from its installation's `src/`; where a package's source is not installed, say how you got it). Check every macro, template and routine the replicas call, also outside the listed files (the libraries moved several versions: newimage 2203 → 2601, newnifti 4 → 5, miscmaths and utils 2203 → 2412, znzlib's `.c` gone).
- Compare the binaries too where it settles a question (`mcflirt`, `bet2`, `betsurf`, the `bet` script and what it calls; compiler and flags of both builds), as part 1 did for AFNI by machine code: a changed result needs a known reason.
- Classify each change: licence or comment text only; no effect on these programs; a change of results, messages, options, exit status or a defect (one registered as FSL-NN that 6.0.7.23 fixes or changes, or a new one).

### 2. The replicas follow 6.0.7.23
- Port each change that matters to the mcflirt and BET replicas and their shared core (`crates-nc/`), in full (every option, message, exit status and failure case of 6.0.7.23, as for 6.0.7.17). `PROVENANCE.md` files name 6.0.7.23 and its package versions.
- **The FSL Licence's source rule:** `crates-nc/fsl-source/` holds the original source of what the translation now comes from: update it with `scripts/fsl_source.py` from 6.0.7.23 (its `MANIFEST.tsv` with each package's new version and SHA-256), handle the removed `znzlib.c` correctly (whatever 6.0.7.23 uses instead), and keep what the licence requires for the files still translated from 6.0.7.17, if any. Say in your report how the folder satisfies the licence after the update.
- **D13 and the register:** a registered defect that 6.0.7.23 fixes: the default stays correct, `reproduce` now gives 6.0.7.23's behaviour (register text through the handoff); a new defect: decided by rules A-D, with its switch. Defect IDs: only **FSL-59 to FSL-68**.
- **The harness picks the FSL version,** as `--afni-version` does (read `validation/src/larmorx_validation/parity/afni_versions.py`): 6.0.7.23 (`~/fsl-6.0.7.23`) by default for the replicas; 6.0.7.17 (`~/fsl`) still selectable (one option and one environment variable, used by the parity suites, the evidence and random testing alike; reconcile with the existing `LARMORX_FSLDIR`); every record states the version it ran. Never modify either installation. fMRIPrep's own FSL (its image) stays what it is: the stage comparisons stay as they are.
- **The step from one version to the next:** in each replica record, the cases whose FSL output changed between 6.0.7.17 and 6.0.7.23 (FSL against FSL, from the cache), with the reason.

### 3. The clean-room original against 6.0.7.23 (run it; never edit its code)
- Run the head-motion correction's suite (`mri-hmc`) against mcflirt 6.0.7.23. Its record keeps its current form (an `experimental` clean-room original, compared within thresholds; the known `real/ds003345/default` failure and the `normmi` follow-up stay as they are).
- Every change of behaviour it must follow: described in `specs/mcflirt.md` by behaviour and listed in your report. Where it agrees with 6.0.7.23 as with 6.0.7.17, move its record to 6.0.7.23; where not, keep its record and default on 6.0.7.17 until a clean-room agent aligns it. Say which.
- BET has no clean-room original yet (it is the next part after this one): update `specs/bet.md` for 6.0.7.23 so its implementer starts from the newest behaviour.
- `pytest tests/parity` must stay green.

### 4. Golden, memory, random testing, site
- Golden checksums: if the replicas' outputs change, re-record from a clean commit (`scripts/record_golden.py`).
- Memory: the replicas' calls in `tests/parity/test_memory_peak.py` against 6.0.7.23 (the rule: at most 10 % above the original's peak plus 8 MiB).
- Random testing: the FSL descriptors run against 6.0.7.23; a short trial (`difftest --tools <the FSL descriptors> --budget 0.5 --out <workspace>/oracles/difftest/<date>-version-fsl`, boxed); triage every difference by the rules.
- The site (a clone of `larmorx_documentation` in your scratch, on a branch; set its git identity to the no-reply address first): the BET and head-motion pages' evidence against 6.0.7.23 (`python -m larmorx_validation evidence <tool> --site <clone>`; cache entries for the new version are computed once), each page saying which FSL version each panel ran and what changed from 6.0.7.17. **Nothing the site shows is removed or reduced**: a before/after inventory per page (every JSON path's count after >= before), before/after screenshots (`tools/review/shoot.mjs`; `validation/investigations/version-afni/site_shots.sh` is a model), and the number check (`LX_EVIDENCE.check()`, printed by `--eval` as a JSON string) on each page and the Validation page. Every hand-written number in the pages' prose that your records change: update it (the number check only covers bound numbers).

## Documents
- **Your own files:** the replica code, the `crates-nc` PROVENANCE files, NOTICE and `fsl-source/`, `specs/mcflirt.md` and `specs/bet.md` as spec writer, the FSL findings files, the records, the API pages' version lines, the permanent cases, `validation/investigations/version-fsl/` (README and probes).
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:** the register rows, sections and history, `docs/api/upstream-defects.md`, `docs/licensing.md` (if anything changes), `UPSTREAM.md`, `upstream.tsv` (if FSL is listed), rule 2 of CLAUDE.md and its FSL row, next-steps (item 3's FSL half; the clean-room alignment list), the decision log if anything needs a decision, `triage.toml` entries, golden.md, the site's `data/status.yml` (generated: `validation/tools.toml`, then `python -m larmorx_validation tables`).
- Anything the rules cannot settle: report it with a recommendation, do not decide it.

## Machine (shared: read this)
- **Threads:** up to **24** (`cargo -j 24`, parity `--jobs` to match).
- **Memory:** your boxes together at most **36 GiB** (`scripts/orchestration/boxed.sh`); every parity tier, random-testing run, evidence export and measurement runs in a box.
- **Another job runs all along:** the heavy random-testing cases (tmux `heavy`, from the sibling checkout `../larmorx-heavy`, one case at a time in a 60 GiB box). Never touch that checkout, that tmux session, `oracles/difftest/2026-10-25/` or the main checkout (`<workspace>/larmorx`).
- FSL stays off `PATH` (`env.sh`); call its programs by full path or through the harness. Never run FSL programs that open a GUI.

## Git and where to work
- **Your worktree branch.** Commit after every meaningful step. Never `Co-Authored-By` or any AI attribution: a commit-msg hook refuses it, and never use `--no-verify`. Do not push or merge.
- **Scratch:** `<scratch>/version-fsl/`, the folder holding this brief.
  - Keep NOTES.md current; HANDOFF.md, builds, the site clone and outputs go there.
  - Never write under any `.claude/` folder.
  - Commit every helper behind a result under `validation/investigations/version-fsl/`.
- **Environment:** `source scripts/orchestration/env.sh` (it now finds the workspace from a worktree by itself).
  - Never `maturin develop`: build the wheel into `<scratch>/wheel` and copy the `.so` (`scripts/orchestration/lxv.sh`); `scripts/orchestration/build-nc.sh` (or the README's equivalent) builds the replica program.
  - Build-cache seed: copy the main checkout's `target/`, `crates-gpl/target/` and `crates-nc/target/` into your worktree, reading only from there.
- **Gates:**
  - `scripts/orchestration/gates-rust.sh <worktree> all` and `gates-python.sh <worktree> --no-build`;
  - `pytest tests/parity`;
  - `python -m larmorx_validation tables --check`;
  - records and golden checksums from clean commits only.
- **Never:** delete work products; modify the FSL installations, the oracle builds, the fMRIPrep runs, the cache's existing entries or any upstream source; ask for sudo; download and compile code outside the documented FSL installations without saying so in your report.
- **Public repository:** no network addresses, account names, hostnames or keys in anything you commit; write local paths as `<workspace>`, `<scratch>` and `~/fsl-<version>`.
- **Waiting:** wait for long runs with one background wait on the log's final line and end your turn until it notifies you; never stop silently.

## Stop after this part; report (concise)
- Each changed FSL file and package: its class, and what the replicas do now; how `fsl-source/` satisfies the licence.
- The records: per suite, cases, statuses against 6.0.7.23, compare_records against the 6.0.7.17 records, the version step (FSL's own changes) with reasons.
- The clean-room head-motion correction against 6.0.7.23: what agrees, what must change (the clean-room agent's list), where its record stands; `specs/bet.md`'s changes.
- Defects (IDs used), golden, memory, the trial's numbers, the site's inventory, screenshots and number check, gates, commits.
