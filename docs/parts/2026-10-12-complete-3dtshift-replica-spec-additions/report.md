# Final report: Complete 3dTshift: replica + spec additions

Agent `a5932ea4ae9ed7aa0`, 2026-10-12. The agent's report to the orchestrator, verbatim.

## Completing 3dTshift, part 1 (replica author / spec writer): done

The work is in four commits on branch `worktree-agent-a5932ea4ae9ed7aa0`. All gates passed before each commit. Nothing is pushed, and `main` had not moved, so no merge was needed.

| Commit | What |
|---|---|
| `633aba9` | The replica's AFNI-16 default; `larmorx_core::defects::AFNI_16`; two GPL golden cases; parity case `header-timing/code2-beyond-tr` |
| `ef3813e` | The replica copies AFNI's second name check (AFNI-12); five more parity cases |
| `966bd98` | Spec additions, register, findings, API pages, GPL README, golden docs, decision-log line; AFNI-07 notice made the same in both modes |
| `b3589fb` | Validation records from `966bd98` (clean, standard tier); GPL golden re-recorded from it |

### Task 1: the replica (`crates-gpl/`)

**AFNI-16, the new default.**
- AFNI writes some nearly plumb grids as exactly plumb. By default the replica now keeps the input's matrix whenever that would discard a tilt (`axes::snaps`).
- It writes such a matrix the way AFNI writes an oblique one: the sform as read, its quaternion, `qoffset` from the matrix's offsets, `pixdim` from the steps.
- It names AFNI-16 in both modes.
- With `LARMORX_UPSTREAM_DEFECTS=reproduce` it writes AFNI's plumb matrix. A new oracle test (`tests/oracle.rs`) confirms the header is AFNI's byte for byte, except `vox_offset`.
- An exactly plumb matrix is written as before in both modes, so no existing output changed.

**AFNI-06:** no code change. The register entry now says class f, kept, help text is the error.

**Two other fixes I found while checking the replica:**
- **AFNI-12, second check.** AFNI checks the output name again when it writes the file. Names that pass the first check only through an exception for names starting with `3dcalc` (e.g. `3dcalc a b.nii`), or whose directory starts with `-`, are not written and AFNI exits 0. The replica wrote them; `reproduce` now matches AFNI, and the default names AFNI-12.
- **AFNI-07 notice.** It is now raised the same way in both modes. Before, a nearly plumb metre grid at the origin got the notice in `fix` but not in `reproduce`.
- The AFNI-03 notice now says when the corrected code-2 slice times exceed the TR and the output becomes a copy.

**Parity, standard tier, against AFNI 25.2.09, from `966bd98`, 251 cases each:**
- **Replica, `reproduce`:** 231 of 231 compared cases bit-identical (Fourier 104 of 104); 18 rejected by both; 2 expected divergences; 0 failures.
- **Replica, default (fix pass):** AFNI-16 is named in its 3 cases, AFNI-03 in 2, AFNI-07 in 5. Each defect is named exactly where the output changed.
- **Clean-room original:** unchanged where compared (222 pass, 162 bit-identical). It has 12 expected divergences; the 6 new ones are behaviours the spec now describes.
- **Six new parity cases:** `geometry/slightly-oblique-qform`, `geometry/slightly-oblique-metre`, `defects/oblique-metre-zero-origin`, `copy/short-slice-spacing`, `copy/alt-two-slices`, `header-timing/code2-beyond-tr`.
- The smoke tier also passes: `tests/parity` for both 3dTshift suites, 466 passed.

**The defaults check ("D13 defaults: replica = original"):**
- It now agrees on `geometry/slightly-oblique`.
- Over all 24 cases where a correction applies: 16 bit-identical, 7 within the original's thresholds, none disagreeing.
- One known difference remains: AFNI-08's copy (`defects/tiny-offsets-copy`), which the original does not follow yet.

