## The glibc maths part, second half: final report (relays 1 to 3, 2026-11-04)

The last relay's report as returned to the orchestrator, condensed for paths; the full texts are in `notes.md` and
`handoff.md` beside this file. The role: the clean-room implementer (never read glibc's source, FSL's source, the
replica crates, the spec writer's model or `validation/investigations/glibc-dbl/tables.py`).

### The functions (relay 1: d0c84e8, 32df5ef)
`sin_glibc`, `cos_glibc` and `sincos_glibc` in `larmorx_core::math`, written from `specs/glibc-dbl.md` alone, with
glibc 2.35's x86-64 FMA build's bits for `reproduce` only; the build without FMA too (`sin_glibc_sse2`,
`cos_glibc_sse2`). Table R generated from its definition by the implementer's own generator
(`validation/investigations/cleanroom-glibc-dbl/cleanroom_tables.py`, mpmath).
- The black-box record: 768 of 768 blocks and 184 of 184 special arguments per function; the SSE2 record 512 of 512.
- Fresh arguments against the system's glibc: 11,811,160,064 per function (FMA build), 5,905,580,032 (SSE2 build);
  0 differ.
- Speed (ns per call, one thread; three argument ranges): `sin_glibc` 10.0 / 13.4 / 34.5, CORE-MATH 6.2 / 6.5 / 12.1,
  glibc 11.2 / 13.5 / 41.1: 1.6 to 2.9 times CORE-MATH's time, as fast as glibc or faster.
- Unit tests compare with glibc's own hashes on every CI platform.

### The `reproduce` paths wired (relays 2a and 2b: b237ac3..c9cf4e9, 7174900..be3db91)
`larmorx_core::math::Libm` (`Libm::new`, `Libm::quiet`) picks correctly rounded or glibc's per call; the default's
values unchanged.
- fslmaths: the double `-sin -cos -exp -log -pow`, the Gaussian kernels, `-dog_edge`, `-bptf`, `-tfce`/`-tfceS`, `-roc`.
- fslstats: `-e`, `-E`.
- FLIRT and its programs: the rotations' `sinf`/`cosf`, the decomposition's `atan2f`, the blurs' `exp`, the sinc
  kernel's `sinf` and `cos`, the mutual information's `logf`; avscale, makerot, `convert_xfm -fixscaleskew`.
- mcflirt's mutual-information costs; FAST; topup's smoothing; bet2's powers; betsurf's blur; 3dTshift's
  `-wsinc5`/`-wsinc9`.
- Not wired, by design: `tan`, `asin`, `acos`, `atan` (the user's decision); fslmaths' ztop/ptoz; the FFT twiddles
  (the original sums directly); mcflirt's and topup's rotations (their specs name no glibc function).
- Notices: GLIBC-07 named exactly, in both modes, only by fslmaths' voxelwise functions, fslstats, avscale, makerot,
  `convert_xfm -fixscaleskew` and 3dTshift; the iterative programs and fslmaths' filters name nothing (the user's
  decision of 2026-11-04).
- Found on the way: `-edge` on a double image computes its squares as products, as FSL does, in both modes.

### Records and golden (relay 2c: bcaae7a, e68b268; from be3db91)
Against main's records only two statuses changed: FLIRT's `real/cost-normmi` and `real/weights-normmi`, fail → pass.

| Record | Cases | Pass / both refuse / expected / fail | What moved |
|---|---|---|---|
| fslmaths | 452 | 384 / 63 / 5 / 0 | 10 new cases pass |
| fslstats | 162 | unchanged | nothing |
| BET | 168 | unchanged | nothing |
| FLIRT | 308 | 210 / 85 / 6 / 7 | 41 cases became FSL's bytes; a few fits moved away within their thresholds |
| FAST | 88 | 60 / 25 / 1 / 2 | fMRIPrep's three calls and every single-channel case FSL's values bit for bit |
| topup | 183 | 94 / 66 / 22 / 1 | 3 new cases |
| head-motion correction | 169 | statuses unchanged | 3 mutual-information cases by thousandths of a mm |
| 3dTshift | 317 | 282 pass | 2 new cases; 180 bit-identical |

- FLIRT toward FSL: the default 12-dof registration and the windowed sinc among the 41; fMRIPrep's three boundary-based
  calls 0.0101 / 0.0011 / 0.006 mm → 0.0065 / 0.0002 / under 0.00001 mm, each inside FSL's own variability.
- FLIRT away from FSL, within thresholds, by the fits' own sensitivity to the last bits (`record_moves.py` measures
  each): `bbr/default` 0.020 → 0.069 mm (threshold 0.1), `real/weights` 0.21 → 0.31 mm (threshold 0.5).
- Golden: smoke tier passed (2,758), `record_golden.py` reused its marker; 223 → 228 cases, 1,583 → 1,623 outputs; no
  earlier checksum changed.
