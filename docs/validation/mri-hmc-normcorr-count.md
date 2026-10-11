# FSL-02: mcflirt's correlation count against the corrected count

Measured before deciding (D13 item 4, [upstream-defects.md](../api/upstream-defects.md)). mcflirt's normalised correlation divides by a count that is a running sum of running sums of the edge weights (`specs/mcflirt.md` §6.4), which makes it effectively uncentred. `lx.mri.hmc(..., normcorr_count="weights")` divides by the sum of the weights instead: a weighted Pearson correlation. Three larmorx configurations are compared with mcflirt 6.0.7's matrices on every run of mcflirt's variability band: `reproduce` (mcflirt's defects, mcflirt's count), `default` (the other defects corrected, mcflirt's count) and `weights` (the corrected count).

Generated with `python -m larmorx_validation.parity.mri_hmc_count` (2026-10-11).

## Against mcflirt and its band

95th percentile of the matrix RMS deviation from mcflirt's matrices (mm, radius 80 mm, about the reference's field-of-view centre). *Band*: mcflirt against itself under small perturbations of its input (the worst of three noise realisations and the reverse order). The last column is the count's own effect on larmorx (`weights` against `default`).

| Run | mcflirt band 95th pct | reproduce | default | weights | weights: median Δparam (mm / °) | weights: FD r (band) | weights vs default |
|---|---|---|---|---|---|---|---|
| `real/ds000005/default` | 0.0219 | 0.0212 | 0.0212 | **0.0234** | 0.0027 / 0.0032 | 0.8412 (0.8749) | 0.0261 |
| `real/ds000005/fmriprep` | 0.1014 | 0.0262 | 0.0269 | **0.0552** | 0.0049 / 0.0044 | 0.8161 (0.7941) | 0.0319 |
| `real/ds000005/opt-meanvol` | 0.0537 | 0.0205 | 0.0227 | **0.0300** | 0.0046 / 0.0051 | 0.7773 (0.6737) | 0.0301 |
| `real/ds000117/default` | 0.0231 | 0.0220 | 0.0220 | **0.0219** | 0.0030 / 0.0044 | 0.8720 (0.8592) | 0.0215 |
| `real/ds000117/fmriprep` | 0.0203 | 0.0191 | 0.0191 | **0.0205** | 0.0028 / 0.0037 | 0.8742 (0.8720) | 0.0197 |
| `real/ds000122/default` | 0.0289 | 0.0288 | 0.0288 | **0.0287** | 0.0038 / 0.0058 | 0.9985 (0.9987) | 0.0285 |
| `real/ds000122/fmriprep` | 0.0312 | 0.0300 | 0.0300 | **0.0312** | 0.0037 / 0.0059 | 0.9981 (0.9977) | 0.0329 |
| `real/ds000210-rest-echo2/default` | 0.0221 | 0.0210 | 0.0210 | **0.0229** | 0.0024 / 0.0031 | 0.9936 (0.9920) | 0.0217 |
| `real/ds000210-rest-echo2/fmriprep` | 0.0229 | 0.0226 | 0.0226 | **0.0236** | 0.0027 / 0.0040 | 0.9868 (0.9799) | 0.0237 |
| `real/ds000258-bigendian/default` | 0.0186 | 0.0178 | 0.0178 | **0.0182** | 0.0021 / 0.0031 | 0.9951 (0.9948) | 0.0184 |
| `real/ds000258-bigendian/fmriprep` | 0.0193 | 0.0181 | 0.0181 | **0.0189** | 0.0022 / 0.0035 | 0.9957 (0.9949) | 0.0196 |
| `real/ds003345/default` | 0.0261 | 0.0278 | 0.0278 | **0.0289** | 0.0040 / 0.0061 | 0.9861 (0.9893) | 0.0293 |
| `real/ds003345/fmriprep` | 0.0299 | 0.0259 | 0.0264 | **0.0313** | 0.0034 / 0.0057 | 0.9838 (0.9840) | 0.0304 |
| `real/ds005040/default` | 0.0263 | 0.0259 | 0.0259 | **0.0305** | 0.0029 / 0.0050 | 0.7249 (0.8091) | 0.0296 |
| `real/ds005040/fmriprep` | 0.0257 | 0.0252 | 0.0252 | **0.0236** | 0.0021 / 0.0043 | 0.8492 (0.8238) | 0.0253 |
| `real/ds006010/default` | 0.0165 | 0.0130 | 0.0130 | **0.0160** | 0.0012 / 0.0004 | 0.9216 (0.9074) | 0.0140 |
| `real/ds006010/fmriprep` | 0.0164 | 0.0142 | 0.0142 | **0.0147** | 0.0015 / 0.0026 | 0.9086 (0.8514) | 0.0148 |
| `real/ds006736/default` | 0.0233 | 0.0188 | 0.0188 | **0.0239** | 0.0031 / 0.0047 | 0.9986 (0.9989) | 0.0269 |
| `real/ds006736/fmriprep` | 0.0259 | 0.0252 | 0.0253 | **0.0264** | 0.0031 / 0.0049 | 0.9986 (0.9974) | 0.0265 |
| `syn/content/background/default` | 3.9616 | 0.0864 | 79.5716 | **79.8477** | 0.0172 / 0.0461 | 0.5961 (1.0000) | 0.9860 |
| `syn/geometry/tiny/default` | 0.3600 | 0.1130 | 1.6253 | **11.2539** | 0.0658 / 3.0932 | 0.8811 (0.9963) | 11.2815 |
| `syn/motion/identical-copies/reffile` | 0.0650 | 0.0575 | 0.0575 | **0.0654** | 0.0044 / 0.0263 | 0.0920 (0.2677) | 0.0558 |
| `syn/motion/large/reffile` | 0.0887 | 0.0646 | 0.0646 | **0.0619** | 0.0053 / 0.0187 | 0.9995 (0.9991) | 0.0604 |
| `syn/motion/small/default` | 0.0558 | 0.0488 | 0.0488 | **0.0640** | 0.0036 / 0.0140 | 0.9974 (0.9974) | 0.0638 |
| `syn/motion/small/dof12` | 1.1565 | 0.1252 | 0.4698 | **0.5715** | 0.0109 / 0.0705 | 0.9558 (0.8346) | 0.4374 |
| `syn/motion/small/dof7` | 0.0759 | 0.0707 | 0.0707 | **0.1078** | 0.0080 / 0.0222 | 0.9940 (0.9968) | 0.0859 |
| `syn/motion/small/meanvol` | 0.4177 | 0.2811 | 0.3990 | **0.6798** | 0.0079 / 0.0136 | 0.9027 (0.9015) | 0.3072 |
| `syn/motion/small/reffile` | 0.1068 | 0.0713 | 0.0862 | **0.0954** | 0.0059 / 0.0200 | 0.9947 (0.9942) | 0.0686 |

