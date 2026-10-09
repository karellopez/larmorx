# SPDX-License-Identifier: Apache-2.0
"""larmorx: neuroimaging tools in Rust, with a Python wrapper and a CLI for every tool.

Use it as ``import larmorx as lx``::

    img = lx.load("sub-01_T1w.nii.gz")        # lx.Image: data, affine, header
    lx.save(img.with_data(img.data * 2), "out.nii.gz", n_threads=4)
"""

from larmorx import afni, ants, io, transforms
from larmorx._core import __version__
from larmorx.image import Image, as_image
from larmorx.io import load, save

__all__ = [
    "Image",
    "__version__",
    "afni",
    "ants",
    "as_image",
    "io",
    "load",
    "save",
    "transforms",
]
