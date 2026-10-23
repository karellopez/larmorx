# Final report: antsAI -b, ANTS-106, ANTs variation

Agent `aa903a90387159f4f`, 2026-10-23. The agent's report to the orchestrator, verbatim.

All three tasks of the brief are done, plus the site part. Every gate passes. The larmorx worktree branch `worktree-agent-aa903a90387159f4f` ends at `1d21507`, 16 commits, clean. The site clone `<scratch>/site` has branch `ai-blobs-evidence`: one squashed commit, `18a24f0`, on top of the site's 2a90427. Nothing was pushed or merged. Every commit uses the no-reply identity with no AI attribution. The exact text for every shared document is in `<scratch>/HANDOFF.md` (scratch = `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/ai-blobs`).

**1. `antsAI -b` (blob alignment), every step ported**
- What was built:
  - ANTs' blob detector, including its per-thread heaps and ANTS-60 (`crates/larmorx-image/src/blobs.rs`).
  - Patch correlations in float, pairing, the landmark distance test and the fits (`crates/larmorx-ants/src/ai_blobs.rs`).
  - ITK's landmark fits for affine, VersorRigid3D and Rigid2D (`crates/larmorx-registration/src/landmark.rs`).
  - vnl/LINPACK QR (`crates/larmorx-core/src/vnl_qr.rs`) and Eigen's 4×4 self-adjoint eigensolver (`crates/larmorx-core/src/eigen_sym.rs`). That solver works on a column-major copy; this was found and matched on 650,003 matrices.
  - CLI `-b n` and `-b [n,m]`; `lx.ants.ai(blobs=…)`; a new `lx.ants.blob_alignment`, which returns the blobs, the correlation matrix, pairs, landmarks and transform.
  - Every message and failure is covered (fewer than 3 blobs, empty blob list, missing image, thin image, no landmarks).
- Parity at one thread against the unmodified ANTs:
  - `ants-ai` standard: 131 cases, all agree; 75 compared, 74 identical as built and 1 with CORE-MATH; 52 rejected by both; 4 expected divergences.
  - Full tier: 144 cases; 86 + 1 of 87 compared; includes all five oracle runs' T1w against MNI.
  - An instrumented copy of antsAI's own code, now committed as `validation/probes/ants-ai-blobs/`, gives the same blobs, correlations, pairs, landmarks and transform bit for bit.
- Threads: ANTs' `-b` crashes (signal 11) at 2, 4 and 6 threads in every run. At one thread it is repeatable. larmorx gives the one-thread result at any thread count.
- Distance from the known transform (mean / largest; full table in the record):

| Moving image | ANTs (one thread) | larmorx default |
|---|---|---|
| Translated phantom | exact | 0.50 / 1.4 mm |
| 3D phantom rotated by 7° and 4°, rigid fit | 34 / 67 mm | refuses (no landmark) |
| Same phantom, affine fit | 654 / 1520 mm | refuses (no landmark) |
| 2D phantom rotated by 9° | 6.2 / 12 mm | 1.6 / 3.5 mm |
| Real T1w, shifted | exact | 1.2 / 2.1 mm |
| Real T1w, rotated by 8° | NaN | 1.0 / 1.9 mm |
| T1w against MNI, any of the five runs | no landmark | no landmark |

- The translated cases are the one place the correction does worse than ANTs: its distance test also keeps a pair matched to a neighbouring blob. The records, findings and site say so.
- Defects found:
  - **ANTS-110** (the landmark test compares displacement lengths, not distances), **ANTS-111** (the first blobs are paired again once correlations run out), **ANTS-112** (the detector does not keep the strongest maxima), **ANTS-114** (no landmark writes a NaN transform) and **ITK-60** (the 2D rigid fit takes −90° near a quarter turn). Each is corrected by default, with a switch to ANTs' behaviour.
  - **ANTS-115** (empty blob list, a crash) is refused.
  - **ANTS-116** (patches can be compared reflected) is marked **decide** for the user.
  - ANTS-60 is now measured. ANTS-113 is unused.
- The rotated T1w was rebuilt as a true rotation in millimetres; a voxel-grid rotation of this T1w (1 × 1.33 × 1.33 mm) was sheared.

**2. ANTS-106 in Atropos**
- A partial-volume label now counts as a member of its own set in the MRF; `reproduce` keeps ANTs' behaviour and its stop.
- Measured on a phantom with known tissue fractions:
  - MRF 0.1: partial-volume Dice 0.295 / 0.405 in ANTs (94.0 % of voxels right) against 0.862 / 0.805 corrected (97.7 %).
  - MRF 0.3: ANTs stops; corrected gives 0.835 / 0.801 (97.8 %).
  - Without the MRF: ANTs 0.728 / 0.578; larmorx's default 0.711 / 0.594 (ITK-50 and ANTS-96 alone).
- On sMRIPrep's call with partial-volume classes on the real T1w, ANTs stops; corrected, it runs.
- `ants-atropos`: 97 cases standard and 102 full, all agree; 58 of 58 and 62 of 62 image cases are bit-identical with CORE-MATH.

**3. ANTs' run-to-run variation**
- The cause is ANTS-72, already in the register. A linear stage after a displacement field picks 1000 voxels with a generator seeded from the clock.
- Confirmed: fixing `time()` and `clock()` through a preloaded library makes the runs identical.
- The corrections harness now runs those references five times and reports the median, the range and ANTs' own spread: up to 8.5 mm apart (`chain/bspline-then-rigid`). That record still passes 19 of 19.
- No registration code changed.

**Tests:**
- Rust unit tests for the detector, QR, Eigen, landmark fits and pairing.
- `tests/python/test_ants_ai_blobs.py` and an ANTS-106 test.
- New golden cases `cli.ants.antsAI.blobs`, `py.ants.blob_alignment` and its `.reproduce`, plus a partial-volume-with-MRF output in `py.ants.atropos`. They were recorded from a clean commit; only the new cases changed.

**Site** (antsAI, Atropos and antsRegistration pages):
- antsAI: three worked examples — the blobs each program finds, the shifted copy, the rotated copy — with pairs, transforms, placed images, mm distances and ANTs' own variability. The verdicts are bit-identical (`reproduce`) and deliberate correction (ANTS-110).
- Atropos: the phantom example and a table of the partial-volume scores.
- antsRegistration: the spread column and corrected text.
- Checks:
  - Number check 315/315, 308/308 and 953/953.
  - Every viewer check passes and the build reports 0 problems.
  - A re-export changes nothing.
  - Your request is done: each Atropos example now has its own outputs list and default windows. Only the first example's grey-matter window changed. Re-exported and checked.
- Screenshots to look at: `<scratch>/shots/final-*` (desktop 1440 and phone 390, light and dark).

**Gates:** both Rust workspaces (fmt, clippy, test); ruff; SPDX; `tests/python` + `tests/golden` 441 passed; `tests/parity` 2087 passed.

**Decisions for you or the user:**
- ANTS-116.
- Eigen's MPL-2.0 file in `larmorx-core`: keep it, or replace it with a clean-room solver that would not be bit-exact (HANDOFF §6).

**Approvals:** nothing needed one. I used port 8793 because 8791 is the other agent's site server.
