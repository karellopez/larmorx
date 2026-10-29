You are fixing the small differences that random testing and the data panel found in larmorx's **clean-room originals** of 3dTshift, 3dAutomask, 3dUnifize and the head-motion correction (github.com/karellopez/larmorx; your worktree is a checkout of main). Aim for 4 to 6 hours.

Read first: the repo's `CLAUDE.md` (rules 1-10, "Agent working rules", "Working on this machine"); `docs/porting-guide.md` §1 (who may read what) and its D13 section; `scripts/orchestration/README.md`; `docs/validation/difftest.md` ("Permanent cases", "Replaying"); and your inputs: `specs/3dTshift.md`, `specs/3dAutomask.md`, `specs/3dUnifize.md`, `specs/mcflirt.md`, `crates/larmorx-afni/PROVENANCE.md`, `crates/larmorx-mri/PROVENANCE.md`, `docs/findings/afni-masks-original.md`, and the public defect pages `docs/api/upstream-defects.md`.

**Testing depth (the user, 2026-10-28):** smart, not exhaustive. Each fix gets a permanent random-testing case and the standard tiers prove nothing else moved. No data panel re-runs beyond the five cases named below, no full tiers, no site evidence work.

## Your role: clean-room implementer (strict)
You work from the specs, AFNI's and FSL's documentation and help texts, published papers, the public defect pages, larmorx's own Apache code, and **black-box runs** of:
- the originals: `oracles/afni-25.2.09/bin/` (AFNI) and `~/fsl/bin/mcflirt` (FSL 6.0.7.17);
- the replica programs `larmorx-gpl afni ...` and `larmorx-nc fsl mcflirt`, built with `scripts/orchestration/build-gpl.sh` and `build-nc.sh`. Run them; never open their code.

**You must never open, read, grep or list:**
- AFNI's source: `<workspace>/tags/AFNI-*`, `<workspace>/afni`, `oracles/afni-*/build`;
- FSL's source anywhere (`crates-nc/fsl-source/`, `tags/` FSL trees, `~/fsl/src`);
- the replica workspaces' code: `crates-gpl/`, `crates-nc/`. You may run cargo there to build.
- the restricted findings: `docs/findings/afni-tshift.md`, `afni-automask.md`, `afni-unifize.md`, `afni-licence-routines.md`, `fsl-mcflirt.md`, `fsl-bet.md`;
- the sections of `docs/findings/upstream-defects.md` marked † or "Restricted";
- `docs/parts/`; the table of AFNI routines in `docs/licensing.md`;
- `validation/difftest/triage.toml`, and the permanent cases' descriptions under `validation/difftest/cases/afni-*` and `fsl-*` (written by replica authors).

The parity, evidence and random-testing modules of these tools (`validation/src/larmorx_validation/parity/afni_*.py`, `mri_hmc*.py`, `difftest/tools/afni_masks.py`, `afni_tshift.py`, `mri_hmc.py`, `validation/investigations/difftest/hmc_residuals.py`) were written by replica authors. Run them as black boxes, and edit them only where your fix needs it. Never use their prose about AFNI's or FSL's internals as implementation knowledge.

Whatever you learn from black-box runs that the spec does not say goes back into the spec as lines marked **(observed)**; nothing else in `specs/`. Record exactly what you used in each crate's `PROVENANCE.md`.

## Task: each difference below, by behaviour
For each item:
- **Reproduce it.** The random-testing reproducers are `oracles/difftest/2026-10-25/findings/<tool>/*.json`; pick the files whose `implementation` is `original`. Run them with `python -m larmorx_validation difftest --replay <file>`, or with `difftest --case TOOL:SEED:N` where a case ID is given.
- **Fix the clean-room original** so it matches the original. Where the original has a defect, use the D13 rules (CLAUDE.md; rules A-D decide without asking): the default and `reproduce` behave as the spec and the public defect page say, and each notice's text is exactly the replica's for the same call (compare with a black-box run of the replica).
- **Add one permanent case** per fixed difference (`difftest --promote <finding> --id <name> --tier smoke`).
- If the spec cannot settle an item and black-box runs cannot either, report it; do not guess.

