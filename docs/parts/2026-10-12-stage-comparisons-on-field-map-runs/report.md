# Final report: Stage comparisons on field-map runs

Agent `acd8a87c00dd1de09`, 2026-10-12. The agent's report to the orchestrator, verbatim.

## Report: queue item 4, stage comparisons on the field-map oracle runs

All four records are made and committed on branch `worktree-agent-acd8a87c00dd1de09`, HEAD `bcd7aa5`, with a clean tree. I have not pushed or merged anything. All four records were generated from the same clean commit, `58230ae`, with `--reuse`. Every difference in every record is explained; none is left unexplained.

### Per run (against fMRIPrep 25.2.6 as it ran)

**ds005040 (PEPOLAR via topup, slice timing)**
- 56 of 65 compared nodes are bit-identical. The other 9 are explained by fMRIPrep's thread count and glibc's maths.
- Head motion passes (FD correlation not required here: largest FD 0.17 mm).
- All four field-map `ResampleSeries` nodes are bit-identical in `reproduce` mode: S02 (unwarp of the boldref), F05, F06, F07.
- The new field-map stages: S01 (the field-map N4) and S03 (the field-map report's antsApplyTransforms) are compared too.

**ds005454 (phasediff, two T1w, 280 volumes)**
- 55 of 68 bit-identical; the other 13 are explained by threads and glibc's maths.
- The pre-template N4 (A02) is now covered for the first time.

**ds005752**
- 65 of 78 bit-identical; the other 13 are explained by threads and glibc's maths.
- **Its field map is never used by fMRIPrep.** The files carry no `IntendedFor`, `B0FieldIdentifier` or `B0FieldSource`, so fMRIPrep drops the estimator (`base.py:995-1050` in 25.2.6). The direct-`fieldmap` path is therefore still not covered. A copy of ds005752 with `IntendedFor` added would cover it.
- Head motion on the 200-volume run: FD correlation 0.974, below the 0.99 threshold (largest FD 0.56 mm). It is within mcflirt's own band on that run, 0.961 to 0.976, which the harness now measures.

**ds000005 (regenerated)**
- 75 of 92 bit-identical, up from 61.
- All 14 `--float 1` outputs are now bit-identical.
- With the GCC 14.3 ANTs oracle, the ANTs build no longer explains any difference; only threads and glibc's maths remain.
- `fmriprep-stages-float.md` is now redundant. I kept it and marked it as history in the validation index rather than deleting it.

### Slice timing (four 3dTshift outputs)
- **GPL replica:** bit-identical to fMRIPrep in 4 of 4, data and header. The one header difference is `vox_offset`, because larmorx writes no AFNI history extension.
- **AFNI references:** the repository's AFNI 25.2.09 build gives the same output, and fMRIPrep's own AFNI reproduces itself on re-run.
- **Clean-room original:** within the `afni-tshift.md` thresholds. It is off by one stored unit in:
  - 189 of 10.3M values (ds005040),
  - 1,808 of 688M values (ds005454),
  - 86 of 39M and 1 of 2M values (ds005752's two runs).
- `fix` mode gives the same files as `reproduce`: no AFNI defect is triggered on this data.

### PIPE-03 on real field maps
**ds005040 (PEPOLAR, Jacobian on)**

| Output grid | Brain voxels changed > 0.1 % | > 1 % | > 5 % | 99th pct | Largest |
|---|---|---|---|---|---|
| native and unwarp_boldref | 0 % (bit-identical) | 0 % | 0 % | – | – |
| `space-T1w` | 78.8 % | 2.8 % | – | 1.4 % | 6.8 % |
| MNI res-2 | 95.6 % | 59.5 % | 5.6 % | 11 % | 56 % |

- The MNI maximum is in the inferior temporal lobes, at (59.5, −18.5, −28.5) mm.
- That changes above 10 % sit mostly below z = −10 mm (94 %) comes from a one-off scratch analysis, not from a record.

**What drives the size of the error.** fMRIPrep's derivative steps over one target voxel. In BOLD voxels that step is:
- 0.57 at 7° in MNI for ds005040, so the correction is too small;
- 1.65 at 5° in MNI for ds005454, so the correction is amplified;
- in `space-T1w`, 1.00 at 3° for ds005040 and 1.00 at 25° for ds005454, whose BOLD is oblique.

**Which factor is right.** I compared both factors with the BOLD-grid factor carried to each output grid:
- ds005040 MNI: fMRIPrep off by 0.012 (median) and 0.10 (99th percentile); larmorx off by 0.005 and 0.063. Part of both residuals is the reference's own error from 3 mm differences.
- ds005454, Jacobian forced on: fMRIPrep off by up to 0.39 (99th percentile); larmorx by 0.007.

**ds005454 as fMRIPrep runs it.** No Jacobian is applied there (PEPOLAR only), so larmorx's default leaves the output unchanged; I checked this. With the Jacobian forced on, 79 % (T1w) and 71 % (MNI) of brain voxels would change by more than 1 %.

**Correction to the earlier synthetic study.** fMRIPrep's `space-T1w` grid is at the BOLD's voxel size, not the 1 mm grid that study used. I added that row to the study.

These numbers are now in `resample-series-jacobian.md`, the PIPE-03 register entry, `api/resample-series.md` and the draft upstream report (still a draft, not filed).

### Harness changes (`validation/.../stages/`, plus tests)
- **F02 pattern fixed:** it now matches `slice_timing_correction`. Slice timing runs in both implementations, in `reproduce` and `fix` modes. Its references are fMRIPrep's AFNI re-run in its image and the AFNI oracle build. Integers are compared in stored units, and headers as stored.
- **Missing inputs restored:** nipype removes 3dTshift's input and `slice_timing.1D` after the run. The harness now reads the input path from the node's `_0x*.json` hash file and rewrites `slice_timing.1D` byte for byte.
- **New stages S01 to S03.**
- **New "missing" entries:** topup, prelude, mri_robust_template, the field-map B-spline fit and evaluation, and sdcflows' Python preparation steps.
- **PIPE-03 fix pass:** added to every field-map `ResampleSeries` node.
- **N4 comparisons:** every run goes through nipype's own `copy_header` in fMRIPrep's image before it is compared (see PIPE-16).
- **mcflirt band:** measured on the run itself whenever the FD correlation applies and larmorx misses it.
- **Bug fixes:**
  - the confounds of runs with a session could not be found;
  - fMRIPrep's `ResampleSeries` re-run failed on nodes without transforms.
- **New module:** `parity/resample_jacobian_oracle.py` (the real-data part of the Jacobian study above).

### Findings and register entries
- **New PIPE-16 (c?, needs your decision for larmorprepx):** nipype's N4 interface with `copy_header=True` writes the corrected image in the input's data type. sdcflows' field-map reference is int16, so fMRIPrep's node output is N4's result re-quantized: up to 0.3 % relative error at the dimmest voxels, median 1.8e-5. Without this step, the harness had counted the rounding as a thread effect. Once the rounding is accounted for, N4 at 8 threads reproduces itself exactly.
- **PIPE-03:** now confirmed on real data.
- **`fmriprep-oracle.md`:** new section on the field-map runs (unused ds005752 field map, Jacobian for PEPOLAR only, how nipype runs 3dTshift, the re-quantization, the `space-T1w` grid, head motion against mcflirt's band).
- **Porting guide:** notes on oracle coverage and on nipype rewriting data and removing files from node directories.
- `overview.md` and the validation index are updated to the new numbers.

### What larmorx tools need
No larmorx library code changed; no comparison exposed a bug in it.
- The field-map path still needs: topup, prelude, `ReconstructFieldmap` and sdcflows' B-spline fit, and antsRegistration (`fmapreg`).
- larmorprepx must decide PIPE-16.
- Optional: ds001600 (four field-map N4s) could also be run through the harness.

### Gates and commits
- **Gates:** `ruff check` and `ruff format --check` pass; `check_spdx` passes; `pytest tests/python tests/golden`: 355 passed; `pytest tests/parity`: 1901 passed. Rust is untouched, so no cargo gates were needed.
- **Commits:**
  - harness: `831da4f`, `885d413`, `31feb12`, `567214d`, `58230ae`;
  - three records: `a0ad741`;
  - docs: `1a44ef6`;
  - test: `1d50c4c`;
  - ds000005 record: `bcd7aa5`.
  No AI attribution in any of them.
- **CLAUDE.md:** not edited; the queue item 4 status line needs updating.

### Housekeeping
- **Stopped runs:** I stopped my own comparison chain three times to fix the harness. Each stop is logged in `oracles/logs/stages-sdc-chain.log`.
- **Archive:** the old ds000005 comparison outputs (GCC 11 references) are in `<workspace>/archive/2026-10-12-stages-ds000005-compare-45564252/`, with a README.
- **Logs:** `oracles/logs/stages-<run>.log`.
