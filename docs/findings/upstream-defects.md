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
> [afni-tshift.md](afni-tshift.md) and [fsl-mcflirt.md](fsl-mcflirt.md). **Clean-room
> implementers must not read those two sections.** The summary table and the gap lists below
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
still to do; – = nothing to do. How the switch works: [api/upstream-defects.md](../api/upstream-defects.md).

| ID | Tool | Defect | Class | fMRIPrep users | larmorx today | Gap |
|---|---|---|---|---|---|---|
| ANTS-01 | ANTs parser | some bracket forms and non-integer values rejected | e | none | same rejections | – |
| ANTS-02 | ANTs programs | crash or abort on missing files, missing arguments, text for numbers | a | none | refuses (exit 1) | – |
| ANTS-03 | ANTs I/O | names of 3 characters or fewer not read or written, exit 0 | e | none | reproduces | **yes** |
| ANTS-04 | ImageMath | exit 0 after a failed operation | e | hides failures | reproduces | **yes** |
| ANTS-05 | ImageMath | operand parsing: `7 ` is a file, `1e` is 0, no operand is 1 | e | none | reproduces | **yes** |
| ANTS-06 | ANTs programs | out-of-range float-to-integer conversions answered as x86-64 does (undefined) | a | none seen | reproduces x86-64 | decide |
| ANTS-07 | ImageMath | a smaller second image read out of bounds | a | none | refuses | – |
| ANTS-08 | ImageMath `/` | divisor ≤ 0: the previous voxel's result | c | none | reproduces | **yes** |
| ANTS-09 | ImageMath `total`, `mean` | the image holds running sums | c | none | reproduces | **yes** |
| ANTS-10 | ImageMath `vtotal` | all-zero output | c | none | reproduces | **yes** |
| ANTS-11 | ImageMath `Neg` | range missed on increasing images | c | none | reproduces | **yes** |
| ANTS-12 | ImageMath `TruncateImageIntensity` | brightest voxel can fall outside the histogram; 64 bins, usage says 65 | c, f | possible | reproduces | **yes** |
| ANTS-13 | ImageMath `Normalize` | range starts at 0 and 1e9 | c | none | reproduces | **yes** |
| ANTS-14 | ImageMath `G` (ITK) | variance above ≈709 voxels² gives NaN | c | none | reproduces | **yes** |
| ANTS-15 | ImageMath `G`, SmoothImage | wrong number of sigmas: unchanged image or 1 mm, exit 0 | e | none | reproduces | **yes** |
| ANTS-16 | ImageMath `Laplacian` | normalize flag read from the sigma argument | e | none (same result) | command line reproduces | **yes** |
| ANTS-17 | ImageMath `MO`, `ME` | `MO r 0` is a dilation of zeros; `ME` keeps other labels as 1 | c? | none | reproduces | decide |
| ANTS-18 | ImageMath `GD`, `GC` (ITK) | sign of a zero extreme depends on threads | b | none | defined signs | – |
| ANTS-19 | ImageMath `FillHoles` | `holeparam ≤ 1` reads outside the image | a | none | reproduces the reads inside the buffer | **yes** |
| ANTS-20 | ImageMath `GetLargestComponent` | nothing kept: every voxel becomes 1 | c | possible | reproduces | **yes** |
| ANTS-21 | ImageMath `ClusterThresholdVariate` | reads outside the image | a | none | not ported | – |
| ANTS-22 | ImageMath `PadImage` | fractional pad grows by ⌊2p⌋, shifts by ⌊p⌋ | c? | none | reproduces | decide |
| ANTS-23 | ImageMath `ExtractContours` | labels are truncated values | c? | none | reproduces | decide |
| ANTS-24 | ThresholdImage | unknown mode names become thresholds at 0; sixth argument read as a file | e, f | none | reproduces | **yes** |
| ANTS-25 | ThresholdImage Otsu with a mask | range from the smallest positive float, `else if` | c | none | reproduces | **yes** |
| ANTS-26 | ThresholdImage `Otsu 0` | never returns | a | none | refuses | – |
| ANTS-27 | MultiplyImages | unreadable second file multiplies by 0, silently | e | possible | command line reproduces | **yes** |
| ANTS-28 | ResampleImageBySpacing, ResampleImage, SmoothImage | uninitialised values reach the output | a | none | refuses | – |
| ANTS-29 | ResampleImage | size 1: infinite spacing, a zero image, exit 0 | c | none | refuses | decide |
| ANTS-30 | ResampleImage | seventh argument read twice | e | none | command line reproduces | **yes** |
| ANTS-31 | ResampleImageBySpacing | `nn` read from the wrong argument; dimension 5 does nothing | e | none | reproduces | **yes** |
| ANTS-32 | ResampleImageBySpacing | sigma `out/in − 1` taken in mm; outside points get the value at index 1 | c? | none | reproduces | decide |
| ANTS-33 | antsApplyTransforms | unknown `-u` ignored; interpolator parameters ignored | e | none | reproduces | **yes** |
| ANTS-34 | antsApplyTransforms `GenericLabel` (ITK) | never finishes on intensity images | a | none | neighbour labels only, same result | – |
| ANTS-35 | N4BiasFieldCorrection | `-c`, `-b`, `-t` without brackets set nothing | e | none | reproduces | **yes** |
| ANTS-36 | N4BiasFieldCorrection | unreadable mask becomes ones; `-r` without a mask does nothing | e | `-r` has no effect | reproduces | **yes** |
| ANTS-37 | N4 (ITK) | result depends on the thread count | b | anatomical N4s | single-thread result | – |
| ANTS-38 | N4 (ITK) | histogram range `else if`: a first-voxel minimum is dropped | c | possible, every N4 | reproduces | **yes** |
| ANTS-39 | N4 (ITK) | 256 bins give a 1024-point FFT | c | none | reproduces | **yes** |
| ANTS-40 | N4 (ITK) | zero and negative values stay in the log image | c? | unmasked N4s | reproduces | decide |
| ANTS-41 | ANTs oracle build (GCC 11) | two float roundings of N4's fit dropped | d | none (fMRIPrep's build follows the source) | reproduces the build | rule 8 |
| ANTS-42 | DenoiseImage (ITK) | large thread dependence, races | b | every run | single-thread result | – |
| ANTS-43 | DenoiseImage (ITK) | Rician correction NaN at high SNR becomes a bias of 0 | c | possible | reproduces | **yes** |
| ANTS-44 | DenoiseImage (ITK) | bias outside the mask keeps an unsmoothed value | c? | none | reproduces | decide |
| ANTS-45 | N4, DenoiseImage | errors explained only with `-v` | f | none | always explained | – |
| ANTS-46 | antsRegistration | BOLD reference's affine stops at 25 of 200 iterations | c? | every run | not ported (A4) | open |
| ITK-01 | ITK `Math::Floor` | 0 instead of −1 for tiny negative values | c | none seen | reproduces | **yes** |
| ITK-02 | ITK reader | a file with more dimensions than `D` loses its direction | c | none seen | reproduces | **yes** |
| ITK-03 | ITK relabelling | float labels stop at 2^24 | c | latent | reproduces | **yes** |
| ITK-04 | ITK Danielsson map | approximate, order-dependent distances | d | none | reproduces | – (D11) |
| ITK-05 | ITK `vnl_erf` | relative error 3e-7 in Gaussian weights | d | Gaussian interpolation | reproduces | – (D11) |
| ITK-06 | nifti_clib | NaN and ±inf read as 0 | c? | none seen | reproduces | decide |
| ITK-07 | ITK NIfTI reader | integer data scaled in float32 | d | scaled inputs | reproduces | – |
| ITK-08 | nifti_clib | asked for `x.nii.gz`, opens `x.nii` | e | none | opens the named file | decide |
| ITK-09 | ITK HDF5 writer | the same transform gives different bytes | b | none | byte-identical | – |
| PIPE-01 | nipype, niworkflows | `shrink_factor=2` dropped: the BOLD reference's N4 shrinks by 4 | c | every run | not a tool defect | decide |
| PIPE-02 | fMRIPrep `ResampleSeries`, OpenBLAS | volumes corrupted when OpenBLAS runs threads | b | outside fMRIPrep's image | thread-independent | – |
| PIPE-03 | fMRIPrep `ResampleSeries` | Jacobian along the wrong target axis | c | PEPOLAR, non-native spaces | corrected by default (`upstream_defects`, 2026-10-11) | – |
| PIPE-04 | nitransforms | `.h5` composites read in name order | c | latent | corrected by default (`upstream_defects`, 2026-10-11) | – |
| PIPE-05 | nitransforms | no displacement beyond a field's last nodes | c? | 2.9 % of MNI points | reproduces | decide |
| PIPE-06 | nitransforms, fMRIPrep | float32 roundings at every step | d | every run | reproduces | – (D11) |
| PIPE-07 | nitransforms | a one-transform head-motion file applied in world space | c? | one-volume runs | reproduces | decide |
| PIPE-08 | fMRIPrep confounds | motion parameters about another centre than mcflirt's | c? | every run | not ported | decide |
| PIPE-09 | fMRIPrep, niworkflows | wrong dtype log lines; a deprecated argument; float64 masks | f | every run | – | – |
| PIPE-10 | fMRIPrep, nipype | options that do nothing | e | their users | – | – |
| PIPE-11 | sMRIPrep | a registration fallback leaves the composite undefined | a | after a failed attempt | – | – |
| PIPE-12 | sMRIPrep | a default that is not an allowed choice | e | none | – | – |
| PIPE-13 | sMRIPrep | the report text prints a module | f | every report | – | – |
| PIPE-14 | sdcflows | `B0FieldTransform.apply` ignores head motion | c | none | – | – |
| PIPE-15 | nipype `MCFLIRT` | emits `-edge`; expects names mcflirt does not write | e | none | – | – |
| SCI-01 | SciPy `map_coordinates` | huge or NaN coordinates read outside the array | a | none | NaN | – |
| SCI-02 | SciPy `map_coordinates` | undefined conversions answered as x86-64 does | a | none | reproduces x86-64 | decide |
| SCI-03 | SciPy splines | `reflect`, `grid-mirror` do not interpolate on short axes | c | none | corrected by default (`upstream_defects`, 2026-10-11) | – |
| SCI-04 | SciPy on aarch64 | fused multiply-adds change last bits | d | ARM machines | never fuses | – |
| NP-01 | numpy, OpenBLAS, Accelerate | 4×4 products and inverses depend on the CPU's kernel | d | every run | one order everywhere | – |
| NP-02 | nibabel, numpy SVD | axis choice at exactly 45° follows the SVD's rounding | d | none | vnl's SVD | – |
| NIB-01 | nibabel | compression decided by the file name | e | none | decided by content | decide |
| NIB-02 | nibabel | `vox_offset` 0: header read as data | c | none | refuses | decide |
| NIB-03 | nibabel | complex data: imaginary part dropped | c | none | raises | decide |
| NIB-04 | nibabel | qform precision depends on the platform | d | none | f64 everywhere | – |
| GLIBC-01 | glibc 2.35 | `exp`, `log`, `sin`, `cos` not correctly rounded | d | every ANTs run | correctly rounded | – |
| GLIBC-02 | glibc 2.35 | `expf`, `logf`, `powf` not correctly rounded | d | N4, DenoiseImage | correctly rounded | – |
| GLIBC-03 | glibc 2.35 | `sinf`, `cosf` not correctly rounded; two builds differ | d | none | see entry | – |
| GLIBC-04 | glibc 2.35 | `pow` not correctly rounded for 5 filter-pole powers | d | none | exact | – |
| ANTSPY-01 | ANTsPy 0.6.3 | inverts an affine without being asked | c? | none | never inverts unasked | – |
| ANTSPY-02 | ANTsPy 0.6.3 | double transforms rounded to float32 | c | none | – | – |
| ANTSPY-03 | ANTsPy 0.6.3 | header direction transposed and rounded | c | none | – | – |
| AFNI-01 † | 3dTshift | negative `scl_slope`: output sign-flipped and scaled | c | rare | original: corrected by default; replica: reproduces | replica |
| AFNI-02 † | 3dTshift | float32 with slope and intercept scaled twice | c | rare | original: corrected by default (scales once), switch and warning; replica: reproduces | replica |
| AFNI-03 † | 3dTshift | header slice code 2 (sequential decreasing): no shift | c | none | original: corrected by default; replica: reproduces | replica |
| AFNI-04 † | 3dTshift | can crash for a shift beyond the series on odd slices | a | none | defined (zeros) | – |
| AFNI-05 † | 3dTshift | `-no_detrend` de-means one voxel of each pair only | c | none | original: corrected by default; replica: reproduces | replica |
| AFNI-06 † | 3dTshift | `-rlt+` restores the intercept; the help says the mean | c? | none | reproduces | decide |
| AFNI-07 † | 3dTshift | metre or micron units: sform not converted, voxel sizes converted | c | rare | original: corrected by default (converts), switch and warning; replica: reproduces | replica |
| AFNI-08 † | 3dTshift | an all-zero slice-time list writes `slice_end` 0 | c | none | original: writes `nz − 1` always (no black-box run found the trigger); replica: reproduces | replica; original: spec |
| AFNI-09 † | 3dTshift | slice-time files rounded two ways | d | none | replica reproduces | – |
| AFNI-10 † | 3dTshift | AFNI's own header extension (7 digits) wins over the NIfTI header | d | AFNI-written inputs | replica reproduces; original ignores | – |
| AFNI-11 † | 3dTshift | existing output: not written, exit 0 | e | none | error (exit 1) | decide |
| AFNI-12 † | 3dTshift | output names with blanks or non-ASCII refused | e | paths with blanks | any name accepted | decide |
| AFNI-13 † | 3dTshift | unknown pattern: "ERROR", not "FATAL ERROR" | f | none | – | – |
| AFNI-14 † | 3dTshift | `-no_detrend` with Fourier: "changing default", then fatal | f | none | – | – |
| AFNI-15 † | AFNI build | parallel make races | b | none | – | – |
| FSL-01 † | mcflirt | last volume not initialised from the previous one | c | every run | corrected by default (`upstream_defects`, 2026-10-11); on a synthetic series with large motion mcflirt's last volume converges to a 180° rotation, the corrected one to 0.36 mm from the truth ([mri-hmc-normcorr-count.md](../validation/mri-hmc-normcorr-count.md)) | – |
| FSL-02 † | mcflirt | normalised correlation effectively uncentred | c | every run | reproduces; the corrected count is opt-in (`normcorr_count="weights"`), measured in [mri-hmc-normcorr-count.md](../validation/mri-hmc-normcorr-count.md): a change about the size of mcflirt's band, no gain against the ground truth | decide |
| FSL-03 † | mcflirt | in-plane mode pads the first stage's reference twice | c, f | none | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-04 † | mcflirt `-gdt` | reference gradient unused; absolute `-out` aborts | c, a | none | not supported | – |
| FSL-05 † | mcflirt | `-scaling`, `-hist` ignored; `-edge` not an option | e | none | ignored with a warning; `-edge` explained (2026-10-11) | – |
| FSL-06 † | mcflirt | unknown `-cost` silently keeps normcorr | e | none | corrected by default (`upstream_defects`, 2026-10-11): an error | – |
| FSL-07 † | mcflirt | numbers read with `atoi`/`atof`; `-help` alone exits 1 | e | none | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-08 † | mcflirt `-stats` | end slices left 0; names with the extension inside | c, e | none | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-09 † | mcflirt | output types: int8 → uint8, truncation, no clamping | c, a | none | corrected by default (`upstream_defects`, 2026-10-11): int8 kept, rounded and clamped; `reproduce` truncates and wraps as x86-64 does | – |
| FSL-10 † | mcflirt | units forced to mm and s: a TR in ms becomes s | c | ms-unit inputs | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-11 † | mcflirt | `FSLOUTPUTTYPE` unset: fails after writing the matrices | e | none | writes `.nii.gz` | decide |
| FSL-12 † | mcflirt | impossible inputs abort after the whole estimation | a | none | refuses first | – |
| FSL-13 † | mcflirt | `.mat` directory quirks without `-mats` | e | none | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-14 † | mcflirt | one NaN voxel makes the volume's matrix NaN | c | rare | corrected by default (`upstream_defects`, 2026-10-11) | – |
| FSL-15 † | mcflirt `-init` | affects the final resampling only | c? | none | reproduces | decide |
| FSL-16 † | mcflirt | rotations orthogonal only to about 1e-8 | d | every run | reproduces | – (D11) |
| FSL-17 † | mcflirt | implementation differs from its paper | f | every run | follows the implementation | – (D11) |
| FSL-18 † | mcflirt Woods cost | squared bin counts | c? | none | reproduces | decide |

