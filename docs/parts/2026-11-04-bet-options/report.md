## BET's `-S`, `-B`, `-A`, `-A2` replica part: final report (relays 1 to 7, 2026-11-04)

The last relay's report as returned to the orchestrator, condensed for paths; the full texts are in `notes.md` and
`handoff.md` beside this file. The role: the replica author and spec writer (FSL's source, the replica crates and the
restricted findings read; no clean-room crate edited). Branch of 37 commits on 4e1f7fc, last b5d4bc13.

### 1. Step (b): the replicas' `reproduce` on glibc's double `sin`, `cos`, `sincos` (relay 1)
- Wired, in `reproduce` only (the default correctly rounded): fslmaths' `-sin`/`-cos` (GLIBC-07 named exactly where a
  value changes); the Kiss FFT's twiddle factors (`check_fft.py`: 0 of 65,792 factors and 0 of 197,376 transform
  values differ from Armadillo's); the sinc kernels' windows (FLIRT, applyxfm4D, mcflirt); avscale's `sqrtaff` (named
  exactly); FLIRT's stage-4 cost table (the same with either function); AFNI's csfft tables and 3dTshift's Fourier
  phase factors (the same floats in both modes, proved over every table length and all 2^32 float inputs).
- Records: `fsl-maths-replica` 422 cases (360 pass, 61 both refuse, 1 expected, 0 fail):
  `difftest/fft-twiddles-glibc-sincos` expected → pass; the new permanent case `dilall-nan-order` passes;
  `fn/double-tan` stays expected (decided). FLIRT, mcflirt and 3dTshift replica suites re-run: unchanged.
- Two replica mistakes fixed, both a NaN's sign and both in main's build too: cprob's `expx2` under the optimiser
  (`-ztop` of a NaN voxel) and `-dilall`'s summing order.

### 2. `standard_space_roi`, `fslroi` and the four options (relays 2 to 5)
- `bet -S`, `-B`, `-A`, `-A2` exactly as FSL 6.0.7.23's script runs them, every program in-process (FLIRT,
  standard_space_roi, FAST, fslmaths, fslstats, betsurf, fslpy's `immv`, GNU dc's arithmetic), the `ERR` trap and
  bash's abort report; `standard_space_roi` (every option, message, status) and `fslroi` (every argument form) as
  replica programs; `lx.fsl.standard_space_roi`, `lx.fsl.fslroi`, `lx.fsl.bet(variation="eyes"|"bias"|"surfaces"|
  "surfaces_t2", t2=, fsl_data=)`.
- The data: `MNI152_T1_2mm` (`-S`, `-B`, `-A`, `-A2`), `MNI152_T1_2mm_brain_mask_dil` (`-S`, `-B`),
  `MNI152_T1_2mm_eye_mask` (`-S`): 1,420,897 bytes, copied byte for byte from fsl-data_standard 2208.0 and committed
  in `crates-nc/wheel-data/` with FSL's Licence, MNI's notice and a manifest a test checks (the user's decisions of
  2026-11-04); the Linux wheel 6.2 MB; `LARMORX_FSL_DATA` and `$FSLDIR` honoured first.
- Record `fsl-bet-replica` (standard_space_roi's and fslroi's cases inside it), 263 cases: 201 pass (600 of 600 files
  byte for byte), 47 refused by both alike, 15 expected (13 FSL crashes filled deterministically in `reproduce`, 1
  FSL-43, 1 pending: fslpy's `imcp` file-type conversion), 0 fail. Against main's 178 cases none of the common cases
  moved.
- Real calls: fMRIPrep runs none of these options; its 29 BET calls stay bit-identical. Six new real heads pass (an
  adult and a tumour with `-S`, a 3.5-year-old and an 82-year-old with `-B`, a 75-year-old at 1.5 T with `-A`, a
  7.5-year-old with `-A2`), and six real standard_space_roi calls.
- `afni-tshift-replica` re-made: 317 cases, the 2 main's suite had gained both pass. The older replica suites (FAST,
  topup, fslstats, 3dAutomask, 3dUnifize) re-run: identical to main's records.

### 3. Specs (relay 4)
`specs/bet.md` §5.3 (the four variations step by step) and §5.4 (the images and how the original finds them: a named
directory, larmorx-nc's copy, `$FSLDIR`, a checksum-verified download), §7, §9, §10, §11.1; `specs/standard_space_roi.md`
(new).

### 4. Defects (FSL-211 to FSL-217; fMRIPrep affected by none)
| ID | Program | Rule | Default |
|---|---|---|---|
| FSL-211 (e) | standard_space_roi: unquoted names | A | the names as given |
| FSL-212 (c) | standard_space_roi: the cleanup's glob | A | only its own files |
| FSL-213 (c) | standard_space_roi: `maskthr` from the environment | A | ignored, named |
| FSL-214 (c) | standard_space_roi: no step's status checked | B | stop at the first failure, exit 1 |
| FSL-215 (c) | fslroi: exit 0 after its errors | A | exit 1 |
| FSL-216 (c) | bet `-S`: the eye threshold through `dc` | A (B for NaN, infinity) | numbers read as numbers |
| FSL-217 (c) | fslroi, fslmerge (and `bet -F`/`-Z`) write int8 as uint8 | A | int8 kept with its values, named |

FSL-63 widened to `-B -s`. The register: 393 entries.

### 5. Golden, memory, random testing (relay 5)
- Golden: `larmorx-nc` 134 cases (746 outputs), 9 new; FLIRT's header checksums moved (its `descrip` FSL's build
  string; data unchanged); larmorx's 228 unchanged; about +14 s per platform.
