You are moving **larmorx's AFNI replicas (3dTshift, 3dAutomask, 3dUnifize) from the GPL package into the main Apache-2.0 package** (github.com/karellopez/larmorx; your worktree is a checkout of main). AFNI 26.3.05 releases the Medical College of Wisconsin's code under CC BY 4.0, and none of the 55 AFNI files the replicas derive from is GPL any more; the user decided the move on 2026-10-29 (decision log, "The AFNI replicas move into the main package"). This is next-steps item 3c. Aim for 6 to 8 hours.

Read first: the repo's `CLAUDE.md` (rules 1-10, "Agent working rules", "Working on this machine"); **`docs/findings/afni-relicensing-26.md`** (the inventory: every file's licence, the C library pieces, what a move needs); `docs/licensing.md`; `docs/porting-guide.md` §1 and its per-tool checklist; `scripts/orchestration/README.md`; `docs/validation/oracle-cache.md`; `docs/validation/golden.md`; `docs/figures.md`; next-steps item 3c; `crates-gpl/README.md`, `crates-gpl/NOTICE`, `crates-gpl/larmorx-gpl-afni/PROVENANCE.md`; `docs/architecture.md`; how the replica is reached today (`python/larmorx/_replica.py`, `python/larmorx/afni/`, the `larmorx` CLI's AFNI commands, `LARMORX_GPL_BIN`, `scripts/orchestration/build-gpl.sh`, the parity suites `afni_replica.py`, `afni_tshift.py`, `afni_automask.py`, `afni_unifize.py`, CI's `gpl` jobs in `.github/workflows/ci.yml`).

**Testing depth (the user, 2026-10-28):** smart, not exhaustive: every changed path once, the standard and smoke tiers, a short random-testing trial.

