# SPDX-License-Identifier: Apache-2.0
"""A recorded recon-all run: its steps, the subject directory before each, its environment.

A recording is what ``validation/investigations/freesurfer-oracle/run_recon.sh`` leaves (the
README there describes it)::

    <run>/RUN_INFO.txt                     what ran, with what; its last line "EXIT <status>"
    <run>/record/steps/NNNN-<program>.json one top-level step of recon-all
    <run>/record/blobs/<aa>/<sha256>       every file's content, once
    <run>/subjects/<id>/                   the finished subject directory

A step's file holds its command line (``argv``), working directory, six environment variables,
times, exit status, the subject directory's state before it (``before``: path -> SHA-256), the
files it wrote with new content (``outputs``) and deleted, and what changed between the previous
step and this one (``between``: commands recon-all ran without ``fs_time``). The state after a
step is ``before`` with ``outputs`` applied and ``deleted`` removed (:meth:`Step.after`).

Nothing here reads FreeSurfer's source: the layout is the recorder's own
(``fs_time_recorder.py``).
"""

from __future__ import annotations

import functools
import json
import os
import re
from dataclasses import dataclass, field
from pathlib import Path

#: What the recorder leaves out of the subject directory (``fs_time_recorder.py``): the top-level
#: ``tmp/``, recon-all's own logs and markers, which change at every step.
SKIP_DIRS = ("tmp",)
SKIP_FILES = frozenset(
    {
        "scripts/recon-all.log",
        "scripts/recon-all-status.log",
        "scripts/recon-all.cmd",
        "scripts/recon-all.done",
        "scripts/recon-all.error",
        "scripts/recon-all.local-copy",
        "scripts/build-stamp.txt",
    }
)
SKIP_PREFIX = "scripts/IsRunning"
#: The environment file recon-all writes into the subject directory at its start (a dump of its
#: environment after its own settings): the base of every step's environment in a replay.
ENV_FILE = "scripts/recon-all.env"
#: The pseudo-step the run script records after recon-all ends.
FINAL = "--larmorx-final"


def skipped(rel: str) -> bool:
    """Whether the recorder leaves the subject directory's file ``rel`` out."""
    first = rel.split("/", 1)[0]
    return (
        ("/" in rel and first in SKIP_DIRS) or rel in SKIP_FILES or rel.startswith(SKIP_PREFIX)
    )


@dataclass(frozen=True)
class Step:
    """One recorded step (``record/steps/NNNN-<program>.json``)."""

    index: int
    program: str
    argv: tuple[str, ...]
    cwd: str
    env: dict[str, str]
    start: float
    seconds: float
    exit: int
    before: dict[str, str]
    outputs: dict[str, str]
    deleted: tuple[str, ...]
    between: dict[str, str]
    between_deleted: tuple[str, ...]
    path: Path = field(compare=False, default=Path())

    @property
    def final(self) -> bool:
        return self.argv[:1] == (FINAL,)

    @property
    def label(self) -> str:
        return f"{self.index:04d}-{self.program}"

    def after(self) -> dict[str, str]:
        """The subject directory's state after the step: path -> SHA-256."""
        state = dict(self.before)
        state.update(self.outputs)
        for rel in self.deleted:
            state.pop(rel, None)
        return state

    @classmethod
    def load(cls, path: Path) -> Step:
        d = json.loads(path.read_text(encoding="utf-8"))
        argv = tuple(d.get("argv") or ())
        program = "final" if argv[:1] == (FINAL,) else (os.path.basename(argv[0]) if argv else "")
        return cls(
            index=int(d["index"]),
            program=program,
            argv=argv,
            cwd=d.get("cwd", ""),
            env=dict(d.get("env") or {}),
            start=float(d.get("start", 0.0)),
            seconds=float(d.get("seconds", 0.0)),
            exit=int(d.get("exit", 0)),
            before=dict(d.get("before") or {}),
            outputs=dict(d.get("outputs") or {}),
            deleted=tuple(d.get("deleted") or ()),
            between=dict(d.get("between") or {}),
            between_deleted=tuple(d.get("between_deleted") or ()),
            path=path,
        )


#: A variable line of ``recon-all.env``'s dump (``NAME=value``).
_ENV_LINE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=(.*)$")


def parse_env(text: str) -> dict[str, str]:
    """The variables of a ``recon-all.env`` file: its ``NAME=value`` lines (the last value of a
    name wins; the file lists recon-all's own settings again at its end). Lines of a multi-line
    value (an exported shell function) are not variables and are left out."""
    out: dict[str, str] = {}
    for line in text.splitlines():
        m = _ENV_LINE.match(line)
        if m:
            out[m.group(1)] = m.group(2)
    return out


