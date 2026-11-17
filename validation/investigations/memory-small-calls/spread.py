# SPDX-License-Identifier: Apache-2.0
"""The run-to-run spread of a small call's peak through the Python entry point, and how often
``tests/parity/test_memory_peak.py``'s rule fails on it with one run of each side or with
several (README.md).

    python -P spread.py <n> <original_mib> [--flirt-env] -- <larmorx's call ...>

Runs ``python -m larmorx --version`` and the call alternately ``n`` times each (in a temporary
folder emptied before each pair), each under ``/usr/bin/time -v`` as the test measures them;
``--flirt-env``: in the environment the test gives FLIRT's calls (the FLIRT suite's
``_environment``, one OpenMP thread). Prints both sides' peaks (their counts), then for blocks
of ``k`` consecutive pairs (k = 1, 3, 5; the runs as the test makes them, one after the other)
the share of blocks in which the rule ``call <= 1.10 * original_mib + entry`` fails with each
estimate of a block: the least peak of each side, the median of each side, and the median of
the pairs' differences.
"""

from __future__ import annotations

import collections
import os
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path


def peak(cmd: list[str], env: dict[str, str], cwd: Path) -> float:
    p = subprocess.run(
        ["/usr/bin/time", "-v", *cmd], capture_output=True, text=True, env=env, cwd=cwd, check=False
    )
    m = re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
    assert m and p.returncode == 0, p.stderr[-400:]
    return int(m.group(1)) / 1024


def counts(v: list[float]) -> str:
    return "; ".join(f"{x:.2f} x{c}" for x, c in sorted(collections.Counter(v).items()))


def main() -> None:
    n, original = int(sys.argv[1]), float(sys.argv[2])
    call = sys.argv[sys.argv.index("--") + 1 :]
    env = dict(os.environ)
    if "--flirt-env" in sys.argv[: sys.argv.index("--")]:
        from larmorx_validation.parity import fsl_flirt_replica as fr

        env = {**fr._environment({}), "OMP_NUM_THREADS": "1", **{
            k: os.environ[k]
            for k in ("PYTHONPATH", "LARMORX_IMPLEMENTATION", "LARMORX_UPSTREAM_DEFECTS")
            if k in os.environ
        }}  # fmt: skip
    pairs = []
    with tempfile.TemporaryDirectory() as d:
        cwd = Path(d) / "run"
        for _ in range(n):
            shutil.rmtree(cwd, ignore_errors=True)
            cwd.mkdir()
            e = peak([sys.executable, "-m", "larmorx", "--version"], env, cwd)
            pairs.append((e, peak(call, env, cwd)))
    allowed = 1.10 * original
    print(f"entry peaks (MiB): {counts([e for e, _ in pairs])}")
    print(f"call peaks (MiB):  {counts([c for _, c in pairs])}")
    print(f"differences (MiB): {counts([c - e for e, c in pairs])}; allowed {allowed:.3f}")
    for k in (1, 3, 5):
        blocks = [pairs[i : i + k] for i in range(0, n - k + 1, k)]
        fails = collections.Counter()
        for b in blocks:
            es, cs = [e for e, _ in b], [c for _, c in b]
            fails["least of each"] += min(cs) - min(es) > allowed
            fails["median of each"] += statistics.median(cs) - statistics.median(es) > allowed
            fails["median difference"] += statistics.median(c - e for e, c in b) > allowed
        print(
            f"k = {k}, {len(blocks)} blocks: fails "
            + ", ".join(f"{w} {f}" for w, f in fails.items())
        )


if __name__ == "__main__":
    main()
