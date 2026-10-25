# Upstream defects: the register

Every defect or questionable behaviour found so far in the tools and pipelines larmorx
reproduces: ANTs and ITK, AFNI, FSL, fMRIPrep and its libraries (nipype, niworkflows,
sMRIPrep, sdcflows, nitransforms), SciPy, numpy and OpenBLAS, nibabel, glibc and ANTsPy. For
each: the evidence, what it does to fMRIPrep users, what larmorx does today, and what decision
D13 ("correct by default, faithful on request", CLAUDE.md) requires. Collected 2026-10-11 from
the other files in `docs/findings/`, the specs, the validation records and `docs/analysis/`.
Add an entry whenever you find a defect ([porting guide](../porting-guide.md) §8); entries are
corrected, never deleted.

> **Restricted entries (†).** The sections "Restricted: AFNI 3dTshift" and "Restricted: FSL
> mcflirt" were written from AFNI's MCW (GPL) and FSL's (non-commercial) source, through
> [afni-tshift.md](afni-tshift.md) and [fsl-mcflirt.md](fsl-mcflirt.md); the section "Restricted: AFNI 3dAutomask and
> 3dUnifize" through [afni-automask.md](afni-automask.md) and [afni-unifize.md](afni-unifize.md). **Clean-room
> implementers must not read those sections.** The summary table and the gap lists below
> describe every entry, † included, by its observable behaviour only, as a spec would (the
> spec writer may read the source, the implementer reads the spec; [licensing.md](../licensing.md)),
> so anyone may read them.

**Classes:**

| Class | Meaning | D13 |
|---|---|---|
| (a) | crash, undefined behaviour, read out of bounds | never reproduced |
| (b) | race or nondeterminism | never reproduced |
| (c) | deterministic bug that changes results | fixed by default, warning when it would have changed the result, compatibility switch that reproduces it |
| (d) | imprecision (e.g. not correctly rounded) | not D13: rule 5 and D11 |
| (e) | command-line or parsing quirk | as (c) |
| (f) | documentation or message only | not D13 |

**c?** marks a *questionable* behaviour: possibly intended, possibly a bug. The user decides
before anything changes.

**Fields of each entry:** the tool and version; what is wrong; the evidence (*read* from the
source, *verified* by running the tool, *validated* in a parity suite; file:line at the pinned
tags of `UPSTREAM.md`); the effect on fMRIPrep users; what larmorx does today; D13 and the gap.

## Summary

Gap: **yes** = a class (c) or (e) defect larmorx reproduces by default today (or, for (a),
reproduces at all); **decide** = needs a user decision first; **rule 8** = to follow the
source (decided); **replica** = corrected in the clean-room original, the replica's switch
still to do; **original** = corrected by default in both 3dTshift implementations, but the
clean-room original's `reproduce` did not give AFNI's result yet (none left since 2026-10-12:
completing 3dTshift, part 2b, then AFNI-26's switch the same day); **switch**, **notice**,
**reproduce** = the default is already correct, the compatibility switch or the notice is still
to add (since 2026-10-25: "Random testing's gaps", below); – = nothing to do. How the switch works: [api/upstream-defects.md](../api/upstream-defects.md).

| ID | Tool | Defect | Class | fMRIPrep users | larmorx today | Gap |
|---|---|---|---|---|---|---|
| ANTS-01 | ANTs parser | some bracket forms and non-integer values rejected | e | none | same rejections | – |
| ANTS-02 | ANTs programs | crash or abort on missing files, missing arguments, text for numbers | a | none | refuses (exit 1) | – |
| ANTS-03 | ANTs I/O | names of 3 characters or fewer not read or written, exit 0 | e | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-04 | ImageMath | exit 0 after a failed operation | e | hides failures | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-05 | ImageMath | operand parsing: `7 ` is a file, `1e` is 0, no operand is 1 | e | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-06 | ANTs programs | out-of-range float-to-integer conversions answered as x86-64 does (undefined) | a | none seen | clamps and warns by default; `reproduce` wraps as x86-64 does (2026-10-12) | – |
| ANTS-07 | ImageMath | a smaller second image read out of bounds | a | none | refuses; `reproduce` reads inside the buffer, 0 beyond, and warns (2026-10-12) | – |
| ANTS-08 | ImageMath `/` | divisor ≤ 0: the previous voxel's result | c | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-09 | ImageMath `total`, `mean` | the image holds running sums | c | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-10 | ImageMath `vtotal` | all-zero output | c | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-11 | ImageMath `Neg` | range missed on increasing images | c | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-12 | ImageMath `TruncateImageIntensity` | brightest voxel can fall outside the histogram; 64 bins, usage says 65 | c, f | possible | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-13 | ImageMath `Normalize` | range starts at 0 and 1e9 | c | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-14 | ImageMath `G`, DenoiseImage's Rician bias (ITK) | variance above ≈709 voxels² gives NaN | c | none | corrected by default (`upstream_defects`, 2026-10-12); in DenoiseImage named in both modes, `reproduce` keeps ITK's NaN kernel (2026-10-25) | – |
| ANTS-15 | ImageMath `G`, SmoothImage | wrong number of sigmas: unchanged image or 1 mm, exit 0 | e | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-16 | ImageMath `Laplacian` | normalize flag read from the sigma argument | e | none (same result) | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-17 | ImageMath `MO`, `ME` | `MO r 0` is a dilation of zeros; `ME` keeps other labels as 1 | c (c? until 2026-10-23) | none | corrected by default: `ME r v` gives the eroded mask of `v`, `MO r 0` opens the zeros (2026-10-23) | – |
| ANTS-18 | ImageMath `GD`, `GC` (ITK) | sign of a zero extreme depends on threads | b | none | defined signs | – |
| ANTS-19 | ImageMath `FillHoles` | `holeparam ≤ 1` reads outside the image | a | none | outside neighbours not counted by default; `reproduce` reads inside the buffer as ANTs, background beyond (2026-10-12) | – |
| ANTS-20 | ImageMath `GetLargestComponent` | nothing kept: every voxel becomes 1 | c | possible | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-21 | ImageMath `ClusterThresholdVariate` | reads outside the image | a | none | not ported | – |
| ANTS-22 | ImageMath `PadImage` | fractional pad grows by ⌊2p⌋, shifts by ⌊p⌋; a negative one also shifts the data one voxel against the origin and leaves the last row the pad value | c (c? until 2026-10-23) | none | corrected by default: `trunc(p)` voxels on each side (2026-10-23) | – |
| ANTS-23 | ImageMath `ExtractContours` | labels are truncated values | c (c? until 2026-10-23) | none | corrected by default: every value its own label (2026-10-23) | – |
| ANTS-24 | ThresholdImage | unknown mode names become thresholds at 0; sixth argument read as a file | e, f | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-25 | ThresholdImage Otsu with a mask | range from the smallest positive float, `else if` | c | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-26 | ThresholdImage `Otsu 0` | never returns | a | none | refuses | – |
| ANTS-27 | MultiplyImages | unreadable second file multiplies by 0, silently | e | possible | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-28 | ResampleImageBySpacing, ResampleImage, SmoothImage | uninitialised values reach the output | a | none | refuses; `reproduce` fills the unwritten buffer with zeros and warns (2026-10-12) | – |
| ANTS-29 | ResampleImage | size 1: infinite spacing, a zero image, exit 0 | c | none | refuses; `reproduce` writes ANTs' image byte for byte (2026-10-12) | – |
| ANTS-30 | ResampleImage | seventh argument read twice | e | none | corrected by default (`upstream_defects`, 2026-10-12); the command-line design approved by the user (2026-10-22) | – |
| ANTS-31 | ResampleImageBySpacing | `nn` read from the wrong argument; dimension 5 does nothing | e | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-32 | ResampleImageBySpacing | sigma `out/in − 1` taken in mm; outside points get the value at index 1 | c (c? until 2026-10-23) | none | corrected by default: the sigma in input voxels, outside points 0 (2026-10-23) | – |
| ANTS-33 | antsApplyTransforms | unknown `-u` ignored; interpolator parameters ignored | e | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-34 | antsApplyTransforms `GenericLabel` (ITK) | never finishes on intensity images | a | none | neighbour labels only, same result | – |
| ANTS-35 | N4BiasFieldCorrection | `-c`, `-b`, `-t` without brackets set nothing | e | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-36 | N4BiasFieldCorrection | unreadable mask becomes ones; `-r` without a mask does nothing | e | `-r` has no effect | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-37 | N4 (ITK) | result depends on the thread count | b | anatomical N4s | single-thread result | – |
| ANTS-38 | N4 (ITK) | histogram range `else if`: a first-voxel minimum is dropped | c | possible, every N4 | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-39 | N4 (ITK) | 256 bins give a 1024-point FFT | c | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-40 | N4 (ITK) | zero and negative values stay in the log image | c (c? until 2026-10-23) | 16 of the 25 N4 calls of the five oracle runs use such voxels (every final anatomical N4); on ds005454 sub-16 ANTs' final N4 corrects nothing | corrected by default: left out of the histogram and the fit (measured, then rule A; 2026-10-23) | – |
| ANTS-41 | ANTs built by GCC 11.4 / 12.3 (compiler bug, not ANTs) | two float roundings of N4's fit dropped (GCC bug 110182) | d | none (fMRIPrep's build follows the source) | follows the source | – (rule 8) |
| ANTS-42 | DenoiseImage (ITK) | large thread dependence, races | b | every run | single-thread result | – |
| ANTS-43 | DenoiseImage (ITK) | Rician correction NaN at high SNR becomes a bias of 0 | c | sMRIPrep's T1w (7-8 % of voxels) | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-44 | DenoiseImage (ITK) | bias outside the mask keeps an unsmoothed value | c (c? until 2026-10-23) | none (no mask) | corrected by default: the smoothed and corrected bias wherever a patch of a centre in the mask reaches (2026-10-23) | – |
| ANTS-45 | N4, DenoiseImage, antsRegistration | errors explained only with `-v` | f | none | always explained | – |
| ANTS-46 | antsRegistration | BOLD reference's affine stops at 25 of 200 iterations | – | every run | explained (A4 part 2): ITK's window convergence test passes when the metric stops improving; not a defect | – |
| ANTS-47 | DenoiseImage | an unreadable mask is no mask, exit 0 | e | none | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ANTS-48 | antsApplyTransforms | transform outputs: crashes and aborts (an unknown extension, `-i` without `-r`, a Gaussian interpolator without `-i`, a collapsed `IdentityTransform`) | a | none | refuses (exit 1) | – |
| ANTS-49 | antsRegistration | a linear stage that throws is dropped and the run goes on, exit 0 | e | none seen | corrected by default: an error (A4 part 2) | – |
| ANTS-50 | antsRegistration | `--save-state` after linear stages only: exit 1 without a message, no transform written | e | none | corrected by default: the state is written (A4 part 2) | – |
| ANTS-51 | antsRegistration | transform files without collapsing: initial transforms skipped by counting `-r` options, each file named after the next stage; a composite `-r` file or a restored state gives a misnamed file, then a read past the end of the names (crash) | a | none (fMRIPrep collapses) | corrected by default; `reproduce` writes ANTs' files up to the crash and stops (A4 part 2); a Gaussian or B-spline smoothing field that ANTs misnames `.mat` or `.txt` written with its own class (2026-10-25) | reproduce (a velocity field under such a name) |
| ANTS-52 | antsAI | the translation grid does not move `Rigid`/`Similarity` start points in 2D nor `Similarity` ones in 3D | c | none (fMRIPrep's grids are for `Affine`) | corrected by default (A4 part 3) | – |
| ANTS-53 | antsAI | `AlignGeometricCenters`/`AlignCentersOfMass` with iterations write an empty `.mat`, exit 0 (`.txt`, `.h5`: a crash) | e | none | corrected by default: the alignment is written (A4 part 3) | – |
| ANTS-54 | antsAffineInitializer | the local-search argument is never read: 20 iterations whatever it says | c (c? until 2026-10-17) | niworkflows passes 10, ANTs runs 20 | corrected by default: the argument is honoured (0: no local search), a notice where it is not 20 (2026-10-17) | – |
| ANTS-55 | antsAI | a translation grid with fewer extents than dimensions is read past the end of its vector | a | none | refuses; `reproduce` reads the missing extents as 0 and warns (A4 part 3) | – |
| ANTS-56 | antsAffineInitializer | a dimension other than 2, 3 or 4 runs as 2D | e | none | corrected by default: an error (A4 part 3) | – |
| ANTS-57 | antsAffineInitializer | two or three arguments abort (`std::string` of a null pointer) instead of printing the usage | a | none | prints ANTs' message, exit 1 | – |
| ANTS-58 | antsAI | `Random` sampling (always) and `--random-seed 0` draw from the clock: no two runs agree | b | none (fMRIPrep uses `Regular` with a seed) | seeded from the seed: repeatable | – |
| ANTS-59 | antsAI | `--convergence`'s window defaults to 5; the help says 10 | f | none | follows the code; documented | – |
| ANTS-60 | antsAI `-b` (ANTs' blob detector) | with more than one thread, per-thread heaps sized by the thread count are indexed by work units, four times as many: `antsAI -b` crashes (signal 11) at 2, 4 and 6 threads, every run | a | none | the one-work-unit result at any thread count (2026-10-23) | – |
| ANTS-61 | antsRegistration IGDM | a stage that combines IGDM with ICP, PSE or JHCT, or lists it after an image metric, hands only one kind of point set to ITK: "Mismatch between number of image pairs and the number of metrics" (the stage dropped, ANTS-49) or a crash | c | none (fMRIPrep uses no point-set metric) | corrected by default: every metric gets its inputs, a notice; `reproduce` throws ITK's exception, or stops where ANTs crashes (A4 part 4) | – |
| ANTS-62 | antsRegistration point-set text reader | a file that ends with a newline gets one more record, from variables the failed read leaves (in the oracle's build a copy of the last point) | a | none | corrected by default: no extra record, a notice; `reproduce` adds the copy of the last record, as the oracle's build, with a notice (A4 part 4) | – |
| ANTS-63 | antsRegistration VTK reader | a binary file's labels are read only as far as one point's components: the other labels are uninitialised memory | a | none | corrected by default: every label read; `reproduce` reads ANTs' bytes and takes the unread labels as 0, with a notice (A4 part 4) | – |
| ANTS-64 | antsRegistration point sets | a sampling percentage below 1 keeps a random fraction drawn with a generator seeded from the clock: no two runs agree | b | none | seeded from the registration's seed (121212 without one): repeatable | – |
| ANTS-65 | antsRegistration IGDM | the estimated intensity sigma ignores both point sets' minima (never updated: `else if`) and subtracts the moving maximum (`std::min(minMoving, maxMoving)`): the difference of the maxima, or the larger maximum | c | none | corrected by default: the range of both sets' intensities, a notice with both values; `reproduce` as ANTs (A4 part 4) | – |
| ANTS-66 | antsRegistration IGDM | the value (the line search's) computes the neighbourhood's centre from `-0.5` times its voxel count: for a radius of 1 or more, a negative index and a read before the intensity arrays | a | none | corrected by default: the centre as the derivative reads it; `reproduce` takes the two intensities as equal, with a notice (A4 part 4) | – |
| ANTS-67 | antsRegistration IGDM | the `kNeighborhood` and `gradientSigma` arguments are parsed and never used (central differences, one neighbour) | c (c? until 2026-10-21) | none | corrected by default: the mean over the k nearest moving points, Gaussian gradients of `gradientSigma` mm, a notice; `reproduce` as ANTs (A5 part 3) | – |
| ANTS-68 | antsRegistration IGDM | the derivative lacks the Gaussians' `1/σ²` factors, so it is not the value's gradient and its two terms are weighed by `σ²` | c (c? until 2026-10-21) | none | corrected by default: the value's gradient, a notice; `reproduce` as ANTs (A5 part 3) | – |
| ANTS-69 | antsRegistration point sets | crashes and a hang: no fixed mask, warped outputs, `--print-similarity-measure-interval` or `--write-interval-volumes` with a point-set metric first, an unreadable point set, image sampling in a stage with a point-set metric, NaN parameters; a text file with a word among its numbers loops forever | a | none | refuses with a message where ANTs crashes (its output up to there); refuses a hanging read | – |
| ANTS-70 | antsRegistration IGDM | the estimated distance sigma is the standard deviation of the nearest distances: 0 when they are all equal (identical point sets), and the registration's parameters become NaN | c (c? until 2026-10-21) | none | corrected by default: at least one voxel of the virtual domain, a notice where the floor applies; `reproduce` as ANTs (the NaN parameters throw, the stage is dropped) (A5 part 3) | – |
| ANTS-71 | antsRegistration SyN | `--print-similarity-measure-interval` crashes without an initial fixed transform (`-q`), without a moving transform before the stage, or with several metrics (null pointers in the observer) | a | none | stops with a message where ANTs crashes; corrected, the missing transforms are the identity and the first metric's images are used (A5 part 1) | – |
| ANTS-72 | antsRegistration | a linear stage after a displacement field (a SyN stage, `-r field`) estimates its parameter scales and learning rate from 1000 voxels drawn with a generator ITK seeds from the clock: not repeatable, whatever the seed | b | none (fMRIPrep's linear stages come first) | draws them with the stage's seed | – |
| ANTS-73 | antsRegistration | an initial transform image that is not a field of the registration's dimension (a scalar image, a 2D field in 3D) is read as one: ITK converts the pixels as gray, complex, RGB or RGBA values | e | none | corrected by default: refused; `reproduce` converts as ITK (A5 part 1) | – |
| ANTS-74 | antsRegistration | `--save-state` after linear stages that follow a SyN stage (or a restored state) saves that SyN state, without the later stages | c | none (fMRIPrep does not save states) | corrected by default: every transform, as after linear stages (ANTS-50); `reproduce` as ANTs (A5 part 1) | – |
| ANTS-75 | antsRegistration | `--print-similarity-measure-interval` and `--write-interval-volumes` copy the stage's transforms by setting each copy's parameters before its fixed parameters: a displacement field, or a B-spline transform of another mesh than 1, refuses, and the stage fails at its first iteration | c | none (fMRIPrep sets neither option) | corrected by default: the transforms are used as they are, a notice; `reproduce` fails as ANTs (A5 part 2) | – |
| ANTS-76 | antsRegistration | collapsing the output transforms (`-z 1`, the default) around B-spline transforms loses transforms: one that follows only B-spline transforms, a B-spline transform after another at the end of the queue; before more linear transforms or fields that B-spline transform joins their run and ANTs crashes | c | none (fMRIPrep uses no B-spline transform) | corrected by default: every transform kept, linear and field runs collapsed around them, a notice; `reproduce` collapses as ANTs, stops where it crashes (A5 part 2) | – |
| ANTS-77 | antsRegistration `BSpline` | a mesh size of 0, or fewer mesh sizes than dimensions (read past the end of the list), divides by zero: a grid of NaN and infinite geometry, vnl's SVD warnings, an exception on garbage indices | a | none | refused with a message (A5 part 2) | – |
| ANTS-78 | antsRegistration `BSplineDisplacementField`, `BSplineExponential` | the meshes "at the base level" are doubled already at the base level (`mesh << (level + 1)`; B-spline SyN uses `mesh << level`) | c (c? until 2026-10-21) | none | corrected by default: `mesh << level` (twice the mesh gives ANTs' result byte for byte), a notice; `reproduce` as ANTs (A5 part 3) | – |
| ANTS-79 | antsRegistration time-varying stages | `TimeVaryingVelocityField`'s number of time indices and `TimeVaryingBSplineVelocityField`'s mesh are read from the last `-t` option on the command line, not the stage's own | e | none | corrected by default: the stage's own, a notice where they differ; `reproduce` as ANTs (A5 part 3) | – |
| ANTS-80 | antsRegistration `Exponential`, `BSplineExponential` | the smoothing sigmas are taken as millimetres even when given in voxels (the stage never passes their units to its method) | c | none | corrected by default: the units as given, a notice; `reproduce` as ANTs (A5 part 3) | – |
| ANTS-81 | antsRegistration `Exp` | without collapsing, a stage given the short name `Exp` writes no `InverseWarp` (the list of transforms with an inverse has only the long names) | e | none | corrected by default: written, a notice; `reproduce` as ANTs (A5 part 3) | – |
| ANTS-82 | antsRegistration `Exponential`, `BSplineExponential`, `TimeVaryingVelocityField` | the stages never set their metric's sampling seed: a sampled metric is seeded from the clock, whatever `--random-seed` or `ANTS_RANDOM_SEED` say | b | none | the registration's seed: repeatable (A5 part 3) | – |
| ANTS-90 | Atropos | `KMeans` with several images: the second k-means starts from the mask's statistics and its labels are never read, so only the first image decides the initial labelling | c | none (sMRIPrep segments one image) | corrected by default (A6) | – |
| ANTS-91 | Atropos | `Otsu`'s range starts at `FLT_MIN` and uses `else if`: a decreasing or negative image gets a wrong range | c | none | corrected by default (A6) | – |
| ANTS-92 | Atropos | `Random` labels voxels outside the mask too, and they enter the first estimates | c | none | corrected by default (A6) | – |
| ANTS-93 | Atropos | the adaptive smoothing weights of several `-a` apply in reverse order | c | none | corrected by default (A6) | – |
| ANTS-94 | Atropos | the outlier filters keep their previous output: box-plot bounds from earlier values too, Grubbs-Rosner's later images get the first image's values and its sample grows | c | none (sMRIPrep uses no outlier handling) | corrected by default (A6) | – |
| ANTS-95 | Atropos | `ManifoldParzenWindows` ignores the posterior weights: every class has the same density | c | none | corrected by default (A6) | – |
| ANTS-96 | Atropos | partial-volume classes are never re-estimated | c | none | corrected by default (A6) | – |
| ANTS-97 | Atropos | `-u 1` stores the priors' voxel numbers in `float` (rounded above 2^24 voxels) | c | none | corrected by default (A6) | – |
| ANTS-98 | Atropos | `KMeans` takes the first image's range from the mask's label 1 only: a mask of 255s gives infinite centres and an error | c | none (fMRIPrep's masks are 0 and 1) | corrected by default (A6) | – |
| ANTS-99 | Atropos, N4BiasFieldCorrection, DenoiseImage | without `-d` every image is read as 3D (Atropos segments it in 3D) | e | none (nipype passes `-d`) | corrected by default (A6; N4BiasFieldCorrection and DenoiseImage since 2026-10-25, named in both modes) | – |
| ANTS-100 | Atropos | priors given as one vector image: every class gets the last component | c | none | corrected by default (A6) | – |
| ANTS-101 | Atropos | an unknown `-p` formulation silently runs Socrates | e | none | corrected by default: an error (A6) | – |
| ANTS-102 | Atropos | `--icm [async,iterations]`, the documented form (and what nipype's `icm_use_synchronous_update` writes), aborts | a | none (sMRIPrep does not set it) | read as documented; `reproduce` stops with ANTs' message (A6) | – |
| ANTS-103 | Atropos | `JointShapeAndOrientationProbability` evaluates its histograms at the raw components and an unset coordinate | a | none | corrected by default: at the tensor's eigenvalues and direction; `reproduce` takes the unset coordinate as 0, with a notice (A6) | – |
| ANTS-104 | Atropos | `ManifoldParzenWindows`' local covariances double their running sums | c | none | corrected by default (A6) | – |
| ANTS-105 | Atropos | `-u 1` with prior probability images and a mask crashes | a | none | runs; `reproduce` stops where ANTs crashes (A6) | – |
| ANTS-106 | Atropos | partial-volume classes with their likelihoods and an MRF hardly win a voxel (a partial-volume label is +1 against every neighbour): Atropos stops when one wins none | c (c? until 2026-10-22) | none | corrected by default: a partial-volume label is a member of its own set (2026-10-23) | – |
| ANTS-107 | Atropos | with Grubbs-Rosner (ANTS-94) the grown sample is weighed with uninitialized memory: the result depends on the heap | a | none | not met by default (ANTS-94 corrected); `reproduce` weighs those samples 0, with a notice (A6) | – |
| ANTS-110 | antsAI `-b` | the blob alignment keeps the pairs whose displacements have about the same length as three others', not those whose distances to three others are preserved (as its comment and ImageMath's `MatchBlobs` say): any rotation leaves no landmark | c | none (fMRIPrep does not align blobs) | corrected by default (2026-10-23) | – |
| ANTS-111 | antsAI `-b` | once no positive patch correlation is left, the pairing takes the first fixed and moving blob again at every step | c | none | corrected by default: the pairing stops (2026-10-23) | – |
| ANTS-112 | antsAI `-b` (the blob detector) | not the N strongest maxima: the threshold of later scales is set while fewer than N are kept, and a heap that has just filled lets a weaker maximum push out its weakest | c | none | corrected by default (2026-10-23) | – |
| ANTS-114 | antsAI `-b` | a rigid or similarity alignment with no landmark writes a transform of NaNs (exit 0) | c | none | corrected by default: an error (2026-10-23) | – |
| ANTS-115 | antsAI `-b` | an empty blob list (a constant image) prints its message, then dereferences a null transform (crash) | a | none | refuses with ANTs' message (exit 1) | – |
| ANTS-116 | antsAI `-b` | the patch rotation `V·Uᵀ` is not checked for a reflection, and the gradients' axes have arbitrary signs: identical patches can be compared reflected or turned (4 of the 13 identical blob pairs whose patches fit, on a T1w and its shifted copy, correlate at 1) | c (c? until 2026-10-23) | none | corrected by default: proper rotations, every sign combination of the axes, the best correlation (2026-10-23) | – |
| ANTS-160 | ResampleImage | Gaussian interpolation given fewer sigmas than dimensions reads the missing ones past the end of its list: exit 0, a result that depends on the heap (usually NaN: zeros in an integer type, with vnl's convergence messages) | a | none | refused in both modes, the message naming ANTS-160 (2026-10-25) | – |
| ANTS-161 | ResampleImage | Gaussian interpolation with a sigma or alpha of 0 or less (`2 0x0x0`): NaN weights, vnl's "Failed to converge" messages, an image of zeros or NaN, exit 0 | c (rule B, 2026-10-25) | none | refused in both modes (the interpolator rejects them) | switch (`reproduce`: ANTs' NaN image) |
| ANTS-162 | N4BiasFieldCorrection, ResampleImage, ResampleImageBySpacing | memory and time are sized from the image's physical extent with no check: N4's lattice has `ceil(extent / distance) + order` control points per axis, doubled at each fitting level, and its image is padded to a whole span; the resamplers' output grid is `extent / spacing` voxels per axis. A header in metres or micrometres, or a 4D image's time axis in seconds, makes a tiny input need gigabytes to terabytes, and the program fills the machine until it is stopped | c (rule C for the method, 2026-10-25) | none (fMRIPrep's calls: millimetres, 3D, lattices of a few thousand points) | the method kept (the same lattice, padding and grid; larmorx's N4 needs 12 bytes per control point against ANTs' 17); by default a guard computes the bytes before allocating and stops at once, with the GiB needed and the likely cause, when they exceed the machine's physical memory (or `LARMORX_MEMORY_LIMIT`); `reproduce` attempts it as ANTs does (2026-10-25) | – (a D11 option: a sparse lattice with virtual padding, the same result in memory proportional to the data) |
| ANTS-163 | ImageMath `Normalize`, `TruncateImageIntensity` | a mask whose size differs from the image's is read at the image's indices, outside the mask's buffer when it is smaller (memory it never wrote) | a | none | refused in both modes, the message naming ANTS-163 (2026-10-25) | – |
| ITK-01 | ITK `Math::Floor` | 0 instead of −1 for tiny negative values | c | none seen | corrected by default (`upstream_defects`, 2026-10-12); the notice is missing in antsRegistration's displacement-field lookups (random testing, 2026-10-25) | notice (registration) |
| ITK-02 | ITK reader | a file with more dimensions than `D` loses its direction | c | none seen | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ITK-03 | ITK relabelling | float labels stop at 2^24 | c | latent | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ITK-04 | ITK Danielsson map | approximate, order-dependent distances | d | none | reproduces | – (D11) |
| ITK-05 | ITK `vnl_erf` | relative error 3e-7 in Gaussian weights | d | Gaussian interpolation | reproduces | – (D11) |
| ITK-06 | nifti_clib | NaN and ±inf read as 0 | c? | none (no non-finite value in the 509 images the five oracle runs pass to ANTs programs) | nifti_clib's behaviour in both modes (measured 2026-10-23) | – (decided 2026-10-23, rule D then C: kept, documented) |
| ITK-07 | ITK NIfTI reader | integer data scaled in float32 | d | scaled inputs | reproduces | – |
| ITK-08 | nifti_clib | asked for `x.nii.gz`, opens `x.nii` | e | none | opens the named file; `reproduce` reads nifti_clib's (2026-10-12) | – |
| ITK-09 | ITK HDF5 writer | the same transform gives different bytes | b | none | byte-identical (object time 0) | – |
| ITK-10 | ITK `Versor<float>` | a versor axis of norm ≥ 1 can fail to load in float (`--float 1`) | c | none seen | corrected by default (`upstream_defects`, 2026-10-12) | – |
| ITK-11 | ITK `GaussianOperator` | Bessel coefficients inaccurate for variances above about 50 voxels² | d | none | reproduces (finite kernels) | – (D11) |
| ITK-12 | ITK HDF5 transform reader | `TransformType` read only as a variable-length string: a fixed-length one (h5py's `numpy.bytes_`) is refused | c | none (ITK writes variable-length strings) | corrected by default (`upstream_defects`, 2026-10-16) | – |
| ITK-13 | ITK HDF5 transform reader | a parameter dataset of the wrong class or rank: the message names `TransformType` and runs the path into "in HDF5 File" | f | none | names the dataset; `reproduce` gives ITK's text (2026-10-16) | – |
| ITK-14 | ITK HDF5 transform reader | parameters wider than 8 bytes (long double, quadruple) read through float | c | none | corrected by default (`upstream_defects`, 2026-10-16) | – |
| ITK-15 | ITK HDF5 transform writer | a transform without parameters cannot be written with compression, which ANTs always asks for ("H5Pset_chunk failed"); a file ITK cannot read is left | c | none | corrected by default (`upstream_defects`, 2026-10-16) | – |
| ITK-16 | ITK HDF5 transform reader | a null variable-length `TransformType` builds a `std::string` from a null pointer | a | none | refuses | – |
| ITK-17 | ITK `CompositeTransformIOHelper` | a component of another dimension is `static_cast` to the composite's: ANTs runs on, exit 0 | a | none | refuses | – |
| ITK-18 | ITK joint-histogram mutual information (ANTs' `MI2`) | each image's marginal density is the other's | c | none (fMRIPrep uses Mattes and CC) | corrected by default (A4 part 2) | – |
| ITK-19 | ITK `MultiStartOptimizerv4` | a start point that throws keeps the previous start point's value (0 for the first), which competes for the minimum | c | possible (two of fMRIPrep's BOLD `init_aff` start points throw) | corrected by default: start points that throw take no part (A4 part 3) | – |
| ITK-20 | ITK Mattes metric | a zero or negative joint-histogram spacing (one intensity, or none, in the analysis region) is refused and leaves the histogram without a buffer: the next evaluation crashes | a | none | refuses at the crash (A4 part 3) | – |
| ITK-21 | ITK `ConjugateGradientLineSearchOptimizerv4` in `antsAffineInitializer` | without a scales estimator the learning rate the line search leaves in one start point starts the next | c (c? until 2026-10-17) | every niworkflows initialization | corrected by default: each start point at the configured rate (0.1), a notice where the result changes (2026-10-17) | – |
| ITK-22 | ITK `LabeledPointSetToPointSetMetricv4` | label sets that differ: a warning, then "Label not found in common label set" for the first fixed point whose label the moving set lacks (the stage dropped, ANTS-49) | c | none | corrected by default: those points take no part (an error when no label is common), a notice; `reproduce` throws ITK's exception (A4 part 4) | – |
| ITK-23 | ITK `GaussianMembershipFunction` | a covariance equal to the identity (JHCT with a point-set sigma of 1, isotropic) keeps the univariate prefactor `1/√(2π)` in any dimension | c | none | corrected by default: the `D`-dimensional prefactor, a notice; `reproduce` as ITK (A4 part 4) | – |
| ITK-24 | ITK JHCT metric | the total number of points leaves out the fixed ones (`m_NumberOfValidPoints` is still 0 in `Initialize`) | c (c? until 2026-10-21) | none | corrected by default: each label's fixed and moving points, a notice; `reproduce` as ITK (A5 part 3) | – |
| ITK-25 | ITK `ManifoldParzenWindowsPointSetFunction` | fewer points than the covariance neighbourhood (5): the loop reads past the end of the list of neighbours | a | none | uses the neighbours there are, in both modes, with a notice (A4 part 4) | – |
| ITK-26 | ITK `ManifoldParzenWindowsPointSetFunction` | anisotropic covariances add each diagonal product twice (`Cout(m, n)` and `Cout(n, m)` with `n = m`) | c | none | corrected by default: once, a notice; `reproduce` as ITK (A4 part 4) | – |
| ITK-27 | ITK `CompensatedSummation` | adding one compensated sum to another adds its compensation first, then its sum | d | none | reproduces (one work unit) | – (D11) |
| ITK-28 | ITK JHCT metric | `alpha` is tested with `!= 1` for the prefactor and with `AlmostEquals` for the value: an alpha within 4 ulps of 1 takes the logarithm and divides by `alpha − 1` | c | none | corrected by default: both test `AlmostEquals`, a notice; `reproduce` as ITK (A4 part 4) | – |
| ITK-29 | ITK `ManifoldParzenWindowsPointSetFunction` | the kernel sigma multiplies the covariance unsquared (a variance of 10), where the regularisation sigma is squared | c (c? until 2026-10-21) | none | corrected by default: `σ²·I`, a notice; `reproduce` as ITK (A5 part 3) | – |
| ITK-30 | ITK `CorrelationImageToImageMetricv4` (ANTs' `GC`) | with a displacement field (SyN) every point's derivative is added to the first voxel's entries: only that voxel moves | c | none (fMRIPrep's SyN uses CC) | corrected by default: each voxel its own derivative; `reproduce` as ITK (A5 part 1) | – |
| ITK-31 | ITK `SyNImageRegistrationMethod` | a stage with only one of its two masks: the role without one keeps the previous evaluation's resampled mask | c | every run: sMRIPrep's brain extraction's SyN stage has a fixed mask only (corrected: the field moves by at most 0.18 mm, median 0.0004 mm, on ds000005) | corrected by default; `reproduce` as ITK (A5 part 1) | – |
| ITK-32 | ITK `LabeledPointSetToPointSetMetricv4` | appends per-label metrics at every `Initialize` and uses the first: in SyN every evaluation compares with the moving points as the first one mapped them | c | none | corrected by default; `reproduce` as ITK (A5 part 1) | – |
| ITK-33 | ITK `SyNImageRegistrationMethod` | with several metrics, the point-set metrics get identity transforms: they never see the initial transforms or the deformation | c | none | corrected by default (the composites); `reproduce` as ITK (A5 part 1) | – |
| ITK-34 | ITK `PointSetToPointSetMetricWithIndexv4` | with a displacement field the points' derivatives are added into the shared field from several threads without synchronisation: SyN with a point-set metric changes from run to run at more than one thread | b | none | the one-thread order at any thread count | – |
| ITK-35 | ITK `BSplineBaseTransform` | no Jacobian with respect to position (`ComputeJacobianWithRespectToPosition` throws "not yet implemented"): a stage after a B-spline transform whose metric needs the composite's Jacobian (linear, displacement-field and `BSpline` stages) fails at its first derivative | c (c? until 2026-10-21) | none | corrected by default: the derivative of the cubic B-spline sum, validated against finite differences, a notice; `reproduce` fails as ITK (the linear stage dropped, ANTS-49) (A5 part 3) | – |
| ITK-36 | ITK `TimeVaryingBSplineVelocityFieldImageRegistrationMethod` | each metric's transforms are set on the whole multi-metric, so every metric sees the transforms the last one set: point-set metrics listed before an image metric see identity fields, image metrics after a point-set metric are warped twice | c | none | corrected by default: each metric its own transforms, a notice; `reproduce` as ITK (A5 part 3) | – |
| ITK-37 | ITK `TimeVaryingBSplineVelocityFieldImageRegistrationMethod` | at the first (and last) time sample the flow's field is zeroed but the previous integration's inverse field is kept, which the point-set metrics use | c | none | corrected by default: zeroed too, a notice; `reproduce` as ITK (A5 part 3) | – |
| ITK-38 | ITK `TimeVaryingBSplineVelocityFieldImageRegistrationMethod` | a single point-set metric's points (and the boundary points) are fitted at their physical positions in the B-spline's parametric domain, which leaves out the direction: with any direction but the identity (an image stored in RAS order) the points fall outside and the fit throws | c | none | corrected by default: the points mapped into the parametric domain, a notice; `reproduce` throws as ITK (A5 part 3) | – |
| ITK-39 | ITK `TimeVaryingVelocityFieldImageRegistrationMethodv4` | the fixed images are resampled through the flow alone: the fixed initial transforms (`-q`) are ignored | c | none | corrected by default: applied, a notice; `reproduce` as ITK (A5 part 3) | – |
| ITK-40 | ITK time-varying registration methods | the images are resampled through the flows but the metric gets the stage's masks unchanged, with identity transforms: the masks no longer cover their images | c | none | corrected by default: the masks resampled with their images, a notice where that changes them; `reproduce` as ITK (A5 part 3) | – |
| ITK-41 | ITK velocity-field transforms | a transform that never integrated its velocity field (no iteration, no resampling) is incomplete: a time-varying one crashes at the next point it maps, an exponential one has no inverse field ("No displacement field is specified.") | a | none | corrected by default: integrated (zero fields where nothing was optimised), a notice; `reproduce` refuses where ANTs crashes and throws ITK's exception (A5 part 3) | – |
| ITK-42 | ITK scales estimator and optimizer | scales all below epsilon (a B-spline-smoothed update whose control points all touch its pinned boundary: the probe vanishes, the scales equal but for rounding) are refused ("m_Scales values must be > epsilon"), although equal scales cancel in the estimated learning rate | c | none | corrected by default: ones, a notice; `reproduce` throws as ITK (A5 part 3) | – |
| ITK-50 | ITK `KdTreeBasedKmeansEstimator` | the point a k-d tree node is split at is left out of the new centres when the node is divided between candidates | c | every run: sMRIPrep's Atropos call (corrected: 1 to 25 of 0.8 to 2.5 million voxels change label on the five oracle runs) | corrected by default (A6) | – |
| ITK-51 | ITK `TDistribution` | density and `InverseCDF` NaN above about 340 degrees of freedom (Gamma overflows): Grubbs-Rosner never finds an outlier in an image | c | none | corrected by default (A6) | – |
| ITK-60 | ITK `LandmarkBasedTransformInitializer` (antsAI `-b`, 2D) | the 2D rigid fit takes −90° whenever the landmarks' summed dot product is within 5e-5 of 0, whatever the sign of the cross product | c | none | corrected by default: `atan2` there too (2026-10-23) | – |
| ITK-95 | ITK `MovingHistogramMorphologyImageFilter` (ImageMath `GO`, `GC`, `GD`, `GE`) | an image whose first axis has one voxel: at a radius of 0 the moving histogram empties and voxels become ±FLT_MAX instead of their own value; at a larger radius some voxels take values from outside the structuring element | c (rule A, 2026-10-25) | none (fMRIPrep's images have no axis of one voxel) | the true result in both modes (the identity at radius 0) | switch and notice (`reproduce`: ITK's values) |
| ITK-96 | ITK HDF5 transform writer | a transform without parameters (an IdentityTransform, or the inverse a displacement field without an inverse field gives) makes a chunk of 0 values: HDF5's error (`H5Pset_chunk failed`), the file part-written (ITK-15's writer behaviour, met in antsRegistration's inverse composite) | a | none | antsRegistration writes no inverse composite there and names ITK-96, in both modes; larmorx's HDF5 writer itself stores such a transform uncompressed (ITK-15) | reproduce (the part-written file and HDF5's messages) |
| ITK-97 | ITK `ImageToImageMetricv4` (antsRegistration's displacement-field stages) | a sampled metric (Random, Regular) with a shrink factor: a sample point (drawn on the fixed image) outside the shrunk virtual region has its derivative added at the region's linear offset of its index, another voxel's parameters (`StorePointDerivativeResult` checks nothing) | c (rule A, 2026-10-25) | none (fMRIPrep's displacement-field stages sample densely) | dropped by default (the point's value still counts); `reproduce` adds it where ITK does; named in both modes with the count | notice in the time-varying stages |
| ITK-98 | ITK `BSplineInterpolateImageFunction` | the mirror boundary is applied once: on an axis too short for the spline's support (2 voxels for orders 3 and 4, 2 or 3 for order 5) a support index stays outside the axis and the coefficient image is read outside its buffer (memory it never wrote) | a | none (fMRIPrep's images have no such axis) | the mirror repeated by default (the decomposition's whole-sample mirror, the exact continuation); `reproduce` keeps ITK's single mirror with those reads as 0; named in both modes in ResampleImage and antsApplyTransforms (2026-10-25) | notice in antsRegistration's warped outputs and the N-D resampler |
| ITK-99 | the same as ITK-97 | the same with a linear offset before or past the derivative array: written outside it (memory it does not own) | a | none | dropped in both modes, named | – |
| ITK-100 | ITK `MattesMutualInformationImageToImageMetricv4` (antsRegistration, antsAI) | the threader's joint histogram images are created at a stage's first evaluation and kept: a later level whose analysis region has one intensity (a bin size of 0) never reaches `ImageBase::SetSpacing`'s exception, which the first level gives; the bin size is used silently, the bin indices NaN or infinite and clamped, the metric 0 | e (rule B, 2026-10-25) | none (fMRIPrep's masks hold more than one intensity) | an error by default, as the first level gives, named; `reproduce` uses it as ITK does (the metric 0), named | – |
| HDF-01 | HDF5 1.12 `H5T__conv_f_f` | non-native floats converted with state carried from one element to the next | c | none (fMRIPrep's are native floats) | corrected by default (`upstream_defects`, 2026-10-16) | – |
| PIPE-01 | nipype, niworkflows | `shrink_factor=2` dropped: the BOLD reference's N4 shrinks by 4 | c | every run | not a tool defect | later (decided 2026-10-23: when larmorprepx is built) |
| PIPE-02 | fMRIPrep `ResampleSeries`, OpenBLAS | volumes corrupted when OpenBLAS runs threads | b | outside fMRIPrep's image | thread-independent | – |
| PIPE-03 | fMRIPrep `ResampleSeries` | Jacobian along the wrong target axis | c | PEPOLAR, non-native spaces | corrected by default (`upstream_defects`, 2026-10-11) | – |
| PIPE-04 | nitransforms | `.h5` composites read in name order | c | latent | corrected by default (`upstream_defects`, 2026-10-11) | – |
| PIPE-05 | nitransforms | no displacement beyond a field's last nodes | c? | 2.9 % of MNI points | reproduces | later (decided 2026-10-23: when larmorprepx is built) |
| PIPE-06 | nitransforms, fMRIPrep | float32 roundings at every step | d | every run | reproduces | – (D11) |
| PIPE-07 | nitransforms | a one-transform head-motion file applied in world space | c? | one-volume runs | reproduces | later (decided 2026-10-23: when larmorprepx is built) |
| PIPE-08 | fMRIPrep confounds | motion parameters about another centre than mcflirt's | c? | every run | not ported | later (decided 2026-10-23: when larmorprepx is built) |
| PIPE-09 | fMRIPrep, niworkflows | wrong dtype log lines; a deprecated argument; float64 masks | f | every run | – | – |
| PIPE-10 | fMRIPrep, nipype | options that do nothing | e | their users | – | – |
| PIPE-11 | sMRIPrep | a registration fallback leaves the composite undefined | a | after a failed attempt | – | – |
| PIPE-12 | sMRIPrep | a default that is not an allowed choice | e | none | – | – |
| PIPE-13 | sMRIPrep | the report text prints a module | f | every report | – | – |
| PIPE-14 | sdcflows | `B0FieldTransform.apply` ignores head motion | c | none | – | – |
| PIPE-15 | nipype `MCFLIRT` | emits `-edge`; expects names mcflirt does not write | e | none | – | – |
| PIPE-16 | nipype `N4BiasFieldCorrection` | `copy_header` writes the corrected image in the input's (integer) type | c? | field-map references (int16) | not a tool defect; decided 2026-10-21: larmorprepx keeps N4's float32 result, `reproduce` fMRIPrep's int16 | – (larmorprepx) |
| PIPE-17 | nitransforms | the first group of an `.h5` file skipped as if it were the composite: one transform fails, a list without a composite loses its first, an empty composite fails | c | none (fMRIPrep's composites) | corrected by default (`upstream_defects`, 2026-10-16) | – |
| SCI-01 | SciPy `map_coordinates` | huge or NaN coordinates read outside the array | a | none | NaN | – |
| SCI-02 | SciPy `map_coordinates` | undefined conversions answered as x86-64 does | a | none | clamps and warns by default; `reproduce` gives x86-64's answers (2026-10-12) | – |
| SCI-03 | SciPy splines | `reflect`, `grid-mirror` do not interpolate on short axes | c | none | corrected by default (`upstream_defects`, 2026-10-11) | – |
| SCI-04 | SciPy on aarch64 | fused multiply-adds change last bits | d | ARM machines | never fuses | – |
| NP-01 | numpy, OpenBLAS, Accelerate | 4×4 products and inverses depend on the CPU's kernel | d | every run | one order everywhere | – |
| NP-02 | nibabel, numpy SVD | axis choice at exactly 45° follows the SVD's rounding | d | none | vnl's SVD | – |
| NIB-01 | nibabel | compression decided by the file name | e | none | decided by content | – (decided 2026-10-23: larmorx's behaviour kept) |
| NIB-02 | nibabel | `vox_offset` 0: header read as data | c | none | refuses | – (decided 2026-10-23: larmorx's behaviour kept) |
| NIB-03 | nibabel | complex data: imaginary part dropped | c | none | raises | – (decided 2026-10-23: larmorx's behaviour kept) |
| NIB-04 | nibabel | qform precision depends on the platform | d | none | f64 everywhere | – |
| GLIBC-01 | glibc 2.35 | `exp`, `log`, `sin`, `cos` not correctly rounded | d | every ANTs run | correctly rounded | – |
| GLIBC-02 | glibc 2.35 | `expf`, `logf`, `powf` not correctly rounded | d | N4, DenoiseImage | correctly rounded | – |
| GLIBC-03 | glibc 2.35 | `sinf`, `cosf` not correctly rounded; two builds differ | d | none | see entry | – |
| GLIBC-04 | glibc 2.35 | `pow` not correctly rounded for 5 filter-pole powers | d | none | exact | – |
| ANTSPY-01 | ANTsPy 0.6.3 | inverts an affine without being asked | c? | none | never inverts unasked | – |
| ANTSPY-02 | ANTsPy 0.6.3 | double transforms rounded to float32 | c | none | – | – |
| ANTSPY-03 | ANTsPy 0.6.3 | header direction transposed and rounded | c | none | – | – |
| AFNI-01 † | 3dTshift, 3dAutomask, 3dUnifize | negative `scl_slope`: output sign-flipped and scaled | c | rare | corrected by default in both implementations (replica 2026-10-12); the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-02 † | 3dTshift, 3dAutomask, 3dUnifize | float32 with slope and intercept scaled twice | c | rare | corrected by default in both implementations (scales once; replica 2026-10-12); the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-03 † | 3dTshift (3dAutomask, 3dUnifize: no slice timing written) | header slice code 2 (sequential decreasing): no shift | c | none | corrected by default in both implementations (replica 2026-10-12); the GPL replicas of 3dAutomask and 3dUnifize name it where the written header changes (2026-10-25), their clean-room originals not yet | – |
| AFNI-04 † | 3dTshift | can crash for a shift beyond the series on odd slices | a | none | defined (zeros) | – |
| AFNI-05 † | 3dTshift | `-no_detrend` de-means one voxel of each pair only | c | none | corrected by default in both implementations (replica 2026-10-12) | – |
| AFNI-06 † | 3dTshift | `-rlt+` restores the intercept; the help says the mean | f | none | AFNI's behaviour, in both implementations and both modes; larmorx's docs say what `-rlt+` does | – (decided 2026-10-12: the help text is the error) |
| AFNI-07 † | 3dTshift, 3dAutomask, 3dUnifize | metre or micron units: voxel sizes converted; the origin not (plumb grid) or the whole sform not (oblique grid) | c | rare | corrected by default in both implementations (converts), `reproduce` gives AFNI's geometry in both (the original since 2026-10-12, spec §6.1); the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-08 † | 3dTshift, 3dAutomask, 3dUnifize | a copy of the input whose kept slice times are all within 3 ms of 0 gets `slice_end` 0 | c | none | corrected by default in both implementations, with a switch (the original since 2026-10-12, spec §6.2) | – |
| AFNI-09 † | 3dTshift | slice-time files rounded two ways | d | none | replica reproduces | – |
| AFNI-10 † | 3dTshift, 3dAutomask, 3dUnifize | AFNI's own header extension (7 digits) wins over the NIfTI header | d | AFNI-written inputs (fMRIPrep's 3dAutomask reads 3dUnifize's output: ds005454's BOLD reference, a header 7th digit) | replicas reproduce; originals ignore (the 3dAutomask original since 2026-10-23: data bit-identical, header as AFNI's without the extension) | – |
| AFNI-11 † | 3dTshift, 3dAutomask, 3dUnifize | existing output: not written, exit 0 (unless `-overwrite` or `AFNI_DECONFLICT` says to overwrite, rename or stop) | e | none | error (exit 1) in both implementations; `reproduce` exits 0 without writing; `-overwrite` and `AFNI_DECONFLICT` as AFNI in both modes (both implementations, the original since 2026-10-12; spec §6.3, §7.1); the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-12 † | 3dTshift, 3dAutomask, 3dUnifize | output names with blanks or non-ASCII refused, at the option scan (exit 1) and, for names starting with `3dcalc`, when written (nothing written, exit 0); with `AFNI_ALLOW_ARBITRARY_FILENAMES` only empty names and, when written, names starting with `-` | e | paths with blanks | any name accepted in both implementations; `reproduce` refuses them at both checks, with the variable as AFNI (the original since 2026-10-12; spec §6.3, §7.1); the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-13 † | 3dTshift | unknown pattern: "ERROR", not "FATAL ERROR" | f | none | AFNI's text in both implementations (the original since 2026-10-12) | – |
| AFNI-14 † | 3dTshift | `-no_detrend` with Fourier: "changing default", then fatal | f | none | AFNI's text in both implementations (the original since 2026-10-12) | – |
| AFNI-15 † | AFNI build | parallel make races | b | none | – | – |
| AFNI-16 † | 3dTshift, 3dAutomask, 3dUnifize | a matrix within about 0.01° to 0.024° of plumb (a single-precision test; the angle depends on the voxel size) is written plumb, its tilt discarded | c | slightly oblique inputs | corrected by default in both implementations (keeps the matrix; decided 2026-10-12); `reproduce` writes AFNI's plumb matrix byte for byte in both (the original since 2026-10-12, spec §6.1); the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-17 † | 3dTshift | the slice fields of a copy describe its slice times only to within 3 ms (slices less than 3 ms apart can be shifted by one in the range) | d | none | both implementations reproduce it in both modes (the original since 2026-10-12, spec §6.2) | – |
| AFNI-18 † | 3dTshift, 3dAutomask, 3dUnifize | an input name with a blank is read as a list of datasets (unless `AFNI_PATH_SPACES_OK`): an input in a directory with a blank cannot be opened | e | paths with blanks | both implementations read one file by default, AFNI's list in `reproduce` (2026-10-12; spec §7.1); the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-19 † | 3dTshift | `AFNI_ALLOW_MILLISECONDS` with `-TR` in ms: `toffset` written in ms, labelled seconds | c | none | both implementations write seconds by default, AFNI's in `reproduce` (2026-10-12; spec §7.1) | – |
| AFNI-20 † | 3dTshift, 3dAutomask, 3dUnifize | a one-row dataset written as a 1D text file under the NIfTI name (unless `AFNI_WRITE_1D_AS_PREFIX`) | c | none | NIfTI in both implementations and both modes, the decision named (AFNI's file has a random identifier) | – |
| AFNI-21 † | 3dTshift | warning and error labels in terminal colour codes, also in files and pipes | f | log files | `reproduce` writes them in both implementations (the original since 2026-10-12); plain labels by default | – |
| AFNI-22 † | 3dTshift, 3dAutomask, 3dUnifize | `-Dname=value` with blanks or a blank around `=` sets another value (`-DX=a b` sets `a=b`) | e | none | both implementations read the value as written by default, as AFNI in `reproduce` (2026-10-12; spec §7.1) | – |
| AFNI-23 † | 3dTshift, 3dAutomask, 3dUnifize | `AFNI_NIFTI_SCALE=NO`: integer data keep the slope and lose the intercept, so their values are off by it | c | none | corrected by default in both implementations (decided 2026-10-12; the original the same day): AFNI's integers kept, the intercept written as `scl_inter`; with `-rlt -ignore`, read as without the variable; reproduced with the switch (spec §7.1); the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-24 † | NIfTI library | the quaternion's quotients and sums in `long double`, so results depend on the build (x87 on x86-64) | d | none | both implementations give x86-64's results on every platform (2026-10-12) | – |
| AFNI-25 † | 3dTshift | `-ignore` as the last argument: a crash (exit 1 from AFNI's signal handler, with a crash banner) | a | none | "-ignore needs an argument!", exit 1, in both implementations and both modes | – |
| AFNI-26 † | 3dTshift, 3dAutomask, 3dUnifize | an output that cannot be opened for writing (a missing or read-only directory, a read-only file, a directory of that name): an error message, nothing written, exit 0 | e | none | an error, exit 1, in both implementations by default; `reproduce` writes nothing and exits 0 with AFNI's message in both (the replica and then the original, 2026-10-12; spec §7.1); the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-40 † | 3dAutomask, 3dUnifize | an option's value missing as the last argument (`-peels`, `-nbhrs`, `-clfrac`, `-SI`, `-dilate`, `-erode`; `-T2up`): a crash | a | none | an error (exit 1) in both modes, after AFNI's messages; the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-41 † | 3dAutomask | `-nbhrs` outside 6..26 silently becomes 17 (the range check never runs); 19..26 act as 18 | e (c? until 2026-10-23) | none | refused by default in both implementations (a fatal error naming AFNI-41; the replica since the small-fixes part, the clean-room original since 2026-10-23); `reproduce` keeps AFNI's behaviour | – (rule B, 2026-10-23) |
| AFNI-42 † | 3dAutomask | `-NN1`, `-NN2`, `-NN3` also set the peeling threshold to 6, 18, 26, overriding `-nbhrs` (an explicit `-NN2` peels with 18, the default with 17) | c? | none | AFNI's behaviour in both modes | – (decided 2026-10-23, rule C: AFNI's behaviour kept, documented) |
| AFNI-43 † | 3dAutomask | `-peels 0` peels once; the help says 0 turns the peeling off | c | none | corrected by default (no peel), named when the mask changes; `reproduce` peels once; the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23; the GPL replica also names it where only the clustering messages differ (2026-10-25) | – |
| AFNI-44 † | 3dAutomask | the large-hole filling runs or not depending on the number of slices only (32 to 93 slices: yes; fewer: no) | c? | every BOLD reference (fMRIPrep's) | AFNI's behaviour in both modes | – (decided 2026-10-23, rule C: AFNI's behaviour kept, documented) |
| AFNI-45 † | 3dUnifize | `-T2up` at or near 100: the percentile read one past the sorted values (memory never written; in AFNI's runs 0, which zeroes the output) | a | none | inverts about the largest value by default, named; `reproduce` fills the read with 0; the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-46 † | 3dUnifize | `-T2` on a 2D image smaller than 16 × 16: an empty automask dereferenced, a crash | a | none | an error (exit 1) in both modes, after AFNI's messages; the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-47 † | 3dUnifize | more than a million voxels with an axis of 2 or 3: the half-size scale image comes back smaller than the input and is read past its end (a crash in AFNI's runs, or values from that memory) | a | none | the scale brought back to the input's grid by default, named; `reproduce` reads AFNI's smaller image, 0 past its end; the clean-room originals of 3dAutomask and 3dUnifize since 2026-10-23 | – |
| AFNI-48 † | 3dUnifize | `-T2` three times: neither inverted back nor edge-clipped (the output stays inverted); the help documents once and twice | e (c? until 2026-10-23) | none | refused by default in both implementations (a fatal error naming AFNI-48; with `-EPI`, which implies `-T2` itself, it runs as in AFNI); `reproduce` keeps AFNI's behaviour | – (rule B, 2026-10-23) |
| AFNI-49 † | 3dUnifize | the help gives `-clfrac`'s default as 0.1, then says it was raised to 0.2 in 2016; the code uses 0.2 | f | none | 0.2; larmorx's docs say so | – |
| AFNI-75 † | 3dUnifize | `-EPI` sets the neighbourhood's radius from the voxel size ("-EPI changes -Urad to N voxels"), unbounded: on micrometre voxels N is about 6100 and the ball of neighbours has about 10¹² points; AFNI fills the machine until it is stopped | c (rule C for the method, 2026-10-25) | none (fMRIPrep does not call `-EPI`; its voxels are millimetres) | the method kept; by default the GPL replica computes the ball's memory first and stops at once, naming AFNI-75 with the GiB needed and the likely cause (micrometre units); `reproduce` attempts it as AFNI does (2026-10-25) | the clean-room original (its spec) |
| FSL-01 † | mcflirt | last volume not initialised from the previous one | c | every run | corrected by default (`upstream_defects`, 2026-10-11); on a synthetic series with large motion mcflirt's last volume converges to a 180° rotation, the corrected one to 0.36 mm from the truth ([mri-hmc-normcorr-count.md](../validation/mri-hmc-normcorr-count.md)) | – |
| FSL-02 † | mcflirt | normalised correlation effectively uncentred | c | every run | reproduces; the corrected count is opt-in (`normcorr_count="weights"`), measured in [mri-hmc-normcorr-count.md](../validation/mri-hmc-normcorr-count.md): a change about the size of mcflirt's band, no gain against the ground truth | – (decided 2026-10-23, rule C: mcflirt's behaviour kept; the corrected count stays opt-in) |
| FSL-03 † | mcflirt | in-plane mode pads the first stage's reference twice | c, f | none | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-04 † | mcflirt `-gdt` | reference gradient unused; absolute `-out` aborts | c, a | none | refused by default (both implementations); the replica reproduces it with `reproduce` (2026-10-24) | – |
| FSL-05 † | mcflirt | `-scaling`, `-hist` ignored; `-edge` not an option | e | none | ignored with a warning; `-edge` explained (2026-10-11) | – |
| FSL-06 † | mcflirt | unknown `-cost` silently keeps normcorr | e | none | corrected by default (`upstream_defects`, 2026-10-11): an error | – |
| FSL-07 † | mcflirt | numbers read with `atoi`/`atof`; `-help` alone exits 1 | e | none | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-08 † | mcflirt `-stats`, `-meanvol` | end slices left 0; names with the extension inside (the `-stats` images and `-meanvol`'s `_mean_reg`) | c, e | none | corrected by default (`upstream_defects`, 2026-10-11); the replica names it for `-meanvol` alone too (2026-10-25, random testing `fsl-mcflirt:20261025:616`) | – |
| FSL-09 † | mcflirt | output types: int8 → uint8, truncation, no clamping | c, a | none | corrected by default (`upstream_defects`, 2026-10-11): int8 kept, rounded and clamped; `reproduce` truncates and wraps as x86-64 does | – |
| FSL-10 † | mcflirt | units forced to mm and s: a TR in ms becomes s | c | ms-unit inputs | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-11 † | mcflirt | `FSLOUTPUTTYPE` unset: fails after writing the matrices | e | none | writes `.nii.gz` | – (decided 2026-10-23: larmorx's behaviour kept) |
| FSL-12 † | mcflirt | impossible inputs abort after the whole estimation | a | none | refuses first | – |
| FSL-13 † | mcflirt | `.mat` directory quirks without `-mats` | e | none | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-14 † | mcflirt | one NaN voxel makes the volume's matrix NaN | c | rare | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-15 † | mcflirt `-init` | affects the final resampling only | c (c? until 2026-10-24) | none | reproduces | – (decided 2026-10-24, rule C: as documented, "apply to all vols"; kept and documented) |
| FSL-16 † | mcflirt | rotations orthogonal only to about 1e-8 | d | every run | reproduces | – (D11) |
| FSL-17 † | mcflirt | implementation differs from its paper | f | every run | follows the implementation | – (D11) |
| FSL-18 † | mcflirt Woods cost | squared bin counts | – (c? until 2026-10-24) | none | reproduces | – (decided 2026-10-24, rule C: not a defect; `n_b² σ_b / Σ_b` is Woods' `n_b σ_b / μ_b`) |
| FSL-20 † | mcflirt | `.mat` directory not creatable (a missing parent): message, exit 0, nothing more written | e | none | the replica: an error by default (rule B, 2026-10-24), mcflirt's exit 0 with `reproduce`; the clean-room original: to do (the spec) | – |
| FSL-21 † | mcflirt histogram costs | `-bins` below 8: no bins in the first stage; arbitrary matrices, `corratio` writes before its arrays and aborts | a | none | the replica: refused by default (rule B, 2026-10-24); `reproduce` gives mcflirt's matrices and, for `corratio`, its abort; the clean-room original: to do (the spec) | – |
| FSL-22 † | `calc_bval` (newimage) | the background value sorts the 2-voxel border with `std::sort` and `operator<`: with NaN voxels not a strict weak ordering (undefined in C++); libstdc++'s introsort leaves a position-dependent order, so the pad value of mcflirt's output (and `affine_transform`'s) is arbitrary | a | none (fMRIPrep's inputs have no NaN) | NaNs sorted last in both modes (defined); `reproduce` reproduces every defined part and warns when NaN border voxels make the original's order undefined (D13, 2026-10-11; libstdc++'s order is not emulated: its code is GPL-3.0, which no FSL-licensed package may include) | – |

**Counts** (254 entries, recounted from the table on 2026-10-25, after the random-testing merge; an entry with two classes
counts under its first; "c (c? until …)" entries, decided, count as (c)):

| Class | Entries | of them questionable (c?) |
|---|---|---|
| (a) crash, undefined behaviour, out of bounds | 46 | |
| (b) race, nondeterminism | 11 | |
| (c) deterministic bug that changes results | 122 | 8 |
| (d) imprecision | 20 | |
| (e) command-line or parsing quirk | 42 | |
| (f) documentation or message only | 11 | |
| explained, not a defect | 2 | |

Six entries have a second class: ANTS-12 and FSL-03 (f), ANTS-24 (f), FSL-04 and FSL-09 (a),
FSL-08 (e). ANTS-46 (an early stop) was explained by A4 part 2 as ITK's design, and FSL-18 (Woods' cost) by the mcflirt replica (2026-10-24) as Woods' own criterion. Gap
column (2026-10-12, after completing 3dTshift, part 2b, and AFNI-26's switch): no **yes**, no
**replica** and no **original** left (the 9 original gaps, AFNI-07, -08, -11, -12, -16, -18,
-19, -22 and -23, closed by the clean-room original that day, and AFNI-26, added the same day
and closed by the original after the replica), 18 decide, 1 rule 8, 1 open. AFNI-06 was decided (class f, kept), AFNI-16 (a
defect, corrected by default) and AFNI-23 (a defect, corrected by default) on 2026-10-12;
AFNI-17 (an imprecision) was added the same day, and AFNI-25 and AFNI-26 (found by the
clean-room implementer of part 2b) after it. The first D13
switches (2026-10-11) corrected 13 entries; part 1 (2026-10-12) the 29 ANTs and ITK entries
marked **yes**, ANTS-06 and SCI-02 (decided), ANTS-47 (new), and gave ANTS-07, -19, -28, -29
and ITK-08 a compatibility switch (Decisions, item 5);
part 2b (2026-10-12) gave the 3dTshift replica the original's corrections (AFNI-01, -02, -03,
-05, -07) and switches for AFNI-08 (its trigger found), AFNI-11 and AFNI-12 (item 5), and
added AFNI-16; completing 3dTshift, part 1 (2026-10-12), changed the replica's default for
AFNI-16 and its `reproduce` for AFNI-12's second check, and wrote the spec additions the
clean-room original needs (`specs/3dTshift.md` §6.1 to §6.5); part 2a (2026-10-12) gave the
replica AFNI's global options and environment variables (spec §7.1), corrected AFNI-11 and
AFNI-12 for them, and added AFNI-18 to AFNI-24; AFNI-23, decided the same day, is corrected by
default in the replica. A4 part 1 (ITK's HDF5 transform files, 2026-10-16) added ITK-12 to
ITK-17, HDF-01 and PIPE-17, the class (c) ones corrected by default with a switch; A4 part 2
(antsApplyTransforms' transform outputs, 2026-10-16) added ANTS-48, and antsRegistration's
linear stages ANTS-49 to ANTS-51 and ITK-18. A4 part 3 (antsAI and antsAffineInitializer,
2026-10-17) added ANTS-52 to ANTS-60 and ITK-19 to ITK-21: ANTS-52, -53, -56 and ITK-19
corrected by default with a switch, ANTS-55 refused by default with a switch, ANTS-54 and
ITK-21 to decide; both decided the same day (defects, corrected by default) and corrected with
a switch (2026-10-17). A4 part 4 (antsRegistration's point-set metrics, 2026-10-18) added
ANTS-61 to ANTS-70 and ITK-22 to ITK-29: ANTS-61, -65, ITK-22, -23, -26 and -28 corrected by
default with a switch; ANTS-62, -63 and -66 (class (a)) corrected by default, their defined
parts reproduced on request; ANTS-64, -69 and ITK-25 not reproduced; ITK-27 an imprecision;
ANTS-67, -68, -70, ITK-24 and ITK-29 to decide. A5 part 1 (SyN, 2026-10-19) added ANTS-71 to
ANTS-74 and ITK-30 to ITK-34; A5 part 2 (the B-spline and displacement-field stages,
2026-10-20) added ANTS-75 to ANTS-78 and ITK-35: ANTS-75 and ANTS-76 corrected by default with
a switch, ANTS-77 refused (class (a)), ANTS-78 and ITK-35 to decide. The user decided
ANTS-67, -68, -70, -78, ITK-24, -29 and -35 on 2026-10-21 (defects, corrected by default with a
switch); A5 part 3 (the velocity-field stages, `antsApplyTransforms` with B-spline transforms,
2026-10-22) corrected them, each with its measured effect ([ants-velocity.md](ants-velocity.md)),
and added ANTS-79 to ANTS-82 and ITK-36 to ITK-42: every class (c) and (e) one corrected by
default with a switch, ITK-41 (class (a)) completed by default and refused where ANTs crashes,
ANTS-82 (class (b)) not reproduced. A6 (Atropos, 2026-10-22, in parallel with A5 part 3) added ANTS-90 to ANTS-107, ITK-50 and ITK-51: every class (c) and (e) one corrected by default with a switch, ANTS-105 and ANTS-107 (class (a)) run or are filled deterministically by default, ANTS-106 (c?) left as ANTs; the user decided ANTS-106 on 2026-10-22 (corrected by default with a switch, to implement). On 2026-10-23 the user decided all of them by four standing rules (decision log): the gap column has no **decide** left; after the antsAI merge the same day it had 10 **yes** (decided, to implement: ANTS-17, -22, -23, -32, -40, -44, -116, ITK-06, AFNI-41, -48), all done by the small-fixes part (below). Before that, the gap column had 22 **decide** (recounted 2026-10-23 from the table, after A5 part 3, A6 and the AFNI masks part were merged; an earlier count of 14 that day had missed the † rows; listed under "Waiting for the user" in [next-steps.md](../next-steps.md)), 1 **yes** (ANTS-106, to implement) and 1 rule 8.
The AFNI part of 2026-10-23 (3dAutomask and 3dUnifize, a GPL replica each; in parallel with
`antsAI -b`) added AFNI-40 to AFNI-49 and extended AFNI-01 (3dAutomask's average of a series
and 3dUnifize's `-EPI` median drop a negative scale factor, as 3dTshift's extraction does):
AFNI-43 (class (c)) corrected by default with a switch; AFNI-45 and AFNI-47 (class (a), reads
of memory AFNI never wrote) defined by default, filled deterministically with 0 on request;
AFNI-40 and AFNI-46 (crashes) refused in both modes; AFNI-49 a help text; AFNI-41, -42, -44 and
-48 to decide. The dataset defects 3dTshift found (AFNI-01, -02, -07, -08, -11, -12, -16, -18,
-20, -22, -23, -26) apply to both programs and are handled by the same replica code; their
tool in `larmorx_core::defects` is now "AFNI".
antsAI's blob alignment (2026-10-23, in parallel with AFNI's 3dAutomask and 3dUnifize) added
ANTS-110 to ANTS-116 (ANTS-113 unused) and ITK-60: ANTS-110, -111, -112, -114 and ITK-60
corrected by default with a switch, ANTS-115 (class (a)) refused, ANTS-116 decided by the rules the same day (rule A; done by the small-fixes part); ANTS-60
measured (a crash at every thread count above one) and now met with the one-thread result; and
corrected ANTS-106 as the user decided on 2026-10-22.
The part of 2026-10-23 and 24 that implemented the decisions taken by the rules on the 23rd ("small
fixes"; one replica author) corrected ANTS-17, -22, -23, -32, -44 and -116 by default with a
switch (rule A), measured ANTS-40 on fMRIPrep's 25 N4 calls and phantoms and corrected it by
default (rule D, then A), measured ITK-06 and kept it (rule D, then C), and made AFNI-41 and
AFNI-48 a fatal error by default in the GPL replicas (rule B; `reproduce` keeps AFNI's
behaviour). The gap column has no **yes** and no **decide** left.
The clean-room originals of 3dAutomask and 3dUnifize (2026-10-23, written from the specs by an
implementer who never read AFNI's source) correct the same defects by default with the same
switch, and implement the user's decisions of 2026-10-23: AFNI-41 and AFNI-48 refused by default
(rule B), AFNI-42 and AFNI-44 kept (rule C). The **yes** of AFNI-41 and AFNI-48 is done for the
originals.
Differential random testing (2026-10-25; [validation/difftest.md](../validation/difftest.md))
added ANTS-160 (class (a): refused in both modes), ANTS-161 (rule B: refused, named), ANTS-162
(rule C for the method, with a guard by default: the case of its first night that filled the
machine), ANTS-163 (class (a): refused, named), ITK-95 (rule A: the identity), ITK-96 (class (a);
ITK-15's writer behaviour, met in antsRegistration's inverse composite), ITK-97 (rule A: dropped
by default, reproduced), ITK-98 and ITK-99 (class (a)), ITK-100 (rule B: an error by default,
reproduced), AFNI-75 (rule C for the method, with a guard by default, as ANTS-162); it extended
ANTS-99 to N4BiasFieldCorrection and DenoiseImage, ANTS-14 to DenoiseImage's Rician bias, and
AFNI-03 to 3dAutomask and 3dUnifize; and it found ITK-01's notice missing in antsRegistration's
field lookups and ANTS-51's velocity fields not reproduced under a `.mat` name. The larmorx
mistakes it found were fixed the same day, each with a unit test and, where the original's
result is defined, a permanent case ([triage](../../validation/difftest/triage.toml)). No entry
it added is **decide** or **yes**; the gaps it left (switches and notices to add, AFNI-75's guard
in the clean-room original) are listed below ("Random testing's gaps").

## D13 gaps: what the next task switches to "correct by default"

The class (c) and (e) defects, and one class (a) read, that larmorx reproduced by default
before D13. For each: the larmorx code path the fix touches. Every fix needs, per the [porting
guide](../porting-guide.md) §8: the corrected default, a warning when the original's result
would have differed, the compatibility switch, a parity run with the switch (bit for bit as
today) and one with the default (the fix's effect reported), and re-recorded golden checksums
for the outputs that change ([validation/golden.md](../validation/golden.md), "Recording").
The proposed fix is in each entry; "(to confirm)" marks a fix whose intended behaviour the
upstream does not state.

**Status (2026-10-12).** The shared switch is in place (`upstream_defects=`,
`LARMORX_UPSTREAM_DEFECTS`, `larmorx_core::defects`; [api/upstream-defects.md](../api/upstream-defects.md)).
Done: fMRIPrep's libraries and SciPy (PIPE-03, PIPE-04, SCI-03; SCI-02 with part 1),
head-motion correction (every FSL entry below; FSL-02 measured, opt-in), the slice-timing
original (AFNI-01, -02, -03, -05, -07; AFNI-08 needs its trigger, see below), and (part 1,
2026-10-12) every ANTs and ITK entry below, with ANTS-06, ANTS-07, ANTS-19, ANTS-28, ANTS-29,
ANTS-47, ITK-08 and SCI-02. What each correction changed in the parity cases is in the
records' "D13 fix pass" lines ([ants-image-math.md](../validation/ants-image-math.md) and the
other ANTs records). Part 2b (2026-10-12) did the 3dTshift replica's switch
([afni-tshift-replica.md](../validation/afni-tshift-replica.md)), and (2026-10-12, completing
3dTshift, part 1) the replica's AFNI-16 default and the spec additions
(`specs/3dTshift.md` §6.1 to §6.5), and (part 2a) AFNI's global options and environment
variables in the replica, with AFNI-18, -19 and -22 (spec §7.1), and AFNI-23 corrected by
default (decided 2026-10-12). The clean-room original followed (completing 3dTshift, part 2b, 2026-10-12): `reproduce`
for AFNI-07 on oblique grids, AFNI-08, AFNI-11, AFNI-12, AFNI-16, AFNI-18, AFNI-19, AFNI-22 and
AFNI-23, AFNI-03's notice when the corrected times make the output a copy, and AFNI's global
options and variables (spec §6.1 to §6.5 and §7.1). AFNI-26's switch, found the same day,
followed in the replica and then in the clean-room original (2026-10-12): nothing is left for
3dTshift.

**Main package (`crates/`, `python/larmorx/`), ANTs and ITK** (done 2026-10-12):

| ID | Defect | Code path | fMRIPrep |
|---|---|---|---|
| ANTS-20 | `GetLargestComponent`: nothing kept → every voxel 1 | `larmorx_ants::image_math::components::largest_component` (CLI and `lx.ants.largest_component`) | sMRIPrep's brain extraction calls it 4 times |
| ANTS-38 | N4: a first-voxel minimum left out of the histogram | `larmorx_image::n4::sharpen` (range loop) | every N4, when triggered |
| ANTS-43 | DenoiseImage: Rician correction NaN → bias 0 | `larmorx_image::adaptive_denoising` (correction factor) | the denoised T1w, effect not measured |
| ANTS-12 | `TruncateImageIntensity`: histogram range `else if` | `larmorx_ants::image_math::intensity::truncate_image_intensity` | sMRIPrep's two truncations, when triggered |
| ANTS-19 (a) | `FillHoles` with `holeparam ≤ 1` reads outside the image | `larmorx_ants::image_math::morphology::fill_holes` (needs: what an outside neighbour counts as) | none (`FillHoles 2`) |
| ANTS-11 | `Neg`: range `else if` | `larmorx_ants::image_math::arithmetic::negative` | none |
| ANTS-08, 09, 10 | `/` keeps the previous result; `total`/`mean` running sums; `vtotal` zeros | `larmorx_ants::image_math::arithmetic` | none |
| ANTS-13 | `Normalize`: range from 0 and 1e9 | `larmorx_ants::image_math::intensity::normalize` | none |
| ANTS-14 | `G`: NaN kernel above ≈709 voxels² | `larmorx_image::discrete_gaussian` (Bessel kernel) | none |
| ANTS-25 | Otsu with a mask: range from `FLT_MIN`, `else if` | `larmorx_ants::threshold_image` (`threshold_image.rs:106`; CLI and `lx.ants.otsu_threshold`) | none |
| ANTS-39 | N4: 256 bins → 1024-point FFT | `larmorx_image::n4` (padded size, `n4.rs:157`) | none (200 bins) |
| ITK-01 | `Math::Floor` of tiny negatives | `larmorx_core::IndexFloor` in the 4D paths of `larmorx_ants::resample`, the field and sinc lookups | none seen |
| ITK-02 | extra dimensions read with an identity direction | `larmorx_ants::image::from_itk` (`image.rs:407`) | none seen |
| ITK-03 | float labels stop at 2^24 | `larmorx_ants::image_math::morphology::float_labels` | latent |
| ANTS-04 | ImageMath exits 0 after a failure | `larmorx_ants::cli::image_math` (dispatch, `OpError`) | hides failed steps |
| ANTS-27 | MultiplyImages: unreadable file → × 0 | `larmorx_ants::cli::multiply_images` | a missing file zeroes a mask silently |
| ANTS-36 | N4: unreadable mask becomes ones; `-r` without a mask | `larmorx_ants::cli::n4_bias_field_correction` | fMRIPrep passes `-r` without a mask |
| ANTS-35 | N4: options without brackets set nothing | `larmorx_ants::cli::n4_bias_field_correction` | none |
| ANTS-16 | `Laplacian` flag from the sigma argument | `larmorx_ants::cli::image_math::gaussian` | none (`1.5 1` gives the same) |
| ANTS-15 | `G`, SmoothImage: wrong number of sigmas | `larmorx_ants::cli::image_math::gaussian`, `larmorx_ants::cli::smooth_image` | none |
| ANTS-03 | names of 3 characters or fewer | `larmorx_ants::image` (`image.rs:384`, `:398`), the ImageMath writers (`output.len() > 3`) | none |
| ANTS-05 | ImageMath operand parsing | `larmorx_ants::cli::cstd::from_string_f32` | none |
| ANTS-24 | ThresholdImage mode names | `larmorx_ants::cli::threshold_image` | none |
| ANTS-30 | ResampleImage's seventh argument (designed: [api/ants-resample-image.md](../api/ants-resample-image.md), "The seventh argument") | `larmorx_ants::cli::resample_image` | none |
| ANTS-31 | ResampleImageBySpacing's arguments | `larmorx_ants::cli::resample_image_by_spacing` | none |
| ANTS-33 | antsApplyTransforms: `-u`, ignored interpolator parameters | `larmorx_ants::cli::apply_transforms`, `OutputType::from_name` | none |

**The decisions of 2026-10-23** (done 2026-10-24, the small-fixes part):

| ID | Defect | Code path | fMRIPrep |
|---|---|---|---|
| ANTS-17 | `ME` keeps other labels, `MO r 0` dilates the zeros | `larmorx_ants::image_math::morphology::morphological` | none |
| ANTS-22 | fractional `PadImage` | `larmorx_ants::image_math::morphology::pad_image` | none |
| ANTS-23 | `ExtractContours` labels truncated | `larmorx_ants::image_math::components::extract_contours` | none |
| ANTS-32 | ResampleImageBySpacing's sigma in mm, outside value | `larmorx_ants::resample_image_by_spacing` (`physical_sigma`, the outside value) | none |
| ANTS-40 | N4's zero and negative voxels in the fit | `larmorx_image::n4` (`nonpositive`, the included voxels) | every N4 with such voxels (16 of 25 calls) |
| ANTS-44 | DenoiseImage's bias outside the mask | `larmorx_image::adaptive_denoising` (`bias_written`) | none (no mask) |
| ANTS-116 | antsAI's patch orientation | `larmorx_ants::ai_blobs` (`Patches::correlation`) | none |
| AFNI-41, AFNI-48 | refused (rule B) | the GPL replicas' command lines (`larmorx-gpl-afni` `cli::automask`, `cli::unifize`) | none |

**The mcflirt replica** (`larmorx-nc`, merged 2026-10-25; its follow-up the same day found no new
defect of mcflirt: the panel's two differing calls and the random tests' 61 findings were the
replica's own, `atanf`'s `aT[0]` taken from fdlibm's comment and a missing FSL-08 notice for
`-meanvol`, both fixed): FSL-04 refused by default with the
switch; the new FSL-20 and FSL-21 refused by default (rule B), each with `reproduce`; it shares
the clean-room original's corrections of FSL-01, -03, -05 to -10, -13 and -14. **Open:** FSL-20
and FSL-21 in the clean-room original (a spec writer adds them to `specs/mcflirt.md`, then a
clean-room implementer).

**Random testing's gaps** (2026-10-25, open; found by differential random testing,
[validation/difftest.md](../validation/difftest.md); every default is already correct, these are
switches and notices still to add):

| ID | Defect | Code path | fMRIPrep |
|---|---|---|---|
| ANTS-161 | ResampleImage's Gaussian sigma or alpha ≤ 0 (`reproduce`: ANTs' NaN image, vnl's messages) | `larmorx_ants::cli::resample_image` | none |
| ITK-01 | the notice in antsRegistration's displacement-field lookups (each lookup's floor counts are dropped) | `larmorx_registration::field::FieldXfm` | none |
| ITK-95 | GO/GC/GD/GE on a first axis of one voxel, at radius 0 and above (`reproduce` and the notice) | `larmorx_image::morphology` (ITK's moving histogram) | none |
| ITK-96 | antsRegistration's part-written inverse composite when a displacement field has no inverse (`reproduce`) | `larmorx_ants::cli::registration` (the writer, `larmorx_io::itk_hdf5`, already reproduces ITK-15's file) | none |
| ITK-97 | the time-varying velocity-field stages: not named (their metrics drop such points in both modes; their sampling is clock-seeded, ANTS-82) | `larmorx_registration::time_varying` | none |
| ITK-98 | the notice in antsRegistration's warped outputs and the N-D resampler (both use the repeated mirror) | `larmorx_registration::resample` | none |
| AFNI-75 | the clean-room original of 3dUnifize: the same guard (its spec, then its agent) | `crates/larmorx-afni` (3dUnifize) | none |
| ANTS-51 | a velocity field written under a `.mat` name in `reproduce` (ANTs writes the velocity lattice) | `larmorx_ants::cli::registration` (`write_one_transform`) | none |

**Main package, fMRIPrep's libraries and SciPy** (done 2026-10-11):

| ID | Defect | Code path | fMRIPrep |
|---|---|---|---|
| PIPE-03 | `ResampleSeries`' Jacobian along the wrong target axis | `larmorx_transform::resample_series` (Jacobian), `lx.transforms.resample_series` | PEPOLAR field maps (Jacobian on by default), T1w and standard spaces |
| PIPE-04 | `.h5` composites read in name order | `python/larmorx/transforms/chain.py:348` (`lx.transforms.load_transforms`) | latent (11+ transforms) |
| SCI-03 | `reflect`/`grid-mirror` splines do not interpolate | `larmorx_interp::ndimage::splines` (`lx.ndimage`) | none |

**Head-motion correction, clean-room original (`crates/larmorx-mri`; fixes need only the
spec, which describes each behaviour)** (done 2026-10-11; FSL-09's reproduced conversion wraps
around beyond the type's range, as mcflirt does on x86-64, observed and added to
`specs/mcflirt.md` §9.4):

| ID | Defect | Code path | fMRIPrep |
|---|---|---|---|
| FSL-01 | last volume not initialised from the previous one | `larmorx_mri::hmc::estimate::chains` | every run's last volume |
| FSL-14 | one NaN voxel makes the matrix NaN | the centre of mass, `larmorx_mri::hmc::rigid` | rare |
| FSL-03 | in-plane mode pads the first stage's reference twice | `larmorx_mri::hmc::estimate` (`estimate.rs:336`) | none |
| FSL-08 | `-stats`: end slices 0; output names | `larmorx_mri::hmc::stats::temporal`, `larmorx_mri::cli::hmc::write_outputs` | none |
| FSL-09 | output types: int8 → uint8, truncation | `larmorx_mri::hmc::image::output_type` and the conversion after it | none |
| FSL-10 | units forced to mm and s | `larmorx_mri::hmc::image` (`image.rs:315`) | ms-unit inputs |
| FSL-05, 06, 07 | ignored options, unknown costs, `atoi` | `larmorx_mri::cli::hmc` (option parsing) | none |
| FSL-13 | `.mat` directory without `-mats` | `larmorx_mri::cli::hmc::write_outputs` | none |

**Slice-timing correction, both implementations** (the clean-room original in
`crates/larmorx-afni`, the replica in `crates-gpl/larmorx-gpl-afni`; the replica's defaults
must change with the original's, see "Decisions" item 3). The original is done
(2026-10-11): AFNI-02's and AFNI-07's observable behaviour was settled by black-box runs and
added to `specs/3dTshift.md` §6; AFNI-08's was not found (no input tried, among more than 500
black-box runs, made 3dTshift write `slice_end` 0), so the original writes `nz − 1` in every
case and has no switch for it until the spec describes the trigger (it does since
2026-10-12, §6.2). The replica is done too
(2026-10-12): every entry below corrected by default, `reproduce` giving AFNI's output bytes,
AFNI-08's trigger found in the source (a "copy of the input" that keeps slice times all within
3 ms of 0, e.g. header slice code 1 with a 0.5 ms duration and a 1 ms TR) and confirmed with
the oracle, and AFNI-11 and AFNI-12 given a switch:

| ID | Defect | Code path | fMRIPrep |
|---|---|---|---|
| AFNI-01 | negative `scl_slope` | original: `larmorx_afni::dataset` (reading and storing back), its warning; the replica's dataset reader and writer; `lx.afni.tshift`'s warning | rare |
| AFNI-05 | `-no_detrend` voxel pairs | original: `larmorx_afni::tshift`, the warnings in `larmorx_afni::cli::tshift` and `lx.afni.tshift`; the replica | none |
| AFNI-03 | header slice code 2 | original: `larmorx_afni::timing::header_timing`; the replica's header reader | none (fMRIPrep passes `-tpattern`) |
| AFNI-02 | float32 with slope and intercept scaled twice | replica: its dataset reader (fix by default). Original: already scales once; it needs the warning and the switch, so the spec writer first adds the behaviour to `specs/3dTshift.md` §6 | rare |
| AFNI-07 | metre or micron units | replica: its writer (fix by default). Original: already converts; needs the switch, after a spec addition (§6) | rare |
| AFNI-08 | all-zero slice-time list | the replica's writer; the original's (`larmorx_afni::timing::copy_slice_fields`, 2026-10-12) | none |
| AFNI-11, AFNI-12 | existing output; refused names | the replica's command line (switch); the original's (`larmorx_afni::cli::tshift`, `larmorx_afni::output`, 2026-10-12) | paths with blanks |
| AFNI-16 | a nearly plumb matrix written plumb | the replica's writer (default changed 2026-10-12); the original's (`larmorx_afni::geometry`, 2026-10-12) | slightly oblique inputs |
| AFNI-18, AFNI-19, AFNI-22 | an input name with a blank; `AFNI_ALLOW_MILLISECONDS`; `-D` values | the replica's input opening, writer and global options (2026-10-12); the original's (`larmorx_afni::input`, `larmorx_afni::settings`, `larmorx_afni::cli::tshift`, 2026-10-12) | paths with blanks |
| AFNI-23 | `AFNI_NIFTI_SCALE=NO` drops the intercept | the replica's reader, writer and `-rlt` path (2026-10-12); the original's (`larmorx_afni::dataset::intercept_plan`, 2026-10-12) | none |
| AFNI-26 | a failed write exits 0 | the replica's command line (2026-10-12); the original's (`larmorx_afni::output::cannot_open`, `larmorx_afni::cli::tshift`, 2026-10-12): `reproduce` prints the NIfTI library's message, `-verbose`'s line, writes nothing and exits 0 (spec §7.1, "A failed write"; parity cases `errors/missing-directory*`, rejected by both with AFNI's exit status and messages in both records) | none |

### Decisions needed before switching

**Decided by the user on 2026-10-11** (decision log, "D13's details"):
- Item 1, undefined conversions: **clamp and warn by default**; the compatibility switch
  reproduces x86-64's wraparound. **Done 2026-10-12** (ANTS-06, SCI-02).
- Item 3, replicas: **the same corrected default as the originals**; the compatibility switch
  must give **exactly the original program's result**.
- Item 4, FSL-02: **measure first** (against mcflirt's band and the synthetic ground truth),
  then decide between fixing by default and keeping it opt-in. **Measured 2026-10-11**
  ([mri-hmc-normcorr-count.md](../validation/mri-hmc-normcorr-count.md)): the corrected count
  moves the matrices by about mcflirt's own variability (0.014 to 0.033 mm at the 95th
  percentile on 19 real runs; band 0.016 to 0.10 mm; 24 of 28 band runs within the band) and
  is not more accurate on the synthetic ground truth (better on 5 of 8 series, worse on 3, by
  0.001 to 0.16 mm). **Decided by the user (2026-10-11): it stays opt-in** (`normcorr_count="weights"`); mcflirt's count remains the default.
- Item 5, refusals: **a switch for these too.** Where the original's output is deterministic
  (e.g. a one-voxel image with infinite spacing), the switch reproduces it exactly. Where it
  depends on memory contents and changes from run to run (an unfilled buffer, an out-of-bounds
  read), no program can reproduce it, not even the original. There the switch reproduces
  everything defined (exit code, files, headers), fills the undefined part deterministically
  and warns that it does. **Done 2026-10-12** for the ANTs and ITK entries: ANTS-29 and ITK-08
  exactly; ANTS-07, ANTS-19 and ANTS-28 with the undefined reads or buffer as 0 (their parity
  cases compare everything but the values). ANTS-02 and ANTS-26 (crashes and a hang) and
  ResampleImage's wrong-length spacing list (an abort after an uninitialised read) define
  nothing beyond the failure: exit 1 in both modes. **Done 2026-10-12** for the 3dTshift
  replica's AFNI-11 and AFNI-12 (AFNI's result with its environment variables unset); the
  clean-room original's switches for them wait for a spec addition.
- Items 2, 6 and 7, defaults until decided otherwise: questionable behaviours stay faithful
  and documented until evidence shows a bug; pipeline choices are made with larmorprepx;
  ResampleImage's corrected command line is designed by the implementer and reviewed. Item 7's
  design (2026-10-12, to review): [api/ants-resample-image.md](../api/ants-resample-image.md),
  "The seventh argument".
   Decided by the rules of 2026-10-23 and done the same day: ANTS-17, ANTS-22, ANTS-23,
   ANTS-32, ANTS-44 corrected (rule A); ANTS-40 measured and corrected (D, then A); ITK-06
   measured and kept (D, then C).

The original proposals, for the record:

1. **Undefined conversions** (ANTS-06, SCI-02). On 2026-10-09 larmorx decided that its
   command lines reproduce ANTs on x86-64 where C++ is undefined (an out-of-range `-u` cast
   wraps through int32), as fMRIPrep users see. D13 says undefined behaviour is never
   reproduced. Proposal: the default saturates (as `lx.ants.apply_transforms(dtype=...)`
   already does) and warns; the compatibility switch keeps the x86-64 answers.
2. **Questionable behaviours (c?)**: is each a bug? ANTS-17, ANTS-22, ANTS-23, ANTS-32,
   ANTS-40, ANTS-44, ITK-06, PIPE-05, PIPE-07, AFNI-06 (decided 2026-10-12: kept, class f),
   AFNI-23 (decided 2026-10-12: a defect, corrected by default), FSL-15, FSL-18. ANTS-40 (N4's
   non-positive log values) and PIPE-05 (no displacement beyond the field) change every run
   they touch, so a fix would move many results. Added by A4 part 3 (2026-10-17): ANTS-54
   (antsAffineInitializer ignores its local-search argument: honouring it would change
   niworkflows' initialization, which passes 10 where ANTs runs 20) and ITK-21 (the learning
   rate carried from one start point to the next in antsAffineInitializer); both decided by
   the user on 2026-10-17 (defects, corrected by default with a switch) and done the same day.
   Added by A4 part 4 (2026-10-18), all in antsRegistration's point-set metrics, which
   fMRIPrep does not use: ANTS-67 (IGDM's `kNeighborhood` and `gradientSigma` ignored), ANTS-68
   (IGDM's derivative without the `1/σ²` factors), ANTS-70 (IGDM's distance sigma, the standard
   deviation of the nearest distances, 0 for identical point sets), ITK-24 (JHCT's total
   number of points without the fixed ones) and ITK-29 (the Parzen kernel sigma used as a
   variance). Each changes results wherever the metric is used; none is reached by fMRIPrep.
   All five decided by the user on 2026-10-21 (defects, corrected by default with a switch),
   with ANTS-78 and ITK-35, and done by A5 part 3 (2026-10-22).
3. **D13 for replicas.** `implementation="auto"` runs the replica whenever it is installed. If
   only the original fixed a defect, installing `larmorx[exact]` would bring it back. This
   register reads D13 as: both implementations have the same corrected default, and the
   replica's bit-exactness is what its compatibility switch keeps. To confirm.
4. **FSL-02** (mcflirt's inflated weight count): a clear bug, but fixing it changes the cost of
   every run, with an effect not yet measured against mcflirt's band. Proposal: measure first,
   then decide between D13 (fix) and keeping it as mcflirt's algorithm (D11 territory).
5. **Refusals or fixes without a switch.** Where the original's result is an empty file, a
   failure, garbage or lost data, larmorx refuses or does the right thing, and parity lists an
   expected divergence: ANTS-29, ITK-08, NIB-01, NIB-02, NIB-03, AFNI-11, AFNI-12, FSL-11. D13
   asks for a switch for every class (c) and (e) defect; proposed: none for these.
6. **Pipeline-level choices for larmorprepx** (not tool code): PIPE-01 (the BOLD reference's
   N4 with the intended shrink factor 2, or the factor 4 fMRIPrep really uses) and PIPE-08
   (motion parameters about mcflirt's centre or fMRIPrep's).
7. **ANTS-30** (ResampleImage's seventh argument): the upstream documents no position for the
   interpolator's parameter, so the corrected command line needs a design.

### Also pending, not D13

- **ANTS-41, rule 8 (decided and done 2026-10-11):** N4 follows the C++ source, not the
  GCC 11 build; the ANTs oracle is rebuilt by a fixed compiler and every ANTs suite is re-run
  against it; the N4 record and golden checksums changed.
- **`antsApplyTransforms --float 1` in single precision** (decided 2026-10-11, before A4):
  larmorx's own difference, not an upstream defect ([validation/fmriprep-stages.md](../validation/fmriprep-stages.md),
  stage A11).

## ANTs and ITK

ANTs v2.6.5 and ITK v5.4.5 unless stated. Details in [ants-cli.md](ants-cli.md),
[ants-image-programs.md](ants-image-programs.md), [ants-gaussian-filters.md](ants-gaussian-filters.md),
[ants-morphology.md](ants-morphology.md), [ants-n4.md](ants-n4.md), [ants-denoise.md](ants-denoise.md),
[itk-nifti.md](itk-nifti.md), [itk-resampling.md](itk-resampling.md),
[fmriprep-oracle.md](fmriprep-oracle.md).

### ANTS-01 Parser rejections — (e)
- **What:** brackets are rewritten only at the ends of a word (`Gaussian(1x2x3,2)` is
  rejected); short options take two characters (`-tf` is the long option `tf`); `Convert<T>`
  is strict (`-s 2.5`, `-r true`, `-d 3d` abort); DenoiseImage's `-r [2]` reads the option's
  empty name and exits 1.
- **Evidence:** read: `antsCommandLineParser.cxx:201`, `antsCommandLineOption.cxx:42,64`;
  verified (N4, DenoiseImage).
- **fMRIPrep:** none; nipype's command lines parse.
- **larmorx:** the same rejections, with a reason and exit 1 where ANTs aborts.
- **D13 / gap:** the results never change, only what is accepted. No gap.

### ANTS-02 Crashes on missing files and arguments — (a)
- **What:** ImageMath, N4 and DenoiseImage dereference a null image when the input is missing
  (SIGSEGV), and so does antsRegistration when a metric's image is missing (after
  "All_Command_lines_OK"); MultiplyImages without its first file or output name aborts (`throw;`); PadImage
  without its pad and ReplaceVoxelValue without its value pass a null pointer to `atof`;
  SmoothImage without an output name, `std::stoi`/`std::stof` of text, ITK exceptions outside
  a `try` (`ThresholdAtMean` above the maximum, `RescaleImage` reversed), a morphology radius
  of −1 (`std::bad_alloc`) and a `PadImage` to zero or negative size all abort.
- **Evidence:** verified: every "rejected by both" table in the ANTs records.
- **fMRIPrep:** none.
- **larmorx:** exit 1 with a message that says what ANTs would have done.
- **D13 / gap:** never reproduced. No gap.

### ANTS-03 Short file names are not read or written — (e)
- **What:** `ReadImage` fails silently for names shorter than 3 characters, `WriteImage`
  writes nothing for them, and ImageMath writes only names longer than 3 characters; the
  program exits 0.
- **Evidence:** read: `Utilities/ReadWriteData.h:204,227,526`, `ImageMath_Templates.hxx:5755,8281`.
- **fMRIPrep:** none (nipype's names are long).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): any non-empty name is read and written
  (`larmorx_ants::image::{read_with, write_with}`, ImageMath's `Context::write_if_long`,
  `PadImage`); the warning comes when such a file was read or written. `reproduce` skips them
  as ANTs does (and exits 1 where ANTs' `PadImage` crashes on the null image).
- **D13 / gap:** done. No gap.

### ANTS-04 ImageMath exits 0 after a failed operation — (e)
- **What:** the dispatch tables return `EXIT_SUCCESS` whatever the operation returned, so an
  operation that prints an error and writes nothing still exits 0 (`Mattes` and
  `ComponentToVector` excepted).
- **Evidence:** read: `ImageMath_Templates.hxx:14892-15435`; verified.
- **fMRIPrep:** a failed ImageMath step goes unnoticed until a later node misses its file.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): exit 1 after an operation that reports a failure and writes
  nothing (`larmorx_ants::cli::image_math`, `OpError::Reported`); `reproduce` exits 0. None of
  the operations ported so far reports one (their failures are ANTS-02 crashes, exit 1 in both
  modes), so the switch waits for the operations that do.
- **D13 / gap:** done. No gap.

### ANTS-05 ImageMath's operand parsing — (e)
- **What:** `from_string<float>` decides between a number and an image: `7 ` (trailing space)
  is an image, `1e` and `-` are the number 0, `1e40` is `FLT_MAX`, and no operand at all is
  the number 1.
- **Evidence:** read: `ImageMath_Templates.hxx:212` (libstdc++'s `num_get`); validated
  (`arithmetic/operand-*`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12) (`larmorx_ants::cli::image_math::arithmetic`,
  `cstd::strict_f32`): an operand that is exactly a decimal number is a number, a malformed or
  out-of-range one (`1e`, `-`, `1e40`) is an error, anything else an image (`7 ` with a blank
  stays a file name, as in ANTs); no operand is an error except for `abs`, `exp`, `total`,
  `mean` and `vtotal`, where ANTs' implicit 1 means the image alone. The warning comes where
  ANTs' reading differs.
- **D13 / gap:** done. No gap.

### ANTS-06 Out-of-range conversions to integers — (a)
- **What:** C++ leaves the conversion of an out-of-range or NaN floating value to an integer
  undefined; ANTs on x86-64 answers with `cvttsd2si`'s wrap, ARM builds saturate:
  - antsApplyTransforms `-u uchar` turns 400.7 into 144 and −3.7 into 253;
  - ResampleImage's `unsigned int` pixels turn −1 into 4294967295;
  - DenoiseImage's mask, read as `unsigned char`, turns 256 into 0 and −1 into 255;
  - ExtractContours turns a negative value into a label near 2^64.
  (Truncating in-range values, e.g. a mask of 0.999 read as 0, is defined C++ and not part of
  this entry.)
- **Evidence:** verified on x86-64 (`-u uchar`, `-u char`); validated (ResampleImage
  `pixel-type/*`, DenoiseImage masks, `extract-contours`).
- **fMRIPrep:** none seen; ANTs' results differ between x86-64 and ARM machines here.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12), as decided (Decisions, item 1): out-of-range values clamp to
  the type's range and NaN becomes 0, with a warning that counts them, in every conversion
  above (`larmorx_ants::image::convert_voxels` for the readers, `cli::apply_transforms::cast_output`
  and `lx.ants.apply_transforms(dtype=...)`, ResampleImage's sizes, `extract_contours`);
  `reproduce` gives x86-64's wrap on every platform. A negative morphology radius stays an
  error in both modes (it is a parameter, and ANTs aborts on it).
- **D13 / gap:** done. No gap.

### ANTS-07 A smaller operand image is read out of bounds — (a)
- **What:** ImageMath's voxel-wise operations read the second image at the first image's
  indices with its own strides; a smaller second image is read past its end. (Its geometry is
  ignored even when it is larger.)
- **Evidence:** read: `ImageMath_Templates.hxx:5583-5762`; validated (`arithmetic/larger-operand`).
- **fMRIPrep:** none.
- **larmorx:** a larger operand as ANTs; a smaller one is an error by default. `reproduce`
  (Decisions, item 5) reads it as ANTs does, at the first image's indices with the second's
  strides, where the reads stay inside its buffer, and takes the reads beyond it as 0, with a
  warning that counts them (`larmorx_ants::image_math::arithmetic::operand_values`).
- **D13 / gap:** done (2026-10-12). No gap.

### ANTS-08 ImageMath `/` keeps the previous voxel's result — (c)
- **What:** the quotient is computed only where the divisor is positive; every other voxel
  gets the previous voxel's result (0 before the first division), because the result variable
  is not reset.
- **Evidence:** read: `ImageMath_Templates.hxx:5583-5762`; validated (`arithmetic//-*`).
- **fMRIPrep:** none (not used).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the quotient wherever the divisor is not 0, 0 where it is
  (`larmorx_ants::image_math::arithmetic`, the command line and `lx.ants.image_arithmetic`);
  the warning counts the voxels that change.
- **D13 / gap:** done. No gap.

### ANTS-09 ImageMath `total` and `mean` write running sums — (c)
- **What:** the printed total and mean are right, but every voxel of the written image holds
  the running sum at that voxel.
- **Evidence:** read: `ImageMath_Templates.hxx:5583-5762`; validated.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): every voxel holds the printed value, the total or the mean
  (`larmorx_ants::image_math::arithmetic`).
- **D13 / gap:** done. No gap.

### ANTS-10 ImageMath `vtotal` writes zeros — (c)
- **What:** `vtotal` is dispatched to the arithmetic function, which has no branch for it: the
  output is all zeros, exit 0.
- **Evidence:** read; validated.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): an error (exit 1); `reproduce` writes the zeros.
- **D13 / gap:** done. No gap.

### ANTS-11 ImageMath `Neg` misses the range — (c)
- **What:** the range is found with `if (v > max) max = v; else if (v < min) min = v;` from
  ∓1e12: a voxel that raises the maximum is never compared with the minimum. In an image whose
  values increase in memory order the minimum stays 1e12 and the result (`max − v`) carries
  errors of about 1e-4.
- **Evidence:** read: `ImageMath_Templates.hxx:7495`; validated.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the true range (`larmorx_ants::image_math::arithmetic::negative`);
  the warning comes when ANTs' range differs.
- **D13 / gap:** done. No gap.

### ANTS-12 `TruncateImageIntensity`'s histogram range — (c), (f)
- **What:** the range is found with `if (v < min) ... else if (v > max)`: if the first
  positive voxel in memory is the brightest, it only sets the minimum, the maximum becomes the
  next brightest, and the brightest voxel falls outside the histogram. (f): the usage says 65
  bins; the code uses 64.
- **Evidence:** read: `ImageMath_Templates.hxx:966-1110`; validated
  (`truncate/first-voxel-max`, fMRIPrep's `0.01 0.999 256`).
- **fMRIPrep:** sMRIPrep truncates the T1w twice (stages A03, A10); it matters only when the
  first positive voxel is the brightest (not on ds000005).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the true range of the counted voxels
  (`larmorx_ants::image_math::intensity::truncate_image_intensity`); the warning comes when
  ANTs' range differs. The usage's 65 bins (f) is not D13.
- **D13 / gap:** done. No gap.

### ANTS-13 ImageMath `Normalize`'s range starts at 0 and 1e9 — (c)
- **What:** range mode starts from `max = 0` and `min = 1e9`: an image without a positive
  value keeps the maximum 0, an image whose values all exceed 1e9 keeps the minimum 1e9.
- **Evidence:** read: `ImageMath_Templates.hxx:9099-9118`; validated.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the true range (`larmorx_ants::image_math::intensity::normalize`).
- **D13 / gap:** done. No gap.

### ANTS-14 ImageMath `G` gives NaN for large variances — (c)
- **What:** ITK's `GaussianOperator` computes `e^t` in the Bessel function `I_0`; above a
  variance of about 709 voxels², `e^t` overflows and the kernel is NaN, so the whole image is
  NaN (`G img 41` on 1.5 mm voxels).
- **Evidence:** read: `itkGaussianOperator.hxx:25-199`; validated (`G/huge-variance`).
- **fMRIPrep:** none (not used).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): where ITK's coefficients are not finite, the kernel of
  `e^{−t} I_k(t)`, computed with exponentially scaled Bessel functions and a Miller recurrence
  started above both `k` and `t` (ITK's start is too low for such `t`, ITK-11); elsewhere
  ITK's kernel bit for bit (`larmorx_image::discrete_gaussian::gaussian_kernel_corrected`,
  `DiscreteGaussianOptions::upstream_defects`).
  DenoiseImage's Rician bias smoothing (variance 2 mm²) meets it on micrometre voxels; until
  2026-10-25 larmorx used the corrected kernel there in both modes. Now `reproduce` keeps ITK's
  NaN kernel (the bias left unsmoothed, as in ANTs) and both modes name ANTS-14 (random testing,
  `ants-denoise:20261025:41`).
- **D13 / gap:** done. No gap.

### ANTS-15 A wrong number of sigmas is not an error — (e)
- **What:** ImageMath `G` with a sigma count other than 1 or the dimension prints "Incorrect
  sigma vector size" and filters with variance 0 (the image comes back unchanged); SmoothImage
  prints the same and smooths with ITK's default of 1 mm. Both exit 0.
- **Evidence:** read: `ImageMath_Templates.hxx:8161-8211`, `SmoothImage.cxx`; validated
  (`G/no-sigma`, `G/wrong-length`, `gaussian/wrong-length`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): exit 1 with the message (`larmorx_ants::cli::image_math::gaussian`,
  `larmorx_ants::cli::smooth_image`); Python's functions raise `ValueError` in both modes.
- **D13 / gap:** done. No gap.

### ANTS-16 `Laplacian` reads its flag from the sigma — (e)
- **What:** `LaplacianImage` does not advance past the sigma, so `sig = atof(argv[5])` and
  `normalize = std::stoi(argv[5])`: `Laplacian img 1.5 0` normalises, `0.8 1` does not,
  `.5` aborts, `-1` normalises.
- **Evidence:** read: `ImageMath_Templates.hxx:9235-9285` (9244-9257); validated
  (`laplacian/*`).
- **fMRIPrep:** `Laplacian img 1.5 1` normalises either way: same result.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the flag from its own argument (`argv[6]`), as `Grad` reads
  it (`larmorx_ants::cli::image_math::gaussian`); the warning comes when the two readings
  differ. `lx.ants.laplacian(img, sigma, normalize=...)` takes them separately anyway.
- **D13 / gap:** done. No gap.

### ANTS-17 `MO` with value 0, `ME` on label maps — (c)
- **What:** ITK's binary opening erodes with its own background value, 0 by default, so
  `MO r 0` (foreground 0) erodes nothing and is a plain dilation of the zeros. ANTs' `ME`
  thresholds the eroded image at 0.5, so on a label map it erodes one label and leaves the
  others above 0.5 as 1.
- **Evidence:** read: `itkBinaryMorphologicalOpeningImageFilter.hxx:40,63`,
  `antsUtilities.h:230-247`; validated (`MO-value-0`, `ME-labels`; the corrections against
  SciPy's binary erosion and opening: `ME-labels-known`, `ME-value-0.3-known`, `MO-value-0-known`,
  bit for bit).
- **fMRIPrep:** none (binary masks with value 1).
- **larmorx:** corrected by default (2026-10-23, rule A): `ME r v` is 1 where the voxel still holds
  `v` after the erosion (the eroded mask of that value), 0 elsewhere; `MO r 0` opens the zeros (an
  erosion with the background 1, then a dilation), the zeros it removes becoming 1. Named where the
  output changes; `reproduce` gives ANTs' output ([ants-morphology.md](ants-morphology.md)).
- **D13 / gap:** done. No gap.

### ANTS-18 Signed zeros in grayscale morphology — (b)
- **What:** ITK's moving-histogram dilation and erosion keep `-0.0` and `+0.0` in one map
  entry whose key is whichever came first, so where a neighbourhood's extreme is zero its sign
  depends on the filling order, which follows ITK's thread split (`GD` on an image of signed
  zeros: up to 820 of 1,680 values differ in sign).
- **Evidence:** read: `itkMorphologyHistogram.h`; verified (`GD-signed-zeros`, `GC-signed-zeros`).
- **fMRIPrep:** none (NIfTI images rarely hold `-0.0`).
- **larmorx:** a zero maximum is `+0.0`, a zero minimum `-0.0` (`f32::total_cmp`).
- **D13 / gap:** never reproduced. No gap.

### ANTS-19 `FillHoles` reads outside the image — (a)
- **What:** with `holeparam ≤ 1`, the hole statistics read the binary image at unclamped
  neighbour indices with `GetPixel`: one row past an edge lands on the neighbouring row; on
  the first or last slice the read lands before or after the buffer, and the result depends on
  whatever memory is there.
- **Evidence:** read: `ImageMath_Templates.hxx:8996`; verified (ANTs exits 0 with some output).
- **fMRIPrep:** none (fMRIPrep and sMRIPrep call `FillHoles 2`).
- **larmorx:** corrected by default (2026-10-12): a neighbour outside the image is not
  counted (`larmorx_ants::image_math::morphology::fill_holes`). `reproduce` (Decisions, item 5)
  reads the neighbours that land inside the buffer as ANTs does and takes those beyond it as
  background, with a warning; ANTs' own result there depends on memory, so the parity case
  `fill-holes/hole-first-slice-0.5` compares everything but the values.
- **D13 / gap:** done. No gap.

### ANTS-20 `GetLargestComponent` turns an empty result into a full image — (c)
- **What:** each kept component's voxels get the component's size; the largest value is found
  and every voxel at least that large becomes 1. When no component reaches `smallest` voxels
  (default 50), or the input is empty, the largest value is 0, so **every voxel becomes 1**,
  background included. (Ties keep every largest component.)
- **Evidence:** read: `ImageMath_Templates.hxx:429-563`; validated (`largest-component/empty`,
  `min-size-above-all`).
- **fMRIPrep:** sMRIPrep's brain extraction calls it four times with the default 50 (the
  dilated brain mask, the WM and GM classes, a final cleanup): a class with no component of 50
  voxels would become the whole image.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): an all-zero image when nothing is kept
  (`larmorx_ants::image_math::components::largest_component`, `lx.ants.largest_component`).
- **D13 / gap:** done. No gap.

### ANTS-21 `ClusterThresholdVariate` reads outside the image — (a)
- **What:** after its histogram loop it reads the mask at the iterator's index, which is
  already past the end of the region, and decides with that value which voxels to zero.
- **Evidence:** read: `ImageMath_Templates.hxx:567-673`.
- **fMRIPrep:** none.
- **larmorx:** not ported ("not supported yet").
- **D13 / gap:** never reproduced. No gap.

### ANTS-22 `PadImage` with a fractional pad — (c)
- **What:** each size becomes `(unsigned)(size + 2p)` but the image is copied at
  `(unsigned)(i + p)`: a pad of 2.5 adds 5 voxels and shifts by 2, so the padding is
  asymmetric. A negative fractional pad also moves the data one voxel more than the origin
  and leaves the last row of each axis the pad value.
- **Evidence:** read: `ImageMath_Templates.hxx:1955-2053`; validated (`pad-image`;
  `pad-fraction-known`, `depad-fraction-known` against NumPy's `pad` and slicing, bit for bit,
  the origin within 1e-4 mm).
- **fMRIPrep:** none (integer pads).
- **larmorx:** corrected by default (2026-10-23, rule A): `trunc(p)` voxels on each side (cropped
  for a negative pad), the data where the new origin says. Named where the pad is fractional;
  `reproduce` gives ANTs' output.
- **D13 / gap:** done. No gap.

### ANTS-23 `ExtractContours` labels by truncated value — (c)
- **What:** each run's label is `static_cast<SizeValueType>(value)`: 1.2 and 1.7 are both
  label 1 and no contour separates them; 0.6 is background.
- **Evidence:** read: `itkLabelContourImageFilter.hxx:144`; validated (`extract-contours`;
  `float-labels-known`, `float-labels-faces-known` against NumPy's neighbour comparison, bit for
  bit).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23, rule A): every value is its own label (±0 the
  background, NaN one label), so 1.2 and 1.7 are separated by a contour and 0.6 is a region.
  Named where the output changes; `reproduce` gives ANTs' output.
- **D13 / gap:** done. No gap.

### ANTS-24 ThresholdImage's arguments — (e), (f)
- **What:** (e) a mode other than exactly `Otsu` or `Kmeans` is a range threshold whose lower
  bound is `atof(mode)`: `otsu 2` thresholds at [0, 2]. (f) The sixth argument is always read
  as a mask image first, so a range threshold with an inside value prints
  " file 255 does not exist . " before using 255.
- **Evidence:** read: `ThresholdImage.cxx:431`; verified; validated.
- **fMRIPrep:** none (a range threshold with numbers).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): a lower bound must be text that `strtod` reads entirely
  (decimal, hexadecimal, `inf`, `nan`); anything else is an error
  (`larmorx_ants::cli::threshold_image`, `cstd::strtod_reads_all`). The (f) message is kept.
- **D13 / gap:** done. No gap.

### ANTS-25 ThresholdImage Otsu with a mask: its range — (c)
- **What:** ANTs' own masked Otsu starts the maximum at `NumericTraits<float>::min()`, which is
  `FLT_MIN` (the smallest positive float), and updates it only when a voxel does not lower the
  minimum: an image without positive values keeps a maximum of `FLT_MIN`, and the first voxel
  is never a candidate for the minimum.
- **Evidence:** read: `ThresholdImage.cxx:152-262` (`:186`); validated.
- **fMRIPrep:** none (not used).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the true range of the voxels in the mask
  (`larmorx_ants::threshold_image`; the command line and `lx.ants.otsu_threshold`).
- **D13 / gap:** done. No gap.

### ANTS-26 `Otsu 0` never returns — (a)
- **What:** with zero thresholds, `IncrementThresholds` never returns false.
- **Evidence:** read (not run).
- **fMRIPrep:** none.
- **larmorx:** an error.
- **D13 / gap:** never reproduced. No gap.

### ANTS-27 MultiplyImages multiplies by 0 when a file is missing — (e)
- **What:** a second argument that cannot be read as an image is `atof`'d: a missing or
  misspelt file multiplies by 0, without a message.
- **Evidence:** read: `MultiplyImages.cxx:80-86`; verified.
- **fMRIPrep:** sMRIPrep's brain extraction multiplies two images (node `08_mult_gm`); a
  missing file would zero the result silently.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): an argument that is neither a readable image nor a number is an
  error (`larmorx_ants::cli::multiply_images`); `lx.ants.multiply_images` raises
  `FileNotFoundError` in both modes.
- **D13 / gap:** done. No gap.

### ANTS-28 Uninitialised memory reaches the output — (a)
- **What:** ResampleImageBySpacing with a smoothed axis of fewer than 4 voxels catches ITK's
  exception ("Exception catched !") and resamples the smoothing filter's never-written buffer:
  values up to ±1e27, different on every run, exit 0. ResampleImage with a spacing list of the
  wrong length leaves the spacing uninitialised (then aborts); SmoothImage's median with a sigma
  list of the wrong length leaves the radius uninitialised.
- **Evidence:** verified (three runs, three outputs); read.
- **fMRIPrep:** none.
- **larmorx:** refuses with ITK's message by default. `reproduce` (Decisions, item 5):
  ResampleImageBySpacing prints what ANTs prints (ITK's two addresses and build directory as
  `0x0` and `<ITK source>`), resamples the buffer filled with zeros, writes the same file and
  exits 0, with a warning; SmoothImage's median uses radius 0. The parity case
  `errors/thin-smoothed-axis` compares everything but the values.
- **D13 / gap:** done (2026-10-12). No gap.

### ANTS-29 ResampleImage to size 1 — (c)
- **What:** by size, the spacing is `old · (n − 1) / (size − 1)`: for size 1 it is infinite;
  vnl's SVD then fails on the grid matrix and ANTs writes a 1 × 1 × 1 image of 0 with infinite
  spacing, exit 0.
- **Evidence:** read: `ResampleImage.cxx:136`; verified.
- **fMRIPrep:** none.
- **larmorx:** refuses sizes below 2 by default; `reproduce` (Decisions, item 5) writes ANTs'
  image byte for byte: spacing `inf`, origin NaN (`origin + D · diag(s) · 0`, as ITK's writer
  computes it), voxels 0 (`larmorx_ants::resample_image::output_geometry`); parity case
  `errors/size-1`.
- **D13 / gap:** done (2026-10-12). No gap.

### ANTS-30 ResampleImage reads its seventh argument twice — (e)
- **What:** `std::stoi(argv[7])` is the pixel type, and the same text is the interpolator's
  parameter (Gaussian sigmas, windowed-sinc window, B-spline order): `… 2 1.5x1.5x1.5` resamples
  an `unsigned char` image; a window letter can never be given (`… 3 l` aborts), so only the
  default Hamming window is reachable; `b` would be a second Lanczos anyway (line 76).
- **Evidence:** read: `ResampleImage.cxx:331-334`, `:144-236`; validated (`interpolation/*`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12) (`larmorx_ants::cli::resample_image`), with the design of
  [api/ants-resample-image.md](../api/ants-resample-image.md), "The seventh argument": each
  interpolator's parameters follow it where ANTs reads them (Gaussian sigma and alpha in
  arguments 7 and 8, the window letter or the B-spline order in 7), and the pixel type comes
  after them (argument 7 for linear and nearest neighbour, as in ANTs; 8 for windowed sinc and
  B-spline; 9 for Gaussian). The window `b` is Blackman, as the usage says (ANTs' code builds
  a second Lanczos). The warning comes when the two readings differ.
- **D13 / gap:** done; the design approved by the user on 2026-10-22 (decision log). No gap.

### ANTS-31 ResampleImageBySpacing's arguments — (e)
- **What:** in 2D the `nn` flag is read from the `addvox` argument whenever an eighth argument
  exists; in 4D it is read as soon as `argc > 9`, so an `addvox` without `nn` aborts; a
  dimension other than 2, 3 or 4 does nothing and exits 0.
- **Evidence:** read: `ResampleImageBySpacing.cxx:130,393`; validated.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): `nn` from its own argument in 2D and 4D, other dimensions an
  error (`larmorx_ants::cli::resample_image_by_spacing`).
- **D13 / gap:** done. No gap.

### ANTS-32 ResampleImageBySpacing's smoothing and outside value — (c)
- **What:** the smoothing sigma is `out/in − 1`, a ratio, which ITK takes in millimetres and
  divides by the spacing again (1 → 2 mm smooths one voxel, 2 → 4 mm half a voxel). Points
  beyond the input get the input's value at index `(1, 1, …)`.
- **Evidence:** read: `ResampleImageBySpacing.cxx:144,190,277,406`; validated
  (`sigma-in-voxels-known`: the 2 mm phantom to 4 mm gives, bit for bit, what ANTs gives for the
  same voxels at 1 mm to 2 mm, where its millimetres are voxels; `outside-zero-known`: ANTs'
  output with its outside voxels 0).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23, rule A): the sigma `out/in − 1` is in input
  voxels (`(out/in − 1) · in` mm, what the ratio means: 1 → 2 mm smooths one voxel at any
  spacing), and the points beyond the input are 0; a one-voxel axis is no longer smoothed into
  ITK's exception. One notice names what changed; `reproduce` gives ANTs' output.
- **D13 / gap:** done. No gap.

### ANTS-33 antsApplyTransforms ignores what it does not understand — (e)
- **What:** an unknown `-u` name silently keeps the default output type; `MultiLabel[sigma,alpha]`
  ignores alpha (always 4.0); `GenericLabel[...]` ignores its parameter (always linear).
- **Evidence:** read: `antsApplyTransforms.cxx:1453-1600`, `make_interpolator_snip.tmpl:127`.
- **fMRIPrep:** none (no alpha, known types).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): an unknown `-u` type is an error; `MultiLabel`'s alpha is used;
  `GenericLabel[Linear]` and `GenericLabel[NearestNeighbor]` are honoured, any other parameter
  is an error (`larmorx_ants::cli::apply_transforms`).
- **D13 / gap:** done. No gap.

### ANTS-34 `GenericLabel` never finishes on intensity images — (a)
- **What:** ITK's `LabelImageGenericInterpolateImageFunction` interpolates every distinct
  value of the image at every voxel; a float intensity image has as many labels as values, so
  it practically never finishes (still running after 9 minutes on a boldref → 1 mm T1w grid).
- **Evidence:** verified.
- **fMRIPrep:** none.
- **larmorx:** evaluates only the labels among the 8 neighbours, with the same result (D11).
- **D13 / gap:** not reproduced. No gap.

### ANTS-35 N4's options without brackets set nothing — (e)
- **What:** `-c`, `-b` and `-t` read only bracketed parameters: `-c 50x50`, `-b 200` and
  `-t 0.15` leave ITK's defaults (one level of 50 iterations, threshold 0.001, 4 control
  points), silently.
- **Evidence:** read: `N4BiasFieldCorrection.cxx:196-227,241-335,389-407`; validated
  (`options/no-brackets`).
- **fMRIPrep:** none (nipype writes brackets).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): an unbracketed value is the option's first parameter
  (`-c 50x50` is `-c [50x50]`; `larmorx_ants::cli::n4_bias_field_correction`).
- **D13 / gap:** done. No gap.

### ANTS-36 N4's mask and rescaling — (e)
- **What:** a mask that cannot be read is replaced by an image of ones ("Mask not read") and
  still counts as given, so restoration and rescaling run; rescaling never runs without a mask,
  and `-r 1` is the same as nothing.
- **Evidence:** read: `N4BiasFieldCorrection.cxx:113-134,495-532`; validated (`options/missing-mask`).
- **fMRIPrep:** the BOLD reference's N4 passes `-r` without `-x`: it does nothing.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): an unreadable mask is an error; `-r` without a mask does
  nothing in both modes, and a warning says so (`larmorx_ants::cli::n4_bias_field_correction`).
- **D13 / gap:** done. No gap.

### ANTS-37 N4 depends on the thread count — (b)
- **What:** ITK's B-spline fit splits the points among work units and sums per-unit lattices;
  float addition is not associative, so the result follows the thread count: against one
  thread, 2 to 12 threads move a T1w by up to 9.5e-4 relative and a BOLD reference by up to
  6.5e-3.
- **Evidence:** read: `itkBSplineScatteredDataPointSetToImageFilter.hxx:335-363,444-452,653-686`;
  verified.
- **fMRIPrep:** both brain-extraction N4s run with `NSLOTS=8`, so they depend on the machine.
- **larmorx:** ANTs' single-thread result on any thread count.
- **D13 / gap:** never reproduced. No gap.

### ANTS-38 N4's histogram range drops a first-voxel minimum — (c)
- **What:** ITK's `SharpenImage` finds the range with `if (v > max) ... else if (v < min)` from
  ∓`FLT_MAX`: the first included voxel always becomes the maximum and is never compared with
  the minimum. If it is the unique smallest value, the histogram's minimum is the next
  smallest; its own bin index is negative, wraps to a huge unsigned value, so it is left out of
  the histogram and sharpened to the last value of the mapping. If the values increase in
  memory order, the minimum is never set at all.
- **Evidence:** read: `itkN4BiasFieldCorrectionImageFilter.hxx:285-294`; validated (`inputs/ramp`).
- **fMRIPrep:** every N4 run, when the first included voxel of the log image is its unique
  minimum (with a mask: the first voxel of the mask in memory order). None of the fMRIPrep,
  sMRIPrep and sdcflows command lines of the parity suite meets it (2026-10-12); 3 masked
  cases on a generated phantom and the `inputs/ramp` case do, by up to 3.9e3.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the true range (`larmorx_image::n4::sharpen`,
  `N4Parameters::upstream_defects`); the warning comes when ITK's range differs.
- **D13 / gap:** done. No gap.

### ANTS-39 N4's FFT size for 256 bins — (c)
- **What:** the histogram is padded to `2^(ceil(logf(bins) / log(2.0)) + 1)`, a float log
  divided by a double log: for 256 bins the quotient is slightly above 8, so the FFT has 1024
  points instead of 512.
- **Evidence:** read: `itkN4BiasFieldCorrectionImageFilter.hxx:331-343`; verified
  (`options/histogram-256`).
- **fMRIPrep:** none (200 bins).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): `2^(ceil(log2(bins)) + 1)` points (`larmorx_image::n4::padding_exponent`).
  Both sizes are enough for the convolution, so they differ only in the FFTs' rounding: the
  sharpening is computed with both sizes when they differ, and the warning comes only where a
  voxel's sharpened value changes (rare: no golden or parity case so far).
- **D13 / gap:** done. No gap.

### ANTS-40 N4 keeps zero and negative values in the log image — (c)
- **What:** an included voxel is replaced by `logf(v)` only if `v > 0`; 0 stays 0 (the log of
  an intensity of 1) and negative values stay negative, inside the histogram. Without a mask, a
  raw image's zero background enters the histogram at log value 0.
- **Evidence:** read: `itkN4BiasFieldCorrectionImageFilter.hxx:137-143`; validated
  (`inputs/outliers`; `inputs/zero-background-known`: larmorx's default equals ANTs' own field
  with a mask of the positive voxels); measured
  ([investigation](../../validation/investigations/ants-40-n4-nonpositive/README.md)): 16 of
  the 25 N4 calls of the five oracle runs use 0 or negative voxels (every final anatomical N4:
  1 to 9 zeros of 10,657 to 35,692 voxels, or 43 negative values; 3 of 6 BOLD references; all 6
  field-map magnitudes, about half their voxels; both T1w-template N4s of ds005454); leaving them
  out changes the field by a global factor of 0.77 to 1.46 and its shape by 0.6 % to 36 % (99th
  percentile), against ANTs' own thread band of 2.1e-6 to 3.1e-3; on phantoms with a known field
  the shape error falls from 0.026 to 0.0097, and the white matter's CV falls on all five final
  N4s (ds005454: 0.0762 → 0.0689, where ANTs' field is 1.0000 to 1.0002: no correction).
- **fMRIPrep:** every N4 (none has a mask): sMRIPrep's final N4 of the brain extraction (and on
  ds005454 sub-16 it corrected nothing), the BOLD references, sdcflows' field-map magnitudes,
  the T1w templates' N4s.
- **larmorx:** corrected by default (2026-10-23, measured, then rule A): the voxels N4 uses whose
  value is 0, negative or NaN take no part in the histogram, the fit or the convergence measure
  (what a mask of the positive voxels does); named with their count; `reproduce` keeps ITK's log
  image ([ants-n4.md](ants-n4.md), "Zero and negative values").
- **D13 / gap:** done. No gap. (Every unmasked N4 with such voxels changes: fMRIPrep's
  comparisons at the end will see it, by design.)

### ANTS-41 ANTs built by GCC 11.4 drops two float roundings (a compiler bug) — (d)
- **What:** not a source defect: a GCC defect. ANTs 2.6.5 built by GCC 11.4 at `-O3` (the
  repository's first oracle, kept as `oracles/ants-2.6.5-gcc11-O3`) computes the B-spline
  fit's `r[0]`, `r[1]` and the points' first two coordinates in double precision in its 2D and
  3D instantiations, where the source rounds them to float (`Scattered.hxx:433-439`,
  `itkImageBase.h:589-598`). GCC's SLP vectorizer builds a `vector(2) double` →
  `vector(2) float` → `vector(2) double` round trip and match.pd's rule for two conversions in
  a row folds it away, comparing `TYPE_PRECISION` (a vector's lane count) instead of the
  element precision: GCC bug 110182, wrong code, fixed in GCC 11.5, 12.4 and 13.2. ANTs built
  at `-O3` by Ubuntu 22.04's GCC 11.4 has it (GCC 12.3 miscompiles the reproduction too).
- **Evidence:** verified: GCC's dumps (the round trips present after `slp1`, gone after
  `fre5`); a 40-line reproduction (GCC 11.4 and 12.3 miscompile it at `-O3`; `-O2`,
  `-fno-tree-vectorize`, `-fno-tree-slp-vectorize`, GCC 8.4, 13.3 and 14.3 do not); N4
  recompiled with each flag set; the disassembly; three other builds (fMRIPrep's ANTs 2.6.2 by
  GCC 14.3, ANTsPy 0.6.3 by GCC 10.2, the new oracle by GCC 14.3) follow the source
  ([ants-n4.md](ants-n4.md), "GCC 11 miscompiles the fit").
- **fMRIPrep:** none; its build follows the source.
- **larmorx:** follows the source since 2026-10-11. `gcc11_double_axes` reproduces the
  miscompiled build for comparisons only; the oracle is built by conda-forge's GCC 14.3 and
  `scripts/build_ants_oracle.sh` refuses a compiler that fails its probe of the bug.
- **D13 / gap:** none: a compiler artefact is not reproduced (rule 8), so there is no
  compatibility switch.

### ANTS-42 DenoiseImage depends on threads and races — (b)
- **What:** the filter splits the centres into slabs (4 work units per thread); each centre adds
  to the output and count of its whole patch and overwrites the bias image there, so near each
  slab border two units write the same voxels in an order that depends on scheduling. On a T1w:
  21 % of voxels change at 2 threads (by up to 97), 85 % at 12 (up to 271); from 8 threads the
  result can change from run to run (seen 2026-10-12; six runs at 8 threads and three at 12 gave
  the same bytes on 2026-10-21, `docs/findings/ants-denoise.md`).
- **Evidence:** read: AdaptiveDenoising `itkAdaptiveNonLocalMeansDenoisingImageFilter.hxx:61`,
  ITK `itkPoolMultiThreader.cxx:88-95`; verified (three runs per thread count).
- **fMRIPrep:** sMRIPrep denoises the T1w with `NSLOTS=8`: fMRIPrep's denoised T1w is not
  reproducible, even on the same machine.
- **larmorx:** ANTs' single-thread result on any thread count.
- **D13 / gap:** never reproduced. No gap.

### ANTS-43 DenoiseImage's Rician correction becomes NaN — (c)
- **What:** for a high SNR, the correction factor's `float(I₀)` overflows while `expf`
  underflows; `0 · ∞` is NaN, which passes both range comparisons, and the bias, NaN, becomes 0.
- **Evidence:** read: `itkAdaptiveNonLocalMeansDenoisingImageFilter.hxx:397-494` (`:482`).
- **fMRIPrep:** sMRIPrep's denoising runs with the Rician model, and meets it. Measured
  2026-10-12 on sMRIPrep's command (one thread's result): on ds000005's and ds000210's raw
  T1w the factor is NaN in 7.4 % and 8.4 % of the voxels, and the corrected bias lowers them
  by a median of 0.58 and 2.9, at most 2.8 and 17.7 (0.7 % of each image's 99th percentile);
  ANTs' own result moves by up to 271 between thread counts on such an image (ANTS-42). In the
  parity suite the correction changes 25 of 40 compared cases.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the factor computed in double with exponentially scaled Bessel
  functions, then ITK's range checks (`larmorx_image::adaptive_denoising`,
  `AdaptiveNlmParameters::upstream_defects`); the warning counts the voxels whose factor ITK
  makes NaN.
- **D13 / gap:** done. No gap.

### ANTS-47 DenoiseImage takes an unreadable mask for no mask — (e)
- **What:** a mask file that cannot be read (missing, misspelt) leaves the mask null, and the
  filter then runs on every voxel, exit 0; only `ReadImage`'s " file … does not exist . " says
  anything.
- **Evidence:** read: `DenoiseImage.cxx:184-195`; validated (`options/missing-mask`).
- **fMRIPrep:** none (sMRIPrep gives no mask).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): an error (`larmorx_ants::cli::denoise_image`); `reproduce` denoises
  the whole image, with a warning.
- **D13 / gap:** done. No gap.

### ANTS-48 antsApplyTransforms' transform outputs crash or abort — (a)
- **What:** with `-o Linear[...]`, `-o CompositeTransform[...]` or `-o DisplacementField[...]`:
  an output name whose extension no transform writer takes makes ITK throw, uncaught, and the
  program aborts (exit 134); an input image without a reference image is still resampled, onto
  a null reference (segmentation fault); a Gaussian or MultiLabel interpolator without an
  input image reads its default sigmas from `inputImages[0]` of an empty vector; and
  `CollapseLinearTransforms` dereferences the null `dynamic_cast` of an `IdentityTransform`.
- **Evidence:** read: `antsApplyTransforms.cxx:578-633, 693-698`,
  `itkantsRegistrationHelper.hxx:3790-3794`; verified (the oracle exits 134, −11 and −11,
  2026-10-16; the `transform-output/errors/*` cases).
- **fMRIPrep:** none.
- **larmorx:** refuses each with a message, exit 1, in both modes.
- **D13 / gap:** class (a), not reproduced. No gap.

### ANTS-44 DenoiseImage's bias outside the mask — (c)
- **What:** the voxels outside the mask that the patches of centres in the mask reach are
  denoised like the others, but ITK smooths and corrects the Rician bias only inside the mask
  (and where the smoothed bias is positive), so those voxels keep the raw noise estimate of the
  last such centre.
- **Evidence:** read: `itkAdaptiveNonLocalMeansDenoisingImageFilter.hxx:397-494`; validated
  (the unit test against a reference implementation in both modes; the record's fix pass).
- **fMRIPrep:** none (no mask).
- **larmorx:** corrected by default (2026-10-23, rule A): every voxel a patch of a centre in the
  mask reaches gets the smoothed and corrected bias; the voxels outside the mask no such patch
  reaches are not denoised, as in ITK. Named with the number of voxels it changes; `reproduce`
  gives ANTs' output.
- **D13 / gap:** done. No gap.

### ANTS-45 Errors explained only with `-v` — (f)
- **What:** N4 exits 1 on a weight image on another grid, and on a mesh of the wrong length,
  but says why only with `-v`; DenoiseImage's invalid noise model and radii likewise; so do
  most of antsRegistration's checks (a missing dimension, option or metric; inconsistent
  levels; an unreadable initial transform), which print to standard output or error only with
  `-v` (parity cases `errors/version-0`, `errors/no-dimension-quiet`,
  `errors/unbalanced-bracket`).
- **Evidence:** read; verified.
- **fMRIPrep:** none.
- **larmorx:** always prints the reason; exit codes as ANTs'.
- **D13 / gap:** not D13. No gap.

### ANTS-46 The BOLD reference's affine registration stops early — explained, not a defect
- **What:** fMRIPrep's affine registration of the BOLD reference to MNI (`Affine[0.1]`, Mattes
  64 bins, `Random 0.2`, convergence `[200, 1e-09, 10]`) ends after iteration 25 of 200 with a
  convergence value of 1.9e-6, above its threshold.
- **Explained (A4 part 2, 2026-10-17):** the last `DIAGNOSTIC` line shows the convergence value
  computed at that iteration; the optimizer stops at the *next* evaluation, before it prints
  anything, because `WindowConvergenceMonitoringFunction` gives the negative slope of a line
  fitted to the last 10 metric values, normalised by their sum: it is **negative** when the
  metric rose over the window, and `GradientDescentOptimizerv4::ResumeOptimization` stops when
  it is `≤ 1e-9`. With `Random 0.2` sampling the metric fluctuates near the optimum, so the
  registration stops as soon as it stops improving, which is what the test is for. Re-running
  fMRIPrep's command line (ds000005, run 1) with `ANTS_RANDOM_SEED=42`: ANTs and larmorx both
  print 36 identical lines and stop at the 37th evaluation, convergence value −1.56e-6.
- **Evidence:** read: `itkGradientDescentOptimizerv4.hxx:72-140` (`ResumeOptimization`, the test at 126),
  `itkWindowConvergenceMonitoringFunction.hxx`; validated (`real/ds000005-sub-01/boldref-affine`
  in `docs/validation/ants-registration.md`).
- **fMRIPrep:** every run; by design.
- **larmorx:** the same stop.
- **D13 / gap:** not a defect. No gap.

### ANTS-49 A linear stage that throws is dropped — (e)
- **What:** `AddLinearTransformToCompositeTransform` catches an ITK exception from the
  registration (a Demons metric in a linear stage, "All samples map outside moving image
  buffer", a joint PDF that sums to zero, ...), logs "Exception caught:" and returns
  `EXIT_FAILURE`, which `DoRegistration` ignores: the stage's transform is not added, the next
  stages run without it, the outputs are written and the program exits 0. Only `--verbose`
  shows the exception.
- **Evidence:** read: `itkantsRegistrationHelper.h:1072-1089`, `itkantsRegistrationHelper.hxx:1464`
  (return value unused); verified (`metrics/demons`, `metrics/no-overlap`).
- **fMRIPrep:** none seen.
- **larmorx:** corrected by default (A4 part 2): the exception is printed as ANTs prints it and the
  run stops with an error naming the stage; `reproduce` drops the stage and goes on, with a
  warning.
- **D13 / gap:** done. No gap.

### ANTS-50 `--save-state` after linear stages only — (e)
- **What:** the registration state is set by SyN stages only; after linear stages
  `antsRegistration` finds it null and returns `EXIT_FAILURE` with no message, after writing the
  warped images and before writing any transform. The option's own description says the state
  is the composite transform "unless the last transform is a SyN transform".
- **Evidence:** read: `antsRegistrationTemplateHeader.h:1301-1307`; verified
  (`initial/save-state`).
- **fMRIPrep:** none (fMRIPrep does not save states).
- **larmorx:** corrected by default (A4 part 2): the state file holds every transform of the
  registration (initial and computed, not collapsed), which `--restore-state` reads back, and
  the outputs follow; `reproduce` exits 1 as ANTs does, with a warning.
- **D13 / gap:** done. No gap.

### ANTS-51 Transform files without collapsing: names and the end of the list — (a)
- **What:** with `-z 0` the transforms are written one file each, named after the stages in
  order (`TransformTypeNames`, popped once per file). The initial transforms are skipped by
  counting the `-r` options, not the transforms they hold: a composite `-r` file, or a
  `--restore-state` file (not counted at all), makes ANTs write an initial transform under a
  stage's name, and then pop from the empty list (undefined behaviour: a garbage name, the
  writer's "Could not create IO object", a segmentation fault). With
  `--initialize-transforms-per-stage` (which needs `-a 1`, so it never reaches this code) the
  same miscount would follow.
- **Evidence:** read: `antsRegistrationTemplateHeader.h:1429-1440`; verified (the oracle exits
  −11 after writing `r1BOGUS.XXXX`, `initial/names-restored`).
- **fMRIPrep:** none (it collapses).
- **larmorx:** corrected by default (A4 part 2): every transform a stage computed is written, at
  its position, named after that stage; `reproduce` writes ANTs' files up to the point where
  ANTs reads past the end of its list, then stops with an error.
- **D13 / gap:** class (a): the crash is not reproduced. Gap (random testing, 2026-10-25,
  `ants-registration:20261025:979` and `:1353`): where the misnaming puts a stage's field under a
  `.mat` or `.txt` name, ANTs' `WriteTransform` (`itkantsReadWriteTransform.h:146-170`) writes the
  transform object itself; larmorx now writes a Gaussian or B-spline smoothing displacement field
  with its own class (bit-identical), but a time-varying (or exponential) velocity field's own
  parameters (the velocity lattice, and its fixed parameters) are not reproduced: larmorx writes
  the integrated displacement field there.

### ANTS-52 antsAI's translation grid ignored by some transforms — (c)
- **What:** for each translation of `-g [step,AxBxC]`, `antsAI` builds the start point from
  the affine search transform, translated and rotated; the `Rigid` and `Similarity` start
  points take only its matrix and the *initial* offset in 2D, and the `Similarity` one in 3D
  never translates, so every translation repeats the same start point (the 3D `Rigid` one is
  translated). `-v` counts them all ("Starting optimizer with N starting points").
- **Evidence:** read: `antsAI.cxx:1411-1432, 1478-1486`; validated (`search/2d-rigid-grid`,
  `search/similarity-grid`: the reproduced results equal the runs without a grid).
- **fMRIPrep:** none (its grids, `[40,0x40x40]`, go with `Affine`).
- **larmorx:** corrected by default (A4 part 3): the offset is translated as the 3D rigid start
  point's is; named when the best start point is a translated one. `reproduce` ignores the
  grid, with a notice.
- **D13 / gap:** done. No gap.

### ANTS-53 antsAI's alignments with iterations write no transform — (e)
- **What:** `AlignGeometricCenters` and `AlignCentersOfMass` with `-c n`, `n > 0`, go on to the
  search: the metric is set up (with its default `IdentityTransform`, whose scales are "1"
  after a warning), the loops list no start point but count them, the optimizer does nothing,
  and the transform writer gets no transform: ITK's MATLAB writer leaves an empty file (exit
  0); its text and HDF5 writers read the first transform of an empty list and crash. The help
  says "the metric isn't actually used" for these two.
- **Evidence:** read: `antsAI.cxx:831-855, 1058-1117, 1338-1577`; verified (an empty `.mat`,
  exit 0; `.txt`, `.h5`: exit −11); validated (`search/align-with-iterations*`,
  `messages/align-iterations-*`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 3): the alignment is written as with 0
  iterations; `reproduce` writes the empty `.mat`, with ANTs' messages, and refuses the text
  and HDF5 cases (the crash).
- **D13 / gap:** done. No gap.

### ANTS-54 antsAffineInitializer ignores its local-search argument — (c; c? until 2026-10-17)
- **What:** the usage describes the eighth argument as the number of local-optimization
  iterations per start point ("Default = 20"); the code skips it unread
  (`// NOT USED: localoptimizeriterations = std::stoi(...)`) and always runs
  `localSearchIterations = 20`.
- **Evidence:** read: `antsAffineInitializer.cxx:193, 235-239, 405`; validated
  (`search/local-search-text`: text there is not even parsed).
- **fMRIPrep:** niworkflows' `AffineInitializer` passes `10` (nipype's default); ANTs runs 20.
- **larmorx:** corrected by default (2026-10-17, the user's decision of that day): the argument
  is read as the commented-out line would read it (`std::stoi`) and gives each start point's
  iterations; 0 runs no local search (`if (localSearchIterations > 0)
  SetLocalOptimizer(...)`, the code's own path); text that is not such a number, a negative
  value or one beyond `int` is an error (exit 1). A notice names ANTS-54 wherever the argument
  is not 20. `reproduce` skips the argument and runs 20, with a notice when it says otherwise.
  On niworkflows' arguments (`15 0.1 0 10`, the 2 mm MNI template and fMRIPrep's T1w of
  ds000005, `real/mni-2mm` of [ants-affine-initializer.md](../validation/ants-affine-initializer.md)),
  larmorx's default (10 iterations, each start point at 0.1, with ITK-21) moves the transform
  by up to 1.41 mm in its translation and 0.15 in its matrix from ANTs' result.
  larmorprepx will pass 20, what fMRIPrep effectively runs.
- **D13 / gap:** done (2026-10-17): the argument is honoured by default, `reproduce` gives
  ANTs' 20. No gap.

### ANTS-55 antsAI reads a short translation grid past its end — (a)
- **What:** `-g [s,AxB]` in 3D (or `[s,A]`) gives a vector of fewer extents than dimensions;
  the search loops read `translationSearchGrid[2]` past its end (undefined).
- **Evidence:** read: `antsAI.cxx:756-772, 1454-1468`; verified (the oracle reads 0 on this
  machine).
- **fMRIPrep:** none.
- **larmorx:** refuses by default (A4 part 3); `reproduce` reads the missing extents as 0, with
  a notice.
- **D13 / gap:** class (a), the defined part reproduced on request. No gap.

### ANTS-56 antsAffineInitializer runs any other dimension as 2D — (e)
- **What:** `switch (std::stoi(argv[1]))` handles 2, 3 and 4; anything else falls through to
  `antsAffineInitializerImp<2>`.
- **Evidence:** read: `antsAffineInitializer.cxx:610-625`; validated (`messages/dimension-5`,
  `-1`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 3): an error; `reproduce` runs it as 2D, with a
  notice.
- **D13 / gap:** done. No gap.

### ANTS-57 antsAffineInitializer aborts on two or three arguments — (a)
- **What:** the usage is printed only below two arguments; with two or three,
  `std::string(argv[k])` of the null pointer after the last one throws `std::logic_error`
  ("basic_string: construction from null is not valid") and the program aborts.
- **Evidence:** read: `antsAffineInitializer.cxx:205-212, 590`; verified (exit 134).
- **fMRIPrep:** none.
- **larmorx:** prints ANTs' message and exits 1, in both modes.
- **D13 / gap:** class (a), not reproduced. No gap.

### ANTS-58 antsAI's Random sampling and seed 0 are not repeatable — (b)
- **What:** `Random` sampling draws its voxels with `ImageRandomConstIteratorWithIndex`, whose
  generator `antsAI` never seeds (the clock); `--random-seed 0` (or `ANTS_RANDOM_SEED=0`) leaves
  the perturbations' generator seeded from the clock too. Two runs give different transforms
  (by up to 0.04 mm on the phantoms).
- **Evidence:** read: `antsAI.cxx:1268-1334`; verified (two runs differ); validated
  (`random/*`, reported).
- **fMRIPrep:** none (`Regular` sampling with `ANTS_RANDOM_SEED` set).
- **larmorx:** seeds both generators from the seed (121212 for 0): repeatable.
- **D13 / gap:** class (b), not reproduced. No gap.

### ANTS-59 antsAI's convergence window: help and code disagree — (f)
- **What:** `--convergence [n,threshold,window]`'s help gives `convergenceWindowSize=10`; the
  code's default is 5.
- **Evidence:** read: `antsAI.cxx:720, 1696`.
- **fMRIPrep:** none (nipype passes the window).
- **larmorx:** follows the code; the menus print ANTs' text; documented in `docs/api/ants-ai.md`.
- **D13 / gap:** not D13. No gap.

### ANTS-60 antsAI's blob detector writes past its per-thread heaps — (a)
- **What:** `MultiScaleLaplacianBlobDetectorImageFilter::GenerateData` sizes `m_BlobHeapPerThread`
  by `GetGlobalDefaultNumberOfThreads()` ("FIXME - setting to 2 threads"), but runs
  `ThreadedGenerateData` through `SingleMethodExecute` on the multi-threader's work units,
  which ITK 5's pool threader makes four times the thread count when there is more than one
  thread: work units past the thread count index the vector past its end.
- **Evidence:** read: `Utilities/itkMultiScaleLaplacianBlobDetectorImageFilter.hxx:79-81, 199`,
  ITK v5.4.5 `Modules/Core/Common/src/itkPoolMultiThreader.cxx:88-95`; verified 2026-10-23:
  `antsAI -b` crashes (signal 11) at 2, 4 and 6 threads in every run, on fMRIPrep's T1w of
  ds000005 and on phantoms (the record's thread band); at one thread its runs are identical.
- **fMRIPrep:** none (it does not align blobs).
- **larmorx:** computes the one-work-unit result at any thread count (2026-10-23).
- **D13 / gap:** class (a), not reproduced. No gap.
### ANTS-61 IGDM with another kind of metric gets no input — (c)
- **What:** `DoRegistration` instantiates the registration method for one kind of point set,
  chosen by the stage's **first** metric (`if (stageMetricList[0].m_MetricType != IGDM)`, the
  labelled point sets; otherwise the intensity ones), and gives it only that kind. A stage
  that combines IGDM with ICP, PSE or JHCT, or lists IGDM after an image metric, leaves the
  other metrics without their point sets: `ImageRegistrationMethodv4` throws "Mismatch between
  number of image pairs and the number of metrics" (the stage is dropped, ANTS-49), or, where
  the counts happen to agree, a metric is cast to the other kind's type and ANTs crashes.
- **Evidence:** read: `itkantsRegistrationHelper.hxx:1188-1240, 1462-1735`,
  `itkImageRegistrationMethodv4.hxx:306`; validated (`point-sets/icp-then-igdm`,
  `point-sets/mattes-then-igdm`, [ants-registration](../validation/ants-registration.md)).
- **fMRIPrep:** none (no point-set metric).
- **larmorx:** corrected by default (A4 part 4): every metric of the stage gets its own inputs,
  with a notice (on the synthetic cases the corrected stage moves the transform by 2e-6 to 0.56
  from ANTs' dropped stage). `reproduce` throws ITK's exception as ANTs, or stops with a message
  where ANTs crashes. SyN stages (A5 part 1): ANTs' SyN method holds labelled point sets only,
  so every IGDM metric is without input (IGDM first is refused by ANTs itself, "Intensity point
  set metric is not implemented yet for the specified transform."); corrected, IGDM runs in
  tangent space with its own inputs (`syn-point-sets/cc-and-igdm`, `cc-igdm-icp`).
- **D13 / gap:** done. No gap.

### ANTS-62 A text point set's final newline adds a record — (a)
- **What:** `ReadPointsFromAvantsFile` loops `while (!inputFile.eof())`; after the last line's
  newline the stream is not yet at its end, so one more iteration reads nothing, and the point
  and label it adds are the loop's uninitialised variables (in the oracle's build, the previous
  record's values: a copy of the last point).
- **Evidence:** read: `Utilities/itkLabeledPointSetFileReader.hxx:167-197`; verified (the
  oracle reads one more point); validated (`point-sets/icp-text`,
  `point-sets/icp-text-no-final-newline`, `point-sets/jhct-text`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 4): no extra record, with a notice naming the file
  (on `icp-text` the transform moves by up to 0.012). `reproduce` adds a copy of the last
  record, as the oracle's build does, with a notice that the value is undefined in ANTs.
- **D13 / gap:** class (a), its defined part reproduced on request. No gap.

### ANTS-63 A binary VTK point set's labels are read short — (a)
- **What:** `ReadScalarsFromVTKFile` allocates `numberOfPoints × numberOfComponents` integers but
  reads `numberOfComponents * sizeof(float)` bytes: one point's labels. The others are
  uninitialised memory.
- **Evidence:** read: `Utilities/itkLabeledPointSetFileReader.hxx:348-360`; validated
  (`point-sets/icp-vtk-binary`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 4): every label is read, with a notice (the
  synthetic case's transform moves by up to 1.7). `reproduce` reads ANTs' bytes and takes the
  unread labels as 0, with a notice.
- **D13 / gap:** class (a), the defined part reproduced on request. No gap.

### ANTS-64 A point set's random fraction is seeded from the clock — (b)
- **What:** a sampling percentage below 1 keeps each point with probability `p`, drawn from a
  Mersenne twister seeded with `SetSeed()`, the clock; `ANTS_RANDOM_SEED` does not reach it.
  Two runs of ANTs keep different points.
- **Evidence:** read: `Utilities/itkLabeledPointSetFileReader.hxx:99-125`; verified (two runs
  differ); validated (`point-sets/icp-sampled`, an expected divergence).
- **fMRIPrep:** none.
- **larmorx:** the generator is seeded from the registration's seed (`--random-seed`,
  `ANTS_RANDOM_SEED`, else 121212): repeatable.
- **D13 / gap:** class (b), not reproduced. No gap.

### ANTS-65 IGDM's estimated intensity sigma — (c)
- **What:** `EstimateIntensityDistanceSigma` tracks each point set's extremes with
  `if (v > max) max = v; else if (v > min) min = v;`: the minima start at the largest value and
  are never updated. It then takes `std::max(maxMoving, maxFixed) - std::min(minMoving,
  maxMoving)`, with the moving maximum where the fixed minimum belongs: the result is the
  difference of the two maxima (0 when they agree, and then the larger maximum, the code's
  fallback), not the intensities' range.
- **Evidence:** read: `Temporary/itkMeanSquaresPointSetToPointSetIntensityMetricv4.hxx:224-307`;
  validated (every `point-sets/igdm*` case without given sigmas).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 4): the range of both point sets' centre
  intensities (the larger maximum when it is 0, as ANTs falls back), with a notice giving both
  values (on the synthetic `igdm` case 131.9 against ANTs' 320.0). `reproduce` as ANTs.
- **D13 / gap:** done. No gap.

### ANTS-66 IGDM's value reads before its intensity arrays — (a)
- **What:** `GetLocalNeighborhoodValue` (the line search's value) computes the neighbourhood's
  centre as `static_cast<SizeValueType>(-0.5 * numberOfVoxelsInNeighborhood) * (D + 1)`: for a
  neighbourhood of more than one voxel (a radius of 1 or more) a negative number converted to
  an unsigned index, and the intensities are read far before both arrays (undefined).
  `GetLocalNeighborhoodValueAndDerivative` uses `+0.5`, the centre.
- **Evidence:** read: `Temporary/itkMeanSquaresPointSetToPointSetIntensityMetricv4.hxx:348-355,
  398-403`; validated (`point-sets/igdm-neighbourhood`, `point-sets/igdm-2d`: expected
  divergences; `point-sets/igdm-2d-radius-0`, a radius of 0, passes).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 4): the value reads the centre as the derivative
  does, with a notice. `reproduce` takes the two intensities as equal (the intensity term 1),
  with a notice; on one 3D case that equals the oracle's output by chance.
- **D13 / gap:** class (a), the defined part reproduced on request. No gap.

### ANTS-67 IGDM's `kNeighborhood` and `gradientSigma` do nothing — (c?)
- **What:** `IGDM[...,kNeighborhood,gradientSigma]` are parsed; `kNeighborhood` goes to
  `AddMetric` but IGDM's setup never passes it on (one nearest point), and `gradientSigma`
  reaches `ImageIntensityAndGradientToPointSetFilter::SetSigma`, which only the Gaussian
  gradient filter would use; the filter uses central differences
  (`m_UseCentralDifferenceFunction = true`).
- **Evidence:** read: `antsRegistrationTemplateHeader.h:1114-1160`,
  `itkantsRegistrationHelper.hxx:1067-1095`, `Utilities/itkImageIntensityAndGradientToPointSetFilter.hxx:29-95`.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A5 part 3, 2026-10-22): the value and derivative are the
  means over the `kNeighborhood` nearest moving points, and the gradients are
  `GradientRecursiveGaussianImageFilter` derivatives of sigma `gradientSigma` mm (the filter's
  other branch; `NormalizeAcrossScale` off, the image's direction used; a sigma not above 0
  keeps central differences); a notice names both. `reproduce`: both ignored, as ANTs.
- **Measured** ([ants-velocity.md](ants-velocity.md)): on the point-set phantoms (a shift of
  3.77 mm, given sigmas) the translation error falls from ANTs' 3.77 mm (IGDM does not move)
  to 0.92 mm with 5 neighbours and 0.62 mm with 5 neighbours and gradients of 1.5 mm.
- **D13 / gap:** decided 2026-10-21 (decision log): a defect, corrected by default with a
  switch. Done. No gap.

### ANTS-68 IGDM's derivative lacks the Gaussians' 1/σ² — (c?)
- **What:** the value is `−exp(−½ d²/σ_d²) · exp(−½ Δ²/σ_i²)`; the local derivative is
  `(closest − point) · I · D + I · D · Δ · ∇I` without the `1/σ_d²` and `1/σ_i²` of the
  gradient: its direction is not the value's gradient where the two terms disagree, and its
  size scales with `σ²`.
- **Evidence:** read: `Temporary/itkMeanSquaresPointSetToPointSetIntensityMetricv4.hxx:396-417`.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A5 part 3): `(c − p)/σ_d² · I·D + I·D·Δ·∇I/σ_i²`, the
  value's gradient (ITK's descent direction, as for the other point-set metrics); a notice.
  `reproduce` as ANTs.
- **Measured:** with one neighbour and central differences (the derivative alone), ANTs' IGDM
  does not move on the phantoms (3.77 mm error, the whole shift: its derivative is no direction
  the line search can descend); corrected 3.30 mm after 40 iterations.
- **D13 / gap:** decided 2026-10-21: a defect, corrected by default with a switch. Done. No gap.

### ANTS-69 Crashes and a hang with point-set metrics — (a)
- **What:** ANTs crashes (a) in a stage of point-set metrics without a fixed mask (the spacing
  of a null virtual image); (b) with warped outputs, `--print-similarity-measure-interval` or
  `--write-interval-volumes` when the first metric is a point-set metric (the observer and the
  resampler use null images); (c) with a point set it cannot read (a message, then a null
  point set); (d) with a Regular or Random sampling strategy in a stage that also holds a
  point-set metric (`SetMetricSamplePoints` casts every metric to an image metric); (e) when the
  transform's parameters become NaN (the k-d tree finds no point and an index past the moving
  set is used; IGDM with ANTS-70). A text point set with a word among its numbers sets the
  stream's fail bit without reaching its end, and the reader loops forever.
- **Evidence:** read: `itkantsRegistrationHelper.hxx` (the virtual domain, the observer),
  `itkImageRegistrationMethodv4.hxx` (`SetMetricSamplePoints`),
  `Utilities/itkLabeledPointSetFileReader.hxx:174`; verified (segmentation faults, the hang);
  validated (`point-sets/icp-no-mask`, `-warped`, `-print-similarity`, `-interval-volumes`,
  `-missing`, `-short-name`, `mattes-regular-then-icp`: rejected by both, ANTs' output up to the
  crash compared; `icp-text-garbage`: an expected divergence).
- **fMRIPrep:** none.
- **larmorx:** prints what ANTs printed up to the crash and stops with a message naming it
  (exit 1); refuses the garbage text file.
- **D13 / gap:** class (a), not reproduced. No gap.

### ANTS-70 IGDM's estimated distance sigma can be 0 — (c?)
- **What:** `EstimateEuclideanDistanceSigma` takes the **standard deviation** of the distances
  from each fixed point to its nearest moving point (Welford's running variance). When every
  distance is the same, identical point sets above all (an image registered to itself), the
  sigma is 0, the distance term `exp(−½ (d/0)²)` is NaN, and so are the parameters after the
  first step.
- **Evidence:** read: `Temporary/itkMeanSquaresPointSetToPointSetIntensityMetricv4.hxx:176-219`;
  verified by larmorx (identical masks: NaN parameters).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A5 part 3): an estimate below one voxel of the virtual
  domain (its smallest spacing; 0 above all) is raised to that voxel, with a notice giving both.
  `reproduce` keeps the estimate; where the parameters become NaN, the nearest-neighbour search
  finds no point and IGDM's `GetPointData` throws ("The corresponding data for point [nan, …]
  (pointId = 18446744073709551615) does not exist."), the stage is dropped (ANTS-49), as in
  ANTs (verified; the NaN signs ANTs prints depend on its compiler: the parity suite masks them).
- **Measured:** a point set registered to itself moved by half a voxel: ANTs' parameters become
  NaN and the stage is dropped; corrected, the registration runs. On the phantoms the estimate
  is below the 2 mm voxel and the floor applies (translation error 3.33 mm against ANTs' 3.77 mm
  with estimated sigmas).
- **D13 / gap:** decided 2026-10-21: a defect, corrected by default with a switch. Done. No gap.


### ANTS-71 `--print-similarity-measure-interval` in SyN stages crashes — (a)
- **What:** the SyN observer computes the full-scale similarity (CC of radius 4 between the
  original images) with the registration's fixed initial transform and the moving composite's
  transforms as it finds them: without `-q` the fixed transform is a null pointer, without a
  moving transform before the stage the composite it reads is empty, and with several metrics
  it casts the multi-metric to an image metric (null). Each is a segmentation fault after the
  first `DIAGNOSTIC` line of the interval.
- **Evidence:** read: `antsDisplacementAndVelocityFieldRegistrationCommandIterationUpdate.h` (the SyN observer's
  `UpdateFullScaleMetricValue`); verified; validated (`syn/print-similarity-crash`,
  `-multi-crash`: rejected by both, ANTs' output up to the crash identical).
- **fMRIPrep:** none (fMRIPrep does not print the similarity).
- **larmorx:** `reproduce` prints what ANTs printed up to the crash and stops with a message;
  corrected (the default), the missing transforms are the identity and the first metric's
  images are compared.
- **D13 / gap:** class (a), not reproduced. No gap.

### ANTS-72 Linear stages after a displacement field are not repeatable — (b)
- **What:** `RegistrationParameterScalesFromPhysicalShift` estimates the scales and the
  learning rate from the corners of the virtual domain only when the moving transform is
  linear; when the composite holds a displacement field (a SyN stage before, `-r field.nii.gz`,
  a restored state), it samples 1000 voxels (`SizeOfSmallDomain`) with
  `ImageRandomConstIteratorWithIndex`, whose Mersenne twister ITK seeds from the clock. Two runs
  of ANTs differ from the first iteration of that stage, `--random-seed` or not.
- **Evidence:** read: `itkRegistrationParameterScalesEstimator.hxx` (`SampleVirtualDomainRandomly`);
  verified (two ANTs runs of `syn/then-linear` differ).
  Also met by the corrections record's ITK-35 references (2026-10-23): ANTs with a B-spline
  transform sampled as a displacement field, then a linear stage, at one thread with
  ANTS_RANDOM_SEED fixed, gives five different transforms in five runs (up to 8.5 mm apart,
  record `ants-registration-corrections`, table "ITK-35's references"); with `time()` and
  `clock()` fixed by a preloaded
  library the runs are identical, and the first metric value of the linear stage is the same in
  every run (its scales differ from the second iteration on). The record runs that reference
  five times and gives the median, the range and ANTs' own spread.
- **fMRIPrep:** none: its linear stages come before SyN.
- **larmorx:** draws the voxels with the stage's seed, so the result is repeatable; the parity
  cases are expected divergences.
- **D13 / gap:** class (b), not reproduced. No gap.

### ANTS-73 Any image is read as an initial displacement field — (e)
- **What:** `itk::ants::ReadTransform` reads a file whose last extension is not a transform
  file's as `Image<Vector<R, D>, D>`. ITK's reader converts whatever the file holds
  (`ConvertPixelBuffer`): a scalar image gives vectors whose components all equal the voxel's
  value (in 2D both "complex" parts), two components in 3D their product in all three
  ("intensity and alpha"), two in 4D only the first vector (the conversion never advances its
  output; the others are uninitialised), more components the first `D`, and an image of fewer
  dimensions is one slice thick. ANTs registers with the result without a word.
- **Evidence:** read: `itkConvertPixelBuffer.hxx:47-160, 290-490, 540-615`; validated
  (`syn/initial-scalar`, `-scalar-2d`, `-field-2d-in-3d`: identical to ANTs with the switch).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A5 part 1): refused with a message naming ANTS-73;
  `reproduce` converts as ITK (the unset vectors of the 4D case are zeros, with a notice).
- **D13 / gap:** done. No gap.

### ANTS-74 `--save-state` after linear stages that follow SyN — (c)
- **What:** the registration state is set only by SyN stages (and `--restore-state`). After a
  SyN stage followed by linear stages, `--save-state` writes the SyN stage's state (its
  composite, fixed-to-middle and moving-to-middle transforms) without the linear stages,
  although the option's description says the state is the composite transform unless the last
  transform is SyN's.
- **Evidence:** read: `antsRegistrationTemplateHeader.h:1301-1341`,
  `itkantsRegistrationHelper.hxx:2197-2205`; validated (`syn/save-state-then-translation`,
  `syn/restore-state-then-translation`: the saved state identical to ANTs' with the switch,
  checked by hand; the cases are expected divergences through ANTS-72).
- **fMRIPrep:** none (fMRIPrep does not save states).
- **larmorx:** corrected by default (A5 part 1): the state holds every transform, as ANTS-50's
  correction does after linear stages; `reproduce` writes ANTs' stale state.
- **D13 / gap:** done. No gap.

### ANTS-75 The observer's transform copies refuse fields and B-spline transforms — (c)
- **What:** `antsRegistrationOptimizerCommandIterationUpdate` copies the fixed and moving
  composites for the full-scale similarity (`--print-similarity-measure-interval`) and for the
  interval volumes (`--write-interval-volumes`) with `CreateAnother()`, `SetParameters`, then
  `SetFixedParameters`. A new displacement field transform has no field when its parameters are
  set, and throws (`Input parameters size (..) does not match internal size (0).`); a new
  B-spline transform has a mesh of 1 (`D·4^D` parameters) and throws for any other mesh
  (`Mismatch between parameters size .. and expected number of parameters ..`). The exception
  ends the stage at its first iteration: with an initial field, or in a
  `GaussianDisplacementField`, `BSplineDisplacementField` or `BSpline` stage, the options cannot
  be used.
- **Evidence:** read: `antsRegistrationOptimizerCommandIterationUpdate.h:256-302, 370`,
  `itkDisplacementFieldTransform.h:281`, `itkBSplineBaseTransform.hxx:50`; validated
  (`displacement-field/gaussian-print-similarity`, `-interval-volumes`,
  `bspline/print-similarity`, and `bspline/print-similarity-mesh-1`, where the copy works).
- **fMRIPrep:** none (fMRIPrep sets neither option).
- **larmorx:** corrected by default (A5 part 2): the similarity and the volumes use the
  transforms as they are, with a notice; `reproduce` fails as ANTs.
- **D13 / gap:** done. No gap.

### ANTS-76 The collapse loses transforms around B-spline transforms — (c)
- **What:** `CollapseCompositeTransform` (`-z 1`, the default) handles linear and
  displacement-field runs. A transform of another category (a `BSplineTransform`) is kept as it
  is but upsets the run logic: the transforms before the first linear transform or field are
  added as they are, and a first linear transform or field that is the queue's last is never
  added (`-r bspline.txt` then a SyN stage: the SyN fields are lost, only `0BSpline.txt` is
  written); after a linear or field run, a B-spline transform following another is never added
  at the end of the queue, and before more linear transforms or fields it joins their run, whose
  collapse throws outside a try block (a crash).
- **Evidence:** read: `itkantsRegistrationHelper.hxx:3896-3992`; validated
  (`bspline-syn/initial-bspline`, `bspline/initial-bspline-syn`,
  `bspline/initial-bspline-syn-composite`: reproduced; corrected, the fields are written).
- **fMRIPrep:** none (fMRIPrep uses no B-spline transform).
- **larmorx:** corrected by default (A5 part 2): every B-spline transform is kept and ends the
  run before it, the linear and field runs around them collapsed as ANTs collapses runs, a
  notice where the result differs; `reproduce` collapses as ANTs and stops where it crashes.
- **D13 / gap:** done. No gap.

### ANTS-77 A B-spline mesh size of 0 — (a)
- **What:** a `BSpline` stage whose mesh has a size of 0, or fewer sizes than dimensions (the
  missing ones read past the end of the vector, `itkantsRegistrationHelper.hxx:3474`), divides
  the physical dimensions by 0: the coefficient grid's spacing is infinite and its geometry NaN,
  vnl prints its SVD warnings ("suspicious return value (3) from SVDC") for every inversion, and
  the registration throws on an image region with garbage indices (`-4611686018427387904`).
- **Evidence:** verified (`BSpline[0.2,0x2x2]`, `BSpline[0.2,2x2]` in 3D: exit 1 after the
  warnings).
- **fMRIPrep:** none.
- **larmorx:** refused with a message (exit 1), after the stage's messages (A5 part 2);
  `bspline/mesh-0` and `bspline/mesh-short` are expected divergences.
- **D13 / gap:** none (class (a): never reproduced).

### ANTS-78 BSplineDisplacementField doubles its base-level mesh — (c?)
- **What:** `-t BSplineDisplacementField[step,updateMesh,totalMesh,order]` takes the meshes "at
  the base level" (the option's help and the log's `updateMeshSizeAtBaseLevel`), but the level
  adaptors set `mesh << (level + 1)`: the base level already uses twice the mesh asked for.
  B-spline SyN and the `BSpline` transform use `mesh << level`.
- **Evidence:** read: `itkantsRegistrationHelper.hxx:1933-1934` (against `:2302-2303` for
  B-spline SyN and `:3493` for `BSpline`); validated (every `displacement-field/bspline-*` case reproduces it).
- **fMRIPrep:** none (fMRIPrep uses no B-spline displacement field).
- **Also:** `BSplineExponential`'s adaptor does the same (A5 part 3).
- **larmorx:** corrected by default (A5 part 3): `mesh << level`, a notice wherever a mesh is
  positive. The corrected stage with twice the mesh is ANTs' stage byte for byte (verified for
  both stages). A coarse mesh at a shrunken level can leave the scales estimator's probe
  nothing (every control point on the pinned boundary): ITK-42, corrected too.
- **Measured:** on the registration phantoms the fields move by up to 3.0 to 5.5 mm (mean 1.1 to
  2.1 mm) for meshes of 3 to 5 or a knot spacing of 12 mm, 0.12 mm for `BSplineExponential`.
- **D13 / gap:** decided 2026-10-21: a defect, corrected by default with a switch. Done. No gap.


### ANTS-79 The time-varying stages read their parameters from the last `-t` — (e)
- **What:** for `TimeVaryingVelocityField` the number of time indices, and for
  `TimeVaryingBSplineVelocityField` the mesh, are parsed from
  `transformOption->GetFunction(0)` (the last `-t` on the command line) instead of the stage's
  own function: with several stages, each takes the last one's.
- **Evidence:** read: `antsRegistrationTemplateHeader.h` (the two cases); validated
  (`time-varying/two-stages`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: each stage its own, a notice where they differ;
  `reproduce` as ANTs.
- **D13 / gap:** class (e), corrected with a switch. No gap.

### ANTS-80 The exponential stages take their sigmas as millimetres — (c)
- **What:** `-s 2x0vox` in an `Exponential` or `BSplineExponential` stage: the stage never calls
  `SetSmoothingSigmasAreSpecifiedInPhysicalUnits`, so the method's default (physical units)
  applies; ANTs logs the sigmas as given.
- **Evidence:** read: `itkantsRegistrationHelper.hxx:3092-3360`; validated (`-s 2x0mm` gives
  the result of `-s 2x0vox` in ANTs: `exponential/sigmas-in-mm`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: the units as given, a notice; `reproduce` as ANTs.
- **D13 / gap:** class (c), corrected with a switch. No gap.

### ANTS-81 `Exp` writes no inverse field — (e)
- **What:** without collapsing, a stage's files are named after the name it was given, and the
  list of transforms with an inverse field has `exponential` but not `exp`: `-t Exp[...] -z 0`
  writes `Warp` and `VelocityField` but no `InverseWarp`.
- **Evidence:** read: `antsRegistrationTemplateHeader.h:1292-1532`; validated
  (`exponential/short-name`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: written, a notice; `reproduce` as ANTs.
- **D13 / gap:** class (e), corrected with a switch. No gap.

### ANTS-82 Three stages sample from the clock — (b)
- **What:** `Exponential`, `BSplineExponential` and `TimeVaryingVelocityField` never call
  `MetricSamplingReinitializeSeed`: a `Regular` or `Random` sampling strategy draws its points
  with ITK's generator seeded from the clock, `--random-seed` and `ANTS_RANDOM_SEED`
  notwithstanding (`TimeVaryingBSplineVelocityField` reseeds through
  `PrepareRegistrationMethod`).
- **Evidence:** read: `itkantsRegistrationHelper.hxx` (the four cases); verified (two runs
  differ); validated (the `*-sampling` cases: expected divergences).
- **fMRIPrep:** none.
- **larmorx:** the registration's seed: repeatable.
- **D13 / gap:** class (b), not reproduced. No gap.


### ANTS-90 Atropos' second k-means changes nothing — (c)
- **What:** with several intensity images, `KMeans` labels the voxels by a k-means of the first
  image, then runs a second k-means on all images. It seeds that one with
  `stats->GetMean(j + 1)` (`Atropos.hxx:1163`): the statistics of the *mask* on the first image,
  so class 1 starts at the mask's mean and the others at 0 (ITK's value for a missing label);
  and it then relabels the voxels from the first classifier's output (`classifier`, not
  `classifier2`, `:1260`). The second k-means is computed and thrown away.
- **Evidence:** read; validated (`kmeans-multivariate`, `log-euclidean`: larmorx reproduces
  ANTs bit for bit with CORE-MATH; corrected, the initial labelling changes).
- **fMRIPrep:** none (sMRIPrep segments one image).
- **larmorx:** corrected by default: the second k-means starts from each class's means in every
  image and its labels are kept, with a notice where they differ; `reproduce` keeps the first
  k-means' labels (A6).
- **D13 / gap:** none.

### ANTS-91 Atropos' Otsu range — (c)
- **What:** `GenerateInitialClassLabelingWithOtsuThresholding` starts the maximum at
  `NumericTraits<float>::min()` (the smallest positive float, not the lowest) and updates it in
  an `else if` after the minimum (`Atropos.hxx:907-922`): a voxel that lowers the minimum never
  raises the maximum, and an image of negative values keeps a maximum of 1.2e-38. The 200-bin
  histogram then misses values or spans an empty range.
- **Evidence:** read; validated (the parity suite's Otsu cases).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: the image's range in the mask, with a notice where it
  differs (A6).
- **D13 / gap:** none.

### ANTS-92 Atropos' random labels outside the mask — (c)
- **What:** `Random` draws a label for every voxel of the image (`Atropos.hxx:622-629`); the
  voxels outside the mask keep theirs until the first update, and enter the classes' first
  estimates.
- **Evidence:** read; validated (`random`, `real/ds000005/random`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: the voxels outside the mask stay unlabelled (the draws
  keep ANTs' order, so the voxels inside get ANTs' labels), with a notice (A6).
- **D13 / gap:** none.

### ANTS-93 Atropos' adaptive smoothing weights in reverse order — (c)
- **What:** `Atropos.cxx:612-637` loops over the `-a` options from the first given to the last
  (the parser stores them in reverse), sets image `count`, but reads its weight from
  `GetFunction(count)`, counted from the last option: with several images, the first gets the
  last option's weight.
- **Evidence:** read.
- **fMRIPrep:** none.
- **larmorx:** corrected by default: each image its own weight, with a notice where they differ
  (A6).
- **D13 / gap:** none.

### ANTS-94 Atropos' outlier filters keep their previous output — (c)
- **What:** the outlier filter is created once and its output list is never cleared.
  `BoxPlotQuantileListSampleFilter::GenerateData` re-runs itself on its own output (an
  `Update` inside `GenerateData`, verified with ITK's code instrumented), so the whiskers come
  from a histogram of the input winsorized at bounds computed from the previous call's values
  plus the kept ones; `GrubbsRosnerListSampleFilter` appends to its output, so Atropos'
  samples of the second and later images are the first image's values, and the sample grows by
  the number of masked voxels at every call (then ANTS-107).
- **Evidence:** read (`antsBoxPlotQuantileListSampleFilter.hxx:46-140`,
  `antsGrubbsRosnerListSampleFilter.hxx`, `Atropos.hxx:1760-1800`); validated (`box-plot`,
  `grubbs-rosner`: identical with CORE-MATH; the multivariate Grubbs case stops identically).
- **fMRIPrep:** none (sMRIPrep uses no outlier handling).
- **larmorx:** corrected by default: each call filters its own input, with a notice where the
  result differs (A6).
- **D13 / gap:** none.

### ANTS-95 ManifoldParzenWindows ignores the posterior weights — (c)
- **What:** `ManifoldParzenWindowsListSampleFunction` builds its Parzen windows from the whole
  sample and `Evaluate` weighs every sample alike: the posterior weights Atropos sets are never
  used, so every class has the same density up to a constant.
- **Evidence:** read (`antsManifoldParzenWindowsListSampleFunction.hxx:175`); validated
  (`manifold-parzen`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: the windows weighed by the posteriors, with a notice (A6).
- **D13 / gap:** none.

### ANTS-96 Partial-volume classes are never re-estimated — (c)
- **What:** `UpdateClassLabeling` re-estimates a partial-volume class only when
  `n == labelSet[d] − 1` (`Atropos.hxx:1413-1421`), `n` the partial-volume class's own index:
  never true. Its Gaussian keeps the means of the initial labelling.
- **Evidence:** read; validated (`partial-volume-likelihoods`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: re-estimated at every iteration from its two classes'
  posteriors, with a notice (A6).
- **D13 / gap:** none.

### ANTS-97 Sparse priors store voxel numbers in float — (c)
- **What:** with `--minimize-memory-usage` the prior probability images are kept as point sets
  whose points are the voxel numbers stored as `float` coordinates: above 2^24 voxels the
  numbers round, and priors land on neighbouring voxels.
- **Evidence:** read (`Atropos.hxx:190-230`, `:279-300`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: exact voxel numbers, with a notice where rounding would
  have moved a prior (A6).
- **D13 / gap:** none.

### ANTS-98 Atropos' K-means range from the mask's label 1 — (c)
- **What:** `KMeans` spreads its initial centres over the first image's range among the
  voxels whose mask value is 1 (`LabelStatisticsImageFilter::GetMinimum(1)`,
  `Atropos.hxx:1011-1012`), although every non-zero voxel is segmented. A mask without 1s (a
  mask of 255s) gives ITK's defaults for a missing label, `FLT_MAX` and `−FLT_MAX`: infinite
  centres in `float`, every voxel in one class, and ITK's LabelMap error.
- **Evidence:** read; validated (`mask-255`: both stop with the same message).
- **fMRIPrep:** none (fMRIPrep's masks hold 0 and 1).
- **larmorx:** corrected by default: the range of the mask's non-zero voxels, with a notice
  where it differs (A6).
- **D13 / gap:** none.

### ANTS-99 Atropos without -d is always 3D — (e)
- **What:** without `-d`, `Atropos.cxx:1710-1712` asks a new image IO for the number of
  dimensions before it has read the file's header; NIfTI's IO answers its default, 3. A 2D or
  4D image is then read as 3D.
- **Also** N4BiasFieldCorrection and DenoiseImage (`N4.cxx:917-919`, `DenoiseImage.cxx:647-649`):
  the same call before the header is read. Found by random testing (2026-10-25,
  `ants-denoise:20261025:188` and `:254`: a 2D and a 4D image without `-d`, written by ANTs as
  3D); until then larmorx used the file's dimension in both modes with no notice. Now the file's
  dimension by default, 3 in `reproduce`, ANTS-99 named in both modes.
- **Evidence:** read; validated (`no-dimension`).
- **fMRIPrep:** none (nipype passes `--image-dimensionality`).
- **larmorx:** corrected by default: the first image's dimension, with a notice (A6).
- **D13 / gap:** none.

### ANTS-100 Vector priors all hold the last component — (c)
- **What:** prior probability images given as one vector image are extracted with one
  `VectorIndexSelectionCastImageFilter`, its index set and `Update`d for each class, and its
  output pointer handed to the filter each time (`Atropos.cxx:242-249`): every class keeps the
  same image, the last component, which each class's normalization then rescales in place.
- **Evidence:** read; validated (`priors-vector`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: each class its component, with a notice (A6).
- **D13 / gap:** none.

### ANTS-101 An unknown posterior formulation runs Socrates — (e)
- **What:** `-p Zeno` matches none of the four names (`Atropos.cxx:344-360`) and Atropos runs
  its default formulation, silently.
- **Evidence:** validated (`unknown-formulation`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: an error naming the four; `reproduce` runs Socrates, with
  a notice (A6).
- **D13 / gap:** none.

### ANTS-102 `--icm [1,2]` aborts — (a)
- **What:** `Atropos.cxx:711` converts the `--icm` option's name to a bool; in the documented
  form `--icm [async,iterations,codes]` the name is empty, the parser throws, and ANTs aborts
  ("Unable to convert '' to type 'b'"). `--icm 1[1,2]` works. nipype's `Atropos` interface
  writes the bracketed form whenever `icm_use_synchronous_update` is set
  (`nipype/interfaces/ants/segmentation.py:219-223`), so such nipype calls abort with ANTs 2.6.5.
- **Evidence:** validated (`icm-brackets`).
- **fMRIPrep:** none (sMRIPrep does not set the ICM options).
- **larmorx:** read as documented; `reproduce` stops with ANTs' message (exit 1) (A6).
- **D13 / gap:** none.

### ANTS-103 The joint shape and orientation model reads an unset coordinate — (a)
- **What:** `JointHistogramParzenShapeAndOrientationListSampleFunction::Evaluate` looks its
  shape and first orientation histograms up at the measurement's first two components (raw
  tensor values, not the eigenvalues and angles the histograms were filled with), at a point
  whose second coordinate it never sets
  (`antsJointHistogramParzenShapeAndOrientationListSampleFunction.hxx:581-608`).
- **Evidence:** read; validated (`joint-shape-orientation`: with the unset coordinate 0, larmorx
  reproduces the oracle's result).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: evaluated at the tensor's normalized eigenvalues and its
  principal direction's angles, where the histograms were filled; `reproduce` takes the unset
  coordinate as 0, with a notice (A6).
- **D13 / gap:** none.

### ANTS-104 ManifoldParzenWindows' local covariances — (c)
- **What:** with a covariance neighbourhood (`ManifoldParzenWindows[s,k,c>0,σ]`) each sample's
  covariance accumulates its neighbours' products into a running matrix whose diagonal is
  doubled at every neighbour and whose lower triangle receives the running upper values.
- **Evidence:** read (`antsManifoldParzenWindowsListSampleFunction.hxx`); validated
  (`manifold-parzen-covariance`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: the weighted local covariance, with a notice (A6).
- **D13 / gap:** none.

### ANTS-105 Minimized memory with priors and a mask crashes — (a)
- **What:** `Atropos.cxx:440` checks the priors against the mask through
  `GetPriorProbabilityImage`, which with `--minimize-memory-usage` rebuilds the prior on the
  first intensity image's grid, before any intensity image is set: a segmentation fault.
- **Evidence:** validated (`priors-minimize-memory`: ANTs killed by SIGSEGV).
- **fMRIPrep:** none.
- **larmorx:** runs; `reproduce` stops where ANTs crashes, with a message (A6).
- **D13 / gap:** none.

### ANTS-106 Partial-volume classes with an MRF hardly win a voxel — (c)
- **What:** with `--use-partial-volume-likelihoods 1`, `ComputeMRFDeltaMatrix`
  (`Atropos.hxx:546-606`) gives two labels of one partial-volume set −1, the same label −2 and
  any other pair +1; it compares the *tissue* labels of each set, never the partial-volume
  class's own label, so a partial-volume label is +1 against every neighbour. No voxel starts
  with a partial-volume label, the MRF makes them improbable everywhere, and the first update
  stops: "Updating the class labeling resulted in 3 non-zero label sets but 5 is requested".
  Without the MRF (`-m [0,...]`) partial-volume classes work.
- **Evidence:** read; validated (`partial-volume-with-mrf`: both stop identically;
  `partial-volume-likelihoods` without the MRF: identical with CORE-MATH); measured on a phantom
  of known fractions (the record's `partial-volume-phantom-mrf-*`): with the MRF at 0.1, ANTs
  labels Dice 0.295 and 0.405 of the two partial-volume classes and 94.0 % of the voxels as the
  truth, corrected 0.862, 0.805 and 97.7 %; at 0.3 ANTs stops, corrected 0.835, 0.801, 97.8 %
  (record); sMRIPrep's call with partial-volume classes stops in ANTs on fMRIPrep's T1w of
  ds000005 and ds005040, and runs corrected.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23, the user's decision of 2026-10-22): a
  partial-volume label is a member of its own set, −1 with its two tissues, −2 with itself, +1
  with any other label; a notice wherever partial-volume classes meet the MRF; `reproduce` keeps
  ANTs' energies (and its stop).
- **D13 / gap:** done. No gap.
### ANTS-107 Grubbs-Rosner's grown sample is weighed with uninitialized memory — (a)
- **What:** with Grubbs-Rosner the samples grow by the masked voxels at every update
  (ANTS-94). `UpdateClassLabeling` allocates the weights for the grown sample
  (`WeightArrayType weights(totalSampleSize)`, a `vnl_vector` left uninitialized) and sets those
  of the masked voxels only (`Atropos.hxx:1361-1372`): from the second update on, the weighted
  means and covariances include whatever the heap held. The same command line gives different
  posteriors when only the length of the output paths changes (18,700 of 35,551 posterior
  values between output directories of 139 and 145 characters; with fresh memory the extra
  weights are 0).
- **Evidence:** verified (`grubbs-rosner` and `grubbs-rosner-outliers` at several output
  paths).
- **fMRIPrep:** none.
- **larmorx:** not met by default (ANTS-94 corrected: the sample does not grow); `reproduce`
  weighs the extra samples 0, what a fresh allocation holds, with a notice (A6). The parity
  cases report ANTs' values without requiring them.
- **D13 / gap:** none.

### ANTS-110 antsAI's blob landmarks test displacement lengths, not distances — (c)
- **What:** `GetTransformFromFeatureMatching` keeps a matched blob pair as a landmark when at
  least 3 other pairs have a "distance ratio" within 0.1 of 1. Its comment, and ImageMath's
  `MatchBlobs` (`ImageMath_Templates.hxx:14320-14366`), mean the ratio of the distances between
  two pairs' blobs in the moving and in the fixed image (a rigid motion preserves them), but the
  code divides the squared lengths of the two pairs' displacements:
  `|f_j − m_j|² / |f_i − m_i|²` (`antsAI.cxx:551-552`, assigned symmetrically inside the double
  loop, so `r(i, j)` ends as the smaller over the larger by index). For a pure translation every
  true pair has the same displacement and the test happens to be strict; for any rotation the
  displacements differ and ANTs keeps no pair, or the wrong ones.
- **Evidence:** read; validated (record `ants-ai`, category `blobs`, and
  `real/ds000005-sub-01/blobs-t1w-*`): a phantom rotated by 7° and 4°, ANTs' rigid fit 34 mm
  and its affine fit 653 mm off; a T1w rotated by 8°, no landmark (ANTS-114: NaN); corrected,
  the phantom keeps no landmark (an error) and the T1w (rotated by 8° in millimetres) a rotation
  of 8.5°, 1.0 mm from the truth on average (record, "Blob alignment against the known
  transforms"; evidence page).
- **fMRIPrep:** none (fMRIPrep does not run `antsAI -b`).
- **larmorx:** corrected by default (2026-10-23): the ratio of the distances,
  `|m_i − m_j|² / |f_i − f_j|²`, a notice where ANTs' test keeps other pairs; `reproduce` keeps
  ANTs' test. For a translation the corrected test admits a pair matched to a neighbouring blob
  (the T1w's shifted copy: 11 landmarks instead of 10, 1.2 mm off on average where ANTs is
  exact).
- **D13 / gap:** done. No gap.

### ANTS-111 antsAI pairs the first blobs again once the correlations run out — (c)
- **What:** the pairing loop takes `arg_max` of the correspondence matrix and zeroes the chosen
  row and column; once every entry is 0, `arg_max` returns entry (0, 0), and while the moving
  blob 0 passes the radius and brightness tests the loop pairs fixed blob 0 with moving blob 0
  again, at every step, until it has `m` pairs or has tried every fixed blob
  (`antsAI.cxx:499-525`).
- **Evidence:** read; validated (the phantoms of the record, the noise case: ANTs' pairs end in
  repeated (0, 0)).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23): the pairing stops when no positive
  correlation is left, a notice with the number of pairs ANTs adds; `reproduce` keeps them.
- **D13 / gap:** done. No gap.

### ANTS-112 antsAI's blob detector does not keep the strongest maxima — (c)
- **What:** `MultiScaleLaplacianBlobDetectorImageFilter` keeps per work unit a min-heap of at
  most N blobs and merges the heaps after each scale. (1) After each scale the weakest blob
  kept becomes the threshold every later scale's candidates must exceed
  (`itkMultiScaleLaplacianBlobDetectorImageFilter.hxx:161-163`), even while fewer than N blobs
  are kept, so weaker maxima of later scales are dropped although there is room. (2) While a
  heap fills, its threshold is not updated (`:296-307`); the first candidate after it is full
  is compared with the stale threshold instead of the heap's weakest value, and a weaker
  maximum can push out the heap's weakest one.
- **Evidence:** read; the corrected and ANTs' lists are computed from the same candidates
  (`larmorx_image::blobs`), unit tests on constructed scale stacks; on the record's phantoms and
  real images the largest scales already fill the list and the two agree (no notice).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23): the N strongest maxima, a notice where the
  lists differ; `reproduce` keeps ANTs' list.
- **D13 / gap:** done. No gap.

### ANTS-114 antsAI's rigid blob alignment without landmarks writes NaNs — (c)
- **What:** with no landmark, ITK's rigid landmark fits (`VersorRigid3DTransform`,
  `Rigid2DTransform`) divide the landmarks' sums by their count, 0, and the transform is NaN;
  antsAI starts its search from it and writes a transform of NaNs, exit 0. (The affine fit
  throws instead: "insufficient number of landmarks", which antsAI does not catch: it aborts.)
- **Evidence:** read (`itkLandmarkBasedTransformInitializer.hxx`); validated (the T1w rotated by
  8°, the noise and constant cases).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23): an error ("no blob pair kept its distances to 3
  others: there is no landmark for the rigid fit"), exit 1; `reproduce` writes ANTs' NaNs.
- **D13 / gap:** done. No gap.

### ANTS-115 antsAI dereferences a null transform when a blob list is empty — (a)
- **What:** when either image has no blob, `GetTransformFromFeatureMatching` prints "The moving
  image or fixed image blobs list is empty." and returns a null pointer
  (`antsAI.cxx:471-475`), which the caller dereferences (`:918`, `:928`): a crash.
- **Evidence:** read.
- **fMRIPrep:** none.
- **larmorx:** prints ANTs' message and refuses (exit 1), in both modes (class (a)).
- **D13 / gap:** class (a), not reproduced. No gap.

### ANTS-116 antsAI compares blob patches possibly reflected — (c)
- **What:** `PatchCorrelation` turns the moving patch onto the fixed one with Wahba's
  `A = V·Uᵀ` of the patches' principal axes (`antsAI.cxx:228-262`), without the determinant
  check that antsAI's own principal-axes alignment makes (`:1001-1023`); and the axes come from
  `vnl_symmetric_eigensystem`, whose eigenvectors' signs are arbitrary. Identical patches can
  then be compared reflected or turned by 180°: on fMRIPrep's T1w of ds000005 against its own
  shifted copy, only 4 of 11 identical blob pairs correlate at 1.
- **Evidence:** read; verified (the blob probe and `lx.ants.blob_alignment`); measured
  ([investigation](../../validation/investigations/ants-116-blob-orientation/README.md)): of the
  identical blob pairs whose patches fit in both images, ANTs correlates 4 of 13 at 1 on the T1w
  and its shifted copy (5 of 19, 14 of 28 on the 3D and 2D phantoms), the correction all of them.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (2026-10-23, rule A): every sign combination of the moving
  patch's axes (4 in 3D, 2 in 2D), each with Kabsch's proper rotation, the largest correlation
  kept; ANTs' rotation is one of the candidates when it is proper (its correlation reused bit for
  bit), a reflection never is. Named where the pairs, the landmarks or the transform change;
  `reproduce` keeps ANTs' method. With it the blob alignment finds every translation exactly, the
  3D rotated phantom 0.50 mm off (ANTs 34 mm), the 2D one 0.85 mm off (ANTs 6.2 mm), the rotated
  T1w 1.08 mm off (ANTs: no landmark).
- **D13 / gap:** done. No gap.

### ANTS-160 ResampleImage reads missing Gaussian sigmas past their list — (a)
- **What:** `ResampleImage d in out size 1 2 <sigmas>` copies `ImageDimension` sigmas from
  `ConvertVector(argv[7])` (`ResampleImage.cxx:178-182`) whatever its length: with fewer values
  than dimensions it reads past the end of the vector. With the heap's contents (in practice
  NaN or 0) the Gaussian weights are NaN, vnl prints "vnl_gamma_cont_frac : Failed to converge
  in 100 steps" for each evaluation, and ANTs writes an image of zeros (an integer type) or NaN,
  exit 0.
- **Evidence:** read; verified (random testing, `ants-resample-image:trial1:5`; `2 1` and
  `2 1x1` in 3D; `validation/investigations/difftest/probe_resample_sigma.py`).
- **fMRIPrep:** none (no Gaussian `ResampleImage`).
- **larmorx:** refused in both modes, the message naming ANTS-160 (2026-10-25); the permanent
  case `difftest/gaussian-sigmas-short` is an expected divergence.
- **D13 / gap:** none (class (a): never reproduced; its ANTs runs are never cached).

### ANTS-161 ResampleImage's Gaussian interpolation with a zero or negative sigma — (c)
- **What:** a sigma or alpha of 0 or less (`… 2 0x0x0`, `… 2 1x1x1 0`) makes ITK's
  `GaussianInterpolateImageFunction` divide by zero: infinite scaling, NaN error-function
  weights (vnl prints its convergence messages for each), an image of NaN (zeros once cast to an
  integer type), exit 0. A negative sigma also becomes the pixel type through ANTS-30 (`-1`:
  "Unsupported pixel type").
- **Evidence:** verified (`probe_resample_sigma.py`).
- **fMRIPrep:** none.
- **larmorx:** refused in both modes ("Gaussian sigma and alpha must be positive"), as the Python
  API always did. Decided by rule B (the input makes no sense, ANTs silently writes NaN) on
  2026-10-25.
- **D13 / gap:** the switch: `reproduce` should give ANTs' NaN image (and vnl's messages).

### ANTS-162 Lattices, paddings and grids sized from the physical extent, unchecked — (c)
- **What:** with `-b [distance, order]`, N4BiasFieldCorrection (`N4BiasFieldCorrection.cxx:
  253-280`) gives each axis `ceil((n - 1) spacing / distance)` spans and pads the image by
  `(spans distance - domain) / spacing + 0.5` voxels so the domain is a whole number of spans;
  ITK's N4 filter fits a lattice of `spans 2^level + order` control points per axis at each
  fitting level (one per `-c` entry, 4 by default). ResampleImage and ResampleImageBySpacing
  make an output grid of `extent / spacing` voxels per axis. Nothing bounds any of them. A
  distance far below the extent (a header in metres: the extent 1000 times what it looks)
  gives billions of control points; a distance far above an axis's extent pads that axis by up
  to `distance / spacing` voxels (a 4D image's time axis in seconds, a header in micrometres);
  millimetre output voxels for an image in metres give billions of voxels. The allocation
  succeeds (Linux overcommits) and the program fills the machine while it writes it, until the
  kernel stops it.
- **Evidence:** measured (differential random testing, 2026-10-25). `ants-n4:20261025:24`
  (`-d 4 -s 3 -b [20,3]`, a 14×28×23×7 image of 4×1×1 m voxels, TR 3 s: 52×27×22 m and 18 s)
  grew to 28 GB resident in four minutes on the first night and took the machine down; random
  testing now holds every case in a memory box. `validation/investigations/difftest/
  n4_memory.py` on smaller variants of its input (each run in a cgroup leaf of its own, peak from
  `memory.peak`): ANTs needs 17 bytes per control point of the largest level and 25 per padded
  voxel, larmorx 12 and 21; 3.3·10⁻⁷ s (ANTs) and 2.6·10⁻⁷ s (larmorx) per control point and
  iteration; the replica bit-identical to ANTs in every variant both finished
  (`docs/findings/ants-n4.md`, "Memory"). The case itself needs 246 GiB at its first level and
  344 TB at its fourth (larmorx 174 GiB and 243 TB), about 72 hours of computation at the first
  level: beyond any machine, like `:56` (2.6 TB) and `:108` (5 TB). ResampleImageBySpacing
  (`scaled_memory.py`): 7.3 bytes per output voxel for ANTs, 12.5 to 16 for larmorx before the
  memory work of the same day, which brought every resampler call measured to ANTs' peak plus
  the interpreter's few MiB ([ants-image-programs.md](ants-image-programs.md), "Memory against
  ANTs").
- **fMRIPrep:** none.
- **larmorx:** the method is kept (rule C: the memory is the work asked for; nothing in the
  result is wrong): the same lattice, padding and grid, the replica bit-identical. By default
  (D13: a crash is never reproduced by default, and exhausting the machine is one) a guard
  computes the bytes larmorx will need before it allocates (`larmorx_core::alloc::check_plan`:
  the largest lattice and the padded image for N4, the output grid for the resamplers) and,
  when they exceed the machine's physical memory or `LARMORX_MEMORY_LIMIT`, stops at once with
  the GiB needed and the likely cause ("the image's voxels are 4000 x 1000 x 1000 x 3 mm, the
  fourth axis in s: it spans 52000 x 27000 x 22000 x 18 against a spline distance of 20 (a
  header whose units are metres or micrometres makes an image 1000 times larger or smaller than
  it looks)"), naming ANTS-162. Every case that fits is unchanged, bit for bit. `reproduce`
  attempts it as ANTs does. Permanent cases `difftest/lattice-beyond-any-machine-4d-metres`,
  `-3d-metres` and `-order-5` (n4:24, :56, :108) expect the guard's error; neither ANTs nor
  larmorx's `reproduce` runs on them. Decided 2026-10-25 (orchestrator, by the rules).
- **D13 / gap:** none. Follow-up (D11): a sparse lattice with the padding kept virtual (a control
  point without data in its support stays 0, and so do its refinements) would give the same
  result in memory proportional to the data.

### ANTS-163 A mask of another size is read at the image's indices — (a)
- **What:** ImageMath's `Normalize` with a mask and `TruncateImageIntensity` with a mask read the
  mask with `GetPixel` at each image voxel's index; a mask smaller than the image (a 3D mask for
  a 4D image of more than one volume: `ImageMath 4 out Normalize a.nii mask.nii`) is read
  outside its buffer.
- **Evidence:** read; verified (random testing, `ants-image-math:20261025:14`: ANTs exit 0 on a
  1×1×1×2 image with a 1×1×1 mask).
- **fMRIPrep:** none.
- **larmorx:** refused in both modes (as before), the message now naming ANTS-163 (2026-10-25).
- **D13 / gap:** none (class (a)).

### ITK-18 The joint-histogram MI swaps its marginals — (c)
- **What:** `JointHistogramMutualInformationImageToImageMetricv4::InitializeForIteration` sums
  the joint density along its first index (the fixed image's bin) into what it calls the
  fixed image's marginal, and along the second into the moving image's: each image gets the
  other's marginal. `ComputeValue` (`p(f,m) log p(f,m) / (p_f · p_m)`) and the derivative
  (`p_m` at the moving value) use them under their names, so the value is not the mutual
  information and the derivative not its gradient (unless the two marginals agree).
- **Evidence:** read: `itkJointHistogramMutualInformationImageToImageMetricv4.hxx:263-295`;
  validated (`metrics/mi2*`: reproduced bit for bit; corrected, a 5-iteration affine changes
  its parameters by up to 0.06).
- **fMRIPrep:** none (fMRIPrep and its libraries use `Mattes` and `CC`).
- **larmorx:** corrected by default (A4 part 2): each image gets its own marginal, with a
  notice; `reproduce` swaps them as ITK does.
- **D13 / gap:** done. No gap.

### ITK-19 A start point that throws keeps the previous value — (c)
- **What:** `MultiStartOptimizerv4::ResumeOptimization` catches an exception in a start point
  (a warning), skips `m_CurrentMetricValue = GetValue()`, and still compares the stale value
  with the minimum: the start point that threw gets the previous start point's value (0 before
  the first), so a first start point that throws stays the best unless another value is below
  0, and the observer's `metricValues[bestIndex]` can point at another start point's value or
  past the end of the list.
- **Evidence:** read: `itkMultiStartOptimizerv4.hxx:160-225`; verified (fMRIPrep's BOLD
  `init_aff`, ds000005 run 1: start points 66 and 213 throw, reproduced bit for bit);
  validated (`real/ds000005-sub-01/bold-mixedgamblestask-run-01`).
- **fMRIPrep:** possible: in the run above the throwing start points were not the best.
- **larmorx:** corrected by default (A4 part 3): start points that throw take no part, a notice
  when ITK would have kept one as the best; `reproduce` as ITK.
- **D13 / gap:** done. No gap.

### ITK-20 The Mattes metric is unusable after a refused histogram spacing — (a)
- **What:** the Mattes threader allocates its joint histogram as an image whose spacing is the
  two bin sizes; when the analysis region has one intensity or none (an empty mask: the range
  stays `[FLT_MAX, −FLT_MAX]`), `ImageBase::SetSpacing` throws (ANTs builds ITK with
  `ITK_LEGACY_REMOVE`, so negative spacings throw too) after the image was created without a
  buffer, and the next evaluation of the metric crashes in `FillBuffer`.
- **Evidence:** read: `itkMattesMutualInformationImageToImageMetricv4GetValueAndDerivativeThreader.hxx:96-124`,
  `itkImageBase.hxx:75-92`; verified (an instrumented `antsAI`: the second start point's
  `FillBuffer` on a null buffer, exit −11).
- **fMRIPrep:** none.
- **larmorx:** the first evaluation throws ITK's exception; where ITK would crash next, larmorx
  refuses (A4 part 3).
- **D13 / gap:** class (a), not reproduced. No gap.

### ITK-21 The learning rate carries over between start points — (c; c? until 2026-10-17)
- **What:** `antsAffineInitializer` gives its local optimizer scales with `SetScales` and no
  scales estimator, so `GradientDescentOptimizerv4` never re-estimates the learning rate: the
  rate the golden-section line search leaves at the end of one start point is where the next
  one starts. Each start point's result depends on the one before it.
- **Evidence:** read: `antsAffineInitializer.cxx:405-470`,
  `itkGradientDescentOptimizerv4.hxx` (`EstimateLearningRate` returns without an estimator);
  validated (every `search/*` case of `ants-affine-initializer` reproduces it bit for bit).
- **fMRIPrep:** every niworkflows initialization without an initial transform.
- **larmorx:** corrected by default (2026-10-17, the user's decision of that day: a multi-start
  search compares its start points, so each must be evaluated alike and independently of their
  order): every start point's local search begins at the configured learning rate, 0.1
  (`multistart::Corrections::restart_learning_rate`). The search also runs the other way, to
  name ITK-21 exactly where the correction changes the result, saying whether another start
  point is chosen or the same one ends elsewhere; this doubles the search's time (only with a
  local search and more than one start point). `reproduce` carries the rate over, as ITK
  does, bit for bit. On niworkflows' arguments with the MNI template and fMRIPrep's T1w, the
  same start point (14 of 16) is chosen either way and its parameters move by up to about 1.9.
- **D13 / gap:** done (2026-10-17): corrected by default, `reproduce` gives ITK's carry-over.
  No gap.

### ITK-22 A label the moving set lacks stops the stage — (c)
- **What:** `LabeledPointSetToPointSetMetricv4` runs one metric per label common to both point
  sets. Label sets that differ give the warning "The label sets are not bijective" at every
  level's `Initialize`; then the first fixed point whose label the moving set lacks throws
  "Label not found in common label set" in `GetLocalNeighborhoodValue` or its derivative, and
  the stage is dropped (ANTS-49): one stray label ends the registration.
- **Evidence:** read: `itkLabeledPointSetToPointSetMetricv4.hxx:84-117, 225-236`; validated
  (`point-sets/icp-not-bijective`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 4): fixed points of labels the moving set lacks take
  no part, with a notice (an error when no label is common, where nothing can be matched).
  `reproduce` throws ITK's exception, as ANTs. In SyN stages (A5 part 1) the moving-to-middle
  update swaps the sets, so a label only the moving set has throws too (in the second
  evaluation; `syn-point-sets/icp-moving-extra-label`); the correction applies both ways. A SyN
  stage's exception is not dropped (ANTs exits 1).
- **D13 / gap:** done. No gap.

### ITK-23 An identity covariance keeps the univariate prefactor — (c)
- **What:** a new `GaussianMembershipFunction` has the identity covariance and the univariate
  prefactor `1/√(2π)`; `SetCovariance` returns early when the covariance it is given equals the
  one it has, so a Gaussian of identity covariance keeps `1/√(2π)` instead of `(2π)^(−D/2)`.
  JHCT sets the identity times `pointSetSigma²` for isotropic covariances: with a sigma of 1,
  every Gaussian of the density has the wrong normalisation.
- **Evidence:** read: `Modules/Numerics/Statistics/include/itkGaussianMembershipFunction.hxx:32,
  103-107, 133`; validated (`point-sets/jhct-isotropic`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 4): the covariance is set like any other, with a
  notice (the synthetic case's transform moves by 1e-14). `reproduce` as ITK.
- **D13 / gap:** done. No gap.

### ITK-24 JHCT's number of points leaves out the fixed ones — (c?)
- **What:** `Initialize` computes `m_TotalNumberOfPoints = m_NumberOfValidPoints + moving points`
  for its prefactors, but `m_NumberOfValidPoints` is only counted later, in
  `InitializeForIteration`: at `Initialize` it is 0, so the total is the moving points alone.
- **Evidence:** read: `itkJensenHavrdaCharvatTsallisPointSetToPointSetMetricv4.hxx:41-63`,
  `itkPointSetToPointSetMetricWithIndexv4.hxx` (`InitializeForIteration`); validated (every JHCT
  case reproduces it).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A5 part 3): the clone's valid fixed points (all of its
  label's: a clone has no virtual domain) plus its moving points; a notice. `reproduce` as ITK.
- **Measured:** the total scales the value and the derivative alike, which the line search and
  the learning rate cancel: parameters within 1.1e-5 of ITK's (isotropic, sigma 2).
- **D13 / gap:** decided 2026-10-21: a defect, corrected by default with a switch. Done. No gap.

### ITK-25 The Parzen covariances read past their neighbours — (a)
- **What:** `ManifoldParzenWindowsPointSetFunction::SetInputPointSet` asks the points locator
  for `m_CovarianceKNeighborhood` (5) neighbours and loops over 5 entries of the returned list,
  which is shorter when the point set (one label's points) has fewer: a read past its end.
- **Evidence:** read: `itkManifoldParzenWindowsPointSetFunction.hxx:86-117`.
- **fMRIPrep:** none.
- **larmorx:** uses the neighbours there are, in both modes, with a notice (the read's values
  are undefined in ITK).
- **D13 / gap:** class (a), not reproduced. No gap.

### ITK-26 The Parzen covariances count the diagonal twice — (c)
- **What:** the anisotropic covariance loop `for (m) for (n = m; …) { Cout(m, n) += c;
  Cout(n, m) += c; }` adds each diagonal product twice: the diagonal is doubled against the
  off-diagonal terms, a covariance that is not the weighted covariance of the neighbours.
- **Evidence:** read: `itkManifoldParzenWindowsPointSetFunction.hxx:106-114`; validated (every
  anisotropic JHCT case).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 4): once, with a notice (on the synthetic cases the
  transform moves by 0.027 to 0.047). `reproduce` as ITK.
- **D13 / gap:** done. No gap.

### ITK-27 Adding compensated sums adds the compensation first — (d)
- **What:** `CompensatedSummation::operator+=(const Self &)` adds the other sum's compensation,
  then its sum, as two elements; the per-thread sums of the point-set metrics are combined so.
  The result differs from the exact sum by rounding, and with the thread count.
- **Evidence:** read: `Modules/Core/Common/include/itkCompensatedSummation.hxx:104-109`,
  `itkPointSetToPointSetMetricWithIndexv4.hxx:179-215`.
- **fMRIPrep:** none.
- **larmorx:** reproduces it for its one work unit (ANTs' one-thread result).
- **D13 / gap:** not D13 (D11).

### ITK-28 JHCT tests alpha = 1 two ways — (c)
- **What:** `Initialize` divides the prefactor by `alpha − 1` when `m_Alpha != 1.0`;
  `ComputeValueAndDerivative` takes the logarithm (the Shannon case) when
  `Math::AlmostEquals(m_Alpha, 1)`, within 4 ulps. An alpha within 4 ulps of 1, not 1, gets the
  logarithm and a prefactor divided by almost 0.
- **Evidence:** read: `itkJensenHavrdaCharvatTsallisPointSetToPointSetMetricv4.hxx:59-62, 121`;
  validated (`point-sets/jhct-alpha-1`: exactly 1, both tests agree).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A4 part 4): both test `AlmostEquals`, with a notice.
  `reproduce` as ITK.
- **D13 / gap:** done. No gap.

### ITK-29 The Parzen kernel sigma is used as a variance — (c?)
- **What:** each point's initial Gaussian has the covariance `identity × m_KernelSigma` (ANTs
  sets 10): the sigma unsquared, where the regularisation sigma is squared
  (`Math::sqr(m_RegularizationSigma)`).
- **Evidence:** read: `itkManifoldParzenWindowsPointSetFunction.hxx:56-60, 129, 138`,
  `itkantsRegistrationHelper.hxx:1058`.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A5 part 3): the kernel covariance `σ²·I` (100·I for ANTs'
  10); a notice where the local covariances are used (anisotropic). `reproduce` as ITK.
- **Measured:** with ITK-24 and ITK-26 corrected too, the translation error on the phantoms
  goes from 0.027 to 0.010 mm (sigma 1, α 1.1) and from 0.104 to 0.058 mm (sigma 2, α 1.5).
- **D13 / gap:** decided 2026-10-21: a defect, corrected by default with a switch. Done. No gap.


### ITK-30 GC with a displacement field: the whole derivative in one voxel — (c)
- **What:** `CorrelationImageToImageMetricv4` is meant to refuse transforms of local support
  (its constructor checks, before any transform is set, so the check never fires). With SyN's
  displacement fields its per-thread sums are sized for every parameter but indexed by the
  local parameter, so every point's `f·g` and `m·g` terms go to the first `D` entries and the
  whole vector is then scaled: only the first voxel's update is not 0, and the stage barely
  moves.
- **Evidence:** read: `itkCorrelationImageToImageMetricv4HelperThreader.hxx`,
  `itkCorrelationImageToImageMetricv4.hxx` (constructor); validated (`syn/gc`: identical to
  ANTs with the switch).
- **fMRIPrep:** none (fMRIPrep's SyN stages use CC).
- **larmorx:** corrected by default (A5 part 1): each voxel gets its own point's terms;
  `reproduce` as ITK, with a notice.
- **D13 / gap:** done. No gap.

### ITK-31 SyN keeps a stale mask for the role without one — (c)
- **What:** `SyNImageRegistrationMethod::ComputeMetricGradientField` resamples and sets the
  fixed and moving masks only for the roles that have one (`if (fixedImageMasks[n])`). With a
  fixed mask and no moving mask (or the reverse), the metric object keeps the mask set by the
  previous call; since SyN swaps the roles, the second call of each iteration, and every call
  from the second iteration on, sees a resampled mask where the stage has none.
- **Evidence:** read: `itkSyNImageRegistrationMethod.hxx:368-426, 502-557`; validated
  (`syn/fixed-mask-only`, `syn/moving-mask-only` on a mask of another grid: identical to ANTs
  with the switch).
- **fMRIPrep:** every run: sMRIPrep's brain extraction (`antsBrainExtraction`) gives its SyN
  stage a fixed mask only (`--masks [template mask, NULL]`), so from the second evaluation the
  metric's moving role keeps the fixed mask resampled for the previous evaluation. Corrected,
  ds000005's brain extraction (full size) keeps its affine transform and its field moves in 79 %
  of the voxels by a median of 0.0004 mm, at most 0.18 mm (0.02 % of the voxels by more than
  0.1 mm), well inside ANTs' own thread band; the full record's fix pass names ITK-31 in all ten
  brain extractions (small and full size).
- **larmorx:** corrected by default (A5 part 1): a role without a mask has none; `reproduce` as
  ITK, with a notice.
- **D13 / gap:** done. No gap.

### ITK-32 The labelled point-set metric keeps its first per-label metrics — (c)
- **What:** `LabeledPointSetToPointSetMetricv4::Initialize` clones one metric per common label,
  initialises it (in tangent space, its moving points mapped into the virtual domain) and
  appends it to `m_PointSetMetricClones`, which is never cleared; the evaluations look a label
  up by its position in the common labels and use the first clones. SyN initialises the metric
  twice per iteration, so every evaluation compares with the moving points as the stage's first
  evaluation mapped them, and the moving-to-middle update compares the moving points with
  themselves. (In linear stages the clones' moving points are not mapped, so it does not
  matter.)
- **Evidence:** read: `itkLabeledPointSetToPointSetMetricv4.hxx:44-124`; validated
  (`syn-point-sets/icp`, `pse`, `jhct` and the others: identical to ANTs with the switch).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A5 part 1): each evaluation uses its own initialisation's
  metrics; `reproduce` as ITK, with a notice.
- **D13 / gap:** done. No gap.

### ITK-33 Point-set metrics among several metrics get identity transforms in SyN — (c)
- **What:** in `ComputeMetricGradientField`, a point-set metric of a multi-metric is given the
  fixed and moving composites (`multiMetric->SetFixedTransform`, which sets every metric's),
  then, because the multi-metric is not itself a point-set metric, every metric of it gets the
  identity displacement field transforms meant for the pre-resampled images. A point-set metric
  among several is thus evaluated where its points were read: it never sees the initial
  transforms or the deformation (the single-metric path excludes point sets from the identity).
- **Evidence:** read: `itkSyNImageRegistrationMethod.hxx:316-328, 566-590`; validated
  (`syn-point-sets/cc-and-icp`, `icp-and-cc`: identical to ANTs with the switch).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A5 part 1): the point-set metrics are evaluated through the
  composites, as a single point-set metric is; `reproduce` as ITK, with a notice.
- **D13 / gap:** done. No gap.

### ITK-34 Point-set derivatives added into the field without synchronisation — (b)
- **What:** `CalculateValueAndDerivative` splits the fixed points into one range per work unit
  and, for transforms of local support, adds each point's derivative into the shared derivative
  array (`StorePointDerivative`, `field[offset + i] += ...`) from all the threads at once. Two
  points of different ranges at the same virtual voxel race: SyN with ICP gave three different
  fields in three runs at 4 threads (one of them the one-thread field) and two in two runs at 8
  (`syn-point-sets/icp`; the records' thread band, 0.023 to 0.025 mm).
- **Evidence:** read: `itkPointSetToPointSetMetricWithIndexv4.hxx:275-377, 437-466`; verified
  (2026-10-19: seven runs of the same command at 1, 4 and 8 threads).
- **fMRIPrep:** none (no point-set metric).
- **larmorx:** the points in order, as one thread adds them, at any thread count.
- **D13 / gap:** class (b), not reproduced. No gap.

### ITK-01 `Math::Floor` of tiny negative values — (c)
- **What:** on x86-64 `itk::Math::Floor` (and `vnl_math::floor`) is
  `round_half_to_even(2x − 0.5) >> 1`; for x in [−2⁻²⁶, 0) in float ([−2⁻⁵⁵, 0) in double) it
  gives 0, not −1, so 4D linear interpolation (`EvaluateUnoptimized`) mixes in the next voxel
  with a tiny negative weight.
- **Evidence:** read: `itkMathDetail.h:414-422`; verified (the last 7 voxels of a 4D case).
- **fMRIPrep:** none seen.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the exact floor (`larmorx_core::IndexFloor`, in the 4D linear
  paths of `larmorx_ants::resample`, antsApplyTransforms' displacement-field and windowed-sinc
  lookups, and DenoiseImage's noise resampling). The two floors differ in the last bits of a
  value at most, and mostly not at all once it is stored, so each output voxel that met a
  differing floor is computed again with the other one, and the warning counts the voxels
  whose stored value changed. `reproduce` takes ITK's floor
  (`larmorx_image::grid::{vnl_floor_f32, itk_floor_f64}`, `ItkReal::itk_floor`).
- **D13 / gap:** done, except the notice in antsRegistration's displacement-field lookups
  (`larmorx_registration::field::FieldXfm`, each lookup with a floor of its own whose counts are
  dropped): random testing found a field value one float ulp from ANTs' by default, with no
  notice (`ants-registration:trial2:35`; `reproduce` exact).

### ITK-02 Extra dimensions are read with an identity direction — (c)
- **What:** a file with more dimensions than the program's `D` is read as its first
  `D`-dimensional block with the ImageIO's default (identity) direction; origin and spacing are
  kept. `ImageMath 3 ... m bold.nii.gz 2` writes the first volume axis-aligned, even if the BOLD
  is oblique.
- **Evidence:** read: `itkImageFileReader.hxx` (`GenerateOutputInformation`); validated
  (`arithmetic/dim3-of-4d`, N4 `inputs/dim3-of-4d`).
- **fMRIPrep:** none seen.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the top-left `D × D` block of the file's direction, where the
  first `D` axes do not reach into the others (every NIfTI time series read in 3D); where they
  do (an oblique 3D file read in 2D) no `D`-dimensional direction describes the block, and
  ITK's identity is kept in both modes (`larmorx_ants::image::from_itk`).
- **D13 / gap:** done. No gap.

### ITK-03 Float labels stop at 2^24 — (c), latent
- **What:** `RelabelComponentImageFilter` counts the new labels in the output pixel type; with
  a `float` output, as in ANTs, `label + 1` stops growing at 2^24.
- **Evidence:** read: `itkRelabelComponentImageFilter.hxx`.
- **fMRIPrep:** latent (16.7 million components).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): labels counted in integers
  (`larmorx_ants::image_math::components::cluster_sizes`, `GetLargestComponent`).
- **D13 / gap:** done. No gap.

### ITK-04 Danielsson's distance map is approximate — (d)
- **What:** offsets propagate through axis neighbours only, so distances are not exactly
  Euclidean, and they depend on the visiting order.
- **Evidence:** read: `itkDanielssonDistanceMapImageFilter.hxx:202-402`; validated.
- **fMRIPrep:** none (FillHoles uses it only as an exact background test).
- **larmorx:** replays ITK's visits (bit-identical).
- **D13 / gap:** not D13; an exact map (Maurer) is a D11 option.

### ITK-05 `vnl_erf` in the Gaussian interpolators — (d)
- **What:** the Gaussian and MultiLabel interpolators use vnl's `erf`, with a relative error of
  3e-7, not the C library's.
- **Evidence:** read ([itk-resampling.md](itk-resampling.md)).
- **fMRIPrep:** antsApplyTransforms with Gaussian or MultiLabel interpolation.
- **larmorx:** reproduces.
- **D13 / gap:** not D13; a D11 option.

### ITK-06 Non-finite values read as 0 — (c?)
- **What:** nifti_clib replaces every NaN and ±inf of float data by 0 as it reads, so no ANTs
  program sees a non-finite value (`ImageMath abs` turns `-inf` into 0).
- **Evidence:** read: `nifti1_io.c:5013-5049`; verified (`head-outliers`); measured
  ([investigation](../../validation/investigations/itk-06-nonfinite/README.md)): the five
  oracle runs pass 509 images to ANTs programs, 420 of them float, none with a NaN or an
  infinity; of the test-data catalog's 149 NIfTI files one has NaN (nibabel's
  `resampled_anat_moved.nii`, outside its field of view). Kept as values, one NaN slab makes
  every value of a smoothed or `ResampleImageBySpacing`d image NaN, and spreads through
  antsApplyTransforms, TruncateImageIntensity, N4 and DenoiseImage (8,906 to 31,741 NaN values;
  DenoiseImage also moves 1.2 million others by up to 16 %).
- **fMRIPrep:** none (no non-finite input in the oracle runs).
- **larmorx:** nifti_clib's behaviour in both modes, documented on the API pages (an image that
  marks missing data with NaN is read with 0 there).
- **D13 / gap:** decided 2026-10-23 (decision log): measured (rule D), then rule C: kept and
  documented, nifti_clib's deliberate design ("fix bad floats"), what every ITK and ANTs user
  gets, with nothing to gain for the pipelines and a NaN that would spread through every
  smoothing step. No gap.

### ITK-07 Integer data scaled in float32 — (d)
- **What:** ITK casts integer values to float and scales in double, then rounds to float; a
  scaled int16 BOLD differs from nibabel's float64 scaling by float32 rounding.
- **Evidence:** read: `itkNiftiImageIO.cxx:600-640,745-795,1275-1310`.
- **fMRIPrep:** scaled inputs to ANTs programs.
- **larmorx:** reproduces in the ANTs programs.
- **D13 / gap:** not D13.

### ITK-08 nifti_clib opens a different file — (e)
- **What:** asked for `x.nii.gz`, a reader built on nifti_clib reads the header from
  `x.nii.gz` and the voxels from `x.nii` when both exist (`nifti_findimgname` tries `.nii`
  before `.nii.gz`).
- **Evidence:** verified with SimpleITK (it corrupted an early benchmark) and with the ANTs
  2.6.5 oracle (ImageMath on `x.nii.gz` with a different `x.nii` beside it, 2026-10-12).
- **fMRIPrep:** none.
- **larmorx:** opens exactly the path given, by default; `reproduce` (Decisions, item 5) reads
  the voxels from the file nifti_clib picks (`larmorx_io::nifti::NiftiPaths::nifti_clib_image_path`,
  the ANTs programs' readers). Where such a file is beside the named one, both are read, and
  the warning comes when their voxels differ (2026-10-12).
- **D13 / gap:** done. No gap.

### ITK-10 A versor axis of norm ≥ 1 can fail to load in float — (c)
- **What:** `VersorRigid3DTransform<float>::SetParameters` (and `Similarity3DTransform<float>`)
  scales an axis whose norm reaches `1 − 1e-10` down to norm `1/(1 + 1e-10)`, in double, and
  stores it as float; rounded to float, the axis can again be longer than 1, and
  `Versor<float>::Set` throws ("Trying to initialize a Versor with a vector whose magnitude is
  greater than 1"). The same file reads fine in double. So `antsApplyTransforms --float 1`
  cannot use some valid versor transforms (axis norm ≥ 1 means a rotation by 180°: such
  parameters are the axis scaled by its norm).
- **Evidence:** read: `itkVersorRigid3DTransform.hxx` (`SetParameters`), `itkVersor.hxx`
  (`Set(const VectorType &)`); verified: parameters `-0.7041559 0.63925344 0.3665738 1 2 3`,
  ANTs 2.6.5 exits 0 with `--float 0` and 1 with `--float 1` ([ants-float.md](ants-float.md)).
- **fMRIPrep:** none seen (fMRIPrep's float transforms are affines).
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12): the versor is computed in double and stored as float, so the
  transform loads (`larmorx_transform::linear::versor`, `TransformChain::prepare`);
  `reproduce` refuses it as ITK does ("versor with magnitude greater than 1 in single
  precision").
- **D13 / gap:** done. No gap.

### ITK-11 `GaussianOperator`'s Bessel functions lose accuracy for large variances — (d)
- **What:** the discrete Gaussian kernel's coefficients `e^{−t} I_k(t)` use Numerical Recipes'
  `bessi`, whose Miller recurrence starts at order `2(k + √(40k))` whatever `t` is; for `t` well
  above that start the ratios `I_k / I_0` are wrong. Against `scipy.special.ive`, the largest
  relative error of a coefficient (k ≤ 32) is 1e-7 up to a variance of 25 voxels², 1.4e-5 at
  50, 1.5e-3 at 100, 5 % at 200 and 52 % at 700, always at k ≈ 3 (the coefficients are then
  normalised, so the kernel keeps its sum but not its shape).
- **Evidence:** read: `itkGaussianOperator.hxx` (`ModifiedBesselI`); measured (the recurrence
  replayed in Python, 2026-10-12).
- **fMRIPrep:** none (`G` is not used).
- **larmorx:** reproduces ITK's kernel bit for bit wherever it is finite; the kernel that
  corrects ANTS-14 (variances above about 709) starts the recurrence above `k + t`.
- **D13 / gap:** not D13 (an imprecision); an accurate kernel is a D11 option.

### ITK-09 ITK's HDF5 writer is not byte-reproducible — (b)
- **What:** ITK stores object modification times, so writing the same transform twice gives
  different bytes; only the datasets are reproducible.
- **Evidence:** verified.
- **fMRIPrep:** none (values unaffected).
- **larmorx:** `lx.transforms.write` gives the same bytes (verified 2026-10-11: two writes
  1.5 s apart are identical). Since 2026-10-16 the writer is larmorx's own (crate
  `larmorx-hdf5`): every object header records `Hdf5WriteOptions.time`, 0 by default; the
  validation passes ITK's time to compare bytes ([itk-hdf5.md](itk-hdf5.md)).
- **D13 / gap:** not reproduced. No gap.

### ITK-12 A fixed-length `TransformType` string cannot be read — (c)
- **What:** `HDF5TransformIOTemplate::Read` reads `TransformType` with a variable-length
  `C_S1` memory type; HDF5 has no conversion from a fixed-length string, so a file whose type
  is stored as one (as h5py writes a `numpy.bytes_` array) fails: "H5Dread failed for variable
  length string".
- **Evidence:** read: `itkHDF5TransformIO.cxx:307-316`; verified (h5py-written file, ANTs
  2.6.5, 2026-10-16; validation `itk-hdf5`, `errors/h5py/fixed-string`).
- **fMRIPrep:** none (ITK writes variable-length strings).
- **larmorx:** reads the string by default, with a warning; `reproduce` fails with ITK's
  messages (`larmorx_io::itk_hdf5`).
- **D13 / gap:** done. No gap.

### ITK-13 Wrong messages for a parameter dataset of the wrong class or rank — (f)
- **What:** `ReadParameters` and `ReadFixedParameters` report a non-float dataset as "Wrong
  data type for <path>in HDF5 File" (no blank) and a dataset of rank other than 1 as "Wrong #
  of dims for TransformType in HDF5 File", whatever the dataset.
- **Evidence:** read: `itkHDF5TransformIO.cxx:171, 176, 218, 223`; verified.
- **fMRIPrep:** none.
- **larmorx:** names the dataset, with a blank; `reproduce` prints ITK's text.
- **D13 / gap:** messages only. No gap.

### ITK-14 Parameters wider than 8 bytes are read through float — (c)
- **What:** `ReadParameters` reads a dataset whose type is 8 bytes as `NATIVE_DOUBLE` and every
  other floating-point type as `NATIVE_FLOAT`: `long double` (x87, 16 bytes) and IEEE
  quadruple parameters lose their precision, even in the double reader.
- **Evidence:** read: `itkHDF5TransformIO.cxx:186-204`; verified (h5py-written `long double`
  parameters, ANTs 2.6.5).
- **fMRIPrep:** none (ITK writes float or double).
- **larmorx:** reads them in the reader's precision by default, warning where the values
  differ; `reproduce` goes through float.
- **D13 / gap:** done. No gap.

### ITK-15 A transform without parameters cannot be written with compression — (c)
- **What:** `WriteParameters` with compression sets a chunk of `min(n, 1 Mi)` values; for a
  transform without parameters (`IdentityTransform`) that is 0, `H5Pset_chunk` fails ("all
  chunk dimensions must be positive"), and `Write` throws "H5Pset_chunk failed". ANTs always
  asks for compression, so ANTs' writer (`itk::ants::WriteTransform`, as in
  `CompositeTransformUtil --assemble`) cannot store an `IdentityTransform` in `.h5`; the file
  is left with what was written before, the transform's empty fixed parameters included, and
  ITK cannot read it back (its `TransformParameters` is missing).
- **Evidence:** read: `itkHDF5TransformIO.cxx:127-138, 462`; verified
  (`CompositeTransformUtil --assemble out.h5 affine.mat identity.txt`, ANTs 2.6.5,
  2026-10-16; validation `itk-hdf5`, `write/itk15-identity-compressed`).
- **fMRIPrep:** none (its transforms have parameters; `antsApplyTransforms -t identity` is a
  12-parameter `MatrixOffsetTransformBase`).
- **larmorx:** stores the empty parameters uncompressed (as ITK does without compression),
  with a warning; `reproduce` fails as ITK does and writes the file ITK leaves, byte for byte.
- **D13 / gap:** done. No gap.

### ITK-16 A null `TransformType` string — (a)
- **What:** a variable-length `TransformType` that is HDF5's null string reads as a null
  `char *`, from which ITK's C++ binding builds a `std::string`: undefined behaviour.
- **Evidence:** read: ITK's HDF5 1.12.1 C++ binding, `c++/src/H5DataSet.cpp:747-763`
  (`p_read_variable_len`: `strg = strg_C`).
- **fMRIPrep:** none.
- **larmorx:** refuses the file in both modes.
- **D13 / gap:** class (a), not reproduced. No gap.

### ITK-17 A composite component of another dimension — (a)
- **What:** `CompositeTransformIOHelperTemplate::InternalSetTransformList` adds each
  component with a `static_cast` to the composite's dimension, so a 2-D transform in a 3-D
  composite becomes a 3-D transform pointer to a 2-D object; antsApplyTransforms runs on and
  exits 0.
- **Evidence:** read: `itkCompositeTransformIOHelper.cxx:197-206`; verified (an h5py-written 3-D
  composite holding `AffineTransform_double_2_2`, ANTs 2.6.5 exits 0, 2026-10-16).
- **fMRIPrep:** none.
- **larmorx:** refuses the file in both modes.
- **D13 / gap:** class (a), not reproduced. No gap.

### HDF-01 HDF5 1.12 converts non-native floats with state carried over — (c)
- **What:** `H5T__conv_f_f` (the soft conversion between floating-point formats that are not
  the platform's, such as half precision, x87 extended or a custom format) keeps variables from
  one element to the next: after a denormalized value every later value of the same conversion
  buffer rounds as a denormal, an unnormalized zero takes the previous value's mantissa width,
  and a denormal that rounds up to the smallest normal loses the carry. ITK reads such
  parameters through it.
- **Evidence:** read: HDF5 1.12.1 `src/H5Tconv.c` (`H5T__conv_f_f`).
- **fMRIPrep:** none (its parameters are native floats).
- **larmorx:** computes each element on its own by default and warns where HDF5's result
  differs; `reproduce` carries the state as HDF5 does (`larmorx_hdf5::datatype::conv_f_f`).
- **D13 / gap:** done. No gap.


### ITK-35 B-spline transforms have no Jacobian with respect to position — (c?)
- **What:** `BSplineBaseTransform::ComputeJacobianWithRespectToPosition` throws ("not yet
  implemented for BSplineTransform"). `CompositeTransform`'s Jacobian with respect to the active
  transform's parameters needs it for every transform applied after the active one, so a stage
  after a `BSpline` stage, or with a B-spline initial transform, fails at its first derivative
  unless its metric works on identity fields (SyN, B-spline SyN): a linear stage is then dropped
  (ANTS-49), a displacement-field or `BSpline` stage ends the run with exit 1. The Jacobian is
  well defined (the derivative of the cubic B-spline sum) and cheap.
- **Evidence:** read: `itkBSplineBaseTransform.h:301`, `itkCompositeTransform.hxx`
  (`ComputeJacobianWithRespectToParametersCachedTemporaries`); validated (`bspline/then-affine`,
  `bspline/then-bspline`, `bspline/initial-bspline-affine`: identical messages).
- **fMRIPrep:** none.
- **larmorx:** corrected by default (A5 part 3): `BSplineXfm::jacobian_position`, the identity
  plus `Σ c_k ⊗ ∇w_k` (the cubic kernel's derivative along one axis times its values along the
  others, through the grid's point-to-index matrix); a notice. `reproduce` fails as ITK.
- **Validated:** against finite differences of the transform (1e-6), of the composite's
  Jacobian (1e-5) and of MeanSquares through it (as accurate as without it); against the closest
  runs ANTs can do (the moving image resampled through the B-spline transform; the transform as
  a displacement field): within 0.3 to 0.4 mm on the phantoms with a mask
  ([ants-velocity.md](ants-velocity.md)). Without a mask a later linear stage can stall where
  points cross the B-spline transform's valid-region border (the transform jumps there).
- **D13 / gap:** decided 2026-10-21: a defect, corrected by default with a switch. Done. No gap.

### ITK-36 The time-varying B-spline method's multi-metric transforms — (c)
- **What:** `TimeVaryingBSplineVelocityFieldImageRegistrationMethod` sets each metric's fixed
  and moving transforms through the multi-metric (`SetFixedTransform` on the whole queue), so
  all metrics are evaluated with the transforms the last metric set: point-set metrics listed
  before an image metric see identity fields (never the deformation); image metrics listed after
  a point-set metric get the flows on top of their already resampled images (warped twice).
- **Evidence:** read: `itkTimeVaryingBSplineVelocityFieldImageRegistrationMethod.hxx:100-200`;
  validated (`time-varying-bspline/cc-and-icp`, `meansquares-icp-cc`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: each metric its own transforms, a notice; `reproduce` as ITK.
- **D13 / gap:** class (c), corrected with a switch. No gap.

### ITK-37 Stale inverse fields at the first and last time samples — (c)
- **What:** at the first time sample the method zeroes the fixed flow's displacement field but
  keeps the previous integration's inverse field (likewise the moving flow at the last), and the
  point-set metrics map their points with those inverses.
- **Evidence:** read: same file; validated (the point-set cases of `time-varying-bspline`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: zeroed too, a notice; `reproduce` as ITK.
- **D13 / gap:** class (c), corrected with a switch. No gap.

### ITK-38 Physical points in a parametric domain — (c)
- **What:** with a single point-set metric, the method fits the points' derivatives (and the
  domain's boundary points) at their physical positions in the B-spline's parametric domain,
  which has the domain's origin and spacing but not its direction: with any direction but the
  identity (NIfTI files stored in RAS order read as LPS: x and y flipped) the points fall outside
  and `BSplineScatteredDataPointSetToImageFilter` throws "The reparameterized point component …
  is outside the corresponding parametric domain".
- **Evidence:** read: same file; verified; validated (`time-varying-bspline/icp-ras`, rejected
  by both, and the `-lpsid` phantoms' cases, identical).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: the points mapped through the domain's continuous index into
  the parametric domain, a notice; `reproduce` as ITK.
- **D13 / gap:** class (c), corrected with a switch. No gap.

### ITK-39 The time-varying method ignores the fixed initial transforms — (c)
- **What:** `TimeVaryingVelocityFieldImageRegistrationMethodv4` resamples the fixed images
  through the flow from `t` back to 0 alone; `antsRegistration -q` (its fixed initial transform)
  is never applied.
- **Evidence:** read: `itkTimeVaryingVelocityFieldImageRegistrationMethodv4.hxx`; validated
  (`time-varying/fixed-initial`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: applied before the flow, a notice; `reproduce` as ITK.
- **D13 / gap:** class (c), corrected with a switch. No gap.

### ITK-40 The time-varying methods' masks are not moved — (c)
- **What:** both time-varying methods resample the images through the flows onto the virtual
  domain and give the metric identity transforms, but give it the stage's masks as they are: the
  masks no longer cover the resampled images.
- **Evidence:** read: both methods; validated (the `masks` cases).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: the masks resampled through the same transforms (nearest
  neighbour), a notice where that changes which voxels they hold; `reproduce` as ITK.
- **D13 / gap:** class (c), corrected with a switch. No gap.

### ITK-41 Velocity-field transforms that never integrated — (a)
- **What:** a stage that neither iterates nor resamples (one level, 0 iterations) never
  integrates its velocity field: a time-varying transform's displacement field has no
  interpolator input and the next point it maps crashes (antsRegistration's warped images); an
  exponential transform keeps its velocity image as its field and has no inverse field, so the
  inverse warped image throws "No displacement field is specified."
- **Evidence:** read: `itkVelocityFieldTransform.hxx`, `itkConstantVelocityFieldTransform.hxx`;
  verified; validated (`*/zero-iterations*`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: integrated (zero fields where nothing was optimised), a
  notice; `reproduce` throws ITK's exception and refuses where ANTs crashes.
- **D13 / gap:** class (a), the crash not reproduced. No gap.

### ITK-42 Scales all below epsilon — (c)
- **What:** `RegistrationParameterScalesFromPhysicalShift` probes a transform of local support
  with a unit change of the central voxel's parameters. Where a B-spline-smoothed update field
  pins every control point to its stationary boundary (a coarse mesh at a shrunken level, e.g.
  `BSplineDisplacementField` 3x3x3 at shrink 2 with ANTS-78 corrected, or a total mesh of 1 in
  ANTs), the probe vanishes and every scale is about 4e-24 to 5e-22 (equal, or equal but for
  rounding on an oblique grid); `ObjectToObjectOptimizerBase`
  refuses scales ≤ ε ("m_Scales values must be > epsilon") and the stage fails, although equal
  scales cancel in the learning rate the optimizer estimates (the scaled gradient `g/s`, the rate
  proportional to `s`).
- **Evidence:** read: `itkObjectToObjectOptimizerBase.cxx:130`,
  `itkRegistrationParameterScalesFromShiftBase.hxx`; validated
  (`displacement-field/bspline-coarse-total`, `corrections/bspline-field-coarse`).
- **fMRIPrep:** none.
- **larmorx:** corrected by default: scales all not above ε replaced by ones, a notice;
  `reproduce` throws as ITK.
- **D13 / gap:** class (c), corrected with a switch. No gap.

### ITK-50 K-means leaves out the k-d tree's split points — (c)
- **What:** `KdTreeBasedKmeansEstimator::Filter` adds a whole node to its closest candidate
  when one candidate remains, and otherwise recurses into the node's two halves
  (`itkKdTreeBasedKmeansEstimator.hxx:198-227`). A non-terminal node of
  `WeightedCentroidKdTreeGenerator`'s tree holds one point itself, the one it is split at (the
  median of its partition dimension); the recursion visits only the halves, so that point is
  left out of the new centres whenever its node is divided between candidates. Each k-means
  pass misses up to one point per divided node.
- **Evidence:** read; validated (every K-means case of the Atropos suite reproduces it with
  `reproduce`, bit for bit with CORE-MATH).
- **fMRIPrep:** every run: sMRIPrep's Atropos call initializes with `KMeans[3]`. Corrected, on
  the five oracle runs 1 to 25 of 0.8 to 2.5 million brain voxels change label (Dice ≥ 0.99999
  for every class) and the posteriors move by at most 0.14; ANTs' own runs with the clock's seed
  differ in 287 labels on ds000005 ([ants-atropos.md](ants-atropos.md)).
- **larmorx:** corrected by default: every point counted, with a notice where the centres
  change (`larmorx_image::kdtree::Kmeans::fix_split_points`); `reproduce` as ITK (A6).
- **D13 / gap:** none.

### ITK-51 Student's distribution is NaN above about 340 degrees of freedom — (c)
- **What:** `TDistribution::PDF` divides `dgamma((n+1)/2)` by `dgamma(n/2)`
  (`itkTDistribution.cxx:84-85`); SLATEC's `dgamma` overflows above 171.6 (it prints "X SO BIG
  GAMMA OVERFLOWS" and returns infinity), so for more than about 340 degrees of freedom the
  density is ∞/∞ = NaN, and `InverseCDF`'s Newton steps (`:215-219`) make the quantile NaN.
  `GrubbsRosnerListSampleFilter` compares its statistic with a NaN critical value: false, so it
  never finds an outlier in a sample of more than about 342 values (any image), and its
  winsorizing bounds are NaN.
- **Evidence:** read; verified (`grubbs-rosner-outliers`: six outliers 1,800 to 3,800 above a
  phantom's brightest tissue are kept; SLATEC's messages on standard error, as ANTs prints
  them).
- **fMRIPrep:** none (sMRIPrep uses no outlier handling).
- **larmorx:** corrected by default: where the Gamma functions would overflow, their ratio is
  taken through `dlngam` (smaller arguments as ITK computes them, bit for bit), with a notice
  where Grubbs-Rosner's result changes; `reproduce` as ITK, its SLATEC messages included (A6).
- **D13 / gap:** none.

### ITK-60 The 2D rigid landmark fit takes −90° near a quarter turn — (c)
- **What:** `LandmarkBasedTransformInitializer`'s `Rigid2DTransform` fit computes the angle as
  `atan2(s_cross, s_dot)` of the summed cross and dot products of the centred landmarks, but
  when `|s_dot| ≤ 5e-5` it takes `−π/2` whatever the sign of `s_cross`
  (`itkLandmarkBasedTransformInitializer.hxx:724-731`): a quarter turn one way gives the
  rotation the other way. With fewer than 2 landmarks it warns ("Less than 2 landmarks
  available. Rotation is not computed", `:735`).
- **Evidence:** read; unit tests of `larmorx_registration::landmark` against ITK's build.
- **fMRIPrep:** none (antsAI `-b` in 2D only).
- **larmorx:** corrected by default (2026-10-23): `atan2` there too, a notice where the angle
  differs; `reproduce` keeps −π/2.
- **D13 / gap:** done. No gap.

### ITK-95 Grayscale morphology on a first axis of one voxel — (c)
- **What:** ImageMath's `GO`, `GC`, `GD` and `GE` with a radius of 0 use a one-voxel
  `BinaryBallStructuringElement`, which `GrayscaleDilateImageFilter` and its siblings run with
  `MovingHistogramImageFilter`. On an image whose first axis has one voxel (1×3×1, 1×3×4, 1×4 in
  2D), the line-by-line histogram moves empty some voxels' histograms, which then give the
  boundary value: ±FLT_MAX (3.4e38) instead of the voxel's own value. A radius of 0 is the
  identity on every other image. The same filters on such an image with a radius above 0 give
  values from outside the structuring element.
- **Evidence:** verified (random testing, `ants-image-math:trial1:47`;
  `validation/investigations/difftest/morphology_radius0.py`: which voxels, per shape and
  operation). Above radius 0: `ants-image-math:20261025:1541` (`ImageMath 4 out GD a 3`, a 4D
  image of 1x7x4x6 voxels): ANTs writes 99 at two voxels where the maximum over ITK's ball of
  radius 3 (the offsets within 3.5 voxels) is 97, larmorx's value; computed by brute force over
  the ball.
- **fMRIPrep:** none (its radii are 1 and more, on images with no axis of one voxel).
- **larmorx:** the true result in both modes (the identity at radius 0; larmorx computes the
  ball's extremum directly, not with ITK's moving histogram). Decided by rule A (the input makes
  sense, the result is wrong) on 2026-10-25.
- **D13 / gap:** the switch and the notice: `reproduce` should give ITK's values (its
  moving-histogram order), and the default should name ITK-95 where they differ.

### ITK-96 The HDF5 writer fails on a transform without parameters — (a)
- **What:** ITK's `HDF5TransformIOTemplate::Write` stores each transform's parameters as a
  chunked, compressed dataset; a transform with no parameters (an `IdentityTransform`, or the
  inverse that a displacement field without an inverse field gives in antsRegistration's
  inverse composite) asks for a chunk of 0 values: `H5Pset_chunk` fails ("all chunk dimensions
  must be positive"), ITK throws, and the file stays part-written (`itkHDF5TransformIO.cxx:462`).
  It is ITK-15's writer behaviour, met where antsRegistration writes its inverse composite.
- **Evidence:** verified (random testing, `itk-transforms:trial2`, `ants-registration:20261025:375`:
  ANTs writes `rInverseComposite.h5` part-written and 14 lines of HDF5 errors).
- **fMRIPrep:** none.
- **larmorx:** antsRegistration writes no inverse composite there and names ITK-96, in both
  modes (`larmorx_ants::cli::registration`); larmorx's HDF5 writer itself stores a transform
  without parameters uncompressed by default and gives ITK's error and part-written file in
  `reproduce` (ITK-15, `larmorx_io::itk_hdf5`).
- **D13 / gap:** the switch: antsRegistration's `reproduce` should write the part-written
  inverse composite and HDF5's messages.

### ITK-97 A sampled metric's derivative outside the virtual region is aliased — (c)
- **What:** with a sampling strategy and a shrink factor, ANTs draws the sample points on the
  fixed image while the stage's virtual domain is the shrunk one; a point near the edge can have
  a virtual index outside the shrunk region. For a transform of local support (the
  displacement-field stages), `ImageToImageMetricv4GetValueAndDerivativeThreaderBase::
  StorePointDerivativeResult` (line 309) adds the point's derivative at
  `ComputeParameterOffsetFromVirtualIndex(index)`, `ComputeOffset(index) · D` without a bounds
  check (`itkObjectToObjectMetric.hxx:346-352`): one past the end of a row is the next row's first
  voxel, and so on (Mattes stores the point's joint-PDF index there in the same way).
- **Evidence:** read; verified (random testing, 2026-10-25: `ants-registration:20261025:263`,
  `:353`, `:387`, `:437`, `:564`: the metric differs from the fourth iteration in the 12th digit,
  identical with `None` sampling or without the shrink factor; reproduced, all five are now
  bit-identical; `validation/investigations/difftest/bisect_registration.py`).
- **fMRIPrep:** none.
- **larmorx:** by default the derivative of such a point is dropped (its value still counts);
  `reproduce` adds it where ITK does (`larmorx_registration::metric::OutsideDerivatives`); both
  modes name ITK-97 with the number of derivatives (points times evaluations). Decided by rule A
  (the input makes sense; the derivative lands on a voxel it does not belong to) on 2026-10-25.
- **D13 / gap:** the linear, displacement-field, SyN and B-spline SyN stages alias in `reproduce`
  and name it in both modes; the time-varying velocity-field stages drop such points in both modes
  and do not name it (their sampling is clock-seeded, ANTS-82, so `reproduce` is not comparable).

### ITK-98 B-spline interpolation reads outside the coefficients on a short axis — (a)
- **What:** `BSplineInterpolateImageFunction::ApplyMirrorBoundaryConditions`
  (`itkBSplineInterpolateImageFunction.hxx:462-495`) mirrors each support index once at each end
  of an axis (`i < 0: -i`; `i >= end: 2 end - i`). On an axis of 2 voxels (orders 3 and 4) or 2
  or 3 voxels (order 5) the points near the end get an index of -1 (or more) after the mirror,
  and `m_Coefficients->GetPixel` reads outside the coefficient image's buffer: the result
  depends on memory ITK never wrote. Orders 0 to 2 are never affected; an axis of one voxel is
  handled.
- **Evidence:** read; verified (random testing, `ants-resample-image:20261025:4` and `:14`,
  where larmorx panicked on the index -1: `ResampleImage 3 a out 19x11x8 1 4 5` on a 3×2×2 image,
  `ResampleImage 3 a out 2.5 0 4` on 2×3×1).
- **fMRIPrep:** none.
- **larmorx:** by default the mirror is repeated until the index lies inside the axis: the
  decomposition (`BSplineDecompositionImageFilter`) assumes the whole-sample mirror of period
  `2 (L - 1)`, so this is the exact continuation of the interpolant (symmetric about every
  sample, as the unit test `a_short_axis_is_mirrored_again` checks). `reproduce` keeps ITK's
  single mirror, everything ITK reads inside the image reproduced, and takes the reads outside it
  as 0. Both modes name ITK-98 with the number of output voxels concerned
  (`larmorx_interp::BSplineMirror`; ResampleImage and antsApplyTransforms). Class (a), never
  reproduced by default; its ANTs runs are never cached.
- **D13 / gap:** the notice in antsRegistration's warped outputs and the N-D resampler
  (`larmorx_registration::resample`), which use the repeated mirror without naming it.

### ITK-99 A sampled metric writes a derivative past its array — (a)
- **What:** as ITK-97, with a linear offset before the array's start or past its end (an index
  before the first voxel, or past the last plane): ITK writes outside the per-thread derivative
  array.
- **Evidence:** read (no crash seen in the random cases).
- **fMRIPrep:** none.
- **larmorx:** dropped in both modes, named ITK-99 (class (a): memory it does not own; its ANTs
  runs are never cached).
- **D13 / gap:** none.

### ITK-100 A later level's zero Mattes bin size is used silently — (e)
- **What:** `MattesMutualInformationImageToImageMetricv4GetValueAndDerivativeThreader::
  BeforeThreadedExecution` (`...Threader.hxx:96-124`) creates its joint histogram images, with
  the two bin sizes as their spacing, only when none exist with the same number of work units
  and bins; the metric object, and so the images, last the whole stage. At the first level a
  bin size of 0 (an analysis region of one intensity: the sampled points, or the masked or whole
  fixed image) makes `ImageBase::SetSpacing` throw (`itkImageBase.hxx:77`); at a later level
  `Initialize` computes the new bin sizes (`itkMattesMutualInformationImageToImageMetricv4.hxx:
  254-261`) and the images are only refilled, so a bin size of 0 is used: every fixed value
  divides by 0, the bin indices are NaN or infinite and clamp to the histogram's edges, and the
  metric is 0 (ANTs then stops the level after its convergence window).
- **Evidence:** read; verified (random testing, 2026-10-25: `ants-registration:20261025:884`, a
  BSpline stage with MI, `Regular` sampling or none, a sparse mask where the full-resolution
  fixed image has one intensity: ANTs' second level prints a metric of `-0` and exits 0; larmorx
  threw the first level's exception at the second level in both modes; now bit-identical in
  `reproduce`).
- **fMRIPrep:** none (its masks hold the brain's intensities at every level).
- **larmorx:** by default the first level's exception at any level (rule B: the input makes no
  sense for mutual information, and ITK silently computes an arbitrary 0 where its first level
  refuses), with a notice naming ITK-100; `reproduce` uses the bin size as ITK does, named
  (`larmorx_registration::metric::mattes::check_joint_pdf_spacing`; unit test
  `a_later_level_zero_bin_size_is_used_as_itk_uses_it`). Decided by rule B on 2026-10-25.
- **D13 / gap:** none.

## fMRIPrep and its libraries

fMRIPrep 25.2.6 (the oracle runs) or its source at `21a490fb`, niworkflows 1.15.2, sdcflows
2.15.1, nipype 1.12.0, nitransforms 25.1.0. Details in [fmriprep-oracle.md](fmriprep-oracle.md),
[fmriprep-resampling.md](fmriprep-resampling.md), [scipy-ndimage.md](scipy-ndimage.md) and
`docs/analysis/01-04`.

### PIPE-01 nipype drops `shrink_factor` — (c)
- **What:** niworkflows passes `shrink_factor=2` to `pe.Node(...)` instead of the N4
  interface; nipype's `Node` silently drops unknown keyword arguments, so the BOLD reference's
  N4 runs with ANTs' default shrink factor 4.
- **Evidence:** read: `niworkflows/func/util.py:378-383` (1.15.2), nipype
  `pipeline/engine/nodes.py:105,181`; verified (`command.txt` of the oracle run: no
  `--shrink-factor`).
- **fMRIPrep:** every run's BOLD reference.
- **larmorx:** not a tool defect: the tools do what they are asked, and the stage comparison
  passes fMRIPrep's command as it ran.
- **D13 / gap:** decided 2026-10-23 (decision log): decided when larmorprepx is built, by the usual rule (the intended behaviour by default, fMRIPrep's on request). Was: **decide** for larmorprepx: the intended 2 or the factor 4 fMRIPrep uses.

### PIPE-02 The one-shot resampler races with OpenBLAS — (b)
- **What:** `ResampleSeries` resamples volumes concurrently (asyncio tasks in a thread pool);
  each maps its coordinates with nibabel's `apply_affine`, an OpenBLAS product. With OpenBLAS's
  pthreads build running threads of its own, results go wrong: 22,635 voxels wrong in 12 of 240
  volumes (by up to 846, data up to 1,320), other volumes on a second run.
- **Evidence:** read: `fmriprep/interfaces/resampling.py:292-297,396-425`,
  `fmriprep/utils/asynctools.py:5-8`; verified (run 01's MNI resampling re-run in fMRIPrep's
  image with `OMP_NUM_THREADS=8`).
- **fMRIPrep:** prevented only by the image's `ENV OMP_NUM_THREADS=1`
  (`fmriprep/Dockerfile:90-91`): fMRIPrep outside its image (pip, pixi), or in Apptainer without
  `--cleanenv` where `OMP_NUM_THREADS` is set, corrupts volumes and reports nothing.
- **larmorx:** the products are its own, in Rust; the result does not depend on any thread count.
- **D13 / gap:** never reproduced. No gap.

### PIPE-03 The Jacobian is taken along the wrong axis — (c)
- **What:** `np.gradient(vsm, axis=pe_info[0])` uses the phase-encoding axis index of the
  *source* grid on the *target* array. It is the right derivative only when the target's axes
  match the source's (the native boldref); in T1w or standard space it differentiates along
  whatever target axis has that index.
- **Evidence:** read: `fmriprep/interfaces/resampling.py:316`; verified on a real PEPOLAR run
  (2026-10-12, below): the stage comparison of fMRIPrep 25.2.6's ds005040 run reproduces its
  four field-map `ResampleSeries` outputs bit for bit with `upstream_defects="reproduce"`
  ([fmriprep-stages-ds005040](../validation/fmriprep-stages-ds005040.md)). First measured on
  real geometry with a synthetic field map (ds000005 has no field maps; the ds001600 run
  skipped its BOLD runs).
- **fMRIPrep:** the Jacobian is on by default for PEPOLAR field maps only
  (`fmriprep/workflows/base.py:853-858` in 25.2.6; `_jacobian_for` at `21a490fb`), and with
  `--force fmap-jacobian` for every kind; outputs in standard spaces (the derivative steps
  over one target voxel, which in BOLD voxels is the wrong length and, when the grids are
  rotated, the wrong direction: 0.57 BOLD voxels in MNI res-2 for ds005040's 3 mm BOLD, so 57 %
  of the correction; 1.65 for ds005454's 1.4 mm BOLD, an amplified one), and in `space-T1w`
  when the BOLD is acquired obliquely (fMRIPrep's `space-T1w` grid is the T1w's orientation at
  the BOLD's voxel size, so only the angle between the grids matters there: 3° for ds005040,
  25° for ds005454).
- **larmorx:** corrected by default (2026-10-11): the Jacobian is `1 + ∇_t vsm · ∂t/∂s_pe`,
  the derivative along the phase-encoding direction mapped into the target grid (the Jacobian
  determinant of the corrected mapping relative to the uncorrected one, by the matrix
  determinant lemma; `∂t/∂s_pe` is column `pe` of the inverse Jacobian of the target-to-source
  voxel map, exact for chains of affines, by central differences through a displacement field).
  On the source's own grid it is fMRIPrep's float32 value bit for bit. A warning says when the
  two differ; `upstream_defects="reproduce"` gives fMRIPrep's
  (`larmorx_transform::resample_series`, `lx.transforms.resample_series`). The effect is in
  [validation/resample-series.md](../validation/resample-series.md) ("D13 fix pass").
- **Measured on real geometry** (2026-10-11,
  [validation/resample-series-jacobian.md](../validation/resample-series-jacobian.md)): the
  grids and transforms of fMRIPrep's ds000005 run with a synthetic 3 T-like field map (shifts up
  to 3.6 voxels, exact factor 0.57 to 1.43). On the native grid the two factors are
  bit-identical. In T1w space fMRIPrep's factor spans only 0.82 to 1.18; it is off the exact
  factor by more than 0.01 (1 % of the signal) in 40 % of brain voxels, by more than 0.05 in
  21 %, with a 99th-percentile error of 0.23. In MNI res-2: 33 % and 14 %, 99th percentile
  0.16. larmorx's factor is within 0.002 (T1w) and 0.008 (MNI) of the exact one at the 99th
  percentile. (fMRIPrep's own `space-T1w` BOLD is not on the 1 mm T1w grid used there but at
  the BOLD's voxel size; on that grid ds000005's 31° angle between the BOLD's `j` axis and the
  T1w's still separates the factors by more than 0.01 in 36 % of brain voxels.)
- **Measured on fMRIPrep's own field maps** (2026-10-12,
  [validation/resample-series-jacobian.md](../validation/resample-series-jacobian.md), "On
  fMRIPrep's own field maps", and the stage records). ds005040 sub-002 (PEPOLAR with `topup`,
  3×3×3.3 mm BOLD, Jacobian on as fMRIPrep runs it): on the boldref grid (`desc-preproc_bold`
  and the unwarped boldref) larmorx's default is fMRIPrep's output bit for bit; in `space-T1w`
  (3 mm) it changes 78.8 % of brain voxels by more than 0.1 % and 2.8 % by more than 1 %
  (99th percentile 1.4 %, largest 6.8 %); in MNI res-2, 95.6 % by more than 0.1 %, 59.5 % by
  more than 1 %, 5.6 % by more than 5 % (99th percentile 11 %, largest 56 %), the changes above
  10 % almost all (94 %) below z = −10 mm, mostly lateral: the inferior temporal lobes,
  where the field is steepest. Against the factor on the BOLD's own grid carried to each output
  grid (a reference with no choice of axis), fMRIPrep's MNI factor is off by 0.012 (median),
  0.10 (99th percentile), larmorx's by 0.005 and 0.063 (the reference's own error, from 3 mm
  central differences, is part of both). ds005454 sub-16 (`phasediff`, oblique 1.4 mm BOLD:
  fMRIPrep runs it without the Jacobian, so larmorx's default leaves its outputs unchanged;
  with the Jacobian forced on): fMRIPrep's factor is off the reference by 0.031 / 0.39 (median
  / 99th percentile) in `space-T1w` and 0.021 / 0.26 in MNI, larmorx's by 0.0006 / 0.007 and
  0.0011 / 0.016; larmorx's default would change 79 % (`space-T1w`) and 71 % (MNI) of brain
  voxels by more than 1 %. ds005752's field map is not used by fMRIPrep (no `IntendedFor`), so
  it does not reach `ResampleSeries`.
- **D13 / gap:** done.

### PIPE-04 `.h5` composites read in name order — (c), latent
- **What:** nitransforms iterates `list(h5group.values())[1:]`, which is h5py's name order: for
  a composite of 11 or more transforms, `"10"` sorts before `"2"` and the chain is misordered.
- **Evidence:** read: `nitransforms/io/itk.py:391`; not verified with such a file.
- **fMRIPrep:** latent (its composites have 2 or 3 transforms).
- **larmorx:** numeric order by default (2026-10-11), with a warning when the two orders
  differ; `upstream_defects="reproduce"` gives the name order (`lx.transforms.load_itk_composite`,
  `load_transforms`, `resample_series`). `lx.transforms.read` sorts numerically.
- **D13 / gap:** done.

### PIPE-05 No displacement beyond a field's last nodes — (c?)
- **What:** `DenseFieldTransform.map` interpolates the field with `cval` NaN and replaces NaN
  results by the input point, so points beyond the outermost nodes are not displaced at all: a
  discontinuity at the field's edge. ITK's `DisplacementFieldTransform` does the same beyond
  half a voxel.
- **Evidence:** read: `nitransforms/nonlinear.py:185-209`; verified: 31,411 of 1,082,035 MNI
  res-2 points (2.9 %) with a res-1 warp.
- **fMRIPrep:** every MNI resampling, at the edge of the warp's field of view.
- **larmorx:** reproduces.
- **D13 / gap:** decided 2026-10-23 (decision log): decided when larmorprepx is built, by the usual rule (the intended behaviour by default, fMRIPrep's on request). Was: **decide.**

### PIPE-06 Float32 roundings at every step — (d)
- **What:** nitransforms parses ITK text parameters as float32 (`io/itk.py:200-203`; ANTs reads
  them in double, about 1e-7 relative apart) and converts coordinates to float32 before every
  affine (`base.py:351-375`); fMRIPrep rounds the target grid's coordinates to float32
  (`resampling.py:572`); the voxel-shift map is float32 (NumPy 2's NEP 50).
- **Evidence:** read; validated (resample-series, bit-identical).
- **fMRIPrep:** every run.
- **larmorx:** reproduces.
- **D13 / gap:** not D13; double precision is a D11 option.

### PIPE-07 A one-transform head-motion file — (c?)
- **What:** `nt.linear.load` turns a one-transform list into an `Affine`, which `resample_image`
  applies in world space as part of the chain (float32-rounded input), not as a voxel-to-voxel
  head-motion matrix.
- **Evidence:** read: `nitransforms/linear.py:451-452`.
- **fMRIPrep:** one-volume series.
- **larmorx:** reproduces.
- **D13 / gap:** decided 2026-10-23 (decision log): decided when larmorprepx is built, by the usual rule (the intended behaviour by default, fMRIPrep's on request). Was: **decide.**

### PIPE-08 Motion parameters about another centre — (c?)
- **What:** fMRIPrep recomputes the motion parameters from the ITK transforms about
  `zooms × scipy.ndimage.center_of_mass(boldref)`, in file order and without subtracting the
  minimum; mcflirt's own parameters use the centre of mass with the minimum subtracted, in its
  radiological order. The translations (and FD) are therefore about a different point.
- **Evidence:** read: `fmriprep/interfaces/confounds.py:102-229`; the mcflirt rule in
  `specs/mcflirt.md` §2, §10.3.
- **fMRIPrep:** every run's motion confounds.
- **larmorx:** the confounds are not ported yet.
- **D13 / gap:** decided 2026-10-23 (decision log): decided when larmorprepx is built, by the usual rule (the intended behaviour by default, fMRIPrep's on request). Was: **decide** for larmorprepx.

### PIPE-09 Misleading log lines and wasted space — (f)
- **What:** the DataSink logs "Changing ... dtype from float64 to float64" for a mask it writes
  as `uint8`, and "int16 to int16" for a boldref written as float32; `fsl2itk` warns
  "Multithreading is deprecated" because fMRIPrep still passes `num_threads`; masks in T1w and
  MNI space pass through the working directory as float64 (8 bytes per voxel).
- **Evidence:** verified (`fmriprep.log`, nibabel on the outputs).
- **fMRIPrep:** every run.
- **larmorx:** –.
- **D13 / gap:** not D13.

### PIPE-10 Options that do nothing — (e)
- **What:** `--force-bbr`, `--force-no-bbr` and `--force-syn` only print a deprecation notice
  (`fmriprep/cli/parser.py:50-70`); `--fmap-bspline` and `--fmap-no-demean` are stored and never
  read (`fmriprep/config.py:585-588`); `maxtasksperchild: 1` is read only by nipype's
  LegacyMultiProc, not by the MultiProc plugin fMRIPrep uses (`config.py:324-327`, nipype
  `multiproc.py:159-167`).
- **Evidence:** read (`docs/analysis/01`, `04`).
- **fMRIPrep:** users of those options.
- **larmorx:** –; larmorprepx must not copy them.
- **D13 / gap:** no tool code. No gap.

### PIPE-11 sMRIPrep: a registration fallback crashes downstream — (a)
- **What:** only `t1w-mni_registration_{precise,testing}_000.json` write a composite transform;
  if attempt 000 fails and 001 or 002 succeeds, `composite_transform` is undefined but sMRIPrep
  reads it (`smriprep/workflows/fit/registration.py:241-244`).
- **Evidence:** read (`docs/analysis/02`).
- **fMRIPrep:** after a failed first registration attempt.
- **larmorx:** –; larmorprepx must not copy it.
- **D13 / gap:** no tool code. No gap.

### PIPE-12 sMRIPrep: a default that is not an allowed choice — (e)
- **What:** the default `'first'` for `--subject-anatomical-reference` is not one of the allowed
  choices (`first-lex`, `unbiased`, `sessionwise`).
- **Evidence:** read: `smriprep/cli/run.py:140`.
- **fMRIPrep:** none (fMRIPrep's own CLI).
- **larmorx:** –.
- **D13 / gap:** no tool code. No gap.

### PIPE-13 sMRIPrep: the report text prints a module — (f)
- **What:** the boilerplate f-string uses `{image}`, which renders nipype's `image` module
  instead of the image type.
- **Evidence:** read: `smriprep/workflows/anatomical.py:1446`.
- **fMRIPrep:** the methods text of every report.
- **larmorx:** –.
- **D13 / gap:** not D13.

### PIPE-14 sdcflows ignores head motion when applying a field — (c)
- **What:** `B0FieldTransform.apply` accepts head-motion transforms and silently ignores them.
- **Evidence:** read: `sdcflows/transform.py:509-524` (`docs/analysis/03`).
- **fMRIPrep:** none (fMRIPrep applies the field in `ResampleSeries`).
- **larmorx:** –.
- **D13 / gap:** no larmorx code. No gap.

### PIPE-15 nipype's `MCFLIRT` interface — (e)
- **What:** `use_contour` emits `-edge`, which mcflirt rejects; with FSL 6 or later the
  interface expects the `-stats` images as `<out>_variance.nii.gz`, which mcflirt does not write
  (FSL-08).
- **Evidence:** verified.
- **fMRIPrep:** none (it uses neither).
- **larmorx:** `larmorx mri hmc` rejects `-edge` and writes mcflirt's names, as mcflirt does.
- **D13 / gap:** nipype's defect; the names are FSL-08's gap.

### PIPE-16 nipype writes N4's corrected image in the input's data type — (c?)
- **What:** nipype's `N4BiasFieldCorrection` interface, with `copy_header=True`, copies the
  input's header onto the outputs after ANTs has written them, and for the corrected image
  keeps **the input's data type** (`_copy_header_map = {"output_image": ("input_image",
  False), ...}`, `keep_dtype=False`; `nipype.utils.imagemanip.copy_header`). When the input is
  an integer image, nibabel re-quantizes N4's float32 result to that type with a scale factor
  of its choosing. sdcflows' brain extraction of the field-map reference (`fmap_preproc_wf/
  <estimator>/brainextraction_wf/n4`) runs N4 with `copy_header=True` on its clipped
  reference, which is int16 (`clipper_pre`): fMRIPrep's corrected reference is N4's output
  rounded to int16 with `scl_slope` 0.0032 and `scl_inter` 105.19 (ds005040).
- **Evidence:** read: nipype 1.12.0 `interfaces/ants/segmentation.py:503-506`,
  `interfaces/mixins/fixheader.py:117-134`, `utils/imagemanip.py:6-20`; sdcflows 2.15.1
  `workflows/ancillary.py:86-91`; verified (fMRIPrep 25.2.6's ds005040 run: the node's output
  is int16 with a scale factor; ANTs' own output, re-run in fMRIPrep's image at the node's 8
  threads, is float32 and differs from it in all 98,545 nonzero voxels, by up to 1.6e-3
  (3e-3 relative at the dimmest voxels, median 1.8e-5); after nipype's own `copy_header` on
  that re-run, bit-identical to fMRIPrep's output).
- **fMRIPrep:** every field-map estimation whose reference is clipped to an integer image
  (sdcflows' `BrainExtraction` mask comes from it); the brain-extraction N4s of the T1w and
  the BOLD reference have float32 inputs, so their data are unchanged.
- **larmorx:** not a tool defect: `larmorx ants N4BiasFieldCorrection` writes float32, as ANTs
  does. The stage comparison runs nipype's own `copy_header` on every run before comparing
  (`larmorx_validation.stages.nodes._NipypePost`).
- **D13 / gap:** decided by the user on 2026-10-21 (decision log): larmorprepx keeps N4's
  float32 output by default; `reproduce` writes the input's type as fMRIPrep does. To implement
  with larmorprepx.

### PIPE-17 nitransforms skips the first group of an `.h5` file — (c)
- **What:** `ITKCompositeH5.from_h5obj` reads `list(h5group.values())[1:]`, assuming group `0` is
  the composite. A file of one transform (what `antsApplyTransforms -o Linear[x.h5]` writes)
  gives an empty list, and `TransformChain([])` fails with `IndexError`; a file of several
  transforms without a composite loses its first; an empty composite fails the same way. The
  misspelt-name check looks at group `1` only.
- **Evidence:** read: nitransforms 25.1.0 `io/itk.py:381-430`, `manip.py:94`; verified
  (validation `itk-hdf5`, `read/itk/linear`, 2026-10-16).
- **fMRIPrep:** none (its composites start with the composite).
- **larmorx:** `lx.transforms.load_itk_composite` reads the first group when it is not a
  composite and an empty composite as the identity, with a warning; `reproduce` behaves as
  nitransforms.
- **D13 / gap:** done. No gap.

### SCI-01 `map_coordinates` reads outside its array — (a)
- **What:** coordinates go through `(npy_intp)floor(cc)`, undefined for NaN and beyond ±2^63; on
  x86-64 the start index wraps, the edge test is skipped, and SciPy reads memory outside its
  array (a 1D float64 array at coordinate 1e20, order 3: one element before the buffer).
- **Evidence:** read: `scipy/ndimage/src/ni_interpolation.c:466-588` (v1.15.2); verified.
- **fMRIPrep:** none (displacement-field NaNs are replaced before interpolation).
- **larmorx:** NaN where SciPy would read outside (expected divergence
  `ndimage/overflowing-coordinates`).
- **D13 / gap:** never reproduced. No gap.

### SCI-02 Undefined conversions answered as x86-64 does — (a)
- **What:** where SciPy stays inside its array, NaN coordinates and NaN results converted to an
  integer output follow x86-64's `cvttsd2si`: 0 for int8, int16 and unsigned types,
  `INT32_MIN` for int32, `INT64_MIN` for int64.
- **Evidence:** read; verified.
- **fMRIPrep:** none.
- **larmorx:** corrected by default (`upstream_defects`, 2026-10-12), as decided with ANTS-06: NaN results become 0 in signed integer
  outputs, and a `cval` that an integer input type cannot hold is clamped (NaN to 0), with a
  warning that counts them (`larmorx_interp::ndimage`); `reproduce` gives x86-64's answers on
  every platform.
- **D13 / gap:** done. No gap.

### SCI-03 `reflect` splines do not interpolate on short axes — (c)
- **What:** with `reflect` or `grid-mirror`, `map_coordinates` at integer coordinates misses
  the samples on short axes: on a 6 × 5 × 4 array of values around 10, by up to 2.5e-6 (order
  2), 1.4e-4 (3), 2.1e-3 (4) and 1.0e-2 (5); every other mode returns the samples to 1e-14.
- **Evidence:** verified in SciPy 1.15 ([scipy-ndimage.md](scipy-ndimage.md)).
- **fMRIPrep:** none (`grid-constant`).
- **larmorx:** corrected by default (2026-10-11). The cause is SciPy's causal initialisation
  for the reflect boundary (`ni_splines.c`, used for `reflect`, `grid-mirror` and `nearest`):
  its last term reads `c[0]` after the loop has already updated it; with the original `c[0]`
  the sum is the exact half-sample-symmetric initialisation and the splines pass through the
  samples (to 1e-9 on the 6 × 5 × 4 array, order 5). A warning says when a coefficient changed;
  `upstream_defects="reproduce"` gives SciPy's (`larmorx_interp::ndimage::splines`,
  `lx.ndimage`, `lx.transforms.resample_series(mode=...)`).
- **D13 / gap:** done.

### SCI-04 Fused multiply-adds on aarch64 — (d)
- **What:** SciPy's x86-64 wheels target baseline x86-64 and cannot fuse; on aarch64 GCC
  contracts `a*b + c` by default, so SciPy's results there can differ in the last bits.
- **Evidence:** read (build defaults); not verified on aarch64.
- **fMRIPrep:** ARM machines.
- **larmorx:** never contracts: SciPy's x86-64 bits everywhere.
- **D13 / gap:** not D13.

### NP-01 numpy's matrices depend on the CPU — (d)
- **What:** numpy's 4×4 products and inverses run in BLAS and LAPACK kernels chosen at run time:
  OpenBLAS's Haswell, SkylakeX, SandyBridge, Nehalem or Prescott kernels on x86-64, others on
  ARM, Accelerate on macOS. Without FMA, products of random affines differ from Haswell's in
  98.7 % of cases; inverses of affines in 18.5 % under SandyBridge; CI saw 1-112 ulp between
  platforms.
- **Evidence:** verified ([numpy-blas.md](numpy-blas.md)).
- **fMRIPrep:** its last bits depend on the machine.
- **larmorx:** OpenBLAS's Haswell order on every platform (`larmorx_transform::openblas`).
- **D13 / gap:** not D13.

### NP-02 Axis choice at exactly 45° — (d)
- **What:** nibabel's `io_orientation` decides axes from `np.linalg.svd`; for grids rotated by
  exactly 45° the cosines tie and the choice follows the SVD's rounding, which is
  platform-dependent.
- **Evidence:** validated (`orientation/45-degrees`).
- **fMRIPrep:** none.
- **larmorx:** vnl's LINPACK SVD; nibabel's choice on 4,300 of 4,800 such grids (expected
  divergence).
- **D13 / gap:** not D13.

### NIB-01 Compression decided by the file name — (e)
- **What:** nibabel 5.4 decides gzip from the name: a gzipped file named `.nii` fails, an
  uncompressed `.nii.gz` is not read.
- **Evidence:** validated (expected divergences in [nifti-io](../validation/nifti-io.md)).
- **fMRIPrep:** none.
- **larmorx:** decides from the content.
- **D13 / gap:** decided 2026-10-23 (decision log): larmorx's behaviour kept (the safer one). Was: **decide** (Decisions, item 5: no switch proposed).

### NIB-02 `vox_offset` 0 reads the header as data — (c)
- **What:** a single-file image with `vox_offset = 0` is read with the header bytes as voxel
  data.
- **Evidence:** validated (`malformed/vox-offset-zero.nii`).
- **fMRIPrep:** none.
- **larmorx:** refuses.
- **D13 / gap:** decided 2026-10-23 (decision log): larmorx's behaviour kept (the safer one). Was: **decide** (item 5).

### NIB-03 Complex data lose their imaginary part — (c)
- **What:** `get_fdata` on complex data silently drops the imaginary part.
- **Evidence:** validated.
- **fMRIPrep:** none.
- **larmorx:** raises.
- **D13 / gap:** decided 2026-10-23 (decision log): larmorx's behaviour kept (the safer one). Was: **decide** (item 5).

### NIB-04 qform precision depends on the platform — (d)
- **What:** nibabel computes the qform in `numpy.longdouble`: 80-bit on Linux x86-64, 64-bit
  elsewhere.
- **Evidence:** validated (within 1e-9 mm).
- **fMRIPrep:** none.
- **larmorx:** f64 everywhere.
- **D13 / gap:** not D13.

### GLIBC-01 `exp`, `log`, `sin`, `cos` are not correctly rounded — (d)
- **What:** glibc 2.35 misrounds about 0.08 % of `exp`, 0.04 % of `log`, 0.2 % of `sin` and
  0.13 % of `cos` results; FFT twiddles through `sincos`: 1 of 2,048 at 1024 points. ITK calls
  them in Gaussian and windowed-sinc weights, Euler matrices, N4's FFT and kernel, and
  DenoiseImage's Bessel functions; macOS and Windows have their own libraries.
- **Evidence:** verified against mpmath ([platform-math.md](platform-math.md)).
- **fMRIPrep:** last bits of ANTs results, different on other platforms.
- **larmorx:** correctly rounded CORE-MATH ports (rule 5); `LD_PRELOAD` of
  `scripts/build_crmath_preload.sh` makes ANTs do the same for parity.
- **D13 / gap:** not D13.

### GLIBC-02 `expf`, `logf`, `powf` are not correctly rounded — (d)
- **What:** `expf` misrounds 170,648 of 4,278,190,080 finite floats (0.004 %), `logf` 416,908
  of 2,139,095,039 positive floats (0.019 %, among them the integers 1579, 3100, 4935, 12014,
  18627, 47539, 56198, 58259, 62326 and 64341), `powf` about 0.06 % of random inputs. One
  misrounded `logf` changes a whole N4 run, about as much as a different thread count.
- **Evidence:** verified on every float input ([platform-math.md](platform-math.md)).
- **fMRIPrep:** N4 (`logf` of every voxel, `expf`), DenoiseImage (`expf` per patch pair),
  ImageMath `^`.
- **larmorx:** correctly rounded (`larmorx_core::math::{expf, logf, powf}`).
  The mcflirt replica (`larmorx-nc`) carries glibc 2.35's FMA build of `sinf`, `cosf`,
  `sincosf` and `logf`, translated from Arm's Optimized Routines (MIT) and verified on all 2^32
  inputs (`crates-nc/larmorx-nc-core/tools/glibc-float-verify`), as FSL's Linux binaries call
  them (2026-10-24).
- **D13 / gap:** not D13.

### GLIBC-03 `sinf`, `cosf` are not correctly rounded, and two builds differ — (d)
- **What:** over all 2^32 floats, glibc 2.35's `sinf` misrounds 29.4 million inputs and `cosf`
  28.2 million. glibc chooses between an FMA and an SSE2 build at load time, and they disagree
  on 12 `sinf` and 22 `cosf` inputs, so AFNI's weighted-sinc output can depend on the CPU.
- **Evidence:** verified on all 2^32 inputs of both functions (2026-10-10, with the
  verification tool of the 3dTshift replica).
- **fMRIPrep:** none (3dTshift's default interpolation is Fourier).
- **larmorx:** the 3dTshift original uses correctly rounded functions; the replica carries a
  translation of glibc's FMA build (LGPL), the one this machine's AFNI runs.
  `Euler3DTransform<float>` calls them too, under `antsApplyTransforms --float 1`; larmorx uses
  correctly rounded ports of CORE-MATH's `sinf` and `cosf` there (2026-10-11), and the CORE-MATH
  preload replaces glibc's `sinf`, `cosf` and `sincosf` ([ants-float.md](ants-float.md)).
  The mcflirt replica (`larmorx-nc`) carries glibc 2.35's FMA build of `sinf`, `cosf`,
  `sincosf` and `logf`, translated from Arm's Optimized Routines (MIT) and verified on all 2^32
  inputs (`crates-nc/larmorx-nc-core/tools/glibc-float-verify`), as FSL's Linux binaries call
  them (2026-10-24). glibc 2.35's `atanf` (fdlibm, no FMA build) is not correctly
  rounded on 21,089,462 of the 2^32 inputs (0.49 %), 7,060 with |x| ≤ 0.05; the replica carries it
  bit for bit (`atanf` on all 2^32 inputs, `atan2f` on 3 × 2^30 seeded pairs: 0 differences;
  2026-10-25).
- **D13 / gap:** not D13.

### GLIBC-04 `pow` for SciPy's filter poles — (d)
- **What:** glibc 2.35's `pow` misrounds 5 of 12,012 powers of the six B-spline filter poles,
  all below 1e-100, where they cannot change a coefficient.
- **Evidence:** verified against exact rational arithmetic.
- **fMRIPrep:** none.
- **larmorx:** exact (`larmorx_core::math::powi`).
- **D13 / gap:** not D13.

### ANTSPY-01 `ants.apply_transforms` inverts unasked — (c?)
- **What:** when given exactly two transforms, the first a `.mat` and the second not,
  `whichtoinvert` defaults to `(True, False)`: the affine is inverted without being asked.
- **Evidence:** read: `ants/registration/apply_transforms.py` (ANTsPy 0.6.3).
- **fMRIPrep:** none (fMRIPrep does not use ANTsPy).
- **larmorx:** `lx.ants.apply_transforms` inverts nothing unless asked.
- **D13 / gap:** no larmorx code. No gap.

### ANTSPY-02 `ants.create_ants_transform` rounds to float32 — (c)
- **What:** parameters are rounded to float32 even with `precision="double"` (an angle of 0.1
  is written as `0.10000000149011612`).
- **Evidence:** verified.
- **fMRIPrep:** none.
- **larmorx:** –; the test data write ITK text directly.
- **D13 / gap:** no larmorx code. No gap.

### ANTSPY-03 `ants.image_header_info`'s direction — (c)
- **What:** the direction is returned transposed and rounded to 4 decimals.
- **Evidence:** verified.
- **fMRIPrep:** none.
- **larmorx:** not used as a geometry oracle.
- **D13 / gap:** no larmorx code. No gap.

## Restricted: AFNI 3dTshift (source-derived)

> **Clean-room implementers: do not read this section.** Written from AFNI 25.2.09's MCW
> source (GPL), through [afni-tshift.md](afni-tshift.md). The clean-room original's
> implementers work from `specs/3dTshift.md`, which describes the behaviours they need.

### AFNI-01 † A negative `scl_slope` flips the data — (c)
- **What:** extraction applies the brick factor only when it is positive
  (`thd_dsetto1D.c`: `if( DSET_BRICK_FACTOR > 0.0 )`), insertion always divides by it
  (`thd_1Dtodset.c`): int16 data with slope −0.5 come out sign-flipped and doubled.
- **Evidence:** read; verified (scaled mean 942 in, −1884 out); spec §6 (observed).
- **fMRIPrep:** a BOLD file with a negative slope (rare).
- **larmorx:** both implementations apply the factor on extraction too by default, and
  reproduce AFNI with the switch, warning in both modes (the original since 2026-10-11, the
  replica since 2026-10-12: `larmorx_gpl_afni::tshift`, `extract`).
- **D13 / gap:** done.

### AFNI-02 † float32 with slope and intercept is scaled twice — (c)
- **What:** 3dTshift works on `EDIT_full_copy` of the input, whose brick factors are set when
  the header is opened (`edt_fullcopy.c:31`), before the data are loaded (`:44`). For float32
  with both a slope and an intercept the factor is set at open time (`thd_niftiread.c:720`) and
  loading also scales the values (`:1068-1088`): the output stores `2x + 3` with `scl_slope = 2`,
  so a reader sees `4x + 6`.
- **Evidence:** read; verified with the oracle (2026-10-10). No parity case (the test data have
  no such file).
- **fMRIPrep:** float32 BOLD with a slope and an intercept (rare).
- **larmorx:** both implementations scale once by default (no brick factor) and keep the
  factor with the switch, warning in both modes (the original since 2026-10-11, the replica
  since 2026-10-12: `larmorx_gpl_afni::dataset::read`).
- **D13 / gap:** done.

### AFNI-03 † Header slice code 2 shifts nothing — (c)
- **What:** for `slice_code = 2` (sequential decreasing) `THD_open_nifti` loops
  `for( kk=slice_end ; kk >= slice_end ; kk-- )` (`thd_niftiread.c:653`): only `slice_end` is
  visited, every offset stays 0, and 3dTshift warns "input has only 1 time offset" and shifts
  nothing. 3dAutomask and 3dUnifize write no slice timing either (a dataset's slice fields are
  zero); the GPL replicas name AFNI-03 where that changes the written header (random testing,
  trial runs of 2026-10-25).
- **Evidence:** read; validated (`header-timing/code2-seq-dec`); spec §3.2 (observed).
- **fMRIPrep:** none (it passes `-tpattern`).
- **larmorx:** both implementations visit the slices from `slice_end` down to
  `slice_start` by default and reproduce AFNI's zeros with the switch, warning in both modes
  (the replica since 2026-10-12: `larmorx_gpl_afni::dataset::slice_offsets`). Only the
  command line's header-timing path meets it; the Python API requires `slice_times`.
- **D13 / gap:** done.

### AFNI-04 † A NULL write in the Fourier pair shift — (a)
- **What:** when both rows' shifts exceed the series, `fft_shift2` writes
  `f[ii] = g[ii] = 0.0` (`thd_shift2.c:80`) even when `g` is NULL, which it is for the last
  voxel of a slice with an odd number of voxels.
- **Evidence:** read.
- **fMRIPrep:** none (it needs a `-tzero` more than `ntt` TRs from a slice time).
- **larmorx:** the replica clears the one row; the original outputs zeros (spec §5.1).
- **D13 / gap:** never reproduced. No gap.

### AFNI-05 † `-no_detrend` de-means the wrong voxel — (c)
- **What:** for the second voxel of each pair, `THD_const_detrend` is called on the first
  voxel's data (`3dTshift.c:632-633`): the second voxel is never de-meaned and the first is
  de-meaned twice; the first's clipping range is recorded between its two de-meanings.
- **Evidence:** read; validated (`options/*-no-detrend`); spec §4 (observed).
- **fMRIPrep:** none (not used).
- **larmorx:** both implementations de-mean each voxel with its own mean by default and
  reproduce AFNI's pairs with the switch, warning in both modes (the replica since
  2026-10-12: `larmorx_gpl_afni::tshift`, `shift_pair`); the original's last bits are not exact
  in either mode.
- **D13 / gap:** done.

### AFNI-06 † `-rlt+` restores the intercept — (f; c? until 2026-10-12)
- **What:** AFNI's help says `-rlt+` adds the mean back; 3dTshift adds the fitted trend's
  intercept, its value at the first time point used (a series `50 + 2i` comes back at level 50,
  not 113).
- **Evidence:** spec §4 (observed).
- **fMRIPrep:** none.
- **larmorx:** both reproduce it, in both modes; the Python API calls it
  `restore="intercept"`, and the docs say what it does (`docs/api/afni-tshift.md`, spec §4).
- **D13 / gap:** decided by the user (2026-10-12): AFNI's behaviour is kept and the help text
  is the error (class f, not D13). No gap.

### AFNI-07 † Metre and micron units — (c)
- **What:** `THD_open_nifti` converts the voxel sizes (the axes' steps `xxdel`...) of a header
  in metres or micrometres to millimetres (`thd_niftiread.c:357-360`, `:466-470`), but neither
  the origins (`xxorg`...) nor `ijk_to_dicom_real`. When the dataset is written,
  `THD_set_dataset_attributes` (`thd_dsetatr.c:167-180`) replaces a plumb `ijk_to_dicom_real`
  by the cardinal matrix of the axes (AFNI-16), so a **plumb** grid comes out with its linear
  part in millimetres and its origin in metres; an **oblique** grid keeps the matrix as read,
  so its whole sform stays in metres while `pixdim` is in millimetres. The qform takes the
  axes' origins (`thd_niftiwrite.c:387-389`): unconverted in both cases.
- **Evidence:** read; verified (2026-10-12, with the oracle: plumb and oblique, sform-only and
  qform-only metre inputs, a micrometre input without a transform; the replica's output is
  AFNI's byte for byte in each). The record of 2026-10-10 ("the replica's sform and qform agree
  with AFNI's") covered only a qform-only input, whose sform the replica wrote from the same
  matrix; D13's sform cases showed the difference (2026-10-11).
- **fMRIPrep:** inputs stored in metres or microns (rare).
- **larmorx:** both implementations convert the whole matrix and the origin by default and
  warn (the original since 2026-10-11, the replica since 2026-10-12:
  `larmorx_gpl_afni::dataset::geometry`, `larmorx_gpl_afni::axes`); the replica reproduces AFNI
  byte for byte with the switch. So does the clean-room original since 2026-10-12 (completing
  3dTshift, part 2b; `larmorx_afni::geometry`, from spec §6.1): `defects/oblique-metre` and
  `-zero-origin`, once expected divergences in its record, pass.
- **D13 / gap:** done. The spec describes AFNI's geometry byte for byte (§6.1: 162 inputs, six
  orientations, plumb, nearly plumb and oblique, sform, qform and both, millimetres, metres and
  micrometres), and when the notice is due: metres or micrometres with an origin, or with an
  entry off the axes (an oblique or nearly plumb matrix, whose conversion differs in at least
  one AFNI-16 mode; both implementations name AFNI-07 so, in both modes alike).

### AFNI-08 † An all-zero slice-time list writes `slice_end` 0 — (c)
- **What:** when the output keeps slice offsets, `populate_nifti_image`
  (`thd_niftiwrite.c:519-594`) derives the slice fields; its "all zeros" test
  (`sfirst == slast && MYFPEQ(tlist[sfirst],0.0)`, `:543`) can never be true, so a list of
  zeros (every time within `min_timing_diff`, 0.003, of 0) finds no pattern and gets
  `slice_end = 0`.
- **Trigger** (found 2026-10-12): the output keeps slice offsets only in a "copy of the input"
  (`TS_copy_input_to_output`, `3dTshift.c:703`). A list of zeros cannot reach one: a header
  list is copied only when a time lies outside `[0, TR]`, and AFNI-03's zeros (code 2) and
  codes 7 and up never do; `-tpattern` lists outside the TR are errors. Times all within
  3 ms of 0 but beyond a TR shorter than them do: header slice code 1 with a duration of 0.5 ms
  and a TR of 1 ms (or `-TR 0.001`) over 4 slices gives 0, 0.5, 1, 1.5 ms, a copy, and
  `slice_end = 0`, slice code 0 and duration 0 in the output. A time axis from AFNI's
  extension whose slice count is not `nz` ("dataset temporal pattern is malformed!") is the
  other route.
- **Evidence:** read; verified with the oracle (2026-10-12, the trigger above).
- **fMRIPrep:** none (header field only, in "copy of input" cases with a TR below the slice
  times).
- **larmorx:** the replica makes the test what it meant ("no time away from 0") by default, so
  such a list gets no slice code, no duration and `slice_end = nz − 1`, and reproduces
  `slice_end = 0` with the switch, warning in both modes (2026-10-12:
  `larmorx_gpl_afni::dataset::slice_fields`). The clean-room original does the same since
  2026-10-12 (completing 3dTshift, part 2b; `larmorx_afni::timing::copy_slice_fields`, from
  spec §6.2). Until then it wrote `nz − 1` always and kept the input header's slice code and
  duration in a copy (code 1, 0.5 ms here), where AFNI derives them from the times.
- **D13 / gap:** done. The spec describes the trigger and the derivation of the slice fields of
  a copy (§6.2; the rule matched AFNI's fields on 150 random header timings, 30 of them
  AFNI-08). The same derivation writes AFNI-17's fields and turns a two-slice header code 3
  into code 1; both implementations follow it (`defects/tiny-offsets-copy`,
  `copy/short-slice-spacing`, `copy/alt-two-slices` pass in both records).

### AFNI-09 † Slice-time files are rounded two ways — (d)
- **What:** a line without a letter, `*` or `@` is read with `strtod` and rounded to float
  (two roundings); a line with one, including the `e` of an exponent, with `sscanf("%f")`, which
  rounds once (`mri_read.c:2300-2361`). They differ only for decimal strings at a float
  rounding tie.
- **Evidence:** read.
- **fMRIPrep:** none in practice.
- **larmorx:** the replica reproduces it; the original parses as a double and rounds (spec §3.1).
- **D13 / gap:** not D13.

### AFNI-10 † AFNI's own header extension wins — (d)
- **What:** for files AFNI wrote, the attributes in its extension (numbers printed with 7
  significant digits, `niml_rowtype.c:925`) replace the NIfTI header's time axis, matrix and
  space (`thd_niftiread.c:779-875`, `thd_initdblk.c:1110-1172`); slice offsets read back from 7
  digits differ in the last bit from the header's.
- **Evidence:** verified (OpenNeuro ds000210: shifts differ by up to 3e-8 TR; 21 of 35 million
  int16 values round the other way).
- **fMRIPrep:** AFNI-written inputs; fMRIPrep passes `-TR` and `-tpattern` itself, so only the
  matrix and space reach it.
- **larmorx:** the replica reads the extension; the original ignores it (within thresholds).
- **D13 / gap:** not D13.

### AFNI-11 † An existing output is not written, exit 0 — (e)
- **What:** 3dTshift prints "dataset NOT written to disk!" and exits 0.
- **Evidence:** validated (`errors/existing-output`). AFNI checks when it writes
  (`thd_writedset.c:149-162`): "output dataset name '...' conflicts with existing file" (the
  name without its directory), "dataset NOT written to disk!", and 3dTshift exits 0.
  `AFNI_DECONFLICT` changes this (corrected 2026-10-12, when this entry said that `NO` stops
  early in every case; verified for each value, shifts and copies, with and without
  `-verbose`): exactly `OVERWRITE` overwrites with a warning (`THD_ok_overwrite`,
  `afni_environ.c:804-810`), quietly after `-overwrite` (`:535-540`, which sets that value);
  any other value starting with `O` or `o` overwrites quietly (`thd_writedset.c:150`); one
  starting with `Y` or `y` renames the output to the first free `_AA1` … `_ZZ9` with a warning
  (`:153-157`, `THD_deconflict_prefix`, `thd_opendset.c:455`); one starting with an upper-case
  `N` stops a shift before the work (`THD_deathcon`, `afni_environ.c:791-796`;
  `3dTshift.c:539-540`: "output dataset already exists", exit 1) but not a copy of the input,
  which 3dTshift writes without that test (AFNI-11 then); lower-case `n` and anything else is
  as unset. With `-verbose`, AFNI-11's run still prints `++ Wrote output: ./out_AA1.nii`,
  the name the deconflicting would have chosen, of a file it did not write
  (`3dTshift.c:696-697` and `:709-710`, after `THD_deconflict_prefix` changed the prefix in
  place).
- **fMRIPrep:** none (fresh working directories).
- **larmorx:** an error, exit 1, in both implementations by default; the replica's switch
  (2026-10-12) prints AFNI's two messages, writes nothing and exits 0, as AFNI does with its
  environment variables unset. Since 2026-10-12 (AFNI's global options, `specs/3dTshift.md`
  §7.1) the replica takes `-overwrite` and `AFNI_DECONFLICT` (also as
  `-DAFNI_DECONFLICT=...`) in both modes, with AFNI's messages: AFNI-11 applies only where AFNI
  would not overwrite, rename or stop; `-verbose`'s misleading name is reproduced in
  `reproduce` only. The clean-room original does all of this too since 2026-10-12 (completing
  3dTshift, part 2b; `larmorx_afni::settings`, `larmorx_afni::cli::tshift`, from spec §6.3 and
  §7.1): `errors/existing-output` and `environment/deconflict-*` give AFNI's exit status and
  messages in its record.
- **D13 / gap:** done (item 5, decided: a switch for failures too).

### AFNI-12 † Output names are refused — (e)
- **What:** `THD_filename_ok` (`thd_filestuff.c:638`) refuses a `-prefix` with control
  characters, white space, shell metacharacters or bytes above 127 anywhere in the path, unless
  `AFNI_ALLOW_ARBITRARY_FILENAMES` is yes (`:647`; corrected 2026-10-12: even then an empty
  name is refused, `:643`, and at the second check below a name that begins with `-`,
  `thd_niftiwrite.c:49`; verified: `-d/out.nii` gives "Illegal filename for NIfTI output",
  nothing written, exit 0).
- **Evidence:** read: `3dTshift.c:391-392`; verified (2026-10-12): "** FATAL ERROR: illegal
  value 'a b.nii' after -prefix", exit 1, also for a blank in a directory name. Each of the 127
  ASCII characters and a non-ASCII one tried in a name: refused exactly as `THD_filename_ok`
  says (controls, DEL, white space, `! " & ' ( ) * ; < > ? [ ] { } |`, bytes above 127).
- **A second check, when the file is written** (found 2026-10-12): `THD_filename_ok` accepts
  any name of more than 6 characters that begins with `3dcalc` (`thd_filestuff.c:644-646`), so
  `-prefix '3dcalc a b.nii'` passes the option scan; `THD_write_nifti` checks again
  (`thd_niftiwrite.c:49-53`) the name `THD_write_3dim_dataset` builds, `DSET_DIRNAME` and the
  prefix (`./3dcalc a b.nii`, `thd_writedset.c:186-189`), which no longer begins with `3dcalc`,
  and a name that begins with `-` (a directory `-d/`): "** ERROR: Illegal filename for NIfTI
  output: ./3dcalc a b.nii", nothing written, exit 0. The existing-file check
  (`thd_writedset.c:149-162`) comes first. Verified with the oracle; `3dcalc/x y.nii` passes
  both and is written.
- **fMRIPrep:** outputs in directories with blanks or accents.
- **larmorx:** both implementations accept any path by default; the replica names AFNI-12 when
  the name is one AFNI refuses, and with the switch (2026-10-12) refuses it with AFNI's message
  (`errors/prefix-with-blank`; AFNI's environment variables unset), at both checks since
  2026-10-12 (`larmorx_gpl_afni::cli::tshift::output_name_ok`; tested against the oracle in
  `tests/oracle.rs`); by default it writes such a name and names AFNI-12. Since 2026-10-12 the
  replica reads `AFNI_ALLOW_ARBITRARY_FILENAMES` (also as `-D`) in both modes: with it yes,
  `reproduce` refuses only what AFNI still refuses (`global/D-arbitrary-filenames`,
  `environment/arbitrary-filenames`). The clean-room original does the same since 2026-10-12
  (completing 3dTshift, part 2b; `larmorx_afni::output`, from spec §6.3 and §7.1), at both
  checks.
- **D13 / gap:** done (item 5).

### AFNI-13 † "ERROR", not "FATAL ERROR" — (f)
- **What:** an unknown `-tpattern` name is reported by `ERROR_message`, then 3dTshift exits 1
  (`thd_timeof.c:186-188`, `3dTshift.c:422`).
- **Evidence:** read; validated (`errors/unknown-pattern`).
- **fMRIPrep:** none.
- **larmorx:** both print AFNI's text and exit 1 (the original "FATAL ERROR" until
  2026-10-12, when it took AFNI's messages, part 2b).
- **D13 / gap:** not D13.

### AFNI-14 † "changing default", then fatal — (f)
- **What:** `-no_detrend` while the method is Fourier prints "found -no_detrend, changing
  default to -heptic" as a fatal error and exits.
- **Evidence:** validated (`errors/no-detrend-fourier`).
- **fMRIPrep:** none.
- **larmorx:** both print AFNI's text (the original explained the order needed until
  2026-10-12, when it took AFNI's messages, part 2b).
- **D13 / gap:** not D13.

### AFNI-16 † A nearly plumb sform is written plumb — (c; c? until 2026-10-12)
- **What:** when a dataset is written, `THD_set_dataset_attributes` (`thd_dsetatr.c:167-180`)
  replaces `ijk_to_dicom_real` by the cardinal matrix of the axes (`THD_dicom_card_xform`,
  `thd_coords.c:567`) whenever `THD_compute_oblique_angle` (`thd_coords.c:717`) returns 0: an
  angle of at most 0.01°, computed from `acos` of a float. As the cosine is a float, the
  effective threshold is the angle whose cosine rounds to 1. Exactly (corrected 2026-10-12):
  the cosine of each column is its largest entry over its length, the length the float32
  square root of a float32 sum of float32 squares, so the threshold depends on where the step
  lies between powers of 2: from 0.0099° (a step just below a power of 2) to 0.0242° (a step
  that is one), about 0.014° for most steps (not "about 0.014°" always, as this entry said
  first). Such an input's obliquity is discarded: its sform and qform come out exactly plumb,
  with the axes' steps and origins. Deliberate in AFNI (it avoids calling float noise
  oblique); a defect for larmorx because it moves the grid by up to 0.024° (0.04 mm at 100 mm
  from the origin).
- **Evidence:** read; verified (2026-10-12: an sform 0.006° from plumb comes out plumb, its
  entries up to 4e-4 from the input's; one 0.017° from plumb is kept). The float32 rule
  predicts AFNI's choice on 240 random matrices 0.004° to 0.03° from plumb (sform and qform,
  six orientations) and on 16 matrices at the float32 boundary for steps of 0.8 to 4 mm
  (`specs/3dTshift.md` §6.1).
- **fMRIPrep:** inputs with tiny rotations in their header (scanner rounding); 3dTshift's output
  geometry then differs from its input's by that much.
- **larmorx:** decided by the user (2026-10-12): keep the input's matrix by default, warn
  when AFNI would have written the plumb one, reproduce it on request. The replica does so
  since 2026-10-12 (`larmorx_gpl_afni::axes::snaps`, `written`): a matrix AFNI calls plumb but
  with a non-zero entry off its axes' rows is written as AFNI writes an oblique one; a matrix
  without one is written from its axes in both modes; AFNI-16 is named in both modes when it
  applies; `reproduce` gives AFNI's header byte for byte (`geometry/slightly-oblique`,
  `-qform`, `-metre`). The clean-room original does the same since 2026-10-12 (completing
  3dTshift, part 2b; `larmorx_afni::geometry::plumb_test`, `output_geometry`, from spec §6.1,
  which describes the test, the cardinal matrix and every header field); until then it kept
  the input's matrix in both modes.
- **D13 / gap:** done.

### AFNI-17 † The slice fields of a copy describe its times within 3 ms — (d)
- **What:** `get_slice_timing_pattern` and `populate_nifti_image` (`thd_niftiwrite.c:519-594`)
  compare slice times within `min_timing_diff`, 0.003 (`:19`), whatever the TR. The range of
  slices described starts at the first time away from 0 by 3 ms or more, widened by one slice
  (`:529-576`), so with slices less than 3 ms apart a time-0 slice can drop out of the range:
  header code 1 over slices 0 … 4, 2 ms apart, TR 5 ms (a copy) is written with `slice_start`
  1, `slice_end` 4, code 1, 2 ms, which describes slices 1 … 4 at 0 … 6 ms instead of 2 … 8 ms.
  With such spacings other codes can come out as other patterns (code 4 over 7 slices 0.52 ms
  apart: code 1 over slices 0 … 1, 1.55 ms). Within AFNI's own 3 ms resolution the times are
  right; beyond it they are not. AFNI-08 is the case where every time is within 3 ms of 0.
- **Evidence:** read; verified (2026-10-12): the rule of `specs/3dTshift.md` §6.2 gives AFNI's
  four fields on 150 random header timings, 28 of them different from the header's.
- **fMRIPrep:** none (slice spacings under 3 ms and a TR below the slice times).
- **larmorx:** both implementations reproduce it in both modes: the replica by its translation
  of the two functions, the clean-room original by spec §6.2 since 2026-10-12 (completing
  3dTshift, part 2b; `larmorx_afni::timing::copy_slice_fields`; until then it wrote the
  header's fields back).
- **D13 / gap:** an imprecision, not D13; no gap.

### AFNI-18 † An input name with a blank is read as a list of datasets — (e)
- **What:** `THD_open_dataset` (`thd_mastery.c:89-100`) hands a name that contains a blank,
  and none of `[count `, `[count_afni `, `[1dcat `, to `THD_open_tcat` unless
  `AFNI_PATH_SPACES_OK` is yes. `THD_open_tcat` (`thd_opentcat.c:85`, `:126`) splits it at
  blanks and `~` (`NI_decode_string_list`) and opens each part as a dataset, to join them in
  time. So an input in a directory whose name has a blank (`/data/my study/bold.nii`) cannot
  be opened: `*+ WARNING: THD_open_tcat: can't open dataset /data/my` and `... study/bold.nii`,
  then `** FATAL ERROR: Can't open input dataset '/data/my study/bold.nii'`, exit 1.
- **Evidence:** read; verified 2026-10-12 (`environment/input-with-blank`; with the variable,
  `environment/path-spaces-ok`).
- **fMRIPrep:** a BIDS dataset or working directory whose path has a blank: the slice-timing
  step fails (fMRIPrep passes absolute paths and does not set the variable; not run with
  fMRIPrep itself).
- **larmorx:** the replica (2026-10-12) reads the name as one file by default and names
  AFNI-18 when it has a blank and `AFNI_PATH_SPACES_OK` is not yes; `reproduce` reads a list
  as AFNI does (`larmorx_gpl_afni::cli::tshift::open_dataset`, `open_list`): one dataset is
  that dataset, several are not supported (an error; AFNI joins them in time).
  `lx.afni.tshift` runs the replica with the variable set, so its temporary paths are one file
  each. The clean-room original does the same since 2026-10-12 (completing 3dTshift, part 2b;
  `larmorx_afni::input`, from spec §7.1); until then it read one file in both modes.
- **D13 / gap:** done.

### AFNI-19 † `AFNI_ALLOW_MILLISECONDS` writes the time offset in milliseconds — (c)
- **What:** with the variable yes, `THD_write_3dim_dataset` skips `DSET_UNMSEC`
  (`thd_writedset.c:134`), which otherwise converts a millisecond time axis to seconds before
  writing. `populate_nifti_image` then converts the TR (`thd_niftiwrite.c:466`) but not the
  time origin (`:513`, `toffset = DSET_TIMEORIGIN`), and labels both seconds. 3dTshift's
  output with `-TR 2000ms` gets TR 2 s and `toffset` 750 "s" where 0.75 is meant.
- **Evidence:** read; verified 2026-10-12 (`environment/allow-milliseconds`). No effect on an
  input whose header is in milliseconds, nor on copies (their axis is converted when read).
- **fMRIPrep:** none (it passes `-TR` in seconds and does not set the variable).
- **larmorx:** the replica (2026-10-12) writes `toffset` in seconds by default and names
  AFNI-19 when the two differ; `reproduce` writes AFNI's (`larmorx_gpl_afni::dataset::write`).
  The parity's fix pass shows it. The clean-room original does the same since 2026-10-12
  (completing 3dTshift, part 2b; `larmorx_afni::cli::tshift`, from spec §7.1).
- **D13 / gap:** done.

### AFNI-20 † A dataset of one row is written as a 1D text file — (c)
- **What:** `THD_write_3dim_dataset` writes a dataset with one voxel along y and z as a 1D text
  file (`THD_write_1D`) under the output's name, unless `AFNI_WRITE_1D_AS_PREFIX` is yes
  (`thd_writedset.c:116-121`). `3dTshift -prefix out.nii` on a 64 × 1 × 1 × 100 series writes
  a text file named `out.nii`, which no NIfTI reader opens, with a random dataset identifier in
  its header.
- **Evidence:** read; verified 2026-10-12 (`environment/one-row-dataset`; with the variable,
  `environment/write-1D-as-prefix` writes NIfTI).
- **fMRIPrep:** none (BOLD series have more than one row).
- **larmorx:** both implementations write NIfTI in both modes and name AFNI-20 (as corrected,
  in both modes) when AFNI would have written text (the original since 2026-10-12, part 2b).
  Not reproduced: the random identifier would make the file differ from run to run. An
  expected divergence in both records.
- **D13 / gap:** none: AFNI's output is not deterministic (class (b) in that part), so the
  switch reproduces only the decision, by naming it.

### AFNI-21 † Warning and error labels in terminal colours, also in files — (f)
- **What:** `output_message` (`debugtrace.c:131-153`) writes the labels of warnings and
  errors between the escape codes `ESC[7m` and `ESC[0m` (inverse video) whether standard error
  is a terminal, a file or a pipe, unless `AFNI_MESSAGE_COLORIZE` starts with `N` or `n`. Logs
  of AFNI programs hold the raw codes.
- **Evidence:** read; verified 2026-10-12 (every message check of the `global` and
  `environment` categories compares them byte for byte).
- **fMRIPrep:** its log files of 3dTshift runs hold the codes (cosmetic).
- **larmorx:** both implementations write AFNI's codes in `reproduce` only; by default plain
  labels (`larmorx_gpl_afni::message::Style`; the original since 2026-10-12, part 2b,
  `larmorx_afni::msg`). Nothing is named (messages only).
- **D13 / gap:** not D13.

### AFNI-22 † `-Dname=value` with blanks, or a blank around `=`, sets another value — (e)
- **What:** `AFNI_prefilter_args` (`afni_environ.c:515-517`) hands the text after `-D` to
  `AFNI_setenv` (`:298-327`), which reads it with `sscanf("%255s %1023s")` as two words, makes
  the first `=` a blank and reads again when either is missing, and `putenv`s
  `word1=word2`: `-DX=a b` sets `X` to `a=b`, `-DX= YES` and `-DX =YES` set `X` to `=YES`,
  `-DX=` and `-D=YES` set nothing (silently), and a name over 255 or a value over 1023
  characters is cut. (`-DAFNI_DEBUG=...` also turns on AFNI's tracing, `:317-323`.)
- **Evidence:** read; verified 2026-10-12 (`global/D-message-prefix-with-blank`,
  `global/D-blank-before-value`, `global/D-empty-value`, `global/D-empty-name`,
  `global/D-value-with-blank`).
- **fMRIPrep:** none (it passes no `-D`).
- **larmorx:** the replica (2026-10-12) reads the name before the first `=` and the value
  after it as written, and names AFNI-22 when AFNI's reading would change what a variable
  3dTshift reads does (a different yes or no, or a different value; `environ::same_effect`);
  `reproduce` reads as AFNI does (`environ::afni_setenv`). A `-D` that sets nothing is
  dropped in both modes, with a warning by default. The clean-room original does the same
  since 2026-10-12 (completing 3dTshift, part 2b; `larmorx_afni::settings`, from spec §7.1).
- **D13 / gap:** done.

### AFNI-23 † `AFNI_NIFTI_SCALE=NO` keeps the slope and drops the intercept — (c; c? until 2026-10-12)
- **What:** with the variable starting with `N` or `n` (`thd_niftiread.c:149-150`), uint8 and
  int16 data with both a slope and an intercept are no longer converted to float32; they
  stay integers with the slope as AFNI's scale factor (`:720-722`) and the intercept is
  dropped (AFNI's scale factors have no offset; `THD_load_nifti` scales only float bricks,
  `:1068-1088`). The values 3dTshift shifts, and writes, are then the true values minus the
  intercept: neither the stored integers nor the scaled values.
- **What the variable is for:** it is in no AFNI documentation (`README.environment`) and
  appears nowhere else in the source. It sets `THD_open_nifti`'s `scale_data` ("convert to
  float and apply `scl_slope`/`scl_inter`") to 0. Until AFNI 20.3 (checked in
  `AFNI_20.3.03`'s and `AFNI_16.0.00`'s `thd_niftiread.c`, the same code) no scale factor was set
  when a file was opened, so the variable made uint8 and int16 data be read as stored, slope
  and intercept both ignored: "do not scale". The changes of 26 January 2021 ("do not convert
  NIFTI scaled shorts to float": a slope alone becomes the brick factor) and 8 March 2021 (the
  factor set at open time) made AFNI keep scaled integers as integers wherever a brick factor
  can carry the scaling. Since then the variable no longer stops the slope (for data with a
  slope alone it changes nothing), and its only remaining effect is to keep integers with an
  intercept as integers, which loses the intercept. Neither "do not scale" nor "keep the
  integers" asks for values off by the intercept: the interaction of the two changes is the
  defect.
- **Evidence:** read; verified 2026-10-12 (`environment/nifti-scale-no`, `-copy`, `-rlt`,
  `-rlt-ignore`, `global/D-nifti-scale-no`; by hand on int16 and uint8 inputs, slope 1
  included, every restore mode, `-ignore`, `-no_detrend`, `-tpattern zero`: the values off by
  the intercept to within half a stored unit, except where `-rlt` removes it with the trend;
  int32 and float32 unchanged).
- **fMRIPrep:** none (it does not set the variable).
- **larmorx:** decided by the user (2026-10-12): corrected by default, the values kept right.
  The variable's remaining purpose, integer data kept as integers, is kept too: the replica
  (`dataset::DroppedIntercept`, `dataset::write`, `cli::tshift::rlt_dropped_intercept`) reads
  the data as AFNI does and writes AFNI's stored values and `scl_slope` (1 where AFNI writes 0)
  with the input's intercept as `scl_inter`, so the output reads back as the run without the
  variable, to within half a stored unit (at most 0.185, 0.25 and 0.5 on the inputs above).
  `-rlt` without `-ignore` leaves nothing to correct (AFNI's output is right); with `-ignore`
  the ignored volumes need the intercept and the others do not, so the data are read as
  without the variable (byte for byte the run without it). Named in both modes where the two
  outputs differ; `reproduce` gives AFNI's bytes. Applying both slope and intercept in every
  case (float32 output, the run without the variable) was the alternative; it would make the
  variable do nothing at all. The clean-room original does the same since 2026-10-12
  (completing 3dTshift, part 2b; `larmorx_afni::dataset`, `larmorx_afni::cli::tshift`, from
  spec §7.1, which describes the behaviour and the correction).
- **D13 / gap:** done.

### AFNI-24 † NIfTI quaternions depend on the build's `long double` — (d)
- **What:** `nifti_dmat44_to_quatern` and `nifti_quatern_to_dmat44`
  (`nifti/nifti2/nifti2_io.c:1619-1664`, `:1757-1870`) and `nifti_dmat33_inverse` (`:2306`)
  mix `long double` constants into double formulas (`0.25l * (r32-r23) / a`,
  `r11 + r22 + r33 + 1.0l`, `1.0l - (b*b+c*c+d*d)`, `2.0l * (b*c-a*d) * yd`, `1.0l / deti`).
  On x86-64 Linux these are x87 operations rounded to 64 bits, then to double: the last bit
  of about one quotient in 4,000 differs from a double division. On aarch64 Linux
  (`long double` 128-bit) and Apple silicon (`long double` = double) AFNI's results differ
  from x86-64's there. NIfTI-2 output shows it (the quaternion parameters are written in
  double); NIfTI-1's float fields practically never.
- **Evidence:** read; verified 2026-10-12: a NIfTI-2 output of an oblique input differed from
  AFNI's in the last bit of `quatern_c` until the replica rounded the same way; since then 120
  random oblique inputs (NIfTI-1 and NIfTI-2) and 160 qform-only, near-180° and trace-near-0
  ones give AFNI's header byte for byte. The emulation gives GCC's double for 5 million random
  operations.
- **fMRIPrep:** none (NIfTI-1, and x86-64 builds only).
- **larmorx:** both implementations give x86-64's results on every platform
  (`larmorx_gpl_afni::x87`, 2026-10-12; the original's own emulation `larmorx_afni::x87`,
  part 2b the same day, matched GCC's `long double` on 6 million random operations).
- **D13 / gap:** an imprecision, not D13 (rule 5, D11).

### AFNI-25 † `-ignore` as the last argument crashes — (a)
- **What:** `-ignore` reads its value as `strtod(argv[++nopt],NULL)` without checking `argc`
  (`3dTshift.c:315-318`), unlike `-TR`, `-tzero`, `-slice`, `-prefix` and `-tpattern`, which
  stop with "needs an argument!". As the last argument it passes `argv[argc]`, a null pointer,
  to `strtod`: a segmentation fault. AFNI's signal handler prints "Fatal Signal 11 (SIGSEGV)
  received", its stack and a crash banner with the build's compile date and binary name, appends
  them to `~/.afni.crashlog`, and exits 1. `-voxshift` has the same missing check
  (`:299`): `THD_open_dataset(NULL)` fails and the message prints the null pointer as
  `'(null)'` (glibc's `printf`): "Can't open -voxshift dataset '(null)'", exit 1.
- **Evidence:** read; verified 2026-10-12 with the oracle (`3dTshift -prefix o.nii -ignore`,
  `3dTshift -ignore`, `3dTshift -voxshift`); first seen by the clean-room implementer (spec
  §7.1, observed).
- **fMRIPrep:** none (it always gives `-ignore` a value).
- **larmorx:** both implementations stop with "-ignore needs an argument!" and exit 1 in both
  modes, as for the other options; the exit status is AFNI's. Not reproduced: the crash banner
  (build-specific text) and the line appended to the crash log in the home directory.
  `-voxshift` is not supported by either implementation (an expected divergence in both
  records), so its `'(null)'` message does not arise.
- **D13 / gap:** never reproduced (a null-pointer read). No gap.

### AFNI-26 † A failed write exits 0 — (e)
- **What:** 3dTshift ignores `DSET_write`'s result (`3dTshift.c:696`, `:709`) and exits 0.
  When a NIfTI output cannot be created (a missing or read-only directory),
  `nifti_image_write_engine` prints "** ERROR (nifti_image_write_engine): cannot open output
  file 'NAME'" (`nifti/nifti2/nifti2_io.c:7908-7912`, the NIfTI library's own message, never in
  colour, AFNI-21) and returns 1; `THD_write_nifti` returns it (`thd_niftiwrite.c:126-127`),
  `THD_write_3dim_dataset` returns False (`thd_writedset.c:238-240`), and nothing reads it.
  With `-verbose` AFNI then prints `++ Wrote output: NAME` of a file it did not write, as in
  AFNI-11. Shifts and copies alike. AFNI's own format (`-prefix sub/out`) is not affected:
  its writer creates the missing directory.
- **Evidence:** read; verified 2026-10-12 with the oracle (`-prefix nodir/out.nii`, `.nii.gz`,
  a copy, a read-only directory, `-verbose`); first seen by the clean-room implementer (spec
  §7.1, "A failed write", observed).
- **fMRIPrep:** none (its working directories exist). Were the write to fail, the node would
  still fail: Nipype checks that the output exists (`out_file`, `exists=True`,
  `nipype/interfaces/afni/base.py:145`; `FileNotFoundError` in `aggregate_outputs`,
  `nipype/interfaces/base/core.py:452-460`).
- **More cases** (found by the clean-room implementer's black-box runs, 2026-10-12; read
  since): a read-only file with `-overwrite` or `AFNI_DECONFLICT=O...`, a directory of the
  output's name (AFNI's existing-output tests are `THD_is_file`, a regular file:
  `thd_opendset.c:479`, `:493`, `3dTshift.c:539`, so a directory is no conflict, and renaming
  can choose a directory's name), a renamed output that cannot be written. A one-row dataset
  (AFNI-20) fails silently: AFNI's 1D text writer prints nothing.
- **larmorx:** both implementations check, in both modes and changing nothing on disk, whether
  AFNI's writer could open the output (a directory of that name; an existing file opened for
  writing without truncation; a new one created and removed; only refusals count: not found,
  not a directory, a directory, no permission, a read-only file system, and on Windows not a
  file another program holds open, which the write waits for): the replica's
  `larmorx_gpl_afni::cli::output::cannot_open`, the original's `larmorx_afni::output::cannot_open`
  (2026-10-12). By default: an error, exit 1, naming AFNI-26 (the replica's default refused a
  read-only file only where its rename failed until the same day: on Windows, after 2 s). With
  the switch: the NIfTI library's message with the name AFNI gives (none for a one-row
  dataset), nothing written, `-verbose`'s line, exit 0: AFNI's exit status and messages byte
  for byte on missing and read-only directories, read-only files with `-overwrite` and
  `AFNI_DECONFLICT`, a directory of the output's name, renaming onto one, `.nii` and `.nii.gz`,
  shifts and copies, relative, `./` and absolute names, `-verbose` (`errors/missing-directory*`
  in both records; the rest by hand). A write that fails after the file is opened (a full
  disk) stays an error in both modes: AFNI's messages then depend on where it failed, and it
  may leave part of a file. Not AFNI-26 but larmorx's own difference: an existing writable
  output in a directory that cannot be written, with `-overwrite`, which AFNI rewrites in
  place and larmorx cannot (its temporary file), an error in both modes.
- **D13 / gap:** done (item 5, decided: a switch for failures too).

### AFNI-15 † The build races under `make -j` — (b)
- **What:** AFNI's niml sub-make races in a parallel build; FSL's own `ld` on `PATH` also breaks
  the build.
- **Evidence:** verified (`scripts/build_afni_oracle.sh`).
- **fMRIPrep:** none.
- **larmorx:** the oracle script works around both.
- **D13 / gap:** no larmorx code. No gap.

## Restricted: AFNI 3dAutomask and 3dUnifize (source-derived)

> **Clean-room implementers: do not read this section.** Written from AFNI 25.2.09's source
> (NIH and MCW files), through [afni-automask.md](afni-automask.md) and
> [afni-unifize.md](afni-unifize.md). The clean-room originals' implementers work from
> `specs/3dAutomask.md` and `specs/3dUnifize.md`.

### AFNI-40 † An option's missing value crashes — (a)
- **What:** 3dAutomask's `-peels`, `-nbhrs`, `-clfrac`/`-mfrac`, `-SI`, `-dilate` and `-erode`
  read `argv[++iarg]` without checking it exists (`3dAutomask.c:139-245`), and so does
  3dUnifize's `-T2up` (`3dUnifize.c:1036`; its other options check). As the last argument,
  `strtod(NULL)`/`strtol(NULL)` crash: signal 11, AFNI's crash banner, exit 1. 3dAutomask's
  `-prefix`, `-apply_prefix` and `-depth` pass the NULL to `THD_filename_ok` instead: "-prefix
  (null) is illegal!", exit 1 (reproduced as is).
- **Evidence:** read; verified with the oracle for each option (2026-10-23); parity cases
  `errors/missing-value-*` (3dAutomask) and `errors/T2up-missing` (3dUnifize): AFNI crashes,
  the replica refuses, the messages before the crash compared.
- **fMRIPrep:** none.
- **larmorx:** the replica stops with one fatal line naming the option and AFNI-40, exit 1, in
  both modes, after the messages AFNI prints before the crash. Not reproduced: the crash banner.
- **D13 / gap:** never reproduced. No gap.

### AFNI-41 † `-nbhrs` outside 6..26 is silently 17 — (e)
- **What:** `-nbhrs` sets `nbhrs` directly; the range check reads `new_nbhrs`, which nothing
  sets, so it never runs (`3dAutomask.c:203-206`, `:276-283`); `THD_automask_set_peelcounts`
  replaces values outside 6..26 by 17 (`thd_automask.c:42-46`), and the peeling uses
  `MIN(18, peelthr)` (`:1335`), so 19..26 act as 18. The help: "should be between 6 and 26".
- **Evidence:** read; verified (2026-10-23; parity cases `options/nbhrs-*`: `nbhrs-5`, `-27`,
  `-0`, `-40` and `nbhrs-40-then-NN1`, each refused by default and bit-identical to AFNI with
  `reproduce`).
- **fMRIPrep:** none (it never passes `-nbhrs`).
- **larmorx:** refused by default (2026-10-23, rule B): after the option scan, a last `-nbhrs`
  value outside 6..26 is a fatal error naming the entry (exit 1, nothing read or written),
  whether or not `-NN1/2/3` then override it (AFNI's dead check meant to refuse it); `reproduce`
  keeps AFNI's behaviour (`THD_automask_set_peelcounts`' 17, the peeling's `MIN(18, ·)`), with a
  notice (`crates-gpl/larmorx-gpl-afni/src/cli/automask.rs`, `parse` and `run`).
- **D13 / gap:** done in the replica. The clean-room original follows from the spec (section 8
  of this handoff gives the behaviour to add).

### AFNI-42 † `-NN1/2/3` change the peeling threshold — (c?)
- **What:** `-NN1`, `-NN2`, `-NN3` also set `nbhrs` to 6, 18 and 26 (`3dAutomask.c:262-273`,
  "keep historic default of 17 for NN2 unless changed via NNx or nbhrs options"), overriding
  any `-nbhrs`. The help says "the default peeling processes still use NN2", true of the
  neighbourhood only: an explicit `-NN2` peels with 18 of 18 neighbours where the default peels
  with 17, and `-NN1` with 6.
- **Evidence:** read; verified (parity cases `options/NN*-dilate-erode-depth`,
  `options/nbhrs-then-NN2`).
- **fMRIPrep:** none.
- **larmorx:** AFNI's behaviour in both modes.
- **D13 / gap:** decided 2026-10-23 (decision log): rule C: AFNI's behaviour kept in both modes and documented (AFNI-44 shapes fMRIPrep's BOLD masks). Was: decide.

### AFNI-43 † `-peels 0` peels once — (c)
- **What:** `THD_automask_set_peelcounts`: `peelcount = (p > 0) ? p : 1`
  (`thd_automask.c:44`), so `-peels 0` peels once; the help: "Turn off by setting to 0".
- **Evidence:** read; verified (AFNI's `-peels 0` mask equals its `-peels 1` mask on fMRIPrep's
  BOLD reference of ds005040; parity case `defects/peels-0`; the site's worked example).
- **fMRIPrep:** none (it never passes `-peels`).
- **larmorx:** corrected by default: no peeling, named when the mask changes; `reproduce`
  peels once (`larmorx_gpl_afni::mask::MaskSettings::set_peelcounts`). The GPL replica of
  3dAutomask also names AFNI-43 where the mask is the same but the clustering messages differ
  (random testing, `afni-automask:20261025:118`).
- **D13 / gap:** done.

### AFNI-44 † The large-hole filling depends on the number of slices — (c?)
- **What:** the large-hole step runs when `nmm > 1 || jj > 0` (`thd_automask.c:388-393`), `jj`
  being the last value assigned to it, `rint(0.016 nz)`. With no axis of 94 voxels or more, a
  volume of 32 to 93 slices runs `THD_mask_fillin_completely(1)` (the small holes, until none is
  left) and one with fewer than 32 does not. Whether `jj > 0` was meant is not clear.
- **Evidence:** read. The parity cases meet both sides (the 40-slice phantoms and fMRIPrep's
  BOLD references, which run the step; the 2-slice and 2 × 2 × 2 inputs, which do not), all
  bit-identical to AFNI; no case isolates the step's effect.
- **fMRIPrep:** its BOLD references (most have 32 to 93 slices): AFNI's behaviour is what
  fMRIPrep gets.
- **larmorx:** AFNI's behaviour in both modes.
- **D13 / gap:** decided 2026-10-23 (decision log): rule C: AFNI's behaviour kept in both modes and documented (AFNI-44 shapes fMRIPrep's BOLD masks). Was: decide (probably keep: changing it changes fMRIPrep's masks).

### AFNI-45 † `-T2up` near 100 reads past the sorted values — (a)
- **What:** `mri_invertcontrast_inplace` (`mri_invert.c:44`): `ii = (int)rintf(nhist * uperc *
  0.01f); ucut = hist[ii]` reads `hist[nhist]` when the index rounds to `nhist`: at `-T2up 100`
  always, and for `u` near 100 when `nhist (1 − u/100) < 0.5`. `hist` is a `malloc` of `nvox`
  floats: what it holds there is memory never written (past the allocation when the whole image
  is in the mask). In AFNI's runs it read 0, which inverts every value to 0: the output is all
  zeros.
- **Evidence:** read; verified (2026-10-23; parity cases `defects/T2up-100`,
  `defects/T2up-99.99`; the site's worked example on fMRIPrep's BOLD reference of ds005040).
- **fMRIPrep:** none (it uses the default 98.5).
- **larmorx:** by default the 100th percentile (the largest sorted value), named;
  `reproduce` fills the read with 0 (D13's deterministic fill), named
  (`larmorx_gpl_afni::unifize::invert_contrast`).
- **D13 / gap:** done.

### AFNI-46 † `-T2` on a small 2D image crashes — (a)
- **What:** with `-T2`, the automask of a 2D image smaller than 16 × 16 is NULL
  (`thd_automask.c:2104`); `mri_invertcontrast_inplace` computes it again and dereferences it
  (`3dUnifize.c:1105-1106`, `mri_invert.c:28-34`): signal 11, exit 1.
- **Evidence:** read; verified (parity case `errors/slice-small-T2`).
- **fMRIPrep:** none.
- **larmorx:** an error (exit 1) in both modes, after AFNI's messages.
- **D13 / gap:** never reproduced. No gap.

### AFNI-47 † A thin volume of more than a million voxels reads past its scale image — (a)
- **What:** above 1,000,000 voxels the local white-matter level is computed on a half-size copy
  (`3dUnifize.c:557`). Halving keeps at least one voxel per axis and doubling keeps an axis of 1
  at 1 (`:166-168`, `:227-229`), so an axis of 2 or 3 voxels comes back as 1: the scale image is
  smaller than the input, and `mri_WMunifize` reads it with the input's indices, past its end
  (`:570-576`); `-ssave` writes it the same way.
- **Evidence:** read; verified on a 720 × 720 × 2 phantom (parity case `defects/slab`): AFNI
  crashed (signal 11) reading past the scale image in every run of 2026-10-23; the case
  compares its messages before the crash and is an expected divergence (with other memory
  AFNI would instead multiply the second slice by whatever it reads).
- **fMRIPrep:** none.
- **larmorx:** by default the half-size image is brought back to the input's grid, named;
  `reproduce` reads AFNI's smaller image linearly with 0 past its end, named
  (`larmorx_gpl_afni::unifize::double_up`).
- **D13 / gap:** done.

### AFNI-48 † `-T2` three times leaves the output inverted — (e)
- **What:** `do_T2 == 1` inverts back, `do_T2 == 2` clips the bright edges instead
  (`3dUnifize.c:1193-1198`); a third `-T2` does neither. The help documents once and twice.
- **Evidence:** read; verified (parity cases `options/T2-T2-T2` and `T2-x4`, refused by
  default and bit-identical to AFNI with `reproduce`; `T2-x3-EPI` runs in both modes, as `-EPI`
  sets `do_T2` itself and its path does not use it).
- **fMRIPrep:** none.
- **larmorx:** refused by default (2026-10-23, rule B): `do_T2 >= 3` without `-EPI` is a fatal
  error naming the entry, before the input is read; `reproduce` keeps AFNI's behaviour (neither
  `do_T2 == 1`'s inversion back nor `do_T2 == 2`'s clipping), with a notice
  (`crates-gpl/larmorx-gpl-afni/src/cli/unifize.rs`).
- **D13 / gap:** done in the replica. The clean-room original follows from the spec.

### AFNI-49 † `-clfrac`'s default in 3dUnifize's help — (f)
- **What:** the help gives the default as 0.1 and then says it was raised to 0.2 in 2016
  (`3dUnifize.c:818-822`); the code uses 0.2 (`:659`).
- **Evidence:** read.
- **fMRIPrep:** none (it passes 0.2).
- **larmorx:** 0.2; `docs/api/afni-unifize.md` says so.
- **D13 / gap:** not D13.

### AFNI-75 † `-EPI`'s ball of neighbours grows without bound on tiny voxels — (c)
- **What:** with `-EPI`, 3dUnifize sets the radius of its local-percentile ball to
  `18.3 / cbrt(|dx dy dz|)` voxels, at least 4.14 (`3dUnifize.c:1060-1068`, voxel sizes in mm;
  `-Urad` and `-rbt` are overridden), with no upper bound; `mri_local_percmean` builds the whole
  ball with `MCW_spheremask(1,1,1,radius)` before using it (`:300-302`, `:425-427`;
  `edt_buildmask.c`), looping over a cube of `(2 radius + 1)³` offsets. A header in micrometres
  (`xyzt_units` 3) makes the voxels 1000 times smaller than they look: a radius of about 6100
  voxels, about 10¹² points of 10 bytes, whatever the image's size.
- **Evidence:** measured (differential random testing, 2026-10-25): `afni-unifize:20261025:89`,
  `:118` and `:226` (4D inputs of 25×32×23×6, 15×12×28×4 and 10×30×27×5 voxels in micrometres,
  with `-EPI`): AFNI exceeded the case's 3 GiB limit in the replica's comparison and ran past
  600 s in the original's; the ball needs about 10¹³ bytes (beyond any machine). The replica's
  `sphere(radius)` is AFNI's loop, so its `reproduce` does the same.
- **fMRIPrep:** none.
- **larmorx:** the method is kept (rule C: the radius is AFNI's design for EPI voxels; the
  memory is the work asked for). By default the GPL replica computes the ball's bytes before it
  is built (`larmorx_gpl_afni::unifize::ball_bytes`: `4/3 π r³` points, 24 bytes each while the
  offsets grow, 4 more per worker's buffer) and, when they exceed the machine's physical memory or
  `LARMORX_MEMORY_LIMIT`, stops at once with the GiB needed, the radius and the voxel sizes, and
  the likely cause ("a header whose units are micrometres ..."), naming AFNI-75; every case that
  fits is unchanged, bit for bit. `reproduce` attempts it as AFNI does. Permanent case
  `difftest/epi-ball-beyond-any-machine` (`afni-unifize:20261025:89`) expects the guard's error;
  neither AFNI nor `reproduce` runs on it. Decided 2026-10-25 by the rules, as ANTS-162 (the
  orchestrator's decision of that day: a guard wherever a size grows the same way).
- **D13 / gap:** the clean-room original of 3dUnifize (`crates/larmorx-afni`) has no such guard:
  its spec needs the black-box behaviour ("-EPI changes -Urad to N voxels", N growing as the
  voxels shrink; on micrometre voxels AFNI exhausts memory) and the guard, then its agent.

### AFNI-01 †, extended to 3dAutomask and 3dUnifize
- **What:** 3dAutomask averages a series' sub-bricks through `THD_extract_array`
  (`thd_median.c:273-308`), whose positive-factor test (`thd_dsetto1D.c:187-191`) drops a
  negative factor and its magnitude; 3dUnifize's `-EPI` median goes through the same routine
  (`thd_median.c:35-38`) while each volume is scaled with its factor, so a series with a
  negative `scl_slope` unifizes to all zeros.
- **Evidence:** read; verified (parity cases `defects/negative-slope-4d` of 3dAutomask and
  `defects/EPI-negative-slope` of 3dUnifize).
- **larmorx:** both replicas apply the factor by default, named whenever a negative factor is
  met (with the number of mask voxels it changes, in 3dAutomask); `reproduce` drops it.
- **D13 / gap:** done.

## Restricted: FSL mcflirt (source-derived)

> **Clean-room implementers: do not read this section.** Written from FSL 6.0.7.17's source
> (FSL Licence), through [fsl-mcflirt.md](fsl-mcflirt.md); file:line refer to `fsl-mcflirt`
> 2111.0, `fsl-newimage` 2203.12 and `fsl-miscmaths` 2203.2. The clean-room original's
> implementers work from `specs/mcflirt.md`, which describes the behaviours they need.

### FSL-01 † The last volume is not initialised from the previous one — (c)
- **What:** after volume `i`, the next volume's first stage starts from `finalmat` only if
  `i' < no_volumes - 1` (`mcflirt.cc:453-455`): an off-by-one that leaves the last volume of
  the series starting from the identity.
- **Evidence:** read; verified (six identical displaced volumes: matrices 0 and 5 identical,
  1-4 not; with `-fudge` all six identical); spec §5.4.
- **fMRIPrep:** every run's last volume (its first stage starts farther from the answer).
- **larmorx:** reproduces (`larmorx_mri::hmc::estimate::chains`).
- **D13 / gap:** initialise it from its predecessor like the others; warn; switch. **Gap.**

### FSL-02 † The correlation's weight count is inflated — (c)
- **What:** in `p_normcorr_smoothed` (`costfns.cc:1788-1902`, likewise the sinc variant) the
  weight count `num` is added to `numA` after each row and `numA` to `numB` after each slice,
  but neither is reset: the final count is a running sum of running sums, about 10⁴ times the
  true weight sum on a 4 mm grid. In `sumxy/(num-1.0) - (sumx*sumy)/(num*num)` (`:1894`) the
  mean terms vanish, so the "normalised correlation" is effectively uncentred. The unsmoothed
  `p_normcorr` counts correctly.
- **Evidence:** read; verified indirectly (a ramp shifted along x is realigned by default, not
  with `-smooth 0`); spec §6.4.
- **fMRIPrep:** every run (normcorr with smoothing is the default). The same function serves
  FLIRT's `-cost normcorr`.
- **larmorx:** reproduces (`larmorx_mri::hmc::cost`).
- **D13 / gap:** decided 2026-10-23 (decision log): rule C: mcflirt's behaviour kept by default, since correcting it was measured to bring no gain against the ground truth; the corrected count stays available (`normcorr_count="weights"`). Was: **decide** (Decisions, item 4): the fix changes the cost of every run.

### FSL-03 † In-plane mode pads the first stage twice — (c), (f)
- **What:** `main` applies `fix2D` to the 8 mm grid (`mcflirt.cc:693`) and `correct` applies it
  again to its copy (`:411-412`), so the stage-1 reference gets two replicated slices at each
  end, the others one. (f) The mode prints `Params:` and `Tolerances:` to stderr for every
  volume, unconditionally (`:238-244`).
- **Evidence:** read; verified on a 4-slice slab; spec §12.
- **fMRIPrep:** none (thin slabs only).
- **larmorx:** reproduces (`larmorx_mri::hmc::estimate`, `estimate.rs:336`).
- **D13 / gap:** pad once; switch. **Gap.**

### FSL-04 † `-gdt` is broken — (c), (a)
- **What:** test volumes are replaced by their gradient, but the reference's gradient is
  computed into a temporary and discarded (`mcflirt.cc:434-440,700-705,728-734`). The saved
  reference is named `grefvol_` glued to the whole `-out` string, so an absolute `-out` gives an
  invalid name and mcflirt aborts.
- **Evidence:** read; verified (`real/ds000005/opt-gdt`).
- **fMRIPrep:** none.
- **larmorx:** refused by default in both implementations (exit 1, the notice names FSL-04);
  the replica reproduces it with `reproduce` (the gradient images registered to the plain
  reference, `grefvol_` written), bit-identical to mcflirt on a relative `-out`
  (`edge/gdt` of the replica's record).
- **D13 / gap:** done (2026-10-24).

### FSL-05 † Options that do nothing — (e)
- **What:** `-scaling` sets a value nothing reads (`estimate_scaling`, `mcflirt.cc:286-294`, is
  never called); `-hist` sets an unused flag; `eval_costs` (`:556-566`, a hard-coded
  `/usr/people/prb/...` path) is unreachable. `-edge` is not an option at all
  (`Globaloptions.cc`), so the edge-detection branches (`:430-433`, `:695-699`) are dead.
- **Evidence:** read; verified (`-edge` rejected).
- **fMRIPrep:** none.
- **larmorx:** accepts `-scaling` and `-hist` silently; rejects `-edge`, as mcflirt does.
- **D13 / gap:** warn that `-scaling` and `-hist` have no effect; switch (exact silence).
  **Gap.**

### FSL-06 † An unknown `-cost` keeps normcorr — (e)
- **What:** an unknown cost name keeps NormCorr, with a message only when `-report` came earlier
  (`Globaloptions.cc:232-253`).
- **Evidence:** read; verified.
- **fMRIPrep:** none.
- **larmorx:** reproduces (`larmorx_mri::cli::hmc`).
- **D13 / gap:** refuse an unknown cost; switch. **Gap.**

### FSL-07 † Numbers read with `atoi`, `-help` alone exits 1 — (e)
- **What:** `-refvol`, `-stages`, `-dof`, `-bins` and `-fov` are read with `atoi` (`abc` is 0,
  `-fov 10.5` is 10; `Globaloptions.cc:228-231`), `-smooth` and `-rotation` with `atof`; fewer
  than 3 arguments print the usage and exit 1, so `mcflirt -help` exits 1; an unknown option in
  last position is reported as "Lacking argument" (checked first).
- **Evidence:** read: `Globaloptions.cc:77-284`; verified.
- **fMRIPrep:** none.
- **larmorx:** reproduces (`larmorx_mri::cli::hmc`).
- **D13 / gap:** refuse values that are not numbers; `-help` exits 0; switch. **Gap.**

### FSL-08 † `-stats`: end slices and names — (c), (e)
- **What:** (c) the mean, variance and sigma loops run `z = 1 … nz-2` (`mcflirt.cc:569-618`), so
  the first and last slices stay 0. (e) The names are the raw `-out` string plus `_meanvol`
  etc., with the image extension added by the writer: `x.nii.gz_meanvol.nii.gz`; nipype looks
  for `x_variance.nii.gz` (PIPE-15).
- **Evidence:** read; verified.
- **fMRIPrep:** none (not used).
- **larmorx:** corrected by default in both implementations: every slice, and the `-stats` and
  `-meanvol` images named after the stem of `-out` (`x_meanvol.nii.gz`, `x_mean_reg.nii.gz`);
  `reproduce` gives mcflirt's slices and names (`larmorx_mri::hmc::stats::temporal`,
  `larmorx_mri::cli::hmc::write_outputs`; the replica's notice for `-meanvol` alone since
  2026-10-25).
- **D13 / gap:** none.

### FSL-09 † Output types and conversion — (c), (a)
- **What:** the output type comes from `dtype(inputfname)` and `closestTemplatedType`
  (`generalio.cc:525-575`): int8 becomes uint8, so negative values are lost; float values are
  converted with a plain C cast (`newimage.h:884-892`): truncation toward zero, no clamping, so
  values beyond the type's range are undefined.
- **Evidence:** read; verified (int16 output = truncated float32).
- **fMRIPrep:** none (it reads only the matrices).
- **larmorx:** reproduces the types and the truncation, and saturates out-of-range values
  (`larmorx_mri::hmc::image::output_type` and the conversion after it).
- **D13 / gap:** int8 stays int8; values rounded to nearest and clamped (to confirm); switch.
  **Gap.**

### FSL-10 † Units forced to millimetres and seconds — (c)
- **What:** `set_fsl_hdr` (`newimageio.h:331-374`) writes `xyzt_units` mm and s whatever the
  input, while `pixdim[4]` is copied: a TR of 2000 in milliseconds comes out as 2000 s.
- **Evidence:** read; verified (ds003345, units 18).
- **fMRIPrep:** inputs with a TR in milliseconds (the corrected series only; fMRIPrep reads the
  matrices).
- **larmorx:** reproduces (`larmorx_mri::hmc::image`, `image.rs:315`).
- **D13 / gap:** keep the input's time unit; switch. **Gap.**

### FSL-11 † `FSLOUTPUTTYPE` is required — (e)
- **What:** without `FSLOUTPUTTYPE`, mcflirt fails (exit 1) at the first image it saves, after
  writing the `.mat` files (`generalio.cc:150-174`).
- **Evidence:** read; verified.
- **fMRIPrep:** none (nipype sets it).
- **larmorx:** writes `.nii.gz` (expected divergence `no-fsloutputtype`).
- **D13 / gap:** decided 2026-10-23 (decision log): larmorx's behaviour kept (the safer one). Was: **decide** (item 5: no switch proposed).

### FSL-12 † Impossible inputs abort late — (a)
- **What:** a reference with another voxel count aborts in the final resampling ("Attempted to
  copydata with non-matching sizes", `newimage.cc:362-369`) after the whole estimation, before
  any matrix is written; `-stages 0` with `-reffile` never reads the reference and aborts
  (`mcflirt.cc:655-660`); an out-of-range `-refvol` aborts ("Invalid t index").
- **Evidence:** verified.
- **fMRIPrep:** none.
- **larmorx:** refuses before any work (exit 1).
- **D13 / gap:** never reproduced. No gap.

### FSL-13 † `.mat` directory quirks without `-mats` — (e)
- **What:** the reference volume's identity is written whenever `-mats`, `-plots`, `-rmsrel`
  or `-rmsabs` is given; with `-rmsrel` or `-rmsabs` alone the `.mat` directory holds only that
  file; with `-plots` alone no directory exists and the write goes to `"/MAT_xxxx"`, failing
  silently (`mcflirt.cc:519-536`).
- **Evidence:** read; verified (`syn/cli/{rmsrel,plots}-only`).
- **fMRIPrep:** none.
- **larmorx:** reproduces the lone file; writes nothing for `-plots` alone
  (`larmorx_mri::cli::hmc::write_outputs`).
- **D13 / gap:** create the directory only with `-mats` (to confirm); switch. **Gap.**

### FSL-14 † One NaN voxel makes the matrix NaN — (c)
- **What:** the volume's centre of mass (`newimage.cc:1464-1477,1537-1594`) includes NaN
  voxels, so one NaN voxel makes the centre, the volume's matrix, its `.par` line and the `.rms`
  values involving it NaN, and its corrected volume the background value.
- **Evidence:** verified (spec §12).
- **fMRIPrep:** BOLD series with a NaN voxel (rare: most writers have none).
- **larmorx:** reproduces (`larmorx_mri::hmc::rigid` and the centre of mass).
- **D13 / gap:** leave non-finite voxels out of the centre and the cost (to confirm); warn;
  switch. **Gap.**

### FSL-15 † `-init` affects only the final resampling — (c)
- **What:** `initmat` is reset to the identity before the stages (`mcflirt.cc:707`); `-init` is
  read only for the final resampling (`:769-774,787-804`), so the matrices are the same with
  and without it.
- **Evidence:** read; verified.
- **fMRIPrep:** none.
- **larmorx:** reproduces, in both implementations.
- **D13 / gap:** decided 2026-10-24, rule C: mcflirt's help says "initial transform matrix to
  apply to all vols", which is what it does (the final resampling); a pipeline that passes
  `-init` gets the composed resampling it asked for. Kept and documented. Was: **decide**
  whether an "initial transform" should start the search.

### FSL-16 † Rotations are not quite orthogonal — (d)
- **What:** `make_rot` stores `theta = norm2(angl)` in a float while `axis = angl/theta` is
  double (`miscmaths.cc:899-939`), so the axis is not exactly unit length and rotations are
  orthogonal only to about 1e-8 (the in-plane mode's `.par` shows `tz ≈ 7e-6` where the matrix
  has none).
- **Evidence:** read; verified (`syn/motion/small/2d`).
- **fMRIPrep:** every run (negligible).
- **larmorx:** reproduces (to follow mcflirt's search path).
- **D13 / gap:** not D13; a D11 option.

### FSL-17 † The implementation differs from its paper and comments — (f)
- **What:** Jenkinson et al. (2002) blur both images at each scale and optimise with Powell's
  method; mcflirt resamples only the reference, without blurring, and runs one sweep of
  line searches in a function named `powell_opt` with no direction update
  (`mcflirt.cc:199-218`). The tolerance comment says "0.02 scale"; the code says 0.002
  (`:173-188`).
- **Evidence:** read; spec §15.
- **fMRIPrep:** every run (this is mcflirt's algorithm).
- **larmorx:** follows the implementation.
- **D13 / gap:** not D13; better algorithms are D11 options (`docs/api/mri-hmc.md`).

### FSL-18 † Woods' cost squares the bin counts — explained, not a defect
- **What:** the Woods cost sums `Sqr(num[b])*stdev/sum[b]` (`costfns.cc:1456-1561`). Since
  `sum[b]/num[b]` is the bin's mean, that is `n_b σ_b / μ_b`, Woods' criterion: not a defect
  (read while translating the replica, 2026-10-24).
- **Evidence:** read.
- **fMRIPrep:** none (Woods registers no BOLD run, for mcflirt or larmorx).
- **larmorx:** reproduces (both implementations; the replica bit for bit).
- **D13 / gap:** decided 2026-10-24, rule C: not a defect; kept. Was: **decide.**

### FSL-20 † An uncreatable `.mat` directory ends the run with status 0 — (e)
- **What:** `Log::establishDir` (`Log.cc`) makes `<out>.mat` (adding `+` while the name exists);
  `decompose_mats` (`mcflirt.cc:489-494`) prints "Error! Could not create directory ... No
  write permission" for a permission error and goes on, but for any other error (a missing
  parent directory: `-out missing/out -mats`) `establishDir` prints "Error!: Unknown Log
  directory return value" and calls `exit(0)`: no matrices, `.par`, `.rms` or corrected series,
  and success reported.
- **Evidence:** read; verified (`edge/out-dir-missing` of the replica's record).
- **fMRIPrep:** none (its `-out` is in an existing work directory).
- **larmorx:** the replica (2026-10-24): an error, exit 1, naming FSL-20 (rule B: the input
  cannot be honoured, and mcflirt reports success); `reproduce` gives mcflirt's message and exit
  0. The clean-room original: to be specified (`specs/mcflirt.md`, the spec writer).
- **D13 / gap:** done in the replica; the original's is open.

### FSL-21 † Histogram costs with fewer than 8 bins have none — (a)
- **What:** the histogram costs get `no_bins / scaling` bins at each stage (`usrsetscale`,
  `mcflirt.cc`; `Costfn::set_no_bins`, `costfns.cc:3622-3661`), so with `-bins` below 8 the
  first (8 mm) stage has none, and below 4 every stage. With no bins, `mutualinfo`, `normmi`
  and `woods` compute a degenerate cost and the search ends at arbitrary matrices (every
  volume the same, or tens of millimetres away; deterministic); `corratio` folds its last bin
  into index -1 (`numy[no_bins-1] += numy[no_bins]`, `costfns.cc:1269-1274`), before its arrays,
  and glibc aborts when they are freed ("double free or corruption (out)", signal 6) at the
  first cost evaluation.
- **Evidence:** read; verified (`edge/bins4-mutualinfo`, `edge/bins1-normmi`,
  `edge/bins2-corratio` of the replica's record).
- **fMRIPrep:** none (it runs normcorr).
- **larmorx:** the replica (2026-10-24): refused by default before any work (rule B: a
  histogram needs at least one bin; mcflirt silently returns arbitrary matrices or corrupts its
  heap), the notice names FSL-21; `reproduce` gives mcflirt's matrices bit for bit and, for
  `corratio`, its abort (when the out-of-bounds write is non-zero, as at every real
  evaluation). The clean-room original: to be specified.
- **D13 / gap:** done in the replica; the original's is open.
### FSL-22 † The background value sorts NaNs: undefined order — (a)
- **What:** newimage's `calc_bval` (`sort(hist.begin(), hist.end()); return hist[numbins/10]`)
  sorts the 2-voxel border with `operator<`; with NaN voxels that is not a strict weak ordering,
  undefined behaviour in C++. libstdc++'s introsort then leaves a position-dependent order, so the
  pad value of mcflirt's output (and of `affine_transform`'s) is arbitrary.
- **Evidence:** the NaN matrix (`validation/probes/fsl-mcflirt-affine-trace/nan_matrix.py`):
  inputs with NaN background voxels in `reproduce`, 32 of 160 cases, the output images of
  volumes whose matrix is NaN padded with another value.
- **fMRIPrep:** none (its inputs have no NaN).
- **larmorx:** NaNs sorted last in both modes (defined, the same on every platform). By D13's
  rule for undefined behaviour (2026-10-11), `reproduce` reproduces every defined part and warns
  when NaN border voxels make the original's order undefined. libstdc++'s order is not emulated:
  its introsort is GPL-3.0 code, which the FSL-licensed package may not include (decided by the
  orchestrator, 2026-10-25).
- **D13 / gap:** the warning in `reproduce` (to add with BET's merge, where `calc_bval` moves
  into `larmorx-nc-core`).

