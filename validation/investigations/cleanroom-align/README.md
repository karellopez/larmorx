# The clean-room originals follow the newest releases (2026-10-29): black-box tools

The tools the clean-room implementer of next-steps item 3b used to bring larmorx's clean-room
originals to the newest releases of their originals: 3dTshift, 3dAutomask and 3dUnifize
(`crates/larmorx-afni`) to AFNI 26.3.05, the head-motion correction (`crates/larmorx-mri`) to
FSL 6.0.7.23's mcflirt, with `-cost mutualinfo` and `normmi` from `specs/mcflirt.md` appendix B.
Black-box only: FSL 6.0.7.23's mcflirt, AFNI 26.3.05, larmorx's original and the replica
programs are run and their outputs read; nothing here reads FSL's, AFNI's or the replicas'
source. What they showed is in [docs/findings/mri-hmc-original.md](../../../docs/findings/mri-hmc-original.md)
and [docs/findings/afni-masks-original.md](../../../docs/findings/afni-masks-original.md).

Run them from the repository with `PYTHONPATH=python:validation/src` and `LARMORX_WORKSPACE`
set; mcflirt is `~/fsl-6.0.7.23/bin/mcflirt` (`$FSL_623`), run with a clean environment
(`FSLDIR`, `FSLOUTPUTTYPE`, `PATH=/usr/bin:/bin`), never with FSL on the path.

| Tool | What it shows | Behind |
|---|---|---|
| `fsl_io_probe.py <case> <work> [--who mcflirt,lx,nc] [--mode fix\|reproduce] [--grep TEXT]` | mcflirt 6.0.7.23, larmorx and the replica program side by side: every `FSLOUTPUTTYPE` (the files, their compression, the decompressed header and values), reading zstd, bzip2 and gzip files under other names, the completion of names without an extension and its errors, an unknown type's message, FSL-60's renamed names (`.img` in a directory or a stem; a pair's header renamed, its data not), FSL-59's `-sinc_final` and `-nn_final` | the I/O of `hmc::files` and `hmc::image`, FSL-59, FSL-60, the notices' words |
| `mi_trace.py <work> --lx <traced larmorx> [--cost c] [--case <finding.json>]... [--env K=V]` | mcflirt's `-verbose 20` trace (every evaluated matrix) against larmorx's, with `-fudge -stages 1`, evaluation by evaluation; larmorx's trace needs a local build with [`trace.patch`](trace.patch) | appendix B checked: 22 of 30 random phantoms identical over every evaluation (below); the in-plane centre |
| `trace_many.py <run> <work> <traced larmorx> <cost> <n> [--env K=V]` | `mi_trace.py` over `n` of a random-testing run's findings of one cost | the 30 traces below |
| `hmc_alignment_bin.py <run> <work> <out.jsonl> --before <larmorx> --after <larmorx>`, `--summary <out.jsonl>` | the measure of [`../cleanroom-fixes/hmc_alignment.py`](../cleanroom-fixes/hmc_alignment.py) (the corrected series' temporal standard deviation in the phantom, larmorx / mcflirt) on a random-testing run's `normmi` and `mutualinfo` findings, two builds of the command line against mcflirt 6.0.7.23 | the before/after numbers below; [`results/hmc-alignment-mi-2026-10-25-findings.jsonl`](results/hmc-alignment-mi-2026-10-25-findings.jsonl) |
| `speed.py <work> --before <larmorx> --after <larmorx>` | a short before/after timing of the changed paths on ds000005's first BOLD run | the speed lines below |

## `mutualinfo` and `normmi` (appendix B)

On the 169 `normmi` and `mutualinfo` findings of the first long random-testing run
(2026-10-25; `oracles/difftest/2026-10-25/findings/mri-hmc/`), larmorx in `reproduce` against
mcflirt 6.0.7.23, the ratio of the corrected series' temporal standard deviations (lower is
better aligned):

| Cost | Build | larmorx lower | within 1 % | higher | median (5 % – 95 %) | above 1.10 | below 0.90 |
|---|---|---|---|---|---|---|---|
| `normmi` (91) | before (fa943b1) | 10 | 13 | 68 | 1.051 (0.929 – 1.330) | 30 | 2 |
| `normmi` (91) | after | 17 | 55 | 19 | 1.000 (0.957 – 1.057) | 3 | 3 |
| `mutualinfo` (78) | before (fa943b1) | 43 | 11 | 24 | 0.983 (0.809 – 1.208) | 7 | 15 |
| `mutualinfo` (78) | after | 10 | 54 | 14 | 1.000 (0.919 – 1.046) | 2 | 2 |

(By default, with mcflirt's defects corrected: `normmi` 1.003, `mutualinfo` 1.010, median. The
earlier part measured the same findings against FSL 6.0.7.17 in process: `normmi` 1.033,
`mutualinfo` 0.981.)

`mi_trace.py` on 30 of these phantoms (`trace_many.py`: 15 per cost, every fifth or sixth
finding), `-fudge -stages 1`:
every evaluated matrix identical to mcflirt's (to its printed 6 decimals) in 11 of 15 `normmi`
and 11 of 15 `mutualinfo` traces (the synthetic series of the recorded runs: 429 of 429 and 422
of 422); the other 8 agree for 19 to 224 evaluations and then differ in the 5th or 6th decimal of
a translation, as `normcorr`'s path does (spec §17, open question 1). One thin slab (finding
00298) differed from the first rotation step on: in the in-plane mode the centre of rotation is
the padded volume's, which the spec does not say; with it, 154 of 154 evaluations agree
(`normcorr` on the same series: 39 identical instead of 1).

