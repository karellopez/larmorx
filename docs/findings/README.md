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

**Defects** (crashes, races, deterministic bugs, imprecision, command-line quirks, wrong
messages) also get an entry in [upstream-defects.md](upstream-defects.md), the register: one
entry per defect with its class, evidence, effect on fMRIPrep users, what larmorx does, and
what decision D13 (correct by default, faithful on request) requires. Its top lists the defects
larmorx still reproduces by default. How to classify and handle a defect:
[porting guide](../porting-guide.md) §8.

**Clean-room implementers** must not read the files written from restricted source
([afni-tshift.md](afni-tshift.md), [fsl-mcflirt.md](fsl-mcflirt.md)) nor the register's
sections marked restricted ([porting guide](../porting-guide.md) §1).

| File | Subject |
|---|---|
| [upstream-defects.md](upstream-defects.md) | the register of every defect found in the originals and pipelines, by class, with what larmorx does and the D13 gaps (its AFNI and FSL sections are **forbidden to clean-room implementers**) |
| [afni-tshift.md](afni-tshift.md) | AFNI 3dTshift: slice-timing correction as fMRIPrep runs it, AFNI's FFT (source-derived; **forbidden to clean-room implementers**) |
| [fsl-mcflirt.md](fsl-mcflirt.md) | FSL mcflirt: head-motion correction as fMRIPrep runs it (source-derived; **forbidden to clean-room implementers**) |
| [itk-hdf5.md](itk-hdf5.md) | ITK's HDF5 transform files: what ITK reads and refuses, what it writes, what HDF5 1.12.1 makes of it byte for byte (file space, object headers, dense groups), zlib-ng's CPU-dependent compression, h5py and nitransforms as readers |
| [ants-cli.md](ants-cli.md) | ANTs command lines: argument parser, `antsApplyTransforms` options and outputs |
| [ants-image-programs.md](ants-image-programs.md) | ImageMath, ThresholdImage, MultiplyImages: dispatch, exit codes, operands, quirks of each operation |
| [ants-gaussian-filters.md](ants-gaussian-filters.md) | ITK's recursive and discrete Gaussian filters; ImageMath `G`, `Laplacian`, `Grad`, `UnsharpMask`; SmoothImage; ResampleImageBySpacing; ResampleImage (pixel types, interpolators, argument quirks) |
| [ants-morphology.md](ants-morphology.md) | ITK's ball and binary/grayscale morphology; ImageMath `MD`/`ME`/`MO`/`MC`/`GD`/`GE`/`GO`/`GC`, `FillHoles`, `PadImage`; connected components and relabelling, `GetLargestComponent`; Danielsson and signed Maurer distance maps (`D`, `MaurerDistance`); `ExtractContours`, `ThresholdAtMean`, `ReplaceVoxelValue` |
| [ants-n4.md](ants-n4.md) | N4BiasFieldCorrection and ITK's N4 filter: how fMRIPrep calls it, the thread dependence of the B-spline fit, GCC 11's miscompile of the fit (GCC bug 110182) and the oracle build that honours the source, histogram sharpening quirks, vnl's FFT, lattice refinement, `ShrinkImageFilter`, glibc's `expf`/`logf` |
| [ants-denoise.md](ants-denoise.md) | DenoiseImage and ITK's adaptive non-local means filter (remote module AdaptiveDenoising): how sMRIPrep calls it, the large and run-dependent thread band, the mask on the shrunk grid, the Rician bias image, the noise resampled with float coordinates, ITK's `floor` of tiny negative values, glibc's `expf` |
| [ants-float.md](ants-float.md) | `antsApplyTransforms --float 1`: what ITK's float instantiation computes in float and in double (points, transforms, inverses, versors, fields, transform files, every interpolator, the output cast), a versor that fails to load in float, glibc's `sinf`/`cosf` |
| [ants-registration.md](ants-registration.md) | antsRegistration's linear stages on ITK's v4 framework: seeds, the thread band, the metrics, sampling, scales and the optimizer, the outputs |
| [ants-ai.md](ants-ai.md) | antsAI and antsAffineInitializer: the initial transforms, Wahba's rotation, antsAI's sampling (`Point<float>`, the perturbations, the clock), the gradient filter's float components, the start points, `MultiStartOptimizerv4` (ITK-19, ITK-20), antsAffineInitializer's arguments, scales and carried learning rate, fMRIPrep's own runs |
| [itk-transforms.md](itk-transforms.md) | ITK transforms: composite order, inverses, displacement fields, transform files |
| [itk-resampling.md](itk-resampling.md) | ITK `ResampleImageFilter` and the interpolators ANTs uses |
| [itk-nifti.md](itk-nifti.md) | ITK's NIfTI reader and writer: geometry, scaling, non-finite values, vector images, the header it writes |
| [antspy.md](antspy.md) | ANTsPy as an oracle: defaults and quirks of its wrappers |
| [nibabel.md](nibabel.md) | nibabel as the reference for NIfTI I/O |
| [platform-math.md](platform-math.md) | `exp`/`log`/`sin`/`cos`/`powf`/`expf`/`logf`: glibc vs the `libm` crate and CORE-MATH, what it does to bit parity, the preload library that makes ANTs correctly rounded |
| [x87-extended.md](x87-extended.md) | the x87's 80-bit `long double` as GCC's x86-64 code computes it: Linux's control word, single rounding with the extended range, Intel's NaN rules on an AMD Zen 4, the real and integer indefinites, the conversions GCC 8.4.0 emits, `fsqrt`/`sqrtl`/`__builtin_sqrtl`, the patterns that need the extended precision (black-box; open to everyone) |
| [vnl-random.md](vnl-random.md) | VXL's `vnl_random` (ITK 4.13.2) and the C library's `drand48` family: `drand64`'s left-to-right draws in GCC 8.4.0's code, two VXL defects, the seeds' low 32 bits, glibc's logarithm in `normal()`, glibc's unseeded state (source-derived from VXL: closed to a clean room of `vnl_random`) |
| [vnl-netlib.md](vnl-netlib.md) | VXL's VNL and netlib routines in ITK 4.13.2: the ports' bit-identity, VXL's L-BFGS first stage (ITK-103, measured), its printing defects, NaN signs where two NaNs meet, x86-64's NaNs in the minimizers on costs that fall without end, the values VNL never writes, the loops that never end (ITK-109, ITK-112, ITK-114 to ITK-116: Brent's loop, `pythag`, `ssvdc`), smaller facts (source-derived from VXL: closed to a clean room of these routines) |
| [minc-xfm.md](minc-xfm.md) | MINC 1.5.1's `.xfm` reader and writer: the upstream, the tokenizer's rules (tabs, comments, NUL bytes, glibc's `sscanf`), FS-71..75, the numbers against MINC, the gaps (MINC volumes) (source-derived from MINC: closed to a clean room of these routines) |
| [nifti-znz.md](nifti-znz.md) | nifticlib 2.0.0's file names and znzlib over zlib 1.2.11's file layer: the upstream, zlib's reading of transparent, multi-member, truncated and damaged files, FS-76..78, the compressed bytes (zlib-rs's) (source-derived from nifticlib and zlib: closed to a clean room of these routines) |
| [gifti.md](gifti.md) | gifticlib 1.12 and expat 2.0.1: the upstreams, why the XML parser is a translation of expat, the behaviour kept (character data's pieces, the default handler, internal subsets, encodings, line numbers), gifticlib's state between calls and messages, FS-80..89 (source-derived from gifticlib and expat: closed to a clean room of these routines) |
| [ctn-dicom.md](ctn-dicom.md) | CTN 3.0.6's `DCM_OpenFile`: the upstream, what it does with a file (options, elements, the dictionary, order, sequences, pixel data, values, the object, the 32-bit arithmetic, conditions and the condition stack, the process state behind `perror`'s texts), FS-90..98, the probe's numbers (source-derived from CTN: closed to a clean room of CTN's reading) |
| [nlohmann-json.md](nlohmann-json.md) | nlohmann/json 3.2.0: what it reads (positions, the last token read, the end, the byte-order mark, numbers, strings, objects) and writes (indentation, escapes and their buffer, Grisu2's digits and the exponent format), the accessors as programs use them, FS-99, the probe (source-derived from nlohmann/json: closed to a clean room of it) |
| [miniz-inflate.md](miniz-inflate.md) | miniz 1.15's inflater: the decoder's resumption and read-ahead, the stream API's dictionary and return codes, headers, FS-281..284, the probe (source-derived from miniz: closed to a clean room of miniz's inflater) |
| [freesurfer-permissive.md](freesurfer-permissive.md) | FreeSurfer 8.2.0's side of the permissive pieces (x87, `vnl_random`, `drand48`, VNL and netlib, MINC's `.xfm`, nifticlib and znzlib, GIFTI and expat, CTN's DICOM library, nlohmann/json): which of its functions call them, its seeds and settings, its copies' changes measured by behaviour, the probes that name its internals (closed to FreeSurfer's clean-room implementers) |
| [scipy-ndimage.md](scipy-ndimage.md) | SciPy's `map_coordinates` and spline prefilter: padding, boundary rules, summation order, undefined behaviour |
| [fmriprep-resampling.md](fmriprep-resampling.md) | fMRIPrep's one-shot resampler and nitransforms: float32 coordinates, BLAS fused products, voxel-shift map, Jacobian, displacement fields |
| [fmriprep-oracle.md](fmriprep-oracle.md) | fMRIPrep 25.2.6 as it actually ran (the first oracle run): the tools and versions in its image, thread counts (`NSLOTS`), seeds, the parameters nipype passes or drops, headers it rewrites, what reproduces run to run; its N4 binary follows the C++ source where the GCC 11 oracle does not; its one-shot resampler races with multi-threaded OpenBLAS |
| [numpy-blas.md](numpy-blas.md) | numpy's 4×4 products and inverses: which BLAS/LAPACK routine, OpenBLAS's kernels per CPU, the Haswell operation order larmorx reproduces, how often the kernels differ |
| [machine-move-proof.md](machine-move-proof.md) | the move to the new main machine (AMD, AVX-512): the originals and larmorx compute alike on both machines; numpy's OpenBLAS kernel pinned for the validation; the tests that depend on the machine's memory |
| [afni-masks-clean-room.md](afni-masks-clean-room.md) | the clean rooms of 3dAutomask and 3dUnifize: what black-box runs showed (AFNI's own header extension in fMRIPrep's input, AFNI-10; glibc's `tanhf` and 3dUnifize's squash; messages), and how the clean room computes the same results faster (unrestricted) |
| [confounds-workflow.md](confounds-workflow.md) | fMRIPrep's confound estimation (`init_bold_confs_wf`): what it runs and which oracle runs reach each path, the wiring's defects (PIPE-41, -42, -49, -79), nipype's unquoted file names (PIPE-80), fMRIPrep's 36 confounds files column by column, what the newest libraries and larmorx's corrections change, the golden and memory findings, the follow-ups |
| [confounds-dvars.md](confounds-dvars.md) | nipype's DVARS with nitime's AR(1) and SciPy's 1 × 1 solve: what decides the bits, PIPE-46, -47, -50, SCI-41, the step from fMRIPrep 25.2.6's libraries per real call |
| [confounds-compcor.md](confounds-compcor.md) | nipype's CompCor, TSNR, NonSteadyStateDetector and FramewiseDisplacement (with nipy's `NIPY` source): PIPE-44, -45, -48 (the retries measured), -51 to -60, the correctly rounded cosine's sign flips on the real components files |
| [confounds-pandas.md](confounds-pandas.md) | pandas' float parser and ujson's formatting under fMRIPrep's table steps (PD-01 to PD-03), the two pandas releases, `GatherConfounds`' row order (PIPE-63), `AddTSVHeader` (PIPE-61, -62) |
| [confounds-model.md](confounds-model.md) | niworkflows' model expansion and spike regressors: the hash-seeded column order (PIPE-43), the formula's defects (PIPE-64 to -70), numpy's powers |
| [confounds-nipype-cli.md](confounds-nipype-cli.md) | nipype's command line `nipypecli run` on the confounds' interfaces: how it builds parsers from traits, the compound inputs' leaked types and the parsers it cannot build (PIPE-81), texts not read by their traits (PIPE-82), its kept limits (PIPE-83), fMRIPrep's own calls through it |
| [confounds-motion.md](confounds-motion.md) | fMRIPrep's motion parameters (both releases' forms) and RMS deviation: nitransforms' reading, SciPy's `Rotation`, glibc's `atan2`/`hypot` on the real tables (D4), PIPE-71 to -73 |
| [confounds-masks.md](confounds-masks.md) | fMRIPrep's aCompCor masks, the crown, niworkflows' morphology and tissue signals: numpy's undefined casts (PIPE-74, -76), PIPE-75, -77, -78, the FreeSurfer path fed FAST's maps (PIPE-79), scikit-image's negative balls |
| [confounds-blas.md](confounds-blas.md) | OpenBLAS's kernels and LAPACK's routines under numpy's `svd`/`pinv`/`lstsq` and SciPy's `gesvd`: the Haswell arithmetic, the depth blocks of 512 and numpy 2.5.4's crash on AMD Zen 4/5 (NP-09, `OPENBLAS_L2_SIZE`), what LAPACK 3.12.0 and 3.12.1 change, numpy 2.3's hang on infinities (NP-10), CORE-MATH's and glibc's `hypot` |
| [confounds-numpy.md](confounds-numpy.md) | numpy's layouts and summation orders at the confounds' call sites, numpy 2.2's 8,192-element chunks, whose maths numpy's `cos`, `exp`, `power` and complex64 product are (glibc, SVML, fused), `min`/`max` signed zeros, out-of-range casts, `savetxt`'s line ends |
| [confounds-fft.md](confounds-fft.md) | SciPy 1.15's pocketfft and 1.18's ducc0 in single precision under nitime: the twiddles (glibc's `sincos` never shows), the libraries' algorithms and where their bits part, NaN signs |

Defects that affect an upstream project's users and are solid enough to file are written up as
reports in [../upstream-reports/](../upstream-reports/README.md).
