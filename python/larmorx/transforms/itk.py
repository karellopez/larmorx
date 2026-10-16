# SPDX-License-Identifier: Apache-2.0
"""ITK transform files (``lx.transforms.read`` / ``write``).

Transforms are kept in ITK's stored form: a class name, parameters and fixed parameters
(:class:`ItkTransform`). A file holds a list of them. When the first is a
``CompositeTransform`` entry, the following ones are its components.

Formats, all read in Rust as ITK v5.4.5 reads them:

- ITK text (``.txt``, ``.tfm``) and MATLAB v4 (``.mat``);
- HDF5 (``.h5``), the format of fMRIPrep's ``*_xfm.h5`` composites (any HDF5 file with a
  ``/TransformGroup`` is read as one, whatever its name, as ITK does);
- displacement fields stored as NIfTI vector images (``.nii``, ``.nii.gz``).

Points are mapped in ITK's physical space (LPS millimetres), as by ANTs.
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass

import numpy as np

from larmorx import _core
from larmorx._defects import UpstreamDefectsName, check, warn

__all__ = ["Hdf5WriteOptions", "ItkTransform", "read", "write"]

PathLike = str | os.PathLike[str]

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


def read(path: PathLike, *, upstream_defects: UpstreamDefectsName = "fix") -> list[ItkTransform]:
    """Read the transforms stored in an ITK transform file, in the file's order (a composite
    first, then its components).

    ``upstream_defects="reproduce"`` reads as ITK does where it is defective
    (``docs/api/transforms-hdf5.md``): it refuses an HDF5 ``TransformType`` stored as a
    fixed-length string (ITK-12) and reads parameters wider than 8 bytes through float
    (ITK-14). A :class:`~larmorx.UpstreamDefectWarning` says when that changes the result."""
    return [ItkTransform(*parts) for parts in _read_parts(path, upstream_defects=upstream_defects)]


@dataclass(frozen=True)
class Hdf5WriteOptions:
    """How :func:`write` stores transforms in an HDF5 file (``docs/api/transforms-hdf5.md``).

    ``compression``: store the parameters in chunks of at most 1 Mi values compressed by
    deflate at level 5, as ANTs always asks ITK to; ``False`` stores them contiguous, ITK's
    own default. ``time``: the time every HDF5 object header records, in seconds since
    1970-01-01 UTC (ITK records the time of writing; 0 makes the same transforms give the same
    bytes). The four strings are the file's ``/ITKVersion``, ``/HDFVersion``, ``/OSName`` and
    ``/OSVersion`` (ITK writes its version, its HDF5's, and the system's name and release).
    """

    compression: bool = True
    time: int = 0
    itk_version: str = "5.4.5"
    hdf_version: str = "HDF5 library version: 1.12.1"
    os_name: str = "larmorx"
    os_version: str = _core.__version__


def write(
    path: PathLike,
    transforms: ItkTransform | Sequence[ItkTransform],
    *,
    hdf5: Hdf5WriteOptions | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
) -> None:
    """Write transforms in ITK's format for the file's extension, as ITK v5.4.5 writes them.

    HDF5 (``.h5``, ``.hdf5``, and ITK's other HDF5 extensions ``.hdf``, ``.h4``, ``.hdf4``,
    ``.he4``, ``.he5``, ``.hd5``, compared case for case as ITK does) takes any transforms,
    composites and displacement fields included, stored as :class:`Hdf5WriteOptions` says:
    the layout is HDF5 1.12.1's for ITK's calls, byte for byte apart from the compressed
    chunks. ``.txt`` and ``.tfm`` take linear transforms; ``.mat`` exactly one linear
    transform. A composite may only come first: the transforms after it are its components.

    ``upstream_defects="reproduce"`` fails where ITK fails: a transform without parameters
    cannot be written with compression (ITK-15; larmorx stores its empty parameters
    uncompressed, with a :class:`~larmorx.UpstreamDefectWarning`), and the file ITK leaves
    behind is written before the error is raised.
    """
    check(upstream_defects)
    items = [transforms] if isinstance(transforms, ItkTransform) else list(transforms)
    o = hdf5 if hdf5 is not None else Hdf5WriteOptions()
    notices = _core.itk_transform_write(
        os.fspath(path),
        [t._as_tuple() for t in items],
        compression=o.compression,
        time=o.time,
        itk_version=o.itk_version,
        hdf_version=o.hdf_version,
        os_name=o.os_name,
        os_version=o.os_version,
        upstream_defects=upstream_defects,
    )
    warn(notices, stacklevel=2)


def _read_parts(
    path: PathLike, precision: str = "float64", upstream_defects: UpstreamDefectsName = "fix"
) -> list[tuple[str, np.ndarray, np.ndarray]]:
    """The stored ``(name, parameters, fixed)`` tuples. With ``precision="float32"``, text
    files are read as ITK's float reader reads them (each number rounded once to float); the
    other formats are read as stored."""
    check(upstream_defects)
    parts, notices = _core.itk_transform_read(os.fspath(path), precision, upstream_defects)
    warn(notices, stacklevel=3)
    return parts
