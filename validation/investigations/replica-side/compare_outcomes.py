# SPDX-License-Identifier: Apache-2.0
"""Compare two runs of the parity smoke tier test by test, from their JUnit XML files.

Usage:
    python -P compare_outcomes.py --a <xml>... --b <xml>... [--out result.json]

``--a`` and ``--b`` each take the JUnit XML files of one run (a serial ``pytest tests/parity``
gives one, ``scripts/orchestration/parity_smoke.py`` two: its parallel part and its serial part).
Every test id (the JUnit ``classname`` and ``name``) gets one outcome: ``passed``, ``failed``,
``error``, ``skipped`` or ``xfailed``. The runs agree when both hold the same test ids with the
same outcome each; skip and xfail reasons are compared too and reported. Exit status 0 when the
outcomes agree, 1 otherwise.
"""

from __future__ import annotations

import argparse
import json
import sys
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path


def outcomes(paths: list[Path]) -> dict[str, tuple[str, str]]:
    """``{test id: (outcome, reason)}`` of the JUnit files ``paths`` (a test id twice: an error)."""
    out: dict[str, tuple[str, str]] = {}
    for path in paths:
        for case in ET.parse(path).getroot().iter("testcase"):
            tid = f"{case.get('classname', '')}::{case.get('name', '')}"
            if tid in out:
                raise SystemExit(f"{tid} appears twice")
            failure, error, skipped = (case.find(t) for t in ("failure", "error", "skipped"))
            if failure is not None:
                out[tid] = ("failed", "")
            elif error is not None:
                out[tid] = ("error", "")
            elif skipped is not None:
                kind = "xfailed" if skipped.get("type") == "pytest.xfail" else "skipped"
                out[tid] = (kind, skipped.get("message", ""))
            else:
                out[tid] = ("passed", "")
    return out


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--a", nargs="+", type=Path, required=True, help="the first run's XML files")
    p.add_argument("--b", nargs="+", type=Path, required=True, help="the second run's XML files")
    p.add_argument("--out", type=Path, help="write the comparison as JSON")
    args = p.parse_args(argv)
    a, b = outcomes(args.a), outcomes(args.b)
    only_a = sorted(set(a) - set(b))
    only_b = sorted(set(b) - set(a))
    both = sorted(set(a) & set(b))
    differ = [t for t in both if a[t][0] != b[t][0]]
    reasons = [t for t in both if a[t][0] == b[t][0] and a[t][1] != b[t][1]]
    result = {
        "a": [str(x) for x in args.a],
        "b": [str(x) for x in args.b],
        "tests": {"a": len(a), "b": len(b), "both": len(both)},
        "outcomes_a": dict(Counter(o for o, _ in a.values())),
        "outcomes_b": dict(Counter(o for o, _ in b.values())),
        "only_in_a": only_a,
        "only_in_b": only_b,
        "outcome_differs": [{"test": t, "a": a[t][0], "b": b[t][0]} for t in differ],
        "reason_differs": [{"test": t, "a": a[t][1], "b": b[t][1]} for t in reasons],
        "identical": not (only_a or only_b or differ),
    }
    print(f"tests: {len(a)} in a, {len(b)} in b, {len(both)} in both")
    print(f"outcomes a: {result['outcomes_a']}")
    print(f"outcomes b: {result['outcomes_b']}")
    for name in ("only_in_a", "only_in_b", "outcome_differs", "reason_differs"):
        print(f"{name}: {len(result[name])}")
        for item in result[name][:20]:
            print(f"  {item}")
    print("IDENTICAL OUTCOMES" if result["identical"] else "OUTCOMES DIFFER")
    if args.out:
        args.out.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    return 0 if result["identical"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
