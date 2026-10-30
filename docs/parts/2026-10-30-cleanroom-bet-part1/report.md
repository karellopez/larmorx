## Report: BET's clean-room original, part 1 (2026-10-30, 17:18 to 20:12)

`bet2` complete; the `bet` script's default and its `-R`, `-F`, `-Z` variations; betsurf not
started (part 2, with the site page). Branch `worktree-agent-a8aa142043f423b7f`, 16 commits from
bed7e04 (4221fc9 to 677e371). The handoff is `handoff.md` here; `notes-for-part2.md` is the
agent's NOTES.md for part 2.

### What was built
- `crates/larmorx-fsl` (Apache-2.0, PROVENANCE): `bet2` with every option, message, exit status
  and failure case of `specs/bet.md` §4, §7, §9; `reproduce` switches for FSL-40, -42, -43, -56,
  -60, -61, -62. The `bet` script: default, `-R` (FSL-63), `-F`, `-Z` (FSL-45), FSL-44, FSL-46;
  `-S`, `-B`, `-A`, `-A2` refused as pending (FLIRT, FAST).
- `larmorx fsl bet2|bet`; `_core.fsl_run_original` with stubs; `lx.fsl.bet(implementation=
  "original")` (`auto`: the replica when installed, else the original). The API page's original
  section; `docs/findings/fsl-bet-original.md` (every black-box observation); `docs/benchmarks/
  fsl-bet.md`; `validation/investigations/cleanroom-bet/`.

### Record (`docs/validation/fsl-bet.md`, standard tier, from the clean 2c5f669)
142 cases: 108 of 111 compared within thresholds; 24 rejected by both; 3 D13 crash-fill cases; 4
pending. fMRIPrep's 29 BET calls bit-identical to FSL and to fMRIPrep's own outputs. 202 of 217
brains, masks and outlines bit-identical, 9 more within thresholds; meshes FSL's to all 6 decimals;
headers FSL's but `descrip`, the qform quaternion allowed 2^-23 (spec §8 leaves FSL's precision
path open). The 3 failures (older-75y, 7t, tumour; Dice 0.980 to 0.995) rerun after a
self-intersecting first pass, where FSL's rerun differs from the spec's rule. Skull (`-s`) reported
only (Dice about 0.9: spec §4.7 leaves the skull search's details open).

### Checks
Golden: 6 new cases (from dbb7dca, the parity smoke tier passing, 2,607 tests). Memory within the
rule (bet2 71 MiB against FSL's 72; `bet -R` 117 against 103 plus the interpreter's 13; `bet -F` 170
against fslmaths' 217). Threads: identical at any count; 3 new programs in `test_thread_pools`.
Speed: fMRIPrep's call (ds000005 BOLD reference) FSL 0.66 s, replica 0.39, original 0.24 (1
thread) and 0.06 (24); `bet2 -m` on real T1w images FSL 1.26 to 3.08 s, replica 0.72 to 1.78,
original 0.42 to 1.19 and 0.16 to 0.53: 2.6 to 3 times FSL's speed on one thread, 6 to 11 times on
24. Random-testing trial (30 min, 718 cases): the replica agreed in every case it finished; the
original 334 of 573 (betsurf's 143 skipped), six fixes from its differences, 164 of the 239 agreeing
again with the final build, 75 left (messages 29, meshes 15, brains and masks 25, outlines 5, a
header 1): small or degenerate random heads, reruns, the `-e` test; no FSL defect (FSL-67, FSL-68
unused). Gates: the three Rust workspaces, Python, `tables --check`; one validation test
(`test_an_unknown_thread_count…`) failed once when run 8-way parallel and passes alone.

### Six questions for the spec writer
The rerun rule (§4.5), the skull search (§4.7), the quaternion's precision path (§8), the `-e`
test (§4.5), the order of the neighbour sums (§8), and moving the black-box observations into the
spec (marked observed).

### Clean-room statement (summary)
Opened: the brief, CLAUDE.md and ORCHESTRATOR-STATE (imported), `specs/bet.md` in full, the porting
guide's §1, speed section and §11, architecture.md, the API page, a benchmark's head as a model,
larmorx's own code (larmorx-mri, larmorx-cli, larmorx-py, larmorx-core, larmorx-io including its ITK
NIfTI writer's quaternion port, larmorx-afni's cfmt and x87 heads), the Python package, the BET
harness's mechanics and other harness files, the tests, the orchestration scripts. Ran as black
boxes: FSL 6.0.7.23's `bet2` and `bet`, the `larmorx-nc` program (benchmark only), the gates'
builds of crates-nc and crates-gpl. Never opened FSL's source, the replica crates, the restricted
findings, the register, `docs/parts/`, the replica parts' investigations, `triage.toml`, the replica
authors' permanent cases, `fsl_versions.py`. One incidental search over `crates/` (for
`allow(clippy` and debug strings) covered the replica crate's files and printed no line from them.

### At the merge (orchestrator)
The handoff integrated: `validation/tools.toml` (fsl-bet's tool lines and a new `fsl-bet` record),
the generated tables, next-steps item 7 (part 1 done, part 2 and the six spec questions),
golden.md, the register's FSL-62 and FSL-56 summary rows (observed); triage: five open entries for
the trial's remaining differences by failed check.
