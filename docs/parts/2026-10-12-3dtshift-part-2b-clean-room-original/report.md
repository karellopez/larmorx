# Final report: 3dTshift part 2b: clean-room original

Agent `ad98aa6636037d334`, 2026-10-12. The agent's report to the orchestrator, verbatim.

## Completing 3dTshift, part 2b: the clean-room original now matches AFNI wherever the spec describes it

The original in `crates/larmorx-afni` now implements spec §6.1–6.5 and §7.1. Its parity record went from 71 expected divergences to 3, the same 3 the replica has, and the 60 global/environment cases now have AFNI's exit status and messages byte for byte. All gates pass on HEAD `f0a4a55`. Nothing is pushed or merged into main.

### Parity against unmodified AFNI 25.2.09
Standard tier, 311 cases, records from clean commit `4c6cd3a`, in `docs/validation/afni-tshift.{md,json}`:

| | Before | Now |
|---|---|---|
| Pass | 223 | 279 |
| Rejected by both | 17 | 29 |
| Expected divergences | 71 | 3 |
| Failures | 0 | 0 |
| Bit-identical data | 163 of 223 | 176 of 279 |

- **The 3 left:** AFNI's own output format and `-voxshift` (both deliberate), and AFNI-20 (AFNI writes a one-row dataset as a text file with a random identifier). The other 69 now pass.
- **Bit-identical breakdown:** Lagrange, weighted-sinc and copies 129 of 132; Fourier 47 of 147. The rest differ only in the documented last bits, all within thresholds.
- **Headers:** header fields are identical in every compared case and the geometry agrees to 1e-6 mm. The one case compared as a whole file (`AFNI_NIFTI_NOEXT`) has the header byte for byte; its data are compared with the thresholds, because Fourier differs in the last bits.
- **Messages:** all 60 global/environment cases are identical in exit status, stdout and stderr, AFNI's colour codes in `reproduce` included.
- **Python API:** 194 of 194 cases from file and 149 of 149 from memory give the command line's output. With `afni_settings`, 28 of 28 do, while the process environment holds AFNI variables that would change the output if read.

**Defaults check (replica record):** the replica is unchanged at 279 of 279 bit-identical. "D13 defaults: replica = original" covers 24 cases: 17 bit-identical, 7 within thresholds (Fourier last bits), 0 disagreeing. AFNI-08's copy is now bit-identical between the two.

