# Provenance: larmorx-mri

**Clean-room implementation. No FSL code was used.**

FSL is distributed under the FSL Licence (non-commercial), so larmorx re-implements its tools
(CLAUDE.md rule 1, `docs/licensing.md`). `hmc` was written by an implementer who never opened
FSL's source code (`~/fsl/src/` or any other copy), the replica crates (`crates-nc/`), notes
derived from the source (`docs/findings/fsl-mcflirt.md`), or the scripts and analyses the spec
author wrote while reading it (`scripts/run_mcflirt_oracle.py`,
`oracles/fsl-6.0.7/mcflirt/analysis/`). Its only inputs were:

| Input | What it gave |
|---|---|
| [`specs/mcflirt.md`](../../specs/mcflirt.md) | the behaviour spec: inputs and conventions, the schedule, the cost function, the optimiser, the outputs, the command line |
| Jenkinson, Bannister, Brady & Smith (2002), *NeuroImage* 17:825–841; Jenkinson & Smith (2001), *Medical Image Analysis* 5:143–156 | the method: multi-resolution schedule, normalised correlation, apodization, rigid parameters |
| `mcflirt -help` (FSL 6.0.7.17) | the options and their defaults |
| Black-box runs of the FSL 6.0.7.17 binary (`~/fsl/bin/mcflirt`) and the recorded oracle runs (`oracles/fsl-6.0.7/mcflirt/runs/*`: `run.json` and output files) | every rule marked *(observed)* in the spec; the float32 rounding of the rotations' sines and cosines, the float32 square roots and the exact three-level sums, settled from `-verbose 20` traces (the evaluated matrices) and added to the spec |
| Textbook mathematics | Gram–Schmidt (QR) factorisation of affine matrices, the cubic B-spline prefilter (Unser 1999), windowed sinc kernels, histogram entropies |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `hmc::volume`, `hmc::image` (reading as float32, scaling, the x reversal of positive-determinant images, output types and headers) | mcflirt's image handling | spec §4.1, §10.1, black-box runs |
| `hmc::rigid` (parameters ↔ matrices, Euler decomposition) | mcflirt's parametrisation | spec §4.4, §13, `-verbose 20` traces |
| `hmc::grid` (8 mm and 4 mm reference grids) | mcflirt's subsampling | spec §5.3 |
| `hmc::cost` (sampling, edge weighting, normalised correlation with mcflirt's count; least squares, correlation ratio, Woods, mutual information) | mcflirt's cost functions | spec §6, appendix A, `-verbose 20` traces |
| `hmc::search` (bracketing, parabolic/golden refinement, one sweep) | mcflirt's optimiser | spec §7 |
| `hmc::estimate` (stages, initialisation chain, mean pass, in-plane mode, stage-4 sinc) | mcflirt's schedule | spec §5, §8, §12, black-box runs |
| `hmc::resample`, `hmc::kernels` (final interpolation, background value) | mcflirt's final resampling | spec §9 |
| `hmc::report` (`.mat`, `.par`, `.rms` text, RMS deviation) | mcflirt's output files | spec §10, Jenkinson (1999) for the RMS deviation |
| `hmc::world` (FSL matrices as RAS and ITK transforms) | nitransforms' reading of FSL matrices | spec §4.3 |
| `cli::hmc` (options, messages, exit codes) | mcflirt's command line | spec §11, `-help`, black-box runs |

Transcendental functions come from `larmorx_core::math` (correctly rounded CORE-MATH ports);
`atan2` comes from the `libm` crate, which has no port in `larmorx_core::math` yet. The parity
record is [docs/validation/mri-hmc.md](../../docs/validation/mri-hmc.md).
