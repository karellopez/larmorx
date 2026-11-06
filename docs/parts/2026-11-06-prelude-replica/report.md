## PRELUDE's replica part: final report (relays 1 to 6, 2026-11-06)

The last relay's report as returned to the orchestrator, condensed; the full texts are in `notes.md`, `handoff.md` and
`handoff-relay5.md` beside this file. The role: the replica author and spec writer (FSL's source, the replica crates and
the restricted findings read; no clean-room crate edited). Branch of 27 commits on 43f0738, last b2a352b8. The part's
agents: a9212aaf7ebf67263, a629d82b6d4ce1ed0, ab52d2a601d874a86, a34f6404e0a5b35ab, a27906fd077775db0,
a69e46e3190d71db6.

### The replica (relays 1 to 3)
- `larmorx-nc fsl prelude`, `larmorx fsl prelude`, `lx.fsl.prelude`: FSL 6.0.7.23's `prelude` (fsl-fugue 2603.0)
  complete, every option, message, exit status, failure case and output type; the new crate
  `crates-nc/larmorx-nc-fugue` (FSL's `prelude.cc` and the `unwarpfns` routines it calls) and newimage's complex-image
  reader in `larmorx-nc-core`; FSL's source in `crates-nc/fsl-source` with its manifest. The unwrapping's FSL loops
  reorganised without their quadratic cost, the arithmetic and its order unchanged (a unit test against a line-by-line
  translation): fMRIPrep's ds005454 call 0.15 s against FSL's 6.5 s (one informal run).
- Record `fsl-prelude-replica` (standard tier, from clean commit f4882fe): 88 cases, 68 bit-identical, 20 refused by both
  alike, 0 expected divergences, 0 failures. fMRIPrep's 7 `prelude` calls (ds005454 ×1, ds002603 ×1, ds002422 ×2,
  ds001600 ×3) bit-identical to FSL and to fMRIPrep's own outputs; the data panel's two other phase-difference maps
  (ds000256, ds004958) among the cases.
- Golden: 15 cases and 4 failure cases (69 checksum rows, recorded from fb86dc8; every other row unchanged). Memory:
  ds005454's call 46.2 and 46.6 MiB against FSL's 35.9 (`-s --removeramps` with every output: 47.2 against 32.5;
  checked at the merge). Random testing: one 0.5 h trial, 50,711 cases, 50,702 agreeing, the other 9 the generator's
  own mistake (fixed, agreeing when drawn again); two replica notice mistakes fixed with permanent cases.

### The spec and the waiting questions (relays 4, 5)
- `specs/prelude.md` (718 lines, behaviour only); a model written from its text alone
  (`validation/investigations/prelude-spec/`, open to the clean-room implementer to run) reproduces FSL's files and `-v`
  lines on 22 of 22 unwrapping cases and fMRIPrep's 7 of 7 calls.
- Answered: topup §7.3 (FSL-273: the field's Jacobian scales; the topup replica's default now scale 1, its record re-made
  177 cases: 107 pass, 67 both, 3 expected, 0 fail); FLIRT §6.4 (normcorr adds each point's weight as visited: the cause
  of `real/weights-normcorr`; `specs/mcflirt.md` §6.4 too); FLIRT §3/§8.2's zero signs (partly: the third skew narrowed);
  fslmaths §10.4, §7, §19; mcflirt appendix A (its leastsquares is FLIRT's). Open: BET, FAST, topup's `--splineorder=2`
  bending and 1×1×3 smoothing, standard_space_roi's headers, FLIRT's other items and `real/weights-woods`.

### Defects (FSL-261 to FSL-273; fMRIPrep affected: FSL-272 on ds002603's map)
Rule A (corrected by default): FSL-261, 262, 264, 265, 266, 268, 269, 273. Rule B (refused): FSL-263, 270, 271.
Rule D, measured, then A: FSL-272 (a NaN first phase voxel skips the range check and the rewrap: on 7 of 8 maps FSL's
output whole cycles wrong in some regions; on ds002603 6,612 voxels, 2.5 % of the mask). Message only, kept: FSL-267.

### The site (relay 6)
A new page for PRELUDE (fMRIPrep's seven calls, FSL-272 on ds002603, FSL-261 on ds002422's second map, a synthetic
phase with a known truth; the table of calls, the trial and the 88 cases), and topup's page re-exported after its
replica's record (177 cases, FSL-273). Build 0 problems, numbers 90/90, viewers 30/30, every page's inventory after >=
before, screenshots at 1440 and 390 in both themes. `evidence-blobs` 067d4e5 (1,209 blobs).

### Merge notes (the orchestrator, 2026-11-06)
- Merged into main fd3df7b without conflict; the site's `prelude-pages` (384bb72) into the site's main aca371e;
  `evidence-blobs` 123c0e6 → 067d4e5 (the manifest's SHA-256 c191a17069cb8c2a matches; no blob replaced, 76 added).
- HANDOFF.toml's 24 operations applied (11 files); `register_counts.py` 408 entries (a 93, b 13, c 201, d 25, e 60,
  f 14), the register's counts updated; `tables --site`, `tables --check`, the summary re-exported; the site build 0
  problems; the site's stale fslmaths and fslstats summaries ("the clean-room original planned") corrected.
- The clean-room audit's tables extended for PRELUDE (the relay's text): `fsl-prelude.md` among the source-derived
  findings, prelude among the FSL tools and their names, the replica's investigations among the brief-restricted
  files, `fsl-fugue` and `larmorx-nc-fugue` in the similarity mappings.
