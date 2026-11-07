## 3dvolreg's replica part: final report (relays 1 to 6, 2026-11-06 and 07)

The last relay's report as returned to the orchestrator, condensed; the full texts are in `notes.md` and `handoff.md`
beside this file. The role: the replica author and spec writer (AFNI 26.3.05's source and the replica crates read; no
clean-room crate edited). Branch of 37 commits on fd3df7b, last 458c73e. The part's agents: a1ad8a5fe6d84dce9,
a1cf390e6ff9d69b7, a9ebe8c3189e65069, a03531d54165580fa, ad17fb9f7ee53ae56, abc04e3d9acb2f4fd.

### The replica
`larmorx afni 3dvolreg`, `lx.afni.volreg(implementation="auto"|"replica")` (`"original"` raises until the clean-room
original exists), in `crates/larmorx-afni-replica` (Apache-2.0 with CC BY 4.0's attribution; NOTICE names 3dvolreg
and carries EISPACK's and libf2c's permission notice for the 3 × 3 SVD): every option (the six interpolation methods
and `-final`, `-twopass` with its blur, coarse shift and rotation searches, the subsampled search for 120 voxels or more
on every axis, `-sinit`, `-zpad`, `-base` as an index or a dataset, the weights, `-edging`, the iteration controls,
`-tshift`, `-rotparent`, `-gridparent`, `-twodup`, `-@`, every output file), every message, exit status and failure
case. Refused: sub-brick selectors (as by every larmorx AFNI tool) and AFNI's `.HEAD/.BRIK` format. 3dvolreg was added
to the oracle builds `oracles/afni-26.3.05` and `oracles/afni-25.2.09` (`build_afni_oracle.sh --add`).

### The record `afni-volreg-replica` (standard tier, from clean commit 1a9e7f2; 386 AFNI runs from the oracle cache)
190 cases: 158 pass, 30 rejected by both, 2 expected divergences (AFNI's own dataset format), 0 failures; 137 of 137
output-writing cases bit-identical; messages 180 of 180; fMRIPrep's own outputs 8 of 8 byte for byte (RobustAverage on
six BOLD runs of five oracle runs: single-band, two volumes, multiband 160 × 160 × 96, multi-echo, multiband-8 uint16,
Philips; sdcflows' PEPOLAR averages on two; the inputs re-made with fMRIPrep's own numpy 2.2.6); fMRIPrep's build of
AFNI 8 of 8; AFNI 25.2.09 → 26.3.05 189 of 189; the Python API against the command line 21 of 21.

### The spec
`specs/3dvolreg.md` (16 sections, behaviour and maths only); its model `validation/investigations/3dvolreg-spec/`
(written from the spec and black-box runs, open to the clean-room implementer): from the spec alone every parameter
within 0.0012 (degrees or mm) of AFNI's on 16 cases; with AFNI's shear-decomposition values put in, bit-identical on
the 12 cases that use no Fourier interpolation. §16 (for the user): whether the spec should describe AFNI's own
shear-decomposition arithmetic so the clean-room original can be bit-identical; the spec writer and the orchestrator
recommend not (the replica gives AFNI's bits; the difference is a fraction of 3dvolreg's convergence threshold).

### Defects (AFNI-86 to AFNI-99; AFNI-91 unused)
Rule A (corrected by default, `reproduce` AFNI's): AFNI-86, 87, 90, 92, 93, 97 (a NULL write on far Fourier
rotations), 98 (`(null)` names), 99 (`-maxdisp` and `-savedisp` computed for mirrored positions on L2R, P2A or S2I
storage: every fMRIPrep input in the record; fMRIPrep writes these numbers and does not use them). Rule B (refused by
default): AFNI-95. Refused in both modes (crashes, an endless loop, a null pointer): AFNI-88, 89, 94, 96. Kept and
documented (rule C, no ID): the blur's row-end mirrors (measured: at most 1.5e-5°), the first pass's weight, the
nearest-neighbour floor, two errors in AFNI's help, the `-_delt` file name. Also answered: `specs/3dTshift.md` §5.2's
open point (`x²` a float32 product).

### Golden, memory, random testing, speed
Golden: the AFNI replicas' 52 cases (129 outputs; 9 for 3dvolreg, text outputs fingerprinted), the main package's 237
cases (`py.afni.fmriprep_volreg_options` added). Memory on fMRIPrep's call: 29.1 MB against AFNI's 27.4 (float32), 22.3
against 20.9 (int16), 31.9 against 25.6 (scaled int16). Random testing: a 30-minute trial of 39,751 cases (all 1,714
differences agreeing after the fixes, AFNI-97 among them), two final checks of 22,539 and 20,824 cases (no difference;
AFNI-99's notice now only where a printed or written value changes); 7 permanent cases. Speed on fMRIPrep's calls, one
thread: 2.2 to 3.1 times AFNI's (ds005454 93.2 s against 40.8 s).

### The site
A new page for 3dvolreg: fMRIPrep's call on ds000005 (the raw run, the volumes 3dvolreg gets, AFNI 26.3.05's, 25.2.09's,
fMRIPrep's AFNI's and the replica's registered series in both modes, the motion parameters and maximum
displacements), the BOLD reference beside fMRIPrep's own, AFNI-99 on the same volumes stored LAS and LPS. Build 0
problems, numbers 97/97, viewers 14/14, every page's inventory after >= before, screenshots at 1440 and 390 in both
themes. `evidence-blobs` b8cffa4 (1,223 blobs).

### Open
AFNI-96 without a suite case (it needs a hand-made AFNI extension); the full tier's 7 more fMRIPrep calls; AFNI-90's
997-byte cut and `-@` without suite cases; AFNI-92's notice whenever its condition holds; the clean-room AFNI crate may
fail on output names of 228 to 255 bytes (relay 3's behaviour-only report, for the clean-room part); the site's home
cards still say the AFNI replicas are in `larmorx-gpl` (the user's design text).

### Merge notes (the orchestrator, 2026-11-07)
- Merged into main 33536e0; three conflicts, each resolved by keeping both sides: `tests/golden/golden.json`
  (`merge_golden.py`, re-recorded from the merge), `tests/golden/registry.py` (`REPLICA_ONLY_FUNCTIONS` holding
  PRELUDE's and 3dvolreg's), `validation/difftest/triage.toml` (both parts' entries; 477). The site's `volreg-pages`
  (9b5c556) into the site's main 72ca95e; `evidence-blobs` 067d4e5 → b8cffa4 (the manifest's SHA-256 745dddd6d7bde217
  matches; no blob replaced, 14 added).
- HANDOFF.toml's 41 operations applied (14 files); `register_counts.py` 421 entries (a 99, b 13, c 205, d 25, e 62,
  f 15); `tables --site`, `tables --check`; the clean-room audit's tables extended for 3dvolreg (the handoff's text:
  `afni-volreg.md` among the source-derived findings, volreg among the AFNI tools and their names, the replica's
  investigations and modules among the brief-restricted files, the replica's modules and four upstream files in the
  similarity mappings).