Within the band (or the spec's 0.1 mm where the band is smaller): reproduce 28 of 28, default 26 of 28, weights 24 of 28.

`default` differs from `reproduce` only where another correction applies: the last volume (FSL-01) and in-plane runs (FSL-03). Where it leaves the band (the synthetic `background` and `tiny` series) the difference is the last volume, and the ground truth below shows which is closer: on `background` mcflirt's last volume, started from the identity, converges to a 180° rotation.

## Against the synthetic ground truth

Mean RMS deviation from the true motion (mm), on the synthetic series registered to the phantom at rest (`-reffile`) or to their own middle volume (mcflirt's default).

| Run | mcflirt | reproduce | default | weights |
|---|---|---|---|---|
| `syn/content/background/default` | 17.9268 | 17.9165 | 0.2992 | **0.4553** |
| `syn/geometry/tiny/default` | 3.2286 | 3.2465 | 3.4459 | **3.3110** |
| `syn/motion/identical-copies/reffile` | 0.2225 | 0.2308 | 0.2384 | **0.2550** |
| `syn/motion/large/reffile` | 0.2250 | 0.2236 | 0.2260 | **0.2382** |
| `syn/motion/small/default` | 0.3828 | 0.3872 | 0.3877 | **0.3811** |
| `syn/motion/small/dof12` | 1.3694 | 1.3704 | 1.4056 | **1.3652** |
| `syn/motion/small/dof7` | 0.3505 | 0.3515 | 0.3582 | **0.3507** |
| `syn/motion/small/reffile` | 0.3520 | 0.3415 | 0.3471 | **0.3463** |