- Memory (larmorx / FSL, MiB): `bet -S -m` 37.6 / 66.9; `bet -B` 104.7 / 114.6; `bet -A2` 46.0 / 71.9;
  `standard_space_roi -b` 35.4 / 66.9.
- Random testing, one 0.5 h trial, 2,699 cases: 2,580 agree, 70 notices, 47 differences, 1 timeout, all triaged; two
  replica notice fixes with permanent cases (FSL-63 named when bet2 refused first; FSL-45 for `bet -Z -n`); the
  clean-room original's 190 differences (betsurf on tiny heads) open for the catch-up.

### 6. Gates
At 47f52a8: `gates-rust.sh all`, `gates-python.sh --no-build`, `tables --check`, `check_spdx`, `fsl_source --check`;
no Rust or package code changed after it. Relays 6 and 7: ruff, SPDX, `tables --check`, pytest of `validation/tests`
and `test_fsl_bet.py`. `validation/tests` under `pytest -n 16` fails one or two oracle-cache tests (different ones each
run) at main 0f10327 too; serially everything passes (`validation/investigations/oracle-cache-xdist/`).

### 7. The site (relay 6; branch bet-options-pages 4dddba7, evidence-blobs a1dfe8b, 1,073 blobs)
BET's page: 11 worked examples (7 new: `-S` on ds000005 and ds005003, `-B` on ds000228 and ds003592 with `-s`, `-A` on
ds002422, `-A2` on ds000256 with its T2w, `standard_space_roi -b` on ds000228), three full-resolution slices of the whole
field of view, FSL 6.0.7.23 beside the replica in both modes (and FSL's plain `bet` beside `-S` and `-B`); in
`reproduce` every replica output FSL's bit for bit, the default differing only in the two `-B` examples (FSL-202, and
FSL-09 on ds003592), measured; sections #options, #ssroi, #data; the case explorer on the 263-case record. fslmaths'
page re-exported from its 422-case record. Checks: build 0 problems; numbers 416/416 and 450/450; 332 viewers, 0 bad;
inventory 594 counts grew, 1 fell (fslmaths' "expected divergence: 1" now 0, the FFT case passing); screenshots 1440
and 390, light and dark.

### 8. Open questions (in next-steps' follow-ups)
fslpy's `imcp` file-type conversion not replicated (refused clearly); when bet2 or betsurf abort inside `bet`, the
replica prints its own line instead of the C++ runtime's text and bash's report; applyxfm4D's and mcflirt's replicas
still write `larmorx-nc fsl ...` as `descrip` where FSL writes newimage's build string; 4 standard_space_roi cases
that keep temporary files changed between FSL 6.0.7.17 and 6.0.7.23 without a classified reason; `shtest.rs`'s header
wording on bash's `test` (behaviour emulated, no GNU code copied); for the catch-up: `REPLICA_ONLY_TOOLS`,
`ORIGINAL_PENDING_CASES`, `REPLICA_ONLY_BET_CASES` put back and `original: true` set once the originals exist. The
record `fsl-bet-replica`'s prose notes keep two outdated sentences (options refused; only bet, bet2, betsurf) until its
next re-make; its cases and numbers are right.

### Merge notes (the orchestrator, 2026-11-04)
- Merged into main after cc9cddc and 960c559 (the user's two data decisions); one conflict, golden.json's `recorded`
  line (`merge_golden.py`, re-recorded from the merge). Site branch 4dddba7 into the site's main 51d5465;
  `evidence-blobs` 67aa66e → a1dfe8b (manifest SHA-256 f3f4bc4edc3fd3ea, the page branch's; no blob replaced, 75 added).
- Found at the merge: two standard_space_roi commands on BET's page showed the oracle installation's absolute path
  (`-roiMASK`, `-maskMASK`); the exporter (`evidence/fsl_bet.py`, `command`) now writes a file of an FSL installation
  as `$FSLDIR/...`, and the two lines of the exported JSON were changed the same way.
- HANDOFF.toml's 40 operations applied (12 files); `register_counts.py` 393 entries (a 91, b 13, c 191, d 24, e 59,
  f 13), nothing waiting for the user; `tables --site`, `tables --check`, the summary re-exported; the site build 0
  problems.
- Verified (`verify-merge.sh` on 04c878b, 22:29-23:04; the fslmaths replica's suite then separately on 5d9409e, the
  same code): the three Rust workspaces' gates, the GPL and nc builds, the Python gates, the smoke tier and golden
  (every checksum unchanged, only the header: 5d9409e). Re-runs against the committed records: `fsl-bet-replica`
  263/263, `afni-tshift-replica` 317/317, `fsl-maths-replica` 422/422, no status or detail differs. The clean-room
  original's `fsl-bet` suite (its record of 168 cases not re-made in this part): the 168 unchanged; of the 23 new
  cases (the options'), 21 are expected divergences (pending in the original) and 2 fail
  (`bet/eyes-no-brain`, `bet/surfaces-t2-missing`: FSL fails midway, the original refuses before running), for the
  catch-up part. The evidence re-export of BET's and fslmaths' pages changed only the exporter's fingerprint (site
  54eb60f).