## Speed (`speed.py`, ds000005's first BOLD run, 64 × 64 × 34 × 240; best of 2, of 5 for 3dTshift)

Seconds, the command line before (fa943b1) and after, on the shared main machine (a heavy job
ran beside it, so a few percent are noise):

| Case | Mode | Threads | Before | After |
|---|---|---|---|---|
| `mri hmc -cost normmi` | reproduce | 1 / 8 | 49.05 / 8.23 | 39.12 / 6.35 |
| `mri hmc -cost mutualinfo` | reproduce | 1 / 8 | 46.44 / 7.89 | 38.27 / 6.26 |
| `mri hmc -nn_final` | default | 1 / 8 | 16.01 / 3.07 | 15.93 / 3.04 |
| `mri hmc -nn_final` | reproduce | 1 / 8 | 15.92 / 2.98 | 16.97 / 3.22 |
| `mri hmc -sinc_final` | default | 1 / 8 | 27.05 / 4.56 | 26.68 / 4.50 |
| `mri hmc -sinc_final` | reproduce | 1 / 8 | 26.74 / 4.47 | 16.98 / 3.18 |
| `afni 3dTshift -no_detrend` | reproduce | 1 / 8 | 0.29 / 0.15 | 0.30 / 0.16 |
| `afni 3dTshift`, header slice code 2 | default | 1 / 8 | 0.45 / 0.12 | 0.45 / 0.12 |
| `afni 3dTshift`, header slice code 2 | reproduce | 1 / 8 | 0.07 / 0.07 | 0.45 / 0.12 |

The histogram costs are about a fifth faster (the test bins once per volume and stage instead
of a pass over the volume at every evaluation; float32 histograms; stage 1's slices in
parallel). Where `reproduce` now computes what the newest original computes, the time follows
it: `-nn_final` and `-sinc_final` resample trilinearly as mcflirt 6.0.7.23 does (FSL-59; the
check that names FSL-59 stops at the first value that differs), and header slice code 2 is
shifted as AFNI 26.3.05 shifts it (AFNI 25.2.09 copied the slices unshifted). The default mode,
whose output did not change, takes the same time.

## The random-testing trial against the newest originals (2026-10-29)

`difftest --tools afni-tshift,afni-automask,afni-unifize,mri-hmc --budget 0.5 --afni-version
26.3.05 --fsl-version 6.0.7.23` (8 jobs, a box of 26 GiB; `<workspace>/oracles/difftest/2026-10-29-cleanroom-align/`):
15,264 cases in 0.51 h.

| Tool | Cases | Agree | Notice | Undefined | Differ | Memory |
|---|---|---|---|---|---|---|
| afni-automask | 4,727 | 4,723 | 4 | 0 | 0 | 0 |
| afni-tshift | 5,676 | 5,607 | 36 | 33 | 0 | 0 |
| afni-unifize | 2,035 | 2,019 | 5 | 0 | 0 | 11 |
| mri-hmc | 2,826 | 1,094 | 0 | 0 | 1,732 | 0 |

- The AFNI tools: no difference. Before the alignment, random testing against AFNI 26.3.05 had
  found 190 findings and 62 notices, every one AFNI-03 or AFNI-05; none now. The notices are
  corrections named where this input shows no change (AFNI-01, -07, -16, -45, as the report
  lists them); the undefined cases are AFNI-20 (a one-row dataset AFNI writes as text with a
  random identifier; the record's documented divergence); the memory cases are 3dUnifize `-EPI`
  balls of neighbours beyond any machine, which the clean-room original has no guard for yet
  (AFNI-75's addition, next-steps), queued as heavy cases.
- The head-motion correction: 1,732 phantoms beyond the record's thresholds, the class the
  permanent case `difftest/phantom-several-minima` explains (small phantoms moved by large
  rotations: the cost has several minima). Measured on all 1,732 with `hmc_alignment_bin.py`
  ([`results/hmc-alignment-trial-2026-10-29.jsonl`](results/hmc-alignment-trial-2026-10-29.jsonl)),
  larmorx in `reproduce` against mcflirt 6.0.7.23, median ratio of the temporal standard
  deviations: `normcorr` 1.000 (1,163; 821 within 1 %), `normmi` 1.001 (126; 57 within 1 %;
  1.085 before), `mutualinfo` 0.999 (161; 63), `corratio` 1.000 (186; 81), `leastsquares` 0.999
  (96; 70): as well aligned as mcflirt's for every cost. The worst ratios (up to 1.76, rotations
  of 10° to 16°) are the same before and after this part. One case's notices differ between the
  modes (`mri-hmc:20261029:45`, `-cost corratio -reffile` on a thin slab: by default FSL-01 is
  not named, because the in-plane mode's FSL-03 correction leaves the previous volume at the
  identity; reproduced, it is): a clean-room difference, reported for a later part (replay:
  `python -m larmorx_validation difftest --replay <run>/findings/mri-hmc/20261029-00045-original.json`).
