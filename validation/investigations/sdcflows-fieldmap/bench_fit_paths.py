# SPDX-License-Identifier: Apache-2.0
"""The fit's two operators against each other (field maps part A, relay 8): wall time of
``lx.sdcflows.bspline_approx`` with the stored collocation matrix and with the weights evaluated
at every product (``low_memory=True``), at 1 and 14 threads, on fMRIPrep's ``bs_filter`` calls,
and the one-thread stored fit in a fresh process against the same fit after the benchmark's
earlier calls in its process.

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python \\
        python -P bench_fit_paths.py paths <run or node text> ...
    ... python -P bench_fit_paths.py process

``paths``: the median of three runs after a warm-up, per operator and thread count (forcing the
stored matrix at 14 threads through ``larmorx._core`` is not exposed: run it on a commit before
3c9c10af, where the stored matrix was the default at every thread count). ``process``: ds001600's
first fit at one thread, five runs in this fresh process, then ``bench_sdcflows.bench_fit`` on
the benchmark's first three calls (sdcflows' runs included) and five more runs. Run in the
newest Python environment, inside a memory box; nothing is compared (``bench_sdcflows.py``
checks the bits).
"""

from __future__ import annotations

import logging
import statistics
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_sdcflows as b

logging.disable(logging.CRITICAL)
import larmorx as lx  # noqa: E402


def files(run: str, node: str):
    base = b.R / f"{run}-noreconall" / b.W / node
    fmap = sorted((base / "phdiff_wf" / "compfmap").glob("*_fmap.nii.gz"))[0]
    return fmap, base / "magnitude_wf" / "brainextraction_wf" / "masker" / "clipped_mask.nii.gz"


def seconds(fmap, mask, n: int, low: bool = False) -> float:
    t0 = time.perf_counter()
    lx.sdcflows.bspline_approx(fmap, mask, n_threads=n, low_memory=low)
    return time.perf_counter() - t0


def paths(texts: list[str]) -> None:
    for run, node in b.FITS:
        if not any(k in f"{run}/{node}" for k in texts):
            continue
        fmap, mask = files(run, node)
        row = []
        for n in (1, 14):
            for low in (False, True):
                seconds(fmap, mask, n, low)
                t = statistics.median(seconds(fmap, mask, n, low) for _ in range(3))
                row.append(f"{n} thr {'low_memory' if low else 'default'} {t:.2f} s")
        print(f"{run} {node.rsplit('/', 1)[-1]}: " + "; ".join(row), flush=True)


def process() -> None:
    fmap, mask = files(*b.FITS[3])
    fresh = [round(seconds(fmap, mask, 1), 2) for _ in range(5)]
    print("ds001600 wf_auto_00000, one thread, fresh process:", fresh, flush=True)
    for run, node in b.FITS[:3]:
        b.bench_fit(run, node, [1, 14], 3, True)
    after = [round(seconds(fmap, mask, 1), 2) for _ in range(5)]
    print("the same after the benchmark's first three calls:", after, flush=True)


if __name__ == "__main__":
    if sys.argv[1:2] == ["paths"]:
        paths(sys.argv[2:])
    elif sys.argv[1:2] == ["process"]:
        process()
    else:
        sys.exit(__doc__)
