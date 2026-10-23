# Final report: D13 part 2b: GPL 3dTshift replica

Agent `a79c06fc44bc6b3ec`, 2026-10-12. The agent's report to the orchestrator, verbatim.

## D13 part 2b: done

The work is on branch `worktree-agent-a79c06fc44bc6b3ec` (worktree `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a79c06fc44bc6b3ec`). `main` (b1a7422) is merged in. Nothing is pushed.

**Gates.**
- Both Rust workspaces pass fmt, clippy with `-D warnings`, and `cargo test --locked`. The GPL golden test passes once its checksums were recorded.
- After `maturin develop --release`, `pytest tests/python tests/golden` passes (345).
- `ruff check`, `ruff format --check` and `check_spdx.py` pass.
- The AFNI smoke-tier parity (`tests/parity/test_afni_tshift*.py`) passes: 454 cases.
- `maturin develop` was run in my worktree, so the venv's `larmorx.pth` now points to it. I ran it after your ANTs job had already imported larmorx, and that job has since finished (ALLDONE). Re-run `maturin develop` in the main checkout when you merge.

### The AFNI defects in the replica
Each is corrected by default and named on stderr as `*+ WARNING: upstream defect …` (the `Notice::line()` format, which `_replica.py` turns into warnings). With `reproduce`, the replica gives AFNI's bytes. Results from the fix pass of the standard tier:

| Defect | What the fix changed |
|---|---|
| AFNI-01 (negative slope) | 7 cases, values up to 8.5e3 stored units |
| AFNI-02 (float32 scaled twice) | 2 cases, `scl_slope` |
| AFNI-03 (header slice code 2) | 1 case, up to 27.8 |
| AFNI-05 (`-no_detrend` voxel pairs) | 4 cases, up to 122 |
| AFNI-07 (metre/micrometre units) | 3 cases, sform and qform origin |
| AFNI-08 | 1 case, `slice_end` |

- **AFNI-08's trigger, found in AFNI's source:** a copy of the input that keeps slice times all within 3 ms of 0. That happens when the times lie beyond the TR, e.g. header code 1, 0.5 ms apart, TR 1 ms. I confirmed it with the AFNI binary. By default the replica writes no slice code and `slice_end` = nz−1.
- **AFNI-11 and AFNI-12** (existing output; refused `-prefix` names) also got a switch, under the decided item 5 ("failures get a switch too"). `reproduce` behaves as AFNI does with its environment variables unset.
- **AFNI-06** stays faithful: it is questionable and waits for your decision.
- **New register entry AFNI-16 (questionable):** AFNI writes a grid less than about 0.014° from plumb as exactly plumb. The replica reproduces this in both modes.

### The sform bug
The cause was broader than the units. AFNI does not write the matrix it read: for a grid it counts as plumb, it writes the cardinal matrix built from its axes (`THD_set_dataset_attributes`). The new module `crates-gpl/larmorx-gpl-afni/src/axes.rs` translates the axes, the obliquity test, the cardinal matrix and NIfTI's quaternion.
- In `reproduce` mode the 348-byte header now matches AFNI's byte for byte, except `vox_offset` (AFNI adds its extension).
- I checked this on plumb, oblique, nearly plumb, qform-only, sform-only and form-less inputs in mm, metres and micrometres, and on five real BOLD runs.
- `tests/oracle.rs` now also compares the header bytes.
- The two former expected divergences pass.

### Parity: records from clean commit 15b7d73, standard tier
- **Replica:** 245 cases. 225 of 225 compared are bit-identical to AFNI (Fourier 101 of 101). 18 are rejected by both, and the only expected divergences are the 2 old ones (AFNI's own output format, `-voxshift`). The Python API gives the command line's output in 191 of 191 cases, and 146 of 146 from memory.
- **Original:** unchanged where compared (222 pass, 162 bit-identical).
- **New cases:** four, so 245 instead of 241: `defects/oblique-metre`, `defects/tiny-offsets-copy`, `geometry/slightly-oblique`, `errors/prefix-with-blank`. All four are expected divergences for the original.

### Replica default against original default
The new check `D13 defaults: replica = original` covers 20 cases. 15 are bit-identical, 3 are within the original's thresholds (Fourier last bits), and none disagree. There are two known differences, both where the original doesn't follow AFNI:
- **`geometry/slightly-oblique`:** the replica writes the plumb matrix, as AFNI does; the original keeps the input's (sform entries 4e-4 apart). The replica is right.
- **`defects/tiny-offsets-copy`:** the replica derives "no slice code, duration 0", which is AFNI's intent once AFNI-08 is fixed. The original copies the input header's code 1 and 0.5 ms. Both agree on `slice_end` = nz−1. The replica follows AFNI's design; this case is contrived.

### For the original (not edited; needs spec additions to `specs/3dTshift.md` §6)
Its `reproduce` mode does not give AFNI's result for:
- AFNI-07 on oblique grids: it converts the linear part, where AFNI keeps the whole sform in metres.
- AFNI-08 (no switch).
- AFNI-11 and AFNI-12 (no switches).
- AFNI-16.

One minor point: in fix mode the original names no AFNI-03 when code-2 times exceed the TR and the output becomes a copy, although that copy differs from AFNI's output.

### Golden
- **Existing 8 cases:** re-recorded from c46c47a. Every header changed (`regular = 'r'`, signed zeros). Only the `-no_detrend` default's data changed.
- **7 new cases:** three defaults and four `*.reproduce` cases, covering AFNI-01/02/03/05/07/08. Each also checks which defects it names.
- `3dTshift.heptic.uint8.no_detrend.reproduce` has the data checksum the default case had before.
- The Python golden suite is unchanged.

### Docs
Updated: the register (AFNI-* "larmorx" lines, the gap legend and counts, AFNI-16), the `afni-tshift.md` findings, `docs/api/afni-tshift.md`, `docs/api/upstream-defects.md`, `crates-gpl/README.md`, `PROVENANCE.md`, `golden.md`, the porting guide (the defaults check), the overview numbers and the decision log. I did not touch `CLAUDE.md`, the original or the spec.

### Commits
- c46c47a: code, tests and harness
- ddafa7a: golden checksums
- 15b7d73: docs
- e8bb4c8: validation records
- 8f2d8ec: decision-log fix
- bf570e0: merge of `main`

All are authored by karellopez with the no-reply email, with no AI attribution.
