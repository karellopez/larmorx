# HANDOFF: the clean-room fixes (2026-10-29)

Branch `worktree-agent-ad07925a8f194e151` (base 8318394). The exact text for each shared
document; each block says where it goes.

## 1. `validation/difftest/triage.toml` (I could not read it: each item by tool, implementation and signature)

Set every entry matching the left column to the new status and explanation:

| Tool / implementation | Signature (the failed checks) | status | explanation (new text) |
|---|---|---|---|
| `afni-tshift` / replica | `D13 defaults: replica = original` (geometry (sform/qform), metre or micrometre units; 83 findings) | fixed | Fixed in the clean-room original (2026-10-29): AFNI-07's default now writes the replica's steps (the file matrix's column lengths, then converted) and, for a qform, the qform matrix computed with millimetre voxel sizes and offsets, rounded once; the notices in the replica's words. All 83 replay as agree. Permanent case `afni-tshift/metre-oblique-default-steps`. |
| `afni-tshift` / replica | `D13 defaults: replica = original`, `D13 fix pass` (20261025-01690: AFNI-01 and toffset) | fixed | Fixed with the toffset item (header times in milliseconds); replays as notice (AFNI-01 named where the input shows no change: the replica's own fix pass, unchanged). |
| `afni-tshift` / original | `header fields (scaling, timing, slice fields, units, xform codes)` (toffset 0.35999995 against 0.36000001; 2 findings) | fixed | Header slice times in milliseconds or microseconds: the step in seconds stays a double and each time is a float32 running sum of it (observed, `-slice k` reads every time back). Both replay as agree. Permanent case `afni-tshift/header-milliseconds-toffset`. |
| `afni-automask` / original | `mask.nii: header bytes`, `mask.nii.gz: header bytes` (tiny oblique images; 5 findings) | fixed | The quaternion's polar decomposition takes `1/det` in x87 extended precision (observed on 1,200 matrices through NIfTI-2 outputs). Replay as agree. Permanent case `afni-automask/tiny-oblique-quaternion-residue`. |
| `afni-automask` / original | `messages: AFNI's, byte for byte` (`[from ?]`; 29 findings) and the one-slice oblique `mask.nii.gz: data` with `-SI` | fixed | The axes' orientation letters by the NIfTI library's closest-orientation method in double (observed on 1,604 matrices); `-SI` follows. Replay as agree. Permanent cases `afni-automask/oblique-orientation-letters`, `afni-automask/plane-oblique-si-orientation`. |
| `afni-automask` / original | `D13 fix pass`, `D13 notices in both modes` (header slice code 2, `masked.nii header changed`, `fix: []; reproduce: ['AFNI-08']`; 10 findings) | fixed | AFNI-03 named in both modes and AFNI-08 in `reproduce`, in the replica's words; the masks harness accepts AFNI-08 in one mode only for header slice code 2 (`one_mode`). Replay as agree. Permanent case `afni-automask/slice-code-2-masked-notices`. |
| `afni-automask` / original and replica | `D13 fix pass` (`no defect named: no values; messages changed`; `-peels 0`; 7 + 6 findings) | fixed | AFNI-43 named where only the clustering messages differ, in the replica's words; a negative `-peels` peels once in both modes as the replica's default. Replay as agree. Permanent case `afni-automask/peels-0-clustering-messages`. |
| `afni-automask` / original | `masked.nii: data` (20261025-01305, the sign of zero) | fixed | int8, uint16, int32, uint32 and float64 data without a usable slope are only converted, so a float64 −0 stays −0. Replays as agree. Permanent case `afni-automask/float64-negative-zero`. |
| `afni-unifize` / original | `D13 fix pass`, `D13 notices in both modes` (`-EPI`, header slice code 2; 7 findings) | fixed | As 3dAutomask's: AFNI-03 named in both modes, AFNI-08 in `reproduce`, the replica's words. Replay as agree. Permanent cases `afni-unifize/epi-slice-code-2-fix-notice`, `afni-unifize/epi-slice-code-2-reproduce-notices`. |
| `afni-unifize` / original | `uni.nii: header bytes`, `uni.nii.gz: header bytes`, `am.nii: header bytes` (2 findings) | fixed | The quaternion fix above. Replay as agree. Permanent case `afni-unifize/tiny-oblique-header`. |
| `mri-hmc` / original | the matrices, parameters, voxels, RMS files and FD beyond the thresholds (782 findings), every cost but `normmi` (691) | explained | Measured on all 782 findings (`validation/investigations/cleanroom-fixes/hmc_alignment.py`): larmorx's corrected series is as well aligned as mcflirt's (temporal SD ratio median 1.000; better in 150, within 1 % in 441, worse in 100). Large motions on small phantoms: a cost with several minima. `docs/findings/mri-hmc-original.md`. Permanent case `mri-hmc/phantom-several-minima` (expected divergence). |
| `mri-hmc` / original | the same, with `-cost normmi` (91 findings) | open | larmorx aligns worse with `normmi` (median ratio 1.033; worse in 62 of 91, 26 above 1.10) and better with `mutualinfo` (0.981). Cause not found by black-box runs (mcflirt's trace prints no cost values; the spec describes the histogram costs in outline only). Needs the spec writer: `normmi` exactly. Permanent case `mri-hmc/normmi-aligns-worse` (status open, expected divergence). |

The five `afni-unifize` / replica findings with signature `original exceeds the memory limit`
are unchanged (not this part's).

**The trial of 2026-10-29** (`oracles/difftest/2026-10-29-cleanroom-trial/`, seed 20261029,
0.5 h, 17,399 cases at larmorx 308b743): 3dAutomask 5,289 cases, 3dTshift 6,352, 3dUnifize
2,596: **no difference** (agree, *notice*, *undefined*, *memory* only); mri-hmc 3,162 cases,
2,138 differ, the same family as the first night's (70 % then, 68 % now): explained, or open
for `normmi` (rows above). Its notices without a visible change are the kinds of the first night
(3dTshift AFNI-01, -03, -05, -07, -16; 3dAutomask AFNI-01; 3dUnifize AFNI-01, -45), AFNI-08's
gone (`one_mode`), and one AFNI-43 with `-q` (`afni-automask:20261029:3425`), fixed in 5130e08
(the notice only where the messages are shown, as the replica) and replaying as agree. Suggested
triage line: `afni-automask` / original, `D13 fix pass` (AFNI-43 with `-q`), status fixed,
"AFNI-43's messages-only notice only where the messages are shown (not with -q), as the replica
(2026-10-29)".

## 2. `docs/findings/upstream-defects.md` (summary table rows; no restricted section touched)

- Row **AFNI-03**, column "larmorx": replace "the GPL replicas of 3dAutomask and 3dUnifize name it where the written header changes (2026-10-25), their clean-room originals not yet" with
  "the GPL replicas of 3dAutomask and 3dUnifize name it where the written header changes (2026-10-25), their clean-room originals too (2026-10-29: in both modes, with AFNI-08 in `reproduce`, in the replica's words)"
- Row **AFNI-08**, column "larmorx": append "; 3dAutomask's masked input and 3dUnifize `-EPI`, both implementations: named where the kept times are all within 3 ms of 0 (in `reproduce`, every header slice code 2) (2026-10-29)"
- Row **AFNI-10**, column "larmorx": append "; AFNI's obliquity report comes from the extension's matrix too: on the data panel's 3dAutomask inputs (3dUnifize's outputs) its angle differs in the 5th digit from the original's (20.646870 against 20.646889 for ds000228), and AFNI on the input without its extension prints the original's (2026-10-29: the parity suite explains these messages by AFNI-10)"
- Row **AFNI-43**, column "larmorx": append "; a negative `-peels` peels once in both modes, as the replica's default (the clean-room original before 2026-10-29 did not peel); named also where only the clustering messages differ and are shown (not with `-q`) (the clean-room original since 2026-10-29)"
- No new defect (AFNI-76 to AFNI-80 and FSL-59 to FSL-63 unused).

## 3. `docs/api/upstream-defects.md` ("What is corrected" table)

- Row **AFNI-43**: replace "`lx.afni.automask(peels=0)`, `larmorx afni 3dAutomask -peels 0`" with "`lx.afni.automask(peels=0)`, `larmorx afni 3dAutomask -peels 0` (a value read as 0; a negative one peels once, as in AFNI)".
- Row **AFNI-03**: replace "`larmorx afni 3dTshift` (header slice timing)" with "`larmorx afni 3dTshift` (header slice timing); 3dAutomask's `-apply_prefix` and 3dUnifize `-EPI` keep the corrected slice fields (both implementations)".

## 4. Generated tables

`python -m larmorx_validation tables` was run in the branch after the records (commit 308b743,
"Generated tables"): README's rows of 3dTshift (280 of 280 compared, 177 bit-identical),
3dAutomask (130 cases) and 3dUnifize (117 cases, 37 of 92 bit-identical, 55 with correctly
rounded maths). `validation/tools.toml`'s hand-written 3dAutomask statement was updated from
its new record: "92 of 106 compared cases that write outputs bit-identical to AFNI (the other
14: inputs carrying AFNI's own header extension, AFNI-10: the data bit-identical, the header
and messages AFNI's for the input without it), fMRIPrep's call on every oracle run's BOLD
reference included, the data panel's too". `tables --check` passes. Re-run it at the merge if
the records change.

## 5. `docs/validation/golden.md`

No change: the golden suite passes unchanged (155 of 155; no checksum moved).

## 6. `docs/next-steps.md`

- In "The order from here", item 2: replace the text with
  "2. **The clean-room fixes: done** (2026-10-29): 12 of the 13 differences random testing and the data panel found in the clean-room originals of 3dTshift, 3dAutomask, 3dUnifize and the head-motion correction fixed or explained by black-box runs (the quaternion's `1/det` in extended precision, the closest-orientation letters, millisecond slice times, AFNI-07's default steps and qform as the replica's, a float64 −0 kept, the notices of AFNI-03, -08, -43, -07 and -16 in the replica's words; the panel's obliquity angles explained by AFNI-10; the head-motion differences explained on all 782 findings) with 13 permanent cases; one left open: `-cost normmi` aligns worse (below, follow-ups). The thread probe of the earlier list was not in this part's brief."
- In the follow-ups, replace the item "**3dAutomask's clean-room original, the panel's cases** (found 2026-10-28) ..." with
  "- **3dAutomask's clean-room original, the panel's cases: done** (2026-10-29): AFNI reports the obliquity of its own header extension's 7-digit matrix (AFNI-10); AFNI on the inputs without the extension prints the original's angle, and the parity suite now explains such messages by AFNI-10 (22 of 22 fMRIPrep cases pass)."
- Add a follow-up:
  "- **The head-motion correction's `-cost normmi`** (found 2026-10-29, `docs/findings/mri-hmc-original.md`): on 91 random phantoms larmorx's corrected series is less aligned than mcflirt's (median temporal-SD ratio 1.033; 26 above 1.10), while `mutualinfo` is better (0.981). The spec describes the histogram costs in outline only (its open question 4): the spec writer describes `normmi` exactly (histogram, fuzzy binning, entropy correction, the normalised form), then a clean-room fix measured with `validation/investigations/cleanroom-fixes/hmc_alignment.py`. Permanent case `mri-hmc/normmi-aligns-worse` (open)."

## 7. `CLAUDE.md` ("Where things are", Next)

Replace "the clean-room fixes (antsAI and antsAffineInitializer's memory and speed: done, 2026-10-28)" with "(antsAI and antsAffineInitializer's memory and speed: done, 2026-10-28; the clean-room fixes: done, 2026-10-29, `-cost normmi` left open)".

## 8. The site (`larmorx_documentation`)

Not touched (no site evidence work in this part). Sentences my fixes may make false, to check
at the merge: any statement on the 3dAutomask page that the original prints `[from ?]` or that
its panel cases fail on the obliquity message; on the head-motion page, any statement that the
random-testing differences are unexplained (now: explained but for `normmi`, open).
