# SPDX-License-Identifier: Apache-2.0
"""The confounds' golden cases (``tests/golden/registry.py``) checked before their checksums are
recorded: each case selected by an id prefix is run twice (the second time below a folder named
``--subdir``, ``RUNNER~1`` by default, as GitHub's Windows runners' temporary paths) and once per
``--threads`` value (the registry's ``CONFOUNDS_THREADS``, which every confounds case passes as
``n_threads``); every fingerprint must be the same. ``--paths`` also checks that no output holds
the case's folder or a temporary path. Prints each case's time and output count, then
``TOTAL ... problems N``.

``python -P golden_cases.py py.confounds py.niworkflows.confounds py.niworkflows.utility
py.niworkflows.morphology py.larmorprepx.confounds py.skimage.ball.negative --threads 1,4
--paths`` (``PYTHONPATH=<repo>/python``, ``OPENBLAS_CORETYPE=Haswell``). Relay 2h-ii
(2026-11-12): 15 cases, 533 outputs, 0.38 s, 0 problems. ``--subdir "a b"`` (a blank in the path):
nipype's ANTs interfaces write the paths inside ``[ ... ]`` unquoted (and BET's input relative
after quoting it), so the shell splits them; since relay 3a (PIPE-80, rule A) larmorx's
``_nipype`` quotes them by default, so ``py.larmorprepx.confounds`` runs there too (its notices
then name PIPE-80, the only output that changes), while ``py.larmorprepx.confounds.reproduce``
and the ``reproduce`` part of the field maps' ``py.larmorprepx.fieldmaps`` fail there as nipype
does.
"""

import argparse
import importlib.util
import os
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location(
    "larmorx_golden_registry", REPO / "tests" / "golden" / "registry.py"
)
assert spec is not None and spec.loader is not None
reg = importlib.util.module_from_spec(spec)
sys.modules["larmorx_golden_registry"] = reg
spec.loader.exec_module(reg)


def run(cid: str, root: Path) -> tuple[dict, dict, float]:
    (root / "work").mkdir(parents=True)
    (root / "inputs").mkdir()
    t0 = time.perf_counter()
    out = reg.CASES[cid].run(reg.Context(root / "work", reg.Inputs(root / "inputs")))
    dt = time.perf_counter() - t0
    return reg.fingerprints(out), reg.expand(out), dt


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("prefixes", nargs="+")
    p.add_argument("--threads", default="")
    p.add_argument("--subdir", default="RUNNER~1")
    p.add_argument("--paths", action="store_true")
    a = p.parse_args()
    ids = sorted(c for c in reg.CASES if any(c.startswith(x) for x in a.prefixes))
    print(len(ids), "cases")
    problems, total, outputs = 0, 0.0, 0
    for cid in ids:
        with tempfile.TemporaryDirectory() as t:
            first, expanded, dt = run(cid, Path(t))
            if a.paths:
                for name, value in expanded.items():
                    if isinstance(value, bytes) and (t.encode() in value or b"/tmp/" in value):
                        problems += 1
                        print(f"  {cid}: {name} holds a temporary path")
        total += dt
        outputs += len(first)
        line = f"{cid}: {dt:.3f} s, {len(first)} outputs"
        with tempfile.TemporaryDirectory() as t:
            again, _, _ = run(cid, Path(t) / a.subdir)
        if again != first:
            problems += 1
            line += f"  NOT DETERMINISTIC {reg.compare(first, again)[:3]}"
        for n in [int(x) for x in a.threads.split(",") if x]:
            saved = reg.CONFOUNDS_THREADS
            reg.CONFOUNDS_THREADS = n
            try:
                with tempfile.TemporaryDirectory() as t:
                    other, _, _ = run(cid, Path(t))
            finally:
                reg.CONFOUNDS_THREADS = saved
            if other != first:
                problems += 1
                line += f"  DIFFERS AT {n} THREADS {reg.compare(first, other)[:3]}"
        print(line, flush=True)
    print(f"TOTAL {total:.3f} s, {outputs} outputs; problems {problems}")
    return 1 if problems else 0


if __name__ == "__main__":
    os.environ.setdefault("OPENBLAS_CORETYPE", "Haswell")
    sys.exit(main())
