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

# --- Replicas: built in, or in other packages and run as programs (larmorx/_replica.py) ------

def replica_registry() -> list[dict[str, str | bool | None]]:
    """Every tool with a replica: ``family``, ``tool``, ``builtin`` (part of larmorx, run in this
    process), ``package``, ``program``, ``env``, ``licence``, ``install`` and ``original``
    (whether larmorx has a clean-room original); a built-in replica's ``package`` is
    ``"larmorx"`` and its ``program``, ``env`` and ``install`` are ``None``."""

def replica_find(family: str, tool: str, dirs: list[str]) -> Path | None:
    """The replica program of ``family tool`` (its environment variable, then ``dirs``, then
    ``PATH``), or ``None`` (not found, or built in); ``ValueError`` if the variable names no
    file, ``KeyError`` if the tool has no replica."""

def fsl_run_original(
    tool: str,
    args: list[str],
    env: list[tuple[str, str]],
    cwd: str | None = None,
) -> tuple[int, str, str]:
    """Run the clean-room original of ``fsl tool`` (``bet``, ``bet2``, ``betsurf``, ``flirt``,
    ``convert_xfm``, ``avscale``, ``img2imgcoord``, ``std2imgcoord``, ``img2stdcoord``,
    ``rmsdiff``, ``makerot``, ``midtrans``, ``pointflirt``, ``applyxfm4D``, ``pairreg``,
    ``extracttxt``, ``aff2rigid`` or ``fast``) with FSL's arguments in this process (without the GIL):
    ``(exit code, standard output, standard error)``. ``env``
    stands for the process environment (never read or changed); relative names start in
    ``cwd``. ``KeyError`` for another tool."""

def replica_run_builtin(
    family: str,
    tool: str,
    args: list[str],
    env: list[tuple[str, str]],
    cwd: str | None = None,
) -> tuple[int, str, str]:
    """Run the built-in replica of ``family tool`` in this process (without the GIL):
    ``(exit code, standard output, standard error)``. ``env`` stands for the process environment
    (never read or changed); relative names start in ``cwd``. ``KeyError`` if the tool has no
    built-in replica."""

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
    nibabel: bool = False,
    lapack: str = "3.12.1",
) -> dict[str, Any]:
    """The header fields nibabel would write for an image; ``nibabel=True`` stores the qform
    with nibabel's own arithmetic (numpy's SVD, det and eigh as LAPACK ``lapack`` computes
    them)."""

def nifti_set_xform(
    header: dict[str, Any],
    which: str,
    affine: np.ndarray,
    code: int,
    nibabel: bool = False,
    lapack: str = "3.12.1",
) -> dict[str, Any]:
    """nibabel's ``header.set_sform(affine, code)`` (``which="sform"``) or
    ``header.set_qform(affine, code)`` (``"qform"``) on header fields; ``nibabel=True``
    stores the qform with nibabel's own arithmetic."""

def nibabel_qform(affine: np.ndarray, lapack: str = "3.12.1") -> dict[str, Any]:
    """nibabel 5.4.2's ``set_qform`` arithmetic: ``qfac``, ``zooms``, ``quaternion`` (w, x,
    y, z) and ``translation`` in float64."""

def numpy_linalg_small(kind: str, a: np.ndarray, lapack: str = "3.12.1") -> Any:
    """numpy.linalg's ``svd`` (3 x 3: ``(u, s[None], vt)``), ``eigh`` (4 x 4, lower: ``(w[None],
    v)``) or ``det(a) > 0`` (``kind="det_positive"``) as numpy computes them with OpenBLAS's
    Haswell kernels and LAPACK ``lapack``, or nibabel's ``io_orientation`` polar factor
    (``kind="io_polar"``: ``P[:, keep] @ Qs[keep]``); ``None`` where numpy raises
    ``LinAlgError``."""

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

def ants_transform_output(
    transforms: list[tuple[Parts, bool]],
    kind: str,
    invert_linear: bool = False,
    reference_grid: Grid | None = None,
    precision: str = "float64",
    upstream_defects: str = "fix",
    n_threads: int = 1,
) -> tuple[Parts, list[Notice]]:
    """``antsApplyTransforms``' transform outputs: the chain as a composite, collapsed to one
    affine transform, or as a displacement field on ``reference_grid``; ``(parts, notices)``."""

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

# --- AFNI 3dAutomask and 3dUnifize, the clean-room originals (larmorx/afni) ------------------

#: An output: the voxels in AFNI's storage type (before the brick factor, the header's
#: ``scl_slope``), in the shape the command line writes, and the header fields.
AfniOutput = tuple[np.ndarray, dict[str, Any]]

def afni_automask_file(
    path: str,
    clfrac: float = 0.5,
    gradual: bool = True,
    peels: int = 1,
    neighbours: int = 17,
    nn: int | None = None,
    exterior_clip: bool = False,
    dilate: int = 0,
    erode: int = 0,
    si: float | None = None,
    apply: bool = False,
    depth: bool = False,
    n_threads: int = 1,
    upstream_defects: str = "fix",
    afni_settings: list[tuple[str, str]] | None = None,
) -> tuple[AfniOutput, AfniOutput | None, AfniOutput | None, list[str], list[Notice]]:
    """3dAutomask on a NIfTI file read with AFNI's rules: the mask, the masked input (with
    ``apply``), the depth (with ``depth``), AFNI's warnings and upstream-defect notices.
    ``ValueError`` for AFNI's fatal errors (no mask, an empty mask) and for ``neighbours``
    outside 6..26 by default (AFNI-41)."""

def afni_unifize_file(
    path: str,
    t2: int = 0,
    t2_percentile: float = 98.5,
    gm: bool = False,
    radius: float = 18.3,
    bottom: float = 70.0,
    top: float = 80.0,
    clfrac: float = 0.2,
    duplo: bool = True,
    epi: bool = False,
    squash: bool = True,
    save_scale: bool = False,
    save_automask: bool = False,
    n_threads: int = 1,
    upstream_defects: str = "fix",
    afni_settings: list[tuple[str, str]] | None = None,
) -> tuple[AfniOutput, AfniOutput | None, AfniOutput | None, list[str], list[Notice]]:
    """3dUnifize on a NIfTI file read with AFNI's rules: the result, the scale factors (with
    ``save_scale``), the automasked input (with ``save_automask``), AFNI's warnings and
    upstream-defect notices. ``ValueError`` for AFNI's fatal errors and for ``t2`` above 2."""

