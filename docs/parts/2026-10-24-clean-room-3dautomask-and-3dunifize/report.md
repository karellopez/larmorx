# Final report: Clean-room 3dAutomask and 3dUnifize

Agent `a4282c736dbd9fe7c`, 2026-10-24. The agent's report to the orchestrator, verbatim.

## Clean-room originals of 3dAutomask and 3dUnifize: report

Both programs are built, validated and documented. Branch `worktree-agent-a4282c736dbd9fe7c` (20 commits from 30c50b0, ending at a98de73). Site branch `afni-masks-original-evidence` (one commit, cf87fef, in `<scratch>/site`). The text for every shared document is in `<scratch>/HANDOFF.md`. Nothing was pushed or merged.

**What was built** (`crates/larmorx-afni`, Apache-2.0):
- `automask`:
  - the clip level is computed from integer histograms per z-slab, which give the spec's double sum exactly;
  - the gradual clip level works by octants;
  - clusters are found by run-based union-find (the root rule is the spec's tie rule);
  - the exterior clip and the depth use breadth-first searches instead of repeated sweeps;
  - every voxel-wise pass runs in parallel and the result is the same at any thread count.
- `unifize`:
  - the ball's percentile mean is computed with a sliding window of counts per rank (block counts, the values read in ascending order) instead of a sort at every voxel; this gives the spec's float sum bit for bit;
  - median-of-7 half-size path, T2 inversion by selection, `-GM`, edge clipping, `-EPI`.
- Command lines `larmorx afni 3dAutomask` and `3dUnifize`, with every option, message, exit status and D13 switch. AFNI-41 and AFNI-48 are refused by default; AFNI-42 and AFNI-44 are kept.
- `lx.afni.automask` and `lx.afni.unifize` take `implementation="auto"|"replica"|"original"`; `.pyi` stubs added.
- CORE-MATH's `tanhf` and `cbrtf` ported to `larmorx_core::math`, bit-identical to the C on all 2^32 floats.
- Unit tests for every step; Python tests; 8 golden cases. The registry's replica-only list is now empty.

**Your spec update (60c963d) is handled** (4cc2835):
- AFNI-41 and AFNI-48 now refuse with the replica's exact messages and notices, at the same point in the run (the last `-nbhrs` decides, with or without `-NN`; `-T2` with `-EPI` is accepted; in `reproduce` the notice is printed after the outputs).
- The same comparison showed the AFNI-45 and `-EPI` AFNI-01 notices were worded differently from main's replica; I matched those too.
- Checked against the corrected replica build (`agent-af2255d42fc4f41d1`) on 24 command lines in both modes: the transcripts are identical except one line, which is outside this part. When the input does not exist, the replica's default mode appends the system error to "Can't open dataset"; the shared `crate::input` used by all the originals does not. 3dTshift has the same difference.
- Your spec lines are copied unchanged into this branch's specs.

**Provenance** (recorded in PROVENANCE.md):
- What I read: the two specs (30c50b0, plus the 60c963d lines), §6–7.1 of `specs/3dTshift.md`, AFNI's `-help`, the summary rows of the defect register, and textbook methods.
- Black-box runs of AFNI and of the replica program, for wording only; CORE-MATH's MIT source.
- **One exposure:** after both programs were complete, a grep over `docs/licensing.md` printed a few rows of the restricted table of AFNI routine names. Nothing was written or changed from it, and it is recorded in PROVENANCE.md.

**Parity**, standard tier, final records from 2a009d1:
- **3dAutomask:** 105 cases, none failing (85 pass, 19 rejected by both, 1 expected divergence).
  - 80 of the 81 compared cases that write outputs are bit-identical to AFNI.
  - The 81st is AFNI-10: fMRIPrep's call on ds005454's BOLD reference reads 3dUnifize's output, which carries AFNI's own header extension. The masks are bit-identical; the header differs in the 7th digit. AFNI run on the input without its extension writes the original's header byte for byte.
- **3dUnifize:** 97 cases, none failing (76 pass, 19 rejected by both, 2 expected divergences).
  - 32 of 72 are bit-identical.
  - The other 40 are within 1e-5 (worst 8.7e-8 of the largest value; at most 1,633 values in a T1w). In all 40, AFNI run with CORE-MATH's correctly rounded maths preloaded gives the original's bytes. The cause is glibc's `tanhf` in AFNI's squash.
- AFNI's own variability: 1 vs 6 threads, and fMRIPrep's build of AFNI, all give the oracle's bytes.
- The Python API gives the command line's outputs bit for bit (20 of 20 cases per suite).

**fMRIPrep's real calls:**
- 3dAutomask: 7 of 7 of fMRIPrep's own outputs bit-identical.
- 3dUnifize: 1 of 7 bit-identical; for the other 6, fMRIPrep's output is AFNI's oracle output bit for bit, so they carry the same `tanhf` last-bit differences.

**Informal timings** (12-CPU machine shared with another agent, load 17–22; indications only, not a benchmark):

| Run | AFNI | replica | original |
|---|---|---|---|
| 3dUnifize, fMRIPrep's call, 1 thread | 93 s | 39 s | 3.8 s |
| 3dUnifize, fMRIPrep's call, 6 threads | 22 s | 8.7 s | 0.94 s |
| 3dUnifize, T1w, 1 thread | 67 s | 35 s | 9.3 s |

- Overall the original's 3dUnifize is 4–25× faster than AFNI and 3–10× faster than the replica.
- 3dAutomask takes about 1.6 s in all three programs at one thread (mostly gzip input/output); the original takes 0.72 s at 6 threads.

**Golden checksums:** recorded from 12cad72 after the parity smoke tier passed (2231 tests, 1 h 28 min; ddba1c9). After 4cc2835, `--check` gives 149 of 149.

**Specs:** 30 lines marked *(observed)*, 19 in 3dAutomask and 11 in 3dUnifize (message order, the unit `scl_slope`, plane outputs written 2D, `-EPI` lines with `-quiet`, and so on).

**Site:** both tool pages now show the original beside AFNI and the replica:
- verdict charts for each implementation, and the original's panels at 1 and 6 threads and in both modes;
- for 3dUnifize, AFNI with correctly rounded maths; AFNI-10 is explained;
- speed charts from the timings.
- Checks:
  - the build has 0 problems;
  - numbers agree 109/109 and 153/153;
  - viewer readouts agree, 32 and 30;
  - a re-export is byte-identical;
  - I reviewed screenshots at 1440 and 390 px, light and dark (`<scratch>/shots/`).
- I found and fixed one bug in my own exporter: the T1w example's default view named a comparison with no displayed panels, which left all its panels blank.

**Gates:** all pass.
- Rust, both workspaces: fmt, clippy and test.
- Python: 465 passed; ruff and SPDX clean.
- Parity: the smoke tier passed inside the golden recording.
- For crates-gpl's notices test I refreshed `crates-gpl/LICENSES/NOTICE-larmorx.txt` as a byte copy of NOTICE; nothing else under `crates-gpl/` was opened.

**Other files:**
- `docs/findings/afni-masks-original.md` (unrestricted);
- both API pages;
- the replica's records moved to `*-replica`;
- the black-box tools committed under `validation/investigations/afni-masks-original/`.

**For the merge:**
- Regenerate the replica records once the corrected replica is in: their `nbhrs-40` and `T2-T2-T2` cases expect a refusal.
- Rebuild the shared `oracles/core-math-preload`: the script now adds `tanhf` and `cbrtf`. My unifize record used a separate build in `oracles/core-math-preload-tanhf`.
- Merge the golden checksums with the decisions part.
- `larmorx_core::defects` gained `AFNI_41` and `AFNI_48`; keep only one copy of each if the replica part added them too.

**Follow-ups:**
- benchmark reports on a quiet machine;
- the missing-input wording difference above;
- a check in the shared evidence code, `common.finish`, that every default comparison has displayed panels.

**Approvals:** none were needed. The thread cap was mostly held. The exceptions: the parity bands run AFNI at 6 threads, and with `--jobs 3` that can briefly exceed 6; the black-box tool checks also ran while the records were going.
