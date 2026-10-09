"""NIfTI-1/2 reading and writing (``.nii``, ``.nii.gz``, ``.hdr``/``.img`` pairs).

The format is implemented in Rust (crate ``larmorx-io``) with nibabel's semantics: the same
header fixes, affine and scaling rules when reading, and the header nibabel would write when
saving. Differences from nibabel are listed in ``docs/api/io.md``.
"""

from __future__ import annotations

import dataclasses
import logging
import os
from dataclasses import dataclass, field
from functools import cached_property
from typing import TYPE_CHECKING, Any, Literal

import numpy as np

from larmorx import _core

if TYPE_CHECKING:
    from larmorx.image import Image

__all__ = ["Extension", "ItkGeometry", "NiftiError", "NiftiHeader", "load", "read_header", "save"]

logger = logging.getLogger("larmorx.io")

NiftiError = _core.NiftiError

PathLike = str | os.PathLike[str]

_SPATIAL_UNITS = {0: "unknown", 1: "meter", 2: "mm", 3: "micron"}
_TIME_UNITS = {0: "unknown", 8: "sec", 16: "msec", 24: "usec", 32: "hz", 40: "ppm", 48: "rads"}
_SECONDS_PER_UNIT = {"sec": 1.0, "msec": 1e-3, "usec": 1e-6}


@dataclass(frozen=True)
class Extension:
    """A header extension: its ``NIFTI_ECODE_*`` code and its content as stored."""

    code: int
    content: bytes

    @property
    def trimmed_content(self) -> bytes:
        """The content without trailing NUL padding (what nibabel reports)."""
        return self.content.rstrip(b"\x00")


@dataclass(frozen=True, eq=False)
class ItkGeometry:
    """Where ITK 5.4.5, and so ANTs, places an image read from a NIfTI file.

    ITK chooses between the qform and the sform with rules that differ from nibabel's, so the
    same file can sit differently in space for ANTs than for nibabel. Coordinates are LPS mm.
    """

    ndim: int
    size: tuple[int, ...]
    spacing: tuple[float, ...]
    origin: tuple[float, ...]
    direction: np.ndarray  # ndim x ndim, columns = index axes in LPS
    source: Literal["qform", "sform", "default"]
    ras_affine: np.ndarray  # the spatial geometry as a RAS+ affine


