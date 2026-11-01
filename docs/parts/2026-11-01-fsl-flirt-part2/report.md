## FLIRT replica, part 2: report

I stopped at a clean boundary at about 20:35 (the start was 16:56). The worktree is `<workspace>/larmorx/.claude/worktrees/agent-a947689786d5ba4b0`, its tree is clean, and the branch has 20 commits from 301fab7 to ed7128c. The handoff is in `<scratch>/HANDOFF.md` and the notes are in `<scratch>/NOTES.md`, where `<scratch>` = `<scratch>`.

**One decision differs from the brief: FSL-75 is not refused in both modes.** The short trial found an ordinary image where FSL survives the heap overflow and writes its matrix. That image is 25×30×27 voxels at 0.5×1.2×1 mm, and its pyramid reaches a 3×3×3 level. A refusal would have failed where FLIRT works. The values FSL keeps are well defined; only the writes past its buffer are not. So I decided it by rule A:
- By default the replica takes the background value over the whole border.
- With `reproduce` it takes the values that fit FSL's buffer, which gives FSL's matrix.
- FSL-75 is named in both modes.
- A 1×1×1 image is still refused in both modes (rule B).

Please confirm this, or say if you want the refusal back.

### What was done, in the brief's order
1. **Site page.** `content/tools/fsl-flirt.md` and the evidence module `fsl_flirt`, in site clone commit 438c4fa on branch `fsl-flirt-page`. The `evidence-blobs` branch is at 2a38d50 (592 blobs) and must be pushed before main.
   - Re-exported from the new record. The page data grew from 0.19 to 0.23 MB; the images are unchanged (39 blobs, 29.97 MB), so the site shows nothing less.
   - Build: 0 problems. Number check 124/124, viewer check 18/18.
   - Screenshots at 1440 and 390, light and dark, are in `<scratch>/shots2`; the earlier ones are in `<scratch>/shots`.
2. **The first trial's findings, all settled.**
   - Heap corruption: traced with a gdb watchpoint to newimage's background value (`calc_bval`), whose buffer is too small for volumes 3 voxels wide. This is FSL-75, decided as above.
   - The 66 differing matrices: FLIRT computes `1/basescale` in single precision; the replica now does too.
   - Fix-pass changes without a notice: FSL-14 and FSL-09 are now named.
   - The extra "Erroneous dof" message had the same base-scale cause.
   - Runs above 3 GiB: FSL itself peaks between 8 GiB and over 39 GiB on them, or runs longer than 30 minutes.
   - Measuring them exposed real memory excess in the replica, now fixed: the default call went from 262 to 187 MiB (FSL: 192), with `-inweight` from 503 to 310 MiB (FSL: 374).
   - FSL-22 (the order of NaN values in the background) is now handled in FLIRT as in mcflirt.
   - Re-checked with `recheck_findings.py`: every finding that fits the trial's memory budget agrees, in both its shrunk and drawn forms.
3. **What part 1 refused.** `-anglerep quaternion` is done. A quaternion message that was printed one time too few is fixed. `alignpaxes`, the general pseudo-inverse and `-debug`'s BBR volume are **not done**.
4. **FSL-67, decided by rule C (kept in both modes).** Computing the normals in millimetres moves fMRIPrep's three BBR results by 0.016, 0 and 0.025 mm on average over the brain. On an anisotropic synthetic case both versions end the same distance from the true motion: 0.060 vs 0.063 mm on average, 0.092 mm at the 95th percentile for both. No gain.
5. **Hang cases in the suite.** FSL-71 runs FSL under a timeout; FSL-72 under an 8 GiB address-space limit, where FSL prints `std::bad_alloc` and exits 0. The FSL-75 crash cases are in the suite too.
6. **FLIRT's other programs.** Done: `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`, each with its command line, `lx.fsl` wrapper, golden case, spec section (§15) and suite cases. makerot has two new defects:
   - FSL-76 (rule A): the axis is divided by the square root of its length, so 30° about (0,0,4) gives 60°.
   - FSL-77 (rule B): an axis or centre of other than three values is read past the end of its values.
   - Probe calls identical to FSL: avscale 16 of 17 (a 5×5 matrix is refused) plus 400 of 400 random matrices; the coordinate programs 21 of 21 each; rmsdiff 40 of 40; makerot 47 of 50. The three makerot calls left use a NaN or infinite angle or axis and print `-nan` where FSL prints `nan` (part 3).
   - Not done: `applyxfm4D`, `midtrans`, `pointflirt`.

