# SPDX-License-Identifier: Apache-2.0
"""An fMRIPrep oracle run as ``scripts/run_fmriprep_oracle.sh`` leaves it, read from text files.

A run directory holds ``RUN_INFO.txt`` (image, digest, version, arguments, threads), ``out/``
(the derivatives), ``work/`` (nipype's working directory) and ``fmriprep.log``. Every nipype
node directory in ``work/`` has ``_report/report.rst``, which lists the node's inputs and
outputs, its run time and its environment; command-line nodes also have ``command.txt``, and
every node its input hash file ``_0x<hash>.json`` (the inputs as they were before nipype
copied or linked any file into the node directory). Only these text files are read, never
nipype's pickles.

fMRIPrep ran in a container, so the paths it recorded are the container's (``/data``,
``/out``, ``/work``, ``/templateflow``). :class:`OracleRun` maps them back to this machine.
"""

from __future__ import annotations

import os
import re
import shlex
from collections.abc import Iterator
from dataclasses import dataclass, field
from functools import cached_property
from pathlib import Path

#: The bind mounts of ``scripts/run_fmriprep_oracle.sh``: container path -> where it is on
#: the host, relative to the run directory (``None``: from RUN_INFO.txt or the workspace).
CONTAINER_MOUNTS = ("/data", "/out", "/work", "/templateflow")


@dataclass(frozen=True)
class Node:
    """One nipype node (or one map-node item) of the working directory."""

    #: The node directory, relative to the working directory (``work/``).
    path: str
    #: The node's name (its directory name; map-node items keep ``_name<i>``).
    name: str
    #: The interface's module, as nipype's report names it (``ants``, ``resampling``, ...).
    interface: str
    inputs: dict[str, str]
    outputs: dict[str, str]
    runtime: dict[str, str]
    environment: dict[str, str]
    #: ``command.txt``, when the node ran a program.
    command: str | None = None
    #: The inputs of the node's hash file ``_0x<hash>.json``: files as their original paths
    #: (nipype's report shows a file it linked into the node directory, which it removes
    #: after the run, at its place in the node directory).
    hashed: dict[str, object] = field(default_factory=dict)

    @property
    def program(self) -> str | None:
        """The program a command-line node ran (the first word of its command)."""
        if not self.command:
            return None
        return os.path.basename(self.command.split()[0])

    @property
    def argv(self) -> list[str]:
        """The command split as nipype's shell would split it."""
        return shlex.split(self.command or "")

    @property
    def duration(self) -> float | None:
        """The run time nipype recorded (seconds)."""
        try:
            return float(self.runtime["duration"])
        except (KeyError, ValueError):
            return None

    @property
    def threads(self) -> int:
        """The threads the node's program could use: ANTs/ITK read ``NSLOTS`` (nipype's
        ``num_threads``), Python interfaces their ``num_threads`` input; else 1."""
        for value in (self.environment.get("NSLOTS"), self.inputs.get("num_threads")):
            try:
                n = int(str(value))
            except (TypeError, ValueError):
                continue
            if n > 0:
                return n
        return 1


def _sections(text: str) -> dict[str, list[str]]:
    """The ``* key : value`` lines of each section of a nipype ``report.rst``."""
    out: dict[str, list[str]] = {}
    lines = text.splitlines()
    current = ""
    for i, line in enumerate(lines):
        underline = lines[i + 1] if i + 1 < len(lines) else ""
        if line.strip() and re.fullmatch(r"[-~=]{3,}", underline.strip() or "x"):
            current = line.strip()
            out.setdefault(current, [])
            continue
        if line.startswith("* "):
            out.setdefault(current, []).append(line[2:])
    return out


def _items(lines: list[str]) -> dict[str, str]:
    items = {}
    for line in lines:
        key, sep, value = line.partition(" : ")
        if sep:
            items[key.strip()] = value.strip()
        elif line.rstrip().endswith(" :"):
            items[line.rstrip()[:-2].strip()] = ""
    return items


def _hashed_inputs(directory: Path) -> dict[str, object]:
    """The inputs of a node's hash file (``_0x<hash>.json``): ``[[name, value], ...]``, a
    file as ``[path, hash]``."""
    import json

    files = sorted(directory.glob("_0x*.json"))
    if not files:
        return {}
    try:
        pairs = json.loads(files[0].read_text(encoding="utf-8"))
    except ValueError:
        return {}
    out: dict[str, object] = {}
    for item in pairs:
        if isinstance(item, list) and len(item) == 2 and isinstance(item[0], str):
            name, value = item
            if (
                isinstance(value, list)
                and len(value) == 2
                and isinstance(value[0], str)
                and value[0].startswith("/")
                and isinstance(value[1], str)
            ):
                value = value[0]
            out[name] = value
    return out


def read_node(work: Path, directory: Path) -> Node:
    """The node in ``directory`` (which has ``_report/report.rst``)."""
    text = (directory / "_report" / "report.rst").read_text(encoding="utf-8", errors="replace")
    header = text.splitlines()[0] if text else ""
    interface = header.rsplit("(", 1)[-1].strip().rstrip(")").strip() if "(" in header else ""
    if interface == directory.name:
        interface = ""  # the report names no module (e.g. fMRIPrep's DataSinks)
    sections = _sections(text)
    command_file = directory / "command.txt"
    command = None
    if command_file.is_file():
        command = command_file.read_text(encoding="utf-8", errors="replace").strip() or None
    runtime = _items(sections.get("Runtime info", []))
    if command is None and runtime.get("cmdline"):
        command = runtime["cmdline"]
    return Node(
        path=directory.relative_to(work).as_posix(),
        name=directory.name,
        interface=interface,
        inputs=_items(sections.get("Execution Inputs", [])),
        outputs=_items(sections.get("Execution Outputs", [])),
        runtime=runtime,
        environment=_items(sections.get("Environment", [])),
        command=command,
        hashed=_hashed_inputs(directory),
    )


