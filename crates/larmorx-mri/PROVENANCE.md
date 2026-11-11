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
| `cli::hmc` (the files opened, FSL-60's notices, the unknown-type message; FSL-09 named in both modes where FSL-59 applies, its counts from mcflirt's trilinear series, `resample::fold_resampled`) | mcflirt 6.0.7.23's command line | spec §4.1, §9.3, §9.4, §10.1, black-box runs, the record's check of the notices in both modes |
| `cli::hmc` and `hmc::image` (FSL-11, 2026-11-11: `FSLOUTPUTTYPE` unset, the images written as `NIFTI_GZ` and FSL-11 named; with `reproduce` FSL's message, `FSLOUTPUTTYPE_UNSET`, at the first image and exit 1; `fsl11_notice`, also for `larmorx_fsl`, since relay 3 a re-export of `larmorx_core::defects::fsl11_notice`, the one text both implementations print; `fsl09_notice` names the program it is given) | mcflirt 6.0.7.23 without `FSLOUTPUTTYPE`: the matrices and the `.par` file written, then FSL's message at the first image (`-meanvol`'s mean image comes first) | the register's FSL-11 summary row, `specs/bet.md` §3.1 (the message), spec §10.1, black-box runs (`validation/investigations/easy-cli/fsl11_probe.py`) |

## FSL-20, FSL-21, FSL-65 and FSL-66 (2026-10-30)

Added by a clean-room implementer who never opened FSL's source (any version), the replica
crates (`crates-nc/`), the restricted findings (`docs/findings/fsl-mcflirt.md`, `fsl-bet.md`, the
register's restricted sections), `docs/parts/`, the spec writer's probes
(`validation/investigations/specs-memory/`) or the version probes, from these inputs only:

| Input | What it gave |
|---|---|
| [`specs/mcflirt.md`](../../specs/mcflirt.md) §10.6, §12.1, §14.1 and appendix B: the lines marked "(spec writer, 2026-10-30)" | the order of mcflirt's writes, what each failure gives in both modes (messages, exit statuses, files), the bins of each stage, mcflirt's ends, appendix B with `B = 0`, the cases |
| The defect register's summary rows of FSL-20, FSL-21, FSL-65 and FSL-66 | the defects' descriptions in `larmorx_core::defects` (FSL-65 and FSL-66 added) |
| Black-box runs of FSL 6.0.7.23's `mcflirt` (`~/fsl-6.0.7.23/bin/mcflirt`) on a 24 × 24 × 16 × 5 phantom: the `mri-hmc` suite's `failures/` cases and [`validation/investigations/cleanroom-additions/hmc_bins_matrices.py`](../../validation/investigations/cleanroom-additions/hmc_bins_matrices.py) | the names in mcflirt's permission and `terminate` messages; the matrices of few bins |

| larmorx | Behaviour reproduced | Source |
|---|---|---|
| `hmc::estimate` (`stage_bins`, `refused_bins`, `mcflirt_end`, `McflirtEnd`, the bins as `i64`) | mcflirt's bins per stage, its crashes (reproduced as errors) and the default's refusals | spec §12.1 |
| `hmc::cost` (`bin_grid_mi`, `TestBins`, `MiHistograms` and the correlation-ratio cells with no bins) | appendix B and A with `B = 0` | spec §12.1, appendix B |
| `cli::hmc` (`write_outputs` in mcflirt's order, `mat_directory`, `mat_parent_exists`, `input_prefix`, the reproduced endings with exit status 0) | mcflirt's outputs that cannot be written | spec §10.6 |

## The shared registration engine (2026-11-02)

FLIRT's clean-room original (`crates/larmorx-fsl/src/flirt/`, written from `specs/flirt.md` by a
clean-room implementer under the same restrictions) runs on `hmc`'s code, which it shares rather
than copies (the user, 2026-11-01: "Avoid duplication is a good quality practice"). The
additions, which leave the head-motion correction's computation unchanged (its standard tier
re-run and compared identical, `docs/validation/mri-hmc.md`):

| larmorx | Addition | For |
|---|---|---|
| `hmc::search::line_search_from` (`line_search` calls it with 10) | the first trial at `guess·u` | FLIRT's bound guess (`specs/flirt.md` §8.2) |
| `hmc::search::{vertex, golden}` public | | the line search's pieces |
| `hmc::cost::Accumulator::point_at` (by default `point`), `PointWeight`, `Weighted`, `sample_weighted`, `least_squares_weighted`, `corr_ratio_weighted`, `mutual_info_weighted` (the unweighted functions call them with no weight) | each point's weight from images of its own | FLIRT's `-refweight`, `-inweight` (§6.4) |
| `hmc::cost::correlation` and `CorrSlice` public (`normcorr` calls it) | | FLIRT's `normcorr` and its weighted `1 − r` |
| `hmc::cost::TestBins::fuzzy` (`TestBins::new` sets mcflirt's 0.5) | the fuzzy fraction | `setoption fuzzyfraction` (§7.3) |
| `hmc::cost::sample_all_weighted` | every slice into one accumulator, in slice order | FLIRT's single-precision bin sums of `corratio` and `woods` |
| `hmc::rigid::{decompose_parts, with_scales_skews}` (`decompose` and `compose` call them) | the decomposition's factors; the linear part from a given rotation | FLIRT's quaternion parameters (`-anglerep quaternion`, §3) |
| `hmc::image::read_series_files`: the stored array dropped once converted, a single volume taking the converted values, `Volume::flip_x_in_place` | the same values with one copy of an image at a time (the reader held up to four) | FLIRT's memory on full-size T1w references (`tests/parity/test_memory_peak.py`); mcflirt's reads too |

## The histogram costs' exact sums (2026-11-02, FLIRT's clean-room original part 2)

`specs/mcflirt.md` appendix A (the spec writer, 2026-11-02): mcflirt's `corratio` and `woods` are
FLIRT's (`specs/flirt.md` §6.2) unchanged. `hmc::cost::{BinMoments, bin_moments, corr_ratio_of,
woods_of}` (moved here from FLIRT's original, which uses them) compute them: single-precision
running sums per bin in the sampling order, only the bins of more than two points (left out of
the count and the total variance too), `η = 1 − A` and the cost `1 − η` each rounded to single,
Woods' terms `((n_b·n_b)·σ_b)/S_b` (or `(n_b·n_b)·σ_b` for a sum that is not positive). This
changes `lx.mri.hmc`'s `-cost corratio` and `-cost woods` (closer to mcflirt: the record's
`opt-cost-corratio` 95th-percentile RMS deviation 0.066 to 0.028 mm, `docs/validation/mri-hmc.md`,
re-made from 04b58ca; every other case identical). `hmc::cost::PointWeight` now receives the
point's edge weight and `TestVolume::edge_factors` gives its three factors (FLIRT's weighted costs,
`specs/flirt.md` §6.4); the head-motion correction passes no weight.

Part 2 of FLIRT's clean-room original, relay 5 (2026-11-02): `hmc::image::to_f32_vec` converts the
stored array where it lies (no copy of it first): one full-size array less at the reading's peak,
the values the same (FLIRT's avscale and midtrans with a 0.8 mm volume within FSL's memory).

The clean-room catch-up, relay 7 (2026-11-05): `hmc::volume::Volume::centre_counted` computes both
of mcflirt's centres of mass as FLIRT's blocked sums (`specs/mcflirt.md` §6.1, the spec writer's
answer of 2026-11-03; `specs/flirt.md` §8.1 step 3), and `hmc::estimate` and the command line print
mcflirt's `WARNING::in calculating COG, total = 0.0` as often as mcflirt 6.0.7.23 does (observed
with `validation/investigations/cleanroom-align/cog_warning_probe.py`, through the oracle cache:
once per registration of a constant test volume, once per `.par` line of a constant reference).
`hmc::rigid`'s `atan2` is `larmorx_core::math::atan2` (rule 5; the `libm` crate's before).

The pair header and ANALYZE's fields (PRELUDE's clean-room part, relay 2, 2026-11-06; black-box
runs of FSL 6.0.7.23's mcflirt through the oracle cache,
`validation/investigations/fsl-pair-header/probe.py`): `hmc::image::write_like_files` writes a
pair's header file with NIfTI's four extension bytes (352 bytes, NIfTI-2 544) for every pair type,
and `extents` 16384 and `regular` `r` into every NIfTI-1 header, as mcflirt writes them whatever
the input holds.

PRELUDE's clean-room part, relay 3 (2026-11-07; the spec writer's answers of 2026-11-06 and
black-box runs of FSL 6.0.7.23 through the oracle cache, no source read):
- `hmc::cost::{CorrSlice, CorrTotal, add_ones, sample_fold}`: the weighted normalised
  correlation's count `N` with each contributing point's weight added to its running total `C` as
  the point is visited, over the whole grid (`specs/mcflirt.md` §6.4, `specs/flirt.md` §6.4; the
  row's total added at once before): each slice's weights kept as runs of unit weights and the
  other values, the slices sampled in parallel groups and replayed in slice order, the same bits
  at any thread count; FLIRT's `normcorr` uses the same.
- `hmc::cost::least_squares` (`leastsquares`): FLIRT's `leastsq` unchanged (`specs/mcflirt.md`
  appendix A): `(w·δ)·δ` and `w` (or `δ·δ` and the count) in three single-precision levels, the
  quotient in single precision, `(max − min)²` of the two volumes' finite values when the weight
  (count) is not above 1 (double sums before).
- `hmc::image::time_step_as_read`, applied by `read_series_files` and `read_stored_files`: the time
  step as FSL's image layer reads it (its magnitude; 1 for an image with a time axis whose time
  step is 0), which every FSL original's and `lx.mri.hmc`'s outputs keep (`specs/flirt.md` §4.1;
  observed for every FSL program with a clean-room original:
  `validation/investigations/fsl-pair-header/time_step.py`).