### Validation
- **Record `fsl-flirt-replica`**, standard tier, made from clean commit b92c7ed: 217 cases, 152 of 152 compared cases bit-identical, 61 rejected by both, 4 expected divergences (FSL-71, FSL-72 and two FSL-75 crash cases). `compare_records.py` against part 1: all 141 earlier cases are unchanged in status and details.
- **mcflirt and BET standard tiers**, re-run because the core changed: 236 of 236 and 178 of 178 cases identical to their committed records.
- **Smoke tier**, run once from 3f6fc31: 2633 passed.
- **Golden checksums**: `record_golden.py` reused the smoke marker and no value changed. `larmorx-nc` gained 7 cases (now 59 cases, 305 outputs); every other checksum is unchanged.
- **Memory tests**: the FLIRT memory test now has 6 cases, all passing: avscale 127 vs 129 MiB, rmsdiff 147 vs 190, omat-only 187 vs 192, inweight 310 vs 374. No new program runs threads, so no thread test was added.
- **Trials**, under the `fsl-flirt-programs` descriptor (new paths only):
  - 30 minutes: 12,376 cases, 12,309 agree. The replica binary was rebuilt three times during it (memory-only fixes and one message fix), so every finding was re-checked with the final build.
  - 6 minutes with makerot added: 3,274 cases, 3,222 agree. The 47 FSL-76 notices without a visible change are fixed (named only where the matrix changes). The 2 differences were an over-strict suite check, now fixed.
  - The quaternion runs over 3 GiB also need more than 20 GiB in FSL.
- **Permanent cases added**: `background-level-overflow-survives`, `background-nan-order`.

### What was run
- **Original programs:** the records ran 13 new FSL runs and replayed 204 from the cache. FSL was also run again on purpose to measure its peak memory. The probe scripts (`validation/probes/fsl-flirt-programs`) run FSL directly, outside the cache; the makerot probes ran about four times.
- **Records:** FLIRT twice (the second only to put every program in the header; both runs identical); mcflirt and BET once each.
- **Smoke tier:** once.
- **Memory boxes:** never more than 80 GiB together.

### Gates
Rust in all three workspaces (fmt, clippy -D warnings, test), Python tests (650 passed), ruff, SPDX, `fsl_source.py --check` (82 files) and `tables --check` all pass.

The README tool table and the validation index were regenerated in the branch, counts only. `test_tables` requires them in sync, so ed7128c touches those two shared files; please run `tables` again after you integrate the tools.toml text.

### Handoff and follow-ups
`HANDOFF.md` holds the exact text for:
- the register: FSL-67 decided; new rows FSL-75, FSL-76, FSL-77; additions to FSL-09, FSL-14 and FSL-22;
- `docs/api/upstream-defects.md`;
- the tools.toml blocks (I checked that they render);
- `golden.md`, next-steps item 7 with the part 3 list, and the FSL row in CLAUDE.md;
- `triage.toml`: the four open entries settled, plus the new trials' entries;
- the site's navigation entry, status regeneration, Roadmap row and Completed row.

Follow-ups: mcflirt's FSL-75 notice and correction (its replica now silently takes FSL's values that fit), and adding `fsl-flirt-programs` to `docs/validation/difftest.md`.

### Left for part 3
- `alignpaxes` (LAPACK `dsyevd`).
- The general parameter-subset pseudo-inverse, `midtrans` and `pointflirt` (all need LAPACK `dgesdd`).
- `applyxfm4D`.
- `-debug`'s BBR volume.
- The coordinate programs' `-warp` (after FNIRT).
- makerot's NaN signs.
- mcflirt's FSL-75 notice and correction.
- Then the clean-room original from `specs/flirt.md`.

New tools behind findings are committed under `validation/investigations/fsl-flirt` (`rsstrace.py`, `gdb_allocations.py`, and `finding_repro.py` and `memory_findings.py` extended) and `validation/probes/fsl-flirt-programs`. All tools now use FSL 6.0.7.23 by default, not `$FSLDIR`, which on this machine points to 6.0.7.17.

---

**Merge note (orchestrator, 2026-11-01).** FSL-75's rule A accepted: it is what D13 prescribes for a
case whose input makes sense (the defined part reproduced with `reproduce`, the corrected value by
default); the single voxel stays refused in both modes (rule B). At the merge the texts that
clean-room authors may read were made behaviour-only (`larmorx_core::defects`' FSL-22, FSL-75 and
FSL-76 descriptions, `docs/api/fsl-flirt.md`'s avscale row, the next-steps and `triage.toml` entries
without FSL's routine names). Smart testing checked: 13 new FSL runs, 204 served by the cache, the
smoke tier once, the findings re-checked through the suite's runner (cached), two short trials of the
new paths only; the record was made twice (the second for its header only) and the program probes ran
FSL outside the cache (seconds each).
