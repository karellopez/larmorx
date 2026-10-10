# SPDX-License-Identifier: Apache-2.0
"""Type stubs for the compiled extension module (crate ``larmorx-py``)."""

from typing import Any

import numpy as np

__version__: str

class NiftiError(ValueError): ...

def cli_main(argv: list[str]) -> tuple[int, str, str]:
    """Run the ``larmorx`` command line; return ``(exit_code, stdout, stderr)``."""

def cli_tools() -> list[tuple[str, list[str]]]:
    """The command line's tool families and their tools: ``[(family, [tool, ...]), ...]``."""

def nifti_read(
    path: str, scaling: str = "auto", n_threads: int = 1
) -> tuple[np.ndarray, np.ndarray, dict[str, Any], list[str], bool]:
    """Read a NIfTI file: ``(data, affine, header fields, fixes applied, scaled)``."""

def nifti_read_header(path: str, fix: bool = False) -> Any:
    """Read a NIfTI header: the fields as stored, or ``(fields, fixes)`` with nibabel's
    load-time fixes applied when ``fix`` is set."""

def nifti_parse_header(block: bytes) -> dict[str, Any]:
    """Parse a 348/540-byte header block (no extensions, no fixes)."""

def nifti_write(
    path: str,
    data: np.ndarray,
    header: dict[str, Any],
    compression_level: int = 2,
    n_threads: int = 1,
) -> None:
    """Write ``data`` with ``header`` fields; ``.gz`` names are compressed."""

def nifti_header_for_image(
    shape: list[int],
    dtype: str,
    affine: np.ndarray,
    template: dict[str, Any] | None = None,
    version: int | None = None,
) -> dict[str, Any]:
    """The header fields nibabel would write for an image."""

def nifti_header_info(header: dict[str, Any]) -> dict[str, Any]:
    """Shape, dtype, zooms, affines, scaling and serialised bytes of header fields."""

def nifti_itk_geometry(header: dict[str, Any]) -> dict[str, Any]:
    """The geometry ITK 5.4.5 reads from header fields (LPS origin, spacing, direction)."""

# --- ITK geometry, transforms and antsApplyTransforms (larmorx/ants) ---------------------------

Grid = dict[str, Any]  # size, spacing, origin (LPS), direction (3x3)
Parts = list[tuple[str, np.ndarray, np.ndarray]]  # (ITK class, parameters, fixed parameters)

def grid_from_ras_affine(shape: tuple[int, int, int], affine: np.ndarray) -> Grid:
    """The LPS grid of an image with ``shape`` and RAS+ ``affine``."""

def grid_ras_affine(grid: Grid) -> np.ndarray:
    """The RAS+ affine of an LPS grid."""

def itk_read_image(path: str, n_threads: int = 1) -> tuple[np.ndarray, dict[str, Any]]:
    """Read a NIfTI image as ITK 5.4.5 does: voxel values and geometry (incl. the 3D grid)."""

def itk_grid(path: str) -> Grid:
    """The 3D grid ITK reads from a NIfTI header."""

def itk_transform_read(path: str) -> Parts:
    """The transforms stored in an ITK text, MATLAB or NIfTI displacement-field file."""

def itk_transform_write(path: str, parts: Parts) -> None:
    """Write linear transforms as ITK text (``.txt``, ``.tfm``) or MATLAB (``.mat``)."""

def ants_apply_transforms(
    data: np.ndarray,
    input_grid: Grid,
    reference_grid: Grid,
    transforms: list[tuple[Parts, bool]],
    interpolation_name: str = "linear",
    sigma: tuple[float, float, float] | None = None,
    alpha: float | None = None,
    order: int = 3,
    default_value: float = 0.0,
    n_threads: int = 1,
) -> np.ndarray:
    """Resample ``data`` onto ``reference_grid`` (``antsApplyTransforms``); float64 result."""