def afni_volreg_cli(
    args: list[str],
    n_threads: int = 1,
    upstream_defects: str = "fix",
    afni_settings: list[tuple[str, str]] | None = None,
) -> tuple[int, str, str]:
    """3dvolreg's clean-room original with its original arguments and AFNI's variables
    ``afni_settings`` (the process environment is not read): ``(exit status, stdout, stderr)``."""

def afni_volreg_savedisp(
    dx: np.ndarray, dy: np.ndarray, dz: np.ndarray
) -> list[tuple[str, np.ndarray]]:
    """3dvolreg's ``-SAVEDISP`` arithmetic on three raw displacement series (each
    ``(volumes, voxels)`` float32, as ``-savedisp`` writes them): the three normalised and their
    sixteen products, ``(suffix, series)`` in the order the program writes them."""

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

def ndimage_affine_transform3(
    input: np.ndarray,
    matrix: list[float],
    offset: list[float],
    output_shape: list[int],
    order: int = 3,
    mode: str = "constant",
    cval: float = 0.0,
    prefilter: bool = True,
    output: str = "float64",
    n_threads: int = 1,
    upstream_defects: str = "fix",
    scipy_version: str = "1.18.1",
) -> tuple[np.ndarray, list[Notice]]:
    """``scipy.ndimage.affine_transform`` of a 3-D ``input``: ``matrix`` 9 values (a 3 x 3
    matrix, row by row: its offset added first, or last before SciPy 1.16, ``scipy_version``) or
    3 (a diagonal: SciPy's ``zoom_shift`` path); the output's values in Fortran order and the
    upstream-defect notices."""

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

# --- SciPy-compatible filters, morphology, labelling (larmorx/_ndimage_filters.py) ------------
# Inputs of any layout (bool as uint8 views); C-ordered results of the input's type.

def ndimage_min_or_max_filter(
    input: np.ndarray,
    footprint: np.ndarray,
    structure: np.ndarray | None,
    mode: str,
    cval: float,
    origins: list[int],
    minimum: bool,
    n_threads: int = 1,
    output: str | None = None,
) -> np.ndarray:
    """``_nd_image.min_or_max_filter``: the minimum or maximum over a boolean ``footprint``,
    with an optional float64 ``structure`` of the footprint's shape; ``output``: the name of
    the type written (``"bool"`` or a numpy type name; the input's by default)."""

def ndimage_min_or_max_filter1d(
    input: np.ndarray,
    size: int,
    axis: int,
    mode: str,
    cval: float,
    origin: int,
    minimum: bool,
    n_threads: int = 1,
    output: str | None = None,
) -> np.ndarray:
    """``_nd_image.min_or_max_filter1d``: the minimum or maximum over ``size`` along ``axis``;
    ``output``: the name of the type written (the input's by default)."""

def ndimage_correlate1d(
    input: np.ndarray,
    weights: list[float],
    axis: int,
    mode: str,
    cval: float,
    origin: int,
    n_threads: int = 1,
    output: str | None = None,
) -> np.ndarray:
    """``_nd_image.correlate1d``: the lines along ``axis`` correlated with ``weights`` and cast
    to ``output`` (the input's type by default)."""

def ndimage_rank_filter(
    input: np.ndarray,
    rank: int,
    footprint: np.ndarray,
    mode: str,
    cval: float,
    origins: list[int],
    n_threads: int = 1,
    output: str | None = None,
) -> np.ndarray:
    """``_nd_image.rank_filter``: the ``rank``-th smallest over a boolean ``footprint``;
    ``output``: the name of the type written (the input's by default)."""

def ndimage_rank_filter_1d(
    input: np.ndarray, rank: int, win_len: int, mode: str, cval: float, origin: int
) -> np.ndarray:
    """``_rank_filter_1d.rank_filter(x, rank, win_len, x_out, mode, cval, origin)`` of a
    float32, float64 or int64 1-D array (Ashelly's running rank): an array of the input's type
    (``cval`` cast to it)."""

def ndimage_binary_erosion(
    input: np.ndarray,
    structure: np.ndarray,
    origins: list[int],
    iterations: int,
    border_value: bool,
    invert: bool,
    brute_force: bool,
    mask: np.ndarray | None = None,
    n_threads: int = 1,
    protect_later: bool = True,
) -> np.ndarray:
    """``_binary_erosion`` after the wrapper's checks (``invert``: a dilation with the reversed
    structure): uint8 0/1 values. ``protect_later=False``: SciPy's later passes into a float
    output array, which protect no element outside the mask (SCI-25 reproduced)."""

def ndimage_label(input: np.ndarray, structure: np.ndarray) -> tuple[np.ndarray, int]:
    """``_ni_label._label`` with SciPy's default output: ``(int32 or int64 labels, number)``."""

def ndimage_sum_labels(input: np.ndarray, labels: np.ndarray, index: list[int]) -> list[float]:
    """The Rust kernel of ``sum_labels`` for a sequence ``index``
    (``larmorx_ndimage::sum_labels_bincount``): the float64 sum per label, the elements added
    in C order, an absent label 0 (SciPy's ``bincount`` and ``unique`` paths give the same
    sums). The Python wrapper ``lx.ndimage.sum_labels`` does not call it: it follows SciPy's
    ``_stats`` with numpy's ``bincount`` itself (SciPy's argument handling, relay 22 of the
    field maps' part B); this binding exposes the crate's own kernel, kept equal to that path
    by ``tests/python/test_ndimage_sum_labels_binding.py``."""

def ndimage_watershed_ift(
    input: np.ndarray,
    markers: np.ndarray,
    structure: np.ndarray,
    output_bits: int,
    reproduce: bool,
) -> tuple[np.ndarray, tuple[bool, bool, bool, bool], int]:
    """``NI_WatershedIFT`` (uint8 or uint16 input, int8 to int64 markers, a signed output of
    ``output_bits`` bits): ``(int64 labels, (across_edge, stale_queue, truncated, oversize),
    differ)``; SciPy's labels with ``reproduce``, else the corrected ones; the events are SciPy's
    run's and ``differ`` the number of elements the two runs label differently."""

def numpy_percentile_weak(a: np.ndarray, q: float, numpy_version: str) -> float:
    """``numpy.percentile(a, q)`` (linear) with a Python-number ``q`` as numpy
    ``numpy_version`` (``"2.3"``, ``"2.5"``, ...) computes it; the value in the array's type."""