**3dTshift (`crates/larmorx-afni`)**
1. On an input whose metre-unit sform is sheared, the default mode (AFNI-07 corrected) writes the output's pixdim and qform 5.5e-05 to 1.2e-04 away from the replica's default. Reproducers: the `afni-tshift` findings whose check list includes `DN defaults: replica = original`.
2. The written `toffset` differs from AFNI's in its last bits: larmorx 0.35999995, AFNI 0.36000001. Reproducers: the 2 `afni-tshift` findings of the original.

**3dAutomask (`crates/larmorx-afni`)**
3. **Header bytes on tiny oblique images.** The written header differs from AFNI's in its bytes for `mask.nii`, `masked.nii` and `mask.nii.gz`. Inputs: 1 x 1 x 2 constant oblique images, and other tiny oblique inputs.
4. **One-slice oblique constant image.** The mask's data differs from AFNI's.
5. **The `[from ?]` message.** AFNI prints `first 0 x-planes are zero [from L]` (or `[from R]`, `[from S]`, and the same for y-planes); larmorx prints `[from ?]`. Seen on oblique series.
6. **The AFNI-03 notice is missing.** Where the default keeps the times of a header slice code 2, the original names no AFNI-03; the replica names it.
7. **The AFNI-43 notice is missing.** With `-peels 0`, the default changes the clustering messages (405 against 406 voxels) without naming AFNI-43. Case `afni-automask:20261025:118`.
8. **The sign of zero.** `-apply_prefix` writes +0 where AFNI keeps the input's -0 (one value of 294; equal as numbers). Case `afni-automask:20261025:1305`.
9. **The obliquity angle in AFNI's message** (from the data panel). On five of fMRIPrep's calls the mask agrees, but the angle printed differs in its last digits:

   | Case | AFNI | larmorx |
   |---|---|---|
   | `fmriprep/ds000228-task_pixar` | 20.646870 | 20.646889 |
   | `fmriprep/ds002422-task_arithm_run_01` | 6.524229 | 6.524259 |
   | `fmriprep/ds002422-task_arithm_run_02` | 6.524229 | 6.524259 |
   | `fmriprep/ds003592-ses_1_task_rest_echo_1` | 12.675413 | 12.675383 |
   | `fmriprep/ds004958-task_rest` | 24.321316 | 24.321308 |

   - **To reproduce:** `parity afni-automask --tier standard --case 'fmriprep/ds000228-task_pixar'` (the inputs come from the fMRIPrep oracle runs).
   - **To find AFNI's computation:** use black-box runs on headers you construct (precision, the matrix it reads, single or double). Then put the rule into the spec, marked (observed).
   - The same message may appear in 3dUnifize and 3dTshift: check them.

**3dUnifize (`crates/larmorx-afni`)**
10. **The AFNI-03 notice is missing.** Where the default keeps a slice code 2's times, AFNI-03 is not named. One header also differs from AFNI's in its bytes.
11. **The AFNI-08 notice is in the wrong mode.** The default changes the header without a notice, and AFNI-08 is named in `reproduce` only. The notices should be as the replica's.
12. **Header bytes on tiny oblique images.** The bytes differ for `uni.nii`, `uni.nii.gz` and the `-amsave` file (`am.nii`). Inputs: tiny oblique constant images.

**Head-motion correction (`crates/larmorx-mri`, `lx.mri.hmc`, experimental)**
13. **The difference.** On small random phantoms (4 to 8 volumes, at most 32 x 32 x 20), the motion estimates go beyond the record's thresholds against mcflirt. The parameters differ from mcflirt's, but the series is aligned about as well: the corrected series' temporal standard deviation was lower than mcflirt's in 8 of 40 sampled cases, within 1 % in 22, and higher in 10, with a median ratio of 1.000.
    - **Reproducers:** 782 findings.
    - **Measure:** `validation/investigations/difftest/hmc_residuals.py` measures the alignment quality.
    - **Decide:**
      - If it aligns as well on a larger sample, as the spec's behaviour allows (a cost with several minima), record the difference as explained. Give the numbers and reason in an unrestricted findings file (e.g. `docs/findings/mri-hmc-original.md`) and in its record.
      - If a subset aligns clearly worse, find the cause by black-box runs and fix it.
      - Either way, add a permanent case that pins the conclusion (an expected divergence with its reason, or a pass).

