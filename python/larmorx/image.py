"""``Image``: the in-memory image type shared by every larmorx tool."""

from __future__ import annotations

import dataclasses
import os
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

import numpy as np

if TYPE_CHECKING:
    from larmorx.io.nifti import NiftiHeader

__all__ = ["Image", "as_image"]


@dataclass(frozen=True, eq=False)
class Image:
    """A voxel array placed in world space by an affine.

    ``data`` is indexed ``[i, j, k, ...]`` like nibabel's arrays (arrays read from files are in
    Fortran order). ``affine`` maps voxel indices ``(i, j, k)`` to RAS+ millimetres. ``header``
    is the NIfTI header the image was read with, if any; it carries metadata such as units,
    the repetition time, slice timing and extensions, and is reconciled with ``affine`` when
    the image is saved.
    """

    data: np.ndarray
    affine: np.ndarray
    header: NiftiHeader | None = None

    def __post_init__(self) -> None:
        data = np.asarray(self.data)
        if not 1 <= data.ndim <= 7:
            raise ValueError(f"images have 1 to 7 dimensions, not {data.ndim}")
        affine = np.array(self.affine, dtype=np.float64)
        if affine.shape != (4, 4):
            raise ValueError(f"the affine must be 4x4, not {affine.shape}")
        affine.setflags(write=False)
        object.__setattr__(self, "data", data)
        object.__setattr__(self, "affine", affine)

    def __repr__(self) -> str:
        sizes = ", ".join(f"{z:.3g}" for z in self.voxel_sizes)
        return f"Image(shape={self.shape}, dtype={self.dtype}, voxel_sizes=({sizes}))"

    @property
    def shape(self) -> tuple[int, ...]:
        return self.data.shape

    @property
    def ndim(self) -> int:
        return self.data.ndim

    @property
    def dtype(self) -> np.dtype:
        return self.data.dtype

    @property
    def voxel_sizes(self) -> tuple[float, float, float]:
        """Lengths of the affine's first three columns (``nibabel.affines.voxel_sizes``)."""
        rzs = self.affine[:3, :3]
        return tuple(float(v) for v in np.sqrt(np.sum(rzs * rzs, axis=0)))  # type: ignore[return-value]

    @property
    def tr(self) -> float | None:
        """Repetition time in seconds, from the header, or ``None``."""
        return None if self.header is None else self.header.tr

    def replace(self, **changes: Any) -> Image:
        """A copy with some fields changed (``data``, ``affine`` or ``header``)."""
        return dataclasses.replace(self, **changes)

    def with_data(self, data: np.ndarray) -> Image:
        """The same affine and header with new data."""
        return dataclasses.replace(self, data=data)

    def save(self, path: str | os.PathLike[str], **options: Any) -> None:
        """Write the image; see :func:`larmorx.io.save`."""
        from larmorx.io.nifti import save

        save(self, path, **options)

    # --- nibabel interoperability ---------------------------------------------------------------

    @classmethod
    def from_nibabel(cls, img: Any, *, dtype: Any = None) -> Image:
        """Convert a nibabel image. ``dtype=None`` reads ``img.dataobj`` (nibabel's stored or
        scaled values); a float dtype calls ``img.get_fdata(dtype=...)``."""
        from larmorx.io.nifti import Extension, NiftiHeader

        data = np.asanyarray(img.dataobj) if dtype is None else img.get_fdata(dtype=dtype)
        header = None
        nib_header = getattr(img, "header", None)
        if (
            nib_header is not None
            and hasattr(nib_header, "binaryblock")
            and hasattr(nib_header, "extensions")
        ):
            from larmorx import _core

            fields = _core.nifti_parse_header(nib_header.binaryblock)
            header = NiftiHeader.from_dict(fields).replace(
                extensions=tuple(
                    Extension(int(e.get_code()), bytes(e.content)) for e in nib_header.extensions
                )
            )
        return cls(data, np.asarray(img.affine, dtype=np.float64), header)

    def to_nibabel(self) -> Any:
        """A ``nibabel.Nifti1Image`` (or ``Nifti2Image`` for a NIfTI-2 header) with the same
        data, affine and header fields."""
        import nibabel as nib

        version = 2 if self.header is not None and self.header.version == 2 else 1
        klass = nib.Nifti2Image if version == 2 else nib.Nifti1Image
        if self.header is None:
            return klass(self.data, self.affine)
        header = klass.header_class(self.header.to_bytes(), endianness=self.header.byte_order)
        for ext in self.header.extensions:
            header.extensions.append(nib.nifti1.Nifti1Extension(ext.code, ext.trimmed_content))
        return klass(self.data, self.affine, header)


def as_image(obj: Any) -> Image:
    """Coerce ``obj`` to an :class:`Image`.

    Accepts an ``Image``, a path to a NIfTI file, a nibabel image, or an ``(array, affine)``
    pair.
    """
    if isinstance(obj, Image):
        return obj
    if isinstance(obj, (str, os.PathLike)):
        from larmorx.io.nifti import load

        return load(obj)
    if hasattr(obj, "dataobj") and hasattr(obj, "affine"):
        return Image.from_nibabel(obj)
    if isinstance(obj, tuple) and len(obj) == 2:
        return Image(np.asarray(obj[0]), np.asarray(obj[1]))
    raise TypeError(f"cannot make an Image from {type(obj).__name__}")
