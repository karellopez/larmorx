# SPDX-License-Identifier: Apache-2.0
"""Memory limits for random cases: every process a case runs is held in a memory box.

On 2026-10-25 one random N4BiasFieldCorrection case of the first night grew to 28 GB resident:
the machine swapped until the editor, and every agent running in it, went down. So a night runs
in a box of its own (``scripts/orchestration/boxed.sh``, which ``difftest-nightly.sh`` uses: a
systemd user scope with a hard memory limit and no swap), and inside that box each worker, with
every original program and larmorx run it makes, is held in a control group of its own with a
hard limit (``--memory-gib``, 3 GiB by default, many times what an input of at most 32^3 voxels
needs) and no swap. The kernel stops the process that exceeds it; the case is recorded with the
verdict ``memory`` ("the original N4BiasFieldCorrection exceeds 3 GiB", "larmorx exceeds 3
GiB"), and the night goes on with a fresh worker. Time is limited as before: each run by an
alarm, each case by its deadline.

How (cgroup v2, which a systemd user scope gives without privileges): the box is the night's
own control group, recognised by its finite ``memory.max``. :meth:`MemoryBox.nest` moves the
processes it holds into a leaf ``harness`` and enables the memory controller below it;
:meth:`MemoryBox.leaf` makes one leaf per worker with ``memory.max`` and ``memory.swap.max = 0``.
A worker enters its leaf before it runs anything, so every process it starts is held there.
Whether a process of a leaf was stopped is read from the leaf's ``memory.events`` (``oom_kill``,
counted whichever limit, the leaf's or the box's, was reached); which one, from the exits this
module records (:func:`watch_exits`): a killed child is the original program (or a larmorx
program run as a separate process, the GPL and FSL replicas); a killed worker is larmorx running
in it.
"""

from __future__ import annotations

import contextlib
import os
import signal
import subprocess
from dataclasses import dataclass
from pathlib import Path

GIB = 1 << 30
#: The default limit of each worker (and so of each case), in GiB.
CASE_GIB = 3.0
#: The environment variable naming the leaf a process and its children run in (so a child
#: harness, :func:`~larmorx_validation.difftest.runner.isolated_judge`, reads the same counters).
ENV = "LARMORX_DIFFTEST_CGROUP"
CGROUP_ROOT = Path("/sys/fs/cgroup")

MEMORY = "memory"


def own_cgroup() -> Path | None:
    """This process's cgroup v2 directory, or ``None`` (cgroup v1, not Linux)."""
    try:
        text = Path("/proc/self/cgroup").read_text(encoding="utf-8")
    except OSError:
        return None
    for line in text.splitlines():
        if line.startswith("0::"):
            path = CGROUP_ROOT / line[3:].lstrip("/")
            return path if path.is_dir() else None
    return None


def _limit_of(path: Path) -> int | None:
    try:
        value = (path / "memory.max").read_text(encoding="utf-8").strip()
    except OSError:
        return None
    return None if value == "max" else int(value)


@dataclass(frozen=True)
class Leaf:
    """A worker's control group: a hard memory limit, no swap."""

    path: Path
    limit: int

    def enter(self) -> None:
        """Moves the calling process (and so everything it starts) into this leaf."""
        (self.path / "cgroup.procs").write_text("0", encoding="ascii")
        os.environ[ENV] = f"{self.path}:{self.limit}"

    def oom_kills(self) -> int:
        """How many of this leaf's processes the kernel stopped for memory, so far."""
        try:
            text = (self.path / "memory.events").read_text(encoding="ascii")
        except OSError:
            return 0
        for line in text.splitlines():
            key, _, value = line.partition(" ")
            if key == "oom_kill":
                return int(value)
        return 0

    def peak(self) -> int | None:
        """The leaf's largest memory use since it was made (``memory.peak``, Linux 5.19)."""
        try:
            return int((self.path / "memory.peak").read_text(encoding="ascii"))
        except (OSError, ValueError):
            return None

    def remove(self) -> None:
        with contextlib.suppress(OSError):
            self.path.rmdir()


@dataclass(frozen=True)
class MemoryBox:
    """The night's own control group, a box with a finite ``memory.max``."""

    root: Path
    limit: int

    @classmethod
    def find(cls) -> MemoryBox | None:
        """The box this process runs in (its own cgroup, or the parent of its ``harness`` leaf
        once nested), or ``None`` when it runs in none."""
        own = own_cgroup()
        if own is None:
            return None
        for path in (own.parent, own) if own.name == "harness" else (own,):
            limit = _limit_of(path)
            if limit is not None and os.access(path / "cgroup.subtree_control", os.W_OK):
                return cls(path, limit)
        return None

    def nest(self) -> None:
        """Moves the box's own processes into its leaf ``harness`` and enables the memory
        controller for the leaves (a cgroup v2 group with controllers for its children holds
        no processes itself)."""
        harness = self.root / "harness"
        harness.mkdir(exist_ok=True)
        procs = (self.root / "cgroup.procs").read_text(encoding="ascii").split()
        for pid in procs:
            with contextlib.suppress(ProcessLookupError, OSError):
                (harness / "cgroup.procs").write_text(pid, encoding="ascii")
        control = self.root / "cgroup.subtree_control"
        if "memory" not in control.read_text(encoding="ascii").split():
            control.write_text("+memory", encoding="ascii")

    def leaf(self, name: str, limit: int, swap: int = 0) -> Leaf:
        """A leaf of the box with a hard limit of ``limit`` bytes and ``swap`` bytes of swap
        (none by default; a heavy case's leaf may swap)."""
        path = self.root / name
        path.mkdir(exist_ok=True)
        (path / "memory.max").write_text(str(int(limit)), encoding="ascii")
        swap_max = path / "memory.swap.max"
        if swap_max.exists():
            swap_max.write_text(str(int(swap)), encoding="ascii")
        return Leaf(path, int(limit))


