# SPDX-License-Identifier: Apache-2.0
"""Type stubs for the compiled extension module (crate ``larmorx-py``)."""

from typing import Any

import numpy as np

__version__: str

class NiftiError(ValueError): ...

def cli_main(argv: list[str]) -> tuple[int, str, str]:
    """Run the ``larmorx`` command line; return ``(exit_code, stdout, stderr)``."""

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

# --- ANTs image programs: ImageMath, ThresholdImage, MultiplyImages (larmorx/ants) -------------

ImageTuple = tuple[np.ndarray, np.ndarray, bytes | None]  # data, RAS+ affine, descrip

def ants_program(
    program: str,
    args: list[str],
    inputs: dict[str, ImageTuple],
    outputs: list[str],
    n_threads: int = 1,
) -> tuple[int, str, str, dict[str, tuple[np.ndarray, np.ndarray, bytes]]]:
    """Run ``ImageMath``, ``ThresholdImage`` or ``MultiplyImages`` with ``args`` on in-memory
    ``inputs`` (by placeholder name); writes to ``outputs`` names are returned:
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