@dataclass(frozen=True, eq=False)
class NiftiHeader:
    """Every field of a NIfTI-1 or NIfTI-2 header, as stored on disk.

    Field names follow ``nifti1.h``/``nifti2.h``. NIfTI-1 values are widened to Python
    ``int``/``float`` without loss. Derived quantities (``shape``, ``affine``, ...) are computed
    by the Rust implementation with nibabel's rules. Use :meth:`replace` to change fields.
    """

    version: int
    byte_order: Literal["<", ">"]
    magic: bytes
    dim_info: int
    dim: tuple[int, ...]
    intent_p1: float
    intent_p2: float
    intent_p3: float
    intent_code: int
    datatype: int
    bitpix: int
    slice_start: int
    pixdim: tuple[float, ...]
    vox_offset: float
    scl_slope: float
    scl_inter: float
    slice_end: int
    slice_code: int
    xyzt_units: int
    cal_max: float
    cal_min: float
    slice_duration: float
    toffset: float
    descrip: bytes
    aux_file: bytes
    qform_code: int
    sform_code: int
    quatern_b: float
    quatern_c: float
    quatern_d: float
    qoffset_x: float
    qoffset_y: float
    qoffset_z: float
    srow_x: tuple[float, ...]
    srow_y: tuple[float, ...]
    srow_z: tuple[float, ...]
    intent_name: bytes
    # NIfTI-1 fields kept unused for Analyze 7.5 compatibility (zero in NIfTI-2).
    data_type: bytes = b""
    db_name: bytes = b""
    extents: int = 0
    session_error: int = 0
    regular: int = 0
    glmax: int = 0
    glmin: int = 0
    # NIfTI-2 only.
    eol_check: bytes = b"\r\n\x1a\n"
    unused_str: bytes = b""
    extensions: tuple[Extension, ...] = field(default=())

    # --- conversion --------------------------------------------------------------------------

    @classmethod
    def from_dict(cls, fields: dict[str, Any]) -> NiftiHeader:
        values = dict(fields)
        for key in ("dim", "pixdim", "srow_x", "srow_y", "srow_z"):
            values[key] = tuple(values[key])
        values["extensions"] = tuple(Extension(int(c), bytes(b)) for c, b in values["extensions"])
        return cls(**values)

    def to_dict(self) -> dict[str, Any]:
        out = {f.name: getattr(self, f.name) for f in dataclasses.fields(self)}
        out["extensions"] = [(e.code, e.content) for e in self.extensions]
        return out

    def replace(self, **changes: Any) -> NiftiHeader:
        """A copy with some fields changed."""
        return dataclasses.replace(self, **changes)

    @cached_property
    def _info(self) -> dict[str, Any]:
        return _core.nifti_header_info(self.to_dict())

    # --- derived quantities --------------------------------------------------------------------

    @property
    def shape(self) -> tuple[int, ...]:
        """The data shape (``dim[1:dim[0]+1]``, with FreeSurfer's long-vector conventions)."""
        return self._info["shape"]

    @property
    def data_dtype(self) -> np.dtype | None:
        """The stored data type, or ``None`` if larmorx does not support it."""
        name = self._info["dtype"]
        return None if name is None else np.dtype(name).newbyteorder(self.byte_order)

    @property
    def zooms(self) -> tuple[float, ...]:
        """``pixdim[1:dim[0]+1]``: voxel sizes, then the time step, ..."""
        return self._info["zooms"]

    @property
    def affine(self) -> np.ndarray:
        """The image affine: sform if ``sform_code > 0``, else qform if ``qform_code > 0``, else
        a default from the voxel sizes (nibabel's ``get_best_affine``)."""
        return self._info["affine"].copy()

    @property
    def qform(self) -> np.ndarray | None:
        """The affine from the qform fields (whatever ``qform_code`` says), or ``None`` if the
        quaternion is invalid."""
        q = self._info["qform"]
        return None if q is None else q.copy()

    @property
    def sform(self) -> np.ndarray:
        """The affine from the sform rows (whatever ``sform_code`` says)."""
        return self._info["sform"].copy()

    @property
    def slope_inter(self) -> tuple[float, float] | None:
        """``(scl_slope, scl_inter)``, or ``None`` when the slope is 0 or not finite."""
        return self._info["slope_inter"]

    @property
    def is_single_file(self) -> bool:
        return self.magic[1:2] == b"+"

    @property
    def description(self) -> str:
        return self.descrip.split(b"\x00", 1)[0].decode("utf-8", "replace")

    @property
    def spatial_unit(self) -> str:
        return _SPATIAL_UNITS.get(self.xyzt_units & 0x07, "unknown")

    @property
    def time_unit(self) -> str:
        return _TIME_UNITS.get(self.xyzt_units & 0x38, "unknown")

    @property
    def tr(self) -> float | None:
        """Time between volumes in seconds (``pixdim[4]`` in the header's time unit), or
        ``None`` for images with fewer than four dimensions or no time unit."""
        if self.dim[0] < 4 or self.time_unit not in _SECONDS_PER_UNIT:
            return None
        return self.pixdim[4] * _SECONDS_PER_UNIT[self.time_unit]

    def to_bytes(self) -> bytes:
        """The header block as it would be stored (without extensions)."""
        return self._info["header_bytes"]

    @cached_property
    def itk_geometry(self) -> ItkGeometry:
        """The geometry ITK 5.4.5 (and so ANTs) reads from this header. Use it on a header as
        stored (:func:`read_header`): nibabel's load-time fixes do not apply to ITK. Raises
        :class:`NiftiError` for headers ITK cannot read (e.g. NIfTI-2)."""
        return ItkGeometry(**_core.nifti_itk_geometry(self.to_dict()))


def read_header(path: PathLike) -> NiftiHeader:
    """The header of a NIfTI file exactly as stored (no fixes applied)."""
    return NiftiHeader.from_dict(_core.nifti_read_header(os.fspath(path), False))


def _scaling(dtype: Any, scaled: bool) -> str:
    if dtype is None:
        return "auto" if scaled else "raw"
    if not scaled:
        raise ValueError("dtype cannot be combined with scaled=False")
    dt = np.dtype(dtype)
    if dt not in (np.dtype(np.float32), np.dtype(np.float64)):
        raise ValueError(f"dtype must be float32 or float64, not {dt}")
    return dt.name