#: The default size of the box a command makes for itself when it runs in none, in GiB.
BOX_GIB = 8.0
#: Set in a command re-run inside a box of its own (so it never boxes itself twice).
BOXED = "LARMORX_DIFFTEST_BOXED"


#: The box of the heavy cases (``difftest --heavy``): memory and swap, in GiB (user,
#: 2026-10-25: heavy cases are run to their end, one at a time, when the machine is quiet).
HEAVY_GIB = 24.0
HEAVY_SWAP_GIB = 48.0


def ensure_box(argv: list[str], box_gib: float = BOX_GIB, swap_gib: float = 0.0) -> None:
    """Re-runs ``python -m larmorx_validation <argv>`` in a memory box of ``box_gib`` GiB and
    ``swap_gib`` GiB of swap (a systemd user scope, as ``scripts/orchestration/boxed.sh`` makes,
    which allows no swap) when this process runs in none; returns when it already does."""
    import shutil
    import sys

    if MemoryBox.find() is not None:
        return
    if os.environ.get(BOXED):
        raise SystemExit(
            "the memory box was not found after systemd-run made it (cgroup v2 not writable?); "
            "--memory-gib 0 runs without limits"
        )
    exe = shutil.which("systemd-run")
    if exe is None:
        raise SystemExit(
            "no memory box and no systemd-run: run it with scripts/orchestration/boxed.sh, or "
            "pass --memory-gib 0 to run without limits"
        )
    command = [
        exe,
        "--user",
        "--scope",
        "--quiet",
        "--collect",
        "-p",
        f"MemoryMax={int(box_gib * GIB)}",
        "-p",
        f"MemorySwapMax={int(swap_gib * GIB)}",
        "--",
        sys.executable,
        "-m",
        "larmorx_validation",
        *argv,
    ]
    sys.stdout.flush()
    sys.stderr.flush()
    os.execve(exe, command, {**os.environ, BOXED: "1"})


def current() -> Leaf | None:
    """The leaf this process was put in (:data:`ENV`), or ``None``."""
    value = os.environ.get(ENV, "")
    path, _, limit = value.rpartition(":")
    if not path or not limit.isdigit() or not Path(path).is_dir():
        return None
    return Leaf(Path(path), int(limit))


def gib(limit: int) -> str:
    """``3 GiB``, ``0.25 GiB``."""
    return f"{limit / GIB:g} GiB"


# ------------------------------------------------------------------------------------------------
# Which process the kernel stopped

#: The programs (by name) that ended with SIGKILL since the last :func:`clear`.
_KILLED: list[str] = []


def _program(args) -> str:
    words = [str(args)] if isinstance(args, (str, bytes, os.PathLike)) else [str(a) for a in args]
    name = Path(words[0]).name if words else "?"
    if name.startswith("python") and "-m" in words[:-1]:
        # python -m larmorx ...: larmorx run as a separate process.
        return words[words.index("-m") + 1]
    if name == "apptainer":
        # apptainer exec [flags] <image> <program> ...: the program after the image.
        for i, w in enumerate(words):
            if w.endswith((".sif", ".simg")) and i + 1 < len(words):
                return Path(words[i + 1]).name
    return name


def watch_exits() -> None:
    """Records every child process of this process that ends with SIGKILL (the kernel's memory
    limit, or a deadline), by its program's name."""
    if getattr(subprocess.Popen, "_larmorx_watched", False):
        return
    handle = subprocess.Popen._handle_exitstatus

    def _handle_exitstatus(self, *args, **kw):
        handle(self, *args, **kw)
        if self.returncode == -signal.SIGKILL:
            _KILLED.append(_program(self.args))

    subprocess.Popen._handle_exitstatus = _handle_exitstatus
    subprocess.Popen._larmorx_watched = True


def clear() -> None:
    _KILLED.clear()


def is_larmorx(program: str) -> bool:
    """``larmorx``, ``larmorx-gpl``, ``larmorx-nc``, ``lx``: larmorx's own programs."""
    return program == "lx" or program.startswith("larmorx")


def culprit() -> str:
    """Who the kernel stopped in this process's leaf, while this process survived: the original
    program, or a larmorx program run as a separate process."""
    for program in reversed(_KILLED):
        return "larmorx" if is_larmorx(program) else f"the original {program}"
    return "a process of the case"


def exceeded(who: str, limit: int) -> str:
    return f"{who} exceeds {gib(limit)}"


__all__ = [
    "BOXED",
    "BOX_GIB",
    "CASE_GIB",
    "ENV",
    "GIB",
    "HEAVY_GIB",
    "HEAVY_SWAP_GIB",
    "MEMORY",
    "Leaf",
    "MemoryBox",
    "clear",
    "culprit",
    "current",
    "ensure_box",
    "exceeded",
    "gib",
    "own_cgroup",
    "watch_exits",
]