def numpy_add_reduce(a: np.ndarray, numpy_version: str) -> float:
    """``numpy.add.reduce(a, axis=None)`` of a float32 or float64 array taken in C order, as
    numpy ``numpy_version`` adds it (pairwise from 2.3; chunks of 8192 before)."""

def numpy_percentile(a: np.ndarray, q: list[float]) -> list[float]:
    """``numpy.percentile(a, q)`` (linear) with a list ``q``: float64 values."""

def numpy_median(a: np.ndarray, axis: int | None = None) -> np.ndarray:
    """``numpy.median(a, axis)`` of a float32 or float64 array."""

def numpy_exp_f32(a: np.ndarray) -> np.ndarray:
    """numpy's float32 ``exp`` (its AVX2/AVX-512 kernel), elementwise."""

def numpy_exp_f64(a: np.ndarray) -> np.ndarray:
    """numpy's float64 ``exp`` on its path without AVX-512 (glibc's), elementwise."""

def numpy_matvec_f64(a: np.ndarray, x: np.ndarray) -> np.ndarray:
    """numpy's ``a.dot(x)`` of a 2-D float64 matrix and a float64 vector (``cblas_matrixproduct``'s
    shapes; OpenBLAS's Haswell ``dgemv_t``, one thread)."""

def numpy_clip_f32(a: np.ndarray, a_min: float, a_max: float) -> np.ndarray:
    """``numpy.clip`` of a float32 array with float32 bounds."""

def numpy_histogram(a: np.ndarray, nbins: int) -> tuple[np.ndarray, np.ndarray]:
    """``numpy.histogram(a, bins=nbins)`` of a float32 or float64 array: ``(int64 counts,
    edges)``."""

def numpy_dot(x: np.ndarray, y: np.ndarray) -> float:
    """``np.dot`` of two float32 or float64 vectors (OpenBLAS's Haswell kernels)."""

# --- scikit-image (larmorx/skimage) and sdcflows' brain_masker -------------------------------

def skimage_ball(radius: float, strict_radius: bool) -> np.ndarray:
    """``skimage.morphology.ball`` as a bool array."""

def skimage_threshold_otsu(image: np.ndarray, nbins: int) -> float:
    """``threshold_otsu`` of a float32 or float64 image (the value in its type)."""

def skimage_threshold_otsu_int(image: np.ndarray) -> int:
    """``threshold_otsu`` of an integer image (int64 values)."""

def skimage_threshold_otsu_hist(counts: np.ndarray, centers: np.ndarray) -> int:
    """``threshold_otsu(hist=(counts, centers))``: float32 counts, float32 or float64 centres;
    the index of the chosen centre."""

def skimage_random_walker(
    data: np.ndarray,
    labels: np.ndarray,
    multichannel: bool,
    spacing: np.ndarray,
    std: float | None,
    beta: float,
    mode: str,
    tol: float,
    prob_tol: float,
    scipy_version: str,
    numbering: str,
    return_full_prob: bool,
    n_threads: int,
    numpy_version: str | None = None,
) -> dict[str, Any]:
    """``random_walker`` after scikit-image's checks (``larmorx.skimage.random_walker``)."""

def sdcflows_brain_masker(
    data: np.ndarray,
    zooms: list[float],
    padding: int,
    numpy_version: str,
    scipy_version: str,
    reproduce: bool,
    n_threads: int,
) -> tuple[np.ndarray, np.ndarray, float, list[int], list[str]]:
    """sdcflows' ``brain_masker`` of a float32 3-D array: ``(probseg, mask, threshold,
    iterations, warnings)``."""

# --- nitransforms chains and fMRIPrep's one-shot resampler (larmorx/transforms) ---------------

Step = tuple[Any, ...]  # ("affine", 4x4) or ("field", deltas (x, y, z, 3), affine, inverse)