def _memory_map(path: PathLike, scaled: bool) -> Image | None:
    """The image with its data memory-mapped, or ``None`` if the file is not eligible
    (compressed, scaled, non-native byte order, or an unsupported type)."""
    from larmorx.image import Image

    name = os.fspath(path)
    lower = name.lower()
    if lower.endswith((".hdr", ".img")):
        image_path = name[:-4] + (".IMG" if name[-4:].isupper() else ".img")
    elif lower.endswith(".nii"):
        image_path = name
    else:
        return None
    for candidate in {name, image_path}:
        with open(candidate, "rb") as f:
            if f.read(2) == b"\x1f\x8b":
                return None
    fields, fixes = _core.nifti_read_header(name, True)
    header = NiftiHeader.from_dict(fields)
    dtype = header.data_dtype
    if (
        dtype is None
        or not dtype.isnative
        or (scaled and header.slope_inter not in (None, (1.0, 0.0)))
    ):
        return None
    offset = int(header.vox_offset)
    if header.is_single_file and offset == 0:
        return None
    for fix in fixes:
        logger.warning("%s: %s", name, fix)
    data = np.memmap(
        image_path, dtype=dtype, mode="c", offset=offset, shape=header.shape, order="F"
    )
    return Image(data, header.affine, header)


def load(
    path: PathLike,
    *,
    dtype: Any = None,
    scaled: bool = True,
    mmap: bool = False,
    n_threads: int = 1,
) -> Image:
    """Read a NIfTI image.

    Parameters
    ----------
    path
        ``.nii``, ``.nii.gz``, ``.hdr``/``.img`` (either name of a pair), ``.hdr.gz``/``.img.gz``.
    dtype
        ``None`` (default) gives what nibabel's ``img.dataobj`` gives: the stored type, or
        float64 if the header scales the data. ``np.float32``/``np.float64`` give what
        ``img.get_fdata(dtype=...)`` gives.
    scaled
        ``False`` returns the stored values, ignoring ``scl_slope``/``scl_inter``.
    mmap
        Memory-map the voxel data instead of reading it (copy-on-write, like nibabel's default
        ``mmap='c'``) when the file allows it: uncompressed, native byte order, no scaling (or
        ``scaled=False``), and ``dtype=None``. Other files are read normally.
    n_threads
        Threads for byte swapping and scaling (0 = all logical CPUs).

    The header fields that nibabel fixes when reading (zero or negative voxel sizes, an invalid
    ``qfac`` or xform code, a wrong ``bitpix``) are fixed in the returned header and logged.
    """
    from larmorx.image import Image

    if mmap and dtype is None and (mapped := _memory_map(path, scaled)) is not None:
        return mapped
    data, affine, fields, fixes, _ = _core.nifti_read(
        os.fspath(path), _scaling(dtype, scaled), n_threads
    )
    for fix in fixes:
        logger.warning("%s: %s", os.fspath(path), fix)
    return Image(data, affine, NiftiHeader.from_dict(fields))


def _storable(data: np.ndarray) -> np.ndarray:
    """``data`` in a type and byte order NIfTI can store, converting losslessly if needed."""
    if data.dtype == np.bool_:
        return data.astype(np.uint8)
    if not data.dtype.isnative:
        data = data.astype(data.dtype.newbyteorder("="))
    if data.dtype.kind not in "iufc" or data.dtype.itemsize not in (1, 2, 4, 8, 16):
        raise TypeError(f"NIfTI cannot store data of type {data.dtype}")
    if data.dtype.kind == "f" and data.dtype.itemsize == 2:
        raise TypeError("NIfTI cannot store float16 data; convert to float32 first")
    return data


def save(
    image: Any,
    path: PathLike,
    *,
    dtype: Any = None,
    version: int | None = None,
    compression_level: int = 2,
    n_threads: int = 1,
) -> None:
    """Write an image as NIfTI; ``.gz`` names are compressed.

    Parameters
    ----------
    image
        An :class:`larmorx.Image`, a nibabel image, or an ``(array, affine)`` pair.
    dtype
        Store the data as this type (``data.astype(dtype)``). By default the array's own type is
        stored, without rescaling.
    version
        1 or 2. By default the header's version, or NIfTI-1 when the shape fits.
    compression_level
        gzip level, 0-9. The default, 2, gives files the size nibabel's level 1 gives (larmorx's
        level 1 is a faster strategy that compresses noticeably less).
    n_threads
        Threads for compression (0 = all logical CPUs). The file is identical for any value.

    The header is the image's header updated as nibabel does: its qform/sform are kept if the
    image affine still matches them, and replaced by the affine (sform code 2, qform code 0)
    otherwise. The scale factors are set to "no scaling" (1 and 0, as nibabel writes them),
    because the data are stored as they are.
    """
    from larmorx.image import as_image

    img = as_image(image)
    data = img.data if dtype is None else np.asarray(img.data).astype(dtype)
    data = _storable(np.asarray(data))
    template = img.header.to_dict() if img.header is not None else None
    fields = _core.nifti_header_for_image(
        list(data.shape),
        data.dtype.name,
        np.asarray(img.affine, dtype=np.float64),
        template,
        version,
    )
    _core.nifti_write(os.fspath(path), data, fields, compression_level, n_threads)
