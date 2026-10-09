"""Reading and writing images (NIfTI-1/2 for now)."""

from larmorx.io.nifti import Extension, NiftiError, NiftiHeader, load, read_header, save

__all__ = ["Extension", "NiftiError", "NiftiHeader", "load", "read_header", "save"]
