# SPDX-License-Identifier: Apache-2.0
"""A fixed address-space limit for the original programs, so an impossible allocation is
refused alike on every machine.

Linux refuses an allocation only when it exceeds what the machine could ever give (its overcommit
heuristic: about RAM plus swap); below that it grants the address space and the process grows
while it writes. So the cases that ask for 96 to 111 GiB (ImageMath's ``MD`` with radius -1:
96.0 GiB; ``difftest/allocation-refused`` of ResampleImage and ResampleImageBySpacing: 109.2 and
110.9 GiB) were refused on the old laptop (31 GiB + 63 GiB of swap: ANTs throws
``std::bad_alloc`` or ITK's ``MemoryAllocationError``, as recorded), and granted on the new main
machine (124 GiB + 199 GiB), where the process grew until its memory box stopped it, and once the
smoke tier's whole box with it (``docs/findings/machine-move-proof.md``).

Every process the harness starts to run an original program (:func:`child_setup`, through the
oracle cache's runner, so the parity suites, ``oracle-cache verify`` and random testing alike)
therefore runs under ``RLIMIT_AS`` = :func:`limit`, inherited by everything it starts (the ``bet``
script's ``bet2``, Apptainer's container); and larmorx's in-process runs of the ANTs image
programs, whose cases expect the same refusal, run with this process's soft limit lowered to it
(:func:`limited`). An allocation beyond it fails at once on any machine, as it did on the old one,
and nothing is allocated.

The limit is :data:`DEFAULT` (64 GiB), or ``LARMORX_ADDRESS_SPACE_LIMIT`` (bytes, or with a
binary suffix: ``96G``; ``off`` for none). 64 GiB is below the smallest refusal the records
expect (96 GiB) and below RAM plus swap of every machine the validation runs on, and far above
the largest real call measured (2026-10-28, ``validation/investigations/address-space/``): the
largest virtual peak of any original program on the data panel's largest inputs is 9.4 GiB
(mcflirt on the 600-volume multiband-8 BOLD series; antsApplyTransforms on it 6.7 GiB, ImageMath
3.6 GiB, 3dTshift with OpenMP at 32 threads 2.6 GiB, sMRIPrep's registrations 2.6 GiB, fMRIPrep's
``bet`` through Apptainer 1.8 GiB); the harness with larmorx in-process peaks at 19.3 GiB
(ImageMath on that series, the comparison of its outputs included).
Random testing derives it from each case's memory budget (:func:`for_memory`: twice it, never
below :data:`DEFAULT`): a night's 3 GiB cases run under 64 GiB, ``difftest --heavy`` under twice
its box (memory and swap), so a heavy case is never refused what its box allows.
"""

from __future__ import annotations

import contextlib
import os
import re
from collections.abc import Callable, Iterator

try:
    import resource
except ImportError:  # pragma: no cover - not POSIX (the originals run on Linux only)
    resource = None  # type: ignore[assignment]

#: The variable that sets another limit (bytes, ``96G``, ``512M``; ``off``: none).
ENV = "LARMORX_ADDRESS_SPACE_LIMIT"
#: The limit when the variable is not set.
DEFAULT = 64 << 30

_UNITS = {"": 1, "K": 1 << 10, "M": 1 << 20, "G": 1 << 30, "T": 1 << 40}


def parse(text: str) -> int | None:
    """``"64G"``, ``"512 MiB"``, ``"68719476736"``: bytes; ``off``, ``none``, ``0``,
    ``unlimited``: ``None`` (no limit). ``ValueError`` for anything else."""
    t = text.strip()
    if t.lower() in ("off", "none", "0", "unlimited", "no"):
        return None
    m = re.fullmatch(r"(\d+(?:\.\d+)?)\s*([KMGT]?)(?:i?B)?", t, flags=re.IGNORECASE)
    if m is None:
        raise ValueError(f"{ENV}={text!r}: bytes, with K, M, G or T, or 'off'")
    return int(float(m.group(1)) * _UNITS[m.group(2).upper()])


def limit() -> int | None:
    """The address-space limit of the original programs, in bytes (``None``: none)."""
    value = os.environ.get(ENV, "")
    return DEFAULT if not value.strip() else parse(value)


def for_memory(nbytes: int) -> int:
    """The limit for runs given ``nbytes`` of memory (and swap) to fill: twice that, at least
    :data:`DEFAULT` (``difftest --heavy``'s box: its cases are run to their end)."""
    return max(DEFAULT, 2 * int(nbytes))


def describe(n: int | None = None) -> str:
    n = limit() if n is None else n
    return "no limit" if n is None else f"{n / (1 << 30):g} GiB"


def _lowered(n: int, current: tuple[int, int]) -> tuple[int, int]:
    """``(soft, hard)`` at most ``n``, never above a lower limit already set."""
    soft, hard = current
    inf = resource.RLIM_INFINITY
    new = n if hard == inf else min(n, hard)
    if soft != inf:
        new = min(new, soft)
    return new, hard


def child_setup() -> Callable[[], None] | None:
    """The function a new process runs before it executes its program (``preexec_fn``): its
    address space limited to :func:`limit`, the hard limit too, so neither it nor what it
    starts can raise it. ``None`` when there is no limit (or no ``resource`` module)."""
    n = limit()
    if n is None or resource is None:
        return None

    def apply() -> None:
        soft, _hard = _lowered(n, resource.getrlimit(resource.RLIMIT_AS))
        resource.setrlimit(resource.RLIMIT_AS, (soft, soft))

    return apply


@contextlib.contextmanager
def limited() -> Iterator[int | None]:
    """This process's soft address-space limit lowered to :func:`limit` inside the block
    (larmorx's in-process runs of the cases that expect a refused allocation); the previous
    limits come back after it. The process's own address space counts too (a few GiB for the
    harness), so the margin below :data:`DEFAULT` is kept large."""
    n = limit()
    if n is None or resource is None:
        yield None
        return
    before = resource.getrlimit(resource.RLIMIT_AS)
    soft, hard = _lowered(n, before)
    resource.setrlimit(resource.RLIMIT_AS, (soft, hard))
    try:
        yield soft
    finally:
        resource.setrlimit(resource.RLIMIT_AS, before)


def current() -> tuple[int, int] | None:
    """This process's ``(soft, hard)`` address-space limits (``None`` without ``resource``)."""
    return None if resource is None else resource.getrlimit(resource.RLIMIT_AS)


__all__ = [
    "DEFAULT",
    "ENV",
    "child_setup",
    "current",
    "describe",
    "for_memory",
    "limit",
    "limited",
    "parse",
]
