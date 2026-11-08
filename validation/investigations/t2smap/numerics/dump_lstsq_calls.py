# SPDX-License-Identifier: Apache-2.0
"""Records every ``numpy.linalg.lstsq`` call tedana's ``t2smap`` makes on a real input.

Runs tedana's own command line (``tedana.workflows.t2smap._main``) in this process with
``numpy.linalg.lstsq`` wrapped: each call's design ``a`` (float64), right-hand sides ``b``
(tedana's float32 ``log(|data| + 1)``, saved column by column) and numpy's results (x,
residuals, rank, s) at the BLAS thread count tedana set (``--n-threads``, 1 by default) go into
``--out`` as ``call<k>_{a,b,x,r,s}.bin`` and ``call<k>_meta.txt`` (m, nrhs, rank, BLAS threads).
``check_lstsq.py --real <out>`` then checks larmorx on them.

    OPENBLAS_CORETYPE=Haswell python -P dump_lstsq_calls.py --out <dir> --workdir <scratch> \\
        -- <t2smap arguments, e.g. -d e1.nii.gz e2.nii.gz e3.nii.gz -e 14 29.96 45.92
           --mask mask.nii.gz --fittype loglin>

The log-linear fit is the same call for ``--fittype loglin`` and for ``curvefit`` (whose
starting values it gives), so ``loglin`` is enough.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path

import numpy as np
from threadpoolctl import threadpool_info


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--workdir", type=Path, required=True)
    ap.add_argument("t2smap_args", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    if os.environ.get("DUMP_LSTSQ_GUARD"):
        raise SystemExit("recursion guard")
    os.environ["DUMP_LSTSQ_GUARD"] = "1"
    out = args.out
    out.mkdir(parents=True, exist_ok=True)
    original = np.linalg.lstsq
    calls = []

    def recording_lstsq(a, b, rcond=None):
        result = original(a, b, rcond=rcond)
        k = len(calls)
        a64 = np.asarray(a, dtype=np.float64)
        m, nrhs = np.shape(b)
        blas = [i.get("num_threads") for i in threadpool_info() if i.get("user_api") == "blas"]
        np.ascontiguousarray(a64, dtype="<f8").tofile(out / f"call{k}_a.bin")
        np.ascontiguousarray(np.asarray(b).T, dtype="<f4").tofile(out / f"call{k}_b.bin")
        x, res, rank, s = result
        np.ascontiguousarray(x, dtype="<f8").tofile(out / f"call{k}_x.bin")
        np.ascontiguousarray(res, dtype="<f8").tofile(out / f"call{k}_r.bin")
        np.ascontiguousarray(s, dtype="<f8").tofile(out / f"call{k}_s.bin")
        (out / f"call{k}_meta.txt").write_text(
            f"{m} {nrhs} {int(rank)} {blas} {np.asarray(b).dtype} {rcond}\n", encoding="utf-8"
        )
        calls.append((m, nrhs))
        return result

    np.linalg.lstsq = recording_lstsq
    from tedana.workflows import t2smap

    argv = [a for a in args.t2smap_args if a != "--"]
    args.workdir.mkdir(parents=True, exist_ok=True)
    t2smap._main([*argv, "--out-dir", str(args.workdir), "--overwrite"])
    print(f"recorded {len(calls)} calls: {calls}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
