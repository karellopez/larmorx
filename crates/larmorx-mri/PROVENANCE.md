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
| Black-box runs of the FSL 6.0.7.17 binary (`~/fsl/bin/mcflirt`) and the recorded oracle runs (`oracles/fsl-6.0.7/mcflirt/runs/*`: `run.json` and output files) | every rule marked *(observed)* in the spec; the float32 rounding of the rotations' sines and cosines, the float32 square roots and the exact three-level sums, settled from `-verbose 20` traces (the evaluated matrices) and added to the spec; for D13, the wraparound of integer outputs beyond their range (§9.4) |
| The defect register's summary table and "D13 gaps" list (`docs/findings/upstream-defects.md`, which describe the FSL entries by observable behaviour only; the restricted FSL section was not opened), `mcflirt -help` and nipype's `MCFLIRT` interface (BSD; its output names) | which of mcflirt's behaviours are defects, and the corrections larmorx makes by default (`larmorx_core::defects`, `upstream_defects`): FSL-01, -03, -05 to -10, -13, -14, and the opt-in count of FSL-02 |
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

**2026-10-29 (the clean-room fixes): no change to this crate.** The 782 random-testing
differences against mcflirt were measured by a clean-room implementer who had never opened
FSL's source, the replica crates or the restricted findings, from black-box runs of mcflirt
(`~/fsl/bin/mcflirt`, FSL 6.0.7.17) and `larmorx mri hmc` on the findings' recipes
(`validation/investigations/cleanroom-fixes/hmc_alignment.py`): explained for every cost but
`normmi`, which is left open ([docs/findings/mri-hmc-original.md](../../docs/findings/mri-hmc-original.md)).

## FSL 6.0.7.23 (2026-10-29)

`hmc` follows mcflirt from FSL 6.0.7.23 (`~/fsl-6.0.7.23`), changed by a clean-room implementer
who never opened FSL's source (any version), the replica crates (`crates-nc/`), the restricted
findings (`docs/findings/fsl-mcflirt.md`, `fsl-bet.md`, the register's restricted sections),
`docs/parts/`, the version probes of the replica authors (`validation/investigations/version-fsl/`,
`validation/src/larmorx_validation/parity/fsl_versions.py`) or their permanent cases, from these inputs only:

| Input | What it gave |
|---|---|
| [`specs/mcflirt.md`](../../specs/mcflirt.md): the lines marked "(FSL 6.0.7.23)" (§4.1, §9.3, §10.1, §13) and appendix B | FSL-59's final interpolation, the zstd and bzip2 output types and their message, reading by content, the completion of names, FSL-60's renaming, `mutualinfo` and `normmi` exactly |
| Black-box runs of FSL 6.0.7.23's `mcflirt` and the replica program (`larmorx-nc fsl mcflirt`, the main checkout's build; its source not opened): [`validation/investigations/cleanroom-align/fsl_io_probe.py`](../../validation/investigations/cleanroom-align/fsl_io_probe.py) | every output type's files, reading zstd, bzip2 and gzip files under other names, the errors of completion (`Multiple possible filenames detected for basename`, `No image files match`), the unknown-type message, where FSL-60 reads and writes (a pair's header renamed, its data file not), and the words of the FSL-59 and FSL-60 notices (the replica's, without its explanation of the cause) |
| `-verbose 20` traces of FSL 6.0.7.23's `mcflirt` (the evaluated matrices) against larmorx's with a local trace build ([`mi_trace.py`](../../validation/investigations/cleanroom-align/mi_trace.py), [`trace.patch`](../../validation/investigations/cleanroom-align/trace.patch)) | appendix B checked evaluation by evaluation; the in-plane mode's centre of rotation is the padded volume's (the spec does not say; observed) |
| The third-party crates `ruzstd` 0.9.1 (MIT) and `bzip2` 0.6.1 with `libbz2-rs-sys` (MIT/Apache-2.0, a pure-Rust bzip2), as libraries | zstd and bzip2 compression and decompression (CLAUDE.md rule 5: pure Rust) |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `hmc::files` (compression by content, the extensions, the completion of names, FSL-60's header name) | FSL 6.0.7.23's image names and formats | spec §4.1, §10.1, black-box runs |
| `hmc::image` (`read_series_files`, `write_like_files`: the other names and compressions; `OUTPUT_TYPES`, the unknown-type message) | FSL 6.0.7.23's reading and writing | spec §4.1, §10.1, black-box runs |
| `hmc::cost` (`bin_grid_mi`, `TestBins`, the contributions, histograms and entropies of `mutual_info`) | mcflirt's `mutualinfo` and `normmi` | spec appendix B, `-verbose 20` traces |
| `hmc::estimate::register` (the padded centre in the in-plane mode) | mcflirt's in-plane mode | spec §12, `-verbose 20` traces |
| `hmc::pipeline` (FSL-59), `hmc::resample::series_differ` | mcflirt 6.0.7.23's `-sinc_final` and `-nn_final` | spec §9.3, black-box runs |
| `cli::hmc` (the files opened, FSL-60's notices, the unknown-type message) | mcflirt 6.0.7.23's command line | spec §4.1, §10.1, black-box runs |
