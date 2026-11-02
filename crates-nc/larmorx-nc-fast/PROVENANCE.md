# Provenance: larmorx-nc-fast

**Replica: translated from FSL's source.** This crate is a step-by-step Rust translation of FSL
6.0.7.23's tissue segmentation program `fast` (FAST4, FMRIB's Automated Segmentation Tool),
written to give FSL's output bit for bit. It is distributed under the FSL Licence's terms
(`LicenseRef-FSL`: non-commercial use only, `../LICENSE`), as part of the `larmorx-nc` package
(`docs/licensing.md`). The parts of FSL's libraries and of the C and C++ libraries it relies on
(newimage's volumes, statistics, file reading and writing, separable convolution and Gaussian
kernels; NewNifti; utils' option parser; glibc's `exp`, `log`, `expf`, `logf` as FSL's binary
binds them and its `rand`; libstdc++'s `std::sort`) are the shared core's, `../larmorx-nc-core`
(its `PROVENANCE.md`). The Apache-2.0 clean-room original of FAST is still to be written, from
`specs/fast.md`; it will use none of this crate's code.

The FSL package is fsl-fast4 2111.3 of the FSL 6.0.7.23 conda installation
(`$FSLDIR/src/fsl-fast4/`; tag on `git.fmrib.ox.ac.uk`); its original source is kept,
unmodified, in `../fsl-source/fsl-fast4/` (the FSL Licence requires it with any transmitted
product).

## Licences of the translated sources

- **FSL** (fsl-fast4 2111.3): "FMRIB Software Library, Release 6.0 (c) 2018, The University of
  Oxford", the FSL Licence (non-commercial). FAST: "FAST4 - FMRIB's Automated Segmentation Tool
  v4, John Vickers, Mark Jenkinson, Matthew Webster and Steve Smith, FMRIB Image Analysis Group;
  Copyright (C) 2005-2012 University of Oxford".

Every source file starts with an SPDX header naming its licence; each module's documentation says
what it translates.

## FSL files and functions translated

| Rust module | FSL file | Functions, classes |
|---|---|---|
| `fast` | fsl-fast4 `fast_two.cc` | `main` (the options in their order, the usage, the option errors, `-H`'s check), `segmentSingleChannel` (the input, its negative values, the outputs `_seg`, `_seg_<n>`, `_prob_<n>`, `_pve_<n>`, `_pveseg`, `_mixeltype`, `_restore`, `_bias`), `prior_registration` (the prior names, `FSLDIR`; `-a`'s registration with FLIRT pending), `segmentMultiChannel` (the channels read from the last words, channel 1 the last; the outputs, `_restore_<n>`, `_bias_<n>`; fewer than two channels; more than four refused: LAPACK's paths are ported up to 4 x 4) |
| `mriseg` | fsl-fast4 `mriseg_two.{h,cc}` | `ZMRISegmentation`: `TanakaCreate`, `TanakaMain`, `BiasRemoval`, `Classification`, `Convolve`, `Dimensions`, `Initialise`, `logGaussian`, `MRFWeightsTotal`, `MRFWeightsInner`, `MRFWeightsAM`, `TanakaHyper`, `TanakaIterations`, `PVEnergy`, `PVClassificationStep`, `ICMPV`, `mixelType`, `PVestimation`, `takeexpo`, `InitSimple`, `calculateVolumes`, `printVolumeTotals`, `InitKernel`, `MeansVariances`, `Initclass`, `UpdateMembers`, `WeightedKMeans`, `qsort`, `pveClassification`; `TanakaPriorHyper` is not translated (it runs only for a negative `-H`, which `main` refuses) |
| `multi_mriseg` | fsl-fast4 `multi_mriseg_two.{h,cc}` | `ZMRIMULTISegmentation`: `TanakaCreate`, `TanakaMain`, `Dimensions`, `Initialise`, `Classification`, `pveClassification`, `logGaussian`, `MRFWeightsTotal`, `MRFWeightsInner`, `MRFWeightsAM`, `TanakaHyper`, `TanakaIterations`, `PVEnergy`, `PVClassificationStep`, `logpveGaussian`, `ICMPV`, `PVMoreSophisticated`, `PVestimation`, `takeexpo`, `InitWeights`, `InitSimple`, `Volumesquant`, `InitKernel`, `Convolve`, `MeansVariances`, `BiasRemoval`, `Initclass`, `UpdateMembers`, `WeightedKMeans`, `qsort`, `mresmatrix`, `covariancematrix`, `M_2PI`; not translated: `TanakaPriorHyper` (a negative `-H`, refused by `main`), `InitclassAlt`, `InitAprioriKMeans`, `UpdateWeights`, `pvmeans`, `pvvar`, `PVMeansVar` (never called) |

## Changes from the originals

- The 4D volumes are flat arrays of `nClasses + 1` volumes (FSL's layout); the products of
  `MeansVariances` are summed as they are computed (FSL stores them in two 4D volumes first), and
  `qsort` copies only what it moves: the same values, less memory.
- The MRF's neighbours are found once per voxel for all classes (FSL looks them up per class):
  each class's sum reads only its own class, so the sums and their order are FSL's.
- `log(sqrt(2 pi sigma^2))` of each class is computed once per sweep (FSL computes it per voxel):
  the same value each time.
- Multi-channel: the matrix expressions follow Armadillo 14.2.1's evaluation (`row * M * col`
  as `(row M) col`, `row * M.i() * col` as `row * solve(M, col)`) with the core's `arma` and
  `lapack`; `logGaussian`'s inverse and normalisation are taken once per class and sweep,
  `logpveGaussian`'s once per mixel type and fraction, and `PVEnergy`'s LU factors once per
  mixel type and fraction (FSL recomputes each per voxel: the same values); `p_bias` without a
  bias field (`-N`), which FSL never writes, is 0 (pending its D13 decision).
- Pending in this part's later steps: `-a`'s priors registered with FLIRT, and the D13 decisions
  of FAST's defects.
