# SPDX-License-Identifier: Apache-2.0 AND MIT
"""niworkflows' ``ConcatenateXFMs``: linear transform files combined into one, written as
nitransforms writes it.

Ported from niworkflows 1.15.4 (``interfaces/nitransforms.py``: ``ConcatenateXFMs``,
``concatenate_xfms``; the same in 1.15.2, fMRIPrep 25.2.6's; Apache-2.0, the NiPreps
developers) with nitransforms 25.1.0's (MIT) ``linear.load``, ``TransformChain.asaffine`` and
ITK text writer (``io/itk.py``: ``ITKLinearTransform.from_ras(...).to_string()``). The files are
read as :func:`larmorx.transforms.load_itk_linear` reads them (nitransforms' readers), combined
with numpy's 4×4 products and written with ``"%g"``: six significant digits, the centre of
rotation folded into the translation (``FixedParameters: 0 0 0``).

FreeSurfer's LTA files (``out_fmt="fs"``, ``.lta`` inputs) come with FreeSurfer's part: they
raise ``NotImplementedError`` here. A file holding several transforms (a series, nitransforms'
``LinearTransformsMapping``) is not combined either (``NotImplementedError``): fMRIPrep passes
single affines only.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from larmorx.transforms._linalg import matmul
from larmorx.transforms.chain import Affine, _itk_linear_text, load_itk_linear

__all__ = ["XFM_FMT", "ConcatenateXfmsResult", "concatenate_xfms"]

PathLike = str | os.PathLike[str]

#: niworkflows' ``XFM_FMT``: the format each suffix is read as.
XFM_FMT = {".lta": "fs", ".txt": "itk", ".mat": "itk", ".tfm": "itk"}
_LTA = (
    "FreeSurfer's LTA format (nitransforms' io.lta) is not ported yet: it comes with "
    "FreeSurfer's part"
)


@dataclass(frozen=True)
class ConcatenateXfmsResult:
    """``ConcatenateXFMs``' outputs: the combined affine (RAS+, reference to moving), its file
    (``out_xfm``: ``<out_dir>/out_fwd.tfm``), the inverse's file with ``inverse``
    (``out_inv``: ``out_inv.tfm``) and the text written (``text``, ``inverse_text``). The paths
    are ``None`` without ``out_dir``."""

    affine: np.ndarray
    text: str
    inverse_text: str | None = None
    out_xfm: Path | None = None
    out_inv: Path | None = None


def _load(path: PathLike) -> Affine:
    """``nitransforms.linear.load(path, fmt=XFM_FMT[suffix])``."""
    suffix = Path(path).suffix
    fmt = XFM_FMT[suffix]  # KeyError for another suffix, as niworkflows
    if fmt == "fs":
        raise NotImplementedError(_LTA)
    loaded = load_itk_linear(path)
    if not isinstance(loaded, Affine):
        raise NotImplementedError(
            f"{os.fspath(path)} holds {len(loaded)} transforms: a series of affines is not "
            "combined (nitransforms' LinearTransformsMapping)"
        )
    return loaded


def concatenate_xfms(
    in_xfms: Sequence[PathLike] | PathLike,
    out_fmt: str = "itk",
    inverse: bool = False,
    reference: PathLike | None = None,
    moving: PathLike | None = None,
    *,
    out_dir: PathLike | None = None,
) -> ConcatenateXfmsResult:
    """niworkflows' ``ConcatenateXFMs``: the transforms of ``in_xfms`` (ITK text ``.txt`` /
    ``.tfm`` or MATLAB ``.mat`` files, listed in the order they apply: the first maps the
    reference's points first) combined into one affine (nitransforms' ``asaffine``: each next
    matrix multiplied from the left) and written in ``out_fmt`` (``"itk"``) as
    ``out_fwd.tfm`` (and its inverse, ``np.linalg.inv``'s, as ``out_inv.tfm`` with
    ``inverse``) in ``out_dir``. ``reference`` and ``moving`` only matter to the LTA format;
    they must exist when given (nipype's ``File(exists=True)``)."""
    if out_fmt not in ("itk", "fs"):
        raise ValueError(
            "The 'out_fmt' trait of a _ConcatenateXFMsInputSpec instance must be 'itk' or "
            f"'fs', but a value of {out_fmt!r} {type(out_fmt)} was specified."
        )
    files = [in_xfms] if isinstance(in_xfms, (str, os.PathLike)) else list(in_xfms)
    for name, value in (
        ("reference", reference),
        ("moving", moving),
        *(("in_xfms", f) for f in files),
    ):
        if value is not None and not os.path.isfile(os.fspath(value)):
            raise FileNotFoundError(f"{name}: no such file '{os.fspath(value)}'")
    if out_fmt == "fs":
        raise NotImplementedError(_LTA)
    affines = [_load(f) for f in files]
    if not affines:
        raise IndexError("list index out of range")
    # TransformChain(affines).asaffine(): retval = affines[0]; retval = xfm @ retval
    retval = affines[0]
    for xfm in affines[1:]:
        retval = Affine(matmul(xfm.matrix, retval.matrix))
    matrix = np.asarray(retval.matrix, dtype=np.float64)
    text = _itk_linear_text(matrix)
    inv_text = _itk_linear_text((~retval).matrix) if inverse else None
    out_xfm = out_inv = None
    if out_dir is not None:
        os.makedirs(out_dir, exist_ok=True)
        out_xfm = Path(os.path.abspath(os.fspath(out_dir))) / "out_fwd.tfm"
        out_xfm.write_text(text, encoding="utf-8", newline="\n")
        if inv_text is not None:
            out_inv = out_xfm.with_name("out_inv.tfm")
            out_inv.write_text(inv_text, encoding="utf-8", newline="\n")
    return ConcatenateXfmsResult(matrix, text, inv_text, out_xfm, out_inv)
