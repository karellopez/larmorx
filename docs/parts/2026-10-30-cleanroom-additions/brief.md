You are the **clean-room implementer** who adds four specified corrections to the clean-room head-motion correction (`lx.mri.hmc`, `larmorx mri hmc`) and AFNI-75's memory guard to the clean-room original of 3dUnifize (github.com/karellopez/larmorx; your worktree is a checkout of main). The specs were written on 2026-10-30 by a spec writer. Aim for 3 to 4 hours; stop at the end of the part.

## Your role: clean-room implementer (read this first)
You work **only from the specs, documentation, papers and black-box runs** of the original programs (CLAUDE.md rule 1; `docs/porting-guide.md` §1). You may read:
- CLAUDE.md, `docs/porting-guide.md` (§1 and "Clean-room originals: the same behaviour, built for speed"), `docs/architecture.md`, `scripts/orchestration/README.md`, `docs/validation/oracle-cache.md`, `docs/validation/difftest.md`;
- `specs/mcflirt.md` (the lines marked "(spec writer, 2026-10-30)": §10.6, §12.1, §14.1, and what they refer to) and `specs/3dUnifize.md` (§7, §9, §9.1, §10, the same marks);
- larmorx's own Apache code: the clean-room crates `crates/larmorx-mri` and `crates/larmorx-afni`, and the shared crates they use (`larmorx-core`, `larmorx-io`, `larmorx-cli`'s non-replica code, `larmorx-py`'s `mri.rs` and AFNI wrappers);
- the API pages `docs/api/mri-hmc.md`, `docs/api/afni-unifize.md`, `docs/api/upstream-defects.md`; the register's header and summary table only (`docs/findings/upstream-defects.md`, never a section marked †);
- the clean-room findings `docs/findings/mri-hmc-original.md`, `docs/findings/afni-masks-original.md`; the records `docs/validation/mri-hmc.md`, `docs/validation/afni-unifize.md`;
- in `docs/next-steps.md`: the follow-ups "mcflirt's FSL-20, FSL-21, FSL-65 and FSL-66 in the clean-room original" and the AFNI-75 line under "Random testing's open items" only;
- the harness mechanics you need (`validation/src/larmorx_validation/parity/mri_hmc.py`, `afni_unifize.py`; `difftest/` and its descriptors `mri_hmc.py`, `afni_masks.py`; the clean-room permanent cases); `validation/investigations/cleanroom-fixes/` and `cleanroom-align/` (earlier clean-room parts' probes).

You must **never** open: AFNI's, FSL's or glibc's source; the replica crates (`crates/larmorx-afni-replica`, `crates-gpl/`, `crates-nc/`); the restricted findings (`docs/findings/afni-*.md` other than `afni-masks-original.md` and `afni-relicensing-26.md`'s licence tables, `fsl-mcflirt.md`, `fsl-bet.md`) or any register section marked †; `docs/parts/`; `validation/investigations/specs-memory/` (the spec writer's probes read the source's behaviour, but ask you to stay with the spec), `version-*/` and `afni-move/`; `validation/difftest/triage.toml`; `parity/afni_versions.py`, `fsl_versions.py`, `python_versions.py`. Running the original programs and the replica programs as black boxes is allowed. At the end, list everything you opened and ran (a "clean-room statement").

**Testing depth (the user, 2026-10-28):** smart, not exhaustive: every new path once, the spec's cases, the standard and smoke tiers, a short random-testing trial.

## Tasks
1. **FSL-20, FSL-21, FSL-65, FSL-66** in the clean-room head-motion correction, exactly as `specs/mcflirt.md` §10.6 and §12.1 say (the default and `reproduce`; messages, exit statuses, files written), with the cases of §14.1 added to the mri-hmc suite (permanent or parity cases).
2. **AFNI-75's guard** in the clean-room original of 3dUnifize, exactly as `specs/3dUnifize.md` §9.1 says (the same memory prediction as the replica, so both refuse the same calls; the limit's order; the messages; `reproduce` without the guard), with the §10 cases added to the afni-unifize suite.
3. Records: the mri-hmc and afni-unifize standard records re-made from a clean commit (compare_records against main's: only the new cases added; no status change elsewhere); golden cases for each new command-line path (rule 10), re-recorded from a clean commit if checksums change; the memory and thread checks green.
4. Random testing: a 30-minute trial of `mri-hmc` and `afni-unifize` (`--out <workspace>/oracles/difftest/<date>-cleanroom-additions`, boxed); differences reported by tool, implementation and failed checks.
5. No site work (the orchestrator re-exports the pages at the merge).

## Documents
- **Your own files:** the clean-room crates and their PROVENANCE, the two API pages, `docs/findings/mri-hmc-original.md`, `afni-masks-original.md`, the records, the clean-room permanent cases, `validation/investigations/cleanroom-additions/` (README and probes).
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:** the register's summary rows (behaviour only), `docs/api/upstream-defects.md`, next-steps (the follow-ups done), triage entries by tool, implementation and failed checks, golden.md, the site's Roadmap row ("Two specified clean-room additions": done).
- No new defect IDs are expected; if black-box runs show one, report it with its evidence (no ID).

## Machine (shared)
- Up to **24** threads; memory boxes together at most **36 GiB** (`scripts/orchestration/boxed.sh`).
- Never touch the main checkout (`<workspace>/larmorx`), `larmorx-heavy` or `oracles/difftest/2026-10-25/`.

## Git and where to work
- Your worktree branch; commit after every meaningful step; never AI attribution, never `--no-verify`; no push, no merge.
- Scratch: `<scratch>/cleanroom-additions/` (this folder): NOTES.md, HANDOFF.md, builds, outputs; never write under a `.claude/` folder.
- `source scripts/orchestration/env.sh`; never `maturin develop` (build the wheel into `<scratch>/wheel`, copy the `.so`: `scripts/orchestration/lxv.sh`); build-cache seed: copy the main checkout's `target/`, reading only from there.
- Gates: `scripts/orchestration/gates-rust.sh <worktree> all`, `gates-python.sh <worktree> --no-build`, `pytest tests/parity`, `python -m larmorx_validation tables --check`; records and golden checksums from clean commits only.
- Never: delete work products; modify the oracle builds or any upstream source; ask for sudo.
- Public repository: no network addresses, account names, hostnames or keys; local paths as `<workspace>` and `<scratch>`.
- Wait for long runs with one background wait on the log's final line; never stop silently.

## Stop after this part; report (concise)
- What each correction does now (default and `reproduce`), the cases added and their results.
- The records (compare_records), golden, memory, threads, the trial.
- Gates, commits, and the clean-room statement.
