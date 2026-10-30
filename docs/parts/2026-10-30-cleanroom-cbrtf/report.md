## Report: the clean-room `cbrtf` with glibc 2.35's bits (GLIBC-05) and the memory test (2026-10-30)

Branch `worktree-agent-ae4a907e734f6ead2`, 8 commits from 2401b1c to 1eac34b (d2b8c21 the function
and its verifier; 733d45e the verifier's result; 9ec1a7e the clean-room 3dUnifize; 930a42f the
memory test, golden case, docs, PROVENANCE; 8d079f0 the record; 9383f27 the GLIBC-05 cases left out
of the replica suite; 657da26 golden; 1eac34b the generated row). The handoff is `handoff.md` here.

### 1. `larmorx_core::math::cbrtf_glibc`
Written only from `specs/cbrtf.md`, plain IEEE operations, no FMA
(`crates/larmorx-core/src/math/cbrtf_glibc.rs`, a clean-room note in its header and PROVENANCE).
Proved by a Rust verifier calling the system's `cbrtf` through `dlopen` as a black box
(`validation/investigations/cleanroom-cbrtf/`): 4,294,967,296 of 4,294,967,296 inputs identical bit
for bit; all 16,777,214 NaN inputs identical (quiet bit set, sign and payload kept); 4096 of 4096
block hashes equal the spec writer's. glibc is not correctly rounded on 453,492,162 inputs (10.6 %).
22 s on 22 threads. 3.1 ns per call (glibc's 7.1 through a pointer, CORE-MATH's 3.7). Unit tests
in `cargo test` on every platform. The spec's §7.2 whole-sequence number does not come out of
glibc's results (`335d1fb5fb714c95` under §7.1's run): for the spec writer.

### 2. The clean-room 3dUnifize `-EPI`
The radius uses `math::cbrtf` by default and `cbrtf_glibc` with `reproduce`. The notice
(`epi_cbrtf_notice`) names GLIBC-05 in both modes only where glibc's last bit changes the ball of
neighbours within the grid's reach or the radius printed with one decimal, with both radii;
skipped where AFNI-75's guard refuses. Parity cases `glibc-05/epi-*` against AFNI 26.3.05 (ball,
ball-one, printed: GLIBC-05 named; same-result, ordinary: not named): `reproduce` gives AFNI's
bytes and messages in all five; the default equals AFNI with CORE-MATH preloaded; in the ball case
the default differs from `reproduce` in 6,321 of 27,648 values (max 1.95). Of the voxel sizes with
two decimals from 0.5 to 4 mm, 9,467 give radii one last bit apart, none changes the ball or the
printed radius on a 24 × 24 × 16 grid. Record (standard, from the clean 930a42f): 129 cases, 108
pass, 19 rejected by both, 2 expected, 0 failures; 44 of 99 compared cases bit-identical;
compare_records: only the 5 new cases. Golden: `cli.afni.3dUnifize.glibc05` (158 cases, 926
outputs; every other checksum unchanged). The replica's suite leaves the GLIBC-05 cases out until a
replica author wires `cbrtf_glibc` into it.

### 3. The memory test
A call through `python -m larmorx` is allowed 1.10 × the original's peak plus the peak of
`python -m larmorx --version` measured the same way (13.0 to 14.0 MiB here); the standalone binary
keeps the 8 MiB margin. 30 of 30 pass; 3dAutomask's strict expected failure removed (18.7 to 19.3
MiB against AFNI's 7.4 to 7.9, allowed 21.6 to 22.7).

### Gates
Rust (three workspaces), Python 633 passed, ruff, SPDX, `pytest tests/parity` 2,595 passed,
`tables --check`.

### Clean-room statement (summary; the full list in the agent's report)
Opened: the brief, CLAUDE.md, ORCHESTRATOR-STATE.md (loaded automatically; it holds no
source-derived detail), `specs/cbrtf.md`, `specs/3dUnifize.md` by line range, the porting guide's §1
and clean-room section, the orchestration scripts, `larmorx-core`'s math and defects, the clean-room
`larmorx-afni` (unifize), the block-hash JSON only, the harness mechanics, the register's lines 1-60
and summary table by line range (searches limited to that range), the API pages, the clean-room
findings, the record, golden.md's head, next-steps' two follow-ups, the tests. Listed directory names
only: `crates-gpl/`, `crates-nc/`, `crates/larmorx-afni-replica/tools/`. Never opened glibc's or
AFNI's source, any replica crate's files, the restricted findings, `docs/parts/`, the rest of
specs-memory, version-* and afni-move, `triage.toml`, the replica authors' permanent cases. Ran as
black boxes: the system's `cbrtf` (dlopen), AFNI 26.3.05 `3dUnifize` (plain and with CORE-MATH
preloaded), the replica through the memory test and the parity smoke tier (and once by accident in
`auto` mode, one warning line seen).

### At the merge (orchestrator)
The handoff integrated: the register's GLIBC-05 row and history, the public defect page, next-steps
(the cbrtf follow-up: the clean-room half done, the replica's half to do; the memory test done),
golden.md. The memory allowance follows the brief's reading (1.10 × the original + the entry
point's start-up peak; the binary keeps 8 MiB), which is the user's decision ("count this
separately").
