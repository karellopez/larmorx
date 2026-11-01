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
the original.
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
from larmorx.fsl.flirt import COSTS, FlirtResult, convert_xfm, flirt
from larmorx.fsl.flirt_tools import (
    AvscaleResult,
    avscale,
    img2imgcoord,
    img2stdcoord,
    rmsdiff,
    std2imgcoord,
)

__all__ = [
    "COSTS",
    "VARIATIONS",
    "AvscaleResult",
    "BetResult",
    "BetsurfResult",
    "FlirtResult",
    "Surface",
    "avscale",
    "bet",
    "betsurf",
    "convert_xfm",
    "flirt",
    "img2imgcoord",
    "img2stdcoord",
    "read_vtk",
    "rmsdiff",
    "std2imgcoord",
]
