"""ITK transform files: ``lx.transforms``.

Transforms are kept in ITK's stored form: a class name, parameters and fixed parameters
(:class:`ItkTransform`). A file holds a list of them. When the first is a
``CompositeTransform`` entry, the following ones are its components.

Formats:

- ITK text (``.txt``, ``.tfm``), MATLAB v4 (``.mat``) and displacement fields stored as NIfTI
  vector images (``.nii``, ``.nii.gz``) are read in Rust.
- HDF5 (``.h5``), the format of fMRIPrep's ``*_xfm.h5`` composites, is read and written with
  h5py, in ITK's layout.

Points are mapped in ITK's physical space (LPS millimetres), as by ANTs.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass

import numpy as np

from larmorx import _core

__all__ = ["ItkTransform", "read", "write"]

PathLike = str | os.PathLike[str]

_H5_SUFFIXES = (".h5", ".hdf5")


@dataclass(frozen=True, eq=False)
class ItkTransform:
    """One transform as ITK stores it.

    ``name`` is the ITK class name (e.g. ``"AffineTransform_double_3_3"``). ``parameters``
    are float32 or float64, as stored; float32 parameters (as float transforms write them)
    are read by ITK into double exactly. ``fixed_parameters`` are float64.
    """

    name: str
    parameters: np.ndarray
    fixed_parameters: np.ndarray

    def __post_init__(self) -> None:
        params = np.asarray(self.parameters)
        if params.dtype not in (np.float32, np.float64):
            params = params.astype(np.float64)
        object.__setattr__(self, "parameters", np.ascontiguousarray(params.ravel()))
        fixed = np.ascontiguousarray(np.asarray(self.fixed_parameters, dtype=np.float64).ravel())
        object.__setattr__(self, "fixed_parameters", fixed)

    def __repr__(self) -> str:
        return (
            f"ItkTransform({self.name!r}, {self.parameters.size} parameters, "
            f"{self.fixed_parameters.size} fixed)"
        )

    @property
    def is_composite(self) -> bool:
        return self.name.startswith("CompositeTransform")

    @property
    def is_displacement_field(self) -> bool:
        return self.name.startswith("DisplacementFieldTransform")

    def _as_tuple(self) -> tuple[str, np.ndarray, np.ndarray]:
        return (self.name, self.parameters, self.fixed_parameters)


def _is_h5(path: PathLike) -> bool:
    return os.fspath(path).lower().endswith(_H5_SUFFIXES)


def read(path: PathLike) -> list[ItkTransform]:
    """Read the transforms stored in an ITK transform file."""
    return [ItkTransform(*parts) for parts in _read_parts(path)]


def write(path: PathLike, transforms: ItkTransform | Sequence[ItkTransform]) -> None:
    """Write transforms in ITK's format for the file's extension.

    ``.h5`` takes any transforms, composites and displacement fields included; ``.txt`` and
    ``.tfm`` take linear transforms; ``.mat`` exactly one linear transform.
    """
    items = [transforms] if isinstance(transforms, ItkTransform) else list(transforms)
    if _is_h5(path):
        _write_h5(path, items)
    else:
        _core.itk_transform_write(os.fspath(path), [t._as_tuple() for t in items])


def _read_parts(path: PathLike) -> list[tuple[str, np.ndarray, np.ndarray]]:
    """The stored ``(name, parameters, fixed)`` tuples; also used by the command line for
    ``.h5`` files."""
    if _is_h5(path):
        return _read_h5(path)
    return _core.itk_transform_read(os.fspath(path))


def _h5py():  # type: ignore[no-untyped-def]
    try:
        import h5py
    except ImportError as e:  # pragma: no cover - h5py is a dependency
        raise ImportError("reading and writing .h5 transforms needs h5py") from e
    return h5py


def _text(value: object) -> str:
    if isinstance(value, np.ndarray):
        value = value.reshape(-1)[0]
    return value.decode() if isinstance(value, bytes) else str(value)


def _read_h5(path: PathLike) -> list[tuple[str, np.ndarray, np.ndarray]]:
    """ITK's HDF5 layout: ``/TransformGroup/<n>/{TransformType, TransformParameters,
    TransformFixedParameters}`` for n = 0, 1, ..."""
    h5py = _h5py()
    out = []
    with h5py.File(os.fspath(path), "r") as f:
        group = f.get("TransformGroup")
        if group is None:
            raise ValueError(f"{os.fspath(path)}: not an ITK transform file (no /TransformGroup)")
        for key in sorted(group, key=int):
            g = group[key]
            name = _text(g["TransformType"][()])
            params = g["TransformParameters"][()] if "TransformParameters" in g else np.zeros(0)
            fixed = (
                g["TransformFixedParameters"][()]
                if "TransformFixedParameters" in g
                else np.zeros(0)
            )
            params = np.asarray(params).ravel()
            if params.dtype not in (np.float32, np.float64):
                params = params.astype(np.float64)
            out.append((name, params, np.asarray(fixed, dtype=np.float64).ravel()))
    if not out:
        raise ValueError(f"{os.fspath(path)}: no transform in file")
    return out


def _write_h5(path: PathLike, transforms: Sequence[ItkTransform]) -> None:
    h5py = _h5py()
    string = h5py.string_dtype("ascii")
    with h5py.File(os.fspath(path), "w") as f:
        f.create_dataset("ITKVersion", data=np.array(["5.4.5"], dtype=object), dtype=string)
        f.create_dataset(
            "HDFVersion", data=np.array([h5py.version.hdf5_version], dtype=object), dtype=string
        )
        f.create_dataset("OSName", data=np.array(["larmorx"], dtype=object), dtype=string)
        f.create_dataset(
            "OSVersion", data=np.array([_core.__version__], dtype=object), dtype=string
        )
        group = f.create_group("TransformGroup")
        for i, t in enumerate(transforms):
            g = group.create_group(str(i))
            g.create_dataset("TransformType", data=np.array([t.name], dtype=object), dtype=string)
            if t.is_composite:
                continue
            g.create_dataset("TransformFixedParameters", data=t.fixed_parameters)
            large = t.parameters.size > 1 << 16
            g.create_dataset(
                "TransformParameters",
                data=t.parameters,
                chunks=(min(t.parameters.size, 1 << 20),) if large else None,
                compression="gzip" if large else None,
            )
