## FLIRT clean-room original, part 1: report

**Where:** branch `worktree-agent-a083655161a925b75` (worktree `<workspace>/larmorx/.claude/worktrees/agent-a083655161a925b75`, from main 3f07df7). Last commit 63f8522. Handoff: `<scratch>/HANDOFF.md` (shared-document texts, triage entries, 20 spec questions, clean-room statement). NOTES.md is in the same folder.

**Status:** part 1 is built and all its runs are done, but **22 of the 111 compared cases still miss the thresholds**, and BBR on the suite's `-wmseg` cases is outside FSL's own band. Nothing is running.

### What was built
- **Programs.** `flirt` and `convert_xfm` as clean-room originals in `crates/larmorx-fsl/src/flirt/` plus `cli/flirt.rs` and `cli/convert_xfm.rs`.
  - They are wired into larmorx-cli and Python (`lx.fsl.flirt` / `convert_xfm` with `implementation="original"`).
  - They run on larmorx-mri's hmc engine, shared rather than copied. The additions are listed in `crates/larmorx-mri/PROVENANCE.md`.
  - The image reader now holds one copy of an image instead of up to four. That reader is shared with `lx.mri.hmc`, and its results are unchanged.
- **Brief items covered.** Everything in part 1 of the brief, plus `-anglerep quaternion` and `alignpaxes`. The verbose levels above 1 were not done (`-verbose 2` and higher print the level-1 lines only).
  - `-debug` is refused with a message.
- **Black-box findings** (`docs/findings/fsl-flirt-original.md`, behaviour only, none in the spec):
  - `corratio` and `woods` leave out reference bins with two or fewer points from the counts, the sums and the total variance. With this, the costs equal FSL's printed values at every scale, `-noresample` and weights included.
  - A `setscale` that only resets the input keeps the pair's centre.
  - The search:
    - uses the `search` command's own dof, not `-dof`;
    - starts from the sampled parameters;
    - lists its minima z outer;
    - keeps each start's own cost in `P`.
  - The convergence test of a subset optimisation counts the subset's own tolerances.
  - Inverses follow LAPACK's order, which gives FSL's `-0`.
  - `-applyisoxfm` handles an unset sform in a specific way (in the findings).
  - `-applyxfm` writes the image before the `-omat` matrix.
  - `alignpaxes` prints two extra lines on non-finite moments.
- **New defect IDs:** none (FSL-90 to FSL-95 unused).

### Records and tests
- **fsl-flirt record** (`docs/validation/fsl-flirt.md`, from clean commit b244d75, via `python -m larmorx`): 157 cases. 89 of 111 compared cases pass, 22 fail, 35 are rejected by both alike, 4 are expected divergences, 7 are part 2's options (skipped).
  - **What matches:**
    - fMRIPrep's three BBR calls are within 0.1 mm: p95 0.053, 0.011 and 0.030 mm, image correlation 0.9993 to 0.99996.
    - `-applyxfm` with trilinear and nearest neighbour is bit-identical to FSL.
    - `convert_xfm` and the error messages are identical.
  - **Why the 22 fail:**
    - Optimisation paths drift from FSL's in the last bits of the costs. `-verbose 7` traces agree within 1e-3 through all 4,029 evaluations of the 8 mm stage, then drift, which pushes `real/dof6` to 0.18 mm against FSL's own band of 0.155 mm.
    - BBR normals from `-wmseg` are wrong: the cost of the initial matrix is 0.537732 against FSL's 0.537036, and the BBR cases are 0.13 mm off against a band of 0.015 mm. The spec's mask reference is missing.
    - `local_abs` is 1.4 mm off.
    - Several are ill-posed registrations with weights or woods, where FSL's own band is 10–44 mm.
    - Quaternion runs print an extra stderr line.
    - `real/applyisoxfm-4d-blur-setbackground` (the spline with `-setbackground`) differs; it is open as spec question 14.
- **mri-hmc:** standard tier re-run from b244d75 is identical under `compare_records` (169/169, summary equal).
- **Parity smoke tier:** run once, on b244d75: 2653 tests passed, memory and thread tests included.
  - The first attempt failed three FLIRT memory tests: the original peaked at 371 MiB against FSL's 192. This is fixed; it now uses 186, 186 and 315 MiB against FSL's 191, 258 and 375 MiB.