def ants_transform_points(points: np.ndarray, transforms: list[tuple[Parts, bool]]) -> np.ndarray:
    """Map ``(n, 3)`` LPS points through ``transforms``."""

# --- AFNI 3dTshift (larmorx/afni) ------------------------------------------------------------

def afni_tshift_file(
    path: str,
    slice_times: str | list[float],
    tr: float | None = None,
    tzero: float | None = None,
    slice: int | None = None,
    ignore: int = 0,
    method: str = "fourier",
    restore: str = "trend",
    detrend: bool = True,
    n_threads: int = 1,
) -> tuple[np.ndarray, dict[str, Any], list[str]]:
    """Read a NIfTI file with AFNI's rules and shift it (``3dTshift``): the voxels in AFNI's
    storage type (before the brick factor, the header's ``scl_slope``), the output header
    fields and warnings."""

def afni_tshift_array(
    data: np.ndarray,
    header: dict[str, Any],
    slice_times: str | list[float],
    tr: float | None = None,
    tzero: float | None = None,
    slice: int | None = None,
    ignore: int = 0,
    method: str = "fourier",
    restore: str = "trend",
    detrend: bool = True,
    n_threads: int = 1,
) -> tuple[np.ndarray, dict[str, Any], list[str]]:
    """Shift a 4D ``uint8``/``int16``/``float32`` array placed by ``header`` (``3dTshift``):
    the shifted voxels, the output header fields and warnings."""

# --- SciPy-compatible interpolation (larmorx/ndimage.py) --------------------------------------

def ndimage_map_coordinates(
    input: np.ndarray,
    coordinates: np.ndarray,
    order: int = 3,
    mode: str = "constant",
    cval: float = 0.0,
    prefilter: bool = True,
    output: str = "float64",
    n_threads: int = 1,
) -> np.ndarray:
    """``scipy.ndimage.map_coordinates`` for a 1- to 4-D ``input`` and ``(ndim, n)`` float64
    ``coordinates``: the ``n`` values in the ``output`` dtype."""

def ndimage_spline_filter1d(
    input: np.ndarray, order: int = 3, axis: int = -1, mode: str = "mirror", n_threads: int = 1
) -> np.ndarray:
    """``scipy.ndimage.spline_filter1d`` on a float64 array (Fortran-ordered result)."""

def ndimage_spline_filter(
    input: np.ndarray, order: int = 3, mode: str = "mirror", n_threads: int = 1
) -> np.ndarray:
    """``scipy.ndimage.spline_filter`` on a float64 array (Fortran-ordered result)."""

def ndimage_prepared_coefficients(
    input: np.ndarray, order: int = 3, mode: str = "grid-constant", cval: float = 0.0
) -> np.ndarray:
    """The padded, prefiltered coefficients ``map_coordinates`` interpolates (3D float32)."""

# --- nitransforms chains and fMRIPrep's one-shot resampler (larmorx/transforms) ---------------

Step = tuple[Any, ...]  # ("affine", 4x4) or ("field", deltas (x, y, z, 3), affine, inverse)

def nitransforms_ndcoords(shape: list[int], affine: np.ndarray) -> np.ndarray:
    """``ImageGrid.ndcoords``: ``(x, y, z, 3)`` float64 RAS+ coordinates of every voxel."""

def nitransforms_map(points: np.ndarray, steps: list[Step], n_threads: int = 1) -> np.ndarray:
    """``TransformChain(steps).map(points)`` for ``(n, 3)`` float64 points."""

def resample_series(
    source: np.ndarray,
    target_shape: list[int],
    target_affine: np.ndarray,
    steps: list[Step],
    ras2vox: np.ndarray,
    hmc: np.ndarray | None = None,
    fieldmap: np.ndarray | None = None,
    pe: list[tuple[int, float]] | None = None,
    order: int = 3,
    mode: str = "grid-constant",
    cval: float = 0.0,
    prefilter: bool = True,
    jacobian: bool = True,
    output: str = "float32",
    n_threads: int = 1,
) -> np.ndarray:
    """fMRIPrep's ``resample_image`` after loading: a float32 3D/4D ``source`` onto the target
    grid through ``steps`` and ``ras2vox``, with per-volume voxel-to-voxel head motion ``hmc``
    (``(t, 4, 4)``), a field map in Hz and readout vectors ``pe``. Fortran-ordered result."""

