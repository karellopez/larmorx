# Findings: how the tools we replicate really behave

A running record of what we learn about the upstream software while porting it. It covers
behaviour that the documentation does not state, quirks that decide whether two results match
bit for bit, and the choices larmorx made because of them. Each entry gives:

- **What**: the behaviour, stated precisely.
- **Where**: the upstream file and line (at the pinned release, see `UPSTREAM.md`), or the
  oracle run that showed it.
- **How we know**: *read* (from the source), *verified* (confirmed by running the upstream
  tool), or *validated* (part of a parity suite in `docs/validation/`).
- **larmorx**: what our port does about it: reproduces it, diverges on purpose, or does not
  support it yet.

New findings are added as they come up, during porting and while parity reports are run.
Entries are not deleted; when one turns out to be wrong it is corrected and the correction is
noted.

| File | Subject |
|---|---|
| [afni-tshift.md](afni-tshift.md) | AFNI 3dTshift: slice-timing correction as fMRIPrep runs it, AFNI's FFT |
| [fsl-mcflirt.md](fsl-mcflirt.md) | FSL mcflirt: head-motion correction as fMRIPrep runs it (source-derived; **forbidden to clean-room implementers**) |
| [ants-cli.md](ants-cli.md) | ANTs command lines: argument parser, `antsApplyTransforms` options and outputs |
| [ants-image-programs.md](ants-image-programs.md) | ImageMath, ThresholdImage, MultiplyImages: dispatch, exit codes, operands, quirks of each operation |
| [ants-gaussian-filters.md](ants-gaussian-filters.md) | ITK's recursive and discrete Gaussian filters; ImageMath `G`, `Laplacian`, `Grad`, `UnsharpMask`; SmoothImage; ResampleImageBySpacing; ResampleImage (pixel types, interpolators, argument quirks) |
| [ants-morphology.md](ants-morphology.md) | ITK's ball and binary/grayscale morphology; ImageMath `MD`/`ME`/`MO`/`MC`/`GD`/`GE`/`GO`/`GC`, `FillHoles`, `PadImage`; connected components and relabelling, `GetLargestComponent`; Danielsson and signed Maurer distance maps (`D`, `MaurerDistance`); `ExtractContours`, `ThresholdAtMean`, `ReplaceVoxelValue` |
| [ants-n4.md](ants-n4.md) | N4BiasFieldCorrection and ITK's N4 filter: how fMRIPrep calls it, the thread dependence of the B-spline fit, GCC 11's double-precision axes, histogram sharpening quirks, vnl's FFT, lattice refinement, `ShrinkImageFilter`, glibc's `expf`/`logf` |
| [ants-denoise.md](ants-denoise.md) | DenoiseImage and ITK's adaptive non-local means filter (remote module AdaptiveDenoising): how sMRIPrep calls it, the large and run-dependent thread band, the mask on the shrunk grid, the Rician bias image, the noise resampled with float coordinates, ITK's `floor` of tiny negative values, glibc's `expf` |
| [itk-transforms.md](itk-transforms.md) | ITK transforms: composite order, inverses, displacement fields, transform files |
| [itk-resampling.md](itk-resampling.md) | ITK `ResampleImageFilter` and the interpolators ANTs uses |
| [itk-nifti.md](itk-nifti.md) | ITK's NIfTI reader and writer: geometry, scaling, non-finite values, vector images, the header it writes |
| [antspy.md](antspy.md) | ANTsPy as an oracle: defaults and quirks of its wrappers |
| [nibabel.md](nibabel.md) | nibabel as the reference for NIfTI I/O |
| [platform-math.md](platform-math.md) | `exp`/`log`/`sin`/`cos`/`powf`/`expf`/`logf`: glibc vs the `libm` crate and CORE-MATH, what it does to bit parity, the preload library that makes ANTs correctly rounded |
| [scipy-ndimage.md](scipy-ndimage.md) | SciPy's `map_coordinates` and spline prefilter: padding, boundary rules, summation order, undefined behaviour |
| [fmriprep-resampling.md](fmriprep-resampling.md) | fMRIPrep's one-shot resampler and nitransforms: float32 coordinates, BLAS fused products, voxel-shift map, Jacobian, displacement fields |
| [fmriprep-oracle.md](fmriprep-oracle.md) | fMRIPrep 25.2.6 as it actually ran (the first oracle run): the tools and versions in its image, thread counts (`NSLOTS`), seeds, the parameters nipype passes or drops, headers it rewrites, what reproduces run to run |
| [numpy-blas.md](numpy-blas.md) | numpy's 4×4 products and inverses: which BLAS/LAPACK routine, OpenBLAS's kernels per CPU, the Haswell operation order larmorx reproduces, how often the kernels differ |
