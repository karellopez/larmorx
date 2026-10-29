## Version update, part 1: AFNI 25.2.09 → 26.3.05 (2026-10-29): report

The GPL replicas now follow AFNI 26.3.05. Between the two releases, the code these three programs run changed in only two places. Both are fixes of defects larmorx already corrected, and AFNI fixed them exactly the way larmorx did. Every gate passes. Defect IDs used: none (no new defect found).

- **Branch:** `worktree-agent-a08130d4a33db225a`, 11 commits from 17fbfdd: fad2f1b, 199d7d6, fc363d5, 3ecd166, 8218894, dc84558, bd39a5d, a5601f1, 39ba240, 5264171, a562e78.
- **Site branch:** `version-afni-26` in `<scratch>/version-afni/site`, commits 4df1bc5 and abc7dbe.
- **Handoff:** `<scratch>/version-afni/HANDOFF.md` (here `handoff.md`), sections 1–14.

**Upstream watch.** `scripts/orchestration/upstream_watch.py`, with a test on a fake workspace (`validation/tests/test_upstream_watch.py`); the nightly job runs it once a week and marks changes that are only licence text. First report: `<workspace>/oracles/logs/upstream-watch/upstream-watch-2026-10-29.md` (in `handoff.md`, section 11).
- **Already newest:** ANTs 2.6.5, FreeSurfer 8.2.0, nibabel 5.4.2, nitransforms 25.1.0.
- **ITK** 5.4.5 → 5.4.8: follows ANTs; the new tag is not fetched.
- **AdaptiveDenoising and CORE-MATH:** both master branches moved.
- **glibc** 2.35 → 2.44: informational, larmorx follows the oracles' glibc.
- **AFNI** 25.2.09 → 26.3.05.
- **OpenBLAS** 0.3.34: 4 of 26 named files changed (`level3.c`, `gemm.c`, `gesv.c`, `param.h`).
- **SciPy** 1.18.1: 5 of 5 changed. **numpy** 2.5.4: 3 of 3 changed.
- **fMRIPrep:** 25.2.7 is out.
- **FSL** 6.0.7.23: 15 of 57 replicated files changed and `znzlib.c` removed; 8 packages changed version.

