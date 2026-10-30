# SPDX-License-Identifier: Apache-2.0
"""The parity smoke tier (``pytest tests/parity``) in parallel, with pytest-xdist.

Usage (from a checkout, with its ``python/`` and ``validation/src`` on ``PYTHONPATH``):

    python -P scripts/orchestration/parity_smoke.py [--workers N] [--serial]
        [--junit-dir DIR] [-- <more pytest arguments>]

Two parts, one after the other:

1. every test file of ``tests/parity`` except ``conftest.SERIAL`` on ``N`` pytest-xdist workers
   (default 16; ``--dist load``: each test goes to the next free worker);
2. the files of ``conftest.SERIAL`` (the memory peaks and the thread pools) alone, serially, so
   that nothing else runs on the machine while they measure.

The exit status is the worse of the two parts' (pytest's own codes). When both pass with no
extra arguments, from a clean commit, the smoke tier's marker is written as a serial
``pytest tests/parity`` writes it (``tests/parity/conftest.py``, ``write_smoke_marker``), which
``scripts/record_golden.py`` reuses. ``--serial`` runs ``pytest tests/parity`` itself, as before
2026-10-30 (the fallback; also taken when pytest-xdist is not installed). ``--junit-dir`` keeps
each part's JUnit XML (``parallel.xml``, ``serial.xml``; or ``whole.xml`` with ``--serial``):
``validation/investigations/replica-side/compare_outcomes.py`` compares two runs test by test.

Every worker is a separate process with its own temporary directories (``tempfile``,
pytest's per-worker ``basetemp``); the oracle-output cache writes atomically and locks each key
(``docs/validation/oracle-cache.md``), so concurrent runs of the same original program are safe.
The last line is ``PARITY-SMOKE exit <status> ...``.
"""

from __future__ import annotations

import argparse
import importlib.util
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PARITY = ROOT / "tests" / "parity"


def _conftest():
    spec = importlib.util.spec_from_file_location("parity_conftest", PARITY / "conftest.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def junit_counts(path: Path) -> dict[str, int]:
    """``passed``, ``skipped`` (xfailed included, as pytest's summary counts them apart) and
    ``failed`` (errors included) of a JUnit XML file, and ``collected``."""
    counts = {"passed": 0, "skipped": 0, "failed": 0, "xfailed": 0, "collected": 0}
    for case in ET.parse(path).getroot().iter("testcase"):
        counts["collected"] += 1
        kinds = {child.tag for child in case}
        skipped = case.find("skipped")
        if "failure" in kinds or "error" in kinds:
            counts["failed"] += 1
        elif skipped is not None and skipped.get("type") == "pytest.xfail":
            counts["xfailed"] += 1
        elif skipped is not None:
            counts["skipped"] += 1
        else:
            counts["passed"] += 1
    return counts


def _pytest(args: list[str], junit: Path) -> tuple[int, float]:
    start = time.monotonic()
    cmd = [sys.executable, "-m", "pytest", "-q", *args, f"--junitxml={junit}"]
    print("+", " ".join(cmd[1:]), flush=True)
    code = subprocess.run(cmd, cwd=ROOT, check=False).returncode
    return code, time.monotonic() - start


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--workers", type=int, default=16, help="pytest-xdist workers (16)")
    parser.add_argument("--serial", action="store_true", help="run pytest tests/parity serially")
    parser.add_argument("--junit-dir", type=Path, help="keep each part's JUnit XML here")
    parser.add_argument("extra", nargs="*", help="more pytest arguments (after --)")
    a = parser.parse_args(argv)
    junit_dir = a.junit_dir or Path(tempfile.mkdtemp(prefix="lx-parity-smoke_"))
    junit_dir.mkdir(parents=True, exist_ok=True)
    have_xdist = importlib.util.find_spec("xdist") is not None
    if a.serial or not have_xdist:
        if not a.serial:
            print("pytest-xdist is not installed: running the smoke tier serially", flush=True)
        code, seconds = _pytest(["tests/parity", *a.extra], junit_dir / "whole.xml")
        print(f"PARITY-SMOKE exit {code} (serial, {seconds:.0f} s)", flush=True)
        return code
    conftest = _conftest()
    serial = [f"tests/parity/{name}" for name in conftest.SERIAL]
    missing = [s for s in serial if not (ROOT / s).is_file()]
    if missing:
        print(f"conftest.SERIAL names files that do not exist: {missing}", flush=True)
        return 4
    ignore = [f"--ignore={s}" for s in serial]
    p_code, p_sec = _pytest(
        ["-n", str(a.workers), "--dist", "load", "tests/parity", *ignore, *a.extra],
        junit_dir / "parallel.xml",
    )
    s_code, s_sec = _pytest([*serial, *a.extra], junit_dir / "serial.xml")
    # A selection (-k, -m, ...) may leave a part with no test (pytest's status 5).
    codes = [c for c in (p_code, s_code) if not (a.extra and c == 5)]
    code = max(codes, default=0)
    counts = {
        k: junit_counts(junit_dir / "parallel.xml")[k] + junit_counts(junit_dir / "serial.xml")[k]
        for k in ("passed", "skipped", "failed", "xfailed", "collected")
    }
    summary = ", ".join(f"{counts[k]} {k}" for k in ("passed", "skipped", "xfailed", "failed"))
    marker = None
    if code == 0 and not a.extra:
        marker = conftest.write_smoke_marker(
            {k: counts[k] for k in ("passed", "skipped", "failed")},
            counts["collected"],
            f"scripts/orchestration/parity_smoke.py ({a.workers} pytest-xdist workers, then "
            f"{', '.join(conftest.SERIAL)} serially)",
        )
        if marker is not None:
            print(f"parity smoke tier passed: marker {marker}", flush=True)
    print(
        f"PARITY-SMOKE exit {code} (parallel, {a.workers} workers: {p_sec:.0f} s, exit {p_code}; "
        f"then serial: {s_sec:.0f} s, exit {s_code}; {summary})",
        flush=True,
    )
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
