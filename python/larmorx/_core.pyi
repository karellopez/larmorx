# SPDX-License-Identifier: Apache-2.0
"""Type stubs for the compiled extension module (crate ``larmorx-py``)."""

from pathlib import Path
from typing import Any

import numpy as np

__version__: str

class NiftiError(ValueError): ...

def cli_main(
    argv: list[str], replica_dirs: list[str] = ..., implementation: str | None = None
) -> tuple[int, str, str]:
    """Run the ``larmorx`` command line; return ``(exit_code, stdout, stderr)``. Tools with a
    replica run its program when ``implementation`` (``"auto"``, ``"replica"``,
    ``"original"``; ``None`` reads ``LARMORX_IMPLEMENTATION``) says so; the program is looked
    for through its environment variable, then in ``replica_dirs``, then on ``PATH``."""

def cli_tools() -> list[tuple[str, list[str]]]:
    """The command line's tool families and their tools: ``[(family, [tool, ...]), ...]``."""

# --- Replicas in other packages, run as separate programs (larmorx/_replica.py) ----------------

def replica_registry() -> list[dict[str, str]]:
    """Every tool with a replica: ``family``, ``tool``, ``package``, ``program``, ``env``,
    ``licence``, ``install``."""

def replica_find(family: str, tool: str, dirs: list[str]) -> Path | None:
    """The replica program of ``family tool`` (its environment variable, then ``dirs``, then
    ``PATH``), or ``None``; ``ValueError`` if the variable names no file, ``KeyError`` if the
    tool has no replica."""

# --- Upstream defects, D13 (larmorx/_defects.py) -----------------------------------------------

#: A notice as Rust returns it: ``(entry, tool, mode, detail)``.
Notice = tuple[str, str, str, str]

def upstream_defects() -> list[tuple[str, str, str, bool]]:
    """Every register entry larmorx has a switch for: ``(id, tool, what,
    fixed_by_default)``."""

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

def itk_transform_read(
    path: str, precision: str = "float64", upstream_defects: str = "fix"
) -> tuple[Parts, list[Notice]]:
    """The transforms stored in an ITK text, MATLAB, HDF5 or NIfTI displacement-field file,
    in the file's order, read as ITK's reader for ``precision`` (``float64`` or ``float32``)
    reads them, and the upstream-defect notices."""

def itk_transform_write(
    path: str,
    parts: Parts,
    compression: bool = True,
    time: int = 0,
    itk_version: str | None = None,
    hdf_version: str | None = None,
    os_name: str | None = None,
    os_version: str | None = None,
    upstream_defects: str = "fix",
) -> list[Notice]:
    """Write transforms in ITK's format for the file name's extension: HDF5 (``.h5``,
    ``.hdf5`` and ITK's other HDF5 extensions; any transforms, as ITK's HDF5 writer with these
    options), ITK text (``.txt``, ``.tfm``; linear) or MATLAB (``.mat``; one linear). Returns
    the upstream-defect notices."""

# --- HDF5 (crate larmorx-hdf5) -----------------------------------------------------------------

def hdf5_group_members(path: str, group: str) -> list[tuple[str, dict[str, Any]]]:
    """The members of an HDF5 group in h5py's order (by name), each with the datasets it holds
    as h5py reads them: strings as ``list[bytes | None]``, floats as float32 (4-byte) or
    float64 arrays. ``KeyError`` if the group does not exist, ``OSError`` if the file is not
    HDF5."""

def hdf5_datasets(path: str) -> list[tuple[str, str, list[int], Any]]:
    """Every group and dataset of an HDF5 file, depth first in name order, as ``(path,
    datatype, shape, value)``: strings as ``list[bytes | None]``, floats as float32/float64
    arrays, integers as their stored bytes, ``None`` otherwise."""

def itk_h5_replay(path: str) -> dict[str, Any]:
    """Replay an ITK-written HDF5 transform file through larmorx's writer with the file's own
    time, strings and compression, and compare the bytes (validation suite ``itk-hdf5``)."""

def hdf5_set_times(data: bytes, time: int) -> bytes:
    """An HDF5 file's bytes with every object header's times set to ``time``."""

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
    precision: str = "float64",
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[Notice]]:
    """Resample ``data`` onto ``reference_grid`` (``antsApplyTransforms``), computing in
    ``precision`` (``float64``, or ``float32`` as ``--float 1``): a result of that type and the
    upstream-defect notices."""

