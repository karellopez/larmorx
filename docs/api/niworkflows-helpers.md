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
