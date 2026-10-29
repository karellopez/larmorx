## Report: the clean-room originals aligned with the newest releases (next-steps item 3b, 2026-10-29)

The clean-room originals follow AFNI 26.3.05 (3dTshift, 3dAutomask, 3dUnifize) and FSL 6.0.7.23's
mcflirt (the head-motion correction); `-cost mutualinfo` and `normmi` are computed exactly as
`specs/mcflirt.md` appendix B describes them. Branch `worktree-agent-a2b31330feb9bf385`, 11 commits
from fa943b1 to c0e9e3f; the handoff is `handoff.md` here.

### What changed
**3dTshift, 3dAutomask, 3dUnifize (AFNI 26.3.05):** `-no_detrend` removes and restores every
voxel's mean in both modes; header slice code 2 runs from `slice_end` down to `slice_start` in
both modes (3dTshift, 3dAutomask `-apply_prefix`, 3dUnifize `-EPI`); AFNI-03 and AFNI-05 never
named; AFNI-08 still named where the kept slice times are all within 3 ms of 0; the three
slice-code-2 permanent cases re-described.

| Tool | Cases | Pass | Rejected by both | Expected divergence | Fail |
|---|---|---|---|---|---|
| 3dTshift | 315 | 280 | 32 | 3 | 0 |
| 3dAutomask | 130 | 110 | 19 | 1 | 0 |
| 3dUnifize | 117 | 96 | 19 | 2 | 0 |

3dTshift: 178 of the 280 passing cases bit-identical. compare_records against the previous
records: no status change; details differ only by the new "AFNI's own step" checks. The 6 + 1 + 2
cases that failed against 26.3.05 before now pass.

**Head-motion correction (FSL 6.0.7.23):** FSL-59 (the default keeps `-sinc_final`/`-nn_final`,
`reproduce` resamples trilinearly, named where the series differ; with an integer output FSL-09
named in both modes from mcflirt's trilinear series); all 16 `FSLOUTPUTTYPE` values with zstd and
bzip2 (pure-Rust `ruzstd` 0.9.1 and `bzip2` 0.6.1; only the root `Cargo.lock` changed); files read
by content, names completed in FSL's order with FSL's error messages, an unknown `FSLOUTPUTTYPE`
refused with FSL's message; FSL-60 (the name given by default, FSL's renamed header with
`reproduce`; named); the in-plane mode's centre of rotation is the padded volume's (settled by
black-box `-verbose 20` traces). Record against 6.0.7.23: 136 cases, 121 pass, 11 rejected by
both, 3 expected divergences, 1 fail (the known `real/ds003345/default`, FD r = 0.9885).

On ds000005 with the histogram costs: `mutualinfo` 95th-percentile matrix deviation 0.049 → 0.0048
mm, FD r 0.64 → 0.98; `normmi` 0.065 → 0.011 mm, FD r 0.54 → 0.95.

### `normmi` and `mutualinfo`, before and after
On the first long random-testing run (2026-10-25), larmorx in `reproduce` against mcflirt 6.0.7.23
(ratio of temporal SDs, lower is better aligned):

| Cost | Build | Median | Within 1 % | Above 1.10 |
|---|---|---|---|---|
| `normmi` (91 findings) | before | 1.051 | 13 | 30 |
| `normmi` (91 findings) | after | 1.000 | 55 | 3 |
| `mutualinfo` (78 findings) | before | 0.983 | 11 | – |
| `mutualinfo` (78 findings) | after | 1.000 | 54 | – |

With `-fudge -stages 1`, mcflirt's `-verbose 20` trace matches larmorx's evaluated matrices on every
evaluation in 22 of 30 random phantoms and on the synthetic series (429 of 429, 422 of 422); the
other 8 agree for 19 to 224 evaluations, then differ in the 5th to 6th decimal, as `normcorr` does
(spec open question 1). The permanent case `mri-hmc/normmi-aligns-worse`: fixed (still an expected
divergence, other minima; larmorx now aligns that phantom better than mcflirt, 0.93, was 1.38).

### Golden, memory, threads, speed
Golden re-recorded from bb36e15: five cases changed (the two 3dTshift `reproduce` cases,
`py.mri.hmc.costs`, the in-plane cases of `py.mri.hmc.options` and `py.mri.hmc.reproduce`), one
added (`cli.mri.hmc.compressed`). Parity smoke tier (both version variables set): 2,581 passed,
with the memory and thread checks (three head-motion programs added to the thread check). Speed on
ds000005: `normmi` 49.1 → 39.1 s (1 thread), 8.2 → 6.4 s (8 threads), `mutualinfo` alike; the
changed options' default paths unchanged; `reproduce` paths now doing what the newest original does:
`-nn_final` 15.9 → 17.0 s, `-sinc_final` 26.7 → 17.0 s, 3dTshift on a slice-code-2 header 0.07 →
0.45 s.

