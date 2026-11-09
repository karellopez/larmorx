# niworkflows' helpers under sdcflows and fMRIPrep

`lx.niworkflows`: the interfaces of niworkflows 1.15.4 (Apache-2.0, the NiPreps developers;
the same on these paths in 1.15.2, fMRIPrep 25.2.6's) that sdcflows' and fMRIPrep's field-map
steps run, ported in plain Python with numpy's, SciPy's, scikit-image's and nibabel's arithmetic
as niworkflows calls it (`lx.ndimage`, `lx.skimage`, `larmorx_core::numpy`,
`larmorx.io.save_nibabel`). A port in the main package; a clean-room original comes later (the
user, 2026-11-06).

**Status: `experimental`** (checked file for file against niworkflows in both kept environments
and against fMRIPrep's own files; the record comes with the part's records).

## `intensity_clip`: `IntensityClip`

| larmorx | niworkflows | Notes |
|---|---|---|
| `intensity_clip(in_file, p_min=35.0, p_max=99.98, nonnegative=True, dtype="int16", invert=False, *, out_dir=None, numpy_version="2.5.4", upstream_defects="fix", n_threads=1)` → `IntensityClipResult(image, out_file)` | `nibabel.IntensityClip` (`_advanced_clip`): the image squeezed, `median_filter(footprint=ball(3))`, the `p_min`/`p_max` percentiles of the positive (`nonnegative`) denoised values, the original clipped there, shifted to 0, divided by its maximum, inverted, `round(255 x)` for `int16`/`uint8`; `cal_max`; `clipped.nii.gz` | a non-3D image: `RuntimeError("<…> is not a 3D file.")`; `numpy_version` picks numpy's percentile arithmetic (2.2/2.3: `float32`; 2.5: `float64`) |

## `intra_modal_merge`: `IntraModalMerge`

| larmorx | niworkflows | Notes |
|---|---|---|
| `in_files` (a path or a list) | `in_files` | the outputs are named after the first file |
| `in_mask=None` | `in_mask` | the mask of `grand_mean_scaling` (`> 0.5`) |
| `hmc=True` | `hmc` | with more than one file: `mcflirt -in <merged> -cost normcorr -out <merged>_mcf.nii.gz -refvol 0 -mats -plots` (nipype's `MCFLIRT`) through `larmorx fsl mcflirt`: the replica with `larmorx-nc` installed, else the clean-room original (`hmc_implementation`: `"auto"`, `"replica"`, `"original"`); `out_mats` (`<out>.mat/MAT_NNNN`) and `out_movpar` (`<out>.par`) as nipype lists them |
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

Checked file for file against niworkflows and sdcflows on the oracle runs' PEPOLAR estimators
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
the oracle runs (PIPE-22, rule C: kept, a correction that brings no measurable gain). A file holding several transforms
(a series) is not combined (`NotImplementedError`: fMRIPrep passes single affines). Checked: the 7
`itk_mat2txt` nodes of the earlier runs byte for byte (and their derivatives), niworkflows' own
interface on them with `inverse`, and every input format combined
(`check_bold_steps.py`'s `concat`).

## `skullstrip_bold_workflow`: `init_skullstrip_bold_wf`

| larmorx | niworkflows | Notes |
|---|---|---|
| `in_file` | `inputnode.in_file` | linked into the first node's folder, as nipype links it (a copy where links cannot be made) |
| `out_dir=None` | the workflow's directory | the nodes' folders `skullstrip_first_pass`, `skullstrip_second_pass`, `combine_masks`, `apply_mask`; a temporary folder otherwise (the paths are then `None`) |
| `bet_implementation="auto"`, `maths_implementation="auto"` | | FSL's `bet` and `fslmaths`: the replica when `larmorx-nc` is installed, else the clean-room original; AFNI's `3dAutomask`: its replica (built in) |
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
process, FSL's replicas or clean-room originals, AFNI's replicas). Settings files are read as
nipype's `from_file` reads them, their values converted as nipype's input traits convert them
(a JSON `6` for a float stage setting is written `6.0`). `antsRegistration` takes fMRIPrep's
`ANTS_RANDOM_SEED` as `ants_random_seed` (ANTs seeds from the clock without one). Checked against
every `command.txt` of the oracle runs for these interfaces (1,583 nodes, the encoding files of
TOPUP against nipype's own) and against nipype's own `cmdline` and outputs on 14 option sets the
runs do not use: `validation/investigations/sdcflows-fieldmap-b/check_nipype.py`. Not ported:
nipype's version checks (the singular `fixed_image_mask`/`moving_image_mask` of ANTs <= 2.1.0
raise), the `xor`/`requires` checks that do not change the line. `antsApplyTransforms` runs on
files through larmorx's port of the program (`_core.ants_file_program`, an explicit thread count:
its result never depends on it), then `FixHeaderApplyTransforms` copies the reference's x-forms
onto the output (`copy_xform`).

## The EPI reference and the steps under it (SyN preprocessing)

| larmorx | niworkflows / nipype / nilearn | Notes |
|---|---|---|
| `validate_image(in_file, *, out_dir=".", upstream_defects="fix", n_threads=1)` → `ValidateImageResult(out_file, out_report, warning)` | `interfaces/header.ValidateImage` | the truth table: matching valid x-forms with codes above 0 (or a qform that only differs by a qform's precision): the input and an empty `report.html`; otherwise `<in>_valid` with the sform copied from a valid qform, the qform from a valid sform (the report gives the change's angle and translation: transforms3d's `decompose44` as it computes it, the axis from `numpy.linalg.eig`), or both set to nibabel's base affine with code 1; the HTML snippet indented by three tabs. An sform is invalid when numpy's determinant is 0 (LAPACK's LU of the 3 x 3 block) or its zooms are not `np.allclose` to `pixdim`; a qform when nibabel's `get_qform` raises (an impossible quaternion). PIPE-24 |
| `nonsteady_states_detector(in_file, nonnegative=True, n_volumes=40, zero_dummy_masked=20, *, numpy_version, upstream_defects="fix", n_threads=1)` → `NonsteadyStatesResult(t_mask, n_dummy)` | `interfaces/bold.NonsteadyStatesDetector` | the first `n_volumes` (10 to 200) clipped to `[0, p99.8]` (`[p0.2, p99.8]` without `nonnegative`), each volume's mean in numpy's summation order (`numpy_version`: 2.2's buffered sums), nipype's `is_outlier`; fewer than two non-steady states: the last `zero_dummy_masked` (2 to 40) of the first `n_volumes` are averaged; a 3D image: `(True,)`, 1 |
| `is_outlier(points, thresh=3.5, *, upstream_defects="fix")` → `int` | nipype 1.12.0 `algorithms/confounds.is_outlier` | the leading observations whose modified z-score `0.6745 * abs(x - median) / MAD` exceeds `thresh`, in the points' type (numpy's medians: `larmorx_core::numpy`); PIPE-23 |
| `binarize(in_file, thresh_low, *, out_dir=".", n_threads=1)` → `BinarizeResult(out_file, out_mask)` | `interfaces/nibabel.Binarize` | `get_fdata()` above `thresh_low` (the rest 0) in the input's header and type, `<in>_masked`; the mask as `uint8`, `<in>_mask` |
| `generate_sampling_reference(fixed_image, moving_image, *, xform_code=None, fov_mask=None, keep_native=True, target_resolution=None, out_dir=".", niworkflows_version="1.15.4", nilearn_version="0.14.1", scipy_version="1.18.1", n_threads=1, upstream_defects="fix")` → path | `interfaces/nibabel.GenerateSamplingReference` (`_gen_reference`) | the fixed image's field of view on a RAS grid of the moving image's voxel sizes after nibabel's `as_closest_canonical` (or `target_resolution`), rounded to 0.001 mm; the fixed image resampled onto it (nearest); `fov_mask` crops the grid to the mask's bounding box plus two voxels ("Fixed image and mask do not have the same dimensions.", "... different affines"); qform and sform of the grid with the sform's code (2 for 1; `xform_code` 2 or 4 forces it); `descrip` names niworkflows' version; `<fixed>_reference`. `nilearn_version`: 0.13 onwards copies the fixed image's header (written unscaled, `scl_slope = scl_inter = 0`), 0.11.1 (fMRIPrep 25.2.6's image) makes a new one and copies by whole voxels where the grids differ by whole voxels |
| `resample_img_nearest(img, target_affine, target_shape=None, *, copy=False, nilearn_version, scipy_version, n_threads=1, upstream_defects="fix")` → `Image` | nilearn 0.14.1 / 0.11.1 `image.resample_img(interpolation="nearest")` | the grid's bounds as OpenBLAS's `dgemm` adds them (one fused multiply-add per term), `BoundingBoxError`, the "no sform" warning, non-finite values (NaN where the voxel taken was not finite, with nilearn's warning), the input itself when nothing needs resampling. nilearn's two inverses of the image's affine: numpy's (`np.linalg.inv`, OpenBLAS's `dgesv`) for the bounds and SciPy's (`scipy.linalg.inv`, `larmorx.transforms._linalg.scipy_inv`) for the sampling points: SciPy 1.18.1's structure detection (a plumb affine is upper triangular: OpenBLAS's `trti2`; an oblique one `getrf`/`getri`; a symmetric one, an axis swap with a zero origin, Cholesky or Bunch-Kaufman) with its `LinAlgWarning` from LAPACK's condition estimates (`gecon`, `trcon`, `pocon`, `sycon`, the `rcond` it prints), or 1.15's `getrf`/`getri` with `scipy_version`, through SciPy's own OpenBLAS (0.3.31.dev, 0.3.28) on the Haswell kernels: identical to SciPy on 27 families of 20,000 matrices and 513 image affines of the oracle runs per environment, every `rcond` identical to SciPy's LAPACK calls (`check_scipy_inv.py`; SCI-18, SCI-19, SCI-20) |
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
field-map sink of the fMRIPrep oracle runs (95 files of 19 estimators, fMRIPrep 25.2.6's image)
and of sdcflows 2.17.1's own BIDS-app runs (75 files of 15 estimators), given each node's own
inputs: every name, uncompressed and gzip byte and sidecar identical (every one of them fixed the
header: the estimators' files carry `sec` units or other codes); and 38 edge cases against
niworkflows' own sink in this environment (`synthetic`: copies and (de)compression with the
related `.mat`, xform codes and oblique affines, units, `RepetitionTime` as input and in
`meta_dict`, spaces and resolutions with TemplateFlow's metadata, masks, `dseg`, `'source'`,
scaled and big-endian data, extensions, NIfTI-2, the `ni1` and an unknown magic, a non-NIfTI file,
several files and sources, `dismiss_entities`, the errors): all identical, the defects' cases with
`upstream_defects="reproduce"`.

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
