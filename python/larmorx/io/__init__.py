# SPDX-License-Identifier: Apache-2.0
"""Reading and writing images (NIfTI-1/2 for now).

- :func:`load`, :func:`save`, :func:`read_header`: larmorx's NIfTI reader and writer (``save``
  stores the array's own type).
- :func:`save_nibabel`, :func:`nibabel_stored`: an image written as nibabel's
  ``img.to_filename`` writes it (the header's data type, float data scaled into an integer type
  with nibabel's ``scl_slope``/``scl_inter``), for ports of Python programs that write through
  nibabel (sdcflows, fMRIPrep's own steps).
"""

from larmorx.io.nibabel_writer import (
    HeaderDataError,
    ScalingError,
    WriterError,
    nibabel_stored,
    save_nibabel,
)
from larmorx.io.nifti import (
    Extension,
    ItkGeometry,
    NiftiError,
    NiftiHeader,
    load,
    read_header,
    save,
)

__all__ = [
    "Extension",
    "HeaderDataError",
    "ItkGeometry",
    "NiftiError",
    "NiftiHeader",
    "ScalingError",
    "WriterError",
    "load",
    "nibabel_stored",
    "read_header",
    "save",
    "save_nibabel",
]