### What was implemented
- **Geometry (§6.1):**
  - AFNI's float32 matrix and steps, its single-precision plumb test, and the cardinal matrix with AFNI's signed zeros.
  - The quaternion uses the NIfTI reference library's method in double, taken from larmorx's own Apache port in larmorx-io.
  - The x87 extended-precision steps (the quotients, `1+r11+r22+r33`, the qform's matrix) are emulated with integers in `larmorx_afni::x87`. It matched GCC's `long double` on 6 million random operations, 1,224 of which differ from plain double.
  - With the switch: AFNI-07 on oblique grids and AFNI-16.
- **§6.2–6.4:** the slice fields of a copy, derived from its kept times (AFNI-08 and AFNI-17); the output-name rules at both of AFNI's checks (AFNI-11, AFNI-12); AFNI-03's notice when the corrected times turn the output into a copy.
- **§7.1:**
  - A table of settings read once from the environment, which is never changed; names match whatever their case on Windows. `-D` wins over the environment.
  - `-overwrite`, `-Dname=value` (AFNI-22), `-ok_1D_text` and `-skip_afnirc` are taken out wherever they stand.
  - Every variable in the spec's table has its effect and AFNI's message (AFNI-18, -19, -20, -21, -23). Any other `-D` gets a "has no effect … (larmorx)" warning.
  - Outputs are written to a temporary file and renamed over the target, retried on Windows for about 2 s.
  - Messages are AFNI's, in AFNI's order. larmorx's own lines use the replica's wording.
- **Python:** the `NotImplementedError` is gone. `lx.afni.tshift(..., implementation="original", afni_settings=...)` applies the six variables and still never reads the environment.

### Spec gaps settled by black-box runs (added to the spec, marked observed)
- The messages of a run: the order of checks (`-slice`, then `-ignore`, both before the TR), what goes to stdout, `-verbose`'s "Wrote output" (labelled after a copy, plain after a shift, naming `out_AA1` even when nothing is written), and numbers printed as `%g`.
- AFNI's lenient 1D parsing: `2x` is 2, `*` and `e5` are 0, colons separate values, and the error shows the row with its comment, cut to 60 characters plus ` ...`. Inline `1D:` strings are strict.
- AFNI-18's list splits at blanks and at `~`, but only a blank makes a name a list.
- `AFNI_NIFTI_VIEW` must be the whole word (any case).
- The template-space codes: this oracle gives TT_N27 → 3 and MNI_152 → 4. The spec had said this oracle finds no table (TT_N27 → 5); the observed values are added.
- The 0.8 mm plumb boundary: the spec's 0.000172634 is the first oblique value; the last plumb one is 0.000172633489, which the original's test gives too.

Two AFNI behaviours I found that are not in the register, recorded in the spec for the spec writer: AFNI crashes on `-ignore` with no value (a segfault), and it exits 0 after failing to write an output into a missing directory. larmorx exits 1 in both modes for both. Each needs a register entry, which goes in the restricted section, so I did not write them.

### Golden and cross-platform tests
- **Recorded:** with `scripts/record_golden.py` from clean commit `d18cdef`, after the parity smoke tier passed: 102 cases, 416 outputs.
- **New cases (5):** the metre grid in both modes, a copy keeping slice times in both modes, `AFNI_NIFTI_SCALE=NO` along three paths, global options with NIfTI-2 output and renaming, and `afni_settings` in Python.
- **Changed checksums:** the four existing 3dTshift cases changed only in header checksums (the original now writes AFNI's header bytes) and in the CLI's stdout, which gains the `++ updating time offset` line. Their data checksums are unchanged.
- **Six-platform test:** `crates/larmorx-cli/tests/afni_globals.rs` covers `-overwrite`, `-D` and renaming, on paths with blanks and non-ASCII characters. A Windows-only test holds the output open for 0.5 s to exercise the rename retry. Neither Windows behaviour could be run here; only CI will check them.

### Docs updated
`docs/api/afni-tshift.md`, `docs/api/upstream-defects.md`, the register's Summary rows and gap lines (nothing restricted: every diff hunk is before the restricted heading, and the "original" gap count is now 0), `crates/larmorx-afni/PROVENANCE.md`, the decision log, `docs/validation/golden.md`, the porting guide, `docs/overview.md`, README and the worktree's CLAUDE.md.

### Clean-room statement
- **Did not open:** AFNI source (`tags/AFNI-25.2.09`, `afni/`, `oracles/afni-25.2.09/build`), `docs/findings/afni-tshift.md`, the content of the restricted register sections, any `crates-gpl/` source, FSL source, `reference_src/`, `crates-nc/`, or other tasks' `/tmp` files. I also did not open ITK's `nifti1_io.c`, which the spec cites; I used larmorx's own Apache port in larmorx-io instead.
- **Opened:** the spec, the porting guide, the API pages, the register's intro, Summary and D13 gaps, the decision log, records, larmorx's own Apache code, the harness and the tests.
- **Ran as black boxes:** the AFNI binary (never a GUI or browser option), the replica program, and GCC `long double` through my own C program. In `crates-gpl/` I ran cargo build, test, clippy and fmt only.
- **Two disclosures:**
  - A heading list of the register showed the restricted sections' titles, not their content. My edit script loaded the whole file but searched and changed only the part before the restricted heading.
  - I started a filesystem-wide `find` for AFNI's `AFNI_atlas_spaces.niml` and stopped it; none of its results were displayed.

### Commits (karellopez, no AI attribution)
- `91f44e9`: WIP checkpoint with the code.
- `3844163`: spec and docs.
- `3de86d7`: merge of main (main is still at `0c82f70`).
- `d18cdef`: Windows path separators in output-name messages; CLAUDE.md.
- `4c6cd3a`: golden checksums.
- `f0a4a55`: records.

I did not use `maturin develop`, because it would have repointed the shared venv to this worktree while your verification run was going. Instead I built the release wheel into the scratchpad and copied the git-ignored `_core.abi3.so` into the worktree's `python/larmorx/`. The merge in the main checkout will need its own `maturin develop`.
