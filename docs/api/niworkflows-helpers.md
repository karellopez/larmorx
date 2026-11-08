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
| `intensity_clip(in_file, p_min=35.0, p_max=99.98, nonnegative=True, dtype="int16", invert=False, *, out_dir=None, numpy_version="2.5.4", n_threads=1)` → `IntensityClipResult(image, out_file)` | `nibabel.IntensityClip` (`_advanced_clip`): the image squeezed, `median_filter(footprint=ball(3))`, the `p_min`/`p_max` percentiles of the positive (`nonnegative`) denoised values, the original clipped there, shifted to 0, divided by its maximum, inverted, `round(255 x)` for `int16`/`uint8`; `cal_max`; `clipped.nii.gz` | a non-3D image: `RuntimeError("<…> is not a 3D file.")`; `numpy_version` picks numpy's percentile arithmetic (2.2/2.3: `float32`; 2.5: `float64`) |

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
| `merge_series(in_files, allow_4D=True, affine_tolerance=None, *, out_dir=None)` → the path | `nibabel.MergeSeries` | each file squeezed; 3D images and (with `allow_4D`) 4D ones' volumes merged (nibabel's `concat_images`), `<first>_merged`; another dimension: `ValueError('Input image has an incorrect number of dimensions (…).')`; with `affine_tolerance`, `np.allclose(atol=…)` against the first (`ValueError('Difference in affines greater than allowed tolerance …')`) and the first's affine kept; without, affines must be identical |
| `reorient_file(in_file, *, target_file=None, target_ornt=None, newpath=None)` → the path | `nibabel.ReorientImage` (`reorient_file`) | to `target_file`'s axis codes or `target_ornt`'s (`TypeError('No target orientation or file is specified.')`), `reoriented.nii.gz` |
| `reorient_image(img, target_ornt)`, `as_reoriented(img, ornt)`, `axcodes2ornt`, `aff2axcodes`, `ornt_transform`, `squeeze_image`, `four_to_three` | `nibabel.reorient_image`; nibabel 5.4.2's functions (MIT) | in memory; nibabel's errors (`Not all axis codes … in label set …`) |

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
raise), the `xor`/`requires` checks that do not change the line; `antsApplyTransforms`' line is
formatted and runs with the SyN steps.
