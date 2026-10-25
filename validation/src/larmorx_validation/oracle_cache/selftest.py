# SPDX-License-Identifier: Apache-2.0
"""A fake parity suite for the cache's own tests (``validation/tests/test_oracle_cache.py``):
its cases run a given program through the cache, in worker processes as ``parity --jobs``
does, so they must be importable here, not from the tests' module."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

from larmorx_validation.oracle_cache import ENV_LOCATION, Policy, run
from larmorx_validation.parity import harness


def env_for(**extra: str) -> dict[str, str]:
    """This process's environment without the fake program's variables, plus ``extra``."""
    env = {k: v for k, v in os.environ.items() if not k.startswith("FAKE_")}
    env.update(extra)
    return env


def fake_suite(program: str, n: int = 4) -> harness.Suite:
    """``n`` cases, each running ``program IN OUT`` on an input of its own."""

    def cases(tier: str) -> list[harness.Case]:
        return [harness.Case(f"c{i}", "test", "a fake case", i) for i in range(n)]

    def run_case(case: harness.Case, checks: harness.CheckList) -> None:
        with tempfile.TemporaryDirectory(prefix="lx-fake-case-") as t:
            src = Path(t) / "in.nii"
            src.write_bytes(bytes([case.payload]) * 10)
            dst = Path(t) / "out.nii"
            r = run([program, src, dst], env=env_for(), policy=Policy(threads=1, family="itk"))
            checks("ran", r.returncode == 0)
            checks("output", dst.read_bytes() == bytes([case.payload]) * 10)

    return harness.Suite("fake", "Fake", "fake", "fake", [], cases, run_case)


class Factory:
    """A picklable ``factory`` for ``harness.run(jobs=...)``: the fake suite, in a worker that
    uses the cache at ``root``."""

    def __init__(self, program: str, root: str, n: int = 4):
        self.program, self.root, self.n = program, root, n

    def __call__(self) -> harness.Suite:
        os.environ[ENV_LOCATION] = self.root
        return fake_suite(self.program, self.n)


__all__ = ["Factory", "env_for", "fake_suite"]
