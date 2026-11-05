## The clean-room catch-up: final report (relays 1 to 10, 2026-11-05)

The last relay's report as returned to the orchestrator, condensed; the full texts are in `notes.md` and `handoff.md`
beside this file. The role: the clean-room implementer (specs, FSL's documentation, black-box runs of FSL 6.0.7.23
through the oracle cache; never FSL's source, the replica crates, the source-derived findings or the register's
restricted sections). Branch of 50 commits on a5ccbb0 (one of them the orchestrator's, c9158f4), last 7db850a.

### Per tool
- **topup** (relay 1, relay 8): the eight items of `specs/topup.md` §13 closed (FSL-201's `reproduce` with §4.6's
  forward difference, the bending energy's sampling, §7.4's matrices row by row, FSL-133, FSL-134's other shape, the
  short-axis prefilter, FSL-145 exactly, the FSL-146 trigger) and two mask mistakes of the original fixed; the scaled
  conjugate gradients count every iteration toward `--miter`. Record `fsl-topup`: 185 cases, 104 pass, 67 rejected by
  both, 14 expected divergences, 0 fail (main's: 94, 66, 22, 1); six movement cases stay expected: FSL's own path moves
  with its inputs' last bits (up to 5e-3, 2e-2 on four volumes), the default's by 1e-7 to 1.4e-5. applytopup `lsr`
  0.13 s on one thread against FSL's 0.42 s.
- **FAST** (relay 2): FSL-202 and FSL-203 corrected by default and named, `reproduce` FSL's; `_restore`'s NaN rule;
  FSL's standard-output buffering at an abort; `MEAN -nan` with a class without weight. With glibc's functions swapped
  in (a development patch only) the original's default equals the replica's bit for bit on every single-channel case.
  Record `fsl-fast`: 102 cases, 64 pass, 35 both, 2 expected, 1 fail (`multi/s5`, explained as before).
- **FLIRT** (relays 3, 4, 10): the thirteen items of `specs/flirt.md` §14 closed; FSL-204's refusal; pairreg's three
  schedules carried; FSL's 4 × 4 inverse failing only on an exact zero; leastsq's three-level single-precision sums;
  applyxfm4D's spline; `-debug`'s levels; the search's starting points sorted as FSL sorts them (libstdc++'s unstable
  `std::sort`, observed on 829 tied points). Record `fsl-flirt`: 339 cases, 238 pass, 89 both, 10 expected, 2 fail (the
  weighted `normcorr` and `woods` registrations, at their costs' last bits; main's: 308, 210 pass, 7 fail).
  fMRIPrep's three boundary-based calls within 0.0000, 0.0002 and 0.0113 mm of FSL's matrices; ds005040's verdict is
  now open (FSL's own variability 0.0086 mm; 0.0065 mm before relay 4's changes).
- **BET** (relays 5, 6, 7, 10): `bet -S`, `-B`, `-A`, `-A2`, `standard_space_roi` and `fslroi` in the original, each
  running the original's own programs in-process; the data search of `specs/bet.md` §5.4 with
  `larmorx fetch fsl-standard`; FSL-211 to FSL-217 in both modes; four betsurf profile rules corrected (the share of
  profiles set aside FSL's on 162 of 164 images, 74 before; ds002603's inner skull 0.026 mm from 0.95 mm). Record
  `fsl-bet`: 252 cases, 198 pass, 46 both, 6 expected, 2 fail (`real/philips-surf-t2`: ds002603's scalp, 2.86 mm;
  `real/child-3y-B`: `-B`'s robust bet2 with `-c` on a neurologically stored image, Dice 0.9983, 2.24 voxels against
  bet2's 2). Speed 1.4 to 2.9 times FSL's on one thread, 6 to 14 times on 28 (`-B -m` 6.2 s against 85.8 s); memory at
  or below FSL's except `fslroi` (69 MiB against 61, within the 81 MiB allowance).
- **`lx.mri.hmc`** (relay 7): the centres of mass as mcflirt's blocked sums, mcflirt's COG warning. Record `mri-hmc`
  (`--jobs 1`): 169 cases, 145 pass, 20 both, 3 expected, 1 fail (`real/ds003345/default`), unchanged.
- **The `libm` crate:** `acos`, `atan2` and `pow` now from `larmorx_core::math` (AFNI's oblique angle unchanged on all
  1.07e9 float inputs; `pow` glibc's bits on all 91 pairs that reach it); the dependency gone from three crates.

### Defects
FSL-241 (d; `midtrans` beyond double precision, exit 0 with NaN rows: rule B, refused in both modes); FSL-242 (c;
topup's rejected last iteration: rule A, the lowest cost kept by default). The register: 395 entries.

### Random testing (relay 8; 30 minutes each)
topup 17,535 cases (4,223 differ: FSL-201's path, measured against FSL's own spread); FAST 19,695 (50); FLIRT 13,907
(1,560; 873 of the path findings agree after relay 10's fix); BET 909 (114 for the original, 20 since fixed; the
replica's 13 are FSL's FLIRT crashing, FSL-75); `mri-hmc` 83,244 (51,681; on 400 sampled differences aligned as well
as mcflirt, median ratio 1.000). Every difference triaged in `validation/difftest/triage.toml`.

### Golden and gates
236 cases, 1,726 outputs: 20 moved (exactly the named ones), 8 new. The three Rust workspaces' gates, ruff, format,
SPDX; pytest 810 passed (`test_tables` until the handoff), the CI-like run 645 passed.

### The site (relays 9, 10)
topup's, FAST's, FLIRT's, BET's and the head-motion correction's pages: the clean-room original's runs refreshed,
BET's option panels for the original (`evidence/fsl_bet_options_original.py`), the case explorers from the re-made
records, the catch-up trials' tables with their triage; build 0 problems, every number and viewer check, the
inventory against the site's main with no count fallen, screenshots at 1440 and 390 in both themes.
`evidence-blobs` 123c0e6 (1,133 blobs).

### Clean-room statements
Relays 1, 2, 4 and 9: none. Relay 3: four directory names under `validation/probes/`, 40 lines of the replica
suite's plumbing (`run_oracle`, `run_replica`, a dataclass's fields: no FSL-derived content, checked by the
orchestrator). Relay 5: file names inside FSL installations. Relay 6: five option-name lines of
`fsl_stats_replica.py`. Relay 7: the first 40 characters of triage lines. Relay 8: three option-string lines of
`fsl_topup_replica.py`. Relay 9: the defect descriptions of two mixed evidence modules (the register's unrestricted
text). Relay 10: directory names under `validation/investigations/`. No clean-room crate was edited after any of them
with knowledge beyond the specs.

### Open (next-steps' follow-ups, op 70 of the handoff)
The spec writer's questions in one list (topup `--splineorder=2` on short axes and smoothing on a 1 × 1 × 3 image;
FLIRT's weighted costs' last bits and the zero skew's sign under gimbal lock; BET's scalp phase §6.5, `-p` on a
collapsed mesh, `bet -R`/`-c` on neurologically stored images; standard_space_roi's output header on 17 cases), the
ds005040 verdict, the head-motion correction's default tail, the BET replica's FSL-75 cases, the oracle cache's empty
directories, CI's golden time.

### Merge notes (the orchestrator, 2026-11-05)
- Merged into main a5ccbb0 without conflict. At the merge: one investigation script's absolute workspace path
  (`cleanroom-flirt/trial_catchup_replay_fixed.py`) replaced by `LARMORX_WORKSPACE` or the repository's parent, as
  the other investigations do; the BET part's open triage entry for trial 2026-11-04-bet-options-6b retired during the
  part (commit c9158f4, kept in the workspace archive).
- Site branch `catchup-pages` 8f321ef into the site's main 54eb60f; `evidence-blobs` a1dfe8b → 123c0e6 (the
  manifest's SHA-256 6ca9d3a050157ca6 matches; 49 blobs replaced, kept in the workspace archive; 109 added).
- HANDOFF.toml's 84 operations applied (8 files); `register_counts.py` 395 entries (a 91, b 13, c 192, d 25, e 59,
  f 13), the register's counts updated; `tables --site`, `tables --check`, the summary re-exported; the site build 0
  problems; the decision-log entry "The clean-room catch-up" (the data cache and command, FSL-241, FSL-242).