def ants_cast_output(
    values: np.ndarray, dtype: str, upstream_defects: str = "fix"
) -> tuple[np.ndarray, list[Notice]]:
    """``antsApplyTransforms -u``'s cast of float64 ``values`` to ``dtype`` (``int8``,
    ``uint8``, ``int16``, ``int32``, ``float32``, ``float64``): out of an integer type's range
    clamped (``fix``) or wrapped as x86-64 converts (``reproduce``; ANTS-06)."""

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
    upstream_defects: str = "fix",
    afni_settings: list[tuple[str, str]] | None = None,
) -> tuple[np.ndarray, dict[str, Any], list[str], list[Notice]]:
    """Read a NIfTI file with AFNI's rules and shift it (``3dTshift``): the voxels in AFNI's
    storage type (before the brick factor, the header's ``scl_slope``), the output header
    fields, warnings and upstream-defect notices. ``afni_settings`` are AFNI's environment
    variables for the call (never read from the process environment)."""

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
    upstream_defects: str = "fix",
    afni_settings: list[tuple[str, str]] | None = None,
) -> tuple[np.ndarray, dict[str, Any], list[str], list[Notice]]:
    """Shift a 4D ``uint8``/``int16``/``float32`` array placed by ``header`` (``3dTshift``):
    the shifted voxels, the output header fields, warnings and upstream-defect notices."""

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
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[Notice]]:
    """``scipy.ndimage.map_coordinates`` for a 1- to 4-D ``input`` and ``(ndim, n)`` float64
    ``coordinates``: the ``n`` values in the ``output`` dtype, and upstream-defect notices."""

def ndimage_spline_filter1d(
    input: np.ndarray,
    order: int = 3,
    axis: int = -1,
    mode: str = "mirror",
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[Notice]]:
    """``scipy.ndimage.spline_filter1d`` on a float64 array (Fortran-ordered result)."""

def ndimage_spline_filter(
    input: np.ndarray,
    order: int = 3,
    mode: str = "mirror",
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[Notice]]:
    """``scipy.ndimage.spline_filter`` on a float64 array (Fortran-ordered result)."""

def ndimage_prepared_coefficients(
    input: np.ndarray,
    order: int = 3,
    mode: str = "grid-constant",
    cval: float = 0.0,
    upstream_defects: str = "fix",
) -> np.ndarray:
    """The padded, prefiltered coefficients ``map_coordinates`` interpolates (3D float32)."""

# --- nitransforms chains and fMRIPrep's one-shot resampler (larmorx/transforms) ---------------

Step = tuple[Any, ...]  # ("affine", 4x4) or ("field", deltas (x, y, z, 3), affine, inverse)

def linalg_matmul4(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """``a[t] @ b[t]`` for two ``(n, 4, 4)`` float64 stacks, in the operation order of numpy's
    OpenBLAS Haswell ``dgemm``."""

def linalg_inv4(a: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """``np.linalg.inv`` of an ``(n, 4, 4)`` float64 stack as OpenBLAS's ``dgesv`` computes it;
    NaN for singular matrices, and which were singular (bool, ``(n,)``)."""

def linalg_closest_orthogonal(rs: np.ndarray) -> np.ndarray:
    """nibabel's ``io_orientation`` polar step for a 3×3 matrix (vnl's SVD)."""

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
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[Notice]]:
    """fMRIPrep's ``resample_image`` after loading: a float32 3D/4D ``source`` onto the target
    grid through ``steps`` and ``ras2vox``, with per-volume voxel-to-voxel head motion ``hmc``
    (``(t, 4, 4)``), a field map in Hz and readout vectors ``pe``. Fortran-ordered result, and
    upstream-defect notices (PIPE-03, SCI-03)."""

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
    upstream_defects: str = "fix",
) -> tuple[int, str, str, dict[str, tuple[np.ndarray, np.ndarray, bytes]]]:
    """Run ``ImageMath``, ``ThresholdImage``, ``MultiplyImages``, ``SmoothImage``, ``N4BiasFieldCorrection``,
    ``ResampleImageBySpacing`` or ``ResampleImage`` with ``args`` on in-memory ``inputs`` (by
    placeholder name);
    writes to ``outputs`` names are returned:
    ``(exit_code, stdout, stderr, {name: (data, ras_affine, descrip)})``. The program runs in
    the D13 mode ``upstream_defects``; its notices are lines of ``stderr``."""