### Random-testing trial (30 min, 15,264 cases, against AFNI 26.3.05 and FSL 6.0.7.23)
AFNI tools: no differences (190 findings and 62 notices before the alignment); the known categories
remain (45 notices with no visible change, 33 AFNI-20, 11 3dUnifize memory cases awaiting AFNI-75's
guard). Head-motion correction: 1,732 phantoms beyond the thresholds, the several-minima class,
as well aligned as mcflirt for every cost (medians 0.999 to 1.001; `normmi` 1.085 before). One
case reported, not fixed: `mri-hmc:20261029:45` names FSL-01 in `reproduce` only (its interplay with
the FSL-03 correction on a thin slab; predates this part).

### Site
Branch `cleanroom-align-newest` (e7dbde2): the four tool pages re-exported, a comparison row of the
clean-room original against AFNI 26.3.05 or mcflirt 6.0.7.23 in each example (8 rows), prose
updated, `tables --site` and the summary regenerated. Inventory: no loss on all five pages. Number
check before → after: 3dTshift 158 → 174, 3dAutomask 153 → 156, 3dUnifize 179 → 199, head motion
238 → 251, Validation 362/362. Build 0 problems; blobs unchanged.

### Gates
Rust (three workspaces), Python (629 passed, ruff, SPDX), `tables --check`: all pass.

### Clean-room statement
**Opened:** the brief; CLAUDE.md; porting guide §1, the speed section and §10; the orchestration
README and scripts; `specs/3dTshift.md` (§3.2, §4, §6.4, §6.5), `3dAutomask.md` §9,
`3dUnifize.md` §9, `mcflirt.md` (§4.1, §9 to §13, §17, appendices A and B); next-steps item 3b and
the three follow-ups named in the brief; `crates/larmorx-afni`, `crates/larmorx-mri`, and from the
shared crates `larmorx-core` (defects, element, math), `larmorx-io` (nifti), `larmorx-py/mri.rs`;
the four API pages, `docs/api/upstream-defects.md`, the register's summary rows only;
`mri-hmc-original.md`, `afni-masks-original.md`; the four records; `figures.md`; the harness
(`parity/mri_hmc.py`, `thread_pools.py`, `difftest/tools/mri_hmc.py`, `permanent.py`, the evidence
modules `afni_step`, `afni_*`, `mri_hmc`, `cleanroom-fixes/hmc_alignment.py`,
`panel-ants/site_inventory.py`, `record_golden.py`'s docstring, the tests); the clean-room
permanent cases only; the `bzip2` and `ruzstd` crate sources; the oracles' `run.json` and manifest
files (argv and environment only); the 2026-10-25 findings JSON; the site clone.
**Ran as black boxes:** mcflirt 6.0.7.23 (`fsl_io_probe.py`, `mi_trace.py`,
`hmc_alignment_bin.py`); the `larmorx-nc` replica (for notice wording); FSL's `zstd` program as a
compressor; AFNI 26.3.05 and 25.2.09 and the GPL replica through the suites and random testing;
`crates-gpl` and `crates-nc` compiled by the gates only.
**Never opened:** AFNI or FSL source, `crates-gpl/`, `crates-nc/`, the restricted findings,
`docs/parts/`, `version-afni/`, `version-fsl/`, `triage.toml`, `afni_versions.py`,
`fsl_versions.py`, `fsl_mcflirt_replica.py`, `fsl_step.py`, `validation/probes/fsl-mcflirt-affine-trace`.
(`validation/investigations/cleanroom-align/trace.patch` traces larmorx's own evaluated matrices:
a patch to the clean-room crate, applied for a local build only.)

### At the merge (orchestrator)
- `DEFAULT["original"]` set to the newest release in `parity/afni_versions.py` (26.3.05) and
  `fsl_versions.py` (6.0.7.23), their comments updated.
- The handoff integrated: the register's summary and D13-gap rows, the public defect page,
  next-steps (3b done; the three follow-ups removed; two added, the spec writer's §4.1/§10.1 notes
  in the first), CLAUDE.md (AFNI and FSL rows, rule 2's version lines, Next), `UPSTREAM.md`,
  golden.md (counts 154 cases, 895 outputs).
- Triage by tool, implementation and checks: the four AFNI entries of the AFNI version update and
  FSL-59's head-motion entry fixed; the two earlier head-motion entries (the several-minima class
  with `normmi`) given a new status, **explained** (an expected difference, measured and explained:
  no mistake and no defect; documented in `docs/validation/difftest.md` and labelled on the site);
  one new open entry, FSL-01's `reproduce`-only notice.
