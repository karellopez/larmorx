# SPDX-License-Identifier: Apache-2.0
"""larmorx: neuroimaging tools in Rust, with a Python wrapper and a CLI for every tool.

Use it as ``import larmorx as lx``::

    img = lx.load("sub-01_T1w.nii.gz")        # lx.Image: data, affine, header
    lx.save(img.with_data(img.data * 2), "out.nii.gz", n_threads=4)

Where an original program has a deterministic defect that changes its results, larmorx
corrects it by default and raises :class:`UpstreamDefectWarning` when it changed a result;
``upstream_defects="reproduce"`` gives the original's result (``docs/api/upstream-defects.md``).
"""

from larmorx import afni, ants, fsl, io, mri, ndimage, transforms
from larmorx._core import __version__
from larmorx._defects import UpstreamDefectWarning
from larmorx._replica import Implementation, ReplicaError, ReplicaNotFoundError
from larmorx.image import Image, as_image
from larmorx.io import load, save

__all__ = [
    "Image",
    "Implementation",
    "ReplicaError",
    "ReplicaNotFoundError",
    "UpstreamDefectWarning",
    "__version__",
    "afni",
    "ants",
    "as_image",
    "fsl",
    "io",
    "load",
    "mri",
    "ndimage",
    "save",
    "transforms",
]
