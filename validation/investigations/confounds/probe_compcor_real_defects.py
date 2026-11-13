# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's real CompCor calls under larmorx's default: which of the 108 ``acompcor``,
``crowncompcor`` and ``tcompcor`` nodes of the oracle runs does the default change, and how?

For every such node of ``oracles/fmriprep-25.2.6`` (its inputs and settings from the node's
``_report/report.rst``, parsed as ``check_compcor.py`` parses them):
``larmorx.confounds.acompcor``/``tcompcor`` with fMRIPrep 25.2.6's libraries (numpy 2.2.6, SciPy
1.15.2) in both modes; prints the default's notices (GLIBC-07: the cosine basis' correctly
rounded ``cos`` against glibc's) and how many values of each file differ from ``reproduce``'s
(the components' and the pre-filter basis' 10-decimal values, each changed component column told
apart as a sign flip (an SVD's sign is arbitrary) or another change; the metadata's text lines). larmorx
alone (no original runs): any environment with the worktree's extension::

    PYTHONPATH=<worktree>/python python -P probe_compcor_real_defects.py [--quick]
"""

from __future__ import annotations

import sys
import tempfile
import warnings
from collections import Counter
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_compcor as cc  # the node parser

import larmorx as lx


def table(p: Path) -> np.ndarray:
    lines = p.read_text().splitlines()
    rows = [line.split("\t") for line in lines[1:] if line]
    return np.array(rows, dtype=float) if rows else np.zeros((0, 0))


def main() -> None:
    quick = "--quick" in sys.argv
    nodes = cc.real_nodes()
    if quick:
        nodes = [n for n in nodes if any(n[0].startswith(q) for q in cc.QUICK_REAL)]
    fns = {
        "acompcor": lx.confounds.acompcor,
        "crowncompcor": lx.confounds.acompcor,
        "tcompcor": lx.confounds.tcompcor,
    }
    totals: Counter = Counter()
    with tempfile.TemporaryDirectory(prefix="lx-probe-compcor-") as tmp:
        tmp = Path(tmp)
        for i, (name, run, node, _, kv) in enumerate(nodes):
            inputs = cc.parse_inputs(run, kv)
            out = {}
            ids: set = set()
            for mode in ("reproduce", "fix"):
                d = tmp / f"{i}-{mode}"
                with warnings.catch_warnings(record=True) as caught:
                    warnings.simplefilter("always")
                    r = fns[node](
                        **inputs,
                        out_dir=d,
                        numpy_version="2.2.6",
                        scipy_version="1.15.2",
                        upstream_defects=mode,
                        n_threads=8,
                    )
                if mode == "fix":
                    ids = {
                        w.message.entry
                        for w in caught
                        if isinstance(w.message, lx.UpstreamDefectWarning)
                    }
                out[mode] = r
            a, b = out["reproduce"], out["fix"]
            ca, cb = table(Path(a.components_file)), table(Path(b.components_file))
            comp = int(np.sum(ca != cb)) if ca.shape == cb.shape else -1
            # per column: equal, the sign flipped (an SVD's sign is arbitrary), or other (the
            # largest difference of the 10-decimal values)
            flipped, other, worst = 0, 0, 0.0
            if comp > 0:
                for j in range(ca.shape[1]):
                    x, y = ca[:, j], cb[:, j]
                    if np.array_equal(x, y):
                        continue
                    if np.array_equal(x, -y):
                        flipped += 1
                    else:
                        other += 1
                        worst = max(worst, float(np.max(np.abs(np.abs(x) - np.abs(y)))))
            basis = 0
            if a.pre_filter_file and b.pre_filter_file:
                pa, pb = table(Path(a.pre_filter_file)), table(Path(b.pre_filter_file))
                basis = int(np.sum(pa != pb)) if pa.shape == pb.shape else -1
            meta = 0
            if a.metadata_file and b.metadata_file:
                la = Path(a.metadata_file).read_text().splitlines()
                lb = Path(b.metadata_file).read_text().splitlines()
                meta = sum(x != y for x, y in zip(la, lb, strict=False)) + abs(len(la) - len(lb))
            totals["nodes"] += 1
            totals["changed"] += bool(comp or basis or meta)
            totals["sign-flipped columns"] += flipped
            totals["other columns"] += other
            for k in ids:
                totals[k] += 1
            print(
                f"{name}: notices {sorted(ids) or '-'}; differ from reproduce: components "
                f"{comp} of {ca.size} (columns: {flipped} sign-flipped, {other} other, largest "
                f"|difference| of the magnitudes {worst:.1e}), pre-filter {basis}, metadata lines "
                f"{meta}",
                flush=True,
            )
    print("totals: " + ", ".join(f"{k} {v}" for k, v in sorted(totals.items())))


if __name__ == "__main__":
    main()
