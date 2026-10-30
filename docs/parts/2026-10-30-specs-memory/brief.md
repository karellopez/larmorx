You are doing a bounded part of larmorx's queue (github.com/karellopez/larmorx; your worktree is a checkout of main): **write three behavioural specs as the spec writer**, and **fix two memory overruns in the AFNI replicas' path**. Aim for 4 to 5 hours; stop at the end of the part.

Read first: the repo's `CLAUDE.md` (rules 1-10, "Agent working rules", "Working on this machine"); `docs/porting-guide.md` §1 (who may read what; what a spec may and may not contain: behaviour, maths, conventions, command line and validation plan, never code, routine names or line references); `scripts/orchestration/README.md`; the register's rows FSL-20, FSL-21, AFNI-75 and their sections (`docs/findings/upstream-defects.md`); `docs/findings/afni-relicensing-26.md` ("The C library's maths"); `docs/parts/2026-10-29-afni-move/report.md` (the memory cases and the `cbrtf` measurement); `tests/parity/test_memory_peak.py`; `specs/mcflirt.md`, `specs/3dUnifize.md`; next-steps' follow-ups "mcflirt's FSL-20 and FSL-21 in the clean-room original", "AFNI-75's guard ... the clean-room original needs it", "3dUnifize's replica memory", "The Python entry point's own memory".

**Testing depth (the user, 2026-10-28):** smart, not exhaustive.

## Your role: spec writer and replica author
You may read FSL's source (`~/fsl-6.0.7.23/src`, `crates-nc/fsl-source/`), AFNI's (`<workspace>/tags/AFNI-26.3.05`), glibc 2.35's, the replica crates (`crates-nc/`, `crates/larmorx-afni-replica`, `crates-gpl/tools/`), and the restricted findings. Because you read them, you **never edit a clean-room crate** (`crates/larmorx-afni`, `crates/larmorx-mri`), and specs contain behaviour only (porting guide §1), each new line marked "(spec writer, 2026-10-30)".

## Tasks
1. **`specs/mcflirt.md`: FSL-20 and FSL-21** for the clean-room head-motion correction: what mcflirt 6.0.7.23 does (messages, exit status, files written) and what larmorx's default and `reproduce` must do (the replica's decisions, rule B), observable by black-box runs; with a validation plan (the cases a clean-room agent adds).
2. **`specs/3dUnifize.md`: AFNI-75's guard** for the clean-room original: when `-EPI` (or the radius it sets) asks for an impossible ball, what the replica's default does (the memory prediction's rule, the message, the exit status) and what `reproduce` does, by behaviour; the validation plan.
3. **A spec of glibc 2.35's `cbrtf`, `specs/cbrtf.md`**, for a clean-room implementer who has never read glibc, to reproduce glibc's bits on every float32 input (the user has not decided yet; the recommendation is a clean-room `cbrtf` with glibc's bits; write the spec so that part can start at once if they agree): the method as mathematics (range reduction, the approximation, its coefficients as numbers, the refinement steps, the order and precision of each operation, the special values), never code; and the validation plan (the exhaustive check against glibc's results through `crates-gpl/tools/cbrtf-measure` or a black-box run of the system's `cbrtf`). Say in your report whether the spec is complete enough for bit-identity.
4. **The replica's 3dUnifize memory** (`crates/larmorx-afni-replica`): today 1.5 × AFNI 26.3.05's peak with `-T2` and 2.8 × with `-EPI` (`validation/investigations/afni-move/memory.py`); bring it within the rule (at most 10 % above AFNI's peak plus 8 MiB) with the results unchanged bit for bit (no full-size copies AFNI does not hold; the model is antsAI's memory part, `docs/parts/` of 2026-10-28 and `docs/findings/ants-ai.md`, "Memory and speed"). Remove the two strict expected failures.
5. **`python -m larmorx`'s own memory** on small images (about 13 MiB more than the binary: the interpreter and numpy): a lazy import on the command-line path so 3dAutomask on the memory test's small image is within the rule; remove that strict expected failure. Nothing else may change.

## Validation
- The three replica suites' standard records re-made from a clean commit only if outputs could change (they must not: compare_records, no status or detail change); golden checksums unchanged; `tests/parity/test_memory_peak.py` and `test_thread_pools.py` green with no expected failure left for these calls; the parity smoke tier green.
- No site work is needed unless a page states the replica's memory; if one does, say so in the handoff.

## Documents
- **Your own files:** the three specs (as spec writer), the replica crate and the Python entry point's code, `tests/parity/test_memory_peak.py`, the restricted findings, `validation/investigations/specs-memory/` (README and probes).
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:** next-steps (the follow-ups done; the clean-room part ready: FSL-20/21, AFNI-75, and `cbrtf` if the user agrees), the register if anything changes, the decision log if anything needs a decision.
- Defect IDs, only if a new defect is found: AFNI-76 to AFNI-80, FSL-65 to FSL-68.

## Machine (shared)
- Up to **24** threads; memory boxes together at most **36 GiB** (`scripts/orchestration/boxed.sh`).
- Never touch the main checkout (`<workspace>/larmorx`), `larmorx-heavy` or `oracles/difftest/2026-10-25/`.

## Git and where to work
- Your worktree branch; commit after every meaningful step; never AI attribution, never `--no-verify`; no push, no merge.
- Scratch: `<scratch>/specs-memory/` (this folder): NOTES.md, HANDOFF.md, builds, outputs; never write under a `.claude/` folder; commit every helper behind a result under `validation/investigations/specs-memory/`.
- `source scripts/orchestration/env.sh`; never `maturin develop` (build the wheel into `<scratch>/wheel`, copy the `.so`: `scripts/orchestration/lxv.sh`); build-cache seed: copy the main checkout's `target/`, `crates-gpl/target/`, `crates-nc/target/`, reading only from there.
- Gates: `scripts/orchestration/gates-rust.sh <worktree> all`, `gates-python.sh <worktree> --no-build`, `pytest tests/parity`, `python -m larmorx_validation tables --check`; records and golden checksums from clean commits only.
- Never: delete work products; modify the oracle builds or any upstream source; ask for sudo.
- Public repository: no network addresses, account names, hostnames or keys; local paths as `<workspace>` and `<scratch>`.
- Wait for long runs with one background wait on the log's final line; never stop silently.

## Stop after this part; report (concise)
- The three specs: what each now says, and whether `cbrtf`'s is complete enough for bit-identity.
- The memory fixes: the peaks before and after against AFNI's, the expected failures removed, compare_records, golden.
- Gates, commits, and anything for the user.
