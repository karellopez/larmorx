"""``antsApplyTransforms`` and ``antsApplyTransformsToPoints``."""

from __future__ import annotations

import os
from collections.abc import Sequence
from typing import Any, Literal

import numpy as np

from larmorx import _core
from larmorx.image import Image, as_image
from larmorx.transforms import ItkTransform, read

__all__ = ["apply_transforms", "apply_transforms_to_points"]

PathLike = str | os.PathLike[str]
TransformLike = PathLike | ItkTransform | Sequence[ItkTransform]

# ANTs' -n names (lowercased) and short aliases.
_INTERPOLATORS = {
    "linear": "linear",
    "nearestneighbor": "nearestneighbor",
    "nearest": "nearestneighbor",
    "bspline": "bspline",
    "gaussian": "gaussian",
    "multilabel": "multilabel",
    "genericlabel": "genericlabel",
    "cosinewindowedsinc": "cosinewindowedsinc",
    "cosine": "cosinewindowedsinc",
    "hammingwindowedsinc": "hammingwindowedsinc",
    "hamming": "hammingwindowedsinc",
    "lanczoswindowedsinc": "lanczoswindowedsinc",
    "lanczos": "lanczoswindowedsinc",
    "blackmanwindowedsinc": "blackmanwindowedsinc",
    "blackman": "blackmanwindowedsinc",
    "welchwindowedsinc": "welchwindowedsinc",
    "welch": "welchwindowedsinc",
}


def _interpolator(name: str) -> str:
    key = name.lower().replace("_", "").replace("-", "")
    try:
        return _INTERPOLATORS[key]
    except KeyError:
        raise ValueError(
            f"unknown interpolation {name!r}; use one of: "
            "linear, nearest, bspline, gaussian, multilabel, genericlabel, "
            "lanczos, hamming, cosine, welch, blackman (or the ANTs names)"
        ) from None


def _transform_items(
    transforms: TransformLike | Sequence[TransformLike], invert: Sequence[bool] | None
) -> list[tuple[list[tuple[str, np.ndarray, np.ndarray]], bool]]:
    if isinstance(transforms, (str, os.PathLike, ItkTransform)):
        transforms = [transforms]
    items = list(transforms)
    if items and all(isinstance(t, ItkTransform) for t in items) and items[0].is_composite:
        items = [items]  # one composite given as its parts
    flags = [False] * len(items) if invert is None else [bool(f) for f in invert]
    if len(flags) != len(items):
        raise ValueError(f"{len(items)} transforms but {len(flags)} invert flags")
    out = []
    for t, flag in zip(items, flags, strict=True):
        if isinstance(t, (str, os.PathLike)):
            parts = read(t)
        elif isinstance(t, ItkTransform):
            parts = [t]
        else:
            parts = list(t)
        out.append(([p._as_tuple() for p in parts], flag))
    return out


def _grid_of(image: Image) -> dict[str, Any]:
    shape = (*image.shape[:3], 1, 1)[:3]
    return _core.grid_from_ras_affine(shape, image.affine)


def _cast(values: np.ndarray, dtype: Any) -> np.ndarray:
    """ANTs' output cast: floats round, integers truncate toward zero (saturating here)."""
    dtype = np.dtype(dtype)
    if dtype.kind in "iu":
        info = np.iinfo(dtype)
        values = np.nan_to_num(np.trunc(values), nan=0.0)
        return np.clip(values, info.min, info.max).astype(dtype)
    return values.astype(dtype, copy=False)


