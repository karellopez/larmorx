# SPDX-License-Identifier: Apache-2.0 AND MIT
"""nibabel's own header arithmetic: the affine nibabel reads from a header, and the qform it
writes, for ports of Python programs that read and write NIfTI files through nibabel.

larmorx's reader (:func:`larmorx.io.load`) and writer compute a qform affine and quaternion in
plain float64 (``crates/larmorx-io``), the same on every platform. nibabel 5.4.2 computes them
differently, and ports of programs built on nibabel (sdcflows, fMRIPrep's own steps) must give
its bits:

- **reading** (:func:`nibabel_affine`, :func:`load_nibabel`): ``get_best_affine`` takes the
  sform when ``sform_code > 0``; else the qform when ``qform_code > 0``, which
  ``get_qform`` builds in numpy's ``longdouble`` (``quaternions.fillpositive``'s
  ``MAX_FLOAT``: x87's extended format on x86-64 Linux, emulated here exactly in rational
  arithmetic, so every platform gets Linux's bits): ``w = √(1 - (b² + c² + d²))`` (0 within
  the header's ``quaternion_threshold``), ``quat2mat``, ``R @ diag(pixdim[1:4] · (1, 1,
  qfac))``, rounded to float64 once; else ``get_base_affine`` (``shape_zoom_affine``: the image
  centred on the origin, the x axis flipped, ``-0.0`` translations of the singleton axes
  included);
- **writing** (:func:`header_for_image`, :func:`set_xform`): ``set_qform`` through
  ``numpy.linalg.svd``, ``det`` and ``eigh`` (``larmorx_transform::numpy_linalg``: numpy's
  LAPACK and OpenBLAS's Haswell kernels, operation for operation), ``_core``'s ``nibabel=True``.

numpy's linear algebra changed between OpenBLAS 0.3.30 (LAPACK 3.12.0: numpy 2.3.5, fMRIPrep
25.2.6's environment) and 0.3.34 (LAPACK 3.12.1: numpy 2.5.4): the quaternions of about 2 % of
oblique affines differ in their last float32 bit. larmorx follows the newest (3.12.1); the
validation of the older environment selects 3.12.0 with :func:`use_lapack`.

Ported from nibabel 5.4.2 (MIT, Copyright the NiBabel developers): ``nifti1.py``
(``Nifti1Header.get_best_affine``, ``get_qform``, ``get_qform_quaternion``,
``quaternion_threshold``, ``set_qform``), ``nifti2.py`` (``quaternion_threshold``),
``analyze.py`` (``get_base_affine``), ``affines.py`` (``shape_zoom_affine``),
``quaternions.py`` (``fillpositive``, ``quat2mat``, ``mat2quat``); numpy 2.5.4's
``LONGDOUBLE_dot`` and the ``matmul`` inner loop for ``longdouble`` (BSD-3-Clause).
"""

from __future__ import annotations

import math
import os
from contextvars import ContextVar
from fractions import Fraction
from typing import Any

import numpy as np

from larmorx import _core
from larmorx.io.nibabel_writer import _Extended, _round

__all__ = [
    "header_for_image",
    "load_nibabel",
    "nibabel_affine",
    "set_xform",
    "use_lapack",
]

PathLike = str | os.PathLike[str]

#: The LAPACK release numpy's linear algebra follows in nibabel's ``set_qform`` (see the
#: module's docstring); a context variable, so only the validation of the older environment
#: changes it (:func:`use_lapack`).
_LAPACK: ContextVar[str] = ContextVar("larmorx_nibabel_lapack", default="3.12.1")


def use_lapack(release: str) -> None:
    """Make nibabel's ``set_qform`` arithmetic follow LAPACK ``release`` ("3.12.1", numpy
    2.5.4's OpenBLAS 0.3.34, the default; "3.12.0", numpy 2.3.5's OpenBLAS 0.3.30) in the
    current context. For validation against an older numpy only."""
    if release not in ("3.12.1", "3.12.0"):
        raise ValueError(f"release must be '3.12.1' or '3.12.0', not {release!r}")
    _LAPACK.set(release)


def header_for_image(
    shape: Any, dtype: str, affine: Any, template: dict | None = None, version: int | None = None
) -> dict:
    """``_core.nifti_header_for_image`` with nibabel's ``set_qform`` arithmetic: the header
    nibabel writes for ``Nifti1Image(data, affine, template)``."""
    return _core.nifti_header_for_image(
        list(shape),
        dtype,
        np.ascontiguousarray(affine, dtype=np.float64),
        template,
        version,
        nibabel=True,
        lapack=_LAPACK.get(),
    )


def set_xform(fields: dict, which: str, affine: Any, code: int) -> dict:
    """nibabel's ``header.set_sform(affine, code)`` / ``set_qform(affine, code)`` on header
    fields, with nibabel's arithmetic."""
    return _core.nifti_set_xform(
        fields,
        which,
        np.ascontiguousarray(affine, dtype=np.float64),
        int(code),
        nibabel=True,
        lapack=_LAPACK.get(),
    )


# ------------------------------------------------------------------------------------------------
# reading


def _ext(v: Any) -> _Extended:
    return _Extended.of(v)


def _ext_sqrt(x: _Extended) -> _Extended:
    """``np.sqrt`` of a positive ``longdouble``: x87's ``fsqrt``, the root rounded to a 64-bit
    significand."""
    q = x.q
    if q == 0:
        return x
    if q < 0:
        return _Extended(Fraction(0))  # not reached: fillpositive tests w2 < 0 first
    e = q.numerator.bit_length() - q.denominator.bit_length()
    # 2^t · √q with 66 to 67 bits; the integer root, and whether anything is left below it.
    t = 66 - e // 2
    scaled = q * Fraction(4) ** t
    n = scaled.numerator // scaled.denominator
    r = math.isqrt(n)
    exact = Fraction(r * r) == scaled
    value = Fraction(2 * r + (0 if exact else 1), 2) / Fraction(2) ** t
    return _Extended(_round(value, 64, None, None))