- Three new cases fail on main's code too (from main's newer suites): FAST `opt/nan-first` (FSL-203), FLIRT
  `programs/pointflirt-no-points` (FSL-204), topup `opt/fsl145-one-coefficient` (FSL-145's stop).
- The `mri-hmc` record must be made with `--jobs 1`.

### The site (relay 3: 48691b7..ab2440d; site 2ab3be7, blobs 67aa66e)
- `refresh_original` in FLIRT's, FAST's and topup's evidence modules and `fsl_maths_original.refresh` re-run only the
  clean-room original's runs; FSL's and the replicas' kept, each cache entry recording why. 3dTshift's, the head-motion
  correction's and BET's examples checked unchanged instead (`evidence_unchanged.py`: 19 of 19 original runs, same
  bytes).
- Verdicts: `reproduce` held to FSL as built, the default to FSL with correctly rounded maths; FAST's new row
  "clean-room original, default − FSL, correctly rounded maths"; a default differing only through GLIBC-07 shown as a
  deliberate correction. Fixed: fslmaths' original runs deleting their own new outputs on a recompute; failed checks
  showing "None (threshold None)"; FLIRT's band test ignoring zero displacements. FLIRT's page budget 50 MB (36 → 42 MB:
  the BBR example's default image stored in full).
- Exports of eight tools from clean ab2440d. FLIRT's original bit-identical 131 → 173; FAST's 13 → 49 ("identical with
  correctly rounded maths" 36 → 0).
- Each page: a paragraph "The C library's last digits (since 2026-11-04)".
- Checks: build 0 problems; number check 0 disagreements on all eight pages; viewer check 0 failures on seven; 128
  screenshots before and after; inventory: 123 counts grew, 3 fell, each a difference that vanished (FAST `<code>` 464 →
  436, `<li>` 87 → 86 the legend's no-longer-used verdict, blobs 218 → 217 an identical poster panel now shared).

### Gates
Relay 2c at be3db91: Rust gates on the three workspaces, smoke tier. Relay 3 at ab2440d (Python only): Python gates
(787 passed, 12 skipped), ruff, ruff format, SPDX, `tables --check`.

### Clean-room statements (slips per relay)
Relay 1: an `ls` of `validation/investigations/cleanroom-cbrtf/`. Relay 2: targeted greps of two replica suites' case
tuples and silent lists. Relay 2b: an `ls` of `parity/`; the signatures of a few `fsl_maths_replica` helpers. Relay 2c:
one traceback line of an unnamed file; `ls` of `tests/parity` and the validation package's top level; `git log` of
main. Relay 3: one grep over `validation/src/larmorx_validation/evidence/*.py` (printed only ANTs module names); the
site paragraphs edited also hold the replicas' side (public text). No relay opened glibc's, FSL's or AFNI's source,
`crates-nc/`, `crates-gpl/`, `crates/larmorx-afni-replica`, `tables.py`, the restricted findings, the register,
`docs/parts/`, `validation/probes/` or `triage.toml`.

### Open questions (now in next-steps' follow-ups)
Spec questions (fslmaths §10.4 and §19/§7; FLIRT §3 and §8.1 step 9); FSL-203, FSL-204 and FSL-145's stop in the
originals; the replicas' step (b); `mri-hmc --jobs 1`; FLIRT's `bbr/type-local_abs` labelled *open* where the record
measures no band within the threshold.

### Merge notes (the orchestrator, 2026-11-04)
- Merged into main a52f0ff (two decision-log commits after the branch's base 2e1a062; no conflict); site branch
  2ab3be7 into the site's main 0dbfddb; `evidence-blobs` 206fa20 → 67aa66e (its manifest's SHA-256 d40498f3e7c9c8a1 the
  page branch's); the 13 blobs it no longer lists archived in `<workspace>/archive/2026-11-04-glibc-maths-2-replaced-blobs/`.
- HANDOFF.toml's two operations applied (golden.md, next-steps). Written by the orchestrator from HANDOFF.md: GLIBC-07's
  register row (its description now names the single-precision functions FSL's and AFNI's programs call, as
  `larmorx_core::defects` does; the gap column the replicas' step (b) only), its section and the register's history
  paragraph; the API page's row; next-steps' order item; `docs/status.md`; CLAUDE.md's "Next"; the site's Roadmap row.
  The register's counts unchanged (386 entries).
- golden.md's header recounted: the Python registry 228 cases, 1,623 outputs (it said 208, 1,283); the AFNI replicas
  42 cases, 94 outputs (it said 40, 90; two cases of 2026-10-30).
- `tables --site`, `tables --check`, the summary re-exported (FAST's and FLIRT's verdict moves, 3dTshift's and
  fslmaths' new cases); the site build: 0 problems.
