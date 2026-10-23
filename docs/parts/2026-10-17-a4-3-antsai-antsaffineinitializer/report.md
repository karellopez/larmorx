# Final report: A4.3: antsAI, antsAffineInitializer

Agent `a00b4c84605ebf2d7`, 2026-10-17. The agent's report to the orchestrator, verbatim.

## A4 part 3 (Part A only): antsAI and antsAffineInitializer

Part A is done to a clean checkpoint with one gap: `antsAI -b`, blob alignment. Worktree `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a00b4c84605ebf2d7`, branch `worktree-agent-a00b4c84605ebf2d7`, HEAD `4f2178f`, tree clean. 16 commits since `e93eb60`, none with AI attribution. Nothing pushed or merged, CLAUDE.md not edited, Part B not started.

**What exists now**
- **Rust:** `larmorx_ants::ai` and `cli::ai` (antsAI), `larmorx_ants::affine_initializer` and `cli::affine_initializer` (antsAffineInitializer).
  - Supporting pieces: the multi-start optimizer, image moments, the `SetMatrix` family for the rigid and similarity transforms, and the affine rotate/translate/scale operations.
  - In `larmorx-core`: vnl's eigen solvers and fixed-size inverses, and correctly rounded `asin`, `acos`, `atan2`, `cbrt` (the preload library now replaces these too; the old one is archived in `archive/2026-10-17-crmath-preload/`).
- **Command line:** `larmorx ants antsAI`, `larmorx ants antsAffineInitializer`. Every option, default, menu, message, exit status and abort text is reproduced, in 2D and 3D, plus 4D for antsAffineInitializer.
- **Python:** `lx.ants.ai` and `lx.ants.affine_initializer` in `python/larmorx/ants/initializers.py`.
- **Docs:** `docs/api/ants-ai.md`, `docs/api/ants-affine-initializer.md`, `docs/findings/ants-ai.md`, PROVENANCE files, the index pages, next-steps and a porting-guide note.

**Parity against the GCC 14.3 ANTs 2.6.5 oracle, one thread** (0 failures in every tier)

| Suite | Tier | Cases | Bit-identical to unmodified ANTs | Same once CORE-MATH replaces glibc's maths | Both reject | Expected divergences |
|---|---|---|---|---|---|---|
| ants-ai | standard | 102 | 51 | 1 (glibc `cbrt`) | 46 | 4 |
| ants-ai | full | 110 | 59 | 1 | 46 | 4 |
| ants-affine-initializer | standard | 38 | 23 | 0 | 13 | 2 |
| ants-affine-initializer | full | 43 | 28 | 0 | 13 | 2 |

- **"Both reject"** includes the cases where ANTs aborts or crashes. There, everything ANTs printed before stopping is identical, and larmorx adds one line and exits 1.
- **Expected divergences:** loops ANTs never finishes (larmorx refuses) and clock-seeded runs (ANTs is not repeatable; larmorx is).
- **Thread band:** ANTs at 2, 4 and 8 threads moves up to 2.4e-13 in a parameter (antsAffineInitializer on the MNI template: 9e-7). larmorx gives the one-thread result at any thread count.
- **Real data:** I re-ran fMRIPrep's own `init_aff` command lines with fMRIPrep's seed: all ten nodes across the five oracle runs.
  - larmorx is within 1.5e-9 of fMRIPrep's own transforms; nine of the ten are within 1.3e-12.
  - fMRIPrep's image (ANTs 2.6.2) at one thread is bit-identical to the oracle and to larmorx.
  - fMRIPrep's runs also called antsAffineInitializer inside the template registration. nipype kept neither its inputs nor its output, so the full tier runs that call on stand-ins with nothing of fMRIPrep's to compare against.

**This changes antsRegistration (A4 part 2) output**
- The gradient filter's components must be rounded to float, as ITK does. Part 2's 2 mm phantoms could not show this; non-power-of-two spacings do.
- I fixed it, added two oblique-grid gradient-filter cases to ants-registration, and re-ran its standard record: 168 cases, 0 failures, 104 bit-identical. Its full record predates the two new cases.
- One golden checksum changed for this reason: the composite-affine stage of `cli.ants.antsRegistration.metrics`.
- The zero-mass exception now reports ITK's line 123 instead of 172.

**Golden tests:** five new cases (`cli.ants.antsAI`, `cli.ants.antsAI.reproduce`, `cli.ants.antsAffineInitializer`, `py.ants.ai`, `py.ants.affine_initializer`), about 1.1 s in total. Recorded from `81ac401` after the parity smoke tier passed (1988 tests); 116 cases overall.

**Defect register entries added**
- Corrected by default, with a switch to reproduce ANTs: ANTS-52 (the translation grid is ignored by some transforms), ANTS-53 (the alignments with iterations write an empty file), ANTS-56 (an unknown dimension runs as 2D), ITK-19 (a start point that throws keeps the previous value).
- Refused by default, with a switch: ANTS-55 (a short grid is read past its end).
- Crashes or nondeterminism, never reproduced: ANTS-57, ANTS-58, ANTS-60, ITK-20.
- Documentation only: ANTS-59.
- I recounted the register's class totals from its table: 160 entries.

**Gates passed:** fmt, clippy and tests in both Rust workspaces; ruff; SPDX check; `pytest tests/python tests/golden` (375 passed); parity smoke tier. I refreshed the NOTICE copy in `crates-gpl`. No dependencies changed, so `crates-gpl/Cargo.lock` is untouched.

**Not done**
- **`antsAI -b` (blob alignment):** refused with a message. A bit-exact port would need ANTs' blob detector, float patch correlations, LINPACK's QR and Eigen's vectorised `SelfAdjointEigenSolver`. The requirements are written up in `docs/findings/ants-ai.md`, section "Blobs". fMRIPrep never uses this option.
- **One antsRegistration message gap:** on a constant image, vnl's "suspicious return value from SVDC" warning is not printed. The transform is identical; this is recorded in `docs/findings/ants-registration.md`.

**Decisions for you**
- ANTS-54: antsAffineInitializer ignores its local-search argument. niworkflows passes 10 and ANTs always runs 20; larmorx currently does what ANTs does.
- ITK-21: antsAffineInitializer's learning rate carries over from one start point to the next. Correct it by default or keep it?
- Whether to port `-b` at all.

Logs are in `oracles/logs/a43-*.log`; resumable notes are in the scratchpad at `a4-ai/NOTES.md`.