## Your role: replica author
You may read AFNI's source (`<workspace>/tags/AFNI-26.3.05`, `AFNI-25.2.09`), glibc's and Arm's Optimized Routines' sources, and the replica crates. You **never edit a clean-room crate** (`crates/larmorx-afni`, `crates/larmorx-mri`), and you never write AFNI routine names into unrestricted documents (the register's summary rows, `docs/api/`, next-steps, the decision log, the site): source detail goes into the restricted findings.

## Tasks
### 1. The C library's maths, made permissive (before moving anything)
- **`cbrtf`, measured first (rule D):** 3dUnifize `-EPI` computes its radius from glibc 2.35's `cbrtf` (LGPL-2.1-or-later, no permissive equal-bit source known). larmorx's own `larmorx_core::math::cbrtf` (CORE-MATH, MIT) differs from glibc's on 453,492,162 of the 2^32 inputs. Measure whether that last bit can ever change 3dUnifize's results or messages: the ball of neighbours (which voxel offsets it holds), the printed radius, anything else the radius feeds, over every input the radius can take (the product of the three voxel sizes as float32: exhaustively where cheap, otherwise the full set of inputs where the two `cbrtf` differ, with the ball's decision boundaries derived exactly). If it never changes anything: use `larmorx_core::math::cbrtf`. If it can: report the cases, and either find a permissive routine with glibc's bits on every input (proved exhaustively) or recommend how to keep the exact behaviour without LGPL code in the main package; do not decide that yourself.
- **`sinf` and `cosf`:** translate them afresh from Arm's Optimized Routines v19.11 (MIT), the code glibc 2.35 ships, both of glibc's x86-64 builds (FMA and SSE2). Do not copy `crates-nc/larmorx-nc-core/src/glibc_float.rs` (it is in the FSL-licensed package). Prove them equal to the current replica's `glibc_sincosf` on all 2^32 inputs of each, by a Rust verifier (no C compilation is needed: the current translation is already proven against glibc). Keep the verifier as a tool under `crates-gpl/tools/` or `tools/` (where its licences allow).
- **`tanhf`, `expm1f`:** fdlibm under Sun's notice, already permissive; carry the notice.

### 2. The move
- The replica crate moves from `crates-gpl/larmorx-gpl-afni` into the main workspace (for example `crates/larmorx-afni-replica`; choose a clear name), with its SPDX headers: `Apache-2.0 AND CC-BY-4.0` for the files derived from MCW's CC BY 4.0 code, `Apache-2.0` for NIH-derived and larmorx's own files, `Apache-2.0 AND MIT` for Arm's, the Sun notice for fdlibm's. **CC BY 4.0's attribution (§3(a))** in each MCW-derived file's header, in `PROVENANCE.md` and in the main package's `NOTICE`: the creator ("Robert W. Cox et al., Medical College of Wisconsin"), the copyright notice, the licence and its URI, the source (AFNI's repository and tag) and that the material was modified (translated to Rust, with the changes PROVENANCE lists); CC BY 4.0's legal code in `LICENSES/`. The NIH files keep their public-domain line and AFNI's request to cite.
- larmorx's own code written for `larmorx-gpl` (`x87`, the CLI's output path, the program's dispatch) is relicensed to Apache-2.0 by its author (the user, who decided the move): say so in PROVENANCE.
- **In-process:** the replica runs inside the main package (the Python extension and the `larmorx` CLI), no separate program; `implementation=` (`auto`, `replica`, `original`) keeps choosing between the replica and the clean-room original. `auto` keeps its documented rule (the replica when available); it is now always available, so measure and report the speed of both implementations on the standard tier's real cases (the user may want to revisit the default; do not change the rule).
- **`larmorx-gpl` keeps what is still GPL:** nothing today (Workbench later). Leave `crates-gpl/` as a documented skeleton (README, LICENSE, NOTICE stating it is reserved for Connectome Workbench's replicas), without code; remove CI's six `gpl` jobs only if nothing remains to test there, and say how many CI minutes that saves (estimate from recent runs). Nothing is published on PyPI yet, so no compatibility alias is needed for `larmorx-gpl afni ...`.
- `cargo deny` (or the licence check the repository uses) allows the new licences for the main crates; `scripts/check_spdx.py` knows the new SPDX expressions.

### 3. Validation
- The three replica suites run the in-process replica; their standard records re-made from a clean commit (`afni-tshift-replica`, `afni-automask-replica`, `afni-unifize-replica`), compare_records against main's: **no status or detail change** expected (the same code, a different path), except what task 1 measures (report any).
- Golden: the GPL golden cases (`crates-gpl/larmorx-gpl-cli/tests/golden.rs`) move into the main package's golden suite, with the same checksums (they must not change), within CI's budget (rule 10).
- Memory (`tests/parity/test_memory_peak.py`) and threads (`test_thread_pools.py`): the replica's calls in-process meet the rules.
- Random testing: a 30-minute trial of the three AFNI descriptors with the in-process replica (`--out <workspace>/oracles/difftest/<date>-afni-move`, boxed); every difference triaged.
- Clean-room rule: the moved crate is replica code; add it to the lists of what clean-room implementers must never open (CLAUDE.md rule 1, porting guide §1: through the handoff).

### 4. Site and documents
- The site (your clone of `larmorx_documentation` at `<scratch>/site`, branch `afni-move`, the no-reply identity, the commit-msg guard and the evidence images in place; commit there; if `git commit` is refused, leave the changes uncommitted and say so): the three AFNI pages and the Validation page say the replica is in the main package (`pip install larmorx`), under which licences; the records re-exported; nothing removed or reduced (before/after inventory per page, screenshots, `LX_EVIDENCE.check()`, the build with 0 problems, `tables --site`). Plain words (no "night").
- **Your own files:** the moved crate and its PROVENANCE, NOTICE and LICENSES, the API pages' licence lines, the restricted AFNI findings, `docs/findings/afni-relicensing-26.md` (what was done), the records, `validation/investigations/afni-move/` (README and probes), the harness and evidence code the move needs, the golden registry, `ci.yml` (only as task 2 says), `crates-gpl/README.md`.
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:** CLAUDE.md (rule 1's licence tracks, rule 2, the AFNI row, "Where things are"), `docs/licensing.md` (rewritten for AFNI), `docs/overview.md` and `README.md` where they describe packages, `UPSTREAM.md`, the register and `docs/api/upstream-defects.md` if anything changes, next-steps (3c done; follow-ups), the decision log (what the rules settled, and anything for the user), `triage.toml` entries, golden.md, the site's Roadmap row, `validation/tools.toml` (then `python -m larmorx_validation tables`).
- Defect IDs, only: AFNI-76 to AFNI-80.
- Anything the rules cannot settle (for example `cbrtf` if its last bit matters): report it with a recommendation.

## Machine (shared)
- Up to **24** threads (`cargo -j 24`, parity `--jobs` to match); memory boxes together at most **36 GiB** (`scripts/orchestration/boxed.sh`; every parity tier, random-testing run, evidence export and long measurement, the exhaustive sweeps included).
- Never touch the main checkout (`<workspace>/larmorx`), `larmorx-heavy` or `oracles/difftest/2026-10-25/`.

## Git and where to work
- Your worktree branch; commit after every meaningful step; never AI attribution, never `--no-verify`; no push, no merge.
- Scratch: `<scratch>/afni-move/` (this folder): NOTES.md, HANDOFF.md, builds, outputs; never write under a `.claude/` folder; commit every helper behind a result under `validation/investigations/afni-move/`.
- `source scripts/orchestration/env.sh`; never `maturin develop` (build the wheel into `<scratch>/wheel`, copy the `.so`: `scripts/orchestration/lxv.sh`); build-cache seed: copy the main checkout's `target/`, `crates-gpl/target/`, `crates-nc/target/`, reading only from there.
- Gates: `scripts/orchestration/gates-rust.sh <worktree> all`, `gates-python.sh <worktree> --no-build`, `pytest tests/parity`, `python -m larmorx_validation tables --check`; records and golden checksums from clean commits only.
- Never: delete work products (moving code with `git mv` is fine; anything removed from the build is archived or stays in git history, say which); modify the oracle builds or any upstream source; ask for sudo.
- Public repository: no network addresses, account names, hostnames or keys; local paths as `<workspace>` and `<scratch>`.
- Wait for long runs with one background wait on the log's final line; never stop silently.

## Stop after this part; report (concise)
- `cbrtf`: the measurement and its outcome; `sinf`/`cosf`: the proof's numbers.
- The move: the new crate, the licences and notices, the in-process path, what stays in `larmorx-gpl`, CI's change and minutes.
- The records (compare_records), golden (unchanged), memory, threads, the trial, the speed of both implementations.
- The site's inventory, screenshots and number check, gates, commits, and anything for the user.
