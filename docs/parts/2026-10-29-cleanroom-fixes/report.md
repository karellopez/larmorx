## Clean-room fixes (2026-10-29): report

**Where it is:** branch `worktree-agent-ad07925a8f194e151`, final commit **bdd85b6**, base 8318394, 11 commits. Nothing pushed or merged. The handoff text for every shared document is in `<scratch>/cleanroom-fixes/HANDOFF.md`, with `NOTES.md` beside it.

**Outcome:** 11 of the 13 items are fixed, one more (item 9) is explained, and item 13 is explained for every cost except `-cost normmi`, which stays open.

### The 13 items
1. **3dTshift, default output on metre/micrometre grids: fixed.** The replica, run as a black box, gives the shared default:
   - the voxel steps come from the file's matrix and are then converted, so a 1 m column is a 1000 mm step, not 999.99994;
   - a qform-only matrix is computed with the voxel sizes and offsets already in millimetres, in double, and rounded once (100 of 100 headers bit for bit).
   - The AFNI-07 and AFNI-16 notices now use the replica's words and order.
   - All 83 replica findings now replay as agree.
2. **3dTshift, `toffset`: fixed.** For headers in ms or µs, the step in seconds stays a double and each slice time is a float32 running sum of it. Every slice time of 16 headers, read back with `-slice k`, matches.
3. **3dAutomask, header bytes on tiny oblique images: fixed.** AFNI takes `1/det` in the polar decomposition's inverse in x87 extended precision. This matched 1,200 of 1,200 quaternions read as doubles from NIfTI-2 outputs; the old double computation matched 1,100.
4. **One-slice oblique constant image, mask data with `-SI`: fixed** by item 5's orientation, which `-SI` also uses.
5. **`[from ?]`: fixed.** The orientation letters come from the NIfTI library's closest-orientation method, computed in double. It matched 1,604 of 1,604 matrices; the old "largest entry per column" rule missed 396. I read this method in ITK 5.4.5's public-domain `nifti1_io.c` (listed in PROVENANCE).
6. **3dAutomask, AFNI-03 notice: fixed.** AFNI-03 is named in both modes and AFNI-08 in reproduce, in the replica's words. The masks harness now accepts AFNI-08 in one mode only for header slice code 2 (a new `one_mode` field).
7. **AFNI-43 notice: fixed.** It now uses the replica's words, including the case where only the clustering messages differ. It is not named with `-q`; that case came up in the trial and is fixed in 5130e08. A negative `-peels` now peels once in both modes, as the replica's default does. The original used to skip peeling, so this is a disagreement with the replica's default that I found along the way.
8. **Sign of zero: fixed.** int8, uint16, int32, uint32 and float64 data without a usable slope are now only converted, so a float64 −0 stays −0.
9. **Panel obliquity angles: explained (AFNI-10).** AFNI reports the obliquity of its own header extension's 7-digit matrix; run on the file without the extension, it prints larmorx's 20.646889. The original keeps ignoring the extension. The parity runner's AFNI-10 explainer now also covers messages, and all 22 fMRIPrep 3dAutomask cases pass, the 5 panel cases included. I checked 3dTshift and 3dUnifize: their inputs carry no extension.
10. **3dUnifize, AFNI-03 notice: fixed** (same change as item 6). The header-bytes part is item 12.
11. **3dUnifize, AFNI-08 in the wrong mode: fixed.** The notices now match the replica's in both modes.
12. **3dUnifize, header bytes on tiny oblique images: fixed** by item 3.
13. **Head-motion correction: explained except `normmi`, which stays open.** I measured all 782 findings, not a sample (`hmc_alignment.py`), comparing the corrected series' temporal standard deviation (larmorx ÷ mcflirt):

| Subset | Findings | Median ratio | larmorx better / within 1 % / worse |
|---|---|---|---|
| All costs but `normmi` | 691 | 1.000 | 150 / 441 / 100 |
| `-cost normmi` | 91 | 1.033 (26 above 1.10) | 12 / 17 / 62 |
| `-cost mutualinfo` | 78 | 0.981 | 45 / 13 / 20 |

   - For every cost but `normmi` this is explained: a cost with several minima on small phantoms with large motion. Permanent case `phantom-several-minima`, expected divergence.
   - For `normmi` black-box runs could not find the cause: mcflirt's verbose trace prints no cost values, and the spec gives the histogram costs only in outline (its open question 4). I did not guess a fix. Permanent case `normmi-aligns-worse` has status open. The next step needs the spec writer to describe `normmi` exactly.

### Spec lines added (observed)
- **`specs/3dTshift.md`:**
  - §3.2: millisecond slice times;
  - §6: the float64 −0;
  - §6.1: the extended-precision `1/det`; AFNI-07's shared default (steps, qform); the notices' words and order;
  - §7.1: the obliquity report reads AFNI's extension.
