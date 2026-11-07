## PRELUDE's clean-room original part: final report (relays 1 to 5, 2026-11-06 and 07)

The last relay's report as returned to the orchestrator, condensed; the full texts are in `notes.md` and `handoff.md`
beside this file. The role: the clean-room implementer (`specs/prelude.md` and the other specs, FSL's documentation and
papers, black-box runs of FSL 6.0.7.23 through the oracle cache; the spec writer's probe and model run, not read).
Branch of 43 commits on 1eb168f, last 245282f. The part's agents: af4a71de4e2ee037f, ab67a91e24eb199bf,
a9e2f00fd73245bd2, a78cd0e2abb1f6620, a8e31783627a47230.

### The original
`crates/larmorx-fsl/src/prelude/` and `cli/prelude.rs`; `larmorx fsl prelude` (`original: true`),
`lx.fsl.prelude(implementation="original")`; `auto` runs the replica when `larmorx-nc` is installed, else the original
(the rule of every FSL tool). Every option, message, exit status and failure case of the spec; FSL-261 to FSL-272
corrected or refused by default as the replica's default, `reproduce` FSL's; complex inputs name GLIBC-07 where the
correctly rounded arctangent and glibc's `atan2f` differ on a voxel. The regions' merging on a heap updated in place
(a test against §4.8 written out literally on 60 random graphs with ties and NaN).

- Record `fsl-prelude` (from clean commit 1ca117e): 166 cases, 138 pass, 28 rejected by both, 0 expected, 0 fail;
  `reproduce` 282 of 282 headers and data FSL's bit for bit; the default 248 of 248 the replica's default; fMRIPrep's
  7 calls fMRIPrep's own outputs bit for bit in both modes, at 1 and 14 threads.
- Speed (one thread; FSL / replica / original): ds002603 28.44 s / 0.374 s / 0.135 s (211 times FSL's speed);
  ds005454 6.68 / 0.173 / 0.069 s (97 times); a 128³ map 183.7 s / 1.36 s / 0.30 s; 256 × 256 × 160: replica 9.41 s,
  original 1.97 s. Memory: fMRIPrep's call 35 MiB against FSL's 32; the 128³ map 59 MiB against FSL's 64 (the
  replica 117). `docs/benchmarks/fsl-prelude.md`.

### The follow-ups the spec answers let it close, and the shared fixes
- FLIRT's `normcorr` count per point (`specs/flirt.md` §6.4): `fsl-flirt` 339 cases, 239 pass, 89 both, 10 expected,
  1 fail (`real/weights-normcorr` now passes; `real/weights-woods` still fails).
- The same engine in `lx.mri.hmc`, its `leastsquares` as FLIRT's: `mri-hmc` (`--jobs 1`) 170 cases, 145 pass, 20 both,
  5 expected, 0 fail (`ds003345/default` passes; mcflirt's matrices as text 231 of 11,476 identical, 54 before). Two
  synthetic runs became expected divergences only after mcflirt's own spread was measured (`hmc_spread.py`:
  `ras-unflipped/reffile` 0.110 mm in mcflirt's 0.065-0.131; `ramp/reffile` 0.342 in 0.094-0.493).
- topup's coefficient-field Jacobian (FSL-273): scale 1 by default, FSL's in `reproduce`; `fsl-topup` 187 cases, 106
  pass, 67 both, 14 expected, 0 fail.
- fslmaths' FFT twiddles in `reproduce`: not applicable (the clean-room fslmaths has no FFT); a spec question.
- The shared FSL image code: pair headers of 352 (NIfTI-1) and 544 (NIfTI-2) bytes, FSL's (105 of 106 header files
  identical, the other FSL-56); the time step as FSL reads it (63 of 66 runs identical, the others FSL-262's default);
  `fsl-maths` 454 cases, 386 pass, 63 both, 5 expected, 0 fail. `fsl-bet` and `fsl-fast` re-run unchanged.

### Golden, random testing, defects
Golden: 3 cases added, 12 re-recorded on purpose (the hmc and mcflirt headers, the count, `leastsquares`); 240 cases
after the merge. Random testing: one trial of 23,780 cases (1,650 differences the time step, fixed with a permanent
case; 14 GLIBC-07 notices explained; 7 open, triaged: 6 oblique inputs with FSL's 1e-15 offsets, 1 FSL-271 with `-s`).
No new defect (FSL-274 to FSL-280 unused).

### Open questions for a spec writer
fslmaths §19 and §10.2 (FSL's own FFT); the pair header's extension bytes in `prelude` §3.3 and `bet` §3.3; the time
step as FSL's image layer reads it in the FSL specs; no `Loading volumes` before a complex image (§4.11); the 2D
numbering after a slice emptied by FSL-269's correction (§4.6, §9.3); FSL's 1e-15 offsets on oblique inputs; FSL-271
with `-s`; where the mcflirt and FLIRT paths still part after 30 to 40 evaluations.

### Clean-room statements
None opened restricted content (every relay's list in `notes.md`): relay 1 a diffstat's restricted file names and its
stand-in script named `larmorx-nc` (judged no slip by the audit at 3dvolreg's merge); relay 2 a traceback's two call
lines of the replica suite, the original's BET suite lines read, the replica's memory test; relay 4 five `docs/parts`
file names, six harness lines of `parity/afni_replica.py`; relay 5 greps without matches, the role lines of other
parts' briefs in the transcripts, the trial merge's conflict hunks. Operational: relay 1's recursive stand-in spawned
about 100,000 processes on 2026-11-06 (cleared; the boxes now cap their tasks).

### Merge notes (the orchestrator, 2026-11-07)
- Merged into main dc79820; four conflicts: `README.md` (generated: regenerated), `tests/golden/golden.json`
  (`merge_golden.py`, re-recorded from the merge), `tests/golden/registry.py` (`REPLICA_ONLY_FUNCTIONS` with 3dvolreg
  only), `validation/difftest/triage.toml` (both sides; 481 entries). The site's `prelude-original-pages` (845fae4) into
  the site's main f16f910 with the merged manifest of 1,223 blobs (the conflict in `blobs.sha256`); `evidence-blobs`
  b8cffa4 → 1b956fc (the manifest's SHA-256 adb6926c77210563; 8 blobs replaced by the original's refreshed runs, kept
  in the workspace archive; 8 added).
- HANDOFF.toml's 63 operations applied (9 files); `register_counts.py` 421 entries; `tables --site`, `tables --check`,
  the summary re-exported; the site build 0 problems.
