# SPDX-License-Identifier: Apache-2.0
"""``lx.fsl.fslroi`` and ``lx.fsl.standard_space_roi``: tools with only their replicas
(``larmorx-nc``) so far. Without the replica's program, ``auto`` and ``replica`` raise
:class:`larmorx._replica.ReplicaNotFoundError` and ``original`` ``NotImplementedError``; the
arguments are checked first. Their outputs are checked against FSL by the BET replica suite
(``fsl-bet``, ``--implementation replica``) and on every platform by larmorx-nc's golden cases."""

from __future__ import annotations

import numpy as np
import pytest

import larmorx as lx
from larmorx import _replica

pytestmark = pytest.mark.usefixtures("no_replica")


def _image() -> lx.Image:
    return lx.Image(np.zeros((4, 4, 4), np.float32), np.eye(4))


def test_fslroi_needs_a_region():
    with pytest.raises(ValueError, match="needs a region"):
        lx.fsl.fslroi(_image())


@pytest.mark.parametrize("function", ["fslroi", "standard_space_roi"])
def test_replica_only(function):
    call = getattr(lx.fsl, function)
    kwargs = {"t": (0, 1)} if function == "fslroi" else {}
    with pytest.raises(NotImplementedError, match="no clean-room original"):
        call(_image(), implementation="original", **kwargs)
    for implementation in ("auto", "replica"):
        with pytest.raises(_replica.ReplicaNotFoundError, match="larmorx-nc"):
            call(_image(), implementation=implementation, **kwargs)


def test_registry_lists_them_without_an_original():
    reg = _replica.registry()
    for tool in ("fslroi", "standard_space_roi"):
        r = reg[("fsl", tool)]
        assert not r.original
        assert r.package == "larmorx-nc"