- **`specs/3dAutomask.md`:**
  - §6.2: the orientation method;
  - §9: AFNI-43 (negative values, the messages-only case, `-q`) and AFNI-03/08 for outputs that keep the time axis.
- **`specs/3dUnifize.md`** §9: `-EPI` follows the same rules.

### Validation
- **compare_records:** no status or detail change in any suite.

| Suite | Re-made from | Cases | New cases |
|---|---|---|---|
| afni-tshift | e82f16e | 315 | 1, passes |
| afni-automask | 5130e08 | 130 | 6 permanent + 15 panel fMRIPrep cases, all pass |
| afni-unifize | e82f16e | 117 | 3 permanent + 15 panel cases, all pass |
| mri-hmc | e82f16e | 136 | 2 expected divergences |

   The one mri-hmc failure, `real/ds003345/default` (FD correlation), was already in the committed record.
- **Golden:** 155 of 155 pass with no checksum change, so nothing was re-recorded.
- **Smoke tier** (`pytest tests/parity`, at 308b743): 2,562 passed. The only later code change is the `-q` fix, and the 3dAutomask standard record re-made after it is identical.
- **Random-testing trial** (0.5 h, 17,399 cases):
  - 3dAutomask, 3dTshift and 3dUnifize: no differences. The notices without a visible change are the first night's kinds; the AFNI-08 ones are gone.
  - mri-hmc: 2,138 of 3,162 differ, the same family as the first night (68 % now, 70 % then). No new difference.
  - Output in `oracles/difftest/2026-10-29-cleanroom-trial/`.
- **Gates:** Rust fmt, clippy and tests pass in all three workspaces; ruff, ruff format and SPDX pass; Python tests 623 pass; `tables --check` passes.

### Documents
- **My own files:** both PROVENANCE files; the 3dAutomask, 3dUnifize and mri-hmc API pages; a new section in `docs/findings/afni-masks-original.md`; new `docs/findings/mri-hmc-original.md`; 13 permanent cases; `validation/investigations/cleanroom-fixes/` (README and probes).
- **Harness changes, minimal:** the AFNI-10 messages explainer and `one_mode` in `parity/afni_replica.py` and `difftest/tools/afni_masks.py`; mri-hmc permanent cases can now expect a divergence (`difftest/tools/mri_hmc.py`).
- **Shared files I changed in the branch:** I ran `tables`, which regenerated README.md, and I updated the hand-written 3dAutomask line in `validation/tools.toml` (commit 308b743), because `tables --check` is a gate.
- **Everything else shared goes through HANDOFF.md:** the triage entries by tool and signature, four register rows (AFNI-03, -08, -10, -43), two public defect-page rows, next-steps (item 2 done, the panel follow-up done, a new `normmi` follow-up), CLAUDE.md, and site sentences to check. No new defect IDs were used.

### Clean-room statement
- **Opened:**
  - BRIEF.md, CLAUDE.md, porting guide §1 and §8, the orchestration README and scripts, difftest.md;
  - the three AFNI specs (3dTshift §3.2, §6–6.1, §7.1; 3dAutomask in full; 3dUnifize §1–3 and §6–10) and §A/§13 of mcflirt.md;
  - both PROVENANCE files, afni-masks-original.md, `docs/api/upstream-defects.md`, and the register's header and summary table only;
  - larmorx's own Apache code (`crates/larmorx-afni`, `larmorx-mri` cost.rs);
  - the harness mechanics (difftest gen, inputs, permanent and core; the descriptors' run code; afni_replica.py; parts of mri_hmc.py);
  - the 2026-10-25 finding JSONs;
  - ITK 5.4.5's public-domain `nifti1_io.c` (quatern, inverse, determ, norms, polar, orientation);
  - docs/next-steps.md sections and the records I re-made.
- **Ran:** AFNI 25.2.09's 3dAutomask and 3dTshift; FSL 6.0.7.17's mcflirt; the replica programs `larmorx-gpl` and `larmorx-nc` (built only, never opened); larmorx.
- **Never opened:** AFNI's or FSL's source, `crates-gpl/` or `crates-nc/` code, the restricted findings or register sections, `docs/parts/`, the licensing routine table, `triage.toml`, or the replica authors' permanent cases.

### Not done or left open
- **`-cost normmi`** (above).
- **The "thread probe"** in next-steps item 2 was not in this part's brief, so I did not touch it.
- **The site:** no evidence work, per the brief.

### At the merge (orchestrator)
- The triage entries applied by tool and signature; the two head-motion entries stay `open` (each groups `normmi` and the other costs under the same failed checks), with the measured explanation and the open `normmi` part in their text.
- The thread probe needed nothing in the clean-room crates (`docs/findings/ants-cli.md`).
- The site's random-testing panels read only nights named by their date (`evidence/difftest.py`, `latest_night`), so a part's trial such as `2026-10-29-cleanroom-trial` never replaces the full night.