## Validation (smart)
- **Standard records** of `afni-tshift`, `afni-automask`, `afni-unifize` and `mri-hmc` (the originals), re-made from clean commits; the oracle cache serves the originals' runs.
- **compare_records** against the committed records (`scripts/orchestration/compare_records.py`):
  - only the cases you fixed may change status, from fail or expected divergence to pass;
  - any other change is a mistake to fix.
  - The five panel cases above must pass.
- **Golden:** golden checksums of the originals that change because of a fix are re-recorded with `scripts/record_golden.py` from a clean commit; say which and why.
- **Smoke tier:** `pytest tests/parity` passes.
- **Random testing:** a short trial of each touched tool (`difftest --tools afni-automask,afni-unifize,afni-tshift,mri-hmc --budget 0.5`, boxed) shows no new difference of the originals.

## Documents
- **Your own files:**
  - the specs' (observed) lines and both `PROVENANCE.md` files;
  - the API pages where behaviour changed;
  - the records, `docs/findings/afni-masks-original.md` and the hmc findings file;
  - the permanent cases.
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:**
  - **`triage.toml`:** each open item's new status and explanation. You cannot read the file; give for each item its tool, its signature (as above) and the new text.
  - **The register and the public defect pages:** only if a notice or a defect's description changes.
  - **The rest:** the validation index and README (generated: edit `validation/tools.toml` and run `python -m larmorx_validation tables`), golden.md, next-steps (item 2 of "The order from here" done; the follow-up "3dAutomask's clean-room original, the panel's cases" done), CLAUDE.md.
  - **The site:** any sentence on these four tools' pages that your fixes make false.
- **New defects:** only AFNI-76 to AFNI-80 and FSL-59 to FSL-63, each decided by rules A-D. None is expected.

## Machine (shared: read this)
- **Threads:** up to **24** (`cargo -j 24`, parity `--jobs` to match).
- **Memory:** your boxes together at most **36 GiB** (`scripts/orchestration/boxed.sh`); every parity tier, random-testing run and measurement runs in a box.
- **Another job runs all along:** the heavy random-testing cases (tmux `heavy`, from the sibling checkout `../larmorx-heavy`, one case at a time in a 60 GiB box). Never touch that checkout, that tmux session, `oracles/difftest/2026-10-25/heavy*`, or the main checkout (`<workspace>/larmorx`).
- **Never** run AFNI options that open a GUI or a browser (`-h_view`, `-h_web`, ...).

## Git and where to work
- **Your worktree branch.** Commit after every meaningful step. Never `Co-Authored-By` or any AI attribution: a commit-msg hook refuses it, and never use `--no-verify`. Do not push or merge.
- **Scratch:** `<scratch>/cleanroom-fixes/`, the folder holding this brief.
  - Keep NOTES.md current; HANDOFF.md, builds and outputs go there.
  - Never write under any `.claude/` folder.
  - Commit every helper behind a result under `validation/investigations/<topic>/`.
- **Environment:** `source scripts/orchestration/env.sh`.
  - Never `maturin develop`: build the wheel into `<scratch>/wheel` and copy the `.so` (`scripts/orchestration/lxv.sh`).
  - Build-cache seed: copy the main checkout's `target/`, `crates-gpl/target/` and `crates-nc/target/` into your worktree, reading only from there.
- **Gates:**
  - `scripts/orchestration/gates-rust.sh <worktree> all` and `gates-python.sh <worktree> --no-build`;
  - `pytest tests/parity`;
  - `python -m larmorx_validation tables --check`;
  - records and golden checksums from clean commits only.
- **Never:** delete work products; modify the oracle builds, the fMRIPrep runs, the cache's existing entries or any upstream source; ask for sudo; put FSL on PATH.
- **Public repository:** no network addresses, account names, hostnames or keys in anything you commit.
- **Waiting:** wait for long runs with one background wait on the log's final line and end your turn until it notifies you; never stop silently.

## Stop after this part; report (concise)
- **Each of the 13 items:** fixed, explained or left, with the cause found by black-box runs.
- **Spec lines added** (observed).
- **compare_records' result per suite**; golden; the smoke tier; the random-testing trial.
- **The clean-room statement:** exactly what you opened and ran.
- **Gates, commits, `<scratch>/HANDOFF.md`,** and anything the rules could not settle.
