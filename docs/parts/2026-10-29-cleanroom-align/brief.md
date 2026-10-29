You are the **clean-room implementer** who brings larmorx's clean-room originals up to the newest releases of their originals: 3dTshift, 3dAutomask and 3dUnifize follow **AFNI 26.3.05**, and the head-motion correction (`lx.mri.hmc`, `larmorx mri hmc`) follows **mcflirt from FSL 6.0.7.23**, with its `-cost normmi` and `mutualinfo` costs made exact (github.com/karellopez/larmorx; your worktree is a checkout of main). This is next-steps item 3b (the user, 2026-10-29: "the clean rooms should be compliant with the new versions"). Aim for 6 to 8 hours.

## Your role: clean-room implementer (read this first)
You work **only from the specs, documentation, papers and black-box runs** of the original programs (CLAUDE.md rule 1; `docs/porting-guide.md` §1). You may read:
- CLAUDE.md, `docs/porting-guide.md` (§1 and "Clean-room originals: the same behaviour, built for speed"), `docs/architecture.md`, `scripts/orchestration/README.md`, `docs/validation/oracle-cache.md`, `docs/validation/difftest.md`, `docs/figures.md`;
- `specs/3dTshift.md`, `specs/3dAutomask.md`, `specs/3dUnifize.md` (the lines marked "(AFNI 26.3.05)") and `specs/mcflirt.md` (the lines marked "(FSL 6.0.7.23)" and **appendix B**, `mutualinfo` and `normmi` exactly);
- larmorx's own Apache code: the clean-room crates `crates/larmorx-afni` and `crates/larmorx-mri`, and the shared crates they use;
- the API pages `docs/api/afni-tshift.md`, `afni-automask.md`, `afni-unifize.md`, `mri-hmc.md`; `docs/api/upstream-defects.md`; the register's header and summary table only (`docs/findings/upstream-defects.md`, never the sections marked †);
- the clean-room findings `docs/findings/afni-masks-original.md` and `docs/findings/mri-hmc-original.md`; the records `docs/validation/afni-tshift.md`, `afni-automask.md`, `afni-unifize.md`, `mri-hmc.md`;
- in `docs/next-steps.md`: item 3b, and the follow-ups "The clean-room originals follow AFNI 26.3.05", "The clean-room head-motion correction follows FSL 6.0.7.23" and "The head-motion correction's `-cost normmi`" only;
- the harness mechanics you need (`validation/src/larmorx_validation/parity/afni_tshift.py`, `afni_automask.py`, `afni_unifize.py`, `afni_replica.py`, `mri_hmc.py`, `mri_hmc_count.py`; `difftest/` and its descriptors `afni_masks.py`, `afni_tshift.py`, `mri_hmc.py`; the evidence modules `evidence/afni_*.py` and `evidence/mri_hmc.py`); `validation/investigations/cleanroom-fixes/` (the earlier clean-room part's probes, `hmc_alignment.py`).

You must **never** open: AFNI's or FSL's source (`<workspace>/tags/AFNI-*`, `~/fsl*/src`, `crates-nc/fsl-source/`); the replica crates (`crates-gpl/`, `crates-nc/`); the restricted findings (`docs/findings/afni-tshift.md`, `afni-automask.md`, `afni-unifize.md`, `afni-licence-routines.md`, `fsl-mcflirt.md`, `fsl-bet.md`) or any register section marked †; `docs/parts/`; `validation/investigations/version-afni/` and `version-fsl/`; `validation/difftest/triage.toml`; the replica authors' permanent cases (`validation/difftest/cases/fsl-*`, and those named for the replicas); **`validation/src/larmorx_validation/parity/afni_versions.py` and `fsl_versions.py`** (written by replica authors: use their option and environment variable, below, and leave the files to the orchestrator). Running the original programs and the replica programs as black boxes is allowed (`larmorx-gpl`, `larmorx-nc` built, never opened). At the end, list in your report everything you opened and ran (a "clean-room statement"), as the earlier clean-room part did.

**Testing depth (the user, 2026-10-28):** smart, not exhaustive: every changed code path once, fMRIPrep's real calls already in the standard tiers, the edge cases, a short random-testing trial, the standard and smoke tiers.

## The versions
The harness runs the clean-room originals against AFNI 25.2.09 and FSL 6.0.7.17 by default until this part is merged. **Run every suite, the smoke tier, random testing and the evidence against the newest releases**: `--afni-version 26.3.05`, `--fsl-version 6.0.7.23`, or the environment variables `LARMORX_AFNI_VERSION=26.3.05` and `LARMORX_FSL_VERSION=6.0.7.23` (set both for `pytest tests/parity`). The orchestrator switches the defaults at the merge.

## Tasks
### 1. AFNI 26.3.05 (the follow-up "The clean-room originals follow AFNI 26.3.05", its five points)
- 3dTshift `-no_detrend`: `reproduce` de-means every voxel with its own mean, as `fix` does; AFNI-05 is never named (`specs/3dTshift.md` §4).
- Header slice code 2 in 3dTshift, 3dAutomask (`-apply_prefix`) and 3dUnifize (`-EPI`): `reproduce` reads the times `end, end−1, …, start` as `fix` does (a copy where they exceed the TR); AFNI-03 is never named (`specs/3dTshift.md` §3.2, §6.4, §6.5; `specs/3dAutomask.md` §9; `specs/3dUnifize.md`).
- The clean-room originals' permanent cases `afni-automask/slice-code-2-masked-notices`, `afni-unifize/epi-slice-code-2-fix-notice` and `epi-slice-code-2-reproduce-notices` re-described for 26.3.05.
- Their standard records re-made against 26.3.05 (from a clean commit); `PROVENANCE.md` version strings (black-box runs of `oracles/afni-26.3.05`).
- Today against 26.3.05: 3dTshift fails 6 of 315 (`options/*-no-detrend` ×4, `header-timing/code2-seq-dec`, `header-timing/code2-beyond-tr`), 3dAutomask 1 of 130, 3dUnifize 2 of 117; everything else agrees. After this part: none of these.

### 2. mcflirt 6.0.7.23 (the follow-up "The clean-room head-motion correction follows FSL 6.0.7.23")
- FSL-59 (`specs/mcflirt.md` §9.3): the default keeps the final interpolation asked for (`-sinc_final`, `-nn_final`); `reproduce` resamples trilinearly as mcflirt 6.0.7.23 does; both name FSL-59 where the two series differ.
- The zstd and bzip2 output types and their message (§10.1), reading by content (§4.1): pure-Rust dependencies only (rule 5; refresh every `Cargo.lock` that changes, and the GPL and NC workspaces' locks if they build the crate you change). Keep the change as small in scope as the spec allows (the head-motion correction's own I/O path, unless the spec says otherwise), with golden cases.
- FSL-60 (§4.1): the default opens the name given; `reproduce` renames as the spec describes; named.
- Its standard record re-made against 6.0.7.23. Today: 120 pass, 2 fail (`real/ds003345/default`, the known one, and `syn/motion/small/nn_final`, FSL-59).

### 3. `-cost normmi` and `mutualinfo` exactly (`specs/mcflirt.md` appendix B)
- Implement both costs as appendix B describes them, built for speed (porting guide), deterministic at any thread count.
- Measure with `validation/investigations/cleanroom-fixes/hmc_alignment.py` on the 91 `normmi` findings and the `mutualinfo` ones of the first long random-testing run (`<workspace>/oracles/difftest/2026-10-25/`; the earlier part's medians: `normmi` 1.033, `mutualinfo` 0.981, larmorx ÷ mcflirt temporal SD), and on the mri-hmc standard tier. Report the medians and the better/within 1 %/worse counts before and after.
- The permanent case `mri-hmc/normmi-aligns-worse`: its status and text from the new measurement (a pass where larmorx now agrees; otherwise its expected divergence explained). Its text says "the first long random-testing run (2026-10-25)", never "night".
- If appendix B leaves a question open, black-box runs settle it; say which in the spec's open questions through your handoff (you do not edit the specs).

### 4. Golden, memory, threads, random testing, speed
- Golden checksums: if outputs change, re-record from a clean commit (`scripts/record_golden.py`).
- The memory and thread checks (`tests/parity/test_memory_peak.py`, `test_thread_pools.py`) stay green; add the new costs' parallel paths to the thread check.
- Random testing: a 30-minute trial of `afni-tshift`, `afni-automask`, `afni-unifize`, `mri-hmc` against the newest releases (`difftest --tools ... --budget 0.5 --afni-version 26.3.05 --fsl-version 6.0.7.23 --out <workspace>/oracles/difftest/<date>-cleanroom-align`, boxed); every difference sorted by the rule (a larmorx mistake fixed with a permanent case; anything else reported with its reproducer).
- Speed: the changed paths no slower than before (a short before/after timing of the costs and of the AFNI options touched).

### 5. The site
Your clone of `larmorx_documentation` is ready: `<scratch>/site`, branch `cleanroom-align-newest`, the no-reply identity, the commit-msg guard and the evidence images in place. Commit there (if `git commit` is refused, leave the changes uncommitted and say so). The four pages (3dTshift, 3dAutomask, 3dUnifize, head-motion correction): the clean-room originals' records and panels against AFNI 26.3.05 and mcflirt 6.0.7.23, each panel naming the version it ran; the head-motion page's `normmi` statements from your measurement. **Nothing the site shows is removed or reduced**: a before/after inventory per page (`validation/investigations/panel-ants/site_inventory.py`), before/after screenshots (`tools/review/shoot.mjs`), `LX_EVIDENCE.check()` on each page and the Validation page, every hand-written number in the prose your records change updated, `tables --site`, the build with 0 problems. The figures cache keys entries by version where a version is part of the case (`evidence/afni_step.py` shows how): never overwrite an existing entry. Plain words in every text: say what a thing is (no "night").

## Documents
- **Your own files:** the clean-room crates, their `PROVENANCE.md`, the four API pages' version lines, `docs/findings/afni-masks-original.md` and `mri-hmc-original.md`, the records, the clean-room permanent cases, `validation/investigations/cleanroom-align/` (README and probes), the evidence modules for these four tools.
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:** the register's summary rows (behaviour only), `docs/api/upstream-defects.md`, next-steps (3b done; follow-ups), CLAUDE.md's AFNI and FSL rows, `UPSTREAM.md`, `triage.toml` entries **by tool, implementation and failed checks** (you cannot read it), golden.md, the spec questions for the spec writer, the site's Roadmap page row, `validation/tools.toml` if a hand-written line changes (then `python -m larmorx_validation tables`).
- Defect IDs, if a new defect of the originals is found by black-box runs: AFNI-76 to AFNI-80, FSL-64 to FSL-68.

## Machine (shared)
- Up to **24** threads (`cargo -j 24`, parity `--jobs` to match); memory boxes together at most **36 GiB** (`scripts/orchestration/boxed.sh`; every parity tier, random-testing run, evidence export and measurement in a box).
- The heavy random-testing job runs all along (tmux `heavy`, the sibling checkout `../larmorx-heavy`, `oracles/difftest/2026-10-25/` as its working folder): never touch it; reading the 2026-10-25 findings for your measurement is fine.
- Never run options of the originals that open a GUI or a browser.

## Git and where to work
- Your worktree branch; commit after every meaningful step; never AI attribution, never `--no-verify`; no push, no merge.
- Scratch: `<scratch>/cleanroom-align/` (this folder): NOTES.md, HANDOFF.md, builds, outputs; never write under a `.claude/` folder; commit every helper behind a result under `validation/investigations/cleanroom-align/`.
- `source scripts/orchestration/env.sh`; never `maturin develop` (build the wheel into `<scratch>/wheel`, copy the `.so`: `scripts/orchestration/lxv.sh`); build-cache seed: copy the main checkout's `target/`, `crates-gpl/target/`, `crates-nc/target/`, reading only from there.
- Gates: `scripts/orchestration/gates-rust.sh <worktree> all`, `gates-python.sh <worktree> --no-build`, `pytest tests/parity` (with both version variables set), `python -m larmorx_validation tables --check`; records and golden checksums from clean commits only.
- Never: delete work products; modify the oracle builds, the FSL installations, the fMRIPrep runs, the cache's existing entries or any upstream source; ask for sudo; put FSL on PATH.
- Public repository: no network addresses, account names, hostnames or keys; local paths as `<workspace>`, `<scratch>`, `~/fsl-<version>`.
- Wait for long runs with one background wait on the log's final line; never stop silently.

## Stop after this part; report (concise)
- What changed in each clean-room original, and its record against the newest release (cases, statuses, compare_records against its previous record).
- `normmi` and `mutualinfo`: the measurements before and after.
- Golden, memory, threads, speed, the trial's numbers and triage, the site's inventory, screenshots and number check, gates, commits.
- The clean-room statement: everything opened and run.
