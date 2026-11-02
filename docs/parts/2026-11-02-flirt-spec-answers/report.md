## FLIRT spec answers: the part is finished (relay 3, the last relay)

All 20 of the clean-room implementer's spec questions are answered in `specs/flirt.md`. The two candidate refusals and the leads are decided and registered as FSL-90 to FSL-93. FSL-94 and FSL-95 are unused, and no new **decide** entry was added. The replica's record and the parity smoke tier both pass from the final commit. Nothing was pushed or merged, and nothing of mine is still running.

**Branch:** `worktree-agent-a1899e54855e47816`
**Worktree:** `<workspace>/larmorx/.claude/worktrees/agent-a1899e54855e47816`
**Base:** main fb1de31

### The questions
Every precision answer is checked by a model written only from the spec's new text (in `validation/investigations/flirt-spec-answers/`). All 20 are high confidence, except question 5's case outcome (medium). "Right / incomplete / wrong" refers to the implementer's observation.

| # | Topic | Answered in | Observation | Model's check against FSL 6.0.7.23 |
|---|---|---|---|---|
| 1 | Sparse bins in `corratio` and `woods` | §6.2, mcflirt appendix A | right | six `corratio` and `woods` values on two images |
| 2 | Woods' formula | §6.2 | incomplete: a bin whose sum is not positive adds `n_b²σ_b`; appendix A corrected | 37174512 |
| 3 | The correlation ratio's last bits | §6.2, §12 | incomplete: two missing double-to-single roundings | the search's first sweep, 23 of 23 evaluations, 9th and 10th included |
| 4 | BBR's normals | §6.5 | wrong reference: a Gaussian of σ = 2 mm and weights 3, 1, 1/3; the voxel order was right | 0.537036 and all 16 printed costs |
| 5 | `-bbrtype local_abs` | §6.5 | right; the error came from the normals | the four `local_abs` costs |
| 6 | The search | §8.1 | (a), (c), (d), (e) right; (b) incomplete: the minima are composed, then decomposed | FSL's four messages on the five minima |
| 7, 8, 10, 12, 17, 18, 19, 20 | confirmations | §8.2, §7.2, §9.3, §8.7, §7.4, §4.2, §8.2, §9.2 | right (19 in the L1 sense) | from the source |
| 9 | `-applyisoxfm` header | §4.1, §9.3 | right for the unset sform; incomplete for the qform | 12 header fields |
| 11 | The inverse's zeros | §12 | right; fused multiply-adds change nothing | FSL's two `-0` |
| 13 | Windowed sinc | §9.2 | incomplete | 40000 of 40000 voxels |
| 14 | Spline with `-setbackground` | §9.3 | incomplete | 172800 of 172800 values |
| 15 | `-usesqform` | §4.1, §8.5 | the single-precision step found: the field of view | `-13.74984741` |
| 16 | `-fixscaleskew` | §3 | incomplete | ten digits |

§14's table has rows 1 to 25 (the 20 questions, the two refusals and the three leads).

**The hmc sentence** (`specs/mcflirt.md` appendix A): mcflirt's `-cost corratio` and `woods` are FLIRT's two costs unchanged. The sparse bins are left out of the count and the total variance too, Woods' terms are as stated there, and §6.2's sums and roundings apply. mcflirt's `corratio` takes the edge weights unless `-smooth 0`; its `woods` never does.

### Refusals and leads decided
Each is in the replica with its `reproduce` switch, in `larmorx_core::defects` (behaviour only), in spec §11, and has a parity case.

