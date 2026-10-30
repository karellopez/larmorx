## Report: the clean-room additions (FSL-20/21/65/66 in `lx.mri.hmc`; AFNI-75's guard in 3dUnifize's original), 2026-10-30

Branch `worktree-agent-a91cbd5bb2b9e5d0e`, 14 commits on d2453b3, head f2ed13e; the handoff is
`handoff.md` here.

### What each correction does now
- **FSL-20 and FSL-65** (spec §10.6): both modes write in mcflirt's order. By default every text
  output is opened (the `.mat` directory made) before anything is written, and what that step made
  is removed if a later part fails; an uncreatable `.mat` directory is the spec's error naming
  FSL-20 (exit 1, nothing written; also with `-rmsrel`/`-rmsabs` without `-mats`, FSL-13 named
  too); an unopenable `.par` or `.rms` an error naming the file and FSL-65; a parent refusing
  permission a plain error. `reproduce`: FSL-20 mcflirt's message and exit 0; permission refused:
  mcflirt's message, the run without matrices, its two `terminate` lines, exit 1; FSL-65 `.rms` not
  written and named; FSL-65 `.par`: `MAT_0000` and the `.rms` files, then mcflirt's message, exit 0.
- **FSL-21 and FSL-66** (spec §12.1): refused by default before any work, command line and Python
  API alike (`-stages 0` not refused; `bins` now `i64`). `reproduce`: `mutualinfo`, `normmi` and
  `woods` run appendix B with no bins; where mcflirt crashes (corratio with no bins; −1 or fewer bins
  in a stage that runs) mcflirt's own lines, the defect named, exit 1, nothing written; `-bins` −1
  to −3 with `normcorr`/`leastsquares` run as mcflirt (FSL-66 named). FSL-65 and FSL-66 added to
  `larmorx_core::defects`.
- **AFNI-75** (spec §9.1): the guard right after the radius line, one-volume inputs included,
  decided before the thread pool; the spec's prediction and limit order; its words byte-identical
  to the replica's on the spec's cases (2.834e4 GiB at radius 6100.0; 7.651e5 GiB at 18300.0; 6.12
  GiB at 366.0); exit 1, nothing written. `reproduce` has no guard and keeps only the ball's lines
  that reach the grid (the same values; 3 µm voxels in 0.01 s and 5 MiB). 0.3 mm and 3 mm voxels:
  AFNI's output bit for bit in both modes. `difftest/runner.py` `GUARDED` names the original too.

### Cases, records, golden, trial
- mri-hmc: 33 `failures/` cases from §14.1 (`parity/mri_hmc_failures.py`, mcflirt 6.0.7.23 live):
  24 pass, 9 rejected by both; `bins8-mutualinfo-s3`'s matrices reported, not required (one bin in
  stage 1: the cost is rounding noise; from 16 bins on within 0.07 mm). afni-unifize: 6
  `guard/epi-*` cases and the permanent case `epi-ball-beyond-any-machine-original`, all pass.
- Records (standard, from the clean 7c9df9d): mri-hmc 169 cases (145 pass, 20 rejected by both, 3
  expected, 1 fail: the known `real/ds003345/default`), afni-unifize 124 (none failing);
  compare_records: no status change, only the new cases (the mri-hmc record names the recording
  `mcflirt-haswell`).
- Golden: 3 new cases (`cli.mri.hmc.unwritable`, `cli.mri.hmc.bins`, `cli.afni.3dUnifize.guard`)
  from the clean bbe680b; the other 154 unchanged; `pytest tests/parity` 2,593 passed, 1 xfailed.
- Trial (0.5 h, larmorx 6f64291): afni-unifize 1,768 cases, 0 differ, 9 guard (both), 1 notice,
  1 undefined; mri-hmc 2,411 cases, 1,528 beyond the thresholds (the several-minima class, median
  alignment ratio 1.000, 5–95 % 0.929–1.047); one notice-only case left open
  (`mri-hmc:20261030:1778`, the class of `20261029:45`). Two earlier attempts kept and named
  (`...-aborted-so-replaced`, `...-exported-tree-wrong-afni`).

### Gates
The three Rust workspaces (fmt, clippy, tests), Python 631 passed, ruff, SPDX, `tables --check`.

### Clean-room statement (summary; the full list in the agent's report)
Opened: the brief, CLAUDE.md, the porting guide's §1 and clean-room section, the two specs' marked
sections, the register's header and summary rows, the API pages, the clean-room findings and
records, next-steps' two lines, the orchestration scripts, the clean-room crates and the shared
code, the harness modules named in the brief and a few more of its mechanics, the clean-room
permanent cases, one recorded run's `run.json`. Ran as black boxes: mcflirt 6.0.7.23, AFNI 26.3.05's
3dUnifize, the replica program. Never opened: AFNI, FSL or glibc source, the replica crates, the
restricted findings, `docs/parts/`, the investigation folders and version files the brief names.

**Two exposures, reported by the agent:** (1) one `grep` over the whole register printed single
lines of the † sections of FSL-20, -21, -65, -66 and AFNI-75 (each line restating behaviour the spec
already gives, by its report); (2) it read the replica authors' permanent case
`epi-ball-beyond-any-machine.json` (an input recipe and the expected guard verdict: black-box
data). **The orchestrator's assessment:** no code and no routine content beyond the spec reached the
implementer through these, and the implementation follows the spec; no rework. The porting guide
§1 now says that a clean-room implementer never searches a file with restricted sections as a whole
and never reads the replica authors' permanent cases.

### At the merge (orchestrator)
The handoff integrated: the register's five rows and the D13 gaps list, the public defect page, next-steps,
triage (the open FSL-01 notice entry extended with `mri-hmc:20261030:1778`), golden.md (157 cases,
910 outputs; the coverage row), `validation/tools.toml` (the mri-hmc reference: FSL 6.0.7.23 on the
Haswell kernels, no longer "until aligned") and the generated tables; the porting guide's search rule.
