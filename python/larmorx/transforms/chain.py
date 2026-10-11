# SPDX-License-Identifier: Apache-2.0 AND MIT
"""Transform chains as fMRIPrep loads them: ``lx.transforms.load_transforms``.

A replica of the parts of nitransforms 25.1.0 (MIT) that fMRIPrep's one-shot resampler uses,
and of fMRIPrep's ``load_transforms`` (``fmriprep/utils/transforms.py``, Apache-2.0):

- :class:`Affine` (nitransforms ``Affine``), :class:`AffineSeries`
  (``LinearTransformsMapping``, e.g. head-motion transforms), :class:`DenseField`
  (``DenseFieldTransform``) and :class:`TransformChain`;
- ITK text (``.txt``, ``.tfm``), MATLAB (``.mat``) and composite HDF5 (``.h5``) files.

Transforms map **reference (target) RAS+ points to moving points** and a chain applies its
steps in order. The 4×4 matrices are computed with nitransforms' expressions (ITK text values
read as float32, centres of rotation as float32, ``LPS @ ... @ LPS``); their products and
inverses run in Rust in the operation order of numpy's OpenBLAS Haswell kernels
(:mod:`larmorx.transforms._linalg`), so they carry nitransforms' bits on an x86-64 machine with
FMA and the same bits on every platform. Mapping points runs in Rust
(``larmorx_transform::nitransforms``).

nitransforms: Copyright (c) 2021 The NiPy developers. MIT License; the full notice is in the
NOTICE file of larmorx.
"""

from __future__ import annotations

import os
from collections.abc import Iterator, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Union

import numpy as np

from larmorx import _core
from larmorx.transforms._linalg import inv, matmul

__all__ = [
    "Affine",
    "AffineSeries",
    "DenseField",
    "Transform",
    "TransformChain",
    "load_itk_composite",
    "load_itk_linear",
    "load_transforms",
]

PathLike = str | os.PathLike[str]

#: nitransforms' ``LPS`` matrix (integer, as in ``nitransforms.io.itk``).
LPS = np.diag([-1, -1, 1, 1])


class TransformFileError(ValueError):
    """A transform file cannot be read (nitransforms' ``TransformFileError``)."""


def _from_matvec(matrix: Any, vector: Any = None) -> np.ndarray:
    """``nibabel.affines.from_matvec``."""
    matrix = np.asarray(matrix)
    nin, nout = matrix.shape
    t = np.zeros((nin + 1, nout + 1), matrix.dtype)
    t[0:nin, 0:nout] = matrix
    t[nin, nout] = 1.0
    if vector is not None:
        t[0:nin, nout] = vector
    return t


def _readonly(a: np.ndarray) -> np.ndarray:
    a.setflags(write=False)
    return a


@dataclass(frozen=True, eq=False)
class Affine:
    """A linear transform: a 4×4 RAS+ matrix mapping reference points to moving points.

    As nitransforms' ``Affine``: the last row must be close to ``(0, 0, 0, 1)`` and is then
    set to it exactly. ``map`` rounds points to float32 before multiplying, as nitransforms
    does.
    """

    matrix: np.ndarray

    def __post_init__(self) -> None:
        m = np.array(self.matrix)
        if m.ndim != 2:
            raise TypeError("Affine should be 2D.")
        if m.shape[0] != m.shape[1]:
            raise TypeError("Matrix is not square.")
        if not np.allclose(m[3, :], (0, 0, 0, 1)):
            raise ValueError(
                f"The last row of a homogeneus matrix should be (0, 0, 0, 1), got {m[3, :]}."
            )
        m[3, :] = (0, 0, 0, 1)
        object.__setattr__(self, "matrix", _readonly(m))

    def __repr__(self) -> str:
        return f"Affine({np.array2string(self.matrix, precision=4)})"

    @property
    def inverse(self) -> np.ndarray:
        """``np.linalg.inv(matrix)``, as nitransforms computes it (in Rust, with the bits of
        numpy's OpenBLAS ``dgesv`` on x86-64 with FMA)."""
        return inv(self.matrix)

    def __invert__(self) -> Affine:
        return Affine(self.inverse)

    def __add__(self, other: Transform) -> TransformChain:
        return TransformChain((self, other))

    def map(self, points: Any, *, n_threads: int = 1) -> np.ndarray:
        """Map ``(n, 3)`` RAS+ points (``Affine.map``)."""
        return TransformChain((self,)).map(points, n_threads=n_threads)