- **FSL-90, rule A** (relay 2's lead). With `-anglerep quaternion`, the search's starting points are Euler angles but FLIRT reads them as a quaternion. On `real/anglerep-quaternion`, four of the five lose their rotation.
  - Default: composed as Euler angles.
  - `reproduce`: FLIRT's composition with its messages.
  - Measured: FLIRT's matrix is 0.37 mm from the Euler run's; the corrected one is 0.23 mm (and 0.19 mm from FLIRT's).
- **FSL-91, rule B.** A `-refweight` or `-inweight` with no voxel of at least 0.01 (an empty mask). The costs are not 0/0, as the implementer thought, but constant; BBR's cost is NaN.
  - FSL is deterministic: it returns rotations of 95 and 143 degrees, and BBR prints 2339 NaN warnings.
  - Default: refused (exit 1, nothing written). `reproduce`: FLIRT's run.
- **FSL-92, rule B.** An input or reference of one voxel (1 × 1 × 1). FSL returns an arbitrary rotation of its search grid (95 to 143 degrees), deterministically.
  - Default: refused. `reproduce`: FLIRT's run.
- **FSL-93, rule A** (relay 2's lead). A header voxel size of 0 makes the base scale 0. Every voxel size becomes infinite, the initial matrix NaN, and a pyramid size an undefined conversion of infinity (−2147483648).
  - FSL never finishes: it was still running at the cache's 60-minute timeout.
  - Default: the 0 counts as 1, as FLIRT's own image reading counts it, and the defect is named. `reproduce`: refused.
  - `-basescale 0`: refused in both modes.
- **Zero BBR normal:** left as documented in §6.5, not registered.

### What was run
- **FSL:** 10 new runs through the oracle cache (`default12`, the five `zero-*` runs including `zero-voxsize`, the three `onevox-*` runs). `quat` came from the cache. `zero-voxsize` timed out and was never stored. The new suite cases were served from those runs, plus one new run (`real/applyxfm-nan-border`). The hang and crash cases run fresh each time by design.
- **Replica in both modes** (`replica_runs.py`): `reproduce` matches FSL's matrix and standard error on all 8 measured runs.
- **The replica's standard tier**, once from 7e43e5c: it failed on `edge/background-overflow`, because FSL-92 now refuses that case's one-voxel input by default. That record is kept in `<scratch>/record-7e43e5c/`.
  - Fixes: the case now uses a 2-voxel input (FSL still crashes there; the replica names FSL-75), and a new case `real/applyxfm-nan-border` keeps FSL-22 covered in FLIRT's padding.
  - Re-run from 2c21ac6: 302 cases, 209 of 209 compared bit-identical, 87 rejected by both, 6 expected divergences, 0 failures.
  - `compare_records`: no earlier case changed status. Only two fix-pass counts changed: `real/anglerep-quaternion` (FSL-90) and `difftest/background-nan-order` (now refused by FSL-92 by default).
- **Parity smoke tier** from 9b8e98d: passed, 2653 tests, 12 workers.
- **Golden cases:** larmorx-nc's re-recorded from 6af5ff9 with three new cases; every other checksum is unchanged.

### Gates
- Rust fmt, clippy and test in root, `crates-gpl` and `crates-nc`: pass.
- ruff, `check_spdx`: pass.
- pytest `tests/python tests/golden validation/tests`: 665 passed.

### Commits
d3b31ed, c8ce906, 63321d9, b727a0c (relays 1 and 2); 193b27b, 6af5ff9, 7e43e5c, 2044c75, 2c21ac6, 9b8e98d (relay 3).

### Handoff files
- `<scratch>/HANDOFF.toml`: 9 operations; `--check` passes in the worktree. They cover:
  - the register: four rows, the history paragraph and a detailed section
  - `docs/api/upstream-defects.md`
  - `docs/api/fsl-flirt.md`: shared with the clean-room part 2, so apply it after both merges
  - next-steps item 7: the questions answered, and the hmc follow-up answered
  - `tools.toml`: FSL-93 added to the expected divergences
  - `triage.toml`
- `<scratch>/HANDOFF.md`: the reasons.

### Not done
- **The clean-room suite will fail on the new cases until the original catches up.** `validation/src/larmorx_validation/parity/fsl_flirt.py` is the clean-room's file and was not touched. It runs the replica suite's cases, so it gains the six new ones. Its `_undefined_case` does not know `hang_fixed`, which `edge/zero-voxel-size` needs. The clean-room original does not yet implement FSL-90 to FSL-93 (spec §11). This is an item for the clean-room part 2.
- No random-testing trial, by the smart-testing rule.
- No site change.

### Merge notes (the orchestrator, 2026-11-02)
- The relays' reports (relay 1: questions 1 to 5, 30 minutes; relay 2: questions 6 to 20; relay 3: this report) were read at each step; each step's new spec text was scanned for FSL's routine names (none; one printed message, `eig_sym(): given matrix is not symmetric`, is FSL's output).
- 8 of the 9 handoff operations applied with `scripts/orchestration/apply_handoff.py` at this merge (its first real use); the `docs/api/fsl-flirt.md` operation waits for the clean-room part 2's merge.
- `bbr_model.py` named the workspace by its absolute path; replaced by the parent that holds `oracles/`.
- The user's decision of the day (spec answers before FAST) in the decision log.
