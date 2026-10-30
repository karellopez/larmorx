# Findings: the clean-room head-motion correction against mcflirt on random phantoms

What the clean-room implementer of `crates/larmorx-mri`'s `hmc` measured about the differences
random testing found between `lx.mri.hmc` and FSL 6.0.7.17's mcflirt (2026-10-29). Black-box
only: both programs were run and their outputs read; nothing here comes from FSL's source or
the replica's (`crates-nc/`), so this file is **unrestricted** (the source-derived findings are
[fsl-mcflirt.md](fsl-mcflirt.md), which the implementer did not open).

## The differences of the first night (2026-10-25)

- **What:** on small random phantoms (4 to 8 volumes of at most 32 × 32 × 20 voxels, a head-like
  phantom moved by a random rigid motion in each volume, rotations of 5° to 30°), 782 cases
  gave motion estimates beyond the record's thresholds against mcflirt (the matrices' RMS
  deviation, the parameters, the corrected voxels, the RMS files, FD).
- **How it was measured:** every one of the 782 findings run again (not a sample): its shrunk
  inputs rebuilt, mcflirt and `larmorx mri hmc` (in `reproduce`, as the night compared, and by
  default) run on its arguments, and the corrected series' temporal standard deviation in the
  phantom compared (voxels of mcflirt's corrected mean above 10 % of its maximum; lower is
  better aligned): [`validation/investigations/cleanroom-fixes/hmc_alignment.py`](../../validation/investigations/cleanroom-fixes/hmc_alignment.py)
  (`--summary` prints the table below from its JSON-lines output).

