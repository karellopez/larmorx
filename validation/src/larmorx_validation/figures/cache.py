# SPDX-License-Identifier: Apache-2.0
"""The figures cache: outputs of the original programs and of larmorx on real data, outside
git (``<workspace>/oracles/figures-cache/<tool>/<case>/``).

Each entry is a directory holding the outputs and ``RUN_INFO.json``: every command with its
exit status, environment settings, threads and seed, the program versions, the larmorx commit
and the inputs' checksums. An entry is complete when ``RUN_INFO.json`` exists (it is written
last); a figure that needs a missing entry computes it, so the figures command recomputes
what is missing and reuses the rest.
"""

from __future__ import annotations

import datetime as dt
import json
import os
import re
import shlex
import subprocess
import sys
import time
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from pathlib import Path

from larmorx_validation.figures.core import Context, git_commit, sha256


@dataclass
class Run:
    """One command of an entry, as it ran."""

    label: str
    argv: list[str]
    env: dict[str, str] = field(default_factory=dict)
    exit: int | None = None
    seconds: float = 0.0
    stderr_tail: str = ""


class Entry:
    """A cache entry being computed: run commands into it, then :meth:`finish`."""

    def __init__(self, ctx: Context, tool: str, case: str, description: str):
        self.ctx = ctx
        self.dir = ctx.cache / tool / case
        self.tool, self.case, self.description = tool, case, description
        self.runs: list[Run] = []
        self.inputs: dict[str, str] = {}
        self.extra: dict = {}

    @property
    def info_path(self) -> Path:
        return self.dir / "RUN_INFO.json"

    @property
    def done(self) -> bool:
        return self.info_path.is_file()

    def info(self) -> dict:
        return json.loads(self.info_path.read_text(encoding="utf-8"))

    def start(self) -> None:
        self.dir.mkdir(parents=True, exist_ok=True)
        if self.info_path.exists():
            self.info_path.unlink()

    def input(self, label: str, path: Path) -> Path:
        self.inputs[label] = f"{path} (sha256 {sha256(path)})"
        return path

    def run(
        self,
        label: str,
        argv: Sequence[str],
        *,
        env: dict[str, str] | None = None,
        unset: Sequence[str] = (),
        check: bool = True,
        timeout: float = 4 * 3600,
    ) -> Run:
        """Run ``argv`` with ``env`` added to (and ``unset`` removed from) this environment."""
        full = {k: v for k, v in os.environ.items() if k not in unset}
        full.update(env or {})
        run = Run(label, [str(a) for a in argv], dict(env or {}))
        self.ctx.log(f"  {self.tool}/{self.case}: {label}")
        t0 = time.perf_counter()
        p = subprocess.run(
            run.argv,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=full,
            timeout=timeout,
        )
        run.seconds = round(time.perf_counter() - t0, 2)
        run.exit = p.returncode
        run.stderr_tail = p.stderr[-2000:]
        self.runs.append(run)
        (self.dir / f"{label}.log").write_text(
            f"$ {shlex.join(run.argv)}\n# env: {run.env}\n# exit {p.returncode}, "
            f"{run.seconds} s\n--- stdout\n{p.stdout}\n--- stderr\n{p.stderr}",
            encoding="utf-8",
        )
        if check and p.returncode != 0:
            raise RuntimeError(f"{label} exited with {p.returncode}:\n{p.stderr[-2000:]}")
        return run

    def finish(self, **extra) -> dict:
        self.extra.update(extra)
        info = {
            "tool": self.tool,
            "case": self.case,
            "description": self.description,
            "computed": dt.datetime.now(dt.UTC).isoformat(timespec="seconds"),
            "larmorx_commit": git_commit(self.ctx.repo),
            "python": sys.version.split()[0],
            "inputs": self.inputs,
            "runs": [r.__dict__ for r in self.runs],
            **self.extra,
        }
        self.info_path.write_text(json.dumps(info, indent=1) + "\n", encoding="utf-8")
        return info


def entry(
    ctx: Context, tool: str, case: str, description: str, compute: Callable[[Entry], None]
) -> Entry:
    """The cache entry ``tool/case``, computed by ``compute(entry)`` if it is not there yet."""
    e = Entry(ctx, tool, case, description)
    if e.done:
        return e
    if not ctx.compute:
        raise FileNotFoundError(f"figures cache entry {tool}/{case} is missing (--no-compute)")
    ctx.log(f"computing {tool}/{case} into {e.dir}")
    e.start()
    compute(e)
    if not e.done:
        e.finish()
    return e


def provenance(e: Entry) -> dict:
    """How a figure's provenance names a cache entry (paths relative to the cache)."""
    info = e.info()
    return {
        "entry": f"{e.tool}/{e.case}",
        "description": info.get("description"),
        "computed": info.get("computed"),
        "larmorx_commit": info.get("larmorx_commit"),
        "commands": [
            {
                "label": r["label"],
                "argv": [_short(a) for a in r["argv"]],
                "env": {k: _short(str(v)) for k, v in r["env"].items()},
                "exit": r["exit"],
                "seconds": r["seconds"],
            }
            for r in info.get("runs", [])
        ],
        **{k: v for k, v in info.items() if k in ("versions", "threads", "seed", "settings")},
    }


#: A larmorx checkout inside a path: a clone named ``larmorx`` or a worktree
#: (``.claude/worktrees/<name>``), followed by one of the repository's own directories.
_CHECKOUT = re.compile(
    r"(?<![\w.~<>-])/(?:[^\s,\[\]\"'=@:;]+/)*?(?:larmorx|\.claude/worktrees/[^/\s,\[\]\"'=@:;]+)"
    r"(?=/(?:crates-gpl|crates|target|python|validation|scripts|tests)/)"
)


def checkout_paths(text: str) -> str:
    """``text`` with any larmorx checkout's path replaced by ``<larmorx>``: a program built in
    a checkout (the replica, ``crates-gpl/target/release/larmorx-gpl``) or a file of one is
    named alike whichever checkout or worktree ran it, so a re-export from another checkout
    gives the same bytes."""
    return _CHECKOUT.sub("<larmorx>", text)


def _short(arg: str) -> str:
    """A path with the machine-specific part replaced (provenance is published)."""
    from larmorx_validation.report import redact

    home = str(Path.home())
    out = checkout_paths(arg)
    # the workspace before the home directory (``redact`` replaces that by ``~``)
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            out = out.replace(str(parent) + "/", "<workspace>/")
            break
    return redact(out).replace(home, "~")


def larmorx_cli(*args: str) -> list[str]:
    """``larmorx`` run by this Python (the checkout's package, as the parity suites run it)."""
    return [sys.executable, "-m", "larmorx", *args]
