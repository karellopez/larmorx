## The spec answers for topup, FAST and FLIRT: final report (relays 1a to 5, 2026-11-03)

The last relay's report as returned to the orchestrator (subagents cannot write report files), for paths and its
layout; the per-question tables are in the specs themselves (`specs/topup.md` §13, `specs/fast.md` §12,
`specs/flirt.md` §14).

The part is done. It answered every question the three clean-room originals had left open, registered four new FSL
defects, corrected them and four already decided ones in the replicas, and updated the site, the handoff and NOTES.

### The answers
- **topup** (9 questions, all high confidence): the rotation derivatives (FSL-201: forward differences of
  single-precision rotation matrices; `rotation_model.py` gives FSL's gradients to 2e-5 to 1.5e-4); the refit between
  levels (right on all three points; probes D to H to 7e-7 Hz rms); the quadratic bending energy at an even spacing
  (incomplete: FSL samples each basis function at the integer offsets strictly inside its support, the inner knots the
  mean of the one-sided second derivatives; reproducer 10753's Hessian to 8.6e-7); `lsr` on lines that divide 2^32
  (wrong cause: the mapping folds and FSL builds the matrices row by row; reproducer 8089 to 3.2e-5, the replica's
  `reproduce` bit for bit); FSL-133's read and FSL-134's field in another shape (incomplete; to 2.6e-5 and 1.1e-5);
  axes of 8 voxels or fewer (a truncated prefilter, about 0.268^N); the conventions (right; FSL-145 lays 1 coefficient
  when the axis is shorter than one spacing, and the general count at spacing 2 for axes of 2 or 3 voxels); FSL-146's
  mechanism (right).
- **FAST** (21 questions): 16 right, 4 incomplete (`_restore` at NaN voxels, the lines an abort loses, the partial
  volumes' near ties (one step, medium-high confidence), case 1840's mechanism), question 6 a defect (FSL-202);
  FSL-203 found on the way. Models: `fast_probes.py`, `variance_measure.py`, `variance_reference.py`,
  `sweep_schedule_model.py` (a lag of two rows bit-identical on 6 of 6 grids), `pv_ties_model.py`.
- **FLIRT part 2** (38 questions; 3, 15 and 29 already settled): 9, 10, 16 (the spec's own error), 20 and 37 wrong
  readings corrected; 2, 7 and 22 incomplete (22: the sinc's last bits come from glibc's `sinf`); the rest right; high
  confidence but 36 (medium). Checks: makerot 11/11, avscale 16/16 and 234/234, readers 19/19; sinc 840,862 of 840,862
  voxels with glibc's sine, spline 13,315/13,315, leastsq 6/6, the equal-cost ties' 3,256 evaluations × 3 traces,
  `-debug` 8/8 levels. `specs/mcflirt.md` §6.1: mcflirt's centres of mass use the same blocked sums (question 25).

### The decisions
- **FSL-201** (rule D, then A): of 22 runs that estimate movements, the exact derivatives end at a lower or equal cost
  in 18; the other 4, cut short, end 0.004 to 0.13 % higher; all 8 random cases whose volumes moved end lower (0.01 to
  44 %); a converged 60-iteration run 38 % lower. fMRIPrep's two calls estimate no movements: unchanged.
- **FSL-202** (rule D, then A, after the orchestrator asked for the gain to be measured): the corrected variance equal
  (or within one voxel) to the double-precision reference on 7 of 7 cases, its own last-bit band 0 where FSL's
  reaches hundreds of voxels. fMRIPrep's FAST outputs change by default (ds000005 54 `_seg` and 112 `_mixeltype`
  voxels; ds000228 66 and 89; ds005003 392 and 453); `reproduce` unchanged.
- **FSL-203** (FAST, a NaN first voxel; rule A) and **FSL-204** (pointflirt without points crashes; rule B, refused
  in both modes).

### The replicas' switches (default the correction, named; `reproduce` FSL's)
FSL-111 (applyxfm4D's uint8 images read as stored), FSL-144 (applytopup `jac` with a z phase encoding refused),
FSL-145 (the general quadratic count; applytopup reads either count; `reproduce` FSL's count and its refit error),
FSL-146 (the cost's own gradient), FSL-201 (exact derivatives), FSL-202 (the variance in double), FSL-203 (the extrema
over the numbers; `reproduce` FSL's exit 255), FSL-204 (refused in both modes). The `larmorx_core::defects` texts of
FSL-125, FSL-134 and FSL-145 corrected; FSL-201 to FSL-204 new.

### What was run
- FSL: every run through `oracle_cache.run` in `use` mode (the relays' probes stored; everything else served).
- Records once each from clean 53796ce: `fsl-topup-replica` 176 cases (106 pass, 67 rejected by both, 3 expected),
  `fsl-fast-replica` 87 (59, 27, 1), `fsl-flirt-replica` 305 (211, 87, 7); `compare_records`: no status change, only
  the new cases and the fix passes the switches touch moved (topup 37, FSL-201; FAST 35, FSL-202).
- The smoke tier once at 53796ce: 2,742 passed. `larmorx-nc`'s golden re-recorded from c7ac893 (121 cases, 679
  outputs; 4 new; only FSL-201/202/145's defaults moved); the main package's golden unchanged.
- The site: FAST's, FLIRT's and topup's evidence re-exported from clean commits (3b65d4e, 4c047cf, e3ad16b); a new
  topup example (`#movements`, `opt/levels`): the replica's default and the clean-room original agree within 3.7e-6 mm
  and 1.9e-7 rad, FSL 3.1e-4 mm and 8.4e-5 rad away. Build 0 problems; number checks FAST 1641/1641, topup 943/943,
  FLIRT 319/319; viewers 520, 182, 38; inventory after >= before on all 21 pages; screenshots before and after.

### Gates
Relay 4: the three Rust workspaces, `gates-python.sh --no-build`, `tables --check`. Relay 5 (no Rust change):
`gates-python --no-build` (608 passed), `tables --check`.

### Commits
Relays 1 to 3: 60667ad, acd1bbe, 842eb4d, d305c81, 0221658, 75860a8, 74466ed, 20c0165, 3d2478a, 5ea4b8a, d7dc8f2,
2412431, ad6610c, de83d14, 5194f70, 81d75d6; relay 4: 7a3fc62 (merge of main), b91a605, e05cb7c, 64c92ec, 53796ce,
0971b20, c7ac893, fabf9c2; relay 5: 3b65d4e, c50ba69, 4c047cf, 5d896a0, e3ad16b. Site: c030e59, 4a70f2e; blobs d3ee6ba
(994).

### What each clean-room original can now close
- topup: FSL-201's `reproduce` (9 record cases, 4,462 random), the bending energy's sampling (47), `lsr`'s matrices
  (8), FSL-133's read (484), FSL-134's other shape, the short axes' prefilter, FSL-145's exact `reproduce`; to check:
  its FSL-146 trigger for a volume with read-out time 0.
- FAST: FSL-202 (the new §4.4 by default) and FSL-203, `_restore` at NaN voxels, the lines lost at an abort, case 1840.
- FLIRT: the 13 numbered follow-ups in next-steps (the trial's 114 `-debug` findings, `real/cost-leastsq`, ...); its
  `reproduce` of `real/interp-sinc` and the `real/dof6`-like traces needs glibc's `sinf`, `cosf` and `atan2f`.
- `lx.mri.hmc`: mcflirt's blocked centres of mass.

### Not done
FSL-111's notice in `reproduce` (by design, the default only); a parity case for applytopup reading a full-count
coefficient file (the golden case covers it); the sign of a near-zero skew in FLIRT's FSL-91 trace (the original's own
arithmetic); whether FSL-203's mechanism reaches BET's, FLIRT's or fslmaths' replicas; `docs/figures.md` on the new
`refresh_replica` command.

### Merge notes (the orchestrator, 2026-11-03)
- Relays 1a, 1b, 2, 2b, 3, 3b, 4 and 5; each report read before the next relay was launched, the orchestrator's items
  in NOTES.md ("FROM THE ORCHESTRATOR"): FSL-201 accepted on its measurement; reproducer 8089 run once more (raised to
  high confidence); FSL-202 held until relay 2b measured the gain (the double-precision reference and the corrected
  variant's own band), then rule A; `specs/mcflirt.md` §6.1 given to the spec writer; sub-task 4 extended to FAST's
  replica (FSL-202, FSL-203) and FLIRT's FSL-204; the site update for the defaults the switches changed.
- Scans: 29 commits, all by the no-reply identity, no AI attribution, no home path, account name or hostname added;
  no clean-room crate or clean-room findings file touched; the shared files touched in the branch are the generated
  tables of README.md and docs/validation/README.md only (regenerated again at the merge). The site branch: two
  commits, the same identity.
- No conflicts in either repository. The 34 handoff operations applied (the BET spec's ANALYZE paragraph accepted:
  the same reading FAST's answer found); the register recounted (383 entries: a 91, b 13, c 182, d 24, e 58, f 13,
  explained 2; no **decide**, no **yes**); the decision log's entry for this part; `docs/status.md`, CLAUDE.md's "Next".
- The blobs: 79 added, 13 replaced (re-exported panels); the replaced ones archived in
  `<workspace>/archive/2026-11-03-spec-answers-replaced-blobs/`. The agents' 66 scratch scripts whose names no
  tracked file carries archived in `<workspace>/archive/2026-11-03-spec-answers-3-scratch-scripts/` (dev patches with
  FSL-licensed code among them: never published).
- For the user, in one batch with GLIBC-07's cbrtf route: copying glibc's exact single-precision functions (Arm's and
  fdlibm's permissive code, now in the FSL replica core) into `larmorx_core::math` for the clean-room originals'
  `reproduce`.
