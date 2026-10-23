# Final report: AFNI 3dAutomask and 3dUnifize

Agent `af5a27ad72e66b922`, 2026-10-23. The agent's report to the orchestrator, verbatim.

## 3dAutomask and 3dUnifize: replicas, specs, records and evidence pages are done

Both replicas match AFNI 25.2.09 bit for bit in every compared case, fMRIPrep's own outputs included. Every gate passes and both trees are clean. Nothing was pushed or merged, and nothing needed approval.

**HANDOFF:** `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/afni-mask-unifize/HANDOFF.md` has the exact text for every shared document: licensing.md, the register (table rows, counts, history, full restricted entries AFNI-40 to -49), docs/api/upstream-defects.md, README, the validation index, golden.md, next-steps (with 4 new **decide** items), CLAUDE.md, and the site's status.yml, site.yml, validation.md and summary.json. It ends with merge notes.

**Licence finding:** both programs need a GPL replica plus a clean-room original.
- Their own files are NIH public domain (`3dAutomask.c`, `3dUnifize.c`, `thd_automask.c`, `thd_cliplevel.c`, `mri_invert.c`, `thd_dset_nbhd.c`, `thd_median.c`, `thd_dsetto3D.c`, `mri_get_cmass.c`).
- Their results depend on MCW (GPL-2) routines:
  - both programs: `THD_extract_array`, `mri_to_float`/`mri_scale_to_float`, `mri_max`, `mri_cut_3D`, `thd_floatscan`, `THD_need_brick_factor`, `mri_new_*`;
  - 3dAutomask: `THD_countmask`;
  - 3dUnifize: `MCW_spheremask`, `qsort_float`, `qmed_float`.
- So both replicas live in `crates-gpl/larmorx-gpl-afni`, with a port of glibc 2.35's `tanhf` and `cbrtf`, checked equal to glibc on all 2^32 float inputs.
- As spec writer I wrote `specs/3dAutomask.md` and `specs/3dUnifize.md`. The clean-room originals are not written.

**What was built (larmorx branch `worktree-agent-af5a27ad72e66b922`, 20 commits from `06fec8f` to `300fcd8`):**
- **Replicas:** every option, AFNI's global options and environment variables, every message, exit status and failure case, with D13 switches.
- **Command line and Python:**
  - `larmorx afni 3dAutomask` and `larmorx afni 3dUnifize` run the replica. There is no original yet, so asking for one is a clear error (exit 127).
  - `lx.afni.automask`, `lx.afni.unifize` and `fmriprep_unifize_options` have typed results.
- **Defect switches:** new switches for AFNI-43, -45 and -47. The shared AFNI defects are now labelled "AFNI" instead of "3dTshift".
- **Oracle:** `build_afni_oracle.sh --add` builds the two programs; BUILD_INFO is updated.
- **Docs:** parity suites, pytest wrappers, API pages, restricted findings, PROVENANCE and NOTICE.
- **Evidence:** two evidence modules. The viewer now draws masks over the image they were made from.

**Parity numbers:**

| | afni-automask (from `0cd5ea2`) | afni-unifize (from `ee419c4`) |
|---|---|---|
| Cases (standard tier) | 105: 85 pass, 19 rejected by both, 1 expected divergence, 0 fail | 97: 76 pass, 19 rejected by both, 2 expected divergences, 0 fail |
| Cases with outputs, bit-identical | 81 of 81 | 72 of 72 |
| Messages identical | 98 of 98 | 93 of 93 |
| fMRIPrep's own outputs (every oracle run) | 7 of 7 | 7 of 7 |
| AFNI at 1 / 6 threads / fMRIPrep's build give the same bytes | 10/10, 7/7, 10/10 | 7/7, 9/9, 9/9 |
| Python API | 20 of 20 | 20 of 20 |

- AFNI's own variability is zero: thread count and build change nothing.
- The full tier's own 10 cases were run once (not a record), all bit-identical: 3 more BOLD series for 3dAutomask; for 3dUnifize, 4 more T1w (one MP2RAGE), `-GM`, `-EPI` on a multiband series and `-nodouble`.
- The parity runs used `OMP_NUM_THREADS=2`, with 1 and 6 added as the band.

**Golden:** 16 cases in crates-gpl (the 3dTshift checksums are unchanged), plus `py.afni.fmriprep_unifize_options` in golden.json. It was recorded after the parity smoke passed (2159 tests); every other checksum is unchanged.

**Findings and register entries:**
- Corrected by default:
  - AFNI-43: `-peels 0` peels once; 343 mask voxels change on fMRIPrep's BOLD reference.
  - AFNI-45: `-T2up 100` reads one past the sorted values; AFNI's output is all zeros.
  - AFNI-47: AFNI reads past its half-size scale image. In our runs AFNI crashed there, so that case is an expected divergence.
- Refused in both modes: AFNI-40 (a missing option value crashes AFNI) and AFNI-46.
- AFNI-49 is a help-text error.
- AFNI-01 now also covers 3dAutomask's series average and 3dUnifize's `-EPI` median.
- **To decide:** AFNI-41, -42, -44 and -48. AFNI-44 matters most: 3dAutomask's large-hole filling depends only on the slice count, which shapes fMRIPrep's BOLD masks.

**Site (clone in `<scratch>/site`, branch `afni-mask-unifize-evidence`, one squashed commit `42a09b0`, no-reply identity):**
- Two pages: `tools/afni-automask` and `tools/afni-unifize`. Shared site documents are untouched.
- 3dAutomask's examples: fMRIPrep's second skull strip of a real ds005040 BOLD reference, the raw BOLD series, and AFNI-43 (a d13 verdict).
- 3dUnifize's examples: fMRIPrep's call at 1 and 6 threads beside fMRIPrep's own output, a ds000005 T1w with its scale image, and AFNI-45 (a d13 verdict).
- Every other comparison is bit-identical, or a reference row with no verdict.
- Checks:
  - the build has 0 problems;
  - number check 51/51 and 50/50, and 90/90 on the Validation summary in a preview copy with the HANDOFF entries applied;
  - the viewer check finds 0 mismatches;
  - re-exporting changes nothing;
  - the evidence adds 0.4 MB and 6.4 MB.
- I reviewed every screenshot. The final set (desktop 1440 and phone 390, light and dark) is in `<scratch>/shots/`.

**Gates:** both Rust workspaces pass fmt, clippy `-D warnings` and tests; ruff, the SPDX check and pytest (439 tests) pass; `tests/parity` passed inside the golden recording.

**For the merge:**
- `evidence/ants_atropos.py` has a display-window bug, not fixed: its two examples share one outputs list, so the first example's default windows are the priors example's. No number or verdict changes.
- AFNI-18 is not named in reproduce mode. This is code shared with 3dTshift.
- The machine was shared and heavily loaded, so I made no full-tier records.

Scratch directory: `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/afni-mask-unifize/`