**Counts** (120 entries; an entry with two classes counts under its first):

| Class | Entries | of them questionable (c?) |
|---|---|---|
| (a) crash, undefined behaviour, out of bounds | 13 | |
| (b) race, nondeterminism | 6 | |
| (c) deterministic bug that changes results | 54 | 15 |
| (d) imprecision | 16 | |
| (e) command-line or parsing quirk | 25 | |
| (f) documentation or message only | 6 | |

Six entries have a second class: ANTS-12 and FSL-03 (f), ANTS-24 (f), FSL-04 and FSL-09 (a),
FSL-08 (e). ANTS-46 (an unexplained early stop) is counted as (c?) until A4 explains it. Gap
column (after the first D13 switches, 2026-10-11): 28 **yes**, 6 replica, 25 decide, 1 rule 8, 1
open; 13 entries switched to corrected by default.

## D13 gaps: what the next task switches to "correct by default"

The class (c) and (e) defects, and one class (a) read, that larmorx reproduces by default
today. For each: the larmorx code path the fix touches. Every fix needs, per the [porting
guide](../porting-guide.md) §8: the corrected default, a warning when the original's result
would have differed, the compatibility switch, a parity run with the switch (bit for bit as
today) and one with the default (the fix's effect reported), and re-recorded golden checksums
for the outputs that change ([validation/golden.md](../validation/golden.md), "Recording").
The proposed fix is in each entry; "(to confirm)" marks a fix whose intended behaviour the
upstream does not state.

**Status (2026-10-11).** The shared switch is in place (`upstream_defects=`,
`LARMORX_UPSTREAM_DEFECTS`, `larmorx_core::defects`; [api/upstream-defects.md](../api/upstream-defects.md)).
Done: fMRIPrep's libraries and SciPy (PIPE-03, PIPE-04, SCI-03), head-motion correction (every
FSL entry below; FSL-02 measured, opt-in), and the slice-timing original (AFNI-01, -02, -03,
-05, -07; AFNI-08 needs its trigger, see below). Still to do: the ANTs and ITK entries, and the
3dTshift replica's switch.

