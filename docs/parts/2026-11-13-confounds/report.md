# The confounds' part: final report (relay 5, agent a70923b6fb1ce4ef1)

The part's final report (2026-11-13). The part is ready to merge: branch `worktree-agent-abafb802f99529ccb`,
clean at **`afbf9787`**, 94 commits not in main (245 since `7cd35c8c`; its first half in main since the
intermediate merge `89030879`). No site branch (the export only: the documentation revision's writer makes the
page). Handoff: `HANDOFF.toml` (106 operations, 18 files) and `HANDOFF-site.toml` (7 operations, 2 files), reasons
in `HANDOFF.md`, all checked on a trial merge with main `bd71286b`.

## The port (complete)
- **fMRIPrep's confound estimation**, bit for bit: `larmorx.pipelines.larmorprepx.confounds.bold_confounds`
  (fMRIPrep 25.2.7's `init_bold_confs_wf` and `ds_confounds`; 25.2.6's releases on request) on `lx.confounds`
  (nipype 1.12.0's `algorithms/confounds.py` complete: CompCor with its pre-filters and SVD fallback, DVARS with
  nitime's AR(1) and SciPy's FFTs, TSNR, the non-steady-state detector, framewise displacement with nipy 0.6.1's
  `NIPY` source; fMRIPrep's confound interfaces: the masks, the motion parameters in both releases' forms, the RMS
  deviation, the tables with pandas' arithmetic) and `lx.niworkflows`' confound steps (the model expansion, spike
  regressors, signals, morphology, utilities); `larmorx nipype run` (nipypecli for the 37 interfaces).
- **Under it, in Rust:** OpenBLAS's level 1 to 3 routines on the Haswell kernels and LAPACK 3.12.1's and 3.12.0's
  QR, LQ, bidiagonal, divide-and-conquer SVD, `DGESDD`, `DGELSD`, `DGESVD` paths (numpy 2.5.4, 2.3.5, 2.2.6; SciPy
  1.18.1 and 1.15's own OpenBLAS), SciPy's single-precision FFTs (pocketfft, ducc0), numpy's reductions at the call
  sites, CORE-MATH's `hypot`.
- **Records** (all pass; none rejected by both; no expected divergence; `experimental`): `confounds-linalg` 32 of 32
  (1,673 checks), `confounds-linalg-full` 12 of 12 (840), `confounds-nipype` 25 of 25 (588), `confounds-nipype-full`
  3 of 3 (90: fMRIPrep's 108 real CompCor nodes), `confounds-fmriprep` 16 of 16 (990).
- **fMRIPrep's own files:** the 36 confounds workflows of 21 oracle runs run end to end with fMRIPrep 25.2.6's
  libraries under `reproduce`: all 36 `desc-confounds_timeseries.tsv` identical column by column (25 directly, 11
  once glibc's `atan2` cells, D4, are taken from the run's own motion table) and all 36 JSON byte for byte; every
  other node file byte for byte (970, and 124 of the motion chain). The step to the newest releases changes
  263,181 cells and 6 JSON values; the default (corrections) against `reproduce` 218,474 cells (PD-01's
  correctly rounded reads, GLIBC-07's CompCor columns), 4 JSON values and PIPE-41's five entries.

## Defects
49 entries, PIPE-41 to PIPE-83, PD-01 to PD-03 (a new family), NP-09, NP-10, SCI-41: rule A 27 (PIPE-47 measured
first), rule B 12, undefined conversions clamped 2 (PIPE-74, -76), PIPE-43 one order (nondeterminism), rule C 3
(PIPE-48, -49, -83), rule D open 1 (PIPE-79: fMRIPrep runs with FreeSurfer needed), PIPE-42 with the reports, NP-09
(a crash) and NP-10 (a hang) never reproduced; 41 with `reproduce`. GLIBC-07, NP-04, PIPE-08 and PIPE-23 extended.
A larmorx constant corrected: `DBDSQR`'s `TOLMUL` (the field maps' code); every record it reaches re-made, no
result changed. Unused: PIPE-84..90, PD-04..10, NP-11..20, SCI-42..55.

## Golden, memory, random testing, speed
- Golden: 16 cases (588 outputs), 334 in all; every earlier checksum unchanged; Windows line ends fixed.
- Memory: 6 calls, larmorx at most 0.96 of the originals' peak (CompCor's two excesses fixed first).
- Random testing: trial 334,124 cases (332,076 agree, 528 notices, 28 differ, 1,492 skipped), every difference
  triaged and fixed (ten permanent cases), checks of the final build 140,003 and 40,567 cases all agree.
- Speed: the 26 Python nodes 1.1 to 1.4 times fMRIPrep's at one thread, 1.5 to 1.6 at eight; the port's own
  `svd`, `lstsq`, `gesvd` 0.22 to 0.42 of numpy's and SciPy's (a follow-up).

## Gates
Rust gates OK in the three workspaces at `a7d3aaa6` (relay 2i; no Rust changed since); Python gates at `6991718a`
(relay 4: 1,526 passed, 13 skipped) and at `afbf9787` (relay 5: 1,526 passed, 13 skipped; CI's environment 992 passed); ruff, SPDX, `tables --check` 0. On
the trial merge: the wheel builds; the doc-reading tests and the golden suite 736 passed, 1 skipped; the site 0
problems, inventory after >= before.

## Open
The site page (the documentation revision), PIPE-79's measurement, `OPENBLAS_L2_SIZE` for everyone, the
`atan2_glibc` micro-part, the speed items, the refused paths, upstream report drafts: next-steps' new block.
Nothing waits for the user.
