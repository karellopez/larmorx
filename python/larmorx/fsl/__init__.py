# SPDX-License-Identifier: Apache-2.0
"""FSL-compatible tools: ``lx.fsl``.

FSL is distributed under the FSL Licence, which allows non-commercial use only, so larmorx keeps
two implementations of each FSL tool apart (``docs/licensing.md``): a bit-exact replica
translated from FSL's source, in the separate package ``larmorx-nc`` (FSL's licence terms,
``pip install larmorx-nc``), and a clean-room original in this Apache-2.0 package, written from
a behaviour spec without FSL's code. The same programs run from the command line as
``larmorx fsl <program> ...`` with their original arguments.

``bet`` and ``betsurf`` have their replica (FSL 6.0.7.23's ``bet``, ``bet2`` and ``betsurf``),
run as a separate process (nothing of it is imported), and their clean-room original (``bet2``,
the ``bet`` script's ``-R``, ``-F``, ``-Z``, and ``betsurf``; written from ``specs/bet.md``):
``implementation="auto"`` (the default) runs the replica when ``larmorx-nc`` is installed, else
the original. ``flirt``, ``convert_xfm`` and FLIRT's other programs (``avscale``, the coordinate
programs ``img2imgcoord``, ``std2imgcoord`` and ``img2stdcoord``, ``rmsdiff``, ``makerot``,
``midtrans``, ``pointflirt``, ``applyxfm4d``) and scripts (``pairreg``, ``extracttxt``,
``aff2rigid``) have both too (the originals written from ``specs/flirt.md``, since 2026-11-02).
``fast`` (FSL's tissue segmentation) has its replica (since 2026-11-02) and its clean-room
original (since 2026-11-03, written from ``specs/fast.md``). ``topup`` and
``applytopup`` (the susceptibility field from reversed phase encodings and its correction, since
2026-11-03) have their replicas and their clean-room originals (written from ``specs/topup.md``).
``fslmaths`` (the image calculator: the generic :func:`fslmaths` and typed wrappers of its
operations, :func:`arithmetic`, :func:`threshold`, :func:`collapse`, ...) and ``fslstats`` (image
statistics: the generic :func:`fslstats` and typed wrappers, :func:`image_mean`,
:func:`centre_of_gravity`, ...) have their replicas only, since 2026-11-03.
"""

from larmorx.fsl.bet import (
    VARIATIONS,
    BetResult,
    BetsurfResult,
    Surface,
    bet,
    betsurf,
    read_vtk,
)
from larmorx.fsl.fast import FastResult, fast
from larmorx.fsl.flirt import COSTS, FlirtResult, convert_xfm, flirt
from larmorx.fsl.flirt_more import (
    aff2rigid,
    applyxfm4d,
    extracttxt,
    midtrans,
    pairreg,
    pointflirt,
)
from larmorx.fsl.flirt_tools import (
    AvscaleResult,
    avscale,
    img2imgcoord,
    img2stdcoord,
    makerot,
    rmsdiff,
    std2imgcoord,
)
from larmorx.fsl.maths import (
    BINARY,
    COLLAPSE,
    DATATYPES,
    SPATIAL_FILTERS,
    STATISTICAL,
    UNARY,
    RocResult,
    TensorDecomposition,
    add_noise,
    apply_mask,
    arithmetic,
    bandpass_temporal_filter,
    binarise,
    collapse,
    dog_edge,
    fslmaths,
    grid,
    normalise_intensity,
    roc,
    roi,
    smooth,
    spatial_filter,
    statistical,
    subsample2,
    tensor_decomp,
    tfce,
    tfce_support,
    threshold,
    unary,
)
from larmorx.fsl.stats import (
    bounding_roi,
    centre_of_gravity,
    extremum_position,
    fslstats,
    image_entropy,
    image_histogram,
    image_mean,
    image_percentile,
    image_range,
    image_std,
    image_volume,
)
from larmorx.fsl.topup import ApplytopupResult, TopupResult, applytopup, topup

__all__ = [
    "BINARY",
    "COLLAPSE",
    "COSTS",
    "DATATYPES",
    "SPATIAL_FILTERS",
    "STATISTICAL",
    "UNARY",
    "VARIATIONS",
    "ApplytopupResult",
    "AvscaleResult",
    "BetResult",
    "BetsurfResult",
    "FastResult",
    "FlirtResult",
    "RocResult",
    "Surface",
    "TensorDecomposition",
    "TopupResult",
    "add_noise",
    "aff2rigid",
    "apply_mask",
    "applytopup",
    "applyxfm4d",
    "arithmetic",
    "avscale",
    "bandpass_temporal_filter",
    "bet",
    "betsurf",
    "binarise",
    "bounding_roi",
    "centre_of_gravity",
    "collapse",
    "convert_xfm",
    "dog_edge",
    "extracttxt",
    "extremum_position",
    "fast",
    "flirt",
    "fslmaths",
    "fslstats",
    "grid",
    "image_entropy",
    "image_histogram",
    "image_mean",
    "image_percentile",
    "image_range",
    "image_std",
    "image_volume",
    "img2imgcoord",
    "img2stdcoord",
    "makerot",
    "midtrans",
    "normalise_intensity",
    "pairreg",
    "pointflirt",
    "read_vtk",
    "rmsdiff",
    "roc",
    "roi",
    "smooth",
    "spatial_filter",
    "statistical",
    "std2imgcoord",
    "subsample2",
    "tensor_decomp",
    "tfce",
    "tfce_support",
    "threshold",
    "topup",
    "unary",
]