**Main package (`crates/`, `python/larmorx/`), ANTs and ITK:**

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
| ITK-01 | `Math::Floor` of tiny negatives | `larmorx_image::grid::{vnl_floor_f32, itk_floor_f64}` in the 4D paths of `larmorx_ants::resample` | none seen |
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
| ANTS-30 | ResampleImage's seventh argument (needs a design) | `larmorx_ants::cli::resample_image` | none |
| ANTS-31 | ResampleImageBySpacing's arguments | `larmorx_ants::cli::resample_image_by_spacing` | none |
| ANTS-33 | antsApplyTransforms: `-u`, ignored interpolator parameters | `larmorx_ants::cli::apply_transforms`, `OutputType::from_name` | none |

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
case and has no switch for it until the spec describes the trigger. The replica is a separate
task:

| ID | Defect | Code path | fMRIPrep |
|---|---|---|---|
| AFNI-01 | negative `scl_slope` | original: `larmorx_afni::dataset` (reading and storing back), its warning; the replica's dataset reader and writer; `lx.afni.tshift`'s warning | rare |
| AFNI-05 | `-no_detrend` voxel pairs | original: `larmorx_afni::tshift`, the warnings in `larmorx_afni::cli::tshift` and `lx.afni.tshift`; the replica | none |
| AFNI-03 | header slice code 2 | original: `larmorx_afni::timing::header_timing`; the replica's header reader | none (fMRIPrep passes `-tpattern`) |
| AFNI-02 | float32 with slope and intercept scaled twice | replica: its dataset reader (fix by default). Original: already scales once; it needs the warning and the switch, so the spec writer first adds the behaviour to `specs/3dTshift.md` §6 | rare |
| AFNI-07 | metre or micron units | replica: its writer (fix by default). Original: already converts; needs the switch, after a spec addition (§6) | rare |
| AFNI-08 | all-zero slice-time list | the replica's writer; the original's writer per spec §6 | none |

### Decisions needed before switching

**Decided by the user on 2026-10-11** (decision log, "D13's details"):
- Item 1, undefined conversions: **clamp and warn by default**; the compatibility switch
  reproduces x86-64's wraparound.
- Item 3, replicas: **the same corrected default as the originals**; the compatibility switch
  must give **exactly the original program's result**.
- Item 4, FSL-02: **measure first** (against mcflirt's band and the synthetic ground truth),
  then decide between fixing by default and keeping it opt-in. **Measured 2026-10-11**
  ([mri-hmc-normcorr-count.md](../validation/mri-hmc-normcorr-count.md)): the corrected count
  moves the matrices by about mcflirt's own variability (0.014 to 0.033 mm at the 95th
  percentile on 19 real runs; band 0.016 to 0.10 mm; 24 of 28 band runs within the band) and
  is not more accurate on the synthetic ground truth (better on 5 of 8 series, worse on 3, by
  0.001 to 0.16 mm). It stays opt-in; the user decides.
- Item 5, refusals: **a switch for these too.** Where the original's output is deterministic
  (e.g. a one-voxel image with infinite spacing), the switch reproduces it exactly. Where it
  depends on memory contents and changes from run to run (an unfilled buffer, an out-of-bounds
  read), no program can reproduce it, not even the original. There the switch reproduces
  everything defined (exit code, files, headers), fills the undefined part deterministically
  and warns that it does.
- Items 2, 6 and 7, defaults until decided otherwise: questionable behaviours stay faithful
  and documented until evidence shows a bug; pipeline choices are made with larmorprepx;
  ResampleImage's corrected command line is designed by the implementer and reviewed.

The original proposals, for the record:

1. **Undefined conversions** (ANTS-06, SCI-02). On 2026-10-09 larmorx decided that its
   command lines reproduce ANTs on x86-64 where C++ is undefined (an out-of-range `-u` cast
   wraps through int32), as fMRIPrep users see. D13 says undefined behaviour is never
   reproduced. Proposal: the default saturates (as `lx.ants.apply_transforms(dtype=...)`
   already does) and warns; the compatibility switch keeps the x86-64 answers.