def ants_image_math_operations() -> list[tuple[str, str]]:
    """The ImageMath operations larmorx implements: ``(name, usage)``."""

def ants_read_float(
    path: str, dim: int = 3, n_threads: int = 1
) -> tuple[np.ndarray, np.ndarray, bytes]:
    """Read a NIfTI image as ANTs reads it into ``itk::Image<float, dim>``."""

def ants_arithmetic(
    op: str,
    a: np.ndarray,
    b: np.ndarray | float,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, float, int, list[Notice]]:
    """ImageMath's voxel-wise arithmetic on float32 arrays: ``(data, result, count,
    notices)``."""

def ants_negative_image(
    a: np.ndarray, n_threads: int = 1, upstream_defects: str = "fix"
) -> tuple[np.ndarray, list[Notice]]:
    """ImageMath ``Neg``: ``(data, notices)``."""

def ants_truncate_image_intensity(
    a: np.ndarray,
    lower_quantile: float = 0.025,
    upper_quantile: float | None = None,
    bins: int = 64,
    mask: np.ndarray | None = None,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, float, float, list[Notice]]:
    """ImageMath ``TruncateImageIntensity``: ``(data, lower, upper, notices)``."""

def ants_normalize_image(
    a: np.ndarray,
    mode: str = "range",
    mask: np.ndarray | None = None,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[Notice]]:
    """ImageMath ``Normalize`` (``mode``: ``range``, ``mean`` or ``mask``): ``(data,
    notices)``."""

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
    a: np.ndarray,
    n_thresholds: int,
    mask: np.ndarray | None = None,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, np.ndarray, list[Notice]]:
    """``ThresholdImage ... Otsu n [mask]``: ``(labels, thresholds, notices)``."""

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
    a: np.ndarray,
    spacing: list[float],
    sigma: list[float],
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[Notice]]:
    """ImageMath ``G``: ``DiscreteGaussianImageFilter`` (sigma in physical units):
    ``(data, notices)``."""

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

def ants_denoise_image(
    image: str | ImageTuple,
    dim: int,
    mask: str | ImageTuple | None = None,
    rician: bool = True,
    shrink_factor: int = 1,
    patch_radius: list[int] = ...,
    search_radius: list[int] = ...,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[
    tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]],
    tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]],
    list[Notice],
]:
    """``DenoiseImage`` (paths or in-memory images; the image read as a ``dim``-dimensional
    float image, the mask as ``unsigned char``): ``(denoised, noise, notices)``, the images as
    ``(data, ras_affine, descrip, spacing)``. ``patch_radius`` (default ``[1]``) and ``search_radius``
    (default ``[2]``) hold one value for every axis or one per axis."""

def ants_n4_bias_field_correction(
    image: str | ImageTuple,
    dim: int,
    mask: str | ImageTuple | None = None,
    weight: str | ImageTuple | None = None,
    shrink_factor: int = 4,
    iterations: list[int] = ...,
    convergence_threshold: float = 0.0,
    spline_distance: float | None = None,
    mesh: list[float] | None = None,
    spline_order: int = 3,
    fwhm: float = 0.15,
    wiener_noise: float = 0.01,
    bins: int = 200,
    rescale_intensities: bool = False,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[
    tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]],
    tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]],
    list[tuple[int, int, float]],
    list[Notice],
]:
    """``N4BiasFieldCorrection`` (paths or in-memory images, read as ``dim``-dimensional float
    images): ``(corrected, bias_field, [(level, iteration, convergence)], notices)``, the images as
    ``(data, ras_affine, descrip, spacing)``. ``spline_distance`` (``-b [mm]``) and ``mesh``
    (``-b [AxBxC]``) are exclusive; ``iterations`` defaults to ``[50, 50, 50, 50]``."""

