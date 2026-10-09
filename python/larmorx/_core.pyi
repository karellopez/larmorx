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
