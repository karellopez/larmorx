# Final report: A5.1: antsRegistration SyN stages

Agent `aca867b0b4aec5f3f`, 2026-10-20. The agent's report to the orchestrator, verbatim.

## A5 part 1 (SyN stages in antsRegistration): done

Branch `worktree-agent-aca867b0b4aec5f3f`, 20 commits on main `79afd99`, head `229cfa3`. The tree is clean. Nothing is pushed or merged, and no commit carries AI attribution.

**Parity with the GCC 14.3 ANTs 2.6.5 oracle.** Both records were run from the clean commit `a3c6cd6` (`docs/validation/ants-registration*.md`):

| Tier | Cases | Identical to unmodified ANTs | Identical with CORE-MATH preload | Rejected by both | Expected divergences | Failures |
|---|---|---|---|---|---|---|
| standard | 298 | 193 | 32 | 57 | 16 | 0 |
| full | 327 | 194 | 59 | 57 | 17 | 0 |

- **Real data:** sMRIPrep's brain extraction and template registration (Rigid, Affine, SyN) ran whole, with fMRIPrep 25.2.6's own command lines on all five oracle runs, on every third voxel and at full size. All 20 cases are identical to ANTs once CORE-MATH is preloaded. With unmodified ANTs they differ through glibc's `log`, and that difference grows to field differences of about 1.6 mm.
- **sdcflows:** its fieldmap-less SyN settings (`sd_syn.json`) are covered by a synthetic case only, which passes. No oracle run used `--use-syn-sdc`.
- **ANTs' thread band:** SyN fields move by 0 to 3.6e-14 mm. Exceptions: 0.023–0.025 mm with ICP (a race in ITK, ITK-34), and 1.4–1.7 mm on the real registrations (the linear stages' thread dependence carries through). larmorx gives the one-thread result at any thread count.
- **Speed:** on a 96³ CC case ANTs takes 122 s at one thread; larmorx 24–41 s at one thread and 5–9 s at 12, bit-identical. These are informal timings on a loaded machine; the benchmark report is still pending.

**What was built**
- **Rust:**
  - `larmorx_registration::syn`, ITK's SyN method as ANTs runs it.
  - `::field`: displacement-field interpolation, composition, inversion, SyN's smoothing and scaling, resampling between levels, and the field transform itself.
  - Every image metric now handles displacement fields, with `Demons` new.
  - Point-set metrics run inside SyN.
  - Composites can hold fields, and the collapse handles them.
- **Command line** (`larmorx_ants::registration::syn`, `cli/registration.rs`):
  - `SyN[step,update,total]` with ANTs' defaults.
  - The SyN `DIAGNOSTIC` output.
  - `Warp`/`InverseWarp` files and composites with fields.
  - Displacement fields as initial transforms (`-r`/`-q`, images or `.h5` composites, 2D/3D/4D).
  - `--save-state`/`--restore-state` of SyN states and `-i 1`.
  - `--write-interval-volumes` and `--print-similarity-measure-interval`.
  - Every message and failure case.
- **Python:** `RegistrationStage("SyN", ..., update_field_variance=, total_field_variance=)`; the `Warp` files are read back as transforms.
- **Not covered:** the other deformable stages are refused with a message naming A5 part 2 (BSplineSyN, BSpline, Gaussian and B-spline displacement fields) or part 3 (velocity fields, exponential).

**Upstream defects found** (`docs/findings/ants-syn.md` and the register)
- Corrected by default, with a switch that reproduces ANTs:
  - ANTS-73: any image accepted as an initial field (now refused).
  - ANTS-74: `--save-state` writes a stale state when linear stages follow SyN.
  - ITK-30: GC puts the whole derivative into one voxel.
  - ITK-31: stale masks when a stage has only one of its two masks.
  - ITK-32: per-label point-set metrics frozen at the first evaluation.
  - ITK-33: point-set metrics among several metrics never see the deformation.
- Crash case, not reproduced: ANTS-71 (the full-scale similarity crashes); larmorx stops with a message.
- Nondeterminism, not reproduced: ANTS-72 (a linear stage after a field samples voxels seeded from the clock) and ITK-34 (a data race; ANTs changes run to run above one thread).
- ANTS-61 and ITK-22 are extended to SyN.

**For you to decide:** ITK-31's correction changes fMRIPrep's brain extraction by default. On ds000005 the affine is unchanged and the field moves by at most 0.18 mm (median 0.0004 mm), well inside ANTs' own thread band.

**Golden cases:** 4 command-line and 1 Python case added. Checksums were recorded from `89da6c9` after the parity smoke tier passed (1989 tests). No existing checksum changed.

**Gates** (at `89da6c9`, the last code change), all passed:
- fmt, clippy and tests in both Rust workspaces (root and `crates-gpl`).
- `ruff check` and `ruff format --check`.
- `scripts/check_spdx.py`.
- `pytest tests/python` (261 passed) and `tests/golden` (126 passed).

**Validation harness changes**
- New `--jobs N` option runs a suite's cases in worker processes.
- ANTs runs may now take up to 12 hours (the full-size template registration takes ANTs 2.5–3 h at one thread).

**Docs updated:** the API page, the findings, the register, both `PROVENANCE.md` files, the validation index, `golden.md` and `next-steps.md`. CLAUDE.md is untouched, so its status section still needs the A5 entry.

**Follow-ups**
- The benchmark report.
- The A4 linear stages still run in the global thread pool instead of `n_threads` (results are unaffected).
- I deleted the temp directories left by my own killed runs. Older `/tmp/lx-ants-inputs-*` and `/tmp/lx-reg-*` directories from earlier agents remain.

Notes are in `scratchpad/a5-syn/NOTES.md`.