**What changed in AFNI.** Every file named in PROVENANCE, plus every function of the three binaries by machine code (both builds use the same GCC and flags).
- **27 files: licence header only** (GPL-2 → CC BY 4.0). No effect.
- **`thd_niftiread.c`:** AFNI-03 fixed (header slice code 2). Affects all three programs.
- **`3dTshift.c`:** AFNI-05 fixed (`-no_detrend`). Both fixes are exactly larmorx's correction, so the replicas now take one path in both modes and print no notice; `reproduce` gives 26.3.05's result, which equals the default.
- **No effect on these programs:** `thd_automask.c` (a new `THD_mask_erode2D`, used only by code these programs don't link); `3ddata.h`, `mrilib.h` (declarations and includes); `AFNI_atlas_spaces.niml` (atlases only); `machdep` (a macOS check); five help-search routines (`-h_*`, `-all_opts`); `-help` text identical except the build line.
- **Harness:** `--afni-version` / `LARMORX_AFNI_VERSION` picks the build: 26.3.05 by default for the replicas, 25.2.09 for the clean-room originals. Every replica case also runs AFNI 25.2.09 and reports "AFNI's own step". The stage comparisons stay pinned to 25.2.09.

**Records (replica, standard tier against 26.3.05, from the clean commit bd39a5d).**

| Replica | Cases | Compared cases that write outputs, bit-identical to 26.3.05 | AFNI's own cases changed 25.2.09 → 26.3.05 |
|---|---|---|---|
| 3dTshift | 315 | 280 of 280 | 6: 4 by AFNI-05, 2 by AFNI-03 |
| 3dAutomask | 125 | 101 of 101 | 1, by AFNI-03 |
| 3dUnifize | 116 | 90 of 90 | 1, by AFNI-03 |

- No AFNI change without a known reason. compare_records against the 25.2.09 records: no status change; the +1 case in 3dAutomask and 3dUnifize is a new permanent case each (`slice-code-2-masked-afni26`, `epi-slice-code-2-afni26`).
- Golden: the GPL checksums re-recorded; `3dTshift.cubic.float32-defects.reproduce` changed and `3dTshift.heptic.uint8.no_detrend.reproduce` removed (one path now); the other 39 unchanged.

**Clean-room originals against 26.3.05** (code not touched): 3dTshift fails 6 of 315 (the 4 `-no_detrend` cases and the 2 header code-2 cases), 3dAutomask 1 of 130 and 3dUnifize 2 of 117 (their slice-code-2 permanent cases); everything else agrees. Their records and default build stay on 25.2.09. The list for the clean-room agent: (1) `-no_detrend`: `reproduce` equals `fix`, AFNI-05 never named; (2) header slice code 2 in all three: `reproduce` equals `fix`, AFNI-03 never named; (3) re-describe their three slice-code-2 permanent cases; (4) re-make their records against 26.3.05 and set `DEFAULT["original"]` to 26.3.05; (5) the version strings in PROVENANCE. Specs updated as spec writer, every change marked "(AFNI 26.3.05)": 3dTshift §3.2, §4, §6.4, §6.5; 3dAutomask §9; 3dUnifize.

**Relicensing inventory** (`docs/findings/afni-relicensing-26.md`; nothing moved):
1. AFNI 26.3.05 relicenses all MCW code under CC BY 4.0 (MCW's grant of May 12, 2026).
2. Of the 55 AFNI files the replicas derive from, 30 MCW files went GPL-2 → CC BY 4.0; 24 have no notice (NIH, public domain); the NIfTI library is public domain. None is still GPL.
3. `sinf`/`cosf` come from Arm's MIT routines, the same bits: the FMA build proven on all 2^32 inputs by crates-nc's verifier; for the SSE2 build the expression trees and constants are identical. The exhaustive C check is committed but was not run (compiling the downloaded source was refused by the permission check).
4. `tanhf`/`expm1f` are fdlibm under Sun's notice, already permissive.
5. `cbrtf` is glibc's own LGPL code with no permissive equal-bit source known; it feeds 3dUnifize `-EPI`'s radius; whether its last bit matters is not measured.

A move would need the CC BY 4.0 attribution per file and in NOTICE, CC BY 4.0's text, `sinf`/`cosf` re-sourced from Arm, `cbrtf` measured or kept apart, the larmorx-gpl-only code relicensed by its author, the crate moved to `crates/`, and the GPL jobs reduced to whatever remains (Workbench, later). The user's decision; recommendation: yes, as its own part.

**Random-testing trial** (0.5 h, `--afni-version 26.3.05`): 13,063 cases. Replicas: no difference. Clean-room originals: 190 findings and 62 notices, every one AFNI-03 or AFNI-05 (`validation/investigations/version-afni/triage_trial.py`).

**Site.** Every example gains an AFNI 26.3.05 panel, "replica − AFNI 26.3.05", and "AFNI 26.3.05 − AFNI 25.2.09" as a reference row (the same bytes on every call shown); the page texts say which version each record and panel ran; no new image blobs. Inventory: no loss on any page; shown numbers 155→159 (3dTshift), 145→154 (3dAutomask), 174→180 (3dUnifize), the only changed numbers increases from the new cases. Number check 158/158, 153/153, 179/179, Validation 350/350; build 0 problems; screenshots before and after in `<scratch>/version-afni/shots/`.

**Gates.** Rust: all three workspaces OK. Python: 628 passed, ruff and SPDX OK. `pytest tests/parity`: 2564 passed. `tables --check`: OK. (In a worktree, `env.sh` defaulted `LARMORX_WORKSPACE` to `.claude/worktrees`; the first gate run failed on the missing venv and passed with the variable set.)

### At the merge (orchestrator)
- The handoff's texts integrated (register rows, sections and history; the public defect page; licensing; `UPSTREAM.md`; `upstream.tsv`; CLAUDE.md rule 2, the AFNI row and the oracle builds; next-steps item 3, the clean-room alignment follow-up and the relicensing question under "Waiting for the user"; golden.md; the decision log). The four triage entries link committed files (the version-afni README and the clean-room originals' own permanent cases, which reproduce AFNI-03 against 26.3.05) instead of the trial's findings outside git.
- `scripts/orchestration/env.sh`: in a git worktree the workspace is now the main checkout's parent (found with `git rev-parse --git-common-dir`), so the gates run in a worktree without setting `LARMORX_WORKSPACE`.
