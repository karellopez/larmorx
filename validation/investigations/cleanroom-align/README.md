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