class Recording:
    """A recorded run's directory (``<workspace>/oracles/freesurfer-8.2.0/runs/<run>/``)."""

    def __init__(self, root: Path | str):
        self.root = Path(root)
        self.record = self.root / "record"
        self.blobs = self.record / "blobs"
        if not (self.record / "steps").is_dir():
            raise FileNotFoundError(f"no recording in {self.root} (record/steps/ is missing)")

    @property
    def name(self) -> str:
        return self.root.name

    @functools.cached_property
    def info(self) -> dict[str, str]:
        """``RUN_INFO.txt``'s ``key: value`` lines (``EXIT`` as ``exit``)."""
        out: dict[str, str] = {}
        p = self.root / "RUN_INFO.txt"
        if not p.is_file():
            return out
        for line in p.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("EXIT "):
                out["exit"] = line[5:].strip()
            elif ": " in line:
                k, v = line.split(": ", 1)
                out.setdefault(k.strip(), v.strip())
        return out

    @functools.cached_property
    def steps(self) -> list[Step]:
        return [Step.load(p) for p in sorted((self.record / "steps").glob("*.json"))]

    def step(self, index: int) -> Step:
        for s in self.steps:
            if s.index == index:
                return s
        raise KeyError(f"{self.name} has no step {index}")

    def select(self, text: str) -> Step:
        """A step by its number (``11``, ``0011``) or by program and occurrence (``mri_normalize``,
        ``mri_normalize:2``: the second step running it)."""
        if text.isdigit():
            return self.step(int(text))
        program, _, nth = text.partition(":")
        n = int(nth) if nth else 1
        found = [s for s in self.steps if s.program == program]
        if len(found) < n:
            raise KeyError(f"{self.name} has {len(found)} steps of {program}, not {n}")
        return found[n - 1]

    def blob(self, sha: str) -> Path:
        return self.blobs / sha[:2] / sha

    @property
    def subject(self) -> str:
        if "subject" in self.info:
            return self.info["subject"]
        return os.path.basename(self.subject_dir)

    @functools.cached_property
    def subjects_dir(self) -> str:
        """The recorded ``SUBJECTS_DIR`` (an absolute path on the machine that ran it)."""
        for s in self.steps:
            if "SUBJECTS_DIR" in s.env:
                return s.env["SUBJECTS_DIR"]
        return str(self.root / "subjects")

    @property
    def subject_dir(self) -> str:
        return f"{self.subjects_dir}/{self.info.get('subject', '')}".rstrip("/")

    @property
    def stream(self) -> str:
        """``classic`` for a ``-no-v8`` run, else ``v8`` (8.2.0's default)."""
        return "classic" if " -no-v8" in self.info.get("command", "") else "v8"

    @functools.cached_property
    def environment(self) -> dict[str, str]:
        """recon-all's environment as its ``scripts/recon-all.env`` recorded it (empty if the
        recording lacks it)."""
        for s in self.steps:
            sha = s.before.get(ENV_FILE) or s.outputs.get(ENV_FILE)
            if sha and self.blob(sha).is_file():
                return parse_env(self.blob(sha).read_text(encoding="utf-8", errors="replace"))
        return {}

    @functools.cached_property
    def links(self) -> dict[str, str]:
        """The symbolic links of the finished subject directory: path -> the link's text. The
        recorder stores a link as the file it names; a rebuild makes the links again where the
        state allows it (:func:`larmorx_validation.freesurfer.replay.rebuild`)."""
        root = self.root / "subjects" / self.subject
        out: dict[str, str] = {}
        if not root.is_dir():
            return out
        for d, dirs, files in os.walk(root):
            for name in dirs + files:
                p = Path(d) / name
                if p.is_symlink():
                    out[p.relative_to(root).as_posix()] = os.readlink(p)
        return out

    @functools.cached_property
    def born(self) -> list[tuple[float, str, str, str]]:
        """The finished ``subjects/`` tree's directories and links with their birth times
        (``(seconds, "d" or "l", path below subjects/, link text)``; GNU find's ``%B``, which
        Linux reads with statx: a directory keeps its birth time, so the directories that
        existed before a step are those born before it started). Birth time 0: unknown."""
        import subprocess

        root = self.root / "subjects"
        if not root.is_dir():
            return []
        p = subprocess.run(
            ["find", str(root), "-mindepth", "1", "(", "-type", "d", "-o", "-type", "l", ")",
             "-printf", r"%B@\t%y\t%P\t%l\n"],
            capture_output=True, text=True, check=False,
        )  # fmt: skip
        out = []
        for line in p.stdout.splitlines():
            parts = line.split("\t")
            if len(parts) != 4:
                continue
            try:
                t = float(parts[0])
            except ValueError:
                t = 0.0
            out.append((t, parts[1], parts[2], parts[3]))
        return out


__all__ = [
    "ENV_FILE",
    "FINAL",
    "SKIP_DIRS",
    "SKIP_FILES",
    "Recording",
    "Step",
    "parse_env",
    "skipped",
]