# --- ANTs image programs: ImageMath, ThresholdImage, MultiplyImages (larmorx/ants) -------------

# data, RAS+ affine, descrip[, spacing of the axes after the third (the time step in seconds)]
ImageTuple = (
    tuple[np.ndarray, np.ndarray, bytes | None]
    | tuple[np.ndarray, np.ndarray, bytes | None, list[float] | None]
)

def ants_program(
    program: str,
    args: list[str],
    inputs: dict[str, ImageTuple],
    outputs: list[str],
    n_threads: int = 1,
) -> tuple[int, str, str, dict[str, tuple[np.ndarray, np.ndarray, bytes]]]:
    """Run ``ImageMath``, ``ThresholdImage``, ``MultiplyImages``, ``SmoothImage``,
    ``ResampleImageBySpacing`` or ``ResampleImage`` with ``args`` on in-memory ``inputs`` (by
    placeholder name);
    writes to ``outputs`` names are returned:
    ``(exit_code, stdout, stderr, {name: (data, ras_affine, descrip)})``."""

def ants_image_math_operations() -> list[tuple[str, str]]:
    """The ImageMath operations larmorx implements: ``(name, usage)``."""

def ants_read_float(
    path: str, dim: int = 3, n_threads: int = 1
) -> tuple[np.ndarray, np.ndarray, bytes]:
    """Read a NIfTI image as ANTs reads it into ``itk::Image<float, dim>``."""

def ants_arithmetic(
    op: str, a: np.ndarray, b: np.ndarray | float, n_threads: int = 1
) -> tuple[np.ndarray, float, int]:
    """ImageMath's voxel-wise arithmetic on float32 arrays: ``(data, result, count)``."""

def ants_negative_image(a: np.ndarray, n_threads: int = 1) -> np.ndarray:
    """ImageMath ``Neg``."""

def ants_truncate_image_intensity(
    a: np.ndarray,
    lower_quantile: float = 0.025,
    upper_quantile: float | None = None,
    bins: int = 64,
    mask: np.ndarray | None = None,
    n_threads: int = 1,
) -> tuple[np.ndarray, float, float]:
    """ImageMath ``TruncateImageIntensity``: ``(data, lower, upper)``."""

def ants_normalize_image(
    a: np.ndarray, mode: str = "range", mask: np.ndarray | None = None, n_threads: int = 1
) -> np.ndarray:
    """ImageMath ``Normalize`` (``mode``: ``range``, ``mean`` or ``mask``)."""

def ants_rescale_image(
    a: np.ndarray, minimum: float, maximum: float, n_threads: int = 1
) -> np.ndarray:
    """ImageMath ``RescaleImage``."""

def ants_to_float(a: np.ndarray) -> np.ndarray:
    """float64 values converted to float32 as a C++ ``static_cast`` converts them."""

def ants_threshold_image(
    a: np.ndarray,
    lower: float,
    upper: float,
    inside: float = 1.0,
    outside: float = 0.0,
    n_threads: int = 1,
) -> np.ndarray:
    """``ThresholdImage`` with a range."""

def ants_otsu_threshold(
    a: np.ndarray, n_thresholds: int, mask: np.ndarray | None = None, n_threads: int = 1
) -> tuple[np.ndarray, np.ndarray]:
    """``ThresholdImage ... Otsu n [mask]``: ``(labels, thresholds)``."""

# The Gaussian group: ``a`` is float32 in 2 to 4 dimensions, ``spacing`` ITK's spacing of every
# axis (mm; seconds for a fourth axis).

