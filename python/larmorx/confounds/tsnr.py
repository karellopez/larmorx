# SPDX-License-Identifier: Apache-2.0
"""nipype's ``TSNR``: the temporal signal-to-noise ratio of a series, with its mean and standard
deviation maps and, optionally, the series with polynomial trends removed.

Ported from nipype 1.12.0 (Apache-2.0, the nipype developers; every environment's):
``algorithms/confounds.py`` (``TSNR``, ``regress_poly``), with numpy's arithmetic under it, bit
for bit: the files read with nibabel's ``get_fdata(dtype=np.float32)``, each reshaped to four
axes and concatenated along the fourth (numpy's own ``reshape`` and ``concatenate``: a 4-D file
stays Fortran-ordered), ``np.nan_to_num``, :func:`larmorx.confounds.regress_poly` with
``remove_mean=False`` (float64 from then on, C-ordered), the mean and standard deviation over
time (numpy ``numpy_version``'s ``mean``/``std(axis=3)``: in index order along the series'
outermost axis, pairwise along a contiguous one), the tSNR ``mean / std`` where the deviation is
above ``1e-3`` (in the data's type) and 0 elsewhere; every map written as nibabel writes
``Nifti1Image(data, affine, header)`` with the first file's header (:func:`larmorx.io.save_nibabel`:
its data type, nibabel's scaling).

Corrected by default (``upstream_defects="reproduce"`` gives nipype's files; the correction names
its ID in a :class:`larmorx.UpstreamDefectWarning`):

- PIPE-51: the maps of a series stored as integers are written in that integer type, quantized
  by nibabel's scaling (256 levels for ``uint8``, 65,536 for ``int16``): nipype's check that
  switches the header to float32 for integer data (``data.dtype.kind == "i"``) never fires, as the
  data are float32 by then. larmorx writes them as float32.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

import numpy as np

from larmorx import _core, _defects
from larmorx._defects import UpstreamDefectsName

__all__ = ["TsnrResult", "tsnr"]

NUMPY_NEWEST = "2.5.4"


@dataclass(frozen=True)
class TsnrResult:
    """``TSNR``'s outputs: the files written (``detrended_file`` only with ``regress_poly``)."""

    tsnr_file: str
    mean_file: str
    stddev_file: str
    detrended_file: str | None


def _paths(in_file: Any) -> list[str]:
    """``InputMultiPath(File(exists=True))``: one path or a list of them, each an existing file."""
    files = [in_file] if isinstance(in_file, (str, os.PathLike)) else list(in_file)
    if not files:
        raise ValueError("TSNR requires a value for input 'in_file'.")
    out = []
    for f in files:
        p = os.fspath(f)
        if not os.path.isfile(p):
            raise ValueError(
                "Each element of the 'in_file' trait of a TSNRInputSpec instance must be a "
                "pathlike object or string representing an existing file, but a value of "
                f"{p!r} {type(f)} was specified."
            )
        out.append(p)
    return out


def _reduce(a: np.ndarray, op: str, axis: int, numpy_version: str) -> np.ndarray:
    """``np.mean``/``np.std(a, axis=axis)`` as numpy ``numpy_version`` sums ``a``'s memory: a
    dense array in an axis order other than C's and Fortran's (``np.concatenate``'s choice for
    volumes reshaped to four axes, whose new axis' stride ties with the third's) is reduced as
    its C-ordered transpose, which numpy's iterator sums the same way."""
    if a.flags.c_contiguous or a.flags.f_contiguous:
        return np.asarray(_core.numpy_reduce(a, op, axis, numpy_version))
    perm = sorted(range(a.ndim), key=lambda k: -abs(a.strides[k]))
    view = a.transpose(perm)
    if not view.flags.c_contiguous:
        view = np.ascontiguousarray(view)  # not reached: nipype's arrays are dense
    out = np.asarray(_core.numpy_reduce(view, op, perm.index(axis), numpy_version))
    rest = [k for k in perm if k != axis]
    return out.transpose([rest.index(k) for k in range(a.ndim) if k != axis])