@dataclass(frozen=True, eq=False)
class AffineSeries:
    """One affine per volume (nitransforms' ``LinearTransformsMapping``), e.g. head motion.

    ``matrices`` has shape ``(t, 4, 4)``; each is normalised as :class:`Affine` does. In a
    resampling chain the series must come last (fMRIPrep applies it in voxel space).
    """

    matrices: np.ndarray

    def __post_init__(self) -> None:
        stack = np.stack([Affine(m).matrix for m in np.asarray(self.matrices)], axis=0)
        object.__setattr__(self, "matrices", _readonly(stack))

    def __repr__(self) -> str:
        return f"AffineSeries({len(self)} affines)"

    def __len__(self) -> int:
        return len(self.matrices)

    def __getitem__(self, i: int) -> Affine:
        return Affine(self.matrices[i])

    def __iter__(self) -> Iterator[Affine]:
        return (Affine(m) for m in self.matrices)

    def __invert__(self) -> AffineSeries:
        return AffineSeries(inv(self.matrices))

    def __add__(self, other: Transform) -> TransformChain:
        return TransformChain((self, other))


@dataclass(frozen=True, eq=False)
class DenseField:
    """A displacement field (nitransforms' ``DenseFieldTransform`` with ``is_deltas=True``).

    ``deltas`` has shape ``(x, y, z, 3)``: the RAS+ displacement (mm) at each node of the grid
    placed by ``affine``. A point is mapped to itself plus the displacement, interpolated with
    SciPy's cubic B-splines (looked up when every point lies on the grid); points outside the
    field are not moved.
    """

    deltas: np.ndarray
    affine: np.ndarray

    def __post_init__(self) -> None:
        deltas = np.asarray(self.deltas, dtype=np.float64)
        if deltas.ndim != 4 or deltas.shape[-1] != 3:
            raise ValueError(f"deltas must have shape (x, y, z, 3), not {deltas.shape}")
        affine = np.array(self.affine, dtype=np.float64)
        if affine.shape != (4, 4):
            raise ValueError("the field affine must be 4x4")
        object.__setattr__(self, "deltas", _readonly(deltas))
        object.__setattr__(self, "affine", _readonly(affine))

    def __repr__(self) -> str:
        return f"DenseField(shape={self.deltas.shape[:3]})"

    @property
    def inverse(self) -> np.ndarray:
        """RAS+ to grid indices: ``np.linalg.inv(affine)`` (``ImageGrid.inverse``)."""
        return inv(self.affine)

    def __add__(self, other: Transform) -> TransformChain:
        return TransformChain((self, other))

    def map(self, points: Any, *, n_threads: int = 1) -> np.ndarray:
        """Map ``(n, 3)`` RAS+ points (``DenseFieldTransform.map``)."""
        return TransformChain((self,)).map(points, n_threads=n_threads)


@dataclass(frozen=True, eq=False)
class TransformChain:
    """Transforms applied in order (nitransforms' ``TransformChain``): the first maps reference
    points, the next maps its result, and so on. Chains may nest."""

    transforms: tuple[Transform, ...]

    def __post_init__(self) -> None:
        object.__setattr__(self, "transforms", tuple(self.transforms))

    def __repr__(self) -> str:
        inner = ", ".join(type(t).__name__ for t in self.transforms)
        return f"TransformChain([{inner}])"

    def __len__(self) -> int:
        return len(self.transforms)

    def __getitem__(self, i: Any) -> Any:
        return self.transforms[i]

    def __iter__(self) -> Iterator[Transform]:
        return iter(self.transforms)

    def __add__(self, other: Transform) -> TransformChain:
        """``chain += xfm``: append (a chain's transforms are appended one by one)."""
        extra = other.transforms if isinstance(other, TransformChain) else (other,)
        return TransformChain(self.transforms + tuple(extra))

    def steps(self) -> list[Affine | AffineSeries | DenseField]:
        """The transforms with nested chains flattened (mapping is the same)."""
        out: list[Affine | AffineSeries | DenseField] = []
        for t in self.transforms:
            out.extend(t.steps() if isinstance(t, TransformChain) else [t])
        return out

    def map(self, points: Any, *, n_threads: int = 1) -> np.ndarray:
        """Map ``(n, 3)`` RAS+ points through every step (``TransformChain.map``)."""
        pts = np.ascontiguousarray(np.asarray(points, dtype=np.float64).reshape(-1, 3))
        return _core.nitransforms_map(pts, _rust_steps(self.steps()), n_threads=int(n_threads))