| Subset | Findings | larmorx lower | within 1 % | higher | ratio larmorx / mcflirt, median (5 % – 95 %) | above 1.10 | below 0.90 |
|---|---|---|---|---|---|---|---|
| all | 782 | 162 | 458 | 162 | 1.000 (0.911 – 1.090) | 37 | 33 |
| `normcorr` (mcflirt's default, and given) | 524 | 77 | 384 | 63 | 1.000 (0.951 – 1.035) | 5 | 9 |
| all but `normmi` | 691 | 150 | 441 | 100 | 1.000 (0.909 – 1.048) | 11 | 30 |
| `corratio` | 52 | 20 | 21 | 11 | 0.998 (0.905 – 1.062) | 2 | 3 |
| `leastsquares` | 37 | 8 | 23 | 6 | 1.000 (0.945 – 1.022) | 0 | 0 |
| `mutualinfo` | 78 | 45 | 13 | 20 | 0.981 (0.809 – 1.089) | 4 | 18 |
| **`normmi`** | **91** | 12 | 17 | **62** | **1.033 (0.924 – 1.203)** | **26** | 3 |

(By default, the corrections of FSL-01 and others widen the spread, all findings: median 1.000,
5 % – 95 % 0.893 – 1.140. mcflirt's own corrected series has a temporal standard deviation 1.30
times smaller than the uncorrected input's, median.)

- **Conclusion, all costs but `normmi`:** the two programs align these phantoms equally well
  (691 findings: median ratio 1.000, larmorx the better aligned in 150 and the less in 100). With motions this
  large and phantoms this small the cost has several minima, and the two optimisers end in
  different ones, which is what the record's thresholds (made for real BOLD runs, where both
  agree within mcflirt's own band) count as a difference. The spec allows this
  ([specs/mcflirt.md](../../specs/mcflirt.md), "Where exact agreement will be hard"). Explained:
  the permanent case `difftest/phantom-several-minima` keeps one such finding as an expected
  divergence with this reason.
- **`-cost normmi`: aligns worse, cause open.** On the 91 findings with the normalised mutual
  information, larmorx's corrected series is the less aligned one in 62 (median ratio 1.033,
  26 above 1.10); with `mutualinfo`, the same histogram and entropies without the
  normalisation, it is the better aligned one (median 0.981). The cause was not found by
  black-box runs in this part: mcflirt's `-verbose 20` trace prints the matrices it evaluates
  but no cost values, and the spec describes the histogram costs only in outline (its open
  question 4: appendix A). Left open: the permanent case `difftest/normmi-aligns-worse`
  (status open) keeps the worst finding (ratio 1.38). Next step: the spec writer describes
  `normmi`'s histogram, its fuzzy binning and the entropy correction exactly (and whether
  mcflirt's normalised cost is `(H1 + H2)/H12` or another form), then a clean-room fix
  measured with the same tool.

## FSL 6.0.7.23 and the exact histogram costs (2026-10-29)

What the clean-room implementer who aligned `hmc` with FSL 6.0.7.23's mcflirt measured, by
black-box runs of mcflirt (`~/fsl-6.0.7.23/bin/mcflirt`), the replica program and larmorx
([`validation/investigations/cleanroom-align/`](../../validation/investigations/cleanroom-align/)).

- **`-cost normmi` and `mutualinfo` exactly (appendix B of the spec).** The costs now follow
  the spec's appendix B step by step: float32 histograms filled in the sampling order, the test
  bins over the whole test volume, the fuzzy weights, x86-64's conversions to integers, the
  entropies over every grid point corrected to the overlap. **Evaluation by evaluation**, with
  `-fudge -stages 1` (`mi_trace.py`: mcflirt's `-verbose 20` trace against a traced build of
  larmorx): every evaluated matrix is mcflirt's, to its printed 6 decimals, on the synthetic
  series of the recorded runs (`normmi` 429 of 429, `mutualinfo` 422 of 422) and on 22 of 30
  random phantoms (11 of 15 per cost); the other 8 agree for 19 to 224 evaluations, then differ
  in the 5th or 6th decimal of a translation, as `normcorr`'s path does (spec §17, open question
  1: the search's float32 last bits, shared by every cost). **On the 169 findings of the first
  long random-testing run** (2026-10-25; larmorx in `reproduce` against mcflirt 6.0.7.23, the
  corrected series' temporal standard deviation, larmorx / mcflirt; `hmc_alignment_bin.py`):

  | Cost | Build | larmorx lower | within 1 % | higher | median (5 % – 95 %) |
  |---|---|---|---|---|---|
  | `normmi` (91) | before | 10 | 13 | 68 | 1.051 (0.929 – 1.330) |
  | `normmi` (91) | after | 17 | 55 | 19 | 1.000 (0.957 – 1.057) |
  | `mutualinfo` (78) | before | 43 | 11 | 24 | 0.983 (0.809 – 1.208) |
  | `mutualinfo` (78) | after | 10 | 54 | 14 | 1.000 (0.919 – 1.046) |

  So `normmi` no longer aligns worse: the open item of the earlier part is closed (the
  permanent case `difftest/normmi-aligns-worse` is re-described from this measurement).
- **The in-plane mode's centre of rotation is the padded volume's** (spec §12 does not say).
  On a thin slab (finding 00298, 26 × 15 × 6 voxels of 3 mm) mcflirt's second evaluated matrix
  (the first rotation step) had another translation than larmorx's: the rotation was about
  another centre. With the centre of mass of the test volume as the in-plane mode pads it (one
  copy of the first and last slices, §12), 154 of 154 `normmi` evaluations agree (`normcorr`
  on the same series: 39 identical instead of 1). Both modes use it; the spec writer may add it
  to §12 as observed.
- **FSL 6.0.7.23's I/O** (`fsl_io_probe.py`): every `FSLOUTPUTTYPE` writes the files mcflirt
  writes (zstd and bzip2 single files and pairs included), whose decompressed headers and values
  read back alike; files are read by their content (a bzip2 or zstd file named `.nii` or
  `.nii.gz`, a gzip file named `.nii.zst`); a name without an extension is completed in FSL's
  order, with FSL's errors for two matches and for none; an unknown `FSLOUTPUTTYPE` prints FSL's
  message (larmorx before any work; mcflirt after the estimation and the `.mat` and `.par`
  files); FSL-60 (an image name holding `.img`): with `reproduce`, larmorx reads and writes the
  files mcflirt 6.0.7.23 does in the 11 probed cases (a pair's header under the renamed name,
  its data file under the name given; the `.mat` directory and `.par` never renamed). FSL-59:
  `-sinc_final` and `-nn_final` give the default run's series with `reproduce`, as mcflirt
  6.0.7.23's do, and are named where the two series differ.
- **Random testing against FSL 6.0.7.23** (a trial of 0.5 h, 2,826 cases, 2026-10-29): 1,732
  phantoms beyond the record's thresholds, as well aligned as mcflirt's for every cost (median
  ratio of the temporal standard deviations 0.999 to 1.001 per cost; `normmi` 1.085 before this
  part): the several-minima class of the first long run. One case names FSL-01 in `reproduce`
  only (`mri-hmc:20261029:45`: `-cost corratio` on a thin slab, where the default's FSL-03
  correction leaves the previous volume's matrix at the identity, so FSL-01 changes nothing
  there): reported, not changed in this part
  ([`validation/investigations/cleanroom-align/README.md`](../../validation/investigations/cleanroom-align/README.md)).

## FSL-20, FSL-21, FSL-65 and FSL-66 in the clean-room original (2026-10-30)

Written from `specs/mcflirt.md` §10.6, §12.1 and §14.1 (the spec writer's lines of 2026-10-30)
and black-box runs of FSL 6.0.7.23's mcflirt, by a clean-room implementer who opened none of
FSL's source, the replica crates or the restricted findings. The cases of §14.1 are in the
`mri-hmc` suite (`failures/<case>`, `validation/src/larmorx_validation/parity/mri_hmc_failures.py`:
mcflirt live, larmorx in both modes); the probes are in
[`validation/investigations/cleanroom-additions/`](../../validation/investigations/cleanroom-additions/README.md).

- **Every case of §14.1 as the spec says** (33 cases on a 24 × 24 × 16 × 5 phantom, mcflirt
  6.0.7.23): mcflirt's exit status (0, signal 6, signal 8) as observed by the spec writer; with
  `reproduce`, larmorx's exit status (mcflirt's, 1 where mcflirt ends by a signal), mcflirt's
  first line of standard error (`Error!: Unknown Log directory return value`, `Error! Could not
  create directory: <-in's directory><out>.mat. No write permission`, `error: unable to open
  output file!`, the `terminate` lines of an image that cannot be written, `double free or
  corruption (out)`, the `std::bad_array_new_length` lines) and the files written, name for name
  (`MAT_0000` and the `.rms` files where the `.par` file cannot be opened; the two `.rms` files
  that can be where two cannot); by default, exit 1 and nothing written, the defect named.
- **What the spec left to the implementer, settled black-box:** mcflirt prints the permission
  message and the `terminate` lines with the names as given (`-out ro/o`: `ro/o.mat`,
  `ro/o.nii.gz`); with `-report` the FSL-20 message follows `Registering volumes ...` and the
  volumes' indices on the same line. In the default mode the `.mat` directory mcflirt would make
  for `-rmsrel` or `-rmsabs` without `-mats` (FSL-13 corrected: none is made) is checked, so a
  missing parent names FSL-20 there too (with FSL-13), as §14.1's `fsl20-rmsrel` asks.
- **Few bins, measured:** with no bins in a stage (`-bins` below 8) `mutualinfo`, `normmi` and
  `woods` run in `reproduce` as appendix B's rules give with `B = 0`; their cost is rounding noise,
  and so are their matrices against mcflirt's (reported, not required: 95th-percentile RMS
  deviation 3.1 mm for `normmi -bins 0`, 4.9 to 7.4 mm for `mutualinfo`, 74 mm for `woods -bins
  1` on the phantom; mcflirt's own are nothing like its default's, §12.1). With one bin in stage 1 (`-bins 8` to 15) the mutual
  information is 0 but for rounding as well: on the phantom 3 of the 4 registered volumes are
  identical to mcflirt's as text after stage 1, the others 0.4 and 2.1 mm away, and after three
  stages up to 1.4 mm; from 16 bins on the two agree within 0.07 mm (`-bins 16`, 32, 64; probe
  `hmc_bins_matrices.py`). So `failures/bins8-mutualinfo-s3` reports its matrices; `corratio`
  with 8 bins agrees within the record's thresholds (0.07 mm).
