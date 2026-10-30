# SPDX-License-Identifier: Apache-2.0
"""A one-thread call starts no thread outside its own pool (CLAUDE.md rule 5: explicit
``n_threads``; the user, 2026-10-26: "n_threads being exact").

Every registered program over its distinct code paths (``larmorx_validation.parity.thread_pools``:
each registration stage and metric, warps, filters, Atropos' initialisations, the AFNI originals,
``mri hmc``, the AFNI replicas, and the FSL replicas where their program is built) runs at one thread on
small seeded inputs; every thread of its processes is listed while it runs. A thread that is not a
process's main thread and not a ``larmorx-<n>`` pool thread is parallel work outside the call's
pool (rayon's global pool, sized by the machine's CPUs: found in the data panel's re-validation,
``docs/findings/ants-cli.md``). Linux only (``/proc``); never in CI (the parity directory).
"""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

tp = pytest.importorskip("larmorx_validation.parity.thread_pools")

if not sys.platform.startswith("linux"):
    pytest.skip("threads are listed from /proc", allow_module_level=True)


def _replicas() -> tuple[str | None]:
    """The FSL replicas' program, where it is built (the AFNI replicas are part of larmorx)."""
    from larmorx_validation.parity.fsl_bet import replica_binary as nc_binary

    nc = nc_binary()
    return (str(nc) if nc else None,)


NAMES = [p.name for p in tp.programs(Path("inputs"), *_replicas(), make=False)]


@pytest.fixture(scope="module")
def setup(tmp_path_factory) -> tuple[Path, dict]:
    d = tmp_path_factory.mktemp("thread-pools")
    return d, {p.name: p for p in tp.programs(d, *_replicas())}


@pytest.mark.parametrize("name", NAMES)
def test_one_thread_call_stays_in_its_pool(name, setup):
    d, programs = setup
    code, pool, other, err = tp.probe(programs[name], d)
    assert code == 0, f"{name}: exit {code}: {err}"
    assert not other, f"{name}: threads outside its pool: {other} (its pool threads: {pool})"
