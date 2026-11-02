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
the original. ``flirt`` and ``convert_xfm`` have both too (the original written from
``specs/flirt.md``, since 2026-11-02); FLIRT's other programs and ``fast`` (FSL's tissue
segmentation, since 2026-11-02) have their replica only.
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

__all__ = [
    "COSTS",
    "VARIATIONS",
    "AvscaleResult",
    "BetResult",
    "BetsurfResult",
    "FastResult",
    "FlirtResult",
    "Surface",
    "aff2rigid",
    "applyxfm4d",
    "avscale",
    "bet",
    "betsurf",
    "convert_xfm",
    "extracttxt",
    "fast",
    "flirt",
    "img2imgcoord",
    "img2stdcoord",
    "makerot",
    "midtrans",
    "pairreg",
    "pointflirt",
    "read_vtk",
    "rmsdiff",
    "std2imgcoord",
]
