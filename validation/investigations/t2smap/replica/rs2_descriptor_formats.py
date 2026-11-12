# SPDX-License-Identifier: Apache-2.0
"""The replica's random-testing descriptor (``difftest/tools/tedana_t2smap.py``) on its ANALYZE
and MGH draws only (relay RS2, 2026-11-12): recipes are drawn for the keys ``rs2-formats-<k>``
until ``--cases`` of them store their echoes as ANALYZE or MGH, and each runs through the
descriptor's own comparison (the replica suite's: both environments, ``reproduce`` bit for bit,
the default pass)::

    PYTHONPATH=<worktree>/python:<worktree>/validation/src LARMORX_GPL_BIN=<larmorx-gpl> \\
        python -P rs2_descriptor_formats.py --cases 24
"""

from __future__ import annotations

import argparse
import sys

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.tools import tedana_t2smap as tool
from larmorx_validation.parity.harness import CheckList


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--cases", type=int, default=24)
    a = ap.parse_args()
    found = bad = 0
    k = 0
    while found < a.cases and k < 5000:
        k += 1
        recipe = tool.draw(gen.Draw(f"rs2-formats-{k}"))
        st = recipe["extra"]["storage"]
        if st["suffix"] not in ("analyze", "mgh"):
            continue
        found += 1
        recipe.update(tool="tedana-t2smap", seed="rs2-formats", n=k)
        case = tool.case(recipe, "replica")
        checks = CheckList()
        status = "pass"
        try:
            tool.run_random(case, checks)
        except Exception as e:  # an outcome of the harness (both reject, skipped)
            status = f"{type(e).__name__}: {getattr(e, 'reason', e)}"[:160]
        failed = [c for c in checks.checks if not c.passed]
        bad += bool(failed)
        mask = recipe["extra"].get("mask", {}).get("suffix", "none")
        print(
            f"{'DIFF' if failed else 'ok  '} rs2-formats-{k} {st['suffix']} {st['dtype']} "
            f"mat={st.get('mat')} mask={mask}: {status}; "
            + "; ".join(f"{c.name}: {str(c.detail)[:200]}" for c in failed),
            flush=True,
        )
    print(f"{found} cases, {bad} differ")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
