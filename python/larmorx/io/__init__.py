"""Reading and writing images (NIfTI-1/2 for now)."""

from larmorx.io.nifti import (
    Extension,
    ItkGeometry,
    NiftiError,
    NiftiHeader,
    load,
    read_header,
    save,
)

__all__ = ["Extension", "ItkGeometry", "NiftiError", "NiftiHeader", "load", "read_header", "save"]
