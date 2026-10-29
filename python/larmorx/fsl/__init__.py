# SPDX-License-Identifier: Apache-2.0
"""FSL-compatible tools: ``lx.fsl``.

FSL is distributed under the FSL Licence, which allows non-commercial use only, so larmorx keeps
two implementations of each FSL tool apart (``docs/licensing.md``): a bit-exact replica
translated from FSL's source, in the separate package ``larmorx-nc`` (FSL's licence terms,
``pip install larmorx-nc``), and a clean-room original in this Apache-2.0 package, written from
a behaviour spec without FSL's code. The same programs run from the command line as
``larmorx fsl <program> ...`` with their original arguments.

``bet`` and ``betsurf`` have their replica (FSL 6.0.7.23's ``bet``, ``bet2`` and ``betsurf``);
their clean-room originals are still to be written (``specs/bet.md``). Until then they need
``larmorx-nc``: with ``implementation="auto"`` (the default) or ``"replica"`` they run its
program as a separate process (nothing of it is imported), and without it they raise
:class:`~larmorx.ReplicaNotFoundError` naming the command that installs it.
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

__all__ = [
    "VARIATIONS",
    "BetResult",
    "BetsurfResult",
    "Surface",
    "bet",
    "betsurf",
    "read_vtk",
]
