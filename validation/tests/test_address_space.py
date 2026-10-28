# SPDX-License-Identifier: Apache-2.0
"""The address-space limit the original programs run under (``larmorx_validation.address_space``):
an allocation beyond it is refused alike on every machine, whatever its memory and swap
(docs/findings/machine-move-proof.md). The limit is applied to the process that runs the
program, inherited by what it starts, and lowered only around larmorx's in-process runs."""

from __future__ import annotations

import subprocess
import sys
import textwrap

import pytest

if sys.platform != "linux":
    pytest.skip("the originals run on Linux (RLIMIT_AS)", allow_module_level=True)

import resource

from larmorx_validation import address_space

GIB = 1 << 30

#: A child that prints its own address-space limit, then its child's (inherited).
REPORT = textwrap.dedent(
    """\
    import resource, subprocess, sys
    print(resource.getrlimit(resource.RLIMIT_AS)[0])
    code = "import resource; print(resource.getrlimit(resource.RLIMIT_AS)[0])"
    print(subprocess.run([sys.executable, "-c", code], capture_output=True, text=True).stdout.strip())
    """
)

#: A child that asks for ``argv[1]`` bytes at once (a refused request fails before any is
#: touched: nothing is allocated).
ALLOCATE = textwrap.dedent(
    """\
    import mmap, sys
    try:
        m = mmap.mmap(-1, int(sys.argv[1]))
    except (OSError, MemoryError, OverflowError) as e:
        print("refused", type(e).__name__)
        sys.exit(3)
    print("granted")
    """
)


def test_the_limit_and_its_variable(monkeypatch):
    monkeypatch.delenv(address_space.ENV, raising=False)
    assert address_space.limit() == address_space.DEFAULT == 64 * GIB
    for text, value in (("96G", 96 * GIB), ("512 MiB", 512 << 20), ("1T", 1 << 40), ("4096", 4096)):
        monkeypatch.setenv(address_space.ENV, text)
        assert address_space.limit() == value
    for text in ("off", "0", "none", "unlimited"):
        monkeypatch.setenv(address_space.ENV, text)
        assert address_space.limit() is None and address_space.child_setup() is None
    monkeypatch.setenv(address_space.ENV, "lots")
    with pytest.raises(ValueError):
        address_space.limit()
    assert address_space.for_memory(72 * GIB) == 144 * GIB
    assert address_space.for_memory(GIB) == address_space.DEFAULT


def test_a_program_runs_under_the_limit_and_what_it_starts_inherits_it(monkeypatch):
    monkeypatch.setenv(address_space.ENV, "3G")
    p = subprocess.run(
        [sys.executable, "-c", REPORT],
        capture_output=True,
        text=True,
        preexec_fn=address_space.child_setup(),
        check=True,
    )
    assert p.stdout.split() == [str(3 * GIB)] * 2


def test_an_allocation_beyond_the_limit_is_refused_at_once(monkeypatch):
    monkeypatch.setenv(address_space.ENV, "2G")
    run = [sys.executable, "-c", ALLOCATE]
    over = subprocess.run(
        [*run, str(3 * GIB)], capture_output=True, text=True, preexec_fn=address_space.child_setup()
    )
    assert over.returncode == 3 and over.stdout.startswith("refused")
    under = subprocess.run(
        [*run, str(256 << 20)],
        capture_output=True,
        text=True,
        preexec_fn=address_space.child_setup(),
    )
    assert under.returncode == 0 and under.stdout.strip() == "granted"


def test_the_oracle_cache_runs_originals_under_the_limit(tmp_path, monkeypatch):
    """Every original program the harness runs goes through the cache's runner: served or not,
    off, refresh, ``verify`` and random testing alike."""
    from larmorx_validation import oracle_cache as oc

    monkeypatch.setenv(address_space.ENV, "5G")
    monkeypatch.setenv(oc.ENV_LOCATION, str(tmp_path / "cache"))
    with oc.using("off"):
        p = oc.run([sys.executable, "-c", REPORT], policy=oc.Policy(threads=1))
    assert p.stdout.split() == [str(5 * GIB)] * 2
    monkeypatch.setenv(address_space.ENV, "off")
    with oc.using("off"):
        p = oc.run([sys.executable, "-c", REPORT], policy=oc.Policy(threads=1))
    soft = resource.getrlimit(resource.RLIMIT_AS)[0]
    assert p.stdout.split() == [str(soft)] * 2


def test_a_lower_limit_already_set_is_kept(monkeypatch):
    soft, hard = resource.getrlimit(resource.RLIMIT_AS)
    assert address_space._lowered(4 * GIB, (2 * GIB, hard)) == (2 * GIB, hard)
    assert address_space._lowered(4 * GIB, (resource.RLIM_INFINITY, 3 * GIB))[0] == 3 * GIB
    assert soft == resource.getrlimit(resource.RLIMIT_AS)[0]


def test_in_process_runs_lower_the_soft_limit_and_restore_it(monkeypatch):
    """larmorx's in-process runs of the ANTs image programs (``ants_programs.run_larmorx``): the
    soft limit inside the block, the previous limits after it, even after an error."""
    before = resource.getrlimit(resource.RLIMIT_AS)
    monkeypatch.setenv(address_space.ENV, "60G")
    with address_space.limited() as n:
        assert n == resource.getrlimit(resource.RLIMIT_AS)[0] <= 60 * GIB
        with pytest.raises((MemoryError, OverflowError, OSError)):
            import mmap

            mmap.mmap(-1, 61 * GIB)
    assert resource.getrlimit(resource.RLIMIT_AS) == before
    with pytest.raises(RuntimeError), address_space.limited():
        raise RuntimeError("larmorx failed")
    assert resource.getrlimit(resource.RLIMIT_AS) == before
    monkeypatch.setenv(address_space.ENV, "off")
    with address_space.limited() as n:
        assert n is None and resource.getrlimit(resource.RLIMIT_AS) == before


def test_the_ants_programs_larmorx_runner_uses_it(monkeypatch):
    """``ants_programs.run_larmorx`` runs larmorx inside :func:`address_space.limited`."""
    import inspect

    from larmorx_validation.parity import ants_programs

    assert "address_space.limited()" in inspect.getsource(ants_programs._run_larmorx)
