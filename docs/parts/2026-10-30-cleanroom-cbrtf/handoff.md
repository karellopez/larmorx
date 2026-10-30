# HANDOFF: the clean-room `cbrtf` with glibc 2.35's bits (GLIBC-05), and the memory test's allowance

Branch: the worktree `agent-ae4a907e734f6ead2` (from main `2401b1c`). Clean-room implementer
(never opened glibc's or AFNI's source, the replica crates, the restricted findings, `docs/parts/`
or the rest of `validation/investigations/specs-memory/`). The clean-room statement is in the
report.

Commits (from `2401b1c`): `d2b8c21` the function and its verifier; `733d45e` the verifier's
result; `9ec1a7e` the clean-room 3dUnifize (GLIBC-05, parity cases, Python test); `930a42f` the
memory test, the golden case, the API page, findings, PROVENANCE; `8d079f0` the afni-unifize
record from `930a42f`; `9383f27` GLIBC-05's cases left out of the replica suite; `657da26` golden
checksums from `9383f27`; `1eac34b` the README's generated row.

## 1. The register (`docs/findings/upstream-defects.md`): the summary row

Insert after GLIBC-04's row (behaviour only):

```
| GLIBC-05 | glibc 2.35 | `cbrtf` not correctly rounded (one unit in the last place on 10.6 % of float inputs): 3dUnifize `-EPI`'s radius (18.3 mm over the cube root of the voxel volume) changes its ball of neighbours or its printed radius on some voxel sizes (11.969702 mm³: radius 8 with glibc's, 7.9999995 correctly rounded) | d | none (fMRIPrep does not call `-EPI`) | correctly rounded by default (`larmorx_core::math::cbrtf`), named where glibc's last bit changes the ball within the grid or the printed radius; `reproduce` uses glibc 2.35's bits (`larmorx_core::math::cbrtf_glibc`, clean-room from `specs/cbrtf.md`, proved on all 2^32 inputs) in the clean-room original (2026-10-30); the replica keeps its warning until a replica author wires the same function | replica |
```

(Gap **replica**: corrected and switchable in the clean-room original; the replica's switch is the
replica author's half. If the register keeps its glibc entries' sections, an entry section for
GLIBC-05 with the evidence: `validation/investigations/cleanroom-cbrtf/README.md` and
`docs/findings/afni-masks-original.md`, "`-EPI`'s radius and glibc's cube root".)

## 2. `docs/api/upstream-defects.md`: a row in "What is corrected"

After AFNI-75's row:

```
| GLIBC-05 | glibc 2.35's `cbrtf` (AFNI's 3dUnifize `-EPI`) | `larmorx afni 3dUnifize -EPI`, `lx.afni.unifize(epi=True)` (the clean-room original, `implementation="original"`) | the cube root of the voxel volume is one unit in the last place from the exact one on 10.6 % of floats; on some voxel sizes that changes the radius's ball of neighbours (11.969702 mm³: 8 voxels instead of 7.9999995) or the radius printed (18.1 instead of 18.0) | the correctly rounded cube root; GLIBC-05 named with both radii where the ball within the grid or the printed radius changes; `reproduce` takes glibc 2.35's bits (a clean-room function proved on all 2^32 inputs) and gives AFNI's bytes |
```

## 3. `docs/next-steps.md`: the two follow-ups

Replace the bullet "3dUnifize `-EPI`'s `cbrtf`: decided by the user (2026-10-30), a clean-room part
to do" with:

```
- **3dUnifize `-EPI`'s `cbrtf`: decided by the user (2026-10-30); the clean-room half done the
  same day, the replica's half to do:** the default keeps the correctly rounded `cbrtf` (CORE-MATH,
  the more accurate: "if Larmorx is already most accurate then the clean room library should be
  for the reproduce mode"), and `reproduce` uses `larmorx_core::math::cbrtf_glibc`, a clean-room
  cube root with glibc 2.35's bits (written from `specs/cbrtf.md` by an implementer who never read
  glibc; proved equal to the system's `cbrtf` on all 2^32 inputs, NaN bits included, and on the
  spec writer's 4096 block hashes: `validation/investigations/cleanroom-cbrtf/`). **Done in the
  clean-room original:** `-EPI`'s radius takes it with `reproduce`; the default names GLIBC-05
  where glibc's last bit changes the ball within the grid or the printed radius (both radii in the
  notice); the parity cases `glibc-05/epi-*` (AFNI 26.3.05's bytes in `reproduce`; the default AFNI's
  with correctly rounded functions preloaded). **To do (a replica author):** the replica's
  3dUnifize takes `cbrtf_glibc` with `reproduce` instead of its last-bit warning, with the same
  GLIBC-05 notice by default (the clean-room original's words, `crates/larmorx-afni/src/unifize/mod.rs`,
  `epi_cbrtf_notice`, may be copied as text: they are larmorx's own Apache lines), the cases
  `glibc-05/epi-*` enabled for the replica in `parity/afni_unifize.py` (`suite()`'s
  `suite_cases` leaves them out for the replica today; the replica suite otherwise shares the
  original's case list, AFNI-75's guard cases included), and the spec's §7.3 run
  (`cbrtf-measure` against `cbrtf_glibc`) and §7.6. Measured incidentally on 2026-10-30 (the
  smoke tier before that filter): the replica's `reproduce` on `glibc-05/epi-ball` differs from
  AFNI in 6,321 of 27,648 values (max 1.95), as its correctly rounded cube root implies.
  **For the spec writer:** §7.2's number `c679cb92d4402e33` is not what the system's own results
  give under §7.1's hash run through all inputs (`335d1fb5fb714c95`; the verifier's README lists
  the conventions tried); the function agrees input by input, so §7.2 should state the convention
  of the 2026-10-23 program, or the number `335d1fb5fb714c95`.
```