def _run_info(path: Path) -> dict[str, str]:
    info: dict[str, str] = {}
    if not path.is_file():
        return info
    for line in path.read_text(encoding="utf-8").splitlines():
        key, sep, value = line.partition(": ")
        if sep and key and not key.startswith(" "):
            info.setdefault(key.strip(), value.strip())
    return info


def _workspace(start: Path) -> Path | None:
    """The workspace root: the directory above ``oracles/``."""
    for parent in [start, *start.parents]:
        if parent.name == "oracles":
            return parent.parent
    return None


@dataclass
class OracleRun:
    """An fMRIPrep oracle run directory."""

    root: Path
    #: Extra or overriding mounts: container path -> host path.
    binds: dict[str, Path] = field(default_factory=dict)

    def __post_init__(self) -> None:
        self.root = Path(self.root).resolve()
        if not (self.root / "work").is_dir():
            raise FileNotFoundError(f"{self.root} has no work/ directory")

    # -- what RUN_INFO.txt says ---------------------------------------------------------------

    @cached_property
    def info(self) -> dict[str, str]:
        return _run_info(self.root / "RUN_INFO.txt")

    @property
    def name(self) -> str:
        return self.root.name

    @property
    def version(self) -> str:
        """fMRIPrep's version (``25.2.6``), from RUN_INFO.txt or the work directory's name."""
        m = re.search(r"v?(\d+\.\d+\.\d+\S*)", self.info.get("fmriprep", ""))
        if m:
            return m.group(1)
        for d in (self.root / "work").iterdir():
            m = re.fullmatch(r"fmriprep_(\d+)_(\d+)_wf", d.name)
            if m:
                return f"{m.group(1)}.{m.group(2)}"
        return "unknown"

    @property
    def image(self) -> Path | None:
        """The container image file (Apptainer ``.sif``)."""
        m = re.match(r"(\S+\.sif)", self.info.get("image file", ""))
        if m and Path(m.group(1)).is_file():
            return Path(m.group(1))
        ws = _workspace(self.root)
        if ws is not None:
            candidate = ws / "oracles" / "images" / f"fmriprep-{self.version}.sif"
            if candidate.is_file():
                return candidate
        return None

    @property
    def digest(self) -> str:
        m = re.search(r"@(sha256:[0-9a-f]+)", self.info.get("image", ""))
        return m.group(1) if m else "unknown"

    @property
    def arguments(self) -> str:
        return self.info.get("arguments", "")

    # -- paths --------------------------------------------------------------------------------

    @cached_property
    def mounts(self) -> dict[str, Path]:
        """Container path -> host path, for every mount of the run."""
        m: dict[str, Path] = {"/out": self.root / "out", "/work": self.root / "work"}
        if bids := self.info.get("BIDS input"):
            m["/data"] = Path(bids)
        ws = _workspace(self.root)
        if ws is not None and (ws / "oracles" / "templateflow").is_dir():
            m["/templateflow"] = ws / "oracles" / "templateflow"
        m.update({k.rstrip("/"): Path(v) for k, v in self.binds.items()})
        return m

    def host(self, container_path: str) -> str:
        """``container_path`` on this machine (unchanged when no mount covers it)."""
        for mount in sorted(self.mounts, key=len, reverse=True):
            if container_path == mount or container_path.startswith(mount + "/"):
                return str(self.mounts[mount]) + container_path[len(mount) :]
        return container_path

    def host_text(self, text: str) -> str:
        """Every container path inside ``text`` (an argument such as ``[ a.mat, 1 ]`` or
        ``Mattes[a,b,32]``) replaced by its host path."""
        mounts = sorted(self.mounts, key=len, reverse=True)
        pattern = "|".join(re.escape(m) for m in mounts)
        return re.sub(
            rf"(?<![\w./-])({pattern})(?=/|$|[\s,\]])",
            lambda mt: str(self.mounts[mt.group(1)]),
            text,
        )

    @property
    def work(self) -> Path:
        return self.root / "work"

    def node_dir(self, node: Node) -> Path:
        return self.work / node.path

    # -- nodes --------------------------------------------------------------------------------

    def nodes(self) -> Iterator[Node]:
        """Every node with a report, in path order."""
        for report in sorted(self.work.rglob("_report/report.rst")):
            yield read_node(self.work, report.parent.parent)

    @cached_property
    def all_nodes(self) -> list[Node]:
        return list(self.nodes())

    def derivatives(self) -> dict[str, str]:
        """Work-directory file -> derivative written from it (from the DataSink nodes'
        ``in_file`` and ``out_file``), both as host paths."""
        out: dict[str, str] = {}
        for node in self.all_nodes:
            ins = _paths(node.inputs.get("in_file", ""))
            outs = [o for o in _paths(node.outputs.get("out_file", "")) if o.startswith("/out/")]
            if not ins or not outs or len(ins) != len(outs):
                continue
            for i, o in zip(ins, outs, strict=False):
                out[self.host(i)] = self.host(o)
        return out


def _paths(value: str) -> list[str]:
    """The file paths in a report value (``'/a'`` or ``['/a', '/b']``)."""
    return re.findall(r"(/[^'\",\]\s]+)", value)


__all__ = ["CONTAINER_MOUNTS", "Node", "OracleRun", "read_node"]