def apply_transforms(
    image: Any,
    reference: Any,
    transforms: TransformLike | Sequence[TransformLike] = (),
    *,
    invert: Sequence[bool] | None = None,
    interpolation: str = "linear",
    sigma: float | Sequence[float] | None = None,
    alpha: float | None = None,
    order: int = 3,
    time_series: bool | None = None,
    default_value: float = 0.0,
    dtype: Any = None,
    n_threads: int = 1,
) -> Image:
    """Resample ``image`` onto the grid of ``reference`` through ``transforms``
    (``antsApplyTransforms``).

    Parameters
    ----------
    image, reference
        Paths to NIfTI files, :class:`~larmorx.Image` objects, nibabel images or
        ``(array, affine)`` pairs. Files are read as ITK reads them (geometry and scaling),
        which is what makes the result match ANTs bit for bit. In-memory images are placed by
        their affine.
    transforms
        Transform files (``.mat``, ``.txt``, ``.h5``, warp ``.nii.gz``) or
        :class:`~larmorx.transforms.ItkTransform` lists, in ``-t`` order: for each point of
        the output grid the first transform is applied first, so ``[warp, affine]`` moves the
        image by the affine, then the warp, as ``-t warp -t affine`` does.
    invert
        One flag per transform (``-t [file,1]``); linear transforms only. Nothing is
        inverted by default. Unlike ANTsPy, larmorx never guesses.
    interpolation
        ``linear``, ``nearest``, ``bspline`` (``order`` 0–5), ``gaussian`` and ``multilabel``
        (``sigma`` in mm, default the input spacing; ``alpha`` in sigmas, default 1 and 4),
        ``genericlabel``, or a windowed sinc: ``lanczos``, ``hamming``, ``cosine``,
        ``welch``, ``blackman``. ANTs' names (``LanczosWindowedSinc``, ...) work too.
    time_series
        Resample each volume of a 4D image (``-e 3``). The default resamples 4D images as
        time series. ``False`` with a 4D image is an error, as with ``-e 0``.
    default_value
        The value of output voxels outside the input (``-f``).
    dtype
        Output type (``-u``). The default is float64, as ANTs computes. Integer types truncate
        toward zero.
    n_threads
        Worker threads (0 = all). The result does not depend on it.
    """
    # The input image.
    if isinstance(image, (str, os.PathLike)):
        data, info = _core.itk_read_image(os.fspath(image), n_threads)
        grid = info["grid"]
        if info["ndim"] < 3:
            data = data.reshape(tuple(grid["size"]), order="F")
        template = None
        if data.ndim == 4:
            from larmorx.io import read_header

            template = read_header(image)
    else:
        img = as_image(image)
        data, grid, template = img.data, _grid_of(img), img.header
        if data.dtype not in (np.float32, np.float64):
            data = data.astype(np.float64)
        if data.ndim < 3:
            data = data.reshape(tuple(grid["size"]), order="F")
    if data.ndim > 4:
        raise ValueError(f"{data.ndim}D images are not supported (scalar and time series are)")
    if data.ndim == 4 and time_series is False:
        raise ValueError("a 4D image needs time_series=True (antsApplyTransforms -e 3)")
    if data.ndim == 3 and time_series:
        data = data[..., np.newaxis]

    # The reference grid.
    if isinstance(reference, (str, os.PathLike)):
        ref_grid = _core.itk_grid(os.fspath(reference))
    else:
        ref_grid = _grid_of(as_image(reference))

    if sigma is not None:
        s = np.broadcast_to(np.asarray(sigma, dtype=np.float64), (3,))
        sigma = tuple(float(v) for v in s)
    values = _core.ants_apply_transforms(
        data,
        grid,
        ref_grid,
        _transform_items(transforms, invert),
        _interpolator(interpolation),
        sigma,
        alpha,
        order,
        float(default_value),
        n_threads,
    )
    out = values if dtype is None else _cast(values, dtype)
    header = template if out.ndim == 4 else None
    return Image(out, _core.grid_ras_affine(ref_grid), header)


def apply_transforms_to_points(
    points: Any,
    transforms: TransformLike | Sequence[TransformLike] = (),
    *,
    invert: Sequence[bool] | None = None,
    coordinates: Literal["lps", "ras"] = "lps",
) -> np.ndarray:
    """Map points through ``transforms`` (``antsApplyTransformsToPoints``).

    ``points`` is an ``(n, 3)`` array (extra columns, such as ANTs' ``t``, are kept
    unchanged). ANTs works in LPS millimetres; with ``coordinates="ras"`` the points are
    given and returned in RAS+. Transforms map points the way ``apply_transforms`` maps output
    voxels to input positions, so to move points from the moving image to the fixed image, give
    the inverse transforms, as with ANTs.
    """
    pts = np.array(points, dtype=np.float64, ndmin=2)
    if pts.ndim != 2 or pts.shape[1] < 3:
        raise ValueError(f"points must have shape (n, 3) or more columns, not {pts.shape}")
    xyz = pts[:, :3].copy()
    flip = np.array([-1.0, -1.0, 1.0])
    if coordinates == "ras":
        xyz *= flip
    elif coordinates != "lps":
        raise ValueError(f"coordinates must be 'lps' or 'ras', not {coordinates!r}")
    mapped = _core.ants_transform_points(
        np.ascontiguousarray(xyz), _transform_items(transforms, invert)
    )
    if coordinates == "ras":
        mapped *= flip
    pts[:, :3] = mapped
    return pts