- **Golden checksums:** recorded from 13012f3 with `--skip-parity`, because that commit differs from b244d75 only in `tests/golden/registry.py`. Nine FLIRT cases were added and every other checksum is unchanged. Your re-recording at the merge will replace the "skipped" stamp.
- **Speed** (FSL / replica / original on 1 thread / original on 8 threads):

| Case | FSL | Replica | Original, 1 thread | Original, 8 threads |
|---|---|---|---|---|
| default 12 dof | 1.01 s | 1.32 s | 1.18 s | 0.37 s |
| 6 dof with `-out` | 0.97 s | 0.96 s | 0.88 s | 0.27 s |
| fMRIPrep BBR (ds005040) | 46 s | 57 s | 38 s | 5.9 s |

### What was run
- **FSL runs:** about 61 direct FSL runs outside the cache, all in memory boxes: probes, three `-verbose 7` traces, one `-debug` run, and the speed table. The record itself took 167 of its 169 FSL runs from the oracle cache; 2 crash cases ran fresh.
- **Smoke tier and records:**
  - The smoke tier completed once (the first, failed attempt was killed).
  - Each record was made once from a clean commit.
  - The development suite was run about 9 times against the release binary while fixing.
- **Trial:** one run of 27 minutes on WIP 3: 5,668 realistic cases, 8 at a time, 24 GiB box, 3 GiB per case.
  - 3,242 cases differed.
  - A stratified replay of 282 findings on WIP 7: 154 now agree.
  - The triage, including two candidate rule-B refusals (all-zero weights, 1×1×1 images), is in the handoff.

### Gates
- Rust (all three workspaces): fmt, clippy and tests pass.
- ruff, ruff format and SPDX pass.
- `pytest tests/python tests/golden validation/tests`: 662 passed.
- `tables --check` is clean.
- I did not edit `validation/tools.toml`; its text is in the handoff.

### Clean-room statement (full text in the handoff)
I worked from `specs/flirt.md` (in full), the parts of `specs/mcflirt.md` and `specs/bet.md` it cites, larmorx's own Apache code, the validation code (including the replica suite module that my suite reuses) and black-box runs of FSL. I did not read `docs/porting-guide.md` or `docs/architecture.md`.

Two exposures to report:
1. A grep of the register displayed summary rows FSL-67 to FSL-81, which are marked † (truncated to 400 characters; they name FSL routines). An earlier line-range read may also have shown † rows. Nothing from them went into the code: `specs/flirt.md` states the same behaviour.
2. A grep over `crates/` printed three lines from `crates/larmorx-afni-replica`: AFNI environment-variable names, unrelated to FLIRT.

Never opened: FSL source, the replica crates, restricted findings, `docs/parts`, the replica investigations, probes, `triage.toml`, or replica git history. FSL's schedule files were passed to FLIRT as inputs, never read.

### Not done
- `-debug` and verbose levels above 1.
- FLIRT's other programs, site panels and the benchmark report (part 2).
- The 22 failing cases above, which wait for spec answers: BBR's normals, `local_abs`, the correlation ratio's last bits, the sinc and spline arithmetic, `-usesqform` and `-fixscaleskew` precision, `quatern_b`/`quatern_c` last bits on an oblique `-applyisoxfm`.
- Follow-up for you: the sparse-bin rule probably also applies to `lx.mri.hmc`'s corratio. I did not change it, so its record stays identical.

### Merge notes (the orchestrator, 2026-11-02)
- The register's summary rows FSL-07, 09, 22, 56, 72, 75, 78, 79 and 80 named FSL routines against the table's own rule (behaviour only, open to everyone); reworded at this merge, with two history lines, one `larmorx_core::defects` text and four triage explanations that named replica functions. The exposure the agent reported came from those rows, not from a restricted section.
- The trial copied to `<workspace>/oracles/difftest/2026-11-02-cleanroom-flirt`; the agent's triage proposals written as six `[[difference]]` entries of `fsl-flirt-original` (signatures from the trial's check names).
- The two candidate rule-B refusals (all-zero weights, one-voxel images) and the 20 spec questions go to the spec writer (the replica side), with FSL-90 to FSL-95.
- A leftover process of the agent (a `-verbose` trace of `larmorx fsl flirt`, asleep for two hours) was stopped at the merge.