Replace the bullet "The Python entry point's own memory: decided by the user (2026-10-30), to do"
with:

```
- **The Python entry point's own memory: decided by the user (2026-10-30), done the same day:**
  `tests/parity/test_memory_peak.py` allows a call through `python -m larmorx` 1.10 × the
  original's peak plus the peak of `python -m larmorx --version` measured the same way, in the
  same environment (13.0 to 14.0 MiB here: the interpreter and the Rust core); the standalone
  binary's calls keep the 8 MiB (`allowance`). 3dAutomask's strict expected failure is removed:
  18.7 to 19.3 MiB in two runs against AFNI's 7.4 to 7.9 and the entry point's 13.0 to 14.0
  (allowed 21.6 to 22.7). The whole file: 30 of 30 pass (2026-10-30, larmorx `9ec1a7e` plus the
  test's change, committed in `930a42f`; log `oracles/logs/cleanroom-cbrtf-memory-2026-10-30.log`).
```

## 4. `docs/validation/golden.md`

- In the table "Where they are": the main package's `157 cases, 910 outputs` becomes `158 cases, 926 outputs`.
- In "What is covered", row "Command line, AFNI, MRI and FSL" (where `cli.afni.3dUnifize.guard`
  is), count 13 → 14, and append: `; 3dUnifize -EPI on voxels whose radius glibc 2.35's cube root
  and the correctly rounded one set differently (cli.afni.3dUnifize.glibc05: 11.969702 mm³, a
  different ball, and 1 × 1 × 1.0421294 mm, a different printed radius; both modes, GLIBC-05 named
  in each) (2026-10-30)`.
- A sentence after the table: `larmorx_core::math::cbrtf_glibc` (glibc 2.35's cube root,
  clean-room, 2026-10-30) is checked on every platform by its unit tests in `cargo test`
  (`the_spec_examples`, `special_values_and_nan_bits`, `matches_glibc_on_a_stride`: the hash of
  every 4099th of the 2^32 inputs, 1,047,809 values; the ignored `matches_glibc_everywhere`: all
  inputs), the spec's golden cases (§7.5) in the form the other `larmorx_core::math` functions have.
- A note with the other re-recordings: `The main package's checksums were re-recorded on 2026-10-30 from 9383f27 (the parity smoke tier passing: 2595 tests): 1 case added, cli.afni.3dUnifize.glibc05 (16 outputs); every other checksum unchanged.`

## 5. The site: the Roadmap row

The row "A cube root with the C library's digits, for reproduce": status **clean-room half done
(2026-10-30)**; text: `larmorx_core::math::cbrtf_glibc, a clean-room cube root with glibc 2.35's
bits, proved on all 2^32 float inputs; the clean-room 3dUnifize -EPI uses it in reproduce (AFNI's
bytes) and names GLIBC-05 by default where glibc's last bit changes the result. The replica's half
waits for a replica author.`

## 6. Code that changed for the tool to work (in the branch)

- `larmorx_core::defects`: `GLIBC_05` (before `AFNI_01`, register order).
- `tests/golden/registry.py`: `cli_status` also names `GLIBC-` IDs; the case
  `cli.afni.3dUnifize.glibc05`; `golden.json` re-recorded.
- `validation/src/larmorx_validation/parity/afni_unifize.py`: the clean-room original's cases
  `glibc-05/epi-*` (`CBRTF_VOXELS`, `_cbrtf_scenarios`, `_check_cbrtf_case`; left out of the
  replica's suite by `suite_cases`); `_epi_phantom` shared with AFNI-75's cases (their inputs
  unchanged, byte for byte).

## 7. The record and the validation index

`docs/validation/afni-unifize.{md,json}` re-made from the clean commit `930a42f` (standard tier):
129 cases, 108 pass, 19 rejected by both, 2 expected divergences, 0 failures; bit-identical to
AFNI 26.3.05 in 44 of the 99 compared cases that write outputs (39 of 94 before: the 5 new cases
are all bit-identical in `reproduce`); `compare_records.py` against main's record: status differs
0, details differ 0, only the 5 new `glibc-05/*` cases. No status change (`validated`). If the
validation index or the site's status names the case count (124), it becomes 129.

## 8. Validation tables

The README's generated tool table re-made with `python -m larmorx_validation tables` (commit
`1eac34b`: the 3dUnifize row's 124 → 129 cases and 39 of 94 → 44 of 99 bit-identical; no
`tools.toml` change); `tables --check` passes. The site's `data/status.yml` is regenerated the
same way at the merge (`tables --site`), if it repeats the case count.

## 9. A note on the memory allowance's wording

`docs/next-steps.md` says "the 8 MiB margin kept for the computation"; the brief said the entry
point's peak comes "in place of the fixed 8 MiB for those calls; the binary's calls keep the 8 MiB
rule". The test follows the brief (`allowance(original, entry)`: 1.10 × original + entry peak;
`allowance(original)`: + 8 MiB for the binary). Every call of the file also passes under the
other reading (1.10 × original + entry peak + 8 MiB), which is looser; the next-steps text above is
written for the brief's reading.