Transform = Union[Affine, AffineSeries, DenseField, TransformChain]  # noqa: UP007


def _rust_steps(steps: Sequence[Affine | AffineSeries | DenseField]) -> list[tuple]:
    out: list[tuple] = []
    for s in steps:
        if isinstance(s, Affine):
            out.append(("affine", np.ascontiguousarray(s.matrix, dtype=np.float64)))
        elif isinstance(s, DenseField):
            out.append(("field", s.deltas, s.affine, np.ascontiguousarray(s.inverse)))
        else:
            raise ValueError(
                "a series of affines (one per volume) can only be the last transform; "
                "it is applied in voxel space by resample_series"
            )
    return out


# ------------------------------------------------------------------------------------------------
# ITK files, as nitransforms reads them


@dataclass
class _ItkLinear:
    """``nitransforms.io.itk.ITKLinearTransform``: parameters (float64 4×4) and the centre of
    rotation (float32)."""

    parameters: np.ndarray
    offset: np.ndarray

    def to_ras(self) -> np.ndarray:
        matrix = self.parameters
        offset = self.offset
        c_neg = _from_matvec(np.eye(3), offset * -1.0)
        c_pos = _from_matvec(np.eye(3), offset)
        # LPS.dot(c_pos.dot(matrix.dot(c_neg.dot(LPS)))), innermost product first
        return matmul(LPS, matmul(c_pos, matmul(matrix, matmul(c_neg, LPS))))


_OFFSET_DTYPE = np.dtype([("offset", "f4", 3)])["offset"]


def _parse_itk_text_one(string: str) -> _ItkLinear:
    """``ITKLinearTransform.from_string``: the parameters are parsed as float32."""
    lines = [line.strip() for line in string.splitlines() if line.strip()]
    if not lines or not lines[0].startswith("#"):
        raise TransformFileError("not an ITK transform")
    if len(lines) > 1 and lines[1][0] == "#":
        lines = lines[1:]  # Drop banner with version
    if len(lines) < 4:
        raise TransformFileError("incomplete ITK transform")
    parameters = np.eye(4, dtype="f4")
    offset = np.genfromtxt([lines[3].split(":")[-1].encode()], dtype=_OFFSET_DTYPE)
    vals = np.genfromtxt([lines[2].split(":")[-1].encode()], dtype="f4")
    parameters[:3, :3] = vals[:-3].reshape((3, 3))
    parameters[:3, 3] = vals[-3:]
    return _ItkLinear(parameters.astype("f8"), np.asarray(offset, dtype="f4").reshape(3))


def _parse_itk_text(text: str) -> list[_ItkLinear]:
    """``ITKLinearTransformArray.from_string``."""
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if not lines or not lines[0].startswith("#") or "Insight Transform File V1.0" not in lines[0]:
        raise TransformFileError("Unknown Insight Transform File format.")
    string = "\n".join(lines[1:])
    return [_parse_itk_text_one(f"#{xfm}") for xfm in string.split("#")[1:]]


def _itk_mat(path: PathLike) -> _ItkLinear:
    """``ITKLinearTransform.from_matlab_dict`` (via larmorx's MATLAB reader)."""
    from larmorx.transforms.itk import read

    items = read(path)
    if len(items) != 1 or items[0].name not in (
        "AffineTransform_double_3_3",
        "AffineTransform_float_3_3",
    ):
        raise NotImplementedError("Unsupported transform type")
    params = items[0].parameters
    parameters = np.eye(4, dtype=params.dtype)
    parameters[:3, :3] = params[:-3].reshape((3, 3))
    parameters[:3, 3] = params[-3:].flatten()
    return _ItkLinear(parameters.astype("f8"), items[0].fixed_parameters.astype("f4"))