def linalg_matmul4(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """``a[t] @ b[t]`` for two ``(n, 4, 4)`` float64 stacks, in the operation order of numpy's
    OpenBLAS Haswell ``dgemm``."""

def linalg_inv4(a: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """``np.linalg.inv`` of an ``(n, 4, 4)`` float64 stack as OpenBLAS's ``dgesv`` computes it;
    NaN for singular matrices, and which were singular (bool, ``(n,)``)."""

def linalg_scipy_inv4(
    a: np.ndarray, scipy_version: str
) -> tuple[str, str, np.ndarray | None, np.ndarray | None, float]:
    """``scipy.linalg.inv`` of a finite float64 4×4 matrix as SciPy ``scipy_version`` (1.15 or
    1.18) computes it: ``(structure, status, inverse, lu, rcond)``; ``status`` is "ok",
    "ill_conditioned", "singular" or "norm_overflow" (SCI-18: ``lu`` is SciPy's result);
    ``rcond`` is the value 1.18 computes and prints (0 for 1.15 and for SCI-18)."""

def linalg_affine_points(affine: np.ndarray, points: np.ndarray, n_threads: int = 1) -> np.ndarray:
    """``affine.dot(np.vstack((points.T, ones)))[:3].T`` for ``(n, 3)`` float64 points, in the
    operation order of numpy's OpenBLAS Haswell ``dgemm`` (``dgemv`` for one point)."""

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
    upstream-defect notices (PIPE-03)."""

# --- ANTs image programs: ImageMath, ThresholdImage, MultiplyImages (larmorx/ants) -------------

# data, RAS+ affine, descrip[, spacing of the axes after the third (the time step in seconds)]
ImageTuple = (
    tuple[np.ndarray, np.ndarray, bytes | None]
    | tuple[np.ndarray, np.ndarray, bytes | None, list[float] | None]
)

def ants_program(
    program: str,
    args: list[str],
    inputs: dict[str, ImageTuple | str],
    outputs: list[str],
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[int, str, str, dict[str, tuple[np.ndarray, np.ndarray, bytes]]]:
    """Run ``ImageMath``, ``ThresholdImage``, ``MultiplyImages``, ``SmoothImage``, ``N4BiasFieldCorrection``,
    ``DenoiseImage``, ``ResampleImageBySpacing``, ``ResampleImage``, ``antsRegistration``,
    ``antsAI``, ``antsAffineInitializer`` or ``Atropos`` with ``args`` on in-memory ``inputs``
    (by placeholder name; a string value is a file the name stands for; other names are files);
    writes to ``outputs`` names are returned:
    ``(exit_code, stdout, stderr, {name: (data, ras_affine, descrip)})``. The program runs in
    the D13 mode ``upstream_defects``; its notices are lines of ``stderr``."""

def ants_file_program(
    program: str, args: list[str], n_threads: int, upstream_defects: str
) -> tuple[int, str, str]:
    """Run ``antsApplyTransforms`` with ``args`` on files (transform files read as the program
    reads them), on ``n_threads`` threads, in the D13 mode ``upstream_defects``:
    ``(exit_code, stdout, stderr)``."""

def ants_image_math_operations() -> list[tuple[str, str]]:
    """The ImageMath operations larmorx implements: ``(name, usage)``."""

def ants_blob_alignment(
    fixed: ImageTuple | str,
    moving: ImageTuple | str,
    dim: int,
    blobs_to_extract: int,
    blobs_to_match: int,
    affine: bool,
    n_threads: int,
    upstream_defects: str,
) -> tuple[dict[str, Any] | None, str | None, list[Notice]]:
    """``antsAI``'s blob alignment with everything it computed: ``(result, error, notices)``;
    ``result`` holds ``fixed_blobs`` and ``moving_blobs`` (``(index, sigma, radius, value)``),
    ``correspondence`` (row-major), ``pairs``, ``landmarks``, ``fixed_landmarks``,
    ``moving_landmarks`` (LPS), ``matrix``, ``offset`` and ``messages``; ``error`` what the
    command line would print where it stops."""

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
    a: np.ndarray,
    operation: str,
    radius: int = 1,
    value: float = 1.0,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[Notice]]:
    """ImageMath ``MD``/``ME``/``MO``/``MC``/``GD``/``GE``/``GO``/``GC`` (``ants::Morphological``)
    with ITK's ball of ``radius`` voxels; ``value`` is the binary operations' foreground
    (``ME``, and ``MO`` with the foreground 0, corrected: ANTS-17): ``(data, notices)``."""

def ants_fill_holes(
    a: np.ndarray, hole_param: float = 2.0, n_threads: int = 1, upstream_defects: str = "fix"
) -> tuple[np.ndarray, list[Notice]]:
    """ImageMath ``FillHoles``: the background regions other than the largest set to 1 (all for
    ``hole_param`` 2; by their share of object neighbours for ``hole_param ≤ 1``): ``(data,
    notices)``."""

def ants_pad_image(
    image: str | ImageTuple,
    dim: int,
    pad: float,
    value: float = 0.0,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[tuple[np.ndarray, np.ndarray, bytes, tuple[float, ...]], list[Notice]]:
    """ImageMath ``PadImage`` on a path (read in ``dim`` dimensions) or an in-memory image,
    ``int(pad)`` voxels on each side (ANTS-22): ``((data, ras_affine, descrip, spacing),
    notices)``."""

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
    """ImageMath ``ExtractContours``: the voxels of each label (every distinct value; ANTs
    truncates the values, ANTS-23) that touch another label, keeping their value; 0
    elsewhere: ``(data, notices)``."""

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

def mri_hmc_world(
    matrices: np.ndarray, params: np.ndarray, series: str, reference: str | None = None
) -> dict[str, Any]:
    """``ras``, ``itk`` (``(N, 4, 4)``) and ``fd`` (``(N,)``) of mcflirt-convention matrices and
    motion parameters computed by the ``larmorx-nc`` replica, for the series at ``series``
    registered to the reference at ``reference`` (``None``: the series' grid)."""

# --- sdcflows' field-map estimation, ported (larmorx/sdcflows) ---------------------------------

def sdcflows_bspline_grid(
    shape: tuple[int, int, int],
    affine: np.ndarray,
    zooms: list[float],
    control_zooms_mm: tuple[float, float, float],
) -> tuple[tuple[int, int, int], np.ndarray]:
    """sdcflows' ``bspline_grid``: the control grid's shape and affine for an image of
    ``shape`` and ``affine`` whose header zooms are ``zooms``."""

def sdcflows_collocation_csr(
    target_shape: tuple[int, int, int],
    target_affine: np.ndarray,
    levels: list[tuple[tuple[int, int, int], np.ndarray]],
    dtype: str = "float32",
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[
    np.ndarray, np.ndarray, np.ndarray, tuple[int, int], list[bool], list[tuple[str, str, str, str]]
]:
    """``sparse_hstack([grid_bspline_weights(target, level) ...]).tocsr()``: data (float64,
    holding the matrix type's values), indices, indptr (int64), shape, one alignment flag per
    level, and the notices of SDC-01 and SDC-02 (corrected with ``"fix"``, sdcflows' matrix bit
    for bit with ``"reproduce"``)."""

def sdcflows_collocation_apply(
    target_shape: tuple[int, int, int],
    target_affine: np.ndarray,
    levels: list[tuple[tuple[int, int, int], np.ndarray]],
    v: np.ndarray,
    transpose: bool = False,
    rows: np.ndarray | None = None,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[tuple[str, str, str, str]]]:
    """``(X @ v, notices)`` (or ``X.T @ v``; ``X[rows] @ v`` with ``rows``) for the float32
    collocation matrix, in scipy's summation order, without storing the matrix; the notices of
    SDC-01 and SDC-02. ``X @ v`` is float64 where scipy's stacked matrix is (a level with an
    axis of no weights: scipy's ``kron`` returns an empty float64 matrix)."""

def sdcflows_dot_f32(x: np.ndarray, y: np.ndarray) -> float:
    """``np.dot(x, y)`` of two float32 vectors (numpy over OpenBLAS's Haswell ``sdot``)."""

def sdcflows_cg_csr(
    data: np.ndarray,
    indices: np.ndarray,
    indptr: np.ndarray,
    shape: tuple[int, int],
    b: np.ndarray,
    rtol: float = 1e-5,
    atol: float = 0.0,
    maxiter: int | None = None,
) -> tuple[np.ndarray, int, int]:
    """SciPy's ``cg`` for a float32 CSR matrix: ``(x, info, iterations)``."""

def sdcflows_ridge_csr(
    data: np.ndarray,
    indices: np.ndarray,
    indptr: np.ndarray,
    shape: tuple[int, int],
    y: np.ndarray,
    alpha: float = 1.0,
    tol: float = 1e-4,
    max_iter: int | None = None,
    n_threads: int = 1,
) -> tuple[np.ndarray, int, int, str]:
    """scikit-learn's ``Ridge(fit_intercept=False)`` ``sparse_cg`` fit of a float32 CSR
    matrix: ``(coef, info, iterations, system)``."""

def sdcflows_ridge_collocation(
    target_shape: tuple[int, int, int],
    target_affine: np.ndarray,
    levels: list[tuple[tuple[int, int, int], np.ndarray]],
    y: np.ndarray,
    alpha: float = 1e-4,
    tol: float = 1e-4,
    max_iter: int | None = None,
    n_threads: int = 1,
) -> tuple[np.ndarray, int, int, str]:
    """The same fit on sdcflows' float32 collocation matrix, without storing it."""

def sdcflows_bspline_approx(
    data: np.ndarray,
    shape: tuple[int, int, int],
    affine: np.ndarray,
    zooms: list[float],
    spacings: list[tuple[float, float, float]],
    mask: np.ndarray | None = None,
    ridge_alpha: float = 1e-4,
    recenter: str = "median",
    extrapolate: bool = True,
    name: str = "",
    resampled: tuple[np.ndarray, tuple[int, int, int], np.ndarray, np.ndarray | None] | None = None,
    n_threads: int = 1,
    upstream_defects: str = "fix",
    low_memory: bool = False,
) -> dict[str, Any]:
    """sdcflows' ``BSplineApprox`` on a loaded field map (flat float32 data, flat bool mask):
    grids, coefficients, center, n_inside, field, error, extrapolated, extreme, info, n_iter,
    system, log, beyond (whether SDC-01 changed, or would have changed, the weights). The fit
    reads the stored collocation matrix at one thread and evaluates the weights at every
    product with several threads or with ``low_memory`` (the same bits)."""

def sdcflows_rescale_affine(
    affine: np.ndarray,
    shape: tuple[int, int, int],
    zooms: tuple[float, float, float],
    new_shape: tuple[float, float, float],
) -> np.ndarray:
    """nibabel's ``rescale_affine(affine, shape, zooms, new_shape)`` (numpy's BLAS order)."""

def sdcflows_resample_to_zooms_grid(
    shape: tuple[int, int, int],
    affine: np.ndarray,
    pre_zooms: tuple[float, float, float],
    zooms: tuple[float, float, float],
) -> tuple[tuple[float, float, float], np.ndarray]:
    """The grid of sdcflows' ``resample_to_zooms``: ``np.rint(shape * pre_zooms / zooms)``
    (float64 values) and its affine."""

def sdcflows_affine_coordinates(
    ref_shape: tuple[int, int, int],
    ref_affine: np.ndarray,
    moving_affine: np.ndarray,
    matrix: np.ndarray | None = None,
    n_threads: int = 1,
) -> np.ndarray:
    """``(3, n)`` voxel coordinates in the moving image of every reference voxel (Fortran
    order), as nitransforms' ``Affine(matrix, reference=ref).apply(moving)`` computes them (the
    identity without ``matrix``)."""

def sdcflows_apply_affine(affine: np.ndarray, points: np.ndarray) -> np.ndarray:
    """nibabel's ``apply_affine(affine, points)`` for ``(n, 3)`` points (numpy's BLAS order)."""

def sdcflows_unwarp_volume(
    volume: np.ndarray,
    fmap_hz: np.ndarray,
    pe_axis: int,
    ro_time: float,
    ro_float64: bool,
    jacobian: bool,
    order: int,
    mode: str,
    cval: float,
    prefilter: bool,
    n_threads: int = 1,
    upstream_defects: str = "fix",
) -> tuple[np.ndarray, list[tuple[str, str, str, str]]]:
    """sdcflows' ``_sdc_unwarp`` for one 3D volume: the volume sampled at its voxels moved
    along ``pe_axis`` by ``fmap_hz * ro_time`` (float32 arithmetic, or float64 with
    ``ro_float64``), with SciPy's ``map_coordinates``, times the Jacobian of the shift with
    ``jacobian``. Returns the float32 volume (C order) and the interpolation's notices."""

# --- the confounds' part: OpenBLAS's level-2/3 kernels and numpy's 2-D matmul -------------------

def blas_dgemm(
    transa: str,
    transb: str,
    m: int,
    n: int,
    k: int,
    alpha: float,
    a: np.ndarray,
    lda: int,
    b: np.ndarray,
    ldb: int,
    beta: float,
    c: np.ndarray,
    ldc: int,
    n_threads: int = 1,
    gemm_q: int = 256,
) -> np.ndarray:
    """``DGEMM`` on flat column-major float64 storage, as OpenBLAS computes it on the Haswell
    kernels at one thread (``gemm_q`` 512 follows OpenBLAS >= 0.3.31 on an AMD Zen 4/5);
    returns the new ``C`` storage."""

def blas_dgemv(
    trans_: str,
    m: int,
    n: int,
    alpha: float,
    a: np.ndarray,
    lda: int,
    x: np.ndarray,
    incx: int,
    beta: float,
    y: np.ndarray,
    incy: int,
) -> np.ndarray:
    """``DGEMV`` on flat float64 storage (positive increments), as OpenBLAS computes it on the
    Haswell kernels; returns the new ``Y``."""

def numpy_matmul(a: np.ndarray, b: np.ndarray, n_threads: int = 1, gemm_q: int = 256) -> np.ndarray:
    """``a @ b`` of two C- or F-contiguous 2-D float64 arrays as numpy dispatches it onto
    OpenBLAS's Haswell kernels (``dsyrk`` for ``x @ x.T`` and ``x.T @ x``); C-ordered."""

def blas_dtrmm(
    side: str,
    uplo: str,
    transa: str,
    diag: str,
    m: int,
    n: int,
    alpha: float,
    a: np.ndarray,
    lda: int,
    b: np.ndarray,
    ldb: int,
    n_threads: int = 1,
    gemm_q: int = 256,
) -> np.ndarray:
    """``DTRMM`` (every side, triangle, transpose and diagonal) on flat column-major float64
    storage, as OpenBLAS computes it on the Haswell kernels at one thread; returns the new
    ``B`` storage."""

def blas_dger(
    m: int,
    n: int,
    alpha: float,
    x: np.ndarray,
    incx: int,
    y: np.ndarray,
    incy: int,
    a: np.ndarray,
    lda: int,
) -> np.ndarray:
    """``DGER`` (any nonzero increments); returns the new ``A`` storage."""

def blas_dtrmv(
    uplo: str, trans_: str, diag: str, n: int, a: np.ndarray, lda: int, x: np.ndarray, incx: int
) -> np.ndarray:
    """``DTRMV`` (any nonzero increment); returns the new ``X`` storage."""

def blas_daxpy(
    n: int, alpha: float, x: np.ndarray, incx: int, y: np.ndarray, incy: int
) -> np.ndarray:
    """``DAXPY`` (any increments); returns the new ``Y`` storage."""

def blas_dscal(n: int, alpha: float, x: np.ndarray, incx: int) -> np.ndarray:
    """``DSCAL`` (nothing for ``incx <= 0``); returns the new ``X`` storage."""

def blas_dcopy(n: int, x: np.ndarray, incx: int, y: np.ndarray, incy: int) -> np.ndarray:
    """``DCOPY``; returns the new ``Y`` storage."""

def blas_dswap(
    n: int, x: np.ndarray, incx: int, y: np.ndarray, incy: int
) -> tuple[np.ndarray, np.ndarray]:
    """``DSWAP`` of two separate vectors; returns the new ``(X, Y)`` storage."""

def blas_idamax(n: int, x: np.ndarray, incx: int, aligned: bool) -> int:
    """``IDAMAX`` (1-based, 0 for ``n = 0`` or ``incx <= 0``) as OpenBLAS's ``iamax_sse2.S``
    computes it; ``aligned``: whether ``x(1)`` lies on a 16-byte boundary in the original's
    memory (only NaN input can tell)."""

def blas_dnrm2(n: int, x: np.ndarray, incx: int) -> float:
    """``DNRM2`` (OpenBLAS's x87 kernel)."""

# --- the confounds' part: LAPACK's QR and LQ routines and their Q (numpy_linalg::blocked) -----

def lapack_dgeqrf(
    m: int,
    n: int,
    a: np.ndarray,
    lda: int,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, int]:
    """LAPACK's blocked ``DGEQRF`` on flat column-major float64 storage, as numpy's OpenBLAS
    computes it on the Haswell kernels at one thread (``lapack`` "3.12.1": numpy 2.5.4,
    "3.12.0": numpy 2.3.5; ``lwork=None``: the optimal workspace); returns ``(A, TAU, WORK(1))``.
    Raises ``ValueError`` with LAPACK's message on an illegal argument."""

def lapack_dgelqf(
    m: int,
    n: int,
    a: np.ndarray,
    lda: int,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, int]:
    """LAPACK's blocked ``DGELQF``, as ``lapack_dgeqrf``."""

def lapack_dgeqr2(
    m: int, n: int, a: np.ndarray, lda: int, lapack: str = "3.12.1"
) -> tuple[np.ndarray, np.ndarray]:
    """LAPACK's ``DGEQR2``; returns ``(A, TAU)``."""

def lapack_dgelq2(
    m: int, n: int, a: np.ndarray, lda: int, lapack: str = "3.12.1"
) -> tuple[np.ndarray, np.ndarray]:
    """LAPACK's ``DGELQ2``; returns ``(A, TAU)``."""

def lapack_dlarft(
    direct: str,
    storev: str,
    n: int,
    k: int,
    v: np.ndarray,
    ldv: int,
    tau: np.ndarray,
    t: np.ndarray,
    ldt: int,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> np.ndarray:
    """LAPACK's ``DLARFT`` (3.12.0: recursive; 3.12.1: ``DLARFT_LVL2`` below 64 reflectors);
    ``t`` is the initial ``T`` storage; returns the new ``T`` storage."""

def lapack_dlarfb(
    side: str,
    trans_: str,
    direct: str,
    storev: str,
    m: int,
    n: int,
    k: int,
    v: np.ndarray,
    ldv: int,
    t: np.ndarray,
    ldt: int,
    c: np.ndarray,
    ldc: int,
    n_threads: int = 1,
    gemm_q: int = 256,
) -> np.ndarray:
    """LAPACK's ``DLARFB`` (every side, transpose, direction and storage); returns the new ``C``
    storage."""

def lapack_dlarf(
    side: str,
    m: int,
    n: int,
    v: np.ndarray,
    incv: int,
    tau: float,
    c: np.ndarray,
    ldc: int,
    implicit_one: bool = False,
) -> np.ndarray:
    """LAPACK's ``DLARF`` (or 3.12.1's ``DLARF1F`` with ``implicit_one``), positive ``incv``;
    returns the new ``C`` storage."""

def lapack_dlarfg(
    n: int, alpha: float, x: np.ndarray, incx: int
) -> tuple[float, np.ndarray, float]:
    """LAPACK's ``DLARFG`` (positive ``incx``); returns ``(beta, X, tau)``."""

def lapack_dorgqr(
    m: int,
    n: int,
    k: int,
    a: np.ndarray,
    lda: int,
    tau: np.ndarray,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, int]:
    """LAPACK's blocked ``DORGQR`` (the first ``n`` columns of ``Q`` from ``k`` reflectors of
    ``DGEQRF``), as ``lapack_dgeqrf``; returns ``(A, WORK(1))``."""

def lapack_dorglq(
    m: int,
    n: int,
    k: int,
    a: np.ndarray,
    lda: int,
    tau: np.ndarray,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, int]:
    """LAPACK's blocked ``DORGLQ``, as ``lapack_dorgqr``."""

def lapack_dorg2r(
    m: int, n: int, k: int, a: np.ndarray, lda: int, tau: np.ndarray, lapack: str = "3.12.1"
) -> np.ndarray:
    """LAPACK's ``DORG2R``; returns the new ``A`` storage."""

def lapack_dorgl2(
    m: int, n: int, k: int, a: np.ndarray, lda: int, tau: np.ndarray, lapack: str = "3.12.1"
) -> np.ndarray:
    """LAPACK's ``DORGL2``; returns the new ``A`` storage."""

def lapack_dormqr(
    side: str,
    trans_: str,
    m: int,
    n: int,
    k: int,
    a: np.ndarray,
    lda: int,
    tau: np.ndarray,
    c: np.ndarray,
    ldc: int,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, int]:
    """LAPACK's blocked ``DORMQR`` (``Q`` of ``DGEQRF``'s reflectors applied to ``C`` from either
    side, transposed or not; ``lwork=None``: the optimal workspace, a smaller one lowers the
    block size as each release does); returns ``(A, C, WORK(1))``."""

def lapack_dormlq(
    side: str,
    trans_: str,
    m: int,
    n: int,
    k: int,
    a: np.ndarray,
    lda: int,
    tau: np.ndarray,
    c: np.ndarray,
    ldc: int,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, int]:
    """LAPACK's blocked ``DORMLQ``, as ``lapack_dormqr``."""

def lapack_dorm2r(
    side: str,
    trans_: str,
    m: int,
    n: int,
    k: int,
    a: np.ndarray,
    lda: int,
    tau: np.ndarray,
    c: np.ndarray,
    ldc: int,
    lapack: str = "3.12.1",
) -> tuple[np.ndarray, np.ndarray, int]:
    """LAPACK's ``DORM2R``; returns ``(A, C, 0)``."""

def lapack_dorml2(
    side: str,
    trans_: str,
    m: int,
    n: int,
    k: int,
    a: np.ndarray,
    lda: int,
    tau: np.ndarray,
    c: np.ndarray,
    ldc: int,
    lapack: str = "3.12.1",
) -> tuple[np.ndarray, np.ndarray, int]:
    """LAPACK's ``DORML2``; returns ``(A, C, 0)``."""

def lapack_dgebrd(
    m: int,
    n: int,
    a: np.ndarray,
    lda: int,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, int]:
    """LAPACK's blocked ``DGEBRD`` (``A = Q·B·Pᵀ``, ``B`` bidiagonal), as ``lapack_dgeqrf``;
    returns ``(A, D, E, TAUQ, TAUP, WORK(1))``."""

def lapack_dgebd2(
    m: int, n: int, a: np.ndarray, lda: int, lapack: str = "3.12.1"
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, int]:
    """LAPACK's ``DGEBD2``; returns ``(A, D, E, TAUQ, TAUP, 0)``."""

def lapack_dlabrd(
    m: int,
    n: int,
    nb: int,
    a: np.ndarray,
    lda: int,
    x: np.ndarray,
    ldx: int,
    y: np.ndarray,
    ldy: int,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    """LAPACK's ``DLABRD`` (the first ``nb`` rows and columns reduced, with the update matrices
    ``X`` and ``Y``; ``x``, ``y`` the initial workspaces); returns ``(A, D, E, TAUQ, TAUP, X,
    Y)``."""

# The divide-and-conquer bidiagonal SVD (the confounds' part, relay 2c-i): LAPACK's DBDSDC and
# its routines at their Fortran interface, flat column-major storage, 1-based int64 indices.

def lapack_dbdsdc(
    uplo_: str,
    compq: str,
    n: int,
    d: np.ndarray,
    e: np.ndarray,
    ldu: int,
    ldvt: int,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, int]:
    """LAPACK's ``DBDSDC(UPLO, 'I', ...)`` (only ``compq="I"``): the singular values (descending)
    and vectors of an ``n x n`` bidiagonal matrix; returns ``(D, E, U, VT, INFO)``."""

def lapack_dlasd0(
    n: int,
    sqre: int,
    d: np.ndarray,
    e: np.ndarray,
    u: np.ndarray,
    ldu: int,
    vt: np.ndarray,
    ldvt: int,
    smlsiz: int,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, int]:
    """LAPACK's ``DLASD0`` (divide and conquer on an ``n x (n + sqre)`` upper bidiagonal
    matrix); returns ``(D, E, U, VT, INFO)``."""

def lapack_dlasd1(
    nl: int,
    nr: int,
    sqre: int,
    d: np.ndarray,
    alpha: float,
    beta: float,
    u: np.ndarray,
    ldu: int,
    vt: np.ndarray,
    ldvt: int,
    idxq: np.ndarray,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, float, float, np.ndarray, np.ndarray, np.ndarray, int]:
    """LAPACK's ``DLASD1`` (one merge); returns ``(D, ALPHA, BETA, U, VT, IDXQ, INFO)``."""

def lapack_dlasd2(
    nl: int,
    nr: int,
    sqre: int,
    d: np.ndarray,
    alpha: float,
    beta: float,
    u: np.ndarray,
    ldu: int,
    vt: np.ndarray,
    ldvt: int,
    ldu2: int,
    ldvt2: int,
    idxq: np.ndarray,
    lapack: str = "3.12.1",
) -> tuple[
    int,
    tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray],
    tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray],
]:
    """LAPACK's ``DLASD2`` (the merge's deflation); returns ``(K, (D, Z, U, VT, DSIGMA, U2,
    VT2), (IDXP, IDX, IDXC, IDXQ, COLTYP))``."""

def lapack_dlasd3(
    nl: int,
    nr: int,
    sqre: int,
    k: int,
    d: np.ndarray,
    ldq: int,
    dsigma: np.ndarray,
    u: np.ndarray,
    ldu: int,
    u2: np.ndarray,
    ldu2: int,
    vt: np.ndarray,
    ldvt: int,
    vt2: np.ndarray,
    ldvt2: int,
    idxc: np.ndarray,
    ctot: tuple[int, int, int, int] | list[int],
    z: np.ndarray,
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, int]:
    """LAPACK's ``DLASD3`` (the secular equation and the vectors' update); returns ``(D, Q, U,
    VT, VT2, Z, INFO)``."""

def lapack_dlasd4(
    n: int, i: int, d: np.ndarray, z: np.ndarray, rho: float
) -> tuple[np.ndarray, float, np.ndarray, int]:
    """LAPACK's ``DLASD4`` (the ``i``-th root of the secular equation); returns ``(DELTA,
    SIGMA, WORK, INFO)``."""

def lapack_dlasd5(
    i: int, d: np.ndarray, z: np.ndarray, rho: float
) -> tuple[np.ndarray, float, np.ndarray]:
    """LAPACK's ``DLASD5`` (the 2 x 2 secular equation); returns ``(DELTA, DSIGMA, WORK)``."""

def lapack_dlaed6(
    kniter: int,
    orgati: bool,
    rho: float,
    d: list[float] | tuple[float, float, float],
    z: list[float] | tuple[float, float, float],
    finit: float,
) -> tuple[float, int]:
    """LAPACK's ``DLAED6`` (a root of the three-pole rational function); returns ``(TAU,
    INFO)``."""

def lapack_dlasdq(
    uplo_: str,
    sqre: int,
    n: int,
    ncvt: int,
    nru: int,
    ncc: int,
    d: np.ndarray,
    e: np.ndarray,
    vt: np.ndarray,
    ldvt: int,
    u: np.ndarray,
    ldu: int,
    c: np.ndarray,
    ldc: int,
    lapack: str = "3.12.1",
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, int]:
    """LAPACK's ``DLASDQ`` (a leaf: ``DBDSQR`` with the vectors, values ascending); returns
    ``(D, E, VT, U, C, INFO)``."""

def lapack_dlasdt(n: int, msub: int) -> tuple[int, int, np.ndarray, np.ndarray, np.ndarray]:
    """LAPACK's ``DLASDT`` (the subproblem tree); returns ``(LVL, ND, INODE, NDIML, NDIMR)``."""

def lapack_dlamrg(n1: int, n2: int, a: np.ndarray, dtrd1: int, dtrd2: int) -> np.ndarray:
    """LAPACK's ``DLAMRG`` (the permutation merging two sorted lists); returns ``INDEX``."""

def lapack_dlascl(
    type_: str,
    kl: int,
    ku: int,
    cfrom: float,
    cto: float,
    m: int,
    n: int,
    a: np.ndarray,
    lda: int,
) -> np.ndarray:
    """LAPACK's ``DLASCL`` (only ``type_="G"``): ``A * cto/cfrom`` in safe steps; returns the new
    ``A`` storage."""

def lapack_dormbr(
    vect_: str,
    side: str,
    trans_: str,
    m: int,
    n: int,
    k: int,
    a: np.ndarray,
    lda: int,
    tau: np.ndarray,
    c: np.ndarray,
    ldc: int,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, int]:
    """LAPACK's ``DORMBR`` (``DGEBRD``'s ``Q`` or ``P`` applied to ``C``; ``lwork=None``: its own
    optimal workspace, handed to ``DORMQR``/``DORMLQ`` as it is); returns ``(A, C, WORK(1))``."""

def lapack_dorgbr(
    vect_: str,
    m: int,
    n: int,
    k: int,
    a: np.ndarray,
    lda: int,
    tau: np.ndarray,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, int]:
    """LAPACK's ``DORGBR`` (``DGEBRD``'s ``Q`` or ``P^T`` generated); returns ``(A, WORK(1))``."""

def lapack_dgesdd(
    jobz: str,
    m: int,
    n: int,
    a: np.ndarray,
    lda: int,
    u: np.ndarray,
    ldu: int,
    vt: np.ndarray,
    ldvt: int,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, int, int]:
    """LAPACK's ``DGESDD`` with ``JOBZ = 'S'`` (the thin SVD, all four paths); returns ``(A, S,
    U, VT, WORK(1), INFO)``."""

def lapack_dgesdd_lwork(m: int, n: int) -> tuple[int, int]:
    """``DGESDD('S')``'s ``(MINWRK, MAXWRK)``."""

def numpy_svd(
    a: np.ndarray, lapack: str = "3.12.1", n_threads: int = 1, gemm_q: int = 256
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """``numpy.linalg.svd(a, full_matrices=False)`` of a float64 matrix: ``(u, s, vt)``."""

def numpy_pinv(
    a: np.ndarray,
    rcond: float = 1e-15,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> np.ndarray:
    """``numpy.linalg.pinv(a, rcond)`` of a float64 matrix."""

def lapack_dlasrt(id: str, d: np.ndarray) -> np.ndarray:
    """LAPACK's ``DLASRT``: ``d`` sorted increasing (``"I"``) or decreasing (``"D"``)."""

def lapack_dlalsd(
    uplo_: str,
    smlsiz: int,
    n: int,
    nrhs: int,
    d: np.ndarray,
    e: np.ndarray,
    b: np.ndarray,
    ldb: int,
    rcond: float,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, int, int]:
    """LAPACK's ``DLALSD`` (``N <= SMLSIZ``; ``NotImplementedError`` beyond); returns ``(D, E,
    B, RANK, INFO)``."""

def lapack_dgelsd(
    m: int,
    n: int,
    nrhs: int,
    a: np.ndarray,
    lda: int,
    b: np.ndarray,
    ldb: int,
    rcond: float,
    lwork: int | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, int, int, int, int]:
    """LAPACK's ``DGELSD`` (``min(m, n) <= 25``; ``NotImplementedError`` beyond); returns ``(A,
    B, S, RANK, WORK(1), IWORK(1), INFO)``."""

def lapack_dgelsd_lwork(m: int, n: int, nrhs: int) -> tuple[int, int, int]:
    """``DGELSD``'s ``(MINWRK, MAXWRK, LIWORK)``."""

def numpy_lstsq(
    a: np.ndarray,
    b: np.ndarray,
    rcond: float | None = None,
    lapack: str = "3.12.1",
    n_threads: int = 1,
    gemm_q: int = 256,
) -> tuple[np.ndarray, np.ndarray, int, np.ndarray]:
    """``numpy.linalg.lstsq(a, b, rcond)`` of float64 arrays: ``(x, residuals, rank, s)``."""

def lapack_dlasd7(
    icompq: int,
    nl: int,
    nr: int,
    sqre: int,
    d: np.ndarray,
    z: np.ndarray,
    vf: np.ndarray,
    vl: np.ndarray,
    alpha: float,
    beta: float,
    dsigma: np.ndarray,
    idxq: np.ndarray,
    perm: np.ndarray,
    givcol: np.ndarray,
    ldgcol: int,
    givnum: np.ndarray,
    ldgnum: int,
    givptr: int,
    c: float,
    s: float,
    lapack: str = "3.12.1",
) -> dict[str, object]:
    """LAPACK's ``DLASD7`` (one merge's sorting and deflation): a dict of its outputs."""

def lapack_dlasd8(
    icompq: int,
    k: int,
    d: np.ndarray,
    z: np.ndarray,
    vf: np.ndarray,
    vl: np.ndarray,
    difl: np.ndarray,
    difr: np.ndarray,
    lddifr: int,
    dsigma: np.ndarray,
) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, int]:
    """LAPACK's ``DLASD8`` (one merge's secular equation): ``(D, Z, VF, VL, DIFL, DIFR, INFO)``."""

def lapack_dlasd6(
    icompq: int,
    nl: int,
    nr: int,
    sqre: int,
    d: np.ndarray,
    vf: np.ndarray,
    vl: np.ndarray,
    alpha: float,
    beta: float,
    idxq: np.ndarray,
    perm: np.ndarray,
    givcol: np.ndarray,
    ldgcol: int,
    givnum: np.ndarray,
    ldgnum: int,
    poles: np.ndarray,
    difl: np.ndarray,
    difr: np.ndarray,
    z: np.ndarray,
    givptr: int,
    c: float,
    s: float,
    lapack: str = "3.12.1",
) -> dict[str, object]:
    """LAPACK's ``DLASD6`` (one merge of ``DLASDA``'s tree): a dict of its outputs."""
