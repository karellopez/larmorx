# niworkflows' helpers under sdcflows and fMRIPrep

`lx.niworkflows`: the interfaces of niworkflows 1.15.4 (Apache-2.0, the NiPreps developers;
the same on these paths in 1.15.2, fMRIPrep 25.2.6's) that sdcflows' and fMRIPrep's field-map
steps run, ported in plain Python with numpy's, SciPy's, scikit-image's and nibabel's arithmetic
as niworkflows calls it (`lx.ndimage`, `lx.skimage`, `larmorx_core::numpy`,
`larmorx.io.save_nibabel`). A replica of niworkflows' Python (bit for bit), in the main package; a
clean room comes later, after FreeSurfer (the user, 2026-11-06 and 2026-11-11).

**Status: `experimental`** (checked file for file against niworkflows in both kept environments
and against fMRIPrep's own files; records: [niworkflows-helpers](../validation/niworkflows-helpers.md)
for the field maps' steps, [confounds-fmriprep](../validation/confounds-fmriprep.md) for the confound
steps and [confounds-nipype](../validation/confounds-nipype.md) for `Robust*CompCor`).

## `intensity_clip`: `IntensityClip`

| larmorx | niworkflows | Notes |
|---|---|---|
| `intensity_clip(in_file, p_min=35.0, p_max=99.98, nonnegative=True, dtype="int16", invert=False, *, out_dir=None, numpy_version="2.5.4", upstream_defects="fix", n_threads=1)` → `IntensityClipResult(image, out_file)` | `nibabel.IntensityClip` (`_advanced_clip`): the image squeezed, `median_filter(footprint=ball(3))`, the `p_min`/`p_max` percentiles of the positive (`nonnegative`) denoised values, the original clipped there, shifted to 0, divided by its maximum, inverted, `round(255 x)` for `int16`/`uint8`; `cal_max`; `clipped.nii.gz` | a non-3D image: `RuntimeError("<…> is not a 3D file.")`; `numpy_version` picks numpy's percentile arithmetic (2.2/2.3: `float32`; 2.5: `float64`) |

## `intra_modal_merge`: `IntraModalMerge`

| larmorx | niworkflows | Notes |
|---|---|---|
| `in_files` (a path or a list) | `in_files` | the outputs are named after the first file |
| `in_mask=None` | `in_mask` | the mask of `grand_mean_scaling` (`> 0.5`) |
| `hmc=True` | `hmc` | with more than one file: `mcflirt -in <merged> -cost normcorr -out <merged>_mcf.nii.gz -refvol 0 -mats -plots` (nipype's `MCFLIRT`) through `larmorx fsl mcflirt`: the replica with `larmorx-nc` installed, else the clean room (`hmc_implementation`: `"auto"`, `"replica"`, `"clean-room"`); `out_mats` (`<out>.mat/MAT_NNNN`) and `out_movpar` (`<out>.par`) as nipype lists them |
| `zero_based_avg=True` | `zero_based_avg` | the average minus its minimum |
| `to_ras=True` | `to_ras` | each file reoriented to the closest RAS orientation first (`reorient`: nibabel's `as_closest_canonical`, `dim_info` moved with the axes), `<name>_ras` |
| `grand_mean_scaling=False` | `grand_mean_scaling` | PIPE-18 (below) |
| `out_dir=None` | the node's directory | without it the files go to a temporary directory and only the images are returned |
| `numpy_version="2.5.4"` | | `grand_mean_scaling`'s percentile |
| `upstream_defects="fix"`, `n_threads=1` | | |

The result, `IntraModalMergeResult(merged, average, out_file, out_avg, out_mats, out_movpar,
warnings)`: the merged series (`<first>_merged`; for a merge of fewer than four dimensions also
the average), the average of the `float32` series over its fourth axis (`<first>_avg`), both
written in the first image's stored type with nibabel's scaling and the header nibabel's
constructors leave on the way ([findings](../findings/sdcflows-estimation.md)). Errors as
niworkflows': a 5D image (`RuntimeError("Input image (…) is 5D.")`), images of different
dimensions, shapes or affines (nibabel's `concat_images`: `ValueError`).

Corrected by default (`upstream_defects="reproduce"` gives niworkflows' behaviour):

| ID | niworkflows does | larmorx by default |
|---|---|---|
| PIPE-18 | `grand_mean_scaling` fails with numpy's `IndexError` for every series of two or more volumes (a mask of the wrong shape) | each volume scaled by the largest median over its own inside the mask, with a warning |
| PIPE-19 | a 6- or 7-dimensional image is averaged over its fourth axis | niworkflows' error for an image of more than four dimensions |
| PIPE-33 | `IntensityClip` of an image constant between its percentiles divides it by its maximum, 0: a NaN image, converted to `int16` or `uint8` by an undefined conversion (0 on x86-64) | a `ValueError` (rule B); `reproduce` niworkflows' result with x86-64's NaN and casts on every platform (`intensity_clip(..., upstream_defects=)`) |
| PIPE-34 | `RobustAverage` divides each gathered volume by its mean over the brightest's: a volume whose mean is 0 after the clipping (an empty volume) becomes NaN or infinite voxels, registered and averaged | a `ValueError` naming the volumes (rule B); `reproduce` niworkflows' arithmetic (`robust_average(..., upstream_defects=)`) |
| PIPE-40 | `IntraModalMerge` with `to_ras` reorients the series to RAS but not `in_mask`, so `grand_mean_scaling` indexes the RAS series with the mask on the input's grid (numpy's `IndexError` when the shapes differ, a mirrored mask when they agree) | the mask reoriented with the series (`as_closest_canonical`), with a warning |

`as_closest_canonical(image)` and `reorient(in_file, newpath=None)` are available on their own.

## `robust_average`: `RobustAverage`

| larmorx | niworkflows | Notes |
|---|---|---|
| `in_file` | `in_file` | a 3D image is the result itself; one volume after squeezing: `<in>_squeezed` |
| `t_mask=None` | `t_mask` | the volumes kept (all); its length must be the series' (`ValueError('Image length (…) unmatched by mask (…)')`) and it must keep one (`ValueError('At least one volume should be selected for slicing')`) |
| `mc_method="AFNI"` | `mc_method` | `"AFNI"`: nipype's `Volreg(interp='Fourier', args='-twopass', zpad=4)`, i.e. `3dvolreg -twopass -Fourier -1Dfile <s>.1D -1Dmatrix_save <s>.aff12.1D -prefix <s>_volreg.nii.gz -zpad 4 -maxdisp1D <s>_md.1D <s>.nii.gz` on the sliced volumes (`volreg_implementation`: the replica by default); `"FSL"`: `mcflirt -refvol 0 -sinc_final` (`mcflirt_implementation`); `None`: no registration |
| `nonnegative=True` | `nonnegative` | the lower clip at 0 (else the 0.2th percentile, then the minimum) |
| `two_pass=True` | `two_pass` | `-twopass` |
| `num_threads=None` | `num_threads` | 3dvolreg's `OMP_NUM_THREADS` (no effect on its result) |
| `out_dir=None`, `numpy_version="2.5.4"`, `upstream_defects="fix"`, `n_threads=1` | | `numpy_version`: the percentiles' arithmetic and the order of the drift's sums (numpy 2.2: chunks of 8192; fMRIPrep 25.2.6's `"2.2.6"`) |

The volumes are read as `float32`, clipped to the 99.8th percentile, divided by the global
signal's drift (each volume's mean over the brightest's), clipped again and written as
`<in>_sliced` with the input's header (extensions cleared); after the registration the series
is clipped once more and its median over the volumes written as `<in>_average`. The result:
`RobustAverageResult(out_file, out_volumes, out_drift, out_hmc, out_hmc_volumes)`.

## `merge_series`, `reorient_file`, `reorient_image`: `MergeSeries`, `ReorientImage`

| larmorx | niworkflows | Notes |
|---|---|---|
| `merge_series(in_files, allow_4D=True, affine_tolerance=None, *, out_dir=None, n_threads=1)` → the path | `nibabel.MergeSeries` | each file squeezed; 3D images and (with `allow_4D`) 4D ones' volumes merged (nibabel's `concat_images`), `<first>_merged`; another dimension: `ValueError('Input image has an incorrect number of dimensions (…).')`; with `affine_tolerance`, `np.allclose(atol=…)` against the first (`ValueError('Difference in affines greater than allowed tolerance …')`) and the first's affine kept; without, affines must be identical |
| `reorient_file(in_file, *, target_file=None, target_ornt=None, newpath=None, n_threads=1)` → the path | `nibabel.ReorientImage` (`reorient_file`) | to `target_file`'s axis codes or `target_ornt`'s (`TypeError('No target orientation or file is specified.')`), `reoriented.nii.gz` |
| `reorient_image(img, target_ornt)`, `as_reoriented(img, ornt)`, `axcodes2ornt(axcodes)`, `aff2axcodes(affine)`, `ornt_transform(start, end)`, `squeeze_image`, `four_to_three` | `nibabel.reorient_image`; nibabel 5.4.2's functions (MIT) | in memory; nibabel's errors (`Not all axis codes … in label set …`) |

Checked file for file against niworkflows and sdcflows on fMRIPrep's reference runs' PEPOLAR estimators
(every option of `RobustAverage`, its one-volume, 3D and error paths) and against fMRIPrep's own
node files: `validation/investigations/sdcflows-fieldmap-b/check_pepolar.py`.

## `copy_xform`, `copy_header`, `apply_mask`, `key_select`: the header fixes, `ApplyMask`, `KeySelect`

| larmorx | niworkflows | Notes |
|---|---|---|
| `copy_xform(ref_image, out_image, message=None, *, n_threads=1)` → the path | `utils/images._copyxform` | the reference's qform and sform (their codes too; a code 0 changes only the code, as nibabel's `set_qform(None, 0)`) into `out_image`'s header, `descrip` = `xform matrices modified by <message>.`, the image rewritten in place with the reference's affine (nibabel's writer). `FixHeaderApplyTransforms` and `FixHeaderRegistration` call it on their outputs |
| `copy_header(hdr_file, in_file, *, out_dir=".", n_threads=1)` → the path | `header.CopyHeader` | `in_file`'s data with `hdr_file`'s affine and header, in `in_file`'s stored type, `<in>_fixhdr` |
| `apply_mask(in_file, in_mask, *, threshold=0.5, out_dir=".", n_threads=1)` → the path | `nibabel.ApplyMask` | the data as nibabel's `dataobj` reads it times `mask > threshold` (a 4D image: every volume), the header kept, `<in>_masked`; other sizes: `ValueError('Image and mask sizes do not match.')`; other affines (`np.allclose`): `ValueError('Image and mask affines are not similar enough.')` |
| `key_select(keys, key, **fields)` → `{field: value, ..., "key": key}` | `utility.KeySelect` | the item at `key`'s index of each field; niworkflows' `ValueError`s: no fields, duplicated keys, a key not in `keys`, a field that is a string, empty or of another length |

## `concatenate_xfms`: `ConcatenateXFMs`

| larmorx | niworkflows | Notes |
|---|---|---|
| `in_xfms` | `in_xfms` | ITK text (`.txt`, `.tfm`) or MATLAB (`.mat`) files, read as nitransforms' `linear.load(fmt="itk")` reads them (`lx.transforms.load_itk_linear`: the parameters as float32 from text, the centre of rotation as float32); another suffix: `KeyError` (niworkflows' `XFM_FMT`); `.lta`: `NotImplementedError` (FreeSurfer's part) |
| `out_fmt="itk"` | `out_fmt` (`itk`, `fs`) | `fs` (LTA): `NotImplementedError` until FreeSurfer's part |
| `inverse=False` | `inverse` | the inverse too (`np.linalg.inv`'s bits), `out_inv.tfm` |
| `reference`, `moving` | `reference`, `moving` | only the LTA format reads them; checked to exist (nipype's `File(exists=True)`) |
| `out_dir=None` | the node's folder | `out_fwd.tfm` (and `out_inv.tfm`); nothing written without it |
| → `ConcatenateXfmsResult(affine, text, inverse_text, out_xfm, out_inv)` | `out_xfm`, `out_inv` | the files listed in the order they apply, combined as nitransforms' `TransformChain.asaffine` (each next matrix multiplied from the left, numpy's products) and written as nitransforms' `ITKLinearTransform.from_ras(...).to_string()`: `LPS @ ras @ LPS` with `"%g"`, six significant digits, `AffineTransform_float_3_3`, `FixedParameters: 0 0 0` (the centre folded into the translation) |

fMRIPrep's `itk_mat2txt` (`bold_fit_wf`) writes the registration's `transform0GenericAffine.mat`
this way; its `out_fwd.tfm` becomes the `desc-fmap_xfm.txt` derivative that every later
`ReconstructFieldmap` reads. The six digits move the BOLD grid's corners by at most 7.9e-05 mm on
fMRIPrep's reference runs (PIPE-22, rule C: kept, a correction that brings no measurable gain). A file holding several transforms
(a series) is not combined (`NotImplementedError`: fMRIPrep passes single affines). Checked: the 7
`itk_mat2txt` nodes of the earlier runs byte for byte (and their derivatives), niworkflows' own
interface on them with `inverse`, and every input format combined
(`check_bold_steps.py`'s `concat`).

## `skullstrip_bold_workflow`: `init_skullstrip_bold_wf`

| larmorx | niworkflows | Notes |
|---|---|---|
| `in_file` | `inputnode.in_file` | linked into the first node's folder, as nipype links it (a copy where links cannot be made) |
| `out_dir=None` | the workflow's directory | the nodes' folders `skullstrip_first_pass`, `skullstrip_second_pass`, `combine_masks`, `apply_mask`; a temporary folder otherwise (the paths are then `None`) |
| `bet_implementation="auto"`, `maths_implementation="auto"` | | FSL's `bet` and `fslmaths`: the replica when `larmorx-nc` is installed, else the clean room; AFNI's `3dAutomask`: its replica (built in) |
| `upstream_defects="fix"`, `n_threads=1` | | |
| → `SkullstripBoldResult(mask_file, skull_stripped_file, bet_brain, bet_mask, automask_mask, automask_brain, commands)` | `outputnode.mask_file`, `skull_stripped_file` | `commands`: nipype's lines, `bet <in> <in>_brain -f 0.20 -m`, `3dAutomask -apply_prefix <in>_brain_masked.nii.gz -dilate 1 -prefix <in>_brain_mask.nii.gz <in>_brain.nii.gz`, `fslmaths <bet mask> -mul <automask> <bet mask>_maths.nii.gz`; the mask report (`SimpleShowMaskRPT`) belongs with the reports and is not made |

## nipype's command lines (`larmorx._nipype`)

The programs under sdcflows', niworkflows' and fMRIPrep's nodes run with the command line nipype
1.12.0 builds (every environment's nipype, fMRIPrep 25.2.6's included): `larmorx._nipype` formats
the inputs of `ants.Registration` (niworkflows' `FixHeaderRegistration`, from sdcflows' settings
files: `lx.sdcflows.registration_settings`), `ants.ApplyTransforms` (`FixHeaderApplyTransforms`),
`ants.N4BiasFieldCorrection`, `ants.ImageMath`, `ants.DenoiseImage`, `fsl.BET`,
`fsl.BinaryMaths`, `fsl.MultiImageMaths`, `fsl.TOPUP` (its encoding file), `afni.Automask` and
`afni.Volreg` in nipype's order (positions first, then by name, negative positions last), with
nipype's file names (`name_source`, `genfile`, the output types' extensions), its shell quoting
and its `_list_outputs`, and runs them in the node's folder through larmorx's ports (ANTs in this
process, FSL's replicas or clean rooms, AFNI's replicas). Settings files are read as
nipype's `from_file` reads them, their values converted as nipype's input traits convert them
(a JSON `6` for a float stage setting is written `6.0`). `antsRegistration` takes fMRIPrep's
`ANTS_RANDOM_SEED` as `ants_random_seed` (ANTs seeds from the clock without one). Checked against
every `command.txt` of fMRIPrep's reference runs for these interfaces (1,583 nodes, the encoding files of
TOPUP against nipype's own) and against nipype's own `cmdline` and outputs on 14 option sets the
runs do not use: `validation/investigations/sdcflows-fieldmap-b/check_nipype.py`. Not ported:
nipype's version checks (the singular `fixed_image_mask`/`moving_image_mask` of ANTs <= 2.1.0
raise), the `xor`/`requires` checks that do not change the line. `antsApplyTransforms` runs on
files through larmorx's port of the program (`_core.ants_file_program`, an explicit thread count:
its result never depends on it), then `FixHeaderApplyTransforms` copies the reference's x-forms
onto the output (`copy_xform`).

**File names with a blank (PIPE-80, corrected by default).** nipype writes some file names
unquoted: the files inside ANTs' bracketed arguments (`--transform [ f, 1 ]`, `--metric MI[ fixed,
moving, ... ]`, `--masks [ f, m ]`, `--initial-moving-transform [ f, 0 ]`, the `--output [ ... ]`
forms of `antsRegistration`, `antsApplyTransforms`, `N4BiasFieldCorrection` and `DenoiseImage`),
`antsApplyTransforms`' `--transform f`, `MultiImageMaths`' operands and TOPUP's `--datain` file;
and BET makes its input's name relative after quoting it. The line runs through the shell, so a
dataset, working directory or output folder whose path holds a blank (or a quote) gives the
program other file names, and the node fails. Each `Command` keeps nipype's line (`cmdline`, as
`command.txt` holds it) and the line with every file name quoted (`quoted_cmdline`; the same text
when no name needs quoting); `larmorx._nipype.words(cmd, upstream_defects)` gives the program the
quoted line's words by default and nipype's with `upstream_defects="reproduce"` (nipype's
failure), with the `PIPE-80` notice whenever the two differ. The output names larmorx makes
absolute for a program that runs in this process (nipype writes them relative to the node's
folder) are quoted in both modes, so a node folder with a blank works as in nipype.

## The EPI reference and the steps under it (SyN preprocessing)

| larmorx | niworkflows / nipype / nilearn | Notes |
|---|---|---|
| `validate_image(in_file, *, out_dir=".", upstream_defects="fix", n_threads=1)` → `ValidateImageResult(out_file, out_report, warning)` | `interfaces/header.ValidateImage` | the truth table: matching valid x-forms with codes above 0 (or a qform that only differs by a qform's precision): the input and an empty `report.html`; otherwise `<in>_valid` with the sform copied from a valid qform, the qform from a valid sform (the report gives the change's angle and translation: transforms3d's `decompose44` as it computes it, the axis from `numpy.linalg.eig`), or both set to nibabel's base affine with code 1; the HTML snippet indented by three tabs. An sform is invalid when numpy's determinant is 0 (LAPACK's LU of the 3 x 3 block) or its zooms are not `np.allclose` to `pixdim`; a qform when nibabel's `get_qform` raises (an impossible quaternion). PIPE-24 |
| `nonsteady_states_detector(in_file, nonnegative=True, n_volumes=40, zero_dummy_masked=20, *, numpy_version, upstream_defects="fix", n_threads=1)` → `NonsteadyStatesResult(t_mask, n_dummy)` | `interfaces/bold.NonsteadyStatesDetector` | the first `n_volumes` (10 to 200) clipped to `[0, p99.8]` (`[p0.2, p99.8]` without `nonnegative`), each volume's mean in numpy's summation order (`numpy_version`: 2.2's buffered sums), nipype's `is_outlier`; fewer than two non-steady states: the last `zero_dummy_masked` (2 to 40) of the first `n_volumes` are averaged; a 3D image: `(True,)`, 1 |
| `is_outlier(points, thresh=3.5, *, upstream_defects="fix")` → `int` | nipype 1.12.0 `algorithms/confounds.is_outlier` | the leading observations whose modified z-score `0.6745 * abs(x - median) / MAD` exceeds `thresh`, in the points' type (numpy's medians: `larmorx_core::numpy`); PIPE-23 |
| `binarize(in_file, thresh_low, *, out_dir=".", n_threads=1)` → `BinarizeResult(out_file, out_mask)` | `interfaces/nibabel.Binarize` | `get_fdata()` above `thresh_low` (the rest 0) in the input's header and type, `<in>_masked`; the mask as `uint8`, `<in>_mask` |
| `generate_sampling_reference(fixed_image, moving_image, *, xform_code=None, fov_mask=None, keep_native=True, target_resolution=None, out_dir=".", niworkflows_version="1.15.4", nilearn_version="0.14.1", scipy_version="1.18.1", n_threads=1, upstream_defects="fix")` → path | `interfaces/nibabel.GenerateSamplingReference` (`_gen_reference`) | the fixed image's field of view on a RAS grid of the moving image's voxel sizes after nibabel's `as_closest_canonical` (or `target_resolution`), rounded to 0.001 mm; the fixed image resampled onto it (nearest); `fov_mask` crops the grid to the mask's bounding box plus two voxels ("Fixed image and mask do not have the same dimensions.", "... different affines"); qform and sform of the grid with the sform's code (2 for 1; `xform_code` 2 or 4 forces it); `descrip` names niworkflows' version; `<fixed>_reference`. `nilearn_version`: 0.13 onwards copies the fixed image's header (written unscaled, `scl_slope = scl_inter = 0`), 0.11.1 (fMRIPrep 25.2.6's image) makes a new one and copies by whole voxels where the grids differ by whole voxels |
| `resample_img_nearest(img, target_affine, target_shape=None, *, copy=False, nilearn_version, scipy_version, n_threads=1, upstream_defects="fix")` → `Image` | nilearn 0.14.1 / 0.11.1 `image.resample_img(interpolation="nearest")` | the grid's bounds as OpenBLAS's `dgemm` adds them (one fused multiply-add per term), `BoundingBoxError`, the "no sform" warning, non-finite values (NaN where the voxel taken was not finite, with nilearn's warning), the input itself when nothing needs resampling. nilearn's two inverses of the image's affine: numpy's (`np.linalg.inv`, OpenBLAS's `dgesv`) for the bounds and SciPy's (`scipy.linalg.inv`, `larmorx.transforms._linalg.scipy_inv`) for the sampling points: SciPy 1.18.1's structure detection (a plumb affine is upper triangular: OpenBLAS's `trti2`; an oblique one `getrf`/`getri`; a symmetric one, an axis swap with a zero origin, Cholesky or Bunch-Kaufman) with its `LinAlgWarning` from LAPACK's condition estimates (`gecon`, `trcon`, `pocon`, `sycon`, the `rcond` it prints), or 1.15's `getrf`/`getri` with `scipy_version`, through SciPy's own OpenBLAS (0.3.31.dev, 0.3.28) on the Haswell kernels: identical to SciPy on 27 families of 20,000 matrices and 513 image affines of fMRIPrep's reference runs per environment, every `rcond` identical to SciPy's LAPACK calls (`check_scipy_inv.py`; SCI-18, SCI-19, SCI-20) |
| `regrid_to_zooms(in_file, zooms, *, order=3, clip=True, smooth=False, out_dir=None, numpy_version, n_threads=1)` → `RegridToZoomsResult(out_file)`; `resample_by_spacing(in_file, zooms, order=3, clip=True, smooth=False, *, numpy_version, n_threads=1)` → `Image` | `interfaces/images.RegridToZooms` (= `interfaces/nibabel.RegridToZooms`), `utils/images.resample_by_spacing` | the field of view on a grid of voxel sizes `zooms` (`ceil(extent / zooms)` voxels, centred): the `float32` cardinal matrix of the header's `pixdim`, its inverse numpy's (computed in `float64` and cast, as numpy does for single-precision input), the sampling points as numpy's `dgemm` adds them, SciPy's `map_coordinates` (`order`, `mode='constant'`, prefiltered) of the file's `dataobj` or, with `smooth`, of `gaussian_filter(get_fdata(), sigma)` (`True`: `max(0, (old / new - 1) / 2)` voxels, PIPE-35; a number: that sigma), `clip` to the input's range; the header's zooms set, the qform and sform carried by `new_affine · inv(affine) · xform` with their codes (both `new_affine`, code 1, when neither is set), written in the header's stored type (scaled); `<in_file>_regrid`. Voxel sizes that are not normal `float32` numbers: nibabel's `HeaderDataError` ("Could not decompose affine:", its matrix not niworkflows'); a 4-D input: SciPy's "invalid shape for coordinate array". Identical to niworkflows on 576 seeded files and on fMRIPrep's 51 own nodes (`res_tmpl`, `res_target` of every run's brain extraction, `zooms_epi` of the SyN runs) in both environments (`check_regrid.py`) |
| `epi_reference_workflow(in_files, t_masks=None, auto_bold_nss=False, *, out_dir, omp_nthreads=1, numpy_version, volreg_implementation="auto", upstream_defects="fix", n_threads=1)` → `EpiReferenceResult` | `workflows/epi/refmap.init_epi_reference_wf` | the nodes `validate_nii`, `select_volumes` (with `auto_bold_nss`), `per_run_avgs` (`RobustAverage`), `clip_avgs`, `n4_avgs` (N4 with nipype's `copy_header`), `clip_bg_noise` (`IntensityClip(2, 100)`), `epi_merge` and `post_merge`, in nipype's map-node folders; one input: `StructuralReference`'s shortcut (its identity LTA, written by FreeSurfer's `lta_convert` and read by nothing on these paths, is not produced: `xfm_files` is `None`); several inputs need FreeSurfer's `mri_robust_template` (`NotImplementedError` until the FreeSurfer part) |

Checked on fMRIPrep 25.2.6's `--use-syn-sdc` runs node by node and as a whole
(`validation/investigations/sdcflows-fieldmap-b/check_syn_preprocessing.py`) and against
niworkflows, nipype and nilearn in both kept environments on seeded inputs (its `library`
section: every row of `ValidateImage`'s table, oblique, plumb, LPS, 4D and non-finite images for
`GenerateSamplingReference` with and without its options).

## `derivatives_data_sink`: `DerivativesDataSink` (the derivatives' names and files)

`python/larmorx/niworkflows/bids.py`: niworkflows 1.15.4's `interfaces/bids.DerivativesDataSink`
(the same in 1.15.2, fMRIPrep 25.2.6's image, and 1.15.3) with its `utils/misc._copy_any`,
`utils/images.unsafe_write_nifti_header_and_data` and `set_consumables`, `data/nipreps.json`
(the derivatives' entities and patterns, [`lx.bids`](bids.md)), pybids 0.22.0's
`parse_file_entities` and `build_path` (`lx.bids.build_path`), nipype 1.12.0's `copyfile` and
nibabel 5.4.2's header setters and writer.

| larmorx | niworkflows | Notes |
|---|---|---|
| `derivatives_data_sink(in_file, source_file, ...)` → `DerivativesDataSinkResult(out_file, out_meta, compression, fixed_hdr)` (`.out`: the path for one file, as nipype passes it on) | `DerivativesDataSink().run()` | one call is one run of the interface |
| `in_file`, `source_file` | the same | a missing `in_file` is a `FileNotFoundError` (nipype's trait check) |
| `base_directory=None` | the same | `None`: the current directory (nipype's node directory) |
| `out_path_base="niworkflows"` | the class attribute (sdcflows' subclass: `'sdcflows'`; fMRIPrep sets `''` on its sinks) | |
| `check_hdr=True`, `compress=()`, `data_dtype=None`, `dismiss_entities=()`, `meta_dict=None` | the inputs of the same names | `compress`: one value for every file, or one per file (PIPE-28); `data_dtype`: a numpy type, `'source'` (the first source's), or by default the suffix's (`DEFAULT_DTYPES`: `mask` uint8, `dseg` int16, `probseg` and `boldref` float32) |
| `allowed_entities=()`, `entities={...}` | the constructor's `allowed_entities` and the entity inputs (`desc`, `suffix`, `datatype`, `space`, `resolution`, `fmapid`, ..., and the allowed ones) | a name that is neither (`ValueError`) belongs in `metadata` |
| `metadata={...}`, `init_metadata={...}` | the metadata inputs set on the interface, and the constructor's other keyword arguments | the sidecar: `meta_dict`, then `init_metadata`, then `metadata`; only `metadata`'s `RepetitionTime` sets the header's fourth voxel size, as niworkflows' input does |
| `standard_spaces=STANDARD_SPACES`, `template_metadata=None`, `templateflow_version=None` | TemplateFlow's `templates()` and `get_metadata(space)`, its version | TemplateFlow 25.1.2's list (both kept environments'); a standard space's `resolution` needs the template's `template_description.json` metadata (`NotImplementedError` without it: TemplateFlow comes with larmorx's templates part); `get_tf_resolution(space, resolution, metadata, version)` gives the text |
| `copy_any(src, dst)` | `utils/misc._copy_any` | gzip level 9, no name, time stamp 0; plain files: a hard link to the real file, else a copy, with the related `.mat`, `.img`, `.HEAD` files (nipype's `copyfile`) |

What it does, as niworkflows: the entities common to every source (`relative_to_root` of each,
with pybids' `bids` and `derivatives` configurations and `nipreps.json`'s), less
`dismiss_entities`, the input files' extensions (compressed or not by `compress`), the entity
inputs on top; `resolution: native` dropped with a `space`; custom entities inserted before the
suffix of every pattern; the first `nipreps.json` pattern that builds a path for each file
(`ValueError` "Could not build path ..." or "Input files (n) not matched ..."). A file already
at its destination is left. A NIfTI file (nibabel's `load` gives a `Nifti1Image`: the NIfTI-1
magic `ni1`/`n+1`, or a NIfTI-2 header that is not CIFTI-2) is rewritten when `check_hdr` finds
its xform codes (1, or 4 for a standard `space`, 2 for another), units (`mm` kept or set,
`sec` for `bold`, else none) or voxel sizes not a derivative's (the qform and sform set from its
affine with nibabel's arithmetic, its scaling and data offset kept, its stored data copied), or
when its data type is not `data_dtype` (the scaled data rounded and cast for an integer type,
scaling 1 and 0); otherwise it is copied. One file gets a sidecar `<name>.json` with the metadata
(keys sorted, two-space indents). The files are niworkflows' bytes: header block, extensions,
padding and data the same, and the gzip stream Python's (the compressed bytes are the platform's
zlib's; the contents are the same everywhere).

Checked (`validation/investigations/sdcflows-fieldmap-b/check_derivatives.py`): every
field-map sink of fMRIPrep's reference runs (95 files of 19 estimators, fMRIPrep 25.2.6's image)
and of sdcflows 2.17.1's own BIDS-app runs (75 files of 15 estimators), given each node's own
inputs: every name, uncompressed and gzip byte and sidecar identical (every one of them fixed the
header: the estimators' files carry `sec` units or other codes); and 38 edge cases against
niworkflows' own sink in this environment (`synthetic`: copies and (de)compression with the
related `.mat`, xform codes and oblique affines, units, `RepetitionTime` as input and in
`meta_dict`, spaces and resolutions with TemplateFlow's metadata, masks, `dseg`, `'source'`,
scaled and big-endian data, extensions, NIfTI-2, the `ni1` and an unknown magic, a non-NIfTI file,
several files and sources, `dismiss_entities`, the errors): all identical, the defects' cases with
`upstream_defects="reproduce"`.

## `robust_acompcor`, `robust_tcompcor`: `RobustACompCor`, `RobustTCompCor`

nipype's `ACompCor` and `TCompCor` as fMRIPrep runs them (`docs/api/confounds.md`):
`lx.niworkflows.robust_acompcor(realigned_file, mask_files=None, **kwargs)` is
`lx.confounds.acompcor(realigned_file, mask_files, **kwargs)`, `robust_tcompcor` is
`lx.confounds.tcompcor`, their files bit for bit (`check_nipype_misc.py`'s `robust` section).
PIPE-48 (kept, no switch): niworkflows reruns the interface after a `numpy.linalg.LinAlgError`, up
to ten more times after random sleeps (about 8 to 9 minutes in all), then raises the last error;
larmorx's computation is deterministic, so it raises that error at once.

## `add_tsv_header`, `dict_merge`, `tsv2json`: `AddTSVHeader`, `DictMerge`, `TSV2JSON`

niworkflows 1.15.4's `interfaces/utility.py` (the same in 1.15.2), under fMRIPrep's confounds
(`docs/api/confounds.md`), in `lx.niworkflows` (with golden cases since 2026-11-12, the
confounds' part).

| larmorx | niworkflows | Notes |
|---|---|---|
| `add_tsv_header(in_file, columns, *, out_dir=".", upstream_defects="fix")` → the path | `AddTSVHeader(in_file, columns)` → `out_file` | numpy's `loadtxt` (whitespace-separated, `#` comments) and `savetxt` (`%.18e`, tabs, the header `"\t".join(columns)`), `<stem>_motion.tsv` in `out_dir`; an empty table: numpy's warning and the header alone; PIPE-61, PIPE-62 (below) |
| `dict_merge(in_dicts)` → a dictionary | `DictMerge(in_dicts)` → `out_dict` | the keys in order, a later dictionary's value winning; an element that is not a dictionary: traits' message as a `ValueError` |
| `tsv2json(in_file, index_column, *, output=UNDEFINED, additional_metadata=None, drop_columns=None, enforce_case=True, out_dir=".", pandas_version="3.0.6", upstream_defects="fix")` → a dictionary or the path | `TSV2JSON` (`_tsv2json`) → `output` | the table read as pandas reads it (PD-01), `additional_metadata` set as columns (before `drop_columns`, which raises pandas' `KeyError` for a missing one) and again on every entry, keyed by `index_column` (a table without it keeps its row numbers, which `enforce_case` cannot turn to snake case: Python's `AttributeError`, as niworkflows), with `enforce_case` the keys in snake case and the fields title-cased then camel-cased (`Csf` → `CSF`); pandas' `ValueError` for keys or fields that are not unique; the numbers as pandas' `to_json` writes them and Python's `json` reads them back (10 decimals: PD-02), NaN and infinities `None`; `output=None`: the ordered dictionary, else written as JSON (indent 4, `\n` line ends), left undefined `<stem>.json` in `out_dir` |

## `expand_model`, `spike_regressors_node`, `normalize_motion_params`: `ExpandModel`, `SpikeRegressors`, `NormalizeMotionParams`

niworkflows 1.15.4's `interfaces/confounds.py` (the same in 1.15.2 and 1.15.3 on these
functions), `larmorx.niworkflows.confounds`: the last two steps of fMRIPrep's confounds
(`docs/api/confounds.md`): `model_expansion` (`ExpandModel` with fMRIPrep's formula
`(dd1(rps + wm + csf + gsr))^^2 + others`) and `spike_regressors` (`SpikeRegressors`), whose
output is fMRIPrep's `desc-confounds_timeseries.tsv`. In `lx.niworkflows` (with golden cases
since 2026-11-12, the confounds' part). The tables are read and written as pandas reads and
writes them (`docs/api/confounds.md`, "pandas' tables": PD-01); `NormalizeMotionParams`' and
`AddTSVHeader`'s text files (numpy's `savetxt`) as numpy writes them on Linux, on every platform
(UTF-8, `\n` line ends; numpy's text mode writes `\r\n` on Windows).

| larmorx | niworkflows | Notes |
|---|---|---|
| `expand_model(confounds_file, model_formula=DEFAULT_FORMULA, *, output_file=None, out_dir=".", pandas_version="3.0.6", upstream_defects="fix")` → the path | `ExpandModel(confounds_file, model_formula, output_file)` → `confounds_file` | `parse_formula(..., unscramble=True)` of the table; `output_file` relative to `out_dir`, by default `<stem>_expansion.tsv` in `out_dir`; a formula that is not a string: traits' message as a `ValueError` |
| `spike_regressors_node(confounds_file, *, fd_thresh=0.5, dvars_thresh=1.5, header_prefix="motion_outlier", lags=(0,), minimum_contiguous=None, concatenate=True, output_format="spikes", output_file=None, out_dir=".", pandas_version="3.0.6", upstream_defects="fix")` → the path | `SpikeRegressors(...)` → `confounds_file` | `spike_regressors` with `framewise_displacement > fd_thresh` and `std_dvars > dvars_thresh` (pandas' `KeyError` when a column is missing); by default `<stem>_desc-motion_outliers.tsv`; traits' messages for inputs of the wrong type |
| `normalize_motion_params(in_file, *, format="FSL", out_dir=".", numpy_version="2.5.4", upstream_defects="fix")` → the path | `NormalizeMotionParams(in_file, format)` → `out_file` | numpy's `loadtxt`, every row through `lx.confounds.normalize_mc_params(row, format)` (`"FSL"`, `"AFNI"` with a leading index column when there are seven, `"FSFAST"`, `"NIPY"`: nipy 0.6.1's own arithmetic, `docs/api/confounds.md`), numpy's `savetxt` (`%.18e`, spaces) as `motion_params.txt`; `"SPM"` and other formats refused as niworkflows' `Enum` refuses them; one column, one value or an empty file: numpy's `AxisError`, too few columns numpy's `IndexError`, as niworkflows; PIPE-70 |
| `parse_formula(model_formula, parent_data, unscramble=False, *, upstream_defects="fix")` → `ModelTerms(variables, data)` | `parse_formula` → `(variables, data)` | `parent_data` a `ConfoundTable` or a mapping of names to columns (1-D float64, int64, uint64, boolean or object arrays, lists of text); the result unpacks as niworkflows' tuple; `data` a `ConfoundTable` (names in order, one column per name); the grammar below; PIPE-43, PIPE-64, PIPE-65, PIPE-66 |
| `temporal_derivatives(order, variables, data, *, upstream_defects="fix")`, `exponential_terms(...)` → `ModelTerms` | `temporal_derivatives`, `exponential_terms` → `(variables, data)` | `<v>_derivative<o>`: numpy's `diff` `o` times, the first `o` rows NaN, order 0 the variable; `<v>_power<o>`: numpy's `x ** o`, power 1 the variable; the orders taken as niworkflows takes them (a set when 0 or 1 is among them: ascending) |
| `spike_regressors(data, criteria=None, header_prefix="motion_outlier", lags=None, minimum_contiguous=None, concatenate=True, output="spikes", *, upstream_defects="fix")` → `ConfoundTable` | `spike_regressors` → a `DataFrame` | `criteria`: column → (`"<"` or `">"`, threshold), by default FD > 0.5 or standardised DVARS > 1.5 (Power et al. 2014; NaN never flagged); the flagged volume always, and `lags` after it; `minimum_contiguous`: every shorter run of unflagged volumes flagged too; one regressor per flagged volume (`<prefix><kk>`, 1 at that volume) or, with `output="mask"`, one column of 1s at the flagged volumes; a table without rows: Python's `ValueError` for the regressors (the mask works), as niworkflows; PIPE-67, PIPE-68, PIPE-69 |

**The formula.** Terms are added with `+`. A term is a column name, a shorthand (`rps` the six
motion parameters `trans_x ... rot_z`; `wm`, `csf`, `gsr` the white-matter, CSF and global
signals; `fd` the framewise displacement; `acc`, `tcc` the `a_comp_cor_<n>` and `t_comp_cor_<n>`
columns; `dv` `std_dvars`; `dvall` every column whose name holds `dvars`; `nss` the
`non_steady_state_outlier<n>` columns; `spikes` the `motion_outlier<n>` columns; `others` every
column the formula does not name, in the table's order) or a group `( ... )` with temporal
derivatives before it (`dd<n>`: the orders 0 to n; `d<n>`: the n-th; `d<n>-<m>`: n to m) and
powers after it (`^^<n>`: 1 to n; `^<n>`; `^<n>-<m>`). An operator applies to the whole group
(`(dd1(rps))^^2`: the motion parameters, their first derivatives and the squares of both; the
power before the derivative when both adjoin one group: `d1(x)^2` is `x_power2_derivative1`). A
column must not be named like an operator for niworkflows (PIPE-65).

**The arithmetic.** A derivative is numpy's `diff` (exact subtractions, integers in their type,
booleans as numpy's `not_equal`); a power of a float64 column is numpy's `x ** o`: `x * x` for
2, 1 for 0, otherwise glibc 2.35's `pow` with `upstream_defects="reproduce"` (numpy's on CPUs
without AVX-512) and the correctly rounded `pow` by default (GLIBC-07, its notice when a value
differs: glibc's `pow` is not correctly rounded on about 0.4 % of cubes, measured by
`check_model.py`'s `power` section). On x86-64 CPUs with AVX-512, numpy's wheels compute such
powers with Intel's SVML instead, which larmorx does not follow (NP-04's class; fMRIPrep's
formula only squares). Integer, boolean and text columns follow numpy's own `**` (wrapping
integers; text: Python's `TypeError`); a group's columns are put side by side in pandas' common
type first (an integer column beside a float one becomes float).

**The column order (PIPE-43).** niworkflows orders a group's variables, and the `others`, as a
Python set of strings, whose order changes from one process to the next (Python's per-process hash
seed): in fMRIPrep's files a variable's square comes before or after the square of its derivative
from run to run (169 and 155 times in the 36 confounds files of fMRIPrep's reference runs). larmorx keeps the formula's order in
both modes: each variable, its derivatives, then their powers in the same order (`x`,
`x_derivative1`, `x_power2`, `x_derivative1_power2`), one order niworkflows can produce; its
notice says so (never reproduced: nondeterminism). The values do not depend on it; compare such
files by column name.

## `fsl_motion_params`, `fsl_rms_deviation`, `framewise_displacement`: niworkflows' copies of fMRIPrep's motion interfaces

`lx.niworkflows` (`larmorx.niworkflows.confounds`; with golden cases since 2026-11-12).
niworkflows' `interfaces/confounds.py` carries fMRIPrep's
`FSLMotionParams`, `FSLRMSDeviation` and `FramewiseDisplacement` (unused by fMRIPrep, which has its
own): `fsl_motion_params(xfm_file, boldref_file, *, out_dir=".", niworkflows_version="1.15.4",
numpy_version="2.5.4", upstream_defects="fix", n_threads=1)` is
`larmorx.confounds.fsl_motion_params` with fMRIPrep 25.2.7's form for niworkflows 1.15.4 and 25.2.6's
for 1.15.2 and 1.15.3 (niworkflows 1.15.4 made fMRIPrep 25.2.7's change in its copy);
`fsl_rms_deviation` and `framewise_displacement` are `larmorx.confounds.fsl_rms_deviation` and
`fmriprep_framewise_displacement` (the same in every release). The options, the arithmetic and the
defects (PIPE-71, PIPE-72, PIPE-73): `docs/api/confounds.md`, "fMRIPrep's motion parameters".

## `binary_dilation`, `binary_subtraction`, `image_binary_dilation`, `signal_extraction`: `BinaryDilation`, `BinarySubtraction`, `SignalExtraction`

`lx.niworkflows` (`larmorx.niworkflows.morphology` and `larmorx.niworkflows.images`; with golden
cases since 2026-11-12).
niworkflows 1.15.4's `interfaces/morphology.py` and `interfaces/images.py` (Apache-2.0, the NiPreps
developers; the same in 1.15.2 and 1.15.3), with SciPy's `binary_dilation`, scikit-image's `ball`,
nibabel and numpy, bit for bit. fMRIPrep's confounds make the crown mask with the first two (the brain
mask dilated by `ball(2)` minus the mask) and the global, CSF, white-matter, CSF + WM and tCompCor
signals with the third.

| larmorx | niworkflows | Notes |
|---|---|---|
| `binary_dilation(in_mask, *, radius=2, out_dir=".", n_threads=1, upstream_defects="fix")` → path | `BinaryDilation(in_mask, radius=2)` → `out_mask` | `np.bool_(dataobj)` (every non-zero value inside, NaN too) dilated by `image_binary_dilation`, written as `uint8` with the mask's header and affine to `dilated_mask.nii.gz`; `radius` a `traits.Int` (`True` is 1; a float: traits' `TraitError` as `ValueError`); a mask that is not 3-D: SciPy's `RuntimeError: structure and input must have same dimensionality` |
| `image_binary_dilation(in_mask, radius=2, *, n_threads=1, upstream_defects="fix")` → array | `image_binary_dilation(in_mask, radius)` | `ndi.binary_dilation(in_mask.astype(bool), ball(radius)).astype(int)` (`int64`); any real radius (`ball(2.5)` is 6 × 6 × 6) |
| `binary_subtraction(in_base, in_subtract, *, out_dir=".", n_threads=1)` → path | `BinarySubtraction(in_base, in_subtract)` → `out_mask` | `np.bool_` of the base set to false where `np.bool_` of the other is true (numpy's boolean indexing: other shapes stop with its `IndexError`), `uint8` with the base's header to `subtracted_mask.nii.gz` |
| `signal_extraction(in_file, label_files, class_labels, *, prob_thres=0.5, out_file="signals.tsv", out_dir=".", numpy_version="2.5.4", n_threads=1, upstream_defects="fix")` → path | `SignalExtraction(in_file, label_files, class_labels, prob_thres, out_file)` → `out_file` | several label files (or one 4-D file of several volumes: nibabel's `four_to_three`): probability maps, each region the voxels `>= prob_thres` in the map's stored type (a Python float against it: numpy's weak scalar; `prob_thres` in `[0, 1]`, else traits' text); one 3-D file: a label image, one region per non-zero value in ascending order (`np.unique`). Each column is `data[mask, :].mean(axis=0)` of `get_fdata()` (a `float32` series widened exactly): the voxels in the mask's C order summed per volume from `+0.0` in that order (pairwise for a series of one volume, in chunks of 8,192 before numpy 2.3: `numpy_version`), divided by the count; an empty region gives NaN (`nan` in the table). larmorx reads the voxels where they are (no `(N, T)` copy) on `n_threads` threads (the result does not depend on them). The table: the labels, then numpy's `astype(str)` of each mean (Python's shortest `repr`), tabs between, `\n` line ends, UTF-8 (numpy's `savetxt` in a text file: the locale's encoding, UTF-8 on Linux; larmorx writes Linux's bytes on every platform). niworkflows' errors are kept: the first map's shape against the series' (`NotImplementedError`), the number of regions against the labels (`ValueError`), a 3-D series (`IndexError: tuple index out of range`), a later map of another shape or more axes (numpy's `IndexError`), a series without volumes (nibabel's empty array: numpy's `IndexError`) |

Corrected by default (`upstream_defects="reproduce"` gives niworkflows' result, with a notice):

| ID | niworkflows does | larmorx by default |
|---|---|---|
| PIPE-77 | a negative `radius` (`traits.Int` takes it) dilates with scikit-image's `ball` of that radius, an arbitrary footprint: -1 is one false element, so the dilated mask is empty; -2 the cross of radius 1; -3 the ball of radius 2 (rule B) | `ValueError` |
| PIPE-78 | a class label holding a tab or a line break is written as it is, which breaks the table's columns or rows (rule B) | `ValueError` |

Kept and documented (rule C): the regions' and masks' affines are not compared with the series'
(niworkflows works on voxel indices); `np.bool_` takes NaN as inside.

**Validation**: `docs/api/confounds.md`, "fMRIPrep's masks".

## Upstream defects of these steps (D13)

| ID | Defect | larmorx by default (`upstream_defects="reproduce"`: the original) | Functions |
|---|---|---|---|
| PIPE-23 | nipype's `is_outlier` divides by the median absolute deviation: 0 when more than half of the values are equal (a constant signal, a padded series), so the values at the median get `0/0 = NaN`, fail the `<= thresh` test and count as outliers; `NonsteadyStatesDetector` then calls every leading volume non-steady (rule A) | the values at the median have a z-score of 0 (not outliers; the others, away from a zero spread, stay infinite) | `is_outlier`, `nonsteady_states_detector`, `epi_reference_workflow` |
| PIPE-24 | `ValidateImage`'s report names the translation's unit with nibabel's `get_xyzt_units`, which raises `KeyError` for a spatial unit code 4 to 7 (rule A: a readable header) | the translation reported in mm, as for an unknown unit | `validate_image` |
| PIPE-26 | `DerivativesDataSink` with two or more list-valued entity inputs matches the files to the names in the order of a Python `set` of the entity names, which changes between processes | the names in the entities' sorted order (nondeterminism is never reproduced; a warning says so) | `derivatives_data_sink` |
| PIPE-27 | its warning about a changed data type names the original type twice ("from float32 to float32") | the new type named (rule A, a message) | `derivatives_data_sink` |
| PIPE-28 | `compress` with fewer values than files (but more than one) fails with Python's `IndexError` | a `ValueError` saying so (rule B) | `derivatives_data_sink` |
| PIPE-29 | the data type coercion casts values out of the integer type's range and NaN with numpy's cast (undefined in C; on x86-64 a wrapped or arbitrary value) | clamped to the range, NaN to 0, with a warning (the rule for undefined conversions) | `derivatives_data_sink`, `fmap_derivatives_workflow` |
| PIPE-30 | a `RepetitionTime` input for an image that is not 4-D makes the header fix set four voxel sizes on it, and nibabel's `HeaderDataError` stops the sink (rule A: a 3-D reference of a BOLD series is a sensible input) | the fourth voxel size set only on an image with a fourth axis (its other sizes kept), the sidecar with the value | `derivatives_data_sink` |
| PIPE-35 | `resample_by_spacing(smooth=True)` sets the Gaussian's sigma to `max(0, (old / new - 1) / 2)` voxels, positive only when the new grid is finer: a coarser grid (the anti-aliasing case) is never smoothed, so fMRIPrep's brain extraction regrids its 1 mm images to 4 mm without smoothing (scikit-image's `rescale` takes `(new / old - 1) / 2`) | rule D: niworkflows' sigma in both modes until the anatomical brain extraction's port measures whether smoothing changes its initialisation | `regrid_to_zooms`, `resample_by_spacing` |
| PIPE-61 | `AddTSVHeader` reads with numpy's `loadtxt`, which squeezes a table of one row to a vector, written by `savetxt` as one column (the row's values one under the other), and a table of one value to a scalar, which `savetxt` refuses (`ValueError: Expected 1D or 2D array, got 0D array instead`) (rule A) | the table's shape kept: one row stays a row, one value is written | `add_tsv_header` |
| PIPE-62 | `AddTSVHeader` writes a header with another number of names than the table's columns as given: the file's header and rows disagree (rule B) | a `ValueError` | `add_tsv_header` |
| PIPE-43 | `parse_formula` orders the variables of each group, and the `others`, as a Python set of strings, whose order changes between processes, so `ExpandModel` writes a variable's expansions in another order from one run to the next | the formula's order in both modes (nondeterminism is never reproduced; a notice says so) | `parse_formula`, `expand_model` |
| PIPE-64 | `_expand_shorthand` replaces the shorthands as text, anywhere and again inside every group: a name holding one is rewritten (`std_dvars` becomes `std_std_dvarsars`, once more in each group), `dvall` becomes `std_dvarsall` (`dv` is replaced first), a shorthand (or `others`) that matches no column leaves an empty term (`IndexError: string index out of range`), and the names `others` leaves out lose every `d<digits>` and `^<digits>` (a column `head1` is then repeated) (rule A) | whole names replaced, once, at the top; a shorthand that matches nothing adds no column | `parse_formula`, `expand_model` |
| PIPE-65 | a term's operators are read with regular expressions over the whole term: text around a group other than a derivative before it and an exponent after it is ignored (`foo(a)`, `(a)b`, `d1x(a)`, `d1(a)(b)`, `dd1-2(a)`, `(a)^ ^2`), an exponent after a group that holds another single-caret exponent fails (`((a)^2)^3`: `TypeError`), and a name that starts like a derivative or ends like an exponent (`d1a`, `a^2`) is taken for an operator without its group (`TypeError`) (rule B for the ignored text, A for the others) | a `ValueError` for text around a group; the exponent after the group read alone; a term without a group read as a name | `parse_formula`, `expand_model` |
| PIPE-66 | a variable reached by two terms (`a + (a)^^2`) is repeated: twice in `parse_formula`'s table, four times in `ExpandModel`'s file (pandas selects every column of a repeated name each time it is named), and a group that repeats one fails with pandas' `ValueError: Shape of passed values ...` (rule A) | each column once (its first) | `parse_formula`, `expand_model` |
| PIPE-67 | `spike_regressors` adds each lag to the volumes the earlier lags flagged: lags `[1, 2]` (or `[0, 1, 2]`) flag the third volume after a spike too (rule A) | every lag added to the flagged volumes only | `spike_regressors`, `spike_regressors_node` |
| PIPE-68 | `minimum_contiguous` takes the table's end one volume too far, so the last run of unflagged volumes counts one volume more and a final run one shorter than the minimum is kept (rule A) | the last run counted as it is | `spike_regressors`, `spike_regressors_node` |
| PIPE-69 | a criterion other than `'<'` and `'>'` is skipped without an error (all of them: `TypeError: reduce() of empty iterable with no initial value`) (rule B) | a `ValueError` | `spike_regressors` |
| PIPE-70 | `NormalizeMotionParams` reads with numpy's `loadtxt`, which squeezes a table of one row to a vector, which `apply_along_axis` refuses (`AxisError: axis 1 is out of bounds for array of dimension 1`) (rule A) | the row converted | `normalize_motion_params` |
| PIPE-77 | `BinaryDilation` and `image_binary_dilation` with a negative radius dilate with scikit-image's `ball` of it, an arbitrary footprint (-1: one false element, the dilated mask empty) (rule B) | a `ValueError` | `binary_dilation`, `image_binary_dilation` |
| PIPE-78 | `SignalExtraction` writes a class label holding a tab or a line break as it is, which breaks the table (rule B) | a `ValueError` | `signal_extraction` |
| GLIBC-07 | glibc's `pow` (numpy's float64 `x ** o` for exponents other than 0, 1 and 2) is not correctly rounded | the correctly rounded `pow` | `exponential_terms`, `parse_formula`, `expand_model` |
| PD-01, PD-02 | pandas' `read_csv` and `to_json` (`docs/api/confounds.md`, "pandas' tables") | correctly rounded readings and 10-decimal values | `tsv2json`, `expand_model`, `spike_regressors_node` |