def load_itk_linear(path: PathLike) -> Affine | AffineSeries:
    """An ITK linear transform file as ``nitransforms.linear.load`` reads it: text files
    (``.txt``, ``.tfm``, one or more transforms) and MATLAB ``.mat`` files (one). A file with
    one transform gives an :class:`Affine`, with several an :class:`AffineSeries`."""
    p = Path(path)
    if not p.exists():
        raise FileNotFoundError(f"[Errno 2] No such file or directory: '{path}'")
    if p.suffix == ".mat":
        return Affine(_itk_mat(p).to_ras())
    text = p.read_text(encoding="utf-8")
    xforms = _parse_itk_text(text)
    if not xforms:
        raise TransformFileError(f"no transform in {path}")
    series = AffineSeries(np.stack([x.to_ras() for x in xforms]))
    return series[0] if len(series) == 1 else series


def load_itk_composite(path: PathLike) -> TransformChain:
    """An ITK composite ``.h5`` file (affines and displacement fields) as
    ``nitransforms.manip.load(path, fmt="h5")`` reads it: the chain lists the transforms in
    reverse file order, so that the last one ITK applies first comes first."""
    import h5py

    retval: list[Transform] = []
    with h5py.File(os.fspath(path), "r") as f:
        h5group = f["TransformGroup"]
        typo = "Transform"
        try:
            h5group["1"][f"{typo}Parameters"]
        except KeyError:
            typo = "Tranform"
        for xfm in list(h5group.values())[1:]:
            kind = xfm["TransformType"][0]
            if kind.startswith(b"AffineTransform"):
                params = np.asanyarray(xfm[f"{typo}Parameters"])
                lin = _ItkLinear(
                    _from_matvec(params[:-3].reshape(3, 3), params[-3:]).astype("f8"),
                    np.asanyarray(xfm[f"{typo}FixedParameters"]).astype("f4"),
                )
                retval.insert(0, Affine(lin.to_ras()))
                continue
            if kind.startswith(b"DisplacementFieldTransform"):
                fixed = xfm[f"{typo}FixedParameters"]
                shape = fixed[:3]
                offset = fixed[3:6]
                zooms = fixed[6:9]
                directions = np.reshape(fixed[9:], (3, 3))
                affine = _from_matvec(directions * zooms, offset)
                field = np.moveaxis(
                    np.reshape(xfm[f"{typo}Parameters"], (3, *shape.astype(int)), order="F"),
                    0,
                    -1,
                )
                field[..., (0, 1)] *= -1.0
                retval.insert(
                    0,
                    DenseField(np.squeeze(field.astype("float")), matmul(LPS, affine)),
                )
                continue
            raise TransformFileError(f"Unsupported transform type {kind}")
    return TransformChain(tuple(retval))


def load_transforms(paths: Sequence[PathLike], inverse: bool | Sequence[bool] = False) -> Transform:
    """fMRIPrep's ``load_transforms``: transform files listed from the moving image to the
    reference (fMRIPrep's order: head motion first), loaded into one chain in nitransforms'
    order (the last file first). ``.h5`` files are composites; anything else is read as an
    ITK linear transform. ``inverse`` inverts files (one flag, or one per file). No files
    gives the identity."""
    paths = list(paths)
    inv = [bool(inverse)] if isinstance(inverse, bool) else [bool(i) for i in inverse]
    if len(inv) == 1:
        inv *= len(paths)
    elif len(inv) != len(paths):
        raise ValueError("Mismatched number of transforms and inverses")
    chain: Transform | None = None
    for path, flag in zip(paths[::-1], inv[::-1], strict=False):
        p = Path(path)
        xfm: Transform = load_itk_composite(p) if p.suffix == ".h5" else load_itk_linear(p)
        if flag:
            if isinstance(xfm, TransformChain):
                raise TypeError("bad operand type for unary ~: 'TransformChain'")
            xfm = ~xfm
        chain = xfm if chain is None else chain + xfm
    return Affine(np.eye(4)) if chain is None else chain