def ants_resample_image_by_spacing(
    image: str | ImageTuple,
    spacing: list[float],
    smooth: bool = True,
    add_voxels: int = 0,
    nearest: bool = False,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]], list[Notice]]:
    """``ResampleImageBySpacing`` on a path (read in ``len(spacing)`` dimensions) or an
    in-memory image: ``((data, ras_affine, descrip, spacing), notices)``."""

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
    upstream_defects: str = "fix",
) -> tuple[tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]], list[Notice]]:
    """``ResampleImage`` by ``spacing`` or ``size`` (``interpolation``: ``linear``, ``nearest``,
    ``gaussian``, ``sinc``, ``bspline``; ``pixel_type``: ``char`` ... ``double``):
    ``((data, ras_affine, descrip, spacing), notices)``."""

# The morphology group: ``a`` is float32 in 2 to 4 dimensions; radii are in voxels.

def ants_morphology(
    a: np.ndarray, operation: str, radius: int = 1, value: float = 1.0, n_threads: int = 1
) -> np.ndarray:
    """ImageMath ``MD``/``ME``/``MO``/``MC``/``GD``/``GE``/``GO``/``GC`` (``ants::Morphological``)
    with ITK's ball of ``radius`` voxels; ``value`` is the binary operations' foreground."""

def ants_fill_holes(
    a: np.ndarray, hole_param: float = 2.0, n_threads: int = 1, upstream_defects: str = "fix"
) -> tuple[np.ndarray, list[Notice]]:
    """ImageMath ``FillHoles``: the background regions other than the largest set to 1 (all for
    ``hole_param`` 2; by their share of object neighbours for ``hole_param ≤ 1``): ``(data,
    notices)``."""

def ants_pad_image(
    image: str | ImageTuple, dim: int, pad: float, value: float = 0.0, n_threads: int = 1
) -> tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]]:
    """ImageMath ``PadImage`` on a path (read in ``dim`` dimensions) or an in-memory image:
    ``(data, ras_affine, descrip, spacing)``."""

# Components, distance maps and label values: ``a`` is float32 in 2 to 4 dimensions.

def ants_largest_component(
    a: np.ndarray, smallest: int = 50, n_threads: int = 1, upstream_defects: str = "fix"
) -> tuple[np.ndarray, list[Notice]]:
    """ImageMath ``GetLargestComponent``: 1 in the largest face-connected component(s) of the
    voxels in ``[0.25, 1e9]``, 0 elsewhere (0 everywhere if no component has ``smallest``
    voxels; ANTs, reproduced: 1 everywhere): ``(data, notices)``."""

def ants_distance_map(a: np.ndarray, spacing: list[float]) -> np.ndarray:
    """ImageMath ``D``: ITK's Danielsson distance map to the non-zero voxels (mm)."""

def ants_maurer_distance(
    a: np.ndarray, spacing: list[float], foreground: float = 1.0, n_threads: int = 1
) -> np.ndarray:
    """ImageMath ``MaurerDistance``: ITK's signed Maurer distance map of the voxels equal to
    ``foreground`` (mm, negative inside)."""

def ants_extract_contours(
    a: np.ndarray, fully_connected: bool = True, n_threads: int = 1, upstream_defects: str = "fix"
) -> tuple[np.ndarray, list[Notice]]:
    """ImageMath ``ExtractContours``: the voxels of each integer label (values truncated)
    that touch another label, keeping their label; 0 elsewhere: ``(data, notices)``."""

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
    upstream_defects: str = "fix",
    normcorr_count: str = "mcflirt",
) -> dict[str, Any]:
    """Head-motion correction of a NIfTI series: ``matrices``, ``ras``, ``itk``, ``params``,
    ``rms_abs``, ``rms_rel``, ``fd``, ``reference_index``, ``in_plane``, ``notices``,
    ``header``, ``data`` (float32 or ``None``) and ``mean`` (or ``None``)."""

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
    upstream_defects: str = "fix",
    normcorr_count: str = "mcflirt",
) -> dict[str, Any]:
    """:func:`mri_hmc_file` for a float32 ``(x, y, z[, t])`` array placed by header fields."""

def mri_read_mat(text: str) -> np.ndarray:
    """An FSL text matrix as a ``(1, 4, 4)`` array."""

def mri_mat_text(m: np.ndarray) -> str:
    """A 4 × 4 matrix in mcflirt's ``.mat`` text format."""

def mri_par_text(params: np.ndarray) -> str:
    """``(N, 6)`` motion parameters in mcflirt's ``.par`` text format."""