def _ext_to_float64(x: _Extended) -> float:
    """``float64(longdouble)``: one rounding to binary64."""
    if x.q == 0:
        return -0.0 if x.negzero else 0.0
    r = _round(x.q, 53, -1022, 1023)
    if r is None:
        return math.inf if x.q > 0 else -math.inf
    if r == 0:
        return -0.0 if x.q < 0 else 0.0
    return float(r)


def _get_qform(h: Any) -> np.ndarray:
    """nibabel's ``get_qform`` of a header, in x87 extended precision (module docstring)."""
    nifti2 = int(h.version) == 2
    thresh = np.finfo(np.float64 if nifti2 else np.float32).eps * 3
    # fillpositive: xyz in longdouble, w2 = 1.0 - xyz @ xyz (matmul's longdouble loop: from
    # 0, each product and sum rounded).
    xyz = [_ext(h.quatern_b), _ext(h.quatern_c), _ext(h.quatern_d)]
    dot = _Extended(Fraction(0))
    for v in xyz:
        dot = dot + v * v
    w2 = _ext(1.0) - dot
    if abs(w2.q) < Fraction(float(thresh)):
        w = _Extended(Fraction(0))
    elif w2.q < 0:
        raise ValueError(f"w2 should be positive, but is {float(w2.q):e}")
    else:
        w = _ext_sqrt(w2)
    x, y, z = xyz
    # quat2mat in longdouble.
    nq = w * w + x * x + y * y + z * z
    if nq.q < Fraction(float(np.finfo(float).eps)):
        r = [[_ext(1.0 if i == j else 0.0) for j in range(3)] for i in range(3)]
    else:
        s = _ext(2.0) / nq
        X, Y, Z = x * s, y * s, z * s
        wX, wY, wZ = w * X, w * Y, w * Z
        xX, xY, xZ = x * X, x * Y, x * Z
        yY, yZ, zZ = y * Y, y * Z, z * Z
        one = _ext(1.0)
        r = [
            [one - (yY + zZ), xY - wZ, xZ + wY],
            [xY + wZ, one - (xX + zZ), yZ - wX],
            [xZ - wY, yZ + wX, one - (xX + yY)],
        ]
    vox = [float(v) for v in h.pixdim[1:4]]
    if any(v < 0 for v in vox):
        raise ValueError("pixdims[1,2,3] should be positive")
    qfac = float(h.pixdim[0])
    if qfac not in (-1.0, 1.0):
        raise ValueError("qfac (pixdim[0]) should be 1 or -1")
    vox[2] = vox[2] * qfac  # float32 (or float64) times ±1: exact, the sign of zero included
    sdiag = [_ext(v) for v in vox]
    out = np.eye(4)
    for i in range(3):
        for j in range(3):
            # np.dot(R, diag(vox)) through LONGDOUBLE_dot: tmp = 0; tmp += R[i][k] · S[k][j].
            tmp = _Extended(Fraction(0))
            for k in range(3):
                skj = sdiag[j] if k == j else _ext(0.0)
                tmp = tmp + r[i][k] * skj
            out[i, j] = _ext_to_float64(tmp)
    out[:3, 3] = [float(h.qoffset_x), float(h.qoffset_y), float(h.qoffset_z)]
    return out


def _base_affine(h: Any) -> np.ndarray:
    """nibabel's ``get_base_affine`` (``shape_zoom_affine(dim, pixdim, x_flip=True)``)."""
    ndim = int(h.dim[0])
    shape = np.asarray(h.dim[1 : ndim + 1], dtype=np.int64)
    dt = np.float64 if int(h.version) == 2 else np.float32
    zooms = np.array(h.pixdim[1 : ndim + 1], dtype=dt)
    if ndim >= 3:
        shape = shape[:3]
        zooms = zooms[:3]
    else:
        full_shape = np.ones((3,))
        full_zooms = np.ones((3,))
        full_shape[:ndim] = shape[:]
        full_zooms[:ndim] = zooms[:]
        shape = full_shape
        zooms = full_zooms
    zooms[0] *= -1
    origin = (shape - 1) / 2.0
    aff = np.eye(4)
    aff[:3, :3] = np.diag(zooms)
    aff[:3, -1] = -origin * zooms
    return aff


def nibabel_affine(header: Any) -> np.ndarray:
    """The affine nibabel 5.4.2 gives an image of this header (``img.affine`` after
    ``nb.load``: ``header.get_best_affine()``), bit for bit (module docstring)."""
    if int(header.sform_code) > 0:
        return np.array(
            [list(header.srow_x), list(header.srow_y), list(header.srow_z), [0.0, 0.0, 0.0, 1.0]],
            dtype=np.float64,
        )
    if int(header.qform_code) > 0:
        return _get_qform(header)
    return _base_affine(header)


def load_nibabel(path: PathLike, *, dtype: Any = None, scaled: bool = True, n_threads: int = 1):
    """:func:`larmorx.io.load` with the affine nibabel reads (:func:`nibabel_affine`): the
    image ``nb.load(path)`` gives, for ports of Python programs that read through nibabel."""
    from larmorx.io.nifti import load

    img = load(path, dtype=dtype, scaled=scaled, n_threads=n_threads)
    affine = nibabel_affine(img.header)
    if affine.tobytes() == np.asarray(img.affine, dtype=np.float64).tobytes():
        return img
    return img.replace(affine=affine)