def ants_smooth_image(
    a: np.ndarray,
    spacing: list[float],
    sigma: list[float],
    physical: bool = False,
    median: bool = False,
    n_threads: int = 1,
) -> np.ndarray:
    """``SmoothImage``: recursive Gaussian (sigma in voxels unless ``physical``) or median
    (radius ``sigma`` voxels)."""

def ants_discrete_gaussian(
    a: np.ndarray, spacing: list[float], sigma: list[float], n_threads: int = 1
) -> np.ndarray:
    """ImageMath ``G``: ``DiscreteGaussianImageFilter`` (sigma in physical units)."""

def ants_laplacian(
    a: np.ndarray,
    spacing: list[float],
    sigma: float = 1.0,
    normalize: bool = False,
    n_threads: int = 1,
) -> np.ndarray:
    """ImageMath ``Laplacian``: ``LaplacianRecursiveGaussianImageFilter``, ``[0, 1]`` if
    ``normalize``."""

def ants_gradient_magnitude(
    a: np.ndarray,
    spacing: list[float],
    sigma: float = 1.0,
    normalize: bool = False,
    n_threads: int = 1,
) -> np.ndarray:
    """ImageMath ``Grad``: ``GradientMagnitudeRecursiveGaussianImageFilter``, ``[0, 1]`` if
    ``normalize``."""

def ants_unsharp_mask(
    a: np.ndarray,
    spacing: list[float],
    amount: float = 0.5,
    radius: float = 1.0,
    threshold: float = 0.0,
    radius_in_spacing_units: bool = False,
    n_threads: int = 1,
) -> np.ndarray:
    """ImageMath ``UnsharpMask``: ``UnsharpMaskImageFilter`` in float."""

def ants_resample_image_by_spacing(
    image: str | ImageTuple,
    spacing: list[float],
    smooth: bool = True,
    add_voxels: int = 0,
    nearest: bool = False,
    n_threads: int = 1,
) -> tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]]:
    """``ResampleImageBySpacing`` on a path (read in ``len(spacing)`` dimensions) or an
    in-memory image: ``(data, ras_affine, descrip, spacing)``."""

def ants_resample_image(
    image: str | ImageTuple,
    dim: int,
    spacing: list[float] | None = None,
    size: list[int] | None = None,
    interpolation: str = "linear",
    sigma: list[float] | None = None,
    alpha: float = 1.0,
    window: str = "hamming",
    order: int = 3,
    pixel_type: str = "float",
    n_threads: int = 1,
) -> tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]]:
    """``ResampleImage`` by ``spacing`` or ``size`` (``interpolation``: ``linear``, ``nearest``,
    ``gaussian``, ``sinc``, ``bspline``; ``pixel_type``: ``char`` ... ``double``):
    ``(data, ras_affine, descrip, spacing)``."""

# The morphology group: ``a`` is float32 in 2 to 4 dimensions; radii are in voxels.

def ants_morphology(
    a: np.ndarray, operation: str, radius: int = 1, value: float = 1.0, n_threads: int = 1
) -> np.ndarray:
    """ImageMath ``MD``/``ME``/``MO``/``MC``/``GD``/``GE``/``GO``/``GC`` (``ants::Morphological``)
    with ITK's ball of ``radius`` voxels; ``value`` is the binary operations' foreground."""

def ants_fill_holes(a: np.ndarray, hole_param: float = 2.0, n_threads: int = 1) -> np.ndarray:
    """ImageMath ``FillHoles``: the background regions other than the largest set to 1 (all for
    ``hole_param`` 2; by their share of object neighbours for ``hole_param ≤ 1``)."""

def ants_pad_image(
    image: str | ImageTuple, dim: int, pad: float, value: float = 0.0, n_threads: int = 1
) -> tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]]:
    """ImageMath ``PadImage`` on a path (read in ``dim`` dimensions) or an in-memory image:
    ``(data, ras_affine, descrip, spacing)``."""

