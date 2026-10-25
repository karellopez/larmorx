# SPDX-License-Identifier: Apache-2.0
"""larmorx: neuroimaging tools in Rust, with a Python wrapper and a CLI for every tool.

Use it as ``import larmorx as lx``::

    img = lx.load("sub-01_T1w.nii.gz")        # lx.Image: data, affine, header
    lx.save(img.with_data(img.data * 2), "out.nii.gz", n_threads=4)

Where an original program has a deterministic defect that changes its results, larmorx
corrects it by default and raises :class:`UpstreamDefectWarning` when it changed a result;
``upstream_defects="reproduce"`` gives the original's result (``docs/api/upstream-defects.md``).

The subpackages and names load on first use (PEP 562), so the command line (``larmorx``,
``lx``), which needs only the Rust core, starts without NumPy: its resident memory stays at
the original programs' (random testing, 2026-10-25: larmorx's peak memory against ANTs').
"""

from __future__ import annotations

import importlib
from typing import TYPE_CHECKING

from larmorx._core import __version__

if TYPE_CHECKING:
    from larmorx import afni, ants, fsl, io, mri, ndimage, transforms
    from larmorx._defects import UpstreamDefectWarning
    from larmorx._replica import Implementation, ReplicaError, ReplicaNotFoundError
    from larmorx.image import Image, as_image
    from larmorx.io import load, save

_SUBMODULES = frozenset({"afni", "ants", "fsl", "io", "mri", "ndimage", "transforms"})
_NAMES = {
    "UpstreamDefectWarning": "larmorx._defects",
    "Implementation": "larmorx._replica",
    "ReplicaError": "larmorx._replica",
    "ReplicaNotFoundError": "larmorx._replica",
    "Image": "larmorx.image",
    "as_image": "larmorx.image",
    "load": "larmorx.io",
    "save": "larmorx.io",
}

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


def __getattr__(name: str):
    if name in _SUBMODULES:
        module = importlib.import_module(f"larmorx.{name}")
    elif name in _NAMES:
        module = getattr(importlib.import_module(_NAMES[name]), name)
    else:
        raise AttributeError(f"module 'larmorx' has no attribute {name!r}")
    globals()[name] = module
    return module


def __dir__() -> list[str]:
    return sorted({*globals(), *__all__})