2. **Questionable behaviours (c?)**: is each a bug? ANTS-17, ANTS-22, ANTS-23, ANTS-32,
   ANTS-40, ANTS-44, ITK-06, PIPE-05, PIPE-07, AFNI-06, FSL-15, FSL-18. ANTS-40 (N4's
   non-positive log values) and PIPE-05 (no displacement beyond the field) change every run
   they touch, so a fix would move many results.
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

- **ANTS-41, rule 8 (decided 2026-10-11):** N4 follows the C++ source, not the GCC 11 build
  (`larmorx_image::bspline::fit::gcc_double_axes` returns 0); the ANTs oracle is rebuilt to
  honour the source's roundings and every ANTs suite is re-run against it; the N4 record and
  golden checksums change.
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
  (SIGSEGV); MultiplyImages without its first file or output name aborts (`throw;`); PadImage
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
- **larmorx:** reproduces (`larmorx_ants::image`, `image.rs:384,398`; ImageMath's writers).
- **D13 / gap:** read and write any non-empty name; warn when ANTs would not have; switch.
  **Gap.**

### ANTS-04 ImageMath exits 0 after a failed operation — (e)
- **What:** the dispatch tables return `EXIT_SUCCESS` whatever the operation returned, so an
  operation that prints an error and writes nothing still exits 0 (`Mattes` and
  `ComponentToVector` excepted).
- **Evidence:** read: `ImageMath_Templates.hxx:14892-15435`; verified.
- **fMRIPrep:** a failed ImageMath step goes unnoticed until a later node misses its file.
- **larmorx:** exit 0 where ANTs returns normally (`larmorx_ants::cli::image_math`).
- **D13 / gap:** exit 1 when no output was written; switch. **Gap.**

### ANTS-05 ImageMath's operand parsing — (e)
- **What:** `from_string<float>` decides between a number and an image: `7 ` (trailing space)
  is an image, `1e` and `-` are the number 0, `1e40` is `FLT_MAX`, and no operand at all is
  the number 1.
- **Evidence:** read: `ImageMath_Templates.hxx:212` (libstdc++'s `num_get`); validated
  (`arithmetic/operand-*`).
- **fMRIPrep:** none.
- **larmorx:** reproduces (`larmorx_ants::cli::cstd::from_string_f32`).
- **D13 / gap:** a malformed number is an error; what a missing operand means is decided per
  operation (to confirm); switch. **Gap.**

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
- **larmorx:** the command lines reproduce x86-64 on every platform (decided 2026-10-09);
  `lx.ants.apply_transforms(dtype=...)` and head-motion correction saturate; a negative
  morphology radius is refused.
- **D13 / gap:** D13 never reproduces undefined behaviour; this conflicts with the 2026-10-09
  decision. **Decide** (Decisions, item 1).

### ANTS-07 A smaller operand image is read out of bounds — (a)
- **What:** ImageMath's voxel-wise operations read the second image at the first image's
  indices with its own strides; a smaller second image is read past its end. (Its geometry is
  ignored even when it is larger.)
- **Evidence:** read: `ImageMath_Templates.hxx:5583-5762`; validated (`arithmetic/larger-operand`).
- **fMRIPrep:** none.
- **larmorx:** a larger operand as ANTs; a smaller one is an error.
- **D13 / gap:** never reproduced. No gap.

### ANTS-08 ImageMath `/` keeps the previous voxel's result — (c)
- **What:** the quotient is computed only where the divisor is positive; every other voxel
  gets the previous voxel's result (0 before the first division), because the result variable
  is not reset.
- **Evidence:** read: `ImageMath_Templates.hxx:5583-5762`; validated (`arithmetic//-*`).
- **fMRIPrep:** none (not used).
- **larmorx:** reproduces (`larmorx_ants::image_math::arithmetic`, the command line and
  `lx.ants.image_arithmetic`).
- **D13 / gap:** divide wherever the divisor is not 0, and write 0 where it is (to confirm);
  warn; switch. **Gap.**

### ANTS-09 ImageMath `total` and `mean` write running sums — (c)
- **What:** the printed total and mean are right, but every voxel of the written image holds
  the running sum at that voxel.
- **Evidence:** read: `ImageMath_Templates.hxx:5583-5762`; validated.
- **fMRIPrep:** none.
- **larmorx:** reproduces (`larmorx_ants::image_math::arithmetic`).
- **D13 / gap:** write the final value (to confirm); switch. **Gap.**

### ANTS-10 ImageMath `vtotal` writes zeros — (c)
- **What:** `vtotal` is dispatched to the arithmetic function, which has no branch for it: the
  output is all zeros, exit 0.
- **Evidence:** read; validated.
- **fMRIPrep:** none.
- **larmorx:** reproduces.
- **D13 / gap:** refuse with a message (ANTs never implemented it); switch. **Gap.**

### ANTS-11 ImageMath `Neg` misses the range — (c)
- **What:** the range is found with `if (v > max) max = v; else if (v < min) min = v;` from
  ∓1e12: a voxel that raises the maximum is never compared with the minimum. In an image whose
  values increase in memory order the minimum stays 1e12 and the result (`max − v`) carries
  errors of about 1e-4.
- **Evidence:** read: `ImageMath_Templates.hxx:7495`; validated.
- **fMRIPrep:** none.
- **larmorx:** reproduces (`larmorx_ants::image_math::arithmetic::negative`).
- **D13 / gap:** compare every voxel with both bounds; warn when the range differs; switch.
  **Gap.**

### ANTS-12 `TruncateImageIntensity`'s histogram range — (c), (f)
- **What:** the range is found with `if (v < min) ... else if (v > max)`: if the first
  positive voxel in memory is the brightest, it only sets the minimum, the maximum becomes the
  next brightest, and the brightest voxel falls outside the histogram. (f): the usage says 65
  bins; the code uses 64.
- **Evidence:** read: `ImageMath_Templates.hxx:966-1110`; validated
  (`truncate/first-voxel-max`, fMRIPrep's `0.01 0.999 256`).
- **fMRIPrep:** sMRIPrep truncates the T1w twice (stages A03, A10); it matters only when the
  first positive voxel is the brightest (not on ds000005).
- **larmorx:** reproduces (`larmorx_ants::image_math::intensity::truncate_image_intensity`).
- **D13 / gap:** the true range; warn; switch. **Gap.**

### ANTS-13 ImageMath `Normalize`'s range starts at 0 and 1e9 — (c)
- **What:** range mode starts from `max = 0` and `min = 1e9`: an image without a positive
  value keeps the maximum 0, an image whose values all exceed 1e9 keeps the minimum 1e9.
- **Evidence:** read: `ImageMath_Templates.hxx:9099-9118`; validated.
- **fMRIPrep:** none.
- **larmorx:** reproduces (`larmorx_ants::image_math::intensity::normalize`).
- **D13 / gap:** the true range; switch. **Gap.**

### ANTS-14 ImageMath `G` gives NaN for large variances — (c)
- **What:** ITK's `GaussianOperator` computes `e^t` in the Bessel function `I_0`; above a
  variance of about 709 voxels², `e^t` overflows and the kernel is NaN, so the whole image is
  NaN (`G img 41` on 1.5 mm voxels).
- **Evidence:** read: `itkGaussianOperator.hxx:25-199`; validated (`G/huge-variance`).
- **fMRIPrep:** none (not used).
- **larmorx:** reproduces (`larmorx_image::discrete_gaussian`).
- **D13 / gap:** compute `e^{−t} I_k(t)` without overflow; warn; switch. **Gap.**

### ANTS-15 A wrong number of sigmas is not an error — (e)
- **What:** ImageMath `G` with a sigma count other than 1 or the dimension prints "Incorrect
  sigma vector size" and filters with variance 0 (the image comes back unchanged); SmoothImage
  prints the same and smooths with ITK's default of 1 mm. Both exit 0.
- **Evidence:** read: `ImageMath_Templates.hxx:8161-8211`, `SmoothImage.cxx`; validated
  (`G/no-sigma`, `G/wrong-length`, `gaussian/wrong-length`).
- **fMRIPrep:** none.
- **larmorx:** reproduces (`larmorx_ants::cli::image_math::gaussian`, `larmorx_ants::cli::smooth_image`).
- **D13 / gap:** exit 1; switch. **Gap.**

### ANTS-16 `Laplacian` reads its flag from the sigma — (e)
- **What:** `LaplacianImage` does not advance past the sigma, so `sig = atof(argv[5])` and
  `normalize = std::stoi(argv[5])`: `Laplacian img 1.5 0` normalises, `0.8 1` does not,
  `.5` aborts, `-1` normalises.
- **Evidence:** read: `ImageMath_Templates.hxx:9235-9285` (9244-9257); validated
  (`laplacian/*`).
- **fMRIPrep:** `Laplacian img 1.5 1` normalises either way: same result.
- **larmorx:** the command line reproduces it (`larmorx_ants::cli::image_math::gaussian`);
  `lx.ants.laplacian(img, sigma, normalize=...)` takes them separately.
- **D13 / gap:** read the flag from its own argument, as `Grad` does; warn when the two
  readings differ; switch. **Gap.**

### ANTS-17 `MO` with value 0, `ME` on label maps — (c?)
- **What:** ITK's binary opening erodes with its own background value, 0 by default, so
  `MO r 0` (foreground 0) erodes nothing and is a plain dilation of the zeros. ANTs' `ME`
  thresholds the eroded image at 0.5, so on a label map it erodes one label and leaves the
  others above 0.5 as 1.
- **Evidence:** read: `itkBinaryMorphologicalOpeningImageFilter.hxx:40,63`,
  `antsUtilities.h:230-247`; validated (`MO-value-0`, `ME-labels`).
- **fMRIPrep:** none (binary masks with value 1).
- **larmorx:** reproduces.
- **D13 / gap:** **decide** whether these are bugs.

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
- **larmorx:** reproduces the reads that wrap inside the buffer and refuses the others
  (`larmorx_ants::image_math::morphology::fill_holes`; expected divergence
  `fill-holes/hole-first-slice-0.5`).
- **D13 / gap:** reads outside the image are never reproduced, not even those that land inside
  the buffer; decide what an outside neighbour counts as (proposal: it is not counted). **Gap.**

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
- **larmorx:** reproduces (`larmorx_ants::image_math::components::largest_component`).
- **D13 / gap:** an all-zero mask when nothing is kept; warn; switch. **Gap.**

### ANTS-21 `ClusterThresholdVariate` reads outside the image — (a)
- **What:** after its histogram loop it reads the mask at the iterator's index, which is
  already past the end of the region, and decides with that value which voxels to zero.
- **Evidence:** read: `ImageMath_Templates.hxx:567-673`.
- **fMRIPrep:** none.
- **larmorx:** not ported ("not supported yet").
- **D13 / gap:** never reproduced. No gap.

### ANTS-22 `PadImage` with a fractional pad — (c?)
- **What:** each size becomes `(unsigned)(size + 2p)` but the image is copied at
  `(unsigned)(i + p)`: a pad of 2.5 adds 5 voxels and shifts by 2, so the padding is
  asymmetric.
- **Evidence:** read: `ImageMath_Templates.hxx:1955-2053`; validated (`pad-image`).
- **fMRIPrep:** none (integer pads).
- **larmorx:** reproduces.
- **D13 / gap:** **decide.**

### ANTS-23 `ExtractContours` labels by truncated value — (c?)
- **What:** each run's label is `static_cast<SizeValueType>(value)`: 1.2 and 1.7 are both
  label 1 and no contour separates them; 0.6 is background.
- **Evidence:** read: `itkLabelContourImageFilter.hxx:144`; validated (`extract-contours`).
- **fMRIPrep:** none.
- **larmorx:** reproduces.
- **D13 / gap:** **decide.**

### ANTS-24 ThresholdImage's arguments — (e), (f)
- **What:** (e) a mode other than exactly `Otsu` or `Kmeans` is a range threshold whose lower
  bound is `atof(mode)`: `otsu 2` thresholds at [0, 2]. (f) The sixth argument is always read
  as a mask image first, so a range threshold with an inside value prints
  " file 255 does not exist . " before using 255.
- **Evidence:** read: `ThresholdImage.cxx:431`; verified; validated.
- **fMRIPrep:** none (a range threshold with numbers).
- **larmorx:** reproduces (`larmorx_ants::cli::threshold_image`).
- **D13 / gap:** refuse a lower bound that is not a number; switch. **Gap.**

### ANTS-25 ThresholdImage Otsu with a mask: its range — (c)
- **What:** ANTs' own masked Otsu starts the maximum at `NumericTraits<float>::min()`, which is
  `FLT_MIN` (the smallest positive float), and updates it only when a voxel does not lower the
  minimum: an image without positive values keeps a maximum of `FLT_MIN`, and the first voxel
  is never a candidate for the minimum.
- **Evidence:** read: `ThresholdImage.cxx:152-262` (`:186`); validated.
- **fMRIPrep:** none (not used).
- **larmorx:** reproduces (`larmorx_ants::threshold_image`, `threshold_image.rs:106`; the command
  line and `lx.ants.otsu_threshold`).
- **D13 / gap:** the true range; switch. **Gap.**

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
- **larmorx:** the command line reproduces (`larmorx_ants::cli::multiply_images`);
  `lx.ants.multiply_images` raises `FileNotFoundError`.
- **D13 / gap:** refuse an argument that is neither a readable image nor a number; switch.
  **Gap.**

### ANTS-28 Uninitialised memory reaches the output — (a)
- **What:** ResampleImageBySpacing with a smoothed axis of fewer than 4 voxels catches ITK's
  exception ("Exception catched !") and resamples the smoothing filter's never-written buffer:
  values up to ±1e27, different on every run, exit 0. ResampleImage with a spacing list of the
  wrong length leaves the spacing uninitialised (then aborts); SmoothImage's median with a sigma
  list of the wrong length leaves the radius uninitialised.
- **Evidence:** verified (three runs, three outputs); read.
- **fMRIPrep:** none.
- **larmorx:** refuses with ITK's message (expected divergence `errors/thin-smoothed-axis`).
- **D13 / gap:** never reproduced. No gap.

### ANTS-29 ResampleImage to size 1 — (c)
- **What:** by size, the spacing is `old · (n − 1) / (size − 1)`: for size 1 it is infinite;
  vnl's SVD then fails on the grid matrix and ANTs writes a 1 × 1 × 1 image of 0 with infinite
  spacing, exit 0.
- **Evidence:** read: `ResampleImage.cxx:136`; verified.
- **fMRIPrep:** none.
- **larmorx:** refuses sizes below 2 (expected divergence `errors/size-1`).
- **D13 / gap:** **decide** (Decisions, item 5: no switch proposed).

### ANTS-30 ResampleImage reads its seventh argument twice — (e)
- **What:** `std::stoi(argv[7])` is the pixel type, and the same text is the interpolator's
  parameter (Gaussian sigmas, windowed-sinc window, B-spline order): `… 2 1.5x1.5x1.5` resamples
  an `unsigned char` image; a window letter can never be given (`… 3 l` aborts), so only the
  default Hamming window is reachable; `b` would be a second Lanczos anyway (line 76).
- **Evidence:** read: `ResampleImage.cxx:331-334`, `:144-236`; validated (`interpolation/*`).
- **fMRIPrep:** none.
- **larmorx:** the command line reproduces (`larmorx_ants::cli::resample_image`).
- **D13 / gap:** the usage text documents no position for the parameter, so the corrected
  command line needs a design (Decisions, item 7); switch. **Gap.**

### ANTS-31 ResampleImageBySpacing's arguments — (e)
- **What:** in 2D the `nn` flag is read from the `addvox` argument whenever an eighth argument
  exists; in 4D it is read as soon as `argc > 9`, so an `addvox` without `nn` aborts; a
  dimension other than 2, 3 or 4 does nothing and exits 0.
- **Evidence:** read: `ResampleImageBySpacing.cxx:130,393`; validated.
- **fMRIPrep:** none.
- **larmorx:** reproduces (`larmorx_ants::cli::resample_image_by_spacing`).
- **D13 / gap:** each value from its own argument; other dimensions refused; switch. **Gap.**

### ANTS-32 ResampleImageBySpacing's smoothing and outside value — (c?)
- **What:** the smoothing sigma is `out/in − 1`, a ratio, which ITK takes in millimetres and
  divides by the spacing again (1 → 2 mm smooths one voxel, 2 → 4 mm half a voxel). Points
  beyond the input get the input's value at index `(1, 1, …)`.
- **Evidence:** read: `ResampleImageBySpacing.cxx:144,190,277,406`; validated.
- **fMRIPrep:** none.
- **larmorx:** reproduces.
- **D13 / gap:** **decide.**

### ANTS-33 antsApplyTransforms ignores what it does not understand — (e)
- **What:** an unknown `-u` name silently keeps the default output type; `MultiLabel[sigma,alpha]`
  ignores alpha (always 4.0); `GenericLabel[...]` ignores its parameter (always linear).
- **Evidence:** read: `antsApplyTransforms.cxx:1453-1600`, `make_interpolator_snip.tmpl:127`.
- **fMRIPrep:** none (no alpha, known types).
- **larmorx:** reproduces (`larmorx_ants::cli::apply_transforms`; `OutputType::from_name`
  returns nothing and the default applies).
- **D13 / gap:** refuse an unknown type; honour the documented alpha (`MultiLabel[<sigma>,<alpha=4.0>]`
  in the usage) or warn; switch. **Gap.**

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
- **larmorx:** reproduces (`larmorx_ants::cli::n4_bias_field_correction`).
- **D13 / gap:** honour the unbracketed value or refuse it; switch. **Gap.**

### ANTS-36 N4's mask and rescaling — (e)
- **What:** a mask that cannot be read is replaced by an image of ones ("Mask not read") and
  still counts as given, so restoration and rescaling run; rescaling never runs without a mask,
  and `-r 1` is the same as nothing.
- **Evidence:** read: `N4BiasFieldCorrection.cxx:113-134,495-532`; validated (`options/missing-mask`).
- **fMRIPrep:** the BOLD reference's N4 passes `-r` without `-x`: it does nothing.
- **larmorx:** reproduces (`larmorx_ants::cli::n4_bias_field_correction`).
- **D13 / gap:** refuse an unreadable mask; warn that `-r` has no effect without a mask;
  switch. **Gap.**

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
  minimum (with a mask: the first voxel of the mask in memory order).
- **larmorx:** reproduces (`larmorx_image::n4::sharpen`).
- **D13 / gap:** the true range; warn when the first voxel was the unique minimum; switch.
  **Gap.**

### ANTS-39 N4's FFT size for 256 bins — (c)
- **What:** the histogram is padded to `2^(ceil(logf(bins) / log(2.0)) + 1)`, a float log
  divided by a double log: for 256 bins the quotient is slightly above 8, so the FFT has 1024
  points instead of 512.
- **Evidence:** read: `itkN4BiasFieldCorrectionImageFilter.hxx:331-343`; verified
  (`options/histogram-256`).
- **fMRIPrep:** none (200 bins).
- **larmorx:** reproduces (`larmorx_image::n4`, `n4.rs:157`).
- **D13 / gap:** an exact power-of-two test; switch. **Gap.**

### ANTS-40 N4 keeps zero and negative values in the log image — (c?)
- **What:** an included voxel is replaced by `logf(v)` only if `v > 0`; 0 stays 0 (the log of
  an intensity of 1) and negative values stay negative, inside the histogram. Without a mask, a
  raw image's zero background enters the histogram at log value 0.
- **Evidence:** read: `itkN4BiasFieldCorrectionImageFilter.hxx:137-143`; validated (`inputs/outliers`).
- **fMRIPrep:** the N4s without a mask: sMRIPrep before the template, the brain extraction's
  first pass, the BOLD reference (a weight image only).
- **larmorx:** reproduces.
- **D13 / gap:** **decide**; a fix would change every unmasked run.

### ANTS-41 The ANTs oracle build drops two float roundings — (d)
- **What:** not a source defect. The repository's ANTs 2.6.5 oracle (GCC 11.4, `-O3`, baseline
  x86-64) computes the B-spline fit's `r[0]`, `r[1]` and the points' first two coordinates in
  double precision in its 2D and 3D instantiations, where the source rounds them to float
  (`itkImageBase.h:589-598`; `divpd` without `cvtpd2ps`).
- **Evidence:** verified: the disassembly; fMRIPrep 25.2.6's ANTs (conda-forge 2.6.2) follows the
  source, and a larmorx build with the source's roundings reproduces all six of fMRIPrep's N4
  outputs ([fmriprep-oracle.md](fmriprep-oracle.md)).
- **fMRIPrep:** none; its build follows the source.
- **larmorx:** reproduces the oracle build (`larmorx_image::bspline::fit::gcc_double_axes`).
- **D13 / gap:** rule 8 (decided 2026-10-11): follow the source, rebuild the oracle. **Rule 8.**

### ANTS-42 DenoiseImage depends on threads and races — (b)
- **What:** the filter splits the centres into slabs (4 work units per thread); each centre adds
  to the output and count of its whole patch and overwrites the bias image there, so near each
  slab border two units write the same voxels in an order that depends on scheduling. On a T1w:
  21 % of voxels change at 2 threads (by up to 97), 85 % at 12 (up to 271); from 8 threads the
  result changes from run to run.
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
- **fMRIPrep:** sMRIPrep's denoising runs with the Rician model; the effect on a T1w has not
  been measured.
- **larmorx:** reproduces (`larmorx_image::adaptive_denoising`).
- **D13 / gap:** evaluate the correction factor without overflow; warn; switch. **Gap.**

### ANTS-44 DenoiseImage's bias outside the mask — (c?)
- **What:** where the smoothed bias is not positive, or outside the mask, the bias keeps its
  unsmoothed value, so a voxel just outside a mask keeps the raw estimate of a centre inside it.
- **Evidence:** read: `itkAdaptiveNonLocalMeansDenoisingImageFilter.hxx:397-494`; validated.
- **fMRIPrep:** none (no mask).
- **larmorx:** reproduces.
- **D13 / gap:** **decide.**

### ANTS-45 Errors explained only with `-v` — (f)
- **What:** N4 exits 1 on a weight image on another grid, and on a mesh of the wrong length,
  but says why only with `-v`; DenoiseImage's invalid noise model and radii likewise.
- **Evidence:** read; verified.
- **fMRIPrep:** none.
- **larmorx:** always prints the reason; exit codes as ANTs'.
- **D13 / gap:** not D13. No gap.

### ANTS-46 The BOLD reference's affine registration stops early — (c?), open
- **What:** fMRIPrep's affine registration of the BOLD reference to MNI (`Affine[0.1]`, Mattes
  64 bins, `Random 0.2`, convergence `[200, 1e-09, 10]`) ends after iteration 25 of 200 with a
  convergence value of 1.9e-6, above its threshold. Not understood yet.
- **Evidence:** verified (its `command.txt` re-run in fMRIPrep's image with `--verbose 1`);
  ANTs 2.6.2, `itkantsRegistrationHelper.hxx:1363-1375`.
- **fMRIPrep:** every run.
- **larmorx:** not ported (A4).
- **D13 / gap:** open; A4 explains it.

### ITK-01 `Math::Floor` of tiny negative values — (c)
- **What:** on x86-64 `itk::Math::Floor` (and `vnl_math::floor`) is
  `round_half_to_even(2x − 0.5) >> 1`; for x in [−2⁻²⁶, 0) in float ([−2⁻⁵⁵, 0) in double) it
  gives 0, not −1, so 4D linear interpolation (`EvaluateUnoptimized`) mixes in the next voxel
  with a tiny negative weight.
- **Evidence:** read: `itkMathDetail.h:414-422`; verified (the last 7 voxels of a 4D case).
- **fMRIPrep:** none seen.
- **larmorx:** reproduces (`larmorx_image::grid::{vnl_floor_f32, itk_floor_f64}`, used by the
  4D paths of `larmorx_ants::resample`).
- **D13 / gap:** the true floor; switch. **Gap.**

### ITK-02 Extra dimensions are read with an identity direction — (c)
- **What:** a file with more dimensions than the program's `D` is read as its first
  `D`-dimensional block with the ImageIO's default (identity) direction; origin and spacing are
  kept. `ImageMath 3 ... m bold.nii.gz 2` writes the first volume axis-aligned, even if the BOLD
  is oblique.
- **Evidence:** read: `itkImageFileReader.hxx` (`GenerateOutputInformation`); validated
  (`arithmetic/dim3-of-4d`, N4 `inputs/dim3-of-4d`).
- **fMRIPrep:** none seen.
- **larmorx:** reproduces (`larmorx_ants::image::from_itk`).
- **D13 / gap:** keep the top-left `D × D` block of the file's direction (to confirm); warn;
  switch. **Gap.**

### ITK-03 Float labels stop at 2^24 — (c), latent
- **What:** `RelabelComponentImageFilter` counts the new labels in the output pixel type; with
  a `float` output, as in ANTs, `label + 1` stops growing at 2^24.
- **Evidence:** read: `itkRelabelComponentImageFilter.hxx`.
- **fMRIPrep:** latent (16.7 million components).
- **larmorx:** reproduces (`larmorx_ants::image_math::morphology::float_labels`).
- **D13 / gap:** count in integers; switch. **Gap** (latent).

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
- **Evidence:** read: `nifti1_io.c:5013-5049`; verified (`head-outliers`).
- **fMRIPrep:** none seen.
- **larmorx:** reproduces in the ANTs programs (`nifti::itk::read_itk_image`).
- **D13 / gap:** **decide** (it is what every ANTs user gets).

### ITK-07 Integer data scaled in float32 — (d)
- **What:** ITK casts integer values to float and scales in double, then rounds to float; a
  scaled int16 BOLD differs from nibabel's float64 scaling by float32 rounding.
- **Evidence:** read: `itkNiftiImageIO.cxx:600-640,745-795,1275-1310`.
- **fMRIPrep:** scaled inputs to ANTs programs.
- **larmorx:** reproduces in the ANTs programs.
- **D13 / gap:** not D13.

### ITK-08 nifti_clib opens a different file — (e)
- **What:** asked for `x.nii.gz`, a reader built on nifti_clib opens `x.nii` instead if both
  exist.
- **Evidence:** verified with SimpleITK (it corrupted an early benchmark).
- **fMRIPrep:** none.
- **larmorx:** opens exactly the path given.
- **D13 / gap:** **decide** (Decisions, item 5: no switch proposed).

### ITK-09 ITK's HDF5 writer is not byte-reproducible — (b)
- **What:** ITK stores object modification times, so writing the same transform twice gives
  different bytes; only the datasets are reproducible.
- **Evidence:** verified.
- **fMRIPrep:** none (values unaffected).
- **larmorx:** `lx.transforms.write` gives the same bytes (verified 2026-10-11: two writes
  1.5 s apart are identical).
- **D13 / gap:** not reproduced. No gap.

## fMRIPrep and its libraries

fMRIPrep 25.2.6 (the oracle run) or its source at `21a490fb`, niworkflows 1.15.2, nipype
1.12.0, nitransforms 25.1.0. Details in [fmriprep-oracle.md](fmriprep-oracle.md),
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
- **D13 / gap:** **decide** for larmorprepx: the intended 2 or the factor 4 fMRIPrep uses.

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
- **Evidence:** read: `fmriprep/interfaces/resampling.py:316`. Not yet exercised by an oracle run:
  ds000005 has no field maps, and the ds001600 run (2026-10-11) processed its field maps but
  skipped its four BOLD runs ("Too short BOLD series (<= 5 timepoints)"); measured instead on
  real geometry with a synthetic field map (below).
- **fMRIPrep:** the Jacobian is on by default for PEPOLAR field maps
  (`fmriprep/workflows/base.py:878-887`); outputs in T1w and standard spaces whose axes are
  ordered differently from the BOLD grid's.
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
  percentile.
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
- **D13 / gap:** **decide.**

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
- **D13 / gap:** **decide.**

### PIPE-08 Motion parameters about another centre — (c?)
- **What:** fMRIPrep recomputes the motion parameters from the ITK transforms about
  `zooms × scipy.ndimage.center_of_mass(boldref)`, in file order and without subtracting the
  minimum; mcflirt's own parameters use the centre of mass with the minimum subtracted, in its
  radiological order. The translations (and FD) are therefore about a different point.
- **Evidence:** read: `fmriprep/interfaces/confounds.py:102-229`; the mcflirt rule in
  `specs/mcflirt.md` §2, §10.3.
- **fMRIPrep:** every run's motion confounds.
- **larmorx:** the confounds are not ported yet.
- **D13 / gap:** **decide** for larmorprepx.

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
- **larmorx:** reproduces x86-64 on every platform (`larmorx_interp::ndimage`).
- **D13 / gap:** **decide**, with ANTS-06.

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
- **D13 / gap:** **decide** (Decisions, item 5: no switch proposed).

### NIB-02 `vox_offset` 0 reads the header as data — (c)
- **What:** a single-file image with `vox_offset = 0` is read with the header bytes as voxel
  data.
- **Evidence:** validated (`malformed/vox-offset-zero.nii`).
- **fMRIPrep:** none.
- **larmorx:** refuses.
- **D13 / gap:** **decide** (item 5).

### NIB-03 Complex data lose their imaginary part — (c)
- **What:** `get_fdata` on complex data silently drops the imaginary part.
- **Evidence:** validated.
- **fMRIPrep:** none.
- **larmorx:** raises.
- **D13 / gap:** **decide** (item 5).

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
- **larmorx:** both implementations reproduce it; the original warns
  (`larmorx_afni::dataset::read`, `lx.afni.tshift`).
- **D13 / gap:** apply the factor on extraction as on insertion; warn; switch. **Gap** (both
  implementations).

### AFNI-02 † float32 with slope and intercept is scaled twice — (c)
- **What:** 3dTshift works on `EDIT_full_copy` of the input, whose brick factors are set when
  the header is opened (`edt_fullcopy.c:31`), before the data are loaded (`:44`). For float32
  with both a slope and an intercept the factor is set at open time (`thd_niftiread.c:720`) and
  loading also scales the values (`:1068-1088`): the output stores `2x + 3` with `scl_slope = 2`,
  so a reader sees `4x + 6`.
- **Evidence:** read; verified with the oracle (2026-10-10). No parity case (the test data have
  no such file).
- **fMRIPrep:** float32 BOLD with a slope and an intercept (rare).
- **larmorx:** the replica reproduces it; the original scales once (correct) without a warning
  or a switch: the spec does not describe the behaviour.
- **D13 / gap:** scale once; warn; switch. **Gap:** the replica's default; the original's
  warning and switch, after the spec writer adds the behaviour to `specs/3dTshift.md` §6.

### AFNI-03 † Header slice code 2 shifts nothing — (c)
- **What:** for `slice_code = 2` (sequential decreasing) `THD_open_nifti` loops
  `for( kk=slice_end ; kk >= slice_end ; kk-- )` (`thd_niftiread.c:653`): only `slice_end` is
  visited, every offset stays 0, and 3dTshift warns "input has only 1 time offset" and shifts
  nothing.
- **Evidence:** read; validated (`header-timing/code2-seq-dec`); spec §3.2 (observed).
- **fMRIPrep:** none (it passes `-tpattern`).
- **larmorx:** both implementations reproduce it (the command line's header-timing path; the
  Python API requires `slice_times`).
- **D13 / gap:** visit the slices from `slice_end` down to `slice_start`; warn; switch. **Gap.**

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
- **larmorx:** both reproduce it and warn (`larmorx_afni::tshift`, `larmorx_afni::cli::tshift`,
  `lx.afni.tshift(detrend=False)`); the original's last bits are not exact yet.
- **D13 / gap:** de-mean each voxel with its own mean; switch. **Gap** (both).

### AFNI-06 † `-rlt+` restores the intercept — (c?)
- **What:** AFNI's help says `-rlt+` adds the mean back; 3dTshift adds the fitted trend's
  intercept, its value at the first time point used (a series `50 + 2i` comes back at level 50,
  not 113).
- **Evidence:** spec §4 (observed).
- **fMRIPrep:** none.
- **larmorx:** both reproduce it; the Python API calls it `restore="intercept"`.
- **D13 / gap:** **decide** whether the help or the code is right.

### AFNI-07 † Metre and micron units — (c)
- **What:** the output sform is the matrix AFNI took from the header, not rescaled when the
  spatial unit is metres or microns, while the voxel sizes written to `pixdim` are (×1000 or
  ×0.001): the output header is inconsistent.
- **Evidence:** verified (2026-10-10, metre-unit inputs; the replica's sform and qform agree with
  AFNI's).
- **fMRIPrep:** inputs stored in metres or microns (rare).
- **larmorx:** the replica reproduces it; the original converts the matrix to millimetres
  (spec §6), with no warning or switch and no parity case.
- **D13 / gap:** convert the sform too; warn; switch. **Gap:** the replica's default; the
  original's switch after a spec addition.

### AFNI-08 † An all-zero slice-time list writes `slice_end` 0 — (c)
- **What:** when the output keeps slice offsets, `get_slice_timing_pattern`
  (`thd_niftiwrite.c:519-594`) derives the slice fields; its "all zeros" test
  (`sfirst == slast && MYFPEQ(tlist[sfirst],0.0)`, `:543`) can never be true, so a list of
  zeros finds no pattern and gets `slice_end = 0`.
- **Evidence:** read.
- **fMRIPrep:** none (header field only, in "copy of input" cases).
- **larmorx:** the replica translates it; the original writes the fields of spec §6.
- **D13 / gap:** make the test reachable; switch. **Gap** (minor, header only).

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
- **Evidence:** validated (`errors/existing-output`).
- **fMRIPrep:** none (fresh working directories).
- **larmorx:** an error, exit 1.
- **D13 / gap:** **decide** (item 5: no switch proposed).

### AFNI-12 † Output names are refused — (e)
- **What:** `THD_filename_ok` (`thd_filestuff.c:638`) refuses a `-prefix` with control
  characters, white space, shell metacharacters or bytes above 127 anywhere in the path, unless
  `AFNI_ALLOW_ARBITRARY_FILENAMES` is set (`:647`).
- **Evidence:** read: `3dTshift.c:391-392`.
- **fMRIPrep:** outputs in directories with blanks or accents.
- **larmorx:** both accept any path; the replica says with `-verbose` when AFNI would refuse.
- **D13 / gap:** **decide** (item 5: no switch proposed).

### AFNI-13 † "ERROR", not "FATAL ERROR" — (f)
- **What:** an unknown `-tpattern` name is reported by `ERROR_message`, then 3dTshift exits 1
  (`thd_timeof.c:186-188`, `3dTshift.c:422`).
- **Evidence:** read; validated (`errors/unknown-pattern`).
- **fMRIPrep:** none.
- **larmorx:** the replica prints AFNI's text; the original "FATAL ERROR"; both exit 1.
- **D13 / gap:** not D13.

### AFNI-14 † "changing default", then fatal — (f)
- **What:** `-no_detrend` while the method is Fourier prints "found -no_detrend, changing
  default to -heptic" as a fatal error and exits.
- **Evidence:** validated (`errors/no-detrend-fourier`).
- **fMRIPrep:** none.
- **larmorx:** the replica prints AFNI's text; the original explains the order needed.
- **D13 / gap:** not D13.

### AFNI-15 † The build races under `make -j` — (b)
- **What:** AFNI's niml sub-make races in a parallel build; FSL's own `ld` on `PATH` also breaks
  the build.
- **Evidence:** verified (`scripts/build_afni_oracle.sh`).
- **fMRIPrep:** none.
- **larmorx:** the oracle script works around both.
- **D13 / gap:** no larmorx code. No gap.

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
- **D13 / gap:** **decide** (Decisions, item 4): the fix changes the cost of every run.

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
- **larmorx:** not supported (exit 1).
- **D13 / gap:** No gap.

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
- **larmorx:** reproduces both (`larmorx_mri::hmc::stats::temporal`,
  `larmorx_mri::cli::hmc::write_outputs`).
- **D13 / gap:** every slice; names from the stem (to confirm); switch. **Gap.**

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
- **D13 / gap:** **decide** (item 5: no switch proposed).

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

### FSL-15 † `-init` affects only the final resampling — (c?)
- **What:** `initmat` is reset to the identity before the stages (`mcflirt.cc:707`); `-init` is
  read only for the final resampling (`:769-774,787-804`), so the matrices are the same with
  and without it.
- **Evidence:** read; verified.
- **fMRIPrep:** none.
- **larmorx:** reproduces.
- **D13 / gap:** **decide** whether an "initial transform" should start the search.

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

### FSL-18 † Woods' cost squares the bin counts — (c?)
- **What:** the Woods cost sums `Sqr(num[b])*stdev/sum[b]` (`costfns.cc:1456-1561`), n_b², where
  Woods' criterion weights by n_b.
- **Evidence:** read.
- **fMRIPrep:** none (Woods registers no BOLD run, for mcflirt or larmorx).
- **larmorx:** reproduces (spec appendix A).
- **D13 / gap:** **decide.**
