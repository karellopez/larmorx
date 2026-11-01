You are continuing FSL's **FLIRT** replica: **part 2** (github.com/karellopez/larmorx; your worktree is a checkout of main at 301fab7, where part 1 is merged: `crates-nc/larmorx-nc-flirt`, `flirt` and `convert_xfm`, record `fsl-flirt-replica` 107 of 107 bit-identical). Aim for 6 to 8 hours; build in the order below and stop at a clean boundary with a NOTES.md that lets the next part continue.

Read first: the repo's `CLAUDE.md` (rules 1-10, "Agent working rules", "Working on this machine"); `docs/porting-guide.md` §1, D13 with rules A-D, the per-tool checklist §11; **part 1's folder** `docs/parts/2026-11-01-fsl-flirt-part1/` (brief, report, notes, handoff: your starting point); `specs/flirt.md`; `docs/findings/fsl-flirt.md`; `docs/api/fsl-flirt.md`; `crates-nc/larmorx-nc-flirt` and `larmorx-nc-core`; `validation/investigations/fsl-flirt/` (the reproducers); `validation/difftest/triage.toml`'s FLIRT entries; `docs/figures.md` and an FSL evidence module with its site page as the model (`fsl_bet` / `content/tools/fsl-bet.md`, `mri_hmc`); `scripts/orchestration/README.md`; `docs/validation/oracle-cache.md`; `docs/validation/difftest.md`.

## Your roles: replica author and spec writer
As in part 1: you may read FSL 6.0.7.23's source (`~/fsl-6.0.7.23/src/`: fsl-flirt, fsl-newimage, fsl-miscmaths, fsl-utils, fsl-znzlib, fsl-armawrap; Armadillo's and LAPACK's sources where the core translates them). You keep `specs/flirt.md` complete for every behaviour you add (behaviour, maths, conventions, messages, failure cases, the validation plan; never code, routine names or source structure). You **never edit a clean-room crate** (`crates/larmorx-fsl`, `crates/larmorx-mri`, `crates/larmorx-afni`).