**Golden:**
- Two new replica cases: `3dTshift.linear.float32-nearly-plumb` and `.reproduce`.
- The `.reproduce` header checksum equals the exactly plumb input's (`d0dbbb…`). Its data checksum equals `3dTshift.linear.float32`'s.
- No existing checksum changed. `golden.tsv` is recorded from `966bd98`.

### Task 2: spec additions (`specs/3dTshift.md`, behaviour terms only)

New §6.1 to §6.5, plus updates to the top paragraph, §3.4, the §6 bullets, §7 and §9. There is no transcribed AFNI code and no AFNI-internal names. The quaternion method points to nifti_clib's public-domain `nifti_mat44_to_quatern`, which ITK 5.4.5 bundles. Every statement is marked (observed) and was checked against `oracles/afni-25.2.09/bin/3dTshift`.

- **§6.1 Geometry (AFNI-07, AFNI-16).**
  - Covers: the matrix AFNI takes (qform by the NIfTI formula; nibabel reads near-180° quaternions differently), the steps, the exact single-precision plumb test, and the cardinal matrix with its signed zeros. It gives every header field: `srow`, codes, quaternion and `qfac`, `qoffset`, `pixdim`, `xyzt_units`. It also covers AFNI-07 on oblique grids, the default, and when each defect is named.
  - Confirmed: 162 inputs byte for byte (six orientations; plumb, nearly plumb and oblique; sform, qform and both; mm, m, µm). The plumb rule predicted AFNI on 240 random matrices and on 16 matrices at the float32 boundary.
  - **Correction to the register:** AFNI's effective threshold is not "about 0.014°". It runs from 0.0099° to 0.0242° depending on the voxel size (0.0242° for 2 mm, 0.0132° for 3 mm).
- **§6.2 Slice fields of a copy (AFNI-08).**
  - Covers: the trigger, and the full rule AFNI uses to derive `slice_code`, `slice_start`, `slice_end` and `slice_duration` from the kept times, with worked examples.
  - Confirmed: the rule gave AFNI's fields on 150 random header timings, 30 of them AFNI-08.
  - New register entry **AFNI-17** (class d, imprecision): with slices less than 3 ms apart, the time-0 slice can drop out of the range.
- **§6.3 Output names (AFNI-11, AFNI-12).** Exact messages and exit codes, when AFNI checks, exact-name conflicts only, the refused character set (every ASCII character tested), the `3dcalc` exception and the second check.
- **§6.4 AFNI-03.** What AFNI writes for header slice code 2, and the corrected copy when the times exceed the TR; the notice is due in both modes.
- **§6.5** A table of `reproduce`, `fix` and when each defect is named.
- **§7** AFNI accepts global options anywhere on the command line (`-overwrite`, `-Dname=value`); both larmorx implementations reject them.

### Docs updated
- Register: AFNI-06 and AFNI-16 entries, AFNI-12, new AFNI-17, gap lines, counts (125 entries; c 55 with 14 questionable; d 18; f 7; gap column 5 original, 18 decide).
- `docs/findings/afni-tshift.md` (restricted, with AFNI file:line citations).
- `docs/api/afni-tshift.md`, `docs/api/upstream-defects.md`, `crates-gpl/README.md`, the replica's `PROVENANCE.md`, `docs/validation/golden.md`, the replica case counts in `docs/overview.md` and `docs/porting-guide.md`, and one decision-log entry.

### Open items
- **Next task (clean-room implementer):** the original still keeps the input's matrix in `reproduce`. It also does not yet snap (AFNI-16), keep oblique metre sforms in metres (AFNI-07), derive copy slice fields (AFNI-08/-17), switch for AFNI-11/-12, or raise the AFNI-03 notice when the output becomes a copy. All of these are expected divergences in its record and are described in spec §6.1–6.5.
- **Global options:** AFNI's `-overwrite` and `-Dname=value` are unknown options in both implementations. This is recorded as a known difference, not a defect; you may want to decide whether larmorx should accept `-overwrite`.
- I did not update `CLAUDE.md`'s status or queue.
