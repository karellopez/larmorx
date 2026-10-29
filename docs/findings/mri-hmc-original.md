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