# Components, distance maps and label values: ``a`` is float32 in 2 to 4 dimensions.

def ants_largest_component(a: np.ndarray, smallest: int = 50, n_threads: int = 1) -> np.ndarray:
    """ImageMath ``GetLargestComponent``: 1 in the largest face-connected component(s) of the
    voxels in ``[0.25, 1e9]``, 0 elsewhere (1 everywhere if no component has ``smallest``
    voxels)."""

def ants_distance_map(a: np.ndarray, spacing: list[float]) -> np.ndarray:
    """ImageMath ``D``: ITK's Danielsson distance map to the non-zero voxels (mm)."""

def ants_maurer_distance(
    a: np.ndarray, spacing: list[float], foreground: float = 1.0, n_threads: int = 1
) -> np.ndarray:
    """ImageMath ``MaurerDistance``: ITK's signed Maurer distance map of the voxels equal to
    ``foreground`` (mm, negative inside)."""

def ants_extract_contours(
    a: np.ndarray, fully_connected: bool = True, n_threads: int = 1
) -> np.ndarray:
    """ImageMath ``ExtractContours``: the voxels of each integer label (values truncated)
    that touch another label, keeping their label; 0 elsewhere."""

def ants_threshold_at_mean(a: np.ndarray, fraction: float = 1.0, n_threads: int = 1) -> np.ndarray:
    """ImageMath ``ThresholdAtMean``: 1 where ``mean · fraction ≤ v ≤ max``, else 0."""

def ants_replace_voxel_value(
    a: np.ndarray, low: float, high: float, value: float, n_threads: int = 1
) -> np.ndarray:
    """ImageMath ``ReplaceVoxelValue``: voxels in ``[low, high]`` set to ``value``."""

# --- Head-motion correction, mcflirt-compatible (larmorx/mri) ---------------------------------

def mri_hmc_file(
    path: str,
    reference: str | None = None,
    ref_index: int | None = None,
    mean: bool = False,
    stages: int = 3,
    final_interp: str = "trilinear",
    dof: int = 6,
    cost: str = "normcorr",
    smooth: float = 1.0,
    rotation: float = 1.0,
    bins: int = 256,
    fudge: bool = False,
    in_plane: str = "auto",
    fov: int = 20,
    init: np.ndarray | None = None,
    resample: bool = True,
    n_threads: int = 1,
) -> dict[str, Any]:
    """Head-motion correction of a NIfTI series: ``matrices``, ``ras``, ``itk``, ``params``,
    ``rms_abs``, ``rms_rel``, ``fd``, ``reference_index``, ``in_plane``, ``header``, ``data``
    (float32 or ``None``) and ``mean`` (or ``None``)."""

def mri_hmc_array(
    data: np.ndarray,
    header: dict[str, Any],
    reference: tuple[np.ndarray, dict[str, Any]] | None = None,
    ref_index: int | None = None,
    mean: bool = False,
    stages: int = 3,
    final_interp: str = "trilinear",
    dof: int = 6,
    cost: str = "normcorr",
    smooth: float = 1.0,
    rotation: float = 1.0,
    bins: int = 256,
    fudge: bool = False,
    in_plane: str = "auto",
    fov: int = 20,
    init: np.ndarray | None = None,
    resample: bool = True,
    n_threads: int = 1,
) -> dict[str, Any]:
    """:func:`mri_hmc_file` for a float32 ``(x, y, z[, t])`` array placed by header fields."""

def mri_read_mat(text: str) -> np.ndarray:
    """An FSL text matrix as a ``(1, 4, 4)`` array."""

def mri_mat_text(m: np.ndarray) -> str:
    """A 4 × 4 matrix in mcflirt's ``.mat`` text format."""

def mri_par_text(params: np.ndarray) -> str:
    """``(N, 6)`` motion parameters in mcflirt's ``.par`` text format."""
