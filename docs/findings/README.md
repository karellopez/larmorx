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
| [itk-transforms.md](itk-transforms.md) | ITK transforms: composite order, inverses, displacement fields, transform files |
| [itk-resampling.md](itk-resampling.md) | ITK `ResampleImageFilter` and the interpolators ANTs uses |
| [itk-nifti.md](itk-nifti.md) | ITK's NIfTI reader and writer: geometry, scaling, non-finite values, vector images, the header it writes |
| [antspy.md](antspy.md) | ANTsPy as an oracle: defaults and quirks of its wrappers |
| [nibabel.md](nibabel.md) | nibabel as the reference for NIfTI I/O |
| [platform-math.md](platform-math.md) | `exp`/`log`/`sin`/`cos`/`powf`: glibc vs the `libm` crate, and what it does to bit parity |