def tsnr(
    in_file: str | os.PathLike[str] | Sequence[str | os.PathLike[str]],
    regress_poly: int | None = None,
    *,
    tsnr_file: str = "tsnr.nii.gz",
    mean_file: str = "mean.nii.gz",
    stddev_file: str = "stdev.nii.gz",
    detrended_file: str = "detrend.nii.gz",
    out_dir: str | os.PathLike[str] = ".",
    numpy_version: str = NUMPY_NEWEST,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
) -> TsnrResult:
    """nipype's ``TSNR`` interface: the tSNR (``mean / std`` over time, 0 where the deviation is
    ``1e-3`` or less), mean and standard deviation maps of ``in_file`` (a 4-D file, or several
    3-D or 4-D files concatenated in time), after removing the Legendre polynomials of degree 1 to
    ``regress_poly`` (an integer of at least 1; the mean is kept) when it is given, in which case
    the detrended series is written too. The files are written in ``out_dir`` (nipype's node
    directory) under the names given. PIPE-51: see the module."""
    from larmorx.image import Image
    from larmorx.io.nibabel_header import load_nibabel
    from larmorx.io.nibabel_writer import save_nibabel

    _defects.check(upstream_defects)
    files = _paths(in_file)
    # traits' Range(low=1): an integer (True is 1) of at least 1.
    if regress_poly is not None and (
        not isinstance(regress_poly, (int, np.integer)) or regress_poly < 1
    ):
        raise ValueError(
            "The 'regress_poly' trait of a TSNRInputSpec instance must be an integer >= 1, but a "
            f"value of {regress_poly!r} {type(regress_poly)} was specified."
        )
    out_dir = os.fspath(out_dir)
    os.makedirs(out_dir, exist_ok=True)

    first = load_nibabel(files[0], dtype=np.float32, n_threads=n_threads)
    header, affine = first.header, np.asarray(first.affine, dtype=np.float64)
    arrays = [np.asarray(first.data)]
    del first
    for f in files[1:]:
        arrays.append(np.asarray(load_nibabel(f, dtype=np.float32, n_threads=n_threads).data))
    arrays = [a.reshape((*a.shape[:3], -1)) for a in arrays]
    # A single 4-D array is what np.concatenate would copy (the same layout and values).
    data = arrays[0] if len(arrays) == 1 else np.concatenate(arrays, axis=3)
    if len(arrays) == 1 and data.ndim != 4:
        data = np.concatenate(arrays, axis=3)  # numpy's AxisError for fewer than four axes
    del arrays
    data = np.nan_to_num(data, copy=not data.flags.writeable)

    stored = header.data_dtype
    dtype = None
    if stored is not None and stored.kind in "iu":
        if upstream_defects == "fix":
            dtype = np.float32
        _defects.warn(
            [
                (
                    "PIPE-51",
                    "nipype TSNR",
                    upstream_defects,
                    f"the series is stored as {stored.name}: "
                    + (
                        "the maps are written as float32"
                        if upstream_defects == "fix"
                        else f"the maps are written as {stored.name}, quantized by nibabel's "
                        "scaling, as nipype writes them"
                    ),
                )
            ]
        )

    def write(arr: np.ndarray, name: str) -> str:
        path = os.path.abspath(os.path.join(out_dir, name))
        save_nibabel(Image(arr, affine, header), path, dtype=dtype, version=1, n_threads=n_threads)
        return path

    detrended_path = None
    if regress_poly is not None:
        shape = data.shape
        flat = np.ascontiguousarray(data.reshape((-1, shape[-1])))
        del data
        # The basis (regress_poly's second value, PIPE-52's) is not used: no notice.
        res, _, _ = _core.nipype_regress_poly(
            int(regress_poly), flat, False, str(numpy_version), "reproduce", n_threads
        )
        del flat
        data = res.reshape(shape)
        detrended_path = write(data, detrended_file)

    mean = _reduce(data, "mean", 3, str(numpy_version))
    std = _reduce(data, "std", 3, str(numpy_version))
    del data
    snr = np.zeros_like(mean)
    nonzero = std > 1.0e-3
    snr[nonzero] = mean[nonzero] / std[nonzero]
    return TsnrResult(
        tsnr_file=write(snr, tsnr_file),
        mean_file=write(mean, mean_file),
        stddev_file=write(std, stddev_file),
        detrended_file=detrended_path,
    )