## Task, in this order
1. **The site's FLIRT page first** (part 1 left it out; the per-part rule: each part publishes its page from its own tests): an evidence module `fsl_flirt` and `content/tools/fsl-flirt.md` in the approved form: raw inputs (the moving and reference images, BBR's white-matter boundary), FSL's result and the replica's (registered images, matrices as text), the differences (overlays, displacement over the brain, image differences), the metrics table with a verdict and its reason per comparison, the record's cases in a case explorer. **From the standard tier's existing results and cached oracle runs only** (fMRIPrep's three BBR calls, a 12-dof call, `-applyxfm`): no new data, no new oracle runs beyond what the page needs. Faces: never a recognisable face (brain-masked, or a head whose defacing is checked). Then `validation/tools.toml` gains the tool's `page` (through the handoff). Number check, re-export check, inventory, screenshots (desktop 1440, phone 390, light and dark) in `<scratch>/shots/`. Site clone: `git clone <workspace>/larmorx_documentation <scratch>/site`, set `user.name karellopez` and `user.email 49761113+karellopez@users.noreply.github.com` before the first commit, branch `fsl-flirt-page`.
2. **The trial's open findings** (`validation/difftest/triage.toml`, the three `unclear` groups; trial in `<workspace>/oracles/difftest/2026-11-01-fsl-flirt`): group the 359 by cause, one representative each, then settle each group:
   - FSL's heap corruption on small or constant images (281, and the `-bins 16` case): trace it the cheapest way (the source along the failing path, `gdb` on FSL's binary for the abort's backtrace); an instrumented build only if those fail, and then only of the files involved (GCC 14.3 in `oracles/toolchains/gcc-14.3`), kept as a probe. Once the trigger is known: undefined behaviour is refused in both modes before any work (rule B), with a new ID, its exact trigger in the spec.
   - The 66 differing matrices (64 with voxels below 0.5 mm or above 5 mm, 2 plain): find the cause; fix the replica (a permanent case), or register FSL's defect.
   - The fix pass changing results without a notice (8: FSL-14 on infinite voxels, `-setbackground` with spline): every default-mode change names its defect (D13).
   - The single cases (the extra "Erroneous dof 3" message; the 2 runs above 3 GiB: measure FSL's peak with the memory pair).
   Re-check the groups with `validation/investigations/fsl-flirt/recheck_findings.py`, not a new full trial.
3. **What part 1 refused:** `-anglerep quaternion`; `alignpaxes`; general parameter-subset masks (the SVD pseudo-inverse: translate the LAPACK routine into the core's `lapack.rs` once, as part 1 did for the others); `-debug`'s BBR volume and its other volumes.
4. **FSL-67's measurement (rule D):** whether dividing BBR's normals by the segmentation's voxel sizes changes the matrices: fMRIPrep's three BBR calls and one strongly anisotropic synthetic segmentation; then rule A (correct by default, `reproduce` keeps FSL's) or rule C (keep), with the numbers.
5. **The hang defects in the suite** (FSL-71 bbrstep 0, FSL-72 an unreadable `read`): one case each, the oracle under a timeout (`oracle_cache.run` with `case_never` or a recorded timeout), larmorx's refusal checked.
6. **FLIRT's other programs**, each a complete replica (every option, message, exit status and failure case) with its CLI (`larmorx-nc fsl <prog>`, `larmorx fsl <prog>` through the dispatch), Python wrapper (`lx.fsl.<prog>`), golden case and spec section: in this order `avscale` (aff2rigid's dependency), `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `applyxfm4D`, `rmsdiff`, `midtrans`, `makerot`, `pointflirt`. What does not fit goes to NOTES.md for a part 3.

## Smart testing: really smart and optimized (the user, 2026-10-28 and 2026-11-01)
The user asked for this explicitly; the orchestrator checks it at the merge.
- **One case per code path**: each new option, program, failure case and defect trigger once; not several images on the same path. fMRIPrep's real calls: the three BBR cases already in the standard tier; do not add the other 19.
- **No heavy testing in this part:** no full tier, no data panel, no long or repeated random-testing runs, no thread-band studies, no repeated benchmarks. The final validation phase does those.
- **Random testing:** the trial's findings re-checked with the reproducers; then **one short trial (at most 30 minutes) of the new paths only** (the new options and programs: restrict the descriptor's grammar for it), every difference triaged.
- **Never compute an original's result twice:** every oracle run through `oracle_cache.run`; re-runs serve the cache; `--oracle-cache refresh` only to prove a varying run.
- **Run only what a change can affect:** during development `cargo test -p <crate>`, targeted `pytest -k`, the suite's `--case`; the parity smoke tier **once**, from the final clean commit, and the golden recording from the same commit (`record_golden.py` reuses its marker); the standard-tier record **once**, from a clean commit, at the end (compare_records against part 1's record: only new or intended changes).
- **mcflirt and BET** only if the core changes: their standard tiers once at the end, compare_records (identical expected).
- Memory and thread tests: add the new programs' calls only where they allocate images or run threads.
- Speed: one informal line per new program only if cheap.

## Documentation
- **Your own files:** the crate, `docs/api/fsl-flirt.md`, `docs/findings/fsl-flirt.md` (restricted), `specs/flirt.md`, the record, the evidence module, the parity suite, the difftest descriptor, permanent cases, `validation/investigations/fsl-flirt/`, the site clone's FLIRT page.
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:** the register's rows and entries, `docs/api/upstream-defects.md`, `validation/tools.toml` (the page, the new programs; then `tables`), golden.md, next-steps item 7, CLAUDE.md's FSL row, `triage.toml` entries settled, the site's status, navigation, Validation page and Roadmap row.

## Defect IDs
New defects only FSL-75 to FSL-89 and PIPE-18 to PIPE-22, each decided by rules A-D and named in the report.

## Machine
- You are the only agent: up to **28 threads** (`cargo -j 28`, parity `--jobs` to match; the smoke tier's default workers); memory boxes together at most **80 GiB** (`scripts/orchestration/boxed.sh`).
- Never touch the main checkout (`<workspace>/larmorx`), `larmorx-heavy` or `oracles/difftest/2026-10-25/`.

## Git and where to work
- Your worktree branch; commit after every meaningful step; never AI attribution, never `--no-verify`; no push, no merge. Never commit while a record or the smoke tier of yours is running in that tree (the smoke marker now refuses a commit that moved during its run).
- Scratch: `<scratch>/` (`<scratch>`; this brief is there as BRIEF.md): NOTES.md current at every step, HANDOFF.md, clones, builds, outputs; never write under a `.claude/` folder. Commit every tool behind a finding or record under `validation/investigations/fsl-flirt/` or `validation/probes/`.
- `source scripts/orchestration/env.sh`; never `maturin develop` (build the wheel into `<scratch>/wheel`, copy the `.so`: `scripts/orchestration/lxv.sh`); build-cache seed: copy the main checkout's `target/`, `crates-gpl/target/`, `crates-nc/target/`, reading only from there.
- Gates before the final commits: `scripts/orchestration/gates-rust.sh <worktree> all`, `gates-python.sh <worktree> --no-build`, the parity smoke tier (once), `python -m larmorx_validation tables --check`.
- Never: delete work products; modify the FSL installations, the oracle builds or any upstream source; ask for sudo.
- Public repository: no network addresses, account names, hostnames or keys; local paths as `<workspace>`, `<scratch>`, `~/fsl-<version>`.
- Wait for long runs with one background wait on the log's final line; never stop silently. If the orchestrator asks for a checkpoint (the weekly usage limit), commit a WIP, update NOTES.md and stop.

## Stop after this part; report (concise)
- The site page (what it shows, the checks); each finding group's cause and decision; the options added; FSL-67's numbers and decision; the programs done and those left.
- The record (counts, compare_records against part 1's), golden, memory, threads, the short trial, new defects with their rule.
- **What you ran, to show the testing was smart:** how many oracle runs were new and how many served by the cache, how often the smoke tier and the record ran, the trial's length and scope.
- Gates, commits, `<scratch>/HANDOFF.md`, anything the rules could not settle, and the "not done" list (only work this kind of part owes: the final validation phase's tests are not "not done").
