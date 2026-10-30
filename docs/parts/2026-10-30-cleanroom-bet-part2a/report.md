## BET clean-room original, part 2a: report (2026-10-30, 21:14 to 22:46)

Everything is done and committed. All 142 standard-tier cases now agree with FSL 6.0.7.23 (before: 3 failures). One decision is for the user: BET's default `tanh` (below). Branch `worktree-agent-a7cce9b972a4c0b86`, 13 commits on e90a5e0 (b721ade to 956c3a0). The final code commit is 8abb5b4, and the record is made from it; the tree is clean. Handoff: `<scratch>/HANDOFF.md`; notes for 2b: `<scratch>/NOTES.md`.

### What changed
- **glibc's `tanh`** (`larmorx_core::math::tanh_glibc`): written from netlib's fdlibm 5.3 (Sun's notice kept). fdlibm as written differs from glibc on about 0.03 % of arguments. Glibc ships Shimizu's 1997 revision, which evaluates `expm1`'s polynomial in Estrin's order; I found that order by black-box testing. With it, **12,918,456,348 of 12,918,456,348 inputs are identical** to the system's `tanh`. The verifier is in `validation/investigations/cleanroom-bet/tanh-verify/`.
- **§8.1 in full:**
  - the mesh held in the file's triangle order, with §4.4's move-to-the-end neighbour lists;
  - the normal and the neighbour mean computed by three divisions;
  - the placement `c + (r/2)((p + c) − c)`, `1/R` from `s_n·n`, and the update `p + ((u1 + u2) + u3)`;
  - the inward depth accumulated, the field of view and `rmax` products in float, `d1`/`d2` truncated;
  - the centre of gravity as one running sum, the radius's float products;
  - the robust range's exact arithmetic, and division in the rasterisation;
  - the self-intersection measure with squared distances and coordinate adjacency.
- **Other spec items:**
  - the `-e` sweep and predicate (§4.5);
  - **the skull search (§4.7)** with §8.1's trilinear formula;
  - **the quaternion's precision path (§8.2)**, with an x87 helper `bet/x87.rs` modelled on larmorx-afni's, and NIfTI-2 doubles;
  - **§3.1**'s name fallback and **§5.2**'s float64 `-F` brain.
- **Found during the retrial:**
  - `bet -Z` now models the quaternion round trip once per FSL program in the chain. The round trip is not idempotent, and this explained 5 cases.
  - The script's steps print FSL's `FSLOUTPUTTYPE` messages.
- **(c):**
  - CORE-MATH's double `tanh` is ported as `larmorx_core::math::tanh`. It is bit-identical to the C on 62,880 worst cases (both signs) and 805 million random inputs; the FMA copy is tested too.
  - GLIBC-06's machinery is wired (`defects::GLIBC_06`, `Deformation::tanh`, a notice naming each output that changes).
  - **BET's default stays on glibc's `tanh`** (see the decision below).
- **Suite (`parity/fsl_bet.py`):** quaternion tolerance 0, and the skull held to the masks' thresholds (part 1 only reported its Dice).
- **Tables:** `validation/tools.toml`'s fsl-bet lines updated, so the generated tables regenerated (786e708). This touches the generated blocks of README.md and docs/validation/README.md; see the handoff.

### The three cases, record, compare_records
- **`real/older-75y`, `real/7t`, `real/tumour`: bit-identical to FSL** (brain, mask, outline, mesh, skull, verbose streams with the rerun totals).
- **Record** (`docs/validation/fsl-bet.md`, from 8abb5b4): **all cases agree**. 111 pass, 24 rejected by both, 7 expected divergences, 0 failures.
  - 217 of 229 brains, masks and outlines are bit-identical, and 12 more are within thresholds.
  - Skulls 13 of 13 bit-identical; headers exact, quaternion included; fMRIPrep's 29 calls bit-identical.
- **compare_records against main's:** 3 statuses go from fail to pass (the three cases). 20 more cases have different details, all improvements (`--binaryMask` cases and skulls now bit-identical); nothing worse.

### The trial's 75 cases
`retrial.py` with the strict harness: **237 of the 239 differing cases agree; 73 of the 75 open ones**. The two left are #677 (`-c` on a neurologically stored image) and #622 (`bet -R`: FSL's `fslstats -C` prints `-0.000000`, so its `dc` prints `dc: stack empty` twice; every file is FSL's). Both come from FSL's 4 × 4 inverse when converting a centre between voxel frames, which the spec leaves open: one question for the spec writer.

### Golden, gates, speed
- **Golden:** re-recorded from 786e708, with the parity smoke tier passing (2,607 tests). Only BET's four cases changed (`cli.fsl.bet`, `cli.fsl.bet.variations`, `cli.fsl.bet2`, `py.fsl.bet`). `--check` at 8abb5b4: 164 of 164 agree.
- **Gates:**
  - `gates-rust.sh all` passes at 8abb5b4 (root, crates-gpl, crates-nc).
  - ruff, ruff format, check_spdx, `tables --check` pass.
  - pytest: 643 passed, 13 skipped serially. Two oracle-cache tests each failed once under 16 xdist workers and pass alone and on rerun: timing flakes.
- **Speed is unchanged:** FSL 1.26 and 3.21 s; the original 0.49 and 1.21 s on one thread, 0.17 and 0.35 s on 24 (ds000005, ds002422).

### Decision for the user: BET's default `tanh` (GLIBC-06)
The rules don't settle it: the user's principle of 2026-10-30 (most accurate maths by default) conflicts with rule C (keep a correction measured to bring no gain) and with the speed goal.
- **Measured** with `crates/larmorx-fsl/examples/tanh_effect.rs` on the 13 real images:
  - 9 give the same outputs with either `tanh`.
  - 4 give different masks: the three rerun cases (Dice 0.9959, 0.9848, 0.9920) and ds000005's BOLD first volume (0.9966). There the correctly rounded surface is not more accurate, just a different one, further from FSL's.
- **Cost:** an exact notice needs a second surface search in practically every run, which makes the default 1.9 times slower (0.43 → 0.83 s and 1.20 → 2.33 s on one thread; still faster than FSL).
- **My recommendation: keep glibc's `tanh` as the default (rule C)**, with the correctly rounded one available on request. That is the branch's state. Switching later is one line in `cli/bet2.rs`.

### Also in the handoff
- The register's GLIBC-06 row and `docs/api/upstream-defects.md` text.
- next-steps item 7 for part 2b.
- Triage notes for #622 and #677.
- golden.md text and the Roadmap row.
- Two tidy-ups for the merge:
  - add `LICENSES/LicenseRef-fdlibm.txt`;
  - remove the now unused `libm` dependency from larmorx-fsl (this changes the three lockfiles, so I left it to the merge).

### Clean-room statement
- **Opened:**
  - the brief, rules.md, CLAUDE.md, ORCHESTRATOR-STATE (imported);
  - my own part's folder (`docs/parts/2026-10-30-cleanroom-bet-part1/{report,notes-for-part2}.md`);
  - `specs/bet.md` (§3 to §5.2, §8 to §11), `validation/investigations/bet-spec-answers/README.md`, the cleanroom-cbrtf README and verifier;
  - larmorx-core (math, defects GLIBC-05 lines, array, PROVENANCE), larmorx-afni `x87.rs` and `unifize/mod.rs` lines 440-476, larmorx-mri `hmc/files.rs` lines 150-330, all of larmorx-fsl;
  - the cleanroom-bet investigation scripts, `parity/fsl_bet.py` (mechanics), `difftest/tools/fsl_bet.py` lines 300-335, `validation/tools.toml` (fsl-bet lines);
  - the orchestration scripts env.sh, gates-rust.sh, compare_records.py and `record_golden.py`;
  - my docs (`fsl-bet-original.md`, `docs/api/fsl-bet.md`, the record);
  - netlib's fdlibm `s_tanh.c`, `s_expm1.c`, `fdlibm.h` and readme;
  - CORE-MATH's `tanh.c` and `tanh.wc`, MIT, from `tags/core-math` at the pinned 040ee482.
- **Ran as black boxes:** the system's `tanh`; FSL 6.0.7.23's `bet2`, `bet` and `fslstats` (`-C`, `-R -m` on FSL's own output); CORE-MATH's C compiled; the crates-nc and crates-gpl gates.
- **Never opened:** FSL's source, `crates-nc/`, `crates-gpl/`, `larmorx-afni-replica`, the restricted findings, the register, other `docs/parts/` folders, replica investigations, `triage.toml`, the permanent cases, `fsl_versions.py`, glibc's source.

### Merge note (orchestrator)
The two tidy-ups were done at the merge: `LICENSES/LicenseRef-fdlibm.txt` and NOTICE's entries
(fdlibm 5.3, CORE-MATH's `tanh.c`); `libm` removed from larmorx-fsl (only the root `Cargo.lock`
changed). GLIBC-06 entered the register as the one **decide** entry.
